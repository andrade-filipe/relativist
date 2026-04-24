# SPEC-00 Task Impact Report

**Date:** 2026-04-05
**Trigger:** SPEC-00 revised from Revised v2 to Revised v3 (adversarial review)
**Source:** `SPEC-00-round2-defender.md` (15 issues addressed, 2 not addressed, 3 partially accepted)

---

## 1. Summary

| Category | Count |
|----------|-------|
| Tasks updated | 0 |
| Tasks created | 0 |
| Tasks obsoleted | 0 |
| Tasks unchanged | 195 |

**No task updates are required.** SPEC-00 is a glossary -- it defines vocabulary but does not carry MUST requirements that tasks implement. The v3 revision added 16 new glossary terms and 1 new domain, corrected the PortRef encoding description, and changed "Rust (proposed)" labels to "Rust (confirmed)". None of these changes conflict with existing task definitions.

---

## 2. Analysis of Tasks Referencing SPEC-00

4 tasks explicitly reference SPEC-00. All references remain valid after the v3 revision.

### TASK-0004: Define PortRef enum
**SPEC-00 references:** Sections 6.1, 6.2 (FreePort Lafont / FreePort Boundary)
**Status:** No change needed. The task's description of `FreePort(u32)` dual purpose (Lafont vs. boundary) is consistent with the revised Section 6.2, which now uses `FreePort(u32)` variant of `enum PortRef` (confirmed). The task already references SPEC-02 R4 as the authoritative type definition.
**PortRef encoding:** The task correctly uses the enum representation (`AgentPort(AgentId, PortId)` / `FreePort(u32)`), not the stale compact u32 encoding that was removed in v3.

### TASK-0018: Implement PartialEq for Net
**SPEC-00 references:** Section 6.12 (Isomorphism of IC-nets)
**Status:** No change needed. Section 6.12 was not renumbered (Domain 4 entries were not added/reordered in v3). The task's distinction between structural equality (PartialEq) and isomorphism (Section 6.12) remains accurate.

### TASK-0024: Implement interact_anni (CON-CON and DUP-DUP rules)
**SPEC-00 references:** Sections 3.3, 3.4 (CON / DUP definitions)
**Status:** No change needed. Sections 3.3 and 3.4 were not modified in v3 (only Sections 3.1, 3.2, 3.7, 3.10-3.14 had "Rust (proposed)" -> "Rust (confirmed)" label changes, which do not affect the CON/DUP definitions referenced by this task).

### TASK-0185: Implement graph isomorphism (nets_isomorphic)
**SPEC-00 references:** Section 6.12 (Isomorphism of IC-nets)
**Status:** No change needed. Same reasoning as TASK-0018: Section 6.12 was not renumbered or redefined.

---

## 3. Analysis of Key SPEC-00 v3 Changes vs. Task Terminology

### 3.1 "Rust (proposed)" -> "Rust (confirmed)" (12 entries)
**Impact:** None. Tasks reference SPEC-02 (not SPEC-00) for Rust type definitions. No task uses the phrase "Rust (proposed)". The label change is internal to the glossary.

### 3.2 PortRef encoding rewrite (Sections 3.11, 6.2)
**Impact:** None. No task references the stale compact u32 bit-packed encoding or the CON/DUP/ERA/VAR tag scheme. TASK-0004 (the PortRef definition task) already uses the correct `enum PortRef { AgentPort, FreePort }` representation per SPEC-02 R4.

### 3.3 Domain 7 -- Benchmarks and Metrics (8 new entries: Sections 9.1-9.8)
**Impact:** None. The 17 tasks that use Profile A/B/C terminology (TASK-0171 through TASK-0177, TASK-0188 through TASK-0195, TASK-0204 through TASK-0206) already use the correct profile names:
- Profile A = Embarrassingly Parallel (matches Section 9.1)
- Profile B = Expansion with Collapse (matches Section 9.1)
- Profile C = Sequential Dependency (matches Section 9.1)

The new metric terms (MIPS, Speedup, Efficiency, Border Ratio, Overhead Ratio, Break-Even Point, Scaling Curve) formalize definitions already used implicitly in TASK-0183 (BenchmarkResult), TASK-0184 (AggregatedStats), TASK-0196 (CSV output), and TASK-0197 (derived metrics). These tasks reference SPEC-09 for metric definitions, not SPEC-00.

### 3.4 Domain 5 -- Grid Infrastructure (16 new entries: Sections 7.5, 7.10-7.20)
**Impact:** None. No tasks reference SPEC-00 Domain 5 sections by number. The renumbering of Sections 7.5-7.8 (shifted to 7.6-7.9 due to WorkerId insertion) does not affect any task.

New glossary terms (WorkerId, Grid Loop, BSP, Core Layer, Infrastructure Layer, Transport, Wire Protocol, Frame, Message, Local Mode, Distributed Mode, Direct Reduction) formalize concepts already correctly used in tasks. Spot-checked:
- TASK-0040 defines `WorkerId = u32` (matches Section 7.5)
- TASK-0082 defines `Message` enum (matches Section 7.17)
- TASK-0076 references "BSP grid loop" (matches Sections 7.10, 7.11)
- TASK-0107 references "BSP superstep" (matches Section 7.11)

### 3.5 Border Redex sub-categories (Section 6.7 expanded)
**Impact:** None. TASK-0075 uses the term "border redexes" correctly (line 34: "border redexes that generate new redexes" = emergent border redexes). The expanded sub-category definitions (Pre-existing vs. Emergent) add precision to the glossary but do not change the semantics referenced by tasks.

---

## 4. Conclusion

SPEC-00 v3 is a terminology-level revision that adds precision and new terms to the glossary. All task files already use terminology consistent with the revised definitions. The key reason no updates are needed:

1. **Tasks reference authoritative specs (SPEC-02 through SPEC-14), not the glossary, for type definitions and requirements.** SPEC-00 is a vocabulary reference, not a requirements source.
2. **The 4 tasks that do reference SPEC-00 sections use section numbers that were not renumbered** (Domain 1 and Domain 4 sections were stable; only Domain 5 was renumbered, and no tasks reference Domain 5 by section number).
3. **Profile A/B/C terminology was already correct** in all 17 benchmark/generator tasks before the glossary formalized it in Domain 7.
4. **The stale PortRef compact encoding** was never propagated to tasks -- TASK-0004 correctly used the enum representation from day one.
