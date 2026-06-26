# TEST-SPEC-0150: Define CoordinatorMetrics struct and registration

**Task:** TASK-0150
**Spec:** SPEC-11
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: CoordinatorMetrics::new() does not panic

**Input:** `CoordinatorMetrics::new()` (with `--features metrics`)
**Expected:** Returns a valid `CoordinatorMetrics` instance
**Verifies:** T5 partial -- constructor works

### T2: All 10 metrics are registered with relativist_ prefix

**Input:** Create metrics, register with a `Registry`, encode to text
**Expected:** Encoded output contains metric names: `relativist_rounds_total`, `relativist_round_duration_seconds`, `relativist_active_workers`, `relativist_partitions_dispatched_total`, `relativist_border_redexes`, `relativist_merge_duration_seconds`, `relativist_split_duration_seconds`, `relativist_dispatch_bytes_total`, `relativist_return_bytes_total`, `relativist_interactions_by_rule_total`
**Verifies:** R12, R17 -- all metrics with correct prefix

### T3: interactions_by_rule_total Family metric works

**Input:** `metrics.interactions_by_rule_total.get_or_create(&vec![("rule".to_string(), "CON-CON".to_string())]).inc();`
**Expected:** Counter increments without panic
**Verifies:** R12 -- per-rule counter via Family

### T4: Histograms use custom IC buckets

**Input:** Observe values in `round_duration_seconds`, encode registry
**Expected:** Encoded output shows bucket boundaries `[0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 30.0]`
**Verifies:** R15 -- custom histogram buckets

### T5: No partition_id or round labels

**Input:** Inspect all metric definitions
**Expected:** No metric uses `partition_id` or `round` as a label key
**Verifies:** R16 -- cardinality control

### T6: Build without metrics feature compiles

**Input:** `cargo check` without `--features metrics`
**Expected:** No metrics code emitted; compilation succeeds
**Verifies:** R10 -- feature gate

---

## Edge Cases

### E1: Register method returns self (builder pattern)

**Verify:** `CoordinatorMetrics::new().register(&mut registry)` returns `CoordinatorMetrics`.
**Why:** Builder pattern for ergonomic initialization.

### E2: Gauge can decrease

**Verify:** `metrics.active_workers` gauge can be incremented and decremented (worker connect/disconnect).
**Why:** Active workers is a gauge, not a counter.
