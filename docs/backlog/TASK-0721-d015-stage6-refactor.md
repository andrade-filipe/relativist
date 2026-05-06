# TASK-0721 — D-015 Stage 6 REFACTOR scope

**Phase:** D-015 (SPEC-27 v3 + HornerCodec) — Stage 6 REFACTOR
**Bundle:** D-015 — SPEC-27 v3 + HornerCodec (Topic 2)
**Status:** TODO
**Priority:** P0 (BLOCKS bundle merge; CLI demo path is broken without these fixes)
**Spec:** SPEC-27 v3 (`specs/SPEC-27-encoder-decoder-api.md`)
**Depends on:** TASK-0709..0719 (must all be in HEAD)
**Estimated complexity:** M (~120 LoC production + ~50 LoC test, well under SDD <200 LoC budget)

---

## Context

Stage 4 (REVIEW) returned AMARELO with 1 MF + 5 SF + 3 NTH.
Stage 5 (QA) escalated to **NOT SAFE-TO-MERGE** with 2 CRITICAL + 4 HIGH bugs.

Bug reports:
- `docs/reviews/D-015-spec27-horner-review.md`
- `docs/qa/D-015-spec27-horner-qa.md`

Without these fixes, the SPEC-27 v3 R21/R22/R23 narrative ("CLI-invocable
demo of distributed polynomial evaluation") is broken — Horner inputs
with `coeffs.len() > 1` return `DecodeFailed("no root")` because
`run_compute_with_encoder` doesn't call `discover_root`. Constant
polynomials are rejected because the registry validator unconditionally
enforces E2 (≥1 redex), and a Normal-Form net has zero redexes.

The bundle's unit tests pass, but they bypass the very pipeline the
TCC narrative depends on. This is a hard prerequisite for merge.

## Scope — fixes from QA + Reviewer

### CRITICAL (2 bugs from QA, both MUST fix)

**BUG-001 — `run_compute_with_encoder` missing `discover_root` after `reduce_all`**

- **Location:** `relativist-core/src/commands.rs:697-731` (`run_compute_with_encoder`)
- **Symptom:** every non-constant-polynomial Horner CLI invocation returns `DecodeFailed("no root")`. Tests pass only because they bypass the registry CLI path.
- **Root cause:** `HornerCodec::encode` builds the net with `net.root = None`. The `reduce_all` step does not set the root; it must be discovered post-reduction.
- **Fix:** In `run_compute_with_encoder`, after `reduce_all(&mut net)`, call `net.discover_root()` (or equivalent helper if naming differs in HEAD). Add log line via `tracing::debug!` confirming the discovered root agent ID.
- **Test:** Add CLI integration test exercising the registry path (currently bypassed by all unit tests). See "New tests" below.

**BUG-002 — Registry validator unconditionally rejects constant-polynomial Horner input**

- **Location:** `relativist-core/src/encoding/traits.rs::validate_encoded_net` (R5 E2 enforcement)
- **Symptom:** `encode_and_validate("horner", {"coeffs":[42],"x":N})` always returns `InvalidNet("E2: net has no redexes")` because the constant-polynomial fast path returns a Normal-Form net.
- **Root cause:** `validate_encoded_net` enforces E2 (≥1 redex) unconditionally. The Codec's encoder produces a valid Normal-Form net for constant inputs (correct behavior), but the registry rejects it.
- **Fix (decide ONE path):**
  - **Path A (preferred):** Add a Codec-level method `accepts_normal_form_input(&self, input: &[u8]) -> bool` that returns true for HornerCodec when `coeffs.len() == 1`. The registry consults this before enforcing E2.
  - **Path B:** Drop E2 enforcement at the registry level entirely; rely on the reducer to no-op on a NF input (which it already does correctly).
- **Recommendation:** **Path A** — keeps E2 as a default safety net for codecs that DO require redexes, while allowing exceptions where they are semantically correct.
- **Test:** Roundtrip `(coeffs=[42], x=99) → 42` via the registry path.

### HIGH (1 bug from QA, MUST fix; 3 deferred to NTH if time)

**BUG-003 — `ChurchArithmeticCodec::encode` does NOT validate `MAX_CHURCH_NAT`**

- **Location:** `relativist-core/src/encoding/codec_church.rs::encode`
- **Symptom:** Crafted JSON with `a > 1_000_000` causes process-aborting panic via `assert!` in `encode_church_into`. DoS surface.
- **Root cause:** `ChurchArithmeticCodec::encode` does not pre-validate inputs against `MAX_CHURCH_NAT`; HornerCodec does. Asymmetric defense.
- **Fix:** Add input validation in `ChurchArithmeticCodec::encode` mirroring `HornerCodec::encode`'s pre-check. Return `EncodeError::InvalidInput` with descriptive message. Use the same `MAX_CHURCH_NAT` constant.
- **Test:** Existing TASK-0710 audit tests are unit-level and don't trip the panic; add UT in `codec_church.rs` testing the bounds rejection.

### MUST-FIX from Reviewer (1)

**MF-001 — `is_recipe_encoder` test is misleading string probe**

- **Location:** `relativist-core/src/encoding/recipe.rs:380-386`
- **Symptom:** Test claims to verify a `RecipeEncoder` trait check but actually does `type_name().contains("HornerCodec")` string probe. Would NOT fail if someone added `impl RecipeEncoder for HornerCodec`.
- **Fix (decide ONE):**
  - **Option A:** Use `static_assertions::assert_not_impl_all!(HornerCodec: RecipeEncoder)` for compile-time bound. Adds dev-dep `static_assertions = "1"`.
  - **Option B:** Honest rename to `default_registry_horner_does_not_implement_recipeencoder_by_string_probe` and document the limitation. ~5 LoC.
- **Recommendation:** **Option A** if dev-dep is acceptable; otherwise Option B with the rename.

### SHOULD-FIX from Reviewer (5; address as time permits in this REFACTOR)

**SF-001 — TODO from reviewer report (read source for exact item; ~10 LoC)**

**SF-002 — `biguint_readback` rustdoc claims topology-mirror that is FALSE**
- **Location:** `relativist-core/src/encoding/biguint_readback.rs` module rustdoc
- **Fix:** Reword to "MIRROR-MOSTLY topology of decode_nat with TASK-0714 recursive helpers (count_chain_through_dups, chain_from_dup_branch) for shared-chain extension." Cite Mackie/Pinto §5 inline link to "Future Work" SPEC-27 §5.1.

**SF-003 — TODO from reviewer report (read source for exact item; ~5 LoC)**

**SF-004 — PT-0715-06 silently treats `Err` as a pass**
- **Location:** `relativist-core/src/encoding/horner.rs::tests::pt_0715_06_horner_distributed_g1_property` (or similar)
- **Symptom:** `if let Ok(out) = codec.decode(&net) { prop_assert_eq!(...) }` no-ops on readback failures. A regression in the readable subset would not trigger property failure.
- **Fix:** Track skip count; `prop_assert!(skips < cases / 2, "too many readback failures, possible regression in readable subset")`. ~5 LoC.

**SF-005 — TODO from reviewer report (read source for exact item; ~5 LoC)**

### G1 strictness hardening (from QA BUG-004 amplification)

**Stronger structural witness for `horner_distributed_g1_in_process_structural_isomorphism`:**

- **Current:** `format!("{:?}", left) == format!("{:?}", right)` on `Result<Value, DecodeError>`. Works but is fragile against agent-ID divergence in DUP-branch error payloads (BUG-004).
- **Improvement (~40 LoC):** Replace debug-string equality with explicit:
  - Both legs return `Result::Ok` AND values match — pass
  - Both legs return `Result::Err` with same variant AND same error message structure — pass
  - Otherwise — fail
  - Detect agent-ID divergence in error contexts as expected (allowed)
- This converts a fragile witness to a robust one without requiring a full Mackie/Pinto readback.

## Files in scope

| File | Change |
|------|--------|
| `relativist-core/src/commands.rs:697-731` | Call `discover_root` after `reduce_all` (BUG-001) |
| `relativist-core/src/encoding/traits.rs::validate_encoded_net` | Codec-level NF bypass for E2 (BUG-002 Path A) OR drop E2 (Path B) |
| `relativist-core/src/encoding/traits.rs` | Add `Codec::accepts_normal_form_input()` if Path A chosen (BUG-002) |
| `relativist-core/src/encoding/horner.rs::Codec::accepts_normal_form_input` | impl Path A (BUG-002) |
| `relativist-core/src/encoding/codec_church.rs::Codec::accepts_normal_form_input` | impl Path A (returns false) (BUG-002) |
| `relativist-core/src/encoding/codec_church.rs::encode` | Add MAX_CHURCH_NAT validation (BUG-003) |
| `relativist-core/src/encoding/recipe.rs:380-386` | Compile-time bound or honest rename (MF-001) |
| `relativist-core/src/encoding/biguint_readback.rs` | Reword module rustdoc (SF-002) |
| `relativist-core/src/encoding/horner.rs::tests` | PT-0715-06 skip-rate guard (SF-004) |
| `relativist-core/tests/horner_distributed_g1.rs` | Stronger structural witness (BUG-004) |
| `relativist-core/tests/horner_codec_cli_roundtrip.rs` | **CREATE.** End-to-end CLI test for `relativist compute --codec horner --input '{...}'`. Covers BUG-001 + BUG-002 fixes. ~80 LoC. |
| `relativist-cli/Cargo.toml` (if Option A for MF-001) | Add `static_assertions = "1"` dev-dep |

## Files explicitly OUT of scope

- HIGH/MEDIUM/LOW bugs from QA report not listed above (BUG-005, BUG-007..BUG-015): defer to follow-up TASKs if reopened, or close as won't-fix during merge review.
- Mackie/Pinto-style readback for nested DUP: SPEC-27 §5.1 Future Work; explicit non-goal of D-015.
- Docker T13 full implementation: cicd follow-up per Round 2 SC-010.
- R26/R27/R28 (RecipeEncoder generators, AssignRecipe.encoder_name, worker registry dispatch): deferred to SPEC-25 M7 per recipe.rs rustdoc.

## Acceptance criteria

1. **AC-1:** `cargo run --release -- compute --codec horner --input '{"coeffs":[3,2,5,1],"x":2}'` returns `{"value":"43","bit_length":6}`. (BUG-001)
2. **AC-2:** `cargo run --release -- compute --codec horner --input '{"coeffs":[42],"x":99}'` returns `{"value":"42","bit_length":6}`. (BUG-002)
3. **AC-3:** `cargo run --release -- compute --encoder church_add --input '{"a":2000000,"b":1}'` returns `EncodeError::InvalidInput` with bound message (NOT panic). (BUG-003)
4. **AC-4:** Adding `impl RecipeEncoder for HornerCodec { ... }` in a patch causes the test to fail (compile-time if Option A; runtime if Option B). (MF-001)
5. **AC-5:** PT-0715-06 fails when readback skip rate exceeds 50%. (SF-004)
6. **AC-6:** `horner_distributed_g1_in_process_structural_isomorphism` test uses a stronger structural witness (not `format!("{:?}")` debug equality). (BUG-004)
7. **AC-7:** `tests/horner_codec_cli_roundtrip.rs` passes covering AC-1, AC-2, AC-3.
8. **AC-8:** All existing pisos hold or rise: default ≥ 1885 (Windows) / 1888 (Linux) / +3 from new IT. (Linux floor: 1886 + 3 = 1889. Windows: 1885 + 3 = 1888.)
9. **AC-9:** `cargo clippy --all-features -- -D warnings` clean.
10. **AC-10:** `cargo fmt --check` clean.
11. **AC-11:** Bug reports `docs/qa/D-015-spec27-horner-qa.md` and `docs/reviews/D-015-spec27-horner-review.md` updated with `Status: FIXED` annotations on BUG-001/002/003 and MF-001/SF-002/SF-004.

## Sequencing note

This task lands as Stage 6 REFACTOR of the D-015 bundle. After it
closes, D-015 is shippable. **It can be done in the same dispatch as
TASK-0720 (D-014 Stage 6 REFACTOR)** because both are owned by the
same `developer` agent and they touch different modules:
- TASK-0720: `relativist-core/src/{bench/, commands.rs (campaign dispatch)}`, `scripts/`
- TASK-0721: `relativist-core/src/{encoding/, commands.rs (compute dispatch)}`

The two CHANGES to `commands.rs` are additive on different functions
(`run_bench_command` for D-014 vs `run_compute_with_encoder` for D-015),
no merge conflict.

The combined REFACTOR dispatch should produce:
- Per-TASK commits for TASK-0720 and TASK-0721 fixes
- Final test run validating both bundles' acceptance criteria simultaneously
- Updated bug reports with FIXED annotations
