# SPEC-13 -- Round 1: Spec Critic Review

**Date:** 2026-04-05
**Reviewer:** Spec Critic (adversarial)
**Target:** SPEC-13-system-architecture.md (status: Draft v1)
**Predecessors consulted:** SPEC-00, SPEC-01, SPEC-02, SPEC-03, SPEC-04, SPEC-05, SPEC-06, SPEC-07, SPEC-08, SPEC-09

---

## Overall Assessment

SPEC-13 is a comprehensive integration spec that successfully ties together the BSP programming model, module structure, FSM design, transport abstraction, and CLI into a coherent architecture. However, it introduces several contradictions with predecessor specs -- particularly SPEC-06 (FSM state names and message catalog), SPEC-05 (termination condition and border redex resolution), and SPEC-07 (CLI subcommand count and naming). These inconsistencies are serious because the implementer will not know which spec is authoritative, risking divergent interpretations and broken invariants.

**Verdict:** NEEDS REVISION

---

## Issues

### SC-001: Coordinator FSM state names contradict SPEC-06
**Severity:** CRITICAL
**Axis:** Consistency
**Section:** 3.5 (R19)
**Requirement:** R19, R21
**Problem:** SPEC-13 R19 defines coordinator states as `Init`, `WaitingForWorkers`, `Partitioning`, `Dispatching`, `WaitingForResults`, `Merging`, `CheckTermination`, `Done`, `Error` (9 states). SPEC-06 R26 defines coordinator states as `WaitingWorkers`, `Idle`, `Partitioning`, `Distributing`, `WaitingResults`, `Merging`, `ShuttingDown`, `Done` (8 states). There are 5 name conflicts:

| SPEC-06 | SPEC-13 | Notes |
|---------|---------|-------|
| `WaitingWorkers` | `WaitingForWorkers` | Different name, same semantics |
| `Idle` | *(missing)* | SPEC-06 has an Idle state between merge and next partition; SPEC-13 goes directly from Merging to Partitioning |
| `Distributing` | `Dispatching` | Different name, same semantics |
| `WaitingResults` | `WaitingForResults` | Different name, same semantics |
| `ShuttingDown` | *(missing)* | SPEC-06 has an explicit shutdown state; SPEC-13 jumps from Done to ShutdownAll action |
| *(none)* | `Init` | New state not in SPEC-06 |
| *(none)* | `CheckTermination` | New state not in SPEC-06 |
| *(none)* | `Error` | New state not in SPEC-06 |

Additionally, SPEC-06 R28 explicitly says the FSM "MAY be implemented implicitly via control flow" and the documented states "serve as a specification of expected behavior, not an implementation prescription." SPEC-13 R22 says it MUST be enum-based. These prescriptions conflict.

**Impact if unresolved:** The implementer must guess which FSM is canonical. If SPEC-06 is followed, SPEC-13 tests fail. If SPEC-13 is followed, SPEC-06 tests fail. SPEC-08 test strategy references SPEC-06's FSM behavior.
**Suggested resolution:** SPEC-13 MUST explicitly supersede SPEC-06 R26-R28 for the FSM definition. Add a note: "SPEC-13 R19-R22 supersede SPEC-06 R26-R28. SPEC-06's FSM was a behavioral specification; SPEC-13 provides the concrete enum-based FSM that the implementation MUST use." Reconcile all state names and ensure both specs use identical terminology.

---

### SC-002: Register/RegisterAck messages not defined in SPEC-06 Message catalog
**Severity:** CRITICAL
**Axis:** Consistency
**Section:** 3.5 (R21), 3.10 (R40)
**Requirement:** R21, R40
**Problem:** SPEC-13's coordinator transition table (R21) references `SendMessage(id, RegisterAck)` and the worker FSM (R25) references `SendMessage(Register)`. The data flow in R40 step 3 says: "Each worker connects, sends Register, receives RegisterAck." However, SPEC-06's `Message` enum (Section 4.1) contains ONLY four variants: `AssignPartition`, `Shutdown`, `PartitionResult`, and `Error`. There is no `Register` or `RegisterAck` variant. SPEC-06 models worker registration implicitly via TCP connection acceptance (R24: "The coordinator MUST wait for all num_workers workers to connect before starting the first round").

This means SPEC-13 introduces two new message types that have no definition, no serialization format, no framing specification, and no presence in the canonical protocol enum.

**Impact if unresolved:** The implementer will create ad-hoc Register/RegisterAck messages with no spec backing, or will try to reconcile with SPEC-06 and skip them. Either way, behavior is underspecified.
**Suggested resolution:** Either (a) add `Register` and `RegisterAck` variants to SPEC-06's `Message` enum with proper field definitions, or (b) remove them from SPEC-13 and clarify that worker registration occurs implicitly upon TCP connection acceptance as SPEC-06 specifies. Option (b) is simpler and consistent with the prototype's approach.

---

### SC-003: Data flow omits border redex resolution via reduce_all after merge
**Severity:** CRITICAL
**Axis:** Completeness | Invariant Preservation
**Section:** 3.10 (R40)
**Requirement:** R40
**Problem:** SPEC-13 R40 step 4e says: `merge([P1', P2', ..., Pk']) -> net'  (SPEC-05)`, then step 4f says: "Check: is net' in Normal Form?" This omits the critical step between merge and termination check: SPEC-05 R15 mandates that "After the merge, the coordinator MUST reduce all border redexes (and any new redexes generated by those reductions) by invoking `reduce_all` (SPEC-03) on the merged net." SPEC-05 R17 further requires: "These derived redexes MUST be resolved within the same `reduce_all` invocation, not deferred to the next round."

The data flow should be:
```
e. merge([P1', P2', ..., Pk']) -> net'
e'. reduce_all(net') -> net''   <-- MISSING STEP
f. Check: is net'' in Normal Form?
```

Without this step, border redexes are never resolved, violating SPEC-01 D3 (Completeness of Border Redex Resolution = Premise P3 of ARG-001).

**Impact if unresolved:** If the implementer follows SPEC-13 R40 literally, border redexes will accumulate and the grid loop may never terminate (violating D6/P5) or produce incorrect results (violating G1).
**Suggested resolution:** Insert a step 4e' in R40: `reduce_all(net') -> net''  (SPEC-03, SPEC-05 R15)`. Update step 4f to reference `net''`. This aligns with SPEC-05 R15-R18.

---

### SC-004: Termination condition uses border_redex_count instead of is_reduced
**Severity:** HIGH
**Axis:** Consistency | Invariant Preservation
**Section:** 3.5 (R21)
**Requirement:** R21, R3
**Problem:** The coordinator transition table (R21) specifies:
- `Merging | MergeComplete(net, 0) | Done` -- terminates when `border_redex_count == 0`
- `Merging | MergeComplete(net, n) [n > 0] | Partitioning` -- re-partitions when borders exist

This decision criteria is based solely on border redex count. But SPEC-05 R27 says: "The termination condition of the grid loop MUST be: the net is in Normal Form (redex queue empty after `reduce_all`)." A net can have zero border redexes but still have local redexes remaining (e.g., if a CON-DUP reduction during border resolution created new internal redexes). Checking only `border_redex_count == 0` is insufficient.

Additionally, R3 correctly states termination requires `border_redexes(merged_net) == 0 AND local_redexes(merged_net) == 0`, but the MergeComplete event only carries `border_redex_count: usize` -- there is no `local_redex_count` or `is_normal_form` field.

**Impact if unresolved:** The grid loop could terminate prematurely with a net that still has unreduced local redexes, producing an incorrect result (violating G1).
**Suggested resolution:** (a) Change the `MergeComplete` event to carry a boolean `is_normal_form: bool` (or the net's `is_reduced()` result) instead of/in addition to `border_redex_count`. (b) The termination check should be `is_normal_form == true`, consistent with SPEC-05 R27. (c) Alternatively, since SPEC-05 R15 mandates `reduce_all` after merge, the net should always be in local normal form after the Merging state, so the correct check is: does the merged-and-fully-reduced net have any redexes at all?

---

### SC-005: CLI subcommand count and names conflict with SPEC-07
**Severity:** HIGH
**Axis:** Consistency
**Section:** 3.11 (R43)
**Requirement:** R43-R48a
**Problem:** SPEC-07 R1 specifies exactly 4 subcommands: `coordinator`, `worker`, `local`, `generate`. SPEC-13 R43 specifies 6 subcommands: `Coordinator`, `Worker`, `Reduce`, `Inspect`, `Generate`, `Compute`. The differences:

| SPEC-07 | SPEC-13 | Status |
|---------|---------|--------|
| `coordinator` | `Coordinator` | Match (case difference is Rust enum vs command) |
| `worker` | `Worker` | Match |
| `local` | *(missing)* | SPEC-13 renames it to `Reduce` (R41: "local execution mode (`relativist reduce`)") |
| `generate` | `Generate` | Match |
| *(none)* | `Inspect` | New subcommand |
| *(none)* | `Compute` | New subcommand (from SPEC-14) |

The renaming of `local` to `reduce` is semantically significant: `local` implies "local grid simulation with partitioning" (SPEC-07 R18 says it "MUST execute the grid loop entirely in-process"), while `reduce` implies "purely local reduction without partitioning" (SPEC-13 R41 says it "MUST bypass the coordinator/worker/protocol infrastructure entirely" and "call `reduce_all` directly"). These are different operations. SPEC-07's `local` runs the full grid cycle in-process; SPEC-13's `reduce` runs only `reduce_all`.

**Impact if unresolved:** The implementer does not know whether to implement SPEC-07's `local` (grid simulation) or SPEC-13's `reduce` (direct reduction), or both. The distinction matters for testing (SPEC-08) and benchmarks (SPEC-09, which references `Local` mode for in-memory grid benchmarks).
**Suggested resolution:** Keep both: `local` for in-memory grid simulation (SPEC-07 R18, SPEC-09 benchmarks) and `reduce` for direct sequential reduction (SPEC-13 R41). Update the Cli enum to have 7 subcommands. Explicitly note that SPEC-13 adds `reduce`, `inspect`, and `compute` to the original 4 from SPEC-07 without removing any.

---

### SC-006: Worker FSM ConnectionLost transition contradicts SPEC-06 abort policy
**Severity:** HIGH
**Axis:** Consistency
**Section:** 3.6 (R25)
**Requirement:** R25
**Problem:** SPEC-13 R25 defines the worker transition: `Any | ConnectionLost | Init | AttemptReconnect, LogTransition`. This implies workers automatically attempt to reconnect after a lost connection. However, SPEC-06 R25 mandates: "If a connection with a worker is lost during execution, the coordinator MUST abort the grid loop and return an error. Fault tolerance is out of scope." If the coordinator aborts on connection loss, worker reconnection is pointless -- the coordinator is already shutting down.

More fundamentally, reconnection during an active grid round would violate the BSP model: the coordinator has dispatched a partition to the disconnected worker. If the worker reconnects, the coordinator would need to re-dispatch the partition (which it may have already discarded) or skip that worker (leaving partitions unresolved).

**Impact if unresolved:** Worker code implements reconnection logic that can never succeed in practice (coordinator has already aborted). Wasted engineering effort and confusing behavior.
**Suggested resolution:** Change the worker's `ConnectionLost` transition to go to `Error` with `LogTransition` and `ShutdownSelf`, consistent with SPEC-06 R25's abort-on-failure policy. Remove `AttemptReconnect`. If reconnection is desired in the future, it should be paired with coordinator-side retry logic (which is explicitly out of scope per Z5).

---

### SC-007: CheckTermination state has no entry in the transition table
**Severity:** HIGH
**Axis:** Completeness
**Section:** 3.5 (R19, R21)
**Requirement:** R19, R21
**Problem:** SPEC-13 R19 defines a `CheckTermination` state, but R21's transition table has no transitions FROM or TO `CheckTermination`. The table goes directly from `Merging -> Done` (when border_redex_count == 0) or `Merging -> Partitioning` (when border_redex_count > 0). The `CheckTermination` state is defined but unreachable.

**Impact if unresolved:** Dead state in the FSM enum. The implementer must guess where CheckTermination fits in the lifecycle, or ignore it.
**Suggested resolution:** Either (a) insert `CheckTermination` into the transition table between `Merging` and `Done`/`Partitioning`: `Merging -> MergeComplete(net, stats) -> CheckTermination`, then `CheckTermination -> (is_normal_form=true) -> Done` and `CheckTermination -> (is_normal_form=false) -> Partitioning`, or (b) remove `CheckTermination` from the state enum since the check is implicit in the MergeComplete event. Option (a) is cleaner and matches SPEC-05 R27.

---

### SC-008: Undefined types in CoordinatorAction: TimerId, MetricEvent, WorkerHandle
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 3.5 (R20)
**Requirement:** R20
**Problem:** The `CoordinatorAction` enum (R20) references `TimerId`, `MetricEvent`, and `Duration`. The coordinator event loop pseudocode (Section 4.2) references `WorkerHandle`. None of these types are defined in SPEC-13 or in any predecessor spec. `Duration` is `std::time::Duration` (standard library) and is fine. But `TimerId`, `MetricEvent`, and `WorkerHandle` are entirely unspecified:
- What is `TimerId`? A `u32`? A string name? An opaque handle?
- What is `MetricEvent`? An enum? What variants does it have?
- What is `WorkerHandle`? What does it contain (TCP stream, worker_id, channel)?

**Impact if unresolved:** The implementer must invent these types, potentially creating inconsistencies with the observability spec (SPEC-11) or metrics collection in SPEC-05/SPEC-09.
**Suggested resolution:** Add brief type definitions to Section 4 or Section 2:
```rust
type TimerId = u32;
enum MetricEvent { RoundStarted(u32), RoundCompleted { round: u32, stats: GridMetrics }, ... }
struct WorkerHandle { id: WorkerId, transport: Box<dyn Transport> }
```
Mark the exact variants as MAY (implementer decides), but define the type's purpose and fields.

---

### SC-009: ChannelTransport bypasses serialization, undermining integration test fidelity
**Severity:** MEDIUM
**Axis:** Completeness | Testability
**Section:** 3.7 (R31), Section 4.5
**Requirement:** R31
**Problem:** R31 states that `ChannelTransport` uses `tokio::sync::mpsc` "with zero serialization overhead." Section 4.5 confirms: "Full BSP loop runs in-process with zero serialization overhead." Open Question 5 acknowledges this gap but defers it. However, SPEC-06 R14 requires `deserialize(serialize(msg)) == msg` -- this invariant is ONLY testable if the message actually goes through serialization. SPEC-08's integration tests (e.g., the round-trip property for grid cycle) would silently pass even if the `Message` type has a serialization bug, because ChannelTransport never serializes.

This is a known gap (acknowledged in OQ-5) but the spec provides no concrete mechanism for testing serialization correctness in the in-memory grid path.

**Impact if unresolved:** Serialization bugs (e.g., a field missing `Serialize` derive, or bincode encoding mismatch) would not be caught by integration tests and would only manifest in TCP mode.
**Suggested resolution:** Upgrade OQ-5 to a requirement: SPEC-13 SHOULD define a `SerializingChannelTransport` that wraps `ChannelTransport` and runs bincode round-trip on every send/recv. At minimum, one integration test per benchmark SHOULD use the serializing variant.

---

### SC-010: R4 is a negative requirement that belongs in v1 exclusions, not in BSP section
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 3.1 (R4)
**Requirement:** R4
**Problem:** R4 says: "Relativist MUST NOT implement MapReduce [...] or Dataflow [...]. The BSP classification MUST be documented in the coordinator's module-level documentation." The first sentence is a design exclusion (negative requirement). The second sentence is a documentation requirement. Both are reasonable but misplaced: the exclusion belongs in Section 3.12 "What is NOT in v1" (alongside R49), and the documentation requirement should be a separate R-number in the module structure section.

**Impact if unresolved:** Minor organizational confusion. No correctness issue.
**Suggested resolution:** Move the exclusion part of R4 into R49's table. Keep the documentation requirement as a separate R-number or merge it with R5's module documentation.

---

### SC-011: async_trait in Transport trait may conflict with Core Layer purity
**Severity:** MEDIUM
**Axis:** Consistency
**Section:** 3.7 (R28)
**Requirement:** R28, R6
**Problem:** R28 defines the `Transport` trait in the `protocol` module with `#[async_trait::async_trait]` and `async fn` methods. The `protocol` module is part of the Infrastructure Layer (R7), so this is consistent. However, the trait definition in R28 uses `async_trait` as a dependency, which OQ-1 acknowledges may be replaceable with native async traits. The issue is that `async_trait` itself pulls in `proc-macro2`, `syn`, and `quote` as compile-time dependencies, increasing compile time. This is not a correctness issue, but OQ-1 should be elevated from "does not block" to "SHOULD evaluate before implementation."

**Impact if unresolved:** Unnecessary compile-time dependency if native async traits suffice.
**Suggested resolution:** Change OQ-1 to a SHOULD requirement: "The implementer SHOULD use native async traits (Rust 1.75+) if they support `Box<dyn Transport>` dispatch. If not, `async_trait` is acceptable."

---

### SC-012: Worker FSM lacks HeartbeatTimeout handling
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 3.6 (R25)
**Requirement:** R25
**Problem:** The coordinator FSM (R21) includes `HeartbeatTimeout(WorkerId)` as an event, transitioning to `Error`. But the worker FSM (R25) has no corresponding heartbeat-related transition or mechanism. If the coordinator expects heartbeats, the worker must send them. If the coordinator uses a timer-based heartbeat (monitoring silence), the spec should clarify that the heartbeat is a coordinator-side timeout on inactivity, not a worker-initiated message.

SPEC-06 has no heartbeat concept at all -- it uses phase-level timeouts (R30: `distribute_timeout`, `collect_timeout`).

**Impact if unresolved:** The implementer may implement heartbeat logic in the coordinator without worker support, leading to premature timeouts. Or the term "heartbeat" may be conflated with the phase timeouts from SPEC-06.
**Suggested resolution:** Clarify that `HeartbeatTimeout` is actually a phase-level inactivity timeout (e.g., "no PartitionResult received within collect_timeout"), not a heartbeat protocol. Rename the event to `PhaseTimeout(WorkerId)` or `CollectTimeout(WorkerId)` to align with SPEC-06 R30-R31 terminology.

---

### SC-013: R38 specifies default features as empty but tokio is always-on
**Severity:** LOW
**Axis:** Consistency
**Section:** 3.9 (R38)
**Requirement:** R38
**Problem:** R38 says "The default feature set MUST be empty." R11 lists `tokio` as an always-on dependency. This is technically consistent (tokio is a dependency, not a feature), but may confuse readers. The statement should clarify that "default features" refers to Cargo features in `[features] default = []`, not to the base dependency set.

**Impact if unresolved:** Potential misunderstanding by the implementer about what "default = []" means.
**Suggested resolution:** Rephrase R38 to: "The `default` Cargo feature set MUST be empty (`default = []`). All always-on dependencies from R11 are unconditional and do not require feature gates."

---

### SC-014: CoordinatorEvent::SplitComplete but split is synchronous
**Severity:** LOW
**Axis:** Completeness
**Section:** 3.5 (R20)
**Requirement:** R20
**Problem:** The `CoordinatorEvent` enum includes `SplitComplete(Vec<Partition>)`. But `split()` is a synchronous Core Layer operation (SPEC-04, from the `partition` module which is in the Core Layer per R6). If the coordinator FSM's event loop runs on the async runtime, calling `split()` is a blocking CPU operation. Unlike `reduce_all` which is offloaded via `spawn_blocking` (R26 for workers), SPEC-13 does not specify how `split()` is invoked. Is it called synchronously in the FSM transition function (making the transition impure)? Or is it offloaded to a blocking task that fires `SplitComplete` as an event?

The same issue applies to `merge()` -- it is a Core Layer synchronous operation, but the FSM receives `MergeComplete` as an event.

**Impact if unresolved:** The boundary between the pure transition function and side effects is blurred. The "stimulus-response pattern" (R20) assumes pure transitions, but if `split()` and `merge()` are called inside transitions, they become impure.
**Suggested resolution:** Clarify in Section 4.2 that `split()` and `merge()` are invoked as actions (like `SendMessage`), not inside the transition function. The action executor calls `split()` synchronously (or via `spawn_blocking` for large nets), and fires `SplitComplete`/`MergeComplete` events back into the FSM. This preserves the stimulus-response purity.

---

### SC-015: run_grid_local function signature inconsistent with SPEC-05
**Severity:** LOW
**Axis:** Consistency
**Section:** 4.5
**Requirement:** (informative section)
**Problem:** SPEC-13 Section 4.5 defines `run_grid_local(net: Net, num_workers: usize)` returning `Result<(Net, GridMetrics), RelativistError>`. SPEC-05 R25 defines `run_grid` accepting `(net: Net, num_workers: u32, strategy: impl PartitionStrategy)` returning `(Net, GridMetrics)`. The discrepancies: (a) `num_workers` is `usize` in SPEC-13 vs `u32` in SPEC-05/SPEC-04, (b) SPEC-13 omits the `strategy` parameter, (c) SPEC-13 wraps in `Result` while SPEC-05 does not.

**Impact if unresolved:** Type mismatch at the integration boundary. Minor but annoying.
**Suggested resolution:** Align `run_grid_local` with SPEC-05's `run_grid` signature: use `u32` for `num_workers`, include a `strategy` parameter, and decide whether to use `Result` (SPEC-05 should probably also use `Result`).

---

### SC-016: No requirement for coordinator to run reduce_all after merge
**Severity:** LOW (because SC-003 already covers the data flow; this is the FSM angle)
**Axis:** Invariant Preservation
**Section:** 3.5 (R21)
**Requirement:** R21
**Problem:** The coordinator FSM transition table shows `Merging -> MergeComplete -> Done/Partitioning`. There is no state or action for running `reduce_all` on the merged net. SPEC-05 R15 requires this step. The FSM should either: (a) include a `Reducing` state between `Merging` and `CheckTermination` for the coordinator-side `reduce_all`, or (b) include `reduce_all` as part of the `Merging` state's responsibility (documented explicitly).

**Impact if unresolved:** Covered by SC-003 and SC-004, but the FSM itself is incomplete.
**Suggested resolution:** Add a note that the `Merging` state encompasses both merge and post-merge `reduce_all`, or add a `ResolvingBorders` state.

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 3 |
| HIGH | 4 |
| MEDIUM | 4 |
| LOW | 5 |

## Mandatory (must fix before implementation)

- **SC-001:** Coordinator FSM state names contradict SPEC-06 -- reconcile state names and establish which spec is authoritative
- **SC-002:** Register/RegisterAck messages not defined in SPEC-06 -- add to Message enum or remove from SPEC-13
- **SC-003:** Data flow omits border redex resolution -- add reduce_all step after merge in R40
- **SC-004:** Termination condition uses border_redex_count only -- should check is_normal_form per SPEC-05 R27
- **SC-005:** CLI subcommand count/names conflict with SPEC-07 -- reconcile `local` vs `reduce` semantics
- **SC-006:** Worker ConnectionLost reconnection contradicts SPEC-06 abort policy
- **SC-007:** CheckTermination state is unreachable in transition table

## Recommended (should fix)

- **SC-008:** Undefined types TimerId, MetricEvent, WorkerHandle
- **SC-009:** ChannelTransport serialization gap undermines integration test fidelity
- **SC-010:** R4 negative requirement misplaced
- **SC-011:** async_trait evaluation should be SHOULD, not deferred
- **SC-012:** HeartbeatTimeout naming conflicts with SPEC-06 phase timeouts

---

## Checklist

### Consistency
- [x] Types match predecessor specs (Symbol, AgentId, PortRef, Net, Agent)
- [x] PortRef encoding consistent with SPEC-00/SPEC-02
- [ ] **FAIL:** FSM state names differ from SPEC-06 R26 (SC-001)
- [ ] **FAIL:** Message types Register/RegisterAck not in SPEC-06 (SC-002)
- [ ] **FAIL:** CLI subcommands differ from SPEC-07 R1 (SC-005)
- [ ] **FAIL:** Worker reconnection contradicts SPEC-06 R25 abort policy (SC-006)
- [ ] **FAIL:** HeartbeatTimeout not aligned with SPEC-06 R30-R31 phase timeouts (SC-012)
- [x] WorkerId type consistent with SPEC-04 (u32)
- [x] GridMetrics structure consistent with SPEC-05 R35
- [x] Error handling with thiserror is consistent with SPEC-07 approach
- [x] Feature flags do not affect Core Layer purity (R6/R8)

### Testability
- [x] R1 (BSP model): testable via in-memory grid cycle
- [x] R2 (barrier sync): testable by verifying all partitions collected before merge
- [ ] **PARTIAL:** R3 (termination): testable but condition is incomplete per SC-004
- [x] R5 (module structure): testable via compile-time dependency checks
- [x] R6 (Core Layer purity): testable by attempting to compile core modules without tokio
- [x] R19 (coordinator FSM states): testable via state enum assertion
- [x] R20 (stimulus-response): testable via pure function unit tests
- [x] R21 (transition table): testable via (state, event) -> (state, actions) assertions
- [ ] **FAIL:** R21 CheckTermination state untestable because unreachable (SC-007)
- [x] R27 (worker isolation): testable by verifying no cross-worker state
- [x] R34 (Core Layer sync): testable by grep for `async fn` in core modules
- [x] R41 (local reduce mode): testable by comparing with grid result

### Completeness
- [ ] **FAIL:** Data flow (R40) omits reduce_all after merge (SC-003)
- [ ] **FAIL:** CheckTermination state defined but unreachable (SC-007)
- [ ] **FAIL:** TimerId, MetricEvent, WorkerHandle undefined (SC-008)
- [ ] **PARTIAL:** ChannelTransport serialization gap acknowledged but unresolved (SC-009)
- [x] Module structure complete (11 modules listed)
- [x] Dependency map complete with versions and justifications
- [x] Error handling complete with per-module enums
- [x] Feature flags complete with dep: syntax
- [x] v1 exclusions comprehensive with justifications
- [x] Rationale section covers all major decisions

### Invariant Preservation
- [x] T1-T5 (theoretical invariants): not affected by architecture decisions
- [ ] **FAIL:** D3 (border redex completeness = P3): data flow omits reduce_all after merge (SC-003)
- [ ] **FAIL:** D6 (protocol termination = P5): termination check on border_redex_count only may not detect local redexes (SC-004)
- [x] D4 (ID uniqueness = P4): static ID space partitioning preserved in architecture
- [x] D5 (exclusive ownership): star topology + no shared state (R35) preserves
- [x] G1 (fundamental property): R41 correctly requires reduce == grid result
- [x] I1-I4 (implementation invariants): Core Layer purity ensures these are tested independently
