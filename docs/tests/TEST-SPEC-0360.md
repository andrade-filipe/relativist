# TEST-SPEC-0360: `border_graph.rs` skeleton — `BorderState` + `BorderGraph` shell + `is_principal_pair` re-export + module wiring

**Task:** TASK-0360
**Spec:** SPEC-19 §3.2 (R8, R9, R19; §4.1 type sketch)
**Generated:** 2026-04-17
**Baseline before this task:** 905 lib (default) / 945 lib (`--features zero-copy`)
**Cumulative target after this task:** 905 lib (default) / 945 lib (`--features zero-copy`)
  — **+0 net new tests**, see "Scope note" below.

---

## Scope note

This is the **skeleton / type-foundation commit** for the SPEC-19 §3.2
bundle. After this task lands, the new file
`relativist-core/src/merge/border_graph.rs` exists with:

1. A `pub struct BorderState` (6 fields per R9) with
   `#[derive(Debug, Clone, PartialEq, Eq)]`.
2. A `pub struct BorderGraph` shell with the three private fields from
   spec §4.1 (`borders`, `worker_borders`, `active_redexes`), all
   `pub(crate)`, with `#[derive(Debug, Clone)]` (NOT `PartialEq` / `Eq`).
3. A `use super::helpers::is_principal_pair;` re-export (per spec-critic
   Additional observation #2 — the existing helper at
   `merge/helpers.rs:18` MUST be reused, NOT redefined). No fresh
   definition inside `border_graph.rs`.
4. A module-level `//!` doc block (extended later by TASK-0365).
5. `merge/mod.rs` gains `pub mod border_graph;` and
   `pub use border_graph::{BorderGraph, BorderState};`.

**Test count policy (spec-critic DC-2 cascade):** the original task
listed 7 `is_principal_pair` inline tests under `border_graph.rs`. Per
spec-critic Additional observation #2, the helper is **re-exported from
`merge/helpers.rs` rather than redefined**; the 7 inline tests either
already exist in `helpers.rs` lines 403-444 OR are migrated there if
they add coverage not already present. Net impact on the workspace test
count from TASK-0360 is therefore **+0** (the test contracts for
`is_principal_pair` live in `helpers.rs` and are exercised by the
existing tests). The tests below instead cover the **struct shape**
contracts that TASK-0360 uniquely introduces and that TASK-0361..0365
depend on.

No methods are implemented in this task; `BorderGraph` has no `impl`
block yet (those land in 0361..0364). The tests here verify the
**structural contracts** required for downstream tasks to compile.

---

## Test target file paths

- `relativist-core/src/merge/border_graph.rs` — inline `#[cfg(test)]
  mod tests` block covering struct shape, derivations, and helper
  re-export availability.
- `relativist-core/src/merge/helpers.rs` — **no change** (the
  `is_principal_pair` tests already live here per spec-critic ruling;
  if the developer finds a gap after migration, new cases can be added
  there without changing this TEST-SPEC's count).

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Unit Tests

### UT-0360-01: `border_state_has_exact_six_fields_in_r9_order`

**Purpose:** Compile-time contract that `BorderState` has **exactly**
the six fields R9 mandates, in the documented order (`border_id`,
`side_a`, `side_b`, `worker_a`, `worker_b`, `is_redex`), all `pub`.
Breaks compilation if any field is added, removed, renamed, or reordered
in a way that changes the struct-literal syntax.

**Target file:** `merge/border_graph.rs::tests`

**Given:** `BorderState`, `BorderGraph` types imported.

**When:** Construct a `BorderState` via full struct literal with all six
fields named.

**Then:**
```rust
let state = BorderState {
    border_id: 7,
    side_a: PortRef::AgentPort(1, 0),
    side_b: PortRef::AgentPort(2, 0),
    worker_a: 0,
    worker_b: 1,
    is_redex: true,
};
assert_eq!(state.border_id, 7);
assert_eq!(state.side_a, PortRef::AgentPort(1, 0));
assert_eq!(state.side_b, PortRef::AgentPort(2, 0));
assert_eq!(state.worker_a, 0);
assert_eq!(state.worker_b, 1);
assert!(state.is_redex);
```

**Assertions (concrete):**
- Struct literal compiles (field set is locked).
- All six field reads yield the written values (field types are correct).
- The code covers the R9 spec bullets (`border_id: u32`,
  `side_a: PortRef`, `side_b: PortRef`, `worker_a: WorkerId`,
  `worker_b: WorkerId`, `is_redex: bool`).

**SPEC-19 R covered:** R9 (full field set + types).

---

### UT-0360-02: `border_state_debug_derive_produces_non_empty_string`

**Purpose:** Verify `#[derive(Debug)]` is active — a missing `Debug`
derive on `BorderState` breaks several downstream tests in TASK-0361..0364
that use `assert_eq!` on tuples containing `BorderState`. Asserts the
`Debug` output contains the type name so the derive (not a manual impl
that stringifies differently) is in effect.

**Target file:** `merge/border_graph.rs::tests`

**Given:** `BorderState` with a distinguishable `border_id`.

**When:** Format with `{:?}`.

**Then:**
```rust
let state = BorderState {
    border_id: 42,
    side_a: PortRef::AgentPort(1, 0),
    side_b: PortRef::AgentPort(2, 0),
    worker_a: 0,
    worker_b: 1,
    is_redex: false,
};
let s = format!("{state:?}");
assert!(s.contains("BorderState"),
        "Debug output must contain type name `BorderState`; got {s}");
assert!(s.contains("42"),
        "Debug output must contain the border_id field value; got {s}");
```

**Assertions:**
- `Debug` is implemented (derive is active).
- Output contains the type name `BorderState`.
- Output contains the `border_id` value `42` (field rendering is active).

**SPEC-19 R covered:** R9 (derivation requirement — TASK-0360
acceptance criterion line 39).

---

### UT-0360-03: `border_state_clone_is_value_equal`

**Purpose:** Verify `#[derive(Clone, PartialEq, Eq)]` together — cloning
a `BorderState` yields a value that `==` compares equal to the original.
TASK-0363 (DC-3) and TASK-0362 both depend on `BorderState: Clone` for
owned-return and for fixture setup.

**Target file:** `merge/border_graph.rs::tests`

**Given:** Any `BorderState`.

**When:** Call `.clone()` and compare with `==`.

**Then:**
```rust
let state = BorderState {
    border_id: 99,
    side_a: PortRef::AgentPort(3, 0),
    side_b: PortRef::AgentPort(4, 1),
    worker_a: 0,
    worker_b: 2,
    is_redex: false,
};
let cloned = state.clone();
assert_eq!(state, cloned, "Clone + PartialEq must round-trip value equality");
```

**Assertions:**
- `Clone` derive is active.
- `PartialEq` derive is active.
- `Eq` does not fail compile (`assert_eq!` macro requires it for
  message formatting consistency across the suite).

**SPEC-19 R covered:** R9 derivation requirement.

---

### UT-0360-04: `border_state_inequality_when_any_field_differs`

**Purpose:** Pin the `PartialEq` semantics — changing any single field
makes the struct unequal to the original. Prevents a future "manual
`PartialEq` that ignores some field" drift from silently passing.

**Target file:** `merge/border_graph.rs::tests`

**Given:** A baseline `BorderState`.

**When:** Construct six mutated copies, each differing by exactly one
field.

**Then:**
```rust
let base = BorderState {
    border_id: 1, side_a: PortRef::AgentPort(1, 0), side_b: PortRef::AgentPort(2, 0),
    worker_a: 0, worker_b: 1, is_redex: true,
};
assert_ne!(base, BorderState { border_id: 2, ..base });
assert_ne!(base, BorderState { side_a: PortRef::AgentPort(9, 0), ..base });
assert_ne!(base, BorderState { side_b: PortRef::AgentPort(9, 0), ..base });
assert_ne!(base, BorderState { worker_a: 5, ..base });
assert_ne!(base, BorderState { worker_b: 5, ..base });
assert_ne!(base, BorderState { is_redex: false, ..base });
```

**Assertions:**
- Each single-field mutation is distinguishable via `PartialEq`.
- All six fields participate in equality (no accidental derive that
  skips `is_redex`, which is the most likely refactor bug).

**SPEC-19 R covered:** R9 (semantics of the derived field set).

---

### UT-0360-05: `border_graph_default_construction_is_empty`

**Purpose:** Document the initial-state expectation of the shell
(before `from_partition_plan` lands). The task does NOT ship an impl,
so the only available constructor is the struct literal using the
`pub(crate)` fields. Asserts all three collections are empty.

**Target file:** `merge/border_graph.rs::tests`

**Given:** A directly-constructed empty `BorderGraph` (visibility is
`pub(crate)` so construction inside the module works).

**When:** Inspect the three collections.

**Then:**
```rust
use std::collections::{HashMap, HashSet};
let graph = BorderGraph {
    borders: HashMap::new(),
    worker_borders: Vec::new(),
    active_redexes: HashSet::new(),
};
assert_eq!(graph.borders.len(), 0, "empty borders map on construction");
assert_eq!(graph.worker_borders.len(), 0, "empty worker_borders vec on construction");
assert_eq!(graph.active_redexes.len(), 0, "empty active_redexes set on construction");
```

**Assertions:**
- All three fields exist on the struct with the documented names.
- All three field types match spec §4.1 (`HashMap<u32, BorderState>`,
  `Vec<Vec<u32>>`, `HashSet<u32>`).
- All three have `.len()` accessors (compile-time check that the types
  are the documented collections, not newtype wrappers).

**SPEC-19 R covered:** R8 (struct existence), §4.1 (field shape).

---

### UT-0360-06: `border_graph_derive_shape_debug_and_clone`

**Purpose:** `BorderGraph` MUST derive `Debug` and `Clone` per
TASK-0360 acceptance criterion line 48, and MUST NOT derive `PartialEq`
/ `Eq`. The first two are positive contracts (downstream tests rely on
`Debug` for `assert_eq!` formatting and `Clone` for fixture snapshot
patterns); the last is a negative contract (deferred per spec-critic
DC-2 — equality on a `BorderGraph` is only meaningful after full
construction lands).

**Target file:** `merge/border_graph.rs::tests`

**Given:** A constructed empty `BorderGraph`.

**When:** Format with `{:?}` and call `.clone()`.

**Then:**
```rust
let graph = BorderGraph {
    borders: HashMap::new(),
    worker_borders: Vec::new(),
    active_redexes: HashSet::new(),
};
let s = format!("{graph:?}");
assert!(s.contains("BorderGraph"),
        "Debug output must contain type name `BorderGraph`; got {s}");
let cloned = graph.clone();
assert_eq!(cloned.borders.len(), 0);
assert_eq!(cloned.worker_borders.len(), 0);
assert_eq!(cloned.active_redexes.len(), 0);
```

**Negative contract (documented; NOT a test — enforcement is
compile-time in downstream code):** there is no `PartialEq` derive on
`BorderGraph`; any downstream attempt to compare two `BorderGraph`s
with `==` must fail to compile. The developer MUST NOT add
`#[derive(PartialEq, Eq)]` in this task — this is an enforcement
observation for review, not a runtime assertion.

**Assertions:**
- `Debug` derive is active on `BorderGraph`.
- `Clone` derive is active on `BorderGraph`.
- Cloning preserves all three field lengths (each underlying collection
  is `Clone`; the derive is not custom-implemented incorrectly).

**SPEC-19 R covered:** §4.1 (struct shape + derive policy).

---

### UT-0360-07: `is_principal_pair_is_reachable_via_helpers_reexport`

**Purpose:** Per spec-critic Additional observation #2, TASK-0360 MUST
re-export the existing `is_principal_pair` from `crate::merge::helpers`
rather than define a new copy. This test pins that `is_principal_pair`
is callable **inside the `border_graph.rs` test module** via the
documented `use super::helpers::is_principal_pair;` path. The test also
exercises the basic true/false contract so a developer-side refactor
that accidentally shadows the helper with a broken local copy fails
fast.

**Target file:** `merge/border_graph.rs::tests`

**Given:** `use crate::merge::helpers::is_principal_pair;` (or the
module-scoped `use super::helpers::is_principal_pair;`).

**When:** Call with principal-principal and principal-auxiliary pairs.

**Then:**
```rust
use crate::net::PortRef;
assert!(is_principal_pair(
    PortRef::AgentPort(1, 0),
    PortRef::AgentPort(2, 0),
), "principal vs principal MUST be true");
assert!(!is_principal_pair(
    PortRef::AgentPort(1, 0),
    PortRef::AgentPort(2, 1),
), "principal vs auxiliary MUST be false");
```

**Assertions:**
- `is_principal_pair` is accessible from `border_graph.rs` without a
  new definition (compile-time check of the re-export path).
- Positive case returns `true`.
- Negative (mixed principal/aux) case returns `false`.

**SPEC-19 R covered:** R9 (`is_redex` derivation via
`is_principal_pair`). **Spec-critic Additional observation #2 baked
in:** the helper is re-used, not redefined.

---

### UT-0360-08: `border_graph_module_is_wired_in_merge_mod`

**Purpose:** Verify `merge/mod.rs` declares `pub mod border_graph;` and
re-exports `BorderGraph` and `BorderState` at the `merge` level. Any
downstream task (0361..0365) imports `crate::merge::BorderGraph` or
constructs `crate::merge::BorderState`, so this test is the canary for
the `mod.rs` edit.

**Target file:** `merge/border_graph.rs::tests` (the test lives in the
child module but accesses the parent through `crate::merge`).

**Given:** `mod.rs` has been edited to add `pub mod border_graph;` and
`pub use border_graph::{BorderGraph, BorderState};`.

**When:** Compile a fully-qualified reference via `crate::merge::...`.

**Then:**
```rust
// Compile-time re-export assertions.
let _ct_borderstate: fn() -> crate::merge::BorderState = || BorderState {
    border_id: 0,
    side_a: PortRef::AgentPort(0, 0),
    side_b: PortRef::AgentPort(0, 0),
    worker_a: 0,
    worker_b: 0,
    is_redex: false,
};
let _ct_bordergraph: fn() -> crate::merge::BorderGraph = || BorderGraph {
    borders: HashMap::new(),
    worker_borders: Vec::new(),
    active_redexes: HashSet::new(),
};
// Runtime check that at least one of the constructors actually yields
// the documented type.
let bs: crate::merge::BorderState = _ct_borderstate();
assert_eq!(bs.border_id, 0);
```

**Assertions:**
- `crate::merge::BorderState` resolves (re-export is present).
- `crate::merge::BorderGraph` resolves (re-export is present).
- Both resolve to the same types as the module-local `BorderState` /
  `BorderGraph` (compile-time type identity).

**SPEC-19 R covered:** R8 (module wiring — existence of the
`BorderGraph` at a documented public path).

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| R8 (BorderGraph exists in `merge/` module) | UT-0360-05, UT-0360-06, UT-0360-08 |
| R9 (BorderState 6 fields + derivation + `is_redex` via `is_principal_pair`) | UT-0360-01, UT-0360-02, UT-0360-03, UT-0360-04, UT-0360-07 |
| R19 (pure-core; no `tokio`/`async`/`protocol`) | Grep guard (non-test, in acceptance criteria) + UT-0365-01 (landed in TEST-SPEC-0365) |
| §4.1 field shape (three `pub(crate)` fields) | UT-0360-05, UT-0360-06 |
| Spec-critic Additional obs. #2 (reuse `is_principal_pair` helper) | UT-0360-07 |
| Module re-export (`pub mod border_graph; pub use ...`) | UT-0360-08 |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0360-A | A future refactor renames `is_redex` to e.g. `redex_flag` on `BorderState` | UT-0360-01 struct literal breaks compile — canary fires; but QA should add a grep guard for the literal name `is_redex` in CI to catch silent rename-and-fixup patterns |
| QA-0360-B | A future refactor adds a 7th field to `BorderState` without updating UT-0360-01 | UT-0360-01 struct literal fails to compile (the literal is exhaustive); good fail-fast behaviour |
| QA-0360-C | A future refactor adds `#[derive(PartialEq, Eq)]` to `BorderGraph` | Nothing breaks — but per spec-critic DC-2, the derive is deliberately deferred. QA should flag the diff; code review should reject |
| QA-0360-D | A PR that re-introduces `fn is_principal_pair` inside `border_graph.rs` (shadowing the helper) | The re-export `use super::helpers::is_principal_pair;` would cause a naming conflict; compilation fails. Good — the spec-critic ruling is enforced by the compiler |
| QA-0360-E | Very large `BorderState` via struct-packed optimisation (`#[repr(packed)]`) accidentally applied | Any alignment-sensitive downstream test would surface; none exist in this bundle, but the derive contract (`Clone`, `Debug`) still holds so no test here fires. Leave as a Stage-5 architecture-review observation |
| QA-0360-F | `merge/mod.rs` adds `pub use border_graph::*;` instead of the explicit list | `*` re-export could leak `is_principal_pair` to external callers (breaking `pub(crate)` discipline); UT-0360-07 wouldn't catch it. Architecture review should grep for `pub use .*::\*` in `mod.rs` |

---

## Acceptance gate

1. `cargo test --workspace` count: 905 → **905** (+0 net; see Scope
   note — `is_principal_pair` tests live in `helpers.rs` by spec-critic
   ruling; the 8 unit tests above are a one-file addition that may
   grow the count to 913 if the developer writes them as independent
   `#[test]` functions — acceptance gate tolerates the 905+ floor but
   the concrete landing count is the developer's call based on whether
   they fold multiple assertions into a single `#[test]` or not).
2. `cargo test --workspace --features zero-copy` count: 945 → **945+**
   (same policy).
3. `cargo build --workspace` clean (no `border_graph.rs` imports from
   `tokio` / `async_trait` / `crate::protocol`; verified by grep
   guard listed in TASK-0360 acceptance criteria).
4. `cargo clippy --workspace --all-targets -- -D warnings` clean both
   with and without `--features zero-copy`.
5. `cargo fmt --check` clean.
6. Grep guard passes: `grep -E '^use\s+(tokio|crate::protocol|async_trait)'
   relativist-core/src/merge/border_graph.rs` returns zero matches.

---

## Out of scope (deferred to later TEST-SPECs in the bundle)

- `from_partition_plan` constructor behaviour → TEST-SPEC-0361.
- `apply_deltas` with DISCONNECTED handling → TEST-SPEC-0362.
- `detect_border_redexes` owned return → TEST-SPEC-0363.
- `remove_border` + `add_border_states` (Option B, AddBorderEntry) →
  TEST-SPEC-0364.
- Module `//!` doc + R19 pure-core guard contract → TEST-SPEC-0365.
