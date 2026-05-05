# TEST-SPEC-0141: Define LogFormat and ProcessRole enums

**Task:** TASK-0141
**Spec:** SPEC-11
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: LogFormat::default returns Text

**Input:** `LogFormat::default()`
**Expected:** `LogFormat::Text`
**Verifies:** R3 -- text is the default format

### T2: ProcessRole variants are distinct

**Input:** `ProcessRole::Coordinator != ProcessRole::Worker && ProcessRole::Local != ProcessRole::Coordinator`
**Expected:** `true`
**Verifies:** Enum variants are correctly differentiated

### T3: LogFormat is Copy and Clone

**Input:** `let f = LogFormat::Text; let f2 = f; let f3 = f.clone();`
**Expected:** Compiles -- all three are usable after assignment
**Verifies:** Copy and Clone derives

### T4: ProcessRole is Copy and Clone

**Input:** `let r = ProcessRole::Worker; let r2 = r; let r3 = r.clone();`
**Expected:** Compiles -- Copy and Clone work
**Verifies:** Required trait derives

### T5: LogFormat has exactly 2 variants

**Input:** Exhaustive match: `match format { LogFormat::Text => ..., LogFormat::Json => ... }`
**Expected:** Compiles without "non-exhaustive" warning
**Verifies:** No unexpected variants

### T6: ProcessRole has exactly 3 variants including Local

**Input:** Exhaustive match: `match role { ProcessRole::Coordinator => ..., ProcessRole::Worker => ..., ProcessRole::Local => ... }`
**Expected:** Compiles without "non-exhaustive" warning
**Verifies:** R33a -- Local variant exists

---

## Edge Cases

### E1: Both enums are in config.rs

**Verify:** `LogFormat` and `ProcessRole` are defined in `src/observability/config.rs`.
**Why:** Module organization per spec.

### E2: Both enums are re-exported from mod.rs

**Verify:** `use relativist::observability::LogFormat;` and `use relativist::observability::ProcessRole;` compile.
**Why:** Public API accessibility.
