# TEST-SPEC-TASK-0723: Tests for TASK-0723 — `biguint_readback` cofactor `c_i >= 2` (single-iteration Horner)

**Task:** TASK-0723
**Spec:** SPEC-27 v3 (`specs/SPEC-27-encoder-decoder-api.md`)
**Bundle:** D-016 — HornerCodec decoder extension
**Requirements covered:** R14' (BigUint readback extension), R16b' (cross-check vs oracle), R4 (NotNormalForm semantics — unchanged)
**Test IDs (from SPEC-27 v3 §7.3):** T7 (pipeline parity), T11 (oracle cross-check), T12 (cross-readback). Direct dependency for T13.
**Inviolable invariants asserted:** SPEC-01 T1-T7 (encoder output); SPEC-14 §4.4 `decode_nat` topology preserved for the c_1==1 fast path; R14' Independence clause preserved (no new call into `decode_nat`).
**Production code under test:** `relativist-core/src/encoding/biguint_readback.rs:60` (`decode_biguint`), `:169` (`count_chain_through_dups`), `:258` (`chain_from_dup_branch`), plus the new helper introduced by TASK-0723 (illustrative: `chain_via_mul_subnet`).

---

## Scope

TASK-0723 extends `count_chain_through_dups` / `chain_from_dup_branch` (or adds a third helper, `chain_via_mul_subnet`) so the readback crosses the DUP-share frame that `wire_mul_into` emits when `coeffs[1] >= 2`. After this TASK, the readable subset MUST include **every** single-iteration Horner input (`coeffs.len() == 2`) with `c_0, c_1, x` in `0..=MAX_CHURCH_NAT`.

**Out of scope** (deferred to TASK-0724): degree >= 2. A test that asserts a `coeffs.len() >= 3` input decodes correctly belongs in TEST-SPEC-0724.

This TEST-SPEC also tightens **PT-0715-06** (`pt_0715_06_skip_rate_is_bounded` in `horner.rs:662`) — the threshold drops from `<= 95%` to `<= 50%` on the same `(1..=10) × (1..=10) × (1..=10)` grid. Sharpening to `<= 5%` is reserved for TASK-0725 (after TASK-0724 covers degree >= 2).

## Test category & location

| # | Category | Cfg gating | File | LoC |
|---|----------|-----------|------|-----|
| UT-0723-01 | unit (in-module) | none | `relativist-core/src/encoding/biguint_readback.rs` | ~25 |
| UT-0723-02 | unit (in-module) | none | same | ~40 |
| UT-0723-03 | unit (in-module) | none | same | ~15 |
| UT-0723-04 | unit (in-module) | none | same | ~20 |
| UT-0723-05 | unit (in-module) | none | same | ~25 |
| UT-0723-06 | unit (in-module) | none | same | ~20 |
| UT-0723-07 | unit (in-module) | none | same | ~25 |
| PT-0723-08 | property test (proptest) | none | same | ~35 |
| RT-0723-09 | regression-tightening | none | `relativist-core/src/encoding/horner.rs:662` (modify) | ~5 |
| UT-0723-10 | unit (error-path) | none | `relativist-core/src/encoding/biguint_readback.rs` | ~15 |

## Test floor delta (from TASK-0723 acceptance criteria)

- default: **+9 new tests** (UT 01-07, PT 08, UT 10) plus 0 net delta from RT-0723-09 (threshold edit, not new test)
- Expected post-TASK: default ≥ 1807, zero-copy ≥ 1851, streaming-no-recycle ≥ 1798, release ≥ 1749

---

## Unit Tests

### UT-0723-01: `decode_biguint_handles_c1_eq_5_canonical`

**Purpose:** Closes Demo 2 of `docs/demos/horner-g1-demonstration.md`. `[3,5]@4 → 23` MUST decode after this TASK lands. This is the **smallest failing input** in HEAD; if this test passes, the cofactor path is structurally correct.

**Preconditions:** HornerCodec + `reduce_all` from HEAD; `discover_root` fallback wired (TASK-0721 BUG-001 already in HEAD).

**Input:**
```rust
let codec = HornerCodec::new();
let mut net = codec.encode(br#"{"coeffs":[3,5],"x":4}"#).unwrap();
reduce_all(&mut net);
if net.root.is_none() {
    crate::encoding::arithmetic::discover_root(&mut net);
}
let result = decode_biguint(&net);
```

**Expected output:**
```rust
assert_eq!(result, Ok(BigUint::from(23u64)));
```

**Edge cases:**
- (EC-1) Result equals `horner_serial(&[3,5], 4).unwrap()` (oracle parity).
- (EC-2) `result.unwrap().bits() == 5` (since `23 = 0b10111`).

---

### UT-0723-02: `decode_biguint_handles_c1_ge_2_small_grid`

**Purpose:** Exhaustive cofactor-path enumeration. After TASK-0723, EVERY case on the `(c0, c1, x)` grid below decodes; skip rate MUST be 0%.

**Input:**
```rust
let codec = HornerCodec::new();
for c0 in 0u64..=5 {
    for c1 in 2u64..=5 {       // cofactor branch: c1 >= 2
        for x in 0u64..=5 {
            let json = format!(r#"{{"coeffs":[{c0},{c1}],"x":{x}}}"#);
            let mut net = codec.encode(json.as_bytes()).unwrap();
            reduce_all(&mut net);
            if net.root.is_none() {
                crate::encoding::arithmetic::discover_root(&mut net);
            }
            let expected = horner_serial(&[c0, c1], x).unwrap();
            let actual = decode_biguint(&net)
                .unwrap_or_else(|e| panic!("c0={c0} c1={c1} x={x}: {e:?}"));
            assert_eq!(actual, expected, "mismatch c0={c0} c1={c1} x={x}");
        }
    }
}
```

**Expected output:** All 216 sub-cases decode and match the oracle. No `Err`.

**Edge cases (auto-covered by the grid; documented for the developer):**
- (EC-1) `c0 == 0` slice — verifies UT-0723-03 in bulk.
- (EC-2) `x == 0` slice — multiplication branch with `x == Church(0)`; reduces to `c0`.
- (EC-3) `x == 1` slice — multiplication-by-one identity; result is `c0 + c1`.

---

### UT-0723-03: `decode_biguint_handles_c0_zero`

**Purpose:** Demo input with leading-zero coefficient — verifies the readback does not mis-handle the `wire_add_into(Church(0), prod)` path.

**Input:**
```rust
let codec = HornerCodec::new();
let mut net = codec.encode(br#"{"coeffs":[0,7],"x":3}"#).unwrap();
reduce_all(&mut net);
if net.root.is_none() {
    crate::encoding::arithmetic::discover_root(&mut net);
}
let result = decode_biguint(&net);
```

**Expected output:**
```rust
assert_eq!(result, Ok(BigUint::from(21u64)));
```

**Edge cases:**
- (EC-1) `coeffs == [0, 1], x == 5` → `Ok(5)` (cofactor degenerates to 1, but encoder still emits the mul scaffold; verifies the fast-path/new-path interface).
- (EC-2) `coeffs == [0, 2], x == 0` → `Ok(0)` (both branches collapse to zero).

---

### UT-0723-04: `decode_biguint_handles_boundary_max_x_with_c1_ge_2`

**Purpose:** High-x boundary case — the encoder generates O(c1 + x) agents per multiplication scaffold. Verifies the recursive readback does not stack-overflow on a 20-30k-agent reduced net.

**Input:**
```rust
let codec = HornerCodec::new();
let mut net = codec.encode(br#"{"coeffs":[10,2],"x":10000}"#).unwrap();
reduce_all(&mut net);
if net.root.is_none() {
    crate::encoding::arithmetic::discover_root(&mut net);
}
let result = decode_biguint(&net);
```

**Expected output:**
```rust
assert_eq!(result, Ok(BigUint::from(20010u64)));
```

**Edge cases:**
- (EC-1) `result.unwrap().bits() == 15` (since `20010 = 0b100111000101010`).
- (EC-2) Test wall-time: this case reduces ~20k agents — note in rustdoc that the test takes ~1-3 s in debug mode and is the heaviest UT in this spec. Acceptable.

---

### UT-0723-05: `decode_biguint_handles_c1_eq_2_smallest`

**Purpose:** Smallest cofactor case `[1, 2] @ 2 = 5`. Recommended by TASK-0723 Notes as the developer's first fixture for `debug_print` inspection.

**Input:**
```rust
let codec = HornerCodec::new();
let mut net = codec.encode(br#"{"coeffs":[1,2],"x":2}"#).unwrap();
reduce_all(&mut net);
if net.root.is_none() {
    crate::encoding::arithmetic::discover_root(&mut net);
}
let result = decode_biguint(&net);
```

**Expected output:**
```rust
assert_eq!(result, Ok(BigUint::from(5u64)));
```

**Edge cases:**
- (EC-1) Symmetric `[2, 1] @ 2`: c_1 == 1 so the fast path (pre-TASK-0723) handles it; result == 4. MUST remain green (regression check for the original chain walker).

---

### UT-0723-06: `decode_biguint_preserves_c1_eq_1_fast_path`

**Purpose:** Regression guard — TASK-0723 MUST NOT degrade the c_1 == 1 single-iteration path that already works in HEAD (`horner_decode_canonical_case_matches_oracle` in `horner.rs:476` covers `[1,1]@2 → 3`; this test sweeps the slice exhaustively at the readback layer).

**Input:**
```rust
let codec = HornerCodec::new();
for c0 in 0u64..=5 {
    for x in 0u64..=5 {
        let json = format!(r#"{{"coeffs":[{c0},1],"x":{x}}}"#);
        let mut net = codec.encode(json.as_bytes()).unwrap();
        reduce_all(&mut net);
        if net.root.is_none() {
            crate::encoding::arithmetic::discover_root(&mut net);
        }
        let expected = horner_serial(&[c0, 1], x).unwrap();
        let actual = decode_biguint(&net).unwrap();
        assert_eq!(actual, expected);
    }
}
```

**Expected output:** All 36 cases decode; no regression on c_1 == 1.

**Edge cases:**
- (EC-1) `c0 == 0, x == 0`: result `0`. Verifies the n=0 self-loop path (E4 of R14') survives integration with the new helper.

---

### UT-0723-07: `decode_biguint_handles_boundary_max_c1`

**Purpose:** Boundary on the cofactor itself — `c_1 == MAX_CHURCH_NAT = 10_000`. The multiplication scaffold expands `c_1` iterations of x; a 10k-iteration scaffold is the upper bound of what the encoder produces for single-iteration Horner.

**Input:**
```rust
let codec = HornerCodec::new();
// c_0 small, c_1 == cap, x small to keep reduction tractable.
let mut net = codec.encode(br#"{"coeffs":[3,10000],"x":2}"#).unwrap();
reduce_all(&mut net);
if net.root.is_none() {
    crate::encoding::arithmetic::discover_root(&mut net);
}
let result = decode_biguint(&net);
```

**Expected output:**
```rust
assert_eq!(result, Ok(BigUint::from(20_003u64)));
```

**Edge cases:**
- (EC-1) Reduction wall-time: ~5-15 s in debug. Document in rustdoc; do NOT gate behind `cfg(release)` — this is the worst single-iteration case and must be exercised in `cargo test`.
- (EC-2) `[0, 10000] @ 1`: result `10000` (boundary on output; coincides with `decode_nat` cap).

---

### UT-0723-10: `decode_biguint_error_message_names_new_helper`

**Purpose:** Acceptance criterion #4 — any `UnrecognizedStructure` fired by the new helper MUST carry a message that names the helper (e.g., `"chain_via_mul_subnet"`). This makes Stage 5 QA bug-hunting cheap.

**Approach:** Construct (manually) a net that LOOKS like a single-iteration Horner output but whose principal port at the cofactor DUP is wired to an ERA (not a CON). The decoder should detect this and emit a named error.

**Input:**
```rust
// Build a minimal malformed cofactor frame: encode `[1, 2] @ 2`, then
// surgically rewire one CON in the mul scaffold to ERA. The exact
// surgery depends on the helper's recognition pattern; the developer
// chooses the smallest malformation that the new helper would reject.
let codec = HornerCodec::new();
let mut net = codec.encode(br#"{"coeffs":[1,2],"x":2}"#).unwrap();
reduce_all(&mut net);
if net.root.is_none() {
    crate::encoding::arithmetic::discover_root(&mut net);
}
// Locate the cofactor DUP's principal target and replace its symbol
// with ERA. (Developer fills in based on the helper they author; the
// test is a behavioural check on the message, not the exact rewiring.)
let _ = net; // placeholder — see TASK-0723 Notes for the recipe

// Assert: error message MUST name the helper.
let result = decode_biguint(&net);
match result {
    Err(DecodeError::UnrecognizedStructure(msg)) => {
        assert!(
            msg.contains("chain_via_mul_subnet")
                || msg.contains("chain_from_dup_branch")
                || msg.contains("count_chain_through_dups"),
            "error message MUST name the helper that detected the malformation: got {msg}"
        );
    }
    other => panic!("expected UnrecognizedStructure, got {other:?}"),
}
```

**Expected output:** `Err(UnrecognizedStructure(_))` whose message contains the name of one of the readback helpers.

**Edge cases:**
- (EC-1) If the developer chooses to use the existing helpers without a new name, the assertion still passes (it accepts any of the three known helper names). This keeps the test resilient to factoring choices.

---

## Property Tests

### PT-0723-08: `decode_biguint_matches_oracle_on_single_iteration_property`

**Property:** For every `(c0, c1, x)` with `c0 in 0..=10_000`, `c1 in 2..=10_000`, `x in 0..=10_000`, `decode_biguint(encode + reduce) == horner_serial(&[c0, c1], x)`. No `Err` is acceptable (fail loudly, do NOT skip).

**Generator strategy:**
```text
c0 in 0u64..=10_000
c1 in 2u64..=10_000           // cofactor branch only
x  in 0u64..=10_000
```

**Assertion:**
```rust
proptest! {
    #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]
    #[test]
    fn decode_biguint_matches_oracle_single_iter_c1_ge_2(
        c0 in 0u64..=10_000,
        c1 in 2u64..=10_000,
        x  in 0u64..=10_000,
    ) {
        let expected = horner_serial(&[c0, c1], x).unwrap();
        let codec = HornerCodec::new();
        let json = format!(r#"{{"coeffs":[{c0},{c1}],"x":{x}}}"#);
        let mut net = codec.encode(json.as_bytes()).unwrap();
        reduce_all(&mut net);
        if net.root.is_none() {
            crate::encoding::arithmetic::discover_root(&mut net);
        }
        let actual = decode_biguint(&net)
            .unwrap_or_else(|e| panic!("c0={c0} c1={c1} x={x}: {e:?}"));
        prop_assert_eq!(actual, expected);
    }
}
```

**Sample size:** 100 internal cases (proptest default). NO silent-skip on `Err` — TASK-0723 AC requires every input in this domain to decode.

**Shrinking note:** On failure, proptest will minimize toward `(c0=0, c1=2, x=0)` — the smallest cofactor case. The shrunk counterexample tells the developer whether the bug is in the new helper (small case fails) or only in the boundary regime (only large cases fail).

**Performance note:** With `cases: 100` and a full grid that includes `c1 == 10_000` and `x == 10_000`, the average reduction takes ~1-5 s, so the proptest may take ~3-8 min. Document this; this is the heaviest test in TASK-0723 and is the safety net for the cofactor path. If wall-time becomes prohibitive in CI, gate to `release` mode and drop `cases` to 25 in debug; do NOT skip the test entirely.

---

## Regression-Tightening Test

### RT-0723-09: `pt_0715_06_skip_rate_is_bounded` threshold lowered to 50%

**Purpose:** Acceptance criterion #3 — the existing `pt_0715_06_skip_rate_is_bounded` (`horner.rs:662`) currently allows up to 95% skip on the `(1..=10) × (1..=10) × (1..=10)` grid. After TASK-0723 the empirical skip rate on that grid should be ~0% (every case is either fast-path c_1 == 1 or new-path c_1 >= 2; both readable). Drop the threshold to **50%** to leave headroom for TASK-0724's degree >= 2 domain (which is NOT exercised by this grid since `coeffs.len() == 2` only).

**Change in `relativist-core/src/encoding/horner.rs` (~5 LoC):**
```rust
// before (~line 682):
let max_skips = (total * 95) / 100;
assert!(
    skips <= max_skips,
    "PT-0715-06 readback skip rate too high: {skips}/{total} (> 95%) \
     — possible regression in readable subset (biguint_readback)"
);

// after (TASK-0723):
let max_skips = total / 2;  // 50% — was 95%; tightened by TASK-0723 RT-0723-09
assert!(
    skips <= max_skips,
    "PT-0715-06 readback skip rate too high: {skips}/{total} (> 50%) \
     — possible regression in cofactor path (biguint_readback `chain_via_mul_subnet`); \
     TASK-0725 will tighten further to 5%"
);
```

**Expected output:** Test passes with the new threshold. If it fails, the cofactor path is incomplete; the developer must revisit TASK-0723's helper.

**Edge cases:**
- (EC-1) If empirical skips on the grid are 0 (target), the threshold could be set to 0; we leave 50% as a regression gate, NOT a tightness gate (TASK-0725 handles tightness).

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-001 | `[3,5]@4` (Demo 2 — first known failing) | `Ok(BigUint::from(23))` | UT-0723-01 |
| EC-002 | Full `(0..=5)×(2..=5)×(0..=5)` cofactor grid | 216 cases, all decode | UT-0723-02 |
| EC-003 | `[0,7]@3` leading-zero coefficient | `Ok(BigUint::from(21))` | UT-0723-03 |
| EC-004 | `[10,2]@10000` high-x boundary | `Ok(BigUint::from(20010))` | UT-0723-04 |
| EC-005 | `[1,2]@2` smallest cofactor (developer fixture) | `Ok(BigUint::from(5))` | UT-0723-05 |
| EC-006 | `(0..=5)×{1}×(0..=5)` fast-path regression | 36 cases, all decode | UT-0723-06 |
| EC-007 | `[3,10000]@2` MAX cofactor | `Ok(BigUint::from(20003))` | UT-0723-07 |
| EC-008 | Full proptest `(c0 in 0..=10k, c1 in 2..=10k, x in 0..=10k)` | 100 cases, no `Err` | PT-0723-08 |
| EC-009 | Malformed cofactor frame (CON replaced by ERA) | `Err(UnrecognizedStructure)` with helper name in msg | UT-0723-10 |
| EC-010 | PT-0715-06 skip rate on `(1..=10)^3` grid | `<= 50%` | RT-0723-09 |

## Mapping to SPEC-27 v3 §7

| SPEC-27 v3 Test ID | Coverage |
|---|---|
| T7 (pipeline parity) | UT-0723-{01..07}, PT-0723-08 |
| T11 (oracle cross-check) | PT-0723-08, UT-0723-02 (per-sub-case oracle compare) |
| T12 (BigUint readback cross-check) | UT-0723-02 (oracle), UT-0723-06 (fast-path regression) |
| T13 (in-process distributed) | Indirect prerequisite: TASK-0725 widens `horner_distributed_g1.rs` to degree-2 once TASK-0724 lands |

## Dependencies Context

- `HornerCodec::encode` (TASK-0714 / TASK-0721) — already in HEAD.
- `reduce_all`, `discover_root` (TASK-0721 BUG-001) — already in HEAD.
- `horner_serial` (TASK-0713) — oracle for every cross-check.
- `count_valid_active_pairs` (TASK-0709) — unchanged; E1 NotNormalForm path stays as-is.
- `wire_mul_into`, `wire_add_into` (`arithmetic.rs`) — encoder-side topology the helper must traverse.
- `num_bigint::BigUint` — `Cargo.toml` already declares the dep (TASK-0712).

## Notes

- The c_1 == 1 fast path MUST remain functionally equivalent — if the developer factors the new helper into the same call chain, UT-0723-06 is the regression sentinel.
- UT-0723-10's exact malformation recipe is intentionally under-specified — the test-generator does NOT know which slot the new helper dispatches on. The developer fills in the surgical edit when they author the helper; the assertion is purely on the error message's `contains` check, which is helper-independent.
- The R14' Independence clause (no call to `decode_nat`) is enforced compile-time by `tests/biguint_readback_independence.rs` (CT-0712-06). TASK-0723 MUST NOT break that test; if the developer accidentally adds a `decode_nat(` call inside the new helper, CT-0712-06 fires.
- Test floor delta: **+9** (UT 01-07, PT 08, UT 10). RT-0723-09 modifies an existing test, no count delta. PT-0715-06 hardening reduces the existing test's threshold; no count delta.
- The proptest PT-0723-08 doubles as the empirical demonstration that **all 216 cases in UT-0723-02 are not a fluke** — proptest's 100 randomized samples cover the full 10_001^3 cube, complementing the deterministic 216-case grid.
