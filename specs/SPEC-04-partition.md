# SPEC-04: Net Partitioning

**Status:** Revised v3.2 — R12 amended per SPEC-21 §3.8 A1 (border-id allocation for streaming pipeline); §4.5 amended per SPEC-21 §3.8 A8 (split() unchanged; chunked pipeline additive)
**Depends on:** SPEC-00 (Glossary), SPEC-01 (Invariants), SPEC-02 (Net Representation), SPEC-03 (Reduction Engine)
**Amends:** SPEC-22 §3.8 A7 (build_subnet — populate per-partition free-list + sparse-build threshold); SPEC-21 §3.8 A1 (R12 — dual-path border-id allocation: batch vs streaming); SPEC-21 §3.8 A8 (§4.5 — split() unchanged; chunked pipeline is an additive alternative entry point)
**Gray zones resolved:** Z2 (partitioning strategy for IC nets)
**References consumed:** REF-001 (Lafont 1990), REF-002 (Lafont 1997), REF-005 (Mackie & Pinto 2002), REF-013 (Mackie 1997), REF-014 (Kahl 2015)
**Discussions consumed:** DISC-003 v2 (strong confluence to distributed determinism, premises P1-P5), DISC-004 v2 (formal partitioning of IC nets, conditions C1-C3, allocation function, wire classification, isomorphism theorem)
**Arguments consumed:** ARG-001 (central argument, premises P1-P6), ARG-002 (partitioning preserves structure, split/merge identity, equivalence of local reduction)
**Code analyses consumed:** AC-002 (Haskell IC.Partition: partitionNet, mergePartitions, findBorderRedexes, FreePort lifecycle), AC-015 (cross-cutting synthesis: CC-4 static ID space partitioning from HVM4)

---

## 1. Purpose

This spec defines the partitioning subsystem of Relativist: how a single IC net is decomposed into disjoint sub-nets for distribution to grid workers, and how the split operation preserves the structural conditions necessary for correct distributed reduction. It formalizes the allocation function sigma, the wire classification (internal, interface, border), the FreePort (Boundary) mechanism, the correctness conditions C1-C3, the static ID space partitioning, the `PartitionStrategy` trait, and the debug assertions. The complementary merge operation (recombining sub-nets after local reduction) is defined in SPEC-05.

## 2. Definitions

Terms defined in SPEC-00 (Glossary) are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **Allocation function (sigma)** | A total function `sigma: A -> {0, 1, ..., n-1}` that assigns each agent in the net to exactly one worker. The function induces a mathematical partition of the agent set into disjoint subsets `A_0, A_1, ..., A_{n-1}`. Any allocation function that satisfies C1-C3 is valid; the choice affects performance but never correctness (DISC-004 v2, Section 1.3; ARG-002, Passo 10). |
| **Internal wire** | A wire whose two endpoints are AgentPorts of agents that belong to the same partition: `sigma(a1) == sigma(a2)`. Internal wires are preserved intact during split. |
| **Interface wire** | A wire involving a pre-existing FreePort (Lafont) from the original net. Assigned to the partition of its AgentPort endpoint, unchanged by split. |
| **Border wire** | A wire whose two AgentPort endpoints belong to agents in different partitions: `sigma(a1) != sigma(a2)`. During split, each border wire is replaced by a pair of FreePort (Boundary) sentinels sharing a unique `borderId` (DISC-004 v2, Section 1.4). |
| **Border ID** | A unique `u32` identifier assigned to each border wire cut during partitioning. Each border ID appears in exactly two distinct partitions. Border IDs MUST NOT collide with pre-existing FreePort IDs (SPEC-00 Section 6.6; DISC-004 v2, Section 1.4). |
| **Border map** | A mapping `borderId -> (PortRef, PortRef)` recording the two original endpoints of each cut border wire. Used by the merge protocol (SPEC-05) to restore connections. |
| **FreePort index** | A `HashMap<u32, PortRef>` maintained per partition that maps `borderId -> AgentPort` local, providing the reverse lookup for `AgentPort -> FreePort(borderId)` connections. Necessary because FreePort (Boundary) sentinels have no slot in the port array (SPEC-02, Section 4.10). |
| **ID range** | An exclusive, contiguous range of `AgentId` values reserved for a worker to generate new agents during local reduction without coordination and without risk of collision. |

---

## 3. Requirements

### 3.1 The split Function

**R1.** The `split` function MUST accept as input a `Net`, a number of workers `num_workers: u32` (n >= 1), and a `PartitionStrategy`, and return a `PartitionPlan` containing `n` partitions and the border map. **(MUST)**

> **Cross-spec note (SPEC-13):** SPEC-13's coordinator FSM uses the action `InvokeSplit { net: Net, num_workers: usize }` and event `SplitComplete(Vec<Partition>)`. These represent the *FSM interface*, not the `split()` function signature. The coordinator's action executor calls `split()` with coordinator-local state (including the strategy), stores the returned `PartitionPlan` (with border map) in coordinator-local state, and fires `SplitComplete(plan.partitions)` into the FSM. The `num_workers` type discrepancy (`u32` here vs. `usize` in SPEC-13) is resolved at the call site by casting `usize as u32`, which is safe for n <= 8 (TCC scope). SPEC-04 uses `u32` for consistency with `WorkerId` and SPEC-05 R25. The border map is retained in coordinator state and passed to `merge()` (SPEC-05 R1) when `InvokeMergeAndReduce` is executed.

**R2.** If `n == 1`, the function MUST return the entire net as a single partition with no borders. This is the trivial case and MUST execute in O(A + W) for the clone, with no additional partitioning overhead (no allocation function computation, no wire classification, no border generation). If the implementation accepts the net by value (`net: Net`), the trivial case MAY be O(1) by moving the net into the partition. **(MUST)**

**R3.** If `n > |A|` (number of live agents), the effective number of non-empty partitions MUST be at most `|A|`. Excess partitions MUST be empty (no agents, no borders, no redexes). The coordinator SHOULD skip dispatching empty partitions to workers to avoid unnecessary network traffic (workers assigned empty partitions would perform trivial no-op reductions). **(MUST for partition semantics; SHOULD for dispatch optimization)**

**R4.** The split operation MUST be deterministic: given the same net and the same allocation function sigma, the output MUST be identical across invocations. **(MUST)**

**R5.** The split operation MUST be a pure function of the net and the partition plan. It MUST NOT depend on external state, wall-clock time, or randomness. **(MUST)**

### 3.2 Correctness Conditions (C1-C3)

These conditions are necessary and sufficient for invariant D1 (SPEC-01) to hold: `merge(split(net)) ~ net`. They are derived from DISC-004 v2, Section 4.1 and formalized in ARG-002, Q5.

**R6 (C1 -- Complete agent coverage).** Every live agent in the net MUST belong to exactly one partition. The sets `A_0, A_1, ..., A_{n-1}` MUST form a mathematical partition of A (disjoint and exhaustive). No agent is lost or duplicated. **(MUST)**

- Formal: `forall a in A: exists unique i such that a in A_i`
- Formal: `A_0 union A_1 union ... union A_{n-1} = A`
- Implements: SPEC-01, D1a; SPEC-01, D5a.

**R7 (C2 -- Complete wire coverage).** Every wire in the net MUST be classified as exactly one of: internal (preserved entirely in one partition), interface (preserved with its pre-existing FreePort in the partition of its AgentPort endpoint), or border (replaced by a pair of FreePort (Boundary) sentinels). No wire is lost. **(MUST)**

- Formal: `W = W_0^internal union ... union W_{n-1}^internal union W^interface union W^border` (disjoint union)
- Implements: SPEC-01, D1b.

**R8 (C3 -- FreePort bijectivity).** For each `borderId` bid generated during split, there MUST exist exactly one `FreePort(bid)` in each of exactly two distinct partitions. The merge can reconnect each border wire unambiguously. **(MUST)**

- Formal: `forall bid in borderMap.keys(): |{i : FreePort(bid) in partition_i}| == 2 and the two i values are distinct`
- Implements: SPEC-01, D1c.

**R9.** No internal wire (both endpoints in the same partition) MUST be altered by the split operation. Internal wires MUST be byte-for-byte identical in the sub-net and in the original net (modulo the sub-net being a subset). **(MUST)**

- Implements: SPEC-01, D1d.

**R10.** In debug mode (`#[cfg(debug_assertions)]`), the split function MUST execute assertions for C1, C2, and C3 before returning. **(MUST)**

### 3.3 FreePort (Boundary) and Border Wires

**R11.** When a wire `(AgentPort(a1, p1), AgentPort(a2, p2))` crosses a partition boundary (`sigma(a1) != sigma(a2)`), the split MUST replace it with:
- In partition `sigma(a1)`: `AgentPort(a1, p1) <-> FreePort(bid)`
- In partition `sigma(a2)`: `AgentPort(a2, p2) <-> FreePort(bid)`

where `bid` is a unique border ID. The border map MUST record `bid -> (AgentPort(a1, p1), AgentPort(a2, p2))`. **(MUST)**

**R12.** Border IDs MUST be globally unique and MUST NOT collide with pre-existing FreePort IDs in the net. Border ID allocation depends on the partitioning entry point:

- **Batch path (`split()` when `chunk_size = u32::MAX`):** New border IDs MUST start from `max_existing_freeport_id + 1` (AC-002: `borderStart = maxFreePortId(netWires) + 1`). This requires a single global scan of the full net before partitioning.
- **Streaming path (`generate_and_partition_chunked`, SPEC-21 §3.3 R17):** Border IDs MUST start at 0 and increment monotonically when no Lafont FreePorts are present in any batch, OR at `max_lafont_freeport_id_in_first_batch + 1` when the first batch carries Lafont FreePorts. Generators that emit Lafont FreePorts SHOULD emit ALL of them in the first batch to simplify the discovery scan (SPEC-21 R29b). The `Partition.border_id_start` and `Partition.border_id_end` (R15a) MUST be set to the global range `[0, border_id_counter)` (or shifted by `max_lafont_freeport_id + 1` when Lafont FreePorts exist).

The two paths produce non-overlapping but distinct border-id ranges; tests that exercise both `split()` and `generate_and_partition_chunked` MUST account for this (different absolute integers, same internal C3-bijectivity guarantee). **(MUST)**

> **Amendment A1 (SPEC-21 §3.8 A1 / R29b):** The dual-path policy above closes SC-018. The streaming pipeline cannot perform a single global scan of "the full net" because the full net does not exist until the stream is exhausted. The first-batch scan is the streaming-equivalent of the batch path's pre-partition scan. See SPEC-21 §3.3 R17 (`generate_and_partition_chunked`) and §4.8 (partition `border_id_start`/`border_id_end` propagation).

**R13.** Each partition MUST maintain a FreePort index (`HashMap<u32, PortRef>`) that maps each `borderId` present in that partition to the local `AgentPort` connected to the corresponding `FreePort(bid)`. This index enables O(1) lookup during merge, eliminating the linear scan of the Haskell prototype (`freePortNeighbor`, AC-002 L3). **(MUST)**

**R14.** Port linearity (SPEC-01, T1) MUST be preserved at partition boundaries: each FreePort (Boundary) participates in exactly one wire in exactly one partition. No boundary FreePort has multiple connections, and no connection is lost. **(MUST)**

- Formal: `forall bid: the wire (AgentPort(a, p), FreePort(bid)) exists in exactly one partition`

**R15.** The distinction between FreePort (Lafont) and FreePort (Boundary) MUST be respected semantically, even though both are represented by the same `FreePort(u32)` variant at the type level (SPEC-00 Sections 6.1 and 6.2; SPEC-02, R4 note). Pre-existing Lafont FreePorts from the original net MUST NOT be treated as border wires. **(MUST)**

**R15a.** Each `Partition` MUST carry metadata sufficient to distinguish boundary FreePort IDs from Lafont FreePort IDs and from `DISCONNECTED` (`u32::MAX`). Specifically, each partition MUST store `border_id_start` and `border_id_end` (the global range of border IDs assigned during this split). A `FreePort(id)` in the port array is a boundary FreePort if and only if `border_id_start <= id && id < border_id_end && id != u32::MAX`. This range check is the mechanism for lazy FreePort index reconstruction (Section 4.6, approach 2). **(MUST)**

### 3.4 Static ID Space Partitioning

**R16.** The `PartitionPlan` MUST assign each worker an exclusive, contiguous range of `AgentId` values for generating new agents during local reduction. **(MUST)**

**R17.** ID ranges MUST be disjoint across workers and MUST span a sufficient interval to accommodate agent creation by reduction rules (CON-DUP creates +2 net agents, CON-ERA and DUP-ERA create 2 new ERA). **(MUST)**

**R18.** The ID range for worker `i` MUST be computed as:
```
chunk_size = u32::MAX / num_workers
range_i = [i * chunk_size, (i + 1) * chunk_size)
```
The last worker's range extends to `u32::MAX` (inclusive). Each partition's `subnet.next_id` MUST be initialized to `max(range_i.start, max_agent_id_in_partition + 1)`. **(MUST)**

**R19.** Static ID space partitioning MUST eliminate the need for post-reduction ID remapping (`remapAllPartitions` in the Haskell prototype, AC-003). **(MUST)**

- Implements: SPEC-01, D4a, D4b.

**R20.** If a worker exhausts its ID range (allocates more than `chunk_size` new agents), the system MUST signal an error. For practical nets within the scope of this TCC (up to tens of thousands of agents, with 8 workers each having ~537 million IDs), exhaustion is not expected. **(MUST)**

### 3.5 Partition Strategy

**R21.** Relativist MUST define a trait `PartitionStrategy` that abstracts the allocation function sigma. **(MUST)**

**R22.** A default implementation (contiguous ID range, round-robin) MUST be provided as baseline. This is the same strategy used by the Haskell prototype (AC-002, `partitionNet`). **(MUST)**

**R23.** The trait SHOULD allow future alternative implementations (topology-aware, redex-aware) without modifying the rest of the partitioning pipeline. **(SHOULD)**

### 3.6 Root Port Propagation

**R28.** If `net.root` is `Some(AgentPort(id, p))`, the sub-net of the partition containing agent `id` MUST set `subnet.root = net.root`. All other partitions MUST set `subnet.root = None`. If `net.root` is `None`, all partitions MUST have `subnet.root = None`. If `net.root` is `Some(FreePort(f))`, the root is a Lafont FreePort (external interface); it MUST be preserved in whichever partition would inherit the interface wire (the partition containing the agent connected to `FreePort(f)`, if any), with other partitions set to `None`. **(MUST)**

### 3.7 Redex Queue Population

**R24.** The redex queue of each partition MUST contain only Active Pairs that are internal to that partition (both agents belong to the same partition). Active Pairs that cross boundaries (border redexes) MUST NOT appear in any partition's redex queue. **(MUST)**

**R25.** Border redexes (pre-existing Active Pairs whose agents were separated by the split) MUST be detectable after merge via the `Net::connect` mechanism (SPEC-02, R13). They MUST NOT be lost. **(MUST)**

### 3.8 Complexity

**R26.** The split operation SHOULD have time complexity O(A + W) where A is the number of agents and W is the number of wires (ports with valid connections in the port array). **(SHOULD)**

**R27.** Space complexity of the PartitionPlan SHOULD be O(A + W + B) where B is the number of border wires. **(SHOULD)**

---

## 4. Design

### 4.1 Types

```rust
/// Identifier of a worker in the grid.
/// Values from 0 to n-1, where n is the number of workers.
pub type WorkerId = u32;

/// An exclusive range of AgentIds reserved for a worker.
/// The worker may generate new IDs in the interval [start, end).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IdRange {
    /// First AgentId in the range (inclusive).
    pub start: AgentId,
    /// Last AgentId in the range (exclusive).
    pub end: AgentId,
}
```

```rust
/// A partition: sub-net assigned to a worker.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Partition {
    /// The sub-net containing the agents of this partition.
    /// Border wires appear as connections to FreePort(borderId).
    pub subnet: Net,

    /// Identifier of the worker responsible for this partition.
    pub worker_id: WorkerId,

    /// Reverse index of boundary FreePorts: borderId -> AgentPort local.
    /// Enables O(1) lookup during merge, instead of linear scan.
    /// Cf. AC-002 L3 (elimination of linear search in freePortNeighbor).
    pub free_port_index: HashMap<u32, PortRef>,

    /// ID range reserved for this worker to generate new agents.
    pub id_range: IdRange,

    /// Range of border IDs assigned to this partition during split.
    /// Used for lazy FreePort index reconstruction: a FreePort(id) in
    /// the port array is a boundary FreePort if and only if
    /// `border_id_start <= id && id < border_id_end && id != u32::MAX`.
    /// FreePort(id) with id < border_id_start is a Lafont FreePort.
    /// FreePort(u32::MAX) is DISCONNECTED (SPEC-02, Section 4.4).
    /// This field enables disambiguation without requiring a HashSet
    /// or relying solely on the free_port_index (which may be stale
    /// after local reduction).
    pub border_id_start: u32,
    pub border_id_end: u32,
}
```

```rust
/// The complete partitioning plan.
/// Note: serde derives are included for consistency with Partition,
/// even though PartitionPlan stays on the coordinator and is not
/// transmitted over the wire. They may be useful for checkpointing
/// or debugging serialization.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PartitionPlan {
    /// List of partitions, one per worker. partitions[i].worker_id == i.
    pub partitions: Vec<Partition>,

    /// Border map: borderId -> (original_endpoint_A, original_endpoint_B).
    /// Records the two endpoints of each wire that was cut.
    /// Used by the merge protocol (SPEC-05) to restore connections.
    pub borders: HashMap<u32, (PortRef, PortRef)>,
}
```

### 4.2 Trait PartitionStrategy

```rust
/// Trait that abstracts the allocation function sigma: AgentId -> WorkerId.
///
/// Implementations of this trait determine how agents are distributed
/// among workers. The correctness of the system does NOT depend on the
/// strategy chosen (DISC-004 v2, Section 1.6: "the distinction between
/// partitionings is one of quality, not correctness"), but performance
/// depends significantly.
pub trait PartitionStrategy {
    /// Assigns each agent to a worker.
    ///
    /// Input: reference to the net and number of workers.
    /// Output: map of AgentId -> WorkerId for every live agent.
    ///
    /// Post-conditions:
    /// - Every live agent in the net has an entry in the returned map (C1).
    /// - Every WorkerId in the map is in range [0, num_workers).
    fn allocate(&self, net: &Net, num_workers: u32) -> HashMap<AgentId, WorkerId>;
}
```

### 4.3 Baseline Strategy: Contiguous ID Range

```rust
/// Partitioning strategy by contiguous ID ranges.
///
/// Live agents are sorted by AgentId in ascending order and divided
/// into chunks of approximately equal size. Worker 0 receives the
/// first ceil(|A|/n) agents, Worker 1 the next, and so on.
///
/// Properties:
/// - O(A) time (traverse all live agents).
/// - Deterministic: same net + same n = same result.
/// - Ignores graph topology (DISC-004 v2, Perspective 1).
/// - May split Active Pairs across partitions (creates border redexes).
/// - Same strategy as the Haskell prototype (AC-002, partitionNet).
///
/// Correctness is guaranteed by strong confluence regardless of the
/// quality of partitioning (DISC-004 v2, Section 1.6; ARG-002, Passo 10).
pub struct ContiguousIdStrategy;

impl PartitionStrategy for ContiguousIdStrategy {
    fn allocate(&self, net: &Net, num_workers: u32) -> HashMap<AgentId, WorkerId> {
        // 1. Collect IDs of all live agents, sorted ascending.
        // 2. Divide into chunks of size ceil(|A| / num_workers).
        // 3. Assign each chunk to a worker sequentially.
        // ... (implementation by the ENGINEER)
        todo!()
    }
}
```

### 4.4 Wire Classification

Given the allocation function sigma, every wire in the net falls into exactly one of three categories (R7, C2):

| Category | Condition | Treatment during split |
|----------|-----------|----------------------|
| **Internal** | `target = AgentPort(b, q)` with `sigma(a) == sigma(b)` | Preserved intact in the partition of `sigma(a)` |
| **Interface** | `target = FreePort(f)` where f is a pre-existing Lafont free port | Inherited by the partition of agent `a` |
| **Border** | `target = AgentPort(b, q)` with `sigma(a) != sigma(b)` | Replaced by `FreePort(bid)` in each of the two partitions; border map updated |

A `DISCONNECTED` target (transient state during reduction, cf. SPEC-02) is ignored during classification.

To avoid processing each border wire twice (once from each side), the split MUST use the convention: a border wire is DETECTED only from the side with the smaller AgentId (`a.id < b.id`), but FreePort entries are generated for BOTH partitions in a single pass. Both `border_entries[sigma(a)]` and `border_entries[sigma(b)]` receive an entry from the same detection step.

### 4.5 The split Algorithm

> **Amendment A8 (SPEC-21 §3.8 A8 / SC-001 part 4):** `split()` is UNCHANGED — same semantics, same R-numbers (R6/R12/R16-R18/R28). The chunked pipeline introduced by SPEC-21 (`generate_and_partition_chunked`, §3.3 R17) is an ALTERNATIVE entry point, selected when `GridConfig.chunk_size != u32::MAX`. The two paths produce structurally compatible output: `split()` returns `PartitionPlan`; `generate_and_partition_chunked` returns `ChunkedPartitionResult { partitions, borders, stats }`, which is convertible to `PartitionPlan` per SPEC-21 R20-R21. `split()` remains the fallback for the v1 backward-compat path under SPEC-21 R26 (short-circuit when `chunk_size = u32::MAX`). The two entry points are non-overlapping but coexistent. This clarification documents the additive nature explicitly so that downstream readers of SPEC-04 are not surprised by SPEC-21's parallel pipeline.

```
fn split(net: &Net, num_workers: u32, strategy: &dyn PartitionStrategy) -> PartitionPlan
```

**Pre-conditions:**
- `net` satisfies invariants T1 (linearity), I1 (bidirectionality), I2 (reference validity) from SPEC-01.
- `num_workers >= 1`.
- The input net MUST NOT contain any stale FreePort (Boundary) sentinels from a previous round. All boundary FreePorts MUST have been resolved by the preceding merge (SPEC-05). In debug mode, the split function SHOULD scan for FreePort values above `max_existing_lafont_freeport_id` and assert that none exist, signaling an error if any remain. This precondition is automatically satisfied when the merge correctly resolves all borders (SPEC-05, R3-R6) and the subsequent `reduce_all` does not reintroduce boundary FreePorts (which it cannot, since the reduction engine does not generate FreePort values).

**Post-conditions:**
- The returned PartitionPlan satisfies C1 (R6), C2 (R7), and C3 (R8).
- Every partition's redex queue contains only internal Active Pairs (R24).
- ID ranges are disjoint and cover the full `u32` space (R16-R18).
- In debug mode, assertions for C1, C2, C3 have passed (R10).

The algorithm operates in 7 steps:

**Step 1: Trivial case.**
If `num_workers <= 1`, return the entire net as a single partition with no borders. The `id_range` covers the entire `u32` space. Return immediately.

**Step 2: Compute the allocation function.**
Invoke `strategy.allocate(net, num_workers)` to obtain the map `sigma: AgentId -> WorkerId`.

**Step 3: Group agents by worker.**
Construct `num_workers` disjoint sets `A_0, A_1, ..., A_{n-1}` by iterating over sigma. This step is O(A).

**Step 4: Classify wires and generate border IDs.**
Traverse the port array of the net. For each live agent `a` and each of its ports `p`:
- Let `target = get_target(AgentPort(a.id, p))`.
- If `target` is `AgentPort(b.id, q)` with `sigma(b.id) == sigma(a.id)`: internal wire. No action (preserved intact).
- If `target` is `AgentPort(b.id, q)` with `sigma(b.id) != sigma(a.id)` and `a.id < b.id`: border wire. Assign a new `borderId`, record in border map, record FreePort entries for both partitions.
- If `target` is `FreePort(f)`: interface wire. Inherited by the partition of agent `a`.
- If `target` is `DISCONNECTED`: skip.

The `borderId` counter starts at `max_existing_freeport_id(net) + 1` (R12).

**Step 5: Build sub-nets.**
For each worker `i`:
- Create a `Net` containing only the agents of `A_i`. The sub-net's `agents` Vec MUST be sized to at least `max_agent_id_in(A_i) + 1`, and its `ports` Vec MUST be sized to at least `(max_agent_id_in(A_i) + 1) * PORTS_PER_SLOT`. Agent slots not belonging to partition `i` MUST be `None`, and their corresponding port slots MUST be set to `DISCONNECTED` (SPEC-02, Section 4.4). This maintains the uniform indexing scheme `id * PORTS_PER_SLOT + port_id` for all agents in the sub-net.
- For internal wires: copy all `PORTS_PER_SLOT` (3) port array entries per agent directly, including `DISCONNECTED` slots (e.g., slots 1 and 2 for ERA agents, which have arity 0 and use only slot 0). This preserves the uniform port array layout regardless of agent arity.
- For border wires: set `port_array[AgentPort(a, p)] = FreePort(bid)` for each agent `a` in `A_i` that has a border connection.
- For interface wires: copy the `FreePort(f)` connection as-is.
- Build the `free_port_index`: for each `(bid, AgentPort(a, p))` pair in this partition, insert `bid -> AgentPort(a, p)`.
- Populate the redex queue with only internal Active Pairs (both agents in `A_i`).
- Set `subnet.root` according to R28 (root port propagation).

**Step 6: Compute ID ranges.**
```
chunk_size = u32::MAX / num_workers
For each worker i:
    id_range.start = i * chunk_size
    id_range.end   = if i == num_workers - 1 { u32::MAX } else { (i+1) * chunk_size }
    subnet.next_id = max(id_range.start, max_agent_id_in(A_i) + 1)
```

**Step 7: Debug assertions.**
In debug mode, verify C1, C2, C3 (see Section 4.8).

**Consolidated pseudocode:**

```
fn split(net, num_workers, strategy) -> PartitionPlan:
    if num_workers <= 1:
        return trivial_plan(net)

    sigma = strategy.allocate(net, num_workers)

    // Group agents by worker
    worker_agents: Vec<Vec<AgentId>> = group_by_worker(sigma, num_workers)

    // Classify wires and generate borders
    border_id_start = max_freeport_id(net) + 1
    border_id_counter = border_id_start
    borders = HashMap::new()
    border_entries: Vec<Vec<(AgentId, PortId, u32)>> = vec![vec![]; num_workers]

    for agent_id in net.live_agents():
        for port_id in 0..total_ports(agent.symbol):
            target = net.get_target(AgentPort(agent_id, port_id))
            match target:
                AgentPort(other_id, other_port):
                    if sigma[agent_id] != sigma[other_id] and agent_id < other_id:
                        bid = border_id_counter++
                        borders[bid] = (AgentPort(agent_id, port_id),
                                        AgentPort(other_id, other_port))
                        border_entries[sigma[agent_id]]
                            .push((agent_id, port_id, bid))
                        border_entries[sigma[other_id]]
                            .push((other_id, other_port, bid))
                _: // internal, interface, or DISCONNECTED — no border action

    // Build each partition
    partitions = []
    chunk_size = u32::MAX / num_workers
    for i in 0..num_workers:
        subnet = build_subnet(net, worker_agents[i], sigma,
                              border_entries[i])
        free_port_index = build_free_port_index(border_entries[i])
        id_range = IdRange {
            start: i * chunk_size,
            end: if i == num_workers - 1 { u32::MAX } else { (i+1) * chunk_size },
        }
        subnet.next_id = max(id_range.start,
                             max_id_in(worker_agents[i]).map(|m| m + 1).unwrap_or(0))
        partitions.push(Partition { subnet, worker_id: i,
                                    free_port_index, id_range,
                                    border_id_start,
                                    border_id_end: border_id_counter })

    debug_assert!(verify_c1_c2_c3(net, &partitions, &borders))
    return PartitionPlan { partitions, borders }
```

**Complexity:** O(A + W) where A is the number of live agents and W is the number of wires (ports with valid connections). Step 4 traverses the port array once (A * PORTS_PER_SLOT). Step 5 copies agents and connections. The HashMap for sigma has O(1) amortized per lookup.

#### 4.5.1 build_subnet — Free-List Population and Sparse-Build Threshold (Amendment A7)

> **Amendment A7 (SPEC-22 §3.8 A7 / R10a, R22, R30):** `build_subnet` MUST populate the partition subnet's `free_list` with all `None` slots in `[partition.id_range.start, partition.id_range.end)` after the live agents are placed. When the dense-arena threshold check fires (`id_range.end - id_range.start > 4 × live_agent_count`), `build_subnet` MUST use `SparseNet` internally and call `to_dense(Some(partition.id_range.clone()))` before returning (SPEC-22 R10a, R22). The exposed signature MAY remain `Net` to preserve API stability; the sparse path is an implementation detail. When `PartitionConfig.sparse_build = false` is forced AND the threshold would fire, `build_subnet` MUST reject with `PartitionError::DenseAllocationExceedsThreshold` (SPEC-22 R30). The free-list MUST contain only IDs strictly within `[id_range.start, id_range.end)` (SPEC-22 R10a, closes SC-006). Cross-references: SPEC-22 R10a (per-partition free-list), R22 (sparse threshold), R30 (sparse_build flag). Closes SC-003, SC-006, SC-009.

### 4.6 FreePort Index Maintenance During Local Reduction

During local reduction by a worker, the reduction rules may alter what is connected to a boundary FreePort. The FreePort index MUST remain consistent with the actual port array state before merge. Three scenarios exist:

**Scenario 1: Reconnection.** Agent `a` connected to `FreePort(bid)` participates in a local redex with agent `c`. The rule may reconnect `FreePort(bid)` to a new agent `d`. The `free_port_index` MUST be updated: `index[bid] = AgentPort(d, p)`.

**Scenario 2: Erasure (FreePort transfer).** Agent `a` has an auxiliary port connected to `FreePort(bid)`, and `a` participates in an erasure rule (ERA interacts with `a`'s principal port). The erasure rule (CON-ERA or DUP-ERA) removes agent `a` and creates 2 new ERA agents, each connected to one of `a`'s former auxiliary ports. If `FreePort(bid)` was connected to `a`'s auxiliary port `p`, the new ERA agent inherits that connection: the FreePort is NOT destroyed but transferred to the new ERA agent's principal port. The `free_port_index` MUST be UPDATED (not removed): `index[bid] = AgentPort(new_era_id, 0)`.

Note: A FreePort (Boundary) connection is NEVER simply deleted during local reduction. It is always either preserved (no interaction), transferred to a replacement agent (Scenarios 2 and 3), or reconnected to a different agent (Scenario 1). The boundary FreePort acts as an impermeable wall: the agent on the boundary side can interact with partners on other ports, but the FreePort connection itself persists through the interaction (inherited by new agents). The only scenario in which a `free_port_index` entry could become stale is if both agents in a local redex are removed without creating any new connections -- but the IC reduction rules always reconnect auxiliary ports (CON-CON, DUP-DUP reconnect 4 wires; CON-ERA, DUP-ERA create 2 new ERA agents connected to the auxiliary ports). Therefore, FreePort entries are always transferred, never orphaned during local reduction.

**Scenario 3: Propagation via CON-DUP.** The CON-DUP commutation rule creates 4 new agents. If one of the original agents had an auxiliary port connected to `FreePort(bid)`, the new agent inherits that connection. The `free_port_index` MUST be updated to point to the new agent.

Two implementation approaches are viable:

1. **Active notification:** The reduction engine invokes a callback when `set_port(port, FreePort(_))` or when `get_target(port)` was a FreePort before overwrite. The callback updates the index.

2. **Lazy reconstruction:** The `free_port_index` is rebuilt by scanning the port array before merge. For each `FreePort(id)` found in the port array, the reconstruction includes it as a boundary FreePort if and only if `partition.border_id_start <= id && id < partition.border_id_end && id != u32::MAX` (R15a). Entries with `id < border_id_start` are Lafont FreePorts (ignored). Entries with `id == u32::MAX` are `DISCONNECTED` sentinels (SPEC-02, Section 4.4; ignored). Complexity: O(A_i * PORTS_PER_SLOT) per partition.

Approach (2) SHOULD be used as baseline due to simplicity. Migration to (1) is warranted only if benchmarks reveal a bottleneck.

### 4.7 Static ID Space Partitioning -- Detail

The `u32` space (~4.29 billion positions) is statically divided among workers:

```
Number of workers:  n
Chunk size:         u32::MAX / n  (integer division, truncated)
Worker i:           [i * chunk_size, (i+1) * chunk_size)
                    except the last worker which extends to u32::MAX (inclusive).

Example with n = 8 (chunk_size = 4_294_967_295 / 8 = 536_870_911):
  Worker 0: [0,              536_870_911)       536_870_911 IDs
  Worker 1: [536_870_911,    1_073_741_822)     536_870_911 IDs
  ...
  Worker 6: [3_221_225_466,  3_758_096_377)     536_870_911 IDs
  Worker 7: [3_758_096_377,  4_294_967_295]     536_870_918 IDs (last worker gets remainder)

Note: The last worker receives slightly more IDs than the others due to
integer division truncation. With 8 workers, the difference is 7 IDs --
negligible. The ID range computation uses u32 arithmetic; to avoid
overflow from (u32::MAX + 1), the formula uses u32::MAX / n directly
and extends the last worker's range to u32::MAX (inclusive).
```

Each worker initializes `subnet.next_id` to the first available ID in its range that is greater than all IDs already present in the sub-net. This ensures that pre-existing IDs (from the original net) are not overwritten.

The pre-allocation is inspired by HVM4 (AC-015, CC-4): each thread receives a slice of the heap `tid * bank_sz`. This completely eliminates the `remapAllPartitions` of the Haskell prototype (AC-003), removing an entire phase from the grid cycle.

**Range exhaustion:** If a worker exhausts its range (more than ~537M new agents with 8 workers), the system MUST signal an error. For practical nets within the scope of this TCC (up to tens of thousands of agents), this is not expected to occur.

### 4.8 Debug Assertions for C1-C3

```rust
/// Verifies C1 (Complete agent coverage): every agent of the original net
/// is in some partition, and no agent appears in more than one partition.
#[cfg(debug_assertions)]
fn assert_coverage_and_disjunction(
    original: &Net,
    partitions: &[Partition],
) {
    let mut seen: HashSet<AgentId> = HashSet::new();
    let mut total = 0;
    for partition in partitions {
        for (i, slot) in partition.subnet.agents.iter().enumerate() {
            if slot.is_some() {
                let id = i as AgentId;
                assert!(
                    seen.insert(id),
                    "C1/C2 violated: agent {} appears in more than one partition",
                    id
                );
                total += 1;
            }
        }
    }
    let original_count = original.agents.iter().filter(|s| s.is_some()).count();
    assert_eq!(
        total, original_count,
        "C1 violated: {} agents in partitions, {} in original net",
        total, original_count
    );
}

/// Verifies C3 (FreePort bijectivity): each borderId appears in exactly
/// two distinct partitions.
#[cfg(debug_assertions)]
fn assert_border_consistency(
    partitions: &[Partition],
    borders: &HashMap<u32, (PortRef, PortRef)>,
) {
    for (&border_id, _) in borders {
        let mut found_in: Vec<WorkerId> = Vec::new();
        for partition in partitions {
            if partition.free_port_index.contains_key(&border_id) {
                found_in.push(partition.worker_id);
            }
        }
        assert_eq!(
            found_in.len(), 2,
            "C3 violated: borderId {} found in {} partitions (expected: 2)",
            border_id, found_in.len()
        );
        assert_ne!(
            found_in[0], found_in[1],
            "C3 violated: borderId {} found twice in the same partition {}",
            border_id, found_in[0]
        );
    }
}
```

Note: A full C4 round-trip assertion (`merge(split(net)) ~ net`) is defined in SPEC-05, since it requires the merge operation. In SPEC-04, the assertions cover only the structural conditions that split itself must guarantee.

### 4.9 Diagram of the Partition-Reduce-Merge Cycle

```
Original Net (Net)
    |
    v
[1] split(net, n, strategy)
    |
    |--- sigma = strategy.allocate(net, n)
    |--- Classify wires (internal vs. interface vs. border)
    |--- Generate border IDs and FreePort (Boundary) sentinels
    |--- Compute ID ranges
    |
    v
 PartitionPlan { partitions: [P0, P1, ..., Pn-1], borders: {...} }
    |
    v
[2] For each Pi (in parallel, on different workers):
    |   Pi.subnet = reduce_all(Pi.subnet)       [SPEC-03]
    |   -- FreePort (Boundary) sentinels act as impermeable walls
    |   -- New agents receive IDs from the reserved range
    |   -- free_port_index may change due to reconnection/erasure
    |
    v
 PartitionPlan with locally reduced sub-nets
    |
    v
[3] merge(plan) -> Recombined Net                [SPEC-05]
    |
    |--- Collect reduced sub-nets from all workers
    |--- Restore border wires via free_port_index + border map
    |--- Detect new redexes (including border redexes)
    |
    v
 Recombined Net (may have residual redexes)
    |
    v
[4] If redexes remain: go to [1] (new round)
    If no redexes: NORMAL FORM REACHED
```

---

## 5. Rationale

### 5.1 Any Valid Partitioning Preserves Correctness

**Decision:** The partitioning subsystem does not require topology-aware or redex-aware allocation.

**Justification:** DISC-004 v2 (Section 1.6) establishes that any partitioning sigma satisfying C1-C3 permits correct local reduction. The argument has two parts:

1. **Equivalence of Local Reduction** (DISC-004 v2, Section 1.5; ARG-002, Part II, Steps 5-7): Reducing an Active Pair `(a, b)` entirely within partition `mu_i` produces the same topological changes as reducing it in the global net `mu`. This follows from Locality (REF-001, p.96; REF-013, p.219: "the part of the net being reduced can be cut out of the net, and the reduced net connected back in, independently of other possible reductions in the net").

2. **Order Independence** (ARG-002, Passo 10): Strong confluence (REF-002, Proposition 1) guarantees that any strategy that reduces all redexes reaches the same Normal Form. The partition-reduce-merge cycle implements a valid strategy (first internal redexes, then border redexes, repeated). The choice of sigma merely determines *which* redexes are internal vs. border in each round -- affecting the number of rounds and communication overhead, never the final result.

The Haskell prototype validates this empirically: 0 failures in ~110 tests with "blind" partitioning (AC-002), confirming that even the simplest allocation function preserves correctness.

### 5.2 Contiguous ID Range as Baseline Strategy

**Decision:** Use contiguous ID range partitioning as the default allocation function.

**Justification:** It is the same strategy as the Haskell prototype (AC-002), enabling direct comparison between prototype and Relativist. It is O(A) in time, deterministic, and trivial to implement. The disadvantage (ignores topology, may maximize border redexes, AC-002 L1) is accepted within the scope of this TCC: the goal is to validate the Fundamental Property `reduce_all(net) ~ run_grid(net, n)` (graph isomorphism), not to optimize performance. The `PartitionStrategy` trait (R21-R23) allows adding better strategies in the future.

**Alternatives considered and retained as future work:**
- **Redex-aware** (DISC-004 v2, Section 2.4): Group agents of Active Pairs as indivisible units. Minimizes border redexes in the first round, but cannot anticipate emergent border redexes from CON-DUP. Moderate additional complexity.
- **Topology-aware via BFS/DFS** (DISC-004 v2, Section 2.3): Grow partitions from seeds. Reduces border wires. Complexity O(A + W). Standard in graph partitioning (METIS, KaHIP), but without IC-specific validation.
- **Static analysis (Mackie)** (REF-013): Approximates interacting agent pairs via abstract interpretation. Never implemented automatically; the underlying problem is undecidable (REF-013, p.222). Out of scope.

### 5.3 Static ID Space Partitioning Instead of Post-Reduction Remapping

**Decision:** Each worker receives a pre-allocated ID range, eliminating `remapAllPartitions`.

**Justification:** The Haskell prototype uses post-facto remapping (AC-003): after local reduction, the coordinator traverses all agents in each partition, renaming IDs that collide. This adds an O(A) phase per round and is prone to bugs (AC-002, L2: "risk of AgentId collision"). The HVM4 model (AC-015, CC-4) demonstrates that static partitioning of the ID space is superior: "each thread has a disjoint slice, zero coordination, zero collision, O(1) allocation." With 8 workers and `u32`, each range has ~537M IDs, more than sufficient.

### 5.4 FreePort Index Instead of Linear Scan

**Decision:** Each partition maintains a `HashMap<u32, PortRef>` for O(1) FreePort lookup.

**Justification:** In the Haskell prototype, `freePortNeighbor` scans ALL wires in a partition to find what is connected to a specific FreePort (AC-002, L3): "O(W) per border, O(W*B) total." The FreePort index reduces this to O(B) total (one O(1) lookup per border). DISC-004 v2 (Section 7.2, OM-3) explicitly recommends this optimization.

### 5.5 split as a Pure Function, merge as the Complement

**Decision:** This spec defines only the split operation. The merge operation (recombining sub-nets after local reduction) is defined in SPEC-05.

**Justification:** Separating split and merge into distinct specs reflects the temporal separation in the grid protocol: split occurs once at the beginning of each round (on the Coordinator), while merge occurs once at the end (also on the Coordinator). The two operations are inverses when no reduction occurs between them (ARG-002, Part I: `merge(split(mu)) = mu`), but after local reduction, the merge must handle evolved sub-nets including potentially missing agents (erasure), new agents (CON-DUP), and changed FreePort connections. This additional complexity warrants its own spec.

### 5.6 Foundations for SPEC-01 Premises P2 and D1

This spec provides the concrete mechanism through which SPEC-01 invariants D1 (Split/Merge Identity) and D2 (Equivalence of Local Reduction) are upheld:

- **D1 (P2a):** The split operation, by satisfying C1-C3 (R6-R8), guarantees that `merge(split(net)) ~ net` (ARG-002, Part I, Steps 1-4). The formal derivation:
  - C1 ensures no agent is lost or duplicated (Step 1).
  - C2 ensures no wire is lost (Step 2).
  - C3 ensures border wires can be univocally restored (Step 3).
  - Together, the merge reconstructs the original net exactly (Step 4).

- **D2 (P2b):** The split preserves internal connections intact (R9) and replaces border connections with FreePort sentinels that correctly inherit the connection identity (R11). By Locality, reducing an internal Active Pair in a partition produces the same result as in the global net (ARG-002, Part II, Steps 5-7).

---

## 6. Haskell Prototype Reference

### 6.1 Function `partitionNet` (AC-002, lines 55-95)

The prototype divides `Map.keys(netAgents)` into `n` contiguous chunks via `splitIntoN`. For each wire, `classifyWires` determines whether it is local or border using lookup in `agentAssign :: Map AgentId WorkerId`. Borders receive sequential IDs starting from `maxFreePortId + 1`. Each partition is constructed with its agents, local wires, and FreePort-bearing wires.

**What Relativist changes:**
- Replaces `Map AgentId Agent` with `Vec<Option<Agent>>` and `[Wire]` with flat port array (SPEC-02). Wire classification operates over the port array, not a list of wires.
- Adds `free_port_index` per partition for O(1) lookup (not present in the prototype).
- Adds static ID space partitioning (not present in the prototype).
- Abstracts the allocation function via the `PartitionStrategy` trait (hardcoded in the prototype).
- Improves complexity from O((A + W) * log A) to O(A + W) by using `Vec` indexed by `AgentId` instead of `Map`.

### 6.2 Function `mergePartitions` (AC-002, lines 104-127)

The prototype unions all agents via `Map.unions`, filters border wires, and reconnects borders via `freePortNeighbor` (linear scan across all partitions).

**What Relativist changes (detailed in SPEC-05):**
- Uses `free_port_index` for O(1) lookup per border (eliminates linear scan).
- Does not need `remapAllPartitions` because IDs are pre-allocated (R16-R19).
- The redex queue is populated incrementally by `connect` (SPEC-02, R13) instead of global `findRedexes`.

### 6.3 Function `findBorderRedexes` (AC-002, lines 137-152)

The prototype traverses all borders and checks whether both endpoints are principal ports. Used by IC.Grid to decide if more rounds are needed.

**What Relativist changes (detailed in SPEC-05):**
- The merge detects border redexes automatically: `connect(port_a, port_b)` inserts into the redex queue when both are principal ports (SPEC-02, R13). A separate `findBorderRedexes` function is unnecessary.

---

## 7. Open Questions

None. All questions necessary for implementation have been resolved in this spec.

Performance questions (optimal partitioning granularity, communication overhead vs. parallelism benefit) are empirical and will be addressed in SPEC-09 (Benchmarks), per gray zones Z4, Z6, Z7.
