# TEST-SPEC-0090: Implement coordinator collect phase (receive results)

**Task:** TASK-0090
**Spec:** SPEC-06
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Collect results from all workers

**Type:** Integration test (async)
**Input:**
```
// 3 mock workers each send PartitionResult { round: 0, partition: reduced_partition_i, stats: stats_i }
let (results, bytes_received) = collect_results(&mut streams, 0, DEFAULT_MAX_PAYLOAD_SIZE, Duration::from_secs(60)).await.unwrap();
```
**Expected:** `results.len() == 3`; each (partition, stats) pair matches the sent data
**Verifies:** R22 -- coordinator collects results from all workers

### T2: Worker Error message aborts collection

**Type:** Integration test (async)
**Input:** Worker 1 sends `Message::Error { round: 0, worker_id: 1, description: "OOM" }`
**Expected:** Returns an error (CoordinatorError or ProtocolError variant containing the worker error info)
**Verifies:** R25 -- worker error causes abort

### T3: Unexpected message type returns UnexpectedMessage

**Type:** Integration test (async)
**Input:** Worker sends `Message::AssignPartition` (wrong direction -- only coordinator sends this)
**Expected:** `Err(ProtocolError::UnexpectedMessage { expected: "PartitionResult", received: "AssignPartition" })`
**Verifies:** Protocol FSM validation

### T4: Timeout returns ProtocolError::Timeout

**Type:** Integration test (async)
**Input:** Set `collect_timeout` to 100ms; mock workers never send results
**Expected:** `Err(ProtocolError::Timeout { phase: "collect", .. })`
**Verifies:** R30 -- collect_timeout is MUST (upgraded in SPEC-06 v3)

### T5: Bytes received tracks total frame sizes

**Type:** Integration test (async)
**Input:** 2 workers send known-size PartitionResult messages
**Expected:** `bytes_received == sum of (FRAME_HEADER_SIZE + payload_size) for each worker`
**Verifies:** R34 -- byte counts for network metrics

### T6: Round number mismatch is rejected

**Type:** Integration test (async)
**Input:** Worker sends `PartitionResult { round: 5, .. }` but coordinator expects round 0
**Expected:** Error indicating round mismatch (protocol desynchronization)
**Verifies:** Round number validation prevents state corruption

---

## Edge Cases

### E1: Connection lost during collection

**Verify:** If one worker's stream closes mid-read, `collect_results` returns `ProtocolError::ConnectionLost`.
**Why:** R25 -- connection loss aborts the grid loop.

### E2: Single worker collection

**Verify:** `collect_results` with 1 worker stream succeeds normally.
**Why:** Degenerate case: collection from a single worker should work identically.
