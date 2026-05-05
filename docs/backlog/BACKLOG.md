# Relativist Implementation Backlog

**Last updated:** 2026-05-05 (D-012 Stage 1 TASK-SPLITTER landed: 4 instrumentation-restore tasks TASK-0615..0618)
**Total tasks:** 328 (158 done, 0 in progress, 169 todo, 1 obsoleted)
**SPEC-20 split:** 36 atomic tasks (TASK-0410..TASK-0455 with intentional gaps) covering R0a..R39, NF-001..NF-011 closures, and §3.8 amendments A1..A8. ~6,400 LoC production + ~3,200 LoC tests.
**SPEC-22 split:** 36 new atomic tasks (TASK-0460..TASK-0500 with intentional gaps at 0470, 0479, 0485, 0494, 0499 — preserved for future expansion / spec polish) covering R1..R32 (incl. letter sub-clauses R6, R9a, R10a, R10b, R10c, R23, R27a, R30) and §3.8 amendments A1..A10 against 7 predecessor specs (SPEC-01, SPEC-02, SPEC-03, SPEC-04, SPEC-05, SPEC-18, SPEC-19). See SPEC-22 section below. Estimated total ~2,070 LoC production + ~1,830 LoC tests.
**D-012 split:** 4 atomic tasks TASK-0615..0618 covering RF-04/05/07 from D011 final baseline analysis + cargo test --release blocker; ~170 LoC production + ~210 LoC tests. Independent ordering: TASK-0617 (release-tests, P0, ~10 LoC) ships first; TASK-0615 (network metric, P0) and TASK-0616 (compute metric, P0) parallelisable; TASK-0618 (MIPS path-a-or-b, P2) decides at DEV time.

**Pipeline:** See `../WORKFLOWS.md` (§1 Development Pipeline) for the 6-stage SDD process.

---

## Phase 1: Core Types (SPEC-02)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0001 | Convert net module to directory structure | P0 | **DONE** | none | S |
| TASK-0002 | Define Symbol enum | P0 | **DONE** | 0001 | S |
| TASK-0003 | Define AgentId and PortId type aliases | P0 | **DONE** | 0001 | S |
| TASK-0004 | Define PortRef enum | P0 | **DONE** | 0003 | S |
| TASK-0005 | Define Agent struct | P0 | **DONE** | 0002, 0003 | S |
| TASK-0006 | Define arity and total_ports functions | P0 | **DONE** | 0002 | S |
| TASK-0007 | Define PORTS_PER_SLOT, port_index, DISCONNECTED | P0 | **DONE** | 0003, 0004 | S |
| TASK-0008 | Define Net struct and constructors | P0 | **DONE** | 0004, 0005, 0007 | M |
| TASK-0009 | Implement create_agent | P0 | **DONE** | 0008, 0006 | S |
| TASK-0010 | Implement get_target and set_port helpers | P0 | **DONE** | 0008, 0007 | S |
| TASK-0011 | Implement connect | P0 | **DONE** | 0010 | S |
| TASK-0012 | Implement disconnect | P0 | **DONE** | 0010 | S |
| TASK-0013 | Implement remove_agent | P0 | **DONE** | 0009, 0012, 0006 | S |
| TASK-0014 | Implement is_reduced and is_valid_redex | P0 | **DONE** | 0010, 0008, 0019 | S |
| TASK-0015 | Implement debug assertions (I1, I2, I3, I6, I7) | P0 | **DONE** | 0010, 0009, 0006 | M |
| TASK-0016 | Define BorderMap type alias | P1 | **DONE** | 0004 | S |
| TASK-0017 | Add serde + bincode serialization | P1 | **DONE** | 0008 | S |
| TASK-0018 | Verify PartialEq and Eq for Net | P1 | **DONE** | 0008 | S |
| TASK-0019 | Implement get_agent and get_agent_mut accessors | P0 | **DONE** | 0008 | S |
| TASK-0231 | Implement count_live_agents and live_agents on Net | P0 | **DONE** | 0008 | S |

## Phase 2: Reduction Engine (SPEC-03)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0020 | Scaffold reduction module structure | P0 | **DONE** | Phase 1 | S |
| TASK-0021 | Define Rule enum and dispatch table | P0 | **DONE** | 0020 | S |
| TASK-0022 | Implement normalize_pair function | P0 | **DONE** | 0020, 0021 | S |
| TASK-0218 | Implement link helper (safe port reconnection) | P0 | **DONE** | 0020, Phase 1 | S |
| TASK-0023 | Implement interact_void (ERA-ERA) | P0 | **DONE** | 0020, Phase 1 | S |
| TASK-0024 | Implement interact_anni (CON-CON, DUP-DUP) | P0 | **DONE** | 0020, 0218, Phase 1 | M |
| TASK-0025 | Implement interact_eras (CON-ERA, DUP-ERA) | P0 | **DONE** | 0020, 0218, Phase 1 | S |
| TASK-0026 | Implement interact_comm (CON-DUP) | P0 | **DONE** | 0020, 0218, Phase 1 | M |
| TASK-0027 | Define StepResult and implement reduce_step | P0 | **DONE** | 0020-0026, 0218 | M |
| TASK-0028 | Define ReductionStats and implement reduce_all | P0 | **DONE** | 0027 | M |
| TASK-0029 | Implement reduce_n (budget-limited reduction) | P0 | **DONE** | 0027, 0028 | S |
| TASK-0030 | Wire up reduction module re-exports | P1 | **DONE** | 0020-0029, 0218 | S |
| TASK-0232 | Fix self-loop annihilation in interact_anni | P0 | **DONE** | 0024 | S |

## Phase 3: Partitioning (SPEC-04)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0040 | Define WorkerId type and IdRange struct | P0 | **DONE** | none | S |
| TASK-0041 | Define Partition struct | P0 | **DONE** | 0040 | S |
| TASK-0042 | Define PartitionPlan struct | P0 | **DONE** | 0041 | S |
| TASK-0043 | Define PartitionStrategy trait | P0 | **DONE** | 0040 | S |
| TASK-0044 | Implement ContiguousIdStrategy | P0 | **DONE** | 0043 | M |
| TASK-0045 | Helper function max_freeport_id | P0 | **DONE** | none | S |
| TASK-0046 | Wire classification logic | P0 | **DONE** | 0045 | M |
| TASK-0047 | Compute static ID space ranges | P0 | **DONE** | 0040 | S |
| TASK-0048 | split() trivial case (n=1) | P0 | **DONE** | 0042 | S |
| TASK-0049 | split() general case orchestrator | P0 | **DONE** | 0048, 0044, 0046, 0047, 0050-0054, 0219, 0220 | M |
| TASK-0050 | Build sub-net for one partition | P0 | **DONE** | 0046 | M |
| TASK-0051 | Redex queue population for partitions | P0 | **DONE** | 0050 | S |
| TASK-0052 | FreePort index construction per partition | P0 | **DONE** | 0046 | S |
| TASK-0053 | Debug assertion for C1 | P1 | TODO | 0041 | S |
| TASK-0054 | Debug assertions for C2 and C3 | P1 | TODO | 0041, 0042 | M |
| TASK-0055 | FreePort index lazy reconstruction | P1 | TODO | 0041 | S |
| TASK-0056 | ID range exhaustion error handling | P2 | TODO | 0040 | S |
| TASK-0219 | Stale boundary FreePort precondition assertion | P1 | TODO | 0045 | S |
| TASK-0220 | Root port propagation during split (R28) | P0 | **DONE** | 0050 | S |

## Phase 4: Merge & Grid Cycle (SPEC-05)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0060 | Define GridMetrics struct | P0 | **DONE** | none | S |
| TASK-0061 | Define WorkerRoundStats struct (6-field, serde, MUST) | P0 | **DONE** | none | S |
| TASK-0062 | Define GridConfig struct | P0 | **DONE** | none | S |
| TASK-0063 | Implement rebuild_free_port_index | P0 | **DONE** | Phase 1 | M |
| TASK-0064 | Implement is_principal_pair helper | P0 | **DONE** | Phase 1 | S |
| TASK-0065 | Merge function - unite agents + internal connections | P0 | **DONE** | 0060, 0064 | M |
| TASK-0066 | Merge function - restore boundary connections | P0 | **DONE** | 0065, 0064 | M |
| TASK-0067 | Merge debug assertions | P1 | **DONE** | 0066 | S |
| TASK-0068 | Implement drain_stale_redexes | P0 | **DONE** | Phase 1 | S |
| TASK-0069 | run_grid skeleton with termination logic | P0 | **DONE** | 0060, 0062, 0068 | M |
| TASK-0070 | run_grid Phase 1 (split) + Phase 2 (local reduce) | P0 | **DONE** | 0069, 0063, 0061 | M |
| TASK-0071 | run_grid Phase 3 (merge + resolve borders + metrics) | P0 | **DONE** | 0070, 0066 | M |
| TASK-0072 | n==1 optimization in run_grid | P1 | **DONE** | 0069 | S |
| TASK-0073 | Implement count_live_agents helper | P0 | **DONE** | 0231 | S |
| TASK-0074 | Integration test: split/merge identity (D1) | P0 | **DONE** | 0066 | M |
| TASK-0075 | Integration test: Fundamental Property G1 | P0 | **DONE** | 0071 | M |
| TASK-0076 | Merge module exports and wiring | P0 | **DONE** | 0060-0065 | S |
| TASK-0230 | Implement verify_no_redexes_full_scan (R41) | P1 | **DONE** | 0014, 0064, Phase 1 | S |

## Phase 5: Wire Protocol (SPEC-06)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0080 | Convert protocol module to directory structure | P0 | **DONE** | none | S |
| TASK-0081 | Define ProtocolError enum | P0 | **DONE** | 0080 | S |
| TASK-0082 | Define Message enum | P0 | **DONE** | 0080 | S |
| TASK-0083 | Define FrameHeader struct and framing constants | P0 | **DONE** | 0080 | S |
| TASK-0084 | Define NodeConfig type | P0 | **DONE** | 0080, 0083 | S |
| TASK-0085 | Implement send_frame function | P0 | **DONE** | 0081, 0082, 0083 | M |
| TASK-0086 | Implement recv_frame function | P0 | **DONE** | 0081, 0082, 0083 | M |
| TASK-0087 | Implement connect_with_retry (exponential backoff) | P1 | **DONE** | 0081, 0084 | S |
| TASK-0088 | Implement coordinator worker-accept phase | P0 | **DONE** | 0081, 0084 | M |
| TASK-0089 | Implement coordinator distribute phase | P0 | **DONE** | 0085, 0082, 0084 | M |
| TASK-0090 | Implement coordinator collect phase | P0 | **DONE** | 0086, 0082, 0084 | M |
| TASK-0091 | Implement coordinator shutdown protocol | P1 | **DONE** | 0085, 0082 | S |
| TASK-0092 | Implement run_coordinator (distributed grid loop) | P0 | **DONE** | 0088-0091, 0094 | M |
| TASK-0093 | Implement run_worker (worker loop) | P0 | **DONE** | 0085-0087, 0082 | M |
| TASK-0094 | Implement GridMetrics network extensions | P1 | **DONE** | 0080 | S |
| TASK-0095 | Implement in-memory transport for testing | P1 | **DONE** | 0085, 0086 | S |
| TASK-0096 | Add protocol crate dependencies to Cargo.toml | P0 | **DONE** | none | S |
| TASK-0212 | Implement SerializingChannelTransport | P1 | TODO | 0095, 0085, 0086 | S |

## Phase 6: CLI & Config (SPEC-07 Revised v3 + SPEC-13)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0100 | Refactor CLI to use Args structs (SPEC-07 Section 4.1, Revised v3) | P0 | **DONE** | Phase 5 | M |
| TASK-0101 | Initialize tracing subscriber in main | P0 | **DONE** | 0100 | S |
| TASK-0102 | Implement CLI-to-config mapping functions | P0 | **DONE** | 0100, 0062, 0084 | M |
| TASK-0103 | Define RelativistError top-level error type | P0 | **DONE** | Phase 5, 0081 | M |
| TASK-0104 | Implement net serialization/deserialization helpers | P0 | **DONE** | 0017, 0103 | S |
| TASK-0105 | Implement metrics output (JSON and CSV) | P1 | **DONE** | 0060, 0103 | M |
| TASK-0106 | Implement print_summary function | P1 | **DONE** | 0060, 0073 | S |
| TASK-0107 | Define CoordinatorState enum and FSM types | P0 | **DONE** | Phase 5, 0103 | M |
| TASK-0108 | Implement coordinator FSM transition function | P0 | **DONE** | 0107 | M |
| TASK-0109 | Define WorkerState enum and FSM types | P0 | **DONE** | Phase 5, 0103 | S |
| TASK-0110 | Implement worker FSM transition function | P0 | **DONE** | 0109 | S |
| TASK-0111 | Implement run_local_command (local mode entry point) | P0 | **DONE** | 0100, 0102, 0104, 0105, 0106, 0069 | M |
| TASK-0112 | Implement run_coordinator_command (coordinator entry point) | P0 | **DONE** | 0100, 0102, 0104-0106, 0107, 0108, 0092 | M |
| TASK-0113 | Implement run_worker_command (worker entry point) | P0 | **DONE** | 0100, 0102, 0109, 0110, 0093 | S |
| TASK-0114 | Implement run_generate_command (workload generator entry point) | P1 | **DONE** | 0100, 0104, 0103 | S |
| TASK-0115 | Align Cargo.toml with SPEC-13 dependency map | P1 | **DONE** | Phase 5 | S |
| TASK-0116 | Wire main.rs entrypoint with tokio and exit codes | P0 | **DONE** | 0100, 0101, 0103, 0111-0114 | S |
| TASK-0117 | Enforce Core/Infrastructure layer boundary | P1 | TODO | 0107, 0109, Phase 5 | S |
| TASK-0118 | Feature-gated module stubs for tls, metrics, otel | P2 | TODO | 0115 | S |
| TASK-0119 | Integration test: CLI end-to-end (local mode round-trip) | P1 | **DONE** | 0111, 0114, 0116 | M |

## Phase 7: Security (SPEC-10)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0120 | Convert security module to directory structure | P0 | **DONE** | none | S |
| TASK-0121 | Define TokenError and SecurityError enums | P0 | **DONE** | 0120 | S |
| TASK-0122 | Define SecurityTier enum and tier detection logic | P0 | **DONE** | 0120, 0121 | S |
| TASK-0123 | Define AuthToken struct with generation and serialization | P0 | **DONE** | 0120, 0121 | M |
| TASK-0124 | Implement AuthToken constant-time verification | P0 | **DONE** | 0123 | S |
| TASK-0125 | Define SecurityConfig struct | P0 | **DONE** | 0122, 0123 | S |
| TASK-0126 | Implement token file write | P1 | **DONE** | 0123 | S |
| TASK-0127 | Extend Message enum with Register, RegisterAck, RegisterNack | P0 | **DONE** | 0082, 0123 | S |
| TASK-0128 | Implement token validation in coordinator accept flow | P0 | **DONE** | 0124, 0125, 0127, 0088 | M |
| TASK-0129 | Implement network binding security | P1 | **DONE** | 0122, 0125 | S |
| TASK-0130 | Define TlsServerConfig (feature-gated) | P1 | **DONE** | 0120, 0121 | M |
| TASK-0131 | Define TlsClientConfig (feature-gated) | P1 | **DONE** | 0120, 0121 | M |
| TASK-0132 | Implement TLS handshake integration for coordinator | P1 | TODO | 0130, 0088 | M |
| TASK-0133 | Implement TLS handshake integration for worker | P1 | TODO | 0131, 0093 | M |
| TASK-0134 | Implement connection limits | P2 | TODO | 0125, 0088 | S |
| TASK-0135 | Implement idle connection timeout | P2 | TODO | 0125, 0088 | S |
| TASK-0136 | Verify message size pre-validation in recv_frame | P0 | **DONE** | 0086 | S |
| TASK-0137 | Add security crate dependencies to Cargo.toml | P0 | **DONE** | none | S |
| TASK-0138 | Implement SecurityConfig builder from CLI flags | P0 | **DONE** | 0122, 0123, 0125, 0126, 0130, 0131 | M |
| TASK-0139 | Security integration tests | P1 | TODO | 0128, 0129, 0132, 0133, 0136, 0138 | L |

## Phase 8: Observability (SPEC-11)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0140 | Convert observability module to directory structure and add dependencies | P0 | **DONE** | none | S |
| TASK-0141 | Define LogFormat and ProcessRole enums | P0 | **DONE** | 0140 | S |
| TASK-0142 | Define ObservabilityConfig struct | P0 | **DONE** | 0141 | S |
| TASK-0143 | Implement default log filter string | P0 | **DONE** | 0140 | S |
| TASK-0144 | Implement init_tracing with fmt::Layer and EnvFilter | P0 | **DONE** | 0142, 0143 | M |
| TASK-0145 | Add #[instrument] to partition split() | P1 | TODO | 0144, Phase 3 | S |
| TASK-0146 | Add #[instrument] to reduction reduce_all() | P1 | TODO | 0144, Phase 2 | S |
| TASK-0147 | Add #[instrument] to merge merge() | P1 | TODO | 0144, Phase 4 | S |
| TASK-0148 | Add #[instrument] to coordinator dispatch() and protocol handle_message() | P1 | TODO | 0144, Phase 5 | S |
| TASK-0149 | Add FSM state transition logging | P1 | TODO | 0144, Phase 5 | M |
| TASK-0150 | Define CoordinatorMetrics struct and registration | P0 | **DONE** | 0140 | M |
| TASK-0151 | Define protocol metrics | P1 | TODO | 0150 | M |
| TASK-0152 | ~~Extend WorkerRoundStats with observability fields~~ | -- | OBSOLETED | 0061 | -- |
| TASK-0153 | Implement coordinator metric aggregation from worker reports | P1 | TODO | 0150, 0061 | M |
| TASK-0154 | Add axum dependency and scaffold metrics_router | P0 | **DONE** | 0140, 0150 | S |
| TASK-0155 | Implement /health and /ready endpoints | P0 | **DONE** | 0154 | S |
| TASK-0156 | Implement /metrics endpoint with Prometheus encoding | P0 | **DONE** | 0154, 0150 | S |
| TASK-0157 | Implement axum HTTP server spawn as background tokio task | P0 | **DONE** | 0154, 0155, 0156 | M |
| TASK-0158 | Add OTel dependencies and init_tracing OTel layer | P2 | TODO | 0144 | M |
| TASK-0159 | Optional trace context in wire protocol messages | P2 | TODO | 0158, Phase 5 | M |
| TASK-0213 | Implement ERROR-level logging requirements (R9a) | P1 | TODO | 0144, Phase 1-6 | M |
| TASK-0214 | Wire AtomicBool readiness flag in coordinator FSM (R22a) | P1 | TODO | 0108, 0154 | S |

## Phase 9: User I/O (SPEC-12)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0160 | Convert io module to directory structure | P0 | **DONE** | none | S |
| TASK-0161 | Define FileIoError, NetFormat, and InspectOutputFormat types | P0 | **DONE** | 0160 | S |
| TASK-0162 | Define NetSummary and ReductionSummary structs | P0 | **DONE** | 0160 | S |
| TASK-0163 | Implement binary format load/save | P0 | **DONE** | 0160, 0161 | S |
| TASK-0164 | Text DSL parser - lexing and declaration collection (Pass 1) | P0 | **DONE** | 0160, 0161 | M |
| TASK-0165 | Text DSL parser - net construction and validation (Pass 2) | P0 | **DONE** | 0164, Phase 1 | M |
| TASK-0166 | Text DSL serializer (format_ic) | P0 | **DONE** | 0160, 0164, Phase 1 | M |
| TASK-0167 | JSON format load/save | P2 | TODO | 0160, 0161 | S |
| TASK-0168 | Implement load_net/save_net dispatch with format detection | P0 | **DONE** | 0161, 0163, 0165, 0166, 0167 | S |
| TASK-0169 | Implement net_summary computation | P0 | **DONE** | 0162, Phase 1 | S |
| TASK-0170 | Implement reduction summary formatting | P0 | **DONE** | 0162, 0169 | S |
| TASK-0171 | Implement generator - ep_annihilation (ERA-ERA pairs) | P0 | **DONE** | 0160, Phase 1 | S |
| TASK-0172 | Implement generators - ep_annihilation_con and ep_annihilation_dup | P0 | **DONE** | 0171, Phase 1 | S |
| TASK-0173 | Implement generator - con_dup_expansion | P0 | **DONE** | 0171, Phase 1 | S |
| TASK-0174 | Implement generator - dual_tree | P0 | **DONE** | 0171, Phase 1 | M |
| TASK-0175 | Implement generator - mixed_rules | P0 | **DONE** | 0171, Phase 1 | M |
| TASK-0176 | Implement generators - tree_sum and tree_sum_balanced | P1 | TODO | 0171, 0174, Phase 1 | M |
| TASK-0177 | Implement generators - erasure_propagation and Church encodings | P1 | TODO | 0171, 0202, 0204, 0205, Phase 1 | M |
| TASK-0178 | Define CLI argument structs for I/O subcommands | P0 | **DONE** | 0161, 0162, 0171 | M |
| TASK-0179 | Integration tests for I/O roundtrips and generators | P1 | TODO | 0163-0177, Phase 2 | L |

## Phase 10: Benchmarks (SPEC-09)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0180 | Scaffold bench module structure | P0 | TODO | Phase 1, Phase 2 | S |
| TASK-0181 | Define BenchmarkId, Mode, and core enums | P0 | TODO | 0180 | S |
| TASK-0182 | Define Benchmark trait | P0 | TODO | 0181, Phase 1 | S |
| TASK-0183 | Define BenchmarkResult and metric structs | P0 | TODO | 0181 | M |
| TASK-0184 | Define BenchmarkSuiteConfig and AggregatedStats | P0 | TODO | 0181, 0183 | M |
| TASK-0185 | Implement graph isomorphism (nets_isomorphic) | P0 | TODO | Phase 1 | M |
| TASK-0186 | Implement statistical functions (mean, std, median) | P0 | TODO | 0180 | S |
| TASK-0187 | Implement memory measurement (get_peak_memory_bytes) | P1 | TODO | 0180 | S |
| TASK-0188 | Implement EP-Annihilation (ERA) benchmark | P0 | TODO | 0182, Phase 1, Phase 2 | S |
| TASK-0189 | Implement EP-Annihilation-CON benchmark | P0 | TODO | 0182, Phase 1 | S |
| TASK-0190 | Implement EP-Annihilation-DUP benchmark | P0 | TODO | 0182, Phase 1 | S |
| TASK-0191 | Implement CON-DUP Expansion benchmark | P0 | TODO | 0182, 0185, Phase 1, Phase 2 | S |
| TASK-0192 | Implement DualTree benchmark | P0 | TODO | 0182, Phase 1, Phase 2 | M |
| TASK-0193 | Implement TreeSum and TreeSumBalanced benchmarks | P0 | TODO | 0182, 0185, Phase 1, Phase 2 | M |
| TASK-0194 | Implement MixedNet benchmark | P0 | TODO | 0182, 0185, Phase 1, Phase 2 | M |
| TASK-0195 | Implement ErasurePropagation benchmark | P0 | TODO | 0182, 0185, Phase 1, Phase 2 | S |
| TASK-0196 | Implement CSV output (detail and summary writers) | P0 | TODO | 0183, 0184 | M |
| TASK-0197 | Implement derived metrics computation and aggregation | P0 | TODO | 0183, 0184, 0186 | M |
| TASK-0198 | Implement benchmark suite runner (run_benchmark_suite) | P0 | TODO | 0182-0197, Phase 2, Phase 4 | M |
| TASK-0199 | Implement CLI binary and Criterion micro-benchmarks | P1 | TODO | 0198, 0184 | M |
| TASK-0221 | Implement ChurchAdd benchmark (R17a MUST) | P0 | TODO | 0182, 0185, 0204, 0203, Phase 1, Phase 2 | S |
| TASK-0222 | Implement ChurchMul benchmark (R17b SHOULD) | P1 | TODO | 0182, 0185, 0205, 0203, Phase 1, Phase 2 | S |

## Phase 11: Encoding (SPEC-14)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0200 | Scaffold encoding module directory structure | P0 | TODO | Phase 1 | S |
| TASK-0201 | Implement encode_church_into (core Church numeral builder) | P0 | TODO | 0200, Phase 1 | M |
| TASK-0202 | Implement encode_nat (Church numeral wrapper) | P0 | TODO | 0201 | S |
| TASK-0203 | Implement decode_nat (Church numeral readback) | P0 | TODO | 0200, Phase 1 | M |
| TASK-0204 | Implement build_add (Church addition) | P0 | TODO | 0201, 0200 | M |
| TASK-0205 | Implement build_mul (Church multiplication) | P0 | TODO | 0201, 0200 | M |
| TASK-0206 | Implement build_exp (Church exponentiation) | P0 | TODO | 0201, 0200 | M |
| TASK-0207 | Implement encoding unit tests (ET-1 through ET-5, ET-9, ET-12) | P0 | TODO | 0201, 0202, 0203 | M |
| TASK-0208 | Implement arithmetic correctness tests (ET-6, ET-7, ET-8, ET-10) | P0 | TODO | 0204, 0205, 0206, 0203, Phase 2 | M |
| TASK-0209 | Implement distributed correctness test (ET-11) | P1 | TODO | 0204, 0203, Phase 4 | M |
| TASK-0210 | Implement compute CLI subcommand | P0 | TODO | 0200, 0204-0206, 0203, Phase 2, 0100 | M |
| TASK-0211 | Implement arithmetic benchmark scenarios (ARITH-*) | P1 | TODO | 0204-0206, 0203, 0182, Phase 2, Phase 4 | L |

## v2 Phase 12: Transport Abstraction (SPEC-17)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0300 | Add transport dependencies (socket2, async-trait) | P0 | **DONE** | none | S |
| TASK-0301 | Define TransportBackend enum and TransportConfig struct | P0 | **DONE** | 0300 | S |
| TASK-0302 | Add transport field to NodeConfig | P0 | **DONE** | 0301 | S |
| TASK-0303 | Define Transport trait and TransportStream type | P0 | **DONE** | 0300 | S |
| TASK-0304 | Implement TcpTransport with TCP tuning | P0 | **DONE** | 0300, 0301, 0303 | M |
| TASK-0305 | Implement UnixTransport (cfg(unix) only) | P0 | **DONE** | 0303 | S |
| TASK-0306 | Implement ChannelTransport | P0 | **DONE** | 0303 | S |
| TASK-0307 | Implement create_transport factory function | P0 | **DONE** | 0304, 0305, 0306 | S |
| TASK-0308 | Refactor coordinator.rs to use Transport trait | P0 | **DONE** | 0302, 0303, 0304, 0306, 0307 | M |
| TASK-0309 | Refactor worker.rs to use Transport trait | P0 | **DONE** | 0302, 0303, 0307 | S |
| TASK-0310 | Add CLI transport flags | P0 | **DONE** | 0301, 0302 | S |
| TASK-0311 | Same-host detection heuristic and integration wiring | P1 | **DONE** | 0308, 0309, 0310 | S |

## v2 Phase 13: Cargo Workspace Restructure (SPEC-26 §3.1, Layer 0)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0320 | Create workspace skeleton (root Cargo.toml, relativist-core/, relativist-cli/) — R1, R2 | P0 | **DONE** | none | S |
| TASK-0321 | Move src/, tests/, benches/ to relativist-core/ via git mv — R3, R4, R6 | P0 | **DONE** | 0320 | S |
| TASK-0322 | Create relativist-cli thin binary (delegates to relativist_core) — R5 | P0 | **DONE** | 0321 | S |
| TASK-0323 | Verify workspace tests pass + lint clean — R6, R7 | P0 | **DONE** | 0322 | S |

## v2 Phase 14: Encoder/Decoder API (SPEC-27)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0330 | Phase 1 — Define Encoder/Decoder/Codec traits + error types — R1-R4 | P0 | **DONE** | 0323 | S |
| TASK-0331 | Phase 1 — Implement encode-contract validator (E1, E2) — R5, R6 | P0 | **DONE** | 0330 | S |
| TASK-0332 | Phase 2 — Refactor Church arithmetic to implement Codec — R7-R9 | P0 | **DONE** | 0331 | S |
| TASK-0333 | Phase 3 — LambdaCodec encoder (REF-005 mapping) — R10-R13 | P0 | **DONE** | 0331 | M |
| TASK-0334 | Phase 3 — LambdaCodec decoder (port-directed readback) — R14-R15 | P0 | **DONE** | 0333 | M |
| TASK-0335 | Phase 3 — LambdaCodec edge cases (identity, beta, erasure, dup) — R16, T5-T9 | P0 | **DONE** | 0334 | M |
| TASK-0336 | Phase 4 — EncoderRegistry struct + ops — R17, R18, R20 | P0 | **DONE** | 0332, 0335 | S |
| TASK-0337 | Phase 4 — default_registry() with 5 codecs — R19 | P0 | **DONE** | 0336 | S |
| TASK-0338 | Phase 5 — compute --encoder dispatch — R21, R23 | P0 | **DONE** | 0337 | S |
| TASK-0339 | Phase 5 — encoders list subcommand — R22 | P0 | **DONE** | 0337 | S |
| TASK-0338 | Phase 5 — CLI `compute --encoder` flag (backward-compatible) — R21, R23 | P0 | TODO | 0337 | S |
| TASK-0339 | Phase 5 — CLI `encoders list` subcommand — R22 | P0 | TODO | 0337 | S |
| TASK-0340 | Phase 6 — RecipeEncoder trait — R24, R25 (R26-R28 deferred → DEFERRED-WORK.md D-001) | P1 | **DONE** | 0337 | S |
| TASK-0341 | Phase 6 — Refactor SPEC-25 generators to RecipeEncoder — R26 | P1 | BLOCKED on M7 (SPEC-25) — see DEFERRED-WORK.md D-001 | 0340 | M |
| TASK-0342 | Phase 6 — Generalize AssignRecipe to carry encoder name — R27, R28 | P1 | BLOCKED on M7 (SPEC-25) — see DEFERRED-WORK.md D-001 | 0341 | M |

## SPEC-18 Wire Format v2 (ROADMAP 2.23, M1, Tier 1 break-even)

| Task | Description | Priority | Status | Depends | Size |
|------|-------------|----------|--------|---------|------|
| TASK-0343 | bincode v2 migration — R1-R4 | P0 | TODO | — | M |
| TASK-0344 | Compact PortRef serde encoding — R5-R8 | P0 | TODO | 0343 | S |
| TASK-0345 | Frame header v2 (9 bytes + flags) — R14-R19 | P0 | TODO | 0343 | S |
| TASK-0346 | LZ4 compression pipeline — R9-R13, R36-R39 | P0 | TODO | 0345 | M |
| TASK-0347 | PROTOCOL_VERSION bump 1→2 (atomic wire break) — R28-R32 | P0 | TODO | 0343, 0344, 0345, 0346 | S |

## SPEC-18 §3.5 Zero-Copy Archive (ROADMAP 2.24, closes DEFERRED-WORK D-002)

Bundle index: `SPEC-18-section-3.5-zero-copy-tasks.md`. Scope is strictly
R20-R27 + §3.9 R36-R37 + §7.2 T11-T14. Total ~600 LoC, 8 atomic tasks,
+24 tests under `--features zero-copy`. FLAG_RESERVED design choice
(Option B recommended: route FLAG_ARCHIVED via `recv_frame` branch, mask
unchanged) flagged for spec-critic.

| Task | Description | Priority | Status | Depends | Size |
|------|-------------|----------|--------|---------|------|
| TASK-0352 | rkyv optional dep + `zero-copy` feature gate — R20 | P0 | TODO | — | S |
| TASK-0353 | Derive Archive/Serialize/Deserialize on 8 hot-path types — R21 | P0 | TODO | 0352 | M |
| TASK-0354 | `ProtocolError::ArchiveValidationFailed` variant — R26, R35 | P0 | TODO | 0352 | S |
| TASK-0355 | 16-byte aligned receive buffer (`AlignedVec`) — R25 | P0 | TODO | 0352, 0353 | S |
| TASK-0356 | `send_frame_v2` rkyv path (hot-path only, optional LZ4) — R22, R23 | P0 | TODO | 0353, 0354 | M |
| TASK-0357 | `recv_frame` archive branch (decompress→CRC→access, R12 ordering) — R12, R22, R24, R26 | P0 | TODO | 0354, 0355, 0356 | M |
| TASK-0358 | `TransportConfig.use_zero_copy` + `--use-zero-copy` CLI — R36, R37 | P0 | TODO | 0356, 0357 | S |
| TASK-0359 | T11-T14 round-trip + corruption + archive-flag suite — R27, T11-T14 | P0 | TODO | 0353, 0356, 0357 | S |

## SPEC-19 §3.1 Coordinator-Free Round (ROADMAP 2.34, Tier 1 break-even)

Bundle index: `SPEC-19-section-3.1-coordinator-free-round-tasks.md`. Scope is
strictly R1-R7 (§3.1). §3.2 (BorderGraph), §3.3 (Delta-Only Protocol), and
§3.4-§3.7 are separate bundles (items 2.35 / 2.26).

| Task | Description | Priority | Status | Depends | Size |
|------|-------------|----------|--------|---------|------|
| TASK-0348 | Add `has_border_activity` field + `compute_border_activity` helper — R1, R2 | P0 | TODO | — | S |
| TASK-0349 | Populate `has_border_activity` at every WorkerRoundStats build site — R2 | P0 | TODO | 0348 | S |
| TASK-0350 | Add `coordinator_free_rounds` config flag + metrics counter — R6, R41p, R45p | P0 | TODO | — (logical: 0348) | S |
| TASK-0351 | Coordinator skip-merge logic + Global Normal Form termination — R3, R4, R5, R6, R7 | P0 | TODO | 0348, 0349, 0350 | M |

## SPEC-19 §3.3 Refactor — post-REVIEW 2026-04-23 (MF-001, MF-002, SF-001, SF-002)

Bundle source: `docs/reviews/REVIEW-SPEC-19-section-3.3-3.5-3.6-item-2.26-BCD-2026-04-23.md`.
Triggered by two Must-Fix items and two Should-Fix items identified during the unified REVIEW of bundle 2.26-B/C/D. Closes DEFERRED-WORK D-003 **partially** (symmetric rules); opens D-004 for asymmetric-rule closure.

| Task | Description | Priority | Status | Depends | Size |
|------|-------------|----------|--------|---------|------|
| TASK-0394 | MF-001 — Worker R23/R26 completion: `local_reconnections` + DC-B5 `pending_commutations → minted_agents` echo | P0 | **DONE** (2026-04-23) | 0381, 0384 | M |
| TASK-0395 | MF-002 — G1 parity integration tests in `merge::grid_delta_integration_tests` (Option b — in-crate; closes UT-0385-06..08 symmetric; asymmetric under `SKIP_ASYMMETRIC` pending D-004) | P0 | **DONE** (2026-04-23) | 0394 | M |
| TASK-0396 | SF-001 — R20 dispatcher fork: `run_grid_entry` routes on `cfg.delta_mode` | P1 | **DONE** (2026-04-23) | 0394 | S |
| TASK-0397 | SF-002 — R43 normalize: `coordinator_free_rounds` defaults to `true` when `delta_mode=true` | P1 | **DONE** (2026-04-23) | — | S |

**Bundle test delta:** 1109 → 1138 default (+29), 1149 → 1178 zero-copy (+29).
**Stage 5 QA probes** added inline post-DEV (Option B rigor pass): `qa_0394_a_duplicate_request_id_in_pending_commutations_is_lenient`, `qa_0394_f_exhaustion_check_treats_next_id_equals_end_as_exhausted`, `qa_0377_l_pure_core_guard_panics_on_synthetic_forbidden_import`. Covers 13 of 15 Q-probes from the REVIEW artifact; Q2/Q3 asymmetric-rule G1 parity remain open per D-004.
**Gates:** `cargo test --workspace --lib` 1138/1178 green, `cargo clippy --workspace --all-targets -- -D warnings` clean both feature configs, `cargo fmt --check` clean.

## D-004 — Coordinator-Side Round-N+2 Finalizer (post-refactor 2026-04-23)

Bundle source: `docs/DEFERRED-WORK.md` D-004 row (opened 2026-04-23 during DEV of TASK-0395). Closes D-003 fully when shipped. Scope: extend `RoundResultPayload` with `minted_agents`, add `BorderGraph::{enqueue_pending_borders, register_minted_agents}`, wire into `run_grid_delta_inner`, flip `const SKIP_ASYMMETRIC: bool = false;`. Destrava Passo 6 M1 exit measurement for all 6 IC rules.

| Task | Description | Priority | Status | Depends | Size |
|------|-------------|----------|--------|---------|------|
| TASK-0398 | Plumbing pure-core: RoundResultPayload.minted_agents; BorderGraph state + methods (enqueue_pending_borders, register_minted_agents); encode/decode_request_id helpers; package_resolutions_with_pending | P0 | **DONE** (2026-04-23) | 2.26-B (shipped), TASK-0394 (shipped), TASK-0395 (shipped) | M |
| TASK-0399 | Integration: wire into run_grid_delta_inner; migrate LocalDeltaDispatch; **REDUCED SCOPE** — plumbing wiring + LocalDeltaDispatch forwarding + `cleanup_t1_violations` helper shipped; `SKIP_ASYMMETRIC` flip blocked on newly-discovered D-005 (CommutationBatch.local_wiring not on wire); `SKIP_ASYMMETRIC = true` retained with 34-line D-005 scoped comment in test file. | P0 | **DONE** (plumbing only; flip deferred to D-005) 2026-04-23 | TASK-0398 | M |

**Bundle acceptance signal (ORIGINAL):** UT-0385-08 passes all 12 parameterized cases (6 fixtures × 2 strict modes) with canonical net-equivalence + total_interactions parity — **NOT MET** due to D-005 structural gap discovered during DEV. **Revised close signal:** TASK-0398 + TASK-0399 plumbing shipped, 1146/1186 test baselines green, REVIEW ALIGNED with 0 Must-Fix. DEFERRED-WORK D-003 remains PARTIAL; D-004 marked PARTIALLY SHIPPED; D-005 row added. Full G1 parity proof waits on D-005 Option A or B.

## D-005 — Worker-Side Application of `CommutationBatch.local_wiring` (Option A — production, wire-level)

Bundle source: `docs/DEFERRED-WORK.md` D-005 row (opened 2026-04-23 during DEV of TASK-0399). Stage 0 SPEC-CRITIC closed on Round 3 with SIGN-OFF (3 LOW NR3 findings non-blocking, absorbable in TASK-0400). Option A elected (production wire-level fix; Option B test-only explicitly rejected by user). Closes D-003 + D-004 + D-005 rows together when shipped. Scope: (a) refactor `PendingCommutation` to Shape A (`target_symbols: Vec<Symbol>` + `local_wiring: Vec<LocalWiringHint>`), introduce `LocalWiringHint` struct and `ProtocolError::MalformedLocalWiring { request_id, reason }` with 7-case `MalformedLocalWiringReason`, bump `PROTOCOL_VERSION` 2→3; (b) populate the wire fields from `CommutationBatch` in `package_resolutions_with_pending`; (c) implement R24.1.6a/b/c mint-then-wire at `worker.rs::handle_round_start` with R23a clause-6 HashSet pre-pass and R33c case dispatch; (d) forward the same transport through `LocalDeltaDispatch` and flip `const SKIP_ASYMMETRIC: bool = false;`.

| Task | Description | Priority | Status | Depends | Size |
|------|-------------|----------|--------|---------|------|
| TASK-0400 | Wire struct rewrite: `PendingCommutation` Shape A + `LocalWiringHint` + `ProtocolError::MalformedLocalWiring` 7-case enum + rkyv conditional derives + `PROTOCOL_VERSION` 2→3. Absorbs NR3-001/002/003. | P0 | TODO | TASK-0398, TASK-0399 (D-004 plumbing shipped) | S |
| TASK-0401 | Resolver-to-wire transport: extend `package_resolutions_with_pending` to populate `target_symbols` + `local_wiring` from `CommutationBatch`; optional `commutation_batch_to_pending` private helper; per-rule UTs (CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA) + order preservation. | P0 | TODO | TASK-0400 | S-M |
| TASK-0402 | Worker-side mint-then-wire: implement R24.1.6a/b/c in `worker.rs::handle_round_start`; R23a clause-6 HashSet pre-pass; R33c case 1/2/3/5/6/7 rejection; R33c case 4 tracing::warn; R24 ordering invariant; per-case UTs + CON-DUP happy path. | P0 | TODO | TASK-0400, TASK-0401 | M |
| TASK-0403 | LocalDeltaDispatch forwarding of `target_symbols` + `local_wiring`; flip `const SKIP_ASYMMETRIC: bool = false`; remove 34-line TASK-0399 skip-comment. **Acceptance gate for bundle: UT-0385-08 green on 12-case parameterized matrix.** | P0 | TODO | TASK-0400, TASK-0401, TASK-0402 | S |

**DAG:** `TASK-0400 → TASK-0401 → TASK-0402 → TASK-0403` (strict linear).
**Bundle acceptance signal:** UT-0385-08 parameterized matrix (6 fixtures × 2 strict modes = 12 cases) passes with `canonicalize(out_delta) == canonicalize(out_v1)` AND `metrics.total_interactions == metrics_v1.total_interactions` on every case. `cargo test --workspace --lib` ≥ 1151 default / ≥ 1192 `--features zero-copy`. Clippy + fmt clean both feature configs. **Closes D-003 + D-004 + D-005 DEFERRED-WORK rows.**

## SPEC-20 Elastic Grid (ROADMAP 2.1, 2.2, 2.3 — Tier 4)

Bundle source: `specs/SPEC-20-elastic-grid.md` (Reviewed v2 — Round 3 closure landed 2026-04-24).
Spec reviews: `docs/spec-reviews/SPEC-REVIEW-20-round-3-2026-04-24.md` (NF closure pass, all 11 NFs CLOSED), `docs/spec-reviews/archive/SPEC-REVIEW-20-round-2-2026-04-24.md` (CONDITIONAL_PASS), `docs/spec-reviews/archive/SPEC-REVIEW-20-round-1-2026-04-24.md` (30 findings).
Theory: ARG-001 (P1-P6), ARG-002, ARG-004, ARG-006 (CLOSED — gates R29a + R39 G1-elastic-departure for v1 + delta-conservative); ARG-005 (CLOSED at SPEC-19 boundary; conditional gate for SPEC-20 R24b-delta optimized only). Anchors per `docs/theory-bridge.md` (last updated 2026-04-24).
Scope: 36 atomic tasks (TASK-0410..TASK-0455 with intentional gaps at 0427-0429, 0431, 0444-0445, 0448-0449, 0453-0454 — preserved for future expansion / spec polish). Estimated total ~6,400 LoC production + ~3,200 LoC tests across 4 phases. Zero regression against v1 (1181 default / 1224 `--features zero-copy`); v1 floor (690 tests) MUST never regress.
Test rows forward-referenced (Stage 2 TEST-GENERATOR consumes): EG-U1..EG-U18 (unit) including EG-U1b, EG-U4-delta-wire-symmetry, EG-U9-extended (R26a both branches), EG-U15a + EG-U15b (NF-010 split); EG-I1..EG-I5 (integration); EG-P1..EG-P6 (property); EG-B1..EG-B3 (benchmark).

### Phase A — Predecessor-spec amendments (§3.8 A1..A8) — non-blocking, cross-spec

These tasks formally extend predecessor specs (SPEC-02, SPEC-04, SPEC-05, SPEC-06, SPEC-13, SPEC-19). They are forward-references for the SPEC-20 implementation but MUST land before any task that consumes them. Tag: `[SPEC-NN amendment]`.

| ID | Title | Priority | Status | Depends | Complexity | Amends |
|----|-------|----------|--------|---------|------------|--------|
| TASK-0410 | Implement `Net::union` structural-concatenation primitive | P0 | TODO | none | S | SPEC-02 (A7) |
| TASK-0411 | Expose `allocate_border_ids` + `remap_partition_ids` on `PartitionPlan` | P0 | TODO | none | M | SPEC-04 (A3, A4) |
| TASK-0412 | Extend `reconstruct` to accept optional `reclaimed_partitions` (3-arg) | P0 | TODO | 0410, 0411 | S | SPEC-19 (A8) |
| TASK-0413 | Conditional `elastic_departure` clause on R25 / PhaseTimeout path | P0 | TODO | none | S | SPEC-06 (A1) |
| TASK-0414 | Register new CoordinatorState / Event / Action enums | P0 | TODO | none | S | SPEC-13 (A2) |
| TASK-0415 | Extend `GridConfig` with 9 elastic-grid fields + R0c immutability discipline | P0 | TODO | none | S | SPEC-05 (A5) |

A6 (SPEC-19 R45 metric coexistence audit) is committed-in-record via R38a and discharged by TASK-0450 (no separate amendment task — the audit is part of the GridMetrics task).

### Phase B — Wire protocol, configuration, hybrid coordinator foundations

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0416 | CLI flags for elastic-grid configuration (R34) | P0 | TODO | 0415 | S |
| TASK-0417 | Bump `PROTOCOL_VERSION` 3 → 4 (R37, R0d) | P0 | TODO | none | S |
| TASK-0418 | Extend `Message` enum with 5 elastic-grid variants (R21, R35, R35a, R36, R35-cross-spec) | P0 | TODO | 0417 | M |
| TASK-0419 | Coordinator handshake branch — `Register` vs `JoinRequest` (R37a, R0d) | P0 | TODO | 0417, 0418 | S |
| TASK-0420 | `WorkerId` reservation + `partition_index` decoupling (R2, R2a, R7, R7a, R11, R11a) | P0 | TODO | 0414, 0418 | M |
| TASK-0421 | ID-range recomputation on `K_eff` change (R8, R13, R30) | P0 | TODO | 0411, 0420 | S |
| TASK-0422 | Coordinator event loop — `tokio::select!` 4-arm pattern (R3, R3b) | P0 | TODO | 0414, 0418, 0415 | M |
| TASK-0423 | Spawn in-process self-worker via `ChannelTransport` (R1, R3, R3a, R4-v1) | P0 | TODO | 0420, 0421, 0422 | M |
| TASK-0424 | Strict-BSP uniformity for self-partition (R3c) | P1 | TODO | 0423, 0436 | S |
| TASK-0425 | `SoloReducing` state + `reduce_n(solo_budget)` batch loop (R5, R5a, R6, R15) | P0 | TODO | 0414, 0415, 0422, 0436 | M |
| TASK-0426 | `TimerKind` enum with `#[repr(u32)]` (NF-008) | P0 | TODO | 0414 | S |
| TASK-0430 | Hybrid dispatch orchestration — `K_eff = K+1` partitioning + self-spawn wiring (R1, R2, R3b, R4-v1) | P0 | TODO | 0420, 0421, 0422, 0423 | M |
| TASK-0437 | Delta-mode self-worker symmetry — full worker delta loop (R4-delta, R4-delta-self-symmetry / NF-003) | P0 | TODO | 0423 | M |

### Phase C — Dynamic joining (Phase 2.2)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0432 | `JoinRequest` handshake handler — assign `WorkerId`, send `JoinAck` (R9, R11, R11a, R17, R35a, R0d) | P0 | TODO | 0418, 0419, 0420 | M |
| TASK-0433 | v1 re-partition on join — `K_eff_new = K_eff_old + J` (R12-v1, R13, R14-v1) | P0 | TODO | 0421, 0432, 0435, 0436 | M |
| TASK-0434 | `pending_connections_queue` buffering (R10b, R16) | P0 | TODO | 0422 | S |
| TASK-0435 | Join-window drain-then-arm protocol with min/max timers (R10, R10a, SC-007) | P0 | TODO | 0426, 0432, 0434 | M |
| TASK-0436 | Extended FSM transition table — all elastic rows (§4.1.4 closes SC-012, SC-018) | P0 | TODO | 0414, 0426, 0420, 0422 | L |
| TASK-0446 | Delta-mode rejoin cycle — mid-run `FinalStateRequest` + reconstruct + fresh `InitialPartition` (R12-delta, R12a, R14-delta) | P0 | TODO | 0412, 0432, 0437, 0435 | M |

### Phase D — Dynamic departure (Phase 2.3) + retained state

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0438 | Departure detection — `collect_timeout` + TCP-error ConnectionLost (R18, R19) | P0 | TODO | 0413, 0414, 0426, 0436 | S |
| TASK-0439 | Retained-state bookkeeping — `retained_initial` + `retained_last_acked` atomic refresh (R23, R23a, R23b-v1, R23b-delta, R23c-v1, R23c-delta, R23d, R31) | P0 | TODO | 0415 | M |
| TASK-0440 | v1-mode departure reclaim + deferred re-`split` (R24a-v1, R24b-v1, R24c, R24d, R25, R26, R30) | P0 | TODO | 0410, 0411, 0438, 0439, 0436 | M |
| TASK-0441 | Graceful `LeaveRequest`/`LeaveAck` flow (R20, R21, R22a, R22b, R22c, R35a) | P0 | TODO | 0418, 0436, 0440 | S |
| TASK-0442 | `D == K_eff` edge case — solo fallback / Error (R26a / NF-007, R27) | P0 | TODO | 0440, 0443, 0425 | S |
| TASK-0443 | Delta-mode departure reclaim + `reconstruct` + re-`split` (R24a-delta, R24b-delta, R25, R26, R27, R28, R29, R29a) | P0 | TODO | 0410, 0411, 0412, 0437, 0439, 0436 | M |
| TASK-0447 | Combined join + departure in the same round (§4.2.3) | P1 | TODO | 0433, 0440, 0446, 0443 | S |

### Phase E — Observability, invariant defense, regression gate

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0450 | `GridMetrics` elastic fields + `WorkerRoundStats::is_coordinator_self` (R38, R38a / NF-004 audit, R38b) — also discharges A6 record | P1 | TODO | 0420 | S |
| TASK-0451 | INFO/WARN logging on join (R17) and departure (R28) | P2 | TODO | 0432, 0441, 0440, 0443 | S |
| TASK-0452 | Invariant defense — debug assertions for D3-elastic, D4-elastic, D5, R31; G1-elastic-departure (R39, R24c, R24d, R11a, R31) | P1 | TODO | 0439, 0440, 0443, 0420 | S |
| TASK-0455 | v1-compatibility regression gate — all elastic flags false = v1 baseline (R32, R39-G1-v1) | P0 | TODO | ALL SPEC-20 | S |

### SPEC-20 Coverage Matrix (R-numbers, NFs, §3.8 amendments → tasks)

Every R-number (R0a..R39 plus letter sub-clauses), every closed NF (NF-001..NF-011), and every §3.8 amendment (A1..A8) MUST appear in at least one task. Coverage check below.

| Spec ID | Subject | Owning Task(s) |
|---------|---------|----------------|
| §3.0 M0 | 4-mode matrix (A/B/C/D) | TASK-0415 |
| R0a | Per-feature mode coverage | TASK-0415 |
| R0b | v1↔delta term interpretation | TASK-0415 |
| R0c | Mode immutability per run | TASK-0415 |
| R0d | Version-mismatch full-rejoin | TASK-0417, TASK-0419, TASK-0432 |
| R1 | Hybrid reduction primitives | TASK-0423, TASK-0430 |
| R2 / R2a | K_eff = K+1; cross-mode WorkerId 0 (SC-016) | TASK-0420, TASK-0430 |
| R3 / R3a / R3b / R3c | tokio::select 4-arm + self-panic + concurrent events + strict_bsp uniformity | TASK-0422, TASK-0423, TASK-0424, TASK-0430 |
| R4-v1 / R4-delta / R4-delta-self-symmetry | Per-mode merge / border resolution; NF-003 self-worker delta loop | TASK-0423, TASK-0430, TASK-0437 |
| R5 / R5a / R6 | Solo mode batch loop, termination, initial_wait_timeout | TASK-0425 |
| R7 / R7a | GridMetrics distinguishes coord-self; WorkerId 0 reserved permanently | TASK-0420, TASK-0450 |
| R8 | Self-partition id-range via `partition_index = 0` | TASK-0421 |
| R9 | Accept new TCP between rounds | TASK-0432 |
| R10 / R10a / R10b | Join-window FSM state, drain-then-arm timing, boundary buffering | TASK-0434, TASK-0435 |
| R11 / R11a | Monotonic WorkerId assignment, partition_index decoupling (D4-elastic) | TASK-0420, TASK-0432 |
| R12-v1 / R12-delta / R12a | Re-partition on join (per-mode); v1-equivalent rejoin wire-cost | TASK-0433, TASK-0446 |
| R13 | ID-range recomputation on K_eff change | TASK-0421, TASK-0433 |
| R14-v1 / R14-delta | Joining worker payload (Partition vs InitialPartition) | TASK-0433, TASK-0446 |
| R15 | Solo→grid transition on first join | TASK-0425 |
| R16 | Mid-round joins are queued | TASK-0434 |
| R17 | INFO logging on join (SHOULD) | TASK-0432, TASK-0451 |
| R18 / R19 | Timeout detection / connection-loss detection | TASK-0438 |
| R20 / R21 | Graceful departure + LeaveRequest variant + LeaveKind | TASK-0418, TASK-0441 |
| R22a / R22b / R22c | Clean leave / urgent leave / lenient upgrade | TASK-0441 |
| R23 / R23a / R23b-v1 / R23b-delta / R23c-v1 / R23c-delta / R23d | Retained-state bookkeeping; release policy (NF-011) | TASK-0439 |
| R24a-v1 / R24b-v1 | v1 catastrophic + post-success departure reclaim | TASK-0440 |
| R24a-delta / R24b-delta | Delta catastrophic + optimized reclaim (R24b-delta optimized CONDITIONAL on ARG-005) | TASK-0443 |
| R24c | D3-elastic invariant — no in-round mixed-merge | TASK-0440, TASK-0443, TASK-0452 |
| R24d | border_id rebase on reclaim (consumes A3) | TASK-0440, TASK-0443, TASK-0452 |
| R25 | Re-partition for K_eff_new = K_eff - D | TASK-0440, TASK-0443 |
| R26 | Multiple simultaneous departures, single cycle | TASK-0440, TASK-0443 |
| R26a | D == K_eff edge case (NF-007) — hybrid solo / non-hybrid Error | TASK-0442 |
| R27 | All-remote-depart fallback rules | TASK-0442, TASK-0443 |
| R28 | WARN logging on departure (SHOULD) | TASK-0443, TASK-0451 |
| R29 / R29a | At-least-once recoverability (ARG-006 v1 CLOSED; delta CONDITIONAL on ARG-005) | TASK-0443 |
| R30 | ID uniqueness preservation via `remap_partition_ids` | TASK-0421, TASK-0440, TASK-0443 |
| R31 | Retained-state release; memory bounds (NF-011) | TASK-0439, TASK-0452 |
| R32 | `retain_partitions=false` ⇒ v1 fatal-on-disconnect | TASK-0415, TASK-0455 |
| R33 / R33a | GridConfig 9 fields + defaults | TASK-0415 |
| R34 | CLI flags exposing R33 fields | TASK-0416 |
| R35 / R35a / R35-cross-spec-version-shape (NF-009) | Wire variants 12-16; LeaveAck-before-close; ProtocolVersionMismatch shape | TASK-0418, TASK-0441 |
| R36 | Serde + rkyv (zero-copy) on new variants | TASK-0418 |
| R37 / R37a | PROTOCOL_VERSION bump 4; Register vs JoinRequest selection | TASK-0417, TASK-0419 |
| R38 / R38a / R38b | GridMetrics elastic fields; non-collision audit (NF-004); is_coordinator_self (SC-027) | TASK-0450 |
| R39 (T1-T7, D1-D6, I1-I5, G1) | Invariant preservation defense (G1-elastic-departure gated by ARG-006 v1 + delta-conservative; CONDITIONAL ARG-005 for delta-optimized) | TASK-0452, TASK-0455 |
| **§3.8 A1** (SPEC-06 R25 conditional) | Elastic-departure overrides v1 PhaseTimeout fatal | TASK-0413 |
| **§3.8 A2** (SPEC-13 R21 transitions) | New CoordinatorState / Event / Action enum rows | TASK-0414 |
| **§3.8 A3** (SPEC-04 new R18a) | `PartitionPlan::allocate_border_ids` (NF-006 + Item 12 fix) | TASK-0411 |
| **§3.8 A4** (SPEC-04 new R19a) | `remap_partition_ids` (NF-006) | TASK-0411 |
| **§3.8 A5** (SPEC-05 GridConfig) | 9 new fields + R33a defaults | TASK-0415 |
| **§3.8 A6** (SPEC-19 R45 coexistence) | Metric non-collision audit committed (NF-004 — audit-only, in-record via R38a) | TASK-0450 |
| **§3.8 A7** (SPEC-02 Net::union) | New structural-concatenation primitive (NF-001) | TASK-0410 |
| **§3.8 A8** (SPEC-19 R38 reconstruct) | Optional 3rd `reclaimed_partitions` arg (NF-005) | TASK-0412 |
| NF-001..NF-011 closures | (see audit pointers in `SPEC-REVIEW-20-round-3-2026-04-24.md` §2) | as above; all NFs covered transitively by amendments + main tasks |

**Coverage completeness check:** every R-number (R0a..R39 inclusive of letter sub-clauses), every NF (NF-001..NF-011), and every §3.8 amendment (A1..A8) appears in at least one task. **PASS — no gaps.**

### SPEC-20 DAG (high-level)

Predecessor amendments first (Phase A), then config + wire foundations (0415-0419), then hybrid coordinator (0420-0430, 0437) in parallel with FSM scaffolding (0414, 0426, 0436), then joining (Phase C: 0432-0435, 0446), then departure (Phase D: 0438-0443, 0447), then observability + regression gate (Phase E: 0450-0455). TASK-0455 is the bundle gate — all SPEC-20 tasks must complete before its regression assertion runs.

### SPEC-20 Bundle gates

- All 36 tasks shipped: status DONE.
- `cargo test --workspace` ≥ 1181 default / ≥ 1224 zero-copy (zero regression on v1 floor of 690).
- New EG-* tests live under `relativist-core/tests/elastic/` and `relativist-core/src/**` per task file expectations.
- Clippy + fmt clean both feature configs.
- TASK-0455 v1-compatibility regression test passes (all elastic flags `false` reproduces v1 baseline byte-identical metrics on EP-Annihilation + DualTree + MixedNet benchmarks).

## SPEC-22 Arena Management (ROADMAP 2.32, 2.33 — free-list variant + SparseNet)

Bundle source: `specs/SPEC-22-arena-management.md` (Reviewed v2 — Round 2 closure landed 2026-04-25).
Spec reviews: `docs/spec-reviews/SPEC-REVIEW-22-round-2-2026-04-25.md` (closure pass — 20/21 CLOSED inline, 1 DEFERRED to TCC-root cleanup), `docs/spec-reviews/SPEC-REVIEW-22-round-1-2026-04-24.md` (BLOCK — 21 findings, 4 CRITICAL / 7 HIGH / 6 MEDIUM / 4 LOW).
Theory: REF-002 (Lafont 1997), REF-003 (HVM2 — arena management), REF-014 (Kahl — GC impact); AC-001 (Haskell IC.Core baseline), AC-006 (HVM2 flat-array rationale), AC-009, AC-011 (free-list ↔ HVM4 static heap partitioning), AC-015 (CC-4 ID space); ARG-002 (border bijection — informs §3.8 SPEC-04/SPEC-05 amendments), ARG-005 (delta recoverability — informs SC-005 BorderGraph constraint, OPEN; CONDITIONAL gate on R10b under delta-optimized strategy).
Scope: 36 atomic tasks (TASK-0460..TASK-0500 with intentional gaps at 0470, 0479, 0485, 0494, 0499 — preserved for future expansion / spec polish). Estimated total ~2,070 LoC production + ~1,830 LoC tests across 6 phases. Zero regression against v2 baseline (1181 default / 1224 `--features zero-copy`); v1 floor (690 tests) MUST never regress.
Test rows forward-referenced (Stage 2 TEST-GENERATOR consumes): T1..T18 (SPEC-22 §7.1 free-list + §7.2 SparseNet) plus T7a (CON-DUP under partial free-list), T8a (wire-version rejection), T9a (Strategy A protected tombstone), T9b (Strategy B border-clean), T14a (partition-scoped to_dense). The full T1..T18 + T7a/T8a/T9a/T9b/T14a coverage maps to the 28 implementation tasks via the matrix below.

### Phase A — Predecessor-spec amendments (§3.8 A1..A10) — non-blocking, cross-spec

These tasks formally extend predecessor specs (SPEC-01, SPEC-02, SPEC-03, SPEC-04, SPEC-05, SPEC-18, SPEC-19). They are forward-references for SPEC-22 implementation but MUST land before any task that consumes them. Tag: `[SPEC-NN amendment]`.

| ID | Title | Priority | Status | Depends | Complexity | Amends |
|----|-------|----------|--------|---------|------------|--------|
| TASK-0460 | Relax I3 (Monotonicity) → I3' (Uniqueness of AgentIds) | P0 | TODO | none | S | SPEC-01 (A1) |
| TASK-0461 | Relax R2 — `AgentId` reuse via free-list with explicit clearing protocol | P0 | TODO | 0460 | S | SPEC-02 (A2) |
| TASK-0462 | Restate R10 — `next_id` increment by `f = k - r` (fresh allocations only) | P0 | TODO | 0460 | S | SPEC-02 (A3) |
| TASK-0463 | Clarify R11 — "next available ID" subsumes free-list pop | P0 | TODO | 0460, 0461, 0462 | S | SPEC-02 (A4) |
| TASK-0464 | Extend R12 — `remove_agent` pushes free-list, purges `freeport_redirects` | P0 | TODO | 0460, 0461 | S | SPEC-02 (A5) |
| TASK-0465 | Reformulate §4.3 debug-assertion language as I3'-compatible | P0 | TODO | 0460 | S | SPEC-03 (A6) |
| TASK-0466 | Extend §4.5 `build_subnet` — populate per-partition free-list + 4× sparse threshold | P0 | TODO | 0460-0464 | S | SPEC-04 (A7) |
| TASK-0467 | Extend §4.2 `merge` — free-list reconciliation across partitions | P0 | TODO | 0460, 0461 | S | SPEC-05 (A8) |
| TASK-0468 | Bump `PROTOCOL_VERSION` 2 → 3 for `Net.free_list` wire layout | P0 | TODO | none | S | SPEC-18 (A9) |
| TASK-0469 | Extend §3.2 `BorderGraph` contract — recycle-protection under delta mode | P0 | TODO | 0460 | S | SPEC-19 (A10) |

### Phase B — Free-list core implementation (R1..R12, R28, R32, R9a, R6, R5)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0471 | Add `free_list: Vec<AgentId>` field to `Net` struct + constructors (R1, R8, R28) | P0 | TODO | 0461 | S |
| TASK-0472 | Modify `create_agent` to pop from free-list (R3, R4, R5) | P0 | TODO | 0471, 0463, 0462 | S |
| TASK-0473 | Modify `remove_agent` to push to free-list + purge `freeport_redirects` (R2, R7) | P0 | TODO | 0471, 0464 | S |
| TASK-0474 | Free-list no-duplicates invariant — debug assertion + optional HashSet shadow (R5, R6) | P0 | TODO | 0473 | S |
| TASK-0475 | Serde + bincode round-trip for `Net.free_list` (R9) | P0 | TODO | 0471, 0473 | S |
| TASK-0476 | Bump `PROTOCOL_VERSION` 2 → 3 + v2-vs-v3 rejection clause (R9a) | P0 | TODO | 0468, 0475 | S |
| TASK-0477 | `count_live_agents` MUST NOT count free-list entries (R11) | P1 | TODO | 0473 | S |
| TASK-0478 | M5-scale bitmap free-list fallback (`bitvec::BitVec` representation) (R32) | P2 | TODO | 0471, 0472, 0473, 0474 | M |

### Phase C — Distributed integration (R10, R10a, R10b, R10c, R12, R22, R30)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0480 | Per-worker ID range constraint on recycle (R10) | P0 | TODO | 0472, 0481 | S |
| TASK-0481 | `build_subnet` populates partition free-list with in-range `None` slots (R10a) | P0 | TODO | 0466, 0471, 0480 | S |
| TASK-0482 | `RecyclePolicy` enum + `GridConfig.recycle_under_delta` + `is_border_protected` wiring (R10b/R10c — Strategy A and Strategy B) | P0 | TODO | 0469, 0473, 0472, 0480 | M |
| TASK-0483 | `merge` free-list reconciliation across partitions (R12 — consumer of A8) | P0 | TODO | 0467, 0471 | S |
| TASK-0484 | `PartitionError::DenseAllocationExceedsThreshold` + `sparse_build` flag rejection at threshold (R30) | P0 | TODO | 0466 | S |

### Phase D — SparseNet (R13..R23, R29)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0486 | Define `SparseNet` struct + constructors (R13, R18, R29) | P0 | TODO | none | S |
| TASK-0487 | SparseNet operations — create/remove/connect/disconnect/get_target/get_agent/is_reduced/count_live (R14, R15, R16, R17) | P0 | TODO | 0486 | M |
| TASK-0488 | `Send + Sync` compile-time assertions for `Net` and `SparseNet` (§4.4) | P1 | TODO | 0471, 0486 | S |
| TASK-0489 | `Net::to_sparse()` conversion (R19) | P0 | TODO | 0486, 0487, 0471 | S |
| TASK-0490 | `SparseNet::to_dense(id_range)` conversion with partition scoping (R20 — closes SC-006) | P0 | TODO | 0486, 0487, 0471 | S |
| TASK-0491 | `Net::is_behaviorally_equal` helper + R21 round-trip closure (closes SC-014) | P0 | TODO | 0489, 0490, 0471 | S |
| TASK-0492 | Sparse-then-dense `build_subnet` integration under 4× threshold (R22 — consumer of A7) | P0 | TODO | 0466, 0481, 0484, 0489, 0490 | M |
| TASK-0493 | CI lint forbidding `SparseNet` imports in `src/reduction/**` (R23 — closes SC-008) | P1 | TODO | 0486 | S |

### Phase E — Invariant amendments + observability (R24..R27, R27a, R31)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0495 | I3' uniqueness debug assertions in `remove_agent` / `create_agent` (R24, R25, R27) | P0 | TODO | 0460, 0472, 0473, 0482 | S |
| TASK-0496 | T1 / I1 / I2 SparseNet debug assertions (R26) | P1 | TODO | 0486, 0487 | S |
| TASK-0497 | SPEC-03 reduction-engine assertion audit — reformulate as I3'-compatible (R27a — consumer of A6) | P0 | TODO | 0465, 0472 | M |
| TASK-0498 | Safe-Rust-only audit — confirm SPEC-22 implementations contain no `unsafe` (R31) | P2 | TODO | 0471, 0472, 0473, 0486, 0487, 0489, 0490 | S |

### Phase F — Regression gate

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0500 | v1 backward-compat regression — all 1181/1224 tests pass with free-list always-on default (R28, R29) | P0 | TODO | ALL SPEC-22 | S |

### SPEC-22 Coverage Matrix (R-numbers, A-amendments → tasks)

Every R-number (R1..R32 inclusive of letter sub-clauses R6, R9a, R10a, R10b, R10c, R23, R27a, R30) and every §3.8 amendment (A1..A10) MUST appear in at least one task. Coverage check below.

| Spec ID | Subject | Owning Task(s) |
|---------|---------|----------------|
| R1 | `Net.free_list: Vec<AgentId>` field | TASK-0471 |
| R2 | `remove_agent` push to free-list | TASK-0473 |
| R3 | `create_agent` free-list-pop OR next_id increment | TASK-0472 |
| R4 | Recycled-slot semantics (Some(Agent), DISCONNECTED ports, no expansion) | TASK-0472 |
| R5 | LIFO ordering | TASK-0472, TASK-0474 |
| R6 | Free-list no-duplicates (MUST + debug_assert + optional HashSet shadow) | TASK-0474 |
| R7 | No `PortRef::AgentPort` references to free-list IDs | TASK-0473, TASK-0495 (family 4) |
| R8 | Constructors initialize empty free-list | TASK-0471 |
| R9 | Serde participation | TASK-0475 |
| R9a | `PROTOCOL_VERSION` 2→3; v2-vs-v3 rejection (closes SC-007) | TASK-0468, TASK-0476 |
| R10 | Per-worker ID range constraint (free-list confined to `[start, end)`) | TASK-0480, TASK-0481 |
| R10a | `build_subnet` populates partition free-list (closes SC-006 dense path) | TASK-0481 (consumes A7) |
| R10b | BorderGraph slot-id stability — Strategy A (`DisableUnderDelta`, default) and Strategy B (`BorderClean`); explicit acceptance criteria for both code paths under `GridConfig.recycle_under_delta` | TASK-0482 (consumes A10) |
| R10c | Protected tombstone semantics (slot None, ports DISCONNECTED, ID NOT in free-list) | TASK-0482, TASK-0495 (family 2) |
| R11 | `count_live_agents` excludes free-list | TASK-0477 |
| R12 | `merge` free-list reconciliation | TASK-0483 (consumes A8) |
| R13 | `SparseNet` field list (incl. `freeport_redirects`, closes SC-011) | TASK-0486 |
| R14 | SparseNet operations parity with Net | TASK-0487 |
| R15 | SparseNet O(1) amortized complexity | TASK-0487 |
| R16 | SparseNet no tombstones | TASK-0487 |
| R17 | SparseNet no ERA auxiliary port entries (sparse equivalent of I6) | TASK-0487 |
| R18 | SparseNet derives (Debug/Clone/Eq/Serialize/Deserialize) | TASK-0486 |
| R19 | `Net::to_sparse()` | TASK-0489 |
| R20 | `SparseNet::to_dense(id_range)` (signature change closes SC-006) | TASK-0490 |
| R21 | Round-trip behavioral equality + `Net::is_behaviorally_equal` helper (closes SC-014) | TASK-0491 |
| R22 | Sparse-then-dense `build_subnet` under 4× threshold (closes SC-009) | TASK-0492 (consumes A7) |
| R23 | DESIGN CONSTRAINT — CI lint forbidding SparseNet in `src/reduction/**` (closes SC-008) | TASK-0493 |
| R24 | I3' (Uniqueness of AgentIds) statement | TASK-0460 (A1), TASK-0495 |
| R25 | D4 preservation under I3' | TASK-0460, TASK-0480, TASK-0495 |
| R26 | SparseNet T1/I1/I2 debug assertions | TASK-0496 |
| R27 | Free-list debug assertions (4 families: post-remove recycle, post-remove protected-tombstone, post-create recycle, periodic) | TASK-0495 |
| R27a | SPEC-03 in-rule assertion audit (closes SC-010; CON-DUP load-bearing) | TASK-0497 (consumes A6) |
| R28 | Always-on default (no feature gate) | TASK-0471, TASK-0472, TASK-0500 |
| R29 | SparseNet always available (no feature gate) | TASK-0486, TASK-0500 |
| R30 | `sparse_build` flag MUST + `PartitionError::DenseAllocationExceedsThreshold` rejection | TASK-0484 |
| R31 | Safe-Rust-only audit (closes SC-017) | TASK-0498 |
| R32 | M5-scale bitmap free-list fallback (closes SC-015) | TASK-0478 |
| **§3.8 A1** (SPEC-01 I3 → I3') | Monotonicity → Uniqueness | TASK-0460 |
| **§3.8 A2** (SPEC-02 R2 reuse) | Lifts "never reused" with clearing protocol | TASK-0461 |
| **§3.8 A3** (SPEC-02 R10 increment) | `f = k - r` accounting under I3' | TASK-0462 |
| **§3.8 A4** (SPEC-02 R11 clarify) | "Next available ID" subsumes free-list pop | TASK-0463 |
| **§3.8 A5** (SPEC-02 R12 extend) | `remove_agent` pushes free-list + purges `freeport_redirects` | TASK-0464 |
| **§3.8 A6** (SPEC-03 §4.3 assertions) | I3'-compatible assertion allowlist/denylist | TASK-0465 |
| **§3.8 A7** (SPEC-04 §4.5 build_subnet) | Per-partition free-list + 4× sparse threshold | TASK-0466 |
| **§3.8 A8** (SPEC-05 §4.2 merge) | Free-list reconciliation algorithm | TASK-0467 |
| **§3.8 A9** (SPEC-18 PROTOCOL_VERSION) | Bump 2 → 3 + v2-vs-v3 rejection | TASK-0468 |
| **§3.8 A10** (SPEC-19 §3.2 BorderGraph) | Recycle-protection under delta mode (Strategy A/B) | TASK-0469 |

**Per-amendment R-number verification (Round 2 §"Round 3 confirmation suggestions" item 1):** every §3.8 amendment cites a verbatim target-spec R-number. All 10 amendments verified against target specs at Round 2 (closure log §"Cross-spec consistency re-audit" lines 132-150) and re-verified by task-splitter against the SPEC-22 frontmatter `Amends:` line:

- A1 → SPEC-01 I3 (lines 289-296) — RESOLVES.
- A2 → SPEC-02 R2 (line 37) — RESOLVES.
- A3 → SPEC-02 R10 (line 58) — RESOLVES.
- A4 → SPEC-02 R11 — RESOLVES (R-number cited verbatim in SPEC-22 §3.8 A4 *Old text*).
- A5 → SPEC-02 R12 — RESOLVES (R-number cited verbatim in SPEC-22 §3.8 A5 *Old text*).
- A6 → SPEC-03 §4.3 (section reference; no R-number — SPEC-03 §4.3 is generic prose) — RESOLVES at section granularity.
- A7 → SPEC-04 §4.5 build_subnet — RESOLVES at section granularity.
- A8 → SPEC-05 §4.2 merge (line 322) — RESOLVES.
- A9 → SPEC-18 R28 (line 163; live constant `PROTOCOL_VERSION = 2` at line 536) — RESOLVES.
- A10 → SPEC-19 §3.2 R8-R12 BorderGraph — RESOLVES at R-number range granularity.

**Coverage completeness check:** every R-number (R1..R32 inclusive of letter sub-clauses) and every §3.8 amendment (A1..A10) appears in at least one task. **PASS — no gaps.**

### SPEC-22 DAG (high-level)

Predecessor amendments first (Phase A: 0460 → 0461/0462 → 0463 → 0464; 0465 ← 0460; 0466 ← 0460-0464; 0467 ← 0460-0461; 0468 standalone; 0469 ← 0460), then free-list core implementation (Phase B: 0471 → 0472, 0473, 0474, 0475, 0477, 0478; 0476 ← 0468+0475), then distributed integration (Phase C: 0480 ← 0472+0481; 0481 ← 0466+0471+0480; 0482 ← 0469+0472+0473+0480; 0483 ← 0467+0471; 0484 ← 0466), then SparseNet (Phase D: 0486 → 0487, 0488, 0489, 0490, 0491, 0492, 0493), then invariants + audit (Phase E: 0495, 0496, 0497, 0498), and finally Phase F regression gate 0500 (depends on ALL).

### SPEC-22 Bundle gates

- All 36 tasks shipped: status DONE.
- `cargo test --workspace` ≥ 1181 default / ≥ 1224 zero-copy (zero regression on v1 floor of 690).
- New SPEC-22 tests live under `relativist-core/src/net/{core,sparse,free_list}.rs` test modules and `relativist-core/tests/spec22_*.rs` integration files.
- Clippy + fmt clean both feature configs.
- TASK-0493 SparseNet-import lint passes; TASK-0498 unsafe-free audit passes.
- TASK-0500 v1-compatibility regression test passes (free-list-aware default `GridConfig` reproduces v2-baseline metrics on EP-Annihilation + DualTree + MixedNet benchmarks).

## D-012: Instrumentation Restore (post-D011 follow-up — RF-04/05/07 + release-tests blocker)

Bundle source: `docs/handoffs/2026-05-05-D012-instrumentation-restore-handoff.md` (READY TO DISPATCH 2026-05-05).
Origin: `docs/analysis/D011-final-baseline-analysis-2026-05-04.md` §3 (Red Flags RF-04, RF-05, RF-07) + §7 (Concrete follow-ups). Surfaced during D-011 LOCK pass and recorded in `docs/next-steps.md` "New follow-up surfaced by post-mortem analysis" block.
Scope: **strictly maintenance/instrumentation** — no spec changes, no behavior change to the production binary's correctness or wall-time. The four work items restore three CSV columns that are structurally zero on every v2 dataset and unblock the broken `cargo test --release` lane. Estimated total ~170 LoC production + ~210 LoC tests across 4 atomic tasks. Should fit a single SDD cycle. Zero regression against current floors (1784 default / 1828 zero-copy / 1775 streaming-no-recycle); v1 floor (690) inviolable.
Independent ordering: TASK-0617 ships first (no deps, unblocks CI release lane). TASK-0615 + TASK-0616 are parallelisable (both extend protocol instrumentation). TASK-0618 decides path (a) implement vs path (b) drop at DEV time.

| ID | Title | Priority | Status | Depends | Complexity | Closes RF |
|----|-------|----------|--------|---------|------------|-----------|
| TASK-0615 | D-011-FU-NETMETRIC — restore `network_send/recv_time_per_round` push sites in coordinator+worker I/O paths | P0 | TODO | none | M | RF-04 |
| TASK-0616 | D-011-FU-COMPMETRIC — aggregate per-worker compute time into coordinator-side `GridMetrics.compute_time_per_round` (TCP path; path-(a) recommended) | P0 | TODO | none | M | RF-05 |
| TASK-0617 | D-011-FU-RELEASE-TESTS — fix `cargo test --release` compilation (debug.rs gating + coordinator.rs match arm) | P0 | TODO | none, can ship first | S | (release-lane blocker — not from RF catalog; logged in next-steps.md 2026-05-05) |
| TASK-0618 | D-011-FU-MIPS — decide implement (`total_interactions` end-to-end) vs drop (`mips_*` columns from CSV); rationale documented in commit body | P2 | TODO | none | S–M | RF-07 |

### D-012 Coverage Matrix (Red Flags + handoff inventory → tasks)

Every red flag from §3 of the analysis (RF-04, RF-05, RF-07) plus the release-tests blocker (logged separately) MUST appear in at least one task. Coverage check below.

| Source | Subject | Owning Task |
|--------|---------|-------------|
| RF-04 (`docs/analysis/D011-final-baseline-analysis-2026-05-04.md` §3 lines 142–146) | `network_time_secs = 0.0` for every v2 round | TASK-0615 |
| RF-05 (analysis §3 lines 148–154) | `compute_time_secs = 0.0` for every v2 row | TASK-0616 |
| RF-07 (analysis §3 lines 174–179) | `mips_mean = 0.000` everywhere — symmetric on v1 + v2 | TASK-0618 |
| `next-steps.md` 2026-05-05 / handoff §2 row 3 | `cargo test --release` does not compile at HEAD `b079cdc` (two pre-existing defects unrelated to D-011) | TASK-0617 |
| Handoff §2 row 1 (D-011-FU-NETMETRIC, HIGH) | Maps 1:1 to RF-04 | TASK-0615 |
| Handoff §2 row 2 (D-011-FU-COMPMETRIC, HIGH) | Maps 1:1 to RF-05 | TASK-0616 |
| Handoff §2 row 3 (D-011-FU-RELEASE-TESTS, MEDIUM) | Maps 1:1 to release blocker | TASK-0617 |
| Handoff §2 row 4 (D-011-FU-MIPS, LOW) | Maps 1:1 to RF-07 | TASK-0618 |

**Coverage completeness check:** every RF in scope (RF-04, RF-05, RF-07) and every handoff §2 inventory row appears in at least one task. **PASS — no gaps.** RF-01, RF-02, RF-03, RF-06, RF-08, RF-09 from the analysis are out of scope per handoff §6 (RF-02 deferred to a separate one-off task or D-013; RF-01/03/06 are TCC framing concerns not DEV scope; RF-08/09 are positive findings — no remediation needed).

### D-012 DAG (high-level)

All 4 tasks are independent (no inter-task dependencies). TASK-0617 is recommended to ship first because it unblocks the CI release lane and lets TASK-0615/0616 exercise `cargo test --release` for invariant-free regression. TASK-0618 path-(a) authors should coordinate with TASK-0616 author to consolidate the `PartitionResult` payload extension into a single wire-format change rather than two separate ones.

### D-012 Bundle gates

- All 4 tasks shipped: status DONE.
- `cargo test --workspace` ≥ 1786 default (1784 baseline + TASK-0615 witness + TASK-0616 witness; TASK-0618 adds +1 either way; TASK-0617 contributes 0 or net-negative to default — release count is separately tracked).
- `cargo test --release` compiles and runs to completion (TASK-0617 gate). Exact post-fix release count documented in commit body and added as a new floor row to `next-steps.md` / `CLAUDE.md`.
- `cargo test --features zero-copy` ≥ 1828 (unchanged); `cargo test --features streaming-no-recycle` ≥ 1775 (unchanged); v1 floor 690 inviolable.
- Clippy + fmt clean across all feature configs (debug AND release for TASK-0617).
- A TCP-mode benchmark produces `rounds.csv` with `network_time_secs > 0` (TASK-0615) and `compute_time_secs > 0` (TASK-0616) on every non-trivial round.
- `summary.csv::mips_mean > 0` (TASK-0618 path a) OR `summary.csv` headers do not contain `mips_mean` / `total_interactions` (TASK-0618 path b).
- Wall-clock ratio v2-post / v1 unchanged outside ±5% on the TASK-0614 verification slot (`ep_con 5M w=2 local`); current ratio 1.11×, ceiling 1.16× post-D-012.

## SPEC-21 Streaming Generation (ROADMAP 2.30 — chunked pipeline + pull dispatch)

Bundle source: `specs/SPEC-21-streaming-generation.md` (Reviewed v2 — Round 2 closure landed 2026-04-25).
Spec reviews: `docs/spec-reviews/SPEC-REVIEW-21-round-2-2026-04-25.md` (closure pass), `docs/spec-reviews/SPEC-REVIEW-21-round-1-2026-04-25.md`.
Theory: REF-002 (Lafont 1997), AC-007 (HVM2 reduction engine — informs §4.6 install_connection border detection), AC-010 (HVM4 WNF — informs §4.9 PartitionAccumulator frame-reuse pattern), AC-014 (Bench Methodology — canonical reference for §7.4 T10 peak-memory measurement), ARG-005 (delta recoverability — informs §3.7 R37b/R37f cross-spec gates).
Scope: 36 atomic tasks (TASK-0510..0517 Phase A amendments + TASK-0520..0524 Phase B foundation types + TASK-0530..0531 Phase C strategies + TASK-0540..0544 Phase D benchmarks + TASK-0550..0554 Phase E accumulator/orchestrator + TASK-0565/0567/0568/0575..0578/0588..0591 Phase F regression/polish/late-binding). Estimated total ~2,400 LoC production + ~2,100 LoC tests across 6 phases. Zero regression against v2 baseline (1181 default / 1224 zero-copy); v1 floor (690) MUST never regress.
Test rows forward-referenced (Stage 2 TEST-GENERATOR consumed): 24 plumbing TEST-SPECs (TEST-SPEC-0510..0517, 0520..0524, 0530..0531, 0540..0544, 0550..0554) + 14 spec-catalog TEST-SPECs (T1..T14). Total 38 SPEC-21 TEST-SPEC files on disk.

### Phase A — Predecessor-spec amendments (§3.8 A1..A8) — non-blocking, cross-spec

| ID | Title | Priority | Status | Depends | Complexity | Amends |
|----|-------|----------|--------|---------|------------|--------|
| TASK-0510 | SPEC-04 R12 amendment — border-id allocation for streaming pipeline | P0 | TODO | none | S | SPEC-04 (A1) |
| TASK-0511 | SPEC-06 amendment — `Message` enum gains `RequestWork` / `NoMoreWork` + PROTOCOL_VERSION sequencing | P0 | TODO | none | S | SPEC-06 (A2) |
| TASK-0512 | SPEC-07 GridConfig amendment — three new streaming fields | P0 | TODO | none | S | SPEC-07 (A3) |
| TASK-0513 | SPEC-09 Benchmark trait amendment — default-impl-bearing `make_net_stream` | P0 | TODO | none | S | SPEC-09 (A4) |
| TASK-0514 | SPEC-13 amendment — coordinator + worker FSM additions for pull dispatch | P0 | TODO | none | S | SPEC-13 (A5) |
| TASK-0515 | SPEC-22 R10b broadening amendment — `(delta_mode \|\| streaming_active)` gate | P0 | TODO | none | S | SPEC-22 (A6) |
| TASK-0516 | SPEC-19 BorderGraph amendment — `extend_with_chunk_borders` method | P0 | TODO | none | S | SPEC-19 (A7) |
| TASK-0517 | SPEC-04 §4.5 clarification — split() unchanged; chunked pipeline additive | P0 | TODO | none | S | SPEC-04 (A8) |

### Phase B — Foundation types (R1..R3, R14, R20-R23)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0520 | `ConnectionDirective` enum (Resolved / Pending) | P0 | TODO | none | S |
| TASK-0521 | `AgentBatch` struct (agents + connection directives) | P0 | TODO | 0520 | S |
| TASK-0522 | `StreamingPartitionStats` (per-batch metrics; chunks_processed pipeline-owned) | P0 | TODO | 0521 | S |
| TASK-0523 | `ChunkedPartitionResult` struct (partitions + borders + stats) | P0 | TODO | 0521, 0522, 0510 | S |
| TASK-0524 | `StreamingPartitionStrategy` trait | P0 | TODO | 0521 | S |

### Phase C — Strategies (R4..R8)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0530 | `RoundRobinStreamingStrategy` (default, R4) | P0 | TODO | 0524 | S |
| TASK-0531 | `FennelStreamingStrategy` (advanced, R5..R6) | P1 | TODO | 0524 | M |

### Phase D — Benchmark integration (R10..R15)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0540 | `Benchmark::make_net_stream` default impl + `default_chunked_iter` helper (R10/R11) | P0 | TODO | 0513, 0521, 0520 | S |
| TASK-0541 | `ep_annihilation_stream` native streaming override (R12 MUST) | P0 | TODO | 0540, 0521, 0520 | S |
| TASK-0542 | `dual_tree_stream` native streaming override with forward refs (R12 SHOULD, R14) | P1 | TODO | 0540, 0521, 0520 | M |
| TASK-0544 | R15 monotonicity discipline (generator-phase contract) | P0 | TODO | 0521 | S |

### Phase E — Accumulator + orchestrator (R17..R23, R29b)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0550 | `PartitionAccumulator` struct + `AccumulatorNet` (default Sparse; SC-006) | P0 | TODO | 0486, 0487 | S |
| TASK-0551 | `add_agent` + `connect` (Sparse path) | P0 | TODO | 0550 | S |
| TASK-0552 | `finalize` (Sparse → Dense via `to_dense(id_range)`; R23, R30) | P0 | TODO | 0550, 0551, 0490 | S |
| TASK-0553 | `install_connection` helper (internal vs border classification; AC-007) | P0 | TODO | 0551 | S |
| TASK-0554 | `generate_and_partition_chunked` orchestrator (T5, T6 partial, T8 partial) | P0 | TODO | 0540, 0524, 0530, 0552, 0553, 0523 | M |

### Phase F — Regression / polish / late-binding (gap-fill wave)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0565 | `GridConfig` streaming fields production (chunk_size, streaming_strategy, dispatch_mode) | P0 | TODO | 0512, 0524, 0530, 0531 | S |
| TASK-0567 | R26 short-circuit + T6/T8 isomorphism oracle (`chunk_size = u32::MAX` → split()) | P0 | TODO | 0540, 0554, 0517, 0565, 0541, 0542, 0531 | M |
| TASK-0568 | CLI streaming flags (`--chunk-size`, `--streaming-strategy`, `--dispatch-mode`) | P1 | TODO | 0512, 0565 | S |
| TASK-0575 | `RequestWork` / `NoMoreWork` wire variants production | P0 | TODO | 0511, 0476 | S |
| TASK-0576 | PROTOCOL_VERSION bump production (defensive `PREVIOUS_LIVE_VERSION + 1`) | P0 | TODO | 0511, 0476, 0575 | S |
| TASK-0577 | Coordinator FSM extension — pull-dispatch states + transitions | P0 | TODO | 0514, 0511, 0575, 0576, 0565, 0554 | L |
| TASK-0578 | Worker FSM extension — pull-dispatch states + heterogeneous-worker simulation harness | P0 | TODO | 0514, 0511, 0575, 0576 | L |
| TASK-0588 | `BorderGraph::extend_with_chunk_borders` call-site discipline (delta+streaming) | P0 | TODO | 0516, 0553, 0554, 0577 | M |
| TASK-0589 | SPEC-22 R10b Strategy A (`DisableUnderDelta`) wiring under streaming | P0 | TODO | 0515, 0482, 0578, 0554 | S |
| TASK-0590 | SPEC-22 R10b Strategy B (`BorderClean`) wiring under streaming | P1 | TODO | 0515, 0482, 0578, 0589 | M |
| TASK-0591 | `streaming-no-recycle` cargo feature gate (alternative one-liner closure of R37b) | P2 | TODO | 0515, 0589, 0590 | S |

### SPEC-21 Coverage Matrix (R-numbers + A-amendments + T-tests → tasks)

Every R-number (R1..R39 inclusive of letter sub-clauses R29b, R37b/c/d/e/f/g), every §3.8 amendment (A1..A8), and every spec-catalog test (T1..T14) MUST appear in at least one task. Coverage check below.

| Spec ID | Subject | Owning Task(s) |
|---------|---------|----------------|
| R1 | `StreamingPartitionStrategy` trait | TASK-0524 |
| R2 | Trait stateful (`&mut self`) | TASK-0524 |
| R3 | Trait `allocate_batch` signature | TASK-0524, TASK-0530 |
| R4 | `RoundRobinStreamingStrategy` default (i % num_workers) | TASK-0530 |
| R5 | `FennelStreamingStrategy` advanced (alpha-parametrized) | TASK-0531 |
| R6 | Fennel cache O(total_agents) memory bound | TASK-0531 |
| R7 | C1 closure (every agent assigned exactly once) | TASK-0530, TASK-0531 |
| R8 | Determinism (same input → same assignment) | TASK-0530, TASK-0531 |
| R9 | Pure-Core (no async/tokio/I/O) | TASK-0524, TASK-0530, TASK-0531 |
| R10 | `Benchmark::make_net_stream` with default impl | TASK-0513 (A4), TASK-0540 |
| R11 | `make_net` UNCHANGED (source-of-truth materialization) | TASK-0540 |
| R12 | Each generator gains streaming variant (ep_annihilation MUST; others SHOULD) | TASK-0541, TASK-0542 |
| R13 | ep_annihilation trivial streaming (informative) | TASK-0541 |
| R14 | Forward references via `PendingConnection` | TASK-0520, TASK-0521, TASK-0542 |
| R15 | Generator-phase monotonicity (strictly stronger than I3') | TASK-0544 |
| R16 | Generator pure-Core; Iterator::next supports pull dispatch | TASK-0540, TASK-0541, TASK-0542 |
| R17 | `generate_and_partition_chunked` orchestrator | TASK-0554 |
| R18 | 8-step per-chunk pipeline | TASK-0554 |
| R19 | Empty pending store assertion at end | TASK-0552, TASK-0554 |
| R20 | `ChunkedPartitionResult` field list | TASK-0523 |
| R21 | Structural compat with PartitionPlan (R20-R21 conversion) | TASK-0523, TASK-0567 |
| R22 | One AgentBatch in flight invariant (peak memory bound) | TASK-0554, TASK-0552 |
| R23 | Per-worker accumulator id-range scoping | TASK-0552 |
| R24 | `chunk_size` configurable via GridConfig | TASK-0512 (A3), TASK-0565, TASK-0568 |
| R25 | `streaming_strategy` selectable via GridConfig | TASK-0512 (A3), TASK-0565, TASK-0568 |
| R26 | `chunk_size = u32::MAX` short-circuit to `split()` (closes SC-014) | TASK-0567 |
| R27 | Invariant preservation (T1, I3', D1 extended) | TASK-0552, TASK-0554 |
| R28 | Debug C1-C3 assertions on finalized output | TASK-0554 |
| R29 | ID range computation identical to SPEC-04 | TASK-0552, TASK-0554 |
| R29b | Border-id allocation (streaming path) | TASK-0510 (A1), TASK-0554 |
| R30 | Pull-based dispatch mode | TASK-0511 (A2), TASK-0577 |
| R31 | Two new `Message` enum variants | TASK-0511 (A2), TASK-0575 |
| R32 | 5-step pull protocol | TASK-0577, TASK-0578 |
| R33 | Pull preserves push invariants (R27-R29) | TASK-0577, TASK-0578, TASK-0567 |
| R34 | `dispatch_mode` field on GridConfig | TASK-0512 (A3), TASK-0565, TASK-0568 |
| R35 | Short-stream edge case (fewer chunks than workers) | TASK-0578 |
| R36 | Delta+pull compatibility (SHOULD baseline; MUST under conjunction) | TASK-0577, TASK-0588 |
| R37 | Pull throughput ≥ push under heterogeneous workers (SHOULD) | TASK-0578 |
| R37b | G1 free-list interaction (closes SC-007) | TASK-0515 (A6), TASK-0589, TASK-0590, TASK-0591 |
| R37c | PROTOCOL_VERSION sequencing (defensive +1) | TASK-0511 (A2), TASK-0576 |
| R37d | BSP barrier under pull dispatch (closes SC-019) | TASK-0577 |
| R37e | Push-mode termination scoping (closes SC-013) | TASK-0575, TASK-0577, TASK-0578 |
| R37f | BorderGraph extension under delta+streaming (closes SC-017) | TASK-0516 (A7), TASK-0588 |
| R37g | Pending-store memory bound `MAX_PENDING_LIFETIME` (closes SC-016) | TASK-0512 (A3, optional 4th field), TASK-0565 |
| **§3.8 A1** (SPEC-04 R12 border-id) | Streaming-path border-id allocation | TASK-0510 |
| **§3.8 A2** (SPEC-06 Message enum + PROTOCOL_VERSION) | RequestWork / NoMoreWork variants + version bump | TASK-0511 |
| **§3.8 A3** (SPEC-07 GridConfig fields) | chunk_size + streaming_strategy + dispatch_mode (+ optional max_pending_lifetime) | TASK-0512 |
| **§3.8 A4** (SPEC-09 Benchmark trait) | `make_net_stream` default-impl-bearing addition | TASK-0513 |
| **§3.8 A5** (SPEC-13 FSM additions) | Coordinator 5 states + worker 2 states (pull-only) | TASK-0514 |
| **§3.8 A6** (SPEC-22 R10b broadening) | `(delta_mode \|\| streaming_active)` gate | TASK-0515 |
| **§3.8 A7** (SPEC-19 BorderGraph extension) | `extend_with_chunk_borders` method signature | TASK-0516 |
| **§3.8 A8** (SPEC-04 §4.5 clarification) | split() unchanged; chunked pipeline additive | TASK-0517 |
| **T1** | Round-robin assignment correctness | TASK-0530 |
| **T2** | AgentBatch construction | TASK-0520, TASK-0521 |
| **T3** | Forward-reference resolution | TASK-0542, TASK-0553, TASK-0554 |
| **T4** | Empty pending store assertion | TASK-0554 |
| **T5** | Streaming pipeline → valid partitions | TASK-0554 |
| **T6** | Streaming-vs-batch equivalence (isomorphism) | TASK-0540, TASK-0517, TASK-0554, TASK-0567 |
| **T7** | End-to-end reduction equivalence (post-streaming) | TASK-0542, TASK-0554, TASK-0567 (run_grid integration) |
| **T8** | Chunk-size independence | TASK-0541, TASK-0554, TASK-0567 |
| **T9** | Strategy independence (RR vs Fennel) | TASK-0530, TASK-0531, TASK-0554 |
| **T10** | Peak memory measurement (one-batch-in-flight invariant) | TASK-0552, TASK-0554 (loose ceiling); strict bound deferred to future hardening task |
| **T11** | Pull-based dispatch protocol exercise | TASK-0577, TASK-0578 |
| **T12** | Pull-vs-push equivalence | TASK-0577, TASK-0578 |
| **T13** | Short-stream / fewer-chunks-than-workers edge case | TASK-0577, TASK-0578 |
| **T14** | Heterogeneous-worker simulation (pull throughput ≥ push) | TASK-0578 |

**Per-amendment R-number verification:** every §3.8 amendment cites a verbatim target-spec R-number / section reference. Re-verified by task-splitter against the SPEC-21 frontmatter `Amends:` line:

- A1 → SPEC-04 R12 — RESOLVES (R-number cited verbatim in SPEC-21 §3.8 A1 *Old text*).
- A2 → SPEC-06 `Message` enum + PROTOCOL_VERSION — RESOLVES at enum-catalog granularity (SPEC-06 R5 discriminant-stability rule + R-NN PROTOCOL_VERSION clause).
- A3 → SPEC-07 `GridConfig` struct — RESOLVES at section granularity (additive struct extension).
- A4 → SPEC-09 R2 `Benchmark` trait — RESOLVES (R-number cited verbatim in SPEC-21 §3.8 A4 *Old text*).
- A5 → SPEC-13 coordinator/worker FSM section — RESOLVES at section granularity (FSM state-list extension).
- A6 → SPEC-22 R10b — RESOLVES (R-number cited verbatim in SPEC-21 §3.8 A6 *Old text*).
- A7 → SPEC-19 §3.2 — RESOLVES at section granularity (additive method on `BorderGraph`).
- A8 → SPEC-04 §4.5 — RESOLVES at section granularity (clarification only; `split()` unchanged).

**Phase F cross-spec audit:** Phase F tasks introduce NO new amendments. They are pure regression / polish / late-binding production wiring. The cross-spec references they touch (SPEC-04 R26 short-circuit oracle in TASK-0567; SPEC-06 PROTOCOL_VERSION in TASK-0576; SPEC-13 FSM scaffolding consumed by TASK-0577/0578; SPEC-19 `extend_with_chunk_borders` consumed by TASK-0588; SPEC-22 R10b consumed by TASK-0589/0590/0591) are all GATED on Phase A amendment tasks (TASK-0510..0517) which carry the formal §3.8 amendment language. **PASS — no new amendments introduced by Phase F.**

**Coverage completeness check:** every R-number (R1..R37, sub-clauses R29b/R37b-g), every §3.8 amendment (A1..A8), and every spec-catalog test (T1..T14) appears in at least one task. **PASS — no gaps.**

### SPEC-21 DAG (high-level)

Phase A predecessor amendments first (TASK-0510..0517 — non-blocking standalone), then foundation types (Phase B: TASK-0520 → 0521 → 0522/0523/0524), strategies (Phase C: 0530 / 0531 ← 0524), benchmark integration (Phase D: 0540 ← 0513+0521+0520; 0541/0542 ← 0540; 0544 ← 0521), accumulator + orchestrator (Phase E: 0550 ← 0486/0487 [SPEC-22]; 0551/0553 ← 0550; 0552 ← 0551+0490 [SPEC-22]; 0554 ← 0540+0524+0530+0552+0553+0523), and finally Phase F gap-fill (0565 ← 0512+0524+0530+0531; 0567 ← 0540+0554+0517+0565+0541+0542+0531; 0568 ← 0512+0565; 0575 ← 0511+0476; 0576 ← 0511+0476+0575; 0577 ← 0514+0511+0575+0576+0565+0554; 0578 ← 0514+0511+0575+0576; 0588 ← 0516+0553+0554+0577; 0589 ← 0515+0482+0578+0554; 0590 ← 0515+0482+0578+0589; 0591 ← 0515+0589+0590).

### SPEC-21 Bundle gates

- All 36 tasks shipped: status DONE.
- `cargo test --workspace` ≥ 1181 default / ≥ 1224 zero-copy (zero regression on v1 floor of 690).
- `cargo test --features streaming-no-recycle` (TASK-0591 column) passes ≥ 1181.
- New SPEC-21 tests live under `relativist-core/src/{partition,bench,io}/streaming.rs` test modules and `relativist-core/tests/spec21_*.rs` integration files; FSM tests under `relativist-net/tests/{coordinator,worker}_pull_*.rs`.
- Clippy + fmt clean across all feature configs.
- TASK-0567 R26 short-circuit + T6/T8 isomorphism oracle passes.
- TASK-0588 BorderGraph extension call-site discipline integration test passes (delta+streaming).
- TASK-0589 / TASK-0590 R10b strategy wiring tests pass; cross-strategy isomorphism preserved.

## Cross-Cutting: Test Strategy (SPEC-08 v3)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0215 | Implement property tests PB13-PB14 (T2/T3 invariant coverage) | P1 | TODO | Phase 1, Phase 2 | S |
| TASK-0216 | Implement property tests PB15-PB16 (Profile B/C targeted generators) | P1 | TODO | Phase 1, Phase 2, Phase 4, 0185 | M |
| TASK-0217 | Implement dedicated D2 local reduction equivalence test | P2 | TODO | Phase 1, Phase 2, Phase 3 | M |
