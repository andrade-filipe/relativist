# TEST-SPEC-T15: Conversion round-trip — Sparse → Dense → Sparse (structural equality)

**SPEC-22 §7.2 ID:** T15.
**Owning task:** TASK-0491 (`is_behaviorally_equal` is NOT used here; full `==` IS used per R21).
**Parent spec:** SPEC-22 §3.2 R21 (round-trip 2: structural equality on SparseNet).
**Type:** unit.
**Theory anchor:** AC-001 (Haskell IC.Core baseline — `Map AgentId Agent` representation has no trailing-slot ambiguity).

---

## Inputs / Fixtures

- A `SparseNet` with 10 agents (mix of CON, DUP, ERA) and a non-trivial wire pattern:
  - Create agents 0-9 via `create_agent`.
  - Connect: `(0,0)-(1,0)`, `(2,0)-(3,0)`, `(0,1)-(2,1)`, `(0,2)-(3,1)`, `(4,0)-FreePort(99)`.
  - Populate `freeport_redirects.insert(99, AgentPort(4, 0))`.
- Conversion sequence:
  1. `let dense = sparse.to_dense(None);`
  2. `let sparse2 = dense.to_sparse();`

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T15-01 | `sparse_to_dense_to_sparse_is_structurally_equal` | original `sparse`, post-conversion `sparse2` | `sparse == sparse2` (full PartialEq on SparseNet) | `true`. (R21 round-trip 2: SparseNet representations have no trailing-slot ambiguity, so byte equality is achievable.) |
| UT-T15-02 | `agents_hashmap_byte_equal` | same | `sparse.agents == sparse2.agents` | `true`. |
| UT-T15-03 | `ports_hashmap_byte_equal` | same | `sparse.ports == sparse2.ports` | `true`. |
| UT-T15-04 | `redex_queue_preserved` | same | `sparse.redex_queue == sparse2.redex_queue` | `true` IF reduction has not been driven; a clean round-trip preserves queue order exactly because both `to_dense` and `to_sparse` clone the `VecDeque` directly. |
| UT-T15-05 | `freeport_redirects_byte_equal` | same | `sparse.freeport_redirects == sparse2.freeport_redirects` | `true`. (Closes SC-001 second surface from the SparseNet side.) |
| UT-T15-06 | `next_id_root_preserved` | same | `sparse.next_id == sparse2.next_id && sparse.root == sparse2.root` | `true`. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Empty SparseNet | Round-trip yields empty SparseNet; `==` passes. |
| EC-2 | Single agent | Round-trip preserves the single entry in `agents` and any port entries. |
| EC-3 | Sparse with `freeport_redirects` keyed by an AgentId of a removed agent (consistent `to_sparse` would have already purged this — but if test injects it directly, the round-trip is faithful) | `==` passes; the corrupt entry is preserved (this is a property of the round-trip, not a sanity check on the sparse net). |
| EC-4 | `next_id` larger than any live agent ID (post-removes) | Round-trip preserves `next_id` exactly. |

## Invariants asserted

- R21 round-trip 2 (Sparse → Dense → Sparse via full `==`).
- D1c (FreePort bijectivity — UT-T15-05).

## ARG/DISC/REF citation

- AC-001 (Haskell IC.Core baseline) — sparse representation is hash-trie-based, lacks trailing-slot ambiguity.

## Determinism notes

`HashMap::==` in Rust is order-independent (compares contents, not iteration order); the test is fully deterministic. Pure synchronous; no tokio.

The KEY DIFFERENCE from T14: T14 uses `is_behaviorally_equal` because dense `Net` has the trailing-slot ambiguity (R21 §3.2 verbatim); T15 uses full `==` because SparseNet does NOT. This asymmetry is the load-bearing observation that motivates R21's two round-trip clauses.

## Cross-test dependencies

- T14 is the dense-side counterpart.
- T18 covers SparseNet serde round-trip with the same `==` strategy.
- TEST-SPEC-0489 / TEST-SPEC-0490 cover the conversion primitives.
