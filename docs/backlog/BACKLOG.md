# Relativist Implementation Backlog

**Last updated:** 2026-04-07
**Total tasks:** 206 (140 done, 0 in progress, 65 todo, 1 obsoleted)

**Pipeline:** See `DEVELOPMENT-PIPELINE.md` for the 7-stage development process.

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

## Cross-Cutting: Test Strategy (SPEC-08 v3)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0215 | Implement property tests PB13-PB14 (T2/T3 invariant coverage) | P1 | TODO | Phase 1, Phase 2 | S |
| TASK-0216 | Implement property tests PB15-PB16 (Profile B/C targeted generators) | P1 | TODO | Phase 1, Phase 2, Phase 4, 0185 | M |
| TASK-0217 | Implement dedicated D2 local reduction equivalence test | P2 | TODO | Phase 1, Phase 2, Phase 3 | M |
