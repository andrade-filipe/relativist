# TASK-0712 — SPEC-27 v3 R14' / R16b': `biguint_readback` module (`decode_biguint`)

**Spec:** SPEC-27 v3
**Requirements:** R14' (BigUint readback algorithm), R16b' (independent module + cross-check with `decode_nat`)
**Priority:** P0 (HornerCodec decoder TASK-0715 wraps this)
**Status:** TODO
**Depends on:** TASK-0711 (R13a' obligation validation — ensures the wire helpers used by HornerCodec encoder are validated)
**Blocked by:** none
**Estimated complexity:** M (~80 LoC production + ~80 LoC tests)
**Bundle:** SPEC-27 Encoder/Decoder API + HornerCodec (Topic 2)

---

## Context

SPEC-27 v3 §6 Phase 3b mandates a standalone BigUint readback module at
`relativist-core::encoding::biguint_readback` exposing
`decode_biguint(net) -> Result<BigUint, DecodeError>`. The function MUST mirror
SPEC-14 §4.4 `decode_nat` topology and traversal exactly, replacing the
`count: u64` accumulator with `count: BigUint`. R14' inlines an ~80-line
normative-control-flow / informative-syntax pseudocode block (covering n=0,
chain-walk, all `UnrecognizedStructure` paths, and `NotNormalForm` with R4
valid-redex semantics).

R14' "Independence from `decode_nat`" clause: `decode_biguint` MUST be a
standalone implementation (NOT a wrapper), so the cross-check in R16b' is
meaningful. A shared private generic `walk_church<Counter>` helper is permitted
as long as both `decode_nat` and `decode_biguint` instantiate it with different
`Counter` types — but the helper itself MUST live in
`relativist-core::encoding::biguint_readback`.

R16b' cross-check: for any net produced by `encode_nat(n)` with the SPEC-14 R4
cap (`n <= 10_000`), `decode_biguint(net) == Ok(BigUint::from(decode_nat(net).unwrap()))`.

## Acceptance Criteria

- [ ] New module `relativist-core/src/encoding/biguint_readback.rs` exposes `pub fn decode_biguint(net: &Net) -> Result<BigUint, DecodeError>`.
- [ ] Algorithm follows the R14' pseudocode block: E1 (R4 valid-redex check via TASK-0709 helper), E2 (root → outer lambda CON), E3 (inner lambda CON), E4 (n=0 self-loop + ERA detection), E5 (application chain walk with `BigUint` accumulator).
- [ ] On invalid structure, returns `DecodeError::UnrecognizedStructure(...)` with a path-identifying message.
- [ ] On non-NF input (per R4 valid-pair semantics, TASK-0709), returns `DecodeError::NotNormalForm { redexes: <count> }`.
- [ ] `Cargo.toml` of `relativist-core` adds `num-bigint = "^0.4"` dependency (MIT/Apache-2.0; matches SPEC-27 v3 §2 `BigUint` definition).
- [ ] `relativist-core::encoding::mod.rs` adds `pub mod biguint_readback;` and re-exports `pub use biguint_readback::decode_biguint;`.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/encoding/biguint_readback.rs` | **CREATE.** | New module with `decode_biguint` + optional shared `walk_church<Counter>` helper. ~80 LoC. |
| `relativist-core/src/encoding/mod.rs` | modify | Add `pub mod biguint_readback;` + re-export. ~2 lines. |
| `relativist-core/Cargo.toml` | modify | Add `num-bigint = "0.4"` to `[dependencies]`. ~1 line. |

## Key Types / Signatures

```rust
use num_bigint::BigUint;
use crate::encoding::traits::DecodeError;
use crate::net::Net;

/// SPEC-27 v3 R14': BigUint readback for Church numerals.
///
/// Mirrors SPEC-14 §4.4 `decode_nat` topology and traversal, replacing the
/// `u64` accumulator with `BigUint`. Independence from `decode_nat` is
/// preserved (R14' Independence clause); they may share a private generic
/// `walk_church<Counter>` helper, but `decode_biguint` MUST NOT delegate to
/// `decode_nat`.
///
/// Errors:
/// - `NotNormalForm` if `count_valid_active_pairs(net) > 0` (SPEC-01 I4 + SPEC-27 v3 R4).
/// - `UnrecognizedStructure` if the net does not match a Church numeral frame.
pub fn decode_biguint(net: &Net) -> Result<BigUint, DecodeError>;
```

## Test Expectations

For Stage 2 (test-generator) — maps to SPEC-27 v3 §7.3 T9, T9b, T11, T12:

- `decode_biguint_zero_returns_zero` — `encode_nat(0)` → `BigUint::from(0u64)`.
- `decode_biguint_small_values` — `encode_nat(7)`, `encode_nat(42)`, `encode_nat(255)` → matching `BigUint::from(...)`.
- `decode_biguint_cross_check_decode_nat_property` — property test (T12): for `n` sampled in `[0, 10_000]`, `decode_biguint(encode_nat(n)) == Ok(BigUint::from(n))`. At least 100 cases.
- `decode_biguint_rejects_non_nf` — net with one valid redex returns `NotNormalForm { redexes: 1 }`.
- `decode_biguint_rejects_malformed_root` — net with non-CON root returns `UnrecognizedStructure`.
- `decode_biguint_independence_from_decode_nat` — assert via `Grep` (test in `tests/` directory) that `decode_biguint` does NOT call `decode_nat` directly. Compile-time enforcement is preferred (e.g., put `decode_biguint` in a child module that does not import `decode_nat`).

## Dependencies Context

- `count_valid_active_pairs(net)` from TASK-0709 (R4 valid-pair semantics).
- `Net::get_target(PortRef) -> PortRef`, `DISCONNECTED` constant (SPEC-02).
- `Symbol::{Con, Era}` enum (SPEC-02).
- `encode_nat(n) -> Net`, `decode_nat(net) -> Result<u64, DecodeError>` from `church.rs` (SPEC-14 §4.4 — for cross-check tests only; the production code does NOT call `decode_nat`).

## Notes

- The shared `walk_church<Counter>` helper is OPTIONAL. If used, the `Counter` trait
  has two implementations (one for `u64`, one for `BigUint`); both `decode_nat`
  and `decode_biguint` MUST instantiate the helper with their respective Counter
  type. The helper MUST live in `biguint_readback` so the dependency direction is
  `church → biguint_readback` (church may import the helper, but `decode_biguint`
  is independent of `decode_nat`).
- If the developer chooses NOT to use the shared helper, the algorithms are
  duplicated — the duplication is acceptable for v3 because the cross-check
  property test (T12) catches divergence on every CI run.
- This task does NOT add `decode_biguint` to `Decoder`'s `decode` impl —
  HornerCodec wraps it in TASK-0715.
- Test floor delta: +5 to +6 unit tests (default + features).
