# SPEC-08 -- Round 2: Defender Response

**Date:** 2026-04-05
**Defender:** Especialista em Specs
**Target:** SPEC-08-test-strategy.md
**Critic review:** SPEC-08-round1-critic.md
**Spec version:** Revised v2 -> Revised v3

---

## Summary

| Response | Count |
|----------|-------|
| ACCEPTED | 14 |
| PARTIALLY ACCEPTED | 3 |
| NOT ADDRESSED | 0 |
| **Total issues** | **17** |

---

## Responses

### SC-001: SPEC-08 does not cover SPEC-10 through SPEC-14 test requirements (59 tests missing)
**Severity:** CRITICAL
**Response:** ACCEPTED
**Action taken:** Adopted option (a) from the critic: expanded SPEC-08's scope to cover all specs. Specific changes:

1. **Purpose statement** (Section 1): Updated to state "every MUST requirement from SPEC-01 through SPEC-14" instead of "SPEC-01 through SPEC-07."
2. **Depends on** (Header): Added SPEC-10, SPEC-11, SPEC-12, SPEC-13, SPEC-14 as dependencies.
3. **Five new requirement sections added:**
   - Section 3.18 "Unit Tests -- Encoding (SPEC-14)" with R39 incorporating ENC-1 through ENC-12 (mapped from SPEC-14 ET-1 through ET-12)
   - Section 3.19 "Unit Tests -- Security (SPEC-10)" with R40 incorporating SEC-1 through SEC-11 (mapped from SPEC-10 T1 through T10)
   - Section 3.20 "Unit Tests -- Observability (SPEC-11)" with R41 incorporating OBS-1 through OBS-10 (mapped from SPEC-11 T1 through T9, T8a)
   - Section 3.21 "Unit Tests -- User I/O (SPEC-12)" with R42 incorporating UIO-1 through UIO-14 (mapped from SPEC-12 T1 through T14)
   - Section 3.22 "Unit Tests -- System Architecture / FSM (SPEC-13)" with R43 incorporating ARCH-1 through ARCH-13

4. **Directory structure** (Section 4.1): Added `tests.rs` entries for all 11 modules from SPEC-13 R5: `encoding/`, `coordinator/`, `worker/`, `config/`, `observability/`, `security/`.
5. **Invariant Coverage Matrix** (Section 4.5): Updated G1 row to include ENC-11. 
6. **Workload Profile matrix** (Section 4.6): Encoding tests are Profile-agnostic (they test arithmetic, not grid workloads) so no change needed there.

Total test count incorporated: 12 (encoding) + 11 (security) + 10 (observability) + 14 (user I/O) + 13 (architecture) = 60 tests from SPEC-10 through SPEC-14, plus the original ~130 tests from SPEC-01 through SPEC-07.
**Spec sections modified:** Header, Section 1, Sections 3.18-3.22, Section 4.1, Section 4.5

---

### SC-002: Test label namespace collisions across specs
**Severity:** CRITICAL
**Response:** ACCEPTED
**Action taken:** Established a global test label namespace convention. Specific changes:

1. **Namespace convention** defined in Section 2 (Definitions): A new term "Test Label Namespace" specifies that tests from other specs use spec-qualified prefixes: `SEC-` (SPEC-10), `OBS-` (SPEC-11), `UIO-` (SPEC-12), `ARCH-` (SPEC-13), `ENC-` (SPEC-14). SPEC-08's own labels retain their existing module-specific prefixes (N, RE, P, M, etc.).

2. **Integration test labels** renamed from `I1-I11` to `INT1-INT11` (see SC-004 for details).

3. **Each new section** (3.18-3.22) includes a namespace note at the top showing the mapping from original spec labels to the global namespace (e.g., "SPEC-14 labels (ET-1 through ET-12) are mapped to `ENC-1` through `ENC-12`").

With this convention, every test label in the project is globally unique. The four different "T1" labels are now: SPEC-01 T1 (invariant), SEC-1 (security), OBS-1 (observability), UIO-1 (user I/O). No collision is possible.
**Spec sections modified:** Section 2, Sections 3.18-3.22 (namespace notes)

---

### SC-003: Cross-reference error -- SPEC-02 serialization requirements are R24-R26, not R22-R24
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** Corrected all three instances:
- Section 2, Round-Trip Test definition: Changed "SPEC-02, R22-R24" to "SPEC-02, R24-R26"
- Section 3.2, test N16: Changed SPEC-02 Req column from "R22, R24" to "R24, R26"
- Section 3.2, test N17: Changed SPEC-02 Req column from "R23" to "R25"
- Section 3.12, test PB8: Changed Source column from "SPEC-02 R22-R24" to "SPEC-02 R24-R26"
**Spec sections modified:** Section 2, Section 3.2 (N16, N17), Section 3.12 (PB8)

---

### SC-004: Integration test labels I1-I11 shadow SPEC-01 invariant labels I1-I5
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** Renamed all integration test labels from `I1-I11` to `INT1-INT11`. This change was applied consistently across:
- Section 3.10 (integration test table): All 11 rows updated
- Section 4.5 (invariant coverage matrix): All references to "I1-I11" changed to "INT1-INT11"; SPEC-01 invariant labels I1-I5 remain unchanged and are now unambiguous
- Section 4.6 (workload profile matrix): All integration test references updated
- Section 6.1 (Haskell prototype mapping table): GridSpec and BenchmarkSpec rows updated
- Section 6.3 (gap coverage table): All integration test references updated
- Section 5.3 (rationale): `I8` changed to `INT8`

No collision remains between SPEC-01 invariant labels (I1-I5) and integration test labels (INT1-INT11).
**Spec sections modified:** Section 3.10, Section 4.5, Section 4.6, Section 5.3, Section 6.1, Section 6.3

---

### SC-005: No tests for SPEC-14 encoding module in directory structure or coverage matrices
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** Added encoding test locations to Section 4.1 (directory structure):
- `src/encoding/tests.rs` for unit tests ENC-1 through ENC-5, ENC-9, ENC-12
- `tests/integration/encoding.rs` for ENC-6 through ENC-8 (arithmetic correctness)
- `tests/integration/encoding_distributed.rs` for ENC-11 (distributed correctness, Fundamental Property)
- `tests/property/encoding.rs` for ENC-10 (proptest for arithmetic)

Added ENC-11 to the G1 row in the invariant coverage matrix (Section 4.5).
**Spec sections modified:** Section 4.1, Section 4.5

---

### SC-006: No tests for SPEC-12 `reduce` subcommand and text DSL
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** Added Section 3.21 "Unit Tests -- User I/O (SPEC-12)" with R42 incorporating all 14 test requirements from SPEC-12 (mapped to UIO-1 through UIO-14). These cover:
- Binary and text format roundtrips (UIO-1, UIO-2)
- Parser error handling (UIO-3, UIO-11, UIO-12, UIO-13)
- Generator validity (UIO-4)
- `inspect` correctness (UIO-5)
- `reduce` correctness and `--max-interactions` (UIO-6, UIO-7)
- File format detection (UIO-8)
- Generator consistency with benchmark suite (UIO-9)
- Size zero edge case (UIO-10)
- Empty net handling (UIO-14)

Tests placed in `src/cli/tests.rs` (co-located with CLI/IO module).
**Spec sections modified:** Section 3.21, Section 4.1

---

### SC-007: No tests for SPEC-10 security layer
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** Added Section 3.19 "Unit Tests -- Security (SPEC-10)" with R40 incorporating all 10 test requirements from SPEC-10 (mapped to SEC-1 through SEC-11, where T10's debug redaction was split into a separate SEC-11 for clarity). Feature-gated tests (SEC-4, SEC-5 for TLS) are documented as requiring `--features tls`.

Added `src/security/tests.rs` to the directory structure.
**Spec sections modified:** Section 3.19, Section 4.1

---

### SC-008: Invariant coverage matrix does not include T2, T3 in property-based tests
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** Added two new property tests:
- PB13: `prop_redex_detection_only_principal_ports` -- Verifies that after building a random net, every entry in the redex queue is a pair connected via port 0. Directly tests T2 across random topologies.
- PB14: `prop_active_pairs_disjoint` -- Verifies that no AgentId appears in more than one active pair in the redex queue at any step during `reduce_all`. Directly tests T3.

Updated the invariant coverage matrix: T2 row now includes PB13, T3 row now includes PB14.
**Spec sections modified:** Section 3.12 (PB13, PB14 added to table), Section 4.5 (T2 and T3 rows updated)

---

### SC-009: Graph isomorphism checker has O(n!) worst-case complexity with no mitigation for medium nets
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** Added a SHOULD requirement to Section 4.4: "The isomorphism checker SHOULD use canonical form computation (e.g., Weisfeiler-Leman refinement followed by canonical ordering) or a polynomial-time heuristic that handles nets up to 500 agents within 100ms. The backtracking fallback MAY be used for nets where the heuristic is inconclusive."

This gives the implementer a concrete performance target (500 agents, 100ms) without mandating a specific algorithm. Open Question 1 has been updated to reference this SHOULD requirement.
**Spec sections modified:** Section 4.4 (added performance target paragraph), Section 7 (OQ-1 updated)

---

### SC-010: Non-termination safety in proptest relies on heuristic budget formula, but budget formula is heuristic
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** Three changes to Section 4.3 (Random Net Generator):
1. Replaced the fixed formula `n * n * 10` with a configurable constant `MAX_PROPTEST_BUDGET` (default: 100,000) and documented that this is a practical ceiling, not a theoretical guarantee.
2. Specified behavior on budget exhaustion: the test SHOULD mark the input as inconclusive via `prop_assume!(false, "budget exceeded -- skipping non-terminating input")` rather than failing. This prevents flaky CI from false positives.
3. Clarified that `proptest::test_runner::Config { timeout: 10_000 }` is a separate safety net for cases where even dequeuing stale redexes is too slow.

Open Question 2 has been marked as RESOLVED with a pointer to Section 4.3.
**Spec sections modified:** Section 4.3 (non-termination safety paragraph rewritten), Section 7 (OQ-2 marked resolved)

---

### SC-011: SPEC-08 directory structure does not include `src/encoding/tests.rs` from SPEC-13 module layout
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** Updated Section 4.1 directory structure to include `tests.rs` for all 11 modules from SPEC-13 R5:

| Module | tests.rs entry | Test IDs |
|--------|---------------|----------|
| `encoding/` | Added | ENC-1 through ENC-12 |
| `coordinator/` | Added | ARCH-1 through ARCH-8, ARCH-13 |
| `worker/` | Added | ARCH-9 through ARCH-12 |
| `config/` | Added | (covered by DP1-DP5 and UIO-8) |
| `observability/` | Added | OBS-1 through OBS-10 |
| `security/` | Added | SEC-1 through SEC-11 |

Previously, only 7 modules had `tests.rs` entries. Now all 11 (plus `cli/`) have entries.
**Spec sections modified:** Section 4.1

---

### SC-012: No test for DUP-CON symmetry in commutation (only CON-DUP tested)
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** Strengthened RE7's description from the vague "produces the same result" to: "Construct a net with DUP principal port connected to CON principal port. Reduce. Verify that the post-reduction topology is isomorphic to the result of RE4 (CON-DUP canonical order)." This makes the symmetry verification topology-precise, not just metric-based.
**Spec sections modified:** Section 3.3 (RE7 row in table)

---

### SC-013: Workload Profile coverage matrix omits Profile B/C for property tests
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** Added two targeted property tests and generators:
- PB15: `prop_condup_fundamental_property` using `arb_condup_net(max_pairs)` -- generates nets with predominantly CON-DUP pairs, guaranteeing Profile B behavior. Verifies `reduce_all(net) ~ run_grid(net, n)`.
- PB16: `prop_chain_fundamental_property` using `arb_chain_net(max_depth)` -- generates tree/chain topologies with level-dependent reduction, approximating Profile C behavior. Verifies `reduce_all(net) ~ run_grid(net, n)`.

Added generator signatures to Section 4.3. Updated the Workload Profile matrix (Section 4.6): Profile B now includes PB15, Profile C now includes PB16. Updated G1 row in the invariant coverage matrix to include PB15 and PB16.
**Spec sections modified:** Section 3.12 (PB15, PB16), Section 4.3 (generator signatures), Section 4.5 (G1 row), Section 4.6 (Profile B and C rows)

---

### SC-014: E9 performance test has no failure criterion tied to complexity
**Severity:** LOW
**Response:** PARTIALLY ACCEPTED
**Action taken:** Adopted both suggestions:
1. Replaced the fixed 30-second threshold with a relative check: run `reduce_all` on nets with 1,000 and 10,000 agents, verify that `time(10k) / time(1k) < 15` (allowing 50% margin over the expected 10x linear scaling).
2. Marked E9 with `#[ignore]` since it is a performance test, not a correctness test, and should not block CI on every push.

The fix differs from the critic's suggestion only in that the exact ratio threshold (15x vs. the critic's unspecified value) was chosen by the defender. A single-ratio check with two data points is not a rigorous complexity proof but is sufficient for a smoke test. Full complexity verification is deferred to SPEC-09 benchmarks which collect multiple data points.
**Spec sections modified:** Section 3.14 (E9 row)

---

### SC-015: `test_net_serialize_roundtrip` (N16) references SPEC-02 R22 and R24, but the correct requirement for self-contained format is R25
**Severity:** LOW
**Response:** ACCEPTED
**Action taken:** Corrected N17's SPEC-02 Req column from "R23" to "R25." This was part of the systematic R-number correction applied in SC-003.
**Spec sections modified:** Section 3.2 (N17 row)

---

### SC-016: SPEC-08 defines `ReductionStrategy` but does not reference SPEC-03 for authority
**Severity:** LOW
**Response:** PARTIALLY ACCEPTED
**Action taken:** Added a note below R38 stating: "The `ReductionStrategy` type and `reduce_all_with_strategy` function are defined here because they exist solely for testing purposes. However, they affect the reduction engine's API surface. This requirement SHOULD be reflected in SPEC-03 during its next revision, even if the implementation is `#[cfg(test)]`-gated."

The fix differs from the critic in that the type definition remains in SPEC-08 (the test strategy spec is the appropriate home for test-only types), with a forward reference to SPEC-03. Moving the type to SPEC-03 would require editing SPEC-03, which is outside SPEC-08's revision scope.
**Spec sections modified:** Section 3.17 (note added below R38)

---

### SC-017: No explicit test for SPEC-01 D2 (local reduction equivalence) isolation
**Severity:** LOW
**Response:** PARTIALLY ACCEPTED
**Action taken:** Added Section 3.23 with R44 (SHOULD) defining test D2-1: `test_d2_local_reduction_equivalence`. This test constructs a net with a known redex, reduces it globally and records the topological delta, partitions the net so the redex is internal, reduces the redex in the partition, and compares the deltas.

The fix is PARTIALLY ACCEPTED because the test is a SHOULD rather than a MUST. D2 is already covered indirectly by PB10 and INT1-INT11. Adding a dedicated D2 test improves diagnosis precision but is not critical for correctness coverage.
**Spec sections modified:** Section 3.23 (new section with R44 and D2-1)

---

## Changes Made to SPEC-08

### Header
- Status changed from "Revised v2" to "Revised v3"
- Added SPEC-10, SPEC-11, SPEC-12, SPEC-13, SPEC-14 to dependencies
- Added adversarial review reference

### Section 1 (Purpose)
- Expanded scope from "SPEC-01 through SPEC-07" to "SPEC-01 through SPEC-14"
- Added mention of global namespace convention for test labels
- Added Encoding, Security, Observability, User I/O, Coordinator FSM, Worker FSM to component list

### Section 2 (Definitions)
- Fixed Round-Trip Test reference: "SPEC-02, R22-R24" -> "SPEC-02, R24-R26"
- Added "Test Label Namespace" term defining global prefix convention

### Section 3.2 (Unit Tests -- Net)
- N16: Changed SPEC-02 Req from "R22, R24" to "R24, R26"
- N17: Changed SPEC-02 Req from "R23" to "R25"

### Section 3.3 (Unit Tests -- Reduction Engine)
- RE7: Strengthened description to require topology isomorphism against RE4 result

### Section 3.10 (Integration Tests)
- Renamed all test labels from I1-I11 to INT1-INT11

### Section 3.12 (Property-Based Tests)
- PB8: Changed Source from "SPEC-02 R22-R24" to "SPEC-02 R24-R26"
- Added PB13: `prop_redex_detection_only_principal_ports` (T2 coverage)
- Added PB14: `prop_active_pairs_disjoint` (T3 coverage)
- Added PB15: `prop_condup_fundamental_property` (Profile B targeted)
- Added PB16: `prop_chain_fundamental_property` (Profile C targeted)

### Section 3.14 (Edge Cases)
- E9: Replaced fixed 30-second threshold with relative `time(10k)/time(1k) < 15` check. Marked `#[ignore]`.

### Section 3.17 (Reduction Strategies)
- Added note below R38 about SPEC-03 forward reference

### Sections 3.18-3.23 (NEW -- Tests for SPEC-10 through SPEC-14 + D2)
- Section 3.18: Encoding tests (R39, ENC-1 through ENC-12)
- Section 3.19: Security tests (R40, SEC-1 through SEC-11)
- Section 3.20: Observability tests (R41, OBS-1 through OBS-10)
- Section 3.21: User I/O tests (R42, UIO-1 through UIO-14)
- Section 3.22: Architecture/FSM tests (R43, ARCH-1 through ARCH-13)
- Section 3.23: Dedicated D2 test (R44, D2-1)

### Section 4.1 (Directory Structure)
- Added `tests.rs` for 6 previously missing modules: `encoding/`, `coordinator/`, `worker/`, `config/`, `observability/`, `security/`
- Added integration test files: `encoding.rs`, `encoding_distributed.rs`, `d2_equivalence.rs`
- Added property test file: `encoding.rs`
- Added targeted generators to `generators.rs` note: `arb_condup_net`, `arb_chain_net`

### Section 4.3 (Random Net Generator)
- Added `arb_condup_net(max_pairs)` and `arb_chain_net(max_depth)` generator signatures
- Replaced heuristic `n * n * 10` budget with configurable `MAX_PROPTEST_BUDGET = 100,000`
- Specified `prop_assume!` for inconclusive inputs on budget exhaustion
- Clarified `timeout` as a separate safety net

### Section 4.4 (Graph Isomorphism Checker)
- Added SHOULD requirement for polynomial-time heuristic (500 agents, 100ms target)

### Section 4.5 (Invariant Coverage Matrix)
- All "I1-I11" references changed to "INT1-INT11"
- T2 row: Added PB13
- T3 row: Added PB14
- D2 row: Added D2-1
- G1 row: Added PB15, PB16, ENC-11

### Section 4.6 (Workload Profile Matrix)
- All integration test references changed to INT prefix
- Profile B: Added PB15
- Profile C: Added PB16

### Section 5.3 (Rationale -- CON-DUP)
- Changed I8 to INT8

### Section 6.1 (Haskell Prototype Mapping)
- Changed I1-I3, I4-I5, I6-I8 to INT1-INT3, INT4-INT5, INT6-INT8
- Changed "12 property tests (PB1-PB12)" to "16 property tests (PB1-PB16)"

### Section 6.3 (Gap Coverage)
- Updated all integration test references to INT prefix
- Updated PB count to PB1-PB16
- Added PB15 to CONDUP gap coverage

### Section 7 (Open Questions)
- OQ-1: Updated to reference the SHOULD performance target
- OQ-2: Marked as RESOLVED (configurable budget + prop_assume!)

---

## Residual Risks

### Label mapping maintenance

The global test label namespace introduces a mapping layer (e.g., SPEC-14 ET-1 = SPEC-08 ENC-1). If SPEC-10 through SPEC-14 add or renumber test requirements in future revisions, SPEC-08's sections 3.18-3.22 must be updated in sync. This is a maintenance cost, but it is justified by the benefit of having a single authoritative test inventory. The alternative (decentralized test strategies across 6 specs) would make it impossible to assess overall test coverage from any single document.

### Cross-spec consistency

This revision references types and FSM states from SPEC-13 (which itself supersedes parts of SPEC-06). The ARCH-1 through ARCH-13 tests reference SPEC-13 R20, R21, R25 transition tables. If those transition tables are modified in a future SPEC-13 revision, the ARCH tests must be updated. This is standard cross-spec dependency management.

### Requirement numbering

The original SPEC-08 had requirements R1-R38. This revision adds R39-R44. If future revisions add more requirements, they continue from R45. No existing requirement numbers were changed or reordered.
