//! Helper functions for merge and grid cycle (SPEC-05).
//!
//! - `is_principal_pair`: checks if both ports are principal (for border redex counting)
//! - `rebuild_free_port_index`: lazy reconstruction of the FreePort index after local reduction
//! - `drain_stale_redexes`: removes stale entries from the redex queue

use std::collections::HashMap;
use std::collections::VecDeque;

use crate::net::{total_ports, Net, PortRef};
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
}
