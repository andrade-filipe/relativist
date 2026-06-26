# TEST-SPEC-0477: `count_live_agents` excludes free-list entries (R11)

**SPEC-22 §7 ID:** none direct (audit + test-coverage task); plus this plumbing file.
**Owning task:** TASK-0477.
**Parent spec:** SPEC-22 §3.1 R11; SPEC-02 R16a (existing `count_live_agents` semantics).
**Type:** unit.

---

## Inputs / Fixtures

- Fresh `Net::new()` with various live + free-list states.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0477-01 | `count_live_excludes_free_list_entries` | 10 agents created (IDs 0-9), 5 removed (IDs 0, 2, 4, 6, 8) | `net.count_live_agents()` | `== 5`. AND `net.free_list.len() == 5`. (Free-list and live count are independent.) |
| UT-0477-02 | `count_live_zero_after_full_removal_with_free_list` | 10 agents created, all 10 removed | `net.count_live_agents()` | `== 0`. AND `net.free_list.len() == 10`. |
| UT-0477-03 | `count_live_unaffected_by_free_list_growth` | 5 live agents; pre-state count == 5; then artificial free-list push of an unrelated ID (test-only synthetic state, ID corresponds to an unused arena slot) | `net.count_live_agents()` | `== 5` (unchanged). |
| UT-0477-04 | `count_live_increments_on_create_with_recycle` | net with 5 live + 3 free-list entries; create 1 agent | `net.count_live_agents()` | `== 6`. AND `net.free_list.len() == 2` (one popped). |
| UT-0477-05 | `count_live_implementation_uses_flatten` | the function body | grep / inspect | uses `agents.iter().flatten().count()` or equivalent (skips `None` slots). The Rustdoc cites SPEC-22 R11. |
| UT-0477-06 | `count_live_after_reduce_all_zero_in_pure_annihilation_net` | 100 CON-CON pairs (T5 fixture); `reduce_all` | `net.count_live_agents()` | `== 0`. AND `free_list.len() == 200`. (Joint with T5.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Empty net | `count_live_agents() == 0`. |
| EC-2 | Net with `agents.len() == 100` but every slot is `None` and free-list is empty (backward-compat with v2-baseline pre-SPEC-22 nets) | `count_live_agents() == 0`. (R11 + R9 backward-compat: pre-existing `None` slots that aren't in the free-list are still excluded from count.) |
| EC-3 | Stress: 1M agents created, 999_999 removed | `count_live_agents() == 1`. Performance: O(n) iteration; should complete in << 1 second. |

## Invariants asserted

- R11 (`count_live_agents` excludes free-list).
- Compatibility with TASK-0231 (existing implementation per SPEC-02 R16a).

## ARG/DISC/REF citation

- None direct.

## Determinism notes

Pure synchronous; deterministic iteration over `agents.iter().flatten()`. No tokio.

## Cross-test dependencies

- T5 / T7 / T16 implicitly rely on this primitive.
- This task is mostly an audit + test-coverage task per TASK-0477 notes.
