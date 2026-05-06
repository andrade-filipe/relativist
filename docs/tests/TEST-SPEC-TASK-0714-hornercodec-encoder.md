# TEST-SPEC-TASK-0714: Tests for TASK-0714 — HornerCodec encoder (Horner recurrence + bounds)

**Task:** TASK-0714
**Spec:** SPEC-27 v3
**Bundle:** D-015 (SPEC-27 Encoder/Decoder API + HornerCodec — Topic 2)
**Requirements covered:** R10' (module location `encoding::horner`), R11' (input schema `{coeffs, x}`), R12' (input bounds via `MAX_CHURCH_NAT`), R13' (Horner construction pseudocode), R16' (edge cases — encode side)
**Test IDs (from SPEC-27 v3 §7.3):** T5, T6 (encode-side), T7 (encode + oracle), T8 (encode + oracle), T10 (encode-side rows of negative cross-check).
**Inviolable invariants asserted:** SPEC-01 T1-T7 (validated via `validate_encoded_net` post-encode); R12' single-source-of-truth (`MAX_CHURCH_NAT` referenced, not literal `10_000`).

---

## Scope

Phase 3a deliverable: encoder-only. The `Decoder` and `Codec` impls plus property tests T11 / T9 / T9b BigUint pipeline tests live in TASK-0715 (because they require `decode_biguint` from TASK-0712 + `horner_serial` from TASK-0713).

This TEST-SPEC verifies:
- R11' JSON schema parsing (`{ "coeffs": [u64..], "x": u64 }`).
- R12' bounds enforcement BEFORE any `encode_church_into` call.
- R13' construction matches the pseudocode (manual structural inspection or end-to-end reduction yielding correct value).
- R16' edge cases (empty coeffs, constant polynomial, x=0, all-zero, max boundary, overflow).
- Encoder reduces (via `reduce_all` + `decode_nat`/`decode_biguint`) to the value computed by `horner_serial`.

## Test category & location

| # | Category | Cfg gating | File | LoC |
|---|----------|-----------|------|-----|
| UT-0714-01 | unit (in-module) | none | `relativist-core/src/encoding/horner.rs` | ~25 |
| UT-0714-02 | unit (in-module) | none | same | ~25 |
| UT-0714-03 | unit (in-module) | none | same | ~25 |
| UT-0714-04 | unit (in-module) | none | same | ~25 |
| UT-0714-05 | unit (in-module) | none | same | ~15 |
| UT-0714-06 | unit (in-module) | none | same | ~20 |
| UT-0714-07 | unit (in-module) | none | same | ~20 |
| UT-0714-08 | unit (in-module) | none | same | ~15 |
| UT-0714-09 | unit (in-module) | none | same | ~20 |
| UT-0714-10 | unit (in-module) | none | same | ~25 |
| CT-0714-11 | source inspection (integration) | none | `relativist-core/tests/horner_encoder_constants.rs` | ~15 |

## Test floor delta (from TASK-0714 acceptance criteria)

- default: **+11** → ≥ 1859
- zero-copy: **+11** → ≥ 1903
- streaming-no-recycle: **+11** → ≥ 1850
- release: **+11** → ≥ 1801

---

## Unit Tests

### UT-0714-01: `horner_encode_constant_polynomial_skips_loop` (T5)

**Purpose:** R16' bullet 2 — `coeffs.len() == 1` MUST skip the Horner loop entirely (no `wire_mul_into` / `wire_add_into` calls). Result reduces to `Church(coeffs[0])`.

**Input:**
```rust
let codec = HornerCodec::new();
// case 1: x = 0
let net1 = codec.encode(br#"{"coeffs":[42],"x":0}"#).unwrap();
let mut n1 = net1; reduce_all(&mut n1);
assert_eq!(decode_nat(&n1).unwrap(), 42);

// case 2: x = 7 (constant polynomial is independent of x)
let net2 = codec.encode(br#"{"coeffs":[42],"x":7}"#).unwrap();
let mut n2 = net2; reduce_all(&mut n2);
assert_eq!(decode_nat(&n2).unwrap(), 42);
```

**Expected output:** Both reductions yield Church(42).

**Edge cases:**
- (EC-1) `coeffs == [0]` → reduces to Church(0).
- (EC-2) `coeffs == [10_000]` → reduces to Church(10_000) (boundary).
- (EC-3) Agent-count check: encoded net has agent count consistent with `encode_church_into(coeffs[0])` ALONE (no extra mul/add scaffold). The reference Church(42) net has a deterministic agent count; encoded constant-polynomial net MUST match (within `+1` for `set_root` invariant if applicable). This catches a regression where encoder runs the Horner loop with `n = 0` and inserts phantom mul/add agents.

---

### UT-0714-02: `horner_encode_smallest_recurrence` (T6 sequential)

**Purpose:** Smallest non-trivial Horner recurrence — `[1,1,1,1,1] @ 2 = 1+2+4+8+16 = 31`.

**Input:**
```rust
let codec = HornerCodec::new();
let net = codec.encode(br#"{"coeffs":[1,1,1,1,1],"x":2}"#).unwrap();
let mut n = net; reduce_all(&mut n);
assert_eq!(decode_nat(&n).unwrap(), 31);
```

**Expected output:** Reduction yields Church(31).

**Edge cases:**
- (EC-1) `validate_encoded_net(&net)` is `Ok(())` immediately after encode (T1-T7 hold; pre-reduction).
- (EC-2) The encoded net has at least one redex (E2 of R5 encode contract).

---

### UT-0714-03: `horner_encode_canonical_case_matches_oracle` (T7)

**Purpose:** Canonical T7 case — expected value derived via oracle, NOT hard-coded. SC-007 closure: `horner_serial` is the source of truth.

**Input:**
```rust
let codec = HornerCodec::new();
let coeffs = vec![3u64, 2, 5, 1];
let x = 2u64;

// 1. Compute expected via oracle.
let expected = horner_serial(&coeffs, x).unwrap();

// 2. Pipeline.
let json = format!(r#"{{"coeffs":[3,2,5,1],"x":2}}"#);
let net = codec.encode(json.as_bytes()).unwrap();
let mut n = net; reduce_all(&mut n);
let actual_u64 = decode_nat(&n).unwrap();
let actual = BigUint::from(actual_u64);
```

**Expected output:** `actual == expected` (both equal 35 per R11' coefficient ordering).

**Edge cases:**
- (EC-1) Hard-coded check: `actual_u64 == 35` (sanity vs `43`, which would indicate reverse-ordering bug).
- (EC-2) Convention regression: separate test `horner_encode_canonical_case_NOT_43` asserting `actual_u64 != 43` (catches a future re-introduction of the explainer-doc convention drift).

---

### UT-0714-04: `horner_encode_sparse_coefficients_match_oracle` (T8)

**Purpose:** R16' all-zero/sparse — `[1, 0, 0, 0, 0, 1] @ 10 = 100_001`. Reduces correctly via mul-by-zero ⇒ zero, add-with-zero ⇒ identity.

**Input:**
```rust
let codec = HornerCodec::new();
let coeffs = [1u64, 0, 0, 0, 0, 1];
let expected = horner_serial(&coeffs, 10).unwrap();
let net = codec.encode(br#"{"coeffs":[1,0,0,0,0,1],"x":10}"#).unwrap();
let mut n = net; reduce_all(&mut n);
let actual = decode_nat(&n).unwrap();
```

**Expected output:** `BigUint::from(actual) == expected`, i.e., `actual == 100_001`.

**Edge cases:**
- (EC-1) All-zero `[0, 0, 0, 0] @ 7` → `actual == 0` (R16' all-zero edge case).
- (EC-2) Single non-zero in middle `[0, 0, 5, 0, 0] @ 3` → expected `5 * 9 = 45`.

---

### UT-0714-05: `horner_encode_evaluation_at_zero` (R16' x=0)

**Purpose:** R16' bullet 3 — `x == 0` → result is `coeffs[0]`. Tests mul-by-zero collapsing in the reducer.

**Input:**
```rust
let codec = HornerCodec::new();
let net = codec.encode(br#"{"coeffs":[7,99,42],"x":0}"#).unwrap();
let mut n = net; reduce_all(&mut n);
assert_eq!(decode_nat(&n).unwrap(), 7);
```

**Expected output:** Reduces to Church(7).

**Edge cases:**
- (EC-1) `coeffs == [0, 99, 42], x == 0` → `0`.
- (EC-2) `coeffs == [10_000, 5, 7, 9], x == 0` → `10_000` (boundary on coeffs[0]).

---

### UT-0714-06: `horner_encode_empty_coeffs_returns_error` (T10 row 1)

**Purpose:** R16' bullet 1 — empty coeffs → `EncodeError::InvalidInput("empty coeffs")`. Negative cross-check: oracle returns `OracleError::EmptyCoeffs` on the same input.

**Input:**
```rust
let codec = HornerCodec::new();
let r = codec.encode(br#"{"coeffs":[],"x":0}"#);
let oracle_r = horner_serial(&[], 0);
```

**Expected output:**
```rust
match r {
    Err(EncodeError::InvalidInput(msg)) => assert!(msg.to_lowercase().contains("empty")),
    other => panic!("expected InvalidInput, got {:?}", other),
}
assert_eq!(oracle_r, Err(OracleError::EmptyCoeffs));
```

**Edge cases:**
- (EC-1) Empty coeffs WITH `x > 10_000`: encoder still returns `InvalidInput("empty coeffs")` — empty check first; matches oracle ordering.

---

### UT-0714-07: `horner_encode_coefficient_overflow_returns_error` (T10 row 3)

**Purpose:** R12' / R16' — any `coeffs[i] > MAX_CHURCH_NAT` → `EncodeError::InvalidInput`. Negative cross-check vs oracle.

**Input:**
```rust
let codec = HornerCodec::new();
let r = codec.encode(br#"{"coeffs":[10001],"x":0}"#);
let oracle_r = horner_serial(&[10_001], 0);
```

**Expected output:**
```rust
assert!(matches!(r, Err(EncodeError::InvalidInput(_))));
assert_eq!(
    oracle_r,
    Err(OracleError::CoefficientOverflow { idx: 0, value: 10_001, max: 10_000 })
);
```

**Edge cases:**
- (EC-1) Mid-array offending coeff: `[1, 2, 99_999, 4]` → encoder rejects; oracle reports `idx == 2`.
- (EC-2) Boundary inclusive: `[10_000] @ 0` → encoder returns `Ok(net)` (not Err).

---

### UT-0714-08: `horner_encode_x_overflow_returns_error` (T10 row 4)

**Purpose:** R12' / R16' — `x > MAX_CHURCH_NAT` → `EncodeError::InvalidInput`. Negative cross-check vs oracle.

**Input:**
```rust
let codec = HornerCodec::new();
let r = codec.encode(br#"{"coeffs":[1],"x":10001}"#);
let oracle_r = horner_serial(&[1], 10_001);
```

**Expected output:**
```rust
assert!(matches!(r, Err(EncodeError::InvalidInput(_))));
assert_eq!(
    oracle_r,
    Err(OracleError::XOverflow { value: 10_001, max: 10_000 })
);
```

**Edge cases:**
- (EC-1) `x = u64::MAX` → encoder rejects; oracle reports `value: u64::MAX`.
- (EC-2) Boundary inclusive: `x == 10_000` → encoder accepts.

---

### UT-0714-09: `horner_encode_boundary_max_accepted` (T10 row 2)

**Purpose:** R16' bullet 5/7 — `coeffs[i] == 10_000` AND `x == 10_000` MUST both be accepted (boundaries inclusive).

**Input:**
```rust
let codec = HornerCodec::new();
let r1 = codec.encode(br#"{"coeffs":[10000],"x":10000}"#);
let r2 = codec.encode(br#"{"coeffs":[10000,10000,10000],"x":10000}"#);
```

**Expected output:** Both `r1` and `r2` are `Ok(_)` and pass `validate_encoded_net`.

**Edge cases:**
- (EC-1) Reduction of `[10_000] @ 10_000` yields Church(10_000) (constant polynomial; x ignored).
- (EC-2) Reduction of `[10_000, 10_000, 10_000] @ 10_000` matches `horner_serial`.

---

### UT-0714-10: `horner_encode_post_encode_validate_t1_t7`

**Purpose:** R5 (encode contract validation) — every `encode` output MUST satisfy T1-T7 + at least one redex (E1, E2). This is partially overlapping with R5 enforcement at registry level (TASK-0717), but verified directly here on the encoder output.

**Input:**
```rust
let codec = HornerCodec::new();
let inputs = [
    br#"{"coeffs":[42],"x":7}"# as &[u8],
    br#"{"coeffs":[1,1,1,1,1],"x":2}"#,
    br#"{"coeffs":[3,2,5,1],"x":2}"#,
    br#"{"coeffs":[1,0,0,0,0,1],"x":10}"#,
];

for input in inputs {
    let net = codec.encode(input).unwrap();
    validate_encoded_net(&net).expect("encode output MUST satisfy T1-T7");
    // E2: at least one redex (count via TASK-0709 helper or net.redex_queue.len() pre-pruning).
    let valid_redexes = count_valid_active_pairs(&net);
    let queue_len = net.redex_queue.len();
    assert!(valid_redexes > 0 || queue_len > 0, "E2: encode output MUST have at least one redex");
}
```

**Expected output:** All 4 inputs pass validation AND have at least one redex.

**Edge cases:**
- (EC-1) Constant polynomial `[42] @ 0` — note: a Church-numeral construction may have zero redexes if the construction yields a normal form directly. R5 E2 may NOT hold in this case; if so, R5 E2 is enforced **only when `coeffs.len() > 1`**. Stage 4 reviewer agent confirms; if the constant polynomial path yields an NF net, the registry-level R5 validation either accepts it (since reduction trivially completes) or this test is updated to skip the E2 assertion for `coeffs.len() == 1`. **Decision:** test asserts `(valid_redexes > 0 || queue_len > 0) || coeffs_len_eq_1`.

---

## Compile-time / Source Inspection Tests

### CT-0714-11: `horner_encoder_uses_max_church_nat_constant`

**Purpose:** R12' single-source-of-truth in encoder. Mirrors CT-0713-09 for the encoder file.

**Approach:** Source-inspection integration test under `relativist-core/tests/horner_encoder_constants.rs`:
```rust
#[test]
fn horner_encoder_does_not_hardcode_cap() {
    let src = include_str!("../src/encoding/horner.rs");
    let code_only: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("///") && !l.trim_start().starts_with("//!"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !code_only.contains("10_000") && !code_only.contains("10000"),
        "HornerCodec::encode MUST source the cap from MAX_CHURCH_NAT (R12' single source of truth)"
    );
    assert!(
        code_only.contains("MAX_CHURCH_NAT"),
        "HornerCodec::encode MUST reference MAX_CHURCH_NAT"
    );
}
```

**Expected output:** Pass iff encoder code references `MAX_CHURCH_NAT` and contains no `10_000` literal.

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-001 | Constant polynomial skip Horner loop | Yields Church(coeffs[0]); no mul/add agents | UT-0714-01 |
| EC-002 | Smallest non-trivial recurrence `[1;5] @ 2` | Reduces to Church(31) | UT-0714-02 |
| EC-003 | Canonical case `[3,2,5,1] @ 2` | Equals `horner_serial`; equals 35; NOT 43 | UT-0714-03 |
| EC-004 | Sparse coeffs `[1,0,0,0,0,1] @ 10` | Reduces to Church(100001) | UT-0714-04 |
| EC-005 | All-zero coeffs `[0,0,0,0] @ 7` | Reduces to Church(0) | UT-0714-04 EC-1 |
| EC-006 | `x = 0` non-trivial coeffs | Reduces to Church(coeffs[0]) | UT-0714-05 |
| EC-007 | Empty coeffs `[] @ 0` | `EncodeError::InvalidInput("empty coeffs")` | UT-0714-06 |
| EC-008 | Coeff overflow `[10001] @ 0` | `EncodeError::InvalidInput`; oracle: `CoefficientOverflow{idx:0,...}` | UT-0714-07 |
| EC-009 | X overflow `[1] @ 10001` | `EncodeError::InvalidInput`; oracle: `XOverflow{...}` | UT-0714-08 |
| EC-010 | Boundary `[10000] @ 10000` | Accepted; `validate_encoded_net` Ok | UT-0714-09 |
| EC-011 | Multi-coeff boundary `[10000,10000,10000] @ 10000` | Accepted; matches oracle | UT-0714-09 EC-2 |
| EC-012 | Encode output T1-T7 | `validate_encoded_net` Ok for all valid inputs | UT-0714-10 |
| EC-013 | No `10_000` literal in encoder source | Source inspection passes | CT-0714-11 |

## Mapping to SPEC-27 v3 §7

| SPEC-27 v3 Test ID | Coverage |
|---|---|
| T5 (constant polynomial) | UT-0714-01 |
| T6 (smallest recurrence, sequential `W=1`) | UT-0714-02 (sequential baseline; `W∈{2,4,8}` lives in TASK-0715/T13) |
| T7 (canonical Horner case via oracle) | UT-0714-03 |
| T8 (sparse coeffs via oracle) | UT-0714-04 |
| T10 (negative cross-check rows) | UT-0714-06 (empty), UT-0714-07 (coeff overflow), UT-0714-08 (x overflow), UT-0714-09 (boundary acceptance) |
| T11 (property test ≥100 cases) | TASK-0715 (encoder + decoder pipeline); foundation here |

## Dependencies Context

- `wire_add_into`, `wire_mul_into` from `arithmetic.rs` (validated by TASK-0711).
- `encode_church_into` from `church.rs` (SPEC-14 R4b).
- `MAX_CHURCH_NAT` from `church.rs` (added in TASK-0713).
- `horner_serial` from `horner_oracle.rs` (TASK-0713) — used in UT-0714-{03, 04, 06, 07, 08, 09} for cross-check.
- `decode_nat` from `church.rs` (SPEC-14 §4.4) — used to decode the reduced net for `u64`-range tests.
- `count_valid_active_pairs` from TASK-0709 — used in UT-0714-10.
- `validate_encoded_net` from `traits.rs:84` — used in UT-0714-{02, 09, 10}.
- `serde_json` for JSON parsing.

## Notes

- This task ships **only the `Encoder` impl**. `Decoder` and `Codec` impls (and the property tests T11) live in TASK-0715. Until TASK-0715 lands, `HornerCodec` is NOT registrable in `EncoderRegistry`.
- HornerCodec is NOT a `RecipeEncoder` (Q4 in SPEC-27 v3 §8). The fallback to centralized partition (R25) is implicit; no extra code in this task; verified in TASK-0719.
- T9 / T9b BigUint pipeline tests live in TASK-0715 because they need `decode_biguint` (TASK-0712). UT-0714-{02..09} use `decode_nat` (which suffices for `u64` range).
- Test floor delta: **+11** total (10 unit in-module + 1 integration source-inspection).
