# TEST-SPEC-0516: SPEC-19 BorderGraph extend amendment (extend_with_chunk_borders method)

**SPEC-21 §7 ID:** plumbing only (gates T6 / T7 under delta+streaming).
**Owning task:** TASK-0516.
**Parent spec:** SPEC-21 §3.7 R37f; §3.8 A7; SC-017 closure.
**Type:** unit + integration (cross-spec dependency on SPEC-19).
**Theory anchor:** ARG-005 (delta border completeness — extends to cross-chunk discovery); ARG-001 G1.

---

## Inputs / Fixtures

- A `BorderGraph` instance pre-populated with a baseline border set (per SPEC-19 §4 fixture).
- A `Vec<BorderEntry>` representing borders discovered in chunk N+1.
- The `GridConfig { delta_mode: true, dispatch_mode: DispatchMode::Push, chunk_size: 16, .. }` fixture activating the conjunction `delta_mode && streaming_active`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0516-01 | `extend_with_chunk_borders_method_present` | the amended `BorderGraph` impl | grep for `pub fn extend_with_chunk_borders(&mut self, new_borders: &[BorderEntry])` (or the exact signature §3.8 A7 ratifies) | method present with the documented signature. |
| UT-0516-02 | `extend_appends_new_borders` | a 2-entry baseline; a 3-entry new_borders slice | invoke `extend_with_chunk_borders(new)` | post-call: graph has 5 entries; pre-call entries unchanged. |
| UT-0516-03 | `extend_idempotent_on_duplicate_entries` | a baseline that already contains one of the new_borders entries | invoke `extend_with_chunk_borders` | the duplicate is NOT double-counted (insert-or-no-op semantics); test asserts final count = baseline + (new \ baseline). |
| UT-0516-04 | `extend_does_not_lose_baseline_entries` | baseline | extend with empty slice | post-call: graph identical to baseline (no spurious mutations). |
| UT-0516-05 | `r37f_call_site_after_install_connection_yielding_border` | the chunked pipeline orchestrator with `delta_mode = true, streaming_active = true`; chunk N processed; chunk N+1 dispatch pending | inspect call ordering: `install_connection` returning a border wire is followed (within the same orchestrator tick) by `BorderGraph::extend_with_chunk_borders(&new_borders)` BEFORE the next `AssignPartition` is sent | call-ordering MUST hold (R37f). The test exercises this via call-site instrumentation (e.g., a mock `BorderGraph` that records call order). |
| UT-0516-06 | `r37f_no_op_under_non_delta_streaming` | `delta_mode = false, streaming_active = true` | run pipeline | `extend_with_chunk_borders` is NOT invoked (R37f conjunction-gate; SHOULD elevation only fires under `delta_mode && streaming_active`). |
| UT-0516-07 | `r37f_no_op_under_delta_non_streaming` | `delta_mode = true, streaming_active = false` (chunk_size = u32::MAX short-circuit) | run pipeline | `extend_with_chunk_borders` is NOT invoked; standard SPEC-19 BorderGraph maintenance handles this case. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `extend_with_chunk_borders` called with an empty new_borders slice during an active streaming run | no-op; no allocation; no mutation. |
| EC-2 | A border entry appears in chunk N+1 that is structurally equivalent to but NOT identical to a chunk N entry (different `AgentId` re-binding) | inserted as a NEW entry (BorderGraph keys on full `(AgentId, PortId)` tuples; structural equivalence at higher levels is decided by D2 cross-round logic, not by the `extend` method). |
| EC-3 | A worker crashes mid-stream and a chunk is re-dispatched | the second `extend_with_chunk_borders` call MUST be idempotent on the chunk's borders (UT-0516-03 covers this scenario). |

## Invariants asserted

- R37f (BorderGraph update under delta+streaming — SHOULD→MUST elevation; closes SC-017).
- §3.8 A7 (SPEC-19 BorderGraph extension method).
- D2 (Border completeness) — preserved across chunks via incremental BorderGraph extension.
- D3 (Cross-round border discovery) — extended to cross-chunk discovery.
- G1 (under delta + streaming) — preserved via call-site discipline.

## ARG/DISC/REF citation

- ARG-005 (delta border completeness — extends to cross-chunk discovery; this amendment is the structural enabler for ARG-005's cross-chunk extension).
- ARG-001 G1 (preserved under the conjunction).

## Determinism notes

UT-0516-05 (call-site ordering verification) MUST use `#[tokio::test(flavor = "current_thread")]` with a deterministic single-runtime executor. The orchestrator is single-tick BSP; the call-order discipline is BSP-tick-bound (no wall-clock dependencies). Tests MUST NOT use `tokio::time::sleep` to enforce ordering; use explicit `await`-pointed sequencing.

## Cross-test dependencies

- TEST-SPEC-0588 (call-site discipline behavioral test) — forward-referenced from TASK-0516 but NOT in scope for the current Stage 2 wave (TASK-0588 not yet authored). Flagged as Stage-2 wave-2 dependency.
- TEST-SPEC-T6 / T7 (streaming-vs-batch isomorphism / end-to-end reduction equivalence under streaming) — exercise the full delta+streaming combination via this amendment.
- M5 milestone gate (`ep_con 100M coordinator-side`) — out of scope for SPEC-21 but enabled by this amendment.
