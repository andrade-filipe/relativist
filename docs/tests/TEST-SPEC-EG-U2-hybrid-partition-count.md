# TEST-SPEC EG-U2: hybrid partition count (R2)

**SPEC-20 §7.1 ID:** EG-U2
**Owning task(s):** TASK-0430.
**Type:** unit.
**Test name:** `test_hybrid_partition_count`.

---

## Inputs / Fixtures

- A small net (`agent_count >= 8`).
- `GridConfig { hybrid_coordinator: true, .. }`.
- K=3 remote workers connected.

## Expected behaviour

The orchestrator computes `K_eff = K + 1 = 4` and calls `split(net, K_eff=4)` producing 4 partitions (3 remote + 1 self).

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `partitions.len() == 4`. |
| A2 | Sum of `partition.agent_count()` equals `net.agent_count()` (no agent dropped). |
| A3 | Pairwise disjoint `AgentId` sets across partitions. |
| A4 | The self-partition is at `partitions[0]` (partition_index = 0 per R8). |
| A5 | The dispatch metrics record `effective_slots_per_round.last() == Some(&4)`. |

## Edge / negative cases

- EC-1: K=0 (hybrid only) → K_eff=1; `partitions.len() == 1`; the only partition is self.
- EC-2: K=1 (hybrid + 1 remote) → K_eff=2; `partitions.len() == 2`.
- EC-3: net.agent_count() < K_eff → `split` may return some empty partitions; assert this is allowed and the count is still K_eff.

## Invariants asserted

- D1 (Split/Merge Identity preconditions).
- D4 (ID Uniqueness across partitions).

## ARG/DISC/REF citation

ARG-002 P2 (split/merge identity).

## Determinism notes

Pure synchronous wrt the split call. Result canonical.

## Cross-test dependencies

Shares the split fixture with EG-U3 (id ranges), EG-U4 (merge).
