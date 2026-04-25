# TEST-SPEC-T5: Reduction with recycling (CON-CON annihilation × 100)

**SPEC-22 §7.1 ID:** T5.
**Owning task:** TASK-0473 (remove_agent push site exercised by SPEC-03 annihilation rule).
**Parent spec:** SPEC-22 §3.1 R2, R11 (count_live_agents excludes free-list); §3.3 R24 (I3' uniqueness).
**Type:** integration (drives reduction over a hand-built net).
**Theory anchor:** REF-002 Theorem 1 (universality of γ/δ/ε); AC-006 (HVM2 arena recycling rationale).

---

## Inputs / Fixtures

- A `Net` built with 100 CON-CON annihilation pairs (200 agents total, IDs 0-199).
  - For each `i in 0..100`: `let a = net.create_agent(CON); let b = net.create_agent(CON); net.connect(AgentPort(a, 0), AgentPort(b, 0));`
  - This produces 100 active pairs in `redex_queue`.
- Pre-state: `agents.len() == 200`, all `Some(CON)`. `free_list == []`. `next_id == 200`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T5-01 | `next_id_unchanged_after_pure_annihilation` | net at pre-state | `net.reduce_all()` | `net.next_id == 200`. CON-CON annihilation is a -2 rule (consumes 2 agents, creates 0); no `create_agent` call fires; `next_id` cannot increment. |
| UT-T5-02 | `free_list_holds_all_annihilated_ids` | same | same | `net.free_list.len() == 200`. Every agent was consumed and its ID pushed. |
| UT-T5-03 | `count_live_agents_is_zero_post_reduction` | same | same | `net.count_live_agents() == 0` (R11: free-list does not count as live). |
| UT-T5-04 | `agents_vec_full_of_none` | same | same | `net.agents.iter().all(\|s\| s.is_none()) == true`. |
| UT-T5-05 | `free_list_no_duplicates_post_reduction` | same | same | sort `net.free_list.clone()`; assert no consecutive equal IDs (R6 invariant; full coverage in T10). |
| UT-T5-06 | `is_reduced_after_full_drain` | same | same | `net.is_reduced() == true` (redex queue is empty). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Single CON-CON pair (smallest non-trivial) | `next_id == 2`, free_list len 2, count_live == 0. Confirms the rule fires and pushes. |
| EC-2 | 100 CON-CON pairs interleaved with 1 CON-DUP pair (commutation) | `next_id > 200` (CON-DUP is +2 net agents per fire; 1 fire ⇒ `next_id` increases by `f = k - r` per §3.8 A3, with some recycled IDs). Free-list balance reflects net 198 frees (200 - 2) plus 4 creates (2 recycled, 2 fresh). Use T6 for the strict CON-DUP assertions; here the interleave just smoke-checks. |
| EC-3 | After reduce_all, manually re-create 50 agents | First 50 reuse from free-list (LIFO); `next_id` unchanged at 200. Free-list shrinks to 150. |

## Invariants asserted

- I3' (uniqueness across all 200 IDs in free-list — preserved by R6).
- R2 (every annihilated agent's ID pushed).
- R11 (count_live_agents excludes free-list).
- R28 (always-on default — reduction reuses free-list without opt-in).

## ARG/DISC/REF citation

- REF-002 Theorem 1 — annihilation rule γγ → empty (CON-CON in this naming) is a fundamental IC reduction rule.
- AC-006 (HVM2 arena management) — the "consumed slot returns to arena" pattern mirrors HVM2's free-list-of-tags.

## Determinism notes

CON-CON annihilation is order-independent (the 100 pairs are non-overlapping). Reduction strategy may visit them in any order; the post-state assertions are order-invariant (count, set membership). No tokio, no async — pure single-threaded `reduce_all`.

## Cross-test dependencies

- Builds on T1, T2, T3 helpers.
- T6 covers the CON-DUP commutation case (the +2 net rule); together they cover both branches of the rule mix.
