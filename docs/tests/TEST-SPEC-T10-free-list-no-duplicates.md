# TEST-SPEC-T10: Free-list no-duplicate invariant (debug assertion fires on direct manipulation)

**SPEC-22 §7.1 ID:** T10.
**Owning task:** TASK-0474 (R6 no-duplicates closure + optional HashSet shadow).
**Parent spec:** SPEC-22 §3.1 R6; SC-018 closure.
**Type:** unit (debug-only assertion fence).
**Theory anchor:** None direct; defensive correctness.

---

## Inputs / Fixtures

- Fresh `Net::new()`.
- Test-only API or `#[cfg(test)]` access to the private `Net.free_list` field, OR an `unsafe`-free helper that constructs a `Net` with a pre-populated `free_list` for the synthetic violation. (TASK-0474's design.)

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T10-01 | `direct_duplicate_push_triggers_debug_assert` (debug-only) | `let mut net = ...; net.free_list.push(7)` (test-only access) | `net.remove_agent(7_with_synthetic_setup)` so that `remove_agent` runs the `debug_assert!(!self.free_list.contains(&id))` immediately before push | `panic::catch_unwind` captures the panic; payload contains "free-list" or the assertion text. (Ensures R6 is enforced at the push site.) |
| UT-T10-02 | `release_build_compiles_assertion_out` (release-only) | `cargo test --release` build of UT-T10-01 | run | `debug_assert!` is dead-code-eliminated; the duplicate push silently succeeds. (Documents the `debug_assertions`-only enforcement; R6 is a debug-mode-enforced invariant per §3.1 R6.) |
| UT-T10-03 | `pop_returns_most_recently_pushed` | direct `net.free_list.push(7); net.free_list.push(11)` | `let v = net.free_list.pop()` | `v == Some(11)`. (LIFO contract smoke check; cited in TASK-0474 acceptance criteria.) |
| UT-T10-04 | `shadow_consistency_after_push_pop_cycle` (CONDITIONAL — only if `free_list_shadow: HashSet<AgentId>` was adopted by DEVELOPER per TASK-0474 OPTIONAL acceptance criterion) | net with shadow active; push 5 distinct IDs, pop them all | shadow is empty post-drain AND len matches Vec at every intermediate state | confirmed. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | The `free_list_shadow` (if adopted) goes out of sync with the `Vec<AgentId>` | A `debug_assert!(self.free_list_shadow.len() == self.free_list.len())` at every push/pop site catches the desync immediately. |
| EC-2 | A duplicate is pushed via two separate code paths (e.g., a buggy `remove_agent` is called twice on the same ID without an intervening `create_agent`) | Either: (a) the second `remove_agent` is a no-op because `agents[id]` is already `None` (existing SPEC-02 R12 guard); OR (b) if guard 'a' is missing, the duplicate push triggers the debug assert. Test EC-2 specifically verifies guard 'a' is in place at SPEC-02 R12. |

## Invariants asserted

- R6 (free-list MUST NOT contain duplicates) — closes SC-018.
- R5 (LIFO ordering — UT-T10-03).

## ARG/DISC/REF citation

- None direct.

## Determinism notes

UT-T10-01 uses `panic::catch_unwind` to assert that the panic fires; the test is deterministic in debug builds. UT-T10-02 is release-only and demonstrates the dead-code-elimination behavior — both must be present and correctly cfg-gated. Pure synchronous; no tokio.

## Cross-test dependencies

- TEST-SPEC-0474 covers the same R6 primitive at the plumbing level; T10 is the spec-catalog mirror.
- The OPTIONAL `free_list_shadow` adoption is a DEVELOPER decision at Stage 3 per TASK-0474 notes; UT-T10-04 must be written conditionally (`#[cfg(feature = "free_list_shadow")]` or similar gate as agreed at impl time).
