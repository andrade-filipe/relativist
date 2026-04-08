# Reviews -- TASK-0022: Implement normalize_pair function

**Date:** 2026-04-08

---

## Code Cleaner: **PASS** -- Function is `#[inline]`, 7 lines of logic, clear doc comment referencing SPEC-03 Section 4.4 and R9. Lists all 4 dispatch guarantees (anni, comm, eras, void). Uses `as u8` cast for symbol comparison, consistent with `repr(u8)` on `Symbol`. Naming is descriptive (`normalize_pair`, `sym_a`, `sym_b`).
## Architecture: **PASS** -- SPEC-03 R9 (pair normalization, sym_a <= sym_b) satisfied. Lives in `src/reduction/dispatch.rs` alongside the dispatch tables it supports. O(1) complexity (two arena lookups + one comparison). The `.unwrap()` on `net.agents[id]` is intentional and safe: `normalize_pair` is only called after `is_valid_redex` confirms both agents exist in `reduce_step`. Re-exported via `mod.rs` for flat access.
## QA: **PASS** -- 3 tests with full coverage: T12 (already-ordered: Con-Dup, Con-Era, Dup-Era unchanged), T13 (reversed: Dup-Con, Era-Con, Era-Dup swapped), T14 (equal symbols: Con-Con, Dup-Dup, Era-Era unchanged). All 9 symbol combinations tested. The only panic path is the `.unwrap()` on a missing agent, which is a logic error in the caller (caught by debug assertions in `reduce_step`).
