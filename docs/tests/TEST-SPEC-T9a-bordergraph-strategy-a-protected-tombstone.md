# TEST-SPEC-T9a: BorderGraph protected tombstone — Strategy A `DisableUnderDelta` (closes SC-005, default policy)

**SPEC-22 §7.1 ID:** T9a.
**Owning task:** TASK-0482 (RecyclePolicy + GridConfig.recycle_under_delta + is_border_protected wiring).
**Parent spec:** SPEC-22 §3.1 R10b (Strategy A), R10c (protected-tombstone semantics); §3.8 A10 (SPEC-19 §3.2 BorderGraph contract amendment); SC-005 closure.
**Type:** integration.
**Theory anchor:** ARG-005 INV-REC (delta border completeness — Strategy A satisfies the soundness condition by suspending recycle entirely under delta mode); SPEC-19 §3.2 BorderGraph contract.

---

## Inputs / Fixtures

- A 2-partition delta-mode scenario with `GridConfig.recycle_under_delta == RecyclePolicy::DisableUnderDelta` (the default).
- **Canonical fixture from Round 2 closure log (verbatim):** worker 0 owns IDs `[0, 100)`; the coordinator's `BorderGraph` records a border at `AgentPort(47, 0)` (i.e., agent 47's principal port is a border endpoint).
- Pre-state of worker 0:
  - `agents[47] = Some(CON)` connected to a partner agent (also in worker 0's range, e.g., agent 48).
  - `worker_0.net.recycle_policy = DisableUnderDelta`.
  - `worker_0.net.is_in_delta_round = true` (delta-round entry has fired per SPEC-19 R38).
  - `worker_0.net.border_entries_shadow = Some({47, ...})` populated by `build_subnet` per TASK-0482.
- Round N event: agent 47's principal-port partner is consumed by a local rule; agent 47 is consumed too (e.g., CON-CON annihilation).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T9a-01 | `agent_47_slot_is_none_post_remove` | pre-state | local rule consumes agent 47 (`net.remove_agent(47)`) | `worker_0.net.agents[47] == None`. (R10c: slot stays `None`.) |
| UT-T9a-02 | `agent_47_NOT_in_free_list` | same | same | `!worker_0.net.free_list.contains(&47)`. (R10c: ID is NOT pushed to free-list.) |
| UT-T9a-03 | `agent_47_in_protected_tombstones_shadow` (debug-only) | same; `#[cfg(debug_assertions)]` | check `worker_0.net.protected_tombstones.as_ref().unwrap().contains(&47)` | `true`. (R10c: tombstone tracked in debug shadow.) |
| UT-T9a-04 | `next_create_agent_returns_id_NOT_47` | same; pre-condition: free-list non-empty (e.g., agent 50 was also consumed earlier) | `let id = worker_0.net.create_agent(DUP)` | `id != 47`. (R10b Strategy A: workers MUST NOT pop from free-list during delta round; falls through to `next_id` allocation. Even if the test forced 47 into free-list, the protected_tombstones shadow guards via R27 family 3.) |
| UT-T9a-05 | `agent_47_reclaimable_after_reconstruct` | same; trigger `reconstruct` (SPEC-19 R38) at clean partition boundary | post-reconstruct, attempt `create_agent` | the `protected_tombstones` shadow has been drained back into `free_list` (per TASK-0482 acceptance criterion); ID 47 is now eligible for recycle. The test asserts the post-reconstruct invariant: 47 IS in the (drained) free-list and the next allocation can return it. |
| UT-T9a-06 | `port_slots_of_47_are_disconnected` | same | check `worker_0.net.ports[port_index(47, 0..3)]` | all DISCONNECTED. (R10c: ports cleared even though slot is a tombstone.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Multiple border-referenced agents consumed in the same round (e.g., agents 47 AND 92) | Both become protected tombstones; neither is in `free_list`. After `reconstruct`, both reclaimable. |
| EC-2 | Worker 0 in non-delta mode (`is_in_delta_round = false`) — same agent 47 consumed | Agent 47 IS pushed to `free_list` (no protection); R10b/R10c only apply under delta mode. (Documents the gating condition.) |
| EC-3 | `border_entries_shadow == None` (worker not under delta protection by build_subnet) | `is_border_protected` returns `false`; agent 47 is treated as ordinary recycle. (Documents the non-distributed fallback per TASK-0473's stub method.) |
| EC-4 | `RecyclePolicy::DisableUnderDelta` set BUT `is_in_delta_round == false` | Behaves as v1 mode: free-list pop is allowed; no protection applies. (The flag gates the policy, not the policy's mere presence.) |

## Invariants asserted

- D2 (Border completeness — preserved by recycle restriction).
- D3 (Cross-round border discovery — preserved).
- G1 (under delta mode — protected by R10b Strategy A).
- I3' stability subclause — border-referenced IDs preserved across rounds via protected tombstones.
- ARG-005 INV-REC (delta recoverability — Strategy A suspends recycle entirely; the coordinator's `BorderState.side_a = AgentPort(47, 0)` resolves to the same `Symbol` next round because the slot is preserved as a tombstone OR because the next round's `reconstruct` re-establishes the canonical view).

## ARG/DISC/REF citation

- ARG-005 INV-REC (delta border completeness — closing reference).
- SPEC-19 §3.2 (BorderGraph contract — amended by §3.8 A10).

## Determinism notes

The test simulates a 2-partition coordinator+worker setup but does NOT require live tokio sockets; the partition state can be constructed in-process with deterministic message ordering. Concretely:

- The worker dispatch loop's `is_in_delta_round` toggle is set/cleared synchronously in the test (no `tokio::select!` race).
- The `border_entries_shadow` is populated synchronously by `build_subnet` (per TASK-0482).
- The `reconstruct` step (UT-T9a-05) is invoked directly by the test; no network round-trip is simulated.
- Use `#[tokio::test(flavor = "current_thread")]` if any portion of the test crosses an async boundary; otherwise plain `#[test]` is sufficient.

The threat model R10b prevents — verbatim from §3.1 R10b: "round N produces border `B = (border_id, AgentPort(47, 0), AgentPort(123, 0))`; in round N+1 worker recycles ID 47 to a different `Symbol`; coordinator dispatches a `CommutationBatch` indexing `AgentPort(47, 0)`; the worker's local `agents[47]` now resolves to a different rule than the BorderGraph computed → G1 violation. R10b prevents the recycle in step 2." UT-T9a-04 is the operational closure of this threat model.

## Cross-test dependencies

- T9b is the Strategy B counterpart; together they validate both code paths under `GridConfig.recycle_under_delta`.
- TEST-SPEC-0482 plumbing test (`protected_tombstone_drained_at_reconstruct`, `default_policy_is_disable_under_delta`) provides primitive coverage; T9a is the integration-level closure.
- TEST-SPEC-0469 (the SPEC-19 §3.2 amendment) is forward-referenced by this test but no separate file exists — A10 is recorded in SPEC-22 §3.8 only.
- TEST-SPEC-EG-U7c (SPEC-20 delta departure-reclaim) shares the `is_in_delta_round` toggle infrastructure; coordinate fixture naming.
