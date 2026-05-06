# TASK-0704 — D-014-SCRIPT: `scripts/stress_curve.sh` Phase 1 + Phase 2 orchestrator

**Phase:** D-014 (Stress Curve Campaign) — Stage 3 DEV scope
**Bundle:** D-014 — Stress Curve Campaign
**Status:** TODO
**Priority:** P0 (the campaign cannot run without it).
**Spec:** none.
**Depends on:** TASK-0700 (`MemoryProbe`), TASK-0701 (`StopRule`), TASK-0702 (descriptor + CLI), TASK-0703 (CSV columns).
**Estimated complexity:** M (~150 LoC bash + ~80 LoC integration test).

---

## Context

The stress-curve campaign is a 2-phase shell-orchestrated job:

- **Phase 1 (in-process):** 12 sequences = 3 workloads × 4 worker counts. Each rep runs in a **child process** (`Command::spawn` from bash, not from Rust) so that `VmHWM` resets between reps. Each child invokes `relativist-bench --campaign stress-curve --workload X --env in_process --workers W --reps 1 --n-target N` and emits one CSV row.

- **Phase 2 (Docker):** mirror matrix using `docker compose run --rm bench-tcp` (profile already shipped per design doc §4.1). Pre-step generates input file once per `(workload, N)` and reuses it across reps.

The script orchestrates pre-conditions, the rep loop with `StopRule` guard, the per-phase CSV concatenation, and the lock-and-manifest produce step (without committing — that's the user's call).

This task ships the script + a smoke integration test that runs `--smoke` mode (1 workload × 1 W × 2 N × 1 rep, ~15 min wall budget) end-to-end and asserts the output structure.

## Files in scope (file:line pointers)

| File | Change |
|------|--------|
| `scripts/stress_curve.sh` | **CREATE.** Bash orchestrator. ~150 LoC, executable bit set. |
| `relativist-core/tests/d014_stress_curve_smoke.rs` | **CREATE.** Integration test that invokes `scripts/stress_curve.sh --smoke --no-docker --output-dir <tmp>` from a `Command::new("bash")` and verifies CSV exists, has expected row count, and `MANIFEST.md` is well-formed. ~80 LoC. Skips on Windows-without-WSL with `#[cfg(unix)]`. |
| `scripts/stress_curve.sh.example.env` (optional) | **CREATE if helpful.** Template env-var file (RAM threshold overrides etc.). ~10 LoC. |

## Files explicitly OUT of scope

- The plot generator — TASK-0705 (script invokes it as `python3 scripts/plot_stress_curve.py` but doesn't reimplement plotting logic).
- The docs page — TASK-0706.
- The actual lock manifest of a real campaign — TASK-0708 (the script's `--smoke` produces a temp dir; the lock dir is real-campaign-only).
- Cross-platform PowerShell port — explicit non-goal; design doc §5 documents Linux + Docker as the target. Windows hosts run via WSL.
- Frozen `results/locked/` paths — read-only.

## Required script CLI

```text
scripts/stress_curve.sh [OPTIONS]

OPTIONS:
  --smoke                Smoke mode: 1 workload (ep_annihilation), W=2,
                         N=[1000, 10000], 1 rep, 15-min total budget.
  --no-docker            Skip Phase 2 (in-process only).
  --resume               Resume from a partial run; reads existing CSV +
                         skips already-completed (workload, env, W, N, rep).
  --output-dir DIR       Override output directory.
                         Default: results/locked/v2_stress_curve_$(date -I)/
  --workloads LIST       Comma-separated subset of {ep_annihilation,
                         dual_tree, condup_expansion}. Default: all 3.
  --workers LIST         Comma-separated subset of {1,2,4,8}. Default: all 4.
  -h | --help            Print this help.

EXIT CODES:
  0   Campaign completed (or smoke ran cleanly)
  1   Pre-condition failed (dirty tree, tests red, no Docker, low RAM, etc.)
  2   Mid-run abort: every (workload, W) hit StopRule before N=10_000
  3   Phase 2 setup failure (Docker compose unavailable, image build failed)
  10  User interrupt (SIGINT/SIGTERM); partial output preserved for --resume
```

## Pre-condition gate (script aborts with exit 1 if any fails)

1. `git status --porcelain` is empty (clean tree).
2. `cargo test --release` exit 0, ≥ release floor (read floor from this task's "Test floor delta" plus prior tasks; queried from the script via parsing `cargo test`'s last `test result` line).
3. `cargo test` (debug, default) exit 0 ≥ default floor.
4. `cargo test --features zero-copy` exit 0 ≥ zero-copy floor.
5. `cargo test --features streaming-no-recycle` exit 0 ≥ streaming-no-recycle floor.
6. `cargo clippy --all-features -- -D warnings` exit 0.
7. `cargo fmt --check` exit 0.
8. `docker compose version` exit 0 (skipped if `--no-docker`).
9. RAM total ≥ 8 GiB (warning if < 16 GiB but not abort).
10. `df --output=avail` (POSIX) ≥ 10 GiB on the output volume.
11. CPU governor not in `powersave` (Linux); print warning if `cpufreq-info` shows powersave.
12. Output dir does not exist (or `--resume` is set AND it does exist).

## Phase 1 loop (in-process)

For each `(workload, W) in 3 × 4`:
1. For each `N` in `[10000, 31623, ..., 1_000_000_000]`:
   1. For each rep in `1..=5`:
      1. `relativist-bench --campaign stress-curve --workload $WL --env in_process --workers $W --reps 1 --n-target $N --append-csv $OUTPUT_DIR/raw/in_process.csv` (each rep is its own child process — VmHWM resets).
      2. Capture wall time, exit code (`0`, `137`/SIGKILL, etc.), peak/current memory from the row just written.
      3. Construct `RepResult` and call `StopRule::check` (via a Rust helper binary `relativist-bench --check-stop` OR via inlined bash arithmetic — choice left to implementer; prefer Rust helper for correctness with `Oom` detection).
   2. If StopRule fires: append sentinel row to CSV with `stop_reason = $reason`, break the N loop.

## Phase 2 loop (Docker)

Same structure with these deltas:
- Pre-step: `docker compose run --rm gen --workload $WL --n $N --output data/input_${WL}_${N}.bin` (generated once per `(WL, N)`, reused across 5 reps).
- Each rep invocation: `docker compose run --rm bench-tcp --campaign stress-curve --workload $WL --env docker_tcp --workers $W --reps 1 --input data/input_${WL}_${N}.bin --append-csv $OUTPUT_DIR/raw/docker_tcp.csv`.
- StopRule wall budget: 7m30s (450s).
- Memory measurement: sum of `docker stats --no-stream --format '{{.MemUsage}}'` for all compose services at end-of-rep, parsed into bytes, fed into the same `MemoryProbe`-style gate.

## Phase 3 aggregation (script-driven, no Rust delta)

1. Concat raw CSVs → `aggregated.csv`.
2. Invoke `python3 scripts/plot_stress_curve.py --input $OUTPUT_DIR/aggregated.csv --output-dir $OUTPUT_DIR/figures/`.
3. Capture environment: `uname -a`, `rustc -V`, `cargo -V`, `cat /proc/meminfo | head -5`, `cat /proc/cpuinfo | grep 'model name' | head -1`, `git rev-parse HEAD` → `$OUTPUT_DIR/raw/env.txt`.
4. SHA-256 every file in `$OUTPUT_DIR/raw/` and `$OUTPUT_DIR/figures/` → `$OUTPUT_DIR/checksums.sha256`.
5. Synthesize `$OUTPUT_DIR/MANIFEST.md` from a template (see TASK-0706 for the `docs/benchmarks/campaigns/stress-curve.md` template — same skeleton).
6. **DO NOT** `git add` or `git commit` — exit cleanly with a printed reminder that user must lock the dir manually.

## Acceptance criteria

1. `scripts/stress_curve.sh --smoke --no-docker --output-dir /tmp/stress_smoke_$$` exits 0 within 20 minutes on a workstation with ≥ 8 GiB RAM.
2. After smoke, `/tmp/stress_smoke_$$` contains: `MANIFEST.md`, `raw/in_process.csv` (≥ 2 rows), `aggregated.csv`, `figures/*.pdf` (≥ 1 PDF), `checksums.sha256` (non-empty).
3. `--resume` on a smoke run that's been interrupted picks up at the next missing `(workload, env, W, N, rep)` tuple and produces an identical final dataset (modulo wall-time noise).
4. Pre-condition gate fires correctly: simulate dirty tree (`touch garbage_file.txt`), invoke script, expect exit 1 + diagnostic.
5. New integration test `d014_stress_curve_smoke` passes on Linux; is correctly skipped (or x-fail-marked) on Windows-without-WSL.
6. Script passes `shellcheck --severity=warning scripts/stress_curve.sh` (no warnings — bash hygiene).
7. `cargo test` floor: **+1 default = ≥ 1812** (cumulative TASK-0700..0704; +1 if smoke test counts as a single integration test target).
8. `cargo test --features zero-copy` floor: **+1 = ≥ 1856**.
9. `cargo test --features streaming-no-recycle` floor: **+1 = ≥ 1803**.
10. `cargo test --release` floor: **+1 = ≥ 1754**.
11. v1 floor (690) inviolable.

## Test floor delta

**+1 default** (smoke integration test on `unix` only). Cumulative after TASK-0700..0704:
- default ≥ 1812
- zero-copy ≥ 1856
- streaming-no-recycle ≥ 1803
- release ≥ 1754

## Implementation hints

1. Use `set -euo pipefail` at the top — fail fast.
2. `bash` 4+ associative arrays make the matrix loops cleaner; the project's CI image has bash 5.x. Document the requirement.
3. Capture each child's stderr separately (`2> $OUTPUT_DIR/raw/${WL}_${W}_${N}_${REP}.stderr`) — overnight failures need forensic logs.
4. Use `trap 'on_interrupt' INT TERM` to handle Ctrl-C cleanly, dumping a partial-state file that `--resume` reads.
5. The `--resume` logic: read `$OUTPUT_DIR/raw/in_process.csv`, build a set of completed `(workload, env, W, N, rep)` tuples, skip them in the loop. If the CSV is malformed (truncated mid-row), `--resume` MUST detect and refuse with exit 1 + a clear error message.
6. Reuse `bench_docker_v2.sh` as a template for Docker compose plumbing. Its profile-bench-tcp invocation pattern is the canonical reference.
7. The `MANIFEST.md` synthesis can be `bash` heredoc + variable substitution — no need for a templating engine. Include the SHA-256 of every file plus the campaign git SHA.

## Estimated LoC

- Script: ~150 LoC bash.
- Test: ~80 LoC Rust integration.
- Total: ~230 LoC. **Slightly over 200.** Acceptable here because bash LoC is low-density (long arg lists, comments) and the test is structurally separate. If during DEV the bash exceeds 180 LoC of code (excl. comments), split into `stress_curve.sh` (top-level orchestrator) + `stress_curve_phase1.sh` + `stress_curve_phase2.sh` (subroutines `source`d).

## Cross-references

- Design doc: `docs/superpowers/specs/2026-05-05-stress-test-large-nets-design.md` §4.2 row 5, §5 Phase 1 + Phase 2 + Phase 3.
- Reference template: `scripts/bench_docker_v2.sh` (Docker compose orchestration pattern).
- Consumes: TASK-0700, TASK-0701, TASK-0702, TASK-0703.
- Consumed by: TASK-0708 (real-campaign run uses the same script with no `--smoke` flag).
