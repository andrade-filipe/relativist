# TASK-0488: `Send + Sync` compile-time assertions for `Net` and `SparseNet`

**Spec:** SPEC-22 §4.4 Send + Sync paragraph (closes SC-016).
**Requirements:** §4.4 mandates `static_assertions::assert_impl_all!(SparseNet: Send, Sync)` and the same for the post-SPEC-22 `Net`.
**Priority:** P1 (forward-compatibility for parallel `build_subnet`).
**Status:** TODO
**Depends on:** TASK-0471 (Net.free_list field exists), TASK-0486 (SparseNet struct exists).
**Blocked by:** none
**Estimated complexity:** S (~10 LoC production + 0 tests — compile-time check IS the test)
**Bundle:** SPEC-22 Arena Management — Phase D (SparseNet).

## Context

SPEC-22 introduces `SparseNet` containing `HashMap<AgentId, Agent>` and `HashMap<(AgentId, PortId), PortRef>` — both `Send + Sync` if their contents are. The current `Net` is `Send + Sync` because all fields are. SPEC-22 R22 proposes building partitions in parallel via `SparseNet`. If `build_subnet` is called in parallel for different partitions, each partition's `SparseNet` must be `Send`-able. R10b's protected_tombstones / border_entries_shadow (HashSet) and `RecyclePolicy` (enum with `Send + Sync` derives) preserve the Send + Sync guarantee on `Net` post-SPEC-22.

## Acceptance Criteria

- [ ] Add `static_assertions = "1"` to `relativist-core/Cargo.toml` if not already present.
- [ ] In `relativist-core/src/net/sparse.rs`, add: `static_assertions::assert_impl_all!(SparseNet: Send, Sync);`
- [ ] In `relativist-core/src/net/core.rs`, add: `static_assertions::assert_impl_all!(Net: Send, Sync);` (if not already present).
- [ ] Verify `cargo check` passes — the assertion failing is a compile-error.
- [ ] No runtime test — the compile-time check IS the test.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/Cargo.toml` | modify (if needed) | Add `static_assertions = "1"`. |
| `relativist-core/src/net/sparse.rs` | modify | `assert_impl_all!(SparseNet: Send, Sync);` |
| `relativist-core/src/net/core.rs` | modify | `assert_impl_all!(Net: Send, Sync);` (if missing). |

## Test Expectations

(None — compile-time check.)

## Invariants Touched

- None directly; forward-compatibility for parallelism.

## Notes

- This is a hygiene task. If `static_assertions` is already a dep, skip the Cargo.toml edit.

## DAG Links

- **Predecessors:** TASK-0471, TASK-0486.
- **Successors:** TASK-0492 (parallel build_subnet may rely on Send + Sync).
