# TEST-SPEC-0170: Implement reduction summary formatting

**Task:** TASK-0170
**Spec:** SPEC-12 R20, R25, R44, R44a, R44b, R45, R46, R47
**Generated:** 2026-04-08 (retroactive)

---

## Unit Tests

### T1: Local reduction summary matches R45 format

**Type:** Unit
**Input:**
```rust
let summary = ReductionSummary {
    agents_before: 1000,
    agents_after: 42,
    redexes_before: 500,
    redexes_after: 0,
    normal_form: true,
    total_interactions: 958,
    duration_secs: 1.234,
    mips: 958.0 / 1.234 / 1_000_000.0,
};
let text = format_reduction_summary(&summary);
```
**Expected:** `text` contains:
- A header line (e.g., `"=== Relativist"`)
- `"Input:"` line with `"1000 agents"` and `"500 redexes"`
- `"Output:"` line with `"42 agents"` and `"0 redexes"` and `"(normal form)"`
- `"Interactions: 958"` (or `"Interactions:"` with `958`)
- `"Duration:"` with `"1.234s"`
- `"MIPS:"` line
**Verifies:** R45 -- human-readable reduction summary format

### T2: Output line shows "(normal form)" when normal_form is true

**Type:** Unit
**Input:** Create a `ReductionSummary` with `normal_form: true`
**Expected:** The formatted text contains `"(normal form)"` after the output stats
**Verifies:** R45 -- normal form suffix

### T3: Output line shows "(NOT normal form: reason)" when normal_form is false (R47)

**Type:** Unit
**Input:**
```rust
let summary = ReductionSummary {
    normal_form: false,
    // termination_reason field if present
    ..default_local_summary()
};
```
**Expected:** The formatted text contains `"NOT normal form"` or a similar warning with the reason (e.g., "max-interactions reached")
**Verifies:** R47 -- termination reason in output

### T4: Duration formatted as seconds with 3 decimal places

**Type:** Unit
**Input:** `ReductionSummary { duration_secs: 0.001, .. }`
**Expected:** Duration line contains `"0.001s"`
**Verifies:** TASK-0170 AC -- 3 decimal places for duration

### T5: MIPS formatted with 3 decimal places

**Type:** Unit
**Input:** `ReductionSummary { mips: 0.776123, .. }`
**Expected:** MIPS line contains `"0.776"` (truncated/rounded to 3 decimals)
**Verifies:** TASK-0170 AC -- 3 decimal places for MIPS

### T6: MIPS calculation correctness

**Type:** Unit
**Input:** `ReductionSummary { total_interactions: 1_000_000, duration_secs: 1.0, .. }`
**Expected:** `mips` field is `1.0` (1M interactions / 1s / 1M = 1.0 MIPS)
**Verifies:** MIPS = total_interactions / duration_secs / 1_000_000

### T7: Metrics JSON is valid JSON (R21)

**Type:** Unit
**Input:** Create a `ReductionSummary`, call the JSON serialization
**Expected:** Output is valid JSON parseable by `serde_json::from_str`
**Verifies:** R21 -- JSON metrics output

### T8: Metrics JSON contains required fields

**Type:** Unit
**Input:**
```rust
let summary = ReductionSummary {
    agents_before: 100, agents_after: 10,
    redexes_before: 50, redexes_after: 0,
    normal_form: true,
    total_interactions: 90,
    duration_secs: 0.5,
    mips: 0.18,
};
let json = serde_json::to_string(&summary).unwrap();
```
**Expected:** JSON contains keys: `"agents_before"`, `"agents_after"`, `"redexes_before"`, `"redexes_after"`, `"normal_form"`, `"total_interactions"`, `"duration_secs"`, `"mips"`
**Verifies:** R21 -- all required fields present in metrics JSON

---

## Edge Cases

### E1: Zero-duration reduction (instant)

**Verify:** A `ReductionSummary` with `duration_secs: 0.0` does not cause a panic or division-by-zero in formatting. The MIPS value may display as `inf` or a sentinel value.
**Why:** An empty net reduces instantly (0 interactions, 0 time).

### E2: Very large interaction count

**Verify:** A `ReductionSummary` with `total_interactions: u64::MAX` formats without overflow in the text output.
**Why:** Large benchmarks may produce billions of interactions.

### E3: Duration with high precision is truncated

**Verify:** A `ReductionSummary` with `duration_secs: 1.23456789` displays as `"1.235s"` (rounded to 3 places, not all 8).
**Why:** TASK-0170 AC specifies 3 decimal places.

---

## Property Tests

### P1: Serialization roundtrip for ReductionSummary

**Property:** For any `ReductionSummary` with valid field values, `serde_json::to_string(&summary)` succeeds and `serde_json::from_str::<serde_json::Value>(&json)` succeeds (valid JSON).
**Generator:** Random values: `agents_before` in `0..10_000`, `duration_secs` in `0.001..100.0`, etc.
**Verifies:** R21 -- JSON output is always valid
