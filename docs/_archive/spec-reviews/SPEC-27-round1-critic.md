# SPEC-27 — Round 1: Spec Critic Review

**Date:** 2026-05-06
**Reviewer:** Spec Critic (adversarial)
**Target:** SPEC-27-encoder-decoder-api.md (status: Revised v2 — Topic 2 alignment)
**Predecessors consulted:** SPEC-00, SPEC-01, SPEC-02 (via cross-ref), SPEC-03 (via cross-ref), SPEC-07 (CLI), SPEC-09 (church_sum_of_squares), SPEC-14, SPEC-25, SPEC-26 (R1-R7), ARG-001 (P1-P6 framework, P3 in particular)
**Closure log consulted:** `docs/spec-reviews/SPEC-27-v2-closure-2026-05-06.md`
**Handoff consulted:** `docs/handoffs/2026-05-06-spec27-horner-revision-handoff.md`
**Design docs consulted:** `docs/superpowers/specs/2026-05-06-horner-distributed-evaluation-design.md`

---

## Overall Assessment

The Topic 2 revision is structurally sound: HornerCodec is clearly framed as the v1 illustration of ARG-001 P3, the LambdaCodec deferral is documented coherently, and the trait API (R1-R6) is minimally disturbed. However, the spec ships with **non-trivial gaps that would force a developer to guess** in three areas: (1) glossary debt — terms central to the spec ("Codec", "Encoder", "Decoder", "HornerCodec", "BigUint readback", "Horner's method", "NormalForm" as JSON error code) are introduced inline without SPEC-00 entries; (2) traceability to the actual SPEC-14 surface — R12' invokes "SPEC-14 R4" caps, but SPEC-14 R4 only caps `encode_nat(n)` itself; the spec elides the chain "coefficient cap → encode_church_into cap → SPEC-14 R4" and doesn't acknowledge that `x` is also passed through `encode_church_into` (so the cap on `x` is structurally identical to the cap on coefficients, not "additionally" inherited); (3) testability — R14' speaks of "Normal Form" but the `DecodeError::NotNormalForm { redexes: usize }` variant assumes the decoder can count redexes on a `&Net`, and the test plan does not specify the oracle's edge-case behavior for the most adversarial input (empty / out-of-range slices). Several requirements also import undefined infrastructure (`encode_church_into` is mentioned in R13' pseudocode but is a SPEC-14 internal helper, not part of the public API surface declared in SPEC-14 R4b — this is fine, but the spec must say so explicitly).

None of the issues block the empirical-validation goal of the v1 codec, but several would silently produce drift between developer interpretation and reviewer/QA expectations.

**Verdict:** NEEDS REVISION

---

## Issues

### SC-001: SPEC-00 glossary gap — six new terms introduced inline

**Severity:** HIGH
**Axis:** Consistency
**Section:** §2 Definitions, §3.4 (R10'-R16b'), §1 (Purpose)
**Requirement:** R3, R10', R11', R14', R15'
**Problem:** The spec introduces and uses several technical terms that are NOT defined in SPEC-00:
- `Codec` — defined inline in §2 (the trait combining Encoder+Decoder), but SPEC-00 has no `Codec` entry. SPEC-00 has only `Encoding` (8b.2) and `Decoding (Readback)` (8b.3) as activity terms; "Codec" as a Rust trait is novel.
- `Encoder` and `Decoder` (as Rust traits) — same status. SPEC-00's 8b.2/8b.3 describe processes, not types.
- `HornerCodec` and `ChurchArithmeticCodec` — defined only in this spec.
- `Horner's method` — defined in §2 but absent from SPEC-00 even though it now appears in R10', R11', R13', §5 Future Work, and three test cases.
- `BigUint` / `BigUint readback` — used in R14', R15', R16b', and several tests; never defined. A reader who does not know the `num-bigint` crate is left guessing whether `bit_length` is the bit count of the absolute value, the two's complement representation, or something else.
- `Normal Form` (R14' uses the capitalized term as if it had a specific meaning here): SPEC-00 5.5 defines Normal Form precisely, but the spec also encodes "not in Normal Form" as `DecodeError::NotNormalForm { redexes: usize }`, which is a stricter operational definition (zero redexes in `redex_queue`).

The SPEC-27 v2 closure log (§4.5) claims completeness, but SPEC-00 was never updated.
**Impact if unresolved:** Developer ambiguity. The glossary is the explicit single source of truth for terminology in this project (CLAUDE.md and spec-critic.md both list it as the first read-in-order). A future reader of SPEC-27 will not know whether `BigUint` is `num_bigint::BigUint`, `num::BigUint`, or a custom struct; will not know if "Horner's method" matches the convention in R11' or the (different) convention used in the design doc; and will not know whether "Codec" is a singleton trait object or a generic type alias.
**Suggested resolution:** Open a follow-up TASK to add the following to SPEC-00 §8b (or a new §8c):
- `Codec`, `Encoder` (as trait), `Decoder` (as trait) — pointing to SPEC-27 R1/R2/R3.
- `HornerCodec`, `ChurchArithmeticCodec` — pointing to SPEC-27 §3.3 / §3.4.
- `Horner's method` — with the exact recurrence, the coefficient-ordering convention from R11', and a back-reference to R13'.
- `BigUint readback` — with `num_bigint::BigUint` named explicitly, plus the bit-length semantics (`BigUint::bits()`).
- A note clarifying that "Normal Form" in `DecodeError::NotNormalForm` is the operational definition (`net.redex_queue.is_empty()`), tied to SPEC-00 5.5.
Until SPEC-00 is amended, SPEC-27 §2 should be tagged as the temporary canonical source for these terms (the spec already uses a definitions table — extending it is cheap).

---

### SC-002: R12' bound-inheritance citation is misleading

**Severity:** HIGH
**Axis:** Consistency
**Section:** §3.4
**Requirement:** R12'
**Problem:** R12' states that the bounds `coeffs[i] <= 10_000` and `x <= 10_000` are "inherited from SPEC-14 R4". SPEC-14 R4 (and R4b) constrains exactly one input: the parameter `n` of `encode_nat(n: u64) -> Net` and `encode_church_into(net: &mut Net, n: u64) -> AgentId`. There is no SPEC-14 requirement that constrains "coefficients" or "x" — there are no such terms in SPEC-14. The chain that R12' is implicitly relying on is:

> Each `coeffs[i]` and `x` are passed to `encode_church_into` (SPEC-14 R4b). `encode_church_into` panics if `n > 10_000` (SPEC-14 R4 + R4b). Therefore HornerCodec MUST validate `coeffs[i] <= 10_000` and `x <= 10_000` *before* calling `encode_church_into` to avoid a panic and instead return `EncodeError::InvalidInput`.

The current R12' wording elides the chain. A reader who has not internalized SPEC-14 will not know that the cap on `x` and the cap on `coeffs[i]` are the *same* cap, applied at the *same* call site (`encode_church_into`), and that violating it triggers a *panic* in the underlying primitive (which the encoder must trap before it happens).
**Impact if unresolved:** A QA agent could reasonably read R12' as "x has its own cap, separate from coefficient caps", and write tests that assume the encoder gracefully handles `x = 12_000` by some other path (e.g., wrapping at `u64::MAX`). The actual behavior is "panic in `encode_church_into`", which the spec considers a bug. Worse: a future change to SPEC-14 R4 (e.g., raising the cap to 100_000) would silently change the semantics of HornerCodec without R12' being updated, because R12' hard-codes `10_000` rather than re-citing SPEC-14 R4 by reference.
**Suggested resolution:** Rewrite R12' as:

> R12'. The encoder MUST reject inputs where any `coeffs[i]` or `x` exceeds the SPEC-14 R4 cap on `encode_church_into(net, n)` (currently `n <= 10_000`). Specifically, the encoder MUST validate `coeffs.len() >= 1`, and for each value `v` in `coeffs ∪ {x}`, that `v <= 10_000`. Violations MUST return `EncodeError::InvalidInput("...")` *before* any call to `encode_church_into`, since exceeding the cap triggers a panic in the underlying primitive (SPEC-14 R4). If SPEC-14 R4 changes its cap, this requirement automatically inherits the new value via the citation chain. **(MUST)**

This makes the panic-trapping responsibility explicit and the inheritance dynamic.

---

### SC-003: `compute_arithmetic` and `sum_of_squares` are dangling references in SPEC-14

**Severity:** HIGH
**Axis:** Consistency
**Section:** §2 (ChurchArithmeticCodec definition), §3.3 (R7-R9), R8 input schema, T3, T4, R19
**Requirement:** R7, R8, R19
**Problem:** §2 defines `ChurchArithmeticCodec` as wrapping "SPEC-14's existing `compute_arithmetic` pipeline (`add`, `mul`, `exp`, `sum_of_squares`)". Three problems:
1. **`compute_arithmetic` is not a SPEC-14 export.** SPEC-14 R3 lists the encoding module's public surface as `encode_nat, decode_nat, build_add, build_mul, build_exp`. There is no `compute_arithmetic`. The closure log (§2.3) repeats this attribution but it is not anchored in SPEC-14 itself. SPEC-26 R12 mentions a Tauri command named `compute_arithmetic`, which is what this is actually wrapping — but that's a future GUI command, not a SPEC-14 primitive. SPEC-07 R1 lists a `compute` CLI subcommand (defined in SPEC-14 R22-R25). The actual function the codec is wrapping is the run logic of the `compute` subcommand, which is described in SPEC-14 §3.6 prose but never named `compute_arithmetic`.
2. **`sum_of_squares` is not a SPEC-14 primitive.** SPEC-14 exposes `build_add`, `build_mul`, `build_exp` only (R15-R17). There is no `build_sum_of_squares` in SPEC-14. SPEC-09 R17d defines a `church_sum_of_squares` benchmark and §SPEC-09:1170 puts it in a separate `church_sum_of_squares.rs` file in the bench module. The `sum_of_squares` op in SPEC-27 R8 must therefore either (a) introduce a new SPEC-14 primitive (which the spec does not declare) or (b) be implemented inside the codec by composing `build_add`/`build_mul` over `1²...n²` (which the spec does not state either). The exp op also needs clarification: SPEC-14 R17 takes `(base, exp)`, but R8's schema has `a, b` — which is which?
3. **R8's input schema accepts a single op but R7 says "no public API change".** R8 introduces a new JSON schema. R9 says "All 690 existing tests MUST pass" — but the existing `compute` subcommand in SPEC-14 R22 takes positional `a b` args of type `u64`, not a JSON `{op, a, b}`. The codec must therefore add a *new* schema layer on top of SPEC-14's existing API. This is fine — but R7's wording "MUST NOT change any existing public API signatures — only add trait implementations" is misleading because the codec's own input schema (R8) IS a new public API surface (the registry-dispatched JSON path), even though SPEC-14's `build_*` functions are unchanged.
**Impact if unresolved:** The developer will look for `compute_arithmetic` in SPEC-14, fail to find it, and make a discretionary decision about where to place the wrapping logic. The QA agent will look for `build_sum_of_squares` and either invent it or open a bug. The reviewer will challenge R7's "no API change" claim. All three are avoidable.
**Suggested resolution:**
1. Replace "wraps SPEC-14's existing `compute_arithmetic` pipeline" with "wraps the existing `compute` CLI subcommand pipeline (SPEC-14 §3.6 R22-R25), which dispatches over `build_add` / `build_mul` / `build_exp` (SPEC-14 R15-R17)".
2. For `sum_of_squares`: either (a) explicitly state that ChurchArithmeticCodec implements `sum_of_squares` *internally* by folding `build_add` over `build_mul(i, i)` for `i in 1..=a` (the size parameter), and that this composition lives inside the codec, NOT in SPEC-14; or (b) drop `sum_of_squares` from R8/R19/T3 and document it as a future codec, since SPEC-09 R17d already provides it as a benchmark. Option (a) is cheaper because SPEC-09 already has the build logic.
3. For the exp operand naming: R8 should add a sentence "for `op = exp`, `a` is the base and `b` is the exponent (matches SPEC-14 R17 ordering)".
4. Soften R7 from "MUST NOT change any existing public API" to "MUST NOT change any existing SPEC-14 public function signatures (R3 export list); the codec adds a new JSON-dispatch surface (R8) on top of the existing primitives."

---

### SC-004: R14' "BigUint readback" lacks an algorithm specification

**Severity:** HIGH
**Axis:** Completeness
**Section:** §3.4
**Requirement:** R14', R16b'
**Problem:** R14' says "Traverse the Church numeral structure rooted at `net.root()`, accumulating the application count in `num_bigint::BigUint` (NOT `u64`)". This sentence is the *entire* algorithm specification. SPEC-14's `decode_nat` has a 90-line pseudocode block (SPEC-14 §4.4) that walks through the lambda-x → app-chain → era-detection cases. R14' offers nothing equivalent. The spec also doesn't say:
- What happens for the n=0 case (Church zero — does the BigUint readback return `BigUint::zero()`? It should, but the spec doesn't say so).
- How to detect a "non-Church-numeral" structure (the existing `decode_nat` returns `Option::None`; what's the BigUint equivalent — `Result<BigUint, DecodeError::UnrecognizedStructure>`?)
- Whether the BigUint readback is implemented as a wrapper over `decode_nat` (which caps at `u64::MAX`) plus an extension for larger values, or as a parallel implementation. R16b' implies they are *separate* implementations cross-checked by property test, but R14' does not say so.
- What the application-counting termination condition is (for `decode_nat` it's "current == lambda_x.p1"; the BigUint version presumably uses the same condition, but it's not written).
**Impact if unresolved:** Two developers implementing R14' independently will produce two different readback functions. One may reuse `decode_nat`'s loop and switch the accumulator type from `u64` to `BigUint`; another may reimplement from scratch and miss the n=0 branch. The R16b' property test will pass for the first but fail for the second on Church(0).
**Suggested resolution:** Add a §3.4 sub-block "R14' algorithm (informative)" that either:
- (a) References SPEC-14 §4.4's pseudocode and says "replace `count: u64` with `count: BigUint`, return `Ok(count)` on chain-end, return `Err(DecodeError::UnrecognizedStructure(...))` on unexpected topology, return `Ok(BigUint::zero())` on the zero case (mirroring `decode_nat`'s `Some(0)`)"; OR
- (b) Inlines a 20-line pseudocode with the same structure as SPEC-14 §4.4.
Whichever option is picked, the spec must state explicitly whether `decode_biguint` shares code with `decode_nat` or runs in parallel (R16b' is a meaningful test only if the two paths are independent — otherwise the cross-check is tautological).

---

### SC-005: `DecodeError::NotNormalForm { redexes: usize }` is over-specified for a `&Net`

**Severity:** MEDIUM
**Axis:** Testability
**Section:** §3.1
**Requirement:** R4
**Problem:** The error variant carries `redexes: usize`, presumably the count of pending active pairs. SPEC-14 R13 says `decode_nat` "MUST NOT modify the input net" and operates on `&Net`. The redex queue is a field of `Net` (per SPEC-02), so reading `net.redex_queue.len()` is fine — but the spec doesn't say which counting semantics apply. Two reasonable readings:
- (a) `redexes = net.redex_queue.len()` — the count of queued redexes (which by I4 may include stale entries).
- (b) `redexes = number of valid active pairs in the net` — the count after stale-entry pruning.
These differ. SPEC-14 §4.4 lines 644-646 use (a) (`if !net.redex_queue.is_empty()`), so consistency would suggest (a). But the variant name `NotNormalForm` implies an actual property of the net (b), not a queue artifact. A worker that ran `reduce_all` and stopped early due to a step budget could have a non-empty queue with stale entries — the net itself might be in Normal Form.
**Impact if unresolved:** Test T9 ("BigUint range") will check the decoded value, but if the spec adopts reading (a), then any test that runs the full distributed pipeline (T13) where the merged net's queue contains stale entries from the merge phase (per SPEC-05) could trip a false `NotNormalForm` error. Reading (b) is correct but more expensive.
**Suggested resolution:** Add to R4:
> The `redexes` field of `NotNormalForm` MUST report `net.redex_queue.len()` after stale-entry pruning per SPEC-01 I4 (or equivalently, the count of valid active pairs detected by the standard redex-detection function). The decoder MUST NOT trigger this error solely on a non-empty queue if all queued entries are stale.
And update R14' to invoke "valid redex count" rather than "zero redexes", consistent with the I4 stale-tolerance design note.

---

### SC-006: T7 expected-value derivation rule is correct but T9 is not testable as written

**Severity:** MEDIUM
**Axis:** Testability
**Section:** §7.3
**Requirement:** Test T9
**Problem:** T9 says: "Input `{"coeffs":[1,1,1,1,...,1],"x":10}` with `coeffs.len() == 20` → result is approximately 10^19, requiring BigUint readback (R14'). MUST cross-match `horner_serial` exactly." Two issues:
1. **The cap `coeffs[i] <= 10_000` and the cap `x <= 10_000` are both fine here**, but the closure log (§4.4) says R14' is exercised by T9 because the result exceeds `u64::MAX`. The actual value of `sum(10^i for i in 0..20) = (10^20 - 1)/9 ≈ 1.11 × 10^19`. `u64::MAX = 1.844 × 10^19`. So the result *does not* exceed `u64::MAX` — it sits inside it. The test as designed therefore does NOT exercise BigUint range.
2. To genuinely exceed `u64::MAX`, the test needs `coeffs.len() >= 21` (sum_{i=0}^{20} 10^i ≈ 1.11 × 10^20, which exceeds `u64::MAX`). Or, more aggressively, use a polynomial like `coeffs = [10000, 10000, 10000, 10000, 10000]` with `x = 10000` → result ≈ 10^20 + ..., exceeding u64.
**Impact if unresolved:** T9 will pass with a u64 accumulator; the developer will not know that R14' wasn't actually exercised; the property test T11 may still catch the issue but only if the prop-test happens to sample large inputs. R14' is therefore under-tested.
**Suggested resolution:** Either:
- (a) Bump T9 to `coeffs.len() == 25` (or larger) to guarantee `result > u64::MAX`. Document the expected value as "computed via `horner_serial`" per the same rule as T7.
- (b) Add an explicit T9b: "Input `{"coeffs":[10000, 10000, 10000, 10000, 10000], "x":10000}` MUST produce a `BigUint` exceeding `u64::MAX`; the test MUST verify `bit_length > 64` and exact equality to `horner_serial`."
Option (b) is preferable because it exercises both the boundary value `10_000` (R16') and the BigUint range (R14') in a single deterministic test.

---

### SC-007: `horner_serial` (R16a') signature is incomplete

**Severity:** MEDIUM
**Axis:** Completeness
**Section:** §3.4
**Requirement:** R16a'
**Problem:** The signature is `pub fn horner_serial(coeffs: &[u64], x: u64) -> num_bigint::BigUint;`. Three gaps:
1. **What does it return for `coeffs.len() == 0`?** R16' specifies `EncodeError::InvalidInput("empty coeffs")` for the encoder, but `horner_serial` is described as "a pure-Rust oracle". An oracle that panics on empty input is fine, but one that silently returns `BigUint::zero()` is dangerous (the property test could cross-check `decode == horner_serial` for `coeffs == []` and pass spuriously by both returning 0).
2. **What about coefficient overflow?** The encoder rejects `coeffs[i] > 10_000`, but `horner_serial` takes `&[u64]` — the developer might naively accept `coeffs[i] > 10_000` and produce a value the IC pipeline cannot reproduce. The property test (T11) could then fail in an opaque way.
3. **Is the function panic-free or does it propagate via `Result`?** A pure oracle should match the encoder's contract (rejecting invalid input the same way), or it should panic deterministically. Either is fine; ambiguity is not.
**Impact if unresolved:** The property test (T11) and the cross-check in T7 both depend on `horner_serial` agreeing with the IC pipeline. If `horner_serial` accepts inputs the IC pipeline rejects, T11 will produce confusing failures. If `horner_serial` panics on inputs the IC pipeline rejects gracefully, T10's negative cases will not be cross-checkable.
**Suggested resolution:** Tighten R16a' to:

> R16a'. A pure-Rust oracle MUST be exposed:
> ```rust
> pub fn horner_serial(coeffs: &[u64], x: u64) -> Result<num_bigint::BigUint, &'static str>;
> ```
> The oracle MUST enforce the same input bounds as the encoder (R12'): `coeffs.len() >= 1`, `coeffs[i] <= 10_000`, `x <= 10_000`. Violations return `Err(...)`. Valid inputs return `Ok(value)` where `value = sum(coeffs[i] * x^i for i in 0..coeffs.len())` computed in `BigUint`. Property tests (T11) MUST sample inputs from the valid range and compare `horner_serial(c, x).unwrap() == decode(reduce_all(encode((c, x)))).unwrap()`.

If a `&'static str` error type is unwelcome for the oracle, use `EncodeError` itself for symmetry.

---

### SC-008: Glossary alias `--codec`/`--encoder` mutual exclusion violates SPEC-07's flag pattern

**Severity:** MEDIUM
**Axis:** Consistency
**Section:** §3.6
**Requirement:** R21
**Problem:** R21 mandates that `--encoder` and `--codec` are clap aliases of each other AND that passing both simultaneously triggers a clap conflict error. In clap's data model, `aliases(...)` makes two flag names refer to a single argument; specifying the same argument twice (regardless of which alias is used) is *not* by default a conflict — clap silently keeps the last value. The spec is asking for two distinct argument bindings with `conflicts_with`, which is a different clap feature than `aliases`. The closure log (§3.1) confirms the editorial decision but says "the implementation cost in clap is one `aliases` macro line" — which is incorrect. Implementing the conflict guard requires:
- Two separate `#[arg]` definitions, one for `--encoder`, one for `--codec`.
- `conflicts_with("encoder")` on the codec arg (and vice versa).
- A post-parse merge step that picks whichever was provided.
This is functionally equivalent to alias-with-conflict, but it is *not* a clap alias.
SPEC-07 R3-R10 do not document any precedent for "alias-with-conflict" — every flag in SPEC-07 has a single canonical name. SPEC-27 is therefore introducing a new CLI pattern.
**Impact if unresolved:** A developer will read R21, write `#[arg(long, alias = "codec")]`, and ship it. The mutual-exclusion check will silently not fire. Test T20 ("Mutually exclusive flags") will fail because clap accepts both. The developer will then have to refactor to two-args-with-conflicts-with, which changes the help-output structure and breaks T19's "identical output" expectation.
**Suggested resolution:** Update R21 to specify the implementation pattern explicitly:

> R21. The `compute` subcommand MUST accept the codec name via either `--encoder <name>` or `--codec <name>`. These MUST be implemented as two separate clap arguments with `conflicts_with` cross-references (NOT a single argument with an alias), so that passing both forms simultaneously yields a clap conflict error. After parsing, exactly one of the two MUST be `Some(...)` if the codec is selected; the application logic MUST coalesce them. The help output MUST list both flags. **(MUST)**

Also recommend documenting this pattern in SPEC-07 as a "dual-form flag" CLI pattern (separate task), since R21 is the first instance.

---

### SC-009: R13' empirical-statement claim is stronger than ARG-001 P3 supports

**Severity:** MEDIUM
**Axis:** Invariant Preservation
**Section:** §3.4
**Requirement:** R13'
**Problem:** R13' says: "The resulting net... MUST produce a Church numeral whose decoded value equals `p(x) = sum(coeffs[i] * x^i for i in 0..=n)`." Then below: "This requirement is the empirical statement of ARG-001 P3: the Normal Form is invariant under reduction order, so any worker count W >= 1 and any BSP schedule MUST produce the same polynomial value." The strong claim is correct as a *consequence* of ARG-001 (P1+P2+P3+P4+P6), but ARG-001 P3 specifically is "Completude da resolucao de fronteira" (border redex completeness). The invariance-under-reduction-order property is captured by P1 (strong confluence, Lafont's theorem) + T6/T7 (uniqueness of Normal Form for terminating nets) + D6 (protocol termination). P3 is a *necessary condition* but not the property the spec is illustrating.

The closure log (§4.3) repeats the same conflation. The Topic 2 design doc and the Horner explainer doc would need cross-checking, but on the face of SPEC-01 + ARG-001, the correct citation for "NF is invariant under reduction order" is **G1 (Fundamental Property)** with **P1 (T4 strong confluence)** as the engine and **P3 (D3 border redex completeness)** as a precondition. Not P3 alone.
**Impact if unresolved:** A reader who actually reads ARG-001 will notice that the "empirical illustration of P3" claim doesn't match P3's text. If this spec is cited in the TCC paper alongside ARG-001, the inconsistency surfaces during defense. A QA agent reading R13' might write a test that exercises P3 specifically (border redex resolution) rather than the full G1 (end-to-end equivalence), and the test would not actually demonstrate what R13' claims.
**Suggested resolution:** Rewrite the second paragraph of R13' as:

> This requirement is the v1 empirical illustration of ARG-001's central thesis (G1, the Fundamental Property): for any terminating net, sequential `reduce_all` and distributed `run_grid` produce isomorphic Normal Forms. P1 (strong confluence, Lafont's theorem) guarantees that the value is invariant under reduction order; P3 (border redex completeness) and P4 (ID consistency) are the distribution-side preconditions that make the property hold under arbitrary worker count W and arbitrary BSP schedule. Test T13 specifically targets G1.

This preserves the empirical-illustration framing while grounding the claim in the right premise.

---

### SC-010: T13 distributed equivalence test does not specify decoder-stage protocol

**Severity:** MEDIUM
**Axis:** Testability
**Section:** §7.3
**Requirement:** Test T13
**Problem:** T13 says: "For each of T6-T9 inputs, run the IC pipeline under sequential reduction (`reduce_all`) and under distributed reduction with W ∈ {2, 4, 8} workers (in-process and Docker TCP). All paths MUST yield the same decoded value." Three gaps:
1. **Where is the decode performed?** NG5 says "Decoding always happens on the coordinator after merge". T13 does not say which net is fed to `decode`. Sequential path: `decode(reduce_all(encode(...)))`. Distributed path: presumably `decode(extract_result(run_grid(encode(...), W)))`, but the spec doesn't say this.
2. **In-process vs Docker TCP** is mentioned in passing but the test plan does not specify whether both transports MUST be exercised, MAY be exercised, or whether the in-process path is sufficient for T13's claim. Docker TCP testing is much heavier and may not be part of `cargo test`.
3. **`run_grid` requires a partitioning**, but HornerCodec is not a `RecipeEncoder` (Q4). The coordinator must do centralized partitioning (R25 fallback). T13 does not say which partitioning algorithm — SPEC-04's default? A specific worker count strategy?
**Impact if unresolved:** Two developers will implement T13 differently. One will skip the Docker TCP variant (calling it integration-test scope); another will write it as a `#[test]` with `#[ignore]`. Reviewer/QA disagreement is guaranteed.
**Suggested resolution:** Tighten T13 to:

> T13. For inputs T6, T7, T8, T9 (and any other Horner test that produces a non-trivial NF):
> - Compute `seq_value = decode(reduce_all(encode(input)))`.
> - For each `W ∈ {2, 4, 8}`:
>   - In-process: compute `inproc_value = decode(extract_result(run_grid(encode(input), W, default_partition_strategy)))`. **(MUST)**
>   - Docker TCP: compute `tcp_value` via the equivalent end-to-end pipeline. **(SHOULD; MAY be `#[ignore]` for `cargo test`, MUST run in CI integration suite.)**
> - Assert `seq_value == inproc_value == tcp_value` (when present).
>
> This test is the empirical demonstration of G1 for HornerCodec (cf. R13').

Also add a sentence to NG5: "T13 specifies decoding on the coordinator's merged net, never on per-worker partitions."

---

### SC-011: §5 Future Work LambdaCodec sketch reuses SPEC-14 R-numbers ambiguously

**Severity:** LOW
**Axis:** Completeness
**Section:** §5.1
**Requirement:** §5.1 (informative)
**Problem:** The LambdaCodec sketch enumerates "Mackie/Pinto encode pipeline (REF-005 Section 5 mapping):" with five bullets describing the lambda → IC mapping. The bullets are not labeled as "future R-numbers" — they're informative — but a careless reader could misread them as the v2 LambdaCodec spec being already drafted. The closing sentence says "Future implementation work (Roadmap candidate D-NN+)" with a placeholder `D-NN+` that is never resolved.
**Impact if unresolved:** Cosmetic but contributes to spec ambiguity over what is decided vs. deferred.
**Suggested resolution:** Add a one-line preamble to §5.1:

> The bullets below are informative sketches of the future LambdaCodec design, NOT v2 normative requirements. Normative requirements MUST be authored in a separate spec (e.g., SPEC-28 or a successor) when LambdaCodec is admitted into the v2 scope.

Also resolve the `D-NN+` placeholder to "TBD" or a real planned slot.

---

### SC-012: Phase table LoC budget for HornerCodec is unaudited

**Severity:** LOW
**Axis:** Completeness
**Section:** §6
**Requirement:** Implementation Phases table
**Problem:** Phase 3 (HornerCodec) lists ~250 LoC, the same envelope as the v1 LambdaCodec. The closure log (§2.10) notes that the budget was "kept (~250 LoC, the same envelope)", but HornerCodec's surface (encoder + BigUint readback + oracle + property tests + edge-case enumeration of R16') is materially smaller than a Lambda parser + Mackie/Pinto encode pipeline + readback. Conversely, R16b' adds `relativist-core::encoding::biguint_readback` as a separate module, not counted in any phase.
**Impact if unresolved:** Cosmetic. The table is "indicative for the SDD task-splitter" (its own footnote).
**Suggested resolution:** Either (a) audit the budget downward (HornerCodec encoder + decoder + oracle + biguint_readback ≈ 150-200 LoC; tests ≈ 100-150 LoC), or (b) split Phase 3 into 3a (HornerCodec encoder/decoder, ~150 LoC), 3b (`biguint_readback` shared helper, ~50 LoC), 3c (oracle + tests, ~100-150 LoC). The split version is more useful for the task-splitter's atomicity rule (each TASK <200 LoC).

---

### SC-013: R5 (encode contract) has no explicit invariant T1-T7 mapping for HornerCodec

**Severity:** MEDIUM
**Axis:** Invariant Preservation
**Section:** §3.2
**Requirement:** R5, R10', R13'
**Problem:** R5 mandates that "Every `Encoder::encode()` output MUST be validated before reduction. Validation checks: E1. Net satisfies T1-T7 from SPEC-01; E2. Net has at least one redex." The validation is centralized in the registry (R5 → R18 `encode_and_validate`). For HornerCodec specifically, the spec does not mandate that the construction sequence in R13' (compose `build_add` and `build_mul`) preserves T1-T7 *during construction* (i.e., before validation runs). SPEC-14 R8 says all `encode_nat` nets satisfy T1-T7, and SPEC-14 R20 / §4.3.1 §4.3.2 imply the same for `build_add`, `build_mul` outputs — but the *composition* of two `build_*` outputs into a Horner accumulator chain is novel. Each `encode_church_into` call adds a Church sub-net to a shared `Net`, and the composition uses `build_add(net, acc, coef_node)` — but SPEC-14 R15 defines `build_add(a: u64, b: u64) -> Net`, not `build_add(net: &mut Net, ...) -> AgentId`. There is no SPEC-14 primitive named `build_add(net: &mut Net, acc: AgentId, x_node: AgentId)` with the signature R13' is implicitly using.
**Impact if unresolved:** R13' pseudocode invokes `build_add(net, acc, x_node)`, `build_mul(net, acc, x_node)`, and `coef_node <- encode_church_into(net, coeffs[k])`. Of these, only `encode_church_into` is a real SPEC-14 export (R4b). `build_add(net, acc, x_node)` is NOT in SPEC-14 — SPEC-14 R15 takes `(u64, u64)` and creates a fresh net. The composable variants the pseudocode needs do not exist. The HornerCodec implementer will have to *introduce* `build_add_into(net: &mut Net, m: AgentId, n: AgentId) -> AgentId` and `build_mul_into(...)` as new primitives — this is implicit in R10' but never stated.
**Suggested resolution:** Add a new R13'a:

> R13'a. SPEC-14's `build_add` and `build_mul` (R15-R16) currently expose only `(u64, u64) -> Net` signatures. HornerCodec requires composable variants `build_add_into(net: &mut Net, m: AgentId, n: AgentId) -> AgentId` and `build_mul_into(net: &mut Net, m: AgentId, n: AgentId) -> AgentId` that operate on an existing net (mirroring `encode_church_into`, SPEC-14 R4b). These variants MUST preserve T1-T7 by the same construction argument as R4b. They MAY be added either as private helpers in SPEC-14's `arithmetic.rs` or as private helpers in HornerCodec's `horner.rs`. The SPEC-14 `build_add` / `build_mul` public exports (R15-R16) MUST remain unchanged in signature. (SPEC-27 v2 does not amend SPEC-14; the new helpers are scoped to whichever module owns them at implementation time.)

Or, more aggressively, open a separate task to amend SPEC-14 R15/R16 with the `_into` variants explicitly (preferred for consistency with R4b). Either way, the spec must close this gap before implementation, because R13''s pseudocode is currently un-implementable as stated.

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 0 |
| HIGH     | 4 |
| MEDIUM   | 7 |
| LOW      | 2 |
| **Total** | **13** |

## Mandatory (must fix before implementation)

- SC-001: SPEC-00 glossary gap — six new terms introduced inline
- SC-002: R12' bound-inheritance citation is misleading
- SC-003: `compute_arithmetic` and `sum_of_squares` are dangling references in SPEC-14
- SC-004: R14' "BigUint readback" lacks an algorithm specification
- SC-013: R5 (encode contract) has no explicit invariant T1-T7 mapping for HornerCodec (and R13' invokes non-existent SPEC-14 `_into` variants)

## Recommended (should fix)

- SC-005: `DecodeError::NotNormalForm { redexes: usize }` is over-specified for a `&Net`
- SC-006: T9 BigUint test does not actually exceed `u64::MAX`
- SC-007: `horner_serial` (R16a') signature is incomplete
- SC-008: `--codec`/`--encoder` mutual exclusion implementation is misstated as a clap alias
- SC-009: R13' empirical-statement claim conflates P3 with G1
- SC-010: T13 distributed equivalence test does not specify decoder-stage protocol
- SC-011: §5 LambdaCodec sketch could be misread as v2 normative
- SC-012: Phase 3 LoC budget unaudited

---

## Checklist

### Consistency
- [x] All terms match SPEC-00 definitions — **FAILED** (SC-001: 6 missing entries)
- [x] Type signatures compatible with predecessor specs — **PARTIAL** (SC-003, SC-013: `compute_arithmetic`/`build_sum_of_squares`/`build_add_into`/`build_mul_into` not in SPEC-14)
- [x] No contradictions with predecessor requirements — **PARTIAL** (SC-002: bound citation chain elided)
- [x] Data flow assumptions match predecessor outputs — **OK with caveats** (SC-013)

### Testability
- [x] Every MUST requirement has a testable criterion — **MOSTLY OK** (SC-005 redex-count semantics; SC-006 T9 bug)
- [x] Boundary conditions defined (0, 1, MAX) — **OK** (R16' enumerates them)
- [x] Error conditions specified — **OK** (R4 + R12'); `horner_serial` error model gap (SC-007)

### Completeness
- [x] Pseudocode provided for non-trivial operations — **FAILED for R14'** (SC-004)
- [x] All edge cases documented — **OK in R16'** (SC-006 affects T9 specifically)
- [x] Rust type signatures for all public types/functions — **PARTIAL** (SC-007 `horner_serial`; SC-013 `_into` variants)
- [x] No undefined terms or dangling references — **FAILED** (SC-001, SC-003)

### Invariant Preservation
- [x] T1-T7 maintained by all operations — **NEEDS R13'a** (SC-013)
- [x] D1-D6 maintained by all operations — **OK by construction** (HornerCodec rides on existing pipeline; D5 confirmed by Q4 + R25 fallback for non-RecipeEncoder)
- [x] I1-I5 maintained by all operations — **OK** (Church construction in SPEC-14 already preserves I1-I5; HornerCodec composes only via SPEC-14 primitives + new `_into` helpers)
- [x] G1 not violatable by any valid operation sequence — **OK with citation fix** (SC-009)

---

## Notes for Round 2 (ESPECIALISTA EM SPECS)

The five MANDATORY items are clustered in two themes: (a) glossary/citation hygiene (SC-001, SC-002, SC-003) — these are mostly textual fixes plus one follow-up SPEC-00 task; (b) implementation enablement (SC-004, SC-013) — R14' needs an algorithm sketch and R13' needs the `_into` helpers acknowledged. None of these block the empirical-validation goal of HornerCodec; they protect downstream stages (task-splitter, developer, QA) from divergent interpretations.

The seven RECOMMENDED items are quality improvements. SC-008 in particular is worth fixing pre-implementation because it changes the clap argument structure (alias vs conflicts_with), which the task-splitter needs to know to size the CLI integration TASK correctly.

The two LOW items are cosmetic.

If all MANDATORY items are resolved in Round 2, the spec advances to Stage 1 (task-splitter). If only some are resolved, the round 3 critic should re-verify each unresolved item.
