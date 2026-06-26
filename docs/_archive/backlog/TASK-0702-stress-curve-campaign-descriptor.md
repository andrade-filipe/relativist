# TASK-0702 — D-014-DESCRIPTOR: `stress-curve` campaign descriptor in `bench/suite.rs`

**Phase:** D-014 (Stress Curve Campaign) — Stage 3 DEV scope
**Bundle:** D-014 — Stress Curve Campaign
**Status:** TODO
**Priority:** P0 (defines the matrix the script + plot consume).
**Spec:** none.
**Depends on:** TASK-0700 (`MemoryProbe`), TASK-0701 (`StopRule`).
**Estimated complexity:** S–M (~80 LoC production + ~60 LoC integration test).

---

## Context

`bench/suite.rs` already defines the `BenchmarkSuite` matrix runner used by every locked baseline (D-010, D-011, D-012). The stress-curve campaign adds **one more campaign descriptor** — it does NOT replace any existing one. The descriptor encodes the matrix from design doc §4.4: 3 workloads × 2 envs × 4 worker counts × 11 N values × 5 reps, with `--release`, `--recycle-policy disable-under-delta`, `--streaming-strategy round-robin`, `--chunk-size 1000`.

The descriptor exposes:
- A canonical `n_seq` sequence (×√10 from 10⁴ to 10⁹).
- The 3 workload identifiers (`ep_annihilation`, `dual_tree`, `condup_expansion`) — already exist as generators per design doc §4.1.
- The `StopRule` defaults per env (5 min in-process, 7m30s docker, 80% RAM gate).
- An entry-point function the script (TASK-0704) invokes per `(workload, env, W)` triple.

This task does NOT execute reps — the script orchestrates child processes. This task wires the descriptor into the existing `bench` CLI so `relativist bench --campaign stress-curve --workload ep_annihilation --env in_process --workers 2` produces a single `(workload, W)` sequence's worth of CSV rows.

> **CLI crate location resolved (2026-05-06).** The workspace has exactly two member crates: `relativist-core` (library) and `relativist-cli` (binary named `relativist`, path `src/main.rs`). There is **no** `relativist-bench` binary crate (the splitter's hint #2 was speculative; verified via `cargo metadata` against `Cargo.toml` workspace members). The `bench` subcommand is dispatched in `relativist-cli/src/main.rs:63` via `Command::Bench(args) => commands::run_bench_command(args)`, where `commands::run_bench_command` lives in `relativist-core/src/commands.rs:288` and consumes `BenchArgs` defined in `relativist-core/src/config.rs:571`. Therefore: add `--campaign` to `BenchArgs` and the dispatch branch to `run_bench_command`; no change to `relativist-cli/src/main.rs`. The `--workload`, `--env`, `--workers` knobs already partially exist (`workers` is `Vec<u32>` at line 581-582; `mode: String` at 585-586 currently encodes `local` vs `tcp` and may be reused as the `env` axis — verify before adding a parallel `env` flag).

## Files in scope (file:line pointers)

| File | Change |
|------|--------|
| `relativist-core/src/bench/suite.rs` | **MODIFY.** Add `pub struct StressCurveDescriptor { ... }` + `impl StressCurveDescriptor` with `n_seq() -> &'static [usize]`, `default_stop_rule(env: Env) -> StopRule`, and `run_one_sequence(workload, env, w) -> SequenceOutcome`. ~80 LoC. |
| `relativist-core/src/bench/mod.rs` | **MODIFY (if `suite` re-exports a public surface).** Re-export `StressCurveDescriptor` if other modules need it. ~1 line. |
| `relativist-core/src/config.rs` (`BenchArgs` struct, line 571) | **MODIFY.** Add `--campaign Option<CampaignKind>`, `--workload Option<StressWorkload>`, `--env Option<Env>` clap args. Reuse existing `--workers Vec<u32>` (line 581) — no new flag needed for that axis. ~25 LoC. |
| `relativist-core/src/commands.rs` (`run_bench_command`, line 288) | **MODIFY.** Add the `if args.campaign == Some(CampaignKind::StressCurve)` dispatch branch that calls `StressCurveDescriptor::run_one_sequence(workload, env, w)`; preserves existing path when `--campaign` absent. ~15 LoC. |
| `relativist-cli/src/main.rs` | **NO CHANGE NEEDED** — existing `Command::Bench(args) => commands::run_bench_command(args)` at line 63 already routes correctly. |
| `relativist-core/tests/d014_stress_curve_descriptor.rs` | **CREATE.** Smoke integration test invoking `run_one_sequence` with a tiny `n_seq` override (`[1_000, 10_000]`) and `wall_budget = 30s`, verifying `SequenceOutcome.completed_reps.len() == 2` and `stop_reason == None`. ~60 LoC. |

## Files explicitly OUT of scope

- The bash script orchestration — TASK-0704.
- CSV column additions — TASK-0703.
- The plot generator — TASK-0705.
- Generators (`ep_annihilation`, `dual_tree`, `condup_expansion`) — already exist per design doc §4.1, this task only references them by name.
- Frozen `results/locked/` directories.

## Required public API

```rust
// relativist-core/src/bench/suite.rs (additive)

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Env { InProcess, DockerTcp }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StressWorkload {
    EpAnnihilation,
    DualTree,
    CondupExpansion,
}

pub struct StressCurveDescriptor;

impl StressCurveDescriptor {
    /// The campaign's canonical 11-point N sweep (×√10 from 10⁴ to 10⁹).
    pub fn n_seq() -> &'static [usize];

    /// Per-env defaults: 5min in-process, 7m30s docker, 80% RAM both.
    pub fn default_stop_rule(env: Env) -> StopRule;

    /// Run a single (workload, env, W) sequence. Drives reps through the
    /// existing bench infrastructure; consumes `MemoryProbe` for the RAM
    /// gate; consumes `StopRule` for sequence termination.
    pub fn run_one_sequence(
        workload: StressWorkload,
        env: Env,
        workers: usize,
        reps: usize,
        n_seq_override: Option<&[usize]>,    // None = use canonical
        stop_rule_override: Option<StopRule>,
    ) -> Result<SequenceOutcome, BenchError>;
}
```

## Acceptance criteria

1. `StressCurveDescriptor::n_seq()` returns exactly `[10_000, 31_623, 100_000, 316_228, 1_000_000, 3_162_278, 10_000_000, 31_622_776, 100_000_000, 316_227_766, 1_000_000_000]` (11 entries, matches design doc §4.4 verbatim).
2. `default_stop_rule(InProcess).wall_budget == Duration::from_secs(300)`; `default_stop_rule(DockerTcp).wall_budget == Duration::from_secs(450)`; both have `memory_fraction_max == 0.80`.
3. `run_one_sequence` returns `SequenceOutcome` with one `RepResult` per completed (N, rep) pair. The outcome's `stop_reason` is `None` if the sequence completes; populated if any rule fires.
4. `run_one_sequence` propagates the `BenchError::MemoryProbe(...)` from TASK-0700 if the host platform is unsupported (macOS) — graceful failure, not panic.
5. `relativist-bench --campaign stress-curve --workload ep_annihilation --env in_process --workers 1 --reps 1 --n-seq 1000,10000` runs a 2-row sequence and emits stdout-friendly CSV.
6. New integration test `d014_stress_curve_descriptor`:
   - On HEAD before this change: FAILS to compile (descriptor doesn't exist).
   - On HEAD after this change: PASSES.
7. `cargo test` floor: **+1 default = ≥ 1809** (cumulative with TASK-0700+0701).
8. `cargo test --features zero-copy` floor: **+1 = ≥ 1853**.
9. `cargo test --features streaming-no-recycle` floor: **+1 = ≥ 1800**.
10. `cargo test --release` floor: **+1 = ≥ 1751**.
11. v1 floor (690) inviolable.
12. `cargo clippy --all-features -- -D warnings` clean.
13. `cargo fmt --check` clean.

## Test floor delta

**+1 default** (one new integration test binary). Cumulative after TASK-0700+0701+0702:
- default ≥ 1809
- zero-copy ≥ 1853
- streaming-no-recycle ≥ 1800
- release ≥ 1751

## Implementation hints

1. Keep the `StressCurveDescriptor` zero-state — it's a namespace, not stateful. All methods are associated functions.
2. The CLI dispatch should route `stress-curve` BEFORE the existing campaign matchers — verify with `relativist-bench --help` after the change to ensure no regression on existing `--campaign` values.
3. The descriptor's `run_one_sequence` calls into the **existing** generator + reduction path; no logic change to `partition/`, `reduction/`, `merge/`, `protocol/`. It's a thin orchestrator.
4. For `in_process` env: invoke generator + `run_grid` directly inside the bench process (single rep per process is the script's responsibility, not this task's).
5. For `docker_tcp` env: panic with `BenchError::Unsupported("docker_tcp orchestration is shell-driven; descriptor only emits the matrix")` — the script in TASK-0704 spawns `docker compose run` and feeds the resulting CSV back into the aggregated dataset; the descriptor itself does not orchestrate Docker.
6. Reuse the existing `BenchmarkSuiteConfig` plumbing where possible (e.g., the `--chunk-size 1000` and recycle-policy flags should hit the same code path the D-012 baseline used). DO NOT duplicate config logic.

## Estimated LoC

- Production: ~80 LoC (struct + 3 methods) + ~30 LoC CLI dispatch = ~110 LoC.
- Tests: ~60 LoC integration test.
- Total: ~170 LoC. Under the 200 LoC ceiling.

## Cross-references

- Design doc: `docs/superpowers/specs/2026-05-05-stress-test-large-nets-design.md` §4.2 row 1, §4.4 (campaign descriptor in TOML-ish notation).
- Consumes: TASK-0700, TASK-0701.
- Consumed by: TASK-0703 (CSV columns), TASK-0704 (script), TASK-0708 (campaign run).
- Existing peer descriptors to match style: search `bench/suite.rs` for `Campaign`, `Suite`, or `Profile` patterns from D-005/D-010 era.
