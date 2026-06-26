---
title: Campaign — Stress Curve (D-014)
summary: Methodology for the D-014 stress-curve campaign sweeping N from 10⁴ to 10⁹ agents to characterise the scaling wall and its failure mode.
keywords: [stress curve, d-014, scaling wall, memory probe, stop rule, N up to 10^9, in_process, docker_tcp, ep_annihilation, dual_tree, condup_expansion, oom, ram gate]
modules: [bench]
specs: [SPEC-19, SPEC-21, SPEC-22]
audience: [contributor, researcher]
status: reference
updated: 2026-06-26
---

# Stress Curve Campaign — v2-development

Methodology page for the D-014 stress-curve campaign. Reviewers consult
this to understand WHY the campaign exists, HOW to reproduce it from a
clean checkout, and HOW to audit the locked output.

## 1. Research question

The TCC's central claim (ARG-001) holds for terminating nets in a
deterministic environment, but the v1 + v2 baseline campaigns
(`v1_local_baseline`, `v2_post_d012_baseline_2026-05-05`) only exercise
sizes up to N=5,000,000 agents. The v2 architecture (SPEC-21 streaming,
SPEC-22 arena, SPEC-19 delta) was *designed* to scale; nothing in the
record proves it does. The stress-curve campaign closes that gap by
sweeping N from 10⁴ to 10⁹ agents on three workloads, two
environments, four worker counts, and characterising the wall (where
the system stops scaling) plus the failure mode at the wall (wall-time,
RAM exhaustion, or OS OOM-killer).

The campaign **characterises the wall — it does not remove it**.
Streaming-reduction follow-up (ROADMAP §2.16) is the work that would
push the wall further; this campaign produces the empirical evidence
that work is needed.

## 2. Scope

| Axis | Values |
|------|--------|
| Workload | `ep_annihilation`, `dual_tree`, `condup_expansion` |
| Env | `in_process`, `docker_tcp` |
| Workers | 1, 2, 4, 8 |
| N (agents) | 11 points × √10 from 10⁴ to 10⁹ |
| Reps | 5 per `(workload, env, W, N)` |

Total upper bound: 3 × 2 × 4 × 11 × 5 = **1320 datapoints**.
Actual count is bounded below by the StopRule (TASK-0701): each
`(workload, env, W)` sequence aborts at the first N that breaches the
wall budget, RAM gate, or OOM signature. Wall-clock estimate per
campaign: **7-8 hours overnight** on a workstation with ≥ 16 GiB RAM.

## 3. Components

| # | Component | Path | Owner TASK |
|---|-----------|------|-----------|
| 1 | Memory probe | `relativist-core/src/bench/memory_probe.rs` | TASK-0700 |
| 2 | Stop rule | `relativist-core/src/bench/stop_rule.rs` | TASK-0701 |
| 3 | Campaign descriptor | `relativist-core/src/bench/suite.rs` (`StressCurveDescriptor`) | TASK-0702 |
| 4 | CSV schema (+4 cols) | `relativist-core/src/bench/csv.rs` + `bench/mod.rs` | TASK-0703 |
| 5 | Bash orchestrator | `reproduce_article/scripts/stress_curve.sh` | TASK-0704 |
| 6 | Plot generator | `reproduce_article/scripts/plot_stress_curve.py` | TASK-0705 |
| 7 | Methodology docs (this page) | `docs/benchmarks/campaigns/stress-curve.md` | TASK-0706 |
| 8 | Integration tests | `relativist-core/tests/d014_*.rs` | TASK-0707 |
| 9 | Campaign run + lock | `reproduce_article/results/locked/v2_stress_curve_<DATE>/` | TASK-0708 |

## 4. CSV schema

The campaign extends the existing 29-column `detail.csv` schema
(SPEC-09 R39a) with 4 stress-curve columns appended at the end. The
existing pre-D-014 readers (`v1_local_baseline`, `v1_stress`,
`v2_post_d012_baseline_2026-05-05`) ignore trailing columns by csv
crate convention — this is forward-compatible by design.

| # | Column | Type | Source |
|---|--------|------|--------|
| 30 | `vmrss_peak_mb` | f64 (MiB) | `MemoryProbe::peak_bytes() / (1024*1024)` at end-of-rep |
| 31 | `vmrss_current_end_mb` | f64 (MiB) | `MemoryProbe::current_bytes() / (1024*1024)` at end-of-rep |
| 32 | `stop_reason` | string | `StopReason` variant name (`""` for normal rep) |
| 33 | `cv_above_gate` | bool | `(stddev/mean) > 0.05` flag from the post-rep aggregator |

`stop_reason` is the empty string for normal rep rows; non-empty values
(`WallTimeExceeded`, `MemoryExceeded`, `Oom`) appear on sentinel rows
emitted by the orchestrator after a `StopRule::check` fires.

The pre-existing SPEC-09 R18a column `peak_memory_during_construction`
is **preserved** alongside the new `vmrss_peak_mb` — the two are
sampled at different program points (construction-complete vs
end-of-rep) and answer different questions.

## 5. How to reproduce

### 5.1 Pre-conditions

The orchestrator (`reproduce_article/scripts/stress_curve.sh`) gates on the following
before kicking off the full campaign:

1. `git status --porcelain` empty (clean tree).
2. `cargo test --release` exits 0, ≥ release floor.
3. `cargo test` (debug, default) exits 0, ≥ default floor.
4. `cargo test --features zero-copy` exits 0.
5. `cargo test --features streaming-no-recycle` exits 0.
6. `cargo clippy --all-features -- -D warnings` exits 0.
7. `cargo fmt --check` exits 0.
8. `docker compose version` exits 0 (skipped under `--no-docker`).
9. RAM total ≥ 8 GiB; warning if < 16 GiB.
10. `df --output=avail` ≥ 10 GiB on the output volume.
11. CPU governor not in `powersave` (Linux); warning otherwise.
12. Output directory does not exist (or `--resume` is set AND it does).

`--smoke` mode bypasses the gate for fast iteration.

### 5.2 Smoke run

```bash
reproduce_article/scripts/stress_curve.sh --smoke
```

Smoke runs `ep_annihilation`, W=2, N=[1000, 10000], 1 rep, 15-minute
total budget. Produces an output tree at
`reproduce_article/results/locked/v2_stress_curve_<DATE>/` (or `--output-dir` override)
with `MANIFEST.md`, `raw/in_process.csv`, `aggregated.csv`,
`figures/*.pdf`, and `checksums.sha256`. Smoke finishing in < 20
minutes on an 8-GiB-RAM host validates the orchestrator end-to-end
before committing to the overnight run.

### 5.3 Full overnight run

```bash
reproduce_article/scripts/stress_curve.sh
```

No flags: full 3×2×4 matrix with the canonical 11-point N sweep, 5
reps per cell. Expected wall ~7-8 hours.

`MANIFEST.md` fields locked at run time:
- git rev SHA at the campaign commit
- `rustc -V` and `cargo -V` outputs
- `/proc/meminfo` snapshot (Linux) / `systeminfo` (Windows + WSL)
- `/proc/cpuinfo` `model name` line (Linux)
- Full bash invocation (canonical `reproduce_article/scripts/stress_curve.sh`)
- Total reps executed
- Total wall-clock time (HH:MM:SS)
- Median CV across all rows
- Histogram of `stop_reason` values
- Free-form "Known anomalies" section (operator fills if any
  `cv_above_gate=true` rows surface at unexpected N)

## 6. Lock procedure

1. After the campaign exits cleanly, audit the output tree:

   ```bash
   ls reproduce_article/results/locked/v2_stress_curve_<DATE>/figures/
   cat reproduce_article/results/locked/v2_stress_curve_<DATE>/MANIFEST.md
   sha256sum -c reproduce_article/results/locked/v2_stress_curve_<DATE>/checksums.sha256
   ```

2. Apply the §8 sanity checks (below). If any fails, **STOP** — do
   not lock; file as a QA blocker (TASK-0707 family extension).

3. The orchestrator does **NOT** `git add` or `git commit`. The
   operator inspects, possibly edits the "Known anomalies" section
   of `MANIFEST.md`, then commits the directory as a separate commit.
   See design doc §10 commit (g).

4. The merge to `main` is approved by the operator after the figures
   are eyeballed for IEEE quality and the dataset matches expectations.

## 7. Failure modes

- **RAM exhausted (`StopReason::MemoryExceeded`):** the rep's
  `vmrss_peak_fraction_of_total` exceeded 0.80. The sequence aborts;
  a sentinel row with `stop_reason = "MemoryExceeded"` is emitted.
  Subsequent N values for the same `(workload, env, W)` are NOT run.
- **OS OOM-killer (`StopReason::Oom`):** the rep child process exited
  with `SIGKILL` (Linux), `137` (bash-mediated 128 + SIGKILL), or
  `0xC0000017` STATUS_NO_MEMORY (Windows). The campaign documents
  the exit code list as `OOM_EXIT_CODES` in
  `relativist-core/src/bench/stop_rule.rs`.
- **Wall-clock budget (`StopReason::WallTimeExceeded`):** the rep
  exceeded its per-env wall budget (5 min in-process, 7m30s docker).
  Indicates either the workload reached a fundamental scaling wall
  or background load corrupted the timing.
- **Smoke fails — do not run overnight.** If
  `reproduce_article/scripts/stress_curve.sh --smoke` exits non-zero, treat it as a
  pre-condition failure: investigate, fix, re-smoke before committing
  the overnight slot.
- **`--resume` semantics:** an interrupted run can resume via
  `reproduce_article/scripts/stress_curve.sh --resume --output-dir <existing>`. The
  script reads the existing `raw/in_process.csv`, builds a set of
  completed `(workload, env, W, N, rep)` tuples, and skips them.
  **A truncated mid-row CSV is detected and the script refuses with
  exit 1** — the operator must `tail` the malformed CSV, manually
  remove the partial row, and re-invoke `--resume`.

## 8. Sanity checks (post-aggregation)

Apply each check against `aggregated.csv` after the run. Failure on
any of these blocks the lock until investigated.

1. `mips ep_annihilation in_process W=1` at large N: plateau between
   10 and 30 MIPS.
2. `wall_time ep_annihilation` log-log slope ≈ 1 (linear in N for
   embarrassingly parallel workloads).
3. `vmrss_peak_mb dual_tree` log-log slope ≈ 1 with W fixed; ≈ -1 with
   N fixed and W varying.
4. `speedup` for `ep_annihilation W=4/W=1` ≈ 2.5-3.5×.
5. `network_time / wall_time` for `docker_tcp` ≈ 30-60% per ARG-004.
6. `all_correct = true` on 100% of non-sentinel rows. Any false
   blocks the lock — the campaign cannot ship a correctness regression.

## 9. Limitations

The campaign characterises the wall — it does not remove it. The
streaming-reduction follow-up (ROADMAP §2.16) is the engineering work
that would push the wall further; this campaign produces the empirical
input that work is needed.

1. The 10⁹ ceiling is aspirational; the StopRule sets the real ceiling
   per `(workload, env, W)`. Sequences with `dual_tree` at large N
   typically abort before 10⁹ on workstation hardware.
2. The `docker_tcp` env has higher network overhead than the
   `in_process` baseline; comparisons across envs are speedup
   ratios, not raw wall-time differences.
3. CV (coefficient of variation, `stddev/mean`) above 0.05 marks
   statistical instability that the operator must inspect before
   publishing a derived figure. CV is computed by the bash orchestrator
   across the 5 reps per `(workload, W, N)` tuple — the per-rep CSV
   row schema does NOT carry a per-row CV flag any more (TASK-0720
   BUG-006: `cv_above_gate` was always emitted as `false` because the
   in-process runner sees `reps=1` per child invocation; the column
   was dead and is now removed from the writer + plotter schemas).
4. The campaign reuses the existing `BenchmarkSuite` runner per rep;
   per-rep cold-cache effects are NOT controlled (each child process
   inherits the page cache state of the parent shell).
5. macOS hosts are explicitly out of scope (`MemoryProbe` returns
   `BenchError::MemoryProbe("macos unsupported")` per TASK-0700).
6. Phase 2 Docker arm requires `docker compose` and the
   `bench-tcp` profile from `docker-compose.yml`. Hosts without
   Docker run with `--no-docker` (Phase 1 only).
7. The empirical `c_o/c_r` ratio observed in the v1 campaigns (~2.2)
   sets a lower bound on observed parallel-speedup; this campaign
   does not change that ratio, only characterises where it stops
   producing useful data.
8. **In-process Rust path memory probe contamination** (TASK-0720
   BUG-003, Fix-B): a Rust caller invoking
   `StressCurveDescriptor::run_one_sequence(_, _, _, reps=N>1, _, _)`
   sees `vmrss_peak_bytes` (= `MemoryProbe::peak_bytes() = VmHWM`)
   monotonically inherited across reps. Rep 1's high-water mark is
   visible to reps 2..N because the probe is constructed once per
   sequence and `VmHWM` is monotonic non-decreasing within a process
   on Linux. The bash orchestrator (`reproduce_article/scripts/stress_curve.sh`) bypasses
   this by fork-execing a fresh child per rep — the canonical
   campaign path is the bash orchestrator, NOT the in-process Rust
   API. The dispatch code emits a `tracing::warn!` when called with
   `reps > 1` so a Rust user sees the limitation loudly. A future
   task may add `prctl(PR_SET_MM, PR_SET_MM_HWM_RESET, ...)` (Linux)
   or `EmptyWorkingSet` (Windows) to reset the watermark in-process;
   v1 of the campaign does not.
9. **SIGINT semantics in the bash orchestrator** (TASK-0720 BUG-004):
   the orchestrator installs a trap on `INT TERM` that forwards the
   signal to the in-flight rep child (tracked via `$REP_PID`),
   waits up to 10 s for graceful shutdown, then escalates to
   `SIGKILL` if needed. Exit code on user-initiated abort is **130**
   (POSIX 128 + SIGINT convention; previously 10). A PID-bearing
   lockfile at `$OUTPUT_DIR/.lock` prevents concurrent invocations:
   `--resume` against a directory whose lock is held by a live PID
   refuses to start; a stale lock (process not alive) is claimed
   with a warning. `kill -9` of the orchestrator itself bypasses the
   trap entirely and may leave a stale lock + a dangling rep child;
   the operator must clean both up by hand.

## 10. Cross-references

- Design doc: `docs/superpowers/specs/2026-05-05-stress-test-large-nets-design.md`
- Locked output (post-run): `reproduce_article/results/locked/v2_stress_curve_<DATE>/`
- ROADMAP item: §2.16 streaming reduction — this campaign characterises
  the wall it leaves; streaming-reduction is the work that pushes the
  wall further (deferred per the (α) decision in the design doc).
- Predecessor campaigns:
  `docs/benchmarks/campaigns/v1-local-baseline.md`,
  `docs/benchmarks/campaigns/v1-stress.md`.
- Argument trail: ARG-001 (central correctness claim), ARG-004
  (viability + break-even), ARG-007 (formal invariants vs empirical
  testing).
- Closure: TASK-0708 runs the campaign, locks the dir, and updates
  `docs/README.md`, `docs/roadmap.md`, `docs/next-steps.md`,
  `CHANGELOG.md`, `docs/backlog/BACKLOG.md`.
