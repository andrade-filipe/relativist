# SPEC-04 -- Round 1: Spec Critic Review

**Date:** 2026-04-05
**Reviewer:** Spec Critic (adversarial)
**Target:** SPEC-04-partition.md (status: Revised v2)
**Predecessors consulted:** SPEC-00, SPEC-01, SPEC-02, SPEC-03, SPEC-05, SPEC-13

---

## Overall Assessment

SPEC-04 is a well-structured and thorough specification of the partitioning subsystem. The split algorithm is clearly articulated in 7 steps, the correctness conditions C1-C3 are rigorously formalized, and the rationale section provides strong justification for every design decision. However, the review reveals several issues: an API mismatch with SPEC-13's revised FSM (which invokes split as an action with a different signature), an under-specified edge case around ERA agents and the port array during sub-net construction, a missing requirement for how Lafont FreePort IDs interact with border ID allocation, and a subtle correctness gap in the FreePort index maintenance during local reduction. None of these individually break the spec, but three are serious enough to warrant revision before implementation.

**Verdict:** NEEDS REVISION

---

## Issues

### SC-001: split() function signature incompatible with SPEC-13 FSM action
**Severity:** CRITICAL
**Axis:** Consistency
**Section:** 4.5 (split algorithm)
**Requirement:** R1
**Problem:** SPEC-04 Section 4.5 defines the split function as:
```
fn split(net: &Net, num_workers: u32, strategy: &dyn PartitionStrategy)
    -> PartitionPlan
```
This takes a `strategy` parameter as a trait object. However, SPEC-13 R20-R21 (Revised v2) defines the coordinator FSM action as:
```rust
InvokeSplit { net: Net, num_workers: usize },
```
and the corresponding event as:
```rust
SplitComplete(Vec<Partition>),
```

There are three discrepancies:

1. **`num_workers` type:** SPEC-04 uses `u32`, SPEC-13 uses `usize`. SPEC-05 R25 also uses `u32`. Two specs agree on `u32`; SPEC-13 disagrees.
2. **Strategy parameter missing in SPEC-13:** The `InvokeSplit` action has no `strategy` field. The coordinator FSM has no way to pass the partitioning strategy to the split action. The strategy must either be stored in coordinator state (not defined in SPEC-13) or hardcoded.
3. **Return type:** SPEC-04 returns `PartitionPlan` (which contains `Vec<Partition>` + `HashMap<u32, (PortRef, PortRef)>` border map). SPEC-13's `SplitComplete` event carries only `Vec<Partition>`, losing the border map. The merge action `InvokeMergeAndReduce(Vec<Partition>)` also takes only `Vec<Partition>` -- but SPEC-05 R1 requires the border map as input to merge. Where does it go?

**Impact if unresolved:** The implementer cannot reconcile the FSM action with the split function signature. The border map is lost between split and merge, making merge impossible. This is a data flow breakage.
**Suggested resolution:** (a) Fix SPEC-13's `InvokeSplit` to carry the strategy and use `u32` for `num_workers`. (b) Change `SplitComplete` to carry `PartitionPlan` instead of `Vec<Partition>`. (c) Change `InvokeMergeAndReduce` to accept `PartitionPlan` (or at minimum `Vec<Partition>` + border map). Alternatively, SPEC-04 can note that these discrepancies are resolved in SPEC-13.

---

### SC-002: sub-net construction does not specify handling of ERA's unused port slots
**Severity:** HIGH
**Axis:** Completeness
**Section:** 4.5 (Step 5: Build sub-nets)
**Requirement:** R9
**Problem:** Step 5 of the split algorithm says: "For internal wires: copy the port array entries directly." The port array uses `PORTS_PER_SLOT = 3` slots per agent (SPEC-02, Section 4.3). ERA agents have arity 0 -- they use only slot 0 (principal port). Slots 1 and 2 are `DISCONNECTED` sentinels (SPEC-02, Section 4.4).

When building the sub-net for partition `i`, the algorithm needs to copy port array entries for each agent in `A_i`. For ERA agents, slots 1 and 2 must be explicitly set to `DISCONNECTED` in the sub-net's port array. But the spec says "copy the port array entries directly" -- does this mean:
(a) Copy all 3 slots for every agent, including DISCONNECTED slots for ERA? This is correct but wasteful and must be clarified.
(b) Copy only the occupied slots (1 for ERA, 3 for CON/DUP)? This would break the uniform indexing scheme (`id * PORTS_PER_SLOT + port_id`).

Additionally, when agents are in a partition, their AgentIds may be sparse (not contiguous). The sub-net's `agents: Vec<Option<Agent>>` must be sized to accommodate the maximum AgentId in the partition. The spec does not discuss how the sub-net's Vec is sized or whether empty slots between agent IDs are filled with `None`. This is important for correctness of `port_index()` which assumes `id * 3 + port_id` indexing.

**Impact if unresolved:** The implementer may create sub-nets with incorrect port array sizing, leading to index-out-of-bounds panics or misaligned port lookups. For sparse ID distributions (e.g., after CON-DUP expansion), the sub-net could waste significant memory if the Vec is sized to `max_agent_id * 3`.
**Suggested resolution:** Add a note to Step 5 clarifying that sub-nets maintain the same indexing scheme as the original net. Agent slots not belonging to the partition MUST be `None`, and their corresponding port slots MUST be `DISCONNECTED`. The sub-net's `agents` Vec MUST be sized to at least `max_agent_id_in_partition + 1`, and the `ports` Vec to `(max_agent_id_in_partition + 1) * PORTS_PER_SLOT`.

---

### SC-003: Border ID collision with Lafont FreePort IDs is under-specified for multi-round scenarios
**Severity:** HIGH
**Axis:** Completeness | Invariant Preservation
**Section:** 3.3 (R12)
**Requirement:** R12, R15
**Problem:** R12 requires: "Border IDs MUST start from `max_existing_freeport_id + 1`." This prevents collision in the first round. But consider round 2: after merge, the net may contain Lafont FreePort IDs from the original net (which persist across rounds) and no boundary FreePort IDs (since merge resolved all of them). When the coordinator re-partitions for round 2, the `max_existing_freeport_id` is computed again.

The question is: what is `max_existing_freeport_id` in round 2? If the original net had Lafont FreePort IDs 0, 1, 2, and round 1 used border IDs 3, 4, 5, then after merge those border IDs are resolved and no longer present in the net. In round 2, `max_existing_freeport_id` would be 2 again (only Lafont FreePort IDs), and new border IDs would start at 3 -- reusing the same IDs as round 1. This is fine because the round 1 border IDs no longer exist.

However, consider a subtler case: if local reduction during round 1 creates a new Lafont-style FreePort (is this even possible?), or if an erasure rule leaves a dangling FreePort boundary sentinel that was not properly cleaned up, the `max_existing_freeport_id` calculation could be wrong.

More concretely: SPEC-04 does not define what happens when the net being partitioned in round N already contains FreePort values from a previous round's incomplete cleanup. R15 says "Pre-existing Lafont FreePorts from the original net MUST NOT be treated as border wires." But what about stale FreePort (Boundary) sentinels that somehow survived a merge?

**Impact if unresolved:** If a FreePort (Boundary) sentinel survives merge (due to a bug in merge or an erasure scenario), the next round's `max_existing_freeport_id` calculation would be elevated, wasting border ID space. More critically, the stale FreePort could be misinterpreted during the next round's wire classification (Step 4), since the algorithm cannot distinguish Lafont from Boundary FreePort at the type level (SPEC-02, R4 note).
**Suggested resolution:** Add a precondition to the split function: "The input net MUST NOT contain any FreePort (Boundary) sentinels. All boundary FreePorts MUST have been resolved by the preceding merge (SPEC-05). If any remain, the split MUST signal an error in debug mode." Additionally, clarify that `max_existing_freeport_id` traverses only the port array values that are `FreePort(_)` variants and takes the maximum of their IDs.

---

### SC-004: FreePort index maintenance Scenario 2 (Erasure) creates an invariant violation not addressed by SPEC-05
**Severity:** HIGH
**Axis:** Invariant Preservation
**Section:** 4.6 (FreePort Index Maintenance During Local Reduction)
**Requirement:** (informative section, but critical for D1/D3)
**Problem:** Section 4.6 Scenario 2 states: "Agent `a` connected to `FreePort(bid)` is destroyed by an erasure rule (ERA interacts with it). The wire `(AgentPort(a, p), FreePort(bid))` disappears. The `free_port_index` MUST remove the entry: `index.remove(bid)`."

This scenario describes a situation where ERA interacts with an agent `a` whose auxiliary port is connected to `FreePort(bid)`. The erasure rule (CON-ERA or DUP-ERA) removes agent `a` and creates 2 new ERA agents connected to `a`'s auxiliary ports. If port `p` of `a` was `aux1` or `aux2`, then one of the new ERA agents would be connected to `FreePort(bid)`. The FreePort is NOT destroyed -- it is inherited by the new ERA agent's principal port.

But if port `p` is the principal port (port 0) of `a`, and `a` is being consumed as part of an active pair with ERA, then the situation is different. The active pair is `(a, era)` connected through their principal ports. The rule removes both and reconnects. But `a`'s principal port was connected to ERA's principal port (forming the redex), not to `FreePort(bid)`. So if `FreePort(bid)` is connected to `a`'s **auxiliary** port, then the new ERA agents inherit that connection, and the FreePort index should be UPDATED, not REMOVED.

The scenario as written conflates two cases:
1. ERA erases an agent whose auxiliary port connects to a FreePort -- the FreePort is inherited by a new ERA agent (index UPDATED).
2. An agent connected to FreePort via its principal port has no interaction partner on that side (because the partner is in another partition) -- the FreePort is effectively a wall. The agent can only interact with another agent on a different port. If that interaction erases the agent, the FreePort connection is inherited by the replacement agents.

There is actually NO scenario in which a FreePort connection simply disappears during local reduction, because FreePort (Boundary) acts as a wall. The only way a FreePort entry could be removed is if ERA-ERA void happens on both sides of a boundary simultaneously (both sides reduce to nothing), but that is resolved at merge time, not locally.

**Impact if unresolved:** The implementer may incorrectly remove FreePort index entries, causing the merge to lose track of border connections. This would violate D1 (split/merge identity) and D3 (border redex completeness).
**Suggested resolution:** Revise Section 4.6 Scenario 2 to clarify: (a) A FreePort connection is NEVER simply deleted during local reduction. It is always either preserved or transferred to a new agent. (b) The only case where the FreePort index entry changes is when the agent connected to it is consumed and replaced (Scenarios 1 and 3). (c) If, after local reduction, the port connected to `FreePort(bid)` has been replaced by a `DISCONNECTED` sentinel (because both agents in a local redex were removed without creating new connections to the FreePort), then the entry SHOULD be removed and the merge should handle this gracefully (as SPEC-05 R6 allows). Add a concrete example showing which reduction scenarios lead to each outcome.

---

### SC-005: Lazy FreePort index reconstruction does not handle the case where FreePort values are not in the port array
**Severity:** HIGH
**Axis:** Completeness
**Section:** 4.6 (approach 2: lazy reconstruction)
**Requirement:** R13
**Problem:** Section 4.6 recommends lazy reconstruction: "The `free_port_index` is rebuilt by scanning the port array before merge." But SPEC-02 Section 4.4 defines `DISCONNECTED = PortRef::FreePort(u32::MAX)`. The lazy reconstruction scans the port array for entries matching `FreePort(_)` -- but it must distinguish between:
- `FreePort(bid)` where `bid` is a boundary FreePort (should be in the index)
- `FreePort(f)` where `f` is a pre-existing Lafont FreePort (should NOT be in the index)
- `FreePort(u32::MAX)` which is `DISCONNECTED` (should be ignored)

How does the lazy reconstruction distinguish these three cases? The type system does not differentiate them (SPEC-02 R4 note: "Both are structurally identical at the type level"). The spec does not provide the mechanism.

One approach: the reconstruction only includes entries whose `FreePort(id)` matches a `border_id` from the original `PartitionPlan.borders` HashMap. But the partition does not carry the global border map -- it only has its local `free_port_index`. During local reduction, new FreePort entries might be created (scenario 3), and the only way to know which IDs are borders vs. Lafont is to have the border ID range.

**Impact if unresolved:** The lazy reconstruction may include Lafont FreePorts in the border index, causing the merge to attempt reconnection of interface ports. Or it may include `DISCONNECTED` sentinels (u32::MAX) as a border, causing a crash or silent corruption.
**Suggested resolution:** Add a requirement that each `Partition` carries metadata to distinguish border FreePort IDs from Lafont FreePort IDs. Options: (a) store the `border_id_range: (u32, u32)` (min, max border ID assigned during split) in the Partition struct, (b) store a `HashSet<u32>` of border IDs assigned to this partition, or (c) require that border IDs are always strictly greater than all Lafont FreePort IDs (which R12 already guarantees) AND that `DISCONNECTED` (u32::MAX) is excluded by range check. Option (c) is the simplest: the reconstruction includes `FreePort(id)` where `border_start <= id < border_end` and `id != u32::MAX`. But this requires the partition to know `border_start` and `border_end`.

---

### SC-006: R18 ID range computation has an off-by-one: last worker range does not include u32::MAX
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 3.4 (R18), 4.7
**Requirement:** R18
**Problem:** R18 specifies:
```
chunk_size = u32::MAX / num_workers
range_i = [i * chunk_size, (i + 1) * chunk_size)
```
and "The last worker's range extends to `u32::MAX` (inclusive)."

However, `u32::MAX = 4_294_967_295`. With 8 workers: `chunk_size = 4_294_967_295 / 8 = 536_870_911` (integer division). Worker 7's range would be `[7 * 536_870_911, u32::MAX] = [3_758_096_377, 4_294_967_295]`. But the example in Section 4.7 says Worker 7 starts at `3_758_096_384`. There is a discrepancy: `7 * 536_870_911 = 3_758_096_377`, not `3_758_096_384`. The example uses `chunk_size = 536_870_912` (as if computed by `(u32::MAX + 1) / 8`), but `u32::MAX + 1` overflows `u32`.

The Step 6 pseudocode in Section 4.5 uses a different formula: `chunk_size = u32::MAX / num_workers`, then `id_range.end = if i == num_workers - 1 { u32::MAX } else { (i+1) * chunk_size }`. For worker 0 with chunk_size 536_870_911: range is `[0, 536_870_911)`. For worker 1: `[536_870_911, 1_073_741_822)`. These ranges are correct but leave a gap at the top: the last worker's range starts at `7 * 536_870_911 = 3_758_096_377` and ends at `u32::MAX = 4_294_967_295`, giving it `536_870_918` IDs instead of `536_870_911`. This asymmetry is acceptable but should be documented.

More importantly, the example numbers in Section 4.7 do not match the formula in R18. This will confuse the implementer.

**Impact if unresolved:** The implementer may use the wrong chunk_size formula, creating overlapping ID ranges (catastrophic for D4) or gaps (wasted but not incorrect).
**Suggested resolution:** Fix the example in Section 4.7 to match the formula in R18. Use consistent numbers: `chunk_size = u32::MAX / 8 = 536_870_911`, Worker 0: `[0, 536_870_911)`, ..., Worker 7: `[3_758_096_377, 4_294_967_295]`. Or change the formula to use `(u32::MAX as u64 + 1) / num_workers as u64` computed in `u64` to avoid overflow, then truncate back to `u32`. Document the asymmetry for the last worker.

---

### SC-007: R2 trivial case claims O(1) but requires cloning the net
**Severity:** MEDIUM
**Axis:** Testability
**Section:** 3.1 (R2)
**Requirement:** R2
**Problem:** R2 states: "If `n == 1`, the function MUST return the entire net as a single partition with no borders. This is the trivial case and MUST execute in O(1) modulo cloning the net."

The phrase "O(1) modulo cloning" is imprecise. Cloning a `Net` is O(A + W) because it must copy the `Vec<Option<Agent>>` (O(A) elements), the `Vec<PortRef>` port array (O(A * 3) elements), and the `VecDeque` redex queue. The "O(1) modulo cloning" effectively means O(A + W), which is the same complexity as the general case (R26).

If the intent is to allow moving the net instead of cloning (taking ownership), the function signature `split(net: &Net, ...)` prevents this because it takes a reference. To achieve true O(1) for `n == 1`, the function would need to take `net: Net` (by value) and move it into the partition.

**Impact if unresolved:** The O(1) claim is misleading. The implementer may spend time trying to achieve O(1) when it is impossible with the reference-based signature.
**Suggested resolution:** Either (a) change the O(1) claim to "O(A + W) for the clone, with no additional partitioning overhead" to be accurate, or (b) change the signature to accept `net: Net` by value so the trivial case can be O(1) by moving the net into the single partition (and the general case clones as needed). Option (b) is better for performance but changes the API.

---

### SC-008: PartitionStrategy trait returns HashMap but ContiguousIdStrategy could return a more efficient structure
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 4.2, 4.3
**Requirement:** R21, R22
**Problem:** The `PartitionStrategy` trait requires `allocate()` to return `HashMap<AgentId, WorkerId>`. For the ContiguousIdStrategy, this means creating a HashMap with one entry per live agent -- O(A) memory and O(A) time for construction.

But ContiguousIdStrategy assigns agents by contiguous ID ranges, which could be represented as a simple sorted list of breakpoints: `[breakpoint_0, breakpoint_1, ..., breakpoint_{n-1}]` where all agents with `id < breakpoint_0` go to worker 0, etc. This would be O(n) memory and O(log n) lookup per agent (binary search), compared to O(A) memory and O(1) amortized lookup for HashMap.

For large nets (tens of thousands of agents), the HashMap is wasteful. The trait forces all strategies to materialize the full assignment, even when a compact representation exists.

**Impact if unresolved:** Performance cost for partitioning is higher than necessary. However, given the TCC scope (tens of thousands of agents), this is not a practical bottleneck.
**Suggested resolution:** Keep the current design for simplicity (the TCC scope does not require optimization). Add a note in the Rationale or as a SHOULD: "Future strategies MAY use more compact representations by extending or replacing the trait." This is already partially covered by R23 ("The trait SHOULD allow future alternative implementations").

---

### SC-009: split algorithm Step 4 processes only agent_id < other_id side of border wires, but port direction is ambiguous
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 4.5 (Step 4)
**Requirement:** R11
**Problem:** Step 4 says: "a border wire is processed only by the side with the smaller AgentId (`a.id < b.id`)." This avoids double-counting. The pseudocode correctly applies this:
```
if sigma[agent_id] != sigma[other_id] and agent_id < other_id:
    bid = border_id_counter++
    borders[bid] = (AgentPort(agent_id, port_id), AgentPort(other_id, other_port))
    border_entries[sigma[agent_id]].push((agent_id, port_id, bid))
    border_entries[sigma[other_id]].push((other_id, other_port, bid))
```

However, the pseudocode iterates `for port_id in 0..total_ports(agent.symbol)`. For agent `a` with `sigma(a) = i`, when iterating port 0 of `a`, if `target = AgentPort(b, 0)` with `sigma(b) = j` and `a.id < b.id`, this creates a border entry. Later, when iterating agent `b` (in the outer loop), port 0 of `b` has `target = AgentPort(a, 0)` with `b.id > a.id`, so the condition `agent_id < other_id` fails and no duplicate entry is created. Good.

But there is a subtlety: the outer loop iterates `for agent_id in net.live_agents()`. The pseudocode does not specify the iteration order. If the iteration processes agents in arbitrary order, the `agent_id < other_id` guard still works correctly regardless of order. This is fine.

However, what happens when the same port of agent `a` is traversed as part of the inner loop for two different ports? This cannot happen because each port is traversed exactly once. OK, this is actually correct.

The real issue is more subtle: when building `border_entries` for partition `sigma[other_id]`, the code pushes `(other_id, other_port, bid)`. But at this point in the iteration, we are processing agent `agent_id`, not `other_id`. The code assumes we can look up `other_port` from `target`. This is correct as long as `target = AgentPort(other_id, other_port)` provides the port_id of the other side. This IS available from the `get_target` result. So the algorithm is correct.

However, the spec does not explicitly state that `border_entries` is populated for BOTH partitions in a single pass. The narrative in Step 4 says "a border wire is processed only by the side with the smaller AgentId" -- but the pseudocode processes BOTH sides (pushing entries for both `sigma[agent_id]` and `sigma[other_id]`). The narrative is misleading because "processed by the side" suggests only one side gets the FreePort entry.

**Impact if unresolved:** Minor narrative confusion. The pseudocode is correct; the narrative is ambiguous. The implementer reading only the narrative (not the pseudocode) might miss that both partitions need FreePort entries.
**Suggested resolution:** Clarify the narrative in Step 4: "a border wire is DETECTED only from the side with the smaller AgentId (to avoid duplication), but FreePort entries are generated for BOTH partitions."

---

### SC-010: No requirement for split to preserve the root port
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 3.1, 4.5
**Requirement:** (missing)
**Problem:** SPEC-02 Section 4.3 defines `root: Option<PortRef>` as a field of Net: "the AgentPort connected to the external observation point of the net." When the net is split into partitions, the root port belongs to exactly one partition (the partition containing the agent whose port is the root). The spec does not require:

1. That the root port be assigned to the sub-net of the partition containing its agent.
2. That the other partitions have `root: None`.
3. What happens if the root port is a `FreePort` (edge case).

The root port is essential for result extraction (SPEC-14 readback, SPEC-12 output). If it is lost during partitioning, the system cannot extract the final result after the last merge.

**Impact if unresolved:** The implementer may not propagate the root port, causing the merged net to lose its observation point. After the final merge, `decode_nat()` (SPEC-14) would fail because it cannot find the root.
**Suggested resolution:** Add R-number: "The sub-net of the partition containing the agent referenced by `net.root` MUST set `subnet.root = net.root`. All other partitions MUST set `subnet.root = None`. If `net.root` is `None`, all partitions MUST have `subnet.root = None`."

---

### SC-011: R3 allows excess empty partitions but does not specify their ID ranges
**Severity:** LOW
**Axis:** Completeness
**Section:** 3.1 (R3), 3.4 (R16-R18)
**Requirement:** R3, R18
**Problem:** R3 says: "If `n > |A|`, excess partitions MUST be empty (no agents, no borders, no redexes)." R18 says each worker gets an ID range. An empty partition still has an ID range assigned, even though it will never use it. This is not incorrect, but the `subnet.next_id` initialization `max(id_range.start, max_agent_id_in(A_i) + 1)` has a subtle issue: for an empty partition, `max_agent_id_in(A_i)` is undefined (there are no agents). The pseudocode uses `.map(|m| m + 1).unwrap_or(0)`, which defaults to 0. So `subnet.next_id = max(id_range.start, 0) = id_range.start`. This is correct.

But what if the system sends empty partitions to workers? The worker would call `reduce_all` on an empty net (zero agents, zero redexes), which should return immediately. This is wasteful network overhead. SPEC-05 R26 says: "If `n == 1`, `run_grid` SHOULD reduce locally without partitioning." But there is no analogous recommendation for skipping empty partitions in the dispatch phase.

**Impact if unresolved:** Workers receive empty partitions and do useless work (trivially fast, but wasteful network traffic). This is a performance issue, not a correctness issue.
**Suggested resolution:** Add a SHOULD: "The coordinator SHOULD skip dispatching empty partitions to workers. Workers assigned empty partitions SHOULD NOT receive `AssignPartition` messages." Or note this as an optimization left to the implementer.

---

### SC-012: PartitionPlan lacks serde derives but Partition has them
**Severity:** LOW
**Axis:** Consistency
**Section:** 4.1 (Types)
**Requirement:** (design section)
**Problem:** The `Partition` struct derives `serde::Serialize, serde::Deserialize` (Section 4.1). The `PartitionPlan` struct does NOT derive serde traits. This is inconsistent. The `PartitionPlan` contains `borders: HashMap<u32, (PortRef, PortRef)>` which is serde-serializable (PortRef already derives serde). If the `PartitionPlan` needs to be transmitted over the wire (e.g., the border map sent from coordinator to workers), it must be serializable.

However, looking at the architecture: the coordinator sends individual `Partition` objects to workers (SPEC-06, `AssignPartition`), not the full `PartitionPlan`. The `PartitionPlan` stays on the coordinator. So serialization of `PartitionPlan` may not be needed.

**Impact if unresolved:** Minor inconsistency. If the border map ever needs to be serialized (e.g., for checkpointing), the derives will be missing.
**Suggested resolution:** Either add serde derives to `PartitionPlan` for consistency, or add a comment explaining why they are intentionally omitted.

---

### SC-013: R4 and R5 (determinism, purity) are not verifiable by automated tests
**Severity:** LOW
**Axis:** Testability
**Section:** 3.1 (R4, R5)
**Requirement:** R4, R5
**Problem:** R4 says "The split operation MUST be deterministic: same net + same sigma = same output." R5 says "MUST NOT depend on external state, wall-clock time, or randomness." These are properties of the implementation, not of the output. They are testable only by:
(a) Running split twice with the same input and comparing outputs (tests R4 but not R5 -- a function could use wall-clock time deterministically if run fast enough).
(b) Code review (tests R5 but not mechanically).

For SPEC-08 (Test Strategy), how should these be tested? Property-based testing can verify R4 by running split multiple times. R5 is essentially a code quality constraint.

**Impact if unresolved:** Minor. R4 is testable by repetition. R5 is a code quality requirement that cannot be mechanically verified.
**Suggested resolution:** Keep as-is. Note in SPEC-08 that R4 is tested via "split(net, plan) == split(net, plan)" assertions. R5 is verified by code review (no side effects, no I/O, no randomness).

---

### SC-014: No explicit requirement for split to handle nets with pre-existing FreePort (Lafont) correctly
**Severity:** LOW
**Axis:** Completeness
**Section:** 3.2, 3.3
**Requirement:** R7, R15
**Problem:** R7 says interface wires (involving a pre-existing Lafont FreePort) are "preserved with its pre-existing FreePort in the partition of its AgentPort endpoint." R15 says Lafont FreePorts "MUST NOT be treated as border wires." These two requirements together define the correct behavior.

However, there is no explicit test requirement or assertion for this case. What if a Lafont FreePort is connected to an agent in partition `i`? The interface wire `(AgentPort(a, p), FreePort(f))` should appear in partition `i`'s port array. But the port array stores `FreePort(f)` at index `port_index(a, p)`. There is no reverse entry (FreePort has no slot in the port array -- SPEC-02, Section 4.10). The `free_port_index` should NOT contain this Lafont FreePort, since it is not a boundary sentinel.

The lazy reconstruction algorithm (Section 4.6 approach 2) scans the port array for `FreePort(_)` entries. If it does not distinguish Lafont from Boundary FreePort IDs, it will incorrectly add Lafont FreePorts to the `free_port_index`. This is the same issue as SC-005 but from the perspective of correctness conditions.

**Impact if unresolved:** Covered by SC-005. This entry reinforces the need for a disambiguation mechanism.
**Suggested resolution:** Same as SC-005.

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 1 |
| HIGH | 4 |
| MEDIUM | 4 |
| LOW | 5 |

## Mandatory (must fix before implementation)

- **SC-001:** split() signature incompatible with SPEC-13 FSM action -- reconcile return type, strategy parameter, and num_workers type
- **SC-002:** Sub-net construction does not specify ERA port slot handling or sparse agent ID sizing
- **SC-003:** Border ID collision risk for multi-round scenarios -- add precondition that input net has no stale boundary FreePorts
- **SC-004:** FreePort index maintenance Scenario 2 (Erasure) incorrectly claims FreePort connections can disappear during local reduction
- **SC-005:** Lazy FreePort index reconstruction cannot distinguish Lafont FreePorts from Boundary FreePorts from DISCONNECTED sentinels

## Recommended (should fix)

- **SC-006:** ID range formula and example numbers are inconsistent -- fix example or formula
- **SC-007:** O(1) claim for trivial case is misleading given reference-based signature
- **SC-008:** PartitionStrategy trait forces HashMap materialization -- acceptable for TCC scope, document as future optimization
- **SC-009:** Step 4 narrative about "processing only one side" is misleading -- both partitions get entries

## Advisory (nice to have)

- **SC-010:** No requirement for preserving root port during split
- **SC-011:** Empty partitions are dispatched to workers -- add SHOULD to skip
- **SC-012:** PartitionPlan lacks serde derives -- add for consistency
- **SC-013:** R4/R5 purity not mechanically verifiable -- acceptable
- **SC-014:** Lafont FreePort handling reinforces SC-005

---

## Checklist

### Consistency
- [x] Types match predecessor specs (Symbol, AgentId, PortRef, Net, Agent from SPEC-02)
- [x] PortRef encoding consistent with SPEC-00/SPEC-02
- [x] Partition struct consistent with SPEC-05 merge expectations (SPEC-05 R1)
- [x] WorkerId type consistent with SPEC-00 Section 7.3
- [x] FreePort semantics consistent with SPEC-00 Sections 6.1, 6.2
- [x] Invariant references (D1, D1a-D1e, D4, D5) match SPEC-01 definitions
- [ ] **FAIL:** split() signature inconsistent with SPEC-13 InvokeSplit action (SC-001)
- [x] ContiguousIdStrategy matches Haskell prototype behavior (AC-002)
- [x] C1-C3 conditions match SPEC-00 Section 6.9 and SPEC-01 D1a-D1c

### Testability
- [x] R1 (split function): testable by invoking split and inspecting output
- [x] R2 (trivial case): testable by calling split with n=1
- [x] R3 (excess partitions): testable with n > agent count
- [x] R4 (determinism): testable by double invocation and equality check
- [x] R6-R8 (C1-C3): testable by debug assertions (R10)
- [x] R9 (internal wires preserved): testable by byte comparison of port entries
- [x] R10 (debug assertions): testable by running in debug mode
- [x] R11 (border wire replacement): testable by inspecting FreePort entries
- [x] R12 (border ID uniqueness): testable by checking range start
- [x] R13 (FreePort index): testable by inspecting HashMap after split
- [x] R16-R19 (ID ranges): testable by checking disjointness
- [x] R20 (range exhaustion): testable by simulating large allocations
- [x] R24-R25 (redex queue): testable by inspecting queue contents
- [ ] **PARTIAL:** R13 lazy reconstruction testability depends on Lafont/Boundary disambiguation (SC-005)

### Completeness
- [x] Split algorithm fully specified in 7 steps with pseudocode
- [x] Edge case n=1 handled (R2)
- [x] Edge case n > |A| handled (R3)
- [ ] **FAIL:** ERA port slot handling in sub-net construction unspecified (SC-002)
- [ ] **FAIL:** Root port preservation unspecified (SC-010)
- [ ] **FAIL:** Lazy reconstruction disambiguation mechanism missing (SC-005)
- [x] ID range computation specified (R18)
- [x] Border map fully specified (R11, border_map type)
- [x] FreePort index specified (R13)
- [x] Complexity requirements stated (R26, R27)
- [x] Rationale section comprehensive (5 subsections)

### Invariant Preservation
- [x] D1 (split/merge identity): addressed by C1-C3 (R6-R8) and D1a-D1e mapping
- [x] D1a (C1, agent coverage): explicit requirement R6
- [x] D1b (C2, wire coverage): explicit requirement R7
- [x] D1c (C3, FreePort bijectivity): explicit requirement R8
- [x] D1d (internal wires unchanged): explicit requirement R9
- [x] D1e (pure function): explicit requirements R4, R5
- [x] D4 (ID uniqueness): addressed by R16-R19
- [x] D5 (exclusive ownership): addressed by R6 (C1 disjointness)
- [x] T1 (linearity at boundaries): addressed by R14
- [ ] **PARTIAL:** D3 (border redex completeness): FreePort index correctness after local reduction depends on SC-004 resolution
- [ ] **PARTIAL:** D1 round-trip: stale FreePort (Boundary) sentinels could break subsequent rounds (SC-003)
