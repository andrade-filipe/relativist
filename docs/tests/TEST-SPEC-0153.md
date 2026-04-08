# TEST-SPEC-0153: Implement coordinator metric aggregation from worker reports

**Task:** TASK-0153
**Spec:** SPEC-11
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Single worker report increments return_bytes_total

**Input:** Create `CoordinatorMetrics`, call `aggregate_worker_stats(&metrics, &stats, 1024)` where `stats` has `interactions_by_rule: [5, 3, 0, 0, 0, 0]`
**Expected:** `return_bytes_total` counter value >= 1024
**Verifies:** T5 partial -- bytes counter updated

### T2: interactions_by_rule_total aggregated by rule name

**Input:** Call `aggregate_worker_stats` with `interactions_by_rule: [10, 5, 2, 0, 0, 0]`
**Expected:** `interactions_by_rule_total` Family has entries for "CON-CON" (10), "CON-DUP" (5), "CON-ERA" (2)
**Verifies:** T5 partial -- per-rule aggregation

### T3: Two worker reports accumulate correctly

**Input:** Call `aggregate_worker_stats` twice with different stats
**Expected:** `return_bytes_total` is the sum of both `payload_bytes`; rule counters are summed
**Verifies:** Multi-worker aggregation within a round

### T4: Build without metrics feature compiles

**Input:** `cargo check` without `--features metrics`
**Expected:** `aggregate_worker_stats` function not present; compilation succeeds
**Verifies:** R10 -- feature gate

---

## Edge Cases

### E1: Zero interactions in a rule are skipped

**Verify:** When `interactions_by_rule[i] == 0`, the corresponding label is not incremented (no zero-value counter entries).
**Why:** Avoids unnecessary label creation for unused rules.

### E2: All 6 rule names are mapped correctly

**Verify:** Rule indices 0-5 map to: `"CON-CON"`, `"CON-DUP"`, `"CON-ERA"`, `"DUP-DUP"`, `"DUP-ERA"`, `"ERA-ERA"`.
**Why:** Rule name mapping must match the SPEC-02 rule definitions.
