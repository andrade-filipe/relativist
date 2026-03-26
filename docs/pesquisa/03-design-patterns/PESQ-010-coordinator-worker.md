---
pesq_id: PESQ-010
title: "Coordinator-Worker Pattern in Distributed Systems"
category: System Design Patterns
date: 2026-03-26
status: Complete
cross_references:
  specs: [SPEC-05, SPEC-06, SPEC-13]
  pesqs: [PESQ-001, PESQ-002, PESQ-003, PESQ-004, PESQ-005]
  discs: [DISC-005, DISC-007]
---

# PESQ-010: Coordinator-Worker Pattern in Distributed Systems

**Category:** System Design Patterns
**Status:** Complete
**Cross-references:**
- Specs: SPEC-05 (merge/grid cycle), SPEC-06 (wire protocol), SPEC-13 (system architecture)
- PESQs: All Category 1 (PESQ-001 to PESQ-005 each implement coordinator-worker)
- Discussions: DISC-005 v2 (cross-boundary protocol), DISC-007 v2 (fault tolerance)

---

## 1. Subject Overview

The Coordinator-Worker pattern (historically "Master-Slave", now deprecated terminology) is the most fundamental distributed computing pattern. A central coordinator node manages task distribution, state tracking, and result aggregation, while worker nodes execute assigned tasks and report results.

This pattern appears in every system analyzed in Category 1:
- **BOINC** (PESQ-001): Server → Clients
- **Apache Ignite** (PESQ-002): Coordinator → Compute Nodes
- **Ray** (PESQ-003): GCS/Driver → Workers
- **Dask** (PESQ-004): Scheduler → Workers
- **HTCondor** (PESQ-005): Central Manager → Execute Nodes

Relativist uses this pattern explicitly: a Coordinator partitions the IC net, dispatches partitions to Workers, collects reduced partitions, and merges them.

---

## 2. Pattern Taxonomy

### 2.1 Topology Variants

| Variant | Description | Examples | Relativist |
|---------|-------------|----------|------------|
| **Star (hub-and-spoke)** | Single coordinator, all workers connect to it | Dask, BOINC | **This one** |
| **Hierarchical** | Coordinators in tree structure | HTCondor (negotiator→schedd→startd) | Too complex for v1 |
| **Federated** | Multiple independent coordinators | BOINC multi-project | Not applicable |
| **Hybrid** | Coordinator for control, peer-to-peer for data | Ray (GCS + object store) | Future consideration |

**Relativist's choice: Star topology.** The coordinator is the single point of contact for all workers. This is the simplest option and sufficient for the v1 scope (up to ~100 workers). If scalability beyond that is needed, the hybrid variant (coordinator for control plane, direct worker-to-worker for data plane) would be the evolution path.

### 2.2 Communication Direction

| Pattern | Description | Relativist |
|---------|-------------|------------|
| **Push (coordinator → worker)** | Coordinator pushes tasks to workers | **Primary: DispatchPartition** |
| **Pull (worker → coordinator)** | Workers request work when idle | Not used in v1 |
| **Hybrid** | Push for dispatch, pull for re-balancing | Future consideration |

Relativist uses **push** because BSP requires all workers to receive their partitions at the start of each round. Pull doesn't fit BSP's synchronous model.

### 2.3 State Location

| Pattern | Coordinator State | Worker State | Relativist |
|---------|------------------|-------------|------------|
| **Coordinator-centric** | Full computation state | Transient (partition only) | **This one** |
| **Worker-centric** | Metadata only | Full local state | Ray's object store |
| **Shared state** | Distributed state store | Shared access | Apache Ignite |

Relativist is coordinator-centric: the coordinator holds the global Net, partition map, worker registry, and round state. Workers hold only their assigned partition during a round.

---

## 3. Key Mechanisms

### 3.1 Worker Lifecycle

Synthesized from PESQ-001 through PESQ-005, the standard worker lifecycle:

```
[Disconnected] → Register → [Idle] → Receive Task → [Busy] → Complete → [Idle] → ...
                                                                              ↘ Timeout → [Failed]
[Idle] → Deregister → [Disconnected]
```

**Relativist mapping:**
- `Register`: Worker sends `Register { capabilities }` to coordinator
- `Idle`: Worker waits for `DispatchPartition`
- `Busy`: Worker runs `reduce()` on partition
- `Complete`: Worker sends `ReturnPartition { partition, metrics }`
- `Failed`: Coordinator detects heartbeat timeout, marks worker as failed

### 3.2 Coordinator Responsibilities

| Responsibility | How Relativist Implements It |
|----------------|------------------------------|
| Task decomposition | `split(net, k)` → k partitions (SPEC-04) |
| Task assignment | Round-robin or capability-based dispatch |
| Progress tracking | Per-worker state: Idle/Busy/Failed |
| Result aggregation | `merge(partitions)` → unified net (SPEC-05) |
| Failure handling | Timeout → re-dispatch partition to another worker |
| Termination detection | All partitions returned with 0 border redexes |

### 3.3 Single Point of Failure (SPOF) Mitigation

The coordinator is inherently a SPOF. Approaches from surveyed systems:

| System | SPOF Mitigation | Complexity |
|--------|----------------|------------|
| BOINC | Redundant schedulers behind load balancer | High |
| Ray | GCS replication via Redis | Medium |
| Dask | None (single scheduler) | None |
| HTCondor | Checkpoint + restart | Low |

**Relativist v1 decision: No SPOF mitigation.** Like Dask, the coordinator is a single process. If it crashes, the computation restarts. This is acceptable for a research system where:
1. Computations are deterministic (P1) — replay produces same result
2. Typical runtimes are minutes, not days
3. Adding HA would triple complexity for marginal benefit

**Future path:** Coordinator checkpoint (serialize state to disk between rounds) would be the simplest HA addition, similar to HTCondor.

### 3.4 Coordinator Bottleneck Analysis

The coordinator handles:
1. **Control messages:** Register, Heartbeat, RoundStart, etc. — lightweight
2. **Data messages:** DispatchPartition, ReturnPartition — potentially large

For Relativist's BSP model, the bottleneck is **serialized merge**. During the merge phase, the coordinator sequentially processes all returned partitions. This is inherent to the architecture (SPEC-05 requires centralized merge due to border_map).

**Mitigation strategies:**
- Minimize partition data size (send only modified agents/wires, not full partition)
- Pipeline: start merging as partitions arrive (don't wait for all)
- Compression for large partitions (optional feature flag)

---

## 4. Comparison: Coordinator Patterns Across Surveyed Systems

| Dimension | BOINC | Ignite | Ray | Dask | HTCondor | **Relativist** |
|-----------|-------|--------|-----|------|----------|--------------|
| Coordinator role | Scheduler | Thin coordinator | GCS + Driver | Scheduler | Central Manager | Coordinator |
| Worker discovery | Client pull | Auto-discovery | Auto-discovery | Manual + auto | ClassAd matching | Manual register |
| Task granularity | Work units (hours) | Compute jobs | Tasks (ms-s) | Tasks (ms-s) | Jobs (hours) | Partitions (per round) |
| State persistence | MySQL DB | Distributed cache | Redis/GCS | In-memory | ClassAd DB | In-memory |
| Failure detection | Deadline + quorum | Heartbeat | Heartbeat | Heartbeat | Heartbeat | Heartbeat |
| Recovery | Re-issue task | Re-route | Lineage replay | Re-compute | Re-schedule | Re-dispatch |
| Synchronization | Async (no barrier) | Async | Async | Semi-sync | Async | **BSP (barrier)** |

**Key insight:** Relativist is the only system using strict BSP barriers. All others allow asynchronous progress. This is by design: IC reduction requires P3 (border completeness) which mandates all partitions be merged before the next round.

---

## 5. Lessons for Relativist

### L1: Keep Worker Registration Simple [ADOPT]
All systems start with a simple registration handshake. Relativist's `Register { capabilities }` → `RegisterAck { worker_id, config }` is correct and sufficient. Don't add complexity (service discovery, auto-scaling) in v1.
→ Informs: SPEC-06, SPEC-13

### L2: Heartbeat is Universal [ADOPT]
Every surveyed system uses heartbeat for failure detection. The pattern is: worker sends periodic heartbeat, coordinator marks worker as failed after N missed heartbeats. Relativist already specifies this in SPEC-06.
→ Informs: SPEC-06

### L3: Coordinator State Should Be Serializable [ADAPT]
Multiple systems (HTCondor, BOINC) persist coordinator state for crash recovery. Relativist v1 doesn't need this, but the coordinator's state struct should be `#[derive(Serialize, Deserialize)]` so checkpoint can be added later without refactoring.
→ Informs: SPEC-13

### L4: BSP Barrier is Correct for IC Reduction [ADOPT]
The analysis confirms that BSP's barrier synchronization is the right fit for Relativist. IC reduction requires global merge (P3: border completeness) between rounds, which is inherently a barrier. Attempting async would violate P3.
→ Informs: SPEC-05, SPEC-13

### L5: Pipeline Merge for Performance [ADAPT]
Start merging partitions as they arrive rather than waiting for all workers. The merge is associative (order doesn't matter for correctness), so pipelining is safe.
→ Informs: SPEC-05, SPEC-13

### L6: Star Topology is Sufficient for v1 [ADOPT]
All surveyed systems started with star topology. Scaling to 100s of workers is achievable with a single coordinator. Hierarchical or hybrid topologies are future optimizations.
→ Informs: SPEC-13

### L7: No Worker-to-Worker Communication in v1 [ADOPT]
In BSP with centralized merge, workers never need to communicate directly. All data flows through the coordinator. This dramatically simplifies the protocol (SPEC-06) and security model (SPEC-10).
→ Informs: SPEC-06, SPEC-10, SPEC-13

---

## 6. Sources

| Source | URL | Accessed |
|--------|-----|----------|
| Master-Worker Pattern (GigaSpaces) | https://docs.gigaspaces.com/ie-resources/solution-hub/master-worker-pattern.html | 2026-03-26 |
| Master-Worker Pattern (Java Design Patterns) | https://java-design-patterns.com/patterns/master-worker/ | 2026-03-26 |
| Distributed System Patterns (GeeksforGeeks) | https://www.geeksforgeeks.org/system-design/distributed-system-patterns/ | 2026-03-26 |
| PESQ-001 through PESQ-005 (internal) | — | — |
