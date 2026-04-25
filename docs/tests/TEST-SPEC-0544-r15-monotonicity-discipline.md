# TEST-SPEC-0544: R15 monotonicity discipline (generator-layer vs arena-layer reconciliation)

**SPEC-21 §7 ID:** plumbing (R15 enforcement; gates I3' reconciliation under streaming).
**Owning task:** TASK-0544.
**Parent spec:** SPEC-21 §3.5 R15; §3.5 closing note (post-dispatch monotonicity-forbidden); SPEC-22 R23 (I3' uniqueness, post-arena).
**Type:** unit + CI lint.
**Theory anchor:** ARG-001 P4 (ID consistency); ARG-002 Q5/C1 (complete coverage).

---

## Inputs / Fixtures

- A "good" generator: `ep_annihilation_stream(20, 4)` — R15-honoring (monotone within and across batches).
- A "good" generator: `dual_tree_stream(8, 4)` — R15-honoring.
- A synthetic "bad" generator: `MalformedMonotonicGenerator` that emits a batch with `(min_id, max_id) = (10, 20)` followed by a batch with `(min_id, max_id) = (15, 25)` — R15 violation (overlap; max_k=20 not strictly less than min_(k+1)=15... actually min_(k+1)=15 < max_k=20, the violation is overlap).
- A SPEC-22 free-list-recycle scenario: a worker arena that has reclaimed IDs from a prior round and is now consuming a new chunk; the chunk's IDs are monotone within itself but NOT necessarily greater than all previously-recycled-and-now-free IDs.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0544-01 | `r15_violation_triggers_debug_assertion` | the malformed generator + a debug-build pipeline | run streaming pipeline with `cfg!(debug_assertions)` | the pipeline panics with assertion message containing "R15" or "monotonicity violation" or equivalent anchor. |
| UT-0544-02 | `r15_honored_no_assertion_ep_annihilation` | the good ep_annihilation generator + debug-build pipeline | run | no panic; pipeline completes successfully. |
| UT-0544-03 | `r15_honored_no_assertion_dual_tree` | the good dual_tree generator + debug-build pipeline | run | no panic; pipeline completes. |
| UT-0544-04 | `r15_release_build_no_assertion` | the malformed generator + a release build (no `debug_assertions`) | run | the pipeline does NOT panic at the R15 site; downstream behavior is undefined per discipline (caller's responsibility). The test asserts the panic does NOT fire in release. |
| UT-0544-05 | `arena_layer_recycle_does_not_trip_r15` | the SPEC-22 recycle fixture + a chunk monotone within itself | run pipeline | NO assertion fires. The assertion checks generator-layer monotonicity ONLY (per-chunk), NOT arena-layer (across-run) monotonicity. (See Determinism notes.) |
| UT-0544-06 | `r15_assertion_message_distinguishes_layer` | UT-0544-01 panic | inspect the panic message | message MUST mention "generator-phase" or "within-chunk" — NOT "arena-layer" or "run-wide". This anchors the I3'/R15 reconciliation discipline in the assertion text. |
| UT-0544-07 | `ci_lint_forbids_post_dispatch_monotonicity_assertions` | a synthetic test source file under `src/protocol/**` or `src/merge/**` containing the pattern `assert!(id > previous_id)` (or equivalent post-dispatch monotonicity check) | run the SPEC-21 §3.5 closing-note CI lint | the lint flags the file (forbidden pattern under streaming-context modules). |
| UT-0544-08 | `i3_prime_uniqueness_under_recycle` | the SPEC-22 recycle fixture run end-to-end | post-merge: extract all AgentIds across the merged net | every `AgentId` is unique (I3' from SPEC-22 R23/R24/R25); no monotonicity expected. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | A batch with `agents.len() = 0` between two non-empty batches | UT-0544-02 honors monotonicity trivially; the empty batch's max is `None`; the assertion treats `None` as "skip". |
| EC-2 | Overflow: `max_id_in_batch_k = u32::MAX`, next batch starts at... | impossible by R15 discipline; if attempted, the assertion fires (via UT-0544-01-style detection). |
| EC-3 | A custom strategy implementer who claims to "preserve R15 across run" via re-sequencing the IDs post-dispatch | UT-0544-07 CI lint catches this; it is forbidden. |

## Invariants asserted

- R15 (generator-phase monotonicity) — enforced via debug assertion.
- I3' (Uniqueness of AgentIds, post-SPEC-22) — preserved trivially under R15 within-chunk; preserved across run under SPEC-22 R23 even when recycling occurs.
- §3.5 closing-note discipline (post-dispatch monotonicity-forbidden) — enforced via CI lint.

## ARG/DISC/REF citation

- ARG-001 P4 (ID consistency).
- ARG-002 Q5/C1 (complete coverage).

## Determinism notes

**R15 vs I3' RECONCILIATION (CRITICAL):** This is the test that explicitly distinguishes the two layers per the user brief and SPEC-21 §3.5 R15 prose:

- **Generator-layer (R15):** Batch IDs strictly monotone WITHIN a single producer chunk AND across consecutive chunks of the same producer. This is a property of the generator output BEFORE arena consumption.
- **Arena-layer (I3' from SPEC-22):** IDs unique across the run, NOT necessarily monotone after free-list recycling. The arena reclaims IDs and reuses them; uniqueness is preserved per SPEC-22 R23, but monotonicity is NOT.

UT-0544-05 + UT-0544-08 jointly cover the compatibility case: a fixture where R15 holds within-chunk AND I3' holds across-run with recycling. UT-0544-07 covers the negative case: code that misinterprets R15 as run-level (forbidden).

The CI lint (UT-0544-07) is implemented as a `Grep`/`rg` pattern over `src/protocol/**` and `src/merge/**` after Phase D ships. The pattern MUST distinguish documented assertion sites (allowed) from naive monotonicity checks (forbidden); the discrimination uses comment anchors (e.g., `// SAFETY: post-dispatch generator-layer R15 only` is allowed; bare `assert!(id > previous_id)` is forbidden).

UT-0544-04 (release-build behavior) requires running the test under `cargo test --release`. CI MUST exercise both build configs.

## Cross-test dependencies

- TEST-SPEC-0541, TEST-SPEC-0542 (good generators) — produce R15-honoring streams; UT-0544-02/03 use them as positive fixtures.
- TEST-SPEC-0521 (AgentBatch struct) — type-level monotonicity is NOT enforced; this TEST-SPEC enforces it at the discipline level.
- TEST-SPEC-0495 (SPEC-22 I3' debug assertions) — sibling test for the arena-layer uniqueness invariant; this TEST-SPEC complements it on the generator-layer side.
- TEST-SPEC-T6 / T7 (full pipeline equivalence) — implicitly depends on R15 holding in production fixtures.
