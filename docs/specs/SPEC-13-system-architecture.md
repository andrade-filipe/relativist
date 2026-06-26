# SPEC-13: System Architecture

**Status:** Revised v2.1 — §3.5/§3.6 FSMs amended per SPEC-21 §3.8 A5 (5 coordinator pull-mode states + 2 worker pull-mode states)
**Depends on:** SPEC-00 (Glossary), SPEC-01 (Invariants), SPEC-02 (Net Representation), SPEC-03 (Reduction Engine), SPEC-04 (Partitioning), SPEC-05 (Merge and Grid Cycle), SPEC-06 (Wire Protocol), SPEC-07 (Deployment), SPEC-08 (Test Strategy), SPEC-09 (Benchmarks)
**Amends:** SPEC-21 §3.8 A5 (§3.5 Coordinator FSM gains `DispatchingFirst`, `AwaitingResults`, `GeneratingNext`, `SendingNoMoreWork`, `AwaitingFinalResults`; §3.6 Worker FSM gains `AwaitingChunkAfterResult`, `FinalReduction`; SPEC-21 R30, R32, R37d, R37e)
**Gray zones resolved:** ---
**Research consumed:** PESQ-010 (Coordinator-Worker Patterns), PESQ-012 (MapReduce/Dataflow/BSP Comparison), PESQ-013 (State Machines for Distributed Protocols), PESQ-023 (Decision Matrix: D1-D8), PESQ-024 (Architecture Recommendations)
**Discussions consumed:** DISC-005 v2 (cross-boundary protocol, centralized merge), DISC-006 v2 (overhead anatomy, break-even), DISC-007 v2 (fault tolerance scope), DISC-008 v2 (shared-to-distributed transition)
**Arguments consumed:** ARG-001 (central argument, P1-P6), ARG-002 (partitioning preserves structure), ARG-003 (merge protocol guarantees frontier completeness), ARG-004 (practical viability and limits)

---

## 1. Purpose

This spec defines the system architecture of Relativist: the programming model classification (BSP), module boundaries and dependency rules, the coordinator and worker finite state machines, the transport abstraction, the async architecture, feature flags, data flow through the system, CLI design, error handling strategy, and explicit exclusions for v1. This is the integration spec that ties together all other specs (SPEC-01 through SPEC-09) into a coherent system design, resolving all 8 architecture decisions from PESQ-023 (D1-D8) and consuming the full architecture blueprint from PESQ-024.

## 2. Definitions

Terms defined in SPEC-00 (Glossary) are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **BSP (Bulk Synchronous Parallel)** | A parallel programming model where computation proceeds in supersteps: each superstep consists of a local computation phase, a communication phase, and a barrier synchronization. Relativist implements BSP where each grid round (SPEC-05) is one superstep. Identified as the correct classification in PESQ-012. |
| **Superstep** | One complete iteration of the BSP cycle in Relativist: split the net into partitions, dispatch to workers, reduce locally, collect results, merge partitions, check termination. Equivalent to one Round (SPEC-05). |
| **Coordinator** | The central process that orchestrates the grid cycle: partitions the net, dispatches partitions to workers, collects results, executes merge, resolves border redexes, and decides whether to continue or terminate. Runs as a tokio async event loop. |
| **Worker** | A remote process that receives a partition, reduces it locally via `reduce_all` (SPEC-03), and returns the reduced partition to the coordinator. Workers have no knowledge of each other and communicate only with the coordinator (star topology, PESQ-010 L6). |
| **Core Layer** | The set of modules (`net`, `reduction`, `partition`, `merge`, `encoding`) that contain pure, synchronous logic with no async runtime dependency, no I/O, and no network access. Core compiles and runs without tokio. |
| **Infrastructure Layer** | The set of modules (`protocol`, `coordinator`, `worker`) that depend on tokio for async I/O and network communication. Infrastructure depends on Core but never the reverse. |
| **Transport** | A trait abstracting the send/receive mechanism. `TcpTransport` for production; `ChannelTransport` (tokio mpsc) for in-memory testing. TLS wraps TCP transparently when the `tls` feature is enabled. |
| **Feature Flag** | A Cargo feature that enables optional functionality at compile time. Relativist uses feature flags for `tls`, `metrics`, and `otel` to keep the default binary lean. |
| **Stimulus-Response Pattern** | An FSM design where the state machine is a pure function `transition(state, event) -> (state, Vec<Action>)`. Events are stimuli; actions are responses. The async runtime dispatches events and executes actions, but the FSM logic itself is synchronous and testable without tokio (PESQ-013 L2). |
| **Star Topology** | Network topology where all workers connect directly to the coordinator, with no worker-to-worker communication. Sufficient for v1 (PESQ-010 L6, L7). |

---

## 3. Requirements

### 3.1 Programming Model (BSP)

**R1.** Relativist MUST implement the BSP programming model where each grid round is one BSP superstep consisting of the phases: split, dispatch, reduce (local), collect, merge, check termination. **(MUST)**

**R2.** Each BSP superstep MUST use barrier synchronization: the coordinator MUST wait for ALL workers to return their reduced partitions before proceeding to the merge phase. No worker's result from round `r` may be merged with results from round `r-1` or `r+1`. **(MUST)**

Rationale: Barrier synchronization is required by P3 of the formal argument (ARG-001): the merge assumes all partitions have been independently reduced. Partial merges would violate the split/merge identity (SPEC-01, D1) and could produce incorrect results.

**R3.** The grid loop MUST terminate when `border_redexes(merged_net) == 0` AND `local_redexes(merged_net) == 0`, i.e., the merged net is in Normal Form (SPEC-05, R10). **(MUST)**

**R4.** The BSP classification MUST be documented in the coordinator's module-level documentation (e.g., `//! Relativist uses the BSP programming model...`). **(MUST)**

**R4a.** Relativist MUST NOT implement MapReduce (lacks iterative rounds with structural merge) or Dataflow (wrong abstraction -- IC reduction is a single repeated operation, not a DAG of distinct operations). See also R49 (v1 exclusions). **(MUST NOT)**

Rationale: PESQ-012 provides exhaustive comparison. The BSP mapping is exact: superstep = grid round, local computation = reduce(partition), communication = ReturnPartition, barrier = coordinator waits for all workers.

### 3.2 Module Structure

**R5.** Relativist MUST be organized as a single crate with the following 11 modules. **(MUST)**

```
src/
├── lib.rs              # Re-exports, top-level error type
├── main.rs             # CLI entry point (clap)
├── net/                # SPEC-02: Net, Agent, Wire, Port, PortRef
│   ├── mod.rs
│   ├── agent.rs
│   ├── wire.rs
│   └── port.rs
├── reduction/          # SPEC-03: reduce(), redex detection, 6 rules
│   └── mod.rs
├── partition/          # SPEC-04: split(), PartitionStrategy, FreePort
│   └── mod.rs
├── merge/              # SPEC-05: merge(), border resolution
│   └── mod.rs
├── encoding/           # SPEC-14: Church numerals, arithmetic, readback
│   ├── mod.rs
│   ├── church.rs
│   └── arithmetic.rs
├── protocol/           # SPEC-06: Message, Transport trait, framing
│   ├── mod.rs
│   ├── message.rs
│   ├── transport.rs    # trait Transport
│   ├── tcp.rs          # TcpTransport
│   └── channel.rs      # ChannelTransport (testing)
├── coordinator/        # Coordinator FSM, round management
│   └── mod.rs
├── worker/             # Worker FSM, reduction loop
│   └── mod.rs
├── config/             # SPEC-07: CLI config, environment parsing
│   └── mod.rs
├── observability/      # Logging setup, metrics registry
│   └── mod.rs
└── security/           # TLS, token authentication
    └── mod.rs
```

**R6.** The Core Layer modules (`net`, `reduction`, `partition`, `merge`, `encoding`) MUST NOT depend on tokio, on any async runtime, or on any I/O crate. They MUST be pure synchronous Rust with no `async fn` signatures. **(MUST)**

**R7.** The Infrastructure Layer modules (`protocol`, `coordinator`, `worker`) MAY depend on tokio and other async/I/O crates. They MUST depend on the Core Layer for data types and algorithms. **(MUST for dependency direction)**

**R8.** The Core Layer MUST NOT depend on the Infrastructure Layer. The dependency direction MUST be: Infrastructure -> Core, never Core -> Infrastructure. **(MUST)**

Rationale: This separation enables (a) testing core logic without an async runtime, (b) potential future extraction into a `relativist-core` crate if the project grows, and (c) clear mental model of what is pure computation vs. what is I/O (PESQ-023 D7, PESQ-024 Section 3).

**R9.** The `observability` and `security` modules MUST be feature-gated where they introduce optional dependencies (`metrics`, `otel`, `tls` features). The always-on parts (basic `tracing` setup) MAY reside in `observability` without feature gates. **(MUST)**

**R10.** The crate MUST produce a single binary named `relativist`. **(MUST)**

### 3.3 Dependency Map

**R11.** The always-on dependencies MUST be limited to the following crates. **(MUST)**

| Crate | Version | Purpose | Justification |
|-------|---------|---------|--------------|
| `tokio` | 1.x | Async runtime | Universal standard (PESQ-009 L3) |
| `serde` | 1.x | Serialization framework | Universal standard (PESQ-009 L2) |
| `bincode` | 2.x | Binary encoding | Validated by 3/4 surveyed frameworks (PESQ-009 Section 3.2) |
| `clap` | 4.x | CLI parsing | Standard for Rust CLIs |
| `tracing` | 0.1 | Structured logging | Single instrumentation API (PESQ-015 L1) |
| `tracing-subscriber` | 0.3 | Log formatting + filtering | Required companion to `tracing` |
| `thiserror` | 2.x | Error type derivation | Typed errors over anyhow (PESQ-023 D2) |
| `rand` | 0.8 | Random number generation | Token generation, test data |

**R12.** Feature-gated dependencies MUST be declared with `dep:` syntax in `Cargo.toml` and MUST only be compiled when the corresponding feature is enabled. **(MUST)**

| Crate | Feature | Purpose |
|-------|---------|---------|
| `rustls` | `tls` | TLS 1.3 implementation (PESQ-017) |
| `tokio-rustls` | `tls` | Async TLS for tokio |
| `rustls-pemfile` | `tls` | PEM file parsing |
| `prometheus-client` | `metrics` | Prometheus metrics exposition (PESQ-016) |
| `axum` | `metrics` | HTTP server for /metrics, /health endpoints |
| `opentelemetry` | `otel` | OpenTelemetry core API (PESQ-014) |
| `opentelemetry-sdk` | `otel` | OpenTelemetry SDK |
| `opentelemetry-otlp` | `otel` | OTLP exporter |
| `tracing-opentelemetry` | `otel` | tracing-to-OTel bridge |

**R13.** Dev/test dependencies MUST include at least the following. **(MUST)**

| Crate | Purpose |
|-------|---------|
| `proptest` | Property-based testing (SPEC-08, PESQ-022) |
| `criterion` | Micro-benchmarks (SPEC-09) |
| `tokio-test` | Async test utilities |
| `rcgen` | Certificate generation for TLS tests |

**R14.** No additional always-on dependencies SHOULD be added without justification. The principle is: build on primitives, not frameworks (PESQ-009 L1). **(SHOULD)**

### 3.4 Error Handling

**R15.** Relativist MUST use `thiserror` for all error type definitions. Each module MUST define its own error enum. **(MUST)**

**R16.** The per-module error enums MUST be at minimum:

```rust
/// Errors from the net representation layer.
#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("agent {0} not found")]
    AgentNotFound(AgentId),
    #[error("port {0:?} is dangling (not connected)")]
    DanglingPort(PortRef),
    #[error("net invariant violated: {0}")]
    InvariantViolation(String),
}

/// Errors from the reduction engine.
#[derive(Debug, thiserror::Error)]
pub enum ReductionError {
    #[error("invalid redex: agents {0} and {1} are not connected via principal ports")]
    InvalidRedex(AgentId, AgentId),
    #[error("net invariant violated: {0}")]
    InvariantViolation(String),
}

/// Errors from the partitioning subsystem.
#[derive(Debug, thiserror::Error)]
pub enum PartitionError {
    #[error("cannot partition net with {agents} agents into {k} partitions")]
    TooFewAgents { agents: usize, k: usize },
    #[error("partition invariant violated: {0}")]
    InvariantViolation(String),
}

/// Errors from the merge subsystem.
#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    #[error("unresolved border: FreePort({0}) has no matching partner")]
    UnresolvedBorder(u32),
    #[error("merge invariant violated: {0}")]
    InvariantViolation(String),
}

/// Errors from the wire protocol and transport.
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("connection lost: {0}")]
    ConnectionLost(#[source] std::io::Error),
    #[error("invalid message: {0}")]
    InvalidMessage(String),
    #[error("message too large: {size} bytes (max {max})")]
    MessageTooLarge { size: usize, max: usize },
    #[error("frame checksum mismatch")]
    ChecksumMismatch,
    #[error("authentication failed")]
    AuthFailed,
    #[error("timeout after {0:?}")]
    Timeout(std::time::Duration),
}

/// Errors from the coordinator.
#[derive(Debug, thiserror::Error)]
pub enum CoordinatorError {
    #[error("worker {0} failed: {1}")]
    WorkerFailed(WorkerId, String),
    #[error("no workers registered within timeout")]
    NoWorkers,
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error(transparent)]
    Partition(#[from] PartitionError),
    #[error(transparent)]
    Merge(#[from] MergeError),
}

/// Errors from the worker.
#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error(transparent)]
    Reduction(#[from] ReductionError),
}
```

**(MUST for the enum structure; individual variants MAY be added or renamed during implementation)**

**R17.** A top-level error type `RelativistError` MUST unify all module errors via `#[from]` conversions. **(MUST)**

```rust
/// Top-level error type for the Relativist binary.
#[derive(Debug, thiserror::Error)]
pub enum RelativistError {
    #[error(transparent)]
    Net(#[from] NetError),
    #[error(transparent)]
    Reduction(#[from] ReductionError),
    #[error(transparent)]
    Partition(#[from] PartitionError),
    #[error(transparent)]
    Merge(#[from] MergeError),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error(transparent)]
    Coordinator(#[from] CoordinatorError),
    #[error(transparent)]
    Worker(#[from] WorkerError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("configuration error: {0}")]
    Config(String),
}
```

**R18.** Errors MUST be classified as either **Transient** (retryable: network timeout, temporary connection loss) or **Fatal** (non-retryable: invalid net structure, protocol violation, invariant violation). The coordinator FSM MUST handle transient errors with retry/re-dispatch and fatal errors with shutdown. **(MUST)**

### 3.5 Coordinator FSM

**R19.** The coordinator MUST implement a finite state machine with the following states. **(MUST)**

> **Note:** SPEC-13 R19-R22 supersede SPEC-06 R26-R28 for the coordinator FSM definition. SPEC-06's FSM was a behavioral specification using informal state names; SPEC-13 provides the concrete enum-based FSM that the implementation MUST use. Where names differ from SPEC-06 (e.g., `Distributing` -> `Dispatching`, `WaitingWorkers` -> `WaitingForWorkers`), SPEC-13 names are authoritative. SPEC-06's `Idle` state is replaced by the `CheckTermination` state, which explicitly checks whether the merged net is in Normal Form before deciding to continue or terminate. SPEC-06's `ShuttingDown` state is subsumed by the `Done` state's `ShutdownAll` action.

```rust
/// Coordinator states.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum CoordinatorState {
    /// Initial state. Loading configuration, binding TCP listener.
    Init,
    /// Waiting for the minimum number of workers to connect.
    WaitingForWorkers,
    /// Partitioning the net into sub-nets for distribution.
    Partitioning,
    /// Sending partitions to workers.
    Dispatching,
    /// Waiting for all workers to return their reduced partitions.
    WaitingForResults,
    /// Merging returned partitions and running reduce_all on the merged net
    /// to resolve border redexes and any derived redexes (SPEC-05 R15-R18).
    Merging,
    /// Checking if the merged-and-reduced net is in Normal Form.
    /// If yes -> Done. If no -> Partitioning (next round).
    CheckTermination,
    /// Reduction complete. Writing output, sending Shutdown to all workers.
    Done,
    /// Fatal error. Shutting down.
    Error,
}
```

**R20.** The coordinator FSM MUST follow the stimulus-response pattern: a pure function `transition(state, event) -> (new_state, Vec<Action>)` that takes the current state and an event, and returns the new state plus a list of side-effectful actions to execute. **(MUST)**

```rust
/// A handle to a connected worker, held by the coordinator.
pub struct WorkerHandle {
    /// The worker's unique identifier, assigned upon connection.
    pub id: WorkerId,
    /// The transport channel to this worker.
    pub transport: Box<dyn Transport>,
}

/// Identifier for a timer managed by the coordinator's async runtime.
type TimerId = u32;

/// Events that drive the coordinator FSM.
#[derive(Debug, Clone)]
pub enum CoordinatorEvent {
    ConfigLoaded,
    WorkerConnected(WorkerId),
    SplitComplete(Vec<Partition>),
    AllDispatched,
    PartitionReturned { worker_id: WorkerId, partition: Partition },
    /// Phase-level inactivity timeout: no PartitionResult received from this
    /// worker within the configured `collect_timeout` (SPEC-06 R30-R31).
    PhaseTimeout(WorkerId),
    /// Merge and post-merge reduce_all completed. `is_normal_form` indicates
    /// whether the merged-and-reduced net has an empty redex queue.
    MergeComplete { net: Net, is_normal_form: bool },
    FatalError(String),
}

/// Actions the coordinator runtime must execute.
/// These are side effects produced by the pure transition function.
/// The async event loop is responsible for executing them.
///
/// `InvokeSplit` and `InvokeMerge` are dispatched as actions (not called
/// inside the transition function) to preserve stimulus-response purity.
/// The runtime executes them (via `spawn_blocking` for large nets if needed)
/// and fires `SplitComplete` / `MergeComplete` events back into the FSM.
#[derive(Debug)]
pub enum CoordinatorAction {
    BindListener(SocketAddr),
    SendMessage(WorkerId, Message),
    /// Invoke split(net, k) as a (possibly blocking) action. Fires
    /// SplitComplete(partitions) when done.
    InvokeSplit { net: Net, num_workers: usize },
    /// Invoke merge(partitions) + reduce_all(merged_net) as a (possibly
    /// blocking) action. Fires MergeComplete { net, is_normal_form } when done.
    InvokeMergeAndReduce(Vec<Partition>),
    StartTimer(TimerId, Duration),
    CancelTimer(TimerId),
    LogTransition { from: CoordinatorState, to: CoordinatorState },
    WriteOutput(Net),
    ShutdownAll,
}
```

**R21.** The coordinator transition table MUST implement at minimum the following transitions. **(MUST)**

| From | Event | To | Actions |
|------|-------|----|---------|
| Init | ConfigLoaded | WaitingForWorkers | BindListener, LogTransition |
| WaitingForWorkers | WorkerConnected(id) [count < min] | WaitingForWorkers | LogTransition |
| WaitingForWorkers | WorkerConnected(id) [count >= min] | Partitioning | InvokeSplit, LogTransition |
| Partitioning | SplitComplete(parts) | Dispatching | LogTransition |
| Dispatching | AllDispatched | WaitingForResults | StartTimer(collect_timer), LogTransition |
| WaitingForResults | PartitionReturned(id, P) [not all] | WaitingForResults | --- |
| WaitingForResults | PartitionReturned(id, P) [all received] | Merging | CancelTimer, InvokeMergeAndReduce, LogTransition |
| WaitingForResults | PhaseTimeout(id) | Error | LogTransition, ShutdownAll |
| Merging | MergeComplete(net, _) | CheckTermination | LogTransition |
| CheckTermination | [is_normal_form == true] | Done | WriteOutput, ShutdownAll, LogTransition |
| CheckTermination | [is_normal_form == false] | Partitioning | InvokeSplit, LogTransition |
| Any | FatalError(e) | Error | LogTransition, ShutdownAll |

> **Note on worker registration:** Worker registration is implicit upon TCP connection acceptance, consistent with SPEC-06 R24 ("The coordinator MUST wait for all num_workers workers to connect"). There are no explicit `Register`/`RegisterAck` messages in the protocol. The coordinator assigns a `WorkerId` upon accepting a connection and fires `WorkerConnected(id)` into the FSM.

> **Note on Merging state:** The `Merging` state encompasses both the merge operation (SPEC-05 R1-R11) and the post-merge `reduce_all` invocation (SPEC-05 R15-R18). The `InvokeMergeAndReduce` action performs both steps as a single unit and reports `is_normal_form` based on whether the redex queue is empty after `reduce_all`.

**R22.** The coordinator FSM MUST be enum-based (not typestate). Typestate encoding would make serialization, logging, and testing harder for no benefit at this scale (PESQ-013 L1). **(MUST)**

**R23.** Every state transition MUST be logged at `INFO` level with the `from` and `to` states, the triggering event, and the round number (if applicable). This enables post-hoc debugging and is essential for the observability story (PESQ-013 L3). **(MUST)**

**R23a. Coordinator FSM — Pull-Mode States (Amendment A5 — SPEC-21 §3.8 A5 / R30, R32, R37d).** When the streaming pipeline (SPEC-21 §3.3 R17) is active under `DispatchMode::Pull` (`GridConfig.dispatch_mode == DispatchMode::Pull`, SPEC-21 R34 / SPEC-07 R11), the coordinator FSM MUST gain the following five additional states:

```rust
/// Pull-mode coordinator states (gated on DispatchMode::Pull).
/// These extend `CoordinatorState` (R19) when the streaming pipeline is active.
DispatchingFirst,        // cold start: sending the first chunk to each worker
AwaitingResults,         // waiting for `PartitionResult` from any worker
GeneratingNext,          // calling `make_net_stream::next` then `strategy.allocate_batch`
SendingNoMoreWork,       // generator stream exhausted: sending `NoMoreWork` to workers issuing `RequestWork`
AwaitingFinalResults,    // awaiting all post-`NoMoreWork` results, then merge
```

**Pull-mode coordinator transitions:**

| From | Event | To | Notes |
|------|-------|----|-------|
| `Init` | `ConfigLoaded` (with `dispatch_mode = Pull`) | `DispatchingFirst` | replaces `WaitingForWorkers → Partitioning → Dispatching` chain in pull mode |
| `DispatchingFirst` | first chunk sent to each worker | `AwaitingResults` | — |
| `AwaitingResults` | `RequestWork(worker_id)` event (stream not exhausted) | `GeneratingNext` | per SPEC-06 R3a |
| `AwaitingResults` | `RequestWork(worker_id)` event (stream exhausted) | `SendingNoMoreWork` | — |
| `GeneratingNext` | chunk-ready (after `strategy.allocate_batch`) | `AwaitingResults` | sends `AssignPartition` |
| `SendingNoMoreWork` | all `NoMoreWork` acks received | `AwaitingFinalResults` | per SPEC-06 R3a `NoMoreWork` variant |
| `AwaitingFinalResults` | all post-`NoMoreWork` results received | `Merging` | BSP barrier per SPEC-21 R37d |

**R23b. Worker FSM — Pull-Mode States (Amendment A5).** The worker FSM MUST gain the following two additional states under `DispatchMode::Pull`:

```rust
/// Pull-mode worker states (gated on DispatchMode::Pull).
/// These extend `WorkerState` (R24) when the streaming pipeline is active.
AwaitingChunkAfterResult,    // entered after sending `PartitionResult`, awaiting `AssignPartition` or `NoMoreWork`
FinalReduction,              // entered upon receiving `NoMoreWork`
```

**Pull-mode worker transitions:**

| From | Event | To | Notes |
|------|-------|----|-------|
| `Reducing` (chunk) | `ReductionComplete(P')` | `AwaitingChunkAfterResult` | also emits `RequestWork { worker_id }` per SPEC-06 R3a |
| `AwaitingChunkAfterResult` | `AssignPartition` | `Reducing` | next chunk |
| `AwaitingChunkAfterResult` | `NoMoreWork` | `FinalReduction` | enters terminal reduction phase |
| `FinalReduction` | reduction-done | `Returning` | sends `PartitionResult` (final) → `Done` |

**R23c. Push-Mode FSMs UNCHANGED (Amendment A5 / SPEC-21 R37e).** When `dispatch_mode == DispatchMode::Push` (or `Auto` resolves to Push), the push-mode FSMs (R19/R21 coordinator transitions; R24/R25 worker transitions) are UNCHANGED. The pull-only states above are gated on `DispatchMode::Pull` in `GridConfig` (SPEC-07 R11 / SPEC-05 §4.1 / SPEC-21 R34). Workers MUST NOT add defensive `NoMoreWork` handling to the push-mode transition table; coordinators MUST NOT emit `NoMoreWork` in push mode. This guarantee is critical for backward compatibility — without it, every existing test scenario would need re-validation. **(MUST)**

**R23d. BSP-Barrier Semantics under Pull Dispatch (Amendment A5 / SPEC-21 R37d).** Under pull dispatch, workers MAY emit `PartitionResult` individually as their chunks complete, but the coordinator MUST NOT begin `merge()` until `NoMoreWork` has been emitted to every worker AND all post-`NoMoreWork` final `PartitionResult` messages have been received (the `AwaitingFinalResults → Merging` transition in R23a). This reduces pull dispatch to a single logical BSP round regardless of wall-clock interleaving, preserving D6 (Protocol termination) and G1 across the pull/push mode split. **(MUST)**

> **Amendment A5 (SPEC-21 §3.8 A5 / R30, R32, R37d, R37e):** Closes SC-001 part 3 and SC-015. The new states are pull-only and gated on `DispatchMode::Pull`. Without this amendment, R30-R32 prose narrative could not be decomposed into FSM-level tasks during Stage 1 (TASK-SPLITTER). Cross-references SPEC-07 R11 (CLI-to-config mapping for `dispatch_mode`), SPEC-05 §4.1 GridConfig (`dispatch_mode` field), SPEC-06 R3a (wire-level variants `RequestWork`/`NoMoreWork`).

### 3.6 Worker FSM

**R24.** The worker MUST implement a finite state machine with the following states. **(MUST)**

```rust
/// Worker states.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum WorkerState {
    /// Initial state. Connecting to coordinator.
    Init,
    /// Connected and idle. Waiting for a partition.
    Idle,
    /// Reducing a partition locally.
    Reducing,
    /// Sending the reduced partition back to the coordinator.
    Returning,
    /// Fatal error.
    Error,
    /// Shutdown received. Exiting.
    Done,
}
```

**R25.** The worker FSM MUST implement at minimum the following transitions. **(MUST)**

| From | Event | To | Actions |
|------|-------|----|---------|
| Init | Connected | Idle | LogTransition |
| Idle | ReceivePartition(P) | Reducing | LogTransition |
| Reducing | ReductionComplete(P') | Returning | LogTransition |
| Returning | SendComplete | Idle | LogTransition |
| Idle | Shutdown | Done | CloseConnection, LogTransition |
| Reducing | ReductionError(e) | Error | SendMessage(Error(e)), LogTransition |
| Any | ConnectionLost | Error | LogTransition, ShutdownSelf |

> **Note on ConnectionLost:** The worker does NOT attempt to reconnect. Per SPEC-06 R25, the coordinator aborts the grid loop upon connection loss. Worker reconnection would be pointless since the coordinator has already shut down. Reconnection logic is a fault tolerance concern (Z5, out of scope for v1).

> **Note on worker registration:** Worker registration is implicit. The worker connects via TCP (with retry per SPEC-06 R23), and the coordinator accepts the connection. There is no explicit `Register` message; the TCP connection itself is the registration event.

**R26.** The worker MUST perform local reduction synchronously (blocking the current task) using `reduce_all` from the `reduction` module (SPEC-03). The worker's async runtime is used for receiving and sending messages, not for parallelizing reduction. **(MUST)**

Rationale: `reduce_all` is a CPU-bound sequential operation on the partition. Running it on a `tokio::task::spawn_blocking` context (or a dedicated thread) prevents blocking the async I/O loop. The reduction itself MUST NOT use async internally.

**R27.** Workers MUST have no knowledge of each other. All communication flows through the coordinator (star topology). No worker-to-worker messages exist in the protocol (PESQ-010 L7, SPEC-06). **(MUST)**

### 3.7 Transport Abstraction

**R28.** The protocol module MUST define a `Transport` trait abstracting the send/receive mechanism. **(MUST)**

```rust
/// Abstraction over the communication channel between coordinator and worker.
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    /// Send a message to the remote end.
    async fn send(&mut self, msg: &Message) -> Result<(), ProtocolError>;

    /// Receive a message from the remote end.
    async fn recv(&mut self) -> Result<Message, ProtocolError>;

    /// Close the connection gracefully.
    async fn close(&mut self) -> Result<(), ProtocolError>;
}
```

**R29.** Relativist MUST provide two Transport implementations. **(MUST)**

| Implementation | Use Case | Description |
|----------------|----------|-------------|
| `TcpTransport` | Production, TCP benchmarks | Length-prefixed bincode frames over TCP (SPEC-06). Used for `TcpLocalhost` and `TcpNetwork` modes. |
| `ChannelTransport` | Unit tests, integration tests | Uses `tokio::sync::mpsc` channels in-memory. No serialization overhead. Enables testing the full grid cycle without TCP. |

**R30.** When the `tls` feature is enabled, `TcpTransport` MUST transparently wrap the TCP stream with TLS 1.3 via `tokio-rustls`. The `Transport` trait interface MUST NOT change -- TLS is an implementation detail of `TcpTransport`, not a separate transport. **(MUST)**

**R31.** The `ChannelTransport` MUST implement the same `Transport` trait, enabling the coordinator and workers to be instantiated in the same process for integration testing (SPEC-08, PESQ-020 L1). This is the in-memory grid mode referenced in SPEC-09 as `Local` mode. **(MUST)**

### 3.8 Async Architecture

**R32.** The coordinator MUST run on a single tokio runtime. Its main loop MUST be an event loop that: (a) accepts new TCP connections, (b) receives messages from workers, (c) processes timer events, and (d) feeds events into the FSM transition function. **(MUST)**

**R33.** Each worker MUST run on its own tokio runtime (separate process). The worker's async loop MUST: (a) maintain a persistent connection to the coordinator, (b) receive partitions, (c) offload reduction to a blocking task (`tokio::task::spawn_blocking`), and (d) send results back. **(MUST)**

**R34.** The Core Layer (`net`, `reduction`, `partition`, `merge`) MUST be fully synchronous. No function in the Core Layer MUST be `async`. No Core Layer module MUST import `tokio`. **(MUST)**

**R35.** There MUST be no shared mutable state between workers. Each worker operates on its own partition independently. The coordinator is the sole point of state aggregation (via merge). **(MUST)**

Rationale: This is a direct consequence of the BSP model (R1-R2) and the star topology (R27). Shared mutable state would require synchronization primitives that are unnecessary in BSP and would violate the independence assumption of P2 (ARG-001).

**R36.** The coordinator SHOULD use `tokio::select!` to multiplex between connection accept, message receive, and timer events in its main loop. **(SHOULD)**

### 3.9 Feature Flags

**R37.** The `Cargo.toml` MUST define exactly the following features. **(MUST)**

```toml
[features]
default = []
tls = ["dep:rustls", "dep:tokio-rustls", "dep:rustls-pemfile"]
metrics = ["dep:prometheus-client", "dep:axum"]
otel = ["dep:opentelemetry", "dep:opentelemetry-sdk", "dep:opentelemetry-otlp", "dep:tracing-opentelemetry"]
full = ["tls", "metrics", "otel"]
```

**R38.** The `default` Cargo feature set MUST be empty (`default = []` in `Cargo.toml`). All always-on dependencies from R11 are unconditional and do not require feature gates. A plain `cargo build` MUST produce a functional binary with TCP communication, structured logging, and token authentication -- but without TLS, Prometheus metrics, or OpenTelemetry tracing. **(MUST)**

**R39.** Feature-gated code MUST use `#[cfg(feature = "...")]` attributes. Feature-gated modules SHOULD provide no-op stubs when the feature is disabled, so that calling code does not need extensive `#[cfg]` blocks. **(MUST for cfg; SHOULD for stubs)**

### 3.10 Data Flow

**R40.** The end-to-end data flow of a distributed reduction MUST follow this sequence. **(MUST)**

```
1. Input: Parse IC net from file (.bin / .json) -> Net
2. Coordinator::Init: Load config, bind TCP, wait for worker connections
3. Workers connect: Each worker establishes TCP connection (registration is
   implicit upon connection acceptance, consistent with SPEC-06 R24)
4. BSP Loop (repeated until Normal Form):
   a. split(net, k) -> [P1, P2, ..., Pk]         (SPEC-04)
   b. dispatch(Pi -> Worker_i) for all i           (SPEC-06)
   c. Workers: reduce_all(Pi) -> Pi'               (SPEC-03)
   d. Coordinator: collect all Pi'                  (SPEC-06)
   e. merge([P1', P2', ..., Pk']) -> net'           (SPEC-05 R1-R11)
   f. reduce_all(net') -> net''                     (SPEC-03, SPEC-05 R15-R18)
   g. Check: is net'' in Normal Form? (redex queue empty)
      - No:  net = net'', goto step 4a
      - Yes: proceed to step 5
5. Output: Write reduced net + metrics
6. Shutdown: Send Shutdown to all workers, close connections
```

> **Note:** Step 4f is critical for correctness. SPEC-05 R15 mandates that `reduce_all` is invoked on the merged net to resolve border redexes and any derived redexes (including CON-DUP cascades per R17). Without this step, border redexes would accumulate, violating D3 (Completeness of Border Redex Resolution = Premise P3 of ARG-001).

**R41.** The direct reduction mode (`relativist reduce`) MUST bypass the coordinator/worker/protocol/partitioning infrastructure entirely. It MUST call `reduce_all` (SPEC-03) directly on the parsed net. The result MUST be structurally identical (isomorphic) to running the grid with any number of workers (Fundamental Property, SPEC-01 G1). **(MUST)**

**R41a.** The in-memory grid mode (`relativist local`) MUST execute the full grid cycle in-process per SPEC-07 R18, using `ChannelTransport` (R31) instead of TCP. Its result MUST also be isomorphic to the `reduce` result (G1). **(MUST)**

**R42.** The input format for nets MUST be bincode (SPEC-02, SPEC-06). The system SHOULD also accept a human-readable JSON format for debugging and test fixture creation. **(MUST for bincode; SHOULD for JSON)**

### 3.11 CLI Design

**R43.** The CLI MUST use `clap` with the derive API and MUST provide at least the following subcommands. SPEC-13 adds `reduce`, `inspect`, and `compute` to the original 4 subcommands from SPEC-07 R1 (`coordinator`, `worker`, `local`, `generate`), without removing any. **(MUST)**

```rust
/// Relativist: Distributed Interaction Combinator Reduction Engine
#[derive(Debug, clap::Parser)]
#[command(name = "relativist", version, about)]
pub enum Cli {
    /// Run as coordinator: partition, dispatch, merge, check termination.
    Coordinator(CoordinatorArgs),
    /// Run as worker: connect to coordinator, reduce partitions.
    Worker(WorkerArgs),
    /// Run in-memory grid simulation (SPEC-07 R18, SPEC-05 run_grid).
    /// Executes the full BSP cycle in-process with ChannelTransport.
    /// Used for benchmarks (SPEC-09 Local mode) and integration tests.
    Local(LocalArgs),
    /// Run purely local reduction (no distribution, no partitioning).
    /// Calls reduce_all directly on the parsed net.
    /// Used for baseline comparison and verifying G1.
    Reduce(ReduceArgs),
    /// Inspect an IC net file (print summary statistics).
    Inspect(InspectArgs),
    /// Generate test IC nets.
    Generate(GenerateArgs),
    /// Encode arithmetic, reduce, decode result.
    Compute(ComputeArgs),
}
```

> **Note on `local` vs `reduce`:** These are semantically different operations. `local` runs the full grid cycle (split, reduce, merge, check) in-process with simulated workers (SPEC-07 R18), exercising the partitioning and merge code paths. `reduce` calls `reduce_all` directly without any partitioning or merge, serving as the sequential baseline. Both are needed: `local` for in-memory grid benchmarks (SPEC-09), `reduce` for the reference sequential result (G1 verification).

**R44.** The `coordinator` subcommand MUST accept at minimum: `--bind` (socket address, default `127.0.0.1:9000`), `--workers` (minimum worker count before starting), `--input` (path to net file), `--output` (path for result, optional), `--log-format` (text|json, optional, default TTY-dependent per SPEC-11 R3), `--metrics-port` (optional, default 9090, feature-gated on `metrics` per SPEC-11 R20), and optional TLS arguments (`--tls-cert`, `--tls-key`) when the `tls` feature is enabled. **(MUST)**

> **Note (2026-04-06):** `--log-format` and `--metrics-port` added to resolve SPEC-11 OQ-2. Consistent with SPEC-07 R3 and SPEC-11 R3/R20.

**R45.** The `worker` subcommand MUST accept at minimum: `--coordinator` (address of coordinator), `--token` (authentication token, optional), `--log-format` (text|json, optional, default TTY-dependent per SPEC-11 R3), and optional `--tls-ca` when the `tls` feature is enabled. **(MUST)**

> **Note (2026-04-06):** `--log-format` added to resolve SPEC-11 OQ-2. Workers do not expose a metrics endpoint (metrics are reported to the coordinator), so `--metrics-port` is coordinator-only.

**R45a.** The `local` subcommand MUST accept the arguments defined in SPEC-07 R5: `--workers`, `--net`, `--max-rounds`, `--output`, `--metrics`, `--strategy`. It MUST execute the full grid cycle in-process (SPEC-07 R18). **(MUST)**

**R46.** The `reduce` subcommand MUST accept at minimum: `--input` (path to net file) and `--output` (path for result, optional). It MUST perform purely local reduction (calling `reduce_all` directly) without any partitioning, merging, or network communication. **(MUST)**

**R47.** The `inspect` subcommand MUST accept a path to a net file and print: agent count, wire count, redex count, agent distribution by symbol (CON, DUP, ERA), and whether the net is in Normal Form. **(MUST)**

**R48.** The `generate` subcommand MUST accept a benchmark name and size, and produce a net file. This enables creating test inputs independently of the benchmark suite (SPEC-09). **(MUST)**

**R48a.** The `compute` subcommand MUST accept an arithmetic operation (add, mul, exp) and two operands, encode them as an IC net via the encoding module (SPEC-14), reduce the net, decode the result, and print a summary. See SPEC-14 (R22-R25) for the full `ComputeArgs` specification. **(MUST)**

### 3.12 What is NOT in v1

**R49.** The following features MUST NOT be implemented in v1. Each is explicitly excluded with justification. **(MUST NOT)**

| Feature | Justification | Source |
|---------|--------------|--------|
| Multi-crate workspace | Over-engineering for a single developer; module boundaries achieve the same goal | PESQ-023 D1 |
| Mutual TLS (mTLS) | PKI complexity; server-side TLS + token auth is sufficient | PESQ-017 L3 |
| Full deterministic simulation testing (Turmoil/MadSim) | Disproportionate integration effort for v1 scope | PESQ-021 L1 |
| Work stealing | Incompatible with BSP barrier synchronization | PESQ-011 L1 |
| Byzantine fault tolerance | Redundant computation not justified for research prototype | PESQ-019 L5 |
| Coordinator high availability (HA) | Single coordinator is a known limitation, acceptable for TCC scope | PESQ-010 Section 3.3 |
| Actor model (e.g., Actix) | Wrong abstraction for BSP; adds framework coupling | PESQ-008 L1 |
| Consensus protocols (Raft, Paxos) | Single coordinator makes election unnecessary | PESQ-009 Section 2.1 |
| Token rotation | Per-session token is sufficient for TCC evaluation | PESQ-018 L4 |
| Intra-worker parallelism (rayon) | Sequential per-worker reduction is simpler and sufficient | PESQ-011 L2 |
| Worker-to-worker communication | Star topology is sufficient; peer-to-peer adds complexity without clear benefit | PESQ-010 L6, L7 |

**R50.** The v1 exclusions SHOULD be documented in a `ROADMAP.md` file in the repository, noting which items are candidates for future work. **(SHOULD)**

### 3.13 Additional Requirements (from review)

**R51.** The implementer SHOULD use Rust's native async traits (stabilized in Rust 1.75+) for the `Transport` trait if they support `Box<dyn Transport>` dispatch. If native async traits do not support dynamic dispatch for the required use case, `async_trait` is acceptable. **(SHOULD)**

**R52.** Relativist SHOULD provide a `SerializingChannelTransport` that wraps `ChannelTransport` and performs a bincode serialize/deserialize round-trip on every `send`/`recv`. At minimum, one integration test per benchmark (SPEC-08) SHOULD use the serializing variant to verify that `Message` serialization is correct (SPEC-06 R14: `deserialize(serialize(msg)) == msg`). **(SHOULD)**

---

## 4. Design

### 4.1 System Diagram

```
┌──────────────────────────────────────────────────────────────────┐
│                           CLI (clap)                             │
│  coordinator | worker | local | reduce | inspect | generate | compute │
└──────────┬────────────────┬──────────────────────────────────────┘
           │                │
     ┌─────▼──────┐   ┌────▼─────┐
     │ Coordinator │   │  Worker   │
     │   (FSM)     │   │  (FSM)   │      Infrastructure
     └──┬──────┬───┘   └────┬─────┘      Layer (async)
        │      │            │
     ┌──▼──────▼────────────▼──┐
     │     protocol/            │
     │  Transport trait         │
     │  TcpTransport            │
     │  ChannelTransport        │
     └──────────┬───────────────┘
                │
┌───────────────▼────────────────────────────────────────┐
│                    CORE LAYER (sync, no I/O)           │
│                                                         │
│  ┌─────────┐  ┌───────────┐  ┌───────────┐  ┌───────┐  ┌──────────┐ │
│  │   net/   │  │reduction/ │  │partition/ │  │merge/ │  │encoding/ │ │
│  │ Agent    │  │ 6 rules   │  │ split()   │  │merge()│  │ Church   │ │
│  │ Wire     │  │reduce_all │  │ FreePort  │  │border │  │ arith    │ │
│  │ PortRef  │  │ redex Q   │  │ strategy  │  │resolve│  │ readback │ │
│  └─────────┘  └───────────┘  └───────────┘  └───────┘  └──────────┘ │
└─────────────────────────────────────────────────────────┘
```

### 4.2 Coordinator Event Loop

The coordinator's async runtime processes events from multiple sources:

```
async fn run_coordinator(config: CoordinatorConfig) -> Result<Net, CoordinatorError> {
    let mut state = CoordinatorState::Init;
    let mut workers: HashMap<WorkerId, WorkerHandle> = HashMap::new();
    let mut net: Net = parse_input(&config.input)?;
    let mut round: u32 = 0;

    // Feed initial event
    let (state, actions) = transition(state, CoordinatorEvent::ConfigLoaded);
    execute_actions(actions);  // actions may include InvokeSplit, InvokeMergeAndReduce

    loop {
        // Multiplex events from connections, timers, internal signals,
        // and completion of blocking tasks (split, merge+reduce_all)
        let event = tokio::select! {
            conn = listener.accept() => { /* wrap as WorkerConnected */ },
            msg = recv_from_any_worker(&workers) => { /* wrap as PartitionReturned */ },
            _ = timer.tick() => { /* wrap as PhaseTimeout */ },
            result = blocking_task_complete() => { /* wrap as SplitComplete or MergeComplete */ },
        };

        let (new_state, actions) = transition(state, event);
        info!(from = ?state, to = ?new_state, round, "state transition");
        state = new_state;
        execute_actions(actions);

        if matches!(state, CoordinatorState::Done | CoordinatorState::Error) {
            break;
        }
    }

    // Return final net or error
}
```

This is pseudocode illustrating the pattern. The actual implementation will differ in details but MUST follow this structure: an event loop feeding events into a pure transition function. Note that `split()` and `merge()` are CPU-bound Core Layer operations invoked as actions (`InvokeSplit`, `InvokeMergeAndReduce`), not called inside the transition function. The action executor dispatches them (via `spawn_blocking` for large nets) and fires completion events (`SplitComplete`, `MergeComplete`) back into the FSM. This preserves the purity of the transition function.

### 4.3 Worker Main Loop

```
async fn run_worker(config: WorkerConfig) -> Result<(), WorkerError> {
    // Connect to coordinator (with retry per SPEC-06 R23).
    // Registration is implicit: the TCP connection itself is the registration.
    let mut transport = TcpTransport::connect(&config.coordinator_addr).await?;

    loop {
        let msg = transport.recv().await?;
        match msg {
            Message::AssignPartition(partition) => {
                // Offload CPU-bound work to blocking thread pool
                let reduced = tokio::task::spawn_blocking(move || {
                    reduce_all(partition.into_net())
                }).await?;

                transport.send(&Message::PartitionResult {
                    partition: reduced.into_partition(),
                }).await?;
            }
            Message::Shutdown => break,
            _ => { /* unexpected message */ }
        }
    }

    transport.close().await?;
    Ok(())
}
```

### 4.4 Module Dependency Graph

```
main.rs ──> config/ ──> (clap)
    │
    ├──> coordinator/ ──> protocol/ ──> net/
    │         │              │           │
    │         ├──> partition/ ──> net/   │
    │         │                          │
    │         ├──> merge/ ──> net/       │
    │         │                          │
    │         └──> reduction/ ──> net/   │
    │                                    │
    ├──> worker/ ──> protocol/ ──> net/  │
    │         │                          │
    │         └──> reduction/ ──> net/   │
    │                                    │
    ├──> encoding/ ──> net/               │
    │                                    │
    ├──> observability/ ──> (tracing)    │
    │                                    │
    └──> security/ ──> (rustls)          │

Legend: ──> means "depends on"
Core modules (net, reduction, partition, merge, encoding) have NO reverse arrows to infrastructure.
```

### 4.5 In-Memory Grid Mode (Testing)

For integration testing without TCP:

```rust
/// Run a complete grid cycle in-memory using ChannelTransport.
/// This is the `Local` mode used in SPEC-09 benchmarks.
/// Signature aligned with SPEC-05 R25's `run_grid`.
pub async fn run_grid_local(
    net: Net,
    num_workers: u32,
    strategy: impl PartitionStrategy,
) -> Result<(Net, GridMetrics), RelativistError> {
    // Create channel pairs for each worker
    let mut channels: Vec<(ChannelTransport, ChannelTransport)> = (0..num_workers)
        .map(|_| ChannelTransport::pair())
        .collect();

    // Spawn coordinator and workers as tokio tasks
    // Coordinator uses one end of each channel pair
    // Workers use the other end
    // Full BSP loop runs in-process with zero serialization overhead
}
```

This enables SPEC-08 integration tests and SPEC-09 Local-mode benchmarks without requiring TCP sockets.

---

## 5. Rationale

### 5.1 Why BSP and not MapReduce or Dataflow

PESQ-012 evaluates three programming models for Relativist. BSP is the correct classification because:

1. **Iteration:** IC reduction requires multiple rounds when border redexes emerge (SPEC-05). MapReduce is inherently single-pass; adapting it to iterative reduction requires an external loop that is effectively BSP.
2. **Barrier:** The merge operation (SPEC-05) requires ALL partitions to be available. This is a natural BSP barrier. Dataflow models allow partial progress, which would violate the merge assumption.
3. **Structural merge:** The merge is not a simple reduce (key-value aggregation). It is a graph reconstruction that reconnects border ports. MapReduce's reduce function cannot express this.
4. **Exact mapping:** Superstep = Round, local computation = reduce_all, communication = ReturnPartition, barrier = collect-all. No conceptual stretching required.

### 5.2 Why a single crate with feature flags

PESQ-023 D1 evaluates three options. A multi-crate workspace (2-3 crates) provides stronger compile-time boundaries but adds boilerplate and cross-crate refactoring friction for a single-developer research project. The single-crate approach with module-level boundaries achieves the same conceptual separation (Core vs. Infrastructure) while remaining easy to navigate. If Relativist grows beyond TCC scope, the module boundaries make extraction into separate crates straightforward. Paladin (PESQ-007) validates this approach at production scale.

### 5.3 Why thiserror over anyhow

PESQ-023 D2 analysis: Relativist needs to distinguish between transient and fatal errors in the coordinator FSM (R18). The coordinator must decide: retry (transient) or shutdown (fatal). `anyhow::Error` erases type information, making this classification impossible at the call site. `thiserror` preserves typed variants that can be matched. The additional boilerplate is justified by correctness of error handling.

### 5.4 Why enum-based FSM over typestate

PESQ-013 evaluates both patterns. Typestate encoding (one struct per state, consuming `self` on transition) provides compile-time guarantees but makes serialization, logging, and dynamic dispatch harder. The coordinator FSM needs to: (a) serialize its state for observability, (b) log transitions with `from`/`to` as strings, (c) store state in a single field that changes at runtime. All three are natural with enums and unnatural with typestate. For a 9-state FSM in a research project, the type-safety benefit of typestate does not justify the ergonomic cost.

### 5.5 Why stimulus-response pattern for the coordinator

The stimulus-response pattern (PESQ-013 L2) separates the FSM logic (pure function) from the I/O runtime (async event loop). This enables:

1. **Deterministic testing:** The transition function can be tested with unit tests that feed events and assert on (state, actions) without any async runtime (PESQ-013 L5).
2. **Clear audit trail:** Every transition is driven by a named event, enabling structured logging.
3. **Simpler reasoning:** Side effects are concentrated in the action executor, not scattered through the FSM logic.

### 5.6 Why star topology

PESQ-010 L6 and L7: Relativist's BSP model requires centralized merge. The coordinator must receive ALL partitions. Worker-to-worker communication would only be useful for direct border redex resolution, which DISC-005 v2 and ARG-003 rejected in favor of centralized merge. Star topology is the simplest topology that supports the protocol and adds no unnecessary complexity.

---

## 6. Haskell Prototype Reference

### 6.1 Architecture comparison

| Aspect | Haskell Prototype | Relativist |
|--------|-------------------|-----------|
| Programming model | Implicit BSP (not named) | Explicit BSP (documented, classified) |
| Coordinator FSM | Implicit in `gridLoop` control flow | Explicit enum-based FSM with transition table |
| Worker FSM | Implicit in `workerLoop` pattern match | Explicit enum-based FSM |
| Module boundaries | 6 Haskell modules (Core, Partition, Protocol, Network, Grid, TreeMapReduce) | 10 Rust modules with Core/Infrastructure split |
| Error handling | Haskell exceptions + `Either` | thiserror enums with classification |
| Transport abstraction | None (TCP hardcoded in Protocol.hs) | `trait Transport` with TCP and Channel impls |
| Feature flags | None | tls, metrics, otel |
| CLI | Basic argument parsing | clap with 7 subcommands |
| Async model | GHC threads + forkIO | tokio + spawn_blocking |
| Observability | printf debugging | tracing + optional Prometheus + optional OTel |

### 6.2 What the prototype got right

1. **Centralized merge.** The coordinator-centric architecture proven correct in the prototype maps directly to Relativist's design.
2. **Persistent connections.** TCP connections reused across rounds, avoiding reconnection overhead.
3. **Length-prefixed framing.** Simple, effective protocol that Relativist adopts (SPEC-06).
4. **Grid loop structure.** The split-dispatch-reduce-collect-merge-check cycle is preserved exactly.

### 6.3 What Relativist changes and why

1. **Explicit FSMs.** The prototype's control flow is implicit in nested pattern matches. Relativist makes it explicit for testability and observability.
2. **Transport abstraction.** The prototype hardcodes TCP. Relativist abstracts it for testing (ChannelTransport) and extensibility.
3. **Core/Infrastructure split.** The prototype mixes I/O throughout (e.g., `traceIO` in pure functions). Relativist enforces a clean boundary.
4. **Feature-gated optional components.** The prototype has no concept of optional features. Relativist allows TLS, metrics, and tracing to be enabled independently.
5. **Structured error handling.** The prototype uses `error "..."` strings. Relativist uses typed error enums.

---

## 7. Open Questions

1. ~~**async_trait vs native async traits.**~~ **RESOLVED (promoted to R51).** See Section 3.13.

2. **Coordinator backpressure.** If a worker returns a very large partition while the coordinator is still merging a previous round's results, the coordinator's receive buffer may grow. The coordinator SHOULD implement backpressure (e.g., limit the number of in-flight rounds to 1), but the exact mechanism is left to the implementer. **(Does NOT block implementation; BSP naturally limits this to 1 round.)**

3. **Graceful shutdown on Ctrl+C.** The coordinator and worker SHOULD handle `SIGINT`/`SIGTERM` for graceful shutdown (closing connections, flushing logs). The exact signal handling mechanism (e.g., `tokio::signal`) is left to the implementer. **(Does NOT block implementation.)**

4. **Config file support.** R43-R48 define CLI arguments. A TOML/YAML config file MAY be added as an alternative to long CLI argument lists, especially for multi-machine deployments. **(Does NOT block implementation; CLI-first, config-file later.)**

5. ~~**ChannelTransport serialization option.**~~ **RESOLVED (promoted to R52).** See Section 3.13.
