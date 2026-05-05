# TEST-SPEC-0040: WorkerId type and IdRange struct

**Task:** TASK-0040
**Spec:** SPEC-04 Section 4.1
**Module:** `src/partition/types.rs`

---

## Unit Tests

### WorkerId

| ID | Test | Expected |
|----|------|----------|
| T1 | `WorkerId` is a `u32` type alias | Type-checks with u32 values |

### IdRange

| ID | Test | Expected |
|----|------|----------|
| T2 | `IdRange` has `start` and `end` fields (both AgentId) | Compiles, fields accessible |
| T3 | `IdRange` derives Debug, Clone, Copy, PartialEq, Eq | All trait operations work |
| T4 | `IdRange` derives Serialize, Deserialize | Round-trips through bincode |
| T5 | `IdRange { start: 0, end: 100 }` represents [0, 100) exclusive | Fields store expected values |
| T6 | Empty range (start == end) is valid | Compiles, no panic |

### Edge cases

| ID | Test | Expected |
|----|------|----------|
| E1 | Full u32 range `{ start: 0, end: u32::MAX }` | No overflow, fields correct |
| E2 | Single-element range `{ start: 5, end: 6 }` | Valid construction |
