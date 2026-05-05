# TEST-SPEC-0616 — Tests for TASK-0616 — D-011-FU-COMPMETRIC: aggregate per-worker compute time on the distributed path

**Task:** TASK-0616 (D-012 Instrumentation Restore — Stage 3 DEV scope).
**Spec:** none (instrumentation-only). Production-side field already declared at `relativist-core/src/merge/types.rs` (`compute_time_per_round: Vec<Duration>`). In-process path already pushes correctly at `relativist-core/src/merge/grid.rs:154,564,1476`. Distributed (TCP) path does NOT.
**Closes red flag:** RF-05 (`docs/analysis/D011-final-baseline-analysis-2026-05-04.md` §3 RF-05, lines 148–154) — `compute_time_secs = 0.0` everywhere on v2 distributed rows.
**Origin:** `docs/handoffs/2026-05-05-D012-instrumentation-restore-handoff.md` §3 D-011-FU-COMPMETRIC + `docs/backlog/TASK-0616-d011-fu-compmetric-aggregate-worker-compute-time.md` Acceptance criteria.
**Test floor delta:** **+1 default** (one new integration-test binary `relativist-core/tests/d012_compute_time_witness.rs`, holding 3–4 `#[tokio::test]`s). Zero-copy and streaming-no-recycle floors unchanged.
**Prerequisites:**
- Path (a) — recommended: requires the developer to extend the `PartitionResult`-equivalent worker→coordinator return message with a `compute_duration: Duration` field.
- Path (b) — fallback: requires TASK-0615 to have landed (compute = wall − merge − network; the network term comes from TASK-0615).
- TASK-0617 not strictly required, but landing it first eases release-mode verification.
- No spec prerequisite for path (a) unless the wire format requires PROTOCOL_VERSION bump (escalate to ESPECIALISTA EM SPECS first; out of scope for this task).

---

## Test inventory

| test_id | level | target file::test_name | path | prerequisite TASK | cfg gates |
|---|---|---|---|---|---|
| IT-0616-01 | integration | `relativist-core/tests/d012_compute_time_witness.rs::tcp_round_records_nonzero_compute_time` | (a) or (b) | path (b) → TASK-0615 | none |
| IT-0616-02 | integration | `relativist-core/tests/d012_compute_time_witness.rs::compute_time_aggregates_across_w_workers` | (a) or (b) | path (b) → TASK-0615 | none |
| IT-0616-03 | integration | `relativist-core/tests/d012_compute_time_witness.rs::worker_reported_compute_matches_coordinator_aggregate_within_tolerance` | (a) ONLY | none | none |
| IT-0616-04 | integration | `relativist-core/tests/d012_compute_time_witness.rs::residual_compute_equals_wall_minus_merge_minus_network` | (b) ONLY | TASK-0615 | none |
| IT-0616-05 | integration | `relativist-core/tests/d012_compute_time_witness.rs::in_process_compute_time_remains_unchanged` | both | none | none |

**Totals:** 0 UT, 5 IT, 0 PT (only 4 of the 5 run on any single landing depending on path choice — see §"Path-conditional execution"). Net floor delta: **+1 default** (single new integration binary; the developer picks either IT-0616-03 or IT-0616-04 to land based on path choice; the other becomes a `#[ignore]` companion or is omitted with a comment).

---

## Path-conditional execution

The developer MUST choose path (a) or path (b) and document the choice in the implementation commit body (per TASK-0616 Acceptance criterion 9). The TEST-SPEC covers BOTH paths so neither implementation goes uncovered:

| Path | Always-run | Conditional |
|---|---|---|
| (a) — recommended: worker reports compute time | IT-0616-01, 02, 03, 05 | IT-0616-04 omitted or `#[ignore]`-gated |
| (b) — fallback: coordinator infers residual | IT-0616-01, 02, 04, 05 | IT-0616-03 omitted or `#[ignore]`-gated |

The unused test (03 or 04) MUST be present in the file with a `#[ignore = "path (X) not chosen — see TEST-SPEC-0616 §Path-conditional execution"]` attribute and a comment block citing the rationale. This preserves the spec's full coverage on git history; a future migration between paths re-enables the dormant test by deleting the `#[ignore]` line.

---

## Per-test specifications

### IT-0616-01 — `tcp_round_records_nonzero_compute_time`

**Purpose.** Headline acceptance witness for RF-05 closure. After a 1-round TCP-mode benchmark with non-trivial reduction work, `metrics.compute_time_per_round[0] > Duration::ZERO`. Pre-fix: FAILS (Vec is empty for distributed path). Post-fix: PASSES on either path (a) or (b).

**Setup.**
- Coordinator + 1 worker over localhost TCP.
- Workload: `dual_tree(depth = 6)` or `ep_annihilation(64)` — guarantees ≥ 1 actual reduction step on the worker.
- `GridConfig` with `workers = 1`, `transport = TCP`.

**Action.** Run 1 BSP round. Capture `GridMetrics`.

**Assertions.**
1. `metrics.compute_time_per_round.len() >= 1`.
2. `metrics.compute_time_per_round[0] > Duration::ZERO`.
3. The value is bounded — `>= Duration::from_micros(1)` (not a stub) and `<= Duration::from_secs(10)` (sanity). Guards against unit-confusion (ns into `Duration::from_secs(...)`).

**Failure message contract.** On (2) failure, panic message MUST cite "RF-05" and `docs/analysis/D011-final-baseline-analysis-2026-05-04.md §3 RF-05`. Example:
```
compute_time_per_round[0] = 0 — RF-05 regression on the distributed path. The in-process path at merge/grid.rs:154,564,1476 already populates this; the TCP path does NOT. See docs/analysis/D011-final-baseline-analysis-2026-05-04.md §3 RF-05 and TASK-0616.
```

**Boundary case coverage.** 1-redex workload is the smallest non-trivial case. 0-redex case is covered by IT-0616-05 negative-control + the TASK-0615 IT-0615-04 heartbeat-only edge.

**Why it must exist.** Direct closure of TASK-0616 acceptance criterion 1.

---

### IT-0616-02 — `compute_time_aggregates_across_w_workers`

**Purpose.** Aggregation correctness across W workers. With W=4 workers all doing roughly equal work, the per-round `compute_time_per_round[i]` reflects the chosen aggregation rule (sum vs max — see TASK-0616 Implementation hint 1). The test pins **whichever rule the developer picked** and documents the choice in the test body comment.

**Setup.**
- Coordinator + 4 workers over localhost TCP.
- Workload sized to give each worker a non-trivial slice — e.g., `ep_annihilation(256)` with `workers = 4`.
- `GridConfig { workers: 4, transport: TCP, .. }`.

**Action.** Run a 1- to 3-round bench. Capture per-round metrics for all workers (worker-reported via path (a)) AND the aggregated `metrics.compute_time_per_round`.

**Assertions.**
1. `metrics.compute_time_per_round.len() >= 1`.
2. `metrics.compute_time_per_round[0] > Duration::ZERO`.
3. **Aggregation rule check (the developer picks ONE of these to enable; the other goes in a comment):**
   - **Rule SUM (total CPU work across W):** `metrics.compute_time_per_round[0] >= max_worker_compute_time * (W as u32) / 2` — i.e., at least half the "if all workers busy in parallel" sum, accounting for stragglers. AND `metrics.compute_time_per_round[0] <= sum_worker_compute_times * 2` — i.e., not more than double (overhead/double-count guard).
   - **Rule MAX (parallel wall-clock):** `metrics.compute_time_per_round[0] >= max_worker_compute_time` AND `metrics.compute_time_per_round[0] <= max_worker_compute_time * 2` — should track the slowest worker, with bookkeeping headroom.
4. A test-body comment block records the developer's rule choice ("SUM" or "MAX") and the rationale ("v1 used SUM" or "v1 used MAX, parity restored" or "no v1 reference; we chose MAX for wall-clock semantics"). This comment is mandatory and reviewer-checked.

**Boundary case coverage.** W=4 spans the P-core boundary on the i7-1365U test machine (per RF-06 of the analysis); workers will have heterogeneous performance, so aggregation must tolerate it. Single-worker (W=1) aggregation is the IT-0616-01 case (degenerate where SUM == MAX).

**Why it must exist.** Constraint statement: "Aggregation correctness across W workers must be testable." Without this test, a path-(a) implementation that forgets to sum across workers (e.g., always reports worker 0's time only) silently passes IT-0616-01.

---

### IT-0616-03 — `worker_reported_compute_matches_coordinator_aggregate_within_tolerance` (path (a) ONLY)

**Purpose.** Path-(a)-specific witness: the worker's measured `reduce_*` duration (from `WorkerRoundStats { reduce_duration_secs, ... }` at `merge/grid.rs:147` for in-process; analogous worker-side timing for TCP) matches the coordinator-side aggregate within 10% (per TASK-0616 acceptance criterion 2).

**Setup.**
- Coordinator + 2 workers over TCP localhost. Workers MUST emit their per-round compute duration in `PartitionResult` (or equivalent) — this is the production change being tested.
- Workload: `dual_tree(depth = 7)` — large enough that worker-side `reduce` time dominates over message-passing latency (target: > 100 ms per worker).

**Action.** Run 1 round. Capture:
- `worker_compute_durations: Vec<Duration>` — extracted from each worker's response message (test must intercept these or read them from a debug field on the coordinator's per-round bookkeeping).
- `metrics.compute_time_per_round[0]` — coordinator's aggregate.

**Assertions.**
1. `metrics.compute_time_per_round[0] > Duration::ZERO`.
2. **Aggregation tolerance (rule-dependent):**
   - If rule is SUM: `aggregate ∈ [0.9 × Σ worker_compute_durations, 1.1 × Σ worker_compute_durations]`.
   - If rule is MAX: `aggregate ∈ [0.9 × max(worker_compute_durations), 1.1 × max(worker_compute_durations)]`.
3. Each `worker_compute_durations[i] > Duration::ZERO` — every worker actually reported.

**Failure message contract.** On (2) failure, panic message MUST cite "TASK-0616 acceptance criterion 2" and report both the worker-side total/max and the coordinator-side aggregate side-by-side, so the developer can see the drift direction at a glance.

**Boundary case coverage.** The 10% band intentionally allows for wire serialization overhead, scheduler jitter, and timer-resolution rounding. Tightening to 1% would flake on slow CI runners; loosening to 50% would mask real bugs (e.g., off-by-one in worker counting).

**Why it must exist.** Constraint statement: "test specs MUST cover both the 'implement (a)' path... since handoff §3 recommends it". Without this test, path (a) could compile and pass IT-0616-01/02 even if the worker-reported value is nonsense.

---

### IT-0616-04 — `residual_compute_equals_wall_minus_merge_minus_network` (path (b) ONLY)

**Purpose.** Path-(b)-specific witness: the residual formula `compute_time = round_wall − merge_time − network_time` is computed correctly and yields a positive, bounded value.

**Setup.**
- Coordinator + 1 worker over TCP localhost.
- Same workload as IT-0616-01 (`dual_tree(depth = 6)`).
- Requires TASK-0615 to have landed (network_time must be non-zero for the residual to be meaningful).

**Action.** Run 1 round. Capture:
- `metrics.compute_time_per_round[0]`
- `metrics.merge_time_per_round[0]` (already populated in v2)
- `metrics.network_send_time_per_round[0] + metrics.network_recv_time_per_round[0]` (from TASK-0615)
- Per-round wall-clock time (e.g., `round_total = network + compute + merge`, or available as a separate field).

**Assertions.**
1. `metrics.compute_time_per_round[0] > Duration::ZERO`.
2. `metrics.compute_time_per_round[0] <= round_wall` — residual ≤ total (sanity).
3. `metrics.compute_time_per_round[0] + metrics.merge_time_per_round[0] + metrics.network_send_time_per_round[0] + metrics.network_recv_time_per_round[0] ∈ [0.95 × round_wall, 1.05 × round_wall]` — the four components account for ≥ 95% of the round wall-clock (5% slack for unaccounted bookkeeping).
4. The residual is non-negative — `compute_time_per_round[0] >= Duration::ZERO`. (Path (b)'s biggest risk: if `network_time` is over-counted, the residual goes negative. The implementation MUST clamp to ZERO and emit a `tracing::warn!` rather than panic. The test asserts the post-clamp value.)

**Failure message contract.** On (3) failure, panic message MUST report the four component values + `round_wall` and the coverage ratio, so the developer can see which component is off.

**Boundary case coverage.** The 95% coverage assertion is the load-bearing check — it pins that all components are accounted for. Loosening to 80% would mask a bug where one component is silently dropped.

**Why it must exist.** Constraint statement: "a fallback test if path (b) ... is taken". Without this test, path (b) could compile and pass IT-0616-01 with a hardcoded `Duration::from_secs(1)` and no one would notice.

---

### IT-0616-05 — `in_process_compute_time_remains_unchanged`

**Purpose.** Negative control. The in-process path at `merge/grid.rs:154,564,1476` already populates `compute_time_per_round` correctly; this task MUST NOT touch it. Test pins the existing behavior so a regression in the in-process path during TASK-0616's distributed-path work is caught.

**Setup.** In-process `run_grid` (no TCP). Workload: same as IT-0616-01 (`dual_tree(depth = 6)`). `GridConfig { transport: InProcess, .. }`.

**Action.** Run 1 round. Capture `GridMetrics`.

**Assertions.**
1. `metrics.compute_time_per_round.len() >= 1`.
2. `metrics.compute_time_per_round[0] > Duration::ZERO` — same as TCP path, but the value comes from the existing instrumentation (not new).
3. The value is in a "reasonable" range — `>= Duration::from_micros(1)` and `<= Duration::from_secs(10)`.

**Boundary case coverage.** This is the in-process boundary; complements TASK-0615's IT-0615-03 which covers in-process for network time.

**Why it must exist.** TASK-0616 acceptance criterion 3: "In-process bench rows (no TCP) remain unchanged — already correct." Without this test, an over-eager refactor that "unifies" both paths could break the in-process path silently.

---

## Notes

### Determinism strategy

Tests use real-clock `Instant`. Per-test wall-clock < 5 s. Tolerance bands (10% in IT-0616-03, 5% in IT-0616-04, factor-2 in IT-0616-02) are wide enough to absorb scheduler jitter on CI runners but tight enough to catch off-by-orders-of-magnitude bugs.

### Path coordination with TASK-0618

Per TASK-0616 Implementation hint and TASK-0618 Implementation hint 1: if both tasks pick path (a), the same `PartitionResult` extension carries TWO new fields — `compute_duration: Duration` (this task) AND `total_interactions: u64` (TASK-0618). Coordinate so only ONE wire-format extension happens. The TEST-SPEC does not pin the field count (that's a code-review concern), but reviewer should verify the developer didn't inadvertently extend the payload twice.

### What this test does NOT cover

- **Network time** — TASK-0615 / TEST-SPEC-0615.
- **Total interactions / MIPS** — TASK-0618 / TEST-SPEC-0618.
- **Wire-format compatibility with older workers** — IF a PROTOCOL_VERSION bump is needed (escalate to ESPECIALISTA EM SPECS), separate task; out of scope here.
- **CON-DUP commutation timing.** The implementation hint flags it ("don't double-count"); a unit test could pin it but the integration-level IT-0616-03/04 covers it transitively.

### Coverage of constraints from the operator prompt

| Constraint | Where |
|---|---|
| Cover path (a) "worker reports per-round duration" | IT-0616-03 (load-bearing for path a) |
| Cover path (b) "compute = wall − merge − network" fallback | IT-0616-04 (load-bearing for path b) |
| Aggregation correctness across W workers must be testable | IT-0616-02 (load-bearing) |

### Cfg gates

None. Recommend re-running under `cargo test --release` post-TASK-0617.

---

## Cross-references

- **Source task:** `docs/backlog/TASK-0616-d011-fu-compmetric-aggregate-worker-compute-time.md`.
- **Bundle handoff:** `docs/handoffs/2026-05-05-D012-instrumentation-restore-handoff.md` §2 row 2, §3 D-011-FU-COMPMETRIC.
- **Red flag:** `docs/analysis/D011-final-baseline-analysis-2026-05-04.md` §3 RF-05 (lines 148–154).
- **Companion TEST-SPECs:** TEST-SPEC-0615 (network time companion); TEST-SPEC-0618 (total_interactions companion if path (a) chosen there).
- **Reference (in-process push sites):** `relativist-core/src/merge/grid.rs:154,564,1476`.

---

## Coverage matrix

| test_id | RF-05 closure | Path (a) | Path (b) | W-aggregation | In-process unchanged |
|---|---|---|---|---|---|
| IT-0616-01 | ✅ | ✅ | ✅ | partial (W=1) | — |
| IT-0616-02 | ✅ | ✅ | ✅ | ✅ (load-bearing) | — |
| IT-0616-03 | path-a closure | ✅ (load-bearing) | — | partial (W=2) | — |
| IT-0616-04 | path-b closure | — | ✅ (load-bearing) | — | — |
| IT-0616-05 | negative control | — | — | — | ✅ (load-bearing) |

---

## Out-of-scope (explicitly NOT specified here)

- Network time — TEST-SPEC-0615.
- MIPS / total_interactions — TEST-SPEC-0618.
- Release-mode test compilation — TEST-SPEC-0617.
- TCC artigo edits.
- Frozen baselines under `results/locked/`.
- v1 code paths.
