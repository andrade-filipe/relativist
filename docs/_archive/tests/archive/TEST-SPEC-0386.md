# TEST-SPEC-0386: Coordinator convergence detection — `check_delta_convergence` (R4, R21.3, R40)

**Task:** TASK-0386
**Spec:** SPEC-19 R4 (Global Normal Form), R21 phase 3 (convergence triggers
  Final Collection), R40 (literal: "all workers report zero local redexes
  AND the BorderGraph contains zero active pairs").
**Spec-critic notes:** DC-C5 (FLIP, option B) — predicate is THREE-conjunct:
  `has_border_activity=false` AND `local_redexes=0` AND
  `BorderGraph::detect_border_redexes().is_empty()`. The task-splitter draft's
  two-predicate version is a spec deviation; this TEST-SPEC reflects the
  ratified three-predicate form. One pre-existing test (`*_ignores_local_redexes`)
  is RENAMED with polarity inversion to
  `check_delta_convergence_requires_no_local_redexes`. One new sanity test
  is added (`*_false_when_one_worker_has_local_redexes`).
**Generated:** 2026-04-17

---

## Scope note

TASK-0386 ships the pure predicate that decides whether the delta BSP loop
exits via the convergence path (R21.3 → R27/R29) versus continuing to the
next round. The signature is:

```rust
pub(crate) fn check_delta_convergence(
    results: &[RoundResultPayload],
    border_graph: &BorderGraph,
) -> bool
```

**DC-C5 ruling:** the predicate is

```
all_no_border_activity = results.iter().all(|r| !r.has_border_activity);
all_no_local_redexes   = results.iter().all(|r| r.stats.local_redexes == 0);
graph_has_no_redexes   = border_graph.detect_border_redexes().is_empty();
return all_no_border_activity && all_no_local_redexes && graph_has_no_redexes;
```

This matches SPEC-19 R40 literal AND v1 `check_global_normal_form`'s shape
(both booleans `&&`-joined; delta mode adds the third graph conjunct).

**Vacuous case:** empty `results` slice + empty graph → `true`. Should not
arise in practice (loop collects ≥1 result/worker/round) but must not panic.

---

## Test target file paths

- `relativist-core/src/merge/grid.rs` — inline `#[cfg(test)] mod tests`.
  Seven new `#[test]` fns (5 reused-from-task-splitter-draft + 1 RENAMED
  with polarity flip + 1 new sanity test).

All tests are synchronous. No `tokio`, no `async`.

---

## Unit Tests

### UT-0386-01: `check_delta_convergence_true_when_all_quiet`

**Purpose:** Happy-path GNF — all three predicates `true` → convergence.

**Target:** `merge/grid.rs::tests`

**Given:**
- `results: Vec<RoundResultPayload>` with 2 entries, each:
  - `has_border_activity = false`
  - `stats.local_redexes = 0`
- `border_graph`: empty (no entries; `detect_border_redexes()` returns `vec![]`).

**When:** `let converged = check_delta_convergence(&results, &border_graph);`

**Then:** `converged == true`.

**Assertions:** All three conjuncts must hold for `true` return.

**SPEC-19 R covered:** R4, R40 (literal three-conjunct).

---

### UT-0386-02: `check_delta_convergence_false_on_worker_activity`

**Purpose:** First conjunct violation.

**Given:**
- `results`: 2 entries; ONE has `has_border_activity = true`, the other
  `false`. Both have `stats.local_redexes = 0`.
- `border_graph`: empty.

**When:** call predicate.

**Then:** `converged == false`.

**Assertions:** Single worker activity blocks convergence.

**SPEC-19 R covered:** R4 (worker-side activity bit).

---

### UT-0386-03: `check_delta_convergence_false_on_graph_redex`

**Purpose:** Third conjunct violation.

**Given:**
- `results`: 2 entries, all quiet (`has_border_activity = false`,
  `local_redexes = 0`).
- `border_graph`: contains one active border-redex pair (constructed via
  `BorderGraph::from_partition_plan` with two adjacent free ports of
  matching polarity — see Fixture Notes).

**When:** call predicate.

**Then:** `converged == false`.

**Assertions:** Coordinator-side cross-partition redex blocks convergence
even when all workers report quiet.

**SPEC-19 R covered:** R4 (graph-redex bit), R40.

---

### UT-0386-04: `check_delta_convergence_false_both`

**Purpose:** Both side bits violated → still `false` (no surprise OR-of-flags).

**Given:**
- `results`: 2 entries, both with `has_border_activity = true`,
  `local_redexes = 1`.
- `border_graph`: contains one active border redex.

**When:** call predicate.

**Then:** `converged == false`.

**Assertions:** Robustness — function does not accidentally return `true`
when both halves are loud.

**SPEC-19 R covered:** R4, R40.

---

### UT-0386-05: `check_delta_convergence_vacuous_empty_results_empty_graph`

**Purpose:** Edge-case input — empty workers slice + empty graph.

**Given:**
- `results: Vec<RoundResultPayload> = Vec::new()`
- `border_graph`: empty.

**When:** call predicate.

**Then:** `converged == true` (vacuous truth — `iter().all(_)` returns `true`
on empty iterator).

**Assertions:** No panic. Documented edge case (loop never reaches with
empty results in practice; this is a robustness check).

**SPEC-19 R covered:** R4 (semantics on degenerate input).

---

### UT-0386-06: `check_delta_convergence_requires_no_local_redexes`  (RENAMED + INVERTED per DC-C5)

**Purpose:** Lock the DC-C5 three-predicate ruling. Was originally
`check_delta_convergence_ignores_local_redexes` in the task-splitter draft;
DC-C5 flips the polarity to require `local_redexes == 0`.

**Target:** `merge/grid.rs::tests`

**Given:**
- `results`: 1 entry with:
  - `has_border_activity = false`
  - `stats.local_redexes = 99` (loud!)
- `border_graph`: empty.

**When:** call predicate.

**Then:** `converged == false` (was `true` in the obsolete two-predicate
draft).

**Assertions:** Documents the DC-C5 ratification — `local_redexes != 0`
blocks convergence even when border activity and graph redexes are quiet.
Doc-comment on the test cites:
- DC-C5 in `docs/spec-reviews/SPEC-19-section-3.3-2.26C-design-choices-2026-04-17.md`
- SPEC-19 R40 literal

**SPEC-19 R covered:** R4 (literal text), R40 (three-conjunct invariant).

---

### UT-0386-07: `check_delta_convergence_false_when_one_worker_has_local_redexes`  (NEW per DC-C5)

**Purpose:** Sanity case for the second conjunct — multi-worker fixture
where ONE worker has nonzero `local_redexes` and the rest are clean. Closes
the folklore gap from DC-C5 rationale point 4.

**Target:** `merge/grid.rs::tests`

**Given:**
- `results`: 3 entries:
  - W0: `has_border_activity = false`, `local_redexes = 0`
  - W1: `has_border_activity = false`, `local_redexes = 1`  (the loud one)
  - W2: `has_border_activity = false`, `local_redexes = 0`
- `border_graph`: empty.

**When:** call predicate.

**Then:** `converged == false`.

**Assertions:** Single worker's nonzero `local_redexes` blocks convergence
even if every other signal is quiet. Confirms `iter().all()` semantics.

**SPEC-19 R covered:** R4, R40 (defense-in-depth against the
"`has_border_activity = false` implies fixed point reached" folklore).

---

## Fixture Notes

**`RoundResultPayload` construction.** Tests build payloads inline with
`Default::default()` or a small helper:

```rust
fn quiet_result(worker_id: u32) -> RoundResultPayload {
    RoundResultPayload {
        worker_id: WorkerId(worker_id),
        has_border_activity: false,
        stats: WorkerRoundStats { local_redexes: 0, ..Default::default() },
        border_deltas: Vec::new(),
        partition_snapshot: None,
    }
}
```

The exact field set is locked by TASK-0384's `RoundResultPayload` definition.
Tests only set the three fields the predicate inspects.

**`BorderGraph` construction.**
- Empty: `BorderGraph::default()` or `BorderGraph::from_partition_plan(&empty_plan)`.
- One active border-redex: build a minimal `PartitionPlan` with two
  partitions whose free-port pairs match polarity and matching wire IDs;
  `BorderGraph::from_partition_plan` populates the entry; `detect_border_redexes()`
  returns `vec![1]` (or similar). The exact construction lives in §3.2 test
  helpers (`tests/border_graph_fixtures.rs` if available).

If the §3.2 bundle did not export a public `BorderGraph::test_with_one_redex()`
helper, this TEST-SPEC adds inline scaffolding inside the test module:

```rust
#[cfg(test)]
fn border_graph_with_one_active_redex() -> BorderGraph {
    let mut plan = PartitionPlan::default();
    // ... two adjacent free ports forming a CON-CON border pair ...
    BorderGraph::from_partition_plan(&plan)
}
```

---

## Coverage mapping

| Requirement / DC | Covered by |
|---|---|
| R4 — Global Normal Form predicate (literal text) | UT-0386-01, UT-0386-06, UT-0386-07 |
| R40 — three-conjunct: workers quiet + workers no local + graph empty | UT-0386-01 (positive), UT-0386-06, UT-0386-07 (DC-C5 FLIP coverage) |
| R21 phase 3 — convergence trigger (predicate semantics) | UT-0386-01 |
| First conjunct violation (border activity) | UT-0386-02 |
| Second conjunct violation (local_redexes) | UT-0386-06, UT-0386-07 |
| Third conjunct violation (graph redex) | UT-0386-03 |
| All-violated robustness | UT-0386-04 |
| Vacuous edge case (empty input) | UT-0386-05 |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|---|---|
| QA-0386-A | Refactor reverts to two-predicate (drops `local_redexes` check) | UT-0386-06, UT-0386-07 fire — DC-C5 violation |
| QA-0386-B | `iter().any()` instead of `iter().all()` (logic bug) | UT-0386-02, UT-0386-07 fire |
| QA-0386-C | Predicate uses `||` instead of `&&` between conjuncts | UT-0386-02 still passes (vacuous on `\|\|`); UT-0386-04 fires |
| QA-0386-D | Predicate inverts the `!r.has_border_activity` polarity | UT-0386-01 fires (returns `false` when should be `true`) |
| QA-0386-E | Predicate panics on empty `results` (uses `.first().unwrap()`) | UT-0386-05 fires |
| QA-0386-F | Refactor introduces `BorderGraph::detect_border_redexes()` short-circuit that re-runs `Net::reduce` mid-check (hot path) | Performance regression; not caught by these unit tests — QA candidate to add wall-time guard |
| QA-0386-G | Refactor swaps `local_redexes` with `interactions_performed` (similar field, wrong semantics) | UT-0386-07 may pass falsely if both fields are 0; QA candidate to add explicit `interactions_performed` ≠ `local_redexes` test |

---

## Acceptance gate

- `cargo test --workspace --lib` floor: +7 new `#[test]` fns. Gate
  tolerates +7 to +8.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- No regression on v1 690-test baseline.

---

## Out of scope (deferred)

- Loop-level integration of this predicate → TASK-0385 (TEST-SPEC-0385 covers).
- Final Collection trigger after `true` return → TASK-0387 (TEST-SPEC-0387).
- `max_rounds` cap interaction (predicate returns `false` but loop still
  exits) → TASK-0388 (TEST-SPEC-0388).
- Defensive `debug_assert!` on the "graph quiet implies workers quiet"
  folklore → not implemented per DC-C5 (the predicates are independent
  signals under strict BSP).
