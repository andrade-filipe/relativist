# TEST-SPEC-0389: `GridConfig.delta_mode: bool` field + Default impl (R41p)

**Task:** TASK-0389
**Spec:** SPEC-19 §3.6 — R41 (partial: `delta_mode` half; sibling `coordinator_free_rounds` shipped in TASK-0350), R42 (default-disabled backwards compatibility)
**Amendment log ref:** `docs/spec-reviews/SPEC-19-section-3.5-3.6-2.26D-design-choices-2026-04-17.md` (AMB-D-1 pre-split R41 partial-shipment split; no AMB touches TASK-0389 directly — pure field addition).
**Generated:** 2026-04-17
**Baseline before this task:** pipeline-state per bundle 2.26-D Stage 2 entry (post TEST-SPEC-0371); see bundle index for exact lib count.
**Cumulative target after this task:** baseline + 2 new `#[test]` fns (acceptance criteria: `+2 unit tests`).

---

## Scope note

TASK-0389 lands a **pure type change** inside `relativist-core/src/merge/types.rs`:

1. Append `pub delta_mode: bool` to `GridConfig` between `strict_bsp` and `coordinator_free_rounds` (canonical SPEC-19 §3.6 ordering).
2. Update the `Default` impl to set `delta_mode: false` (R42 backwards compatibility).
3. Add two inline unit tests that lock the default polarity and field settability.

**Inert field contract.** At the close of this task the field is declared but no call site reads it. Downstream:
- TASK-0390 wires `--delta-mode` through CLI.
- TASK-0391 formalises the R42 behavioural regression.
- Bundle 2.26-C's `run_grid_delta` loop (separate sub-bundle) is the only runtime consumer.

**Out of scope for this TEST-SPEC:**
- CLI parse → config plumbing (→ TEST-SPEC-0390).
- Behavioural regression vs v1 smoke baseline (→ TEST-SPEC-0391).
- ROADMAP amendment narrative (→ TEST-SPEC-0392).
- Docstring polish + doctest (→ TEST-SPEC-0393).

No `R38/R39` scaffolding in this task — proof-pending invariants are addressed in TEST-SPEC-0391 only to the extent of the R42 behavioural regression; TASK-0392 is the documentation-only vehicle for the R38/R39 narrative.

---

## Test target file paths

- `relativist-core/src/merge/types.rs` — inline `#[cfg(test)] mod tests` block (existing); two new `#[test]` fns.
- Same module hosts the precedent test pair `grid_config_default_disables_coordinator_free_rounds` / `grid_config_coordinator_free_rounds_is_settable` (TASK-0350). Both new `delta_mode` tests co-locate with those precedents.

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Unit Tests

### UT-0389-01: `grid_config_default_disables_delta_mode`

**Purpose:** R42 default-polarity lock. Asserts that `GridConfig::default().delta_mode == false`. Primary R42 regression guard against an accidental default flip (a future refactor might naively copy `coordinator_free_rounds: true` test fixture semantics into the struct default — this test fires the moment that happens).

**Target file:** `merge/types.rs::tests`

**Given:** `GridConfig` has the new `delta_mode: bool` field and a `Default` impl.

**When:** Construct `GridConfig::default()`.

**Then:**
```rust
#[test]
fn grid_config_default_disables_delta_mode() {
    let cfg = GridConfig::default();
    assert!(
        !cfg.delta_mode,
        "SPEC-19 R42: delta_mode MUST default to false for v1 backwards compatibility"
    );
}
```

**Assertions:**
- The `Default` impl builds (compile-time guard that the field was not omitted from the impl).
- `cfg.delta_mode == false` (runtime R42 lock).

**SPEC-19 R covered:** R41 (field presence in `Default` impl), R42 (default polarity is `false`).

**Proof-pending vs operational:** operational (deterministic, single-pass, no scaffolding).

---

### UT-0389-02: `grid_config_delta_mode_is_settable`

**Purpose:** Round-trip the new field through a `..GridConfig::default()` spread literal and through `Clone`. Pins three invariants in one compact test:
1. The field is `pub` (struct literal from a test module requires `pub`).
2. Field-spread construction still compiles (existing call sites use this pattern; if the new field accidentally lacked a default binding, every call site in the workspace would break — this test is the canary).
3. `Clone` copies the field (pre-existing `#[derive(Debug, Clone)]` on `GridConfig` covers this; the test re-asserts at runtime to lock the derive).

**Target file:** `merge/types.rs::tests`

**Given:** `GridConfig` has `delta_mode: bool` and derives `Clone`.

**When:** Construct via struct-spread literal, mutate the field to `true`, clone, compare both sides.

**Then:**
```rust
#[test]
fn grid_config_delta_mode_is_settable() {
    let cfg = GridConfig {
        delta_mode: true,
        ..GridConfig::default()
    };
    assert!(cfg.delta_mode, "struct-spread literal must set delta_mode");
    // All other fields retain their defaults (R42: changing delta_mode does
    // not implicitly alter siblings).
    let default_cfg = GridConfig::default();
    assert_eq!(cfg.num_workers, default_cfg.num_workers);
    assert_eq!(cfg.max_rounds, default_cfg.max_rounds);
    assert_eq!(cfg.strict_bsp, default_cfg.strict_bsp);
    assert_eq!(cfg.coordinator_free_rounds, default_cfg.coordinator_free_rounds);

    // Clone round-trip.
    let cloned = cfg.clone();
    assert!(cloned.delta_mode);
}
```

**Assertions:**
- `delta_mode: true` is read back as `true` after construction.
- No sibling field was mutated by the spread construction.
- `Clone` preserves the field value.

**SPEC-19 R covered:** R41 (field publicly settable), R42 (siblings untouched when delta_mode is flipped).

**Proof-pending vs operational:** operational.

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| R41 — `delta_mode: bool` field exists on `GridConfig` | UT-0389-01 (compile-time via struct access), UT-0389-02 (struct literal) |
| R41 — field is `pub` | UT-0389-02 (external-module struct literal) |
| R41 — field has canonical ordering (between `strict_bsp` and `coordinator_free_rounds`) | Structural, enforced at source review (no runtime test — ordering has no runtime effect; locked in TASK-0389 acceptance criteria) |
| R42 — `Default` impl sets `delta_mode: false` | UT-0389-01 |
| R42 — siblings unchanged by the field addition | UT-0389-02 |
| R42 — `Clone` derive still carries (regression guard against a future `#[derive]` reshuffle that accidentally drops `Clone`) | UT-0389-02 |
| `GridConfig { .. }` literals workspace-wide compile unchanged | Indirect — any downstream compile failure from the spread pattern would fail `cargo build` before tests run; UT-0389-02 exercises the same pattern in-module |

**Proof scaffolding note:** TASK-0389 carries NO R38/R39 proof-pending work. Those invariants are Section 8 / ARG-005 deliverables and are narratively documented in TASK-0392; no test at this level asserts mechanized proof progress.

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0389-A | `Default` impl accidentally sets `delta_mode: true` | UT-0389-01 fires — R42 violation at default |
| QA-0389-B | Field declared as `pub(crate)` instead of `pub` | UT-0389-02 fails to compile from a test submodule that imports `GridConfig` externally; in-module version still passes — flag at Stage 5 if a `pub(crate)` regression sneaks in |
| QA-0389-C | Field type widened to `Option<bool>` or `DeltaMode` enum | UT-0389-02 `delta_mode: true` literal fails to compile (type mismatch); canary |
| QA-0389-D | `#[derive(Clone)]` dropped on `GridConfig` | UT-0389-02 `.clone()` fails to compile; canary |
| QA-0389-E | Field renamed to `enable_delta` or similar | All existing call sites break (CLI plumbing in 0390, downstream grid loop) — `cargo build` fails before test execution; UT-0389-01/02 both fail to compile |
| QA-0389-F | Canonical ordering reversed (`coordinator_free_rounds` before `delta_mode`) | No runtime effect, not caught by tests; Stage 4 (reviewer) enforces the SPEC-19 §3.6 R41 ordering |
| QA-0389-G | A subtle `Default` impl refactor introduces `delta_mode: bool::default()` (still `false`) instead of the explicit `false` — semantically identical but stylistically non-conforming | UT-0389-01 passes either way; Stage 4 style review matter |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: baseline → baseline + 2 (`grid_config_default_disables_delta_mode`, `grid_config_delta_mode_is_settable`).
2. `cargo test --workspace --lib --features zero-copy` count: matches (+2, feature flag does not gate this module).
3. `cargo build --workspace` clean (default features).
4. `cargo build --workspace --features zero-copy` clean.
5. `cargo clippy --workspace --all-targets -- -D warnings` clean.
6. `cargo fmt --check` clean.

---

## Out of scope (deferred to later TEST-SPECs in the bundle)

- CLI `--delta-mode` parse + threading → TEST-SPEC-0390.
- R42 behavioural smoke regression (v1 output byte-parity with `delta_mode = false`) → TEST-SPEC-0391.
- ROADMAP §3.5 invariant amendment narrative (G1/D3/D6) → TEST-SPEC-0392.
- `delta_mode` docstring polish + doctest → TEST-SPEC-0393.
