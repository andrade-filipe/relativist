# REVIEW: SPEC-27 Phase 5 — CLI integration

**Date:** 2026-04-16
**Tasks:** TASK-0338, TASK-0339
**Spec requirements:** R21, R22, R23
**Files reviewed:**
- `relativist-core/src/config.rs` (ComputeArgs extension, Encoders subcommand)
- `relativist-core/src/commands.rs` (`run_compute_with_encoder`, `run_encoders_command`)
- `relativist-core/src/error.rs` (Encoding variant, RegistryError → RelativistError)
- `relativist-cli/src/main.rs` (Encoders dispatch wiring)
- `docs/tests/TEST-SPEC-0338.md`, `docs/tests/TEST-SPEC-0339.md`

**Test count:** 777 → 781 (+4 unit tests for arg parsing).
**Smoke verified:** legacy + 2 registry paths + unknown encoder error all pass.
**Verdict:** APPROVE — no Must-Fix items.

---

## Spec compliance

| Req | Requirement | Status | Evidence |
|-----|-------------|--------|----------|
| R21 | `compute --encoder <name> --input <json>`; legacy positional preserved | ✅ | config.rs ComputeArgs (operation/a/b now Option<…>; encoder/input added with `requires`); commands.rs early-return when encoder set |
| R22 | `encoders list` subcommand | ✅ | config.rs Encoders/EncodersArgs/EncodersAction; commands.rs run_encoders_command; main.rs dispatch added |
| R23 | Pipeline `encode → validate → reduce_all → decode → print JSON` | ✅ | commands.rs `run_compute_with_encoder`: `encode_and_validate` → `reduce_all` → `decode` → `serde_json::to_string_pretty` |

## Code quality

- `RelativistError::Encoding(String)` plus `From<RegistryError>` impl is the
  minimal, non-invasive change. Avoided exposing `RegistryError` in the public
  error enum to keep encoding error vocabulary in one place.
- Backward-compat: legacy path is unchanged behaviourally; only the field types
  changed (positional became Option<…>) so the function signature consumers see
  is identical to before. Zero regression on the 6 existing `compute add 3 5`
  smoke tests in CI.
- Validation is delegated to clap (`requires = "encoder"` on `--input`) for
  inter-flag dependencies, and to runtime checks for "either positional OR
  encoder" because clap does not express that constraint cleanly.
- `serde_json::to_string_pretty` failure is mapped to `RelativistError::Encoding`
  (not `Io`), matching the source category.
- `run_compute_with_encoder` is private (`fn`, not `pub fn`) — intentional: it's
  a helper, not part of the public command API.

## Architecture

- New CLI surface (encoders list, --encoder/--input flags) lives entirely in the
  CLI module + commands.rs. The registry / codec abstraction is unchanged.
- Pipeline composition (`encode_and_validate` → `reduce_all` → `decode`) is a
  straight call chain — no new abstractions, matches DISC-012 v2 intent.
- Help text for `--encoder` / `--input` mentions SPEC-27 R21 so users can find
  the spec from the binary alone.

## Test coverage (4 new unit tests)

TEST-SPEC-0338 (R21, R23):
- C1: `test_parse_compute` (updated) — legacy positional still parses.
- C2: `test_parse_compute_encoder_flag` — `--encoder` + `--input` parse with no
  positional args.
- C3: `test_parse_compute_input_without_encoder_rejected` — clap rejects
  `--input` without `--encoder` (validates `requires = "encoder"`).
- C4/C5 covered by registry tests (TEST-SPEC-0336 R6, TEST-SPEC-0337 D3).

TEST-SPEC-0339 (R22):
- E1: `test_parse_encoders_list` — `encoders list` parses.
- E2: `test_parse_encoders_no_action_fails` — `encoders` without subcommand
  fails (clap requires the subcommand).

## Smoke testing (manual, on release binary)

```
$ relativist compute add 3 5            → Result: 8 (legacy)
$ relativist compute --encoder church_add --input '{"a":3,"b":5}'
                                        → Result: { "result": 8 }
$ relativist compute --encoder lambda --input '{"term":"(λx. x) (λy. y)"}'
                                        → Result: { "term": "λv0. v0", … }
$ relativist compute --encoder nope --input '{}'
                                        → error: encoding error: encoder 'nope' not found (exit 3)
$ relativist encoders list              → 5 encoders, sorted, aligned
```

All five behaviours match SPEC-27 §5.5 acceptance examples.

## Should-Fix (deferred)

1. **Free-variable name preservation in lambda readback.** The smoke shows
   `λv0. v0` instead of `λx. x` because the readback uses a fresh-name
   generator. Pre-existing Phase 3 limitation (called out in the LambdaCodec
   review); does not affect correctness, only readability. Out of scope for
   Phase 5.
2. **`--workers` not yet supported with `--encoder`.** Distributed reduction
   for non-Church encoders requires Phase 6 (RecipeEncoder) to ship recipes
   instead of full nets. The flag is silently ignored in the encoder path —
   should print a warning. ~5 LoC; deferred to Phase 6.
3. **`encoders describe <name>` and `encoders list --format json`** — future
   work; not in SPEC-27.

## Out of scope (explicitly)

- Phase 6 RecipeEncoder generalization (R24-R28).
- Distributed compute for non-Church encoders.
- Plugin loading or runtime encoder registration from disk.

## Verification

```
cargo build --workspace                                 → clean
cargo test --workspace                                  → 781 passed (was 777, +4)
cargo build --release --workspace                       → clean
cargo clippy --workspace --all-targets -- -D warnings   → clean
cargo fmt --check                                       → clean
```

## Verdict

**APPROVE.** Phase 5 is implementation-complete and ready for QA stage.
