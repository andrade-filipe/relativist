# TASK-0498: Safe-Rust-only audit — confirm SPEC-22 implementations contain no `unsafe` (R31)

**Spec:** SPEC-22 §3.5 R31 (closes SC-017).
**Requirements:** R31 (SPEC-22 implementations MUST be expressible in safe Rust — no `unsafe` blocks. Future bit-packed migration is SPEC-23's responsibility; SPEC-22 takes no `unsafe` boundary.).
**Priority:** P2 (hygiene; forward-compat handshake with SPEC-23).
**Status:** TODO
**Depends on:** TASK-0471, TASK-0472, TASK-0473, TASK-0486, TASK-0487, TASK-0489, TASK-0490 (every code-touching SPEC-22 task that could introduce `unsafe`).
**Blocked by:** none
**Estimated complexity:** S (~10 LoC audit + ~10 LoC CI grep)
**Bundle:** SPEC-22 Arena Management — Phase E (invariant amendments).

## Context

R31 affirms SPEC-22 takes no `unsafe` boundary. Any future bit-packed `Net.ports` / `SparseNet.ports` migration requires `unsafe transmute`-style accessors and is SPEC-23's responsibility. This task is the **audit**: verify every file modified by SPEC-22 tasks contains zero `unsafe` blocks.

## Acceptance Criteria

- [ ] Audit every file modified by SPEC-22 tasks (TASK-0471 through TASK-0497):
  - `relativist-core/src/net/core.rs`
  - `relativist-core/src/net/sparse.rs`
  - `relativist-core/src/net/free_list.rs`
  - `relativist-core/src/net/mod.rs`
  - `relativist-core/src/partition/helpers.rs`
  - `relativist-core/src/partition/types.rs`
  - `relativist-core/src/merge/engine.rs`
  - `relativist-core/src/merge/config.rs`
  - `relativist-core/src/error.rs`
  - `relativist-core/src/reduction/**/*.rs`
- [ ] Verify zero `unsafe` keyword occurrences (case-sensitive, whole-word) in each file.
- [ ] Add a CI grep step that fails if `unsafe` appears in any of the above files post-SPEC-22.
- [ ] If `unsafe` is introduced legitimately (e.g., by a SPEC-22-adjacent task), flag for SPEC-23 escalation.
- [ ] Document the affirmation in a comment at the top of `relativist-core/src/net/core.rs`: "// SPEC-22 R31: this module contains no `unsafe` blocks. Bit-packed migration is SPEC-23's responsibility."

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `.github/workflows/ci.yml` *(or `lint.yml`)* | modify | Add a `grep -r "unsafe" src/net/ src/partition/ src/merge/ src/reduction/` step that fails on match. |
| `relativist-core/src/net/core.rs` | modify (comment-only) | Add the R31 affirmation comment. |

## Test Expectations

TEST-SPEC-0498:
- CI smoke: introduce `unsafe { }` in `src/net/core.rs`; CI fails. Remove; CI passes. (Manual one-time verification.)

## Invariants Touched

- None at runtime.
- Forward-compat handshake with SPEC-23.

## Notes

- The `bitvec` crate (TASK-0478) MAY use `unsafe` internally; that's acceptable because it's an external crate, not SPEC-22 implementation code. The audit scope is `src/`, not deps.

## DAG Links

- **Predecessors:** TASK-0471, TASK-0472, TASK-0473, TASK-0486, TASK-0487, TASK-0489, TASK-0490.
- **Successors:** TASK-0500.
