# TASK-0711 — SPEC-27 v3 R13a': `wire_add_into` / `wire_mul_into` obligation validation (Phase 3a promotion)

**Spec:** SPEC-27 v3
**Requirements:** R13a' (composable arithmetic helpers obligation set)
**Priority:** P0 (HornerCodec encoder TASK-0714 calls these helpers directly)
**Status:** TODO
**Depends on:** none
**Blocked by:** none
**Estimated complexity:** S (~0 production LoC + ~80 LoC tests; promotion-and-validation)
**Bundle:** SPEC-27 Encoder/Decoder API + HornerCodec (Topic 2)

---

## Context

SPEC-27 v3 R13a' (closure of SC-013, Caminho A) requires the existence of
`pub(crate)` PortRef-based composable helpers `wire_add_into` /
`wire_mul_into` in `relativist-core::encoding::arithmetic`, with three
obligations: (1) T1-T7 invariant preservation, (2) reduction equivalence to
Church `m+n` / `m*n`, (3) `pub(crate)` privacy (NOT in SPEC-14 R3 export list).

**Q5 finding (SPEC-27 v3 §8):** the helpers **already exist** as
`pub(crate) fn wire_add_into(net: &mut Net, m_port: PortRef, n_port: PortRef)
-> AgentId` and `pub(crate) fn wire_mul_into(...)` in `arithmetic.rs:92` and
`arithmetic.rs:224` (introduced for SPEC-09 R17d `church_sum_of_squares`).
Their signatures already match R13a'. Phase 3a is therefore a
**promotion-and-validation** task, not a new-construction task: the implementer
adds direct test coverage that exercises the R13a' obligation set on synthetic
inputs (separate from the `build_add` / `build_mul` round-trips already tested),
and the reviewer/QA confirms the obligations are met by inspection.

If a future implementation review (Stage 4 reviewer agent) finds that the
existing helpers do NOT satisfy R13a''s obligations as stated, the implementer
MUST add new helpers under different names — but SPEC-27 v3 explicitly does NOT
assume that fallback is needed.

## Acceptance Criteria

- [ ] Direct unit tests added that invoke `wire_add_into(net, m_port, n_port)` and `wire_mul_into(net, m_port, n_port)` with synthetic Church-numeral sub-nets (built via `encode_church_into`) — separate from the existing `build_add` / `build_mul` round-trip tests at `arithmetic.rs:737, 745`.
- [ ] Tests assert obligation 1 (T1-T7 preservation): `validate_encoded_net(&net)` succeeds before AND after the helper call.
- [ ] Tests assert obligation 2 (reduction equivalence): for `m_port` rooted at `Church(m)` and `n_port` rooted at `Church(n)`, `decode_nat(reduce_all(net))` yields `m + n` (resp. `m * n`) for at least 5 distinct `(m, n)` pairs each, including `(0, 0)`, `(1, 1)`, `(7, 9)`, `(0, 5)`, `(5, 0)`.
- [ ] Privacy obligation 3 verified by inspection: the public re-export list in `relativist-core/src/encoding/mod.rs` does NOT include `wire_add_into` or `wire_mul_into` (already true at HEAD; no change required).
- [ ] Rustdoc on both helpers updated to cite SPEC-27 v3 R13a' in addition to SPEC-09 R17d.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/encoding/arithmetic.rs` | modify | Add R13a'-named test cases in the existing `#[cfg(test)] mod tests` block (~80 LoC). Update rustdoc on both helpers (~6 lines) to cite SPEC-27 v3 R13a'. |

## Key Types / Signatures

(Existing — no API changes.)

```rust
pub(crate) fn wire_add_into(net: &mut Net, m_port: PortRef, n_port: PortRef) -> AgentId;
pub(crate) fn wire_mul_into(net: &mut Net, m_port: PortRef, n_port: PortRef) -> AgentId;
```

## Test Expectations

For Stage 2 (test-generator) — direct R13a' tests separate from `build_add` / `build_mul`:

- `wire_add_into_preserves_t1_t7_for_distinct_subnets` — assert `validate_encoded_net` before + after.
- `wire_add_into_reduces_to_church_sum_for_5_pairs` — `(0,0), (1,1), (7,9), (0,5), (5,0)` mapped to `0, 2, 16, 5, 5`.
- `wire_mul_into_preserves_t1_t7_for_distinct_subnets`.
- `wire_mul_into_reduces_to_church_product_for_5_pairs` — `(0,0), (1,1), (3,4), (0,7), (7,0)` mapped to `0, 1, 12, 0, 0`.
- `wire_helpers_are_pub_crate_only` — compile-time check via a doc-test or
  module-private assertion: an external integration test in `tests/` that tries
  `use relativist_core::encoding::arithmetic::wire_add_into;` MUST fail to compile.
  (Implementation: add a `compile_fail` doc-test on the helper.)

These tests are net-additive to SPEC-27 v3 §7 catalog (no new T-id) but support
T6, T7, T11, T13 (HornerCodec integration tests) by validating the foundation.

## Dependencies Context

- `validate_encoded_net(net) -> Result<(), EncodeError>` from `traits.rs:84`.
- `encode_church_into(net, n) -> AgentId` from `church.rs` (SPEC-14 R4b).
- `decode_nat(net) -> Result<u64, DecodeError>` from `church.rs` (SPEC-14 §4.4).
- `reduce_all(net)` from `relativist-core::reduction`.

## Notes

- The existing `test_wire_add_into_port_based_preserves_build_add` (line 737) and
  `test_wire_mul_into_port_based_preserves_build_mul` (line 745) tests confirm
  that the thin wrappers `build_add`/`build_mul` produce the same NF as the
  composable helpers when called on `(a, b)` operands. This task adds the
  *direct-helper* coverage that R13a' demands (separate from the `build_*`
  wrappers).
- This task does NOT promote the helpers to `pub` (privacy obligation 3 keeps
  them `pub(crate)`). HornerCodec (TASK-0714) lives in the same crate
  (`relativist-core::encoding::horner`) and can call them directly.
- If the Stage 4 reviewer or Stage 5 QA finds that the existing helpers do NOT
  satisfy obligations 1 or 2 on edge cases, the developer files a follow-up
  task to add new helpers under different names; SPEC-27 v3 does not amend
  SPEC-14 in either case.
