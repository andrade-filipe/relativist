# TASK-0724 — D-016 biguint_readback: handle nested Horner composition for degree ≥ 2

**Spec:** SPEC-27 v3 (`specs/SPEC-27-encoder-decoder-api.md`) — R14' (BigUint readback) + R16b' (cross-check with oracle)
**Requirements:** R14' (readback extension), R16b' (oracle agreement), G1 evidence for non-trivial polynomials
**Priority:** P0 (unblocks Demo 4/5 of `docs/demos/horner-g1-demonstration.md`; second of 3 known decoder gaps)
**Status:** TODO
**Depends on:** TASK-0723 (the cofactor c_i ≥ 2 fix is a structural prerequisite — nested Horner produces nested cofactor DUPs)
**Blocked by:** TASK-0723
**Estimated complexity:** M (~140 LoC production + recursion-depth audit)
**Bundle:** D-016 — HornerCodec decoder extension

---

## Context

After TASK-0723 lands, `decode_biguint` correctly reads back any
**single-iteration** Horner output — i.e., `coeffs.len() == 2` with
arbitrary `c_0, c_1, x` in bounds. The next decoder gap is **degree ≥ 2**.

The two known failure cases that this TASK closes:

```
$ relativist compute --codec horner --input '{"coeffs":[1,1,1],"x":2}'
error: encoding error: unrecognized net structure: non-CON in app chain
                                                       # expected 1+1*2+1*4 = 7

$ relativist compute --codec horner --input '{"coeffs":[1,0,1],"x":3}'
error: encoding error: unrecognized net structure: non-CON in app chain
                                                       # expected 1+0+1*9 = 10
```

Why this fails (even after TASK-0723): the encoder runs the Horner
recurrence `acc_{k} = acc_{k+1} * x + coeffs[k]`. Each iteration `k`
wraps the previous accumulator in **one more mul + add layer**, which
post-reduction yields a Church chain whose path from the outer
`lambda_x.p2` to the `x` binding crosses **k nested DUP frames**, one
per iteration. The TASK-0723 helper handles ONE such frame (the case
`n = 1`). For `n >= 2`, the helper must recurse into the nested
multiplications produced by the inner accumulators.

Demo failure surface in scope for this task:
- `{"coeffs":[1,1,1],"x":2}` → expected `1 + 1·2 + 1·4 = 7`. (canonical degree-2)
- `{"coeffs":[1,0,1],"x":3}` → expected `1 + 0 + 1·9 = 10`. (sparse degree-2)
- `{"coeffs":[3,2,5,1],"x":2}` → expected `35` (matches existing oracle assertion in `horner.rs::horner_encode_canonical_case_matches_oracle`). (degree-3)
- `{"coeffs":[1,0,0,0,0,1],"x":10}` → expected `100001` (matches existing oracle assertion). (degree-5 sparse)
- `{"coeffs":[1,1,1,1,1],"x":2}` → expected `31` (geometric, degree-4).
- `{"coeffs":[1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1],"x":10}` → expected `1111111111111111111111111` (T9 BigUint witness from PT-0715-03 — DEGREE 24). MUST decode after this task to retire the "T9 readback is a v1 limitation" doc-comment.

**Boundary**: the existing `MAX_CHURCH_NAT = 10_000` cap still applies
to each `coeffs[i]` and to `x`. The result `p(x)` may exceed `u64::MAX`
(this is exactly the point of `BigUint` readback — T9 witness).

## Acceptance Criteria

- [ ] `decode_biguint` on the NF of `HornerCodec::encode({"coeffs":[c0, ..., cn], "x":x})` returns `Ok(BigUint::from(p(x)))` for any `n >= 2` with `coeffs[i], x` all in bounds. Verified against `horner_serial`.
- [ ] All 6 demo inputs above decode to the expected values when run through `HornerCodec` + `reduce_all` + `decode_biguint`.
- [ ] The T9 BigUint witness `{"coeffs":[1; 25],"x":10}` decodes to `"1111111111111111111111111"` (NOT a `DecodeError::UnrecognizedStructure`). PT-0715-03 is upgraded from "encode + validate only" to a full pipeline cross-check.
- [ ] Recursion depth on `decode_biguint` MUST tolerate at least `coeffs.len() == 64` without exhausting the stack. The existing `if depth > 64` guard in `count_chain_through_dups` and `chain_from_dup_branch` is reviewed and either raised, switched to an explicit stack-based traversal, or documented as the de facto upper bound for HornerCodec inputs.
- [ ] No regression on TASK-0723's behavioural envelope (single-iteration, c_1 >= 2). Run the TASK-0723 unit tests as part of this TASK's gate.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/encoding/biguint_readback.rs` | modify | Extend `chain_via_mul_subnet` (or whatever helper TASK-0723 introduced) to recurse through nested `wire_mul_into` outputs. The recursion structure mirrors the Horner encoder's `for k in (0..n).rev()` loop — each level of nesting corresponds to one mul + add scaffold. ~100-120 LoC. |
| `relativist-core/src/encoding/biguint_readback.rs` | modify | Add 4 new unit tests covering the demo inputs (degree 2 dense, degree 2 sparse, degree 3 canonical, degree 5 sparse). |
| `relativist-core/src/encoding/horner.rs` | modify | Promote PT-0715-03 (`horner_pipeline_biguint_range_25_coeffs`) from "encode + validate only" to **full pipeline cross-check** against `horner_serial`. Also strengthen UT-0714-02, UT-0714-03, UT-0714-05 to assert `reduce_and_decode` returns the expected value (currently several of these only assert `validate_encoded_net`). ~30 LoC delta across multiple existing tests. |
| `relativist-core/src/encoding/horner.rs` | modify | Update PT-0715-06 input domain to allow `coeffs.len() in 2..=4` (currently restricted to `2..=2`) AND tighten the readback skip threshold further (target: ≤ 25% after this task). ~10 LoC delta. |
| `relativist-core/tests/horner_distributed_g1.rs` | modify | If any G1 distributed test in this file is currently gated on "single-iteration only" via the v1 readback limitation, broaden it to multi-iteration. Inspect the file first; do NOT widen tests beyond the v1 readback envelope demonstrated by TASK-0723 + TASK-0724. ~20-30 LoC delta. |

## Key Types / Signatures

No new public API. May introduce a private recursive helper, e.g.:

```rust
/// Read back a nested Horner accumulator subnet rooted at `acc_port`.
/// Recursion mirrors the encoder loop: each call handles one
/// (mul, add) scaffold and recurses on the inner accumulator's port.
///
/// `remaining_depth` is the upper bound on encoder iterations the
/// caller still expects to see — used purely as a recursion guard,
/// NOT as an input to the semantics. The semantics fall out of the
/// agent graph topology alone.
fn chain_via_nested_horner(
    net: &Net,
    acc_port: PortRef,
    lam_x: AgentId,
    remaining_depth: usize,
) -> Result<BigUint, DecodeError>;
```

(Naming illustrative; developer chooses.)

## Test Expectations (for Stage 2 test-generator)

Maps to SPEC-27 v3 §7 (T7, T9 promoted from gated to full, T11, T12,
plus the G1 narrative in `discussoes/argumentos/ARG-001-...`).

- **UT-0724-01** — `decode_biguint_handles_degree_2_dense`: `{"coeffs":[1,1,1],"x":2}` → `Ok(7)`.
- **UT-0724-02** — `decode_biguint_handles_degree_2_sparse_zero_middle`: `{"coeffs":[1,0,1],"x":3}` → `Ok(10)`.
- **UT-0724-03** — `decode_biguint_handles_degree_3_canonical`: `{"coeffs":[3,2,5,1],"x":2}` → `Ok(35)`.
- **UT-0724-04** — `decode_biguint_handles_degree_5_sparse`: `{"coeffs":[1,0,0,0,0,1],"x":10}` → `Ok(100001)`.
- **UT-0724-05** — `decode_biguint_t9_biguint_witness_full_pipeline`: `{"coeffs":[1; 25],"x":10}` → `Ok(BigUint::parse_bytes(b"1111111111111111111111111", 10).unwrap())`; `bits() > 64`.
- **PT-0724-06** — `decode_biguint_matches_oracle_degree_2_to_4_property`: proptest over `coeffs.len() in 2..=4`, each `coeffs[i] in 0..=20`, `x in 0..=20`. 200 cases. Cross-check vs `horner_serial`. Skip rate MUST be 0%.
- **PT-0724-07** — `decode_biguint_recursion_depth_64_no_overflow`: `coeffs.len() == 64`, all-ones, `x == 2`. MUST NOT exhaust stack; MUST decode to `Ok(2^64 - 1)` (the geometric sum).

## Dependencies Context

- Builds DIRECTLY on TASK-0723's `chain_via_mul_subnet` (or equivalently-named helper). Read TASK-0723's final commit before starting.
- `horner_serial` (TASK-0713) — oracle.
- `reduce_all`, `discover_root` — already wired (TASK-0721 BUG-001).
- The Horner encoder loop in `relativist-core/src/encoding/horner.rs` lines 162-186 is the structural blueprint for the recursive readback.

## Notes

- The recursion depth concern is real: a polynomial with `coeffs.len() = 64`
  encodes a chain of 63 nested mul+add scaffolds. The existing `depth > 64`
  guard in `count_chain_through_dups` may need to become `depth > 256` or
  switch to an explicit `Vec<...>` work stack. Property test PT-0724-07
  forces the decision.
- Be careful with PT-0724-06's input grid: `coeffs.len() in 2..=4` with each
  coefficient ≤ 20 and `x` ≤ 20 keeps the reduced net under ~10k agents
  (encoder generates O(sum(coeffs) + x · n)). Larger grids slow proptest.
- DO NOT widen the input domain to `coeffs.len() > 4` in the property test
  without also adding a `release` cfg-gate — debug-mode tests already slow
  on the existing PT-0715-06 grid.
- The existing test comment in `biguint_readback.rs` lines 11-19 (`Topology
  relationship to decode_nat`) MUST be updated to reflect that v1
  readback now also handles nested Horner — strike the
  "Future Mackie/Pinto-style readback would close this gap" sentence or
  reframe it as "WAN-scale Mackie/Pinto would replace this recursive
  readback when bound by network latency, but is not required for
  HornerCodec correctness".

## Sequencing within D-016

This TASK lands SECOND, after TASK-0723. Then TASK-0725 (property test
expansion at full bounds + 95% coverage of the readable subset) builds
on the combined helper. TASK-0726 (doc cleanup) runs last.
