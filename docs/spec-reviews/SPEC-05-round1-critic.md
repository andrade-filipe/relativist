# SPEC-05 -- Round 1: Spec Critic Review

**Date:** 2026-04-05
**Reviewer:** Spec Critic (adversarial)
**Target:** SPEC-05-merge.md (status: Revised v2)
**Predecessors consulted:** SPEC-00, SPEC-01, SPEC-02, SPEC-04, SPEC-06, SPEC-11, SPEC-13

---

## Overall Assessment

SPEC-05 is a well-structured spec that covers partition merge, border redex detection and resolution, the grid loop algorithm, and the metrics framework. It successfully ties together the partitioning (SPEC-04), reduction engine (SPEC-03), and the formal invariant framework (SPEC-01 D1-D6, G1). The pseudocode is detailed, the rationale section is thorough, and the informal completeness proof is rigorous. However, there are several consistency issues with the revised SPEC-13 (Revised v2) and SPEC-11 (Revised v2), which have superseded or extended definitions that SPEC-05 still uses in their original form. There are also gaps in the termination condition specification, incomplete handling of edge cases in the merge algorithm, and testability concerns around the `drain_stale_redexes` approach.

**Verdict:** NEEDS REVISION

---

## Issues

### SC-001: WorkerRoundStats definition is stale -- missing SPEC-11 extensions
**Severity:** CRITICAL
**Axis:** Consistency
**Section:** 4.1 (Types), 3.7 (R37)
**Requirement:** R37
**Problem:** SPEC-05 R37 defines `WorkerRoundStats` with 3 fields:
- `worker_id: WorkerId`
- `agents_before: usize`
- `agents_after: usize`
- `local_redexes: usize`

SPEC-11 Section 4.4 and OQ-1 explicitly extend `WorkerRoundStats` with two additional fields:
- `reduce_duration_secs: f64` -- wall-clock time for reduce_all in this worker
- `interactions_by_rule: [u64; 6]` -- per-rule interaction breakdown (CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA)

SPEC-11 OQ-1 states: "This extension SHOULD be reflected in SPEC-05 R37 and SPEC-06 R12 during their next revision cycle. Until then, the extended definition in this spec (Section 4.4) is normative for observability purposes."

Since SPEC-05 is the canonical home of `WorkerRoundStats` (it is defined in SPEC-05's types section and referenced by SPEC-06 R12), the canonical definition is now inconsistent with the normative extensions. SPEC-06's `PartitionResult` message carries `stats: WorkerRoundStats` -- the serialized struct must match between coordinator and worker. If one side uses the SPEC-05 definition and the other uses the SPEC-11 extension, bincode deserialization will fail silently or panic.

Additionally, the `interactions_by_rule` field is essential for the `interactions_by_rule_total` Prometheus metric (SPEC-11 R12, `CoordinatorMetrics`). Without this field, the coordinator cannot populate the per-rule Family metric.

**Impact if unresolved:** The implementer must choose between the SPEC-05 definition (3 fields) and the SPEC-11 definition (5 fields). Since both are normative in their respective scopes, any implementation will violate one spec. More concretely, the coordinator's Prometheus metrics (SPEC-11 R12) require per-rule interaction data that is not present in the SPEC-05 canonical struct.
**Suggested resolution:** Update SPEC-05 R37 to include the two additional fields from SPEC-11:
```rust
pub struct WorkerRoundStats {
    pub worker_id: WorkerId,
    pub agents_before: usize,
    pub agents_after: usize,
    pub local_redexes: usize,
    /// Wall-clock duration of reduce_all for this worker (seconds).
    pub reduce_duration_secs: f64,
    /// Per-rule interaction counts: [CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA].
    pub interactions_by_rule: [u64; 6],
}
```
Add `serde::Serialize, serde::Deserialize` derives (required by SPEC-06 R12). Close SPEC-11 OQ-1.

---

### SC-002: Termination condition in run_grid uses only redex_queue.is_empty(), inconsistent with SPEC-13 is_normal_form
**Severity:** CRITICAL
**Axis:** Consistency | Invariant Preservation
**Section:** 4.5 (Grid Loop Algorithm), 3.5 (R27)
**Requirement:** R27
**Problem:** SPEC-05's `run_grid` pseudocode (Section 4.5) checks termination as:
```
if current_net.redex_queue.is_empty():
    drain_stale_redexes(&mut current_net)
    if current_net.redex_queue.is_empty():
        metrics.converged = true
        break
```

This check occurs at the **beginning** of the loop, before any work in the current round. It checks the merged net from the previous round.

SPEC-13 Revised v2, on the other hand, introduces a `CheckTermination` state in the coordinator FSM (R19) and uses `is_normal_form: bool` in the `MergeComplete` event (R20):
```rust
MergeComplete { net: Net, is_normal_form: bool },
```

The transition table (R21) specifies:
```
Merging | MergeComplete(net, _) | CheckTermination | LogTransition
CheckTermination | [is_normal_form == true] | Done | WriteOutput, ShutdownAll, LogTransition
CheckTermination | [is_normal_form == false] | Partitioning | InvokeSplit, LogTransition
```

There are two discrepancies:
1. **When the check happens:** SPEC-05 checks at the top of the loop (before split). SPEC-13 checks after merge+reduce_all (via CheckTermination). Functionally these are equivalent for the second-and-subsequent rounds, but SPEC-13's approach is cleaner because it checks immediately after reduce_all rather than at the start of the next iteration.
2. **How the check is performed:** SPEC-05 uses `redex_queue.is_empty()` + `drain_stale_redexes`. SPEC-13 uses `is_normal_form: bool` computed during the `InvokeMergeAndReduce` action. The `is_normal_form` field abstracts the check and avoids exposing the stale-redex-draining implementation detail in the FSM.

The SPEC-13 `InvokeMergeAndReduce` action note (R21) says: "The `InvokeMergeAndReduce` action performs both steps [merge + reduce_all] as a single unit and reports `is_normal_form` based on whether the redex queue is empty after `reduce_all`." This implies that the merge action already runs reduce_all and drains stale redexes internally. SPEC-05's pseudocode does this differently -- it runs reduce_all as a separate step after merge, then checks at the top of the next iteration.

**Impact if unresolved:** The implementer gets two different termination architectures. SPEC-05 says check at loop top; SPEC-13 says check after merge via FSM state. The grid loop pseudocode in SPEC-05 is the reference implementation for `run_grid` (the core layer function), while SPEC-13's FSM is the reference for the coordinator (infrastructure layer). These need to be aligned.
**Suggested resolution:** Update SPEC-05 Section 4.5 pseudocode to check termination after merge+reduce_all (not at loop top), returning `is_normal_form` from the merge+reduce_all sequence. This aligns with SPEC-13's `CheckTermination` state. Alternatively, add a note that the pseudocode's top-of-loop check is functionally equivalent to SPEC-13's post-merge check, since the net is unchanged between the end of one round and the beginning of the next.

---

### SC-003: merge() return type inconsistency with SPEC-13 InvokeMergeAndReduce
**Severity:** HIGH
**Axis:** Consistency
**Section:** 4.2 (Merge Algorithm), 3.1 (R1)
**Requirement:** R1
**Problem:** SPEC-05 R1 specifies: "`merge` function MUST accept as input a `Vec<Partition>` and a border map `HashMap<u32, (PortRef, PortRef)>`, and return a single recombined `Net`."

The Section 4.2 pseudocode returns `(Net, u32)` -- the net plus the border redex count.

SPEC-13 R20 defines `InvokeMergeAndReduce(Vec<Partition>)` as an action, and the completion event is `MergeComplete { net: Net, is_normal_form: bool }`. This action encompasses both merge and reduce_all.

The inconsistency is that SPEC-05 defines merge and reduce_all as two separate operations:
1. `merge(partitions, borders) -> (Net, u32)` -- merge only
2. `reduce_all(merged_net) -> interactions` -- border resolution

But SPEC-13 combines them into a single `InvokeMergeAndReduce` action. This raises the question: should the merge function accept the border map as a separate parameter (as SPEC-05 R1 says), or should it be part of the partitions (the `PartitionPlan` already contains `borders`)?

Additionally, SPEC-05's `merge()` signature takes `Vec<Partition>` + `borders`, but the `PartitionPlan` struct (SPEC-04 Section 4.1) already contains both `partitions: Vec<Partition>` and `borders: HashMap<u32, (PortRef, PortRef)>`. It would be more natural for `merge()` to accept a `PartitionPlan` or `&PartitionPlan`.

**Impact if unresolved:** Minor API design friction. The implementer must decide whether merge takes separate arguments or a PartitionPlan. More importantly, the relationship between SPEC-05's two-step (merge then reduce_all) and SPEC-13's one-step (InvokeMergeAndReduce) needs clarification.
**Suggested resolution:** (a) Clarify in SPEC-05 that `merge()` is the pure merge step and `reduce_all()` is called separately. SPEC-13's `InvokeMergeAndReduce` is a coordinator-level convenience that calls both. (b) Consider updating R1 to accept `&PartitionPlan` instead of separate arguments, since the plan already bundles partitions and borders. (c) Add a note that the `is_normal_form` check (used by SPEC-13's MergeComplete) is computed after `reduce_all`, not by `merge()` itself.

---

### SC-004: GridMetrics lacks SPEC-11's per-rule interaction tracking
**Severity:** HIGH
**Axis:** Consistency | Completeness
**Section:** 3.7 (R35), 4.1 (Types)
**Requirement:** R35
**Problem:** SPEC-05 R35 defines `GridMetrics` with `total_interactions: u64` and `local_interactions_per_round: Vec<u64>`, but these are aggregate counts. SPEC-11 R12 requires `interactions_by_rule_total` as a Prometheus Counter Family with 6 rule labels (CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA).

For the coordinator to populate `interactions_by_rule_total`, it needs per-rule interaction data from workers (via `WorkerRoundStats.interactions_by_rule`, see SC-001) and from its own border resolution. But `GridMetrics` has no field to accumulate per-rule totals. The coordinator must either:
- Aggregate from `WorkerRoundStats.interactions_by_rule` per round (but GridMetrics has no per-rule field)
- Track per-rule counts directly in Prometheus counters (bypassing GridMetrics)

If per-rule counts bypass GridMetrics, the metrics structure becomes incomplete for offline analysis (SPEC-09 benchmarks).

**Impact if unresolved:** The benchmark suite (SPEC-09) and Prometheus metrics (SPEC-11) have different views of interaction data. GridMetrics only has aggregate counts; Prometheus needs per-rule breakdowns. The implementer must invent a solution.
**Suggested resolution:** Add a `total_interactions_by_rule: [u64; 6]` field to `GridMetrics`. Each round, the coordinator accumulates worker-reported per-rule counts plus its own border-resolution per-rule counts. This ensures both the benchmark CSV output and Prometheus metrics can report per-rule breakdowns.

---

### SC-005: Merge algorithm discards partition redex queues -- potential correctness concern
**Severity:** HIGH
**Axis:** Completeness | Invariant Preservation
**Section:** 4.2 (Merge Algorithm, Step 2 note)
**Requirement:** R9
**Problem:** The merge algorithm (Section 4.2, Step 2 note) says: "residual redexes from partition queues are discarded during merge. Step 3 and the subsequent `reduce_all` will detect redexes via `connect`."

This means that if a partition had residual local redexes in its queue after `reduce_all` (which should not happen if `reduce_all` fully reduces the partition), those redexes are silently dropped. R18 says: "The `reduce_all` after merge MUST process both border redexes and any residual local redexes that may exist in the queue."

The contradiction: R18 assumes residual local redexes may exist in the merged net's queue, but the merge algorithm discards all partition queues. The only redexes in the merged net's queue are those detected by `connect()` during Step 3 (border reconnection). If there were residual local redexes, they would be lost.

Under what conditions could residual local redexes exist? If `reduce_all` runs to completion on each partition, the partition's redex queue should be empty (or contain only stale entries). But if `reduce_n` (budget-limited) is used instead of `reduce_all`, residual valid redexes could exist. SPEC-05 R24's grid loop pseudocode uses `reduce_all`, so this should not occur in normal operation. However, R29 allows `max_rounds` timeout, and future versions might use budget-limited reduction.

Additionally, the merge's Step 2 copies internal connections but does NOT re-scan for redexes among internal connections. It relies entirely on Step 3's `connect()` for redex detection. But `connect()` is only called for border wires (Step 3). Internal connections are copied directly via `set_port()` (Step 2), which does NOT detect redexes. Therefore, if there were any internal redexes not in the partition queue (a theoretical impossibility if `connect()` in SPEC-02 R13 always fires, but a practical risk if an agent's port was set directly), they would be silently lost.

**Impact if unresolved:** For normal operation with `reduce_all`, this is likely safe. But the spec's own R18 mentions "residual local redexes" as a possibility, and the merge algorithm silently discards them. This is an internal inconsistency within SPEC-05.
**Suggested resolution:** Either (a) update the merge algorithm to carry over valid redexes from partition queues (filtering out stale ones), or (b) update R18 to clarify that after `reduce_all`, partition queues are guaranteed empty (only stale entries), so discarding them is safe. Add a debug assertion: after merge Step 2, if any partition had non-stale redexes in its queue, it is a bug. Option (b) is simpler and correct for v1.

---

### SC-006: run_grid signature uses &dyn PartitionStrategy but SPEC-13 uses usize for num_workers
**Severity:** MEDIUM
**Axis:** Consistency
**Section:** 4.5 (Grid Loop Algorithm), 3.5 (R25)
**Requirement:** R25
**Problem:** SPEC-05 R25 specifies `run_grid` accepting `num_workers: u32`. The pseudocode in Section 4.5 shows:
```
fn run_grid(net: Net, config: &GridConfig, strategy: &dyn PartitionStrategy) -> (Net, GridMetrics)
```
Where `GridConfig.num_workers` is `u32`.

SPEC-13 Section 4.5 (informative) defines:
```
run_grid_local(net: Net, num_workers: usize) -> Result<(Net, GridMetrics), RelativistError>
```
With `usize` instead of `u32`, missing the `strategy` parameter, and wrapping in `Result`.

The SPEC-13 round 1 review (SC-015) already identified this discrepancy. The SPEC-13 Revised v2 data flow (R40) uses `split(net, k)` with `k` undefined as a type.

While SPEC-13 Revised v2 added `run_grid_local` as `R41a` (using `local` subcommand for in-memory grid), the type discrepancy remains unresolved.

**Impact if unresolved:** Type mismatch at the API boundary between core layer (`run_grid` from SPEC-05) and infrastructure layer (coordinator from SPEC-13). Minor but creates implementation friction.
**Suggested resolution:** Align on `u32` for `num_workers` everywhere (SPEC-05 is the canonical source). SPEC-13's `run_grid_local` should delegate to SPEC-05's `run_grid` with the same signature.

---

### SC-007: No specification for handling FreePort (Lafont) during merge
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 4.2 (Merge Algorithm), 3.1 (R3)
**Requirement:** R3
**Problem:** SPEC-05's merge algorithm (Section 4.2, Step 2) copies connections from partitions to the result net. When copying, it checks:
```
match target:
    AgentPort(_, _): // Internal connection: copy directly
    FreePort(_):     // Boundary connection: skip (will be restored in Step 3)
```

But SPEC-04 R15 distinguishes between FreePort (Lafont) -- pre-existing interface ports from the original net -- and FreePort (Boundary) -- synthetic markers created by split. Both are represented as `FreePort(u32)` at the type level (SPEC-00 Sections 6.1, 6.2; SPEC-02 R4 note).

During merge Step 2, the algorithm treats ALL `FreePort(_)` targets as boundary connections to be skipped. But FreePort (Lafont) ports should be preserved -- they represent the interface of the net and should appear in the merged result. If a port is connected to `FreePort(5)` where 5 is a Lafont free port (not a boundary marker), the merge algorithm would mark it as `DISCONNECTED` and then try to restore it in Step 3 -- but Step 3 only processes borders from the border map. Since Lafont free ports are not in the border map, they would be silently lost.

SPEC-04 R15 says: "Pre-existing Lafont FreePorts from the original net MUST NOT be treated as border wires." The split operation preserves them correctly, but the merge algorithm does not distinguish between the two kinds of FreePort.

**Impact if unresolved:** If the input net has interface ports (FreePort Lafont), the merge would lose them, producing a net with DISCONNECTED ports where Lafont free ports should be. This violates D1 (split/merge identity). For nets without Lafont free ports (common in the benchmark suite), this is a non-issue.
**Suggested resolution:** In merge Step 2, when encountering a `FreePort(id)` target, check whether `id` is in the border map. If yes, it is a boundary FreePort -- skip and handle in Step 3. If no, it is a Lafont FreePort -- copy it directly to the result net. Add a note referencing SPEC-04 R15 and SPEC-00 Sections 6.1/6.2.

---

### SC-008: compute_time_per_round conflates reduction time and free_port_index reconstruction time
**Severity:** MEDIUM
**Axis:** Completeness | Testability
**Section:** 4.5 (Grid Loop Algorithm), 3.7 (R35)
**Requirement:** R35
**Problem:** In the grid loop pseudocode (Section 4.5), the `compute_time_per_round` timer wraps both `reduce_all` and `rebuild_free_port_index`:
```
let t_compute = Instant::now()
...
    let interactions = reduce_all(&mut partition.subnet)
    partition.free_port_index = rebuild_free_port_index(&partition.subnet)
...
metrics.compute_time_per_round.push(t_compute.elapsed())
```

The `rebuild_free_port_index` operation (Section 4.3) has complexity O(A_i * PORTS_PER_SLOT). For large nets, this scan may be non-trivial. Conflating it with `reduce_all` time makes it impossible to attribute overhead correctly in benchmarks.

SPEC-09 benchmarks aim to decompose the grid cycle into its constituent costs (DISC-006 v2 Section 1.1). If index reconstruction cost is hidden inside `compute_time`, the overhead analysis will be inaccurate.

The Haskell prototype's `GridMetrics` (AC-004) separately tracked `remap_time` as a distinct phase. Relativist eliminated remapping but introduced index reconstruction, which serves a similar purpose. It deserves its own timing bucket.

**Impact if unresolved:** Benchmark overhead decomposition (SPEC-09) will incorrectly attribute index reconstruction cost to reduction cost. The error may be small for most nets but could be significant for nets with many border wires.
**Suggested resolution:** Add `index_rebuild_time_per_round: Vec<Duration>` to `GridMetrics`. Time `rebuild_free_port_index` separately from `reduce_all` in the grid loop. This enables accurate overhead decomposition.

---

### SC-009: merge_time_per_round conflates merge and border resolution
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 4.5 (Grid Loop Algorithm), 3.7 (R35)
**Requirement:** R35
**Problem:** In the grid loop pseudocode (Section 4.5):
```
let t_merge = Instant::now()
let (merged_net, border_redex_count) = merge(plan.partitions, &plan.borders)
let border_interactions = reduce_all(&mut merged_net)
metrics.merge_time_per_round.push(t_merge.elapsed())
```

The `merge_time_per_round` includes both the structural merge (O(A_total + B)) and the `reduce_all` for border redexes (O(S_border)). For nets with CON-DUP cascades at borders, `S_border` can significantly exceed `B` (as noted in R40), making the border resolution dominate the merge time.

Conflating these two operations makes it impossible to distinguish:
- **Merge overhead:** The structural cost of reconstructing the net (a fixed cost of the protocol).
- **Border computation:** The actual computation performed at borders (problem-dependent, varies with topology).

These are fundamentally different: merge overhead is "wasted work" (protocol tax), while border computation is "useful work" that would need to happen anyway.

**Impact if unresolved:** Benchmark analysis cannot distinguish protocol overhead from useful computation in the merge phase. DISC-006 v2's overhead formula treats merge as overhead, but border reduction is not overhead -- it is deferred computation.
**Suggested resolution:** Split `merge_time_per_round` into two fields:
- `merge_structural_time_per_round: Vec<Duration>` -- time for the merge operation itself
- `border_reduce_time_per_round: Vec<Duration>` -- time for `reduce_all` after merge

Alternatively, keep `merge_time_per_round` as-is but add a separate `border_reduce_time_per_round`. This aligns with the separate `border_interactions_per_round` that already tracks border work in interaction count.

---

### SC-010: drain_stale_redexes approach may mask lost-redex bugs
**Severity:** MEDIUM
**Axis:** Testability
**Section:** 4.4 (Stale Redex Draining)
**Requirement:** (informative section, but referenced by R27 termination logic)
**Problem:** The `drain_stale_redexes` function (Section 4.4) filters the redex queue to contain only valid entries. The grid loop uses it to confirm Normal Form before terminating. Open Question 3 acknowledges: "If a bug causes a 'lost redex' (a redex not inserted into the queue), only a full scan of the port array would detect it."

The spec leaves the choice between drain-only and full-scan to the ENGINEER. However, this decision has correctness implications: if `connect()` has a bug that fails to insert a redex into the queue, the grid loop would declare Normal Form prematurely, violating G1. The drain approach is an optimistic check that assumes `connect()` is correct.

For a research prototype whose primary goal is validating the Fundamental Property (G1), correctness assurance should take priority over performance. A full scan after merge+reduce_all is O(A * PORTS_PER_SLOT), which is dominated by the merge cost (also O(A_total)). The marginal cost is negligible.

**Impact if unresolved:** A subtle bug in `connect()` could cause G1 violations that are extremely hard to diagnose, because the system would silently declare Normal Form while redexes remain.
**Suggested resolution:** Upgrade OQ-3 to a SHOULD requirement: "After the final `reduce_all` in each round, and before declaring Normal Form, the grid loop SHOULD perform a full scan of the port array to detect any redexes not present in the queue. This scan SHOULD be enabled by default in debug mode and MAY be disabled in release mode via a configuration option."

---

### SC-011: No explicit specification for initial round's normal form check
**Severity:** LOW
**Axis:** Completeness
**Section:** 4.5 (Grid Loop Algorithm)
**Requirement:** R27
**Problem:** The grid loop pseudocode checks for Normal Form at the top of the loop:
```
loop:
    if current_net.redex_queue.is_empty():
        drain_stale_redexes(&mut current_net)
        if current_net.redex_queue.is_empty():
            metrics.converged = true
            break
```

On the first iteration, `current_net` is the input net. If the input net is already in Normal Form, the loop terminates immediately without any rounds. This is correct behavior (Case 2 in Section 4.8 covers the empty net case).

However, the spec does not explicitly state what happens when the input net is non-empty but already in Normal Form (no redexes, but has agents). The `drain_stale_redexes` function would process an empty queue and confirm Normal Form. The `metrics.rounds` would be 0 and `metrics.converged` would be true.

SPEC-13's FSM, by contrast, always starts from Init -> WaitingForWorkers -> Partitioning, which means it would attempt to split even a net in Normal Form. The first round would split, dispatch to workers (who find no redexes), collect, merge, and then CheckTermination would find is_normal_form == true.

**Impact if unresolved:** Minor discrepancy: SPEC-05 says 0 rounds for an already-reduced net; SPEC-13's FSM would execute 1 round. The results are the same (the net is unchanged), but metrics would differ: 0 rounds vs 1 round.
**Suggested resolution:** Add a note in SPEC-05 Section 4.5: "The initial Normal Form check (before the first round) avoids unnecessary split/merge for nets already in Normal Form. In the distributed context (SPEC-13), the coordinator MAY perform this check before entering the BSP loop, or it MAY execute one round and detect Normal Form during CheckTermination."

---

### SC-012: Border map is passed by reference but partitions are consumed by merge
**Severity:** LOW
**Axis:** Completeness
**Section:** 4.2 (Merge Algorithm)
**Requirement:** R1
**Problem:** The merge function signature in Section 4.2 is:
```
fn merge(partitions: Vec<Partition>, borders: &HashMap<u32, (PortRef, PortRef)>) -> (Net, u32)
```

The `partitions` are consumed (moved) by the function, while `borders` is borrowed. This is reasonable from a Rust ownership perspective (partitions contain the agents/ports that are moved into the result net). However, the border map is part of the `PartitionPlan` (SPEC-04 Section 4.1), and the grid loop pseudocode (Section 4.5) passes `plan.partitions` and `&plan.borders` separately, which requires partial moves from the plan struct.

In Rust, you cannot move `plan.partitions` out of a struct while still borrowing `plan.borders` in the same expression without restructuring the plan. The pseudocode elides this ownership issue.

**Impact if unresolved:** Minor implementation friction. The implementer will need to destructure `PartitionPlan` before calling `merge`, or change the API to accept `PartitionPlan` by value.
**Suggested resolution:** Either (a) change merge to accept `PartitionPlan` by value and decompose internally, or (b) add a note that the implementer should destructure the plan: `let PartitionPlan { partitions, borders } = plan; merge(partitions, &borders)`.

---

### SC-013: R19 (border_interactions_per_round) is SHOULD, but the grid loop pseudocode always collects it
**Severity:** LOW
**Axis:** Consistency
**Section:** 3.3 (R19), 4.5 (Grid Loop Algorithm)
**Requirement:** R19
**Problem:** R19 says: "The number of interactions performed during border redex resolution SHOULD be recorded separately in `GridMetrics.border_interactions_per_round`." The RFC-2119 keyword is SHOULD (recommended but not required).

However, `GridMetrics` (R35) lists `border_interactions_per_round: Vec<u64>` as a MUST field. And the grid loop pseudocode (Section 4.5) unconditionally populates it:
```
let border_interactions = reduce_all(&mut merged_net)
metrics.border_interactions_per_round.push(border_interactions as u64)
```

If R19 is SHOULD, the field should also be SHOULD in R35. If R35 is MUST, R19 should be upgraded to MUST for consistency.

**Impact if unresolved:** Minor RFC-2119 confusion. No practical issue since the pseudocode always collects this metric.
**Suggested resolution:** Upgrade R19 from SHOULD to MUST, since R35 already lists the field as MUST and the pseudocode always populates it.

---

### SC-014: No specification for how reduce_all reports per-rule interaction counts
**Severity:** LOW
**Axis:** Completeness
**Section:** 3.7 (R35, R37), 3.3 (R19)
**Requirement:** R35, R37
**Problem:** SPEC-11 extends `WorkerRoundStats` with `interactions_by_rule: [u64; 6]` (SC-001). The worker populates this from its local `reduce_all` invocation. But `reduce_all` (SPEC-03) currently returns only the total interaction count. For per-rule breakdown, `reduce_all` (or the reduction engine) must track counts per rule type.

SPEC-05 does not mention per-rule tracking in its grid loop pseudocode. The grid loop calls:
```
let interactions = reduce_all(&mut partition.subnet)
```

And constructs `WorkerRoundStats` with only `local_redexes: usize` (the queue length before reduction, not even the interaction count).

Furthermore, the border resolution at the coordinator also calls `reduce_all`, and the per-rule breakdown of border interactions would be needed for the `interactions_by_rule_total` Prometheus metric. But `GridMetrics` has no per-rule field (see SC-004).

This is a cross-spec gap: SPEC-03 (reduction engine) defines what `reduce_all` returns; SPEC-05 (grid cycle) uses it; SPEC-11 (observability) needs per-rule data. The spec chain is incomplete.

**Impact if unresolved:** The implementer must modify SPEC-03's `reduce_all` to return per-rule counts, update SPEC-05's grid loop to pass them through, and add per-rule fields to GridMetrics. Without specification, this will be ad-hoc.
**Suggested resolution:** (a) In SPEC-05, update the grid loop pseudocode to show how per-rule counts flow from `reduce_all` to `WorkerRoundStats` and from border resolution to `GridMetrics`. (b) Add a note that `reduce_all` SHOULD return a `ReductionStats` struct (defined in SPEC-03) that includes both total count and per-rule breakdown. (c) Reference SPEC-03's `ReductionStats` in the grid loop.

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 2 |
| HIGH | 3 |
| MEDIUM | 4 |
| LOW | 5 |

## Mandatory (must fix before implementation)

- **SC-001:** WorkerRoundStats missing SPEC-11 extensions (reduce_duration_secs, interactions_by_rule) -- update R37
- **SC-002:** Termination check architecture differs between SPEC-05 (top-of-loop) and SPEC-13 (post-merge FSM state) -- align or document equivalence
- **SC-003:** merge() API inconsistency with SPEC-13's InvokeMergeAndReduce -- clarify relationship
- **SC-004:** GridMetrics lacks per-rule interaction tracking needed by SPEC-11 Prometheus metrics
- **SC-005:** Merge discards partition redex queues but R18 mentions "residual local redexes" -- resolve internal inconsistency

## Recommended (should fix)

- **SC-006:** run_grid num_workers type mismatch (u32 vs usize) with SPEC-13
- **SC-007:** Merge algorithm does not distinguish FreePort (Lafont) from FreePort (Boundary) -- risks losing interface ports
- **SC-008:** compute_time conflates reduce_all and index rebuild -- add separate timing
- **SC-009:** merge_time conflates structural merge and border resolution -- add separate timing
- **SC-010:** drain_stale_redexes should be upgraded with full-scan verification for correctness assurance

---

## Checklist

### Consistency
- [x] Types match predecessor specs (Symbol, AgentId, PortRef, Net, Agent)
- [x] PortRef encoding consistent with SPEC-00/SPEC-02
- [x] WorkerId type consistent with SPEC-04 (u32)
- [ ] **FAIL:** WorkerRoundStats definition stale -- missing SPEC-11 extensions (SC-001)
- [ ] **FAIL:** Termination condition architecture differs from SPEC-13 FSM (SC-002)
- [ ] **FAIL:** merge() return type and relationship to SPEC-13 InvokeMergeAndReduce unclear (SC-003)
- [ ] **FAIL:** GridMetrics lacks per-rule interaction fields needed by SPEC-11 (SC-004)
- [x] Border map structure consistent with SPEC-04 PartitionPlan
- [x] GridConfig.num_workers is u32, consistent with SPEC-04 WorkerId
- [ ] **PARTIAL:** run_grid_local (SPEC-13) uses usize for num_workers (SC-006)
- [x] R19 border_interactions field in GridMetrics consistent with R35 MUST list
- [x] Partition struct usage consistent with SPEC-04 definition

### Testability
- [x] R1 (merge function): testable via round-trip with split
- [x] R2 (no AgentId collisions): testable via assertion after merge
- [x] R5 (boundary reconnection): testable by checking all borders resolved
- [x] R10 (split/merge identity): directly testable property-based test
- [x] R11 (debug assertions): testable by running in debug mode
- [x] R12 (automatic border detection): testable by checking redex queue after merge
- [x] R14 (O(B) detection): testable via timing benchmarks
- [x] R15 (reduce_all after merge): testable by checking no redexes remain
- [x] R24 (run_grid): testable end-to-end
- [x] R27 (termination): testable for known terminating nets
- [x] R30 (convergence for terminating nets): testable via step budget verification
- [ ] **PARTIAL:** R37 (WorkerRoundStats): testable but definition is stale (SC-001)
- [ ] **PARTIAL:** Normal Form detection relies on drain_stale_redexes which may miss lost redexes (SC-010)

### Completeness
- [x] Merge algorithm fully specified with pre/post-conditions
- [x] Border redex detection mechanism specified (on-the-fly via connect)
- [x] Border redex resolution mechanism specified (reduce_all)
- [x] Grid loop algorithm fully specified with pseudocode
- [x] FreePort index reconstruction specified with complexity
- [x] Special cases enumerated (n=1, empty net, all-border, non-terminating, full erasure)
- [x] Informal completeness proof provided (Section 4.6)
- [ ] **FAIL:** Merge algorithm does not handle FreePort (Lafont) correctly (SC-007)
- [ ] **FAIL:** compute_time conflates two operations (SC-008)
- [ ] **FAIL:** merge_time conflates two operations (SC-009)
- [ ] **PARTIAL:** drain_stale_redexes vs full-scan left as open question (SC-010)
- [ ] **PARTIAL:** Per-rule interaction tracking not specified in grid loop flow (SC-014)

### Invariant Preservation
- [x] D1 (split/merge identity): R10 directly requires it; informal proof in Section 4.6
- [x] D2 (local reduction equivalence): preserved by using same reduction engine (R16)
- [x] D3 (border redex completeness = P3): R31-R33 require it; R15 enforces via reduce_all
- [x] D4 (ID uniqueness = P4): R2, R8 preserve; relies on SPEC-04 R16-R19
- [x] D5 (exclusive ownership): preserved by disjoint partitions
- [x] D6 (protocol termination = P5): R30 proves convergence for terminating nets
- [x] G1 (fundamental property): Section 4.6 informal proof establishes
- [x] T1 (linearity): R11 verifies via assert_all_invariants after merge
- [ ] **RISK:** Internal inconsistency between R18 (residual redexes) and merge discarding queues (SC-005)
- [ ] **RISK:** FreePort (Lafont) loss during merge would violate D1 for nets with interface ports (SC-007)
