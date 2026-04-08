# TEST-SPEC-0113: Implement run_worker_command (worker entry point)

**Task:** TASK-0113
**Spec:** SPEC-07, SPEC-13
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Malformed coordinator address returns error

**Type:** Unit test
**Input:** `WorkerArgs { coordinator: "not-an-address".to_string(), log_format: None }`
**Expected:** `Err(e)` where `e.exit_code() == 1` (Config error)
**Verifies:** R16 -- invalid address format produces exit code 1

### T2: Worker connects and shuts down cleanly

**Type:** Integration test (async)
**Input:**
```
// Start a mock coordinator that accepts 1 connection and sends Shutdown
let args = WorkerArgs { coordinator: format!("127.0.0.1:{}", port), log_format: None };
run_worker_command(args).await.unwrap();
```
**Expected:** `Ok(())`
**Verifies:** R17 -- worker lifecycle: connect, work, shutdown

### T3: Connection failure returns exit code 2

**Type:** Integration test (async)
**Input:** `WorkerArgs { coordinator: "127.0.0.1:1".to_string(), .. }` (no listener)
**Expected:** `Err(e)` where `e.exit_code() == 2` (Communication error)
**Verifies:** R17 -- connection failure after retries

### T4: Function is async

**Type:** Compilation test
**Input:** `run_worker_command(args).await`
**Expected:** Compiles with `.await`
**Verifies:** Worker uses tokio for TCP operations

### T5: Shutdown logged at INFO level

**Type:** Integration test (async)
**Input:** Run worker that receives Shutdown; capture tracing output
**Expected:** Log contains "worker shutdown complete" at INFO level
**Verifies:** Acceptance criteria: shutdown logging

---

## Edge Cases

### E1: IPv6 coordinator address

**Verify:** `WorkerArgs { coordinator: "[::1]:9000".to_string(), .. }` parses successfully (may fail to connect if no listener, but parsing succeeds).
**Why:** IPv6 support via standard SocketAddr parsing.

### E2: Worker has no --output or --metrics flags

**Verify:** `WorkerArgs` struct does NOT have `output` or `metrics` fields.
**How:** Attempting to set `args.output` fails to compile.
**Why:** Workers only reduce and return partitions; output and metrics are coordinator responsibilities.
