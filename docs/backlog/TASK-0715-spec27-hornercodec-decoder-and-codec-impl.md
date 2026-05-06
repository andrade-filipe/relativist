# TASK-0715 — SPEC-27 v3 R14', R15', R16': HornerCodec decoder + `Codec` impl

**Spec:** SPEC-27 v3
**Requirements:** R14' (decoder calls `decode_biguint`), R15' (output schema with `bit_length`), R16' (edge cases — decode side), R3 (HornerCodec implements `Codec`)
**Priority:** P0 (closes Phase 3c; HornerCodec is registrable after this)
**Status:** TODO
**Depends on:** TASK-0712 (`decode_biguint`), TASK-0713 (`horner_serial` oracle), TASK-0714 (HornerCodec encoder)
**Blocked by:** none
**Estimated complexity:** S–M (~80 LoC production + ~80 LoC tests including T11 property test)
**Bundle:** SPEC-27 Encoder/Decoder API + HornerCodec (Topic 2)

---

## Context

This task closes Phase 3c of SPEC-27 v3 §6 by:
1. Implementing `Decoder` for `HornerCodec` — wrapping `decode_biguint` from
   TASK-0712 and serializing to the R15' output schema.
2. Implementing `Codec` for `HornerCodec` — providing the
   `description()` string mandated by R3.
3. Adding the property test T11 (cross-check encoder+decoder against
   `horner_serial` oracle on at least 100 valid inputs + 30 negative inputs).
4. Adding the BigUint-range tests T9 (25 coefficients) and T9b (boundary value
   `10_000` + BigUint range) that require the full encode→reduce→decode pipeline.

R15' output schema:

```json
{
  "value": "<base-10 BigUint string>",
  "bit_length": <usize>
}
```

`bit_length` follows `BigUint::bits()` semantics (§2): number of bits to
represent the value in base 2 (e.g., `BigUint::from(0u64).bits() == 0`,
`BigUint::from(u64::MAX).bits() == 64`).

## Acceptance Criteria

- [ ] `impl Decoder for HornerCodec { fn decode(&self, net: &Net) -> Result<serde_json::Value, DecodeError> }` calls `decode_biguint(net)` and serializes to the R15' schema.
- [ ] `impl Codec for HornerCodec { fn description(&self) -> &str { "Polynomial evaluation via Horner's method" } }` (or close paraphrase consistent with R22 listing).
- [ ] Output JSON has exactly two top-level keys (`"value"` and `"bit_length"`); `value` is a base-10 string; `bit_length` is a non-negative integer.
- [ ] `bit_length` MUST be derived from `BigUint::bits()` (NOT hard-coded).
- [ ] Decoder propagates `DecodeError::NotNormalForm { redexes }` from `decode_biguint` unchanged (R4 valid-pair semantics, TASK-0709).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/encoding/horner.rs` | modify | Add `Decoder` + `Codec` impls. ~30 LoC production. Add property test T11 + T9/T9b ~80 LoC. |

## Key Types / Signatures

```rust
use crate::encoding::biguint_readback::decode_biguint;
use crate::encoding::traits::{Codec, Decoder, DecodeError};

impl Decoder for HornerCodec {
    fn decode(&self, net: &Net) -> Result<serde_json::Value, DecodeError> {
        let value = decode_biguint(net)?;
        Ok(serde_json::json!({
            "value": value.to_string(),
            "bit_length": value.bits() as usize,
        }))
    }
}

impl Codec for HornerCodec {
    fn description(&self) -> &str { "Polynomial evaluation via Horner's method" }
}
```

## Test Expectations

For Stage 2 (test-generator) — maps to SPEC-27 v3 §7.3 T7, T8, T9, T9b, T10, T11:

- `horner_decode_canonical_case_matches_oracle` — `[3,2,5,1] @ 2`: pipeline yields `{"value": "35", "bit_length": 6}`; expected value via `horner_serial([3,2,5,1], 2).unwrap()`. (T7)
- `horner_decode_sparse_coefficients_match_oracle` — `[1,0,0,0,0,1] @ 10`: `value == "100001"`; cross-checked against `horner_serial`. (T8)
- `horner_pipeline_biguint_range_25_coeffs` — `[1; 25] @ 10`: `value == "1111111111111111111111111"` (≈ 10²⁴), `bit_length > 64`, exact equality to `horner_serial(...).unwrap().to_string()` and `bits()`. (T9)
- `horner_pipeline_boundary_max_inputs` — `[10000; 5] @ 10000`: `bit_length > 64`, exact equality to `horner_serial`. (T9b)
- `horner_property_test_oracle_agreement` — proptest with at least 100 valid `(coeffs, x)` cases (`coeffs.len() ∈ [1,15]`, each `coeffs[i] ∈ [0, 10000]`, `x ∈ [0, 10000]`) asserting `decode(reduce_all(encode)).value == horner_serial(coeffs, x).unwrap().to_string()`. (T11 positive)
- `horner_property_test_negative_cross_check` — proptest with at least 30 out-of-range cases (one or more `coeffs[i] > 10000`, OR `x > 10000`, OR `coeffs.len() == 0`) asserting that the encoder returns `EncodeError::InvalidInput` AND `horner_serial` returns the matching `OracleError` family (R16a' negative cross-check). (T11 negative)
- `horner_decode_rejects_non_nf` — net with one valid redex returns `DecodeError::NotNormalForm { redexes: 1 }` (R4 + R14').

## Dependencies Context

- `decode_biguint` from TASK-0712.
- `horner_serial` and `OracleError` from TASK-0713.
- `HornerCodec` and its `Encoder` impl from TASK-0714.
- `serde_json::json!` macro (existing dependency).

## Notes

- After this task, `HornerCodec` satisfies `Codec` (`Encoder + Decoder + description`)
  and is therefore registrable in `EncoderRegistry`. TASK-0716 performs that
  registration.
- T13 (full distributed equivalence test for `W ∈ {2, 4, 8}`) is NOT in this task —
  it requires `run_grid` integration and lives as a separate test-generator
  TEST-SPEC under SPEC-27 v3 §7.3 T13. The test-generator agent will fold T13
  into a TEST-SPEC consumed during Stage 3 by the developer agent; it does not
  add new code beyond test wiring.
- Property tests use `proptest` (already in `relativist-core/Cargo.toml`
  `[dev-dependencies]`).
- Test floor delta estimate: +6 unit tests + 2 property tests (default + features
  budgets — see `cargo test --features {zero-copy,streaming-no-recycle}` rules in
  CLAUDE.md).
