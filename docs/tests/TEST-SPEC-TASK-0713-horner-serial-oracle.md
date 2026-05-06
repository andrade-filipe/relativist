# TEST-SPEC-TASK-0713: Tests for TASK-0713 — `horner_serial` oracle with `OracleError`

**Task:** TASK-0713
**Spec:** SPEC-27 v3
**Bundle:** D-015 (SPEC-27 Encoder/Decoder API + HornerCodec — Topic 2)
**Requirements covered:** R16a' (oracle returns `Result<BigUint, OracleError>`; same input bounds as encoder; single-source-of-truth `MAX_CHURCH_NAT`), R12' (cap inheritance from SPEC-14 R4)
**Test IDs (from SPEC-27 v3 §7.3):** Foundational for T7, T9, T9b, T10 (negative cross-check), T11 (property test).
**Inviolable invariants asserted:** Cap value `MAX_CHURCH_NAT = 10_000` is the single source of truth (no `10_000` literal in oracle code); `OracleError` family matches encoder `EncodeError::InvalidInput` family for the same input.

---

## Scope

Per Round 2 closure SC-007: `horner_serial` returns `Result<BigUint, OracleError>` with three variants (`EmptyCoeffs`, `CoefficientOverflow {idx, value, max}`, `XOverflow {value, max}`). The oracle MUST enforce **the same input bounds as the encoder (R12')**. T7 / T8 / T9 / T9b expected values are computed via this oracle (NOT hard-coded), and T11 negative cross-check (≥30 cases) verifies oracle and encoder reject the same out-of-range inputs with matching error families.

This TEST-SPEC covers the oracle in isolation. Cross-checks against the codec live in TASK-0714/TASK-0715 TEST-SPECs.

## Test category & location

| # | Category | Cfg gating | File | LoC |
|---|----------|-----------|------|-----|
| UT-0713-01 | unit (in-module) | none | `relativist-core/src/encoding/horner_oracle.rs` | ~15 |
| UT-0713-02 | unit (in-module) | none | same | ~15 |
| UT-0713-03 | unit (in-module) | none | same | ~15 |
| UT-0713-04 | unit (in-module) | none | same | ~25 |
| UT-0713-05 | unit (in-module) | none | same | ~25 |
| UT-0713-06 | unit (in-module) | none | same | ~15 |
| UT-0713-07 | unit (in-module) | none | same | ~15 |
| UT-0713-08 | unit (in-module) | none | same | ~15 |
| CT-0713-09 | source inspection (integration) | none | `relativist-core/tests/horner_oracle_constants.rs` | ~20 |

## Test floor delta (from TASK-0713 acceptance criteria)

- default: **+9** → ≥ 1848 (cumulative on top of 0709/0710/0711/0712)
- zero-copy: **+9** → ≥ 1892
- streaming-no-recycle: **+9** → ≥ 1839
- release: **+9** → ≥ 1790

---

## Unit Tests

### UT-0713-01: `horner_serial_constant_polynomial`

**Purpose:** R16' bullet 2 — constant polynomial (`coeffs.len() == 1`) returns `coeffs[0]` regardless of `x`.

**Input:**
```rust
let r1 = horner_serial(&[42], 0).unwrap();
let r2 = horner_serial(&[42], 7).unwrap();
let r3 = horner_serial(&[0], 99).unwrap();
```

**Expected output:**
```rust
assert_eq!(r1, BigUint::from(42u64));
assert_eq!(r2, BigUint::from(42u64));
assert_eq!(r3, BigUint::from(0u64));
```

**Edge cases:**
- (EC-1) `[0] @ 0` → `0` (degenerate but valid).
- (EC-2) `[10_000] @ 0` → `10_000` (boundary inclusive per R12').

---

### UT-0713-02: `horner_serial_canonical_explainer_case`

**Purpose:** T7 expected-value source. With R11' coefficient-ordering (`coeffs[0]` = constant term), `[3, 2, 5, 1] @ 2` = `3 + 2·2 + 5·4 + 1·8 = 35`.

**Input:**
```rust
let r = horner_serial(&[3, 2, 5, 1], 2).unwrap();
```

**Expected output:** `r == BigUint::from(35u64)`.

**Edge cases:**
- (EC-1) Reverse coefficients `[1, 5, 2, 3] @ 2` = `1 + 5·2 + 2·4 + 3·8 = 43` — verifies the explainer-doc value `43` corresponds to the **opposite** ordering convention (sanity: the test-spec authors anchored this to keep R11' pinned).
- (EC-2) Same coefficients with `x = 0` → `coeffs[0] = 3`.

---

### UT-0713-03: `horner_serial_sparse_coefficients`

**Purpose:** T8 expected-value source. `[1, 0, 0, 0, 0, 1] @ 10` = `1 + 0 + 0 + 0 + 0 + 100_000 = 100_001`.

**Input:**
```rust
let r = horner_serial(&[1, 0, 0, 0, 0, 1], 10).unwrap();
```

**Expected output:** `r == BigUint::from(100_001u64)`.

**Edge cases:**
- (EC-1) All zeros except first: `[7, 0, 0, 0] @ 10` = `7`.
- (EC-2) All zeros (handled in T10): `[0, 0, 0, 0] @ 7` = `0`.

---

### UT-0713-04: `horner_serial_biguint_range_25_coeffs`

**Purpose:** T9 — strictly exceeds `u64::MAX`. SC-006: `coeffs.len() == 25`, `x = 10` → result `(10^25 - 1)/9 ≈ 1.11 × 10^24`.

**Input:**
```rust
let coeffs = vec![1u64; 25];
let r = horner_serial(&coeffs, 10).unwrap();
```

**Expected output:**
```rust
let expected_str = "1111111111111111111111111";  // 25 '1's
assert_eq!(r.to_string(), expected_str);
assert!(r.bits() > 64, "T9 BigUint witness: bits must exceed u64");
```

**Edge cases:**
- (EC-1) Verify `r > BigUint::from(u64::MAX)` — strict inequality; SC-006 closure on T9.
- (EC-2) `coeffs.len() == 24` (just below) — result still BigUint per arithmetic, but specifically `(10^24 - 1)/9 ≈ 1.11 × 10^23`. Optional: skip; T9 is the canonical SC-006 witness.

---

### UT-0713-05: `horner_serial_boundary_max_inputs` (T9b)

**Purpose:** T9b — boundary acceptance for max coeff (10_000) AND max x (10_000) with BigUint range.

**Input:**
```rust
let coeffs = vec![10_000u64, 10_000, 10_000, 10_000, 10_000];
let r = horner_serial(&coeffs, 10_000).unwrap();
```

**Expected output:**
```rust
// p(10_000) = 10_000 * (1 + 10_000 + 10_000^2 + 10_000^3 + 10_000^4)
//           = 10_000 * (10_000^5 - 1) / 9_999
// = 10_000 * 11111111111111111111 / ... ≈ 1.000_100_010_001 × 10^20
assert!(r.bits() > 64);
let expected = expected_via_independent_arithmetic();  // computed separately as fixture
assert_eq!(r, expected);
```

**Edge cases:**
- (EC-1) Both boundaries simultaneously (coeff = 10_000 AND x = 10_000) MUST be accepted (R12' inclusive).
- (EC-2) Compare to `[10_000; 5] @ 9_999` (just below x cap) — result still valid; smaller magnitude.

**Note on expected value:** TASK-0713 NOTE recommends sourcing the expected value via direct BigUint arithmetic in the test fixture (NOT a string literal), to keep the test self-checking even if Rust's BigUint impl changes representation.

---

### UT-0713-06: `horner_serial_empty_coeffs_returns_error`

**Purpose:** R16' bullet 1 — empty coeffs → `OracleError::EmptyCoeffs`.

**Input:**
```rust
let r = horner_serial(&[], 0);
let r2 = horner_serial(&[], 99);
```

**Expected output:**
```rust
assert_eq!(r, Err(OracleError::EmptyCoeffs));
assert_eq!(r2, Err(OracleError::EmptyCoeffs));
```

**Edge cases:**
- (EC-1) Empty coeffs error MUST be returned regardless of `x` value (even `x > MAX_CHURCH_NAT`); ordering: empty check BEFORE coefficient bound check, AND BEFORE x bound check.

---

### UT-0713-07: `horner_serial_coefficient_overflow_returns_error`

**Purpose:** R12' / R16' — `coeffs[i] > MAX_CHURCH_NAT` → `OracleError::CoefficientOverflow { idx, value, max }`.

**Input:**
```rust
let r1 = horner_serial(&[10_001], 0);
let r2 = horner_serial(&[1, 2, 99_999, 4], 5);  // overflow at idx 2
```

**Expected output:**
```rust
assert_eq!(r1, Err(OracleError::CoefficientOverflow { idx: 0, value: 10_001, max: 10_000 }));
assert_eq!(r2, Err(OracleError::CoefficientOverflow { idx: 2, value: 99_999, max: 10_000 }));
```

**Edge cases:**
- (EC-1) `[10_000] @ 0` → `Ok(_)` (boundary inclusive).
- (EC-2) Multiple offending coeffs (e.g., `[10_001, 99_999, ...]`) — oracle MUST report the **first** (smallest index); test asserts `idx == 0`.

---

### UT-0713-08: `horner_serial_x_overflow_returns_error`

**Purpose:** R12' / R16' — `x > MAX_CHURCH_NAT` → `OracleError::XOverflow { value, max }`.

**Input:**
```rust
let r1 = horner_serial(&[1], 10_001);
let r2 = horner_serial(&[1], u64::MAX);
```

**Expected output:**
```rust
assert_eq!(r1, Err(OracleError::XOverflow { value: 10_001, max: 10_000 }));
assert_eq!(r2, Err(OracleError::XOverflow { value: u64::MAX, max: 10_000 }));
```

**Edge cases:**
- (EC-1) `[1] @ 10_000` → `Ok(BigUint::from(1u64))` (boundary inclusive).
- (EC-2) Both coeff overflow AND x overflow: `[10_001] @ 10_001` — oracle returns `CoefficientOverflow` (per check order in TASK-0713 NOTE: empty → coeffs → x).

---

## Compile-time / Source Inspection Tests

### CT-0713-09: `horner_serial_uses_max_church_nat_constant`

**Purpose:** R12' / R16a' — single source of truth. Oracle code MUST reference `MAX_CHURCH_NAT` constant, NOT the literal `10_000`.

**Approach:** Source-inspection integration test under `relativist-core/tests/horner_oracle_constants.rs`:
```rust
#[test]
fn horner_oracle_does_not_hardcode_cap() {
    let src = include_str!("../src/encoding/horner_oracle.rs");
    // Filter out rustdoc lines (they may reference 10_000 in examples).
    let code_only: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("///") && !l.trim_start().starts_with("//!"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !code_only.contains("10_000") && !code_only.contains("10000"),
        "horner_serial MUST source the cap from MAX_CHURCH_NAT constant (R12' single source of truth)"
    );
    // Positively assert the constant is referenced.
    assert!(
        code_only.contains("MAX_CHURCH_NAT"),
        "horner_serial MUST reference MAX_CHURCH_NAT (single source of truth)"
    );
}
```

**Expected output:** Test passes iff oracle code does not literal-encode `10_000` AND references `MAX_CHURCH_NAT`.

**Edge cases:**
- (EC-1) Rustdoc references to `10_000` are filtered out (they document the current value but do not constitute a literal in code).

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-001 | Constant polynomial `[42] @ x` | `Ok(42)` for any `x ≤ cap` | UT-0713-01 |
| EC-002 | Canonical Horner `[3,2,5,1] @ 2` | `Ok(35)` per R11' | UT-0713-02 |
| EC-003 | Sparse `[1,0,0,0,0,1] @ 10` | `Ok(100001)` | UT-0713-03 |
| EC-004 | BigUint range `[1;25] @ 10` | `bits() > 64`; SC-006 witness | UT-0713-04 |
| EC-005 | Boundary `[10_000;5] @ 10_000` | `Ok(_)`, `bits() > 64` | UT-0713-05 |
| EC-006 | Empty coeffs `[] @ 0` | `Err(EmptyCoeffs)` | UT-0713-06 |
| EC-007 | Coeff overflow `[10_001] @ 0` | `Err(CoefficientOverflow{idx:0,value:10001,max:10000})` | UT-0713-07 |
| EC-008 | X overflow `[1] @ 10_001` | `Err(XOverflow{value:10001,max:10000})` | UT-0713-08 |
| EC-009 | Boundary `[10_000] @ 0` | `Ok(10_000)` | UT-0713-07 EC-1 |
| EC-010 | Boundary `[1] @ 10_000` | `Ok(1)` | UT-0713-08 EC-1 |
| EC-011 | Multiple coeff overflow | Reports first offending index | UT-0713-07 EC-2 |
| EC-012 | Empty + x overflow combined | `EmptyCoeffs` (empty checked first) | UT-0713-06 EC-1 |
| EC-013 | Cap source-of-truth | No `10_000` literal in oracle code | CT-0713-09 |

## Mapping to SPEC-27 v3 §7

| SPEC-27 v3 Test ID | Coverage |
|---|---|
| T7 (canonical case) | Oracle expected value: UT-0713-02 |
| T8 (sparse) | Oracle expected value: UT-0713-03 |
| T9 (BigUint range, 25 coeffs) | Oracle expected value: UT-0713-04 |
| T9b (boundary `[10000;5] @ 10000`) | Oracle expected value: UT-0713-05 |
| T10 (negative cross-check rows) | Oracle error variants: UT-0713-06, UT-0713-07, UT-0713-08 |
| T11 (property test, ≥30 negative cases) | Foundation; full property test in TASK-0715 |

## Dependencies Context

- `num_bigint::BigUint` (added to `Cargo.toml` by TASK-0712 if not yet present).
- `MAX_CHURCH_NAT` constant defined in this task (consumed by TASK-0714 R12').
- `thiserror` already in `Cargo.toml`.

## Notes

- Pre-loop validation order MUST be: (1) empty coeffs → (2) per-coefficient bound → (3) x bound. This matches the enum variant ordering top-to-bottom and is asserted indirectly by UT-0713-06 EC-1 (empty + x_overflow returns EmptyCoeffs) and UT-0713-08 EC-2 (coeff_overflow + x_overflow returns CoefficientOverflow).
- Constant polynomial fast path (`coeffs.len() == 1`) MUST short-circuit before entering the Horner accumulator loop (TASK-0713 acceptance criterion). UT-0713-01 verifies the result; an additional micro-check could inspect timing or trace, but is OUT OF SCOPE for v3.
- Test floor delta: **+9** total (8 unit in-module + 1 integration source-inspection).
