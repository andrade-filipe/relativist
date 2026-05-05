# QA — D-012 Instrumentation Restore (Stage 5, adversarial)

**QA agent:** Stage 5 (QA) of SDD pipeline
**Date:** 2026-05-05
**Scope:** Commits `360d6ea`, `fd2cafe`, `ca3634c`, `ac828f9` on `v2-development`
**Inputs read:**
- `docs/reviews/REVIEW-D012-instrumentation-restore-2026-05-05.md` (Stage 4 reviewer)
- `docs/handoffs/2026-05-05-D012-instrumentation-restore-handoff.md`
- `docs/backlog/TASK-0615..0618-*.md`
- `docs/tests/TASK-0615..0618-tests.md`
- `docs/analysis/D011-final-baseline-analysis-2026-05-04.md`
- Source: `relativist-core/src/protocol/coordinator.rs`, `bench/suite.rs`, `bench/csv.rs`, `merge/grid.rs`, `protocol/worker.rs`, `merge/types.rs`, `io/mod.rs`, `commands.rs`
- Tests: `relativist-core/tests/d012_*.rs`
- Scripts: `scripts/bench_docker_v2.sh`
- Data: `results/locked/v2_d011_final_baseline_2026-05-04/{detail,summary,rounds}.csv`

---

## Verdict

**REJECT** — 2 CRITICAL, 3 HIGH, 4 MEDIUM, 2 LOW.

Two findings should block Stage 6 close:
- **QA-D012-001 (CRITICAL).** SUM aggregation in TASK-0616 produces `compute_time > wall_clock` on multi-worker TCP (workers run in parallel — sum across workers is wall-clock total CPU work, not wall-clock time). Downstream `bench/suite.rs:531-535` derives `overhead_ratio = 1.0 - compute_total/elapsed`, which goes NEGATIVE when this happens. The summary CSV gains a negative-overhead row, and the analysis chapter loses its central c_o/c_r decomposition. The "parity with in-process" rationale is structurally false — in-process runs workers SERIALLY (loop), TCP runs them in PARALLEL.
- **QA-D012-002 (CRITICAL).** RF-07 root cause is in `scripts/bench_docker_v2.sh:283` — the shell script literally writes `mips = 0.0  # populated from detail rows; summary doesn't recompute`. TASK-0618 closure was attached to the wrong layer. The `detail.csv` of the v2 baseline already has `mips=1.067` and `total_interactions=500000` on tcp_localhost rows; the witness for "RF-07 closure" pins what was already true while the actual zero-MIPS bug ships in a bash file the witness never opens.

The remaining HIGH/MEDIUM findings are mostly pre-existing landmines that the new instrumentation walks past: bench harness mode handling that hardcodes `Mode::Local` regardless of the `--mode=tcp_localhost` flag (so the entire SUM/PARALLEL bug above is ALSO unreachable from the `bench` subcommand path), the `Duration::from_secs_f64(+inf)` panic vector if a worker reports infinity, sequential-await-of-parallel-recv timing semantics that doesn't match what the analysis doc assumes, and an early-return parity gap that the reviewer flagged (MF-001) — confirmed and extended below.

The reviewer's MF-001 and MF-002 are NOT duplicated here; cross-referenced where they intersect with new evidence.

---

## Severity counts

- **CRITICAL:** 2
- **HIGH:** 3
- **MEDIUM:** 4
- **LOW:** 2
- **Total:** 11

---

## Critical findings (Stage 6 must address; otherwise the metric is dishonest)

### QA-D012-001 — SUM aggregation makes `compute_time_per_round > wall_clock_per_round` on multi-worker TCP, producing NEGATIVE `overhead_ratio` in the bench CSV

**Severity:** CRITICAL
**Category:** Logic Error / Aggregation Semantics
**File:** `relativist-core/src/protocol/coordinator.rs:1397-1400` (TASK-0616 push site); cross-references `relativist-core/src/bench/suite.rs:526-535` (overhead_ratio derivation), `relativist-core/src/bench/mod.rs` (BenchmarkResult.overhead_ratio).

**Attack scenario.** Run `relativist coordinator --workers=4` against a workload where each worker computes for ~100 ms in parallel. Workers run TRULY concurrently (separate threads / processes / containers).
- Worker 0 ⇒ `reduce_duration_secs = 0.100`
- Worker 1 ⇒ `reduce_duration_secs = 0.100`
- Worker 2 ⇒ `reduce_duration_secs = 0.100`
- Worker 3 ⇒ `reduce_duration_secs = 0.100`

TASK-0616 (`coordinator.rs:1397`):
```rust
let compute_time_secs: f64 = worker_stats.iter().map(|s| s.reduce_duration_secs).sum();
// = 0.4 seconds
metrics.compute_time_per_round.push(Duration::from_secs_f64(0.4));
```

The wall-clock for that round (parallel work) is ≈ 0.100 s + dispatch + collect + merge ≈ 0.150 s.

When `bench/suite.rs:526` consumes `metrics.compute_time_per_round`:
```rust
let compute_total: f64 = grid_metrics.compute_time_per_round.iter().map(|d| d.as_secs_f64()).sum();
// = 0.4 (per round, summed across rounds = 0.4 * R)
let overhead_ratio = if elapsed > 0.0 { 1.0 - (compute_total / elapsed) } else { 0.0 };
// = 1.0 - (0.4 / 0.15) = -1.67
```

`overhead_ratio` is now **NEGATIVE**. It writes to `summary.csv::overhead_ratio_mean` per `bench/csv.rs:215`. The analysis doc (D011-final-baseline-analysis §4) uses `wall_dist - wall_seq` as the c_o estimate "for lack of per-component breakdown"; a negative-overhead row makes the per-component breakdown actively misleading.

**Observed.** TASK-0616 commit body cites parity with in-process: "mirrors the in-process path at `merge/grid.rs:103,154`, which pushes `t_compute.elapsed()` — the wall-clock of the SEQUENTIAL worker loop — equivalent to a sum since the workers run serially." That is structurally true for `run_grid` (see `merge/grid.rs:103-154`: a `for partition in plan.partitions.iter_mut()` loop — workers run SEQUENTIALLY in-process). It is structurally FALSE for `run_coordinator` over TCP, where workers genuinely run in parallel on separate runtimes/hosts. SUM and SEQUENTIAL-WALLCLOCK are equal IFF the underlying execution is sequential. They diverge as soon as W > 1 with real concurrency.

**Expected.** One of:
- (a) Aggregate using **MAX** across workers per round (= the slowest worker's wallclock = the BSP critical-path duration). This matches "compute time on the wire" in the analysis doc's mental model.
- (b) Push the SUM but document explicitly that this is "total worker CPU-time" (not wall-clock) and rename the field accordingly. Then `overhead_ratio`'s formula must be reworked.
- (c) Push BOTH (max per round, sum per round) under different names. Most useful for the TCC's c_o/c_r argument.

**Suggested fix (Stage 6).** Land path (a) — change `coordinator.rs:1397` to:
```rust
let compute_time_secs: f64 = worker_stats.iter().map(|s| s.reduce_duration_secs)
    .fold(0.0_f64, f64::max);
```
and add IT-0616-A6 ("multi-worker TCP compute_time ≤ wall-clock per round; SUM-vs-MAX semantic is documented in the field's doc-comment").

Alternative — land path (a) on the TCP path AND document that the in-process `run_grid` measures sequential-loop wallclock (which is approximately `sum(reduce_duration_secs)` per the loop body). The two paths report different but commensurate semantics: max-of-parallel-workers (TCP) ≈ slowest-worker-wallclock (sequential-loop in-process). This is closer to honest.

The current state is a hard-error CSV column that future analysis cannot trust on any multi-worker TCP run.

---

### QA-D012-002 — RF-07's literal failure mode lives in `scripts/bench_docker_v2.sh:283`, NOT in any production Rust file. TASK-0618's witness pins what was already true; the literal CSV bug stays unfixed.

**Severity:** CRITICAL
**Category:** Test Contract Fidelity / Wrong Layer
**Files:** `scripts/bench_docker_v2.sh:283` (the actual bug); `relativist-core/tests/d012_mips_witness.rs:55-130` (the misleading witness); `results/locked/v2_d011_final_baseline_2026-05-04/{detail,summary}.csv` (forensic evidence).

**Attack — already executed: read the locked baseline.**
Run:
```bash
awk -F',' 'NR==1{next} $3=="tcp_localhost" {print $1","$8","$9}' \
    results/locked/v2_d011_final_baseline_2026-05-04/detail.csv | head -3
```
Output:
```
ep_annihilation_con,500000,1.067
ep_annihilation_con,500000,1.046
ep_annihilation_con,500000,1.070
```
i.e., `detail.csv::total_interactions` and `detail.csv::mips` are **already non-zero** on every tcp_localhost row of the v2 baseline that supposedly motivated TASK-0618. RF-07's claim "v2-baseline shows total_interactions = 0 in all 40 rows" is false for tcp_localhost (only true for `sequential` rows, which is structurally correct — sequential mode doesn't populate grid metrics).

The `summary.csv::mips_mean = 0.000` finding IS real, but its root cause is at:
```bash
# scripts/bench_docker_v2.sh:283
mips = 0.0  # populated from detail rows; summary doesn't recompute
```
This Python-embedded-in-bash literal hardcodes the column to zero in `write_summary_row`. The witness `tcp_round_records_nonzero_total_interactions_and_mips` (which doesn't even exercise the TCP path — see SF-003 in the reviewer's report — and runs `run_grid` in-process) cannot detect this. RF-07's actual root cause was never reproducible from the Rust suite.

**Observed.** TASK-0618 commit body (`ac828f9`) line "PATH DECISION: path (a) — implement / Production changes: NONE" is honest: there's no production change because the production path was never broken. The commit body's footnote — "The v2-baseline showing total_interactions = 0 in all 40 rows is a separate latent issue (likely a CSV-writer-path artifact in a specific bench mode); the contract this task pins is 'a non-trivial bench through `run_grid` populates `total_interactions > 0`,' which is now test-asserted." — is closer to the truth, but still misattributes the bug.

**Expected.** Either:
1. **Fix `scripts/bench_docker_v2.sh:283`** to compute mips from the per-rep detail rows (a 3-line Python edit: read the same wall_clock + total_interactions used in `write_detail_row`, recompute, write to the summary cell). This actually closes RF-07.
2. **Demote TASK-0618's "Closes RF-07" claim** to "Pins the production data flow that ships `total_interactions` end-to-end (already correct in code; locks a regression sentinel). The literal RF-07 failure mode lives in `scripts/bench_docker_v2.sh:283` and is tracked as TASK-0619."

**Why this is CRITICAL.** The TASK-0618 commit message "Closes: TASK-0618 (D-011-FU-MIPS), RF-07" carries a false closure. Phase 3 LAN datasets generated by `bench_docker_v2.sh` will continue to ship `summary.csv::mips_mean = 0.000` regardless of any Rust-side fix. The TCC's analysis pipeline depends on this column being correct in summary.csv (the analysis script reads summary, not detail).

The reviewer's MF-002 partly identifies this gap (the witness doesn't exercise summary.csv). This finding sharpens it: the witness CANNOT exercise the summary.csv that's broken, because the writer of that CSV cell is in bash, not Rust.

**Suggested fix (Stage 6).**
- Path 1: file TASK-0619 to fix `bench_docker_v2.sh:283`; downgrade TASK-0618's commit message to "pins production data flow"; do NOT mark RF-07 closed.
- Path 2: fix `bench_docker_v2.sh:283` in this bundle (small enough), keep the witness as "production-side regression sentinel," and ALSO add a `tests/scripts/test_summary_writer.sh` that runs `bench_docker_v2.sh` against a small fixture metrics.json and asserts `mips_mean > 0` in the produced summary.csv.

Path 2 is preferred because it actually closes the literal RF-07 failure mode.

---

## High findings (data integrity — fix in Stage 6 or formalize a follow-up)

### QA-D012-003 — `cargo bench`-suite path NEVER executes the TASK-0615/0616 instrumentation; the bench harness hardcodes `Mode::Local` and routes ALL grid measurements through `run_grid` (in-process), regardless of `--mode=tcp_localhost`.

**Severity:** HIGH
**Category:** Architectural Mismatch / Coverage Gap
**File:** `relativist-core/src/bench/suite.rs:481-632` (`measure_grid` calls `run_grid`, line 494); `:574` (`mode: Mode::Local` hardcoded in returned `BenchmarkResult`); `:937-968` (the `for &workers in &config.workers` loop calls `measure_grid` regardless of `config.mode`).

**Attack scenario.** Run `relativist bench --benchmark=ep_annihilation --sizes=100 --workers=4 --mode=tcp_localhost --csv-summary=/tmp/s.csv --csv-rounds=/tmp/r.csv`.
- `commands.rs:299` parses `--mode=tcp_localhost` → `Mode::TcpLocalhost`.
- `suite.rs::run_benchmark_suite` → `measure_grid` (via `for rep in 0..config.repetitions` at line 953-968).
- `measure_grid` (line 481-632) is the SOLE measurement function for any non-sequential mode: it calls `run_grid(net_clone, &config, &strategy)` at line 494 (in-process path).
- Returned `BenchmarkResult { mode: Mode::Local, ... }` at line 574.
- The `network_send_time_per_round`, `network_recv_time_per_round`, and `compute_time_per_round` Vecs read from `grid_metrics` are populated by `run_grid` (in-process), NOT by `run_coordinator` (TCP).
- `network_*_time_per_round` will be EMPTY because `run_grid` doesn't populate them; the zip at `suite.rs:621-626` produces an empty `network_time_per_round`.
- `compute_time_per_round` is populated by `merge/grid.rs:154` (sequential loop wallclock), as before.
- The CSV row gets `mode=local` (because of the hardcoded `Mode::Local` overwrite!) and `network_time_secs=0.0`.

**Observed (in the locked v2 baseline).** `results/locked/v2_d011_final_baseline_2026-05-04/detail.csv` rows with `mode=tcp_localhost` are NOT generated by the `bench` subcommand; they are generated by the `bench_docker_v2.sh` shell script which runs `relativist coordinator` + `relativist worker` directly and writes the CSV by parsing `metrics.json` in Python. Confirmed by reading the script (lines 165-238).

So:
- The `bench` subcommand's `--mode=tcp_localhost` is essentially a no-op (or worse, mislabels the in-process row). RF-04's claim that v2's network_time_secs = 0.0 was correct **for the bench subcommand**, but irrelevant — the bench subcommand was never going to populate it anyway because it doesn't go through `run_coordinator`.
- The TASK-0615 instrumentation IS reached by the docker bench (via `relativist coordinator` → `metrics.json` → shell script → `rounds.csv`).
- The witness tests in `d012_*_witness.rs` exercise `run_coordinator` directly — correct path.
- BUT no test or guard catches the `bench --mode=tcp_localhost` mislabeling. A future operator running `cargo run --release --bench` with `--mode=tcp_localhost` will get rows labeled `mode=local` (the hardcoded overwrite) and `network_time_secs=0.0` (because the in-process path doesn't populate it).

**Expected.** Either:
1. Branch `measure_grid` on `params.mode`: when `Mode::TcpLocalhost`, fork an async tokio runtime, spin up a coordinator + N workers via `ChannelTransport::pair` (or real TCP), call `run_coordinator` and capture the per-round metrics. Substantial — likely 100-200 LoC. Out of D-012 scope.
2. Reject `--mode=tcp_localhost` from the `bench` subcommand with a clear error message: "the `bench` subcommand only measures in-process. For TCP-localhost benchmarks, use `scripts/bench_docker_v2.sh`."
3. At minimum, fix the `mode: Mode::Local` hardcode at `suite.rs:574` to honor `params.mode` so CSV consumers can at least filter rows correctly.

**Suggested fix (Stage 6).** Path 3 is a 1-line change (`mode: params.mode` instead of `mode: Mode::Local`), which honors the operator's request even if the underlying path doesn't actually do TCP. Path 2 is also acceptable. Path 1 is correct but out of D-012 scope — file TASK-0620.

**Why this is HIGH.** Combined with QA-D012-001 (SUM/MAX semantic gap), the TCP-path instrumentation is correct on docker bench but unreachable on the `bench` subcommand. A reader of the test inventory would conclude "TASK-0615 closes RF-04 for all TCP runs" — false for the `bench` subcommand. This compounds with reviewer's SF-003 (IT-0618-A1 misleadingly named "tcp_round_*") into a pattern: the bundle's tests don't exercise the path the bundle's commit messages claim.

---

### QA-D012-004 — TASK-0615 measures sequential-await-of-parallel-recv, mislabeled as "network recv time"

**Severity:** HIGH
**Category:** Metric Semantics Mislabel
**File:** `relativist-core/src/protocol/coordinator.rs:969-975` (collect-loop `for (wid, stream) in streams_to_poll`).

**Attack scenario.** 4 workers all dispatched in parallel at t=0. They all complete ~100 ms compute in parallel, then send `PartitionResult`. All 4 frames arrive at the coordinator's TCP buffers by ~t=120 ms.

Coordinator's collect loop:
```rust
for (wid, stream) in streams_to_poll {           // SEQUENTIAL iteration
    let recv_future = recv_frame(stream, ...);
    let t_recv = Instant::now();
    let recv_outcome = tokio::time::timeout(...).await;  // awaits THIS worker's frame
    network_recv_time = network_recv_time.saturating_add(t_recv.elapsed());
    ...
}
```

Trace:
- Iter wid=0: `t_recv = 100ms` (since round start), `await` returns at 120ms (data already in buffer or arriving). `t_recv.elapsed() ≈ 20 ms`.
- Iter wid=1: `t_recv = 120ms`, frame already in buffer (arrived at 120ms). `t_recv.elapsed() ≈ 0 ms`.
- Iter wid=2: `t_recv = 120ms`, ditto. `t_recv.elapsed() ≈ 0 ms`.
- Iter wid=3: `t_recv = 120ms`, ditto. `t_recv.elapsed() ≈ 0 ms`.
- `network_recv_time ≈ 20 ms` per round.

**This is NOT "the network recv time."** It is "the wallclock waiting for the slowest worker's compute to finish, plus the actual network recv for that worker." The other 3 workers' "network recv time" is recorded as ~0, which is also misleading — their bytes traveled the wire, just before the coordinator was looking.

The handoff §3 explicitly says: "the minimum-disruption pattern is `Instant::now()` straddling each protocol call site. Beware of `tokio::time::timeout`-wrapped calls — measure the `await`, not the timeout overhead." TASK-0615 followed the pattern literally — but the resulting measurement is dominated by **worker compute time of the slowest worker**, not by network time.

**Observed.** TEST-SPEC-0615 IT-0615-02 asserts `send_t0 != recv_t0` (defends against copy-paste bug) and ranges in `[1ns, 10s]`. Both pass with the current implementation, but neither pins the semantic. A network with zero physical latency (ChannelTransport, in-memory tokio duplex) will measure `network_recv_time ≈ slowest_worker_compute_time`. The test fixtures use ChannelTransport — so the measured `network_recv_time` is dominated by the redex reduction in the worker, NOT by any bytes-on-wire phenomenon.

**Expected.** Two options:
- (a) Rename the field to `collect_wait_time_per_round` (the wall-clock spent in the coordinator's collect phase, including waiting for the slowest worker). This is honest about what's measured.
- (b) Use `tokio::join!` or `select_all` to drive all recv_frames concurrently, then record the WALLCLOCK of the collect phase (single `Instant::now()` at start, single `.elapsed()` at end). This measures "how long the collect phase took as a whole," still mislabeled "network" but at least round-aligned.
- (c) Have workers timestamp their `PartitionResult` payload with `send_at = Instant::now()` and the coordinator computes `recv_at - send_at` per worker → **actual** network transit time. Requires wire-format extension.

**Suggested fix (Stage 6).** Path (a) is a doc-only fix (rename the field, update doc-comments, update CSV column header). Path (b) is structural and changes the metric's semantic. Path (c) is correct but expensive.

Recommend (a) + an explicit doc comment in `merge/types.rs` saying "this is collect-phase wallclock, not pure network transit; on a fast link it equals the slowest worker's residual compute." The TCC analysis chapter should NOT use this field as "transport overhead" without that caveat.

The current state is a metric that LOOKS like it answers "how much time was on the wire" but actually answers "how long did the slowest worker take that the coordinator hadn't already accounted for elsewhere."

---

### QA-D012-005 — `Duration::from_secs_f64(+inf)` panics on hostile or buggy worker; TASK-0616 only guards against negative via `.max(0.0)`

**Severity:** HIGH
**Category:** Panic Path / Hostile Input
**File:** `relativist-core/src/protocol/coordinator.rs:1397-1400`.

**Attack scenario.** A worker sends `WorkerRoundStats { reduce_duration_secs: f64::INFINITY, ... }`.
- bincode (the wire codec) serializes `f64::INFINITY` as the IEEE 754 bit pattern; deserialization on the coordinator side produces `f64::INFINITY` faithfully.
- Coordinator reaches `coordinator.rs:1397`:
  ```rust
  let compute_time_secs: f64 = worker_stats.iter().map(|s| s.reduce_duration_secs).sum();
  // = f64::INFINITY
  metrics.compute_time_per_round.push(Duration::from_secs_f64(compute_time_secs.max(0.0)));
  // f64::INFINITY.max(0.0) == f64::INFINITY
  // Duration::from_secs_f64(f64::INFINITY) PANICS:
  //   "secs is not finite"
  ```
- Coordinator panics. The whole bench dies. Other workers' connections drop. Metrics file unwritten.

How could a worker produce INFINITY? Several paths:
- A clock source that returns a future timestamp before the start (NTP step backward during a round) — `Instant::elapsed()` saturates, but `Duration::as_secs_f64()` won't produce INF unless the saturation pattern hits exactly the f64 boundary. Unlikely on monotonic clocks but POSSIBLE on platforms where Tokio's runtime falls back to wall-clock instants in test paths.
- A worker compiled with a different feature flag that injects telemetry overflow protection that maps "saturated" to `f64::INFINITY`. Doesn't exist today, but the wire is now sensitive to it.
- A malicious or compromised worker (security model: workers are trusted today, but the v2 architecture explicitly contemplates untrusted workers — see SPEC-10 token auth). A compromised worker that sends `f64::INFINITY` denies-of-service the entire grid.

`f64::NAN` is fine (`NAN.max(0.0) == 0.0` per IEEE 754; verified via Rust's f64::max docs: "If one of the arguments is NaN, then the other argument is returned"). But `+infinity` is not.

**Observed.** No test in `d012_compute_time_witness.rs` covers this case. IT-0616-03 only asserts `s.reduce_duration_secs >= 0.0` (line 339), which doesn't catch infinity.

**Expected.** Defensive coordinator-side validation:
```rust
let compute_time_secs: f64 = worker_stats.iter()
    .map(|s| s.reduce_duration_secs.clamp(0.0, 3600.0))  // 1-hour ceiling per worker
    .sum();
metrics.compute_time_per_round.push(Duration::from_secs_f64(compute_time_secs));
```
or use `Duration::try_from_secs_f64` (added in Rust 1.66) which returns `Result` instead of panicking, then fallback to `Duration::ZERO` on error with a `tracing::warn!`.

**Suggested fix (Stage 6).** ~3 LoC. Add IT-0616-A6 negative test: feed a worker_stats with `reduce_duration_secs = f64::INFINITY` directly to the aggregation site (extract the SUM line into a helper) and assert no panic + a clamped value pushed.

**Why this is HIGH.** Worker-controlled f64 reaches `Duration::from_secs_f64` after one max(0.0) only. The handoff explicitly mentioned defending against >584-year overflow ("Use `saturating_add` to defend against pathological Duration overflow"); the actual production code uses `from_secs_f64` which has a NARROWER finite-only requirement that `.max(0.0)` doesn't enforce. The bundle declares it has fixed the overflow concern; it has fixed only one half.

---

## Medium findings (correctness/coverage; should fix or formalize)

### QA-D012-006 — IT-0616-04 (#[ignore] body = `panic!`) is a future-proof booby trap

**Severity:** MEDIUM
**Category:** Test Hygiene / Code Smell
**File:** `relativist-core/tests/d012_compute_time_witness.rs:356-366`.

**Attack scenario.** A future maintainer follows TEST-SPEC-0616's documented procedure: "remove the `#[ignore]` line to re-enable the test when path (b) is in effect." On removal, the test body is `panic!("path (b) not in effect — see #[ignore] reason")`. The test always panics. The maintainer wastes time debugging — the panic message says "path (b) not in effect," but the maintainer just MIGRATED to path (b) and expects the test to validate that.

**Observed.** Reviewer's SF-002 flagged this as Significant. Confirming as Medium QA finding (separate evidence: this is also a documentation booby trap, not just a code smell — the test's stated procedure for re-activation will produce a confusing failure mode).

**Expected.** Two options:
- (a) Replace body with `let _ = (Duration::ZERO, Duration::ZERO);` and a top-line comment: "TODO: when path (b) is implemented, restore this body to compute residual = wall - merge - network and assert against `metrics.compute_time_per_round[0]` within tolerance."
- (b) Delete the test entirely — its existence as a placeholder serves no practical witness purpose; rely on TEST-SPEC-0616 git-history archaeology if path (b) is ever needed.

**Suggested fix (Stage 6).** Path (b). The placeholder is more harmful than helpful.

---

### QA-D012-007 — Reviewer's MF-001 confirmed; new evidence: `compute_time_per_round` parity gap is REACHABLE under common error conditions, not theoretical

**Severity:** MEDIUM
**Category:** Correctness / Length-Parity (cross-reference reviewer's MF-001)
**File:** `relativist-core/src/protocol/coordinator.rs:1399` (push site for `compute_time_per_round`); `:912/929/986/1111/1148/1161/1192/1246/1323` (early-return sites).

**Attack scenario beyond MF-001.** The reviewer noted that 9 early-return sites bypass `compute_time_per_round.push`. A common adversarial trigger:
- Worker dies mid-collect → `Ok(Err(e))` at line 1156 → `handle_connection_loss` returns `ConnectionLossOutcome::Abort` → `push_partial_round_metrics(&mut metrics); return Err(...)`.
- `effective_slots_per_round.len() == metrics.rounds + 1` (incremented at start of round).
- `compute_time_per_round.len() == metrics.rounds` (last round didn't reach the push site).
- The bench harness at `bench/suite.rs:611-615` zips by `iter()` — zip stops at the shorter; the LAST round's compute_time silently drops.
- More dangerous: the rounds CSV writer at `bench/csv.rs:151-185` indexes `r.compute_time_per_round.get(round).copied().unwrap_or(0.0)` — for round R-1, returns 0.0. The OPERATOR sees "round R-1 had 0 ms compute," which is indistinguishable from "round R-1 had a fast worker." Silent data loss.

**Observed.** No test checks the parity invariant `compute_time_per_round.len() == effective_slots_per_round.len()` after an error path. The reviewer suggested `debug_assert_eq!` as option 3 of MF-001 fix; concur, plus add it for the new TASK-0615 Vecs too.

**Expected.** Either:
- (a) Extend `push_partial_round_metrics` to push `Duration::ZERO` for the 5 missing Vecs (`bytes_sent_per_round`, `bytes_received_per_round`, `network_send_time_per_round`, `network_recv_time_per_round`, `compute_time_per_round`).
- (b) Add `debug_assert_eq!(metrics.compute_time_per_round.len(), metrics.effective_slots_per_round.len(), "compute_time parity broken on early return path")` at the function exit (bottom of `run_coordinator`).

**Suggested fix (Stage 6).** Both (a) AND (b). Same recommendation as reviewer's MF-001.

**Cross-reference.** Reviewer's MF-001. New evidence: cited the rounds.csv silent-data-loss pattern (in MF-001 the reviewer cited "downstream consumer using zip"; here we add the explicit `csv.rs:151-185` indexing pattern which loses silently).

---

### QA-D012-008 — `network_recv_time` measurement INCLUDES the `tokio::time::timeout` overhead despite explicit comment claim "Records the actual `await` time, not the timeout overhead."

**Severity:** MEDIUM
**Category:** Documentation/Implementation Mismatch
**File:** `relativist-core/src/protocol/coordinator.rs:973-975`.

**Attack scenario.** Configure `config.collect_timeout = 5 sec`. A worker takes 4.999 sec to send (NEAR but under the timeout).
- `t_recv = Instant::now()` at line 973.
- `tokio::time::timeout(5sec, recv_future).await` — returns `Ok(Ok((msg, n)))` at 4.999 sec.
- `t_recv.elapsed() = 4.999 sec`. Pushed to `network_recv_time`.

This is what the comment says ("Records the actual `await` time"). But:

A worker that ACTUALLY TIMES OUT (5.001 sec):
- `t_recv = Instant::now()`.
- `tokio::time::timeout(5sec, recv_future).await` — returns `Err(_)` at 5.000 sec (timeout fired).
- `t_recv.elapsed() = 5.000 sec`. **STILL PUSHED to `network_recv_time`.**
- Code falls into `Err(_) => let outcome = handle_phase_timeout(...)` (line 1184).

So the metric is contaminated by 5-second pseudo-recv-times whenever a worker times out and the elastic-departure path triggers. The comment claims this is intentional; the README/csv consumer doesn't know this.

A worker that DROPS THE CONNECTION early (e.g., crashes):
- `recv_frame` returns `Ok(Err(IoError))` at, say, 0.001 sec.
- `t_recv.elapsed() = 0.001 sec`. Pushed to `network_recv_time`.
- Code falls into `Ok(Err(e))` (line 1156).

OK that one is reasonable. But the timeout case is genuinely problematic — a CSV row showing `network_time_secs ≈ 5.000` per round in a benchmark with `collect_timeout = 5s` will be impossible to distinguish from a 5-second-network-bound workload.

**Observed.** The handoff implementation hint #1 explicitly says "measure the `await`, not the timeout overhead." The implementation measures the await INCLUDING the timeout. This is exactly the failure mode the handoff warned against.

**Expected.** Move the measurement INSIDE the `Ok(Ok(_))` branch:
```rust
let t_recv = Instant::now();
let recv_outcome = tokio::time::timeout(config.collect_timeout, recv_future).await;
let elapsed = t_recv.elapsed();
match recv_outcome {
    Ok(Ok((msg, nbytes))) => {
        network_recv_time = network_recv_time.saturating_add(elapsed);  // only count successful recv
        ...
    }
    Ok(Err(e)) => { /* DON'T accumulate — connection was lost, not a network event */ }
    Err(_) => { /* DON'T accumulate — timeout, not a network event */ }
}
```

**Suggested fix (Stage 6).** ~5 LoC restructure. Add IT-0615-A5: configure a worker that intentionally hangs for 6 sec; assert `network_recv_time < 5 sec` (the timeout) but the round still records SOME recv on the OTHER worker that did succeed.

---

### QA-D012-009 — `tcp_round_records_send_and_recv_separately` (IT-0615-02) `assert_ne!(send_t0, recv_t0)` — flaky on coarse-resolution clocks

**Severity:** MEDIUM
**Category:** Test Flakiness / Platform Dependency
**File:** `relativist-core/tests/d012_network_time_witness.rs:219-224`.

**Attack scenario.** Run the test on a Windows CI runner with QPC granularity ~16 ms (legacy hardware) or on a virtualized environment where `Instant::now()` has ms-scale resolution rather than ns. The send and recv on a `tokio::io::duplex` channel both complete in <1 ms. Quantized to 16 ms → both = 0 ms = exactly equal → test fails.

**Observed.** The test claims "tokio's duplex pipe still has scheduler-level jitter (microsecond range)." This is true on Linux/macOS with high-resolution monotonic clocks; on Windows the QPC resolution is platform-dependent. On legacy Windows VMs (Azure Standard_B2s before 2019) clock resolution can be 15-16 ms.

**Expected.** Replace `assert_ne!` with `assert!(send_t0.abs_diff(recv_t0) <= Duration::from_millis(50)` — proves they were measured independently (within 50 ms of each other) without requiring strict inequality.

Or, recognize that the `assert_ne!` IS the real witness for the copy-paste bug it was defending against (same accumulator wired to both fields would yield exactly-equal nanosecond values). Keep the assertion but bound it to `Duration::from_micros(1)` resolution: `assert!(send_t0.as_nanos().abs_diff(recv_t0.as_nanos()) > 1)`. Less strict than `!=`, robust against ms-resolution clocks.

**Suggested fix (Stage 6).** ~3 LoC. Reviewer's NTH-002 noted the OneShotTransport adapter pattern; this is a different test-hygiene issue.

---

## Low findings (informational / defer)

### QA-D012-010 — TASK-0617's `cfg(not(debug_assertions))` test arm validates the WRONG variant on hostile input

**Severity:** LOW
**Category:** Test Logic Soundness
**File:** `relativist-core/src/coordinator.rs:1862-1880`.

**Attack scenario.** A future code change makes `try_transition(DispatchingFirst, RequestWorkReceived { worker_id: 99 })` return `WorkerIdMismatch { ... }` (which is plausible — worker_id 99 is not a registered worker in this test context where `CoordinatorPullContext::new(4)` only knows workers 0..3).
- Today the test panics with "expected UnexpectedEvent."
- This panic is correct only IF the FSM is implemented to check event-type first, worker-id second. If a future refactor checks worker-id first (which is reasonable: "rejecting on worker_id_mismatch is more informative than 'unexpected event'"), the test will panic — the test's `panic!` arm fires.

The test has no recovery procedure: a future maintainer hits the panic and must investigate "is this a regression or a deliberate FSM change?" The error message gives no hint.

**Observed.** This is the test the developer ADDED a `WorkerIdMismatch` arm to in TASK-0617, not introduced. Reviewer's NTH-003 noted the `panic!` vs `unreachable!` choice; concurring with handoff's recommendation.

**Expected.** Make the panic message more diagnostic:
```rust
PullCoordinatorError::WorkerIdMismatch { .. } => {
    panic!(
        "FSM behavior change: DispatchingFirst + RequestWork now rejects on \
         worker_id_mismatch first. Update this test if intentional, or restore \
         event-type-first ordering. See ut_0577_08."
    );
}
```

**Suggested fix (Stage 6).** ~5 LoC message improvement. Trivial.

---

### QA-D012-011 — Reviewer's NTH-001 (release floor 1736) needs persistence in `CLAUDE.md` AND a corresponding test in `relativist-core/tests/test_floors.rs` if such a file exists

**Severity:** LOW
**Category:** Documentation / Floor Tracking
**File:** `relativist-core/CLAUDE.md` (Build & Test section).

**Observed.** Reviewer's NTH-001 noted that `cargo test --release: 1736` should land in CLAUDE.md. Ratifying. The 1726 → 1736 bump (TASK-0615/0616/0618 added 10 IT tests) is correct per the commit log. CLAUDE.md still says "Tests: 1181 default / 1224 zero-copy on v2-development" which is from before D-011 — out of date even before D-012.

**Expected.** Stage 6 should also bump the CLAUDE.md test counts to:
- `cargo test`: 1794 (default debug)
- `cargo test --features zero-copy`: 1838
- `cargo test --features streaming-no-recycle`: 1785
- `cargo test --release`: 1736
- v1 floor: 690

**Suggested fix (Stage 6).** ~5 LoC docs change. Reviewer flagged the same.

---

## Out-of-scope items (pre-existing, not introduced by D-012)

1. **Bench harness `mode: Mode::Local` hardcode at `suite.rs:574`.** Pre-existing per QA-D012-003. Worth filing TASK-0620.
2. **`scripts/bench_docker_v2.sh:283` hardcoded `mips = 0.0`.** Pre-existing per QA-D012-002. Worth filing TASK-0619.
3. **`bench` subcommand has no TCP path.** Pre-existing per QA-D012-003. Major architectural gap; out of D-012 scope.
4. **`condup_expansion` setup-time asymmetry (RF-02).** Explicitly out of D-012 scope per handoff §6.
5. **CI release-test lane.** Per TEST-SPEC-0617 §"CI lane (recommended follow-up)" — explicitly out of D-012 scope.
6. **`run_distributed_one_worker` helper duplication** between `d012_network_time_witness.rs` and `d012_compute_time_witness.rs`. Reviewer noted as "out-of-scope items observed."
7. **The 80 `cfg(debug_assertions)` occurrences across 11 files.** Pre-existing tech debt; TASK-0617 cleaned up only the 4 that broke release builds. The next release-mode test that breaks will surface a 5th.

---

## Severity guide for Stage 6 prioritization

**Must address before close:**
- QA-D012-001 (CRIT) — SUM/MAX semantic bug. Direct CSV correctness.
- QA-D012-002 (CRIT) — Wrong-layer RF-07 closure claim. Honest commit message.
- QA-D012-003 (HIGH) — Hardcode `Mode::Local` in suite.rs:574 OR reject `--mode=tcp_localhost` from `bench` subcommand.
- QA-D012-004 (HIGH) — Rename `network_*_time_per_round` to `collect_wait_time_per_round` and document semantic, OR file follow-up TASK.
- QA-D012-005 (HIGH) — Replace `Duration::from_secs_f64` panic vector with `try_from_secs_f64` or clamp.

**Should address (formalize as TASKs if not fixed):**
- QA-D012-006 (MED) — Replace IT-0616-04 `panic!` body or delete.
- QA-D012-007 (MED) — MF-001 fix path (a)+(b). Reviewer agrees.
- QA-D012-008 (MED) — Move `network_recv_time` accumulation inside `Ok(Ok(_))` arm.
- QA-D012-009 (MED) — Tighten or relax IT-0615-02's `assert_ne!`.

**Defer:**
- QA-D012-010 (LOW) — Diagnostic message for FSM panic.
- QA-D012-011 (LOW) — CLAUDE.md update (reviewer NTH-001).

**Followup TASKs to file:**
- TASK-0619: Fix `scripts/bench_docker_v2.sh:283` hardcoded `mips = 0.0`.
- TASK-0620: Add real TCP-mode path to `bench` subcommand (or reject the flag).
- TASK-0621: Audit remaining 80 `cfg(debug_assertions)` occurrences for release-mode regressions before next release-test bundle.
