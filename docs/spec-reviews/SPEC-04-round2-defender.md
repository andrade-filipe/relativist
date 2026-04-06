# SPEC-04 -- Round 2: Defender Response

**Date:** 2026-04-05
**Defender:** Especialista em Specs
**Target:** SPEC-04-partition.md
**Critic review:** SPEC-04-round1-critic.md
**Spec version:** Revised v2 -> Revised v3

---

## Summary

| Response | Count |
|----------|-------|
| ACCEPTED | 10 |
| PARTIALLY ACCEPTED | 4 |
| NOT ADDRESSED | 0 |
| **Total issues** | **14** |

---

## Responses

### SC-001: split() function signature incompatible with SPEC-13 FSM action
**Response:** ACCEPTED
**Action taken:** Rewrote R1 to clearly define the `split()` function signature as `fn split(net: &Net, num_workers: u32, strategy: &dyn PartitionStrategy) -> PartitionPlan`. Added a cross-spec note below R1 that explicitly explains how the FSM interface in SPEC-13 relates to the actual function signature. The three discrepancies are resolved as follows:

1. **`num_workers` type:** SPEC-04 retains `u32` for consistency with `WorkerId` (SPEC-00) and SPEC-05 R25. SPEC-13 uses `usize` in the FSM action because it is an infrastructure-layer type. The action executor resolves the discrepancy via `usize as u32` cast, which is safe for n <= 8 (TCC scope).
2. **Strategy parameter:** The strategy is coordinator-local state, not part of the FSM event/action. The action executor calls `split()` with the strategy from its own state. SPEC-13's `InvokeSplit` action is a trigger, not a complete function call specification. The cross-spec note makes this explicit.
3. **Return type / border map flow:** The coordinator's action executor calls `split()`, receives a full `PartitionPlan` (including the border map), stores the border map in coordinator-local state, and fires `SplitComplete(plan.partitions)` into the FSM with only the `Vec<Partition>`. When `InvokeMergeAndReduce` is executed, the action executor passes both the partitions and the stored border map to `merge()` (SPEC-05 R1). This data flow is now documented in the cross-spec note. The border map is NOT lost -- it resides in coordinator state between split and merge.

**Spec sections modified:** Section 3.1 (R1 -- rewritten with cross-spec note)

---

### SC-002: sub-net construction does not specify handling of ERA's unused port slots
**Response:** ACCEPTED
**Action taken:** Expanded Step 5 of the split algorithm (Section 4.5) to address both concerns:

1. **ERA port slots:** Explicitly stated that all `PORTS_PER_SLOT` (3) port array entries per agent are copied directly, including `DISCONNECTED` slots for ERA agents (slots 1 and 2). This preserves the uniform port array layout regardless of agent arity.
2. **Sparse agent ID sizing:** Added explicit sizing requirements: the sub-net's `agents` Vec MUST be sized to at least `max_agent_id_in(A_i) + 1`, and its `ports` Vec to `(max_agent_id_in(A_i) + 1) * PORTS_PER_SLOT`. Agent slots not belonging to the partition MUST be `None`, and their corresponding port slots MUST be `DISCONNECTED`. This maintains the `id * PORTS_PER_SLOT + port_id` indexing invariant from SPEC-02.

**Spec sections modified:** Section 4.5 (Step 5 -- expanded with ERA handling and sparse sizing)

---

### SC-003: Border ID collision with Lafont FreePort IDs is under-specified for multi-round scenarios
**Response:** ACCEPTED
**Action taken:** Added a precondition to the split algorithm (Section 4.5) requiring that the input net MUST NOT contain any stale FreePort (Boundary) sentinels from a previous round. In debug mode, the split function SHOULD scan for FreePort values above `max_existing_lafont_freeport_id` and assert that none exist. The note explains that this precondition is automatically satisfied when the merge correctly resolves all borders (SPEC-05, R3-R6) and `reduce_all` does not reintroduce boundary FreePorts (the reduction engine does not generate FreePort values). This eliminates ambiguity about the `max_existing_freeport_id` computation in round N: it will always reflect only Lafont FreePorts, since boundary FreePorts from round N-1 have been fully resolved.

**Spec sections modified:** Section 4.5 (pre-conditions -- added stale boundary FreePort precondition)

---

### SC-004: FreePort index maintenance Scenario 2 (Erasure) creates an invariant violation
**Response:** ACCEPTED
**Action taken:** Completely rewrote Scenario 2 in Section 4.6. The original text incorrectly stated that the FreePort connection "disappears" and the index entry should be "removed." The corrected version explains that:

1. When ERA interacts with agent `a` whose auxiliary port connects to `FreePort(bid)`, the erasure rule (CON-ERA or DUP-ERA) creates 2 new ERA agents connected to `a`'s former auxiliary ports. The FreePort is NOT destroyed but transferred to the new ERA agent's principal port.
2. The `free_port_index` MUST be UPDATED, not removed: `index[bid] = AgentPort(new_era_id, 0)`.

Added a comprehensive note below Scenario 2 explaining that a FreePort (Boundary) connection is NEVER simply deleted during local reduction. The boundary FreePort acts as an impermeable wall. The IC reduction rules always reconnect auxiliary ports (CON-CON/DUP-DUP reconnect 4 wires; CON-ERA/DUP-ERA create 2 new ERA agents). Therefore, FreePort entries are always transferred, never orphaned.

**Spec sections modified:** Section 4.6 (Scenario 2 -- complete rewrite with explanatory note)

---

### SC-005: Lazy FreePort index reconstruction cannot distinguish Lafont/Boundary/DISCONNECTED FreePorts
**Response:** ACCEPTED
**Action taken:** This was the deepest issue in the review. The solution adds two new artifacts:

1. **New fields on Partition struct (Section 4.1):** Added `border_id_start: u32` and `border_id_end: u32` to the `Partition` struct. These fields record the global range of border IDs assigned during this split. The discrimination rule is: a `FreePort(id)` in the port array is a boundary FreePort if and only if `border_id_start <= id && id < border_id_end && id != u32::MAX`.
2. **New requirement R15a (Section 3.3):** Formalized the requirement that each partition MUST carry this metadata. The range check is declared as THE mechanism for lazy FreePort index reconstruction.
3. **Updated lazy reconstruction (Section 4.6, approach 2):** The reconstruction algorithm now uses the `border_id_start/end` range to discriminate boundary FreePorts from Lafont FreePorts and `DISCONNECTED` sentinels.
4. **Updated pseudocode (Section 4.5):** The consolidated pseudocode now tracks `border_id_start` and `border_id_end: border_id_counter`, passing them to each Partition.

This approach was chosen (option (c) from the critic's suggestion, refined) because R12 already guarantees that border IDs are strictly greater than all Lafont FreePort IDs. The `border_id_start/end` range makes this guarantee operationally useful at the Partition level, without requiring a `HashSet<u32>` (which would add O(B) memory and serialization overhead).

**Spec sections modified:** Section 3.3 (added R15a), Section 4.1 (Partition struct -- added border_id_start, border_id_end), Section 4.5 (pseudocode -- tracks border_id_start/end), Section 4.6 (lazy reconstruction -- uses range check)

---

### SC-006: R18 ID range computation has an off-by-one; example numbers inconsistent
**Response:** ACCEPTED
**Action taken:** Fixed the example in Section 4.7 to match the formula in R18. The corrected example uses `chunk_size = 4_294_967_295 / 8 = 536_870_911` (integer division). Worker 0 starts at 0, Worker 7 starts at `3_758_096_377` (= 7 * 536_870_911), and Worker 7's range extends to `4_294_967_295` (inclusive), giving it 536_870_918 IDs. Added a note documenting the asymmetry for the last worker (7 extra IDs due to integer division truncation) and explaining the u32 overflow avoidance.

**Spec sections modified:** Section 4.7 (example -- corrected numbers and added note)

---

### SC-007: R2 trivial case claims O(1) but requires cloning the net
**Response:** ACCEPTED
**Action taken:** Changed R2's complexity claim from "O(1) modulo cloning the net" to "O(A + W) for the clone, with no additional partitioning overhead." Added a note that if the implementation accepts the net by value (`net: Net`), the trivial case MAY be O(1) by moving the net into the partition. The current signature uses `&Net` (reference), so cloning is necessary. The implementer may choose to change the ownership model if the performance benefit is warranted.

**Spec sections modified:** Section 3.1 (R2 -- corrected complexity claim)

---

### SC-008: PartitionStrategy trait forces HashMap materialization
**Response:** PARTIALLY ACCEPTED
**Action taken:** No change to the trait itself. The HashMap materialization is acceptable for TCC scope (tens of thousands of agents). R23 already says "The trait SHOULD allow future alternative implementations." The concern is valid for large-scale use but does not affect correctness or performance within the TCC's experimental scope. This is a performance optimization left to future work, as documented in Section 5.2 (alternatives considered).

**Spec sections modified:** (none -- already covered by R23 and Section 5.2)

---

### SC-009: Step 4 narrative about "processing only one side" is misleading
**Response:** ACCEPTED
**Action taken:** Clarified the narrative in Section 4.4. Changed from "a border wire is processed only by the side with the smaller AgentId. The other side receives its FreePort entry as a derived consequence" to "a border wire is DETECTED only from the side with the smaller AgentId, but FreePort entries are generated for BOTH partitions in a single pass. Both `border_entries[sigma(a)]` and `border_entries[sigma(b)]` receive an entry from the same detection step." The pseudocode was already correct; only the narrative was ambiguous.

**Spec sections modified:** Section 4.4 (wire classification narrative -- clarified)

---

### SC-010: No requirement for split to preserve the root port
**Response:** ACCEPTED
**Action taken:** Added a new section 3.6 (Root Port Propagation) with requirement R28. The requirement specifies:
- If `net.root` is `Some(AgentPort(id, p))`, only the partition containing agent `id` gets `subnet.root = net.root`; all others get `None`.
- If `net.root` is `None`, all partitions get `None`.
- If `net.root` is `Some(FreePort(f))`, the root is preserved in the partition inheriting the interface wire.

Also updated Step 5 of the split algorithm to reference R28. Renumbered subsequent sections (3.7 Redex Queue Population, 3.8 Complexity).

**Spec sections modified:** Section 3.6 (new -- R28), Section 4.5 (Step 5 -- added root propagation)

---

### SC-011: R3 allows excess empty partitions but does not specify their ID ranges
**Response:** PARTIALLY ACCEPTED
**Action taken:** Extended R3 to include a SHOULD recommendation: "The coordinator SHOULD skip dispatching empty partitions to workers to avoid unnecessary network traffic." The ID range behavior for empty partitions (defaulting to `id_range.start` as `next_id`) was already correct in the pseudocode, as the critic noted. This is a dispatch optimization, not a correctness issue, so SHOULD is appropriate.

**Spec sections modified:** Section 3.1 (R3 -- added SHOULD for dispatch optimization)

---

### SC-012: PartitionPlan lacks serde derives but Partition has them
**Response:** ACCEPTED
**Action taken:** Added `serde::Serialize, serde::Deserialize` derives to `PartitionPlan` in Section 4.1. Added a doc comment explaining that serde derives are included for consistency and potential future use (debugging, checkpointing), even though `PartitionPlan` stays on the coordinator and is not transmitted over the wire.

**Spec sections modified:** Section 4.1 (PartitionPlan struct -- added serde derives and doc comment)

---

### SC-013: R4/R5 (determinism, purity) are not verifiable by automated tests
**Response:** PARTIALLY ACCEPTED
**Action taken:** No change to the spec. As the critic correctly notes, R4 is testable by repetition (`split(net, plan) == split(net, plan)`) and R5 is a code quality constraint verified by code review. SPEC-08 should note this in its test plan. No spec change needed here -- the requirements are appropriate as written.

**Spec sections modified:** (none -- R4/R5 are appropriate as-is)

---

### SC-014: No explicit requirement for split to handle nets with pre-existing FreePort (Lafont) correctly
**Response:** PARTIALLY ACCEPTED
**Action taken:** This issue is fully covered by the resolution of SC-005 (R15a, border_id_start/end range check). The lazy reconstruction algorithm now explicitly handles all three cases:
- `FreePort(id)` with `id < border_id_start`: Lafont FreePort (ignored in reconstruction)
- `FreePort(id)` with `border_id_start <= id < border_id_end`: boundary FreePort (included)
- `FreePort(u32::MAX)`: DISCONNECTED sentinel (ignored)

No additional change needed beyond what SC-005 already provides.

**Spec sections modified:** (covered by SC-005 changes)

---

## Changes Made to SPEC-04

### Header
- Status changed from "Revised v2" to "Revised v3"

### Section 3.1 (The split Function)
- R1: Rewritten to specify function signature directly (net, num_workers, strategy). Added cross-spec note explaining relationship to SPEC-13's FSM actions and data flow for border map.
- R2: Changed O(1) claim to "O(A + W) for the clone, with no additional partitioning overhead." Added note about by-value optimization.
- R3: Added SHOULD for skipping dispatch of empty partitions.

### Section 3.3 (FreePort and Border Wires)
- R15a: New requirement. Each Partition MUST carry `border_id_start` and `border_id_end` for FreePort disambiguation during lazy reconstruction.

### Section 3.6 (Root Port Propagation) -- NEW
- R28: New requirement. Root port MUST be propagated to the partition containing the root agent. All other partitions get `root = None`.

### Section 3.7 (Redex Queue Population) -- renumbered from 3.6
- No content changes, only section number.

### Section 3.8 (Complexity) -- renumbered from 3.7
- No content changes, only section number.

### Section 4.1 (Types)
- Partition struct: Added `border_id_start: u32` and `border_id_end: u32` fields with doc comments explaining the discrimination rule.
- PartitionPlan struct: Added `serde::Serialize, serde::Deserialize` derives with explanatory doc comment.

### Section 4.4 (Wire Classification)
- Clarified narrative: border wire is "DETECTED" only from one side, but FreePort entries are generated for BOTH partitions.

### Section 4.5 (The split Algorithm)
- Pre-conditions: Added stale boundary FreePort precondition with debug assertion.
- Step 5: Expanded to specify ERA port slot handling, sparse agent ID sizing, uniform PORTS_PER_SLOT copying, and root port propagation (R28).
- Consolidated pseudocode: Tracks `border_id_start` and `border_id_end: border_id_counter`, passes both to Partition constructor.

### Section 4.6 (FreePort Index Maintenance)
- Scenario 2: Complete rewrite. FreePort connections are NEVER deleted during local reduction; they are transferred to replacement agents. The `free_port_index` MUST be UPDATED, not removed.
- Added comprehensive note below Scenario 2 explaining why FreePort entries are always transferred.
- Lazy reconstruction (approach 2): Updated to use `border_id_start/end` range check for FreePort disambiguation. Explicitly excludes Lafont FreePorts and DISCONNECTED sentinels.

### Section 4.7 (Static ID Space Partitioning)
- Fixed example numbers to match R18 formula: `chunk_size = 536_870_911`, Worker 7 starts at `3_758_096_377`. Documented asymmetry for last worker.

---

## Requirement Count (after revision)

| Level | Count |
|-------|-------|
| MUST | 24 (R1-R22, R24-R25, R28, R15a) |
| SHOULD | 4 (R3 dispatch, R23, R26, R27) |
| MAY | 1 (R2 by-value optimization) |
| **Total** | **29** |

---

## Verification

All CRITICAL and HIGH issues have been ACCEPTED:
- SC-001 (CRITICAL): ACCEPTED -- cross-spec note documents FSM-to-function mapping, border map data flow
- SC-002 (HIGH): ACCEPTED -- ERA port slots and sparse sizing specified
- SC-003 (HIGH): ACCEPTED -- stale boundary FreePort precondition added
- SC-004 (HIGH): ACCEPTED -- Scenario 2 completely rewritten (FreePort never deleted)
- SC-005 (HIGH): ACCEPTED -- border_id_start/end added to Partition, lazy reconstruction updated

All MEDIUM issues addressed:
- SC-006 (MEDIUM): ACCEPTED -- example numbers fixed
- SC-007 (MEDIUM): ACCEPTED -- O(1) claim corrected
- SC-008 (MEDIUM): PARTIALLY ACCEPTED -- acceptable for TCC scope
- SC-009 (MEDIUM): ACCEPTED -- narrative clarified

All LOW issues addressed:
- SC-010 (LOW): ACCEPTED -- R28 added
- SC-011 (LOW): PARTIALLY ACCEPTED -- SHOULD added to R3
- SC-012 (LOW): ACCEPTED -- serde derives added
- SC-013 (LOW): PARTIALLY ACCEPTED -- no change needed
- SC-014 (LOW): PARTIALLY ACCEPTED -- covered by SC-005 resolution
