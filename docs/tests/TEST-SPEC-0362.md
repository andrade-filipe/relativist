# TEST-SPEC-0362: `BorderGraph::apply_deltas` — incremental redex set + DISCONNECTED handling

**Task:** TASK-0362
**Spec:** SPEC-19 §3.2 (R11, R17, R18; §4.2 pseudocode)
**Generated:** 2026-04-17
**Baseline before this task:** 911+ lib (post-TASK-0361)
**Cumulative target after this task:** 921+ lib (≥ +10 new tests)

---

## Scope note

TASK-0362 ships two artefacts into `border_graph.rs`:

1. A new `pub struct BorderDelta { pub border_id: u32, pub new_target:
   PortRef }` with `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`. No
   serde / rkyv derives (item 2.26 adds them non-breakingly).
2. A new method
   `pub fn apply_deltas(&mut self, worker_id: WorkerId, deltas:
   &[BorderDelta])` with the behaviour pinned in TASK-0362 Acceptance
   Criteria (ownership dispatch, incremental `active_redexes`, R17
   double-disconnect removal, silent skip for stale/unknown deltas).

**Spec-critic DC-1 baked in:** the DISCONNECTED sentinel is
`crate::net::DISCONNECTED` (alias for `PortRef::FreePort(u32::MAX)`) —
**no `BorderTarget` enum**, **no `Option<PortRef>`**. All tests
construct disconnect deltas via `new_target: DISCONNECTED` (or
`PortRef::FreePort(u32::MAX)` if direct construction is preferred).

**Silent-skip contract (R11):** the spec §3.2 pseudocode silently
skips deltas whose `border_id` is absent from `borders` and deltas
whose `worker_id` owns neither side. TASK-0362 positively asserts this
as a contract (item 2.26 depends on it).

**Incremental invariant (R18 SHOULD):** for any reachable graph state,
`active_redexes == {bid : borders[bid].is_redex == true}`. Multiple
tests below verify this cross-sectionally.

The tests use the shared `make_plan` fixtures from TEST-SPEC-0361 plus
a new helper `make_graph_with_one_border(side_a, side_b)` built via
`BorderGraph::from_partition_plan(&plan)`.

---

## Test target file paths

- `relativist-core/src/merge/border_graph.rs` — extend inline
  `#[cfg(test)] mod tests` block.

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Shared fixtures

```rust
use crate::net::{PortRef, DISCONNECTED};

/// Build a 2-worker graph containing exactly one border with the
/// given endpoint configuration. Worker 0 owns side_a, worker 1 owns
/// side_b. Border id = 1.
fn make_graph_with_one_border(side_a: PortRef, side_b: PortRef) -> BorderGraph {
    let plan = make_plan(
        vec![(0, vec![(1, side_a)]), (1, vec![(1, side_b)])],
        vec![1],
    );
    BorderGraph::from_partition_plan(&plan)
}
```

The `p(id)` / `aux(id, slot)` helpers from TEST-SPEC-0361 are reused.

---

## Unit Tests

### UT-0362-01: `apply_delta_principal_to_principal_marks_redex`

**Purpose:** A border starting as principal / auxiliary has its
auxiliary side upgraded to principal via a delta ⇒ `is_redex` becomes
`true`, `active_redexes` gains the `border_id`.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_graph_with_one_border(p(0), aux(1, 1));
assert!(!graph.borders.get(&1).unwrap().is_redex,
        "precondition: border starts as not-redex");
assert!(graph.active_redexes.is_empty());
// Worker 1 owns side_b (the auxiliary side). It emits a delta upgrading
// its side to a principal port (AgentPort(9, 0)).
let delta = BorderDelta { border_id: 1, new_target: p(9) };
```

**When:** `graph.apply_deltas(1 /* worker_id */, &[delta]);`

**Then:**
```rust
let state = graph.borders.get(&1).expect("border still present");
assert!(state.is_redex, "after delta, both sides principal ⇒ is_redex = true");
assert!(graph.active_redexes.contains(&1));
assert_eq!(graph.active_redexes.len(), 1);
// Cross-check: side_b was updated, side_a unchanged.
assert_eq!(state.side_b, p(9));
assert_eq!(state.side_a, p(0));
```

**SPEC-19 R covered:** R11 (single-side update, recompute `is_redex`),
R18 (incremental `active_redexes` insertion).

---

### UT-0362-02: `apply_delta_principal_to_aux_clears_redex`

**Purpose:** A border starting as redex has one side demoted to
auxiliary via a delta ⇒ `is_redex` becomes `false`, `active_redexes`
loses the `border_id`.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_graph_with_one_border(p(0), p(1));
assert!(graph.borders.get(&1).unwrap().is_redex);
assert!(graph.active_redexes.contains(&1));
// Worker 0 owns side_a. Delta moves side_a from principal to aux.
let delta = BorderDelta { border_id: 1, new_target: aux(5, 1) };
```

**When:** `graph.apply_deltas(0 /* worker_id */, &[delta]);`

**Then:**
```rust
let state = graph.borders.get(&1).unwrap();
assert!(!state.is_redex, "principal → aux demotion clears is_redex");
assert!(!graph.active_redexes.contains(&1));
assert!(graph.active_redexes.is_empty());
assert_eq!(state.side_a, aux(5, 1));
```

**SPEC-19 R covered:** R11, R18 (incremental removal).

---

### UT-0362-03: `apply_delta_wrong_worker_silent_skip`

**Purpose:** A delta whose `worker_id` owns neither side of the named
border MUST be silently skipped — no state change, no panic.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_graph_with_one_border(p(0), aux(1, 1));
let state_before = graph.borders.get(&1).unwrap().clone();
let delta = BorderDelta { border_id: 1, new_target: p(99) };
```

**When:** `graph.apply_deltas(42 /* unrelated worker_id */, &[delta]);`

**Then:**
```rust
let state_after = graph.borders.get(&1).unwrap();
assert_eq!(*state_after, state_before,
        "delta from a worker that owns neither side is silently skipped");
// No panic occurred (implicit — the test reached this line).
```

**SPEC-19 R covered:** R11 silent-skip contract.

---

### UT-0362-04: `apply_delta_unknown_border_silent_skip`

**Purpose:** A delta whose `border_id` is not in `borders` MUST be
silently skipped (the border may have been removed earlier in the same
round by a prior delta, or by a coordinator-side resolution).

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_graph_with_one_border(p(0), aux(1, 1));
let n_before = graph.borders.len();
let delta = BorderDelta { border_id: 999 /* unknown */, new_target: p(0) };
```

**When:** `graph.apply_deltas(0, &[delta]);`

**Then:**
```rust
assert_eq!(graph.borders.len(), n_before,
        "unknown border_id does not mutate the borders map");
// No panic.
```

**SPEC-19 R covered:** R11 silent-skip contract.

---

### UT-0362-05: `apply_delta_disconnect_one_side_keeps_border`

**Purpose:** A DISCONNECTED delta on ONE side (the other side still
connected) updates the side to the sentinel and keeps the border
alive. `is_redex` is `false` (DISCONNECTED is not an `AgentPort`).

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_graph_with_one_border(p(0), p(1));
assert!(graph.borders.get(&1).unwrap().is_redex, "precondition: starts redex");
// Worker 0's side gets erased — report DISCONNECTED.
let delta = BorderDelta { border_id: 1, new_target: DISCONNECTED };
```

**When:** `graph.apply_deltas(0, &[delta]);`

**Then:**
```rust
let state = graph.borders.get(&1).expect("border still alive");
assert_eq!(state.side_a, DISCONNECTED);
assert_eq!(state.side_b, p(1),
        "side_b is untouched since only worker 0's side was disconnected");
assert!(!state.is_redex,
        "DISCONNECTED ≠ AgentPort(_, 0); is_redex cleared");
assert!(!graph.active_redexes.contains(&1));
```

**SPEC-19 R covered:** R17 single-side erasure, R11 redex recomputation,
DC-1 sentinel form (`DISCONNECTED = PortRef::FreePort(u32::MAX)`).

---

### UT-0362-06: `apply_delta_disconnect_both_sides_removes_border`

**Purpose:** When both sides reach DISCONNECTED, the border MUST be
removed from `borders` and `active_redexes`. `worker_borders` entries
MAY remain stale (per spec §4.2 note) — test MUST NOT assert their
removal.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_graph_with_one_border(p(0), p(1));
let disc_a = BorderDelta { border_id: 1, new_target: DISCONNECTED };
let disc_b = BorderDelta { border_id: 1, new_target: DISCONNECTED };
```

**When:** Two `apply_deltas` calls, one per worker (simulating
independent worker rounds).
```rust
graph.apply_deltas(0, &[disc_a]);
graph.apply_deltas(1, &[disc_b]);
```

**Then:**
```rust
assert!(!graph.borders.contains_key(&1),
        "border removed after both sides DISCONNECTED");
assert!(!graph.active_redexes.contains(&1));
// NB: NO assertion about worker_borders[0] / worker_borders[1] — spec
// §4.2 note permits stale entries. A future task can compact.
```

**SPEC-19 R covered:** R17 double-erasure ⇒ removal.

---

### UT-0362-07: `apply_deltas_empty_slice_is_noop`

**Purpose:** Passing `&[]` MUST NOT mutate any field.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_graph_with_one_border(p(0), p(1));
let snapshot = graph.clone();
```

**When:** `graph.apply_deltas(0, &[]);`

**Then:**
```rust
assert_eq!(graph.borders.len(), snapshot.borders.len());
assert_eq!(graph.active_redexes.len(), snapshot.active_redexes.len());
// Per-field check for the single known border.
assert_eq!(graph.borders.get(&1), snapshot.borders.get(&1));
```

**SPEC-19 R covered:** R11 (empty-batch no-op).

---

### UT-0362-08: `apply_deltas_batch_mixed_targets_applied_per_delta`

**Purpose:** A batch of multiple deltas (some to redex, some to
non-redex, some stale) updates each relevant state exactly once and
preserves the incremental invariant post-batch.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
// 3 borders:
//  1: principal / aux        → not a redex; about to become one
//  2: principal / principal  → redex; about to become non-redex
//  3: principal / principal  → redex; remains a redex (delta from wrong worker, silent skip)
let plan = make_plan(
    vec![
        (0, vec![(1, p(10)), (2, p(20)), (3, p(30))]),
        (1, vec![(1, aux(11, 1)), (2, p(21)), (3, p(31))]),
    ],
    vec![1, 2, 3],
);
let mut graph = BorderGraph::from_partition_plan(&plan);

// Worker 1 emits 3 deltas:
//  border 1: upgrade side_b to principal   → becomes redex
//  border 2: demote side_b to aux          → ceases to be redex
//  border 999: unknown                      → silent skip
let deltas = [
    BorderDelta { border_id: 1,   new_target: p(40) },
    BorderDelta { border_id: 2,   new_target: aux(50, 1) },
    BorderDelta { border_id: 999, new_target: p(60) },
];
```

**When:** `graph.apply_deltas(1 /* worker_id */, &deltas);`

**Then:**
```rust
// Border 1: now a redex.
assert!(graph.borders.get(&1).unwrap().is_redex);
assert!(graph.active_redexes.contains(&1));

// Border 2: no longer a redex.
assert!(!graph.borders.get(&2).unwrap().is_redex);
assert!(!graph.active_redexes.contains(&2));

// Border 3: untouched (no delta referenced it; worker 1 owns side_b,
// but no delta was aimed at border 3).
assert!(graph.borders.get(&3).unwrap().is_redex);
assert!(graph.active_redexes.contains(&3));

// Border 999: still absent (silent skip).
assert!(!graph.borders.contains_key(&999));

// Cross-sectional invariant:
//   active_redexes == {bid : borders[bid].is_redex == true}
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
```

**SPEC-19 R covered:** R11 (per-delta semantics), R18 (incremental
invariant preserved across a batch with mixed transitions).

---

### UT-0362-09: `apply_delta_redundant_no_change_keeps_active_redexes_stable`

**Purpose:** A delta that re-writes the same value (no net change) MUST
NOT flip `active_redexes` membership — the `was_redex ↔ is_redex`
comparison is what drives the `HashSet` update; equal values should
produce no transition.

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_graph_with_one_border(p(0), p(1));
// Worker 0 rewrites side_a to the exact same principal port.
let delta = BorderDelta { border_id: 1, new_target: p(0) };
```

**When:** `graph.apply_deltas(0, &[delta]);`

**Then:**
```rust
assert!(graph.borders.get(&1).unwrap().is_redex);
assert!(graph.active_redexes.contains(&1));
assert_eq!(graph.active_redexes.len(), 1);
```

**SPEC-19 R covered:** R11 + R18 idempotence property.

---

### UT-0362-10: `border_delta_struct_derives_debug_clone_copy_eq`

**Purpose:** Pin the `BorderDelta` struct shape + derives. Downstream
wire-encoding (item 2.26) depends on `Copy` + `PartialEq` + `Eq` being
present. `Debug` is required by `assert_eq!` formatting in the tests
above.

**Target file:** `merge/border_graph.rs::tests`

**Given:** `BorderDelta` type.

**When:** Construct, copy, compare.

**Then:**
```rust
let d1 = BorderDelta { border_id: 5, new_target: p(7) };
let d2 = d1;                  // Copy active
let d3 = d1.clone();          // Clone active
assert_eq!(d1, d2);           // PartialEq + Eq active
assert_eq!(d1, d3);
let s = format!("{d1:?}");    // Debug active
assert!(s.contains("BorderDelta"));
assert!(s.contains("5"));
// Negative: differing field yields inequality.
assert_ne!(d1, BorderDelta { border_id: 6, new_target: p(7) });
assert_ne!(d1, BorderDelta { border_id: 5, new_target: p(8) });
```

**Assertions:**
- All four derives (`Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`) active.
- Both fields (`border_id`, `new_target`) participate in equality.

**SPEC-19 R covered:** TASK-0362 acceptance criterion #1 (struct shape
+ derives).

---

### UT-0362-11: `apply_deltas_preserves_incremental_invariant_under_disconnect_then_reconnect`

**Purpose:** Stress the state machine: a border goes principal/principal →
DISCONNECTED/principal → principal/principal across 2 calls. The
`active_redexes` set MUST track precisely: present at start, removed
after the DISCONNECTED delta, re-inserted after the reconnect (the
border is STILL ALIVE after the single disconnect; see UT-0362-05).

**Target file:** `merge/border_graph.rs::tests`

**Given:**
```rust
let mut graph = make_graph_with_one_border(p(0), p(1));
assert!(graph.active_redexes.contains(&1));  // t0: redex

// Step 1: worker 0 disconnects.
graph.apply_deltas(0, &[BorderDelta { border_id: 1, new_target: DISCONNECTED }]);
assert!(graph.borders.contains_key(&1), "border still alive (one-sided disc)");
assert!(!graph.active_redexes.contains(&1), "t1: not a redex");

// Step 2: worker 0 reconnects via new agent.
graph.apply_deltas(0, &[BorderDelta { border_id: 1, new_target: p(99) }]);
```

**When:** The two `apply_deltas` calls above.

**Then:**
```rust
let state = graph.borders.get(&1).expect("border still alive");
assert_eq!(state.side_a, p(99));
assert_eq!(state.side_b, p(1));
assert!(state.is_redex, "t2: principal/principal again");
assert!(graph.active_redexes.contains(&1), "t2: back in active_redexes");
```

**SPEC-19 R covered:** R11 + R17 (single-side disconnect keeps border
alive) + R18 (incremental invariant across redex → not-redex → redex
transitions).

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| R11 (per-delta ownership dispatch, recompute `is_redex`) | UT-0362-01, UT-0362-02, UT-0362-03, UT-0362-08 |
| R11 silent-skip (unknown `border_id`) | UT-0362-04, UT-0362-08 |
| R11 silent-skip (wrong worker) | UT-0362-03 |
| R11 empty-batch no-op | UT-0362-07 |
| R17 single-side DISCONNECTED keeps border | UT-0362-05, UT-0362-11 |
| R17 double-side DISCONNECTED removes border | UT-0362-06 |
| R18 incremental `active_redexes` invariant | UT-0362-01, UT-0362-02, UT-0362-08, UT-0362-09, UT-0362-11 |
| DC-1 DISCONNECTED sentinel form (`crate::net::DISCONNECTED`) | UT-0362-05, UT-0362-06, UT-0362-11 (all use the named constant, not a local literal) |
| `BorderDelta` struct shape + derives | UT-0362-10 |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0362-A | 10k deltas in a single batch, all targeting the same border (last-write-wins semantics) | Stresses the silent-skip + recompute path; final state MUST match the last applicable delta; `active_redexes` MUST match the final `is_redex` |
| QA-0362-B | Fuzz: random delta sequences over a fixed small graph; property: `active_redexes == {bid : borders[bid].is_redex}` holds at every step | Expensive to write deterministically; acceptable as an adversarial probe. Do NOT add as required `#[test]` (non-determinism risk) |
| QA-0362-C | Delta with `border_id` in `plan.borders` but `worker_id` = another partition's `worker_id` that is NOT either side's owner (the "third worker" case) | Currently handled by silent skip (else branch at line 148 of TASK-0362 Key Types). QA should confirm no state drift |
| QA-0362-D | Delta carrying `new_target = PortRef::FreePort(x)` for a border_id where `x` is NOT `u32::MAX` (not the sentinel — a genuine free port reference) | Is this semantically valid? The spec treats FreePort as distinct from AgentPort. `is_principal_pair(FreePort(_), _) → false` always, so `is_redex` is cleared. QA should confirm the expected transition |
| QA-0362-E | Two deltas in the same batch: first DISCONNECTs side_a, second reconnects side_a in the same call | The function iterates the slice in order; invariants MUST hold per-delta. Intra-batch re-entrance is not tested deterministically; QA flags |
| QA-0362-F | Delta with `new_target` equal to the current `side_a` value (no-op) but from the WRONG worker | Silent skip (UT-0362-03) already covers this; QA can vary to ensure no transient `is_redex` flicker |
| QA-0362-G | `apply_deltas` called with a `worker_id` that matches `worker_a == worker_b` (same-worker border, see QA-0361-E) | Undefined per spec; may panic, may update side_a arbitrarily. QA flag for spec-critic |

---

## Acceptance gate

1. `cargo test --workspace` count: 911 → **921+** (≥ +10; 11 tests
   listed above, minus possible merging of related small tests — the
   floor is 10 per TASK-0362 line 103).
2. Same +10 under `--features zero-copy` (958 → 968 post-0361).
3. All previously passing tests still pass (no regression).
4. `cargo clippy --workspace --all-targets -- -D warnings` clean.
5. `cargo fmt --check` clean.
6. R19 grep guard still passes (no new `use tokio` / `async` /
   `crate::protocol` lines introduced).
7. No `unwrap()` in production code in the updated `border_graph.rs`.

---

## Out of scope (deferred to later TEST-SPECs)

- `detect_border_redexes` owned return → TEST-SPEC-0363.
- `remove_border` + `add_border_states` (AddBorderEntry) →
  TEST-SPEC-0364.
- Module doc + R19 pure-core guard → TEST-SPEC-0365.
- Wire-encoding of `BorderDelta` (item 2.26) — explicitly out of scope
  for this bundle.
