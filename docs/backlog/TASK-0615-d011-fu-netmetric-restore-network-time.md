# TASK-0615 — D-011-FU-NETMETRIC: restore per-round network time instrumentation

**Phase:** D-012 (Instrumentation Restore) — Stage 3 DEV scope
**Bundle:** D-012 — Instrumentation Restore (successor of D-011, post-mortem follow-up)
**Status:** TODO
**Priority:** P0 (HIGH severity per `docs/analysis/D011-final-baseline-analysis-2026-05-04.md` §3 RF-04 + §7 row 1)
**Closes red flag:** RF-04 (`docs/analysis/D011-final-baseline-analysis-2026-05-04.md` §3 RF-04, lines 142–146)
**Spec:** none (instrumentation-only; no spec change required). Touches code already declared in `relativist-core/src/merge/types.rs:69,72` and consumed by `relativist-core/src/bench/suite.rs:621-625`.
**Depends on:** none. Independent of TASK-0616/0617/0618. (TASK-0616 path-(b) residual would consume this metric, but path-(a) — recommended — does not.)
**Estimated complexity:** S–M (~50 LoC production + ~80 LoC test).

---

## Context

The D-011 final baseline established that `rounds.csv::network_time_secs` is structurally `0.0` across every row of every v2 dataset. Investigation in RF-04 traced the defect to a **plumbing gap**: the `GridMetrics` struct already declares the fields and the bench harness already reads them into `BenchmarkResult`, but **no production code path ever pushes a `Duration` into the Vecs**. The pipeline is wired except for the producer end.

v1's `phase2_rounds.csv` shows ~0.29 s/round on the same TCP-localhost slot, so v1 had this metric instrumented. The metric was lost during the v2 refactor — most likely SPEC-17 (transport abstraction) moved the I/O into the `Transport` trait without carrying the timing hooks across.

Restoring this metric is a **prerequisite for the TCC's c_o/c_r empirical signature** (ARG-006 / ARG-007). Without per-round network time, `wall_dist - wall_seq` collapses to a single black-box overhead estimate, defensible but fragile under examination.

## Files in scope (file:line pointers)

| File | Change |
|------|--------|
| `relativist-core/src/merge/types.rs:69,72` | **Read-only reference.** `GridMetrics.network_send_time_per_round: Vec<Duration>` and `network_recv_time_per_round: Vec<Duration>` are already declared. Do NOT redefine. |
| `relativist-core/src/bench/suite.rs:621-625` | **Read-only reference.** Already reads from `metrics` and zips into `BenchmarkResult.network_time_per_round`. |
| `relativist-core/src/bench/csv.rs:159` | **Read-only reference.** CSV writer for `network_time_secs` already wired. |
| `relativist-core/src/protocol/coordinator.rs` | **MODIFY.** Around the per-round wire-facing calls (`recv_frame` / `send_frame` / batch-recv-awaiting-all-workers), wrap with `Instant::now()` + accumulate into per-round `Duration`. At end of round, push to `metrics.network_recv_time_per_round` and `metrics.network_send_time_per_round`. Locate call sites by searching for `recv_frame|read_frame|send_frame|write_frame` in `relativist-core/src/protocol/`. |
| `relativist-core/src/protocol/worker.rs` | **MODIFY (if symmetric metric is desired worker-side).** Worker-side timing is optional for the coordinator-side CSV column; acceptance criterion below targets coordinator-side rows only. If worker-side timing is added, it must NOT bubble back to coordinator (that is TASK-0616's territory). |
| `relativist-core/tests/d012_network_time_witness.rs` | **CREATE.** New integration test (~80 LoC) — 1-round dummy bench in TCP mode, assert `metrics.network_recv_time_per_round[0] > Duration::ZERO` and `metrics.network_send_time_per_round[0] > Duration::ZERO`. |

## Files explicitly OUT of scope

- `relativist-core/src/merge/grid.rs` (in-process path) — already pushes compute time at lines 154/564/1476; this task does not touch it.
- Any change to wire format, `Message` enum, or framing semantics.
- v1 code paths.
- Frozen baselines under `results/locked/`.
- Worker-side compute aggregation — that is TASK-0616.

## Acceptance criteria

1. After a TCP-mode benchmark completes, every row of `rounds.csv` from a non-trivial round (≥ 1 redex reduced) has `network_time_secs > 0.0`.
2. Pre-existing in-process bench rows (no TCP) remain `0.0` for `network_time_secs` (this metric is TCP-mode-only by definition).
3. New integration test `d012_network_time_witness` passes:
   - On HEAD before this change: FAILS (asserts 0 == nonzero).
   - On HEAD after this change: PASSES.
4. `cargo test --workspace` ≥ 1784 default + 1 (the new witness) = **≥ 1785 default**. Zero-copy floor unchanged at 1828. Streaming-no-recycle floor unchanged at 1775. v1 floor (690) inviolable.
5. `cargo clippy --all-targets --all-features -- -D warnings` clean.
6. `cargo fmt --check` clean.
7. No change in wall-clock ratio v2-post / v1 outside ±5% (instrumentation MUST be cheap). Verify by re-running TASK-0614 bench-verification slot (`ep_con 5M w=2 local`) post-change; ratio must remain ≤ 1.16× v1 (current 1.11×, +5% headroom).

## Test floor delta

**+1 default** (one new integration test binary). Floor expectation after this task lands: **≥ 1785 default**, **1828 zero-copy**, **1775 streaming-no-recycle**.

## Implementation hints

1. The minimum-disruption pattern is `Instant::now()` straddling each protocol call site. Beware of `tokio::time::timeout`-wrapped calls — measure the `await`, not the timeout overhead.
2. The coordinator's per-round loop already has clear phase boundaries (split → dispatch → collect → merge). Place the recv timer inside the collect phase; place the send timer inside the dispatch phase.
3. Use `Duration::checked_add` to accumulate; if it overflows (>584 years per round), prefer to log and zero rather than panic.
4. Do NOT use `std::time::SystemTime` — use `tokio::time::Instant` (or `std::time::Instant`) for monotonic measurement.

## Estimated LoC

- Production: ~50 LoC across `protocol/coordinator.rs` (and optionally `protocol/worker.rs`).
- Tests: ~80 LoC for `d012_network_time_witness.rs`.
- Total: ~130 LoC. Well under the 200 LoC ceiling.

## Cross-references

- `docs/analysis/D011-final-baseline-analysis-2026-05-04.md` §3 RF-04 (root mechanism), §6 verdict item 6 (defines this as a Phase 3 LAN prereq), §7 follow-up table row 1 (HIGH severity).
- `results/locked/v2_d011_final_baseline_2026-05-04/MANIFEST.md` "Known instrumentation defect" section.
- `docs/handoffs/2026-05-05-D012-instrumentation-restore-handoff.md` §2 row 1, §3 D-011-FU-NETMETRIC subsection.
- `docs/next-steps.md` "New follow-up surfaced by post-mortem analysis" bullet.
