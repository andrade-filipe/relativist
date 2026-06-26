# TEST-SPEC-T14: Conversion round-trip — Dense → Sparse → Dense (closes SC-014, SC-001 second surface)

**SPEC-22 §7.2 ID:** T14.
**Owning task:** TASK-0491 (`is_behaviorally_equal` helper); joint with TASK-0489 (`to_sparse`), TASK-0490 (`to_dense`).
**Parent spec:** SPEC-22 §3.2 R19, R20, R21; §4.6 conversion bodies.
**Type:** unit.
**Theory anchor:** REF-002 (Lafont 1997 — IC structural identity is representation-invariant); AC-001 (Haskell IC.Core baseline).

---

## Inputs / Fixtures

- A dense `Net` with **10 agents and some `None` slots from removals**:
  1. Create 10 agents at IDs 0-9 (mix of CON, DUP, ERA).
  2. Wire a non-trivial connection pattern (3 redexes, 4 internal wires, 2 FreePort connections).
  3. Pre-populate `freeport_redirects` with 1 entry: `freeport_redirects.insert(99, AgentPort(5, 0))`.
  4. Remove 3 agents (IDs 2, 5, 8) ⇒ free-list = `[2, 5, 8]`, agents `[Some, Some, None, Some, Some, None, Some, Some, None, Some]`.
- Conversion sequence:
  1. `let sparse = net.to_sparse();`
  2. `let net2 = sparse.to_dense(None);` — whole-net case.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T14-01 | `dense_to_sparse_to_dense_is_behaviorally_equal` | original `net`, post-conversion `net2` | `net.is_behaviorally_equal(&net2)` | `true`. (R21 round-trip 1 — closes SC-014.) |
| UT-T14-02 | `freeport_redirects_preserved_bit_exact` | same | `net.freeport_redirects == net2.freeport_redirects` | `true`. (Closes SC-001 second surface — entry `(99, AgentPort(5, 0))` survives both conversions intact.) |
| UT-T14-03 | `live_agent_set_preserved` | same | `net.agents.iter().filter(|x| x.is_some()).copied().collect::<HashSet<_>>() == net2.agents.iter().filter(|x| x.is_some()).copied().collect::<HashSet<_>>()` | `true`. |
| UT-T14-04 | `next_id_preserved` | same | `net.next_id == net2.next_id` | `true`. |
| UT-T14-05 | `root_preserved` | same | `net.root == net2.root` | `true`. |
| UT-T14-06 | `redex_queue_preserved_up_to_ordering` | same | sort `net.redex_queue.iter().copied()` and `net2.redex_queue.iter().copied()`; compare | equal. (R21 redex-queue is order-up-to-permutation.) |
| UT-T14-07 | `agents_len_may_differ` (documents R21 byte-equality non-requirement) | same | `net.agents.len()` vs `net2.agents.len()` | MAY differ — `net.agents.len() == 10`, `net2.agents.len() == 10` IF the trailing `None` at ID 8 is preserved by `to_dense(None)` (which uses `max_id + 1` = 10). The test asserts: `net2.agents.len() <= net.agents.len()` (no spurious growth) AND `is_behaviorally_equal` ignores trailing-slot differences. |
| UT-T14-08 | `free_list_set_preserved` | same | sort `net.free_list.clone()` and `net2.free_list.clone()`; compare as sets | sets equal. (Per R21 `is_behaviorally_equal` definition: free-list is compared as a set; LIFO order is not behaviorally significant across the conversion.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Net with no `None` slots (no prior removes) | `to_sparse().to_dense(None)` produces a net with empty `free_list`. `is_behaviorally_equal` passes. |
| EC-2 | Net with all `None` slots except one (e.g., 1 live agent at ID 5 in a length-10 arena) | `max_id == 5`; `to_dense(None)` produces an arena of length 6, free-list `[0, 1, 2, 3, 4]`. (No ID 5 in free-list since it's live.) `is_behaviorally_equal` passes against original (after trailing-`None` trim of original at IDs 6-9). |
| EC-3 | Net with empty `freeport_redirects` | UT-T14-02 trivially passes; both sides empty. |
| EC-4 | Net with a redex queue entry that references a removed agent (a buggy state) | The conversion preserves the bug faithfully; `is_behaviorally_equal` passes (both nets are bug-equivalent). The bug is caught elsewhere (T7 / R27). |

## Invariants asserted

- R19 (`to_sparse` semantics).
- R20 (`to_dense(None)` whole-net case).
- R21 (round-trip behavioral equality — closes SC-014).
- D1c (FreePort bijectivity preserved by `freeport_redirects` copy — UT-T14-02 closes SC-001 second surface).

## ARG/DISC/REF citation

- REF-002 (IC structural identity — the conversion is structure-preserving).
- AC-001 (Haskell IC.Core baseline — `Map AgentId Agent` ↔ HashMap parity).

## Determinism notes

`HashMap` iteration is non-deterministic, but `to_sparse` followed by `to_dense(None)` is deterministic in OUTCOME (the resulting `Net` has well-defined agents/ports content, only the redex_queue order may permute). UT-T14-06 explicitly normalizes via sort to absorb the permutation. Pure synchronous; no tokio.

`is_behaviorally_equal` (R21 / TASK-0491) is the load-bearing helper; without it, UT-T14-01 would fail on byte-equality due to trailing-slot trim. The test MUST use the helper, NOT `==`.

## Cross-test dependencies

- T15 is the inverse round-trip (Sparse → Dense → Sparse via `==`).
- T8 reuses `is_behaviorally_equal` for serde round-trip.
- TEST-SPEC-0491 covers the helper at plumbing level.
- T14a is the partition-scoped variant (`to_dense(Some(range))`).
