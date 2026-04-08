# TEST-SPEC-0142: Define ObservabilityConfig struct

**Task:** TASK-0142
**Spec:** SPEC-11
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: default_coordinator has Coordinator role

**Input:** `ObservabilityConfig::default_coordinator().role`
**Expected:** `ProcessRole::Coordinator`
**Verifies:** R31

### T2: default_worker has Worker role

**Input:** `ObservabilityConfig::default_worker().role`
**Expected:** `ProcessRole::Worker`
**Verifies:** R31

### T3: default_local has Local role

**Input:** `ObservabilityConfig::default_local().role`
**Expected:** `ProcessRole::Local`
**Verifies:** R33a

### T4: Default log format is Text

**Input:** `ObservabilityConfig::default_coordinator().log_format`
**Expected:** `LogFormat::Text`
**Verifies:** Text is the default format

### T5: Default metrics port is 9090 (feature-gated)

**Input:** `ObservabilityConfig::default_coordinator().metrics_port` (with `--features metrics`)
**Expected:** `9090`
**Verifies:** R20 default port

### T6: ObservabilityConfig derives Debug and Clone

**Input:** `let c = ObservabilityConfig::default_coordinator(); let c2 = c.clone(); format!("{:?}", c2);`
**Expected:** Compiles and does not panic
**Verifies:** Required derives

---

## Edge Cases

### E1: Metrics fields absent without feature

**Verify:** When compiled without `--features metrics`, `ObservabilityConfig` does not have `metrics_port` or `metrics_bind` fields.
**Why:** Feature-gated fields must not exist when feature is disabled.

### E2: Config is re-exported from mod.rs

**Verify:** `use relativist::observability::ObservabilityConfig;` compiles.
**Why:** Public API accessibility.
