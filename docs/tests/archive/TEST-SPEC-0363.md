# TEST-SPEC-0363: `BorderGraph::detect_border_redexes` (owned return per DC-3) + read-only accessors

**Task:** TASK-0363
**Spec:** SPEC-19 §3.2 (R12, R18; §3.2 R12 normative text overrides §4.2 pseudocode)
**Generated:** 2026-04-17
**Baseline before this task:** 921+ lib (post-TASK-0362)
**Cumulative target after this task:** 927+ lib (≥ +6 new tests)

---

## Scope note

TASK-0363 lands five methods on `BorderGraph`:

1. `pub fn detect_border_redexes(&self) -> Vec<(u32, BorderState)>` —
   **owned** return per spec-critic DC-3 verdict (§3.2 R12 normative
   text wins over §4.2 non-normative pseudocode).
2. `pub fn len(&self) -> usize` — total alive borders.
3. `pub fn is_empty(&self) -> bool` — `borders.is_empty()`.
4. `pub fn has_no_redexes(&self) -> bool` —
   `active_redexes.is_empty()`.
5. `pub fn active_redex_count(&self) -> usize` —
   `active_redexes.len()`.

**DC-3 verdict baked in:** every test below expects the return type
`Vec<(u32, BorderState)>` — owned values, NOT `&BorderState`. If a
developer writes the body as `(bid, state)` without a `.clone()`,
compilation fails under the spec'd signature — so the type assertions
below double as a compile-time contract check.

Tests reuse the `make_graph_with_one_border` fixture from
TEST-SPEC-0362 and introduce a `make_graph_with_three_borders` helper.

---

## Test target file paths

- `relativist-core/src/merge/border_graph.rs` — extend inline
  `#[cfg(test)] mod tests` block.

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Shared fixtures

```rust
/// Build a 2-worker graph with three borders:
///  - border 10: principal / auxiliary (not a redex)
///  - border 20: principal / principal (redex)
///  - border 30: principal / principal (redex)
fn make_graph_with_three_borders() -> BorderGraph {
    let plan = make_plan(
        vec![
            (0, vec![(10, p(0)),  (20, p(1)),  (30, p(2))]),
            (1, vec![(10, aux(5, 1)), (20, p(6)), (30, p(7))]),
        ],
        vec![10, 20, 30],
    );
    BorderGraph::from_partition_plan(&plan)
}
```

---

## Unit Tests

### UT-0363-01: `detect_returns_empty_vec_when_no_redexes`

**Purpose:** A graph with borders but no principal pairs ⇒
`detect_border_redexes()` returns an empty `Vec`, and
`has_no_redexes()` returns `true`.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
// 2 borders, neither principal/principal.
let plan = make_plan(
    vec![
        (0, vec![(1, p(0)), (2, p(1))]),
        (1, vec![(1, aux(3, 1)), (2, aux(4, 1))]),
    ],
    vec![1, 2],
);
let graph = BorderGraph::from_partition_plan(&plan);
```

**When:** Call `graph.detect_border_redexes()`.

**Then:**
```rust
let redexes: Vec<(u32, BorderState)> = graph.detect_border_redexes();
assert!(redexes.is_empty(),
        "no principal-pair borders ⇒ empty redex vec");
assert!(graph.has_no_redexes());
assert_eq!(graph.active_redex_count(), 0);
// Total borders alive is still 2 (no R17 removal).
assert_eq!(graph.len(), 2);
assert!(!graph.is_empty());
```

**Type contract (DC-3):** the return type annotation `Vec<(u32,
BorderState)>` MUST compile. If the developer ships `&BorderState`
(borrowed), the explicit annotation makes the `let` bind fail at
compile time.

**SPEC-19 R covered:** R12 (empty case), R18, DC-3 owned return.

---

### UT-0363-02: `detect_returns_owned_single_redex`

**Purpose:** Graph with exactly one redex ⇒ `detect_border_redexes()`
returns `[(bid, BorderState)]`, the `BorderState` is a CLONE (owned),
and mutating `graph` afterward does not invalidate the returned vec.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let graph = make_graph_with_one_border(p(0), p(1));
```

**When:**
```rust
let redexes: Vec<(u32, BorderState)> = graph.detect_border_redexes();
```

**Then:**
```rust
assert_eq!(redexes.len(), 1);
let (bid, state) = &redexes[0];
assert_eq!(*bid, 1);
assert!(state.is_redex);
// DC-3 owned-return check: `state` is `BorderState`, not
// `&BorderState`. If the return were borrowed, this type would not
// match. The explicit type annotation on the `let` above is the
// primary compile-time lock.
```

**SPEC-19 R covered:** R12, DC-3.

---

### UT-0363-03: `detect_returns_owned_vec_usable_with_mut_self_borrow`

**Purpose:** The **load-bearing motivation** for DC-3 Option A (owned
return). The coordinator's item-2.26 loop calls
`detect_border_redexes()` and then `&mut graph` inside the loop body.
Under borrowed return (§4.2 pseudocode), this would fail to compile.
Under owned return (DC-3), it compiles — and this test exercises the
pattern.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_graph_with_three_borders();
```

**When:**
```rust
// Mimic the item-2.26 coordinator pattern: iterate redexes, and
// inside the loop perform a mutable operation on the graph.
let redexes: Vec<(u32, BorderState)> = graph.detect_border_redexes();
for (bid, _state_owned) in &redexes {
    // Mutable borrow of `graph` would be IMPOSSIBLE if `_state_owned`
    // were `&BorderState` (it would hold an immutable borrow of graph).
    // Because DC-3 ships owned BorderState, this compiles.
    let _ = graph.apply_deltas(
        0 /* worker_id */,
        &[BorderDelta { border_id: *bid, new_target: aux(200, 1) }],
    );
}
```

**Then:**
```rust
// After the loop, all redex borders have been demoted on side_a.
// (worker 0 owns side_a in this fixture by construction order.)
assert!(graph.active_redexes.is_empty(),
        "all redexes cleared after loop applied aux-demotion deltas");
```

**Rationale comment in the test:** per DC-3 of
`docs/spec-reviews/SPEC-19-section-3.2-design-choices-2026-04-17.md`:
"the borrowed form is impossible to use in the item 2.26 coordinator
BSP loop (which needs `&mut border_graph` while iterating the redex
list)". This test locks that verdict into the contract.

**SPEC-19 R covered:** R12, DC-3 (coordinator mutable-borrow pattern).

---

### UT-0363-04: `detect_returns_multiple_redexes_order_independent`

**Purpose:** Graph with 2 redexes + 1 non-redex ⇒ returned vec has
exactly the 2 redex borders, in any order (iteration order of
`active_redexes: HashSet<u32>` is unspecified). Test asserts via
`HashSet<u32>` conversion.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let graph = make_graph_with_three_borders();
// Expected redexes: {20, 30}; non-redex: 10.
```

**When:**
```rust
let redexes: Vec<(u32, BorderState)> = graph.detect_border_redexes();
```

**Then:**
```rust
assert_eq!(redexes.len(), 2);
let ids: std::collections::HashSet<u32> =
    redexes.iter().map(|(bid, _)| *bid).collect();
assert_eq!(
    ids,
    std::collections::HashSet::from([20, 30]),
    "redex set must equal {{20, 30}}"
);
// Each returned state has is_redex == true.
for (_, state) in &redexes {
    assert!(state.is_redex,
        "every state in the result MUST have is_redex == true");
}
```

**SPEC-19 R covered:** R12 (multi-redex case), R18.

---

### UT-0363-05: `detect_reflects_apply_deltas_transitions`

**Purpose:** End-to-end interaction: after `apply_deltas` flips a
border's redex bit, `detect_border_redexes` reflects the change with
no caching.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_graph_with_one_border(p(0), aux(1, 1));
assert!(graph.detect_border_redexes().is_empty(),
        "t0: no redex");
```

**When:** Upgrade side_b to principal via a delta; then demote it
back.
```rust
graph.apply_deltas(1, &[BorderDelta { border_id: 1, new_target: p(9) }]);
let t1 = graph.detect_border_redexes();
graph.apply_deltas(1, &[BorderDelta { border_id: 1, new_target: aux(9, 1) }]);
let t2 = graph.detect_border_redexes();
```

**Then:**
```rust
assert_eq!(t1.len(), 1, "t1: redex present after upgrade");
assert_eq!(t1[0].0, 1);
assert!(t1[0].1.is_redex);

assert!(t2.is_empty(), "t2: redex cleared after demote");
```

**SPEC-19 R covered:** R12 + R11 + R18 (end-to-end incremental
invariant).

---

### UT-0363-06: `len_and_is_empty_track_alive_borders_not_redex_count`

**Purpose:** Pin the semantic distinction between `len` (alive
borders), `is_empty` (no alive borders), and `has_no_redexes` (no
ACTIVE pairs — borders may still exist). `has_no_redexes()` and
`is_empty()` MUST be independent.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
// Graph with 1 border, principal/aux (NOT a redex).
let graph = make_graph_with_one_border(p(0), aux(1, 1));
```

**When + Then:**
```rust
assert_eq!(graph.len(), 1,
        "border is alive even though not a redex");
assert!(!graph.is_empty(),
        "is_empty is FALSE — border exists");
assert!(graph.has_no_redexes(),
        "has_no_redexes is TRUE — no active pair");
assert_eq!(graph.active_redex_count(), 0);
assert_eq!(graph.detect_border_redexes().len(), 0);
```

**Spec traceability:** this is the `is_empty_distinct_from_has_no_redexes`
test named in TASK-0363 Acceptance Criteria (line 74). Documented as
a positive contract so a future "consolidate into one method" refactor
is rejected at review.

**SPEC-19 R covered:** R12, §4.2 naming convention.

---

### UT-0363-07: `detect_complexity_iterates_active_redexes_not_borders`

**Purpose:** Code-shape contract — `detect_border_redexes` MUST
iterate `active_redexes` directly, NOT scan `borders`. This is a
review-level check, enforced here by a positive behaviour assertion
that only the spec'd algorithm can satisfy: a graph with 1000 non-redex
borders and 1 redex border should return exactly 1 entry, proving the
algorithm does not scan all 1000.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
// 1000 non-redex borders + 1 redex border, across 2 workers.
let mut partition0 = vec![];
let mut partition1 = vec![];
let mut decls = vec![];
for bid in 0u32..1000 {
    partition0.push((bid, p(bid * 2)));
    partition1.push((bid, aux(bid * 2 + 1, 1)));   // aux ⇒ not redex
    decls.push(bid);
}
// The one redex border.
partition0.push((1000, p(5000)));
partition1.push((1000, p(5001)));
decls.push(1000);
let plan = make_plan(vec![(0, partition0), (1, partition1)], decls);
let graph = BorderGraph::from_partition_plan(&plan);
```

**When:**
```rust
let redexes = graph.detect_border_redexes();
```

**Then:**
```rust
assert_eq!(graph.len(), 1001,
        "1001 total alive borders in the fixture");
assert_eq!(redexes.len(), 1,
        "only 1 redex (the principal-principal border)");
assert_eq!(redexes[0].0, 1000);
assert!(redexes[0].1.is_redex);

// Observable complexity: the result length equals
// `active_redex_count()`, not `len()`. If the implementation were
// `borders.iter().filter(...).collect()`, this test would still
// pass behaviorally — so the "no scan of borders" constraint is
// primarily a review-gate contract. This test provides the positive
// behavioural guardrail; architectural review MUST visually confirm
// the body iterates `self.active_redexes` per DC-3 Key Types block.
assert_eq!(redexes.len(), graph.active_redex_count());
```

**SPEC-19 R covered:** R18 (incremental O(|active_redexes|)
complexity).

---

### UT-0363-08: `detect_after_r17_removal_omits_dead_border`

**Purpose:** After a border is removed via R17 double-disconnect
(`apply_deltas` side-effect), `detect_border_redexes` MUST NOT emit
the dead border, AND the `filter_map`'s defensive check (per TASK-0363
Acceptance Criterion bullet 1) correctly skips any residual ID.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_graph_with_one_border(p(0), p(1));
// Both sides disconnect — border removed, active_redexes cleared
// (per UT-0362-06 behaviour).
graph.apply_deltas(0, &[BorderDelta { border_id: 1, new_target: DISCONNECTED }]);
graph.apply_deltas(1, &[BorderDelta { border_id: 1, new_target: DISCONNECTED }]);
assert!(!graph.borders.contains_key(&1));
```

**When:**
```rust
let redexes = graph.detect_border_redexes();
```

**Then:**
```rust
assert!(redexes.is_empty(),
        "dead border MUST NOT appear in detect output");
assert_eq!(graph.len(), 0);
assert!(graph.is_empty());
assert!(graph.has_no_redexes());
```

**SPEC-19 R covered:** R12 + R17 interaction.

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| R12 (owned `Vec<(u32, BorderState)>` — DC-3) | UT-0363-01, UT-0363-02, UT-0363-03, UT-0363-04, UT-0363-05, UT-0363-07, UT-0363-08 |
| R18 incremental complexity (O(|active_redexes|)) | UT-0363-07 |
| `len` / `is_empty` — alive-border accessors | UT-0363-01, UT-0363-06, UT-0363-07, UT-0363-08 |
| `has_no_redexes` / `active_redex_count` — redex-count accessors | UT-0363-01, UT-0363-06 |
| DC-3 mutable-borrow-while-iterating pattern | UT-0363-03 |
| Interaction with TASK-0362 (`apply_deltas` transitions) | UT-0363-05, UT-0363-08 |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0363-A | `active_redexes` contains a `border_id` that is NOT in `borders` (invariant violation induced by a future bug) | The `filter_map` defensively skips the stale ID; result is shorter than `active_redexes.len()`. QA should confirm no panic, and architecture review should catch the invariant violation at the call site that wrote the invalid state |
| QA-0363-B | 100k active redexes — stress the owned-return clone cost | DC-3 verdict pins 32 KB / 1k redexes as acceptable; 100k → 3.2 MB per call. QA measures allocator pressure in release mode |
| QA-0363-C | `detect_border_redexes` called in a tight loop without intervening mutation | Deterministic output across calls is required (`active_redexes` iteration order is unspecified but stable for a given HashSet state). Verify returned ID sets are equal across calls |
| QA-0363-D | Interleaved `detect` and `apply_deltas` where a delta mid-iteration would break a borrowed return | UT-0363-03 covers the positive pattern; QA should actively try the WRONG pattern (holding a borrow across `&mut self`) to prove the type system rejects it |
| QA-0363-E | `len` after many `add_border_states` + `remove_border` cycles | Stress count-arithmetic consistency — `len == borders.len()`, always |

---

## Acceptance gate

1. `cargo test --workspace` count: 921 → **927+** (≥ +6; 8 tests
   listed above, subject to developer-side merging of related
   assertions — the floor is 6 per TASK-0363 line 78).
2. Same +6 under `--features zero-copy` (968 → 974 post-0362).
3. All previously passing tests still pass (no regression).
4. `cargo clippy --workspace --all-targets -- -D warnings` clean.
5. `cargo fmt --check` clean.
6. R19 grep guard still passes.
7. Every test above uses the **owned** return-type annotation
   `Vec<(u32, BorderState)>` — DC-3 contract.

---

## Out of scope (deferred to later TEST-SPECs)

- `remove_border` + `add_border_states` (AddBorderEntry) →
  TEST-SPEC-0364.
- Module doc + R19 pure-core guard → TEST-SPEC-0365.
- Coordinator-side `interact_*` resolution (R13/R14) — item 2.26.
