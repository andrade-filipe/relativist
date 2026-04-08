# TEST-SPEC-0093: Implement run_worker (worker loop)

**Task:** TASK-0093
**Spec:** SPEC-06
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Single round: receive partition, reduce, return result

**Type:** Integration test (async)
**Input:**
```
// Mock coordinator sends: AssignPartition { round: 0, partition: test_partition_with_redex() }
// Then sends: Shutdown
let result = run_worker(&config).await;
```
**Expected:** `result.is_ok()`; mock coordinator receives `PartitionResult { round: 0, partition: reduced, stats }` where `stats.agents_before > 0` and `stats.local_redexes >= 0`
**Verifies:** R18, R32 -- worker receives, reduces, and returns partition

### T2: Multiple rounds before shutdown

**Type:** Integration test (async)
**Input:** Mock coordinator sends `AssignPartition` 3 times (rounds 0, 1, 2) then `Shutdown`
**Expected:** Worker sends 3 `PartitionResult` messages, then exits with `Ok(())`
**Verifies:** Worker loop continues until Shutdown

### T3: Shutdown message causes clean exit

**Type:** Integration test (async)
**Input:** Mock coordinator sends only `Shutdown` (no AssignPartition)
**Expected:** `run_worker` returns `Ok(())`
**Verifies:** Worker handles immediate shutdown without doing any work

### T4: Unexpected message returns UnexpectedMessage

**Type:** Integration test (async)
**Input:** Mock coordinator sends `PartitionResult` (wrong direction)
**Expected:** `Err(ProtocolError::UnexpectedMessage { expected: "AssignPartition or Shutdown", .. })`
**Verifies:** FSM validation rejects invalid messages

### T5: WorkerRoundStats has all 6 fields

**Type:** Integration test (async)
**Input:** Run a single round; extract the stats from the returned PartitionResult
**Expected:** Stats contain: `worker_id`, `agents_before`, `agents_after`, `local_redexes`, `reduce_duration_secs` (>= 0.0), `interactions_by_rule` (HashMap with rule names)
**Verifies:** SPEC-06 v3 / SPEC-11 Section 4.4 -- 6-field WorkerRoundStats

---

## Edge Cases

### E1: Partition with no redexes

**Verify:** Worker receives a partition whose subnet has no redexes. `reduce_all` completes immediately; `PartitionResult` has `agents_before == agents_after` and `local_redexes == 0`.
**Why:** Tests that the worker handles empty work gracefully.

### E2: Connection lost during recv

**Verify:** If the coordinator drops the connection mid-round (after sending AssignPartition but before worker sends result), worker returns `Err(ProtocolError::ConnectionLost(_))`.
**Why:** R25 -- connection loss from worker's perspective.
