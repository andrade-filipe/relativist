# TASK-0478: M5-scale bitmap free-list fallback (`bitvec::BitVec` representation)

**Spec:** SPEC-22 §3.5 R32 (closes SC-015).
**Requirements:** R32 (when `free_list.len() * 4 > MAX_FREELIST_BYTES = 64 MB`, switch from `Vec<AgentId>` to `bitvec::BitVec` representation; bitmap MUST preserve LIFO via high-water-mark cursor scanning downward on `pop`).
**Priority:** P2 (M5 / 100M-scale only; OPTIONAL for v1 implementations targeting <10M).
**Status:** TODO
**Depends on:** TASK-0471, TASK-0472, TASK-0473, TASK-0474 (`Vec`-based free-list fully working).
**Blocked by:** none
**Estimated complexity:** M (~120 LoC production + ~80 LoC tests; new dep `bitvec` if not already present)
**Bundle:** SPEC-22 Arena Management — Phase B (free-list core implementation; M5-scale feature).

## Context

R32 commits the v1 path to no-cap (40 MB at 10M is acceptable) AND commits the M5 path to a `bitvec::BitVec` representation when `free_list.len() * 4 > MAX_FREELIST_BYTES = 64 MB` (12.5 MB at 100M, vs 400 MB unbounded). The bitmap representation MUST preserve the LIFO contract via a high-water-mark cursor: the bitmap is a set; the cursor is the "most recently set" index, scanning downward on `pop`. R32 is "**MUST at M5; MAY at <10M**" — implementations MAY ship the `Vec` representation for v1 milestones; v2 / M5 implementations MUST ship the bitmap fallback.

## Acceptance Criteria

- [ ] Define an enum `FreeListRepr { Vec(Vec<AgentId>), Bitmap { bits: bitvec::BitVec, cursor: AgentId } }` (or equivalent) in `relativist-core/src/net/free_list.rs` (NEW MODULE).
- [ ] Implement `FreeListRepr::push(&mut self, id: AgentId)`, `FreeListRepr::pop(&mut self) -> Option<AgentId>`, `FreeListRepr::contains(&self, id: AgentId) -> bool`, `FreeListRepr::len(&self) -> usize`, `FreeListRepr::is_empty(&self) -> bool`.
- [ ] Implement automatic switching: in `push`, if the current representation is `Vec` AND `vec.len() * 4 > MAX_FREELIST_BYTES`, convert to `Bitmap`. (Switching back is OPTIONAL and out of scope here.)
- [ ] In `Bitmap` mode: `pop` scans downward from `cursor`, finds the highest set bit ≤ `cursor`, clears it, sets `cursor = found_index - 1`, returns the index. `push` sets the bit and updates `cursor = max(cursor, id)`.
- [ ] LIFO contract preserved across the switch: a `pop` after the switch returns the most-recently-pushed ID (i.e., the highest set bit since the most recent push).
- [ ] `MAX_FREELIST_BYTES` is a `pub const` (default 64 MB) with a clear Rustdoc citing R32.
- [ ] Replace `Net.free_list: Vec<AgentId>` with `Net.free_list: FreeListRepr` (transparent — all call sites use the trait-like API).
- [ ] Serde + rkyv: serialize as `Vec<AgentId>` always (compact wire format); deserialize as `Vec` and let push-time switching handle the bitmap conversion organically. Document this asymmetry.
- [ ] Test: stress test creating 100M (or simulated 100M via reduced threshold for test speed) agents, verify the switch happens, verify LIFO survives the switch.
- [ ] Test: round-trip — push 1M IDs (forces bitmap switch under reduced `MAX_FREELIST_BYTES`), pop them all, assert LIFO order.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/free_list.rs` | create | New module: `FreeListRepr` enum + push/pop/contains/len/is_empty. |
| `relativist-core/src/net/mod.rs` | modify | `pub mod free_list;` and re-export `FreeListRepr`, `MAX_FREELIST_BYTES`. |
| `relativist-core/src/net/core.rs` | modify | `pub free_list: FreeListRepr` (was `Vec<AgentId>`); update `new()`/`with_capacity()` to construct `FreeListRepr::Vec(Vec::new())`. |
| `relativist-core/Cargo.toml` | modify | Add `bitvec = "1"` dependency if not present. |

## Key Types / Signatures

```rust
pub const MAX_FREELIST_BYTES: usize = 64 * 1024 * 1024;  // 64 MB; SPEC-22 R32

pub enum FreeListRepr {
    Vec(Vec<AgentId>),
    Bitmap { bits: bitvec::BitVec, cursor: i64 },  // -1 when empty
}

impl FreeListRepr {
    pub fn new() -> Self { FreeListRepr::Vec(Vec::new()) }
    pub fn push(&mut self, id: AgentId);
    pub fn pop(&mut self) -> Option<AgentId>;
    pub fn contains(&self, id: AgentId) -> bool;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0478:
- `vec_to_bitmap_switch_at_threshold` (with reduced `MAX_FREELIST_BYTES` for test speed).
- `bitmap_pop_preserves_lifo`.
- `bitmap_push_pop_cycle_round_trip`.
- `bitmap_contains_o1`.
- `serde_emits_vec_form_regardless_of_repr`.

## Invariants Touched

- R5 (LIFO — preserved across the switch via the high-water-mark cursor).
- R6 (no duplicates — bitmap naturally enforces; setting a bit twice is idempotent).
- R32 (MUST at M5).

## Notes

- This task is OPTIONAL for v1 implementations. M5 / 100M-scale workloads MUST ship it.
- The cursor mechanism is the trickiest part; document it carefully. Reference Round 2 closure log §SC-015 for the design rationale.
- The implementation MAY also handle "shrink-back" from bitmap to `Vec` when `len()` drops below a low watermark (e.g., 25% of `MAX_FREELIST_BYTES`), but this is a follow-up optimization not required by R32.

## DAG Links

- **Predecessors:** TASK-0471, TASK-0472, TASK-0473, TASK-0474.
- **Successors:** none directly; consumed transitively at M5 milestones (out of SPEC-22 scope).
