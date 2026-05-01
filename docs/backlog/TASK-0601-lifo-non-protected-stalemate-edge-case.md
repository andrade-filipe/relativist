# TASK-0601 — LIFO non-protected stalemate edge-case fix (QA-D010-016)

**Phase:** B-4c (D-011 hardening — third of three sub-tasks)
**Bundle:** D-011 — Tier 3 Hardening + Bench Enablement
**Status:** PENDING
**Priority:** P3 (LOW — narrow edge case, no test currently fails)
**Spec:** SPEC-21 §3 streaming dispatch fairness invariant.
**Origin:** QA-D010-016 — under LIFO dispatch ordering with no protected-window guarantee, a worker can perpetually defer a redex if the coordinator keeps re-dispatching newer chunks first.
**Estimated complexity:** S (~30 LoC production + ~30 LoC test — narrow edge fix)
**Estimated stages duration:** Stages 2→3→4→5→6 over ~0.25 to 0.5 day.

---

## Context

QA-D010-016 identified a narrow edge case in pull-mode streaming: when the coordinator's pending-store dispatch policy is LIFO and there is no protected-window guarantee on already-dispatched-but-not-yet-completed chunks, a worker can be permanently starved of completing a stale chunk because newer ones keep arriving on top.

This is a fairness / starvation issue — no current test triggers it (LIFO + adversarial generator + slow worker). The fix is to add a protected-window invariant: a chunk that has been dispatched and acknowledged by a worker MUST be completable in bounded steps regardless of newer chunk arrivals.

## Dependencies

- None on D-011 amendments.
- Parallel-OK with B-4a/B-4b.

## Files in scope

| File | Change |
|------|--------|
| `relativist-core/src/partition/streaming.rs` (or pending-store dispatch site) | Add a protected-window guarantee: tag in-flight chunks; LIFO dispatch picks the youngest *un-protected* chunk. |
| `relativist-core/tests/spec21_lifo_starvation_edge.rs` (new) | Adversarial test: slow worker + LIFO + adversarial generator → must NOT starve; assert bounded completion of all chunks. |

## Files explicitly OUT of scope

- FIFO dispatch path — already correct.
- Any change to chunk-size selection logic (orthogonal to ordering).

## Acceptance criteria

1. LIFO dispatch implements a protected-window guarantee: in-flight chunks are completable in O(N) coordinator steps regardless of new arrivals.
2. New regression test demonstrates the previously starvation-prone scenario now terminates within a bounded number of dispatch rounds.
3. Existing tests pass with zero regression. No measurable performance overhead in the FIFO path.

## Test floor delta expected

**+2 to +3 tests** added.

## Notes

- LOW severity — landing this is a defense-in-depth move, not a fire.
- The protected-window invariant is a small but precise addition; favor a `chunk_in_flight: bool` flag over a counter unless QA Stage 5 demands stronger ordering.
