# TEST-SPEC-0415: `GridConfig` 9 elastic fields + defaults + validate (SPEC-05 A5)

**SPEC-20 §7 ID:** none direct (functional default behaviour gates EG-I5 via TASK-0455).
**Owning task:** TASK-0415.
**Parent spec:** SPEC-05 (amended via SPEC-20 §3.8 A5); SPEC-20 R0a, R0b, R0c, R33, R33a.
**Type:** unit.

---

## Inputs / Fixtures

- `GridConfig::default()`.
- A handful of mutated configs (`hybrid_coordinator=true`, `elastic_departure=true` with `retain_partitions=false`, inverted join-window bounds, `solo_budget=0`).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0415-01 | `grid_config_defaults_match_r33a` | — | `let c = GridConfig::default()` | `c.hybrid_coordinator == false`; `c.elastic_departure == false`; `c.retain_partitions == false` (pre-normalize); `c.elastic_join == false` (pre-normalize); `c.checkpoint_partitions == false`; `c.initial_wait_timeout == Duration::from_secs(30)`; `c.join_window_min == Duration::from_millis(50)`; `c.join_window_max == Duration::from_millis(500)`; `c.solo_budget == 10_000`. **Every** field cross-checked with its R33a value. |
| UT-0415-02 | `grid_config_derived_retain_partitions_when_elastic_departure` | `let mut c = GridConfig::default(); c.elastic_departure = true; c.retain_partitions = false;` | `c.normalize()` | `c.retain_partitions == true` after normalize. |
| UT-0415-03 | `grid_config_derived_elastic_join_when_hybrid` | `let mut c = GridConfig::default(); c.hybrid_coordinator = true;` | `c.normalize()` | `c.elastic_join == true`. |
| UT-0415-04 | `grid_config_derived_elastic_join_when_elastic_departure` | `let mut c = GridConfig::default(); c.elastic_departure = true;` | `c.normalize()` | `c.elastic_join == true` (also flips retain_partitions per UT-0415-02). |
| UT-0415-05 | `validate_rejects_retain_false_with_departure_true` | a config in which the user explicitly forces `retain_partitions=false; elastic_departure=true` (skipping normalize) | `c.validate()` | Returns `Err(ConfigError::RetainRequiredForDeparture)`. |
| UT-0415-06 | `validate_rejects_inverted_join_window_bounds` | `c.join_window_min = Duration::from_millis(500); c.join_window_max = Duration::from_millis(50);` | `c.validate()` | Returns `Err(ConfigError::JoinWindowOrdering)`. |
| UT-0415-07 | `validate_rejects_zero_solo_budget` | `c.solo_budget = 0` | `c.validate()` | Returns `Err(ConfigError::SoloBudgetZero)`. |
| UT-0415-08 | `validate_accepts_default` | `GridConfig::default()` after normalize | `c.validate()` | Returns `Ok(())`. |
| UT-0415-09 | `active_mode_returns_v1_lenient_for_default` | `GridConfig::default()` (delta_mode=false, strict_bsp=false) | `c.active_mode()` | Returns `ExecutionMode::V1Lenient` (mode A). |
| UT-0415-10 | `active_mode_full_matrix` | parameterised over the 4 combinations of `(delta_mode, strict_bsp)` | `c.active_mode()` | Returns `{V1Lenient, V1Strict, DeltaLenient, DeltaStrict}` respectively (M0). |
| UT-0415-11 | `grid_config_serde_roundtrip_preserves_new_fields` | `let c = GridConfig::default()` with one elastic field flipped | `let bytes = bincode::serialize(&c)?; let c2: GridConfig = bincode::deserialize(&bytes)?;` | `c2 == c`; all 9 new fields survive round-trip. |
| UT-0415-12 | `grid_config_serde_default_back_compat` | a serialized older `GridConfig` blob (or one we stub by serializing `OldGridConfig` then deserializing as `GridConfig` if `#[serde(default)]` is in place) | deserialize | Old blob deserializes; new fields take their defaults. (If unfeasible due to serde struct shape, document as covered by `#[serde(default)]` + a unit assertion.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `join_window_min == join_window_max` | `validate` returns `Ok` (equal bounds are legal — degenerate but not invalid). |
| EC-2 | `initial_wait_timeout == 0s` | Currently no validate rule; document as "permitted but degenerate" (R6 minimum behaviour: timeout fires immediately). Either add a guard or accept; developer choice — assert chosen behaviour. |
| EC-3 | `normalize` called twice | Idempotent: second call leaves config unchanged. |

## Invariants asserted

None directly (configuration surface).

## ARG/DISC/REF citation

None.

## Determinism notes

Pure synchronous; deterministic.

## Cross-test dependencies

- UT-0415-01 anchors EG-I5 (v1-compatibility) — every default value asserted here is the per-field guarantee that flips to v1 when the user passes no flags.
- TASK-0455 uses these defaults as its regression baseline; any change to a default value MUST be reflected here AND in TEST-SPEC-EG-I5.
