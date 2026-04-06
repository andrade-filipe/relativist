# SPEC-01 Task Impact Report

**Date:** 2026-04-05
**Trigger:** SPEC-01 revised from Revised v2 to Revised v3 (adversarial review)
**Source:** `SPEC-01-round2-defender.md` (13 issues addressed)

---

## 1. Summary

| Category | Count |
|----------|-------|
| Tasks updated | 5 |
| Tasks created | 0 |
| Tasks obsoleted | 0 |
| Tasks unchanged (SPEC-01 refs) | 12 |
| **Total tasks referencing SPEC-01** | **17** |

No new tasks were needed because the new invariants I6 and I7 are implemented within the existing TASK-0015 (`assert_all_invariants`), which is the single task responsible for all debug assertion implementations. The T1 rewrite affects the same assertion function. All downstream tasks that call `assert_all_invariants()` automatically benefit from the expanded checks.

---

## 2. Key Changes in SPEC-01 Revised v3

| Change | Scope | Impact on tasks |
|--------|-------|-----------------|
| T1 rewritten with DISCONNECTED exception, port-to-self rejection, cross-port self-loop acceptance | Section 3.1 | TASK-0015 (assertion rewrite) |
| New invariant I6 (ERA Auxiliary Slot Cleanliness) | Section 3.3 | TASK-0015 (new assertion function) |
| New invariant I7 (Root Port Consistency) | Section 3.3 | TASK-0015 (new assertion function) |
| Failure model updated for SPEC-10 auth phase | Section 4.4 | No task impact (informational) |
| "When to verify" expanded for encoding/parsing/generation | Section 3.1 (T1) | TASK-0165 (already validates; now explicit) |
| Two verification modes documented (panic vs Result::Err) | Section 4.3 | TASK-0015, TASK-0165 (already implemented both modes) |
| Invariant error code SHOULD recommendation | Section 4.3 | TASK-0015 (pseudocode already uses codes) |
| Dependency hierarchy updated with I6, I7 | Section 4.1 | No task impact (informational) |

---

## 3. Updated Tasks

### TASK-0015: Implement debug assertions (I1, I2, I3, I6, I7)
**Change:** Major update. Title expanded from "I1, I2, I3" to "I1, I2, I3, I6, I7". Context section now documents all 5 invariant checks. Acceptance criteria adds `assert_era_slots_clean()` for I6 and `assert_root_consistent()` for I7. `assert_all_invariants()` now calls 5 assertion functions instead of 3. The `assert_adjacency_consistent` pseudocode is rewritten to match SPEC-01 Revised v3's `assert_ports_consistent`: handles DISCONNECTED at root port, rejects port-to-self connections, accepts cross-port self-loops. Full pseudocode for `assert_era_slots_clean` and `assert_root_consistent` added. Test expectations expanded with 6 new test cases covering I6, I7, T1 root port exception, port-to-self rejection, and cross-port self-loop acceptance.
**Trigger:** SC-001 (T1 rewrite), SC-003 (new I6), SC-004 (new I7), SC-002 (cross-port self-loops).
**Sections modified:** Title, Context, Acceptance Criteria, Key Types / Signatures (complete rewrite of code block), Test Expectations.

### TASK-0027: Define StepResult and implement reduce_step
**Change:** Minor update. Doc comment for `reduce_step` updated from "verify T1, I1, I2" to "verify T1, I1, I2, I6, I7". No behavioral change -- the function already calls `net.assert_all_invariants()` which will include the new checks once TASK-0015 is implemented.
**Trigger:** SC-003 (new I6), SC-004 (new I7).
**Sections modified:** Key Types / Signatures (doc comment only).

### TASK-0067: Implement merge debug assertions
**Change:** Context updated to list all 5 invariants checked by `assert_all_invariants()`: T1, I1, I2, I6, I7. Acceptance criteria and Dependencies Context sections updated similarly. Previously listed only "T1 (linearity), I1 (bidirectionality), I2 (reference validity)".
**Trigger:** SC-003 (new I6), SC-004 (new I7).
**Sections modified:** Context, Acceptance Criteria, Dependencies Context (Notes section).

### TASK-0165: Implement Text DSL parser - net construction and validation (Pass 2)
**Change:** Context and acceptance criteria updated to include I6 and I7 in the post-construction validation. The parser already validates unconditionally (not just in debug mode) because user input is untrusted. The note about using the same invariant checks as TASK-0015 now explicitly mentions I6 and I7.
**Trigger:** SC-006 (T1 "when to verify" now explicitly lists encoding and parsing), SC-003 (new I6), SC-004 (new I7).
**Sections modified:** Context, Acceptance Criteria, Notes.

### TASK-0213: Implement ERROR-level logging requirements (R9a)
**Change:** Invariant references updated from "SPEC-01 T1-T7" to "SPEC-01 Revised v3: T1-T7, I1-I7" in both the context and acceptance criteria. The invariant count is now T1-T7, D1-D6, I1-I7, G1.
**Trigger:** SC-003 (new I6), SC-004 (new I7).
**Sections modified:** Context, Acceptance Criteria.

### BACKLOG.md
**Change:** TASK-0015 title updated from "Implement debug assertions (I1, I2, I3)" to "Implement debug assertions (I1, I2, I3, I6, I7)" in the Phase 1 table.
**Trigger:** Consistency with TASK-0015 title change.
**Sections modified:** Phase 1: Core Types (SPEC-02) table.

---

## 4. Unchanged Tasks (with SPEC-01 references)

### TASK-0003: Define AgentId and PortId type aliases
**Reason:** References I3 (monotonically increasing IDs). I3 was updated only in scope (per-partition semantics), not in formal statement. The type alias definition is unaffected.

### TASK-0028: Define ReductionStats and implement reduce_all
**Reason:** References T7 (invariant step count). T7 was unchanged in Revised v3.

### TASK-0029: Implement reduce_n (budget-limited reduction)
**Reason:** References I5 (termination). I5 was unchanged in Revised v3.

### TASK-0044: Implement ContiguousIdStrategy
**Reason:** References I3 (monotonic IDs). I3 scope clarification does not change the implementation.

### TASK-0053: Debug assertion for C1
**Reason:** Partition-layer assertion. Does not reference any invariant affected by the revision.

### TASK-0054: Debug assertions for C2 and C3
**Reason:** Partition-layer assertion. Does not reference any invariant affected by the revision.

### TASK-0068: Implement drain_stale_redexes
**Reason:** References I4 (stale redexes). I4 was unchanged in Revised v3.

### TASK-0074: Integration test - split/merge identity (D1)
**Reason:** References D1. D1 was unchanged in Revised v3.

### TASK-0075: Integration test - Fundamental Property G1
**Reason:** References G1 and T7. G1 was updated only in "how to verify" (added decode_nat comparison). This does not change the test structure, which already uses isomorphism.

### TASK-0201: Implement encode_church_into
**Reason:** References I3 (monotonic IDs via create_agent). Unchanged.

### TASK-0204: Implement build_add
**Reason:** References I3 (monotonic IDs). Unchanged.

### TASK-0207: Implement encoding unit tests
**Reason:** ET-9 calls `assert_all_invariants()`. The expanded checks (I6, I7) will be picked up automatically when TASK-0015 is implemented. No task changes needed.

### TASK-0209: Implement distributed correctness test
**Reason:** References G1. The G1 test structure is unchanged.

---

## 5. Why No New Tasks Were Needed

The new invariants I6 (ERA Auxiliary Slot Cleanliness) and I7 (Root Port Consistency) are **assertion-only** invariants. They:

1. Do not require new data structures or new fields in existing types.
2. Do not alter the reduction engine, partitioning, merge, or wire protocol logic.
3. Are implemented as two new assertion functions (`assert_era_slots_clean`, `assert_root_consistent`) within the existing TASK-0015 scope.
4. Are called via `assert_all_invariants()`, which is already invoked at all required verification points (reduction, merge, split, encoding, parsing, generation).

The T1 rewrite similarly does not require new tasks: it refines the existing `assert_adjacency_consistent` function (now `assert_ports_consistent` in the spec pseudocode) within TASK-0015, adding root port exception handling, port-to-self rejection, and explicit acceptance of cross-port self-loops.

---

## 6. Cross-Spec Consistency Notes

1. **SPEC-08 (Test Strategy)** references "I1-I5" in several places. A future consistency pass should update SPEC-08 to reference "I1-I7." This is noted in the defender response (Section "Residual Risks", point 4) but is outside the scope of this task impact report (SPEC-08 has its own revision cycle).

2. **SPEC-02 Section 4.6** (`assert_adjacency_consistent`) differs from SPEC-01 Revised v3's `assert_ports_consistent`. SPEC-01's version is stricter: DISCONNECTED is only allowed at the root port. The implementer SHOULD use SPEC-01's version as authoritative. This is noted in the defender response (Section "Cross-spec consistency notes", point 3).

3. **Invariant count is now T1-T7, D1-D6, I1-I7, G1** (total: 21 invariants, up from 19 in Revised v2).
