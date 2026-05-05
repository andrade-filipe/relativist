# SPEC-00 -- Round 1: Spec Critic Review

**Date:** 2026-04-05
**Reviewer:** Spec Critic (adversarial)
**Target:** SPEC-00-glossary.md (status: Revised v2)
**Predecessors consulted:** SPEC-01 through SPEC-14 (all specs), BACKLOG.md

---

## Overall Assessment

SPEC-00 was written early as the terminological foundation for the project. It covers IC theory (Domain 1-3), distribution concepts (Domain 4), grid infrastructure (Domain 5), the formal argument framework (Domain 6), and encoding/readback (Domain 6b, added by amendment). The glossary is strong on theoretical and partitioning terminology -- Domains 1-5 are thorough and well-referenced. However, since 14 additional specs have been written and revised since SPEC-00 was last updated, significant terminology gaps have emerged. Entire categories of terms from the wire protocol (SPEC-06), system architecture (SPEC-13), security (SPEC-10), observability (SPEC-11), user I/O (SPEC-12), benchmarks (SPEC-09), and test strategy (SPEC-08) are used across specs but have no glossary entry. The glossary also contains stale "Rust (proposed)" annotations that contradict confirmed design decisions in SPEC-02, and the PortRef encoding description is inconsistent with the authoritative definition in SPEC-02.

The glossary is normative ("This glossary is normative for all other specs"), which means every term used in any spec should either be defined here or in the local definitions section of the spec that introduces it. While some specs do define their own local terms (e.g., SPEC-02 defines "Port Array", "Stale Redex"; SPEC-03 defines "Interaction Counter", "link"), many cross-spec terms that appear in 3+ specs have no canonical home.

**Verdict:** NEEDS REVISION

---

## Issues

### SC-001: WorkerId not defined in glossary despite cross-spec usage
**Severity:** HIGH
**Axis:** Completeness
**Problem:** The term `WorkerId` is a fundamental type used across SPEC-04 (R16, R17, R18 -- type alias, ID range partitioning), SPEC-05 (R37 -- WorkerRoundStats), SPEC-06 (R36 -- NodeConfig field), SPEC-08 (numerous tests), SPEC-09 (benchmark parameters), SPEC-11 (structured log fields), and SPEC-13 (R20, R21, R24 -- FSM events and transitions). SPEC-04 Section 4.1 defines it as `pub type WorkerId = u32`. Yet the glossary has no entry for WorkerId, even though it defines AgentId (Section 3.14) and the conceptual Worker (Section 7.4).
**Impact if unresolved:** An implementer looking up "WorkerId" in the canonical glossary finds nothing. They must discover the type alias buried in SPEC-04 Section 4.1. Since the glossary defines AgentId with its type and constraints, the absence of WorkerId creates an asymmetry that may cause the implementer to guess at the type.
**Suggested resolution:** Add a WorkerId entry to Domain 5 (Grid Infrastructure), between Worker and Round. Definition: "Unique identifier for a Worker within a Grid execution. Type alias `u32`, values in `[0, n-1]` where n is the number of workers." Cross-reference SPEC-04 Section 4.1.

---

### SC-002: PortRef encoding description contradicts SPEC-02
**Severity:** HIGH
**Axis:** Consistency
**Problem:** SPEC-00 Section 3.11 defines PortRef's Rust representation as: "Compact encoding: `u32` with `(val << TAG_BITS) | tag`, where tag distinguishes CON/DUP/ERA/VAR (AC-015 CC-1)." However, SPEC-02 Section 4.1 (the authoritative spec for net representation) defines PortRef as a Rust enum:
```rust
pub enum PortRef {
    AgentPort(AgentId, PortId),
    FreePort(u32),
}
```
The compact u32 encoding with CON/DUP/ERA/VAR tags is from AC-015 (a code analysis of HVM2), not from the confirmed Relativist design. SPEC-02 explicitly chose the enum representation. The tags CON/DUP/ERA/VAR make no sense for PortRef (those are Symbol values, not port reference kinds). The glossary description conflates two different encoding schemes.
**Impact if unresolved:** An implementer reading the glossary will attempt to use a compact u32 bit-packed PortRef, then discover SPEC-02 mandates an enum. This creates confusion about which is authoritative for the Rust type.
**Suggested resolution:** Update the "Rust (proposed)" row of Section 3.11 to match SPEC-02 R4: `enum PortRef { AgentPort(AgentId, PortId), FreePort(u32) }`. Remove the compact encoding description or relegate it to a "Note: AC-015 proposes a compact u32 encoding as a future optimization."

---

### SC-003: Overhead Profile / Workload Profile not defined in glossary
**Severity:** HIGH
**Axis:** Completeness
**Problem:** The concept of Overhead Profiles A/B/C is central to the experimental evaluation and is referenced across SPEC-08 (Workload Profile definition, test organization), SPEC-09 (R8 -- mandatory benchmarks organized by profile), SPEC-12 (example generators by profile), SPEC-14 (Arithmetic Net references Profile B), and even within SPEC-00 itself (Section 8b.4 Arithmetic Net: "Exhibits Profile B overhead behavior"). Yet the glossary has no entry for Overhead Profile, Profile A, Profile B, or Profile C. The Arithmetic Net entry references "Overhead Profile" in its Related To field, but that term has no glossary definition.
**Impact if unresolved:** A reader encountering "Profile B" in the glossary's own Arithmetic Net definition has no way to look it up in the same glossary. The self-referential gap undermines the glossary's role as a self-contained vocabulary.
**Suggested resolution:** Add an entry "Overhead Profile" to a new Domain 7 (Benchmarks and Metrics) or to Domain 5. Define the three profiles:
- **Profile A (Embarrassingly Parallel):** Independent redexes, 1 round, zero border redexes.
- **Profile B (Expansion with Collapse):** CON-DUP dominant, multiple rounds, emergent borders.
- **Profile C (Sequential Dependency):** Level-dependent, many rounds, massive borders.
Cross-reference ARG-004 Part I Step 6, DISC-006 v2 Section 4.1.

---

### SC-004: "Rust (proposed)" labels are stale -- decisions are confirmed
**Severity:** MEDIUM
**Axis:** Currency
**Problem:** The glossary uses "Rust (proposed)" for 12 type descriptions (Sections 3.1, 3.2, 3.7, 3.10, 3.11, 3.12, 3.13, 3.14, 6.2, 8b.1, 8b.3, 8b.4). However, SPEC-02 explicitly labels these as "confirmed decisions" (e.g., SPEC-02 R6: "confirmed decision"; SPEC-00 Section 3.12 Net: "confirmed decision"). Some entries self-contradict: the Net definition says "(confirmed decision)" in the definition text but labels the row "Rust (proposed)." The distinction between "proposed" and "confirmed" matters because an implementer may treat "proposed" types as negotiable.
**Impact if unresolved:** An implementer may attempt to change a "proposed" type (e.g., use a different PortRef encoding), not realizing the decision has been confirmed by SPEC-02 and is now mandatory.
**Suggested resolution:** Replace all "Rust (proposed)" labels with "Rust" or "Rust (confirmed)" for types that SPEC-02 has finalized. Only types genuinely unresolved should retain "proposed."

---

### SC-005: Grid Loop / Grid Cycle not defined in glossary
**Severity:** MEDIUM
**Axis:** Completeness
**Problem:** The term "Grid Loop" (or "Grid Cycle") is used extensively across SPEC-05 (R24-R33 -- entire section 3.5 titled "Grid Loop"), SPEC-06 (R19, R24, R25 -- connection lifetime, abort), SPEC-07 (R13 step 3 -- "start the grid loop"), SPEC-08, SPEC-09, and SPEC-13 (R32, R40). SPEC-05 Section 2 defines it locally as "The outer loop that repeats Rounds until Normal Form." However, it is not in the glossary. The glossary defines "Round" (Section 7.5) but not the loop that repeats rounds.
**Impact if unresolved:** A reader must locate the definition in SPEC-05 Section 2. Since "Grid Loop" appears across 6+ specs, it deserves a glossary entry for quick lookup.
**Suggested resolution:** Add a "Grid Loop" entry to Domain 5 referencing SPEC-05 R24-R30. Definition: "The outer loop that repeats Rounds of the BSP cycle (split, distribute, reduce, collect, merge, resolve borders) until the net reaches Normal Form. Equivalent to `run_grid` (SPEC-05 R24)."

---

### SC-006: Emergent Border Redex not defined in glossary
**Severity:** MEDIUM
**Axis:** Completeness
**Problem:** The glossary defines "Border Redex" (Section 6.7) and mentions emergent border redexes within that definition ("Emergent: Local reduction creates new agents whose principal ports connect to boundary FreePorts"). However, "Emergent Border Redex" is a distinct concept with its own nuances (only detectable after merge, may arise from CON-DUP commutation, critical for P3). SPEC-05 Section 2 gives it a full definition. SPEC-01 D3, ARG-003, and DISC-005 all reference it as a separate concept from pre-existing border redexes.
**Impact if unresolved:** The distinction between pre-existing and emergent border redexes is critical for understanding why multiple rounds may be necessary. Lumping them into a single Border Redex entry buries the emergent concept.
**Suggested resolution:** Either (a) add a separate "Emergent Border Redex" entry to Domain 4, or (b) expand the Border Redex entry with explicit sub-definitions (similar to how Annihilation has sub-rules), clearly distinguishing "Pre-existing" and "Emergent" as named sub-categories.

---

### SC-007: BSP / Superstep not defined in glossary
**Severity:** MEDIUM
**Axis:** Completeness
**Problem:** BSP (Bulk Synchronous Parallel) is the programming model classification for Relativist, established in SPEC-13 R1-R4 and sourced from PESQ-012. The term "BSP" and "Superstep" appear in SPEC-13 (definitions + R1-R4), SPEC-11 (span fields reference "grid_cycle" supersteps), and SPEC-09 (benchmark modes reference BSP barrier semantics). The glossary has no entry for BSP or Superstep. The closest is "Round" (Section 7.5), but Round does not mention BSP.
**Impact if unresolved:** A reader encountering "BSP superstep" in SPEC-13 must look up the term externally or in SPEC-13's local definitions. Since BSP is the project's declared programming model, it should be in the canonical glossary.
**Suggested resolution:** Add "BSP (Bulk Synchronous Parallel)" to Domain 5. Definition: "A parallel programming model where computation proceeds in supersteps: local computation, communication, barrier synchronization. Relativist implements BSP where each Round (Section 7.5) is one superstep." Cross-reference PESQ-012, SPEC-13 R1.

---

### SC-008: Core Layer / Infrastructure Layer not defined in glossary
**Severity:** MEDIUM
**Axis:** Completeness
**Problem:** The Core Layer / Infrastructure Layer separation is a fundamental architectural constraint introduced by SPEC-13 R6-R8 and referenced throughout:
- SPEC-13 R6: Core Layer MUST NOT depend on tokio
- SPEC-13 R34: Core Layer MUST be fully synchronous
- SPEC-14 R1: encoding module MUST reside in the Core Layer
- SPEC-12: pure parts of `io/` in Core Layer, impure parts in Infrastructure Layer
- SPEC-11: observability module spans both layers

The glossary has no entries for "Core Layer" or "Infrastructure Layer."
**Impact if unresolved:** These architectural concepts govern which modules can depend on what. Without glossary entries, a developer must find them in SPEC-13 Section 2.
**Suggested resolution:** Add entries for "Core Layer" and "Infrastructure Layer" to Domain 5 or a new Domain 7 (Architecture). Brief definitions with cross-reference to SPEC-13 R5-R8.

---

### SC-009: Transport trait / TcpTransport / ChannelTransport not defined in glossary
**Severity:** MEDIUM
**Axis:** Completeness
**Problem:** The Transport trait is the network abstraction layer defined in SPEC-13 R28. It has two implementations: TcpTransport (production) and ChannelTransport (testing). These are referenced across SPEC-06 (R16-R22 -- TCP transport), SPEC-08 (integration tests use ChannelTransport), SPEC-09 (Mode enum references Local=ChannelTransport, TcpLocalhost/TcpNetwork=TcpTransport), SPEC-10 (R28a -- TLS wraps TcpTransport only), SPEC-11, SPEC-12, and SPEC-13 (R28-R31, R52). The glossary has no entry for Transport, TcpTransport, or ChannelTransport. SPEC-13 Section 2 defines "Transport" locally, but given its cross-spec usage, it should be in the glossary.
**Impact if unresolved:** The Transport abstraction is a central design decision (enables testing without TCP). Its absence from the glossary means developers must discover it in SPEC-13.
**Suggested resolution:** Add "Transport" to Domain 5 with sub-entries or notes for TcpTransport and ChannelTransport.

---

### SC-010: FreePort (Boundary) Rust encoding contradicts SPEC-02
**Severity:** MEDIUM
**Axis:** Consistency
**Problem:** SPEC-00 Section 6.2 says the Rust representation is "Dedicated FPORT tag in the compact encoding, or sentinel value in the VAR space (see AC-015 Z7)." This references the compact u32 encoding from AC-015, which was NOT adopted. SPEC-02 R4 defines FreePort as a variant of the `enum PortRef { AgentPort(AgentId, PortId), FreePort(u32) }`. The FPORT tag description is wrong.
**Impact if unresolved:** Same family as SC-002 -- reinforces the stale compact encoding narrative.
**Suggested resolution:** Update the Rust row to: `FreePort(u32)` variant of `enum PortRef`. Remove the FPORT tag reference.

---

### SC-011: MIPS / Speedup / Efficiency / Border Ratio / Break-Even Point not defined
**Severity:** MEDIUM
**Axis:** Completeness
**Problem:** SPEC-09 (Benchmarks) defines multiple quantitative metrics that are central to the TCC's experimental evaluation: MIPS (Millions of Interactions Per Second), Speedup, Efficiency, Border Ratio, Overhead Ratio, Break-Even Point, and Scaling Curve. These terms appear in SPEC-09 (R4, R6 parameters, Section 3.3 metrics), SPEC-12 (Reduction Summary prints MIPS), and SPEC-08 (benchmark verification). None are in the glossary.
**Impact if unresolved:** These are the metrics that will populate Section 4 of the TCC paper. Their absence from the glossary means there is no single canonical definition of "Speedup" or "MIPS" -- only SPEC-09's local definitions.
**Suggested resolution:** Add a "Domain 7: Benchmarks and Metrics" section to the glossary, or append entries to Domain 5. At minimum, define: MIPS, Speedup, Efficiency, Border Ratio, Overhead Ratio, Break-Even Point.

---

### SC-012: Wire Protocol / Frame / Message not defined in glossary
**Severity:** MEDIUM
**Axis:** Completeness
**Problem:** The wire protocol is the binary communication layer between coordinator and workers. "Wire Protocol," "Frame," and "Message" are defined locally in SPEC-06 Section 2 and used across SPEC-06, SPEC-10 (R19 extends Message enum), SPEC-11 (protocol metrics), SPEC-13 (R28, transport abstraction), and SPEC-08 (round-trip serialization tests). None appear in the glossary. The glossary covers the Coordinator and Worker roles but not how they communicate.
**Impact if unresolved:** A reader looking for the canonical definition of the Message enum finds no glossary entry. They must go to SPEC-06 Section 4.1.
**Suggested resolution:** Add "Wire Protocol," "Frame," and "Message" to Domain 5.

---

### SC-013: Error types (RelativistError, ProtocolError, etc.) not in glossary
**Severity:** LOW
**Axis:** Completeness
**Problem:** SPEC-13 R15-R17 defines 7 per-module error enums (NetError, ReductionError, PartitionError, MergeError, ProtocolError, CoordinatorError, WorkerError) plus a top-level RelativistError. SPEC-12 introduces FileIoError. SPEC-10 introduces SecurityError and TokenError. These error types are implementation-level details, but they appear across multiple specs and the backlog tasks. The glossary does not mention any error types.
**Impact if unresolved:** Minor. Error types are well-defined in SPEC-13 and each owning spec. However, the complete error taxonomy is scattered.
**Suggested resolution:** This is OPTIONAL. Consider adding a brief entry "Error Types" to the glossary with a cross-reference to SPEC-13 R15-R17, or leave it as-is since each spec defines its own errors locally.

---

### SC-014: SecurityConfig / SecurityTier / AuthToken not in glossary
**Severity:** LOW
**Axis:** Completeness
**Problem:** SPEC-10 introduces SecurityTier (the three-tier security model), SecurityConfig, AuthToken, and related concepts. These are used within SPEC-10 and referenced by SPEC-13 (R9, R37, R44, R45) and SPEC-11 (security log levels). The glossary has no security domain.
**Impact if unresolved:** Security concepts are self-contained in SPEC-10. Cross-spec references are limited. Low impact.
**Suggested resolution:** OPTIONAL. If a Domain 8 (Security) is desired, add entries for SecurityTier, AuthToken. Otherwise, SPEC-10's local definitions suffice.

---

### SC-015: Local Mode / Distributed Mode not in glossary
**Severity:** LOW
**Axis:** Completeness
**Problem:** SPEC-07 Section 2 defines "Local Mode" and "Distributed Mode" as distinct execution modes. These are referenced across SPEC-07, SPEC-08 (test tiers), SPEC-09 (Mode enum with Sequential, Local, TcpLocalhost, TcpNetwork), SPEC-12 (`reduce` subcommand vs `local`), and SPEC-13 (R41 vs R41a). The glossary does not define these terms.
**Impact if unresolved:** Minor ambiguity. "Local" can mean "reduce without grid" (SPEC-13 R41 `reduce`) or "grid simulation in-process" (SPEC-07 `local`). The glossary could clarify this distinction.
**Suggested resolution:** Add "Local Mode" and "Distributed Mode" to Domain 5, clearly distinguishing "Local Mode (in-memory grid simulation via `relativist local`)" from "Direct Reduction (no grid, via `relativist reduce`)."

---

### SC-016: PortRef encoding mentions VAR tag that does not exist in Relativist
**Severity:** LOW
**Axis:** Clarity
**Problem:** SPEC-00 Section 3.11 PortRef mentions "tag distinguishes CON/DUP/ERA/VAR." The VAR tag is an HVM2 concept (variables in lambda calculus). Relativist's pure IC system has no variables and no VAR type. The mention of VAR in the PortRef encoding is misleading -- it comes from AC-015's analysis of HVM2, not from Relativist's design.
**Impact if unresolved:** Confusion about whether Relativist has a VAR concept. It does not.
**Suggested resolution:** Remove the VAR reference from the PortRef encoding description (or update the entire row per SC-002).

---

### SC-017: IdRange not in glossary
**Severity:** LOW
**Axis:** Completeness
**Problem:** SPEC-04 Section 4.1 defines `struct IdRange { start: AgentId, end: AgentId }` as the exclusive range of AgentIds reserved for a worker. The glossary defines "ID Space Partitioning" (Section 6.11) conceptually but does not mention the IdRange type, which is used in SPEC-04 (Partition struct), SPEC-05, SPEC-06 (R12 -- serializable types), and the backlog.
**Impact if unresolved:** Minor. The concept is well-covered by "ID Space Partitioning." The type alias is an implementation detail.
**Suggested resolution:** OPTIONAL. Mention IdRange in the ID Space Partitioning entry or leave it to SPEC-04.

---

### SC-018: Mapping table does not include SPEC-10/11/12/13/14 types
**Severity:** LOW
**Axis:** Currency
**Problem:** The mapping table in Section 9 maps Lafont -> Haskell -> Rust for core IC types and partitioning types. It does not include any types introduced by SPEC-10 through SPEC-14: SecurityTier, AuthToken, Transport, CoordinatorState, WorkerState, FileIoError, NetFormat, Text DSL, Church Numeral encoding types, ComputeArgs, etc. The table's last row is `GridMetrics`.
**Impact if unresolved:** The mapping table is incomplete but still useful for its original scope (IC theory + distribution). The omission of later specs is expected since those specs were written after the glossary.
**Suggested resolution:** OPTIONAL. Either (a) extend the table with a "SPEC-10+" section for infrastructure/security/observability types, or (b) add a note: "This table covers core IC and distribution types. For infrastructure types introduced by SPEC-10 through SPEC-14, refer to each spec's Section 2 (Definitions)."

---

### SC-019: Arithmetic Net definition references "Overhead Profile" which has no glossary entry
**Severity:** LOW
**Axis:** Consistency (self-referential gap)
**Problem:** The Arithmetic Net entry (Section 8b.4) states "Exhibits Profile B overhead behavior" and lists "Overhead Profile" in Related To. But the glossary has no "Overhead Profile" entry. This is a self-referential gap: the glossary references a term that it does not define.
**Impact if unresolved:** Covered by SC-003 at the cross-spec level, but this is specifically a self-referential consistency issue within the glossary itself.
**Suggested resolution:** Resolved by SC-003 (adding the Overhead Profile entry).

---

### SC-020: Glossary says "Open Questions: None" but terminology gaps exist
**Severity:** LOW
**Axis:** Currency
**Problem:** Section 11 says "None. Domain 6 (Encoding & Readback) was added by amendment for SPEC-14. Additional terms may be introduced by future specs, provided they are registered here by amendment." This implies the glossary is complete. However, 14 specs have been written since, introducing dozens of terms not registered in the glossary. The statement is factually outdated.
**Impact if unresolved:** A reader may trust the "None" assertion and not look for missing terms.
**Suggested resolution:** Update Section 11 to acknowledge the gaps: "Multiple terms introduced by SPEC-06 through SPEC-14 are not yet registered in this glossary. A revision pass is planned to add entries for: WorkerId, BSP, Transport, Overhead Profile, Grid Loop, and metric definitions (MIPS, Speedup, etc.)."

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 0 |
| HIGH | 3 |
| MEDIUM | 9 |
| LOW | 8 |

---

## Mandatory (must fix before implementation)

- **SC-001:** Add WorkerId to glossary -- fundamental type used across 7+ specs with no canonical definition
- **SC-002:** Fix PortRef Rust encoding to match SPEC-02's enum definition, not AC-015's compact u32
- **SC-003:** Add Overhead Profile (A/B/C) to glossary -- referenced by the glossary's own Arithmetic Net entry and central to SPEC-09

## Recommended (should fix)

- **SC-004:** Replace "Rust (proposed)" with "Rust (confirmed)" for all types finalized by SPEC-02
- **SC-005:** Add Grid Loop entry -- used across 6+ specs
- **SC-006:** Expand Border Redex entry to distinguish pre-existing vs emergent, or add separate entry
- **SC-007:** Add BSP / Superstep entry -- declared programming model
- **SC-008:** Add Core Layer / Infrastructure Layer entries -- fundamental architectural constraint
- **SC-009:** Add Transport entry -- central abstraction layer
- **SC-010:** Fix FreePort (Boundary) Rust encoding to match SPEC-02
- **SC-011:** Add key benchmark metrics (MIPS, Speedup, Efficiency, Border Ratio, Break-Even)
- **SC-012:** Add Wire Protocol / Frame / Message entries
- **SC-020:** Update "Open Questions: None" to acknowledge pending terminology

## Optional (nice to have)

- **SC-013:** Error type taxonomy cross-reference
- **SC-014:** Security domain entries (SecurityTier, AuthToken)
- **SC-015:** Local Mode vs Distributed Mode vs Direct Reduction clarification
- **SC-016:** Remove VAR tag from PortRef description
- **SC-017:** Mention IdRange in ID Space Partitioning entry
- **SC-018:** Extend mapping table or add a scope note
- **SC-019:** Resolved by SC-003

---

## Checklist

### Completeness
- [x] IC theory terms (Agent, Symbol, Port, Wire, Net, etc.) -- comprehensive
- [x] Interaction rules (Annihilation, Commutation, Erasure, Void) -- comprehensive
- [x] Formal properties (Strong Confluence, Locality, Linearity, etc.) -- comprehensive
- [x] Distribution terms (Partition, Boundary, Border Redex, etc.) -- comprehensive
- [x] Grid roles (Coordinator, Worker, Grid, Node) -- present
- [x] Formal argument framework (P1-P6) -- comprehensive
- [x] Encoding/Readback terms (Church Numeral, Arithmetic Net, etc.) -- present (added by amendment)
- [ ] **FAIL:** WorkerId type not defined (SC-001)
- [ ] **FAIL:** Grid Loop / Grid Cycle not defined (SC-005)
- [ ] **FAIL:** Emergent Border Redex not defined as distinct concept (SC-006)
- [ ] **FAIL:** BSP / Superstep not defined (SC-007)
- [ ] **FAIL:** Core Layer / Infrastructure Layer not defined (SC-008)
- [ ] **FAIL:** Transport / TcpTransport / ChannelTransport not defined (SC-009)
- [ ] **FAIL:** Overhead Profile (A/B/C) not defined (SC-003)
- [ ] **FAIL:** Benchmark metrics (MIPS, Speedup, etc.) not defined (SC-011)
- [ ] **FAIL:** Wire Protocol / Frame / Message not defined (SC-012)

### Consistency
- [x] Agent, Symbol, AgentId, Net definitions match SPEC-02 usage
- [x] Partition, PartitionPlan, Merge definitions match SPEC-04/SPEC-05 usage
- [x] Formal argument P1-P6 definitions match SPEC-01 and ARG-001
- [x] Church Numeral, Encoding, Decoding definitions match SPEC-14
- [ ] **FAIL:** PortRef Rust encoding contradicts SPEC-02 R4 (SC-002)
- [ ] **FAIL:** FreePort (Boundary) Rust encoding contradicts SPEC-02 (SC-010)
- [ ] **FAIL:** "Rust (proposed)" labels stale for confirmed decisions (SC-004)
- [ ] **FAIL:** PortRef mentions VAR tag that doesn't exist in Relativist (SC-016)
- [ ] **FAIL:** Arithmetic Net references "Overhead Profile" with no glossary entry (SC-019)

### Clarity
- [x] Definitions are unambiguous for IC theory terms
- [x] FreePort (Lafont) vs FreePort (Boundary) distinction is clear and well-documented
- [x] Conditions C1-C3 are clear and formally stated
- [x] Mapping table provides clear cross-reference between nomenclatures
- [ ] **PARTIAL:** PortRef encoding description is confusing due to stale compact encoding reference (SC-002)

### Currency
- [x] Domain 6b added for SPEC-14 encoding terms
- [ ] **FAIL:** "Rust (proposed)" not updated to "confirmed" (SC-004)
- [ ] **FAIL:** Section 11 "Open Questions: None" is outdated (SC-020)
- [ ] **FAIL:** No terms from SPEC-06, SPEC-10, SPEC-11, SPEC-12, SPEC-13 infrastructure specs (SC-007 through SC-015)
