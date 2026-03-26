---
pesq_id: PESQ-011
title: "Work-Stealing Scheduling Patterns"
category: System Design Patterns
date: 2026-03-26
status: Complete
cross_references:
  specs: [SPEC-05, SPEC-13]
  pesqs: [PESQ-003, PESQ-004, PESQ-010]
  discs: [DISC-006]
---

# PESQ-011: Work-Stealing Scheduling Patterns

**Category:** System Design Patterns
**Status:** Complete
**Cross-references:**
- Specs: SPEC-05 (merge/grid cycle), SPEC-13 (system architecture)
- PESQs: PESQ-003 (Ray), PESQ-004 (Dask), PESQ-010 (coordinator-worker)
- Discussions: DISC-006 v2 (communication overhead and granularity)

---

## 1. Subject Overview

Work stealing is a scheduling strategy where idle processors "steal" work from busy processors' queues. Originated in Multilisp (1980s) and formalized by Blumofe & Leiserson (1999), it's used in Cilk, Java Fork/Join, .NET Task Parallel Library, and Rust's tokio and rayon.

The core insight: instead of centralized task assignment, each processor maintains a local deque (double-ended queue). When a processor's deque is empty, it randomly selects another processor and steals from the bottom of that processor's deque.

**Theoretical bound:** Expected execution time is T₁/P + O(T∞), where T₁ is serial time and T∞ is critical path length.

---

## 2. How Work Stealing Works

### 2.1 Core Algorithm

```
Each processor P_i has a local deque D_i:

loop:
  if D_i is not empty:
    task = D_i.pop_top()    // execute own work (LIFO)
    execute(task)
  else:
    victim = random_select(processors - {P_i})
    task = victim.D.steal_bottom()  // steal from bottom (FIFO)
    if task != null:
      execute(task)
    else:
      yield()  // no work available anywhere
```

**Key insight:** The asymmetry between pop (LIFO from top) and steal (FIFO from bottom) is deliberate:
- LIFO execution: locality-friendly (recently created tasks share cache)
- FIFO stealing: steals the largest subtask (created earlier = higher in tree)

### 2.2 Variants

| Variant | Description | Used by |
|---------|-------------|---------|
| **Child stealing** | Spawned child goes to parent's deque; parent continues | Library implementations (easier) |
| **Continuation stealing** | Parent's continuation goes to deque; child executes | Cilk (requires compiler support) |
| **Adaptive stealing** | Adjusts steal frequency based on load | Distributed systems |
| **Leapfrogging** | Steal from processor that stole from you | Reduced communication |

### 2.3 Work Stealing in Distributed Systems

In distributed settings, work stealing faces additional challenges:
- **Network latency:** Stealing across nodes is orders of magnitude slower than local
- **Data locality:** Stolen tasks may need data from the victim's memory
- **Load information:** Global load state is expensive to maintain

**Dask's approach** (PESQ-004): The centralized scheduler maintains global task state and proactively rebalances by moving tasks from overloaded workers to idle ones. This is "coordinator-mediated work stealing" rather than peer-to-peer stealing.

---

## 3. Relevance to Relativist

### 3.1 Why Work Stealing Doesn't Fit BSP

Relativist uses BSP (Bulk Synchronous Parallel):
1. **Partition phase:** Coordinator splits net into k partitions
2. **Compute phase:** All workers reduce in parallel
3. **Barrier:** Wait for all workers to finish
4. **Merge phase:** Coordinator merges all results

Work stealing is fundamentally **asynchronous** — there's no barrier. A slow worker's tasks get redistributed dynamically. But in BSP:
- All workers must complete before merge (barrier)
- Partitions are **not arbitrarily splittable** — splitting a partition mid-reduction would create new border redexes
- The coordinator already knows the global state (no need for decentralized decisions)

**Verdict: Work stealing is INCOMPATIBLE with Relativist's BSP model.**

### 3.2 Where Work-Stealing Concepts Could Apply

Despite the model mismatch, two concepts are adaptable:

#### A. Intra-Worker Parallelism (rayon)
Within a single worker, the reduction of a partition could use rayon's work-stealing thread pool to parallelize independent redex reductions. Since all redexes within a partition that don't share agents can be reduced in parallel, work stealing at the thread level is sound.

**However:** SPEC-03 specifies sequential reduction within a worker (simpler, deterministic). Parallel intra-worker reduction is a v2 optimization, not v1.

#### B. Load-Aware Partition Sizing
The coordinator can observe worker completion times and adjust partition sizes for the next round:
- Fast workers get larger partitions
- Slow workers get smaller partitions
- This is "preventive load balancing" rather than reactive stealing

This aligns with DISC-006's analysis of adaptive granularity.

### 3.3 Why Round-Robin Dispatch is Sufficient for v1

For Profile A (embarrassingly parallel) workloads, partitions are roughly equal in work. Round-robin dispatch produces balanced load without any stealing.

For Profile B (expansion/collapse) and Profile C (sequential dependency), load imbalance exists. But the BSP barrier means the round is bounded by the slowest worker regardless — stealing doesn't help because the barrier prevents useful preemption.

The correct mitigation is **better partitioning** (SPEC-04), not runtime stealing.

---

## 4. Comparison Table

| Dimension | Work Stealing | BSP (Relativist) | Assessment |
|-----------|--------------|-------------------|------------|
| Scheduling | Decentralized, reactive | Centralized, proactive | Different paradigms |
| Granularity | Fine (individual tasks) | Coarse (partitions per round) | BSP is coarser |
| Load balancing | Dynamic (steal when idle) | Static per round (partition sizing) | BSP simpler |
| Barrier | None | Yes (mandatory per round) | Incompatible |
| Data movement | Task + data stolen together | Data dispatched by coordinator | Different flows |
| Fault tolerance | Victim crash = lost tasks | Worker crash = re-dispatch partition | Both handle crashes |
| Communication | Peer-to-peer steal requests | Star topology through coordinator | BSP simpler |
| Determinism | Non-deterministic execution order | Deterministic within round (P1) | BSP preserves P1 |
| Best for | Irregular, fine-grained parallelism | Regular, coarse-grained parallelism | IC reduction is coarse |
| Implementations | Cilk, rayon, tokio, Go runtime | Pregel, Apache Giraph, Relativist | Different ecosystems |

---

## 5. Lessons for Relativist

### L1: Work Stealing is Wrong for BSP [REJECT]
The fundamental incompatibility between work stealing (async, decentralized) and BSP (sync barriers, centralized) means work stealing should not be adopted at the inter-worker level.
→ Informs: SPEC-13

### L2: rayon for Intra-Worker Parallelism [REJECT for v1, NOTE for v2]
rayon's work-stealing thread pool could parallelize independent redex reductions within a partition. But v1 specifies sequential reduction for simplicity and determinism. Mark for future optimization.
→ Informs: SPEC-03, SPEC-13 (roadmap)

### L3: Adaptive Partition Sizing Over Stealing [ADAPT]
Instead of stealing work at runtime, the coordinator can adapt partition sizes based on worker performance metrics from previous rounds. This achieves load balancing without the complexity of distributed stealing.
→ Informs: SPEC-04, SPEC-05, SPEC-13

### L4: Dask-Style Centralized Rebalancing [ADAPT]
If load imbalance is detected mid-round (a worker finishes much earlier), the coordinator could split unfinished partitions and re-dispatch fragments. This is "coordinator-mediated stealing" and is simpler than peer-to-peer. Not for v1, but architecturally compatible.
→ Informs: SPEC-13 (roadmap)

---

## 6. Sources

| Source | URL | Accessed |
|--------|-----|----------|
| Work stealing (Wikipedia) | https://en.wikipedia.org/wiki/Work_stealing | 2026-03-26 |
| Blumofe & Leiserson (1999) | https://dl.acm.org/doi/10.1145/324133.324234 | 2026-03-26 |
| Dask Work Stealing docs | https://distributed.dask.org/en/stable/work-stealing.html | 2026-03-26 |
| Adaptive Async Work-Stealing (2024) | https://arxiv.org/html/2401.04494v2 | 2026-03-26 |
| Efficient Work-Stealing Scheduler (2020) | https://tsung-wei-huang.github.io/papers/icpads20.pdf | 2026-03-26 |
