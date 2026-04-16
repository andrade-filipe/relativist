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

## Next Spec

**SPEC-27** (Encoder/Decoder Trait API and Problem Registry) — Layers 1-3 of DISC-012.
- Phase 1: Traits (~100 LoC) — `Encoder`, `Decoder`, `Codec` in `relativist_core::encoding::traits`
- Phase 2: Church refactoring (~100 LoC)
- Phase 3: LambdaCodec (~250 LoC) — REF-005 Mackie/Pinto
- Phase 4: Registry (~200 LoC)
- Phase 5: CLI integration (~100 LoC)
- Phase 6: RecipeEncoder generalization (~150 LoC)

Total: ~900 LoC across ~12-15 atomic tasks.
