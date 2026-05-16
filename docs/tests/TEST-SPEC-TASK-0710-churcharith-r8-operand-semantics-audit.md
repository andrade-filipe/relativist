# TEST-SPEC-TASK-0710: Tests for TASK-0710 — ChurchArithmeticCodec audit + R8 operand semantics

**Task:** TASK-0710
**Spec:** SPEC-27 v3
**Bundle:** D-015 (SPEC-27 Encoder/Decoder API + HornerCodec — Topic 2)
**Requirements covered:** R7 (SPEC-14 R3 signatures unchanged), R8 (operand semantics for `exp` / `sum_of_squares` — SC-003), R9 (690 v1 tests pass)
**Test IDs (from SPEC-27 v3 §7.2):** T3 (round-trip per op), T4 (all previously-passing tests pass).

---

## Scope

This task is primarily **audit-and-document** for `ChurchArithmeticCodec` (which already exists in `relativist-core/src/encoding/codec_church.rs` per HEAD). It locks in v3 R8 operand semantics with explicit unit tests for the canonical edge cases:

- `exp`: `a` is base, `b` is exponent (SPEC-14 R17 ordering — `build_exp(a, b)` so result is `a^b`).
- `sum_of_squares`: `a` is `n` (upper bound); `b` is ignored / MAY be omitted.

Decision per Round 2 closure SC-003: R7 was softened to "MUST NOT change SPEC-14 R3 public function signatures" (only); R8 v3 explicitly pins `(a, b)` mapping per op; `build_sum_of_squares` lives alongside R3 helpers per SPEC-09 R17d.

## Test category & location

| # | Category | Cfg gating | File | LoC |
|---|----------|-----------|------|-----|
| UT-0710-01 | unit (in-module) | none | `relativist-core/src/encoding/codec_church.rs` | ~20 |
| UT-0710-02 | unit (in-module) | none | same | ~25 |
| UT-0710-03 | unit (in-module) | none | same | ~25 |
| UT-0710-04 | unit (in-module) | none | same | ~20 |
| UT-0710-05 | unit (in-module) | none | same | ~25 |

## Test floor delta (from TASK-0710 acceptance criteria)

- default: **+5** → ≥ 1829 (cumulative on top of TASK-0709)
- zero-copy: **+5** → ≥ 1873
- streaming-no-recycle: **+5** → ≥ 1820
- release: **+5** → ≥ 1771

---

## Unit Tests

### UT-0710-01: `church_codec_add_a_plus_b`

**Purpose:** Verify `op = "add"` invokes `build_add(a, b)` and the round-trip yields `a + b`.

**Input:**
```rust
let codec = ChurchArithmeticCodec::new(ChurchOp::Add);
let input = br#"{"op":"add","a":3,"b":5}"#;
let net = codec.encode(input).unwrap();
let mut net = net;
reduce_all(&mut net);
let out = codec.decode(&net).unwrap();
```

**Expected output:**
```rust
assert_eq!(out["result"].as_u64().unwrap(), 8);
assert!(out["interactions"].as_u64().is_some());  // present, non-null
```

**Edge cases:**
- (EC-1) `a = 0, b = 0` → `result == 0`.
- (EC-2) `a = 1, b = 0` → `result == 1` (additive identity).

---

### UT-0710-02: `church_codec_exp_a_is_base_b_is_exponent`

**Purpose:** Pin v3 R8 operand mapping for `exp`: `a` is **base**, `b` is **exponent**, matching `build_exp(a, b)` (SPEC-14 R17). Closes SC-003 / Topic 2 brief §6.

**Input:**
```rust
let codec = ChurchArithmeticCodec::new(ChurchOp::Exp);
let input = br#"{"op":"exp","a":2,"b":3}"#;
let net = codec.encode(input).unwrap();
let mut net = net;
reduce_all(&mut net);
let out = codec.decode(&net).unwrap();
```

**Expected output:** `out["result"].as_u64().unwrap() == 8` (i.e., `2^3`, NOT `3^2`).

**Edge cases:**
- (EC-1) Reverse: `{"op":"exp","a":3,"b":2}` → `result == 9` (i.e., `3^2`). MUST be a separate assertion to catch operand-swap regressions.
- (EC-2) Identity: `a=5, b=0` → `result == 1` (any base raised to zero).
- (EC-3) Trivial: `a=1, b=10` → `result == 1`.

---

### UT-0710-03: `church_codec_sum_of_squares_uses_a_only`

**Purpose:** Pin v3 R8 mapping for `sum_of_squares`: `a` is upper bound `n`; `b` is ignored. JSON MAY omit `b` entirely.

**Input (b omitted):**
```rust
let codec = ChurchArithmeticCodec::new(ChurchOp::SumOfSquares);
let input = br#"{"op":"sum_of_squares","a":3}"#;
let net = codec.encode(input).unwrap();
let mut net = net;
reduce_all(&mut net);
let out = codec.decode(&net).unwrap();
```

**Expected output:** `out["result"].as_u64().unwrap() == 14` (= `1 + 4 + 9`).

**Edge cases:**
- (EC-1) `a = 0` → `result == 0` (empty sum).
- (EC-2) `a = 1` → `result == 1`.
- (EC-3) `a = 5` → `result == 55` (= `1 + 4 + 9 + 16 + 25`).

---

### UT-0710-04: `church_codec_sum_of_squares_b_ignored_when_present`

**Purpose:** Defensive — if user passes a stray `b`, codec MUST still produce same result. Closes SC-003 wording "b ignored".

**Input (b present, but should be silently ignored):**
```rust
let codec = ChurchArithmeticCodec::new(ChurchOp::SumOfSquares);
let input_with_b    = br#"{"op":"sum_of_squares","a":3,"b":99}"#;
let input_without_b = br#"{"op":"sum_of_squares","a":3}"#;
let net_with    = codec.encode(input_with_b).unwrap();
let net_without = codec.encode(input_without_b).unwrap();
let mut n1 = net_with;    reduce_all(&mut n1);
let mut n2 = net_without; reduce_all(&mut n2);
let r1 = codec.decode(&n1).unwrap();
let r2 = codec.decode(&n2).unwrap();
```

**Expected output:** `r1["result"] == r2["result"]` AND both equal 14.

**Edge cases:**
- (EC-1) `b = 0` (default-y) — same result.
- (EC-2) `b = u64::MAX` — same result (SHOULD NOT cause overflow since `b` is unused).

---

### UT-0710-05: `church_codec_mul_a_times_b`

**Purpose:** Round-trip for `mul`, completing the T3 quad. Per R8: both `a` and `b` are operands.

**Input:**
```rust
let codec = ChurchArithmeticCodec::new(ChurchOp::Mul);
let input = br#"{"op":"mul","a":4,"b":7}"#;
let net = codec.encode(input).unwrap();
let mut net = net;
reduce_all(&mut net);
let out = codec.decode(&net).unwrap();
```

**Expected output:** `out["result"].as_u64().unwrap() == 28`.

**Edge cases:**
- (EC-1) `a = 0, b = 7` → `result == 0` (zero-product).
- (EC-2) `a = 1, b = 99` → `result == 99` (multiplicative identity).
- (EC-3) `a = 99, b = 0` → `result == 0` (commutativity).

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-001 | `add` round-trip 3+5 | result = 8 | UT-0710-01 |
| EC-002 | `mul` round-trip 4×7 | result = 28 | UT-0710-05 |
| EC-003 | `exp` operand order: a is base | `2^3 = 8`, not `3^2 = 9` | UT-0710-02 |
| EC-004 | `exp` reverse order check | `3^2 = 9` | UT-0710-02 EC-1 |
| EC-005 | `sum_of_squares` b omitted in JSON | parses; result = 14 | UT-0710-03 |
| EC-006 | `sum_of_squares` b present but bogus | same result as b omitted | UT-0710-04 |
| EC-007 | `add(0,0)` | result = 0 | UT-0710-01 EC-1 |
| EC-008 | `exp(_, 0) = 1` | identity | UT-0710-02 EC-2 |
| EC-009 | `sum_of_squares(0)` empty sum | result = 0 | UT-0710-03 EC-1 |

## Mapping to SPEC-27 v3 §7

| SPEC-27 v3 Test ID | Coverage |
|---|---|
| T3 (round-trip add/mul/exp/sum_of_squares) | UT-0710-01 (add), UT-0710-05 (mul), UT-0710-02 (exp), UT-0710-03 (sum_of_squares) |
| T4 (all 690 v1 tests still pass) | CI gate; no separate test added (CLAUDE.md `cargo test` floor) |

## Notes

- This task adds **no production code** if the existing `ChurchArithmeticCodec::encode` already routes `(a, b)` per v3 R8. Stage 4 reviewer agent verifies via inspection (per TASK-0710 Notes).
- Rustdoc updates on the codec module to cite SPEC-27 v3 R7-R9 (mention SPEC-14 R3 invariance) are the only doc deltas.
- This task does NOT modify the registry — TASK-0716 swaps `lambda` for `horner`; ChurchArithmeticCodec entries are untouched.
- Test floor delta is **+5** unit tests, in-module on `codec_church.rs`.
- T4 (CI gate "all 690 v1 tests pass unchanged") is enforced by the CLAUDE.md test floor; we do NOT add a sentinel test for it.
