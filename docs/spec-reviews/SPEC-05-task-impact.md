# SPEC-05 Task Impact Report

**Date:** 2026-04-05
**Trigger:** SPEC-05 revised from Revised v2 to Revised v3 (adversarial review, 14 issues)
**Source:** `SPEC-05-round2-defender.md` (12 accepted, 2 partially accepted, 0 not addressed)
**Reviewer:** Task Updater

---

## 1. Summary

| Category | Count |
|----------|-------|
| Tasks updated | 16 |
| Tasks created | 1 |
| Tasks obsoleted | 1 |
| Tasks unchanged (SPEC-05 refs) | 32 |
| **Total tasks referencing SPEC-05** | **50** |

---

## 2. Key Changes in SPEC-05 Revised v3

| Change | SC | Impact on tasks |
|--------|-----|-----------------|
| WorkerRoundStats R37 upgraded SHOULD->MUST, +reduce_duration_secs, +interactions_by_rule, +serde derives | SC-001 | TASK-0061, TASK-0070, TASK-0152 (obsoleted) |
| Termination check moved from top of loop to after merge+reduce_all, aligned with SPEC-13 CheckTermination | SC-002 | TASK-0069 |
| merge() R1 accepts PartitionPlan by value (not separate Vec+HashMap) | SC-003 | TASK-0065, TASK-0066, TASK-0071 |
| GridMetrics R35: +total_interactions_by_rule: [u64; 6] | SC-004 | TASK-0060, TASK-0071 |
| Partition queue discard clarified; debug assertion on queue staleness at merge time | SC-005 | TASK-0065 |
| num_workers type consistency note (u32, not usize) | SC-006 | TASK-0069 |
| FreePort (Lafont) vs FreePort (Boundary) distinction during merge using border map | SC-007 | TASK-0065 |
| New R35a: index_rebuild_time_per_round (SHOULD) | SC-008 | TASK-0060 |
| Merge timing split: merge_time (structural) + border_reduce_time (resolution) | SC-009 | TASK-0060, TASK-0071 |
| New R41: full-scan NF verification in debug mode (SHOULD) | SC-010 | TASK-0069, TASK-0230 (created) |
| Initial Normal Form check [step 0] before grid loop | SC-011 | TASK-0069 |
| PartitionPlan by value resolves partial-move issue (same as SC-003) | SC-012 | TASK-0065, TASK-0066 |
| R19 upgraded SHOULD->MUST (border_interactions_per_round) | SC-013 | TASK-0071 |
| reduce_all per-rule data flow documented (reduction_stats_to_by_rule helper) | SC-014 | TASK-0070, TASK-0071 |

---

## 3. Tasks Updated

### TASK-0060: Define GridMetrics struct
**Change:** Added 3 new fields: `total_interactions_by_rule: [u64; 6]` (SC-004), `border_reduce_time_per_round: Vec<Duration>` (SC-009), `index_rebuild_time_per_round: Vec<Duration>` (SC-008, R35a). Changed `merge_time_per_round` doc to "structural merge only" (SC-009). Updated code block, acceptance criteria, and notes.
**Sections modified:** Acceptance Criteria, Key Types / Signatures, Notes.

### TASK-0061: Define WorkerRoundStats struct
**Change:** Major update. Priority upgraded P1->P0 (R37 now MUST). Title reflects 6-field struct. Two new fields added: `reduce_duration_secs: f64` and `interactions_by_rule: [u64; 6]` (SC-001). serde derives added (`serde::Serialize, serde::Deserialize`). Context updated to declare this as canonical definition resolving SPEC-11 OQ-1. Note added that TASK-0152 is now obsoleted.
**Sections modified:** Title, Priority, Context, Acceptance Criteria, Key Types / Signatures, Test Expectations, Notes.

### TASK-0065: Merge function - unite agents + internal connections
**Change:** Merge signature changed to accept `PartitionPlan` by value with internal destructuring (SC-003, SC-012). Step 2 now distinguishes Boundary vs Lafont FreePorts using border map as discriminator (SC-007). Debug assertion on partition queue staleness added (SC-005). Requirements updated to include R1 (SC-003), R9(a) (SC-005).
**Sections modified:** Requirements, Context, Acceptance Criteria, Key Types / Signatures, Notes.

### TASK-0066: Merge function - restore boundary connections
**Change:** `borders` is now owned (destructured from PartitionPlan by value, SC-003/SC-012). Code block updated to iterate `&borders`. Dependencies Context updated to reference by-value PartitionPlan.
**Sections modified:** Key Types / Signatures, Dependencies Context, Notes.

### TASK-0067: Merge debug assertions
**Change:** Context and Acceptance Criteria updated to list all 5 invariants checked by `assert_all_invariants()`: T1, I1, I2, I6, I7 (previously listed only T1, I1, I2). This aligns with SPEC-01 v3 invariant additions.
**Sections modified:** Context, Acceptance Criteria, Dependencies Context.

### TASK-0068: Implement drain_stale_redexes
**Change:** Notes updated to reflect v3 call sites: drain_stale_redexes is now called at two points -- step [0] (initial Normal Form check before loop) and step [7] (termination check after merge+reduce_all). Previously stated "at the top of the loop" which was stale for v3.
**Sections modified:** Notes.

### TASK-0069: run_grid skeleton with termination logic
**Change:** Major restructuring. Termination check moved from top of loop to step [7] after merge+reduce_all (SC-002). Initial Normal Form check [step 0] added before the loop (SC-011). Debug-mode `verify_no_redexes_full_scan` call added at termination check (SC-010, R41). num_workers type note added (SC-006). Code block completely rewritten.
**Sections modified:** Acceptance Criteria, Key Types / Signatures, Dependencies Context, Notes.

### TASK-0070: run_grid Phase 1 (split) + Phase 2 (local reduce)
**Change:** Phase 2 now times each worker's `reduce_all` separately for `reduce_duration_secs` (SC-001). Maps `ReductionStats` to per-rule `[u64; 6]` via `reduction_stats_to_by_rule` helper (SC-001, SC-014). Accumulates `local_by_rule: [u64; 6]` across workers. WorkerRoundStats construction updated to 6 fields. Code block updated.
**Sections modified:** Acceptance Criteria, Key Types / Signatures, Dependencies Context, Notes.

### TASK-0071: run_grid Phase 3 (merge + resolve borders + metrics)
**Change:** `merge(plan)` by value (SC-003). Timing split into `merge_time_per_round` (structural) + `border_reduce_time_per_round` (SC-009). Per-rule accumulation into `total_interactions_by_rule` from `local_by_rule + border_by_rule` (SC-004). R19 noted as MUST (SC-013). Code block updated.
**Sections modified:** Acceptance Criteria, Key Types / Signatures, Dependencies Context, Notes.

### TASK-0072: n==1 optimization in run_grid
**Change:** Code block updated to populate `total_interactions_by_rule` via `reduction_stats_to_by_rule(&stats)` in the n==1 early-return path (SC-004). Acceptance criteria updated to include `total_interactions_by_rule`. SPEC-05 v3 note added.
**Sections modified:** Acceptance Criteria, Key Types / Signatures, Notes.

### TASK-0073: Implement count_live_agents helper
**Change:** Dependency updated from `none` to `TASK-0231` (SPEC-02 R16a/R16b -- count_live_agents as a MUST on Net). Context notes that this task becomes a consumer/wiring task if TASK-0231 (Net method) is completed first.
**Sections modified:** Depends on, Context.

### TASK-0082: Define Message enum
**Change:** `WorkerRoundStats` dependency context updated to reference 6-field struct with `reduce_duration_secs` and `interactions_by_rule`. SPEC-06 v3 changes also applied (7 variants).
**Sections modified:** Dependencies Context.

### TASK-0090: Implement coordinator collect phase
**Change:** Return type updated to use `WorkerRoundStats` (6-field per SPEC-05 v3). Dependencies Context notes SPEC-05 merge module.
**Sections modified:** Dependencies Context.

### TASK-0093: Implement run_worker
**Change:** Notes section updated: WorkerRoundStats now has 6 fields. Worker must instrument timing around `reduce_all` and collect per-rule interaction counts.
**Sections modified:** Context (SPEC-06 v3 note), Test Expectations (6-field verification).

### TASK-0095: Implement in-memory transport for testing
**Change:** Test data notes updated to reflect 6-field `WorkerRoundStats`.
**Sections modified:** Notes (implicit -- the struct change propagates through serialization tests).

### TASK-0153: Implement coordinator metric aggregation from worker reports
**Change:** Dependency updated from TASK-0152 to TASK-0061 (TASK-0152 is obsoleted; the canonical 6-field WorkerRoundStats is now in TASK-0061).
**Sections modified:** Depends on, Dependencies Context.

---

## 4. Tasks Created

### TASK-0230: Implement verify_no_redexes_full_scan (R41)
- **Priority:** P1
- **Spec:** SPEC-05
- **Requirements:** R41 (SHOULD)
- **Rationale:** SC-010 promoted OQ-3 to R41 as a SHOULD requirement for defense-in-depth Normal Form verification. Scans the entire port array in debug mode to detect redexes not present in the queue. Called from `run_grid` termination check under `#[cfg(debug_assertions)]` (TASK-0069).
- **Dependencies:** TASK-0014 (is_valid_redex), TASK-0064 (is_principal_pair), Phase 1
- **Complexity:** S

---

## 5. Tasks Obsoleted

### TASK-0152: Extend WorkerRoundStats with observability fields
- **Reason:** SPEC-05 v3 (SC-001) upgraded R37 to MUST and integrated the two observability fields (`reduce_duration_secs`, `interactions_by_rule`) directly into the canonical `WorkerRoundStats` definition (TASK-0061). SPEC-11 OQ-1 is resolved. TASK-0152's planned extension is now redundant with TASK-0061.
- **Downstream impact:** TASK-0153 dependency updated from TASK-0152 to TASK-0061.

---

## 6. Unchanged Tasks (with SPEC-05 references)

### Phase 1 tasks with tangential SPEC-05 references
**TASK-0004** (PortRef enum), **TASK-0010** (get_target/set_port), **TASK-0011** (connect), **TASK-0012** (disconnect), **TASK-0016** (BorderMap), **TASK-0019** (get_agent accessors): Reference SPEC-05 in explanatory comments only (e.g., "FreePort targets resolved during merge (SPEC-05)"). No behavioral change from the v3 revision.

### Phase 2 tasks with tangential SPEC-05 references
**TASK-0025** (interact_eras), **TASK-0029** (reduce_n), **TASK-0030** (reduction module exports): Reference SPEC-05 contextually. The v3 revision does not change the reduction engine interface.

### Phase 3 tasks with tangential SPEC-05 references
**TASK-0042** (PartitionPlan), **TASK-0051** (redex queue population), **TASK-0052** (FreePort index construction), **TASK-0055** (FreePort index lazy reconstruction), **TASK-0056** (ID range exhaustion), **TASK-0218** (link helper), **TASK-0219** (stale boundary assertion): Reference SPEC-05 for merge context. The v3 changes to merge signature and semantics do not affect the split/partition interface -- PartitionPlan struct is unchanged.

### Phase 4 tasks unchanged in substance
**TASK-0062** (GridConfig), **TASK-0063** (rebuild_free_port_index), **TASK-0064** (is_principal_pair), **TASK-0074** (split/merge identity test), **TASK-0075** (Fundamental Property G1 test), **TASK-0076** (merge module wiring): GridConfig has no new fields from v3. The rebuild and helper functions were already specified correctly. Tests and wiring tasks reference the merge function but their test logic is invariant to the v3 changes (same function contract, same correctness properties).

### Phase 5-6 tasks with SPEC-05 cross-references
**TASK-0084** (NodeConfig), **TASK-0092** (run_coordinator), **TASK-0094** (GridMetrics network extensions), **TASK-0100** (CLI refactor), **TASK-0102** (CLI-to-config mapping), **TASK-0106** (print_summary), **TASK-0108** (coordinator FSM), **TASK-0111** (run_local_command), **TASK-0117** (layer boundary): Reference `GridConfig`, `GridMetrics`, or `run_grid` from SPEC-05. The v3 changes are internal to the merge module and do not alter the public contract visible to these tasks.

### Phase 11 and cross-cutting tasks
**TASK-0209** (distributed correctness test ET-11): References `run_grid` by public API. The v3 changes are internal.

---

## 7. BACKLOG.md Changes

- Updated total task count: 205 -> 206 (1 new task)
- Updated status breakdown: 205 todo -> 205 todo, 1 obsoleted
- Phase 4 table: added TASK-0230 entry
- Phase 4 table: TASK-0061 priority updated P1 -> P0
- Phase 8 table: TASK-0152 status changed from TODO to OBSOLETED
- Phase 8 table: TASK-0153 dependency changed from `0150, 0152` to `0150, 0061`

---

## 8. Cross-Spec Notes

1. **SPEC-03 OQ-4:** SPEC-05 v3 raised OQ-4 as a cross-spec concern: `ReductionStats` SHOULD be extended with 6 per-rule counters to eliminate the need for the `reduction_stats_to_by_rule` mapping helper. This should be addressed during SPEC-03's next revision, not via a backlog task.

2. **SPEC-11 OQ-1 resolved:** SPEC-05 R37 now contains the extended `WorkerRoundStats` fields. SPEC-11 OQ-1 is declared resolved. TASK-0152 is obsoleted. SPEC-11 Section 4.4's definition is no longer normative; SPEC-05 is canonical.

3. **SPEC-09 enrichment:** GridMetrics now has `total_interactions_by_rule`, `border_reduce_time_per_round`, and `index_rebuild_time_per_round`. SPEC-09 may reference these new fields for richer benchmark analysis. No immediate task impact -- the benchmark tasks (Phase 10) already consume GridMetrics generically.

4. **SPEC-13 alignment confirmed:** Termination check is now aligned with SPEC-13's CheckTermination FSM state. No SPEC-13 task changes needed.
