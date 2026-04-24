# SPEC-20: Elastic Grid

**Status:** Draft — Round 2 (post-SPEC-REVIEW-20-round-1-2026-04-24)
**Depends on:** SPEC-01 (Invariants), SPEC-04 (Partitioning), SPEC-05 (Merge and Grid Cycle), SPEC-06 (Wire Protocol), SPEC-13 (System Architecture), SPEC-17 (Transport Abstraction), SPEC-18 (Wire Format v2), SPEC-19 (Delta Protocol)
**ROADMAP items:** 2.1 (Coordinator as Worker), 2.2 (Dynamic Worker Joining), 2.3 (Dynamic Worker Departure)
**References consumed:** REF-002 (Lafont 1997), REF-005 (Mackie & Pinto 2002), REF-017 (Foster — grid resource dynamics)
**Arguments consumed:** ARG-001 (central argument, P1-P6, including the inductive proof of round equivalence), ARG-002 (partitioning preserves structure, C1-C3), ARG-004 (practical viability, Passo 12: re-execution under confluence), **ARG-006** (mixed-trace recoverability for elastic departure — closes §3.3 R29a / §3.7 R39 for v1 mode; delta mode CONDITIONAL on ARG-005). See `codigo/relativist/docs/theory-bridge.md` for absolute paths.
**Briefings consumed:** BRIEF-20260415-v2-codebase-assessment (Sections 4.2-4.3), BRIEF-20260415-v2-fundamentacao-teorica (Tier 2 Elastic Grid)
**Spec reviews addressed:** SPEC-REVIEW-20-round-1-2026-04-24 (30 findings — see §11 Change Log).

---

## 1. Purpose

This spec defines the Elastic Grid architecture for Relativist v2: three features that allow the set of participating nodes to change dynamically during a distributed reduction. In v1, the worker count is fixed at startup (SPEC-06, R24; SPEC-13, R21) and the coordinator performs only orchestration. SPEC-20 removes both rigidities:

1. **Coordinator as Worker (2.1):** The coordinator keeps one partition for itself and reduces it locally, increasing effective parallelism from K to K+1 and enabling single-machine operation without idle waiting.
2. **Dynamic Worker Joining (2.2):** New workers can connect between BSP rounds and receive partitions in the next round, scaling up without restart.
3. **Dynamic Worker Departure (2.3):** Workers can leave gracefully or be detected as failed via timeout, with their work reclaimed and redistributed among remaining nodes.

All three features compose with **both** the v1 full-merge protocol (SPEC-05/SPEC-06) **and** the v2 delta protocol (SPEC-19). §3.0 specifies the execution-mode matrix; subsequent sections specify behavior per mode.

All three features are **confluence-enabled**: they are correct exclusively because strong confluence (SPEC-01, T4; ARG-001, P1) guarantees that the result of reduction is identical regardless of who reduces what and in what order. The specific recoverability claim required for retained-partition re-dispatch under departure (§3.3) is *not* a direct corollary of ARG-001 Passo 4; it requires an explicit mixed-trace extension whose proof is provided by ARG-006 (mixed-trace recoverability, CLOSED for v1 mode; delta mode CONDITIONAL on ARG-005 via R24a conservative fallback — see §3.3.6 and §3.7).

---

## 2. Definitions

Terms defined in SPEC-00 (Glossary), SPEC-01, SPEC-04, SPEC-05, SPEC-06, SPEC-13, SPEC-17, SPEC-18, and SPEC-19 are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **Hybrid Node** | A coordinator that also participates in reduction by keeping one partition for itself (2.1). The coordinator alternates between its orchestration role (dispatch, collect, merge) and its worker role (local `reduce_all`). |
| **Self-Partition** | The partition retained by the coordinator in hybrid mode. It is reduced locally by the coordinator during the same BSP phase in which remote workers reduce their partitions. The coordinator implements this by spawning a local in-process worker that speaks the same protocol as remote workers via `ChannelTransport` (SPEC-17 R15). |
| **Effective Worker Count** | The total number of nodes performing reduction in a given round. In v1 non-hybrid mode, this equals K (the number of remote workers). In hybrid mode, this equals K+1 (K remote workers + 1 coordinator self-partition). Denoted `K_eff`. |
| **Active Worker Set (`W_active`)** | The set of `WorkerId` values currently participating in reduction. Sparse: WorkerIds may be retired and never reused. |
| **Active Worker Slot (`partition_index`)** | A worker's *position* in the round's `ActiveWorkerSet` sorted by `WorkerId`, in `[0, K_eff)`. Distinct from `WorkerId`. ID-range computation (SPEC-04 R16-R19) consumes `partition_index`, NOT `WorkerId` (D4-elastic; closes SC-006). |
| **Elastic Membership** | The ability for the set of participating workers to grow (joining) or shrink (departure) between BSP rounds without restarting the reduction. |
| **Join Window** | The interval between the end of a merge/check-termination phase and the start of the next partition/dispatch phase, during which the coordinator drains pending worker connections and applies graceful departures. Bounded by `[join_window_min, join_window_max]` (R10a). |
| **Departure** | The removal of a worker from `W_active`, by graceful request (`LeaveRequest`) or by timeout/connection loss detection. |
| **Graceful Departure (clean)** | A worker-initiated departure where the worker first returns its `PartitionResult` (v1 mode) or `RoundResult` (delta mode) for the current round and *then* sends `LeaveRequest{kind: AfterResult}`. R22a. |
| **Graceful Departure (urgent)** | A worker-initiated departure where the worker sends `LeaveRequest{kind: Urgent}` *without* returning a current-round result, signaling that it cannot complete the in-flight round. R22b. The coordinator treats this as a timeout for the current round and a graceful departure for future rounds. |
| **Timeout Departure** | A coordinator-initiated departure: the coordinator detects that a worker has not responded within `collect_timeout` (SPEC-06, R30). |
| **Connection-Loss Departure** | A coordinator-initiated departure detected immediately via TCP `accept`/`recv` I/O error on the worker's stream, faster than `collect_timeout`. |
| **`retained_initial`** | The round-0 `Partition` (v1) or `InitialPartition.partition` (delta) snapshot of a worker, held by the coordinator. Used ONLY for catastrophic departure that occurs before any `PartitionResult`/`RoundResult` is received (R23b, R24a). |
| **`retained_last_acked`** | The most recently committed worker state. In v1 mode, this is the latest `PartitionResult.partition` received from the worker. In delta mode, this is `(border_graph_snapshot_at_round_n_1, last_round_result_deltas)` — sufficient to reconstruct the worker's contribution as of the last successful round. Used for departure that occurs after at least one successful round (R23c, R24b). |
| **Solo Mode (`SoloReducing` state)** | A coordinator-only execution path entered when no remote workers are connected and `hybrid_coordinator = true`. The coordinator reduces the entire net via `reduce_n(solo_budget)` in a loop, polling the async event loop for join events between batches. R5/R5a. |

---

## 3. Requirements

### 3.0 Execution Mode Matrix

SPEC-20 features compose with two pre-existing execution modes: the v1 full-merge protocol (default) and the v2 delta protocol (SPEC-19, opt-in via `GridConfig.delta_mode`). Each elastic feature MUST behave correctly under both, and the `strict_bsp` orthogonal flag (SPEC-05 R30a, SPEC-19 R30/R40) MUST not change SPEC-20 semantics other than the round-count bound. Closes **SC-001**.

**M0.** SPEC-20 MUST support exactly the four mode combinations below. Each combination is a supported configuration for v2; mixing semantics (e.g., starting in v1 mode and switching to delta mode mid-run) is OUT OF SCOPE for v2 and explicitly forbidden by R0c. **(MUST)**

| Mode | `delta_mode` | `strict_bsp` | Per-round protocol | SPEC-20 retained-state shape |
|------|:------------:|:------------:|--------------------|------------------------------|
| A. v1-lenient | false | false | SPEC-05 R24-R30 (split, dispatch, reduce, collect, merge, terminate) | `retained_initial = Partition`, `retained_last_acked = PartitionResult.partition` |
| B. v1-strict  | false | true  | SPEC-05 R30a (multi-round full merge until cascade settles) | Same as A |
| C. delta-lenient | true | false | SPEC-19 R20-R30 (`InitialPartition` once, `RoundStart`/`RoundResult` deltas, `FinalStateRequest` at convergence) | `retained_initial = InitialPartition.partition`, `retained_last_acked = (BorderGraph snapshot at round N-1, last RoundResult deltas)` |
| D. delta-strict  | true | true  | SPEC-19 R20-R30 with R40 strict cascade dispatch | Same as C |

**R0a (per-feature mode coverage).**
Each elastic feature MUST be specified per execution mode where its behavior differs. Where the table below says "shared", the requirement text in §3.1-§3.3 applies verbatim to both v1 and delta. Where it says "per-mode", the requirement specifies mode-specific behavior in nested clauses (e.g., R4-v1 vs R4-delta).

| Feature | Specification scope |
|---------|---------------------|
| 2.1 Hybrid Coordinator (§3.1) | Per-mode: §3.1.A covers v1; §3.1.B covers delta. |
| 2.2 Dynamic Joining (§3.2) | Per-mode: §3.2.A covers v1; §3.2.B covers delta (snapshot via `reconstruct(border_graph, partitions)`). |
| 2.3 Dynamic Departure (§3.3) | Per-mode: §3.3.A covers v1; §3.3.B covers delta (retained-state shape differs; recovery flow differs). |
| Wire Protocol (§3.5) | Shared: same `JoinRequest`/`JoinAck`/`LeaveRequest`/`LeaveAck` discriminants, regardless of mode. |
| Configuration (§3.4) | Shared. |
| Metrics (§3.6) | Additive with SPEC-19 metrics in delta mode. |
| Invariants (§3.7) | Shared with mode-specific defense statements. |

**R0b.** When the active mode is delta (C or D), the v1 `merge`-then-redistribute terminology in §3.1-§3.3 MUST be interpreted as the delta equivalents:
- "merge all partitions" (v1) → "apply `BorderGraph.apply_deltas()` and resolve any newly-detected border redexes" (delta, SPEC-19 R11-R15).
- "re-partition the merged net" (v1) → "compute a `PartitionPlan` from `reconstruct(border_graph, worker_partitions)` (SPEC-19 R38) and dispatch new partitions via `InitialPartition`" (delta).
- "retained partition" (v1) → see `retained_initial` / `retained_last_acked` definitions in §2.
- "the coordinator holds the merged net" (v1) → "the coordinator holds the `BorderGraph` plus K (or K_eff) `retained_last_acked` snapshots refreshed every round" (delta).

The pseudocode and sequence diagrams in §4 are normative; the prose in §3.1-§3.3 follows the table.

**R0c (mode immutability per run).** The active execution mode (A/B/C/D) MUST be fixed at the start of a grid run by `GridConfig.delta_mode` and `GridConfig.strict_bsp` and MUST NOT change for the remainder of the run. Switching modes mid-run is forbidden because the retained-state shape would have to migrate (e.g., from `Partition` to `(BorderGraph, deltas)`), and that migration is unspecified and out of scope. **(MUST)**

**R0d (full-rejoin under mode-mismatch).** A worker that connects with a `Register` payload claiming a `protocol_version` different from the coordinator's own `PROTOCOL_VERSION = 4` MUST be rejected with `RegisterNack { reason: ProtocolVersionMismatch }` (consistent with SPEC-19 R37 / NF-002). This prevents accidental v1↔v2 mixing within a session. **(MUST)**

### 3.1 Coordinator as Worker (Hybrid Node) — ROADMAP 2.1

**R1.** When operating in hybrid mode (`GridConfig.hybrid_coordinator: bool`, see R33 for default), the coordinator MUST retain one partition for itself (the *self-partition*) and reduce it locally during the same BSP round in which remote workers reduce their partitions. The reduction MUST proceed via the same primitives used by remote workers: in v1 mode, `reduce_all`; in delta mode, the worker delta loop (SPEC-19 R24). **(MUST)**

**R2.** The coordinator MUST allocate `K_eff = K + 1` slots when in hybrid mode, where K is the number of remote workers in `W_active` at the start of the round. The self-partition is the *role* that the coordinator plays; the corresponding `WorkerId` is reserved as `0` (R7-R7a), and its `partition_index` within the round's slot ordering is also `0` (because WorkerId 0 sorts first among non-negative WorkerIds). **(MUST)**

**R2a (cross-mode worker_id 0 semantics; closes SC-016).**
- In **non-hybrid mode** (`hybrid_coordinator = false`), `WorkerId = 0` refers to the first remote worker, consistent with SPEC-04 R16-R19. The coordinator does NOT participate in reduction.
- In **hybrid mode** (`hybrid_coordinator = true`), `WorkerId = 0` is reserved exclusively for the coordinator self-partition; the first remote worker receives `WorkerId = 1`.

Tests `EG-U1b` (worker_id semantics differ across modes) MUST cover both branches.

**R3 (concurrency model; closes SC-002).** In hybrid mode the coordinator MUST run its self-partition reduction as a local in-process worker that communicates with the coordinator's own event loop via `ChannelTransport` (SPEC-17 R15), so that the self-partition appears to the FSM as just another connected worker. The coordinator's main event loop MUST NOT block on any single arm; it MUST `tokio::select!` over at least the following four arms:

```text
loop {
    tokio::select! {
        // (a) result/round messages from any worker (remote OR self via channel)
        msg = workers.next_message() => fsm.handle(WorkerMessage(id, msg)),
        // (b) timer events (initial_wait, join_window, collect)
        timer_id = timer_wheel.next() => fsm.handle(TimerFired(timer_id)),
        // (c) new TCP accepts (held in a "pending connections" queue per R10b)
        Some(stream) = listener.accept() => pending.push(stream),
        // (d) self-partition task panic propagation
        panic = self_join_handle.panic_signal() => fsm.handle(SelfPartitionPanic(panic)),
    }
}
```

All four arms MUST produce `CoordinatorEvent` values that flow through the *pure* FSM transition function (SPEC-13 R20). `reduce_all` is a Core-layer synchronous primitive (SPEC-13 R34); the in-process self-worker is responsible for the async↔sync bridge via `tokio::task::spawn_blocking`, exactly as a remote worker bridges its tokio I/O to its local Core reduction.

This pattern guarantees: (i) `LeaveRequest`, `WorkerJoined`, `PhaseTimeout`, and `RoundResult` events from remote workers are processed *while* the self-partition reduction is in-flight; (ii) panic in the self-partition spawn_blocking task surfaces as a `SelfPartitionPanic(reason)` event that the FSM handles deterministically (R3a below); (iii) the FSM is single-threaded and deterministic — no event is lost or reordered.

**R3a.** A panic or unexpected termination of the self-partition task MUST surface to the FSM via the new `SelfPartitionPanic(String)` event (§4.1.2). The FSM MUST transition `WaitingForResults × SelfPartitionPanic → Error` (and propagate via the standard SPEC-13 R21 `Error` actions). The self-partition is NOT eligible for elastic departure recovery (it is the coordinator's own process; if it panics, the run is fatal). **(MUST)**

**R3b.** While the self-partition spawn_blocking task is in flight, the coordinator MUST continue to process incoming `LeaveRequest`, `RoundResult` (delta), `PartitionResult` (v1), and `WorkerConnectionLost` events. These events MUST be applied to FSM state at receive time, not deferred until the self-partition completes. The coordinator MUST treat the self-partition as one of `K_eff` results to await before transitioning to the merge/border-graph-apply phase. **(MUST)**

**R3c (interaction with `strict_bsp`; closes SC-002 sub-issue 4).** In `strict_bsp = true` mode, the post-merge/post-border-resolve `reduce_all` (v1) or in-round border resolution (delta SPEC-19 R13) is skipped, and border-derived redexes are deferred to the next round (SPEC-05 R30a; SPEC-19 R40). The self-partition MUST follow the *same* short-circuit rule: its border-origin redexes are *not* short-circuited and contribute to the next round identically to those of any remote partition. R4's "treated identically" claim is preserved exactly because the self-partition flows through the same merge/border-graph code path; no special branch exists for the self-partition. **(MUST)**

**R4 (per-mode merge / border resolution).**
- **R4-v1 (mode A or B).** After all `K_eff` results are received (`PartitionResult` from remote workers + the self-partition's terminal `Partition`), the coordinator MUST merge all `K_eff` partitions using `merge()` (SPEC-05 R1-R11). The self-partition MUST be treated identically to any remote partition: same `free_port_index` reconstruction (SPEC-05 R20-R23), same border reconnection, same invariant checks. **(MUST)**
- **R4-delta (mode C or D).** After all `K_eff` round results are received (`RoundResult` from remote workers + the self-worker's local `RoundResult` synthesized via the in-process channel), the coordinator MUST `BorderGraph.apply_deltas` for each result (SPEC-19 R11), detect border redexes (R12), and resolve them at the coordinator (R13-R15). The self-worker's `RoundResult` flows through the identical `apply_deltas` path. At Global Normal Form, `FinalStateRequest` is sent to ALL members of `W_active` *including the self-worker*; the self-worker responds with `FinalStateResult` containing its current partition; the final `merge()` (SPEC-19 R29) treats it identically. **(MUST)**

**R5 (solo mode; closes SC-009).** When `K == 0` at the start of a round AND `hybrid_coordinator = true`, the coordinator MUST enter the `SoloReducing` FSM state and reduce the entire net via `reduce_n(net, solo_budget)` in a loop (NOT `reduce_all` end-to-end), polling the async event loop between successive `reduce_n` batches for `WorkerJoined` events. This trades minor per-batch overhead for join responsiveness. The default `solo_budget = 10_000` interactions; setting `solo_budget = u32::MAX` degenerates to `reduce_all` with no join responsiveness (acceptable for benchmark-only configurations). The previous wording "no split/merge overhead" (R5 v1 draft) is RETAINED in the sense that `split()` is not invoked, but the round-loop polling is explicitly part of solo-mode cost. **(MUST)**

**R5a (solo termination).** The `SoloReducing` state terminates on either: (i) `reduce_n` returns with empty redex queue (Local Normal Form, transition to `Done`), or (ii) `WorkerJoined(id)` event fires (transition to `CheckTermination` then `Partitioning` with `K_eff = 2` as per R15). In case (ii), if the in-flight `reduce_n` batch is mid-execution, the coordinator MUST allow that batch to complete (no preemption mid-batch) before processing the join. **(MUST)**

**R6 (initial wait / hybrid; closes SC-020).** In hybrid mode, if `initial_wait_timeout` (default 30 seconds, R33) elapses without any worker connections, the coordinator MUST begin solo reduction (transition `WaitingForWorkers × InitialWaitTimeout [K=0, hybrid=true] → SoloReducing`) and MUST cancel `worker_connect_timeout` (SPEC-06 R24, default 120s) at that point. `initial_wait_timeout` supersedes `worker_connect_timeout` whenever `hybrid_coordinator = true`. In non-hybrid mode, `worker_connect_timeout` applies and a coordinator with K=0 connections at `worker_connect_timeout` MUST abort with the v1 fatal-error path (SPEC-06 R24, SPEC-13 R21 `Error` transition). **(MUST)**

**R7.** The `GridMetrics` (SPEC-05 R34-R37; SPEC-19 R45) MUST distinguish coordinator-local reduction from remote-worker reduction. The coordinator's per-round local reduction time and interaction count MUST be recorded as a separate entry in `worker_stats_per_round` with `worker_id = 0` AND a new boolean field `is_coordinator_self: bool = true` on `WorkerRoundStats` (closes SC-027). Log analyzers and benchmark tooling MUST key on `is_coordinator_self`, NOT on `worker_id == 0` alone, to distinguish hybrid-mode coordinator self-stats from non-hybrid first-remote-worker stats. **(MUST)**

**R7a (`WorkerId = 0` reservation).** `WorkerId = 0` is reserved permanently for the coordinator self-partition role. The monotonic `WorkerId` counter for joining workers (R11) MUST start at `1`. In non-hybrid mode, `WorkerId = 0` MAY be assigned to the first remote worker for backwards compatibility with v1 tests; this is a non-conflicting use because non-hybrid mode does not have a self-partition. **(MUST)**

**R8 (id-range computation; closes SC-014, SC-006).** The self-partition's `IdRange` MUST be obtained by calling SPEC-04's `compute_id_ranges(K_eff)` (which returns `Vec<IdRange>` of length `K_eff`; the function does NOT take `next_id` as an argument — that was a wording error in the v1 draft of SPEC-20) and selecting the entry at `partition_index = 0`. The coordinator then sets the self-partition's sub-net `next_id` per SPEC-04 R18 (`max(range.start, max_agent_id_in_partition + 1)`). The same algorithm applies to remote workers using their `partition_index`, NOT their `WorkerId`. **(MUST)**

### 3.2 Dynamic Worker Joining — ROADMAP 2.2

**R9.** Between BSP rounds, the coordinator MUST accept new TCP connections from workers that were not present at the start of the current grid session. **(MUST)**

**R10 (join window timing; closes SC-007).** The join window is the FSM state `AcceptingMembershipChanges`, entered from `CheckTermination` when `is_normal_form == false` AND `(elastic_join || elastic_departure) == true`. The window has a minimum duration `join_window_min` (default 50 ms, R33) and a maximum duration `join_window_max` (default 500 ms, R33). It closes on the first `MembershipWindowClosed` timer event. **(MUST)**

**R10a.** When the coordinator transitions into `AcceptingMembershipChanges`, it MUST first drain all pending TCP connections that arrived between the previous dispatch and now (the queue maintained per R10b). Each drained connection completes the `Register` handshake (R11). The coordinator then arms the `join_window_min` timer; on `MembershipWindowClosed_min`, if no further pending connections have queued during the drain, it transitions to `Partitioning`. If new pending connections did queue during the drain, it arms `join_window_max - join_window_min` and transitions to `Partitioning` on either the next drain-empty observation or the timer expiry, whichever comes first. **(MUST)**

**R10b (boundary buffering; closes SC-012).** TCP `accept()` completions during any non-`AcceptingMembershipChanges` state (`Init`, `WaitingForWorkers`, `Partitioning`, `Dispatching`, `WaitingForResults`, `Merging`, `CheckTermination`) MUST be buffered in a coordinator-local `pending_connections_queue: VecDeque<TcpStream>`. The `Register` handshake MUST NOT be processed for buffered connections; only the raw stream is queued. Processing begins only after the FSM enters `AcceptingMembershipChanges` (R10a). The FSM transitions for these states MUST include explicit handlers for `WorkerJoined(id)` events (which represent only handshake-completed joins): `WaitingForResults × WorkerJoined(id) → WaitingForResults` with action `QueueWorkerForNextWindow(id)`, and similarly for `Partitioning`, `Dispatching`, `Merging`. This eliminates the FSM-totality gap flagged by SC-012. **(MUST)**

**R11 (WorkerId assignment; closes SC-006 + SC-023).** A joining worker MUST complete the `Register`/`RegisterAck` (or `JoinRequest`/`JoinAck` per R35) handshake before being included in `W_active`. The coordinator MUST assign a unique `WorkerId` from a monotonically increasing counter scoped to the BSP run, *starting at 1* (the value `0` is reserved per R7a). A departed worker's `WorkerId` MUST NOT be reused within the same run. The coordinator MUST track `next_worker_id: u32`; if `next_worker_id` would exceed `u32::MAX`, the coordinator MUST reject the join with `JoinNack { reason: WorkerIdSpaceExhausted }` (closes SC-023; see R35a). For TCC-scope workloads (≤ 8 workers, ≤ a few churn cycles), exhaustion is not expected. **(MUST)**

**R11a (partition_index decoupling; D4-elastic; closes SC-006).** The `partition_index` of a worker in a given round is its *position* in the round's `W_active ∪ {self if hybrid}` set sorted ascending by `WorkerId`. `partition_index` is dense in `[0, K_eff)`; `WorkerId` is sparse. SPEC-04's `compute_id_ranges(K_eff)` returns `Vec<IdRange>` indexed by `partition_index`, NOT by `WorkerId`. A new sub-invariant is added to §3.7 ("D4-elastic"). **(MUST)**

**R12 (per-mode re-partition).**
- **R12-v1.** When new workers join, the coordinator MUST re-partition the merged net (the output of the round's `merge()` — SPEC-05 R1-R11) for `K_eff_new = K_new + (1 if hybrid_coordinator)` slots at the start of the next round. The re-partition uses the standard `split()` (SPEC-04 R1) with the updated slot count. **(MUST)**
- **R12-delta.** When new workers join, the coordinator MUST: (i) at the join window, instruct surviving workers to send `FinalStateResult` (a one-time mid-run `FinalStateRequest`); (ii) `reconstruct(border_graph, worker_partitions)` (SPEC-19 R38) into a single net; (iii) call `split()` on that net for `K_eff_new` slots; (iv) dispatch fresh `InitialPartition` messages to *all* members of the new `W_active`, including the previously-active workers (which discard their old partitions and adopt the new one). This is the only delta-mode rejoin mechanism in v2; an alternative incremental "split-the-BorderGraph" optimization is OUT OF SCOPE for SPEC-20 (deferred to a future spec). **(MUST)**

**R12a (delta-mode rejoin cost.)** The `FinalStateResult`-then-`split`-then-`InitialPartition` cycle of R12-delta carries v1-equivalent wire cost for the rejoin round only (sum of `Partition` sizes). Subsequent rounds revert to delta-mode wire cost. This explicit cost is acceptable because joins are rare events (TCC scope) and avoiding state migration drift is more important than join-round wire-cost optimization. Documented in R12-delta and reflected in SPEC-19 break-even analysis as `c_o_join` overhead. **(informative)**

**R13 (id-range recomputation; closes SC-014).** ID ranges MUST be recomputed for all `K_eff_new` partition_indices using SPEC-04's `compute_id_ranges(K_eff_new)`. The function signature is unchanged from SPEC-04 R18; SPEC-20 only changes *when* it is called and *what value* of `K_eff` is passed. **(MUST)**

**R14 (joining worker payload).**
- **R14-v1.** A joining worker MUST receive a full `Partition` via `AssignPartition` (SPEC-06 R2) at the start of its first participating round. No special state requirements; the worker is a fresh participant.
- **R14-delta.** A joining worker MUST receive an `InitialPartition` (SPEC-19 R31, discriminant 7) at the start of its first participating round, after the rejoin cycle of R12-delta. The joining worker has no previous border state and starts the delta loop fresh.

**(MUST)**

**R15 (solo→grid transition; closes SC-009).** When a worker joins while the coordinator is in `SoloReducing`, the FSM MUST transition `SoloReducing × WorkerJoined(id) → CheckTermination` after the in-flight `reduce_n` batch completes (R5a). At `CheckTermination`, if the net is not in normal form, the FSM transitions to `AcceptingMembershipChanges` per R10, and the next round dispatches the (newly partitioned) net to `K_eff = 2` (1 self + 1 remote). If the net IS in normal form, the FSM transitions directly to `Done` and sends `Shutdown` to the joining worker. **(MUST)**

**R16 (mid-round joins are queued).** Workers that connect during an active round (after `Dispatching` and before `MembershipWindowClosed` of the next round) MUST be buffered per R10b and registered only at the next join window per R10a-R10b. They MUST NOT receive partitions mid-round.

**Justification:** The BSP barrier synchronization model (SPEC-05; SPEC-19 R20-R30) requires all workers to operate on partitions from the same split (v1) or the same delta state (v2). Injecting a worker mid-round would violate the barrier.

**(MUST)**

**R17.** The coordinator SHOULD log each worker join event at `INFO` level, including the new `K_eff_new`, the joining worker's `WorkerId`, the `partition_index` it will occupy in the next round, and the round number at which it will first participate. **(SHOULD)**

### 3.3 Dynamic Worker Departure — ROADMAP 2.3

#### 3.3.1 Detection

**R18 (timeout detection; closes SC-011 partial).** The coordinator MUST detect worker departure via the existing `collect_timeout` (SPEC-06 R30; default 600 s in v1, see R33 for elastic-mode override). If a worker does not return its current-round result (`PartitionResult` in v1, `RoundResult` in delta) within `collect_timeout`, the coordinator MUST treat it as departed under `elastic_departure = true`, rather than as the v1 fatal error (SPEC-06 R25, SPEC-13 R21 `WaitingForResults × PhaseTimeout → Error`). When `elastic_departure = false`, the v1 fatal-error behavior is preserved exactly. The conditional override of SPEC-06 R25 and SPEC-13 R21 is formally amended in §3.8. **(MUST)**

**R19.** Additionally, if the TCP connection to a worker is closed unexpectedly (I/O error during send or recv), the coordinator MUST treat that worker as departed immediately, without waiting for `collect_timeout`. This applies in both v1 and delta modes. **(MUST)**

#### 3.3.2 Graceful Departure (split-mode; closes SC-008)

**R20.** A worker MAY send a `LeaveRequest` to indicate departure. The coordinator MUST acknowledge with `LeaveAck` (R35) before closing the connection, and MUST remove the worker from `W_active` at the next round boundary. **(MUST)**

**R21 (LeaveRequest variant).** The `LeaveRequest` message MUST be a new variant appended to the `Message` enum with discriminant 14 (R35). Schema:

```rust
/// Discriminant: 14 (worker -> coordinator).
LeaveRequest {
    /// Reason for departure: clean (after returning current-round result)
    /// vs urgent (cannot complete current round).
    kind: LeaveKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeaveKind {
    /// R22a: worker has already returned PartitionResult/RoundResult for the
    /// current round; this LeaveRequest applies only to future rounds.
    AfterResult,
    /// R22b: worker cannot complete the current round; coordinator MUST treat
    /// as a timeout for the current round AND a graceful departure for future.
    Urgent,
}
```

The `worker_id` field is NOT carried on the wire because the coordinator already binds each TCP stream to a `WorkerId` at `Register` time (R11; consistent with SPEC-06 R2a). This closes the SC-004 sub-issue on redundant `worker_id`. **(MUST)**

**R22a (clean leave).** A worker that sends `LeaveRequest { kind: AfterResult }` MUST first complete the current round and return its result. The coordinator's FSM transition `WaitingForResults × WorkerLeft(id, AfterResult) → WaitingForResults` MUST apply actions `StoreResult(id, result_already_received), RemoveWorkerForNextRound(id), LogDeparture(AfterResult)`. **(MUST for worker behavior)**

**R22b (urgent leave).** A worker that cannot complete the current round MUST send `LeaveRequest { kind: Urgent }` *without* the corresponding result message. The coordinator's FSM transition `WaitingForResults × WorkerLeft(id, Urgent) → WaitingForResults` MUST apply the *same* actions as the timeout path: `ReclaimPartition(id, retained_initial), RemoveWorkerForCurrentAndFutureRounds(id), LogDeparture(Urgent)`. The urgent-leave path therefore composes the timeout-recovery logic for the current round with the graceful-departure bookkeeping for future rounds. **(MUST)**

**R22c.** If the coordinator receives `LeaveRequest { kind: AfterResult }` while *no* current-round result has been received from that worker, the coordinator MUST silently upgrade the request to `Urgent` semantics (apply R22b's actions) and log a `WARN`. Workers SHOULD NOT do this; the coordinator is lenient to avoid deadlock. **(MUST for coordinator; SHOULD for worker correctness)**

#### 3.3.3 Retained State Bookkeeping (closes SC-003)

**R23 (state retention enabled-by).** When `GridConfig.retain_partitions: bool` is `true` (default `true` whenever `elastic_departure = true`, R33), the coordinator MUST maintain *two* retained-state slots per worker, with semantics that depend on the active execution mode (M0):

**R23a (retained slot inventory).**
- `retained_initial[w]`: the round-0 dispatch state for worker `w`. Allocated once per worker (at the worker's first `AssignPartition` in v1 or first `InitialPartition` in delta) and held for the *entire* run. Released only when `w` either completes the run successfully or is shut down via `Shutdown`.
- `retained_last_acked[w]`: the most recent committed worker state. Refreshed *atomically* at every successful round boundary, defined as: "the round N+1 dispatch has been transmitted to all surviving members of `W_active`". Released only when its replacement (`retained_last_acked[w]_round_n+1`) has been successfully transmitted, OR when `w` departs and the slot has been consumed by a re-dispatch.

**R23b (per-mode contents of `retained_initial`).**
- **R23b-v1.** `retained_initial[w] = Partition` — the exact `Partition` structure sent to `w` via `AssignPartition` at round 0. Memory: O(|partition_w|).
- **R23b-delta.** `retained_initial[w] = Partition` — the `Partition` payload of the `InitialPartition` (SPEC-19 R31) sent at round 0. Memory: O(|partition_w|).

**R23c (per-mode contents of `retained_last_acked`).**
- **R23c-v1.** `retained_last_acked[w] = PartitionResult.partition` — the most recent `PartitionResult` the worker reported. Memory: O(|partition_w_evolved|).
- **R23c-delta.** `retained_last_acked[w] = (border_graph_snapshot_at_round_n_1: BorderGraph, last_round_result: RoundResult)` — sufficient to reconstruct the worker's state via `apply_deltas` from `retained_initial`, OR (recommended) the coordinator MAY snapshot the worker's full partition by piggybacking a `FinalStateRequest` to `w` whenever it is the slowest or whenever the operator opts into checkpointing. Memory: O(|border_graph| + |last_deltas|), or O(|partition_w|) if checkpoint mode is enabled. The default is the lightweight (border_graph, deltas) form; checkpoint mode is configurable via `GridConfig.checkpoint_partitions: bool` (R33).

**R23d.** When both retained slots exist, the coordinator MUST consult them in priority order: `retained_last_acked` first; `retained_initial` only if `retained_last_acked[w]` is `None` (i.e., `w` departed before its first round completed). The choice is encoded in R24. **(MUST)**

#### 3.3.4 Re-Dispatch on Departure (closes SC-003, SC-021)

**R24 (per-mode re-dispatch on departure).**
- **R24a (catastrophic departure, no successful round; uses `retained_initial`).** If a worker `w` departs (timeout, connection-loss, or `Urgent` leave) before *any* round has produced an acknowledged result for `w`, the coordinator MUST reclaim `retained_initial[w]`. In **v1 mode**, the partition is held until the next round boundary, where it is merged into the `merge()` call of the *current* round (the round in which `w` departed) ONLY IF surviving partitions have *not yet been reduced* (i.e., `w` departed before any survivors returned). If survivors have already reduced (mixed-trace case), the coordinator MUST defer the reclaimed partition to the *next* round: it is included as one of the `K_eff_new` slots, where `K_eff_new = K_eff - 1` (the departed worker's slot is reabsorbed via re-`split()` of the merged net + reclaimed partition, NOT by mixed-state merging within the same round). This deferred path closes the SC-003 D3-elastic concern. In **delta mode**, the catastrophic-loss path requires the rejoin cycle of R12-delta: `FinalStateRequest` to survivors, `reconstruct(border_graph, surviving_partitions, retained_initial[w])` (treating `retained_initial[w]` as if it were `w`'s `FinalStateResult`), `split()` for `K_eff_new`, fresh `InitialPartition` to all members of the new `W_active`. **(MUST)**

- **R24b (departure after at least one successful round; uses `retained_last_acked`).** If `w` departs after at least one round has produced an acknowledged result for `w`, the coordinator MUST reclaim `retained_last_acked[w]` (NOT `retained_initial`). The reclaim is then handled identically to R24a but using `retained_last_acked[w]` in place of `retained_initial[w]`. The R24a deferral rule (mixed-trace prevention) applies equally. **(MUST)**

**R24c (D3-elastic invariant; closes SC-003 sub-issue 1).** A new sub-invariant is added to §3.7 ("D3-elastic"):
> When a reclaimed partition (from `retained_initial` or `retained_last_acked`) is re-introduced into the system, it MUST be re-introduced via a re-`split()` (v1) or `reconstruct + split + InitialPartition` (delta) at a clean round boundary. It MUST NOT be merged with surviving partitions whose evolution diverged from the reclaimed partition's reduction trace. This re-establishes a uniform partition state before reduction resumes, preserving D3 (border completeness) by construction.

**R24d (border_id rebase; closes SC-021).** When a reclaimed partition is re-`split` per R24a/b, the coordinator MUST allocate a *fresh* `border_id` range for the reclaimed partition by extending SPEC-04's border_id allocator (`PartitionPlan.allocate_border_ids(count: u32)`). The reclaimed partition's old `border_id` values MUST be discarded; the partition's `free_port_index` MUST be rebuilt during the re-`split`. SPEC-04 is amended to expose this `allocate_border_ids` primitive (§3.8, amendment to SPEC-04 R15). **(MUST)**

**R25 (re-partition after departure).** After R24a/b's reclaim, the coordinator MUST re-partition for `K_eff_new = K_eff - D` slots at the next round (where `D` = number of departed workers in this transition). The re-partition uses `split()` per R12-v1 / R12-delta. **(MUST)**

**R26 (multiple simultaneous departures).** If multiple workers depart in the same round, the coordinator MUST handle all departures collectively in a single re-partition cycle: reclaim all `D` workers' state per R24a/b (by category), include all reclaimed partitions in the re-`split` input (v1) or in the `reconstruct` input (delta), and dispatch the new `K_eff_new = K_eff - D` partitions. The coordinator MUST NOT perform multiple sequential re-partitions for departures observed in the same window. **(MUST)**

**R27.** If all remote workers depart and `hybrid_coordinator = true`, the coordinator MUST fall back to solo reduction (R5/R5a). If `hybrid_coordinator = false` AND all remote workers depart, the coordinator MUST transition to `Error` (the v1 fatal path) because there is no executor left. **(MUST)**

**R28.** The coordinator SHOULD log each departure event at `WARN` level, including the departed worker's `WorkerId`, the departure type (`timeout`, `connection_loss`, `leave_after_result`, `leave_urgent`), the round number, and which retained slot (`retained_initial` or `retained_last_acked`) was consumed. **(SHOULD)**

#### 3.3.5 At-Least-Once Semantics

**R29.** The departure recovery strategy implements at-least-once semantics for partition reduction. A departed worker may have partially or fully reduced its partition before failing. The coordinator re-introduces the reclaimed state via R24a/b at a clean round boundary, where it is treated as a fresh partition by `split()` (v1) or `reconstruct + split + InitialPartition` (delta). Some reductions may therefore be performed twice across the system's history. The recoverability claim that re-execution converges to the same Normal Form is *not* a direct corollary of ARG-001 Passo 4 (which addresses order-of-reduction in a single-trace setting, not mixed-trace re-execution). The required extension is provided by ARG-006 (mixed-trace recoverability; CLOSED for v1 via P10+P12, see §3.3.6 and §3.7); SPEC-20 cites ARG-001 as supporting confluence base, and ARG-004 Passo 12 as a partial precedent for re-execution safety. For delta mode, ARG-006's R24b optimized path remains CONDITIONAL on ARG-005 (SPEC-19 delta protocol guarantees); the R24a conservative path (always using `retained_initial[w]`) is CLOSED without requiring ARG-005. **(informative; see R29a for the formal gate)**

**R29a (closes SC-005; updated 2026-04-24).** D-008 Stage 6 sign-off status:
- **v1 mode (A, B):** CLOSED via ARG-006. The mixed-trace recoverability proof (P10 idempotency + P11 snapshot consistency + P12 mixed-trace recoverability, following from P1+P10) discharges the R24a/R24c re-introduction-at-clean-boundary path without additional scope restriction.
- **Delta mode (C, D), R24a conservative path:** CLOSED via ARG-006 (uses `retained_initial[w]` unconditionally; mathematically equivalent to v1 re-split-from-initial case).
- **Delta mode (C, D), R24b optimized path:** CONDITIONAL on ARG-005 (depends on SPEC-19 delta protocol's border-completeness guarantees C-DEL1/C-DEL2; a departed worker's `retained_last_acked[w] = BorderGraph + applied deltas` is a valid re-introduction state only if ARG-005 holds).
Until ARG-005 lands, implementers MAY ship delta mode with R24b disabled (fall back to R24a-delta conservative); this restricted scope is CLOSED. **(MUST)**

**R30 (ID uniqueness preservation; closes SC-006).** At-least-once semantics MUST NOT introduce ID collisions. When a reclaimed partition is re-introduced via re-`split()`, ID ranges MUST be recomputed via SPEC-04's `compute_id_ranges(K_eff_new)` indexed by `partition_index` (R11a). The reclaimed partition becomes a fresh `Partition` with a new `IdRange`; agents within it carry IDs in the *old* range, but the re-`split()` call MUST renumber them via SPEC-04's existing remap path (§3.8 amends SPEC-04 to expose `remap_partition_ids(partition, new_range) -> Partition`). Agents created by the departed worker that were NEVER acknowledged are lost with the worker; only the reclaimed `retained_initial`/`retained_last_acked` agents enter the new round, and they carry IDs that are renumbered to fit the new `IdRange`. There is no risk of ID collision from un-acknowledged partial agents because they are simply not in the reclaimed state. **(MUST)**

#### 3.3.6 Memory Cost (closes SC-013)

**R31 (retained-state release; upgraded SHOULD→MUST).** The coordinator MUST release `retained_last_acked[w]` for round N as soon as both:
(a) a *new* `retained_last_acked[w]_round_n+1` has been *fully* committed (the next round's dispatch has been transmitted to all surviving members of `W_active`), AND
(b) the worker `w` has not departed.

If `w` departs between (a) and the start of round N+1's dispatch, the coordinator MUST consult `retained_last_acked[w]_round_n+1` (the freshest commit) per R24b.

`retained_initial[w]` is held for the entire run and released only at run end OR when `w` is permanently removed from the `WorkerId` namespace (which only happens at run end, per R11). Memory bound: `O(sum_{w in W_ever_active} |partition_w|)`.

`retained_last_acked` memory bound: `O(sum_{w in W_active} |partition_w|)` at any instant (one slot per active worker; replaced atomically at each round). **(MUST)**

**R32.** The `GridConfig` MUST provide `retain_partitions: bool` to allow disabling retention when departure detection is not needed (e.g., trusted LAN environments). When `retain_partitions = false`, `elastic_departure` MUST also be `false` (or the validator rejects the configuration), and `PhaseTimeout` reverts to v1 fatal error behavior (R18 condition). **(MUST)**

### 3.4 Configuration

**R33.** The `GridConfig` struct (SPEC-05; extended by SPEC-19 R41) MUST be extended with the following fields:

```rust
pub struct GridConfig {
    // ... existing v1 fields (num_workers, max_rounds, strict_bsp) ...
    // ... existing SPEC-19 fields (delta_mode, coordinator_free_rounds) ...

    /// Enable hybrid mode: coordinator reduces one partition locally.
    /// Default: FALSE for backwards-compatible benchmark reproduction
    /// (closes SC-010). The CLI flag `--hybrid` is opt-in.
    pub hybrid_coordinator: bool,

    /// Enable elastic departure: departed workers trigger re-dispatch
    /// instead of fatal error. Default: false (v1 compatibility).
    pub elastic_departure: bool,

    /// Retain pre-reduction copies of dispatched partitions for
    /// re-dispatch on worker departure. Default: true when
    /// elastic_departure is true; otherwise false.
    pub retain_partitions: bool,

    /// Enable accept of new TCP connections between BSP rounds.
    /// Default: true when hybrid_coordinator is true OR elastic_departure
    /// is true; otherwise false (v1 fixed worker count).
    pub elastic_join: bool,

    /// In delta mode, additionally snapshot each worker's full Partition
    /// per round (heavyweight checkpoint). When false (default), only
    /// (BorderGraph, last RoundResult deltas) is retained per worker.
    /// See R23c-delta.
    pub checkpoint_partitions: bool,

    /// Time to wait for initial worker connections before the
    /// hybrid coordinator begins solo reduction. Default: 30 seconds.
    /// Supersedes worker_connect_timeout when hybrid_coordinator = true.
    pub initial_wait_timeout: Duration,

    /// Minimum duration of the join window (R10a). Default: 50 ms.
    pub join_window_min: Duration,

    /// Maximum duration of the join window (R10a). Default: 500 ms.
    pub join_window_max: Duration,

    /// Per-batch interaction budget for solo mode (R5). Default: 10_000.
    /// Set to u32::MAX to degenerate to single-batch reduce_all (no join
    /// responsiveness; benchmark-only).
    pub solo_budget: u32,
}
```
**(MUST)**

**R33a (defaults table; closes SC-010, SC-020).**

| Field | Default | Rationale |
|-------|---------|-----------|
| `hybrid_coordinator` | `false` | Preserve v1 benchmark baseline; opt-in. |
| `elastic_departure` | `false` | Preserve v1 fatal-on-disconnect behavior. |
| `retain_partitions` | `false` (auto-true if `elastic_departure = true`) | Memory cost only when needed. |
| `elastic_join` | `false` (auto-true if `hybrid_coordinator || elastic_departure`) | Disabled when no elastic feature is in use. |
| `checkpoint_partitions` | `false` | Lightweight delta retention by default. |
| `initial_wait_timeout` | `30 s` | Hybrid mode tolerates short startup waits. |
| `join_window_min` | `50 ms` | Drain pending connects without measurable round overhead. |
| `join_window_max` | `500 ms` | Cap on adversarial drag-out. |
| `solo_budget` | `10_000` | Trade ~1% per-batch overhead for join responsiveness. |

**R34.** CLI arguments (SPEC-07, `clap`) MUST expose the new configuration fields:
- `--hybrid` / `--no-hybrid` (default: `--no-hybrid` per R33a)
- `--elastic-departure` / `--no-elastic-departure` (default: `--no-elastic-departure`)
- `--elastic-join` / `--no-elastic-join` (default: derived per R33a)
- `--retain-partitions` / `--no-retain-partitions` (default: derived per R33a)
- `--checkpoint-partitions` / `--no-checkpoint-partitions` (default: `--no-checkpoint-partitions`)
- `--initial-wait-timeout <SECONDS>` (default: 30)
- `--join-window-min-ms <MS>` (default: 50)
- `--join-window-max-ms <MS>` (default: 500)
- `--solo-budget <N>` (default: 10000)
**(MUST)**

### 3.5 Wire Protocol Extensions (closes SC-004)

**R35.** The `Message` enum (SPEC-06 R1-R5; extended by SPEC-19 R31-R32 with discriminants 7-11) MUST be extended with the following four variants, appended in the order shown to preserve discriminant stability:

| Discriminant | Variant | Direction | Payload |
|:---:|---------|:---------:|---------|
| 12 | `JoinRequest` | W → C | `protocol_version: u32`, `auth_token: Option<[u8; 32]>` (per SPEC-10 if auth is enabled), `worker_capabilities: WorkerCapabilities` (compatibility flags, currently empty struct reserved for v3) |
| 13 | `JoinAck` | C → W | `assigned_worker_id: WorkerId`, `partition_index: u32` (the worker's slot in the next round), `next_round_number: u32` (the round at which the worker first participates) |
| 14 | `LeaveRequest` | W → C | `kind: LeaveKind` (per R21) |
| 15 | `LeaveAck` | C → W | (empty payload; receipt-only acknowledgement) |
| 16 | `JoinNack` | C → W | `reason: JoinNackReason` (R35a) |

Schemas:

```rust
/// Discriminant 12.
JoinRequest {
    protocol_version: u32,
    auth_token: Option<[u8; 32]>,
    worker_capabilities: WorkerCapabilities,
}

/// Discriminant 13.
JoinAck {
    assigned_worker_id: WorkerId,
    partition_index: u32,
    next_round_number: u32,
}

/// Discriminant 14.
LeaveRequest {
    kind: LeaveKind, // see R21
}

/// Discriminant 15.
LeaveAck;

/// Discriminant 16.
JoinNack {
    reason: JoinNackReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JoinNackReason {
    /// Coordinator's PROTOCOL_VERSION differs from JoinRequest.protocol_version.
    ProtocolVersionMismatch { coordinator: u32, worker: u32 },
    /// elastic_join is disabled in the active GridConfig.
    ElasticJoinDisabled,
    /// WorkerId counter reached u32::MAX (R11; SC-023).
    WorkerIdSpaceExhausted,
    /// Authentication failed (SPEC-10).
    AuthenticationFailed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerCapabilities {
    // Reserved for v3; intentionally empty to claim wire space.
}
```

**(MUST)**

**R35a (acknowledgement semantics; closes SC-004 sub-issue and SC-017).** The coordinator MUST send `LeaveAck` (discriminant 15) before closing the worker's TCP connection. The worker MUST NOT close its connection before receiving `LeaveAck`. This eliminates the race where the worker disappears before the coordinator's bookkeeping is updated. The pre-existing `Shutdown` message (SPEC-06) is reserved exclusively for *coordinator-initiated* termination (run end, hard error); it is NOT used as a `LeaveRequest` ack. This decouples the two semantically distinct events. Closes SC-017 ambiguity. **(MUST)**

**R36.** All new variants MUST be serializable and deserializable via serde + bincode (SPEC-06 R4) and MUST satisfy the SPEC-18 wire-format requirements (rkyv archive + bytecheck under `--features zero-copy`). **(MUST)**

**R37 (PROTOCOL_VERSION bump; closes SC-004).** The wire protocol version MUST be bumped: `PROTOCOL_VERSION` (defined in `protocol/coordinator.rs`, last bumped to `3` by SPEC-19 R37) MUST increment from `3` to `4`. Justification: SPEC-20 introduces five new `Message` variants (discriminants 12-16) and a new payload type (`LeaveKind`, `WorkerCapabilities`, `JoinNackReason`); a v3 coordinator cannot bincode-decode a `JoinRequest` from a v4 worker, and vice versa. The existing `HandshakeAck` rejection path (SPEC-06 R2a; SPEC-19 R37) handles version mismatch by closing the connection with `RegisterNack` / `JoinNack` (R0d, R35a). Production cost is zero: v1 is frozen, v2 is dev-branch only. **(MUST)**

**R37a (Register vs JoinRequest selection).** A worker that connects to the coordinator's listener at any time during the run MUST use the existing `Register` handshake (SPEC-06 R2a) if it is participating in the *initial* `WaitingForWorkers` window, OR the new `JoinRequest` handshake if it is connecting *after* the coordinator has started the BSP loop (any state ≠ `WaitingForWorkers`). The coordinator MUST distinguish the two by inspecting the first message on the new connection: a `Register` initiates v1 startup registration; a `JoinRequest` initiates mid-session join. Mixing the two on a single connection is a protocol violation and MUST be rejected. **(MUST)**

### 3.6 Metrics Extensions

**R38.** The `GridMetrics` struct (SPEC-05 R34-R37; extended by SPEC-19 R45) MUST be extended with elastic-grid-specific metrics:

```rust
/// Number of workers that joined between rounds, per round.
pub workers_joined_per_round: Vec<u32>,

/// Number of workers that departed between rounds, per round.
pub workers_departed_per_round: Vec<u32>,

/// Effective slot count (K_eff) at the start of each round.
pub effective_slots_per_round: Vec<u32>,

/// Number of partitions re-dispatched due to worker departure, per round.
pub partitions_redispatched_per_round: Vec<u32>,

/// Per-round count of `retained_initial` reclaims (R24a).
pub retained_initial_reclaims_per_round: Vec<u32>,

/// Per-round count of `retained_last_acked` reclaims (R24b).
pub retained_last_acked_reclaims_per_round: Vec<u32>,

/// Coordinator overhead absorbed by the join window per round (delta mode
/// only; on rejoin rounds, equals the cost of FinalStateRequest cycle).
pub join_round_overhead_ms_per_round: Vec<u64>,
```
**(MUST)**

**R38a (additivity with SPEC-19 metrics; closes SC-019).** When both delta protocol (SPEC-19) and elastic grid (SPEC-20) are active, all metrics from both specs coexist in a single `GridMetrics` struct. SPEC-19's `border_graph_apply_deltas_time_per_round`, `delta_bytes_sent_per_round`, etc., are additive with the SPEC-20 fields above. There MUST NOT be a field name collision; any field-name collision discovered at implementation time is a spec defect to be reported back to ESPECIALISTA EM SPECS for arbitration. The current draft has no overlap. **(MUST)**

**R38b (`is_coordinator_self` field on WorkerRoundStats; closes SC-027).** `WorkerRoundStats` (SPEC-05 R37) MUST gain a boolean `is_coordinator_self: bool` field. Set to `true` for the coordinator's hybrid self-partition entry (R7); `false` for all remote workers. Log analyzers and benchmark tooling MUST key on this field, NOT on `worker_id == 0` alone. **(MUST)**

### 3.7 Invariant Preservation (post-Round-2 audit)

**R39.** All SPEC-01 invariants MUST be preserved under elastic grid operations across all four execution modes (M0). The Round 1 review flagged D1, D3, D4, D5, D6, G1 as AT_RISK; this section restores each to PRESERVED via explicit defenses (or, where formal proof is pending, marks the gap and the gating mechanism).

| Invariant | Status | Defense |
|-----------|:------:|---------|
| **T1-T7 (Theoretical)** | PRESERVED | Reduction rules and net structure are untouched. Strong confluence (T4 = P1) is the *enabler*, not the target. |
| **D1 (Split/Merge Identity)** | PRESERVED | The mixed-state-merge concern raised in SC-003 is eliminated by R24c (D3-elastic): reclaimed state is re-introduced via re-`split()` at a clean round boundary, never directly merged with surviving evolved partitions. The `merge()` function therefore only ever sees a uniform partition state. The atomic `retained_last_acked` refresh (R31) ensures the snapshot is consistent. |
| **D2 (Local Reduction Equivalence)** | PRESERVED | Each worker (remote OR self) reduces locally via `reduce_all` (v1) or the delta loop (delta). Unaffected. |
| **D3 (Border Completeness)** | PRESERVED via D3-elastic (R24c) | Reclaimed partitions never participate in mixed-trace merges. The new `border_id` rebase rule (R24d, with SPEC-04 amendment in §3.8) ensures no border_id range overlap between reclaimed and freshly-allocated partitions. SPEC-19 R39 (D3a-d) continues to govern delta-mode border resolution. |
| **D4 (ID Uniqueness)** | PRESERVED via D4-elastic (R11a) | `compute_id_ranges(K_eff)` is called with the round's slot count, and ID ranges are indexed by `partition_index` (dense), NOT by `WorkerId` (sparse). SC-006's premature exhaustion concern is eliminated. The signature mismatch flagged by SC-014 is fixed in R8/R13/R30 by reverting to SPEC-04's actual `compute_id_ranges(K_eff)` signature. R24d's border_id rebase prevents border_id collision after re-`split()`. |
| **D5 (Exclusive Ownership)** | PRESERVED | At any moment, each agent belongs to exactly one slot. `retained_initial` and `retained_last_acked` are *snapshots* not *live partitions*; they enter the live set only at a clean round boundary via re-`split`/`reconstruct`, at which point the departed worker's live copy is no longer in any executor. The atomic refresh of `retained_last_acked` (R31) prevents transient double-ownership. |
| **D6 (Protocol Termination)** | PRESERVED | Each round consumes ≥ 1 interaction from the finite total (T7). Membership changes between rounds do not add interactions; they only redistribute them. R24a/b's deferred re-introduction adds at most one extra round per departure event; bounded by the total churn count. SC-002 (mid-reduce events) is eliminated by R3's `tokio::select!` pattern. SC-007 (join-window race) is eliminated by R10a/b's drain-then-arm protocol. SC-009 (solo-mode preemption) is eliminated by R5/R5a's `reduce_n(budget)` loop. SC-012 (missing FSM transitions) is eliminated by R10b. SC-018 (`SoloReducing` state) is added in §4.1.1. SC-020 (timer conflict) is eliminated by R6's MUST. |
| **G1 (Fundamental Property), v1 modes (A, B)** | PRESERVED via R39-G1-v1 below | |
| **G1 (Fundamental Property), delta modes (C, D), R24a-delta conservative path** | CLOSED via ARG-006 (R29a) | |
| **G1 (Fundamental Property), delta modes (C, D), R24b-delta optimized path** | CONDITIONAL on ARG-005 (R29a) | |
| **G1 (Fundamental Property), elastic departure path (v1 mode)** | CLOSED via ARG-006 (R29a) | |
| **G1 (Fundamental Property), elastic departure path (delta mode, conservative)** | CLOSED via ARG-006 (R29a) | |
| **G1 (Fundamental Property), elastic departure path (delta mode, optimized)** | CONDITIONAL on ARG-005 (R29a) | |
| **I1-I7 (Implementation)** | PRESERVED | I1, I2, I3 are per-partition invariants, unaffected by membership changes. I4 (redex queue validity): stale entries continue to be tolerated. I5: `reduce_all` (v1) and the delta loop (delta) terminate for terminating nets. I6, I7: per-agent and per-net invariants, untouched. |
| **P1 (ARG-001)** | PRESERVED | Same as T4. |
| **P5 (ARG-001 termination)** | PRESERVED via D6 | |

**R39-G1-v1.** In modes A (v1-lenient) and B (v1-strict), without elastic departure (`elastic_departure = false`), G1 is preserved by exactly the v1 argument: `reduce_all(net) ~ extract_result(run_grid(net, K_eff))`. The hybrid coordinator (R1-R7) and dynamic joining (R9-R17) features change `K_eff` but not the reduction strategy; ARG-001 Passos 5-12 (round equivalence under any K_eff) apply directly because each round still consists of `split → reduce_local → merge`, and Passo 4 (confluence) covers any subset of redexes reduced in any order. **(PRESERVED)**

**R39-G1-elastic-departure.** With `elastic_departure = true`, G1 is preserved with the following status breakdown:
- **v1 mode (A, B):** PRESERVED via ARG-006. The argument: "Given a terminating net `mu`, strong confluence (T4), and a sequence of `(round, partition_state)` pairs where some `partition_state[round_k]` is a reclaimed `retained_initial` or `retained_last_acked` snapshot from round `j < k`, re-introduced via re-`split` at round `k`'s boundary, the resulting reduction converges to `NF(mu)`." ARG-006 discharges this via P10 (idempotency of `reduce_all` over intermediate states, corollary of P1+P6) + P11 (retained-snapshot consistency, R23+R31) + P12 (mixed-trace recoverability, consequence of P1+P10). R24c's D3-elastic constraint (no in-round mixing) ensures re-introduction always happens at a uniform partition boundary, which reduces the proof obligation to ARG-001 Passo 11 applied over an extended trace. ARG-006 strength: Moderado-Forte (same class as ARG-001). **(PRESERVED via ARG-006)**
- **Delta mode (C, D), R24a conservative path:** PRESERVED via ARG-006. Uses `retained_initial[w]` unconditionally; mathematically equivalent to v1 re-split-from-initial case. **(PRESERVED via ARG-006)**
- **Delta mode (C, D), R24b optimized path:** CONDITIONAL on ARG-005. Uses `retained_last_acked[w] = BorderGraph + applied deltas`, which requires SPEC-19 delta protocol's border-completeness guarantees (C-DEL1/C-DEL2) to constitute a valid re-introduction state. ARG-005 (delta border completeness, strength Moderado-Forte) discharges this. Until ARG-005 lands, implementers MUST fall back to R24a-delta conservative (CLOSED). **(CONDITIONAL on ARG-005)**

R29a gates D-008 sign-off on these per-path statuses. Empirical validation pending via EG-I3 / EG-I5a / EG-P2 / EG-P5 (see §7). **(PRESERVED for v1 and delta-conservative; CONDITIONAL for delta-optimized)**

**R39-G1-delta.** In delta modes (C, D), G1 is *conditionally* preserved via SPEC-19 R38's recoverability claim (also pending formal proof). SPEC-20 inherits SPEC-19's gating: any G1 deficiency in SPEC-19's recoverability proof propagates to SPEC-20 delta modes. **(CONDITIONAL on SPEC-19 R38 proof)**

**(MUST for the explicit defenses; v1-mode G1 and delta-conservative G1 CLOSED via ARG-006; delta-optimized G1 CONDITIONAL on ARG-005 per R29a / SPEC-19 R38)**

### 3.8 Amendments to Predecessor Specs (closes SC-011)

SPEC-20 amends the following requirements of predecessor specs. The amendments are formal and MUST be cross-referenced in those specs' next revision; ESPECIALISTA EM SPECS owns the cross-reference patches.

**A1. SPEC-06 R25 amendment.** SPEC-06 R25 ("If a connection with a worker is lost during execution, the coordinator MUST abort the grid loop and return an error") is amended with a conditional clause:

> *... unless `GridConfig.elastic_departure = true`, in which case the `WaitingForResults × PhaseTimeout(id)` and the `WaitingForResults × ConnectionLost(id)` transitions are handled per SPEC-20 §3.3 R18-R19 (coordinator reclaims state and removes the worker from `W_active`).*

**A2. SPEC-13 R21 amendment.** SPEC-13 R21's transition table is amended with the new rows defined in §4.1.3 below. The amendment introduces:
- New states: `AcceptingMembershipChanges`, `SoloReducing`.
- New events: `WorkerJoined(id)`, `WorkerLeft(id, kind)`, `WorkerConnectionLost(id)`, `MembershipWindowClosed`, `SelfPartitionReduced(stats)`, `SelfPartitionPanic(reason)`, `InitialWaitTimeout`, `SoloReductionComplete`.
- New transitions per §4.1.3.
- New actions: `RegisterWorker(id)`, `RemoveWorker(id)`, `ReclaimPartition(id, slot)`, `QueueWorkerForNextWindow(id)`, `LogJoin`, `LogDeparture`.

**A3. SPEC-04 R15 amendment (border_id allocator).** SPEC-04 R15 (border_id allocation, currently described as a one-shot allocation at `split()`-time) is amended with a new dynamic primitive:

> *R15a: SPEC-04 MUST expose `PartitionPlan::allocate_border_ids(count: u32) -> Range<u32>` that returns a fresh disjoint border_id range and updates the plan's internal `next_border_id` cursor. This primitive is required by SPEC-20 R24d for departure recovery.*

**A4. SPEC-04 R19 amendment (id remap).** SPEC-04 R19 (`remapAllPartitions` is no longer needed because IDs are pre-allocated) is amended with a narrow exception:

> *R19a: For SPEC-20's elastic departure recovery (R30), SPEC-04 MUST expose `remap_partition_ids(partition: Partition, new_range: IdRange) -> Partition` to renumber a reclaimed partition's agents into a fresh `IdRange` allocated for the new round. This is the ONLY supported caller of remap; v1 reduction continues to require zero remaps.*

**A5. SPEC-05 GridConfig amendment.** SPEC-05's `GridConfig` definition is extended with all the new fields listed in R33. Field defaults follow R33a.

**A6. SPEC-19 R45 amendment (metric coexistence).** SPEC-19's `GridMetrics` extension is composable with SPEC-20's per R38a. No collision in current draft.

---

## 4. Design

### 4.1 Coordinator FSM Extensions

The coordinator FSM (SPEC-13 R19-R21) is extended with new states, events, actions, and transitions. The existing states are preserved; new behavior is added strictly via *new* states, events, and transitions.

#### 4.1.1 Extended State Enum

```rust
pub enum CoordinatorState {
    // Existing from SPEC-13 R19:
    Init,
    WaitingForWorkers,
    Partitioning,
    Dispatching,
    WaitingForResults,
    Merging,
    CheckTermination,
    Done,
    Error,

    // New for SPEC-20:
    /// Accepting new worker connections and processing departures
    /// between BSP rounds (R10).
    AcceptingMembershipChanges,

    /// Coordinator reduces alone via `reduce_n(solo_budget)` loop
    /// because no remote workers are connected (R5/R5a). Closes SC-018.
    SoloReducing,
}
```

#### 4.1.2 Extended Events and Actions

```rust
pub enum CoordinatorEvent {
    // Existing from SPEC-13 R21 omitted ...

    // New for SPEC-20:
    WorkerJoined(WorkerId),
    WorkerLeft(WorkerId, LeaveKind),     // R22a/R22b
    WorkerConnectionLost(WorkerId),
    MembershipWindowClosed,
    SelfPartitionReduced(WorkerRoundStats),  // delta carries RoundResult-equivalent
    SelfPartitionPanic(String),                // R3a
    InitialWaitTimeout,
    SoloReductionComplete,
    SoloReduceBatchComplete,                   // emitted between reduce_n(budget) batches
}

pub enum CoordinatorAction {
    // Existing from SPEC-13 R21 omitted ...

    // New for SPEC-20:
    RegisterWorker(WorkerId),                  // moves pending → W_active
    RemoveWorker(WorkerId),                    // removes from W_active
    QueueWorkerForNextWindow(WorkerId),        // R10b
    ReclaimPartition(WorkerId, RetainedSlot),  // R24a/b; RetainedSlot ∈ {Initial, LastAcked}
    LogJoin(WorkerId),
    LogDeparture(WorkerId, DepartureKind),
    InvokeSplitAndDispatch(K_eff),             // wraps split + dispatch + retained-state seeding
    SpawnSelfPartition(Partition),             // R3
    PollPendingConnections,                    // R10a drain
}
```

#### 4.1.3 TimerKind Enum (closes SC-022)

```rust
/// Replaces all symbolic timer names ("initial_wait_timer", "join_window_timer",
/// "collect_timer") with a typed enum. SPEC-13 R21's `TimerId = u32` is derived
/// deterministically from `TimerKind` (e.g., as `kind as u32`).
pub enum TimerKind {
    InitialWait,
    JoinWindowMin,
    JoinWindowMax,
    Collect,
}
```

All transition rows in §4.1.4 use `StartTimer(TimerKind::X, duration)` and `CancelTimer(TimerKind::X)`.

#### 4.1.4 Extended Transition Table

Extended transitions (additions to SPEC-13 R21; existing rows unchanged unless noted). Transitions for FSM totality (SC-012, SC-018) are explicitly enumerated.

| From | Event | To | Actions | Condition |
|------|-------|----|---------|-----------|
| Init | ConfigLoaded | WaitingForWorkers | BindListener, StartTimer(InitialWait, initial_wait_timeout), LogTransition | always |
| WaitingForWorkers | InitialWaitTimeout | SoloReducing | CancelTimer(InitialWait), LogTransition | `K=0 && hybrid_coordinator` |
| WaitingForWorkers | InitialWaitTimeout | Error | CancelTimer(InitialWait), LogTransition | `K=0 && !hybrid_coordinator` (delegates to v1 worker_connect_timeout per R6) |
| WaitingForWorkers | WorkerConnected(id) [count >= min] | Partitioning | CancelTimer(InitialWait), InvokeSplitAndDispatch(K_eff), LogTransition | standard path |
| Partitioning | AllPartitionsReady | Dispatching | (per SPEC-13) | unchanged |
| Partitioning | WorkerJoined(id) | Partitioning | QueueWorkerForNextWindow(id), LogJoin | mid-state buffering (R10b) |
| Dispatching | AllDispatched [hybrid] | WaitingForResults | StartTimer(Collect), SpawnSelfPartition(self_partition), LogTransition | hybrid path (R3) |
| Dispatching | AllDispatched [!hybrid] | WaitingForResults | StartTimer(Collect), LogTransition | non-hybrid |
| Dispatching | WorkerJoined(id) | Dispatching | QueueWorkerForNextWindow(id), LogJoin | mid-state buffering |
| WaitingForResults | PartitionReturned(id, P) [all K_eff received] | Merging | CancelTimer(Collect), InvokeMergeOrApplyDeltas(all_results), LogTransition | both v1 and delta |
| WaitingForResults | SelfPartitionReduced(stats) [not all received yet] | WaitingForResults | StoreResult(0, stats), LogTransition | self finished first |
| WaitingForResults | SelfPartitionPanic(reason) | Error | CancelTimer, ShutdownAll(reason), LogTransition | R3a |
| WaitingForResults | PhaseTimeout(id) [elastic_departure] | WaitingForResults | ReclaimPartition(id, RetainedSlot::auto_pick), RemoveWorker(id), LogDeparture(Timeout) | elastic recovery (R18) |
| WaitingForResults | PhaseTimeout(id) [!elastic_departure] | Error | LogTransition, ShutdownAll | v1 fatal (R18) |
| WaitingForResults | WorkerConnectionLost(id) [elastic_departure] | WaitingForResults | ReclaimPartition(id, RetainedSlot::auto_pick), RemoveWorker(id), LogDeparture(ConnLost) | R19 |
| WaitingForResults | WorkerConnectionLost(id) [!elastic_departure] | Error | LogTransition, ShutdownAll | v1 fatal |
| WaitingForResults | WorkerLeft(id, AfterResult) | WaitingForResults | StoreResult(id, prev_result), RemoveWorkerForNextRound(id), LogDeparture(LeaveAfter), Send LeaveAck(id) | R22a |
| WaitingForResults | WorkerLeft(id, Urgent) | WaitingForResults | ReclaimPartition(id, RetainedSlot::auto_pick), RemoveWorker(id), LogDeparture(LeaveUrgent), Send LeaveAck(id) | R22b |
| WaitingForResults | WorkerJoined(id) | WaitingForResults | QueueWorkerForNextWindow(id), LogJoin | R10b mid-state |
| Merging | MergeComplete | CheckTermination | (per SPEC-13) | unchanged |
| Merging | WorkerJoined(id) | Merging | QueueWorkerForNextWindow(id), LogJoin | R10b mid-state |
| CheckTermination | NormalForm | Done | LogTransition, BroadcastShutdown | unchanged |
| CheckTermination | NotNormalForm [elastic_join \|\| elastic_departure] | AcceptingMembershipChanges | StartTimer(JoinWindowMin), PollPendingConnections, LogTransition | new elastic path |
| CheckTermination | NotNormalForm [!elastic_join && !elastic_departure] | Partitioning | InvokeSplitAndDispatch(K_eff), LogTransition | v1-equivalent path |
| AcceptingMembershipChanges | WorkerJoined(id) | AcceptingMembershipChanges | RegisterWorker(id), LogJoin | R10a drain |
| AcceptingMembershipChanges | WorkerLeft(id, _) | AcceptingMembershipChanges | RemoveWorker(id), LogDeparture, Send LeaveAck(id) | R22 graceful |
| AcceptingMembershipChanges | MembershipWindowClosed | Partitioning | InvokeSplitAndDispatch(K_eff_new), LogTransition | R10a closure |
| SoloReducing | SoloReduceBatchComplete [redexes_remain] | SoloReducing | (loop continues; no transition; emit poll) | R5 batch loop |
| SoloReducing | SoloReductionComplete | Done | LogTransition | R5a (i) |
| SoloReducing | WorkerJoined(id) | CheckTermination | QueueWorkerForNextWindow(id), LogJoin (transition deferred until end of in-flight batch) | R5a (ii), R15 |

#### 4.1.5 Hybrid Dispatch Pseudocode (rewritten for R3 concurrency clarity)

```text
Partitioning:
  partitions = split(net, K_eff)             # list indexed by partition_index ∈ [0, K_eff)
  retained_initial[w] = clone(partitions[partition_index_of(w)])  for each w in W_active
                                              # only when retain_partitions=true
  if hybrid_coordinator:
    self_partition = partitions[0]            # partition_index 0, WorkerId 0
    remote_partitions = partitions[1..K_eff]
  else:
    self_partition = None
    remote_partitions = partitions

Dispatching:
  for (partition, worker) in zip(remote_partitions, sorted_remote_workers_by_id):
    transport.send(worker, AssignPartition(partition))   # v1
    OR
    transport.send(worker, InitialPartition(round=N, partition))  # delta
  if hybrid_coordinator:
    self_worker_handle = spawn_local_worker_via_channel_transport(self_partition)
    # This spawns an in-process worker (SPEC-17 R15) that speaks the same wire
    # protocol. Self-partition reduction runs on a spawn_blocking thread; its
    # result message flows back through the channel like any remote worker's.
  arm StartTimer(Collect, collect_timeout)
  -> WaitingForResults

WaitingForResults:
  loop {
    select! {
      msg = workers.next_message() => fsm.handle(WorkerMessage(id, msg)),
      tk  = timer_wheel.next()     => fsm.handle(TimerFired(tk)),
      stm = listener.accept()      => pending_connections_queue.push(stm),
      pn  = self_worker_handle.panic_signal() => fsm.handle(SelfPartitionPanic(pn)),
    }
  }
  # All four arms produce events processed by the *pure* FSM transition function.
```

### 4.2 Re-Partition / Recovery Algorithms

#### 4.2.1 Worker Joining

**v1 mode (A or B).**
1. Round N completes normally with K_eff_old workers (`merge()` produces the merged net).
2. FSM enters `AcceptingMembershipChanges`; J new workers are registered (R10a-b).
3. `partitions = split(merged_net, K_eff_new)` where `K_eff_new = K_eff_old + J`.
4. `id_ranges = compute_id_ranges(K_eff_new)` (SPEC-04 R18; per R8/R13).
5. Partitions are dispatched to all `K_eff_new` slots (self-partition + K + J remote).

**delta mode (C or D).**
1. Round N completes normally; coordinator holds `BorderGraph + retained_last_acked[w]` per worker.
2. FSM enters `AcceptingMembershipChanges`; J new workers are registered.
3. Coordinator broadcasts `FinalStateRequest` to all surviving members of `W_active` (one-time mid-run); collects `FinalStateResult.partition` from each.
4. `merged_net = reconstruct(border_graph, surviving_partitions)` per SPEC-19 R38.
5. `partitions = split(merged_net, K_eff_new)`; `id_ranges = compute_id_ranges(K_eff_new)`.
6. Coordinator dispatches fresh `InitialPartition` to *all* members of new `W_active` (previously-active + new). Workers discard their old partition state and adopt the new one. The `BorderGraph` is reinitialized from the new `PartitionPlan` (SPEC-19 R10).
7. Subsequent rounds revert to delta-mode wire cost.

#### 4.2.2 Worker Departure

**v1 mode (A or B).**
1. During `WaitingForResults`, D workers depart (timeout / connection-loss / urgent leave); 0 ≤ D ≤ K_eff.
2. For each departed worker `w`, the coordinator reclaims `retained_last_acked[w]` if it exists, else `retained_initial[w]` (R23d, R24a/b auto-pick).
3. The K_eff_old - D surviving slots return their normal results (`PartitionResult` for v1).
4. **D3-elastic enforcement (R24c):** the coordinator does NOT call `merge()` on a mixed set within this round. Instead it transitions to `Merging` with the K_eff_old - D survivor results, completes the round's merge as if those were the only slots, and then enters `AcceptingMembershipChanges`. At the join window (which doubles as a departure-resolution window), the coordinator includes the reclaimed partitions in the next round's re-`split()` input by treating them as additional input nets to be unioned via `Net::union` before split.
5. `K_eff_new = K_eff_old - D` slots are dispatched in round N+1.

**delta mode (C or D).**
1. During `WaitingForResults`, D workers depart.
2. For each departed worker, the coordinator reclaims `retained_last_acked[w]` (which is `(border_graph_snapshot, last_round_result)` or, with `checkpoint_partitions = true`, a full `Partition`).
3. The coordinator broadcasts `FinalStateRequest` to surviving workers, collects their current partitions.
4. `merged_net = reconstruct(border_graph, surviving_partitions, reclaimed_partitions)` — `reconstruct` is extended (SPEC-19 amendment) to accept reclaimed partitions reconstructed from `retained_last_acked` (apply the snapshot's deltas to its corresponding `retained_initial` and union the result).
5. `partitions = split(merged_net, K_eff_new = K_eff_old - D)`.
6. Fresh `InitialPartition` to all surviving workers; new `BorderGraph` from the new `PartitionPlan`.

#### 4.2.3 Combined Join and Departure

If workers both join and depart between the same two rounds, the coordinator processes departures first (reclaim states per R24a/b), then registers joins per R10a-b, then computes:
```text
K_eff_new = (K_eff_old - D) + J     # where D = departed, J = joined
           = K_remote_remaining + J_new + (1 if hybrid_coordinator)
```
A single `split()` (v1) or `reconstruct + split + InitialPartition` (delta) cycle handles both; no sequential repartitions.

### 4.3 Timeout and Detection

#### 4.3.1 Round-Level Timeout

The existing `collect_timeout` (SPEC-06 R30; default 600 s) serves as the departure detection mechanism. No additional heartbeat protocol is required for SPEC-20.

When `elastic_departure = true`:
- If a worker's result is not received within `collect_timeout`, the coordinator fires `PhaseTimeout(worker_id)`.
- The transition is to `WaitingForResults` (not `Error`); `ReclaimPartition` is invoked; the worker is removed from `W_active`.
- Collection continues for remaining workers. When all responsive workers have returned (or timed out), the coordinator proceeds to `Merging`.

#### 4.3.2 Connection-Level Detection

TCP connection closure is detected immediately via the tokio I/O error path. This provides faster departure detection than `collect_timeout` for abrupt failures (process crash, network partition).

### 4.4 Message Flow Diagrams

#### 4.4.1 Normal Round (Hybrid Mode, No Membership Changes, v1)

```text
Coordinator                     Worker A             Worker B
    |                              |                     |
    |--- AssignPartition(P_A) ---->|                     |
    |--- AssignPartition(P_B) ---------------------------->|
    | [spawn_local_worker(P_self)] |                     |
    | (in-process channel-transport delivers PartitionResult to coordinator FSM)
    |<-- PartitionResult(P_A') ----|                     |
    |<-- PartitionResult(P_B') ----------------------------|
    |<== PartitionResult(P_self') from in-process self-worker
    | [merge(P_self', P_A', P_B')]                       |
    | [reduce_all(merged) -- borders, lenient mode]      |
    | [check termination]                                |
```

#### 4.4.2 Normal Round (Hybrid Mode, Delta)

```text
Coordinator                     Worker A             Worker B
    |                              |                     |
    | [Round 0: InitialPartition to A, B, and self via channel]
    | [Rounds 1+: RoundStart with deltas]
    |--- RoundStart(deltas_A) ---->|                     |
    |--- RoundStart(deltas_B) ---------------------------->|
    | (in-process) self-worker applies deltas, reduces, returns RoundResult
    |<-- RoundResult(deltas_A) ----|                     |
    |<-- RoundResult(deltas_B) ----------------------------|
    |<== RoundResult(deltas_self) from in-process channel
    | [BorderGraph.apply_deltas(self, A, B)]
    | [resolve border redexes per SPEC-19 R13-R15]
    | [check termination]
```

#### 4.4.3 Worker Joining Between Rounds (delta mode)

```text
Coordinator                     Worker A          Worker B (new)
    |                              |                   |
    | [round N completes; CheckTermination → AcceptingMembershipChanges]
    |<---------- JoinRequest --------------------------|
    |---- JoinAck(WorkerId=2, partition_index=2, next_round=N+1) ---->|
    | [MembershipWindowClosed]
    | [FinalStateRequest to A]
    |--- FinalStateRequest -------->|
    |<-- FinalStateResult(P_A_now) -|
    | [reconstruct(border_graph, {P_A_now, P_self_now})]
    | [split(merged, K_eff_new=3)]
    |--- InitialPartition(P_A_new) ->|                  |
    |--- InitialPartition(P_B_new) -------------------->|
    | [Rounds N+1+: standard delta loop with K_eff=3]
```

#### 4.4.4 Worker Departure (Timeout, v1 mode)

```text
Coordinator                     Worker A          Worker B (fails)
    |                              |                   |
    |--- AssignPartition(P_A) ---->|                   |
    |--- AssignPartition(P_B) -------------------------->|
    | [retain_initial[B] = clone(P_B); retain_initial[A] = clone(P_A)]
    | [spawn_local_worker(P_self)] |                   |
    |                              | [reduce_all(P_A)] | [CRASH]
    |                              |                   X
    |<-- PartitionResult(P_A') ----|
    | [collect_timeout fires for B]
    | [WaitingForResults × PhaseTimeout(B)
    |   → ReclaimPartition(B, RetainedSlot::Initial)
    |   → RemoveWorker(B), LogDeparture(Timeout)]
    | [merge(P_self', P_A')]  -- only K_eff_old - D = 2 partitions in-merge (D3-elastic)
    | [CheckTermination → AcceptingMembershipChanges]
    | [Inside AcceptingMembershipChanges, reclaimed P_B is unioned into the
    |  merged net before re-split for K_eff_new = K_eff_old - 1 = 2]
    |--- AssignPartition(P_self_new) ---> [self channel]
    |--- AssignPartition(P_A_new) ---->|
```

#### 4.4.5 Graceful Departure (clean, v1 mode)

```text
Coordinator                     Worker A          Worker B (leaving)
    |                              |                   |
    |--- AssignPartition(P_A) ---->|                   |
    |--- AssignPartition(P_B) -------------------------->|
    |                              | [reduce_all(P_A)] | [reduce_all(P_B)]
    |<-- PartitionResult(P_A') ----|                   |
    |<-- PartitionResult(P_B') -------------------------- |
    |<-- LeaveRequest(kind=AfterResult) -----------------|
    |--- LeaveAck ---------------------------------------->|
    |                              |                   [TCP close]
    | [merge(P_self', P_A', P_B')]
    | [CheckTermination → AcceptingMembershipChanges]
    | [RemoveWorkerForNextRound(B); K_eff_new = K_eff_old - 1]
```

#### 4.4.6 Graceful Departure (urgent split-mode, v1)

```text
Coordinator                     Worker A          Worker B (leaving urgently)
    |                              |                   |
    |--- AssignPartition(P_A) ---->|                   |
    |--- AssignPartition(P_B) -------------------------->|
    |                              | [reduce_all(P_A)] | [partial reduce; OOM]
    |<-- PartitionResult(P_A') ----|                   |
    |<-- LeaveRequest(kind=Urgent) ----------------------|
    |--- LeaveAck ---------------------------------------->|
    | [WaitingForResults × WorkerLeft(B, Urgent)
    |   → ReclaimPartition(B, RetainedSlot::Initial)
    |   → RemoveWorker(B), LogDeparture(LeaveUrgent)]
    | [merge(P_self', P_A')]  -- 2 partitions only (D3-elastic)
    | [reclaimed P_B is unioned in the next AcceptingMembershipChanges]
```

---

## 5. Rationale

### 5.1 Why Confluence Is Necessary But Not Sufficient

The Elastic Grid architecture rests on strong confluence (SPEC-01 T4; REF-002 Proposition 1; ARG-001 P1) plus an explicit *mixed-trace recoverability* extension (ARG-006, CLOSED for v1 and delta-conservative; delta-optimized mode additionally CONDITIONAL on ARG-005 per R29a).

**Confluence guarantees that:**

1. **The result is independent of partition count.** Splitting a net into K partitions or K+1 partitions produces the same normal form after complete reduction (ARG-001 Passos 5-12). This justifies coordinator-as-worker (2.1) and dynamic joining (2.2) without further extension.

2. **The result is independent of reduction order within a single trace.** Reducing partition A fully, then partition B partially, then continuing both in the next round, produces the same normal form as reducing A and B in lockstep (ARG-001 Passo 4 = T4).

3. **Total interaction count is invariant.** Regardless of how many workers participate or how work is redistributed, the total number of reduction steps to normal form is the same (T7).

**Confluence does NOT directly cover:**

4. **Mixed-trace recoverability.** When a reclaimed partition (snapshot from round 0 or round N-1) is re-introduced at round N alongside survivors that have evolved to a *different* intermediate state via a divergent reduction trace, the resulting net is the image of the original under TWO different partial traces. Passo 4 covers permutation of order *within a single trace*; it does not cover the union of two traces. **ARG-006 discharges this gap** for v1 and delta-conservative modes via P10 (idempotency of `reduce_all`), P11 (snapshot consistency), and P12 (mixed-trace recoverability as a consequence of P1+P10). The R24c "deferred re-split" rule reduces the proof obligation by ensuring the union happens at a clean boundary (re-`split` from a uniform state) rather than as an in-round mixed merge. For delta-mode's R24b optimized path, additional border-completeness guarantees from ARG-005 (SPEC-19) are required.

### 5.2 Why No Consensus Is Needed

Traditional distributed systems require consensus protocols (Raft, Paxos) for dynamic membership changes because operations are not commutative or idempotent. In Relativist:

- **Commutativity:** Strong confluence implies that any two disjoint active pairs can be reduced in either order (T4). Stronger than consensus.
- **Idempotency:** Reducing an already-reduced pair is a no-op (the pair no longer exists). Re-introducing a reclaimed partition produces the same final net as reducing it once, because the redundant reductions simply find no active pairs. (This claim is established by ARG-006 P10 for v1 and delta-conservative modes; delta-optimized additionally requires ARG-005.)
- **Single coordinator:** The coordinator is the sole decision-maker for membership changes. No distributed agreement is needed because only one node makes membership decisions.

### 5.3 Comparison Across v1, v2-elastic, v2-delta, v2-delta+elastic

| Aspect | v1 (SPEC-06/13) | v2 elastic only (SPEC-20, no SPEC-19) | v2 delta only (SPEC-19, no SPEC-20) | v2 elastic + delta (both) |
|--------|-----------------|--------------------------------------|--------------------------------------|----------------------------|
| Worker count | Fixed at startup | Dynamic between rounds | Fixed at startup | Dynamic between rounds |
| Coordinator role | Orchestration only | + optional reduction (hybrid) | Orchestration + border-graph maintenance | + optional reduction |
| Worker departure | Fatal error | Reclaim + re-dispatch | Fatal error | Reclaim + re-dispatch (delta-aware) |
| Worker state across rounds | Reset every round | Reset every round | Persistent (R22) | Persistent + checkpointable |
| Per-round wire cost | O(sum partition sizes) | Same | O(deltas) | O(deltas), spike on join (R12a) |
| Merged net at coordinator | Yes, every round | Yes, every round | Only at convergence | Only at convergence + on rejoin |
| K_eff | K | K+1 (hybrid) or K | K | K+1 (hybrid) or K |
| ID range computation | Once at startup | Recomputed each round if K changes | Once at startup | Recomputed on join/leave |
| Partition retention | Not retained | Retained when `elastic_departure` | Not retained | Retained per R23a |

---

## 6. Migration Path

### 6.1 v1 Static Workers to v2 Elastic

The migration is designed for full backward compatibility. Default configuration values (R33a) reproduce v1 behavior exactly.

**Step 1: Hybrid coordinator (low risk).**

1. Add `hybrid_coordinator: bool` to `GridConfig` (default: **false** per R33a; closes SC-010).
2. Modify the coordinator's `Dispatching` state to spawn the in-process self-worker via `ChannelTransport` (R3) when hybrid mode is on.
3. Modify `WaitingForResults` to wait for `K_eff` results (K remote + 1 self-channel).
4. Add `is_coordinator_self` field to `WorkerRoundStats` (R38b).
5. No wire protocol changes needed at this step.
6. All 1181 existing v2 tests pass with `hybrid_coordinator = false`.

**Step 2: Dynamic joining (medium risk).**

1. Add `elastic_join: bool`, `join_window_min/max: Duration` to `GridConfig`.
2. Add `AcceptingMembershipChanges` state to coordinator FSM; add the eight FSM transitions for `WorkerJoined` across non-AcceptingMembershipChanges states (R10b).
3. Modify TCP listener to accept connections during grid execution and buffer to `pending_connections_queue` (R10b).
4. Implement R10a drain protocol with `JoinWindowMin`/`JoinWindowMax` timers.
5. Add `JoinRequest`/`JoinAck`/`JoinNack` wire variants (R35; discriminants 12, 13, 16). Bump `PROTOCOL_VERSION` to 4 (R37).
6. Add `next_worker_id: u32` counter starting at 1 (R7a, R11). Implement `partition_index` decoupling (R11a).

**Step 3: Dynamic departure (medium-high risk; CLOSED via ARG-006 for v1 mode per R29a; delta-optimized path additionally gated on ARG-005).**

1. Add `elastic_departure: bool`, `retain_partitions: bool`, `checkpoint_partitions: bool` to `GridConfig`.
2. Implement two-slot retention (R23a-d): `retained_initial` and `retained_last_acked` per worker.
3. Implement R24a/b deferred re-split protocol; D3-elastic enforcement (R24c).
4. Implement R24d border_id rebase (requires SPEC-04 amendment A3 to land first).
5. Implement R30 id-remap on reclaim (requires SPEC-04 amendment A4 to land first).
6. Add `LeaveRequest`/`LeaveAck` wire variants (discriminants 14, 15) with `LeaveKind { AfterResult, Urgent }` enum. Implement R22a/R22b/R22c FSM transitions and R35a ack semantics.
7. Change `PhaseTimeout` and `ConnectionLost` transitions to recovery path when `elastic_departure = true` (A1: SPEC-06 R25 amendment).

**Step 4: Delta-mode integration (high risk; gated on SPEC-19 stability; R24a-delta conservative path CLOSED via ARG-006; R24b-delta optimized path CONDITIONAL on ARG-005).**

1. Implement R0a per-feature mode coverage; ensure each elastic feature has explicit per-mode handlers per R12-v1/R12-delta, R14-v1/R14-delta, R23b-v1/R23b-delta, R23c-v1/R23c-delta, R24a/b per-mode branches.
2. Implement R12-delta rejoin cycle (`FinalStateRequest` mid-run, reconstruct, split, fresh `InitialPartition`).
3. Implement delta-mode retained state shape (`(BorderGraph snapshot, last RoundResult deltas)` per R23c-delta).
4. Validate G1-CONDITIONAL (R39-G1-delta, R39-G1-elastic-departure) by composing tests EG-I3, EG-I4 with `delta_mode = true`.

### 6.2 Feature Flags

All four features are independently enableable via `GridConfig`:

| Feature | Config Field | Default | Can disable? |
|---------|-------------|---------|--------------|
| Coordinator as Worker | `hybrid_coordinator` | `false` (per SC-010) | Yes |
| Dynamic Joining | `elastic_join` | derived | Yes |
| Dynamic Departure | `elastic_departure` | `false` | Yes (reverts to v1 fatal error per R32) |
| Partition Retention | `retain_partitions` | derived | Yes (forces `elastic_departure = false`) |
| Heavy Checkpoint (delta) | `checkpoint_partitions` | `false` | Yes |

---

## 7. Test Strategy

### 7.1 Unit Tests

| ID | Test | Requirement | Description |
|----|------|-------------|-------------|
| EG-U1 | `test_hybrid_coordinator_single_machine` | R1, R5 | Coordinator reduces alone (K=0, hybrid). Verify normal form matches `reduce_all`. |
| EG-U1a | `test_solo_join_during_solo_reduction` | R5/R5a, R15, SC-009 | Coordinator in `SoloReducing`; worker joins at t=k batches; verify transition to `CheckTermination` after current batch completes, then to grid mode. |
| EG-U1b | `test_worker_id_zero_semantics_per_mode` | R2a, SC-016 | In hybrid mode, `WorkerId 0` is the self-partition; in non-hybrid mode, `WorkerId 0` is the first remote worker. |
| EG-U2 | `test_hybrid_partition_count` | R2 | With K=3 remote workers, verify `split()` produces K_eff=4 partitions. |
| EG-U3 | `test_hybrid_self_partition_id_range` | R8 | Verify self-partition (partition_index=0) receives the first IdRange from `compute_id_ranges(K_eff)`. |
| EG-U4 | `test_hybrid_merge_includes_self` | R4-v1 | Merge K+1 partitions (self + K remote). Verify result matches sequential `reduce_all`. |
| EG-U4-delta | `test_hybrid_apply_deltas_includes_self` | R4-delta | Apply deltas from K+1 round results (self + K remote) to BorderGraph. Verify converged state matches v1 hybrid. |
| EG-U5 | `test_dynamic_join_repartition_v1` | R12-v1, R13 | Start v1 K=2, add 1 worker. Verify next round uses K_eff=4 (with hybrid) and ID ranges are disjoint. |
| EG-U5-delta | `test_dynamic_join_repartition_delta` | R12-delta | Start delta K=2, add 1 worker. Verify FinalStateRequest cycle, reconstruct, fresh InitialPartition. |
| EG-U6 | `test_dynamic_join_mid_round_queued` | R10b, R16 | Worker connecting mid-round is buffered and not dispatched until next window. |
| EG-U6a | `test_join_window_boundary_race` | R10a-b, SC-007 | Inject `tokio::yield_now()` to force `MembershipWindowClosed` and `WorkerJoined` race; verify deterministic round assignment. |
| EG-U7 | `test_departure_reclaim_initial` | R23a-b, R24a | Worker departs before any round result; reclaim `retained_initial`; correctness preserved. |
| EG-U7a | `test_departure_reclaim_border_id_rebase` | R24d, SC-021 | Reclaimed partition has border_ids 100-105; surviving partitions have border_ids 200-210; verify no overlap and correct reconnection after re-split. |
| EG-U7b | `test_departure_reclaim_last_acked_v1` | R23c-v1, R24b | Worker departs after 2 successful rounds; reclaim `retained_last_acked` (the round-2 partition), not `retained_initial`. |
| EG-U7c | `test_departure_reclaim_last_acked_delta` | R23c-delta, R24b | Same as EG-U7b but in delta mode; reclaim `(BorderGraph snapshot, last RoundResult deltas)`. |
| EG-U8 | `test_departure_multiple_workers_v1` | R26 | 2 of 4 workers depart simultaneously; one re-split for K_eff=3 (with hybrid: K_eff=2). |
| EG-U9 | `test_departure_all_workers_solo_fallback` | R27 | All remote workers depart; coordinator falls back to `SoloReducing`. |
| EG-U10 | `test_graceful_leave_after_round` | R20, R22a | Worker sends PartitionResult then `LeaveRequest{AfterResult}`; verify `LeaveAck` is sent and worker removed for next round. |
| EG-U10a | `test_graceful_leave_urgent_v1` | R22b, SC-008 | Worker sends `LeaveRequest{Urgent}` mid-round; coordinator uses timeout-recovery path for current round. |
| EG-U10b | `test_graceful_leave_urgent_delta` | R22b | Same as EG-U10a in delta mode. |
| EG-U10c | `test_graceful_leave_after_result_no_result_received` | R22c | Worker sends `AfterResult` but coordinator never received result; coordinator silently upgrades to Urgent. |
| EG-U11 | `test_join_and_departure_same_round` | §4.2.3, R26 | 1 worker joins, 1 departs between same rounds; verify K_eff and single-cycle handling. |
| EG-U12 | `test_id_ranges_no_collision_after_repartition` | R13, R30, R11a | After K changes, all ID ranges disjoint across all K_eff slots. |
| EG-U12a | `test_partition_index_vs_worker_id_decoupling` | R11a, SC-006 | With WorkerIds {0, 1, 5, 7} and K_eff=4, ranges are `[0,c), [c,2c), [2c,3c), [3c,4c)` keyed on partition_index, not on WorkerId 5/7. |
| EG-U13 | `test_retained_partition_atomic_release` | R31 (MUST), SC-013 | Verify `retained_last_acked[w]_round_n` is held until round N+1 dispatch is fully transmitted. |
| EG-U14 | `test_worker_id_exhaustion_join_nack` | R11, R35a, SC-023 | After `next_worker_id == u32::MAX`, next `JoinRequest` receives `JoinNack { reason: WorkerIdSpaceExhausted }`. |
| EG-U15 | `test_protocol_version_mismatch_rejection` | R0d, R37 | v3 worker connects to v4 coordinator; rejected with `RegisterNack { ProtocolVersionMismatch }`. |
| EG-U16 | `test_self_partition_panic_to_error` | R3a | Inject panic in self-partition spawn_blocking task; verify `WaitingForResults × SelfPartitionPanic → Error`. |
| EG-U17 | `test_strict_bsp_self_partition_uniformity` | R3c | In strict_bsp mode, self-partition border-origin redexes are deferred to next round identically to remote. |
| EG-U18 | `test_initial_wait_timeout_supersedes_worker_connect_timeout` | R6, SC-020 | Hybrid mode, K=0, `initial_wait_timeout=30s`, `worker_connect_timeout=120s`; verify solo reduction starts at 30s. |
| EG-U19 | `test_leave_ack_before_close` | R35a, SC-017 | Verify coordinator sends `LeaveAck` before TCP close on graceful departure; worker MUST NOT close before receiving LeaveAck. |

### 7.2 Integration Tests

| ID | Test | Requirement | Description |
|----|------|-------------|-------------|
| EG-I1 | `test_hybrid_grid_correctness_v1` | R1, R4-v1, G1 | Hybrid `run_grid` for several benchmark nets; verify `reduce_all(net) ~ extract_result(run_grid(net, K_eff))`. |
| EG-I1-delta | `test_hybrid_grid_correctness_delta` | R1, R4-delta, G1 (CONDITIONAL on ARG-005 for R24b path; CLOSED via ARG-006 for R24a path) | Hybrid `run_grid_delta` for the same nets; verify equivalence. R24a conservative path exercised unconditionally; R24b optimized path gated on ARG-005. |
| EG-I2 | `test_elastic_join_correctness_v1` | R9-R14, R12-v1, G1 | v1, start with 1 worker, add 2 more after round 1; final result matches `reduce_all`. |
| EG-I2-delta | `test_elastic_join_correctness_delta` | R9-R14, R12-delta, G1 (CONDITIONAL) | Delta, start with 1 worker, add 2 after round 1; FinalStateRequest cycle works; final result matches `reduce_all`. |
| EG-I3 | `test_elastic_departure_correctness_v1` | R18-R26, G1 (CLOSED via ARG-006) | v1, start with 4 workers, simulate 1 departure at round 2; final result matches `reduce_all`. Empirical validation of ARG-006 P10/P12. |
| EG-I3-delta | `test_elastic_departure_correctness_delta` | R18-R26, G1 (CONDITIONAL) | Delta version of EG-I3. |
| EG-I4 | `test_elastic_churn_correctness` | R9-R30, G1 (CONDITIONAL) | Start with 2 workers, add 3, remove 2, add 1 across multiple rounds; final result matches `reduce_all`. |
| EG-I5 | `test_v1_compatibility_mode` | R32, R39-G1-v1 | Run with all elastic flags `false`; behavior identical to v1 `run_grid`. Zero-regression on the existing 1181-test baseline. |
| EG-I5a | `test_condup_cascades_with_retained_redispatch` | R24c, SC-005, SC-015 | CON-DUP-heavy net with departure mid-cascade; reclaimed partition re-introduced at clean boundary; verify final result matches `reduce_all`. |
| EG-I5b | `test_emergent_borders_across_retained_evolved` | R24d, SC-021 | Workload with emergent borders in both reclaimed and surviving partitions; verify border_id rebase prevents collision. |

### 7.3 Property-Based Tests

| ID | Test | Requirement | Description |
|----|------|-------------|-------------|
| EG-P1 | `prop_hybrid_normal_form_invariant` | G1 | Random terminating nets, random K ∈ [0, 8]: `reduce_all(net) ~ run_grid(net, K, hybrid=true)`. |
| EG-P2 | `prop_departure_normal_form_invariant_v1` | G1 (CLOSED via ARG-006) | Random nets, random departure schedules in v1 mode. Empirical signature for ARG-006. |
| EG-P3 | `prop_id_ranges_disjoint_after_repartition` | D4-elastic, R30 | Random K_eff changes; all ID ranges disjoint. |
| EG-P4 | `prop_full_matrix_correctness` | G1 (CONDITIONAL), SC-015 | Random `(hybrid ∈ {on,off}) × (strict_bsp ∈ {on,off}) × (delta_mode ∈ {on,off}) × (join schedule) × (leave schedule) × random K × random terminating net`. Final result matches `reduce_all`. |
| EG-P5 | `prop_condup_heavy_churn` | G1 (CLOSED via ARG-006 for v1/delta-conservative; CONDITIONAL on ARG-005 for delta-optimized), SC-015 | CON-DUP-heavy generator (`ep_annihilation_con` from SPEC-09) × random membership changes. |
| EG-P6 | `prop_delta_elastic_correctness` | R0a, G1 (CONDITIONAL), SC-001 | Pure delta-mode + elastic features; random scenarios; verify equivalence. |

### 7.4 Benchmark Tests

| ID | Test | Requirement | Description |
|----|------|-------------|-------------|
| EG-B1 | `bench_hybrid_vs_nonhybrid` | R1, SC-010 | Compare wall-clock of hybrid vs non-hybrid for the same net and K. Run with `--hybrid` AND `--no-hybrid` to fulfill master plan Gate 1 ("apples-to-apples vs apples-to-oranges"). |
| EG-B2 | `bench_retention_memory_overhead` | R31, R23 | Measure peak memory with and without partition retention for large nets, in v1 and delta modes. |
| EG-B3 | `bench_join_round_overhead_delta` | R12a, R12-delta | Measure `c_o_join` overhead of mid-run join in delta mode (FinalStateRequest cycle cost). |

---

## 8. Open Questions (resolved post-Round-2)

This section retains the original open questions for traceability; all are now resolved per SC-026.

**OQ-1 (RESOLVED in R10a/R10b).** Join window duration. Resolution: `[join_window_min=50ms, join_window_max=500ms]` configurable, drain-then-arm protocol per R10a-b.

**OQ-2 (RESOLVED in R2/R2a).** Self-partition assignment strategy. Resolution: always `partition_index = 0` (the first slot in sorted-by-WorkerId order); `WorkerId = 0` is reserved for the self-partition role. A future "smaller-partition-to-coordinator" optimization is OUT OF SCOPE for SPEC-20 and tracked separately for v3 (closes SC-024).

**OQ-3 (RESOLVED in R3 + R3c).** Interaction with strict BSP mode. Resolution: hybrid mode and strict BSP compose orthogonally; the self-partition is treated identically to remote partitions in both lenient and strict modes (R3c). The `tokio::select!` pattern (R3) ensures FSM events are processed regardless of self-partition reduction state.

**OQ-4 (RESOLVED — DEFERRED).** Partition retention vs. checkpoint to disk. Resolution: in-memory retention only (R23, R31). Disk checkpointing is deferred to a future fault-tolerance spec (SPEC-23 Fault Tolerance, currently scoped for v3+). The `checkpoint_partitions: bool` field (R33) provides the lightweight delta-mode equivalent.

**OQ-5 (RESOLVED in R11/R11a).** Worker ID assignment for re-joined workers. Resolution: always assign a new `WorkerId` (never reuse) per R11. Decoupling `WorkerId` (sparse) from `partition_index` (dense, R11a) means re-join with a fresh `WorkerId` does NOT cause ID-range fragmentation; range assignment is keyed on `partition_index`. SC-006 closed.

---

## 9. Open Issues (post-Round-2)

The following residual items are NOT blockers for spec-critic Round 2 endorsement but MUST be tracked as follow-ups:

1. **ARG-006 (mixed-trace recoverability proof)** CLOSED (2026-04-24) — v1 mode and delta-conservative path proved by P10+P11+P12 over ARG-001 P1-P6; see `discussoes/argumentos/ARG-006-mixed-trace-recoverability.md`. **ARG-005 (delta border completeness)** CLOSED (2026-04-24) — see `discussoes/argumentos/ARG-005-delta-border-completeness.md`; gates only the delta-optimized R24b path. R24/R29/EG-I3/EG-I5a/EG-P2/EG-P5 are now CLOSED for v1 and delta-conservative; only EG-I1-delta R24b-optimized path and EG-P5 delta-optimized variant remain empirically pending pending SPEC-19 implementation. See `codigo/relativist/docs/theory-bridge.md` for full bridge.
2. **SPEC-04 amendments (A3, A4)** must land in SPEC-04's next revision. ESPECIALISTA EM SPECS owns the cross-spec patch coordination.
3. **SPEC-06 amendment (A1)** and **SPEC-13 amendment (A2)** must land in their next revisions.
4. **SPEC-19 reconstruct extension** for accepting reclaimed partitions (cited in §4.2.2 delta-mode departure step 4) needs a small amendment to SPEC-19 R29; owned jointly with the SPEC-19 maintainer.
5. **Possible SPEC-20 split** into SPEC-20 (Hybrid + Joining) and SPEC-21 (Departure) — Round 2 critic considered this split. With ARG-006 now CLOSED (2026-04-24) for v1 and delta-conservative paths, the departure G1 claim is no longer CONDITIONAL for the main execution paths, so the original splitting rationale is weakened. The split remains open as an optional organizational improvement (no correctness necessity). ESPECIALISTA EM SPECS leaves this for future endorsement; no commitment.
6. **`ChannelTransport` self-worker integration with `WorkerCapabilities`** — the in-process self-worker currently uses an empty capabilities struct; a v3 enhancement might allow the self-worker to advertise extended capabilities (e.g., zero-copy via shared memory bypassing serialization). Tracked as roadmap item, not in scope.

---

## 10. New Questions From Round 2

All Round 1 OQs have been resolved per §8. Any new questions raised during Round 2 review will be appended here. Currently empty.

---

## 11. Change Log

This section maps every Round 1 finding to its outcome in this revision. CLOSED = addressed by an explicit edit in this revision; DEFERRED = explicitly scoped out with rationale and gating mechanism.

### Round 3 — 2026-04-24 (housekeeping after TCC-root theoretical work)

**Trigger:** ARG-006 written (closes R29a + R39 G1-elastic for v1 mode and delta-conservative path); ARG-005 written (separately closes SPEC-19 R38/R39/R40 and gates SPEC-20's R24b-delta optimized path); DISC-013 disambiguates ARG names.

**Changes:**
- Renamed all 17 occurrences of "ARG-005" to "ARG-006" throughout the spec where the original meaning was mixed-trace recoverability (now ARG-006 per DISC-013), in §§ 1, 3.3, 3.7, 5, 7, 8, 11. Where the meaning is genuinely SPEC-19's delta-protocol dependency (delta-optimized R24b path), the reference correctly points to ARG-005. The naming change preserves semantics for the local proof obligation; see DISC-013 (`discussoes/exploracoes/DISC-013-arg-005-disambiguation.md`) for rationale.
- Updated frontmatter "Arguments consumed:" to reflect ARG-006 as CLOSED for v1 and delta-conservative, CONDITIONAL on ARG-005 only for delta-optimized R24b path.
- Updated §3.7 G1-elastic gate table to expand into per-path rows: v1-mode CLOSED, delta-conservative CLOSED, delta-optimized CONDITIONAL on ARG-005.
- Updated R29a to explicitly state per-path closure status.
- Updated R39-G1-elastic-departure with per-mode breakdown (PRESERVED for v1/delta-conservative via ARG-006 P10/P11/P12; CONDITIONAL on ARG-005 for delta-optimized only).
- Updated §5.1 / §5.2 / §6.1 Steps 3-4 / §7 test matrix accordingly.
- Updated §9 open-issue 1 to mark ARG-006 and ARG-005 as CLOSED.
- Added cross-reference to `codigo/relativist/docs/theory-bridge.md` for absolute paths to TCC-root sources.

**Source artifacts (TCC root):**
- `discussoes/argumentos/ARG-006-mixed-trace-recoverability.md` (the mixed-trace recoverability proof; closes R29a / R39-G1-elastic-departure for v1 and delta-conservative)
- `discussoes/argumentos/ARG-005-delta-border-completeness.md` (SPEC-19's delta protocol proof; gates SPEC-20's R24b-delta optimized path)
- `discussoes/exploracoes/DISC-013-arg-005-disambiguation.md` (naming decision: SPEC-19 keeps ARG-005, SPEC-20's mixed-trace argument renamed to ARG-006)

**Round 3 spec status:** still **CONDITIONAL_PASS** from Round 2 (NF-001 `Net::union` and other Round-2 NFs remain open). This Round 3 only closed NF-002 (ARG-005 name collision) — the other Round-2 NFs are addressed by SPEC-CRITIC Round 3 in Wave 1 of the v2 Pre-DEV bundle (separate work). No semantic changes to any requirement R-NN; this is purely nomenclature + status update.

---

### CRITICAL

| Finding | Outcome | Edits |
|---------|---------|-------|
| **SC-001** Zero integration with SPEC-19 Delta Protocol | CLOSED | Added §3.0 Execution Mode Matrix (M0/R0a/R0b/R0c/R0d). Rewrote §3.1 with R4-v1/R4-delta. Rewrote §3.2 with R12-v1/R12-delta, R14-v1/R14-delta. Rewrote §3.3 with per-mode retained-state slots (R23b/R23c) and per-mode departure recovery (R24a/R24b). Updated §4.4 with delta sequence diagrams. §6.1 Step 4 covers delta-mode integration migration. |
| **SC-002** Hybrid coordinator concurrency model underspecified | CLOSED | R3 specifies the `tokio::select!` pattern across 4 arms with the FSM as single-threaded sink. R3a adds `SelfPartitionPanic` event and the `WaitingForResults × SelfPartitionPanic → Error` transition. R3b commits to processing membership/timeout events while self-reduce is in flight. R3c clarifies strict_bsp uniformity. R3 explicitly bridges via `ChannelTransport` (SPEC-17 R15) so self-worker IS a worker, not a special case. |
| **SC-003** Retained-partition semantics violate D3/D5 | CLOSED | R23 split into `retained_initial` (R23b) and `retained_last_acked` (R23c) per §3.3.3. R24c adds the D3-elastic invariant: reclaimed state is re-introduced via re-`split` at a clean boundary, NEVER mixed-merged in-round. R24d adds border_id rebase via SPEC-04 amendment A3. R31 atomic refresh closes the D5 transient-double-ownership concern. |
| **SC-004** Wire protocol incomplete | CLOSED | R35 defines all five new variants (`JoinRequest`/`JoinAck`/`LeaveRequest`/`LeaveAck`/`JoinNack`) with discriminants 12-16 (after SPEC-19's 7-11). R37 bumps `PROTOCOL_VERSION` from 3 to 4. R35a adds explicit ack semantics. R21's `LeaveRequest` no longer carries redundant `worker_id`. R37a clarifies `Register` vs `JoinRequest` selection. |
| **SC-005** ARG-001 Passo 4 cited outside scope | CLOSED via honest deferral (Round 2) + CLOSED via ARG-006 (Round 3, 2026-04-24) | R29 acknowledges that mixed-trace recoverability is NOT a corollary of Passo 4. R29a originally opened ARG-006 (renamed from ARG-005 per DISC-013) and gated D-008 sign-off on either ARG-006 landing or scope reduction. **ARG-006 landed 2026-04-24**; R39-G1-elastic-departure now marks G1 PRESERVED for v1/delta-conservative via ARG-006 P10/P11/P12; only delta-optimized R24b path remains CONDITIONAL on ARG-005. §5.1 updated. |
| **SC-006** WorkerId vs partition_index conflation | CLOSED | R11/R11a decouple `WorkerId` (sparse, monotonic, never-reused, starts at 1) from `partition_index` (dense, `[0, K_eff)`, the position in the sorted active set). New D4-elastic sub-invariant in §3.7. R8/R13/R30 align with SPEC-04's actual `compute_id_ranges(K_eff)` signature. New test EG-U12a covers the {0,1,5,7} sparse-WorkerId scenario. |

### HIGH

| Finding | Outcome | Edits |
|---------|---------|-------|
| **SC-007** Join window boundary not testable | CLOSED | R10/R10a/R10b specify the drain-then-arm protocol with `join_window_min=50ms` / `join_window_max=500ms`. R10b enumerates the FSM transitions for `WorkerJoined` in non-AcceptingMembershipChanges states (also closes SC-012). New test EG-U6a injects `tokio::yield_now()` to force the boundary race. |
| **SC-008** Graceful departure split-mode ambiguous | CLOSED | R21 adds `LeaveKind { AfterResult, Urgent }` to the wire schema. R22a/R22b/R22c split the cases with explicit FSM transitions (§4.1.4). New tests EG-U10a/EG-U10b cover Urgent in both modes; EG-U10c covers the AfterResult-but-no-result-received upgrade. |
| **SC-009** Solo-mode preemption underspecified | CLOSED | R5 (rewritten) commits to `reduce_n(solo_budget)` loop with `solo_budget=10000` default. R5a defines the two termination conditions (Local Normal Form OR `WorkerJoined`). R15 specifies the `SoloReducing × WorkerJoined → CheckTermination` transition with batch-completion rule. New `SoloReducing` state (closes SC-018). New test EG-U1a. |
| **SC-010** `hybrid_coordinator` default contradicts v1 baseline | CLOSED | R33a defaults table sets `hybrid_coordinator: false`. §6.1 Step 1 updated. EG-B1 explicitly runs both `--hybrid` and `--no-hybrid`. |
| **SC-011** Silently supersedes SPEC-06 R25 / SPEC-13 R21 | CLOSED | New §3.8 "Amendments to Predecessor Specs" lists A1 (SPEC-06 R25 conditional clause), A2 (SPEC-13 R21 new transitions/states/events/actions), A3/A4 (SPEC-04 amendments), A5 (SPEC-05 GridConfig extension), A6 (SPEC-19 metric coexistence). ESPECIALISTA EM SPECS will issue forward-reference patches in the predecessor specs. |
| **SC-012** Missing `WaitingForResults × WorkerJoined` transition | CLOSED | R10b enumerates `WorkerJoined` handlers for `Partitioning`, `Dispatching`, `WaitingForResults`, `Merging` (all → same state, action `QueueWorkerForNextWindow(id)`). FSM is now total over the new event set. |
| **SC-013** R31 SHOULD release under-constrained | CLOSED | R31 upgraded SHOULD → MUST with precise atomic-refresh semantics (release `retained_last_acked[w]_round_n` only after round N+1 dispatch fully transmitted). New test EG-U13. |
| **SC-014** `compute_id_ranges` signature drift | CLOSED | R8/R13/R30 corrected to use SPEC-04's actual `compute_id_ranges(K_eff) -> Vec<IdRange>` signature. R8 also clarifies the post-call step to set sub-net `next_id` per SPEC-04 R18. |
| **SC-015** Test plan missing mixed matrix proptests | CLOSED | New EG-P4 (full matrix `hybrid × strict × delta × join × leave × K`), EG-P5 (CON-DUP-heavy + churn), EG-P6 (delta-mode elastic). |
| **SC-016** `worker_id = 0` double-booking with SPEC-04 | CLOSED | R2a explicitly defines `WorkerId = 0` semantics per mode. R7a confirms reservation. New test EG-U1b cross-mode. R38b's `is_coordinator_self` field on `WorkerRoundStats` keys log analysis on role, not WorkerId value (also closes SC-027). |

### MEDIUM

| Finding | Outcome | Edits |
|---------|---------|-------|
| **SC-017** `Shutdown` overloaded for graceful-leave | CLOSED | R35a explicitly reserves `Shutdown` for coordinator-initiated termination ONLY; `LeaveAck` (discriminant 15) is the dedicated graceful-leave ack. §4.4.5 diagram updated. |
| **SC-018** Solo-mode FSM transition missing | CLOSED | New `SoloReducing` state in §4.1.1. Transitions in §4.1.4 cover `WaitingForWorkers × InitialWaitTimeout → SoloReducing` (R6), `SoloReducing × SoloReductionComplete → Done`, `SoloReducing × WorkerJoined → CheckTermination` (R15). |
| **SC-019** GridMetrics interaction with SPEC-19 | CLOSED | R38a explicit additivity statement. Field-name audit performed (no current overlap). |
| **SC-020** `initial_wait_timeout` vs `worker_connect_timeout` | CLOSED | R6 upgraded SHOULD → MUST. R6 explicit: `initial_wait_timeout` supersedes `worker_connect_timeout` whenever `hybrid_coordinator = true`. R33a defaults locked in. New test EG-U18. |
| **SC-021** Mixed merge signature undefined in SPEC-05 | CLOSED | R24c/R24d eliminate mixed-merge entirely (D3-elastic invariant: re-introduce via re-`split` at clean boundary). Border_id rebase via SPEC-04 amendment A3. New test EG-U7a. |
| **SC-022** TimerKind not in SPEC-13 | CLOSED | New §4.1.3 introduces `TimerKind { InitialWait, JoinWindowMin, JoinWindowMax, Collect }` enum; all transitions in §4.1.4 use typed names. |
| **SC-023** Coordinator memory under churn | CLOSED | R11 caps `next_worker_id` at `u32::MAX` with `JoinNack { WorkerIdSpaceExhausted }` rejection (R35a). R31 atomic refresh bounds `retained_last_acked` to O(K_eff active workers). `retained_initial` bounded by O(sum partition sizes for ever-active workers, which is capped by net size). New test EG-U14. |

### LOW

| Finding | Outcome | Edits |
|---------|---------|-------|
| **SC-024** OQ-2 self-partition smaller-than-partition-0 | CLOSED in OQ-2 resolution | R2/R2a fix `partition_index = 0` for self-partition; smaller-partition optimization deferred to v3. |
| **SC-025** §5.3 comparison missing SPEC-19 columns | CLOSED | §5.3 now has four columns: v1, v2-elastic-only, v2-delta-only, v2-elastic+delta. |
| **SC-026** OQs unresolved before Stage 1 | CLOSED | All five OQs resolved in §8 with explicit cross-references to the resolving requirements. |
| **SC-027** `worker_id = 0` not uniquely identifiable in logs | CLOSED | R7 + R38b add `is_coordinator_self: bool` to `WorkerRoundStats`. |

### Untestability Catalog Outcomes

| Item | Outcome |
|------|---------|
| R3 ("not concurrent" testability) | CLOSED via R3 `tokio::select!` pattern + R3a panic event; tested by EG-U16. |
| R6 SHOULD → MUST | CLOSED in R6. |
| R10 OQ-1 dependency | CLOSED in R10a/R10b. |
| R15 solo→grid | CLOSED in R5a + R15. |
| R22 split-mode | CLOSED in R22a/R22b/R22c. |
| R24 ARG-001 Passo 4 reliance | CLOSED via R29a gating on ARG-006 (CLOSED 2026-04-24); delta-optimized path additionally gated on ARG-005 (CLOSED 2026-04-24). |
| R29 informative | Unchanged (informative remains informative). |
| R31 SHOULD → MUST | CLOSED in R31. |
| R37 informative | Unchanged. |
| OQ-1, OQ-2, OQ-3, OQ-5 | All resolved per §8. |

### Net edit count

- 6 CRITICAL: 6 CLOSED, 0 DEFERRED.
- 11 HIGH: 11 CLOSED, 0 DEFERRED.
- 9 MEDIUM: 9 CLOSED, 0 DEFERRED.
- 4 LOW: 4 CLOSED, 0 DEFERRED.
- **30/30 findings addressed in Round 2.**

### Carry-forward to Round 3 / future work

- Formal proof discharge of ARG-006 (the CONDITIONAL items renamed from "ARG-005" per DISC-013). **CLOSED 2026-04-24** via `discussoes/argumentos/ARG-006-mixed-trace-recoverability.md`. ARG-005 (delta border completeness, SPEC-19) also CLOSED 2026-04-24 via `discussoes/argumentos/ARG-005-delta-border-completeness.md`; gates only the delta-optimized R24b path.
- Predecessor-spec patches (A1-A6) need to land in SPEC-04, SPEC-05, SPEC-06, SPEC-13, SPEC-19 next revisions.
- Possible SPEC-20 split (open issue 5 in §9) for spec-critic endorsement decision.
