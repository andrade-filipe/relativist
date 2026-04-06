# TEST-SPEC-0029: reduce_n (budget-limited reduction)

**Task:** TASK-0029
**Spec:** SPEC-03 Section 4.6.3
**Module:** `src/reduction/engine.rs`

---

## Unit Tests

| ID | Test | Expected |
|----|------|----------|
| T1 | Empty net with budget=10 | total_interactions=0, returns immediately |
| T2 | Single ERA-ERA with budget=10 | total=1, void_count=1 (stops at NormalForm before budget) |
| T3 | Budget=0 performs no reductions | total=0 even if redexes exist |
| T4 | Budget=1 on net with 3 ERA-ERA pairs | total=1, exactly one pair reduced |
| T5 | Budget exactly equals required steps | total=budget, net in NormalForm |
| T6 | Budget exceeds required steps | total < budget, net in NormalForm |
| T7 | Stats consistency: total == anni + comm + eras + void | Invariant holds |
| T8 | Stats consistency: total == sum(interactions_by_rule) | Invariant holds |
| T9 | Net NOT in normal form when budget < required | Remaining redexes still in queue |

### Edge cases

| ID | Test | Expected |
|----|------|----------|
| E1 | Budget=usize::MAX on small net | Terminates normally (no overflow concern at u64 scale) |
| E2 | reduce_n then reduce_all finishes the job | Combined stats cover all interactions |
