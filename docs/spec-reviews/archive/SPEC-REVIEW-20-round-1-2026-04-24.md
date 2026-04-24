# SPEC-REVIEW-20 — Round 1: Elastic Grid — Adversarial Review

**Date:** 2026-04-24
**Reviewer:** spec-critic (adversarial, Stage 0 of D-006/D-007/D-008 SDD cycle — shared review)
**Target:** `specs/SPEC-20-elastic-grid.md` (598 lines, Status: Draft, v1 — never before adversarially reviewed)
**Bundles covered:** D-006 (§3.1 Hybrid Coordinator), D-007 (§3.2 Dynamic Join), D-008 (§3.3 Dynamic Departure)
**Predecessors consulted:** SPEC-00, SPEC-01 (T1-T7, D1-D6, I1-I7, G1), SPEC-04 (partition), SPEC-05 (merge, GridConfig, strict_bsp), SPEC-06 (Message FSM, discriminant stability), SPEC-13 (coordinator/worker FSMs, Transport), SPEC-17 (Transport trait), SPEC-19 (Delta Protocol, BorderGraph, `run_grid_delta`), ARG-001 (P1-P6), `docs/plans/2026-04-24-tier-4-master-plan.md`, SPEC-REVIEW-19 §3.4 (R1/R2/R3 as rigor calibration).

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 6 |
| HIGH     | 11 |
| MEDIUM   | 9 |
| LOW      | 4 |
| **Total**| **30** |

**Gate decision: BLOCK.**

SPEC-20 was drafted on 2026-04-15 — nine days *before* SPEC-19 Delta Protocol (D-005) shipped on 2026-04-24. **SPEC-20 is written exclusively against v1 `run_grid` and has zero awareness of `run_grid_delta`, `BorderGraph`, or stateful workers.** Every single requirement that references "the merged net", "re-partition", "retained partition", or "split()" is silently incompatible with the delta execution path that is now the production default for break-even. Grepping SPEC-20 for `run_grid_delta`, `BorderGraph`, `SPEC-19` — zero hits. Grepping for `strict_bsp` — one hit, and only as a comment placeholder.

Beyond the delta-protocol gap, SPEC-20 has architecturally ambiguous concurrency in §3.1 (spawn_blocking + async collection without a documented rendezvous for mid-reduce worker messages), under-specified wire schemas (`JoinRequest`/`JoinAck`/`LeaveAck` mentioned in the master plan are missing from the spec; `LeaveRequest` has no companion ack), and at least one likely-violated invariant in §3.3 (D3 "completeness of border redex resolution" is not preserved when retained partitions are re-merged with partially-reduced ones — the spec's ARG-001 Passo 4 citation is asserted, not proven, and the border-redex bookkeeping is missing entirely).

The three-round SPEC-19 §3.4 cycle was driven by a single wire-schema misalignment. SPEC-20 has *multiple* such misalignments plus a whole-architecture mismatch with the delta protocol. A single Round 2 pass is unrealistic; Round 2 must re-scope the spec against SPEC-19, and Round 3 will still be needed to close residual testability gaps.

---

## Findings

### SC-001 (CRITICAL) — SPEC-20 has zero integration with SPEC-19 Delta Protocol

**Severity:** CRITICAL
**Axis:** E (interaction with recently-closed features)
**Location:** entire spec; especially §3.1 R4 ("merge all partitions"), §3.2 R12 ("re-partition … at the start of the next round"), §3.3 R23 ("retain a copy of each partition dispatched"), §4.2 ("re-partition algorithm"), §6 "Migration Path".
**Problem:** SPEC-20 assumes the v1 BSP cycle `split → dispatch → reduce_local → collect → merge → reduce_all(merged)`. Under the delta protocol (SPEC-19 §3.3, R20-R30), the coordinator **does not hold the full merged net between rounds** — it holds a `BorderGraph`; workers are stateful and keep their partitions across rounds; the cycle is `InitialPartition (round 0) → {RoundStart(border_deltas) → RoundResult(border_deltas)} → FinalStateRequest → FinalStateResult → merge`. None of SPEC-20's requirements compose with this model:

- **§3.1 R4 "merge all partitions each round"** contradicts SPEC-19 R20-R22: there is no per-round merge; the merge happens once, at the end, via `FinalStateRequest`.
- **§3.2 R12 "re-partition at the next round"** requires calling `split()` on "the merged net from the completed round" (§4.2.1 step 3), but in delta mode the coordinator has no merged net — only a `BorderGraph` and K stateful worker partitions.
- **§3.3 R23 "retain a copy of each partition"** was defined for v1, where the coordinator sends a fresh `AssignPartition` every round. In delta mode, the worker keeps its partition. "Retained" is ambiguous: is it the round-0 `InitialPartition` snapshot, or the partition at round N-1? Both have different correctness implications.
- **§3.3 R24 "reclaim the departed worker's partition using the retained copy"** is operationally meaningless in delta mode: the retained copy is round-0 state; other workers have advanced by N rounds; merging a round-0 partition with round-N partitions violates the recoverability claim in SPEC-19 R38.

**Impact if unresolved:** D-006/007/008 cannot coexist with SPEC-19 in a single binary. Either Elastic Grid silently requires `--no-delta` (defeating the break-even narrative), or the v2 pipeline produces non-canonical results on mid-run join/leave — exactly the G1 violation the TCC thesis disproves.

**Suggested fix (mandatory):**
1. Add an explicit §3.0 "Execution Mode Matrix" clarifying SPEC-20 semantics under each of the four mode combinations: `{v1 full-merge, delta} × {strict_bsp true, false}`.
2. For delta mode, rewrite §3.2 (join) as: "New worker receives an `InitialPartition` derived by splitting the `BorderGraph`-reconstructed state — see SPEC-19 R38 recoverability — at the end of the current delta round, and enters the stateful protocol from the next round forward." Requires coordinating with SPEC-19's amendment path to add a "join-snapshot" primitive or reusing `reconstruct(border_graph, worker_partitions)` (SPEC-19 R38).
3. For delta mode, rewrite §3.3 (departure) as: "Retained round-0 `InitialPartition` is NOT sufficient; the coordinator MUST also retain either (a) the last `RoundResult` border deltas reported by the departing worker, or (b) trigger a `FinalStateRequest` at round N-1 and checkpoint the result." Pick one; document the memory cost trade-off.
4. §3.1 (hybrid coordinator) must be rewritten as "coordinator runs a local worker instance that speaks the delta protocol to itself via `ChannelTransport` (SPEC-17 R15)." This is the only design that composes cleanly with SPEC-19; the current spec's "merge self-partition identically to remote partitions" (R4) only works under v1.
5. Explicit declaration: "If SPEC-19 delta mode is disabled (`--delta false`), SPEC-20 reverts to the v1 cycle exactly as written in §3.1-§3.3." This preserves the v1 compatibility mode claimed in §6.

**Invariant affected:** G1 (potentially VIOLATED in delta mode if R4/R24 are applied literally because the recoverability argument of SPEC-19 R38 breaks when a round-0 snapshot is merged with round-N state).

---

### SC-002 (CRITICAL) — Hybrid coordinator concurrency model is underspecified; async message pump vs. self-reduce blocking

**Severity:** CRITICAL
**Axis:** C (FSM/protocol correctness), G (scope discipline)
**Location:** §3.1 R3, §3.1 implementation note, §4.1.4 "Hybrid Dispatch Flow", Open Question OQ-3.
**Problem:** R3 says the coordinator "MUST NOT block on its own local reduction while dispatching partitions to or collecting results from remote workers" and the implementation note suggests `spawn_blocking(reduce_all(self_partition))` awaited concurrently with remote `PartitionResult` futures. The spec does not answer these operational questions:

1. **Mid-reduce async events.** While the self-partition `reduce_all` is running on a `spawn_blocking` thread, can the async event loop process `LeaveRequest` (SPEC-20 §3.3), `WorkerJoined` (§3.2), `PartitionResult`, and `PhaseTimeout` events? If yes — where are they buffered, and what happens if the hybrid coordinator's self-reduce completes *after* a departure timeout for one of its own expected-remote results? If no — the async runtime is effectively single-threaded for membership events, and the claimed "K+1 effective workers" is a lie (it is "K workers + 1 CPU-bound blocking task that freezes the FSM").
2. **Self-partition reclaim on coordinator failure.** If the spawn_blocking task panics or the runtime is torn down mid-reduce, the coordinator state is "waiting for K_eff results, one of which never arrives." R18-R19 (timeout detection) only apply to *remote* workers (they require TCP-level detection); nothing in R18-R19 fires on a local panic.
3. **Interaction with SPEC-13 R32 ("single tokio runtime")** and SPEC-13 R34 ("Core Layer MUST be fully synchronous"). `reduce_all` is Core and must not see tokio. The spec says "spawn_blocking" but R3 does not cite SPEC-13 R32/R34; the caller-side async↔sync bridge is undefined.
4. **Ordering against strict_bsp.** When `GridConfig.strict_bsp = true` (SPEC-05 R30a), the post-merge reduce_all is skipped. In hybrid mode, does this mean the self-partition's border-derived redexes are also skipped? If yes, the coordinator's self-partition contributes to strict-mode cascades *differently* from remote partitions, breaking R4's claim that "self-partition MUST be treated identically to any remote partition."

**Impact if unresolved:** Either a deadlock on self-reduce completion after remote timeout, or an unbounded message queue holding departure/join events until the blocking task returns, or a silent departure of the self-partition that no test can observe. The test plan (EG-I1) only checks canonical equivalence of end state; it cannot catch a mid-round coordinator freeze that still converges correctly.

**Suggested fix (mandatory):**
1. §3.1 must specify the exact `tokio::select!` pattern for hybrid mode, e.g., "the coordinator's main event loop `tokio::select!`s over: (a) `PartitionResult` receives, (b) the `JoinHandle` of the self-partition spawn_blocking task, (c) worker connection events, (d) timer events. All four arms produce events that flow into the pure FSM transition function (SPEC-13 R20)."
2. Add new event: `SelfPartitionPanic(String)` (currently the only new event is `SelfPartitionReduced(Partition)` per §4.1.2). Add new FSM transition `WaitingForResults × SelfPartitionPanic → Error`.
3. Explicitly state which events the coordinator processes *while* the spawn_blocking task is in flight, and prove (by construction) that a departure timeout during self-reduce is handled correctly.
4. Clarify strict-mode interaction: "In strict mode, the self-partition's border-origin redexes are enqueued into the merged redex queue identically to any remote partition's border-origin redexes. The self-partition does NOT benefit from special short-circuit treatment."

**Invariant affected:** D6 (protocol termination) AT_RISK if mid-reduce events are silently dropped.

---

### SC-003 (CRITICAL) — Partition retention semantics violate D3 and D5 when re-dispatched after a worker departure

**Severity:** CRITICAL
**Axis:** A (invariant preservation), B (confluence dependency)
**Location:** §3.3 R23-R26, §3.3 R29 (at-least-once semantics), §3.7 R39 "D3 preserved" and "D5 preserved" claims, §4.2.2 step 4, §5.1 bullet 3.
**Problem:** The spec claims "strong confluence guarantees that re-dispatch from the original state is safe" (R24, citing ARG-001 Passo 4 and ARG-004 Passo 12). This is partially true for the final Normal Form, but it is **silent on two mechanisms that both D3 and D5 require to be explicit**:

1. **D3 (border redex completeness):** When the retained unreduced partition of a departed worker is merged with the partially-reduced partitions of surviving workers, the border wires that crossed from the departed partition into surviving ones **no longer have the topology expected by the survivors' reduced state**. Concretely: if surviving worker A reduced its partition down by k steps, the FreePort(bid) in A's partition may now point to an agent `a'` that did not exist at round 0. When the coordinator merges A's evolved partition with the departed worker's round-0 retained copy, the FreePort(bid) on the departed side still points to round-0 agents. The merge (SPEC-05 R4-R5) will reconnect the two sides via `Net::connect`, producing a wire between `a'` (post-reduction) and a round-0 agent that may no longer be reachable from anywhere. This is *not* an invariant violation of T1/I1 (the port array is still bidirectional), but it is a **semantic mismatch** that the spec must analyze: does the resulting net have extra agents that will never reduce? Do the surviving side's border redexes that were already resolved need to be un-resolved?

   The spec hand-waves this with "strong confluence guarantees correctness" but ARG-001 Passo 4 applies to reduction of the *same* initial state; here we are mixing **two different reductive traces into one merged net** — that requires a *new* argument that is not in ARG-001.

2. **D5 (exclusive agent ownership):** R29 claims "the departed worker's results are discarded entirely (never merged), there is no risk of ID collision from partially-created agents." This is false as written. The departed worker's results are indeed discarded, but **if the departed worker already sent a partial result before departing (e.g., it sent PartitionResult at round N-1 and departed at round N)**, the merged net at round N-1 already contains agents with IDs in the departed worker's range. At round N+1's re-dispatch, the retained-partition's round-0 agents are IDs within the same range. A re-dispatch that assigns these round-0 IDs to a *different* worker's ID range (via `compute_id_ranges(K_eff-1, merged.next_id)` per R30) will renumber them — fine — but if the retained partition is merged back *without* renumbering (as §4.2.2 step 5 implies), the ID-range-to-worker mapping is corrupted. There is a window where "agent X in round N+1's partition is a descendant of agent Y in round N-1's merged net, whose ID range now belongs to a joining worker." D5 does not trivially hold.

3. **R30 "ID Uniqueness":** is particularly subtle — it claims no collision because "departed worker's results are discarded entirely." But SPEC-20 §3.3.2 R22 explicitly allows a departing worker to send `PartitionResult` for the current round *and then* send `LeaveRequest`. So at round N, worker B's current-round results ARE merged (it was graceful), only future rounds use the retained copy — but the spec's R24 does not distinguish "which round's retained copy" we re-dispatch.

**Impact if unresolved:** G1 VIOLATED. The test plan has EG-U7/EG-U8/EG-I3 checking "final result matches `reduce_all`" but those tests only cover abrupt crash departures. Graceful-departure-then-re-dispatch, and mixed-round retained partitions, are untested and likely broken.

**Suggested fix (mandatory):**
1. Split retained partition into **two distinct concepts**:
   - `retained_initial: Partition` — the round-0 dispatch copy, used only on catastrophic departure before any PartitionResult is received.
   - `retained_last_acked: Partition` — the most recent PartitionResult for this worker, used on departures after at least one round was completed.
   Specify which one is used in which departure scenario, with a requirement per scenario.
2. Add a new formal sub-invariant "D3-elastic": "When a departed worker's retained partition is re-dispatched, the coordinator MUST NOT merge it with the surviving workers' evolved partitions in the same round. Instead, it MUST be inserted as a new partition at the *next* round boundary, where all partitions will be re-split from the merged net by `split()`. This preserves D3 by restoring a uniform partition state before reduction resumes."
3. Explicitly document the ARG-001 extension required: "re-execution of lost work is safe provided (i) the retained partition is merged only with partitions that were themselves reduced from the *same* initial state, and (ii) ID ranges are re-computed to cover both surviving and retained agents without overlap." Without this formal extension, the claim of confluence safety is unsupported.
4. Add tests for the mixed-round scenario: EG-U14 (graceful departure at round N, re-dispatch at round N+1), EG-U15 (catastrophic departure at round N mid-reduce, retained round-0 copy re-dispatched at round N+1).

**Invariant affected:** D3 AT_RISK, D5 AT_RISK, G1 AT_RISK.

---

### SC-004 (CRITICAL) — Wire protocol schemas are incomplete: missing `JoinRequest`, `JoinAck`, `LeaveAck`

**Severity:** CRITICAL
**Axis:** D (wire protocol ambiguity)
**Location:** §3.5 R35, §4.1.2 `WorkerJoined(WorkerId)`, §4.4.2 diagram (`Register`/`RegisterAck`), master plan §2 D-007 ("`JoinRequest`, `JoinAck`") and D-008 ("`LeaveRequest`, `LeaveAck`").
**Problem:** The master plan explicitly says D-007 introduces `JoinRequest` + `JoinAck` and D-008 introduces `LeaveRequest` + `LeaveAck`. SPEC-20 defines **only `LeaveRequest`**. Nothing defines:

- **`JoinRequest` / `JoinAck`:** §3.2 R11 says the joining worker "MUST complete the registration handshake (SPEC-06, R2a; SPEC-10 if auth is enabled)." But SPEC-06 R2a's handshake is `Register` / `RegisterAck` — designed for *initial* worker registration *during WaitingForWorkers*. The spec does not say whether a mid-session join reuses `Register/RegisterAck` or introduces new variants. §4.4.2 shows `Register`/`RegisterAck` for a mid-session join, implying reuse. If reuse is intended, say so *explicitly* and state that §3.2 R11's join is exactly the SPEC-06 R2a handshake re-run. If new variants are intended (as the master plan says), define their schemas.
- **`LeaveAck`:** §3.3.2 R20 says "the coordinator MUST acknowledge receipt" but R21 only defines `LeaveRequest`. The acknowledgment mechanism is undefined. Does the coordinator send a `LeaveAck`, or reuse `Shutdown` (as §3.7 R37 hints: "existing `Shutdown` sufficient")? Without a distinct ack, a race arises: worker sends LeaveRequest, immediately closes connection without waiting for ack; coordinator's `WorkerLeft` event fires but worker is already gone; coordinator's later `Shutdown` send fails silently.

**Additional sub-issues:**
- R35 defines `LeaveRequest { worker_id }` — redundant, because the coordinator already knows the worker_id of the connection it received the message on. This is a signal that the wire schema was not actually thought through.
- No discriminant number is specified. SPEC-19 R31-R32 assigned discriminants 7-11. SPEC-20 must continue from 12. The spec does not say so.
- No protocol version bump is specified. SPEC-19 R37 bumped PROTOCOL_VERSION from 2 to 3 because adding a trailing field broke bincode decode. SPEC-20 adds a whole new variant; a version bump is required (or an explicit justification that v3 coordinators can receive `LeaveRequest` from v3 workers without issue).

**Impact if unresolved:** Implementation will guess, and the guess will not match what Stage 2 test-generator assumes. Protocol version mismatch may not be caught at runtime; wire serialization may regress.

**Suggested fix (mandatory):**
1. Define all four variants concretely in §3.5 with exact bincode schemas:
```
/// Discriminant: 12
JoinRequest { auth_token: Option<[u8; 32]> }
/// Discriminant: 13
JoinAck { worker_id: WorkerId, round: u32 }
/// Discriminant: 14
LeaveRequest
/// Discriminant: 15
LeaveAck
```
2. Justify the choice (new variants vs. reuse of Register/RegisterAck). The review recommends NEW variants because Register/RegisterAck have authentication-related payloads that don't fit mid-session scenarios cleanly.
3. Bump PROTOCOL_VERSION from 3 to 4. Add a requirement: "R36a: A v3 worker connecting to a v4 coordinator (or vice versa) MUST be rejected at the Register handshake (NF-002, SPEC-19 R37 pattern)."
4. Add explicit ack semantics: "R20a: The coordinator MUST send `LeaveAck` before closing the connection. The worker MUST NOT close its connection before receiving `LeaveAck`. This prevents the race where the worker disappears before the coordinator's bookkeeping is updated."

**Invariant affected:** I4 (wire serialization integrity) if unversioned.

---

### SC-005 (CRITICAL) — §3.3 R24 invokes ARG-001 Passo 4 outside its scope; confluence proof for mixed-trace merges is not in ARG-001

**Severity:** CRITICAL
**Axis:** B (confluence dependency)
**Location:** §3.3 R24, R29, §3.7 R39 (D6 argument), §5.1 bullets 2 and 3, §5.2.
**Problem:** Multiple requirements cite ARG-001 Passo 4 as the justification for safety of re-dispatch and re-execution. Passo 4 of ARG-001 states: "the order and distribution of work are irrelevant for the final result, given strong confluence." This is an order-of-reduction property. It does NOT address:

1. **Mixed intermediate state:** Passo 4 assumes a single initial net `μ` and reasons about different reduction strategies leading to the same normal form. §3.3 R24 merges a round-0 partition (the retained copy) with round-N partitions from survivors. This produces a net that is the image of `μ` under *two different* partial reduction traces. Passo 4 is silent on whether such a net converges to NF(μ).
2. **Non-disjoint work:** Passo 4 relies on Passo 2 (disjointness of redexes). When a retained round-0 partition is re-dispatched, its agents may have already been referenced (as border endpoints) by the survivors' evolving states. The coordinator's merge step must re-establish the FreePort bijectivity (SPEC-04 C3) under this non-standard condition.

§5.1 bullet 3 states "Re-execution is safe. If a worker's result is lost, re-reducing from the original partition state produces the same contributions to the global reduction (ARG-004, Passo 12)." ARG-004 Passo 12 is a stronger claim than Passo 4 — it specifically discusses re-execution — and SPEC-20 *should* be citing Passo 12 consistently, not Passo 4. Even then, Passo 12's argument relies on a carefully defined "rollback point"; SPEC-20 does not define a rollback point and implicitly assumes round 0 is always correct.

**Impact if unresolved:** The entire §3.3 departure recovery correctness argument rests on a theorem that has not been proved for this specific operational pattern. If an adversarial workload can produce a departure pattern that violates confluence by mixing traces (e.g., CON-DUP cascades that create emergent borders in both the retained partition and the survivors' evolving states), G1 fails. The test plan's EG-I3 ("simulate 1 departure at round 2") is too weak — it doesn't exercise CON-DUP emergent borders with retained partitions.

**Suggested fix (mandatory):**
1. Add an explicit new Passo to ARG-001 (or a new ARG-005) that proves: "given terminating net μ, strong confluence, and a sequence of `(partition, reduction)` steps where some partitions are reset to an earlier state (round 0) and re-dispatched, the resulting reduction converges to NF(μ)." This is a formal deliverable, not a spec edit.
2. Until the proof is delivered, mark §3.3 as "requires ARG-001 extension (pending)" and gate D-008 on that proof landing. The existing precedent is SPEC-19 R38 ("proof pending") — SPEC-20 must follow the same honesty standard.
3. Replace the ARG-001 Passo 4 citations in R24/R29/§5.1 with ARG-004 Passo 12 (re-execution) and a forward reference to the pending proof.
4. Add tests specifically designed to stress mixed-trace correctness: EG-I5a (CON-DUP cascades during retained-partition re-dispatch), EG-I5b (emergent borders across retained + evolved partitions).

**Invariant affected:** G1 UNDEFINED until proof lands.

---

### SC-006 (CRITICAL) — §3.2 R11 "monotonic WorkerId counter" is undefined for joined workers vs. initial workers

**Severity:** CRITICAL
**Axis:** A (invariant preservation), C (FSM correctness)
**Location:** §3.2 R11 + implementation note, §3.1 R2 ("self-partition MUST be assigned `worker_id = 0`"), §3.2 R12, SPEC-04 R16-R19 (ID range computation tied to worker index).
**Problem:** §3.2 R11 says joined workers receive monotonically increasing `WorkerId` values not reused. But:

1. §3.1 R2 **hard-codes `worker_id = 0`** for the coordinator's self-partition. This means `WorkerId=0` is reserved forever (good), but the "monotonic from coordinator-local counter" starts where? At 1, or does it include 0? Spec does not say.
2. `compute_id_ranges(K_eff_new, merged.next_id)` (R13) computes ID ranges by `worker_index * chunk_size` (SPEC-04 R18). If `K_eff_new = 3` but the active WorkerIds are `{0, 1, 4}` (1 left, 2 joined and left, 4 joined fresh), should the ID range computation use `worker_id` or a re-indexed position `[0..K_eff_new)`? The former causes gaps (`WorkerId=4` gets the 5th chunk of u32 space, when only 3 workers exist); the latter contradicts R11's "never reuse" principle.
3. §3.3 R30 says "ID ranges MUST be recomputed via `compute_id_ranges(K_eff_new, merged_net.next_id)`" — same ambiguity.

In practice, SPEC-04 R18's formula `range_i = [i * chunk_size, (i+1) * chunk_size)` uses `i` as a **position index** from 0 to K-1. If a joined worker gets WorkerId 7, using it as `i` means it gets the range `[7 * chunk_size, 8 * chunk_size)` — fine until a following worker tries the same, and the ID space is exhausted at chunk index `K_max_ever_connected`, not `K_currently_active`.

**Impact if unresolved:** Two workers can receive overlapping ID ranges across rounds, violating D4 (ID uniqueness) and corrupting the merged net. Alternatively, the ID space is exhausted prematurely. Either way, tests EG-U12 (ID ranges disjoint) will fail or hide the bug.

**Suggested fix (mandatory):**
1. Clarify in §3.2 R11: "WorkerId values are assigned monotonically from a counter starting at 1 (0 is reserved for the coordinator self-partition per §3.1 R2)."
2. Split WorkerId from `partition_index` explicitly: define "the partition_index of a partition is its position in the round's `ActiveWorkerSet` sorted by WorkerId, NOT the WorkerId itself." `compute_id_ranges` uses `partition_index`.
3. Add a new invariant: "D4-elastic: WorkerIds may be sparse; partition_indices are dense. ID range computation uses partition_index, not WorkerId. A departed WorkerId's ID range is retired (not reused)."
4. Add test EG-U12a: with WorkerIds `{0, 1, 5, 7}` and K_eff=4, verify ranges are `[0, chunk), [chunk, 2*chunk), [2*chunk, 3*chunk), [3*chunk, 4*chunk)` and none are tied to WorkerId 5 or 7.

**Invariant affected:** D4 (ID uniqueness) VIOLATED as written.

---

### SC-007 (HIGH) — §3.2 R16 "join window" boundary is textually defined but not operationally testable

**Severity:** HIGH
**Axis:** C (FSM correctness), F (testability)
**Location:** §3.2 R10, R16, §4.1.3 transition "`CheckTermination × elastic=true → AcceptingMembershipChanges`", §4.1.3 transition "`AcceptingMembershipChanges × MembershipWindowClosed → Partitioning`", OQ-1.
**Problem:** The Join Window is "the interval between the completion of the merge/check-termination phase and the start of the next partition/dispatch phase" (R10). The FSM implements this as a new state `AcceptingMembershipChanges`. OQ-1 is explicitly unresolved: "How long should the window remain open? Options: (a) fixed timer, (b) immediate close, (c) configurable."

1. Without a resolution to OQ-1, Stage 1 (task-splitter) cannot determine whether the window uses a timer (needs `StartTimer`/`MembershipWindowClosed` events) or is polled (needs a `poll_pending_connections` action). The §4.1.3 transition table lists `StartTimer(join_window_timer)` as an action, but R10 does not mandate the timer approach.
2. **Boundary race:** §3.2 R9 says "the coordinator MUST accept new TCP connections." If a `JoinRequest` arrives precisely at the boundary — *after* the `MembershipWindowClosed` event fired but *before* the FSM transitioned to `Partitioning` — is it included in the current round's K_eff or the next? The spec does not say. EG-U6 ("worker connecting mid-round is queued") tests the post-dispatch case but not the transition-edge case.
3. §3.2 R16 says "workers that connect during an active round MUST be queued and included only at the next join window." This contradicts §4.1.3's transition `AcceptingMembershipChanges × WorkerJoined(id) → AcceptingMembershipChanges` which registers them immediately. The spec needs to clarify: a TCP connection during `WaitingForResults` is queued; a `WorkerJoined(id)` event in `AcceptingMembershipChanges` is registered. The queue-to-event pipeline is undefined.

**Impact if unresolved:** Implementation will inconsistently handle boundary joins. EG-U6 tests the simple case; a concurrent multi-worker join at the window boundary is untested.

**Suggested fix (mandatory):**
1. Resolve OQ-1 now, not later. Recommended: "a minimum 50ms drain window with immediate-close if no pending connections." Add explicit fields: `join_window_min: Duration` (default 50ms), `join_window_max: Duration` (default 500ms).
2. Add R10a: "When the coordinator transitions into `AcceptingMembershipChanges`, it immediately drains all pending TCP connections registered since the last dispatch. If any are drained, it waits `join_window_min` for more; it then closes the window regardless of `join_window_max`. Timer-based exit is the sole closure mechanism; polling is forbidden."
3. Add R10b: "A worker's TCP `accept()` completion during `Dispatching`, `WaitingForResults`, or `Merging` MUST be buffered in a pending-connections queue. The worker's `Register`/`JoinRequest` MUST NOT be processed during those states. Processing begins when the state is `AcceptingMembershipChanges`."
4. Add a boundary-race test: EG-U6a — simulate `tokio::task::yield_now().await` injection to force the window-close event and a `WorkerJoined` event to race; verify the worker is always included in a deterministic round (either current or next).

**Invariant affected:** D6 (protocol termination) AT_RISK if boundary races lead to lost joins.

---

### SC-008 (HIGH) — §3.3 R22 "graceful departure without PartitionResult" is ambiguous; split into current/future round

**Severity:** HIGH
**Axis:** C (FSM correctness), F (testability)
**Location:** §3.3 R22, §4.4.4 diagram (shows a different flow from R22).
**Problem:** R22 says: "A worker that sends LeaveRequest MUST first complete any in-progress reduction and return its PartitionResult for the current round before sending LeaveRequest. **If the worker cannot complete its round, it SHOULD send LeaveRequest without PartitionResult; the coordinator treats this as a timeout departure for the current round and a graceful departure for future rounds.**" The bold clause introduces a split-mode departure:

1. **"Timeout for current round, graceful for future rounds"** — this means the retained partition IS re-dispatched for the current round (timeout path, R24) AND the worker is removed from the set for the next round. No test verifies both behaviors compose correctly.
2. §4.4.4's "Graceful Departure" diagram shows the happy case: worker sends PartitionResult, then LeaveRequest. The R22 split-mode case is undiagrammed.
3. There is no mechanism to report *why* a worker cannot complete — is this reported in `LeaveRequest`? R35 defines `LeaveRequest { worker_id }` with no reason field. The coordinator cannot distinguish "cannot complete due to OOM" from "cannot complete due to user cancellation."
4. The FSM transition table (§4.1.3) for `WorkerLeft(id)` states "`StoreResult(id, ...), RemoveWorker(id), LogDeparture` | Graceful: partition already returned". The "partition already returned" assumption contradicts R22's split-mode case.

**Impact if unresolved:** Implementation will hit an unhandled case where `LeaveRequest` arrives during `WaitingForResults` without a PartitionResult from that worker. Either the coordinator deadlocks or silently drops the worker, creating an undetectable partial-result state.

**Suggested fix (mandatory):**
1. Add to R35: `LeaveRequest { worker_id: WorkerId, kind: LeaveKind }` where `LeaveKind ∈ {AfterResult, Urgent}`.
2. Split R22 into R22a (clean case, AfterResult) and R22b (urgent case, Urgent). For R22b, the FSM MUST transition as if a timeout occurred for the current round and mark the worker as departed for future rounds.
3. Add FSM transition: `WaitingForResults × WorkerLeft(id, Urgent) → WaitingForResults` with actions `ReclaimPartition(id), RemoveWorker(id), LogDeparture` (same as PhaseTimeout elastic_departure=true path).
4. Add test EG-U10a (clean AfterResult) and EG-U10b (Urgent split-mode).

**Invariant affected:** D3 (border completeness) AT_RISK for R22b case.

---

### SC-009 (HIGH) — §3.1 R5 / R6 solo-mode semantics contradict §3.2 R15 join-while-solo transition

**Severity:** HIGH
**Axis:** A (consistency), C (FSM correctness)
**Location:** §3.1 R5 ("if K==0, reduce entire net locally as if `num_workers = 1`"), §3.1 R6 ("after `initial_wait_timeout`, coordinator SHOULD begin reducing alone"), §3.2 R15 ("worker joining mid-execution MUST cause the coordinator to switch from solo mode to grid mode at the next round boundary").
**Problem:** In solo mode, the coordinator is not in BSP; it is calling `reduce_all` on the entire net without split/merge (§3.1 R5: "No split/merge overhead is incurred"). If a new worker joins during solo mode, there is no "next round boundary" because there are no rounds. R15 says "at the next round boundary, the coordinator MUST partition the net (which it has been reducing alone) for K_eff=2" — but `reduce_all` is a single synchronous call; there is no preemption point.

1. Does the coordinator check for join events between reduction steps (violating the "no overhead" claim of R5)?
2. Does the coordinator wait for `reduce_all` to fully complete (in which case, a worker joining mid-reduction won't be noticed until the reduction is done, potentially violating termination latency expectations)?
3. If the coordinator uses `reduce_n(budget)` with a small budget to get preemption points, R5's "no overhead" is false, and the behavior should be documented.

**Impact if unresolved:** In solo mode, join latency is either infinite (never noticed) or the "no overhead" optimization is silently broken. Either way, a test like "coordinator in solo mode receives join at t=1s, new round starts at t=2s with K_eff=2" has no deterministic implementation.

**Suggested fix (mandatory):**
1. Resolve the solo-mode preemption question: either (a) solo mode uses `reduce_n(budget)` with a configurable budget, trading throughput for join responsiveness, or (b) solo mode is "all or nothing" and joins during solo reduction are queued until `reduce_all` completes. Pick one.
2. Update R5 to reflect the chosen semantics: "R5 (revised): If K==0, the coordinator reduces the entire net via `reduce_n(budget=solo_budget)` in a loop, polling for join events between each budgeted batch. Default `solo_budget = 10000` interactions. Setting `solo_budget = u32::MAX` degenerates to `reduce_all` with no join responsiveness."
3. Add test EG-U1a (join during solo reduction, verify switch to grid mode).

**Invariant affected:** D6 (protocol termination) preserved either way, but test-writer semantics are not reproducible.

---

### SC-010 (HIGH) — §3.1 R6 default `hybrid_coordinator = true` contradicts §6.1 Step 1 "v1 benchmarks set it to `false`"

**Severity:** HIGH
**Axis:** A (consistency)
**Location:** §3.1 R6, §3.4 R33 (GridConfig default), §6.1 Step 1 ("v1 benchmarks set it to `false`").
**Problem:** R33 declares "hybrid_coordinator: bool, Default: true." §6.1 Step 1 says: "`hybrid_coordinator: true` for new deployments, but v1 benchmarks set it to `false`." SPEC-13/SPEC-06/SPEC-09 benchmark suite is the existing v1 baseline. If SPEC-20 lands with default true, the next `cargo bench` produces apples-to-oranges data: v1 results were without hybrid; v2 results will be with hybrid *and K workers plus coordinator CPU*.

This is not purely a config concern — it is a **benchmark correctness concern**. The "break-even" story (c_o/c_r = 2.2, documented in ROADMAP §2.40) measures coordinator overhead against remote reduction cost. Hybrid mode redefines what "coordinator" means by making it also a reducer. The ratio must be re-derived under hybrid mode or the default must remain `false`.

**Impact if unresolved:** Stage 6 benchmark data will be inconsistent; the TCC break-even claim may silently change basis.

**Suggested fix (mandatory):**
1. Change R33 default: `hybrid_coordinator: false`. Document in §6.1 Step 1: "Default preserves v1 baseline for apples-to-apples benchmark comparison. The CLI flag `--hybrid` is opt-in."
2. Add a new acceptance gate in the master plan (§2, Gate 1): "Benchmark `ep_con 1M w=2` passes with `--no-hybrid` (v1 parity) AND with `--hybrid` (improvement claim)."
3. Document in §5.3 Comparison table that "v2 hybrid" is a new row, distinct from "v2 non-hybrid" which reproduces v1 exactly.

**Invariant affected:** none directly, but experimental validity is compromised.

---

### SC-011 (HIGH) — §3.3 R18 silently supersedes SPEC-06 R25 and SPEC-13 R21 without version gate

**Severity:** HIGH
**Axis:** A (consistency with predecessors)
**Location:** §3.3 R18 "Migration note", SPEC-06 R25, SPEC-13 R21 table row `WaitingForResults × PhaseTimeout(id) → Error`.
**Problem:** R18 states: "The v1 behavior is preserved when `elastic_departure = false` (default for v1 compatibility)." This is a behavioral toggle, not a spec change, but the spec does not state:

1. Does SPEC-06 R25 formally acquire a conditional clause "unless GridConfig.elastic_departure = true"? If yes, SPEC-06 must be amended; SPEC-20 cannot unilaterally override a predecessor spec's MUST requirement.
2. Does SPEC-13 R21 acquire a new transition variant for `elastic_departure = true`? SPEC-13 is normative for the FSM; SPEC-20's §4.1.3 "extended transition table" is informative at best.

This is the exact pattern SPEC-REVIEW-19 flagged: a new spec claiming precedence over existing invariants without formal amendment. SPEC-19 was required to amend SPEC-01 D3 (R39) and G1 (R38) with explicit language. SPEC-20 does not amend SPEC-06 R25 or SPEC-13 R21.

**Impact if unresolved:** When SPEC-06 is next revised, the implementer reads SPEC-06 R25 as "MUST abort grid loop on connection loss." The fact that SPEC-20 softens this for `elastic_departure = true` is not discoverable from SPEC-06. Test generation for v1 tests against SPEC-06 R25's fatal-error behavior; v2 tests need a different expectation. The spec ownership chain is broken.

**Suggested fix (mandatory):**
1. Add an explicit "§3.8 Amendment to Predecessor Specs" section to SPEC-20 listing: "R18 amends SPEC-06 R25 by adding the conditional clause 'unless `elastic_departure = true`, in which case the `WaitingForResults × PhaseTimeout(id)` transition is handled per SPEC-20 §3.3 R24.'"
2. Similar amendment for SPEC-13 R21 (new transition rows listed with authoritative status).
3. Coordinate with ESPECIALISTA EM SPECS to also issue a patch note in SPEC-06 and SPEC-13 pointing forward to SPEC-20's amendment.

**Invariant affected:** no invariant, but spec ownership coherence is broken.

---

### SC-012 (HIGH) — Missing transition: `WaitingForResults × WorkerJoined(id)` handler

**Severity:** HIGH
**Axis:** C (FSM correctness)
**Location:** §4.1.3 Extended Transition Table.
**Problem:** The table shows `WaitingForResults × PhaseTimeout`, `WaitingForResults × WorkerConnectionLost`, `WaitingForResults × WorkerLeft`, but no handler for `WaitingForResults × WorkerJoined(id)`. This is because §3.2 R16 says joins during active rounds are queued. But the FSM event `WorkerJoined(id)` (§4.1.2) is defined without qualification — it fires "whenever a worker connects during an active session, not during initial WaitingForWorkers" (§4.1.2). So if the event fires in `WaitingForResults`, the FSM has no transition defined. Rust's `match` exhaustiveness will force the implementer to either (a) error, (b) silently no-op, (c) invent a default handler — none of which are in the spec.

**Suggested fix:**
1. Add explicit transition: `WaitingForResults × WorkerJoined(id) → WaitingForResults` with actions `QueueWorkerForNextWindow(id)`. Define `QueueWorkerForNextWindow` in §4.1.2 actions.
2. Similar handlers needed for every non-`AcceptingMembershipChanges` state that could plausibly receive `WorkerJoined(id)`: `Partitioning`, `Dispatching`, `Merging`.

**Invariant affected:** D6 AT_RISK if no-op is chosen.

---

### SC-013 (HIGH) — R31 "SHOULD release retained partition when PartitionResult received" is under-constrained

**Severity:** HIGH
**Axis:** F (testability), A (invariant preservation)
**Location:** §3.3.5 R31.
**Problem:** R31 says the coordinator "SHOULD release retained partitions as soon as the corresponding `PartitionResult` is received." SHOULD, not MUST. This leaves open:

1. If an implementation holds retained partitions for the full grid run ("to be safe against a future departure"), §3.3.5's memory cost blow-up is realized. The bound becomes O(sum_rounds(sum(|P_i|))), not the O(sum(|P_i|)) claimed in R31.
2. If released, there is no fallback for a departure that happens *after* all PartitionResults were received for the current round. SPEC-20 §3.3.2 R22 explicitly allows LeaveRequest *after* PartitionResult — and the retained partition has already been freed.
3. If a worker graceful-leaves after PartitionResult is received, then fails mid-next-round *before* the new retained-copy is snapshotted, there is a narrow window where no retained copy exists. Spec is silent.

**Impact if unresolved:** Either implementation is memory-wasteful (holding all retained partitions forever) or correctness-unsafe (releasing too eagerly). EG-U13 ("retained partition released on ack") tests one policy; the other is untested.

**Suggested fix:**
1. Upgrade SHOULD to MUST, with an exact semantic: "R31 (revised): The coordinator MUST release a retained partition as soon as both (a) the corresponding `PartitionResult` is received AND (b) the next round's dispatch (including re-snapshotting of retained partitions) has completed. Between these two events, the retained partition is 'in-flight' and holds memory for at most the duration of one round."
2. Add the narrow-window failure case to the test matrix.

**Invariant affected:** I5-elastic (memory bound) UNDEFINED.

---

### SC-014 (HIGH) — §3.1 R8 id-range computation uses "K_eff" when `num_workers` is overloaded

**Severity:** HIGH
**Axis:** A (consistency with SPEC-04)
**Location:** §3.1 R8, SPEC-04 R16-R19.
**Problem:** R8 says "self-partition MUST receive an ID range (SPEC-04, R16-R19) computed by `compute_id_ranges(K_eff, net.next_id)` for `worker_id = 0`, ensuring no ID collisions with remote workers."

SPEC-04 R18's formula is `chunk_size = u32::MAX / num_workers`. In R8, `num_workers = K_eff = K + 1` where K is *remote* workers. This is consistent.

But in §3.2 R13, "ID ranges MUST be recomputed for all K_eff_new nodes using `compute_id_ranges(K_eff_new, merged_net.next_id)`." And in §3.3 R30, same for K_eff-D. `compute_id_ranges` takes two parameters. But SPEC-04 §4.7 says `chunk_size` uses `u32::MAX / n` — `n` is the count. The function does not take `net.next_id` as an argument. It computes ranges based solely on count. The sub-net's `next_id` is initialized as `max(range_i.start, max_agent_id_in_partition + 1)` (SPEC-04 R18).

So SPEC-20 R8/R13/R30's signature `compute_id_ranges(K, next_id)` does not match SPEC-04 R18's actual signature `compute_id_ranges(K)`. This is either (a) SPEC-20 inventing a new signature, or (b) SPEC-20 informally naming the full "compute + apply" pipeline. Either way, it is ambiguous.

**Impact if unresolved:** Stage 1 will implement an API different from SPEC-04; stage 4 review will catch this; re-work.

**Suggested fix:**
1. Align SPEC-20 R8/R13/R30 with SPEC-04's actual function: `compute_id_ranges(K_eff)` returns `Vec<IdRange>`; the coordinator then applies `range.start` and `max_agent_id_in_partition + 1` to set each sub-net's `next_id` per SPEC-04.
2. If SPEC-20 wants a unified call, introduce a new function `recompute_ranges_and_repartition(merged_net, K_eff) -> PartitionPlan`; this would be a SPEC-04 extension and must land as a SPEC-04 amendment, not a SPEC-20 requirement.

**Invariant affected:** D4 (ID uniqueness) consistent if SPEC-04's semantics are used, but the spec ambiguity will cause bugs.

---

### SC-015 (HIGH) — Test plan has no property tests for mixed `hybrid × join × leave × strict_bsp` matrix

**Severity:** HIGH
**Axis:** F (testability)
**Location:** §7.3 Property-Based Tests (EG-P1, EG-P2, EG-P3).
**Problem:** The three proptests cover:
- EG-P1: hybrid + K random, no membership changes.
- EG-P2: random departure schedules (no joins, no hybrid-vs-nonhybrid variance).
- EG-P3: ID range disjointness.

Missing proptests:
1. Joint hybrid × join × leave + strict_bsp. This is the scenario flagged by the master plan's Gate 1 integration test ("3-worker hybrid run with mid-round join + mid-round leave produces canonically-equivalent result"). Acceptance test is not a proptest; no randomized coverage.
2. CON-DUP-heavy workloads specifically. The CON-DUP commutation rule creates 4 new agents and is the main source of emergent border redexes (SPEC-05 R32). Any correctness claim for departure + re-dispatch is empty if CON-DUP cascades are not exercised.
3. Delta mode × elastic grid. Per SC-001, the delta-mode path is currently absent from the spec; its proptest coverage must also be specified.

**Impact if unresolved:** Acceptance Gate 1 depends on one integration test that is insufficient. Without proptests, rare races (boundary join + CON-DUP cascade + departure-with-retained) will escape detection.

**Suggested fix:**
1. Add EG-P4: random (hybrid ∈ {on, off}) × random (strict_bsp ∈ {on, off}) × random (join schedule) × random (leave schedule) × random K × random terminating net — verify canonical equivalence to `reduce_all`.
2. Add EG-P5: CON-DUP-heavy generator (use `ep_annihilation_con` from SPEC-09) × random membership changes × verify canonical equivalence.
3. Add EG-P6 (delta mode): once SC-001 is resolved, add a proptest that runs under delta mode.

**Invariant affected:** G1 under-tested.

---

### SC-016 (MEDIUM) — §3.1 R7 "worker_id = 0 for coordinator self-partition" double-books WorkerId 0 with SPEC-04 implicit usage

**Severity:** MEDIUM
**Axis:** A (consistency)
**Location:** §3.1 R2, R7; SPEC-04 R16-R19 ("Worker 0: [0, chunk_size)").
**Problem:** In SPEC-04, Worker 0 is an arbitrary index into a non-hybrid K-worker grid. In SPEC-20, `worker_id = 0` is specifically the hybrid coordinator's self-partition. If an existing SPEC-04 test (or a v1 benchmark) assumes `worker_id = 0` is a remote worker, that test breaks under hybrid=true. Tests across SPEC-04 and SPEC-20 must be audited.

**Suggested fix:**
1. Add §3.1 R2a: "In non-hybrid mode (`hybrid_coordinator = false`), `worker_id = 0` refers to the first remote worker, consistent with SPEC-04 R16-R19. In hybrid mode, `worker_id = 0` is reserved for the coordinator self-partition, and the first remote worker receives `worker_id = 1`."
2. Audit existing tests for worker_id = 0 usage; add cross-mode test `EG-U1b: worker_id=0 semantics differ across hybrid/non-hybrid modes`.

**Invariant affected:** none formal, but test portability is reduced.

---

### SC-017 (MEDIUM) — `Shutdown` is overloaded for graceful-leave flow in §4.4.4

**Severity:** MEDIUM
**Axis:** D (wire protocol), C (FSM correctness)
**Location:** §4.4.4 diagram last line "`Shutdown(B) →`", §3.7 R37.
**Problem:** R37 says "existing `Shutdown` message is sufficient: joining workers receive `AssignPartition`; the coordinator sends `Shutdown` when it no longer needs a worker." Under normal end-of-session `Shutdown`, the semantic is "session over, stop." Under graceful-leave `Shutdown`, the semantic is "your LeaveRequest was acked, stop." The worker cannot distinguish these two; if the worker's FSM (SPEC-13 R25 `Idle × Shutdown → Done`) doesn't care, this is fine — but that is not guaranteed.

**Suggested fix:**
1. Either introduce a distinct `LeaveAck` (see SC-004) and reserve `Shutdown` strictly for session-end, OR
2. Explicitly state R37a: "When `Shutdown` is sent in response to `LeaveRequest`, it carries the identical semantics as end-of-session `Shutdown`. The worker does not need to distinguish them. Log-level differentiation is logged at the coordinator only."

**Invariant affected:** none.

---

### SC-018 (MEDIUM) — §3.2 R15 "switch from solo mode to grid mode" is missing an FSM transition

**Severity:** MEDIUM
**Axis:** C (FSM correctness)
**Location:** §3.2 R15, §4.1.3 transition table.
**Problem:** R15 says "a worker joining mid-execution MUST cause the coordinator to switch from solo mode to grid mode at the next round boundary." The FSM transition table has no "solo mode" state. If solo mode is represented as `Partitioning` with `K=1` (per R5, `num_workers = 1`), the transition must be specified: `Partitioning × WorkerJoined(id)` is not in the table. If it is represented as a separate `SoloReducing` state (which makes sense given R5's "no split/merge overhead"), that state is not defined.

**Suggested fix:**
1. Add a new state `SoloReducing` to §4.1.1.
2. Add transitions: `WaitingForWorkers × InitialWaitTimeout [K=0, hybrid=true] → SoloReducing`, `SoloReducing × SoloReductionComplete → Done`, `SoloReducing × WorkerJoined(id) → CheckTermination` (with action `QueueWorkerForNextWindow(id)`, reduction re-started as grid mode next round).

**Invariant affected:** D6 preserved (reduction completes either way), but FSM is not total.

---

### SC-019 (MEDIUM) — §3.6 GridMetrics extensions are additive but missing interaction with SPEC-19's delta metrics

**Severity:** MEDIUM
**Axis:** E (cross-spec interaction), F (testability)
**Location:** §3.6 R38, SPEC-19 §3.6 (delta-specific metrics).
**Problem:** SPEC-19 introduced `border_graph_apply_deltas_time_per_round`, `bytes_per_round_delta`, etc. SPEC-20 R38 adds elastic-specific metrics (workers_joined, workers_departed, etc.). These must be composable — a test measuring delta-mode + elastic grid needs both sets. The spec does not say how the two sets compose (are they merged into one GridMetrics struct? separate structs? a compose trait?).

**Suggested fix:**
1. State explicitly: "R38a: When both SPEC-19 delta protocol and SPEC-20 elastic grid are active, all metrics from both specs coexist in a single `GridMetrics` struct. Delta-specific metrics fields and elastic-specific metrics fields are additive and do not conflict."
2. Audit field-name collisions.

**Invariant affected:** none, but observability is incomplete.

---

### SC-020 (MEDIUM) — `initial_wait_timeout` default of 30s conflicts with SPEC-06 R24 `worker_connect_timeout` default of 120s

**Severity:** MEDIUM
**Axis:** A (consistency)
**Location:** §3.1 R6, §3.4 R33 (`initial_wait_timeout: Duration, Default: 30 seconds`), SPEC-06 R24 (`worker_connect_timeout, default 120 seconds`).
**Problem:** `initial_wait_timeout` (30s) is the timer after which the hybrid coordinator begins solo reduction if no workers have connected. `worker_connect_timeout` (120s) is the timer after which the non-hybrid coordinator aborts. If `hybrid_coordinator = false`, which timer applies? R6 says "If `hybrid_coordinator = false`, the coordinator MUST continue waiting until `worker_connect_timeout` ... expires." Good, non-hybrid uses 120s. If `hybrid_coordinator = true`, R6 says "After this timeout [initial_wait_timeout = 30s], if ... no workers have connected, the coordinator SHOULD begin reducing alone." SHOULD, not MUST. If SHOULD is ignored and the coordinator waits for `worker_connect_timeout = 120s`, is it an SPEC-20 violation? The spec does not say.

**Suggested fix:**
1. Make R6 a MUST: "If `hybrid_coordinator = true` AND `initial_wait_timeout` elapses without any worker connections, the coordinator MUST begin solo reduction, canceling `worker_connect_timeout`."
2. State that `initial_wait_timeout` supersedes `worker_connect_timeout` when hybrid=true.

**Invariant affected:** D6 (protocol termination) AT_RISK — a coordinator may be stuck for 120s before solo reduction starts if SHOULD is ignored.

---

### SC-021 (MEDIUM) — §4.2.2 step 4 merge signature is not defined in SPEC-05

**Severity:** MEDIUM
**Axis:** A (consistency with SPEC-05)
**Location:** §4.2.2 step 4 "The merge includes: K_eff - D successfully reduced partitions ... + D retained (unreduced) partitions."
**Problem:** SPEC-05 R1 defines `merge(PartitionPlan) -> (Net, border_redex_count)`. SPEC-05 R3-R7 defines the merge processing. SPEC-20 §4.2.2 implies merging "mixed reduced + unreduced partitions" is the same operation, citing §6.1 Step 3 ("no code change to `merge()` itself; it operates on `Vec<Partition>` regardless of reduction state"). Two problems:

1. The **free_port_index** of an unreduced partition is the original `split()`-time index. The free_port_index of a reduced partition has been updated per SPEC-05 §4.3. The merge operation (SPEC-05 R4) consults the free_port_index; if the unreduced partition's index points to agents that were retained but whose border ID was *also* reassigned by the rejoin step, the index is inconsistent.
2. The **border_id_start / border_id_end** fields of an unreduced partition are from the initial split. Across multiple rounds of re-split (per R12, R25), border IDs are re-allocated. The unreduced partition's border IDs may overlap with a new round's freshly-allocated border IDs. SPEC-04 R15a's range check (`border_id_start <= id < border_id_end`) will misclassify IDs if the ranges overlap.

**Impact if unresolved:** Merge produces incorrect topology; G1 silently fails.

**Suggested fix:**
1. Add R25a: "When a retained (unreduced) partition is merged with reduced partitions, the coordinator MUST first rebase the retained partition's border IDs into the new round's border ID range by calling a new `rebase_border_ids(partition, old_range, new_range) -> Partition` function. SPEC-04 must be amended to provide this function (new R15b)."
2. Add a test case: EG-U7a — retained partition with border IDs 100-105 merged with reduced partitions having border IDs 200-210; verify no range overlap and correct reconnection.

**Invariant affected:** D4 (ID uniqueness via border_id uniqueness) VIOLATED unless rebased.

---

### SC-022 (MEDIUM) — §3.1 R6 uses `initial_wait_timer` as a TimerId name not present in SPEC-13

**Severity:** MEDIUM
**Axis:** A (consistency)
**Location:** §4.1.3 transition table "BindListener, StartTimer(initial_wait_timer), LogTransition".
**Problem:** SPEC-13 R19's actions include `StartTimer(TimerId, Duration)` and `CancelTimer(TimerId)`, where `TimerId = u32`. SPEC-20 uses symbolic names `initial_wait_timer`, `join_window_timer`, `collect_timer`. These must be enumerated either as numeric constants or as a named enum.

**Suggested fix:**
1. Add to §4.1.1: `pub enum TimerKind { InitialWait, JoinWindow, Collect, }` and derive `TimerId` from it deterministically.
2. Update all §4.1.3 rows accordingly.

**Invariant affected:** none, cosmetic/consistency.

---

### SC-023 (MEDIUM) — No specification of coordinator's memory behavior under rapid join/leave churn

**Severity:** MEDIUM
**Axis:** F (testability), A (invariant preservation)
**Location:** §3.3.5 R31, §7 tests.
**Problem:** No requirement bounds the coordinator's memory under churn. An adversarial workload with 100 joins + 100 departures per round (worst case) could cause unbounded `WorkerId` allocation (§3.2 R11 says never reuse) and unbounded retained-partition snapshots. The spec does not say whether the coordinator has a bounded `max_workers_history` or an unbounded one.

**Suggested fix:**
1. Add R32a: "The coordinator MUST maintain a WorkerId counter bounded by `u32::MAX`. If the counter reaches u32::MAX, subsequent joins MUST be rejected with a new variant `JoinNack { reason: 'worker_id_space_exhausted' }`. In practice, this is not expected for TCC-scope workloads."
2. Add test EG-U14: churn test with 10000 joins/leaves verifies bounded memory.

**Invariant affected:** I5-elastic (memory bound) UNDEFINED.

---

### SC-024 (LOW) — §3.1 R2 binds "worker_id = 0" at partition time but coordinator may not always be partition 0

**Severity:** LOW
**Axis:** A (consistency), G (scope)
**Location:** §3.1 R2, OQ-2.
**Problem:** OQ-2 raises whether the coordinator should take the smallest partition instead of always partition 0. If the coordinator takes the smallest partition, its WorkerId is still 0, but its `partition_index` in the sorted list of partitions is not 0. This breaks the assumption that `partitions[0]` is the coordinator's partition in §4.1.4's "Partitioning" pseudocode.

**Suggested fix:**
1. Either resolve OQ-2 to "always partition 0" (preserving the pseudocode), or redefine `partitions[0]` in §4.1.4 as a role-based label, not an index.
2. Track OQ-2 explicitly for v3 scope; SPEC-20 freezes on "partition 0 == self-partition."

---

### SC-025 (LOW) — §5.3 comparison table rows "v1 (SPEC-06/SPEC-13)" and "v2 (SPEC-20)" miss SPEC-19 entirely

**Severity:** LOW
**Axis:** E (cross-spec interaction)
**Location:** §5.3 Comparison table.
**Problem:** The table compares v1 and v2-elastic but does not mention v2-delta (SPEC-19) or v2-delta+elastic (SC-001). A reader of SPEC-20 alone would conclude v2 is elastic-only.

**Suggested fix:**
1. Add a column for "v2 (SPEC-19 Delta)" and "v2 (SPEC-19 + SPEC-20)" to the table.
2. At minimum, add a footnote: "This table assumes v1 full-merge mode. For v2 delta mode interactions, see §3.0 Execution Mode Matrix (new, per SC-001)."

---

### SC-026 (LOW) — §8 OQ-1 through OQ-5 should be resolved before Stage 1, not deferred

**Severity:** LOW
**Axis:** F (testability), G (scope)
**Location:** §8 Open Questions.
**Problem:** OQ-1 (join window duration), OQ-2 (self-partition assignment strategy), OQ-3 (strict BSP interaction), OQ-4 (partition retention vs checkpoint), OQ-5 (worker ID reuse) are all ambiguities. Stage 1 (task-splitter) cannot produce testable tasks from "here are 5 open questions." Every OQ must be resolved (either "decision made" or "deferred to vN+1 with explicit consequences") before Stage 1 can begin.

**Suggested fix:**
1. Resolve OQ-1 (see SC-007), OQ-2 (see SC-024), OQ-3 (see SC-002), OQ-4 ("deferred to SPEC-23 fault tolerance"), OQ-5 ("decided: new WorkerId, no reuse — but see SC-006 on WorkerId vs partition_index").
2. Move resolved content from §8 into appropriate §3 requirements.

---

### SC-027 (LOW) — §3.1 R7 metrics: "worker_id = 0" for coordinator-local is recoverable but not uniquely identifiable in multi-run logs

**Severity:** LOW
**Axis:** F (testability)
**Location:** §3.1 R7.
**Problem:** R7 records coordinator local reduction with `worker_id = 0`. In a benchmark run matrix, logs from hybrid-mode runs will have `worker_id = 0` entries; logs from non-hybrid-mode runs will also have `worker_id = 0` entries (as the first remote worker). Log analysis conflates the two.

**Suggested fix:**
1. Add a boolean `is_coordinator_self: bool` field to `WorkerRoundStats` (or encode self-partition as `worker_id = -1` / u32::MAX).
2. Alternative: log `worker_id = 0` with a metadata tag `role = 'coordinator_self' | 'remote'`.

---

## Invariant-by-invariant audit

| Invariant | Status under SPEC-20 | Finding |
|-----------|----------------------|---------|
| **T1 (port linearity)** | PRESERVED | Reduction is via `reduce_all` which preserves T1 by construction. Unaffected by membership changes. |
| **T2 (principal-port interaction)** | PRESERVED | Same. |
| **T3 (active-pair disjointness)** | PRESERVED | Same. |
| **T4 (strong confluence = P1)** | PRESERVED (by construction) | SPEC-20 does not alter reduction rules; T4 holds. |
| **T5 (6 rules)** | PRESERVED | Rules are untouched. |
| **T6 (unique normal form)** | PRESERVED | Under assumption that G1 holds; see G1 row. |
| **T7 (invariant step count)** | PRESERVED | At-least-once re-execution (R29) makes the *observed* system-wide step count exceed T7's invariant count, but the *net's reduction* step count to NF is unchanged. OK, but SPEC-20 does not distinguish — minor readability issue. |
| **D1 (split/merge identity)** | AT_RISK | §4.2.2 step 4 "merge mixed reduced+unreduced partitions" — the identity `merge(split(net)) ~ net` applies only if `split()` is followed by `merge()` with no reduction between. SPEC-20's mixed-partition merge is novel and not trivially covered by D1's derivation. SC-003, SC-021. |
| **D2 (local reduction equivalence)** | PRESERVED | Each worker still reduces locally via `reduce_all`. Unaffected. |
| **D3 (border redex completeness)** | AT_RISK | SC-003, SC-008. Retained-partition re-merge with surviving evolved partitions may leave border redexes unresolved or create spurious ones via border_id range overlap. |
| **D4 (ID uniqueness)** | AT_RISK | SC-006 (WorkerId vs. partition_index ambiguity), SC-014 (`compute_id_ranges` signature drift), SC-021 (border_id range overlap). |
| **D5 (exclusive ownership)** | AT_RISK | SC-003. "Retained partition as snapshot, not live" claim in §3.7 R39 is valid *only* if re-dispatch is atomic with respect to surviving partitions' state. The spec does not guarantee atomicity. |
| **D6 (protocol termination)** | AT_RISK | SC-002 (mid-reduce event buffering), SC-007 (join window boundary race), SC-009 (solo-mode preemption), SC-012 (missing `WaitingForResults × WorkerJoined` transition), SC-020 (timer conflict). |
| **I1 (bidirectional port array)** | PRESERVED | Per-worker invariant, unaffected. |
| **I2 (reference validity)** | PRESERVED | Same. |
| **I3 (monotonic IDs)** | PRESERVED per partition | But see SC-006 on range ownership. |
| **I4 (redex queue validity)** | PRESERVED | Stale entries are tolerated; unchanged. |
| **I5 (local termination)** | PRESERVED | `reduce_all` terminates for terminating nets. |
| **I6 (ERA aux slot cleanliness)** | PRESERVED | Per-agent invariant. |
| **I7 (root port consistency)** | PRESERVED | Per-net invariant. |
| **G1 (fundamental property)** | **AT_RISK / UNDEFINED in delta mode** | SC-001 (delta-mode incompatibility), SC-003 (mixed-trace merge), SC-005 (ARG-001 Passo 4 cited outside scope). Tests EG-I1/I2/I3 cover v1-mode-only. |
| **P1 (from ARG-001)** | PRESERVED | Same as T4. |
| **P5 (protocol termination from ARG-001)** | AT_RISK | Propagated from D6. |

Legend: PRESERVED | AT_RISK | VIOLATED | UNDEFINED

---

## Cross-spec consistency audit

| Predecessor | Compatibility | Key issues |
|-------------|---------------|------------|
| **SPEC-01** | AT_RISK | D3, D4, D5, D6, G1 flagged above. SPEC-20 §3.7 R39 makes optimistic claims that this review contests. |
| **SPEC-04** | INCONSISTENT | `compute_id_ranges` signature drift (SC-014); border_id range rebase missing (SC-021); §3.1 R2 self-partition semantics overload SPEC-04 R16-R19 (SC-016). |
| **SPEC-05** | INCONSISTENT | `merge()` mixed-partition call is new territory (SC-003, SC-021); `GridConfig` extension path (SC-019, SC-020, SC-022). |
| **SPEC-06** | INCONSISTENT | R25 fatal-error behavior silently overridden (SC-011); new variants missing ack pairs and discriminants (SC-004); `PROTOCOL_VERSION` bump missing (SC-004). |
| **SPEC-10** | UNKNOWN | Authentication reference in §3.2 R11 is handwavy — does `JoinRequest` re-auth via SPEC-10? Spec is silent. |
| **SPEC-13** | INCONSISTENT | New FSM states/events inserted without formal amendment (SC-011); missing transitions (SC-012, SC-018); concurrency model underspecified (SC-002). |
| **SPEC-17** | COMPATIBLE WITH UNSTATED ASSUMPTION | New messages must flow through Transport trait — SPEC-20 does not say so but also does not break it. Safe to assume Transport. |
| **SPEC-18** | COMPATIBLE | Discriminant append stability preserved if SPEC-20 starts from 12. |
| **SPEC-19 (Delta Protocol)** | **INCOMPATIBLE** | SC-001. This is the dominant finding. |

---

## Untestability catalog (requirements that cannot be tested as written)

1. **R3** ("coordinator MUST NOT block ... self-reduction MUST be concurrent"). No concrete mechanism for "concurrent" is specified; no test can determine whether the implementation is truly concurrent vs. serialized-with-async-overhead. See SC-002.
2. **R6 SHOULD clause** ("coordinator SHOULD begin reducing alone"). SHOULD is not testable as a hard requirement.
3. **R10 (join window definition)** depends on OQ-1 resolution. See SC-007.
4. **R15** ("coordinator MUST switch from solo mode to grid mode at the next round boundary"). Solo mode has no round boundaries. See SC-009, SC-018.
5. **R22 split-mode** ("LeaveRequest without PartitionResult treated as timeout + graceful"). The composition of the two treatments is not specified. See SC-008.
6. **R24** ("re-dispatch from original state is safe under strong confluence") — relies on an extension of ARG-001 not yet proved. See SC-005.
7. **R29 at-least-once** — informative, not testable. OK.
8. **R31 SHOULD** release of retained partitions. Not testable. See SC-013.
9. **R37 informative** — OK.
10. **OQ-1, OQ-2, OQ-3, OQ-5** — unresolved open questions. See SC-026.

---

## Gate decision

**BLOCK.**

**Rationale:**

SPEC-20 cannot proceed to Stage 1 (task-splitter) without substantial revision because:

1. **SC-001 (delta-protocol incompatibility) is a full-stack gap.** D-005 landed on 2026-04-24, one day before this review. SPEC-20 drafted 2026-04-15 treats the pre-delta execution model as the only one. Without resolution, implementing SPEC-20 as written would produce a binary incompatible with the delta protocol default — silently regressing the break-even story.
2. **SC-002, SC-003, SC-004, SC-005, SC-006 (the other 5 CRITICALs)** each independently meet the SPEC-REVIEW-19 §3.4 bar ("architecturally wrong wire shape, not a typo"): underspecified concurrency, invariant-at-risk retention semantics, missing wire variants, unproven confluence extension, ambiguous WorkerId vs. partition_index.
3. The **11 HIGHs** are largely cross-cutting: FSM totality (SC-012, SC-018), config inconsistency (SC-010, SC-020), test coverage (SC-015), amendment-to-predecessors missing (SC-011).
4. The invariant audit flags **D1, D3, D4, D5, D6, G1 as AT_RISK/UNDEFINED** — the majority of SPEC-01. A spec that risks most of SPEC-01 without explicit amendment requires a full re-scoping pass.

Round 2 cannot simply clean up findings; it must also re-scope SPEC-20 against SPEC-19. Realistic estimate: Round 2 produces a revised draft; Round 3 closes residual testability gaps; Round 4 is the final sign-off. This is the same 3-round cycle the context message hoped to avoid but is unavoidable given the pre-delta drafting context.

---

## Specialist-em-specs TODO list (concrete spec edits before Round 2 can start)

1. **§3.0 (new) Execution Mode Matrix.** Enumerate all four mode combinations `{v1-full-merge, v2-delta} × {strict_bsp-true, false}` and state SPEC-20 semantics for each. Delta-mode requirements must rewrite §3.1 R4, §3.2 R12, §3.3 R23-R26 to target `BorderGraph` + stateful-worker semantics. (SC-001)
2. **§3.1 R3 + §4.1 concurrency pattern.** Specify the exact `tokio::select!` pattern, buffering rules for `LeaveRequest`/`WorkerJoined`/`PhaseTimeout` events during self-reduce, and FSM event ordering. Add `SelfPartitionPanic` event and transition. (SC-002)
3. **§3.3 retained-partition semantics.** Split into `retained_initial` (round-0) vs `retained_last_acked`. Specify which is used in each departure scenario. Add formal sub-invariant "D3-elastic" on the rebased merge behavior. Amend ARG-001 (or open ARG-005) with the mixed-trace confluence proof. (SC-003, SC-005, SC-021)
4. **§3.5 wire protocol.** Define `JoinRequest`, `JoinAck`, `LeaveAck` with exact schemas and discriminants 12-15. Bump `PROTOCOL_VERSION` 3→4. Add an explicit ack semantics requirement. (SC-004)
5. **§3.2 R11 WorkerId vs. partition_index.** Decouple them formally. Define new D4-elastic sub-invariant. Update R8/R13/R30 `compute_id_ranges` call signatures to match SPEC-04. (SC-006, SC-014)
6. **§3.2 R10/R16 join window.** Resolve OQ-1. Define min/max timers. Add R10a/R10b. Add boundary-race test (EG-U6a). (SC-007)
7. **§3.3 R22 split-mode departure.** Add `LeaveKind` enum. Split R22 into R22a (clean) / R22b (urgent). Add FSM transitions. (SC-008)
8. **§3.1 R5/R6 solo-mode preemption.** Resolve the `reduce_all` vs. `reduce_n(budget)` question. Update R5 to reflect preemption semantics. (SC-009)
9. **§3.4 default for `hybrid_coordinator`.** Change default to `false` to preserve v1 benchmark baseline. (SC-010)
10. **§3.8 (new) Amendment to Predecessor Specs.** List each requirement SPEC-20 supersedes from SPEC-06, SPEC-13. Coordinate with ESPECIALISTA EM SPECS to add forward references. (SC-011)
11. **§4.1.3 FSM table totality.** Add `WaitingForResults × WorkerJoined(id)`, `Partitioning × WorkerJoined(id)`, `Dispatching × WorkerJoined(id)`, `Merging × WorkerJoined(id)` transitions with `QueueWorkerForNextWindow` action. Add `SoloReducing` state. (SC-012, SC-018)
12. **§3.3 R31 retained partition release.** Upgrade SHOULD → MUST with precise release semantics. (SC-013)
13. **§7.3 proptests.** Add EG-P4 (joint hybrid × join × leave × strict), EG-P5 (CON-DUP-heavy × churn), EG-P6 (delta-mode elastic). (SC-015)
14. **§3.6 R38a.** State additivity with SPEC-19 metrics. Audit for collisions. (SC-019)
15. **§3.1 R6 timer conflict.** Make R6 a MUST. State that `initial_wait_timeout` supersedes `worker_connect_timeout` in hybrid mode. (SC-020)
16. **§4.1 TimerKind enum.** Replace symbolic timer names with an enum. (SC-022)
17. **§3.3 R32a.** Add WorkerId exhaustion handling. (SC-023)
18. **§5.3 comparison table.** Add SPEC-19 columns. (SC-025)
19. **§8 Open Questions.** Resolve all OQs (SC-026) and move resolutions into §3.
20. **§7.1 test plan.** Add EG-U1a, EG-U1b, EG-U6a, EG-U7a, EG-U10a/b, EG-U12a, EG-U14, EG-I5a/b. (various SC-items)

---

**End of Round 1 review.** Round 2 target: all CRITICAL and HIGH findings resolved; Round 3 target: MEDIUM/LOW findings and testability gaps closed. Round 4 target: sign-off.
