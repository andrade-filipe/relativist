# TEST-SPEC-0497: SPEC-03 reduction-engine assertion audit (R27a — closes SC-010)

**SPEC-22 §7 ID:** T7a (spec-catalog) plus this plumbing file.
**Owning task:** TASK-0497.
**Parent spec:** SPEC-22 §3.3 R27a; §3.8 A6.
**Type:** unit + integration + meta-test (optional grep).

---

## Inputs / Fixtures

- The post-audit `src/reduction/` files.
- A test fixture: CON-DUP active pair under a partial free-list (T7a fixture).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0497-01 | `t7a_release_build_passes` (release-only) | T7a fixture | `cargo test --release` runs the rule with the partial free-list | reduction completes; no `assert!(new_id > old_max_id)` fires; observable post-state matches T7a UT-T7a-01..04. |
| UT-0497-02 | `t7a_debug_build_passes` (debug-only) | same | `cargo test` (debug) | reduction completes; only allowed assertion patterns execute and pass. |
| UT-0497-03 | `audit_no_forbidden_assertion_patterns_in_reduction` (OPTIONAL meta-test) | grep `src/reduction/**/*.rs` at test time | search for forbidden patterns: `assert!(new_id > old_max_id)`, `assert!(new_id == self.next_id - 1)`, `debug_assert!(new_id > self.next_id - 1)`, any monotonicity claim | no matches found. (Marked OPTIONAL per TASK-0497 — DEVELOPER decides whether to include this meta-assertion.) |
| UT-0497-04 | `assert_next_id_valid_preserved` | net with non-empty free-list and `next_id` | the existing `assert_next_id_valid` (SPEC-02 §4.5) helper | passes; the check `(i as u32) < self.next_id` for `slot.is_some()` is consistent with I3'. |
| UT-0497-05 | `condup_4_creates_no_inter_call_monotonicity_assert` | targeted: `interact_comm` (CON-DUP) | inspect the function source | between the 4 `create_agent` calls, no assertion compares `new_id` to a previously-returned ID. (Audit confirmation.) |
| UT-0497-06 | `audit_comment_at_top_of_modified_files` | each file in the audit list (`anni.rs`, `comm.rs`, `eras.rs`, `void.rs`, `step.rs`) | grep | each contains a comment "SPEC-22 R27a audit: <count> forbidden patterns replaced; allowed patterns retained." (per TASK-0497 acceptance). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Audit finds no forbidden patterns to begin with | The task ships with the T7a test as evidence; the comment cites "0 forbidden patterns replaced". |
| EC-2 | A future PR introduces a forbidden pattern | UT-0497-03 (if implemented) catches at test time. CI lint (if implemented at the cicd agent's discretion) catches at PR time. |
| EC-3 | A subtle monotonicity claim hidden in a macro expansion | The audit must follow macro expansions; `cargo expand` may help. Documented as a Stage 3 DEVELOPER chore. |

## Invariants asserted

- R27a (SPEC-03 in-rule assertion language is I3'-compatible).
- §3.8 A6 (SPEC-03 §4.3 amendment).
- I3' uniqueness preserved across CON-DUP firing.

## ARG/DISC/REF citation

- REF-002 (γδ commutation).
- AC-006 (HVM2 +2 commutation rationale).

## Determinism notes

UT-0497-01 / UT-0497-02 must both pass — the test is run twice (once in release, once in debug) per TASK-0497 acceptance. Pure synchronous; no tokio. The optional grep meta-test runs at test-time and is deterministic.

## Cross-test dependencies

- T7a (spec-catalog) is the integration mirror; this plumbing test covers the audit closure.
- TEST-SPEC-0465 is the SPEC-03 amendment side; A6 is recorded in SPEC-22 §3.8.
