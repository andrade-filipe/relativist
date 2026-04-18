//! Coordinator-side connectivity tracker for border wires (SPEC-19 §3.2).
//!
//! `BorderGraph` is the coordinator's per-round view of the inter-partition
//! wires (borders) that cross worker boundaries. Unlike the v1 "full merged
//! net" rebuild performed each BSP round by [`crate::merge::merge`], this
//! graph maintains only the endpoint tuples that the coordinator needs to
//! detect, resolve, and remove cross-worker redexes — which is the minimum
//! the delta protocol (§3.3, shipping under item 2.26) has to see.
//!
//! # Coordinator-side lifecycle (R13-R15, ships under item 2.26)
//!
//! The graph exposes five pure primitives that together form the
//! coordinator's round-level vocabulary:
//!
//! 1. [`BorderGraph::from_partition_plan`] — initialize the graph from a
//!    fresh [`crate::partition::PartitionPlan`] at grid start-up (R10).
//! 2. [`BorderGraph::apply_deltas`] — fold per-worker delta batches into
//!    the graph, maintaining the incremental `active_redexes` invariant
//!    (R11, R18). DISCONNECTED sentinel handling per R17.
//! 3. [`BorderGraph::detect_border_redexes`] — enumerate the currently
//!    active cross-worker redexes as owned `Vec<(u32, BorderState)>`
//!    tuples (R12); the owned shape lets the coordinator iterate while
//!    holding `&mut self` for resolution (see spec-critic DC-3).
//! 4. [`BorderGraph::remove_border`] — annihilation removal (R16).
//! 5. [`BorderGraph::add_border_states`] — batch insertion of new borders
//!    produced by CON-DUP dispatch. Input is `Vec<AddBorderEntry>` — the
//!    graph computes `is_redex` from `(side_a, side_b)` via
//!    [`crate::merge::helpers::is_principal_pair`], enforcing R9 at the
//!    primitive boundary (spec-critic DC-4 Option B).
//!
//! Read-only accessors: [`BorderGraph::len`], [`BorderGraph::is_empty`],
//! [`BorderGraph::has_no_redexes`], [`BorderGraph::active_redex_count`].
//!
//! The `worker_borders` reverse index (R23) is populated here but consumed
//! by the worker-dispatch path that ships under item 2.26 (the delta-mode
//! BSP loop in `run_grid_delta`).
//!
//! # Pure-core invariant (R19)
//!
//! `border_graph.rs` lives in `merge/` and MUST remain pure-core: no
//! `tokio`, no `async`, no imports from `crate::protocol`, no I/O. The
//! only dependencies are `crate::net` (types), `crate::partition::types`
//! (plan shape + `WorkerId`), and the local helper
//! [`crate::merge::helpers::is_principal_pair`]. A source-file scan test
//! (`border_graph_source_respects_r19_pure_core_invariant`) enforces this
//! invariant at every `cargo test`.
//!
//! # Out of scope (item 2.26 territory)
//!
//! - Coordinator-side `interact_*` dispatch loop (R13, R14).
//! - Worker-side delta emission and the stateful-worker lifecycle
//!   (R20-R30).
//! - `Message::RoundStart` / `Message::RoundResult` wire-format
//!   extensions.
//! - `GridConfig.delta_mode` flag and the `run_grid_delta` BSP loop
//!   (§3.3, §4.3).

use std::collections::{HashMap, HashSet};

use crate::net::{AgentId, PortRef, Symbol, DISCONNECTED};
use crate::partition::types::{PartitionPlan, WorkerId};

use super::helpers::is_principal_pair;

/// One border's coordinator-side state (SPEC-19 R9).
///
/// A border is a cross-worker wire: two `PortRef` endpoints, one owned by
/// each worker. `is_redex` caches `is_principal_pair(side_a, side_b)` so
/// the coordinator can answer "is this border reducible?" in O(1) without
/// re-inspecting the ports — and so [`BorderGraph::active_redexes`] can be
/// maintained incrementally (R18).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorderState {
    /// Unique border identifier (matches the `border_id` in `PartitionPlan.borders`).
    pub border_id: u32,
    /// Endpoint owned by `worker_a`.
    pub side_a: PortRef,
    /// Endpoint owned by `worker_b`.
    pub side_b: PortRef,
    /// Worker that owns `side_a`.
    pub worker_a: WorkerId,
    /// Worker that owns `side_b`.
    pub worker_b: WorkerId,
    /// Cached principal-pair flag: `is_principal_pair(side_a, side_b)`.
    pub is_redex: bool,
}

/// Coordinator-side connectivity tracker for border wires (SPEC-19 §4.1).
///
/// Maintains the three invariants:
///
/// - `borders`: `border_id -> BorderState` for every alive border.
/// - `worker_borders`: reverse index; `worker_borders[w]` lists every
///   border_id where `worker_a == w` or `worker_b == w`. Populated here;
///   consumed under item 2.26 (R23). Stale entries are tolerated after
///   `remove_border` per §4.2 note.
/// - `active_redexes`: `{bid : borders[bid].is_redex == true}`. Maintained
///   incrementally by `apply_deltas` / `add_border_states` /
///   `remove_border` (R18 SHOULD).
#[derive(Debug, Clone)]
pub struct BorderGraph {
    /// `border_id -> BorderState` for every alive border.
    pub(crate) borders: HashMap<u32, BorderState>,
    /// `worker_borders[w]` lists every border_id that worker `w` participates
    /// in. Indexed by `worker_id as usize`; sized at construction to
    /// `plan.partitions.len()`. Consumed by the worker-dispatch path under
    /// SPEC-19 §4.1 R23 (item 2.26); tolerated stale entries after
    /// `remove_border`.
    // SPEC-19 §4.1 R23 (item 2.26)
    #[allow(dead_code)]
    pub(crate) worker_borders: Vec<Vec<u32>>,
    /// `{bid : borders[bid].is_redex == true}`. Incrementally maintained
    /// across all mutation paths (R18).
    pub(crate) active_redexes: HashSet<u32>,
}

/// A single border mutation reported by a worker at round boundary
/// (SPEC-19 §3.2, R11, R17).
///
/// The `new_target` field carries the worker's new view of *its own side*
/// of the named border. Reconnects use `PortRef::AgentPort(_, _)`;
/// erasures use the DISCONNECTED sentinel
/// (`PortRef::FreePort(u32::MAX)` — see [`crate::net::DISCONNECTED`],
/// spec-critic DC-1).
///
/// `serde::Serialize + Deserialize` derives per SPEC-19 §3.4 R33 (DC-A1,
/// 2026-04-17 amendment). The derives were missing from the §3.2 ship
/// because the §3.2 bundle did not yet consume the wire path; §3.4 needs
/// them so `Vec<BorderDelta>` can appear inside `Message::RoundStart` /
/// `Message::RoundResult` payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BorderDelta {
    /// Border affected by this delta.
    pub border_id: u32,
    /// Worker's new-round view of its own side of the border.
    pub new_target: PortRef,
}

/// Internal port reconnection dispatched coordinator → worker after a
/// CON-DUP border-redex resolution produces new agents whose auxiliary
/// ports are not themselves borders (SPEC-19 §3.3 R23, §3.4 R33, DC-B3
/// — 2026-04-17 amendment).
///
/// The worker MUST apply the reconnection to its stored partition before
/// running `reduce_all` for the round: port `port` of `agent_id` is now
/// connected to `new_target`. Neither `BorderDelta` (which names border
/// ids, not agent ports) nor `resolved_borders` / `new_borders` can
/// express this interior rewire.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LocalReconnection {
    /// Agent on the worker side whose port needs rewiring.
    pub agent_id: AgentId,
    /// Port of that agent (0 = principal, 1..arity = aux).
    pub port: u8,
    /// New endpoint this port connects to.
    pub new_target: PortRef,
}

/// Coordinator → worker AgentId allocation request carried on
/// `Message::RoundStart` (SPEC-19 §3.3 R23, §3.4 R33, R48, DC-B5 —
/// 2026-04-17 amendment).
///
/// Phase 1 of the 2-phase AgentId allocation flow: the coordinator
/// asks the worker to mint one fresh `AgentId` from the worker's own
/// `id_range` (SPEC-04) for a new agent of shape `(symbol_type, arity)`.
/// The worker echoes the minted id back via `MintedAgent` on the next
/// `Message::RoundResult`, correlated by `request_id`.
///
/// `request_id` values are allocated by the coordinator from a
/// monotonically increasing counter scoped to the BSP run (R48); the
/// worker MUST NOT overlap the coordinator-reserved AgentId range
/// `u32::MAX - 10_000 .. u32::MAX`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PendingCommutation {
    /// Correlation id, unique within this BSP run (R48).
    pub request_id: u32,
    /// Symbol type for the new agent (CON, DUP, ERA).
    pub symbol_type: Symbol,
    /// Arity of the new agent (0 for ERA; 2 for CON/DUP — two aux ports).
    pub arity: u8,
}

/// Worker → coordinator response to a `PendingCommutation`, carried on
/// `Message::RoundResult.minted_agents` (SPEC-19 §3.3 R26, §3.4 R33, R48,
/// DC-B5 — 2026-04-17 amendment).
///
/// Pairs the coordinator-issued `request_id` with the worker-allocated
/// `AgentId`. The coordinator treats an unmatched `request_id` as a
/// protocol violation (R48).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MintedAgent {
    /// Matches the `PendingCommutation.request_id` from the paired
    /// `Message::RoundStart` (R48 correlation key).
    pub request_id: u32,
    /// The `AgentId` the worker has allocated for this request. MUST NOT
    /// overlap the coordinator-reserved range
    /// `u32::MAX - 10_000 .. u32::MAX` (R48).
    pub minted_agent_id: AgentId,
}

/// Coordinator's input to [`BorderGraph::add_border_states`] (SPEC-19 R15
/// part 3; spec-critic DC-4 Option B).
///
/// Carries only the connectivity fields. The graph computes `is_redex`
/// from `(side_a, side_b)` via
/// [`crate::merge::helpers::is_principal_pair`], enforcing R9 at the
/// primitive boundary — a caller cannot construct a `BorderState` whose
/// cached `is_redex` disagrees with the endpoints because this input type
/// does not expose the field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddBorderEntry {
    /// Unique border identifier. Must not already be present in the graph.
    pub border_id: u32,
    /// Endpoint owned by `worker_a`.
    pub side_a: PortRef,
    /// Endpoint owned by `worker_b`.
    pub side_b: PortRef,
    /// Worker that owns `side_a`. Must be `< worker_borders.len()`.
    pub worker_a: WorkerId,
    /// Worker that owns `side_b`. Must be `< worker_borders.len()`.
    pub worker_b: WorkerId,
}

impl BorderGraph {
    /// Initialize the graph from a [`PartitionPlan`] (R10).
    ///
    /// Walks every partition's `free_port_index` to collect the two
    /// sightings (owner `WorkerId` + local `PortRef`) of every border.
    /// Then validates the C3 invariant: every `border_id` declared in
    /// `plan.borders` has EXACTLY two sightings, and every sighted
    /// `border_id` appears in `plan.borders`. Panics on violation with
    /// a message naming the offending `border_id` and the sighting count.
    ///
    /// Finally, populates `borders`, `worker_borders` (sized to
    /// `plan.partitions.len()`), and seeds `active_redexes` with the
    /// subset of border_ids whose `is_redex == true`.
    ///
    /// Complexity: O(B) over `sum_i |partitions[i].free_port_index|`.
    pub fn from_partition_plan(plan: &PartitionPlan) -> Self {
        let n_partitions = plan.partitions.len();

        // Collect sightings: border_id -> Vec<(worker_id, port_ref)>.
        //
        // Using a Vec (rather than a fixed-size [Option; 2]) lets us detect
        // triple-or-higher sighting violations explicitly in the validation
        // pass below.
        let mut sightings: HashMap<u32, Vec<(WorkerId, PortRef)>> = HashMap::new();
        for partition in &plan.partitions {
            let worker = partition.worker_id;
            for (bid, port) in &partition.free_port_index {
                sightings.entry(*bid).or_default().push((worker, *port));
            }
        }

        // Validate C3 bidirectionally.
        //
        // 1. Every border_id declared in plan.borders has exactly 2 sightings.
        // 2. Every sighted border_id is declared in plan.borders.
        // (Orphan-in-partition and orphan-in-plan cases, plus triple-sighting.)
        for bid in plan.borders.keys() {
            let count = sightings.get(bid).map(|v| v.len()).unwrap_or(0);
            if count != 2 {
                panic!(
                    "SPEC-19 C3 invariant violated: border_id {bid} has \
                     {count} sightings in partitions (expected exactly 2)"
                );
            }
        }
        for (bid, sights) in &sightings {
            let count = sights.len();
            if count != 2 {
                panic!(
                    "SPEC-19 C3 invariant violated: border_id {bid} has \
                     {count} sightings in partitions (expected exactly 2)"
                );
            }
            if !plan.borders.contains_key(bid) {
                panic!(
                    "SPEC-19 C3 invariant violated: border_id {bid} is \
                     sighted in partitions but absent from plan.borders"
                );
            }
        }

        // Build borders + worker_borders + active_redexes in one pass.
        let mut borders: HashMap<u32, BorderState> = HashMap::with_capacity(sightings.len());
        let mut worker_borders: Vec<Vec<u32>> = vec![Vec::new(); n_partitions];
        let mut active_redexes: HashSet<u32> = HashSet::new();

        for (bid, sights) in sightings {
            // Validated above: exactly 2 entries.
            let (wa, pa) = sights[0];
            let (wb, pb) = sights[1];
            let is_redex = is_principal_pair(pa, pb);
            let state = BorderState {
                border_id: bid,
                side_a: pa,
                side_b: pb,
                worker_a: wa,
                worker_b: wb,
                is_redex,
            };
            borders.insert(bid, state);
            // Indexing: worker ids are dense 0..n_partitions-1 per SPEC-04.
            worker_borders[wa as usize].push(bid);
            worker_borders[wb as usize].push(bid);
            if is_redex {
                active_redexes.insert(bid);
            }
        }

        BorderGraph {
            borders,
            worker_borders,
            active_redexes,
        }
    }

    /// Apply a batch of deltas emitted by one worker at round boundary
    /// (R11, R17, R18).
    ///
    /// For each delta:
    ///
    /// - If the `border_id` is unknown (not in `borders`), silently skip.
    /// - If the `worker_id` owns neither side of the border, silently
    ///   skip (per §3.2 pseudocode).
    /// - Otherwise, update the matching side to `new_target`.
    ///   - If the OTHER side is already DISCONNECTED and the new target
    ///     is also DISCONNECTED, remove the border (R17 double-erasure).
    ///   - Else, recompute `is_redex` from the updated sides and
    ///     incrementally update `active_redexes` (R18).
    ///
    /// Empty slice is a no-op.
    pub fn apply_deltas(&mut self, worker_id: WorkerId, deltas: &[BorderDelta]) {
        for delta in deltas {
            // Unknown border_id: silent skip (spec §3.2 pseudocode).
            let Some(state) = self.borders.get_mut(&delta.border_id) else {
                continue;
            };

            // Dispatch the update to the side owned by `worker_id`.
            let updates_a = state.worker_a == worker_id;
            let updates_b = state.worker_b == worker_id;
            if !updates_a && !updates_b {
                // Worker owns neither side: silent skip.
                continue;
            }

            let was_redex = state.is_redex;

            if updates_a {
                state.side_a = delta.new_target;
            } else {
                // updates_b
                state.side_b = delta.new_target;
            }

            // R17: double-DISCONNECTED ⇒ border dies.
            if state.side_a == DISCONNECTED && state.side_b == DISCONNECTED {
                // Remove the border entirely. worker_borders entries are
                // left stale (spec §4.2 note).
                self.borders.remove(&delta.border_id);
                if was_redex {
                    self.active_redexes.remove(&delta.border_id);
                }
                continue;
            }

            // Recompute is_redex and update active_redexes incrementally.
            let is_redex_now = is_principal_pair(state.side_a, state.side_b);
            state.is_redex = is_redex_now;

            match (was_redex, is_redex_now) {
                (false, true) => {
                    self.active_redexes.insert(delta.border_id);
                }
                (true, false) => {
                    self.active_redexes.remove(&delta.border_id);
                }
                // (true, true) | (false, false): no membership change.
                _ => {}
            }
        }
    }

    /// Enumerate the currently active border redexes (R12, DC-3).
    ///
    /// Returns owned `Vec<(u32, BorderState)>` so the coordinator can
    /// iterate while holding `&mut self` for resolution (e.g., calling
    /// [`apply_deltas`] or [`remove_border`] inside the loop body).
    ///
    /// Complexity: O(|active_redexes|) — iterates the incremental set,
    /// not the full `borders` map (R18).
    ///
    /// The function defensively skips any `border_id` in `active_redexes`
    /// whose entry has been dropped from `borders` (this should not
    /// happen under a correct implementation, but the `filter_map` keeps
    /// a stray invariant violation from producing a panic).
    ///
    /// [`apply_deltas`]: Self::apply_deltas
    /// [`remove_border`]: Self::remove_border
    pub fn detect_border_redexes(&self) -> Vec<(u32, BorderState)> {
        self.active_redexes
            .iter()
            .filter_map(|bid| self.borders.get(bid).map(|s| (*bid, s.clone())))
            .collect()
    }

    /// Total number of alive borders.
    pub fn len(&self) -> usize {
        self.borders.len()
    }

    /// `true` iff no borders are alive.
    ///
    /// Distinct from [`has_no_redexes`](Self::has_no_redexes): a graph
    /// can hold non-redex borders that are still alive.
    pub fn is_empty(&self) -> bool {
        self.borders.is_empty()
    }

    /// `true` iff no border is currently a principal-pair (R12, R18).
    ///
    /// Distinct from [`is_empty`](Self::is_empty): non-redex borders may
    /// still be alive.
    pub fn has_no_redexes(&self) -> bool {
        self.active_redexes.is_empty()
    }

    /// Number of currently active border redexes.
    pub fn active_redex_count(&self) -> usize {
        self.active_redexes.len()
    }

    /// Remove a border (annihilation, R16).
    ///
    /// Returns `Some(state)` if the border was present, `None` otherwise.
    /// Also clears the `active_redexes` entry if the border was a redex.
    /// `worker_borders` entries are intentionally left stale (§4.2 note);
    /// any consumer of `worker_borders` must cross-check against `borders`.
    pub fn remove_border(&mut self, border_id: u32) -> Option<BorderState> {
        let state = self.borders.remove(&border_id)?;
        if state.is_redex {
            self.active_redexes.remove(&border_id);
        }
        Some(state)
    }

    /// Batch-insert new borders produced by CON-DUP dispatch (R15 part 3;
    /// spec-critic DC-4 Option B).
    ///
    /// For each entry:
    ///
    /// - Compute `is_redex` from `(side_a, side_b)` via
    ///   [`is_principal_pair`] — the caller does NOT supply this bit.
    /// - Insert into `borders`, `worker_borders[worker_a]`,
    ///   `worker_borders[worker_b]`, and (if redex) `active_redexes`.
    ///
    /// Panics:
    /// - `duplicate border_id {bid} in add_border_states` — if an entry's
    ///   `border_id` is already present in `self.borders`.
    /// - `out-of-bounds worker {w} for border_id {bid}` — if either
    ///   `worker_a` or `worker_b` is `>= self.worker_borders.len()`.
    ///
    /// Empty input vector is a no-op.
    pub fn add_border_states(&mut self, entries: Vec<AddBorderEntry>) {
        for entry in entries {
            // Defensive panic 1: duplicate border_id.
            if self.borders.contains_key(&entry.border_id) {
                panic!(
                    "duplicate border_id {} in add_border_states",
                    entry.border_id
                );
            }
            // Defensive panic 2: worker_a / worker_b bounds.
            let n_workers = self.worker_borders.len();
            if (entry.worker_a as usize) >= n_workers {
                panic!(
                    "out-of-bounds worker {} for border_id {}",
                    entry.worker_a, entry.border_id
                );
            }
            if (entry.worker_b as usize) >= n_workers {
                panic!(
                    "out-of-bounds worker {} for border_id {}",
                    entry.worker_b, entry.border_id
                );
            }

            let is_redex = is_principal_pair(entry.side_a, entry.side_b);
            let state = BorderState {
                border_id: entry.border_id,
                side_a: entry.side_a,
                side_b: entry.side_b,
                worker_a: entry.worker_a,
                worker_b: entry.worker_b,
                is_redex,
            };
            self.borders.insert(entry.border_id, state);
            self.worker_borders[entry.worker_a as usize].push(entry.border_id);
            self.worker_borders[entry.worker_b as usize].push(entry.border_id);
            if is_redex {
                self.active_redexes.insert(entry.border_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{Net, Symbol};
    use crate::partition::types::{IdRange, Partition};

    // -----------------------------------------------------------------
    // Shared fixtures (used across TEST-SPEC-0360..0365).
    // -----------------------------------------------------------------

    /// Principal port on agent `id` (slot 0).
    fn p(id: u32) -> PortRef {
        PortRef::AgentPort(id, 0)
    }

    /// Auxiliary port on agent `id` at slot `slot`.
    fn aux(id: u32, slot: u8) -> PortRef {
        PortRef::AgentPort(id, slot)
    }

    /// Build a `PartitionPlan` with one or more partitions and an optional
    /// border declaration set. Each `(worker_id, free_port_entries)`
    /// tuple becomes one `Partition`. `border_decls` lists the
    /// `border_id`s that will be declared in `plan.borders` with dummy
    /// endpoint values (tests here only depend on the KEY set of
    /// `plan.borders` for C3 validation).
    fn make_plan(
        partitions: Vec<(WorkerId, Vec<(u32, PortRef)>)>,
        border_decls: Vec<u32>,
    ) -> PartitionPlan {
        let mut built = Vec::with_capacity(partitions.len());
        for (worker_id, entries) in partitions {
            let mut free_port_index = HashMap::new();
            for (bid, port) in entries {
                free_port_index.insert(bid, port);
            }
            let mut subnet = Net::new();
            // Touch the subnet so non-trivial fixtures are easier to
            // reason about; not required for BorderGraph logic.
            let _ = subnet.create_agent(Symbol::Era);
            built.push(Partition {
                subnet,
                worker_id,
                free_port_index,
                id_range: IdRange {
                    start: 0,
                    end: u32::MAX,
                },
                border_id_start: 0,
                border_id_end: u32::MAX,
            });
        }
        let mut borders: HashMap<u32, (PortRef, PortRef)> = HashMap::new();
        for bid in border_decls {
            borders.insert(bid, (PortRef::FreePort(0), PortRef::FreePort(0)));
        }
        PartitionPlan {
            partitions: built,
            borders,
        }
    }

    /// Build a 2-worker graph with exactly one border (id = 1). Worker 0
    /// owns `side_a`; worker 1 owns `side_b`.
    fn make_graph_with_one_border(side_a: PortRef, side_b: PortRef) -> BorderGraph {
        let plan = make_plan(
            vec![(0, vec![(1, side_a)]), (1, vec![(1, side_b)])],
            vec![1],
        );
        BorderGraph::from_partition_plan(&plan)
    }

    /// Build a 2-worker graph with three borders:
    ///  - 10: principal / auxiliary (not a redex)
    ///  - 20: principal / principal (redex)
    ///  - 30: principal / principal (redex)
    fn make_graph_with_three_borders() -> BorderGraph {
        let plan = make_plan(
            vec![
                (0, vec![(10, p(0)), (20, p(1)), (30, p(2))]),
                (1, vec![(10, aux(5, 1)), (20, p(6)), (30, p(7))]),
            ],
            vec![10, 20, 30],
        );
        BorderGraph::from_partition_plan(&plan)
    }

    /// Build a 2-worker graph with zero borders.
    fn make_empty_two_worker_graph() -> BorderGraph {
        let plan = make_plan(vec![(0, vec![]), (1, vec![])], vec![]);
        BorderGraph::from_partition_plan(&plan)
    }

    // -----------------------------------------------------------------
    // TEST-SPEC-0360 — struct shape, derives, module wiring.
    // -----------------------------------------------------------------

    /// UT-0360-01: BorderState has exactly the six fields R9 mandates.
    #[test]
    fn border_state_has_exact_six_fields_in_r9_order() {
        let state = BorderState {
            border_id: 7,
            side_a: PortRef::AgentPort(1, 0),
            side_b: PortRef::AgentPort(2, 0),
            worker_a: 0,
            worker_b: 1,
            is_redex: true,
        };
        assert_eq!(state.border_id, 7);
        assert_eq!(state.side_a, PortRef::AgentPort(1, 0));
        assert_eq!(state.side_b, PortRef::AgentPort(2, 0));
        assert_eq!(state.worker_a, 0);
        assert_eq!(state.worker_b, 1);
        assert!(state.is_redex);
    }

    /// UT-0360-02: `#[derive(Debug)]` active on BorderState.
    #[test]
    fn border_state_debug_derive_produces_non_empty_string() {
        let state = BorderState {
            border_id: 42,
            side_a: PortRef::AgentPort(1, 0),
            side_b: PortRef::AgentPort(2, 0),
            worker_a: 0,
            worker_b: 1,
            is_redex: false,
        };
        let s = format!("{state:?}");
        assert!(
            s.contains("BorderState"),
            "Debug output must contain type name `BorderState`; got {s}"
        );
        assert!(
            s.contains("42"),
            "Debug output must contain the border_id field value; got {s}"
        );
    }

    /// UT-0360-03: `#[derive(Clone, PartialEq, Eq)]` active on BorderState.
    #[test]
    fn border_state_clone_is_value_equal() {
        let state = BorderState {
            border_id: 99,
            side_a: PortRef::AgentPort(3, 0),
            side_b: PortRef::AgentPort(4, 1),
            worker_a: 0,
            worker_b: 2,
            is_redex: false,
        };
        let cloned = state.clone();
        assert_eq!(state, cloned);
    }

    /// UT-0360-04: Every field participates in PartialEq.
    #[test]
    fn border_state_inequality_when_any_field_differs() {
        let base = BorderState {
            border_id: 1,
            side_a: PortRef::AgentPort(1, 0),
            side_b: PortRef::AgentPort(2, 0),
            worker_a: 0,
            worker_b: 1,
            is_redex: true,
        };
        assert_ne!(
            base,
            BorderState {
                border_id: 2,
                ..base.clone()
            }
        );
        assert_ne!(
            base,
            BorderState {
                side_a: PortRef::AgentPort(9, 0),
                ..base.clone()
            }
        );
        assert_ne!(
            base,
            BorderState {
                side_b: PortRef::AgentPort(9, 0),
                ..base.clone()
            }
        );
        assert_ne!(
            base,
            BorderState {
                worker_a: 5,
                ..base.clone()
            }
        );
        assert_ne!(
            base,
            BorderState {
                worker_b: 5,
                ..base.clone()
            }
        );
        assert_ne!(
            base,
            BorderState {
                is_redex: false,
                ..base.clone()
            }
        );
    }

    /// UT-0360-05: Direct struct-literal construction yields all-empty graph.
    #[test]
    fn border_graph_default_construction_is_empty() {
        let graph = BorderGraph {
            borders: HashMap::new(),
            worker_borders: Vec::new(),
            active_redexes: HashSet::new(),
        };
        assert_eq!(graph.borders.len(), 0);
        assert_eq!(graph.worker_borders.len(), 0);
        assert_eq!(graph.active_redexes.len(), 0);
    }

    /// UT-0360-06: Debug + Clone derives on BorderGraph.
    #[test]
    fn border_graph_derive_shape_debug_and_clone() {
        let graph = BorderGraph {
            borders: HashMap::new(),
            worker_borders: Vec::new(),
            active_redexes: HashSet::new(),
        };
        let s = format!("{graph:?}");
        assert!(
            s.contains("BorderGraph"),
            "Debug output must contain type name `BorderGraph`; got {s}"
        );
        let cloned = graph.clone();
        assert_eq!(cloned.borders.len(), 0);
        assert_eq!(cloned.worker_borders.len(), 0);
        assert_eq!(cloned.active_redexes.len(), 0);
    }

    /// UT-0360-07: `is_principal_pair` is reachable via re-export
    /// (spec-critic Additional observation #2).
    #[test]
    fn is_principal_pair_is_reachable_via_helpers_reexport() {
        assert!(
            is_principal_pair(PortRef::AgentPort(1, 0), PortRef::AgentPort(2, 0)),
            "principal vs principal MUST be true"
        );
        assert!(
            !is_principal_pair(PortRef::AgentPort(1, 0), PortRef::AgentPort(2, 1)),
            "principal vs auxiliary MUST be false"
        );
    }

    /// UT-0360-08: Module wiring: `crate::merge::BorderGraph` and
    /// `crate::merge::BorderState` re-exports resolve.
    #[test]
    fn border_graph_module_is_wired_in_merge_mod() {
        let _ct_borderstate: fn() -> crate::merge::BorderState = || BorderState {
            border_id: 0,
            side_a: PortRef::AgentPort(0, 0),
            side_b: PortRef::AgentPort(0, 0),
            worker_a: 0,
            worker_b: 0,
            is_redex: false,
        };
        let _ct_bordergraph: fn() -> crate::merge::BorderGraph = || BorderGraph {
            borders: HashMap::new(),
            worker_borders: Vec::new(),
            active_redexes: HashSet::new(),
        };
        let bs: crate::merge::BorderState = _ct_borderstate();
        assert_eq!(bs.border_id, 0);
    }

    // -----------------------------------------------------------------
    // TEST-SPEC-0361 — `from_partition_plan`.
    // -----------------------------------------------------------------

    /// UT-0361-01: zero-border plan ⇒ fully empty graph.
    #[test]
    fn from_partition_plan_empty_zero_borders() {
        let plan = make_plan(vec![(0, vec![])], vec![]);
        let graph = BorderGraph::from_partition_plan(&plan);
        assert!(graph.borders.is_empty());
        assert_eq!(graph.worker_borders.len(), 1);
        assert!(graph.worker_borders[0].is_empty());
        assert!(graph.active_redexes.is_empty());
    }

    /// UT-0361-02: principal/principal border ⇒ marked redex, in active set.
    #[test]
    fn from_partition_plan_single_principal_principal_border_marks_redex() {
        let plan = make_plan(vec![(0, vec![(42, p(7))]), (1, vec![(42, p(8))])], vec![42]);
        let graph = BorderGraph::from_partition_plan(&plan);
        assert_eq!(graph.borders.len(), 1);
        assert!(graph.borders.contains_key(&42));
        let state = graph.borders.get(&42).expect("border 42 present");
        assert!(state.is_redex);
        assert!(graph.active_redexes.contains(&42));
        assert_eq!(graph.active_redexes.len(), 1);
        let owners: HashSet<WorkerId> = [state.worker_a, state.worker_b].iter().copied().collect();
        assert_eq!(owners, HashSet::from([0, 1]));
        assert!(graph.worker_borders[0].contains(&42));
        assert!(graph.worker_borders[1].contains(&42));
    }

    /// UT-0361-03: principal/aux border ⇒ NOT redex.
    #[test]
    fn from_partition_plan_principal_aux_is_not_redex() {
        let plan = make_plan(
            vec![(0, vec![(7, p(3))]), (1, vec![(7, aux(4, 1))])],
            vec![7],
        );
        let graph = BorderGraph::from_partition_plan(&plan);
        assert_eq!(graph.borders.len(), 1);
        let state = graph.borders.get(&7).expect("border 7 present");
        assert!(!state.is_redex);
        assert!(graph.active_redexes.is_empty());
    }

    /// UT-0361-04: multi-border fixture ⇒ worker_borders populated for every
    /// border the worker participates in.
    #[test]
    fn from_partition_plan_two_borders_worker_borders_all_populated() {
        let plan = make_plan(
            vec![
                (0, vec![(10, p(0)), (20, aux(1, 1))]),
                (1, vec![(10, aux(2, 1)), (20, p(3))]),
            ],
            vec![10, 20],
        );
        let graph = BorderGraph::from_partition_plan(&plan);
        assert_eq!(graph.borders.len(), 2);
        assert_eq!(graph.worker_borders.len(), 2);
        assert_eq!(graph.worker_borders[0].len(), 2);
        assert_eq!(graph.worker_borders[1].len(), 2);
        let worker0: HashSet<u32> = graph.worker_borders[0].iter().copied().collect();
        assert_eq!(worker0, HashSet::from([10, 20]));
        let worker1: HashSet<u32> = graph.worker_borders[1].iter().copied().collect();
        assert_eq!(worker1, HashSet::from([10, 20]));
        assert!(graph.active_redexes.is_empty());
    }

    /// UT-0361-05: mixed redex + non-redex ⇒ active_redexes equals the redex
    /// subset exactly.
    #[test]
    fn from_partition_plan_mixed_redex_and_non_redex_seeds_active_set_correctly() {
        let plan = make_plan(
            vec![
                (0, vec![(100, p(0)), (101, p(1)), (102, p(2))]),
                (1, vec![(100, p(10)), (101, aux(11, 1)), (102, p(12))]),
            ],
            vec![100, 101, 102],
        );
        let graph = BorderGraph::from_partition_plan(&plan);
        assert_eq!(graph.borders.len(), 3);
        assert!(graph.borders.get(&100).unwrap().is_redex);
        assert!(!graph.borders.get(&101).unwrap().is_redex);
        assert!(graph.borders.get(&102).unwrap().is_redex);
        let expected: HashSet<u32> = HashSet::from([100, 102]);
        let actual: HashSet<u32> = graph.active_redexes.iter().copied().collect();
        assert_eq!(actual, expected);
    }

    /// UT-0361-06: orphan border declared in plan.borders ⇒ panic.
    #[test]
    #[should_panic(expected = "99")]
    fn from_partition_plan_panics_on_orphan_border() {
        let plan = make_plan(vec![(0, vec![]), (1, vec![])], vec![99]);
        let _ = BorderGraph::from_partition_plan(&plan);
    }

    /// UT-0361-07: triple-sighting ⇒ panic (message includes sighting count
    /// 3 and/or border_id 55).
    #[test]
    #[should_panic(expected = "3")]
    fn from_partition_plan_panics_on_triple_sighting() {
        let plan = make_plan(
            vec![
                (0, vec![(55, p(0))]),
                (1, vec![(55, p(1))]),
                (2, vec![(55, p(2))]),
            ],
            vec![55],
        );
        let _ = BorderGraph::from_partition_plan(&plan);
    }

    /// UT-0361-08: orphan free_port_index entry (border sighted but not
    /// declared in plan.borders) ⇒ panic.
    #[test]
    #[should_panic(expected = "77")]
    fn from_partition_plan_panics_on_orphan_free_port_entry() {
        let plan = make_plan(vec![(0, vec![(77, p(0))]), (1, vec![(77, p(1))])], vec![]);
        let _ = BorderGraph::from_partition_plan(&plan);
    }

    // -----------------------------------------------------------------
    // TEST-SPEC-0362 — `apply_deltas`.
    // -----------------------------------------------------------------

    /// UT-0362-01: principal → principal upgrade ⇒ is_redex flips to true.
    #[test]
    fn apply_delta_principal_to_principal_marks_redex() {
        let mut graph = make_graph_with_one_border(p(0), aux(1, 1));
        assert!(!graph.borders.get(&1).unwrap().is_redex);
        assert!(graph.active_redexes.is_empty());
        // Worker that owns the auxiliary side upgrades to principal.
        let state_before = graph.borders.get(&1).unwrap();
        let aux_side_worker = if state_before.side_a == aux(1, 1) {
            state_before.worker_a
        } else {
            state_before.worker_b
        };
        let delta = BorderDelta {
            border_id: 1,
            new_target: p(9),
        };
        graph.apply_deltas(aux_side_worker, &[delta]);
        let state = graph.borders.get(&1).expect("border still present");
        assert!(state.is_redex);
        assert!(graph.active_redexes.contains(&1));
        assert_eq!(graph.active_redexes.len(), 1);
        // The side that was upgraded now equals p(9); the other is unchanged.
        let endpoints: HashSet<PortRef> = [state.side_a, state.side_b].into_iter().collect();
        assert_eq!(endpoints, HashSet::from([p(0), p(9)]));
    }

    /// UT-0362-02: principal → aux demotion ⇒ is_redex flips to false.
    #[test]
    fn apply_delta_principal_to_aux_clears_redex() {
        let mut graph = make_graph_with_one_border(p(0), p(1));
        assert!(graph.borders.get(&1).unwrap().is_redex);
        assert!(graph.active_redexes.contains(&1));
        let state_before = graph.borders.get(&1).unwrap();
        // Demote side_a (worker_a's side).
        let target_worker = state_before.worker_a;
        let delta = BorderDelta {
            border_id: 1,
            new_target: aux(5, 1),
        };
        graph.apply_deltas(target_worker, &[delta]);
        let state = graph.borders.get(&1).unwrap();
        assert!(!state.is_redex);
        assert!(!graph.active_redexes.contains(&1));
        assert!(graph.active_redexes.is_empty());
        assert_eq!(state.side_a, aux(5, 1));
    }

    /// UT-0362-03: delta from a worker owning neither side ⇒ silent skip.
    #[test]
    fn apply_delta_wrong_worker_silent_skip() {
        let mut graph = make_graph_with_one_border(p(0), aux(1, 1));
        let state_before = graph.borders.get(&1).unwrap().clone();
        let delta = BorderDelta {
            border_id: 1,
            new_target: p(99),
        };
        graph.apply_deltas(42, &[delta]);
        let state_after = graph.borders.get(&1).unwrap();
        assert_eq!(*state_after, state_before);
    }

    /// UT-0362-04: unknown border_id ⇒ silent skip.
    #[test]
    fn apply_delta_unknown_border_silent_skip() {
        let mut graph = make_graph_with_one_border(p(0), aux(1, 1));
        let n_before = graph.borders.len();
        let delta = BorderDelta {
            border_id: 999,
            new_target: p(0),
        };
        graph.apply_deltas(0, &[delta]);
        assert_eq!(graph.borders.len(), n_before);
    }

    /// UT-0362-05: single-side DISCONNECTED ⇒ border alive, is_redex cleared.
    #[test]
    fn apply_delta_disconnect_one_side_keeps_border() {
        let mut graph = make_graph_with_one_border(p(0), p(1));
        assert!(graph.borders.get(&1).unwrap().is_redex);
        let state_before = graph.borders.get(&1).unwrap().clone();
        let target_worker = state_before.worker_a;
        let target_side_before = state_before.side_a;
        let other_side_before = state_before.side_b;
        let delta = BorderDelta {
            border_id: 1,
            new_target: DISCONNECTED,
        };
        graph.apply_deltas(target_worker, &[delta]);
        let state = graph.borders.get(&1).expect("border still alive");
        assert_eq!(state.side_a, DISCONNECTED);
        // other side_b is untouched.
        assert_eq!(state.side_b, other_side_before);
        // The formerly "target" side now differs.
        assert_ne!(state.side_a, target_side_before);
        assert!(!state.is_redex);
        assert!(!graph.active_redexes.contains(&1));
    }

    /// UT-0362-06: both sides DISCONNECTED ⇒ border removed.
    #[test]
    fn apply_delta_disconnect_both_sides_removes_border() {
        let mut graph = make_graph_with_one_border(p(0), p(1));
        let state_before = graph.borders.get(&1).unwrap().clone();
        let (wa, wb) = (state_before.worker_a, state_before.worker_b);
        let disc = BorderDelta {
            border_id: 1,
            new_target: DISCONNECTED,
        };
        graph.apply_deltas(wa, &[disc]);
        graph.apply_deltas(wb, &[disc]);
        assert!(!graph.borders.contains_key(&1));
        assert!(!graph.active_redexes.contains(&1));
    }

    /// UT-0362-07: empty delta batch ⇒ no-op.
    #[test]
    fn apply_deltas_empty_slice_is_noop() {
        let mut graph = make_graph_with_one_border(p(0), p(1));
        let snapshot = graph.clone();
        graph.apply_deltas(0, &[]);
        assert_eq!(graph.borders.len(), snapshot.borders.len());
        assert_eq!(graph.active_redexes.len(), snapshot.active_redexes.len());
        assert_eq!(graph.borders.get(&1), snapshot.borders.get(&1));
    }

    /// UT-0362-08: mixed batch — some redex-creating, some redex-dissolving,
    /// some silent-skip — each applied exactly once; invariant preserved.
    #[test]
    fn apply_deltas_batch_mixed_targets_applied_per_delta() {
        let plan = make_plan(
            vec![
                (0, vec![(1, p(10)), (2, p(20)), (3, p(30))]),
                (1, vec![(1, aux(11, 1)), (2, p(21)), (3, p(31))]),
            ],
            vec![1, 2, 3],
        );
        let mut graph = BorderGraph::from_partition_plan(&plan);
        // Worker 1 owns whichever side it sighted; apply deltas as "worker 1".
        let deltas = [
            BorderDelta {
                border_id: 1,
                new_target: p(40),
            },
            BorderDelta {
                border_id: 2,
                new_target: aux(50, 1),
            },
            BorderDelta {
                border_id: 999,
                new_target: p(60),
            },
        ];
        graph.apply_deltas(1, &deltas);
        assert!(graph.borders.get(&1).unwrap().is_redex);
        assert!(graph.active_redexes.contains(&1));
        assert!(!graph.borders.get(&2).unwrap().is_redex);
        assert!(!graph.active_redexes.contains(&2));
        assert!(graph.borders.get(&3).unwrap().is_redex);
        assert!(graph.active_redexes.contains(&3));
        assert!(!graph.borders.contains_key(&999));
        // Cross-sectional invariant.
        let from_borders: HashSet<u32> = graph
            .borders
            .iter()
            .filter(|(_, s)| s.is_redex)
            .map(|(bid, _)| *bid)
            .collect();
        let from_active: HashSet<u32> = graph.active_redexes.iter().copied().collect();
        assert_eq!(from_active, from_borders);
    }

    /// UT-0362-09: redundant same-value delta ⇒ active_redexes unchanged.
    #[test]
    fn apply_delta_redundant_no_change_keeps_active_redexes_stable() {
        let mut graph = make_graph_with_one_border(p(0), p(1));
        let state_before = graph.borders.get(&1).unwrap().clone();
        let target_worker = state_before.worker_a;
        let same_target = state_before.side_a;
        let delta = BorderDelta {
            border_id: 1,
            new_target: same_target,
        };
        graph.apply_deltas(target_worker, &[delta]);
        assert!(graph.borders.get(&1).unwrap().is_redex);
        assert!(graph.active_redexes.contains(&1));
        assert_eq!(graph.active_redexes.len(), 1);
    }

    /// UT-0362-10: BorderDelta shape + all derives.
    #[test]
    fn border_delta_struct_derives_debug_clone_copy_eq() {
        let d1 = BorderDelta {
            border_id: 5,
            new_target: p(7),
        };
        let d2 = d1; // Copy
        #[allow(clippy::clone_on_copy)]
        let d3 = d1.clone(); // Clone
        assert_eq!(d1, d2);
        assert_eq!(d1, d3);
        let s = format!("{d1:?}");
        assert!(s.contains("BorderDelta"));
        assert!(s.contains('5'));
        assert_ne!(
            d1,
            BorderDelta {
                border_id: 6,
                new_target: p(7)
            }
        );
        assert_ne!(
            d1,
            BorderDelta {
                border_id: 5,
                new_target: p(8)
            }
        );
    }

    /// UT-0362-11: disconnect then reconnect ⇒ invariant tracked across
    /// all transitions.
    #[test]
    fn apply_deltas_preserves_incremental_invariant_under_disconnect_then_reconnect() {
        let mut graph = make_graph_with_one_border(p(0), p(1));
        assert!(graph.active_redexes.contains(&1));
        let state_t0 = graph.borders.get(&1).unwrap().clone();
        let target_worker = state_t0.worker_a;
        // Step 1: disconnect.
        graph.apply_deltas(
            target_worker,
            &[BorderDelta {
                border_id: 1,
                new_target: DISCONNECTED,
            }],
        );
        assert!(graph.borders.contains_key(&1));
        assert!(!graph.active_redexes.contains(&1));
        // Step 2: reconnect via new principal.
        graph.apply_deltas(
            target_worker,
            &[BorderDelta {
                border_id: 1,
                new_target: p(99),
            }],
        );
        let state = graph.borders.get(&1).expect("border still alive");
        let endpoints: HashSet<PortRef> = [state.side_a, state.side_b].into_iter().collect();
        assert_eq!(endpoints, HashSet::from([p(99), state_t0.side_b]));
        assert!(state.is_redex);
        assert!(graph.active_redexes.contains(&1));
    }

    // -----------------------------------------------------------------
    // TEST-SPEC-0363 — `detect_border_redexes` + read-only accessors.
    // -----------------------------------------------------------------

    /// UT-0363-01: no redexes ⇒ empty vec + accessor consistency.
    #[test]
    fn detect_returns_empty_vec_when_no_redexes() {
        let plan = make_plan(
            vec![
                (0, vec![(1, p(0)), (2, p(1))]),
                (1, vec![(1, aux(3, 1)), (2, aux(4, 1))]),
            ],
            vec![1, 2],
        );
        let graph = BorderGraph::from_partition_plan(&plan);
        let redexes: Vec<(u32, BorderState)> = graph.detect_border_redexes();
        assert!(redexes.is_empty());
        assert!(graph.has_no_redexes());
        assert_eq!(graph.active_redex_count(), 0);
        assert_eq!(graph.len(), 2);
        assert!(!graph.is_empty());
    }

    /// UT-0363-02: single redex ⇒ owned `(bid, BorderState)`.
    #[test]
    fn detect_returns_owned_single_redex() {
        let graph = make_graph_with_one_border(p(0), p(1));
        let redexes: Vec<(u32, BorderState)> = graph.detect_border_redexes();
        assert_eq!(redexes.len(), 1);
        let (bid, state) = &redexes[0];
        assert_eq!(*bid, 1);
        assert!(state.is_redex);
    }

    /// UT-0363-03: DC-3 load-bearing: owned return enables `&mut self`
    /// inside the iteration loop.
    #[test]
    fn detect_returns_owned_vec_usable_with_mut_self_borrow() {
        let mut graph = make_graph_with_three_borders();
        let redexes: Vec<(u32, BorderState)> = graph.detect_border_redexes();
        for (bid, state_owned) in &redexes {
            // The delta must be applied from whichever worker owns a
            // principal-side (each principal-principal border — both workers
            // own a principal side). Pick worker_a deterministically.
            let target_worker = state_owned.worker_a;
            // Mutable borrow of `graph` would be impossible if `state_owned`
            // were `&BorderState`. Under DC-3, state_owned is owned.
            graph.apply_deltas(
                target_worker,
                &[BorderDelta {
                    border_id: *bid,
                    new_target: aux(200, 1),
                }],
            );
        }
        assert!(graph.active_redexes.is_empty());
    }

    /// UT-0363-04: multiple redexes ⇒ returned ids equal {20, 30} as a set.
    #[test]
    fn detect_returns_multiple_redexes_order_independent() {
        let graph = make_graph_with_three_borders();
        let redexes: Vec<(u32, BorderState)> = graph.detect_border_redexes();
        assert_eq!(redexes.len(), 2);
        let ids: HashSet<u32> = redexes.iter().map(|(bid, _)| *bid).collect();
        assert_eq!(ids, HashSet::from([20, 30]));
        for (_, state) in &redexes {
            assert!(state.is_redex);
        }
    }

    /// UT-0363-05: `detect` reflects `apply_deltas` transitions with no
    /// caching.
    #[test]
    fn detect_reflects_apply_deltas_transitions() {
        let mut graph = make_graph_with_one_border(p(0), aux(1, 1));
        assert!(graph.detect_border_redexes().is_empty());
        let state = graph.borders.get(&1).unwrap().clone();
        let aux_worker = if state.side_a == aux(1, 1) {
            state.worker_a
        } else {
            state.worker_b
        };
        graph.apply_deltas(
            aux_worker,
            &[BorderDelta {
                border_id: 1,
                new_target: p(9),
            }],
        );
        let t1 = graph.detect_border_redexes();
        assert_eq!(t1.len(), 1);
        assert_eq!(t1[0].0, 1);
        assert!(t1[0].1.is_redex);
        graph.apply_deltas(
            aux_worker,
            &[BorderDelta {
                border_id: 1,
                new_target: aux(9, 1),
            }],
        );
        let t2 = graph.detect_border_redexes();
        assert!(t2.is_empty());
    }

    /// UT-0363-06: `len`/`is_empty`/`has_no_redexes` are independent.
    #[test]
    fn len_and_is_empty_track_alive_borders_not_redex_count() {
        let graph = make_graph_with_one_border(p(0), aux(1, 1));
        assert_eq!(graph.len(), 1);
        assert!(!graph.is_empty());
        assert!(graph.has_no_redexes());
        assert_eq!(graph.active_redex_count(), 0);
        assert_eq!(graph.detect_border_redexes().len(), 0);
    }

    /// UT-0363-07: detect iterates active_redexes (O(|active|)), not
    /// borders — 1000 non-redex + 1 redex returns exactly 1.
    #[test]
    fn detect_complexity_iterates_active_redexes_not_borders() {
        let mut partition0 = vec![];
        let mut partition1 = vec![];
        let mut decls = vec![];
        for bid in 0u32..1000 {
            partition0.push((bid, p(bid * 2)));
            partition1.push((bid, aux(bid * 2 + 1, 1)));
            decls.push(bid);
        }
        partition0.push((1000, p(5000)));
        partition1.push((1000, p(5001)));
        decls.push(1000);
        let plan = make_plan(vec![(0, partition0), (1, partition1)], decls);
        let graph = BorderGraph::from_partition_plan(&plan);
        let redexes = graph.detect_border_redexes();
        assert_eq!(graph.len(), 1001);
        assert_eq!(redexes.len(), 1);
        assert_eq!(redexes[0].0, 1000);
        assert!(redexes[0].1.is_redex);
        assert_eq!(redexes.len(), graph.active_redex_count());
    }

    /// UT-0363-08: after R17 double-disconnect, the dead border is
    /// omitted from detect output.
    #[test]
    fn detect_after_r17_removal_omits_dead_border() {
        let mut graph = make_graph_with_one_border(p(0), p(1));
        let state = graph.borders.get(&1).unwrap().clone();
        let (wa, wb) = (state.worker_a, state.worker_b);
        graph.apply_deltas(
            wa,
            &[BorderDelta {
                border_id: 1,
                new_target: DISCONNECTED,
            }],
        );
        graph.apply_deltas(
            wb,
            &[BorderDelta {
                border_id: 1,
                new_target: DISCONNECTED,
            }],
        );
        assert!(!graph.borders.contains_key(&1));
        let redexes = graph.detect_border_redexes();
        assert!(redexes.is_empty());
        assert_eq!(graph.len(), 0);
        assert!(graph.is_empty());
        assert!(graph.has_no_redexes());
    }

    // -----------------------------------------------------------------
    // TEST-SPEC-0364 — `remove_border` + `add_border_states`.
    // -----------------------------------------------------------------

    /// UT-0364-01: remove present border ⇒ Some(state); map loses entry.
    #[test]
    fn remove_border_present_returns_state_and_clears_map() {
        let mut graph = make_graph_with_one_border(p(0), p(1));
        assert!(graph.borders.contains_key(&1));
        let removed = graph.remove_border(1);
        let state = removed.expect("border 1 present; remove should return Some");
        assert_eq!(state.border_id, 1);
        let endpoints: HashSet<PortRef> = [state.side_a, state.side_b].into_iter().collect();
        assert_eq!(endpoints, HashSet::from([p(0), p(1)]));
        assert!(!graph.borders.contains_key(&1));
        assert_eq!(graph.len(), 0);
    }

    /// UT-0364-02: remove absent border ⇒ None.
    #[test]
    fn remove_border_absent_returns_none() {
        let mut graph = make_empty_two_worker_graph();
        let removed = graph.remove_border(999);
        assert!(removed.is_none());
        assert!(graph.borders.is_empty());
    }

    /// UT-0364-03: remove clears active_redexes entry.
    #[test]
    fn remove_border_clears_active_redex_membership() {
        let mut graph = make_graph_with_one_border(p(0), p(1));
        assert!(graph.active_redexes.contains(&1));
        let _ = graph.remove_border(1);
        assert!(!graph.active_redexes.contains(&1));
        assert!(graph.has_no_redexes());
        assert_eq!(graph.active_redex_count(), 0);
    }

    /// UT-0364-04: remove leaves worker_borders stale (§4.2 note).
    #[test]
    fn remove_border_leaves_worker_borders_stale() {
        let mut graph = make_graph_with_one_border(p(0), p(1));
        assert!(graph.worker_borders[0].contains(&1));
        assert!(graph.worker_borders[1].contains(&1));
        let _ = graph.remove_border(1);
        assert!(graph.worker_borders[0].contains(&1));
        assert!(graph.worker_borders[1].contains(&1));
    }

    /// UT-0364-05: add principal/principal entry ⇒ stored is_redex=true,
    /// in active_redexes.
    #[test]
    fn add_border_states_inserts_redex_entry_with_graph_derived_bit() {
        let mut graph = make_empty_two_worker_graph();
        let entry = AddBorderEntry {
            border_id: 50,
            side_a: p(10),
            side_b: p(20),
            worker_a: 0,
            worker_b: 1,
        };
        graph.add_border_states(vec![entry]);
        assert_eq!(graph.len(), 1);
        let state = graph.borders.get(&50).expect("border 50 inserted");
        assert_eq!(state.border_id, 50);
        assert_eq!(state.side_a, p(10));
        assert_eq!(state.side_b, p(20));
        assert_eq!(state.worker_a, 0);
        assert_eq!(state.worker_b, 1);
        assert!(state.is_redex);
        assert!(graph.active_redexes.contains(&50));
    }

    /// UT-0364-06: add principal/aux entry ⇒ stored is_redex=false.
    #[test]
    fn add_border_states_inserts_non_redex_entry_with_graph_derived_bit() {
        let mut graph = make_empty_two_worker_graph();
        let entry = AddBorderEntry {
            border_id: 51,
            side_a: p(10),
            side_b: aux(20, 1),
            worker_a: 0,
            worker_b: 1,
        };
        graph.add_border_states(vec![entry]);
        let state = graph.borders.get(&51).expect("border 51 inserted");
        assert!(!state.is_redex);
        assert!(!graph.active_redexes.contains(&51));
    }

    /// UT-0364-07: add updates worker_borders for BOTH sides.
    #[test]
    fn add_border_states_updates_worker_borders_for_both_sides() {
        let mut graph = make_empty_two_worker_graph();
        let entry = AddBorderEntry {
            border_id: 52,
            side_a: p(10),
            side_b: aux(20, 1),
            worker_a: 0,
            worker_b: 1,
        };
        graph.add_border_states(vec![entry]);
        assert!(graph.worker_borders[0].contains(&52));
        assert!(graph.worker_borders[1].contains(&52));
    }

    /// UT-0364-08: batch insertion + invariant.
    #[test]
    fn add_border_states_batch_processes_all_entries_and_preserves_invariant() {
        let mut graph = make_empty_two_worker_graph();
        let entries = vec![
            AddBorderEntry {
                border_id: 100,
                side_a: p(0),
                side_b: p(1),
                worker_a: 0,
                worker_b: 1,
            },
            AddBorderEntry {
                border_id: 101,
                side_a: p(2),
                side_b: aux(3, 1),
                worker_a: 0,
                worker_b: 1,
            },
            AddBorderEntry {
                border_id: 102,
                side_a: p(4),
                side_b: p(5),
                worker_a: 0,
                worker_b: 1,
            },
        ];
        graph.add_border_states(entries);
        assert_eq!(graph.len(), 3);
        assert!(graph.borders.get(&100).unwrap().is_redex);
        assert!(!graph.borders.get(&101).unwrap().is_redex);
        assert!(graph.borders.get(&102).unwrap().is_redex);
        let from_borders: HashSet<u32> = graph
            .borders
            .iter()
            .filter(|(_, s)| s.is_redex)
            .map(|(bid, _)| *bid)
            .collect();
        let from_active: HashSet<u32> = graph.active_redexes.iter().copied().collect();
        assert_eq!(from_active, from_borders);
        assert_eq!(from_active, HashSet::from([100, 102]));
    }

    /// UT-0364-09: duplicate border_id ⇒ panic.
    #[test]
    #[should_panic(expected = "duplicate")]
    fn add_border_states_panics_on_duplicate_id() {
        let mut graph = make_graph_with_one_border(p(0), p(1));
        let entry = AddBorderEntry {
            border_id: 1,
            side_a: p(10),
            side_b: p(20),
            worker_a: 0,
            worker_b: 1,
        };
        graph.add_border_states(vec![entry]);
    }

    /// UT-0364-10: out-of-bounds worker ⇒ panic.
    #[test]
    #[should_panic(expected = "worker")]
    fn add_border_states_panics_on_out_of_bounds_worker() {
        let mut graph = make_empty_two_worker_graph();
        let entry = AddBorderEntry {
            border_id: 42,
            side_a: p(0),
            side_b: p(1),
            worker_a: 5,
            worker_b: 1,
        };
        graph.add_border_states(vec![entry]);
    }

    /// UT-0364-11: empty vec ⇒ no-op.
    #[test]
    fn add_border_states_empty_vec_is_noop() {
        let mut graph = make_graph_with_one_border(p(0), p(1));
        let snapshot = graph.clone();
        graph.add_border_states(vec![]);
        assert_eq!(graph.borders.len(), snapshot.borders.len());
        assert_eq!(graph.active_redexes.len(), snapshot.active_redexes.len());
        assert_eq!(
            graph.worker_borders[0].len(),
            snapshot.worker_borders[0].len()
        );
        assert_eq!(
            graph.worker_borders[1].len(),
            snapshot.worker_borders[1].len()
        );
    }

    /// UT-0364-12: DC-4 load-bearing — caller cannot poison `is_redex`
    /// because `AddBorderEntry` has no such field.
    #[test]
    fn add_border_states_enforces_is_redex_invariant() {
        let mut graph = make_empty_two_worker_graph();

        // Part 1: principal/principal ⇒ is_redex = true.
        graph.add_border_states(vec![AddBorderEntry {
            border_id: 1,
            side_a: p(7),
            side_b: p(8),
            worker_a: 0,
            worker_b: 1,
        }]);
        let s1 = graph.borders.get(&1).expect("border 1 present");
        assert!(s1.is_redex);
        assert!(graph.active_redexes.contains(&1));

        // Part 2: principal/aux ⇒ is_redex = false.
        graph.add_border_states(vec![AddBorderEntry {
            border_id: 2,
            side_a: p(9),
            side_b: aux(10, 1),
            worker_a: 0,
            worker_b: 1,
        }]);
        let s2 = graph.borders.get(&2).expect("border 2 present");
        assert!(!s2.is_redex);
        assert!(!graph.active_redexes.contains(&2));

        // Invariant cross-check.
        let from_borders: HashSet<u32> = graph
            .borders
            .iter()
            .filter(|(_, s)| s.is_redex)
            .map(|(bid, _)| *bid)
            .collect();
        let from_active: HashSet<u32> = graph.active_redexes.iter().copied().collect();
        assert_eq!(from_active, from_borders);
    }

    // -----------------------------------------------------------------
    // TEST-SPEC-0365 — module doc + R19 pure-core + Send/Sync.
    // -----------------------------------------------------------------

    /// UT-0365-01: source file honours R19 (no tokio / async_trait /
    /// crate::protocol imports).
    #[test]
    fn border_graph_source_respects_r19_pure_core_invariant() {
        let source: &str = include_str!("border_graph.rs");
        for (line_number, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("use ") {
                continue;
            }
            for forbidden in &["use tokio", "use async_trait", "use crate::protocol"] {
                assert!(
                    !trimmed.starts_with(forbidden),
                    "R19 pure-core invariant violated at line {}: `{}` \
                     (border_graph.rs MUST NOT import tokio / async_trait / crate::protocol)",
                    line_number + 1,
                    trimmed,
                );
            }
        }
    }

    /// UT-0365-02: module doc references coordinator lifecycle.
    #[test]
    fn border_graph_module_doc_references_coordinator_lifecycle() {
        let source: &str = include_str!("border_graph.rs");
        assert!(
            source.contains("Coordinator-side lifecycle"),
            "module doc MUST contain the heading `Coordinator-side lifecycle`"
        );
        assert!(
            source.contains("Pure-core invariant"),
            "module doc MUST contain the heading `Pure-core invariant`"
        );
        assert!(
            source.contains("Out of scope"),
            "module doc MUST contain the heading `Out of scope`"
        );
        for primitive in &[
            "detect_border_redexes",
            "apply_deltas",
            "remove_border",
            "add_border_states",
            "from_partition_plan",
        ] {
            assert!(
                source.contains(primitive),
                "module doc MUST reference primitive `{primitive}`"
            );
        }
        assert!(
            source.contains("AddBorderEntry"),
            "module doc MUST reference `AddBorderEntry` (DC-4 cascade — \
             the add_border_states input struct)"
        );
    }

    /// UT-0365-03: BorderGraph and its friends are Send + Sync.
    #[test]
    fn border_graph_and_friends_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<BorderGraph>();
        assert_send_sync::<BorderState>();
        assert_send_sync::<BorderDelta>();
        assert_send_sync::<AddBorderEntry>();
    }

    // -----------------------------------------------------------------
    // SPEC-19 §3.4 wire-type round-trip tests (item 2.26-A — DC-A1 + DC-B3
    // + DC-B5 amendments, 2026-04-17). Locks R33's bincode round-trip
    // identity at the struct level; variant-level round-trips live in
    // `protocol::types::tests`.
    // -----------------------------------------------------------------

    /// DC-A1: `BorderDelta` round-trips under bincode v2. This test was
    /// deferred from the §3.2 ship because the struct lacked serde
    /// derives at that time; the derives are added by the §3.4 bundle
    /// and this test pins the property.
    #[test]
    fn test_border_delta_bincode_roundtrip() {
        let delta = BorderDelta {
            border_id: 1234,
            new_target: PortRef::AgentPort(42, 1),
        };
        let bytes = crate::protocol::bincode_v2::encode(&delta)
            .expect("BorderDelta must encode under bincode v2");
        let decoded: BorderDelta = crate::protocol::bincode_v2::decode_value(&bytes)
            .expect("BorderDelta must decode under bincode v2");
        assert_eq!(decoded, delta);
    }

    /// DC-B3: `LocalReconnection` round-trips under bincode v2.
    #[test]
    fn test_local_reconnection_bincode_roundtrip() {
        let rec = LocalReconnection {
            agent_id: 7,
            port: 2,
            new_target: PortRef::AgentPort(11, 1),
        };
        let bytes = crate::protocol::bincode_v2::encode(&rec)
            .expect("LocalReconnection must encode under bincode v2");
        let decoded: LocalReconnection = crate::protocol::bincode_v2::decode_value(&bytes)
            .expect("LocalReconnection must decode under bincode v2");
        assert_eq!(decoded, rec);
    }

    /// DC-B5: `PendingCommutation` round-trips under bincode v2.
    #[test]
    fn test_pending_commutation_bincode_roundtrip() {
        let pc = PendingCommutation {
            request_id: 42,
            symbol_type: Symbol::Dup,
            arity: 2,
        };
        let bytes = crate::protocol::bincode_v2::encode(&pc)
            .expect("PendingCommutation must encode under bincode v2");
        let decoded: PendingCommutation = crate::protocol::bincode_v2::decode_value(&bytes)
            .expect("PendingCommutation must decode under bincode v2");
        assert_eq!(decoded, pc);
    }

    /// DC-B5: `MintedAgent` round-trips under bincode v2.
    #[test]
    fn test_minted_agent_bincode_roundtrip() {
        let ma = MintedAgent {
            request_id: 42,
            minted_agent_id: 103,
        };
        let bytes = crate::protocol::bincode_v2::encode(&ma)
            .expect("MintedAgent must encode under bincode v2");
        let decoded: MintedAgent = crate::protocol::bincode_v2::decode_value(&bytes)
            .expect("MintedAgent must decode under bincode v2");
        assert_eq!(decoded, ma);
    }

    // -----------------------------------------------------------------
    // Stage 5 QA — SPEC-19 §3.2 adversarial probes (qa agent, 2026-04-17).
    //
    // Probes from REVIEW-SPEC-19-section-3.2-2026-04-17.md §9 plus the
    // orchestrator's QA brief. Each probe targets a real-bug surface, not
    // a happy-path restatement. Probes are black-box tests on the public
    // primitives (plus `pub(crate)` field access for invariant cross-
    // checks). All expected to PASS on the current implementation; any
    // failure flags a Stage 6 REFACTOR item.
    // -----------------------------------------------------------------

    mod adversarial_probes {
        use super::*;

        /// Q1 — `border_id = u32::MAX` is a legal HashMap key (it is NOT the
        /// DISCONNECTED sentinel — DISCONNECTED is `PortRef::FreePort(u32::MAX)`,
        /// i.e. a PortRef variant, not a border_id). Confirm the whole
        /// lifecycle (init → apply_deltas → detect → remove) round-trips
        /// without any branch mistaking the key for the sentinel.
        ///
        /// This specifically rules out a defect class where some future
        /// refactor adds `if bid == u32::MAX { treat_as_disconnected }` at
        /// a key dispatch site.
        #[test]
        fn q1_border_id_umax_roundtrips_without_disconnected_confusion() {
            let plan = make_plan(
                vec![(0, vec![(u32::MAX, p(0))]), (1, vec![(u32::MAX, p(1))])],
                vec![u32::MAX],
            );
            let mut graph = BorderGraph::from_partition_plan(&plan);
            assert_eq!(graph.len(), 1);
            assert!(graph.borders.contains_key(&u32::MAX));
            assert!(graph.active_redexes.contains(&u32::MAX));
            assert_eq!(graph.active_redex_count(), 1);

            // detect returns it.
            let redexes = graph.detect_border_redexes();
            assert_eq!(redexes.len(), 1);
            assert_eq!(redexes[0].0, u32::MAX);

            // apply_delta on id == u32::MAX demotes to aux — normal update path.
            let state_before = graph.borders.get(&u32::MAX).unwrap().clone();
            let target_worker = state_before.worker_a;
            graph.apply_deltas(
                target_worker,
                &[BorderDelta {
                    border_id: u32::MAX,
                    new_target: aux(5, 1),
                }],
            );
            assert!(graph.borders.contains_key(&u32::MAX));
            assert!(!graph.borders.get(&u32::MAX).unwrap().is_redex);
            assert!(!graph.active_redexes.contains(&u32::MAX));

            // remove_border on u32::MAX works.
            let removed = graph.remove_border(u32::MAX);
            assert!(removed.is_some());
            assert!(!graph.borders.contains_key(&u32::MAX));
            assert_eq!(graph.len(), 0);
        }

        /// Q2 — `from_partition_plan` ordering determinism on `side_a/side_b`.
        ///
        /// The outer iteration is over a `HashMap` (`sightings`, L222) whose
        /// ordering is non-deterministic across runs. However, the
        /// `(wa, pa)` vs `(wb, pb)` assignment depends on the Vec inside
        /// each sighting entry, whose insertion order follows the stable
        /// outer `plan.partitions` iteration (L180-184). So for a given
        /// `plan` the *unordered pair* `{side_a, side_b}` is deterministic,
        /// and furthermore the side assignment itself IS deterministic
        /// because the Vec append order matches `partitions[0]` before
        /// `partitions[1]`.
        ///
        /// This probe pins that property: 100 reconstructions on identical
        /// input yield byte-identical `(side_a, worker_a, side_b, worker_b)`
        /// tuples for every border. A future refactor that shuffles
        /// `sightings` insertion (e.g., iterating `free_port_index`, itself
        /// a HashMap) would break this — this test catches that regression.
        #[test]
        fn q2_from_partition_plan_side_assignment_is_deterministic_across_runs() {
            // 3 partitions, 3 borders, mixed redex/non-redex to exercise
            // all three paths.
            let build = || {
                make_plan(
                    vec![
                        (0, vec![(10, p(0)), (20, aux(1, 1))]),
                        (1, vec![(10, p(5)), (30, p(6))]),
                        (2, vec![(20, p(11)), (30, aux(12, 2))]),
                    ],
                    vec![10, 20, 30],
                )
            };
            let plan0 = build();
            let graph0 = BorderGraph::from_partition_plan(&plan0);
            let snap = |g: &BorderGraph, bid: u32| {
                let s = g.borders.get(&bid).expect("border present");
                (s.side_a, s.worker_a, s.side_b, s.worker_b, s.is_redex)
            };
            let ref_10 = snap(&graph0, 10);
            let ref_20 = snap(&graph0, 20);
            let ref_30 = snap(&graph0, 30);
            for _ in 0..100 {
                let plan = build();
                let g = BorderGraph::from_partition_plan(&plan);
                assert_eq!(snap(&g, 10), ref_10, "border 10 drift across runs");
                assert_eq!(snap(&g, 20), ref_20, "border 20 drift across runs");
                assert_eq!(snap(&g, 30), ref_30, "border 30 drift across runs");
            }
        }

        /// Q3 — C3 panic message under 4+ sightings names the TRUE count,
        /// not a hard-coded "3". Extends UT-0361-07 which only covered the
        /// 3-sighting case.
        ///
        /// The panic path (L195-198) formats `{count}` from the Vec length;
        /// this probe pins that contract by asserting the message contains
        /// the string "4". A bug that hard-coded "3" (from the original
        /// duplicate-sighting case) would fail this probe.
        #[test]
        #[should_panic(expected = "4")]
        fn q3_c3_panic_names_true_sighting_count_not_hardcoded_three() {
            let plan = make_plan(
                vec![
                    (0, vec![(77, p(0))]),
                    (1, vec![(77, p(1))]),
                    (2, vec![(77, p(2))]),
                    (3, vec![(77, p(3))]),
                ],
                vec![77],
            );
            let _ = BorderGraph::from_partition_plan(&plan);
        }

        /// Q4 — DC-1 sentinel discipline. DISCONNECTED is specifically the
        /// `PortRef::FreePort(u32::MAX)` value; nothing else should be
        /// conflated with it.
        ///
        /// Part A: a FreePort whose border id happens to be `u32::MAX - 1`
        /// must NOT be treated as DISCONNECTED. Because `FreePort(_)` is
        /// never a principal port, a FreePort/FreePort pair is never a
        /// redex — but the border stays ALIVE (single-side delta does not
        /// trigger R17 double-erasure).
        ///
        /// Part B: a real DISCONNECTED sentinel (`FreePort(u32::MAX)`)
        /// coming as `new_target` from the worker side IS treated as
        /// DISCONNECTED and correctly clears is_redex on the affected side.
        ///
        /// Part C: verifies R17 double-disconnect path requires BOTH sides
        /// at exactly `FreePort(u32::MAX)`; a pair of
        /// `(FreePort(u32::MAX - 1), FreePort(u32::MAX - 1))` does NOT
        /// trigger removal.
        #[test]
        fn q4_disconnected_sentinel_discipline_umax_only() {
            // Part A: FreePort(u32::MAX - 1) is not DISCONNECTED; border alive.
            let mut graph = make_graph_with_one_border(p(0), p(1));
            let state_before = graph.borders.get(&1).unwrap().clone();
            let target_worker = state_before.worker_a;
            graph.apply_deltas(
                target_worker,
                &[BorderDelta {
                    border_id: 1,
                    new_target: PortRef::FreePort(u32::MAX - 1),
                }],
            );
            assert!(
                graph.borders.contains_key(&1),
                "FreePort(u32::MAX - 1) MUST NOT trigger disconnect path"
            );
            assert!(
                !graph.borders.get(&1).unwrap().is_redex,
                "FreePort endpoint cannot form a principal pair"
            );
            assert!(!graph.active_redexes.contains(&1));

            // Part C: a second delta on the OTHER side with FreePort(u32::MAX - 1)
            //         also MUST NOT trigger R17 removal — only genuine
            //         FreePort(u32::MAX) pairs do.
            let other_worker = state_before.worker_b;
            graph.apply_deltas(
                other_worker,
                &[BorderDelta {
                    border_id: 1,
                    new_target: PortRef::FreePort(u32::MAX - 1),
                }],
            );
            assert!(
                graph.borders.contains_key(&1),
                "two FreePort(u32::MAX - 1) sides MUST NOT trigger R17 removal"
            );

            // Part B: real DISCONNECTED clears is_redex on that side, and
            //         a followup DISCONNECTED on the other side DOES remove.
            let mut graph2 = make_graph_with_one_border(p(0), p(1));
            let st = graph2.borders.get(&1).unwrap().clone();
            graph2.apply_deltas(
                st.worker_a,
                &[BorderDelta {
                    border_id: 1,
                    new_target: DISCONNECTED,
                }],
            );
            assert!(graph2.borders.contains_key(&1));
            assert_eq!(
                graph2.borders.get(&1).unwrap().side_a,
                DISCONNECTED,
                "real DISCONNECTED must land on side_a"
            );
            assert!(!graph2.borders.get(&1).unwrap().is_redex);
            graph2.apply_deltas(
                st.worker_b,
                &[BorderDelta {
                    border_id: 1,
                    new_target: DISCONNECTED,
                }],
            );
            assert!(
                !graph2.borders.contains_key(&1),
                "two real DISCONNECTEDs trigger R17 removal"
            );
        }

        /// Q5 — `apply_deltas` silent-skip for a worker_id that owns NO
        /// borders at all. The graph already covers the "owns other
        /// borders but not this one" case (UT-0362-03); this probe pins
        /// the "never-seen worker_id" extreme — including one larger than
        /// `worker_borders.len()` — as a no-op, NOT a panic or bounds
        /// fault.
        ///
        /// `apply_deltas` must not index `worker_borders` directly — it
        /// only compares `worker_id` to `state.worker_a`/`worker_b`
        /// equality. A `WorkerId` of `u32::MAX` must therefore be
        /// harmless.
        #[test]
        fn q5_apply_deltas_unknown_worker_id_is_silent_noop() {
            let mut graph = make_graph_with_one_border(p(0), p(1));
            let snapshot = graph.clone();

            // Unknown worker larger than `worker_borders.len()` — must not panic.
            graph.apply_deltas(
                u32::MAX,
                &[BorderDelta {
                    border_id: 1,
                    new_target: aux(42, 1),
                }],
            );
            assert_eq!(
                graph.borders.get(&1),
                snapshot.borders.get(&1),
                "unknown worker_id must not mutate the border"
            );
            assert_eq!(graph.active_redexes, snapshot.active_redexes);

            // Worker id that is in-range but owns no borders in THIS graph
            // (the 2-worker fixture covers 0 + 1; check worker 7 as owner-of-nothing).
            graph.apply_deltas(
                7,
                &[
                    BorderDelta {
                        border_id: 1,
                        new_target: aux(50, 1),
                    },
                    BorderDelta {
                        border_id: 999,
                        new_target: p(60),
                    },
                ],
            );
            assert_eq!(graph.borders.get(&1), snapshot.borders.get(&1));
            assert_eq!(graph.active_redexes, snapshot.active_redexes);
        }

        /// Q6 — Idempotence of same-delta double apply: `active_redexes`
        /// is a HashSet, so double-insert is trivially a no-op, but we
        /// pin the cross-sectional invariant `{bid : borders[bid].is_redex}
        /// == active_redexes` across both apply calls. Guards against a
        /// future refactor that migrates `active_redexes` to a Vec or
        /// Multiset (would break idempotence silently).
        #[test]
        fn q6_apply_deltas_is_idempotent_on_redex_membership() {
            let mut graph = make_graph_with_one_border(p(0), aux(1, 1));
            // Promote to redex
            let st = graph.borders.get(&1).unwrap().clone();
            let aux_worker = if st.side_a == aux(1, 1) {
                st.worker_a
            } else {
                st.worker_b
            };
            let promote = BorderDelta {
                border_id: 1,
                new_target: p(9),
            };
            graph.apply_deltas(aux_worker, &[promote]);
            assert!(graph.active_redexes.contains(&1));
            assert_eq!(graph.active_redex_count(), 1);

            // Apply the exact same delta again — must be a no-op on membership
            // and invariant.
            graph.apply_deltas(aux_worker, &[promote]);
            assert_eq!(graph.active_redex_count(), 1);
            assert!(graph.borders.get(&1).unwrap().is_redex);

            // Cross-sectional invariant.
            let from_borders: HashSet<u32> = graph
                .borders
                .iter()
                .filter(|(_, s)| s.is_redex)
                .map(|(bid, _)| *bid)
                .collect();
            let from_active: HashSet<u32> = graph.active_redexes.iter().copied().collect();
            assert_eq!(from_active, from_borders);
        }

        /// Q7 — DC-4 end-to-end: mixed `AddBorderEntry` batch — half
        /// principal/principal (redex), half principal/aux (non-redex) —
        /// all states land, `active_redexes` contains exactly the redex
        /// subset, `worker_borders` is pushed for both sides of EVERY
        /// entry. This pins DC-4 Option B beyond UT-0364-12 (which only
        /// checks 2 insertions in sequence).
        #[test]
        fn q7_add_border_states_mixed_batch_is_redex_decided_per_entry() {
            let mut graph = make_empty_two_worker_graph();
            let entries = vec![
                AddBorderEntry {
                    border_id: 1,
                    side_a: p(10),
                    side_b: p(11),
                    worker_a: 0,
                    worker_b: 1,
                }, // redex
                AddBorderEntry {
                    border_id: 2,
                    side_a: p(20),
                    side_b: aux(21, 1),
                    worker_a: 0,
                    worker_b: 1,
                }, // not redex
                AddBorderEntry {
                    border_id: 3,
                    side_a: aux(30, 2),
                    side_b: p(31),
                    worker_a: 0,
                    worker_b: 1,
                }, // not redex
                AddBorderEntry {
                    border_id: 4,
                    side_a: p(40),
                    side_b: p(41),
                    worker_a: 0,
                    worker_b: 1,
                }, // redex
                AddBorderEntry {
                    border_id: 5,
                    side_a: aux(50, 1),
                    side_b: aux(51, 2),
                    worker_a: 0,
                    worker_b: 1,
                }, // not redex
                AddBorderEntry {
                    border_id: 6,
                    side_a: p(60),
                    side_b: p(61),
                    worker_a: 0,
                    worker_b: 1,
                }, // redex
            ];
            graph.add_border_states(entries);
            assert_eq!(graph.len(), 6);

            // Per-entry is_redex correctness.
            for bid in [1, 4, 6] {
                assert!(
                    graph.borders.get(&bid).unwrap().is_redex,
                    "border {bid} principal/principal MUST be redex"
                );
                assert!(graph.active_redexes.contains(&bid));
            }
            for bid in [2, 3, 5] {
                assert!(
                    !graph.borders.get(&bid).unwrap().is_redex,
                    "border {bid} non-principal pair MUST NOT be redex"
                );
                assert!(!graph.active_redexes.contains(&bid));
            }

            // Cross-sectional invariant.
            let from_borders: HashSet<u32> = graph
                .borders
                .iter()
                .filter(|(_, s)| s.is_redex)
                .map(|(bid, _)| *bid)
                .collect();
            let from_active: HashSet<u32> = graph.active_redexes.iter().copied().collect();
            assert_eq!(from_active, from_borders);
            assert_eq!(from_active, HashSet::from([1, 4, 6]));

            // worker_borders updated for every entry on both sides.
            let w0: HashSet<u32> = graph.worker_borders[0].iter().copied().collect();
            let w1: HashSet<u32> = graph.worker_borders[1].iter().copied().collect();
            assert_eq!(w0, HashSet::from([1, 2, 3, 4, 5, 6]));
            assert_eq!(w1, HashSet::from([1, 2, 3, 4, 5, 6]));
        }

        /// Q8 — `remove_border` of a never-present id is a no-op (returns
        /// None, does not panic, leaves `active_redexes` untouched).
        /// Extends UT-0364-02 by verifying the full invariant survives.
        #[test]
        fn q8_remove_border_absent_id_is_noop_returning_none() {
            let mut graph = make_graph_with_one_border(p(0), p(1));
            let snapshot = graph.clone();

            let absent1 = graph.remove_border(9999);
            assert!(absent1.is_none());
            let absent2 = graph.remove_border(u32::MAX);
            assert!(absent2.is_none());
            let absent3 = graph.remove_border(0); // 0 is also a valid-never-seen id
            assert!(absent3.is_none());

            // State entirely preserved.
            assert_eq!(graph.borders.len(), snapshot.borders.len());
            assert_eq!(graph.borders.get(&1), snapshot.borders.get(&1));
            assert_eq!(graph.active_redexes, snapshot.active_redexes);
            assert_eq!(graph.worker_borders, snapshot.worker_borders);
        }

        /// Q9 — Large-graph stress: 10k borders, repeatedly flip each
        /// border between redex and non-redex. `active_redexes.len()` must
        /// converge deterministically to whichever end-state the deltas
        /// leave behind. Checks that the incremental invariant maintenance
        /// in `apply_deltas` doesn't accumulate stale entries at scale.
        #[test]
        fn q9_stress_10k_borders_flip_convergence() {
            const N: u32 = 10_000;
            let mut p0 = Vec::with_capacity(N as usize);
            let mut p1 = Vec::with_capacity(N as usize);
            let mut decls = Vec::with_capacity(N as usize);
            // Initial state: all principal/principal (all redex).
            for bid in 0..N {
                p0.push((bid, p(bid * 2)));
                p1.push((bid, p(bid * 2 + 1)));
                decls.push(bid);
            }
            let plan = make_plan(vec![(0, p0), (1, p1)], decls);
            let mut graph = BorderGraph::from_partition_plan(&plan);
            assert_eq!(graph.active_redex_count(), N as usize);

            // Demote every border on worker 0's side to aux → active
            // drops to 0.
            let demote: Vec<BorderDelta> = (0..N)
                .map(|bid| BorderDelta {
                    border_id: bid,
                    new_target: aux(bid, 1),
                })
                .collect();
            let worker0 = graph.borders.get(&0).unwrap().worker_a;
            graph.apply_deltas(worker0, &demote);
            assert_eq!(
                graph.active_redex_count(),
                0,
                "all borders demoted MUST leave active_redexes empty"
            );

            // Re-promote — active count must bounce back to N exactly.
            let promote: Vec<BorderDelta> = (0..N)
                .map(|bid| BorderDelta {
                    border_id: bid,
                    new_target: p(bid * 2),
                })
                .collect();
            graph.apply_deltas(worker0, &promote);
            assert_eq!(
                graph.active_redex_count(),
                N as usize,
                "all borders re-promoted MUST restore active_redexes size"
            );

            // One more demote-promote cycle — no drift, no accumulation.
            graph.apply_deltas(worker0, &demote);
            graph.apply_deltas(worker0, &promote);
            assert_eq!(graph.active_redex_count(), N as usize);

            // Cross-sectional invariant at scale.
            let from_borders = graph.borders.iter().filter(|(_, s)| s.is_redex).count();
            assert_eq!(from_borders, graph.active_redex_count());
        }

        /// Q10 — Degenerate same-worker-on-both-sides border.
        ///
        /// The current implementation does not forbid `worker_a ==
        /// worker_b`. A delta from that worker ambiguously updates
        /// side_a (the `updates_a` branch wins on tie, L283). This
        /// probe pins that behavior so a future refactor doesn't
        /// silently change it. Whether the semantics are *desirable*
        /// is an OQ (see REVIEW §9 item 9). Today's contract: side_a
        /// wins when `worker_a == worker_b`.
        ///
        /// Fixture note: the graph indexes `worker_borders[wid]`, so
        /// `wid` must be `< plan.partitions.len()`. We use two
        /// partitions with the SAME `worker_id = 0` to get a genuine
        /// `worker_a == worker_b == 0` border while staying in bounds.
        #[test]
        fn q10_same_worker_both_sides_side_a_wins_on_tie() {
            // Two partitions, both owned by worker 0 (so `worker_borders`
            // has len 2 and index 0 is valid).
            let plan = make_plan(vec![(0, vec![(1, p(0))]), (0, vec![(1, p(1))])], vec![1]);
            let mut graph = BorderGraph::from_partition_plan(&plan);
            let state = graph.borders.get(&1).expect("border present").clone();
            assert_eq!(
                state.worker_a, state.worker_b,
                "fixture: same worker both sides"
            );
            // Apply a delta as worker 0.
            let before_side_a = state.side_a;
            let before_side_b = state.side_b;
            graph.apply_deltas(
                0,
                &[BorderDelta {
                    border_id: 1,
                    new_target: aux(99, 2),
                }],
            );
            let after = graph.borders.get(&1).unwrap();
            // Current contract: `updates_a` wins on tie.
            assert_eq!(
                after.side_a,
                aux(99, 2),
                "side_a MUST take the delta on worker_a == worker_b tie"
            );
            assert_ne!(after.side_a, before_side_a);
            // side_b untouched.
            assert_eq!(
                after.side_b, before_side_b,
                "side_b MUST be untouched on same-worker tie"
            );
        }

        /// Q11 — After `remove_border` resolves a redex, the border
        /// disappears from BOTH `borders` AND `active_redexes`, and
        /// `detect_border_redexes` no longer surfaces it.
        ///
        /// Extends UT-0364-03 by pinning the post-removal `detect()`
        /// contract (that function's defensive `filter_map` would
        /// *mask* a bug where the remove forgot to scrub
        /// `active_redexes` — this probe prevents that by checking
        /// both containers directly).
        #[test]
        fn q11_remove_border_on_redex_fully_scrubs_active_and_detect() {
            let mut graph = make_graph_with_three_borders();
            assert!(graph.active_redexes.contains(&20));
            assert!(graph.active_redexes.contains(&30));
            assert_eq!(graph.active_redex_count(), 2);

            // Remove one of the redex borders.
            let removed = graph.remove_border(20);
            assert!(removed.is_some());
            assert!(
                !graph.borders.contains_key(&20),
                "remove must drop from borders"
            );
            assert!(
                !graph.active_redexes.contains(&20),
                "remove must scrub active_redexes for a redex entry"
            );
            assert_eq!(graph.active_redex_count(), 1);

            // detect() no longer surfaces border 20.
            let detected: HashSet<u32> = graph
                .detect_border_redexes()
                .iter()
                .map(|(b, _)| *b)
                .collect();
            assert_eq!(detected, HashSet::from([30]));
        }

        /// Q12 — `worker_borders` consistency under `add_border_states`
        /// followed by `apply_deltas`. Pins DC-2 (`worker_borders` is
        /// populated for every border the graph knows about). The field
        /// is `#[allow(dead_code)]` today because item 2.26 is the first
        /// consumer — but if this probe fails, item 2.26 inherits a
        /// landmine (a border with no entry in `worker_borders`).
        ///
        /// Also verifies that `apply_deltas` does NOT touch
        /// `worker_borders` (it's a mutation-free field from
        /// `apply_deltas`' perspective). Otherwise, a redex-toggle
        /// delta could silently churn the reverse index.
        #[test]
        fn q12_worker_borders_consistency_after_add_and_apply() {
            let mut graph = make_empty_two_worker_graph();
            let entry = AddBorderEntry {
                border_id: 77,
                side_a: p(0),
                side_b: aux(1, 1),
                worker_a: 0,
                worker_b: 1,
            };
            graph.add_border_states(vec![entry]);
            // Both workers' reverse index must contain the new border.
            assert!(
                graph.worker_borders[0].contains(&77),
                "worker_borders[0] MUST contain 77 after add_border_states"
            );
            assert!(
                graph.worker_borders[1].contains(&77),
                "worker_borders[1] MUST contain 77 after add_border_states"
            );
            let w0_len_before = graph.worker_borders[0].len();
            let w1_len_before = graph.worker_borders[1].len();

            // apply_deltas must not mutate worker_borders.
            graph.apply_deltas(
                0,
                &[BorderDelta {
                    border_id: 77,
                    new_target: p(9),
                }],
            );
            assert_eq!(
                graph.worker_borders[0].len(),
                w0_len_before,
                "apply_deltas MUST NOT mutate worker_borders[worker_a]"
            );
            assert_eq!(
                graph.worker_borders[1].len(),
                w1_len_before,
                "apply_deltas MUST NOT mutate worker_borders[worker_b]"
            );
            // The entries are still there.
            assert!(graph.worker_borders[0].contains(&77));
            assert!(graph.worker_borders[1].contains(&77));
        }

        /// Q13 — Mid-batch panic in `add_border_states` leaves the
        /// already-inserted prefix consistent (REVIEW §9 item 6).
        ///
        /// Concretely: `add_border_states` has no transaction semantics.
        /// If entry index 2 panics on duplicate, entries 0 and 1 are
        /// already in the graph. This probe pins: (a) the panic is
        /// catchable via `std::panic::catch_unwind`, (b) after the
        /// panic the invariant `{bid : borders[bid].is_redex} ==
        /// active_redexes` STILL holds on the partial batch, (c) the
        /// successfully-inserted entries ARE still present (no roll
        /// back, by design).
        ///
        /// This is the "partial-failure correctness" gate. A future
        /// refactor that introduces buffering or rollback MUST NOT
        /// regress this contract silently.
        #[test]
        fn q13_add_border_states_mid_batch_panic_leaves_prefix_consistent() {
            use std::panic;
            use std::panic::AssertUnwindSafe;

            let mut graph = make_graph_with_one_border(p(0), p(1));
            // `border_id = 1` is present from the fixture. A batch that
            // inserts 100, then 101, then a duplicate (1) MUST panic after
            // 100 and 101 are already inserted.
            let batch = vec![
                AddBorderEntry {
                    border_id: 100,
                    side_a: p(10),
                    side_b: p(11),
                    worker_a: 0,
                    worker_b: 1,
                },
                AddBorderEntry {
                    border_id: 101,
                    side_a: p(20),
                    side_b: aux(21, 1),
                    worker_a: 0,
                    worker_b: 1,
                },
                AddBorderEntry {
                    border_id: 1, // duplicate — panics
                    side_a: p(30),
                    side_b: p(31),
                    worker_a: 0,
                    worker_b: 1,
                },
                AddBorderEntry {
                    border_id: 200, // never reached
                    side_a: p(40),
                    side_b: p(41),
                    worker_a: 0,
                    worker_b: 1,
                },
            ];

            let result = panic::catch_unwind(AssertUnwindSafe(|| {
                graph.add_border_states(batch);
            }));
            assert!(result.is_err(), "duplicate id MUST panic mid-batch");

            // Successfully-inserted prefix survives.
            assert!(
                graph.borders.contains_key(&100),
                "prefix entry 100 must survive"
            );
            assert!(
                graph.borders.contains_key(&101),
                "prefix entry 101 must survive"
            );
            // Never-reached entries absent.
            assert!(!graph.borders.contains_key(&200));
            // Original fixture intact.
            assert!(graph.borders.contains_key(&1));

            // Cross-sectional invariant survives the partial batch.
            let from_borders: HashSet<u32> = graph
                .borders
                .iter()
                .filter(|(_, s)| s.is_redex)
                .map(|(bid, _)| *bid)
                .collect();
            let from_active: HashSet<u32> = graph.active_redexes.iter().copied().collect();
            assert_eq!(
                from_active, from_borders,
                "mid-batch panic MUST NOT desync active_redexes from is_redex bits"
            );
            // Specific expected memberships: 100 and 1 are redex; 101 is not.
            assert!(from_active.contains(&1));
            assert!(from_active.contains(&100));
            assert!(!from_active.contains(&101));
        }
    }
}
