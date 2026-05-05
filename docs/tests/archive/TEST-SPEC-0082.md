# TEST-SPEC-0082: Define Message enum

**Task:** TASK-0082
**Spec:** SPEC-06
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: All 7 variants constructible and serializable

**Type:** Unit test
**Input:** Construct each variant (`AssignPartition`, `Shutdown`, `PartitionResult`, `Error`, `Register`, `RegisterAck`, `RegisterNack`) and call `bincode::serialize(&msg)`
**Expected:** Each serialization succeeds (returns `Ok(Vec<u8>)`)
**Verifies:** R4 -- all variants are serde-serializable via bincode

### T2: Round-trip serialization for AssignPartition

**Type:** Unit test
**Input:**
```
let msg = Message::AssignPartition { round: 5, partition: test_partition() };
let bytes = bincode::serialize(&msg).unwrap();
let decoded: Message = bincode::deserialize(&bytes).unwrap();
```
**Expected:** `decoded` matches the original (round == 5, partition fields match)
**Verifies:** R14 -- `deserialize(serialize(msg)) == msg`

### T3: Round-trip serialization for PartitionResult

**Type:** Unit test
**Input:**
```
let msg = Message::PartitionResult { round: 3, partition: test_partition(), stats: test_stats() };
let bytes = bincode::serialize(&msg).unwrap();
let decoded: Message = bincode::deserialize(&bytes).unwrap();
```
**Expected:** Decoded round == 3, stats fields match (worker_id, agents_before, agents_after, local_redexes, reduce_duration_secs, interactions_by_rule)
**Verifies:** R14 for PartitionResult with 6-field WorkerRoundStats

### T4: Shutdown serialized size is minimal

**Type:** Unit test
**Input:** `let bytes = bincode::serialize(&Message::Shutdown).unwrap();`
**Expected:** `bytes.len() <= 8` (just the enum discriminant, no payload)
**Verifies:** Shutdown variant has no fields, so serialized size is small

### T5: Error variant contains required fields

**Type:** Unit test
**Input:**
```
let msg = Message::Error { round: 0, worker_id: 42, description: "OOM".to_string() };
```
**Expected:** Compiles; round == 0, worker_id == 42, description == "OOM"
**Verifies:** Error variant has round, worker_id, description fields

### T6: Message derives Debug and Clone

**Type:** Unit test
**Input:** `let msg = Message::Shutdown; let _ = format!("{:?}", msg); let _ = msg.clone();`
**Expected:** Both operations succeed
**Verifies:** `#[derive(Debug, Clone)]` on Message

---

## Edge Cases

### E1: Variant discriminant stability (append-only)

**Verify:** New variants (Register, RegisterAck, RegisterNack) are appended after the 4 core variants, not inserted in the middle.
**How:** Serialize `Message::Shutdown` with bincode, check the discriminant byte is stable across builds. Alternatively, verify source order in `types.rs`.
**Why:** R5 -- bincode discriminant stability requires append-only variant ordering.

### E2: Empty partition in AssignPartition

**Verify:** `Message::AssignPartition { round: 0, partition: empty_partition() }` serializes and deserializes correctly.
**Why:** Edge case: a partition with zero agents and no connections should still round-trip.
