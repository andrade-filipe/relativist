//! Compact wire representation of a partition's `Net` (L6 mitigation).
//!
//! The in-memory `Net` layout is dense: `agents: Vec<Option<Agent>>` and
//! `ports: Vec<PortRef>` are sized to `max_id + 1` and `(max_id + 1) * 3`
//! respectively, even when most slots are `None` or `DISCONNECTED`. Under
//! `ContiguousIdStrategy`, the last worker's subnet always spans the full
//! `max_id + 1` range regardless of how many live agents it actually owns,
//! which pushes large partitions past the 256 MiB protocol frame cap (see
//! `PHASE2-FINDINGS.md` L6).
//!
//! `CompactSubnet` stores only live agents and non-`DISCONNECTED` ports,
//! along with the arena sizes needed to rebuild the original dense layout
//! on the receiving side. It is used via serde's `serialize_with` /
//! `deserialize_with` on the `Partition::subnet` field — in-memory the
//! subnet is still a `Net`; only the wire form is compressed.
//!
//! Round-trip invariant: `Net -> CompactSubnet -> Net` preserves
//! `agents`, `ports`, `redex_queue`, `next_id`, and `root` byte-for-byte.
//! `freeport_redirects` is `#[serde(skip)]` on `Net` and is not carried.

use std::collections::VecDeque;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::net::{port_index, Agent, AgentId, Net, PortRef, DISCONNECTED, PORTS_PER_SLOT};

/// Sparse wire-only representation of a partition sub-`Net`.
///
/// Per live agent we carry `(id, agent, [p0, p1, p2])`. The three-port array
/// is serialised verbatim — `DISCONNECTED` entries pay only the enum tag, so
/// densely connected agents cost no more than the dense encoding while the
/// whole triple disappears for tombstoned slots.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(
    feature = "zero-copy",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub struct CompactSubnet {
    /// Target length of `Net::agents` after inflation (equals the original
    /// `agents.len()`). Stored explicitly so round-trip preserves the arena
    /// size, not just the live agent IDs.
    pub agent_arena_len: u32,

    /// Live slots, one entry per non-`None` agent:
    /// `(agent_id, agent, [port0, port1, port2])`. The receiver rebuilds
    /// the dense arenas by sizing to `agent_arena_len` and scattering each
    /// entry into its slot.
    pub live: Vec<(AgentId, Agent, [PortRef; PORTS_PER_SLOT])>,

    /// Full redex queue (small compared to agents/ports).
    pub redex_queue: VecDeque<(AgentId, AgentId)>,

    /// Next free agent id.
    pub next_id: AgentId,

    /// Optional root port.
    pub root: Option<PortRef>,

    /// SPEC-19 §3.4 R35a (added in D-011 Phase A, commit c4c80b8): mirrors
    /// `Net.free_list` (the recycled-id ledger — SPEC-22 R1, R10c). Stored
    /// as the LAST struct member so no earlier field's encoded position
    /// changes (R35a clause (a) — positioning is normative). LIFO ordering
    /// MUST be preserved across the round-trip (R35a clause (b)).
    ///
    /// Closes QA-D009-001: prior to R35a, the wire form silently dropped
    /// `free_list`, causing `next_id` divergence between coordinator and
    /// worker after every cross-worker partition transfer (SPEC-22 R10b/R12a
    /// violation).
    pub free_list: Vec<AgentId>,
}

impl CompactSubnet {
    /// Builds a compact view of `net` by filtering out `None` agent slots.
    /// Live agents keep all three port entries inline; `DISCONNECTED` ports
    /// on a live agent are preserved verbatim so full round-trip is exact.
    pub fn from_net(net: &Net) -> Self {
        let arena_len = net.agents.len();

        let mut live: Vec<(AgentId, Agent, [PortRef; PORTS_PER_SLOT])> = Vec::new();
        for (idx, slot) in net.agents.iter().enumerate() {
            if let Some(agent) = slot {
                let base = port_index(idx as AgentId, 0);
                let ports = [net.ports[base], net.ports[base + 1], net.ports[base + 2]];
                live.push((idx as AgentId, *agent, ports));
            }
        }

        Self {
            agent_arena_len: arena_len as u32,
            live,
            redex_queue: net.redex_queue.clone(),
            next_id: net.next_id,
            root: net.root,
            // SPEC-19 R35a clause (b): preserve free_list order verbatim
            // (LIFO stack — SPEC-22 R10c). Cloned because Net is borrowed.
            free_list: net.free_list.clone(),
        }
    }

    /// Inflates back into a dense `Net`, re-creating `agents` and `ports`
    /// arenas sized to `agent_arena_len` and filled with `None` /
    /// `DISCONNECTED` sentinels before applying the live entries.
    pub fn into_net(self) -> Net {
        let arena_len = self.agent_arena_len as usize;
        let mut agents: Vec<Option<Agent>> = vec![None; arena_len];
        let mut ports: Vec<PortRef> = vec![DISCONNECTED; arena_len * PORTS_PER_SLOT];

        for (id, agent, slot_ports) in self.live {
            let idx = id as usize;
            if idx < agents.len() {
                agents[idx] = Some(agent);
                let base = port_index(id, 0);
                if base + PORTS_PER_SLOT <= ports.len() {
                    ports[base] = slot_ports[0];
                    ports[base + 1] = slot_ports[1];
                    ports[base + 2] = slot_ports[2];
                }
            }
        }

        Net {
            agents,
            ports,
            redex_queue: self.redex_queue,
            next_id: self.next_id,
            root: self.root,
            freeport_redirects: std::collections::HashMap::new(),
            // SPEC-19 R35a (TASK-0596 / commit c4c80b8): restore the
            // free_list captured by `from_net` instead of the pre-fix
            // `Vec::new()` (which silently dropped the recycled-id ledger
            // — QA-D009-001 root cause).
            free_list: self.free_list,
            id_range: None,
            border_entries_shadow: None,
            recycle_policy: crate::net::core::RecyclePolicy::DisableUnderDelta,
            is_in_delta_round: false,
            streaming_active: false,
            // TASK-0598: counter fields always present (no cfg gate).
            protected_tombstones: None,
            free_list_pops: 0,
            free_list_pops_border: 0,
            free_list_pops_non_border: 0,
            // TASK-0601 (QA-D010-016): LIFO non-protected stalemate fallback counter.
            lifo_stalemate_fallbacks: 0,
        }
    }
}

/// serde `serialize_with` adapter used on `Partition::subnet`.
pub fn serialize_subnet_compact<S: Serializer>(
    net: &Net,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    CompactSubnet::from_net(net).serialize(serializer)
}

/// serde `deserialize_with` adapter used on `Partition::subnet`.
pub fn deserialize_subnet_compact<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Net, D::Error> {
    let compact = CompactSubnet::deserialize(deserializer)?;
    Ok(compact.into_net())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{Net, PortRef, Symbol};

    fn nets_equivalent(a: &Net, b: &Net) -> bool {
        a.agents == b.agents
            && a.ports == b.ports
            && a.redex_queue == b.redex_queue
            && a.next_id == b.next_id
            && a.root == b.root
            // SPEC-19 R35a (TASK-0596): free_list MUST be carried by CompactSubnet.
            // Without this conjunct the round-trip helper would silently green-light
            // a regression where the wire form drops the recycled-id ledger
            // (the original D-009 / QA-D009-001 bug).
            && a.free_list == b.free_list
    }

    // T1: Empty net round-trip preserves everything.
    #[test]
    fn test_empty_net_roundtrip() {
        let net = Net::new();
        let compact = CompactSubnet::from_net(&net);
        let back = compact.into_net();
        assert!(nets_equivalent(&net, &back));
    }

    // T2: Single agent round-trip.
    #[test]
    fn test_single_agent_roundtrip() {
        let mut net = Net::new();
        let _ = net.create_agent(Symbol::Era);
        let compact = CompactSubnet::from_net(&net);
        let back = compact.into_net();
        assert!(nets_equivalent(&net, &back));
    }

    // T3: Connected pair round-trip preserves port targets.
    #[test]
    fn test_connected_pair_roundtrip() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));

        let compact = CompactSubnet::from_net(&net);
        let back = compact.into_net();
        assert!(nets_equivalent(&net, &back));
    }

    // T4: Tombstone slot (remove_agent leaves agents[id] = None) is preserved.
    #[test]
    fn test_tombstone_slot_roundtrip() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Era);
        net.remove_agent(a);

        let compact = CompactSubnet::from_net(&net);
        let back = compact.into_net();

        // arena length must be preserved so that later create_agent continues
        // to allocate at the same next_id.
        assert_eq!(net.agents.len(), back.agents.len());
        assert_eq!(net.next_id, back.next_id);
        assert!(back.agents[a as usize].is_none());
        assert_eq!(back.agents[b as usize].map(|ag| ag.id), Some(b));
    }

    // T5: Bincode round-trip via the serde adapters matches the direct conversion.
    #[test]
    fn test_bincode_roundtrip_via_adapters() {
        let mut net = Net::new();
        let c1 = net.create_agent(Symbol::Con);
        let c2 = net.create_agent(Symbol::Con);
        let d = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(c1, 0), PortRef::AgentPort(d, 0));
        net.connect(PortRef::AgentPort(c1, 1), PortRef::AgentPort(c2, 1));
        net.connect(PortRef::AgentPort(c1, 2), PortRef::FreePort(42));

        let compact = CompactSubnet::from_net(&net);
        let bytes = crate::protocol::bincode_v2::encode(&compact).unwrap();
        let restored: CompactSubnet = crate::protocol::bincode_v2::decode_value(&bytes).unwrap();
        let back = restored.into_net();

        assert!(nets_equivalent(&net, &back));
    }

    // T6: Redex queue is preserved.
    #[test]
    fn test_redex_queue_preserved() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        // Connecting two principal ports enqueues the pair.
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        assert!(!net.redex_queue.is_empty());

        let compact = CompactSubnet::from_net(&net);
        let back = compact.into_net();
        assert_eq!(net.redex_queue, back.redex_queue);
    }

    // T7: Root is preserved.
    #[test]
    fn test_root_preserved() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.root = Some(PortRef::AgentPort(a, 0));

        let compact = CompactSubnet::from_net(&net);
        let back = compact.into_net();
        assert_eq!(net.root, back.root);
    }

    // T8: Compact form is actually smaller for sparse arenas.
    //
    // TASK-0596 update: post-SPEC-19 R35a the wire form carries `free_list`
    // (a `Vec<AgentId>` whose serialized cost scales with the number of
    // tombstones), so `compact * 3 < dense` is no longer the right ratio
    // when the partition has many tombstones with their ids parked in
    // free_list. The test's intent is to validate that the AGENTS ARENA
    // compression survives — we therefore drain `free_list` before
    // encoding (simulating the steady-state where tombstones have been
    // collapsed by the recycle policy, or the distributed-mode case where
    // recycling is disabled and `free_list` stays empty).
    #[test]
    fn test_compact_smaller_for_sparse() {
        // Create a net, grow the arena, then remove most agents to simulate
        // a last-worker subnet under ContiguousIdStrategy.
        let mut net = Net::new();
        let mut ids = Vec::new();
        for _ in 0..1000 {
            ids.push(net.create_agent(Symbol::Era));
        }
        // Keep only the last 10 live.
        for &id in &ids[..990] {
            net.remove_agent(id);
        }
        // Drain the free_list so this test isolates the agents-arena win
        // (post-R35a free_list is part of the wire form on both sides; see
        // doc-comment above).
        net.free_list.clear();

        let dense_bytes = crate::protocol::bincode_v2::encode(&net).unwrap();
        let compact = CompactSubnet::from_net(&net);
        let compact_bytes = crate::protocol::bincode_v2::encode(&compact).unwrap();

        assert!(
            compact_bytes.len() * 3 < dense_bytes.len(),
            "compact ({}) should be <<< dense ({}) for sparse net",
            compact_bytes.len(),
            dense_bytes.len()
        );
    }

    // -----------------------------------------------------------------------
    // TASK-0353 — rkyv round-trip for CompactSubnet (SPEC-18 §3.5).
    //
    // CompactSubnet has no PartialEq derive — we compare via inflation
    // back to a Net (which DOES implement PartialEq) plus the structural
    // fields that survive the wire form (`agent_arena_len`, `next_id`,
    // `root`, `redex_queue`, and the `live` count).
    //
    // CompactSubnet is the bincode v2 wire form of Partition.subnet but
    // its rkyv derive is wired up so the type can be archived directly
    // by anything that needs to (no production caller does today; the
    // derive guards against future regressions and exercises the test
    // cube fully).
    // -----------------------------------------------------------------------

    // -----------------------------------------------------------------------
    // TASK-0596 — SPEC-19 §3.4 R35a: CompactSubnet wire form MUST round-trip
    // `Net.free_list` (closes QA-D009-001). The bug-witness is UT-0596-02.
    //
    // SPEC dependencies asserted by these tests:
    //   - SPEC-19 R35a (commit c4c80b8): wire suffix + PROTOCOL_VERSION bump.
    //   - SPEC-22 R9a:  Net.free_list serde MUST be byte-for-byte preserved.
    //   - SPEC-22 R10b/R12a: next_id consistency across coordinator/worker.
    //   - SPEC-22 R10c: LIFO recycle order (Vec is order-sensitive, NOT a set).
    // -----------------------------------------------------------------------

    /// UT-0596-01: empty `free_list` round-trips through `CompactSubnet`.
    /// Catches a future regression where the new field is added but typed as
    /// `Option<Vec<_>>` and silently defaulted on the empty side.
    #[test]
    fn round_trip_with_empty_free_list() {
        let mut net = Net::new();
        let _ = net.create_agent(Symbol::Con);
        // free_list left as the default empty Vec.
        assert!(net.free_list.is_empty(), "precondition");

        let compact = CompactSubnet::from_net(&net);
        assert_eq!(
            compact.free_list,
            Vec::<AgentId>::new(),
            "UT-0596-01: from_net must capture an empty free_list verbatim",
        );

        let back = compact.into_net();
        assert!(
            back.free_list.is_empty(),
            "UT-0596-01: into_net must restore an empty free_list",
        );
        assert_eq!(back.free_list.len(), net.free_list.len());
        assert!(nets_equivalent(&net, &back));
    }

    /// UT-0596-02: THE BUG-WITNESS TEST. Headline regression for QA-D009-001.
    /// A populated `free_list` MUST survive `from_net -> into_net`. Before
    /// SPEC-19 R35a this fails because `into_net` hard-coded
    /// `free_list: Vec::new()`.
    #[test]
    fn round_trip_with_populated_free_list() {
        // CAVEAT (TASK-0596 fixture lesson): a `create_agent + remove_agent`
        // cycle DOES NOT grow `next_id` past the first round, because
        // `create_agent` recycles the popped id. Grow the arena by issuing
        // 10 fresh creates first, THEN remove the ones we want tombstoned,
        // THEN install the test-prescribed free_list verbatim.
        let mut net = Net::new();
        let mut all_ids = Vec::with_capacity(10);
        for _ in 0..10 {
            all_ids.push(net.create_agent(Symbol::Era));
        }
        // Live agents at ids {0, 4, 9}; remove the others to populate the arena
        // with tombstones. The auto-pushed free_list is overwritten below.
        let live: Vec<AgentId> = vec![0, 4, 9];
        for id in &all_ids {
            if !live.contains(id) {
                net.remove_agent(*id);
            }
        }
        net.free_list.clear();
        net.free_list = vec![7u32, 3u32, 1u32];
        assert_eq!(net.next_id, 10);

        let compact = CompactSubnet::from_net(&net);
        assert_eq!(
            compact.free_list,
            vec![7u32, 3u32, 1u32],
            "UT-0596-02: from_net must capture the populated free_list",
        );

        let back = compact.into_net();
        assert_eq!(
            back.free_list,
            vec![7u32, 3u32, 1u32],
            "UT-0596-02: into_net must restore the populated free_list \
             (BUG-WITNESS for QA-D009-001 — pre-R35a returned Vec::new())",
        );
        assert_eq!(back.free_list.len(), 3);
        // SPEC-22 R10b: next_id consistency.
        assert_eq!(back.next_id, 10);
        // Live agents preserved through the sparse path.
        for id in live {
            assert!(back.agents[id as usize].is_some());
        }
        assert!(nets_equivalent(&net, &back));
    }

    /// UT-0596-03: `free_list` is a LIFO stack (SPEC-22 R10c); element order
    /// is observable behavior. Asserts Vec equality, NOT set equality.
    #[test]
    fn round_trip_preserves_free_list_order() {
        let mut net = Net::new();
        // Grow arena to next_id = 10 via fresh creates (see CAVEAT above).
        let ids: Vec<AgentId> = (0..10).map(|_| net.create_agent(Symbol::Era)).collect();
        for id in ids {
            net.remove_agent(id);
        }
        net.free_list.clear();
        // Deliberately NOT sorted — order is the property under test.
        net.free_list = vec![5u32, 2u32, 8u32];

        let compact = CompactSubnet::from_net(&net);
        let back = compact.into_net();

        assert_eq!(
            back.free_list,
            vec![5u32, 2u32, 8u32],
            "UT-0596-03: order must be preserved verbatim (no sort, no dedup)",
        );
        // Top of stack: SPEC-22 R5/R10c specify push/pop at the END of the
        // Vec, so the LIFO top is the LAST element (the next id `create_agent`
        // would pop) — here `8` for `[5, 2, 8]`. The first element is the
        // BOTTOM of stack.
        assert_eq!(back.free_list.last().copied(), Some(8u32));
    }

    /// UT-0596-04: stress with sparse, non-monotonic `AgentId`s.
    /// Catches buggy implementations that treat `free_list` as a sorted set.
    #[test]
    fn round_trip_with_sparse_non_monotonic_free_list() {
        let mut net = Net::new();
        // Grow arena to length 100 via fresh creates (see CAVEAT above).
        let ids: Vec<AgentId> = (0..100).map(|_| net.create_agent(Symbol::Era)).collect();
        for id in ids {
            net.remove_agent(id);
        }
        net.free_list.clear();
        net.free_list = vec![97u32, 2u32, 50u32, 13u32];

        let compact = CompactSubnet::from_net(&net);
        let back = compact.into_net();

        assert_eq!(
            back.free_list,
            vec![97u32, 2u32, 50u32, 13u32],
            "UT-0596-04: scattered ids must round-trip verbatim",
        );
        // Every id stays inside the arena (no fabricated out-of-arena ids).
        let arena_len = back.agents.len() as AgentId;
        for &id in &back.free_list {
            assert!(
                id < arena_len,
                "UT-0596-04: round-tripped id {} >= arena_len {}",
                id,
                arena_len,
            );
        }
    }

    /// UT-0596-05: meta-test guarding the test-suite itself. Two nets that
    /// differ ONLY in `free_list` must be reported as not equivalent by the
    /// helper; otherwise UT-0596-01..04 could silently green-light a broken
    /// implementation that loses the field.
    #[test]
    fn nets_equivalent_helper_compares_free_list() {
        let mut a = Net::new();
        let _ = a.create_agent(Symbol::Era);
        let id = a.create_agent(Symbol::Era);
        a.remove_agent(id);
        let mut b = a.clone();
        // Construct deliberately divergent free_lists.
        a.free_list = vec![1u32];
        b.free_list = Vec::new();

        assert!(
            !nets_equivalent(&a, &b),
            "UT-0596-05: helper MUST distinguish nets that differ only in free_list",
        );
        // And the symmetric check: two nets agreeing on free_list and rest are equivalent.
        b.free_list = vec![1u32];
        assert!(nets_equivalent(&a, &b));
    }

    /// UT-0596-06 (zero-copy): rkyv archived form preserves `free_list`.
    /// Guards SPEC-18 R31/R33 — the bincode and rkyv paths must stay
    /// symmetric across the new field.
    #[cfg(feature = "zero-copy")]
    #[test]
    fn archived_round_trip_preserves_free_list() {
        let mut net = Net::new();
        // Grow arena via fresh creates (see CAVEAT above).
        let ids: Vec<AgentId> = (0..10).map(|_| net.create_agent(Symbol::Era)).collect();
        for id in ids {
            net.remove_agent(id);
        }
        net.free_list.clear();
        net.free_list = vec![7u32, 3u32, 1u32];
        let compact = CompactSubnet::from_net(&net);

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&compact).expect("serialize");
        let archived =
            rkyv::access::<rkyv::Archived<CompactSubnet>, rkyv::rancor::Error>(bytes.as_ref())
                .expect("access");
        let back: CompactSubnet =
            rkyv::deserialize::<CompactSubnet, rkyv::rancor::Error>(archived).expect("deserialize");

        assert_eq!(
            back.free_list,
            vec![7u32, 3u32, 1u32],
            "UT-0596-06: rkyv-archived form must carry free_list",
        );
        assert_eq!(back.into_net().free_list, net.free_list);
    }

    /// UT-0353-07: CompactSubnet round-trips via rkyv. Equality is checked
    /// by inflating both sides back to Net (which implements PartialEq).
    #[cfg(feature = "zero-copy")]
    #[test]
    fn rkyv_round_trip_compact_subnet_minimal() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(42));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(u32::MAX));

        let compact = CompactSubnet::from_net(&net);
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&compact).expect("serialize");
        let archived =
            rkyv::access::<rkyv::Archived<CompactSubnet>, rkyv::rancor::Error>(bytes.as_ref())
                .expect("access");
        let back: CompactSubnet =
            rkyv::deserialize::<CompactSubnet, rkyv::rancor::Error>(archived).expect("deserialize");

        assert_eq!(back.agent_arena_len, compact.agent_arena_len);
        assert_eq!(back.next_id, compact.next_id);
        assert_eq!(back.root, compact.root);
        assert_eq!(back.redex_queue, compact.redex_queue);
        assert_eq!(back.live.len(), compact.live.len());

        // Inflated nets must compare equal via the shared PartialEq impl.
        let original_inflated = compact.clone().into_net();
        let round_inflated = back.into_net();
        assert_eq!(
            original_inflated, round_inflated,
            "CompactSubnet -> rkyv -> CompactSubnet must inflate to an equal Net"
        );
    }
}
