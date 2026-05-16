# TASK-0710 — SPEC-27 v3 R7-R9: ChurchArithmeticCodec audit + R8 operand semantics

**Spec:** SPEC-27 v3
**Requirements:** R7 (softened — only SPEC-14 R3 signatures unchanged), R8 (operand semantics for `exp` / `sum_of_squares`), R9 (690 v1 tests pass)
**Priority:** P1
**Status:** TODO
**Depends on:** TASK-0709 (R4 NotNormalForm semantics in scope)
**Blocked by:** none
**Estimated complexity:** S (~30 LoC production + ~40 LoC tests)
**Bundle:** SPEC-27 Encoder/Decoder API + HornerCodec (Topic 2)

---

## Context

SPEC-27 v3 R8 (closure of SC-003) makes operand semantics for `op = "exp"` and
`op = "sum_of_squares"` explicit:

- `exp`: `a` is the base, `b` is the exponent (matching SPEC-14 R17 ordering
  `build_exp(base, exp) -> Net` — codec invokes `build_exp(a, b)`).
- `sum_of_squares`: `a` is the upper bound `n`; `b` is ignored / MAY be omitted.

`ChurchArithmeticCodec` already exists in `relativist-core/src/encoding/codec_church.rs`
(shipped pre-v3). This task audits its v3 conformance and adds explicit unit tests
covering the v3 operand-semantic clarifications. It also asserts that
`build_sum_of_squares` (SPEC-09 R17d) is reachable from this codec via the existing
`ChurchOp::SumOfSquares` variant. No SPEC-14 R3 public function signatures may
change (R7).

## Acceptance Criteria

- [ ] Audit `ChurchArithmeticCodec::encode` confirms the JSON dispatch matches v3 R8 operand mapping: `{op:"exp", a:2, b:3}` invokes `build_exp(2, 3)` (i.e., `2^3 = 8`).
- [ ] Audit `ChurchArithmeticCodec::encode` confirms `{op:"sum_of_squares", a:3}` invokes `build_sum_of_squares(3)` (i.e., `1+4+9 = 14`); `b` MUST be optional in the input schema.
- [ ] Rustdoc on the codec module updated to cite SPEC-27 v3 R7-R9 (mention SPEC-14 R3 invariance explicitly).
- [ ] No SPEC-14 R3 signature changes (verify by `Grep` over `encoding::church` and `encoding::arithmetic` public re-exports).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/encoding/codec_church.rs` | modify | Audit + comment; add unit tests for v3 R8 operand semantics if absent. |
| `relativist-core/src/encoding/mod.rs` | unchanged | Re-export list MUST stay byte-identical to current state (R7). |

## Key Types / Signatures

(Existing — no API changes.)

```rust
pub struct ChurchArithmeticCodec { /* ... */ }
pub enum ChurchOp { Add, Mul, Exp, SumOfSquares }

impl ChurchArithmeticCodec {
    pub fn new(op: ChurchOp) -> Self;
}
```

JSON input schema (v3 R8):

```json
{ "op": "add" | "mul" | "exp" | "sum_of_squares", "a": <u64>, "b": <u64> }
```

Output schema:

```json
{ "result": <u64>, "interactions": <u64> }
```

## Test Expectations

For Stage 2 (test-generator) — maps to SPEC-27 v3 §7.2 T3, T4:

- `church_codec_exp_a_is_base_b_is_exponent` — `{op:"exp", a:2, b:3}` → `result == 8` (T3 row).
- `church_codec_sum_of_squares_uses_a_only` — `{op:"sum_of_squares", a:3}` → `result == 14`; `b` field omitted in JSON MUST parse without error.
- `church_codec_sum_of_squares_b_ignored_when_present` — `{op:"sum_of_squares", a:3, b:99}` → identical result to `b` omitted (defensive against accidental `b` in user input).
- `all_690_v1_tests_still_pass` — covered by CI `cargo test` floor; no separate test added.

## Dependencies Context

- `build_add(a, b) -> Net`, `build_mul(a, b) -> Net`, `build_exp(base, exp) -> Net` from SPEC-14 R15-R17 (already exported via `relativist-core::encoding::arithmetic`).
- `build_sum_of_squares(n) -> Net` from SPEC-09 R17d.
- `ChurchArithmeticCodec` and `ChurchOp` already exist — see git history at HEAD.

## Notes

- This is primarily an audit-and-document task; if the existing codec already
  handles v3 R8 correctly, the task delivers tests + rustdoc only (no production
  changes). The Stage 4 reviewer agent verifies via inspection.
- T4 (all 690 v1 tests pass unchanged) is a CI gate, not a code change.
- This task does NOT add the `ChurchArithmeticCodec` to the registry — that's
  TASK-0716 (it is already registered; TASK-0716 only removes `lambda` and adds `horner`).
