# TEST-SPEC-0388: `max_rounds` cap + non-convergence indicator (R30)

**Task:** TASK-0388
**Spec:** SPEC-19 R30 (preserve `max_rounds` cap; on hit, MUST initiate
  Final State Collection and return partial net with non-convergence
  indicator).
**Spec-critic notes:** No DC-Cn directly amends TASK-0388. Implicit DC-C3
  ties: this task assumes both `(delta_mode=true, strict_bsp ∈ {true,false})`
  cells of R40 already work (TASK-0385 lands them). DC-C5's three-predicate
  convergence semantics determine when the cap is the ONLY exit reason.
**Generated:** 2026-04-17

---

## Scope note

TASK-0388 ships:

1. The pure helper `check_max_rounds_cap(config, metrics) -> bool`.
2. The wiring in TASK-0385's `run_grid_delta_inner` loop that, when the
   helper returns `true`, sets `metrics.delta_max_rounds_hit = Some(true)`
   AND `metrics.converged = false`, then breaks to Final Collection.
3. Distinguishing-state contract: natural convergence leaves
   `delta_max_rounds_hit = None` and sets `converged = true`. Cap-hit sets
   both fields.
4. **v1 non-regression guard:** v1 `run_grid` MUST NEVER touch
   `delta_max_rounds_hit`. Verified by a new lib test that runs an existing
   v1 integration and asserts the field stays `None`.

**Helper signature:**

```rust
pub(crate) fn check_max_rounds_cap(
    config: &GridConfig,
    metrics: &GridMetrics,
) -> bool {
    match config.max_rounds {
        None => false,
        Some(m) => metrics.rounds >= m,
    }
}
```

---

## Test target file paths

- `relativist-core/src/merge/grid.rs` — inline `#[cfg(test)] mod tests`.
  Five new `#[test]` fns for the pure helper.
- `relativist-core/tests/grid_delta_maxrounds.rs` — NEW integration test
  file. Three new `#[test]` fns for end-to-end loop behavior.
- One additional v1-regression `#[test]` fn placed inline in
  `relativist-core/src/merge/grid.rs` (or in `tests/grid_v1_regression.rs`
  if a dedicated v1 regression file already exists).

All tests are synchronous.

---

## Unit Tests (inline in `merge/grid.rs`)

### UT-0388-01: `check_max_rounds_cap_none_returns_false`

**Purpose:** Unbounded mode — `max_rounds = None` always returns `false`.

**Given:**
- `config.max_rounds = None`.
- `metrics.rounds`: try several values (0, 100, `usize::MAX`) via parameterized
  loop or three sub-asserts.

**When:** call `check_max_rounds_cap(&config, &metrics)`.

**Then:** Always `false`.

**Assertions:** `None` is the unbounded sentinel; never trips the cap.

**SPEC-19 R covered:** R30 (preserve v1 unbounded mode).

---

### UT-0388-02: `check_max_rounds_cap_below_returns_false`

**Purpose:** Below-cap counter → no cap.

**Given:**
- `config.max_rounds = Some(5)`, `metrics.rounds = 3`.

**When:** call helper.

**Then:** `false`.

**SPEC-19 R covered:** R30.

---

### UT-0388-03: `check_max_rounds_cap_at_returns_true`

**Purpose:** Boundary inclusivity — `rounds >= max` (NOT `>`).

**Given:**
- `config.max_rounds = Some(5)`, `metrics.rounds = 5`.

**When:** call helper.

**Then:** `true`.

**Assertions:** Cap is inclusive — when the loop is about to start round 6
(having completed rounds 0..=4 and incremented the counter to 5), the cap
fires. Matches v1 `run_grid` semantics.

**SPEC-19 R covered:** R30 (boundary semantics).

---

### UT-0388-04: `check_max_rounds_cap_above_returns_true`

**Purpose:** Strict-above check (defense against off-by-one regressions).

**Given:**
- `config.max_rounds = Some(5)`, `metrics.rounds = 100`.

**When:** call helper.

**Then:** `true`.

**SPEC-19 R covered:** R30.

---

### UT-0388-05: `check_max_rounds_cap_zero_immediately_true`

**Purpose:** Edge case — `Some(0)` means "do no rounds at all"; cap fires
on the first loop iteration before ANY work.

**Given:**
- `config.max_rounds = Some(0)`, `metrics.rounds = 0`.

**When:** call helper.

**Then:** `true`.

**Assertions:** Matches v1 `run_grid` `Some(0)` semantics.

**SPEC-19 R covered:** R30 (zero-cap edge case).

---

## Integration Tests (in `tests/grid_delta_maxrounds.rs`)

### IT-0388-01: `run_grid_delta_respects_max_rounds_cap`

**Purpose:** End-to-end — net that needs ≥5 rounds to converge with
`max_rounds = Some(2)` exits at the cap, runs Final Collection per R30,
and reports `delta_max_rounds_hit == Some(true)`.

**Target:** NEW `tests/grid_delta_maxrounds.rs`.

**Given:**
- A `Net` constructed to require ≥5 BSP rounds to reach Normal Form when
  split across 2 workers (e.g., a chain of border-redexes that each round
  resolves only one). Easiest fixture: a Church numeral `mul 5 5` or a
  hand-built 5-deep chain.
- `GridConfig { workers: 2, max_rounds: Some(2), strict_bsp: true, ... }`.
- In-process mock dispatch (same `InProcessTwoWorkerDispatch` style as
  IT-0387-01).

**When:** `let (net, metrics) = run_grid_delta_with_metrics(net.clone(), &config, &mut dispatch)?;`

**Then:**
- `metrics.rounds == 2` (cap fired after round 2 completed and counter
  incremented; loop checks at top of round 3 attempt).
- `metrics.delta_max_rounds_hit == Some(true)`.
- `metrics.converged == false`.
- The returned `net` is NOT in Normal Form (has remaining redexes;
  assert via `net.has_active_redex() == true` or by partition agent
  count > 0 with known unresolved structure).
- `metrics.merge_time_per_round.len() == 3` (2 round merges + 1 final
  collection merge — adjust if implementation records only the final).
- Final Collection actually ran (not just an early return) — assert
  `net.live_agent_count() > 0` (output is a partial net, not an empty
  fallback).

**Assertions:** Cap path produces a partial net per R30 — caller can
distinguish "partial" via `delta_max_rounds_hit`.

**SPEC-19 R covered:** R30 (cap + Final Collection on cap).

---

### IT-0388-02: `run_grid_delta_natural_convergence_leaves_delta_max_rounds_hit_none`

**Purpose:** The negative case — when natural convergence wins the race,
`delta_max_rounds_hit` stays `None`, distinguishing exit reasons.

**Given:**
- A `Net` that converges in 1 round (e.g., 4-CON-CON fixture from
  IT-0387-01).
- `GridConfig { workers: 2, max_rounds: Some(100), strict_bsp: true, ... }`.

**When:** call `run_grid_delta`.

**Then:**
- `metrics.delta_max_rounds_hit == None` (NOT `Some(false)` — the field
  is the indicator-only flag; "natural" exit leaves it untouched).
- `metrics.converged == true`.
- `metrics.rounds <= 2`.
- Net is in Normal Form (`net.has_active_redex() == false`).

**Assertions:** Caller can check `delta_max_rounds_hit.is_some()` to
detect "non-convergence-class" exits without inspecting `converged`
(though both fields agree here).

**SPEC-19 R covered:** R30 (None vs Some distinguishes exit class), R4
(GNF correctness preserved).

---

### IT-0388-03: `run_grid_delta_zero_max_rounds_returns_partial_immediately`

**Purpose:** `Some(0)` edge case — zero rounds runs zero loop iterations,
Final Collection still runs (per R30 literal "MUST initiate Final State
Collection"), output is the trivial union of input partitions.

**Given:**
- Any `Net`.
- `GridConfig { workers: 2, max_rounds: Some(0), strict_bsp: true, ... }`.

**When:** call `run_grid_delta`.

**Then:**
- `metrics.rounds == 0`.
- `metrics.delta_max_rounds_hit == Some(true)`.
- `metrics.converged == false`.
- Returned net's `live_agent_count` equals the original input net's
  `live_agent_count` (no agents lost — Final Collection assembled the
  Round-0 partitions back together via `merge()`).
- The returned net is structurally equivalent to the input net modulo
  partition splitting/merging artifacts (any wire renumbering done by
  `split_partition_plan` ↔ `merge` round-trip).

**Assertions:** Zero-cap path is well-defined and matches v1 `Some(0)`
behavior (which also runs `merge()` on the initial split).

**SPEC-19 R covered:** R30 (zero-cap + R30 mandate of Final Collection
even on cap).

---

## v1 Non-Regression Guard

### UT-0388-06: `grid_metrics_v1_never_sets_delta_max_rounds_hit`

**Purpose:** Defense-in-depth — verify that v1 `run_grid` does not touch
the new `delta_max_rounds_hit` field. Catches accidental cross-pollination
during v2 development.

**Target:** Inline in `merge/grid.rs::tests`, or in
`relativist-core/tests/grid_v1_regression.rs` if that file exists. This
TEST-SPEC defaults to inline.

**Given:**
- A v1-style fixture: any small `Net` (e.g., 2-CON-CON), invoke v1
  `run_grid(net, &config)` with `config.max_rounds = None`, `Some(0)`,
  and `Some(100)` (three sub-cases).

**When:** Three v1 invocations.

**Then:** For all three:
- `metrics.delta_max_rounds_hit == None` (untouched by v1).

**Assertions:** v1 path is sterile w.r.t. the v2-specific field. A v2
helper sneaking into v1 `run_grid` (e.g., via shared loop refactor) trips
this test.

**SPEC-19 R covered:** R30 (v1 non-regression discipline).

**Bonus assertion (if cheap):** `grep` invariant — no occurrence of
`delta_max_rounds_hit` inside the body of `pub fn run_grid` (lexical
discipline). Implemented as a doc-comment note in this TEST-SPEC; the
actual lib test relies on runtime observation, not source-text grep.

---

## Fixture Notes

**5+ round non-convergence fixture (IT-0388-01).** Two options:

- **Option A (recommended):** hand-build a 5-deep chain of border redexes
  where each round resolves exactly one, by constructing a partition plan
  with two workers and 5 cross-partition CON-DUP pairs arranged so that
  each round's resolution exposes the next.
- **Option B (fallback):** `church_mul(5, 5)` — known to need many rounds.
  Uses `encoding/` module helpers (already shipped). Test asserts only
  `rounds == 2` (cap), not exact convergence trajectory.

If Option A fixture is non-trivial to build, Option B is the test-spec
default. Either way, the test only asserts (i) cap fired, (ii) partial
net returned, (iii) `delta_max_rounds_hit = Some(true)`.

**`InProcessTwoWorkerDispatch`.** Reuse the fixture from TEST-SPEC-0387
IT-0387-01. May need to add a counter to ensure the dispatch's RoundStart
handler returns `has_border_activity = true` for at least 5 rounds (i.e.,
the workers don't accidentally converge before the cap fires). For the
`mul(5,5)` fixture this is automatic.

**v1 `run_grid` invocation.** Standard — `run_grid(net, &config)` returns
`(Net, GridMetrics)`. No mock dispatch needed (v1 uses internal
single-process scheduler).

---

## Coverage mapping

| Requirement / DC | Covered by |
|---|---|
| R30 — `max_rounds` cap preservation (helper semantics) | UT-0388-01..05 |
| R30 — `max_rounds = None` is unbounded | UT-0388-01 |
| R30 — boundary inclusivity (`>=`) | UT-0388-03 |
| R30 — zero-cap edge case | UT-0388-05, IT-0388-03 |
| R30 — cap hit triggers Final Collection | IT-0388-01, IT-0388-03 |
| R30 — `delta_max_rounds_hit = Some(true)` on cap | IT-0388-01, IT-0388-03 |
| R30 — `converged = false` on cap | IT-0388-01, IT-0388-03 |
| R30 — natural convergence leaves `delta_max_rounds_hit = None` | IT-0388-02 |
| R30 — `converged = true` on natural convergence | IT-0388-02 |
| R30 — partial net is well-formed (Final Collection ran) | IT-0388-01, IT-0388-03 |
| R30 — v1 `run_grid` non-regression (untouched field) | UT-0388-06 |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|---|---|
| QA-0388-A | Loop uses `>` instead of `>=` (off-by-one) | UT-0388-03 fires |
| QA-0388-B | Cap-hit path SKIPS Final Collection (early `return Ok(net)` before `run_grid_delta_final_collect`) | IT-0388-01 fires (returned net is the pre-merge Vec\<Partition\>, not a merged Net) |
| QA-0388-C | Cap-hit path forgets `converged = false` (defaults to `true`) | IT-0388-01 asserts `converged == false` |
| QA-0388-D | Cap-hit path forgets `delta_max_rounds_hit = Some(true)` | IT-0388-01 fires |
| QA-0388-E | Natural convergence accidentally sets `delta_max_rounds_hit = Some(false)` | IT-0388-02 fires (asserts `== None`, not `!= Some(true)`) |
| QA-0388-F | Refactor merges v1 and v2 loop bodies; v1 starts touching `delta_max_rounds_hit` | UT-0388-06 fires |
| QA-0388-G | `Some(0)` path panics in `split_partition_plan` (zero rounds path doesn't even reach split) | IT-0388-03 fires |
| QA-0388-H | Cap-hit metrics record `merge_time_per_round` for the zero rounds (extra spurious entry) | IT-0388-01 / IT-0388-03 assert exact `len()` |
| QA-0388-I | `usize::MAX` rounds counter overflow when adding +1 in unbounded mode | UT-0388-01 sub-case `rounds = usize::MAX` covers; helper itself is overflow-safe (no arithmetic) |
| QA-0388-J | Cap firing leaks the workers' `WorkerContext` (no shutdown signal sent) | Not caught by these tests; QA candidate to add `Drop` instrumentation |

---

## Acceptance gate

- `cargo test --workspace --lib` floor: +5 unit + 1 v1-regression = +6
  inline `#[test]` fns.
- `cargo test --workspace --test grid_delta_maxrounds` floor: +3 new
  `#[test]` fns.
- Combined: +9 tests across the bundle attributable to TASK-0388.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- No regression on v1 690-test baseline. v1 `run_grid` behavior bit-identical.

---

## Out of scope (deferred)

- Lenient-BSP cap interaction (`strict_bsp = false` + `max_rounds`) → DC-C3
  cell `(true, false)`; tested in TEST-SPEC-0385, not here.
- `Message::Shutdown` dispatch on cap → existing v1 FSM (out of bundle).
- Coordinator-side telemetry/log on cap-hit → SPEC-11 observability.
- `delta_max_rounds_hit` exposure to CLI / human-readable report → SPEC-07.
- Per-worker partial-state retention on cap (workers may still hold their
  `delta_state` post-`Returning`) → SPEC-19 R28 already mandates `.take()`
  in `handle_final_state_request`; tested in TEST-SPEC-0383.
- Cross-cap-and-natural race conditions (cap fires same round as
  convergence) → ordering: TASK-0385 checks cap FIRST, so cap wins on tie;
  test covered indirectly by IT-0388-01 (set `max_rounds` to exactly the
  natural-convergence round count).
