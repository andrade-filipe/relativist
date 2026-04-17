# REVIEW — SPEC-19 §3.1 (item 2.34) Coordinator-Free Round (Merge Avoidance)

**Date:** 2026-04-16
**Reviewer:** reviewer agent (unified Code Quality + Architecture)
**Bundle:** TASK-0348, TASK-0349, TASK-0350, TASK-0351
**Stage:** 4 (REVIEW)
**Test count:** 850 → 878 (+28 inline/integration unit tests; cargo test clean)
**Lint:** `cargo clippy -- -D warnings` clean
**Format:** `cargo fmt --check` clean
**Release smoke:** `cargo build --release` clean

---

## 1. Verdict

**APPROVE.** No MUST-FIX items. 2 NICE-TO-HAVE items (non-blocking). All of R1–R7 PASS. Stage 5 QA may proceed.

The bundle is a textbook additive optimization: every data-path change is gated behind `GridConfig::coordinator_free_rounds` (default `false`, preserving v1 behavior), strong confluence (T4) is respected (skip-merge never alters the normal form because principal-port border quiescence implies no cross-partition redexes remain for this round), and wire-format forward-compatibility (R7) is preserved by bincode-v2 struct-append (no new `Message::*` variants).

---

## 2. Per Pre-Flagged Smell Verdicts

### Smell #1 — `UT-0351-09` uses source-grep via runtime `concat` to avoid self-grep

**Verdict: ACCEPT (as-is).** The test is structural, not behavioral, and `["Mes", "sage::"].concat()` genuinely prevents the self-match failure mode. The file comment flags the brittleness and the TEST-SPEC explicitly marks AST-based migration as a v2 follow-up. The test serves its R7 purpose (guard: no new `Message::*` variants leak into `protocol/coordinator.rs`) at zero production cost.

**Rationale for not escalating:** An AST-based matcher (syn) would require adding a dev-dependency only for this one test, and the test's signal (did someone add a wire variant?) is already catchable at PR review time. The self-grep mitigation is documented, correct, and local.

### Smell #2 — Skip-merge branch still calls `merge()`

**Verdict: ACCEPT (faithful to §3.1).** The spec bans skipping the *merge-redistribute* logical step; in the in-process `run_grid` path the concrete work that R3 saves is `reduce_border_once` + the second `reduce_all`, not the structural `merge()` reassembly itself (which is O(n) and side-effect-free). The inline comment in `grid.rs` correctly labels the wire-level round-trip saving as the future benefit (item 2.26 distributed transport). The UT-0351-10/11 equivalence tests would fail immediately if `merge()` were erroneously skipped and re-split inconsistently.

### Smell #3 — GNF branch increments `coordinator_free_rounds`

**Verdict: ACCEPT (correct semantics).** R4 defines GNF as the terminal specialization of R3: both conditions (no border activity AND zero local redexes) imply the coordinator's redistribute work is unnecessary. Counting the GNF round as "coordinator-free" aligns the metric with the spec's intent (rounds where the coordinator did not have to redistribute). `UT-0351-06` asserts `metrics.coordinator_free_rounds >= 1` on a pre-normalized net, locking in the semantic.

### Smell #4 — Vacuous-true on empty-workers input

**Verdict: ACCEPT (unreachable in core path).** `run_grid` asserts `num_workers >= 1` upstream; `compute_border_activity` on an empty `free_port_index` returning `false` (via `Iterator::any`) is the Rust-idiomatic identity. Adding a defensive early-return would be dead code. **QA Probe A** (below) covers this as a black-box guard instead of polluting production.

---

## 3. Additional Findings

### 3.1 Positive

- **Helper placement (`merge/helpers.rs`):** `compute_border_activity(&Partition) -> bool` is correctly located at the `merge` layer — it consumes a `Partition` (from `partition/`) and is called by both `protocol/worker.rs` (wire path) and `merge/grid.rs` (in-process loop). Module dependency direction (`net ← reduction ← partition ← merge ← protocol`) is preserved. No async leaks into core.
- **R1 ordering:** `protocol/worker.rs:236-257` calls `compute_border_activity(&partition)` AFTER `rebuild_free_port_index`. This is critical — computing on a stale index would silently break R2. The ordering is not merely correct; the TEST-SPEC-0349 adversarial case for a deleted border agent would catch a regression.
- **R43 default (flag off):** `GridConfig::default()` sets `coordinator_free_rounds: false`, which means zero v1 tests are changed in semantics. The 690 v1 tests still pass exactly as before; the +28 tests are strictly additive.
- **No `unwrap()`, no `unsafe`, no `println!`** in the production deltas. `tracing` is used consistently.
- **Forward-compat GridConfig construction:** all 3 production constructors (`bench/suite.rs`, `bench/benchmarks/*`, `config.rs::build_grid_config*`) use `..GridConfig::default()`. No manual field list that would break on future additions.
- **Wire FSM intact (R7):** `Message` enum in `protocol/types.rs` is unchanged; `PROTOCOL_VERSION` remains 2 (from SPEC-18); bincode-v2 append to `WorkerRoundStats` is forward-compatible.

### 3.2 Neutral observations

- The `worker.rs` FSM stub currently hardcodes `has_border_activity: false` with a comment pointing to the real production path in `protocol/worker.rs`. This is consistent with how other fields are stubbed in `worker.rs`, but it does mean that any future caller of the non-protocol FSM would observe conservative (pessimistic) behavior — i.e., the coordinator would never skip merge. That is safe (R3 is an OPTIONAL optimization) and aligned with how the stub is used today (tests only).

---

## 4. Spec Compliance Matrix (R1–R7)

| Item | Requirement | Location | Verdict |
|------|-------------|----------|---------|
| **R1** | Helper inspects `free_port_index` for principal-port endpoints | `merge/helpers.rs::compute_border_activity` via `matches!(p, PortRef::AgentPort(_, 0))` | **PASS** |
| **R2** | Round result MUST include `has_border_activity: bool` | `merge/types.rs::WorkerRoundStats.has_border_activity`; wired in `protocol/worker.rs` after `rebuild_free_port_index` | **PASS** |
| **R3** | Coordinator MAY skip merge-redistribute when all workers report false | `merge/grid.rs` skip-branch guarded by `config.coordinator_free_rounds && all_false` | **PASS** |
| **R4** | When also `local_redexes == 0`, declare GNF, terminate | `check_global_normal_form` helper + termination branch; UT-0351-06 | **PASS** |
| **R5** | Optimization MUST NOT alter final result (T4 confluence) | UT-0351-10/11 equivalence tests (flag on vs. flag off); UT-12 `church_add(2,3)→5` G1 check | **PASS** |
| **R6** | SHOULD use under strict BSP | Default is off; non-strict path does skip when enabled (UT-0351-08 verifies no-skip under `strict_bsp=false` when flag is also off; strict+on is the intended combo) | **PASS** |
| **R7** | Compatible with v1 full-partition and v2 delta protocols | No new `Message::*` variants; `WorkerRoundStats` additive (bincode v2); UT-0351-09 structural guard | **PASS** |

---

## 5. MUST-FIX Items

**None.**

---

## 6. NICE-TO-HAVE Items (non-blocking)

1. **CLI exposure (follow-up task).** `GridConfig::coordinator_free_rounds` is not wired to `config.rs` CLI flags today. The field is only reachable by library callers / benchmarks. Add a `--coordinator-free-rounds` flag (default false) in a follow-up backlog item so that Phase 3 LAN campaigns can toggle it without code changes. **Non-blocking** — the flag exists and is testable via library surface; CLI is orthogonal.

2. **Stub unification consideration.** `src/worker.rs` (FSM stub) duplicates the stat-build shape of `src/protocol/worker.rs` (real path). Extracting a `build_round_stats(&partition, local_redexes_resolved) -> WorkerRoundStats` helper in `merge/helpers.rs` would eliminate the divergence risk (next field added might be forgotten in the stub). **Non-blocking** — the stub's conservative `false` is safe; this is a refactor for maintainability.

---

## 7. QA Probes for Stage 5

The five probes documented by the developer stand. I add three more (A, B, C). **Total: 8 probes.**

### Already documented (restated for completeness)

1. **Oscillating activity:** craft a net where border activity flips true→false→true→false across rounds; assert metrics count only the false rounds and final state equals flag-off baseline.
2. **`max_rounds=0` with flag on:** ensure we don't count a GNF on a zero-round run.
3. **Lenient BSP + flag on:** confirm skip still yields correct result (R5 must hold in all BSP modes even though R6 only SHOULDs strict).
4. **Decode-after-skip on cascade_cross:** end-to-end workload where skip paths dominate; decode equality vs. flag-off.
5. **Large partition principal-port scan perf:** 100k aux-port partition with a single principal border; assert `compute_border_activity` is not accidentally O(n²).

### New probes I would add

**A. Empty-partition edge-case guard (covers Smell #4):** synthesize a config that would drive `compute_border_activity` with an empty `free_port_index` (even though the `assert num_workers >= 1` path prevents it in `run_grid`, call the helper directly from a black-box test). Assert returns `false` and does not panic. This pins the vacuous-true contract.

**B. Two-worker cascade round-boundary mix:** a net where round N reports `all_false` (skip-merge) but round N+1 has border activity. Assert that the N+1 redistribute path executes correctly (fresh split/merge), no stale partition state leaks, and final decode matches sequential reference.

**C. Strict-BSP skip + telemetry audit:** with flag on and strict BSP, drive a workload with known K skip-rounds. Assert `metrics.coordinator_free_rounds == K` exactly (not ≥, not approximate) and that non-skip rounds increment the counter zero times. This is the telemetric correctness probe — guards against someone double-counting GNF as a separate field.

---

## Appendix — Files Reviewed

**Production (Rust):**
- `relativist-core/src/merge/helpers.rs` (new helper + inline tests)
- `relativist-core/src/merge/grid.rs` (R3/R4 branches, GNF helper)
- `relativist-core/src/merge/types.rs` (WorkerRoundStats, GridConfig, GridMetrics additions)
- `relativist-core/src/worker.rs` (FSM stub)
- `relativist-core/src/protocol/worker.rs` (real wire path, R1 ordering)
- `relativist-core/src/protocol/types.rs` (Message enum unchanged — R7)
- `relativist-core/src/protocol/frame.rs` (test fixture update)
- `relativist-core/src/protocol/coordinator.rs` (UT-0351-09 structural guard)
- `relativist-core/src/bench/suite.rs`, `bench/benchmarks/church_sum_of_squares.rs`, `bench/benchmarks/cascade_cross.rs`
- `relativist-core/src/config.rs`

**Docs (read-only):**
- `.claude/agents/reviewer.md`
- `docs/pipeline-state.md`
- `specs/SPEC-19-delta-protocol.md` §3.1
- `docs/backlog/TASK-0348.md`, `TASK-0349.md`, `TASK-0350.md`, `TASK-0351.md`
- `docs/tests/TEST-SPEC-0348.md`, `TEST-SPEC-0349.md`, `TEST-SPEC-0350.md`, `TEST-SPEC-0351.md`

---

**End of review.**
