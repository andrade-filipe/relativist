# TEST-SPEC-T11: SparseNet construction and live agent count

**SPEC-22 §7.2 ID:** T11.
**Owning task:** TASK-0487 (SparseNet operations).
**Parent spec:** SPEC-22 §3.2 R14 (count_live_agents = `agents.len()`), R15 (O(1) complexity), R16 (no tombstones).
**Type:** unit.
**Theory anchor:** AC-001 (Haskell IC.Core baseline `Map AgentId Agent`); AC-006 (HVM2 RBag pattern, contrasting with sparse representation).

---

## Inputs / Fixtures

- Fresh `SparseNet::new()`.
- 5 agents added via `create_agent` with a mix of symbols: `[CON, DUP, ERA, CON, DUP]` ⇒ IDs 0-4.
- 2 of the 5 removed: `remove_agent(1)`, `remove_agent(3)`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T11-01 | `count_after_5_creates` | post-creates, pre-removes | `sparse.count_live_agents()` | `== 5`. |
| UT-T11-02 | `count_after_2_removes` | post-removes | `sparse.count_live_agents()` | `== 3`. |
| UT-T11-03 | `agents_hashmap_len_matches_count` | post-removes | `sparse.agents.len()` | `== 3`. (R14: count = HashMap::len.) |
| UT-T11-04 | `no_tombstones_in_agents_hashmap` | post-removes | iterate `sparse.agents`; assert no `(_, dummy_agent_marker)` entries | only entries for IDs 0, 2, 4 exist; no entries for 1 or 3. (R16.) |
| UT-T11-05 | `removed_id_no_longer_in_agents_hashmap` | post-removes | `sparse.agents.contains_key(&1)` and `sparse.agents.contains_key(&3)` | both `false`. |
| UT-T11-06 | `live_agents_iterator_yields_3` | post-removes | `sparse.live_agents().count()` | `== 3`. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Empty SparseNet (`new()` only) | `count_live_agents() == 0`; `agents.is_empty()`. |
| EC-2 | All 5 removed | `count_live_agents() == 0`; `agents.is_empty()`. (Memory is proportional to live agents — R16.) |
| EC-3 | 1000 agents created, 999 removed | `count == 1`; `agents.len() == 1`. (Stress: confirms no tombstones accumulate.) |
| EC-4 | `with_capacity(100)` then 5 creates | `count == 5`; `agents.capacity() >= 100` (capacity hint observed). |

## Invariants asserted

- R14 (operation parity with Net — count_live_agents semantics).
- R15 (O(1) complexity — empirically observable via stress; not benchmark-hard).
- R16 (no tombstones — UT-T11-04/05).

## ARG/DISC/REF citation

- AC-001 (Haskell IC.Core `Map AgentId Agent` baseline) — sparse representation matches this pattern.
- AC-006 (HVM2 RBag) — the sparse design is intentionally NOT the HVM2 pattern; this test confirms the SparseNet stays sparse.

## Determinism notes

`HashMap` iteration order in Rust is **non-deterministic** (per-process random seed via `RandomState`). UT-T11-04 and UT-T11-06 MUST NOT assume any particular iteration order. Use set-based assertions (`HashSet::from(...)` or sorted-vec comparison) when comparing iteration output. Pure synchronous; no tokio.

## Cross-test dependencies

- T12 (bidirectionality), T13 (ERA cleanliness), T17 (redex detection), T18 (serde) all build on T11's fixture pattern. Use a `fn fresh_sparse_with_5_mixed() -> SparseNet` helper.
