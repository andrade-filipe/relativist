# TASK-0723 — D-016 biguint_readback: handle Church mul output for coefficient c_i ≥ 2

**Spec:** SPEC-27 v3 (`specs/SPEC-27-encoder-decoder-api.md`) — R14' (BigUint readback) + R16b' (cross-check with oracle)
**Requirements:** R14' (readback extension), R16b' (oracle agreement)
**Priority:** P0 (unblocks Demo 2/3 of `docs/demos/horner-g1-demonstration.md`; first of 3 known decoder gaps)
**Status:** TODO
**Depends on:** none (extends existing `biguint_readback.rs` shipped by TASK-0712 / TASK-0721)
**Blocked by:** none
**Estimated complexity:** M (~120 LoC production + delta in existing helpers)
**Bundle:** D-016 — HornerCodec decoder extension

---

## Context

`HornerCodec::encode` produces a net whose Normal Form, post-`reduce_all`,
is **always a Church-numeral chain** under the canonical CON/DUP frame
(this is exactly what Lafont's confluence guarantees — same observable as
the oracle `horner_serial`). The encoder + reducer are correct in all
cases listed below.

The **decoder** (`relativist-core/src/encoding/biguint_readback.rs`,
`decode_biguint` → `count_chain_through_dups` → `chain_from_dup_branch`)
ships in HEAD a v1 traversal that handles only the "simplest" Horner
output — single iteration with `coeffs[1] == 1`. The first known
failure case is

```
$ relativist compute --codec horner --input '{"coeffs":[3,5],"x":4}'
error: encoding error: unrecognized net structure: non-CON in app chain
```

Why this fails: when `c_1 == 5` (not 1), the encoder builds
`prod = acc * x = Church(1) * Church(5)` and feeds the result via
`wire_add_into` to `Church(3)`. The reduced NF for that subnet exposes
the multiplication's DUP-share frame to the chain walk in a topology
the current `count_chain_through_dups` does not recognize: instead of a
clean Church chain of 23 CON applications (the expected value), the
traversal lands on a chain node whose principal is the DUP that
discharged the `x = 5` cofactor, and the walker errors at the next CON
agent because it expected port 1 (result) but saw port 2 (continuation
after DUP dispatch). The bug is in the **readback**, not the reducer.

Demo failure surface in scope for this task:
- `{"coeffs":[3,5],"x":4}` → expected `3 + 5*4 = 23`. (canonical case)
- `{"coeffs":[2,3],"x":2}` → expected `2 + 3*2 = 8`.
- `{"coeffs":[0,7],"x":3}` → expected `7*3 = 21`. (covers c_0 = 0)
- `{"coeffs":[10,2],"x":10000}` → expected `20010`. (high x, c_1 ≥ 2)

**Out of scope** for this TASK: degree ≥ 2 (covered by TASK-0724).
A `coeffs.len() == 2` input with `coeffs[1] >= 2` MUST decode after this
task lands.

## Acceptance Criteria

- [ ] `decode_biguint` on the NF of `HornerCodec::encode({"coeffs":[c0,c1],"x":x})` returns `Ok(BigUint::from(c0 + c1*x))` for every `(c0, c1, x)` with `c0, c1, x` in `0..=MAX_CHURCH_NAT`, `c1 >= 2` (the previously-broken cofactor case) — failing only on `c0 + c1*x > 10_000^2` if u64 overflow guards trip in a test helper (not in `decode_biguint` itself; `BigUint` has no overflow).
- [ ] The 4 demo inputs above all decode to the expected values when reduced + decoded via `HornerCodec`. Specifically `{"coeffs":[3,5],"x":4}` returns `{"value":"23","bit_length":5}`.
- [ ] When the v1 fast-path (c_1 == 1 single-iteration) covers a request, the new helpers MUST be functionally equivalent (no regression on TASK-0715 PT-0715-06 skip rate; in fact, the skip rate on the `1..=10 × 1..=10 × 1..=10` grid SHOULD drop sharply — see PT regression assertion below).
- [ ] All UnrecognizedStructure errors that fire in the post-fix decoder MUST carry a path-identifying message naming the specific helper (`count_chain_through_dups`, `chain_from_dup_branch`, or the new helper this task introduces) so QA can locate failures cheaply.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/encoding/biguint_readback.rs` | modify | Extend `count_chain_through_dups` + `chain_from_dup_branch` (or add a third recursive helper, e.g. `chain_via_mul_subnet`) to cross the DUP-share frame produced by `wire_mul_into` when `c_i >= 2`. Add inline rustdoc explaining the new traversal case and citing Mackie/Pinto §5 (already referenced in module doc). ~80-100 LoC production, plus 2-3 short helper-level rustdoc updates. |
| `relativist-core/src/encoding/biguint_readback.rs` | modify | Add 3 new unit tests covering the c_i ≥ 2 path (one per demo input listed above). Keep existing tests untouched. |
| `relativist-core/src/encoding/horner.rs` | modify | Strengthen PT-0715-06 (the `pt_0715_06_skip_rate_is_bounded` test) to assert the skip rate has **dropped below 50%** on the 1..=10 × 1..=10 × 1..=10 grid — empirically this should drop to **near 0%** after TASK-0723, but 50% is a robust regression gate that still leaves room for TASK-0724's domain (degree ≥ 2). ~3 LoC change to the threshold + comment explaining the new lower bound. |
| `docs/demos/horner-g1-demonstration.md` | NOT modified by this task | The "Limitações conhecidas" section gets cleaned up by TASK-0726 after both TASK-0723 and TASK-0724 land. |

## Key Types / Signatures

No new public API. The existing signature is preserved:

```rust
pub fn decode_biguint(net: &Net) -> Result<BigUint, DecodeError>;
```

A new private helper is acceptable; suggested signature:

```rust
/// Traverse a DUP-share frame produced by `wire_mul_into` and return the
/// Church-numeral count of the multiplied subnet rooted at `prod_port`.
/// Called from `count_chain_through_dups` when the principal-port walk
/// lands on a DUP whose two auxiliary destinations encode a Church
/// multiplication (a copy-and-iterate frame), not a simple share.
fn chain_via_mul_subnet(
    net: &Net,
    prod_port: PortRef,
    lam_x: AgentId,
    depth: usize,
) -> Result<BigUint, DecodeError>;
```

(Developer may choose a different internal factoring; the name above is
illustrative. The acceptance criteria are behavioural, not structural.)

## Test Expectations (for Stage 2 test-generator)

Maps to SPEC-27 v3 §7 (T7 pipeline parity, T11 oracle cross-check, T12
cross-readback). All tests live in `biguint_readback.rs` `#[cfg(test)]`
module unless noted.

- **UT-0723-01** — `decode_biguint_handles_c1_ge_2_canonical`: encode + reduce + decode `{"coeffs":[3,5],"x":4}` → `Ok(BigUint::from(23u64))`.
- **UT-0723-02** — `decode_biguint_handles_c1_ge_2_small_grid`: enumerate `(c0, c1, x)` in `(0..=5) × (2..=5) × (0..=5)` (216 cases); compare against `horner_serial`. Skip rate MUST be 0%.
- **UT-0723-03** — `decode_biguint_handles_c0_zero`: `{"coeffs":[0,7],"x":3}` → `Ok(BigUint::from(21u64))`.
- **UT-0723-04** — `decode_biguint_handles_boundary_max_x_with_c1_ge_2`: `{"coeffs":[10, 2],"x":10000}` → `Ok(BigUint::from(20010u64))`.
- **PT-0723-05** — `decode_biguint_matches_oracle_on_single_iteration_property`: proptest over `c0 in 0..=10_000`, `c1 in 2..=10_000`, `x in 0..=10_000`; cross-check vs `horner_serial`. 100 cases minimum. NO `Err` → skip allowed.
- **PT-0715-06 hardening** — bound the `pt_0715_06_skip_rate_is_bounded` threshold to ≤ 50% (down from ≤ 95%) AND ensure the test actually decreases on the 1..=10 × 1..=10 × 1..=10 grid; document the threshold in the rustdoc.

## Dependencies Context

- `HornerCodec::encode` (already in HEAD via TASK-0714 / TASK-0721) produces the net.
- `reduce_all(&mut net)` + `discover_root(&mut net)` (already wired in HEAD via TASK-0721 BUG-001).
- `count_valid_active_pairs` (TASK-0709) is unchanged; E1 check at the top of `decode_biguint` stays as-is.
- `horner_serial` (TASK-0713) is the oracle for cross-check tests.
- `wire_mul_into` and `wire_add_into` (existing in `relativist-core/src/encoding/arithmetic.rs`) determine the encoder-side topology this readback must traverse.

## Notes

- The encoder topology to study before writing the helper:
  `wire_mul_into(acc_port, x_port)` returns a CON whose port 1 is the
  multiplication result wire AND whose internal structure includes a DUP
  that copies the `acc` numeral `c_1` times. After `reduce_all`, the
  Church chain that represents the result is reachable from
  `AgentPort(prod_id, 1)` via a path that crosses one or more DUPs
  whose **two auxiliary destinations together form one consolidated
  Church chain** (not two independent chains the way `chain_from_dup_branch`
  assumes today). The new helper has to recognize this pattern.
- Strong recommendation: write the encoder + reducer fixture for the
  smallest failing case (`{"coeffs":[1,2],"x":2}`, expected 5) and
  inspect the post-reduction net via `net.debug_print()` (or
  `format!("{net:?}")`). The agent layout will tell you the exact DUP
  signature to dispatch on.
- The existing `chain_from_dup_branch` (lines 258-326 in
  `biguint_readback.rs`) treats DUPs as "transparent shares" — that
  assumption is RIGHT for the single-iteration c_1 == 1 case but WRONG
  for c_1 >= 2 because the DUP discharges multiple iterations of the
  same factor. The fix is to detect the DUP's role (share vs cofactor)
  by examining its principal-side context.
- DO NOT modify the `decode_nat`-side path in `church.rs`. The R14'
  Independence clause requires `decode_biguint` to remain independent;
  `decode_nat` stays as the canonical Church reader.
- Test floor delta after this TASK: +4 unit tests + 1 property test +
  tightened PT-0715-06 threshold (no count delta there). Estimated
  test count increase: +5.

## Sequencing within D-016

This TASK lands FIRST. TASK-0724 (degree ≥ 2) builds on the helper
introduced here and may further refactor it. TASK-0725 (property test
expansion) is downstream and verifies the combined cofactor + degree
fix.
