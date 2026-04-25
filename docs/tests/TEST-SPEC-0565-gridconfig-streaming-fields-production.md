# TEST-SPEC-0565: GridConfig streaming fields production (chunk_size, streaming_strategy, dispatch_mode, optional max_pending_lifetime)

**SPEC-21 §7 ID:** plumbing only (production-side closure of TEST-SPEC-0512 amendment-level coverage; transitively gates T6/T8 via R26 short-circuit and T11/T12/T13/T14 via DispatchMode).
**Owning task:** TASK-0565.
**Parent spec:** SPEC-21 §3.3 R24, §3.4 R25, §3.6 R34, §3.7 R37g (optional `max_pending_lifetime`); §3.8 A3 (consumer of TASK-0512). Closes SC-001 part 2.
**Type:** unit (struct field plumbing + serde defaults + builder factory).
**Theory anchor:** None direct (configuration surface).

---

## Inputs / Fixtures

- A freshly constructed `GridConfig::default()` (post-TASK-0565 landing).
- An old-format JSON config file body: `{"num_workers":4}` (missing the four new SPEC-21 fields).
- Explicit-construct `GridConfig { chunk_size: 1024, streaming_strategy: StreamingStrategyConfig::Fennel { alpha: 1.0 }, dispatch_mode: DispatchMode::Pull, max_pending_lifetime: 16, ..default() }`.
- A bincode encoder/decoder pinned to the live SPEC-18 wire version.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0565-01 | `gridconfig_has_chunk_size_field` | `GridConfig::default()` | read `.chunk_size` | `== 10_000` (R24 placeholder; doc-comment cites Q2 / SC-024). |
| UT-0565-02 | `gridconfig_has_streaming_strategy_field` | default config | read `.streaming_strategy` | `== StreamingStrategyConfig::RoundRobin` (R25 default). |
| UT-0565-03 | `gridconfig_has_dispatch_mode_field` | default config | read `.dispatch_mode` | `== DispatchMode::Auto` (R34 default). |
| UT-0565-04 | `gridconfig_optional_max_pending_lifetime_default` (CONDITIONAL on TASK-0565 shipping the optional fourth field) | default config | read `.max_pending_lifetime` | `== 16` (R37g default). If TASK-0565 ships only three fields, this test is skipped and the test-list note flags TASK-0568 fourth-flag drop. |
| UT-0565-05 | `serde_default_loads_old_config_unchanged` | the old-format JSON body `{"num_workers":4}` | `serde_json::from_str::<GridConfig>(body)` | `Ok(cfg)`; `cfg.chunk_size == 10_000`; `cfg.streaming_strategy == RoundRobin`; `cfg.dispatch_mode == Auto`; (and `cfg.max_pending_lifetime == 16` if shipped). Pre-SPEC-21 config files load identically — additive-compat regression. |
| UT-0565-06 | `serde_round_trip_preserves_all_new_fields` | the explicit-field config | bincode serialize then deserialize | decoded equals original (`PartialEq`); all four fields bit-identical. |
| UT-0565-07 | `streaming_strategy_build_round_robin` | `StreamingStrategyConfig::RoundRobin` and `num_workers = 4` | call `.build(num_workers)` | returns `Box<dyn StreamingPartitionStrategy>` whose `.name()` (or `Debug`) identifies a `RoundRobinStreamingStrategy` instance bound to `num_workers = 4`. |
| UT-0565-08 | `streaming_strategy_build_fennel` | `StreamingStrategyConfig::Fennel { alpha: 1.5 }` and `num_workers = 4` | `.build(num_workers)` | returns a `Box<dyn StreamingPartitionStrategy>` identifying a `FennelStreamingStrategy { alpha: 1.5 }` bound to `num_workers = 4`. |
| UT-0565-09 | `streaming_strategy_default_round_robin` | `StreamingStrategyConfig::default()` | match the variant | `StreamingStrategyConfig::RoundRobin`. |
| UT-0565-10 | `dispatch_mode_default_auto` | `DispatchMode::default()` | match | `DispatchMode::Auto`. |
| UT-0565-11 | `enums_derive_required_traits` | `StreamingStrategyConfig` and `DispatchMode` | check derives | both derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize` (compile-time; ensured via blanket trait-bound test pattern). |
| UT-0565-12 | `field_order_appended_after_spec20_elastic_fields` | the live `GridConfig` struct definition | grep ordering | the four new SPEC-21 fields appear AFTER the SPEC-20 elastic fields per TASK-0512 line-60 coordination note. (Bincode is order-sensitive; this gate prevents accidental reorderings that would break SPEC-20 ↔ SPEC-21 coexistence.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `chunk_size = 0` | If `GridConfig::validate()` is invoked, returns an error (chunk size 0 is meaningless per TEST-SPEC-0512 EC-1). |
| EC-2 | `chunk_size = u32::MAX` with `dispatch_mode = Pull` | accepted by serde; runtime helper `should_use_streaming(&cfg)` returns `false` (R26 short-circuit wins per TEST-SPEC-0512 EC-3). Joint coverage with TEST-SPEC-0567. |
| EC-3 | `Fennel { alpha: f32::NAN }` | accepted at the type level (no validation in this task); validation is downstream concern (per SPEC-21 R5 default-only path). |
| EC-4 | Future fifth field added | UT-0565-12 ordering gate MUST anchor on the SPEC-20 last-field name to avoid false positives; document the anchor explicitly. |

## Invariants asserted

- R24 (chunk_size production wiring + Q2/SC-024 doc-comment).
- R25 (streaming_strategy production wiring + factory helper).
- R34 (dispatch_mode production wiring).
- R37g (max_pending_lifetime production wiring; conditional).
- §3.8 A3 (SPEC-07 amendment).
- Additive-compat: old config files load with defaults applied (UT-0565-05).

## ARG/DISC/REF citation

- None direct (configuration surface; no theory anchor required).

## Determinism notes

Pure synchronous, no tokio, no RNG. Bincode 1.x serde tests are deterministic by construction. `streaming_strategy.build(num_workers)` returns a deterministically-typed `Box<dyn StreamingPartitionStrategy>`; the `RoundRobinStreamingStrategy` interior counter starts at zero per TEST-SPEC-0530.

## Cross-test dependencies

- TEST-SPEC-0512 (SPEC-07 amendment-level coverage) — predecessor; this task is the production-side closure.
- TEST-SPEC-0524 (`StreamingPartitionStrategy` trait) — provides the trait used by the factory helper.
- TEST-SPEC-0530 (RoundRobinStreamingStrategy) — provides the default strategy concrete type.
- TEST-SPEC-0531 (FennelStreamingStrategy) — provides the Fennel variant.
- TEST-SPEC-0567 (R26 short-circuit) — joint coverage on EC-2 and the `chunk_size = u32::MAX` short-circuit path.
- TEST-SPEC-0568 (CLI flags) — consumer; CLI parses onto these fields.
- TEST-SPEC-0577 / TEST-SPEC-0578 (FSM gating) — consumer; FSMs branch on `dispatch_mode`.
- TEST-SPEC-EG-U15a (SPEC-20 GridConfig elastic fields) — coexistence gate (UT-0565-12 field ordering).
