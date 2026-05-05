# TEST-SPEC-0478: M5-scale bitmap free-list fallback (R32)

**SPEC-22 §7 ID:** none direct (M5-scale primitive); plus this plumbing file.
**Owning task:** TASK-0478.
**Parent spec:** SPEC-22 §3.5 R32 (closes SC-015).
**Type:** unit + benchmark (stress).

---

## Inputs / Fixtures

- A `FreeListRepr` newtype with two variants: `Vec(Vec<AgentId>)` and `Bitmap { bits: bitvec::BitVec, cursor: i64 }`.
- A test-only override of `MAX_FREELIST_BYTES` (e.g., `MAX_FREELIST_BYTES = 1024` instead of the default `64 * 1024 * 1024`) so the switch trigger fires at small `len`. Use `#[cfg(test)]` `const_set` or a feature flag.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0478-01 | `vec_to_bitmap_switch_at_threshold` | `FreeListRepr::Vec(vec![0; N])` with `N * 4 == MAX_FREELIST_BYTES + 1` (threshold-trigger size) | `repr.push(N as AgentId)` | the next `push` triggers the switch; post-state is `FreeListRepr::Bitmap { .. }`. The bitmap's bits represent the prior `N + 1` IDs. |
| UT-0478-02 | `bitmap_pop_preserves_lifo` | `FreeListRepr::Bitmap` with bits set at IDs `{2, 5, 7, 11}` (in push order — most recent push was 11; cursor == 11) | `repr.pop()` | returns `Some(11)`. Cursor moves to 10. Bit 11 is cleared. |
| UT-0478-03 | `bitmap_consecutive_pops_descend_lifo` | continued from UT-0478-02 | 3 successive pops | returns `Some(7), Some(5), Some(2)` in order. |
| UT-0478-04 | `bitmap_push_pop_cycle_round_trip` | `Bitmap { bits, cursor }`; push `[3, 7, 11]`; pop all 3 | popped sequence is `[11, 7, 3]` (LIFO). |
| UT-0478-05 | `bitmap_contains_o1` | `FreeListRepr::Bitmap` with 1M bits set | `repr.contains(id)` for arbitrary id | O(1) bit-test; runs in << 1 microsecond. |
| UT-0478-06 | `bitmap_len_correct_after_push_pop` | sequence of pushes and pops | `repr.len()` after each op | matches the count of set bits. |
| UT-0478-07 | `serde_emits_vec_form_regardless_of_repr` | `FreeListRepr::Bitmap` with bits at `{2, 5, 7}` | `bincode::serialize(&repr).unwrap()` | the bytes encode a `Vec<AgentId>` (compact wire format per TASK-0478 acceptance). Deserialization yields `FreeListRepr::Vec(vec![2, 5, 7])` (or push-time switched form, depending on implementation). |
| UT-0478-08 | `is_empty_after_drain` | bitmap with 100 bits set; pop all | `repr.is_empty() == true`; cursor reset. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Switch back from bitmap to Vec at low watermark (OPTIONAL per TASK-0478 notes) | If implemented, the switch-back fires when `len()` drops below 25% of threshold. (Skip this test if the OPTIONAL feature is not implemented.) |
| EC-2 | Push duplicate ID into bitmap | Bit is already set; `push` is idempotent (R6 naturally enforced). `len()` unchanged. |
| EC-3 | Pop from empty bitmap | Returns `None`; cursor stays at -1 (sentinel). |
| EC-4 | Push and pop interleaved at small scale (<10 entries) | Always uses `Vec` representation; switch never fires. (Documents the v1 path stays in `Vec`.) |
| EC-5 | rkyv round-trip under `--features zero-copy` | Bitmap form is serialized as Vec form (per the asymmetry documented in TASK-0478). |

## Invariants asserted

- R32 (M5-scale bitmap fallback when `len * 4 > MAX_FREELIST_BYTES`).
- R5 (LIFO preserved across the switch via cursor).
- R6 (no duplicates — bitmap idempotent set semantics).

## ARG/DISC/REF citation

- AC-006 (HVM2 arena management) — informs the LIFO preservation under bitmap.
- AC-009 (HVM4 unified heap) — bitmap as a packed representation of the heap-tag set.

## Determinism notes

Bitmap operations are deterministic. The cursor scanning-downward is deterministic. Pure synchronous; no tokio. The OPTIONAL switch-back-to-Vec is implementation-defined; if implemented, the watermark threshold is documented in the TASK-0478 acceptance criteria.

## Cross-test dependencies

- TEST-SPEC-0471 covers the Vec-only baseline; TEST-SPEC-0478 covers the bitmap fallback.
- This is a P2 OPTIONAL task per TASK-0478 priority; v1 implementations MAY skip the bitmap path entirely. M5 / 100M-scale implementations MUST ship it.
- TEST-SPEC-0500 regression gate verifies the Vec-only path stays unaffected.
