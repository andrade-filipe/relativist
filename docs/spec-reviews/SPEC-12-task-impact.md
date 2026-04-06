# SPEC-12 Task Impact Report

**Date:** 2026-04-05
**Spec version:** Draft v1 -> Revised v2
**Defender response:** SPEC-12-round2-defender.md
**Tasks analyzed:** 20 tasks referencing SPEC-12 (TASK-0160 through TASK-0179) + 2 cross-referencing tasks (TASK-0104, TASK-0117)

---

## 1. Summary Table

| Task ID | Title | Impact | Change Type |
|---------|-------|--------|-------------|
| TASK-0160 | Convert io module to directory structure | UPDATED | Types filename reference fixed (IoError -> FileIoError) |
| TASK-0161 | Define FileIoError, NetFormat, and InspectOutputFormat types | UPDATED | IoError renamed to FileIoError (SC-004); all signatures, ACs, tests updated |
| TASK-0162 | Define NetSummary and ReductionSummary structs | UPDATED | ReductionSummary redesigned (SC-011, SC-020): flat Optional fields, removed GridReductionInfo, added R44a/R44b |
| TASK-0163 | Implement binary format load/save | UPDATED | IoError -> FileIoError (SC-004) |
| TASK-0164 | Text DSL parser - lexing and declaration collection | UPDATED | IoError -> FileIoError (SC-004) |
| TASK-0165 | Text DSL parser - net construction and validation | UPDATED | Major: added R54-R57 (root semantics), R58-R59 (edge cases), IoError -> FileIoError, parse_ic return type fixed (SC-005) |
| TASK-0166 | Text DSL serializer (format_ic) | NO CHANGE | No IoError references, no affected signatures |
| TASK-0167 | JSON format load/save | UPDATED | IoError -> FileIoError (SC-004) |
| TASK-0168 | Implement load_net/save_net dispatch | UPDATED | IoError -> FileIoError (SC-004) |
| TASK-0169 | Implement net_summary computation | UPDATED | Wire count definition clarified (SC-013), arity-aware iteration added (R61) |
| TASK-0170 | Implement reduction summary formatting | UPDATED | Requirements expanded (R44a, R44b), flat grid fields, removed GridReductionInfo reference, conditional Speedup/Efficiency/Overhead |
| TASK-0171 | Implement generator - ep_annihilation | UPDATED | Added R60 (size=0), R61 (arity-aware validation), fixed Church cross-refs (SC-016) |
| TASK-0172 | Implement generators - ep_annihilation_con/dup | UPDATED | Added R38a reference, R60 (size=0) |
| TASK-0173 | Implement generator - con_dup_expansion | UPDATED | Added R60 (size=0) |
| TASK-0174 | Implement generator - dual_tree | UPDATED | Added R60 (size=0), clarified depth=0 handling |
| TASK-0175 | Implement generator - mixed_rules | UPDATED | Added R60 (size=0) |
| TASK-0176 | Implement generators - tree_sum and tree_sum_balanced | UPDATED | Added R60 (size=0) |
| TASK-0177 | Implement generators - erasure_propagation and Church encodings | UPDATED | Added R42a, R60, fixed Church cross-refs (R26 -> R4/R15/R16) |
| TASK-0178 | Define CLI argument structs for I/O subcommands | UPDATED | format -> input_format (SC-017), added R44a timing scope, IoError -> FileIoError |
| TASK-0179 | Integration tests for I/O roundtrips and generators | UPDATED | Added T10-T14 coverage (5 new test requirements) |
| TASK-0104 | Implement net serialization/deserialization helpers | NO CHANGE | SPEC-07 task; not affected by SPEC-12 changes |
| TASK-0117 | Enforce Core/Infrastructure layer boundary | UPDATED | Added io module mixed-layer annotation per SPEC-12 Section 4.3 |

**Summary:** 19 tasks updated, 2 tasks unchanged, 0 tasks created, 0 tasks removed.

---

## 2. Details for Each Changed Task

### TASK-0160 (minor)
- **Change:** Module layout reference updated: `types.rs -- IoError, NetFormat, ParseError` -> `types.rs -- FileIoError, NetFormat`
- **Change:** Re-export note updated: `IoError` -> `FileIoError`
- **Root cause:** SC-004 (IoError renamed to FileIoError)

### TASK-0161 (significant)
- **Change:** Task title changed from "Define IoError, NetFormat..." to "Define FileIoError, NetFormat..."
- **Change:** All references to `IoError` replaced with `FileIoError` throughout the task
- **Change:** Added revision note explaining the rename rationale and SPEC-13 R17 composition note
- **Change:** Added note about `RelativistError::Io` -> `RelativistError::FileIo` migration
- **Root cause:** SC-004 (IoError renamed to FileIoError to avoid std::io::Error collision)

### TASK-0162 (significant)
- **Change:** Requirements line expanded: added R44a, R44b
- **Change:** `ReductionSummary` struct completely redesigned:
  - Removed `max_interactions_reached: bool` -> replaced by `normal_form: bool` + `termination_reason: Option<String>`
  - Removed `grid: Option<GridReductionInfo>` -> replaced by flat `Option` fields: `rounds`, `workers`, `speedup`, `efficiency`, `overhead_pct`
  - Removed `GridReductionInfo` struct entirely
- **Change:** Acceptance criteria rewritten to match new struct fields
- **Change:** Added notes for R44a (duration_secs scope) and R44b (conditional baseline metrics)
- **Root cause:** SC-011 (R44a/R44b added), SC-020 (ReductionSummary struct defined in spec)

### TASK-0163, TASK-0164, TASK-0167, TASK-0168 (minor)
- **Change:** All `IoError` references replaced with `FileIoError`
- **Root cause:** SC-004

### TASK-0165 (significant)
- **Change:** Requirements line expanded: added R54, R55, R56, R57, R58, R59
- **Change:** All `IoError` references replaced with `FileIoError`
- **Change:** 6 new acceptance criteria added:
  - R54: at most one root declaration per file
  - R55: no root -> net.root = None
  - R56: root port must be valid, AgentPort and FreePort permitted
  - R57: root is not a wire, not counted in wire count
  - R58: self-loop rejection
  - R59: free-to-free wire rejection
- **Change:** 5 new test expectations added (T11a/b/c, T12, T13)
- **Change:** Notes section expanded with R54-R59 implementation details
- **Root cause:** SC-006 (R54-R57), SC-012 (R58-R59), SC-004, SC-005

### TASK-0169 (moderate)
- **Change:** Wire count AC updated with precise definition from R29 v2 (canonical ordering, FreePort exclusion)
- **Change:** Added arity-aware iteration per R61
- **Change:** Free port count also updated to iterate only over valid port range
- **Root cause:** SC-013 (R29 wire count clarification), SC-010 (R61 arity-aware validation)

### TASK-0170 (moderate)
- **Change:** Requirements line expanded: added R44a, R44b
- **Change:** Acceptance criteria updated:
  - Grid field checks changed from `grid: Some(...)` to individual `rounds`, `speedup`, etc.
  - max_interactions_reached replaced by normal_form + termination_reason
  - Added conditional Speedup/Efficiency/Overhead display (R44b)
  - Added R44a timing scope note
- **Change:** Test expectations updated to cover R44b conditional display
- **Change:** Dependencies: `GridReductionInfo` removed from imports
- **Root cause:** SC-011 (R44a/R44b), SC-020 (struct redesign)

### TASK-0171 (moderate)
- **Change:** Requirements expanded: added R60, R61
- **Change:** Church numeral cross-references fixed: SPEC-14 R26 -> R4, R15, R16 with size warnings
- **Change:** Added AC for size=0 producing empty net (R60)
- **Change:** Added AC for arity-aware validation in debug assertions (R61)
- **Root cause:** SC-016, SC-018, SC-010

### TASK-0172 (moderate)
- **Change:** Requirements expanded: added R38a, R60
- **Change:** Updated DUP generator reference from "R33" to "R38a" (now has its own requirement with pseudocode)
- **Change:** Added AC for size=0 producing empty net (R60)
- **Root cause:** SC-015 (R38a), SC-018 (R60)

### TASK-0173, TASK-0174, TASK-0175, TASK-0176 (minor)
- **Change:** Requirements expanded: added R60
- **Change:** TASK-0174 also updated depth=0 handling note per R60 vs R39
- **Root cause:** SC-018 (R60)

### TASK-0177 (moderate)
- **Change:** Requirements expanded: added R42a, R60
- **Change:** Church cross-references fixed: R26 -> R4/R15/R16
- **Change:** Added revision note explaining all changes
- **Change:** Added ACs for R60 (size=0) and R42a (erasure_propagation pseudocode)
- **Root cause:** SC-007 (R42a pseudocode), SC-016 (cross-ref fix), SC-018 (R60)

### TASK-0178 (moderate)
- **Change:** Requirements expanded: added R44a, R44b
- **Change:** ReduceArgs.format renamed to input_format (SC-017)
- **Change:** IoError -> FileIoError throughout
- **Change:** Added AC for R44a timing scope (exclude file I/O from duration_secs)
- **Change:** --format note updated to --input-format
- **Root cause:** SC-017 (field rename), SC-004 (error type rename), SC-011 (R44a)

### TASK-0179 (significant)
- **Change:** Requirements expanded: added T10, T11, T12, T13, T14
- **Change:** 5 new test acceptance criteria added
- **Change:** Context updated with revision note
- **Change:** "All 9 test requirements" updated to "All 14 test requirements"
- **Root cause:** SC-006 (T11), SC-012 (T12, T13), SC-018 (T10, T14)

### TASK-0117 (minor)
- **Change:** Added mixed-layer annotation for io module in lib.rs template
- **Root cause:** SC-001 (io module layer classification per Section 4.3)

---

## 3. New Tasks

No new tasks were created. All new SPEC-12 v2 requirements (R38a, R42a, R44a, R44b, R54-R61, T10-T14) map cleanly to existing tasks via scope expansion:

| New Requirement | Covered By | Method |
|----------------|------------|--------|
| R38a (ep_annihilation_dup pseudocode) | TASK-0172 | Existing task already implemented this generator; requirement reference added |
| R42a (erasure_propagation pseudocode) | TASK-0177 | Existing task already implemented this generator; pseudocode now available |
| R44a (duration_secs scope) | TASK-0170, TASK-0178 | Added to acceptance criteria |
| R44b (conditional baseline metrics) | TASK-0162, TASK-0170 | ReductionSummary struct redesigned; formatting updated |
| R54 (one root per file) | TASK-0165 | Added to acceptance criteria and tests |
| R55 (no root = None) | TASK-0165 | Added to acceptance criteria and tests |
| R56 (root port validation) | TASK-0165 | Added to acceptance criteria |
| R57 (root is not a wire) | TASK-0165 | Added to acceptance criteria |
| R58 (self-loop rejection) | TASK-0165 | Added to acceptance criteria and tests |
| R59 (free-to-free rejection) | TASK-0165 | Added to acceptance criteria and tests |
| R60 (size=0 for all generators) | TASK-0171-0177, TASK-0179 | Added to all generator tasks and integration tests |
| R61 (arity-aware validation) | TASK-0171, TASK-0169 | Added to validation logic and summary computation |
| T10 (size zero test) | TASK-0179 | Added to integration test requirements |
| T11 (root declaration test) | TASK-0165, TASK-0179 | Added to parser tests and integration tests |
| T12 (self-loop test) | TASK-0165, TASK-0179 | Added to parser tests and integration tests |
| T13 (free-to-free test) | TASK-0165, TASK-0179 | Added to parser tests and integration tests |
| T14 (empty net test) | TASK-0179 | Added to integration test requirements |

---

## 4. Requirement Coverage Verification

### SPEC-12 Revised v2 -- Complete Requirement-to-Task Mapping

| Requirement | Level | Task(s) | Covered? |
|-------------|-------|---------|----------|
| R1 (three input formats) | MUST | TASK-0168 | YES |
| R2 (binary = serde+bincode) | MUST | TASK-0163 | YES |
| R3 (binary = .bin) | MUST | TASK-0163 | YES |
| R4 (binary roundtrip) | MUST | TASK-0163 | YES |
| R5 (binary default for benchmarks) | SHOULD | TASK-0163 | YES |
| R6 (text DSL = .ic) | MUST | TASK-0164 | YES |
| R7 (text DSL grammar) | MUST | TASK-0164, TASK-0165 | YES |
| R8 (port name mapping) | MUST | TASK-0164, TASK-0165 | YES |
| R9 (ERA auxiliary rejection) | MUST | TASK-0165 | YES |
| R10 (sequential AgentId) | MUST | TASK-0164, TASK-0165 | YES |
| R11 (T1/I2 validation) | MUST | TASK-0165 | YES |
| R12 (comments and blanks) | MUST | TASK-0164 | YES |
| R13 (CON-CON example) | Informative | TASK-0165 | YES |
| R14 (CON-DUP example) | Informative | TASK-0165 | YES |
| R15 (serializer roundtrip) | MUST | TASK-0166 | YES |
| R16 (JSON format) | MUST | TASK-0167 | YES |
| R17 (JSON deferral) | MAY | TASK-0167 | YES |
| R18 (output format by extension) | MUST | TASK-0168, TASK-0178 | YES |
| R19 (no --output = no file) | MUST | TASK-0178 | YES |
| R20 (print reduction summary) | MUST | TASK-0170 | YES |
| R21 (--metrics JSON) | SHOULD | TASK-0170, TASK-0178 | YES |
| R22 (per-round CSV) | SHOULD | TASK-0105 | YES |
| R23 (reduce = local reduce_all) | MUST | TASK-0178 | YES |
| R24 (ReduceArgs struct) | MUST | TASK-0178 | YES |
| R25 (reduce prints summary) | MUST | TASK-0178, TASK-0170 | YES |
| R26 (--max-interactions) | MUST | TASK-0178 | YES |
| R27 (inspect = load + stats) | MUST | TASK-0178 | YES |
| R28 (InspectArgs struct) | MUST | TASK-0178, TASK-0161 | YES |
| R29 (inspect statistics) | MUST | TASK-0169 | YES |
| R30 (inspect JSON output) | MUST | TASK-0169 | YES |
| R31 (inspect text output) | MUST | TASK-0169 | YES |
| R32 (generate subcommand) | MUST | TASK-0178 | YES |
| R33 (GenerateArgs + ExampleNet enum) | MUST | TASK-0171, TASK-0178 | YES |
| R34 (generator validation) | MUST | TASK-0171-0177 | YES |
| R35 (generator pure functions) | MUST | TASK-0171-0177 | YES |
| R36 (generators in io/examples.rs) | MUST | TASK-0171 | YES |
| R37 (ep_annihilation) | MUST | TASK-0171 | YES |
| R38 (ep_annihilation_con) | MUST | TASK-0172 | YES |
| R38a (ep_annihilation_dup) | MUST | TASK-0172 | YES |
| R39 (dual_tree) | MUST | TASK-0174 | YES |
| R40 (con_dup_expansion) | MUST | TASK-0173 | YES |
| R41 (mixed_rules) | MUST | TASK-0175 | YES |
| R42 (tree_sum, tree_sum_balanced) | MUST | TASK-0176 | YES |
| R42a (erasure_propagation) | MUST | TASK-0177 | YES |
| R43 (generate confirmation) | MUST | TASK-0178 | YES |
| R44 (reduction summary) | MUST | TASK-0170 | YES |
| R44a (duration_secs scope) | MUST | TASK-0170, TASK-0178 | YES |
| R44b (conditional baseline metrics) | MUST | TASK-0162, TASK-0170 | YES |
| R45 (summary format) | MUST | TASK-0170 | YES |
| R46 (grid metrics in summary) | MUST | TASK-0170 | YES |
| R47 (not-normal-form reason) | MUST | TASK-0170 | YES |
| R48 (format auto-detection) | MUST | TASK-0161, TASK-0168 | YES |
| R49 (unrecognized extension error) | MUST | TASK-0161, TASK-0168 | YES |
| R50 (--input-format override) | MUST | TASK-0161, TASK-0168, TASK-0178 | YES |
| R51 (io module API) | MUST | TASK-0165, TASK-0166, TASK-0168 | YES |
| R52 (FileIoError type) | MUST | TASK-0161 | YES |
| R53 (compute subcommand) | MUST | TASK-0210 (Phase 11) | YES |
| R54 (one root per file) | MUST | TASK-0165 | YES |
| R55 (no root = None) | MUST | TASK-0165 | YES |
| R56 (root port validation) | MUST | TASK-0165 | YES |
| R57 (root is not a wire) | MUST | TASK-0165 | YES |
| R58 (self-loop rejection) | MUST | TASK-0165 | YES |
| R59 (free-to-free rejection) | MUST | TASK-0165 | YES |
| R60 (size=0 for generators) | MUST | TASK-0171-0177 | YES |
| R61 (arity-aware T1 validation) | MUST | TASK-0171, TASK-0169 | YES |
| T1 (binary roundtrip) | MUST | TASK-0179 | YES |
| T2 (text DSL roundtrip) | MUST | TASK-0179 | YES |
| T3 (text DSL errors) | MUST | TASK-0179 | YES |
| T4 (generator validity) | MUST | TASK-0179 | YES |
| T5 (inspect correctness) | MUST | TASK-0179 | YES |
| T6 (reduce correctness) | MUST | TASK-0179 | YES |
| T7 (max-interactions) | MUST | TASK-0179 | YES |
| T8 (format detection) | MUST | TASK-0179 | YES |
| T9 (generator consistency) | MUST | TASK-0179 | YES |
| T10 (size zero) | MUST | TASK-0179 | YES |
| T11 (root declarations) | MUST | TASK-0165, TASK-0179 | YES |
| T12 (self-loop) | MUST | TASK-0165, TASK-0179 | YES |
| T13 (free-to-free) | MUST | TASK-0165, TASK-0179 | YES |
| T14 (empty net) | MUST | TASK-0179 | YES |

**Coverage: 100% -- All 72 requirements (R1-R61, T1-T14) are mapped to at least one task.**

---

## 5. Cross-Spec Impact Notes

### SPEC-13 R17 (RelativistError)
- TASK-0103 (Define RelativistError) will need updating when the `io` module is integrated: `Io(#[from] std::io::Error)` must be replaced by `FileIo(#[from] FileIoError)`. This is documented in TASK-0161 notes but not enforced by a separate task, since TASK-0103 is in Phase 6 (CLI & Config) and will naturally incorporate this when the io module is ready.

### SPEC-13 R5 (Module list)
- The `io` module is not in SPEC-13 R5's original 11-module list. SPEC-12 v2 Section 1 documents the required amendment. This affects TASK-0117 (layer boundary enforcement) which was updated to annotate the io module's mixed Core/Infrastructure nature.

### SPEC-07 R22-R25 and SPEC-13 R42 (Superseded)
- SPEC-12 R1-R50 supersede SPEC-07 R22-R25 and SPEC-13 R42. TASK-0104 (net serialization helpers) was not modified because it implements the SPEC-07-era binary-only serialization, which is a subset of SPEC-12's capabilities. When TASK-0168 (load_net/save_net dispatch) is completed, TASK-0104's functionality will be subsumed.
