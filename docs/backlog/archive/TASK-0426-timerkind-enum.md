# TASK-0426: `TimerKind` enum with `#[repr(u32)]` (NF-008)

**Spec:** SPEC-20 §4.1.3 (closes SC-022 and NF-008).
**Requirements:** NF-008 MUST upgrade; `TimerKind` carries `#[repr(u32)]` so `TimerId = kind as u32` is a stable, portable operation.
**Priority:** P0 (fix all symbolic timer names in FSM transition table).
**Status:** TODO
**Depends on:** TASK-0414.
**Blocked by:** TASK-0414.
**Estimated complexity:** S (~20-40 LoC + rewiring sites)
**Bundle:** SPEC-20 Elastic Grid — Phase 2.1 Hybrid Coordinator.

## Context

SPEC-13 R21's `TimerId = u32` was previously derived from implementation-private mappings ("initial_wait_timer", "join_window_timer", "collect_timer" strings). NF-008 mandates a typed `TimerKind` enum with `#[repr(u32)]` so the cast is stable and portable; test assertions and log analysis can decode `TimerId -> TimerKind` deterministically.

## Acceptance Criteria

- [ ] Add enum in `relativist-core/src/protocol/timers.rs` (or equivalent):

```rust
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerKind {
    InitialWait    = 0,
    JoinWindowMin  = 1,
    JoinWindowMax  = 2,
    Collect        = 3,
}
```

- [ ] Replace all symbolic timer names across the coordinator with `TimerKind::X`.
- [ ] Conversion: `let tid: u32 = TimerKind::InitialWait as u32;` is the canonical map.
- [ ] FSM transition table (TASK-0436) uses `StartTimer(TimerKind::X, duration)` and `CancelTimer(TimerKind::X)`.
- [ ] Add compile-time sentinel test asserting `TimerKind::InitialWait as u32 == 0`, etc.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/timers.rs` | modify | Introduce `TimerKind`; rewire all timer-arming call sites. |
| `relativist-core/src/protocol/coordinator.rs` | modify | Replace symbolic timer names. |

## Test Expectations (forward-ref)

- TEST-SPEC-0426 sentinel: `TimerKind::X as u32` values 0..3 stable.

## Invariants Touched

- None (type-surface change).

## Notes

- NF-008 is **LOW** priority from spec-critic but made MUST by SPEC-20 Round 3 closure; this task discharges it fully.

## DAG Links

- **Predecessors:** TASK-0414.
- **Successors:** TASK-0425 (solo timer), TASK-0435 (join-window timers), TASK-0436 (FSM wiring).
