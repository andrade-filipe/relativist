# TEST-SPEC-0047: Compute static ID space ranges

**Task:** TASK-0047
**Spec:** SPEC-04
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: 1 worker -- entire u32 space
`compute_id_ranges(1)` returns `[IdRange { start: 0, end: u32::MAX }]`.

### T2: 2 workers -- two halves
`compute_id_ranges(2)` returns 2 ranges. First range starts at 0, second range ends at `u32::MAX`. Ranges are contiguous: `ranges[0].end == ranges[1].start`.

### T3: 8 workers -- 8 ranges covering full space
`compute_id_ranges(8)` returns 8 ranges. `ranges[0].start == 0`, `ranges[7].end == u32::MAX`. Each range is approximately `u32::MAX / 8` (~537M) IDs wide.

### T4: Ranges are disjoint and contiguous
For `num_workers=4`, verify `ranges[i].end == ranges[i+1].start` for all `i` in `0..3`. No overlap, no gap.

### T5: Ranges cover full u32 space
For `num_workers=3`, verify `ranges[0].start == 0` and `ranges[2].end == u32::MAX`.

### T6: Last worker gets remainder
For `num_workers=3`, chunk_size is `u32::MAX / 3 = 1431655765`. Workers 0 and 1 each get exactly `1431655765` IDs. Worker 2 extends from `2 * 1431655765` to `u32::MAX`, getting the remainder.

## Edge Cases

### E1: num_workers equals u32::MAX
`compute_id_ranges(u32::MAX)` returns `u32::MAX` ranges. Each range has approximately 1 ID. No overflow or panic.

### E2: num_workers=2 boundary correctness
`compute_id_ranges(2)`: `ranges[0] = IdRange { start: 0, end: 2147483647 }`, `ranges[1] = IdRange { start: 2147483647, end: u32::MAX }`. Verify the exact boundary value.
