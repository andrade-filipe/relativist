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

use std::collections::{HashSet, VecDeque};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::net::{port_index, Agent, AgentId, Net, PortRef, DISCONNECTED, PORTS_PER_SLOT};

/// QA-D011-002: errors raised when validating a `CompactSubnet` before
/// inflating it into a `Net`. Surfaced through `try_into_net` (the
/// validating constructor) and through `deserialize_subnet_compact`'s
/// custom-error path so a hostile or corrupted wire payload fails loudly
/// instead of silently corrupting the receiver.
///
/// SPEC-22 R4(b) and R10c forbid recycled-slot collisions and duplicate
/// free-list entries; SPEC-19 R35a (commit `c4c80b8`) is silent on
/// integrity validation, so this enum is the receiver-side enforcement
/// of those invariants on the wire path.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CompactSubnetError {
    /// A free-list id is `>= agent_arena_len`. SPEC-22 R10 forbids fabricated
    /// out-of-arena ids.
    #[error("QA-D011-002: free_list id {id} is out of bounds (agent_arena_len = {arena_len})")]
    FreeListIdOutOfBounds { id: AgentId, arena_len: u32 },

    /// A free-list id is `>= next_id`. SPEC-22 R10b/R10c: every recyclable id
    /// MUST have been issued previously by `create_agent` or `from_net`'s
    /// arena scan, both of which bound ids by `next_id`.
    #[error(
        "QA-D011-002: free_list id {id} is >= next_id ({next_id}); free-list cannot reference unallocated ids"
    )]
    FreeListIdAboveNextId { id: AgentId, next_id: AgentId },

    /// A free-list id appears more than once. SPEC-22 R10c: the LIFO is a
    /// sequence with NO duplicates (a duplicate would be popped twice and
    /// the second pop would land in a slot already restored to live state,
    /// violating R4(b)).
    #[error("QA-D011-002: free_list contains duplicate id {id}")]
    FreeListDuplicateId { id: AgentId },

    /// A free-list id overlaps with a live agent. SPEC-22 R4(b): a recycled
    /// slot must be `None` before pop. If id `i` is in `free_list` AND in
    /// `live`, the next `create_agent` pop would write into a live slot,
    /// destroying the previously-live agent. This is the QA-D011-002
    /// CRITICAL: silent in release builds (the `debug_assert!` is elided).
    #[error("QA-D011-002: free_list id {id} overlaps with a live agent (R4(b) violation)")]
    FreeListOverlapsLiveAgent { id: AgentId },
}

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

    /// Validates `free_list` integrity against `agent_arena_len`, `next_id`,
    /// and the live-agent set (QA-D011-002). Returns the first violation found
    /// (errors are mutually exclusive in practice; surfacing the first one keeps
    /// the diagnostic small and stable).
    ///
    /// SPEC-22 R4(b): recycled slot must be `None` before pop. SPEC-22 R10c:
    /// the LIFO is a sequence with no duplicates and no overlap with live
    /// agents. SPEC-22 R10: ids are bounded by the arena.
    ///
    /// This is the receiver-side wire-integrity check; the sender side is
    /// trusted to produce a valid `Net` (R4 invariant). On the wire path we
    /// MUST NOT trust the payload — a hostile or corrupted peer can craft a
    /// `free_list` whose entries overlap with live agents (silent corruption
    /// in release; debug-assert panic in debug). This validator catches all
    /// four corruption modes from the QA-D011 audit (EC-1.3, EC-1.4, EC-1.5,
    /// out-of-arena).
    pub fn validate_free_list(&self) -> Result<(), CompactSubnetError> {
        // Short-circuit for the common empty case.
        if self.free_list.is_empty() {
            return Ok(());
        }

        // Build the live-agent id set for O(1) overlap checks.
        let mut live_ids: HashSet<AgentId> = HashSet::with_capacity(self.live.len());
        for (id, _, _) in &self.live {
            live_ids.insert(*id);
        }

        let mut seen: HashSet<AgentId> = HashSet::with_capacity(self.free_list.len());
        for &id in &self.free_list {
            if id >= self.agent_arena_len {
                return Err(CompactSubnetError::FreeListIdOutOfBounds {
                    id,
                    arena_len: self.agent_arena_len,
                });
            }
            if id >= self.next_id {
                return Err(CompactSubnetError::FreeListIdAboveNextId {
                    id,
                    next_id: self.next_id,
                });
            }
            if !seen.insert(id) {
                return Err(CompactSubnetError::FreeListDuplicateId { id });
            }
            if live_ids.contains(&id) {
                return Err(CompactSubnetError::FreeListOverlapsLiveAgent { id });
            }
        }
        Ok(())
    }

    /// Inflates back into a dense `Net`, re-creating `agents` and `ports`
    /// arenas sized to `agent_arena_len` and filled with `None` /
    /// `DISCONNECTED` sentinels before applying the live entries.
    ///
    /// QA-D011-002: this method is infallible by API contract (multiple
    /// in-tree call sites depend on that) — but on the wire path that
    /// contract is too permissive: a hostile peer can craft a
    /// `free_list` with duplicates or live-overlapping ids, producing a
    /// `Net` that violates SPEC-22 R4(b)/R10c silently in release builds.
    /// We split the difference: `into_net` validates and SANITISES (drops
    /// the offending free-list entries) on corruption, emitting
    /// `tracing::error!` with the violation; the validating constructor
    /// [`Self::try_into_net`] returns `Err(CompactSubnetError)` instead.
    /// The serde deserialize adapter ([`deserialize_subnet_compact`]) uses
    /// `try_into_net` so the wire path fails loudly.
    pub fn into_net(self) -> Net {
        match self.validate_free_list() {
            Ok(()) => self.into_net_unchecked(),
            Err(e) => {
                tracing::error!(
                    error = %e,
                    "QA-D011-002: CompactSubnet free_list integrity violation; sanitising and proceeding. \
                     This indicates a hostile or corrupted wire payload — investigate the producer."
                );
                // Sanitise: drop free_list entries that violate any invariant
                // (out-of-arena, above next_id, duplicates, overlap with live).
                // The remaining valid entries preserve as much of the recycled-id
                // ledger as can be safely recovered.
                let mut sanitised = self;
                let arena_len = sanitised.agent_arena_len;
                let next_id = sanitised.next_id;
                let live_ids: HashSet<AgentId> =
                    sanitised.live.iter().map(|(id, _, _)| *id).collect();
                let mut seen: HashSet<AgentId> = HashSet::with_capacity(sanitised.free_list.len());
                sanitised.free_list.retain(|&id| {
                    id < arena_len && id < next_id && !live_ids.contains(&id) && seen.insert(id)
                });
                sanitised.into_net_unchecked()
            }
        }
    }

    /// Validating constructor (QA-D011-002): inflates a `CompactSubnet` into
    /// a `Net`, returning `Err(CompactSubnetError)` on `free_list` integrity
    /// violation. Used by the serde deserialize adapter so the wire path
    /// fails loudly on hostile or corrupted payloads.
    pub fn try_into_net(self) -> Result<Net, CompactSubnetError> {
        self.validate_free_list()?;
        Ok(self.into_net_unchecked())
    }

    /// Internal: the original infallible inflator, factored out so
    /// [`Self::into_net`] (sanitising) and [`Self::try_into_net`] (validating)
    /// share the same materialisation logic.
    fn into_net_unchecked(self) -> Net {
        let arena_len = self.agent_arena_len as usize;
        let mut agents: Vec<Option<Agent>> = vec![None; arena_len];
        let mut ports: Vec<PortRef> = vec![DISCONNECTED; arena_len * PORTS_PER_SLOT];

        let mut dropped_live = 0u32;
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
            } else {
                // QA-D011-011 (MEDIUM): surface dropped live agents instead
                // of silently discarding them. A `live` entry whose `id` is
                // beyond `agent_arena_len` is a wire corruption that the
                // sender's `from_net` cannot legitimately produce.
                dropped_live += 1;
            }
        }
        if dropped_live > 0 {
            tracing::error!(
                dropped = dropped_live,
                arena_len = arena_len,
                "QA-D011-011: CompactSubnet::into_net dropped {} live agent(s) with id >= agent_arena_len; \
                 wire payload is corrupted or hostile.",
                dropped_live
            );
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
///
/// QA-D011-002: invokes [`CompactSubnet::try_into_net`] so a hostile or
/// corrupted wire payload (e.g., `free_list` with duplicates or
/// live-overlapping ids) fails loudly via `serde::de::Error::custom`,
/// instead of silently producing a corrupted `Net` that violates SPEC-22
/// R4(b)/R10c. Note: the in-memory `into_net` path remains
/// sanitising-with-error-log, preserving in-tree caller assumptions.
pub fn deserialize_subnet_compact<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Net, D::Error> {
    use serde::de::Error;
    let compact = CompactSubnet::deserialize(deserializer)?;
    compact.try_into_net().map_err(|e| {
        tracing::error!(
            error = %e,
            "QA-D011-002: deserialize_subnet_compact rejected wire payload \
             (CompactSubnet free_list integrity violation)"
        );
        D::Error::custom(format!("CompactSubnet integrity violation: {e}"))
    })
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

    // -----------------------------------------------------------------------
    // QA-D011-002 (CRITICAL) — wire integrity validation in into_net /
    // try_into_net.
    //
    // Pre-fix: a hostile or corrupted wire payload with a free_list containing
    // duplicates, ids overlapping live agents, ids >= next_id, or ids beyond
    // agent_arena_len would produce a Net that violates SPEC-22 R4(b)/R10c.
    // In debug builds the next `create_agent` would panic via debug_assert!;
    // in release builds the live agent's slot would be silently overwritten
    // when the duplicate id is popped — irrecoverable corruption.
    //
    // Post-fix: `validate_free_list` is the receiver-side enforcement.
    // `try_into_net` returns `Err(CompactSubnetError)`; `into_net` sanitises
    // (drops bad entries) and emits `tracing::error!`; the serde
    // `deserialize_with` adapter uses `try_into_net` so the wire path fails
    // loudly.
    // -----------------------------------------------------------------------

    /// Build a `CompactSubnet` with a custom `free_list`, sized so all of
    /// `next_id`, `agent_arena_len`, and the live arena agree by construction.
    /// Helper for the adversarial QA-D011-002 cases below.
    fn make_compact_with_free_list(
        next_id: AgentId,
        live: Vec<(AgentId, Agent, [PortRef; PORTS_PER_SLOT])>,
        free_list: Vec<AgentId>,
    ) -> CompactSubnet {
        CompactSubnet {
            agent_arena_len: next_id,
            live,
            redex_queue: VecDeque::new(),
            next_id,
            root: None,
            free_list,
        }
    }

    /// QA-D011-002 — duplicate ids in `free_list` are rejected by
    /// `validate_free_list` and `try_into_net`.
    #[test]
    fn qa_d011_002_free_list_duplicate_id_is_rejected() {
        let live = vec![(
            0u32,
            Agent {
                symbol: Symbol::Era,
                id: 0,
            },
            [DISCONNECTED; PORTS_PER_SLOT],
        )];
        // free_list contains id 1 twice (next_id=3 means ids 0..3 are valid).
        let compact = make_compact_with_free_list(3, live, vec![1u32, 2u32, 1u32]);
        assert_eq!(
            compact.validate_free_list(),
            Err(CompactSubnetError::FreeListDuplicateId { id: 1 }),
            "QA-D011-002: validate_free_list MUST reject duplicate id"
        );
        assert!(
            matches!(
                compact.try_into_net(),
                Err(CompactSubnetError::FreeListDuplicateId { id: 1 })
            ),
            "QA-D011-002: try_into_net MUST propagate the duplicate-id error"
        );
    }

    /// QA-D011-002 — `free_list` overlapping with a live agent id is rejected.
    /// This is the headline corruption mode: in release, the next `create_agent`
    /// would silently overwrite the live agent.
    #[test]
    fn qa_d011_002_free_list_overlaps_live_agent_is_rejected() {
        let live = vec![(
            5u32,
            Agent {
                symbol: Symbol::Con,
                id: 5,
            },
            [DISCONNECTED; PORTS_PER_SLOT],
        )];
        // id 5 is BOTH live AND in free_list.
        let compact = make_compact_with_free_list(6, live, vec![5u32]);
        assert_eq!(
            compact.validate_free_list(),
            Err(CompactSubnetError::FreeListOverlapsLiveAgent { id: 5 }),
            "QA-D011-002: validate_free_list MUST reject live-overlap"
        );
        assert!(
            matches!(
                compact.try_into_net(),
                Err(CompactSubnetError::FreeListOverlapsLiveAgent { id: 5 })
            ),
            "QA-D011-002: try_into_net MUST propagate the live-overlap error"
        );
    }

    /// QA-D011-002 — `free_list` id >= `next_id` is rejected.
    /// Catches a payload that fabricates ids never issued by `create_agent`.
    #[test]
    fn qa_d011_002_free_list_id_above_next_id_is_rejected() {
        // next_id = 3 (ids 0..3 are valid), free_list contains id 5.
        // arena_len = next_id = 3, so id 5 is also out-of-bounds; the
        // out-of-bounds check fires first because the validator returns the
        // FIRST violation. Catch the OOB case here; the strict
        // "id >= next_id" branch is exercised in the dedicated test below.
        let compact = make_compact_with_free_list(3, vec![], vec![5u32]);
        assert_eq!(
            compact.validate_free_list(),
            Err(CompactSubnetError::FreeListIdOutOfBounds {
                id: 5,
                arena_len: 3,
            }),
            "QA-D011-002: out-of-bounds id (which is also >= next_id) MUST surface as OOB"
        );
    }

    /// QA-D011-002 — id strictly between `agent_arena_len` and `next_id`
    /// is impossible by construction (arena_len <= next_id is not enforced
    /// at the type level, but `from_net` always produces them equal). We
    /// can construct a CompactSubnet manually where they DIFFER, so this
    /// test exercises the dedicated `FreeListIdAboveNextId` branch.
    #[test]
    fn qa_d011_002_free_list_id_above_next_id_below_arena_is_rejected() {
        let compact = CompactSubnet {
            agent_arena_len: 100, // arena large enough
            live: vec![],
            redex_queue: VecDeque::new(),
            next_id: 3, // but only ids 0..3 ever issued
            root: None,
            free_list: vec![5u32], // 5 < 100 (in-bounds) but >= 3 (above issued range)
        };
        assert_eq!(
            compact.validate_free_list(),
            Err(CompactSubnetError::FreeListIdAboveNextId { id: 5, next_id: 3 }),
            "QA-D011-002: id below arena_len but above next_id MUST be rejected"
        );
    }

    /// QA-D011-002 — `into_net` (infallible, sanitising) drops bad entries
    /// and produces a Net that DOES NOT violate R4(b). Verify the produced
    /// Net is consistent: ids in the kept free_list are all `is_none()`.
    #[test]
    fn qa_d011_002_into_net_sanitises_corrupt_free_list() {
        let live = vec![(
            5u32,
            Agent {
                symbol: Symbol::Con,
                id: 5,
            },
            [DISCONNECTED; PORTS_PER_SLOT],
        )];
        // free_list contains: 5 (overlaps live), 99 (out-of-bounds), 5 again
        // (duplicate of overlapping), 2 (valid).
        let compact = make_compact_with_free_list(10, live, vec![5u32, 99u32, 5u32, 2u32]);
        let net = compact.into_net();

        // Only the valid entry survives.
        assert_eq!(
            net.free_list,
            vec![2u32],
            "QA-D011-002: into_net MUST sanitise — only valid entries survive"
        );
        // The live agent is preserved.
        assert!(
            net.agents[5].is_some(),
            "QA-D011-002: live agent must be preserved through sanitised inflation"
        );
        // The recovered free_list entry maps to a None slot.
        assert!(
            net.agents[2].is_none(),
            "QA-D011-002: surviving free_list id must point to a None slot (R4(b))"
        );
    }

    /// QA-D011-002 — round-trip integrity for valid payloads is unchanged.
    /// Regression sentinel: validation must not over-fire on legitimate inputs.
    #[test]
    fn qa_d011_002_valid_free_list_passes_validation() {
        let mut net = Net::new();
        let ids: Vec<AgentId> = (0..10).map(|_| net.create_agent(Symbol::Era)).collect();
        for id in &ids[..3] {
            net.remove_agent(*id);
        }
        // free_list now has the 3 removed ids; live agents are 3..10.
        let compact = CompactSubnet::from_net(&net);
        assert_eq!(
            compact.validate_free_list(),
            Ok(()),
            "QA-D011-002: legitimate `from_net` output MUST pass validation"
        );
        let back = compact.try_into_net().expect("try_into_net must succeed");
        assert!(nets_equivalent(&net, &back));
    }

    /// QA-D011-002 — bincode wire-path: a tampered payload (duplicate id in
    /// free_list) is rejected by the deserialize adapter, not silently
    /// inflated into a corrupt Net.
    #[test]
    fn qa_d011_002_bincode_deserialize_rejects_tampered_free_list() {
        // Encode a tampered CompactSubnet directly — bypasses from_net's
        // legitimate path, simulating a hostile peer.
        let tampered = make_compact_with_free_list(5, vec![], vec![1u32, 1u32]);
        let bytes = crate::protocol::bincode_v2::encode(&tampered).unwrap();

        // Direct decode produces the (untrusted) payload.
        let decoded: CompactSubnet = crate::protocol::bincode_v2::decode_value(&bytes).unwrap();
        // Validation rejects it.
        assert!(
            decoded.try_into_net().is_err(),
            "QA-D011-002: try_into_net MUST reject tampered free_list (duplicate id)"
        );
    }
}
