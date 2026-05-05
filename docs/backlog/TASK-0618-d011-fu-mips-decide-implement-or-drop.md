# TASK-0618 — D-011-FU-MIPS: decide implement-or-drop for `mips_*` and `total_interactions` columns

**Phase:** D-012 (Instrumentation Restore) — Stage 3 DEV scope
**Bundle:** D-012 — Instrumentation Restore
**Status:** TODO
**Priority:** P2 (LOW severity per `docs/analysis/D011-final-baseline-analysis-2026-05-04.md` §3 RF-07 + §7 row 3 — symmetric defect on v1 and v2; harmless to comparison; pure presentation hygiene)
**Closes red flag:** RF-07 (`docs/analysis/D011-final-baseline-analysis-2026-05-04.md` §3 RF-07, lines 174–179)
**Spec:** none for path (b). Path (a) is purely additive instrumentation.
**Depends on:** none. Independent of TASK-0615/0616/0617.
**Estimated complexity:** S (path b: ~10 LoC production + ~10 LoC test) or M (path a: ~30 LoC production + ~20 LoC test).

---

## Context

`summary.csv::mips_mean = 0.000` and `detail.csv::total_interactions = 0` across **all 40 rows of v2-post and all 40 rows of v1**. Both columns are dead. The bench harness never populates `total_interactions`; `mips` is derived from it (`total_interactions / wall_clock_secs * 1e-6`), so when the input is zero, the output is zero.

Per RF-07: this is a **pre-existing dead column on both v1 and v2 — symmetric, harmless to comparison.** §6 verdict item 2 instructs the TCC to "Drop `mips_mean`, `network_time_secs`, `compute_time_secs` from any TCC table or figure that uses this dataset."

The dropped-from-tables decision is REDATOR territory and out of D-012 scope. What is in D-012 scope: **decide whether to fix the column or remove the column from the CSV writers**, so future runs do not present a zeroed metric to readers (TCC examiners, future contributors).

## Decision required

The implementation MUST choose **path (a) implement** or **path (b) drop**, document the rationale in a one-paragraph note in the implementation commit body, and split the work accordingly.

### Path (a) — implement `total_interactions` end-to-end

- Worker accumulates `total_interactions` per round (already partially in `WorkerRoundStats { local_redexes, ... }` at `merge/grid.rs:146`).
- Worker reports it on the per-round response (`PartitionResult`-equivalent message — same payload as TASK-0616's `compute_duration`).
- Coordinator sums into `BenchmarkResult.total_interactions`.
- `bench/suite.rs::aggregate` already derives `mips` correctly from `total_interactions / wall_clock_secs * 1e-6` — no harness change needed.

**Acceptance for (a):** `summary.csv::mips_mean > 0` for any non-trivial bench.

**Pros:** more useful CSV column; brings v2 to parity with what it was supposed to measure all along.
**Cons:** ~30 LoC production + ~20 LoC test; risks colliding with TASK-0616 if both touch the same `PartitionResult` payload (coordinate ordering: TASK-0616 lands first, then this task piggybacks on the extended payload).

### Path (b) — drop the columns

- Remove `mips`, `mips_mean`, `total_interactions` from CSV writers (`bench/csv.rs`, `bench/validate.rs`).
- Remove the corresponding fields from `BenchmarkResult` and `AggregatedStats` structs in `bench/mod.rs` (or set them to `Option<…>` and skip None).
- Update any test that asserts on these fields.

**Acceptance for (b):** `summary.csv` headers no longer contain `mips_mean` / `total_interactions` / related; `detail.csv` likewise; `cargo test` passes.

**Pros:** smaller change; no dead column to defend at TCC examination; honest failure mode.
**Cons:** loses the column permanently (a future restoration would re-introduce schema churn); v1's `summary.csv` retains the dead column unless we also patch v1 (FROZEN — do NOT).

### Recommendation guidance (not pre-decided)

Path (b) is **lower risk** and **smaller**. Path (a) is **more useful** but carries coordination overhead with TASK-0616. The handoff §3 D-011-FU-MIPS subsection explicitly states: "Decision is part of the task scope — task author should pick one with a one-paragraph rationale; trade-off is 'more useful CSV column' (a) vs 'no dead column to defend' (b)."

The author of this task is the developer who picks up Stage 3 DEV. They MUST document the choice in the implementation commit body. Suggested rationale framework:

- If TCC defense timeline is tight → path (b).
- If Phase 3 LAN run is more than 2 weeks out → path (a) (use the time to instrument honestly).
- If TASK-0616 lands path (a) (worker-→-coordinator additive payload extension), strongly prefer path (a) here too (one consolidated payload extension is cleaner than two separate ones).

## Files in scope (file:line pointers)

### Path (a)

| File | Change |
|------|--------|
| `relativist-core/src/merge/grid.rs:146` | **Read-only reference.** `WorkerRoundStats { local_redexes, ... }` — `local_redexes` is the per-round interaction count source. |
| `relativist-core/src/protocol/messages.rs` (or wherever `PartitionResult` lives) | **MODIFY.** Add `total_interactions: u64` field to the worker-→-coordinator return message. Coordinate with TASK-0616 to avoid two separate payload extensions. |
| `relativist-core/src/protocol/worker.rs` | **MODIFY.** Worker accumulates `local_redexes` across the round; populates the new field. |
| `relativist-core/src/protocol/coordinator.rs` | **MODIFY.** On collect phase, sum `total_interactions` across all workers per round; on bench end, hand to `BenchmarkResult.total_interactions`. |
| `relativist-core/src/bench/suite.rs` | **MODIFY (light).** Verify `aggregate` derives `mips` from the now-populated counter. No formula change. |
| `relativist-core/tests/d012_mips_witness.rs` | **CREATE.** ~20 LoC — assert `summary.csv::mips_mean > 0` after a 1-round non-trivial bench. |

### Path (b)

| File | Change |
|------|--------|
| `relativist-core/src/bench/csv.rs` | **MODIFY.** Remove `mips`, `mips_mean`, `total_interactions` from header strings and row writers. |
| `relativist-core/src/bench/validate.rs` | **MODIFY.** Same fields removed from validation/expected-shape code. |
| `relativist-core/src/bench/mod.rs` | **MODIFY.** Remove fields from `BenchmarkResult` and `AggregatedStats` (or `Option`-ize and skip when `None`). |
| Tests asserting on the dropped columns | **MODIFY.** Update assertions; remove if exclusively about the dropped columns. |
| `relativist-core/tests/d012_mips_columns_dropped.rs` | **CREATE.** ~10 LoC — assert `summary.csv` headers no longer contain `mips_mean` etc. |

## Files explicitly OUT of scope

- v1 code paths (FROZEN). v1's `summary.csv` will retain the dead column under both paths; document this in the commit body as expected.
- TCC artigo edits (REDATOR territory) — §6 verdict item 2 of the analysis is TCC-side guidance, not DEV-side.
- Frozen baselines under `results/locked/`.
- Network-time / compute-time aggregation — TASK-0615 / TASK-0616.

## Acceptance criteria

1. **Path decision documented.** Implementation commit body contains a one-paragraph rationale stating "path (a)" or "path (b)" and why.
2. **Path (a):** `summary.csv::mips_mean > 0` for any non-trivial bench; new witness test `d012_mips_witness` PASSES.
3. **Path (b):** `summary.csv` headers do NOT contain `mips_mean`, `total_interactions`, or related (`mips`, `mips_std`, etc.); new witness test `d012_mips_columns_dropped` PASSES.
4. `cargo test --workspace` ≥ current default floor + 1 (the new witness): **≥ 1786 default** (post-TASK-0615 + 0616) or ≥ 1785 (post-TASK-0615 only) or ≥ 1785 (standalone). Zero-copy / streaming-no-recycle floors unchanged. v1 floor (690) inviolable.
5. `cargo clippy --all-targets --all-features -- -D warnings` clean.
6. `cargo fmt --check` clean.

## Test floor delta

**+1 default** (one new witness test). Floor expectation depends on which other D-012 tasks have landed first; see acceptance criterion 4.

## Implementation hints

1. **If you pick path (a)**, coordinate with the TASK-0616 author. The cleanest sequence is: TASK-0616 lands first with `compute_duration`, then this task piggybacks on the same `PartitionResult` extension by adding `total_interactions`. Two fields, one wire-format change. If you reverse the order, the wire format gets two extensions — wasteful.
2. **If you pick path (b)**, audit all callers of `BenchmarkResult.total_interactions` and `AggregatedStats.mips_mean` to ensure removal is clean. `Grep "mips_mean\|total_interactions" relativist-core/` is the discovery query.
3. **Either path:** `bench/suite.rs::aggregate`'s formula `mips = total_interactions / wall_clock_secs * 1e-6` is correct as-is. Do NOT touch it under path (a); under path (b), remove the assignment site.
4. **v1 frozen baseline:** under both paths, v1's `summary.csv` (frozen at `results/locked/v1_local_baseline/`) retains `mips_mean = 0.000` columns. This is expected and not in scope.

## Estimated LoC

- Path (a): ~30 LoC production + ~20 LoC test = ~50 LoC. Well under 200.
- Path (b): ~10 LoC production + ~10 LoC test = ~20 LoC. Trivially under 200.

## Cross-references

- `docs/analysis/D011-final-baseline-analysis-2026-05-04.md` §3 RF-07 (root mechanism), §6 verdict item 2 (TCC-side framing), §7 follow-up table row 3 (LOW severity).
- `results/locked/v2_d011_final_baseline_2026-05-04/MANIFEST.md` "Known instrumentation defect" section.
- `docs/handoffs/2026-05-05-D012-instrumentation-restore-handoff.md` §2 row 4, §3 D-011-FU-MIPS subsection (the path-(a)-or-(b) language is verbatim from there).
- TASK-0616 (compute-time companion; payload coordination opportunity if both authors pick path (a)).
