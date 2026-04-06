# SPEC-08 Task Impact Report

**Date:** 2026-04-05
**Trigger:** SPEC-08 revised from v2 to v3 (adversarial review, 17 issues)
**Reviewer:** Task Updater

---

## Summary

| Category | Count |
|----------|-------|
| Tasks updated (cross-references added) | 8 |
| Tasks created | 3 |
| Tasks requiring no change | 187 |
| BACKLOG.md updated | Yes (total: 195 -> 198) |

---

## Key Changes in SPEC-08 v3

1. **Scope expansion:** SPEC-01-07 -> SPEC-01-14 (5 new test sections: 3.18-3.23)
2. **Global test label namespace:** SEC-, OBS-, UIO-, ARCH-, ENC- prefixes
3. **Integration test rename:** I1-I11 -> INT1-INT11
4. **Cross-reference fix:** R22-R24 -> R24-R26 (SPEC-02 serialization)
5. **New property tests:** PB13-PB16
6. **New requirements:** R39-R44
7. **66 new test definitions** across sections 3.18-3.23
8. **6 new module test files** in directory structure
9. **Graph isomorphism performance target:** 500 agents, 100ms (SHOULD)
10. **Dedicated D2 test:** R44 (SHOULD)

---

## Tasks Updated

### TASK-0185: Implement graph isomorphism (nets_isomorphic)
- **Change:** Added SPEC-08 v3 SHOULD performance target (500 agents, 100ms) from SC-009 to requirements and notes
- **Rationale:** SPEC-08 Section 4.4 now specifies a concrete performance target for the isomorphism checker

### TASK-0207: Implement encoding unit tests (ET-1 through ET-5, ET-9, ET-12)
- **Change:** Added SPEC-08 ENC- namespace cross-reference (ET-1 = ENC-1, ..., ET-12 = ENC-12, R39)
- **Rationale:** SPEC-08 v3 maps SPEC-14's ET- labels to the global ENC- namespace

### TASK-0208: Implement arithmetic correctness tests (ET-6, ET-7, ET-8, ET-10)
- **Change:** Added SPEC-08 as additional spec reference; added ENC- namespace mapping in requirements and notes
- **Rationale:** Same as TASK-0207

### TASK-0209: Implement distributed correctness test (ET-11)
- **Change:** Added SPEC-08 as additional spec reference; added ENC-11 namespace mapping in requirements
- **Rationale:** Same as TASK-0207

### TASK-0139: Security integration tests
- **Change:** Added SPEC-08 as additional spec reference; mapped T1-T10 (SPEC-10) to SEC-1 through SEC-11 (SPEC-08 R40)
- **Rationale:** SPEC-08 v3 maps SPEC-10's T1-T10 to the global SEC- namespace

### TASK-0179: Integration tests for I/O roundtrips and generators
- **Change:** Added SPEC-08 as additional spec reference; mapped T1-T14 (SPEC-12) to UIO-1 through UIO-14 (SPEC-08 R42)
- **Rationale:** SPEC-08 v3 maps SPEC-12's T1-T14 to the global UIO- namespace

### TASK-0108: Implement coordinator FSM transition function
- **Change:** Added SPEC-08 as additional spec reference; added ARCH-1 through ARCH-8, ARCH-13 (SPEC-08 R43) to requirements
- **Rationale:** SPEC-08 v3 Section 3.22 defines ARCH tests that map directly to this task's test expectations

### TASK-0110: Implement worker FSM transition function
- **Change:** Added SPEC-08 as additional spec reference; added ARCH-9 through ARCH-12 (SPEC-08 R43) to requirements
- **Rationale:** SPEC-08 v3 Section 3.22 defines ARCH tests that map directly to this task's test expectations

---

## Tasks Created

### TASK-0215: Implement property tests PB13-PB14 (T2/T3 invariant coverage)
- **Priority:** P1
- **Spec:** SPEC-08, R25
- **Rationale:** SC-008 identified that T2 (only principal ports form redexes) and T3 (active pairs are disjoint) had no property-based test coverage. PB13 and PB14 fill these gaps in the invariant coverage matrix.
- **Dependencies:** Phase 1, Phase 2

### TASK-0216: Implement property tests PB15-PB16 (Profile B/C targeted generators)
- **Priority:** P1
- **Spec:** SPEC-08, R25-R27
- **Rationale:** SC-013 identified that property tests did not cover Profile B (CON-DUP expansion) or Profile C (sequential dependency). PB15 and PB16 with targeted generators `arb_condup_net` and `arb_chain_net` fill these gaps. Also extends G1 coverage.
- **Dependencies:** Phase 1, Phase 2, Phase 4, TASK-0185

### TASK-0217: Implement dedicated D2 local reduction equivalence test
- **Priority:** P2
- **Spec:** SPEC-08, R44 (SHOULD)
- **Rationale:** SC-017 added a dedicated test for SPEC-01 D2 in isolation. D2 is already covered indirectly by PB10 and INT1-INT11, so this is SHOULD priority for improved diagnostic precision.
- **Dependencies:** Phase 1, Phase 2, Phase 3

---

## Tasks NOT Requiring Changes

### Integration test label rename (I1-I11 -> INT1-INT11)
No existing tasks referenced the old SPEC-08 integration test labels I1-I11. All `I1`-`I5` references in tasks refer to SPEC-01 invariants, which are unchanged. No action needed.

### Cross-reference fix (R22-R24 -> R24-R26)
No existing tasks referenced the incorrect SPEC-02 R22-R24 cross-references from SPEC-08. The SPEC-02 requirement numbers used in tasks (TASK-0017 for serialization, etc.) were already correct. No action needed.

### SPEC-08 requirement renumbering
SPEC-08 v3 did NOT renumber existing requirements R1-R38. New requirements R39-R44 were appended. No renumbering impact on existing tasks.

### Observability tests (OBS-1 through OBS-10, SPEC-08 R41)
The OBS tests map to test expectations already embedded in Phase 8 implementation tasks (TASK-0144 for tracing, TASK-0155/0156/0157 for HTTP endpoints). No dedicated test task existed before, and none is needed -- the tests are naturally co-located with the implementation. The SPEC-08 OBS- namespace provides a unified tracking label.

### E9 performance test update
SPEC-08 v3 changed E9 from a fixed 30-second threshold to a relative `time(10k)/time(1k) < 15` check and marked it `#[ignore]`. No existing task references E9 directly. This will be picked up when Phase 10 edge case tests are implemented.

### RE7 strengthening
SPEC-08 v3 strengthened RE7's description to require topology isomorphism against RE4. No existing task references RE7 directly. This will be picked up when Phase 2 reduction engine tests are implemented (naturally part of TASK-0024/0026 test expectations).

---

## BACKLOG.md Changes

- Updated total task count: 195 -> 198
- Added new section: "Cross-Cutting: Test Strategy (SPEC-08 v3)"
- Added entries for TASK-0215, TASK-0216, TASK-0217

---

## Residual Notes

1. **Label mapping maintenance:** If SPEC-10 through SPEC-14 are revised and add/renumber test requirements, the SPEC-08 namespace mappings (SEC-, OBS-, UIO-, ARCH-, ENC-) and the corresponding task cross-references must be updated in sync. This is a known maintenance cost documented in the SPEC-08 v3 defender response.

2. **No tasks reference SPEC-08 R38 note about SPEC-03:** SC-016 added a forward reference from SPEC-08 R38 to SPEC-03 for the `ReductionStrategy` type. This should be picked up during SPEC-03's next revision, not via a backlog task.

3. **Configurable budget for property tests:** SC-010 replaced the heuristic `n * n * 10` budget with `MAX_PROPTEST_BUDGET = 100,000`. This affects all property test tasks but is a generator-level change, not a test-level change. It will be handled when the random net generator is implemented (part of TASK-0216 and the broader property test infrastructure).
