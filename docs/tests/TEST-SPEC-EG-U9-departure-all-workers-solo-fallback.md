# TEST-SPEC EG-U9: all-remote-workers departure solo fallback (R26a, R27)

**SPEC-20 §7.1 ID:** EG-U9 (extended per Round 3 NF-007 closure to cover BOTH branches)
**Owning task(s):** TASK-0442.
**Type:** unit.
**Test name:** `test_departure_all_workers_solo_fallback`.

---

## Inputs / Fixtures

Two configurations covering the BOTH branches of R26a:

**Branch (a) — Hybrid:**
- `hybrid_coordinator = true`; K_remote=2 (K_eff=3).
- BOTH remote workers depart simultaneously (`D = K_remote = 2`; `D == K_eff - 1` since hybrid; equivalently `D + 1 == K_eff` so the self remains).

**Branch (b) — Non-hybrid:**
- `hybrid_coordinator = false`; K_remote=3 (K_eff=3).
- ALL 3 remote workers depart (`D == K_eff == 3`).

## Expected behaviour

**Branch (a) — Hybrid (D == K_eff - 1, all remotes gone):**
1. Coordinator detects D departures; `K_eff_new = K_eff - D = 1` (only self remains).
2. R26a: skip `FinalStateRequest` broadcast (no recipients).
3. Discard `retained_last_acked` of all D departed workers (no longer needed).
4. Reclaim their `retained_initial` (or last-acked) snapshots; queue them for next-window re-introduction per R5a/R15.
5. Transition to `SoloReducing` per R27.
6. Self-partition continues progress; the reclaimed snapshots are re-introduced when (and if) new workers join (via R15).

**Branch (b) — Non-hybrid (D == K_eff, all gone):**
1. Coordinator detects D departures; K_eff_new = 0; no reducer remains.
2. Transition to `Error` (no progress possible).

## Assertions

**Branch (a):**

| # | Assertion |
|---|-----------|
| A1 | FSM trace shows: `... → AcceptingMembershipChanges → ReclaimingPartitions → SoloReducing → ...`. |
| A2 | No `FinalStateRequest` was sent. (Captured wire trace: the FinalStateRequest message count from the coordinator across all departed peer streams is zero.) |
| A3 | `retained_last_acked` for the 2 departed workers is cleared. |
| A4 | The reclaimed partitions are tracked in a "next-window queue" (cross-check with R5a/R15 enqueue mechanism). |
| A5 | `canonicalise(final_solo_result) == canonicalise(reduce_all(input))` IF the input net is reachable from the self-partition's view + reclaimed snapshots. (If reclaim is queued for a future join that never happens, the test deliberately ends after a finite solo budget; assert "no panic, no deadlock".) |

**Branch (b):**

| # | Assertion |
|---|-----------|
| B1 | FSM transitions: `... → AcceptingMembershipChanges → Error`. |
| B2 | The error variant carries `AllRemoteWorkersDeparted` (or equivalent). |
| B3 | `retained_*` state is released (no leak). |
| B4 | The coordinator's run-loop returns `Err(...)`; no panic. |

## Edge / negative cases

- EC-1: D == K_eff (covers exact equality, the NF-007 anchor scenario; addressed by Branch (a) for hybrid, Branch (b) for non-hybrid).
- EC-2: D < K_eff but K_eff_new == 1 in non-hybrid (degenerate — only 1 worker remains) → NOT this test's territory; cross-link to a future or existing test.
- EC-3: D == K_eff in hybrid mode but the self-partition itself panics simultaneously → transitions to `Error` per EG-U16.

## Invariants asserted

- D3-elastic (R24c).
- D6 (Protocol Termination).
- G1 PRESERVED in branch (a) via ARG-006; NOT applicable in branch (b) where the run aborts.

## ARG/DISC/REF citation

ARG-006 (mixed-trace recoverability, branch (a)).

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. Both branches use scripted departures via `tokio::io::duplex` close.

## Cross-test dependencies

- EG-U16 (self-partition panic) for the related Error transition.
- EG-U7/8 for departure mechanics.
