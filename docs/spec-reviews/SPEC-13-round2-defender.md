# SPEC-13 -- Round 2: Defender Response

**Date:** 2026-04-05
**Defender:** Especialista em Specs
**Target:** SPEC-13-system-architecture.md
**Critic review:** SPEC-13-round1-critic.md
**Spec version:** Draft v1 -> Revised v2

---

## Summary

| Response | Count |
|----------|-------|
| ACCEPTED | 12 |
| PARTIALLY ACCEPTED | 3 |
| NOT ADDRESSED | 1 |
| **Total issues** | **16** |

---

## Responses

### SC-001: Coordinator FSM state names contradict SPEC-06
**Response:** ACCEPTED
**Action taken:** Added an explicit supersession note to R19 stating that "SPEC-13 R19-R22 supersede SPEC-06 R26-R28 for the coordinator FSM definition." The note maps each SPEC-06 state name to its SPEC-13 counterpart and explains the rationale for differences (e.g., `Idle` replaced by `CheckTermination`, `ShuttingDown` subsumed by `Done` + `ShutdownAll` action). State names in SPEC-13 are now declared as authoritative. The R22 requirement for enum-based FSM remains, with the note clarifying that SPEC-06 R28's "MAY be implemented implicitly" is superseded by SPEC-13's concrete prescription.
**Spec sections modified:** Section 3.5 (R19 -- added supersession note and updated doc comments)

### SC-002: Register/RegisterAck messages not defined in SPEC-06 Message catalog
**Response:** ACCEPTED
**Action taken:** Adopted option (b) from the critic: removed all references to `Register` and `RegisterAck` messages. Worker registration is now implicit upon TCP connection acceptance, consistent with SPEC-06 R24. Specific changes:
- `CoordinatorEvent::WorkerRegistered(WorkerId)` renamed to `WorkerConnected(WorkerId)`
- Transition table row updated: `WaitingForWorkers | WorkerConnected(id)` with no `SendMessage(id, RegisterAck)` action
- Worker FSM row `Init | Connected | Idle` no longer has `SendMessage(Register)` action
- Data flow (R40) step 3 rewritten: "Workers connect: Each worker establishes TCP connection (registration is implicit upon connection acceptance, consistent with SPEC-06 R24)"
- Worker main loop pseudocode (Section 4.3) updated to remove `transport.send(&Message::Register {...})` call
- Added explicit notes to both coordinator (R21) and worker (R25) transition tables explaining implicit registration
**Spec sections modified:** Section 3.5 (R20, R21), Section 3.6 (R25), Section 3.10 (R40), Section 4.3

### SC-003: Data flow omits border redex resolution via reduce_all after merge
**Response:** ACCEPTED
**Action taken:** Inserted step 4f in R40 data flow: `reduce_all(net') -> net'' (SPEC-03, SPEC-05 R15-R18)`. Updated the termination check to reference `net''`. Added a note below the data flow explaining why this step is critical (border redex completeness, D3, P3). Also added `InvokeMergeAndReduce` action to `CoordinatorAction` enum that encapsulates both merge and reduce_all as a single unit, with `MergeComplete` event reporting `is_normal_form: bool` after completion.
**Spec sections modified:** Section 3.5 (R20 -- CoordinatorAction), Section 3.10 (R40)

### SC-004: Termination condition uses border_redex_count instead of is_reduced
**Response:** ACCEPTED
**Action taken:** Changed `MergeComplete { net: Net, border_redex_count: usize }` to `MergeComplete { net: Net, is_normal_form: bool }`. The `is_normal_form` field reflects whether the merged-and-reduced net (after `reduce_all` per SPEC-05 R15) has an empty redex queue. The transition table now goes from `Merging -> MergeComplete -> CheckTermination`, and `CheckTermination` checks `is_normal_form` to decide between `Done` (true) and `Partitioning` (false). This is consistent with SPEC-05 R27 ("the net is in Normal Form, redex queue empty after reduce_all"). The `InvokeMergeAndReduce` action performs both merge and reduce_all, so by the time `MergeComplete` fires, the net is fully locally reduced -- `is_normal_form` captures the complete termination condition.
**Spec sections modified:** Section 3.5 (R20 -- CoordinatorEvent, R21 -- transition table)

### SC-005: CLI subcommand count and names conflict with SPEC-07
**Response:** PARTIALLY ACCEPTED
**Action taken:** The critic correctly identified that `local` and `reduce` are semantically different operations. Both subcommands are now present in the CLI enum:
- `Local(LocalArgs)` -- runs the full BSP grid cycle in-process with ChannelTransport (SPEC-07 R18), used for SPEC-09 benchmarks and integration tests
- `Reduce(ReduceArgs)` -- calls `reduce_all` directly without partitioning or merge, used as the sequential baseline and for G1 verification

Added R45a requiring the `local` subcommand to accept SPEC-07 R5's arguments. Added R41a requiring the `local` mode result to be isomorphic to the `reduce` result (G1). Updated the system diagram and section 6.1 to reflect 7 subcommands. Added an explanatory note below the CLI enum documenting the distinction.

The fix differs from the critic's suggestion in that SPEC-13 R43 now explicitly states it "adds `reduce`, `inspect`, and `compute` to the original 4 subcommands from SPEC-07 R1 without removing any," rather than stating a total count of 7 in isolation. This makes the relationship between specs clear.
**Spec sections modified:** Section 3.10 (R41, added R41a), Section 3.11 (R43, added R45a), Section 4.1 (system diagram), Section 6.1 (table)

### SC-006: Worker FSM ConnectionLost transition contradicts SPEC-06 abort policy
**Response:** ACCEPTED
**Action taken:** Changed the worker's `ConnectionLost` transition from `Any | ConnectionLost | Init | AttemptReconnect, LogTransition` to `Any | ConnectionLost | Error | LogTransition, ShutdownSelf`. Added an explanatory note: the worker does NOT attempt to reconnect because the coordinator has already aborted per SPEC-06 R25. Reconnection is a fault tolerance concern (Z5, out of scope for v1).
**Spec sections modified:** Section 3.6 (R25 -- transition table and added note)

### SC-007: CheckTermination state has no entry in the transition table
**Response:** ACCEPTED
**Action taken:** Adopted option (a) from the critic. Inserted `CheckTermination` into the transition table:
- `Merging | MergeComplete(net, _) | CheckTermination | LogTransition`
- `CheckTermination | [is_normal_form == true] | Done | WriteOutput, ShutdownAll, LogTransition`
- `CheckTermination | [is_normal_form == false] | Partitioning | InvokeSplit, LogTransition`

This makes the termination check explicit and eliminates the dead state. Combined with SC-004's fix, the `CheckTermination` state inspects the `is_normal_form` field from the `MergeComplete` event to decide whether to terminate or continue.
**Spec sections modified:** Section 3.5 (R21 -- transition table)

### SC-008: Undefined types in CoordinatorAction: TimerId, MetricEvent, WorkerHandle
**Response:** ACCEPTED
**Action taken:** Added type definitions to Section 4 (Design), within the R20 code block:
- `type TimerId = u32;`
- `struct WorkerHandle { id: WorkerId, transport: Box<dyn Transport> }`

`MetricEvent` was removed from `CoordinatorAction` entirely. The `EmitMetric(MetricEvent)` action variant was removed because: (a) metrics collection is an implementation concern handled by the observability module, not an FSM action, and (b) SPEC-11 (Observability) is the appropriate spec for defining metric types. Metrics can be collected by the action executor as a side effect of other actions (e.g., recording round duration when processing `MergeComplete`). This avoids defining a type whose variants would be speculative at this level.
**Spec sections modified:** Section 3.5 (R20 -- added WorkerHandle and TimerId definitions, removed EmitMetric)

### SC-009: ChannelTransport bypasses serialization, undermining integration test fidelity
**Response:** PARTIALLY ACCEPTED
**Action taken:** Promoted OQ-5 to a SHOULD requirement (R52): "Relativist SHOULD provide a `SerializingChannelTransport` that wraps `ChannelTransport` and performs a bincode serialize/deserialize round-trip on every `send`/`recv`. At minimum, one integration test per benchmark SHOULD use the serializing variant." OQ-5 is now marked as resolved with a pointer to R52.

The fix differs from the critic's suggestion in severity: the critic proposed upgrading to a requirement, but a SHOULD is more appropriate because (a) the primary purpose of ChannelTransport is fast in-memory testing, (b) serialization correctness is independently tested by SPEC-06 R14's round-trip property test, and (c) mandating serialization in every integration test would slow CI unnecessarily. A single test per benchmark exercising the serializing variant is sufficient to catch derive issues.
**Spec sections modified:** Section 3.13 (added R52), Section 7 (OQ-5 resolved)

### SC-010: R4 is a negative requirement that belongs in v1 exclusions
**Response:** PARTIALLY ACCEPTED
**Action taken:** Split R4 into two requirements: R4 (documentation requirement: "BSP classification MUST be documented in coordinator's module-level documentation") and R4a (negative requirement: "Relativist MUST NOT implement MapReduce or Dataflow"). R4a now includes a cross-reference to R49 (v1 exclusions). The requirements remain in Section 3.1 (Programming Model) rather than being moved to Section 3.12, because the negative requirement is fundamentally about the programming model choice, not about feature scope. The cross-reference to R49 provides traceability without restructuring.
**Spec sections modified:** Section 3.1 (R4 split into R4 and R4a)

### SC-011: async_trait in Transport trait may conflict with Core Layer purity
**Response:** ACCEPTED
**Action taken:** Promoted OQ-1 to a SHOULD requirement (R51): "The implementer SHOULD use Rust's native async traits (stabilized in Rust 1.75+) for the Transport trait if they support `Box<dyn Transport>` dispatch. If native async traits do not support dynamic dispatch for the required use case, `async_trait` is acceptable." OQ-1 is now marked as resolved with a pointer to R51. Note: the `Transport` trait is in the Infrastructure Layer (`protocol` module), not the Core Layer, so there is no Core Layer purity concern. The issue is purely about compile-time dependency weight.
**Spec sections modified:** Section 3.13 (added R51), Section 7 (OQ-1 resolved)

### SC-012: Worker FSM lacks HeartbeatTimeout handling
**Response:** ACCEPTED
**Action taken:** Renamed `HeartbeatTimeout(WorkerId)` to `PhaseTimeout(WorkerId)` in `CoordinatorEvent`. Added a doc comment: "Phase-level inactivity timeout: no PartitionResult received from this worker within the configured `collect_timeout` (SPEC-06 R30-R31)." The transition table now uses `PhaseTimeout(id)` instead of `HeartbeatTimeout(id)`. This aligns with SPEC-06 R30-R31's phase timeout terminology and clarifies that this is a coordinator-side inactivity timeout, not a heartbeat protocol. The worker FSM does not need a corresponding heartbeat mechanism.
**Spec sections modified:** Section 3.5 (R20 -- CoordinatorEvent, R21 -- transition table)

### SC-013: R38 specifies default features as empty but tokio is always-on
**Response:** ACCEPTED
**Action taken:** Rephrased R38 to: "The `default` Cargo feature set MUST be empty (`default = []` in `Cargo.toml`). All always-on dependencies from R11 are unconditional and do not require feature gates." This clarifies that "default features" refers specifically to `[features] default = []` in Cargo.toml, not to the base dependency set.
**Spec sections modified:** Section 3.9 (R38)

### SC-014: CoordinatorEvent::SplitComplete but split is synchronous
**Response:** ACCEPTED
**Action taken:** Added `InvokeSplit { net: Net, num_workers: usize }` and `InvokeMergeAndReduce(Vec<Partition>)` to `CoordinatorAction`. These are actions dispatched by the transition function. The action executor invokes `split()` and `merge()+reduce_all()` as (possibly blocking) operations and fires `SplitComplete`/`MergeComplete` events back into the FSM. Added doc comments on `CoordinatorAction` explaining this pattern. Updated the coordinator event loop pseudocode (Section 4.2) to include `blocking_task_complete()` as a select branch and added an explanatory paragraph about preserving stimulus-response purity. This cleanly separates the pure transition function from CPU-bound side effects.
**Spec sections modified:** Section 3.5 (R20 -- CoordinatorAction, R21 -- transition table), Section 4.2

### SC-015: run_grid_local function signature inconsistent with SPEC-05
**Response:** ACCEPTED
**Action taken:** Changed `run_grid_local` signature in Section 4.5 to use `u32` for `num_workers` (matching SPEC-05 R25) and added a `strategy: impl PartitionStrategy` parameter. Kept the `Result<..., RelativistError>` wrapper because the async in-memory grid mode can genuinely fail (e.g., reduction error, merge error). SPEC-05's `run_grid` returning bare `(Net, GridMetrics)` is a local/sync function; the async wrapper naturally introduces fallibility from the tokio runtime.
**Spec sections modified:** Section 4.5

### SC-016: No requirement for coordinator to run reduce_all after merge
**Response:** NOT ADDRESSED (covered by SC-003, SC-004, and SC-007)
**Action taken:** No separate change needed. SC-003 added `reduce_all` to the data flow (R40). SC-004 changed the termination condition to `is_normal_form`. SC-007 wired `CheckTermination` into the transition table. SC-014 added `InvokeMergeAndReduce` as an action that encapsulates merge + reduce_all. The "Note on Merging state" explicitly documents that the `Merging` state encompasses both merge and post-merge `reduce_all`. The FSM is now complete with respect to this requirement. This issue is fully resolved by the combined effect of SC-003, SC-004, SC-007, and SC-014 -- no additional change is needed.
**Spec sections modified:** (none -- covered by other issues)

---

## Changes Made to SPEC-13

### Header
- Status changed from "Draft v1" to "Revised v2"

### Section 3.1 (Programming Model)
- R4 split into R4 (documentation requirement) and R4a (negative requirement with cross-reference to R49)

### Section 3.5 (Coordinator FSM)
- R19: Added supersession note explicitly declaring SPEC-13 R19-R22 as authoritative over SPEC-06 R26-R28, with state-by-state mapping
- R19: Updated doc comments on `Merging` state to mention reduce_all (SPEC-05 R15-R18) and on `CheckTermination` state to clarify decision logic
- R20: Added `WorkerHandle` struct definition and `type TimerId = u32` definition
- R20: Renamed `WorkerRegistered(WorkerId)` to `WorkerConnected(WorkerId)` in CoordinatorEvent
- R20: Renamed `HeartbeatTimeout(WorkerId)` to `PhaseTimeout(WorkerId)` with doc comment referencing SPEC-06 R30-R31
- R20: Changed `MergeComplete { net: Net, border_redex_count: usize }` to `MergeComplete { net: Net, is_normal_form: bool }`
- R20: Removed `EmitMetric(MetricEvent)` from CoordinatorAction
- R20: Added `InvokeSplit { net: Net, num_workers: usize }` and `InvokeMergeAndReduce(Vec<Partition>)` to CoordinatorAction with doc comments explaining stimulus-response purity
- R21: Complete rewrite of transition table:
  - Removed `SendMessage(id, RegisterAck)` from WaitingForWorkers transitions
  - Added `InvokeSplit` action to WaitingForWorkers-to-Partitioning transition
  - Changed timer name from `round_timer` to `collect_timer`
  - Replaced `HeartbeatTimeout` with `PhaseTimeout`
  - Added `Merging -> MergeComplete -> CheckTermination` transition
  - Added `CheckTermination -> [is_normal_form == true] -> Done` transition
  - Added `CheckTermination -> [is_normal_form == false] -> Partitioning` transition
  - Added `InvokeMergeAndReduce` and `InvokeSplit` actions to appropriate rows
- R21: Added notes on implicit worker registration and Merging state scope

### Section 3.6 (Worker FSM)
- R25: Changed `Init | Connected | Idle | SendMessage(Register)` to `Init | Connected | Idle | LogTransition`
- R25: Changed `Any | ConnectionLost | Init | AttemptReconnect, LogTransition` to `Any | ConnectionLost | Error | LogTransition, ShutdownSelf`
- R25: Added notes on ConnectionLost behavior and implicit registration

### Section 3.7 (Transport Abstraction)
- R31: No change to text, but context updated by R52 (SerializingChannelTransport)

### Section 3.9 (Feature Flags)
- R38: Rephrased to explicitly reference `default = []` in Cargo.toml and clarify that always-on dependencies are unconditional

### Section 3.10 (Data Flow)
- R40: Updated step 3 to remove Register/RegisterAck, describe implicit connection-based registration
- R40: Inserted step 4f (`reduce_all(net') -> net''`) between merge and termination check
- R40: Updated termination check to reference `net''` instead of `net'`
- R40: Added explanatory note about why step 4f is critical (D3, P3)
- R41: Clarified as "direct reduction mode" with explicit mention of "no partitioning"
- R41a: Added new requirement for in-memory grid mode (`relativist local`) with G1 reference

### Section 3.11 (CLI Design)
- R43: Added `Local(LocalArgs)` subcommand to CLI enum, bringing total to 7 subcommands
- R43: Added note explicitly stating SPEC-13 adds 3 subcommands to SPEC-07's original 4
- R43: Added explanatory note on `local` vs `reduce` semantic distinction
- R45a: Added new requirement for `local` subcommand arguments (references SPEC-07 R5)
- R46: Clarified "no partitioning, merging, or network communication"

### Section 3.13 (Added -- Additional Requirements from review)
- R51: SHOULD use native async traits for Transport if they support Box<dyn Transport>
- R52: SHOULD provide SerializingChannelTransport for integration test fidelity

### Section 4.1 (System Diagram)
- Updated CLI bar to include `local` subcommand

### Section 4.2 (Coordinator Event Loop)
- Updated pseudocode: `WorkerRegistered` -> `WorkerConnected`, `HeartbeatTimeout` -> `PhaseTimeout`
- Added `blocking_task_complete()` as a select branch
- Added comment about InvokeSplit/InvokeMergeAndReduce
- Added explanatory paragraph about stimulus-response purity for split/merge

### Section 4.3 (Worker Main Loop)
- Removed `transport.send(&Message::Register {...})` call
- Added comment about implicit registration
- Changed `Message::ReturnPartition` to `Message::PartitionResult` (consistent with SPEC-06 R3)

### Section 4.5 (In-Memory Grid Mode)
- Changed `run_grid_local` signature: `num_workers: usize` -> `num_workers: u32`, added `strategy: impl PartitionStrategy`

### Section 6.1 (Architecture comparison table)
- Changed "clap with 5 subcommands" to "clap with 7 subcommands"

### Section 7 (Open Questions)
- OQ-1: Marked as resolved, promoted to R51
- OQ-5: Marked as resolved, promoted to R52

---

## Residual Risks

### SC-016 (NOT ADDRESSED -- but fully covered)

SC-016 identified that the FSM lacked a reduce_all step after merge. This was independently and completely resolved by the combined fixes for SC-003 (data flow), SC-004 (termination condition), SC-007 (CheckTermination state), and SC-014 (InvokeMergeAndReduce action). The "Note on Merging state" in R21 explicitly documents that Merging encompasses merge + reduce_all. There is no residual risk.

### Cross-spec consistency note

This revision introduces terminology and FSM definitions that supersede parts of SPEC-06 (R26-R28) and extend SPEC-07 (R1, adding 3 subcommands). SPEC-06 and SPEC-07 have not been modified (they are outside this spec's territory). The implementer MUST treat SPEC-13 as authoritative where it explicitly supersedes predecessor specs. A future consistency pass across all specs (SPEC-06, SPEC-07, SPEC-08) may be beneficial to update cross-references, but is not blocking for implementation since SPEC-13 clearly declares which predecessor requirements it overrides.
