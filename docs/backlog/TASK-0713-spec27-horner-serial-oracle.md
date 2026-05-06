# TASK-0713 — SPEC-27 v3 R16a': `horner_serial` oracle with `OracleError`

**Spec:** SPEC-27 v3
**Requirements:** R16a' (pure-Rust oracle returning `Result<BigUint, OracleError>`; same input bounds as encoder)
**Priority:** P0 (HornerCodec property tests T11 cross-check against this oracle)
**Status:** TODO
**Depends on:** none (independent of HornerCodec encoder; pure-Rust straight-line code)
**Blocked by:** none
**Estimated complexity:** S (~40 LoC production + ~50 LoC tests)
**Bundle:** SPEC-27 Encoder/Decoder API + HornerCodec (Topic 2)

---

## Context

SPEC-27 v3 R16a' (closure of SC-007) requires a pure-Rust oracle function that
computes `p(x) = sum(coeffs[i] * x^i for i in 0..coeffs.len())` via a
straight-line `BigUint` accumulator loop (no IC reduction). The oracle MUST
enforce **the same input bounds as the encoder (R12'/R16')**: `coeffs.len() >= 1`,
`coeffs[i] <= 10_000` for every `i`, and `x <= 10_000`. Violations MUST return
the matching `OracleError` variant. The cap value MUST be sourced from the same
shared constant `MAX_CHURCH_NAT` as R12' (single source of truth).

The oracle is used by:
- T7 (canonical Horner case — expected value computed via oracle, NOT hard-coded).
- T9, T9b (BigUint-range cases — same).
- T10 (negative cross-check — encoder `EncodeError::InvalidInput` MUST match `OracleError`).
- T11 (property test — at least 100 valid cases + 30 negative cases).

R16a' new signature (v3): `pub fn horner_serial(coeffs: &[u64], x: u64) -> Result<BigUint, OracleError>`.

## Acceptance Criteria

- [ ] `pub const MAX_CHURCH_NAT: u64 = 10_000;` added to `relativist-core::encoding::church` (single source of truth — R12' citation chain).
- [ ] `pub enum OracleError { EmptyCoeffs, CoefficientOverflow { idx, value, max }, XOverflow { value, max } }` defined with `#[derive(Debug, thiserror::Error, PartialEq, Eq)]`.
- [ ] `pub fn horner_serial(coeffs: &[u64], x: u64) -> Result<BigUint, OracleError>` implemented as a straight-line `BigUint` accumulator loop following the Horner recurrence.
- [ ] Bounds enforced **before** the loop: empty coeffs → `EmptyCoeffs`; any `coeffs[i] > MAX_CHURCH_NAT` → `CoefficientOverflow { idx: i, value: coeffs[i], max: MAX_CHURCH_NAT }`; `x > MAX_CHURCH_NAT` → `XOverflow { value: x, max: MAX_CHURCH_NAT }`.
- [ ] Constant polynomial special case (`coeffs.len() == 1`) returns `Ok(BigUint::from(coeffs[0]))` without entering the Horner loop.
- [ ] Module location: `relativist-core/src/encoding/horner_oracle.rs` (separate from the codec module to keep the dependency direction `codec → oracle` and not the reverse).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/encoding/horner_oracle.rs` | **CREATE.** | `OracleError` enum + `horner_serial` function. ~40 LoC + ~50 LoC tests. |
| `relativist-core/src/encoding/church.rs` | modify | Add `pub const MAX_CHURCH_NAT: u64 = 10_000;` (cite R12'/SPEC-14 R4 in rustdoc). ~3 lines. |
| `relativist-core/src/encoding/mod.rs` | modify | `pub mod horner_oracle; pub use horner_oracle::{horner_serial, OracleError};`. ~2 lines. |

## Key Types / Signatures

```rust
use num_bigint::BigUint;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OracleError {
    #[error("empty coeffs")]
    EmptyCoeffs,
    #[error("coefficient at index {idx} = {value} exceeds cap (max {max})")]
    CoefficientOverflow { idx: usize, value: u64, max: u64 },
    #[error("x = {value} exceeds cap (max {max})")]
    XOverflow { value: u64, max: u64 },
}

/// SPEC-27 v3 R16a': pure-Rust Horner oracle with explicit error model.
///
/// Computes `p(x) = sum(coeffs[i] * x^i for i in 0..coeffs.len())` via a
/// straight-line `BigUint` accumulator loop. Enforces the same input bounds as
/// the encoder (R12'): non-empty `coeffs`, every `coeffs[i] <= MAX_CHURCH_NAT`,
/// `x <= MAX_CHURCH_NAT`.
pub fn horner_serial(coeffs: &[u64], x: u64) -> Result<BigUint, OracleError>;
```

## Test Expectations

For Stage 2 (test-generator) — maps to SPEC-27 v3 §7.3 T7, T9, T9b, T10, T11:

- `horner_serial_constant_polynomial` — `[42] @ 0` → `42`; `[42] @ 7` → `42`.
- `horner_serial_canonical_explainer_case` — `[3,2,5,1] @ 2` → `35` (T7 canonical input).
- `horner_serial_sparse_coefficients` — `[1,0,0,0,0,1] @ 10` → `100001` (T8 cross-check).
- `horner_serial_biguint_range_25_coeffs` — `[1; 25] @ 10` → `(10^25 - 1) / 9` exactly; assert `bits() > 64` (T9 BigUint witness).
- `horner_serial_boundary_max_inputs` — `[10000, 10000, 10000, 10000, 10000] @ 10000` → returns `Ok(_)` with `bits() > 64` (T9b boundary).
- `horner_serial_empty_coeffs_returns_error` — `[]` → `OracleError::EmptyCoeffs`.
- `horner_serial_coefficient_overflow_returns_error` — `[10001] @ 0` → `OracleError::CoefficientOverflow { idx: 0, value: 10001, max: 10000 }`.
- `horner_serial_x_overflow_returns_error` — `[1] @ 10001` → `OracleError::XOverflow { value: 10001, max: 10000 }`.

## Dependencies Context

- `num_bigint::BigUint` (added to `Cargo.toml` by TASK-0712 — if TASK-0712 has not yet been merged, this task adds it; the dependency is idempotent).
- `MAX_CHURCH_NAT` constant defined in this task (consumed by TASK-0714 R12' encoder validation).

## Notes

- Implementation is purely arithmetic (no IC nets, no reduction). The oracle is
  the *reference semantics* against which the codec is verified.
- The `from_u64` conversion fits all `coeffs[i]` and `x` (since both are `u64`),
  so no `TryInto` needed; just `BigUint::from(coeffs[i])`.
- Pre-loop validation MUST be exhaustive: do NOT short-circuit on empty before
  checking coefficient bounds, because R12' specifies an explicit ordering of
  checks (empty first, then per-coeff, then x). Recommended order matches the
  enum variants top-to-bottom.
- Single source of truth (`MAX_CHURCH_NAT`) shared with TASK-0714 R12'
  encoder validation prevents divergence if SPEC-14 R4 raises the cap in the future.
