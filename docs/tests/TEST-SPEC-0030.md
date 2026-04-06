# TEST-SPEC-0030: Wire up reduction module re-exports

**Task:** TASK-0030
**Spec:** SPEC-03 (module facade)
**Module:** `src/reduction/mod.rs`

---

## Unit Tests

| ID | Test | Expected |
|----|------|----------|
| T1 | All public types accessible via `crate::reduction::*` | Compiles |
| T2 | `reduction::Rule` and `reduction::SpecificRule` are usable | Compiles, match works |
| T3 | `reduction::reduce_all` callable | Returns ReductionStats |
| T4 | `reduction::reduce_n` callable | Returns ReductionStats |
| T5 | `reduction::reduce_step` callable | Returns StepResult |
| T6 | Individual rule functions accessible | All 4 interact_* callable |

### Edge cases

| ID | Test | Expected |
|----|------|----------|
| E1 | Internal items (link helper, tables) NOT re-exported | Module-private stays private |
