# TASK-0716 — SPEC-27 v3 R19, R20: default_registry — drop `lambda`, add `horner`

**Spec:** SPEC-27 v3
**Requirements:** R19 (default_registry contents — 5 codecs, no `"lambda"`), R20 (duplicate-name rejection — already shipped)
**Priority:** P0 (gates CLI integration TASK-0717; v3 default_registry MUST match the spec exactly)
**Status:** TODO
**Depends on:** TASK-0715 (HornerCodec implements `Codec`)
**Blocked by:** none
**Estimated complexity:** S (~10 LoC production + ~25 LoC tests)
**Bundle:** SPEC-27 Encoder/Decoder API + HornerCodec (Topic 2)

---

## Context

The current `default_registry()` in `relativist-core/src/encoding/registry.rs:92`
registers 5 codecs: `church_add`, `church_mul`, `church_exp`,
`church_sum_of_squares`, **`lambda`**. SPEC-27 v3 R19 mandates that the default
registry contain `church_add`, `church_mul`, `church_exp`,
`church_sum_of_squares`, **`horner`** — `"lambda"` MUST NOT appear (it is now in
§5.1 Future Work).

This task swaps the registration: removes `LambdaCodec::new()` registration,
adds `HornerCodec::new()` registration. `LambdaCodec` itself is NOT deleted from
the codebase — it stays as code so a future SPEC-28 (or successor) can promote
it back into the registry without re-implementing the Mackie/Pinto pipeline.
The codec module just stops being part of the *default* registry.

R20 (duplicate-name rejection) is already implemented and shipped; this task
only adds an updated test that verifies the new 5-codec layout.

## Acceptance Criteria

- [ ] `default_registry()` registers exactly 5 codecs in this order: `church_add`, `church_mul`, `church_exp`, `church_sum_of_squares`, `horner`.
- [ ] `default_registry()` does NOT register `LambdaCodec`.
- [ ] `LambdaCodec` module (`codec_lambda.rs`) and its public re-export remain in the codebase (still constructable by user code; just not in the default registry).
- [ ] `default_registry().get("lambda").is_none()` (T16 row 2 — verifies §5.1 Future Work status).
- [ ] `default_registry().get("horner").is_some()` and its `description()` matches the R22 listing (`"Polynomial evaluation via Horner's method"`).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/encoding/registry.rs` | modify | Replace `r.register(Box::new(LambdaCodec::new()))` line with `r.register(Box::new(HornerCodec::new()))`. Update `default_registry` rustdoc to cite SPEC-27 v3 R19 (5 v1 codecs, `lambda` deferred to §5.1). ~3 lines changed. |
| `relativist-core/src/encoding/registry.rs` | modify | Update / add tests: `default_registry_lists_v3_5_codecs`, `default_registry_excludes_lambda` (was previously asserting `lambda` present — invert). ~25 LoC tests. |

## Key Types / Signatures

(No new APIs; only the body of `default_registry` changes.)

```rust
pub fn default_registry() -> EncoderRegistry {
    let mut r = EncoderRegistry::new();
    r.register(Box::new(ChurchArithmeticCodec::new(ChurchOp::Add)))
        .expect("church_add registers");
    r.register(Box::new(ChurchArithmeticCodec::new(ChurchOp::Mul)))
        .expect("church_mul registers");
    r.register(Box::new(ChurchArithmeticCodec::new(ChurchOp::Exp)))
        .expect("church_exp registers");
    r.register(Box::new(ChurchArithmeticCodec::new(ChurchOp::SumOfSquares)))
        .expect("church_sum_of_squares registers");
    r.register(Box::new(HornerCodec::new()))
        .expect("horner registers");
    r
}
```

## Test Expectations

For Stage 2 (test-generator) — maps to SPEC-27 v3 §7.4 T14, T15, T16:

- `default_registry_contains_5_v3_codecs` — `list().len() == 5`; names exactly `["church_add", "church_mul", "church_exp", "church_sum_of_squares", "horner"]` (T14).
- `default_registry_excludes_lambda` — `get("lambda").is_none()` (T16 row 2).
- `default_registry_unknown_returns_none` — `get("nonexistent").is_none()` (T16 row 1).
- `register_horner_twice_returns_duplicate_name_error` — `r.register(HornerCodec::new())` after `default_registry()` returns `RegistryError::DuplicateName` (T15).

## Dependencies Context

- `HornerCodec` from TASK-0715 (must implement `Codec`).
- `ChurchArithmeticCodec`, `ChurchOp`, `EncoderRegistry`, `RegistryError` already shipped.
- `LambdaCodec` stays available via `relativist_core::encoding::codec_lambda::LambdaCodec` for opt-in user registration.

## Notes

- Keep the existing `register_and_get_round_trip` test that uses `LambdaCodec`
  (it exercises the registry mechanics, not the default contents). It just
  shouldn't rely on `LambdaCodec` being in `default_registry()`.
- Order of the 5 codecs is editorial but SHOULD match the R19 bullet order for
  greppability and to keep `encoders list` (TASK-0718) output stable.
- This task does NOT delete `LambdaCodec` from the codebase. SPEC-27 v3 §5.1
  declares it Future Work — deletion would be a separate decision.
