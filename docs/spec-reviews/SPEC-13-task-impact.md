# SPEC-13 Task Impact Report

**Date:** 2026-04-05
**Spec version:** Draft v1 -> Revised v2
**Reviewer:** Task Updater

---

## 1. Summary Table

| Category | Count | Task IDs |
|----------|-------|----------|
| **Updated** | 8 | TASK-0100, TASK-0107, TASK-0108, TASK-0109, TASK-0110, TASK-0111, TASK-0116, TASK-0149 |
| **Created** | 1 | TASK-0212 |
| **Obsoleted** | 0 | --- |
| **Unchanged** | 12 | TASK-0084, TASK-0101, TASK-0102, TASK-0103, TASK-0104, TASK-0112, TASK-0113, TASK-0114, TASK-0115, TASK-0117, TASK-0118, TASK-0119, TASK-0161, TASK-0178 |

---

## 2. Details for Each Updated Task

### TASK-0100: Refactor CLI to use Args structs
**What changed:** CLI `Command` enum now has 7 variants (was 4). Added `Reduce(ReduceArgs)`, `Inspect(InspectArgs)`, `Compute(ComputeArgs)`.
**Why:** SC-005 clarified that `local` and `reduce` are semantically different operations. SPEC-13 R43 now adds 3 subcommands to SPEC-07's original 4. R45a added for `local` subcommand arguments.
**Spec change:** Section 3.11 (R43, R45a)

### TASK-0107: Define CoordinatorState enum and FSM types
**What changed:** Multiple type signature changes:
- `CoordinatorEvent::WorkerRegistered(WorkerId)` -> `WorkerConnected(WorkerId)` (SC-002: implicit registration)
- `CoordinatorEvent::HeartbeatTimeout(WorkerId)` -> `PhaseTimeout(WorkerId)` (SC-012: phase-level timeout terminology)
- `CoordinatorEvent::MergeComplete { net, border_redex_count: usize }` -> `MergeComplete { net, is_normal_form: bool }` (SC-004: termination condition)
- Added `InvokeSplit { net, num_workers }` and `InvokeMergeAndReduce(Vec<Partition>)` to `CoordinatorAction` (SC-014: stimulus-response purity)
- Removed `EmitMetric(MetricEvent)` from `CoordinatorAction` (SC-008: metrics are observability concern)
- Added `WorkerHandle` struct and `type TimerId = u32` definitions (SC-008: previously undefined types)
**Why:** SC-002, SC-004, SC-008, SC-012, SC-014 from adversarial review.
**Spec change:** Section 3.5 (R19, R20)

### TASK-0108: Implement coordinator FSM transition function
**What changed:** Complete rewrite of transition table:
- `WorkerRegistered` -> `WorkerConnected`, removed `SendMessage(id, RegisterAck)` action
- `HeartbeatTimeout` -> `PhaseTimeout`
- Added `InvokeSplit` action to WaitingForWorkers->Partitioning and CheckTermination->Partitioning transitions
- Added `InvokeMergeAndReduce` action to WaitingForResults->Merging transition
- Added `Merging -> MergeComplete -> CheckTermination` transition (SC-007)
- Added `CheckTermination -> Done` and `CheckTermination -> Partitioning` transitions based on `is_normal_form` (SC-004, SC-007)
- Removed direct `Merging -> Done` / `Merging -> Partitioning` transitions (replaced by CheckTermination intermediate state)
- Timer name changed from `round_timer` to `collect_timer`
**Why:** SC-002 (implicit registration), SC-003 (reduce_all after merge), SC-004 (is_normal_form), SC-007 (CheckTermination state), SC-012 (PhaseTimeout), SC-014 (InvokeSplit/InvokeMergeAndReduce actions).
**Spec change:** Section 3.5 (R20, R21)

### TASK-0109: Define WorkerState enum and FSM types
**What changed:** `WorkerAction::AttemptReconnect` removed and replaced with `ShutdownSelf`.
**Why:** SC-006: worker must NOT attempt reconnection on ConnectionLost because the coordinator has already aborted per SPEC-06 R25. Reconnection is fault tolerance (Z5, out of scope for v1).
**Spec change:** Section 3.6 (R25)

### TASK-0110: Implement worker FSM transition function
**What changed:** Two transition table changes:
- `Init | Connected | Idle` action changed from `SendMessage(Register)` to `LogTransition` (SC-002: implicit registration)
- `Any | ConnectionLost` target changed from `Init` to `Error`, action changed from `AttemptReconnect, LogTransition` to `LogTransition, ShutdownSelf` (SC-006: no reconnection)
**Why:** SC-002 (no Register message), SC-006 (ConnectionLost -> Error, not Init).
**Spec change:** Section 3.6 (R25)

### TASK-0111: Implement run_local_command (local mode entry point)
**What changed:** Requirement reference updated from `R41 (SPEC-13)` to `R41a (SPEC-13)`.
**Why:** SC-005 split R41 into two requirements: R41 (direct reduction mode, `relativist reduce`) and R41a (in-memory grid mode, `relativist local`). TASK-0111 implements the local mode, which maps to R41a.
**Spec change:** Section 3.10 (R41, R41a)

### TASK-0116: Wire main.rs entrypoint with tokio and exit codes
**What changed:** Must now dispatch 7 subcommands (was 4). Added `Command::Reduce`, `Command::Inspect`, `Command::Compute` match arms.
**Why:** SC-005: SPEC-13 R43 now specifies 7 subcommands. All must be wired in `main.rs`.
**Spec change:** Section 3.11 (R43)

### TASK-0149: Add FSM state transition logging
**What changed:** Corrected coordinator and worker FSM state names to match Revised v2. Coordinator events now reference `WorkerConnected` and `PhaseTimeout` (not `WorkerRegistered` and `HeartbeatTimeout`).
**Why:** SC-002 (WorkerConnected), SC-012 (PhaseTimeout), SC-007 (CheckTermination state enumerated in coordinator FSM).
**Spec change:** Section 3.5 (R20), Section 3.6 (R25)

---

## 3. Details for Each New Task

### TASK-0212: Implement SerializingChannelTransport
**Covers:** R52 (SPEC-13, SHOULD)
**Phase:** Phase 5 (Wire Protocol)
**Priority:** P1
**Why:** SC-009 identified that `ChannelTransport` bypasses serialization, undermining integration test fidelity. R52 was promoted from OQ-5 to require a `SerializingChannelTransport` that performs a bincode round-trip on every send/recv. At minimum, one integration test per benchmark should use the serializing variant.

---

## 4. Unchanged Tasks -- Rationale

| Task ID | Why unchanged |
|---------|---------------|
| TASK-0084 | SPEC-06 types (NodeConfig, NodeRole). Not affected by SPEC-13 revision. |
| TASK-0101 | Tracing init. R9 (SPEC-13) unchanged. |
| TASK-0102 | CLI-to-config mapping. R43 (SPEC-13) changed subcommand count, but the mapping functions for coordinator/worker/local/generate are unchanged. New subcommand mapping is handled by Phase 9/11 handler tasks. |
| TASK-0103 | RelativistError type. R15-R18 unchanged. |
| TASK-0104 | Net serialization helpers. SPEC-07 requirements unchanged. |
| TASK-0112 | Coordinator entry point. R32, R36 unchanged. Depends on TASK-0108 (updated), but its own wiring is unaffected. |
| TASK-0113 | Worker entry point. R26, R33 unchanged. |
| TASK-0114 | Generate entry point. SPEC-07 requirements unchanged. |
| TASK-0115 | Cargo.toml alignment. R11, R12, R37, R38 changes were cosmetic (R38 rephrased but meaning unchanged). |
| TASK-0117 | Core/Infrastructure boundary. R6-R8 unchanged. |
| TASK-0118 | Feature-gated stubs. R9, R37-R39 unchanged in substance. |
| TASK-0119 | CLI integration test. R41 -> R41a affects TASK-0111 (updated), but the test itself tests the same local mode pipeline. |
| TASK-0161 | I/O types (SPEC-12). References SPEC-12's R52, not SPEC-13's R52. Different specs, no conflict. |
| TASK-0178 | CLI argument structs for I/O subcommands (SPEC-12). Defines ReduceArgs and InspectArgs. Already aligned with SPEC-13 R43's subcommand design. |

---

## 5. Requirement Coverage Verification (SPEC-13 MUST requirements)

| Requirement | Description | Covered by Task(s) | Notes |
|-------------|-------------|---------------------|-------|
| R1 | BSP programming model | TASK-0069, TASK-0070, TASK-0071 | Phase 4 (grid cycle) |
| R2 | Barrier synchronization | TASK-0069, TASK-0090 | Phase 4 + Phase 5 |
| R3 | Termination condition | TASK-0069, TASK-0108 | Phase 4 + Phase 6 |
| R4 | BSP documentation | TASK-0107 | Module doc comment |
| R4a | MUST NOT MapReduce/Dataflow | Architectural constraint | Negative requirement, enforced by design |
| R5 | Module structure (11 modules) | TASK-0001, TASK-0020, TASK-0040, TASK-0060, TASK-0080, TASK-0107, TASK-0109, TASK-0117 | Each phase scaffolds its module |
| R6 | Core Layer: no tokio | TASK-0117 | Audit task |
| R7 | Infrastructure -> Core dependency | TASK-0117 | Audit task |
| R8 | Core MUST NOT depend on Infra | TASK-0117 | Audit task |
| R9 | Feature-gated observability/security | TASK-0118 | Phase 6 |
| R10 | Single binary named `relativist` | TASK-0116 | Phase 6 |
| R11 | Always-on dependencies | TASK-0115 | Phase 6 |
| R12 | Feature-gated dependencies with `dep:` | TASK-0115 | Phase 6 |
| R13 | Dev/test dependencies | TASK-0115 | Phase 6 |
| R14 | No unnecessary dependencies (SHOULD) | TASK-0115 | Phase 6 |
| R15 | thiserror for all error types | TASK-0103 | Phase 6 |
| R16 | Per-module error enums | TASK-0103 | Phase 6 |
| R17 | RelativistError top-level unifier | TASK-0103 | Phase 6 |
| R18 | Error classification (Transient/Fatal) | TASK-0103, TASK-0108 | Phase 6 |
| R19 | Coordinator FSM states (9 states) | TASK-0107 | Phase 6 |
| R20 | Coordinator stimulus-response pattern | TASK-0107, TASK-0108 | Phase 6 |
| R21 | Coordinator transition table | TASK-0108 | Phase 6 |
| R22 | Enum-based FSM | TASK-0107 | Phase 6 |
| R23 | Transition logging at INFO | TASK-0149 | Phase 8 |
| R24 | Worker FSM states (6 states) | TASK-0109 | Phase 6 |
| R25 | Worker transition table | TASK-0110 | Phase 6 |
| R26 | Worker synchronous reduction | TASK-0113 | Phase 6 |
| R27 | Workers: no peer knowledge | TASK-0109 | Phase 6 |
| R28 | Transport trait | Phase 5 tasks (TASK-0085, TASK-0086) | Phase 5 |
| R29 | Two Transport implementations | TASK-0095, Phase 5 | Phase 5 |
| R30 | TLS wraps TcpTransport | TASK-0130, TASK-0131, TASK-0132, TASK-0133 | Phase 7 |
| R31 | ChannelTransport same trait | TASK-0095 | Phase 5 |
| R32 | Coordinator tokio runtime | TASK-0112 | Phase 6 |
| R33 | Worker own tokio runtime | TASK-0113 | Phase 6 |
| R34 | Core Layer fully synchronous | TASK-0117 | Phase 6 |
| R35 | No shared mutable state | Architectural constraint | Enforced by BSP design |
| R36 | tokio::select! (SHOULD) | TASK-0112 | Phase 6 |
| R37 | Cargo.toml features | TASK-0115 | Phase 6 |
| R38 | default = [] | TASK-0115 | Phase 6 |
| R39 | Feature-gated code with #[cfg] | TASK-0118 | Phase 6 |
| R40 | End-to-end data flow | TASK-0069-0071, TASK-0092, TASK-0093 | Phase 4 + Phase 5 |
| R41 | Direct reduction mode (`reduce`) | TASK-0178 (ReduceArgs + run_reduce) | Phase 9 |
| R41a | In-memory grid mode (`local`) | TASK-0111 | Phase 6 |
| R42 | Input format: bincode MUST, JSON SHOULD | TASK-0104, TASK-0163, TASK-0167 | Phase 6 + Phase 9 |
| R43 | CLI with 7 subcommands | TASK-0100, TASK-0116 | Phase 6 |
| R44 | `coordinator` subcommand args | TASK-0100 | Phase 6 |
| R45 | `worker` subcommand args | TASK-0100 | Phase 6 |
| R45a | `local` subcommand args (SPEC-07 R5) | TASK-0100 | Phase 6 |
| R46 | `reduce` subcommand args | TASK-0178 | Phase 9 |
| R47 | `inspect` subcommand args | TASK-0178 | Phase 9 |
| R48 | `generate` subcommand args | TASK-0100, TASK-0114 | Phase 6 |
| R48a | `compute` subcommand args | TASK-0210 | Phase 11 |
| R49 | v1 exclusions | Architectural constraint | Negative requirements |
| R50 | ROADMAP.md (SHOULD) | Documentation task | Not a code task |
| R51 | Native async traits (SHOULD) | TASK-0115 (notes on async-trait) | Implementation decision |
| R52 | SerializingChannelTransport (SHOULD) | **TASK-0212** (NEW) | Phase 5 |

### Coverage gaps: None

All MUST requirements from SPEC-13 Revised v2 map to at least one task. SHOULD requirements (R14, R36, R50, R51, R52) are covered by existing tasks or the new TASK-0212. Negative requirements (R4a, R49) are architectural constraints enforced by design, not individual tasks.

---

## 6. Cross-Spec Consistency Notes

The SPEC-13 Revised v2 introduces terminology that supersedes parts of SPEC-06 (R26-R28) and extends SPEC-07 (R1). This is noted in the spec's supersession note (R19) and in the defender response's residual risks section. Task updates in this report use the SPEC-13 Revised v2 terminology (e.g., `WorkerConnected`, `PhaseTimeout`, `InvokeSplit`, `InvokeMergeAndReduce`, `is_normal_form`). If SPEC-06 or SPEC-07 tasks are later updated for consistency, those tasks should adopt the same terminology.
