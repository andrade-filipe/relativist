---
pesq_id: PESQ-003
title: "Ray: Distributed AI Computing Framework"
category: Grid Computing Architectures
date_created: 2026-03-25
status: Complete
---

# PESQ-003: Ray -- Distributed AI Computing Framework

**Category:** Grid Computing Architectures
**Status:** Complete
**Cross-references:**
- Specs: SPEC-05 (merge and grid cycle), SPEC-06 (wire protocol), SPEC-07 (deployment), SPEC-11 (observability, future)
- References: REF-003 (HVM2), REF-017 (Foster 2001)
- Discussions: DISC-005 v2 (cross-boundary protocol), DISC-007 v2 (fault tolerance)
- Other PESQs: PESQ-001 (BOINC, for contrast), PESQ-002 (Apache Ignite, for contrast)

---

## 1. Subject Overview

Ray is an open-source distributed computing framework originally developed at UC Berkeley's RISELab by Philipp Moritz, Robert Nishihara, and Ion Stoica, with the first paper published at OSDI 2018. Ray was designed to support the emerging class of AI applications that require both fine-grained task parallelism (training) and stateful computation (serving), with the key insight that these workloads need a unified system rather than separate specialized frameworks. The project is maintained by Anyscale (founded 2019 by the original authors) and was accepted into the PyTorch Foundation in December 2025, validating its role as a standard distributed AI computing framework.

**Scale:** Ray version 2.54.0 (February 2026) has accumulated over 237 million PyPI downloads. Production deployments include OpenAI (ChatGPT training coordination), Anthropic, Uber, Spotify, Netflix, Shopify, Pinterest, ByteDance, and Instacart. Ray clusters in production range from a handful of nodes to thousands of machines.

**Computation model:** Ray implements a **dynamic task graph** model with two core primitives: **tasks** (stateless remote function calls) and **actors** (stateful remote class instances). Unlike batch-oriented frameworks (Spark, MapReduce) or static DAG frameworks (Dask, Airflow), Ray constructs its task graph dynamically at runtime -- the execution of one task may trigger the creation of arbitrarily many new tasks. This is fundamentally different from Relativist's **fixed split/merge cycle** (SPEC-05), where the computation structure is determined by the coordinator at each round.

**Key design constraints that shaped Ray:**
1. **Sub-millisecond task overhead:** AI workloads (especially reinforcement learning) require millions of fine-grained tasks per second. The OSDI 2018 paper demonstrated 1.8 million tasks/second.
2. **Heterogeneous computation:** Training (stateless, data-parallel) and serving (stateful, latency-sensitive) coexist in the same cluster.
3. **Dynamic task graphs:** The task DAG is not known in advance; it unfolds as the application executes.
4. **Fault tolerance without checkpointing:** Lineage-based reconstruction avoids the cost of persistent checkpoints for intermediate results.
5. **Language transparency:** Python-first API with C++ core runtime for performance.

**Primary reference:** Moritz, P., Nishihara, R., Wang, S., Tumanov, A., Liaw, R., Liang, E., Elibol, M., Yang, Z., Paul, W., Jordan, M.I., Stoica, I. (2018). "Ray: A Distributed Framework for Emerging AI Applications." *Proceedings of the 13th USENIX Symposium on Operating Systems Design and Implementation (OSDI 18)*, pp. 561-577.

---

## 2. Architecture / Design

### 2.1 High-Level Architecture

Ray uses a **hierarchical architecture** with a head node running cluster-level services and worker nodes running per-node daemons. Unlike Apache Ignite's peer-to-peer mesh (PESQ-002), Ray has a clear asymmetry between head and worker nodes, similar to Relativist's coordinator/worker split (SPEC-07).

```
+------------------------------------------------------------------+
|                         HEAD NODE                                 |
|                                                                   |
|  +-------------------+  +------------------+  +--------------+   |
|  | Global Control    |  | Dashboard        |  | Autoscaler   |   |
|  | Service (GCS)     |  | (port 8265)      |  | (optional)   |   |
|  |  - Node Manager   |  |  - Jobs view     |  |              |   |
|  |  - Actor Manager  |  |  - Cluster view  |  |              |   |
|  |  - Placement Mgr  |  |  - Actors view   |  |              |   |
|  |  - Resource Mgr   |  |  - Metrics view  |  |              |   |
|  |  - Job Manager    |  |  - Logs view     |  |              |   |
|  |  - Function Reg.  |  +------------------+  +--------------+   |
|  +-------------------+                                            |
|           |                                                       |
|  +-------------------+  +------------------+  +--------------+   |
|  | Raylet            |  | Object Store     |  | Metrics      |   |
|  | (local scheduler) |  | (shared memory)  |  | Agent        |   |
|  |  - NodeManager    |  |  /dev/shm based  |  | (port 8080)  |   |
|  |  - WorkerPool     |  |  30% of RAM      |  | Prometheus   |   |
|  |  - Scheduler      |  |  or 200GB max    |  | format       |   |
|  |  - ObjectManager  |  +------------------+  +--------------+   |
|  +-------------------+         |                                  |
|           |                    |                                  |
|  +--------v--------------------v-+                                |
|  | Worker Processes (Python/C++) |                                |
|  | Each embeds CoreWorker:       |                                |
|  |  - TaskManager               |                                |
|  |  - ReferenceCounter           |                                |
|  |  - MemoryStore (small objs)   |                                |
|  |  - ObjectRecoveryManager      |                                |
|  +-------------------------------+                                |
+------------------------------------------------------------------+
           |              gRPC              |
           v                               v
+------------------------------------------------------------------+
|                       WORKER NODE N                               |
|                                                                   |
|  +-------------------+  +------------------+  +--------------+   |
|  | Raylet            |  | Object Store     |  | Metrics      |   |
|  | (local scheduler) |  | (shared memory)  |  | Agent        |   |
|  +-------------------+  +------------------+  +--------------+   |
|           |                    |                                  |
|  +--------v--------------------v-+                                |
|  | Worker Processes               |                                |
|  +-------------------------------+                                |
+------------------------------------------------------------------+
```

### 2.2 Global Control Service (GCS)

The GCS is Ray's centralized metadata authority, running on the head node as a standalone `gcs_server` process. It is the single source of truth for cluster-wide state.

**Internal components:**

| Component | Responsibility |
|-----------|---------------|
| **Node Manager** | Node registration, health monitoring, node-level resource tracking |
| **Actor Manager** | Actor creation, scheduling, location registry, restart logic |
| **Placement Group Manager** | Scheduling and lifecycle of placement groups (resource co-location bundles) |
| **Resource Manager** | Cluster-wide resource allocation decisions |
| **Job Manager** | Job registration, configuration, lifecycle tracking |
| **Worker Manager** | Worker process registration and failure detection |
| **Function Registry** | Storage of remote function and actor class definitions |

**Storage backends:** The GCS supports two storage backends: in-memory (default, faster but non-durable) and Redis (for high availability). Communication with the GCS is via gRPC, with configurable timeouts (`GCS_RPC_TIMEOUT_SECONDS`).

**Single point of failure:** By default, the GCS is a single point of failure. If the head node crashes, the entire cluster fails. To achieve HA, operators must configure an external Redis instance (`RAY_REDIS_ADDRESS`). When Redis-backed, the GCS persists metadata to Redis and can recover state upon restart. During recovery (up to 60 seconds by default, configurable via `RAY_gcs_rpc_server_reconnect_timeout_s`), existing tasks and actors continue running but new actor creation, placement group operations, and worker registration are blocked.

### 2.3 Raylet (Per-Node Daemon)

Each node (including the head node) runs a Raylet daemon -- Ray's per-node agent responsible for local resource management, task scheduling, and object management. The Raylet is implemented in C++ for performance.

**Internal components:**

| Component | Function |
|-----------|----------|
| `NodeManager` | Overall node orchestration |
| `WorkerPool` | Worker process lifecycle management (spawn, kill, recycle) |
| `ClusterResourceScheduler` | Task placement decisions based on local and cluster resource availability |
| `LocalObjectManager` | Object spilling (to disk/S3) and eviction under memory pressure |
| `ObjectManager` | Cross-node object transfer coordination (pull-based) |

**Gossip protocol:** Raylets share resource availability information with each other via a gossip protocol, updating approximately every 1 second. This provides an eventually consistent cluster-wide view of resources, enabling distributed scheduling decisions without requiring every scheduling request to go through the GCS.

**Stateless design:** Raylets are designed to be stateless with respect to task execution. If a Raylet crashes, the node is marked as dead, and even if the Raylet restarts on the same physical machine, it receives a new unique identifier -- preventing stale state issues.

### 2.4 Object Store (Plasma)

Ray's object store is a per-node shared-memory system based on Apache Arrow's Plasma, providing zero-copy data sharing between worker processes on the same node.

**Implementation details:**
- Pre-allocated shared memory region (typically `/dev/shm` on Linux)
- Default capacity: 30% of available RAM or 200GB maximum, minimum 75MB
- Single allocator process using `dlmalloc` for memory management
- Worker processes map the shared memory into their address space via memory-mapped files and Unix socket IPC
- Objects are immutable once sealed (write-once, read-many)

**Object lifecycle states:**

```
CREATE --> WRITE --> SEAL --> ACCESS --> EVICT/SPILL
  |          |        |        |           |
  |  allocate|  data  | immut. | zero-copy | LRU eviction
  |  memory  | write  | & vis. | reads     | or disk spill
```

1. **CREATE:** Allocation request reserves memory
2. **WRITE:** Serialized data written to mapped memory
3. **SEAL:** Object becomes immutable and visible to other workers
4. **ACCESS:** Other workers on the same node map memory for zero-copy reads
5. **EVICT/SPILL:** LRU eviction for unreferenced objects; spill to disk/S3 when capacity exceeded

**Zero-copy semantics:** For numpy arrays and objects supporting the out-of-band buffer protocol (pickle protocol 5), `ray.get()` returns data backed directly by shared object store memory. No deserialization or copy occurs. This is critical for AI workloads where objects are often multi-gigabyte tensors.

**Inter-node transfer:** Objects needed on a remote node are transferred via a pull-based protocol through gRPC with chunking. Small objects (<100KB, configurable via `max_direct_call_object_size`) are inlined directly in task submission messages, bypassing the object store entirely.

**Memory pressure handling:** When the object store reaches capacity, the system applies three strategies in order: (1) LRU eviction of unreferenced objects, (2) remote push to nodes with available capacity, (3) external spill to disk or cloud storage (S3, GCS). Objects are automatically restored from spill storage on access.

### 2.5 CoreWorker (Per-Process Runtime)

Every worker process and driver embeds a `CoreWorker` instance -- the client-side distributed execution runtime. This is the interface between application code and the Ray system.

**Key components:**

| Component | Function |
|-----------|----------|
| `TaskManager` | Task lifecycle management and streaming generators |
| `NormalTaskSubmitter` | Routes stateless tasks to the Raylet scheduler |
| `ActorTaskSubmitter` | Directs actor tasks to specific actor workers |
| `TaskReceiver` | Executes incoming tasks in the worker |
| `ReferenceCounter` | Distributed garbage collection via ownership tracking |
| `MemoryStore` | In-process cache for small objects |
| `PlasmaStoreProvider` | Interface to shared-memory object store |
| `ObjectRecoveryManager` | Lineage-based reconstruction of lost objects |

### 2.6 Communication Model

Ray uses gRPC for all inter-process and inter-node communication, with distinct communication patterns for different operations:

| Communication | Protocol | Direction | Purpose |
|--------------|----------|-----------|---------|
| Worker <-> Raylet | IPC (Unix socket) | Bidirectional | Task scheduling, resource requests |
| Worker <-> Worker | gRPC | Direct | Task submission, object transfer (small) |
| Raylet <-> Raylet | gRPC | P2P (gossip) | Resource availability updates (~1s) |
| Raylet <-> GCS | gRPC | Star | Metadata queries, actor registration |
| ObjectManager <-> ObjectManager | gRPC | Pull-based | Large object transfer (chunked) |
| Metrics Agent -> Prometheus | HTTP | Pull (scrape) | Metrics exposition (port 8080) |
| Dashboard | HTTP | Client -> Head | Web UI (port 8265) |

**Key distinction from BOINC and Ignite:** Ray uses **direct worker-to-worker communication** for task submission. When a worker submits a task that is scheduled on a remote node, it sends the `PushTask` gRPC directly to the target worker -- the Raylet only handles scheduling (granting a "lease"), not data routing. This minimizes coordinator bottleneck.

---

## 3. Key Mechanisms

### 3.1 Dynamic Task Graph with Futures

Ray constructs its computation graph dynamically at runtime through two primitives:

**Tasks (stateless):**
```python
@ray.remote
def f(x):
    return x * 2

# Returns ObjectRef (future) immediately
ref = f.remote(42)

# Block until result is ready
result = ray.get(ref)  # 84
```

**Actors (stateful):**
```python
@ray.remote
class Counter:
    def __init__(self):
        self.n = 0
    def increment(self):
        self.n += 1
        return self.n

counter = Counter.remote()
refs = [counter.increment.remote() for _ in range(10)]
results = ray.get(refs)  # [1, 2, 3, ..., 10]
```

**Automatic dependency resolution:** When an `ObjectRef` is passed as an argument to another remote function, Ray automatically resolves the dependency: the downstream task will not execute until the upstream object is available. This creates an implicit directed acyclic graph (DAG) of task dependencies.

```python
@ray.remote
def add(a, b):
    return a + b

# Implicit DAG: add depends on f(1) and f(2)
ref_a = f.remote(1)
ref_b = f.remote(2)
ref_c = add.remote(ref_a, ref_b)  # executes after f(1) and f(2) complete
```

**DAG API (Ray 2.0+):** For more explicit graph construction, Ray provides a unified DAG API using `.bind()` to define computation graphs lazily before execution:

```python
# Build DAG lazily
dag = add.bind(f.bind(1), f.bind(2))
# Execute
result = ray.get(dag.execute())
```

**Contrast with Relativist:** Relativist does not have a task graph. Its computation model is a **fixed iterative cycle**: partition -> distribute -> reduce_local -> collect -> merge -> resolve_borders -> repeat. The "tasks" (local reductions) are not dynamically generated; they are structurally determined by the partitioning step at each round. Ray's dynamic DAG is far more expressive but introduces complexity in scheduling and fault tolerance that Relativist avoids.

### 3.2 Hierarchical Scheduling

Ray uses a **distributed, bottom-up, pull-based** scheduling model, in contrast to centralized schedulers (like Spark's driver or Relativist's coordinator).

**Two-level scheduling flow:**

1. **Local level:** When a worker calls `f.remote()`, it requests a "lease" from its local Raylet. The Raylet checks local resource availability.
2. **Spillback:** If the local node lacks resources, the Raylet performs "spillback" -- it suggests another node based on its gossip-informed view of cluster resources. The worker then requests a lease from that node's Raylet.
3. **Execution:** Once a lease is granted, the worker sends the task directly to the assigned worker process via gRPC.

**Scheduling strategies:**

| Strategy | Behavior | Use Case |
|----------|----------|----------|
| `DEFAULT` | Top-k random selection from lowest-utilization nodes. Packs aggressively below 80% utilization threshold. | General purpose |
| `SPREAD` | Distribute tasks across all available nodes | Minimize resource contention |
| `NODE_AFFINITY` | Pin task to a specific node ID (hard or soft) | Data locality, GPU affinity |
| `PlacementGroup` | Route to pre-reserved resource bundles | Gang scheduling, co-location |

**Data locality awareness:** For tasks (not actors), Ray preferentially schedules on nodes where the largest task arguments already reside in the local object store. This locality preference takes precedence over the DEFAULT strategy's load-balancing heuristic. If multiple arguments are on different nodes, the node with the most bytes locally is preferred.

**Node selection logic (DEFAULT strategy):**
1. Filter out infeasible nodes (lacking required resource types entirely)
2. Score remaining nodes by resource utilization (lower = better)
3. Apply aggressive packing when nodes are below 80% utilized (`RAY_scheduler_spread_threshold=0.5`)
4. Randomize selection among top ~20% ranked candidates (`RAY_scheduler_top_k_fraction`)
5. Strong bias toward local node execution (minimize network transfer)

**Contrast with Relativist:** Relativist uses a **centralized, push-based** model. The coordinator decides all partition-to-worker assignments, creates the `PartitionPlan` (SPEC-04), and pushes partitions to workers (SPEC-06). Workers have no input into scheduling. This is simpler and appropriate for Relativist's BSP-like model where every worker must process exactly one partition per round. Ray's distributed scheduling is necessary because Ray tasks are heterogeneous, dynamic, and potentially millions per second.

### 3.3 Distributed Ownership Model and Garbage Collection

Ray implements distributed garbage collection through an **ownership model** -- each object has a designated owner (the worker process that created it or whose task produced it as a return value).

**Reference types tracked per object:**
- **Local references:** In-process Python reference count
- **Submitted task references:** Tasks pending execution that reference the object
- **Lineage references:** Kept for potential reconstruction
- **Borrower list:** Other workers that received the ObjectRef

**GC protocol:**
1. When a borrower finishes using an object, it publishes an asynchronous notification to the owner
2. The owner merges borrower information into its tracking
3. Deletion safety check: `(local_ref_count + submitted_task_ref_count + borrowers.size()) == 0`
4. If safe, the owner signals the object store to free memory

**Design trade-offs:**
- Reference counting (not tracing GC): chosen for ML workloads with few reference cycles
- Asynchronous pub-sub for GC notifications: higher cleanup latency but non-blocking task completion
- Eager deletion: respects GPU memory constraints where delayed cleanup is costly
- Eventual consistency: prioritizes throughput over immediate reclamation

**Contrast with Relativist:** Relativist uses arena-based allocation with `Vec<Option<Agent>>` (SPEC-02). Agents are "garbage collected" by the reduction rules themselves (ERA interactions annihilate agents). There is no distributed GC because the coordinator holds the entire net in memory; partitions are ephemeral and discarded after collection. This is fundamentally simpler than Ray's ownership model.

### 3.4 Lineage-Based Fault Tolerance

Ray's fault tolerance strategy is **lineage reconstruction** -- re-executing the task chain that produced a lost object, rather than checkpointing intermediate results.

**Recovery flow for lost objects:**
1. A worker requests an object that has been evicted or lost (node failure)
2. Ray first searches other nodes for existing copies
3. If no copies exist, Ray identifies the task that originally produced the object
4. Ray re-executes that task, recursively reconstructing any missing arguments

**Limitations of lineage reconstruction:**
- Objects created via `ray.put()` (not as task return values) are non-recoverable
- Actor task results are not reconstructable by default (requires explicit `max_task_retries > 0`)
- Maximum retry limit: non-actor tasks default to 3 retries; actor tasks default to 0
- **Owner failure is fatal:** If the owner process dies, the object is permanently lost. Ray does not support recovery from owner failure.
- Lineage metadata storage is bounded (`RAY_max_lineage_bytes`, default 1GB)

**Error types on failure:**
| Error | Cause |
|-------|-------|
| `OwnerDiedError` | Owner process lost |
| `ObjectReconstructionFailedError` | Reconstruction impossible (e.g., max retries exceeded) |
| `ObjectLostError` | No reachable copies and no lineage |
| `ObjectFetchTimedOutError` | Network retrieval timeout |

**Node failure semantics:**
- **Worker node failure:** All running tasks and actors fail; all objects owned by that node's workers are lost. Task/actor/object fault tolerance mechanisms attempt recovery on other nodes.
- **Head node failure:** The entire cluster fails (unless GCS HA with Redis is configured). With HA, existing tasks continue during GCS recovery, but new scheduling operations are blocked for up to 60 seconds.
- **Raylet failure:** Node is marked as dead. Even if the Raylet restarts on the same machine, it gets a new unique ID -- preventing stale state.

**Actor fault tolerance:**
- `max_restarts` controls automatic actor recovery (default: 0 = no restart; -1 = infinite)
- On restart, the actor's constructor re-runs (state is lost unless application implements checkpointing)
- `max_task_retries` controls retry of individual actor method calls
- Non-detached actors fate-share with their creator: if the owner dies, the actor dies regardless of `max_restarts`
- Detached actors (`lifetime="detached"`) survive owner death and continue restarting

**Contrast with Relativist:** Relativist v1 has **no fault tolerance** (SPEC-07, R44; DISC-007 v2). A single worker failure halts the entire computation. However, Ray's lineage reconstruction is conceptually interesting for Relativist's future: because IC reduction has **strong confluence** (SPEC-01, T4), re-reducing a partition on a different worker is guaranteed to produce the same result -- a stronger guarantee than Ray's assumption that tasks are "often deterministic." If Relativist adopted lineage-style recovery in v2+, the coordinator could simply re-send a partition to another worker upon failure.

### 3.5 Actor Model

Actors are Ray's mechanism for **stateful distributed computation**. An actor is a long-running worker process that maintains state across method invocations.

**Key properties:**
- Each actor runs in a dedicated worker process
- Methods execute **sequentially by default** (configurable via `max_concurrency`)
- Actor-to-actor calls use sequence numbers for strict ordering guarantees
- Flow control: bounded window per (client -> actor) pair
- Actor location is tracked in the GCS actor registry
- Actors can be named (for discovery) and detached (for persistence beyond creator lifetime)

**Contrast with Relativist:** Relativist workers are **stateless** within the grid loop: they receive a partition, reduce it, return the result, and have no memory of previous rounds. There is no actor model. The closest analogy is that the coordinator itself is a "stateful actor" that maintains the Net across rounds, but this is implicit rather than a programming model.

### 3.6 Dashboard and Observability

Ray provides a comprehensive built-in observability system -- its **Dashboard** and **metrics pipeline** -- that is particularly relevant for Relativist's future SPEC-11 (observability).

#### 3.6.1 Ray Dashboard (Web UI, port 8265)

The Dashboard is a web-based interface on the head node providing real-time visibility into the cluster.

**Views and capabilities:**

| View | Information Displayed | Debugging Features |
|------|----------------------|-------------------|
| **Jobs** | Active, finished, failed jobs; tasks/actors grouped by state; parent-child relationships | Stack traces, CPU flame graphs (profiling) |
| **Cluster** | Hierarchical view of nodes -> workers -> GPU assignments; resource utilization per node | Node-level CPU/memory/disk/network graphs |
| **Actors** | Actor metadata, state, all executed tasks; logs per actor | Error messages, log access |
| **Metrics** | Time-series graphs: tasks/actors/placement groups by state; logical and physical resource usage; per-component CPU/memory | Time range selection, auto-refresh (15s) |
| **Logs** | Ray logs organized by node and filename | Full-text search |
| **Serve** | (Ray Serve only) Deployment configs, replica info, application metrics | Deployment-level metrics |

**Task Timeline Analysis:** Users can download Chrome tracing files for visualization using Perfetto or Chrome DevTools (`chrome://tracing`), enabling microsecond-level analysis of task execution, scheduling delays, and data transfer times.

**Limitation:** The Jobs detail page can only display up to 10K tasks per job.

#### 3.6.2 Metrics Pipeline (Prometheus + Grafana)

Ray exposes metrics in **Prometheus format** via a per-node metrics agent (default port 8080).

**Service discovery mechanisms:**
1. **File-based:** Auto-generated `/tmp/ray/prom_metrics_service_discovery.json` on the head node, updated periodically with all metrics agent addresses.
2. **HTTP-based:** Endpoint at `http://<head>:8265/api/prometheus/sd` returning Prometheus-compatible JSON targets.

**Auto-generated Prometheus config:** Ray generates `/tmp/ray/session_latest/metrics/prometheus/prometheus.yml` with file-based service discovery and a 15-second scrape interval.

**System metrics exposed (examples):**
- Task counts by state (pending, running, finished, failed)
- Actor counts by state
- Object store memory usage (used, available, spilled, evicted)
- Node resource utilization (CPU, GPU, memory, disk, network)
- Scheduling latency
- gRPC request latency
- GCS operation latency

**Custom application metrics API:**
```python
from ray.util.metrics import Counter, Gauge, Histogram

requests = Counter("my_requests_total", description="Total requests")
queue_size = Gauge("my_queue_size", description="Current queue depth")
latency = Histogram("my_request_latency_seconds", description="Request latency",
                    boundaries=[0.01, 0.05, 0.1, 0.5, 1.0])
```

**Grafana integration:** Pre-built Grafana dashboard templates are available (Grafana dashboard ID 14708) that visualize Ray's system metrics. KubeRay exposes a built-in Prometheus exporter on port 8080 by default.

**Programmatic metrics access:** `ray.nodes()` API returns endpoint information (combining `NodeManagerAddress` with `MetricsExportPort`) for custom monitoring integrations.

---

## 4. Comparison with Relativist's Context

### 4.1 Fundamental Computation Model

| Dimension | Ray | Relativist |
|-----------|-----|------------|
| **Paradigm** | Dynamic task graph (tasks + actors) | Iterative BSP-like graph reduction (split-reduce-merge-repeat) |
| **Task granularity** | Fine-grained (microsecond tasks to hours-long actors) | Coarse-grained (one partition per worker per round) |
| **Task generation** | Dynamic at runtime (execution creates new tasks) | Static per round (coordinator decides partitioning) |
| **Statefulness** | Actors maintain state; tasks are stateless | Workers are stateless; coordinator maintains net state |
| **Correctness guarantee** | Programmer's responsibility (determinism assumed, not guaranteed) | Strong confluence: `reduce_all(net) ~ run_grid(net, n)` (graph isomorphism) (SPEC-01, G1) |
| **Data model** | Arbitrary Python objects in shared-memory store | Interaction net graph (agents + wires) in arena (SPEC-02) |
| **Communication** | Hierarchical: worker-worker direct, Raylet gossip, GCS centralized | Star: workers only talk to coordinator (SPEC-06) |
| **Iteration** | Implicit (application-driven loops) | Explicit multi-round loop until Normal Form (SPEC-05) |

### 4.2 Scheduling Comparison

| Aspect | Ray | Relativist |
|--------|-----|------------|
| **Scheduling model** | Distributed, bottom-up, pull-based | Centralized, top-down, push-based |
| **Scheduler location** | Every node (Raylet) + gossip | Coordinator only |
| **Task assignment** | Worker requests lease from Raylet; Raylet grants or spills back | Coordinator assigns partition to worker (SPEC-04, SPEC-06) |
| **Data locality** | Locality-aware: prefer nodes with argument data | No locality (partitions sent fresh each round) |
| **Load balancing** | Dynamic: top-k random selection, data locality, spread strategies | Static: 1:1 partition-to-worker mapping |
| **Scheduling latency** | Sub-millisecond (target: millions of tasks/sec) | Per-round (one scheduling decision per round) |
| **Resource awareness** | Fine-grained: CPU, GPU, memory, custom resources | None (homogeneous workers assumed) |

### 4.3 Object Store vs Serialized Partitions

| Aspect | Ray | Relativist |
|--------|-----|------------|
| **Data sharing model** | Shared-memory object store (Plasma) on each node | Serialized partitions over TCP (SPEC-06) |
| **Intra-node sharing** | Zero-copy via memory-mapped files | Not applicable (single-process workers) |
| **Inter-node transfer** | Pull-based gRPC with chunking | Push-based: coordinator sends, workers return (SPEC-06) |
| **Object mutability** | Immutable (write-once, read-many) | Partitions are mutable during local reduction |
| **Memory management** | LRU eviction + disk spill + cloud spill | Coordinator holds entire net in RAM (SPEC-02) |
| **Serialization** | pickle protocol 5 (with zero-copy support) | serde + bincode (SPEC-06, R4) |
| **Small object optimization** | Inline in task messages (<100KB) | All partitions go through TCP frames |

**Key insight:** Ray's object store is designed for sharing large immutable objects (tensors, datasets) between many tasks on the same node. Relativist's partitions are mutable (workers reduce them) and ephemeral (discarded after collection). The shared-memory model is irrelevant to Relativist because each worker runs as a separate process on a separate machine, and there is no intra-node multi-process sharing.

### 4.4 Fault Tolerance Comparison

| Aspect | Ray | Relativist v1 |
|--------|-----|---------------|
| **Object loss recovery** | Lineage reconstruction (re-execute task chain) | None; computation fails |
| **Worker node failure** | Automatic task/actor/object recovery on other nodes | Coordinator timeout -> failure (SPEC-06, R30) |
| **Head node failure** | Cluster fails (unless GCS HA with Redis) | Coordinator failure = total failure |
| **Actor recovery** | Configurable restarts (`max_restarts`), manual checkpointing | Not applicable |
| **Determinism guarantee** | Assumed ("tasks are often deterministic") | Proven (strong confluence, SPEC-01, T4) |
| **Single point of failure** | GCS (head node) | Coordinator (head node) |

**Critical comparison:** Both Ray and Relativist have a single point of failure at the head node. Ray mitigates this with optional Redis-backed HA; Relativist does not mitigate it at all in v1. However, Relativist has a unique advantage: strong confluence **mathematically guarantees** that re-executing a partition produces identical results, while Ray merely assumes task determinism. This makes Relativist's potential v2+ fault tolerance strictly stronger than Ray's: the coordinator can re-send any partition to any worker with certainty of correctness.

### 4.5 Observability Comparison

| Aspect | Ray | Relativist v1 |
|--------|-----|---------------|
| **Web dashboard** | Comprehensive (Jobs, Cluster, Actors, Metrics, Logs views) | None |
| **Metrics format** | Prometheus (per-node metrics agent, port 8080) | `tracing` crate logs (SPEC-07) |
| **Custom metrics** | Application-level Counter/Gauge/Histogram API | `GridMetrics` struct (SPEC-05, R31-R34) |
| **Visualization** | Grafana dashboards (pre-built templates) | JSON/CSV metrics output (SPEC-07) |
| **Tracing** | Chrome tracing files for task timeline analysis | `tracing` crate with `RUST_LOG` |
| **Service discovery** | Automatic (file-based + HTTP-based for Prometheus) | Manual |
| **Real-time monitoring** | Yes (15-second refresh, auto-scaling feedback) | None (post-hoc analysis only) |

---

## 5. Lessons for Relativist (ADOPT / ADAPT / REJECT)

### L1. Hierarchical Scheduling (Global + Local) -- REJECT for v1

**Ray mechanism:** Distributed scheduling with per-node Raylets making local decisions, gossip-based resource sharing (~1s updates), and spillback to other nodes when local resources are insufficient. Achieves 1.8M tasks/second.

**Relevance to Relativist:** Relativist's scheduling problem is fundamentally different. There is exactly one scheduling decision per round (partitioning), made entirely by the coordinator. With 8 workers and a BSP-like synchronous model, there is no need for distributed scheduling, gossip protocols, or spillback mechanisms. The coordinator already knows the full graph and all worker addresses.

**Verdict: REJECT.** The scheduling problem Relativist faces (partition a graph into N pieces for N known workers) is not the scheduling problem Ray solves (assign millions of heterogeneous tasks to thousands of nodes with varying resources). Centralized push-based scheduling is correct for Relativist's scope.

### L2. Dynamic Task Graph with Futures -- REJECT

**Ray mechanism:** Tasks return ObjectRefs (futures) immediately. Dependencies are resolved automatically. The task graph unfolds dynamically at runtime.

**Relevance to Relativist:** Relativist's computation structure is not a task graph. It is an iterative loop with a fixed phase sequence: partition -> distribute -> reduce_local -> collect -> merge -> resolve_borders (SPEC-05). There are no dependencies between tasks within a round (workers are independent), and the next round depends entirely on the previous round's merge result.

**Verdict: REJECT.** The dynamic DAG model adds complexity without benefit for Relativist's fixed iterative structure.

### L3. Shared-Memory Object Store (Plasma) -- REJECT

**Ray mechanism:** Per-node shared-memory store with zero-copy reads, LRU eviction, and disk/cloud spill. Enables efficient data sharing between multiple worker processes on the same machine.

**Relevance to Relativist:** Relativist workers are single-process, single-machine entities. Each worker receives one partition via TCP, reduces it in-process, and returns the result. There is no intra-node multi-process data sharing. The partition data is mutable during reduction (unlike Plasma's immutable objects). The coordinator holds the entire net in `Vec<Option<Agent>>` (SPEC-02), not in a shared-memory store.

**Verdict: REJECT.** The shared-memory model solves a problem (multi-process data sharing on one machine) that does not exist in Relativist's architecture.

### L4. Lineage-Based Object Reconstruction -- ADAPT for v2+

**Ray mechanism:** When an object is lost, Ray re-executes the task chain that produced it, recursively reconstructing missing arguments. This avoids the cost of persistent checkpoints.

**Relevance to Relativist:** In Relativist's model, the "lineage" of a partition result is trivially short: (1) coordinator sends partition, (2) worker reduces it. If a worker fails, the coordinator still holds the original partition (SPEC-06 design: coordinator retains partition data until collection completes). "Lineage reconstruction" for Relativist is simply: re-send the partition to another worker (or reduce locally on the coordinator).

**Adaptation:** For v2+, implement simple re-execution on worker failure: if `collect_timeout` expires, re-assign the partition to another worker. Strong confluence guarantees identical results -- a stronger guarantee than Ray's probabilistic determinism assumption.

**Verdict: ADAPT for v2+.** The concept maps cleanly to Relativist, but the implementation is trivially simple compared to Ray's because Relativist's "lineage" is always one step deep.

### L5. GCS as Single Source of Truth -- ADOPT (already present)

**Ray mechanism:** The GCS is the authoritative store for cluster-wide metadata: node membership, actor locations, placement groups, job state. All components query the GCS for ground truth.

**Relevance to Relativist:** Relativist's coordinator already serves this role: it holds the Net, the `PartitionPlan`, the border map, `GridMetrics`, and all worker connection state (SPEC-05, SPEC-06). The coordinator is the single source of truth by design.

**Lesson reinforced:** The GCS pattern validates Relativist's coordinator-centric architecture for small clusters. At Relativist's scale (8 nodes), a centralized authority is simpler and avoids the consistency challenges of distributed metadata. Ray needed the GCS because even with distributed scheduling, there must be one place that authoritatively knows where actors live and what resources exist.

**Verdict: ADOPT (already present).** Relativist's coordinator is its GCS equivalent. No changes needed.

### L6. Prometheus Metrics Exposition -- ADOPT for SPEC-11

**Ray mechanism:** Each node runs a metrics agent exposing Prometheus-format metrics on port 8080. Auto-generated service discovery files enable Prometheus to find all exporters. Pre-built Grafana dashboards provide visualization.

**Relevance to Relativist:** This is directly applicable to SPEC-11 (observability). Relativist already uses the `tracing` crate (SPEC-07) and collects `GridMetrics` (SPEC-05), but has no structured metrics exposition. Adding a Prometheus endpoint to the coordinator (and optionally to workers) would enable real-time monitoring of the grid loop.

**Adaptation for Relativist (SPEC-11 input):**
- Coordinator should expose a Prometheus endpoint (e.g., port 9090) with metrics: rounds completed, interactions per round, border redexes per round, partition sizes, round duration, phase durations (partition/distribute/reduce/collect/merge), worker response times.
- Workers MAY expose a Prometheus endpoint with metrics: local reduction time, interactions performed, partition size received/returned.
- Use the `prometheus` or `metrics` Rust crate for exposition.
- Provide a Grafana dashboard template (JSON) in the repo for the pre-defined metrics.

**Verdict: ADOPT for SPEC-11.** Prometheus metrics exposition is the industry standard. Relativist should adopt it for experimental observability.

### L7. Per-Node Metrics Agent with Service Discovery -- ADAPT for SPEC-11

**Ray mechanism:** Automatic service discovery via file-based JSON (for Prometheus file_sd_configs) or HTTP endpoint (for HTTP-based SD). This allows Prometheus to dynamically discover all metrics exporters without manual configuration.

**Relevance to Relativist:** With only 8 nodes and static configuration (SPEC-07, R10), automatic service discovery is overkill. However, the coordinator could serve a simple HTTP endpoint listing all worker addresses and their metrics ports, enabling Prometheus to scrape all nodes from a single configuration.

**Adaptation:** The coordinator's Prometheus endpoint should include a `/sd` path that returns a JSON array of all worker metrics endpoints. This is minimal effort and enables Prometheus to monitor the entire cluster from the coordinator's address alone.

**Verdict: ADAPT for SPEC-11.** Simplified service discovery via coordinator endpoint.

### L8. Dashboard Web UI -- REJECT for v1, ADAPT for v2+

**Ray mechanism:** Comprehensive web dashboard with Jobs, Cluster, Actors, Metrics, and Logs views. Profiling via stack traces and CPU flame graphs. Task timeline analysis via Chrome tracing.

**Relevance to Relativist:** A web dashboard is far beyond v1 scope. Relativist's observability for the TCC is post-hoc: run the experiment, collect `GridMetrics` JSON/CSV output (SPEC-07), analyze offline. Real-time dashboards are valuable for debugging but not required for the experimental evaluation.

**Adaptation for v2+:** A minimal web UI showing: current round, phase (partitioning/distributing/reducing/collecting/merging), worker status (connected/reducing/idle), and live metrics would be valuable for debugging larger deployments.

**Verdict: REJECT for v1. ADAPT for v2+.** Post-hoc metrics analysis is sufficient for the TCC.

### L9. Custom Application Metrics API (Counter/Gauge/Histogram) -- ADOPT for SPEC-11

**Ray mechanism:** Application code can define custom Counter, Gauge, and Histogram metrics that are automatically exposed via the Prometheus endpoint. This enables domain-specific observability without modifying the system.

**Relevance to Relativist:** Relativist already defines `GridMetrics` (SPEC-05, R31-R34) with fields like `rounds`, `interactions_per_round`, `border_redexes_per_round`, `timings_per_phase`. These map naturally to Prometheus metric types:

| GridMetrics field | Prometheus type | Metric name (proposed) |
|-------------------|----------------|----------------------|
| `rounds` | Counter | `relativist_rounds_total` |
| `interactions_per_round[i]` | Histogram | `relativist_interactions_per_round` |
| `border_redexes_per_round[i]` | Histogram | `relativist_border_redexes_per_round` |
| `round_duration` | Histogram | `relativist_round_duration_seconds` |
| `partition_duration` | Histogram | `relativist_phase_duration_seconds{phase="partition"}` |
| `distribute_duration` | Histogram | `relativist_phase_duration_seconds{phase="distribute"}` |
| `reduce_duration` | Histogram | `relativist_phase_duration_seconds{phase="reduce"}` |
| `collect_duration` | Histogram | `relativist_phase_duration_seconds{phase="collect"}` |
| `merge_duration` | Histogram | `relativist_phase_duration_seconds{phase="merge"}` |

**Verdict: ADOPT for SPEC-11.** Map GridMetrics fields to Prometheus metric types. Use a Rust Prometheus client crate.

### L10. Chrome Tracing for Task Timeline Analysis -- ADAPT for SPEC-11

**Ray mechanism:** Ray can export Chrome tracing files that visualize task execution timelines with microsecond precision, showing scheduling delays, execution time, and data transfer time per task.

**Relevance to Relativist:** The `tracing` crate already used by Relativist (SPEC-07) supports the `tracing-chrome` layer, which outputs Chrome tracing format. This would enable visualizing the grid loop phases per round: when partitioning started/ended, when each worker received/returned its partition, when merge started/ended.

**Adaptation:** SPEC-11 should specify that Relativist supports an optional `--trace <PATH>` CLI flag that enables Chrome tracing output. This requires adding the `tracing-chrome` crate and instrumenting the grid loop phases with `tracing::instrument` spans.

**Verdict: ADAPT for SPEC-11.** Minimal effort (one crate + span annotations) for high-value debugging capability.

### L11. Gossip-Based Resource Sharing -- REJECT

**Ray mechanism:** Raylets share resource availability via gossip protocol (~1s updates), enabling distributed scheduling decisions without centralized queries.

**Relevance to Relativist:** Relativist has no distributed scheduling. The coordinator knows all workers and their status (connected/busy/idle) directly via the persistent TCP connections (SPEC-06). There is nothing to gossip about.

**Verdict: REJECT.** Not applicable to star topology with centralized scheduling.

### L12. Actor Model for Stateful Computation -- REJECT

**Ray mechanism:** Actors maintain state across invocations, with sequential method execution guarantees, configurable concurrency, and lifecycle management (restarts, checkpointing).

**Relevance to Relativist:** Relativist workers are stateless by design. They receive a partition, reduce it, and return it. The coordinator is the only stateful entity. Introducing actors would add complexity without benefit for the current computation model.

**Verdict: REJECT.** Stateless workers are the correct design for Relativist's BSP-like model.

### L13. Direct Worker-to-Worker Communication -- REJECT for v1, NOTE for v2+

**Ray mechanism:** Workers send tasks directly to other workers via gRPC, bypassing the scheduler for the data path. The scheduler only handles "lease" grants, not data routing.

**Relevance to Relativist:** In Relativist, all data flows through the coordinator (star topology, SPEC-06). Workers never communicate with each other. This is correct for the merge-centric model: the coordinator must see all partitions to perform the merge. Direct worker-to-worker communication would only be relevant if Relativist adopted a distributed merge protocol (e.g., adjacent partitions merge their shared borders directly), which is far beyond v1 scope.

**Verdict: REJECT for v1.** Star topology with coordinator-mediated data flow is correct.

### L14. Ownership-Based Distributed Garbage Collection -- REJECT

**Ray mechanism:** Distributed reference counting with owner tracking, borrower notifications, and asynchronous cleanup.

**Relevance to Relativist:** Not applicable. Relativist's memory management is trivial: the coordinator owns the Net, partitions are created/destroyed each round, workers' partition data is freed after collection. There are no shared references across processes. Arena allocation in `Vec<Option<Agent>>` with `None` marking freed slots (SPEC-02) handles all GC needs.

**Verdict: REJECT.** The problem does not exist in Relativist's architecture.

---

## 6. Comparison Table (Ray vs Relativist)

| Dimension | Ray | Relativist | Notes |
|-----------|-----|------------|-------|
| **Year / Maturity** | 2017-present, production (OSDI 2018) | 2026, TCC prototype | Production vs research |
| **Language** | C++ core, Python API | Rust (single binary) | |
| **Computation model** | Dynamic task graph (tasks + actors) | Iterative BSP-like graph reduction | Fundamentally different task model |
| **Task granularity** | Microsecond to hours | One partition per worker per round | Ray: fine-grained. Relativist: coarse-grained |
| **Task generation** | Dynamic at runtime | Static per round (coordinator decides) | Ray: application-driven. Relativist: coordinator-driven |
| **Network topology** | Hierarchical (GCS + Raylets + workers) | Star (coordinator-centric) | Ray: more scalable. Relativist: simpler |
| **Communication protocol** | gRPC (protobuf) | TCP + length-prefixed bincode (SPEC-06) | Both use TCP underneath |
| **Scheduling** | Distributed, bottom-up, pull-based | Centralized, top-down, push-based | Different scalability requirements |
| **Data locality** | Core feature (schedule tasks where data lives) | None (partitions sent fresh each round) | Ray: persistent data. Relativist: ephemeral |
| **Object store** | Shared-memory Plasma (zero-copy, immutable) | Serialized partitions over TCP | Fundamentally different data sharing model |
| **Fault tolerance** | Lineage reconstruction + actor restarts + GCS HA | None in v1 | Ray: comprehensive. Relativist: none |
| **Determinism** | Assumed ("tasks often deterministic") | Proven (strong confluence, SPEC-01) | Relativist has stronger guarantee |
| **Single point of failure** | GCS / head node (mitigated by Redis HA) | Coordinator (not mitigated) | Same architectural weakness |
| **Observability** | Dashboard + Prometheus + Grafana + Chrome tracing | tracing crate + GridMetrics JSON | Ray: comprehensive. Relativist: minimal |
| **Metrics format** | Prometheus (auto-discovered) | JSON/CSV (post-hoc) | Ray: real-time. Relativist: batch |
| **Custom metrics** | Counter/Gauge/Histogram API | GridMetrics struct fields | Similar data, different exposition |
| **Scale target** | Thousands of nodes, millions of tasks/sec | 8 machines, one task per worker per round | Orders of magnitude difference |
| **Trust model** | Trusted (managed infrastructure) | Trusted (controlled lab) | Both trusted |
| **Data mutability** | Immutable objects (write-once in store) | Mutable partitions (workers reduce in-place) | Different by design |
| **GC mechanism** | Distributed ownership + reference counting | Arena allocation + reduction rules | Relativist: trivially simple |
| **Statefulness** | Actors maintain state; tasks are stateless | All workers stateless; coordinator stateful | Different computation models |
| **Iteration** | Application-driven (implicit loops) | System-driven (explicit round loop) | Different control models |
| **Worker autonomy** | High (request leases, choose tasks) | None (receive partition, reduce, return) | Reflects centralized vs distributed control |
| **Deployment** | Kubernetes, VMs, cloud managed (Anyscale) | Single binary + Docker (SPEC-07) | Different operational complexity |

---

## 7. Sources

### Academic Papers

- Moritz, P., Nishihara, R., Wang, S., Tumanov, A., Liaw, R., Liang, E., Elibol, M., Yang, Z., Paul, W., Jordan, M.I., Stoica, I. (2018). "Ray: A Distributed Framework for Emerging AI Applications." *Proceedings of the 13th USENIX Symposium on Operating Systems Design and Implementation (OSDI 18)*, pp. 561-577. [USENIX](https://www.usenix.org/conference/osdi18/presentation/moritz) | [PDF](https://www.usenix.org/system/files/osdi18-moritz.pdf) | [arXiv:1712.05889](https://arxiv.org/abs/1712.05889)

### Ray Official Documentation (v2.54.0)

- [Ray Dashboard (Getting Started)](https://docs.ray.io/en/latest/ray-observability/getting-started.html)
- [Scheduling Strategies](https://docs.ray.io/en/latest/ray-core/scheduling/index.html)
- [Placement Groups](https://docs.ray.io/en/latest/ray-core/scheduling/placement-group.html)
- [Objects](https://docs.ray.io/en/latest/ray-core/objects.html)
- [Object Fault Tolerance](https://docs.ray.io/en/latest/ray-core/fault_tolerance/objects.html)
- [Actor Fault Tolerance](https://docs.ray.io/en/latest/ray-core/fault_tolerance/actors.html)
- [GCS Fault Tolerance](https://docs.ray.io/en/latest/ray-core/fault_tolerance/gcs.html)
- [Node Fault Tolerance](https://docs.ray.io/en/latest/ray-core/fault_tolerance/nodes.html)
- [Collecting and Monitoring Metrics](https://docs.ray.io/en/latest/cluster/metrics.html)
- [Using Prometheus and Grafana](https://docs.ray.io/en/latest/cluster/kubernetes/k8s-ecosystem/prometheus-grafana.html)
- [Tasks](https://docs.ray.io/en/latest/ray-core/tasks.html)
- [Ray Core Walkthrough](https://docs.ray.io/en/latest/ray-core/walkthrough.html)
- [Ray DAG API](https://docs.ray.io/en/latest/ray-core/ray-dag.html)
- [Serialization](https://docs.ray.io/en/latest/ray-core/objects/serialization.html)
- [Fault Tolerance Overview](https://docs.ray.io/en/latest/ray-core/fault-tolerance.html)
- [Large Cluster Best Practices](https://docs.ray.io/en/latest/cluster/vms/user-guides/large-cluster-best-practices.html)

### Technical Deep Dives

- [Ray Core System Design: A Deep Dive -- Nikolai Karpov](https://nkkarpov.github.io/blog/ray-core-system-design/)
- [ray-project/ray -- DeepWiki](https://deepwiki.com/ray-project/ray)
- [Ray: A Distributed System for AI -- BAIR Blog](https://bair.berkeley.edu/blog/2018/01/09/ray/)
- [Micah Lerner: Ray OSDI 2018 Paper Review](https://www.micahlerner.com/2021/06/27/ray-a-distributed-framework-for-emerging-ai-applications.html)

### Plasma Object Store

- [The Plasma In-Memory Object Store -- Ray Project Blog (2017)](https://ray-project.github.io/2017/08/08/plasma-in-memory-object-store.html)
- [Plasma In-Memory Object Store -- Apache Arrow Blog](https://arrow.apache.org/blog/2017/08/08/plasma-in-memory-object-store/)

### Industry Adoption

- [Redis in Ray: Past and Future -- Anyscale Blog](https://www.anyscale.com/blog/redis-in-ray-past-and-future)
- [PyTorch Foundation Welcomes Ray -- Linux Foundation](https://www.linuxfoundation.org/press/pytorch-foundation-welcomes-ray-to-deliver-a-unified-open-source-ai-compute-stack)
- [How Ray Powers ChatGPT -- The New Stack](https://thenewstack.io/how-ray-a-distributed-ai-framework-helps-power-chatgpt/)
- [Unleashing ML Innovation at Spotify with Ray -- Spotify Engineering](https://engineering.atspotify.com/2023/02/unleashing-ml-innovation-at-spotify-with-ray)
- [Ray Clusters for AI: Distributed Computing Architecture -- Introl Blog (2025)](https://introl.com/blog/ray-clusters-distributed-ai-computing-infrastructure-guide-2025)

### Grafana and Monitoring

- [Ray Grafana Dashboard (ID 14708)](https://grafana.com/grafana/dashboards/14708-ray/)
- [Ray Monitoring with Prometheus & Grafana -- Databricks Community](https://community.databricks.com/t5/technical-blog/ray-monitoring-made-easy-prometheus-amp-grafana-with-ray-on/ba-p/108641)

### GitHub

- [ray-project/ray](https://github.com/ray-project/ray) (38k+ stars)
- [ray PyPI](https://pypi.org/project/ray/) (v2.54.0, 237M+ downloads)
