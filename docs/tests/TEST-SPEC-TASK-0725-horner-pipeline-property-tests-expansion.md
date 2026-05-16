# TEST-SPEC-TASK-0725: Tests for TASK-0725 — Horner pipeline property tests (full envelope)

**Task:** TASK-0725
**Spec:** SPEC-27 v3 (`specs/SPEC-27-encoder-decoder-api.md`)
**Bundle:** D-016 — HornerCodec decoder extension
**Requirements covered:** R16' (T11 positive cross-check), R16b' (cross-readback), G1 evidence at degree >= 2.
**Test IDs (from SPEC-27 v3 §7.3):** T7, T9 (BigUint witness — deterministic), T11 (oracle cross-check at full bounds), T13 (in-process distributed equivalence, degree >= 2).
**Inviolable invariants asserted:** SPEC-01 T1-T7 (encoder); G1 (Fundamental Property — distributed value == sequential value).
**Production code under test:** NONE — this is a **test-only audit pass**. The tests EXERCISE the helpers from TASK-0723 + TASK-0724. Failures here trigger a Stage 5 QA escalation against those tasks.

---

## Scope

This TEST-SPEC creates `relativist-core/tests/horner_pipeline_property.rs` (NEW file) containing the property tests at the **full input envelope** allowed by SPEC-27 v3. The four slices A-D come from TASK-0725 acceptance criteria; the test-generator MAY refine the property distributions or factor common fixture code but MUST preserve them.

This TEST-SPEC also performs the FINAL hardening of PT-0715-06 (skip-rate threshold → ≤ 5%) and widens `horner_distributed_g1.rs` to the canonical degree-3 G1 witness `{"coeffs":[3,2,5,1],"x":2}` across `W ∈ {1, 2, 4, 8}`.

## Test category & location

| # | Category | Cfg gating | File | LoC |
|---|----------|-----------|------|-----|
| PT-0725-A | property test (proptest) — Slice A | none | `relativist-core/tests/horner_pipeline_property.rs` **(CREATE)** | ~25 |
| PT-0725-B | property test (proptest) — Slice B | none | same | ~25 |
| PT-0725-C | property test (proptest) — Slice C boundary | `cfg!(not(debug_assertions))` for full case count | same | ~30 |
| UT-0725-D | unit (deterministic T9 witness) | none | same | ~20 |
| UT-0725-E | unit (G1 degree-3 cross-check) | none | `relativist-core/tests/horner_distributed_g1.rs` (modify) | ~30 |
| RT-0725-01 | PT-0715-06 final threshold tightening to ≤ 5% | none | `relativist-core/src/encoding/horner.rs:682` (modify) | ~5 |
| HELPER-0725 | shared fixture | none | `relativist-core/tests/horner_pipeline_property.rs` **(CREATE)** | ~15 |

## Test floor delta (from TASK-0725 acceptance criteria)

- default: **+5 new tests** (PT A-C, UT D-E) + 0 net delta from RT-0725-01
- Expected post-TASK: default ≥ 1819, zero-copy ≥ 1863, streaming-no-recycle ≥ 1810, release ≥ 1761

---

## Shared Test Helper

### HELPER-0725: `pipeline_value`

**Purpose:** Factor the encode → reduce → discover_root → decode_biguint pipeline into a single helper so each property test focuses on its slice.

**Signature (specification):**
```rust
/// Run the full HornerCodec pipeline and return the BigUint value.
/// Used by property tests in this file.
fn pipeline_value(coeffs: &[u64], x: u64) -> Result<num_bigint::BigUint, String> {
    use relativist_core::encoding::{Codec, Encoder, HornerCodec};
    use relativist_core::encoding::biguint_readback::decode_biguint;
    use relativist_core::reduction::reduce_all;

    let codec = HornerCodec::new();
    let json = serde_json::json!({"coeffs": coeffs, "x": x});
    let bytes = serde_json::to_vec(&json).map_err(|e| e.to_string())?;
    let mut net = codec.encode(&bytes).map_err(|e| format!("encode: {e:?}"))?;
    reduce_all(&mut net);
    if net.root.is_none() {
        relativist_core::encoding::arithmetic::discover_root(&mut net);
    }
    decode_biguint(&net).map_err(|e| format!("decode: {e:?}"))
}
```

(If `decode_biguint` is not re-exported by the public API, the test helper accesses it via `relativist-core/src/encoding/mod.rs` re-export or via a `pub(crate)` re-export wrapper added to `lib.rs` in this TASK; the developer chooses.)

---

## Property Tests

### PT-0725-A: `pipeline_value_matches_oracle_slice_A_dense_small`

**Purpose:** Slice A — `coeffs.len() == 2`, `c_i in 0..=20`, `x in 0..=20`. Full coverage of TASK-0723's domain at randomized resolution (the deterministic 216-case grid in UT-0723-02 covers only `(0..=5) × (2..=5) × (0..=5)`; this slice expands to `(0..=20)^2 × (0..=20)` and includes c_1 == 1 fast path).

**Generator strategy:**
```text
c0 in 0u64..=20
c1 in 0u64..=20   // includes c1 == 0 (oracle returns c0); includes c1 == 1 (fast path)
x  in 0u64..=20
```

**Assertion:**
```rust
proptest! {
    #![proptest_config(ProptestConfig {
        cases: if cfg!(debug_assertions) { 50 } else { 200 },
        ..ProptestConfig::default()
    })]
    #[test]
    fn pt_0725_a_slice_a_dense_small(
        c0 in 0u64..=20,
        c1 in 0u64..=20,
        x  in 0u64..=20,
    ) {
        let coeffs = [c0, c1];
        let expected = horner_serial(&coeffs, x).unwrap();
        let actual = pipeline_value(&coeffs, x)
            .unwrap_or_else(|e| panic!("c0={c0} c1={c1} x={x}: {e}"));
        prop_assert_eq!(actual, expected);
    }
}
```

**Sample size:** 50 (debug) / 200 (release).

**Shrinking note:** Failures minimize toward `(0, 0, 0)`. Any failure here means TASK-0723 or TASK-0724 regressed on the simplest possible cofactor case.

---

### PT-0725-B: `pipeline_value_matches_oracle_slice_B_degree_3_modest`

**Purpose:** Slice B — `coeffs.len() == 3`, each `c_i in 0..=50`, `x in 0..=50`. Verifies TASK-0724's typical degree-2 case at a moderate input scale (the deterministic UT-0724-03 covers only `[3,2,5,1]@2`; this exercises 200 random degree-2 cases).

**Generator strategy:**
```text
coeffs: vec of length exactly 3, each element 0u64..=50
x:      0u64..=50
```

**Assertion:**
```rust
proptest! {
    #![proptest_config(ProptestConfig {
        cases: if cfg!(debug_assertions) { 50 } else { 200 },
        ..ProptestConfig::default()
    })]
    #[test]
    fn pt_0725_b_slice_b_degree_3_modest(
        coeffs in proptest::collection::vec(0u64..=50, 3..=3),
        x in 0u64..=50,
    ) {
        let expected = horner_serial(&coeffs, x).unwrap();
        let actual = pipeline_value(&coeffs, x)
            .unwrap_or_else(|e| panic!("coeffs={coeffs:?} x={x}: {e}"));
        prop_assert_eq!(actual, expected);
    }
}
```

**Sample size:** 50 (debug) / 200 (release).

**Shrinking note:** Failures minimize toward `[0, 0, 0]@0`. A typical failure mode the developer should expect: if the recursion guard in `biguint_readback.rs` is set too low (e.g., the TASK-0724 guard at `depth > 64` is not bumped to handle the additional `c_i in 0..=50` overhead), some shrunk counterexamples will show a `DecodeError::UnrecognizedStructure("max recursion depth")` message — that's the signal to revisit the guard.

---

### PT-0725-C: `pipeline_value_matches_oracle_slice_C_boundary_near_cap`

**Purpose:** Slice C — `coeffs.len() == 2`, `c_i in 9990..=10_000`, `x in 9990..=10_000`. Stress-tests the readback near `MAX_CHURCH_NAT`. Each case produces a reduced net of ~20-40k agents (encoder is O(c_1 + x)).

**Generator strategy:**
```text
c0 in 9990u64..=10_000
c1 in 9990u64..=10_000
x  in 9990u64..=10_000
```

**Assertion:**
```rust
proptest! {
    #![proptest_config(ProptestConfig {
        // Debug mode: 25 cases (each takes 5-30s); release: 200.
        cases: if cfg!(debug_assertions) { 25 } else { 200 },
        ..ProptestConfig::default()
    })]
    #[test]
    fn pt_0725_c_slice_c_boundary_near_cap(
        c0 in 9990u64..=10_000,
        c1 in 9990u64..=10_000,
        x  in 9990u64..=10_000,
    ) {
        let coeffs = [c0, c1];
        let expected = horner_serial(&coeffs, x).unwrap();
        let actual = pipeline_value(&coeffs, x)
            .unwrap_or_else(|e| panic!("c0={c0} c1={c1} x={x}: {e}"));
        prop_assert_eq!(actual, expected);
    }
}
```

**Sample size:** 25 (debug) / 200 (release).

**Performance note:** Each case reduces ~20-40k agents. Debug wall-time ~30-60 s for 25 cases. Acceptable per TASK-0725 Notes ("be prepared for the IT file's wall-time to grow ~10-20 s").

**Shrinking note:** Failures minimize toward `(9990, 9990, 9990)`. Any failure indicates the readback degrades at the cap — most likely a u32 / usize narrowing bug if BigUint is misused.

---

## Unit Tests

### UT-0725-D: `pipeline_value_t9_biguint_witness_deterministic`

**Purpose:** Deterministic T9 witness — `coeffs = [1; 25]`, `x = 10`. The same content as UT-0724-05 but invoked from the IT file (cross-crate integration). Catches re-exports going stale.

**Input:**
```rust
use num_bigint::BigUint;

#[test]
fn pt_0725_d_t9_witness_deterministic() {
    let coeffs = vec![1u64; 25];
    let actual = pipeline_value(&coeffs, 10).expect("T9 witness must decode");
    let expected = BigUint::parse_bytes(b"1111111111111111111111111", 10).unwrap();
    assert_eq!(actual, expected);
    assert!(expected.bits() > 64, "T9 witness must exceed u64::MAX");
}
```

**Expected output:** Pass.

**Edge cases:**
- (EC-1) Result `bits() == 84` exactly (matches RT-0724-09 promotion).
- (EC-2) If the test fails with `Err(decode: ...)`, TASK-0724's recursion guard is not high enough for `coeffs.len() == 25`.

---

### UT-0725-E: `g1_in_process_distributed_degree_3` (widening of `horner_distributed_g1.rs`)

**Purpose:** **Headline empirical G1 witness for D-016.** Encode `{"coeffs":[3,2,5,1],"x":2}` → reduce sequentially → 35; reduce distributed with `W ∈ {1, 2, 4, 8}` → 35 each. This is the canonical degree-3 case (matches UT-0714-03 and UT-0724-03).

**Location:** `relativist-core/tests/horner_distributed_g1.rs` — MODIFY the existing file to add this test. Use the in-process `local` mode (matches Demo 3 in `docs/demos/horner-g1-demonstration.md`).

**Input (specification — the developer adapts to the existing `horner_distributed_g1.rs` helper conventions):**
```rust
#[test]
fn g1_in_process_distributed_degree_3_canonical() {
    use relativist_core::encoding::{Codec, Encoder, HornerCodec};
    use relativist_core::merge::{run_grid, GridConfig};
    use relativist_core::partition::ContiguousIdStrategy;
    use relativist_core::reduction::reduce_all;

    let json = br#"{"coeffs":[3,2,5,1],"x":2}"#;
    let expected_value = "35";

    // Sequential baseline.
    let codec = HornerCodec::new();
    let mut net_seq = codec.encode(json).unwrap();
    reduce_all(&mut net_seq);
    if net_seq.root.is_none() {
        relativist_core::encoding::arithmetic::discover_root(&mut net_seq);
    }
    let seq_out = codec.decode(&net_seq).unwrap();
    assert_eq!(
        seq_out["value"].as_str().unwrap(),
        expected_value,
        "ARG-001 P1 sequential baseline: degree-3 must reduce to {expected_value}"
    );

    // Distributed cross-check for each W.
    for &w in &[1usize, 2, 4, 8] {
        let mut net_w = codec.encode(json).unwrap();
        let cfg = GridConfig::new_in_process(w, ContiguousIdStrategy);
        let merged = run_grid(&mut net_w, &cfg).expect("in-process grid run");
        let mut merged_net = merged.net;
        if merged_net.root.is_none() {
            relativist_core::encoding::arithmetic::discover_root(&mut merged_net);
        }
        let out = codec.decode(&merged_net).unwrap_or_else(|e| {
            panic!(
                "ARG-001 P1 (Lafont confluence) FAILED for W={w}: \
                 distributed reduction diverged from sequential ({expected_value}) — \
                 readback error: {e:?}"
            )
        });
        assert_eq!(
            out["value"].as_str().unwrap(),
            expected_value,
            "G1 (ARG-001 P1 Lafont confluence) FAILED for W={w}: distributed != sequential"
        );
    }
}
```

(The exact `GridConfig::new_in_process(...)` constructor name follows the existing `horner_distributed_g1.rs` patterns; the developer matches the file's conventions.)

**Expected output:** Sequential and all 4 distributed runs decode to `"35"`.

**Edge cases:**
- (EC-1) If any single `W` value diverges, the panic message MUST cite ARG-001 P1 explicitly so the failure narrative writes itself (per TASK-0725 Notes).
- (EC-2) `W == 1` is a degenerate case (single worker = essentially sequential). It is still asserted so the test loop is uniform.
- (EC-3) Wall-time: each W run reduces ~500-2000 agents; total ~1-3 s. Acceptable in debug.

---

## Regression-Tightening

### RT-0725-01: PT-0715-06 final threshold tightening to ≤ 5%

**Location:** `relativist-core/src/encoding/horner.rs:682` (modify).

**Change:**
```rust
// after (TASK-0725):
let max_skips = total / 20;  // 5% — was 25% (TASK-0724); was 50% (TASK-0723); was 95% (HEAD)
assert!(
    skips <= max_skips,
    "PT-0715-06 readback skip rate too high: {skips}/{total} (> 5%) \
     — target after TASK-0723+TASK-0724 is 0%; 5% is the final regression gate \
     (TASK-0725 RT-0725-01 set this threshold)"
);
```

**Expected output:** Pass with empirical skip count ~0/1000 (or, if the developer chose a non-zero implementation, ≤ 50/1000).

**Note:** The comment in the test body that cites TASK-0721 SF-004 (the original guard) should be augmented to mention TASK-0725 as the final-threshold-setter and TASK-0723/0724 as the enablers (per TASK-0725 AC bullet 4).

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-001 | Slice A: dense small `(c_i in 0..=20, x in 0..=20)` | 50/200 cases, all match oracle | PT-0725-A |
| EC-002 | Slice A includes `c1 == 0` (mul-by-zero collapse) | Decodes; result `c0` | PT-0725-A boundary |
| EC-003 | Slice A includes `c1 == 1` (fast-path regression check) | Decodes via pre-TASK-0723 path | PT-0725-A boundary |
| EC-004 | Slice B: degree-3 `(c_i in 0..=50, x in 0..=50)` | 50/200 cases, all match oracle | PT-0725-B |
| EC-005 | Slice C: boundary `(c_i in 9990..=10000, x in 9990..=10000)` | 25/200 cases, all match oracle | PT-0725-C |
| EC-006 | T9 deterministic witness `[1; 25] @ 10` | Decodes to `"1111111111111111111111111"` | UT-0725-D |
| EC-007 | G1 degree-3 `[3,2,5,1]@2` across `W ∈ {1, 2, 4, 8}` | All decode to `"35"` | UT-0725-E |
| EC-008 | PT-0715-06 skip rate on `(1..=10)^3` grid | `≤ 5%` | RT-0725-01 |

## Mapping to SPEC-27 v3 §7

| SPEC-27 v3 Test ID | Coverage |
|---|---|
| T7 (pipeline parity) | PT-0725-{A,B,C}, UT-0725-D |
| T9 (BigUint witness) | UT-0725-D (deterministic from IT file) |
| T11 (oracle cross-check at full bounds) | PT-0725-{A,B,C} (200 release / 50-25 debug cases each) |
| T13 (in-process distributed equivalence at degree >= 2) | UT-0725-E (canonical degree-3 with W ∈ {1, 2, 4, 8}) |
| G1 (Fundamental Property — ARG-001 P1) | UT-0725-E (empirical witness) |

## Dependencies Context

- TASK-0723 + TASK-0724 production code MUST be in HEAD.
- `horner_serial` (TASK-0713) — oracle.
- `run_grid`, `GridConfig::new_in_process(...)`, `ContiguousIdStrategy` (existing in `relativist-core`).
- `pipeline_value` helper — created in this TASK as part of HELPER-0725.

## Notes

- This TASK does NOT add production code. If ANY of PT-0725-{A,B,C} or UT-0725-{D,E} fails on landing, it triggers a Stage 5 QA escalation against TASK-0723 or TASK-0724 (whichever is implicated by the failure). PT-0725-{A,B,C} are the regression safety net.
- The `pipeline_value` helper (HELPER-0725) may need `decode_biguint` re-exported from `relativist-core/src/lib.rs` if it is not currently public. The developer adds the re-export (`pub use encoding::biguint_readback::decode_biguint;` or similar) as part of this TASK if needed. Note this in the commit message.
- UT-0725-E's panic message MUST cite ARG-001 P1 explicitly — see TASK-0725 Notes ("the failure narrative writes itself"). If the developer omits it, REVIEWER catches it in Stage 4.
- PT-0715-06's threshold drop to 5% (RT-0725-01) is the **final** value. After D-016 closes, this threshold should remain at 5% unless a future readback regression is intentional.
- Test floor delta: **+5** (PT A-C, UT D-E). RT-0725-01 is a threshold edit, no count delta.
- Surprising edge case for the developer: **Slice C (boundary near cap) with 200 release cases takes ~15-30 minutes wall-time**. Acceptable per TASK-0725 Notes but flag in commit. If CI cannot tolerate, drop release cases to 100 (still well above the 50-case AC floor); do NOT skip Slice C entirely.
- Surprising edge case #2: **`pipeline_value` propagates `Err` as `String`, NOT as the original `DecodeError`** — this lets the helper handle both encoder and decoder errors uniformly. The proptest assertions use `unwrap_or_else(|e| panic!(...))`, so the original error variant is preserved in the panic message via `Debug`.
