# TEST-SPEC-0390: `--delta-mode` CLI flag threads through `CoordinatorArgs` + `LocalArgs` (R41p, R42)

**Task:** TASK-0390
**Spec:** SPEC-19 §3.6 — R41 (CLI plumbing half; field landed by TASK-0389), R42 (absent flag ⇒ `delta_mode = false` ⇒ no behavioral regression)
**Amendment log ref:** `docs/spec-reviews/SPEC-19-section-3.5-3.6-2.26D-design-choices-2026-04-17.md` (AMB-D-1 partial-ship precedent for compound R41; AMB-D-2 behavioral-not-source reading of R42 — both directly applicable to this task's CLI surface). **No AMB amendment touches TASK-0390 directly**; the task-splitter's design stands (pure additive `#[arg(long, default_value_t = false)]` plumbing mirroring TASK-0358's `use_zero_copy`).
**Generated:** 2026-04-17
**Baseline before this task:** post TEST-SPEC-0389 — cumulative 970 default lib / 1010 `--features zero-copy` (968 pre-bundle baseline + 2 from TASK-0389). Bundle-level anchor is the pre-bundle 968 / 1008 recorded in `docs/backlog/SPEC-19-section-3.5-3.6-config-amendments-tasks.md`.
**Cumulative target after this task:** **+3** new `#[test]` fns — 973 default lib / 1013 `--features zero-copy`.

---

## Scope note

TASK-0390 adds the CLI surface for the `delta_mode` field that TASK-0389 landed on `GridConfig`. Scope:

1. Append `pub delta_mode: bool` with `#[arg(long, default_value_t = false)]` to `CoordinatorArgs` in `relativist-core/src/config.rs` (position: after `use_zero_copy`, matching SPEC-19 §3.6 canonical ordering).
2. Append the same field/attribute to `LocalArgs` (after `strict_bsp`).
3. Thread `args.delta_mode` into `GridConfig.delta_mode` inside `build_grid_config` and `build_grid_config_from_local`.
4. Extend the existing `make_coordinator_args` fixture (around L989) with `delta_mode: false` (a `make_local_args` fixture is NOT added — only existing fixtures are extended; see TASK-0390 Acceptance Criteria line 6).
5. Add 3 new unit tests pinning: default polarity, coordinator CLI round-trip, local CLI round-trip.

**Inert field contract.** As of this task's land, no production read-path consumes `GridConfig.delta_mode`; the flag is therefore a **parse-only opt-in** with zero behavioral effect. Downstream:
- TASK-0391 formalises the R42 behavioral regression smoke.
- TASK-0392 documents the R38/R39/R40 narrative (ROADMAP-only).
- TASK-0393 polishes the `delta_mode` docstring + adds a doctest.
- Sub-bundle 2.26-C's `run_grid_delta` is the eventual consumer.

**Out of scope for this TEST-SPEC:**
- `GridConfig.delta_mode` field presence + default polarity → TEST-SPEC-0389.
- Behavioral smoke regression against v1 `church_add(2,3)` baseline → TEST-SPEC-0391.
- ROADMAP §3.5 invariant amendment narrative (G1/D3/D6) → TEST-SPEC-0392.
- Doctest on `GridConfig` → TEST-SPEC-0393.

**Worker subcommand deliberately excluded** (task spec §Notes): `worker --delta-mode` would be a user-facing red herring — workers inherit delta mode via the wire protocol (sub-bundle 2.26-A), not via a local CLI flag. No test here asserts worker flag absence explicitly; Stage 4 reviewer reads the `WorkerArgs` struct directly.

**`compute` subcommand deliberately excluded** for the same reason (task spec §Notes).

---

## Test target file paths

- `relativist-core/src/config.rs` — inline `#[cfg(test)] mod tests` block. All 3 new `#[test]` fns + a 1-line extension of the `make_coordinator_args` fixture (see §Fixture extension below). No new test files.

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Fixture extension (not a new test; precondition for tests below)

`make_coordinator_args` (existing helper around `relativist-core/src/config.rs:989`) gains a `delta_mode: false` field in its returned literal. This single-line edit is MANDATORY before UT-0390-01 compiles. No `make_local_args` helper is added by this task (TASK-0390 Acceptance Criteria line 6 explicitly forbids it); `LocalArgs` literals inside the three new tests are constructed inline.

---

## Unit Tests

### UT-0390-01: `cli_delta_mode_default_is_false_on_coordinator_and_local`

**Purpose:** R42 default-polarity lock at the CLI layer, BOTH subcommands in one test. Asserts that the absence of `--delta-mode` on either subcommand produces `args.delta_mode = false` AND the threaded `GridConfig.delta_mode = false`. This is the targeted R42 guard at the CLI boundary (TEST-SPEC-0389 UT-0389-01 is the same guard at the type boundary; TEST-SPEC-0391 is the end-to-end smoke).

**Target file:** `config.rs::tests`

**Given:** `make_coordinator_args(4, None)` returns a `CoordinatorArgs` with no `--delta-mode`; a hand-constructed `LocalArgs` literal with `delta_mode: false` explicit.

**When:** Build the `GridConfig` for both via `build_grid_config` / `build_grid_config_from_local`.

**Then:**
```rust
#[test]
fn cli_delta_mode_default_is_false_on_coordinator_and_local() {
    // Coordinator side
    let coord_args = make_coordinator_args(4, None);
    assert!(
        !coord_args.delta_mode,
        "SPEC-19 R42: absence of --delta-mode on coordinator MUST leave \
         CoordinatorArgs.delta_mode = false"
    );
    let coord_cfg = build_grid_config(&coord_args);
    assert!(
        !coord_cfg.delta_mode,
        "SPEC-19 R42: the derived GridConfig MUST also carry delta_mode = false"
    );

    // Local side — construct LocalArgs inline (no make_local_args helper per
    // TASK-0390 §Acceptance Criteria line 6)
    let local_args = LocalArgs {
        workers: 2,
        input: std::path::PathBuf::from("test.bin"),
        // ... all other LocalArgs fields populated with reasonable defaults,
        //     matching the existing pattern used by any pre-existing inline
        //     LocalArgs literal in config::tests ...
        delta_mode: false,
        ..LocalArgs::default()  // if Default is impl'd; otherwise list fields
    };
    assert!(!local_args.delta_mode);
    let local_cfg = build_grid_config_from_local(&local_args);
    assert!(!local_cfg.delta_mode);
}
```

**Assertions:**
- `CoordinatorArgs` default parse leaves `delta_mode = false`.
- `LocalArgs` literal with `delta_mode: false` propagates to `GridConfig`.
- Neither side silently flips the field.

**SPEC-19 R covered:** R41 (field reaches `GridConfig` from both subcommands), R42 (absence ⇒ false ⇒ no behavioral change at the parse layer).

**Proof-pending vs operational:** operational (deterministic, single-pass).

---

### UT-0390-02: `cli_delta_mode_flag_threads_through_coordinator`

**Purpose:** `clap` parses `--delta-mode` on `coordinator`, and the flag reaches `GridConfig.delta_mode` via `build_grid_config`. R41 CLI surface for the coordinator subcommand.

**Target file:** `config.rs::tests`

**Given:** An argv slice for the `coordinator` subcommand including `--delta-mode` plus the minimal required args (`--workers`, `--input`) already used by other `config::tests` precedents (e.g., the `cli_use_zero_copy_*` pair from TASK-0358).

**When:** `Cli::try_parse_from` → destructure → build config.

**Then:**
```rust
#[test]
fn cli_delta_mode_flag_threads_through_coordinator() {
    let cli = Cli::try_parse_from([
        "relativist", "coordinator",
        "--workers", "4",
        "--input", "input.bin",
        "--delta-mode",
    ]).expect("clap should parse --delta-mode on coordinator");
    match cli.command {
        Command::Coordinator(args) => {
            assert!(
                args.delta_mode,
                "--delta-mode MUST set CoordinatorArgs.delta_mode = true"
            );
            let cfg = build_grid_config(&args);
            assert!(
                cfg.delta_mode,
                "build_grid_config MUST thread args.delta_mode into GridConfig"
            );
        }
        other => panic!("expected Command::Coordinator, got {:?}", other),
    }
}
```

**Assertions:**
- `clap` accepts the long-form `--delta-mode` without value (`default_value_t = false` + presence = `true`).
- The flag lands on `args.delta_mode`.
- `build_grid_config` copies it to `GridConfig.delta_mode`.

**SPEC-19 R covered:** R41 (coordinator CLI half threaded end-to-end).

**Proof-pending vs operational:** operational.

---

### UT-0390-03: `cli_delta_mode_flag_threads_through_local`

**Purpose:** Same end-to-end verification for the `local` subcommand. Local-mode is symmetric to coordinator-mode for this flag (TASK-0390 §Acceptance Criteria line 2).

**Target file:** `config.rs::tests`

**Given:** An argv slice for the `local` subcommand including `--delta-mode` plus `--workers 2 --input test.bin`.

**When:** `Cli::try_parse_from` → destructure → `build_grid_config_from_local`.

**Then:**
```rust
#[test]
fn cli_delta_mode_flag_threads_through_local() {
    let cli = Cli::try_parse_from([
        "relativist", "local",
        "--workers", "2",
        "--input", "test.bin",
        "--delta-mode",
    ]).expect("clap should parse --delta-mode on local");
    match cli.command {
        Command::Local(args) => {
            assert!(args.delta_mode, "LocalArgs.delta_mode MUST be true");
            let cfg = build_grid_config_from_local(&args);
            assert!(
                cfg.delta_mode,
                "build_grid_config_from_local MUST thread the flag to GridConfig"
            );
        }
        other => panic!("expected Command::Local, got {:?}", other),
    }
}
```

**Assertions:**
- `clap` accepts the long-form flag on `local`.
- The flag lands on `LocalArgs.delta_mode`.
- `build_grid_config_from_local` copies it to `GridConfig.delta_mode`.

**SPEC-19 R covered:** R41 (local CLI half threaded end-to-end).

**Proof-pending vs operational:** operational.

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| R41 — `CoordinatorArgs.delta_mode` field present + `#[arg(long)]` attribute | UT-0390-02 (clap accepts the long form → compile + parse prove field presence) |
| R41 — `LocalArgs.delta_mode` field present + `#[arg(long)]` attribute | UT-0390-03 (same) |
| R41 — `build_grid_config` threads `args.delta_mode` → `GridConfig.delta_mode` | UT-0390-01 (false case), UT-0390-02 (true case) |
| R41 — `build_grid_config_from_local` threads `args.delta_mode` → `GridConfig.delta_mode` | UT-0390-01 (false case), UT-0390-03 (true case) |
| R42 — absence of `--delta-mode` on coordinator parse ⇒ `delta_mode = false` | UT-0390-01 (coord half), indirect in every pre-existing `config::tests` that never passes `--delta-mode` (regression-free by the same reasoning TASK-0358 used for `use_zero_copy`) |
| R42 — absence of `--delta-mode` on local parse ⇒ `delta_mode = false` | UT-0390-01 (local half) |
| R42 — every pre-existing `config::tests` continues to pass | Bundle-level (no per-test assertion here; the acceptance gate below verifies it) |
| Worker subcommand does NOT accept `--delta-mode` | Structural (not tested — `WorkerArgs` lacks the field; any test would need to invoke `clap::try_parse_from` with `--delta-mode` on worker and expect an error, which is brittle against `clap`'s error taxonomy. Stage 4 reviewer verifies by reading `WorkerArgs` directly.) |
| `compute` subcommand does NOT accept `--delta-mode` | Structural (same — not runtime-tested) |

**Proof scaffolding note:** TASK-0390 carries NO R38/R39 proof-pending work. Those invariants are Section 8 / ARG-005 deliverables and are narratively documented in TASK-0392. No test at this layer marks `#[ignore]` for proof deferral (the proof-pending pattern is confined to runtime-behaviour stubs for future bundles; CLI plumbing has no analogue).

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0390-A | Default-value flip: `#[arg(long, default_value_t = true)]` | UT-0390-01 fires on both halves — R42 regression |
| QA-0390-B | CLI flag named `--delta_mode` (underscore) instead of `--delta-mode` (hyphen) — `clap` default renames `field_name` → `--field-name` | UT-0390-02/03 fail to parse with hyphen; canary. Stage 4 verifies the attribute matches the attribute on the `use_zero_copy` precedent. |
| QA-0390-C | `build_grid_config` drops the `delta_mode: args.delta_mode` line (copy-paste omission) | UT-0390-02 fires — the threaded `cfg.delta_mode` stays at `Default` (`false`) while `args.delta_mode == true` |
| QA-0390-D | `build_grid_config_from_local` drops the threading (asymmetry bug) | UT-0390-03 fires; UT-0390-02 stays green (coordinator-only coverage would hide the bug without this local-half test) |
| QA-0390-E | Flag added to `WorkerArgs` (out-of-scope per §Notes) | No test here catches it; Stage 4 reviewer checks `WorkerArgs` struct diff (or Stage 5 QA greps for `--delta-mode` in worker args) |
| QA-0390-F | `CoordinatorArgs` fixture `make_coordinator_args` not updated with `delta_mode: false` — test file fails to compile | Compile-time canary; Stage 2 test-generator notes this in §Fixture extension above |
| QA-0390-G | `LocalArgs` fixture (if one existed) not updated — this task FORBIDS adding one | Architecture-level; Stage 4 reviewer grep for `make_local_args` after this PR |
| QA-0390-H | `#[arg(long, default_value_t = false, short = 'd')]` added by a future PR | Stage 4 reviewer veto; conflicts with other short flags. Not tested here (no short-form coverage). |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 970 → **973** (+3 new `#[test]` fns; 0 `#[ignore]` stubs).
2. `cargo test --workspace --lib --features zero-copy` count: 1010 → **1013** (+3; feature flag does not gate `config::tests`).
3. `cargo build --workspace` clean (default features).
4. `cargo build --workspace --features zero-copy` clean.
5. `cargo clippy --workspace --all-targets -- -D warnings` clean.
6. `cargo fmt --check` clean.
7. Every pre-existing `config::tests` test that exercised `Cli::try_parse_from` on `coordinator` or `local` without `--delta-mode` continues to pass without modification (R42 behavioral-invariance check at the unit-test layer — indirect, relies on the hard floor "test count MUST NOT decrease" rule).

---

## Out of scope (deferred to later TEST-SPECs in the bundle)

- R42 behavioural smoke regression against v1 baseline (`church_add(2,3)` snapshot) → TEST-SPEC-0391.
- ROADMAP §3.5 invariant amendment narrative (G1/D3/D6) → TEST-SPEC-0392.
- `delta_mode` docstring polish + doctest demonstrating opt-in pattern → TEST-SPEC-0393.
- Worker-side delta-mode signalling via wire protocol → sub-bundle 2.26-A (InitialPartition or equivalent).
- `run_grid_delta` dispatch when `cfg.delta_mode == true` → sub-bundle 2.26-C.
