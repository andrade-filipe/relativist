# TEST-SPEC EG-U16: self-partition panic → Error (R3a)

**SPEC-20 §7.1 ID:** EG-U16
**Owning task(s):** TASK-0422, TASK-0423, TASK-0436.
**Type:** unit.
**Test name:** `test_self_partition_panic_to_error`.

---

## Inputs / Fixtures

- Hybrid; K_remote=1.
- Test-only injected panic: the self-partition reduction (running in `tokio::task::spawn_blocking`) is replaced with a closure that panics immediately when invoked.

## Expected behaviour

R3a: a panic in the self-partition's `spawn_blocking` task is captured by `tokio::task::JoinHandle::await` returning `Err(JoinError::Panic)`. The FSM treats this as `SelfPartitionPanic(msg)` and transitions: `WaitingForResults × SelfPartitionPanic → Error`.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `run_grid` returns `Err(GridError::SelfPartitionPanic { msg })` with the panic payload included as a string. |
| A2 | The remote worker's stream is closed cleanly (or with a `Shutdown` per SPEC-06). |
| A3 | `metrics.workers_departed_per_round` does NOT count the self-panic as a departure (it's a different category). |
| A4 | No `unwrap()` is called inside the FSM error path; all errors propagate via `Result`. |

## Edge / negative cases

- EC-1: panic payload is non-string (e.g., a custom error type) — coordinator captures `Box<dyn Any>` and renders to a debug string.
- EC-2: panic occurs AFTER the self-partition has produced a partial border-delta — the partial result is discarded; no contamination of `bg`.
- EC-3: panic occurs during the very first round (no prior state) — `Error` transition still works; no panic propagation up the stack.

## Invariants asserted

- D6 (Protocol Termination via clean error path).

## ARG/DISC/REF citation

None.

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. Panic is injected via a test-only feature flag or a constructor parameter that allows passing a custom reduction closure.

## Cross-test dependencies

EG-U9 branch (b) (non-hybrid Error transition is similar but for a different cause).
