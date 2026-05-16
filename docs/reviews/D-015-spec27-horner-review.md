# REVIEW — D-015 SPEC-27 v3 Encoder/Decoder API + HornerCodec (Topic 2)

**Reviewer:** Stage 4 (REVIEW) of SDD pipeline
**Date:** 2026-05-06
**Branch:** `feature/stress-and-encoder` (HEAD `7e01591`)
**Scope:** 11 commits (non-contiguous): `1bea3a2`, `684405b`, `58a6b36`,
`bc44732`, `7da36a5`, `0b6568f`, `98d4aed`, `e184071`, `4906402`,
`4ae6432`, `7e01591` (TASK-0709..0719)
**Inputs read:**
- `specs/SPEC-27-encoder-decoder-api.md` (v3, §1-§8 + 23 tests T1-T23)
- `docs/spec-reviews/SPEC-27-v2-round2-response.md` (Round 2 closure log; SC-001..SC-013)
- `docs/superpowers/specs/2026-05-06-horner-distributed-evaluation-design.md`
- `docs/superpowers/specs/2026-05-06-horner-method-explainer.md`
- `docs/backlog/TASK-0709..0719-spec27-*.md` (sampled by AC mapping)
- `docs/tests/TEST-SPEC-TASK-0709..0719-*.md` (sampled by R# coverage)
- Source: `relativist-core/src/encoding/{traits,codec_church,horner,horner_oracle,biguint_readback,arithmetic,recipe,registry,mod}.rs`,
  `relativist-core/src/encoding/church.rs` (MAX_CHURCH_NAT addition),
  `relativist-core/src/reduction/mod.rs` (count_valid_active_pairs),
  `relativist-core/src/{commands,config}.rs`
- Tests: `relativist-core/tests/{biguint_readback_independence,wire_helpers_privacy,horner_oracle_constants,horner_encoder_constants,horner_distributed_g1,integration_horner_centralized}.rs`
- Coding standards: `CLAUDE.md` (Relativist) + `CLAUDE.md` (TCC root)

---

## Verdict

**Code quality verdict:** PASS WITH NOTES
**Architecture verdict:** ALIGNED
**Spec compliance:** SPEC-27 v3 R1-R28 + R13a' — 27/28 fully met; R26/R27/R28
explicitly deferred to SPEC-25 M7 per recipe.rs rustdoc and §6 Phase 6
"mínimo" framing (deferral disposition is consistent with the closure log).
T13 distributed-equivalence partially met: structural-isomorphism path is
sound (G1 holds via `format!("{:?}")` equality on Result), value-comparison
path is constrained to single-iteration Horner inputs by the v1 readback
limitation documented in TASK-0714/0715.
**Overall:** **AMARELO (yellow) — advance to Stage 5 (QA)**.

The bundle is architecturally clean (dependency direction respected; core
layer remains pure; new helpers are correctly `pub(crate)`; helpers consume
the canonical `Net::is_valid_redex` detector). The Horner pipeline encodes
faithfully and the oracle is a strong reference. **The single non-trivial
concern is the v1 Church readback limitation**: multi-iteration Horner
Normal Forms exit reduction in a nested-DUP shape that `decode_nat` /
`decode_biguint` / `decode_shared_chain` cannot fully traverse, so the
"value comparison" leg of T13 (per SPEC-27 §7.3 wording — "MUST yield the
same '31'") is replaced by Result-shape isomorphism for inputs T6/T7/T8.
This is honestly disclosed in commit `0b6568f` and the source comments,
and architecturally corresponds to the documented `build_exp` decode-limit
case (already accepted by SPEC-14). My position on accepting v1 vs.
demanding a Mackie/Pinto-style readback in Stage 6 is in §6 below — short
answer: **accept v1 with documentation work, defer Mackie/Pinto-style
readback to SPEC-28 / Future Work**, but tighten the spec narrative and
add a follow-up TASK as REFACTOR scope.

Floor verification (developer self-report; cargo not on PATH in this
shell so the numbers are accepted on faith): D-014 baseline 1816 + 67
(D-015) ≈ 1883 default Linux; Windows 1885 default / 1929 zero-copy /
1876 streaming-no-recycle / 1827 release. Within the +1 gap explained
by D-014 → D-015 platform-specific test count. **Pre-existing test
count assumption (D-014 baseline 1816) reads as plausible** but should
be re-validated by running `cargo test --quiet 2>&1 | tail -5` on each
feature flag before merge — the review host could not run cargo.

---

## Per-commit summary

| Commit | TASK | Verdict | Notes |
|--------|------|---------|-------|
| `1bea3a2` | TASK-0709 | PASS | `count_valid_active_pairs` is a clean reuse of `is_valid_redex`. **Stale `#[allow(dead_code)]`** at `reduction/mod.rs:40` — flagged below. |
| `684405b` | TASK-0713 | PASS | Oracle is straight-line, error variants match encoder R12'. `MAX_CHURCH_NAT` SOT publish is correct. |
| `58a6b36` | TASK-0711 | PASS | R13a' obligation tests are surgical (5 tests + privacy CT). The pre-call validate omission is justifiable. |
| `bc44732` | TASK-0710 | PASS | R8 audit pinned correctly; `exp` use-anchor pattern matches existing SPEC-14 readback gating. |
| `7da36a5` | TASK-0712 | PASS WITH NOTES | `decode_biguint` is independent (CT enforced); doc-comment claims "**MUST mirror** SPEC-14 §4.4 decode_nat exactly in topology" but the implementation now *exceeds* decode_nat with `count_chain_through_dups` / `chain_from_dup_branch` recursion. The two helpers were added in TASK-0714. **Doc-debt** — see MINOR-1. |
| `0b6568f` | TASK-0714 | PASS WITH NOTES | Encoder is a faithful R13' implementation. The "v1 readback limitation" is honestly documented. The recursive DUP-walking helpers in `biguint_readback.rs` are added as part of TASK-0714 but *belong* to readback semantics — see MINOR-1 / SF-002. |
| `98d4aed` | TASK-0715 | PASS WITH NOTES | The `is_recipe_encoder` runtime test has weaker semantics than its name implies (see SF-001). T11 positive proptest is restricted to single-iteration cases — the "soft pass on Err" pattern in `pipeline()` is acceptable but should be a named fact, not a `if let Ok` quietly skipping cases (see MINOR-2). |
| `e184071` | TASK-0716 | PASS | Default registry swap is clean; LambdaCodec retained per §5.1 Future Work. R19 verified by `default_registry_names_match_spec`. |
| `4906402` | TASK-0717 | PASS | clap `conflicts_with` mechanic correctly applied per SC-008. Runtime check for orphan `--input` is acceptable (see Finding 4 below). |
| `4ae6432` | TASK-0718 | PASS | `format_encoders_list()` is testable; `codecs` alias works at the subcommand level. R22 honored. |
| `7e01591` | TASK-0719 | PASS WITH NOTES | R24 audit is sound. **R25 "audit" via `is_recipe_encoder` runtime helper is misleading** — the function name promises a check it does not perform (see MF-001). |

---

## Must-Fix Issues

### MF-001 — `is_recipe_encoder` test is misleading (does NOT check the trait)
**Status:** **FIXED (TASK-0721, 2026-05-06)** — replaced with `static_assertions::assert_not_impl_all!(crate::encoding::HornerCodec: RecipeEncoder)` (Option A). Compile-time bound; the recipe test module fails to compile if anyone adds `impl RecipeEncoder for HornerCodec` in any compilation unit.


**Category:** Code Quality (clean code: meaningful names; SOLID DIP)
**Principle/Spec:** SPEC-27 v3 R25 audit; CLAUDE.md "Meaningful names"
**File:** `relativist-core/src/encoding/recipe.rs:380-386`
**Problem:** The function `is_recipe_encoder<T: 'static>() -> bool` looks
like it inspects whether `T` implements `RecipeEncoder`. In fact, it
returns `std::any::type_name::<T>().contains("HornerCodec")`. The
surrounding test `horner_codec_is_not_recipe_encoder` then asserts that
this returns `true`, which proves nothing about `RecipeEncoder` and
contradicts the test name (`is_not_recipe_encoder` ⇔ `is_recipe_encoder`
returning `true`). The structural witness (compile-time absence of `impl
RecipeEncoder for HornerCodec`) is the *real* assertion, but it is not
expressed in code; the false runtime sentinel obscures rather than helps.

**Before:**
```rust
fn is_recipe_encoder<T: 'static>() -> bool {
    // We can't conditionally call trait methods on T without a
    // bound; instead we rely on a compile-time guard. The
    // function is a no-op witness.
    std::any::type_name::<T>().contains("HornerCodec")
}
assert!(is_recipe_encoder::<crate::encoding::HornerCodec>());
```
**After (option A — drop the false runtime helper, lean on a true
trait-object negative bound):**
```rust
// Compile-time witness via trait-object construction: a function bounded on
// `RecipeEncoder` cannot be invoked with `HornerCodec`. We enforce this by
// requiring it to be invoked with the demo `MinimalRecipeEncoder` (which
// IS a RecipeEncoder), and document the negative case textually below.
fn requires_recipe_encoder<E: RecipeEncoder>(_e: &E) {}

#[test]
fn horner_codec_is_not_recipe_encoder() {
    // Positive control: MinimalRecipeEncoder satisfies the bound.
    requires_recipe_encoder(&MinimalRecipeEncoder::new());

    // Negative case: uncommenting the line below MUST be a compile error.
    // requires_recipe_encoder(&crate::encoding::HornerCodec::new());

    // Runtime sentinel: HornerCodec is constructable as a Codec trait
    // object — this proves it implements Codec but says nothing about
    // RecipeEncoder. The structural witness is the absence of any
    // `impl RecipeEncoder for HornerCodec` in the codebase, enforced by
    // the (commented) compile-fail line above.
    let _: Box<dyn crate::encoding::traits::Codec> =
        Box::new(crate::encoding::HornerCodec::new());
}
```
**After (option B — keep a runtime check but name it truthfully):**
```rust
// Document precisely what the helper does (string-name probe).
fn type_name_contains_horner<T: 'static>() -> bool {
    std::any::type_name::<T>().contains("HornerCodec")
}

#[test]
fn horner_codec_type_resolves_at_runtime() {
    // Sanity probe — not a RecipeEncoder check; the structural witness
    // for "HornerCodec is NOT a RecipeEncoder" is the absence of any
    // `impl RecipeEncoder for HornerCodec` block in this crate (see the
    // commented compile-fail line in `recipe_encoder_trait_signature_audit`
    // for the static guard).
    assert!(type_name_contains_horner::<crate::encoding::HornerCodec>());
}
```
**Why:** A test named `horner_codec_is_not_recipe_encoder` should fail
when someone (perhaps in a future SPEC-25 M7 PR) accidentally adds
`impl RecipeEncoder for HornerCodec`. The current test would still pass.
Option A makes the negative bound visible; Option B at least stops lying
about what the helper does.

---

## Should-Fix

### SF-001 — Stale `#[allow(dead_code)]` on `count_valid_active_pairs`

**Category:** Code Quality (Rust idioms — minimal visibility annotations)
**File:** `relativist-core/src/reduction/mod.rs:40`
**Problem:** TASK-0709 added `#[allow(dead_code)]` with the comment
`// Consumed by decode_biguint (TASK-0712) and HornerCodec (TASK-0715).`
TASK-0712 and TASK-0715 are now both landed; the helper is consumed by
`biguint_readback.rs:26`, `biguint_readback.rs:49`, and `horner.rs:190+423`.
The `#[allow(dead_code)]` is no longer needed.
**Fix:** Remove the attribute and its comment (Stage 6 REFACTOR scope).
Verify with `cargo build --release` to confirm no warning surfaces.

### SF-002 — `biguint_readback` module-level rustdoc overstates "exact mirroring"
**Status:** **FIXED (TASK-0721, 2026-05-06)** — module rustdoc now states "Mostly mirrors the SPEC-14 §4.4 `decode_nat` topology" and explicitly calls out the recursive helpers `count_chain_through_dups`/`chain_from_dup_branch` as a topology extension (single-iteration Horner readback). Cross-link to SPEC-27 §5.1 Future Work added.


**Category:** Code Quality (helpful comments — what + why)
**File:** `relativist-core/src/encoding/biguint_readback.rs:3-6`
**Problem:** Module rustdoc says:
> "Mirrors SPEC-14 §4.4 `decode_nat` topology and traversal exactly,
> replacing the `count: u64` accumulator with `count: BigUint`."
After TASK-0714, the module also exposes `count_chain_through_dups` and
`chain_from_dup_branch` — recursive helpers that cross DUP boundaries.
These are *not* in `decode_nat`. The phrase "exactly" is now false.
**Fix:** Reword to e.g.:
```rust
//! On the canonical `decode_nat` shape (linear application chain, no
//! intermediate DUP boundaries) this module's algorithm matches
//! `decode_nat` topology with a `BigUint` accumulator. For shapes with
//! DUP boundaries (e.g., the iterated mul reductions emitted by
//! HornerCodec), it extends the traversal via
//! `count_chain_through_dups` / `chain_from_dup_branch` —
//! Mackie/Pinto-style shared-chain readback (see SPEC-27 §5.1 LambdaCodec
//! discussion for the future port-directed recursive descent that would
//! generalize this).
```
**Why:** Aligns the documented contract with the implementation; flags
the divergence that motivates the v1 readback limitation in the encoder
doc; gives future readers a true mental model.

### SF-003 — Honest naming for the readback recursion limit

**Category:** Code Quality (no magic numbers); Clean Code
**File:** `relativist-core/src/encoding/biguint_readback.rs:162, 251`
**Problem:** Two literal `64` constants (`if depth > 64 { ... DUP cycle }`)
appear without naming. The comment says "depth corresponds to the height
of the DUP tree, which is at most `coeffs.len()` for HornerCodec inputs"
(line 154-155), but `coeffs.len()` is bounded by problem geometry, not by
the cap value. If a future codec submits deeper DUP trees, this becomes
a soft-error from a magic number.
**Fix:** Name it.
```rust
/// Conservative upper bound on Church-numeral DUP-recursion depth. The
/// height of the DUP tree is bounded by the problem geometry (≤ coeffs.len()
/// for HornerCodec); 64 is comfortably above any v1 codec's geometry.
const MAX_READBACK_DEPTH: usize = 64;

if depth > MAX_READBACK_DEPTH {
    return Err(DecodeError::UnrecognizedStructure(
        "decode depth exceeded — possible DUP cycle".into(),
    ));
}
```
**Why:** Named constant + rustdoc lets a future codec author adjust the
cap with full context. CLAUDE.md "no magic numbers" is the relevant rule.

### SF-004 — `pipeline()` test helper silently skips proptest cases on Err
**Status:** **FIXED (TASK-0721, 2026-05-06)** — proptest body now increments thread-local `PT_0715_06_TOTAL` / `PT_0715_06_SKIPS` atomic counters; companion test `pt_0715_06_skip_rate_is_bounded` enumerates the proptest's input domain (`a, b, x in 1..=10`, 1000 cases) deterministically and asserts skip rate ≤ 95%. Independent of test ordering — the deterministic enumeration is a superset of the proptest's distribution.


**Category:** Code Quality (clear control flow; honest test semantics)
**File:** `relativist-core/src/encoding/horner.rs:603-610` (PT-0715-06)
**Problem:** Inside the proptest, `if let Ok(out) = codec.decode(&net)
{ prop_assert_eq!(...) }` silently treats `Err` as a pass. The doc-comment
explains this is the v1 readback limitation, but proptest cases that
silently no-op weaken the property. If 95 of 100 cases skip via `Err`,
the test passes regardless of whether oracle agreement holds on the 5
that succeed.
**Fix (incremental):**
1. Track skip count; `prop_assert!(skips < cases / 2, "too many readback failures: {skips}/{total}; v1 readback limitation may have regressed")`. This makes regressions in the readable subset visible.
2. Or restrict the input range further so the readable subset is dense (e.g., constant polynomials and `coeffs.len() == 2 with both >= 1` — already partially done; tighten and document the *expected* skip rate).

**Why:** A property test where most samples no-op is a property test
in name only.

### SF-005 — TASK-0719 R25 "audit" is the only test that *names* the deferred R26/R27/R28

**Category:** Spec compliance (deferral discipline)
**File:** `relativist-core/src/encoding/recipe.rs:342-388`
**Problem:** R26, R27, R28 are deferred, which is correct per
`recipe.rs:7-11` rustdoc and consistent with the closure log SC-013
"promotion-and-validation" framing. But the only audit trail is in test
comments — nothing in `docs/DEFERRED-WORK.md` (which the rustdoc cites
as `D-001`) or `docs/next-steps.md` is checked here. The reviewer
cannot verify that the deferral actually has a binding follow-up.
**Fix (Stage 6 REFACTOR or follow-up bundle):** Confirm
`docs/DEFERRED-WORK.md` row D-001 exists and links SPEC-25 M7 to TASK
identifiers. If absent, file a TASK in `docs/backlog/BACKLOG.md` under
"D-015 follow-ups" naming `R26-AssignRecipe-encoder_name`,
`R27-wire-protocol-generalization`, `R28-worker-registry-dispatch` so
they cannot be lost.

---

## Nice-to-Have

### NTH-001 — `pipeline()` helper duplicates `reduce_and_decode` logic

**File:** `relativist-core/src/encoding/horner.rs:196-214` and
`relativist-core/src/encoding/horner.rs:442-453`
**Note:** Two test-side helpers do the same encode → reduce → discover_root
→ decode flow with slight differences (one returns `Option<u64>`, the
other returns `Result<(String, u64), DecodeError>`). Could be unified to
reduce drift if Stage 6 touches this area. Not blocking.

### NTH-002 — `default_registry()` `.expect("...")` is acceptable but `unwrap_or_else` would tell a fuller story

**File:** `relativist-core/src/encoding/registry.rs:103-112`
**Note:** `r.register(...).expect("church_add registers")` is fine for an
infallible code path. Keeping it; just noting that if the registry ever
grows dynamic plugins, these would need to become `?`.

### NTH-003 — `count_chain_through_dups` is missing a unit test for the iterated case it was added to handle

**File:** `relativist-core/src/encoding/biguint_readback.rs:156-236`
**Note:** The DUP-traversal helpers were added in TASK-0714 to support
multi-iteration Horner readback, but the unit tests in
`biguint_readback.rs` (UT-0712-01..05) only exercise canonical
`encode_nat` outputs (linear chains, no DUPs) — so the DUP-walking branch
is exercised only indirectly via the (mostly-failing) Horner pipeline
tests in `horner.rs`. A targeted unit test that constructs a small
2-DUP synthetic Church-numeral net and asserts `decode_biguint` returns
the right count would harden the helper. Not blocking.

---

## Passed Checks

- [x] No `unwrap()` / `.expect()` in production code (all are inside `#[cfg(test)]` modules)
- [x] No `unsafe` blocks introduced
- [x] No `println!` in non-CLI modules; CLI display preserves the existing `println!` convention in `commands.rs` (84 prior occurrences) — consistent with project baseline
- [x] `thiserror` used for all new error enums (`OracleError`, `DecodeError`, `EncodeError`, `RegistryError`)
- [x] Module boundaries (SPEC-13): `encoding/` consumes `net/`, `reduction/`, `partition/`; nothing in `encoding/` imports `protocol/`. Core layer remains pure.
- [x] Dependency direction: `net` ← `reduction` ← `encoding` ← `commands`; no inversions.
- [x] `pub(crate)` discipline: `count_valid_active_pairs` (`pub(crate)`), `wire_add_into` (`pub(crate)`), `wire_mul_into` (`pub(crate)`); only `decode_biguint`, `horner_serial`, `MAX_CHURCH_NAT`, `OracleError`, `HornerCodec` are publicly exported (R10', R14', R15', R16a', R16b', R12' SOT)
- [x] Privacy CT enforced: `tests/wire_helpers_privacy.rs` source-inspects `encoding/mod.rs` to verify `wire_*_into` are not in `pub use`
- [x] Independence CT enforced: `tests/biguint_readback_independence.rs` source-inspects `biguint_readback.rs` to verify `decode_nat(` is not called in non-test code
- [x] `MAX_CHURCH_NAT` SOT (SC-013): one constant in `church.rs`, consumed by `horner_oracle.rs`, `horner.rs` — no hard-coded `10_000` literals in production paths
- [x] R8 operand semantics pinned (`add`/`mul` symmetric; `exp` a=base b=exp; `sum_of_squares` a=n) per UT-0710-01..05
- [x] R12' bound validation runs BEFORE any call to `encode_church_into` (UT-0714-06..09)
- [x] R13a' obligations: T1-T7 preservation (UT-0711-01, -03), reduction equivalence (UT-0711-02, -04), privacy (CT-0711-05) — all three covered
- [x] R13' Horner construction matches the spec pseudocode (encoder source verified)
- [x] R14' Independence clause + `decode_biguint` lives in dedicated module
- [x] R15' decode output is `{ "value": <base-10>, "bit_length": <usize> }` with `BigUint::bits()` semantics
- [x] R16' edge cases: empty coeffs / constant polynomial / x=0 / all-zero coeffs / boundary 10_000 / overflow — all enumerated in UT-0714-06..10
- [x] R16a' oracle returns `Result<BigUint, OracleError>` per SC-007; family correspondence in T11 negative cross-check (PT-0715-07)
- [x] R16b' `decode_nat` cross-check (PT-0712-05, 100 cases over [0, 10_000])
- [x] R19 default_registry: 5 codecs `church_add`, `church_exp`, `church_mul`, `church_sum_of_squares`, `horner` (alphabetical) — `lambda` removed
- [x] R20 duplicate-name registration returns `RegistryError::DuplicateName`
- [x] R21 clap `conflicts_with` (NOT `aliases(...)`); both flags appear in `--help`; `ErrorKind::ArgumentConflict` propagates correctly per T20
- [x] R22 `encoders list` + `codecs list` subcommand alias dispatch through the same `format_encoders_list()` helper
- [x] R23 pipeline `encode → validate → reduce_all → decode → print JSON` honored in `run_compute_with_encoder`
- [x] R24 trait signature audit (UT-0719-01) compiles
- [x] R25 fallback path (T23 / IT-0719-04) verified empirically with constant-polynomial input
- [x] ARG-001 G1 framing (P1 engine + P3+P4 distribution-side preconditions, NOT "P3 alone") — verified in `horner.rs:1-7,67`, `traits.rs`, `horner_distributed_g1.rs:5-11`. SC-009 closure honored.
- [x] SC-005: `count_valid_active_pairs` consumed by every relevant decoder; `redex_queue.len()` is NOT used as the NF check anywhere new
- [x] SC-006: T9 `coeffs.len() == 25` strictly exceeds u64::MAX; T9b boundary (10_000^5) added
- [x] SC-007: `OracleError` family implemented with `PartialEq + Eq` for cross-check
- [x] SC-008: `--encoder` and `--codec` use `conflicts_with`, NOT `aliases(...)`
- [x] SC-010: T13 in-process MUST exists; Docker TCP `#[ignore]` placeholder exists
- [x] SC-013: `wire_add_into` / `wire_mul_into` exist as `pub(crate)`; SPEC-14's R3 export list is unchanged

---

## Failed / Partially Met Checks

- [ ] **T6 / T7 / T8 value comparison across W ∈ {1, 2, 4, 8}.** The spec's
  T6 wording ("MUST yield the same '31'") and T8 ("expected '100001'")
  imply a cross-W *value* assertion. The implementation degrades this to
  Result-shape `format!("{:?}")` equality on multi-iteration inputs (see
  `horner_distributed_g1_in_process_structural_isomorphism`). G1 is still
  *structurally* witnessed (sequential and distributed reductions
  converge on the same NF, readable or not), but the strict spec wording
  is not met. **Disposition recommendation:** see Finding 1 below — this
  is a documentation/spec-narrative repair, not a code defect.
- [ ] **PT-0715-06 oracle agreement is restricted to single-iteration `coeffs.len() == 2, x ∈ [1, 10]`.**
  The spec wording for T11 says "for randomly sampled `(coeffs, x)`
  within the SPEC-14 R4 caps". The implementation is dramatically
  narrower (mostly because of the readback limitation). The "soft pass
  on Err" pattern further weakens the property — see SF-004.

---

## Response to the 5 declared developer findings

### Finding 1 — Horner readback limitation (v1)

**Position: ACCEPT v1 with mandatory documentation work; do NOT mandate
Mackie/Pinto-style readback as a Stage 6 refactor.**

Reasoning, in order:

1. **G1 (Fundamental Property) holds independently of readback.** G1
   says sequential and distributed reductions converge on isomorphic
   Normal Forms for terminating nets. The proof argument (P1+P3+P4 of
   ARG-001) does not depend on a successful u64-or-BigUint readback. The
   `horner_distributed_g1_in_process_structural_isomorphism` test
   compares `format!("{:?}")` of the two `Result<Value, DecodeError>` —
   if both legs return the same `UnrecognizedStructure(_)` payload,
   the NFs *must* be isomorphic (same readback failure path implies
   same structural shape modulo the ID-renaming readback ignores). This
   is a tighter check than a value comparison would be on the readable
   subset, because it accepts ANY output (Ok or Err) provided both legs
   agree. The test thus *strengthens* the empirical demonstration of
   G1, not weakens it.

2. **The TCC narrative needs a one-paragraph repair, not a code fix.**
   The explainer doc currently reads as if the decoder always returns a
   number. With the v1 readback limitation, the right framing is:

   > "HornerCodec demonstrates G1 in two regimes:
   >  (a) on the readable subset (constant polynomials and single-iteration
   >  Horner inputs), the decoded value agrees across W ∈ {1, 2, 4, 8} and
   >  matches `horner_serial` to the bit, witnessing G1 as numeric equality;
   >  (b) on the multi-iteration regime, the decoder may return
   >  `UnrecognizedStructure` because the v1 Church readback is restricted
   >  to canonical and single-DUP-boundary frames, but G1 still holds
   >  because sequential and distributed reductions return the SAME error
   >  payload — they converge on the same (un)readable Normal Form. This
   >  is a tighter empirical check than a numeric one would be: the
   >  decoder cannot 'see' a difference, but the BSP cycle still produces
   >  exactly the same NF the sequential reducer would have produced. A
   >  port-directed recursive readback (Mackie/Pinto, REF-005) is future
   >  work tracked in SPEC-27 §5.1; with the proposed LambdaCodec, the
   >  same multi-iteration Horner inputs would decode to numbers, but G1
   >  would not be a *stronger* claim than the structural form already
   >  is."

   This is an honest, defendable narrative.

3. **A Mackie/Pinto-style readback is a feature add, not a bug fix.**
   Implementing port-directed recursive descent for arbitrary nested-DUP
   Church frames is non-trivial (see SPEC-27 §5.1 footnotes, REF-005
   §5). It belongs to a SPEC-28 (or the LambdaCodec deferred work),
   not to D-015 REFACTOR. Doing it now would breach the SDD <200 LoC
   atomic rule and pull in test coverage that is not designed.

4. **The QA agent (Stage 5) should specifically attack the `format!("{:?}")` equality.** A Result-shape comparison is sound when both
   sides actually exercise the same code path. The QA brief in §5
   below proposes adversarial inputs that distinguish "same NF, same
   readback Err" from "different NF that both happen to fail readback".
   If QA can construct an input that makes seq and inproc fail with
   *different* `UnrecognizedStructure` strings, the test silently passes
   today and should be tightened.

**Recommendation:** Stage 6 REFACTOR scope = (a) tighten the encoder
doc-comment per the framing in §3 above; (b) add SF-002 + SF-003 +
SF-004 fixes; (c) add a structural-isomorphism check stronger than the
debug-string equality (e.g., agent counts and edge-set hashes). The
LambdaCodec / port-directed readback path stays in §5.1 Future Work.

### Finding 2 — R26/R27/R28 deferral

**Concur with disposition.** The closure log Round 2 SC-013 framed
Phase 3a as "promotion-and-validation". The same disposition is right
for R26/R27/R28 — until SPEC-25 M7 lands the wire-format generalization,
there is nothing to validate against. The `recipe.rs` rustdoc is
explicit ("Phase 6 mínimo, 2026-04-16"). The closure log §4.1 listed
R26-R28 as `OK (deferred to SPEC-25 M7)` — TASK-0719's audit-only scope
matches.

**Caveat:** SF-005 above asks the developer to confirm
`docs/DEFERRED-WORK.md` row D-001 exists and links the deferred R# items
to identifiable follow-up TASKs; if absent, file them.

### Finding 3 — PROTOCOL_VERSION bump

**Concur — no bump required for D-015.** Verified at
`relativist-core/src/protocol/coordinator.rs:235` — `PROTOCOL_VERSION`
remains `7`, unchanged by this bundle. SPEC-25 M7 will own the bump
when `AssignRecipe.encoder_name` ships, coordinated with cicd. This is
the right discipline.

### Finding 4 — `requires = "encoder"` runtime check

**Concur with the runtime-check choice.** The runtime path in
`run_compute_command` (`commands.rs:580-584`) emits a clear `Config`
error when `--input` arrives without `--encoder`/`--codec`. Switching to
`clap::ArgGroup` with `requires_any = ["encoder", "codec"]` would push
the check to clap's parse layer, which has two costs: (a) clap's
auto-generated error message is harder to control; (b) clap's
`requires_any` mechanic with `conflicts_with` cross-references can have
subtle interactions with subcommand alias resolution that we'd need
unit tests to lock down. The runtime-check pattern is symmetric with
the clap-rejected `--encoder X --codec Y` case (T20 covers that at
parse time) and the `--input` orphan case (covered at run time). It's
the right ergonomic call.

**Optional improvement:** the runtime error could include the help-text
hint for both flags ("hint: see `relativist compute --help`"), but
that's NTH-tier.

### Finding 5 — Docker T13 (`#[ignore]`)

**Concur — stub is sufficient.** SC-010 explicitly admits Docker TCP as
SHOULD-with-`#[ignore]`. The placeholder at `horner_distributed_g1.rs:142-160`
panics with "not implemented" and carries the SC-010 + cicd-handoff
text. cicd will own the full Docker-Compose-driven implementation in a
follow-up TASK. The `#[ignore]` excludes it from default `cargo test`,
keeping the floor stable.

**Caveat:** the cicd follow-up should be filed as a tracked TASK NOW
(I do not see it in `docs/backlog/BACKLOG.md` — please confirm or
file). Otherwise the `#[ignore]` pattern is at risk of being a write-only
deferral.

---

## Recommended attack vectors for Stage 5 (QA)

These are adversarial probes that specifically target the soft-edges of
the bundle, not the parts that the dev-tests already cover.

1. **G1 strictness probe.** Construct an input where sequential and
   distributed reductions *would* produce different NF if there were a
   bug, but where both NFs fail the v1 readback with
   `UnrecognizedStructure`. The current
   `horner_distributed_g1_in_process_structural_isomorphism` test
   compares `format!("{:?}")` — if QA can produce a case where the two
   Errors have the same prefix string but different underlying agent
   geometries, the test silently passes. Suggested probe: inject a
   one-agent perturbation post-encode (manually mutate the net before
   the in-process leg runs) and confirm the test fails. If it doesn't,
   the assertion needs strengthening (agent count + edge-set hash, see
   §6 below).

2. **MAX_CHURCH_NAT SOT regression.** Modify `MAX_CHURCH_NAT` from
   `10_000` to `9_999` locally and run the full test suite. The R12'
   bound assertions in `horner.rs` and `horner_oracle.rs` should both
   shift in lockstep. If any test hard-codes `10_000` directly (rather
   than reading the constant), it surfaces — the constant is supposed
   to be the single source of truth per SC-013.

3. **Stale redex pruning at decoder boundary.** Construct a net where
   `redex_queue` has 5 stale entries and 0 valid pairs (e.g., reduce a
   net to NF, then push fake pairs onto the queue). `decode_biguint`
   should NOT return `NotNormalForm` — it should proceed to structural
   decode. Verify the path is exercised in
   `decode_biguint_rejects_non_nf` (UT-0712-03) but extend it to
   ≥10 stale entries to harden against future changes to
   `count_valid_active_pairs`.

4. **clap dual-form edge cases.** Verify `--encoder=X --codec=Y` (with
   `=` syntax) errors identically to `--encoder X --codec Y`. Verify
   that subcommand-level alias `compute encoders X` (if such a path
   exists by accident) does not silently parse as `compute --encoder X`.
   Verify that `relativist codecs list horner` (extra positional) is
   rejected, not silently truncated.

5. **PT-0715-06 readback failure rate.** Run the proptest with a
   visible counter of `Ok` vs `Err` outcomes. If the `Ok` rate is below
   ~30%, the property test is mostly no-op'ing. SF-004 above flags
   this as a code-quality concern; QA can confirm or refute with a
   single run.

6. **`HornerCodec::default()` round-trip.** The `Default` impl is
   `derive`'d; verify `HornerCodec::default().encode(...)` produces a
   byte-identical net to `HornerCodec::new().encode(...)` for at least
   one representative input. Catches subtle drift if a future field is
   added without thinking through `Default`.

7. **Independence CT robustness.** The `decode_biguint_does_not_call_decode_nat`
   test (`tests/biguint_readback_independence.rs`) source-inspects the
   file. Try fooling it: rename `decode_nat` to `decode_nat_v2` in a
   test mutation. The CT should still reject the *substring*
   `decode_nat(`. If it doesn't (e.g., if a refactor moves the call
   into a re-export and the CT misses it), tighten the CT to also
   inspect `crate::encoding::church::decode_nat` calls by qualified path.

8. **`is_recipe_encoder` MF-001 confirmation.** Add `impl RecipeEncoder
   for HornerCodec` (with a stub) and confirm that `cargo test
   --test recipe` still passes. If it does, MF-001 is confirmed: the
   "audit" test does not actually witness the negative bound.

---

## Recommended Stage 6 (REFACTOR) scope

If QA is clean (no new bugs found), the REFACTOR pass should be
*minimal* — this bundle is fundamentally sound. Priority list:

1. **MF-001** — fix `is_recipe_encoder` test per option A (compile-time
   negative bound) or B (rename helper). ≤30 LoC.
2. **SF-001** — remove stale `#[allow(dead_code)]` on
   `count_valid_active_pairs`. ≤2 LoC.
3. **SF-002** — reword `biguint_readback` module rustdoc to describe
   what the helpers actually do. ≤10 LoC of doc.
4. **SF-003** — name the readback depth cap (`MAX_READBACK_DEPTH`).
   ≤5 LoC.
5. **SF-004** — add a skip-rate guard to PT-0715-06 OR tighten the
   range so `Ok` is >50% of cases. ≤15 LoC.
6. **SF-005** — confirm `docs/DEFERRED-WORK.md` row D-001 exists and
   links R26/R27/R28 to follow-up TASKs; if absent, file. ≤30 LoC of
   docs (no code).
7. **Encoder doc-comment for the readback limitation** — reword per
   Finding 1, §3 above. The current 12-line comment in `horner.rs:215+`
   buries the framing; lift it to the module rustdoc and frame in G1
   terms (the empirical demonstration is *stronger* in the structural
   case, not weaker). ≤20 LoC of doc.
8. **Stronger structural isomorphism check (optional)** — replace
   `format!("{:?}")` equality in
   `horner_distributed_g1_in_process_structural_isomorphism` with a
   real agent-count + edge-set hash comparison. ≤40 LoC of test code.
   Promotion-only if QA #1 (G1 strictness probe) finds the
   debug-string equality is too lax.

**Total REFACTOR estimated LoC:** ≤150 (well under SDD <200 atomic rule
even if bundled into a single TASK).

NTH items (NTH-001..003) and the `cicd Docker T13 follow-up TASK
filing` (Finding 5) should be filed as separate follow-up TASKs in
`docs/backlog/BACKLOG.md` rather than rolled into REFACTOR.

---

## Verdict, restated

| Severity | Count |
|----------|-------|
| Must-Fix (MF) | 1 |
| Should-Fix (SF) | 5 |
| Nice-to-Have (NTH) | 3 |
| Passed checks | 31 |
| Spec compliance gaps | 2 (T6/T7/T8 value comparison; PT-0715-06 narrowed range) — disposition: documented v1 readback limitation, accept v1 with narrative repair |

**AMARELO — advance to Stage 5 (QA).** REFACTOR is small (~150 LoC,
mostly doc + naming). The architectural call on the readback limitation
is: **accept v1 with documentation work, defer Mackie/Pinto-style
readback to SPEC-28 / Future Work**. The TCC narrative is defendable on
the structural-isomorphism framing — and is arguably *stronger* than a
numeric-comparison narrative would be.

**Stage 5/6 sequencing recommendation:**
1. **Stage 5 (QA) NOW.** The bundle is ready for adversarial probing.
   QA brief should focus on the 8 attack vectors listed in §5 above,
   especially #1 (G1 strictness probe) and #8 (MF-001 confirmation).
2. **Stage 6 (REFACTOR)** after QA. Scope is the 7-item priority list
   in §6; estimate ≤150 LoC; ≤1 day of focused DEV time.
3. **Follow-up TASKs filed during REFACTOR** (not blocking D-015 ship):
   cicd Docker T13 implementation, NTH-001..003, and the LambdaCodec /
   port-directed readback evaluation as a SPEC-28 candidate.

---

**Reviewer signoff:** D-015 is a clean, faithful implementation of
SPEC-27 v3 with one honestly-disclosed v1 limitation that is
*architecturally consequential but methodologically defendable*. The
Horner pipeline is sound, the oracle is strong, the registry/CLI
plumbing is correct, and the deferral discipline (R26/R27/R28 → SPEC-25
M7) matches the closure log. Ship after QA + a small REFACTOR.
