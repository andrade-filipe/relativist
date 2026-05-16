# TASK-0720 — D-014 Follow-up: Bug Fixes (Stage 6 REFACTOR scope)

**Phase:** D-014 (Stress Curve Campaign) — Stage 6 REFACTOR scope
**Bundle:** D-014 — Stress Curve Campaign
**Status:** TODO
**Priority:** P0 (BLOCKS TASK-0708 overnight dispatch and bundle merge)
**Spec:** none.
**Depends on:** TASK-0700..0708 (must be all in HEAD; this task fixes bugs found in QA Stage 5).
**Estimated complexity:** M (~150 LoC production + ~100 LoC test).

---

## Context

Stage 4 (REVIEW) returned AMARELO with MF-001 (println!-as-CSV gap) flagged.
Stage 5 (QA) escalated to **NOT SAFE-TO-MERGE** with 4 CRITICAL + 5 HIGH bugs.
Bug report: `docs/qa/D-014-stress-curve-qa.md`.

Without this task landing, TASK-0708's overnight run produces 7-8h of
unplottable garbage data. This is therefore a hard prerequisite for
the campaign run AND for merging the D-014 bundle into main.

## Scope — bugs to fix

### CRITICAL (4 bugs, all MUST fix)

**BUG-001 — `--campaign stress-curve` emits `println!` instead of CSV row**
- **Location:** `relativist-core/src/commands.rs:332-338`
- **Symptom:** `println!("stress-curve outcome: ...")` summary line is printed; bash redirects stdout into `raw/in_process.csv`. Overnight produces ~1320 lines of debug text, zero CSV rows.
- **Root cause:** dispatch path is sentinel-only; no actual CSV row write happens.
- **Fix:**
  - Replace `println!` with `tracing::info!` (CLAUDE.md rule: no `println!` in production)
  - Wire dispatch into `bench/csv.rs::write_csv_detail` (or equivalent) so each rep produces a real `BenchmarkResult` row
  - Ensure `vmrss_peak_mb`, `vmrss_current_end_mb`, `stop_reason`, `cv_above_gate` are populated end-to-end

**BUG-002 — CSV schema name divergence (writer vs plotter)**
- **Locations:**
  - `relativist-core/src/bench/csv.rs:90-95` (writer header)
  - `scripts/plot_stress_curve.py:34-46` (REQUIRED_COLUMNS)
- **Symptom:** 6 of 11 columns name-mismatch. Writer emits `benchmark, input_size, mode, workers, repetition, wall_clock_secs, ...`; plotter expects `workload, env, workers, n, rep, wall_seconds, ...`.
- **Fix (decide ONE of the two paths):**
  - **Path A:** Update `plot_stress_curve.py` REQUIRED_COLUMNS to match writer schema (writer is canonical; plotter conforms).
  - **Path B:** Add column rename in writer for `--campaign stress-curve` mode (writer supports both legacy schema AND stress-curve schema).
  - Recommendation: **Path A** (less surface area; legacy CSV consumers unaffected).

**BUG-003 — Rust API VmHWM contamination across reps**
- **Location:** `relativist-core/src/bench/suite.rs::StressCurveDescriptor::run_one_sequence`
- **Symptom:** `MemoryProbe` constructed once; `peak_bytes()` is monotonic per-process. Reps 2..N inherit rep 1's high-water-mark.
- **Fix (decide ONE):**
  - **Fix-A (preferred):** Reconstruct `MemoryProbe` per-rep AND reset `VmHWM` between reps via Linux `prctl(PR_SET_MM, PR_SET_MM_HWM_RESET, ...)` or `/proc/self/clear_refs`. Windows: `EmptyWorkingSet` from `psapi.dll`. Both behind cfg-gated.
  - **Fix-B (escape hatch):** Document the limitation explicitly in `docs/benchmarks/campaigns/stress-curve.md` AND add a runtime warning when `run_one_sequence` is called with `reps > 1`. Bash bypasses via per-rep child-fork; in-process Rust path keeps the limitation but is loud about it.
  - Recommendation: **Fix-B** is faster (no platform-specific code); **Fix-A** is correct but adds ~80 LoC plus testing.

**BUG-004 — Bash SIGINT does not kill in-flight child**
- **Location:** `scripts/stress_curve.sh` trap handlers
- **Symptom:** Ctrl+C kills parent shell but child rep keeps running; orphan can corrupt CSV mid-write; no lockfile to prevent concurrent `--resume`.
- **Fix:**
  - Add `trap 'kill -SIGTERM $REP_PID 2>/dev/null; wait $REP_PID; exit 130' INT TERM`
  - Add lockfile at `$RESULTS_DIR/.lock` (PID-bearing); `--resume` checks for stale lock; abort if lock is held by live PID
  - Document that `kill -9` of bash leaves an unrecoverable mess (we honor SIGINT/SIGTERM only)

### HIGH (2 of the 5 are bundled here; rest deferred to TASK-0721 if needed)

**BUG-005 — Plot script PDF placeholder leaks into real campaign**
- **Location:** `scripts/stress_curve.sh` (placeholder fallback) + `scripts/plot_stress_curve.py` (full mode)
- **Symptom:** When matplotlib is missing, bash writes `smoke_placeholder.pdf` so smoke test passes. Pre-condition gate `--smoke` mode bypasses the matplotlib check; in `--smoke`-bypassed full mode, missing matplotlib still reaches the orchestrator (no early abort).
- **Fix:** In full (non-smoke) mode, abort with exit 1 BEFORE running any reps if `python3 + matplotlib + pandas + numpy` are missing. The placeholder fallback is `--smoke`-only.

**BUG-006 — `cv_above_gate` is a dead column in the writer**
- **Location:** `relativist-core/src/bench/csv.rs` and `relativist-core/src/bench/mod.rs::BenchmarkResult`
- **Symptom:** Column always emitted as `false` (or `null`) because the bench harness doesn't actually compute CV across reps for stress-curve sequences (each child gets reps=1).
- **Fix:** Either (a) populate `cv_above_gate` from the bash orchestrator's per-(workload, W, N) CV computation across the 5 child results, OR (b) drop the column from the schema with a corresponding update to the plotter.
- Recommendation: **(b)** — schema is leaner and the orchestrator owns CV anyway.

## Files in scope

| File | Change |
|------|--------|
| `relativist-core/src/commands.rs:332-338` | Replace `println!` with `tracing::info!`; wire to CSV writer for each rep |
| `relativist-core/src/bench/suite.rs` | Per-rep `MemoryProbe` (Fix-B docs caveat OR Fix-A reset) |
| `relativist-core/src/bench/csv.rs:90-95` | Possibly drop `cv_above_gate` column |
| `relativist-core/src/bench/mod.rs::BenchmarkResult` | Possibly drop `cv_above_gate` field |
| `scripts/plot_stress_curve.py:34-46` | Update REQUIRED_COLUMNS to match writer (Path A) |
| `scripts/stress_curve.sh` | SIGINT/SIGTERM trap; lockfile; abort early if matplotlib stack missing |
| `docs/benchmarks/campaigns/stress-curve.md` | Document VmHWM limitation (Fix-B) and SIGINT semantics |
| `relativist-core/tests/d014_writer_to_plot_roundtrip.rs` | **CREATE.** Integration test covering TG-001: spawn `cargo run --release -- bench --campaign stress-curve` → capture stdout to CSV → invoke `plot_stress_curve.py` → assert exit 0 + ≥ 1 PDF. ~80 LoC. |

## Files explicitly OUT of scope

- HIGH/MEDIUM/LOW bugs that don't block merge (BUG-007..BUG-020): deferred to TASK-0721 if reopened, or filed individually.
- Edge cases EC-001..006 and stress scenarios SS-001..006: deferred.
- Test-coverage gaps TG-002..008: deferred (TG-001 is the critical one).
- Schema redesign for `cv_above_gate` (if Fix-A path chosen for that bug): would be a new TASK.

## Acceptance criteria

1. **AC-1:** `cargo run --release -- bench --campaign stress-curve --workload ep_annihilation --env in-process --workers 2 --n-seq 1000 --output /tmp/test.csv` produces a valid CSV with header matching the plot script REQUIRED_COLUMNS schema. (BUG-001 + BUG-002)
2. **AC-2:** `python3 scripts/plot_stress_curve.py /tmp/test.csv --out /tmp/figs/` exits 0 and produces ≥ 1 PDF. (BUG-002)
3. **AC-3:** `StressCurveDescriptor::run_one_sequence(_, _, _, reps=3, _, _)` either resets VmHWM between reps OR emits a `tracing::warn!` saying "VmHWM not reset; rep 1 watermark inherited". (BUG-003)
4. **AC-4:** `scripts/stress_curve.sh` interrupted via SIGINT mid-rep cleans up child PID and exits with 130. Re-running with `--resume` either continues correctly OR detects stale lock and refuses with exit 1. (BUG-004)
5. **AC-5:** `scripts/stress_curve.sh` (full mode, non-`--smoke`) without matplotlib aborts BEFORE running any reps. (BUG-005)
6. **AC-6:** `cv_above_gate` column either populated correctly OR removed from schema (writer + plotter + docs). (BUG-006)
7. **AC-7:** New IT `tests/d014_writer_to_plot_roundtrip.rs` passes. (TG-001)
8. **AC-8:** All existing pisos hold: default ≥ 1820 (1819 + 1 new IT) / zero-copy ≥ 1864 / streaming-no-recycle ≥ 1811 / release ≥ 1762 (Linux); -3 on Windows for cfg(unix) gating.
9. **AC-9:** `cargo clippy --all-features -- -D warnings` clean.
10. **AC-10:** `cargo fmt --check` clean.
11. **AC-11:** Bug report `docs/qa/D-014-stress-curve-qa.md` updated with `Status: FIXED` annotations on BUG-001..006.

## Test expectations

Reuse the existing TEST-SPEC schemas where applicable:
- TEST-SPEC-0700, 0701 (memory probe, stop rule) — should remain green
- TEST-SPEC-0702 (campaign descriptor) — should remain green
- TEST-SPEC-0703 (CSV schema) — UPDATE if schema changes (Path A: drop `cv_above_gate` field assertions)
- TEST-SPEC-0704 (bash orchestrator) — UPDATE if SIGINT or lockfile semantics change
- TEST-SPEC-0705 (plot generator) — UPDATE if REQUIRED_COLUMNS changes
- New TG-001: write a `tests/d014_writer_to_plot_roundtrip.rs` covering AC-1+AC-2 end-to-end

## Sequencing note

This task lands as Stage 6 REFACTOR of the D-014 bundle. After it
closes:
1. TASK-0708 overnight can be dispatched (operator-driven; ~7-8h)
2. After campaign produces locked dir, the bundle is mergeable to `main`

**This task is BLOCKED on D-015 developer (Topic 2) completing,** because dispatching a fresh `developer` agent on the same `relativist-core` crate while D-015 is mid-implementation creates concurrent `src/` writes. Wait until D-015 commits land, then dispatch.
