# TEST-SPEC-0028: ReductionStats + reduce_all

**Task:** TASK-0028
**Spec:** SPEC-03 Section 4.6.2
**Module:** `src/reduction/engine.rs`

---

## Unit Tests

### ReductionStats struct

| ID | Test | Expected |
|----|------|----------|
| T1 | `ReductionStats` has all 7 fields (total, 4 category, interactions_by_rule) | Compiles, fields accessible |
| T2 | `ReductionStats` derives Debug, Clone | `format!("{:?}", stats)` works, `.clone()` works |
| T3 | Default initialization (all zeros) | All fields are 0 |

### reduce_all function

| ID | Test | Expected |
|----|------|----------|
| T4 | Empty net (no agents) | Returns stats with total_interactions = 0 |
| T5 | Single ERA-ERA pair | total=1, void_count=1, interactions_by_rule[5]=1 |
| T6 | Single CON-CON pair | total=1, anni_count=1, interactions_by_rule[0]=1 |
| T7 | Single CON-DUP pair (commutation) | total=1, comm_count=1, interactions_by_rule[1]=1 |
| T8 | Multi-step: CON-ERA creates 2 ERA, which form new ERA-ERA pair | total=2, eras_count=1, void_count=1 |
| T9 | Stats consistency: total == anni + comm + eras + void | Assert invariant after reduction |
| T10 | Stats consistency: total == sum(interactions_by_rule) | Assert invariant after reduction |
| T11 | Net is in normal form after reduce_all returns | `reduce_step` returns NormalForm |

### Edge cases

| ID | Test | Expected |
|----|------|----------|
| E1 | Net with agents but no redexes | Returns stats with total=0 |
| E2 | Multiple same-type redexes (3x ERA-ERA) | total=3, void_count=3, interactions_by_rule[5]=3 |
