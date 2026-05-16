# SPEC-27 v2 → v3 — Round 2 Spec-Critic Response (Closure Log)

**Date:** 2026-05-06
**Author agent:** ESPECIALISTA EM SPECS (Camada 1)
**Spec:** `codigo/relativist/specs/SPEC-27-encoder-decoder-api.md`
**Round 1 review:** `codigo/relativist/docs/spec-reviews/SPEC-27-round1-critic.md` (NEEDS REVISION; 4 HIGH + 7 MEDIUM + 2 LOW = 13 issues)
**Status produced:** Revised v3 (Round 2 spec-critic response) — addresses all 13 Round 1 issues; ready for sdd-pipeline Stage 1 (task-splitter) OR optional Round 3 critic re-verification.

---

## 1. Mandate

Round 1 produced a NEEDS REVISION verdict with 5 MANDATORY items (SC-001, SC-002, SC-003, SC-004, SC-013) and 7 RECOMMENDED items (SC-005, SC-006, SC-007, SC-008, SC-009, SC-010, SC-011) plus 1 LOW (SC-012). The user verified two issues directly against SPEC-14 prior to Round 2 and confirmed:

- **SC-013 is fully procedente.** SPEC-14 R15-R17 expose `build_add(a: u64, b: u64) -> Net`, `build_mul(a: u64, b: u64) -> Net`, `build_exp(base: u64, exp: u64) -> Net` — all create a *new* `Net` and take `u64`, NOT `AgentId` in an existing net. The Round 1 R13' pseudocode invokes `build_add(net, acc, x_node)` with a signature that does not exist in SPEC-14 R3.
- **SC-002 is partially procedente.** SPEC-14 R4 caps `encode_nat(n) at n <= 10_000` and R4b extends the cap to `encode_church_into(net, n)`. R12' should cite both R4 and R4b explicitly to make the cap-inheritance chain unambiguous.

A new architectural finding emerged during Round 2 prep: the helpers required by SC-013's resolution **already exist** in `relativist-core::encoding::arithmetic` as `wire_add_into(net, m_port, n_port) -> AgentId` and `wire_mul_into(net, m_port, n_port) -> AgentId` (`pub(crate)` privacy, PortRef-based signatures, introduced for SPEC-09 R17d `church_sum_of_squares`). This shifts SC-013's resolution from "implementer must write new helpers" to "implementer reuses existing helpers; SPEC-27 v3 declares the contract they satisfy" — a cleaner, lower-risk fix.

---

## 2. Per-issue dispositions

The disposition codes are ACCEPTED (issue accepted, fix applied to v3), DEFENDED (issue rebutted with technical argument), DEFERRED (issue acknowledged but resolution pushed to a follow-up), or PARTIAL (mixed).

### SC-001 — SPEC-00 glossary gap (HIGH, Consistency) — **ACCEPTED**

**Resolution.** §2 of SPEC-27 v3 expanded with **eleven** locally-defined entries, all tagged `(Relativist, this spec)` to mark the spec as the temporary canonical source:

- `Encoder` (trait), `Decoder` (trait), `Codec` (trait), `EncoderRegistry`, `HornerCodec`, `ChurchArithmeticCodec`, `RecipeEncoder`, `Encode Contract`, `Horner's method`, `BigUint`, `BigUint readback`, `NotNormalForm (operational)`.

`BigUint` entry names `num_bigint::BigUint` explicitly, fixes the version range (`^0.4`), the license (MIT/Apache-2.0), and the bit-length semantics (`BigUint::bits()` with worked examples for 0, 1, and `u64::MAX`). `Horner's method` carries the exact recurrence with the R11' coefficient-ordering convention pinned in the entry. `NotNormalForm (operational)` formalizes the operational refinement of SPEC-00 §5.5 used by R4. The territory rule (Camada 1) prevents this round from editing SPEC-00 directly; the explicit "Relativist, this spec" tags make clear that a future SPEC-00 amendment task should absorb these terms (out of scope of v3).

### SC-002 — R12' bound-inheritance citation misleading (HIGH, Consistency) — **ACCEPTED**

**Resolution.** R12' rewritten end-to-end. The new wording (a) explicitly cites SPEC-14 R4 *and* R4b and traces the chain `coeffs[i] / x → encode_church_into → SPEC-14 R4b cap → SPEC-14 R4 cap`; (b) makes the panic-trapping responsibility explicit ("exceeding the cap triggers a panic in `encode_church_into`, which the encoder MUST trap before it happens"); (c) makes the cap value dynamic by recommending a single shared constant `MAX_CHURCH_NAT` rather than a `10_000` literal duplicated across the codec. The implementation is forbidden from hardcoding `10_000` divorced from the SPEC-14 cap.

The Round 1 reviewer suggested HIGH; the user confirmed the issue but suggested this could be argued at MEDIUM. Decision: ACCEPTED with the full HIGH-grade rewrite, since the panic-trapping requirement is a correctness invariant the encoder MUST satisfy, not just a citation polish. Cost is minimal.

### SC-003 — `compute_arithmetic` and `sum_of_squares` dangling references (HIGH, Consistency) — **ACCEPTED (PARTIAL DEFENSE)**

**Resolution.**

1. **`compute_arithmetic` removed from §2 ChurchArithmeticCodec definition.** The entry now reads "wraps the existing `compute` CLI subcommand pipeline (SPEC-14 §3.6 R22-R25), which dispatches over `build_add` / `build_mul` / `build_exp` (SPEC-14 R15-R17) and `build_sum_of_squares` (which exists in `relativist-core::encoding::arithmetic` per SPEC-09 R17d but is not a SPEC-14-listed primitive)". The reviewer's suggested wording was adopted near-verbatim.

2. **`sum_of_squares` defended.** Per the user's hint to verify against the actual codebase, `build_sum_of_squares(n: u64) -> Net` *does* exist in `relativist-core::encoding::arithmetic` (per SPEC-09 R17d — a non-frozen demonstrative benchmark). It is implemented internally as a right-associated `wire_add_into` chain over pre-encoded `Church(i^2)` (the "pre-encoded squares" fallback), not via `build_sum_of_squares` in SPEC-14. The Round 1 reviewer reasoned this primitive *might* not exist; in fact it does. R8 v3 documents this explicitly: "`build_sum_of_squares` is *not* a SPEC-14 R3 export — it is a SPEC-09-derived helper that lives alongside `build_add`/`build_mul`/`build_exp` in the same module."

3. **R8 operand semantics clarified.** v3 R8 explicitly maps `op = "exp"` operands to SPEC-14 R17 ordering (`a` = base, `b` = exponent), and `op = "sum_of_squares"` operand semantics (`a` = `n`, `b` ignored).

4. **R7 softened.** The Round 1 reviewer's suggested wording was adopted: R7 now reads "MUST NOT change any existing **SPEC-14 R3 public function signatures**" instead of the broader "any existing public API" — the codec layer can add a new JSON-dispatch surface (R8) without violating R7.

### SC-004 — R14' BigUint readback lacks an algorithm specification (HIGH, Completeness) — **ACCEPTED**

**Resolution.** R14' v3 inlines a ~80-line normative-control-flow / informative-syntax pseudocode block that mirrors SPEC-14 §4.4 `decode_nat` topology and traversal exactly, replacing `count: u64` with `count: BigUint` and adapting the return type to `Result<BigUint, DecodeError>`. The pseudocode covers the n=0 case (returns `BigUint::from(0u64)`), the chain-walk case, all `UnrecognizedStructure` paths, and the `NotNormalForm` path with the R4 valid-redex semantics.

R14' v3 also adds an explicit "Independence from `decode_nat`" clause: `decode_biguint` MUST be a standalone implementation, NOT a wrapper over `decode_nat`, to keep R16b's cross-check meaningful. A shared `walk_church<Counter>` helper is permitted as long as both `decode_nat` and `decode_biguint` instantiate it with different `Counter` types — that satisfies the "independent code paths" intent because the helper is generic and gets monomorphized into two distinct functions.

### SC-005 — `NotNormalForm { redexes: usize }` over-specified (MEDIUM, Testability) — **ACCEPTED**

**Resolution.** R4 v3 adds a "Semantics of `NotNormalForm.redexes`" clause: the field MUST report the count of *valid* active pairs after stale-entry pruning per SPEC-01 I4, NOT `net.redex_queue.len()`. The decoder MUST NOT trigger `NotNormalForm` solely because `redex_queue.len() > 0`; it must first prune stale entries (or use the standard valid-redex detector). This protects T13's distributed pipeline from false positives caused by stale queue entries from cross-partition merges. Implementers MAY reuse the `reduce_all` validation helper.

R14' v3 pseudocode reflects the same semantics in its E1 step (`count_valid_active_pairs(net)` rather than `net.redex_queue.len()`).

### SC-006 — T9 BigUint test does not actually exceed `u64::MAX` (MEDIUM, Testability) — **ACCEPTED**

**Resolution.** T9 v3 changed from `coeffs.len() == 20` to `coeffs.len() == 25` (giving result `(10^25 - 1)/9 ≈ 1.11 × 10^24`, strictly larger than `u64::MAX = 1.844 × 10^19`). T9 MUST verify (a) `bit_length > 64` and (b) exact equality to `horner_serial(coeffs, 10).unwrap().to_string()`. Tests MUST NOT hard-code the bit count — `bit_length` is derived from `horner_serial(...).bits()`.

A new T9b was added per the reviewer's suggested resolution (b): input `[10000, 10000, 10000, 10000, 10000] @ x = 10000` exercises both the boundary value `10_000` (R16') and BigUint range (R14') in a single deterministic test.

### SC-007 — `horner_serial` (R16a') signature incomplete (MEDIUM, Completeness) — **ACCEPTED**

**Resolution.** R16a' v3 changes the signature from `pub fn horner_serial(coeffs: &[u64], x: u64) -> BigUint` to `pub fn horner_serial(coeffs: &[u64], x: u64) -> Result<BigUint, OracleError>` with a new `OracleError` enum covering `EmptyCoeffs`, `CoefficientOverflow { idx, value, max }`, `XOverflow { value, max }`. The oracle MUST enforce the same input bounds as the encoder (R12'). A new "negative cross-check" obligation was added: T11 must also assert that the oracle and the codec produce **the same error family** on the same out-of-range input (at least 30 cases), preventing "oracle silently accepts what codec rejects" failure modes.

### SC-008 — `--codec`/`--encoder` mutual exclusion misstated as clap alias (MEDIUM, Consistency) — **ACCEPTED**

**Resolution.** The Round 1 reviewer's diagnosis was correct: clap `aliases(...)` does NOT produce mutual-exclusion errors when both spellings are passed (it silently keeps the last value). The v2 closure log §3.1 incorrectly claimed "the implementation cost in clap is one `aliases` macro line", which would have produced incorrect runtime behavior.

R21 v3 reformulated with explicit `#[arg(long = "encoder", conflicts_with = "codec")]` / `#[arg(long = "codec", conflicts_with = "encoder")]` pattern. Both flags appear separately in the help output; the application logic coalesces the two `Option<String>` fields. T20 v3 explicitly tests the `conflicts_with` mechanics (the error message MUST mention both flag names; the exit code MUST be `ErrorKind::ArgumentConflict`).

A trailing "Pattern note for SPEC-07" recommends a separate task to register "dual-form flag with `conflicts_with`" as a project-wide convention. SPEC-27 v3 does NOT amend SPEC-07 (territory rule).

### SC-009 — R13' empirical-statement claim conflates P3 with G1 (MEDIUM, Invariant Preservation) — **ACCEPTED**

**Resolution.** Confirmed by reading ARG-001: P3 = "Completude da resolucao de fronteira" (border redex completeness). The "NF invariant under reduction order" property is captured by **G1 (the Fundamental Property, the central thesis)** with **P1 (strong confluence, Lafont's Proposition 1)** as the engine and **P3 + P4** as the distribution-side preconditions that lift P1's local guarantee to BSP grids.

R13' v3 rationale rewritten to cite "ARG-001's central thesis (G1, the Fundamental Property)" with P1 named as the engine and P3+P4 as preconditions. The header front-matter `Arguments consumed:` row was also updated. T13 v3 retargeted to G1 explicitly ("This test is the empirical demonstration of ARG-001 G1 for HornerCodec"; G1 specialized to decoded-value equality, which is a strictly weaker but TCC-relevant projection of structural isomorphism).

### SC-010 — T13 distributed equivalence test does not specify decoder-stage protocol (MEDIUM, Testability) — **ACCEPTED**

**Resolution.** T13 v3 fully rewritten. (a) Decoder stage explicit: `decode(extract_result(run_grid(...)))`. (b) In-process MUST for `cargo test`; Docker TCP SHOULD with `#[ignore]` permitted, MUST run in CI integration suite (cicd agent follow-up). (c) Partitioning specified: SPEC-04 default round-robin (per SPEC-07 R3 default `--strategy round-robin`), via R25 fallback (HornerCodec is not a RecipeEncoder per Q4). (d) Decoding location explicit: coordinator's merged net (NG5).

### SC-011 — §5.1 LambdaCodec sketch reuses R-numbers ambiguously (LOW, Completeness) — **ACCEPTED**

**Resolution.** §5.1 v3 prefixed with an "Informative scope note" block: "The bullets in §5.1 are informative sketches of a future LambdaCodec design — they are NOT v2 normative requirements and MUST NOT be read as such." The `D-NN+` placeholder was resolved to "slot TBD — to be assigned by sdd-pipeline once a follow-on D-cycle is opened for codec extensions" (rather than "TBD" alone, which leaves the assignment authority unclear).

### SC-012 — Phase 3 LoC budget unaudited (LOW, Completeness) — **ACCEPTED**

**Resolution.** Phase 3 v3 split into 3a/3b/3c per the reviewer's suggested resolution (b):

- 3a: HornerCodec encoder + composable `wire_*_into` helpers (~150 LoC, depends on Phases 1-2)
- 3b: BigUint readback module (~80 LoC, depends on 3a)
- 3c: Oracle + Horner tests (~120 LoC, depends on 3a + 3b)

Total Phase 3 stays in the same envelope but each sub-phase is now individually below the SDD <200 LoC TASK atomicity rule. The footnote on the table explicitly cites this rationale.

### SC-013 — R5 / R13' invokes non-existent SPEC-14 `_into` variants (MEDIUM in Round 1, CRITICAL in user's view, Invariant Preservation) — **ACCEPTED (Caminho A)**

**Resolution.** This was the most consequential issue. Three resolution paths were considered:

- **Caminho A (chosen).** Add a new R13a' to SPEC-27 v3 that specifies the composable helpers (`wire_add_into`, `wire_mul_into`) with PortRef-based signatures, declared to live as `pub(crate)` in `relativist-core::encoding::arithmetic` (alongside SPEC-14's `build_add` / `build_mul`). SPEC-14 is NOT amended; helpers stay private to the crate. HornerCodec, residing in the same crate (`relativist-core::encoding::horner`), can call them directly. The obligation set (T1-T7 preservation, reduction equivalence, privacy) is explicit in R13a'.
- **Caminho B (rejected).** Reuse only the existing `build_add(a, b) -> Net` API to produce sub-nets that get merged into a global net. **Inviable** as the user predicted: each `build_add` creates an independent `Net` with its own ID space and root, which cannot be cleanly merged into a Horner accumulator chain without re-implementing the "merge two nets into one" logic from scratch (effectively reimplementing `wire_add_into` in user space).
- **Caminho C (rejected).** Express the entire Horner expression as a single lambda built via combinator composition. Theoretically works but produces a much larger initial net, requires explicit application CON nodes for every Horner step (which `wire_add_into` already encapsulates), and significantly inflates LoC. Lower clarity, higher risk.

**Architectural finding from codebase inspection (Q5).** As of v0.20.0-pre (REF-019), `wire_add_into(net, m_port, n_port) -> AgentId` and `wire_mul_into(net, m_port, n_port) -> AgentId` already exist as `pub(crate)` helpers in `relativist-core::encoding::arithmetic`, introduced for SPEC-09 R17d `church_sum_of_squares`. Their signatures already match R13a' (PortRef-based). Phase 3a of §6 is therefore a *promotion-and-validation* task, not a *new-construction* task: the implementer reuses the existing helpers, adds direct test coverage that exercises the R13a' obligation set on synthetic inputs (separate from `build_add`/`build_mul` round-trips), and has the reviewer/QA confirm the obligations are met by inspection. This was added as Q5 in §8 Open Questions, with an explicit fallback ("if the existing helpers do NOT satisfy R13a''s obligations, the implementation MUST add new helpers under different names").

The user's recommendation (Caminho A) was followed exactly. The territoriality rule (Camada 1: SPEC-27 v3 cannot edit SPEC-14) was preserved.

R13' pseudocode v3 was rewritten to use the new R13a' API: `wire_mul_into(&mut net, acc_port, x_port) -> AgentId` followed by `wire_add_into(&mut net, prod_port, coef_port) -> AgentId`, with PortRef conversions at call sites via `PortRef::AgentPort(id, 0)`.

---

## 3. Summary of structural decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| SC-013 resolution | Caminho A — new R13a' specifying `wire_add_into` / `wire_mul_into` PortRef-based helpers | Cleanest territory split; helpers already exist in code (Q5); SPEC-14 stays untouched |
| SPEC-00 glossary debt | Local §2 entries tagged "(Relativist, this spec)" | Cannot edit SPEC-00 from this spec; tags make absorption deferral explicit |
| Clap alias mechanics | `conflicts_with` (NOT `aliases`) | Required for mutual-exclusion semantics; reviewer's diagnosis correct |
| SC-002 severity downgrade | Kept HIGH and applied full rewrite | Panic-trapping is a correctness obligation, not a polish |
| BigUint algorithm | ~80-line pseudocode block, independent of `decode_nat` | R16b' cross-check requires independence; matches SPEC-14 §4.4 topology |
| Phase 3 split | 3a / 3b / 3c | SDD <200 LoC atomicity rule; aligns to natural module boundaries |
| T9 size | 25 coefficients, value ≈ 1.11 × 10^24 | Strictly exceeds `u64::MAX`; T9 was previously trivially passing under u64 accumulator |
| T13 transport split | In-process MUST; Docker TCP SHOULD with `#[ignore]` | Keeps `cargo test` fast; CI integration suite covers TCP |

---

## 4. Coherence validation (post-fixes)

### 4.1 Internal coherence

| Item | Status | Notes |
|------|--------|-------|
| R1-R3 (traits) | OK | Untouched. |
| R4 (error types) | UPDATED | New `NotNormalForm.redexes` semantics tied to SPEC-01 I4 (SC-005). |
| R5-R6 (encode contract) | OK | Untouched (SC-013 resolved via R13a' addition, not R5 modification). |
| R7-R9 (ChurchArithmeticCodec) | UPDATED | R7 softened (SPEC-14 R3 only); R8 operand semantics clarified (SC-003). |
| R10' (module location) | UPDATED | Now references R13a' helpers. |
| R11' (input schema) | OK | Untouched. |
| R12' (input bounds) | UPDATED | Citation chain to SPEC-14 R4 + R4b explicit; cap dynamic via `MAX_CHURCH_NAT` (SC-002). |
| R13a' (composable helpers) | NEW | Closes SC-013. |
| R13' (Horner construction) | UPDATED | Pseudocode uses `wire_*_into`; rationale re-anchored to G1 + P1 + P3 + P4 (SC-009). |
| R14' (BigUint readback) | UPDATED | ~80-line algorithm pseudocode + independence clause (SC-004). |
| R15' (output schema) | UPDATED | `bit_length` semantics pinned (SC-001 BigUint entry). |
| R16' (edge cases) | OK | Untouched. |
| R16a' (oracle) | UPDATED | Returns `Result<BigUint, OracleError>`; matches encoder bounds (SC-007). |
| R16b' (BigUint helper) | UPDATED | Independence clause aligned with R14' (SC-004 / SC-007 cross-impact). |
| R17-R20 (registry) | OK | Untouched. |
| R21 (CLI dual-flag) | UPDATED | Reformulated with explicit `conflicts_with` mechanics (SC-008). |
| R22 (encoders list) | OK | Untouched. |
| R23 (compute pipeline) | OK | Untouched. |
| R24-R28 (RecipeEncoder) | OK | Untouched. |
| §5 Future Work | UPDATED | Informative preamble + slot resolution (SC-011). |
| §6 Phases | UPDATED | Phase 3 split into 3a/3b/3c (SC-012). |
| §7 Tests T1-T4, T14-T23 | OK | Untouched. |
| §7 Tests T5-T13 | UPDATED | T7 oracle `.unwrap()`; T9 widened to 25 coeffs; T9b added; T10 negative cross-check; T11 negative cross-check; T13 fully rewritten (SC-006, SC-007, SC-009, SC-010). |
| §7 T20 (clap) | UPDATED | Tests `conflicts_with` mechanics (SC-008). |
| §8 Open Questions | UPDATED | Q5 added documenting Q5 (SC-013 implementation realization). |

### 4.2 Coherence with predecessor specs (no edits made; verified by inspection)

- **SPEC-00 §5.5 Normal Form:** R4 v3 `NotNormalForm (operational)` definition refines but does NOT contradict SPEC-00 §5.5; the operational layer is documented as such in §2.
- **SPEC-01 I4:** R4 / R14' v3 explicitly tie `NotNormalForm.redexes` semantics to I4 stale-entry pruning.
- **SPEC-01 T1-T7:** R13a' obligation set requires T1-T7 preservation by `wire_*_into` helpers (proof argument identical to SPEC-14 §4.3.1 / §4.3.2 modulo composition).
- **SPEC-04 default partition strategy:** T13 v3 names `round-robin` per SPEC-07 R3 default.
- **SPEC-05 BSP cycle:** T13 v3 invokes `run_grid` and references SPEC-05 implicitly via NG5 + R25 fallback.
- **SPEC-07 R3:** R21 v3 introduces a new "dual-form flag with `conflicts_with`" CLI pattern; SPEC-07 amendment is recommended as a separate task but NOT done in this revision.
- **SPEC-09 R17d:** R8 operand semantics for `sum_of_squares` references R17d as the source of `build_sum_of_squares`.
- **SPEC-14 R3:** No public function signature changed (R7 enforces this).
- **SPEC-14 R4 / R4b:** R12' explicitly cites both; cap dynamic via `MAX_CHURCH_NAT`.
- **SPEC-14 R15-R17:** Public signatures unchanged. `wire_*_into` are NOT in R3 export list (R13a' privacy clause).
- **SPEC-14 §4.4 `decode_nat`:** R14' v3 mirrors topology; R16b' cross-check is meaningful (independence clause).
- **SPEC-25 R7-R9, R26-R28:** HornerCodec is NOT a RecipeEncoder (Q4); R25 fallback applies.
- **SPEC-26 R1-R7:** Workspace prerequisite unchanged.
- **ARG-001:** Front-matter row updated; R13' rationale and T13 retargeted to G1 + P1 + P3 + P4.

### 4.3 Testability

Every MUST in v3 maps to at least one test in §7:

| Requirement | Test(s) |
|-------------|---------|
| R4 `NotNormalForm` semantics | Implicit in T9, T9b, T13 (queue may carry stale entries from merges) |
| R7 (no SPEC-14 R3 change) | T4 (690 v1 tests pass) |
| R8 operand semantics (exp, sum_of_squares) | T3 |
| R10' (module location) | All HornerCodec tests T5-T13 |
| R11' (input schema) | T5-T9, T9b, T10 |
| R12' (caps + panic-trapping) | T10 (negative cases), T11 negative cross-check |
| R13a' (composable helpers obligations) | New direct-helper tests in Phase 3a (Q5) + indirect via T6-T9, T9b, T13 |
| R13' (Horner construction + G1) | T6-T9, T9b, T13 |
| R14' (BigUint algorithm) | T9 (large), T9b (BigUint + boundary), T11, T12 |
| R15' (output schema with `bit_length`) | T9, T9b (verify `bit_length > 64`) |
| R16' (edge cases) | T5, T8, T10 |
| R16a' (oracle + Result<_, OracleError>) | T7, T9, T9b, T10 (negative), T11 (negative cross-check) |
| R16b' (BigUint helper independence) | T12 |
| R21 (`conflicts_with`) | T18, T19, T20 |

### 4.4 Completeness checklist (Round 1 § matched)

- All MUST have testable criteria — **PASS** (every MUST mapped above).
- Boundary conditions defined — **PASS** (R16' enumerates 0, 1, MAX, overflow; T9b exercises 10_000 boundary in BigUint range).
- Error conditions specified — **PASS** (R4 + R12' + R16a' OracleError covers all paths).
- Pseudocode for non-trivial operations — **PASS** (R14' inlines BigUint readback; R13' inlines Horner construction).
- Rust type signatures — **PASS** (R4 errors, R13a' helpers, R16a' oracle all spelled out).
- No undefined terms — **PASS** (§2 expanded with 11 entries).
- T1-T7 maintained — **PASS** (R13a' obligations explicit).
- D1-D6 maintained — **PASS** (HornerCodec rides on existing pipeline; R25 fallback for distribution; D5 confirmed).
- I1-I5 maintained — **PASS** (Church construction in SPEC-14 already preserves I1-I5; HornerCodec only composes via SPEC-14 primitives + R13a' helpers that share construction logic).
- G1 not violatable — **PASS** (R13' rationale and T13 framed correctly per SC-009 fix).

---

## 5. What this revision does NOT do

- Does NOT modify SPEC-14, SPEC-00, SPEC-25, SPEC-07, or any other spec. Verified by Grep over the workspace `specs/` directory: the only spec file touched is `SPEC-27-encoder-decoder-api.md`.
- Does NOT introduce new networking dependencies. R13a' helpers are pure synchronous Rust in the Core Layer.
- Does NOT introduce performance requirements. T13 measures equivalence of decoded values, not wall-clock time.
- Does NOT prescribe specific clap struct field names beyond the canonical pattern in R21 (developer chooses idiomatic naming).
- Does NOT amend SPEC-14 to expose `wire_*_into` publicly. They remain `pub(crate)` per R13a' privacy clause.

---

## 6. Recommendation for next pipeline step

Two options, in order of preference:

1. **Stage 1 (task-splitter) directly.** All 5 mandatory items (SC-001..004, SC-013) are resolved with concrete fixes; all 7 recommended items (SC-005..011) are accepted; both LOW items (SC-011, SC-012) are accepted. The disposition log above documents the chain of reasoning for each issue. The spec is internally coherent (§4.1) and consistent with all predecessors (§4.2) without amending any of them. The task-splitter can begin immediately.

2. **Optional Round 3 spec-critic re-verification.** If the user prefers an additional adversarial pass to sanity-check the new R13a' obligation set, the BigUint readback algorithm, and the `conflicts_with` clap mechanics before SDD splitting starts, a Round 3 critic run is supported. Round 3 should focus on:
   - R13a' obligation completeness (does the obligation set on `wire_*_into` cover everything HornerCodec depends on?)
   - R14' pseudocode end-state coverage (are there Church-numeral-shaped inputs that the pseudocode would mis-decode?)
   - T13 `extract_result(run_grid(...))` API correctness (does the function exist with that exact signature in v0.20.0-pre? — if not, T13 needs to name the actual extraction primitive)
   - Q5 obligation validation (does the existing `wire_*_into` implementation in `arithmetic.rs` actually satisfy R13a''s obligations, by inspection?)

**Recommendation: option 1 (proceed to Stage 1).** The 13 issues from Round 1 are exhaustively addressed; the user's preferred Caminho A for SC-013 is implemented; the spec is in a state where it can be split into atomic tasks without ambiguity. A Round 3 pass is supported but not required.

---

## 7. Files touched in Round 2

- `codigo/relativist/specs/SPEC-27-encoder-decoder-api.md` — content edits per §2 above (header revision history; §2 expansion with 11 new entries; R4 NotNormalForm semantics; R7 softened; R8 operand semantics; R10' R13a' R13' R14' R15' R16a' R16b' updated; R21 reformulated; §5.1 informative preamble + slot resolution; §6 Phase 3 split; T7-T13, T9b, T20 updated; Q5 added).
- `codigo/relativist/docs/spec-reviews/SPEC-27-v2-round2-response.md` — this file (new).
- `progress.md` (TCC root) — Round 2 entry recording the SPEC-27 v3 revision.

No other file in the workspace was modified. Camada 1 territory rules (only `codigo/relativist/specs/` + `codigo/relativist/docs/spec-reviews/` + `progress.md` for ESPECIALISTA EM SPECS) preserved.
