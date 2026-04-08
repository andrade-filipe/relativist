# TEST-SPEC-0112: Implement run_coordinator_command (coordinator entry point)

**Task:** TASK-0112
**Spec:** SPEC-07, SPEC-13
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Coordinator command completes with mock workers

**Type:** Integration test (async)
**Input:**
```
// Create a small net file; spawn 2 worker tasks
let args = CoordinatorArgs {
    workers: 2,
    bind: "127.0.0.1:0".parse().unwrap(),
    input: net_path.clone(),
    max_rounds: Some(10),
    output: Some(output_path.clone()),
    metrics: Some(metrics_path.clone()),
    strategy: "round-robin".to_string(),
    log_format: None,
};
run_coordinator_command(args).await.unwrap();
```
**Expected:** `Ok(())`; output file and metrics file created
**Verifies:** R13 -- coordinator lifecycle end-to-end

### T2: Missing input file returns error with exit code 1

**Type:** Integration test (async)
**Input:** `CoordinatorArgs { input: PathBuf::from("/nonexistent.bin"), .. }`
**Expected:** `Err(e)` where `e.exit_code() == 1`
**Verifies:** R14 -- net file not found produces clear diagnostic

### T3: Output file written after distributed reduction

**Type:** Integration test (async)
**Input:** Run coordinator with `--output result.bin`
**Expected:** `result.bin` exists and is deserializable to a valid Net
**Verifies:** R24 -- output written after completion

### T4: Metrics file written after distributed reduction

**Type:** Integration test (async)
**Input:** Run coordinator with `--metrics metrics.json`
**Expected:** `metrics.json` exists and contains valid JSON with network metric fields (bytes_sent, bytes_received)
**Verifies:** R27 -- metrics output for distributed mode

### T5: Function is async

**Type:** Compilation test
**Input:** `run_coordinator_command(args).await`
**Expected:** Compiles with `.await`
**Verifies:** Coordinator uses tokio for TCP operations

---

## Edge Cases

### E1: Corrupt input file returns error with path in message

**Verify:** A file containing random bytes produces an error message that includes the file path.
**Why:** R14 -- diagnostic messages must be actionable.

### E2: Print summary called after successful completion

**Verify:** After `run_coordinator_command` completes, the execution summary has been printed to stdout.
**How:** Capture stdout and check for "Relativist Execution Summary" or equivalent.
**Why:** SPEC-07 R15 -- summary is always printed.
