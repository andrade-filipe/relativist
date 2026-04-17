# TEST-SPEC-0350: `coordinator_free_rounds` config flag + metrics counter

**Task:** TASK-0350
**Spec:** SPEC-19 §3.1 R6 (SHOULD under strict BSP), R41 (config flag),
R43 (default = false in v1 mode), R45 partial (`coordinator_free_rounds: u32`
counter only — per-round delta vectors deferred to item 2.26).
**Generated:** 2026-04-16
**Baseline before this task:** 856+ (post-TASK-0348; parallel with TASK-0349)
**Cumulative target after this task:** 858+ (≥ +2 new tests)

---

## Scope note

This task adds **only data**, not behavior:

- `GridConfig.coordinator_free_rounds: bool` (R41, default `false` per R43 SHOULD).
- `GridMetrics.coordinator_free_rounds: u32` (R45 partial, default `0`).

No existing code path changes. Every existing test, benchmark, and CLI
invocation MUST produce a bit-identical result. Wiring the consumption
of these fields belongs to TASK-0351.

R6's gating ("SHOULD use under strict BSP") is enforced by the
*consumer* in TASK-0351, NOT here. This task only proves that the data
plumbing is in place and inert.

---

## Test target file paths

- `relativist-core/src/merge/types.rs` — extend existing `#[cfg(test)] mod tests`
  block with two new defaults assertions.

All tests are synchronous `#[test]` units.

---

## Unit Tests

### UT-0350-01: `grid_config_default_disables_coordinator_free_rounds` (R43)

**Purpose:** R43 SHOULD — in v1 mode (which is the only mode shipped),
the flag defaults to `false`. Preserves bit-identical behavior for every
existing caller.
**Target file:** `merge/types.rs::tests`
**Preconditions:** None.

**Input:**
```rust
let cfg = GridConfig::default();
```

**Expected output:**
```rust
assert!(!cfg.coordinator_free_rounds,
        "R43 SHOULD: v1 mode defaults to false");
// Spot-check that no other defaulted field changed.
assert_eq!(cfg.num_workers, 1);
assert!(cfg.strict_bsp);   // or whatever current default is
```

**SPEC-19 R covered:** R6 (SHOULD enforcement carrier), R41 (field exists),
R43 (default value).

---

### UT-0350-02: `grid_metrics_default_zero_coordinator_free_rounds` (R45 partial)

**Purpose:** R45 partial — counter is initialized to `0` and exists on
the metrics struct so TASK-0351 can `+=`.
**Target file:** `merge/types.rs::tests`
**Preconditions:** None.

**Input:**
```rust
let m = GridMetrics::default();
```

**Expected output:**
```rust
assert_eq!(m.coordinator_free_rounds, 0u32,
           "R45 partial: counter must default to 0");
```

**SPEC-19 R covered:** R45 partial (counter only — vectors out of scope).

---

## Implicit "no behavior change" coverage (existing tests)

The existing 850-test baseline acts as the regression net for this task:
since the new flag defaults `false` and the counter defaults `0`, every
existing test that constructs `GridConfig::default()` or
`GridMetrics::default()` keeps producing the same result. No new assertion
is needed — failure to maintain this property surfaces as a regression
in the existing suite.

If the test-splitter discovers any inline test fixture using
`GridConfig { num_workers, max_rounds, strict_bsp }` field-by-field
(without `..default()`), TASK-0350 must **either** add the new field
explicitly **or** refactor that fixture to use `..GridConfig::default()`.
The same applies to `GridMetrics { … }` fixtures. Either approach is
acceptable; the ergonomic preference is `..default()` for forward
compatibility.

---

## Adversarial probes (QA candidates for Stage 5)

| # | Scenario | Why dangerous | Stage |
|---|----------|---------------|-------|
| QA-0350-A | `coordinator_free_rounds = true` while `strict_bsp = false` (lenient mode) | TASK-0351 will gate the skip on `strict_bsp = true`; this combination is permitted by config but observably inert. QA should confirm no skip occurs (covered concretely as TEST-SPEC-0351 UT-05 — flagged here as the entry point). | QA / cross-task |
| QA-0350-B | Decode a serialized `GridConfig` produced before the new field was added | If `GridConfig` is serialized anywhere on disk (e.g. checkpoint or replay file), bincode v2 struct-append rules apply: forward-compat only when both sides are rebuilt | QA |
| QA-0350-C | Set `coordinator_free_rounds = true` programmatically + observe counter stays `0` after a round whose stats DO contain border activity | Verifies the increment is gated on the right predicate (TASK-0351 territory; flagged here for cross-task QA) | QA / cross-task |

---

## R6 cross-task gating contract (informational)

The `coordinator_free_rounds` flag is **decoupled** from `strict_bsp`
at the type level — both are independent `bool`s on `GridConfig`. The
consumer in TASK-0351 enforces R6 by ANDing them in the skip predicate:

```rust
if all_no_border && config.coordinator_free_rounds && config.strict_bsp { … }
```

TEST-SPEC-0351 covers the AND truth table (UT-05 specifically asserts
that `(true, false)` does NOT skip).

---

## Acceptance Gate

1. `cargo test --workspace` count: 856 → **858+** (≥ +2: UT-01, UT-02).
2. All previously passing tests still pass (no regression — every
   `GridConfig::default()` / `GridMetrics::default()` call remains valid).
3. `cargo clippy --workspace --all-targets -- -D warnings` clean.
4. `cargo fmt --check` clean.
5. Release smoke `compute add 3 5 → 8` works (the CLI doesn't expose the
   new flag yet — out of scope for this bundle per task notes).

## Out of Scope

- CLI flag for `coordinator_free_rounds` — explicitly out of bundle.
- Counter increment logic (TASK-0351).
- The other R45 fields (`border_deltas_received_per_round`, etc.) —
  deferred to item 2.26.
- R44 (decoupling delta_mode from coordinator_free_rounds) — applies
  to the bigger delta protocol; for THIS bundle, `delta_mode` does not
  exist yet, so R44 is vacuously satisfied.
