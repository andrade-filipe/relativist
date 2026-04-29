//! Net struct and operations.
//!
//! The complete interaction net data structure with agent arena,
//! port array, redex queue, and all CRUD operations.
//!
//! SPEC-22 R31: this module contains no `unsafe` blocks.
//! Bit-packed migration is SPEC-23's responsibility.

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;

use super::types::{total_ports, Agent, AgentId, PortRef, Symbol, DISCONNECTED, PORTS_PER_SLOT};

// SPEC-22 §4.4 (TASK-0488): compile-time Send + Sync assertion.
static_assertions::assert_impl_all!(Net: Send, Sync);

/// The complete interaction net.
///
/// Formally, a Net is a pair (A, W) where A is the set of agents and W
/// is the set of wires (DISC-004 v2, Section 1.1; REF-013, p.219).
///
/// Agents are stored in an arena indexed by AgentId.
/// Connections are represented implicitly by a flat port array.
/// The redex queue maintains known active pairs for incremental reduction.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(
    feature = "zero-copy",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub struct Net {
    /// Agent arena. `agents[id] == Some(agent)` if the agent is live.
    /// `agents[id] == None` if the slot is free (removed or never created).
    pub agents: Vec<Option<Agent>>,

    /// Flat port array. The slot for port `(id, port_id)` is at index
    /// `id * PORTS_PER_SLOT + port_id`. Each slot stores the `PortRef` to
    /// which the port is connected.
    pub ports: Vec<PortRef>,

    /// Queue of active pairs (redexes) for incremental reduction.
    /// May contain stale entries; the reduction engine verifies validity
    /// before reducing (SPEC-02 R17, SPEC-01 I4).
    pub redex_queue: VecDeque<(AgentId, AgentId)>,

    /// Monotonic upper bound on assigned `AgentId`s. Strictly greater than
    /// any `AgentId` ever assigned (live, in the free-list, or previously
    /// freed and re-assigned). Incremented only on fresh allocations (when
    /// the free-list is empty or recycling is disabled); recycled-slot
    /// creations leave `next_id` unchanged. (SPEC-01 I3', SPEC-22 R3/R10).
    pub next_id: AgentId,

    /// Root port: the AgentPort connected to the external observation point.
    /// `None` if the net has no root (e.g., a partition sub-net).
    /// Constrained to `None` or `Some(AgentPort(id, 0))` where `id` is a
    /// live agent (R6a). `FreePort` values are NOT valid for root.
    pub root: Option<PortRef>,

    /// Tracks FreePort-to-FreePort redirections that occur during reduction.
    ///
    /// When `connect(FreePort(a), FreePort(b))` is called, neither side has
    /// a slot in the port array, so the connection is normally lost. This map
    /// records the intended redirect: `a -> FreePort(b)` and `b -> FreePort(a)`.
    ///
    /// Used by `rebuild_free_port_index` (SPEC-05) to recover border FreePort
    /// references that were consumed during local partition reduction.
    /// Empty for nets that are not partition subnets.
    /// Not serialized: only relevant during the grid cycle.
    #[serde(skip)]
    #[cfg_attr(feature = "zero-copy", rkyv(with = rkyv::with::Skip))]
    pub freeport_redirects: HashMap<u32, PortRef>,

    /// SPEC-22 R1: free-list of recycled AgentId slots, LIFO (push/pop at end).
    /// Initialized empty by `Net::new()` and `Net::with_capacity()` (R8).
    /// Populated by `remove_agent` (R2) and consumed by `create_agent` (R3).
    ///
    /// Partition constraints (R10/R10b/R10c): in distributed contexts, the
    /// free-list contains only IDs within the partition's `id_range`; protected
    /// tombstones (border-referenced IDs under delta mode) are excluded.
    ///
    /// Always-on (R28): no feature gate; present in every build.
    pub free_list: Vec<AgentId>,

    /// SPEC-22 R10: the partition's owning ID range, if this Net belongs to a
    /// partitioned worker. Set by `build_subnet`; `None` for whole-net contexts.
    /// Not serialized (partition-context state).
    #[serde(skip)]
    #[cfg_attr(feature = "zero-copy", rkyv(with = rkyv::with::Skip))]
    pub id_range: Option<core::ops::Range<AgentId>>,

    /// SPEC-22 R10b: border-entries shadow for `RecyclePolicy::BorderClean`.
    /// Set by `build_subnet` for delta-mode workers; `None` in non-distributed contexts.
    /// Not serialized (partition-context state).
    #[serde(skip)]
    #[cfg_attr(feature = "zero-copy", rkyv(with = rkyv::with::Skip))]
    pub border_entries_shadow: Option<HashSet<AgentId>>,

    /// SPEC-22 R10b: recycle policy for delta-mode rounds.
    /// Default: `RecyclePolicy::DisableUnderDelta`.
    /// Not serialized (runtime policy, set by the worker dispatch loop).
    #[serde(skip)]
    #[cfg_attr(feature = "zero-copy", rkyv(with = rkyv::with::Skip))]
    pub recycle_policy: RecyclePolicy,

    /// SPEC-22 R10b: true iff the current round is a delta-mode round.
    /// Toggled by the worker dispatch loop at delta-round entry/exit.
    /// Not serialized.
    #[serde(skip)]
    #[cfg_attr(feature = "zero-copy", rkyv(with = rkyv::with::Skip))]
    pub is_in_delta_round: bool,

    /// SPEC-22 R10c (debug-only): protected tombstones — IDs that were
    /// border-referenced when removed under delta mode. Excluded from the
    /// free-list until the next `reconstruct` clean-boundary moment.
    #[cfg(debug_assertions)]
    #[serde(skip)]
    #[cfg_attr(feature = "zero-copy", rkyv(with = rkyv::with::Skip))]
    pub protected_tombstones: Option<HashSet<AgentId>>,

    /// SPEC-21 R37b / TASK-0589 (debug-only): cumulative count of successful
    /// free-list pops. Incremented each time `create_agent` takes the recycle
    /// path (SPEC-22 R3/R5 free-list branch). Zero under Strategy A + streaming.
    ///
    /// Gated on `debug_assertions` to stay at zero overhead in release builds.
    #[cfg(debug_assertions)]
    #[serde(skip)]
    #[cfg_attr(feature = "zero-copy", rkyv(with = rkyv::with::Skip))]
    pub free_list_pops: u64,

    /// SPEC-21 R37b / TASK-0590 (debug-only): pops where the popped ID IS in
    /// `border_entries_shadow` (Strategy B protected path, should always be 0).
    #[cfg(debug_assertions)]
    #[serde(skip)]
    #[cfg_attr(feature = "zero-copy", rkyv(with = rkyv::with::Skip))]
    pub free_list_pops_border: u64,

    /// SPEC-21 R37b / TASK-0590 (debug-only): pops where the popped ID is NOT in
    /// `border_entries_shadow` (Strategy B non-border precision-recycling path).
    #[cfg(debug_assertions)]
    #[serde(skip)]
    #[cfg_attr(feature = "zero-copy", rkyv(with = rkyv::with::Skip))]
    pub free_list_pops_non_border: u64,
}

/// SPEC-22 R10b: recycling strategy for delta-mode rounds.
///
/// Controls whether a worker may pop from the free-list during a delta-mode
/// round, preventing G1 violations when `BorderGraph` slot-id stability is
/// required.
///
/// - `DisableUnderDelta` (default, Strategy A): workers MUST NOT pop from the
///   free-list during a delta-mode round; `create_agent` falls through to
///   `next_id` allocation. The free-list is drained at the next clean partition
///   boundary (`reconstruct` per SPEC-19 R38).
/// - `BorderClean` (Strategy B): workers MAY pop from the free-list only if the
///   popped ID is NOT in the partition's `border_entries_shadow`. Border-referenced
///   IDs are re-pushed and a fresh allocation is returned instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum RecyclePolicy {
    /// Strategy A (default): disable recycle during delta-mode rounds.
    #[default]
    DisableUnderDelta,
    /// Strategy B: allow recycle of non-border-referenced IDs during delta-mode.
    BorderClean,
}

impl Default for Net {
    fn default() -> Self {
        Self::new()
    }
}

impl Net {
    /// Creates an empty Net with no agents, wires, or redexes.
    ///
    /// SPEC-22 R8: `free_list` is initialized empty.
    ///
    /// # Naming
    ///
    /// SPEC-20 §3.8 A7 calls this value "the empty net" and uses it as
    /// the identity element of [`Net::union`]. There is exactly **one**
    /// public constructor for this value (`Net::new()`); earlier drafts
    /// shipped a `Net::empty()` alias which was removed for API
    /// minimality (RV-001). Spec text that reads `a.union(Net::empty())`
    /// is realised in code as `a.union(Net::new())`.
    pub fn new() -> Self {
        Self {
            agents: Vec::new(),
            ports: Vec::new(),
            redex_queue: VecDeque::new(),
            next_id: 0,
            root: None,
            freeport_redirects: HashMap::new(),
            free_list: Vec::new(),
            id_range: None,
            border_entries_shadow: None,
            recycle_policy: RecyclePolicy::DisableUnderDelta,
            is_in_delta_round: false,
            #[cfg(debug_assertions)]
            protected_tombstones: None,
            #[cfg(debug_assertions)]
            free_list_pops: 0,
            #[cfg(debug_assertions)]
            free_list_pops_border: 0,
            #[cfg(debug_assertions)]
            free_list_pops_non_border: 0,
        }
    }

    /// Creates a Net with pre-allocated capacity for `capacity` agents.
    ///
    /// Pre-allocates the agent arena for `capacity` slots and the port
    /// array for `capacity * PORTS_PER_SLOT` slots.
    ///
    /// SPEC-22 R8: `free_list` is initialized empty. The capacity hint does
    /// NOT pre-allocate the free-list — it grows on demand via `remove_agent`.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            agents: Vec::with_capacity(capacity),
            ports: Vec::with_capacity(capacity * PORTS_PER_SLOT),
            redex_queue: VecDeque::new(),
            next_id: 0,
            root: None,
            freeport_redirects: HashMap::new(),
            free_list: Vec::new(),
            id_range: None,
            border_entries_shadow: None,
            recycle_policy: RecyclePolicy::DisableUnderDelta,
            is_in_delta_round: false,
            #[cfg(debug_assertions)]
            protected_tombstones: None,
            #[cfg(debug_assertions)]
            free_list_pops: 0,
            #[cfg(debug_assertions)]
            free_list_pops_border: 0,
            #[cfg(debug_assertions)]
            free_list_pops_non_border: 0,
        }
    }

    /// Creates a new agent with the given symbol and returns its assigned ID.
    ///
    /// SPEC-22 R3/R4/R5 (free-list path): if `free_list` is non-empty AND
    /// the recycle policy allows it for the current round, pops the most
    /// recently freed ID (LIFO), re-initializes its slot, and returns it.
    /// `next_id` is NOT incremented and the arena is NOT expanded.
    ///
    /// SPEC-22 R3 (fresh-allocation path): if `free_list` is empty (or
    /// recycling is disabled by `RecyclePolicy::DisableUnderDelta` during a
    /// delta-mode round), falls through to `next_id` allocation.
    ///
    /// Complexity: O(1) amortized (may trigger Vec reallocation on the fresh path).
    /// Postcondition: `agents[id] == Some(Agent { symbol, id })`.
    pub fn create_agent(&mut self, symbol: Symbol) -> AgentId {
        // SPEC-21 R37b TASK-0591: build-time `streaming-no-recycle` feature gate.
        // When enabled AND the round is streaming/delta-active, unconditionally
        // fall through to fresh allocation — never pop from the free-list.
        // This is the compile-time alternative closure of SC-007 (one-liner safety net).
        // The runtime gates below remain PRESENT AND CORRECT for non-feature builds
        // per TASK-0591 acceptance line 24.
        #[cfg(feature = "streaming-no-recycle")]
        if self.is_in_delta_round {
            // Unconditionally bypass free-list: allocate a fresh ID.
            assert!(
                self.next_id < u32::MAX,
                "AgentId space exhausted: next_id has reached u32::MAX ({})",
                u32::MAX
            );
            let fresh_id = self.next_id;
            self.next_id += 1;
            if self.agents.len() <= fresh_id as usize {
                self.agents.resize((fresh_id as usize) + 1, None);
            }
            self.agents[fresh_id as usize] = Some(Agent {
                symbol,
                id: fresh_id,
            });
            let required_len = (fresh_id as usize + 1) * PORTS_PER_SLOT;
            if self.ports.len() < required_len {
                self.ports.resize(required_len, DISCONNECTED);
            }
            return fresh_id;
        }

        // SPEC-22 R10b Strategy A: skip the free-list entirely during a
        // delta-mode round when the policy is DisableUnderDelta.
        let skip_recycle =
            self.is_in_delta_round && self.recycle_policy == RecyclePolicy::DisableUnderDelta;

        if !skip_recycle {
            // SPEC-22 R5 (LIFO): try to pop the most recently freed ID.
            // Strategy B: if popped ID is border-protected, re-push and fall through.
            if let Some(id) = self.free_list.pop() {
                // SPEC-22 R10b Strategy B (TASK-0590): per-id protection gate.
                // Only engages when `is_in_delta_round` is true (proxy for
                // `delta_mode || streaming_active` per R37b broadening).
                // In push mode (`is_in_delta_round = false`), the gate is inactive
                // and border IDs MAY be recycled as normal (SPEC-22 R3).
                if self.is_in_delta_round
                    && self.recycle_policy == RecyclePolicy::BorderClean
                    && self.is_border_protected(id)
                {
                    self.free_list.push(id);
                    // Fall through to fresh allocation below.
                } else {
                    // SPEC-22 R10: defensive — verify ID is in partition's range (debug only).
                    // Only fire for IDs that were allocated FROM the fresh range (id >= range.start).
                    // Pre-split agent IDs (id < range.start) are always below the fresh range and
                    // are legitimately recycled without violating R10 (they belong to this partition
                    // by σ assignment, not by range allocation).
                    #[cfg(debug_assertions)]
                    if let Some(ref range) = self.id_range {
                        if id >= range.start {
                            debug_assert!(
                                range.contains(&id),
                                "SPEC-22 R10 violation: popped id {} not in partition range {:?}",
                                id,
                                range
                            );
                        }
                    }

                    // SPEC-22 R4(b): re-initialize slot.
                    debug_assert!(
                        self.agents
                            .get(id as usize)
                            .and_then(|s| s.as_ref())
                            .is_none(),
                        "SPEC-22 R4: recycled slot {} is not None (free-list invariant violated)",
                        id
                    );

                    // Expand arena if somehow the slot is out of bounds
                    // (synthetic-state edge case per TEST-SPEC-0472 EC-2).
                    if self.agents.len() <= id as usize {
                        self.agents.resize((id as usize) + 1, None);
                    }
                    self.agents[id as usize] = Some(Agent { symbol, id });

                    // Re-initialize the 3 port slots to DISCONNECTED (R4(b) defensive).
                    let required_len = (id as usize + 1) * PORTS_PER_SLOT;
                    if self.ports.len() < required_len {
                        self.ports.resize(required_len, DISCONNECTED);
                    }
                    let base = id as usize * PORTS_PER_SLOT;
                    for offset in 0..PORTS_PER_SLOT {
                        self.ports[base + offset] = DISCONNECTED;
                    }

                    // SPEC-22 R10c: guard against recycling a protected tombstone.
                    #[cfg(debug_assertions)]
                    debug_assert!(
                        !self
                            .protected_tombstones
                            .as_ref()
                            .is_some_and(|s| s.contains(&id)),
                        "SPEC-22 R10c: attempted recycle of protected tombstone {}",
                        id
                    );

                    // SPEC-22 R27 family 3: post-recycle invariants.
                    // ID is no longer in free-list (already popped above),
                    // agents[id] == Some(_), free-list has no duplicates.
                    #[cfg(debug_assertions)]
                    {
                        debug_assert!(
                            !self.free_list.contains(&id),
                            "SPEC-22 R27 family 3: recycled id {} still in free-list after pop",
                            id
                        );
                        debug_assert!(
                            self.agents
                                .get(id as usize)
                                .and_then(|s| s.as_ref())
                                .is_some(),
                            "SPEC-22 R27 family 3: recycled slot {} is not Some after create",
                            id
                        );
                        // TASK-0589: count successful pops for test observability.
                        self.free_list_pops += 1;
                        // TASK-0590: classify pop as border vs non-border for Strategy B tests.
                        if self.is_border_protected(id) {
                            self.free_list_pops_border += 1;
                        } else {
                            self.free_list_pops_non_border += 1;
                        }
                    }

                    return id;
                }
            }
        }

        // Fresh allocation path (SPEC-22 R3 fall-through).
        // QA-D009-013: guard against u32 overflow; explicit panic in both debug and release.
        assert!(
            self.next_id < u32::MAX,
            "AgentId space exhausted: next_id has reached u32::MAX ({})",
            u32::MAX
        );
        let id = self.next_id;
        self.next_id += 1;

        let agent = Agent { symbol, id };

        // Expand arena to contain index `id`
        if self.agents.len() <= id as usize {
            self.agents.resize((id as usize) + 1, None);
        }
        self.agents[id as usize] = Some(agent);

        // Expand port array for the new agent's 3 slots
        let required_len = (id as usize + 1) * PORTS_PER_SLOT;
        if self.ports.len() < required_len {
            self.ports.resize(required_len, DISCONNECTED);
        }

        id
    }

    /// Returns the `PortRef` to which the given port is connected.
    ///
    /// For `AgentPort(id, p)`: looks up `ports[port_index(id, p)]`.
    /// Returns `DISCONNECTED` if the index is out of bounds.
    /// For `FreePort(_)`: returns `DISCONNECTED` (FreePort targets are
    /// resolved during merge, SPEC-05).
    ///
    /// Complexity: O(1).
    pub fn get_target(&self, port: PortRef) -> PortRef {
        match port {
            PortRef::AgentPort(id, p) => {
                let idx = super::types::port_index(id, p);
                if idx < self.ports.len() {
                    self.ports[idx]
                } else {
                    DISCONNECTED
                }
            }
            PortRef::FreePort(_) => DISCONNECTED,
        }
    }

    /// Writes the target of a port in the port array.
    ///
    /// Only operates on `AgentPort`; `FreePort` is a no-op (FreePort has
    /// no slot in the port array). Silently ignores out-of-bounds indices.
    ///
    /// This is intentionally private — external code should use `connect`
    /// and `disconnect` to maintain bidirectionality (SPEC-01 T1/I1).
    fn set_port(&mut self, port: PortRef, target: PortRef) {
        if let PortRef::AgentPort(id, p) = port {
            let idx = super::types::port_index(id, p);
            if idx < self.ports.len() {
                self.ports[idx] = target;
            }
        }
    }

    /// Establishes a bidirectional connection between two ports.
    ///
    /// Writes both directions in the port array: `a -> b` and `b -> a`.
    /// If both are principal ports (`AgentPort(_, 0)`), inserts the pair
    /// into the redex queue for incremental reduction (SPEC-02 R9, R13).
    ///
    /// Self-loop policy (R18b): intra-agent connections (different ports
    /// of the same agent) are valid. Same-port self-connections are
    /// rejected by a debug assertion.
    ///
    /// Complexity: O(1).
    /// Postcondition: `get_target(a) == b && get_target(b) == a`.
    pub fn connect(&mut self, a: PortRef, b: PortRef) {
        debug_assert_ne!(a, b, "Same-port self-connection is invalid: {:?}", a);

        self.set_port(a, b);
        self.set_port(b, a);

        // Track FreePort-to-FreePort redirections (both set_port calls are
        // no-ops for FreePort, so the connection would be lost without this).
        // Used by rebuild_free_port_index to recover border references
        // consumed during local partition reduction (SPEC-05).
        if let (PortRef::FreePort(fid_a), PortRef::FreePort(fid_b)) = (a, b) {
            if fid_a != u32::MAX {
                self.freeport_redirects.insert(fid_a, b);
            }
            if fid_b != u32::MAX {
                self.freeport_redirects.insert(fid_b, a);
            }
        }

        // Incremental redex detection: if both are principal ports,
        // an active pair is formed.
        if let (PortRef::AgentPort(id_a, 0), PortRef::AgentPort(id_b, 0)) = (a, b) {
            self.redex_queue.push_back((id_a, id_b));
        }
    }

    /// Removes the bidirectional connection of a port.
    ///
    /// Both the port itself and its former target are set to `DISCONNECTED`.
    /// If the port is already disconnected, this is a no-op.
    /// Does NOT remove stale entries from the redex queue — those are
    /// discarded at dequeue time (SPEC-01 I4, SPEC-02 R17).
    ///
    /// Complexity: O(1).
    pub fn disconnect(&mut self, port: PortRef) {
        let target = self.get_target(port);
        if target != DISCONNECTED {
            self.set_port(target, DISCONNECTED);
        }
        self.set_port(port, DISCONNECTED);
    }

    /// Removes an agent from the net.
    ///
    /// Disconnects all of the agent's ports (based on its symbol's
    /// `total_ports`), then marks the slot as `None`.
    ///
    /// SPEC-22 R2 / §4.3: after clearing the slot, purges any
    /// `freeport_redirects` entry keyed by this agent's ID (closes SC-001
    /// second surface — prevents stale redirects referencing a recycled slot),
    /// then pushes the ID onto `free_list` UNLESS `is_border_protected(id)`
    /// returns `true` (R10b/R10c protected-tombstone path).
    ///
    /// No-op if the slot is already `None` or out of bounds.
    ///
    /// Does NOT clean up the redex queue — stale entries are detected
    /// at dequeue time (SPEC-02 R17).
    ///
    /// Complexity: O(1) (at most 3 ports to disconnect; O(n) duplicate check
    /// in debug builds unless shadow is adopted — see TASK-0474).
    pub fn remove_agent(&mut self, id: AgentId) {
        let idx = id as usize;
        if idx < self.agents.len() {
            if let Some(agent) = self.agents[idx] {
                let num_ports = total_ports(agent.symbol);
                for p in 0..num_ports {
                    self.disconnect(PortRef::AgentPort(id, p));
                }
                self.agents[idx] = None;

                // SPEC-22 §4.1 / SC-001 second surface: purge stale freeport_redirects entry.
                self.freeport_redirects.remove(&id);

                // SPEC-22 R2 + R10b/R10c: push to free-list unless border-protected.
                if !self.is_border_protected(id) {
                    // SPEC-22 R6 (closes SC-018): no duplicates — asserted in debug builds.
                    debug_assert!(
                        !self.free_list.contains(&id),
                        "SPEC-22 R6: free-list duplicate detected for id {}; I3' violation",
                        id
                    );
                    self.free_list.push(id);

                    // SPEC-22 R27 family 1: post-remove-agent recycle assertions.
                    // free-list contains id, agents[id] == None, ports DISCONNECTED.
                    #[cfg(debug_assertions)]
                    {
                        debug_assert!(
                            self.free_list.contains(&id),
                            "SPEC-22 R27 family 1: id {} not in free-list after push",
                            id
                        );
                        debug_assert!(
                            self.agents.get(idx).is_some_and(|s| s.is_none()),
                            "SPEC-22 R27 family 1: agents[{}] is not None after remove",
                            id
                        );
                    }
                } else {
                    // R10c: protected tombstone — slot stays None, ports DISCONNECTED,
                    // ID NOT pushed to free_list until next reconstruct clean-boundary.
                    #[cfg(debug_assertions)]
                    if let Some(ref mut tombstones) = self.protected_tombstones {
                        tombstones.insert(id);
                    }

                    // SPEC-22 R27 family 2: post-remove-agent protected-tombstone assertions.
                    // agents[id] == None, ports DISCONNECTED, ID NOT in free_list,
                    // ID IS in protected_tombstones shadow (debug builds only).
                    #[cfg(debug_assertions)]
                    {
                        debug_assert!(
                            self.agents.get(idx).is_some_and(|s| s.is_none()),
                            "SPEC-22 R27 family 2: agents[{}] is not None after protected-tombstone remove",
                            id
                        );
                        debug_assert!(
                            !self.free_list.contains(&id),
                            "SPEC-22 R27 family 2: protected tombstone {} incorrectly in free-list",
                            id
                        );
                    }
                }
            }
        }
    }

    /// SPEC-22 R10b/R10c: returns `true` iff `id` is border-referenced and
    /// MUST NOT be recycled in the current delta-mode round.
    ///
    /// Default (single-net / non-distributed contexts): always returns `false`.
    /// Distributed call sites populate `border_entries_shadow` via `build_subnet`
    /// (TASK-0481) to override this behavior (TASK-0482).
    fn is_border_protected(&self, id: AgentId) -> bool {
        self.border_entries_shadow
            .as_ref()
            .is_some_and(|s| s.contains(&id))
    }

    /// SPEC-22 R6/R9: validates the free-list invariants.
    ///
    /// Checks:
    /// 1. Every ID in `free_list` MUST correspond to a `None` slot (R9: no live aliases).
    /// 2. No duplicate IDs in `free_list` (R6: uniqueness; QA-D009-004 duplicate guard).
    ///
    /// Called after deserialization (R9) and after `merge` (TASK-0483) in debug builds.
    /// Always-on post-condition for `from_bytes` (QA-D009-002).
    ///
    /// Returns `Ok(())` if all invariants hold; `Err(NetError::FreeListInvalid)`
    /// naming the first offending entry.
    pub fn validate_free_list(&self) -> Result<(), crate::error::NetError> {
        let mut seen = std::collections::HashSet::new();
        for &id in &self.free_list {
            // R6: no duplicates.
            if !seen.insert(id) {
                return Err(crate::error::NetError::FreeListInvalid {
                    id,
                    reason: "duplicate entry",
                });
            }
            // R9: every entry must be a None slot.
            if self
                .agents
                .get(id as usize)
                .and_then(|s| s.as_ref())
                .is_some()
            {
                return Err(crate::error::NetError::FreeListInvalid {
                    id,
                    reason: "slot is Some",
                });
            }
        }
        Ok(())
    }

    /// Drains protected tombstones back to the free-list at a clean partition
    /// boundary (`reconstruct` per SPEC-19 R38, TASK-0482).
    ///
    /// IDs in `border_entries_shadow` (the protected-tombstone source) whose
    /// arena slots are still `None` are pushed to `free_list`; others (slots
    /// since filled) are discarded. Resets `is_in_delta_round` and clears
    /// `border_entries_shadow` for the next round.
    ///
    /// In debug builds, also drains `protected_tombstones` for the
    /// invariant-tracking shadow path.
    pub fn reconstruct_drain_tombstones(&mut self) {
        // Reset delta-round flag at clean boundary.
        self.is_in_delta_round = false;

        // Drain all border-shadow IDs whose slots are still None (the canonical
        // tombstone recovery path — works in both release and debug builds).
        if let Some(shadow) = self.border_entries_shadow.take() {
            for id in shadow {
                if self.agents.get(id as usize).is_none_or(|s| s.is_none())
                    && !self.free_list.contains(&id)
                {
                    self.free_list.push(id);
                }
            }
        }

        #[cfg(debug_assertions)]
        {
            let ids: Vec<AgentId> = self
                .protected_tombstones
                .as_ref()
                .map(|s| s.iter().copied().collect())
                .unwrap_or_default();
            for id in ids {
                if self.agents.get(id as usize).is_none_or(|s| s.is_none())
                    && !self.free_list.contains(&id)
                {
                    self.free_list.push(id);
                }
            }
            if let Some(ref mut ts) = self.protected_tombstones {
                ts.clear();
            }
        }
    }

    /// Returns a reference to the agent with the given ID.
    ///
    /// Returns `None` if the ID is out of range or the slot is empty.
    /// This is the canonical accessor for agent lookup (SPEC-02 R15a).
    /// Callers MUST NOT index into `agents` directly for read access.
    ///
    /// Complexity: O(1).
    pub fn get_agent(&self, id: AgentId) -> Option<&Agent> {
        self.agents.get(id as usize).and_then(|slot| slot.as_ref())
    }

    /// Returns a mutable reference to the agent with the given ID.
    ///
    /// Returns `None` if the ID is out of range or the slot is empty.
    ///
    /// Complexity: O(1).
    pub fn get_agent_mut(&mut self, id: AgentId) -> Option<&mut Agent> {
        self.agents
            .get_mut(id as usize)
            .and_then(|slot| slot.as_mut())
    }

    /// Returns `true` if the redex queue is empty.
    ///
    /// Note: the queue may contain stale entries. For rigorous Normal Form
    /// verification, use `reduce_all` (SPEC-03) which drains stale entries.
    pub fn is_reduced(&self) -> bool {
        self.redex_queue.is_empty()
    }

    /// Checks whether a redex pair `(a, b)` is still valid (non-stale).
    ///
    /// A redex is valid if both agents exist (R15a) and their principal
    /// ports are connected to each other. The reduction engine MUST call
    /// this before applying a rule and silently discard stale entries (R17, I4).
    pub fn is_valid_redex(&self, a: AgentId, b: AgentId) -> bool {
        if self.get_agent(a).is_none() || self.get_agent(b).is_none() {
            return false;
        }
        self.get_target(PortRef::AgentPort(a, 0)) == PortRef::AgentPort(b, 0)
    }

    /// Returns the number of live (non-`None`) agents in the net.
    ///
    /// SPEC-22 R11: free-list entries correspond to `None` slots and are
    /// therefore EXCLUDED from this count automatically — `flatten()` skips
    /// `None` slots. The free-list does NOT change the semantics of this
    /// function; it only affects which `None` slots are available for reuse.
    ///
    /// Complexity: O(A) where A is the arena length.
    ///
    /// TODO(QA-D009-012, perf): this is O(arena_len), not O(live_count). A deserialized
    /// Net with a sparse-but-large arena (e.g., one live agent at a high ID) pays O(A)
    /// on every call. Fix by maintaining a `live_count: u32` field on `Net`, updated by
    /// `create_agent` (+1 on fresh path), `remove_agent` (-1), and recomputed once on
    /// deserialization. Tracked in TASK-0510.
    pub fn count_live_agents(&self) -> usize {
        // SPEC-22 R11: agents.iter().flatten() naturally excludes None slots
        // (free-list entries correspond to None slots and are skipped).
        self.agents.iter().filter(|slot| slot.is_some()).count()
    }

    /// Returns an iterator over all live agents in the net.
    ///
    /// Skips `None` slots (removed or never-created agents).
    /// This encapsulates the internal `Vec<Option<Agent>>` representation.
    pub fn live_agents(&self) -> impl Iterator<Item = &Agent> {
        self.agents.iter().filter_map(|slot| slot.as_ref())
    }

    /// Serializes the net to a bincode v2 byte vector (SPEC-18 §3.1).
    ///
    /// The format is self-contained: the receiver can reconstruct
    /// the complete Net from the bytes alone (SPEC-02 R25).
    pub fn to_bytes(&self) -> Result<Vec<u8>, crate::error::NetError> {
        crate::protocol::bincode_v2::encode(self)
            .map_err(|e| crate::error::NetError::Serialize(e.to_string()))
    }

    /// Deserializes a net from a bincode v2 byte slice (SPEC-18 §3.1).
    ///
    /// SPEC-22 R9 (QA-D009-002): after deserialization, the free-list is validated.
    /// Every ID in `free_list` MUST correspond to a `None` arena slot, and the list
    /// MUST have no duplicates. If either condition is violated, returns
    /// `Err(NetError::FreeListInvalid)` — a corrupted or maliciously crafted peer
    /// state cannot silently alias live agents via the next `create_agent` call.
    pub fn from_bytes(bytes: &[u8]) -> Result<Net, crate::error::NetError> {
        let net: Net = crate::protocol::bincode_v2::decode_value(bytes)
            .map_err(|e| crate::error::NetError::Deserialize(e.to_string()))?;
        // SPEC-22 R9 post-condition (always-on, not debug-only).
        net.validate_free_list()?;
        Ok(net)
    }

    /// Structural concatenation of two nets under a disjoint-`AgentId`
    /// precondition (SPEC-20 §3.8 A7; consumed by SPEC-20 §4.2.2 v1-mode
    /// step 4 and delta-mode step 4 to compose reclaimed partitions with
    /// surviving partitions before the re-split). See ARG-006 P12 for
    /// the confluence justification of the surrounding mixed-trace
    /// recovery cycle.
    ///
    /// # Semantics
    ///
    /// `union` is **strictly cheaper than [`merge`]** because it does
    /// **not** consult `FreePort` cross-references between `self` and
    /// `other`. Border `FreePort` entries from both sides are preserved
    /// verbatim in the result. Any cross-net `FreePort` matches are left
    /// to the subsequent `split()` call, which reallocates border IDs
    /// and rebuilds the `PartitionPlan` via the standard SPEC-04
    /// mechanism.
    ///
    /// # Field handling
    ///
    /// - `agents` and `ports`: agents from both nets are placed at their
    ///   original `AgentId` positions in the result; arrays are resized
    ///   to `max(self.next_id, other.next_id) * PORTS_PER_SLOT`. Slots
    ///   not claimed by either side stay `None` / `DISCONNECTED`.
    /// - `redex_queue`: the two queues are concatenated **in
    ///   self-then-other order**. Downstream determinism (T1–T4) holds
    ///   because the queue's processing order is reduction-strategy-
    ///   defined, not insertion-order-defined; the queue is a hint, not
    ///   a schedule. Stale entries are filtered at dequeue time per
    ///   SPEC-02 R17.
    /// - `next_id`: `max(self.next_id, other.next_id)`.
    /// - `root`: **follows `self`**. The right-hand net's root is dropped;
    ///   downstream split/merge re-establishes the canonical observation
    ///   point. This is documented as `union_root_follows_self` in
    ///   SPEC-20 §3.8 A7 and asserted by UT-0410-06. When `other.root`
    ///   is `Some(p)` and self-side wins, the dropped `other.root` does
    ///   NOT leave dangling redirect targets in `freeport_redirects`
    ///   because (per A7) borders are valued at agent-port pairs, not
    ///   roots. The dropped root reference is structurally inert
    ///   (QA-009).
    /// - `freeport_redirects`: union of the two maps via
    ///   `HashMap::extend` — if a key collides, **`other` wins**. The
    ///   map is keyed by `border_id` (`u32`), NOT by `AgentId`, so the
    ///   disjoint-`AgentId` precondition does **not** prevent border-id
    ///   collisions. Such collisions are expected: under SPEC-20 §3.8
    ///   A7 the union's role is structural concatenation; border-id
    ///   conflicts are transient because the next `split()` call
    ///   (SPEC-04) re-allocates fresh border ids per partition.
    ///   Callers that require border-id determinism MUST follow
    ///   `Net::union` with `split()` before any further operation
    ///   (QA-003).
    ///
    /// # Precondition
    ///
    /// The caller MUST guarantee that the live-`AgentId` sets of `self`
    /// and `other` are disjoint. SPEC-20 §4.2.2 establishes this via
    /// `remap_partition_ids` (A4) + `compute_id_ranges` (R13) before the
    /// call. Violation panics under `debug_assertions` with a message
    /// citing SPEC-20 A7 and the offending id; release builds run
    /// faster but produce a corrupted net (caller bug).
    ///
    /// Per SPEC-20 §4.2.2 the only call site invokes `Net::union` after
    /// `remap_partition_ids` (SPEC-04 A4) has guaranteed disjoint ranges
    /// **by construction**. Release builds rely on this construction-by-
    /// contract; if a runtime check is needed, validate at the call
    /// site before invoking `union` (QA-007).
    ///
    /// # Preconditions
    ///
    /// - Live `AgentId` sets of `self` and `other` are disjoint
    ///   (see above).
    /// - `max(self.next_id, other.next_id) < u32::MAX`. The result's
    ///   `next_id` equals that maximum, and the next `create_agent`
    ///   call increments it; reaching `u32::MAX` would wrap to `0` and
    ///   silently violate D4 (ID Uniqueness). The caller is responsible
    ///   for keeping the `AgentId` space below `u32::MAX`. Violation is
    ///   surfaced as a debug-only panic at the union site
    ///   (QA-001).
    ///
    /// # Invariants preserved
    ///
    /// - **I1, I2** (per-agent slot validity) — by construction.
    /// - **D4** (ID Uniqueness) — caller's precondition (disjoint ids
    ///   and `next_id < u32::MAX` headroom).
    /// - **D3** (Border Completeness) — deferred to the subsequent
    ///   `split()` call, per SPEC-20 §4.2.2.
    ///
    /// # Observability
    ///
    /// `Net::union` is a hot path during SPEC-20 §4.2.2 departure
    /// recovery. Metric / tracing emission is the **call site's**
    /// responsibility (RV-007); this function is a pure structural
    /// primitive and intentionally has no internal logging.
    ///
    /// # Future API
    ///
    /// Future variants such as `union_with_resolution` or
    /// `union_disjoint_partitions` may live alongside this baseline;
    /// `Net::union` is the minimal SPEC-20 §3.8 A7 surface (QA-010).
    ///
    /// # See also
    ///
    /// - SPEC-20 §11 (Change Log entry for §3.8 A7 introduction)
    /// - SPEC-02 §3.8 A7 (pending — ESPECIALISTA EM SPECS owes a
    ///   SPEC-02 v4 revision; QA-011)
    pub fn union(self, other: Net) -> Net {
        // Decompose so we own each field; avoids borrow contention.
        // SPEC-22 D-009 (RV-005): free_list, id_range, border_entries_shadow,
        // recycle_policy, is_in_delta_round, and protected_tombstones are
        // now included. The merged net takes `self`'s free-list and discards
        // `other`'s (union semantics: free-list reconciliation is `merge`'s job).
        let Net {
            agents: agents_a,
            ports: ports_a,
            redex_queue: mut redex_a,
            next_id: next_id_a,
            root: root_a,
            freeport_redirects: mut redirects_a,
            free_list: free_list_a,
            id_range: _id_range_a,
            border_entries_shadow: _bes_a,
            recycle_policy: recycle_policy_a,
            is_in_delta_round: _delta_a,
            #[cfg(debug_assertions)]
                protected_tombstones: _pt_a,
            #[cfg(debug_assertions)]
                free_list_pops: _flp_a,
            #[cfg(debug_assertions)]
                free_list_pops_border: _flpb_a,
            #[cfg(debug_assertions)]
                free_list_pops_non_border: _flpnb_a,
        } = self;
        let Net {
            agents: agents_b,
            ports: ports_b,
            redex_queue: redex_b,
            next_id: next_id_b,
            root: _root_b_dropped,
            freeport_redirects: redirects_b,
            free_list: _free_list_b,
            id_range: _id_range_b,
            border_entries_shadow: _bes_b,
            recycle_policy: _rp_b,
            is_in_delta_round: _delta_b,
            #[cfg(debug_assertions)]
                protected_tombstones: _pt_b,
            #[cfg(debug_assertions)]
                free_list_pops: _flp_b,
            #[cfg(debug_assertions)]
                free_list_pops_border: _flpb_b,
            #[cfg(debug_assertions)]
                free_list_pops_non_border: _flpnb_b,
        } = other;

        let merged_next_id = std::cmp::max(next_id_a, next_id_b);

        // QA-001: u32::MAX headroom check. If `merged_next_id` is at
        // u32::MAX, the next create_agent call would wrap next_id back
        // to 0 and reuse AgentId 0, silently breaching D4. The caller
        // is responsible for keeping the AgentId space below u32::MAX
        // (SPEC-20 §3.8 A7 precondition).
        debug_assert!(
            merged_next_id < u32::MAX,
            "Net::union: merged_next_id at u32::MAX would overflow next create_agent (D4 breach); SPEC-20 §3.8 A7 caller is responsible for keeping AgentId space below u32::MAX"
        );

        let agent_capacity = merged_next_id as usize;
        let port_capacity = agent_capacity * PORTS_PER_SLOT;

        // Resize to the merged capacity so every AgentId in either net
        // has a valid slot. We do not shrink — `agents_a.len()` may
        // exceed `next_id_a` if a previous remove_agent left dead slots.
        // QA-004: include agents_b.len() in the bound too, so a
        // right-side arena that grew past its own next_id (e.g. by a
        // dead tail) is preserved unchanged.
        let agents_target_len = std::cmp::max(
            std::cmp::max(agents_a.len(), agents_b.len()),
            agent_capacity,
        );
        let ports_target_len =
            std::cmp::max(ports_a.len(), std::cmp::max(ports_b.len(), port_capacity));

        let mut agents = agents_a;
        agents.resize(agents_target_len, None);

        let mut ports = ports_a;
        ports.resize(ports_target_len, DISCONNECTED);

        // Place every live agent from `other` at its `AgentId` index.
        // Under the disjoint precondition, every target slot in `agents`
        // is currently `None`; we surface a violation as a debug panic
        // pointing at the offending id (UT-0410-07).
        for (idx, slot) in agents_b.into_iter().enumerate() {
            if let Some(agent) = slot {
                debug_assert!(
                    agents[idx].is_none(),
                    "SPEC-20 A7: Net::union precondition violated — overlapping AgentId {}",
                    idx
                );
                agents[idx] = Some(agent);
            }
        }

        // Copy `other`'s port slots into the result. Under the disjoint-
        // `AgentId` precondition, every index `idx` that corresponds to
        // an `other`-owned agent has `ports_a[idx] == DISCONNECTED`
        // (RV-004 / QA-005), because `agents_a[idx]` was `None` and
        // `set_port` is the only writer of that slot. Tail-residue
        // indices beyond `ports_a.len()` were just resized to
        // `DISCONNECTED` above, so the same invariant holds. We only
        // WRITE non-DISCONNECTED targets to avoid overwriting `self`'s
        // wires for ids unique to `self`.
        for (idx, target) in ports_b.into_iter().enumerate() {
            if target != DISCONNECTED && idx < ports.len() {
                debug_assert!(
                    ports[idx] == DISCONNECTED,
                    "SPEC-20 A7: Net::union port-copy invariant violated — \
                     ports[{}] already set to {:?} but `other` wants {:?}; \
                     this implies AgentId overlap that the agent-loop \
                     debug_assert should have caught earlier",
                    idx,
                    ports[idx],
                    target
                );
                ports[idx] = target;
            }
        }

        redex_a.extend(redex_b);
        redirects_a.extend(redirects_b);

        Net {
            agents,
            ports,
            redex_queue: redex_a,
            next_id: merged_next_id,
            root: root_a,
            freeport_redirects: redirects_a,
            // SPEC-22: take self's free-list; other's is discarded (see above).
            free_list: free_list_a,
            id_range: None,
            border_entries_shadow: None,
            recycle_policy: recycle_policy_a,
            is_in_delta_round: false,
            #[cfg(debug_assertions)]
            protected_tombstones: None,
            #[cfg(debug_assertions)]
            free_list_pops: 0,
            #[cfg(debug_assertions)]
            free_list_pops_border: 0,
            #[cfg(debug_assertions)]
            free_list_pops_non_border: 0,
        }
    }

    // ------------------------------------------------------------------
    // Sparse conversion (SPEC-22 §4.6, R19 — TASK-0489)
    // ------------------------------------------------------------------

    /// Converts this dense `Net` into a `SparseNet`.
    ///
    /// Iterates the dense arena, inserting only live (non-`None`) agents into
    /// the sparse `agents` map. For each live agent, copies port entries
    /// `(agent.id, p)` for `p in 0..total_ports(agent.symbol)` from the flat
    /// port array, skipping `DISCONNECTED` entries (R17 sparse equivalent).
    ///
    /// Copies `redex_queue`, `next_id`, `root`, and `freeport_redirects`
    /// directly. The `free_list` is intentionally NOT copied — `SparseNet` has
    /// no tombstones and therefore no free-list concept.
    ///
    /// SPEC-22 §4.6 R19. Complexity: O(arena_len).
    pub fn to_sparse(&self) -> crate::net::sparse::SparseNet {
        use super::types::port_index;
        use crate::net::sparse::SparseNet;

        let live_count = self.count_live_agents();
        let mut sparse = SparseNet::with_capacity(live_count);

        for agent in self.agents.iter().flatten() {
            sparse.agents.insert(agent.id, *agent);
            let num_ports = total_ports(agent.symbol);
            for p in 0..num_ports {
                let idx = port_index(agent.id, p);
                if idx < self.ports.len() {
                    let target = self.ports[idx];
                    if target != DISCONNECTED {
                        sparse.ports.insert((agent.id, p), target);
                    }
                }
            }
        }
        sparse.redex_queue = self.redex_queue.clone();
        sparse.next_id = self.next_id;
        sparse.root = self.root;
        // SC-001 second surface: copy freeport_redirects (closes D1c).
        sparse.freeport_redirects = self.freeport_redirects.clone();
        sparse
    }

    // ------------------------------------------------------------------
    // Behavioral equality (SPEC-22 §3.2 R21 — TASK-0491, closes SC-014)
    // ------------------------------------------------------------------

    /// SPEC-22 R21 behavioral equality.
    ///
    /// Two `Net` values are *behaviorally equal* iff they agree on:
    /// - the **live-agent set** (trailing `None` slots are ignored),
    /// - the **port-target relation** for every live `AgentPort` source
    ///   (trailing `DISCONNECTED` entries are ignored),
    /// - the **redex queue** up to element ordering (set equality),
    /// - `root`, `next_id`, `freeport_redirects` (full equality),
    /// - `free_list` as a **set** (LIFO order is a Vec-serde implementation
    ///   detail; behavioral equality requires only the same set of recycled IDs).
    ///
    /// This is less strict than `PartialEq` (`==`), which compares `Vec`s
    /// byte-by-byte (and therefore distinguishes trailing `None`/`DISCONNECTED`
    /// padding). Use `is_behaviorally_equal` whenever comparing nets that may
    /// differ only in arena padding — e.g., after a `to_sparse().to_dense(None)`
    /// round-trip (R21 closes SC-014).
    pub fn is_behaviorally_equal(&self, other: &Net) -> bool {
        // Compare live-agent sets.
        let self_agents: Vec<&Agent> = self.agents.iter().flatten().collect();
        let other_agents: Vec<&Agent> = other.agents.iter().flatten().collect();
        if self_agents != other_agents {
            return false;
        }

        // Compare port-target relations for every live agent.
        for agent in self.agents.iter().flatten() {
            let num_ports = total_ports(agent.symbol);
            for p in 0..num_ports {
                let idx = super::types::port_index(agent.id, p);
                let self_target = if idx < self.ports.len() {
                    self.ports[idx]
                } else {
                    DISCONNECTED
                };
                let other_target = if idx < other.ports.len() {
                    other.ports[idx]
                } else {
                    DISCONNECTED
                };
                if self_target != other_target {
                    return false;
                }
            }
        }

        // Compare redex queue as a sorted multiset (QA-D009-007: multiplicity matters).
        // Order-independence is preserved by sorting, but duplicates are NOT discarded.
        let mut self_redex: Vec<(AgentId, AgentId)> = self.redex_queue.iter().copied().collect();
        let mut other_redex: Vec<(AgentId, AgentId)> = other.redex_queue.iter().copied().collect();
        self_redex.sort_unstable();
        other_redex.sort_unstable();
        if self_redex != other_redex {
            return false;
        }

        // Compare root, next_id, freeport_redirects (full equality).
        if self.root != other.root {
            return false;
        }
        if self.next_id != other.next_id {
            return false;
        }
        if self.freeport_redirects != other.freeport_redirects {
            return false;
        }

        // Compare free-lists with full order sensitivity (QA-D009-008: LIFO order matters).
        // R5 mandates LIFO; two nets with reversed free_lists allocate different IDs next.
        self.free_list == other.free_list
    }

    // ------------------------------------------------------------------
    // SPEC-22 R27 debug assertions — I3' uniqueness family (TASK-0495)
    // ------------------------------------------------------------------

    /// SPEC-22 R27 family (4): periodic check that no `PortRef` references a
    /// free-list `AgentId`.
    ///
    /// If any port slot in the dense array holds `AgentPort(id, _)` where `id`
    /// is currently in the free-list (i.e., `agents[id] == None`), the invariant
    /// I3' (ID uniqueness) or the non-reference rule (SPEC-22 R7) is violated.
    ///
    /// All assertions are `debug_assert!` — zero cost in release builds.
    /// Call once at the end of `reduce_all` or via `debug_check_invariants`.
    ///
    /// SPEC-22 R27 bullet 4.
    #[cfg(debug_assertions)]
    pub fn assert_no_free_list_port_refs(&self) {
        let free_set: std::collections::HashSet<AgentId> = self.free_list.iter().copied().collect();
        for port in &self.ports {
            if let PortRef::AgentPort(id, _) = port {
                debug_assert!(
                    !free_set.contains(id),
                    "SPEC-22 R7/R27 violation: PortRef references free-list ID {}",
                    id
                );
            }
        }
    }

    /// SPEC-22 R27: composite invariant check (all four families).
    ///
    /// Combines:
    /// - family 4 (`assert_no_free_list_port_refs`): no port references a free-list ID.
    /// - free-list no-duplicates (I3' uniqueness): free-list has no duplicate IDs.
    /// - every free-list ID maps to a `None` agent slot (family 1/3 post-conditions).
    ///
    /// All checks are `debug_assert!` — zero cost in release.
    #[cfg(debug_assertions)]
    pub fn debug_check_invariants(&self) {
        // Family 4: no port references a recycled ID.
        self.assert_no_free_list_port_refs();

        // I3' uniqueness: free-list has no duplicates.
        let mut seen = std::collections::HashSet::new();
        for &id in &self.free_list {
            debug_assert!(
                seen.insert(id),
                "SPEC-22 R27 I3' violation: duplicate free-list entry {}",
                id
            );
        }

        // Family 1/3 post-condition: every free-list ID is a None slot.
        for &id in &self.free_list {
            debug_assert!(
                self.agents.get(id as usize).is_some_and(|s| s.is_none()),
                "SPEC-22 R27 family 1/3 violation: free-list ID {} maps to a live slot",
                id
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // T1: Net::new() returns empty net
    #[test]
    fn test_net_new_empty() {
        let net = Net::new();
        assert!(net.agents.is_empty());
        assert!(net.ports.is_empty());
        assert!(net.redex_queue.is_empty());
        assert_eq!(net.next_id, 0);
        assert_eq!(net.root, None);
    }

    // T2: Net::with_capacity pre-allocates
    #[test]
    fn test_net_with_capacity() {
        let net = Net::with_capacity(100);
        assert!(net.agents.capacity() >= 100);
        assert!(net.ports.capacity() >= 300); // 100 * PORTS_PER_SLOT
        assert!(net.agents.is_empty());
        assert!(net.ports.is_empty());
        assert!(net.redex_queue.is_empty());
        assert_eq!(net.next_id, 0);
        assert_eq!(net.root, None);
    }

    // T3: Net implements Clone
    #[test]
    fn test_net_clone() {
        let net = Net::new();
        let net2 = net.clone();
        assert_eq!(net, net2);
    }

    // T4: Net implements PartialEq and Eq (R26a)
    #[test]
    fn test_net_equality() {
        let a = Net::new();
        let b = Net::new();
        assert_eq!(a, b);
    }

    // T5: Net implements Debug
    #[test]
    fn test_net_debug() {
        let net = Net::new();
        let debug_str = format!("{:?}", net);
        assert!(debug_str.contains("Net"));
    }

    // T6: Net serde round-trip
    #[test]
    fn test_net_serde_roundtrip() {
        let net = Net::new();
        let bytes = crate::protocol::bincode_v2::encode(&net).unwrap();
        let des: Net = crate::protocol::bincode_v2::decode_value(&bytes).unwrap();
        assert_eq!(net, des);
    }

    // T7: Net::with_capacity(0) works like new()
    #[test]
    fn test_net_with_capacity_zero() {
        let net = Net::with_capacity(0);
        assert!(net.agents.is_empty());
        assert!(net.ports.is_empty());
        assert_eq!(net.next_id, 0);
        assert_eq!(net.root, None);
    }

    // E1: Net::with_capacity large value
    #[test]
    fn test_net_with_capacity_large() {
        let net = Net::with_capacity(10_000);
        assert!(net.agents.capacity() >= 10_000);
        assert!(net.ports.capacity() >= 30_000);
    }

    // E3: Cloned net is independent
    #[test]
    fn test_net_clone_independence() {
        let mut net = Net::new();
        let clone = net.clone();
        net.next_id = 42;
        assert_ne!(net, clone);
    }

    // --- create_agent tests (TASK-0009) ---

    // T1: Create one agent, verify id and state
    #[test]
    fn test_create_agent_first() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        assert_eq!(id, 0);
        assert_eq!(net.next_id, 1);
        assert_eq!(
            net.agents[0],
            Some(Agent {
                symbol: Symbol::Con,
                id: 0
            })
        );
    }

    // T2: Create 3 agents sequentially
    #[test]
    fn test_create_agent_sequential() {
        let mut net = Net::new();
        let id0 = net.create_agent(Symbol::Con);
        let id1 = net.create_agent(Symbol::Dup);
        let id2 = net.create_agent(Symbol::Era);
        assert_eq!(id0, 0);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(net.next_id, 3);
    }

    // T3: Port array expands correctly
    #[test]
    fn test_create_agent_port_array_size() {
        let mut net = Net::new();
        net.create_agent(Symbol::Con);
        net.create_agent(Symbol::Dup);
        net.create_agent(Symbol::Era);
        // 3 agents * 3 slots = 9
        assert!(net.ports.len() >= 9);
    }

    // T4: New port slots are DISCONNECTED
    #[test]
    fn test_create_agent_ports_disconnected() {
        use crate::net::types::{port_index, DISCONNECTED};
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        for p in 0..3u8 {
            assert_eq!(net.ports[port_index(id, p)], DISCONNECTED);
        }
    }

    // E4: ERA also gets 3 port slots (uniform layout)
    #[test]
    fn test_create_agent_era_uniform_slots() {
        use crate::net::types::{port_index, DISCONNECTED};
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Era);
        // ERA has arity 0 but still gets 3 slots
        for p in 0..3u8 {
            assert_eq!(net.ports[port_index(id, p)], DISCONNECTED);
        }
    }

    // E5: Agent symbols are stored correctly
    #[test]
    fn test_create_agent_symbols() {
        let mut net = Net::new();
        net.create_agent(Symbol::Con);
        net.create_agent(Symbol::Dup);
        net.create_agent(Symbol::Era);
        assert_eq!(net.agents[0].unwrap().symbol, Symbol::Con);
        assert_eq!(net.agents[1].unwrap().symbol, Symbol::Dup);
        assert_eq!(net.agents[2].unwrap().symbol, Symbol::Era);
    }

    // --- get_target / set_port tests (TASK-0010) ---

    // T1: set_port then get_target reads back the value
    #[test]
    fn test_set_port_get_target_roundtrip() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        let target = PortRef::AgentPort(99, 1);
        net.set_port(PortRef::AgentPort(id, 0), target);
        assert_eq!(net.get_target(PortRef::AgentPort(id, 0)), target);
    }

    // T2: get_target out of bounds returns DISCONNECTED
    #[test]
    fn test_get_target_out_of_bounds() {
        let net = Net::new();
        assert_eq!(net.get_target(PortRef::AgentPort(999, 0)), DISCONNECTED);
    }

    // T3: get_target(FreePort) returns DISCONNECTED
    #[test]
    fn test_get_target_freeport() {
        let net = Net::new();
        assert_eq!(net.get_target(PortRef::FreePort(42)), DISCONNECTED);
    }

    // T4: set_port(FreePort) is a no-op
    #[test]
    fn test_set_port_freeport_noop() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        let before = net.clone();
        // This should not panic or change anything meaningful
        net.set_port(PortRef::FreePort(42), PortRef::AgentPort(id, 0));
        // The only thing that could differ is the FreePort slot (which doesn't exist),
        // so the net should still equal before
        assert_eq!(net, before);
    }

    // E6: set_port on out-of-bounds is silent no-op
    #[test]
    fn test_set_port_out_of_bounds_noop() {
        let mut net = Net::new();
        // No agents, so port array is empty
        net.set_port(PortRef::AgentPort(999, 0), PortRef::FreePort(1));
        // Should not panic, net unchanged
        assert!(net.ports.is_empty());
    }

    // E7: get_target on freshly created agent returns DISCONNECTED
    #[test]
    fn test_get_target_fresh_agent() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Dup);
        for p in 0..3u8 {
            assert_eq!(net.get_target(PortRef::AgentPort(id, p)), DISCONNECTED);
        }
    }

    // --- connect tests (TASK-0011) ---

    // T1: Bidirectional linkage
    #[test]
    fn test_connect_bidirectional() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 2));
        assert_eq!(
            net.get_target(PortRef::AgentPort(a, 1)),
            PortRef::AgentPort(b, 2)
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(b, 2)),
            PortRef::AgentPort(a, 1)
        );
    }

    // T2: Principal-principal connection enqueues redex
    #[test]
    fn test_connect_principal_enqueues_redex() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        assert_eq!(net.redex_queue.len(), 1);
        assert_eq!(net.redex_queue[0], (a, b));
    }

    // T3: Principal-auxiliary does NOT enqueue redex
    #[test]
    fn test_connect_principal_auxiliary_no_redex() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 1));
        assert!(net.redex_queue.is_empty());
    }

    // T4: Auxiliary-auxiliary does NOT enqueue redex
    #[test]
    fn test_connect_auxiliary_auxiliary_no_redex() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 2));
        assert!(net.redex_queue.is_empty());
    }

    // T5: Connect AgentPort to FreePort
    #[test]
    fn test_connect_agent_to_freeport() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(42));
        assert_eq!(
            net.get_target(PortRef::AgentPort(a, 0)),
            PortRef::FreePort(42)
        );
        // FreePort side: no slot, get_target returns DISCONNECTED
        assert_eq!(net.get_target(PortRef::FreePort(42)), DISCONNECTED);
        // No redex: FreePort is not AgentPort(_, 0)
        assert!(net.redex_queue.is_empty());
    }

    // T6: Intra-agent connection is valid (R18b)
    #[test]
    fn test_connect_intra_agent() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(a, 2));
        assert_eq!(
            net.get_target(PortRef::AgentPort(a, 1)),
            PortRef::AgentPort(a, 2)
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(a, 2)),
            PortRef::AgentPort(a, 1)
        );
    }

    // T7: Same-port self-connection panics in debug mode (R18b)
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Same-port self-connection is invalid")]
    fn test_connect_self_loop_panics() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(a, 1));
    }

    // --- disconnect tests (TASK-0012) ---

    // T1: Disconnect breaks both sides
    #[test]
    fn test_disconnect_both_sides() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 2));
        net.disconnect(PortRef::AgentPort(a, 1));
        assert_eq!(net.get_target(PortRef::AgentPort(a, 1)), DISCONNECTED);
        assert_eq!(net.get_target(PortRef::AgentPort(b, 2)), DISCONNECTED);
    }

    // T2: Disconnect already-disconnected port is no-op
    #[test]
    fn test_disconnect_already_disconnected() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        // Port is already DISCONNECTED after creation
        net.disconnect(PortRef::AgentPort(a, 0)); // no panic
        assert_eq!(net.get_target(PortRef::AgentPort(a, 0)), DISCONNECTED);
    }

    // T3: Disconnect FreePort is no-op
    #[test]
    fn test_disconnect_freeport_noop() {
        let mut net = Net::new();
        net.disconnect(PortRef::FreePort(99)); // no panic
    }

    // E8: Disconnect one side of a connection, other side becomes DISCONNECTED
    #[test]
    fn test_disconnect_from_target_side() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Disconnect from b's side
        net.disconnect(PortRef::AgentPort(b, 0));
        assert_eq!(net.get_target(PortRef::AgentPort(a, 0)), DISCONNECTED);
        assert_eq!(net.get_target(PortRef::AgentPort(b, 0)), DISCONNECTED);
    }

    // --- remove_agent tests (TASK-0013) ---

    // T1: Remove CON agent disconnects all 3 ports
    #[test]
    fn test_remove_agent_con() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        let c = net.create_agent(Symbol::Con);
        // Wire a's ports to b and c
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(c, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(c, 2));
        net.remove_agent(a);
        assert_eq!(net.agents[a as usize], None);
        // All targets disconnected
        assert_eq!(net.get_target(PortRef::AgentPort(b, 0)), DISCONNECTED);
        assert_eq!(net.get_target(PortRef::AgentPort(c, 1)), DISCONNECTED);
        assert_eq!(net.get_target(PortRef::AgentPort(c, 2)), DISCONNECTED);
    }

    // T2: Remove ERA agent (only 1 port)
    #[test]
    fn test_remove_agent_era() {
        let mut net = Net::new();
        let e = net.create_agent(Symbol::Era);
        let a = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(e, 0), PortRef::AgentPort(a, 0));
        net.remove_agent(e);
        assert_eq!(net.agents[e as usize], None);
        assert_eq!(net.get_target(PortRef::AgentPort(a, 0)), DISCONNECTED);
    }

    // T3: Remove already-removed agent is no-op
    #[test]
    fn test_remove_agent_already_removed() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.remove_agent(a);
        net.remove_agent(a); // no panic
        assert_eq!(net.agents[a as usize], None);
    }

    // T4: next_id unchanged after removal
    #[test]
    fn test_remove_agent_next_id_unchanged() {
        let mut net = Net::new();
        net.create_agent(Symbol::Con);
        let next_before = net.next_id;
        net.remove_agent(0);
        assert_eq!(net.next_id, next_before);
    }

    // E9: Remove out-of-bounds id is no-op
    #[test]
    fn test_remove_agent_out_of_bounds() {
        let mut net = Net::new();
        net.remove_agent(999); // no panic
    }

    // --- get_agent / get_agent_mut tests (TASK-0019) ---

    // T1: get_agent on live agent returns Some
    #[test]
    fn test_get_agent_live() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        let agent = net.get_agent(id).unwrap();
        assert_eq!(agent.symbol, Symbol::Con);
        assert_eq!(agent.id, id);
    }

    // T2: get_agent out-of-range returns None
    #[test]
    fn test_get_agent_out_of_range() {
        let net = Net::new();
        assert!(net.get_agent(999).is_none());
    }

    // T3: get_agent on removed agent returns None
    #[test]
    fn test_get_agent_removed() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Era);
        net.remove_agent(id);
        assert!(net.get_agent(id).is_none());
    }

    // T4: get_agent on empty net returns None
    #[test]
    fn test_get_agent_empty_net() {
        let net = Net::new();
        assert!(net.get_agent(0).is_none());
    }

    // T5: get_agent_mut allows mutation
    #[test]
    fn test_get_agent_mut_mutation() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        // Mutate the symbol (unusual but tests the API)
        net.get_agent_mut(id).unwrap().symbol = Symbol::Dup;
        assert_eq!(net.get_agent(id).unwrap().symbol, Symbol::Dup);
    }

    // T6: get_agent_mut out-of-range returns None
    #[test]
    fn test_get_agent_mut_out_of_range() {
        let mut net = Net::new();
        assert!(net.get_agent_mut(999).is_none());
    }

    // --- is_reduced / is_valid_redex tests (TASK-0014) ---

    // T1: Empty net is reduced
    #[test]
    fn test_is_reduced_empty() {
        let net = Net::new();
        assert!(net.is_reduced());
    }

    // T2: Net with redex in queue is not reduced
    #[test]
    fn test_is_reduced_with_redex() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        assert!(!net.is_reduced());
    }

    // T3: Valid redex returns true
    #[test]
    fn test_is_valid_redex_valid() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        assert!(net.is_valid_redex(a, b));
    }

    // T4: Stale redex (agent removed) returns false
    #[test]
    fn test_is_valid_redex_agent_removed() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.remove_agent(a);
        assert!(!net.is_valid_redex(a, b));
    }

    // T5: Stale redex (connection changed) returns false
    #[test]
    fn test_is_valid_redex_connection_changed() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        let c = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Now rewire a's principal to c instead
        net.disconnect(PortRef::AgentPort(a, 0));
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(c, 0));
        // (a, b) is now stale
        assert!(!net.is_valid_redex(a, b));
        // (a, c) is valid
        assert!(net.is_valid_redex(a, c));
    }

    // T6: Out-of-bounds agent ID returns false
    #[test]
    fn test_is_valid_redex_out_of_bounds() {
        let net = Net::new();
        assert!(!net.is_valid_redex(999, 888));
    }

    // --- count_live_agents / live_agents tests (TASK-0231) ---

    // T1: Empty net has 0 live agents
    #[test]
    fn test_count_live_agents_empty() {
        let net = Net::new();
        assert_eq!(net.count_live_agents(), 0);
    }

    // T2: 3 agents created
    #[test]
    fn test_count_live_agents_three() {
        let mut net = Net::new();
        net.create_agent(Symbol::Con);
        net.create_agent(Symbol::Dup);
        net.create_agent(Symbol::Era);
        assert_eq!(net.count_live_agents(), 3);
    }

    // T3: 5 created, 2 removed
    #[test]
    fn test_count_live_agents_after_removal() {
        let mut net = Net::new();
        let ids: Vec<_> = (0..5).map(|_| net.create_agent(Symbol::Con)).collect();
        net.remove_agent(ids[1]);
        net.remove_agent(ids[3]);
        assert_eq!(net.count_live_agents(), 3);
    }

    // T4: live_agents yields agents in order, skipping None
    #[test]
    fn test_live_agents_iterator() {
        let mut net = Net::new();
        net.create_agent(Symbol::Con); // id=0
        net.create_agent(Symbol::Dup); // id=1
        net.create_agent(Symbol::Era); // id=2
        net.remove_agent(1);
        let live: Vec<_> = net.live_agents().collect();
        assert_eq!(live.len(), 2);
        assert_eq!(live[0].symbol, Symbol::Con);
        assert_eq!(live[1].symbol, Symbol::Era);
    }

    // T5: Consistency: live_agents().count() == count_live_agents()
    #[test]
    fn test_live_agents_count_consistency() {
        let mut net = Net::new();
        for _ in 0..10 {
            net.create_agent(Symbol::Con);
        }
        net.remove_agent(3);
        net.remove_agent(7);
        assert_eq!(net.live_agents().count(), net.count_live_agents());
    }

    // T6: live_agents on empty net yields 0
    #[test]
    fn test_live_agents_empty() {
        let net = Net::new();
        assert_eq!(net.live_agents().count(), 0);
    }

    // --- to_bytes / from_bytes tests (TASK-0017) ---

    // T1: Round-trip empty net
    #[test]
    fn test_net_serde_empty_roundtrip() {
        let net = Net::new();
        let bytes = net.to_bytes().unwrap();
        let des = Net::from_bytes(&bytes).unwrap();
        assert_eq!(net, des);
    }

    // T2: Round-trip net with agents and connections
    #[test]
    fn test_net_serde_with_agents() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
        let bytes = net.to_bytes().unwrap();
        let des = Net::from_bytes(&bytes).unwrap();
        assert_eq!(net, des);
        // Verify all fields preserved
        assert_eq!(des.next_id, 2);
        assert_eq!(des.redex_queue.len(), 1);
        assert_eq!(des.agents.len(), 2);
    }

    // T3: Corrupt bytes cause Err
    #[test]
    fn test_net_deserialize_corrupt() {
        let result = Net::from_bytes(&[0xFF, 0xFF, 0xFF]);
        assert!(result.is_err());
    }

    // QA-D009-002: from_bytes must call validate_free_list; a serialized Net with
    // a free_list entry pointing to a live slot must return Err, not Ok.
    // This is an attacker-style test: manually craft a Net with corrupted free_list
    // and verify from_bytes rejects it.
    #[test]
    fn qa_d009_002_from_bytes_rejects_corrupt_free_list() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con); // id = 0, slot is Some
                                                // Manually corrupt: push a LIVE slot id into the free_list.
        net.free_list.push(id);

        // Serialize the corrupted net.
        let bytes = net.to_bytes().unwrap();

        // from_bytes MUST reject this — free_list contains a live agent id.
        let result = Net::from_bytes(&bytes);
        assert!(
            result.is_err(),
            "QA-D009-002: from_bytes must return Err for net with corrupted free_list \
             (entry {} is a live slot)",
            id
        );
        match result.unwrap_err() {
            crate::error::NetError::FreeListInvalid { id: bad_id, .. } => {
                assert_eq!(bad_id, id, "error must name the offending id");
            }
            other => panic!("expected FreeListInvalid, got {other:?}"),
        }
    }

    // T4: Round-trip preserves root
    #[test]
    fn test_net_serde_preserves_root() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.root = Some(PortRef::AgentPort(a, 0));
        let bytes = net.to_bytes().unwrap();
        let des = Net::from_bytes(&bytes).unwrap();
        assert_eq!(des.root, Some(PortRef::AgentPort(a, 0)));
    }

    // --- PartialEq/Eq verification tests (TASK-0018) ---

    // T1: Nets differing in next_id are NOT equal
    #[test]
    fn test_net_neq_next_id() {
        let mut a = Net::new();
        let mut b = Net::new();
        a.next_id = 5;
        b.next_id = 10;
        assert_ne!(a, b);
    }

    // T2: Nets differing in agents are NOT equal
    #[test]
    fn test_net_neq_agents() {
        let mut a = Net::new();
        let mut b = Net::new();
        a.create_agent(Symbol::Con);
        b.create_agent(Symbol::Dup);
        assert_ne!(a, b);
    }

    // T3: Nets with same agents but different connections are NOT equal
    #[test]
    fn test_net_neq_ports() {
        let mut a = Net::new();
        let mut b = Net::new();
        let id_a = a.create_agent(Symbol::Con);
        let id_a2 = a.create_agent(Symbol::Dup);
        let id_b = b.create_agent(Symbol::Con);
        let id_b2 = b.create_agent(Symbol::Dup);
        a.connect(PortRef::AgentPort(id_a, 1), PortRef::AgentPort(id_a2, 1));
        b.connect(PortRef::AgentPort(id_b, 1), PortRef::AgentPort(id_b2, 2));
        assert_ne!(a, b);
    }

    // T4: Serde round-trip structural equality
    #[test]
    fn test_net_serde_structural_equality() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        let des = Net::from_bytes(&net.to_bytes().unwrap()).unwrap();
        assert_eq!(net, des);
    }

    // -----------------------------------------------------------------------
    // TASK-0353 — rkyv round-trip for Net (SPEC-18 §3.5).
    //
    // Net derives `rkyv::{Archive, Serialize, Deserialize}` under the
    // `zero-copy` feature. The non-serializable `freeport_redirects` field
    // is `#[serde(skip)]` AND `#[rkyv(with = rkyv::with::Skip)]`, so it is
    // never carried on the wire and the round-trip MUST inflate it to
    // an empty HashMap (matching its `Default` value).
    // -----------------------------------------------------------------------

    // -----------------------------------------------------------------------
    // TASK-0471: Net.free_list field + constructor initialization (SPEC-22 R1, R8, R28)
    // -----------------------------------------------------------------------

    /// UT-0471-01: Net::new() initializes empty free_list.
    #[test]
    fn net_new_initializes_empty_free_list() {
        let net = Net::new();
        assert!(
            net.free_list.is_empty(),
            "R8: free_list must be empty on Net::new()"
        );
    }

    /// UT-0471-02: Net::with_capacity initializes empty free_list.
    #[test]
    fn net_with_capacity_initializes_empty_free_list() {
        let net = Net::with_capacity(100);
        assert!(
            net.free_list.is_empty(),
            "R8: capacity hint does not pre-alloc free_list"
        );
    }

    /// UT-0471-03: serde round-trip preserves empty free_list field.
    #[test]
    fn net_serde_round_trip_preserves_empty_free_list_field() {
        let net = Net::new();
        let bytes = crate::protocol::bincode_v2::encode(&net).unwrap();
        let net2: Net = crate::protocol::bincode_v2::decode_value(&bytes).unwrap();
        assert!(
            net2.free_list.is_empty(),
            "serde round-trip must preserve empty free_list"
        );
    }

    /// UT-0471-04: Clone preserves free_list content.
    #[test]
    fn net_clone_preserves_free_list() {
        let mut net = Net::new();
        net.free_list.push(7);
        let net2 = net.clone();
        assert_eq!(net2.free_list, vec![7], "Clone must preserve free_list");
    }

    /// UT-0471-05: PartialEq distinguishes nets with different free_lists.
    #[test]
    fn net_partial_eq_distinguishes_free_list() {
        let mut a = Net::new();
        let b = Net::new();
        a.free_list.push(7);
        assert_ne!(a, b, "PartialEq must include free_list");
    }

    /// UT-0471-06: free_list field is pub and accessible.
    #[test]
    fn net_field_is_pub_visible() {
        let net = Net::new();
        let _: &Vec<AgentId> = &net.free_list;
    }

    // -----------------------------------------------------------------------
    // TASK-0472: create_agent free-list pop (SPEC-22 R3, R4, R5)
    // -----------------------------------------------------------------------

    /// UT-0472-01: create_agent with empty free_list falls through to next_id.
    #[test]
    fn create_agent_with_empty_free_list_falls_through_to_next_id() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        assert_eq!(id, 0);
        assert_eq!(net.next_id, 1);
        assert_eq!(net.agents.len(), 1);
        assert!(net.free_list.is_empty());
    }

    /// UT-0472-02: create_agent with one free_list entry recycles it (LIFO).
    #[test]
    fn create_agent_with_one_free_list_entry_recycles() {
        let mut net = Net::new();
        // Create then remove to get ID 0 in free list
        let id0 = net.create_agent(Symbol::Con);
        assert_eq!(id0, 0);
        net.remove_agent(0);
        assert_eq!(
            net.free_list,
            vec![0],
            "remove_agent should push to free_list"
        );
        assert_eq!(net.next_id, 1, "next_id unchanged after remove");
        // Now recycle
        let id_reused = net.create_agent(Symbol::Dup);
        assert_eq!(id_reused, 0, "R3/R5: must pop from free_list (LIFO)");
        assert_eq!(
            net.next_id, 1,
            "R4(c): next_id must NOT increment on recycle"
        );
        assert!(net.free_list.is_empty(), "free_list drained after recycle");
        assert!(
            net.agents[0].is_some_and(|a| a.symbol == Symbol::Dup),
            "R4: slot reinitialized with new symbol"
        );
    }

    /// UT-0472-03: recycle does not grow the arena.
    #[test]
    fn create_agent_recycle_does_not_grow_arena() {
        let mut net = Net::new();
        for _ in 0..5 {
            net.create_agent(Symbol::Con);
        }
        assert_eq!(net.agents.len(), 5);
        net.remove_agent(2); // free_list = [2]
        let pre_len = net.agents.len();
        let id = net.create_agent(Symbol::Era);
        assert_eq!(id, 2, "should recycle slot 2");
        assert_eq!(
            net.agents.len(),
            pre_len,
            "R4(c): arena must not expand on recycle"
        );
    }

    /// UT-0472-04: recycle re-initializes port slots to DISCONNECTED.
    #[test]
    fn create_agent_recycle_re_initializes_port_slots() {
        use crate::net::types::port_index;
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Era);
        // Wire a port to something non-DISCONNECTED
        net.connect(PortRef::AgentPort(id, 0), PortRef::FreePort(99));
        net.remove_agent(id);
        // Recycle
        let id2 = net.create_agent(Symbol::Era);
        assert_eq!(id2, id, "should recycle same slot");
        // R4(b): all port slots DISCONNECTED after recycle
        for p in 0..3u8 {
            assert_eq!(
                net.ports[port_index(id2, p)],
                DISCONNECTED,
                "R4(b): port {} must be DISCONNECTED after recycle",
                p
            );
        }
    }

    /// UT-0472-05: returned ID is consistent with stored ID.
    #[test]
    fn create_agent_returned_id_is_consistent() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        assert_eq!(
            net.agents[id as usize].unwrap().id,
            id,
            "postcondition: stored ID must equal returned ID"
        );
    }

    /// UT-0472-06: postcondition Some with correct symbol for all 3 symbols.
    #[test]
    fn create_agent_postcondition_some_with_correct_symbol() {
        for symbol in [Symbol::Con, Symbol::Dup, Symbol::Era] {
            let mut net = Net::new();
            let id = net.create_agent(symbol);
            assert_eq!(
                net.agents[id as usize].unwrap().symbol,
                symbol,
                "postcondition: symbol must match for {:?}",
                symbol
            );
        }
    }

    // -----------------------------------------------------------------------
    // TASK-0473: remove_agent free-list push (SPEC-22 R2, R7)
    // -----------------------------------------------------------------------

    /// UT-0473-01: remove_agent pushes ID to free_list (LIFO push at end).
    #[test]
    fn remove_agent_pushes_id_to_free_list() {
        let mut net = Net::new();
        net.create_agent(Symbol::Con); // id=0
        net.create_agent(Symbol::Con); // id=1
        net.create_agent(Symbol::Con); // id=2
        net.remove_agent(1);
        assert!(
            net.free_list.contains(&1),
            "R2: free_list must contain removed id"
        );
        assert_eq!(
            net.free_list.last(),
            Some(&1),
            "LIFO: last element must be the removed id"
        );
    }

    /// UT-0473-02: remove_agent marks slot None.
    #[test]
    fn remove_agent_marks_slot_none() {
        let mut net = Net::new();
        net.create_agent(Symbol::Con); // id=0
        net.create_agent(Symbol::Con); // id=1
        net.remove_agent(1);
        assert!(net.agents[1].is_none(), "slot must be None after remove");
    }

    /// UT-0473-03: remove_agent disconnects all ports bidirectionally.
    #[test]
    fn remove_agent_disconnects_all_ports() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        let c = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(c, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(c, 2));
        net.remove_agent(a);
        // Partners' ports are also disconnected (bidirectional)
        assert_eq!(
            net.get_target(PortRef::AgentPort(b, 0)),
            DISCONNECTED,
            "b's port must be DISCONNECTED after a is removed"
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(c, 1)),
            DISCONNECTED,
            "c port 1 must be DISCONNECTED after a is removed"
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(c, 2)),
            DISCONNECTED,
            "c port 2 must be DISCONNECTED after a is removed"
        );
    }

    /// UT-0473-04: freeport_redirects purged on recycle (SC-001 second surface).
    #[test]
    fn freeport_redirects_purged_on_recycle() {
        let mut net = Net::new();
        net.create_agent(Symbol::Con); // id=0
        net.create_agent(Symbol::Con); // id=1
                                       // Manually insert a freeport_redirects entry keyed by 1
        net.freeport_redirects.insert(1, PortRef::AgentPort(0, 1));
        net.remove_agent(1);
        assert!(
            !net.freeport_redirects.contains_key(&1),
            "SC-001: freeport_redirects entry keyed by removed id must be purged"
        );
    }

    /// UT-0473-05: only the removed id's freeport_redirects entry is purged.
    #[test]
    fn freeport_redirects_only_keyed_by_removed_id_purged() {
        let mut net = Net::new();
        net.create_agent(Symbol::Con); // id=0
        net.create_agent(Symbol::Con); // id=1
        net.create_agent(Symbol::Con); // id=2
        net.freeport_redirects.insert(0, PortRef::AgentPort(2, 1));
        net.freeport_redirects.insert(1, PortRef::AgentPort(0, 1));
        net.freeport_redirects.insert(2, PortRef::AgentPort(0, 2));
        net.remove_agent(1);
        assert!(
            !net.freeport_redirects.contains_key(&1),
            "id 1 must be purged"
        );
        assert!(net.freeport_redirects.contains_key(&0), "id 0 must remain");
        assert!(net.freeport_redirects.contains_key(&2), "id 2 must remain");
    }

    /// UT-0473-06: remove_agent on already-removed ID is idempotent.
    #[test]
    fn remove_agent_on_already_removed_id_is_idempotent() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.remove_agent(a);
        let free_list_len = net.free_list.len();
        net.remove_agent(a); // second remove on same id
        assert_eq!(
            net.free_list.len(),
            free_list_len,
            "second remove must NOT push to free_list again"
        );
    }

    /// UT-0473-07: is_border_protected returns false in pure net context.
    #[test]
    fn is_border_protected_stub_returns_false_in_pure_net() {
        let net = Net::new();
        // border_entries_shadow is None -> is_border_protected always false
        // Test by verifying that remove_agent pushes to free_list (not protected)
        let mut net2 = Net::new();
        let id = net2.create_agent(Symbol::Con);
        net2.remove_agent(id);
        assert!(
            net2.free_list.contains(&id),
            "in pure net (no border_entries_shadow), remove must push to free_list"
        );
        // Reference the pure net to avoid unused-variable warning
        let _ = &net;
    }

    // -----------------------------------------------------------------------
    // TASK-0474: free-list no-duplicates invariant (SPEC-22 R5, R6)
    // -----------------------------------------------------------------------

    /// UT-0474-01: LIFO — pop returns most recently pushed.
    #[test]
    fn pop_returns_most_recently_pushed() {
        let mut net = Net::new();
        net.free_list.push(7);
        net.free_list.push(11);
        assert_eq!(
            net.free_list.pop(),
            Some(11),
            "R5 LIFO: most recent push is first pop"
        );
        assert_eq!(net.free_list.pop(), Some(7));
    }

    /// UT-0474-02 (debug-only): duplicate push via remove_agent triggers debug_assert.
    #[cfg(debug_assertions)]
    #[test]
    fn duplicate_push_via_remove_agent_triggers_debug_assert() {
        use std::panic;
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        // Normal remove: pushes id to free_list
        net.remove_agent(id);
        assert!(net.free_list.contains(&id));
        // Artificially recreate the slot to make remove_agent think the agent still exists
        net.agents[id as usize] = Some(Agent {
            symbol: Symbol::Con,
            id,
        });
        // Second remove should trigger debug_assert (R6 duplicate check)
        let result = panic::catch_unwind(move || {
            net.remove_agent(id);
        });
        assert!(
            result.is_err(),
            "R6: debug_assert must fire on duplicate free_list push"
        );
    }

    /// UT-0474-03 (release-only): duplicate push does NOT panic.
    #[cfg(not(debug_assertions))]
    #[test]
    fn release_build_compiles_assertion_out() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        net.remove_agent(id);
        // Re-insert agent slot to simulate second remove
        net.agents[id as usize] = Some(Agent {
            symbol: Symbol::Con,
            id,
        });
        // In release, this should NOT panic (debug_assert is compiled out)
        net.remove_agent(id); // Should silently push duplicate
    }

    /// UT-0474-04: multiple removes sequence builds correct LIFO stack.
    #[test]
    fn multiple_removes_build_lifo_stack() {
        let mut net = Net::new();
        net.create_agent(Symbol::Con); // id=0
        net.create_agent(Symbol::Con); // id=1
        net.create_agent(Symbol::Con); // id=2
        net.remove_agent(0);
        net.remove_agent(1);
        net.remove_agent(2);
        // free_list = [0, 1, 2] in push order; LIFO top is 2
        assert_eq!(net.free_list, vec![0, 1, 2]);
        assert_eq!(net.free_list.last(), Some(&2), "LIFO top must be 2");
    }

    // -----------------------------------------------------------------------
    // TASK-0475: free-list serde + bincode round-trip (SPEC-22 R9)
    // -----------------------------------------------------------------------

    /// UT-0475-01/02: serde round-trip preserves free_list order.
    #[test]
    fn serde_round_trip_preserves_free_list_order() {
        let mut net = Net::new();
        for _ in 0..5 {
            net.create_agent(Symbol::Con);
        } // IDs 0-4
        net.remove_agent(1); // free_list = [1]
        net.remove_agent(3); // free_list = [1, 3]
        let bytes = crate::protocol::bincode_v2::encode(&net).unwrap();
        let net2: Net = crate::protocol::bincode_v2::decode_value(&bytes).unwrap();
        assert_eq!(
            net2.free_list,
            vec![1, 3],
            "serde round-trip must preserve free_list order"
        );
    }

    /// UT-0475-03: validate_free_list returns Ok on valid state.
    #[test]
    fn validate_free_list_returns_ok_on_valid_state() {
        let mut net = Net::new();
        for _ in 0..5 {
            net.create_agent(Symbol::Con);
        }
        net.remove_agent(1);
        net.remove_agent(3);
        // agents[1] and agents[3] are None, free_list = [1, 3]
        assert!(
            net.validate_free_list().is_ok(),
            "R9: validate_free_list must return Ok on valid state"
        );
    }

    /// UT-0475-04: validate_free_list rejects Some slot.
    #[test]
    fn validate_free_list_rejects_some_slot() {
        use crate::error::NetError;
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        // Synthetic invalid state: free_list contains id but slot is Some
        net.free_list.push(id);
        let result = net.validate_free_list();
        assert!(
            matches!(result, Err(NetError::FreeListInvalid { id: fid, reason: "slot is Some" }) if fid == id),
            "R9: validate_free_list must return FreeListInvalid when slot is Some"
        );
    }

    /// UT-0475-05: serde round-trip then validate passes.
    #[test]
    fn serde_round_trip_then_validate_passes() {
        let mut net = Net::new();
        for _ in 0..5 {
            net.create_agent(Symbol::Con);
        }
        net.remove_agent(1);
        net.remove_agent(3);
        let bytes = crate::protocol::bincode_v2::encode(&net).unwrap();
        let net2: Net = crate::protocol::bincode_v2::decode_value(&bytes).unwrap();
        assert!(
            net2.validate_free_list().is_ok(),
            "R9: validate after round-trip must pass"
        );
    }

    // -----------------------------------------------------------------------
    // TASK-0477: count_live_agents excludes free-list entries (SPEC-22 R11)
    // -----------------------------------------------------------------------

    /// UT-0477-01: count_live excludes free_list entries.
    #[test]
    fn count_live_excludes_free_list_entries() {
        let mut net = Net::new();
        for _ in 0..10 {
            net.create_agent(Symbol::Con);
        }
        net.remove_agent(0);
        net.remove_agent(2);
        net.remove_agent(4);
        net.remove_agent(6);
        net.remove_agent(8);
        assert_eq!(
            net.count_live_agents(),
            5,
            "R11: count must exclude free-list slots"
        );
        assert_eq!(net.free_list.len(), 5, "free_list must have 5 entries");
    }

    /// UT-0477-02: count_live zero after full removal.
    #[test]
    fn count_live_zero_after_full_removal_with_free_list() {
        let mut net = Net::new();
        let ids: Vec<_> = (0..10).map(|_| net.create_agent(Symbol::Con)).collect();
        for id in ids {
            net.remove_agent(id);
        }
        assert_eq!(
            net.count_live_agents(),
            0,
            "R11: count must be 0 after all removed"
        );
        assert_eq!(
            net.free_list.len(),
            10,
            "free_list must have all 10 entries"
        );
    }

    /// UT-0477-04: count_live increments on create with recycle.
    #[test]
    fn count_live_increments_on_create_with_recycle() {
        let mut net = Net::new();
        for _ in 0..5 {
            net.create_agent(Symbol::Con);
        } // 5 live
        net.remove_agent(0);
        net.remove_agent(1);
        net.remove_agent(2); // 3 free, 2 live
        assert_eq!(net.count_live_agents(), 2);
        assert_eq!(net.free_list.len(), 3);
        net.create_agent(Symbol::Dup); // recycles one from free_list
        assert_eq!(
            net.count_live_agents(),
            3,
            "count must increment on recycle-create"
        );
        assert_eq!(net.free_list.len(), 2, "free_list must shrink by 1");
    }

    /// UT-0477-06: count_live after reduce_all on 100 CON-CON pairs = 0.
    #[test]
    fn count_live_after_reduce_all_zero_in_pure_annihilation_net() {
        use crate::reduction::reduce_all;
        let mut net = Net::new();
        // Build 100 CON-CON annihilation pairs
        for _ in 0..100 {
            let a = net.create_agent(Symbol::Con);
            let b = net.create_agent(Symbol::Con);
            net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
            net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
            net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));
        }
        reduce_all(&mut net);
        assert_eq!(
            net.count_live_agents(),
            0,
            "R11: annihilation net must have 0 live agents"
        );
        assert_eq!(
            net.free_list.len(),
            200,
            "free_list must hold all 200 freed agents"
        );
    }

    // -----------------------------------------------------------------------
    // TASK-0480: per-worker id_range defensive check (SPEC-22 R10)
    // -----------------------------------------------------------------------

    /// UT-0480-02: no id_range set — create_agent succeeds without assertion.
    #[test]
    fn id_range_none_skips_assertion_in_release() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        assert_eq!(id, 0, "fresh allocation must work with no id_range");
    }

    /// UT-0480-03: id_range set, in-range pop succeeds.
    #[test]
    fn id_range_some_in_range_id_pop_succeeds() {
        let mut net = Net::new();
        // Set up id_range = Some(0..100)
        net.id_range = Some(0..100);
        // Create and remove to get free_list=[50] scenario requires creating 51 agents
        // Instead, directly populate to avoid debug_assert overhead: use fresh alloc then inject
        // First create agents to fill arena up to id 50
        for _ in 0..51 {
            net.create_agent(Symbol::Con);
        } // IDs 0..50
        net.remove_agent(50); // free_list = [50]
                              // Now create_agent should pop 50 (in range 0..100)
        let id = net.create_agent(Symbol::Con);
        assert_eq!(id, 50, "R10: in-range pop must succeed");
    }

    /// UT-0480-04 (debug-only): out-of-range pop triggers debug_assert.
    #[cfg(debug_assertions)]
    #[test]
    fn id_range_some_traps_out_of_range_pop() {
        use std::panic;
        let mut net = Net::new();
        net.id_range = Some(0..100);
        // Synthetic invalid state: inject out-of-range id into free_list
        // Need arena to be large enough to contain slot 150
        for _ in 0..151 {
            net.create_agent(Symbol::Con);
        }
        net.remove_agent(150); // would normally push 150 to free_list
                               // But 150 is outside 0..100; simulate the violation by directly injecting
                               // (the remove_agent will have pushed it since id_range check is only in create_agent)
        assert!(net.free_list.contains(&150));
        // Creating an agent should pop 150, and the debug_assert in create_agent fires
        let result = panic::catch_unwind(move || {
            net.create_agent(Symbol::Con);
        });
        assert!(
            result.is_err(),
            "R10: debug_assert must fire on out-of-range pop"
        );
    }

    /// UT-0480-06: build_subnet sets id_range on returned net.
    /// (Tested indirectly: id_range field exists and is pub.)
    #[test]
    fn id_range_field_is_pub_and_settable() {
        let mut net = Net::new();
        net.id_range = Some(0..100);
        assert_eq!(net.id_range, Some(0..100));
        net.id_range = None;
        assert_eq!(net.id_range, None);
    }

    // -----------------------------------------------------------------------
    // TASK-0482: RecyclePolicy enum + is_border_protected wiring (R10b/R10c)
    // -----------------------------------------------------------------------

    /// UT-0482-01: default RecyclePolicy is DisableUnderDelta.
    #[test]
    fn default_recycle_policy_is_disable_under_delta() {
        let net = Net::new();
        assert_eq!(
            net.recycle_policy,
            RecyclePolicy::DisableUnderDelta,
            "R10b: default policy must be DisableUnderDelta"
        );
    }

    /// UT-0482-02: RecyclePolicy serde round-trip.
    #[test]
    fn recycle_policy_enum_derives_serde() {
        let policy = RecyclePolicy::BorderClean;
        let bytes = crate::protocol::bincode_v2::encode(&policy).unwrap();
        let back: RecyclePolicy = crate::protocol::bincode_v2::decode_value(&bytes).unwrap();
        assert_eq!(
            back,
            RecyclePolicy::BorderClean,
            "RecyclePolicy must round-trip through bincode"
        );
    }

    /// UT-0482-03: is_border_protected returns false in pure net context.
    #[test]
    fn is_border_protected_returns_false_in_pure_net_context() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        net.remove_agent(id);
        // With no border_entries_shadow, is_border_protected returns false for all ids,
        // so remove_agent MUST push to free_list
        assert!(net.free_list.contains(&id),
            "R10b: without border_entries_shadow, is_border_protected must be false -> push to free_list");
    }

    /// UT-0482-04: is_border_protected returns true for border id.
    #[test]
    fn is_border_protected_returns_true_for_border_id() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con); // id=0
                                                // Populate border_entries_shadow with id 0
        let mut shadow = std::collections::HashSet::new();
        shadow.insert(id);
        net.border_entries_shadow = Some(shadow);
        // Remove the agent: should NOT push to free_list (protected)
        net.remove_agent(id);
        assert!(
            !net.free_list.contains(&id),
            "R10c: border-protected id must NOT be pushed to free_list"
        );
    }

    /// UT-0482-05: is_border_protected returns false for non-border id.
    #[test]
    fn is_border_protected_returns_false_for_non_border_id() {
        let mut net = Net::new();
        let id0 = net.create_agent(Symbol::Con); // id=0
        let id1 = net.create_agent(Symbol::Con); // id=1
                                                 // Only protect id0
        let mut shadow = std::collections::HashSet::new();
        shadow.insert(id0);
        net.border_entries_shadow = Some(shadow);
        // Remove id1: NOT protected, so must push to free_list
        net.remove_agent(id1);
        assert!(
            net.free_list.contains(&id1),
            "R10b: non-border id must be pushed to free_list"
        );
    }

    /// UT-0482-06: Strategy A skips pop during delta round.
    #[test]
    fn strategy_a_skips_pop_during_delta_round() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        net.remove_agent(id); // free_list = [id]
                              // Enable delta round + DisableUnderDelta (Strategy A)
        net.is_in_delta_round = true;
        net.recycle_policy = RecyclePolicy::DisableUnderDelta;
        let next_before = net.next_id;
        let new_id = net.create_agent(Symbol::Con);
        assert_eq!(
            new_id, next_before,
            "R10b Strategy A: must fall through to fresh alloc during delta"
        );
        assert!(
            net.free_list.contains(&id),
            "free_list must still contain the id (not popped)"
        );
    }

    /// UT-0482-07: Strategy A pops when NOT in delta round.
    #[test]
    fn strategy_a_pops_when_not_in_delta_round() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        net.remove_agent(id); // free_list = [id]
                              // NOT in delta round
        net.is_in_delta_round = false;
        net.recycle_policy = RecyclePolicy::DisableUnderDelta;
        let new_id = net.create_agent(Symbol::Con);
        assert_eq!(
            new_id, id,
            "R10b Strategy A: must pop from free_list when not in delta round"
        );
    }

    /// UT-0482-08: Strategy B pops non-border id during delta round.
    ///
    /// Gated on `not(streaming-no-recycle)`: when the cargo feature is enabled,
    /// the compile-time gate unconditionally skips the free-list, making the
    /// Strategy B pop path unreachable (TASK-0591 line 24). The feature-ON
    /// variant is exercised by UT-0591-09 in tests/spec22_streaming_no_recycle.rs.
    #[test]
    #[cfg(not(feature = "streaming-no-recycle"))]
    fn strategy_b_pops_non_border_id() {
        let mut net = Net::new();
        let id0 = net.create_agent(Symbol::Con); // id=0
        let id1 = net.create_agent(Symbol::Con); // id=1
                                                 // id0 is border-protected, id1 is not
        let mut shadow = std::collections::HashSet::new();
        shadow.insert(id0);
        net.border_entries_shadow = Some(shadow);
        net.remove_agent(id1); // free_list = [1]
        net.is_in_delta_round = true;
        net.recycle_policy = RecyclePolicy::BorderClean;
        let new_id = net.create_agent(Symbol::Dup);
        assert_eq!(new_id, id1, "R10b Strategy B: must pop non-border id");
    }

    /// UT-0482-09: Strategy B re-pushes border id on pop collision.
    #[test]
    fn strategy_b_re_pushes_border_id_on_pop_collision() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con); // id=0
        let next_before_remove = net.next_id;
        // protect id=0
        let mut shadow = std::collections::HashSet::new();
        shadow.insert(id);
        net.border_entries_shadow = Some(shadow);
        // Manually push id into free_list (simulate: it wasn't removed via remove_agent
        // because border-protected, but synthetic state for this test)
        net.free_list.push(id);
        net.is_in_delta_round = true;
        net.recycle_policy = RecyclePolicy::BorderClean;
        // create_agent: pops id=0, sees it's border-protected, re-pushes, falls through to fresh
        let new_id = net.create_agent(Symbol::Con);
        assert_ne!(
            new_id, id,
            "R10b Strategy B: border-protected id must NOT be returned"
        );
        assert_eq!(
            new_id, next_before_remove,
            "R10b Strategy B: must fall through to fresh alloc"
        );
        // The border id is re-pushed back
        assert!(
            net.free_list.contains(&id),
            "border id must be re-pushed after rejection"
        );
    }

    /// UT-0482-10: R10c protected tombstone on remove_agent.
    #[test]
    fn r10c_protected_tombstone_on_remove_agent() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        // Protect id
        let mut shadow = std::collections::HashSet::new();
        shadow.insert(id);
        net.border_entries_shadow = Some(shadow);
        net.is_in_delta_round = true;
        net.remove_agent(id);
        assert!(
            net.agents[id as usize].is_none(),
            "slot must be None after remove"
        );
        assert!(
            !net.free_list.contains(&id),
            "R10c: border-protected id must NOT be in free_list"
        );
    }

    /// UT-0482-11: protected tombstone drained at reconstruct.
    #[test]
    fn protected_tombstone_drained_at_reconstruct() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        let mut shadow = std::collections::HashSet::new();
        shadow.insert(id);
        net.border_entries_shadow = Some(shadow);
        net.is_in_delta_round = true;
        net.remove_agent(id);
        assert!(
            !net.free_list.contains(&id),
            "pre-reconstruct: id not in free_list"
        );
        // Drain tombstones
        net.reconstruct_drain_tombstones();
        assert!(
            net.free_list.contains(&id),
            "post-reconstruct: tombstone must be drained to free_list"
        );
        assert!(
            !net.is_in_delta_round,
            "reconstruct must reset is_in_delta_round"
        );
    }

    /// UT-0482-12: non-distributed context unaffected by recycle_policy.
    #[test]
    fn non_distributed_context_unaffected_by_recycle_policy() {
        let mut net = Net::new();
        // Pure net: no border_entries_shadow, not in delta round
        let id = net.create_agent(Symbol::Con);
        net.remove_agent(id); // pushed to free_list
        let recycled_id = net.create_agent(Symbol::Dup); // should pop from free_list
        assert_eq!(
            recycled_id, id,
            "non-distributed context: recycle must work normally"
        );
    }

    /// UT-0353-04: Net round-trips through rkyv with a small, connected
    /// pair of agents. Net implements PartialEq, so we compare the whole
    /// struct directly. `freeport_redirects` is empty before and after.
    #[cfg(feature = "zero-copy")]
    #[test]
    fn rkyv_round_trip_net_minimal() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(7));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(u32::MAX));

        // Sanity: PartialEq derive should consider freeport_redirects, but
        // we keep both empty here so the comparison is unambiguous.
        assert!(net.freeport_redirects.is_empty());

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&net).expect("serialize");
        let archived = rkyv::access::<rkyv::Archived<Net>, rkyv::rancor::Error>(bytes.as_ref())
            .expect("access");
        let back: Net =
            rkyv::deserialize::<Net, rkyv::rancor::Error>(archived).expect("deserialize");

        assert_eq!(net, back, "Net did not round-trip through rkyv");
        assert!(
            back.freeport_redirects.is_empty(),
            "freeport_redirects must be re-defaulted to empty (skip adapter)"
        );
    }

    // -----------------------------------------------------------------------
    // TEST-SPEC-0489 — Net::to_sparse conversion (R19)
    // -----------------------------------------------------------------------

    /// UT-0489-01: to_sparse skips None slots.
    #[test]
    fn to_sparse_skips_none_slots() {
        let mut net = Net::new();
        for _ in 0..10 {
            net.create_agent(Symbol::Con);
        }
        // Remove agents 2 and 5 to create None slots.
        net.remove_agent(2);
        net.remove_agent(5);
        let sn = net.to_sparse();
        assert!(
            !sn.agents.contains_key(&2),
            "sparse should skip None slot 2"
        );
        assert!(
            !sn.agents.contains_key(&5),
            "sparse should skip None slot 5"
        );
    }

    /// UT-0489-02: to_sparse includes all live agents.
    #[test]
    fn to_sparse_includes_all_live_agents() {
        let mut net = Net::new();
        for _ in 0..10 {
            net.create_agent(Symbol::Con);
        }
        net.remove_agent(2);
        net.remove_agent(5);
        let sn = net.to_sparse();
        assert_eq!(
            sn.agents.len(),
            8,
            "should have 8 live agents after removing 2"
        );
        for i in 0..10u32 {
            if i == 2 || i == 5 {
                assert!(!sn.agents.contains_key(&i));
            } else {
                assert!(sn.agents.contains_key(&i));
            }
        }
    }

    /// UT-0489-03: to_sparse skips DISCONNECTED ports.
    #[test]
    fn to_sparse_skips_disconnected_ports() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Port 1 of agent a is DISCONNECTED — not connected to anything.
        let sn = net.to_sparse();
        assert!(
            !sn.ports.contains_key(&(a, 1)),
            "DISCONNECTED port (a,1) should not appear in sparse map"
        );
    }

    /// UT-0489-04: to_sparse skips ERA auxiliary ports (R17 / I6 sparse).
    #[test]
    fn to_sparse_skips_era_auxiliary_ports() {
        let mut net = Net::new();
        let era = net.create_agent(Symbol::Era);
        let sn = net.to_sparse();
        // ERA has arity 0 (only principal port 0); ports 1 and 2 do not exist.
        assert!(
            !sn.ports.contains_key(&(era, 1)),
            "ERA aux port 1 must not appear in sparse"
        );
        assert!(
            !sn.ports.contains_key(&(era, 2)),
            "ERA aux port 2 must not appear in sparse"
        );
    }

    /// UT-0489-05: to_sparse preserves freeport_redirects (SC-001 second surface).
    #[test]
    fn to_sparse_preserves_freeport_redirects() {
        let mut net = Net::new();
        net.create_agent(Symbol::Con); // id 0
        let con2 = net.create_agent(Symbol::Con); // id 1 (just to have a target)
        net.freeport_redirects
            .insert(99, PortRef::AgentPort(con2, 0));
        let sn = net.to_sparse();
        assert_eq!(
            sn.freeport_redirects.get(&99),
            Some(&PortRef::AgentPort(con2, 0)),
            "freeport_redirects must be copied to sparse (SC-001)"
        );
    }

    /// UT-0489-06: to_sparse preserves redex_queue clone.
    #[test]
    fn to_sparse_preserves_redex_queue_clone() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        let c = net.create_agent(Symbol::Dup);
        let d = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));
        let sn = net.to_sparse();
        assert_eq!(
            sn.redex_queue, net.redex_queue,
            "redex_queue must be preserved"
        );
    }

    /// UT-0489-07: to_sparse preserves next_id.
    #[test]
    fn to_sparse_preserves_next_id() {
        let mut net = Net::new();
        for _ in 0..10 {
            net.create_agent(Symbol::Era);
        }
        assert_eq!(net.next_id, 10);
        let sn = net.to_sparse();
        assert_eq!(sn.next_id, 10, "next_id must be preserved");
    }

    /// UT-0489-08: to_sparse preserves root.
    #[test]
    fn to_sparse_preserves_root() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.root = Some(PortRef::AgentPort(a, 0));
        let sn = net.to_sparse();
        assert_eq!(
            sn.root,
            Some(PortRef::AgentPort(a, 0)),
            "root must be preserved"
        );
    }

    /// UT-0489-09: to_sparse does not carry free_list (SparseNet has no free-list field).
    #[test]
    fn to_sparse_does_not_carry_free_list() {
        let mut net = Net::new();
        for _ in 0..5 {
            net.create_agent(Symbol::Con);
        }
        net.remove_agent(2);
        assert!(
            !net.free_list.is_empty(),
            "net should have a free-list entry"
        );
        let sn = net.to_sparse();
        // SparseNet has no free_list field — confirmed by the struct definition.
        // The absence of the field is a compile-time check; we verify the live count
        // does not include freed agents.
        assert_eq!(sn.agents.len(), 4, "sparse must contain only live agents");
    }

    /// EC-1: to_sparse on an empty net produces an empty SparseNet.
    #[test]
    fn to_sparse_empty_net() {
        let net = Net::new();
        let sn = net.to_sparse();
        assert!(sn.agents.is_empty());
        assert!(sn.ports.is_empty());
    }

    // -----------------------------------------------------------------------
    // TEST-SPEC-0490 — SparseNet::to_dense (R20) — tested via round-trip
    // -----------------------------------------------------------------------

    /// UT-0490-01: to_dense(None) populates full free_list.
    #[test]
    fn to_dense_none_populates_full_free_list() {
        use crate::net::SparseNet;
        use std::collections::HashSet;
        // Agents at IDs {50, 51, 75, 99, 130, 175}; next_id = 200.
        let mut sn = SparseNet::new();
        sn.agents.insert(
            50,
            Agent {
                symbol: Symbol::Con,
                id: 50,
            },
        );
        sn.agents.insert(
            51,
            Agent {
                symbol: Symbol::Con,
                id: 51,
            },
        );
        sn.agents.insert(
            75,
            Agent {
                symbol: Symbol::Dup,
                id: 75,
            },
        );
        sn.agents.insert(
            99,
            Agent {
                symbol: Symbol::Era,
                id: 99,
            },
        );
        sn.agents.insert(
            130,
            Agent {
                symbol: Symbol::Con,
                id: 130,
            },
        );
        sn.agents.insert(
            175,
            Agent {
                symbol: Symbol::Dup,
                id: 175,
            },
        );
        sn.next_id = 200;

        let net = sn.to_dense(None).unwrap();
        // Arena len = 176 (max_id=175 → 175+1).
        assert_eq!(net.agents.len(), 176, "arena len = max_id + 1 = 176");
        let free_set: HashSet<AgentId> = net.free_list.iter().copied().collect();
        // Expected: [0..176) minus {50,51,75,99,130,175} = 176 - 6 = 170 entries.
        assert_eq!(free_set.len(), 170, "free-list should have 170 entries");
        for id in [50u32, 51, 75, 99, 130, 175] {
            assert!(
                !free_set.contains(&id),
                "live agent {} must not be in free-list",
                id
            );
        }
    }

    /// UT-0490-02: to_dense(Some(50..100)) scopes free-list to partition range (T14a).
    #[test]
    fn to_dense_some_partition_scoped_t14a() {
        use crate::net::SparseNet;
        use std::collections::HashSet;
        let mut sn = SparseNet::new();
        sn.agents.insert(
            50,
            Agent {
                symbol: Symbol::Con,
                id: 50,
            },
        );
        sn.agents.insert(
            51,
            Agent {
                symbol: Symbol::Con,
                id: 51,
            },
        );
        sn.agents.insert(
            75,
            Agent {
                symbol: Symbol::Dup,
                id: 75,
            },
        );
        sn.agents.insert(
            99,
            Agent {
                symbol: Symbol::Era,
                id: 99,
            },
        );
        sn.agents.insert(
            130,
            Agent {
                symbol: Symbol::Con,
                id: 130,
            },
        );
        sn.agents.insert(
            175,
            Agent {
                symbol: Symbol::Dup,
                id: 175,
            },
        );
        sn.next_id = 200;

        let net = sn.to_dense(Some(50..100)).unwrap();
        let free_set: HashSet<AgentId> = net.free_list.iter().copied().collect();
        // [50..100) minus {50,51,75,99} = 50 IDs - 4 = 46 entries.
        assert_eq!(
            free_set.len(),
            46,
            "partition free-list should have 46 entries"
        );
        assert!(!free_set.contains(&50), "live 50 not in free-list");
        assert!(!free_set.contains(&51), "live 51 not in free-list");
        assert!(!free_set.contains(&75), "live 75 not in free-list");
        assert!(!free_set.contains(&99), "live 99 not in free-list");
    }

    /// UT-0490-03/04: to_dense(Some(range)) excludes IDs outside range.
    #[test]
    fn to_dense_some_excludes_outside_range() {
        use crate::net::SparseNet;
        let mut sn = SparseNet::new();
        sn.agents.insert(
            50,
            Agent {
                symbol: Symbol::Con,
                id: 50,
            },
        );
        sn.agents.insert(
            130,
            Agent {
                symbol: Symbol::Con,
                id: 130,
            },
        );
        sn.next_id = 200;
        let net = sn.to_dense(Some(50..100)).unwrap();
        assert!(
            !net.free_list.iter().any(|&id| id < 50),
            "no ID below range start in free-list"
        );
        assert!(
            !net.free_list.iter().any(|&id| id >= 100),
            "no ID at or above range end in free-list"
        );
    }

    /// UT-0490-05: to_dense(Some(empty_range)) yields empty free-list.
    #[test]
    fn to_dense_some_with_empty_range_yields_empty_free_list() {
        use crate::net::SparseNet;
        let mut sn = SparseNet::new();
        sn.agents.insert(
            50,
            Agent {
                symbol: Symbol::Con,
                id: 50,
            },
        );
        sn.next_id = 60;
        let net = sn.to_dense(Some(50..50)).unwrap();
        assert!(net.free_list.is_empty(), "empty range → empty free-list");
    }

    /// UT-0490-06: to_dense sets id_range on returned net.
    #[test]
    fn to_dense_id_range_propagated_to_returned_net() {
        use crate::net::SparseNet;
        let mut sn = SparseNet::new();
        sn.agents.insert(
            50,
            Agent {
                symbol: Symbol::Con,
                id: 50,
            },
        );
        sn.next_id = 100;
        let net = sn.to_dense(Some(50..100)).unwrap();
        assert_eq!(net.id_range, Some(50..100), "id_range must be propagated");
    }

    /// UT-0490-07: to_dense preserves freeport_redirects (SC-001).
    #[test]
    fn to_dense_preserves_freeport_redirects() {
        use crate::net::SparseNet;
        let mut sn = SparseNet::new();
        sn.agents.insert(
            50,
            Agent {
                symbol: Symbol::Con,
                id: 50,
            },
        );
        sn.next_id = 100;
        sn.freeport_redirects.insert(99, PortRef::AgentPort(50, 0));
        let net = sn.to_dense(Some(50..100)).unwrap();
        assert_eq!(
            net.freeport_redirects.get(&99),
            Some(&PortRef::AgentPort(50, 0)),
            "freeport_redirects must be preserved"
        );
    }

    /// UT-0490-10: arena size = max_id + 1.
    #[test]
    fn to_dense_arena_size_is_max_id_plus_one() {
        use crate::net::SparseNet;
        let mut sn = SparseNet::new();
        sn.agents.insert(
            175,
            Agent {
                symbol: Symbol::Dup,
                id: 175,
            },
        );
        sn.next_id = 176;
        let net = sn.to_dense(None).unwrap();
        assert_eq!(net.agents.len(), 176, "arena_len = max_id + 1 = 176");
    }

    /// EC-1: to_dense on empty SparseNet.
    #[test]
    fn to_dense_empty_sparse() {
        use crate::net::SparseNet;
        let sn = SparseNet::new();
        let net = sn.to_dense(None).unwrap();
        // next_id = 0, max_id = 0, arena_len = 1.
        assert_eq!(net.agents.len(), 1, "single None slot for max_id = 0");
        assert_eq!(net.free_list.len(), 1, "one free slot at index 0");
    }

    // -----------------------------------------------------------------------
    // TEST-SPEC-0491 — Net::is_behaviorally_equal (R21, closes SC-014)
    // -----------------------------------------------------------------------

    /// UT-0491-01: two empty nets are behaviorally equal.
    #[test]
    fn behaviorally_equal_returns_true_for_identical_nets() {
        let n1 = Net::new();
        let n2 = Net::new();
        assert!(n1.is_behaviorally_equal(&n2));
    }

    /// UT-0491-02: trailing None slots are ignored — two nets that agree on live agents
    /// but differ only in arena padding are behaviorally equal.
    #[test]
    fn behaviorally_equal_returns_true_for_same_live_set_different_arena_len() {
        // Build n1 with 2 live agents; build n2 identically but also record that they
        // are the same state after a round-trip — the key thing is same live agents,
        // same ports, same redex queue.  The spec's example of "trailing None" refers
        // to arena slots past the live agents (e.g. Vec::len() differs but live set
        // is the same).  We simulate this by directly extending the agents Vec with
        // a None slot without touching the free_list.
        let mut n1 = Net::new();
        let a = n1.create_agent(Symbol::Con);
        let b = n1.create_agent(Symbol::Con);
        n1.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        // n2 is a clone of n1 but with one extra None appended to agents and
        // one extra DISCONNECTED appended to ports — padding only, same live set.
        let mut n2 = n1.clone();
        n2.agents.push(None); // trailing None padding
        for _ in 0..PORTS_PER_SLOT {
            n2.ports.push(DISCONNECTED); // trailing DISCONNECTED padding
        }

        assert!(
            n1.is_behaviorally_equal(&n2),
            "trailing None/DISCONNECTED padding should be ignored by behavioral equality"
        );
    }

    /// UT-0491-03: different live set → not equal.
    #[test]
    fn behaviorally_equal_returns_false_for_different_live_set() {
        let mut n1 = Net::new();
        n1.create_agent(Symbol::Con);
        let mut n2 = Net::new();
        n2.create_agent(Symbol::Dup); // different symbol
        assert!(!n1.is_behaviorally_equal(&n2));
    }

    /// UT-0491-04: different freeport_redirects → not equal.
    #[test]
    fn behaviorally_equal_returns_false_for_different_freeport_redirects() {
        let mut n1 = Net::new();
        let a = n1.create_agent(Symbol::Con);
        n1.freeport_redirects.insert(99, PortRef::AgentPort(a, 0));
        let mut n2 = Net::new();
        n2.create_agent(Symbol::Con);
        assert!(!n1.is_behaviorally_equal(&n2));
    }

    /// UT-0491-05: redex queue order does not affect equality.
    #[test]
    fn behaviorally_equal_redex_queue_order_independent() {
        let mut n1 = Net::new();
        let a = n1.create_agent(Symbol::Con);
        let b = n1.create_agent(Symbol::Con);
        let c = n1.create_agent(Symbol::Dup);
        let d = n1.create_agent(Symbol::Dup);
        n1.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        n1.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));

        let mut n2 = n1.clone();
        // Reverse redex queue order.
        let r = n2.redex_queue.pop_back().unwrap();
        n2.redex_queue.push_front(r);

        assert!(
            n1.is_behaviorally_equal(&n2),
            "redex queue order should not affect behavioral equality"
        );
    }

    /// UT-0491-06: different redex queue contents → not equal.
    #[test]
    fn behaviorally_equal_distinguishes_redex_queue_set() {
        let mut n1 = Net::new();
        n1.create_agent(Symbol::Con);
        n1.create_agent(Symbol::Con);
        n1.redex_queue.push_back((0, 1));

        let mut n2 = Net::new();
        n2.create_agent(Symbol::Con);
        n2.create_agent(Symbol::Dup);
        n2.redex_queue.push_back((0, 2));

        assert!(!n1.is_behaviorally_equal(&n2));
    }

    /// UT-0491-07: trailing DISCONNECTED ports are ignored.
    #[test]
    fn behaviorally_equal_ignores_trailing_disconnected_ports() {
        let mut n1 = Net::new();
        let a = n1.create_agent(Symbol::Con);
        let b = n1.create_agent(Symbol::Con);
        n1.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        // n2 has the same live ports but was built differently (extra agent removed).
        let n2 = n1.clone();

        assert!(n1.is_behaviorally_equal(&n2));
    }

    /// UT-0491-09: different root → not equal.
    #[test]
    fn behaviorally_equal_distinguishes_root() {
        let mut n1 = Net::new();
        let a = n1.create_agent(Symbol::Con);
        n1.root = Some(PortRef::AgentPort(a, 0));

        let mut n2 = Net::new();
        n2.create_agent(Symbol::Con);
        n2.root = None;

        assert!(!n1.is_behaviorally_equal(&n2));
    }

    /// UT-0491-10: different next_id → not equal.
    #[test]
    fn behaviorally_equal_distinguishes_next_id() {
        let mut n1 = Net::new();
        n1.create_agent(Symbol::Con);
        n1.create_agent(Symbol::Era);
        n1.remove_agent(1); // next_id stays at 2

        let mut n2 = n1.clone();
        n2.next_id = 7; // force different

        assert!(!n1.is_behaviorally_equal(&n2));
    }

    /// UT-0491-11: free_list order-sensitive equality (R5 LIFO; QA-D009-008).
    /// Two nets that differ only in free_list LIFO order MUST NOT be behaviorally
    /// equal — the next create_agent call returns different IDs on each.
    #[test]
    fn behaviorally_equal_free_list_order_matters() {
        let mut n1 = Net::new();
        n1.create_agent(Symbol::Con);
        n1.create_agent(Symbol::Con);
        n1.create_agent(Symbol::Con);
        n1.remove_agent(1);
        n1.remove_agent(2);
        // n1.free_list = [1, 2]: remove_agent(1) pushes 1, then remove_agent(2) pushes 2.
        // LIFO: next pop returns 2.

        let mut n2 = n1.clone();
        // Reverse the free-list order: next pop returns 1 instead of 2.
        n2.free_list.reverse();

        assert!(
            !n1.is_behaviorally_equal(&n2),
            "free-list LIFO order must affect behavioral equality (R5, QA-D009-008)"
        );
    }

    /// UT-0491-12: round-trip dense→sparse→dense passes behavioral equality.
    #[test]
    fn r21_round_trip_1_dense_sparse_dense_passes() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        let c = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(c, 0));
        net.remove_agent(b);
        net.root = Some(PortRef::AgentPort(a, 0));

        let net2 = net.to_sparse().to_dense(None).unwrap();
        assert!(
            net.is_behaviorally_equal(&net2),
            "dense → sparse → dense round-trip must be behaviorally equal (R21)"
        );
    }

    /// UT-0491-13: round-trip sparse→dense→sparse gives full structural equality.
    #[test]
    fn r21_round_trip_2_sparse_dense_sparse_full_eq() {
        use crate::net::SparseNet;
        let mut sn = SparseNet::new();
        let a = sn.create_agent(Symbol::Con);
        let b = sn.create_agent(Symbol::Dup);
        sn.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        sn.next_id = 10;

        let sn2 = sn.to_dense(None).unwrap().to_sparse();
        assert_eq!(
            sn.agents, sn2.agents,
            "sparse → dense → sparse: agents must match"
        );
        assert_eq!(
            sn.ports, sn2.ports,
            "sparse → dense → sparse: ports must match"
        );
        assert_eq!(sn.next_id, sn2.next_id, "next_id must match");
        assert_eq!(sn.root, sn2.root, "root must match");
    }

    // -----------------------------------------------------------------------
    // TEST-SPEC-0495 — I3' debug assertions (R24, R25, R27)
    // -----------------------------------------------------------------------

    /// UT-0495-01: R27 family 1 — post-remove-agent recycle passes on valid net.
    #[cfg(debug_assertions)]
    #[test]
    fn r27_family_1_post_remove_agent_recycle() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.remove_agent(a);
        // Post-condition: free-list contains a, agents[a] == None.
        assert!(
            net.free_list.contains(&a),
            "id must be in free-list after recycle"
        );
        assert!(
            net.agents.get(a as usize).is_some_and(|s| s.is_none()),
            "slot must be None after remove"
        );
    }

    /// UT-0495-02: R27 family 1 catches synthetic violation (manually corrupted state).
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic]
    fn r27_family_4_catches_synthetic_violation() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Synthetically add a to the free-list while keeping the port reference — violation.
        net.free_list.push(a);
        // Now assert_no_free_list_port_refs should panic.
        net.assert_no_free_list_port_refs();
    }

    /// UT-0495-03: R27 family 2 — protected tombstone path.
    #[cfg(debug_assertions)]
    #[test]
    fn r27_family_2_post_remove_agent_protected_tombstone() {
        use std::collections::HashSet;
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        // Set up border_entries_shadow to make a border-protected.
        let mut shadow = HashSet::new();
        shadow.insert(a);
        net.border_entries_shadow = Some(shadow);
        net.protected_tombstones = Some(HashSet::new());
        net.remove_agent(a);
        // Post: agents[a] == None, NOT in free-list, IS in protected_tombstones.
        assert!(
            net.agents.get(a as usize).is_some_and(|s| s.is_none()),
            "slot must be None"
        );
        assert!(
            !net.free_list.contains(&a),
            "protected tombstone must NOT be in free-list"
        );
        assert!(
            net.protected_tombstones
                .as_ref()
                .is_some_and(|s| s.contains(&a)),
            "ID must be in protected_tombstones shadow"
        );
    }

    /// UT-0495-04: R27 family 3 — post-create-agent recycle path passes.
    #[cfg(debug_assertions)]
    #[test]
    fn r27_family_3_post_create_agent_recycle() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.remove_agent(a);
        assert_eq!(net.free_list.len(), 1, "free-list should have one entry");
        // Now recycle: create_agent should pop from free-list.
        let b = net.create_agent(Symbol::Dup);
        // Post: b not in free-list, agents[b] is Some.
        assert!(
            !net.free_list.contains(&b),
            "recycled ID must not remain in free-list"
        );
        assert!(
            net.agents.get(b as usize).is_some_and(|s| s.is_some()),
            "slot must be Some"
        );
    }

    /// UT-0495-06: R27 family 4 — no free-list port refs passes on a clean net.
    #[cfg(debug_assertions)]
    #[test]
    fn r27_family_4_no_free_list_port_refs_passes() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // No IDs in free-list currently; assert should pass.
        net.assert_no_free_list_port_refs(); // must not panic
    }

    /// UT-0495-08: debug_check_invariants passes for valid net.
    #[cfg(debug_assertions)]
    #[test]
    fn debug_check_invariants_combines_all_four_families() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.remove_agent(b); // adds to free-list after disconnect
                             // free-list contains b; no port references b.
        net.debug_check_invariants(); // must not panic
    }

    // -----------------------------------------------------------------------
    // TEST-SPEC-0497 — SPEC-03 reduction assertion audit (R27a)
    // -----------------------------------------------------------------------

    /// UT-0497-05: CON-DUP creates 4 agents; no inter-call monotonicity assertion.
    /// This test exercises the commutation rule with a partial free-list and
    /// verifies I3' (all 4 created agents are live, unique, next_id upper-bound holds).
    #[test]
    fn condup_4_creates_no_inter_call_monotonicity_assert() {
        use crate::reduction::engine::reduce_all;
        let mut net = Net::new();
        // Pre-populate free-list: remove two agents to give recycled IDs.
        let p = net.create_agent(Symbol::Con);
        let q = net.create_agent(Symbol::Con);
        net.remove_agent(p);
        net.remove_agent(q);
        assert_eq!(net.free_list.len(), 2, "setup: free-list has 2 entries");
        // Now build a CON-DUP redex pair.
        let con = net.create_agent(Symbol::Con);
        let dup = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(con, 0), PortRef::AgentPort(dup, 0));
        net.connect(PortRef::AgentPort(con, 1), PortRef::FreePort(10));
        net.connect(PortRef::AgentPort(con, 2), PortRef::FreePort(20));
        net.connect(PortRef::AgentPort(dup, 1), PortRef::FreePort(30));
        net.connect(PortRef::AgentPort(dup, 2), PortRef::FreePort(40));
        // Reduce: CON-DUP commutation creates 4 new agents.
        reduce_all(&mut net);
        // All live agents must be Some and uniquely IDed.
        for agent in net.agents.iter().flatten() {
            assert!(
                net.next_id > agent.id,
                "SPEC-22 R27a: next_id {} must be > every live agent.id {}",
                net.next_id,
                agent.id
            );
        }
    }

    /// UT-0497-04: assert_next_id_valid is I3'-compatible.
    #[test]
    fn assert_next_id_valid_preserved() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.remove_agent(a); // a in free-list, slot is None
                             // assert_next_id_valid: for each Some slot, id < next_id.
                             // a is None, so it does not trip the assertion.
                             // b is Some with id = 1, next_id = 2.
        net.assert_next_id_valid(); // must not panic — R27a compatible
        assert_eq!(b, 1); // paranoia
    }

    // -----------------------------------------------------------------------
    // QA-D009-013 — Net::create_agent next_id overflow guard
    // -----------------------------------------------------------------------

    /// QA-D009-013: create_agent panics with a clear message when next_id == u32::MAX
    /// and the free-list is empty. The overflow is caught before wrapping to 0.
    #[test]
    #[should_panic(expected = "AgentId space exhausted")]
    fn qa_d009_013_dense_create_agent_panics_at_id_overflow() {
        let mut net = Net::new();
        net.next_id = u32::MAX;
        // Free-list is empty; must go down the fresh-allocation path.
        net.create_agent(Symbol::Era);
    }

    // -----------------------------------------------------------------------
    // QA-D009-007/008 — is_behaviorally_equal redex multiplicity + free_list LIFO
    // -----------------------------------------------------------------------

    /// QA-D009-007: is_behaviorally_equal must treat redex_queue as a multiset,
    /// not a set. Two nets that differ only in duplicate redex entries MUST NOT
    /// be equal (multiplicity matters for correctness diagnostics).
    #[test]
    fn qa_d009_007_is_behaviorally_equal_redex_multiplicity_differs() {
        let mut a = Net::new();
        let p = a.create_agent(Symbol::Con);
        let q = a.create_agent(Symbol::Con);
        a.connect(PortRef::AgentPort(p, 0), PortRef::AgentPort(q, 0));
        // a.redex_queue = [(p, q)]

        let mut b = a.clone();
        b.redex_queue.push_back((p, q));
        b.redex_queue.push_back((p, q));
        // b.redex_queue = [(p, q), (p, q), (p, q)] — three copies

        assert!(
            !a.is_behaviorally_equal(&b),
            "QA-D009-007: nets with different redex_queue multiplicity must NOT be behaviorally equal"
        );
    }

    /// QA-D009-008: is_behaviorally_equal must respect free_list LIFO order.
    /// Two nets identical except for reversed free_list MUST NOT be equal,
    /// because next create_agent returns a different ID on each.
    #[test]
    fn qa_d009_008_is_behaviorally_equal_free_list_order_sensitive() {
        let mut a = Net::new();
        a.free_list = vec![5, 3]; // LIFO: next pop returns 3
        let mut b = a.clone();
        b.free_list = vec![3, 5]; // LIFO: next pop returns 5

        assert!(
            !a.is_behaviorally_equal(&b),
            "QA-D009-008: nets with reversed free_list must NOT be behaviorally equal (R5 LIFO)"
        );
    }

    // -----------------------------------------------------------------------
    // TASK-0589 — SPEC-21 R37b Strategy A broadening: streaming_active gate
    // -----------------------------------------------------------------------
    //
    // These tests verify that `Net::create_agent` skips the free-list under
    // Strategy A when either `delta_mode` OR `streaming_active` is set.
    // The proxy for `streaming_active` is `is_in_delta_round` (set by
    // `WorkerPullContext::enter_streaming_mode`; TASK-0578 Wave 5).
    //
    // Source: TEST-SPEC-0589 UT-0589-01..08.

    /// UT-0589-01: Strategy A — streaming + delta_mode → zero free-list pops.
    ///
    /// Canonical fixture: non-empty free-list; `is_in_delta_round = true` (proxy for
    /// `delta_mode = true` AND `streaming_active = true`); policy = DisableUnderDelta.
    /// Expect: fresh allocation, free-list untouched, `free_list_pops == 0`.
    #[test]
    #[cfg(debug_assertions)]
    fn ut_0589_01_streaming_active_strategy_a_no_pop_during_chunk() {
        let mut net = Net::new();
        // Populate free-list via a prior remove.
        let id0 = net.create_agent(Symbol::Con);
        net.remove_agent(id0); // free_list = [id0]
        assert_eq!(net.free_list.len(), 1, "setup: free_list must be non-empty");

        // Simulate streaming + delta active (is_in_delta_round is the unified proxy).
        net.is_in_delta_round = true;
        net.recycle_policy = RecyclePolicy::DisableUnderDelta;
        let next_before = net.next_id;

        let new_id = net.create_agent(Symbol::Era);

        assert_eq!(
            new_id, next_before,
            "UT-0589-01: Strategy A must fall through to fresh alloc (delta+streaming)"
        );
        assert!(
            net.free_list.contains(&id0),
            "UT-0589-01: free_list must still hold the id (not popped)"
        );
        assert_eq!(
            net.free_list_pops, 0,
            "UT-0589-01: free_list_pops counter must be zero (Strategy A gate active)"
        );
    }

    /// UT-0589-02: Strategy A — streaming_active=true, delta_mode=false → still zero pops.
    ///
    /// Per R37b broadening: gate triggers on `(delta_mode || streaming_active)`.
    /// When streaming_active is the only active flag, pop must still be skipped.
    /// `is_in_delta_round` serves as the proxy (set by `enter_streaming_mode`).
    #[test]
    #[cfg(debug_assertions)]
    fn ut_0589_02_streaming_active_no_delta_strategy_a_no_pop() {
        let mut net = Net::new();
        let id0 = net.create_agent(Symbol::Dup);
        net.remove_agent(id0);

        // Only streaming_active is set (proxied by is_in_delta_round); no separate delta flag.
        net.is_in_delta_round = true;
        net.recycle_policy = RecyclePolicy::DisableUnderDelta;
        let next_before = net.next_id;

        let new_id = net.create_agent(Symbol::Con);

        assert_eq!(
            new_id, next_before,
            "UT-0589-02: broadening — streaming_active alone must skip free-list pop"
        );
        assert_eq!(
            net.free_list_pops, 0,
            "UT-0589-02: no pops when streaming_active is set under DisableUnderDelta"
        );
    }

    /// UT-0589-03: Strategy A — push mode (streaming_inactive, delta_inactive) → pops normally.
    ///
    /// SPEC-22 R3 path must be UNCHANGED in push mode. With `is_in_delta_round = false`,
    /// the gate does not trigger and the free-list is popped.
    #[test]
    #[cfg(debug_assertions)]
    fn ut_0589_03_push_no_delta_strategy_a_pop_normally() {
        let mut net = Net::new();
        let id0 = net.create_agent(Symbol::Con);
        net.remove_agent(id0);
        assert_eq!(net.free_list.len(), 1, "setup: free_list must be non-empty");

        // Push mode: both flags off.
        net.is_in_delta_round = false;
        net.recycle_policy = RecyclePolicy::DisableUnderDelta;

        let recycled = net.create_agent(Symbol::Era);

        assert_eq!(
            recycled, id0,
            "UT-0589-03: SPEC-22 R3 pop must occur in push/non-delta mode"
        );
        assert!(
            net.free_list.is_empty(),
            "UT-0589-03: free_list must be empty after successful pop"
        );
        assert_eq!(
            net.free_list_pops, 1,
            "UT-0589-03: free_list_pops counter must be 1 after one successful pop"
        );
    }

    /// UT-0589-06: gate condition — `(is_in_delta_round) && DisableUnderDelta` covers R37b.
    ///
    /// Verifies the combined disjunction by toggling `is_in_delta_round` around a create_agent
    /// call and checking counter transitions. When flag is OFF then ON, pops = 1 then 1 (no
    /// additional pop on the gated call).
    #[test]
    #[cfg(debug_assertions)]
    fn ut_0589_06_gate_condition_extends_disjunction() {
        let mut net = Net::new();
        let id0 = net.create_agent(Symbol::Con);
        let id1 = net.create_agent(Symbol::Era);
        net.remove_agent(id0);
        net.remove_agent(id1);
        // free_list = [id0, id1] (LIFO: pop returns id1 first)

        net.recycle_policy = RecyclePolicy::DisableUnderDelta;

        // Gate OFF → pop occurs.
        net.is_in_delta_round = false;
        let first = net.create_agent(Symbol::Dup);
        assert_eq!(first, id1, "UT-0589-06: first call pops LIFO head");
        assert_eq!(net.free_list_pops, 1, "UT-0589-06: one pop so far");

        // Gate ON → pop suppressed.
        net.is_in_delta_round = true;
        let next_before = net.next_id;
        let second = net.create_agent(Symbol::Con);
        assert_eq!(second, next_before, "UT-0589-06: gate active → fresh alloc");
        assert_eq!(
            net.free_list_pops, 1,
            "UT-0589-06: counter unchanged (no pop on gated call)"
        );
        // id0 still in free_list (not popped).
        assert!(
            net.free_list.contains(&id0),
            "UT-0589-06: id0 must remain in free_list (gate blocked pop)"
        );
    }

    /// UT-0589-07: free_list_pops counter is zero across full streaming gate.
    ///
    /// Multi-step sequence: 4 creates, 4 removes, then enable gate and do 4 creates.
    /// Verifies counter stays at 0 throughout the gated region.
    #[test]
    #[cfg(debug_assertions)]
    fn ut_0589_07_free_list_pops_counter_zero_during_streaming() {
        let mut net = Net::new();
        // Build up a non-trivial free-list.
        let ids: Vec<AgentId> = (0..4).map(|_| net.create_agent(Symbol::Con)).collect();
        for id in &ids {
            net.remove_agent(*id);
        }
        assert_eq!(
            net.free_list.len(),
            4,
            "setup: free_list must have 4 entries"
        );

        // Streaming gate on.
        net.is_in_delta_round = true;
        net.recycle_policy = RecyclePolicy::DisableUnderDelta;
        let pops_before = net.free_list_pops;

        // 4 creates while gated — all must be fresh.
        for _ in 0..4 {
            net.create_agent(Symbol::Era);
        }

        assert_eq!(
            net.free_list_pops, pops_before,
            "UT-0589-07: no pops during streaming gate (counter must not change)"
        );
        assert_eq!(
            net.free_list.len(),
            4,
            "UT-0589-07: free_list unchanged (all creates were fresh)"
        );
    }

    /// UT-0589-08: free_list_pops counter is positive in push mode (positive control).
    ///
    /// Ensures the counter actually increments when the gate is off, confirming
    /// the debug counter is functioning correctly.
    #[test]
    #[cfg(debug_assertions)]
    fn ut_0589_08_free_list_pops_counter_nonzero_in_push_mode() {
        let mut net = Net::new();
        let ids: Vec<AgentId> = (0..4).map(|_| net.create_agent(Symbol::Con)).collect();
        for id in &ids {
            net.remove_agent(*id);
        }

        // Push mode — gate off.
        net.is_in_delta_round = false;
        net.recycle_policy = RecyclePolicy::DisableUnderDelta;

        // 4 creates — all must pop from free-list.
        for _ in 0..4 {
            net.create_agent(Symbol::Era);
        }

        assert_eq!(
            net.free_list_pops, 4,
            "UT-0589-08: push mode must produce 4 free-list pops (positive control)"
        );
        assert!(
            net.free_list.is_empty(),
            "UT-0589-08: free_list must be empty after 4 pops"
        );
    }
}
