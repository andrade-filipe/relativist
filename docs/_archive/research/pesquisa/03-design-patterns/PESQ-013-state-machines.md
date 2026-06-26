---
pesq_id: PESQ-013
title: "State Machines in Distributed Systems"
category: System Design Patterns
date: 2026-03-26
status: Complete
cross_references:
  specs: [SPEC-06, SPEC-13]
  pesqs: [PESQ-004, PESQ-010]
  discs: [DISC-005, DISC-007]
---

# PESQ-013: State Machines in Distributed Systems

**Category:** System Design Patterns
**Status:** Complete
**Cross-references:**
- Specs: SPEC-06 (wire protocol), SPEC-13 (system architecture)
- PESQs: PESQ-004 (Dask stimulus-response FSM), PESQ-010 (coordinator-worker lifecycle)
- Discussions: DISC-005 v2 (cross-boundary protocol), DISC-007 v2 (fault tolerance)

---

## 1. Subject Overview

State machines (FSMs — Finite State Machines) are a fundamental tool for modeling distributed system behavior. They provide:
- **Explicit states:** Every possible system state is enumerated
- **Defined transitions:** Only valid state changes are possible
- **Verifiable behavior:** Can be model-checked for deadlocks, livelocks, safety/liveness properties

In Rust, the type system enables **typestate patterns** — encoding states as types so invalid transitions are caught at compile time.

---

## 2. State Machines in Distributed Systems

### 2.1 Where FSMs Apply

| Component | States Model | Example |
|-----------|-------------|---------|
| **Protocol** | Message sequence / handshake | TCP states (SYN, ESTABLISHED, FIN_WAIT) |
| **Node lifecycle** | Coordinator/worker lifecycle | Dask scheduler states (PESQ-004) |
| **Task lifecycle** | Individual task progress | Dask task states (waiting → processing → memory) |
| **Consensus** | Agreement protocol phases | Raft (Follower → Candidate → Leader) |
| **Connection** | Per-connection state | TLS handshake states |

### 2.2 Dask's Stimulus-Response Model (PESQ-004)

Dask's scheduler uses a sophisticated FSM:
- **Task states:** released → waiting → queued → processing → memory/erred/forgotten
- **Transitions triggered by stimuli:** task submitted, dependency resolved, worker reports complete
- Each transition may produce **recommendations** (further stimuli)
- The scheduler processes stimuli in a loop until quiescent

This pattern is powerful because:
1. All state changes are explicit and traceable
2. Each transition has clear preconditions
3. The system is testable (inject stimulus → check resulting state)

---

## 3. Relativist's State Machines

### 3.1 Coordinator FSM

The coordinator manages the entire grid cycle. Its states:

```
              Register
[Init] ────────────────→ [WaitingForWorkers]
                              │
                         min_workers reached
                              │
                              ▼
                        [Partitioning]
                              │
                         split(net, k)
                              │
                              ▼
                        [Dispatching]
                              │
                         all partitions sent
                              │
                              ▼
                        [WaitingForResults]
                              │
          ┌───────────────────┼──────────────────┐
          │                   │                   │
     all returned      timeout (some)        worker failed
          │                   │                   │
          ▼                   ▼                   ▼
      [Merging]        [HandleStraggler]    [HandleFailure]
          │                   │                   │
          │                   └───────────────────┘
          │                          │
          │                     re-dispatch
          │                          │
          ▼                          ▼
    [CheckTermination]        [WaitingForResults]
          │
    ┌─────┴─────┐
    │           │
  done      more rounds
    │           │
    ▼           ▼
  [Done]   [Partitioning]
```

**States:**

| State | Description | Entry Action |
|-------|-------------|-------------|
| `Init` | Startup, loading config | Parse CLI, bind TCP |
| `WaitingForWorkers` | Accepting registrations | Listen for `Register` messages |
| `Partitioning` | Splitting net into partitions | `split(net, k)` |
| `Dispatching` | Sending partitions to workers | Send `DispatchPartition` to each |
| `WaitingForResults` | Waiting for all workers to return | Start round timer |
| `Merging` | Combining returned partitions | `merge(partitions)` |
| `CheckTermination` | Evaluating if reduction is complete | Check `border_redexes == 0` |
| `HandleStraggler` | Dealing with slow workers | Re-dispatch or wait |
| `HandleFailure` | Dealing with crashed workers | Remove worker, re-dispatch |
| `Done` | Computation complete | Write output, report metrics |

### 3.2 Worker FSM

```
[Init] → Register → [Idle] → ReceivePartition → [Reducing]
                       ↑                              │
                       │                         reduce(partition)
                       │                              │
                       │                              ▼
                       └──── ReturnPartition ── [Returning]

[Idle] → Shutdown → [Done]
[Reducing] → Error → [Error] → report → [Idle]
```

**States:**

| State | Description | Entry Action |
|-------|-------------|-------------|
| `Init` | Startup, connecting to coordinator | TCP connect, send `Register` |
| `Idle` | Waiting for work | Send periodic `Heartbeat` |
| `Reducing` | Executing reduction on partition | `reduce(partition)` |
| `Returning` | Sending results back | Send `ReturnPartition` |
| `Error` | Reduction failed | Send `Error` to coordinator |
| `Done` | Graceful shutdown | Close connection |

### 3.3 Connection FSM

Per-worker connection on the coordinator side:

```
[Connecting] → handshake → [Connected] → heartbeat timeout → [Suspect]
                                                                 │
                                             ┌───────────────────┤
                                             │                   │
                                        heartbeat resumes    3× timeout
                                             │                   │
                                             ▼                   ▼
                                        [Connected]          [Failed]
```

---

## 4. Rust Implementation Patterns

### 4.1 Enum-Based FSM (Recommended for Relativist)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
enum CoordinatorState {
    Init,
    WaitingForWorkers { registered: Vec<WorkerId> },
    Partitioning,
    Dispatching { sent: usize, total: usize },
    WaitingForResults { received: usize, total: usize },
    Merging,
    CheckTermination { border_redexes: usize },
    Done { rounds: usize, duration: Duration },
}
```

**Advantages:**
- States are exhaustive (match arms cover all cases)
- Each state carries its own data (no Option fields)
- Serializable (for checkpoint/logging)
- Simple to test (construct any state directly)

**Disadvantage:** Transitions not enforced at compile-time (runtime match).

### 4.2 Typestate Pattern (Alternative)

```rust
struct Coordinator<S: State> { common: CommonData, state: S }

struct Init;
struct WaitingForWorkers { registered: Vec<WorkerId> }
struct Dispatching { sent: usize, total: usize }

impl Coordinator<Init> {
    fn start_accepting(self) -> Coordinator<WaitingForWorkers> { ... }
}
impl Coordinator<WaitingForWorkers> {
    fn begin_round(self) -> Coordinator<Dispatching> { ... }
}
```

**Advantages:**
- Invalid transitions are compile errors
- Zero-cost (no runtime checks)

**Disadvantages:**
- Verbose (one impl block per state)
- Difficult to store heterogeneously (can't put `Coordinator<A>` and `Coordinator<B>` in the same variable)
- Difficult to serialize

### 4.3 Recommendation for Relativist

**Use enum-based FSM** for both Coordinator and Worker:
1. The coordinator state needs to be stored in a single variable (it changes over time)
2. Serialization is needed for logging and potential checkpoint
3. The number of states is small (8 for coordinator, 6 for worker) — manual validation is manageable
4. Transition logic lives in a single `fn handle_event(state, event) -> state` function, which is easily testable

Use `tracing::info!("state_transition", from = ?old, to = ?new)` for observability.

---

## 5. Stimulus-Response Pattern for Relativist

Adapting Dask's pattern (PESQ-004):

```rust
enum Event {
    WorkerRegistered(WorkerId),
    PartitionReturned(WorkerId, Partition),
    HeartbeatReceived(WorkerId),
    HeartbeatTimeout(WorkerId),
    RoundTimerExpired,
    ShutdownRequested,
}

struct Transition {
    new_state: CoordinatorState,
    actions: Vec<Action>,
}

enum Action {
    SendMessage(WorkerId, Message),
    StartTimer(Duration, Event),
    WriteMergedNet(Net),
    EmitMetric(MetricEvent),
}

fn transition(state: &CoordinatorState, event: Event) -> Transition {
    match (state, event) {
        (WaitingForWorkers { registered }, WorkerRegistered(id)) => {
            let mut new_reg = registered.clone();
            new_reg.push(id);
            if new_reg.len() >= min_workers {
                Transition {
                    new_state: Partitioning,
                    actions: vec![/* begin split */],
                }
            } else {
                Transition {
                    new_state: WaitingForWorkers { registered: new_reg },
                    actions: vec![],
                }
            }
        }
        // ... other transitions
    }
}
```

**Benefits:**
- Pure function: `(State, Event) → (State, Actions)` — easily testable
- Actions are data (not side effects) — can be inspected/mocked
- Complete traceability: every state change has a cause (event)

---

## 6. Comparison Table

| Dimension | Enum FSM | Typestate | Stimulus-Response | **Recommendation** |
|-----------|----------|-----------|-------------------|-------------------|
| Compile-time safety | Partial | Full | Partial | Enum FSM |
| Serialization | Easy | Hard | Easy | Enum FSM |
| Testability | Good | Good | Excellent | Stimulus-Response |
| Code volume | Low | High | Medium | Enum FSM base + S-R for coordinator |
| Observability | Manual logging | Type in logs | Event log | Stimulus-Response |
| Async compatibility | Natural | Awkward | Natural | Either works |

**Decision:** Use **enum-based FSM** as the state representation, with **stimulus-response pattern** for the coordinator's main loop. Worker is simple enough for plain enum + match.

---

## 7. Lessons for Relativist

### L1: Enum-Based FSM for All State [ADOPT]
Use Rust enums for Coordinator, Worker, and Connection states. Each variant carries its own data. Derive Serialize, Deserialize, Debug.
→ Informs: SPEC-13

### L2: Stimulus-Response for Coordinator [ADOPT]
The coordinator's main loop should be `fn transition(state, event) -> (state, actions)`. This is pure, testable, and traceable. Inspired by Dask (PESQ-004).
→ Informs: SPEC-13, SPEC-08

### L3: Log Every State Transition [ADOPT]
Every `(old_state, event, new_state)` triple should be logged at INFO level. This provides complete system traceability without external tools.
→ Informs: SPEC-11, SPEC-13

### L4: Typestate is Over-Engineering for This Scale [REJECT]
With 8 coordinator states and 6 worker states, the typestate pattern adds more boilerplate than value. Enum + exhaustive match is sufficient.
→ Informs: SPEC-13

### L5: FSM Enables Deterministic Testing [ADOPT]
A pure `transition(state, event)` function can be tested without network, timers, or I/O. Feed synthetic events, check resulting states and actions. This directly supports SPEC-08's testing strategy.
→ Informs: SPEC-08, SPEC-13

---

## 8. Sources

| Source | URL | Accessed |
|--------|-----|----------|
| Pretty State Machine Patterns in Rust | https://hoverbear.org/blog/rust-state-machine-pattern/ | 2026-03-26 |
| A Fistful of States (Rust FSM) | https://deislabs.io/posts/a-fistful-of-states/ | 2026-03-26 |
| rust-fsm crate | https://docs.rs/rust-fsm/ | 2026-03-26 |
| Oblivious State Machines in Rust | https://medium.com/ing-blog/oblivious-state-machines-in-rust-b1c9c7a84e76 | 2026-03-26 |
| Dask Scheduler State Machine | https://distributed.dask.org/en/stable/scheduling-state.html | 2026-03-26 |
