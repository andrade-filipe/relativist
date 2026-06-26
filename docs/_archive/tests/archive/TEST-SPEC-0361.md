# TEST-SPEC-0361: `BorderGraph::from_partition_plan` constructor

**Task:** TASK-0361
**Spec:** SPEC-19 §3.2 (R10; §4.2 pseudocode)
**Generated:** 2026-04-17
**Baseline before this task:** 905+ lib (post-TASK-0360)
**Cumulative target after this task:** 911+ lib (≥ +6 new tests)

---

## Scope note

TASK-0361 implements the constructor that lifts a `PartitionPlan` into a
fully-populated `BorderGraph`. Per R10, the constructor MUST:

1. Iterate all `plan.partitions` once (O(B)) and collect the two
   sightings (owner `WorkerId` + `PortRef`) of every `border_id`
   observed in `partition.free_port_index`.
2. Validate that every `border_id` declared in `plan.borders` has
   **exactly 2** sightings (partitioning C3 invariant). Panic with a
   clear message on violation.
3. Populate `borders: HashMap<u32, BorderState>` with one entry per
   validated border, computing `is_redex` via `is_principal_pair`.
4. Populate `worker_borders: Vec<Vec<u32>>` (indexed by `worker_id as
   usize`, sized `plan.partitions.len()`); each border's `border_id`
   is pushed into both `worker_borders[wa]` and `worker_borders[wb]`.
5. Seed `active_redexes: HashSet<u32>` with exactly those `border_id`s
   where `is_redex == true`.

The tests are inline `#[cfg(test)] mod tests` extensions in
`border_graph.rs`. Each constructs a synthetic `PartitionPlan` via a
shared fixture helper (introduced below as `make_plan_*`) and asserts
on the resulting `BorderGraph`'s public observable state — which, since
the fields are `pub(crate)`, means module-local field reads from inside
the `#[cfg(test)]` block (valid because the tests live in the same
crate).

**HashMap iteration order note (per TASK-0361 line 222-229):** because
the sighting collection walks a `HashMap`, the `(side_a, side_b)`
assignment is arbitrary. Tests MUST assert **set-equality**
(`{worker_a, worker_b} == {0, 1}` via a `HashSet<WorkerId>` built from
the two fields), NOT positional equality, to prevent flakiness.

---

## Test target file paths

- `relativist-core/src/merge/border_graph.rs` — extend inline
  `#[cfg(test)] mod tests` block. New fixture helpers live at the top
  of this `mod tests` (or in a sub-module `mod fixtures`).
- No changes to any other file.

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Shared fixtures (specified once; tests reference them)

```rust
use crate::partition::types::{Partition, PartitionPlan, WorkerId};
use crate::net::{Net, PortRef};
use std::collections::HashMap;

/// Build a PartitionPlan with one or more partitions and an optional
/// borders map. Each `(worker_id, free_port_entries)` tuple becomes
/// one Partition whose `free_port_index` is populated.
/// `border_decls` is the top-level `plan.borders` map: border_id ->
/// (original_left, original_right) — the tests only care about the keys
/// for C3 validation, so the values can be `(PortRef::FreePort(0),
/// PortRef::FreePort(0))` dummies unless otherwise noted.
fn make_plan(
    partitions: Vec<(WorkerId, Vec<(u32, PortRef)>)>,
    border_decls: Vec<u32>,
) -> PartitionPlan;

/// Convenience: principal port on agent `id` at slot 0.
fn p(id: u32) -> PortRef { PortRef::AgentPort(id, 0) }

/// Convenience: auxiliary port on agent `id` at slot `slot`.
fn aux(id: u32, slot: u8) -> PortRef { PortRef::AgentPort(id, slot) }
```

The test-generator leaves the exact `Partition` struct-literal form to
the developer (the dependency-context note in TASK-0361 confirms the
fields exist in `partition/types.rs`); the assertions below depend only
on the observable `BorderGraph` state, not on `Partition`'s internals.

---

## Unit Tests

### UT-0361-01: `from_partition_plan_empty_zero_borders`

**Purpose:** Zero partitions' worth of borders ⇒ the graph is fully
empty. Baseline sanity.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
// One partition, worker_id = 0, empty free_port_index, no borders.
let plan = make_plan(vec![(0, vec![])], vec![]);
```

**When:** `let graph = BorderGraph::from_partition_plan(&plan);`

**Then:**
```rust
assert!(graph.borders.is_empty(),
        "zero-border plan must produce empty borders map");
assert_eq!(graph.worker_borders.len(), 1,
        "worker_borders is sized to partitions.len() (1) even with no borders");
assert!(graph.worker_borders[0].is_empty(),
        "no borders means no entries in worker_borders[0]");
assert!(graph.active_redexes.is_empty(),
        "no borders means no active redexes");
```

**SPEC-19 R covered:** R10 (empty case, structural init).

---

### UT-0361-02: `from_partition_plan_single_principal_principal_border_marks_redex`

**Purpose:** A single border whose two sides are both principal ports
MUST be marked as a redex: present in `active_redexes` and
`state.is_redex == true`.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
// 2 partitions (workers 0 and 1). Border 42 sighted on both:
//   partition 0: free_port_index[42] = AgentPort(7, 0)   (principal)
//   partition 1: free_port_index[42] = AgentPort(8, 0)   (principal)
let plan = make_plan(
    vec![
        (0, vec![(42, p(7))]),
        (1, vec![(42, p(8))]),
    ],
    vec![42],
);
```

**When:** `let graph = BorderGraph::from_partition_plan(&plan);`

**Then:**
```rust
assert_eq!(graph.borders.len(), 1);
assert!(graph.borders.contains_key(&42));
let state = graph.borders.get(&42).expect("border 42 present");

// is_redex derivation (R9).
assert!(state.is_redex,
        "principal vs principal sides must yield is_redex = true");

// active_redexes seeding.
assert!(graph.active_redexes.contains(&42),
        "redex border must be in active_redexes");
assert_eq!(graph.active_redexes.len(), 1);

// Owner assignment: side positions are arbitrary (HashMap iteration
// order), so assert the SET of workers is {0, 1} rather than positional.
let owners: std::collections::HashSet<WorkerId> =
    [state.worker_a, state.worker_b].iter().copied().collect();
assert_eq!(
    owners,
    std::collections::HashSet::from([0, 1]),
    "the two owners of the border MUST be workers 0 and 1"
);

// worker_borders reverse index: both workers see border 42.
assert!(graph.worker_borders[0].contains(&42));
assert!(graph.worker_borders[1].contains(&42));
```

**SPEC-19 R covered:** R10 (populate all `BorderState` entries), R9
(`is_redex` derivation), §4.1 (worker_borders reverse index).

---

### UT-0361-03: `from_partition_plan_principal_aux_is_not_redex`

**Purpose:** A border whose side A is principal and side B is
auxiliary MUST NOT be marked as a redex.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let plan = make_plan(
    vec![
        (0, vec![(7, p(3))]),       // principal side
        (1, vec![(7, aux(4, 1))]),  // auxiliary side (slot 1)
    ],
    vec![7],
);
```

**When:** `let graph = BorderGraph::from_partition_plan(&plan);`

**Then:**
```rust
assert_eq!(graph.borders.len(), 1);
let state = graph.borders.get(&7).expect("border 7 present");
assert!(!state.is_redex,
        "principal vs auxiliary must NOT be a redex");
assert!(graph.active_redexes.is_empty(),
        "no active redexes when no principal-pair borders exist");
```

**SPEC-19 R covered:** R10 + R9.

---

### UT-0361-04: `from_partition_plan_two_borders_worker_borders_all_populated`

**Purpose:** Multi-border fixture pins that `worker_borders` is
populated for **every** border the worker participates in, not just
the first.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
// 2 partitions share 2 borders; worker 0 and worker 1 each see both.
//  border 10: worker 0 side principal, worker 1 side auxiliary → not a redex
//  border 20: worker 0 side auxiliary, worker 1 side principal → not a redex
let plan = make_plan(
    vec![
        (0, vec![(10, p(0)),      (20, aux(1, 1))]),
        (1, vec![(10, aux(2, 1)), (20, p(3))]),
    ],
    vec![10, 20],
);
```

**When:** `let graph = BorderGraph::from_partition_plan(&plan);`

**Then:**
```rust
assert_eq!(graph.borders.len(), 2);
assert_eq!(graph.worker_borders.len(), 2);
assert_eq!(graph.worker_borders[0].len(), 2,
        "worker 0 participates in both borders");
assert_eq!(graph.worker_borders[1].len(), 2,
        "worker 1 participates in both borders");
let worker0: std::collections::HashSet<u32> =
    graph.worker_borders[0].iter().copied().collect();
assert_eq!(worker0, std::collections::HashSet::from([10, 20]));
let worker1: std::collections::HashSet<u32> =
    graph.worker_borders[1].iter().copied().collect();
assert_eq!(worker1, std::collections::HashSet::from([10, 20]));

// Neither border is a redex (mixed principal/aux).
assert!(graph.active_redexes.is_empty());
```

**SPEC-19 R covered:** R10 (multi-border population), §4.1.

---

### UT-0361-05: `from_partition_plan_mixed_redex_and_non_redex_seeds_active_set_correctly`

**Purpose:** When some borders are redexes and some are not, only the
redex ones appear in `active_redexes`. Cross-check against the
`is_redex` field on each `BorderState`.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
// 3 borders across 2 workers:
//  100: principal  / principal  → redex
//  101: principal  / auxiliary  → not a redex
//  102: principal  / principal  → redex
let plan = make_plan(
    vec![
        (0, vec![(100, p(0)), (101, p(1)), (102, p(2))]),
        (1, vec![(100, p(10)), (101, aux(11, 1)), (102, p(12))]),
    ],
    vec![100, 101, 102],
);
```

**When:** `let graph = BorderGraph::from_partition_plan(&plan);`

**Then:**
```rust
assert_eq!(graph.borders.len(), 3);
assert!(graph.borders.get(&100).unwrap().is_redex);
assert!(!graph.borders.get(&101).unwrap().is_redex);
assert!(graph.borders.get(&102).unwrap().is_redex);

// active_redexes invariant: equals the set of border_ids with is_redex=true.
let expected: std::collections::HashSet<u32> =
    std::collections::HashSet::from([100, 102]);
let actual: std::collections::HashSet<u32> =
    graph.active_redexes.iter().copied().collect();
assert_eq!(actual, expected,
        "active_redexes must equal {{bid : borders[bid].is_redex == true}}");
```

**SPEC-19 R covered:** R10, R9, R18 (incremental redex set seeded).

---

### UT-0361-06: `from_partition_plan_panics_on_orphan_border_in_plan_borders`

**Purpose:** `plan.borders` contains a `border_id` that is NOT present
in any partition's `free_port_index` ⇒ C3 invariant violation ⇒ panic
with a message naming the offending `border_id`.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
// Plan declares border 99 but neither partition sights it.
let plan = make_plan(
    vec![
        (0, vec![]),
        (1, vec![]),
    ],
    vec![99],
);
```

**When:** Call `BorderGraph::from_partition_plan(&plan)`.

**Then:**
```rust
#[test]
#[should_panic(expected = "99")]
fn from_partition_plan_panics_on_orphan_border() {
    let plan = make_plan(vec![(0, vec![]), (1, vec![])], vec![99]);
    let _ = BorderGraph::from_partition_plan(&plan);
}
```

**Assertions:**
- Panic substring contains `"99"` (the offending `border_id`).
- Panic substring contains either `"C3"` / `"invariant"` / `"sightings"`
  — test uses `#[should_panic(expected = "99")]` which is the **least
  brittle** form (matches any substring containing `99`); a stricter
  `expected = "border_id 99"` can be used if the panic wording is
  pinned verbatim in TASK-0361.

**SPEC-19 R covered:** R10 panic-on-C3-violation (orphan case).

---

### UT-0361-07: `from_partition_plan_panics_on_triple_sighting`

**Purpose:** The SAME `border_id` appears in THREE partitions'
`free_port_index` ⇒ C3 violation ⇒ panic naming the sighting count.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
// 3 partitions (workers 0, 1, 2) all claim border 55. Invalid.
let plan = make_plan(
    vec![
        (0, vec![(55, p(0))]),
        (1, vec![(55, p(1))]),
        (2, vec![(55, p(2))]),
    ],
    vec![55],
);
```

**When:** Call `BorderGraph::from_partition_plan(&plan)`.

**Then:**
```rust
#[test]
#[should_panic(expected = "3")]   // 3 sightings
fn from_partition_plan_panics_on_triple_sighting() {
    let plan = make_plan(
        vec![(0, vec![(55, p(0))]),
             (1, vec![(55, p(1))]),
             (2, vec![(55, p(2))])],
        vec![55],
    );
    let _ = BorderGraph::from_partition_plan(&plan);
}
```

**Assertions:**
- Panic substring contains `"3"` (the sighting count).
- Panic substring contains `"55"` (the offending `border_id`). If the
  developer's panic message only includes one of these, test MAY relax
  to `expected = "sightings"` — but the preferred contract pins both.

**SPEC-19 R covered:** R10 panic-on-C3-violation (triple-sighting case).

---

### UT-0361-08: `from_partition_plan_panics_on_orphan_free_port_entry_with_no_plan_border`

**Purpose:** The inverse C3 violation: a partition's `free_port_index`
has an entry whose `border_id` is NOT in `plan.borders`. This catches
a `split()` bug that leaves a stale `free_port_index` entry.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
// Partitions sight border 77, but plan.borders does NOT list it.
let plan = make_plan(
    vec![
        (0, vec![(77, p(0))]),
        (1, vec![(77, p(1))]),
    ],
    vec![],  // <- empty, no borders declared
);
```

**When:** Call `BorderGraph::from_partition_plan(&plan)`.

**Then:**
```rust
#[test]
#[should_panic(expected = "77")]
fn from_partition_plan_panics_on_orphan_free_port_entry() {
    let plan = make_plan(
        vec![(0, vec![(77, p(0))]), (1, vec![(77, p(1))])],
        vec![],
    );
    let _ = BorderGraph::from_partition_plan(&plan);
}
```

**Assertions:**
- Panic substring contains `"77"` (the orphan `border_id`).

**Note:** this test and UT-0361-06 together cover the two asymmetric
directions of the C3 orphan case (plan declares but partition omits;
partition declares but plan omits).

**SPEC-19 R covered:** R10 panic-on-C3-violation (inverse orphan case).

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| R10 (initialize from PartitionPlan, exactly-2-sightings invariant) | UT-0361-01, UT-0361-02, UT-0361-04, UT-0361-05, UT-0361-06, UT-0361-07, UT-0361-08 |
| R9 (`is_redex` derivation via `is_principal_pair`) | UT-0361-02 (positive), UT-0361-03 (negative), UT-0361-05 (mixed) |
| R18 (incremental `active_redexes` seeded during init) | UT-0361-02, UT-0361-05 |
| §4.1 `worker_borders` reverse index co-seeded | UT-0361-01 (empty), UT-0361-02 (single), UT-0361-04 (multi) |
| Panic on C3 violation (orphan-in-plan, triple-sighting, orphan-in-partition) | UT-0361-06, UT-0361-07, UT-0361-08 |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0361-A | PartitionPlan with 1000+ partitions and 10k borders | Stresses the O(B) complexity bound (R18); time budget per fixture-generation should stay sub-millisecond |
| QA-0361-B | `worker_id` value `u32::MAX` in a partition (near-boundary dense indexing) | Would overflow `worker_borders: Vec<Vec<u32>>` sized by `partitions.len()`; the spec assumes dense 0..N-1 indexing per SPEC-04 — any sparse or MAX-valued ID would panic on index. QA should test the panic path vs. a silent out-of-bounds write |
| QA-0361-C | Partition with `free_port_index` entry value `PortRef::FreePort(u32::MAX)` (DISCONNECTED sentinel) on construction | Is a DISCONNECTED sentinel at init time legal? The spec doesn't forbid it, but logically a freshly-split partition should not have DISCONNECTED free ports yet. QA probes the `is_principal_pair(DISCONNECTED, _) → false` path — which should yield `is_redex = false`, no panic |
| QA-0361-D | Same border_id declared in `plan.borders` twice (`vec![42, 42]`) | `make_plan` fixture may dedupe, but if the underlying `PartitionPlan.borders` is a `HashMap`, the duplicate is silently collapsed. QA should confirm the invariant holds regardless |
| QA-0361-E | Border with both sides sighted by the **same** worker | The spec §3.2 R8-R10 implicitly assumes two different workers per border (otherwise there is no inter-partition wire). If this fixture is constructed, the `{worker_a, worker_b}` set is `{x}` not `{0, 1}`. Current code would treat it as a valid border. QA should flag whether this is a C3 violation or a legitimate same-worker edge case — ask spec-critic if unclear |
| QA-0361-F | Fuzz: random `PartitionPlan` with consistent C3 (borders declared iff sighted twice) | Property test: the invariant `active_redexes == {bid : borders[bid].is_redex}` holds post-init for any valid plan. Can be expressed via `proptest` if the fixture helper supports it; leave as QA adversarial probe, NOT a required `#[test]` (keeps the suite deterministic) |

---

## Acceptance gate

1. `cargo test --workspace` count: 905 → **911+** (≥ +6; the 8 tests
   above, minus possible merging of the two panic-case tests into one
   parameterized test via `rstest`, yields 6-8 new unit tests).
2. Same +6 under `--features zero-copy`.
3. All previously passing tests still pass (no regression).
4. `cargo clippy --workspace --all-targets -- -D warnings` clean.
5. `cargo fmt --check` clean.
6. R19 grep guard still passes (no new `use tokio` / `use async` /
   `use crate::protocol` lines introduced).

---

## Out of scope (deferred to later TEST-SPECs)

- `apply_deltas` behaviour (single/double update, DISCONNECTED, redex
  creation/dissolution) → TEST-SPEC-0362.
- `detect_border_redexes` owned return shape → TEST-SPEC-0363.
- `remove_border` + `add_border_states` with `AddBorderEntry` →
  TEST-SPEC-0364.
- Module doc / R19 pure-core guard → TEST-SPEC-0365.
