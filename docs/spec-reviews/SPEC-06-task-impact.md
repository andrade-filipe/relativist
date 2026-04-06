# SPEC-06 Task Impact Report

**Date:** 2026-04-05
**Trigger:** SPEC-06 revision from Revised v2 to Revised v3 (17 critic issues resolved)
**Spec review:** SPEC-06-round2-defender.md
**Tasks affected:** 5 updated, 0 created, 0 removed

---

## 1. Summary Table

| Task ID | Title | Action | Reason |
|---------|-------|--------|--------|
| TASK-0080 | Convert protocol module to directory structure | **UPDATED** | Removed stale NodeRole references in file descriptions (SC-005) |
| TASK-0081 | Define ProtocolError enum | **ALREADY UPDATED** | All v3 changes applied by prior agent: Io->ConnectionLost, PayloadTooLarge.declared->size, +AuthFailed, WorkerError/WorkerCountMismatch moved to CoordinatorError (SC-003, SC-004) |
| TASK-0082 | Define Message enum | **ALREADY UPDATED** | All v3 changes applied by prior agent: 7 variants, R2a, R5 append-only note (SC-001, SC-013) |
| TASK-0083 | Define FrameHeader struct and framing constants | **NO CHANGE** | No SPEC-06 v3 changes affect framing constants |
| TASK-0084 | Define NodeConfig type | **ALREADY UPDATED** | All v3 changes applied by prior agent: bind: SocketAddr, NodeRole removed, collect_timeout MUST (SC-005, SC-014) |
| TASK-0085 | Implement send_frame function | **ALREADY UPDATED** | All v3 changes applied: explicit bincode config R11, ConnectionLost (SC-016) |
| TASK-0086 | Implement recv_frame function | **UPDATED** | Fixed stale `bincode::deserialize` reference to use explicit config per R11 (SC-016) |
| TASK-0087 | Implement connect_with_retry | **UPDATED** | Fixed stale `ProtocolError::Io` reference in context paragraph (SC-004) |
| TASK-0088 | Implement coordinator worker-accept phase | **ALREADY UPDATED** | All v3 changes applied: R2a, R26 historical, SocketAddr, WorkerCountMismatch->CoordinatorError (SC-001, SC-002, SC-005) |
| TASK-0089 | Implement coordinator distribute phase | **UPDATED** | Fixed stale `ProtocolError::Io` in acceptance criteria (SC-004) |
| TASK-0090 | Implement coordinator collect phase | **ALREADY UPDATED** | All v3 changes applied: collect_timeout MUST, ConnectionLost, WorkerError->CoordinatorError (SC-004, SC-014) |
| TASK-0091 | Implement coordinator shutdown protocol | **ALREADY UPDATED** | All v3 changes applied: R26 historical, SPEC-13 state names (SC-002) |
| TASK-0092 | Implement run_coordinator | **ALREADY UPDATED** | All v3 changes applied: R2a, R26 historical (SC-001, SC-002) |
| TASK-0093 | Implement run_worker | **ALREADY UPDATED** | All v3 changes applied: R27 historical, 6-field WorkerRoundStats (SC-002, SC-006) |
| TASK-0094 | Implement GridMetrics network extensions | **NO CHANGE** | No SPEC-06 v3 changes affect metrics convenience methods |
| TASK-0095 | Implement in-memory transport for testing | **ALREADY UPDATED** | All v3 changes applied: 7 Message variants (SC-001) |
| TASK-0096 | Add protocol crate dependencies | **NO CHANGE** | No SPEC-06 v3 changes affect crate dependencies |
| TASK-0212 | Implement SerializingChannelTransport | **NO CHANGE** | Already references R14 correctly |
| BACKLOG.md | Backlog title for TASK-0084 | **UPDATED** | Removed "and NodeRole" from title (SC-005) |

### Cross-Phase Tasks Referencing SPEC-06

| Task ID | Title | Phase | Action | Reason |
|---------|-------|-------|--------|--------|
| TASK-0041 | Define Partition struct | 3 | **NO CHANGE** | References SPEC-06 only for serde derives context; no v3 impact |
| TASK-0061 | Define WorkerRoundStats struct | 4 | **NO CHANGE** | Already has 6-field definition; SPEC-06 v3 WorkerRoundStats change (SC-006) is consistent |
| TASK-0070 | run_grid Phase 1+2 | 4 | **NO CHANGE** | Already has 6-field WorkerRoundStats and timing instrumentation |
| TASK-0102 | CLI-to-config mapping | 6 | **ALREADY UPDATED** | NodeRole removed, SocketAddr, all defaults correct |
| TASK-0107 | Define CoordinatorState enum | 6 | **NO CHANGE** | SPEC-13 task; SPEC-06 R2a note already in TASK-0108 |
| TASK-0108 | Coordinator FSM transition function | 6 | **ALREADY UPDATED** | R2a tier-dependent registration note added |
| TASK-0109 | Define WorkerState enum | 6 | **NO CHANGE** | Already references SPEC-06 R25 correctly |
| TASK-0110 | Worker FSM transition function | 6 | **ALREADY UPDATED** | R2a tier-dependent registration note added |
| TASK-0112 | run_coordinator_command | 6 | **NO CHANGE** | Delegates to run_coordinator; no direct SPEC-06 v3 impact |
| TASK-0113 | run_worker_command | 6 | **NO CHANGE** | Delegates to run_worker; no direct SPEC-06 v3 impact |
| TASK-0115 | Align Cargo.toml with SPEC-13 | 6 | **NO CHANGE** | No SPEC-06 v3 changes affect Cargo.toml |
| TASK-0117 | Enforce Core/Infrastructure boundary | 6 | **NO CHANGE** | Layer boundary unchanged by SPEC-06 v3 |
| TASK-0125 | Define SecurityConfig struct | 7 | **NO CHANGE** | Already notes max_message_size delegation to SPEC-06 R9 |
| TASK-0127 | Extend Message enum with Register variants | 7 | **NO CHANGE** | Task is additive; SPEC-06 v3 now includes registration variants in canonical definition but TASK-0127 still needed for SPEC-10 payload struct details |
| TASK-0136 | Verify message size pre-validation | 7 | **NO CHANGE** | Already uses PayloadTooLarge (not MessageTooLarge), field `size` (not `declared`) |
| TASK-0138 | SecurityConfig builder from CLI | 7 | **NO CHANGE** | Already notes no --max-message-size flag |
| TASK-0139 | Security integration tests | 7 | **NO CHANGE** | Already references SPEC-06 R9 correctly for T5 |
| TASK-0151 | Define protocol metrics | 8 | **NO CHANGE** | Message type labels unaffected |
| TASK-0159 | Optional trace context | 8 | **NO CHANGE** | Message variant names already correct |
| TASK-0163 | Implement binary format load/save | 9 | **NO CHANGE** | References SPEC-06 only for context; binary format unaffected |
| TASK-0213 | ERROR-level logging | 8 | **NO CHANGE** | Protocol error names (checksum mismatch, message too large, connection lost) are descriptive, not enum variant names |

---

## 2. Details for Each Changed Task

### TASK-0080: Convert protocol module to directory structure

**Changes:**
- File description for `src/protocol/config.rs` updated: "placeholder for NodeConfig, NodeRole" -> "placeholder for NodeConfig (NodeRole removed in SPEC-06 v3)"
- Notes section updated: removed "NodeRole" from sub-module breakdown, added note about NodeRole removal (SC-005)

**Root cause:** SC-005 removed NodeRole from the protocol. The config sub-module no longer needs a NodeRole placeholder.

### TASK-0086: Implement recv_frame function

**Changes:**
- Acceptance criteria updated: `bincode::deserialize` changed to explicit bincode configuration `bincode::config::standard().with_little_endian().with_fixed_int_encoding()` per R11
- Pseudocode comment updated to note explicit config per R11

**Root cause:** SC-016 (R11 explicit bincode configuration). The send_frame task (TASK-0085) was already updated by the prior agent but recv_frame was missed. Both send and recv must use the same explicit configuration for wire format consistency.

### TASK-0087: Implement connect_with_retry (exponential backoff)

**Changes:**
- Context paragraph updated: `ProtocolError::Io` changed to `ProtocolError::ConnectionLost` with note about SPEC-06 v3 rename

**Root cause:** SC-004 renamed `Io` to `ConnectionLost`. The acceptance criteria were already updated by the prior agent but the context paragraph was missed.

### TASK-0089: Implement coordinator distribute phase

**Changes:**
- Acceptance criteria updated: `ProtocolError::Io` changed to `ProtocolError::ConnectionLost` with note about SPEC-06 v3 rename

**Root cause:** SC-004 renamed `Io` to `ConnectionLost`. The context and FSM references were already updated by the prior agent but one acceptance criterion still referenced the old name.

### BACKLOG.md

**Changes:**
- TASK-0084 title changed from "Define NodeConfig and NodeRole types" to "Define NodeConfig type"

**Root cause:** SC-005 removed NodeRole. The backlog title was stale.

---

## 3. Requirement Coverage Verification

All 17 SPEC-06 v3 changes (SC-001 through SC-017) are now reflected in the task backlog:

| SC | Requirement Change | Task(s) Affected |
|----|-------------------|-----------------|
| SC-001 | Message enum +3 registration variants | TASK-0082 (updated), TASK-0095 (updated), TASK-0127 (unchanged, additive) |
| SC-002 | R26-R28 demoted to historical | TASK-0088, TASK-0089, TASK-0090, TASK-0091, TASK-0092, TASK-0093 (all updated) |
| SC-003 | PayloadTooLarge naming | TASK-0081 (updated), TASK-0136 (already correct) |
| SC-004 | ProtocolError restructure (Io->ConnectionLost, +AuthFailed, -WorkerError/-WorkerCountMismatch) | TASK-0081, TASK-0085, TASK-0086, TASK-0087, TASK-0088, TASK-0089, TASK-0090 (all updated) |
| SC-005 | NodeConfig: host+port->bind SocketAddr, NodeRole removed | TASK-0080, TASK-0084, TASK-0087, TASK-0088, TASK-0102, BACKLOG.md (all updated) |
| SC-006 | WorkerRoundStats 6-field update | TASK-0093 (updated), TASK-0061, TASK-0070 (already correct) |
| SC-007 | Registration handshake in pseudocode | TASK-0088, TASK-0092 (updated) |
| SC-008 | R17 default bind address | TASK-0084 (updated) |
| SC-009 | R12 transitive serde derives | TASK-0082 (updated) |
| SC-010 | R25 cleanup on connection loss | TASK-0090 (updated) |
| SC-011 | R22 concurrent collection note | TASK-0090 (notes section, already correct) |
| SC-012 | R9 payload default rationale | Informational, no task impact |
| SC-013 | R5 append-only enum discipline | TASK-0082 (updated) |
| SC-014 | R30 collect_timeout SHOULD->MUST | TASK-0084, TASK-0090 (updated) |
| SC-015 | Shutdown bytes not in per-round metrics | Informational, no task impact |
| SC-016 | R11 explicit bincode config | TASK-0085, TASK-0086 (updated) |
| SC-017 | Transport trait reference | Informational, no task impact |

**Coverage:** All 17 changes from the SPEC-06 v3 revision are reflected in the task backlog. No orphan tasks or uncovered changes remain.

---

## 4. Changes NOT Made (and why)

| Item | Reason not changed |
|------|--------------------|
| TASK-0083 | Framing constants (header size, max payload default) are unchanged in v3 |
| TASK-0094 | Metric convenience methods are unaffected by protocol type changes |
| TASK-0096 | Crate dependencies are unaffected (bincode, crc32fast, tokio, serde, futures all unchanged) |
| TASK-0041 | Only references SPEC-06 for serde derive context; no v3 structural impact |
| TASK-0107 | SPEC-13 task; R2a note already in companion TASK-0108 |
| TASK-0109 | SPEC-13 task; already references SPEC-06 R25 correctly |
| TASK-0112, TASK-0113 | Wiring tasks that delegate to Phase 5 functions; no direct protocol type references |
| TASK-0125, TASK-0127, TASK-0136, TASK-0138, TASK-0139 | SPEC-10 tasks; already use correct v3 terminology or are unaffected |
| TASK-0151, TASK-0159, TASK-0163, TASK-0213 | Phase 8/9 tasks; references are descriptive/contextual, not structural |
