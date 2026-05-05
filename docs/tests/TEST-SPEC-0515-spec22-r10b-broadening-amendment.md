# TEST-SPEC-0515: SPEC-22 R10b broadening amendment (free-list × streaming protected-tombstone discipline)

**SPEC-21 §7 ID:** plumbing only (gates G1 closure under streaming).
**Owning task:** TASK-0515.
**Parent spec:** SPEC-21 §3.7 R37b; §3.8 A6; SC-007 closure.
**Type:** unit + integration (cross-spec dependency on SPEC-22 fixtures).
**Theory anchor:** ARG-001 G1 (global determinism); ARG-005 P7/P8 (delta border completeness).

---

## Inputs / Fixtures

- **Canonical SPEC-22 R10b fixture (reused, NOT duplicated):** "worker 0 owns id_range `[0, 100)`, border at `AgentPort(47, 0)`" — sourced from TEST-SPEC-0482 and the T9a/T9b fixture helpers.
- **NEW SPEC-21 extension:** the canonical fixture extended with a streaming chunk that produces a NEW border reference at slot 47 mid-stream (i.e., a fresh `Pending → Resolved` connection that creates a coordinator border map entry for `AgentId(47)` after the recycle window has already completed for that ID once).
- The `streaming_no_recycle` cargo feature gate, exercised in BOTH states (gate ON disables free-list pop; gate OFF allows pop with R10b protections).
- Two `RecyclePolicy` instances: `DisableUnderDelta` (Strategy A, default) and `BorderClean` (Strategy B, opt-in).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0515-01 | `r10b_text_broadens_to_streaming_or_delta` | the amended SPEC-22 R10b text | grep for "delta_mode || streaming_active" or equivalent disjunction-broadening clause | substring present. The amendment MUST NOT keep the old "delta_mode == true" sole-trigger language. |
| UT-0515-02 | `streaming_active_strategy_a_no_pop_during_chunk` | the canonical fixture; `RecyclePolicy::DisableUnderDelta`; streaming pipeline mid-chunk | invoke `worker.create_agent(...)` requiring a fresh ID | `create_agent` MUST allocate a fresh ID via `id_range_counter` increment, NOT pop from the free-list. (Strategy A behavior under streaming.) |
| UT-0515-03 | `streaming_active_strategy_b_borderclean_pop` | canonical fixture; `RecyclePolicy::BorderClean`; the recycled ID `47` IS NOT in the worker's `border_entries` set | `worker.create_agent(...)` | MAY pop from free-list (Strategy B's selective-recycle behavior). |
| UT-0515-04 | `streaming_active_strategy_b_protected_tombstone` | canonical fixture; `RecyclePolicy::BorderClean`; the recycled ID `47` IS in `border_entries` | `worker.create_agent(...)` | MUST NOT pop ID `47` (protected tombstone honored). MAY pop a different ID. |
| UT-0515-05 | `streaming_no_recycle_gate_on_disables_pop` | `streaming_no_recycle` cargo feature ENABLED; canonical fixture extended | `worker.create_agent(...)` during streaming | always allocates fresh ID; `free_list.pop()` is unreachable. (Gate-ON behavior.) |
| UT-0515-06 | `streaming_no_recycle_gate_off_allows_pop_with_r10b_protections` | `streaming_no_recycle` cargo feature DISABLED; canonical fixture extended | `worker.create_agent(...)` during streaming with `RecyclePolicy::DisableUnderDelta` | follows UT-0515-02 (no pop while delta+streaming active); equivalent to gate-ON when both delta AND streaming flags are set; gate-OFF still falls back to R10b discipline. |
| UT-0515-07 | `g1_preserved_strategy_a_streaming` | full pipeline: `dual_tree(8)` with chunk_size=4, `RecyclePolicy::DisableUnderDelta` | run streaming pipeline + merge | result `nets_isomorphic` to `reduce_all(make_net(dual_tree(8)))`. (G1 preserved.) |
| UT-0515-08 | `g1_preserved_strategy_b_streaming` | same as UT-0515-07 with `RecyclePolicy::BorderClean` | run + merge | same isomorphism property. |
| UT-0515-09 | `mid_stream_border_does_not_corrupt_protected_tombstones` | the canonical fixture extended with the SPEC-21 mid-stream border at slot 47 | run pipeline; observe whether `border_entries` is updated after the new border resolves | `border_entries` MUST include `AgentId(47)` BEFORE the next `create_agent` request can pop it (call-site ordering: `install_connection` updates `border_entries` first; `create_agent` checks second). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Both `delta_mode = false` AND `streaming_active = false` | R10b discipline does NOT engage (predecessor v1 behavior); free-list pop is unconditionally allowed. |
| EC-2 | `delta_mode = true` AND `streaming_active = false` | classic SPEC-22 R10b behavior; coverage in TEST-SPEC-0482 / T9a / T9b (PRECEDENT, unchanged). |
| EC-3 | `delta_mode = true` AND `streaming_active = true` | combined regime; Strategy A disables pop unconditionally; Strategy B applies BOTH border-entries protection layers (delta-side + streaming-side). |
| EC-4 | Mid-stream border resolution races with a `create_agent` request on the same ID | call-site ordering MUST be enforced (install_connection completes before create_agent is dispatched in the same worker tick); the test exercises this with a controlled tokio runtime. |

## Invariants asserted

- R37b (G1 free-list interaction under streaming — closes SC-007).
- §3.8 A6 (SPEC-22 R10b broadening + cargo-feature-gate alternative documented).
- G1 — preserved by broadened recycle protection.
- D2 / D3 (border completeness, cross-round border discovery) — preserved across chunks.

## ARG/DISC/REF citation

- ARG-001 G1.
- ARG-005 P7/P8 (delta border completeness — extended to streaming via the broadening amendment).

## Determinism notes

The mid-stream border resolution path (UT-0515-09 / EC-4) is only deterministic under a single-threaded controlled runtime: `#[tokio::test(flavor = "current_thread")]`. Multi-thread runtimes would introduce an `install_connection` vs `create_agent` race; the spec's call-site ordering discipline is BSP-tick-bound (R37d) and is NOT a wall-clock guarantee. Tests MUST NOT use `tokio::time::sleep` to enforce ordering; use explicit `await`-pointed sequencing.

The cargo feature gate (`streaming_no_recycle`) is a BUILD-TIME configuration; UT-0515-05 / UT-0515-06 require running the test suite under both gate states. CI MUST exercise both configs:
- `cargo test --features streaming_no_recycle` (gate ON)
- `cargo test` (gate OFF, default)

## Cross-test dependencies

- **SPEC-22 fixture reuse (mandatory):** TEST-SPEC-0482 (RecyclePolicy + protected tombstones) is the canonical-fixture source. This TEST-SPEC EXTENDS that fixture with a SPEC-21-specific mid-stream border emergence; do NOT duplicate the SPEC-22 fixture body.
- TEST-SPEC-T9a (BorderGraph Strategy A) — sibling for delta-only path (PRECEDENT).
- TEST-SPEC-T9b (BorderGraph Strategy B) — sibling for delta-only path (PRECEDENT).
- TEST-SPEC-0589, TEST-SPEC-0590, TEST-SPEC-0591 — forward-referenced from TASK-0515 but NOT in scope for the current Stage 2 wave (TASKs 0589-0591 not yet authored). Flagged as Stage-2 wave-2 dependencies.
