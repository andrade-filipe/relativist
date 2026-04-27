//! `remap_partition_ids` — renumbers a reclaimed partition into a fresh `IdRange`.
//!
//! This module implements SPEC-20 §3.8 A4 (owned by SPEC-04 next revision as R19a).
//! The function is the **only** supported caller of partition ID remapping; v1
//! reduction continues to require zero remaps (SPEC-04 R19).

use std::collections::HashMap;
use std::collections::VecDeque;

use crate::error::PartitionError;
use crate::net::{Agent, Net, PortRef, DISCONNECTED, PORTS_PER_SLOT};

use super::types::{IdRange, Partition};

/// Renumbers every agent in `partition` into consecutive IDs drawn from `new_range`.
///
/// # SPEC reference
///
/// SPEC-20 §3.8 A4 (owned by SPEC-04 next revision as R19a).
/// Required by SPEC-20 R30 (ID uniqueness preservation): when a reclaimed
/// partition is re-introduced via re-`split()`, the coordinator calls this
/// function to guarantee that the reclaimed partition's `AgentId` values are
/// disjoint from those of the surviving partitions, satisfying the
/// `Net::union` disjointness precondition (SPEC-20 §3.8 A7).
/// Only called from `AcceptingMembershipChanges` in the coordinator FSM;
/// workers never invoke it.
///
/// # Semantics
///
/// - Every live agent with old id `old_id` is assigned new id
///   `new_range.start + rank(old_id)`, where `rank` maps sorted old ids to
///   `0, 1, 2, …` in ascending order.
/// - Every internal `PortRef::AgentPort(old_id, port)` is rewritten to
///   `PortRef::AgentPort(new_id, port)` via the same mapping.
/// - `PortRef::FreePort(bid)` entries are left bit-identical: border IDs are
///   rebased separately via `PartitionPlan::allocate_border_ids` (A3) on a
///   distinct axis. SPEC-20 §4.2.2 orchestrates the two calls in order.
/// - The `id_range` field on the returned partition is set to `new_range`.
/// - `next_id` is set to `new_range.start + partition.agent_count()`.
///
/// See ARG-001 P4 (ID consistency) for the confluence argument showing that
/// structure-preserving renumbering does not violate D4 (ID Uniqueness).
///
/// # Errors
///
/// Returns `PartitionError::NewRangeTooSmall { partition_size, range_size }`
/// when `partition.agent_count() > new_range.len()`.
///
/// # Panics (debug only)
///
/// A `debug_assert!` fires when `new_range.start` overflows `u32` when
/// combined with `agent_count`. This mirrors the QA-001 boundary vigilance
/// applied to TASK-0410 (`Net::union`).
pub fn remap_partition_ids(
    partition: Partition,
    new_range: IdRange,
) -> Result<Partition, PartitionError> {
    // Guard against inverted ranges early: new_range.len() returns 0 via
    // saturating_sub for inverted ranges, which would mislead the caller
    // with a NewRangeTooSmall error rather than a clear "range is inverted"
    // diagnostic (RV-002 / QA-005).
    debug_assert!(
        new_range.start <= new_range.end,
        "remap_partition_ids: new_range is inverted (start={} > end={}); \
         caller supplied a malformed IdRange (QA-005)",
        new_range.start,
        new_range.end,
    );

    let agent_count = partition.agent_count();
    let range_size = new_range.len();

    // Validate range is large enough before mutating anything.
    if agent_count > range_size {
        return Err(PartitionError::NewRangeTooSmall {
            partition_size: agent_count,
            range_size,
        });
    }

    // Empty partition: trivially succeeds.
    // QA-004: Net::new() sets next_id = 0, but the documented postcondition
    // is `next_id = new_range.start + agent_count = new_range.start`.
    // Set next_id explicitly so any subsequent create_agent() mints IDs
    // from new_range.start, not from 0 (which would collide with surviving
    // partitions whose ranges overlap [0, new_range.start)).
    // QA-002: ..partition would preserve free_port_index byte-identical,
    // leaving stale AgentPort entries pointing at non-existent ids.
    // Clear it: an empty subnet has no live agents, so no valid boundary.
    if agent_count == 0 {
        let mut subnet = Net::new();
        subnet.next_id = new_range.start;
        return Ok(Partition {
            subnet,
            id_range: new_range,
            free_port_index: HashMap::new(),
            ..partition
        });
    }

    debug_assert!(
        new_range.start.checked_add(agent_count).is_some(),
        "remap_partition_ids: new_range.start + agent_count overflows u32"
    );

    // Build the old_id → new_id mapping: sort live ids ascending, assign
    // new_range.start, start+1, start+2, … in order.
    let mut live_ids: Vec<u32> = partition
        .subnet
        .agents
        .iter()
        .enumerate()
        .filter_map(|(i, slot)| slot.as_ref().map(|_| i as u32))
        .collect();
    live_ids.sort_unstable();

    let id_map: HashMap<u32, u32> = live_ids
        .iter()
        .enumerate()
        .map(|(rank, &old_id)| (old_id, new_range.start + rank as u32))
        .collect();

    // Compute the new arena size: max(new_id) + 1 = new_range.start + agent_count.
    let new_max_id = new_range.start + agent_count - 1;
    let arena_len = new_max_id as usize + 1;
    let ports_len = arena_len * PORTS_PER_SLOT;

    let mut new_agents: Vec<Option<Agent>> = vec![None; arena_len];
    let mut new_ports: Vec<PortRef> = vec![DISCONNECTED; ports_len];

    for (&old_id, &new_id) in &id_map {
        // Copy the agent into its new slot.
        let agent =
            partition.subnet.agents[old_id as usize].expect("live_ids contains only live agents");
        new_agents[new_id as usize] = Some(Agent {
            symbol: agent.symbol,
            id: new_id,
        });

        // Rewrite all port slots for this agent.
        for port_id in 0..PORTS_PER_SLOT {
            let old_port_idx = old_id as usize * PORTS_PER_SLOT + port_id;
            let new_port_idx = new_id as usize * PORTS_PER_SLOT + port_id;

            let old_target = if old_port_idx < partition.subnet.ports.len() {
                partition.subnet.ports[old_port_idx]
            } else {
                DISCONNECTED
            };

            // Rewrite AgentPort references; leave FreePort and DISCONNECTED as-is.
            let new_target = match old_target {
                PortRef::AgentPort(ref_id, port) => {
                    if let Some(&mapped) = id_map.get(&ref_id) {
                        PortRef::AgentPort(mapped, port)
                    } else {
                        // Reference to an agent outside the partition — should
                        // not occur in a well-formed partition (all internal
                        // AgentPort targets must be intra-partition per SPEC-04).
                        // A debug_assert fires in debug builds so that malformed
                        // inputs are caught early; in release builds the stale
                        // reference is preserved to avoid silent data loss
                        // (consistent with the spec invariant that this code
                        // path is unreachable on valid input — QA-003 / RV-005).
                        debug_assert!(
                            id_map.contains_key(&ref_id),
                            "remap_partition_ids: stray AgentPort target {} in port array at \
                             old_id={}, port={}; partition is malformed (QA-003)",
                            ref_id,
                            old_id,
                            port_id
                        );
                        old_target
                    }
                }
                // FreePort (border_id or Lafont) — preserved unchanged (A4 spec).
                PortRef::FreePort(_) => old_target,
            };

            new_ports[new_port_idx] = new_target;
        }
    }

    // Rebuild the redex queue: remap both agent ids.
    // Only pairs where both ids are in id_map are retained (same-partition pairs).
    // A well-formed partition from split() only contains intra-partition redex
    // pairs (SPEC-04 invariant). An asymmetric pair — where one element is live
    // and the other is not — indicates a malformed input; a debug_assert fires
    // in debug builds (QA-006).
    let new_redex_queue: VecDeque<(u32, u32)> = partition
        .subnet
        .redex_queue
        .iter()
        .filter_map(|&(a, b)| {
            let mapped_a = id_map.get(&a);
            let mapped_b = id_map.get(&b);
            debug_assert!(
                mapped_a.is_some() == mapped_b.is_some(),
                "remap_partition_ids: redex_queue entry ({}, {}) is asymmetric — \
                 one element is live and the other is not; SPEC-04 forbids cross-partition \
                 redex pairs (QA-006)",
                a,
                b,
            );
            Some((*mapped_a?, *mapped_b?))
        })
        .collect();

    // Rebuild free_port_index: border_ids are unchanged; the AgentPort values
    // on the partition side must be remapped.
    //
    // For a well-formed partition (as produced by `split()`), every
    // `free_port_index` entry's `AgentPort` target corresponds to a live agent.
    // Entries referencing dead agents are silently dropped (filter_map);
    // a `debug_assert!` fires in debug builds if this invariant is violated
    // (RV-005). This is the "drop-on-dead-agent" policy — deliberately
    // asymmetric with the port-array path (which preserves the stale reference)
    // because the index is reconstructable from the port array on demand.
    let new_free_port_index: HashMap<u32, PortRef> = partition
        .free_port_index
        .iter()
        .filter_map(|(&bid, &port_ref)| {
            let remapped = match port_ref {
                PortRef::AgentPort(old_id, port) => {
                    debug_assert!(
                        id_map.contains_key(&old_id),
                        "free_port_index entry bid={} points to dead agent {}; \
                         partition is malformed (RV-005)",
                        bid,
                        old_id,
                    );
                    id_map
                        .get(&old_id)
                        .map(|&new_id| PortRef::AgentPort(new_id, port))
                }
                // FreePort in the index is unusual but preserve it if present.
                PortRef::FreePort(_) => Some(port_ref),
            };
            remapped.map(|r| (bid, r))
        })
        .collect();

    let new_subnet = Net {
        agents: new_agents,
        ports: new_ports,
        redex_queue: new_redex_queue,
        next_id: new_range.start + agent_count,
        root: partition.subnet.root.and_then(|r| match r {
            PortRef::AgentPort(old_id, port) => id_map
                .get(&old_id)
                .map(|&new_id| PortRef::AgentPort(new_id, port)),
            other => Some(other),
        }),
        freeport_redirects: HashMap::new(),
        free_list: Vec::new(),
        id_range: None,
        border_entries_shadow: None,
        recycle_policy: crate::net::core::RecyclePolicy::DisableUnderDelta,
        is_in_delta_round: false,
        #[cfg(debug_assertions)]
        protected_tombstones: None,
    };

    Ok(Partition {
        subnet: new_subnet,
        worker_id: partition.worker_id,
        free_port_index: new_free_port_index,
        id_range: new_range,
        border_id_start: partition.border_id_start,
        border_id_end: partition.border_id_end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::Symbol;

    // Build a minimal partition: 2 Con agents at ids [0, 1],
    // one internal wire (p0 aux1 ↔ p1 aux1), one FreePort(10) on p0 aux2.
    fn make_two_agent_partition() -> Partition {
        let arena_len = 2;
        let mut agents = vec![None; arena_len];
        let mut ports = vec![DISCONNECTED; arena_len * PORTS_PER_SLOT];

        agents[0] = Some(Agent {
            symbol: Symbol::Con,
            id: 0,
        });
        agents[1] = Some(Agent {
            symbol: Symbol::Con,
            id: 1,
        });

        // Agent 0 aux1 ↔ Agent 1 aux1
        ports[1] = PortRef::AgentPort(1, 1);
        ports[PORTS_PER_SLOT + 1] = PortRef::AgentPort(0, 1);

        // Agent 0 aux2 → FreePort(10)
        ports[2] = PortRef::FreePort(10);

        let mut free_port_index = HashMap::new();
        free_port_index.insert(10u32, PortRef::AgentPort(0, 2));

        use std::collections::VecDeque;
        let subnet = Net {
            agents,
            ports,
            redex_queue: VecDeque::new(),
            next_id: 2,
            root: None,
            freeport_redirects: HashMap::new(),
            free_list: Vec::new(),
            id_range: None,
            border_entries_shadow: None,
            recycle_policy: crate::net::core::RecyclePolicy::DisableUnderDelta,
            is_in_delta_round: false,
            #[cfg(debug_assertions)]
            protected_tombstones: None,
        };

        Partition {
            subnet,
            worker_id: 0,
            free_port_index,
            id_range: IdRange { start: 0, end: 2 },
            border_id_start: 10,
            border_id_end: 11,
        }
    }

    // UT (inline): remap [0,2) → [100,102): ids become 100, 101.
    #[test]
    fn test_remap_basic_renumber() {
        let p = make_two_agent_partition();
        let new_range = IdRange {
            start: 100,
            end: 102,
        };
        let result = remap_partition_ids(p, new_range).expect("remap must succeed");

        assert_eq!(result.subnet.count_live_agents(), 2);
        // IDs must be exactly 100, 101.
        let mut ids: Vec<u32> = result
            .subnet
            .agents
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|_| i as u32))
            .collect();
        ids.sort();
        assert_eq!(ids, vec![100, 101]);
    }

    // UT: internal wire is rewritten.
    #[test]
    fn test_remap_rewrites_edges() {
        let p = make_two_agent_partition();
        let new_range = IdRange {
            start: 100,
            end: 102,
        };
        let result = remap_partition_ids(p, new_range).expect("remap must succeed");

        // Agent 100 aux1 must point to Agent 101 aux1, and vice versa.
        assert_eq!(
            result.subnet.ports[100 * PORTS_PER_SLOT + 1],
            PortRef::AgentPort(101, 1),
            "agent 100 aux1 must point to agent 101 aux1"
        );
        assert_eq!(
            result.subnet.ports[101 * PORTS_PER_SLOT + 1],
            PortRef::AgentPort(100, 1),
            "agent 101 aux1 must point to agent 100 aux1"
        );
    }

    // UT: FreePort(10) is preserved.
    #[test]
    fn test_remap_preserves_freeport() {
        let p = make_two_agent_partition();
        let new_range = IdRange {
            start: 100,
            end: 102,
        };
        let result = remap_partition_ids(p, new_range).expect("remap must succeed");

        // FreePort(10) was on agent 0 aux2; after remap agent 0 → 100.
        assert_eq!(
            result.subnet.ports[100 * PORTS_PER_SLOT + 2],
            PortRef::FreePort(10),
            "FreePort(10) must be unchanged"
        );
    }

    // UT: undersized range returns NewRangeTooSmall.
    #[test]
    fn test_remap_too_small_range() {
        let p = make_two_agent_partition(); // 2 agents
        let undersized = IdRange {
            start: 100,
            end: 101,
        }; // size 1 < 2
        let result = remap_partition_ids(p, undersized);
        assert!(
            matches!(
                result,
                Err(PartitionError::NewRangeTooSmall {
                    partition_size: 2,
                    range_size: 1,
                })
            ),
            "must return NewRangeTooSmall"
        );
    }

    // UT: free_port_index is remapped correctly.
    #[test]
    fn test_remap_updates_free_port_index() {
        let p = make_two_agent_partition();
        let new_range = IdRange {
            start: 200,
            end: 202,
        };
        let result = remap_partition_ids(p, new_range).expect("remap must succeed");

        // bid 10 was → AgentPort(0, 2); after remap agent 0 → 200.
        assert_eq!(
            result.free_port_index.get(&10),
            Some(&PortRef::AgentPort(200, 2)),
            "free_port_index must be updated to new id"
        );
    }
}
