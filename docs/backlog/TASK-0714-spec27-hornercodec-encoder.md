# TASK-0714 — SPEC-27 v3 R10'-R13', R16': HornerCodec encoder (Horner recurrence + bounds)

**Spec:** SPEC-27 v3
**Requirements:** R10' (module location), R11' (input schema), R12' (input bounds via `MAX_CHURCH_NAT`), R13' (Horner construction pseudocode), R16' (edge cases)
**Priority:** P0 (Phase 3a deliverable; HornerCodec is the v1 codec illustrating ARG-001 G1)
**Status:** TODO
**Depends on:** TASK-0711 (R13a' wire helper validation), TASK-0713 (`MAX_CHURCH_NAT` + `horner_serial` for tests)
**Blocked by:** none
**Estimated complexity:** M (~120 LoC production + ~70 LoC tests)
**Bundle:** SPEC-27 Encoder/Decoder API + HornerCodec (Topic 2)

---

## Context

SPEC-27 v3 §3.4 specifies `HornerCodec`, the v1 codec illustrating ARG-001 G1
empirically: a classically-sequential algorithm (Horner's recurrence) executed
correctly under distributed reduction with arbitrary worker count `W` and BSP
scheduling. The encoder is composed exclusively on top of SPEC-14 primitives
(`encode_church_into`, R4b) and the R13a' composable helpers (`wire_add_into`,
`wire_mul_into`) validated in TASK-0711.

This task ships **only the encoder**; the decoder (which wraps `decode_biguint`
from TASK-0712) and the registry registration land in TASK-0715 and TASK-0716
respectively. The split keeps each task under the SDD <200 LoC ceiling
(SPEC-27 v3 §6 Phase 3a/3b/3c rationale, SC-012 closure).

## Acceptance Criteria

- [ ] New module `relativist-core/src/encoding/horner.rs` exposes `pub struct HornerCodec` with `pub fn new() -> Self` and `impl Encoder for HornerCodec`.
- [ ] `Encoder::name()` returns `"horner"`; `Encoder::encode(input)` parses JSON per R11' schema (`{ "coeffs": [u64..], "x": u64 }`) and constructs the IC net per R13' pseudocode.
- [ ] R12' bound validation runs **before** any call to `encode_church_into`: empty coeffs → `EncodeError::InvalidInput("empty coeffs")`; `coeffs[i] > MAX_CHURCH_NAT` → `EncodeError::InvalidInput(...)` identifying the offending index and value; `x > MAX_CHURCH_NAT` → `EncodeError::InvalidInput(...)`. The cap value is read from `church::MAX_CHURCH_NAT` (NOT a `10_000` literal).
- [ ] R16' edge cases handled correctly: `coeffs.len() == 1` skips the Horner loop and emits a single-`encode_church_into` net + `set_root` (constant polynomial fast path); `x == 0` and all-zero coefficients reduce correctly via the standard reducer (no encoder special-case beyond constant polynomial).
- [ ] R13' construction follows the pseudocode literally: `acc = encode_church_into(coeffs[n])`; for each `k` from `n-1` down to `0`: `prod = wire_mul_into(net, acc_port, x_port)`, `acc = wire_add_into(net, prod_port, coef_port)`. Each call to `wire_*_into` operates on the SAME `net` (R13a' composability requirement).
- [ ] `relativist-core::encoding::mod.rs` adds `pub mod horner;` and `pub use horner::HornerCodec;`.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/encoding/horner.rs` | **CREATE.** | New module with `HornerCodec` struct + `Encoder` impl. ~120 LoC production + ~70 LoC unit tests. |
| `relativist-core/src/encoding/mod.rs` | modify | `pub mod horner; pub use horner::HornerCodec;`. ~2 lines. |

## Key Types / Signatures

```rust
use num_bigint::BigUint;
use serde::Deserialize;
use crate::encoding::traits::{Encoder, EncodeError};
use crate::encoding::arithmetic::{wire_add_into, wire_mul_into};
use crate::encoding::church::{encode_church_into, MAX_CHURCH_NAT};
use crate::net::{Net, PortRef};

#[derive(Debug, Deserialize)]
struct HornerInput {
    coeffs: Vec<u64>,
    x: u64,
}

/// SPEC-27 v3 R10': polynomial evaluation via Horner's method, composed on top
/// of SPEC-14 Church arithmetic primitives plus R13a' composable helpers.
/// Empirical illustration of ARG-001 G1.
pub struct HornerCodec;

impl HornerCodec {
    pub fn new() -> Self { Self }
}

impl Encoder for HornerCodec {
    fn name(&self) -> &str { "horner" }
    fn encode(&self, input: &[u8]) -> Result<Net, EncodeError> { /* R12' bounds + R13' construction */ }
}
```

R13' construction (pseudocode reference, copy from SPEC-27 v3 §3.4 R13'):

```text
let mut net = Net::new();
let acc_id = encode_church_into(&mut net, coeffs[n]);
let mut acc_port = PortRef::AgentPort(acc_id, 0);

for k in (0..n).rev() {
    let x_id = encode_church_into(&mut net, x);
    let x_port = PortRef::AgentPort(x_id, 0);
    let prod_id = wire_mul_into(&mut net, acc_port, x_port);
    let prod_port = PortRef::AgentPort(prod_id, 0);
    let coef_id = encode_church_into(&mut net, coeffs[k]);
    let coef_port = PortRef::AgentPort(coef_id, 0);
    let new_acc_id = wire_add_into(&mut net, prod_port, coef_port);
    acc_port = PortRef::AgentPort(new_acc_id, 0);
}

net.set_root(acc_port);
```

## Test Expectations

For Stage 2 (test-generator) — maps to SPEC-27 v3 §7.3 T5, T6, T7, T8, T10, T11 (encode-side):

- `horner_encode_constant_polynomial_skips_loop` — `[42] @ 0`, `[42] @ 7`: net has one Church sub-net rooted at `coeffs[0]`; reduction yields `Church(42)` (T5).
- `horner_encode_smallest_recurrence` — `[1,1,1,1,1] @ 2`: net reduces to `Church(31)` (T6 sequential baseline).
- `horner_encode_canonical_case_matches_oracle` — `[3,2,5,1] @ 2`: assert `decode_nat(reduce_all(encode))` equals `horner_serial([3,2,5,1], 2).unwrap().to_string()` (T7 — value computed via oracle, NOT hard-coded).
- `horner_encode_sparse_coefficients` — `[1,0,0,0,0,1] @ 10`: result via `decode_biguint`/`decode_nat` matches `horner_serial(...).unwrap()` (T8 cross-check).
- `horner_encode_empty_coeffs_returns_error` — `{ "coeffs": [], "x": 0 }` → `EncodeError::InvalidInput("empty coeffs")` (T10 row 1).
- `horner_encode_coefficient_overflow_returns_error` — `{ "coeffs": [10001], "x": 0 }` → `EncodeError::InvalidInput` mentioning index/value (T10 row 3).
- `horner_encode_x_overflow_returns_error` — `{ "coeffs": [1], "x": 10001 }` → `EncodeError::InvalidInput` mentioning the cap (T10 row 4).
- `horner_encode_boundary_max_accepted` — `{ "coeffs": [10000], "x": 10000 }` → `Ok(net)` (T10 row 2).
- (T11 property test landed in TASK-0715 alongside the decoder — encoder-only here.)

## Dependencies Context

- `wire_add_into`, `wire_mul_into` from `arithmetic.rs` (R13a', validated in TASK-0711).
- `encode_church_into` from `church.rs` (SPEC-14 R4b).
- `MAX_CHURCH_NAT` from `church.rs` (TASK-0713).
- `Net::new()`, `Net::set_root(PortRef)`, `PortRef::AgentPort` (SPEC-02).
- `EncodeError` enum from `traits.rs`.
- `serde_json` already in `relativist-core` `Cargo.toml`.

## Notes

- This task ships an `Encoder` impl ONLY. The `Codec` impl (which requires
  `Decoder` per SPEC-27 R3) lands in TASK-0715. Until then, `HornerCodec` is NOT
  registrable in `EncoderRegistry` (registry expects `Box<dyn Codec>`).
- HornerCodec is NOT a `RecipeEncoder` (Q4 in SPEC-27 v3 §8). The fallback to
  centralized partition (R25) is implicit; no extra code in this task.
- Constant-polynomial fast path (`coeffs.len() == 1`) is mandatory per R16'
  bullet 2: the encoder MUST skip the Horner loop entirely (no degenerate
  `wire_mul_into` with phantom `x_port`). Test
  `horner_encode_constant_polynomial_skips_loop` enforces by asserting the
  resulting net has zero application CON agents beyond the Church-numeral
  scaffold (or, more pragmatically, by counting agents and comparing to a
  reference Church(coeffs[0]) net).
- T9 / T9b BigUint-range cases land in TASK-0715 because they require the
  decoder; this task only verifies encoder-side correctness via the oracle.
