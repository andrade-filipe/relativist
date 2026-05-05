# TASK-0616 — D-011-FU-COMPMETRIC: aggregate per-worker compute time on the distributed path

**Phase:** D-012 (Instrumentation Restore) — Stage 3 DEV scope
**Bundle:** D-012 — Instrumentation Restore
**Status:** TODO
**Priority:** P0 (HIGH severity per `docs/analysis/D011-final-baseline-analysis-2026-05-04.md` §3 RF-05 + §7 row 2)
**Closes red flag:** RF-05 (`docs/analysis/D011-final-baseline-analysis-2026-05-04.md` §3 RF-05, lines 148–154)
**Spec:** none (instrumentation-only; production message payload may grow by one `Duration` field — additive, no version bump required if the wire format already carries unknown trailing fields tolerantly; otherwise see Implementation hints).
**Depends on:** none for path (a) (recommended). Path (b) — residual measurement — would depend on TASK-0615; **path (a) is the recommended approach** so this task is independent.
**Estimated complexity:** M (~80 LoC production + ~100 LoC test).

---

## Context

The companion red flag to RF-04. `rounds.csv::compute_time_secs` is structurally `0.0` across every v2 row. Mechanism: the `GridMetrics.compute_time_per_round: Vec<Duration>` Vec is declared but never populated for the **distributed (TCP) code path**. The **in-process path** already pushes correctly — see `relativist-core/src/merge/grid.rs:154,564,1476`, where `WorkerRoundStats { reduce_duration_secs, ... }` (declared at `merge/grid.rs:147`) is consumed and aggregated.

For TCP mode, workers either don't measure `reduce_*` duration or the measurement is lost in transit. The fix lives at the boundary where `PartitionResult` payloads return from workers to the coordinator.

Two implementation paths — **the task author MUST choose one and document the rationale in the implementation commit**:

- **Path (a) — recommended.** Worker measures `reduce_*` duration, includes it as a new `Duration` field in `PartitionResult` (or equivalent return-message), coordinator sums across workers per round and pushes to `metrics.compute_time_per_round`. Structurally honest: each component reports what it actually measured.
- **Path (b).** Coordinator infers `compute_time = round_total - merge_time - network_time`. Mathematically convenient but introduces measurement coupling: any error in `merge_time` or `network_time` (TASK-0615) propagates into `compute_time`. Also requires TASK-0615 to land first.

§3 RF-05 of the analysis recommends path (a). The handoff §3 D-011-FU-COMPMETRIC also recommends (a). This task adopts **path (a) by default**; if the developer hits an obstacle (e.g., wire-format extension forbidden by SPEC-18), they may fall back to (b) with a one-paragraph rationale in the commit body.

## Files in scope (file:line pointers)

| File | Change |
|------|--------|
| `relativist-core/src/merge/grid.rs:147` | **Read-only reference.** `WorkerRoundStats { reduce_duration_secs, local_redexes, ... }` already declared with `reduce_duration_secs` field. In-process path already populates it. |
| `relativist-core/src/merge/grid.rs:154,564,1476` | **Read-only reference.** In-process aggregation site — pattern to mirror for distributed path. |
| `relativist-core/src/protocol/messages.rs` (or wherever `PartitionResult` lives) | **MODIFY (path a).** Add `compute_duration: Duration` field to the worker-→-coordinator return message. Field MUST be additive (default `Duration::ZERO` for backward compatibility on older worker builds, if SPEC-18 PROTOCOL_VERSION 3 is unwilling to bump to 4). |
| `relativist-core/src/protocol/worker.rs` | **MODIFY.** Worker measures `Instant::now()` straddling the per-round `reduce_all` (or equivalent) call; populates the new field on the response message. |
| `relativist-core/src/protocol/coordinator.rs` | **MODIFY.** On collect phase, sum `compute_duration` across all workers for the round; push the sum (or max — see Implementation hints) into `metrics.compute_time_per_round`. |
| `relativist-core/tests/d012_compute_time_witness.rs` | **CREATE.** New integration test (~100 LoC) — 1-round dummy bench in TCP mode, assert `metrics.compute_time_per_round[0] > Duration::ZERO` and roughly proportional to actual reduction work. |

## Files explicitly OUT of scope

- `merge::grid` in-process path — already correct, do not touch.
- Wire format breaking changes (PROTOCOL_VERSION bump). If genuinely required, escalate to ESPECIALISTA EM SPECS first; do not bump version inside this task.
- v1 code paths.
- Frozen baselines under `results/locked/`.
- Network-time aggregation — TASK-0615.
- `total_interactions` / MIPS — TASK-0618.

## Acceptance criteria

1. After a TCP-mode benchmark completes, every row of `rounds.csv` from a non-trivial round (≥ 1 redex reduced) has `compute_time_secs > 0.0`.
2. Sum of per-round `compute_time_secs` across workers roughly matches per-worker `reduce_duration_secs` reported by `WorkerRoundStats` on the same workload (within 10% — bookkeeping overhead and timer noise).
3. In-process bench rows (no TCP) remain unchanged — already correct.
4. New integration test `d012_compute_time_witness` passes after the change; FAILS before.
5. `cargo test --workspace` ≥ 1785 (post-TASK-0615) + 1 = **≥ 1786 default** (or ≥ 1785 if TASK-0615 has not landed). Zero-copy floor unchanged. Streaming-no-recycle floor unchanged. v1 floor (690) inviolable.
6. `cargo clippy --all-targets --all-features -- -D warnings` clean.
7. `cargo fmt --check` clean.
8. Wall-clock ratio v2-post / v1 unchanged outside ±5% (one extra `Duration` field per round-trip is negligible). Verify on `ep_con 5M w=2 local` slot.
9. **Path decision documented** — the implementation commit body states "path (a) — worker reports compute time" or "path (b) — coordinator infers compute time as residual" with one paragraph of rationale.

## Test floor delta

**+1 default** (one new integration test binary). Floor expectation after this task lands (assuming TASK-0615 already landed): **≥ 1786 default**, **1828 zero-copy**, **1775 streaming-no-recycle**.

## Implementation hints

1. **Aggregation choice — sum vs max.** Per-round `compute_time` across W workers can be aggregated as `sum` (total CPU work) or `max` (wall-clock parallel time, since workers run concurrently). The CSV column name `compute_time_secs` is ambiguous; v1's behavior is the empirical reference. Default to `max` for parity with wall-clock semantics; if v1 used `sum`, switch and note it in the commit.
2. **Wire-format additive extension.** If the `PartitionResult`-equivalent is `bincode`-serialized and the receiver tolerates trailing unknown fields, you can append `compute_duration` without bumping PROTOCOL_VERSION. If `bincode` is strict, you'll need a SPEC-18 amendment — escalate first.
3. **Don't double-count CON-DUP commutation.** If `apply_pending_commutation` runs locally (no wire), its time should land in `compute_time`, not `network_time`. The in-process path's pattern (`merge/grid.rs:154`) is the reference.
4. **Worker-side timer scope.** Time the actual `reduce_all` (or `reduce_n`, depending on path) — exclude partition-deserialize and result-serialize. Those should be charged to `network_time` (per TASK-0615).

## Estimated LoC

- Production: ~80 LoC across `protocol/messages.rs`, `protocol/worker.rs`, `protocol/coordinator.rs`.
- Tests: ~100 LoC for `d012_compute_time_witness.rs`.
- Total: ~180 LoC. Under the 200 LoC ceiling. **If the implementation grows beyond 200 LoC** (likely only if a SPEC-18 amendment is required), split into:
  - `TASK-0616a` — wire-format extension (additive, depends on ESPECIALISTA EM SPECS amendment)
  - `TASK-0616b` — coordinator-side aggregation + test
  - Document the split rationale in the commit body and update BACKLOG.md accordingly.

## Cross-references

- `docs/analysis/D011-final-baseline-analysis-2026-05-04.md` §3 RF-05 (root mechanism), §6 verdict item 6 (Phase 3 LAN prereq), §7 follow-up table row 2 (HIGH severity).
- `results/locked/v2_d011_final_baseline_2026-05-04/MANIFEST.md` "Known instrumentation defect" section.
- `docs/handoffs/2026-05-05-D012-instrumentation-restore-handoff.md` §2 row 2, §3 D-011-FU-COMPMETRIC subsection.
- `docs/next-steps.md` "New follow-up surfaced by post-mortem analysis" bullet (network metric mentioned by name; compute metric is the parallel companion).
- TASK-0615 (network time companion).
