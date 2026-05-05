# TEST-SPEC-0397: R43 normalize — `coordinator_free_rounds` auto-enabled under `delta_mode`

**See also:** [docs/backlog/TASK-0397.md](../backlog/TASK-0397.md)
  — SF-002 closure; review at `docs/reviews/REVIEW-SPEC-19-section-3.3-3.5-3.6-item-2.26-BCD-2026-04-23.md`.

**Task:** TASK-0397
**Spec:** SPEC-19 §3.6 R43 (`coordinator_free_rounds` MUST default to `true` under `delta_mode = true`).
**Spec-critic verdicts consumed:**
  - **DC-0397-A (proposed):** option (c) — unconditional override with tracing warning.
  - **DC-0397-B (proposed):** normalize call at CLI construction and as public builder; do NOT change `Default`.
  - **DC-0397-C (proposed):** method name `normalize`.
**Generated:** 2026-04-23
**Baseline before this task:** 1039 lib (default) / 1079 lib (`--features zero-copy`) — assumes this task may ship in parallel with TASK-0394/0395/0396 (all independent).
**Cumulative target after this task:** +4 tests.

---

## Scope note

This TEST-SPEC verifies SPEC-19 R43's normalization contract: default construction preserves v1 behavior (R42 baseline); setting `delta_mode=true` via any path (direct field set, CLI flag, builder method) flips `coordinator_free_rounds=true` through `.normalize()`.

Per DC-0397-A (option c, default): the normalization is an UNCONDITIONAL override — even if a caller explicitly sets `coordinator_free_rounds=false` and `delta_mode=true` simultaneously, `.normalize()` forces `coordinator_free_rounds=true`. Behavior is justified by the spec's "MUST default" wording; preservation of user intent is left for a follow-up DC revision.

The 4 tests below cover the three key axes (default path, `delta_mode=true` auto-normalization, explicit-false override) plus the CLI integration that is the most common user-facing entry point.

---

## Test target file paths

- `relativist-core/src/merge/types.rs` — inline `#[cfg(test)] mod tests` block.
  - 3 new `#[test]` fns: UT-0397-01..03.
- `relativist-core/src/config.rs` — inline `#[cfg(test)] mod tests` block (CLI integration).
  - 1 new `#[test]` fn: UT-0397-04.

All tests are synchronous. No tokio, no async.

---

## Unit Tests

### UT-0397-01: `default_grid_config_preserves_v1_R42_baseline`

**Purpose:** `Default::default()` alone (without `.normalize()`) MUST NOT change `coordinator_free_rounds`. R42 baseline: both fields `false`. This test locks in the "default is pure default" convention from DC-0397-B.

**Target:** `merge/types.rs::tests`.

**Given:** `let cfg = GridConfig::default();`.

**When:** Read `cfg.delta_mode` and `cfg.coordinator_free_rounds` directly.

**Then:**
- `cfg.delta_mode == false`.
- `cfg.coordinator_free_rounds == false`.
- (Optionally — for explicit readability) `cfg.normalize() == cfg` (normalize is idempotent on the default).

**Assertions:** `Default::default()` is still the identity for both fields; `normalize` is not silently embedded in `Default`.

**SPEC-19 R covered:** R42 baseline preservation.

---

### UT-0397-02: `normalize_with_delta_mode_true_sets_coordinator_free_rounds_true`

**Purpose:** Happy path — `delta_mode=true` + default `coordinator_free_rounds=false` → `.normalize()` flips to `true`.

**Target:** `merge/types.rs::tests`.

**Given:**
```rust
let cfg = GridConfig {
    delta_mode: true,
    coordinator_free_rounds: false,
    ..Default::default()
};
```

**When:** `let normalized = cfg.normalize();`.

**Then:**
- `normalized.delta_mode == true` (unchanged).
- `normalized.coordinator_free_rounds == true` (R43 enforcement).
- All other fields (`num_workers`, `max_rounds`, `strict_bsp`) unchanged byte-for-byte.

**Assertions:** R43 is enforced by `.normalize()`; other field values pass through transparently.

**SPEC-19 R covered:** R43 primary path.

---

### UT-0397-03: `normalize_with_delta_mode_true_and_explicit_coordinator_free_rounds_false_forces_true`

**Purpose:** DC-0397-A (option c) — even when the caller EXPLICITLY sets `coordinator_free_rounds=false`, enabling `delta_mode=true` forces `coordinator_free_rounds=true` on `.normalize()`.

**Target:** `merge/types.rs::tests`.

**Given:**
```rust
let cfg = GridConfig {
    delta_mode: true,
    coordinator_free_rounds: false,   // EXPLICIT — user may think this sticks
    ..Default::default()
};
```

**When:** `let normalized = cfg.normalize();`.

**Then:**
- `normalized.coordinator_free_rounds == true` (R43 override wins over explicit false).
- `normalized.delta_mode == true`.

**Assertions:** `normalize` is the authoritative enforcer; callers cannot silently defeat R43 by setting the field before calling `.normalize()`.

**Alternative assertion under DC-0397-A option (b):** if spec-critic flips DC-0397-A to preserve user intent, the assertion becomes `coordinator_free_rounds == false` here and the test body requires a `coordinator_free_rounds_user_set` tracking bit. Flag in TASK-0397's `Notes`.

**SPEC-19 R covered:** R43 strict-interpretation path (DC-0397-A option c).

---

### UT-0397-04: `build_grid_config_with_delta_mode_flag_normalizes_coordinator_free_rounds`

**Purpose:** CLI integration — `build_grid_config(args)` calls `.normalize()` before returning, so `--delta-mode` users get R43 behavior.

**Target:** `config.rs::tests`.

**Given:**
```rust
let args = CoordinatorArgs {
    workers: 2,
    delta_mode: true,    // the flag under test
    max_rounds: None,
    strict_bsp: false,
    // ... other args at defaults ...
};
```

**When:** `let cfg = build_grid_config(&args);`.

**Then:**
- `cfg.delta_mode == true`.
- `cfg.coordinator_free_rounds == true` (normalized at build time).
- `cfg.num_workers == 2`.

**Assertions:** The CLI path produces a config where R43 is already enforced; users calling `relativist local --workers 2 --delta-mode ...` run with coordinator-free-rounds enabled without knowing about it.

**Also verify the symmetric `build_grid_config_from_local`:** add a mirror case (can be same test fn or a sibling `_from_local` fn). Recommendation: sibling `#[test]` fn named `build_grid_config_from_local_with_delta_mode_flag_normalizes_coordinator_free_rounds` — counts as **+1 additional test** (so UT-0397-04 plus its mirror = 2, total +5). **Decision deferred to DEV:** inline both paths in one `#[test]` fn (counted as 1) OR split (counted as 2). **Default plan:** split for diagnostic clarity.

**SPEC-19 R covered:** R43 via CLI construction.

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| SPEC-19 R42 — `Default::default()` alone preserves baseline | UT-0397-01 |
| SPEC-19 R43 — `delta_mode=true` primary path | UT-0397-02 |
| SPEC-19 R43 — unconditional override (DC-0397-A option c) | UT-0397-03 |
| SPEC-19 R43 — CLI integration path | UT-0397-04 (and its `_from_local` mirror if split) |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0397-A | `.normalize()` called twice — idempotence | Calling `.normalize()` on an already-normalized config MUST NOT change anything. UT-0397-01's optional assertion covers this on the default; extend to `.normalize().normalize()` on a delta_mode config. |
| QA-0397-B | `delta_mode=false + coordinator_free_rounds=true` + `.normalize()` → stays as-is (R44 path) | The normalization should ONLY fire when `delta_mode=true`. This is the R44 case: `coordinator_free_rounds=true, delta_mode=false` is legal and MUST be preserved through `.normalize()`. **QA probe: explicit test.** |
| QA-0397-C | Missing tracing subscriber — does `tracing::debug!` cause issues? | No — tracing macros are no-ops when no subscriber is attached. QA smoke to confirm. |
| QA-0397-D | `build_grid_config` is not the only `GridConfig` construction site — are there others? | Grep `GridConfig {` across the codebase; if ANY construction site bypasses `.normalize()` for a CLI-driven path, that's a hole. Default plan: internal construction sites (tests, benchmarks) don't need normalization because they set fields explicitly. **QA: enumerate all construction sites and classify each as "CLI/user-facing" vs. "internal".** |
| QA-0397-E | Serde round-trip preserves normalization state | If `GridConfig` is serialized to JSON/CBOR (for checkpoints or IPC), the serialized form is the post-normalize state. Re-deserialize → re-normalize is idempotent (QA-0397-A). No hidden state lost. |
| QA-0397-F | `--coordinator-free-rounds=false --delta-mode=true` CLI combination | Today no `--coordinator-free-rounds` flag exists; if added later, DC-0397-A's "unconditional override" interacts with it. User's explicit `false` gets overridden → tracing warning fires → test should check for the warning string. (Out of scope for this task; flag for future when the flag is added.) |
| QA-0397-G | `normalize` called on a shared `&mut GridConfig` vs. owned `GridConfig` | Current default plan: `normalize(self) -> Self` consumes `self`. If mutation-based `normalize(&mut self)` is preferred, DC-0397-C adjusts the signature. QA confirm one or the other is chosen consistently. |
| QA-0397-H | `PartialEq` on `GridConfig` — does `normalize()` fiat alter equality comparisons in tests? | UT-0397-03's `assert_ne!(cfg, cfg.normalize())` confirms that `.normalize()` does CHANGE the value (not a no-op on non-normalized input). Test: `cfg != cfg.normalize()` when `delta_mode=true` and `coordinator_free_rounds=false`. |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: baseline → baseline + **4** (or **5** if UT-0397-04 splits CLI/local_main).
2. `cargo test --workspace --lib --features zero-copy` count: +4 (or +5).
3. `cargo build --workspace` clean.
4. `cargo build --workspace --features zero-copy` clean.
5. `cargo clippy --workspace --all-targets -- -D warnings` clean.
6. `cargo fmt --check` clean.
7. Existing `grid_config_default_disables_delta_mode` test (`types.rs:440-445`) still GREEN.
8. Existing `r42_default_delta_mode_preserves_v1_smoke_output` test still GREEN.
9. Doc-comment on `coordinator_free_rounds` field updated to describe R43 normalization.
10. `normalize` is `pub` (callers can invoke programmatically); field visibility unchanged.

---

## Notes on interaction with other TEST-SPECs

- **TEST-SPEC-0391 (R42 regression — 2.26-D polish):** existing test uses `GridConfig::default()` + explicit `delta_mode=false`. On `.normalize()` the branch not taken, so R42 is preserved. UT-0397-01 is a narrower sibling canary.
- **TEST-SPEC-0385 UT-0385-06..08 (MF-002 via TASK-0395):** the G1 parity tests construct `GridConfig { delta_mode: true, ..Default::default() }` and may or may not call `.normalize()`. **Recommendation:** TASK-0395's UT-0385-06..08 should call `.normalize()` to exercise the integrated state. Update TEST-SPEC-0385 / TASK-0395 DEV if not already.
- **TEST-SPEC-0396 UT-0396-02 (SF-001 router):** the router test passes `GridConfig { delta_mode: true }`; after TASK-0397 ships, the normalized config will also have `coordinator_free_rounds=true`. Router semantics unchanged.

---

## Out of scope

- **`--coordinator-free-rounds` CLI flag** — not added today; TASK-0397 assumes only `--delta-mode` is user-facing. A future task may add the flag + respect its user value per a revised DC-0397-A.
- **`Default::default()` auto-normalization** — rejected per DC-0397-B. Default stays "pure default"; `.normalize()` is explicit.
- **Performance comparison with / without coordinator-free-rounds under delta_mode** — R43 enforcement is binary (flag flipped); downstream benchmarks measure the effect. Not a test-level concern.
- **Coordinator-side enforcement of R43** — the coordinator's round loop already branches on `coordinator_free_rounds` (shipped 2.34). This task only ensures the flag reaches the coordinator in the right state; downstream logic is unchanged.
