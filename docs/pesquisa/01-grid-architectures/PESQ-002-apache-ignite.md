---
pesq_id: PESQ-002
title: "Apache Ignite: Compute Grid Architecture"
category: Grid Computing Architectures
date_created: 2026-03-25
status: Complete
---

# PESQ-002: Apache Ignite -- Compute Grid Architecture

**Category:** Grid Computing Architectures
**Status:** Complete
**Cross-references:**
- Specs: SPEC-05 (merge and grid cycle), SPEC-04 (partitioning), SPEC-06 (wire protocol), SPEC-13 (system architecture, future)
- References: REF-017 (Foster 2001), REF-007 (Casanova 2002)
- Discussions: DISC-005 v2 (cross-boundary protocol), DISC-004 v2 (partitioning)
- Other PESQs: PESQ-001 (BOINC, for contrast), PESQ-012 (MapReduce/Dataflow/BSP, future)

---

## 1. Subject Overview

Apache Ignite is an open-source distributed computing platform originally developed by GridGain Systems (founded 2010), open-sourced in 2014, and graduated to a top-level Apache Software Foundation project on September 18, 2015. The current stable release is 2.17.0 (February 2025), with a preview 3.0.0 release also available. Ignite is written primarily in Java (with C#, C++, and SQL support) and runs on IA-32, x86-64, PowerPC, and SPARC architectures under the Apache License 2.0.

**Core identity:** Ignite is an **in-memory computing platform** that unifies two tightly coupled subsystems: an **in-memory data grid** (distributed key-value store with SQL support) and a **compute grid** (distributed task execution with MapReduce semantics). The distinctive design principle is **co-location**: computation moves to where data resides, rather than moving data to computation. This is fundamentally different from both BOINC (PESQ-001), which distributes independent tasks with no shared state, and Relativist, which distributes partitions of a single graph.

**Scale:** Ignite targets enterprise deployments ranging from a handful of nodes to hundreds or thousands of server nodes in production clusters. Unlike BOINC's millions of anonymous volunteers, Ignite operates on managed infrastructure with known, trusted nodes.

**Computation model:** Ignite's compute grid implements a **MapReduce-style** execution model with three phases: **map** (split a task into jobs and assign to nodes), **execute** (run jobs on assigned nodes), and **reduce** (aggregate results). This maps directly to the classic MapReduce pattern but with lower latency (in-memory, sub-second) compared to batch-oriented systems like Hadoop. The model has structural similarities to Relativist's grid cycle (SPEC-05) but operates on independent jobs rather than partitions of a shared interaction net.

**Primary distinction from Relativist:** Ignite's jobs are **independent**: each job receives its input, produces its output, and has no awareness of sibling jobs (unless explicitly using task sessions). Relativist's partitions are **coupled**: they are fragments of a single graph connected by border wires, and the merge phase must restore these connections to maintain the graph's global invariants. This fundamental difference shapes every architectural decision.

---

## 2. Architecture / Design

### 2.1 High-Level Architecture

Ignite uses a **shared-nothing peer-to-peer** architecture with two node types and a pluggable Service Provider Interface (SPI) system for discovery, communication, failover, and load balancing.

```
+--------------------------------------------------------------------+
|                     IGNITE CLUSTER                                  |
|                                                                     |
|  Discovery Ring (TcpDiscoverySpi, port 47500)                       |
|  +--------+    +--------+    +--------+    +--------+               |
|  | Server |<-->| Server |<-->| Server |<-->| Server |-->(ring)      |
|  | Node 1 |    | Node 2 |    | Node 3 |    | Node 4 |               |
|  +---+----+    +---+----+    +---+----+    +---+----+               |
|      |             |             |             |                     |
|      +-------------+-------------+-------------+                    |
|            Direct P2P Communication                                 |
|            (TcpCommunicationSpi, port 47100)                        |
|                         |                                           |
|                    +----+----+                                      |
|                    | Client  |  (connects to one server)            |
|                    | Node    |                                      |
|                    +---------+                                      |
+--------------------------------------------------------------------+
          |                                    |
   +------+------+                     +-------+------+
   | In-Memory   |                     | Compute      |
   | Data Grid   |                     | Grid         |
   | (Caches,    |                     | (Tasks,      |
   |  Partitions,|                     |  Jobs,       |
   |  SQL,       |                     |  Closures,   |
   |  Affinity)  |                     |  MapReduce)  |
   +-------------+                     +--------------+
```

### 2.2 Node Types

**Server Nodes:** The workhorses of the cluster. Server nodes:
- Store data (cache partitions) in off-heap memory managed outside the JVM garbage collector.
- Execute compute jobs dispatched by the compute grid.
- Participate in the discovery ring for cluster membership and failure detection.
- Serve as coordinators for distributed operations (e.g., partition map exchange).

**Client Nodes:** Lightweight entry points for applications. Client nodes:
- Do not store data or participate in the discovery ring.
- Connect to a single server node as a router for all cluster operations.
- Submit compute tasks and query cached data.
- Have a longer failure detection timeout (30s vs 10s for servers).

**Contrast with Relativist:** Relativist has a strict **coordinator/worker** separation (SPEC-05, SPEC-06). The coordinator holds the full Net, performs partitioning and merge, and dispatches work. Workers are stateless executors that receive a partition, reduce it, and return the result. In Ignite, any server node can initiate or execute tasks; there is no fixed coordinator. Ignite's client nodes are analogous to Relativist's CLI client that submits work, but Ignite clients can be long-lived application processes.

### 2.3 Discovery and Topology

**TcpDiscoverySpi** organizes server nodes into a **ring topology** for cluster membership management:
- Default discovery port: 47500 (range of 100 ports).
- New nodes find the cluster by probing addresses from an `IpFinder` (static list, multicast, or ZooKeeper-based).
- A joining node sends a `JoinRequestMessage` to any ring member, which forwards it to the **coordinator** (the oldest node in the ring).
- The coordinator inserts the new node between the last node and itself, then propagates `NodeAddedMessage` around the ring.
- Failure detection: each server monitors its next ring neighbor. If a heartbeat fails, the failure is propagated around the ring and the dead node is removed.

**TcpCommunicationSpi** handles all operational messages (task execution, data queries, replication) via **direct peer-to-peer TCP connections**:
- Default communication port: 47100 (range of 100 ports).
- `tcpNoDelay = true` by default for low latency.
- Idle connection timeout: 600 seconds.
- Optional paired connections (separate sockets for incoming/outgoing).

**Contrast with Relativist:** Relativist uses a star topology (SPEC-06): all workers connect only to the coordinator. Workers never communicate with each other. Ignite's P2P communication SPI allows any node to send messages to any other node, enabling direct data transfer for affinity-based computations. Relativist's star topology is simpler but creates a coordinator bottleneck; Ignite's mesh topology is more scalable but more complex.

### 2.4 In-Memory Data Grid

The data grid is a distributed key-value store with:

**Partitioning:** Every cache is divided into a fixed number of **partitions** (default: 1024). The **rendezvous hashing** algorithm assigns each partition to a primary server node and zero or more backup nodes. The affinity function guarantees that when topology changes, partitions migrate only to/from the joining/leaving node (minimal data movement).

**Cache Modes:**
- **PARTITIONED:** Partitions distributed equally across server nodes. Most scalable mode. Each key update only propagates to primary + backup nodes.
- **REPLICATED:** All data replicated to every node. Maximum read availability but every write propagates to all nodes.

**Partition Map Exchange (PME):** When topology changes (node join/leave, cache start/stop), a cluster-wide PME process occurs: the coordinator collects partition information from all nodes, creates a complete partition map, and redistributes it so every node knows where every key lives.

**Rebalancing:** After PME, partitions migrate automatically to their new owners to restore balance.

**Contrast with Relativist:** Relativist's "partitioning" (SPEC-04) splits an interaction net graph into sub-nets with FreePort boundary sentinels. Ignite's partitioning splits a key-value space into hash ranges. Both aim to distribute data across nodes, but the semantics are completely different:
- Ignite partitions are independent: no partition references keys in another partition (queries may join across partitions, but the data model itself is independent).
- Relativist partitions are coupled: border wires explicitly connect agents across partition boundaries, requiring a merge phase to restore the graph.
- Ignite's partition count is fixed (1024) and does not change at runtime. Relativist re-partitions the net at every round based on the current graph structure.

### 2.5 Compute Grid

The compute grid provides the `IgniteCompute` interface for distributed task execution. Three execution patterns:

1. **Distributed Closures:** `run()`, `call()`, `apply()` execute `IgniteRunnable`, `IgniteCallable`, or `IgniteClosure` on one or more nodes. Simplest API for ad-hoc computation.
2. **Broadcast:** `broadcast()` executes a task on **all** nodes in the target cluster group. Useful for cache warming, cleanup, or aggregation.
3. **MapReduce Tasks:** `execute(ComputeTask, arg)` implements the full map/reduce lifecycle with explicit control over job splitting, node assignment, failover, and result aggregation.

All three patterns have synchronous and asynchronous variants (returning `IgniteFuture` for async).

---

## 3. Key Mechanisms

### 3.1 ComputeTask Interface: Map / Result / Reduce

The core of Ignite's compute grid is the `ComputeTask<T, R>` interface with three methods:

```java
public interface ComputeTask<T, R> extends Serializable {

    // Phase 1: Split task into jobs and assign to nodes.
    // subgrid: available nodes (order randomized by framework).
    // arg: task argument passed by the caller.
    // Returns: Map of (ComputeJob -> ClusterNode) assignments.
    @NotNull Map<? extends ComputeJob, ClusterNode> map(
        List<ClusterNode> subgrid,
        @Nullable T arg
    ) throws IgniteException;

    // Phase 2: Called after EACH job completes.
    // res: the completed job's result.
    // rcvd: all previously received results.
    // Returns: WAIT (continue), REDUCE (skip to reduce), FAILOVER (retry).
    ComputeJobResultPolicy result(
        ComputeJobResult res,
        List<ComputeJobResult> rcvd
    ) throws IgniteException;

    // Phase 3: Aggregate all job results into a single task result.
    // results: all completed job results.
    @Nullable R reduce(List<ComputeJobResult> results) throws IgniteException;
}
```

**Lifecycle:**
1. The caller invokes `ignite.compute().execute(MyTask.class, arg)`.
2. Ignite calls `map(subgrid, arg)`: the task creates `ComputeJob` instances and assigns each to a `ClusterNode`. This is the **splitting and mapping** phase.
3. Each job is serialized and sent to its assigned node for execution.
4. As each job completes, `result(res, rcvd)` is called on the originating node. The return value controls flow:
   - `WAIT`: continue waiting for remaining jobs.
   - `REDUCE`: stop waiting and proceed immediately to the reduce phase (early termination).
   - `FAILOVER`: re-execute this job on a different node (triggered automatically on exceptions by `ComputeTaskAdapter`).
5. After all jobs complete (or `REDUCE` is returned), `reduce(results)` aggregates the results into the final output.

**Analogy to Relativist's grid cycle (SPEC-05):**

| Ignite Phase | Relativist Phase | Correspondence |
|-------------|-----------------|----------------|
| `map()` / `split()` | `partition()` + `distribute()` | Split work into units, assign to nodes |
| Job execution on nodes | `reduce_local()` on workers | Independent parallel computation |
| `result()` callbacks | `collect()` phase | Coordinator receives results from workers |
| `reduce()` | `merge()` + `resolve_borders()` | Combine results into final output |
| — (jobs are independent) | Border redex detection & resolution | **No Ignite equivalent** |
| — (single-shot) | Grid loop (repeat until Normal Form) | **No Ignite equivalent** |

The critical differences are: (a) Ignite's `reduce()` is a simple aggregation (sum, concatenate, etc.) because jobs are independent; Relativist's merge must reconstruct a graph by restoring border wires, then resolve emergent redexes. (b) Ignite tasks execute once (map-execute-reduce); Relativist loops until Normal Form (potentially many rounds of split-reduce-merge).

### 3.2 ComputeTaskSplitAdapter: Automatic Load-Balanced Assignment

`ComputeTaskSplitAdapter<T, R>` hides the explicit node assignment of `map()` by providing a simpler `split()` method:

```java
public abstract class ComputeTaskSplitAdapter<T, R>
    extends ComputeTaskAdapter<T, R> {

    // User implements this: create jobs from input.
    // Node assignment is handled automatically by the framework.
    protected abstract Collection<? extends ComputeJob> split(
        int gridSize,
        T arg
    ) throws IgniteException;

    // map() is auto-implemented: distributes split() jobs
    // across nodes using the configured load balancing SPI.
}
```

The adapter takes the collection of jobs returned by `split()` and distributes them across the cluster using the configured load balancing strategy (round-robin by default). This is analogous to Relativist's approach where the coordinator decides partitioning (SPEC-04) and assigns partitions to workers based on a simple index mapping (SPEC-06, Section 4.6).

**Key difference:** Ignite's `split()` can create **any number** of jobs (potentially more jobs than nodes, enabling fine-grained load balancing). Relativist creates exactly `n` partitions for `n` workers (SPEC-04, R3). Ignite handles the many-to-few mapping via load balancing; Relativist uses a 1:1 partition-to-worker assignment.

### 3.3 Load Balancing Strategies

Ignite provides pluggable load balancing via `LoadBalancingSpi`:

| Strategy | Description | Mode |
|----------|------------|------|
| **RoundRobinLoadBalancingSpi** (default) | Sequential distribution across nodes. Two modes: per-task (random start, iterate) and global (single queue across all tasks). | Static |
| **WeightedRandomLoadBalancingSpi** | Random node selection with configurable weights (default weight: 10). Higher-weight nodes receive proportionally more jobs. | Static |
| **JobStealingCollisionSpi** | "Late" load balancing: under-utilized nodes steal queued jobs from over-utilized nodes. Requires `JobStealingFailoverSpi`. | Dynamic |
| **Affinity-based** | Collocated computations: `affinityRun()` / `affinityCall()` send tasks to nodes that hold specific cache keys. Not a general load balancer; specific to data-affinity scenarios. | Data-driven |

**Important limitation:** Load balancing does **not** apply to collocated computations (`affinityRun/affinityCall`), since those are explicitly targeted at specific data-owning nodes.

**Contrast with Relativist:** Relativist uses **static equal partitioning** (SPEC-04): the net is split into `n` partitions of approximately equal size, and partition `i` goes to worker `i`. There is no dynamic load balancing, no work stealing, and no data affinity. This is a deliberate simplicity choice for the TCC scope. Ignite's dynamic strategies (especially job stealing) are relevant for future versions where partition sizes may be unbalanced due to graph structure.

### 3.4 ClusterGroup: Node Selection and Topology Filtering

`ClusterGroup` defines a subset of cluster nodes for targeted task execution:

```java
// Execute on all server nodes (default)
IgniteCompute compute = ignite.compute();

// Execute only on remote nodes (exclude local)
IgniteCompute remote = ignite.compute(ignite.cluster().forRemotes());

// Execute on nodes matching a predicate
ClusterGroup filtered = ignite.cluster().forPredicate(
    node -> node.attribute("os") == "linux"
);
IgniteCompute linuxOnly = ignite.compute(filtered);
```

Filtering criteria include: OS type, CPU count, memory available, custom node attributes, data affinity (nodes holding specific cache partitions), and topology predicates.

**Contrast with Relativist:** Relativist has a flat worker list (SPEC-06, R21): the coordinator connects to all workers in the config and distributes partitions to all of them. There is no node filtering or topology-based selection. In a controlled lab environment with 8 identical machines (SPEC-07, SPEC-09), filtering is unnecessary. However, the concept of `ClusterGroup` is relevant if Relativist were to support heterogeneous environments in the future.

### 3.5 Task Sessions: Distributed Attribute Sharing

For each `ComputeTask` execution, Ignite creates a `ComputeTaskSession` that is visible to the task and all its jobs:

- **setAttribute(key, value):** Set a distributed attribute visible to all sibling jobs and the task itself.
- **waitForAttribute(key):** Block until a specific attribute is set by any sibling job (synchronous barrier).
- **ComputeTaskSessionAttributeListener:** Asynchronous notification when attributes change.

**Attribute ordering guarantee:** The sequence in which session attributes are set is consistent across all jobs. If job A sets attribute X before attribute Y, all other jobs observe X before Y. This ordering is also fault-tolerant (preserved across failover).

**Disabled by default:** For performance, distributed task session attributes require the `@ComputeTaskSessionFullSupport` annotation on the task class. Without it, the session exists but attribute distribution is disabled.

**Contrast with Relativist:** Relativist has no inter-worker communication during a round. Workers receive a partition, reduce it locally, and return the result. All coordination happens at the coordinator during the merge phase (SPEC-05). The closest analogue to task sessions is `GridMetrics` (SPEC-05, R34-R37), which accumulates execution statistics, but this is collected by the coordinator after each round, not shared between workers during execution. Ignite's task sessions enable a richer coordination model (e.g., synchronization barriers between jobs) that is irrelevant to Relativist's BSP-like execution model where workers are completely independent within a round.

### 3.6 Collocated Computations (Data Affinity)

Ignite's most distinctive feature is **collocated computation**: sending computation to where data lives, instead of moving data to computation.

```java
// Execute on the node that holds key=42 in "myCache"
ignite.compute().affinityRun("myCache", 42, () -> {
    // This code runs on the node that is the primary owner of key 42.
    // Can access the data locally (no network hop).
    cache.localPeek(42);
});

// Execute on all partitions of a cache
for (int part = 0; part < 1024; part++) {
    ignite.compute().affinityRun("myCache", part, () -> {
        // Process partition data locally
    });
}
```

The affinity function determines which node owns a given key or partition, and the compute grid routes the closure to that node. This eliminates network round-trips for data access.

**Contrast with Relativist:** Relativist's computation model is fundamentally different. In Relativist, the coordinator holds the entire Net in memory, partitions it (SPEC-04), and sends partitions to workers. Workers do not "own" persistent data; they receive a sub-net, reduce it, and return it. There is no concept of "sending computation to data" because data (the Net) is centralized at the coordinator and redistributed every round.

However, Ignite's affinity principle has an interesting theoretical connection to Relativist's partitioning strategy: both aim to maximize **local computation** and minimize **cross-boundary interaction**. Ignite achieves this by placing computation next to data; Relativist achieves this by cutting the graph along edges that are not active pairs, minimizing border redexes (SPEC-04, R9-R10). The optimization objective is the same (minimize communication), but the mechanism is completely different.

### 3.7 Fault Tolerance: Automatic Job Failover

Ignite provides comprehensive fault tolerance for compute tasks through the `FailoverSpi` mechanism:

**AlwaysFailoverSpi (default):** When a job fails (node crash, exception, timeout):
1. The framework calls `ComputeTask.result()`, which returns `FAILOVER`.
2. The `AlwaysFailoverSpi` selects a new target node:
   - **Preferred:** A node that has NOT executed any other job from the same task (to avoid concentration).
   - **Fallback:** A node already running other jobs from the same task.
3. The job is re-serialized and sent to the new node for re-execution.
4. Maximum failover attempts: 5 (configurable). If exhausted, the job fails permanently.

**NeverFailoverSpi:** Disables failover entirely. Failed jobs immediately fail.

**JobStealingFailoverSpi:** Specialized for work-stealing load balancing.

**Guaranteed execution:** Ignite states: "All jobs and closures are guaranteed to be executed as long as there is at least one node standing." This is a strong guarantee enabled by the combination of automatic failover and the assumption that jobs are **idempotent** (re-executing a failed job produces the same result).

**Contrast with Relativist:** Relativist v1 has **no fault tolerance** (SPEC-07, R44; DISC-007 v2). If a worker crashes mid-reduction, the coordinator's `collect` phase times out (SPEC-06, R30: 600s), and the entire computation fails. This is an explicit scope decision for the TCC.

However, Ignite's failover model is theoretically compatible with Relativist's semantics. Because IC reduction has **strong confluence** (SPEC-01, T4), re-reducing a partition on a different worker produces the same result. If Relativist adopted failover in v2+, the coordinator could detect a worker failure, re-send the partition to another worker (or reduce it locally), and the overall computation would remain correct. This is a stronger guarantee than Ignite's reliance on idempotent jobs, because IC strong confluence mathematically guarantees identical results regardless of reduction order.

### 3.8 Distributed Closures and Broadcast

For simpler use cases that do not need the full MapReduce lifecycle, Ignite provides closure-based execution:

```java
// Execute a callable on a single node (load-balanced)
Integer result = ignite.compute().call(() -> {
    return heavyComputation();
});

// Execute a closure on each element, across nodes
Collection<Integer> results = ignite.compute().apply(
    word -> word.length(),       // closure
    Arrays.asList("hello", "world")  // input collection
);

// Broadcast: execute on ALL nodes
ignite.compute().broadcast(() -> {
    System.out.println("Running on node: " + ignite.cluster().localNode().id());
});
```

**Broadcast** executes the same closure on every node in the cluster group. This has no direct analogue in Relativist: Relativist sends **different** data (a unique partition) to each worker, not the same data to all workers.

**Timeout control:** `ignite.compute().withTimeout(5000)` sets a maximum execution time. If exceeded, all spawned jobs are cancelled.

---

## 4. Comparison with Relativist's Context

### 4.1 Fundamental Computation Model

| Dimension | Apache Ignite | Relativist |
|-----------|--------------|------------|
| **Paradigm** | MapReduce (split-execute-reduce) | Iterative BSP-like graph reduction (split-reduce-merge-repeat) |
| **Job independence** | Jobs are independent (no shared state unless task sessions used) | Partitions are coupled by border wires |
| **Iteration** | Single-shot (one map-execute-reduce cycle per task) | Multi-round loop until Normal Form |
| **Result aggregation** | User-defined reduce function (simple aggregation) | Graph merge + border redex resolution (complex structural operation) |
| **Correctness guarantee** | Programmer's responsibility (no framework guarantee) | Strong confluence: `reduce_all(net) == extract_result(run_grid(net, n))` (SPEC-01, G1) |
| **Data model** | Key-value pairs in distributed caches | Interaction net graph (agents + wires) in arena |
| **Data location** | Distributed across nodes (affinity-based) | Centralized at coordinator, dispatched per round |
| **Communication** | P2P mesh (any node to any node) | Star (workers only talk to coordinator) |

### 4.2 Split/Map/Reduce vs. Split/Distribute/Reduce/Collect/Merge

Ignite's three-phase model and Relativist's five-phase cycle are structurally similar but semantically different:

```
IGNITE:
  map(nodes, arg)        --> {Job1: Node1, Job2: Node2, ...}
  [jobs execute on nodes] --> [Result1, Result2, ...]
  reduce(results)        --> FinalResult

RELATIVIST (per round):
  partition(net, n)       --> [Partition1, Partition2, ..., PartitionN]
  distribute(partitions)  --> [send partition_i to worker_i]
  reduce_local(partition) --> [reduced_partition on each worker]  (parallel)
  collect(results)        --> [ReducedPartition1, ..., ReducedPartitionN]
  merge(partitions)       --> merged_net  (restore borders, detect border redexes)
  reduce_all(merged_net)  --> net_after_border_resolution
  [if redexes remain: repeat from partition()]
```

The critical distinction is in the "reduce" semantics:
- **Ignite's reduce:** A pure aggregation function. Each job result is a self-contained value. The reduce combines them (sum, merge lists, build map, etc.). No structural relationship between job results.
- **Relativist's merge:** A structural reconstruction. Each partition result contains agents with FreePort boundary sentinels that must be resolved by reconnecting border wires. New redexes may emerge at boundaries (SPEC-05, R12). The merge must preserve the graph's linearity invariant (SPEC-01, T1; SPEC-02, I1).

### 4.3 Architecture Topology

| Aspect | Apache Ignite | Relativist |
|--------|--------------|------------|
| **Topology** | P2P mesh (ring for discovery, full mesh for communication) | Star (coordinator-centric) |
| **Node roles** | Server (data+compute), Client (entry point) | Coordinator (state+orchestration), Worker (stateless executor) |
| **Discovery** | Automatic (TcpDiscoverySpi ring, IP finder) | Manual (config file with addresses, SPEC-06 R21) |
| **Coordinator** | Dynamic (oldest node), changes on failure | Fixed (configured at startup), single point of failure |
| **Communication** | TCP with multiple SPIs, per-operation connections | Persistent TCP with length-prefixed bincode frames (SPEC-06) |
| **Ports** | 47500 (discovery) + 47100 (communication) | Single port (coordinator listens, workers connect) |

### 4.4 Fault Tolerance Comparison

| Aspect | Apache Ignite | Relativist v1 |
|--------|--------------|---------------|
| **Node failure** | Automatic detection (ring heartbeat) + automatic failover | Timeout-based detection (600s), no recovery |
| **Job failure** | Re-execute on another node (AlwaysFailoverSpi) | Entire computation fails |
| **Max retries** | Configurable (default: 5) | None |
| **Idempotency** | Assumed by framework | Guaranteed by strong confluence (SPEC-01, T4) |
| **Topology changes** | Dynamic: nodes join/leave, PME redistributes data | Static: worker set fixed for entire computation |
| **State persistence** | Optional native persistence + WAL | None (in-memory only) |

### 4.5 Data Distribution

| Aspect | Apache Ignite | Relativist |
|--------|--------------|------------|
| **Partitioning** | Hash-based (rendezvous hashing), fixed 1024 partitions | Graph-based (ID-range + redex-aware), `n` partitions for `n` workers (SPEC-04) |
| **Partition granularity** | Key-level (each key hashes to a partition) | Agent-level (groups of agents form a sub-net) |
| **Border semantics** | None (partitions are independent key ranges) | FreePort sentinels mark cut edges (SPEC-04, R7-R8) |
| **Re-partitioning** | On topology change only (PME + rebalance) | Every round (re-partition the full net) |
| **Data locality** | Persistent (keys stay on assigned nodes) | Transient (partitions sent fresh each round) |
| **Affinity** | Data-to-node affinity via hash function | None (coordinator assigns arbitrarily) |

---

## 5. Lessons for Relativist (ADOPT / ADAPT / REJECT)

### L1. ComputeTask Three-Phase Model (map/result/reduce) -- ADAPT

**Ignite mechanism:** The `ComputeTask` interface cleanly separates concerns: `map()` handles splitting and assignment, `result()` handles per-job feedback, `reduce()` handles aggregation. The `ComputeTaskSplitAdapter` further simplifies by separating splitting from node assignment.

**Relevance to Relativist:** Relativist's grid cycle (SPEC-05) already has a similar phase separation: `partition()`, `distribute()`, `reduce_local()`, `collect()`, `merge()`. But the phases are implicit in the `run_grid` function rather than formalized as a trait or interface.

**Adaptation:** SPEC-13 (System Architecture) should consider formalizing the grid cycle phases as distinct, named operations with clear input/output types. This does NOT mean adopting Ignite's `ComputeTask` interface (which assumes independent jobs), but rather ensuring that each phase has a well-defined signature:

```
partition : (Net, GridConfig) -> PartitionPlan
distribute : (PartitionPlan, Workers) -> ()       // sends partitions
collect    : (Workers) -> Vec<Partition>           // receives results
merge      : (Vec<Partition>, BorderMap) -> Net
check_done : (Net) -> bool                        // Normal Form?
```

This formalization aids testability (each phase can be tested independently) and future extensibility.

**Verdict: ADAPT.** Formalize the grid cycle phases as named operations in SPEC-13, inspired by Ignite's clean separation but adapted to Relativist's graph semantics.

### L2. ComputeTaskSplitAdapter Automatic Load Balancing -- REJECT for v1, ADAPT for v2+

**Ignite mechanism:** `ComputeTaskSplitAdapter.split()` returns a collection of jobs. The framework automatically assigns jobs to nodes using the configured load balancing SPI. The programmer does not choose which node runs which job.

**Relevance to Relativist:** In v1, Relativist uses a 1:1 static mapping: partition `i` goes to worker `i` (SPEC-04, R3; SPEC-06). This is appropriate for 8 identical machines running a TCC experiment.

**Why reject for v1:** Interaction net partitions are not interchangeable. Each partition contains a specific set of agents and border wires. Sending partition 3 to worker 7 instead of worker 3 changes nothing about correctness (strong confluence), but the 1:1 mapping simplifies debugging and metrics attribution.

**Why adapt for v2+:** In a heterogeneous or dynamically-sized cluster, automatic partition-to-worker mapping based on worker load, partition size, or network proximity would improve utilization. The coordinator could implement a simple assignment strategy (e.g., assign larger partitions to faster workers) without requiring Ignite's full SPI framework.

**Verdict: REJECT for v1. ADAPT for v2+** with a simpler assignment heuristic.

### L3. Job Stealing for Dynamic Load Balancing -- REJECT for v1, ADAPT for v2+

**Ignite mechanism:** `JobStealingCollisionSpi` allows under-utilized nodes to steal queued jobs from over-utilized nodes, rebalancing work after initial assignment.

**Relevance to Relativist:** In Relativist's BSP-like model, all workers must complete their partition before the next round begins (SPEC-05, R27-R28). The round bottleneck is the **slowest worker**. If partitions are unbalanced (some workers finish quickly, others are slow), the fast workers idle while waiting.

**Why reject for v1:** Work stealing requires workers to communicate with each other (or with a central job queue), which breaks Relativist's star topology (SPEC-06). Additionally, a single partition is a monolithic graph reduction task; it cannot be "stolen" mid-execution without complex checkpointing.

**Why adapt for v2+:** Instead of job stealing, Relativist could adopt **adaptive partitioning**: if round N reveals that worker A finished in 10ms and worker B in 500ms, round N+1 could assign worker A a larger partition. This achieves the same goal (balanced load) without runtime work migration.

**Verdict: REJECT for v1. ADAPT for v2+** as adaptive partitioning, not true work stealing.

### L4. ClusterGroup Node Filtering -- REJECT for v1

**Ignite mechanism:** `ClusterGroup` filters nodes by attributes (OS, memory, CPU, custom tags) for targeted task execution.

**Relevance to Relativist:** Relativist operates on a fixed, homogeneous set of 8 machines (SPEC-07, SPEC-09). All workers are identical. There is nothing to filter.

**Verdict: REJECT.** The problem domain does not apply for v1.

### L5. Task Sessions for Inter-Job Communication -- REJECT

**Ignite mechanism:** `ComputeTaskSession` enables jobs to share attributes and synchronize via `waitForAttribute()`.

**Relevance to Relativist:** Relativist workers are completely isolated during a round (SPEC-05, SPEC-06). There is no inter-worker communication by design. The coordinator handles all coordination during the merge phase. Introducing inter-worker communication would violate the BSP-like model and complicate the correctness argument (SPEC-01, ARG-001).

**Verdict: REJECT.** Fundamentally incompatible with Relativist's isolation model.

### L6. Collocated Computation (Data Affinity) -- REJECT

**Ignite mechanism:** `affinityRun/affinityCall` send computation to nodes that hold specific cached data, eliminating network transfers.

**Relevance to Relativist:** Relativist's data (the Net) is centralized at the coordinator. Workers do not persistently hold data between rounds. The coordinator partitions the net fresh each round and sends partitions to workers. There is no persistent data locality to exploit.

**Theoretical note:** If Relativist evolved to keep partitions on workers across rounds (avoiding the re-send cost), data affinity would become relevant: the coordinator could detect which agents are still on which worker and re-partition to maximize locality. However, this would require workers to maintain state between rounds, fundamentally changing the architecture.

**Verdict: REJECT.** Incompatible with Relativist's stateless worker model.

### L7. Automatic Failover with Topology-Aware Routing -- ADAPT for v2+

**Ignite mechanism:** `AlwaysFailoverSpi` automatically re-routes failed jobs to nodes that haven't executed other jobs from the same task, with configurable retry limits.

**Relevance to Relativist:** Relativist v1 has no failover (SPEC-07, R44). However, Ignite's approach is directly applicable to a future Relativist version:
- The coordinator retains the original partition data until the worker responds (SPEC-06).
- If a worker fails, the coordinator can re-send the partition to another worker (or reduce it locally).
- Strong confluence guarantees that re-reducing a partition produces the same result (stronger than Ignite's assumption of idempotent jobs).
- The coordinator can track which workers have failed and exclude them from future rounds.

**Adaptation:** For v2+, implement a simple failover: if `collect_timeout` expires for a worker, mark it as failed, re-send its partition to the least-loaded surviving worker (or reduce locally on the coordinator). Maximum retry: configurable (like Ignite's default of 5).

**Verdict: ADAPT for v2+.** The mechanism is sound and particularly well-suited to Relativist thanks to strong confluence.

### L8. P2P Communication Topology -- REJECT for v1

**Ignite mechanism:** Full mesh P2P communication where any node can send messages to any other node, using `TcpCommunicationSpi`.

**Relevance to Relativist:** Relativist uses a strict star topology (SPEC-06) where workers only communicate with the coordinator. P2P worker-to-worker communication would enable:
- Direct border redex resolution between adjacent partitions (without routing through the coordinator).
- Distributed merge (each pair of adjacent partitions merges locally).

However, this would dramatically complicate the correctness argument, the protocol, and the implementation. The star topology is the right choice for a TCC prototype.

**Verdict: REJECT for v1.** Star topology is simpler and sufficient.

### L9. Ring-Based Discovery Protocol -- REJECT

**Ignite mechanism:** `TcpDiscoverySpi` forms a ring of server nodes for membership management, with automatic failure detection via neighbor monitoring.

**Relevance to Relativist:** Relativist uses manual configuration: the coordinator's address and port are specified in a config file, and workers are listed explicitly (SPEC-06, R21; SPEC-07, R10). There is no dynamic discovery. For 8 machines in a lab, manual configuration is simpler and more predictable.

**Verdict: REJECT.** Automatic discovery adds complexity without benefit for the TCC scope.

### L10. Partition Map Exchange (PME) for Topology Consistency -- ADAPT (concept only)

**Ignite mechanism:** PME ensures all nodes have a consistent view of data distribution after topology changes. The coordinator collects partition information from all nodes, creates a complete map, and redistributes it.

**Relevance to Relativist:** Relativist's coordinator already performs an analogous function: it creates a `PartitionPlan` (SPEC-04) that contains the full border map, and distributes partitions to workers. Workers do not need a global view because they only process their local partition. However, the concept of ensuring a **consistent global view** before proceeding with computation is a sound principle.

**Adaptation:** SPEC-13 should formalize the coordinator's responsibility to ensure that all workers have received their partition data (acknowledgement) before the reduction phase begins. This is already implied by SPEC-06's protocol (send partition, wait for result), but an explicit "all workers ready" barrier would make the BSP semantics clearer.

**Verdict: ADAPT (concept only).** Formalize the "all partitions dispatched" barrier in SPEC-13.

### L11. Off-Heap Memory Management -- REJECT for v1, NOTE for future

**Ignite mechanism:** Ignite uses off-heap memory regions managed independently of the JVM garbage collector, avoiding GC pauses.

**Relevance to Relativist:** Relativist is written in Rust, which has no garbage collector. Memory management is deterministic (ownership + borrowing). The arena-based allocation in SPEC-02 (`Vec<Option<Agent>>`) already provides predictable memory behavior. There is no GC pause problem to solve.

**Note:** Rust's allocator is already "off-heap" by default. The closest Ignite equivalent in Rust would be a custom allocator for the arena (e.g., `jemalloc` for reduced fragmentation), but this is a performance optimization, not an architectural decision.

**Verdict: REJECT.** Not applicable to Rust's memory model.

### L12. Pluggable SPI Architecture -- ADAPT (concept only)

**Ignite mechanism:** Ignite uses Service Provider Interfaces (SPIs) for discovery, communication, failover, load balancing, and collision resolution. Each SPI can be swapped independently.

**Relevance to Relativist:** Relativist already uses Rust traits for extensibility: `PartitionStrategy` (SPEC-04, R21) is a trait that allows pluggable partitioning algorithms. SPEC-13 should extend this pattern to other subsystems where pluggability is valuable (e.g., serialization format, transport protocol).

**Adaptation:** Define traits (Rust equivalent of SPIs) for the major subsystem boundaries in SPEC-13. This does not mean implementing multiple strategies in v1; it means designing interfaces that allow future extension without refactoring.

**Verdict: ADAPT (concept only).** Extend the trait-based extensibility pattern from SPEC-04 to other subsystems in SPEC-13.

---

## 6. Comparison Table (Apache Ignite vs Relativist)

| Dimension | Apache Ignite | Relativist | Notes |
|-----------|--------------|------------|-------|
| **Year / Maturity** | 2010-present (GridGain), ASF top-level since 2015 | 2026, TCC prototype | Production vs research |
| **Language** | Java (+ C#, C++, SQL) | Rust | |
| **Computation model** | MapReduce (map/execute/reduce) | Iterative BSP-like graph reduction | Fundamentally different iteration model |
| **Network topology** | P2P mesh (ring discovery + full mesh comm) | Star (coordinator-centric) | Ignite is more scalable; Relativist is simpler |
| **Communication protocol** | TCP with pluggable SPIs | TCP + length-prefixed bincode (SPEC-06) | Ignite: heavier, more flexible. Relativist: lean, purpose-built |
| **Node roles** | Server (data+compute) / Client (entry) | Coordinator (state+orchestration) / Worker (stateless) | Ignite: symmetric. Relativist: asymmetric |
| **Discovery** | Automatic (TcpDiscoverySpi ring) | Manual (config file) | Different complexity requirements |
| **Load balancing** | Pluggable: round-robin, weighted, job stealing | Static 1:1 partition-to-worker | Ignite: dynamic. Relativist: simple/static |
| **Fault tolerance** | Comprehensive (auto failover, max 5 retries) | None in v1 | Biggest operational gap |
| **Idempotency / Determinism** | Assumed by framework | Guaranteed by strong confluence (SPEC-01) | Relativist has a stronger theoretical guarantee |
| **Data model** | Key-value pairs in distributed caches | Interaction net graph in arena (SPEC-02) | Completely different data abstractions |
| **Data partitioning** | Hash-based (rendezvous), fixed 1024 partitions | Graph-based (ID-range + redex-aware), dynamic per round | Ignite: static assignment. Relativist: re-partitions every round |
| **Data locality** | Persistent (keys stay on nodes) | Transient (partitions sent each round) | Ignite: long-lived caches. Relativist: ephemeral per-round |
| **Collocated computation** | Core feature (affinityRun/affinityCall) | Not applicable (stateless workers) | Ignite's defining capability |
| **Inter-job communication** | Task sessions (setAttribute/waitForAttribute) | None (workers isolated within round) | Different coordination models |
| **Result aggregation** | User-defined reduce function | Graph merge + border redex resolution (SPEC-05) | Relativist's merge is a complex structural operation |
| **Iteration** | Single-shot per task | Multi-round loop until Normal Form | Critical architectural difference |
| **Memory management** | Off-heap (avoid JVM GC pauses) | Rust ownership (no GC) | Not applicable to Rust |
| **Scale target** | Enterprise clusters (tens to thousands of nodes) | 8 physical machines (TCC scope) | Different orders of magnitude |
| **Trust model** | Trusted (managed infrastructure) | Trusted (controlled lab) | Both operate in trusted environments |
| **Serialization** | Java serialization / custom | serde + bincode (SPEC-06) | bincode is faster and more compact |
| **Persistence** | Optional WAL + native persistence | None (in-memory only) | Different durability requirements |
| **Configuration** | XML / Java API / Spring | CLI + config file (SPEC-07) | Different complexity levels |
| **Deployment** | Multi-node with auto-discovery | Single binary + Docker (SPEC-07) | Relativist: minimal operational burden |

---

## 7. Sources

### Apache Ignite Official Documentation

- [Distributed Computing Overview](https://ignite.apache.org/docs/latest/distributed-computing/distributed-computing)
- [MapReduce API](https://ignite.apache.org/docs/latest/distributed-computing/map-reduce)
- [Collocated Computations](https://ignite.apache.org/docs/latest/distributed-computing/collocated-computations)
- [Load Balancing](https://ignite.apache.org/docs/latest/distributed-computing/load-balancing)
- [Fault Tolerance](https://ignite.apache.org/docs/latest/distributed-computing/fault-tolerance)
- [Data Partitioning](https://ignite.apache.org/docs/latest/data-modeling/data-partitioning)
- [Network Configuration](https://ignite.apache.org/docs/latest/clustering/network-configuration)
- [In-Memory Data Grid Use Case](https://ignite.apache.org/use-cases/in-memory-data-grid.html)

### Apache Ignite Legacy Documentation (readme.io)

- [Compute Grid Overview](https://apacheignite.readme.io/docs/compute-grid)
- [MapReduce & ForkJoin](https://apacheignite.readme.io/docs/compute-tasks)
- [Fault Tolerance](https://apacheignite.readme.io/docs/fault-tolerance)
- [Data Grid](https://apacheignite.readme.io/docs/data-grid)
- [Partitioning and Replication](https://apacheignite.readme.io/docs/cache-modes)
- [Clients and Servers](https://apacheignite.readme.io/docs/clients-vs-servers)

### Apache Ignite Javadoc

- [ComputeTask (Ignite 2.17.0)](https://ignite.apache.org/releases/latest/javadoc/org/apache/ignite/compute/ComputeTask.html)
- [ComputeTaskSplitAdapter (Ignite 2.17.0)](https://ignite.apache.org/releases/latest/javadoc/org/apache/ignite/compute/ComputeTaskSplitAdapter.html)
- [AlwaysFailoverSpi (Ignite 2.17.0)](https://ignite.apache.org/releases/latest/javadoc/org/apache/ignite/spi/failover/always/AlwaysFailoverSpi.html)

### Source Code

- [ComputeTask.java (GitHub)](https://github.com/apache/ignite/blob/master/modules/core/src/main/java/org/apache/ignite/compute/ComputeTask.java)
- [ComputeTaskMapExample.java (GitHub)](https://github.com/apache/ignite/blob/master/examples/src/main/java/org/apache/ignite/examples/computegrid/ComputeTaskMapExample.java)

### Encyclopedia

- [Apache Ignite (Wikipedia)](https://en.wikipedia.org/wiki/Apache_Ignite)

### Community / Architecture Analysis

- [TCP Discovery SPI Under the Hood (Apache Wiki)](https://cwiki.apache.org/confluence/display/IGNITE/TCP+Discovery+SPI+under+the+hood)
- [A Guide to Apache Ignite (Baeldung)](https://www.baeldung.com/apache-ignite)
