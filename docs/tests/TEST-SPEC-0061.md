# TEST-SPEC-0061: Define WorkerRoundStats struct

**Task:** TASK-0061
**Spec:** SPEC-05 (R37, SC-001)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: All 6 fields accessible
Create a `WorkerRoundStats` with concrete values: `worker_id: 3`, `agents_before: 100`, `agents_after: 80`, `local_redexes: 20`, `reduce_duration_secs: 0.042`, `interactions_by_rule: [5, 3, 2, 4, 1, 5]`. Assert each field matches the expected value.

### T2: Serialization round-trip (bincode)
Create a `WorkerRoundStats` with `worker_id: 1`, `agents_before: 50`, `agents_after: 40`, `local_redexes: 10`, `reduce_duration_secs: 1.5`, `interactions_by_rule: [2, 3, 1, 0, 4, 0]`. Serialize to bincode bytes. Deserialize back. Assert all 6 fields match the original.

### T3: Serialization round-trip (JSON)
Same as T2 but using `serde_json`. Serialize to JSON string, deserialize back, assert field equality. Confirms serde derives work for multiple formats.

### T4: Derives Debug and Clone
Create a `WorkerRoundStats`, call `format!("{:?}", stats)` to verify Debug. Call `stats.clone()` to verify Clone.

### T5: interactions_by_rule has exactly 6 elements
Create a `WorkerRoundStats` with `interactions_by_rule: [0, 0, 0, 0, 0, 0]`. Assert array length is 6. Map to CON-CON=0, CON-DUP=1, CON-ERA=2, DUP-DUP=3, DUP-ERA=4, ERA-ERA=5.

## Edge Cases

### E1: Zero values in all fields
Create `WorkerRoundStats { worker_id: 0, agents_before: 0, agents_after: 0, local_redexes: 0, reduce_duration_secs: 0.0, interactions_by_rule: [0; 6] }`. Assert all fields are zero. Serialize and deserialize, verify round-trip.

### E2: Large values
Create with `worker_id: u32::MAX`, `agents_before: usize::MAX`, `reduce_duration_secs: f64::MAX`, `interactions_by_rule: [u64::MAX; 6]`. Assert fields accessible without overflow at construction time.

### E3: Module compiles
`cargo check` passes with the `WorkerRoundStats` struct in `src/merge.rs`.
