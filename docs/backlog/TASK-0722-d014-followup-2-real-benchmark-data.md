# TASK-0722 — D-014 Follow-up #2: Real Benchmark Data in Stress-Curve Dispatch

**Phase:** D-014 (Stress Curve Campaign) — Stage 6 REFACTOR continuation
**Bundle:** D-014 — Stress Curve Campaign
**Status:** TODO
**Priority:** P0 (blocks operator's overnight campaign run; first real smoke surfaced this)
**Spec:** none (bug fix)
**Depends on:** TASK-0720 (which closed but missed these two bugs)
**Estimated complexity:** S (~60 LoC production + ~30 LoC test)

---

## Context

Operator (Filipe) ran `scripts/stress_curve.sh --smoke` for the first time after TASK-0720 closure on 2026-05-06 and surfaced **two bugs the QA Stage 5 + Stage 6 REFACTOR did not catch:**

### BUG-A — Orphan `println!` in `bench/suite.rs:948`

```rust
println!("  {} — {}", bench_id, bench.describe(size));
```

This line is pre-existing (likely D-009 era) and was missed by the D-014 QA Stage 5 because the agent focused on new code introduced by TASK-0700..0708. When the `--campaign stress-curve` dispatch invokes `run_benchmark_suite`, this `println!` writes the workload banner to stdout. The bash orchestrator (`scripts/stress_curve.sh`) captures stdout into the per-rep CSV file. Result: every CSV file has a banner line on line 1 that the plot script reads as the header, then reports `required column 'benchmark' missing`.

**Symptom (verbatim from operator):**

```
=== Phase 3: aggregation ===
ERROR: required column 'benchmark' missing from
  C:\...\results\locked\v2_stress_curve_2026-05-06\aggregated.csv
WARN: plot script exited non-zero
```

**Symptom in the CSV file:**

```
  ep_annihilation — 1000 ERA-ERA pairs (void annihilation)        <- banner
benchmark,input_size,mode,workers,...,stop_reason                 <- real header
ep_annihilation,1000,local,2,0,true,...                           <- data row
```

**Fix:** Replace the `println!` with `tracing::info!` (CLAUDE.md "no `println!` in production" rule). One-line change.

### BUG-B — Dispatch synthesizes hardcoded zeros instead of using real benchmark data

The `--campaign stress-curve` dispatch path (`commands.rs:300-451`) calls `StressCurveDescriptor::run_one_sequence`, which internally calls `run_benchmark_suite` per N. `run_benchmark_suite` already returns `SuiteResult { results: Vec<BenchmarkResult>, ... }` with **fully populated** counters (interactions, mips, rounds, bytes, agent counts, all per-rule breakdowns).

But `run_one_sequence` **discards** that data and only returns `RepResult { wall, vmrss_peak_bytes, n, child_exit }`. The dispatch then synthesizes a fresh `BenchmarkResult` from scratch with **hardcoded zeros** for every counter:

```rust
// commands.rs:398-436 (post-TASK-0720)
rows.push(crate::bench::BenchmarkResult {
    benchmark: workload_id,
    input_size: rep.n.try_into().unwrap_or(u32::MAX),
    ...
    total_interactions: 0,       // ← HARDCODED ZERO
    mips: 0.0,                   // ← HARDCODED ZERO
    rounds: 0,                   // ← HARDCODED ZERO
    bytes_sent: 0,               // ← HARDCODED ZERO
    bytes_received: 0,           // ← HARDCODED ZERO
    agent_count_at_construction_complete: 0,   // ← HARDCODED ZERO
    live_agent_count_watermark: 0,             // ← HARDCODED ZERO
    interactions_by_rule: ...::default(),      // ← all zero
    ...
});
```

**Symptom (verbatim from CSV after BUG-A fix would still surface this):**

```
ep_annihilation,1000,local,2,0,true,0.000609,0,0.000,0,...
```

`total_interactions=0`, `mips=0`, `rounds=0`, `agent_count_at_construction_complete=0`. The bench was actually executed (wall=0.000609s confirms it ran), but the data is thrown away and zeros are stored.

This invalidates **every** sanity check from `docs/benchmarks/campaigns/stress-curve.md` §8 (mips plateau, log-log slope, speedup, etc.) — they all read columns that are zero.

**Fix:** Plumb real `BenchmarkResult` data through:

1. Modify `RepResult` (in `relativist-core/src/bench/stop_rule.rs`) to include `Vec<BenchmarkResult>` (or whatever shape lets us preserve per-rep details).
2. Modify `StressCurveDescriptor::run_one_sequence` in `bench/suite.rs` to capture `SuiteResult.results` from each `run_benchmark_suite` call and store them in the returned `RepResult` / `SequenceOutcome`.
3. Modify the dispatch in `commands.rs:300-451` to iterate the real `BenchmarkResult`s from each `RepResult` and emit them via `write_csv_detail` instead of synthesizing fake rows.

### Why QA + Reviewer + TASK-0720 missed both

- BUG-A is in **pre-existing code** (`suite.rs:948`). The QA Stage 5 attack vectors (which I authored in this TCC root session) focused on new code from TASK-0700..0708. Since `bench/suite.rs::run_benchmark_suite` is unchanged in D-014, it was outside the audit scope. The audit explicitly said "Stage 5 QA should adversarially probe ... `StopRule::check` NaN/INFINITY ... bash interrupt handling" — none of which surface a stdout-banner contamination.
- BUG-B is a logic bug introduced by **TASK-0720 itself** (the BUG-001 fix wired up CSV emission but did not plumb the real data). The TG-001 IT (`tests/d014_writer_to_plot_roundtrip.rs`) only validates the writer-to-plotter schema match and "non-empty CSV" — it does not assert that `total_interactions > 0` or any other counter is populated. The plot script (Path A in TASK-0720) was updated to consume the writer schema, so the column names line up; the plot just plots zeros, but the script doesn't fail on zero data.

## Files in scope

| File | Change |
|------|--------|
| `relativist-core/src/bench/suite.rs:948` | `println!` → `tracing::info!` (BUG-A) |
| `relativist-core/src/bench/stop_rule.rs::RepResult` | Add `pub bench_results: Vec<BenchmarkResult>` field (BUG-B step 1) |
| `relativist-core/src/bench/suite.rs::StressCurveDescriptor::run_one_sequence` | Capture `suite_outcome.results` from each `run_benchmark_suite` call into the new field (BUG-B step 2) |
| `relativist-core/src/commands.rs:300-451` | Iterate `rep.bench_results` and emit those instead of synthesizing zeros (BUG-B step 3) |
| `relativist-core/tests/d014_smoke_data_integrity.rs` | **CREATE.** New IT: spawn `relativist bench --campaign stress-curve --workload ep_annihilation --env in-process --workers 2 --n-seq 1000`, capture stdout to a temp CSV, assert `total_interactions > 0`, `mips > 0`, `rounds > 0`, `agent_count_at_construction_complete > 0`. ~30 LoC. |

## Files explicitly OUT of scope

- Other `println!` calls elsewhere in the codebase (this TASK fixes only the one at `suite.rs:948`; broader audit deferred).
- Changes to the `BenchmarkResult` struct shape.
- Changes to the bash orchestrator (`scripts/stress_curve.sh`) — the existing `tail -n +2` for second-and-subsequent rep CSV captures still works correctly after BUG-A fix removes the banner.
- The plot script `scripts/plot_stress_curve.py` — already correctly consumes the writer schema after TASK-0720 Path A.

## Acceptance criteria

1. **AC-1 (BUG-A):** `bench/suite.rs:948` does NOT use `println!`. `cargo run --release -- bench --campaign stress-curve --workload ep_annihilation --env in-process --workers 2 --n-seq 1000` emits CSV with header on line 1, no banner.
2. **AC-2 (BUG-B):** Same invocation as AC-1. Resulting CSV row has `total_interactions > 0`, `mips > 0`, `rounds >= 0` (some workloads may have rounds=0 legitimately for trivial N), `agent_count_at_construction_complete > 0` for `ep_annihilation N=1000` (expect ~2000 agents from 1000 ERA-ERA pairs at construction).
3. **AC-3:** New IT `tests/d014_smoke_data_integrity.rs` passes and asserts the four invariants above. Test self-skips with a clear message if `target/release/relativist[.exe]` is missing (consistent with `tests/d014_writer_to_plot_roundtrip.rs`).
4. **AC-4:** `scripts/stress_curve.sh --smoke` finishes with exit 0; resulting `aggregated.csv` has `header on line 1`, every column populated with non-zero values for ep_annihilation; plot script (when matplotlib is present) generates non-trivial PDFs.
5. **AC-5:** All existing pisos hold or rise: default ≥ 1900 (Windows) + 1 new IT = 1901. Final exact piso reported in commit message.
6. **AC-6:** `cargo clippy --all-features -- -D warnings` clean.
7. **AC-7:** `cargo fmt --check` clean.
8. **AC-8:** Bug reports updated:
   - `docs/qa/D-014-stress-curve-qa.md` — add BUG-A and BUG-B retroactively, mark `Status: FIXED via TASK-0722`.
   - `docs/reviews/D-014-stress-curve-review.md` — add a follow-up note that the writer-to-plotter roundtrip test was insufficient (only validated schema, not data integrity).

## Sequencing note

This is a quick fix landing on top of TASK-0720. After it closes, the operator
re-runs `scripts/stress_curve.sh --smoke`; if smoke produces non-zero data,
operator dispatches the real overnight campaign per TASK-0708 SENTINEL.

The combined D-014 + D-015 bundle remains shippable after this lands (no new
spec changes, no new unsafe surface, no new dependencies).
