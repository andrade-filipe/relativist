# TEST-SPEC-TASK-0724: Tests for TASK-0724 — `biguint_readback` nested Horner (degree >= 2)

**Task:** TASK-0724
**Spec:** SPEC-27 v3 (`specs/SPEC-27-encoder-decoder-api.md`)
**Bundle:** D-016 — HornerCodec decoder extension
**Requirements covered:** R14' (BigUint readback recursion), R16b' (cross-check vs oracle), G1 evidence for non-trivial polynomials.
**Test IDs (from SPEC-27 v3 §7.3):** T7 (pipeline parity), T9 (BigUint witness — **promoted** from "encode + validate only" to full pipeline), T9b (boundary BigUint), T11 (oracle cross-check), T12 (cross-readback).
**Inviolable invariants asserted:** SPEC-01 T1-T7 (encoder unchanged); SPEC-14 §4.4 self-loop frame still detected; R14' Independence still holds (no new `decode_nat` call).
**Production code under test:** `relativist-core/src/encoding/biguint_readback.rs` (helpers from TASK-0723 extended), plus the new helper (illustrative: `chain_via_nested_horner`).

---

## Scope

After TASK-0723, single-iteration Horner outputs decode for any `(c_0, c_1, x)` in bounds. TASK-0724 extends that to **arbitrary degree** — `coeffs.len() >= 2`. The traversal must recurse through `n - 1` nested mul + add scaffolds (one per Horner iteration), and the recursion guard (`depth > 64` currently in `count_chain_through_dups` and `chain_from_dup_branch`) must be reviewed / raised / replaced.

This TEST-SPEC also **promotes** several existing TASK-0715 tests from "encode + validate only" to **full pipeline cross-check**: UT-0715-02 (sparse), UT-0715-03 (T9 25-coeff witness), UT-0715-04 (T9b boundary). Until TASK-0724, those tests assert only that the **encoder** produces a valid net — the readback panics. After TASK-0724, the asserts must compare decoded values against the oracle.

PT-0715-06 input domain is widened from `coeffs.len() in 2..=2` to `2..=4`, and the skip-rate threshold tightens to **≤ 25%** (final tightening to ≤ 5% is reserved for TASK-0725).

## Test category & location

| # | Category | Cfg gating | File | LoC |
|---|----------|-----------|------|-----|
| UT-0724-01 | unit (in-module) | none | `relativist-core/src/encoding/biguint_readback.rs` | ~20 |
| UT-0724-02 | unit (in-module) | none | same | ~20 |
| UT-0724-03 | unit (in-module) | none | same | ~25 |
| UT-0724-04 | unit (in-module) | none | same | ~25 |
| UT-0724-05 | unit (in-module) | none | same | ~30 |
| PT-0724-06 | property test (proptest) | none | same | ~40 |
| PT-0724-07 | unit (recursion-depth) | none | same | ~30 |
| RT-0724-08 | promotion of UT-0715-02 (sparse) | none | `relativist-core/src/encoding/horner.rs:504` (modify) | ~10 |
| RT-0724-09 | promotion of UT-0715-03 (T9 witness) | none | `relativist-core/src/encoding/horner.rs:534` (modify) | ~10 |
| RT-0724-10 | promotion of UT-0715-04 (T9b boundary) | none | `relativist-core/src/encoding/horner.rs:549` (modify) | ~10 |
| RT-0724-11 | PT-0715-06 widen + tighten | none | `relativist-core/src/encoding/horner.rs:607` (modify) | ~10 |
| RT-0724-12 | optional widening of `horner_distributed_g1.rs` | none | `relativist-core/tests/horner_distributed_g1.rs` (modify) | ~25 |

## Test floor delta (from TASK-0724 acceptance criteria)

- default: **+7 new tests** (UT 01-05, PT 06-07) + 0 net delta from RT-0724-{08..12} (promotions and threshold edits, not new tests)
- Expected post-TASK: default ≥ 1814, zero-copy ≥ 1858, streaming-no-recycle ≥ 1805, release ≥ 1756

---

## Unit Tests

### UT-0724-01: `decode_biguint_handles_degree_2_dense`

**Purpose:** Closes Demo 4. Smallest degree-2 polynomial — `[1, 1, 1] @ 2 = 7`. Forces ONE recursive descent into a nested mul scaffold.

**Input:**
```rust
let codec = HornerCodec::new();
let mut net = codec.encode(br#"{"coeffs":[1,1,1],"x":2}"#).unwrap();
reduce_all(&mut net);
if net.root.is_none() {
    crate::encoding::arithmetic::discover_root(&mut net);
}
let result = decode_biguint(&net);
```

**Expected output:**
```rust
assert_eq!(result, Ok(BigUint::from(7u64)));
```

**Edge cases:**
- (EC-1) Oracle parity: `horner_serial(&[1,1,1], 2).unwrap() == 7`.
- (EC-2) `result.unwrap().bits() == 3` (since `7 = 0b111`).

---

### UT-0724-02: `decode_biguint_handles_degree_2_sparse_zero_middle`

**Purpose:** Closes Demo 5. Sparse degree-2 polynomial with middle-zero coefficient — `[1, 0, 1] @ 3 = 10`. Verifies the recursion crosses the `c_1 == 0` accumulator (which the encoder still emits as a `wire_add_into(Church(0), ...)` scaffold — NOT optimised away).

**Input:**
```rust
let codec = HornerCodec::new();
let mut net = codec.encode(br#"{"coeffs":[1,0,1],"x":3}"#).unwrap();
reduce_all(&mut net);
if net.root.is_none() {
    crate::encoding::arithmetic::discover_root(&mut net);
}
let result = decode_biguint(&net);
```

**Expected output:**
```rust
assert_eq!(result, Ok(BigUint::from(10u64)));
```

**Edge cases:**
- (EC-1) `[0, 1, 0] @ 5` → `Ok(5)` (zero leading, non-zero middle, zero trailing — note: trailing zero forces multiplication by zero inside the recurrence, exercising the `Church(0) * x = Church(0)` reduction path).
- (EC-2) `[1, 0, 0] @ 4` → `Ok(1)` (all higher-order terms multiply out; recursion must collapse to the constant).

---

### UT-0724-03: `decode_biguint_handles_degree_3_canonical`

**Purpose:** Canonical degree-3 case `[3, 2, 5, 1] @ 2 = 35`. Matches `horner_encode_canonical_case_matches_oracle` (UT-0714-03) — same expected value, but now compared at the readback layer, NOT just the encoder.

**Input:**
```rust
let codec = HornerCodec::new();
let mut net = codec.encode(br#"{"coeffs":[3,2,5,1],"x":2}"#).unwrap();
reduce_all(&mut net);
if net.root.is_none() {
    crate::encoding::arithmetic::discover_root(&mut net);
}
let result = decode_biguint(&net);
```

**Expected output:**
```rust
assert_eq!(result, Ok(BigUint::from(35u64)));
// Convention-regression check: MUST NOT be 43 (reverse-ordering bug).
assert_ne!(result.as_ref().unwrap(), &BigUint::from(43u64));
```

**Edge cases:**
- (EC-1) Oracle parity: `horner_serial(&[3,2,5,1], 2).unwrap() == 35`.
- (EC-2) Permutation `[1, 5, 2, 3] @ 2` → result differs from 35 (sanity for coefficient-ordering).

---

### UT-0724-04: `decode_biguint_handles_degree_5_sparse`

**Purpose:** Degree-5 sparse — `[1, 0, 0, 0, 0, 1] @ 10 = 100001`. Maximally stresses the recursion through 4 zero-middle iterations. Closes the existing UT-0714-04 / UT-0715-02 "encode + validate only" gating.

**Input:**
```rust
let codec = HornerCodec::new();
let mut net = codec.encode(br#"{"coeffs":[1,0,0,0,0,1],"x":10}"#).unwrap();
reduce_all(&mut net);
if net.root.is_none() {
    crate::encoding::arithmetic::discover_root(&mut net);
}
let result = decode_biguint(&net);
```

**Expected output:**
```rust
assert_eq!(result, Ok(BigUint::from(100_001u64)));
```

**Edge cases:**
- (EC-1) `result.unwrap().bits() == 17` (since `100001 = 0b11000011010100001`).
- (EC-2) Variant `[5, 0, 0, 0, 0, 5] @ 10` → `Ok(BigUint::from(500_005u64))`.
- (EC-3) Wall-time note: reduces ~5-10k agents; ~1-3 s in debug. Acceptable.

---

### UT-0724-05: `decode_biguint_t9_biguint_witness_full_pipeline`

**Purpose:** T9 BigUint witness — `[1; 25] @ 10 = "1111111111111111111111111"` (25 ones, decimal). The result exceeds `u64::MAX` (`bits() == 83` — well above 64). This is the **headline G1 witness** that TASK-0724 unlocks; PT-0715-03 in `horner.rs:534` currently gates on "encode + validate only" and MUST be promoted to full pipeline.

**Input:**
```rust
use num_bigint::BigUint;

let codec = HornerCodec::new();
let coeffs_json = br#"{"coeffs":[1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1],"x":10}"#;
let mut net = codec.encode(coeffs_json).unwrap();
reduce_all(&mut net);
if net.root.is_none() {
    crate::encoding::arithmetic::discover_root(&mut net);
}
let result = decode_biguint(&net);
```

**Expected output:**
```rust
let expected = BigUint::parse_bytes(b"1111111111111111111111111", 10).unwrap();
assert_eq!(result, Ok(expected.clone()));
assert!(expected.bits() > 64, "T9 witness: MUST exceed u64::MAX");
```

**Edge cases:**
- (EC-1) Oracle parity: `horner_serial(&[1; 25], 10).unwrap() == expected`.
- (EC-2) `expected.bits() == 84` (decimal length 25, log2(10^25) ≈ 83.05).
- (EC-3) Wall-time: this is the heaviest deterministic UT in D-016 — reduces ~50-200k agents; ~10-60 s in debug, ~2-15 s in release. Document; do NOT gate behind `cfg(release)` (T9 is a SPEC-required witness).

---

## Property Tests

### PT-0724-06: `decode_biguint_matches_oracle_degree_2_to_4_property`

**Property:** For every `(coeffs, x)` with `coeffs.len() in 2..=4`, each `coeffs[i] in 0..=20`, `x in 0..=20`, `decode_biguint(encode + reduce) == horner_serial(&coeffs, x)`. No `Err` is acceptable.

**Generator strategy:**
```text
coeffs: vec of length 2..=4, each element 0u64..=20
x:      0u64..=20
```

**Assertion:**
```rust
proptest! {
    #![proptest_config(ProptestConfig { cases: 200, .. ProptestConfig::default() })]
    #[test]
    fn decode_biguint_matches_oracle_degree_2_to_4(
        coeffs in proptest::collection::vec(0u64..=20, 2..=4),
        x in 0u64..=20,
    ) {
        let expected = horner_serial(&coeffs, x).unwrap();
        let codec = HornerCodec::new();
        let json_obj = serde_json::json!({"coeffs": coeffs, "x": x});
        let bytes = serde_json::to_vec(&json_obj).unwrap();
        let mut net = codec.encode(&bytes).expect("valid input encodes");
        reduce_all(&mut net);
        if net.root.is_none() {
            crate::encoding::arithmetic::discover_root(&mut net);
        }
        let actual = decode_biguint(&net)
            .unwrap_or_else(|e| panic!("coeffs={coeffs:?} x={x}: {e:?}"));
        prop_assert_eq!(actual, expected);
    }
}
```

**Sample size:** 200 internal cases.

**Shrinking note:** On failure, proptest shrinks toward the minimal `(coeffs, x)` showing divergence — most likely `coeffs = [0, 0, 1]` or similar. The shrunk counterexample tells the developer whether the recursion depth (`coeffs.len()`) or the coefficient magnitudes are the root cause.

**Performance note:** `coeffs.len() == 4`, all coefficients == 20, `x == 20` produces a reduced net of ~10-20k agents. With 200 cases the proptest takes ~3-10 min in debug. If wall-time is prohibitive, cap `cases: 50` in debug via `if cfg!(debug_assertions) { 50 } else { 200 }`.

---

### PT-0724-07: `decode_biguint_recursion_depth_64_no_overflow`

**Purpose:** Recursion-depth guard verification. Forces the readback to descend through 63 nested mul+add scaffolds. MUST NOT stack-overflow; MUST decode to the correct geometric sum.

**Input:**
```rust
let codec = HornerCodec::new();
let coeffs = vec![1u64; 64];   // 64 ones
let x = 2u64;
let json_obj = serde_json::json!({"coeffs": coeffs, "x": x});
let bytes = serde_json::to_vec(&json_obj).unwrap();
let mut net = codec.encode(&bytes).unwrap();
reduce_all(&mut net);
if net.root.is_none() {
    crate::encoding::arithmetic::discover_root(&mut net);
}
let result = decode_biguint(&net);
```

**Expected output:**
```rust
// Geometric sum: 1 + 2 + 4 + ... + 2^63 = 2^64 - 1.
let expected = (BigUint::from(1u64) << 64u32) - BigUint::from(1u64);
assert_eq!(result, Ok(expected));
```

**Edge cases:**
- (EC-1) The existing `if depth > 64` guards in `count_chain_through_dups` (line 169) and `chain_from_dup_branch` (line 258) MUST be raised to at least `>= 128` or switched to an explicit `Vec` work-stack. Document the chosen approach in the helper's rustdoc.
- (EC-2) Wall-time: ~5-30 s in debug. This is acceptable — recursion depth is the safety property under test.
- (EC-3) Variant `[1; 128] @ 2` is intentionally OUT of scope (would force a second depth-guard bump and explode reduction time); the developer notes it as a future deferral if 64 holds.

---

## Promotions of existing TASK-0715 tests

The following are EDITS to existing tests in `horner.rs`. They do NOT increase the test count; they tighten the assertions from "encode + validate only" (current gating) to "full pipeline matches oracle".

### RT-0724-08: Promote `horner_decode_sparse_coefficients_match_oracle` (UT-0715-02)

**Location:** `relativist-core/src/encoding/horner.rs:504`.

**Change:** Replace the `match pipeline(...)` arm that currently accepts `Err(DecodeError::UnrecognizedStructure(_))` as expected. After TASK-0724 the multi-iteration sparse case MUST decode to `100001`. The block becomes:
```rust
// after TASK-0724:
let (v, bl) = pipeline(br#"{"coeffs":[1,0,0,0,0,1],"x":10}"#)
    .expect("post-TASK-0724: nested Horner decodes");
assert_eq!(v, "100001");
assert_eq!(bl, 17);
```

**Expected output:** Pass. If the recursion guard or the helper fails on degree-5 sparse, this test surfaces the regression at the same time as UT-0724-04 (intentional double-coverage at module + pipeline layers).

---

### RT-0724-09: Promote `horner_pipeline_biguint_range_25_coeffs` (UT-0715-03 / T9)

**Location:** `relativist-core/src/encoding/horner.rs:534`.

**Change:** Add the full pipeline cross-check after the existing encode-side validation:
```rust
// after TASK-0724 — add at end of the test:
let (v, bl) = pipeline(br#"{"coeffs":[1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1],"x":10}"#)
    .expect("post-TASK-0724: T9 BigUint witness decodes");
assert_eq!(v, "1111111111111111111111111");
assert!(bl > 64, "T9: bit_length must exceed u64::MAX");
assert_eq!(bl, 84);  // ceil(log2(10^25)) ≈ 84
```

**Expected output:** Pass. This is the canonical T9 BigUint witness that ARG-001 cites.

---

### RT-0724-10: Promote `horner_pipeline_boundary_max_inputs` (UT-0715-04 / T9b)

**Location:** `relativist-core/src/encoding/horner.rs:549`.

**Change:** Add the full pipeline cross-check:
```rust
// after TASK-0724 — add at end of the test:
let (v, bl) = pipeline(br#"{"coeffs":[10000,10000,10000,10000,10000],"x":10000}"#)
    .expect("post-TASK-0724: T9b boundary BigUint decodes");
let oracle = horner_serial(&[10_000; 5], 10_000).unwrap();
assert_eq!(v, oracle.to_string());
assert_eq!(bl, oracle.bits());
```

**Performance note:** This is the heaviest boundary case in D-016. The reduction produces ~500k-1M agents. Wall-time in debug may exceed 60 s. If CI cannot tolerate this, gate the **promoted** assertion (NOT the encode-side check) behind `if cfg!(not(debug_assertions))` so debug runs only validate the encoder and release runs validate end-to-end. Document the choice; do NOT silently weaken.

---

### RT-0724-11: PT-0715-06 widen domain + tighten threshold to ≤ 25%

**Location:** `relativist-core/src/encoding/horner.rs:607` (proptest input domain), `:682` (threshold).

**Change in proptest input domain (~5 LoC):**
```rust
// before:
coeffs in proptest::collection::vec(1u64..=10u64, 2..=2),
// after (TASK-0724):
coeffs in proptest::collection::vec(0u64..=10u64, 2..=4),  // widen to degree 3
```

**Change in `pt_0715_06_skip_rate_is_bounded` threshold (~5 LoC):**
```rust
// after (TASK-0724):
let max_skips = (total * 25) / 100;  // 25% — was 50% (TASK-0723); was 95% (HEAD)
assert!(
    skips <= max_skips,
    "PT-0715-06 readback skip rate too high: {skips}/{total} (> 25%) \
     — possible regression in nested-Horner path (biguint_readback); \
     TASK-0725 will tighten to 5%"
);
```

**Expected output:** Pass. If skip rate exceeds 25% on the `(1..=10)^3` grid (note: grid is still `coeffs.len() == 2`; the proptest widening is independent), the cofactor or recursion path has regressed.

**Note:** The grid loop in `pt_0715_06_skip_rate_is_bounded` (`horner.rs:666`) iterates over `coeffs.len() == 2` only and is NOT widened by this TASK — leaving the deterministic grid narrow keeps wall-time bounded. The proptest (random input) is the place that widens to degree 4.

---

### RT-0724-12: Optionally widen `horner_distributed_g1.rs` (single degree-2 case)

**Location:** `relativist-core/tests/horner_distributed_g1.rs` (inspect first; existing T13 file).

**Change:** If the file currently restricts T13 inputs to `coeffs.len() == 2` (per its lines 26-35 doc-comment), add ONE degree-2 case `{"coeffs":[1,1,1],"x":2}` (smallest, expected 7) that exercises G1 across `W ∈ {1, 2, 4, 8}`. ~20-25 LoC delta.

**Boundary:** Do NOT widen beyond ONE case in this TASK. The full widening (degree 3 with `{"coeffs":[3,2,5,1],"x":2}`) is TASK-0725's job (UT-0725-E). This RT-0724-12 is a sanity check that the T13 test still compiles after TASK-0724's helper changes.

**Expected output:** Each `W ∈ {1, 2, 4, 8}` invocation decodes to `7`.

**Conditional:** If `horner_distributed_g1.rs` already passes for degree-2 inputs after TASK-0723 + TASK-0724 (no code change needed), the developer SKIPS this RT and notes it as "no-op" in the commit. Stage 5 QA confirms.

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-001 | `[1,1,1]@2` (Demo 4) | `Ok(BigUint::from(7))` | UT-0724-01 |
| EC-002 | `[1,0,1]@3` (Demo 5, sparse middle-zero) | `Ok(BigUint::from(10))` | UT-0724-02 |
| EC-003 | `[3,2,5,1]@2` (canonical degree-3) | `Ok(BigUint::from(35))` | UT-0724-03 |
| EC-004 | `[1,0,0,0,0,1]@10` (sparse degree-5) | `Ok(BigUint::from(100001))` | UT-0724-04 |
| EC-005 | `[1; 25]@10` (T9 BigUint witness) | `Ok("1111111111111111111111111")`; `bits() > 64` | UT-0724-05 |
| EC-006 | Proptest `coeffs.len() in 2..=4, each in 0..=20, x in 0..=20` | 200 cases, no `Err` | PT-0724-06 |
| EC-007 | `[1; 64]@2` recursion depth 63 | `Ok(2^64 - 1)`; no stack overflow | PT-0724-07 |
| EC-008 | `[10000; 5]@10000` (T9b boundary) — promoted full pipeline | matches oracle | RT-0724-10 |
| EC-009 | PT-0715-06 skip rate on `(1..=10)^3` | `≤ 25%` | RT-0724-11 |
| EC-010 | `coeffs.len() in 2..=4` proptest of PT-0715-06 | passes | RT-0724-11 |

## Mapping to SPEC-27 v3 §7

| SPEC-27 v3 Test ID | Coverage |
|---|---|
| T7 (pipeline parity) | UT-0724-{01..05}, RT-0724-{08,09,10} |
| T9 (BigUint witness, 25 coeffs) | UT-0724-05, RT-0724-09 (promotion) |
| T9b (boundary BigUint) | RT-0724-10 (promotion) |
| T11 (oracle cross-check) | PT-0724-06, RT-0724-11 (PT-0715-06 widening) |
| T12 (BigUint readback cross-check) | UT-0724-{01..05} (each compares against oracle directly) |
| T13 (in-process distributed) | RT-0724-12 (single degree-2 case); full widening in TASK-0725 |

## Dependencies Context

- TASK-0723 production code MUST be in HEAD (new helper for c_i >= 2 single-iteration).
- `horner_serial` (TASK-0713) — oracle.
- `reduce_all`, `discover_root` (TASK-0721 BUG-001).
- `num_bigint::BigUint` (TASK-0712).
- Horner encoder loop `relativist-core/src/encoding/horner.rs:162-186` — structural blueprint for the recursive readback.

## Notes

- The `Topology relationship to decode_nat` rustdoc block in `biguint_readback.rs` lines 8-19 (currently still says "Future Mackie/Pinto-style readback would close this gap") MUST be updated by the developer in this TASK — strike the sentence or reframe to "WAN-scale Mackie/Pinto would replace this recursive readback when bound by network latency, but is not required for HornerCodec correctness". This is a doc edit, NOT a test, but if the developer forgets, the doc lies. Recommend an inline reminder in the helper rustdoc.
- The recursion-depth guard decision (raise to 128, raise to 256, or switch to explicit `Vec` work-stack) is left to the developer. PT-0724-07 with `coeffs.len() == 64` forces a decision; document it in the helper rustdoc.
- PT-0724-06 must NOT silently skip on `Err`. If the test-generator wishes to allow a documented escape valve, it MUST be a `prop_assume!(false)` with a explanatory log, NOT a silent `unwrap_or_else` returning ok. The whole point of TASK-0724 is to close the readback gap, so any `Err` is a hard failure.
- The RT-0724-12 widening to `horner_distributed_g1.rs` is intentionally a **single case** — TASK-0725 (UT-0725-E) does the full widening with `{"coeffs":[3,2,5,1],"x":2}` and `W ∈ {1, 2, 4, 8}`.
- Test floor delta: **+7** (UT 01-05, PT 06-07). RT-0724-{08..12} are promotions / threshold edits / domain widenings, no count delta.
- Surprising edge case for the developer: **`[10000; 5] @ 10000` is borderline-too-heavy for debug-mode CI**. The promoted RT-0724-10 may need to be gated behind `cfg!(not(debug_assertions))` for the pipeline assertion (encoder validation can stay in debug). The developer must choose explicitly — document in the commit.
