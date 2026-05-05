# TEST-SPEC EG-U1: hybrid coordinator, single machine

**SPEC-20 §7.1 ID:** EG-U1
**Owning task(s):** TASK-0422 (event loop), TASK-0423 (self-worker spawn), TASK-0425 (solo loop), TASK-0430 (orchestrator).
**Type:** unit (single-binary, hybrid mode, K=0 remote workers).
**Test name:** `test_hybrid_coordinator_single_machine`.

---

## Inputs / Fixtures

- `GridConfig { hybrid_coordinator: true, elastic_join: false, elastic_departure: false, .. defaults }`.
- A small terminating `Net` (e.g. `ep_annihilation_con(N=4)` from SPEC-09 generators).
- Coordinator started with 0 remote workers.

## Expected behaviour

Coordinator detects K=0, spawns the in-process self-worker via `ChannelTransport`, runs the partition through hybrid `run_grid` with `K_eff = 1`, and returns a final `Net` whose canonical form equals `reduce_all(input_net)`.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `run_grid(net.clone(), GridConfig{ hybrid_coordinator: true, ..}, 0_remote)` returns `Ok(Net)`. |
| A2 | `canonicalise(result) == canonicalise(reduce_all(net.clone()))`. |
| A3 | `metrics.total_interactions == reduce_all_metrics.total_interactions` (within fixed permutation). |
| A4 | The `WorkerRoundStats` for `WorkerId(0)` has `is_coordinator_self == true`. |
| A5 | No remote TCP listener was bound (coordinator may still bind for parallel testing — assertion: no remote worker connected). |

## Edge / negative cases

- EC-1: input `net` is already in normal form (0 redexes) — the coordinator returns immediately with `result == net` after one trivial round. metrics: `rounds == 1` or `0` (whichever the FSM defines as "no work to do"); document the choice.
- EC-2: `net` with a single redex — exactly one self-round suffices.
- EC-3: `hybrid_coordinator = false` AND K=0 — coordinator MUST behave per v1: hang on `worker_connect_timeout` (covered by EG-U18 via R6 supersession; here we assert the contrast).

## Invariants asserted

- T1-T7 (per-agent slot validity preserved through reduction).
- D1 (Split/Merge Identity) — at K_eff=1 the cycle is trivial but must not corrupt the net.
- G1 (Fundamental Property) — `reduce_all(net) ~ run_grid(net, hybrid, K=0)`.

## ARG/DISC/REF citation

ARG-001 (G1 anchor); none additional.

## Determinism notes

The hybrid event loop uses `tokio::select!`. To make the test deterministic:
- Use `#[tokio::test(flavor = "current_thread", start_paused = true)]` so the test owns the runtime clock.
- The self-worker reduction completes on a single `spawn_blocking` step; only one event arm fires before `CheckTermination`.
- Result equality is checked via canonical-form comparison (`canonicalise`), not pointer equality.

## Cross-test dependencies

- Shares the canonicalisation helper with EG-U2, EG-U3, EG-U4, EG-I1, EG-P1.
- TASK-0414's enums (`AcceptingMembershipChanges`, `SoloReducing`) and TASK-0426's `TimerKind` are imported.
