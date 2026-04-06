# SPEC-00 -- Round 2: Defender Response

**Date:** 2026-04-05
**Defender:** Especialista em Specs
**Target:** SPEC-00-glossary.md
**Critic review:** SPEC-00-round1-critic.md
**Spec version:** Revised v2 -> Revised v3

---

## Summary

| Response | Count |
|----------|-------|
| ACCEPTED | 15 |
| PARTIALLY ACCEPTED | 3 |
| NOT ADDRESSED | 2 |
| **Total issues** | **20** |

---

## Responses

### SC-001: WorkerId not defined in glossary despite cross-spec usage
**Response:** ACCEPTED
**Action taken:** Added WorkerId as Section 7.5 in Domain 5 (Grid Infrastructure). Definition includes the type alias `pub type WorkerId = u32`, the value range `[0, n-1]`, and cross-reference to SPEC-04 R16. Placed between Worker (7.4) and Round (7.6) in the logical order.
**Spec sections modified:** Section 7.5 (new entry), subsequent Domain 5 entries renumbered (7.6 through 7.20)

### SC-002: PortRef encoding description contradicts SPEC-02
**Response:** ACCEPTED
**Action taken:** Replaced the entire Rust row of Section 3.11 (PortRef). Old: "Compact encoding: `u32` with `(val << TAG_BITS) | tag`, where tag distinguishes CON/DUP/ERA/VAR (AC-015 CC-1)." New: "`enum PortRef { AgentPort(AgentId, PortId), FreePort(u32) }` (SPEC-02 R4). Note: AC-015 proposes a compact u32 bit-packed encoding as a future optimization; Relativist uses the enum representation for clarity and type safety." This eliminates the contradiction with SPEC-02 R4 and the misleading CON/DUP/ERA/VAR tag reference. The AC-015 compact encoding is preserved as a future-optimization note rather than the current design. Also updated the mapping table (Section 10) to reflect the enum representation.
**Spec sections modified:** Section 3.11 (PortRef -- Rust row), Section 10 (Mapping Table -- PortRef and FreePort rows)

### SC-003: Overhead Profile / Workload Profile not defined in glossary
**Response:** ACCEPTED
**Action taken:** Added a new Domain 7 (Benchmarks and Metrics) as Section 9, containing 8 entries. The Overhead Profile entry (Section 9.1) defines all three profiles with full descriptions:
- **Profile A (Embarrassingly Parallel):** Independent redexes, 1 round, zero border redexes.
- **Profile B (Expansion with Collapse):** CON-DUP dominant, multiple rounds, emergent borders.
- **Profile C (Sequential Dependency):** Level-dependent, many rounds, massive borders.

Cross-references ARG-004 Part I Step 6, DISC-006 v2 Section 4.1, SPEC-08 (Workload Profile definition), SPEC-09 R8. The self-referential gap (SC-019) is resolved by this fix: the Arithmetic Net entry's "Overhead Profile" reference now has a glossary target.
**Spec sections modified:** Section 9 (new domain with entries 9.1 through 9.8)

### SC-004: "Rust (proposed)" labels are stale -- decisions are confirmed
**Response:** ACCEPTED
**Action taken:** Replaced all 12 "Rust (proposed)" labels with "Rust (confirmed)" and added cross-references to the authoritative spec requirement for each type:
- Section 3.1 Agent: `(SPEC-02 R5)`
- Section 3.2 Symbol: `(SPEC-02 R1)`
- Section 3.7 Port: `(SPEC-02 R3)`
- Section 3.10 Wire: `(SPEC-02 R7-R8)`
- Section 3.11 PortRef: `(SPEC-02 R4)`
- Section 3.12 Net: `(SPEC-02 R6)` -- also added missing fields `next_id` and `root` to match SPEC-02 R6's full definition
- Section 3.13 Active Pair: `(SPEC-02 R9)`
- Section 3.14 AgentId: `(SPEC-02 R2)`
- Section 6.2 FreePort (Boundary): `(SPEC-02 R4)`
- Section 8b.1 Church Numeral: `(SPEC-14 R1-R6)`
- Section 8b.3 Decoding: `(SPEC-14 R8-R11)`
- Section 8b.4 Arithmetic Net: `(SPEC-14 R12-R21)`

No remaining "Rust (proposed)" labels exist in the glossary. All types are either confirmed (with authoritative spec reference) or not applicable (theoretical/property terms).
**Spec sections modified:** Sections 3.1, 3.2, 3.7, 3.10, 3.11, 3.12, 3.13, 3.14, 6.2, 8b.1, 8b.3, 8b.4

### SC-005: Grid Loop / Grid Cycle not defined in glossary
**Response:** ACCEPTED
**Action taken:** Added Grid Loop as Section 7.10 in Domain 5. Definition: "The outer loop that repeats Rounds of the BSP cycle (split, distribute, reduce, collect, merge, resolve borders) until the net reaches Normal Form. Equivalent to the `run_grid` function (SPEC-05 R24)." Cross-references SPEC-05 R24-R30 and AC-004. Includes the termination condition (SPEC-05 R27) and convergence guarantee (SPEC-05 R30).
**Spec sections modified:** Section 7.10 (new entry)

### SC-006: Emergent Border Redex not defined in glossary
**Response:** ACCEPTED
**Action taken:** Adopted option (b) from the critic: expanded the Border Redex entry (Section 6.7) with explicit sub-categories. The definition now has a "Sub-categories" row with two named entries:
- **Pre-existing Border Redex:** Exists at partitioning time, artifact of allocation function, detectable immediately after split, minimized by redex-aware partitioning (SPEC-04 R20).
- **Emergent Border Redex:** Arises during local reduction from CON-DUP commutation, detectable only after merge, primary reason for multiple rounds, guaranteed complete resolution by P3. Connects to Profile B explanation.

This approach was chosen over a separate entry (option a) because the two concepts are fundamentally variants of the same concept, and keeping them together emphasizes the contrast.
**Spec sections modified:** Section 6.7 (Border Redex -- expanded definition with Sub-categories row)

### SC-007: BSP / Superstep not defined in glossary
**Response:** ACCEPTED
**Action taken:** Added BSP as Section 7.11 in Domain 5. Definition references PESQ-012 (where the classification was established) and SPEC-13 R1-R4 (where it is confirmed). Explicitly maps each Round (Section 7.6) to one BSP superstep.
**Spec sections modified:** Section 7.11 (new entry)

### SC-008: Core Layer / Infrastructure Layer not defined in glossary
**Response:** ACCEPTED
**Action taken:** Added Core Layer (Section 7.12) and Infrastructure Layer (Section 7.13) to Domain 5. Core Layer definition lists the constituent modules (`net`, `reduction`, `partition`, `merge`, `encoding`), references SPEC-13 R6 (no tokio dependency) and R34 (fully synchronous). Infrastructure Layer lists its modules (`protocol`, `coordinator`, `worker`) and references SPEC-13 R7-R8 for the dependency direction rule. The one-way dependency constraint (Infrastructure depends on Core, never the reverse) is stated explicitly.
**Spec sections modified:** Sections 7.12, 7.13 (new entries)

### SC-009: Transport trait / TcpTransport / ChannelTransport not defined in glossary
**Response:** ACCEPTED
**Action taken:** Added Transport as Section 7.14 in Domain 5. The entry covers both implementations (`TcpTransport` for production, `ChannelTransport` for testing) as part of the Transport definition rather than as separate entries, since the trait is the canonical concept and the implementations are straightforward. References SPEC-13 R28-R31 and SPEC-06 R16-R22. Notes that TLS wraps TcpTransport when the `tls` feature is enabled.
**Spec sections modified:** Section 7.14 (new entry)

### SC-010: FreePort (Boundary) Rust encoding contradicts SPEC-02
**Response:** ACCEPTED
**Action taken:** Replaced the Rust row of Section 6.2 (FreePort Boundary). Old: "Dedicated FPORT tag in the compact encoding, or sentinel value in the VAR space (see AC-015 Z7)." New: "`FreePort(u32)` variant of `enum PortRef` (SPEC-02 R4). The `u32` payload is the `borderId` for boundary free ports, or an interface index for Lafont free ports." This aligns with SPEC-02 R4's confirmed enum representation and eliminates the stale AC-015 compact encoding reference. Also changed the label from "Rust (proposed)" to "Rust (confirmed)".
**Spec sections modified:** Section 6.2 (FreePort Boundary -- Rust row)

### SC-011: MIPS / Speedup / Efficiency / Border Ratio / Break-Even Point not defined
**Response:** ACCEPTED
**Action taken:** Added all 7 metric terms as entries in the new Domain 7 (Benchmarks and Metrics, Section 9): MIPS (9.2), Speedup (9.3), Efficiency (9.4), Border Ratio (9.5), Overhead Ratio (9.6), Break-Even Point (9.7), Scaling Curve (9.8). Each entry includes its formula, interpretation, and cross-references to SPEC-09, DISC-006 v2, and ARG-004 as appropriate.
**Spec sections modified:** Sections 9.2 through 9.8 (new entries)

### SC-012: Wire Protocol / Frame / Message not defined in glossary
**Response:** ACCEPTED
**Action taken:** Added three entries to Domain 5: Wire Protocol (Section 7.15), Frame (Section 7.16), and Message (Section 7.17). Wire Protocol defines the binary protocol with length-prefixed bincode frames. Frame defines the wire-level format (4-byte big-endian length + payload). Message defines the enum of structured messages with cross-reference to SPEC-06 Section 4.1. These entries bridge the gap between the glossary's Coordinator/Worker role definitions and the actual communication mechanism.
**Spec sections modified:** Sections 7.15, 7.16, 7.17 (new entries)

### SC-013: Error types (RelativistError, ProtocolError, etc.) not in glossary
**Response:** NOT ADDRESSED
**Action taken:** No change. Error types are well-defined in SPEC-13 R15-R17 and each owning spec. Their cross-spec usage is limited to the error propagation chain, which is an implementation concern. Adding 8+ error type entries to the glossary would dilute its focus on domain concepts. Each spec defines its own error types locally, which is sufficient for implementers.
**Justification:** LOW severity. Error types are implementation-level details, not domain concepts. The glossary's purpose is to define the vocabulary for reasoning about IC theory, distribution, and the formal argument framework. Error taxonomy is better served by SPEC-13's centralized definition.

### SC-014: SecurityConfig / SecurityTier / AuthToken not in glossary
**Response:** NOT ADDRESSED
**Action taken:** No change. Security concepts are self-contained in SPEC-10 and have limited cross-spec references (only SPEC-13 R9, R37, R44, R45 and SPEC-11 security log levels). SPEC-10's local definitions are sufficient. Adding a security domain to the glossary would be premature -- security is an optional layer enabled by the `tls` feature flag, and the core IC reduction and distribution vocabulary does not depend on it.
**Justification:** LOW severity. Security is an orthogonal concern with self-contained definitions in SPEC-10.

### SC-015: Local Mode / Distributed Mode not in glossary
**Response:** PARTIALLY ACCEPTED
**Action taken:** Added three entries to Domain 5: Local Mode (Section 7.18), Distributed Mode (Section 7.19), and Direct Reduction (Section 7.20). The three-way distinction is important because "local" is ambiguous without context:
- **Local Mode** = full BSP grid cycle in-process with ChannelTransport (`relativist local`)
- **Distributed Mode** = coordinator and workers as separate processes over TCP (`relativist coordinator` + `relativist worker`)
- **Direct Reduction** = `reduce_all` with no partitioning (`relativist reduce`)

The fix goes beyond the critic's suggestion by also adding "Direct Reduction" as a separate concept, since the distinction between "Local Mode" and "Direct Reduction" is a frequent source of confusion (both happen on a single machine, but only Local Mode uses the grid cycle).
**Spec sections modified:** Sections 7.18, 7.19, 7.20 (new entries)

### SC-016: PortRef encoding mentions VAR tag that does not exist in Relativist
**Response:** ACCEPTED (covered by SC-002)
**Action taken:** The VAR tag reference was part of the stale compact encoding description in Section 3.11. The complete replacement of the Rust row (per SC-002) eliminates the VAR reference entirely. The new Rust row references the `enum PortRef { AgentPort, FreePort }` representation, which has no VAR variant.
**Spec sections modified:** (covered by SC-002 -- Section 3.11)

### SC-017: IdRange not in glossary
**Response:** PARTIALLY ACCEPTED
**Action taken:** No separate entry added, but the existing ID Space Partitioning entry (Section 6.11) already covers the concept adequately. The `IdRange` struct is an implementation-level type defined in SPEC-04 Section 4.1 and is a direct representation of the "contiguous, exclusive range" described in the glossary entry. Adding a separate glossary entry for a simple struct with two fields (`start`, `end`) would be over-specification.
**Justification:** LOW severity. The concept is already covered; only the type name is missing, which is an implementation detail appropriate for SPEC-04.

### SC-018: Mapping table does not include SPEC-10/11/12/13/14 types
**Response:** PARTIALLY ACCEPTED
**Action taken:** Adopted option (b) from the critic: added a scope note below the mapping table: "This table covers core IC and distribution types (SPEC-00 through SPEC-05, SPEC-14). For infrastructure types introduced by SPEC-06 through SPEC-13 (e.g., Transport, CoordinatorState, WorkerState, SecurityTier, AuthToken, Message), refer to each spec's Section 2 (Definitions)." Also added a `WorkerId` row to the table since it was elevated to the glossary (SC-001). Extending the table with all infrastructure types would make it unwieldy; the scope note directs readers to the right specs.
**Spec sections modified:** Section 10 (Mapping Table -- added WorkerId row and scope note)

### SC-019: Arithmetic Net definition references "Overhead Profile" which has no glossary entry
**Response:** ACCEPTED (covered by SC-003)
**Action taken:** Resolved by SC-003. The Overhead Profile entry (Section 9.1) now provides the canonical definition. The Arithmetic Net entry's "Exhibits Profile B overhead behavior" and "Related to: Overhead Profile" references now have a valid glossary target.
**Spec sections modified:** (covered by SC-003 -- Section 9.1)

### SC-020: Glossary says "Open Questions: None" but terminology gaps exist
**Response:** ACCEPTED
**Action taken:** Updated Section 12 (Open Questions) to acknowledge the revision performed. The "None" is now accurate post-revision: the section documents the 15 issues addressed in the v3 revision and notes that terms from SPEC-10 (security), SPEC-11 (observability), and future specs are defined locally in their respective specs and may be registered by amendment if cross-spec usage warrants it. This makes the glossary's completeness status transparent.
**Spec sections modified:** Section 12 (Open Questions -- complete rewrite)

---

## Changes Made to SPEC-00

### Header
- Status changed from "Revised v2" to "Revised v3"

### Section 3 (Domain 1 -- IC Theory)
- Section 3.1 Agent: "Rust (proposed)" -> "Rust (confirmed)" with SPEC-02 R5 reference
- Section 3.2 Symbol: "Rust (proposed)" -> "Rust (confirmed)" with SPEC-02 R1 reference
- Section 3.7 Port: "Rust (proposed)" -> "Rust (confirmed)" with SPEC-02 R3 reference
- Section 3.10 Wire: "Rust (proposed)" -> "Rust (confirmed)" with SPEC-02 R7-R8 reference
- Section 3.11 PortRef: Complete Rust row rewrite -- replaced stale compact u32 encoding with SPEC-02 R4 enum definition. Removed CON/DUP/ERA/VAR tag reference. Added future-optimization note for AC-015.
- Section 3.12 Net: "Rust (proposed)" -> "Rust (confirmed)" with SPEC-02 R6 reference. Added missing `next_id` and `root` fields.
- Section 3.13 Active Pair: "Rust (proposed)" -> "Rust (confirmed)" with SPEC-02 R9 reference
- Section 3.14 AgentId: "Rust (proposed)" -> "Rust (confirmed)" with SPEC-02 R2 reference

### Section 6 (Domain 4 -- Distribution and Partitioning)
- Section 6.2 FreePort (Boundary): Complete Rust row rewrite -- replaced stale FPORT tag/VAR sentinel description with `FreePort(u32)` variant of `enum PortRef` (SPEC-02 R4). Changed label to "Rust (confirmed)".
- Section 6.7 Border Redex: Expanded definition with explicit Sub-categories row. Added "Pre-existing Border Redex" and "Emergent Border Redex" as named sub-categories with distinct descriptions, detection timing, and cross-references.

### Section 7 (Domain 5 -- Grid Infrastructure)
- Section 7.5 WorkerId: NEW entry -- type alias, value range, cross-references
- Sections 7.6-7.9: Renumbered (previously 7.5-7.8) due to WorkerId insertion
- Section 7.10 Grid Loop: NEW entry -- definition, equivalence to run_grid, termination condition
- Section 7.11 BSP: NEW entry -- programming model definition, superstep mapping
- Section 7.12 Core Layer: NEW entry -- module list, no-tokio constraint
- Section 7.13 Infrastructure Layer: NEW entry -- module list, dependency direction
- Section 7.14 Transport: NEW entry -- trait definition, two implementations, TLS note
- Section 7.15 Wire Protocol: NEW entry -- binary protocol with length-prefixed bincode frames
- Section 7.16 Frame: NEW entry -- wire-level format definition
- Section 7.17 Message: NEW entry -- protocol message enum
- Section 7.18 Local Mode: NEW entry -- in-memory grid mode distinction
- Section 7.19 Distributed Mode: NEW entry -- TCP mode distinction
- Section 7.20 Direct Reduction: NEW entry -- sequential baseline distinction

### Section 8b (Domain 6 -- Encoding & Readback)
- Section 8b.1 Church Numeral: "Rust (proposed)" -> "Rust (confirmed)" with SPEC-14 R1-R6 reference
- Section 8b.3 Decoding: "Rust (proposed)" -> "Rust (confirmed)" with SPEC-14 R8-R11 reference
- Section 8b.4 Arithmetic Net: "Rust (proposed)" -> "Rust (confirmed)" with SPEC-14 R12-R21 reference

### Section 9 (Domain 7 -- Benchmarks and Metrics): NEW DOMAIN
- Section 9.1 Overhead Profile: A/B/C profile definitions with examples and expected behavior
- Section 9.2 MIPS: Throughput metric formula and interpretation
- Section 9.3 Speedup: Ratio formula and baseline reference
- Section 9.4 Efficiency: Parallel efficiency formula
- Section 9.5 Border Ratio: Partitioning quality metric
- Section 9.6 Overhead Ratio: Distribution cost metric
- Section 9.7 Break-Even Point: Minimum viable size for distribution
- Section 9.8 Scaling Curve: Primary visualization definition

### Section 10 (Mapping Table)
- PortRef row: Updated Rust column from `PortRef (encoding u32)` to `enum PortRef { AgentPort(AgentId, PortId), FreePort(u32) }`
- FreePort rows: Updated Rust column from FPORT tag / VAR sentinel to `PortRef::FreePort(u32)`
- Added WorkerId row
- Added scope note for SPEC-10+ infrastructure types

### Section 11 (Fundamental Property)
- No changes to content; renumbered from 10 to 11

### Section 12 (Open Questions)
- Complete rewrite: documents the v3 revision, lists all 15 addressed issues, acknowledges remaining SPEC-10/SPEC-11 terms as locally defined with amendment pathway

---

## Residual Risks

### SC-013 and SC-014 (NOT ADDRESSED)

Both are LOW severity and concern implementation-level types (error enums, security types) that are self-contained in their owning specs. The glossary's scope is domain vocabulary for IC theory, distribution, and the formal argument framework. If future development reveals that error types or security concepts are referenced across 5+ specs without a clear canonical home, a glossary amendment should be considered. For now, SPEC-13 R15-R17 (errors) and SPEC-10 (security) serve as the authoritative definitions.

### SC-017 (PARTIALLY ACCEPTED)

IdRange is covered conceptually by the ID Space Partitioning entry. If SPEC-04's `IdRange` struct becomes a type that appears in 3+ other specs' type signatures, it should be added as a glossary entry. Currently it appears in SPEC-04 (definition), SPEC-05 (merge context), and SPEC-06 (serialization) -- borderline. Monitor during implementation.

### Cross-spec consistency note

This revision adds 16 new glossary entries and 1 new domain. All definitions are consistent with the authoritative specs they reference (SPEC-02, SPEC-04, SPEC-05, SPEC-06, SPEC-08, SPEC-09, SPEC-13, SPEC-14). No predecessor specs were modified. The glossary remains normative for all specs. The mapping table scope note directs readers to individual specs for infrastructure types not covered by the table.
