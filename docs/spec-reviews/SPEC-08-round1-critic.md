# SPEC-08 -- Round 1: Spec Critic Review

**Date:** 2026-04-05
**Reviewer:** Spec Critic (adversarial)
**Target:** SPEC-08-test-strategy.md (status: Revised v2)
**Predecessors consulted:** SPEC-00, SPEC-01, SPEC-02, SPEC-03, SPEC-04, SPEC-05, SPEC-09, SPEC-10, SPEC-11, SPEC-12, SPEC-13, SPEC-14

---

## Overall Assessment

SPEC-08 is the most thorough testing spec I have reviewed in this project. It defines 4 test tiers, 130+ individually named tests across 17 requirement groups, a property-based testing strategy with 12 properties, a runtime invariant checker, an isomorphism verifier, a random net generator, and a complete invariant coverage matrix. The design is clearly informed by the Haskell prototype gap analysis and directly addresses the weaknesses identified in DISC-003 v2 and AC-001 through AC-005.

However, SPEC-08 was written before SPEC-10 through SPEC-14 reached Revised v2 status. As a result, it has zero coverage of the 59 test requirements defined in those five specs (SPEC-10 T1-T10, SPEC-11 T1-T9/T8a, SPEC-12 T1-T14, SPEC-14 ET-1 through ET-12). Worse, the test label namespaces used by those specs collide with SPEC-08's own labels. This creates a fragmented, inconsistent testing landscape where the implementer cannot determine which spec is authoritative for test organization. Several cross-reference errors also undermine trust in the spec's precision.

**Verdict:** NEEDS REVISION

---

## Issues

### SC-001: SPEC-08 does not cover SPEC-10 through SPEC-14 test requirements (59 tests missing)
**Severity:** CRITICAL
**Axis:** Completeness
**Section:** 3 (all of Requirements)
**Requirement:** R1 (test organization), R7 (SPEC-02 coverage), entire Section 3
**Problem:** SPEC-08's purpose statement says: "The goal is that every MUST requirement from SPEC-01 through SPEC-07 has at least one corresponding test." This scope explicitly excludes SPEC-08 itself (meta), SPEC-09 (benchmarks, not functional tests), but also excludes SPEC-10 (Security), SPEC-11 (Observability), SPEC-12 (User I/O), SPEC-13 (System Architecture), and SPEC-14 (Arithmetic Encoding). These five specs define a total of 59 test requirements:

| Spec | Test Labels | Count | Examples |
|------|-------------|-------|----------|
| SPEC-10 | T1-T10 | 10 | Token generation, TLS handshake, tier detection |
| SPEC-11 | T1-T9, T8a | 10 | Tracing init, JSON format, health endpoints, metrics |
| SPEC-12 | T1-T14 | 14 | Binary roundtrip, text DSL, inspect, reduce, generators |
| SPEC-14 | ET-1 through ET-12 | 12 | Church encoding structure, arithmetic correctness, distributed correctness |
| SPEC-13 | (embedded in FSM/transport sections) | ~13 | ChannelTransport, FSM transitions, etc. |

None of these appear in SPEC-08's directory structure (Section 4.1), invariant coverage matrix (Section 4.5), or workload profile coverage matrix (Section 4.6). The implementer has no single source of truth for the complete test inventory.

**Impact if unresolved:** The implementer must manually aggregate test requirements from 6 different specs. Tests from SPEC-10 through SPEC-14 have no assigned location in the directory structure, no integration into the CI pipeline description (R5), and no inclusion in coverage tracking (R6). The claim "every MUST requirement has at least one test" is false.
**Suggested resolution:** Either (a) update SPEC-08's scope statement and add sections 3.18 through 3.22 covering Security tests, Observability tests, User I/O tests, Architecture tests, and Encoding tests -- incorporating the test labels already defined in those specs, OR (b) explicitly state that SPEC-08 covers SPEC-01 through SPEC-07 only and that each subsequent spec is self-contained for its own test requirements. Option (a) is strongly preferred because SPEC-08 is the canonical test strategy and should provide a unified view.

---

### SC-002: Test label namespace collisions across specs
**Severity:** CRITICAL
**Axis:** Consistency
**Section:** 3 (all test ID tables)
**Requirement:** R2 (naming convention)
**Problem:** SPEC-08 defines test labels using short prefixes: N1-N18, RE1-RE21, P1-P12, M1-M7, PS1-PS5, FI1-FI3, GL1-GL5, WP1-WP3, DP1-DP5, I1-I11, F1-F5, PB1-PB12, CD1-CD4, E1-E9, E2E1-E2E5, IV1-IV6. Other specs define their own test labels:

| Collision | SPEC-08 usage | Other spec usage |
|-----------|--------------|-----------------|
| **T1-T10** | Not used as test IDs in SPEC-08, but T1-T7 are SPEC-01 invariant labels referenced throughout | SPEC-10 uses T1-T10 for security tests; SPEC-11 uses T1-T9 for observability tests; SPEC-12 uses T1-T14 for user I/O tests |
| **I1-I11** | SPEC-08 Section 3.10 uses I1-I11 for integration pipeline tests | SPEC-01 uses I1-I5 for implementation invariants; confusion when SPEC-08 Section 4.5 matrix says "All I1-I11 (via invariant checker)" in the T1 row |

The T-prefix collision is the most dangerous: SPEC-08's invariant coverage matrix (Section 4.5) references "T1 (Linearity)" as a SPEC-01 invariant, but a test labeled T1 in SPEC-10 means "Token generation MUST be tested." In the same project, `T1` cannot mean two different things. Similarly, SPEC-11's T1 is "tracing subscriber initialization" and SPEC-12's T1 is "binary roundtrip." There are now **four different T1s** across the spec suite.

**Impact if unresolved:** When a CI report says "T1 failed," the implementer does not know whether it refers to a security test, an observability test, a user I/O test, or a SPEC-01 invariant verification. Ambiguity in test identification undermines the entire testing strategy.
**Suggested resolution:** Establish a global test label namespace. Options: (a) prefix all test labels with the spec number (e.g., `S08-N1`, `S10-T1`, `S11-T1`), or (b) use module-specific prefixes (e.g., `SEC-1` for security, `OBS-1` for observability, `UIO-1` for user I/O, `ENC-1` for encoding). SPEC-08, as the canonical test strategy spec, MUST define and enforce this namespace. If SPEC-08 is updated per SC-001, it should also assign unique prefixes to all tests.

---

### SC-003: Cross-reference error -- SPEC-02 serialization requirements are R24-R26, not R22-R24
**Severity:** HIGH
**Axis:** Consistency
**Section:** 2 (Definitions), 3.2 (R7, table row N16), 3.12 (PB8)
**Requirement:** R7, R25
**Problem:** SPEC-08 references serialization requirements as "SPEC-02, R22-R24" in three places:
- Section 2, Round-Trip Test definition: "deserialize(serialize(net)) == net (SPEC-02, R22-R24)"
- Section 3.2, test N16: "R22, R24" in the SPEC-02 Req column
- Section 3.12, test PB8: "SPEC-02 R22-R24" in the Source column

In SPEC-02 (Revised v2), the actual serialization requirements are:
- **R24:** Net MUST be serializable via serde + bincode
- **R25:** Serialized format MUST be self-contained
- **R26:** Serialization MUST preserve identity: `deserialize(serialize(net)) == net`

SPEC-02's R22 is about the Net being a concrete representation of `(A, W)` from DISC-004 v2, and R23 is about distinguishing internal vs. boundary free ports. Neither has anything to do with serialization.

**Impact if unresolved:** An implementer looking up "SPEC-02 R22" to understand what test N16 verifies will find a requirement about formal semantics, not serialization. This creates confusion about what the test actually validates.
**Suggested resolution:** Correct all three references to "SPEC-02, R24-R26." Specifically: N16 should reference "R24, R26", N17 should reference "R25", and PB8 should reference "SPEC-02 R24-R26."

---

### SC-004: Integration test labels I1-I11 shadow SPEC-01 invariant labels I1-I5
**Severity:** HIGH
**Axis:** Consistency
**Section:** 3.10 (R22), 4.5 (Invariant Coverage Matrix)
**Requirement:** R22
**Problem:** SPEC-08 uses "I1" through "I11" as integration test IDs (Section 3.10: `test_pipeline_era_chain_2w` is I1, `test_pipeline_mixed_2w` is I2, etc.). But SPEC-01 defines I1 through I5 as implementation invariant labels (I1 = Bidirectional Port Array, I2 = Reference Validity, I3 = ID Monotonicity, I4 = Redex Queue Validity, I5 = Termination of reduce_all).

This collision becomes confusing in the Invariant Coverage Matrix (Section 4.5), where the row for SPEC-01's "I1 (Bidirectional port array)" says its integration tests are "All (via checker)" -- but the "All" refers to I1-I11 integration tests, not to invariant I1.

Additionally, the row for SPEC-01's "I2 (Reference validity)" says its integration tests are "All (via checker)" -- but I2 is also `test_pipeline_mixed_2w`. A reader scanning the matrix cannot disambiguate.

**Impact if unresolved:** The invariant coverage matrix is ambiguous. An implementer might think SPEC-01 invariant I2 is directly verified by integration test I2, when in fact the matrix means "all integration tests indirectly verify I2 via the runtime invariant checker."
**Suggested resolution:** Rename integration test labels from I1-I11 to INT1-INT11 (or IT1-IT11) to avoid collision with SPEC-01's I-prefix.

---

### SC-005: No tests for SPEC-14 encoding module in directory structure or coverage matrices
**Severity:** HIGH
**Axis:** Completeness
**Section:** 4.1 (Directory Structure), 4.5 (Invariant Coverage Matrix)
**Requirement:** R1 (test organization)
**Problem:** SPEC-14 defines an `encoding` module (`src/encoding/`) with 12 test requirements (ET-1 through ET-12) covering Church numeral encoding/decoding, arithmetic operations, invariant preservation, property-based tests, and distributed correctness tests. SPEC-13 R5 includes `encoding/` in the module structure.

However, SPEC-08's directory structure (Section 4.1) has no entry for encoding tests:
- No `src/encoding/tests.rs` for unit tests
- No `tests/integration/encoding.rs` for arithmetic correctness tests
- No `tests/property/encoding.rs` for property tests (ET-10 specifies proptest)
- No entry in the invariant coverage matrix for encoding-related invariant verification

This means ET-1 through ET-12 have no assigned home in the project's test structure.

**Impact if unresolved:** The implementer must invent a test location for encoding tests, potentially placing them inconsistently with the rest of the test structure.
**Suggested resolution:** Add to Section 4.1:
- `src/encoding/tests.rs` for unit tests ET-1 through ET-5, ET-9, ET-12
- `tests/integration/encoding.rs` for ET-6 through ET-8 (arithmetic correctness)
- `tests/integration/encoding_distributed.rs` for ET-11 (distributed correctness, Fundamental Property)
- `tests/property/encoding.rs` for ET-10 (proptest)

---

### SC-006: No tests for SPEC-12 `reduce` subcommand and text DSL
**Severity:** HIGH
**Axis:** Completeness
**Section:** 3.9 (R21, Deployment tests)
**Requirement:** R21
**Problem:** SPEC-08 R21 defines 5 deployment tests (DP1-DP5) covering CLI argument parsing for `coordinator`, `worker`, `local`, and `generate` subcommands. SPEC-12 introduces the `reduce` and `inspect` subcommands, and SPEC-13 introduces `compute`. SPEC-12 defines 14 test requirements (T1-T14) covering binary roundtrip, text DSL parsing, inspect correctness, reduce correctness, and generator consistency.

SPEC-08's DP1-DP5 do not cover:
- The `reduce` subcommand (SPEC-12 T6, T7)
- The `inspect` subcommand (SPEC-12 T5, T14)
- The `compute` subcommand (SPEC-14 R22-R25)
- The text DSL parser and formatter (SPEC-12 T2, T3, T11, T12, T13)
- File format detection (SPEC-12 T8)

**Impact if unresolved:** The CLI/IO layer has 14+ test requirements defined in SPEC-12 that are invisible to SPEC-08's test strategy. The implementer following only SPEC-08 will ship an undertested CLI.
**Suggested resolution:** Either expand DP1-DP5 to DP1-DP19 (or similar) covering all CLI subcommands and I/O formats, or add a new section 3.18 "Unit Tests -- User I/O (SPEC-12)" incorporating SPEC-12's T1-T14.

---

### SC-007: No tests for SPEC-10 security layer
**Severity:** HIGH
**Axis:** Completeness
**Section:** 3 (entire Requirements section)
**Requirement:** (missing)
**Problem:** SPEC-10 defines 10 test requirements (T1-T10) covering token generation, TLS handshake, message size limits, unauthorized registration rejection, localhost binding, tier detection, and token debug redaction. Security tests are critical because a failure in token validation or TLS could expose the system to unauthorized workers corrupting computation results.

SPEC-08 has no security test section, no mention of SPEC-10, and the directory structure has no `src/security/tests.rs` or `tests/integration/security.rs`.

**Impact if unresolved:** Security tests are defined only in SPEC-10 with no integration into the overall test strategy, CI pipeline, or coverage tracking.
**Suggested resolution:** Add a new section "3.19 Unit Tests -- Security (SPEC-10)" incorporating T1-T10. Add `src/security/tests.rs` to the directory structure. Ensure feature-gated tests (T4, T5 for TLS) are documented as requiring `--features tls` to compile.

---

### SC-008: Invariant coverage matrix does not include T2, T3 in property-based tests
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 4.5 (Invariant Coverage Matrix)
**Requirement:** R25 (property-based tests)
**Problem:** The invariant coverage matrix (Section 4.5) shows:

| Invariant | Property Tests |
|-----------|---------------|
| T2 (Principal port interaction) | --- |
| T3 (Disjoint active pairs) | --- |

T2 and T3 have no property-based test coverage. While T2 is verified by unit tests (N9, N10, RE1-RE6) and T3 is verified by the invariant checker (IV6), neither has statistical validation across thousands of random nets. This is a gap because:

- T2 depends on the `connect` function correctly detecting principal ports. A subtle bug (e.g., off-by-one in PortId comparison) might pass targeted unit tests but fail on unusual net topologies generated by proptest.
- T3 is checked only by IV6 (redex queue scan). A property test like `prop_disjoint_active_pairs` would provide stronger evidence.

The runtime invariant checker (`assert_all_invariants`) covers I1, I2, I3, I4, and T3 (via IV6), but T2 is only implicitly covered by PB1 (if linearity holds and principal ports are correctly identified, T2 holds). This implicit coverage is not documented.

**Impact if unresolved:** The invariant coverage matrix understates the gap for T2 and T3. The evidence for these invariants relies entirely on targeted unit tests, which is weaker than the thousands of random cases that property tests provide for T1, T4, T6, T7.
**Suggested resolution:** Add PB13: `prop_redex_detection_only_principal_ports` -- verify that after building a random net, every entry in the redex queue is a pair connected via port 0. This directly tests T2 across random topologies. For T3, PB14: `prop_active_pairs_disjoint` -- verify that no AgentId appears in more than one active pair in the redex queue.

---

### SC-009: Graph isomorphism checker has O(n!) worst-case complexity with no mitigation for medium nets
**Severity:** MEDIUM
**Axis:** Testability
**Section:** 4.4 (Graph Isomorphism Checker), 3.11 (R24)
**Requirement:** R24
**Problem:** The isomorphism checker described in Section 4.4 uses a backtracking algorithm with O(n! * k) worst-case complexity. The spec acknowledges this is "sufficiently fast" for nets < 100 agents and offers a "weaker verification" (agent count by symbol + wire count) as an approximation for larger nets.

However, many tests require isomorphism on nets that are significantly larger than 100 agents:
- F1: `test_fundamental_iso_era_pairs` with 100 ERA-ERA pairs = 200 agents (before reduction; after reduction, 0 agents -- so isomorphism is trivial)
- PB5: `prop_fundamental_property` with random nets up to `max_agents` (unspecified, but proptest typically generates up to 100+ agents; post-reduction sizes vary)
- E9: `test_large_net_performance` with 10,000+ agents (but this tests performance, not isomorphism)
- SPEC-14 ET-11: distributed correctness for `add(50, 50)` producing Church(100) = 201 agents in Normal Form

For post-reduction Church numerals with ~200 agents, the backtracking algorithm will be slow unless heuristic pruning is very effective. The spec defers this decision to implementation (Open Question 1) but provides no intermediate requirement -- there is no MUST for an algorithm that scales to at least 500 agents, which is needed for SPEC-09 benchmarks.

**Impact if unresolved:** Property tests and fundamental property tests may timeout or run prohibitively slowly when random nets produce normal forms with 50-200 agents. The implementer may weaken all isomorphism checks to agent-count comparisons, undermining the primary improvement over the Haskell prototype (Section 5.1: "graph isomorphism instead of agent counting").
**Suggested resolution:** Add a SHOULD requirement: "The isomorphism checker SHOULD use canonical form computation (e.g., Weisfeiler-Leman refinement followed by canonical ordering) or a polynomial-time heuristic that handles nets up to 500 agents within 100ms. The backtracking fallback MAY be used for nets where the heuristic is inconclusive." This gives the implementer a concrete performance target.

---

### SC-010: Non-termination safety in proptest relies on reduce_n budget, but budget formula is heuristic
**Severity:** MEDIUM
**Axis:** Testability
**Section:** 4.3 (Random Net Generator)
**Requirement:** R28
**Problem:** R28 says: "the generator SHOULD either filter out non-terminating nets or use `reduce_n(budget)` with a generous budget as a safety valve." Section 4.3 provides a budget formula: `budget = n * n * 10`. This is a heuristic with no formal justification.

Consider: a net with 10 agents containing CON-DUP pairs can expand to hundreds of agents before collapsing. The interaction count for a single CON-DUP expansion followed by annihilation cascade is not `O(n^2)` -- it can be exponential in pathological cases (e.g., nested DUP chains). The formula `n * n * 10` with `n = 10` gives budget = 1000, which may be insufficient for some topologies and may cause legitimate terminating nets to be incorrectly treated as "non-terminating" (false positive) when the budget is exhausted.

Conversely, `proptest::test_runner::Config { timeout: 10_000 }` (10 seconds) is a reasonable fallback, but the spec does not specify what should happen when the timeout fires: should the test pass (assuming non-termination), fail, or be skipped?

**Impact if unresolved:** False positives (legitimate terminating nets exceeding the budget) will cause property tests to fail intermittently, leading to flaky CI. False negatives (non-terminating nets that happen to be slow) will cause test timeouts without clear resolution.
**Suggested resolution:** (a) Replace the fixed budget formula with a configurable constant (e.g., `MAX_PROPTEST_BUDGET = 100_000`) and document that this is a practical ceiling, not a theoretical guarantee. (b) Specify that when the budget is exceeded, the test SHOULD be marked as inconclusive (not failed): "if `reduce_n(budget)` does not reach Normal Form, the test SHOULD skip this input (via `prop_assume!`) rather than fail." (c) The `timeout` in proptest config is a separate safety net for cases where even dequeuing stale redexes is too slow.

---

### SC-011: SPEC-08 directory structure does not include `src/encoding/tests.rs` from SPEC-13 module layout
**Severity:** MEDIUM
**Axis:** Consistency
**Section:** 4.1 (Directory Structure)
**Requirement:** R1
**Problem:** SPEC-13 R5 defines 11 modules including `encoding/` (SPEC-14). SPEC-08's directory structure (Section 4.1) lists `tests.rs` files for 7 modules: `net`, `reduction`, `partition`, `merge`, `grid`, `protocol`, `cli`. Missing from SPEC-08's directory structure:

| Module (SPEC-13) | Has tests.rs in SPEC-08? | Tests defined elsewhere? |
|-------------------|--------------------------|--------------------------|
| `encoding/` | No | SPEC-14 ET-1 to ET-12 |
| `coordinator/` | No | SPEC-13 R19-R22 define FSM tests |
| `worker/` | No | SPEC-13 R25 defines FSM tests |
| `config/` | No | (merged into cli tests DP1-DP5?) |
| `observability/` | No | SPEC-11 T1-T9 |
| `security/` | No | SPEC-10 T1-T10 |

Six modules lack a `tests.rs` entry in SPEC-08's canonical directory structure.

**Impact if unresolved:** The implementer has no guidance on where to place unit tests for 6 out of 11 modules. Tests for these modules will be placed ad-hoc, degrading organizational consistency.
**Suggested resolution:** Update Section 4.1 to include `tests.rs` for all 11 modules from SPEC-13 R5.

---

### SC-012: No test for DUP-CON symmetry in commutation (only CON-DUP tested)
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 3.3 (R8, R9)
**Requirement:** R8, R9
**Problem:** SPEC-08 defines RE4 (`test_reduce_con_dup_expand`) for the CON-DUP commutation rule, and RE7 (`test_dispatch_symmetric_con_dup`) to verify that `(Dup, Con)` normalized to `(Con, Dup)` produces the same result. However, RE7 only tests that dispatch normalization occurs -- it does not verify the full post-reduction topology when the input order is `(Dup, Con)`.

Consider a bug where normalization swaps the agents but does not correctly swap the port references (e.g., `aux1(a)` and `aux1(b)` get confused because `a` and `b` switched roles). RE7 only checks "same result" which is vague -- does it mean the same Net? The same agent count? Isomorphism?

RE4 verifies exact topology but only for the canonical `(Con, Dup)` order. There is no test that constructs a DUP-CON active pair (DUP principal port connected to CON principal port) and verifies the exact post-reduction topology.

**Impact if unresolved:** A normalization bug that incorrectly swaps port references would not be caught by RE7's vague "same result" check. The implementer may implement RE7 as a simple agent count comparison, missing topological errors.
**Suggested resolution:** Strengthen RE7's description: "Construct a net with DUP principal port connected to CON principal port. Reduce. Verify that the post-reduction topology is isomorphic to the result of RE4 (CON-DUP canonical order)." This ensures that symmetry normalization preserves exact topology, not just a weak metric.

---

### SC-013: Workload Profile coverage matrix omits Profile B/C for property tests
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 4.6 (Workload Profile Test Coverage)
**Requirement:** R25 (property-based tests)
**Problem:** The Workload Profile Test Coverage matrix (Section 4.6) shows:

| Profile | Property |
|---------|----------|
| A (EP) | PB5 |
| B (Expansion) | PB5, PB6 |
| C (Sequential) | PB5 |

PB5 (`prop_fundamental_property`) is the catch-all property test. But PB5 uses `arb_net`, which generates random nets with random symbol distributions. Random nets do not reliably produce Profile B behavior (CON-DUP expansion) or Profile C behavior (level-dependent cascades). PB6 specifically targets CON-DUP, but there is no equivalent property test specifically targeting Profile C (DualTree-like sequential cascade patterns).

The random net generator (Section 4.3) generates nets with randomly paired ports, which tends to produce "flat" topologies (many independent redexes = Profile A) rather than tree-structured cascades (Profile C). The `arb_net_weighted` variant controls symbol distribution but not topology structure.

**Impact if unresolved:** Property tests provide strong coverage for Profile A (random nets tend to be embarassingly parallel) but weak coverage for Profiles B and C. The statistical confidence claim is overstated for the profiles that are most challenging for distribution.
**Suggested resolution:** Add two targeted property generators: (a) `arb_condup_net(max_pairs)` -- generates nets with predominantly CON-DUP pairs, guaranteeing Profile B behavior; (b) `arb_chain_net(max_depth)` -- generates tree or chain topologies with level-dependent reduction, approximating Profile C. Then add corresponding property tests PB13/PB14 to the matrix.

---

### SC-014: E9 performance test has no failure criterion tied to complexity
**Severity:** LOW
**Axis:** Testability
**Section:** 3.14 (R30)
**Requirement:** R30
**Problem:** E9 (`test_large_net_performance`) specifies: "Net with 10,000+ agents: reduce_all completes in reasonable time (< 30 seconds). Verifies O(S) complexity. SPEC-03 R22." The 30-second threshold is arbitrary and machine-dependent. A CI server with a slow CPU might fail E9 even with a correct O(S) implementation. Conversely, an O(S^2) implementation might pass E9 on a fast machine with S < 10,000.

Furthermore, E9 claims to "verify O(S) complexity" but a single data point (one net, one threshold) cannot verify asymptotic complexity. Complexity verification requires at least 2-3 data points at different scales to confirm that the growth rate is linear.

**Impact if unresolved:** E9 is flaky on slow CI machines and does not actually verify what it claims (O(S) complexity). It serves only as a smoke test for "not catastrophically slow."
**Suggested resolution:** (a) Replace the fixed 30-second threshold with a relative check: run `reduce_all` on nets with 1,000 and 10,000 agents, verify that `time(10k) / time(1k) < 15` (allowing for a 50% margin over the expected 10x linear scaling). (b) Mark E9 with `#[ignore]` since it is a performance test, not a correctness test, and should not block CI on every push.

---

### SC-015: `test_net_serialize_roundtrip` (N16) references SPEC-02 R22 and R24, but the correct SPEC-02 requirement for self-contained format is R25
**Severity:** LOW
**Axis:** Consistency
**Section:** 3.2 (R7, N16-N17)
**Requirement:** R7
**Problem:** N17 (`test_net_serialize_self_contained`) cites "R23" as the SPEC-02 requirement. In SPEC-02, R23 is about distinguishing FreePort (Lafont) vs. FreePort (Boundary) in the representation. The self-contained serialization requirement is actually SPEC-02 R25: "The serialized format MUST be self-contained."

This is a companion to SC-003 -- the R-number references are systematically off by 2.

**Impact if unresolved:** Minor confusion. The implementer looking up SPEC-02 R23 will find an unrelated requirement.
**Suggested resolution:** Correct N17's SPEC-02 Req column from "R23" to "R25."

---

### SC-016: SPEC-08 defines `ReductionStrategy` but does not reference SPEC-03 for authority
**Severity:** LOW
**Axis:** Consistency
**Section:** 3.17 (R37-R38)
**Requirement:** R37
**Problem:** R37 introduces a `ReductionStrategy` enum with `First`, `Last`, `Random(u64)` and a `reduce_all_with_strategy` function. These are defined entirely within SPEC-08 (the test strategy spec) rather than in SPEC-03 (the reduction engine spec). This means the reduction engine's public API is split across two specs: SPEC-03 defines `reduce_step`, `reduce_all`, and `reduce_n`, while SPEC-08 defines `reduce_all_with_strategy`.

SPEC-03 makes no mention of reduction strategies or the ability to select redex order. This is a design decision that belongs in SPEC-03 (even if only for test mode) because it affects the reduction engine's API surface.

R38 acknowledges this by suggesting `#[cfg(test)]` feature-gating, but the type definition and API belong in the reduction module, not in the test strategy spec.

**Impact if unresolved:** Minor organizational issue. The implementer must look at two specs to understand the full reduction engine API.
**Suggested resolution:** Add a note in SPEC-08 R37 that this requirement SHOULD be reflected in SPEC-03 during its next revision. Alternatively, SPEC-03 should add a requirement for `ReductionStrategy` support, even if `#[cfg(test)]`-gated.

---

### SC-017: No explicit test for SPEC-01 D2 (local reduction equivalence) isolation
**Severity:** LOW
**Axis:** Completeness
**Section:** 4.5 (Invariant Coverage Matrix)
**Requirement:** R22 (integration tests)
**Problem:** The invariant coverage matrix shows D2 (Local Reduction Equivalence) covered by:
- Property tests: PB10
- Integration tests: I1-I11

However, D2's formal statement is: "reducing an Active Pair in a partition produces the same topological changes as reducing that pair in the global net." None of the listed tests explicitly verify this in isolation. PB10 checks that "partition, reduce locally, merge produces a valid net" -- but this tests the entire pipeline, not D2 specifically. I1-I11 also test the full pipeline.

A dedicated D2 test would: (a) construct a net with a known redex, (b) reduce the redex globally and record the topological delta, (c) partition the net such that the redex is internal, (d) reduce the redex in the partition, (e) compare the topological delta. This is not what any listed test does.

**Impact if unresolved:** D2 is verified only indirectly. A bug where partition reduction produces a slightly different topology (e.g., incorrect port ordering in a sub-net) would not be caught until the full pipeline test fails, making diagnosis harder.
**Suggested resolution:** Add a dedicated test (or property test) that explicitly verifies D2 by comparing single-step reduction in isolation vs. single-step reduction in a partition.

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 2 |
| HIGH | 5 |
| MEDIUM | 5 |
| LOW | 5 |

## Mandatory (must fix before implementation)

- **SC-001:** SPEC-08 does not cover SPEC-10 through SPEC-14 test requirements -- 59 tests missing from the canonical test strategy
- **SC-002:** Test label namespace collisions -- T1 means 4 different things across specs; I1 means 2 different things within SPEC-08 itself
- **SC-003:** Cross-reference error -- serialization requirements are SPEC-02 R24-R26, not R22-R24
- **SC-004:** Integration test labels I1-I11 shadow SPEC-01 invariant labels I1-I5
- **SC-005:** No encoding module tests in directory structure or coverage matrices (SPEC-14 ET-1 to ET-12)
- **SC-006:** No tests for SPEC-12 subcommands and text DSL (T1-T14)
- **SC-007:** No tests for SPEC-10 security layer (T1-T10)

## Recommended (should fix)

- **SC-008:** Invariant coverage matrix lacks property tests for T2 and T3
- **SC-009:** Graph isomorphism O(n!) with no concrete performance target for medium nets
- **SC-010:** Non-termination safety relies on heuristic budget formula with no specification for inconclusive results
- **SC-011:** Directory structure missing tests.rs for 6 of 11 modules
- **SC-012:** DUP-CON symmetry test RE7 has vague "same result" criterion
- **SC-013:** Workload Profile coverage matrix weak for Profiles B and C in property tests
