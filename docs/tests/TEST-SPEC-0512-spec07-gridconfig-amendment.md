# TEST-SPEC-0512: SPEC-07 GridConfig amendment (chunk_size, streaming_strategy, dispatch_mode fields)

**SPEC-21 §7 ID:** plumbing only (transitively gates T6, T8 via short-circuit).
**Owning task:** TASK-0512.
**Parent spec:** SPEC-21 §3.4 R24, R25; §3.6 R34; §3.8 A3; §6.2 short-circuit policy (R26).
**Type:** unit (struct construction + serde + default verification).
**Theory anchor:** None direct (configuration surface).

---

## Inputs / Fixtures

- A freshly constructed `GridConfig::default()`.
- Constructor call with explicit fields: `chunk_size: 10_000`, `streaming_strategy: StreamingStrategyConfig::RoundRobin`, `dispatch_mode: DispatchMode::Auto`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0512-01 | `default_chunk_size_is_10000` | `GridConfig::default()` | read `.chunk_size` | `== 10_000` (R24 placeholder; doc-comment MUST tag it as benchmark-TBD). |
| UT-0512-02 | `default_chunk_size_doc_tags_benchmark_tbd` | grep the `chunk_size` field's Rustdoc | check for "benchmark calibration" or "Q2" or "TBD before v2 release" | substring present (R24 closing-clause discipline). |
| UT-0512-03 | `default_streaming_strategy_is_round_robin` | default config | read `.streaming_strategy` | `== StreamingStrategyConfig::RoundRobin` (R25 default). |
| UT-0512-04 | `default_dispatch_mode_is_auto` | default config | read `.dispatch_mode` | `== DispatchMode::Auto` (R34 default). |
| UT-0512-05 | `chunk_size_u32_max_short_circuits_to_split` | construct `GridConfig { chunk_size: u32::MAX, .. default() }` | invoke `should_use_streaming(&config)` (the dispatch helper) | returns `false` (R26 short-circuit; the streaming pipeline is NOT activated). |
| UT-0512-06 | `chunk_size_below_max_activates_streaming` | construct `GridConfig { chunk_size: 1000, .. default() }` | `should_use_streaming(&config)` | returns `true`. |
| UT-0512-07 | `serde_round_trip_preserves_three_fields` | the explicit-field constructor | serde to bincode then back | the decoded config has the three fields equal to the original (`PartialEq`). |
| UT-0512-08 | `streaming_strategy_fennel_constructible` | `StreamingStrategyConfig::Fennel { alpha: 1.0 }` | serde round-trip | OK; the `alpha` field round-trips bit-identically. |
| UT-0512-09 | `dispatch_mode_pull_constructible` | `DispatchMode::Pull` | serde round-trip | OK; equals original. |
| UT-0512-10 | `dispatch_mode_push_constructible` | `DispatchMode::Push` | serde round-trip | OK; equals original. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `chunk_size = 0` | configuration validation fails at `GridConfig::validate()` (treat as caller error: chunk size 0 is meaningless). |
| EC-2 | `chunk_size = 1` | accepted; tests in T8 chunk-size-independence will exercise this minimal value. |
| EC-3 | Conflicting `dispatch_mode = Pull` with `chunk_size = u32::MAX` | validation MUST emit a warning (logged via `tracing`) noting the short-circuit overrides the pull mode. Behavior MUST default to `split()` non-streaming path (short-circuit wins). |
| EC-4 | Future field added between Rustdoc lines | UT-0512-02 grep gate MUST anchor on the `chunk_size` field name to avoid false positives. |

## Invariants asserted

- R24 (chunk_size configurability + benchmark-TBD discipline).
- R25 (streaming_strategy selectable; default RoundRobin).
- R34 (dispatch_mode three variants; default Auto).
- R26 (short-circuit at `chunk_size = u32::MAX`).

## ARG/DISC/REF citation

- None direct (configuration surface).

## Determinism notes

Pure synchronous, no tokio, no RNG. Serde tests use bincode 1.x (consistent with v1 wire format). Deterministic by construction.

## Cross-test dependencies

- TEST-SPEC-0567 (R26 short-circuit) — forward-referenced from TASK-0512 / TASK-0517 but NOT in scope for the current Stage 2 wave (TASK-0567 not yet authored). Flagged as a Stage-2 wave-2 dependency.
- TEST-SPEC-0565 / TEST-SPEC-0568 — same status; out of scope for Stage 2 wave 1.
- TEST-SPEC-EG-U15a (SPEC-20 GridConfig elastic fields) — ensure no field-name collision when SPEC-20 + SPEC-21 GridConfig amendments coexist.
