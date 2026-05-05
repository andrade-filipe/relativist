# TEST-SPEC-0426: `TimerKind` enum `#[repr(u32)]` sentinel (NF-008)

**SPEC-20 §7 ID:** none (mechanical / sentinel).
**Owning task:** TASK-0426.
**Parent spec:** SPEC-20 §4.1.3; closes SC-022 / NF-008.
**Type:** unit (compile-time / discriminant stability).

---

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0426-01 | `timer_kind_initial_wait_is_zero` | — | `TimerKind::InitialWait as u32` | `== 0`. |
| UT-0426-02 | `timer_kind_join_window_min_is_one` | — | `TimerKind::JoinWindowMin as u32` | `== 1`. |
| UT-0426-03 | `timer_kind_join_window_max_is_two` | — | `TimerKind::JoinWindowMax as u32` | `== 2`. |
| UT-0426-04 | `timer_kind_collect_is_three` | — | `TimerKind::Collect as u32` | `== 3`. |
| UT-0426-05 | `timer_kind_repr_attribute_is_u32` | — | `std::mem::size_of::<TimerKind>()` | `== 4` (the `#[repr(u32)]` discipline). |
| UT-0426-06 | `timer_kind_partial_eq_works` | — | `assert_eq!(TimerKind::InitialWait, TimerKind::InitialWait)`; `assert_ne!(TimerKind::InitialWait, TimerKind::Collect)` | All assertions hold. |
| UT-0426-07 | `timer_kind_clone_copy` | — | `let a = TimerKind::JoinWindowMin; let b = a;` | Both bindings live (Copy semantics). |
| UT-0426-08 | `timer_kind_debug_format_contains_name` | each variant | `format!("{:?}", k)` | Output starts with the variant name. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | A future task adds a 5th variant | The four existing discriminants 0..3 MUST remain stable; a new variant takes 4 (or higher); document via test. |

## Invariants asserted

None.

## ARG/DISC/REF citation

None.

## Determinism notes

Pure synchronous; trivial.

## Cross-test dependencies

- TASK-0436 (FSM transitions), TASK-0435 (join-window timers), TASK-0425 (solo timer) each call `TimerKind::X as u32` and convert back via index — keeping these discriminants stable is a contract.
