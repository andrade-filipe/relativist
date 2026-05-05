# TEST-SPEC-0111: Implement run_local_command (local mode entry point)

**Task:** TASK-0111
**Spec:** SPEC-07
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Complete local mode pipeline

**Type:** Integration test
**Input:**
```
// Create a small net file with 10 agents and redexes
let local_args = LocalArgs {
    workers: 2,
    input: net_path.clone(),
    max_rounds: Some(100),
    output: Some(output_path.clone()),
    metrics: Some(metrics_path.clone()),
    strategy: "round-robin".to_string(),
    log_format: None,
};
run_local_command(local_args).unwrap();
```
**Expected:** Function returns `Ok(())`; output file and metrics file exist
**Verifies:** R18 -- local mode lifecycle end-to-end

### T2: Output file is deserializable

**Type:** Integration test
**Input:** After `run_local_command`, load the output .bin file
**Expected:** `load_net_from_file(&output_path).is_ok()`; the loaded net has zero or fewer redexes than the original
**Verifies:** R24 -- output format matches input format (chaining)

### T3: Metrics file is valid JSON

**Type:** Integration test
**Input:** After `run_local_command` with `--metrics metrics.json`, read the file
**Expected:** `serde_json::from_str::<Value>(&content).is_ok()`; JSON contains `"rounds"` key
**Verifies:** R27 -- metrics output

### T4: Non-existent input file returns error

**Type:** Unit test
**Input:** `LocalArgs { input: PathBuf::from("/nonexistent.bin"), .. }`
**Expected:** `Err(RelativistError::Io(_))` -- exit code 1
**Verifies:** R14 -- input file not found produces clear error

### T5: No output/metrics flags skips file writing

**Type:** Integration test
**Input:** `LocalArgs { output: None, metrics: None, .. }`
**Expected:** `Ok(())`; no files created beyond the input
**Verifies:** Optional flags are truly optional

### T6: Function is synchronous (no async)

**Type:** Compilation test
**Input:** `run_local_command(args)` -- not `.await`
**Expected:** Compiles without async; return type is `Result<(), RelativistError>`
**Verifies:** R18 -- local mode does not require tokio

---

## Edge Cases

### E1: Network metric fields are empty in local mode

**Verify:** After `run_local_command`, the metrics have empty `bytes_sent_per_round` and `bytes_received_per_round` vectors.
**Why:** R21 -- local mode has no network activity.

### E2: Invalid strategy name returns error

**Verify:** `LocalArgs { strategy: "nonexistent".to_string(), .. }` returns `Err(RelativistError::Config(_))`.
**Why:** `parse_strategy` validates the strategy name before running the grid loop.
