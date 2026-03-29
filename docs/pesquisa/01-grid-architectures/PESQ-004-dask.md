---
pesq_id: PESQ-004
title: "Dask Distributed: Dynamic Task Scheduler Architecture"
category: Grid Computing Architectures
date_created: 2026-03-25
status: Complete
---

# PESQ-004: Dask Distributed -- Dynamic Task Scheduler Architecture

**Category:** Grid Computing Architectures
**Status:** Complete
**Cross-references:**
- Specs: SPEC-06 (wire protocol), SPEC-13 (system architecture, FSM design), SPEC-05 (merge and grid cycle)
- References: REF-003 (HVM2), REF-017 (Foster 2001)
- Discussions: DISC-005 v2 (cross-boundary protocol), DISC-006 v2 (communication overhead)
- Other PESQs: PESQ-001 (BOINC, for contrast), PESQ-002 (Apache Ignite, for contrast), PESQ-003 (Ray, for contrast)

---

## 1. Subject Overview

Dask is an open-source library for parallel computing in Python, created by Matthew Rocklin at Continuum Analytics (now Anaconda) with the first paper published at SciPy 2015. The `dask.distributed` module is a centrally managed, distributed, dynamic task scheduler that coordinates worker processes across multiple machines. Unlike the core Dask library (which provides single-machine thread/process parallelism), `dask.distributed` extends Dask to clusters and is the module most relevant to Relativist. The project is maintained by the Dask community and commercially supported by Coiled (founded 2020 by Matthew Rocklin).

**Scale:** As of March 2026, the `dask` package receives approximately 24.5 million monthly PyPI downloads (~890K daily). The main `dask/dask` repository has 13.6k GitHub stars; the `dask/distributed` repository has 1.7k stars. Dask runs in production for "100,000s of clusters and 1,000,000,000s of Python functions" according to the project website. Production users include Capital One, Barclays, Walmart, NASA, Los Alamos National Laboratories, Oak Ridge National Laboratory, and hundreds of other institutions. Capital One reported 91% reduction in model training times with early Dask implementations.

**Computation model:** Dask Distributed implements a **dynamic DAG (directed acyclic graph) task scheduler**. Clients submit task graphs to a central scheduler, which manages dependencies, assigns tasks to workers, and tracks results. Tasks are Python functions with declared dependencies; the scheduler resolves the dependency graph and executes tasks as their inputs become available. This is fundamentally different from Relativist's **fixed iterative BSP-like cycle** (SPEC-05), where the computation structure is partition -> distribute -> reduce -> collect -> merge -> repeat, with no task graph or dynamic dependency resolution.

**Key design constraints that shaped Dask Distributed:**
1. **Low overhead per task:** Each task suffers approximately 1ms of overhead, enabling small computations and network roundtrips to complete in under 10ms.
2. **Dynamic task graphs:** The scheduler accepts new tasks continuously from multiple clients; the graph grows during execution.
3. **Memory awareness:** Scheduling policies are designed to minimize memory footprint, with depth-first execution and spill-to-disk capabilities.
4. **Python-native:** The entire stack is Python (with some C extensions), using Tornado/asyncio for asynchronous I/O.
5. **Resilience to worker failure:** The scheduler tracks task lineage and can resubmit tasks to surviving workers when a worker dies.

**Primary reference:** Rocklin, M. (2015). "Dask: Parallel Computation with Blocked algorithms and Task Scheduling." In *Proceedings of the 14th Python in Science Conference (SciPy 2015)*, pp. 126-132. DOI: [10.25080/majora-7b98e3ed-013](https://doi.org/10.25080/majora-7b98e3ed-013)

---

## 2. Architecture / Design

### 2.1 High-Level Architecture

Dask Distributed follows a **centralized scheduler with distributed workers** model. A single scheduler process orchestrates all task execution, communicating with multiple worker processes and client processes over TCP. The scheduler is single-threaded, event-driven, and asynchronous (Tornado/asyncio), processing all events sequentially with sub-millisecond latency.

```
+------------------------------------------------------------------+
|                         SCHEDULER                                 |
|  (single-threaded, async event loop, Tornado/asyncio)            |
|                                                                   |
|  +-------------------+  +------------------+  +--------------+   |
|  | Task State Machine|  | Scheduling       |  | Work         |   |
|  |  - tasks: dict    |  | Policies         |  | Stealing     |   |
|  |  - workers: dict  |  |  - LIFO depth    |  |  - occupancy |   |
|  |  - clients: dict  |  |  - decide_worker |  |  - stealable |   |
|  |  - transition()   |  |  - root-task     |  |    bins      |   |
|  |  - transitions()  |  |    co-assignment |  |  - rebalance |   |
|  +-------------------+  +------------------+  +--------------+   |
|           |                    |                    |             |
|  +-------------------+  +------------------+  +--------------+   |
|  | Stimulus Handlers |  | Client Interface |  | Dashboard    |   |
|  |  - task-finished  |  |  - update-graph  |  | (Bokeh)      |   |
|  |  - task-erred     |  |  - client-       |  | port 8787    |   |
|  |  - reschedule     |  |    releases-keys |  |              |   |
|  |  - register-worker|  |  - scatter       |  |              |   |
|  +-------------------+  +------------------+  +--------------+   |
+----------|-------------------------------|------------------------+
           |  TCP (batched + RPC)          |  TCP
           v                               v
+-------------------+            +-------------------+
|   WORKER 1        |    TCP     |   WORKER N        |
|                   |<---------->|                   |
|  +-------------+  |  (P2P      |  +-------------+  |
|  | WorkerState |  |  gather_   |  | WorkerState |  |
|  |  - tasks    |  |  dep)      |  |  - tasks    |  |
|  |  - data     |  |            |  |  - data     |  |
|  |  - ready    |  |            |  |  - ready    |  |
|  |    heap     |  |            |  |    heap     |  |
|  +-------------+  |            |  +-------------+  |
|  +-------------+  |            |  +-------------+  |
|  | ThreadPool  |  |            |  | ThreadPool  |  |
|  | Executor    |  |            |  | Executor    |  |
|  +-------------+  |            |  +-------------+  |
|  +-------------+  |            |  +-------------+  |
|  | Spill-to-   |  |            |  | Spill-to-   |  |
|  | Disk (LRU)  |  |            |  | Disk (LRU)  |  |
|  +-------------+  |            |  +-------------+  |
+-------------------+            +-------------------+
           ^                               ^
           |           TCP                 |
           v                               v
+-------------------+            +-------------------+
|   CLIENT 1        |            |   CLIENT M        |
|  - submit()       |            |  - submit()       |
|  - Future objects  |            |  - Future objects  |
|  - .result()      |            |  - .result()      |
+-------------------+            +-------------------+
```

### 2.2 Scheduler

The scheduler is the central brain of Dask Distributed. It runs as a single Python process with a single-threaded async event loop (Tornado/asyncio). All state transitions happen sequentially on this event loop -- there is no internal parallelism within the scheduler itself. This design simplifies reasoning about concurrency at the cost of making the scheduler a potential bottleneck for very large clusters.

**Core responsibilities:**
1. Accept task graphs from clients (via `update-graph` messages).
2. Track task state transitions through a well-defined state machine.
3. Assign ready tasks to workers using scheduling policies (LIFO depth-first, co-assignment, queuing).
4. Receive completion/error notifications from workers.
5. Trigger work stealing when load is imbalanced.
6. Report task results back to clients.
7. Track data location across workers (which worker holds which result).

**Core data structures:**
- `tasks`: Dictionary mapping task keys to `TaskState` objects (the primary state store).
- `workers`: Dictionary mapping worker addresses to `WorkerState` objects (tracks resources, occupancy, held data).
- `clients`: Dictionary mapping client identifiers to `ClientState` objects (tracks which tasks each client cares about).
- `idle`: Set of workers with available capacity.
- `saturated`: Set of workers whose computing power is fully exploited.
- `unrunnable`: Set of tasks in `no-worker` state awaiting appropriate workers.

**Event processing model:** The scheduler is purely event-driven. Events (called "stimuli") arrive from workers and clients. Each stimulus triggers state transitions, which may cascade: the `transitions()` function (plural) runs `transition()` repeatedly until no further task-transitions are recommended, reaching a steady state. All event handling completes in approximately 1 millisecond.

### 2.3 Workers

Each worker is a separate Python process that:
1. Connects to the scheduler on startup.
2. Receives task assignments from the scheduler.
3. Gathers dependencies from peer workers (P2P data transfer).
4. Executes tasks in a local `ThreadPoolExecutor`.
5. Stores results in local memory (with spill-to-disk under memory pressure).
6. Reports completion/error back to the scheduler via batched stream.

**Internal architecture (three layers):**

| Layer | Responsibility |
|-------|---------------|
| `WorkerState` | Pure state machine with no I/O or threading awareness. Holds `TaskState` collections, produces `recommendations` and `instructions`. |
| `BaseWorker` | Wraps `WorkerState`, adds asyncio awareness. Defines abstract methods for `execute()` and `gather_dep()`. |
| `Worker` | Concrete implementation with real TCP communication, thread pool execution, and disk spilling. |

This layered design separates the state machine logic from I/O concerns, making the state machine independently testable -- a pattern highly relevant to Relativist's SPEC-13 FSM design.

### 2.4 Communication Model

Dask Distributed uses two communication patterns over TCP:

| Pattern | Mechanism | Use Case |
|---------|-----------|----------|
| **RPC (Request-Response)** | Dedicated TCP connection per call, with response expected | Client -> Scheduler task submission, result gathering |
| **Batched Stream** | Persistent TCP connection, fire-and-forget messages accumulated and sent in batches | Worker -> Scheduler notifications (task-finished, task-erred, heartbeat) |

**Scheduler -> Worker communication:** The scheduler sends task assignments via RPC. The worker receives `compute-task` messages containing the function, argument keys, and locations of dependencies on peer workers.

**Worker -> Scheduler communication:** Workers send fire-and-forget messages through the batched stream channel. Messages like `task-finished` and `task-erred` are accumulated and sent in bulk, reducing per-message TCP overhead. The batched stream also carries heartbeat messages to keep the connection alive and prevent firewalls from closing it.

**Worker -> Worker communication (P2P):** Workers gather dependencies directly from peer workers via `gather_dep()`. For each dependency, the worker selects a peer at random from the scheduler's `who_has` mapping. To improve bandwidth, the worker opportunistically batches multiple dependencies known to be on the same peer, up to a configurable `transfer_message_bytes_limit` (default: 50MB). There is at most one concurrent `gather_dep()` asyncio task per peer worker, with a global limit of 50 concurrent incoming transfers (`transfer_incoming_count_limit`) to prevent network fragmentation.

**Key contrast with Relativist:** Dask's P2P data transfer between workers is essential because task inputs may reside on different workers. In Relativist, all data flows through the coordinator (star topology, SPEC-06). Workers never communicate with each other. The coordinator sends complete partitions (with all necessary data) to each worker, eliminating the need for P2P dependency gathering.

### 2.5 Protocol and Wire Format

Dask uses a custom serialization protocol over TCP, not a standard RPC framework like gRPC:

**Frame structure on the wire:**
1. 8-byte unsigned integer: number of frames (N).
2. N x 8-byte unsigned integers: length of each frame.
3. Frame 0: Administrative header (msgpack-encoded, optionally compressed).
4. Frame 1: Administrative message (msgpack-encoded).
5. Frame 2: Payload header (msgpack-encoded).
6. Frames 3..N: Payload frames (language-specific serialized data).

**Serialization strategy (two-tier):**
- **Small metadata:** MsgPack (fast, compact, no separate headers needed -- "much faster than JSON").
- **Large data / Python objects:** CloudPickle (handles arbitrary Python objects, closures, functions).
- **Compression:** LZ4 or Snappy applied to payloads exceeding 1KB, only if compression achieves at least 10% improvement. The system samples 10KB chunks from five locations to avoid wasting time on uncompressible data.

**Cross-language design decision:** "The client and workers must share the same language and software environment, the scheduler may differ." The scheduler handles only msgpack-encoded metadata and never unpacks language-specific serialized data. This enables a potential C++ or Rust scheduler implementation without modifying workers/clients.

**Contrast with Relativist (SPEC-06):**

| Aspect | Dask Distributed | Relativist |
|--------|-----------------|------------|
| Framing | Variable-length multi-frame (frame count + lengths + frames) | Fixed 8-byte header (4B length + 4B CRC32) + payload |
| Metadata serialization | MsgPack | bincode (via serde) |
| Data serialization | CloudPickle (Python objects) | bincode (Rust types: Net, Partition, Agent) |
| Compression | LZ4/Snappy (adaptive, >1KB) | None in v1 |
| Integrity | TCP-level only (no application checksum) | CRC32C per frame (SPEC-06 R10) |
| Multi-language | Scheduler can differ from workers | Single Rust binary for all roles |

---

## 3. Key Mechanisms

This section focuses heavily on the scheduler and worker state machines, as these directly inform SPEC-13 FSM design.

### 3.1 Scheduler State Machine

The scheduler manages each task through a finite state machine with seven active states plus one terminal state:

**Task states:**

| State | Description | Entry Condition |
|-------|-------------|-----------------|
| `released` | Known but not actively computing or in memory | Initial state, or task no longer needed by any client/dependent |
| `waiting` | On track to be computed, waiting on dependencies to arrive in memory | Task has unfinished dependencies |
| `no-worker` | Ready to be computed, but no appropriate worker exists | All dependencies satisfied, but resource constraints or worker restrictions prevent assignment |
| `queued` | Ready to be computed, but all workers are already full | All dependencies satisfied, but `worker_saturation` config limits prevent assignment |
| `processing` | Assigned to a worker; the scheduler does not track execution details | Scheduler has sent task to worker |
| `memory` | Computation completed successfully; result in memory on one or more workers | Worker reports `task-finished` |
| `erred` | Task computation, or one of its dependencies, encountered an error | Worker reports `task-erred` |
| `forgotten` | No longer needed by any client or dependent; immediately dereferenced | All clients release the task, no dependents remain |

**State transition diagram:**

```
                      update-graph (client submits)
                               |
                               v
                          [released]
                          /        \
            has deps?   /            \  no deps, workers available?
                       v              v
                  [waiting]      [no-worker] / [queued] / [processing]
                       |              |
          deps ready   |     worker   |
                       |   available  |
                       v              |
                  [no-worker]         |
                  [queued]            |
                  [processing] <------+
                       |
                       |  worker reports
                       v
              +--------+--------+
              |                 |
              v                 v
          [memory]          [erred]
              |                 |
              v                 v
         (client gathers   (client gathers
          or releases)      or releases)
              |                 |
              v                 v
         [forgotten]       [forgotten]
              |                 |
              v                 v
        (dereferenced)   (dereferenced)
```

**Transition implementation:** Each transition is implemented as a dedicated method (e.g., `transition_released_waiting`, `transition_waiting_processing`). A central `transition()` function routes from current to target state. The `transitions()` function (plural) handles cascading: a single stimulus may trigger multiple task transitions (e.g., finishing task A makes tasks B and C ready, which are then assigned to workers). The cascade continues until steady state is reached.

**Stimulus types (events that trigger transitions):**

| Source | Stimulus | Triggers |
|--------|----------|----------|
| Worker | `task-finished` | processing -> memory; may cascade waiting -> processing for dependents |
| Worker | `task-erred` | processing -> erred; may cascade dependents -> erred |
| Worker | `reschedule` | processing -> released -> waiting (task explicitly requests rescheduling) |
| Worker | `long-running` | Adjusts worker occupancy (task identified as long-running, frees thread slot) |
| Worker | `add-keys` | Registers additional data locations |
| Worker | `register-worker` | Enables no-worker -> processing transitions for waiting tasks |
| Worker | `unregister` | processing -> released for tasks on lost worker; triggers resubmission |
| Client | `update-graph` | Creates new tasks in released state; triggers transition cascade |
| Client | `client-releases-keys` | May trigger memory -> released -> forgotten |

**Critical design insight for Relativist:** Dask's scheduler state machine is designed for dynamic, incrementally growing task graphs with complex dependencies. Relativist's coordinator FSM (SPEC-06, R26) has a fundamentally different structure: it cycles through fixed phases (WaitingWorkers -> Idle -> Partitioning -> Distributing -> WaitingResults -> Merging -> Idle/ShuttingDown -> Done) without per-task state tracking. The coordinator does not manage individual task dependencies because all workers in a round are independent and receive complete partitions. However, the cascade pattern -- where one state transition triggers others until steady state -- is a useful design principle for handling round completion: receiving the last worker's result should cascade into the merge phase.

### 3.2 Worker State Machine

The worker state machine is more complex than the scheduler's, managing both task execution and data fetching:

**Task states on the worker:**

| State | Category | Description |
|-------|----------|-------------|
| `released` | Special | Known but inactive; retained when dependents exist locally |
| `waiting` | Pre-execution | Scheduler queued the task; dependencies exist cluster-wide but not locally |
| `fetch` | Data fetching | Task data available on peer workers, queued for network transfer |
| `flight` | Data fetching | Data actively transferring from a peer worker |
| `missing` | Data fetching | All known peer workers lack the data; scheduler will be queried for replicas |
| `ready` | Pre-execution | All dependencies in local memory; queued in `ready` heap for thread pool |
| `constrained` | Pre-execution | Like `ready`, but with user-specified resource constraints |
| `executing` | Execution | Running on a thread in the thread pool |
| `long-running` | Execution | Running but has called `secede()`; does not count toward concurrency limits |
| `memory` | Terminal | Execution completed or data successfully transferred |
| `error` | Terminal | Execution failed or serialization/deserialization failed |
| `rescheduled` | Special | Task raised `Reschedule` exception; transitory, immediately transitions to released |
| `cancelled` | Cancellation | Scheduler requested release during flight/executing/long-running; has `previous` substate |
| `resumed` | Cancellation | Recovery from cancelled state; has `previous` and `next` substates |
| `forgotten` | Terminal | No dependents/dependencies; immediately dereferenced and garbage-collected |

**Normal task execution pipeline:**

```
[waiting] --> [ready] / [constrained] --> [executing] --> [memory] / [error]
                                              |
                                              v (secede)
                                        [long-running] --> [memory] / [error]
```

**Dependency fetch pipeline:**

```
[fetch] --> [flight] --> [memory]     (success)
   ^            |
   |            v
   +------- [fetch]                   (peer unavailable, retry with different peer)
   |
   v
[missing] --> [fetch]                 (scheduler provides new replica locations)
```

**Cancellation semantics (most complex part):**

The `cancelled` state exists because threads and asyncio tasks cannot be immediately aborted. When the scheduler tells a worker to release a task that is in `flight`, `executing`, or `long-running`, the worker transitions to `cancelled(previous=<original_state>)`. The running thread/asyncio task proceeds to completion, but its output is discarded.

Three possible outcomes from `cancelled`:
1. **Normal completion:** task transitions to `released` -> `forgotten`.
2. **Scheduler re-requests same action:** task reverts to `previous` state (output discarded, must redo).
3. **Scheduler requests opposite action:** task enters `resumed(previous=X, next=Y)` state:
   - `resumed(flight -> waiting)`: data fetch completes, then task will be computed.
   - `resumed(executing -> fetch)`: computation completes (output discarded), then data is re-fetched.
   - `resumed(long-running -> fetch)`: same as above.

**Stimulus-response architecture:**

The worker state machine uses a clean separation between state logic and I/O:

```
Stimulus (external event)
    |
    v
WorkerState.handle_stimulus()
    |
    +--> _handle_<stimulus_name>()
    |       |
    |       v
    |    recommendations: dict[TaskKey -> NewState]
    |       |
    |       v
    |    _transitions()
    |       |
    |       +--> _transition_<start>_<end>() for each recommendation
    |       |       |
    |       |       v
    |       |    more recommendations + instructions
    |       |
    |       v (repeat until no more recommendations)
    |
    v
instructions: list[Instruction]
    |
    v
BaseWorker / Worker executes instructions
    (send message, spawn thread, gather dependency)
```

**Instruction types (output of state machine, executed by the I/O layer):**
- `SendMessageToScheduler`: fire-and-forget message via batched stream.
- `Execute`: spawn a thread to run the task function.
- `GatherDep`: initiate an asyncio task to fetch data from a peer worker.

**Critical design insight for Relativist:** The three-layer architecture (WorkerState -> BaseWorker -> Worker) is an excellent pattern for testability. The `WorkerState` is a pure state machine with no I/O -- it takes stimuli and produces instructions. This means the entire state machine can be tested deterministically without network or threading. Relativist's SPEC-13 should consider a similar separation: a pure `CoordinatorState` / `WorkerState` that produces instructions, and an I/O layer that executes them. This would enable deterministic simulation testing (PESQ-020/021).

### 3.3 Scheduling Policies

Dask's scheduling policies determine (a) the order in which tasks execute and (b) which worker executes each task:

#### 3.3.1 LIFO Depth-First Execution

When a worker completes a task, its immediate dependents (downstream tasks) receive top priority. This **depth-first** strategy:
- Encourages finishing in-progress computation chains before starting new ones.
- Minimizes the number of intermediate results simultaneously in memory.
- Reduces disk spilling by keeping the "working set" small.
- Can conflict with first-come-first-served fairness between clients (mitigated by coarse-grained fairness: when workers exhaust related tasks, they pull from the common pool in FIFO order).

**Priority tuple:** Each task receives a priority `(submission_order, graph_priority)`:
- `submission_order`: Per-graph counter enforcing FIFO ordering between client submissions.
- `graph_priority`: Client-generated priority reflecting critical path analysis and graph structure (computed by traversals similar to `dask/order.py`).

Tie-breaking uses critical path analysis: tasks with longer critical paths and more descendants are prioritized.

#### 3.3.2 Worker Assignment (decide_worker)

When a task becomes ready, the scheduler selects a worker using a multi-criteria decision:

1. **Dependency locality:** Identify workers that already hold task dependencies. Restrict candidates to workers holding the most input data.
2. **Resource constraints:** Respect user-provided restrictions (e.g., GPU requirements).
3. **Occupancy minimization:** Among tied candidates, select the worker where the task will start soonest (queue occupancy + estimated data transfer time).
4. **Root task co-assignment:** For "root-ish" task groups (>2x more tasks than cluster threads, <5 unique dependencies), batch tasks onto the same worker to minimize future cross-worker data transfer.

#### 3.3.3 Scheduler-Side Queuing

When `worker_saturation` is configured (default: 1.1), the scheduler holds excess tasks in a `queued` state rather than assigning all ready tasks immediately. Tasks are released to workers as threads become available, up to `ceil(worker_saturation * nthreads)` tasks per worker. This prevents memory exhaustion from over-eager task assignment but adds scheduler-worker roundtrip latency.

#### 3.3.4 Work Stealing

Work stealing activates when idle workers exist alongside saturated workers. The scheduler maintains a list of "stealable" task bins ordered by computation-to-communication ratio:

| Bin | Ratio | Policy |
|-----|-------|--------|
| 1 | >= 8 | Always steal (computation dominates) |
| 2 | >= 4 | Steal if significant imbalance |
| 3 | >= 2 | Steal if moderate imbalance |
| ... | ... | ... |
| N | 1/256 | Never steal (communication dominates) |

The ratio is estimated using: (a) exponentially weighted moving average of function runtime, and (b) total bytes of dependencies that would need to be transferred.

**Transactional safety:** When the scheduler decides to steal a task, it queries the busy worker first. The worker responds whether the task can be safely rerouted (still queued) or is already running/complete, preventing duplicate execution.

**Contrast with Relativist:** Relativist has no scheduling policy decisions. The coordinator assigns exactly one partition to each worker per round via a static 1:1 mapping (SPEC-04, SPEC-06). There is no dependency locality to optimize, no work stealing, and no queuing -- because all workers must process their partition before the next round begins (BSP synchronization barrier). Dask's scheduling complexity arises from its dynamic task graph; Relativist's simplicity arises from its fixed iterative structure.

### 3.4 Memory Management

Dask workers implement a tiered memory management system with four thresholds (as percentage of the worker's memory limit):

| Threshold | Default | Action |
|-----------|---------|--------|
| **Target** | 60% | Begin spilling least-recently-used (LRU) managed data to disk |
| **Spill** | 70% | Aggressive spilling based on process memory (not just managed memory) |
| **Pause** | 80% | Stop accepting new tasks; allow in-flight disk writes to complete |
| **Terminate** | 95% | Nanny kills the worker process; tasks are resubmitted elsewhere |

**Memory categories:**
- **Managed memory:** Sum of `sizeof()` for all Dask-tracked data in worker RAM. Excludes spilled data.
- **Unmanaged memory:** Memory Dask cannot track -- Python interpreter, loaded modules, task heap usage, garbage collection backlogs, memory fragmentation.
- **Unmanaged recent:** Unmanaged memory appearing within the last 30 seconds (typically temporary task spikes). Excluded from rebalancing decisions to avoid premature action.
- **Spilled:** Managed data written to disk via the worker's local directory.

**Invariant:** `managed + unmanaged + unmanaged_recent = process_memory (RSS)`

**Spill-to-disk mechanism:** When the target threshold is crossed, the worker dumps the least-recently-used data to disk (by default `/tmp`). Data is transparently restored from disk when needed. The worker uses Python's `sizeof` protocol (with special handling for NumPy arrays and Pandas DataFrames) to estimate data sizes.

**Active Memory Manager (AMM):** A scheduler-side component that periodically (every 2 seconds by default) evaluates the distribution of data across workers and can: (a) replicate frequently-accessed data to reduce transfer costs, (b) drop unnecessary replicas, (c) rebalance data across workers.

**Contrast with Relativist:** Relativist uses arena allocation with `Vec<Option<Agent>>` (SPEC-02). The coordinator holds the entire net in memory; there is no spill-to-disk mechanism. Partitions are ephemeral (created per round, discarded after collection). Worker memory is trivially managed: receive partition, reduce in-place, return result, free. Dask's memory management complexity arises from persistent intermediate results that must survive across task boundaries; Relativist has no such concern because data flows strictly through the coordinator.

### 3.5 Fault Tolerance

Dask Distributed provides automatic task resubmission on worker failure:

**Worker failure detection:**
1. **Clean socket closure:** The scheduler detects `IOError` immediately when the TCP connection drops.
2. **Unclean failure:** If the socket does not close cleanly (e.g., kernel OOM kill, hardware crash), the system waits approximately 3 seconds (heartbeat timeout) before marking the worker as failed.

**Recovery flow:**
1. Scheduler detects worker loss.
2. All tasks in `processing` state on the lost worker transition to `released`.
3. All data known to be exclusively on the lost worker is marked as lost.
4. The scheduler's `transitions()` cascade resubmits affected tasks: `released -> waiting -> processing` on a surviving worker.
5. If a lost task's dependencies are also lost, the entire lineage (chain of dependencies) is recomputed.

**KilledWorker exception:** If a task is sent to a worker and that worker dies, and the same task is resubmitted to another worker which also dies, after a configurable number of deaths (`distributed.scheduler.allowed-failures`, default: 3), Dask "blames" the task itself and raises a `KilledWorker` exception to the client. This prevents a faulty function (e.g., one causing segmentation faults) from killing all workers in the cluster.

**Nanny process:** Each worker is managed by a Nanny process that monitors resource usage and can forcefully kill the worker if memory exceeds the `terminate` threshold (95%). The Nanny can restart the worker automatically.

**Scheduler failure:** There is **no persistence mechanism** to recover scheduler state. If the scheduler process crashes, all ongoing computation is lost. Workers and clients can reconnect after scheduler restart, but the task graph, data locations, and in-progress computations are gone. This is a known limitation.

**User code failures:** Exceptions raised during task execution are captured, serialized, and transmitted to the client. The worker and scheduler continue operating normally. Dependent tasks also transition to `erred`.

**Contrast with Relativist:** Relativist v1 has **no fault tolerance** (SPEC-07 R44, DISC-007 v2). A single worker failure halts computation. However, Dask's lineage-based resubmission maps naturally to Relativist's model: the coordinator retains the original partition until it receives the result (SPEC-06 design). If a worker fails, the coordinator could re-send the partition to another worker. Strong confluence (SPEC-01) provides a mathematically stronger guarantee than Dask's "hope that the function is deterministic" -- re-reduction is provably identical.

### 3.6 Observability

Dask Distributed includes a built-in **Bokeh-based dashboard** on port 8787 (scheduler) providing real-time visualization:

**Dashboard views:**
- **Task Stream:** Shows tasks executing on each worker over time, with color-coding by task type.
- **Progress:** Bar chart of task states (waiting, processing, memory, erred) by task group.
- **Workers:** Per-worker view of CPU, memory, data held, and tasks processed.
- **Graph:** Visualization of the task dependency graph.
- **Profile:** Execution profiling (statistical profiling of worker threads at 100Hz).
- **Memory:** Per-worker memory breakdown (managed, unmanaged, spilled).

**Monitoring:** Dask exposes Prometheus-format metrics when configured, enabling Grafana dashboards for production monitoring.

**Contrast with Relativist:** Relativist v1 has no dashboard. Observability is limited to `tracing` crate logs and `GridMetrics` JSON output (SPEC-05, SPEC-07). For SPEC-11, Prometheus exposition is planned (see PESQ-003, L6).

---

## 4. Comparison with Relativist's Context

### 4.1 Fundamental Computation Model

| Dimension | Dask Distributed | Relativist |
|-----------|-----------------|------------|
| **Paradigm** | Dynamic DAG task scheduling | Iterative BSP-like graph reduction (split-reduce-merge-repeat) |
| **Task model** | Arbitrary Python functions with declared dependencies | Fixed: one partition per worker per round |
| **Task generation** | Dynamic (clients submit graphs continuously) | Static per round (coordinator decides partitioning) |
| **Dependencies** | Complex DAG between tasks; scheduler resolves automatically | None within a round (workers are independent); full dependency between rounds |
| **Scheduler** | Central, single-threaded, event-driven | Central, but phases are sequential (BSP barriers) |
| **Data model** | Arbitrary Python objects (NumPy arrays, DataFrames, etc.) | Interaction net graph (agents + wires) in arena (SPEC-02) |
| **Communication** | Scheduler-worker (RPC + batched) + worker-worker (P2P gather) | Star: workers only talk to coordinator (SPEC-06) |
| **Memory model** | Persistent results across tasks with LRU spill | Ephemeral partitions (created/destroyed per round) |
| **Correctness** | Programmer's responsibility (results may differ on retry if impure) | Strong confluence: `reduce_all(net) ~ run_grid(net, n)` (graph isomorphism) (SPEC-01) |

### 4.2 State Machine Comparison (Most Relevant for SPEC-13)

| Aspect | Dask Scheduler FSM | Relativist Coordinator FSM (SPEC-06) |
|--------|-------------------|--------------------------------------|
| **Granularity** | Per-task states (thousands of tasks, each with independent state) | Per-phase states (single cycle: Idle -> Partitioning -> Distributing -> WaitingResults -> Merging) |
| **States** | 7 active + 1 terminal (released, waiting, no-worker, queued, processing, memory, erred, forgotten) | 8 states (WaitingWorkers, Idle, Partitioning, Distributing, WaitingResults, Merging, ShuttingDown, Done) |
| **Transition triggers** | Stimuli from workers (task-finished, task-erred) and clients (update-graph, release-keys) | Phase completion (all partitions sent, all results received, merge complete) |
| **Cascade** | One stimulus can trigger many transitions (finishing task A enables tasks B, C, D) | No cascade (phases are sequential, not dependency-driven) |
| **Concurrency** | Many tasks in different states simultaneously | All workers in the same phase simultaneously (BSP barrier) |

| Aspect | Dask Worker FSM | Relativist Worker FSM (SPEC-06) |
|--------|----------------|--------------------------------|
| **States** | 14+ states (waiting, fetch, flight, missing, ready, constrained, executing, long-running, memory, error, cancelled, resumed, rescheduled, released, forgotten) | 5 states (Connecting, Idle, Reducing, Sending, Done) |
| **Complexity** | High: manages both data fetching (fetch/flight/missing) and task execution (ready/executing/long-running), plus cancellation (cancelled/resumed) | Low: simple linear pipeline (receive -> reduce -> send) |
| **Data fetching** | Worker fetches dependencies P2P from peers; complex state machine for network saturation, retry, missing data | Not applicable (coordinator sends complete partitions) |
| **Cancellation** | Complex three-state system (cancelled with previous substate, resumed with previous+next) | Not applicable (tasks cannot be cancelled in v1) |
| **I/O separation** | WorkerState (pure logic) -> BaseWorker (asyncio) -> Worker (concrete I/O) | Implicit via control flow (SPEC-06 R28) |

### 4.3 Communication Comparison

| Aspect | Dask Distributed | Relativist (SPEC-06) |
|--------|-----------------|----------------------|
| **Scheduler -> Worker** | RPC: `compute-task` (function + arg keys + who_has) | RPC-like: `AssignPartition` (complete partition data) |
| **Worker -> Scheduler** | Batched stream: fire-and-forget (`task-finished`, `task-erred`) | Send-wait: `PartitionResult` (coordinator blocks until all received) |
| **Worker -> Worker** | P2P: `gather_dep` (TCP, batched up to 50MB per peer) | None (star topology) |
| **Framing** | Multi-frame: frame count + lengths + msgpack headers + payload frames | Single frame: 8-byte header (length + CRC32) + bincode payload |
| **Serialization** | MsgPack (metadata) + CloudPickle (Python objects) + LZ4/Snappy | serde + bincode (all types) |
| **Integrity** | TCP-level only | CRC32C per frame |
| **Connection pattern** | Persistent + batched stream | Persistent (SPEC-06 R19) |
| **Data transfer** | Worker gathers dependencies from peers on demand | Coordinator pushes complete partitions to workers |

### 4.4 Fault Tolerance Comparison

| Aspect | Dask Distributed | Relativist v1 |
|--------|-----------------|---------------|
| **Worker failure** | Automatic resubmission to surviving workers | Coordinator aborts (SPEC-06 R25) |
| **Lineage tracking** | Full task graph maintained; any lost result can be recomputed | Trivial: coordinator retains original partition |
| **Determinism guarantee** | Hope-based ("functions should be deterministic") | Proven (strong confluence, SPEC-01) |
| **Scheduler failure** | Fatal; no persistence mechanism | Fatal; no recovery |
| **Bad function detection** | KilledWorker after `allowed-failures` deaths | Not applicable (pure IC reduction rules) |
| **Memory pressure** | Graceful degradation: spill -> pause -> nanny kill -> resubmit | None; OOM = process crash = computation failure |

---

## 5. Lessons for Relativist (ADOPT / ADAPT / REJECT)

### L1. Stimulus-Response State Machine Architecture -- ADOPT for SPEC-13

**Dask mechanism:** The worker state machine uses a clean stimulus-response pattern: stimuli arrive, `handle_stimulus()` processes them through `_handle_<name>()` methods, produces recommendations (task -> new state), which cascade through `_transitions()` until steady state. The output is a list of instructions (send message, execute task, gather dependency) that the I/O layer executes.

**Relevance to Relativist:** This pattern directly applies to Relativist's coordinator and worker FSMs. Instead of implementing the FSM as implicit control flow (SPEC-06 R28 says this is allowed), SPEC-13 should define:
- `CoordinatorState` struct holding all state (net, partition plan, worker connections, metrics).
- `CoordinatorStimulus` enum: `WorkerConnected`, `PartitioningComplete`, `PartitionSent`, `ResultReceived`, `MergeComplete`, `AllWorkersShutdown`.
- `CoordinatorInstruction` enum: `SendPartition(WorkerId, Partition)`, `PerformMerge`, `SendShutdown(WorkerId)`, `ReportComplete`.
- `handle_stimulus(&mut self, stimulus) -> Vec<Instruction>` method.

This enables deterministic testing of the coordinator logic without network I/O.

**Verdict: ADOPT.** The stimulus-response pattern is the single most valuable design insight from Dask for Relativist's SPEC-13. It cleanly separates state logic from I/O, enabling deterministic simulation testing (PESQ-020/021).

### L2. Three-Layer Worker Architecture (State -> Async -> Concrete) -- ADOPT for SPEC-13

**Dask mechanism:** Workers are structured as three layers: `WorkerState` (pure state machine, no I/O), `BaseWorker` (asyncio-aware, abstract methods), `Worker` (concrete TCP/threading implementation). The `WorkerState` can be tested in isolation by feeding it stimuli and asserting on produced instructions.

**Relevance to Relativist:** This layered architecture maps cleanly to Rust:
- `WorkerState` -> pure Rust struct with `fn handle_stimulus(&mut self, s: Stimulus) -> Vec<Instruction>`. No `async`, no `tokio`, no network types.
- `WorkerRunner` (or similar) -> async Rust struct that owns a `WorkerState`, reads TCP, converts messages to stimuli, feeds them to `WorkerState`, and executes returned instructions via `tokio`.

**Verdict: ADOPT.** The three-layer pattern provides testability and clean separation of concerns. Relativist's worker is much simpler (no P2P fetching, no cancellation), but the principle applies directly.

### L3. Cascading Transition Resolution -- ADAPT for SPEC-13

**Dask mechanism:** The `transitions()` function runs `transition()` repeatedly: one transition may produce recommendations for other tasks, which trigger further transitions, until steady state (no more recommendations). This handles complex dependency chains where finishing one task enables many others.

**Relevance to Relativist:** Relativist's coordinator FSM does not have complex inter-task dependencies. However, a lightweight cascade is useful: receiving the last `PartitionResult` should automatically transition the coordinator from `WaitingResults` to `Merging`. The coordinator can check `received_count == num_workers` after each result and cascade if true.

**Adaptation:** Implement a simple loop in the coordinator's `handle_stimulus()` that processes recommendations until empty. For v1, cascades will be at most 1-2 levels deep (e.g., last result received -> begin merge -> merge complete -> check for normal form -> idle or shutdown).

**Verdict: ADAPT.** Use the cascade pattern but expect shallow cascades (1-2 levels) given Relativist's fixed-phase structure.

### L4. Dynamic DAG Task Scheduling -- REJECT

**Dask mechanism:** Tasks are submitted dynamically by clients, forming a growing DAG. The scheduler resolves dependencies, selects workers based on data locality and occupancy, and continuously processes new submissions while executing existing tasks.

**Relevance to Relativist:** Relativist does not have a task graph. Its computation model is a fixed iterative cycle with BSP barriers. There are no dependencies between workers within a round, and the next round depends entirely on the previous round's merge result. Dynamic scheduling adds complexity without benefit.

**Verdict: REJECT.** The DAG model solves a problem (arbitrary inter-task dependencies in a continuously growing graph) that does not exist in Relativist's fixed iterative structure.

### L5. LIFO Depth-First Execution for Memory Minimization -- REJECT (but note the principle)

**Dask mechanism:** When a worker finishes a task, its dependents get top priority. This minimizes the number of intermediate results in memory simultaneously, reducing spill-to-disk and memory pressure.

**Relevance to Relativist:** Within a single worker's local reduction (SPEC-03, `reduce_all`), the reduction engine processes redexes from a `VecDeque`. The order of redex processing does not affect correctness (strong confluence), but it could affect memory usage: reducing a redex may create new agents, and reducing redexes in certain orders could minimize peak agent count. However, this is an optimization concern for SPEC-03/SPEC-09, not a scheduling policy concern.

**Verdict: REJECT.** The principle (depth-first to minimize memory) is noted but applies to local reduction engine optimization, not to distributed scheduling. Relativist's 1:1 partition-to-worker mapping has no scheduling decisions to optimize.

### L6. Work Stealing with Compute-to-Communicate Ratio -- REJECT for v1

**Dask mechanism:** When idle workers exist alongside saturated workers, the scheduler steals tasks from busy workers. Theft is profitable only when computation time >> communication time (ratio >= 8 in the best bin). Tasks are organized into stealable bins by this ratio.

**Relevance to Relativist:** Relativist uses BSP synchronization: all workers must complete their partition before the next round begins. Work stealing within a round is not applicable because: (a) each worker has exactly one task (reduce its partition), (b) partition sizes are determined by the coordinator at partition time, and (c) mid-round task migration would require re-partitioning the net.

**However:** If one worker finishes significantly faster than others (due to partition imbalance), it sits idle. Future versions could explore "partition splitting" where the coordinator dynamically re-partitions a slow worker's remaining work. This is conceptually similar to work stealing but operates at the partition level, not the task level.

**Verdict: REJECT for v1.** BSP synchronization makes task-level work stealing inapplicable. Partition-level rebalancing is a v2+ concern (Z7: granularity of work).

### L7. Batched Fire-and-Forget Communication -- ADAPT for SPEC-06 / SPEC-13

**Dask mechanism:** Workers send status updates (task-finished, task-erred, heartbeat) through a batched stream -- messages are accumulated and sent in bulk rather than individually. This reduces per-message TCP overhead. These messages are fire-and-forget (no response expected).

**Relevance to Relativist:** Relativist's current protocol (SPEC-06) uses a synchronous send-wait pattern: coordinator sends `AssignPartition`, waits for `PartitionResult`. There is no batching because each worker sends exactly one message per round. However, if Relativist adds heartbeat messages (SPEC-06 future extension), batching would reduce overhead.

**Adaptation for SPEC-13:** The coordinator's `Distributing` phase already sends all partitions concurrently (SPEC-06 R21). This is effectively "batched sending." The `WaitingResults` phase could process results as they arrive (rather than blocking on all) -- a form of batched receiving. This aligns with PESQ-001 L9 (asynchronous backend processing).

**Verdict: ADAPT.** The batched pattern already partially exists in SPEC-06. SPEC-13 should formalize "process results as they arrive" behavior during the collection phase.

### L8. P2P Worker-to-Worker Data Transfer -- REJECT for v1

**Dask mechanism:** Workers fetch dependencies directly from peer workers via `gather_dep()`. This avoids routing all data through the scheduler, reducing the central bottleneck. The scheduler only provides `who_has` mappings (which worker holds which data); actual data flows peer-to-peer.

**Relevance to Relativist:** In Relativist, all data flows through the coordinator (star topology, SPEC-06). The coordinator sends complete partitions; workers never need data from each other. P2P transfer would only be relevant if Relativist adopted a distributed merge protocol (e.g., adjacent partitions merge their shared borders directly), which is far beyond v1 scope.

**Verdict: REJECT for v1.** Star topology with coordinator-mediated data flow is correct for Relativist's merge-centric model.

### L9. Spill-to-Disk Memory Management -- REJECT

**Dask mechanism:** Workers spill LRU data to disk when memory thresholds are crossed. Data is transparently restored on access. Four thresholds provide graceful degradation: target -> spill -> pause -> terminate.

**Relevance to Relativist:** Relativist holds the entire net in coordinator memory (`Vec<Option<Agent>>`, SPEC-02). Workers receive ephemeral partitions that are freed after returning results. There is no concept of "persistent intermediate results" that could be spilled. If the net exceeds coordinator memory, the computation simply cannot proceed -- there is no meaningful subset to spill because the net is a connected graph.

**Verdict: REJECT.** The memory model (arena-allocated connected graph vs. independent cacheable results) is fundamentally different.

### L10. Nanny Process for Worker Health Monitoring -- ADAPT for v2+

**Dask mechanism:** Each worker is supervised by a Nanny process that monitors resource usage, can forcefully kill the worker (at 95% memory), and restart it automatically.

**Relevance to Relativist:** For v1, workers are simple processes with no supervisor (SPEC-07). The Docker deployment provides container-level monitoring but no application-level health management. For v2+, a lightweight supervisor (similar to Dask's Nanny) could monitor worker memory usage and preemptively report to the coordinator before OOM kills.

**Verdict: ADAPT for v2+.** A lightweight worker supervisor is a sensible addition for production deployments. For v1, Docker container health checks are sufficient.

### L11. Scheduler-Side Task Queuing (worker_saturation) -- REJECT

**Dask mechanism:** The scheduler holds excess tasks in a `queued` state, releasing them to workers only as threads become available. This prevents over-committing work and controls memory usage.

**Relevance to Relativist:** Relativist assigns exactly one partition per worker per round. There is no over-commitment to prevent. The coordinator naturally queues work by not starting the next round until the current one completes (BSP barrier).

**Verdict: REJECT.** BSP barriers naturally prevent over-commitment.

### L12. Per-Task State Tracking on Scheduler -- REJECT (but note principle)

**Dask mechanism:** The scheduler maintains per-task `TaskState` objects with individual states, dependencies, and history. This enables fine-grained dependency resolution, work stealing, and fault recovery at task granularity.

**Relevance to Relativist:** Relativist does not track individual tasks. The coordinator tracks per-worker status (connected/busy/idle) and per-round metadata (partition sizes, timing). The "task" granularity is the partition, and there is exactly one per worker per round.

**However:** The principle of tracking per-entity state with explicit transitions is sound. SPEC-13 should define a per-worker state object on the coordinator (e.g., `WorkerStatus { state: WorkerPhase, last_round: u32, partition_size: usize }`).

**Verdict: REJECT (per-task tracking), but ADOPT the principle of per-worker state objects.**

### L13. KilledWorker / allowed-failures Bad Task Detection -- REJECT

**Dask mechanism:** If a task kills multiple workers, it is marked as "bad" and a `KilledWorker` exception is raised to the client, preventing an infinite loop of killing workers.

**Relevance to Relativist:** Relativist workers execute the IC reduction engine (SPEC-03), which is pure graph manipulation. The only way a "task" (partition reduction) could kill a worker is via a bug in the reduction engine itself, which would be caught by the test suite (SPEC-08), not by runtime bad-task detection. Furthermore, v1 has no fault tolerance, so the concept does not apply.

**Verdict: REJECT.** IC reduction is deterministic and does not have "bad tasks." Implementation bugs are caught by testing.

### L14. Bokeh Dashboard for Real-Time Monitoring -- REJECT for v1, NOTE for v2+

**Dask mechanism:** Built-in web dashboard with task stream, progress bars, worker memory, and profiling views.

**Relevance to Relativist:** Beyond v1 scope. Post-hoc analysis of `GridMetrics` JSON/CSV is sufficient for the TCC experimental evaluation. A web dashboard is a significant engineering effort for minimal research benefit.

**Verdict: REJECT for v1.** Post-hoc analysis is sufficient. Note for v2+: a minimal status endpoint (JSON over HTTP) showing current round, phase, and worker status would be lower effort than a full dashboard.

---

## 6. Comparison Table (Dask Distributed vs Relativist)

| Dimension | Dask Distributed | Relativist | Notes |
|-----------|-----------------|------------|-------|
| **Year / Maturity** | 2015-present, production | 2026, TCC prototype | Production vs research |
| **Language** | Python (Tornado/asyncio, some C extensions) | Rust (tokio async) | Different performance profiles |
| **Computation model** | Dynamic DAG task scheduling | Iterative BSP-like graph reduction | Fundamentally different |
| **Task granularity** | Fine-grained (millisecond functions to hours-long tasks) | Coarse-grained (one partition per worker per round) | Dask: millions of tasks. Relativist: N tasks per round |
| **Task generation** | Dynamic (clients submit continuously) | Static per round (coordinator decides) | |
| **Scheduler** | Central, single-threaded, event-driven (Tornado) | Central, phase-sequential (tokio) | Both are single-process schedulers |
| **Scheduler FSM** | Per-task: 7 active states + forgotten | Per-phase: 8 states in cycle | Dask: thousands of concurrent FSMs. Relativist: one cycle |
| **Worker FSM** | 14+ states (fetch, flight, ready, executing, cancelled, resumed...) | 5 states (Connecting, Idle, Reducing, Sending, Done) | Dask complexity from P2P fetch + cancellation |
| **State machine pattern** | Stimulus-response with cascade and instructions | Implicit control flow (SPEC-06 R28) | Dask's pattern is superior for testability |
| **Network topology** | Hybrid: scheduler-worker (star) + worker-worker (P2P) | Star only (coordinator-centric) | |
| **Communication** | RPC + batched stream (fire-and-forget) + P2P gather | Send-wait over persistent TCP (SPEC-06) | |
| **Serialization** | MsgPack (metadata) + CloudPickle (data) + LZ4/Snappy | serde + bincode (all types) | |
| **Framing** | Multi-frame (frame count + lengths + frames) | Single frame (8B header + payload) | Relativist is simpler |
| **Integrity** | TCP-level only | CRC32C per frame (SPEC-06 R10) | Relativist adds application-level integrity |
| **Scheduling** | Multi-criteria: locality, occupancy, depth-first, work stealing | None (1:1 partition-to-worker mapping) | |
| **Data locality** | Core feature (schedule tasks where data lives, P2P transfer) | Not applicable (partitions sent fresh each round) | |
| **Work stealing** | Occupancy-based with compute/communicate ratio bins | Not applicable (BSP barriers) | |
| **Memory management** | Tiered: target -> spill -> pause -> terminate | Arena allocation in coordinator; ephemeral partitions | |
| **Fault tolerance** | Automatic resubmission + KilledWorker + Nanny supervisor | None in v1 (SPEC-07 R44) | |
| **Determinism** | Hope-based ("functions should be deterministic") | Proven (strong confluence, SPEC-01) | Relativist has mathematically stronger guarantee |
| **Scheduler failure** | Fatal (no persistence) | Fatal (no persistence) | Same weakness |
| **Observability** | Bokeh dashboard + Prometheus + profiling | tracing crate + GridMetrics JSON | |
| **Scale target** | 1000s of workers, millions of tasks/sec | 8 machines, N tasks per round | Orders of magnitude difference |
| **Trust model** | Trusted (managed infrastructure) | Trusted (controlled lab) | Both trusted |
| **Deployment** | pip install + Coiled SaaS + Kubernetes | Single binary + Docker (SPEC-07) | |
| **Worker autonomy** | Moderate (executes assigned tasks, fetches own dependencies P2P) | None (receives complete partition, reduces, returns) | |

---

## 7. Sources

### Academic Papers

- Rocklin, M. (2015). "Dask: Parallel Computation with Blocked algorithms and Task Scheduling." In *Proceedings of the 14th Python in Science Conference (SciPy 2015)*, pp. 126-132. [DOI: 10.25080/majora-7b98e3ed-013](https://doi.org/10.25080/majora-7b98e3ed-013) | [PDF](https://proceedings.scipy.org/articles/Majora-7b98e3ed-013.pdf)

### Dask Official Documentation (v2026.1.1)

- [Dask.distributed Home](https://distributed.dask.org/)
- [Scheduler State Machine](https://distributed.dask.org/en/stable/scheduling-state.html)
- [Worker State Machine](https://distributed.dask.org/en/stable/worker-state.html)
- [Scheduling Policies](https://distributed.dask.org/en/stable/scheduling-policies.html)
- [Work Stealing](https://distributed.dask.org/en/stable/work-stealing.html)
- [Communications](https://distributed.dask.org/en/stable/communications.html)
- [Foundations](https://distributed.dask.org/en/stable/foundations.html)
- [Protocol](https://distributed.dask.org/en/stable/protocol.html)
- [Journey of a Task](https://distributed.dask.org/en/stable/journey.html)
- [Worker Memory Management](https://distributed.dask.org/en/stable/worker-memory.html)
- [Resilience](https://distributed.dask.org/en/latest/resilience.html)
- [Why Did My Worker Die?](https://distributed.dask.org/en/stable/killed.html)
- [Active Memory Manager](https://distributed.dask.org/en/stable/active_memory_manager.html)
- [Dask Scheduling Overview](https://docs.dask.org/en/stable/scheduling.html)

### GitHub

- [dask/distributed](https://github.com/dask/distributed) (1.7k stars)
- [dask/dask](https://github.com/dask/dask) (13.6k stars)
- [distributed PyPI](https://pypi.org/project/distributed/) (v2026.1.2)
- [dask PyPI](https://pypi.org/project/dask/) (v2026.3.0, ~24.5M monthly downloads)
- [Scheduling State Machine RST source](https://github.com/dask/distributed/blob/main/docs/source/scheduling-state.rst)
- [Worker State Machine RST source](https://github.com/dask/distributed/blob/main/docs/source/worker-state.rst)

### Dask Project and Community

- [Dask Project Website](https://www.dask.org/)
- [Coiled (commercial Dask support)](https://coiled.io/)
- [PyPI Download Statistics for Dask](https://pypistats.org/packages/dask)
- [Dask in Production: Multi-Scheduler Architectures -- Coiled Blog](https://www.coiled.io/blog/dask-in-production-multi-scheduler-architectures)
- [Tackling Unmanaged Memory with Dask -- Coiled Blog](https://docs.coiled.io/blog/tackling-unmanaged-memory-with-dask.html)

### Industry Adoption

- [Making Python Data Science Enterprise-Ready with Dask -- NVIDIA Blog](https://developer.nvidia.com/blog/making-python-data-science-enterprise-ready-with-dask/)
- [Dask in Production -- SciPy 2024 Talk](https://cfp.scipy.org/2024/talk/NGRVJJ/)
