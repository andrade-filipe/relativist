# TEST-SPEC-0041: Partition struct

**Task:** TASK-0041
**Spec:** SPEC-04 Section 4.1
**Module:** `src/partition/types.rs`

---

## Unit Tests

| ID | Test | Expected |
|----|------|----------|
| T1 | `Partition` has all 6 fields (subnet, worker_id, free_port_index, id_range, border_id_start, border_id_end) | Compiles, fields accessible |
| T2 | `Partition` derives Debug, Clone, Serialize, Deserialize | All trait operations work |
| T3 | `free_port_index` is `HashMap<u32, PortRef>` | Insert and lookup work |
| T4 | `border_id_start` and `border_id_end` are `u32` | Fields store expected values |
| T5 | Partition round-trips through bincode | Serialize + deserialize equality |

### Edge cases

| ID | Test | Expected |
|----|------|----------|
| E1 | Empty partition (empty net, no borders) | Valid construction |
| E2 | Partition with empty free_port_index | Valid, HashMap is empty |
