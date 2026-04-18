//! Coordinator-side border-redex resolution (SPEC-19 §3.2 R13-R15 parts 1-2;
//! SPEC-19 §3.3, item 2.26).
//!
//! When `BorderGraph::detect_border_redexes` yields an active redex
//! (both endpoints on principal ports), the coordinator materializes
//! the two agents from the involved workers' cached partitions, runs
//! the appropriate `interact_*` rule (SPEC-03), and packages the
//! resulting port reconnections as `BorderDelta`s. R14 requires the
//! resolver to mirror the 6 IC rules without calling the mutating
//! `interact_*` functions directly — this module operates on read-only
//! partition views plus a mutable `BorderGraph`.
//!
//! **pure-core invariant (R19 inherited from `merge/`):** NO `tokio`,
//! NO `async_trait`, NO `crate::protocol::*` imports. The async
//! wire-layer caller lives under `coordinator.rs` (item 2.26-C) and
//! calls INTO this module — not the other way around.
//!
//! **Invariants preserved by the resolver:**
//! - **T5 (interaction count budget):** every border-redex resolution
//!   corresponds to exactly one IC rule firing, counted against the
//!   global interaction budget.
//! - **SPEC-03 6-rule closure:** CON-CON, DUP-DUP, ERA-ERA, CON-DUP,
//!   CON-ERA, DUP-ERA are the only dispatch targets — no new rules.
//! - **Border-graph consistency (R9 derived bit):** after each
//!   resolution, `BorderGraph::remove_border` / `add_border_states`
//!   / `apply_deltas` keeps the graph in sync with the workers'
//!   cached partitions.
//!
//! **Design-choice rulings (spec-critic verdict 2026-04-17, file
//! `docs/spec-reviews/SPEC-19-section-3.3-2.26B-design-choices-2026-04-17.md`):**
//! - **DC-B1** — coordinator caches `&[Partition]` (Option A). The
//!   resolver is a pure function of the cache state at round
//!   boundaries; cache maintenance (applying `BorderDelta`s,
//!   registering `MintedAgent`s from `RoundResult`, and applying
//!   `LocalReconnection`s to each worker's cached partition) is
//!   2.26-C territory.
//! - **DC-B2** — `materialize_agent` keeps `Option` return; the
//!   caller-side panic via `assert_agent` lives in TASK-0373. The
//!   helper's defensive `Option` never panics.
//! - **DC-B4** — border-adjacent agents are pinned in the worker's
//!   local `reduce_all` for the round in which they are
//!   border-principal (new R40c in §3.5, owned by 2.26-D). No race
//!   window at this call site; `None` from `materialize_agent` is a
//!   cache-maintenance bug (DC-B1 surface), not a race artefact.
//! - **DC-B5** — CON-DUP commutation uses 2-phase AgentId allocation.
//!   The resolver emits `pending_commutations: Vec<CommutationBatch>`
//!   (one batch per involved worker — balanced as 1 Dup + 1 Con each)
//!   plus `pending_new_borders: Vec<PendingNewBorder>` carrying
//!   `PendingPortRef::Pending` tokens for cross-partition wires whose
//!   concrete `AgentId` endpoints only materialize in round N+2 after
//!   `MintedAgent` echoes arrive. `new_borders: Vec<AddBorderEntry>`
//!   stays EMPTY at resolver time; `graph.add_border_states` is NEVER
//!   called from the resolver — finalization is owned by 2.26-C.
//!   Intra-batch sibling wires are encoded with R48 slot markers
//!   (`AgentPort(SLOT_MARKER_BASE + slot, port)`) inside
//!   `CommutationBatch.local_wiring`.
//! - **DC-B6** — CON-ERA / DUP-ERA preserve auxiliary-port borders.
//!   When a consumed non-ERA agent had an auxiliary port connected
//!   to an existing `FreePort(bid)` tracked in `graph.borders`, the
//!   resolver reuses `bid` in a `PendingNewBorder` (rather than
//!   allocating a fresh id). Round-N+2 finalization applies the
//!   resulting endpoint change via `graph.apply_deltas` — NOT
//!   `add_border_states` — so wire continuity (SPEC-03 R14) is
//!   preserved through the reduction. Fresh `BorderIdAllocator` ids
//!   are used ONLY for CON-DUP cross wires.

// SPEC-19 §3.3 item 2.26-B — the resolver's public (pub(crate)) surface
// is consumed by `package_resolutions` (TASK-0375) and the async
// coordinator path (item 2.26-C). Until those ship, every name here
// looks unused to `-D dead-code`. We tag each item with an
// `#[allow(dead_code)]` + a line pointing to the follow-up task, the
// same precedent used for `BorderGraph::worker_borders` in
// `border_graph.rs` (line ~111, R23 reverse index).
use crate::merge::border_graph::{AddBorderEntry, BorderDelta, BorderGraph, BorderState};
use crate::net::{AgentId, PortRef, Symbol};
use crate::partition::types::{Partition, WorkerId};

/// Per-worker output of a single border-redex resolution (DC-B3 split).
///
/// The resolver splits a worker's deltas into two buckets:
///
/// - `border_deltas` — updates to existing borders (wire type
///   `BorderDelta`, compact and frozen per §3.2 R33 DC-1).
/// - `local_reconnections` — raw `(PortRef, PortRef)` pairs describing
///   connections between auxiliary-port targets that emerged from the
///   resolved redex. The pairs live at the resolver's abstraction level;
///   `package_resolutions` (TASK-0375) translates each pair into the
///   wire-level `LocalReconnection { agent_id, port, new_target }` or
///   into `AddBorderEntry`s when the pair spans two partitions.
#[derive(Debug, Clone, Default)]
// Consumed by `package_resolutions` (TASK-0375).
#[allow(dead_code)]
pub(crate) struct WorkerDeltas {
    pub(crate) border_deltas: Vec<BorderDelta>,
    pub(crate) local_reconnections: Vec<(PortRef, PortRef)>,
}

/// Coordinator's resolution output for ONE border redex (DC-B3 + DC-B7).
///
/// TASK-0375's `package_resolutions` folds a stream of
/// `BorderResolution`s into per-worker `RoundStartDispatch` structs
/// that the async caller ships as `Message::RoundStart` frames.
#[derive(Debug, Clone, Default)]
// Consumed by `package_resolutions` (TASK-0375) + async coordinator
// (item 2.26-C).
#[allow(dead_code)]
pub(crate) struct BorderResolution {
    /// Deltas keyed by worker. Each entry is unique per worker id.
    pub(crate) worker_deltas: Vec<(WorkerId, WorkerDeltas)>,
    /// Resolved borders as `(border_id, worker_a, worker_b)` triples
    /// (DC-B7). `package_resolutions` fans each triple into both
    /// workers' per-worker `resolved_borders` buckets so each worker
    /// clears its `free_port_index` entry for `border_id`.
    pub(crate) resolved_borders: Vec<(u32, WorkerId, WorkerId)>,
    /// Concrete new borders ready for `graph.add_border_states`.
    /// EMPTY for CON-DUP under DC-B5 (see `pending_new_borders`
    /// instead — CON-DUP defers concrete AgentId substitution to
    /// round N+2). Empty for the three same-symbol rules.
    pub(crate) new_borders: Vec<AddBorderEntry>,
    /// Per-worker agent-mint requests for CON-DUP commutation
    /// expansions and CON-ERA / DUP-ERA erasures (DC-B5 2-phase). The
    /// `package_resolutions` pass (TASK-0375) fans each batch into the
    /// wire-layer `crate::merge::PendingCommutation` (one entry per
    /// requested agent) that ships on `Message::RoundStart`.
    pub(crate) pending_commutations: Vec<CommutationBatch>,
    /// Pending new cross-partition borders whose endpoints cannot yet
    /// be expressed as concrete `PortRef`s because they reference
    /// newly-minted agents whose `AgentId`s ship in `MintedAgent`
    /// round N+1. `package_resolutions` keeps them opaque; 2.26-C's
    /// coordinator loop substitutes concrete ports after
    /// `minted_agents` arrives, then calls `add_border_states`.
    pub(crate) pending_new_borders: Vec<PendingNewBorder>,
}

/// Per-worker payload for the `RoundStart` wire message (SPEC-19
/// §3.2 R15 part 1 + §3.3 R23). Pure-core mirror of the wire variant
/// `Message::RoundStart` (2.26-A `protocol/messages.rs`); the
/// coordinator's async dispatch path (2.26-C) converts this struct
/// into the wire frame at send time. `package_resolutions` folds a
/// stream of `BorderResolution`s (one per border redex detected this
/// round) into one `RoundStartDispatch` per worker.
///
/// DC-B3 split: `border_deltas` (port-update to existing borders) and
/// `local_reconnections` (intra-partition wire fix-ups) travel as
/// sibling fields — the wire-layer `WorkerDeltas` type they
/// ultimately become is carried through unchanged.
///
/// DC-B5 deferral: `new_borders` carries only CONCRETE new borders
/// ready for the worker's `FreePort` table. CON-DUP cross wires whose
/// endpoints reference not-yet-minted agents stay at coordinator
/// state in `BorderResolution.pending_new_borders` (a different
/// field) until round N+2 finalization. `pending_commutations` fans
/// out per worker via each `CommutationBatch.worker` field.
///
/// DC-B7 triple fan: `resolved_borders: Vec<(u32, WorkerId,
/// WorkerId)>` triples in the input fan into `Vec<u32>` entries on
/// BOTH sides' dispatches so each worker can clear its `FreePort`
/// entry for the consumed border.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
// Consumed by the async coordinator dispatch path (item 2.26-C).
#[allow(dead_code)]
pub(crate) struct RoundStartDispatch {
    pub(crate) border_deltas: Vec<BorderDelta>,
    pub(crate) local_reconnections: Vec<(PortRef, PortRef)>,
    pub(crate) resolved_borders: Vec<u32>,
    pub(crate) new_borders: Vec<(u32, PortRef)>,
    pub(crate) pending_commutations: Vec<CommutationBatch>,
}

/// Groups a batch of `BorderResolution`s into per-worker
/// `RoundStartDispatch` payloads (SPEC-19 §3.2 R15 part 1, §3.3 R23).
///
/// Invariants:
/// - Output length is exactly `num_workers` — R23 mandates the
///   coordinator sends `RoundStart` to EVERY worker every round, even
///   when the payload is empty (workers advance their round counter
///   off the heartbeat).
/// - Output is ordered by `WorkerId` ascending (`0..num_workers`) for
///   deterministic BSP replay.
/// - `resolved_borders` triples `(bid, wa, wb)` fan into BOTH
///   `per_worker[wa]` AND `per_worker[wb]` (unless `wa == wb`, in
///   which case `bid` is pushed once only — avoids the self-border
///   double-clear regression called out in QA-0375-G).
/// - `new_borders: Vec<AddBorderEntry>` fans similarly, carrying
///   `(border_id, side_a)` to `worker_a` and `(border_id, side_b)` to
///   `worker_b` (with the same self-border guard).
/// - `pending_commutations` fans to exactly one worker per batch (the
///   `CommutationBatch.worker` field), never duplicated.
/// - `pending_new_borders` is INTENTIONALLY dropped — those entries
///   stay at coordinator state until round N+2 finalization (DC-B5).
///
/// The input's iteration order inside each resolution is preserved
/// in the output; the resolution-level order (outer `Vec`) determines
/// per-worker insertion order. No sorting, no deduplication.
#[allow(dead_code)]
pub(crate) fn package_resolutions(
    resolutions: Vec<BorderResolution>,
    num_workers: usize,
) -> Vec<(WorkerId, RoundStartDispatch)> {
    let mut per_worker: Vec<RoundStartDispatch> = (0..num_workers)
        .map(|_| RoundStartDispatch::default())
        .collect();

    for resolution in resolutions {
        for (wid, wd) in resolution.worker_deltas {
            let dispatch = &mut per_worker[wid as usize];
            dispatch.border_deltas.extend(wd.border_deltas);
            dispatch.local_reconnections.extend(wd.local_reconnections);
        }

        for (bid, wa, wb) in resolution.resolved_borders {
            per_worker[wa as usize].resolved_borders.push(bid);
            if wa != wb {
                per_worker[wb as usize].resolved_borders.push(bid);
            }
        }

        for entry in resolution.new_borders {
            per_worker[entry.worker_a as usize]
                .new_borders
                .push((entry.border_id, entry.side_a));
            if entry.worker_a != entry.worker_b {
                per_worker[entry.worker_b as usize]
                    .new_borders
                    .push((entry.border_id, entry.side_b));
            }
        }

        for batch in resolution.pending_commutations {
            per_worker[batch.worker as usize]
                .pending_commutations
                .push(batch);
        }
    }

    per_worker
        .into_iter()
        .enumerate()
        .map(|(w, dispatch)| (w as WorkerId, dispatch))
        .collect()
}

/// Coordinator-issued correlation handle for a 2-phase commutation
/// request (DC-B5). One `CommutationBatch` in a `BorderResolution`
/// carries one `CommutationId`; the same id appears on every
/// `PendingPortRef::Pending { commutation_id, .. }` that references
/// an agent minted by that batch. Distinct from the wire-layer
/// `PendingCommutation.request_id` field — the resolver-level handle
/// is coarser-grained (one per batch, not per agent).
pub(crate) type CommutationId = u64;

/// Per-worker, per-resolution commutation/erasure request (DC-B5
/// 2-phase). The addressed worker mints `target_symbols.len()`
/// agents from its own `IdRange`; their `(request-index, AgentId)`
/// pairs echo back in `Message::RoundResult.minted_agents` via the
/// wire-layer `PendingCommutation` flow (§3.4 TASK-0370).
///
/// `local_wiring` gives the hints the worker applies immediately at
/// round N+1 to wire each new agent's auxiliary-port neighbours
/// BEFORE echoing its id; entries are `(agent_slot, port_slot,
/// target)` where `agent_slot < target_symbols.len()` and
/// `port_slot ∈ {0, 1, 2}` (`0` = principal, `1..=2` = aux). The
/// principal-port neighbour is typically the consumed agent's former
/// auxiliary-port target; aux-port wiring is used for CON-DUP
/// internal cross-pattern wires that live ENTIRELY on this worker.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct CommutationBatch {
    pub(crate) commutation_id: CommutationId,
    pub(crate) worker: WorkerId,
    pub(crate) target_symbols: Vec<Symbol>,
    pub(crate) local_wiring: Vec<(u8, u8, PortRef)>,
}

/// Reference to a port that may not yet have a concrete `AgentId`
/// (DC-B5 placeholder). `Concrete` carries a live `PortRef`;
/// `Pending` is a handle into a not-yet-finalized
/// `CommutationBatch.target_symbols` slot.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum PendingPortRef {
    Concrete(PortRef),
    Pending {
        commutation_id: CommutationId,
        agent_slot: u8,
        port_slot: u8,
    },
}

/// New or preserved cross-partition border with possibly pending
/// endpoints (DC-B5 CON-DUP + DC-B6 CON-ERA / DUP-ERA).
///
/// When `border_id` is a FRESH id (allocated via
/// `BorderIdAllocator`), the entry is a NEW cross-partition wire the
/// commutation produced. When `border_id` is an EXISTING id from
/// `graph.borders`, the entry signals a DC-B6 "update existing
/// border" (CON-ERA / DUP-ERA preserving an auxiliary border whose
/// endpoint re-points to a newly-minted ERA's principal port). The
/// coordinator's round-N+2 finalization step (item 2.26-C)
/// disambiguates by consulting `graph.borders.contains_key`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct PendingNewBorder {
    pub(crate) border_id: u32,
    pub(crate) side_a: PendingPortRef,
    pub(crate) side_b: PendingPortRef,
    pub(crate) worker_a: WorkerId,
    pub(crate) worker_b: WorkerId,
}

/// Monotonic `u32` counter for allocating fresh cross-partition
/// border ids during CON-DUP resolution. Seeded from the current
/// graph's maximum border id (+1) so new ids never collide with
/// existing borders. Coordinator-owned; passed `&mut` through the
/// resolver for back-to-back CON-DUP calls.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct BorderIdAllocator {
    next: u32,
}

#[allow(dead_code)]
impl BorderIdAllocator {
    /// Seed from the graph's current `max(border_id) + 1`; uses `0`
    /// as the seed for an empty graph.
    pub(crate) fn from_graph(graph: &BorderGraph) -> Self {
        let next = graph
            .borders
            .keys()
            .copied()
            .max()
            .map(|m| m.saturating_add(1))
            .unwrap_or(0);
        Self { next }
    }

    /// Allocate the next fresh id. Panics on `u32::MAX` overflow (a
    /// single BSP run creating ≥ 2^32 borders is out of scope).
    pub(crate) fn next(&mut self) -> u32 {
        let id = self.next;
        self.next = self
            .next
            .checked_add(1)
            .expect("border_resolver: BorderIdAllocator exhausted (u32::MAX borders in one run)");
        id
    }
}

/// Monotonic `u64` counter for `CommutationId` allocation. A single
/// BSP round may produce multiple CON-DUP / CON-ERA / DUP-ERA batches
/// across many borders; each batch needs a distinct correlation
/// handle so `PendingPortRef::Pending` tokens resolve unambiguously.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub(crate) struct CommutationIdAllocator {
    next: u64,
}

#[allow(dead_code)]
impl CommutationIdAllocator {
    pub(crate) fn new() -> Self {
        Self { next: 0 }
    }
    pub(crate) fn next(&mut self) -> CommutationId {
        let id = self.next;
        self.next = self.next.checked_add(1).expect(
            "border_resolver: CommutationIdAllocator exhausted (u64::MAX commutations in one run)",
        );
        id
    }
}

/// Given a principal-port endpoint (`AgentPort(id, 0)`) on a worker's
/// cached partition, return the live agent's `(AgentId, Symbol)` pair.
/// Returns `None` if the port is not an `AgentPort`, if the referenced
/// agent slot is vacant (`None`) or past the arena, or if the port slot
/// is not principal (`!= 0`).
///
/// Callers MUST only invoke this for `is_redex == true` border sides
/// (both endpoints principal, per R13). The `None` return path is
/// defensive so the dispatcher can surface a typed error instead of
/// panicking. Per DC-B2, the panic policy is enforced at the caller
/// (`assert_agent`), not in the helper.
// Called by `assert_agent` below AND will be invoked directly by
// `package_resolutions` (TASK-0375) / the async coordinator
// (item 2.26-C). The tests in this file exercise it too.
#[allow(dead_code)]
pub(crate) fn materialize_agent(partition: &Partition, port: PortRef) -> Option<(AgentId, Symbol)> {
    match port {
        PortRef::AgentPort(id, 0) => partition
            .subnet
            .agents
            .get(id as usize)?
            .as_ref()
            .map(|agent| (id, agent.symbol)),
        _ => None,
    }
}

/// DC-B2 caller-side panic: wraps `materialize_agent` and panics with
/// a grep-able message when the lookup fails. A `None` here MUST
/// indicate a coordinator-cache-maintenance bug per DC-B1 (the R40c
/// pinning discipline in §3.5 rules out a race artefact, DC-B4).
fn assert_agent(
    partition: &Partition,
    port: PortRef,
    border_id: u32,
    side_name: &str,
) -> (AgentId, Symbol) {
    match materialize_agent(partition, port) {
        Some(pair) => pair,
        None => panic!(
            "border_resolver: agent missing for border {border_id} on \
             side {side_name} (cache desync — check coordinator cache \
             maintenance per DC-B1)"
        ),
    }
}

/// Coordinator-side resolution of a single border redex (SPEC-19 §3.2
/// R13 + R14 + R15 part 2).
///
/// `graph` MUST hold an entry for `border_id`; both endpoints of that
/// entry MUST be principal-port agents present in the respective
/// worker's cached partition (enforced by the §3.5 R40c pinning
/// discipline and the coordinator's own cache maintenance, DC-B1).
///
/// Behaviour:
/// 1. Look up the border's `BorderState` and clone it (so later
///    `graph.remove_border` does not invalidate it).
/// 2. Read the principal-port endpoint from each worker's partition
///    via `materialize_agent` + `assert_agent` (DC-B2 panic format).
/// 3. Dispatch on the symbol pair `(sym_a, sym_b)`:
///    - same-symbol rules (CON-CON, DUP-DUP, ERA-ERA) resolve here.
///    - asymmetric pairs (CON-DUP / CON-ERA / DUP-ERA) are TASK-0374's
///      territory; the dispatch arm invokes `todo!` so Stage 2 tests
///      that only exercise the same-symbol matrix pass while the
///      asymmetric path remains unimplemented.
/// 4. Call `graph.remove_border(border_id)` — same-symbol rules
///    annihilate the border unconditionally (R16). CON-DUP's
///    `add_border_states` call lives in TASK-0374.
/// 5. Return the `BorderResolution`. Side-effect on `graph` is
///    the `remove_border` call in step 4.
// Wired into the async coordinator in item 2.26-C (after
// `package_resolutions` ships in TASK-0375). Tests cover the
// dispatcher here.
#[allow(dead_code)]
pub(crate) fn resolve_border_redex(
    graph: &mut BorderGraph,
    partitions: &[Partition],
    border_id: u32,
    border_alloc: &mut BorderIdAllocator,
    commutation_alloc: &mut CommutationIdAllocator,
) -> BorderResolution {
    let state: BorderState = graph.borders.get(&border_id).cloned().unwrap_or_else(|| {
        panic!(
            "border_resolver: border {border_id} not present in graph \
                 (caller must pass an active border_id — see DC-B1)"
        )
    });

    let partition_a = &partitions[state.worker_a as usize];
    let partition_b = &partitions[state.worker_b as usize];

    let (id_a, sym_a) = assert_agent(partition_a, state.side_a, border_id, "side_a");
    let (id_b, sym_b) = assert_agent(partition_b, state.side_b, border_id, "side_b");

    // Dispatch on the symbol pair. Each asymmetric arm passes
    // already-normalized arguments to the private body — the body
    // receives CON (or the non-ERA agent) in the "canonical" slot so
    // the topology math inside the body reads straight off SPEC-03.
    // The body-level `&*graph` reborrow gives an immutable view for
    // existing-border lookups (DC-B6); the `&mut graph` resumes for
    // the trailing `remove_border` call after the body returns.
    let resolution = match (sym_a, sym_b) {
        (Symbol::Con, Symbol::Con) => resolve_con_con(&state, partition_a, partition_b, id_a, id_b),
        (Symbol::Dup, Symbol::Dup) => resolve_dup_dup(&state, partition_a, partition_b, id_a, id_b),
        (Symbol::Era, Symbol::Era) => resolve_era_era(&state),
        (Symbol::Con, Symbol::Dup) => resolve_con_dup(
            &state,
            partition_a,
            partition_b,
            id_a,
            id_b,
            state.worker_a,
            state.worker_b,
            border_alloc,
            commutation_alloc,
        ),
        (Symbol::Dup, Symbol::Con) => resolve_con_dup(
            &state,
            partition_b,
            partition_a,
            id_b,
            id_a,
            state.worker_b,
            state.worker_a,
            border_alloc,
            commutation_alloc,
        ),
        (Symbol::Con, Symbol::Era) | (Symbol::Dup, Symbol::Era) => resolve_non_era_era(
            &state,
            &*graph,
            partition_a,
            id_a,
            sym_a,
            state.worker_a,
            state.worker_b,
            commutation_alloc,
        ),
        (Symbol::Era, Symbol::Con) | (Symbol::Era, Symbol::Dup) => resolve_non_era_era(
            &state,
            &*graph,
            partition_b,
            id_b,
            sym_b,
            state.worker_b,
            state.worker_a,
            commutation_alloc,
        ),
    };

    graph.remove_border(border_id);
    resolution
}

/// CON-CON: cross-pattern annihilation (SPEC-03 `interact_anni` mirror).
///
/// Pair `(a.1 ↔ b.2, a.2 ↔ b.1)`. The pairs are emitted against
/// `worker_a`'s bucket because the resolver treats the `side_a` worker
/// as the canonical attribution point; `package_resolutions`
/// (TASK-0375) re-fans each pair to the proper worker when one
/// endpoint is purely local to that worker or when both endpoints
/// cross partition boundaries.
fn resolve_con_con(
    state: &BorderState,
    partition_a: &Partition,
    partition_b: &Partition,
    id_a: AgentId,
    id_b: AgentId,
) -> BorderResolution {
    let t_a1 = partition_a.subnet.get_target(PortRef::AgentPort(id_a, 1));
    let t_a2 = partition_a.subnet.get_target(PortRef::AgentPort(id_a, 2));
    let t_b1 = partition_b.subnet.get_target(PortRef::AgentPort(id_b, 1));
    let t_b2 = partition_b.subnet.get_target(PortRef::AgentPort(id_b, 2));

    let worker_a_deltas = WorkerDeltas {
        border_deltas: Vec::new(),
        local_reconnections: vec![(t_a1, t_b2), (t_a2, t_b1)],
    };

    BorderResolution {
        worker_deltas: vec![(state.worker_a, worker_a_deltas)],
        resolved_borders: vec![(state.border_id, state.worker_a, state.worker_b)],
        ..Default::default()
    }
}

/// DUP-DUP: parallel-pattern annihilation (SPEC-03 `interact_anni`
/// parallel branch). Pair `(a.1 ↔ b.1, a.2 ↔ b.2)`.
fn resolve_dup_dup(
    state: &BorderState,
    partition_a: &Partition,
    partition_b: &Partition,
    id_a: AgentId,
    id_b: AgentId,
) -> BorderResolution {
    let t_a1 = partition_a.subnet.get_target(PortRef::AgentPort(id_a, 1));
    let t_a2 = partition_a.subnet.get_target(PortRef::AgentPort(id_a, 2));
    let t_b1 = partition_b.subnet.get_target(PortRef::AgentPort(id_b, 1));
    let t_b2 = partition_b.subnet.get_target(PortRef::AgentPort(id_b, 2));

    let worker_a_deltas = WorkerDeltas {
        border_deltas: Vec::new(),
        local_reconnections: vec![(t_a1, t_b1), (t_a2, t_b2)],
    };

    BorderResolution {
        worker_deltas: vec![(state.worker_a, worker_a_deltas)],
        resolved_borders: vec![(state.border_id, state.worker_a, state.worker_b)],
        ..Default::default()
    }
}

/// ERA-ERA: void (SPEC-03 `interact_void` mirror). ERA has arity 0, so
/// there are no auxiliary-port targets to re-route. The resolver only
/// records the border as resolved.
fn resolve_era_era(state: &BorderState) -> BorderResolution {
    BorderResolution {
        resolved_borders: vec![(state.border_id, state.worker_a, state.worker_b)],
        ..Default::default()
    }
}

/// R48 slot-marker base for intra-`CommutationBatch` wiring references.
/// `CommutationBatch.local_wiring` entries may point at a sibling
/// agent in the SAME batch whose concrete `AgentId` is still pending.
/// Encode that sibling as `AgentPort(SLOT_MARKER_BASE + sibling_slot,
/// port)` — worker-side code substitutes the concrete `AgentId` before
/// applying the wire. The range `u32::MAX - 10_000 .. u32::MAX` is
/// reserved by SPEC-19 R48 so these markers never collide with a
/// worker-allocated `AgentId`.
// Consumed at the worker side by `run_grid_delta` (item 2.26-C).
#[allow(dead_code)]
pub(crate) const SLOT_MARKER_BASE: u32 = u32::MAX - 10_000;

/// Encode a slot-relative sibling reference for intra-batch wiring.
/// `slot` is the agent's position in the owning
/// `CommutationBatch.target_symbols`.
fn slot_marker(slot: u8) -> AgentId {
    SLOT_MARKER_BASE + slot as u32
}

/// CON-DUP: commutation (SPEC-03 `interact_comm` mirror) — the only
/// rule that INCREASES agent count (+2 net). The coordinator-side
/// resolution is 2-phase per DC-B5: each involved worker gets a
/// `CommutationBatch` requesting 2 new agents (1 Dup + 1 Con) from
/// its own `IdRange`; cross-partition wires surface as
/// `PendingNewBorder` entries with `PendingPortRef::Pending` tokens
/// referencing the batch's commutation handle + agent slot.
///
/// Agent assignment (balanced — each worker mints 1 Dup + 1 Con):
///
/// | Agent | Symbol | Worker       | Slot | Inherits side of |
/// |-------|--------|--------------|------|------------------|
/// | p     | Dup    | worker_con   | 0    | con.1            |
/// | r     | Con    | worker_con   | 1    | dup.1            |
/// | q     | Dup    | worker_dup   | 0    | con.2            |
/// | s     | Con    | worker_dup   | 1    | dup.2            |
///
/// Wires (mirror of `interact_comm` in `reduction/rules.rs`):
/// - External: p.0↔t_c1, q.0↔t_c2, r.0↔t_d1, s.0↔t_d2
/// - Internal: p.1↔r.1, p.2↔s.1, q.1↔r.2, q.2↔s.2
///
/// Wires whose endpoints both land on the same minting worker become
/// `CommutationBatch.local_wiring` hints (intra-worker apply). Wires
/// crossing worker boundaries (or re-routing through an existing
/// `FreePort(bid)` whose `bid` is NOT tracked in `graph.borders`)
/// surface as fresh `PendingNewBorder`s. No `add_border_states` call
/// at this stage — round N+2 finalizes once `MintedAgent` echoes
/// arrive (see DC-B5 flow step 3).
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
fn resolve_con_dup(
    state: &BorderState,
    partition_con: &Partition,
    partition_dup: &Partition,
    id_con: AgentId,
    id_dup: AgentId,
    worker_con: WorkerId,
    worker_dup: WorkerId,
    border_alloc: &mut BorderIdAllocator,
    commutation_alloc: &mut CommutationIdAllocator,
) -> BorderResolution {
    // Auxiliary-port targets read from each worker's partition view.
    // `get_target` yields `AgentPort(_, _)` when the target is a live
    // local agent; `FreePort(bid)` when it already crossed a border.
    let t_c1 = partition_con
        .subnet
        .get_target(PortRef::AgentPort(id_con, 1));
    let t_c2 = partition_con
        .subnet
        .get_target(PortRef::AgentPort(id_con, 2));
    let t_d1 = partition_dup
        .subnet
        .get_target(PortRef::AgentPort(id_dup, 1));
    let t_d2 = partition_dup
        .subnet
        .get_target(PortRef::AgentPort(id_dup, 2));

    let cid_con = commutation_alloc.next();
    let cid_dup = commutation_alloc.next();

    // Slot-layout constants — encoded in both `target_symbols` order
    // and in every `PendingPortRef::Pending { agent_slot, .. }` below.
    const SLOT_P: u8 = 0; // worker_con batch slot 0 — Dup (p)
    const SLOT_R: u8 = 1; // worker_con batch slot 1 — Con (r)
    const SLOT_Q: u8 = 0; // worker_dup batch slot 0 — Dup (q)
    const SLOT_S: u8 = 1; // worker_dup batch slot 1 — Con (s)

    let mut con_wiring: Vec<(u8, u8, PortRef)> = Vec::new();
    let mut dup_wiring: Vec<(u8, u8, PortRef)> = Vec::new();
    let mut pending_new_borders: Vec<PendingNewBorder> = Vec::new();

    // External principal wires (4) — emitted via helper that classifies
    // each target as LOCAL (→ local_wiring) or CROSS (→ PendingNewBorder).
    emit_external_principal(
        t_c1,
        worker_con,
        cid_con,
        SLOT_P,
        worker_con,
        &mut con_wiring,
        &mut pending_new_borders,
        border_alloc,
    );
    emit_external_principal(
        t_c2,
        worker_con,
        cid_dup,
        SLOT_Q,
        worker_dup,
        &mut dup_wiring,
        &mut pending_new_borders,
        border_alloc,
    );
    emit_external_principal(
        t_d1,
        worker_dup,
        cid_con,
        SLOT_R,
        worker_con,
        &mut con_wiring,
        &mut pending_new_borders,
        border_alloc,
    );
    emit_external_principal(
        t_d2,
        worker_dup,
        cid_dup,
        SLOT_S,
        worker_dup,
        &mut dup_wiring,
        &mut pending_new_borders,
        border_alloc,
    );

    // Internal aux-to-aux wires (4). Under the balanced assignment
    // above, exactly 2 of the 4 are intra-worker (p.1↔r.1 on worker_con,
    // q.2↔s.2 on worker_dup) and 2 cross (p.2↔s.1, q.1↔r.2).
    // Intra-worker wires use slot-marker placeholders (R48) inside
    // `local_wiring`; the worker substitutes concrete `AgentId`s once
    // it has minted both sibling agents.
    con_wiring.push((SLOT_P, 1, PortRef::AgentPort(slot_marker(SLOT_R), 1)));
    dup_wiring.push((SLOT_Q, 2, PortRef::AgentPort(slot_marker(SLOT_S), 2)));
    pending_new_borders.push(PendingNewBorder {
        border_id: border_alloc.next(),
        side_a: PendingPortRef::Pending {
            commutation_id: cid_con,
            agent_slot: SLOT_P,
            port_slot: 2,
        },
        side_b: PendingPortRef::Pending {
            commutation_id: cid_dup,
            agent_slot: SLOT_S,
            port_slot: 1,
        },
        worker_a: worker_con,
        worker_b: worker_dup,
    });
    pending_new_borders.push(PendingNewBorder {
        border_id: border_alloc.next(),
        side_a: PendingPortRef::Pending {
            commutation_id: cid_dup,
            agent_slot: SLOT_Q,
            port_slot: 1,
        },
        side_b: PendingPortRef::Pending {
            commutation_id: cid_con,
            agent_slot: SLOT_R,
            port_slot: 2,
        },
        worker_a: worker_dup,
        worker_b: worker_con,
    });

    BorderResolution {
        resolved_borders: vec![(state.border_id, state.worker_a, state.worker_b)],
        pending_commutations: vec![
            CommutationBatch {
                commutation_id: cid_con,
                worker: worker_con,
                target_symbols: vec![Symbol::Dup, Symbol::Con],
                local_wiring: con_wiring,
            },
            CommutationBatch {
                commutation_id: cid_dup,
                worker: worker_dup,
                target_symbols: vec![Symbol::Dup, Symbol::Con],
                local_wiring: dup_wiring,
            },
        ],
        pending_new_borders,
        ..Default::default()
    }
}

/// Classify a principal-port target for a newly-minted CON-DUP agent
/// and emit the appropriate wiring bucket (local_wiring on the minting
/// worker's batch, or a fresh `PendingNewBorder`).
///
/// - `target`: the partition-local view of the former aux neighbour of
///   the deleted CON/DUP.
/// - `target_home`: the worker that owns the view `target` was read
///   from (i.e. the worker where an `AgentPort` target lives locally).
/// - `cid` / `new_slot` / `new_worker`: identity of the new agent whose
///   principal port is being wired.
///
/// Preserves the DC-B5 2-phase contract: `local_wiring` carries
/// resolver-time concrete `PortRef`s for purely local hops; cross
/// wires defer concrete-id substitution to round N+2 via
/// `PendingPortRef::Pending` tokens.
#[allow(clippy::too_many_arguments)]
fn emit_external_principal(
    target: PortRef,
    target_home: WorkerId,
    cid: CommutationId,
    new_slot: u8,
    new_worker: WorkerId,
    new_worker_wiring: &mut Vec<(u8, u8, PortRef)>,
    pending_new_borders: &mut Vec<PendingNewBorder>,
    border_alloc: &mut BorderIdAllocator,
) {
    match target {
        PortRef::AgentPort(_, _) if new_worker == target_home => {
            // Both endpoints on the same worker — wire locally.
            new_worker_wiring.push((new_slot, 0, target));
        }
        _ => {
            // Cross-worker principal OR already-cross FreePort. Either
            // way, the resolver emits a fresh `PendingNewBorder`. Under
            // DC-B6 the "preserve-existing-border" path belongs to
            // CON-ERA / DUP-ERA; for CON-DUP we always allocate a
            // fresh id so the round-N+2 finalizer can populate both
            // endpoints cleanly.
            pending_new_borders.push(PendingNewBorder {
                border_id: border_alloc.next(),
                side_a: PendingPortRef::Pending {
                    commutation_id: cid,
                    agent_slot: new_slot,
                    port_slot: 0,
                },
                side_b: PendingPortRef::Concrete(target),
                worker_a: new_worker,
                worker_b: target_home,
            });
        }
    }
}

/// CON-ERA / DUP-ERA: asymmetric erasure (SPEC-03 `interact_eras`
/// mirror). The non-ERA agent is consumed; 2 new ERA agents are minted
/// on the non-ERA's worker, each inheriting one of the non-ERA's
/// auxiliary-port neighbours via its principal port.
///
/// **DC-B6 preservation rule.** When an auxiliary-port neighbour of
/// the consumed non-ERA is a `FreePort(bid)` for an EXISTING border
/// `bid` (tracked in `graph.borders`), the resolver does NOT allocate
/// a fresh id. It emits a `PendingNewBorder { border_id: bid, ... }`
/// that reuses `bid`; round-N+2 finalization (item 2.26-C) then calls
/// `graph.apply_deltas` (not `add_border_states`) to update the
/// existing entry's endpoint to the new ERA's principal. This
/// preserves wire continuity across the reduction (SPEC-03 R14
/// connectivity preservation).
///
/// Intra-worker neighbours (`AgentPort(_, _)`) surface as
/// `CommutationBatch.local_wiring` hints — the new ERA's principal
/// wires directly to the live agent on the same worker.
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
fn resolve_non_era_era(
    state: &BorderState,
    graph: &BorderGraph,
    partition_non_era: &Partition,
    non_era_id: AgentId,
    non_era_symbol: Symbol,
    worker_non_era: WorkerId,
    worker_era: WorkerId,
    commutation_alloc: &mut CommutationIdAllocator,
) -> BorderResolution {
    debug_assert!(
        matches!(non_era_symbol, Symbol::Con | Symbol::Dup),
        "resolve_non_era_era precondition: non_era_symbol must be CON or DUP, got {non_era_symbol:?}"
    );
    let _ = non_era_symbol; // arity is fixed at 2 for CON/DUP (SPEC-03).
    let _ = worker_era; // carried for 2.26-C finalization symmetry.

    let t_1 = partition_non_era
        .subnet
        .get_target(PortRef::AgentPort(non_era_id, 1));
    let t_2 = partition_non_era
        .subnet
        .get_target(PortRef::AgentPort(non_era_id, 2));

    let cid = commutation_alloc.next();

    // Slot layout inside the non-ERA worker's `CommutationBatch`:
    //   slot 0 — ERA inheriting aux port 1 target (t_1)
    //   slot 1 — ERA inheriting aux port 2 target (t_2)
    const SLOT_E1: u8 = 0;
    const SLOT_E2: u8 = 1;

    let mut local_wiring: Vec<(u8, u8, PortRef)> = Vec::new();
    let mut pending_new_borders: Vec<PendingNewBorder> = Vec::new();

    emit_erasure_principal(
        t_1,
        cid,
        SLOT_E1,
        worker_non_era,
        graph,
        &mut local_wiring,
        &mut pending_new_borders,
    );
    emit_erasure_principal(
        t_2,
        cid,
        SLOT_E2,
        worker_non_era,
        graph,
        &mut local_wiring,
        &mut pending_new_borders,
    );

    BorderResolution {
        resolved_borders: vec![(state.border_id, state.worker_a, state.worker_b)],
        pending_commutations: vec![CommutationBatch {
            commutation_id: cid,
            worker: worker_non_era,
            target_symbols: vec![Symbol::Era, Symbol::Era],
            local_wiring,
        }],
        pending_new_borders,
        ..Default::default()
    }
}

/// Classify the principal-port target for a newly-minted ERA agent
/// under CON-ERA / DUP-ERA and emit either:
/// - a local_wiring hint (target is a live local agent), or
/// - a `PendingNewBorder` reusing an EXISTING border id (DC-B6
///   preservation — the new ERA becomes the replacement endpoint of
///   an auxiliary-port border that survives the reduction), or
/// - a `PendingNewBorder` with a fresh-ish placeholder when the
///   target is `FreePort(bid)` but no graph entry is tracked (treated
///   as a dangling wire — `side_b` concrete `FreePort`, same worker).
fn emit_erasure_principal(
    target: PortRef,
    cid: CommutationId,
    era_slot: u8,
    era_worker: WorkerId,
    graph: &BorderGraph,
    local_wiring: &mut Vec<(u8, u8, PortRef)>,
    pending_new_borders: &mut Vec<PendingNewBorder>,
) {
    match target {
        PortRef::AgentPort(_, _) => {
            // Live local agent on the non-ERA worker — wire via
            // local_wiring; the minted ERA's principal connects
            // directly to the existing agent.
            local_wiring.push((era_slot, 0, target));
        }
        PortRef::FreePort(bid) => match graph.borders.get(&bid) {
            Some(existing) => {
                // DC-B6: preserve the existing border — reuse `bid`.
                // The other side stays Concrete; our side becomes
                // the pending ERA's principal.
                let (other_side, other_worker) = if existing.worker_a == era_worker {
                    (existing.side_b, existing.worker_b)
                } else {
                    (existing.side_a, existing.worker_a)
                };
                pending_new_borders.push(PendingNewBorder {
                    border_id: bid,
                    side_a: PendingPortRef::Pending {
                        commutation_id: cid,
                        agent_slot: era_slot,
                        port_slot: 0,
                    },
                    side_b: PendingPortRef::Concrete(other_side),
                    worker_a: era_worker,
                    worker_b: other_worker,
                });
            }
            None => {
                // Dangling FreePort — no graph entry. Keep the
                // `FreePort(bid)` concrete on `side_b` so downstream
                // code can surface the inconsistency without
                // panicking here.
                pending_new_borders.push(PendingNewBorder {
                    border_id: bid,
                    side_a: PendingPortRef::Pending {
                        commutation_id: cid,
                        agent_slot: era_slot,
                        port_slot: 0,
                    },
                    side_b: PendingPortRef::Concrete(target),
                    worker_a: era_worker,
                    worker_b: era_worker,
                });
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{Net, DISCONNECTED};
    use crate::partition::types::{IdRange, Partition};
    use std::collections::{HashMap, HashSet};

    fn single_con_partition() -> (Partition, AgentId) {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        let partition = Partition {
            subnet: net,
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange { start: 0, end: 1 },
            border_id_start: 0,
            border_id_end: 0,
        };
        (partition, id)
    }

    // -----------------------------------------------------------------
    // TEST-SPEC-0373 fixtures
    // -----------------------------------------------------------------
    //
    // Each `two_partition_*_fixture` builds a 2-partition setup where
    // agent_0 of each partition has its principal port connected to a
    // border `FreePort(border_id)` and its aux ports wired to local
    // agents. The returned `BorderGraph` has exactly ONE border (id =
    // `border_id`) of the requested redex shape. The graph is built by
    // direct construction (not `from_partition_plan`) so the fixture
    // depends on nothing outside the resolver's own preconditions.

    fn build_two_partition_same_symbol_fixture(
        symbol: Symbol,
        border_id: u32,
    ) -> (BorderGraph, Vec<Partition>) {
        let make_partition = |worker_id: WorkerId| -> Partition {
            let mut net = Net::new();
            let a0 = net.create_agent(symbol);
            let a1 = net.create_agent(Symbol::Era);
            let a2 = net.create_agent(Symbol::Era);
            // Border principal: a0.0 ↔ FreePort(border_id).
            net.connect(PortRef::AgentPort(a0, 0), PortRef::FreePort(border_id));
            // Aux targets: a0.1 ↔ a1.0 (principal of local Era),
            // a0.2 ↔ a2.0 (principal of local Era). These become the
            // `t_a1 / t_a2 / t_b1 / t_b2` values the resolver pulls
            // via `get_target`.
            net.connect(PortRef::AgentPort(a0, 1), PortRef::AgentPort(a1, 0));
            net.connect(PortRef::AgentPort(a0, 2), PortRef::AgentPort(a2, 0));
            let mut free_port_index = HashMap::new();
            free_port_index.insert(border_id, PortRef::AgentPort(a0, 0));
            Partition {
                subnet: net,
                worker_id,
                free_port_index,
                id_range: IdRange {
                    start: 0,
                    end: u32::MAX,
                },
                border_id_start: 0,
                border_id_end: u32::MAX,
            }
        };
        let p0 = make_partition(0);
        let p1 = make_partition(1);

        let mut borders = HashMap::new();
        borders.insert(
            border_id,
            BorderState {
                border_id,
                side_a: PortRef::AgentPort(0, 0),
                side_b: PortRef::AgentPort(0, 0),
                worker_a: 0,
                worker_b: 1,
                is_redex: true,
            },
        );
        let mut active_redexes = HashSet::new();
        active_redexes.insert(border_id);
        let graph = BorderGraph {
            borders,
            worker_borders: vec![vec![border_id], vec![border_id]],
            active_redexes,
        };

        (graph, vec![p0, p1])
    }

    fn build_two_partition_era_era_fixture(border_id: u32) -> (BorderGraph, Vec<Partition>) {
        let make_partition = |worker_id: WorkerId| -> Partition {
            let mut net = Net::new();
            let a0 = net.create_agent(Symbol::Era);
            // Era has arity 0: only the principal is a real port; connect
            // it to the border FreePort so the sighting is well-formed.
            net.connect(PortRef::AgentPort(a0, 0), PortRef::FreePort(border_id));
            let mut free_port_index = HashMap::new();
            free_port_index.insert(border_id, PortRef::AgentPort(a0, 0));
            Partition {
                subnet: net,
                worker_id,
                free_port_index,
                id_range: IdRange {
                    start: 0,
                    end: u32::MAX,
                },
                border_id_start: 0,
                border_id_end: u32::MAX,
            }
        };
        let p0 = make_partition(0);
        let p1 = make_partition(1);

        let mut borders = HashMap::new();
        borders.insert(
            border_id,
            BorderState {
                border_id,
                side_a: PortRef::AgentPort(0, 0),
                side_b: PortRef::AgentPort(0, 0),
                worker_a: 0,
                worker_b: 1,
                is_redex: true,
            },
        );
        let mut active_redexes = HashSet::new();
        active_redexes.insert(border_id);
        let graph = BorderGraph {
            borders,
            worker_borders: vec![vec![border_id], vec![border_id]],
            active_redexes,
        };

        (graph, vec![p0, p1])
    }

    #[test]
    fn materialize_agent_returns_symbol_for_principal_port_of_live_agent() {
        let (partition, id) = single_con_partition();
        let result = materialize_agent(&partition, PortRef::AgentPort(id, 0));
        assert_eq!(result, Some((id, Symbol::Con)));
    }

    #[test]
    fn materialize_agent_returns_none_for_non_principal_port_slot() {
        let (partition, id) = single_con_partition();
        let left = materialize_agent(&partition, PortRef::AgentPort(id, 1));
        let right = materialize_agent(&partition, PortRef::AgentPort(id, 2));
        assert_eq!(left, None);
        assert_eq!(right, None);
    }

    #[test]
    fn materialize_agent_returns_none_for_free_port_and_disconnected() {
        let (partition, _id) = single_con_partition();
        let free = materialize_agent(&partition, PortRef::FreePort(42));
        let disc = materialize_agent(&partition, DISCONNECTED);
        assert_eq!(free, None);
        assert_eq!(disc, None);
    }

    #[test]
    fn materialize_agent_returns_none_for_vacated_agent_slot() {
        let mut net = Net::new();
        let _a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        let _c = net.create_agent(Symbol::Era);
        net.remove_agent(b);
        let partition = Partition {
            subnet: net,
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange { start: 0, end: 3 },
            border_id_start: 0,
            border_id_end: 0,
        };
        let vacant = materialize_agent(&partition, PortRef::AgentPort(b, 0));
        let out_of_range = materialize_agent(&partition, PortRef::AgentPort(999, 0));
        assert_eq!(vacant, None);
        assert_eq!(out_of_range, None);
    }

    // -----------------------------------------------------------------
    // TEST-SPEC-0373 — `resolve_border_redex` dispatcher + same-symbol
    // rule bodies (CON-CON, DUP-DUP, ERA-ERA). The dispatcher's
    // asymmetric branches (CON-DUP / CON-ERA / DUP-ERA) are out of
    // scope here; they land in TEST-SPEC-0374.
    // -----------------------------------------------------------------

    /// UT-0373-01 — CON-CON cross pattern, border removed, DC-B3 +
    /// DC-B7 output shape.
    #[test]
    fn resolve_con_con_removes_border_and_emits_cross_pattern_deltas() {
        let (mut graph, partitions) = build_two_partition_same_symbol_fixture(Symbol::Con, 0);
        let mut border_alloc = BorderIdAllocator::from_graph(&graph);
        let mut commutation_alloc = CommutationIdAllocator::new();

        let r = resolve_border_redex(
            &mut graph,
            &partitions,
            0,
            &mut border_alloc,
            &mut commutation_alloc,
        );

        // DC-B7 triple: exactly one resolved border, with the input's
        // worker_a / worker_b orientation preserved.
        assert_eq!(r.resolved_borders, vec![(0, 0, 1)]);
        // CON-CON produces no new borders (R14 Anni).
        assert!(r.new_borders.is_empty());

        // DC-B3: the CON-CON rule body records both aux-to-aux
        // reconnections on worker_a's bucket (resolver convention;
        // `package_resolutions` re-fans them later per DC-B3).
        assert_eq!(r.worker_deltas.len(), 1);
        let (w, wd) = &r.worker_deltas[0];
        assert_eq!(*w, 0);
        assert!(
            wd.border_deltas.is_empty(),
            "same-symbol rule emits no BorderDelta entries with local-only aux targets",
        );
        assert_eq!(
            wd.local_reconnections.len(),
            2,
            "CON-CON must emit exactly 2 aux-to-aux reconnections"
        );
        // Cross pattern: (t_a1 ↔ t_b2, t_a2 ↔ t_b1).
        // In this fixture, t_a1 = t_b1 = AgentPort(1, 0) (P0 and P1
        // both create Era at id 1 as the first aux target) and
        // t_a2 = t_b2 = AgentPort(2, 0). The cross pattern therefore
        // pairs index 1 with index 2 and vice versa, distinguishable
        // from the parallel pattern which would self-pair.
        assert_eq!(
            wd.local_reconnections[0],
            (PortRef::AgentPort(1, 0), PortRef::AgentPort(2, 0)),
            "first pair must be (t_a1, t_b2) = cross",
        );
        assert_eq!(
            wd.local_reconnections[1],
            (PortRef::AgentPort(2, 0), PortRef::AgentPort(1, 0)),
            "second pair must be (t_a2, t_b1) = cross",
        );

        // R15 part 2: border removed from graph + active_redexes.
        assert!(
            !graph.borders.contains_key(&0),
            "remove_border(0) must drop the entry from `borders`"
        );
        assert!(
            !graph.active_redexes.contains(&0),
            "remove_border(0) must drop the entry from `active_redexes`"
        );
    }

    /// UT-0373-02 — DUP-DUP parallel pattern, border removed, DC-B3 +
    /// DC-B7 output shape.
    #[test]
    fn resolve_dup_dup_removes_border_and_emits_parallel_pattern_deltas() {
        let (mut graph, partitions) = build_two_partition_same_symbol_fixture(Symbol::Dup, 0);
        let mut border_alloc = BorderIdAllocator::from_graph(&graph);
        let mut commutation_alloc = CommutationIdAllocator::new();

        let r = resolve_border_redex(
            &mut graph,
            &partitions,
            0,
            &mut border_alloc,
            &mut commutation_alloc,
        );

        assert_eq!(r.resolved_borders, vec![(0, 0, 1)]);
        assert!(r.new_borders.is_empty());

        assert_eq!(r.worker_deltas.len(), 1);
        let (w, wd) = &r.worker_deltas[0];
        assert_eq!(*w, 0);
        assert!(wd.border_deltas.is_empty());
        assert_eq!(wd.local_reconnections.len(), 2);
        // Parallel pattern: (t_a1 ↔ t_b1, t_a2 ↔ t_b2). With the same
        // fixture layout as UT-0373-01 the pairs self-align on each
        // aux index — distinguishable from the cross pattern.
        assert_eq!(
            wd.local_reconnections[0],
            (PortRef::AgentPort(1, 0), PortRef::AgentPort(1, 0)),
            "first pair must be (t_a1, t_b1) = parallel",
        );
        assert_eq!(
            wd.local_reconnections[1],
            (PortRef::AgentPort(2, 0), PortRef::AgentPort(2, 0)),
            "second pair must be (t_a2, t_b2) = parallel",
        );

        assert!(!graph.borders.contains_key(&0));
        assert!(!graph.active_redexes.contains(&0));
    }

    /// UT-0373-03 — ERA-ERA void; no aux payload, border removed.
    #[test]
    fn resolve_era_era_removes_border_with_zero_deltas() {
        let (mut graph, partitions) = build_two_partition_era_era_fixture(0);
        let mut border_alloc = BorderIdAllocator::from_graph(&graph);
        let mut commutation_alloc = CommutationIdAllocator::new();

        let r = resolve_border_redex(
            &mut graph,
            &partitions,
            0,
            &mut border_alloc,
            &mut commutation_alloc,
        );

        assert_eq!(r.resolved_borders, vec![(0, 0, 1)]);
        assert!(r.new_borders.is_empty());
        // TEST-SPEC-0373 resolved-ambiguity: worker_deltas MAY be empty
        // OR may carry per-worker entries with all-empty inner vectors.
        // Assert the weaker "every present entry is empty" contract.
        for (_w, wd) in &r.worker_deltas {
            assert!(
                wd.border_deltas.is_empty(),
                "ERA-ERA emits no BorderDelta entries"
            );
            assert!(
                wd.local_reconnections.is_empty(),
                "ERA-ERA has arity 0 — no aux targets to reconnect"
            );
        }

        assert!(!graph.borders.contains_key(&0));
        assert!(!graph.active_redexes.contains(&0));
    }

    /// UT-0373-04 — dispatcher normalizes (symbol_a, symbol_b) by
    /// consulting the `BorderState`'s actual orientation. Swapping
    /// `worker_a / worker_b` must still reach the same private body.
    #[test]
    fn resolve_border_redex_dispatches_by_symbol_pair_and_normalizes_order() {
        // Variant 1: canonical orientation (worker_a = 0, worker_b = 1).
        let (mut graph_a, parts_a) = build_two_partition_same_symbol_fixture(Symbol::Con, 0);
        let mut border_alloc_a = BorderIdAllocator::from_graph(&graph_a);
        let mut commutation_alloc_a = CommutationIdAllocator::new();
        let r_a = resolve_border_redex(
            &mut graph_a,
            &parts_a,
            0,
            &mut border_alloc_a,
            &mut commutation_alloc_a,
        );
        assert_eq!(r_a.resolved_borders, vec![(0, 0, 1)]);
        assert!(r_a.new_borders.is_empty());
        assert!(!graph_a.borders.contains_key(&0));

        // Variant 2: mirror orientation. Build the same fixture, then
        // flip worker_a / worker_b in the `BorderState`. The partitions
        // are indexed by `state.worker_a / worker_b` so the resolver
        // still reads P1 as `side_a` and P0 as `side_b`.
        let (mut graph_b, parts_b) = build_two_partition_same_symbol_fixture(Symbol::Con, 0);
        {
            let state = graph_b.borders.get_mut(&0).unwrap();
            state.worker_a = 1;
            state.worker_b = 0;
        }
        let mut border_alloc_b = BorderIdAllocator::from_graph(&graph_b);
        let mut commutation_alloc_b = CommutationIdAllocator::new();
        let r_b = resolve_border_redex(
            &mut graph_b,
            &parts_b,
            0,
            &mut border_alloc_b,
            &mut commutation_alloc_b,
        );
        // DC-B7 triple reflects the CURRENT orientation.
        assert_eq!(r_b.resolved_borders, vec![(0, 1, 0)]);
        assert!(r_b.new_borders.is_empty());
        // The worker_deltas bucket is keyed on `state.worker_a`
        // (resolver convention) — now worker 1 in the mirror variant.
        assert_eq!(r_b.worker_deltas.len(), 1);
        assert_eq!(r_b.worker_deltas[0].0, 1);
        assert_eq!(r_b.worker_deltas[0].1.local_reconnections.len(), 2);
        assert!(!graph_b.borders.contains_key(&0));
    }

    /// UT-0373-05 — targeted removal: `remove_border(bid)` touches only
    /// the passed id; other borders (even non-redex) survive.
    #[test]
    fn resolve_border_redex_on_con_con_preserves_graph_non_redex_borders() {
        let (mut graph, partitions) = build_two_partition_same_symbol_fixture(Symbol::Con, 0);
        // Add a second border (id = 1) that is NOT a redex (principal /
        // aux). The fixture graph is built by direct construction; we
        // reuse the existing partitions as carriers because the
        // resolver only touches the passed `border_id`.
        graph.borders.insert(
            1,
            BorderState {
                border_id: 1,
                side_a: PortRef::AgentPort(1, 0),
                side_b: PortRef::AgentPort(1, 1), // auxiliary port — NOT a redex
                worker_a: 0,
                worker_b: 1,
                is_redex: false,
            },
        );
        graph.worker_borders[0].push(1);
        graph.worker_borders[1].push(1);
        // border 1 is NOT inserted into active_redexes (is_redex == false).
        let mut border_alloc = BorderIdAllocator::from_graph(&graph);
        let mut commutation_alloc = CommutationIdAllocator::new();

        let r = resolve_border_redex(
            &mut graph,
            &partitions,
            0,
            &mut border_alloc,
            &mut commutation_alloc,
        );

        assert_eq!(r.resolved_borders, vec![(0, 0, 1)]);
        // resolved_borders names ONLY border 0 — NEVER border 1.
        assert!(
            r.resolved_borders.iter().all(|(bid, _, _)| *bid == 0),
            "resolver must not list border 1 in resolved_borders"
        );

        assert!(
            !graph.borders.contains_key(&0),
            "border 0 (the target) must be removed"
        );
        assert!(
            graph.borders.contains_key(&1),
            "border 1 (untouched) must survive the call"
        );
        // active_redexes was never seeded with border 1 — still empty.
        assert!(graph.active_redexes.is_empty());
    }

    /// UT-0373-06 — DC-B2 caller-side panic. Simulate a cache desync by
    /// vacating the worker's agent slot AFTER the graph is built. The
    /// resolver MUST panic with a uniform, grep-able message that names
    /// the border, the side, mentions "cache desync", and points to
    /// DC-B1 (the cache-maintenance rule surface).
    #[test]
    fn resolve_border_redex_panics_with_dc_b2_message_on_missing_agent() {
        let (graph, mut partitions) = build_two_partition_same_symbol_fixture(Symbol::Con, 0);
        // Cache desync: vacate P0's agent_0 AFTER graph construction.
        // The graph still believes P0.agent_0 is live; `materialize_agent`
        // returns None; `assert_agent` panics with the DC-B2 format.
        partitions[0].subnet.remove_agent(0);

        let mut graph_owned = graph;
        let mut border_alloc = BorderIdAllocator::from_graph(&graph_owned);
        let mut commutation_alloc = CommutationIdAllocator::new();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            resolve_border_redex(
                &mut graph_owned,
                &partitions,
                0,
                &mut border_alloc,
                &mut commutation_alloc,
            )
        }));

        let err = result.expect_err(
            "resolver must panic when a border side's agent is absent \
             (cache desync per DC-B1)",
        );
        // The panic payload is a `String` (produced by `panic!(...)`
        // with a format argument in the resolver) or a `&'static str`
        // if a future refactor uses `expect(...)`. Handle both.
        let msg: String = err
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| err.downcast_ref::<&'static str>().map(|s| (*s).to_string()))
            .unwrap_or_else(|| {
                panic!(
                    "panic payload must be `String` or `&'static str` — \
                     got an unknown type; check DC-B2 panic format"
                )
            });

        assert!(
            msg.contains("border_resolver: agent missing for border 0"),
            "panic header missing or border id wrong; got: {msg}"
        );
        assert!(
            msg.contains("side "),
            "panic message must name the offending side (side_a / side_b); got: {msg}"
        );
        assert!(
            msg.contains("cache desync"),
            "panic message must hint at cache desync; got: {msg}"
        );
        assert!(
            msg.contains("DC-B1"),
            "panic message must cite DC-B1 per DC-B2 verdict; got: {msg}"
        );
    }

    // -----------------------------------------------------------------
    // TEST-SPEC-0374 fixtures — asymmetric (CON-DUP, CON-ERA, DUP-ERA)
    // -----------------------------------------------------------------

    /// Build a 2-partition fixture where P0's `agent_0` has symbol
    /// `sym_a` and P1's `agent_0` has symbol `sym_b`. Each non-ERA
    /// agent's auxiliary ports are wired to 2 LOCAL Era agents (arity
    /// 0, principal side) — stable targets for the commutation /
    /// erasure topology to re-route through. The single border in the
    /// returned `BorderGraph` is always principal-principal on the
    /// `agent_0`s (a redex for the resolver to consume).
    fn build_two_partition_asymmetric_fixture(
        sym_a: Symbol,
        sym_b: Symbol,
        border_id: u32,
    ) -> (BorderGraph, Vec<Partition>) {
        let make_partition = |worker_id: WorkerId, sym: Symbol| -> Partition {
            let mut net = Net::new();
            let a0 = net.create_agent(sym);
            net.connect(PortRef::AgentPort(a0, 0), PortRef::FreePort(border_id));
            if sym != Symbol::Era {
                let a1 = net.create_agent(Symbol::Era);
                let a2 = net.create_agent(Symbol::Era);
                net.connect(PortRef::AgentPort(a0, 1), PortRef::AgentPort(a1, 0));
                net.connect(PortRef::AgentPort(a0, 2), PortRef::AgentPort(a2, 0));
            }
            let mut free_port_index = HashMap::new();
            free_port_index.insert(border_id, PortRef::AgentPort(a0, 0));
            Partition {
                subnet: net,
                worker_id,
                free_port_index,
                id_range: IdRange {
                    start: 0,
                    end: u32::MAX,
                },
                border_id_start: 0,
                border_id_end: u32::MAX,
            }
        };
        let p0 = make_partition(0, sym_a);
        let p1 = make_partition(1, sym_b);

        let mut borders = HashMap::new();
        borders.insert(
            border_id,
            BorderState {
                border_id,
                side_a: PortRef::AgentPort(0, 0),
                side_b: PortRef::AgentPort(0, 0),
                worker_a: 0,
                worker_b: 1,
                is_redex: true,
            },
        );
        let mut active_redexes = HashSet::new();
        active_redexes.insert(border_id);
        let graph = BorderGraph {
            borders,
            worker_borders: vec![vec![border_id], vec![border_id]],
            active_redexes,
        };
        (graph, vec![p0, p1])
    }

    /// CON-ERA (or DUP-ERA) with a SURVIVING auxiliary border. The
    /// consumed non-ERA's aux port 1 connects to `FreePort(bid_aux)`;
    /// the other side of `bid_aux` lives on P1 as the principal port
    /// of a local Era. Aux port 2 of the non-ERA connects to a local
    /// Era. The principal border `bid_principal` remains a redex.
    fn build_con_era_aux_border_fixture(
        non_era_symbol: Symbol,
        bid_principal: u32,
        bid_aux: u32,
    ) -> (BorderGraph, Vec<Partition>) {
        assert!(
            matches!(non_era_symbol, Symbol::Con | Symbol::Dup),
            "fixture: non_era_symbol must be Con or Dup",
        );
        // Worker 0: non-ERA (agent 0) with principal to bid_principal,
        // aux[1] to bid_aux, aux[2] to a local Era (agent 1).
        let p0 = {
            let mut net = Net::new();
            let a0 = net.create_agent(non_era_symbol);
            let a1 = net.create_agent(Symbol::Era);
            net.connect(PortRef::AgentPort(a0, 0), PortRef::FreePort(bid_principal));
            net.connect(PortRef::AgentPort(a0, 1), PortRef::FreePort(bid_aux));
            net.connect(PortRef::AgentPort(a0, 2), PortRef::AgentPort(a1, 0));
            let mut free_port_index = HashMap::new();
            free_port_index.insert(bid_principal, PortRef::AgentPort(a0, 0));
            free_port_index.insert(bid_aux, PortRef::AgentPort(a0, 1));
            Partition {
                subnet: net,
                worker_id: 0,
                free_port_index,
                id_range: IdRange {
                    start: 0,
                    end: u32::MAX,
                },
                border_id_start: 0,
                border_id_end: u32::MAX,
            }
        };
        // Worker 1: ERA (agent 0) principal to bid_principal; local
        // Era (agent 1) principal to bid_aux — the "other side" of
        // the surviving auxiliary border.
        let p1 = {
            let mut net = Net::new();
            let a0 = net.create_agent(Symbol::Era);
            let a1 = net.create_agent(Symbol::Era);
            net.connect(PortRef::AgentPort(a0, 0), PortRef::FreePort(bid_principal));
            net.connect(PortRef::AgentPort(a1, 0), PortRef::FreePort(bid_aux));
            let mut free_port_index = HashMap::new();
            free_port_index.insert(bid_principal, PortRef::AgentPort(a0, 0));
            free_port_index.insert(bid_aux, PortRef::AgentPort(a1, 0));
            Partition {
                subnet: net,
                worker_id: 1,
                free_port_index,
                id_range: IdRange {
                    start: 0,
                    end: u32::MAX,
                },
                border_id_start: 0,
                border_id_end: u32::MAX,
            }
        };

        let mut borders = HashMap::new();
        borders.insert(
            bid_principal,
            BorderState {
                border_id: bid_principal,
                side_a: PortRef::AgentPort(0, 0),
                side_b: PortRef::AgentPort(0, 0),
                worker_a: 0,
                worker_b: 1,
                is_redex: true,
            },
        );
        borders.insert(
            bid_aux,
            BorderState {
                border_id: bid_aux,
                // non-ERA's aux port 1 on worker 0; local Era's principal on worker 1.
                side_a: PortRef::AgentPort(0, 1),
                side_b: PortRef::AgentPort(1, 0),
                worker_a: 0,
                worker_b: 1,
                is_redex: false,
            },
        );
        let mut active_redexes = HashSet::new();
        active_redexes.insert(bid_principal);
        let graph = BorderGraph {
            borders,
            worker_borders: vec![vec![bid_principal, bid_aux], vec![bid_principal, bid_aux]],
            active_redexes,
        };
        (graph, vec![p0, p1])
    }

    // -----------------------------------------------------------------
    // TEST-SPEC-0374 — CON-DUP + CON-ERA + DUP-ERA asymmetric rules
    // -----------------------------------------------------------------

    /// UT-0374-01 — CON-DUP emits one `CommutationBatch` per worker
    /// under DC-B5 2-phase AgentId allocation. Each batch requests 1
    /// Con + 1 Dup; concrete new-border finalization is deferred.
    #[test]
    fn resolve_con_dup_emits_pending_commutations_for_both_workers() {
        let (mut graph, partitions) =
            build_two_partition_asymmetric_fixture(Symbol::Con, Symbol::Dup, 0);
        let mut border_alloc = BorderIdAllocator::from_graph(&graph);
        let mut commutation_alloc = CommutationIdAllocator::new();

        let r = resolve_border_redex(
            &mut graph,
            &partitions,
            0,
            &mut border_alloc,
            &mut commutation_alloc,
        );

        assert_eq!(
            r.pending_commutations.len(),
            2,
            "DC-B5: one CommutationBatch per involved worker"
        );
        let w0 = r
            .pending_commutations
            .iter()
            .find(|pc| pc.worker == 0)
            .expect("worker 0 batch present");
        let w1 = r
            .pending_commutations
            .iter()
            .find(|pc| pc.worker == 1)
            .expect("worker 1 batch present");
        assert_eq!(
            w0.target_symbols.len(),
            2,
            "each worker mints 2 agents under balanced assignment"
        );
        assert_eq!(w1.target_symbols.len(), 2);
        // Balanced assignment: each worker mints exactly 1 CON + 1 DUP.
        let count_sym = |batch: &CommutationBatch, s: Symbol| -> usize {
            batch.target_symbols.iter().filter(|x| **x == s).count()
        };
        assert_eq!(count_sym(w0, Symbol::Con), 1);
        assert_eq!(count_sym(w0, Symbol::Dup), 1);
        assert_eq!(count_sym(w0, Symbol::Era), 0);
        assert_eq!(count_sym(w1, Symbol::Con), 1);
        assert_eq!(count_sym(w1, Symbol::Dup), 1);
        assert_eq!(count_sym(w1, Symbol::Era), 0);

        assert!(
            r.pending_new_borders.len() <= 4,
            "≤ 4 new cross-partition wires (2 external + 2 internal)"
        );
        assert!(
            r.new_borders.is_empty(),
            "DC-B5: concrete new borders NOT finalized at resolver stage"
        );
        assert_eq!(r.resolved_borders, vec![(0, 0, 1)]);
        assert!(
            !graph.borders.contains_key(&0),
            "original border removed in this round (DC-B5 step 1)"
        );
    }

    /// UT-0374-02 — `PendingPortRef::Pending` tokens correlate with
    /// emitted commutation batches: every pending token names a
    /// `commutation_id` present in `pending_commutations`, and its
    /// `agent_slot` is within the batch's `target_symbols` bounds.
    #[test]
    fn resolve_con_dup_pending_new_borders_carry_placeholder_refs() {
        let (mut graph, partitions) =
            build_two_partition_asymmetric_fixture(Symbol::Con, Symbol::Dup, 0);
        let mut border_alloc = BorderIdAllocator::from_graph(&graph);
        let mut commutation_alloc = CommutationIdAllocator::new();

        let r = resolve_border_redex(
            &mut graph,
            &partitions,
            0,
            &mut border_alloc,
            &mut commutation_alloc,
        );

        assert!(
            !r.pending_new_borders.is_empty(),
            "non-trivial fixture produces at least one cross wire"
        );
        for pnb in &r.pending_new_borders {
            let at_least_one_pending = matches!(pnb.side_a, PendingPortRef::Pending { .. })
                || matches!(pnb.side_b, PendingPortRef::Pending { .. });
            assert!(
                at_least_one_pending,
                "PendingNewBorder must carry at least one `Pending` side"
            );
            for side in [&pnb.side_a, &pnb.side_b] {
                if let PendingPortRef::Pending {
                    commutation_id,
                    agent_slot,
                    port_slot,
                } = side
                {
                    let batch = r
                        .pending_commutations
                        .iter()
                        .find(|pc| pc.commutation_id == *commutation_id)
                        .unwrap_or_else(|| {
                            panic!(
                                "pending token commutation_id {commutation_id} orphan — \
                                 no matching CommutationBatch",
                            )
                        });
                    assert!(
                        (*agent_slot as usize) < batch.target_symbols.len(),
                        "agent_slot {} out of range for batch with {} target symbols",
                        agent_slot,
                        batch.target_symbols.len()
                    );
                    assert!(
                        *port_slot <= 2,
                        "port_slot {port_slot} must be 0 (principal) or 1..=2 (aux)"
                    );
                }
            }
        }
    }

    /// UT-0374-03 — CON-ERA preserves an auxiliary-port border via
    /// DC-B6: the existing border id is reused in `pending_new_borders`
    /// and the graph entry survives. Only the principal border is
    /// removed.
    #[test]
    fn resolve_con_era_preserves_auxiliary_border_via_apply_deltas() {
        let (mut graph, partitions) = build_con_era_aux_border_fixture(Symbol::Con, 0, 7);
        let mut border_alloc = BorderIdAllocator::from_graph(&graph);
        let mut commutation_alloc = CommutationIdAllocator::new();

        let r = resolve_border_redex(
            &mut graph,
            &partitions,
            0,
            &mut border_alloc,
            &mut commutation_alloc,
        );

        assert_eq!(
            r.resolved_borders,
            vec![(0, 0, 1)],
            "only the principal border is resolved this round"
        );
        assert!(
            r.new_borders.is_empty(),
            "DC-B6: erasure never calls add_border_states"
        );
        assert_eq!(
            r.pending_commutations.len(),
            1,
            "one batch — only the non-ERA worker mints 2 new ERAs"
        );
        let batch = &r.pending_commutations[0];
        assert_eq!(batch.worker, 0, "non-ERA worker owns the mint");
        assert_eq!(batch.target_symbols, vec![Symbol::Era, Symbol::Era]);

        assert!(
            graph.borders.contains_key(&7),
            "DC-B6: auxiliary border 7 survives the erasure"
        );
        let found_update_for_7 = r
            .worker_deltas
            .iter()
            .flat_map(|(_, wd)| wd.border_deltas.iter())
            .any(|d| d.border_id == 7)
            || r.pending_new_borders.iter().any(|pnb| pnb.border_id == 7);
        assert!(
            found_update_for_7,
            "resolver must update (not recreate) border 7 via BorderDelta or PendingNewBorder"
        );
    }

    /// UT-0374-04 — DUP-ERA mirrors CON-ERA: same DC-B6 preservation,
    /// same batch shape, symmetric topology.
    #[test]
    fn resolve_dup_era_mirror_of_con_era_preserves_auxiliary_border() {
        let (mut graph, partitions) = build_con_era_aux_border_fixture(Symbol::Dup, 0, 7);
        let mut border_alloc = BorderIdAllocator::from_graph(&graph);
        let mut commutation_alloc = CommutationIdAllocator::new();

        let r = resolve_border_redex(
            &mut graph,
            &partitions,
            0,
            &mut border_alloc,
            &mut commutation_alloc,
        );

        assert_eq!(r.resolved_borders, vec![(0, 0, 1)]);
        assert!(r.new_borders.is_empty());
        assert_eq!(r.pending_commutations.len(), 1);
        let batch = &r.pending_commutations[0];
        assert_eq!(batch.worker, 0);
        assert_eq!(batch.target_symbols, vec![Symbol::Era, Symbol::Era]);
        assert!(graph.borders.contains_key(&7));
        let found_update_for_7 = r
            .worker_deltas
            .iter()
            .flat_map(|(_, wd)| wd.border_deltas.iter())
            .any(|d| d.border_id == 7)
            || r.pending_new_borders.iter().any(|pnb| pnb.border_id == 7);
        assert!(found_update_for_7);
    }

    /// UT-0374-05 — `BorderIdAllocator` monotonicity: two back-to-back
    /// CON-DUP resolutions sharing the same allocator produce a
    /// strictly-increasing, collision-free set of fresh border ids,
    /// all > `max(existing_border_ids)`.
    #[test]
    fn border_id_allocator_produces_unique_ids_across_back_to_back_con_dups() {
        // Two independent CON-DUP borders (ids 0 and 1) on disjoint
        // partition pairs — we need 4 partitions total. Build them
        // directly: workers 0/1 hold border 0; workers 2/3 hold border 1.
        let make_con_partition = |worker_id: WorkerId, border_id: u32| -> Partition {
            let mut net = Net::new();
            let a0 = net.create_agent(Symbol::Con);
            let a1 = net.create_agent(Symbol::Era);
            let a2 = net.create_agent(Symbol::Era);
            net.connect(PortRef::AgentPort(a0, 0), PortRef::FreePort(border_id));
            net.connect(PortRef::AgentPort(a0, 1), PortRef::AgentPort(a1, 0));
            net.connect(PortRef::AgentPort(a0, 2), PortRef::AgentPort(a2, 0));
            let mut free_port_index = HashMap::new();
            free_port_index.insert(border_id, PortRef::AgentPort(a0, 0));
            Partition {
                subnet: net,
                worker_id,
                free_port_index,
                id_range: IdRange {
                    start: 0,
                    end: u32::MAX,
                },
                border_id_start: 0,
                border_id_end: u32::MAX,
            }
        };
        let make_dup_partition = |worker_id: WorkerId, border_id: u32| -> Partition {
            let mut net = Net::new();
            let a0 = net.create_agent(Symbol::Dup);
            let a1 = net.create_agent(Symbol::Era);
            let a2 = net.create_agent(Symbol::Era);
            net.connect(PortRef::AgentPort(a0, 0), PortRef::FreePort(border_id));
            net.connect(PortRef::AgentPort(a0, 1), PortRef::AgentPort(a1, 0));
            net.connect(PortRef::AgentPort(a0, 2), PortRef::AgentPort(a2, 0));
            let mut free_port_index = HashMap::new();
            free_port_index.insert(border_id, PortRef::AgentPort(a0, 0));
            Partition {
                subnet: net,
                worker_id,
                free_port_index,
                id_range: IdRange {
                    start: 0,
                    end: u32::MAX,
                },
                border_id_start: 0,
                border_id_end: u32::MAX,
            }
        };
        let partitions = vec![
            make_con_partition(0, 0),
            make_dup_partition(1, 0),
            make_con_partition(2, 1),
            make_dup_partition(3, 1),
        ];

        let mut borders = HashMap::new();
        borders.insert(
            0,
            BorderState {
                border_id: 0,
                side_a: PortRef::AgentPort(0, 0),
                side_b: PortRef::AgentPort(0, 0),
                worker_a: 0,
                worker_b: 1,
                is_redex: true,
            },
        );
        borders.insert(
            1,
            BorderState {
                border_id: 1,
                side_a: PortRef::AgentPort(0, 0),
                side_b: PortRef::AgentPort(0, 0),
                worker_a: 2,
                worker_b: 3,
                is_redex: true,
            },
        );
        let mut active_redexes = HashSet::new();
        active_redexes.insert(0);
        active_redexes.insert(1);
        let mut graph = BorderGraph {
            borders,
            worker_borders: vec![vec![0], vec![0], vec![1], vec![1]],
            active_redexes,
        };

        let max_existing: u32 = 1;
        let mut border_alloc = BorderIdAllocator::from_graph(&graph);
        let mut commutation_alloc = CommutationIdAllocator::new();

        let r0 = resolve_border_redex(
            &mut graph,
            &partitions,
            0,
            &mut border_alloc,
            &mut commutation_alloc,
        );
        let r1 = resolve_border_redex(
            &mut graph,
            &partitions,
            1,
            &mut border_alloc,
            &mut commutation_alloc,
        );

        let all_new_ids: Vec<u32> = r0
            .pending_new_borders
            .iter()
            .chain(r1.pending_new_borders.iter())
            .map(|pnb| pnb.border_id)
            .collect();
        assert!(
            !all_new_ids.is_empty(),
            "back-to-back CON-DUP resolutions must produce at least one new border id each"
        );
        for id in &all_new_ids {
            assert!(
                *id > max_existing,
                "new border id {id} must be > max existing {max_existing}"
            );
        }
        let mut sorted = all_new_ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            all_new_ids.len(),
            "allocator produced duplicate ids across back-to-back resolutions"
        );
    }

    /// UT-0374-06 — the dispatcher reaches the correct asymmetric
    /// helper regardless of whether the symbol appears on side_a or
    /// side_b. CON-DUP variants emit `pending_commutations`; CON-ERA
    /// and DUP-ERA variants emit a single-worker batch of 2 Eras.
    /// Same-symbol pairs are covered by UT-0373-01..03; this test
    /// completes the 3×3 dispatch matrix for the 6 asymmetric arms.
    #[test]
    fn resolve_border_redex_dispatches_asymmetric_pairs_regardless_of_order() {
        let cases: [(Symbol, Symbol); 6] = [
            (Symbol::Con, Symbol::Dup),
            (Symbol::Dup, Symbol::Con),
            (Symbol::Con, Symbol::Era),
            (Symbol::Era, Symbol::Con),
            (Symbol::Dup, Symbol::Era),
            (Symbol::Era, Symbol::Dup),
        ];
        for (sym_a, sym_b) in cases {
            let (mut graph, partitions) = build_two_partition_asymmetric_fixture(sym_a, sym_b, 0);
            let mut border_alloc = BorderIdAllocator::from_graph(&graph);
            let mut commutation_alloc = CommutationIdAllocator::new();

            let r = resolve_border_redex(
                &mut graph,
                &partitions,
                0,
                &mut border_alloc,
                &mut commutation_alloc,
            );

            assert_eq!(
                r.resolved_borders,
                vec![(0, 0, 1)],
                "dispatch arm for ({sym_a:?}, {sym_b:?}) must emit the DC-B7 triple"
            );
            assert!(
                !graph.borders.contains_key(&0),
                "border removed regardless of asymmetric orientation"
            );
            let is_comm = matches!(
                (sym_a, sym_b),
                (Symbol::Con, Symbol::Dup) | (Symbol::Dup, Symbol::Con)
            );
            if is_comm {
                assert_eq!(
                    r.pending_commutations.len(),
                    2,
                    "CON-DUP variant ({sym_a:?}, {sym_b:?}) — one batch per worker"
                );
            } else {
                assert_eq!(
                    r.pending_commutations.len(),
                    1,
                    "CON-ERA / DUP-ERA variant ({sym_a:?}, {sym_b:?}) — batch only on non-ERA worker"
                );
                let batch = &r.pending_commutations[0];
                assert_eq!(batch.target_symbols, vec![Symbol::Era, Symbol::Era]);
            }
        }
    }

    // -----------------------------------------------------------------
    // TEST-SPEC-0375 — RoundStartDispatch + package_resolutions
    // -----------------------------------------------------------------

    #[test]
    fn package_resolutions_empty_produces_one_default_per_worker() {
        // UT-0375-01: R23 — coordinator still addresses every worker
        // when no borders were resolved this round; each worker gets
        // a default-empty dispatch so its BSP heartbeat advances.
        let result = package_resolutions(Vec::new(), 3);
        assert_eq!(result.len(), 3);
        for (i, (wid, dispatch)) in result.iter().enumerate() {
            assert_eq!(*wid, i as WorkerId);
            assert!(dispatch.border_deltas.is_empty());
            assert!(dispatch.local_reconnections.is_empty());
            assert!(dispatch.resolved_borders.is_empty());
            assert!(dispatch.new_borders.is_empty());
            assert!(dispatch.pending_commutations.is_empty());
        }
    }

    #[test]
    fn package_resolutions_fans_worker_deltas_to_correct_dispatch() {
        // UT-0375-02: DC-B3 per-worker fan of border_deltas +
        // local_reconnections; the resolved_borders triple fans to
        // both sides; untouched workers stay default.
        let resolution = BorderResolution {
            worker_deltas: vec![
                (
                    0,
                    WorkerDeltas {
                        border_deltas: vec![BorderDelta {
                            border_id: 0,
                            new_target: PortRef::AgentPort(7, 0),
                        }],
                        local_reconnections: vec![(
                            PortRef::AgentPort(1, 1),
                            PortRef::AgentPort(2, 1),
                        )],
                    },
                ),
                (
                    1,
                    WorkerDeltas {
                        border_deltas: vec![BorderDelta {
                            border_id: 0,
                            new_target: PortRef::AgentPort(9, 0),
                        }],
                        local_reconnections: Vec::new(),
                    },
                ),
            ],
            resolved_borders: vec![(0, 0, 1)],
            ..Default::default()
        };
        let result = package_resolutions(vec![resolution], 3);

        assert_eq!(result.len(), 3);

        assert_eq!(result[0].0, 0);
        assert_eq!(result[0].1.border_deltas.len(), 1);
        assert_eq!(result[0].1.border_deltas[0].border_id, 0);
        assert_eq!(
            result[0].1.border_deltas[0].new_target,
            PortRef::AgentPort(7, 0)
        );
        assert_eq!(
            result[0].1.local_reconnections,
            vec![(PortRef::AgentPort(1, 1), PortRef::AgentPort(2, 1))]
        );
        assert_eq!(result[0].1.resolved_borders, vec![0]);
        assert!(result[0].1.new_borders.is_empty());
        assert!(result[0].1.pending_commutations.is_empty());

        assert_eq!(result[1].0, 1);
        assert_eq!(result[1].1.border_deltas.len(), 1);
        assert_eq!(
            result[1].1.border_deltas[0].new_target,
            PortRef::AgentPort(9, 0)
        );
        assert!(result[1].1.local_reconnections.is_empty());
        assert_eq!(result[1].1.resolved_borders, vec![0]);

        assert_eq!(result[2].0, 2);
        assert!(result[2].1.border_deltas.is_empty());
        assert!(result[2].1.local_reconnections.is_empty());
        assert!(result[2].1.resolved_borders.is_empty());
        assert!(result[2].1.new_borders.is_empty());
        assert!(result[2].1.pending_commutations.is_empty());
    }

    #[test]
    fn package_resolutions_fans_resolved_border_triples_to_both_workers() {
        // UT-0375-03: DC-B7 triple fan — each worker whose side was
        // consumed by a border redex receives the bid in its
        // resolved_borders bucket so it can clear the FreePort entry.
        let resolution = BorderResolution {
            resolved_borders: vec![(0, 0, 1), (5, 1, 2)],
            ..Default::default()
        };
        let result = package_resolutions(vec![resolution], 3);

        assert_eq!(result[0].1.resolved_borders, vec![0]);
        assert_eq!(result[1].1.resolved_borders, vec![0, 5]);
        assert_eq!(result[2].1.resolved_borders, vec![5]);
    }

    #[test]
    fn package_resolutions_fans_pending_commutations_to_owning_worker() {
        // UT-0375-04: DC-B5 per-worker fan — each CommutationBatch
        // travels to exactly ONE worker (the batch.worker field); no
        // duplication across workers.
        let resolution = BorderResolution {
            pending_commutations: vec![
                CommutationBatch {
                    commutation_id: 1,
                    worker: 0,
                    target_symbols: vec![Symbol::Con, Symbol::Dup],
                    local_wiring: Vec::new(),
                },
                CommutationBatch {
                    commutation_id: 2,
                    worker: 1,
                    target_symbols: vec![Symbol::Con, Symbol::Dup],
                    local_wiring: Vec::new(),
                },
            ],
            ..Default::default()
        };
        let result = package_resolutions(vec![resolution], 3);

        assert_eq!(result[0].1.pending_commutations.len(), 1);
        assert_eq!(result[0].1.pending_commutations[0].commutation_id, 1);
        assert_eq!(result[1].1.pending_commutations.len(), 1);
        assert_eq!(result[1].1.pending_commutations[0].commutation_id, 2);
        assert!(result[2].1.pending_commutations.is_empty());
    }

    #[test]
    fn package_resolutions_is_deterministic_and_ordered_by_worker_id() {
        // UT-0375-05: byte-identical output across two invocations
        // with cloned inputs; outer Vec sorted 0..num_workers
        // regardless of resolution insertion order. Relies on
        // RoundStartDispatch deriving PartialEq + Eq.
        let r_a = BorderResolution {
            worker_deltas: vec![
                (
                    2,
                    WorkerDeltas {
                        border_deltas: vec![BorderDelta {
                            border_id: 3,
                            new_target: PortRef::AgentPort(11, 0),
                        }],
                        local_reconnections: Vec::new(),
                    },
                ),
                (
                    0,
                    WorkerDeltas {
                        border_deltas: vec![BorderDelta {
                            border_id: 4,
                            new_target: PortRef::AgentPort(12, 0),
                        }],
                        local_reconnections: Vec::new(),
                    },
                ),
            ],
            ..Default::default()
        };
        let r_b = BorderResolution {
            worker_deltas: vec![(
                1,
                WorkerDeltas {
                    border_deltas: vec![BorderDelta {
                        border_id: 5,
                        new_target: PortRef::AgentPort(13, 0),
                    }],
                    local_reconnections: Vec::new(),
                },
            )],
            ..Default::default()
        };
        let input = vec![r_a, r_b];

        let out1 = package_resolutions(input.clone(), 3);
        let out2 = package_resolutions(input.clone(), 3);

        assert_eq!(out1, out2);
        assert_eq!(out1[0].0, 0);
        assert_eq!(out1[1].0, 1);
        assert_eq!(out1[2].0, 2);
    }

    // -----------------------------------------------------------------
    // TEST-SPEC-0376 — end-to-end integration: resolve_border_redex +
    // package_resolutions on 2-partition fixtures, one per IC rule.
    // Reuses the existing `build_*_fixture` helpers (TASK-0373/0374)
    // because the bundle-level "tie it together" contract only adds
    // composition coverage — fixture shapes are identical.
    // -----------------------------------------------------------------

    #[test]
    fn con_con_border_redex_end_to_end_resolves_and_packages() {
        // UT-0376-01: R13 + R14 (Anni cross) + R15 parts 1-2 +
        // DC-B3 + DC-B7. Resolver returns the expected per-rule
        // shape; the border vanishes from the graph; the packaged
        // dispatch fans `resolved_borders` to both sides.
        let (mut graph, partitions) = build_two_partition_same_symbol_fixture(Symbol::Con, 0);
        assert_eq!(graph.detect_border_redexes().len(), 1);

        let mut border_alloc = BorderIdAllocator::from_graph(&graph);
        let mut commutation_alloc = CommutationIdAllocator::new();
        let r = resolve_border_redex(
            &mut graph,
            &partitions,
            0,
            &mut border_alloc,
            &mut commutation_alloc,
        );

        assert_eq!(r.resolved_borders, vec![(0, 0, 1)]);
        assert!(r.new_borders.is_empty());
        assert!(r.pending_commutations.is_empty());
        assert!(!graph.borders.contains_key(&0));
        assert!(!graph.active_redexes.contains(&0));

        let packaged = package_resolutions(vec![r], 2);
        assert_eq!(packaged.len(), 2);
        assert_eq!(packaged[0].0, 0);
        assert_eq!(packaged[0].1.resolved_borders, vec![0]);
        assert_eq!(packaged[1].0, 1);
        assert_eq!(packaged[1].1.resolved_borders, vec![0]);
        // Resolver convention: same-symbol rule bodies park both aux
        // reconnections in worker_a's bucket (UT-0373-01). Worker 1's
        // dispatch stays empty of reconnections after packaging.
        assert_eq!(packaged[0].1.local_reconnections.len(), 2);
        assert!(packaged[1].1.local_reconnections.is_empty());
    }

    #[test]
    fn dup_dup_border_redex_end_to_end_resolves_and_packages() {
        // UT-0376-02: DUP-DUP parallel pattern; same composition
        // contract as UT-0376-01.
        let (mut graph, partitions) = build_two_partition_same_symbol_fixture(Symbol::Dup, 0);
        assert_eq!(graph.detect_border_redexes().len(), 1);

        let mut border_alloc = BorderIdAllocator::from_graph(&graph);
        let mut commutation_alloc = CommutationIdAllocator::new();
        let r = resolve_border_redex(
            &mut graph,
            &partitions,
            0,
            &mut border_alloc,
            &mut commutation_alloc,
        );

        assert_eq!(r.resolved_borders, vec![(0, 0, 1)]);
        assert!(r.new_borders.is_empty());
        assert!(r.pending_commutations.is_empty());
        assert!(!graph.borders.contains_key(&0));

        let packaged = package_resolutions(vec![r], 2);
        assert_eq!(packaged.len(), 2);
        assert_eq!(packaged[0].1.resolved_borders, vec![0]);
        assert_eq!(packaged[1].1.resolved_borders, vec![0]);
        assert_eq!(packaged[0].1.local_reconnections.len(), 2);
        assert!(packaged[1].1.local_reconnections.is_empty());
    }

    #[test]
    fn era_era_border_redex_end_to_end_resolves_and_packages() {
        // UT-0376-03: Void rule; zero auxiliary, shortest path.
        let (mut graph, partitions) = build_two_partition_era_era_fixture(0);
        assert_eq!(graph.detect_border_redexes().len(), 1);

        let mut border_alloc = BorderIdAllocator::from_graph(&graph);
        let mut commutation_alloc = CommutationIdAllocator::new();
        let r = resolve_border_redex(
            &mut graph,
            &partitions,
            0,
            &mut border_alloc,
            &mut commutation_alloc,
        );

        assert_eq!(r.resolved_borders, vec![(0, 0, 1)]);
        assert!(r.new_borders.is_empty());
        assert!(r.pending_commutations.is_empty());
        assert!(r.worker_deltas.is_empty());
        assert!(!graph.borders.contains_key(&0));

        let packaged = package_resolutions(vec![r], 2);
        assert_eq!(packaged.len(), 2);
        for (_, dispatch) in &packaged {
            assert_eq!(dispatch.resolved_borders, vec![0]);
            assert!(dispatch.border_deltas.is_empty());
            assert!(dispatch.local_reconnections.is_empty());
            assert!(dispatch.new_borders.is_empty());
            assert!(dispatch.pending_commutations.is_empty());
        }
    }

    #[test]
    fn con_dup_border_redex_end_to_end_emits_pending_commutations() {
        // UT-0376-04: DC-B5 2-phase commutation. Resolver emits one
        // batch per worker; `package_resolutions` fans each batch to
        // its addressed worker.
        let (mut graph, partitions) =
            build_two_partition_asymmetric_fixture(Symbol::Con, Symbol::Dup, 0);
        assert_eq!(graph.detect_border_redexes().len(), 1);

        let mut border_alloc = BorderIdAllocator::from_graph(&graph);
        let mut commutation_alloc = CommutationIdAllocator::new();
        let r = resolve_border_redex(
            &mut graph,
            &partitions,
            0,
            &mut border_alloc,
            &mut commutation_alloc,
        );

        assert_eq!(r.resolved_borders, vec![(0, 0, 1)]);
        assert!(r.new_borders.is_empty());
        assert_eq!(r.pending_commutations.len(), 2);
        assert!(r.pending_new_borders.len() <= 4);
        assert!(!graph.borders.contains_key(&0));

        let packaged = package_resolutions(vec![r], 2);
        assert_eq!(packaged.len(), 2);
        assert_eq!(packaged[0].1.pending_commutations.len(), 1);
        assert_eq!(packaged[0].1.pending_commutations[0].worker, 0);
        assert_eq!(packaged[1].1.pending_commutations.len(), 1);
        assert_eq!(packaged[1].1.pending_commutations[0].worker, 1);
        assert_eq!(packaged[0].1.resolved_borders, vec![0]);
        assert_eq!(packaged[1].1.resolved_borders, vec![0]);
    }

    #[test]
    fn con_era_border_redex_end_to_end_preserves_auxiliary_border() {
        // UT-0376-05: DC-B6 auxiliary-border preservation. Principal
        // redex on border 0 vanishes; auxiliary border 7 survives
        // with an endpoint update threaded through either
        // `worker_deltas.border_deltas` or `pending_new_borders`.
        let (mut graph, partitions) = build_con_era_aux_border_fixture(Symbol::Con, 0, 7);
        assert_eq!(graph.detect_border_redexes().len(), 1);

        let mut border_alloc = BorderIdAllocator::from_graph(&graph);
        let mut commutation_alloc = CommutationIdAllocator::new();
        let r = resolve_border_redex(
            &mut graph,
            &partitions,
            0,
            &mut border_alloc,
            &mut commutation_alloc,
        );

        assert_eq!(r.resolved_borders, vec![(0, 0, 1)]);
        assert!(r.new_borders.is_empty());
        assert!(!graph.borders.contains_key(&0));
        assert!(
            graph.borders.contains_key(&7),
            "DC-B6: auxiliary border 7 must survive the principal redex"
        );

        let found_update_for_7 = r
            .worker_deltas
            .iter()
            .flat_map(|(_, wd)| wd.border_deltas.iter())
            .any(|d| d.border_id == 7)
            || r.pending_new_borders.iter().any(|pnb| pnb.border_id == 7);
        assert!(
            found_update_for_7,
            "auxiliary border 7 endpoint must be updated via BorderDelta or PendingNewBorder"
        );

        let packaged = package_resolutions(vec![r], 2);
        assert_eq!(packaged.len(), 2);
        assert!(packaged
            .iter()
            .any(|(_, d)| d.resolved_borders.contains(&0)));
    }

    #[test]
    fn dup_era_border_redex_end_to_end_preserves_auxiliary_border() {
        // UT-0376-06: mirror of UT-0376-05 with Dup as the non-ERA.
        let (mut graph, partitions) = build_con_era_aux_border_fixture(Symbol::Dup, 0, 7);
        assert_eq!(graph.detect_border_redexes().len(), 1);

        let mut border_alloc = BorderIdAllocator::from_graph(&graph);
        let mut commutation_alloc = CommutationIdAllocator::new();
        let r = resolve_border_redex(
            &mut graph,
            &partitions,
            0,
            &mut border_alloc,
            &mut commutation_alloc,
        );

        assert_eq!(r.resolved_borders, vec![(0, 0, 1)]);
        assert!(!graph.borders.contains_key(&0));
        assert!(graph.borders.contains_key(&7));

        let found_update_for_7 = r
            .worker_deltas
            .iter()
            .flat_map(|(_, wd)| wd.border_deltas.iter())
            .any(|d| d.border_id == 7)
            || r.pending_new_borders.iter().any(|pnb| pnb.border_id == 7);
        assert!(found_update_for_7);

        let packaged = package_resolutions(vec![r], 2);
        assert_eq!(packaged.len(), 2);
    }

    #[test]
    fn resolve_border_redex_on_absent_border_panics_per_dc_b2() {
        // UT-0376-07: invariant check. Resolving a border that is no
        // longer in the graph must panic (DC-B2 preference over
        // silent default) — silent no-op would mask a coordinator
        // cache desync bug.
        use std::panic::{catch_unwind, AssertUnwindSafe};

        let (mut graph, partitions) = build_two_partition_same_symbol_fixture(Symbol::Con, 0);
        let mut border_alloc = BorderIdAllocator::from_graph(&graph);
        let mut commutation_alloc = CommutationIdAllocator::new();

        let _ = resolve_border_redex(
            &mut graph,
            &partitions,
            0,
            &mut border_alloc,
            &mut commutation_alloc,
        );
        assert!(!graph.borders.contains_key(&0));

        let outcome = catch_unwind(AssertUnwindSafe(|| {
            resolve_border_redex(
                &mut graph,
                &partitions,
                0,
                &mut border_alloc,
                &mut commutation_alloc,
            )
        }));
        assert!(
            outcome.is_err(),
            "second resolve on absent border must panic, not silently default"
        );
        let payload = outcome.err().unwrap();
        let msg: String = if let Some(s) = payload.downcast_ref::<&'static str>() {
            (*s).to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            String::new()
        };
        assert!(
            msg.contains("border 0 not present") || msg.contains("agent missing for border 0"),
            "panic payload must diagnose absent border; got: {msg:?}"
        );
    }

    #[test]
    fn border_resolver_pure_core_no_forbidden_imports() {
        // TEST-SPEC-0377 T1: programmatic R19 pure-core guard.
        // DC-B8 factoring — scan logic lives in the shared helper.
        // DC-B9 cardinality canary — narrowing the list without a
        // new spec-critic verdict fires this assertion first.
        let src = include_str!("border_resolver.rs");

        assert_eq!(
            crate::merge::internal::pure_core_guard::FORBIDDEN_USE_PREFIXES.len(),
            5,
            "DC-B9: forbidden-prefix list must contain exactly 5 entries; \
             see merge/internal/pure_core_guard.rs for the canonical list. \
             Adjust this assertion ONLY if DC-B9 is formally revised by a \
             new spec-critic verdict"
        );
        for prefix in [
            "use tokio",
            "use async_trait",
            "use crate::protocol",
            "use crate::coordinator",
            "use crate::worker",
        ] {
            assert!(
                crate::merge::internal::pure_core_guard::FORBIDDEN_USE_PREFIXES.contains(&prefix),
                "DC-B9: expected forbidden prefix {prefix:?} in the guard's \
                 prefix list — drift between test and helper means the \
                 guard is silently weaker than spec",
            );
        }

        crate::merge::internal::pure_core_guard::assert_no_forbidden_imports(
            src,
            "border_resolver.rs",
        );
    }

    #[test]
    fn border_resolver_module_doc_cites_spec_sections_and_dc_rulings() {
        const SRC: &str = include_str!("border_resolver.rs");
        let doc_block: String = SRC
            .lines()
            .take_while(|l| {
                let t = l.trim_start();
                t.is_empty() || t.starts_with("//!")
            })
            .collect::<Vec<_>>()
            .join("\n");
        for needle in [
            "SPEC-19 §3.2",
            "R13",
            "R14",
            "R15",
            "§3.3",
            "2.26",
            "R19",
            "pure-core",
            "DC-B1",
            "DC-B2",
            "DC-B4",
            "DC-B5",
            "DC-B6",
            "tokio",
            "protocol",
        ] {
            assert!(
                doc_block.contains(needle),
                "border_resolver.rs //! block missing {needle:?}",
            );
        }
    }
}
