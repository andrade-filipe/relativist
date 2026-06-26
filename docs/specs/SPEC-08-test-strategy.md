# SPEC-08: Test Strategy

**Status:** Revised v3
**Depends on:** SPEC-00 (Glossary), SPEC-01 (Invariants), SPEC-02 (Net Representation), SPEC-03 (Reduction Engine), SPEC-04 (Partitioning), SPEC-05 (Merge and Grid Cycle), SPEC-06 (Wire Protocol), SPEC-07 (Deployment), SPEC-10 (Security), SPEC-11 (Observability), SPEC-12 (User I/O), SPEC-13 (System Architecture), SPEC-14 (Arithmetic Encoding)
**Gray zones resolved:** ---
**References consumed:** REF-001 (Lafont 1990), REF-002 (Lafont 1997), REF-003 (HVM2), REF-005 (Mackie & Pinto 2002), REF-018 (Arrighi et al. 2024)
**Discussions consumed:** DISC-003 v2 (strong confluence to distributed determinism, P1-P5, evidence classification), DISC-006 v2 (overhead and granularity, test scenario design)
**Arguments consumed:** ARG-001 (central argument, P1-P6 framework, fundamental property), ARG-002 (partitioning preserves structure, split/merge identity, C1-C3), ARG-003 (merge protocol guarantees border completeness, P3), ARG-004 (practical viability, workload profiles A/B/C)
**Code analyses consumed:** AC-001 (CoreSpec.hs, 13 tests, 5 gaps), AC-002 (PartitionSpec.hs, partition tests, merge tests), AC-004 (GridSpec.hs + TreeMapReduceSpec.hs, 6 fundamental property tests), AC-005 (BenchmarkSpec.hs, 27 tests, 6 gaps)
**Adversarial review consumed:** SPEC-08-round1-critic.md (17 issues: 2 CRITICAL, 5 HIGH, 5 MEDIUM, 5 LOW)

---

## 1. Purpose

This spec defines the complete TDD test strategy for Relativist: the test taxonomy (unit, integration, property-based, end-to-end), the specific tests for each component (Net, Reduction, Partition, Merge, Grid Loop, Wire Protocol, Deployment, Encoding, Security, Observability, User I/O, Coordinator FSM, Worker FSM), the property-based testing strategy using `proptest` for statistical validation of IC invariants, the verification of the Fundamental Property (SPEC-01, G1), the runtime invariant checker, the test fixtures and random net generators, the graph isomorphism checker, and the mapping of gaps identified in the Haskell prototype that Relativist MUST cover. The goal is that every MUST requirement from SPEC-01 through SPEC-14 has at least one corresponding test. Test requirements defined within individual specs (SPEC-10 T1-T10, SPEC-11 T1-T9/T8a, SPEC-12 T1-T14, SPEC-14 ET-1 through ET-12) are incorporated into this spec using a global namespace convention to avoid label collisions.

## 2. Definitions

Terms defined in SPEC-00 (Glossary) are used without redefinition. Terms introduced in this spec:

| Term | Definition |
|------|-----------|
| **Test Fixture** | An interaction net constructed programmatically for use as test input. Each fixture has a name, known agents, known wires, and an expected result after reduction. Fixtures are deterministic and shared across test files. |
| **Round-Trip Test** | A test that verifies that an operation followed by its inverse produces the original value. Examples: `merge(split(net, n)) ~ net` (SPEC-01, D1), `deserialize(serialize(net)) == net` (SPEC-02, R24-R26). |
| **Test Label Namespace** | A globally unique prefix for test IDs within each spec's scope. Convention: tests defined within SPEC-08 use module-specific prefixes (N, RE, P, M, PS, FI, GL, WP, DP, INT, F, PB, CD, E, E2E, IV). Tests defined in other specs use spec-qualified prefixes: `SEC-` (SPEC-10), `OBS-` (SPEC-11), `UIO-` (SPEC-12), `ARCH-` (SPEC-13), `ENC-` (SPEC-14). This convention ensures no label collides across the spec suite. |
| **Property-Based Test** | A test that generates random inputs via `proptest`, applies an operation, and verifies that an invariant holds for all generated inputs. Provides statistical confidence over thousands of cases, addressing the evidence gap identified in DISC-003 v2, Section 5.3. |
| **Golden Test** | A test that compares the output of an operation against a previously validated and stored result. Used for regression testing of reduction of known nets. |
| **Invariant Checker** | A function that verifies all invariants from SPEC-01 (T1-T7, D1-D6, I1-I5, G1) applicable to a Net at a given point. Activated via `debug_assertions` after each `reduce_step` and after each merge. |
| **Isomorphism Check** | Comparison of two nets by graph isomorphism (structural equality modulo AgentId renaming, SPEC-00 Section 6.12). Strictly stronger than comparison by agent count. Used to verify the Fundamental Property (G1). |
| **Workload Profile** | One of three categories of test nets (ARG-004, Parte I, Passo 6): **Profile A** (Embarrassingly Parallel -- independent redexes, 1 round, zero border redexes; e.g., EP-Annihilation), **Profile B** (Expansion with Collapse -- CON-DUP dominant, multiple rounds, emergent borders; e.g., CON-DUP Expansion), **Profile C** (Sequential Dependency -- level-dependent, many rounds, massive borders; e.g., DualTree). |

---

## 3. Requirements

### 3.1 Test Organization

**R1.** Tests MUST be organized in four tiers: **(MUST)**
- (a) **Unit tests** per module in `src/<module>/tests.rs` (inline `#[cfg(test)]` modules).
- (b) **Integration tests** in `tests/integration/`.
- (c) **Property-based tests** in `tests/property/`.
- (d) **End-to-end tests** in `tests/e2e/` (require Docker or multiple processes; marked `#[ignore]`).

**R2.** Each test function MUST follow the naming convention: `test_<module>_<functionality>_<scenario>`. Example: `test_reduction_con_dup_cross_config`. **(MUST)**

**R3.** The project MUST use `cargo test` as the standard test runner. Tests that require external infrastructure (Docker, TCP connections between processes) MUST be marked with `#[ignore]` and executed separately via `cargo test -- --ignored`. **(MUST)**

**R4.** The project MUST use the `proptest` crate for property-based tests. **(MUST)**

**R5.** The project SHOULD integrate tests with CI (GitHub Actions) executing `cargo test` on each push and `cargo test -- --ignored` on a scheduled basis (e.g., nightly). **(SHOULD)**

**R6.** The project SHOULD track code coverage using `cargo-tarpaulin` or `cargo-llvm-cov`. A minimum line coverage target of 80% SHOULD be established after the first modules are implemented. **(SHOULD)**

### 3.2 Unit Tests -- Net (SPEC-02)

**R7.** The Net module MUST have unit tests covering all CRUD operations, redex queue behavior, and serialization. **(MUST)**

| ID | Test | SPEC-02 Req | What it verifies |
|----|------|-------------|------------------|
| N1 | `test_net_empty` | R6 | `Net::new()` has 0 agents, 0 wires, empty redex queue, `next_id == 0` |
| N2 | `test_net_create_agent_con` | R11 | `create_agent(Con)` returns AgentId, agent present in arena with correct symbol |
| N3 | `test_net_create_agent_dup` | R11 | Same for Dup |
| N4 | `test_net_create_agent_era` | R11 | Same for Era |
| N5 | `test_net_create_agent_id_monotonic` | R10 | IDs are strictly increasing on each `create_agent` |
| N6 | `test_net_remove_agent` | R12 | Slot marked as `None`, all ports disconnected |
| N7 | `test_net_remove_agent_id_not_reused` | R12, I3 | After removal, `next_id` does not regress |
| N8 | `test_net_connect_bidirectional` | R13, R18 | `connect(a, b)` implies `get_target(a) == b` and `get_target(b) == a` |
| N9 | `test_net_connect_principal_creates_redex` | R13 | `connect(AgentPort(x, 0), AgentPort(y, 0))` inserts into the redex queue |
| N10 | `test_net_connect_auxiliary_no_redex` | R13 | `connect(AgentPort(x, 1), AgentPort(y, 1))` does NOT insert into the redex queue |
| N11 | `test_net_disconnect` | R14 | Disconnected port no longer points to any target |
| N12 | `test_net_get_target` | R15 | Returns the correct PortRef after `connect` |
| N13 | `test_net_is_reduced_empty` | R16 | Empty net: `is_reduced() == true` |
| N14 | `test_net_is_reduced_with_redex` | R16 | Net with redex: `is_reduced() == false` |
| N15 | `test_net_stale_redex_discarded` | R17 | Redex whose agent was removed is discarded silently |
| N16 | `test_net_serialize_roundtrip` | R24, R26 | `deserialize(serialize(net)) == net` |
| N17 | `test_net_serialize_self_contained` | R25 | Deserialized bytes reconstruct a complete Net |
| N18 | `test_net_freeport_index` | R6(e) | `free_port_index` correctly maps border IDs to AgentPorts |

### 3.3 Unit Tests -- Reduction Engine (SPEC-03)

**R8.** The reduction engine MUST have unit tests for each of the 6 interaction rules, verifying exact post-reduction topology. **(MUST)**

| ID | Test | Rule | What it verifies |
|----|------|------|------------------|
| RE1 | `test_reduce_con_con_cross` | CON-CON (Annihilation) | 2 agents removed, 0 created, cross-connect: `target(a.aux1) <-> target(b.aux2)`, `target(a.aux2) <-> target(b.aux1)`. SPEC-01 T5. |
| RE2 | `test_reduce_dup_dup_parallel` | DUP-DUP (Annihilation) | 2 removed, 0 created, parallel-connect: `target(a.aux1) <-> target(b.aux1)`, `target(a.aux2) <-> target(b.aux2)`. SPEC-01 T5. |
| RE3 | `test_reduce_era_era_void` | ERA-ERA (Void) | 2 removed, 0 created, empty net. SPEC-01 T5. |
| RE4 | `test_reduce_con_dup_expand` | CON-DUP (Commutation) | 2 removed, 4 created (2 DUP + 2 CON), 8 internal wires in crossed configuration. SPEC-01 T5. |
| RE5 | `test_reduce_con_era_propagate` | CON-ERA (Erasure) | 2 removed, 2 ERA created, each ERA connected to a former auxiliary neighbor of the CON. SPEC-01 T5. |
| RE6 | `test_reduce_dup_era_propagate` | DUP-ERA (Erasure) | 2 removed, 2 ERA created, each ERA connected to a former auxiliary neighbor of the DUP. SPEC-01 T5. |

**R9.** The reduction engine MUST have tests for the dispatch mechanism and normalization of symmetric pairs. **(MUST)**

| ID | Test | What it verifies |
|----|------|------------------|
| RE7 | `test_dispatch_symmetric_con_dup` | Construct a net with DUP principal port connected to CON principal port. Reduce. Verify that the post-reduction topology is isomorphic to the result of RE4 (CON-DUP canonical order). SPEC-03 R9. |
| RE8 | `test_dispatch_symmetric_con_era` | `(Era, Con)` normalized to `(Con, Era)` produces the same result. SPEC-03 R9. |
| RE9 | `test_dispatch_symmetric_dup_era` | `(Era, Dup)` normalized to `(Dup, Era)` produces the same result. SPEC-03 R9. |
| RE10 | `test_dispatch_all_9_pairs` | All 9 pairs `(Symbol, Symbol)` resolve to one of the 6 rules. SPEC-03 R8. |

**R10.** The reduction engine MUST have tests for the reduction loop. **(MUST)**

| ID | Test | What it verifies |
|----|------|------------------|
| RE11 | `test_reduce_step_valid` | `reduce_step` consumes one redex and increments the interaction counter |
| RE12 | `test_reduce_step_stale` | `reduce_step` discards a stale redex silently. SPEC-03 R19, SPEC-01 I4. |
| RE13 | `test_reduce_all_empty` | `reduce_all` on an empty net returns immediately with 0 interactions. SPEC-03 R13. |
| RE14 | `test_reduce_all_single_redex` | Net with 1 redex: `reduce_all` performs 1 interaction. SPEC-03 R13. |
| RE15 | `test_reduce_all_chain` | ERA><CON(ERA,ERA) -> cascade -> empty net (maps to AC-001 `test_reduceAllChain`). SPEC-03 R13. |
| RE16 | `test_reduce_n_budget` | `reduce_n(budget=1)` on a net with 3 redexes: performs 1 interaction and stops. SPEC-03 R14. |
| RE17 | `test_reduce_n_early_stop` | `reduce_n(budget=100)` on a net with 2 redexes: performs 2 interactions and stops. SPEC-03 R14. |

**R11.** The reduction engine MUST have tests for incremental redex detection during reduction. **(MUST)**

| ID | Test | What it verifies |
|----|------|------------------|
| RE18 | `test_incremental_con_dup_new_redexes` | CON-DUP rule: if external neighbors are principal ports, new redexes are inserted in the queue. SPEC-03 R11, R18. |
| RE19 | `test_incremental_annihilate_new_redex` | CON-CON with cross-connect: if reconnection creates an active pair, it is detected. SPEC-03 R11. |
| RE20 | `test_incremental_no_false_redex` | Reconnection between auxiliary ports does NOT insert into the queue. SPEC-03 R11. |

**R12.** The reduction engine SHOULD have tests for the interaction counter discriminated by rule type (SPEC-03, R17). **(SHOULD)**

| ID | Test | What it verifies |
|----|------|------------------|
| RE21 | `test_counter_by_rule_type` | Mixed net: annihilation/commutation/erasure/void counters sum to the total |

### 3.4 Unit Tests -- Partition (SPEC-04)

**R13.** The partition module MUST have tests for the correctness conditions C1-C3 and the split/merge round-trip. **(MUST)**

| ID | Test | SPEC-04 Req | What it verifies |
|----|------|-------------|------------------|
| P1 | `test_partition_c1_coverage` | R6 (C1) | Every agent present in exactly one partition. SPEC-01 D1a, D5a. |
| P2 | `test_partition_c2_wire_coverage` | R7 (C2) | Every wire classified as internal, interface, or border; none lost. SPEC-01 D1b. |
| P3 | `test_partition_c3_freeport_bijectivity` | R8 (C3) | Each border_id has exactly 2 FreePort sentinels in 2 distinct partitions. SPEC-01 D1c. |
| P4 | `test_partition_roundtrip_identity` | D1 | `merge(split(net, n)) ~ net` for a non-trivial net. SPEC-01 D1. |
| P5 | `test_partition_roundtrip_various_n` | D1 | Round-trip for n = 1, 2, 3, 4, 8. SPEC-01 D1. |

**R14.** The partition module MUST have tests for FreePort (Boundary) and border wires. **(MUST)**

| ID | Test | SPEC-04 Req | What it verifies |
|----|------|-------------|------------------|
| P6 | `test_freeport_created_on_border` | R11 | Border wire generates FreePort(bid) on each side with unique border_id |
| P7 | `test_freeport_index_populated` | R13 | `free_port_index` contains correct mapping border_id -> AgentPort |
| P8 | `test_freeport_linearity` | R14 | Each FreePort participates in exactly one wire |
| P9 | `test_freeport_id_no_collision` | R12 | Border IDs do not collide with pre-existing FreePort IDs |

**R15.** The partition module MUST have tests for static ID space partitioning. **(MUST)**

| ID | Test | SPEC-04 Req | What it verifies |
|----|------|-------------|------------------|
| P10 | `test_id_range_disjoint` | R16 | Worker ID ranges do not overlap |
| P11 | `test_id_range_no_collision_after_reduce` | R17 | After local reduction with CON-DUP (which creates agents), IDs do not collide between workers. SPEC-01 D4. |
| P12 | `test_next_id_initialized` | R18 | `next_id` of partition >= start of the assigned range |

### 3.5 Unit Tests -- Merge (SPEC-05)

**R16.** The merge module MUST have tests for all merge operations and border redex handling. **(MUST)**

| ID | Test | SPEC-05 Req | What it verifies |
|----|------|-------------|------------------|
| M1 | `test_merge_agents_unified` | R2, R3 | All agents from all partitions present in the result net. No ID collisions. |
| M2 | `test_merge_border_reconnected` | R4, R5 | Border wires restored correctly via free_port_index and border map. `Net::connect` used for reconnection. |
| M3 | `test_merge_erased_border_discarded` | R6 | Border whose agent was removed by erasure is discarded silently (not an error). |
| M4 | `test_merge_both_sides_erased_discarded` | R7 | Border where both sides were erased is discarded silently. |
| M5 | `test_merge_redex_queue_populated` | R9 | Restored wires that form a redex (both endpoints are principal ports) are inserted into the queue. |
| M6 | `test_merge_next_id_max` | R8 | `next_id` of result net is the maximum of all partitions' `next_id` values. |
| M7 | `test_merge_invariants_hold` | R11 | After merge in debug mode, `assert_all_invariants()` passes (T1, I1, I2). |

### 3.6 Unit Tests -- Partition Strategy (SPEC-04)

**R17.** The partition strategy MUST have tests for the baseline round-robin strategy. **(MUST)**

| ID | Test | What it verifies |
|----|------|------------------|
| PS1 | `test_roundrobin_all_agents_assigned` | Every live agent receives a WorkerId |
| PS2 | `test_roundrobin_balanced` | Partitions have approximately equal size (+/- 1 agent) |
| PS3 | `test_partition_trivial_n1` | n=1 returns the entire net without borders. SPEC-04 R2. |
| PS4 | `test_partition_n_greater_than_agents` | n > agents: excess partitions are empty. SPEC-04 R3. |
| PS5 | `test_partition_deterministic` | Same net + same plan produces identical output across invocations. SPEC-04 R4. |

### 3.7 Unit Tests -- FreePort Index Reconstruction (SPEC-05)

**R18.** The FreePort index reconstruction MUST have tests covering the three scenarios from SPEC-05 R22. **(MUST)**

| ID | Test | SPEC-05 Req | What it verifies |
|----|------|-------------|------------------|
| FI1 | `test_freeport_index_after_reconnection` | R22 (scenario 1) | After local reduction reconnects a FreePort to a new agent, the index reflects the new endpoint |
| FI2 | `test_freeport_index_after_erasure` | R22 (scenario 2) | After erasure of the agent connected to FreePort(bid), the index no longer contains bid |
| FI3 | `test_freeport_index_after_condup` | R22 (scenario 3) | After CON-DUP with FreePort inheritance, the index points to the correct new agent |

### 3.8 Unit Tests -- Grid Loop (SPEC-05)

**R19.** The grid loop MUST have unit tests for termination and convergence behavior. **(MUST)**

| ID | Test | SPEC-05 Req | What it verifies |
|----|------|-------------|------------------|
| GL1 | `test_grid_loop_terminates_empty` | R27 | Empty net: loop terminates immediately with 0 rounds |
| GL2 | `test_grid_loop_terminates_single_redex` | R27 | Net with 1 internal redex, 2 workers: converges in 1 round |
| GL3 | `test_grid_loop_multiple_rounds` | R28 | Net requiring >1 round (border redexes after first merge): converges |
| GL4 | `test_grid_loop_max_rounds_respected` | R29 | `max_rounds` limit is respected; metrics indicate non-convergence |
| GL5 | `test_grid_loop_n1_degenerate` | R26 | n=1: reduces locally without partitioning (equivalent to `reduce_all`) |

### 3.9 Unit Tests -- Wire Protocol and Deployment (SPEC-06, SPEC-07)

**R20.** The wire protocol MUST have unit tests for message serialization and framing. **(MUST)**

| ID | Test | SPEC-06 Req | What it verifies |
|----|------|-------------|------------------|
| WP1 | `test_message_serialize_roundtrip` | --- | `deserialize(serialize(msg)) == msg` for all message types |
| WP2 | `test_frame_length_prefix` | --- | Frame header contains correct length for variable-size payloads |
| WP3 | `test_frame_checksum` | --- | Checksum in frame header matches computed checksum of payload |

**R21.** Deployment-related tests SHOULD verify CLI argument parsing and workload generators. **(SHOULD)**

| ID | Test | What it verifies |
|----|------|------------------|
| DP1 | `test_cli_coordinator_args` | `coordinator` subcommand parses required arguments |
| DP2 | `test_cli_worker_args` | `worker` subcommand parses required arguments |
| DP3 | `test_cli_local_args` | `local` subcommand parses required arguments |
| DP4 | `test_cli_generate_args` | `generate` subcommand parses required arguments |
| DP5 | `test_workload_generators` | Each pre-defined workload (tree-sum, era-chain, con-dup-expansion, dual-tree, tree-sum-balanced) produces a valid Net satisfying T1, I1, I2, I3 |

---

### 3.10 Integration Tests

**R22.** Relativist MUST have integration tests exercising the complete local pipeline: construction -> partition -> local reduction -> merge -> border resolution -> verification. These tests use local mode (SPEC-07) and do NOT require Docker or TCP. **(MUST)**

| ID | Test | Workload Profile | What it verifies |
|----|------|------------------|------------------|
| INT1 | `test_pipeline_era_chain_2w` | A (EP) | ERA-ERA chain, 2 workers. Result == sequential reduction. Maps to AC-004 `test_gridEqualsLocalEraChain`. |
| INT2 | `test_pipeline_mixed_2w` | A/B mixed | Mixed net (CON + ERA + DUP + ERA), 2 workers. Maps to AC-004 `test_gridEqualsLocalMixed`. |
| INT3 | `test_pipeline_era_4w` | A (EP) | 4 pairs ERA-ERA, 4 workers. Maps to AC-004 `test_gridEqualsLocal4Workers`. |
| INT4 | `test_pipeline_treesum_2w` | A (EP) | TreeSum [1,2,3,4], 2 workers. `extract_result == 10`. Maps to AC-004 `test_gridSum4`. |
| INT5 | `test_pipeline_treesum_4w` | A (EP) | TreeSum [1..8], 4 workers. `extract_result == 36`. Maps to AC-004 `test_gridSum8_4w`. |
| INT6 | `test_pipeline_dual_tree_depth3_2w` | C (Sequential) | DualTree depth 3, 2 workers. 0 agents in result. Maps to AC-005 `test_dualTreeGrid3`. |
| INT7 | `test_pipeline_dual_tree_depth5_4w` | C (Sequential) | DualTree depth 5, 4 workers. 0 agents in result. Maps to AC-005 `test_dualTreeGrid5`. |
| INT8 | `test_pipeline_condup_expansion_2w` | B (Expansion) | CON-DUP Expansion, 2 workers. Result == sequential. Addresses AC-005 Gap L5. |
| INT9 | `test_pipeline_multiple_rounds` | B/C | Net requiring >1 round (border redexes after first merge). Verify convergence. |
| INT10 | `test_pipeline_empty_net` | --- | Empty net, any n. Returns immediately. |
| INT11 | `test_pipeline_single_border_redex` | --- | 1 active pair, 2 workers (1 agent per worker). The redex is a border redex. Verify resolution. |

### 3.11 Fundamental Property Tests

**R23.** Relativist MUST have tests that verify the Fundamental Property (SPEC-01, G1) by graph isomorphism, not merely by agent count or scalar extraction. **(MUST)**

```
reduce_all(net) ~ run_grid(net, n)
```

where `~` denotes isomorphism (SPEC-00 Section 6.12).

| ID | Test | Workload Profile | What it verifies |
|----|------|------------------|------------------|
| F1 | `test_fundamental_iso_era_pairs` | A (EP) | EP-Annihilation: nets with 10, 50, 100 ERA-ERA pairs, for n = 1, 2, 4. Graph isomorphism. |
| F2 | `test_fundamental_iso_treesum` | A (EP) | TreeSum [1..N] for N = 4, 8, 16, for n = 1, 2, 4. Graph isomorphism. |
| F3 | `test_fundamental_iso_dual_tree` | C (Sequential) | DualTree depth 1-5, for n = 1, 2, 4. Graph isomorphism. |
| F4 | `test_fundamental_iso_condup` | B (Expansion) | CON-DUP Expansion, N = 2, 5, 10, for n = 1, 2. Graph isomorphism. |
| F5 | `test_fundamental_interaction_count_invariant` | All | For each net and each n: total interactions (sequential) == total (distributed). Verifies SPEC-01 T7. |

**R24.** The isomorphism verification MUST be implemented as a helper function available to all test modules. **(MUST)**

```rust
/// Verifies whether two nets are isomorphic (same topology modulo AgentId renaming).
/// Returns true if there exists a bijection between AgentIds that preserves:
/// - Symbols: agent_a.symbol == agent_b.symbol
/// - Connectivity: if port(a, p) <-> port(a', p'), then port(f(a), p) <-> port(f(a'), p')
/// - FreePort references: preserved without renaming
///
/// SPEC-00 Section 6.12 defines isomorphism formally.
fn nets_isomorphic(a: &Net, b: &Net) -> bool
```

### 3.12 Property-Based Tests

**R25.** Relativist MUST have property-based tests using `proptest` to generate random nets and verify invariants across thousands of cases. This directly addresses the evidence gap identified in DISC-003 v2, Section 5.3: "QuickCheck to generate random IC nets and verify the property for thousands of cases." **(MUST)**

The random net generators MUST produce valid nets (satisfying T1, T2, I1, I2). The generation strategy is:

1. Choose number of agents N (proptest parameter).
2. For each agent, choose symbol randomly from {Con, Dup, Era}.
3. Connect ports randomly respecting linearity (each port connects to exactly one other port). Unmatched ports connect to FreePorts.
4. All connections between principal ports form redexes and are inserted into the queue.
5. Verify `debug_assert!(net.assert_all_invariants())`.

| ID | Property | What it verifies | SPEC-01 invariant | Source |
|----|----------|------------------|-------------------|--------|
| PB1 | `prop_linearity_preserved_after_reduce` | After `reduce_all`, T1 (linearity) holds | T1 | SPEC-01 |
| PB2 | `prop_confluence_same_result` | Reducing with different strategies (First, Last, Random) produces isomorphic nets | T4, T6 | SPEC-01, ARG-001 P1 |
| PB3 | `prop_interaction_count_invariant` | All reduction strategies produce the same total interaction count | T7 | SPEC-01, DISC-003 v2 Sec 1.3 |
| PB4 | `prop_roundtrip_partition_merge` | `merge(split(net, n)) ~ net` for random n | D1 | SPEC-01, ARG-002 |
| PB5 | `prop_fundamental_property` | `reduce_all(net) ~ run_grid(net, n)` for random n | G1 | SPEC-01, ARG-001 |
| PB6 | `prop_condup_expand_then_collapse` | Nets with CON-DUP redexes: after expansion + full reduction, result is isomorphic to sequential | T4, G1 | DISC-003 v2 Sec 5.2 pt 3 |
| PB7 | `prop_all_six_rules_exercised` | For sufficiently large random nets, all 6 rules are exercised in at least some executions | T5 | DISC-003 v2 Sec 5.2 pt 2 |
| PB8 | `prop_serialization_roundtrip` | `deserialize(serialize(net)) == net` for random nets | --- | SPEC-02 R24-R26 |
| PB9 | `prop_no_dangling_references_after_reduce` | After `reduce_all`, every AgentPort in the port array points to an existing agent | I2 | SPEC-01 |
| PB10 | `prop_merge_after_local_reduce_correct` | Partition, reduce locally, merge: result net satisfies T1, I1, I2, D1 | D1-D4 | SPEC-01, ARG-002, ARG-003 |
| PB11 | `prop_border_redex_completeness` | After merge + reduce_all, no unresolved border redexes remain | D3 | SPEC-01, ARG-003 P3 |
| PB12 | `prop_id_uniqueness_after_merge` | After distributed reduce + merge, no duplicate AgentIds exist | D4 | SPEC-01, ARG-001 P4 |
| PB13 | `prop_redex_detection_only_principal_ports` | After building a random net, every entry in the redex queue is a pair connected via port 0. Directly tests T2 across random topologies. | T2 | SPEC-01 |
| PB14 | `prop_active_pairs_disjoint` | After building a random net and running reduce_all, verify that no AgentId appears in more than one active pair in the redex queue at any step. | T3 | SPEC-01 |
| PB15 | `prop_condup_fundamental_property` | Using `arb_condup_net(max_pairs)` (predominantly CON-DUP pairs), verify `reduce_all(net) ~ run_grid(net, n)` for random n. Targets Profile B behavior. | G1 | ARG-004 |
| PB16 | `prop_chain_fundamental_property` | Using `arb_chain_net(max_depth)` (tree/chain topologies with level-dependent reduction), verify `reduce_all(net) ~ run_grid(net, n)` for random n. Targets Profile C behavior. | G1 | ARG-004 |

**R26.** The property-based tests MUST generate nets that exercise ALL 6 interaction rules, not only ERA-ERA. This directly addresses the critical weakness identified in DISC-003 v2, Section 5.2, point 3: "Only ERA-ERA pairs as distributable redexes." **(MUST)**

**R27.** The random net generator SHOULD be configurable to control the symbol distribution, enabling targeted tests (e.g., 50% CON, 50% DUP to maximize commutation). **(SHOULD)**

**R28.** The random net generator MUST handle potential non-termination: for tests using `reduce_all`, the generator SHOULD either filter out non-terminating nets or use `reduce_n(budget)` with a generous budget as a safety valve. Non-terminating nets (REF-002, Figure 3) MUST NOT cause test hangs. **(MUST for safety; SHOULD for the specific mechanism)**

### 3.13 CON-DUP Distributed Tests

**R29.** Relativist MUST have specific tests for nets exercising the CON-DUP commutation rule in distributed context. This directly addresses: AC-005 Gap L5 ("No CONDUP tests in grid"), AC-005 Gap L7 ("CON-DUP Expansion poorly tested"), and DISC-003 v2 Section 5.2 point 3 ("Rules like CON-DUP, which alter topology and create new agents, are the most challenging for the merge/remap protocol"). CON-DUP is the only rule that increases the agent count (+2 per interaction) and is the fundamental mechanism of work expansion for grid computing. **(MUST)**

| ID | Test | What it verifies |
|----|------|------------------|
| CD1 | `test_condup_creates_new_ids_in_range` | Agents created by CON-DUP stay within the worker's `id_range`. SPEC-04 R16-R18, SPEC-01 D4. |
| CD2 | `test_condup_no_id_collision_after_merge` | 2 workers execute CON-DUP, merge has no ID collision. SPEC-01 D4. |
| CD3 | `test_condup_border_emerging_resolved` | CON-DUP in worker A creates an agent that forms a redex with an agent in worker B. Emergent border redex resolved in the same or next round. SPEC-01 D3c, D3d. |
| CD4 | `test_condup_expansion_multiple_rounds` | CON-DUP net requiring multiple rounds. Final result isomorphic to sequential. SPEC-01 G1. |

### 3.14 Edge Cases

**R30.** Relativist MUST have tests for known edge cases. **(MUST)**

| ID | Test | What it verifies |
|----|------|------------------|
| E1 | `test_empty_net_all_operations` | All operations on an empty net: split, reduce_all, merge, serialize. No panics. |
| E2 | `test_single_agent_no_redex` | 1 agent with no active pair. Already in normal form. |
| E3 | `test_all_redexes_on_border` | All redexes are border redexes (0 local work). Pipeline converges via border resolution. |
| E4 | `test_all_redexes_internal` | 0 border redexes. Pipeline converges in 1 round. |
| E5 | `test_self_loop_era` | 2 ERA connected principal-to-principal (self-contained). Annihilate normally. |
| E6 | `test_partition_1_worker` | n=1: net returned intact, no borders. SPEC-04 R2. |
| E7 | `test_partition_more_workers_than_agents` | n > agents: excess partitions are empty. SPEC-04 R3. |
| E8 | `test_net_with_preexisting_freeports` | Net already contains FreePort (Lafont) from prior context. New partitioning does not collide. SPEC-04 R15. |
| E9 | `test_large_net_performance` | Run `reduce_all` on nets with 1,000 and 10,000 agents. Verify that `time(10k) / time(1k) < 15` (50% margin over the expected 10x linear scaling). Marked `#[ignore]` since it is a performance test, not a correctness test. SPEC-03 R22. |

### 3.15 End-to-End Tests (Docker/TCP)

**R31.** Relativist MUST have end-to-end tests that exercise the full distributed pipeline: coordinator + real TCP workers. These tests MUST be marked `#[ignore]` and executed separately. **(MUST)**

| ID | Test | What it verifies |
|----|------|------------------|
| E2E1 | `test_e2e_era_chain_2_workers_tcp` | EP-Annihilation (100 pairs), 2 workers via TCP localhost. `reduce_all(net) ~ run_grid(net, 2)`. SPEC-01 G1. |
| E2E2 | `test_e2e_treesum_4_workers_tcp` | TreeSum [1..16], 4 workers via TCP. Correct result. SPEC-01 G1. |
| E2E3 | `test_e2e_condup_2_workers_tcp` | CON-DUP Expansion, 2 workers via TCP. Result isomorphic to sequential. Profile B coverage. |
| E2E4 | `test_e2e_dual_tree_2_workers_tcp` | DualTree depth 4, 2 workers via TCP. Multiple rounds. Profile C coverage. |
| E2E5 | `test_e2e_docker_compose_4_workers` | Full Docker Compose deployment with 4 workers. Fundamental Property verified. SPEC-07 deploy target `docker-local`. |

**R32.** End-to-end tests SHOULD use `cargo test -- --ignored` and SHOULD be documented in the project README with setup instructions. **(SHOULD)**

### 3.16 Runtime Invariant Checker

**R33.** Relativist MUST implement a runtime invariant checker that verifies all applicable invariants from SPEC-01 when `#[cfg(debug_assertions)]` is active. **(MUST)**

```rust
impl Net {
    /// Verifies all system invariants.
    /// MUST be called after each reduce_step in debug mode (SPEC-03 R15).
    /// MUST be called after each merge in debug mode (SPEC-05 R11).
    /// In release mode, MAY be disabled for performance.
    ///
    /// Panics with a descriptive message if any invariant is violated.
    #[cfg(debug_assertions)]
    pub fn assert_all_invariants(&self) {
        self.assert_linearity();          // T1
        self.assert_bidirectional();      // I1
        self.assert_valid_references();   // I2
        self.assert_monotonic_ids();      // I3
        self.assert_valid_redex_queue();  // I4
    }

    /// T1: Every port of every live agent is connected to exactly one other port.
    #[cfg(debug_assertions)]
    fn assert_linearity(&self)

    /// I1: If ports[a] == b, then ports[b] == a.
    #[cfg(debug_assertions)]
    fn assert_bidirectional(&self)

    /// I2: Every AgentPort(id, p) reference: agents[id].is_some() and p <= arity.
    #[cfg(debug_assertions)]
    fn assert_valid_references(&self)

    /// I3: next_id > max(ids of live agents).
    #[cfg(debug_assertions)]
    fn assert_monotonic_ids(&self)

    /// I4: For each (a, b) in the queue, if both exist, they are connected via port 0.
    /// Stale redexes (removed agents) are permitted.
    #[cfg(debug_assertions)]
    fn assert_valid_redex_queue(&self)
}
```

**R34.** The individual invariant checks MUST have the following costs: **(MUST)**

| ID | Invariant | What it verifies | Cost |
|----|-----------|------------------|------|
| IV1 | T1 | Each port of each live agent connects to exactly one other port | O(A * 3) |
| IV2 | I1 | Bidirectionality: `ports[idx(a)] == b` implies `ports[idx(b)] == a` | O(A * 3) |
| IV3 | I2 | Every AgentPort reference points to an existing agent with valid port | O(A * 3) |
| IV4 | I3 | `next_id > max(id of live agents)` | O(A) |
| IV5 | I4 | Each `(a, b)` in redex queue: if both exist, connected via principal ports | O(\|queue\|) |
| IV6 | T3 | No AgentId appears in more than one active pair | O(\|redexes\|) |

**R35.** The invariant checker MUST be executed after each successful `reduce_step` when `#[cfg(debug_assertions)]` is active (per SPEC-02 R20, SPEC-03 R15). **(MUST)**

**R36.** In release mode, the invariant checker MAY be disabled for performance. **(MAY)**

### 3.17 Reduction Strategies for Confluence Testing

**R37.** To empirically validate strong confluence (SPEC-01 T4, equivalent to Premise P1 from ARG-001), Relativist MUST support at least 3 reduction strategies in test mode. **(MUST)**

```rust
/// Reduction strategy for selecting which redex to process next.
/// All strategies MUST produce the same Normal Form (SPEC-01 T4, T6).
/// All strategies MUST produce the same total interaction count (SPEC-01 T7).
pub enum ReductionStrategy {
    /// Reduce the first redex in the queue (FIFO). Default production strategy.
    First,
    /// Reduce the last redex in the queue (LIFO).
    Last,
    /// Reduce a random redex from the queue (deterministic seed).
    Random(u64),
}

/// Reduce the net using the given strategy.
/// Returns the total number of interactions.
pub fn reduce_all_with_strategy(net: &mut Net, strategy: ReductionStrategy) -> u64
```

**R38.** The `ReductionStrategy` enum and `reduce_all_with_strategy` function MAY be feature-gated behind `#[cfg(test)]` to avoid inclusion in production builds. **(MAY)**

> **Note:** The `ReductionStrategy` type and `reduce_all_with_strategy` function are defined here because they exist solely for testing purposes. However, they affect the reduction engine's API surface. This requirement SHOULD be reflected in SPEC-03 during its next revision, even if the implementation is `#[cfg(test)]`-gated.

### 3.18 Unit Tests -- Encoding (SPEC-14)

> **Namespace:** Tests defined in SPEC-14 use the `ENC-` prefix. The original SPEC-14 labels (ET-1 through ET-12) are mapped to `ENC-1` through `ENC-12` in this spec's global namespace.

**R39.** The encoding module MUST have unit tests covering Church numeral construction, roundtrip, arithmetic correctness, invariant preservation, and decode rejection. These tests are defined in SPEC-14 and incorporated here for unified tracking. **(MUST)**

| ID | SPEC-14 Label | Test | What it verifies |
|----|---------------|------|------------------|
| ENC-1 | ET-1 | `test_encode_nat_zero_structure` | `encode_nat(0)` produces exactly 2 CON + 1 ERA with correct topology |
| ENC-2 | ET-2 | `test_encode_nat_one_structure` | `encode_nat(1)` produces exactly 3 CON + 0 DUP + 0 ERA with correct topology |
| ENC-3 | ET-3 | `test_encode_nat_two_structure` | `encode_nat(2)` produces exactly 4 CON + 1 DUP with correct topology |
| ENC-4 | ET-4 | `test_encode_nat_normal_form` | For n in {0, 1, 2, 5, 10, 100}, `encode_nat(n)` produces a net with zero redexes |
| ENC-5 | ET-5 | `test_encode_decode_roundtrip` | For n in {0, 1, 2, 3, 5, 10, 50, 100}, `decode_nat(&encode_nat(n))` returns `Some(n)` |
| ENC-6 | ET-6 | `test_add_correctness` | For (a, b) in {(0,0), (0,1), (1,0), (1,1), (2,3), (10,20), (50,50), (100,100)}: `reduce_all(add(a, b))` yields `decode_nat == Some(a+b)` |
| ENC-7 | ET-7 | `test_mul_correctness` | For (a, b) in {(0,1), (1,0), (1,1), (2,3), (5,5), (10,10)}: `reduce_all(mul(a, b))` yields `decode_nat == Some(a*b)` |
| ENC-8 | ET-8 | `test_exp_correctness` | For (a, b) in {(2,0), (2,1), (2,3), (2,8), (3,3)}: `reduce_all(exp(a, b))` yields `decode_nat == Some(a^b)` |
| ENC-9 | ET-9 | `test_encoding_invariant_preservation` | All encoding and arithmetic nets satisfy T1-T7 via `assert_all_invariants()` |
| ENC-10 | ET-10 | `prop_add_commutative` | Property test: for random a, b in [0, 100], `add(a, b) ~ add(b, a)` after reduction |
| ENC-11 | ET-11 | `test_add_distributed_correctness` | For (a, b) = (50, 50) and k in {1, 2, 4}: `run_grid(add(50, 50), k) ~ reduce_all(add(50, 50))`. Fundamental Property for encoding. |
| ENC-12 | ET-12 | `test_decode_rejection` | `decode_nat` returns `None` for non-Church nets: `ep_annihilation(5)`, empty net, net with non-zero redexes |

### 3.19 Unit Tests -- Security (SPEC-10)

> **Namespace:** Tests defined in SPEC-10 use the `SEC-` prefix. The original SPEC-10 labels (T1 through T10) are mapped to `SEC-1` through `SEC-10`.

**R40.** The security module MUST have unit tests covering token generation, validation, TLS, message limits, and tier detection. Tests requiring the `tls` feature MUST be feature-gated. **(MUST)**

| ID | SPEC-10 Label | Test | What it verifies |
|----|---------------|------|------------------|
| SEC-1 | T1 | `test_token_generation_unique` | Two consecutive `AuthToken::generate()` calls produce different tokens |
| SEC-2 | T2 | `test_token_serialization_roundtrip` | `AuthToken::from_base64(token.to_base64()).unwrap()` verifies against original |
| SEC-3 | T3 | `test_token_validation` | Correct token passes `verify()`, wrong token fails, empty token fails |
| SEC-4 | T4 | `test_tls_handshake` | (Feature-gated: `--features tls`) TLS handshake succeeds with valid cert/key |
| SEC-5 | T5 | `test_tls_reject_invalid_cert` | (Feature-gated: `--features tls`) Connection rejected with invalid certificate |
| SEC-6 | T6 | `test_message_size_limit` | Frame with declared length exceeding `max_payload_size` rejected without reading payload |
| SEC-7 | T7 | `test_connection_idle_timeout` | Connection with no messages for longer than `idle_timeout` is closed |
| SEC-8 | T8 | `test_unauthorized_registration_rejected` | Worker without correct token in Tier 2/3 disconnected; coordinator continues accepting others |
| SEC-9 | T9 | `test_localhost_default_binding` | Coordinator started without `--bind` only reachable from `127.0.0.1` |
| SEC-10 | T10 | `test_tier_detection` | Correct `SecurityTier` detected for each combination of flags |
| SEC-11 | T10 | `test_token_debug_redacted` | `format!("{:?}", token)` contains `"[REDACTED]"`, not raw bytes |

### 3.20 Unit Tests -- Observability (SPEC-11)

> **Namespace:** Tests defined in SPEC-11 use the `OBS-` prefix. The original SPEC-11 labels (T1 through T9, T8a) are mapped to `OBS-1` through `OBS-10`.

**R41.** The observability module MUST have unit tests covering tracing initialization, log format, metrics endpoints, and feature-gating. Tests requiring the `metrics` feature MUST be feature-gated. **(MUST)**

| ID | SPEC-11 Label | Test | What it verifies |
|----|---------------|------|------------------|
| OBS-1 | T1 | `test_tracing_init_no_panic` | `init_tracing()` with default configuration does not panic |
| OBS-2 | T2 | `test_tracing_double_init_panics` | Calling `init_tracing()` a second time panics |
| OBS-3 | T3 | `test_json_log_format` | Captured output is valid JSON with `"level"`, `"target"`, `"fields"`, `"timestamp"` keys |
| OBS-4 | T4 | `test_rust_log_override` | `RUST_LOG=relativist::reduction=trace` causes TRACE events to appear |
| OBS-5 | T5 | `test_span_fields` | Reduction and merge spans contain expected fields (net_id, round, worker_id) |
| OBS-6 | T6 | `test_health_endpoint` | (Feature-gated: `--features metrics`) `GET /health` returns HTTP 200 with body `"ok"` |
| OBS-7 | T7 | `test_ready_endpoint_states` | (Feature-gated: `--features metrics`) `GET /ready` returns 503 in `Init`, 200 in working states, 503 in `Error` |
| OBS-8 | T8 | `test_metrics_endpoint` | (Feature-gated: `--features metrics`) `GET /metrics` returns parseable Prometheus format with `relativist_` prefix |
| OBS-9 | T8a | `test_metrics_content_type` | (Feature-gated: `--features metrics`) `GET /metrics` response has `Content-Type` containing `application/openmetrics-text` |
| OBS-10 | T9 | `test_metrics_feature_disabled` | Without `metrics` feature, HTTP server not started; no metrics code compiled |

### 3.21 Unit Tests -- User I/O (SPEC-12)

> **Namespace:** Tests defined in SPEC-12 use the `UIO-` prefix. The original SPEC-12 labels (T1 through T14) are mapped to `UIO-1` through `UIO-14`.

**R42.** The user I/O module MUST have unit tests covering binary and text format roundtrips, parser error handling, CLI subcommands (`reduce`, `inspect`), file format detection, and generator consistency. **(MUST)**

| ID | SPEC-12 Label | Test | What it verifies |
|----|---------------|------|------------------|
| UIO-1 | T1 | `test_binary_roundtrip` | For each generator, `deserialize(serialize(generate(n))) == generate(n)` for N in {1, 10, 100} |
| UIO-2 | T2 | `test_text_dsl_roundtrip` | For each generator, `parse_ic(format_ic(generate(n)))` produces structurally equivalent net for N in {1, 5, 10} |
| UIO-3 | T3 | `test_text_dsl_parser_errors` | Rejects: missing wire endpoint, unknown agent name, ERA with auxiliary port, duplicate port connection |
| UIO-4 | T4 | `test_generator_validity` | Each generator produces valid net (T1-T7 from SPEC-01) for N in {1, 10, 100, 1000} |
| UIO-5 | T5 | `test_inspect_correctness` | For `ep_annihilation(10)`: 20 agents, 10 redexes, 0 CON, 0 DUP, 20 ERA, 0 free ports, not normal form |
| UIO-6 | T6 | `test_reduce_correctness` | For `ep_annihilation(10)`: output has 0 agents, 0 redexes (Normal Form) |
| UIO-7 | T7 | `test_reduce_max_interactions` | `reduce` with `--max-interactions 5` on `ep_annihilation(10)`: stops early, output NOT in Normal Form |
| UIO-8 | T8 | `test_file_format_detection` | `.bin` -> bincode, `.ic` -> text DSL, `.json` -> JSON; `.xyz` -> `UnrecognizedFormat` error |
| UIO-9 | T9 | `test_generator_consistency` | Each `ExampleNet` variant matches corresponding `Benchmark::make_net` from SPEC-09 |
| UIO-10 | T10 | `test_generator_size_zero` | For each generator, `generate(0)` produces empty net; `reduce` yields 0 interactions |
| UIO-11 | T11 | `test_text_dsl_root_declaration` | (a) two `root` declarations -> error; (b) no `root` -> `root == None`; (c) `root free(0)` -> `Some(FreePort(0))` |
| UIO-12 | T12 | `test_text_dsl_self_loop` | `wire a.left a.left` produces parse error |
| UIO-13 | T13 | `test_text_dsl_free_to_free` | `wire free(0) free(1)` produces parse error |
| UIO-14 | T14 | `test_empty_net_io` | `inspect` on empty net: 0 agents, 0 wires, 0 redexes, normal_form: true. `reduce` completes with 0 interactions. |

### 3.22 Unit Tests -- System Architecture / FSM (SPEC-13)

> **Namespace:** Tests for SPEC-13 architectural requirements use the `ARCH-` prefix.

**R43.** The coordinator and worker FSM modules MUST have unit tests verifying state transitions, the stimulus-response pattern, and error handling. These tests validate the pure transition function without requiring an async runtime. **(MUST)**

| ID | Test | SPEC-13 Req | What it verifies |
|----|------|-------------|------------------|
| ARCH-1 | `test_coordinator_fsm_init_to_waiting` | R21 | Transition from `Init` to `WaitingForWorkers` on startup event |
| ARCH-2 | `test_coordinator_fsm_all_workers_connected` | R21 | Transition from `WaitingForWorkers` to `Partitioning` when all workers connect |
| ARCH-3 | `test_coordinator_fsm_partitioning_to_collecting` | R21 | `SplitComplete` event triggers `Partitioning -> Collecting` with `DistributePartitions` action |
| ARCH-4 | `test_coordinator_fsm_collecting_to_merging` | R21 | All `PartitionResult` events received triggers `Collecting -> Merging` |
| ARCH-5 | `test_coordinator_fsm_merge_to_check_termination` | R21 | `MergeComplete` event triggers `Merging -> CheckTermination` |
| ARCH-6 | `test_coordinator_fsm_check_termination_done` | R21 | `is_normal_form == true` triggers `CheckTermination -> Done` with `WriteOutput, ShutdownAll` actions |
| ARCH-7 | `test_coordinator_fsm_check_termination_loop` | R21 | `is_normal_form == false` triggers `CheckTermination -> Partitioning` with `InvokeSplit` action |
| ARCH-8 | `test_coordinator_fsm_phase_timeout` | R21 | `PhaseTimeout(id)` in `Collecting` triggers `Error` state |
| ARCH-9 | `test_worker_fsm_init_to_idle` | R25 | Worker transitions from `Init` to `Idle` upon connection |
| ARCH-10 | `test_worker_fsm_receive_partition` | R25 | Worker transitions from `Idle` to `Reducing` upon receiving partition |
| ARCH-11 | `test_worker_fsm_reduce_complete` | R25 | Worker transitions from `Reducing` to `Idle` after sending `PartitionResult` |
| ARCH-12 | `test_worker_fsm_connection_lost` | R25 | `ConnectionLost` in any state triggers `Error` with `ShutdownSelf` action (no reconnect) |
| ARCH-13 | `test_coordinator_fsm_pure_function` | R20 | Transition function is a pure function: same (state, event) always produces same (new_state, actions) |

### 3.23 Dedicated D2 Test (SPEC-01)

**R44.** Relativist SHOULD have a dedicated test that verifies SPEC-01 D2 (Local Reduction Equivalence) in isolation, comparing single-step reduction in a partition against single-step reduction in the global net. **(SHOULD)**

| ID | Test | What it verifies |
|----|------|------------------|
| D2-1 | `test_d2_local_reduction_equivalence` | Construct a net with a known redex. Reduce the redex globally and record the topological delta. Partition the net so the redex is internal. Reduce the redex in the partition. Compare the topological delta: both must be identical (same agents removed, same agents created, same connections made). Directly tests D2 without pipeline noise. |

---

## 4. Design

### 4.1 Directory Structure

```
codigo/relativist/
  src/
    net/
      mod.rs
      tests.rs          # Unit tests N1-N18
    reduction/
      mod.rs
      tests.rs          # Unit tests RE1-RE21
    partition/
      mod.rs
      tests.rs          # Unit tests P1-P12, PS1-PS5
    merge/
      mod.rs
      tests.rs          # Unit tests M1-M7, FI1-FI3
    grid/
      mod.rs
      tests.rs          # Unit tests GL1-GL5
    protocol/
      mod.rs
      tests.rs          # Unit tests WP1-WP3
    cli/
      mod.rs
      tests.rs          # Unit tests DP1-DP5, UIO-1 through UIO-14
    encoding/
      mod.rs
      tests.rs          # Unit tests ENC-1 through ENC-12
    coordinator/
      mod.rs
      tests.rs          # Unit tests ARCH-1 through ARCH-8, ARCH-13
    worker/
      mod.rs
      tests.rs          # Unit tests ARCH-9 through ARCH-12
    config/
      mod.rs
      tests.rs          # Config parsing tests (covered by DP1-DP5 and UIO-8)
    observability/
      mod.rs
      tests.rs          # Unit tests OBS-1 through OBS-10
    security/
      mod.rs
      tests.rs          # Unit tests SEC-1 through SEC-11
  tests/
    common/
      mod.rs            # Shared fixtures, helpers, generators
      fixtures.rs       # Pre-built test nets
      generators.rs     # proptest random net generators (arb_net, arb_net_weighted, arb_condup_net, arb_chain_net)
      isomorphism.rs    # nets_isomorphic function
      strategies.rs     # ReductionStrategy + reduce_all_with_strategy
    integration/
      pipeline.rs       # Tests INT1-INT11
      fundamental.rs    # Tests F1-F5
      edge_cases.rs     # Tests E1-E9
      condup.rs         # Tests CD1-CD4
      encoding.rs       # Tests ENC-6 through ENC-8 (arithmetic correctness)
      encoding_distributed.rs  # Test ENC-11 (distributed correctness, Fundamental Property)
      d2_equivalence.rs # Test D2-1 (dedicated D2 verification)
    property/
      invariants.rs     # Tests PB1-PB3, PB9, PB13, PB14
      partition.rs      # Tests PB4, PB10-PB12
      distributed.rs    # Tests PB5-PB7, PB15, PB16
      serialization.rs  # Test PB8
      encoding.rs       # Test ENC-10 (proptest for arithmetic)
    e2e/
      tcp_pipeline.rs   # Tests E2E1-E2E4 (#[ignore])
      docker.rs         # Test E2E5 (#[ignore])
```

### 4.2 Test Fixtures

Pre-built nets reusable across tests. Each fixture is a function that returns a Net with known topology.

```rust
/// Module of shared test fixtures.
/// Each fixture is a pure function returning a Net with known topology.
pub mod fixtures {
    use crate::{Net, Symbol};

    /// Empty net. 0 agents, 0 wires.
    pub fn empty_net() -> Net

    /// N pairs of ERA-ERA connected via principal ports.
    /// N independent redexes. Normal form: 0 agents.
    /// Profile A (Embarrassingly Parallel).
    pub fn era_pairs(n: usize) -> Net

    /// N pairs of CON-CON connected via principal ports.
    /// Auxiliary ports connected to FreePorts.
    /// Normal form: 0 CON agents, FreePorts cross-reconnected.
    pub fn con_pairs(n: usize) -> Net

    /// N pairs of DUP-DUP connected via principal ports.
    /// Auxiliary ports connected to FreePorts.
    /// Normal form: 0 DUP agents, FreePorts parallel-reconnected.
    pub fn dup_pairs(n: usize) -> Net

    /// N pairs of CON-DUP connected via principal ports.
    /// Auxiliary ports connected to FreePorts.
    /// Commutation rule: expands before collapsing.
    /// Profile B (Expansion with Collapse).
    pub fn condup_pairs(n: usize) -> Net

    /// TreeSum: tree of CON agents encoding addition.
    /// Equivalent to `mkTree` from the Haskell prototype (AC-004).
    /// Profile A.
    pub fn tree_sum(values: &[usize]) -> Net

    /// Balanced TreeSum: same as tree_sum but with balanced tree structure.
    /// Equivalent to `mkTreeBalanced` from the Haskell prototype (AC-004).
    pub fn tree_sum_balanced(values: &[usize]) -> Net

    /// DualTree: two binary trees of CON connected at the roots.
    /// Equivalent to `mkDualTreeNet` from the Haskell prototype (AC-005).
    /// Profile C (Sequential Dependency).
    pub fn dual_tree(depth: usize) -> Net

    /// Mixed net: CON + ERA + DUP + ERA. Exercises erasure and annihilation.
    /// Equivalent to `test_gridEqualsLocalMixed` from the prototype (AC-004).
    pub fn mixed_net() -> Net

    /// ERA chain: ERA><CON(ERA,ERA) -> cascade -> empty.
    /// Tests cascading erasure.
    pub fn era_chain() -> Net
}
```

### 4.3 Random Net Generator (proptest)

```rust
use proptest::prelude::*;

/// proptest strategy for generating valid IC nets.
///
/// Parameters:
/// - `max_agents`: maximum number of agents (1..max_agents)
///
/// Guarantees:
/// - Generated net satisfies T1 (linearity)
/// - Principal ports paired form valid redexes
/// - Unmatched ports connect to FreePorts
/// - Net is valid per SPEC-01 (I1, I2, I3)
pub fn arb_net(max_agents: usize) -> impl Strategy<Value = Net>

/// Variant with control over symbol distribution.
/// Allows targeted testing (e.g., heavy CON-DUP for commutation testing).
pub fn arb_net_weighted(
    max_agents: usize,
    con_weight: u32,
    dup_weight: u32,
    era_weight: u32,
) -> impl Strategy<Value = Net>
```

**Generation algorithm:**

```
1. Generate n = uniform(1, max_agents)
2. For each i in 0..n:
     agents[i] = Agent { symbol: weighted_choice(Con, Dup, Era), id: i }
3. Collect all ports: for each agent, generate its ports (0..=arity)
4. Shuffle the port list
5. Pair sequentially: ports[0] <-> ports[1], ports[2] <-> ports[3], ...
6. If one port remains (odd count), connect to FreePort(0)
7. Build Net with these agents and connections
8. Populate redex queue with pairs of connected principal ports
9. Verify: debug_assert!(net.assert_all_invariants())
```

**Targeted generators for workload profiles:**

```rust
/// Generates nets with predominantly CON-DUP pairs, guaranteeing Profile B behavior.
/// The generator creates `max_pairs` CON-DUP active pairs (CON principal port
/// connected to DUP principal port), with auxiliary ports connected to FreePorts
/// or to other agents to create cascading interactions.
pub fn arb_condup_net(max_pairs: usize) -> impl Strategy<Value = Net>

/// Generates tree or chain topologies with level-dependent reduction,
/// approximating Profile C (Sequential Dependency) behavior.
/// The generator creates binary trees of CON agents up to `max_depth`,
/// producing nets where reduction at one level enables reduction at the next.
pub fn arb_chain_net(max_depth: usize) -> impl Strategy<Value = Net>
```

**Non-termination safety:** The generator SHOULD favor nets biased toward termination. A practical approach:

- Increase the weight of ERA (which destroys agents) relative to DUP (which creates agents via CON-DUP).
- For tests using `reduce_all`, wrap in `reduce_n(budget)` with a configurable `MAX_PROPTEST_BUDGET` constant (default: 100,000). This is a practical ceiling, not a theoretical guarantee.
- If `reduce_n(budget)` does not reach Normal Form, the test SHOULD mark the input as inconclusive via `prop_assume!(false, "budget exceeded -- skipping non-terminating input")` rather than failing. This avoids flaky CI from false positives.
- For property tests, use `proptest::test_runner::Config { timeout: 10_000, .. }` as a separate safety net for cases where even dequeuing stale redexes is too slow.

### 4.4 Graph Isomorphism Checker

```rust
/// Verifies isomorphism between two nets.
///
/// Algorithm:
/// 1. If |agents(a)| != |agents(b)|, return false.
/// 2. Group agents by symbol. If per-symbol counts differ, return false.
/// 3. Attempt to build a bijection f: AgentId_a -> AgentId_b by backtracking:
///    a. For each agent in a (ordered by id), try to map to each
///       unmapped agent in b with the same symbol.
///    b. For each candidate, verify that connectivity is preserved:
///       for every port p of the agent, get_target(a, agent_a, p) mapped by f
///       must equal get_target(b, f(agent_a), p).
///    c. If consistent, proceed to the next agent. Otherwise, backtrack.
/// 4. If a complete bijection is found, return true.
///
/// Complexity: O(n! * k) worst case (n = agents, k = ports per agent).
/// In practice, for small nets (< 100 agents), sufficiently fast.
/// For larger nets, use heuristic filtering by degree and neighborhood.
fn nets_isomorphic(a: &Net, b: &Net) -> bool
```

**Performance target:** The isomorphism checker SHOULD use canonical form computation (e.g., Weisfeiler-Leman refinement followed by canonical ordering) or a polynomial-time heuristic that handles nets up to 500 agents within 100ms. The backtracking fallback MAY be used for nets where the heuristic is inconclusive. **(SHOULD)**

**Note for large nets:** For integration tests with large nets (> 50 agents), full isomorphism may be expensive. In those cases, a weaker verification (agent count by symbol + total wire count + normal form check) MAY be accepted as an approximation, provided the test documents the limitation. **(MAY)**

### 4.5 Invariant Coverage Matrix

This matrix maps every MUST invariant from SPEC-01 to the tests that verify it:

| SPEC-01 Invariant | Direct Tests | Property Tests | Integration Tests |
|--------------------|-------------|----------------|-------------------|
| T1 (Linearity) | IV1 | PB1, PB9 | All INT1-INT11 (via invariant checker) |
| T2 (Principal port interaction) | N9, N10, RE1-RE6 | PB13 | --- |
| T3 (Disjoint active pairs) | IV6 | PB14 | --- |
| T4 (Strong confluence) | --- | PB2 | F1-F5 |
| T5 (Rule correctness) | RE1-RE6, RE10 | PB7 | --- |
| T6 (Unique normal form) | --- | PB2 | F1-F4 |
| T7 (Invariant interaction count) | --- | PB3 | F5 |
| D1 (Split/merge identity) | P4, P5, M1-M7 | PB4, PB10 | --- |
| D2 (Local reduction equivalence) | D2-1 | PB10 | INT1-INT11 |
| D3 (Border redex completeness) | M5, CD3 | PB11 | INT9, INT11 |
| D4 (ID uniqueness) | P10, P11, CD1, CD2 | PB12 | --- |
| D5 (Exclusive ownership) | P1, P2 | PB4 | --- |
| D6 (Protocol termination) | GL1-GL3 | --- | INT9 |
| I1 (Bidirectional port array) | N8, IV2 | PB1 | All INT1-INT11 (via checker) |
| I2 (Reference validity) | IV3 | PB9 | All INT1-INT11 (via checker) |
| I3 (ID monotonicity) | N5, N7, IV4 | --- | --- |
| I4 (Redex queue validity) | N15, RE12, IV5 | --- | --- |
| I5 (Termination of reduce_all) | RE13, RE17, GL1, E9 | --- | --- |
| G1 (Fundamental Property) | --- | PB5, PB15, PB16 | F1-F4, E2E1-E2E5, ENC-11 |

### 4.6 Workload Profile Test Coverage

This matrix ensures all three workload profiles from ARG-004 are adequately tested:

| Profile | Description | Fixture | Integration | Fundamental | Property | E2E |
|---------|-------------|---------|-------------|-------------|----------|-----|
| A (EP) | Independent redexes, 1 round, 0 borders | `era_pairs`, `tree_sum` | INT1, INT3, INT4, INT5 | F1, F2 | PB5 | E2E1, E2E2 |
| B (Expansion) | CON-DUP, multiple rounds, emergent borders | `condup_pairs` | INT8, INT9 | F4 | PB5, PB6, PB15 | E2E3 |
| C (Sequential) | Level-dependent, many rounds, massive borders | `dual_tree` | INT6, INT7 | F3 | PB5, PB16 | E2E4 |

---

## 5. Rationale

### 5.1 Why graph isomorphism instead of agent counting

The most critical weakness identified in DISC-003 v2 (Section 5.2, point 5) is that the Haskell prototype verifies correctness only by agent counting (`countAgents`) or by a scalar metric (`extractResult`). Two nets with the same agent count but different topologies would be considered "equal." Relativist elevates the standard to graph isomorphism, which is the semantically correct verification of equivalence. This directly strengthens the empirical evidence for the Fundamental Property (G1).

### 5.2 Why property-based testing

DISC-003 v2 (Section 5.3) explicitly recommends "QuickCheck to generate random IC nets and verify the property for thousands of cases." Relativist adopts `proptest` (the Rust equivalent of QuickCheck) to:
- Generate nets with topologies that no manual test covers
- Exercise all 6 interaction rules (not only ERA-ERA)
- Find edge cases automatically via `proptest`'s shrinking
- Increase the datapoint count from ~110 (Haskell prototype) to ~10,000+
- Provide reproducible failure cases via deterministic seeds

### 5.3 Why CON-DUP tests in distributed context

AC-005 Gap L5: "No CONDUP tests in grid (only sequential)." AC-005 Gap L7: "CON-DUP Expansion poorly tested." DISC-003 v2, Section 5.2, point 3: "Rules like CON-DUP, which alter topology and create new agents, are the most challenging for the merge/remap protocol." CON-DUP is the only rule that increases the agent count (+2 per interaction) and is the fundamental mechanism of work expansion in grid computing. It can generate emergent border redexes (SPEC-01 D3c, D3d), requiring multiple rounds for resolution. Testing it extensively in distributed context is essential for validating the protocol's completeness (P3, ARG-003).

### 5.4 Why runtime invariant checking after each reduce_step

SPEC-01 defines 22 invariants (T1-T7, D1-D6, I1-I5, G1), and SPEC-02 R20 requires debug assertions after each operation. A subtle bug in a reduction rule (e.g., cross instead of parallel in DUP-DUP) may not be detected by point tests if the net is small. Checking invariants after EACH reduction step is O(A*3) per step, which is acceptable in debug mode and catches bugs at their root -- before they propagate through multiple reduction steps and become undiagnosable.

### 5.5 Why multiple reduction strategies

Strong confluence (SPEC-01 T4, ARG-001 P1) guarantees that all reduction strategies produce the same result and the same interaction count. Testing with different strategies (First, Last, Random) is the most direct way to empirically validate this property. If any bug violates confluence, different strategies will expose divergent results. This is the primary mechanism for testing PB2 and PB3.

### 5.6 Why end-to-end tests with Docker

The Fundamental Property (G1) is the central claim of the TCC. Testing it only in local mode (in-process, no TCP) would leave the network layer untested. End-to-end tests with real TCP workers (and optionally Docker Compose) verify that serialization, framing, network transmission, and the coordinator/worker FSM (SPEC-06) do not corrupt the net. SPEC-07 defines Docker Compose as a SHOULD for reproducibility; E2E5 validates this deployment target.

### 5.7 Why four test tiers

The four tiers (unit, integration, property, e2e) serve complementary purposes:
- **Unit** tests isolate individual components and verify SPEC requirements in isolation.
- **Integration** tests verify the local pipeline end-to-end without network overhead.
- **Property** tests provide statistical confidence across thousands of random inputs.
- **E2E** tests validate the complete distributed system including the wire protocol.

A defect caught at a lower tier is cheaper to diagnose. A defect that escapes all tiers would require debugging across the full distributed stack.

---

## 6. Haskell Prototype Reference

### 6.1 Mapping of existing tests

The Haskell prototype has 4 test suites with approximately 46 tests total:

| Suite | Analysis | Tests | Relativist Mapping |
|-------|----------|-------|--------------------|
| CoreSpec.hs | AC-001 | 13 | RE1-RE6 (6 rules), RE13-RE17 (reduce_all/reduce_n), N1-N4 |
| PartitionSpec.hs | AC-002 | ~10 (estimated) | P1-P12, PS1-PS5 (broader coverage) |
| GridSpec.hs | AC-004 | 3 | INT1-INT3 (pipeline with isomorphism) |
| TreeMapReduceSpec.hs | AC-004 | 3 | INT4-INT5 (tree sum with extract_result) |
| BenchmarkSpec.hs | AC-005 | 27 | F1-F4 (fundamental), INT6-INT8 (integration) |

### 6.2 What Relativist changes

1. **Isomorphism instead of counting:** The prototype uses `countAgents result == 0` or `extractResult == N`. Relativist uses `nets_isomorphic` for semantically complete verification.

2. **Property-based testing:** The prototype has none. Relativist adds `proptest` with random net generators producing 16 property tests (PB1-PB16), including targeted generators for Profile B (`arb_condup_net`) and Profile C (`arb_chain_net`).

3. **Integrated invariant checker:** The prototype does not verify invariants after each step. Relativist verifies T1, I1, I2, I3, I4, T3 after each `reduce_step` in debug mode.

4. **CON-DUP distributed coverage:** The prototype only tests CON-DUP sequentially. Relativist has tests CD1-CD4 and INT8 for CON-DUP in grid context.

5. **All 6 rules in property tests:** The prototype's main benchmark focuses on ERA-ERA. Relativist generates random nets with all 3 symbols, exercising all 6 rules.

6. **Multiple reduction strategies:** The prototype always reduces the first redex. Relativist tests First/Last/Random to validate confluence (PB2, PB3).

7. **End-to-end tests with TCP:** The prototype's grid tests use in-process simulation. Relativist adds E2E1-E2E5 with real TCP connections and Docker.

8. **Three workload profiles:** The prototype focuses on Profile A (EP-Annihilation). Relativist explicitly covers Profile B (CON-DUP Expansion) and Profile C (DualTree), as classified by ARG-004.

9. **Merge-specific tests:** The prototype does not isolate merge testing. Relativist adds M1-M7 and FI1-FI3 for the merge module specifically.

10. **FreePort index reconstruction tests:** New in Relativist (FI1-FI3), covering the three scenarios from SPEC-05 R22 that the prototype handles implicitly.

### 6.3 Gap coverage

| Haskell Gap | Source | Relativist Tests |
|-------------|--------|------------------|
| No direct `reconnect` test | AC-001 L1 | N8, RE1-RE6 |
| No `nextAgentId` after removal test | AC-001 L2 | N7 |
| No direct `portNeighbor` test | AC-001 L3 | N12 |
| No confluence with different order test | AC-001 L4 | PB2, PB3 |
| No large nets (>10 agents) | AC-001 L5 | F1-F4, PB1-PB16, E9 |
| No e2e benchmark tests | AC-005 L1 | F1-F5, INT1-INT11, E2E1-E2E5 |
| No CONDUP in grid | AC-005 L5 | INT8, F4, CD1-CD4, PB6, PB15, E2E3 |
| CON-DUP Expansion poorly tested | AC-005 L7 | CD1-CD4, INT8, F4, PB6, PB15 |

---

## 7. Open Questions

1. **Scalable isomorphism:** For nets with >100 agents, the backtracking isomorphism algorithm may be slow. Alternatives include canonical form (Weisfeiler-Leman refinement + canonical ordering) or graph hashing. The SHOULD requirement for a polynomial-time heuristic handling up to 500 agents within 100ms (Section 4.4) provides a performance target, but the specific algorithm can be decided during implementation. **(Does NOT block implementation; heuristic acceptable for large nets.)**

2. ~~**Non-terminating net generation:**~~ **RESOLVED.** The `arb_net` generator uses a configurable `MAX_PROPTEST_BUDGET` constant (default: 100,000) with `prop_assume!` to skip inconclusive inputs rather than failing. See Section 4.3.

3. **E2E test infrastructure:** The exact Docker Compose configuration for E2E5 will be defined by SPEC-07's Docker deployment section. The test assumes the infrastructure exists. If Docker is unavailable, E2E5 is skipped via `#[ignore]`. **(Does NOT block implementation.)**

4. **Rayon for local mode parallelism:** SPEC-07 mentions `rayon` as a SHOULD for parallel local mode. If adopted, integration tests should also run with rayon enabled to catch concurrency bugs. The decision can be deferred. **(Does NOT block implementation.)**
