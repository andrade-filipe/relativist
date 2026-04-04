# Relativist Implementation Backlog

**Last updated:** 2026-04-04
**Total tasks:** 80 (0 done, 0 in progress, 80 todo)

**Pipeline:** See `DEVELOPMENT-PIPELINE.md` for the 7-stage development process.

---

## Phase 1: Core Types (SPEC-02)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0001 | Convert net module to directory structure | P0 | TODO | none | S |
| TASK-0002 | Define Symbol enum | P0 | TODO | 0001 | S |
| TASK-0003 | Define AgentId and PortId type aliases | P0 | TODO | 0001 | S |
| TASK-0004 | Define PortRef enum | P0 | TODO | 0003 | S |
| TASK-0005 | Define Agent struct | P0 | TODO | 0002, 0003 | S |
| TASK-0006 | Define arity and total_ports functions | P0 | TODO | 0002 | S |
| TASK-0007 | Define PORTS_PER_SLOT, port_index, DISCONNECTED | P0 | TODO | 0003, 0004 | S |
| TASK-0008 | Define Net struct and constructors | P0 | TODO | 0004, 0005, 0007 | M |
| TASK-0009 | Implement create_agent | P0 | TODO | 0008, 0006 | S |
| TASK-0010 | Implement get_target and set_port helpers | P0 | TODO | 0008, 0007 | S |
| TASK-0011 | Implement connect | P0 | TODO | 0010 | S |
| TASK-0012 | Implement disconnect | P0 | TODO | 0010 | S |
| TASK-0013 | Implement remove_agent | P0 | TODO | 0009, 0012, 0006 | S |
| TASK-0014 | Implement is_reduced and is_valid_redex | P0 | TODO | 0010, 0008 | S |
| TASK-0015 | Implement debug assertions (I1, I2, I3) | P0 | TODO | 0010, 0009, 0006 | M |
| TASK-0016 | Define BorderMap type alias | P1 | TODO | 0004 | S |
| TASK-0017 | Add serde + bincode serialization | P1 | TODO | 0008 | S |
| TASK-0018 | Implement PartialEq for Net | P1 | TODO | 0008 | S |

## Phase 2: Reduction Engine (SPEC-03)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0020 | Scaffold reduction module structure | P0 | TODO | Phase 1 | S |
| TASK-0021 | Define Rule enum and dispatch table | P0 | TODO | 0020 | S |
| TASK-0022 | Implement normalize_pair function | P0 | TODO | 0020, 0021 | S |
| TASK-0023 | Implement interact_void (ERA-ERA) | P0 | TODO | 0020, Phase 1 | S |
| TASK-0024 | Implement interact_anni (CON-CON, DUP-DUP) | P0 | TODO | 0020, Phase 1 | M |
| TASK-0025 | Implement interact_eras (CON-ERA, DUP-ERA) | P0 | TODO | 0020, Phase 1 | S |
| TASK-0026 | Implement interact_comm (CON-DUP) | P0 | TODO | 0020, Phase 1 | M |
| TASK-0027 | Define StepResult and implement reduce_step | P0 | TODO | 0020-0026 | M |
| TASK-0028 | Define ReductionStats and implement reduce_all | P0 | TODO | 0027 | M |
| TASK-0029 | Implement reduce_n (budget-limited reduction) | P0 | TODO | 0027, 0028 | S |
| TASK-0030 | Wire up reduction module re-exports | P1 | TODO | 0020-0029 | S |

## Phase 3: Partitioning (SPEC-04)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0040 | Define WorkerId type and IdRange struct | P0 | TODO | none | S |
| TASK-0041 | Define Partition struct | P0 | TODO | 0040 | S |
| TASK-0042 | Define PartitionPlan struct | P0 | TODO | 0041 | S |
| TASK-0043 | Define PartitionStrategy trait | P0 | TODO | 0040 | S |
| TASK-0044 | Implement ContiguousIdStrategy | P0 | TODO | 0043 | M |
| TASK-0045 | Helper function max_freeport_id | P0 | TODO | none | S |
| TASK-0046 | Wire classification logic | P0 | TODO | 0045 | M |
| TASK-0047 | Compute static ID space ranges | P0 | TODO | 0040 | S |
| TASK-0048 | split() trivial case (n=1) | P0 | TODO | 0042 | S |
| TASK-0049 | split() general case orchestrator | P0 | TODO | 0048, 0044, 0046, 0047, 0050-0054 | M |
| TASK-0050 | Build sub-net for one partition | P0 | TODO | 0046 | M |
| TASK-0051 | Redex queue population for partitions | P0 | TODO | 0050 | S |
| TASK-0052 | FreePort index construction per partition | P0 | TODO | 0046 | S |
| TASK-0053 | Debug assertion for C1 | P1 | TODO | 0041 | S |
| TASK-0054 | Debug assertions for C2 and C3 | P1 | TODO | 0041, 0042 | M |
| TASK-0055 | FreePort index lazy reconstruction | P1 | TODO | 0041 | S |
| TASK-0056 | ID range exhaustion error handling | P2 | TODO | 0040 | S |

## Phase 4: Merge & Grid Cycle (SPEC-05)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0060 | Define GridMetrics struct | P0 | TODO | none | S |
| TASK-0061 | Define WorkerRoundStats struct | P1 | TODO | none | S |
| TASK-0062 | Define GridConfig struct | P0 | TODO | none | S |
| TASK-0063 | Implement rebuild_free_port_index | P0 | TODO | Phase 1 | M |
| TASK-0064 | Implement is_principal_pair helper | P0 | TODO | Phase 1 | S |
| TASK-0065 | Merge function - unite agents + internal connections | P0 | TODO | 0060, 0064 | M |
| TASK-0066 | Merge function - restore boundary connections | P0 | TODO | 0065, 0064 | M |
| TASK-0067 | Merge debug assertions | P1 | TODO | 0066 | S |
| TASK-0068 | Implement drain_stale_redexes | P0 | TODO | Phase 1 | S |
| TASK-0069 | run_grid skeleton with termination logic | P0 | TODO | 0060, 0062, 0068 | M |
| TASK-0070 | run_grid Phase 1 (split) + Phase 2 (local reduce) | P0 | TODO | 0069, 0063, 0061 | M |
| TASK-0071 | run_grid Phase 3 (merge + resolve borders + metrics) | P0 | TODO | 0070, 0066 | M |
| TASK-0072 | n==1 optimization in run_grid | P1 | TODO | 0069 | S |
| TASK-0073 | Implement count_live_agents helper | P0 | TODO | none | S |
| TASK-0074 | Integration test: split/merge identity (D1) | P0 | TODO | 0066 | M |
| TASK-0075 | Integration test: Fundamental Property G1 | P0 | TODO | 0071 | M |
| TASK-0076 | Merge module exports and wiring | P0 | TODO | 0060-0065 | S |

## Phase 5: Wire Protocol (SPEC-06)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| TASK-0080 | Convert protocol module to directory structure | P0 | TODO | none | S |
| TASK-0081 | Define ProtocolError enum | P0 | TODO | 0080 | S |
| TASK-0082 | Define Message enum | P0 | TODO | 0080 | S |
| TASK-0083 | Define FrameHeader struct and framing constants | P0 | TODO | 0080 | S |
| TASK-0084 | Define NodeConfig and NodeRole types | P0 | TODO | 0080, 0083 | S |
| TASK-0085 | Implement send_frame function | P0 | TODO | 0081, 0082, 0083 | M |
| TASK-0086 | Implement recv_frame function | P0 | TODO | 0081, 0082, 0083 | M |
| TASK-0087 | Implement connect_with_retry (exponential backoff) | P1 | TODO | 0081, 0084 | S |
| TASK-0088 | Implement coordinator worker-accept phase | P0 | TODO | 0081, 0084 | M |
| TASK-0089 | Implement coordinator distribute phase | P0 | TODO | 0085, 0082, 0084 | M |
| TASK-0090 | Implement coordinator collect phase | P0 | TODO | 0086, 0082, 0084 | M |
| TASK-0091 | Implement coordinator shutdown protocol | P1 | TODO | 0085, 0082 | S |
| TASK-0092 | Implement run_coordinator (distributed grid loop) | P0 | TODO | 0088-0091, 0094 | M |
| TASK-0093 | Implement run_worker (worker loop) | P0 | TODO | 0085-0087, 0082 | M |
| TASK-0094 | Implement GridMetrics network extensions | P1 | TODO | 0080 | S |
| TASK-0095 | Implement in-memory transport for testing | P1 | TODO | 0085, 0086 | S |
| TASK-0096 | Add protocol crate dependencies to Cargo.toml | P0 | TODO | none | S |

## Phase 6: CLI & Config (SPEC-07 + SPEC-13)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| -- | *Not yet decomposed* | -- | -- | -- | -- |

## Phase 7: Security (SPEC-10)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| -- | *Not yet decomposed* | -- | -- | -- | -- |

## Phase 8: Observability (SPEC-11)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| -- | *Not yet decomposed* | -- | -- | -- | -- |

## Phase 9: User I/O (SPEC-12)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| -- | *Not yet decomposed* | -- | -- | -- | -- |

## Phase 10: Benchmarks (SPEC-09)

| ID | Title | Priority | Status | Depends | Complexity |
|----|-------|----------|--------|---------|------------|
| -- | *Not yet decomposed* | -- | -- | -- | -- |
