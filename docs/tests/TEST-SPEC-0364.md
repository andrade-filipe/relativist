# TEST-SPEC-0364: `BorderGraph::remove_border` + `add_border_states` (DC-4: `AddBorderEntry` input, graph recomputes `is_redex`)

**Task:** TASK-0364
**Spec:** SPEC-19 §3.2 (R15 part 3 — primitive; R16)
**Generated:** 2026-04-17
**Baseline before this task:** 927+ lib (post-TASK-0363)
**Cumulative target after this task:** 938+ lib (≥ +11 new tests per DC-4)

---

## Scope note

TASK-0364 lands three artefacts into `border_graph.rs`:

1. A new `pub struct AddBorderEntry` (5 connectivity fields — **NO
   `is_redex`**) with `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`.
   Per spec-critic DC-4, the caller provides only the connectivity;
   the graph computes `is_redex` via `is_principal_pair`.
2. A new method `pub fn remove_border(&mut self, border_id: u32) ->
   Option<BorderState>` (R16).
3. A new method `pub fn add_border_states(&mut self, entries:
   Vec<AddBorderEntry>)` (R15 part 3, primitive only).

**DC-4 verdict baked in:** every test below constructs
`AddBorderEntry` values (NOT `BorderState`). If a developer ships
Option A (caller passes `BorderState`, manages `is_redex`), the test
fixtures fail to compile on the missing type. **Test count per DC-4:
exactly 11** (10 from the original task + 1 new
`add_border_states_enforces_is_redex_invariant`).

Tests reuse fixtures from TEST-SPEC-0361 (`p`, `aux`, `make_plan`).
A fresh helper `make_empty_two_worker_graph()` is introduced for the
add tests since they start from an empty graph.

---

## Test target file paths

- `relativist-core/src/merge/border_graph.rs` — extend inline
  `#[cfg(test)] mod tests` block.

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Shared fixtures

```rust
/// 2-worker graph with NO borders. worker_borders is sized to 2.
fn make_empty_two_worker_graph() -> BorderGraph {
    // Construct via from_partition_plan with 2 empty partitions.
    let plan = make_plan(vec![(0, vec![]), (1, vec![])], vec![]);
    BorderGraph::from_partition_plan(&plan)
}
```

---

## Unit Tests — `remove_border`

### UT-0364-01: `remove_border_present_returns_state_and_clears_map`

**Purpose:** Removing an existing border returns `Some(state)` and
deletes the entry from `borders`.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_graph_with_one_border(p(0), p(1));
assert!(graph.borders.contains_key(&1));
```

**When:**
```rust
let removed = graph.remove_border(1);
```

**Then:**
```rust
let state = removed.expect("border 1 was present; remove should return Some");
assert_eq!(state.border_id, 1);
// The returned state's endpoints reflect the pre-remove values.
// (Exact side_a/side_b positions are HashMap-order dependent; assert
// that the pair is {p(0), p(1)}.)
let endpoints: std::collections::HashSet<PortRef> =
    [state.side_a, state.side_b].into_iter().collect();
assert_eq!(endpoints, std::collections::HashSet::from([p(0), p(1)]));
// borders map is now empty for id 1.
assert!(!graph.borders.contains_key(&1));
assert_eq!(graph.len(), 0);
```

**SPEC-19 R covered:** R16 (annihilation removal, return consumed
state for audit).

---

### UT-0364-02: `remove_border_absent_returns_none`

**Purpose:** Removing a border that does not exist returns `None` and
does not panic.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_empty_two_worker_graph();
```

**When:**
```rust
let removed = graph.remove_border(999);
```

**Then:**
```rust
assert!(removed.is_none(),
        "remove_border on absent id MUST return None");
assert!(graph.borders.is_empty());
```

**SPEC-19 R covered:** R16 (absent-case tolerance).

---

### UT-0364-03: `remove_border_clears_active_redex_membership`

**Purpose:** Removing a redex border MUST remove it from
`active_redexes`. `has_no_redexes()` and `active_redex_count()`
reflect the clearance.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_graph_with_one_border(p(0), p(1));
assert!(graph.active_redexes.contains(&1),
        "precondition: border 1 is a redex");
```

**When:**
```rust
let _ = graph.remove_border(1);
```

**Then:**
```rust
assert!(!graph.active_redexes.contains(&1),
        "remove_border MUST clear the active_redex entry");
assert!(graph.has_no_redexes());
assert_eq!(graph.active_redex_count(), 0);
```

**SPEC-19 R covered:** R16, R18 (incremental set consistency).

---

### UT-0364-04: `remove_border_leaves_worker_borders_stale`

**Purpose:** **Positive contract** — `remove_border` MUST NOT touch
`worker_borders`. Stale entries are tolerated per spec §4.2 note. A
future "compact worker_borders" refactor must be a deliberate
behavioural change, not an accidental one.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_graph_with_one_border(p(0), p(1));
assert!(graph.worker_borders[0].contains(&1));
assert!(graph.worker_borders[1].contains(&1));
```

**When:**
```rust
let _ = graph.remove_border(1);
```

**Then:**
```rust
// worker_borders entries STILL contain the stale border_id 1. This is
// the documented design contract (spec §4.2 note).
assert!(graph.worker_borders[0].contains(&1),
        "worker_borders[0] MUST remain stale after remove_border");
assert!(graph.worker_borders[1].contains(&1),
        "worker_borders[1] MUST remain stale after remove_border");
```

**Spec traceability:** positive contract enshrined in TASK-0364
Acceptance Criteria line 81 ("remove_border_leaves_worker_borders_stale").

**SPEC-19 R covered:** R16 + §4.2 note (stale tolerance).

---

## Unit Tests — `add_border_states` (DC-4: Option B)

### UT-0364-05: `add_border_states_inserts_redex_entry_with_graph_derived_bit`

**Purpose:** Adding an `AddBorderEntry` with principal/principal sides
MUST:
- Insert into `borders` with `is_redex = true` (graph-computed).
- Insert the `border_id` into `active_redexes`.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_empty_two_worker_graph();
let entry = AddBorderEntry {
    border_id: 50,
    side_a: p(10),
    side_b: p(20),
    worker_a: 0,
    worker_b: 1,
};
```

**When:**
```rust
graph.add_border_states(vec![entry]);
```

**Then:**
```rust
assert_eq!(graph.len(), 1);
let state = graph.borders.get(&50).expect("border 50 inserted");
assert_eq!(state.border_id, 50);
assert_eq!(state.side_a, p(10));
assert_eq!(state.side_b, p(20));
assert_eq!(state.worker_a, 0);
assert_eq!(state.worker_b, 1);
assert!(state.is_redex,
        "graph-derived is_redex MUST be TRUE for principal/principal");
assert!(graph.active_redexes.contains(&50));
```

**SPEC-19 R covered:** R15 part 3 (primitive), R9 (graph-enforced
invariant via `is_principal_pair`).

---

### UT-0364-06: `add_border_states_inserts_non_redex_entry_with_graph_derived_bit`

**Purpose:** Adding an `AddBorderEntry` with principal/auxiliary sides
MUST:
- Insert into `borders` with `is_redex = false` (graph-computed).
- NOT insert into `active_redexes`.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_empty_two_worker_graph();
let entry = AddBorderEntry {
    border_id: 51,
    side_a: p(10),
    side_b: aux(20, 1),
    worker_a: 0,
    worker_b: 1,
};
```

**When:**
```rust
graph.add_border_states(vec![entry]);
```

**Then:**
```rust
let state = graph.borders.get(&51).expect("border 51 inserted");
assert!(!state.is_redex,
        "graph-derived is_redex MUST be FALSE for principal/aux");
assert!(!graph.active_redexes.contains(&51));
```

**SPEC-19 R covered:** R15 part 3, R9.

---

### UT-0364-07: `add_border_states_updates_worker_borders_for_both_sides`

**Purpose:** Adding an entry MUST push `border_id` into BOTH
`worker_borders[worker_a]` and `worker_borders[worker_b]`.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_empty_two_worker_graph();
let entry = AddBorderEntry {
    border_id: 52,
    side_a: p(10),
    side_b: aux(20, 1),
    worker_a: 0,
    worker_b: 1,
};
```

**When:**
```rust
graph.add_border_states(vec![entry]);
```

**Then:**
```rust
assert!(graph.worker_borders[0].contains(&52),
        "worker_borders[0] MUST include the new border_id");
assert!(graph.worker_borders[1].contains(&52),
        "worker_borders[1] MUST include the new border_id");
```

**SPEC-19 R covered:** R15 part 3 + §4.1 worker_borders invariant.

---

### UT-0364-08: `add_border_states_batch_processes_all_entries_and_preserves_invariant`

**Purpose:** A batch of multiple entries is fully applied, each
entry's `is_redex` is independently computed, and the invariant
`active_redexes == {bid : borders[bid].is_redex}` holds post-batch.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_empty_two_worker_graph();
let entries = vec![
    AddBorderEntry { border_id: 100, side_a: p(0), side_b: p(1), worker_a: 0, worker_b: 1 },
    AddBorderEntry { border_id: 101, side_a: p(2), side_b: aux(3, 1), worker_a: 0, worker_b: 1 },
    AddBorderEntry { border_id: 102, side_a: p(4), side_b: p(5), worker_a: 0, worker_b: 1 },
];
```

**When:**
```rust
graph.add_border_states(entries);
```

**Then:**
```rust
assert_eq!(graph.len(), 3);
assert!(graph.borders.get(&100).unwrap().is_redex);
assert!(!graph.borders.get(&101).unwrap().is_redex);
assert!(graph.borders.get(&102).unwrap().is_redex);

// Invariant cross-check.
let from_borders: std::collections::HashSet<u32> = graph
    .borders
    .iter()
    .filter(|(_, s)| s.is_redex)
    .map(|(bid, _)| *bid)
    .collect();
let from_active: std::collections::HashSet<u32> =
    graph.active_redexes.iter().copied().collect();
assert_eq!(from_active, from_borders,
        "active_redexes MUST equal {{bid : borders[bid].is_redex}}");
assert_eq!(from_active, std::collections::HashSet::from([100, 102]));
```

**SPEC-19 R covered:** R15 part 3 (batch), R18 (invariant).

---

### UT-0364-09: `add_border_states_panics_on_duplicate_border_id`

**Purpose:** Inserting an entry whose `border_id` already exists in
`self.borders` panics with a message naming the duplicate.

**Target file:** `merge/border_graph.rs::tests`

**Given:** A graph with border `7` already present.

**When:** Call `add_border_states` with an entry at `border_id: 7`.

**Then:**
```rust
#[test]
#[should_panic(expected = "duplicate")]
fn add_border_states_panics_on_duplicate_id() {
    let mut graph = make_graph_with_one_border(p(0), p(1));
    // Precondition: border 1 already exists. Our duplicate uses id 1.
    let entry = AddBorderEntry {
        border_id: 1,                   // <- duplicate
        side_a: p(10), side_b: p(20),
        worker_a: 0, worker_b: 1,
    };
    graph.add_border_states(vec![entry]);
}
```

**Assertions:**
- Panic substring contains `"duplicate"` (minimum — TASK-0364 panic
  wording per Key Types block line 257: `"duplicate border_id {} in
  add_border_states"`). A stricter `expected = "duplicate border_id
  1"` is acceptable if the developer pins the message verbatim.

**SPEC-19 R covered:** TASK-0364 Acceptance Criteria defensive panic.

---

### UT-0364-10: `add_border_states_panics_on_out_of_bounds_worker`

**Purpose:** Inserting an entry whose `worker_a` or `worker_b` is
`>= self.worker_borders.len()` panics with a message naming the
border_id and the offending worker.

**Target file:** `merge/border_graph.rs::tests`

**Given:** A 2-worker graph (`worker_borders.len() == 2`, valid
worker IDs are 0 and 1).

**When:** Insert an entry with `worker_a: 5` (out of bounds).

**Then:**
```rust
#[test]
#[should_panic(expected = "worker")]
fn add_border_states_panics_on_out_of_bounds_worker() {
    let mut graph = make_empty_two_worker_graph();
    let entry = AddBorderEntry {
        border_id: 42,
        side_a: p(0), side_b: p(1),
        worker_a: 5,                    // <- invalid (only 0, 1 legal)
        worker_b: 1,
    };
    graph.add_border_states(vec![entry]);
}
```

**Assertions:**
- Panic substring contains `"worker"` (minimum — TASK-0364 panic
  wording per Key Types block line 264: `"out-of-bounds worker {}
  for border_id {}"`). A stricter `expected = "out-of-bounds worker
  5"` is acceptable.

**SPEC-19 R covered:** TASK-0364 Acceptance Criteria defensive panic.

---

### UT-0364-11: `add_border_states_empty_vec_is_noop`

**Purpose:** Calling `add_border_states(vec![])` must not mutate any
field.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_graph_with_one_border(p(0), p(1));
let snapshot = graph.clone();
```

**When:** `graph.add_border_states(vec![]);`

**Then:**
```rust
assert_eq!(graph.borders.len(), snapshot.borders.len());
assert_eq!(graph.active_redexes.len(), snapshot.active_redexes.len());
assert_eq!(graph.worker_borders[0].len(), snapshot.worker_borders[0].len());
assert_eq!(graph.worker_borders[1].len(), snapshot.worker_borders[1].len());
```

**SPEC-19 R covered:** TASK-0364 Acceptance Criteria (empty-vec
tolerance).

---

### UT-0364-12: `add_border_states_enforces_is_redex_invariant` (DC-4 mandated test #11)

**Purpose:** **The load-bearing test of DC-4.** Proves that the
`AddBorderEntry` input type **cannot** encode a broken `is_redex`
bit, because the field does not exist on the input type — the graph
computes it. Two positive cases: principal/principal ⇒ stored state
has `is_redex == true`; principal/auxiliary ⇒ stored state has
`is_redex == false`. Under Option A (caller supplies `BorderState`
with caller-trusted `is_redex`), this invariant could be violated
by a misbehaving caller; under Option B (the spec-critic verdict),
it is **impossible** to violate at the primitive boundary.

**Target file:** `merge/border_graph.rs::tests`

**Given:** An empty 2-worker graph.

**When:** Two consecutive `add_border_states` calls with contrasting
input shapes.

**Then:**
```rust
#[test]
fn add_border_states_enforces_is_redex_invariant() {
    let mut graph = make_empty_two_worker_graph();

    // Part 1: principal / principal entry ⇒ stored is_redex == true.
    graph.add_border_states(vec![AddBorderEntry {
        border_id: 1,
        side_a: p(7),                 // principal (slot 0)
        side_b: p(8),                 // principal (slot 0)
        worker_a: 0,
        worker_b: 1,
    }]);
    let s1 = graph.borders.get(&1).expect("border 1 present");
    assert!(s1.is_redex,
            "R9 invariant: principal/principal MUST store is_redex = true");
    assert!(graph.active_redexes.contains(&1));

    // Part 2: principal / auxiliary entry ⇒ stored is_redex == false.
    graph.add_border_states(vec![AddBorderEntry {
        border_id: 2,
        side_a: p(9),                 // principal
        side_b: aux(10, 1),           // auxiliary (slot 1)
        worker_a: 0,
        worker_b: 1,
    }]);
    let s2 = graph.borders.get(&2).expect("border 2 present");
    assert!(!s2.is_redex,
            "R9 invariant: principal/aux MUST store is_redex = false");
    assert!(!graph.active_redexes.contains(&2));

    // Part 3: cross-sectional invariant still holds.
    let from_borders: std::collections::HashSet<u32> = graph
        .borders
        .iter()
        .filter(|(_, s)| s.is_redex)
        .map(|(bid, _)| *bid)
        .collect();
    let from_active: std::collections::HashSet<u32> =
        graph.active_redexes.iter().copied().collect();
    assert_eq!(from_active, from_borders);
}
```

**Type-level lock (DC-4):** `AddBorderEntry` has **no `is_redex`
field**. This test's struct literals MUST compile without an
`is_redex:` assignment; if a developer regresses to Option A, the
literal becomes invalid.

**Spec traceability:** per the TASK-0364 amendment table (DC-4, row
"Acceptance Criteria — inline test list"): "11 tests (adds
`add_border_states_enforces_is_redex_invariant`) — invariant
enforcement is testable under Option B but impossible under Option
A". The test title above matches verbatim.

**SPEC-19 R covered:** R9 invariant (`is_redex == is_principal_pair
(side_a, side_b)`) — graph-enforced.

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| R16 (annihilation: remove, return `Option<BorderState>`) | UT-0364-01, UT-0364-02 |
| R16 + R18 (active_redexes cleared on remove) | UT-0364-03 |
| R16 + §4.2 note (worker_borders stale tolerance) | UT-0364-04 |
| R15 part 3 (add primitive; batch insertion) | UT-0364-05..08, UT-0364-11 |
| R9 graph-enforced `is_redex` invariant (DC-4) | UT-0364-05, UT-0364-06, UT-0364-08, UT-0364-12 |
| R15 part 3 worker_borders co-updated | UT-0364-07 |
| Defensive panic: duplicate border_id | UT-0364-09 |
| Defensive panic: out-of-bounds worker | UT-0364-10 |
| DC-4 Option B (`AddBorderEntry` input, no `is_redex` field) | All add tests (UT-0364-05..12) — type-level |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0364-A | `remove_border` called in a tight loop over `detect_border_redexes()` output | Classic pattern for annihilation resolution; confirm no panic, no invariant drift, final `active_redexes` is empty |
| QA-0364-B | `add_border_states` with 10k entries in a single batch | Stress per-entry panic checks + HashMap insertion cost; verify no quadratic behaviour |
| QA-0364-C | `AddBorderEntry` with `side_a == side_b` (self-loop) | Semantically valid per R15 part 3? A self-loop would have `is_principal_pair` behaviour defined only if both are `AgentPort(_, 0)` with distinct IDs. QA probes behaviour |
| QA-0364-D | `AddBorderEntry` with `side_a = DISCONNECTED` | `is_principal_pair(DISCONNECTED, _)` is `false`; stored `is_redex` is `false`. QA confirms no special-case needed |
| QA-0364-E | `remove_border` twice in a row on the same id | Second call returns `None`, no panic, active_redexes unchanged |
| QA-0364-F | `add_border_states` → `remove_border` → `add_border_states` with same id | Should round-trip cleanly; the id is NOT considered a duplicate after removal |
| QA-0364-G | `add_border_states` with `AddBorderEntry` whose `is_principal_pair` boundary includes `PortRef::FreePort(x)` (x != u32::MAX) | `is_principal_pair` returns `false` for any `FreePort`; stored `is_redex = false`. QA confirms |
| QA-0364-H | Fuzz: random sequences of `add` / `remove` / `apply_deltas`; invariant `active_redexes == {bid : borders[bid].is_redex}` holds at every step | Non-deterministic; leave as adversarial probe, not required `#[test]` |

---

## Acceptance gate

1. `cargo test --workspace` count: 927 → **938+** (≥ +11; DC-4
   mandates exactly 11 tests per TASK-0364 amendment line 229).
2. Same +11 under `--features zero-copy` (974 → 985 post-0363).
3. All previously passing tests still pass (no regression).
4. `cargo clippy --workspace --all-targets -- -D warnings` clean.
5. `cargo fmt --check` clean.
6. R19 grep guard still passes.
7. Every add-test uses `AddBorderEntry` (NOT `BorderState`) — DC-4
   type-level contract.
8. UT-0364-12 (`add_border_states_enforces_is_redex_invariant`) is
   present under exactly this name.

---

## Out of scope (deferred to later TEST-SPECs)

- Module `//!` doc + R19 pure-core guard → TEST-SPEC-0365.
- Coordinator-side dispatch + `interact_*` resolution (R13/R14) —
  item 2.26.
- Wire-encoding of `BorderDelta` (R20-R36) — item 2.26.
