# SPEC-05: Partition Merge and Grid Cycle

**Status:** Revised v3
**Depends on:** SPEC-01 (Invariants), SPEC-02 (Net Representation), SPEC-03 (Reduction Engine), SPEC-04 (Partitioning)
**Gray zones resolved:** Z3 (cross-boundary interaction protocol)
**References consumed:** REF-001 (Lafont 1990), REF-002 (Lafont 1997), REF-003 (HVM2), REF-005 (Mackie & Pinto 2002), REF-013 (Mackie 1997), REF-014 (Kahl 2015)
**Discussions consumed:** DISC-003 v2 (strong confluence to distributed determinism, P1-P5), DISC-004 v2 (formal partitioning, isomorphism split/merge), DISC-005 v2 (centralized merge protocol, border redexes, alternatives analysis)
**Arguments consumed:** ARG-001 (central argument, P1-P6), ARG-002 (partitioning preserves structure, merge(split(net)) = net), ARG-003 (merge protocol guarantees frontier resolution completeness, P3)
**Code analyses consumed:** AC-002 (Haskell IC.Partition: mergePartitions, findBorderRedexes, freePortNeighbor), AC-003 (Haskell IC.Protocol/Network: remapAllPartitions, gridLoop), AC-004 (Haskell IC.Grid: GridMetrics, go loop), AC-015 (cross-cutting synthesis: CC-4 static ID space partitioning)
**Review history:** Round 1 critic (SPEC-05-round1-critic.md, 14 issues: 2 CRITICAL, 3 HIGH, 4 MEDIUM, 5 LOW), Round 2 defender response (SPEC-05-round2-defender.md)

---

## 1. Purpose

This spec defines how Relativist reconstructs the net after distributed reduction (partition merge), detects and resolves border redexes, and orchestrates the complete grid computing cycle until the net reaches its Normal Form. The merge is the logical complement of the split operation (SPEC-04): where SPEC-04 decomposes the net into sub-nets with FreePort (Boundary) sentinels at cut points, SPEC-05 recombines those sub-nets by restoring the original connections via border IDs, resolves the Active Pairs that emerge at boundaries, and decides whether further rounds are needed.

The relationship between split and merge is formally characterized by the split/merge identity (SPEC-01, D1; ARG-002, Part I): for a non-reduced net, `merge(split(net)) ~ net`. For sub-nets that have undergone local reduction, the merge reconstructs the net with updated agents and updated wire topologies, then resolves border redexes to produce a globally consistent net.

The correctness of merge, combined with strong confluence (SPEC-01, T4), guarantees the Fundamental Property of Relativist (SPEC-01, G1):

```
reduce_all(net) ~ extract_result(run_grid(net, n))
```

## 2. Definitions

Terms defined in SPEC-00 (Glossary, when written) and in SPEC-01/SPEC-02/SPEC-04 are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **Merge** | The process of recombining locally-reduced partitions back into a single Net. The merge unites agents from all partitions, copies internal connections, restores boundary connections via the `free_port_index` and border map, and populates the redex queue of the resulting net. Merge is the inverse of split: it resolves FreePort (Boundary) sentinels back into direct wires. |
| **Border Redex** | An Active Pair (redex) whose two agents resided in different partitions before the merge. Detected when the restoration of a boundary connection creates a wire between two principal ports (`AgentPort(_, 0) <-> AgentPort(_, 0)`). Equivalent to Cross-Boundary Active Pair (SPEC-01, D3). |
| **Emergent Border Redex** | A border redex that did not exist in the original net, but emerged because a local reduction rule (especially CON-DUP commutation) reconnected an auxiliary port to a FreePort (Boundary), and the agent on the other side of the boundary (in another partition) also has its principal port free. Only detectable after merge. Captured in the same round (during merge reconnection) or in a subsequent round (DISC-005 v2, Section 1.3; ARG-003, R2b). |
| **Round** | One complete iteration of the grid computing cycle: split, distribute, reduce_local, collect, merge, resolve_borders. Each round consumes at least one interaction from the total interaction budget (SPEC-01, T7). |
| **Grid Loop** | The outer loop that repeats Rounds until the net reaches Normal Form (no redexes). Equivalent to `gridLoop`/`go` in the Haskell prototype (AC-004). |
| **GridMetrics** | A structure that accumulates execution metrics across rounds: round count, interactions per round, border redexes per round, timings per phase. Essential for benchmarks (SPEC-09). Inspired by `GridMetrics` in the Haskell prototype (AC-004). |

---

## 3. Requirements

### 3.1 Partition Merge

**R1.** The `merge` function MUST accept as input a `PartitionPlan` (consumed by value) containing the partitions after local reduction and the border map, and return a single recombined `Net` plus the number of border redexes detected. The function is the pure merge step; post-merge `reduce_all` is invoked separately by the caller. In SPEC-13's coordinator FSM, the `InvokeMergeAndReduce` action encapsulates both `merge()` and `reduce_all()` as a single coordinator-level convenience; the `is_normal_form` field in `MergeComplete` is computed after `reduce_all`, not by `merge()` itself. **(MUST)**

**R2.** The merge MUST unite agents from all partitions into a single arena. There MUST NOT be AgentId collisions between partitions. This is guaranteed by static ID space partitioning (SPEC-04, R16-R19), which assigns each worker a disjoint ID range. **(MUST)**

**R3.** The merge MUST copy all internal connections (wires not involving FreePort (Boundary) sentinels) from each partition to the result net. **(MUST)**

**R4.** For each `border_id` in the border map, the merge MUST consult the `free_port_index` of the partitions to determine the `PortRef` currently connected to `FreePort(border_id)` on each side. Each border_id appears in exactly two distinct partitions (SPEC-04, R8/C3). **(MUST)**

**R5.** The merge MUST restore boundary connections by connecting the two endpoints found via the `free_port_index` for each border_id, replacing FreePort (Boundary) sentinels with a direct wire. The reconnection MUST use `Net::connect` (SPEC-02, R13), which detects new redexes on-the-fly when both endpoints are principal ports. **(MUST)**

**R6.** If during the merge one side of a border wire cannot be found (because the agent was removed by an erasure rule during local reduction, and the `free_port_index` no longer contains the entry for that border_id), the border wire MUST be discarded silently. This is NOT an error: strong confluence guarantees the result remains correct (DISC-005 v2, Section 4.1; AC-002, `reconnectBorder _ -> Nothing`). **(MUST)**

**R7.** If both sides of a border wire have been removed by erasure, the border MUST be discarded silently. **(MUST)**

**R8.** The `next_id` of the result net MUST be the maximum of all `next_id` values from the partitions. This ensures that agents created in future rounds do not collide with any partition's agents. **(MUST)**

**R9.** The redex queue of the result net MUST contain new redexes created by boundary reconnection (detected automatically by `Net::connect` per SPEC-02, R13). Partition redex queues are discarded during merge because `reduce_all` (SPEC-03 R13) guarantees that after full local reduction, the partition queue contains only stale entries (referring to agents already consumed). In debug mode, a diagnostic assertion SHOULD verify that no non-stale redexes exist in any partition queue at merge time (see Section 4.2, Step 2 note). **(MUST)**

**R10.** The merge MUST satisfy invariant D1 of SPEC-01 (split/merge identity): if no local reduction has occurred, `merge(split(net, plan)) ~ net` (structural isomorphism modulo agent ID renaming). This is the foundational correctness guarantee, derived from ARG-002 Part I (Steps 1-4). **(MUST)**

**R11.** After merge, in debug mode (`#[cfg(debug_assertions)]`), the result net MUST be verified with `assert_all_invariants()` (SPEC-02, Section 4.6), checking at minimum: T1 (linearity/bidirectionality), I1 (bidirectional port array), I2 (reference validity). **(MUST)**

### 3.2 Border Redex Detection

**R12.** Border redex detection MUST be automatic and incremental: when `Net::connect(a, b)` restores a boundary connection and both `a` and `b` are `AgentPort(_, 0)` (principal ports), the pair is inserted into the redex queue automatically (SPEC-02, R13). No separate `findBorderRedexes` function is required. **(MUST)**

**R13.** The number of border redexes detected during merge MUST be recorded in `GridMetrics.border_redexes_per_round` for experimental analysis (SPEC-09). **(MUST)**

**R14.** Detection MUST be efficient: O(B) where B is the number of border wires, since each border is reconnected exactly once. **(MUST)**

### 3.3 Border Redex Resolution

**R15.** After the merge, the coordinator MUST reduce all border redexes (and any new redexes generated by those reductions) by invoking `reduce_all` (SPEC-03) on the merged net. This approach is strictly more complete than the Haskell prototype's selective resolution (`foldl reduce merged borderPairs`, AC-004), which may leave non-border redexes generated by CON-DUP cascades for the next round. **(MUST)**

**R16.** Border redex resolution MUST use the same reduction engine as local reduction (SPEC-03). There are no special rules for border redexes: they are ordinary Active Pairs subject to the same 6 interaction rules (SPEC-01, T5). **(MUST)**

**R17.** The resolution of border redexes may generate new redexes (especially via the CON-DUP commutation rule, which creates 4 new agents). These derived redexes MUST be resolved within the same `reduce_all` invocation, not deferred to the next round. **(MUST)**

**R18.** The `reduce_all` after merge MUST process all redexes in the merged net's queue: border redexes detected by `connect()` during merge reconnection, and any derived redexes generated by their resolution (e.g., CON-DUP cascades). Since `reduce_all` was invoked on each partition before merge, partition queues are guaranteed to contain only stale entries at merge time (R9); therefore the merged queue contains only border-origin redexes. Strong confluence guarantees that the order of resolution is irrelevant (SPEC-01, T4; DISC-005 v2, Section 8.2; ARG-001, Step 4). **(MUST)**

**R19.** The number of interactions performed during border redex resolution MUST be recorded separately in `GridMetrics.border_interactions_per_round` to distinguish boundary work from local work. **(MUST)**

### 3.4 FreePort Index Reconstruction

**R20.** Before returning a locally-reduced partition to the coordinator, the worker MUST ensure that the `free_port_index` accurately reflects the current state of FreePort (Boundary) connections. **(MUST)**

**R21.** The baseline approach MUST be lazy reconstruction: a complete scan of the sub-net's port array to build a fresh `HashMap<u32, PortRef>` mapping each `border_id` to its current `AgentPort` endpoint. This is recommended by SPEC-04 Section 4.6 as the simplest and least error-prone approach. **(MUST)**

**R22.** The reconstruction MUST handle three scenarios correctly (SPEC-04, Section 4.6 informative scenarios):
1. **Reconnection:** An agent connected to `FreePort(bid)` participates in a local redex; the rule reconnects `FreePort(bid)` to a new agent. The index must reflect the new endpoint.
2. **Erasure:** The agent connected to `FreePort(bid)` is destroyed by an erasure rule. The index no longer contains `bid`. During merge, the border is discarded silently (R6).
3. **CON-DUP with FreePort:** One of the original agents had an auxiliary port connected to `FreePort(bid)`. The 4 new agents inherit connections. The index must point to whichever new agent inherited the FreePort connection.
**(MUST)**

**R23.** The lazy reconstruction SHOULD have complexity O(A_i * PORTS_PER_SLOT) per partition, where A_i is the number of live agents. This is dominated by the cost of local reduction. **(SHOULD)**

### 3.5 Grid Loop

**R24.** Relativist MUST implement a function `run_grid` that executes the complete distributed reduction cycle: split, distribute, reduce_local, collect, merge, resolve_borders, and repeat until Normal Form. **(MUST)**

**R25.** The `run_grid` function MUST accept as input: (a) the initial `Net`, (b) the number of workers `n >= 1`, (c) the partitioning strategy (trait `PartitionStrategy`, SPEC-04, R21). It MUST return: (a) the `Net` in Normal Form (or partial result if timeout), (b) the accumulated `GridMetrics`. **(MUST)**

**R26.** If `n == 1`, `run_grid` SHOULD reduce locally without partitioning (degenerate case, equivalent to `reduce_all`). This avoids unnecessary split/merge overhead. **(SHOULD)**

**R27.** The termination condition of the grid loop MUST be: the net is in Normal Form (redex queue empty after `reduce_all`). If after merge and border resolution no redexes remain, the loop terminates. **(MUST)**

**R28.** If after merge and border resolution redexes still exist, a new round MUST be initiated: the net is re-partitioned and the cycle repeats. **(MUST)**

**R29.** The grid loop SHOULD have a configurable maximum round limit (`max_rounds: Option<u32>`). If `max_rounds` is `Some(limit)` and the limit is reached without convergence, the loop SHOULD terminate and return the net in its current state with a non-convergence indicator in the metrics. **(SHOULD)**

**R30.** For terminating nets (scope of this TCC), the grid loop MUST converge in a finite number of rounds. The justification is: each round reduces at least all internal redexes of all partitions; strong confluence guarantees that no reduction is "wasted"; and the total number of interactions is finite and invariant (SPEC-01, T7). Therefore each round makes progress and the grid loop converges. **(MUST for terminating nets)**

### 3.6 Completeness (Premise P3 of the Formal Argument Framework)

**R31.** The grid protocol MUST satisfy premise P3 (SPEC-01, D3; ARG-001; ARG-003): every redex of the original net MUST eventually be reduced, including border redexes and emergent border redexes. **(MUST)**

**R32.** Completeness MUST be guaranteed by construction through three mechanisms (DISC-005 v2, Section 4.2; ARG-003, Part I):
1. **Bidirectional FreePort:** Each cut wire generates exactly two FreePort (Boundary) sentinels with the same `border_id` in exactly two distinct partitions (SPEC-04, R8/C3).
2. **Exhaustive reconnection:** The merge restores all borders registered in the border map (R4-R5). No border is skipped.
3. **Cycle until stabilization:** The grid loop repeats while redexes exist (R27-R28). Emergent border redexes created by CON-DUP in round N are detected during the merge of round N (if the new agents have already inherited FreePort connections to principal ports) or at the split/merge of round N+1 (when re-partitioning places the new agents in context where they can be matched).
**(MUST)**

**R33.** The operational reformulation of P3 (DISC-005 v2, Section 4.4; ARG-003, Part III, Step 13) MUST be adopted: "The partition-reduce-merge cycle eventually resolves all redexes of the net. If the original net has a Normal Form, the grid protocol reaches it." **(MUST)**

### 3.7 Metrics

**R34.** Relativist MUST collect per-round metrics in a `GridMetrics` structure to enable the benchmarks of SPEC-09. **(MUST)**

**R35.** The metrics MUST include at least:
- `rounds: u32` -- total number of rounds executed.
- `total_interactions: u64` -- sum of all interactions (local + border) across all rounds.
- `total_interactions_by_rule: [u64; 6]` -- per-rule interaction totals: [CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA]. Accumulated from `WorkerRoundStats.interactions_by_rule` (workers) and border-resolution `ReductionStats` (coordinator). Required by SPEC-11 R12 for the `interactions_by_rule_total` Prometheus metric.
- `local_interactions_per_round: Vec<u64>` -- local worker interactions per round.
- `border_interactions_per_round: Vec<u64>` -- border interactions (coordinator, after merge) per round.
- `border_redexes_per_round: Vec<u32>` -- border redexes detected by merge per round.
- `agents_per_round: Vec<usize>` -- number of live agents at the start of each round.
- `partition_time_per_round: Vec<Duration>` -- partitioning time per round.
- `compute_time_per_round: Vec<Duration>` -- local reduction time (all workers) per round. In local simulation, this includes `rebuild_free_port_index` time. In distributed mode (SPEC-06), it measures the coordinator's wall-clock waiting time for all workers.
- `merge_time_per_round: Vec<Duration>` -- structural merge time per round (excludes border resolution).
- `border_reduce_time_per_round: Vec<Duration>` -- time for `reduce_all` after merge per round (border resolution).
- `total_time: Duration` -- wall-clock total.
**(MUST)**

**R35a.** The metrics SHOULD additionally include:
- `index_rebuild_time_per_round: Vec<Duration>` -- time for `rebuild_free_port_index` per round (timed separately from `reduce_all`). Enables accurate overhead decomposition for SPEC-09 benchmarks.
**(SHOULD)**

**R36.** In distributed context (SPEC-06), the metrics SHOULD additionally include:
- `network_send_time_per_round: Vec<Duration>` -- time to send partitions.
- `network_recv_time_per_round: Vec<Duration>` -- time to receive results.
- `bytes_sent_per_round: Vec<usize>` -- bytes sent per round.
- `bytes_received_per_round: Vec<usize>` -- bytes received per round.
- `worker_stats_per_round: Vec<Vec<WorkerRoundStats>>` -- per-worker statistics per round.
**(SHOULD)**

**R37.** The `WorkerRoundStats` structure MUST contain:
- `worker_id: WorkerId`
- `agents_before: usize` -- agents in the partition before local reduction.
- `agents_after: usize` -- agents in the partition after local reduction.
- `local_redexes: usize` -- local redexes reduced.
- `reduce_duration_secs: f64` -- wall-clock duration of `reduce_all` for this worker (seconds).
- `interactions_by_rule: [u64; 6]` -- per-rule interaction counts: [CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA]. Copied directly from SPEC-03's `ReductionStats.interactions_by_rule` (SPEC-03 R17, Section 4.3.1). The index order matches the `SpecificRule` enum discriminants defined in SPEC-03.

> **Note on per-rule tracking (OQ-4 RESOLVED):** SPEC-03 now tracks 6 per-rule counters directly via `ReductionStats.interactions_by_rule: [u64; 6]` and the `SpecificRule` enum (Section 4.3.1). No mapping or disambiguation is needed; the values can be copied directly from the `ReductionStats` returned by `reduce_all`.

This struct MUST derive `serde::Serialize` and `serde::Deserialize` for wire transmission (SPEC-06 R12). This definition supersedes the SPEC-11 OQ-1 extension; SPEC-11 OQ-1 is resolved.
**(MUST)**

### 3.8 Complexity

**R38.** The merge MUST have time complexity O(A_total + B) where `A_total` is the total number of agents across all partitions and `B` is the number of borders. **(MUST)**

**R39.** Border redex detection MUST have complexity O(B), since it occurs on-the-fly during reconnection (R12). **(MUST)**

**R40.** Border redex resolution has complexity O(S_border) where `S_border` is the number of interactions needed to resolve all border redexes and their derived redexes. In the worst case (CON-DUP cascade), `S_border` can significantly exceed `B`. **(informative)**

---

## 4. Design

### 4.1 Types

```rust
use std::time::Duration;

/// Metrics collected during grid loop execution.
///
/// Inspired by GridMetrics from the Haskell prototype (AC-004),
/// with per-round granularity for experimental analysis.
#[derive(Debug, Clone, Default)]
pub struct GridMetrics {
    /// Total number of rounds executed.
    pub rounds: u32,

    /// Sum of all interactions (local + border) across all rounds.
    pub total_interactions: u64,

    /// Per-rule interaction totals across all rounds and all sources
    /// (workers + border resolution):
    /// [CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA].
    /// Required by SPEC-11 R12 for Prometheus `interactions_by_rule_total`.
    pub total_interactions_by_rule: [u64; 6],

    /// Local worker interactions per round.
    pub local_interactions_per_round: Vec<u64>,

    /// Border interactions (coordinator, after merge) per round.
    pub border_interactions_per_round: Vec<u64>,

    /// Border redexes detected by merge per round.
    pub border_redexes_per_round: Vec<u32>,

    /// Number of live agents at the start of each round.
    pub agents_per_round: Vec<usize>,

    /// Partitioning time per round.
    pub partition_time_per_round: Vec<Duration>,

    /// Local reduction time (all workers) per round.
    /// In local simulation, includes rebuild_free_port_index unless
    /// index_rebuild_time_per_round is separately tracked.
    pub compute_time_per_round: Vec<Duration>,

    /// Structural merge time per round (excludes border resolution).
    pub merge_time_per_round: Vec<Duration>,

    /// Time for reduce_all after merge per round (border resolution).
    pub border_reduce_time_per_round: Vec<Duration>,

    /// Time for rebuild_free_port_index per round (SHOULD, R35a).
    /// Enables accurate overhead decomposition for SPEC-09 benchmarks.
    pub index_rebuild_time_per_round: Vec<Duration>,

    /// Wall-clock total execution time.
    pub total_time: Duration,

    /// Did the grid converge to Normal Form?
    /// false if max_rounds was reached before convergence.
    pub converged: bool,

    /// Per-worker statistics, per round (populated in distributed context).
    pub worker_stats_per_round: Vec<Vec<WorkerRoundStats>>,
}

/// Statistics of a single worker in a specific round.
/// Canonical definition: SPEC-05 R37. Resolves SPEC-11 OQ-1.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkerRoundStats {
    pub worker_id: WorkerId,
    pub agents_before: usize,
    pub agents_after: usize,
    pub local_redexes: usize,
    /// Wall-clock duration of reduce_all for this worker (seconds).
    pub reduce_duration_secs: f64,
    /// Per-rule interaction counts:
    /// [CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA].
    pub interactions_by_rule: [u64; 6],
}
```

```rust
/// Configuration for the grid loop.
#[derive(Debug, Clone)]
pub struct GridConfig {
    /// Number of workers.
    pub num_workers: u32,

    /// Maximum number of rounds. None = no limit.
    pub max_rounds: Option<u32>,

    // Note: the partition strategy is passed as a parameter to run_grid
    // rather than stored here, because trait objects are not Clone.
    // The ENGINEER decides the approach (generic, Box<dyn>, etc.).
}
```

### 4.2 Merge Algorithm

```
fn merge(plan: PartitionPlan) -> (Net, u32)
// Accepts PartitionPlan by value (consumes partitions and borders).
// Returns: (merged net, number of border redexes detected)
```

> **Note on API design:** The function accepts `PartitionPlan` by value rather than separate `Vec<Partition>` and `&HashMap<u32, (PortRef, PortRef)>` arguments. This avoids partial move issues in Rust (cannot move `plan.partitions` while borrowing `plan.borders`) and is more natural since `PartitionPlan` already bundles both components. Internally, the function destructures: `let PartitionPlan { partitions, borders, .. } = plan;`

**Pre-conditions:**
- All partitions have been locally reduced and their `free_port_index` has been reconstructed (R20-R22).
- AgentIds across partitions are mutually disjoint (guaranteed by SPEC-04, R16-R19 static ID space partitioning).
- The `borders` map is the border map from the original `PartitionPlan`.

**Post-conditions:**
- The result net contains all live agents from all partitions.
- All FreePort (Boundary) sentinels have been resolved (either reconnected or discarded).
- Pre-existing Lafont FreePorts are preserved in the result net.
- The redex queue contains only newly detected border redexes (from boundary reconnection via `connect`). Partition queues are discarded (guaranteed stale after `reduce_all`, R9).
- In debug mode, `assert_all_invariants()` passes.

**Step 1: Compute capacity.**
```
total_agents = sum(partition.subnet.agents.len() for partition in partitions)
max_next_id  = max(partition.subnet.next_id for partition in partitions)
result = Net::with_capacity(total_agents)
result.next_id = max_next_id
```

**Step 2: Unite agents and internal connections.**
For each partition:
```
for (i, slot) in partition.subnet.agents.iter().enumerate():
    if let Some(agent) = slot:
        result.agents[i] = Some(agent)  // expand if necessary
        // Copy connections for ALL ports of this agent
        for p in 0..total_ports(agent.symbol):
            target = partition.subnet.get_target(AgentPort(agent.id, p))
            if target != DISCONNECTED:
                match target:
                    AgentPort(_, _):
                        // Internal connection: copy directly
                        result.set_port(AgentPort(agent.id, p), target)
                    FreePort(fid):
                        if borders.contains_key(&fid):
                            // Boundary FreePort: will be restored in Step 3
                            // Temporarily mark as DISCONNECTED
                            result.set_port(AgentPort(agent.id, p), DISCONNECTED)
                        else:
                            // Lafont FreePort (pre-existing interface port):
                            // copy directly (SPEC-04, R15)
                            result.set_port(AgentPort(agent.id, p), FreePort(fid))
```

> **Note on FreePort distinction (cf. SPEC-04 R15, SPEC-00 Sections 6.1/6.2):** Both Lafont FreePorts (pre-existing interface ports of the original net) and Boundary FreePorts (synthetic markers from split) use the same `FreePort(u32)` variant. The border map serves as the discriminator: if the `fid` appears in the border map, it is a Boundary FreePort; otherwise, it is a Lafont FreePort that must be preserved in the merged net.

**Debug assertion on partition queues:** After reduce_all, partition queues SHOULD contain only stale entries. In debug mode, the merge SHOULD verify this:
```
debug_assert!(
    partition.subnet.redex_queue.iter().all(|(a, b)|
        !partition.subnet.is_valid_redex(*a, *b)
    ),
    "Non-stale redexes found in partition {} queue after reduce_all (bug in reduction engine)",
    partition.worker_id
);
```

**Step 3: Restore boundary connections.**
```
border_redex_count = 0

for (border_id, (_orig_a, _orig_b)) in &borders:
    // Look up the current endpoint for this border_id in each partition
    current_a: Option<PortRef> = None
    current_b: Option<PortRef> = None

    for partition in &partitions:
        if let Some(port_ref) = partition.free_port_index.get(&border_id):
            if current_a.is_none():
                current_a = Some(*port_ref)
            else:
                current_b = Some(*port_ref)

    match (current_a, current_b):
        (Some(port_a), Some(port_b)):
            result.connect(port_a, port_b)
            // connect() automatically inserts into redex queue if both
            // are principal ports (SPEC-02, R13)
            if is_principal_pair(port_a, port_b):
                border_redex_count += 1
        (Some(_), None) | (None, Some(_)):
            // One side removed by erasure. Discard silently (R6).
            pass
        (None, None):
            // Both sides removed. Discard silently (R7).
            pass

return (result, border_redex_count)
```

Where `is_principal_pair` is:
```rust
fn is_principal_pair(a: PortRef, b: PortRef) -> bool {
    matches!(
        (a, b),
        (PortRef::AgentPort(_, 0), PortRef::AgentPort(_, 0))
    )
}
```

**Complexity:** O(A_total + B), satisfying R38.

### 4.3 FreePort Index Lazy Reconstruction

As specified in SPEC-04 Section 4.6 and required by R20-R22, the baseline approach is lazy reconstruction: before returning the locally-reduced partition to the coordinator, the worker rebuilds the `free_port_index` by scanning the port array:

```
fn rebuild_free_port_index(subnet: &Net) -> HashMap<u32, PortRef> {
    let mut index = HashMap::new();
    for (i, slot) in subnet.agents.iter().enumerate():
        if let Some(agent) = slot:
            for p in 0..total_ports(agent.symbol):
                let target = subnet.get_target(AgentPort(agent.id, p))
                if let FreePort(border_id) = target:
                    index.insert(border_id, AgentPort(agent.id, p))
    index
}
```

**Complexity:** O(A_i * PORTS_PER_SLOT) per partition, where A_i is the number of live agents. Total across all partitions: O(A_total * PORTS_PER_SLOT), which is O(A_total) in practice.

**Scenario coverage (informative):**

1. **Reconnection:** Agent `a` connected to `FreePort(bid)` participates in a local redex. The rule reconnects `FreePort(bid)` to a new agent `d`. After reconstruction: `index[bid] = AgentPort(d, p)`.

2. **Erasure:** Agent `a` connected to `FreePort(bid)` is destroyed by an erasure rule. The wire `(AgentPort(a, p), FreePort(bid))` disappears. After reconstruction: `index` does not contain `bid`. During merge, the border is discarded (R6).

3. **CON-DUP with FreePort:** One of the original agents had an auxiliary port connected to `FreePort(bid)`. The 4 new agents created by CON-DUP inherit connections. After reconstruction: `index[bid]` points to whichever new agent inherited the FreePort connection.

### 4.4 Stale Redex Draining

The redex queue may contain stale entries (SPEC-02, Section 2; SPEC-03). Before declaring Normal Form, it is necessary to confirm there are no valid redexes hidden behind stale entries.

```rust
/// Traverses the redex queue and discards all stale entries.
/// After this function, the queue contains only valid redexes.
fn drain_stale_redexes(net: &mut Net) {
    let mut valid = VecDeque::new();
    while let Some((a, b)) = net.redex_queue.pop_front() {
        if net.is_valid_redex(a, b) {
            valid.push_back((a, b));
        }
    }
    net.redex_queue = valid;
}
```

**Complexity:** O(Q) where Q is the size of the redex queue.

**R41 (Normal Form verification).** After the final `reduce_all` in each round, and before declaring Normal Form, the grid loop SHOULD perform a full scan of the port array to detect any redexes not present in the queue. This scan SHOULD be enabled by default in debug mode (`#[cfg(debug_assertions)]`) and MAY be disabled in release mode via a configuration option (e.g., `GridConfig.verify_normal_form: bool`). The full scan has complexity O(A * PORTS_PER_SLOT), which is dominated by the merge cost. For a research prototype whose primary goal is validating G1, correctness assurance takes priority over performance. **(SHOULD)**

**Alternative (informative):** The `drain_stale_redexes` approach alone is sufficient if `connect` (SPEC-02, R13) is correct, because every created redex is inserted into the queue. The full scan serves as a defense-in-depth measure: if `connect()` has a bug that fails to insert a redex, the drain approach would declare Normal Form prematurely, violating G1. The full scan catches this class of bugs.

### 4.5 Grid Loop Algorithm

```
fn run_grid(
    net: Net,
    config: &GridConfig,
    strategy: &dyn PartitionStrategy,
) -> (Net, GridMetrics)
```

> **Note on `num_workers` type:** `GridConfig.num_workers` is `u32`, consistent with SPEC-04's `WorkerId` type. SPEC-13's `run_grid_local` delegates to this function with the same type.

**Pseudocode:**

```
fn run_grid(net, config, strategy) -> (Net, GridMetrics):
    let mut current_net = net
    let mut metrics = GridMetrics::default()
    let start_time = Instant::now()

    // Initial Normal Form check: avoids unnecessary split/merge for
    // nets already in Normal Form. In distributed context (SPEC-13),
    // the coordinator MAY perform this check before entering the BSP
    // loop, or MAY execute one round and detect Normal Form during
    // CheckTermination. Both approaches are correct.
    drain_stale_redexes(&mut current_net)
    if current_net.redex_queue.is_empty():
        metrics.converged = true
        metrics.total_time = start_time.elapsed()
        return (current_net, metrics)

    loop:
        // Check round limit
        if let Some(max) = config.max_rounds:
            if metrics.rounds >= max:
                metrics.converged = false
                break

        metrics.agents_per_round.push(count_live_agents(&current_net))

        // === PHASE 1: SPLIT ===
        let t_partition = Instant::now()
        let plan = split(&current_net, config.num_workers, strategy)
        metrics.partition_time_per_round.push(t_partition.elapsed())

        // === PHASE 2: LOCAL REDUCTION (per worker, in parallel) ===
        let t_compute = Instant::now()
        let mut local_interactions: u64 = 0
        let mut local_by_rule: [u64; 6] = [0; 6]
        let mut worker_stats: Vec<WorkerRoundStats> = Vec::new()

        for partition in &mut plan.partitions:
            let agents_before = count_live_agents(&partition.subnet)
            let redexes_before = partition.subnet.redex_queue.len()

            let t_reduce = Instant::now()
            let reduction_stats: ReductionStats = reduce_all(&mut partition.subnet)
            let reduce_duration = t_reduce.elapsed()
            local_interactions += reduction_stats.total_interactions

            // Per-rule counts are available directly from ReductionStats (SPEC-03 R17)
            let by_rule = reduction_stats.interactions_by_rule
            for i in 0..6: local_by_rule[i] += by_rule[i]

            // Rebuild free_port_index (lazy reconstruction, cf. Section 4.3)
            let t_index = Instant::now()
            partition.free_port_index =
                rebuild_free_port_index(&partition.subnet)
            // index_rebuild_time tracked if R35a is implemented

            let agents_after = count_live_agents(&partition.subnet)
            worker_stats.push(WorkerRoundStats {
                worker_id: partition.worker_id,
                agents_before,
                agents_after,
                local_redexes: redexes_before,
                reduce_duration_secs: reduce_duration.as_secs_f64(),
                interactions_by_rule: by_rule,
            })

        metrics.compute_time_per_round.push(t_compute.elapsed())
        metrics.local_interactions_per_round.push(local_interactions)
        metrics.worker_stats_per_round.push(worker_stats)

        // === PHASE 3: MERGE ===
        let t_merge = Instant::now()
        let (merged_net, border_redex_count) = merge(plan)
        metrics.merge_time_per_round.push(t_merge.elapsed())
        metrics.border_redexes_per_round.push(border_redex_count)

        // === PHASE 4: RESOLVE BORDERS (reduce_all on merged net) ===
        let t_border = Instant::now()
        let border_stats: ReductionStats = reduce_all(&mut merged_net)
        metrics.border_reduce_time_per_round.push(t_border.elapsed())

        let border_by_rule = border_stats.interactions_by_rule
        metrics.border_interactions_per_round
            .push(border_stats.total_interactions)

        // Accumulate per-rule totals
        for i in 0..6:
            metrics.total_interactions_by_rule[i] +=
                local_by_rule[i] + border_by_rule[i]
        metrics.total_interactions +=
            local_interactions + border_stats.total_interactions

        current_net = merged_net
        metrics.rounds += 1

        // === TERMINATION CHECK (aligned with SPEC-13 CheckTermination) ===
        // Check Normal Form after merge + reduce_all.
        // This corresponds to SPEC-13's CheckTermination FSM state,
        // where is_normal_form is computed from the merged-and-reduced net.
        drain_stale_redexes(&mut current_net)
        if current_net.redex_queue.is_empty():
            // Optional: full scan for defense-in-depth (R41)
            #[cfg(debug_assertions)]
            verify_no_redexes_full_scan(&current_net)
            metrics.converged = true
            break

    metrics.total_time = start_time.elapsed()
    (current_net, metrics)
```

> **Note on termination check placement:** The termination check occurs after merge + `reduce_all` (Phase 4), not at the top of the loop. This aligns with SPEC-13's `CheckTermination` FSM state, which transitions from `Merging -> MergeComplete -> CheckTermination` and inspects `is_normal_form` (whether the redex queue is empty after `reduce_all`). The `is_normal_form` field in SPEC-13's `MergeComplete` event is logically equivalent to the `current_net.redex_queue.is_empty()` check after `drain_stale_redexes`.

**Note on Phase 2 (local simulation):** In local simulation mode (no network), workers are processed sequentially in the same process. In distributed mode (SPEC-06), Phase 2 involves: serializing and sending each partition to the worker via TCP, waiting for each worker to return its reduced partition, and deserializing the results. The interface remains the same; the difference is the implementation of "local reduction" (in-process vs. over the network).

**Note on `reduction_stats_to_by_rule` (informative, OQ-4 RESOLVED):** SPEC-03 now provides `ReductionStats.interactions_by_rule: [u64; 6]` directly (see SPEC-03, Section 4.3.1 and 4.6.2). The `reduction_stats_to_by_rule` helper is no longer needed; callers can use `reduction_stats.interactions_by_rule` directly as the `[u64; 6]` array required by R37.

### 4.6 Informal Proof of Completeness (P3)

**Claim:** For every terminating net `mu`, `run_grid(mu, n)` converges to the Normal Form of `mu`.

**Argument (structure from DISC-005 v2, Section 4.2; ARG-003, Parts I-III):**

1. **A terminating net implies a finite number of interactions.** By SPEC-01, T7, the total number of interactions to reach Normal Form is invariant and finite. Let `S` be this number.

2. **Each round makes progress.** In each round:
   - Phase 2 (local reduction) reduces at least the internal redexes of the partitions. If at least one partition has a local redex, at least one interaction occurs.
   - Phase 3 (merge + resolve borders) restores boundary connections and reduces border redexes and any new redexes they generate. If at least one border redex exists, at least one interaction occurs.
   - If neither local redexes nor border redexes existed, the net was already in Normal Form and the loop terminates at the termination check (step [7]) at the end of the current round.

3. **The number of rounds is bounded.** Each round consumes at least one interaction from the total budget S. Therefore, the number of rounds is at most S. In practice, it is much smaller: each round consumes many interactions simultaneously (all local redexes of all workers + border redexes).

4. **No redex is permanently lost.**
   - Local redexes are resolved in Phase 2 (completeness of `reduce_all`, SPEC-03).
   - Pre-existing border redexes (those that existed at the time of partitioning) are detected in Phase 3 via boundary reconnection (R5, R12). This follows from the exhaustive traversal of all borders (ARG-003, Part I, Steps 1-4).
   - Emergent border redexes (via CON-DUP) are detected in the merge of the round where the FreePort inheritance occurs (Phase 3 reconnection detects them on-the-fly), or in the subsequent round when re-partitioning and re-merging reveals the newly formed Active Pair (ARG-003, Part II, Steps 5-9).
   - Redexes generated by border redex resolution are resolved within the same `reduce_all` invocation (R17).

5. **Conclusion.** Since the budget S is finite and each round makes progress, the loop converges. Since no redex is permanently lost, the final net contains no redexes. Since the Normal Form is unique (SPEC-01, T6), the final net is isomorphic to `reduce_all(mu)`. This establishes the Fundamental Property G1 for the grid protocol.

**Condition:** The net MUST be terminating. For non-terminating nets, the grid loop does not converge (just as `reduce_all` does not converge). The `max_rounds` option (R29) serves as a safeguard in that case.

### 4.7 Grid Cycle Diagram

```
   +================+
   | Input Net       |
   +================+
           |
           v
   [0] Initial Normal Form check
       drain_stale_redexes
       queue empty? ------YES-----> NORMAL FORM (0 rounds)
           |
           NO
           |
           v
   +----------------------------------------------------------+
   |       |                                                   |
   |       v                                                   |
   | [1] max_rounds reached? ---YES-----> TIMEOUT              |
   |       |                            (return partial)       |
   |       NO                                                  |
   |       |                                                   |
   |       v                                                   |
   | [2] SPLIT                                                 |
   |     split(net, n, strategy)                               |
   |     -> PartitionPlan { partitions, borders }              |
   |       |                                                   |
   |       v                                                   |
   | [3] LOCAL REDUCTION (per worker, in parallel)             |
   |     For each partition:                                   |
   |       reduce_all(partition.subnet) -> ReductionStats      |
   |       rebuild_free_port_index(partition.subnet)           |
   |       Populate WorkerRoundStats (incl. interactions_by_rule)|
   |       |                                                   |
   |       v                                                   |
   | [4] MERGE (structural)                                    |
   |     merge(plan) -> (merged Net, border_redex_count)       |
   |       |                                                   |
   |       v                                                   |
   | [5] RESOLVE BORDERS                                       |
   |     reduce_all(merged_net) -> ReductionStats              |
   |       |                                                   |
   |       v                                                   |
   | [6] Accumulate metrics (incl. per-rule), rounds++         |
   |       |                                                   |
   |       v                                                   |
   | [7] TERMINATION CHECK (= SPEC-13 CheckTermination)        |
   |     drain_stale_redexes                                   |
   |     queue empty? ------YES-----> NORMAL FORM              |
   |       |                          (converged = true)       |
   |       NO                                                  |
   |       |                                                   |
   +-------+----(next round)-----------------------------------+
```

### 4.8 Special Cases

**Case 1: n == 1 (single worker).**
With a single worker, the split produces one partition with no borders. The round executes `reduce_all` on the entire net. No border redexes exist. The loop converges in a single round. This case SHOULD be optimized to avoid split/merge overhead (R26).

**Case 2: Empty net (0 agents).**
The net has no redexes. The loop terminates immediately at step [1].

**Case 3: All redexes are border redexes.**
If the partitioning separated all Active Pairs, Phase 2 does no work (each partition is already in local Normal Form). Phase 3 detects and resolves all border redexes. If the resolution does not generate new redexes, the loop terminates in one round. If it generates new redexes (via CON-DUP), the loop continues.

**Case 4: Non-terminating net.**
The grid loop does not converge (just as `reduce_all` would not converge). The `max_rounds` option (R29) serves as a safeguard. The TCC operates under the premise of terminating nets (OBJETIVO_TCC.md: "Ideal scenario").

**Case 5: All agents on one side of a border were erased.**
If all boundary ports of a partition were destroyed by erasure, the corresponding borders are discarded silently (R6, R7). This may produce disconnected ports (`DISCONNECTED`) in agents on the other side. These disconnected ports do NOT violate invariant T1 if the corresponding agents were also destroyed (erasure propagates). If an agent survives with a DISCONNECTED port, this indicates a bug in the reduction engine (the port should have been reconnected by some rule). In debug mode, `assert_all_invariants` catches this case.

### 4.9 Interaction Between Merge and Static ID Space Partitioning

Static ID space partitioning (SPEC-04, R16-R19) guarantees that AgentIds across all partitions are mutually disjoint. This radically simplifies the merge:

1. **No collision checking:** When copying agents from partitions to the result net, there is no need to verify whether a slot is already occupied. Each agent's ID is globally unique by construction.

2. **No remapping:** The Haskell prototype executes `remapAllPartitions` before merge (AC-003, AC-004) to avoid ID collisions between agents created by different workers during local reduction. Relativist eliminates this phase entirely. The cost savings are significant: the Haskell prototype's remapping requires scanning all agents, computing a remap table, and updating all references (AC-003, `remapPartitions`).

3. **`next_id` at merge:** The `next_id` of the result net is the maximum of all partition `next_id` values (R8). This ensures agents created in future rounds do not collide with agents from any partition.

This design decision is directly inspired by HVM4's static thread-local ID banks (AC-015, CC-4), adapted from shared-memory to distributed-memory. The tradeoff is upfront allocation of ID space (some ranges may be underutilized), but this is negligible given the u32 space (~4 billion IDs, ~537 million per worker with 8 workers).

---

## 5. Rationale

### 5.1 Centralized Merge Over Alternatives

**Decision:** The coordinator executes the full merge, collecting all partitions and reconstructing the entire net.

**Justification:** DISC-005 v2 (Sections 3 and 3.5) analyzes 4 alternatives:
- **Alternative A (agent migration):** Significantly higher implementation complexity, cascade of migrations with CON-DUP, global ownership state. Assessment: moderate.
- **Alternative B (peer-to-peer):** O(n^2) channels, distributed termination detection, potential deadlock. Assessment: weak for the TCC.
- **Alternative C (half-merge):** Integration complexity for partial results, CON-DUP generates new agents that are difficult to integrate. Assessment: moderate.
- **Alternative D (perfect partitioning):** Impossible in general (Mackie, REF-013, p.222: detecting future interactions is computationally equivalent to computing the entire result).

Centralized merge is the correct choice for this TCC because: (1) simplicity of implementation and verification, (2) self-evident correctness (the coordinator has a global view), (3) the TCC's scope is validating the Fundamental Property, not optimizing communication performance, (4) the Haskell prototype empirically validates the approach (0 failures in ~110 tests, DISC-005 v2, Section 5.3).

### 5.2 reduce_all After Merge Instead of Selective Resolution

**Decision:** After merge, invoke `reduce_all` on the entire net rather than resolving only border redexes selectively.

**Justification:** The Haskell prototype resolves border redexes with `foldl reduce merged borderPairs` (AC-004), which applies rules only to boundary pairs. This is correct but potentially incomplete: if resolving a border redex via CON-DUP creates new redexes that are not border redexes, those new redexes remain pending for the next round.

Relativist adopts a simpler and more complete approach: invoke `reduce_all` on the merged net. This automatically resolves:
- Border redexes detected during merge.
- New redexes generated by border redex resolution (including CON-DUP cascades).
- Any residual redexes that were in the partition queues.

Strong confluence guarantees that the resolution order is irrelevant (SPEC-01, T4; DISC-005 v2, Section 8.2): resolving everything at once is as correct as resolving border redexes first and internal redexes after. The additional cost is minimal: if there are no residual redexes beyond border redexes, `reduce_all` does exactly the same work as selective resolution.

### 5.3 Lazy Reconstruction of FreePort Index

**Decision:** Rebuild the `free_port_index` by scanning the port array after local reduction, rather than maintaining the index incrementally during reduction.

**Justification:** SPEC-04, Section 4.6 presents both approaches and recommends lazy reconstruction as the baseline. The lazy approach is:
- **Simpler:** Does not require modification to the reduction engine (SPEC-03) to notify FreePort changes.
- **Less error-prone:** A single correct scan replaces multiple callbacks.
- **Acceptable cost:** O(A_i * PORTS_PER_SLOT) per partition, dominated by the cost of local reduction.
- **Sufficient for the TCC:** The scan overhead is negligible compared to the cost of serialization/transfer.

If benchmarks reveal a bottleneck, migration to active notification is possible without altering the merge interface.

### 5.4 Round Limit as a Safeguard

**Decision:** Include `max_rounds` as an optional parameter of the grid loop.

**Justification:** SPEC-01 (D6) identifies the need for a practical safeguard against non-terminating nets or bugs. For terminating nets (scope of the TCC), the grid loop provably converges (Section 4.6). But in testing and development, malformed nets or reduction engine bugs can cause infinite loops. The `max_rounds` is a practical safeguard, not a theoretical necessity.

### 5.5 Merge as the Inverse of Split

**Decision:** The merge is designed and validated as the logical inverse of split.

**Justification:** ARG-002 (Part I, Steps 1-4) formally derives that `merge(split(net)) ~ net` under conditions C1-C3. This identity is the basis for invariant D1 (SPEC-01). The merge algorithm in Section 4.2 preserves this identity by:
- Copying all agents without loss or duplication (Step 2, using disjoint ID space from SPEC-04 R16-R19, corresponding to C1).
- Copying all internal wires unchanged (Step 2, corresponding to C2 for internal wires).
- Restoring all border wires via the border map (Step 3, corresponding to C2 for border wires and C3 for FreePort bijectivity).

For reduced sub-nets, the identity becomes: `merge(reduce_local(split(net)))` is a net where all local redexes have been resolved but border redexes remain. The `reduce_all` after merge (R15) completes the job. The split/merge cycle with interleaved reduction is the engine that drives the net toward Normal Form.

---

## 6. Haskell Prototype Reference

### 6.1 Function `mergePartitions` (AC-002, lines 104-127)

The prototype unites agents via `Map.unions` (left-biased), filters boundary wires, and reconnects borders via `freePortNeighbor` (linear scan: O(W*B) total).

**What Relativist changes:**
1. Uses `free_port_index` for O(1) lookup per border, eliminating the linear scan (AC-002, Limitation L3).
2. Does not need `remapAllPartitions` because IDs are pre-allocated (SPEC-04, R16-R19). This eliminates an entire phase from the Haskell prototype's pipeline (AC-003).
3. The redex queue is populated incrementally by `connect` (SPEC-02, R13), instead of a global `findRedexes`.
4. Invokes `reduce_all` after merge instead of resolving only border redexes selectively. This is strictly more complete.

### 6.2 Function `findBorderRedexes` (AC-002, lines 137-152)

The prototype traverses all borders and checks whether both endpoints are principal ports. Returns a list of tuples `(agentId1, agentId2, worker1, worker2)`.

**What Relativist changes:**
Eliminates this function entirely. Border redex detection is on-the-fly: `Net::connect(port_a, port_b)` during merge automatically inserts into the redex queue when both ports are principal (SPEC-02, R13). The border redex count for metrics is tracked during merge via `is_principal_pair` (Section 4.2).

### 6.3 Function `remapAllPartitions` (AC-003)

The prototype executes a post-reduction remapping of all new AgentIds to avoid collisions between workers. The algorithm computes `globalMaxId = nextAgentId(currentNet)`, then for each partition identifies "new" agents and remaps their IDs to values above `globalMaxId`.

**What Relativist changes:**
Eliminates this function entirely. Static ID space partitioning (SPEC-04, R16-R19) assigns each worker a disjoint range at split time, so agents created during local reduction are guaranteed unique without post-hoc remapping. This removes the O(A_new) remapping cost per round and simplifies the merge pipeline.

### 6.4 Grid Loop `go` (AC-004, lines ~105-125)

The prototype implements the loop as a recursive function `go(currentNet, metrics)`:
1. `findRedexes(currentNet) == []` -> terminate.
2. `coordinatorDistribute(currentNet, effectiveWorkers)` -> plan.
3. `map workerReduce (planPartitions plan)` -> reduced partitions.
4. `remapAllPartitions(...)` -> remapped partitions.
5. `mergePartitions(plan { planPartitions = remappedParts })` -> merged net.
6. `findBorderRedexes(updatedPlan)` -> border redexes.
7. `foldl reduce merged borderPairs` -> net after borders.
8. Accumulate metrics, `go(afterBorder, metrics')`.

**What Relativist changes:**
1. Eliminates step 4 (`remapAllPartitions`): replaced by static ID space partitioning (SPEC-04).
2. Consolidates steps 5-7 into merge + `reduce_all` (simpler and more complete).
3. Adds `max_rounds` as a safeguard (step 2 did not exist in the prototype).
4. Adds lazy reconstruction of the `free_port_index` before returning the reduced partition.
5. Collects metrics with per-round granularity (AC-004 collected similar metrics, but Relativist systematizes the collection).

### 6.5 Metrics (AC-004, GridMetrics)

The prototype collects: rounds, total interactions, timing per phase (partition, compute, remap, merge, border), network timing (send, recv), bytes transferred, worker stats. The structure uses `++` to accumulate worker stats (O(n) per round).

**What Relativist changes:**
1. Uses pre-allocated `Vec` instead of O(n) accumulation.
2. Adds `border_redexes_per_round` to quantify partitioning quality.
3. Adds `agents_per_round` to track net size across rounds.
4. Adds `converged: bool` to indicate whether the grid reached Normal Form or timed out.
5. Removes `remap_time` (no remapping phase in Relativist).

---

## 7. Open Questions

1. **Optimization: skip re-partitioning if topology changed little.** If border redex resolution generates few new redexes, re-partitioning the entire net is wasteful. A heuristic "skip re-partition if border_redexes < threshold" could save the partitioning cost in subsequent rounds. The decision to implement this optimization MUST be informed by benchmarks (SPEC-09) and is left to the ENGINEER.

2. **Parallelization of border redex resolution at the coordinator.** DISC-005 v2 (Question 5) observes that border redexes between disjoint pairs of agents commute by strong confluence and could be resolved in parallel. However, Relativist v1 resolves them sequentially via `reduce_all`. Intra-coordinator parallelization is a future optimization.

3. ~~**Choice between drain_stale_redexes and full scan for Normal Form detection.**~~ **RESOLVED (v3).** Promoted to R41 as a SHOULD requirement: full scan in debug mode by default, configurable in release mode. See Section 4.4.

4. ~~**(cross-spec) Per-rule interaction tracking in SPEC-03.**~~ **RESOLVED (2026-04-06).** SPEC-03 now defines `SpecificRule` enum (6 variants: ConCon, ConDup, ConEra, DupDup, DupEra, EraEra) in Section 4.3.1, adds `interactions_by_rule: [u64; 6]` to `ReductionStats`, and returns `StepResult::Reduced(Rule, SpecificRule)` from `reduce_step`. The `reduction_stats_to_by_rule` helper is no longer needed; `ReductionStats.interactions_by_rule` can be used directly.
