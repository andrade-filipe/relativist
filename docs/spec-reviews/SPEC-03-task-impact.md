# SPEC-03 Task Impact Report

**Date:** 2026-04-05
**Trigger:** SPEC-03 revised from Revised v2 to Revised v3 (adversarial review)
**Source:** `SPEC-03-round2-defender.md` (12 issues addressed: 10 ACCEPTED, 2 PARTIALLY ACCEPTED)

---

## 1. Summary

| Category | Count |
|----------|-------|
| Tasks updated | 9 |
| Tasks created | 1 |
| Tasks obsoleted | 0 |
| Tasks unchanged (SPEC-03 refs) | 18 |
| **Total tasks referencing SPEC-03** | **28** |

One new task (TASK-0218) was created for the `link` helper function introduced by R25/R26. Nine existing tasks were updated to reflect v3 changes (new `link` helper, caller-managed counters, preconditions, O(1) amortized complexity, `is_reduced()` caveat). The BACKLOG.md was also updated with the new task and dependency adjustments.

---

## 2. Key Changes in SPEC-03 Revised v3

| Change | Source Issue | Scope | Impact on tasks |
|--------|-------------|-------|-----------------|
| New `link` helper wrapping `Net::connect` with removed-agent guard | SC-001 | Section 4.5, 4.7 | TASK-0218 (new), TASK-0024, TASK-0025, TASK-0026, TASK-0030 |
| New R25: self-referencing auxiliary port edge case | SC-001 | Section 3.8, 4.1.2 | TASK-0218 (new), TASK-0024 |
| New R26: FreePort boundary sentinel I1 relaxation | SC-002 | Section 3.9 | TASK-0218 (new), TASK-0025, TASK-0026 |
| Interaction counter returns applied rule (caller-managed) | SC-004, SC-007 | Section 2, 3.4 (R12) | TASK-0027, TASK-0028, TASK-0029, TASK-0072 |
| Explicit preconditions on all `interact_*` functions | SC-003 | Section 4.5 | TASK-0023, TASK-0024, TASK-0025, TASK-0026 |
| `is_reduced()` caveat documented | SC-005 | Section 4.8 | TASK-0014 |
| O(1) amortized qualification for R20 | SC-006 | Section 3.6, 4.10 | TASK-0025, TASK-0026 |
| Vec reallocation safety note | SC-006 | Section 4.5, 4.9 | TASK-0026 |
| T2 preservation note in rule preamble | SC-012 | Section 4.1 | No task impact (informational) |
| Stale Redex definition replaced with cross-reference | SC-011 | Section 2 | No task impact (informational) |

---

## 3. New Task

### TASK-0218: Implement link helper (safe port reconnection)
**Reason:** SPEC-03 v3 introduces a `link` helper function (Section 4.5) that wraps `Net::connect` with a guard for removed agents. This is a new module-private function in `src/reduction/rules.rs` required by R25 (self-referencing auxiliary ports) and R26 (FreePort boundary sentinels). All interaction functions (`interact_anni`, `interact_comm`, `interact_eras`) now use `link` for external wire reconnection instead of calling `net.connect()` directly.
**Trigger:** SC-001, SC-002.
**Priority:** P0 (critical path -- blocks TASK-0024, TASK-0025, TASK-0026).

---

## 4. Updated Tasks

### TASK-0014: Implement is_reduced and is_valid_redex
**Change:** Added SPEC-03 v3 caveat (SC-005) to Notes section: `is_reduced()` is a necessary but not sufficient condition for Normal Form when stale entries exist. The canonical way to verify Normal Form is `reduce_all`, not `is_reduced()` alone.
**Trigger:** SC-005.
**Sections modified:** Notes (new paragraph).

### TASK-0024: Implement interact_anni (CON-CON, DUP-DUP)
**Change:** Major update. Dependencies now include TASK-0218 (link helper). Acceptance criteria updated: reconnection uses `link()` helper (NOT `net.connect()` directly) to handle self-referencing edge case (R25). Requirements list expanded with R25. Edge case documentation added for full and partial self-references. Pseudocode updated to use `link()`. Explicit preconditions added to docstring per SC-003. Test expectations expanded with R25 self-referencing test cases (full and partial). Notes section includes SPEC-03 v3 change description.
**Trigger:** SC-001 (R25), SC-003 (preconditions).
**Sections modified:** Requirements, Depends on, Context, Acceptance Criteria, Key Types / Signatures, Test Expectations, Notes.

### TASK-0025: Implement interact_eras (CON-ERA, DUP-ERA)
**Change:** Major update. Dependencies now include TASK-0218 (link helper). Acceptance criteria updated: reconnection uses `link()` for external wires. Requirements list expanded with R26. FreePort behavior (R26) documented in context. Explicit preconditions added to docstring per SC-003. Complexity updated from O(1) to O(1) amortized per R20 (Vec reallocation from create_agent). Notes section includes SPEC-03 v3 change description.
**Trigger:** SC-002 (R26), SC-003 (preconditions), SC-006 (O(1) amortized).
**Sections modified:** Requirements, Depends on, Context, Acceptance Criteria, Key Types / Signatures, Notes.

### TASK-0026: Implement interact_comm (CON-DUP)
**Change:** Major update. Dependencies now include TASK-0218 (link helper). Acceptance criteria updated: external wires use `link()`, internal wires remain `net.connect()`. Requirements list expanded with R26. FreePort behavior (R26) and Vec reallocation safety documented in context. Explicit preconditions added to docstring per SC-003. Complexity updated from O(1) to O(1) amortized per R20. Notes section includes SPEC-03 v3 change description.
**Trigger:** SC-002 (R26), SC-003 (preconditions), SC-006 (O(1) amortized, Vec reallocation safety).
**Sections modified:** Requirements, Depends on, Context, Acceptance Criteria, Key Types / Signatures, Notes.

### TASK-0023: Implement interact_void (ERA-ERA)
**Change:** Minor update. Explicit precondition added to acceptance criteria and docstring per SC-003: both agents MUST be live and both MUST be Era.
**Trigger:** SC-003 (preconditions).
**Sections modified:** Acceptance Criteria, Key Types / Signatures.

### TASK-0027: Define StepResult and implement reduce_step
**Change:** Moderate update. `StepResult::Reduced` now carries the applied `Rule` variant. `reduce_step` returns `StepResult::Reduced(rule)` instead of incrementing a counter (SC-004/SC-007). Counter management moved to caller (`reduce_all`, `reduce_n`). Notes section includes SPEC-03 v3 change description.
**Trigger:** SC-004 (caller-managed counters), SC-007 (step 5 mismatch).
**Sections modified:** Key Types / Signatures, Notes.

### TASK-0028: Define ReductionStats and implement reduce_all
**Change:** Moderate update. `ReductionStats` now has per-rule counters (`anni_count`, `comm_count`, `eras_count`, `void_count`) and a `record(rule)` helper method. `reduce_all` returns `ReductionStats` (not `usize`). The caller-managed counter pattern is fully reflected.
**Trigger:** SC-004 (caller-managed counters), R17 (per-rule profiling).
**Sections modified:** Context, Key Types / Signatures, Test Expectations.

### TASK-0029: Implement reduce_n (budget-limited reduction)
**Change:** Minor update. Return type updated from `usize` to `ReductionStats`. Uses `ReductionStats::record()` helper.
**Trigger:** SC-004 (caller-managed counters).
**Sections modified:** Key Types / Signatures.

### TASK-0072: Implement n==1 optimization in run_grid
**Change:** Minor update. Dependencies Context updated: `reduce_all` returns `ReductionStats` (not `usize`). Pseudocode updated to use `stats.total_interactions` instead of treating the return as an integer. Notes section includes SPEC-03 v3 change description.
**Trigger:** SC-004 (caller-managed counters, `ReductionStats` return type).
**Sections modified:** Key Types / Signatures, Dependencies Context, Notes.

### BACKLOG.md
**Change:** TASK-0218 added to Phase 2 table. Dependency chains updated: TASK-0024, TASK-0025, TASK-0026 now depend on TASK-0218. TASK-0027 depends on TASK-0218. TASK-0030 depends on TASK-0218. TASK-0218 has priority P0, status TODO, complexity S.
**Trigger:** TASK-0218 creation.
**Sections modified:** Phase 2: Reduction Engine (SPEC-03) table.

---

## 5. Unchanged Tasks (with SPEC-03 references)

### TASK-0015: Implement debug assertions (I1, I2, I3, I6, I7)
**Reason:** References SPEC-03 only in Notes section ("These assertions are called by the reduction engine (SPEC-03) after each rule application"). The assertion functions themselves are unchanged by SPEC-03 v3 -- the expanded T2 check is a property of `connect`, not of the assertion suite.

### TASK-0019: Define get_agent helper
**Reason:** References SPEC-03 as a downstream consumer. No changes to the helper itself.

### TASK-0020: Scaffold reduction module structure
**Reason:** Already mentions `link` helper in the context description (line 15: "rules (the 4 interaction functions + `link` helper)"). No structural changes needed.

### TASK-0021: Define Rule enum and dispatch table
**Reason:** The `Rule` enum and dispatch table are unchanged in v3. SC-008 (sub-category profiling) was PARTIALLY ACCEPTED and deferred -- no enum split.

### TASK-0022: Implement normalize_pair function
**Reason:** Unchanged. Normalization logic is identical in v2 and v3.

### TASK-0030: Wire up reduction module re-exports
**Reason:** `link` is module-private (`fn`, not `pub fn`) and is not re-exported. The public API surface is unchanged.

### TASK-0056: ID range exhaustion error handling
**Reason:** References SPEC-03 only tangentially (reduction engine context). No v3 impact.

### TASK-0068: Implement drain_stale_redexes
**Reason:** References SPEC-03 for `is_valid_redex`. `is_valid_redex` is unchanged in SPEC-03 v3.

### TASK-0070: Implement run_grid Phase 1 (split) + Phase 2 (local reduce)
**Reason:** Already uses `reduce_all(&mut partition.subnet)` returning `ReductionStats` per SPEC-05 v3 changes. The SPEC-03 v3 `ReductionStats` type is consistent with what this task already expects.

### TASK-0071: Implement run_grid Phase 3 (merge + resolve borders + metrics)
**Reason:** Already uses `reduce_all` returning `ReductionStats`. No additional SPEC-03 v3 impact beyond what SPEC-05 v3 already addressed.

### TASK-0075: Integration test - Fundamental Property G1
**Reason:** References `reduce_all` from SPEC-03. The return type change is transparent to the test (it compares Normal Forms, not stats directly).

### TASK-0076: Merge module exports and wiring
**Reason:** Imports `reduce_all` from `crate::reduction`. The import path and function name are unchanged.

### TASK-0092: Implement run_coordinator (distributed grid loop)
**Reason:** References `reduce_all()` from SPEC-03. The function is called in Phase 3 (border resolution). Return type change is handled by the caller's existing `ReductionStats` handling.

### TASK-0093: Implement run_worker (worker loop)
**Reason:** References `reduce_all()` from SPEC-03. The worker already collects `ReductionStats` per SPEC-06 v3 changes.

### TASK-0109: Define WorkerState enum and FSM types
**Reason:** References SPEC-03 only in a doc comment. No structural impact.

### TASK-0117: Enforce Core/Infrastructure layer boundary
**Reason:** References `reduction` module as a Core Layer module. No v3 impact on layer classification.

### TASK-0208: Implement arithmetic correctness tests (ET-6, ET-7, ET-8, ET-10)
**Reason:** Uses `reduce_all(&mut net)` followed by `decode_nat`. The two-step pattern was already adopted per SC-004. No further changes needed.

---

## 6. Why Only One New Task Was Needed

The SPEC-03 v3 changes fall into three categories:

1. **New functionality (R25, R26):** The `link` helper function is the only new code artifact. It is a small (~15 LoC) module-private function in `rules.rs`. A dedicated task (TASK-0218) was created because it is a discrete, testable unit that other tasks depend on.

2. **API refinement (SC-004):** The `StepResult::Reduced(Rule)` return type and caller-managed counters change the interface of `reduce_step`, `reduce_all`, and `reduce_n`. These changes are localized to TASK-0027, TASK-0028, and TASK-0029, which already own those functions.

3. **Documentation improvements (SC-003, SC-005, SC-006, SC-011, SC-012):** Preconditions, caveats, and complexity qualifications are documentation-only changes within existing task descriptions. No new implementation work is generated.

---

## 7. Residual Risks and Cross-Spec Notes

### SC-008: Deferred sub-category profiling
The `Rule::Anni` enum variant covers both CON-CON and DUP-DUP. If SPEC-09 benchmarking reveals a need for finer-grained profiling, the implementer MAY split into `Rule::AnniCon` and `Rule::AnniDup`. This would require updating TASK-0021, TASK-0024, TASK-0027, and TASK-0028. Currently deferred (SHOULD-level, R17).

### SC-009: Cross-reference error in SPEC-09
SPEC-09 Section 4.5 incorrectly references "SPEC-03, R16" for the per-rule counter; the correct reference is R17. This is outside SPEC-03's territory. Documented for the SPEC-09 review round.

### SC-010: Test gap in SPEC-08
A dedicated test `RE_ALL_STALE` should be added to SPEC-08 for `reduce_all` on a net with only stale redexes. This is outside SPEC-03's territory. Documented for the SPEC-08 review round.

### SPEC-00 counter definition discrepancy
SPEC-00 defines "Interaction Counter" as "incremented on each successful reduce_step." SPEC-03 v3 changes this to caller-managed via `ReductionStats`. SPEC-00 should be updated in a future consistency pass. Documented in the defender response but not resolved (outside SPEC-03's territory).

### TASK-0072 stale return type (now fixed)
TASK-0072 previously referenced `reduce_all(net: &mut Net) -> usize`, which was inconsistent with SPEC-03 v3's `-> ReductionStats` change. Updated in this impact assessment.
