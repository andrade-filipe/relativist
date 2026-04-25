# TEST-SPEC-T5: Streaming pipeline produces valid partitions (C1/C2/C3)

**SPEC-21 §7.1 ID:** T5.
**Owning task:** TASK-0554.
**Parent spec:** SPEC-21 §3.3 R17, R18; §3.5 R28; §7.1 T5; SPEC-04 §4.8 C1-C3 assertions.
**Type:** integration.
**Theory anchor:** ARG-002 Q5/C1-C3 (split/merge identity).

---

## Inputs / Fixtures

- `ep_annihilation_stream(100, chunk_size=20)` with `num_workers=4`.
- The SPEC-04 §4.8 `assert_coverage_and_disjunction` and `assert_border_consistency` helpers.

## Unit Tests

| ID | Test | Then |
|----|------|------|
| UT-T5-01 | Run `generate_and_partition_chunked(ep_annihilation_stream(100), chunk_size=20, num_workers=4)` | `Ok(ChunkedPartitionResult)`. |
| UT-T5-02 | C1: all 200 agents (ep_annihilation produces 2 × size = 200) are present across partitions | sum of `partitions[i].subnet.agents.len()` (live) `== 200`; no agent appears in two partitions. |
| UT-T5-03 | C2: all 100 wires are present (internal or border) | `internal_wires + border_wires == 100`. |
| UT-T5-04 | C3: each border ID appears in exactly 2 partitions | for each `bid` in result: `partitions.iter().filter(|p| p.has_freeport(bid)).count() == 2`. |
| UT-T5-05 | The §4.8 SPEC-04 assertions pass when invoked on the finalized `Vec<Partition>` | `assert_coverage_and_disjunction(&partitions)` does not panic; `assert_border_consistency(&partitions)` does not panic. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `num_workers = 1` (no border wires possible) | UT-T5-04 trivially holds (0 borders); UT-T5-02/03 still apply. |
| EC-2 | `chunk_size = 1` | many small chunks; correctness unchanged; performance worse. |
| EC-3 | A generator that produces 0 wires (only standalone agents) | UT-T5-03 expects 0; UT-T5-04 trivially holds. |

## Invariants asserted

- R17, R18 (function and processing sequence).
- R28 (debug-mode C1-C3 assertions).
- C1, C2, C3 (full coverage).

## Determinism notes

Pipeline is single-threaded; `#[test]` is sufficient (no async). Use `cfg!(debug_assertions)` to enable the §4.8 assertions; the test MUST run in debug builds.

## Cross-test dependencies

- **TEST-SPEC-0554** (orchestrator) — UT-0554-01 implements this T-test.
- **TEST-SPEC-0553** (install_connection) — produces border IDs verified in UT-T5-04.
- **TEST-SPEC-T6** (streaming vs batch isomorphism) — sibling: T5 verifies internal correctness; T6 verifies output equivalence with `split()`.
- **TEST-SPEC-0510** (SPEC-04 R12 amendment) — UT-T5-04 border IDs may differ from `split()`-produced IDs (per the amendment); the test asserts the bijectivity property, not absolute integer equality.
