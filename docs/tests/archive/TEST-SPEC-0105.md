# TEST-SPEC-0105: Implement metrics output (JSON and CSV)

**Task:** TASK-0105
**Spec:** SPEC-07
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: JSON output is valid and round-trips

**Type:** Unit test
**Input:**
```
let metrics = test_grid_metrics_with_3_rounds();
let path = tmp_dir.path().join("metrics.json");
write_metrics(&metrics, &path).unwrap();
let content = std::fs::read_to_string(&path).unwrap();
let _: serde_json::Value = serde_json::from_str(&content).unwrap();
```
**Expected:** File exists; content is valid JSON; contains keys like `rounds`, `converged`, `total_interactions`
**Verifies:** R27 -- JSON metrics output

### T2: CSV output has correct header

**Type:** Unit test
**Input:**
```
let metrics = test_grid_metrics_with_3_rounds();
let path = tmp_dir.path().join("metrics.csv");
write_metrics(&metrics, &path).unwrap();
let content = std::fs::read_to_string(&path).unwrap();
let first_line = content.lines().next().unwrap();
```
**Expected:** First line is `round,agents,local_interactions,border_interactions,border_redexes,partition_time_ms,compute_time_ms,merge_time_ms,bytes_sent,bytes_received,network_send_time_ms,network_recv_time_ms`
**Verifies:** R29 -- CSV column headers

### T3: CSV has correct number of data lines

**Type:** Unit test
**Input:** Metrics with 3 rounds
**Expected:** CSV has 1 header line + 3 data lines = 4 total lines
**Verifies:** R29 -- one line per round

### T4: Unrecognized extension defaults to JSON

**Type:** Unit test
**Input:**
```
let path = tmp_dir.path().join("metrics.txt");
write_metrics(&metrics, &path).unwrap();
let content = std::fs::read_to_string(&path).unwrap();
let _: serde_json::Value = serde_json::from_str(&content).unwrap();
```
**Expected:** File contains valid JSON despite `.txt` extension
**Verifies:** R28 -- default to JSON for unknown extensions

### T5: Local mode metrics have zero network columns in CSV

**Type:** Unit test
**Input:** Metrics with empty `bytes_sent_per_round` and `bytes_received_per_round` vectors
**Expected:** CSV network columns contain `0` for each round
**Verifies:** R21 -- local mode produces zero network metrics

### T6: JSON durations are floating-point seconds

**Type:** Unit test
**Input:** Metrics with `total_time = Duration::from_millis(1500)`
**Expected:** JSON contains `"total_time": 1.5` (or similar float representation)
**Verifies:** Duration serialization in JSON format

---

## Edge Cases

### E1: Zero rounds produces header-only CSV

**Verify:** Metrics with 0 rounds produces a CSV with only the header line (no data lines).
**Why:** Edge case: net already in normal form before any round executes.

### E2: Missing extension defaults to JSON

**Verify:** `write_metrics(&metrics, Path::new("/tmp/metrics"))` (no extension) produces valid JSON.
**Why:** `path.extension()` returns `None` for paths without extensions.
