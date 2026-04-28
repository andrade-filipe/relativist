//! Sparse interaction net — HashMap-backed alternative to `Net`.
//!
//! `SparseNet` is a construction-time and partition-time representation
//! that is strictly memory-proportional to live agents (no tombstones,
//! no trailing `None` slots). It mirrors `Net`'s public API but uses
//! `HashMap` storage.
//!
//! SPEC-22 §4.4 R13, R18, R29.
//! SPEC-22 R31: this module contains no `unsafe` blocks.

use std::collections::{HashMap, VecDeque};

use super::types::{total_ports, Agent, AgentId, PortId, PortRef, Symbol, DISCONNECTED};
use crate::net::PORTS_PER_SLOT;

static_assertions::assert_impl_all!(SparseNet: Send, Sync);

/// A sparse interaction net backed by `HashMap` storage.
///
/// Unlike `Net`, `SparseNet` stores only live agents and their live port
/// entries — there are no `None` tombstones in the arena and no trailing
/// `DISCONNECTED` slots in the port array. Memory is therefore strictly
/// proportional to the number of live agents (SPEC-22 R16).
///
/// `SparseNet` is the preferred representation during partition construction
/// and when the ID space is sparse (SPEC-22 R22). It is converted to a
/// dense `Net` before the reduction loop via `SparseNet::to_dense`.
///
/// SPEC-22 §4.4, R13, R18, R29.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SparseNet {
    /// Live agents keyed by `AgentId`. Only contains live (non-tombstone) agents.
    pub agents: HashMap<AgentId, Agent>,

    /// Port connections. `(agent_id, port_id) -> PortRef`.
    /// Only live ports are stored — no ERA auxiliary entries (R17 / I6 sparse),
    /// no DISCONNECTED entries.
    pub ports: HashMap<(AgentId, PortId), PortRef>,

    /// Active pairs pending reduction.
    pub redex_queue: VecDeque<(AgentId, AgentId)>,

    /// Next fresh `AgentId`. Strictly greater than any ID ever issued.
    pub next_id: AgentId,

    /// Root port reference. `None` for sub-nets / partitions.
    pub root: Option<PortRef>,

    /// FreePort-to-FreePort redirections during local reduction.
    ///
    /// Mirrors `Net::freeport_redirects`. `#[serde(skip)]` because this
    /// is partition-context runtime state, not on-wire data (D1c, SC-011).
    #[serde(skip)]
    pub freeport_redirects: HashMap<u32, PortRef>,
}

impl Default for SparseNet {
    fn default() -> Self {
        Self::new()
    }
}

impl SparseNet {
    /// Creates an empty `SparseNet` with no agents, ports, or redexes.
    ///
    /// SPEC-22 §4.5.
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            ports: HashMap::new(),
            redex_queue: VecDeque::new(),
            next_id: 0,
            root: None,
            freeport_redirects: HashMap::new(),
        }
    }

    /// Creates an empty `SparseNet` with pre-allocated bucket capacity.
    ///
    /// `agents` is pre-allocated with `capacity` buckets.
    /// `ports` is pre-allocated with `capacity * PORTS_PER_SLOT` buckets.
    ///
    /// SPEC-22 §4.5.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            agents: HashMap::with_capacity(capacity),
            ports: HashMap::with_capacity(capacity.saturating_mul(PORTS_PER_SLOT)),
            redex_queue: VecDeque::new(),
            next_id: 0,
            root: None,
            freeport_redirects: HashMap::new(),
        }
    }

    // ------------------------------------------------------------------
    // Core operations (SPEC-22 §4.5, R14-R17)
    // ------------------------------------------------------------------

    /// Creates an agent with the given symbol and returns its assigned ID.
    ///
    /// Inserts the agent into the `agents` HashMap and increments `next_id`.
    /// No port entries are created at this point — ports are established by
    /// calling `connect` later.
    ///
    /// Complexity: O(1) amortized (SPEC-22 R15).
    pub fn create_agent(&mut self, symbol: Symbol) -> AgentId {
        // QA-D009-013: guard against u32 overflow; explicit panic in both debug and release.
        assert!(
            self.next_id < u32::MAX,
            "AgentId space exhausted: next_id has reached u32::MAX ({})",
            u32::MAX
        );
        let id = self.next_id;
        self.next_id += 1;
        self.agents.insert(id, Agent { symbol, id });
        id
    }

    /// Creates an agent with the given symbol at a **specific** `AgentId`.
    ///
    /// Unlike [`create_agent`], this method inserts the agent at `id` without
    /// consuming `next_id`. It is used by the streaming pipeline
    /// (`PartitionAccumulator::add_agent`, TASK-0551) where the generator has
    /// pre-assigned IDs (R15 monotonicity contract).
    ///
    /// # Invariants
    ///
    /// - `id` MUST NOT already be present in `agents` (I3' uniqueness, SPEC-22
    ///   R14). In debug builds this is enforced by a `debug_assert!`. In release
    ///   builds the existing entry is silently overwritten (HashMap::insert).
    /// - `next_id` is updated to `max(next_id, id + 1)` so that subsequent
    ///   `create_agent()` calls continue to produce fresh IDs above `id`.
    ///
    /// Complexity: O(1) amortized (SPEC-22 R15).
    pub fn create_agent_at(&mut self, id: AgentId, symbol: Symbol) {
        debug_assert!(
            !self.agents.contains_key(&id),
            "create_agent_at: id {} already exists in SparseNet (I3' uniqueness violation)",
            id
        );
        self.agents.insert(id, Agent { symbol, id });
        // Keep next_id ahead of all assigned IDs so create_agent() stays fresh.
        if id >= self.next_id {
            self.next_id = id.saturating_add(1);
        }
    }

    /// Removes an agent from the net.
    ///
    /// Removes the agent's entry from `agents` and all port entries for its
    /// valid ports from `ports`. No tombstones are left (SPEC-22 R16).
    /// If the agent does not exist, this is a no-op (idempotent).
    ///
    /// Also purges any `freeport_redirects` entry keyed by this ID (SC-001).
    ///
    /// Complexity: O(ports) = O(1) since at most 3 ports per agent (R15).
    pub fn remove_agent(&mut self, id: AgentId) {
        if let Some(agent) = self.agents.remove(&id) {
            let num_ports = total_ports(agent.symbol);
            for p in 0..num_ports {
                if let Some(PortRef::AgentPort(tid, tp)) = self.ports.remove(&(id, p)) {
                    // Bidirectional removal: remove the reverse entry too.
                    self.ports.remove(&(tid, tp));
                }
            }
            // SC-001: purge stale freeport_redirects entry.
            self.freeport_redirects.remove(&id);
        }
    }

    /// Establishes a bidirectional connection between two ports.
    ///
    /// Inserts entries for both directions in `ports` (only for `AgentPort`
    /// endpoints — `FreePort` is represented by absence).
    ///
    /// ERA agents have arity 0; attempts to write port entries for ERA
    /// auxiliary slots (port 1, 2) are silently skipped (R17).
    ///
    /// If both endpoints are principal ports (port 0), the pair is enqueued
    /// as a redex (R14 parity with `Net::connect`).
    ///
    /// FreePort-to-FreePort redirections are tracked in `freeport_redirects`.
    ///
    /// Complexity: O(1) amortized (R15).
    pub fn connect(&mut self, a: PortRef, b: PortRef) {
        debug_assert_ne!(a, b, "SparseNet: same-port self-connection: {:?}", a);

        self.write_port(a, b);
        self.write_port(b, a);

        // Track FreePort-to-FreePort redirections (mirrors Net::connect).
        if let (PortRef::FreePort(fid_a), PortRef::FreePort(fid_b)) = (a, b) {
            if fid_a != u32::MAX {
                self.freeport_redirects.insert(fid_a, b);
            }
            if fid_b != u32::MAX {
                self.freeport_redirects.insert(fid_b, a);
            }
        }

        // Incremental redex detection.
        if let (PortRef::AgentPort(id_a, 0), PortRef::AgentPort(id_b, 0)) = (a, b) {
            self.redex_queue.push_back((id_a, id_b));
        }
    }

    /// Removes the bidirectional connection of a port.
    ///
    /// Removes both the forward and reverse entries from `ports`.
    /// If the port is already disconnected, this is a no-op.
    ///
    /// Complexity: O(1) amortized (R15).
    pub fn disconnect(&mut self, port: PortRef) {
        if let PortRef::AgentPort(id, p) = port {
            if let Some(PortRef::AgentPort(tid, tp)) = self.ports.remove(&(id, p)) {
                // Remove the reverse entry.
                self.ports.remove(&(tid, tp));
            }
        }
    }

    /// Returns the `PortRef` to which the given port is connected.
    ///
    /// For `AgentPort(id, p)`: returns `ports.get(&(id, p))` cloned, or
    /// `DISCONNECTED` if no entry exists.
    /// For `FreePort(_)`: returns `DISCONNECTED` (sparse representation;
    /// redirects are resolved via `freeport_redirects`).
    ///
    /// Complexity: O(1) amortized (R15).
    pub fn get_target(&self, port: PortRef) -> PortRef {
        match port {
            PortRef::AgentPort(id, p) => self.ports.get(&(id, p)).copied().unwrap_or(DISCONNECTED),
            PortRef::FreePort(_) => DISCONNECTED,
        }
    }

    /// Returns a reference to the agent with the given ID.
    ///
    /// Returns `None` if no live agent exists with that ID.
    /// Complexity: O(1) amortized (R15).
    pub fn get_agent(&self, id: AgentId) -> Option<&Agent> {
        self.agents.get(&id)
    }

    /// Returns a mutable reference to the agent with the given ID.
    ///
    /// Returns `None` if no live agent exists with that ID.
    /// Complexity: O(1) amortized (R15).
    pub fn get_agent_mut(&mut self, id: AgentId) -> Option<&mut Agent> {
        self.agents.get_mut(&id)
    }

    /// Returns `true` if the redex queue is empty.
    pub fn is_reduced(&self) -> bool {
        self.redex_queue.is_empty()
    }

    /// Returns the number of live agents.
    ///
    /// O(1) because `HashMap::len()` is O(1) (R14/R15).
    pub fn count_live_agents(&self) -> usize {
        self.agents.len()
    }

    /// Returns an iterator over all live agents.
    ///
    /// Iteration order is non-deterministic (HashMap ordering). Use only
    /// where ordering does not matter.
    pub fn live_agents(&self) -> impl Iterator<Item = &Agent> {
        self.agents.values()
    }

    // ------------------------------------------------------------------
    // Conversions (SPEC-22 §4.6, R20)
    // ------------------------------------------------------------------

    /// Hard upper bound on the dense arena slot count (QA-D009-005).
    ///
    /// Prevents a malformed or attacker-controlled `max_id` near `u32::MAX`
    /// from triggering a multi-GiB allocation. 16 million slots × 3 ports ×
    /// 8 bytes/slot ≈ 384 MiB — well within process limits for test and
    /// small-scale grid runs, and safely below the 256 MiB protocol frame cap.
    /// Large-scale deployment should use partition-scoped `id_range` calls with
    /// properly bounded ranges rather than single-arena whole-net conversions.
    pub const MAX_DENSE_ARENA_SLOTS: usize = 1 << 24; // 16_777_216

    /// Converts this `SparseNet` into a dense `Net`.
    ///
    /// Computes `max_id = agents.keys().max()`, allocates a `Vec<Option<Agent>>`
    /// of size `max_id + 1` and a port array of size `(max_id + 1) * 3`, then
    /// populates them from the sparse maps.
    ///
    /// Free-list population depends on `id_range`:
    /// - `Some(range)`: only `None` indices inside `[range.start, range.end)`
    ///   are added to the free-list (partition-context call).
    /// - `None`: all `None` indices in the arena `[0, arena_len)` are added
    ///   (whole-net call).
    ///
    /// Copies `redex_queue`, `next_id`, `root`, and `freeport_redirects`.
    ///
    /// # Errors
    ///
    /// - `Err(NetError::InvalidIdRange)` if `id_range.start > id_range.end` (QA-D009-006).
    /// - `Err(NetError::DenseAllocationExceedsThreshold)` if the computed `arena_len`
    ///   exceeds `MAX_DENSE_ARENA_SLOTS` (QA-D009-005 DoS guard).
    ///
    /// SPEC-22 §4.6 R20 (closes SC-006).
    pub fn to_dense(
        &self,
        id_range: Option<core::ops::Range<AgentId>>,
    ) -> Result<crate::net::Net, crate::error::NetError> {
        use crate::net::core::{Net, RecyclePolicy};
        use crate::net::types::port_index;

        // QA-D009-006: validate id_range before any arithmetic.
        if let Some(ref r) = id_range {
            if r.start > r.end {
                return Err(crate::error::NetError::InvalidIdRange {
                    start: r.start,
                    end: r.end,
                });
            }
        }

        // Determine arena size.
        // SPEC-22 R20: arena_len = max_id + 1, sized to fit all live agent IDs.
        // The arena is bounded by the highest live agent ID, NOT by the end of the
        // assigned id_range. Free-list scanning is clamped to agents that actually
        // exist in the arena; IDs above max_id are allocated freshly via next_id.
        //
        // QA-D009-005: do NOT extend arena_len to range.end — that was the source
        // of unbounded GiB allocations when range spans up to u32::MAX. The previous
        // max(max_id+1, range_end) design caused 34+ GiB allocation for a partition
        // with 1 live agent at ID=1 and range=[100000..u32::MAX).
        let max_id = self.agents.keys().max().copied().unwrap_or(0);
        let arena_len = max_id as usize + 1;

        // QA-D009-005: hard allocation cap — reject arenas that would allocate
        // more than MAX_DENSE_ARENA_SLOTS slots (≈384 MiB for agents+ports).
        // With the arena_len = max_id+1 fix above this guard is a belt-and-braces
        // defence against an adversarially large max_id (e.g., from a deserialized
        // SparseNet with a single agent at ID = u32::MAX - 1).
        let live_count = self.agents.len();
        if arena_len > Self::MAX_DENSE_ARENA_SLOTS {
            return Err(crate::error::NetError::DenseAllocationExceedsThreshold {
                arena_len,
                max: Self::MAX_DENSE_ARENA_SLOTS,
                live_count,
            });
        }

        // Allocate dense storage.
        let mut agents: Vec<Option<Agent>> = vec![None; arena_len];
        let mut ports: Vec<PortRef> = vec![DISCONNECTED; arena_len * PORTS_PER_SLOT];

        // Populate agents.
        for (&id, &agent) in &self.agents {
            agents[id as usize] = Some(agent);
        }

        // Populate port entries.
        for (&(id, p), &target) in &self.ports {
            let idx = port_index(id, p);
            if idx < ports.len() {
                ports[idx] = target;
            }
        }

        // Free-list construction (SC-006 fix, QA-D009-005 bounded).
        // Clamp hi to arena_len: IDs above max_id do not have arena slots and
        // are allocated freshly via next_id. The id_range upper bound may be much
        // larger than arena_len (e.g., a partition range of [100000..u32::MAX) for
        // a worker that holds agents at IDs 0-10). We only add None slots that
        // actually exist in the arena.
        let (lo, hi_unclamped) = match &id_range {
            Some(r) => (r.start as usize, r.end as usize),
            None => (0, arena_len),
        };
        let hi = hi_unclamped.min(arena_len);
        let lo = lo.min(arena_len); // lo is clamped too (defensive)
        let mut free_list = Vec::new();
        if lo < hi {
            for (i, slot) in agents[lo..hi].iter().enumerate() {
                if slot.is_none() {
                    free_list.push((lo + i) as AgentId);
                }
            }
        }

        let id_range_clone = id_range.clone();

        Ok(Net {
            agents,
            ports,
            redex_queue: self.redex_queue.clone(),
            next_id: self.next_id,
            root: self.root,
            freeport_redirects: self.freeport_redirects.clone(),
            free_list,
            id_range: id_range_clone,
            border_entries_shadow: None,
            recycle_policy: RecyclePolicy::DisableUnderDelta,
            is_in_delta_round: false,
            #[cfg(debug_assertions)]
            protected_tombstones: None,
            #[cfg(debug_assertions)]
            free_list_pops: 0,
        })
    }

    // ------------------------------------------------------------------
    // Debug assertions (SPEC-22 R26)
    // ------------------------------------------------------------------

    /// SPEC-22 R26: verify T1 (Port Linearity), I1 (Bidirectional Consistency),
    /// and I2 (Reference Validity) on the sparse representation.
    ///
    /// - **T1/I1 bidirectional:** for each `((a_id, p), q)` in `ports`, if `q`
    ///   is `AgentPort(b_id, b_p)`, then `ports.get(&(b_id, b_p)) == Some(&AgentPort(a_id, p))`.
    ///   Root-port exception: skip if `q == self.root`.
    /// - **I2 agent existence:** for every `AgentPort(id, _)` value in `ports`,
    ///   `agents.contains_key(&id)` is true.
    /// - **I2 port arity:** for every `AgentPort(id, p)` value, `p < total_ports(agents[&id].symbol)`.
    ///
    /// All checks are `debug_assert!` — zero cost in release builds.
    #[cfg(debug_assertions)]
    pub fn assert_invariants(&self) {
        for (&(a_id, a_p), &target) in &self.ports {
            if let PortRef::AgentPort(b_id, b_p) = target {
                // T1/I1: skip root-port exception.
                let root_match = self.root == Some(target);
                if !root_match {
                    debug_assert_eq!(
                        self.ports.get(&(b_id, b_p)),
                        Some(&PortRef::AgentPort(a_id, a_p)),
                        "SPEC-22 R26 I1 violation: ({},{}) -> ({},{}) but reverse missing",
                        a_id,
                        a_p,
                        b_id,
                        b_p
                    );
                }

                // I2: referenced agent must exist.
                debug_assert!(
                    self.agents.contains_key(&b_id),
                    "SPEC-22 R26 I2 violation: port value AgentPort({},{}) references missing agent",
                    b_id, b_p
                );

                // I2: referenced port must be within arity.
                if let Some(agent) = self.agents.get(&b_id) {
                    debug_assert!(
                        b_p < total_ports(agent.symbol),
                        "SPEC-22 R26 I2 violation: port AgentPort({},{}) exceeds arity {} of symbol {:?}",
                        b_id, b_p, total_ports(agent.symbol), agent.symbol
                    );
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Private helpers
    // ------------------------------------------------------------------

    /// Writes a single directed port entry, respecting ERA cleanliness (R17).
    ///
    /// ERA agents have only port 0 (principal); writes to port 1 or 2 of an
    /// ERA agent are silently dropped (R17 — sparse equivalent of I6).
    fn write_port(&mut self, port: PortRef, target: PortRef) {
        if let PortRef::AgentPort(id, p) = port {
            // QA-D009-014: catch misordered call sequences (connect before create_agent).
            debug_assert!(
                self.agents.contains_key(&id),
                "write_port: agent {} not found — connect called before create_agent?",
                id
            );
            // R17: skip ERA auxiliary port slots (port 1, 2 of ERA agents).
            if let Some(agent) = self.agents.get(&id) {
                if p >= total_ports(agent.symbol) {
                    return; // ERA aux (or any out-of-arity) — skip silently.
                }
            }
            // Non-DISCONNECTED target only (sparse: no DISCONNECTED entries stored).
            if target != DISCONNECTED {
                self.ports.insert((id, p), target);
            }
        }
        // FreePort as `port` — no-op (no slot in the sparse map).
    }
}

// ------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::Symbol;

    // ------------------------------------------------------------------
    // TEST-SPEC-0486 — SparseNet struct + constructors
    // ------------------------------------------------------------------

    /// UT-0486-01: `new()` initializes all fields to empty/default.
    #[test]
    fn sparse_new_initializes_empty() {
        let sn = SparseNet::new();
        assert!(sn.agents.is_empty(), "agents should be empty");
        assert!(sn.ports.is_empty(), "ports should be empty");
        assert!(sn.redex_queue.is_empty(), "redex_queue should be empty");
        assert_eq!(sn.next_id, 0, "next_id should be 0");
        assert!(sn.root.is_none(), "root should be None");
        assert!(
            sn.freeport_redirects.is_empty(),
            "freeport_redirects should be empty"
        );
    }

    /// UT-0486-02: `with_capacity(100)` pre-allocates buckets.
    #[test]
    fn sparse_with_capacity_pre_allocates_buckets() {
        let sn = SparseNet::with_capacity(100);
        assert!(
            sn.agents.capacity() >= 100,
            "agents capacity should be >= 100"
        );
        assert!(
            sn.ports.capacity() >= 100 * PORTS_PER_SLOT,
            "ports capacity should be >= 100 * PORTS_PER_SLOT"
        );
    }

    /// UT-0486-03: `Debug` formatting does not panic.
    #[test]
    fn sparse_derives_debug() {
        let sn = SparseNet::new();
        let s = format!("{:?}", sn);
        assert!(!s.is_empty());
    }

    /// UT-0486-04: `Clone` produces an equal copy.
    #[test]
    fn sparse_derives_clone() {
        let mut sn = SparseNet::new();
        for _ in 0..5 {
            sn.create_agent(Symbol::Con);
        }
        let sn2 = sn.clone();
        assert_eq!(sn, sn2);
    }

    /// UT-0486-05: `PartialEq`/`Eq` works correctly.
    #[test]
    fn sparse_derives_partial_eq_eq() {
        let sn1 = SparseNet::new();
        let sn2 = SparseNet::new();
        assert_eq!(sn1, sn2);

        let mut sn3 = SparseNet::new();
        sn3.create_agent(Symbol::Era);
        assert_ne!(sn1, sn3);
    }

    /// UT-0486-06: serde round-trip (smoke).
    #[test]
    fn sparse_derives_serialize_deserialize() {
        let mut sn = SparseNet::new();
        for _ in 0..5 {
            sn.create_agent(Symbol::Con);
        }
        let bytes = crate::protocol::bincode_v2::encode(&sn).unwrap();
        let sn2: SparseNet = crate::protocol::bincode_v2::decode_value(&bytes).unwrap();
        assert_eq!(sn.agents, sn2.agents);
        assert_eq!(sn.ports, sn2.ports);
        assert_eq!(sn.next_id, sn2.next_id);
        assert_eq!(sn.root, sn2.root);
    }

    /// UT-0486-07/08: freeport_redirects has #[serde(skip)] and no feature gate.
    /// (Structural compile-time test — if this file compiles, the field exists and
    /// no feature gate surrounds the struct.)
    #[test]
    fn freeport_redirects_field_present_with_serde_skip() {
        let sn = SparseNet::new();
        // Access the field — if it didn't exist, this wouldn't compile.
        assert!(sn.freeport_redirects.is_empty());
    }

    /// EC-1: `with_capacity(0)` does not panic.
    #[test]
    fn sparse_with_capacity_zero() {
        let _sn = SparseNet::with_capacity(0);
    }

    // ------------------------------------------------------------------
    // TEST-SPEC-0487 — SparseNet operations
    // ------------------------------------------------------------------

    /// UT-0487-01: create_agent inserts into HashMap and increments next_id.
    #[test]
    fn create_agent_inserts_into_hashmap() {
        let mut sn = SparseNet::new();
        let id = sn.create_agent(Symbol::Con);
        assert!(sn.agents.contains_key(&id));
        assert_eq!(id, 0);
        assert_eq!(sn.next_id, 1);
    }

    /// UT-0487-02: remove_agent removes from HashMap and clears port entries.
    #[test]
    fn remove_agent_removes_from_hashmap_and_ports() {
        let mut sn = SparseNet::new();
        let a = sn.create_agent(Symbol::Con);
        let b = sn.create_agent(Symbol::Con);
        // Principal-principal connect.
        sn.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        sn.remove_agent(a);
        assert!(!sn.agents.contains_key(&a), "agent 0 should be removed");
        assert!(
            !sn.ports.contains_key(&(a, 0)),
            "port (a,0) should be removed"
        );
        // Bidirectional: b's port should also be cleared.
        assert!(
            !sn.ports.contains_key(&(b, 0)),
            "port (b,0) should be cleared (bidirectional removal)"
        );
    }

    /// UT-0487-03: connect inserts bidirectional entries.
    #[test]
    fn connect_inserts_bidirectional_entries() {
        let mut sn = SparseNet::new();
        let a = sn.create_agent(Symbol::Con);
        let b = sn.create_agent(Symbol::Con);
        sn.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 1));
        assert_eq!(
            sn.ports.get(&(a, 0)),
            Some(&PortRef::AgentPort(b, 1)),
            "forward direction"
        );
        assert_eq!(
            sn.ports.get(&(b, 1)),
            Some(&PortRef::AgentPort(a, 0)),
            "reverse direction"
        );
    }

    /// UT-0487-04: connecting principal-principal enqueues a redex.
    #[test]
    fn connect_principal_principal_enqueues_redex() {
        let mut sn = SparseNet::new();
        let a = sn.create_agent(Symbol::Con);
        let b = sn.create_agent(Symbol::Con);
        sn.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        assert_eq!(sn.redex_queue.len(), 1);
    }

    /// UT-0487-05: connecting auxiliary ports does not enqueue a redex.
    #[test]
    fn connect_aux_aux_no_redex() {
        let mut sn = SparseNet::new();
        let a = sn.create_agent(Symbol::Con);
        let b = sn.create_agent(Symbol::Con);
        sn.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 2));
        assert!(sn.redex_queue.is_empty());
    }

    /// UT-0487-06: disconnect removes bidirectional entries.
    #[test]
    fn disconnect_removes_bidirectional_entries() {
        let mut sn = SparseNet::new();
        let a = sn.create_agent(Symbol::Con);
        let b = sn.create_agent(Symbol::Con);
        sn.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 1));
        sn.disconnect(PortRef::AgentPort(a, 0));
        assert!(!sn.ports.contains_key(&(a, 0)), "(a,0) should be removed");
        assert!(!sn.ports.contains_key(&(b, 1)), "(b,1) should be removed");
    }

    /// UT-0487-07: get_target returns the connected port.
    #[test]
    fn get_target_returns_some_for_connected_port() {
        let mut sn = SparseNet::new();
        let a = sn.create_agent(Symbol::Con);
        let b = sn.create_agent(Symbol::Con);
        sn.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 1));
        assert_eq!(
            sn.get_target(PortRef::AgentPort(a, 0)),
            PortRef::AgentPort(b, 1)
        );
    }

    /// UT-0487-08: get_target returns DISCONNECTED for a FreePort.
    #[test]
    fn get_target_returns_disconnected_for_freeport() {
        let sn = SparseNet::new();
        assert_eq!(sn.get_target(PortRef::FreePort(99)), DISCONNECTED);
    }

    /// UT-0487-09: get_agent returns Some for a live agent.
    #[test]
    fn get_agent_returns_some_for_live() {
        let mut sn = SparseNet::new();
        let id = sn.create_agent(Symbol::Con);
        assert_eq!(
            sn.get_agent(id),
            Some(&Agent {
                symbol: Symbol::Con,
                id: 0
            })
        );
    }

    /// UT-0487-10: get_agent returns None after remove_agent.
    #[test]
    fn get_agent_returns_none_for_removed() {
        let mut sn = SparseNet::new();
        let id = sn.create_agent(Symbol::Con);
        sn.remove_agent(id);
        assert!(sn.get_agent(id).is_none());
    }

    /// UT-0487-11: is_reduced is true when redex queue is empty.
    #[test]
    fn is_reduced_true_when_redex_queue_empty() {
        let sn = SparseNet::new();
        assert!(sn.is_reduced());
    }

    /// UT-0487-12: count_live_agents returns HashMap::len().
    #[test]
    fn count_live_agents_uses_hashmap_len() {
        let mut sn = SparseNet::new();
        for _ in 0..7 {
            sn.create_agent(Symbol::Era);
        }
        assert_eq!(sn.count_live_agents(), sn.agents.len());
        assert_eq!(sn.count_live_agents(), 7);
    }

    /// UT-0487-13: ERA auxiliary ports are NOT inserted on connect (R17).
    #[test]
    fn era_aux_ports_not_inserted_on_connect() {
        let mut sn = SparseNet::new();
        let era_id = sn.create_agent(Symbol::Era);
        let other_id = sn.create_agent(Symbol::Con);
        // Connect ERA principal to CON principal — valid.
        sn.connect(
            PortRef::AgentPort(era_id, 0),
            PortRef::AgentPort(other_id, 0),
        );
        // ERA auxiliary ports (1, 2) must NOT exist in the ports map.
        assert!(
            !sn.ports.contains_key(&(era_id, 1)),
            "ERA aux port 1 must not exist"
        );
        assert!(
            !sn.ports.contains_key(&(era_id, 2)),
            "ERA aux port 2 must not exist"
        );
    }

    /// UT-0487-14: live_agents iterator returns all live agents.
    #[test]
    fn live_agents_iterator_count() {
        let mut sn = SparseNet::new();
        for _ in 0..5 {
            sn.create_agent(Symbol::Con);
        }
        assert_eq!(sn.live_agents().count(), 5);
    }

    /// EC-3: remove_agent on non-existent agent is a no-op.
    #[test]
    fn remove_nonexistent_agent_is_noop() {
        let mut sn = SparseNet::new();
        sn.remove_agent(999); // Should not panic.
    }

    // ------------------------------------------------------------------
    // TEST-SPEC-0496 — SparseNet debug assertions
    // ------------------------------------------------------------------

    /// sparse_assert_invariants_passes_on_valid_net
    #[cfg(debug_assertions)]
    #[test]
    fn sparse_assert_invariants_passes_on_valid_net() {
        let mut sn = SparseNet::new();
        let a = sn.create_agent(Symbol::Con);
        let b = sn.create_agent(Symbol::Con);
        let c = sn.create_agent(Symbol::Dup);
        sn.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 0));
        sn.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(c, 0));
        sn.assert_invariants(); // must not panic
    }

    /// sparse_assert_invariants_catches_one_way_port_violation
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic]
    fn sparse_assert_invariants_catches_one_way_port_violation() {
        let mut sn = SparseNet::new();
        let a = sn.create_agent(Symbol::Con);
        let b = sn.create_agent(Symbol::Con);
        sn.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Artificially remove the reverse direction.
        sn.ports.remove(&(b, 0));
        sn.assert_invariants(); // should panic
    }

    /// sparse_assert_invariants_catches_dangling_agent_reference
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic]
    fn sparse_assert_invariants_catches_dangling_agent_reference() {
        let mut sn = SparseNet::new();
        let a = sn.create_agent(Symbol::Con);
        let b = sn.create_agent(Symbol::Con);
        sn.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Remove agent b without cleaning ports — creates dangling reference.
        sn.agents.remove(&b);
        sn.assert_invariants(); // should panic
    }

    /// sparse_assert_invariants_catches_oob_port_arity
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic]
    fn sparse_assert_invariants_catches_oob_port_arity() {
        let mut sn = SparseNet::new();
        let a = sn.create_agent(Symbol::Con);
        let era = sn.create_agent(Symbol::Era);
        // Manually insert an out-of-arity port entry for ERA (aux port 1).
        sn.ports.insert((era, 1), PortRef::AgentPort(a, 0));
        sn.ports.insert((a, 0), PortRef::AgentPort(era, 1)); // reverse
        sn.assert_invariants(); // should panic — ERA has no port 1
    }

    /// QA-D009-005: to_dense must return Err when attacker-controlled max_id
    /// would require a >MAX_DENSE_ARENA_SLOTS allocation (DoS guard).
    #[test]
    fn qa_d009_005_to_dense_rejects_attacker_max_id() {
        use crate::error::NetError;

        let mut sn = SparseNet::new();
        // Single agent at a very high ID — this would require a ~17 GiB arena.
        let high_id: u32 = SparseNet::MAX_DENSE_ARENA_SLOTS as u32 + 1;
        sn.agents.insert(
            high_id,
            crate::net::Agent {
                symbol: Symbol::Era,
                id: high_id,
            },
        );
        sn.next_id = high_id + 1;

        let result = sn.to_dense(None);
        assert!(
            result.is_err(),
            "QA-D009-005: to_dense with attacker-controlled high max_id must return Err"
        );
        match result.unwrap_err() {
            NetError::DenseAllocationExceedsThreshold { arena_len, max, .. } => {
                assert!(
                    arena_len > max,
                    "arena_len ({arena_len}) should exceed max ({max})"
                );
            }
            other => panic!("expected DenseAllocationExceedsThreshold, got {other:?}"),
        }
    }

    /// QA-D009-006: to_dense must return Err on inverted id_range (start > end).
    #[test]
    #[allow(clippy::reversed_empty_ranges)] // intentionally inverted range to test the guard
    fn qa_d009_006_to_dense_rejects_inverted_range() {
        use crate::error::NetError;

        let mut sn = SparseNet::new();
        sn.create_agent(Symbol::Era);

        let result = sn.to_dense(Some(50..10)); // start > end — intentionally inverted
        assert!(
            result.is_err(),
            "QA-D009-006: to_dense with inverted id_range must return Err"
        );
        match result.unwrap_err() {
            NetError::InvalidIdRange { start, end } => {
                assert_eq!(start, 50);
                assert_eq!(end, 10);
            }
            other => panic!("expected InvalidIdRange, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // QA-D009-013: SparseNet::create_agent next_id overflow guard
    // -----------------------------------------------------------------------

    /// QA-D009-013: create_agent panics when next_id is u32::MAX (no free-list slot).
    /// The overflow is caught by a checked_add assertion rather than silent wrap.
    #[test]
    #[should_panic(expected = "AgentId space exhausted")]
    fn qa_d009_013_sparse_create_agent_panics_at_id_overflow() {
        let mut sn = SparseNet::new();
        sn.next_id = u32::MAX;
        // Free-list is empty; must go down the fresh-allocation path.
        // checked_add on u32::MAX should panic with "AgentId space exhausted".
        sn.create_agent(Symbol::Era);
    }

    // -----------------------------------------------------------------------
    // QA-D009-014: SparseNet::write_port unguarded ERA aux port
    // -----------------------------------------------------------------------

    /// QA-D009-014: write_port with a port for an agent that doesn't exist yet
    /// must trigger a debug_assert in debug builds (misordered call detection).
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "write_port: agent")]
    fn qa_d009_014_sparse_write_port_panics_for_unknown_agent_in_debug() {
        let mut sn = SparseNet::new();
        // Call connect directly for an agent that was never created (id = 99).
        sn.connect(PortRef::AgentPort(99, 0), PortRef::AgentPort(0, 0));
    }
}
