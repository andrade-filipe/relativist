# TEST-SPEC-0396: R20 dispatcher fork — `run_grid_entry` routes on `cfg.delta_mode`

**See also:** [docs/backlog/TASK-0396.md](../backlog/TASK-0396.md)
  — SF-001 closure; review at `docs/reviews/REVIEW-SPEC-19-section-3.3-3.5-3.6-item-2.26-BCD-2026-04-23.md`.

**Task:** TASK-0396
**Spec:** SPEC-19 §3.3 R20 (delta_mode MUST gate delta BSP loop).
**Spec-critic verdicts consumed:**
  - **DC-0395-A (shared with TASK-0395):** test-support feature gate for `run_grid_delta` / `WorkerDispatch` exposure under `#[cfg(feature = "test-support")]`.
  - **DC-0396-A (proposed):** entry function name `run_grid_entry`.
  - **DC-0396-B (proposed):** panic on `delta_mode=true + dispatch=None`, not Result.
  - **DC-0396-C (proposed):** keep `run_grid_delta` as `pub(crate)` + feature-gated re-export.
**Generated:** 2026-04-23
**Baseline before this task:** depends on TASK-0394 and TASK-0395 landing first (estimated 1053 lib / 1093 zero-copy post both).
**Cumulative target after this task:** +4 tests (1057 lib / 1097 zero-copy).

---

## Scope note

This TEST-SPEC verifies the R20 dispatcher contract: `run_grid_entry` observes `cfg.delta_mode` and dispatches accordingly. The 4 tests cover the three behavior axes (false, true+dispatch, true+None) plus an end-to-end smoke that the router does not introduce regressions on a known-good workload.

Integration-level G1 parity between v1 and v2 paths is already covered by TASK-0395's UT-0385-06..08; this TEST-SPEC focuses on the ROUTER boundary, not the end-to-end equivalence.

---

## Test target file paths

- `relativist-core/src/merge/grid.rs` — inline `#[cfg(test)] mod tests` block.
  - 4 new `#[test]` fns: UT-0396-01..04.
- OR the tests may live in `relativist-core/tests/grid_delta_roundloop.rs` (if TASK-0395's integration file is already open for additions and co-locating with LocalDeltaDispatch is cleaner). **Default plan:** inline in `grid.rs::tests` for 3 out of 4; UT-0396-04 may move to the integration file to reuse `LocalDeltaDispatch`.

All tests are synchronous `#[test]` units. No tokio, no async.

---

## Unit Tests

### UT-0396-01: `run_grid_entry_with_delta_mode_false_delegates_to_v1`

**Purpose:** Happy path A — `delta_mode=false` routes to `run_grid` (v1); `dispatch` argument may be `None` and is unused.

**Target:** `merge/grid.rs::tests` (inline).

**Given:** `config = GridConfig { delta_mode: false, num_workers: 2, ..Default::default() }`; a small net (e.g., single CON-CON internal redex). No `WorkerDispatch` created.

**When:** `run_grid_entry(net, &config, &ContiguousIdStrategy, None)`.

**Then:**
- Returned `(out, metrics)` MUST be byte-identical to `run_grid(net_clone, &config, &ContiguousIdStrategy)`.
- `metrics.delta_mode == false` (or unset — whichever semantics match the v1 path's metrics emission).

**Assertions:** The router correctly skips the delta branch when `delta_mode=false` AND does not require a `WorkerDispatch`.

**SPEC-19 R covered:** R20 (false branch).

---

### UT-0396-02: `run_grid_entry_with_delta_mode_true_and_dispatch_delegates_to_delta`

**Purpose:** Happy path B — `delta_mode=true + dispatch=Some(...)` routes to `run_grid_delta`.

**Target:** `tests/grid_delta_roundloop.rs` (integration — needs `LocalDeltaDispatch` from TASK-0395) OR inline if a mock dispatch is simpler. **Default plan:** integration, to reuse `LocalDeltaDispatch`.

**Given:** `config = GridConfig { delta_mode: true, num_workers: 2, ..Default::default() }`; a small net (same fixture as UT-0396-01 for direct comparability); `let mut dispatch = LocalDeltaDispatch::new(2)`.

**When:** `run_grid_entry(net, &config, &ContiguousIdStrategy, Some(&mut dispatch))`.

**Then:**
- Returned `(out, metrics)` MUST equal `run_grid_delta(net_clone, &config, &ContiguousIdStrategy, &mut dispatch_clone)` when run standalone (modulo dispatch state).
- `metrics.delta_mode == true`.
- `metrics.rounds >= 1` (delta always runs at least Round 0 + 1 delta round).

**Assertions:** The router forwards to the delta path; no behavior substitution.

**SPEC-19 R covered:** R20 (true branch).

---

### UT-0396-03: `run_grid_entry_with_delta_mode_true_and_no_dispatch_panics`

**Purpose:** Pre-condition assertion — calling with `delta_mode=true + dispatch=None` panics with a descriptive message.

**Target:** `merge/grid.rs::tests` (inline).

**Given:** `config = GridConfig { delta_mode: true, num_workers: 2, ..Default::default() }`; a trivial net.

**When:** `run_grid_entry(net, &config, &ContiguousIdStrategy, None)` inside `std::panic::catch_unwind(AssertUnwindSafe(|| { ... }))`.

**Then:**
- The `catch_unwind` Result MUST be `Err(_)`.
- The panic payload (after `downcast_ref::<String>()` or `downcast_ref::<&str>()`) MUST contain the substring `"SPEC-19 R20"` AND `"delta_mode"` AND `"WorkerDispatch"` (case-insensitive).
- Panic message MUST NOT leak internal implementation details (e.g., `"unwrap()"` — use `.expect(...)` with a clear message).

**Assertions:** Pre-condition panic with a grep-able message that makes the error discoverable.

**SPEC-19 R covered:** R20 (error path when caller breaks the contract).

---

### UT-0396-04: `run_grid_entry_roundtrip_matches_legacy_run_grid_on_church_add_smoke`

**Purpose:** Regression — the R42 smoke test (`r42_default_delta_mode_preserves_v1_smoke_output` at `grid.rs:1968-2039`) that asserts `church_add(2, 3) → Church(5)` through `run_grid(delta_mode=false)` must remain equivalent when routed through `run_grid_entry`.

**Target:** `merge/grid.rs::tests` (inline; sibling to the existing R42 test).

**Given:** Church encoding of `add(2, 3)` built via `encoding::build_add`; `config = GridConfig { delta_mode: false, num_workers: 2, ..Default::default() }`; no dispatch.

**When:** Compare three invocations:
1. `legacy = run_grid(net.clone(), &config, &ContiguousIdStrategy)`.
2. `entry_false = run_grid_entry(net.clone(), &config, &ContiguousIdStrategy, None)`.
3. Existing R42 test baseline (for reference — not re-run here, but trusted).

**Then:**
- `legacy.total_interactions == entry_false.total_interactions`.
- `legacy.rounds == entry_false.rounds`.
- `legacy.interactions_by_rule == entry_false.interactions_by_rule`.
- `decode_nat(legacy.out) == 5 == decode_nat(entry_false.out)`.

**Assertions:** Router does not introduce metric drift on the v1 path. This IS the regression canary for R42 under the new dispatcher.

**SPEC-19 R covered:** R20 (routing correctness), R42 (v1 preservation).

**Note:** If UT-0396-04 starts failing, it means the router is NOT a transparent pass-through on the v1 path — likely an off-by-one in metric aggregation or an accidental double-invocation. Bisect TASK-0396's DEV against UT-0396-04's failure signature.

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| SPEC-19 R20 — `delta_mode=false` routes v1 | UT-0396-01 |
| SPEC-19 R20 — `delta_mode=true + dispatch` routes delta | UT-0396-02 |
| SPEC-19 R20 — `delta_mode=true + no dispatch` explicit failure | UT-0396-03 |
| SPEC-19 R42 — v1 path preserved through router | UT-0396-04 |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0396-A | `run_grid_entry(net, cfg{delta_mode: false}, strat, Some(&mut dispatch))` — passing a dispatch that's never used | Router ignores dispatch on the v1 branch; dispatch is silently dropped (if it had a Drop impl with side effects, those fire). **QA probe: dispatch's Drop is observed.** Expected: no observable change vs. UT-0396-01. |
| QA-0396-B | `run_grid_entry` with `config.num_workers = 0` | `run_grid_delta` has `assert!(config.num_workers >= 1)`; verify the panic propagates through the router. |
| QA-0396-C | `run_grid_entry` called from multiple threads simultaneously | Not supported (BSP is sequential); but if someone tries, what happens? v1 path is self-contained; delta path depends on mutable dispatch — Send+Sync on the trait may already forbid this. **QA: confirm Send+Sync story.** |
| QA-0396-D | `run_grid_entry` called with `config.max_rounds = Some(0)` and `delta_mode = true` | Delta loop should exit immediately with `delta_max_rounds_hit = Some(true)`; verify router surfaces this cleanly. |
| QA-0396-E | Re-entrancy: UT-0396-04 calls `run_grid` inside the test, then `run_grid_entry` — are net identities shared? | Net is cloned between invocations; should be safe. Verify no hidden global state. |
| QA-0396-F | Panic on `None` — message drift | If a refactor changes the panic message, UT-0396-03's substring checks should catch it. But QA probe: grep the binary for the literal message, confirm it's unique. |
| QA-0396-G | CLI invocation with `--delta-mode` but no coordinator mode available | `commands.rs::local_main` currently calls `run_grid` directly. After this task routes through `run_grid_entry`, `local_main` must construct an `Option<dyn WorkerDispatch>` — today's only in-process dispatch is `LocalDeltaDispatch` from tests. Production CLI with `--delta-mode` SHOULD NOT panic with UT-0396-03's message; instead the CLI must short-circuit with a clean error before calling `run_grid_entry`. **This is the placeholder error from TASK-0396 acceptance criterion §5.** QA probe: verify the CLI's clean error message. |
| QA-0396-H | `run_grid_entry` returns — metrics' `delta_mode` field correctness | If `GridMetrics.delta_mode: Option<bool>` is set by `run_grid_delta` but not by `run_grid`, the router's v1 path may leave it `None` or `false`. Confirm the metrics match per-path expectations; ensure UT-0396-04's `metrics.delta_mode` assertion is explicit. |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: baseline (post-TASK-0394 + TASK-0395) → baseline + **4**.
2. `cargo test --workspace --lib --features zero-copy` count: +4.
3. `cargo build --workspace` clean.
4. `cargo build --workspace --features zero-copy` clean.
5. `cargo clippy --workspace --all-targets -- -D warnings` clean.
6. `cargo fmt --check` clean.
7. `TODO(2.26-D)` comment at `grid.rs:346-349` REMOVED.
8. R42 regression test (existing `r42_default_delta_mode_preserves_v1_smoke_output`) still GREEN.
9. `run_grid_entry` is `pub`; `run_grid_delta` visibility per DC-0396-C (default: `pub(crate)` + feature-gate).
10. CLI `--delta-mode` + no-dispatch mode returns a CLEAN error (not a panic) per TASK-0396 §5 placeholder contract.

---

## Notes on interaction with other TEST-SPECs

- **TEST-SPEC-0394 (MF-001):** this TEST-SPEC's UT-0396-02 depends on correctness of worker-side `handle_round_start` (TASK-0394 shipped). If TASK-0394 is broken, UT-0396-02 still passes AS-A-ROUTING-TEST (because `run_grid_delta` executes) but the net-output assertion might be nonsense. UT-0396-02's default assertion is metadata-level (metrics, rounds ≥ 1); do not add correctness assertions that duplicate TEST-SPEC-0385's work.
- **TEST-SPEC-0385 UT-0385-06..08 (MF-002 via TASK-0395):** the true G1 parity check lives there. This TEST-SPEC's UT-0396-04 is a narrower regression canary for the R42 smoke path under the router — a necessary but not sufficient check.
- **TEST-SPEC-0391 (R42 regression — TASK-0391):** that task's scope is R42 proper. UT-0396-04 here is a sibling regression canary routing through the new dispatcher. If TASK-0391's DEV is deferred or partial, UT-0396-04 still runs independently.

---

## Out of scope

- **TCP dispatch binding** — the real `impl WorkerDispatch for CoordinatorConnection` lives in `protocol/coordinator.rs` per DC-C2; that async binding is deferred beyond this refactor bundle.
- **CLI coordinator-mode end-to-end** — `relativist coordinator --delta-mode` on real TCP. Requires the TCP dispatch binding above.
- **Performance comparison router vs. direct `run_grid`** — the router is a thin wrapper; performance assertions are not part of this TEST-SPEC.
- **`LocalDeltaDispatch` exposure in prod builds** — `test-support` feature gate per DC-0395-A; tests opt in via dev-dependency config.
- **Cross-crate exposure** — `run_grid_entry` is `pub` within `relativist-core`; re-export from `relativist-cli` or downstream benchmark crates is a separate gate (not covered here).
