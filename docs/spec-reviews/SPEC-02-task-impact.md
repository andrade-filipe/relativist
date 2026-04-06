# SPEC-02 Task Impact Report

**Date:** 2026-04-05
**Trigger:** SPEC-02 revised from Revised v2 to Revised v3 (adversarial review)
**Source:** `SPEC-02-round2-defender.md` (15 issues addressed: 11 ACCEPTED, 4 PARTIALLY ACCEPTED)

---

## 1. Summary

| Category | Count |
|----------|-------|
| Tasks updated | 14 |
| Tasks created | 2 |
| Tasks obsoleted | 0 |
| Tasks unchanged (SPEC-02 refs) | ~54 |
| **Total tasks referencing SPEC-02** | **~70** |

Two new tasks were created: TASK-0019 (get_agent/get_agent_mut accessors, R15a/R15b) and TASK-0231 (count_live_agents/live_agents, R16a/R16b). Fourteen existing tasks were updated to reflect v3 changes (root constraint R6a, self-loop policy R18b, PartialEq/Eq R26a, root port exception R18a, ERA unused port assertion, BorderMap type alias relocation, FreePort disconnect staleness, serialization DISCONNECTED sentinels, canonical accessor usage). The BACKLOG.md was updated with both new tasks and dependency adjustments.

---

## 2. Key Changes in SPEC-02 Revised v3

| Change | Source Issue | Scope | Impact on tasks |
|--------|-------------|-------|-----------------|
| R6a: root constrained to `None` or `Some(AgentPort(id, 0))` | SC-006 | Section 3.1 | TASK-0008, TASK-0015 |
| R15a/R15b: `get_agent`/`get_agent_mut` canonical accessors | SC-001 | Section 3.3, 4.5.9 | TASK-0019 (new), TASK-0014, TASK-0203 |
| R16a/R16b: `count_live_agents`/`live_agents` on Net | SC-005 | Section 3.3, 4.5.10 | TASK-0231 (new), TASK-0073, TASK-0069, TASK-0070, TASK-0106, TASK-0188, TASK-0189, TASK-0190, TASK-0192 |
| R18a: root port T1 exception | SC-002 | Section 3.4, 4.6 | TASK-0015 |
| R18b: self-loop policy (intra-agent valid, same-port invalid) | SC-003 | Section 3.4, 4.5.4 | TASK-0011, TASK-0015 |
| R26a: `PartialEq`/`Eq` derive for Net | SC-011 | Section 3.6, 4.3 | TASK-0008, TASK-0018 |
| R14 update: FreePort disconnect staleness documented | SC-004 | Section 3.3 | TASK-0012 |
| R10 update: direct mutation warning | SC-014 | Section 3.2 | (informational, no dedicated task) |
| BorderMap type alias moved to SPEC-04 | SC-010 | Section 4.10 | TASK-0016 |
| ERA unused port assertion (`assert_era_unused_ports_clean`) | SC-007 | Section 4.6 | TASK-0015 |
| `is_valid_redex` updated to use `get_agent` | SC-001 | Section 4.5.8 | TASK-0014 |
| `connect` doc comments (FreePort redex, arity, self-loop) | SC-008, SC-015 | Section 4.5.4 | TASK-0011 |
| Serialization DISCONNECTED sentinel documentation | SC-013 | Section 4.9 | TASK-0017 |
| Root behavior during reduction documented | SC-009 | Section 4.9 | (informational, no task impact) |
| `is_reduced` semantics clarification | SC-012 | Section 4.9 | TASK-0014 |

---

## 3. New Tasks

### TASK-0019: Implement get_agent and get_agent_mut accessors
**Reason:** SPEC-02 v3 (SC-001) added R15a and R15b as MUST-level requirements for canonical agent lookup on Net. These replace the verbose inline pattern `self.agents.get(id as usize).and_then(|slot| slot.as_ref())` that was duplicated across SPEC-14, SPEC-05, and other specs. `get_agent` is declared as the canonical accessor; callers MUST NOT index into `agents` directly.
**Trigger:** SC-001.
**Priority:** P0 (critical path -- used by TASK-0014, TASK-0203, and many downstream tasks).

### TASK-0231: Implement count_live_agents and live_agents on Net
**Reason:** SPEC-02 v3 (SC-005) added R16a and R16b as MUST-level requirements for agent counting and iteration on Net. These formalize the iteration pattern that 6+ successor specs needed. `count_live_agents` is O(A) and `live_agents` returns an iterator using `filter_map(|s| s.as_ref())`. Multiple downstream tasks updated to use `net.count_live_agents()` instead of free function helpers.
**Trigger:** SC-005.
**Priority:** P0 (critical path -- used by TASK-0069, TASK-0070, TASK-0073, TASK-0106, TASK-0188, and others).

---

## 4. Updated Tasks

### TASK-0008: Define Net struct and constructors
**Change:** Major update. Requirements list expanded with R6a and R26a. Acceptance criteria updated: `Net` derives `PartialEq, Eq` (R26a); `root` field constrained to `None` or `Some(AgentPort(id, 0))` (R6a); FreePort roots explicitly invalid. Net derive list in code example updated. Doc comment on `root` field specifies R6a constraint. Notes section documents R26a relationship with TASK-0018.
**Trigger:** SC-006 (R6a), SC-011 (R26a).
**Sections modified:** Requirements, Acceptance Criteria, Key Types / Signatures, Notes.

### TASK-0011: Implement connect
**Change:** Major update. Requirements list expanded with R18b (self-loop policy). Acceptance criteria updated: debug assertion rejects same-port self-connections (`debug_assert_ne!(a, b)`); intra-agent connections (different ports of same agent) are explicitly VALID. Code example includes R18b debug assertion, self-loop policy doc comment, and FreePort redex detection doc comment. Test expectations expanded with self-loop and intra-agent connection test cases. Notes section documents arity validation trade-off.
**Trigger:** SC-003 (R18b), SC-008 (FreePort redex note), SC-015 (arity validation).
**Sections modified:** Requirements, Acceptance Criteria, Key Types / Signatures, Test Expectations, Notes.

### TASK-0012: Implement disconnect
**Change:** Minor update. Notes section expanded with R14 FreePort staleness documentation: when the target is a `FreePort(bid)`, the `set_port` call on the FreePort side is a no-op, and the corresponding Border Map entry becomes stale by design (SPEC-05 R6 handles this during merge).
**Trigger:** SC-004 (R14 update).
**Sections modified:** Notes.

### TASK-0014: Implement is_reduced and is_valid_redex
**Change:** Moderate update. Requirements now reference R15a for agent lookup. Dependencies updated: added TASK-0019. Code example updated: `is_valid_redex` uses `self.get_agent(a)` and `self.get_agent(b)` instead of verbose inline pattern. `is_reduced` notes expanded with SPEC-03 v3 caveat about queue-based semantics. Dependencies Context updated.
**Trigger:** SC-001 (R15a), SC-012 (`is_reduced` semantics).
**Sections modified:** Requirements, Depends on, Key Types / Signatures, Dependencies Context, Notes.

### TASK-0015: Implement debug assertions (I1, I2, I3, I6, I7)
**Change:** Major update. Requirements list expanded with R18a and R18b. Acceptance criteria expanded: `assert_adjacency_consistent` now handles root port exception (R18a) by computing root agent ID; `assert_era_unused_ports_clean` (new) verifies ERA unused port slots contain DISCONNECTED; `assert_root_consistent` (new) verifies R6a + I7; `assert_all_invariants` calls five functions instead of three. Code example includes full implementation of root port exception, ERA port check, root consistency check. Test expectations expanded with ERA, root, FreePort, and cross-port self-loop test cases.
**Trigger:** SC-002 (R18a), SC-003 (R18b implied), SC-006 (R6a/I7), SC-007 (ERA unused ports).
**Sections modified:** Requirements, Context, Acceptance Criteria, Key Types / Signatures, Test Expectations.

### TASK-0016: Define BorderMap type alias
**Change:** Minor update. Spec attribution changed from SPEC-02 to "SPEC-04 (originally SPEC-02; type alias moved to SPEC-04 in SPEC-02 Revised v3)". Context updated: BorderMap type alias code block was removed from SPEC-02 Section 4.10; SPEC-02 now retains only the conceptual description; type definition deferred to SPEC-04.
**Trigger:** SC-010 (BorderMap relocation).
**Sections modified:** Spec (header), Context.

### TASK-0017: Add serde + bincode serialization support
**Change:** Minor update. Notes section expanded with SPEC-02 v3 (SC-013) serialization DISCONNECTED sentinel documentation: the serialized port array may contain `FreePort(u32::MAX)` in ERA unused port slots and root agent's principal port.
**Trigger:** SC-013 (serialization format).
**Sections modified:** Notes.

### TASK-0018: Verify PartialEq and Eq for Net
**Change:** Moderate update. Title changed from "Implement PartialEq for Net" to "Verify PartialEq and Eq for Net". Requirements expanded with R26a. Context updated: R26a now mandates `Net` derives both `PartialEq` and `Eq`, included directly in TASK-0008. This task becomes a verification/testing task. Code example updated to show derives are already present. Notes section documents the role change.
**Trigger:** SC-011 (R26a).
**Sections modified:** Title, Requirements, Context, Acceptance Criteria, Key Types / Signatures, Notes.

### TASK-0069: Implement run_grid function skeleton
**Change:** Minor update. Code example updated: `count_live_agents(&current_net)` changed to `current_net.count_live_agents()` (R16a method call). Dependencies Context updated to reference `Net::count_live_agents(&self)` from TASK-0231. Notes updated to document R16a change.
**Trigger:** SC-005 (R16a).
**Sections modified:** Key Types / Signatures, Dependencies Context, Notes.

### TASK-0070: Implement run_grid Phase 1 and Phase 2
**Change:** Minor update. Code example updated: two instances of `count_live_agents(&partition.subnet)` changed to `partition.subnet.count_live_agents()` (R16a method call). Dependencies Context updated to reference `Net::count_live_agents(&self)` from TASK-0231.
**Trigger:** SC-005 (R16a).
**Sections modified:** Key Types / Signatures, Dependencies Context.

### TASK-0073: Implement count_live_agents helper
**Change:** Minor update. Dependency changed from `TASK-0220` (incorrect -- that is root port propagation) to `TASK-0231` (correct -- R16a/R16b on Net). Context notes that this task becomes a consumer/wiring task: the merge module should call `net.count_live_agents()` instead of implementing its own helper.
**Trigger:** SC-005 (R16a), dependency correction.
**Sections modified:** Depends on, Context.

### TASK-0106: Implement print_summary function
**Change:** Minor update. Acceptance criteria updated: uses `net.count_live_agents()` (R16a, TASK-0231) instead of importing from TASK-0073. Code example updated. Dependencies Context updated.
**Trigger:** SC-005 (R16a).
**Sections modified:** Acceptance Criteria, Key Types / Signatures, Dependencies Context.

### TASK-0188, TASK-0189, TASK-0190, TASK-0192: Benchmark verify functions
**Change:** Minor update. Code examples updated: `count_live_agents(distributed_result)` changed to `distributed_result.count_live_agents()` (R16a method call). Dependencies Context updated in TASK-0188.
**Trigger:** SC-005 (R16a).
**Sections modified:** Key Types / Signatures, Dependencies Context (TASK-0188 only).

### TASK-0203: Implement decode_nat (Church numeral readback)
**Change:** Minor update. Dependencies updated: added TASK-0019 (get_agent). Context notes that `get_agent` is now a MUST-level public API on Net (R15a, TASK-0019). The local helper previously defined in this task is no longer needed.
**Trigger:** SC-001 (R15a).
**Sections modified:** Depends on, Context, Dependencies Context, Notes.

### BACKLOG.md
**Change:** TASK-0019 and TASK-0231 added to Phase 1 table. TASK-0014 dependency updated to include TASK-0019. TASK-0018 title updated to "Verify PartialEq and Eq for Net". TASK-0073 dependency updated from `none` to `0231`. Total count updated.
**Trigger:** TASK-0019 and TASK-0231 creation.
**Sections modified:** Phase 1: Core Types (SPEC-02) table, Phase 4: Merge & Grid Cycle (SPEC-05) table.

---

## 5. Unchanged Tasks (with SPEC-02 references)

The following tasks reference SPEC-02 but were not impacted by the v3 changes:

### Phase 1 Core Types
- **TASK-0001** (module structure): Structural prerequisite, no type changes.
- **TASK-0002** (Symbol enum): R1 unchanged.
- **TASK-0003** (AgentId/PortId): R2/R3 unchanged.
- **TASK-0004** (PortRef enum): R4 unchanged.
- **TASK-0005** (Agent struct): R5 unchanged.
- **TASK-0006** (arity/total_ports): R1 implicit, unchanged.
- **TASK-0007** (PORTS_PER_SLOT, port_index, DISCONNECTED): R8 partial, unchanged.
- **TASK-0009** (create_agent): R11 unchanged.
- **TASK-0010** (get_target/set_port): R15 unchanged.
- **TASK-0013** (remove_agent): R12 unchanged.

### Other Phases
Tasks in Phases 2-11 that reference SPEC-02 types (Net, PortRef, AgentPort, etc.) but do not use any changed requirements were not modified. These include TASK-0020 through TASK-0029 (reduction), TASK-0040 through TASK-0056 (partitioning), TASK-0060 through TASK-0076 (merge/grid, except those listed above), TASK-0080 through TASK-0096 (protocol), TASK-0100 through TASK-0119 (CLI), TASK-0120 through TASK-0139 (security), TASK-0140 through TASK-0159 (observability), TASK-0160 through TASK-0179 (I/O), and others.

---

## 6. Why Two New Tasks Were Needed

The SPEC-02 v3 changes fall into three categories:

1. **New public API (R15a, R15b, R16a, R16b):** Four new MUST-level methods on Net that formalize previously ad-hoc patterns. These are discrete, testable units with multiple downstream consumers, warranting dedicated tasks (TASK-0019, TASK-0231).

2. **Constraint tightening (R6a, R18a, R18b, R26a):** New requirements that constrain existing types or add debug assertions. These changes are localized to existing tasks (TASK-0008 for R6a/R26a, TASK-0011 for R18b, TASK-0015 for R18a/R6a/I6/I7).

3. **Documentation improvements (SC-004, SC-008, SC-009, SC-010, SC-012, SC-013, SC-014, SC-015):** FreePort disconnect staleness, BorderMap relocation, serialization sentinels, root behavior, `is_reduced` semantics, and `connect` doc comments. These are documentation-only changes within existing task descriptions.

---

## 7. Residual Risks and Cross-Spec Notes

### SPEC-12 R56 and T11c contradiction
R6a constrains `root` to `AgentPort` values, contradicting SPEC-12 R56 (which allows FreePort roots) and T11c (which tests `root free(0)`). SPEC-02 declares `AgentPort`-only roots as authoritative. SPEC-12 needs a revision to reconcile R56 and T11c. This is outside SPEC-02's territory; documented in the defender response.

### TASK-0073 dependency correction
The previous agent set TASK-0073's dependency to `TASK-0220 (SPEC-02 R16a/R16b)` but TASK-0220 is actually about root port propagation (SPEC-04 R28). This was corrected to `TASK-0231`. The SPEC-05 task-impact report also references this correction.

### Field visibility (`agents`, `ports` remain `pub`)
R10 documents that direct mutation bypasses invariant checks, but fields remain public. Debug assertions (R20) provide the safety net. The implementer may choose to make fields private, but the spec does not mandate it.

### `is_reduced` naming retained
The function name `is_reduced` was retained despite the critic's suggestion to rename to `is_queue_empty`. Documentation clarifies the semantics. No task impact beyond the note in TASK-0014.
