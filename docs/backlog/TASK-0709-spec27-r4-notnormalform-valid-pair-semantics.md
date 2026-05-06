# TASK-0709 — SPEC-27 v3 R4: `NotNormalForm.redexes` valid-pair semantics + I4 prune helper

**Spec:** SPEC-27 v3
**Requirements:** R4 (updated v3 — `NotNormalForm.redexes` semantics tied to SPEC-01 I4), R5, R6
**Priority:** P0 (foundational — every Decoder impl in v3 depends on the corrected semantics)
**Status:** TODO
**Depends on:** none
**Blocked by:** none
**Estimated complexity:** S (~40 LoC production + ~30 LoC tests)
**Bundle:** SPEC-27 Encoder/Decoder API + HornerCodec (Topic 2)

---

## Context

SPEC-27 v3 R4 closes Round 1 SC-005 by tightening the semantics of
`DecodeError::NotNormalForm.redexes`: the field MUST count **valid** active pairs
after stale-entry pruning per SPEC-01 I4, NOT `net.redex_queue.len()`. The
existing `traits.rs` definition (`#[error("net is not in normal form (has
{redexes} redexes)")]`) does not contradict the v3 wording, but the helper that
populates `redexes` is currently absent from `relativist-core`. Without a
canonical `count_valid_active_pairs(&Net)` helper, every future decoder
(HornerCodec in TASK-0712, `decode_biguint` in TASK-0712, plus existing
`LambdaCodec`) is at risk of false-positive `NotNormalForm` errors when a
freshly merged distributed net carries stale queue entries (T13 distributed
pipeline; SPEC-05 cross-partition merges).

This task introduces the helper and updates the existing `DecodeError::NotNormalForm`
documentation to reference SPEC-27 v3 R4 explicitly. It does NOT change the error
variant signature (`{ redexes: usize }`).

## Acceptance Criteria

- [ ] `pub(crate) fn count_valid_active_pairs(net: &Net) -> usize` added to `relativist-core::reduction` (or `relativist-core::encoding::traits`, developer's choice; the helper MUST live in a single canonical module).
- [ ] Helper prunes stale entries in `net.redex_queue` per SPEC-01 I4 (an entry `(a, b)` is valid iff both `a` and `b` are live agents AND their principal ports are mutually connected at port 0).
- [ ] `DecodeError::NotNormalForm` rustdoc cites SPEC-27 v3 R4 and SPEC-01 I4 explicitly; the variant signature is unchanged.
- [ ] No existing test regresses; v1 floor (690) inviolable.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/encoding/traits.rs` | modify | Update `DecodeError::NotNormalForm` rustdoc to cite SPEC-27 v3 R4 + SPEC-01 I4. Add `pub use` for the helper if it lives in `reduction`. |
| `relativist-core/src/reduction/mod.rs` (or appropriate sub-module) | modify | Add `pub(crate) fn count_valid_active_pairs(net: &Net) -> usize`. May reuse the standard valid-redex detector already used by `reduce_all`. |

## Key Types / Signatures

```rust
/// SPEC-27 v3 R4 + SPEC-01 I4: counts active pairs in `net.redex_queue` AFTER
/// stale-entry pruning. An entry `(a, b)` is valid iff both `a` and `b` are
/// live agents AND `net.get_target(AgentPort(a, 0)) == AgentPort(b, 0)` AND
/// `net.get_target(AgentPort(b, 0)) == AgentPort(a, 0)`. Returns the count of
/// valid pairs (NOT `net.redex_queue.len()`).
pub(crate) fn count_valid_active_pairs(net: &Net) -> usize;
```

## Test Expectations

For Stage 2 (test-generator):

- `count_valid_pairs_excludes_stale_after_remove_agent` — build a net with one
  redex, remove one of the two agents, assert helper returns 0 even though
  `redex_queue.len() == 1`.
- `count_valid_pairs_includes_live_redex` — fresh net with one true redex returns 1.
- `count_valid_pairs_zero_on_normal_form` — reduce a small net to NF, assert helper returns 0.
- Maps to SPEC-27 v3 §7.1 implicitly (T1, T2 decode contract validation also exercise the helper indirectly).

## Dependencies Context

- `Net::get_target(PortRef) -> PortRef`, `Net::is_live(AgentId) -> bool` (SPEC-02 R10/R11).
- `redex_queue: VecDeque<(AgentId, AgentId)>` field exists on `Net` (SPEC-02).
- SPEC-01 I4 stale-entry pruning rule already documented in SPEC-01.

## Notes

- Phase 1 of SPEC-27 v3 §6 ("Traits") is essentially already shipped. This task
  is the v3 *delta* — it only addresses the SC-005 closure. No trait signature changes.
- Whether the helper lives in `reduction` or `encoding::traits` is editorial; the
  rustdoc tagline must be the same in either case.
- The helper MUST be `pub(crate)` (R13a' privacy convention; HornerCodec lives in
  the same crate so `pub(crate)` is sufficient).
