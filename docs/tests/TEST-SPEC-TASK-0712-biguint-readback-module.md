# TEST-SPEC-TASK-0712: Tests for TASK-0712 — `biguint_readback` module (`decode_biguint`)

**Task:** TASK-0712
**Spec:** SPEC-27 v3
**Bundle:** D-015 (SPEC-27 Encoder/Decoder API + HornerCodec — Topic 2)
**Requirements covered:** R14' (BigUint readback algorithm — independent from `decode_nat`), R16b' (cross-check property vs `decode_nat`), R4 (NotNormalForm semantics — uses TASK-0709 helper)
**Test IDs (from SPEC-27 v3 §7.3):** Supports T9, T9b (BigUint range — pipeline tests in TASK-0715), T11, T12 (cross-check property).
**Inviolable invariants asserted:** SPEC-14 §4.4 `decode_nat` topology preserved; SPEC-01 I4 stale-pruning enforced via `count_valid_active_pairs`.

---

## Scope

This task creates `relativist-core/src/encoding/biguint_readback.rs` exposing `decode_biguint(net) -> Result<BigUint, DecodeError>`. The function MUST mirror SPEC-14 §4.4 `decode_nat` topology and traversal exactly, replacing the `u64` accumulator with `BigUint`. It is **standalone** per R14' Independence clause: `decode_biguint` MUST NOT delegate to `decode_nat`.

The cross-check property (R16b' / T12): for any `n <= 10_000` (SPEC-14 R4 cap), `decode_biguint(encode_nat(n)) == Ok(BigUint::from(n))` AND `BigUint::from(decode_nat(encode_nat(n)).unwrap()) == decode_biguint(encode_nat(n)).unwrap()`.

## Test category & location

| # | Category | Cfg gating | File | LoC |
|---|----------|-----------|------|-----|
| UT-0712-01 | unit (in-module) | none | `relativist-core/src/encoding/biguint_readback.rs` | ~15 |
| UT-0712-02 | unit (in-module) | none | same | ~25 |
| UT-0712-03 | unit (in-module) | none | same | ~25 |
| UT-0712-04 | unit (in-module) | none | same | ~25 |
| PT-0712-05 | property test (proptest) | none | same | ~30 |
| CT-0712-06 | compile-time independence check | none | `relativist-core/tests/biguint_readback_independence.rs` | ~15 |

## Test floor delta (from TASK-0712 acceptance criteria)

- default: **+5 unit/property tests** + **+1 integration test** = **+6** → ≥ 1839
- zero-copy: **+6** → ≥ 1883
- streaming-no-recycle: **+6** → ≥ 1830
- release: **+6** → ≥ 1781

(PT-0712-05 counts as 1 proptest invocation containing N internal cases — the test-runner sees it as a single test name but with at least 100 internal samples.)

---

## Unit Tests

### UT-0712-01: `decode_biguint_zero_returns_zero`

**Purpose:** n=0 special case — verifies E4 of R14' pseudocode (self-loop + ERA detection).

**Input:**
```rust
let net = encode_nat(0);
let result = decode_biguint(&net);
```

**Expected output:**
```rust
assert_eq!(result, Ok(BigUint::from(0u64)));
```

**Edge cases:**
- (EC-1) `BigUint::from(0u64).bits() == 0` (verifies §2 bit-length semantics, used downstream by R15').

---

### UT-0712-02: `decode_biguint_small_values_match_decode_nat`

**Purpose:** Sanity check for non-zero small values; mirrors `decode_nat` exactly.

**Input:**
```rust
for &n in &[1u64, 7u64, 42u64, 255u64, 10_000u64] {
    let net = encode_nat(n);
    let big = decode_biguint(&net).unwrap();
    let small = decode_nat(&net).unwrap();
    assert_eq!(big, BigUint::from(small));
    assert_eq!(big, BigUint::from(n));
}
```

**Expected output:** All assertions pass.

**Edge cases:**
- (EC-1) `n = 10_000` — boundary value (SPEC-14 R4 cap); MUST be accepted.
- (EC-2) `n = 1` — smallest non-zero (regression for chain-walk with single CON application).

---

### UT-0712-03: `decode_biguint_rejects_non_nf`

**Purpose:** Verify that a net with at least one valid active pair returns `NotNormalForm`. Uses the TASK-0709 `count_valid_active_pairs` helper.

**Preconditions:** Build a net with one true redex (two CON agents principal-to-principal, queue contains the redex).

**Input:**
```rust
let mut net = Net::new();
let a = net.add_agent(Symbol::Con);
let b = net.add_agent(Symbol::Con);
net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
net.redex_queue.push_back((a, b));

let result = decode_biguint(&net);
```

**Expected output:**
```rust
match result {
    Err(DecodeError::NotNormalForm { redexes }) => assert_eq!(redexes, 1),
    other => panic!("expected NotNormalForm{{1}}, got {:?}", other),
}
```

**Edge cases:**
- (EC-1) Queue contains a stale entry but no live redex → MUST decode successfully (NOT `NotNormalForm`); confirms TASK-0709 helper integration.
- (EC-2) Queue contains 3 valid redexes → `redexes == 3`.

---

### UT-0712-04: `decode_biguint_rejects_malformed_root`

**Purpose:** Verify `UnrecognizedStructure` paths from R14' pseudocode (E2 root not AgentPort(_, 0); E3 inner not CON; E4 missing ERA on n=0 frame; E5 broken application chain).

**Input:** Several malformed nets, one per E-path:

(a) Root is None:
```rust
let net = Net::new();  // no root set
let result = decode_biguint(&net);
assert!(matches!(result, Err(DecodeError::DecodeFailed(_))));
```

(b) Root is AgentPort with wrong slot (not 0):
```rust
let mut net = Net::new();
let a = net.add_agent(Symbol::Con);
net.set_root(PortRef::AgentPort(a, 1));
let result = decode_biguint(&net);
assert!(matches!(result, Err(DecodeError::UnrecognizedStructure(_))));
```

(c) Root is AgentPort(_, 0) but the agent's symbol is ERA (not CON for outer lambda):
```rust
let mut net = Net::new();
let a = net.add_agent(Symbol::Era);
net.set_root(PortRef::AgentPort(a, 0));
let result = decode_biguint(&net);
assert!(matches!(result, Err(DecodeError::UnrecognizedStructure(_))));
```

**Expected output:** Each malformed input returns the appropriate `DecodeError` variant with a descriptive message.

**Edge cases:**
- (EC-1) Outer CON found but inner CON missing — `UnrecognizedStructure`.
- (EC-2) Application chain broken mid-walk (DISCONNECTED port) — `UnrecognizedStructure`.

---

## Property Tests

### PT-0712-05: `decode_biguint_cross_check_decode_nat_property` (R16b' / T12)

**Property:** For any `n` in `[0, 10_000]`, `decode_biguint(encode_nat(n)) == Ok(BigUint::from(decode_nat(encode_nat(n)).unwrap()))`.

**Generator strategy:**
```text
arb_n: any u64 in [0, 10_000]  // SPEC-14 R4 cap
```

**Assertion:**
```rust
proptest! {
    #[test]
    fn decode_biguint_cross_check_decode_nat(n in 0u64..=10_000u64) {
        let net = encode_nat(n);
        let big   = decode_biguint(&net).unwrap();
        let small = decode_nat(&net).unwrap();
        prop_assert_eq!(big.clone(), BigUint::from(small));
        prop_assert_eq!(big, BigUint::from(n));
    }
}
```

**Sample size:** At least 100 internal cases (proptest default). Per R16b'.

**Shrinking note:** On failure, proptest minimizes `n` toward 0; report the smallest `n` exhibiting divergence, which would either expose (a) a wrap-around bug in the BigUint accumulator if `n > u64::MAX % small`, or (b) a structural mis-translation between `decode_nat` and `decode_biguint` topology. Both indicate R14' Independence violation or topology drift.

**Boundary cases (auto-included):**
- `n = 0` (E4 path).
- `n = 1` (smallest non-zero chain).
- `n = 10_000` (cap).

---

## Compile-time Tests

### CT-0712-06: `decode_biguint_independence_from_decode_nat`

**Purpose:** R14' Independence clause — `decode_biguint` MUST NOT call `decode_nat`. Verified at compile time / via static inspection.

**Approach (preferred):** Static check in an integration test under `relativist-core/tests/biguint_readback_independence.rs`:
```rust
// SPEC-27 v3 R14' Independence clause: decode_biguint MUST NOT delegate to decode_nat.
// This test reads the file content and asserts no `decode_nat(` substring appears in
// the implementation block (as opposed to the doc comment, which may reference
// SPEC-14 §4.4 by name but not by call).

#[test]
fn decode_biguint_does_not_call_decode_nat() {
    let src = include_str!("../src/encoding/biguint_readback.rs");
    // Strip rustdoc lines (those that start with `///`) before searching.
    let code_only: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("///") && !l.trim_start().starts_with("//!"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !code_only.contains("decode_nat("),
        "decode_biguint MUST be standalone — must not call decode_nat (R14' Independence)"
    );
}
```

**Expected output:** Test passes iff `biguint_readback.rs` source code (excluding rustdoc) does NOT contain the substring `decode_nat(`.

**Edge cases:**
- (EC-1) Rustdoc comments referencing `decode_nat` by name (e.g., `/// Mirrors decode_nat topology...`) MUST NOT trigger the test failure (filter out lines starting with `///`).
- (EC-2) If a shared `walk_church<Counter>` helper is used and lives in `biguint_readback`, the helper itself MAY be called by both `decode_nat` and `decode_biguint`, but `decode_biguint` MUST NOT directly call `decode_nat`. The test as written enforces this.

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-001 | `n = 0` | `Ok(BigUint::from(0u64))` | UT-0712-01 |
| EC-002 | `n = 10_000` (boundary) | `Ok(BigUint::from(10_000u64))` | UT-0712-02 EC-1 |
| EC-003 | One valid redex queued | `NotNormalForm { redexes: 1 }` | UT-0712-03 |
| EC-004 | One stale entry, no live redex | Decodes successfully (no false positive) | UT-0712-03 EC-1 |
| EC-005 | Root not AgentPort(_, 0) | `UnrecognizedStructure` | UT-0712-04 |
| EC-006 | Outer agent is ERA (not CON) | `UnrecognizedStructure` | UT-0712-04 |
| EC-007 | Cross-check on full range `[0, 10_000]` | `decode_biguint == BigUint::from(decode_nat)` | PT-0712-05 |
| EC-008 | `decode_biguint` does not call `decode_nat` | grep-source confirms | CT-0712-06 |

## Mapping to SPEC-27 v3 §7

| SPEC-27 v3 Test ID | Coverage |
|---|---|
| T9 (BigUint range, 25 coeffs) | TASK-0715 (pipeline test); this TEST-SPEC provides the readback foundation |
| T9b (boundary `[10000;5] @ 10000`) | TASK-0715; same foundation |
| T11 (property test vs oracle) | TASK-0715 (pipeline); this TEST-SPEC's PT-0712-05 is the readback-only sister property |
| T12 (BigUint readback cross-check) | PT-0712-05 (canonical T12 implementation) |

## Dependencies Context

- `count_valid_active_pairs(&Net) -> usize` from TASK-0709 (R4 valid-pair semantics).
- `Net::get_target(PortRef) -> PortRef`, `DISCONNECTED` constant (SPEC-02).
- `encode_nat(n) -> Net`, `decode_nat(net) -> Result<u64, DecodeError>` from `church.rs` (SPEC-14 §4.4 — for cross-check tests; the production code does NOT call `decode_nat`).
- `num_bigint::BigUint` from `num-bigint = "^0.4"` (added to `Cargo.toml` in this task).
- `proptest` already in `[dev-dependencies]`.

## Notes

- The shared `walk_church<Counter>` helper is OPTIONAL. If used, it MUST live in `biguint_readback` (per TASK-0712 acceptance criteria); both `decode_nat` and `decode_biguint` MAY instantiate it with different counter types. CT-0712-06 still passes because `decode_biguint` calls `walk_church`, not `decode_nat` directly.
- T11 negative-cross-check (encoder error families matching oracle errors) does NOT run in this TEST-SPEC — it lives in TASK-0715 (HornerCodec decoder + property tests).
- Test floor delta: **+6** total (5 unit/property in-module + 1 integration test).
