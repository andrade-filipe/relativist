# Roadmap

This document lists features explicitly excluded from v1 (SPEC-13, R49-R50) and future architectural evolutions. Items marked with **(confluence-enabled)** are made possible specifically by the strong confluence property of Interaction Combinators -- they would be incorrect or require complex consensus in systems without this guarantee.

---

## v1 Scope (TCC)

v1 implements the minimum architecture that validates the TCC hypothesis: a single coordinator with K static workers in a star topology, using BSP (Bulk Synchronous Parallel) with barrier synchronization. This architecture is deliberately simple because the research question is about **correctness of distributed IC reduction**, not about grid infrastructure sophistication.

---

## v2 — Elastic Grid Architecture **(confluence-enabled)**

The central insight: strong confluence (Lafont 1997, Property P1 in ARG-001) guarantees that the result of reduction is identical regardless of **who** reduces **what** and in **what order**. This means work can be freely redistributed at any point without affecting correctness. v1 does not exploit this; v2 would.

### 2.1 Coordinator as Worker (hybrid node)

**v1 limitation:** The coordinator only orchestrates (partition, dispatch, collect, merge). It does not reduce. If only 1 machine is available, it waits for workers and does nothing.

**v2 change:** The coordinator keeps one partition for itself and reduces it locally while workers reduce theirs. When all workers return, the coordinator merges all partitions including its own.

**Why confluence makes this safe:** The coordinator's local reduction and the workers' reductions are independent. Strong confluence guarantees the merged result is identical to sequential `reduce_all` on the full net, regardless of which node reduced which partition.

**Complexity:** Low. The coordinator's BSP loop adds one local `reduce_all` call in parallel with the collect phase. No protocol changes needed -- the coordinator simply doesn't dispatch one partition and reduces it itself.

**Impact:** A single machine can start useful work immediately. With K machines total, the effective parallelism is K (not K-1 as in v1).

### 2.2 Dynamic Worker Joining **(confluence-enabled)**

**v1 limitation:** Worker count is fixed at startup (SPEC-06, R24). The coordinator waits for exactly K workers, then starts. No workers can join or leave during execution.

**v2 change:** Workers can join between BSP rounds (at the barrier synchronization point). When a new worker connects:

1. The current round completes normally with the existing workers.
2. At the next barrier, the coordinator accepts the new worker.
3. The net is re-partitioned with K+1 workers for the next round.

**Why confluence makes this safe:** Between rounds, the coordinator holds the complete merged net. Re-partitioning this net for K+1 workers is exactly the same operation as the initial partition -- strong confluence guarantees that reducing K+1 partitions produces the same result as reducing K partitions. The net's reduction history is irrelevant; only its current state matters.

**Practical scenario:** A user launches a large reduction on their local machine (coordinator + self-worker per 2.1). Later, 7 more machines become available and join. The work is automatically redistributed without restarting.

**Complexity:** Medium. Requires:
- New coordinator FSM state: `AcceptingWorkers` between rounds.
- Dynamic partition count in `split()`.
- Worker registration protocol that works mid-execution (not just at startup).
- Graceful handling of the K=0 case (coordinator reducing alone until someone joins).

Does NOT require:
- Consensus (single coordinator decides).
- State transfer to new workers (they receive a fresh partition like any other worker).
- Changes to the wire protocol messages (same `AssignPartition` / `PartitionResult`).

### 2.3 Dynamic Worker Departure **(confluence-enabled)**

**v1 limitation:** If a worker disconnects, the execution fails. No fault tolerance (OBJETIVO_TCC.md: out of scope).

**v2 change:** If a worker disconnects mid-round, the coordinator can:
1. Wait for timeout.
2. Reclaim the lost partition (the coordinator sent a copy, so it still has it).
3. Re-partition the lost work among remaining workers in the next round.

**Why confluence makes this safe:** The lost worker may have partially reduced its partition, but since the coordinator retains the original, it can simply re-dispatch the unreduced partition. Strong confluence guarantees that re-reducing from the original state produces the correct result. No rollback protocol needed.

**Complexity:** Medium-High. Adds timeout handling, partition retention, and re-dispatch logic. The "retain original partition" strategy trades memory for simplicity.

### 2.4 Distributed Partitioning and Merge **(confluence-enabled)**

**v1 limitation:** The coordinator is the sole point of aggregation (SPEC-13, R35). ALL partitions return to the coordinator for merge. For very large nets, this is a bottleneck: O(A) data flows through a single node every round.

**v2 change:** Hierarchical coordination. Instead of a flat star topology:

```
v1 (star):           v2 (tree):
    C                     C
   /|\                   / \
  W W W               SC    SC
                      / \   / \
                     W   W W   W
```

Sub-coordinators (SC) merge their local group's partitions and send the partial result up. The root coordinator merges the sub-results.

**Why confluence makes this safe:** Merge is associative under strong confluence. `merge(merge(P1, P2), merge(P3, P4))` produces the same net as `merge(P1, P2, P3, P4)`. The order and grouping of merges does not affect the final result.

**Complexity:** High. Requires:
- Tree topology construction and management.
- Sub-coordinator role (new node type or hybrid mode).
- Distributed border resolution (border redexes between sub-groups must be resolved at the appropriate tree level).
- Distributed termination detection (the root must know when ALL sub-trees have reached normal form).
- O(n^2) potential border interactions in the worst case (SPEC-05, Section 5.1: peer-to-peer assessment).

**Assessment:** This is a research project in its own right. The TCC should mention it as future work enabled by confluence, but implementing it would require a separate effort.

---

## v2 — Other Features

### 2.5 Coordinator as Worker (without being coordinator)

Flip the model: any machine can submit a net for reduction, and the grid itself decides who coordinates. This requires leader election (Raft, Paxos, or simpler alternatives for a known-membership grid).

**v1 exclusion source:** SPEC-13 R49 (Consensus protocols).

### 2.6 Work Stealing

Workers that finish early steal work from slower workers. Incompatible with BSP barrier synchronization (SPEC-13 R49), but could work in an asynchronous model.

**v1 exclusion source:** SPEC-13 R49 (Work stealing), PESQ-011 L1.

### 2.7 Intra-Worker Parallelism

Workers use `rayon` to reduce their partition with multiple threads. Each worker already has a disjoint ID range (SPEC-04 R16-R20), which could be further subdivided for thread-local allocation.

**v1 exclusion source:** SPEC-13 R49 (rayon intra-worker), PESQ-011 L2.

### 2.8 Automatic Node Discovery

Replace manual `--coordinator HOST:PORT` with multicast/DNS-SD/Consul-based discovery.

**v1 exclusion source:** SPEC-07, Section 5.5 (manual discovery sufficient for 8 machines on LAN).

### 2.9 Fault Tolerance

Checkpointing, re-dispatch on failure, worker health monitoring. Builds on 2.3 (dynamic departure) with persistence.

**v1 exclusion source:** OBJETIVO_TCC.md (out of scope), SPEC-13 R49 (Byzantine fault tolerance).

### 2.10 Multi-Tenancy and Job Queuing

Multiple users submit nets for reduction. The grid schedules and executes them, possibly concurrently on different subsets of workers.

### 2.11 Intelligent Partitioning

Replace round-robin with redex-aware or graph-aware partitioning strategies that minimize border redexes.

**v1 exclusion source:** SPEC-09 R29 (MAY for alternative strategies).

### 2.12 GPU Workers

Heterogeneous compute: some workers reduce on GPU (following HVM2's approach).

### 2.13 Visualization

Graphviz export of net state, live progress dashboard, reduction animation.

### 2.14 WASM Target

Browser-based IC reduction for education and demonstration.

---

## The Confluence Argument for the Paper

The features in Section 2 (2.1-2.4) share a common theoretical foundation that should be presented in the TCC's Discussion section (Section 5):

> Strong confluence does not merely guarantee correctness for a fixed distributed configuration. It guarantees correctness under **any** redistribution of work at **any** point during reduction. This means:
>
> - A coordinator can participate in reduction without affecting the result (2.1).
> - Workers can join mid-execution and receive fresh partitions without invalidating prior work (2.2).
> - Workers can depart and their unreduced partitions can be re-dispatched without rollback (2.3).
> - Merge can be performed hierarchically because it is associative under confluence (2.4).
>
> These properties are unique to Interaction Combinators among distributed reduction models. Systems based on lambda calculus or term rewriting require explicit confluence checks or deterministic scheduling to achieve the same guarantees.
>
> v1 validates the fundamental property (distributed reduction = sequential reduction) with the simplest architecture. The architectural evolutions above are not speculative -- they are direct corollaries of the same property, requiring only engineering effort, not new theoretical results.

This argument strengthens the TCC's contribution: the prototype validates the foundation, and the roadmap demonstrates the breadth of what that foundation enables.
