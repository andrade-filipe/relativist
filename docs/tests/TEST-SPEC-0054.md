# TEST-SPEC-0054: Debug assertions for C2 (wire coverage) and C3 (FreePort bijectivity)

**Task:** TASK-0054
**Spec:** SPEC-04
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: C3 passes with correct 1-border split
Net with 2 agents in different partitions, 1 border wire (bid=10). Both partitions have `free_port_index` containing bid 10. Call `assert_c3(&partitions, &borders)`. Must not panic.

### T2: C3 panics when borderId in only 1 partition
Border map has bid=10, but only partition 0 has bid 10 in its `free_port_index`. Call `assert_c3`. Expected: panic with message indicating C3 violation.

### T3: C3 panics when borderId in 3 partitions
Border map has bid=10. Three partitions all have bid 10 in their `free_port_index`. Call `assert_c3`. Expected: panic indicating borderId appears in more than 2 partitions.

### T4: C2 passes with correct split (all wires accounted for)
Net with 4 agents, 2 internal wires, 1 border wire. Partitions correctly reflect all connections. Call `assert_c2(&net, &partitions, &borders)`. Must not panic.

### T5: C2 panics when a wire is dropped
Net with 2 agents connected. Partition subnet has one agent but its port is `DISCONNECTED` instead of `FreePort(bid)` (wire was lost). Call `assert_c2`. Expected: panic indicating wire coverage violation.

### T6: C3 passes with empty border map
No border wires. `borders` is empty. Call `assert_c3(&partitions, &borders)`. Must not panic (vacuously true).

## Edge Cases

### E1: Assertions compiled out in release mode
Both `assert_c2` and `assert_c3` are gated by `#[cfg(debug_assertions)]`. In release builds, they do not exist.

### E2: C3 with many border IDs
Border map has 50 entries. Each bid appears in exactly 2 partitions. Call `assert_c3`. Must not panic. Verifies iteration over large border maps works correctly.
