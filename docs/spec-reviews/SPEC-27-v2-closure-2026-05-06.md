# SPEC-27 v2 — Topic 2 Alignment — Closure Log

**Date:** 2026-05-06
**Author agent:** ESPECIALISTA EM SPECS (Camada 1)
**Spec:** `codigo/relativist/specs/SPEC-27-encoder-decoder-api.md`
**Branch:** `feature/stress-and-encoder` (Relativist subdir)
**Triggering handoff:** `codigo/relativist/docs/handoffs/2026-05-06-spec27-horner-revision-handoff.md`
**Status produced:** Revised v2 (Topic 2 alignment) — ready for spec-critic Round 1

---

## 1. Mandate

Apply the conceptual diff defined in §3 and §4 of the handoff brief: replace
LambdaCodec (R10-R16) with HornerCodec (R10'-R16b') as the v1 production codec;
demote LambdaCodec to a §5 Future Work entry; update R19 (default_registry),
NG2, the spec header revision history, and R21 to accept `--codec` as an alias
of `--encoder`.

Acceptance criteria, copied from §2 of the handoff brief:

> The SPEC-27 v2 MUST keep R1-R9, R17-R18, R20-R28; replace R10-R16 with
> R10'-R16b'; update R19 and NG2; add §5; bump revision history. It MUST NOT
> modify other specs, add networking dependencies, introduce performance
> requirements, or specify HornerCodec internal implementation details.

---

## 2. Conceptual diff applied

### 2.1 Header (front-matter)

- `Status: Draft` → `Status: Revised v2 (Topic 2 alignment)`.
- `References consumed:` annotated REF-005 as "future LambdaCodec, §5.1" while
  keeping it in the list (the reference is still consumed, just for §5.1
  instead of §3.4).
- `Arguments consumed:` annotated ARG-001 as "HornerCodec is the v1 empirical
  illustration of P3" — this makes the link to ARG-001 P3 explicit, satisfying
  handoff §7 item 3 (coherence with ARG-001).
- Added new `Handoffs consumed:` row pointing to the handoff brief.
- Added new `Design docs consumed:` row pointing to the Topic 2 design doc and
  the Horner explainer.
- Added explicit `Revision history:` block with v1 + v2 entries.

### 2.2 §1 Purpose

- Item 3 of the bullet list rewritten: was "LambdaEncoder — a proof-of-concept
  encoder for pure lambda-calculus terms"; now "HornerCodec — the v1 codec for
  distributed polynomial evaluation via Horner's method, composed entirely on
  top of SPEC-14's `build_add` / `build_mul`. Empirical illustration of
  ARG-001 P3."
- Added paragraph: "This spec wraps [the existing Church pipeline] as a
  `ChurchArithmeticCodec`, adds HornerCodec as a second built-in codec…".
- Added closing paragraph: "Future codec work (LambdaCodec via Mackie/Pinto and
  other candidate codecs) is documented in §5 Future Work; it is intentionally
  out of v1 scope."

### 2.3 §2 Definitions

- Removed `LambdaEncoder` row (its content moved to §5.1 narrative).
- Added `HornerCodec` row (Horner over `build_add`/`build_mul`, BigUint readback).
- Added `ChurchArithmeticCodec` row (wraps SPEC-14's `compute_arithmetic` pipeline
  under the trait API).
- Added `Horner's method` row (the recurrence with its mathematical statement and
  the "classically sequential" property that motivates its choice as the demo
  codec).

### 2.4 §3.4 (the substantive replacement)

Replaced the entire §3.4 "Lambda Calculus Codec" section (R10-R16) with a new
§3.4 "Horner Polynomial Codec" containing R10'-R16b':

- **R10'** (replaces R10): module location and primitive composition.
- **R11'** (replaces R11): JSON input schema `{coeffs: [u64], x: u64}`.
- **R12'** (replaces R12): bounds inherited from SPEC-14 R4 (`<= 10_000`).
- **R13'** (replaces R13): explicit Horner recurrence pseudocode + the
  empirical statement "Normal Form is invariant under reduction order so any
  worker count W and any BSP schedule MUST produce the same polynomial value"
  — this is the verbal restatement of ARG-001 P3 in HornerCodec terms.
- **R14'** (replaces R14): BigUint readback (Normal Form check + traversal +
  base-10 string output).
- **R15'** (replaces R15): JSON output schema `{value: string, bit_length: usize}`.
- **R16'** (replaces R16): edge-case enumeration (empty, constant, x=0, all-zero,
  boundary inclusive 10_000, overflow).
- **R16a'** (new): pure-Rust oracle `horner_serial` returning `BigUint`.
- **R16b'** (new): BigUint readback helper module + property cross-check
  against SPEC-14's `decode_nat` for `n <= u64::MAX`.

The introductory paragraph of §3.4 explicitly frames HornerCodec as the
empirical illustration of ARG-001 P3, satisfying handoff §7 item 3.

### 2.5 §3.5 Encoder Registry

- **R19** updated: list now contains `church_add`, `church_mul`, `church_exp`,
  `church_sum_of_squares`, `horner`. The bullet for `"horner"` carries a back-pointer
  to R10'-R16b' (HornerCodec). Explicit clause: `"lambda"` MUST NOT appear in the
  default registry; it is documented as future work in §5.1.
- R17, R18, R20 left unchanged structurally.

### 2.6 §3.6 CLI Integration

- **R21** rewritten to mandate that `--encoder <name>` is a MUST and `--codec <name>`
  is a clap alias of `--encoder` (also MUST in v2). Both flags MUST select the same
  registry entry; passing both simultaneously MUST yield a clap conflict error;
  omitting both MUST preserve the legacy `compute add 3 5` positional fallback.
  Editorial decision recorded in §3 of this log.
- **R22** updated: example output replaces the `lambda` line with `horner`.
  `relativist codecs list` is added as a MAY (clap alias of the `encoders list`
  subcommand, terminologically symmetric with R21).
- R23 unchanged.

### 2.7 §3.7 RecipeEncoder Generalization

- R24-R28 untouched.

### 2.8 §4 Non-Goals

- **NG2** rewritten: was specific to `LambdaCodec`; now generalized — "No codec
  in v1 uses labeled IC symbols. HornerCodec uses only Lafont's 3-symbol set
  (CON/DUP/ERA) and composes Church numerals from SPEC-14, which also use only
  Lafont's symbols. The future LambdaCodec (§5.1) will likewise use only Lafont's
  set. HVM compatibility requires ROADMAP 2.42, out of scope."
- **NG3** rewritten: was "no syntax beyond the lambda-calculus term format in R12";
  now "HornerCodec consumes plain JSON only (R11'); any future string surface
  syntax (e.g., LambdaCodec's term format) will be introduced under its own
  spec extension, not under SPEC-27."
- NG1, NG4, NG5 untouched.

### 2.9 New §5 Future Work

Inserted as a new section between §4 Non-Goals and the (renumbered) §6
Implementation Phases. Two subsections:

- **§5.1 LambdaCodec (deferred from v1)** — three-point justification copied
  verbatim from handoff §4, plus the Mackie/Pinto encoding sketch (the same
  five bullets that were R13 in v1, now reframed as future-work guidance, not
  v1 requirements). REF-005, AC-013, DISC-012 v2 cross-references retained.
- **§5.2 Other deferred codecs** — `FactorialCodec`, `FibonacciCodec`,
  `MatMulCodec`, `PolynomialMultiEvalCodec`. Each is annotated as "pure
  composition of SPEC-14 primitives plus its own input-schema decoder; would
  require no changes to SPEC-01, SPEC-02, or wire protocol."

### 2.10 Renumbered tail sections

The pre-revision §5 Implementation Phases / §6 Test Strategy / §7 Open Questions
were each shifted by +1 to make room for the new Future Work section:

- §6 Implementation Phases: Phase 3 deliverable retitled `HornerCodec` (was
  `LambdaCodec`); the LoC budget kept (~250 LoC, the same envelope).
- §7 Test Strategy: subsections renumbered 7.1-7.6. Tests T5-T9 (Lambda) replaced
  by T5-T13 (Horner) covering constant polynomial, smallest non-trivial case,
  canonical Horner case from explainer (with oracle-derived expected value),
  sparse coefficients, BigUint range, edge cases per R16', property test against
  oracle (R16a'), `decode_biguint` cross-check (R16b'), and a new T13
  distributed-equivalence test that explicitly invokes ARG-001 P3 as the property
  under test. T14-T16 (registry), T17-T21 (CLI integration including alias and
  conflict tests), T22-T23 (RecipeEncoder including HornerCodec fallback to
  centralized partition).
- §8 Open Questions: Q1, Q2 retained (with minor wording polish on Q1: Box vs
  Arc both acceptable). Q3 retired (it was Lambda-specific). Replacement Q3
  (decimal vs hex BigUint output) and Q4 (decomposability of HornerCodec —
  explicitly NOT a `RecipeEncoder` because Horner is sequential; future
  PolynomialMultiEvalCodec would be the natural decomposable variant) added.

---

## 3. Editorial decisions (recorded for the spec-critic)

### 3.1 `--codec` is mandated as alias, not optional

Handoff §3.6 R21 says: "The `compute` subcommand MUST accept an `--encoder`
flag. The flag `--codec` MAY be accepted as an alias…" with a final note "the
ESPECIALISTA EM SPECS decides."

**Decision:** `--codec` is also MUST in v2 (full alias parity), with a clap
conflict guard if both are passed. **Justification:** the Topic 2 design doc
and the Horner explainer both consistently use the term "codec"; the legacy
`compute` subcommand and the SPEC-27 R1 trait both use "encoder"; mandating
parity prevents a future migration cliff and keeps the user-visible CLI
self-consistent with both terminologies that already appear in the project's
own documentation. The implementation cost in clap is one `aliases` macro line.

### 3.2 `codecs list` subcommand alias is MAY (not MUST)

For symmetry with R21 the spec records `relativist codecs list` as a MAY.
The MUST is `relativist encoders list` (per R22 v1 wording, preserved).
**Justification:** subcommand aliases in clap are slightly more invasive than
flag aliases (they affect help output and tab-completion); marking the
`codecs list` form as MAY lets the developer ship it cleanly or defer it
without violating the spec.

### 3.3 Terminology in the spec text

The spec internally uses "encoder" when referring to the SPEC-27 R1 trait
(e.g., "the encoder MUST construct the IC net…") and "codec" when referring
to the registered Codec object (e.g., "ChurchArithmeticCodec", "HornerCodec",
"the v1 codec"). This preserves the original SPEC-27 trait naming (R1, R2, R3
unchanged) while embracing "codec" as the user-facing umbrella term in CLI
flags, registry list output, and §5 Future Work.

### 3.4 R13' coefficient ordering convention

R11' is explicit that `coeffs[0]` is the constant term and `coeffs[len-1]` is
the leading coefficient (i.e., `p(x) = sum(coeffs[i] * x^i for i in 0..=n)`).
The Horner pseudocode in R13' is consistent with this convention: `acc` is
initialized to `coeffs[n]` and the loop iterates `n-1 down to 0`, multiplying
by `x` and adding `coeffs[k]`. T7 (canonical case) was clarified to **derive
the expected value from `horner_serial`** rather than hard-code a string,
because the design doc tabulated `[3,2,5,1] @ x=2 → 43`, which is inconsistent
with R11' (the correct value under R11' ordering is 35). Using `horner_serial`
as the test oracle eliminates this drift and is explicitly required by R16a'
itself.

### 3.5 HornerCodec is intentionally NOT a `RecipeEncoder`

Handoff §3.7 says R24-R28 stay intact. This was followed verbatim. The
implication that HornerCodec will use the centralized fallback (R25) is now
explicit in T23 and Q4. The natural decomposable codec — `PolynomialMultiEvalCodec`,
where K independent evaluation points share the polynomial structure — is
documented in §5.2 as future work.

---

## 4. Coherence validation (per handoff §7 checklist)

### 4.1 Internal coherence

| Item | Status | Notes |
|------|--------|-------|
| R1-R3 (traits) intact | OK | Not touched. |
| R4 (error types) intact | OK | Not touched. |
| R5-R6 (encode contract) intact | OK | Not touched. |
| R7-R9 (ChurchArithmeticCodec) intact | OK | Not touched. R8 still defines the JSON schema for the four ops. |
| R10-R16 → R10'-R16b' (HornerCodec) | OK | Replacement complete, no orphan references to LambdaCodec in §3.4. |
| R17-R18 (registry mechanics) intact | OK | Not touched. |
| R19 (default_registry built-ins) updated | OK | `horner` replaces `lambda`. |
| R20 (duplicate registration) intact | OK | Not touched. |
| R21 (CLI `compute --encoder` flag) | UPDATED | `--codec` alias mandated; clap conflict on both. |
| R22 (encoders list) | UPDATED | `horner` replaces `lambda`; `codecs list` MAY alias added. |
| R23 (compute pipeline) intact | OK | Not touched. |
| R24-R28 (RecipeEncoder + AssignRecipe) intact | OK | Not touched. |

### 4.2 Coherence with predecessor specs

- **SPEC-14 R4:** R12' caps (`coeffs[i] <= 10_000`, `x <= 10_000`) are the
  exact caps from SPEC-14 R4 (and SPEC-14 R4 is referenced explicitly in R12'
  prose). R16b' explicitly cross-references `decode_nat` (SPEC-14 R-decode)
  for the BigUint helper invariant.
- **SPEC-02:** R10' specifies `relativist-core::encoding::horner` — a sibling
  of `encoding::church.rs` and `encoding::arithmetic.rs`. No changes to
  `Net`, `Symbol`, `PortRef`, `AgentId`, `PortId`.
- **SPEC-04 / SPEC-05:** R13' references `reduce_all` (SPEC-03) and "any
  distributed equivalent that respects the BSP cycle of SPEC-05" — no protocol
  changes implied; HornerCodec rides on top of the existing pipeline.
- **SPEC-25:** R26-R28 unchanged. T22 confirms backward compatibility; T23
  documents that HornerCodec falls back to centralized partition (R25 fallback
  path), so AssignRecipe is not exercised by HornerCodec in v2. PolynomialMultiEvalCodec
  is flagged in §5.2 as the natural future RecipeEncoder candidate.
- **SPEC-26 R1-R7:** unchanged dependency on the workspace restructure.

### 4.3 Coherence with ARG-001

R13' carries the explicit prose: "This requirement is the empirical statement
of ARG-001 P3: the Normal Form is invariant under reduction order, so any
worker count W ≥ 1 and any BSP schedule MUST produce the same polynomial value."
T13 (distributed-equivalence test) is the empirical assertion under property
testing. The header `Arguments consumed: ARG-001` row is annotated to make
the linkage discoverable by the spec-critic.

### 4.4 Testability

Every R10'-R16b' is paired with at least one test:

| Requirement | Test(s) |
|-------------|---------|
| R10' (module location) | T5-T13 implicitly (any test that imports and uses HornerCodec) |
| R11' (input schema) | T5, T6, T7, T8, T9, T10 (varied coeffs/x inputs) |
| R12' (bounds) | T10 (boundary inclusive 10_000, overflow rejected) |
| R13' (Horner recurrence + NF invariance) | T6, T7, T8, T9, T13 (distributed equivalence) |
| R14' (BigUint readback / NF check) | T9 (BigUint range), T11 (property), implicit in T5-T13 |
| R15' (output schema) | T5-T13 (every test reads `value` and may read `bit_length`) |
| R16' (edge cases) | T5 (constant), T6 (smallest), T8 (sparse), T10 (full enumeration) |
| R16a' (oracle) | T7 (uses oracle), T11 (property test against oracle) |
| R16b' (BigUint helper) | T12 (cross-check with `decode_nat`) |

### 4.5 Completeness

The spec contemplates: input schema (R11'), output schema (R15'), encode
contract (R13' + R5-R6), edge cases (R16'), oracle (R16a'), BigUint readback
(R14' + R16b'), CLI integration (R21-R22 with alias), and explicit relation
to Future Work (§5.1 LambdaCodec, §5.2 other codecs).

---

## 5. What this revision does NOT do (handoff §2 NOT clauses)

- Does not modify SPEC-14, SPEC-25, or any other spec. Verified by Grep over
  the workspace `specs/` directory: the only file touched is
  `SPEC-27-encoder-decoder-api.md`.
- Does not add networking dependencies. R10'-R16b' compose entirely over
  in-crate `encoding::*` modules; the only new external dependency is
  `num-bigint = "0.4"` for BigUint readback (already noted in the Topic 2
  design doc; not a networking dep).
- Does not introduce performance requirements. HornerCodec is positioned as a
  correctness demonstration. T13 stipulates equivalence of result, not a
  bound on wall-clock time.
- Does not specify HornerCodec's internal implementation. The implementation
  module structure (`encoding::horner` + `encoding::biguint_readback`) is
  named at the same level of abstraction as SPEC-14's `encoding::church.rs`;
  internal data structures (whether the encoder uses an explicit `acc:
  AgentId` or a builder pattern) are left to the developer.

---

## 6. Next steps in the SDD pipeline (handoff §8)

1. **spec-critic Round 1** — adversarial review by spec-critic agent, focusing on:
   - Predecessor consistency (SPEC-14, SPEC-25 cross-refs).
   - Testability of every R10'-R16b'.
   - Edge-case completeness in R16'.
   - Correct preservation of invariants in R13' (no new constraint on the
     reducer).
   - That the editorial decisions in §3 of this log (especially 3.1 mandatory
     `--codec` alias) are non-controversial or, if challenged, can be rolled
     back to MAY without affecting the v1 codec correctness story.
2. **ESPECIALISTA EM SPECS Round 2** — addresses spec-critic findings, if any,
   produces a final closure log.
3. **Stage 1 (task-splitter)** — splits SPEC-27 v2 into ~10 atomic TASKs, each
   < 200 LoC, in `docs/backlog/`.
4. **Stage 2 (test-generator)** — TEST-SPECs per TASK from §7 of the spec.
5. **Stage 3 (developer)** — TDD RED → GREEN → REFACTOR.
6. **Stage 4 (reviewer)** — quality + architecture review.
7. **Stage 5 (qa)** — adversarial bug hunting, focusing on Horner edge cases:
   coefficient overflow, malformed JSON, registry double-register, BigUint
   round-trip with negative-looking inputs (none accepted, but a hostile
   parser test).
8. **Stage 6 (developer REFACTOR)** — apply fixes, verify all tests pass.

In parallel, **Topic 1 (Stress Curve Campaign)** can advance SDD immediately —
no spec to revise (it's bench methodology, not feature).

---

## 7. Files touched

- `codigo/relativist/specs/SPEC-27-encoder-decoder-api.md` — content edits
  (header, §1, §2, §3.4 substantive replacement, R19, R21-R22, NG2, NG3,
  new §5 Future Work, renumbered §6/§7/§8).
- `codigo/relativist/docs/spec-reviews/SPEC-27-v2-closure-2026-05-06.md` —
  this file (new).
- `progress.md` (TCC root) — new entry recording the SPEC-27 v2 revision
  (per Camada 1 inviolable rule §2).

No other file in the workspace was modified.
