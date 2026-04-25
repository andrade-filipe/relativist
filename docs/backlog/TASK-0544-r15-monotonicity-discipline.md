# TASK-0544: Generator R15 monotonicity contract — debug assertions + I3' reconciliation discipline

**Spec:** SPEC-21 §3.2 R15; SPEC-21 §3.5 closing note (closes SC-009); SPEC-21 §3.5 R27 I3' clause.
**Requirements:** R15 (generator-phase monotonicity contract), R27 I3' clause (uniqueness post-SPEC-22 §3.8 A1), §3.5 closing note (post-dispatch monotonicity-forbidden discipline).
**Priority:** P0 (correctness gate; failure here propagates to D1 / G1 violations).
**Status:** TODO
**Depends on:** TASK-0521 (AgentBatch), TASK-0540 (Benchmark trait), TASK-0541 (ep_annihilation), TASK-0542 (dual_tree).
**Blocked by:** none
**Estimated complexity:** S (~40 LoC production debug-assertions + ~80 LoC tests).
**Bundle:** SPEC-21 Streaming Generation — Phase D (generators).

## Context

R15 is a **generator-phase contract** strictly stronger than SPEC-01 I3' (post-SPEC-22 §3.8 A1):

> The generator MUST assign `AgentId` values to agents in a globally unique, monotonically increasing sequence across all batches. The maximum `AgentId` in batch `k` MUST be less than the minimum `AgentId` in batch `k+1`.

Satisfying R15 trivially satisfies I3'. **The contract scope is the generation pipeline only** (`make_net_stream`, `generate_and_partition_chunked`). Once dispatched, the worker arena MAY recycle slot IDs per I3' / SPEC-22 R1-R10c.

**§3.5 closing-note discipline (closes SC-009):** Code in `src/partition/streaming.rs` MUST NOT assume monotonicity on agents created post-dispatch — e.g., MUST NOT write `assert!(new_id > old_max_id)` patterns (cf. SPEC-22 §3.8 A6 forbidden assertion list).

**Two enforcement surfaces:**
1. **Generator-phase debug assertion (R15 enforcement).** A wrapper around `make_net_stream`'s iterator that, in `#[cfg(debug_assertions)]`, tracks the running max AgentId across batches and asserts the next batch's min > stored max. Lives in `src/bench/streaming.rs` or `src/partition/streaming.rs`.
2. **Post-dispatch discipline (closing-note enforcement).** A code-review obligation + CI lint forbidding the `assert!(new_id > old_max_id)` pattern in `src/partition/streaming.rs`. The lint mirrors SPEC-22 TASK-0493 (CI lint forbidding `SparseNet` imports in `src/reduction/**`).

## Acceptance Criteria

- [ ] Implement a debug-only wrapper iterator `R15MonotonicityChecker` in `relativist-core/src/bench/streaming.rs` (or partition/streaming.rs) that wraps `Box<dyn Iterator<Item = AgentBatch>>` and asserts `current_batch_min_agent_id > previous_batch_max_agent_id` (or that the previous max is `None` for the first batch) under `#[cfg(debug_assertions)]`.
- [ ] The wrapper is opt-in via a constructor `pub fn r15_monotonicity_checked(stream: Box<dyn Iterator<Item = AgentBatch>>) -> Box<dyn Iterator<Item = AgentBatch>>`. The default pipeline MAY apply this wrapper unconditionally in debug builds (DEVELOPER decides — recommendation: always-on in debug).
- [ ] Add a CI lint (custom rustc/clippy attribute or grep-based CI step) forbidding the regex pattern `assert!\s*\(.*new_id\s*>\s*old_max_id\)` AND the structurally similar `> .*max_assigned_id` patterns in `src/partition/streaming.rs`. Mirror the SPEC-22 TASK-0493 pattern.
- [ ] Add Rustdoc to `AgentBatch` (TASK-0521 file) re-emphasizing R15 is a generator obligation, NOT a downstream invariant.
- [ ] Test: malformed generator (deliberately non-monotonic batch sequence) triggers the debug assertion; release-mode builds DO NOT trigger (R15 is debug-only enforcement).
- [ ] Test: post-dispatch worker code that recycles slot IDs (per SPEC-22) MUST NOT trip any assertion in `src/partition/streaming.rs` — verified by exercising a streaming + free-list-recycle scenario at integration level.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/bench/streaming.rs` | modify | Add `R15MonotonicityChecker` wrapper iterator + `r15_monotonicity_checked` helper. |
| `relativist-core/src/partition/streaming.rs` | modify | Add post-dispatch discipline Rustdoc warning to module-level docs. |
| CI workflow (`.github/workflows/...` or repo-root `xtask`/Makefile) | modify | Add forbidden-pattern lint step. |

## Key Types / Signatures

```rust
/// Debug-mode wrapper that asserts R15 monotonicity across batches.
pub struct R15MonotonicityChecker {
    inner: Box<dyn Iterator<Item = AgentBatch>>,
    #[cfg(debug_assertions)]
    last_max_id: Option<AgentId>,
}

impl Iterator for R15MonotonicityChecker { /* ... */ }

pub fn r15_monotonicity_checked(
    stream: Box<dyn Iterator<Item = AgentBatch>>,
) -> Box<dyn Iterator<Item = AgentBatch>>;
```

## Test Expectations (forward-ref)

TEST-SPEC-0544:
- R15 violation triggers assertion in debug mode (deliberately malformed generator).
- R15 honored: ep_annihilation_stream + dual_tree_stream pass through the checker without assertion.
- Post-dispatch slot-recycle scenario: SPEC-22 free-list-recycle on the worker side does NOT trip any streaming-side assertion.
- CI lint detects forbidden patterns in a synthetic test source file.

## Invariants Touched

- R15 (generator-phase monotonicity) — enforced.
- I3' (uniqueness, post-SPEC-22) — preserved trivially under R15.
- §3.5 closing-note discipline (post-dispatch monotonicity-forbidden) — enforced via CI lint.

## Notes

- The debug-only enforcement is intentional: release-mode generators are presumed correct (the assertion is the safety net during development).
- The CI lint is the primary defense against the post-dispatch monotonicity-assumption regression. Without it, future contributors might naively add `assert!(new_id > old_max_id)` and silently violate the streaming + free-list-recycle interaction (SPEC-22 §3.8 A6).
- Consumed by TASK-0540, TASK-0541, TASK-0542 (generators verified to honor R15 via the checker), TASK-0571 (broader I3'-discipline audit).

## DAG Links

- **Predecessors:** TASK-0521, TASK-0540, TASK-0541, TASK-0542.
- **Successors:** TASK-0571 (I3' streaming-side compatibility audit), TASK-0600 (regression — verifies CI lint passes).
