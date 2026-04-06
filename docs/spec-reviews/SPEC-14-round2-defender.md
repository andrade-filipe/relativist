# SPEC-14 -- Round 2: Defender Response

**Date:** 2026-04-05
**Defender:** Especialista em Specs
**Target:** SPEC-14-encoding.md
**Critic review:** SPEC-14-round1-critic.md
**Spec version:** Draft v1 -> Revised v2

---

## Summary

| Response | Count |
|----------|-------|
| ACCEPTED | 14 |
| PARTIALLY ACCEPTED | 2 |
| NOT ADDRESSED | 2 |
| **Total issues** | **18** |

---

## Responses

### SC-001: R9 contradicts the Section 4.1 algorithm and SPEC-02 root port design
**Response:** ACCEPTED
**Action taken:** R9 rewritten to specify `net.root = Some(PortRef::AgentPort(lam_f, 0))`, matching SPEC-02 Section 4.9. All port connection tables updated: `CON_0.p0` column now shows `root (DISCONNECTED in port array)` instead of `FreePort(0) [root]`. Added explicit note explaining that the root agent's principal port has no internal peer in the port array (contains `DISCONNECTED`), and the external reference is provided by `net.root`. The R5 diagram also updated to show `root (net.root = AgentPort(CON_0, 0))` instead of `FreePort(0) [root]`. R6 port connectivity updated similarly. General wiring in Church(n) section also corrected.
**Spec sections modified:** 3.2 (R9), R5 diagram, R6 port connectivity, 4.1 (construction algorithm), 4.2 (all port connection tables, all redex verifications, general Church(n) wiring)

### SC-002: `get_agent` method not defined in SPEC-02
**Response:** ACCEPTED
**Action taken:** The decode algorithm (Section 4.4) now uses a local helper function `get_agent(net, id) -> Option<&Agent>` implemented as `net.agents.get(id as usize).and_then(|slot| slot.as_ref())`. An explicit note clarifies this is NOT part of SPEC-02's public API but a local utility for the encoding module. The implementer MAY inline it or add it as a private method. This approach avoids requiring changes to SPEC-02 while making the decode algorithm implementable.
**Spec sections modified:** 4.4 (decode algorithm -- complete rewrite)

### SC-003: `get_target` return type mismatch -- `PortRef` vs `Option<PortRef>`
**Response:** ACCEPTED
**Action taken:** The decode algorithm rewritten to use `net.get_target(port) -> PortRef` as defined in SPEC-02, checking against the `DISCONNECTED` sentinel instead of using the `?` operator. All `get_target(...)?.` patterns replaced with explicit `let target = net.get_target(...); if target == DISCONNECTED { return None; }` patterns. Added `use crate::net::{DISCONNECTED};` import. Introductory paragraph in Section 4.4 now explicitly states that `get_target` returns `PortRef` (not `Option<PortRef>`), returning `DISCONNECTED` for invalid ports.
**Spec sections modified:** 4.4 (decode algorithm -- complete rewrite)

### SC-004: `reduce_all_and_return` function undefined
**Response:** ACCEPTED
**Action taken:** All test pseudocode (ET-6, ET-7, ET-8, ET-10) rewritten as explicit two-step patterns matching SPEC-03's API: `let mut net = build_op(a, b); reduce_all(&mut net); assert_eq!(decode_nat(&net), Some(expected));`. R18 (correctness requirement) also rewritten to show the two-step pattern with an explicit comment referencing SPEC-03 R13. Section 8.3 verification property also rewritten as multi-step Rust code.
**Spec sections modified:** 3.4 (R18), 7 (ET-6, ET-7, ET-8, ET-10), 8.3 (verification property)

### SC-005: Test label collision with SPEC-01 invariants
**Response:** ACCEPTED
**Action taken:** All 12 test labels renamed from `T1`-`T12` to `ET-1` through `ET-12` (Encoding Tests). Added introductory note to Section 7: "Test labels use the prefix `ET-` (Encoding Tests) to avoid collision with SPEC-01 invariant labels T1-T7." ET-9 now unambiguously reads "invariants T1-T7 from SPEC-01" with its own label being `ET-9`.
**Spec sections modified:** 7 (all test requirements renamed)

### SC-006: Multiplication and exponentiation net construction critically underspecified
**Response:** ACCEPTED
**Action taken:** Section 4.3.2 (Multiplication) expanded from 2 sentences to a full specification with: (a) 4 numbered construction steps, (b) explicit wire connections for the `mul` combinator structure, (c) explanation of the key difference from addition (function composition vs. sequential application), (d) a worked example for `build_mul(2, 2)` showing 13 initial agents, 2 initial redexes, and the expected result of church(4). Section 4.3.3 (Exponentiation) expanded to include: (a) 4 numbered construction steps, (b) explicit wire connections, (c) note about the design choice between direct application and wrapped lambda form, (d) identification of the initial redex.
**Spec sections modified:** 4.3.2 (multiplication -- major expansion), 4.3.3 (exponentiation -- major expansion)

### SC-007: Sub-net ID space composition not specified for arithmetic net construction
**Response:** ACCEPTED
**Action taken:** Added an explicit "ID composition" paragraph at the top of Section 4.3.1 stating: all arithmetic construction functions MUST construct everything in a single `Net` by calling `encode_church_into` (R4b) sequentially. Because `encode_church_into` calls `net.create_agent` on the shared net, all agent IDs are assigned monotonically from the same `next_id` counter, satisfying invariant I3 (SPEC-01) without any ID remapping. The `build_add` construction steps updated to show `encode_church_into(&mut net, a)` and `encode_church_into(&mut net, b)` calls. Same approach stated in 4.3.2 and 4.3.3.
**Spec sections modified:** 4.3.1 (addition -- construction steps rewritten), 4.3.2 (multiplication), 4.3.3 (exponentiation)

### SC-008: Duplicate Section 9 numbering
**Response:** ACCEPTED
**Action taken:** "Cross-References (Specs Affected)" renumbered from Section 9 to Section 10.
**Spec sections modified:** 10 (renumbered from 9)

### SC-009: Church(0) self-loop on CON_1 and T1/I1 implications
**Response:** ACCEPTED
**Action taken:** Added an explicit note after the R5 diagram documenting that the self-loop `CON_1.p1 <-> CON_1.p2` is correct and satisfies T1 and I1. The note includes: (a) a worked verification showing `ports[ports[p]] = p` holds for both ports, (b) semantic explanation (self-loops represent the identity function), (c) explicit instruction that implementers MUST NOT add assertions rejecting self-loops. Also added a "Note on self-loop" in the Church(0) port connection table.
**Spec sections modified:** 3.2 (R5 diagram note), 4.2 (Church(0) table note)

### SC-010: `encode_nat` returns `Net` but `build_add` needs to compose nets
**Response:** ACCEPTED
**Action taken:** Added requirement R4b: `encode_church_into(net: &mut Net, n: u64) -> AgentId` -- an internal builder variant that constructs a Church numeral inside an existing net and returns the root agent's AgentId. `encode_nat` is specified as a convenience wrapper that creates a new `Net`, calls `encode_church_into`, sets root, and returns the net. The construction algorithm in Section 4.1 rewritten to show both functions, with `encode_church_into` as the core logic and `encode_nat` as the wrapper. All arithmetic construction sections (4.3.1-4.3.3) updated to use `encode_church_into`.
**Spec sections modified:** 3.2 (new R4b), 4.1 (construction algorithm rewritten to show both functions)

### SC-011: `set_root` method not defined in SPEC-02
**Response:** ACCEPTED
**Action taken:** Replaced `net.set_root(PortRef::AgentPort(lam_f, 0))` with `net.root = Some(PortRef::AgentPort(lam_f, 0))` in the construction algorithm (Section 4.1), matching SPEC-02 Section 4.9's direct field assignment pattern. Added comment referencing SPEC-02.
**Spec sections modified:** 4.1 (construction algorithm)

### SC-012: No edge case specification for `encode_nat` overflow or max range boundary
**Response:** ACCEPTED
**Action taken:** R4 updated to specify: "For n > 10_000, the function MUST panic with a descriptive message." The function signature's doc comment now includes a `# Panics` section. R4b (`encode_church_into`) also includes the same panic specification. The construction algorithm in Section 4.1 includes `assert!(n <= 10_000, ...)` at the top of both functions.
**Spec sections modified:** 3.2 (R4, R4b), 4.1 (construction algorithm -- assert added)

### SC-013: Complexity claim for `build_add` reduction interactions is O(a+b) -- needs justification
**Response:** PARTIALLY ACCEPTED
**Action taken:** Added a "Complexity justification" paragraph after the R20 complexity table in Section 3.5. The justification provides an informal derivation of the O(a+b), O(a*b), and O(a^b) bounds based on the Church encoding structure (how many times each numeral applies its function). However, a formal proof is not provided -- the paragraph explicitly notes these are "order-of-magnitude estimates" and that "exact interaction counts depend on the encoding strategy (direct construction vs. combinator composition) and will be determined empirically during benchmarking."

The critic's suggestion to provide a formal derivation is valid in principle but exceeds the scope of a spec document. The encoding module is a demonstration tool, not a performance-critical component. The benchmarks (Section 8) will establish the actual counts. The asymptotic bounds are sufficient for capacity planning and benchmark design.
**Spec sections modified:** 3.5 (complexity justification paragraph added after R20 table)

### SC-014: SPEC-14 test T9 references "invariants T1-T7 from SPEC-01" but SPEC-14 tests are also named T1-T8
**Response:** ACCEPTED
**Action taken:** Resolved by SC-005. Test labels renamed to ET-1 through ET-12. ET-9 now reads: "for all encodings and arithmetic operations in ET-1 through ET-8, the generated nets MUST satisfy invariants T1-T7 from SPEC-01." No ambiguity remains.
**Spec sections modified:** 7 (ET-9)

### SC-015: Test T11 mixes `reduce_all` signatures inconsistently
**Response:** ACCEPTED
**Action taken:** ET-11 (formerly T11) rewritten as explicit multi-step Rust code showing: (a) local reduction with `reduce_all(&mut local_net)` followed by `decode_nat(&local_net)`, (b) distributed reduction with `run_grid(build_add(50, 50), k)` followed by `decode_nat(&distributed_net)`, (c) `assert_eq!(local_result, distributed_result)`. This matches SPEC-03's API exactly.
**Spec sections modified:** 7 (ET-11)

### SC-016: Spec deviates from template -- missing standard sections
**Response:** NOT ADDRESSED
**Action taken:** The deviation is deliberate and adds value. SPEC-14's additional sections (Test Requirements and Arithmetic Benchmark Scenarios) provide essential content for implementability and experimental evaluation. Other specs in the Relativist suite (e.g., SPEC-09) follow similar patterns with extended sections beyond the template. The Section 7 (Test Requirements) placement is consistent with SPEC-08's role as the test strategy spec -- SPEC-14's tests are encoding-specific and belong here rather than in a generic test document. Reorganizing into the template structure would reduce readability by burying test requirements inside the Design section.

The duplicate Section 9 numbering (a concrete structural issue) has been fixed as SC-008 -- the remaining template deviation is a stylistic choice that does not impede navigation.
**Spec sections modified:** None (structural choice preserved)

### SC-017: `build_exp` correctness requirement tension between SHOULD (R17) and MUST (R18)
**Response:** ACCEPTED
**Action taken:** R17 promoted from SHOULD to MUST: `build_exp` is now mandatory. This resolves the contradiction with R18 ("all arithmetic nets MUST reduce correctly") and ET-8 (which tests exponentiation as a MUST). The promotion is justified because: (a) the ARITH-EXP benchmarks in Section 8 are primary evaluation scenarios for the TCC, (b) exponentiation is the simplest combinator to implement (just one application node), and (c) it demonstrates Profile B/C behavior essential for the TCC's experimental analysis.
**Spec sections modified:** 3.4 (R17)

### SC-018: No specification of how `build_add` creates redexes at application boundaries
**Response:** PARTIALLY ACCEPTED
**Action taken:** Added a worked example for `build_add(1, 1)` using the direct construction approach. The example shows: agent IDs assigned by sequential `encode_church_into` calls, combinator agents, wiring, initial redexes, and expected result. However, the exact port-level wiring for the direct construction approach is acknowledged as non-trivial ("The exact wiring for the direct construction approach is non-trivial because it involves threading `f` and `x` through two Church numeral sub-nets"). The spec instructs the implementer to verify correctness via the roundtrip test `decode_nat(&net) == Some(2)`.

A full port connection table for `build_add(1, 1)` is not provided because the exact wiring depends on the construction strategy chosen (combinator composition vs. direct construction -- R15 SHOULD). Instead, the worked example provides enough structure to guide implementation while the roundtrip tests (ET-5, ET-6) serve as the definitive correctness check.

Additionally, a worked example for `build_mul(2, 2)` was added in Section 4.3.2, showing 13 initial agents, 2 initial redexes, and the expected reduction to church(4).
**Spec sections modified:** 4.3.1 (worked example for build_add(1,1)), 4.3.2 (worked example for build_mul(2,2))

---

## Changes Made to SPEC-14

### Header
- Status updated from "Draft v1" to "Revised v2"

### Section 3.2 (Church Numeral Encoding)
- **R4:** Added panic specification for n > 10_000 with descriptive message; added `# Panics` doc comment
- **R4b:** New requirement added -- `encode_church_into(net: &mut Net, n: u64) -> AgentId` for composable construction
- **R5:** Diagram updated to show `root (net.root = AgentPort(CON_0, 0))` instead of `FreePort(0) [root]`; added self-loop correctness note with T1/I1 verification
- **R6:** Port connectivity updated to show `root (net.root = AgentPort(CON_f, 0))` instead of `FreePort(0) [root]`
- **R9:** Completely rewritten to specify `net.root = Some(PortRef::AgentPort(lam_f, 0))` with SPEC-02 reference; added note about DISCONNECTED in port array vs. root field

### Section 3.4 (Arithmetic Operations)
- **R17:** Promoted from SHOULD to MUST
- **R18:** Rewritten to show two-step pattern (`reduce_all(&mut net)` then `decode_nat(&net)`)

### Section 3.5 (Complexity Bounds)
- Added "Complexity justification" paragraph after R20 table

### Section 4.1 (Construction Algorithm)
- Rewritten to show `encode_nat` as wrapper and `encode_church_into` as core logic
- `net.set_root(...)` replaced with `net.root = Some(...)`
- Added `assert!` for n > 10_000 in both functions

### Section 4.2 (Port Connection Tables)
- All tables: `FreePort(0) [root]` replaced with `root (DISCONNECTED in port array)`
- All tables: added `net.root = Some(AgentPort(0, 0))`
- Church(0): added "Note on CON_0.p0" explaining DISCONNECTED
- Church(0): added "Note on self-loop" for CON_1 auxiliary self-connection
- All redex verifications: `FreePort` references replaced with `DISCONNECTED (root)`
- General Church(n): `FreePort(0) [root]` replaced with `root (DISCONNECTED in port array; ...)`

### Section 4.3.1 (Addition)
- Added "ID composition" paragraph explaining single-net construction via `encode_church_into`
- Construction steps rewritten to show `encode_church_into` calls with explicit return values
- Added worked example for `build_add(1, 1)` with agent IDs, wiring, initial redexes

### Section 4.3.2 (Multiplication)
- Completely rewritten from 2 sentences to full specification with 4 construction steps
- Added explicit wire connections for mul combinator
- Added worked example for `build_mul(2, 2)` with agent counts, initial redexes, expected result

### Section 4.3.3 (Exponentiation)
- Completely rewritten from 2 sentences to full specification with 4 construction steps
- Added explicit wire connections
- Added note about design choice (direct application vs. wrapped lambda)

### Section 4.4 (Decode Algorithm)
- Complete rewrite to use SPEC-02's actual API
- Added local `get_agent` helper with `net.agents.get(id as usize).and_then(|slot| slot.as_ref())`
- All `get_target(...)?.` patterns replaced with explicit DISCONNECTED checks
- Added `use crate::net::{DISCONNECTED}` import
- Added introductory paragraph explaining API alignment with SPEC-02

### Section 7 (Test Requirements)
- All test labels renamed from T1-T12 to ET-1 through ET-12
- Added introductory note explaining the `ET-` prefix
- ET-6, ET-7, ET-8, ET-10: rewritten as explicit two-step Rust code (no `reduce_all_and_return`)
- ET-9: cross-reference clarified (ET-1 through ET-8 for own tests, T1-T7 for SPEC-01 invariants)
- ET-11: rewritten as multi-step Rust code with local and distributed paths

### Section 8.3 (Verification Property)
- Pseudocode rewritten as explicit multi-step Rust code

### Section 10 (Cross-References)
- Renumbered from Section 9 to Section 10 (fixing duplicate numbering)

---

## Residual Risks

### SC-016: Template deviation (NOT ADDRESSED)
**Risk level:** LOW. The deviation is deliberate and consistent with other specs in the suite. No functional impact. The duplicate numbering (the concrete issue) has been fixed. Cross-spec navigation is minimally affected because the section titles are descriptive.

### SC-013: Complexity bounds are asymptotic estimates (PARTIALLY ACCEPTED)
**Risk level:** LOW. The bounds serve as capacity planning guidance, not performance contracts. Empirical benchmarks (SPEC-09, Section 8 of this spec) will establish exact counts. The added justification paragraph makes the estimate basis explicit.

### SC-018: Worked example for build_add(1,1) is partial (PARTIALLY ACCEPTED)
**Risk level:** MEDIUM. The direct construction wiring is acknowledged as non-trivial and the example does not provide a complete port connection table. However, this is mitigated by: (a) the roundtrip tests (ET-5, ET-6) which serve as the definitive correctness check, (b) the combinator composition approach (which is the default) is fully traceable from the Church numeral tables, and (c) the mul(2,2) example in Section 4.3.2 provides additional worked-example coverage. The implementer has sufficient guidance to derive the correct wiring.
