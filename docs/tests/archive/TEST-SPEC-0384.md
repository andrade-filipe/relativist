# TEST-SPEC-0384: `run_grid_delta` scaffolding + `WorkerDispatch` trait + `delta_mode` dispatch

**Task:** TASK-0384
**Spec:** SPEC-19 §3.3 R20 (`GridConfig.delta_mode` dispatch), R21 (lifecycle frame).
**Spec-critic amendments incorporated:**
- DC-C2 (ratified, option C) — pure-core `run_grid_delta` + synchronous `WorkerDispatch` trait;
  async impl lives in `coordinator.rs` (OUT of bundle)
- DC-C3 (FLIP from task-splitter default A → option C) — REMOVE the `strict_bsp` hard-assert.
  `run_grid_delta` MUST accept BOTH `(delta_mode=true, strict_bsp=true)` AND
  `(delta_mode=true, strict_bsp=false)`. Branching logic is the round loop's job (TASK-0385).
**Provenance:** `docs/spec-reviews/SPEC-19-section-3.3-2.26C-design-choices-2026-04-17.md` §DC-C2, §DC-C3
**Generated:** 2026-04-17

---

## Scope note

TASK-0384 ships the OUTER SHELL of the delta-mode coordinator entry
point:

1. New trait `WorkerDispatch` in `merge/types.rs` with three
   SYNCHRONOUS methods (`dispatch_initial`, `dispatch_round_start`,
   `dispatch_final_state_request`).
2. New struct `RoundResultPayload` in `merge/types.rs` (pure-core
   mirror of `Message::RoundResult` with added `worker_id`).
3. New fields on `GridMetrics`: `delta_mode: bool` (default `false`),
   `delta_max_rounds_hit: Option<bool>` (default `None`).
4. New public function `run_grid_delta(net, config, strategy, dispatch)
   -> (Net, GridMetrics)` in `merge/grid.rs`.
5. File-local stub `run_grid_delta_inner(...) -> Result<Net, GridError>`
   that `todo!()`s — the real loop is TASK-0385.

**DC-C3 firewall:** the entry point MUST NOT assert `strict_bsp == true`.
Both values are legal per SPEC-19 R40. The four-cell matrix
`delta_mode × strict_bsp` is fully defined; the entry shell touches
none of its cells (it just routes to the inner loop).

**DC-C4 stub:** `GridConfig.delta_mode` lands in 2.26-D. Until then,
the `run_grid_delta` function is unconditionally delta-mode (caller
chooses entry point). A `// TODO(2.26-D)` comment marks the stub.
Tests UT-0384-01..05 do NOT depend on the field existing.

---

## Test target file paths

- `relativist-core/src/merge/grid.rs` — inline `#[cfg(test)] mod tests`.
  Five new `#[test]` fns.
- `relativist-core/src/merge/types.rs` — co-locate fixture helpers if
  needed (`MockDispatch` / `NoopDispatch` types may live in
  `merge/grid.rs::tests` to keep them out of production code).

All tests are synchronous. No `tokio`, no `async`.

---

## Test fixtures

`NoopDispatch` — minimal `WorkerDispatch` impl used by tests that need
to assert the dispatch path is taken (or NOT taken). Stores call-count
counters per method; methods return `Ok(Vec::new())` for round_start /
final_state_request and `Ok(())` for dispatch_initial. Lives in
`#[cfg(test)] mod tests` block.

```rust
struct NoopDispatch {
    initial_calls: usize,
    round_start_calls: usize,
    final_state_calls: usize,
}
```

---

## Unit Tests

### UT-0384-01: `run_grid_delta_accepts_both_strict_bsp_values`  (DC-C3 firewall)

**Purpose:** Lock DC-C3 ruling — the entry point MUST accept
`strict_bsp ∈ {true, false}` without panicking. Replaces the
deprecated `run_grid_delta_rejects_nonstrict_bsp` test (which encoded
the WRONG semantics per spec-critic).

**Target:** `merge/grid.rs::tests`

**Given:** Two configs differing ONLY in `strict_bsp`:
- `cfg_strict = GridConfig { num_workers: 2, strict_bsp: true, ..Default::default() }`
- `cfg_lenient = GridConfig { num_workers: 2, strict_bsp: false, ..Default::default() }`
- A net with at least one redex (so we don't short-circuit on the
  already-normalized branch).
- `NoopDispatch` for both calls.

**When:** Call `run_grid_delta(net.clone(), &cfg_strict, &strategy, &mut dispatch1);`
AND separately `run_grid_delta(net.clone(), &cfg_lenient, &strategy, &mut dispatch2);`

**Then:**
- Neither call panics on the strict_bsp value.
- Both calls return `(_, metrics)` with `metrics.delta_mode == true`.
- (The inner loop is `todo!()` per TASK-0384, so the round loop itself
  may panic; this test should drive a code path that EXITS BEFORE the
  inner loop runs — e.g. via the already-normalized short-circuit
  in step UT-0384-02. Until TASK-0385 lands, defer this test or
  mark `#[ignore]` and re-enable post-TASK-0385.)

**Note for the developer:** if the round-loop stub `todo!()` makes
this test unrunnable until TASK-0385, mark with
`#[ignore = "enable once TASK-0385 implements run_grid_delta_inner"]`
and lock the test count via the `#[ignore]` line.

**Assertions:** No `assert!(config.strict_bsp, ...)` panic at the
entry point.

**SPEC-19 R covered:** R20, R40 (4-cell matrix completeness) + DC-C3.

---

### UT-0384-02: `run_grid_delta_short_circuits_on_normalized_net`

**Purpose:** Pre-loop short-circuit — if the input net has no redexes,
return immediately with `metrics.converged = true`, `metrics.rounds = 0`,
`metrics.delta_mode = true`. Inner loop is NOT entered, so the
`todo!()` stub is not hit.

**Target:** `merge/grid.rs::tests`

**Given:**
- A net with empty `redex_queue` after `drain_stale_redexes` (use the
  trivial empty net or a net of disconnected `Erase` agents).
- `cfg = GridConfig { num_workers: 2, ..Default::default() }`.
- `NoopDispatch`.

**When:** `let (_net, metrics) = run_grid_delta(net, &cfg, &strategy, &mut dispatch);`

**Then:**
- `metrics.converged == true`
- `metrics.rounds == 0`
- `metrics.delta_mode == true`
- `metrics.delta_max_rounds_hit == None`
- `dispatch.initial_calls == 0` (didn't dispatch — already normalized)
- `dispatch.round_start_calls == 0`
- `dispatch.final_state_calls == 0`.

**Assertions:** Short-circuit path is taken; no dispatch I/O fired.

**SPEC-19 R covered:** R20 (entry shell), R4 (Normal Form detection
trivially satisfied).

---

### UT-0384-03: `run_grid_delta_delegates_single_worker_to_run_single_worker`

**Purpose:** `n == 1` degenerate case is delegated to the existing
`run_single_worker` path (no dispatch I/O). Mirrors v1 `run_grid`.

**Target:** `merge/grid.rs::tests`

**Given:**
- A net with at least one CON-CON redex.
- `cfg = GridConfig { num_workers: 1, ..Default::default() }`.
- `NoopDispatch`.

**When:** Call `run_grid_delta(net, &cfg, &strategy, &mut dispatch);`

**Then:**
- `dispatch.initial_calls == 0` and `dispatch.round_start_calls == 0`
  AND `dispatch.final_state_calls == 0` (single-worker path bypassed
  the dispatch trait entirely).
- `metrics.rounds == 1` (single-worker path performs one local
  reduction round — matches v1 `run_grid`'s single-worker contract).
- `metrics.delta_mode == true` (still flagged as delta-mode entry).

**Assertions:** Single-worker shortcut runs WITHOUT exercising
`WorkerDispatch`.

**SPEC-19 R covered:** R20 + R21 (single-worker degenerate).

---

### UT-0384-04: `grid_metrics_default_delta_fields_are_off`

**Purpose:** Lock the new `GridMetrics` field defaults — `delta_mode:
false`, `delta_max_rounds_hit: None`. v1 code paths that build
`GridMetrics::default()` (e.g. v1 `run_grid` start) MUST NOT see
either field flip.

**Target:** `merge/grid.rs::tests` OR `merge/types.rs::tests`.

**Given:** Call `GridMetrics::default()`.

**When:** Inspect fields.

**Then:**
- `metrics.delta_mode == false`
- `metrics.delta_max_rounds_hit == None`.

**Assertions:** Both new fields default to "off" (v1 backwards
compatibility — same R42 spirit as the `delta_mode` config field).

**SPEC-19 R covered:** R20 (delta_mode marker default), R30 (max-rounds
indicator default).

---

### UT-0384-05: `worker_dispatch_trait_is_object_safe`

**Purpose:** Compile-time guarantee that `&mut dyn WorkerDispatch` is
constructible — i.e. all three methods are object-safe (no generic
parameters on methods, no `Self` return types, no async). If a future
refactor adds a generic method or an async signature to the trait,
this test fails to compile.

**Target:** `merge/grid.rs::tests`.

**Given:** A `NoopDispatch` value.

**When:** Construct `&mut dyn WorkerDispatch` and call each method.

**Then:**
```rust
let mut dispatch = NoopDispatch::default();
let dispatch_ref: &mut dyn WorkerDispatch = &mut dispatch;
let _ = dispatch_ref.dispatch_initial(&plan);
let _ = dispatch_ref.dispatch_round_start(&dispatch_payload);
let _ = dispatch_ref.dispatch_final_state_request(0);
```

**Assertions:** Trait object is usable. (Compile-time test — body just
exercises the trait once for each method.)

**SPEC-19 R covered:** DC-C2 (ratified) — synchronous trait shape locked.

---

## Coverage mapping

| Requirement / DC | Covered by |
|---|---|
| R20 — entry-point dispatch | UT-0384-01, UT-0384-02, UT-0384-03 |
| R20 — `delta_mode` marker on metrics | UT-0384-02, UT-0384-04 |
| R21 phase 1 — Round 0 dispatch path entry | UT-0384-02 (pre-dispatch), UT-0384-03 (single-worker) |
| R30 — `delta_max_rounds_hit` default `None` | UT-0384-04 |
| R40 — 4-cell matrix completeness (entry side) + DC-C3 | UT-0384-01 |
| DC-C2 (ratified) — `WorkerDispatch` synchronous trait shape | UT-0384-05 |
| Short-circuit on already-normalized | UT-0384-02 |
| `n == 1` degenerate (delegate to `run_single_worker`) | UT-0384-03 |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|---|---|
| QA-0384-A | Future refactor reintroduces `assert!(config.strict_bsp, ...)` at entry | UT-0384-01 fires — DC-C3 violation |
| QA-0384-B | `WorkerDispatch::dispatch_round_start` made `async` | UT-0384-05 compile-fails (async fns not object-safe in stable Rust without `#[async_trait]` workaround) |
| QA-0384-C | `WorkerDispatch` gains a generic method | UT-0384-05 compile-fails |
| QA-0384-D | `GridMetrics::default()` flipped `delta_mode: true` accidentally | UT-0384-04 fires — would break every v1 metric assertion |
| QA-0384-E | Single-worker path silently calls `dispatch.dispatch_initial` | UT-0384-03 fires (call count > 0) |
| QA-0384-F | `run_grid_delta_inner` `todo!()` reached during UT-0384-02 (short-circuit not taken) | Test panics with "not yet implemented" — short-circuit broken |
| QA-0384-G | Future refactor changes `RoundResultPayload` field types | Downstream tests in TEST-SPEC-0385 fire (positional bincode-style match arms) |

---

## Acceptance gate

- `cargo test --workspace --lib` floor: +5 new `#[test]` fns
  (UT-0384-01 may need to be `#[ignore]`'d until TASK-0385 lands the
  inner loop body; the test count line still advances by +5 because
  cargo counts ignored tests).
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- No regression on v1 690-test baseline. v1 `run_grid` untouched.

---

## Out of scope (deferred)

- Round loop body (Round 0 dispatch + R21 phase 2 loop) → TEST-SPEC-0385.
- Convergence predicate → TEST-SPEC-0386.
- Final State Collection → TEST-SPEC-0387.
- `max_rounds` cap wiring → TEST-SPEC-0388.
- Async `impl WorkerDispatch for CoordinatorConnection` in
  `coordinator.rs` → 2.26-C-wire or 2.26-D.
- `GridConfig.delta_mode` field landing → TASK-0389 (2.26-D).
