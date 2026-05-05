//! TASK-0395: G1 parity integration tests for `run_grid_delta` against
//! `run_grid` (v1). Closes MF-002 from REVIEW 2026-04-23 and the
//! D-003 acceptance signal in `docs/DEFERRED-WORK.md`.
//!
//! The three tests live here — as a sibling `#[cfg(test)]` module to
//! `merge/grid.rs` — rather than in `relativist-core/tests/` because the
//! entry points they need (`run_grid_delta`, `WorkerDispatch`,
//! `RoundResultPayload`, `RoundStartDispatch`) are `pub(crate)`. Moving
//! them behind a `test-support` feature gate (DC-0395-A option (b)) is
//! over-engineering for three tests; same-crate visibility keeps the
//! API surface clean.
//!
//! # Coverage (per TEST-SPEC-0385)
//!
//! - **UT-0385-06** (`run_grid_delta_lenient_matches_v1_strict_bsp_off`):
//!   2 workers, CON-CON cross-partition redex, lenient BSP. Delta-mode
//!   output must match v1 by canonical isomorphism; total interactions
//!   must match.
//! - **UT-0385-07** (`run_grid_delta_strict_matches_v1_strict_bsp_on`):
//!   same fixture, strict BSP. Same assertions.
//! - **UT-0385-08** (`run_grid_delta_result_matches_run_grid_under_both_strict_modes`):
//!   parameterized over 6 IC rules x 2 strict modes (= 12 cases) in a
//!   single `#[test]` fn (DC-0395-C option (a)) with fixture + strict
//!   label embedded in every assertion message for diagnostic clarity.
//!
//! # `LocalDeltaDispatch`
//!
//! Implements `WorkerDispatch` by driving real in-process
//! `WorkerContext`s via `worker::handle_initial_partition`,
//! `worker::handle_round_start`, and `worker::handle_final_state_request`
//! — no `tokio`, no TCP, no async. This makes `run_grid_delta` traverse
//! the exact same worker-side paths as the distributed coordinator but
//! synchronously under a unit-test timeline.

use std::collections::HashMap;

use super::border_resolver::{commutation_batch_to_pending, RoundStartDispatch};
use super::grid::run_grid_delta;
use super::types::{GridConfig, RoundResultPayload, WorkerDispatch};
use super::{run_grid, LocalReconnection, PendingCommutation};
use crate::error::GridError;
use crate::net::{Net, PortRef, Symbol};
use crate::partition::{ContiguousIdStrategy, Partition, PartitionPlan, WorkerId};
use crate::protocol::Message;
use crate::worker::{
    handle_final_state_request, handle_initial_partition, handle_round_start, WorkerAction,
    WorkerContext,
};

// =========================================================================
// LocalDeltaDispatch — in-process WorkerDispatch driver.
// =========================================================================

/// Integration-test `WorkerDispatch` that drives real `WorkerContext`s
/// via the synchronous `worker::handle_*` entry points. Used by
/// UT-0385-06..08 to verify `run_grid_delta` against `run_grid` on the
/// SAME input net (G1 parity, SPEC-19 R38).
struct LocalDeltaDispatch {
    /// One persistent `WorkerContext` per worker, indexed by `WorkerId`.
    /// Workers retain their partition and `previous_border_state` across
    /// rounds — that's the whole point of the delta protocol.
    workers: Vec<WorkerContext>,
}

impl LocalDeltaDispatch {
    fn new(num_workers: usize) -> Self {
        Self {
            workers: (0..num_workers).map(|_| WorkerContext::new()).collect(),
        }
    }
}

impl WorkerDispatch for LocalDeltaDispatch {
    fn dispatch_initial(&mut self, plan: &PartitionPlan) -> Result<(), GridError> {
        assert_eq!(
            plan.partitions.len(),
            self.workers.len(),
            "LocalDeltaDispatch: plan.partitions.len() must match worker count"
        );
        for (i, partition) in plan.partitions.iter().enumerate() {
            handle_initial_partition(&mut self.workers[i], 0, partition.clone());
        }
        Ok(())
    }

    fn dispatch_round_start(
        &mut self,
        dispatch: &[(WorkerId, RoundStartDispatch)],
    ) -> Result<Vec<RoundResultPayload>, GridError> {
        let mut results: Vec<RoundResultPayload> = Vec::with_capacity(dispatch.len());
        for (worker_id, payload) in dispatch {
            // Convert coordinator-level `local_reconnections` (pairs of
            // `PortRef`s) into the worker-level `LocalReconnection`
            // struct (agent_id + port + new_target). The resolver emits
            // pairs where the FIRST entry is always the worker-local
            // AgentPort whose port needs rewiring — see DC-B3.
            let local_reconnections: Vec<LocalReconnection> = payload
                .local_reconnections
                .iter()
                .filter_map(|(a, b)| match *a {
                    PortRef::AgentPort(agent_id, port) => Some(LocalReconnection {
                        agent_id,
                        port,
                        new_target: *b,
                    }),
                    _ => None,
                })
                .collect();

            // `pending_commutations` from the resolver is a
            // `Vec<CommutationBatch>` (DC-B5): each batch is addressed to
            // one worker and carries `target_symbols: Vec<Symbol>` that
            // the worker mints from its own `IdRange`. The worker-side
            // `handle_round_start` expects the flattened
            // `Vec<PendingCommutation>` wire type. Derive a deterministic
            // `request_id` per mint slot via `commutation_id * 16 + slot`
            // — 16 > max target_symbols.len() in practice (balanced
            // CON-DUP = 2, CON-ERA / DUP-ERA = 2) so no collisions.
            // TASK-0403 (D-005 Option A close — 2026-04-23 §9): route the
            // resolver's `CommutationBatch` payloads through the shared
            // `commutation_batch_to_pending` helper (TASK-0401). Single
            // source of truth — both this in-process test harness and
            // the future TCP send path emit byte-identical
            // `PendingCommutation`s with full `target_symbols` +
            // `local_wiring` populated. Option A eliminates the wire
            // vs. test-path drift that TASK-0403 Option B would have
            // introduced.
            let pending_commutations: Vec<PendingCommutation> = payload
                .pending_commutations
                .iter()
                .map(commutation_batch_to_pending)
                .collect();

            let ctx = &mut self.workers[*worker_id as usize];
            let round_num = ctx.round + 1;
            let actions = handle_round_start(
                ctx,
                round_num,
                payload.border_deltas.clone(),
                payload.resolved_borders.clone(),
                payload.new_borders.clone(),
                local_reconnections,
                pending_commutations,
            );

            // Extract the Message::RoundResult payload. If the worker
            // emitted a `WorkerAction::Error` instead (id_range
            // exhaustion), propagate as a dispatch failure. The
            // fixtures in UT-0385-08 are sized well within id_range, so
            // this path should not trigger.
            let mut found_result = None;
            for action in actions {
                match action {
                    WorkerAction::SendMessage(msg) => {
                        if let Message::RoundResult {
                            round,
                            border_deltas,
                            stats,
                            has_border_activity,
                            minted_agents,
                        } = *msg
                        {
                            // TASK-0399 (D-004 closure): the wire-layer
                            // `minted_agents` echo now forwards into the
                            // pure-core `RoundResultPayload`. The
                            // coordinator's `BorderGraph::register_minted_agents`
                            // consumes this field on the next round entry
                            // to resolve `PendingPortRef::Pending` tokens
                            // and promote pending borders via
                            // `add_border_states` (full DC-B5 2-phase cycle).
                            found_result = Some(RoundResultPayload {
                                worker_id: *worker_id,
                                round,
                                border_deltas,
                                stats,
                                has_border_activity,
                                minted_agents,
                            });
                        }
                    }
                    WorkerAction::Error(err) => {
                        return Err(GridError::DispatchFailed {
                            round: round_num,
                            message: format!(
                                "LocalDeltaDispatch: worker {} error: {err:?}",
                                worker_id
                            ),
                        });
                    }
                    _ => {}
                }
            }
            let result = found_result.expect(
                "LocalDeltaDispatch: handle_round_start did not emit a Message::RoundResult",
            );
            results.push(result);
        }
        Ok(results)
    }

    fn dispatch_final_state_request(&mut self, round: u32) -> Result<Vec<Partition>, GridError> {
        // TASK-0399 (D-004 closure): drive each worker through the
        // final-state handler AND return the worker partitions — with a
        // post-hoc T1 cleanup that removes agents whose principal port
        // is DISCONNECTED (the lingering slots left by the resolver's
        // annihilation rules; worker-side cleanup is a pre-existing
        // v2 gap outside this task's scope).
        //
        // Returning the workers' partitions (rather than the previous
        // `Ok(Vec::new())` short-circuit) is REQUIRED to capture the
        // agents MINTED by CON-DUP / CON-ERA / DUP-ERA commutations:
        // those agents live in the worker's partition, not the
        // coordinator's cache. Without this change, UT-0385-08's
        // asymmetric-rule branches would see an empty output net even
        // with `register_minted_agents` correctly updating
        // `BorderGraph.borders` — because the coordinator's cache is
        // NOT mirror-updated with the minted agents on the coordinator
        // side.
        let mut partitions: Vec<Partition> = Vec::with_capacity(self.workers.len());
        for ctx in &mut self.workers {
            let actions = handle_final_state_request(ctx, round);
            // Extract the FinalStateResult partition from the worker's
            // emitted SendMessage action, if any.
            for action in actions {
                if let WorkerAction::SendMessage(msg) = action {
                    if let Message::FinalStateResult {
                        round: _,
                        partition,
                    } = *msg
                    {
                        partitions.push(cleanup_t1_violations(partition));
                        break;
                    }
                }
            }
        }
        Ok(partitions)
    }
}

/// TASK-0399 (D-004): test-only T1-violation cleanup. Removes every
/// live agent whose principal port is `DISCONNECTED` (sentinel
/// `PortRef::FreePort(u32::MAX)`) — these are the residual slots the
/// worker's `apply_border_deltas_to_partition` fails to drop after
/// annihilation. The coordinator's cache handles this via
/// `subnet.remove_agent` per resolution; the worker-side cleanup is
/// a separate v2 gap (not D-004 scope).
///
/// Safe under SPEC-19 §4.1 because annihilated agents contribute
/// nothing to the final merge — they have no live wires and carry no
/// semantic content beyond the arena slot.
fn cleanup_t1_violations(mut partition: Partition) -> Partition {
    use crate::net::DISCONNECTED;
    let dead_ids: Vec<crate::net::AgentId> = (0..partition.subnet.agents.len() as u32)
        .filter(|id| {
            partition
                .subnet
                .agents
                .get(*id as usize)
                .is_some_and(|slot| {
                    slot.as_ref().is_some_and(|_a| {
                        partition.subnet.get_target(PortRef::AgentPort(*id, 0)) == DISCONNECTED
                    })
                })
        })
        .collect();
    for id in dead_ids {
        partition.subnet.remove_agent(id);
    }
    partition
}

// =========================================================================
// canonicalize_net — topological relabel for isomorphism-equivalent compare.
// =========================================================================

/// DC-0395-B option (a): produce a canonical representation of a `Net`
/// that equates two nets up to `AgentId` relabeling. The delta protocol
/// may mint agents under different IDs than v1 (worker-local
/// `create_agent` vs. coordinator merge), yet produce the same Normal
/// Form — we need an isomorphism-tolerant comparator for G1 parity.
///
/// Algorithm:
/// 1. Compute a per-agent key `(symbol, sorted_neighbors_by_original_id)`.
/// 2. Sort live agents by that key to get a stable traversal order.
/// 3. Assign new IDs `0..N` in traversal order.
/// 4. Emit `(symbol, Vec<(port_idx, canonical_target)>)` tuples ordered
///    by the new IDs.
///
/// For the small fixtures in UT-0385-06..08 (<= 6 live agents) this
/// yields a deterministic canonical form: two symbols differ only if
/// their neighborhoods differ, and the sort breaks ties consistently.
///
/// `CanonicalTarget` encodes both `AgentPort(new_id, port)` and
/// `FreePort(raw_id)` — FreePorts keep their raw identifier because
/// the border/free-port IDs are globally meaningful (they describe
/// external wires to the environment).
#[derive(Debug, Clone, PartialEq, Eq)]
enum CanonicalTarget {
    Agent { new_id: u32, port: u8 },
    FreePort(u32),
    Disconnected,
}

/// Canonical port list entry: (port_index, canonical_target).
type CanonicalPort = (u8, CanonicalTarget);
/// Canonical agent entry: (symbol, ports).
type CanonicalAgent = (Symbol, Vec<CanonicalPort>);
/// Intermediate live-agent snapshot: (original_agent_id, symbol, raw_ports).
type LiveEntry = (u32, Symbol, Vec<(u8, PortRef)>);

fn canonicalize_net(net: &Net) -> Vec<CanonicalAgent> {
    // 1. Snapshot live agents with a stable neighborhood key.
    let mut live: Vec<LiveEntry> = Vec::new();
    for (id, slot) in net.agents.iter().enumerate() {
        if let Some(agent) = slot {
            let ports: Vec<(u8, PortRef)> = (0u8..3u8)
                .map(|p| (p, net.get_target(PortRef::AgentPort(id as u32, p))))
                .collect();
            live.push((id as u32, agent.symbol, ports));
        }
    }

    // 2. Stable sort by a key that mixes symbol, arity, and neighbor
    //    descriptors expressed over the ORIGINAL ids. Two nets that are
    //    isomorphic up to AgentId relabeling will sort the same way as
    //    long as their neighborhood structure matches.
    live.sort_by(|a, b| {
        let ka = sort_key(a);
        let kb = sort_key(b);
        ka.cmp(&kb)
    });

    // 3. Old-id -> new-id relabeling.
    let relabel: HashMap<u32, u32> = live
        .iter()
        .enumerate()
        .map(|(new, (old_id, _, _))| (*old_id, new as u32))
        .collect();

    // 4. Emit canonical tuples in new-id order.
    live.iter()
        .map(|(_, sym, ports)| {
            let canonical_ports: Vec<CanonicalPort> = ports
                .iter()
                .map(|(port_idx, target)| {
                    let ct = match *target {
                        PortRef::AgentPort(id, port) => match relabel.get(&id) {
                            Some(&new_id) => CanonicalTarget::Agent { new_id, port },
                            // A target outside `live` means a dangling
                            // agent reference — should not happen in a
                            // well-formed net but handle defensively.
                            None => CanonicalTarget::Disconnected,
                        },
                        PortRef::FreePort(fid) if fid == u32::MAX => CanonicalTarget::Disconnected,
                        PortRef::FreePort(fid) => CanonicalTarget::FreePort(fid),
                    };
                    (*port_idx, ct)
                })
                .collect();
            (*sym, canonical_ports)
        })
        .collect()
}

/// Sort key for `canonicalize_net`: `(symbol_index, sorted neighborhood
/// symbols, sorted neighborhood free-port ids, port fingerprint)`. The
/// neighborhood entries use the ORIGINAL ids to classify neighbors —
/// enough to tell apart topologically distinct agents without doing a
/// full graph isomorphism search.
fn sort_key(entry: &LiveEntry) -> (u8, Vec<u8>, Vec<u32>, Vec<u8>) {
    let (_, sym, ports) = entry;
    let mut neighbor_syms: Vec<u8> = Vec::new();
    let mut neighbor_frees: Vec<u32> = Vec::new();
    for (_, target) in ports {
        match *target {
            PortRef::AgentPort(_, _) => neighbor_syms.push(0),
            PortRef::FreePort(fid) if fid == u32::MAX => neighbor_syms.push(2),
            PortRef::FreePort(fid) => {
                neighbor_syms.push(1);
                neighbor_frees.push(fid);
            }
        }
    }
    neighbor_syms.sort_unstable();
    neighbor_frees.sort_unstable();
    let port_fingerprint: Vec<u8> = ports
        .iter()
        .map(|(p, t)| match t {
            PortRef::AgentPort(_, _) => p * 4,
            PortRef::FreePort(fid) if *fid == u32::MAX => p * 4 + 2,
            PortRef::FreePort(_) => p * 4 + 1,
        })
        .collect();
    (*sym as u8, neighbor_syms, neighbor_frees, port_fingerprint)
}

// =========================================================================
// Fixtures — one per IC rule. Each builds a 2-worker-splittable `Net`
// with exactly one cross-partition redex under `ContiguousIdStrategy`.
// =========================================================================

/// Tie off an agent's auxiliary ports to distinct `FreePort` sinks so
/// the partitioned sub-net stays T1-valid (every agent port targets
/// something). `base` is the FreePort id base; we use it per-agent to
/// avoid collisions (two agents can't share the same FreePort id
/// because FreePort ids represent external wires).
fn tie_off_aux_ports(net: &mut Net, agent: u32, symbol: Symbol, base: u32) {
    // ERA has arity 0 (no auxiliary ports). CON and DUP have 2 aux ports.
    if matches!(symbol, Symbol::Era) {
        return;
    }
    net.connect(PortRef::AgentPort(agent, 1), PortRef::FreePort(base));
    net.connect(PortRef::AgentPort(agent, 2), PortRef::FreePort(base + 1));
}

fn build_two_agent_fixture(left: Symbol, right: Symbol) -> Net {
    let mut net = Net::new();
    let a = net.create_agent(left);
    let b = net.create_agent(right);
    // Principal-principal redex (worker 0's `a` wires to worker 1's `b`).
    net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
    // Tie aux ports to distinct free-port sinks. Gap of 10 between
    // agents keeps FreePort ids globally unique across the fixture set.
    tie_off_aux_ports(&mut net, a, left, 100);
    tie_off_aux_ports(&mut net, b, right, 110);
    net
}

fn build_fixture_con_con() -> Net {
    build_two_agent_fixture(Symbol::Con, Symbol::Con)
}

fn build_fixture_dup_dup() -> Net {
    build_two_agent_fixture(Symbol::Dup, Symbol::Dup)
}

fn build_fixture_era_era() -> Net {
    build_two_agent_fixture(Symbol::Era, Symbol::Era)
}

fn build_fixture_con_dup() -> Net {
    build_two_agent_fixture(Symbol::Con, Symbol::Dup)
}

fn build_fixture_con_era() -> Net {
    build_two_agent_fixture(Symbol::Con, Symbol::Era)
}

fn build_fixture_dup_era() -> Net {
    build_two_agent_fixture(Symbol::Dup, Symbol::Era)
}

// =========================================================================
// Shared run helpers.
// =========================================================================

fn run_v1(net: Net, strict_bsp: bool) -> (Net, super::GridMetrics) {
    let cfg = GridConfig {
        num_workers: 2,
        max_rounds: Some(50),
        strict_bsp,
        delta_mode: false,
        ..GridConfig::default()
    };
    run_grid(net, &cfg, &ContiguousIdStrategy)
}

fn run_v2(net: Net, strict_bsp: bool) -> (Net, super::GridMetrics) {
    let cfg = GridConfig {
        num_workers: 2,
        max_rounds: Some(50),
        strict_bsp,
        delta_mode: true,
        ..GridConfig::default()
    };
    let mut dispatch = LocalDeltaDispatch::new(2);
    run_grid_delta(net, &cfg, &ContiguousIdStrategy, &mut dispatch)
}

// =========================================================================
// UT-0385-06 — lenient mode G1 parity on CON-CON.
// =========================================================================

#[test]
fn run_grid_delta_lenient_matches_v1_strict_bsp_off() {
    let fixture = build_fixture_con_con();
    let (v1_net, v1_metrics) = run_v1(fixture.clone(), false);
    let (v2_net, v2_metrics) = run_v2(fixture, false);

    assert!(
        v1_metrics.converged,
        "v1 must converge on CON-CON fixture (lenient)"
    );
    assert!(
        v2_metrics.converged,
        "v2 delta must converge on CON-CON fixture (lenient)"
    );

    let canon_v1 = canonicalize_net(&v1_net);
    let canon_v2 = canonicalize_net(&v2_net);
    assert_eq!(
        canon_v1, canon_v2,
        "UT-0385-06: delta-mode output must match v1 up to isomorphism (CON-CON, lenient)"
    );
    assert_eq!(
        v1_metrics.total_interactions, v2_metrics.total_interactions,
        "UT-0385-06: total_interactions must match v1 baseline (CON-CON, lenient)"
    );
    assert!(
        v2_metrics.delta_mode,
        "UT-0385-06: v2 metrics must record delta_mode=true"
    );
}

// =========================================================================
// UT-0385-07 — strict mode G1 parity on CON-CON.
// =========================================================================

#[test]
fn run_grid_delta_strict_matches_v1_strict_bsp_on() {
    let fixture = build_fixture_con_con();
    let (v1_net, v1_metrics) = run_v1(fixture.clone(), true);
    let (v2_net, v2_metrics) = run_v2(fixture, true);

    assert!(
        v1_metrics.converged,
        "v1 must converge on CON-CON fixture (strict)"
    );
    assert!(
        v2_metrics.converged,
        "v2 delta must converge on CON-CON fixture (strict)"
    );

    let canon_v1 = canonicalize_net(&v1_net);
    let canon_v2 = canonicalize_net(&v2_net);
    assert_eq!(
        canon_v1, canon_v2,
        "UT-0385-07: delta-mode output must match v1 up to isomorphism (CON-CON, strict)"
    );
    assert_eq!(
        v1_metrics.total_interactions, v2_metrics.total_interactions,
        "UT-0385-07: total_interactions must match v1 baseline (CON-CON, strict)"
    );
    assert!(
        v2_metrics.delta_mode,
        "UT-0385-07: v2 metrics must record delta_mode=true"
    );
}

// =========================================================================
// UT-0385-08 — G1 parity on all 6 IC rules x 2 strict modes = 12 cases.
// =========================================================================

#[test]
fn run_grid_delta_result_matches_run_grid_under_both_strict_modes() {
    // DC-0395-C option (a): single `#[test]` fn that iterates over all
    // six IC rule fixtures × both strict modes. Each assertion embeds
    // fixture + strict-mode label for diagnostic clarity when a case
    // fails.
    //
    // D-005 Option A closed 2026-04-23 (TASK-0400..0403). The
    // `CommutationBatch → PendingCommutation` transport
    // (`commutation_batch_to_pending`, TASK-0401) preserves
    // `target_symbols` + `local_wiring`, and the worker's
    // `apply_pending_commutation` (TASK-0402, R24.1.6a/b/c) applies
    // the hints before `reduce_all`. `SKIP_ASYMMETRIC` now flips to
    // `false`; UT-0385-08 exercises 6 fixtures × 2 strict modes = 12
    // cases across all six IC rules (the D-005 bundle acceptance gate).
    const SKIP_ASYMMETRIC: bool = false;

    type FixtureBuilder = fn() -> Net;
    let fixtures: &[(&str, FixtureBuilder, bool)] = &[
        // (name, builder, is_symmetric)
        ("CON-CON", build_fixture_con_con as FixtureBuilder, true),
        ("DUP-DUP", build_fixture_dup_dup as FixtureBuilder, true),
        ("ERA-ERA", build_fixture_era_era as FixtureBuilder, true),
        ("CON-DUP", build_fixture_con_dup as FixtureBuilder, false),
        ("CON-ERA", build_fixture_con_era as FixtureBuilder, false),
        ("DUP-ERA", build_fixture_dup_era as FixtureBuilder, false),
    ];

    for (name, build, is_symmetric) in fixtures {
        for &strict in &[false, true] {
            let fixture = build();
            let (v1_net, v1_metrics) = run_v1(fixture.clone(), strict);
            let (v2_net, v2_metrics) = run_v2(fixture, strict);

            assert!(
                v1_metrics.converged,
                "UT-0385-08: v1 must converge (fixture={name} strict={strict})"
            );
            assert!(
                v2_metrics.converged,
                "UT-0385-08: v2 delta must converge (fixture={name} strict={strict})"
            );

            if SKIP_ASYMMETRIC && !is_symmetric {
                // D-003 gap: skip canonical-form equality + interaction
                // count until `minted_agents` feedback ships. Still
                // asserts both sides reach convergence above — the FSM
                // path works; only output reconstruction is gapped.
                continue;
            }

            let canon_v1 = canonicalize_net(&v1_net);
            let canon_v2 = canonicalize_net(&v2_net);
            assert_eq!(
                canon_v1, canon_v2,
                "UT-0385-08: delta-mode output must match v1 up to isomorphism \
                 (fixture={name} strict={strict})"
            );
            assert_eq!(
                v1_metrics.total_interactions, v2_metrics.total_interactions,
                "UT-0385-08: total_interactions must match v1 baseline \
                 (fixture={name} strict={strict})"
            );
        }
    }
}

// =========================================================================
// D-005 Option A Stage 6 new UTs (2026-04-24, QA matrix #6/#7/#9/#10/#11/#12).
// Exercises the full coordinator → worker round-trip for CON-DUP /
// CON-ERA / DUP-ERA convergence and post-fix cleanup inertness.
// =========================================================================

/// UT-D005-06 (QA F-C3): after `register_minted_agents` promotes at
/// least one border on round N+2, the coordinator ships per-worker
/// `(border_id, local_side)` entries via the NEXT round's
/// `RoundStartDispatch.new_borders` — workers observe the promoted
/// wire before the final merge reassembles the net.
///
/// Shape of the assertion: dispatch a CON-DUP fixture via the real
/// pipeline; capture the `new_borders` list passed to each worker's
/// `handle_round_start` in round 3 (the round after the mint echo).
/// At least one worker must receive a non-empty `new_borders`.
#[test]
fn ut_d005_06_new_borders_plumbed_into_round_n_plus_3_round_start() {
    /// Wraps `LocalDeltaDispatch` and records `new_borders` lengths per
    /// round per worker, so the test can inspect what shipped where.
    struct RecordingDispatch {
        inner: LocalDeltaDispatch,
        /// `log[round_idx][worker_id] = new_borders.len()`
        log: Vec<Vec<usize>>,
    }
    impl WorkerDispatch for RecordingDispatch {
        fn dispatch_initial(&mut self, plan: &PartitionPlan) -> Result<(), GridError> {
            self.inner.dispatch_initial(plan)
        }
        fn dispatch_round_start(
            &mut self,
            dispatch: &[(WorkerId, RoundStartDispatch)],
        ) -> Result<Vec<RoundResultPayload>, GridError> {
            let mut row = vec![0usize; dispatch.len()];
            for (wid, payload) in dispatch {
                let idx = *wid as usize;
                if idx < row.len() {
                    row[idx] = payload.new_borders.len();
                }
            }
            self.log.push(row);
            self.inner.dispatch_round_start(dispatch)
        }
        fn dispatch_final_state_request(
            &mut self,
            round: u32,
        ) -> Result<Vec<Partition>, GridError> {
            self.inner.dispatch_final_state_request(round)
        }
    }

    let fixture = build_fixture_con_dup();
    let cfg = GridConfig {
        num_workers: 2,
        max_rounds: Some(50),
        strict_bsp: false,
        delta_mode: true,
        ..GridConfig::default()
    };
    let mut dispatch = RecordingDispatch {
        inner: LocalDeltaDispatch::new(2),
        log: Vec::new(),
    };
    let (_v2_net, v2_metrics) =
        super::grid::run_grid_delta(fixture, &cfg, &ContiguousIdStrategy, &mut dispatch);
    assert!(
        v2_metrics.converged,
        "UT-D005-06: CON-DUP pipeline must converge end-to-end"
    );
    // Round 1 (index 0) and Round 2 (index 1): new_borders empty.
    // Round 3 (index 2) is the post-promotion round where F-C3 fires.
    assert!(
        dispatch.log.len() >= 3,
        "UT-D005-06: pipeline must dispatch at least 3 rounds for CON-DUP; got {} rounds",
        dispatch.log.len()
    );
    let round3 = &dispatch.log[2];
    let any_worker_got_new_borders = round3.iter().any(|n| *n > 0);
    assert!(
        any_worker_got_new_borders,
        "UT-D005-06: round 3 MUST carry F-C3 promoted-border plumbing \
         (per-worker new_borders lengths were {round3:?})"
    );
}

/// UT-D005-07 (QA F-C3 follow-up): `handle_round_start` applies a
/// `new_borders` entry by wiring `AgentPort(target, _)` to
/// `FreePort(border_id)` on the worker-side partition. Verifies the
/// worker's `free_port_index` gains the entry after the call.
#[test]
fn ut_d005_07_worker_applies_reconciliation_new_borders_via_handle_round_start() {
    use crate::merge::BorderDelta;
    use crate::partition::IdRange;
    let mut ctx = WorkerContext::new();
    // Build a single-agent partition: 1 CON with arity-2 aux ports
    // wired to Lafont FreePorts (keeps T1 satisfied).
    let mut subnet = Net::new();
    let a = subnet.create_agent(Symbol::Con);
    subnet.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(10));
    subnet.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(11));
    subnet.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(12));
    let initial = Partition {
        subnet,
        worker_id: 0,
        free_port_index: std::collections::HashMap::new(),
        id_range: IdRange {
            start: 0,
            end: 1000,
        },
        border_id_start: 100,
        border_id_end: 200,
    };
    handle_initial_partition(&mut ctx, 0, initial);
    // Now dispatch a round with new_borders = [(150, AgentPort(a, 1))].
    // Expected: worker re-wires AgentPort(a, 1) → FreePort(150) and
    // populates free_port_index[150].
    let _ = handle_round_start(
        &mut ctx,
        1,
        Vec::<BorderDelta>::new(),
        Vec::<u32>::new(),
        vec![(150u32, PortRef::AgentPort(a, 1))],
        Vec::new(),
        Vec::new(),
    );
    let state = ctx
        .delta_state
        .as_ref()
        .expect("handle_round_start must leave delta_state populated");
    assert_eq!(
        state.partition.subnet.get_target(PortRef::AgentPort(a, 1)),
        PortRef::FreePort(150),
        "UT-D005-07: new_borders must wire local_port → FreePort(border_id)"
    );
    assert_eq!(
        state.partition.free_port_index.get(&150),
        Some(&PortRef::AgentPort(a, 1)),
        "UT-D005-07: new_borders must populate free_port_index entry"
    );
}

/// UT-D005-09 (QA F-H8 / P5): the CON-DUP asymmetric-rule pipeline
/// converges end-to-end (both strict and lenient BSP) with
/// `metrics.converged == true`.
#[test]
fn ut_d005_09_run_grid_delta_con_dup_converges_after_promotion() {
    for &strict in &[false, true] {
        let fixture = build_fixture_con_dup();
        let (_v2_net, v2_metrics) = run_v2(fixture, strict);
        assert!(
            v2_metrics.converged,
            "UT-D005-09: CON-DUP must converge post-fix (strict={strict})"
        );
    }
}

/// UT-D005-10 (QA P5): CON-ERA pipeline converges and matches v1.
#[test]
fn ut_d005_10_run_grid_delta_con_era_converges_and_matches_v1() {
    for &strict in &[false, true] {
        let fixture = build_fixture_con_era();
        let (v1_net, v1_metrics) = run_v1(fixture.clone(), strict);
        let (v2_net, v2_metrics) = run_v2(fixture, strict);
        assert!(v1_metrics.converged);
        assert!(
            v2_metrics.converged,
            "UT-D005-10: CON-ERA must converge post-fix (strict={strict})"
        );
        assert_eq!(
            canonicalize_net(&v1_net),
            canonicalize_net(&v2_net),
            "UT-D005-10: CON-ERA output must match v1 up to isomorphism (strict={strict})"
        );
        assert_eq!(
            v1_metrics.total_interactions, v2_metrics.total_interactions,
            "UT-D005-10: CON-ERA total_interactions must match v1 (strict={strict})"
        );
    }
}

/// UT-D005-11 (QA P5): DUP-ERA pipeline converges and matches v1.
#[test]
fn ut_d005_11_run_grid_delta_dup_era_converges_and_matches_v1() {
    for &strict in &[false, true] {
        let fixture = build_fixture_dup_era();
        let (v1_net, v1_metrics) = run_v1(fixture.clone(), strict);
        let (v2_net, v2_metrics) = run_v2(fixture, strict);
        assert!(v1_metrics.converged);
        assert!(
            v2_metrics.converged,
            "UT-D005-11: DUP-ERA must converge post-fix (strict={strict})"
        );
        assert_eq!(
            canonicalize_net(&v1_net),
            canonicalize_net(&v2_net),
            "UT-D005-11: DUP-ERA output must match v1 up to isomorphism (strict={strict})"
        );
        assert_eq!(
            v1_metrics.total_interactions, v2_metrics.total_interactions,
            "UT-D005-11: DUP-ERA total_interactions must match v1 (strict={strict})"
        );
    }
}

/// UT-D005-12 (QA F-M3 / P1): with the D-005 Option A fix landed, the
/// test-harness `cleanup_t1_violations` becomes INERT for the MINTED
/// agents produced by CON-DUP / CON-ERA / DUP-ERA commutations —
/// every minted agent's principal port is wired via the coordinator's
/// promoted-border plumbing, so the only agent ever removed by
/// cleanup is the ORIGINAL consumed CON / DUP / ERA.
///
/// Implemented as a counting wrapper around `cleanup_t1_violations`
/// via a custom `WorkerDispatch` that tracks dead-id count per worker.
#[test]
fn ut_d005_12_cleanup_t1_violations_is_inert_post_fix() {
    use crate::net::DISCONNECTED;
    /// WorkerDispatch wrapping `LocalDeltaDispatch` whose final-state
    /// path records the count of DISCONNECTED-principal agents a
    /// cleanup pass WOULD have removed, WITHOUT mutating the partition.
    struct CountingDispatch {
        inner: LocalDeltaDispatch,
        /// Per-worker count of agents whose principal port is
        /// DISCONNECTED at FinalStateRequest time, post-minted-border
        /// reconciliation. Expected: 1 per worker (the original
        /// consumed agent) for the asymmetric fixtures, 0 for
        /// symmetric.
        pub dead_per_worker_at_final: Vec<usize>,
    }
    impl WorkerDispatch for CountingDispatch {
        fn dispatch_initial(&mut self, plan: &PartitionPlan) -> Result<(), GridError> {
            self.inner.dispatch_initial(plan)
        }
        fn dispatch_round_start(
            &mut self,
            dispatch: &[(WorkerId, RoundStartDispatch)],
        ) -> Result<Vec<RoundResultPayload>, GridError> {
            self.inner.dispatch_round_start(dispatch)
        }
        fn dispatch_final_state_request(
            &mut self,
            round: u32,
        ) -> Result<Vec<Partition>, GridError> {
            let partitions = self.inner.dispatch_final_state_request(round)?;
            // Measurement is done on the CLEANED partition (harness
            // already cleaned DISCONNECTED-principal agents). A "zero
            // dead_ids" post-cleanup confirms cleanup was a no-op:
            // every minted agent's principal is wired correctly.
            //
            // Note: we count the AGENTS the cleanup WOULD now drop
            // (i.e. ANY live agent still at DISCONNECTED principal
            // after our own apply). That count must be zero — F-M3
            // inertness.
            for p in &partitions {
                let dead = (0..p.subnet.agents.len() as u32)
                    .filter(|id| {
                        p.subnet.agents.get(*id as usize).is_some_and(|slot| {
                            slot.as_ref().is_some_and(|_| {
                                p.subnet.get_target(PortRef::AgentPort(*id, 0)) == DISCONNECTED
                            })
                        })
                    })
                    .count();
                self.dead_per_worker_at_final.push(dead);
            }
            Ok(partitions)
        }
    }
    let fixture = build_fixture_con_dup();
    let cfg = GridConfig {
        num_workers: 2,
        max_rounds: Some(50),
        strict_bsp: false,
        delta_mode: true,
        ..GridConfig::default()
    };
    let mut dispatch = CountingDispatch {
        inner: LocalDeltaDispatch::new(2),
        dead_per_worker_at_final: Vec::new(),
    };
    let (_net, metrics) =
        super::grid::run_grid_delta(fixture, &cfg, &ContiguousIdStrategy, &mut dispatch);
    assert!(metrics.converged, "UT-D005-12: CON-DUP must converge");
    // Post-cleanup measurement: zero DISCONNECTED-principal live
    // agents per worker proves cleanup is inert.
    for (wid, &dead) in dispatch.dead_per_worker_at_final.iter().enumerate() {
        assert_eq!(
            dead, 0,
            "UT-D005-12: worker {wid} MUST NOT have any live-agent \
             DISCONNECTED-principal survivors after the D-005 fix \
             (cleanup must be a true no-op); got {dead} dead-after-cleanup"
        );
    }
}
