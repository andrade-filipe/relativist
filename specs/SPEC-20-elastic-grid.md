# SPEC-20: Elastic Grid

**Status:** Draft
**Depends on:** SPEC-01 (Invariants), SPEC-04 (Partitioning), SPEC-05 (Merge and Grid Cycle), SPEC-06 (Wire Protocol), SPEC-13 (System Architecture)
**ROADMAP items:** 2.1 (Coordinator as Worker), 2.2 (Dynamic Worker Joining), 2.3 (Dynamic Worker Departure)
**References consumed:** REF-002 (Lafont 1997), REF-005 (Mackie & Pinto 2002), REF-017 (Foster — grid resource dynamics)
**Arguments consumed:** ARG-001 (central argument, P1-P6), ARG-002 (partitioning preserves structure, C1-C3), ARG-004 (practical viability, Passo 12: re-execution under confluence)
**Briefings consumed:** BRIEF-20260415-v2-codebase-assessment (Sections 4.2-4.3), BRIEF-20260415-v2-fundamentacao-teorica (Tier 2 Elastic Grid)

---

## 1. Purpose

This spec defines the Elastic Grid architecture for Relativist v2: three features that allow the set of participating nodes to change dynamically during a distributed reduction. In v1, the worker count is fixed at startup (SPEC-06, R24; SPEC-13, R21) and the coordinator performs only orchestration. SPEC-20 removes both rigidities:

1. **Coordinator as Worker (2.1):** The coordinator keeps one partition for itself and reduces it locally, increasing effective parallelism from K to K+1 and enabling single-machine operation without idle waiting.
2. **Dynamic Worker Joining (2.2):** New workers can connect between BSP rounds and receive partitions in the next round, scaling up without restart.
3. **Dynamic Worker Departure (2.3):** Workers can leave gracefully or be detected as failed via timeout, with their work reclaimed and redistributed among remaining nodes.

All three features are **confluence-enabled**: they are correct exclusively because strong confluence (SPEC-01, T4; ARG-001, P1) guarantees that the result of reduction is identical regardless of who reduces what and in what order (ARG-001, Passo 4: "the ORDER and DISTRIBUTION of work are irrelevant").

---

## 2. Definitions

Terms defined in SPEC-00 (Glossary), SPEC-01, SPEC-04, SPEC-05, SPEC-06, and SPEC-13 are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **Hybrid Node** | A coordinator that also participates in reduction by keeping one partition for itself (2.1). The coordinator alternates between its orchestration role (dispatch, collect, merge) and its worker role (local `reduce_all`). |
| **Self-Partition** | The partition retained by the coordinator in hybrid mode. It is reduced locally by the coordinator during the same BSP phase in which remote workers reduce their partitions. |
| **Effective Worker Count** | The total number of nodes performing reduction in a given round. In v1, this equals K (the number of remote workers). In v2 hybrid mode, this equals K+1 (K remote workers + 1 coordinator self-partition). Denoted `K_eff`. |
| **Elastic Membership** | The ability for the set of participating workers to grow (joining) or shrink (departure) between BSP rounds without restarting the reduction. |
| **Join Window** | The interval between the end of a merge/check-termination phase and the start of the next partition/dispatch phase, during which the coordinator accepts new worker connections. |
| **Departure** | The removal of a worker from the active set, either by graceful request (`LeaveRequest`) or by timeout detection. |
| **Graceful Departure** | A worker-initiated departure: the worker sends a `LeaveRequest` message and the coordinator removes it from the active set at the next round boundary. |
| **Timeout Departure** | A coordinator-initiated departure: the coordinator detects that a worker has not responded within `collect_timeout` (SPEC-06, R30) and treats it as departed. |
| **Retained Partition** | A copy of the partition sent to each worker, held by the coordinator in memory, to enable re-dispatch upon worker departure. Only maintained when departure detection is enabled. |
| **Active Worker Set** | The set of workers currently participating in reduction. Denoted `W_active`. This set changes between rounds as workers join or depart. |

---

## 3. Requirements

### 3.1 Coordinator as Worker (Hybrid Node) — ROADMAP 2.1

**R1.** When operating in hybrid mode (`GridConfig.hybrid_coordinator: bool`, default `true`), the coordinator MUST retain one partition for itself (the self-partition) and reduce it locally using `reduce_all` (SPEC-03) during the same BSP round in which remote workers reduce their partitions. **(MUST)**

**R2.** The coordinator MUST partition the net for `K_eff = K + 1` nodes when in hybrid mode, where K is the number of remote workers. The self-partition MUST be assigned `worker_id = 0`, and remote workers MUST receive `worker_id` values in `[1, K]`. **(MUST)**

**R3.** The coordinator MUST NOT block on its own local reduction while dispatching partitions to or collecting results from remote workers. The self-partition reduction MUST be performed concurrently with the await of `PartitionResult` messages from remote workers. **(MUST)**

- **Implementation note:** The coordinator spawns `reduce_all(self_partition)` as a `tokio::task::spawn_blocking` task (since `reduce_all` is a blocking CPU-bound operation in the pure Core layer) and awaits it concurrently with the `PartitionResult` collection futures from remote workers.

**R4.** After all remote workers have returned their `PartitionResult` messages and the coordinator's local reduction has completed, the coordinator MUST merge all partitions (self-partition + K remote partitions) using the standard `merge()` function (SPEC-05, R1-R11). The self-partition MUST be treated identically to any remote partition during merge: same `free_port_index` reconstruction (SPEC-05, R20-R23), same border reconnection, same invariant checks. **(MUST)**

**R5.** When `K == 0` (no remote workers), the coordinator MUST reduce the entire net locally as if `run_grid` were called with `num_workers = 1`. This is the degenerate case where the coordinator functions as a standalone reducer. No split/merge overhead is incurred (SPEC-04, R2: trivial case). **(MUST)**

**R6.** The `K == 0` case MUST be the default behavior when no workers connect within a configurable initial wait period (`initial_wait_timeout: Duration`, default: 30 seconds). After this timeout, if `hybrid_coordinator = true` and no workers have connected, the coordinator SHOULD begin reducing alone and accept workers joining later (see R14). If `hybrid_coordinator = false`, the coordinator MUST continue waiting until `worker_connect_timeout` (SPEC-06, R24) expires or the minimum worker count is reached. **(MUST for timeout behavior; SHOULD for transition to solo reduction)**

**R7.** The `GridMetrics` (SPEC-05, R34-R37) MUST distinguish coordinator-local reduction from worker reduction. The coordinator's local reduction time and interaction count MUST be recorded as a separate entry in `worker_stats_per_round` with `worker_id = 0`. **(MUST)**

**R8.** The coordinator self-partition MUST receive an ID range (SPEC-04, R16-R19) computed by `compute_id_ranges(K_eff, net.next_id)` for `worker_id = 0`, ensuring no ID collisions with remote workers (SPEC-01, D4). **(MUST)**

### 3.2 Dynamic Worker Joining — ROADMAP 2.2

**R9.** Between BSP rounds, the coordinator MUST accept new TCP connections from workers that were not present at the start of the current grid session. **(MUST)**

**R10.** The join window MUST be defined as the interval between the completion of the merge/check-termination phase and the start of the next split/dispatch phase. During this window, the coordinator MUST process pending connection requests and register new workers into the active worker set. **(MUST)**

**R11.** A joining worker MUST complete the registration handshake (SPEC-06, R2a; SPEC-10 if auth is enabled) before being included in the active worker set. The coordinator MUST assign a unique `WorkerId` to the joining worker that does not collide with any currently active or previously used `WorkerId` in this session. **(MUST)**

- **Implementation note:** `WorkerId` values are assigned monotonically from a coordinator-local counter. A departed worker's ID is never reused within the same session to prevent confusion in logs and metrics.

**R12.** When new workers join, the coordinator MUST re-partition the net for `K_eff_new = K_new + (1 if hybrid_coordinator)` nodes at the start of the next round. The re-partition uses the standard `split()` function (SPEC-04, R1) with the updated worker count. **(MUST)**

**R13.** ID ranges MUST be recomputed for all `K_eff_new` nodes using `compute_id_ranges(K_eff_new, merged_net.next_id)`. The existing `compute_id_ranges` function already accepts `num_workers` dynamically (BRIEF-20260415-v2-codebase-assessment, Section 4.3). **(MUST)**

**R14.** A joining worker MUST receive a full partition via `AssignPartition` (SPEC-06, R2), identical in format to any other worker's partition. The joining worker has no special state requirements; it is a fresh participant that reduces whatever partition it receives. **(MUST)**

**R15.** When no workers are initially connected and the coordinator is reducing alone (R5-R6), a worker joining mid-execution MUST cause the coordinator to switch from solo mode to grid mode at the next round boundary. The coordinator MUST partition the net (which it has been reducing alone) for `K_eff = 2` and dispatch one partition to the new worker while keeping the self-partition. **(MUST)**

**R16.** Workers that connect during an active round (after dispatch but before merge completion) MUST be queued and included only at the next join window. They MUST NOT receive partitions mid-round. **(MUST)**

- **Justification:** The BSP barrier synchronization model (SPEC-05) requires all workers to operate on partitions from the same split. Injecting a worker mid-round would violate the barrier.

**R17.** The coordinator SHOULD log each worker join event at `INFO` level, including the new `K_eff`, the joining worker's `WorkerId`, and the round number at which it will first participate. **(SHOULD)**

### 3.3 Dynamic Worker Departure — ROADMAP 2.3

#### 3.3.1 Timeout Detection

**R18.** The coordinator MUST detect worker departure via the existing `collect_timeout` mechanism (SPEC-06, R30-R31). If a worker does not return a `PartitionResult` within `collect_timeout`, the coordinator MUST treat it as departed rather than as a fatal error (superseding v1 behavior in SPEC-06, R25 and SPEC-13, R21 `PhaseTimeout` transition to `Error`). **(MUST)**

- **Migration note:** In v1, `PhaseTimeout` transitions to `Error` and aborts execution. SPEC-20 changes this: `PhaseTimeout` transitions to a new recovery path (see Section 4.1, FSM extensions). The v1 behavior is preserved when `elastic_departure = false` (default for v1 compatibility).

**R19.** Additionally, if the TCP connection to a worker is closed unexpectedly (I/O error during send or recv), the coordinator MUST treat that worker as departed immediately, without waiting for `collect_timeout`. **(MUST)**

#### 3.3.2 Graceful Departure

**R20.** A worker MAY send a `LeaveRequest` message to the coordinator to indicate that it wishes to depart. The coordinator MUST acknowledge receipt and remove the worker from the active set at the next round boundary. **(MUST for coordinator handling; MAY for worker sending)**

**R21.** The `LeaveRequest` message MUST be a new variant appended to the `Message` enum (SPEC-06, R5: discriminant stability). **(MUST)**

```
LeaveRequest { worker_id: WorkerId }
```

**R22.** A worker that sends `LeaveRequest` MUST first complete any in-progress reduction and return its `PartitionResult` for the current round before sending `LeaveRequest`. If the worker cannot complete its round, it SHOULD send `LeaveRequest` without `PartitionResult`; the coordinator treats this as a timeout departure for the current round and a graceful departure for future rounds. **(MUST for completing current round; SHOULD for incomplete departure)**

#### 3.3.3 Partition Retention and Re-dispatch

**R23.** When `GridConfig.retain_partitions: bool` is `true` (default `true` when `elastic_departure = true`), the coordinator MUST retain a copy of each partition dispatched to a remote worker. This retained copy is the pre-reduction state of the partition. **(MUST)**

**R24.** Upon detecting a worker departure (timeout or connection loss), the coordinator MUST reclaim the departed worker's partition using the retained copy. The departed worker may have partially reduced its partition, but since the coordinator retains the original, re-dispatch from the original state is safe under strong confluence (P1; ARG-004, Passo 12: "re-execution of lost work is safe because reducing any valid sub-net from any valid intermediate state converges to the same normal form"). **(MUST)**

**R25.** After reclaiming a departed worker's partition, the coordinator MUST re-partition all work for `K_eff - 1` nodes at the next round. The reclaimed partition is merged back into the coordinator's net (or included as an additional partition in the merge) and then redistributed via `split()` with the reduced worker count. **(MUST)**

**R26.** If multiple workers depart in the same round, the coordinator MUST handle all departures collectively: reclaim all departed workers' partitions, merge them all, and re-partition for the remaining `K_eff - D` nodes (where D is the number of departed workers). **(MUST)**

**R27.** If all remote workers depart and the coordinator is in hybrid mode, it MUST fall back to solo reduction (R5). **(MUST)**

**R28.** The coordinator SHOULD log each departure event at `WARN` level, including the departed worker's `WorkerId`, the departure type (timeout, connection loss, or graceful), and the round number. **(SHOULD)**

#### 3.3.4 At-Least-Once Semantics

**R29.** The departure recovery strategy implements at-least-once semantics for partition reduction. A departed worker may have partially or fully reduced its partition before failing. The coordinator re-dispatches the original (unreduced) partition, which means some reductions may be performed twice across the system's history. Strong confluence (P1) guarantees that this redundancy does not affect correctness: reducing a net that has already been partially reduced produces the same normal form as reducing the original net (ARG-001, Passo 4; ARG-004, Passo 12). **(informative)**

**R30.** At-least-once semantics MUST NOT introduce ID collisions. When a departed worker's original partition is re-dispatched, ID ranges MUST be recomputed via `compute_id_ranges(K_eff_new, merged_net.next_id)` for the new effective worker count. Agents created by the departed worker (in the unreturned partition) are lost with the worker; the re-dispatch starts from the retained pre-reduction copy, whose agents have IDs in the original ID range. Since the departed worker's results are discarded entirely (never merged), there is no risk of ID collision from partially-created agents. **(MUST)**

#### 3.3.5 Memory Cost

**R31.** Partition retention (R23) incurs a memory cost of O(sum(|P_i|)) where |P_i| is the size of partition i. This is bounded by the size of the original net (since partitions are disjoint subsets of the net by D1a/C1). The coordinator SHOULD release retained partitions as soon as the corresponding `PartitionResult` is received, keeping only unacknowledged partitions in memory. **(SHOULD)**

**R32.** The `GridConfig` MUST provide `retain_partitions: bool` to allow users to disable partition retention when departure detection is not needed (e.g., trusted LAN environments). When `retain_partitions = false`, departure detection MUST be disabled and `PhaseTimeout` MUST revert to v1 fatal error behavior. **(MUST)**

### 3.4 Configuration

**R33.** The `GridConfig` struct (SPEC-05, `merge/types.rs`) MUST be extended with the following fields:

```rust
pub struct GridConfig {
    // ... existing fields (num_workers, max_rounds, strict_bsp) ...

    /// Enable hybrid mode: coordinator reduces one partition locally.
    /// Default: true.
    pub hybrid_coordinator: bool,

    /// Enable elastic departure: departed workers trigger re-dispatch
    /// instead of fatal error. Default: false (v1 compatibility).
    pub elastic_departure: bool,

    /// Retain pre-reduction copies of dispatched partitions for
    /// re-dispatch on worker departure. Default: true when
    /// elastic_departure is true.
    pub retain_partitions: bool,

    /// Time to wait for initial worker connections before the
    /// coordinator begins solo reduction (hybrid mode only).
    /// Default: 30 seconds.
    pub initial_wait_timeout: Duration,
}
```
**(MUST)**

**R34.** CLI arguments (SPEC-07, `clap`) MUST expose the new configuration fields:
- `--hybrid` / `--no-hybrid` (default: `--hybrid`)
- `--elastic-departure` / `--no-elastic-departure` (default: `--no-elastic-departure`)
- `--retain-partitions` / `--no-retain-partitions` (default: follows `elastic_departure`)
- `--initial-wait-timeout <SECONDS>` (default: 30)
**(MUST)**

### 3.5 Wire Protocol Extensions

**R35.** The `Message` enum (SPEC-06, R1-R5) MUST be extended with the following variant, appended at the end to preserve discriminant stability:

```rust
/// Worker -> Coordinator: request to leave the grid gracefully.
/// Sent after the worker completes its current round.
/// Discriminant: next available after existing variants.
LeaveRequest { worker_id: WorkerId },
```
**(MUST)**

**R36.** The `LeaveRequest` message MUST be serializable and deserializable via serde + bincode, consistent with SPEC-06, R4. **(MUST)**

**R37.** No new coordinator-to-worker message variants are required for SPEC-20. The existing `AssignPartition` and `Shutdown` messages are sufficient: joining workers receive `AssignPartition`; the coordinator sends `Shutdown` when it no longer needs a worker (or at session end). **(informative)**

### 3.6 Metrics Extensions

**R38.** The `GridMetrics` struct (SPEC-05, R34-R37) MUST be extended with elastic-grid-specific metrics:

```rust
/// Number of workers that joined between rounds, per round.
pub workers_joined_per_round: Vec<u32>,

/// Number of workers that departed between rounds, per round.
pub workers_departed_per_round: Vec<u32>,

/// Effective worker count (K_eff) at the start of each round.
pub effective_workers_per_round: Vec<u32>,

/// Number of partitions re-dispatched due to worker departure, per round.
pub partitions_redispatched_per_round: Vec<u32>,
```
**(MUST)**

### 3.7 Invariant Preservation

**R39.** All SPEC-01 invariants MUST be preserved under elastic grid operations. Specifically:

- **T1-T7 (Theoretical):** Unaffected. The elastic grid changes who reduces and when, not the reduction rules or net structure. Strong confluence (T4 = P1) is the enabler, not the target.
- **D1 (Split/Merge Identity):** Preserved. Re-partitioning uses the same `split()` function with updated worker count. C1-C3 hold by construction (SPEC-04, R6-R8).
- **D2 (Local Reduction Equivalence):** Preserved. The coordinator's local reduction uses the same `reduce_all` engine as workers.
- **D3 (Border Completeness):** Preserved. Merge and border resolution follow the same protocol (SPEC-05, R12-R19) regardless of which node produced each partition.
- **D4 (ID Uniqueness):** Preserved with attention. `compute_id_ranges()` MUST be called with the updated `K_eff` at each re-partition (R8, R13, R30). The function already accepts `num_workers` dynamically.
- **D5 (Exclusive Ownership):** Preserved. Each agent belongs to exactly one partition at any time. The coordinator's retained copy (R23) is a snapshot, not a live partition; it enters the system only if the worker departs, at which point the worker's live copy is discarded.
- **D6 (Protocol Termination):** Preserved. Each round consumes at least one interaction from the finite total (T7). Changing the worker count between rounds does not add interactions; it only changes how work is distributed. Re-dispatching a retained partition restarts reduction from an earlier state, but the total interaction count remains bounded (ARG-001, P5).
- **G1 (Fundamental Property):** Preserved. `reduce_all(net) ~ extract_result(run_grid(net, n))` holds for any sequence of K_eff values across rounds, because strong confluence guarantees the same normal form regardless of reduction strategy (ARG-001, Passo 4).

**(MUST)**

---

## 4. Design

### 4.1 Coordinator FSM Extensions

The coordinator FSM (SPEC-13, R19-R21) is extended with new transitions for elastic behavior. The existing states are preserved; new behavior is added to existing transitions.

#### 4.1.1 Extended State Enum

```rust
pub enum CoordinatorState {
    // ... all existing states from SPEC-13 R19 ...
    Init,
    WaitingForWorkers,
    Partitioning,
    Dispatching,
    WaitingForResults,
    Merging,
    CheckTermination,
    Done,
    Error,

    // New state for elastic grid:
    /// Accepting new worker connections and processing departures
    /// between BSP rounds. Entered after CheckTermination when
    /// is_normal_form == false and elastic membership is enabled.
    AcceptingMembershipChanges,
}
```

#### 4.1.2 Extended Events

```rust
pub enum CoordinatorEvent {
    // ... all existing events from SPEC-13 R21 ...

    // New events for elastic grid:
    /// A new worker connected during an active session (not during
    /// initial WaitingForWorkers).
    WorkerJoined(WorkerId),

    /// A worker departed gracefully via LeaveRequest.
    WorkerLeft(WorkerId),

    /// A worker's TCP connection was lost unexpectedly.
    WorkerConnectionLost(WorkerId),

    /// The membership change window has closed (timer-based or
    /// immediate if no pending connections).
    MembershipWindowClosed,

    /// Coordinator's local reduction of self-partition completed.
    SelfPartitionReduced(Partition),
}
```

#### 4.1.3 Extended Transition Table

New and modified transitions (additions to SPEC-13, R21):

| From | Event | To | Actions | Condition |
|------|-------|----|---------|-----------|
| Init | ConfigLoaded | WaitingForWorkers | BindListener, StartTimer(initial_wait_timer), LogTransition | `hybrid_coordinator = false` |
| Init | ConfigLoaded | WaitingForWorkers | BindListener, StartTimer(initial_wait_timer), LogTransition | `hybrid_coordinator = true` |
| WaitingForWorkers | InitialWaitTimeout [K=0, hybrid=true] | Partitioning | InvokeSplit(net, 1), LogTransition | Solo mode: coordinator reduces alone |
| WaitingForWorkers | WorkerConnected(id) [count >= min] | Partitioning | CancelTimer(initial_wait_timer), InvokeSplit(net, K_eff), LogTransition | Standard path |
| WaitingForResults | PartitionReturned(id, P) [all remote received, self done] | Merging | CancelTimer, InvokeMergeAndReduce(all_partitions), LogTransition | Hybrid: wait for self-partition too |
| WaitingForResults | SelfPartitionReduced(P) [not all remote] | WaitingForResults | StoreResult(0, P) | Self-partition finished first |
| WaitingForResults | PhaseTimeout(id) [elastic_departure=true] | WaitingForResults | ReclaimPartition(id), RemoveWorker(id), LogDeparture | Reclaim and continue waiting for others |
| WaitingForResults | PhaseTimeout(id) [elastic_departure=false] | Error | LogTransition, ShutdownAll | v1 behavior: fatal |
| WaitingForResults | WorkerConnectionLost(id) [elastic_departure=true] | WaitingForResults | ReclaimPartition(id), RemoveWorker(id), LogDeparture | Immediate departure detection |
| WaitingForResults | WorkerLeft(id) | WaitingForResults | StoreResult(id, ...), RemoveWorker(id), LogDeparture | Graceful: partition already returned |
| CheckTermination | [is_normal_form == false, elastic=true] | AcceptingMembershipChanges | StartTimer(join_window_timer), LogTransition | Allow joins/departures |
| CheckTermination | [is_normal_form == false, elastic=false] | Partitioning | InvokeSplit, LogTransition | v1 path: skip membership window |
| AcceptingMembershipChanges | WorkerJoined(id) | AcceptingMembershipChanges | RegisterWorker(id), LogJoin | Accept new worker |
| AcceptingMembershipChanges | WorkerLeft(id) | AcceptingMembershipChanges | RemoveWorker(id), LogDeparture | Accept graceful departure |
| AcceptingMembershipChanges | MembershipWindowClosed | Partitioning | InvokeSplit(net, K_eff_new), LogTransition | Proceed with updated K_eff |

#### 4.1.4 Hybrid Dispatch Flow

In hybrid mode, the Dispatching state sends partitions to K remote workers and retains the self-partition:

```
Partitioning:
  split(net, K_eff) -> partitions[0..K_eff]
  self_partition = partitions[0]           // kept locally
  remote_partitions = partitions[1..K_eff] // dispatched

Dispatching:
  for each remote_partition:
    send(AssignPartition, worker)
    if retain_partitions: store_retained_copy(worker_id, partition)
  spawn_blocking(reduce_all(self_partition))
  -> AllDispatched

WaitingForResults:
  concurrently:
    await PartitionResult from each remote worker
    await SelfPartitionReduced from local task
  when all K_eff partitions ready:
    -> Merging
```

### 4.2 Re-Partition Algorithm

When the worker count changes between rounds (due to joins or departures), the coordinator re-partitions the merged net.

#### 4.2.1 Worker Joining (K -> K+J)

1. The current round completes normally with K_eff workers.
2. At the `AcceptingMembershipChanges` window, J new workers are registered.
3. The merged net from the completed round is the input to `split()`.
4. `split(merged_net, K_eff + J, strategy)` produces K_eff + J partitions.
5. `compute_id_ranges(K_eff + J, merged_net.next_id)` assigns disjoint ID ranges.
6. Partitions are dispatched to all K_eff + J nodes (self-partition + K + J remote).

No special handling is needed: the merged net is a complete, valid net. Splitting it for a different number of workers is exactly the same operation as the initial split (SPEC-04, R1). Strong confluence guarantees the result is identical to any other valid partition count (ARG-001, Passo 4).

#### 4.2.2 Worker Departure (K -> K-D)

1. During `WaitingForResults`, D workers time out or disconnect.
2. For each departed worker, the coordinator reclaims the retained (pre-reduction) copy of that worker's partition.
3. The coordinator collects results from the remaining K_eff - D workers (including self-partition if hybrid).
4. The merge includes:
   - K_eff - D successfully reduced partitions (from responsive workers).
   - D retained (unreduced) partitions (from departed workers).
5. `merge()` processes all K_eff partitions, producing a valid merged net.
6. The coordinator re-partitions for K_eff - D in the next round.

**Why merging reduced + unreduced partitions is safe:** Strong confluence (T4) guarantees that reducing a subset of redexes in a terminating net produces a valid intermediate state. The retained partitions contain unreduced redexes that were also present in the original net. Merging the partially-advanced workers' results with the unchanged departed workers' partitions produces a net that is a valid intermediate state of the original net's reduction sequence. The subsequent round(s) will reduce the remaining redexes to normal form. This is formally justified by ARG-001, Passo 4: any partition of redexes into subsets, reduced in any order, converges to the unique normal form.

#### 4.2.3 Combined Join and Departure

If workers both join and depart between the same two rounds, the coordinator processes departures first (reclaim partitions) and then computes the new K_eff:

```
K_eff_new = (K_eff - D) + J    // where D = departed, J = joined
         = K_remote_remaining + J_new + (1 if hybrid)
```

### 4.3 Timeout Detection

#### 4.3.1 Round-Level Timeout

The existing `collect_timeout` (SPEC-06, R30; default 600 seconds) serves as the departure detection mechanism. No additional heartbeat protocol is required for SPEC-20.

When `elastic_departure = true`:
- If a worker's `PartitionResult` is not received within `collect_timeout`, the coordinator fires `PhaseTimeout(worker_id)`.
- Instead of transitioning to `Error` (v1), the coordinator reclaims the worker's retained partition and removes the worker from the active set.
- Collection continues for remaining workers. When all responsive workers have returned (or timed out), the coordinator proceeds to merge with whatever partitions are available.

#### 4.3.2 Connection-Level Detection

TCP connection closure is detected immediately via the `tokio` I/O error path. This provides faster departure detection than `collect_timeout` for abrupt failures (process crash, network partition).

### 4.4 Message Flow Diagrams

#### 4.4.1 Normal Round (Hybrid Mode, No Membership Changes)

```
Coordinator                     Worker A             Worker B
    |                              |                     |
    |--- AssignPartition(P_A) ---->|                     |
    |--- AssignPartition(P_B) ---------------------------->|
    |                              |                     |
    | [reduce_all(P_self)]         | [reduce_all(P_A)]  | [reduce_all(P_B)]
    |                              |                     |
    |<-- PartitionResult(P_A') ----|                     |
    |<-- PartitionResult(P_B') ----------------------------|
    | [SelfPartitionReduced(P_self')]                     |
    |                              |                     |
    | [merge(P_self', P_A', P_B')]                       |
    | [reduce_all(merged) -- borders]                    |
    | [check termination]                                |
```

#### 4.4.2 Worker Joining Between Rounds

```
Coordinator                     Worker A          Worker B (new)
    |                              |                   |
    | [round N completes]          |                   |
    | [AcceptingMembershipChanges] |                   |
    |<---------- Register ----------------------------|
    |----------- RegisterAck ----------------------------->|
    | [MembershipWindowClosed]     |                   |
    |                              |                   |
    | [split(net, K_eff=3)]        |                   |
    |--- AssignPartition(P_A) ---->|                   |
    |--- AssignPartition(P_B) --------------------------->|
    | [reduce_all(P_self)]         |                   |
    ...
```

#### 4.4.3 Worker Departure (Timeout)

```
Coordinator                     Worker A          Worker B (fails)
    |                              |                   |
    |--- AssignPartition(P_A) ---->|                   |
    |--- AssignPartition(P_B) --------------------------->|
    | [retain P_B copy]            |                   |
    |                              |                   |
    | [reduce_all(P_self)]         | [reduce_all(P_A)] | [CRASH]
    |                              |                   X
    |<-- PartitionResult(P_A') ----|
    | [collect_timeout for B]
    | [ReclaimPartition(B) -> use retained P_B]
    |
    | [merge(P_self', P_A', P_B)]  // P_B is unreduced original
    | [reduce_all(merged)]
    | [re-partition for K_eff=2 next round]
```

#### 4.4.4 Graceful Departure

```
Coordinator                     Worker A          Worker B (leaving)
    |                              |                   |
    |--- AssignPartition(P_A) ---->|                   |
    |--- AssignPartition(P_B) --------------------------->|
    |                              |                   |
    | [reduce_all(P_self)]         | [reduce_all(P_A)] | [reduce_all(P_B)]
    |                              |                   |
    |<-- PartitionResult(P_A') ----|                   |
    |<-- PartitionResult(P_B') ----------------------------|
    |<-- LeaveRequest(B) ------------------------------|
    |                              |                   |
    | [merge(P_self', P_A', P_B')]                     |
    | [RemoveWorker(B)]                                |
    | [re-partition for K_eff=2 next round]            |
    |--- Shutdown(B) ------------------------------------->|
    |                              |                   [done]
```

---

## 5. Rationale

### 5.1 Why Confluence Makes This Safe

The entire Elastic Grid architecture rests on a single theoretical pillar: strong confluence (SPEC-01, T4; REF-002, Proposition 1; ARG-001, P1).

**Strong confluence guarantees that:**

1. **The result is independent of partition count.** Splitting a net into K partitions or K+1 partitions produces the same normal form after complete reduction (ARG-001, Passo 4). This enables coordinator-as-worker (2.1) and dynamic joining (2.2).

2. **The result is independent of reduction order.** Reducing partition A fully, then partition B partially, then redistributing B's remaining work, produces the same normal form as reducing A and B in parallel (ARG-001, Passo 4; T6). This enables re-dispatch after departure (2.3).

3. **Re-execution is safe.** If a worker's result is lost, re-reducing from the original partition state produces the same contributions to the global reduction (ARG-004, Passo 12). No rollback protocol, consensus algorithm, or distributed transaction is needed.

4. **Total interaction count is invariant.** Regardless of how many workers participate or how work is redistributed, the total number of reduction steps to normal form is the same (T7). Elastic membership changes affect latency (wall-clock time) but not total computational work.

### 5.2 Why No Consensus Is Needed

Traditional distributed systems require consensus protocols (Raft, Paxos) for dynamic membership changes because operations are not commutative or idempotent. In Relativist:

- **Commutativity:** Strong confluence implies that any two disjoint active pairs can be reduced in either order (T4). This is a stronger property than what consensus protocols provide.
- **Idempotency:** Reducing an already-reduced pair is a no-op (the pair no longer exists). Re-dispatching a partition and reducing it again produces the same net as reducing it once, because the redundant reductions simply find no active pairs.
- **Single coordinator:** The coordinator is the sole decision-maker for membership changes. No distributed agreement is needed because only one node makes membership decisions.

### 5.3 Comparison to v1

| Aspect | v1 (SPEC-06/SPEC-13) | v2 (SPEC-20) |
|--------|----------------------|--------------|
| Worker count | Fixed at startup | Dynamic between rounds |
| Coordinator role | Orchestration only | Orchestration + optional reduction |
| Worker departure | Fatal error (abort) | Reclaim and re-dispatch (configurable) |
| Minimum workers | `num_workers` (mandatory) | 0 (coordinator can reduce alone) |
| K_eff | K | K+1 (hybrid) or K (non-hybrid) |
| ID range computation | Once at startup | Recomputed each round if K changes |
| Partition retention | Not retained | Retained when `elastic_departure = true` |

---

## 6. Migration Path

### 6.1 v1 Static Workers to v2 Elastic

The migration is designed for full backward compatibility. Default configuration values reproduce v1 behavior exactly.

**Step 1: Hybrid coordinator (low risk)**

1. Add `hybrid_coordinator: bool` to `GridConfig` (default: `true` for new deployments, but v1 benchmarks set it to `false`).
2. Modify the coordinator's `Dispatching` state to retain one partition and spawn local reduction.
3. Modify `WaitingForResults` to wait for `K_eff` results (K remote + 1 local) instead of K.
4. No wire protocol changes needed.
5. All 690 existing tests pass with `hybrid_coordinator = false`.

**Step 2: Dynamic joining (medium risk)**

1. Add `AcceptingMembershipChanges` state to coordinator FSM.
2. Modify TCP listener to accept connections during grid execution (not just during `WaitingForWorkers`).
3. Keep a monotonic `WorkerId` counter (never reuse IDs).
4. `split()` and `compute_id_ranges()` already accept dynamic `num_workers`; no changes to partition module.
5. Wire protocol unchanged: joining workers use the standard `Register`/`RegisterAck` handshake.

**Step 3: Dynamic departure (medium-high risk)**

1. Add `elastic_departure: bool` and `retain_partitions: bool` to `GridConfig`.
2. Implement partition retention: clone each `Partition` before dispatch.
3. Change `PhaseTimeout` transition from `Error` to recovery path when `elastic_departure = true`.
4. Add `LeaveRequest` variant to `Message` enum (appended at end for discriminant stability).
5. Implement merge of mixed reduced/unreduced partitions (no code change to `merge()` itself; it operates on `Vec<Partition>` regardless of reduction state).

### 6.2 Feature Flags

All three features are independently enableable via `GridConfig`:

| Feature | Config Field | Default | Can disable? |
|---------|-------------|---------|--------------|
| Coordinator as Worker | `hybrid_coordinator` | `true` | Yes |
| Dynamic Joining | Always enabled when hybrid | n/a | Joining workers are simply accepted |
| Dynamic Departure | `elastic_departure` | `false` | Yes (reverts to v1 fatal error) |
| Partition Retention | `retain_partitions` | follows `elastic_departure` | Yes |

---

## 7. Test Strategy

### 7.1 Unit Tests

| ID | Test | Requirement | Description |
|----|------|-------------|-------------|
| EG-U1 | `test_hybrid_coordinator_single_machine` | R1, R5 | Coordinator reduces alone (K=0). Verify normal form matches `reduce_all`. |
| EG-U2 | `test_hybrid_partition_count` | R2 | With K=3 remote workers, verify `split()` produces K_eff=4 partitions. |
| EG-U3 | `test_hybrid_self_partition_id_range` | R8 | Verify self-partition receives `worker_id=0` range from `compute_id_ranges(K_eff, ...)`. |
| EG-U4 | `test_hybrid_merge_includes_self` | R4 | Merge K+1 partitions (self + K remote). Verify result matches sequential `reduce_all`. |
| EG-U5 | `test_dynamic_join_repartition` | R12, R13 | Start with K=2, add 1 worker. Verify next round uses K_eff=4 and ID ranges are disjoint. |
| EG-U6 | `test_dynamic_join_mid_round_queued` | R16 | Worker connecting mid-round is not dispatched until next round. |
| EG-U7 | `test_departure_reclaim_unreduced` | R24, R25 | Simulate 1 of 3 workers departing. Verify retained partition is merged unreduced and result is correct. |
| EG-U8 | `test_departure_multiple_workers` | R26 | Simulate 2 of 4 workers departing simultaneously. Verify correct re-partition for K_eff=3. |
| EG-U9 | `test_departure_all_workers_solo_fallback` | R27 | All remote workers depart. Verify coordinator falls back to solo reduction and reaches normal form. |
| EG-U10 | `test_graceful_leave_after_round` | R20, R22 | Worker sends PartitionResult then LeaveRequest. Verify worker removed and next round has K_eff-1. |
| EG-U11 | `test_join_and_departure_same_round` | Section 4.2.3 | 1 worker joins, 1 departs between same rounds. Verify K_eff computed correctly. |
| EG-U12 | `test_id_ranges_no_collision_after_repartition` | R13, R30 | After K changes, verify all ID ranges are disjoint across all K_eff nodes. |
| EG-U13 | `test_retained_partition_released_on_ack` | R31 | Verify retained partition memory is freed when PartitionResult is received. |

### 7.2 Integration Tests

| ID | Test | Requirement | Description |
|----|------|-------------|-------------|
| EG-I1 | `test_hybrid_grid_correctness` | R1, R4, G1 | Run `run_grid` with hybrid=true for several benchmark nets. Verify `reduce_all(net) ~ extract_result(run_grid(net, K_eff))`. |
| EG-I2 | `test_elastic_join_correctness` | R9-R14, G1 | Start grid with 1 worker, add 2 more after round 1. Verify final result matches `reduce_all`. |
| EG-I3 | `test_elastic_departure_correctness` | R18-R25, G1 | Start grid with 4 workers, simulate 1 departure at round 2. Verify final result matches `reduce_all`. |
| EG-I4 | `test_elastic_churn_correctness` | R9-R30, G1 | Start with 2 workers, add 3, remove 2, add 1 across multiple rounds. Verify final result matches `reduce_all`. |
| EG-I5 | `test_v1_compatibility_mode` | R32 | Run with `hybrid_coordinator=false`, `elastic_departure=false`. Verify behavior identical to v1 `run_grid`. |

### 7.3 Property-Based Tests

| ID | Test | Requirement | Description |
|----|------|-------------|-------------|
| EG-P1 | `prop_hybrid_normal_form_invariant` | G1 | For random terminating nets and random K in [0, 8]: `reduce_all(net) ~ run_grid(net, K, hybrid=true)`. |
| EG-P2 | `prop_departure_normal_form_invariant` | G1 | For random nets and random departure schedules: final result matches `reduce_all`. |
| EG-P3 | `prop_id_ranges_disjoint_after_repartition` | D4 | For random K_eff changes, verify all ID ranges are disjoint. |

### 7.4 Benchmark Tests

| ID | Test | Requirement | Description |
|----|------|-------------|-------------|
| EG-B1 | `bench_hybrid_vs_nonhybrid` | R1 | Compare wall-clock time of hybrid mode vs non-hybrid for the same net and K. Expected: hybrid is faster (K+1 vs K effective workers). |
| EG-B2 | `bench_retention_memory_overhead` | R31 | Measure peak memory with and without partition retention for large nets. |

---

## 8. Open Questions

**OQ-1. Join window duration.** How long should the `AcceptingMembershipChanges` window remain open? Options: (a) fixed timer (e.g., 5 seconds), (b) immediate close if no pending connections, (c) configurable. Current design leans toward (b) with a short timer as fallback. Needs benchmarking.

**OQ-2. Self-partition assignment strategy.** Should the coordinator always take `worker_id=0` (first partition), or should it take the smallest partition to minimize its local reduction time (since it also has orchestration duties)? Current design uses `worker_id=0` for simplicity. A smarter assignment could reduce tail latency.

**OQ-3. Interaction with strict BSP mode.** In strict BSP mode (SPEC-05, R30a), multiple rounds occur naturally. How does elastic membership interact with the strict BSP multi-round loop? Current design: membership changes are processed at every `CheckTermination` boundary, regardless of BSP mode. This means the worker count can change at each strict-BSP round, which is safe under confluence but may complicate benchmark analysis.

**OQ-4. Partition retention vs. checkpoint.** The retained partition strategy (R23) keeps full copies in coordinator memory. For very large nets, this may be prohibitive. An alternative is to checkpoint partitions to disk. This is deferred to ROADMAP 2.9 (Fault Tolerance) and out of scope for SPEC-20.

**OQ-5. Worker ID assignment for re-joined workers.** If a departed worker reconnects (same machine, new process), should it receive a new `WorkerId` or reclaim its old one? Current design: always new ID (R11 note). This is simpler but means metrics cannot correlate pre- and post-departure worker identity.