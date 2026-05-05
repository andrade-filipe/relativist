# TEST-SPEC-T3: Free-list exhaustion (recycle then fall through to fresh allocation)

**SPEC-22 §7.1 ID:** T3.
**Owning task:** TASK-0472 (create_agent — both branches: pop and fresh).
**Parent spec:** SPEC-22 §3.1 R3 (free-list-pop OR next_id increment), R4 (slot reuse), R10 amendment §3.8 A3 (`f = k - r` accounting).
**Type:** unit.
**Theory anchor:** AC-009 / AC-011 (HVM4 bump allocation under static heap partitioning — informs the fresh-allocation fall-through path).

---

## Inputs / Fixtures

- Fresh `Net::new()`.
- Pre-state (post 3 creates + 3 removes):
  - `agents = [None, None, None]`.
  - `free_list = [0, 1, 2]` in some push-order-determined permutation (assume create-then-remove of IDs 0, 1, 2 in order: free-list ends up `[0, 1, 2]`).
  - `next_id = 3`.
- Expected post-state after 5 successive `create_agent(Symbol::DUP)`:
  - First 3 IDs come from the free-list (LIFO popped: `[2, 1, 0]`).
  - Last 2 IDs come from `next_id` increment: `[3, 4]`.
  - Final `agents.len() == 5`, `next_id == 5`, `free_list = []`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T3-01 | `first_three_creates_recycle_lifo` | net at pre-state | `let ids = (0..3).map(\|_\| net.create_agent(DUP)).collect::<Vec<_>>()` | `ids == vec![2, 1, 0]` (LIFO). |
| UT-T3-02 | `next_two_creates_allocate_fresh` | continued | `let ids2 = (0..2).map(\|_\| net.create_agent(DUP)).collect::<Vec<_>>()` | `ids2 == vec![3, 4]` (fresh, monotonic). |
| UT-T3-03 | `next_id_increments_only_on_fresh` | full sequence | check `net.next_id` | `net.next_id == 5` — exactly `f = k - r = 5 - 3 = 2` increments (per §3.8 A3 amended R10). |
| UT-T3-04 | `arena_grows_only_on_fresh` | full sequence | `let len = net.agents.len()` | `len == 5` (the 2 fresh allocations grew the arena from 3 → 5; the 3 recycles did not). |
| UT-T3-05 | `free_list_drained_to_empty` | full sequence | check `net.free_list` | `net.free_list.is_empty()`. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Free-list has more entries than the number of creates: 5 entries, 3 creates | First 3 IDs are LIFO from free-list; remaining 2 entries stay in free-list. `next_id` unchanged. |
| EC-2 | Free-list empty from the start; 5 creates | All 5 IDs are fresh: `[0, 1, 2, 3, 4]`. `next_id = 5`. (Confirms the fresh-only path remains unchanged from SPEC-02 R11.) |
| EC-3 | Mixed interleave: create, remove, create, create — free-list has 1 entry; 2 creates | First create recycles (1 ID popped); second create allocates fresh (`next_id` increment). |

## Invariants asserted

- R3 (free-list-pop OR next_id increment — not both for any single create).
- R4 (slot reuse semantics on recycle path; arena expansion only on fresh path).
- §3.8 A3 amended R10: `f = k - r` accounting (verified by UT-T3-03).
- I3' (uniqueness across the 5 returned IDs — all distinct).

## ARG/DISC/REF citation

- AC-009 (HVM4 bump allocation) — fresh-allocation path mirrors HVM4's `next_id += 1` when no recycled term is available.
- AC-011 (HVM4 static heap partitioning) — under partitioning (R10), the fresh-vs-recycle decision is partition-local.

## Determinism notes

Pure synchronous; deterministic LIFO + monotonic `next_id` increment. No async, no tokio.

## Cross-test dependencies

- Builds on T1's helper.
- Together with T1 + T2, defines the full driver-level free-list contract (recycle reuses, exhaustion falls through, LIFO order).
