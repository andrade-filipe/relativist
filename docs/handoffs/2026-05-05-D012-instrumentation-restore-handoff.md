# Handoff — D-012 Instrumentation Restore (task-splitter)

**Status:** READY TO DISPATCH
**Saved:** 2026-05-05
**Active bundle:** D-012 (Instrumentation Restore). Successor of D-011 (CLOSED+LOCKED 2026-05-05). Driven by red flags surfaced in `docs/analysis/D011-final-baseline-analysis-2026-05-04.md` (RF-04, RF-05, RF-07) plus a release-mode test-suite blocker discovered during D-011 closure.
**Source documents:**
- `docs/analysis/D011-final-baseline-analysis-2026-05-04.md` §3 (Red Flags) + §7 (Concrete follow-ups)
- `docs/next-steps.md` "New follow-up surfaced by post-mortem analysis" + the four `D-011-FU-*` items
- `results/locked/v2_d011_final_baseline_2026-05-04/MANIFEST.md` "Known instrumentation defect" + "Test-floor status" sections

---

## 1. Why this bundle exists

The D-011 final baseline produced credible wall-time and byte-count data, but **three CSV columns are structurally zero** across every row of every v2 dataset (post-fix and pre-fix): `network_time_secs`, `compute_time_secs`, `mips_mean`. v1's frozen baseline has at least `network_time_secs` populated (~0.29 s/round on TCP-localhost). The defect is therefore a **v2 instrumentation regression**, lost during the v2 refactor that introduced SPEC-17 (transport abstraction), SPEC-18/19 (wire format v2 + delta protocol), and SPEC-21 (streaming).

The TCC's central c_o/c_r argument depends on decomposing distributed wall-time into compute vs transport. Without these per-round timings, the analysis must estimate `c_o ≈ wall_dist - wall_seq` as a single black-box proxy — defensible but fragile under examination. **Phase 3 LAN cannot produce ARG-006/007's empirical signature without these metrics restored.**

A separate but related blocker: `cargo test --release` does not compile at HEAD `b079cdc` (two pre-existing defects unrelated to D-011). This blocks any release-mode CI lane and any future bench that wants to combine release optimizations with `debug_assertions=false` invariant verification.

The bundle's scope is **strictly maintenance/instrumentation** — no new specs, no new features, no behavior change to the production binary's correctness or wall-time. It is "make the measurements honest before we measure on real LAN".

## 2. Inventory (4 work items)

Numbering must start at **TASK-0615** (last allocated task: TASK-0614 in commit `0fd27c0`).

| Follow-up | Severity | Title | Estimated LoC (prod / test) |
|---|---|---|---|
| D-011-FU-NETMETRIC | HIGH | Restore `network_send/recv_time_per_round` push sites in the TCP/transport I/O paths so `rounds.csv::network_time_secs` is non-zero on every TCP-mode round. | ~50 / ~80 |
| D-011-FU-COMPMETRIC | HIGH | Aggregate per-worker compute time into coordinator-side `GridMetrics.compute_time_per_round` for the **distributed (TCP)** code path. The in-process path already pushes (`merge/grid.rs:154,564,1476`); the distributed path does not bubble worker timings back. | ~80 / ~100 |
| D-011-FU-RELEASE-TESTS | MEDIUM | Make `cargo test --release` compile at HEAD. Two minimal fixes: (a) gate the `mod tests` at the bottom of `relativist-core/src/net/debug.rs:282-319` with `#[cfg(all(test, debug_assertions))]` so debug-only Net assertions tests don't reference symbols that vanish in release; (b) extend the non-exhaustive match in `relativist-core/src/coordinator.rs:1871-1873` (test `ut_0577_08_rejected_transition_dispatching_first_request_work`) to handle `PullCoordinatorError::WorkerIdMismatch { .. }`. | ~10 / 0 |
| D-011-FU-MIPS | LOW | Either (a) implement end-to-end `total_interactions` accounting (workers ship their per-round interaction counts → coordinator aggregates → `BenchmarkResult.total_interactions` → `mips` derived), or (b) remove the `mips_*` and `total_interactions` columns from `summary.csv` / `detail.csv` to avoid presenting a zeroed metric. **Decision is part of the task scope** — task author should pick one with a one-paragraph rationale; trade-off is "more useful CSV column" (a) vs "no dead column to defend" (b). | ~30 / ~20 (if a) or ~10 / ~10 (if b) |

**Total estimated:** ~170 LoC production + ~210 LoC tests across 4 atomic tasks. Should fit in a single SDD cycle (one DEV pass, one REVIEW, one QA, one REFACTOR).

## 3. Concrete pointers (file + line) for each work item

### D-011-FU-NETMETRIC

The `GridMetrics` struct (`relativist-core/src/merge/types.rs:69,72`) already has the fields declared:
```rust
pub network_send_time_per_round: Vec<Duration>,
pub network_recv_time_per_round: Vec<Duration>,
```

The bench harness already reads them (`relativist-core/src/bench/suite.rs:621-625`) and zips them into the `network_time_per_round` field of `BenchmarkResult`. The CSV writer reads from there (`bench/csv.rs:159`). **The entire pipeline is wired except for the production push sites.**

Recommended approach: instrument inside the coordinator's per-round loop, around the wire-facing calls in `relativist-core/src/protocol/coordinator.rs` and `relativist-core/src/protocol/worker.rs`. The 12 protocol files are listed in `grep recv_frame|read_frame|send_frame|write_frame relativist-core/src/`. The minimum-disruption pattern:

1. Around each round's `recv_frame` (or batch of recvs awaiting all workers), wrap with `Instant::now()` and accumulate into a per-round `Duration`.
2. At end of round, push to `metrics.network_recv_time_per_round` (and analogously for send).
3. Verify in test: a TCP-localhost bench round reports `network_recv_time > 0` and `network_send_time > 0`.

Acceptance: any row of `rounds.csv` from a TCP-mode benchmark has `network_time_secs > 0`. CI integration test (e.g., a 1-round dummy bench in `bench/validate.rs::tests`) asserts non-zero.

### D-011-FU-COMPMETRIC

The in-process path pushes: `relativist-core/src/merge/grid.rs:154,564,1476`. The distributed path does not. Workers DO measure `reduce_duration` and report it in `WorkerRoundStats { reduce_duration_secs, ... }` (`merge/grid.rs:147`), but only for the in-process model.

For TCP mode, look at how `PartitionResult` payloads come back from workers. The worker's per-round compute time is currently lost in transit (or never measured worker-side). Two implementation paths:

(a) Worker measures `reduce_*` duration, includes it as a new field in `PartitionResult` / equivalent message, coordinator sums across workers per round and pushes to `metrics.compute_time_per_round`.
(b) Coordinator infers compute time = `round_total - merge_time - network_time` (only viable after FU-NETMETRIC lands; defines compute as residual).

Path (a) is structurally honest; path (b) is mathematically convenient but introduces measurement coupling. **Recommend (a).**

Acceptance: TCP-mode `rounds.csv::compute_time_secs > 0` for non-trivial reduction rounds; sum of per-round compute_time roughly matches the worker-side `reduce_duration_secs` across all workers.

### D-011-FU-RELEASE-TESTS

Two surgical edits, no design decision needed.

(a) `relativist-core/src/net/debug.rs:282`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{PortRef, Symbol, DISCONNECTED};
    use super::super::core::Net;
    // ... 12 #[test] functions ...
}
```

becomes:

```rust
#[cfg(all(test, debug_assertions))]
mod tests {
    // unchanged body
}
```

(The `impl Net` block at the top of the file is already gated by `#[cfg(debug_assertions)]`; gating the test module symmetrically restores compilation in release.)

(b) `relativist-core/src/coordinator.rs:1871-1873`:

```rust
match err.unwrap_err() {
    PullCoordinatorError::UnexpectedEvent { .. } => {}
}
```

becomes:

```rust
match err.unwrap_err() {
    PullCoordinatorError::UnexpectedEvent { .. } => {}
    PullCoordinatorError::WorkerIdMismatch { .. } => {
        panic!("unexpected WorkerIdMismatch from DispatchingFirst + RequestWork; \
                expected UnexpectedEvent");
    }
}
```

(The `WorkerIdMismatch` variant was added by QA-D010-002 closure in commit `7fca43e` after this test was written; the test was simply never re-built in release mode, so the non-exhaustive match never surfaced.)

Acceptance: `cargo test --release` builds and runs to completion; the existing test floor (1784 default; verify exact release count after fix) is reported.

### D-011-FU-MIPS

Decision required. If implementing (a):
- Worker accumulates `total_interactions` per round (already partially in `WorkerRoundStats { local_redexes, ... }` at `merge/grid.rs:146`), reports it in `PartitionResult`-equivalent message, coordinator sums into `BenchmarkResult.total_interactions`. `bench/suite.rs::aggregate` already derives `mips` from `total_interactions / wall_clock_secs * 1e-6`.
- Acceptance: `summary.csv::mips_mean > 0` for any non-trivial bench.

If removing (b):
- Drop `mips`, `mips_mean`, `total_interactions` from the CSV writers (`bench/csv.rs`, `bench/validate.rs`) and from `BenchmarkResult`/`AggregatedStats` structs in `bench/mod.rs`.
- Update any test that asserts on these fields.
- Acceptance: `summary.csv` headers no longer contain the dropped columns; `cargo test` passes.

The task author should pick (a) or (b), document the choice in the task file, and split the implementation accordingly.

## 4. What task-splitter should produce

Following the standard pattern from `BACKLOG.md` and prior splits (TASK-0410..TASK-0500 SPEC-20 / SPEC-22):

1. **4 atomic task files** in `docs/backlog/`:
   - `TASK-0615-d011-fu-netmetric-restore-network-time.md`
   - `TASK-0616-d011-fu-compmetric-aggregate-worker-compute-time.md`
   - `TASK-0617-d011-fu-release-tests-fix-cargo-test-release.md`
   - `TASK-0618-d011-fu-mips-decide-implement-or-drop.md`

   Each file should follow the existing TASK template: priority, depends, complexity, scope (file pointers), acceptance criteria, estimated LoC, links back to RF-NN in the analysis doc.

2. **`BACKLOG.md` update** — add a new section "## D-012: Instrumentation Restore" between SPEC-22 and the open SPEC-21 hardening items, with a small table mirroring SPEC-20/22 sections. Bump the "Total tasks" / "todo" counters.

3. **Coverage matrix** (optional but consistent with prior splits): map each task → red flag → analysis section.

4. **Dependency note**: TASK-0617 (release tests) is independent and can ship first. TASK-0615 should ship before TASK-0616 if path (b) of FU-COMPMETRIC is chosen (residual measurement); if path (a), they're independent. TASK-0618 is independent. All 4 can be parallelised but a single bundle is cleaner.

## 5. Test floor entering D-012

| Profile | Floor |
|---|---|
| `cargo test` (default debug) | 1784 |
| `cargo test --features zero-copy` | 1828 |
| `cargo test --features streaming-no-recycle` | 1775 |
| `cargo test --release` | currently broken — see TASK-0617 |
| v1 inviolable floor | 690 |

After D-012 closure, expected delta: `cargo test --release` reports 1784+ (modulo any tests that genuinely fire only in debug); other profiles unchanged or +N from the new instrumentation tests.

## 6. Out of scope for this bundle

- Any change to wire format, transport behavior, or correctness semantics.
- Any change to v1 (frozen).
- Any modification of frozen baselines under `results/locked/`.
- The `condup_expansion` setup-time asymmetry (RF-02 / D-011-FU-CONDUP) — flagged as LOW in the analysis; defer to a separate one-off task or absorb into D-013.
- Any TCC artigo edits (REDATOR territory).

## 7. Agent prompt (paste verbatim into task-splitter)

```
You are dispatched to execute Stage 1 (SPLITTING) of the SDD pipeline
for bundle D-012 — Instrumentation Restore.

INPUTS — read in this order BEFORE acting:
1. docs/handoffs/2026-05-05-D012-instrumentation-restore-handoff.md —
   this document. §2 has the inventory of 4 work items; §3 has concrete
   file+line pointers for each; §4 has the expected output structure;
   §6 lists out-of-scope items.
2. docs/analysis/D011-final-baseline-analysis-2026-05-04.md — §3 RF-04,
   RF-05, RF-07 (root mechanism for each follow-up); §7 (concrete
   follow-up table).
3. docs/next-steps.md — "New follow-up surfaced by post-mortem analysis"
   block under the D-011 CLOSED+LOCKED section.
4. docs/backlog/BACKLOG.md — for format consistency. Note "Last updated"
   line, the per-spec sections (SPEC-20, SPEC-22), and the TASK-NNNN
   numbering convention with intentional gaps.
5. results/locked/v2_d011_final_baseline_2026-05-04/MANIFEST.md —
   "Known instrumentation defect" and "Test-floor status" sections.

NON-INPUTS — do NOT read or modify:
- specs/ directory (no spec changes in this bundle)
- src/, tests/ (developer territory)
- docs/progress.md (historical only)
- Any results/locked/ contents

DELIVERABLES (exactly):
A. 4 task files in docs/backlog/ named TASK-0615 through TASK-0618 per
   §4 of the handoff. Each file follows the existing template
   (priority, depends, complexity, scope with file:line pointers,
   acceptance criteria, estimated LoC, RF cross-ref).
B. Update docs/backlog/BACKLOG.md:
   - Bump "Last updated" to 2026-05-05
   - Bump "Total tasks" and the appropriate counters
   - Add a "## D-012: Instrumentation Restore" section with a small
     table for the 4 tasks
   - Add a coverage line under the existing "**Total tasks:**" header
     summarizing this bundle (e.g., "**D-012 split:** 4 atomic tasks
     TASK-0615..0618 covering RF-04/05/07 from D011 final baseline
     analysis + cargo test --release blocker; ~170 LoC production +
     ~210 LoC tests.")
C. NO src/ or tests/ edits. NO spec edits. NO behavioral changes.

CONSTRAINTS:
- TASK-0617 (release-tests) must be marked "Depends: none, can ship
  first" — it unblocks CI release lane independently.
- TASK-0618 (MIPS) explicitly requires the implementation to choose
  path (a) implement or (b) drop, with a one-paragraph rationale; the
  decision is part of the task author's scope, not pre-decided here.
- Each task should specify which red flag (RF-04/05/07) it closes and
  reference the analysis doc.
- Do NOT exceed 200 LoC per task estimate; if any task expands beyond
  that, propose a split into letter-suffixed sub-tasks (e.g., 0616a /
  0616b) and document the split rationale.

REPORT FORMAT (when done):
- One-paragraph summary of what was created
- List of files written/modified with line counts
- Confirmation that all 4 RF cross-refs land
- Any deviation from the handoff §2 inventory (with rationale)

After reporting, halt. Do not advance to Stage 2 (test-generator);
the operator will run that explicitly.
```

---

**Operator note:** when you (the human) dispatch this, you can either run task-splitter directly from this Claude Code session via the `Agent` tool, or invoke it from the TCC root session as you prefer. The handoff is self-contained and the prompt above is paste-ready.
