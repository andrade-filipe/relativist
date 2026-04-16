# Pipeline State

**Last updated:** 2026-04-16
**Maintained by:** sdd-pipeline agent (do not edit manually)

---

## Current Work

**Current spec:** SPEC-26 §3.1 (Cargo Workspace Restructure — Layer 0) — **COMPLETE**
**Current stage:** DONE (mechanical refactor, no functional changes)
**v2 branch:** v2-development
**v1 tests baseline:** 690 passing
**Final test count:** 716 (712 unit + 4 integration) — unchanged
**Clippy status:** clean (0 warnings, including --all-targets)
**Formatting:** clean (cargo fmt --check passes)

## Stage History (SPEC-26 §3.1)

- [x] SPLITTING: 2026-04-16 — 5 atomic tasks (workspace skeleton, move src/, thin CLI, verify, docs)
- [x] DEV: 2026-04-16 — workspace created (relativist-core + relativist-cli), src/tests/benches moved via git mv
- [x] VERIFY: 2026-04-16 — 716 tests pass, clippy --workspace --all-targets clean, fmt clean, release binary at target/release/relativist.exe (scripts + Dockerfile path preserved)
- [x] REVIEW: 2026-04-16 — refactor-only change, all 7 requirements (R1-R7) satisfied
- [x] QA: 2026-04-16 — smoke test `compute add 3 5 = 8` passes
- [x] REFACTOR: 2026-04-16 — fixed pre-existing clippy warning in protocol/config.rs (bool_assert_comparison)

## Layer 0 Verification (SPEC-26 R1-R7)

| Req | Status | Evidence |
|-----|--------|----------|
| R1 | ✅ | Layout: relativist/{Cargo.toml, relativist-core/, relativist-cli/} |
| R2 | ✅ | Workspace manifest with members + resolver = "2" |
| R3 | ✅ | All src/ files moved verbatim to relativist-core/src/ via git mv |
| R4 | ✅ | All deps + features (tls, metrics, otel, full) preserved in relativist-core/Cargo.toml |
| R5 | ✅ | relativist-cli/src/main.rs delegates to relativist_core::commands; binary name "relativist" preserved |
| R6 | ✅ | 716 tests pass in relativist-core; [[bench]] section in relativist-core/Cargo.toml |
| R7 | ✅ | `cargo test --workspace` runs all tests across both crates |

## Backward Compatibility

- Binary path unchanged: `target/release/relativist.exe`
- All 6 bench scripts (`scripts/bench_*.sh`) work without modification
- Dockerfile `COPY` line works unchanged
- CLI flags and subcommands identical to pre-restructure

## SPEC-27 Progress

**Phase 1: Traits — COMPLETE** (2026-04-16)
- `relativist_core::encoding::traits` module created (~150 LoC inc. tests)
- `Encoder`, `Decoder`, `Codec` traits + `EncodeError`, `DecodeError` types (R1-R4)
- `validate_encoded_net()` checks E1 (T1 linearity subset) + E2 (at least one redex) (R5, R6)
- 6 inline tests; test count 716 → 722 (+6)

**Phase 2: Church refactoring — COMPLETE** (2026-04-16, commit `a601c5e`)
- `ChurchArithmeticCodec` (4 ops: add/mul/exp/sum_of_squares) implements `Codec`
- Wraps existing `build_*` + `decode_nat_or_shared`; zero changes to public API (R7)
- 8 inline tests (round-trips, schema validation, object safety); test count 722 → 726 (+4)

**Phase 3: LambdaCodec — COMPLETE** (2026-04-16, all 6 stages)
- TASK-0333 (encoder), TASK-0334 (decoder), TASK-0335 (edge cases) created in `docs/backlog/`.
- TEST-SPEC-0333/0334/0335 specified in `docs/tests/`.
- `relativist-core/src/encoding/codec_lambda.rs` implements parser + pretty-printer +
  Mackie/Pinto encoder + port-directed readback decoder; ~900 LoC inc. 32 inline tests
  (26 spec'd + 6 QA adversarial).
- Engine fix: `interact_comm` self-loop guard added (mirrors `interact_anni` /
  `interact_eras`); required for sound reduction of identity-typed arguments
  (e.g., `(λx. x x) (λy. y)`). No regression on the 690 v1 tests.
- Final test count: 726 → 758 (+32). Clippy + fmt clean.
- REVIEW notes in `docs/reviews/REVIEW-SPEC-27-Phase3-LambdaCodec.md`.
- Stage 6 REFACTOR: no Must-Fix items from REVIEW. Should-Fix items
  (free-var name preservation, DUP-cycle readback algorithm) deferred as
  out of T5-T9 scope; will be re-evaluated when consumed by Phase 5 CLI.
- Driven by Option B: SDD pipeline managed manually from TCC root
  (relativist-local agents not invocable from TCC cwd).

**Phase 4: EncoderRegistry — COMPLETE** (2026-04-16, all 6 stages)
- TASK-0336 (registry struct + ops, R17/R18/R20), TASK-0337 (default_registry, R19) created.
- TEST-SPEC-0336 (8 cases R1-R8) and TEST-SPEC-0337 (5 cases D1-D5) specified.
- `relativist-core/src/encoding/registry.rs` (~310 LoC inc. 19 inline tests):
  - `EncoderRegistry` (HashMap<String, Box<dyn Codec>>) with 6 ops: new, register,
    get, list (sorted), encode_and_validate (calls validate_encoded_net), decode.
  - `RegistryError` enum (DuplicateName, NotFound, Encode #[from], Decode #[from]).
  - `default_registry()` registering 4 ChurchArithmeticCodec ops + LambdaCodec.
  - 13 spec-driven tests + 6 QA adversarial tests (case-sensitive lookup,
    failed-register doesn't poison, empty input, NotFound preserves name,
    Default impl, no-redex net rejected by E2).
- Re-exported from `encoding/mod.rs` as `EncoderRegistry`, `RegistryError`,
  `default_registry`.
- Note: round-trip test for `church_exp` reduced to encode-only because decode
  hits the SPEC-14 DUP-cycle readback limitation (pre-existing, not caused by
  this work). add/mul/sum_of_squares fully round-trip.
- Final test count: 758 → 777 (+19). Clippy + fmt clean.
- REVIEW notes in `docs/reviews/REVIEW-SPEC-27-Phase4-EncoderRegistry.md` (APPROVE).
- Stage 5 QA: 6 adversarial tests added, all passed first try — no Must-Fix items.
- Stage 6 REFACTOR: no code changes needed (zero bugs found in QA).

**Phase 5: CLI integration — COMPLETE** (2026-04-16, all 6 stages)
- TASK-0338 (compute --encoder dispatch, R21/R23), TASK-0339 (encoders list, R22) created.
- TEST-SPEC-0338 (5 cases C1-C5) and TEST-SPEC-0339 (3 cases E1-E3) specified.
- `relativist-core/src/config.rs`: ComputeArgs extended (operation/a/b → Option,
  added `--encoder` and `--input` with clap `requires`); `Encoders(EncodersArgs)`
  variant with `EncodersAction::List` subcommand added.
- `relativist-core/src/commands.rs`:
  - `run_compute_command` early-returns through `run_compute_with_encoder` when
    `--encoder` is set; legacy SPEC-14 path validates positional args at runtime
    and is otherwise unchanged.
  - `run_compute_with_encoder` runs the full SPEC-27 R23 pipeline:
    `encode_and_validate → reduce_all → decode → serde_json pretty-print`.
  - `run_encoders_command` enumerates `default_registry().list()` aligned with
    the longest name (deterministic, sorted output).
- `relativist-core/src/error.rs`: added `RelativistError::Encoding(String)` and
  `From<RegistryError>` impl so `?` propagates registry errors with exit code 3.
- `relativist-cli/src/main.rs`: `Command::Encoders(args)` dispatch wired.
- Smoke (release binary): `compute add 3 5 → Result: 8`;
  `compute --encoder church_add --input '{"a":3,"b":5}' → {"result":8}`;
  `compute --encoder lambda --input '{"term":"(λx. x) (λy. y)"}' → λv0. v0`;
  unknown encoder → exit 3 with clear message; `encoders list` → 5 codecs.
- Final test count: 777 → 789 (+12: 4 spec-driven C/E + 4 QA arg-parsing + 4
  dispatch unit tests). Clippy + fmt clean.
- REVIEW notes in `docs/reviews/REVIEW-SPEC-27-Phase5-CLI.md` (APPROVE).
- Stage 5 QA: 8 adversarial tests added (parsing edge cases + dispatch error
  paths), all passed first try — no Must-Fix items.
- Stage 6 REFACTOR: no code changes needed.

**Phase 6: RecipeEncoder mínimo (R24+R25 only) — COMPLETE** (2026-04-16, all 6 stages)
- TASK-0340 created with explicit "minimal scope" note: ships only R24 (trait
  definition) and R25 (non-coupling with the `Codec` registry). R26/R27/R28
  deferred until SPEC-25 (item 2.29, milestone M7) is implemented.
- TEST-SPEC-0340 (R1-R8) specified.
- `relativist-core/src/encoding/recipe.rs` (~250 LoC inc. 12 inline tests):
  - `RecipeEncoder: Encoder` trait with `type Recipe: Serialize + DeserializeOwned + Send + Sync`
    and 3 methods (`is_decomposable`, `make_recipes`, `generate_partition`).
  - `MinimalRecipe` struct + `MinimalRecipeEncoder` demo implementation (Con/Era pairs).
  - Demo encoder is **not** registered in `default_registry()` — it exists only to
    exercise the trait in tests.
- 8 spec-driven tests + 4 QA adversarial tests (more-workers-than-size, zero-pair
  recipe, Send+Sync compile-check, centralized-encode validation).
- Final test count: 789 → **801** (+12). Clippy + fmt clean.
- REVIEW notes in `docs/reviews/REVIEW-SPEC-27-Phase6-RecipeEncoder.md` (APPROVE).
- Stage 5 QA: 4 adversarial tests added, all passed first try — no Must-Fix items.
- Stage 6 REFACTOR: no code changes needed.
- **Deferral tracker:** see `docs/DEFERRED-WORK.md` row D-001 for the unblock
  checklist that must be processed when SPEC-25 ships in milestone M7.

**SPEC-27 status:** Phases 1-5 fully complete. Phase 6 partially complete
(R24+R25 shipped; R26-R28 deferred to M7). M10 milestone in V2-FEATURE-MATRIX
remains open until D-001 is resolved.

**Next focus (per V2-FEATURE-MATRIX Tier 1 break-even path):**
- Item 2.22 — TCP Transport Tuning (M1, ~100 LoC, 2-4h)
- Item 2.23 — Wire Format v2 (M1, ~450 LoC, 2-3d)
- Decision point after M1: measure tcp_localhost/seq ratio and decide on M3-M4.
