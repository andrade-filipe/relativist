//! Helper functions for merge and grid cycle (SPEC-05).
//!
//! - `is_principal_pair`: checks if both ports are principal (for border redex counting)
//! - `rebuild_free_port_index`: lazy reconstruction of the FreePort index after local reduction
//! - `drain_stale_redexes`: removes stale entries from the redex queue

use std::collections::HashMap;
use std::collections::VecDeque;

use crate::merge::BorderDelta;
use crate::net::{total_ports, Net, PortRef, DISCONNECTED};
use crate::partition::Partition;

/// Returns true if both ports are principal ports (port index 0).
///
/// Used during merge to count border redexes for metrics (SPEC-05, R12-R13).
/// The actual redex detection is performed by Net::connect (SPEC-02, R13);
/// this function only identifies principal-principal pairs for counting.
pub(crate) fn is_principal_pair(a: PortRef, b: PortRef) -> bool {
    matches!((a, b), (PortRef::AgentPort(_, 0), PortRef::AgentPort(_, 0)))
}

/// Rebuilds the free_port_index for a partition by scanning the port array.
///
/// After local reduction, the connections to FreePort (Boundary) sentinels
/// may have changed (reconnection, erasure/transfer, CON-DUP inheritance).
/// This function produces a fresh index reflecting the current state.
///
/// Uses `border_id_start` and `border_id_end` (SPEC-04, R15a) to discriminate:
/// - FreePort(id) with border_id_start <= id < border_id_end: boundary (included)
/// - FreePort(id) with id < border_id_start: Lafont FreePort (excluded)
/// - FreePort(u32::MAX): DISCONNECTED sentinel (excluded)
///
/// Complexity: O(A_i * PORTS_PER_SLOT) where A_i is the number of live agents.
///
/// SPEC-05, R20-R23; SPEC-04, Section 4.6, R15a.
pub fn rebuild_free_port_index(
    subnet: &Net,
    border_id_start: u32,
    border_id_end: u32,
) -> HashMap<u32, PortRef> {
    let mut index = HashMap::new();

    // Phase 1: scan agent ports for boundary FreePorts still held by live agents
    for (i, slot) in subnet.agents.iter().enumerate() {
        if let Some(agent) = slot {
            let num_ports = total_ports(agent.symbol);
            for p in 0..num_ports {
                let target = subnet.get_target(PortRef::AgentPort(i as u32, p));
                if let PortRef::FreePort(bid) = target {
                    // Include only boundary FreePorts (not Lafont or DISCONNECTED)
                    if bid >= border_id_start && bid < border_id_end && bid != u32::MAX {
                        index.insert(bid, PortRef::AgentPort(agent.id, p));
                    }
                }
            }
        }
    }

    // Phase 2: recover border references lost in FreePort-to-FreePort links.
    //
    // When a local reduction links a border FreePort(B) to a Lafont FreePort(N),
    // connect(FreePort, FreePort) is a no-op in the port array. The redirect
    // is recorded in freeport_redirects so we can recover B's destination here.
    for (&fid, &target) in &subnet.freeport_redirects {
        if fid >= border_id_start && fid < border_id_end && fid != u32::MAX {
            // Only add if not already found by Phase 1 (agent port takes priority)
            index.entry(fid).or_insert(target);
        }
    }

    index
}

/// SPEC-19 §3.1 R1: scans `partition.free_port_index` and returns `true`
/// if at least one local border endpoint is a principal port
/// (`AgentPort(_, 0)`).
///
/// IC concept: each agent has one **principal port** (port index 0) and
/// `arity` auxiliary ports. A redex (active pair) only fires when two
/// principal ports meet. So a border endpoint that is principal is
/// "potentially active" — it could form a border redex once paired with
/// the remote side. When **no** worker has any principal-port border
/// endpoint after local reduction, no border redex can fire this round
/// no matter how merge runs (R5, T4 strong confluence): the coordinator
/// is free to skip merge entirely (R3 + R6 under strict BSP).
///
/// Pure / O(|free_port_index|). No allocation. The helper does not scan
/// the underlying `Net`; it consults the index already maintained by
/// `rebuild_free_port_index` (R1 ordering: this MUST be called *after*
/// `rebuild_free_port_index`).
pub(crate) fn compute_border_activity(partition: &Partition) -> bool {
    partition
        .free_port_index
        .values()
        .any(|p| matches!(p, PortRef::AgentPort(_, 0)))
}

/// Traverses the redex queue and discards all stale entries (SPEC-05, Section 4.4).
///
/// A stale redex is one where either agent has been consumed, or the
/// principal port connection has changed since the redex was inserted.
/// After this function, the queue contains only valid redexes.
///
/// Complexity: O(Q) where Q is the size of the redex queue.
pub fn drain_stale_redexes(net: &mut Net) {
    let mut valid = VecDeque::new();
    while let Some((a, b)) = net.redex_queue.pop_front() {
        if net.is_valid_redex(a, b) {
            valid.push_back((a, b));
        }
    }
    net.redex_queue = valid;
}

/// SPEC-19 R23 (TASK-0381): apply the three-part payload of
/// `Message::RoundStart` to a worker's stored `Partition`. All three
/// sub-operations mutate both the underlying `Net` (so the post-apply
/// subnet is a well-formed partition ready for `reduce_all`) and the
/// `free_port_index` (so any caller that skips rebuild still observes
/// the new topology).
///
/// **Semantics (per R23):**
/// 1. **`border_deltas` — reconnection.** For each
///    `BorderDelta { border_id, new_target }`, the local port currently
///    bound to `FreePort(border_id)` is disconnected and `new_target`
///    becomes the new local side of the border. `new_target` is a local
///    port within this worker's subnet (an `AgentPort` the coordinator
///    computed via border-resolver book-keeping). When `new_target ==
///    crate::net::DISCONNECTED` (DC-C6 sentinel), the border is simply
///    dropped — no new wire is created and the index entry is removed.
/// 2. **`resolved_borders` — removal.** For each `border_id`, the local
///    port currently bound to `FreePort(border_id)` is disconnected and
///    the index entry is removed. This encodes coordinator-side
///    annihilation / void resolution.
/// 3. **`new_borders` — insertion.** For each `(border_id, port_ref)`,
///    a fresh `FreePort(border_id) ↔ port_ref` wire is established and
///    the index is populated. This encodes CON-DUP expansion that gave
///    the worker a new cross-partition wire.
///
/// Pure (no I/O, no async). No reduction: `reduce_all` is the caller's
/// next step (R24.2). Idempotent against a well-formed input.
pub(crate) fn apply_border_deltas_to_partition(
    partition: &mut Partition,
    border_deltas: &[BorderDelta],
    resolved_borders: &[u32],
    new_borders: &[(u32, PortRef)],
) {
    // [1] Re-point existing borders. Disconnect the old local-side port,
    // then connect the new local-side port to FreePort(border_id).
    for delta in border_deltas {
        if let Some(&old_target) = partition.free_port_index.get(&delta.border_id) {
            if matches!(old_target, PortRef::AgentPort(_, _)) {
                partition.subnet.disconnect(old_target);
            }
        }
        if delta.new_target == DISCONNECTED {
            // DC-C6: the remote side is gone; drop the border entirely.
            partition.free_port_index.remove(&delta.border_id);
        } else {
            partition
                .subnet
                .connect(delta.new_target, PortRef::FreePort(delta.border_id));
            partition
                .free_port_index
                .insert(delta.border_id, delta.new_target);
        }
    }

    // [2] Remove resolved borders. Disconnect the local side, drop index.
    for bid in resolved_borders {
        if let Some(&old_target) = partition.free_port_index.get(bid) {
            if matches!(old_target, PortRef::AgentPort(_, _)) {
                partition.subnet.disconnect(old_target);
            }
        }
        partition.free_port_index.remove(bid);
    }

    // [3] Insert new borders. CON-DUP expansion at the coordinator
    // created a new cross-partition wire whose local end is `target`.
    //
    // D-005 F-H7 (2026-04-24): debug-assert the `target` AgentId belongs
    // to THIS partition's arena, catching cross-worker mis-sends at dev
    // time. A `PortRef::FreePort(_)` target is always allowed (it models
    // Lafont/dangling sentinels or intra-worker sentinel handoffs).
    //
    // D-005 F-H8 tail (2026-04-24): when a promoted border arrives with
    // an id beyond the partition's current `border_id_end`, extend the
    // end so future `rebuild_free_port_index` passes include it (the
    // worker's rebuild filter is `bid < border_id_end`, and a minted
    // border id allocated post-split would otherwise be dropped,
    // stranding the associated principal-port wire at DISCONNECTED
    // after merge's boundary restoration).
    for (bid, target) in new_borders {
        debug_assert!(
            match target {
                PortRef::AgentPort(id, _) => {
                    let id = *id;
                    id >= partition.id_range.start && id < partition.id_range.end
                }
                PortRef::FreePort(_) => true,
            },
            "apply_border_deltas_to_partition: new_borders target {:?} is outside worker \
             {}'s id_range {:?} for border_id {} (F-H7 cross-arena injection)",
            target,
            partition.worker_id,
            partition.id_range,
            bid
        );
        partition.subnet.connect(*target, PortRef::FreePort(*bid));
        partition.free_port_index.insert(*bid, *target);
        if *bid != u32::MAX && *bid >= partition.border_id_end {
            partition.border_id_end = bid.saturating_add(1);
        }
    }
}

/// SPEC-19 R25 (TASK-0382): diff the previously-reported border-port
/// state against the freshly rebuilt `free_port_index` and emit only
/// the changed entries as `BorderDelta`s for inclusion in the round's
/// `Message::RoundResult`.
///
/// Three cases handled:
/// - **Changed endpoint:** `previous[K] != current[K]` → delta with
///   the new target.
/// - **Newly-created border:** `K ∈ current ∧ K ∉ previous` → delta
///   with the new target (first report after CON-DUP expansion gave
///   the worker a new `FreePort(K)`).
/// - **Disconnected border:** `K ∈ previous ∧ K ∉ current` → delta
///   with sentinel `crate::net::DISCONNECTED` (DC-C6 option (c)
///   locked-in 2026-04-17 — reuses the `FreePort(u32::MAX)` sentinel
///   that SPEC-18 §4.3 already collapses to a single wire byte).
///
/// Pure / O(|previous| + |current|). No allocation beyond the result
/// vector. No ordering guarantee (caller sorts if needed).
pub(crate) fn compute_outgoing_deltas(
    previous: &HashMap<u32, PortRef>,
    current: &HashMap<u32, PortRef>,
) -> Vec<BorderDelta> {
    let mut out = Vec::new();

    for (&bid, &new_target) in current {
        match previous.get(&bid) {
            Some(prev) if *prev == new_target => {}
            _ => out.push(BorderDelta {
                border_id: bid,
                new_target,
            }),
        }
    }

    for &bid in previous.keys() {
        if !current.contains_key(&bid) {
            out.push(BorderDelta {
                border_id: bid,
                new_target: DISCONNECTED,
            });
        }
    }

    out
}

// ---------------------------------------------------------------------------
// MF-001: generate_and_partition_chunked_with_delta (SPEC-21 R37f call-site)
// ---------------------------------------------------------------------------

/// Production call-site for SPEC-21 R37f: run the streaming partition pipeline
/// and extend the coordinator's `BorderGraph` with the accumulated borders when
/// `delta_mode && streaming_active`.
///
/// This wrapper lives in `merge/` (above `partition/`) so it can import both
/// `BorderGraph` (from `merge::border_graph`) and
/// `generate_and_partition_chunked` (from `partition::streaming`). The `partition`
/// crate cannot import `merge` types (inviolable dependency direction per SPEC-13).
///
/// # Call-site discipline (R37f MUST)
///
/// When `delta_mode && streaming_active`, the coordinator MUST call this wrapper
/// (not the bare `generate_and_partition_chunked`) so the `BorderGraph` stays fresh
/// before the next `AssignPartition` dispatch. Under any other combination the
/// `border_graph` argument is `None` and the function behaves identically to
/// `generate_and_partition_chunked`.
///
/// # Arguments
///
/// - `stream`: the agent batch iterator (consumed by the partition pipeline).
/// - `num_workers`: number of workers in the current round.
/// - `strategy`: streaming partition strategy.
/// - `border_graph`: if `Some(&mut bg)` AND `delta_mode && streaming_active`, the
///   final `ChunkedPartitionResult.borders` map is applied to `bg` via
///   `extend_with_chunk_borders` before returning. Pass `None` for non-delta or
///   non-streaming runs.
/// - `delta_mode`: flag indicating a delta-mode round is active (SPEC-19).
/// - `streaming_active`: flag indicating chunked dispatch is active (SPEC-21 R37b).
pub fn generate_and_partition_chunked_with_delta(
    stream: Box<dyn Iterator<Item = crate::partition::streaming::AgentBatch>>,
    num_workers: u32,
    strategy: &mut dyn crate::partition::streaming::StreamingPartitionStrategy,
    border_graph: Option<&mut crate::merge::BorderGraph>,
    delta_mode: bool,
    streaming_active: bool,
) -> Result<crate::partition::streaming::ChunkedPartitionResult, crate::error::PartitionError> {
    generate_and_partition_chunked_with_delta_and_config(
        stream,
        num_workers,
        strategy,
        border_graph,
        delta_mode,
        streaming_active,
        u32::MAX,
        u32::MAX,
    )
}

/// QA-D010-009: configuration-aware variant of
/// [`generate_and_partition_chunked_with_delta`] that propagates
/// `chunk_size` and `max_pending_lifetime` from `GridConfig` into the
/// streaming pipeline. Coordinator/run-grid call-sites should use this
/// to enforce the R37g malformed-stream budget.
///
/// `chunk_size == u32::MAX` selects the R26 short-circuit path.
/// `max_pending_lifetime == u32::MAX` disables the lifetime check (legacy).
#[allow(clippy::too_many_arguments)]
pub fn generate_and_partition_chunked_with_delta_and_config(
    stream: Box<dyn Iterator<Item = crate::partition::streaming::AgentBatch>>,
    num_workers: u32,
    strategy: &mut dyn crate::partition::streaming::StreamingPartitionStrategy,
    border_graph: Option<&mut crate::merge::BorderGraph>,
    delta_mode: bool,
    streaming_active: bool,
    chunk_size: u32,
    max_pending_lifetime: u32,
) -> Result<crate::partition::streaming::ChunkedPartitionResult, crate::error::PartitionError> {
    use crate::partition::streaming::generate_and_partition_chunked_with_chunk_size_and_lifetime;

    let result = generate_and_partition_chunked_with_chunk_size_and_lifetime(
        stream,
        num_workers,
        strategy,
        chunk_size,
        max_pending_lifetime,
    )?;

    // SPEC-21 R37f: extend the BorderGraph with this chunk's new borders when
    // `delta_mode && streaming_active`. Both flags must be set per the MUST condition.
    if delta_mode && streaming_active {
        if let Some(bg) = border_graph {
            bg.extend_with_chunk_borders(&result.borders);
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::Symbol;
    use crate::partition::IdRange;

    /// Builds a `Partition` whose `free_port_index` is exactly `idx`.
    /// All other fields are zeroed defaults — `compute_border_activity`
    /// only consults `free_port_index`, so the rest is irrelevant.
    fn make_partition_with_index(idx: HashMap<u32, PortRef>) -> Partition {
        Partition {
            subnet: Net::new(),
            worker_id: 0,
            free_port_index: idx,
            id_range: IdRange { start: 0, end: 100 },
            border_id_start: 0,
            border_id_end: 0,
        }
    }

    // === SPEC-19 §3.1 R1 — compute_border_activity tests (TASK-0348) ===

    // UT-0348-01: Empty index ⇒ no border endpoints ⇒ false.
    #[test]
    fn compute_border_activity_returns_false_for_empty_index() {
        let partition = make_partition_with_index(HashMap::new());
        assert!(
            !compute_border_activity(&partition),
            "empty free_port_index must yield false"
        );
    }

    // UT-0348-02: Auxiliary ports (slot 1, 2) must NOT trigger activity.
    #[test]
    fn compute_border_activity_returns_false_when_only_aux_ports() {
        let mut idx = HashMap::new();
        idx.insert(0, PortRef::AgentPort(0, 1));
        idx.insert(1, PortRef::AgentPort(1, 2));
        idx.insert(2, PortRef::AgentPort(2, 1));
        let partition = make_partition_with_index(idx);
        assert!(
            !compute_border_activity(&partition),
            "auxiliary-only ports must yield false"
        );
    }

    // UT-0348-03: A single principal-port border endpoint flips the flag.
    #[test]
    fn compute_border_activity_returns_true_for_single_principal_port() {
        let mut idx = HashMap::new();
        idx.insert(0, PortRef::AgentPort(0, 0));
        let partition = make_partition_with_index(idx);
        assert!(
            compute_border_activity(&partition),
            "single principal-port endpoint must yield true"
        );
    }

    // UT-0348-04: Sanity — every entry principal still yields true.
    #[test]
    fn compute_border_activity_returns_true_when_all_principal() {
        let mut idx = HashMap::new();
        for i in 0..4u32 {
            idx.insert(i, PortRef::AgentPort(i, 0));
        }
        let partition = make_partition_with_index(idx);
        assert!(compute_border_activity(&partition));
    }

    // UT-0348-05: Existence semantics — one principal among non-principals.
    // Also verifies FreePort variants are accepted as non-principal.
    #[test]
    fn compute_border_activity_returns_true_for_mixed_with_principal() {
        let mut idx = HashMap::new();
        idx.insert(0, PortRef::FreePort(7));
        idx.insert(1, PortRef::AgentPort(0, 1));
        idx.insert(2, PortRef::AgentPort(1, 0)); // <- the one principal
        idx.insert(3, PortRef::FreePort(8));
        let partition = make_partition_with_index(idx);
        assert!(
            compute_border_activity(&partition),
            "mixed with one principal must yield true"
        );
    }

    // === SPEC-19 §3.1 — TASK-0349 wiring tests ===
    //
    // These tests exercise the *full* path: build a real subnet with a
    // boundary FreePort, run `rebuild_free_port_index`, and check that
    // `compute_border_activity` reflects the post-rebuild state. They
    // are the unit-level proof that the TASK-0349 wiring (R1 ordering:
    // rebuild BEFORE compute) holds end-to-end on a real `Partition`.

    // UT-0349-01 (positive): a CON agent whose principal port is a
    // boundary FreePort must produce a `true` activity flag after the
    // wiring path: rebuild_free_port_index -> compute_border_activity.
    #[test]
    fn ut_0349_01_wiring_principal_border_port_yields_true() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        // Principal (port 0) -> boundary FreePort(border_id_start = 100).
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(100));
        // Auxiliaries -> Lafont FreePorts (excluded from index).
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));

        let idx = rebuild_free_port_index(&net, 100, 200);
        let partition = Partition {
            subnet: net,
            worker_id: 0,
            free_port_index: idx,
            id_range: IdRange { start: 0, end: 100 },
            border_id_start: 100,
            border_id_end: 200,
        };
        assert!(
            compute_border_activity(&partition),
            "principal-port border endpoint must propagate as true"
        );
    }

    // UT-0349-02 (negative): a CON whose ONLY boundary endpoints are
    // auxiliary ports yields `false` — auxiliary endpoints can never
    // form a redex (R5: only principal-principal pairs are redexes).
    #[test]
    fn ut_0349_02_wiring_auxiliary_only_border_yields_false() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        // Principal -> Lafont (excluded).
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));
        // Both auxiliaries -> boundary FreePorts (included, but aux).
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(100));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(101));

        let idx = rebuild_free_port_index(&net, 100, 200);
        assert_eq!(idx.len(), 2, "both aux endpoints must be indexed");
        let partition = Partition {
            subnet: net,
            worker_id: 0,
            free_port_index: idx,
            id_range: IdRange { start: 0, end: 100 },
            border_id_start: 100,
            border_id_end: 200,
        };
        assert!(
            !compute_border_activity(&partition),
            "auxiliary-only borders cannot fire a redex; flag must be false"
        );
    }

    // UT-0349-03 (R1 ordering): if `compute_border_activity` is called
    // on a STALE index (i.e. before `rebuild_free_port_index`), it may
    // disagree with the post-rebuild result. This test pins down the
    // ordering contract by computing both values and asserting that
    // the rebuild is what makes the flag truthful for the current net.
    #[test]
    fn ut_0349_03_wiring_rebuild_must_precede_compute() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(100));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));

        // STALE: empty index (simulating "compute before rebuild").
        let stale = Partition {
            subnet: Net::new(),
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange { start: 0, end: 100 },
            border_id_start: 100,
            border_id_end: 200,
        };
        assert!(
            !compute_border_activity(&stale),
            "stale (empty) index reads false even when net has activity"
        );

        // FRESH: rebuilt index reflects the real net state.
        let fresh_idx = rebuild_free_port_index(&net, 100, 200);
        let fresh = Partition {
            subnet: net,
            worker_id: 0,
            free_port_index: fresh_idx,
            id_range: IdRange { start: 0, end: 100 },
            border_id_start: 100,
            border_id_end: 200,
        };
        assert!(
            compute_border_activity(&fresh),
            "post-rebuild index must surface the principal border endpoint"
        );
    }

    // === SPEC-19 §3.1 Stage 5 QA probes (qa agent, 2026-04-16) ===
    //
    // Adversarial probes from REVIEW-SPEC-19-section-3.1-2026-04-16.md §7.
    // Direct black-box pins on `compute_border_activity` semantics that the
    // production helpers in `merge/grid.rs` rely on.

    /// Probe A — empty `free_port_index` MUST yield `false` (not vacuous-true).
    ///
    /// REVIEW Smell #4: the reviewer accepted vacuous-true on empty-workers as
    /// "unreachable in core path" because `run_grid` asserts `num_workers >= 1`.
    /// This probe pins the contract from the helper side: even if a future
    /// caller bypasses `run_grid`, an empty-port index must NEVER be reported
    /// as "border-active". Returning `true` here would silently engage skip-merge
    /// without any actual workers, which defense-in-depth must prevent.
    #[test]
    fn qa_probe_a_empty_workers_returns_false_not_vacuous_true() {
        // Helper handles a "no workers" / "no border ports" call gracefully
        // by returning `false` (`Iterator::any` over an empty iter is false).
        let partition = make_partition_with_index(HashMap::new());
        assert!(
            !compute_border_activity(&partition),
            "QA probe A: empty free_port_index MUST yield false (defense-in-depth)"
        );

        // Pin defense-in-depth at the run_grid integration layer too:
        // the upstream `assert!(num_workers >= 1)` will trigger before any
        // helper call, so passing 0 workers panics instead of silently
        // skipping. This is what we want — never a vacuous-true skip.
        // (The actual panic check is performed in
        // `qa_probe_a_run_grid_panics_on_zero_workers` in grid.rs.)
    }

    /// Probe F — empty AND non-principal-only border maps both return `false`.
    ///
    /// Reviewer-added probe from REVIEW §7. Pins both edge cases:
    ///   (1) `free_port_index` empty (zero entries) ⇒ false
    ///   (2) `free_port_index` non-empty but every entry is non-principal
    ///       (FreePort variants OR aux-port AgentPort entries) ⇒ false
    ///
    /// (1) is the same case as Probe A but kept separate per the review's
    /// enumeration. (2) is the sharper case: a malformed-but-syntactically-valid
    /// index could come from a buggy `rebuild_free_port_index` that returned
    /// stale FreePort-to-FreePort redirects. The helper must still report
    /// `false` because no principal port is involved.
    #[test]
    fn qa_probe_f_empty_and_non_principal_border_maps_return_false() {
        // Case 1: empty index.
        let empty = make_partition_with_index(HashMap::new());
        assert!(
            !compute_border_activity(&empty),
            "QA probe F.1: empty free_port_index must yield false"
        );

        // Case 2a: non-empty index, every entry is a `FreePort` redirect
        //   (no AgentPort at all — the most pathological case).
        let mut redirect_only = HashMap::new();
        redirect_only.insert(100, PortRef::FreePort(0));
        redirect_only.insert(101, PortRef::FreePort(7));
        redirect_only.insert(102, PortRef::FreePort(u32::MAX));
        let p2a = make_partition_with_index(redirect_only);
        assert!(
            !compute_border_activity(&p2a),
            "QA probe F.2a: FreePort-only index must yield false (no principal port)"
        );

        // Case 2b: non-empty index, every entry is a non-principal AgentPort
        //   (auxiliary ports — index 1 or 2). This is the realistic shape
        //   for a quiescent partition whose only border endpoints are
        //   already aux-only.
        let mut aux_only = HashMap::new();
        for bid in 100..110u32 {
            aux_only.insert(bid, PortRef::AgentPort(bid - 100, 1));
        }
        for bid in 110..120u32 {
            aux_only.insert(bid, PortRef::AgentPort(bid - 110, 2));
        }
        let p2b = make_partition_with_index(aux_only);
        assert!(
            !compute_border_activity(&p2b),
            "QA probe F.2b: aux-port-only index must yield false"
        );

        // Case 2c: mixed FreePort + aux AgentPort, still no principal.
        let mut mixed_no_principal = HashMap::new();
        mixed_no_principal.insert(100, PortRef::FreePort(0));
        mixed_no_principal.insert(101, PortRef::AgentPort(0, 1));
        mixed_no_principal.insert(102, PortRef::AgentPort(1, 2));
        mixed_no_principal.insert(103, PortRef::FreePort(99));
        let p2c = make_partition_with_index(mixed_no_principal);
        assert!(
            !compute_border_activity(&p2c),
            "QA probe F.2c: mixed FreePort + aux must yield false"
        );
    }

    // === is_principal_pair tests (TASK-0064) ===

    // T1: Both principal ports -> true
    #[test]
    fn test_principal_pair_both_principal() {
        assert!(is_principal_pair(
            PortRef::AgentPort(1, 0),
            PortRef::AgentPort(2, 0)
        ));
    }

    // T2: Principal + auxiliary -> false
    #[test]
    fn test_principal_pair_one_auxiliary() {
        assert!(!is_principal_pair(
            PortRef::AgentPort(1, 0),
            PortRef::AgentPort(2, 1)
        ));
    }

    // T3: Auxiliary + principal -> false
    #[test]
    fn test_principal_pair_first_auxiliary() {
        assert!(!is_principal_pair(
            PortRef::AgentPort(1, 1),
            PortRef::AgentPort(2, 0)
        ));
    }

    // T4: FreePort + AgentPort -> false
    #[test]
    fn test_principal_pair_freeport_agent() {
        assert!(!is_principal_pair(
            PortRef::FreePort(5),
            PortRef::AgentPort(2, 0)
        ));
    }

    // T5: FreePort + FreePort -> false
    #[test]
    fn test_principal_pair_both_freeport() {
        assert!(!is_principal_pair(
            PortRef::FreePort(5),
            PortRef::FreePort(6)
        ));
    }

    // === rebuild_free_port_index tests (TASK-0063) ===

    // T1: Subnet with no FreePort connections -> empty index
    #[test]
    fn test_rebuild_no_freeports() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));

        let index = rebuild_free_port_index(&net, 100, 200);
        assert!(index.is_empty());
    }

    // T2: Subnet with one boundary FreePort -> single entry
    #[test]
    fn test_rebuild_one_boundary_freeport() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        // Port 0 -> FreePort(100) (boundary)
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(100));
        // Ports 1, 2 -> Lafont FreePorts (below border_id_start)
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));

        let index = rebuild_free_port_index(&net, 100, 200);
        assert_eq!(index.len(), 1);
        assert_eq!(index[&100], PortRef::AgentPort(a, 0));
    }

    // T3: Lafont FreePorts are excluded
    #[test]
    fn test_rebuild_excludes_lafont_freeports() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        // All ports -> Lafont FreePorts (below border_id_start=50)
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(2));

        let index = rebuild_free_port_index(&net, 50, 100);
        assert!(index.is_empty());
    }

    // T4: DISCONNECTED is excluded
    #[test]
    fn test_rebuild_excludes_disconnected() {
        // DISCONNECTED = FreePort(u32::MAX), should never be in the index
        let net = Net::new();
        let index = rebuild_free_port_index(&net, 0, u32::MAX);
        assert!(index.is_empty());
    }

    // T5: Multiple boundary FreePorts produce correct multi-entry index
    #[test]
    fn test_rebuild_multiple_boundary_freeports() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        // a: port 0 -> FreePort(100), port 1 -> FreePort(101)
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(100));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(101));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));
        // b: port 0 -> FreePort(102)
        net.connect(PortRef::AgentPort(b, 0), PortRef::FreePort(102));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(0)); // Lafont

        let index = rebuild_free_port_index(&net, 100, 200);
        assert_eq!(index.len(), 3);
        assert_eq!(index[&100], PortRef::AgentPort(a, 0));
        assert_eq!(index[&101], PortRef::AgentPort(a, 1));
        assert_eq!(index[&102], PortRef::AgentPort(b, 0));
    }

    // T6: ERA agent with boundary FreePort on principal port
    #[test]
    fn test_rebuild_era_with_boundary() {
        let mut net = Net::new();
        let e = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(e, 0), PortRef::FreePort(50));

        let index = rebuild_free_port_index(&net, 50, 100);
        assert_eq!(index.len(), 1);
        assert_eq!(index[&50], PortRef::AgentPort(e, 0));
    }

    // T7: Removed agent (None slot) is skipped
    #[test]
    fn test_rebuild_skips_removed_agents() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(100));
        // Remove the agent
        net.remove_agent(a);

        let index = rebuild_free_port_index(&net, 100, 200);
        assert!(index.is_empty());
    }

    // === drain_stale_redexes tests (TASK-0068) ===

    // T1: Empty queue stays empty
    #[test]
    fn test_drain_empty_queue() {
        let mut net = Net::new();
        drain_stale_redexes(&mut net);
        assert!(net.redex_queue.is_empty());
    }

    // T2: Queue with only valid redexes -> all retained
    #[test]
    fn test_drain_all_valid() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // connect() already pushed (a, b) into redex_queue
        assert_eq!(net.redex_queue.len(), 1);

        drain_stale_redexes(&mut net);
        assert_eq!(net.redex_queue.len(), 1);
        assert_eq!(net.redex_queue[0], (a, b));
    }

    // T3: Queue with only stale redexes -> empty after drain
    #[test]
    fn test_drain_all_stale() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Remove agents, making the redex stale
        net.remove_agent(a);
        net.remove_agent(b);

        assert_eq!(net.redex_queue.len(), 1);
        drain_stale_redexes(&mut net);
        assert!(net.redex_queue.is_empty());
    }

    // T4: Mixed valid and stale -> only valid retained, in order
    #[test]
    fn test_drain_mixed() {
        let mut net = Net::new();
        // Create two valid redexes
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let c = net.create_agent(Symbol::Era);
        let d = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));

        // Make first redex stale by removing agent a
        net.remove_agent(a);
        net.remove_agent(b);

        assert_eq!(net.redex_queue.len(), 2);
        drain_stale_redexes(&mut net);
        assert_eq!(net.redex_queue.len(), 1);
        assert_eq!(net.redex_queue[0], (c, d));
    }

    // === SPEC-19 R25 — compute_outgoing_deltas tests (TASK-0382) ===

    #[test]
    fn compute_outgoing_deltas_empty_both() {
        let previous: HashMap<u32, PortRef> = HashMap::new();
        let current: HashMap<u32, PortRef> = HashMap::new();
        assert!(compute_outgoing_deltas(&previous, &current).is_empty());
    }

    #[test]
    fn compute_outgoing_deltas_unchanged_entry_emits_nothing() {
        let mut previous = HashMap::new();
        let mut current = HashMap::new();
        previous.insert(5, PortRef::AgentPort(1, 0));
        current.insert(5, PortRef::AgentPort(1, 0));
        assert!(compute_outgoing_deltas(&previous, &current).is_empty());
    }

    #[test]
    fn compute_outgoing_deltas_changed_entry_emits_delta() {
        let mut previous = HashMap::new();
        let mut current = HashMap::new();
        previous.insert(5, PortRef::AgentPort(1, 0));
        current.insert(5, PortRef::AgentPort(2, 1));
        let out = compute_outgoing_deltas(&previous, &current);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].border_id, 5);
        assert_eq!(out[0].new_target, PortRef::AgentPort(2, 1));
    }

    #[test]
    fn compute_outgoing_deltas_new_entry_emits_delta() {
        let previous = HashMap::new();
        let mut current = HashMap::new();
        current.insert(9, PortRef::AgentPort(3, 0));
        let out = compute_outgoing_deltas(&previous, &current);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].border_id, 9);
        assert_eq!(out[0].new_target, PortRef::AgentPort(3, 0));
    }

    #[test]
    fn compute_outgoing_deltas_removed_entry_emits_sentinel() {
        let mut previous = HashMap::new();
        let current: HashMap<u32, PortRef> = HashMap::new();
        previous.insert(7, PortRef::AgentPort(4, 0));
        let out = compute_outgoing_deltas(&previous, &current);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].border_id, 7);
        assert_eq!(
            out[0].new_target, DISCONNECTED,
            "DC-C6: removed border must emit DISCONNECTED sentinel, \
             not FreePort(border_id)"
        );
    }

    #[test]
    fn compute_outgoing_deltas_mixed() {
        // 5 previous: [0, 1, 2, 3, 4]; current drops 4, changes 1, keeps 0, 2, 3;
        // current adds 10. Expected deltas: {1 changed, 4 disconnected, 10 new}.
        let mut previous = HashMap::new();
        let mut current = HashMap::new();
        previous.insert(0, PortRef::AgentPort(10, 0));
        previous.insert(1, PortRef::AgentPort(11, 0));
        previous.insert(2, PortRef::AgentPort(12, 0));
        previous.insert(3, PortRef::AgentPort(13, 0));
        previous.insert(4, PortRef::AgentPort(14, 0));
        current.insert(0, PortRef::AgentPort(10, 0));
        current.insert(1, PortRef::AgentPort(99, 1));
        current.insert(2, PortRef::AgentPort(12, 0));
        current.insert(3, PortRef::AgentPort(13, 0));
        current.insert(10, PortRef::AgentPort(50, 0));

        let mut out = compute_outgoing_deltas(&previous, &current);
        out.sort_by_key(|d| d.border_id);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].border_id, 1);
        assert_eq!(out[0].new_target, PortRef::AgentPort(99, 1));
        assert_eq!(out[1].border_id, 4);
        assert_eq!(out[1].new_target, DISCONNECTED);
        assert_eq!(out[2].border_id, 10);
        assert_eq!(out[2].new_target, PortRef::AgentPort(50, 0));
    }

    #[test]
    fn compute_outgoing_deltas_large_unchanged() {
        let mut previous = HashMap::new();
        let mut current = HashMap::new();
        for i in 0..1000u32 {
            previous.insert(i, PortRef::AgentPort(i, 0));
            current.insert(i, PortRef::AgentPort(i, 0));
        }
        assert!(compute_outgoing_deltas(&previous, &current).is_empty());
    }

    // DC-C6 amendment (2026-04-17): disconnect / reconnect path coverage.
    #[test]
    fn compute_outgoing_deltas_encodes_disconnect_as_sentinel() {
        let mut previous = HashMap::new();
        previous.insert(3, PortRef::AgentPort(7, 0));
        let current: HashMap<u32, PortRef> = HashMap::new();
        let out = compute_outgoing_deltas(&previous, &current);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].border_id, 3);
        assert_eq!(out[0].new_target, DISCONNECTED);
    }

    #[test]
    fn compute_outgoing_deltas_encodes_reconnect_as_agentport() {
        let previous: HashMap<u32, PortRef> = HashMap::new();
        let mut current = HashMap::new();
        current.insert(3, PortRef::AgentPort(11, 0));
        let out = compute_outgoing_deltas(&previous, &current);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].border_id, 3);
        assert!(matches!(out[0].new_target, PortRef::AgentPort(_, _)));
    }

    // ---------------------------------------------------------------------------
    // MF-001: generate_and_partition_chunked_with_delta tests (SPEC-21 R37f)
    // ---------------------------------------------------------------------------

    /// Build an empty BorderGraph for MF-001 tests. Mirrors the
    /// `make_empty_border_graph` private helper in `border_graph.rs::tests`.
    fn make_empty_border_graph_mf001(num_workers: usize) -> crate::merge::BorderGraph {
        use std::collections::{HashMap, HashSet};
        crate::merge::BorderGraph {
            borders: HashMap::new(),
            worker_borders: vec![Vec::new(); num_workers],
            active_redexes: HashSet::new(),
            pending_new_borders: Vec::new(),
            resolved_mints: HashMap::new(),
        }
    }

    /// MF-001-A: Under delta_mode=true AND streaming_active=true, the function
    /// calls extend_with_chunk_borders and populates the BorderGraph.
    #[test]
    fn mf001_a_delta_and_streaming_active_extends_border_graph() {
        use crate::net::Symbol;
        use crate::partition::streaming::{
            AgentBatch, ConnectionDirective, RoundRobinStreamingStrategy,
        };

        // Build a 2-worker stream: 2 agents connected across workers
        // (agent 0 -> worker 0, agent 1 -> worker 1, wire between them).
        let stream = Box::new(std::iter::once(AgentBatch {
            agents: vec![(0u32, Symbol::Era), (1u32, Symbol::Era)],
            connections: vec![ConnectionDirective::Resolved {
                source: (0u32, 0u8),
                target: (1u32, 0u8),
            }],
        }));
        let mut strategy = RoundRobinStreamingStrategy::new(2);
        let mut border_graph = make_empty_border_graph_mf001(2);

        let result = super::generate_and_partition_chunked_with_delta(
            stream,
            2,
            &mut strategy,
            Some(&mut border_graph),
            true, // delta_mode = true
            true, // streaming_active = true
        )
        .expect("partition must succeed");

        // The cross-worker wire must have produced a border entry.
        assert!(
            !result.borders.is_empty(),
            "MF-001-A: result.borders must be non-empty"
        );

        // BorderGraph must have been extended with the same entries.
        assert_eq!(
            border_graph.borders.len(),
            result.borders.len(),
            "MF-001-A: BorderGraph must contain same number of borders as result"
        );
    }

    /// MF-001-B: Under delta_mode=false (no delta), extend_with_chunk_borders is NOT called.
    /// The BorderGraph stays empty even if streaming_active=true.
    #[test]
    fn mf001_b_no_delta_border_graph_not_extended() {
        use crate::net::Symbol;
        use crate::partition::streaming::{
            AgentBatch, ConnectionDirective, RoundRobinStreamingStrategy,
        };

        let stream = Box::new(std::iter::once(AgentBatch {
            agents: vec![(0u32, Symbol::Era), (1u32, Symbol::Era)],
            connections: vec![ConnectionDirective::Resolved {
                source: (0u32, 0u8),
                target: (1u32, 0u8),
            }],
        }));
        let mut strategy = RoundRobinStreamingStrategy::new(2);
        let mut border_graph = make_empty_border_graph_mf001(2);

        let _result = super::generate_and_partition_chunked_with_delta(
            stream,
            2,
            &mut strategy,
            Some(&mut border_graph),
            false, // delta_mode = false
            true,  // streaming_active = true
        )
        .expect("partition must succeed");

        // BorderGraph must NOT have been extended.
        assert!(
            border_graph.borders.is_empty(),
            "MF-001-B: BorderGraph must remain empty when delta_mode=false"
        );
    }

    /// MF-001-C: With border_graph=None, the function succeeds without panic
    /// regardless of delta_mode/streaming_active flags.
    #[test]
    fn mf001_c_none_border_graph_is_safe() {
        use crate::net::Symbol;
        use crate::partition::streaming::{AgentBatch, RoundRobinStreamingStrategy};

        let stream = Box::new(std::iter::once(AgentBatch {
            agents: vec![(0u32, Symbol::Era)],
            connections: vec![],
        }));
        let mut strategy = RoundRobinStreamingStrategy::new(1);

        let result = super::generate_and_partition_chunked_with_delta(
            stream,
            1,
            &mut strategy,
            None,
            true,
            true,
        )
        .expect("partition must succeed");

        assert_eq!(
            result.partitions.len(),
            1,
            "MF-001-C: one partition for one worker"
        );
    }
}
