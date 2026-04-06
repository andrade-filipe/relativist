# SPEC-04 Task Impact Report

**Date:** 2026-04-05
**Trigger:** SPEC-04 revised from Revised v2 to Revised v3 (adversarial review)
**Source:** `SPEC-04-round2-defender.md` (14 issues addressed: 10 ACCEPTED, 4 PARTIALLY ACCEPTED)

---

## 1. Summary

| Category | Count |
|----------|-------|
| Tasks updated | 8 |
| Tasks created | 2 |
| Tasks obsoleted | 0 |
| Tasks unchanged (SPEC-04 refs) | 22 |
| **Total tasks referencing SPEC-04** | **32** |

Two new tasks were created for entirely new requirements introduced in v3: TASK-0219 (stale boundary FreePort precondition assertion, SC-003) and TASK-0220 (root port propagation R28, SC-010). The remaining changes affected existing Phase 3 tasks and downstream consumers of the Partition/PartitionPlan types.

---

## 2. Key Changes in SPEC-04 Revised v3

| Change | Source | Scope | Impact on tasks |
|--------|--------|-------|-----------------|
| R1 rewritten with cross-spec note for FSM mapping | SC-001 | Section 3.1 | TASK-0048 (doc comment), TASK-0049 (no structural change) |
| ERA port slots: all 3 copied uniformly | SC-002 | Section 4.5 Step 5 | TASK-0050 (acceptance criteria expanded) |
| Precondition: no stale boundary FreePorts | SC-003 | Section 4.5 pre-conditions | **TASK-0219 (new)**, TASK-0049 (calls assertion) |
| FreePort connections never deleted during local reduction | SC-004 | Section 4.6 Scenario 2 | TASK-0055 (notes updated), TASK-0063 (notes updated) |
| `border_id_start`/`border_id_end` in Partition struct | SC-005 | Section 4.1, R15a | TASK-0041, TASK-0046, TASK-0048, TASK-0049, TASK-0055, TASK-0063, TASK-0070 |
| O(A+W) complexity correction for R2 trivial case | SC-007 | Section 3.1 | TASK-0048 (doc comment updated) |
| Wire classification narrative clarified | SC-009 | Section 4.4 | TASK-0046 (narrative only, no code impact) |
| New R28: root port propagation | SC-010 | Section 3.6 (new) | **TASK-0220 (new)**, TASK-0049, TASK-0050 |
| R3 SHOULD for skipping empty partition dispatch | SC-011 | Section 3.1 | TASK-0049 (acceptance criteria note) |
| serde derives on PartitionPlan | SC-012 | Section 4.1 | TASK-0042 (derives updated) |
| Section renumbering (3.6->3.7, 3.7->3.8) | Multiple | Sections 3.6-3.8 | No task impact (informational) |

---

## 3. New Tasks

### TASK-0219: Stale boundary FreePort precondition assertion
**Created for:** SC-003 (SPEC-04 v3 precondition: input net MUST NOT contain stale boundary FreePorts from a previous round)
**Priority:** P1
**Depends on:** TASK-0045 (max_freeport_id)
**Summary:** Implements a `#[cfg(debug_assertions)]` function that scans the net's port array before split to detect stale boundary FreePort sentinels left by an incomplete merge. Called at the top of `split()` (TASK-0049).

### TASK-0220: Root port propagation during split (R28)
**Created for:** SC-010 (new R28: root port must be propagated to exactly one partition)
**Priority:** P0
**Depends on:** TASK-0050 (build_subnet)
**Summary:** Implements `propagate_root()` which determines which partition inherits the original net's `root` field. If `net.root` is `Some(AgentPort(id, p))`, only the partition containing agent `id` gets it. If `None`, all partitions get `None`. If `Some(FreePort(f))`, the partition inheriting the interface wire gets it.

---

## 4. Updated Tasks

### TASK-0041: Define Partition struct
**Change:** Major update. Added `border_id_start: u32` and `border_id_end: u32` fields to the Partition struct (R15a). Requirements list expanded to include R15a. Doc comments explain the discrimination rule for lazy FreePort index reconstruction. Test expectations include border_id_start/end verification, including `border_id_start == border_id_end` for partitions with no borders.
**Trigger:** SC-005 (border_id_start/end for FreePort disambiguation).
**Sections modified:** Requirements, Acceptance Criteria, Key Types / Signatures, Test Expectations, Dependencies Context.

### TASK-0042: Define PartitionPlan struct
**Change:** Added `serde::Serialize, serde::Deserialize` derives to PartitionPlan. Doc comment explains serde derives are included for consistency and potential future use (debugging, checkpointing), even though PartitionPlan stays on the coordinator.
**Trigger:** SC-012 (PartitionPlan lacked serde derives).
**Sections modified:** Acceptance Criteria, Key Types / Signatures, Notes.

### TASK-0046: Wire classification logic
**Change:** `WireClassification` struct now includes `border_id_start: u32` and `next_border_id: u32` fields for R15a. These track the [border_id_start, border_id_end) range assigned during classification. Requirements list expanded to include R15a. Acceptance criteria include tracking border_id_start.
**Trigger:** SC-005 (border_id_start/end metadata needed by Partition).
**Sections modified:** Requirements, Acceptance Criteria, Key Types / Signatures.

### TASK-0048: split() trivial case (num_workers=1)
**Change:** Acceptance criteria adds `border_id_start` and `border_id_end` are equal (no borders, range is empty) for the trivial case. Doc comment updated to reflect O(A+W) complexity claim (was O(1) in v2).
**Trigger:** SC-005 (border_id_start/end), SC-007 (O(A+W) correction).
**Sections modified:** Acceptance Criteria.

### TASK-0049: split() general case orchestrator
**Change:** Major update. Requirements list expanded to include R15a and R28. Acceptance criteria now includes: (1) pre-condition assertion for stale boundary FreePorts (TASK-0219), (2) Step 5d for root port propagation via TASK-0220, (3) Step 7 passing `border_id_start` and `border_id_end: border_id_counter` to each Partition, (4) R3 SHOULD note for skipping empty partition dispatch. Dependencies expanded to include TASK-0219 and TASK-0220. Pseudocode in Key Types section shows the complete wiring.
**Trigger:** SC-003 (precondition), SC-005 (border_id_start/end), SC-010 (R28), SC-011 (R3 dispatch SHOULD).
**Sections modified:** Requirements, Acceptance Criteria, Key Types / Signatures, Dependencies Context.

### TASK-0050: Build sub-net for one partition
**Change:** Acceptance criteria expanded to specify: (1) all `PORTS_PER_SLOT` (3) port array entries per agent are copied uniformly, including `DISCONNECTED` slots for ERA agents (slots 1 and 2); (2) sparse sizing -- sub-net's agents Vec sized to `max_agent_id_in(A_i) + 1`, ports Vec to `(max_agent_id_in(A_i) + 1) * PORTS_PER_SLOT`; (3) root port propagation reference to R28/TASK-0220. Requirements list expanded to include R28.
**Trigger:** SC-002 (ERA port slots, sparse sizing), SC-010 (R28).
**Sections modified:** Requirements, Acceptance Criteria, Key Types / Signatures, Test Expectations, Notes.

### TASK-0055: FreePort index lazy reconstruction
**Change:** Major update. Acceptance criteria now specify using `border_id_start` and `border_id_end` from the Partition to discriminate boundary FreePorts (R15a). The three-case discrimination rule is explicit: boundary (`border_id_start <= id < border_id_end`), Lafont (`id < border_id_start`), DISCONNECTED (`u32::MAX`). Notes section updated to explain that FreePort connections are NEVER simply deleted during local reduction (SC-004, Scenario 2 rewrite). Requirements list includes R15a.
**Trigger:** SC-004 (Scenario 2: FreePort transfer, not deletion), SC-005 (border_id_start/end for discrimination).
**Sections modified:** Requirements, Acceptance Criteria, Key Types / Signatures, Test Expectations, Notes.

### TASK-0063: Implement rebuild_free_port_index (SPEC-05)
**Change:** Downstream consumer of Partition.border_id_start/end. Acceptance criteria and function signature updated to accept `border_id_start` and `border_id_end` parameters (R15a). The discrimination logic matches TASK-0055. Notes updated to explain FreePort connections are NEVER deleted (SC-004). Requirements include R15a cross-reference.
**Trigger:** SC-004 (FreePort transfer semantics), SC-005 (border_id_start/end parameters).
**Sections modified:** Requirements, Acceptance Criteria, Key Types / Signatures, Test Expectations, Notes.

### BACKLOG.md
**Change:** Phase 3 table updated. TASK-0219 and TASK-0220 added. TASK-0049 depends list updated to include TASK-0220. Total task count updated from 199 to 201.
**Trigger:** New tasks TASK-0219 and TASK-0220.
**Sections modified:** Phase 3: Partitioning (SPEC-04) table, header total count.

---

## 5. Unchanged Tasks (with SPEC-04 references)

### Phase 3 -- Direct SPEC-04 tasks

#### TASK-0040: Define WorkerId type and IdRange struct
**Reason:** No v3 changes affect WorkerId or IdRange. The types, fields, and derives remain identical.

#### TASK-0043: Define PartitionStrategy trait
**Reason:** The trait signature is unchanged. SC-008 (HashMap materialization concern) was PARTIALLY ACCEPTED with no spec change. R23 already covers future alternatives.

#### TASK-0044: Implement ContiguousIdStrategy
**Reason:** The allocation function is unchanged. No v3 change affects how agents are assigned to workers.

#### TASK-0045: Helper function max_freeport_id
**Reason:** The function signature and behavior are unchanged. The stale boundary FreePort precondition (SC-003) uses this function but does not change it.

#### TASK-0047: Compute static ID space ranges
**Reason:** SC-006 fixed the example numbers in Section 4.7 but did not change the formula in R18. The implementation is unaffected.

#### TASK-0051: Redex queue population for partitions
**Reason:** R24/R25 unchanged. Redex queue filtering logic is not affected by any v3 change.

#### TASK-0052: FreePort index construction per partition
**Reason:** The initial FreePort index construction is unchanged. The border entries from wire classification already provide the data needed. The border_id_start/end metadata is set on the Partition struct, not on the FreePort index.

#### TASK-0053: Debug assertion for C1
**Reason:** C1 (complete agent coverage) is unchanged. The assertion logic is not affected by v3 changes.

#### TASK-0054: Debug assertions for C2 and C3
**Reason:** C2 and C3 are unchanged. The assertion logic is not affected by v3 changes.

#### TASK-0056: ID range exhaustion error handling
**Reason:** R20 is unchanged. IdRange methods are not affected by v3 changes.

### Phase 1 -- SPEC-02 types used by SPEC-04

#### TASK-0004: Define PortRef enum
**Reason:** PortRef enum is unchanged. The FreePort variant's dual purpose (Lafont vs. boundary) is documented but structurally identical.

#### TASK-0016: Define BorderMap type alias
**Reason:** BorderMap type alias is unchanged. The border_id_start/end metadata is on Partition, not on the BorderMap.

### Phase 2 -- SPEC-03 reduction engine

#### TASK-0029: Implement reduce_n
**Reason:** References SPEC-04/SPEC-05 for grid context. SC-004's correction (FreePort connections never deleted) affects the conceptual model but not the reduction engine implementation, which already handles reconnections correctly.

### Phase 4 -- SPEC-05 merge and grid cycle

#### TASK-0060: Define GridMetrics struct
**Reason:** GridMetrics fields are defined by SPEC-05, not SPEC-04. No v3 change affects metrics structure.

#### TASK-0061: Define WorkerRoundStats struct
**Reason:** WorkerRoundStats is defined by SPEC-05 R37. No SPEC-04 v3 change affects it.

#### TASK-0065: Merge function - unite agents
**Reason:** Merge accepts PartitionPlan by value. The new Partition fields (border_id_start/end) are present but not used during agent unification. Lafont vs. boundary FreePort discrimination during merge uses the border map as discriminator, not border_id_start/end.

#### TASK-0066: Merge function - restore boundary connections
**Reason:** Boundary restoration uses `free_port_index`, which is rebuilt (TASK-0063) before merge. The rebuild uses border_id_start/end, but TASK-0066 itself only reads the rebuilt index.

#### TASK-0069: run_grid skeleton
**Reason:** Grid loop structure is defined by SPEC-05. No SPEC-04 v3 change affects the loop skeleton.

#### TASK-0070: run_grid Phase 1 (split) + Phase 2 (local reduce)
**Reason:** Already references `border_id_start`/`border_id_end` in the `rebuild_free_port_index` call (lines 67-69 of the pseudocode). This was updated as part of TASK-0063's update. No further changes needed.

#### TASK-0074: Integration test - split/merge identity (D1)
**Reason:** D1 round-trip test structure is unchanged. The test calls split/merge and checks isomorphism. Internal changes to Partition fields are transparent to the test.

#### TASK-0075: Integration test - Fundamental Property G1
**Reason:** G1 test structure is unchanged. Calls run_grid and compares with reduce_all. Internal partition changes are transparent.

### Phase 5 -- SPEC-06 wire protocol

#### TASK-0082: Define Message enum
**Reason:** Message variants embed `Partition` struct by value. The new fields (border_id_start/end) are automatically serialized/deserialized via serde. No Message enum change needed.

#### TASK-0089: Coordinator distribute phase
**Reason:** Distributes Partition structs to workers. The new fields are transparent (serde handles them).

#### TASK-0090: Coordinator collect phase
**Reason:** Receives Partition structs from workers. Same transparency reasoning.

#### TASK-0092: Implement run_coordinator
**Reason:** Orchestrates split/distribute/collect/merge. Internally calls split() which now produces Partitions with border_id_start/end, but this is transparent to the coordinator loop.

#### TASK-0095: In-memory transport for testing
**Reason:** Tests message serialization round-trips. Partition struct changes are handled by serde. Test data construction may need border_id_start/end values, but this is a minor detail in test setup, not a task-level change.

### Phase 6 -- SPEC-13 FSM

#### TASK-0107: Define CoordinatorState enum and FSM types
**Reason:** CoordinatorAction::InvokeSplit uses `num_workers: usize`. SC-001 documented the usize-to-u32 cast resolution, but this is handled at the call site (action executor), not in the FSM type definitions. No change needed.

#### TASK-0109: Define WorkerState enum and FSM types
**Reason:** WorkerEvent::ReceivePartition embeds a Partition. The new fields are transparent.

#### TASK-0117: Enforce Core/Infrastructure layer boundary
**Reason:** Layer boundary audit is unaffected by Partition field changes. All changes are within the Core Layer.

#### TASK-0127: Extend Message enum with registration variants
**Reason:** Registration variants do not involve Partition structs.

---

## 6. Why Only 2 New Tasks Were Needed

The 14 issues from the adversarial review fell into three categories:

1. **Corrections to existing spec text** (SC-004 Scenario 2 rewrite, SC-006 example numbers, SC-007 O(1) correction, SC-009 narrative clarification): These fixed inaccuracies but did not add new functionality. Existing tasks already covered the correct behavior.

2. **New metadata/fields on existing types** (SC-005 border_id_start/end, SC-012 serde on PartitionPlan): These extended existing tasks (TASK-0041, TASK-0042, TASK-0046, etc.) rather than requiring new tasks.

3. **Genuinely new requirements** (SC-003 stale FreePort precondition, SC-010 root port propagation R28): These required new tasks because they introduce functionality not covered by any existing task.

The SC-001 cross-spec note (FSM mapping) is purely documentary -- it clarifies the relationship between SPEC-04's `split()` signature and SPEC-13's FSM actions without introducing new implementation work.

---

## 7. Cross-Spec Consistency Notes

1. **SPEC-05 (Merge):** TASK-0063 (`rebuild_free_port_index`) was updated to use `border_id_start`/`border_id_end` parameters (R15a). TASK-0070 (`run_grid` Phase 2) already passes these values from the Partition struct. No SPEC-05 spec text change is needed because the merge spec defers FreePort discrimination to SPEC-04's R15a mechanism.

2. **SPEC-13 (FSM):** SC-001 resolved the split() function signature discrepancy between SPEC-04 (u32, strategy parameter, returns PartitionPlan) and SPEC-13 (usize, no strategy, fires SplitComplete with Vec<Partition>). The cross-spec note in SPEC-04 R1 documents the resolution. SPEC-13 does not need a corresponding update because the FSM actions are intentionally abstract triggers, not function call specifications.

3. **SPEC-08 (Test Strategy):** SC-013 noted that R4 (determinism) and R5 (purity) are testable but via different mechanisms (repetition for R4, code review for R5). SPEC-08 SHOULD note this in its test plan. This is outside the scope of this task impact report.

4. **Requirement count is now 29** (was 27 in v2): R15a (border_id_start/end metadata) and R28 (root port propagation) are the two new MUST requirements. R3 gained a SHOULD clause for dispatch optimization. Section renumbering (3.6->3.7, 3.7->3.8) does not affect requirement semantics.
