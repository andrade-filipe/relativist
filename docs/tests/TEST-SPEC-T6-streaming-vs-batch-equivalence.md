# TEST-SPEC-T6: Streaming vs batch equivalence

**SPEC-21 §7.2 ID:** T6.
**Owning task:** TASK-0554 + TASK-0540 + TASK-0517.
**Parent spec:** SPEC-21 §3.5 R26 (v1 backward-compat / isomorphism); §6.1 v1-to-streaming migration; §7.2 T6.
**Type:** integration.
**Theory anchor:** ARG-001 G1; ARG-002 Q7 (isomorphism).

---

## Inputs / Fixtures

- A benchmark with both `make_net` and `make_net_stream` (post-TASK-0540 default impl OR post-TASK-0541/0542 native overrides).
- The `is_behaviorally_equal` helper (SPEC-22 R21 / TEST-SPEC-0491).

## Unit Tests

| ID | Test | Then |
|----|------|------|
| UT-T6-01 | For `ep_annihilation_pure(100)`: (a) `split(make_net(100), 4) → merge` and (b) `generate_and_partition_chunked(make_net_stream, chunk_size=20, num_workers=4) → merge` | `is_behaviorally_equal(merged_a, merged_b) == true`. |
| UT-T6-02 | For `dual_tree(8)`: same comparison | `is_behaviorally_equal == true`. |
| UT-T6-03 | Short-circuit case: `chunk_size = u32::MAX` routes to `split()` | run pipeline with this config; result is byte-identical to direct `split()` invocation (no streaming overhead). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | A benchmark using the default `make_net_stream` impl (single-batch wrap) | UT-T6-01 still passes (the default impl is structurally equivalent). |
| EC-2 | Border IDs differ between paths | acceptable per TEST-SPEC-0510 cross-path note; isomorphism is structural, not bit-identical. |

## Invariants asserted

- R26 (isomorphism, not bit-identity).
- D1 (Split/Merge Identity, extended).
- G1 (full-cycle equivalence — at the partition+merge level, before reduction).

## Determinism notes

`is_behaviorally_equal` is deterministic (canonical-form comparison). Pipeline runs are single-threaded.

## Cross-test dependencies

- **TEST-SPEC-0540** (default-impl path equivalence at benchmark layer).
- **TEST-SPEC-0517** (split additive amendment + R26 short-circuit).
- **TEST-SPEC-0554** (orchestrator) — UT-0554-05/06 implement this T-test.
- **TEST-SPEC-T7** (extends to full reduction).
- **TEST-SPEC-0491** (is_behaviorally_equal helper).
