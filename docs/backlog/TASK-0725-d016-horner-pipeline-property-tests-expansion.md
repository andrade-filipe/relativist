# TASK-0725 — D-016 Horner pipeline: expand property tests over full bounds (degree 2-4, full MAX_CHURCH_NAT)

**Spec:** SPEC-27 v3 (`specs/SPEC-27-encoder-decoder-api.md`) — R16' (T11 positive cross-check), R16b' (cross-readback)
**Requirements:** T7 (pipeline parity), T9 (BigUint witness), T11 (oracle cross-check), G1 (Fundamental Property)
**Priority:** P1 (validation depth — without it, the TASK-0723/0724 fixes are correct on demos but unaudited at scale)
**Status:** TODO
**Depends on:** TASK-0723 + TASK-0724 (both must be in HEAD for the property tests to pass deterministically)
**Blocked by:** TASK-0724
**Estimated complexity:** S–M (~80 LoC test code, 0 LoC production)
**Bundle:** D-016 — HornerCodec decoder extension

---

## Context

After TASK-0723 + TASK-0724, the readable subset of `HornerCodec` covers
**all** valid inputs that the encoder accepts (`coeffs.len() >= 1`,
each value `<= MAX_CHURCH_NAT = 10_000`). This TASK adds property tests
that exercise this full envelope and serve two roles:

1. **Regression dam against future readback rewrites.** Any change to
   `biguint_readback.rs` (e.g., the Mackie/Pinto future-work refactor
   mentioned in SPEC-27 §5.1) MUST keep these tests green.
2. **Empirical G1 witness at scale.** The existing
   `horner_distributed_g1.rs` (Topic 2) tests already exercise G1 on
   single-iteration inputs (Demo 2 ≡ Demo 3 in
   `docs/demos/horner-g1-demonstration.md`). After TASK-0724, the same
   `sequential vs distributed(W) → same value` cross-check becomes
   meaningful for degree ≥ 2 polynomials — multi-iteration G1
   witnesses, which are the cleanest empirical evidence for ARG-001.

Importantly, this TASK does NOT add production code. It is a
**test-only audit pass**. If TASK-0723 or TASK-0724 leaves any gap, this
TASK's property tests will surface it as a hard fail and trigger a
follow-up REFACTOR.

## Acceptance Criteria

- [ ] New IT file `relativist-core/tests/horner_pipeline_property.rs` with at least 3 property tests covering `coeffs.len() in 2..=4 × c_i in 0..=MAX_CHURCH_NAT × x in 0..=MAX_CHURCH_NAT`, each ≥ 50 cases (debug) / 200 cases (`release`).
- [ ] Each property test cross-checks `HornerCodec`(encode → reduce_all → discover_root → decode) against `horner_serial`. Mismatches MUST fail loudly (no silent `Err` → pass).
- [ ] The pipeline test grid covers AT LEAST these representative slices:
  - **Slice A (dense small):** `coeffs.len() == 2`, `c_i in 0..=20`, `x in 0..=20`. Verifies TASK-0723's full coverage.
  - **Slice B (degree-3 modest):** `coeffs.len() == 3`, `c_i in 0..=50`, `x in 0..=50`. Verifies TASK-0724's typical case.
  - **Slice C (boundary near cap):** `coeffs.len() == 2`, `c_i in 9990..=10_000`, `x in 9990..=10_000`. Verifies boundary coefficients.
  - **Slice D (T9 BigUint witness):** explicit case `coeffs.len() == 25`, all-ones, `x = 10` — deterministic single-case test (not a proptest), MUST decode to `"1111111111111111111111111"`.
- [ ] PT-0715-06 in `horner.rs` is updated one final time: the skip-rate threshold drops to **≤ 5%** on the 1..=10 × 1..=10 × 1..=10 grid (target is 0%; 5% is the regression gate). Comment cites TASK-0725 as the threshold-setter and TASK-0723/0724 as the enablers.
- [ ] The G1 distributed-vs-sequential cross-check tests in `relativist-core/tests/horner_distributed_g1.rs` are widened to cover at least one degree-2 case (`{"coeffs":[3,2,5,1],"x":2}` is a natural choice — matches existing oracle assertions) AND verify that `--workers 1`, `--workers 2`, `--workers 4`, `--workers 8` all return the same value.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/tests/horner_pipeline_property.rs` | **CREATE.** | ~3-4 property tests covering Slices A-D above; helper `pipeline_value(coeffs, x) -> Result<BigUint, ...>` factored out. ~70-90 LoC. |
| `relativist-core/src/encoding/horner.rs` | modify | Update PT-0715-06 threshold to ≤ 5%; update comment; possibly delete the now-obsolete "skip-rate guard" once it has converged. ~5 LoC. |
| `relativist-core/tests/horner_distributed_g1.rs` | modify | Add degree-2 G1 cross-check case as described in AC-5. ~15-25 LoC. |

## Key Types / Signatures

No production-side signatures. Suggested test helper:

```rust
/// Run the full HornerCodec pipeline and return the BigUint value (or
/// propagate the error). For property-test use in
/// `tests/horner_pipeline_property.rs`.
fn pipeline_value(coeffs: &[u64], x: u64) -> Result<BigUint, Box<dyn std::error::Error>>;
```

## Test Expectations (for Stage 2 test-generator)

The TASK IS the test. The test-generator MAY refine the property
distributions, add shrinking strategies, or factor common fixture code
— but MUST preserve the 4 slices A-D and the ≤ 5% skip-rate target.

Specific gates the test-generator MUST emit:

- **PT-0725-A** — Slice A property: ≥ 50 cases, `prop_assert_eq!(pipeline_value(&coeffs, x).unwrap(), horner_serial(&coeffs, x).unwrap())`.
- **PT-0725-B** — Slice B property: same shape.
- **PT-0725-C** — Slice C property: same shape with boundary domain.
- **UT-0725-D** — Slice D deterministic case (T9 witness).
- **UT-0725-E** — G1 cross-check degree-2: encode `{"coeffs":[3,2,5,1],"x":2}`, reduce sequentially → 35; reduce distributed with W=1,2,4,8 → 35 each. Use the in-process `local` mode (matches the existing Demo 3 in `docs/demos/horner-g1-demonstration.md`).

## Dependencies Context

- TASK-0723 + TASK-0724 production code MUST be in HEAD.
- `horner_serial` (TASK-0713) — oracle.
- `local` mode helpers in `relativist-core` for distributed-mode invocation (already used by existing `horner_distributed_g1.rs`).
- Existing `pipeline()` helper in `horner.rs::tests` (lines 458-469) — extract or adapt for the new IT file (private `pub(crate)` may be required).

## Notes

- This TASK is the **first opportunity** to delete the "Limitações conhecidas" section in `docs/demos/horner-g1-demonstration.md` — but DO NOT do it here. That's TASK-0726's job (a doc-only TASK keeps the dependency graph clean).
- Property test cases at `release` mode SHOULD be 200; at debug mode 50 is sufficient. Use `#![proptest_config(ProptestConfig { cases: if cfg!(debug_assertions) { 50 } else { 200 }, ..Default::default() })]`.
- Slice C runs the encoder near its cap; the encoded net per case is ~20-40k agents and the reduction takes O(seconds). Be prepared for the IT file's wall-time to grow ~10-20s. Acceptable.
- The G1 cross-check (UT-0725-E) is the empirical headline result of D-016. Make sure its failure message cites ARG-001 P1 (Lafont confluence) explicitly so the failure narrative writes itself.
- Test floor delta: this TASK adds ~5 tests. Combined with TASK-0723 (+5) and TASK-0724 (+7) the total D-016 test floor delta is ~+17.

## Sequencing within D-016

LAST functional TASK in D-016 (TASK-0726 is doc-only). Run after both
TASK-0723 and TASK-0724 are green. If PT-0725-A/B/C fails on landing,
file a Stage 5 QA escalation — this is the safety net.
