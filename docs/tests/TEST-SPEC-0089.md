# TEST-SPEC-0089: Implement coordinator distribute phase (concurrent send)

**Task:** TASK-0089
**Spec:** SPEC-06
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: All workers receive correct AssignPartition messages

**Type:** Integration test (async)
**Input:**
```
// Create 3 mock workers (TcpListener + accept)
// Create 3 test partitions
distribute_partitions(&mut streams, &partitions, 0, Duration::from_secs(60)).await.unwrap();
// Each mock worker reads one frame via recv_frame
```
**Expected:** Worker 0 receives `AssignPartition { round: 0, partition: partitions[0] }`, worker 1 receives `partitions[1]`, etc.
**Verifies:** R21 -- each worker receives its assigned partition

### T2: Bytes sent equals sum of frame sizes

**Type:** Integration test (async)
**Input:** Distribute 2 partitions of known serialized sizes
**Expected:** `bytes_sent == (FRAME_HEADER_SIZE + partition_0_size) + (FRAME_HEADER_SIZE + partition_1_size)`
**Verifies:** R34 -- total bytes includes header per frame

### T3: Timeout returns ProtocolError::Timeout

**Type:** Integration test (async)
**Input:** Create mock workers that never read (causing backpressure), set `distribute_timeout` to 100ms
**Expected:** `Err(ProtocolError::Timeout { phase: "distribute", .. })`
**Verifies:** R30 -- distribute_timeout is enforced

### T4: Send failure propagates ConnectionLost

**Type:** Integration test (async)
**Input:** Close one worker stream before calling `distribute_partitions`
**Expected:** `Err(ProtocolError::ConnectionLost(_))`
**Verifies:** R25 -- connection loss is detected and propagated

### T5: Sends are concurrent (not sequential)

**Type:** Integration test (async)
**Input:** Use 3 workers with artificial 100ms read delays. Measure total distribute time.
**Expected:** Total time < 200ms (concurrent sends overlap), NOT 300ms+ (sequential)
**Verifies:** R21 -- all sends initiated before awaiting any result

---

## Edge Cases

### E1: Single worker distribution

**Verify:** `distribute_partitions` with 1 worker and 1 partition succeeds normally.
**Why:** Degenerate case: concurrency with a single element should not cause errors.

### E2: Large partition serialization

**Verify:** A partition with 10,000 agents serializes and sends without exceeding DEFAULT_MAX_PAYLOAD_SIZE.
**Why:** Tests realistic payload sizes without hitting the size limit.
