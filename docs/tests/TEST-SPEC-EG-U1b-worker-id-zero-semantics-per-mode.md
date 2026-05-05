# TEST-SPEC EG-U1b: WorkerId 0 semantics per mode (R2a, SC-016)

**SPEC-20 §7.1 ID:** EG-U1b
**Owning task(s):** TASK-0420 (WorkerId reservation), TASK-0436 (FSM).
**Type:** unit.
**Test name:** `test_worker_id_zero_semantics_per_mode`.

---

## Inputs / Fixtures

Two configurations:
- **Hybrid:** `GridConfig { hybrid_coordinator: true, .. }`. K=2 remote workers connect.
- **Non-hybrid:** `GridConfig { hybrid_coordinator: false, .. }`. K=2 remote workers connect.

A small net (any terminating fixture).

## Expected behaviour

- In **hybrid** mode: `WorkerId(0)` is permanently reserved for the in-process self-partition; remote workers are assigned `WorkerId(1), WorkerId(2)`.
- In **non-hybrid** mode: `WorkerId(0)` is the first remote worker; subsequent are `WorkerId(1)`, etc.

## Assertions

| # | Assertion |
|---|-----------|
| A1 (hybrid) | `WorkerId(0)` is recorded in `metrics.workers` with `is_coordinator_self == true`. |
| A2 (hybrid) | The two remote workers have `WorkerId(1)` and `WorkerId(2)` with `is_coordinator_self == false`. |
| A3 (hybrid) | `next_worker_id` after the initial cohort is `3`. |
| A4 (non-hybrid) | The first remote worker has `WorkerId(0)` and `is_coordinator_self == false`. |
| A5 (non-hybrid) | The two remotes have `WorkerId(0)` and `WorkerId(1)`; `next_worker_id == 2`. |
| A6 (cross-mode) | A late join in hybrid mode is NEVER assigned `WorkerId(0)` (R7a: reserved permanently for the self-partition role for the run's lifetime). |

## Edge / negative cases

- EC-1: hybrid mode, K=0 (no remote workers ever) — `WorkerId(0)` is the only id used.
- EC-2: non-hybrid mode, all remote workers depart and rejoin — new `WorkerId` assigned; `0` is NOT reused for the rejoiner (R11 monotonic).
- EC-3: hybrid mode, the self-partition panics (EG-U16) — `WorkerId(0)` is NOT reassigned to a remote even after panic transitions FSM to `Error`.

## Invariants asserted

- D4-elastic (R11a): `WorkerId` (sparse) is decoupled from `partition_index` (dense). Self-partition's `partition_index = 0` always; its `WorkerId == 0` only in hybrid mode.

## ARG/DISC/REF citation

None.

## Determinism notes

Async test (`#[tokio::test(flavor = "current_thread", start_paused = true)]`). Worker connect order is deterministically scripted via `tokio::io::duplex` and explicit await ordering.

## Cross-test dependencies

- EG-U12a (partition_index decoupling) shares the WorkerId/partition_index decoupling assertion.
- EG-U14 (WorkerId exhaustion) extends the monotonic discipline tested here.
