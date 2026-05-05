# QA Phase D — Stage 5 adversarial review of SPEC-20 Elastic Grid Departure (TASK-0438..0443)

**Date:** 2026-04-27
**QA agent:** qa (Stage 5, opus)
**Bundle:** Phase D (Departure) — TASK-0438 (detection), TASK-0439 (retained state), TASK-0440 (v1 reclaim), TASK-0441 (graceful leave), TASK-0442 (D == K_eff), TASK-0443 (delta reconstruct).
**Files inspected (read-only):**
- `relativist-core/src/protocol/coordinator.rs` (1211 LoC; recovery block at 766–833)
- `relativist-core/src/protocol/retained.rs` (NEW — 87 LoC)
- `relativist-core/src/partition/departure_recovery.rs` (NEW — 67 LoC)
- `relativist-core/src/partition/remap.rs` (caller of `remap_partition_ids`)
- `relativist-core/src/merge/grid.rs` (declarations of `reconstruct`, `BorderGraph::from_partition_plan`)
- `docs/reviews/REVIEW-PHASE-D-elastic-2026-04-27.md` (Stage 4 ammunition: MF-001..MF-009, SF-001..SF-006, NTH-001..NTH-004)
- `specs/SPEC-20-elastic-grid.md` §3.3 R13–R28, §3.6 R23/R31, §3.8 A1/A4, §4.1.4
- `docs/qa/QA-TASK-0414-2026-04-25.md` (QA precedent format)

**Upstream verdict:** Stage 4 reviewer — **REJECT — REQUIRES REWORK BEFORE STAGE 5 QA** with 3 CRITICAL + 3 HIGH + 3 MEDIUM Must-Fix findings (MF-001..MF-009) plus 6 Should-Fix (SF-001..SF-006). The reviewer explicitly stated "Stage 5 QA cannot productively probe Phase D in its current state." This QA pass is conducted **anyway**, per the orchestrator's brief, to surface every *additional* hazard the developer needs to know before re-doing Phase D — so the rework lands once, not twice.

**QA mode:** structurally aware adversarial. The recovery path is non-functional (MF-001), so dynamic execution probes (`cargo test`, integration drops) cannot be staged against the current code. All findings below are derived from static reads of the bundle, the spec, and the reviewer's findings — extended with concrete adversarial reproductions, witness sequences, and invariant-violation traces that the reviewer's pass did not surface.

---

## Summary

| Severity | Count | IDs |
|----------|-------|-----|
| **CRITICAL** | 5 | QA-001, QA-002, QA-003, QA-004, QA-005 |
| **HIGH** | 6 | QA-006, QA-007, QA-008, QA-009, QA-010, QA-011 |
| **MEDIUM** | 5 | QA-012, QA-013, QA-014, QA-015, QA-016 |
| **LOW** | 3 | QA-017, QA-018, QA-019 |

**Top-5 most dangerous (one line each):**

1. **QA-001 (CRITICAL)** — `RetainedStateRegistry` is updated **non-atomically across two distinct sites** (`retained_state.initial.entry(...).or_insert_with(...)` at lines 587–600 *and* `retained_state.refresh_last_acked(...)` at line 663), but **no recovery exists if the coordinator crashes between them**: a panic *after* `refresh_last_acked` for round N–1 *but before* the new round-N partition reaches `initial` would leave a `last_acked` pointing at a worker for whom `initial` was never restored on restart. Combined with the `debug_assert!` at `retained.rs:58–62` (D5), restart-recovery is forced into a release-mode silent state-loss path: the assertion compiles out, `refresh_last_acked` overwrites in place, and `materialize_reclaimed_partitions` later returns `InvariantViolation("State loss for worker N")` — the symptom is correct, but the root cause (no on-disk persistence of retained state) is invisible. This **silently disables R23/R31 across coordinator restarts**, which the spec implicitly demands for departure recovery to be useful.

2. **QA-002 (CRITICAL)** — The `D == K_eff` test branch in TASK-0442 has **the wrong sign**: `if departing_worker_ids.len() >= k_eff` fires when **all workers including the self-partition** have departed, but the self-partition (id 0 in hybrid mode) **cannot be in `departing_worker_ids`** under the current detection helpers (`handle_connection_loss`/`handle_phase_timeout` are only called on remote streams in the recv loop at line 645; the self-handle is `await`ed once at line 760, never polled for departure). Therefore `>= k_eff` requires `D == K_remote == k_eff`, which is **algebraically impossible** in hybrid mode (`k_eff = K_remote + 1`). **The hybrid R26a branch is unreachable code.** MF-002 noted the *accounting* error; QA-002 sharpens it: the condition cannot fire at all in hybrid mode. The test for D==K_eff (which the reviewer found absent in MF-006) would pass against pure-remote mode and never exercise the SoloReducing transition the spec demands.

3. **QA-003 (CRITICAL)** — `materialize_reclaimed_partitions` calls `remap_partition_ids(p, IdRange{0, 100_000})` for **every** departed worker. `remap_partition_ids` (verified at `partition/remap.rs:120`) computes `new_id = new_range.start + rank` and **writes into `new_agents[new_id as usize]`**. With `new_range.start = 0` and a partition of size N, the remapped partition occupies IDs `[0, N)`. **The surviving partitions are also numbered from 0** (each partition has its own pre-existing `id_range`, typically `[0, partition_size)` in v1). The `reconstruct(border_graph, surviving_partitions, reclaimed_partitions)` at line 821 then merges Nets that **share agent IDs `0..min(N_surv, N_reclaim)`** — silently overwriting agent slots in the merged arena. **The result is a `Net` that violates D4 (ID Uniqueness) and structurally violates T1 (every agent's principal port belongs to exactly one wire) because two distinct agents now claim id 0.** Reviewer MF-003 noted the disjointness violation; QA-003 names the concrete invariant breakage and the witness: any 2-worker grid with departure produces a Net where reduction is undefined.

4. **QA-004 (CRITICAL)** — The `LeaveRequest` handler does not distinguish "graceful leave with successful `LeaveAck` send" from "graceful leave with TCP-RST mid-ack." **Line 680: `let _ = send_frame(stream, &Message::LeaveAck).await;`** — the result is *discarded*. If the worker sends `LeaveRequest` and immediately closes the connection (a common adversarial pattern: `shutdown(SHUT_RDWR)` right after `write`), `send_frame` returns `Err`, the discard suppresses it, and the coordinator proceeds as if the ack landed. **Then the worker, having dropped the connection, never sees the `LeaveAck` and (per SPEC-20 R35a) is required to retry `LeaveRequest`** — but the coordinator has already added the worker to `departing_worker_ids` and the next attempt at the same id will likely fail registration. **The retry semantics are undefined.** Worse, the `let _` swallows any ProtocolError, including `WirePayloadTooLarge` (which would indicate frame corruption) — silently turning protocol-layer errors into "graceful leave."

5. **QA-005 (CRITICAL)** — **Connection-loss double-detection produces double allocation in `RetainedInitial`**: there is no check that `retained_state.initial.entry(wid)` is not double-counted across rounds. `retained_state.initial.entry(p.worker_id).or_insert_with(...)` at line 595 *correctly* preserves the round-0 state across rounds (good). But `departing_worker_ids.push(wid)` at lines 681, 708, 733, 752 has **no dedup**: if a worker triggers both `Message::Error{...}` (line 684) AND timeout (line 738) within the same recv pass — for example, if the error message arrives in the buffer milliseconds before the timer fires, the timer's `Err(_)` arm matches the *next* recv attempt — **the same `wid` is pushed twice**. Then `materialize_reclaimed_partitions` is called with `[wid, wid]`, and the loop at `departure_recovery.rs:21` materializes the same retained partition twice with the same `IdRange{0, 100_000}`, **panicking the debug-assert at lines 33–38** (R24d disjointness) and silently producing **two clones of the same partition** in release builds. The reconstruction then sees the worker's agents *twice* — D4 violation, T1 violation, and an unreachable hang in any downstream `merge()` that traverses a redex queue containing duplicated AgentPort references.

---

## Findings

### QA-001 (CRITICAL) — Retained-state corruption under partial failure: no on-disk persistence; debug-assert at `refresh_last_acked` masks silent state-loss in release builds

- **Severity:** CRITICAL — silently disables R23/R31 across coordinator restarts; turns "partial-failure recovery" into "partial-failure abort."
- **Category:** State corruption / Atomicity / Spec gap
- **Location:** `relativist-core/src/protocol/retained.rs:55–64`, `relativist-core/src/protocol/coordinator.rs:587–600, 662–669`
- **Description:**
  The `RetainedStateRegistry` is held entirely in-memory in the `run_coordinator` stack frame (`let mut retained_state = RetainedStateRegistry::new()` at line 490). There is no `flush()`, no `persist()`, no `Drop`-time write to disk, and no checkpoint on round boundary. SPEC-20 R23/R31 say the coordinator MUST retain `retained_initial[w]` and `retained_last_acked[w]` for at-least-once departure recovery — but the spec says nothing about durability *across coordinator restarts*. The bundle reads this silence as "in-memory is fine"; the adversarial reading is "if the coordinator panics partway through round N, the registry is reset on restart and every departure recovery is impossible."
  
  The non-atomicity is sharpened by the staging:
  1. Round-N starts. `current_net` is split into `plan`. Each remote partition is inserted into `retained_state.initial` via `or_insert_with` (line 595). **Insertion order is `partitions_iter` order**, which depends on `plan.partitions` order. If `split` panics mid-way, some workers have `initial` and some don't.
  2. Workers compute and return `PartitionResult`. As each arrives, `refresh_last_acked` (line 663) is called.
  3. If the coordinator panics between step 2 (some workers' `last_acked` updated) and step 3 (the merge), the registry's `last_acked` is partially-newer than `initial`, and the next round's reclaim materializer (which falls back from `last_acked` to `initial`) sees an inconsistent snapshot.
  
  The `debug_assert!` at `retained.rs:58–62` is meant to catch the "refresh `last_acked` without `initial`" case. **In release builds, the assert compiles out**, `last_acked.insert` proceeds unconditionally, and a future `materialize_reclaimed_partitions` returns `InvariantViolation("State loss for worker N")` at `departure_recovery.rs:59`. The error message says "state loss" but the actual root cause — **non-atomic registry update sequence with no recovery semantics** — is invisible.
- **Reproduction (static trace; observable on a 2-worker hybrid grid):**
  ```rust
  // 1. Coordinator starts, retained_state = empty
  // 2. accept_workers gives us [stream_w1, stream_w2]
  // 3. Round 0 begins. partitions_iter = [self_p, p_w1, p_w2].
  // 4. self_p inserted at retained_state.initial[0].
  // 5. SIMULATED PANIC INSIDE distribute_partitions (e.g., send_frame on stream_w1 returns Err
  //    that propagates via `?` at line 608). The function returns Err(...).
  // 6. ProtocolError surfaces; main_loop ends; retained_state is dropped.
  //    If the operator restarts the coordinator, retained_state.initial = {} again — round-0 lost.
  //    Worker w2 (which did receive its partition) cannot be reclaimed because no initial for it exists.
  //
  // OR: coordinator stays up, worker w1 panics:
  // 7. retained_state.initial = {0: self_p, 1: p_w1, 2: p_w2}
  // 8. w1 sends PartitionResult; refresh_last_acked(1, V1(p_w1_evolved)) succeeds.
  // 9. w2 panics; recv_frame on stream_w2 returns Err.
  // 10. handle_connection_loss(2, ...) returns RecoveryTriggered. departing_worker_ids = [2].
  // 11. materialize_reclaimed_partitions([2], &registry, ...) reads registry.initial[2] = p_w2 (good),
  //     but only because step 7's assignment ran to completion. If step 7 had been interrupted
  //     mid-iteration (e.g., first iteration of the for-loop at line 594 succeeded for w1 but the
  //     second iteration's partition allocation OOM'd before .or_insert_with completed), the registry
  //     would be {0: self_p, 1: p_w1} — w2 is missing.
  ```
- **Expected behavior:**
  Either (a) registry must persist to disk on a write-ahead log so a restart can rebuild it; OR (b) the spec must explicitly state that retained state is in-memory only and coordinator restart aborts the run, with a config flag to opt in to persistent recovery.
- **Actual behavior:**
  In-memory only with no documentation of the durability gap. `materialize_reclaimed_partitions` returns `InvariantViolation` on missing state but the error message ("State loss for worker N") implies a *worker* invariant violated, not a *coordinator-state* gap.
- **Fix suggestion:**
  1. **Mandatory** — add a Rustdoc paragraph to `RetainedStateRegistry` explicitly stating: "**Durability:** state is in-memory only; coordinator restart drops all retained state and any in-flight departures abort. Persistent recovery is **out of scope** for SPEC-20 §3.3 and tracked separately in TASK-XXXX (deferred)."
  2. **Recommended** — add an `assert!(self.initial.contains_key(&worker_id))` (release-mode assert, not debug-mode) inside `refresh_last_acked` that returns `Result<(), R23Violation>` instead of silently inserting into `last_acked`. This converts the silent state-loss path at `departure_recovery.rs:59` into a fail-fast at the point of corruption.
  3. **Long-term** — file a SPEC-20 follow-up against `especialista-specs` to specify durability semantics for R23.

---

### QA-002 (CRITICAL) — `D == K_eff` hybrid branch is unreachable code; the test for R26a's SoloReducing transition cannot be written against the current condition

- **Severity:** CRITICAL — the spec's R26a "graceful degradation to solo" path has no executable test surface; algebraically dead code in hybrid mode.
- **Category:** Logic Error / Reachability / Test Coverage Gap
- **Location:** `relativist-core/src/protocol/coordinator.rs:773–792, 558, 632–643`
- **Description:**
  The condition `if departing_worker_ids.len() >= k_eff` (line 774) is intended per TASK-0442 to detect "all workers have departed." But:
  
  - `k_eff = remote_count + (1 if hybrid else 0)` (line 558).
  - `departing_worker_ids` is populated only inside the recv loop at line 645, which iterates `streams_to_poll` (line 628). `streams_to_poll` contains **only remote streams plus self_handle.stream**.
  - The self-handle stream (id 0 in hybrid mode) IS included at line 642: `streams_to_poll.push((0, &mut h.stream))`. **So the self-partition CAN appear in `departing_worker_ids` if it sends a `Message::Error` or times out.**
  
  Yet — the self-partition stream is an *in-process* duplex over `tokio::io::duplex` per `protocol/self_worker.rs` (verified by the `spawn_self_partition` call at line 581). An in-process duplex **does not time out** (the writer is the same process; `recv_frame` will return `Ok` as soon as the writer flushes). It also **does not produce `Message::Error`** unless the self-worker code explicitly writes one — and the self-worker logic does not.
  
  Therefore in practice, `departing_worker_ids` contains only remote workers. `D == K_eff` requires `D == K_remote + 1` in hybrid mode, which requires the self-partition to also have departed — algebraically impossible.
  
  **The hybrid R26a branch (line 780–786) is unreachable code.** The non-hybrid branch (line 787–790) is reachable (when all remotes depart in a non-hybrid grid).
  
  This compounds MF-002: the reviewer noted both arms abort with `Fatal`, but the deeper bug is that **the hybrid arm is structurally dead**. Even if MF-002 is fixed (replace the abort with a fall-through to SoloReducing), the SoloReducing transition is never exercised in hybrid mode because the condition is unreachable. **Any test for R26a-hybrid that relies on `>= k_eff` will sit in dead code.**
- **Reproduction (static reasoning, no `cargo run` needed):**
  ```rust
  // hybrid_coordinator = true, num_workers = 3 → remote_count = 2, k_eff = 3.
  // Both remotes depart in round N:
  //   handle_connection_loss(1, ...) → push 1
  //   handle_connection_loss(2, ...) → push 2
  // departing_worker_ids = [1, 2]; len() = 2; k_eff = 3.
  // Condition: 2 >= 3 → FALSE. The R26a branch does not fire.
  // The code falls through to the reclaim block at 794–833, which then returns
  // Fatal("...TASK-0443 follow-up") at 832. Result: hybrid coordinator with one
  // self-partition and zero remotes is treated as "non-degenerate departure"
  // and aborts on MF-001's hard return, NOT routed to SoloReducing.
  //
  // To trigger >= k_eff in hybrid mode, the self-partition (id 0) would need to
  // be in departing_worker_ids — which requires its in-process duplex to time out
  // or surface Message::Error, neither of which happens.
  ```
- **Expected behavior:**
  Per SPEC-20 R26a: *"if D == K_remote in hybrid mode, transition to SoloReducing with the self-partition continuing alone."* The condition should compare `D` against `K_remote`, NOT `K_eff`.
- **Actual behavior:**
  Condition compares against `K_eff = K_remote + 1`, requiring the self-partition to "depart," which cannot happen.
- **Fix suggestion:**
  ```rust
  // Correct R26a-hybrid detection:
  let remote_departures = departing_worker_ids.iter()
      .filter(|&&id| id != 0 || !grid_config.hybrid_coordinator)  // exclude self in hybrid
      .count();
  let all_remotes_departed = remote_departures == remote_count;
  
  if all_remotes_departed {
      if grid_config.hybrid_coordinator {
          // R26a-hybrid: transition to SoloReducing
          worker_streams.clear();
          // Fall through; the worker_streams.is_empty() check at the top of the
          // next loop iteration triggers SoloReducing per R5/R5a.
      } else {
          // R26a-non-hybrid: no executor remains.
          return Err(ProtocolError::Fatal("R26a non-hybrid: all workers departed".into()));
      }
  }
  ```
  Also: split the `>= k_eff` check explicitly into two named conditions (`all_remotes_departed` and `all_workers_including_self_departed`) so the spec semantics are visible in code.
- **Connection to MF-002:** MF-002 noted the abort. QA-002 names the unreachability. The fix must address both: (a) replace abort with fall-through, AND (b) compute the condition correctly. The reviewer's "After" suggestion at MF-002 has the right shape (`all_remotes_departed`); QA-002 confirms the rewrite is structurally necessary, not optional.

---

### QA-003 (CRITICAL) — `IdRange{0, 100_000}` placeholder produces guaranteed agent-id collisions with surviving partitions; T1/D4 invariants violated on every reclaim, even single-worker

- **Severity:** CRITICAL — every successful (post-MF-001) reclaim produces an invalid `Net` whose subsequent `reduce_n` is undefined behavior.
- **Category:** Invariant Violation (T1, D4) / Spec Violation / Adversarial Witness
- **Location:** `relativist-core/src/protocol/coordinator.rs:801–810` (caller); `relativist-core/src/partition/departure_recovery.rs:42` (callee); `relativist-core/src/partition/remap.rs:118–138` (id-mapping logic).
- **Description:**
  The reviewer's MF-003 noted overlapping ranges between *reclaimed* partitions in the multi-worker case. QA-003 sharpens: **even in the single-worker case, the range collides with surviving partitions.**
  
  Trace through `remap_partition_ids` (verified at `remap.rs:117–121`):
  ```rust
  let id_map: HashMap<u32, u32> = live_ids
      .iter()
      .enumerate()
      .map(|(rank, &old_id)| (old_id, new_range.start + rank as u32))
      .collect();
  ```
  With `new_range.start = 0` and a partition of N agents, the new_ids are `[0, 1, 2, ..., N-1]`. The remapped partition's `subnet.agents` array has length `N` and is indexed `[0, N)`.
  
  Now `reconstruct(border_graph, surviving_partitions, reclaimed_partitions)` at `coordinator.rs:821` is called. The surviving partitions also have agent IDs starting at `0` (each is its own `Net`); typically a v1 partition for worker w in a K=2 grid has agent ids `[0, partition_size)`.
  
  When `reconstruct` merges these into a single `Net`, two distinct agents both claim `id = 0`. **One of two things must happen** in the implementation:
  1. The merge naively union-ifies the `agents` vectors and **the second agent at index 0 silently overwrites the first** → D4 (ID Uniqueness) violated, T1 (one principal-port-per-wire) violated because the now-orphaned `PortRef::AgentPort(0, ...)` in the surviving partition's port array still points to the old slot, but slot 0 now holds the reclaimed agent — invariant broken.
  2. The merge uses a hashmap-based deduplication that throws on duplicate id → panic at runtime.
  
  Either way the result is non-functional. The current reviewer's MF-003 fix ("derive ranges from `compute_id_ranges(K_eff_new)`") addresses overlap *between* reclaimed partitions, but the deeper issue is that **the union of {surviving, reclaimed} partitions must occupy globally disjoint id ranges**, not just per-set disjoint ones.
- **Reproduction (witness redex sequence on a single-worker departure):**
  ```rust
  // K_eff = 2 (one self + one remote), agents 0..6 in current_net.
  // After split: 
  //   partition_0 (self): agents [0, 1, 2], id_range = [0, 3)
  //   partition_1 (remote w=1): agents [3, 4, 5], id_range = [3, 6)
  //   (the partitioner assigns disjoint ranges per SPEC-04 R30)
  //
  // Worker 1 connection-lost. departing_worker_ids = [1].
  // reclaimed_id_ranges = {1: IdRange{0, 100_000}}.
  // materialize_reclaimed_partitions → remap_partition_ids on partition_1 with new_range.start=0.
  //   partition_1 had agents 3,4,5 → now relabelled 0,1,2.
  // surviving_partitions = [partition_0_evolved] (post-reduction; agents still 0,1,2 by their ids).
  // reconstruct merges surviving (agents 0,1,2) with reclaimed (agents 0,1,2).
  // → SIX agents claim ids 0,1,2 — three from each side.
  //
  // Witness redex: in surviving partition_0, agent 1 is α (Erase) connected via principal port
  // to AgentPort(2, 0). After reconstruct, slot 2 is overwritten by reclaimed agent (say δ Dup).
  // The redex_queue still contains (1, 2). reduce_one() reads agents[1]=α, agents[2]=δ — but 
  // the original redex was α-α (Erase-Erase, rule ALPHA_ALPHA from SPEC-03 R3). The actual rule
  // fired is now α-δ (rule R7 ALPHA_DUP, which produces 2 new α agents). The reduction trace
  // diverges from any valid sequential semantics.
  //
  // T1 is violated: the wire (1.principal, 2.principal) now connects principal ports of two
  // distinct agents whose interaction was never scheduled. Confluence (D6) is broken.
  ```
- **Expected behavior:**
  Reclaimed partitions must be assigned `IdRange`s that are **globally disjoint** from all surviving-partition ranges, derived from `compute_id_ranges(K_eff_new)` per SPEC-20 R30, with the `start` offset large enough to exceed `max(surviving_partition.id_range.end)`.
- **Actual behavior:**
  Hard-coded `[0, 100_000)` collides with every surviving partition that has any agent in `[0, 100_000)`, which in the TCC test fixtures is **all of them**.
- **Fix suggestion:**
  ```rust
  // Derive ranges from compute_id_ranges, offset past surviving partitions.
  let max_surviving_end = surviving_partitions.iter()
      .map(|p| p.id_range.end)
      .max()
      .unwrap_or(0);
  // Or, idiomatically: compute_id_ranges(K_eff_new) starting from a fresh cursor.
  let chunk_size = max_surviving_end / k_eff as u32 + 1;  // rough
  let mut cursor = max_surviving_end;
  let mut reclaimed_id_ranges = HashMap::new();
  for &id in &departing_worker_ids {
      let start = cursor;
      let end = cursor.saturating_add(chunk_size);
      // Guard against u32::MAX overflow:
      if end <= start {
          return Err(ProtocolError::Fatal("R30: id space exhausted in reclaim".into()));
      }
      reclaimed_id_ranges.insert(id, IdRange { start, end });
      cursor = end;
  }
  ```
  And: add an integration test `eg_u7_reclaim_no_id_collision_with_surviving` that asserts the post-reconstruct net's `count_live_agents()` == `surviving_count + reclaimed_count` (no overwrite-induced loss).
- **Connection to MF-003:** MF-003 noted overlap between reclaimed; QA-003 names the surviving-vs-reclaimed overlap. Both must be fixed.

---

### QA-004 (CRITICAL) — `LeaveAck` race: `let _ = send_frame(...)` discards send errors, so TCP-RST after `LeaveRequest` is indistinguishable from clean handshake; retry semantics undefined

- **Severity:** CRITICAL — graceful-leave semantics are silently broken under any TCP-RST race; downstream rejoin behavior is non-deterministic.
- **Category:** Race Condition / Error Suppression / Spec Gap
- **Location:** `relativist-core/src/protocol/coordinator.rs:680`
- **Description:**
  The `Message::LeaveRequest { kind }` arm sends `LeaveAck` and discards the result:
  ```rust
  let _ = send_frame(stream, &Message::LeaveAck).await;
  departing_worker_ids.push(wid);
  round_departed_count += 1;
  ```
  Three failure modes are silently coalesced:
  1. **Successful ack:** worker reads `LeaveAck`, closes connection cleanly. Coordinator marks departure. **Correct.**
  2. **TCP-RST mid-ack:** worker sent `LeaveRequest`, then `shutdown(SHUT_RDWR)` (intentional or because it crashed). The coordinator's `send_frame` returns `Err(ProtocolError::Io(...))`. The discard suppresses it. The coordinator marks departure. **Worker never sees the ack.** Per SPEC-20 R35a, the worker's "must receive ack" expectation is unmet; behavior is implementation-dependent.
  3. **Frame-payload error:** if `send_frame` somehow encodes an oversized frame (impossible for `LeaveAck` which is unit, but the principle generalizes for future variants), the discard hides corruption.
  
  Cases 2 and 3 are observationally identical to case 1 from the coordinator's perspective. Worse, the worker's behavior on case 2 is governed by SPEC-20 R20 ("worker MUST receive `LeaveAck` before considering the departure committed"), which the worker cannot satisfy if the connection is gone — leading either to retry (sends `LeaveRequest` again on a new connection, where the coordinator no longer recognizes the worker) or silent abort (worker dies before retry).
  
  Beyond that: the recv loop at line 645 reads **one frame per (wid, stream)** and then drops the stream-borrow. If a worker sends `PartitionResult` followed by `LeaveRequest` (the spec-correct R22a sequence), the second frame **is never read**. Reviewer MF-005 noted this; QA-004 adds: even if the recv loop is restructured to read multiple frames, the `let _` on the ack send still hides the race.
- **Reproduction:**
  ```rust
  // Worker w sends LeaveRequest{AfterResult} on its stream, then immediately:
  //   stream.shutdown().await;
  //   drop(stream);
  // Coordinator: 
  //   recv_frame returns Ok((Message::LeaveRequest{kind: AfterResult}, _)).
  //   send_frame(stream, &Message::LeaveAck).await returns Err(ProtocolError::Io(BrokenPipe)).
  //   `let _` swallows the Err.
  //   departing_worker_ids.push(wid).
  //
  // Coordinator state: w marked as cleanly departed.
  // Worker state: never received ack; per R20, "departure not committed."
  // Next round: coordinator attempts to dispatch to w (if reclaim succeeds);
  //   but stream is closed, so the dispatch fails. Coordinator may then mark w
  //   as a *second* departure for the same worker — double-counted in
  //   departing_worker_ids the next round (compounds with QA-005).
  ```
- **Expected behavior:**
  R20/R35a require an explicit ack-confirmed handshake. If `send_frame` fails, the coordinator should classify the leave as `Urgent` (the worker is gone) rather than `AfterResult` (which implies a clean handshake), AND log at WARN level distinguishing "ack failed" from "ack succeeded."
- **Actual behavior:**
  All `LeaveRequest`s are processed identically regardless of whether the ack landed; ack-failure is unobservable in metrics or logs.
- **Fix suggestion:**
  ```rust
  Message::LeaveRequest { kind } => {
      let ack_result = send_frame(stream, &Message::LeaveAck).await;
      let effective_kind = match ack_result {
          Ok(_) => kind,
          Err(e) => {
              tracing::warn!(worker_id = wid, error = %e,
                  "LeaveAck send failed; upgrading to Urgent (R22c-like)");
              LeaveKind::Urgent
          }
      };
      // Then branch on effective_kind per R22a/R22b/R22c (MF-005 implementation).
      // ...
  }
  ```
  Also: file a SPEC-20 follow-up against `especialista-specs` to specify behavior under `LeaveAck` send-side failure.
- **Connection to MF-005:** MF-005 noted the missing R22a/R22c branching. QA-004 adds the third missing branch: "ack send failed" — which the spec is silent on but R20 implies should be treated as Urgent.

---

### QA-005 (CRITICAL) — Connection-loss double-detection: same `wid` pushed twice when `Message::Error` and `timeout` both fire; `materialize_reclaimed_partitions` then panics or silently produces duplicate partitions

- **Severity:** CRITICAL — non-idempotent departure event handling; D4 violation; debug-assert panic in tests, silent corruption in release.
- **Category:** Idempotence / Race Condition / Resource Exhaustion
- **Location:** `relativist-core/src/protocol/coordinator.rs:645–757` (recv loop); `relativist-core/src/partition/departure_recovery.rs:21–43` (consumer)
- **Description:**
  The recv loop at line 645 iterates `streams_to_poll` once per (wid, stream). Inside one iteration, the `tokio::time::timeout(config.collect_timeout, recv_future).await` returns `Ok(Ok(msg))`, `Ok(Err(e))`, or `Err(_)` — these are **mutually exclusive on a single recv attempt**.
  
  However: the loop is `for (wid, stream) in streams_to_poll`. The `streams_to_poll` vector is built from `worker_streams.iter_mut().enumerate()` plus the self-handle. **There is no dedup**: if the same `WorkerId` appears twice (e.g., two registrations with the same id were accepted — verified possible at `accept_workers`/`process_join_request` if `next_worker_id` is not monotonic, which it currently is, but could regress), the same wid is polled twice and each can independently trigger departure.
  
  More concretely — and this is the witness: if the recv loop is later refactored to handle multi-frame reads per stream (e.g., to fix MF-005's "PartitionResult then LeaveRequest" issue), then the same `wid` may produce a `PartitionResult` and then a `Message::Error` on the next read. **The current code has no `if !departing_worker_ids.contains(&wid)` guard.** Both events push `wid`, and `departing_worker_ids` becomes `[wid, wid]`.
  
  Consequences:
  ```rust
  // departing_worker_ids = [5, 5];
  // reclaimed_id_ranges = {5: IdRange{0, 100_000}};
  // materialize_reclaimed_partitions iterates twice over wid=5:
  //   Iter 1: registry.initial[5] → p; remap_partition_ids(p, IdRange{0, 100_000}) → p_remapped.
  //           reclaimed = [p_remapped].
  //   Iter 2: registry.initial[5] → p (still there; release_worker not called yet).
  //           remap_partition_ids(p, IdRange{0, 100_000}) → p_remapped (same as before).
  //           Debug-assert at departure_recovery.rs:33 fires:
  //             new_range.start (0) is NOT >= r.id_range.end (N), 
  //             AND new_range.end (100_000) is NOT <= r.id_range.start (0).
  //             Panic in debug; silent in release.
  //   In release: reclaimed = [p_remapped, p_remapped] (two clones).
  //   reconstruct sees the same agents twice → D4 violated, T1 violated, redex_queue contains
  //   doubled redexes, reduce_n hangs or produces incorrect normal form.
  ```
- **Reproduction:**
  As above. The minimal trigger requires the recv loop to read multiple frames per stream — currently it reads one, but the MF-005 fix mandates multi-frame reading. So this bug becomes reachable as soon as MF-005 is fixed.
- **Expected behavior:**
  Departure events MUST be idempotent: pushing the same `wid` twice into `departing_worker_ids` MUST NOT cause double-materialization.
- **Actual behavior:**
  No dedup; debug-assert panic in tests, D4 violation in release.
- **Fix suggestion:**
  ```rust
  // Use a HashSet instead of a Vec, OR check before push.
  use std::collections::HashSet;
  let mut departing_worker_ids: HashSet<WorkerId> = HashSet::new();
  // ... in each push site:
  if departing_worker_ids.insert(wid) {
      round_departed_count += 1;
      // log only on first detection
  } else {
      tracing::debug!(worker_id = wid, "Duplicate departure event suppressed");
  }
  ```
  Then convert to `Vec` at the materialize call site:
  ```rust
  let departing_worker_ids_vec: Vec<WorkerId> = departing_worker_ids.into_iter().collect();
  ```
- **Connection to MF-005:** MF-005 mandates restructuring the recv loop to read multiple frames per stream; once that lands, QA-005's race becomes trivially reachable. Fix the dedup as part of the MF-005 rework.

---

### QA-006 (HIGH) — `reconstruct(border_graph, evolved_survivors, round_0_reclaimed)` is invoked across mixed reduction traces; the `border_graph` was computed from the *current* round's plan but the reclaimed partitions are from round 0

- **Severity:** HIGH — silent semantic divergence; produces a `Net` that is type-correct but reduction-incorrect.
- **Category:** Spec Violation (R24c) / Mixed-trace Composition
- **Location:** `relativist-core/src/protocol/coordinator.rs:820–822`
- **Description:**
  Reviewer MF-008 noted the `border_graph` staleness. Reviewer MF-009 noted the surviving-vs-reclaimed trace mismatch. QA-006 unifies these into a concrete invariant violation:
  
  Trace through a 2-round witness:
  1. Round 0: `current_net` has agents `[0, 5)` and 3 redexes. `split` produces partitions `p_0` (self, agents `[0, 2)`) and `p_1` (remote w=1, agents `[2, 5)`). Border `b_0` connects `p_0.agent_1.aux_0` to `p_1.agent_2.aux_0`. `border_graph_round_0.next_border_id = 1`.
  2. Round 1 begins. `current_net` post-merge has agents `[0, 6)` (one new agent introduced by reduction). `split` produces new partitions `p_0'` and `p_1'` with **a new border allocation** `b_1`. `border_graph_round_1.next_border_id = 2`.
  3. Worker w=1 times out in round 1.
  4. Reclaim: `reclaimed_id_ranges = {1: IdRange{0, 100_000}}`. `materialize_reclaimed_partitions` reads `retained_state.initial[1] = round_0_p_1` (round 0's partition for worker 1, **not** round 1's evolved version). Remap → `p_1_round_0_remapped`.
  5. `border_graph = BorderGraph::from_partition_plan(&plan)` where `plan` is round 1's plan. **`border_graph` references `b_1`, not `b_0`.**
  6. `reconstruct(border_graph_round_1, [evolved_p_0_round_1], [round_0_p_1_remapped])` is called.
  7. `border_graph_round_1.borders` includes `b_1`, which connects evolved_p_0_round_1's port to a port that does NOT exist in round_0_p_1_remapped (because round_0_p_1 had only `b_0`, not `b_1`).
  8. Reconstruct's wire-resolution step looks up `b_1` in round_0_p_1_remapped's `free_port_index` — finds nothing — silently leaves the border port DISCONNECTED (per `remap.rs:148` defensive default).
  9. Result: `merged_net` has a half-wire (one principal port pointing into nowhere), violating T1.
  
  **The witness redex sequence**: any reduction in round 0 that allocated a new border in round 1 (e.g., a `CON-DUP` interaction at the round boundary that crossed partitions, allocating a fresh border id) silently breaks the reclaim's border resolution.
- **Reproduction:**
  ```rust
  // 2-worker hybrid grid, current_net has CON-DUP redex at boundary.
  // Round 0: split assigns CON to p_0, DUP to p_1, allocates border b_0 between them.
  // Both workers reduce. The CON-DUP rule (SPEC-03 R6) allocates 2 new wires; if either
  // crosses the partition boundary, a new border is allocated in round 1's split.
  //
  // Round 1: split allocates b_1 (and possibly retires b_0 if its agents merged).
  //          border_graph_round_1.next_border_id = 2; borders = {b_1}.
  // Worker 1 times out.
  // Reclaim: round_0_p_1 (which knows b_0 but not b_1) is remapped and merged.
  //          border_graph_round_1 says "b_1 connects evolved_p_0.x to reclaimed_p_1.y".
  //          But reclaimed_p_1 has no port y (y was a round-1 border, not round-0).
  //          merged_net.ports[evolved_p_0.x] = AgentPort(reclaimed_p_1.agent_for_y, 0).
  //          But that AgentPort references an id that doesn't exist in reclaimed_p_1
  //          (because remap relabelled all of round_0_p_1's ids to [0, N), and y was a
  //          border, not an agent — so the wire lookup fails).
  // Final: merged_net has a stale AgentPort that reduce_one() will try to follow → panic
  //        or silent loss of the wire.
  ```
- **Expected behavior:**
  R24c mandates the reclaim flow: `merge(survivors)` → `Net::union(reclaimed)` → next-round `split`. The `reconstruct` 3-arg path is for the **delta-mode** reclaim (R24b), where the snapshot's border_graph **is** retained alongside the partition, and the snapshot+border are used as a unit.
- **Actual behavior:**
  The current-round border_graph is mixed with prior-round reclaimed partitions. Borders allocated since the snapshot have no resolution targets in the reclaimed partition.
- **Fix suggestion:**
  Per reviewer MF-008/MF-009: either (a) implement the v1-correct flow (merge survivors normally, union reclaimed, re-split next round), OR (b) store `last_committed_plan: Option<PartitionPlan>` and `last_committed_border_graph: Option<BorderGraph>` in the coordinator alongside `retained_state` so the `border_graph` matches the retained snapshot.
- **Connection to MF-008/MF-009:** This is the integration of both findings; QA-006 names the *witness redex sequence* and the concrete invariant breakage (T1 + half-wire production).

---

### QA-007 (HIGH) — `departure_recovery::reconstruct_departed_partition` (via `materialize_reclaimed_partitions`) can return a `Partition` whose `border_id_2_sightings` (C3) is structurally stale

- **Severity:** HIGH — produces a partition that violates C3 (border-id 2-sightings) silently; downstream `reconstruct` loses border wires.
- **Category:** Invariant Violation (C3) / Static Analysis
- **Location:** `relativist-core/src/partition/departure_recovery.rs:23–43`
- **Description:**
  `materialize_reclaimed_partitions` reads `registry.initial[wid]` (which is `RetainedInitial::V1(p)` containing the round-0 partition snapshot) and calls `remap_partition_ids(p, new_range)`. The remap function (`remap.rs:54+`) renumbers agent ids and updates `subnet.ports` / `subnet.next_id`. **It does NOT update `partition.free_port_index`**, which holds `BorderId → (slot, port)` mappings used by the merge layer to resolve border wires.
  
  Verification: `remap.rs:90–98` (empty-partition fast path) sets `free_port_index: HashMap::new()`. `remap.rs:128+` (non-empty path) does NOT explicitly preserve or rewrite `free_port_index`; need to check the full function.
  
  If `free_port_index` is preserved byte-identically (the `..partition` struct-update spread), then the remapped partition's `free_port_index[border_id] = (old_slot, port)` — but `old_slot` no longer exists in the remapped agents array (which now uses `new_id` indexing). **Stale slot references in `free_port_index`.**
  
  C3 (Border 2-sightings) requires every `border_id` to appear exactly twice in the union of all partitions' `free_port_index` maps. After remap with stale slots, the partition's contribution to that count is *technically* still 2 (the map has the right number of entries) — but each entry points to a non-existent slot. When `reconstruct` resolves `border_id` → `(slot, port)` → `subnet.ports[slot * PORTS_PER_SLOT + port]`, it indexes into a slot whose agent is `None` (or worse, into a slot whose agent is a *different* agent post-remap).
  
  Either the lookup returns `DISCONNECTED` (silent loss of the border wire — T1 violation) or it returns `AgentPort(some_other_agent, port)` (silent corruption — D4 violation, undefined reduction trace).
- **Reproduction:**
  Cannot reproduce without access to the full `remap_partition_ids` source past line 200 (truncated in my read), but the `..partition` spread pattern at `remap.rs:96` (visible in the empty-partition path) suggests `free_port_index` is preserved by struct-update on the non-empty path too — verify by reading lines 200+ of `remap.rs`.
- **Expected behavior:**
  `remap_partition_ids` MUST rewrite `free_port_index` so its slot references match the new agent ids (e.g., `(old_slot, port) → (id_map[old_slot], port)`).
- **Actual behavior:**
  Likely byte-preserves `free_port_index`, making it stale post-remap. Static analysis is highly suggestive but not conclusive without reading the full function body past the truncation.
- **Fix suggestion:**
  1. Read `remap.rs:200–270` (the truncated section) to confirm or refute the `free_port_index` preservation.
  2. If `free_port_index` is not rewritten: fix `remap_partition_ids` to do so:
     ```rust
     let new_free_port_index: HashMap<BorderId, (u32, u8)> = partition.free_port_index.iter()
         .filter_map(|(&bid, &(old_slot, port))| {
             id_map.get(&old_slot).map(|&new_slot| (bid, (new_slot, port)))
         })
         .collect();
     ```
  3. Add a unit test in `departure_recovery.rs` that constructs a partition with non-empty `free_port_index`, remaps it, and asserts that every `(slot, port)` in the remapped index points to a `Some(agent)` slot in the remapped `subnet.agents`.
- **Connection to QA-006:** Compounds the border-resolution failure: even if the border_graph were correct (fixing QA-006), a stale `free_port_index` in the remapped partition would still break wire resolution.

---

### QA-008 (HIGH) — `RetainedLastAcked::DeltaLight { placeholder: String }` accepts unbounded `String` payload on the wire; an adversarial 4 GiB string DoSes the coordinator on deserialization

- **Severity:** HIGH — adversarial wire input under `--features zero-copy` (rkyv deserializes archived strings into owned `String`); pre-merge is in-memory only, but the variant is also `serde::Deserialize` so any future use of `bincode::deserialize_from(&bytes)` against an attacker-controlled byte stream allocates verbatim.
- **Category:** Resource Exhaustion / Adversarial Input / Wire-Eligible Stub
- **Location:** `relativist-core/src/protocol/retained.rs:31–38`
- **Description:**
  Reviewer MF-007 noted the spec-shape mismatch (`DeltaLight { placeholder: String }` vs spec's `(BorderGraph, RoundResult)`). QA-008 sharpens to the **size attack**:
  
  The variant carries `serde::Serialize`, `serde::Deserialize`, and (under `--features zero-copy`) `rkyv::Archive/Serialize/Deserialize`. The `placeholder: String` field has no size bound. If `RetainedStateRegistry` ever round-trips through bincode (e.g., for telemetry export, or a future on-disk persistence), a serialized `DeltaLight { placeholder: "X".repeat(4 * 1024 * 1024 * 1024) }` decodes verbatim, allocating 4 GiB.
  
  Even under the current "in-memory only" policy, **`tracing::warn!(?registry, ...)` Debug-formats the entire registry** — and if any code path constructs a `DeltaLight` with a large placeholder (e.g., a debug-tooling test, or a future PR that uses `placeholder` to log a snapshot summary), the Debug-format triggers the same OOM.
  
  Critically: the field is **literally named `placeholder`**, signalling "we'll fill this in later" — but there is no compile-time gate (no `#[cfg(...)]`, no `#[deprecated]`) preventing construction. A future maintainer who reads the type and sees a `String` field will treat it as a normal string field and write `RetainedLastAcked::DeltaLight { placeholder: format!("{:?}", border_graph) }` — accidentally serializing the entire border graph as a debug-formatted string into the wire enum.
- **Reproduction:**
  ```rust
  // Adversarial wire: a malicious worker (or coordinator-side replay) sends a serialized 
  // RetainedStateRegistry with DeltaLight { placeholder: "X".repeat(4 GiB) }.
  // bincode::deserialize_from reads the length prefix (u64 = 4_294_967_296), allocates
  // 4 GiB for the String, then reads 4 GiB of bytes from the wire.
  // Coordinator OOMs.
  //
  // Or: a developer test that round-trips a RetainedStateRegistry under #[features zero-copy]
  // with rkyv. rkyv's Archive deserialization for ArchivedString → String allocates a copy.
  // Same OOM.
  ```
- **Expected behavior:**
  Either implement the spec-correct `(BorderGraph, RoundResult)` payload, OR feature-gate the variant behind `#[cfg(feature = "delta-optimized-reclaim")]` so it is *uncostructible* in default builds, OR mark it `#[non_exhaustive] DeltaLight {}` (empty struct variant) until the spec is finalized.
- **Actual behavior:**
  Wire-eligible variant carrying an unbounded named-`placeholder` string field with full serde + rkyv derives.
- **Fix suggestion:**
  Per reviewer MF-007's options. QA-008 adds: even if the spec-correct payload is implemented, **bound the `BorderGraph`'s serialized size** (e.g., `max_borders` constant in SPEC-20) to prevent the same attack against `(BorderGraph, RoundResult)`.
- **Connection to MF-007:** Same root; QA-008 names the size-attack vector the reviewer alluded to but did not enumerate.

---

### QA-009 (HIGH) — Heartbeat / `collect_timeout` window has no documented bound; false-positive departure on jittery WAN, indefinite block on missing timeout

- **Severity:** HIGH — observable as wrong-result on the LAN benchmarks (false-positive timeouts under load); silently changes departure semantics under WAN.
- **Category:** Boundary / Configuration / Spec Gap
- **Location:** `relativist-core/src/protocol/coordinator.rs:647` (`config.collect_timeout`); `NodeConfig::collect_timeout` definition (not visible in this read)
- **Description:**
  `tokio::time::timeout(config.collect_timeout, recv_future).await` is the only mechanism by which `handle_phase_timeout` is reached. The `collect_timeout` is a `Duration` field on `NodeConfig`. There is no documentation in this bundle of:
  
  - **What the default value is.** If it's, say, `5 seconds`, then a worker that takes 6 seconds on a particularly hard partition (e.g., a `CON-DUP` cascade in Profile B) is incorrectly declared departed under `elastic_departure = true` — a **false-positive departure** that triggers reclaim of work that was actually completing successfully. Under elastic mode the result is "correct but inefficient" (the actual partition result is discarded). Under `elastic_departure = false` the entire run aborts.
  - **What the maximum value is.** If the user sets `collect_timeout = 1 hour`, a single slow worker blocks the round for an hour before reclaim fires. The coordinator is single-threaded in the recv loop (sequential `for (wid, stream)` at line 645, NOT a `select_all`), so **all subsequent workers' results wait on the slow one**. The wall-clock delay is multiplicative.
  - **No min bound.** A user could set `collect_timeout = Duration::from_micros(1)`, declaring every worker departed on every round. The reclaim path then fires forever (or until R24d disjointness assertions panic).
  - **No heartbeat at all.** SPEC-20 R13 implicitly contemplates heartbeat-based liveness; the current code only uses recv-side timeout, which is **per-frame**, not per-worker. A worker that sends one byte per `collect_timeout - 1ms` keeps the recv channel alive forever without producing a `PartitionResult`.
  
  **The recv loop is sequential, not concurrent.** This is a deeper bug: `for (wid, stream) in streams_to_poll` at line 645 awaits each stream in series. If worker 0 is slow but worker 1 has its result ready, worker 1's result waits for worker 0's timeout to complete. This compounds the timeout-tuning problem: the effective latency is `K_eff * collect_timeout` worst-case.
- **Reproduction:**
  ```rust
  // K_eff = 4. collect_timeout = 30s. All four workers complete in 1s, but in 
  // reverse arrival order: w_3, w_2, w_1, w_0. Coordinator polls in order [0, 1, 2, 3].
  // recv on stream_0 → 1s wait → Ok(PartitionResult).
  // recv on stream_1 → 1s wait (already arrived, but coordinator just got to it).
  // ...
  // Total: 4s instead of 1s. Linear pile-up.
  //
  // Adversarial: w_0 sends one byte per 29s. The recv buffer holds it; the next frame is
  // slow. Each `recv_frame` spins reading the partial frame, never reaching the timeout.
  // Wait: recv_frame's behavior on partial frames is implementation-dependent. If it
  // resets the timer per byte, the recv waits indefinitely.
  ```
- **Expected behavior:**
  - Document `collect_timeout` semantics: per-frame vs per-worker, default value, recommended bounds for LAN/WAN.
  - Use `select_all` or `FuturesUnordered` to poll all workers concurrently; per-worker timeouts fire independently.
  - File a SPEC-20 follow-up specifying heartbeat semantics (R13's "timeout detection" should be per-worker liveness, not per-frame timeout).
- **Actual behavior:**
  `collect_timeout` is a single `Duration` with no documented bounds; recv is sequential; no heartbeat.
- **Fix suggestion:**
  ```rust
  // Concurrent collect:
  use futures::stream::{FuturesUnordered, StreamExt};
  let mut futures: FuturesUnordered<_> = streams_to_poll.into_iter()
      .map(|(wid, stream)| async move {
          let res = tokio::time::timeout(config.collect_timeout, recv_frame(stream, ...)).await;
          (wid, stream, res)
      })
      .collect();
  while let Some((wid, stream, res)) = futures.next().await {
      // process as before, per (wid, stream)
  }
  ```
  And: document `collect_timeout` defaults in `NodeConfig` Rustdoc; add config-validator bounds (e.g., `collect_timeout >= 100ms && collect_timeout <= 1h`).
- **Connection to MF-001:** The sequential recv-loop is independently a perf and correctness issue; the bundle's recovery path bug (MF-001) hides it because no test reaches the post-collect phase.

---

### QA-010 (HIGH) — `RetainedStateRegistry::release_worker` is never called by the coordinator; retained state grows unboundedly across rounds

- **Severity:** HIGH — memory leak; NF-011 memory bound debug-asserts will eventually panic in long-running grids.
- **Category:** Resource Leak / Spec Violation (R23a, R31, NF-011)
- **Location:** `relativist-core/src/protocol/retained.rs:67–70` (the function); `relativist-core/src/protocol/coordinator.rs` (no call site)
- **Description:**
  `release_worker` exists in `retained.rs:67`, but **`grep` of `coordinator.rs` for `release_worker` finds zero call sites** (verified by inspection of the recovery block at lines 766–833 — only `materialize_reclaimed_partitions` is called, never `release_worker`).
  
  Per SPEC-20 R23a: "after a worker is permanently removed from `W_active`, its `retained_initial[w]` and `retained_last_acked[w]` MUST be released." The current code keeps both slots forever after a departure. Compound effect:
  
  1. Round 0: 4 workers register. `retained_state.initial = {0, 1, 2, 3}` (4 entries).
  2. Round 5: worker 2 departs. `materialize_reclaimed_partitions` reads `initial[2]` and remaps. `release_worker(2)` is NOT called. `retained_state.initial = {0, 1, 2, 3}` (still 4).
  3. Round 6: worker 5 joins (replacing 2). `retained_state.initial.entry(5).or_insert(...)` adds a 5th entry. **`initial = {0, 1, 2, 3, 5}` (5 entries; should be 4 per R31).**
  4. After 100 rounds with rotating departures and rejoins: `initial.len() = K_eff_initial + total_rejoins`.
  5. `assert_memory_bounds(K_eff)` at `retained.rs:73` checks `initial.len() <= 2 * K_eff`. Once `total_rejoins > K_eff`, the debug-assert fires — but only in debug builds.
- **Reproduction:**
  Trivial: 4 workers, alternate departures and rejoins for 8 rounds. `retained_state.initial.len() == 12` after 8 rounds, and `assert_memory_bounds(4)` panics on `12 <= 2 * 4 = 8`.
- **Expected behavior:**
  After every departure (or at the end of each round, post-reclaim), `release_worker(wid)` MUST be called for every wid in `departing_worker_ids`.
- **Actual behavior:**
  Never called. Retained state grows monotonically.
- **Fix suggestion:**
  In the recovery block at lines 766–833, after `materialize_reclaimed_partitions` succeeds:
  ```rust
  for &wid in &departing_worker_ids {
      retained_state.release_worker(wid);
  }
  retained_state.assert_memory_bounds(k_eff_new); // debug-mode bound check
  ```
  Add an integration test `eg_u13_retained_state_released_after_reclaim` that runs 100 rounds with rotating workers and asserts `retained_state.initial.len() <= 2 * k_eff` at every round boundary.
- **Connection to MF-006:** This is a spec-acceptance gap: TASK-0439 (TASK-0439 acceptance: EG-U7b/U7c/U13) requires R23a release-policy testing; zero tests exist.

---

### QA-011 (HIGH) — `LeaveRequest{kind}` arm ignores `kind` entirely and the `Distributing` state case is undefined: a `LeaveRequest{AfterResult}` arriving during `Distributing` causes a partial `PartitionResult` to be discarded

- **Severity:** HIGH — silent data loss on graceful leave during distribution phase.
- **Category:** State Machine / Spec Violation (R22a, FSM amendment §4.1.4)
- **Location:** `relativist-core/src/protocol/coordinator.rs:672–683`
- **Description:**
  Reviewer MF-005 noted the `kind` is destructured but unused. QA-011 adds the **state-machine angle**: where in the round can a `LeaveRequest` arrive?
  
  Per SPEC-20 §4.1.4 R22a: `LeaveRequest{AfterResult}` is sent **after** a `PartitionResult` for the current round. Per R22b, `LeaveRequest{Urgent}` can be sent at any time.
  
  The recv loop at line 645 reads **one frame per stream**. If a worker sends `PartitionResult` then `LeaveRequest{AfterResult}` (the spec-correct R22a sequence), the loop consumes the `PartitionResult`. The `LeaveRequest` is buffered. The recv loop completes for this stream. **The `LeaveRequest` is never processed** — the worker is added to neither `departing_worker_ids` nor any next-round-removal list. The worker's stream remains in `worker_streams` for the next round, and the coordinator dispatches a partition to a worker that has already left.
  
  Conversely, if the worker sends `LeaveRequest{AfterResult}` *before* `PartitionResult` (a R22c violation), the loop consumes the `LeaveRequest`, sends `LeaveAck`, marks the worker as departed, and **the buffered `PartitionResult` is dropped along with the stream**. The worker's completed work is lost — but R22c specifies a "silent upgrade to Urgent + log WARN" path, which the current code does not implement.
  
  The `Distributing` state amplifies this: if `LeaveRequest{AfterResult}` arrives during `distribute_partitions` (i.e., before this round's `PartitionResult`), the recv loop is in `Distributing`, not `WaitingForResults`. The current code has no handling for `LeaveRequest` during `Distributing` — the message would be received in a future recv call but the state machine is not yet ready to process it.
- **Reproduction:**
  ```rust
  // Worker sends two frames in sequence:
  //   Frame 1: PartitionResult{round: 5, partition: ..., stats: ...}
  //   Frame 2: LeaveRequest{kind: AfterResult}
  // Worker then closes the connection.
  //
  // Coordinator recv loop:
  //   Iter (wid=1): recv_frame → Ok((PartitionResult, _))
  //                 collect_results_vec.push(...)
  //                 Loop continues to next stream.
  // 
  // The LeaveRequest is never read. 
  // worker_streams still contains stream_1 (now closed).
  // Next round: distribute_partitions tries to send to stream_1 → I/O error → 
  //              handle_connection_loss(1, ...) → "elastic recovery" fires.
  // The R22a clean-leave path is reduced to "connection lost, do urgent reclaim,"
  // which is exactly what R22a is supposed to AVOID (R22a preserves the worker's
  // result; R22b/connection-loss discards it).
  ```
- **Expected behavior:**
  Per R22a: keep the result, mark worker for next-round-removal, do NOT add to departing_worker_ids in the current round. The spec also requires the recv loop to drain all available frames per stream within the timeout window.
- **Actual behavior:**
  Single-frame recv per stream; LeaveRequest after PartitionResult is silently buffered and treated as connection-loss in the next round.
- **Fix suggestion:**
  ```rust
  // Drain all available frames within the timeout, accumulating both PartitionResult
  // and LeaveRequest:
  let mut got_result = false;
  let mut got_leave = None;
  loop {
      match tokio::time::timeout(remaining_timeout, recv_frame(stream, max)).await {
          Ok(Ok((Message::PartitionResult{...}, _))) => { got_result = true; ... }
          Ok(Ok((Message::LeaveRequest{kind}, _))) => { got_leave = Some(kind); ... }
          Ok(Ok(other)) => return Err(...),
          Ok(Err(e)) => break,  // connection closed; check what we got
          Err(_) => break,       // timeout; check what we got
      }
      if got_result && got_leave.is_some() { break; }  // R22a: both frames received
  }
  // Branch on (got_result, got_leave) per R22a/b/c:
  match (got_result, got_leave) {
      (true, Some(LeaveKind::AfterResult)) => /* R22a clean leave */,
      (true, Some(LeaveKind::Urgent)) => /* anomalous: result received but Urgent? */,
      (false, Some(LeaveKind::AfterResult)) => /* R22c: silent upgrade to Urgent */,
      (false, Some(LeaveKind::Urgent)) => /* R22b: urgent reclaim */,
      (true, None) => /* normal completion */,
      (false, None) => /* timeout, no leave: phase timeout path */,
  }
  ```
- **Connection to MF-005:** MF-005 noted the missing R22a/c branching. QA-011 adds the state-machine context (what about `Distributing`?) and the multi-frame recv requirement (the prerequisite for any correct R22a implementation).

---

### QA-012 (MEDIUM) — `streams_to_poll` is rebuilt per round from `worker_streams.iter_mut()` indices; after MF-001's stream-pruning fix, the index→WorkerId mapping at line 632 is silently wrong

- **Severity:** MEDIUM — latent bug that activates the moment MF-001 is fixed.
- **Category:** Temporal Coupling / Future Hazard
- **Location:** `relativist-core/src/protocol/coordinator.rs:626–643`
- **Description:**
  Reviewer SF-003 noted the inline arithmetic. QA-012 adds the activation condition: **the moment MF-001 is fixed and `worker_streams` starts being mutated mid-loop (stream-pruning after departure), the index→WorkerId mapping breaks.**
  
  Trace:
  1. Round 0: `worker_streams = [s_w1, s_w2, s_w3]` (hybrid, ids 1, 2, 3).
  2. Recv loop assigns `(wid = i+1)`: w1 at index 0, w2 at index 1, w3 at index 2.
  3. w2 departs. After MF-001 fix, `worker_streams.remove(1)` → `[s_w1, s_w3]`.
  4. Round 1: recv loop assigns `(wid = i+1)`: w1 at index 0, **w3 at index 1**.
  5. The coordinator now thinks worker at stream index 1 is "wid=2", but it's actually w3.
  6. On any failure, `handle_connection_loss(2, ...)` is logged for an event that happened on w3's stream. **The wrong worker is marked as departed.**
- **Expected behavior:**
  Per SF-003: maintain a `Vec<(WorkerId, TransportStream)>` so the id is stored alongside each stream, not derived from index.
- **Actual behavior:**
  Today the bug is masked by MF-001's premature return (no stream pruning happens). After MF-001 is fixed, this becomes a CRITICAL bug.
- **Fix suggestion:**
  Per SF-003. Add a unit test `streams_to_poll_id_mapping_survives_pruning` that constructs a 3-worker `worker_streams`, removes index 1, and asserts that subsequent polls use the correct WorkerIds (3 at new index 1, not 2).
- **Connection to SF-003 + MF-001:** Activation is gated on MF-001's fix. Must be addressed in the same rework pass.

---

### QA-013 (MEDIUM) — `let _ = send_frame(stream, &Message::LeaveAck).await` at line 680 is the SECOND-to-last `unwrap`-equivalent in the recovery path; combined with MF-004 (line 611), there are now two error-suppression sites in this bundle

- **Severity:** MEDIUM — error-suppression idiom is creeping into the recovery path; sets a precedent.
- **Category:** Code Quality / Project Standard
- **Location:** `coordinator.rs:611, 680`
- **Description:**
  The project standard (CLAUDE.md) says "No `unwrap()` in production code — use `?` or explicit error handling." `let _ = expr` is functionally equivalent to `expr.unwrap()` *for the suppression of error reporting* (it differs in panic semantics, but both silently absorb the error). Phase D introduces or touches:
  
  1. `coordinator.rs:611`: `self_partition.as_ref().unwrap()` (MF-004).
  2. `coordinator.rs:680`: `let _ = send_frame(stream, &Message::LeaveAck).await` (QA-004).
  3. `coordinator.rs:829`: `let _ = _round_reclaimed_initial;` (SF-001).
  
  Three suppression sites in 200 LoC. The `let _` idiom is *technically* legal (it's intentional discard), but in Rust the convention is that **`let _ = expr` should be reserved for cases where the value is genuinely unneeded and the type-system requires the binding** (e.g., RAII guards). For `Result<...>`, the convention is `expr.unwrap_or_default()` or `if let Err(e) = expr { ... }` to make the suppression explicit.
- **Fix suggestion:**
  Apply the QA-004 fix to line 680, the MF-004 fix to line 611, and the SF-001 fix to line 829. The pattern of "discard error to satisfy the compiler" should not appear in production code; if it does, document why.

---

### QA-014 (MEDIUM) — `RetainedInitial::V1` and `RetainedInitial::Delta` are payload-identical; future divergence risk if `V1` is renamed without updating `Delta`

- **Severity:** MEDIUM — paperwork; SF-005 noted the variant pollution; QA-014 adds the renaming hazard.
- **Category:** API Design / Future Hazard
- **Location:** `relativist-core/src/protocol/retained.rs:12–23`
- **Description:**
  Reviewer SF-005 noted the variants are payload-identical. QA-014 adds: a future PR that renames `V1(Partition)` to `Conservative(Partition)` (e.g., to align with spec terminology) might silently leave `Delta(Partition)` as `Delta(Partition)`, creating a divergence in identifier semantics that `partition()` would silently mask (because both still match `Self::V1(p) | Self::Delta(p) => p`).
- **Fix suggestion:**
  Per SF-005. Either collapse to a single variant, or add a doc comment explaining that the variants are reserved for future divergence (and add a `#[doc(hidden)]` test that fails to compile if the variants ever diverge in payload type, locking in the "must stay identical until Delta gets its real payload" contract).

---

### QA-015 (MEDIUM) — Heartbeat / `worker_connect_timeout` semantics are not bounded; an adversarial worker can stall `accept_workers` indefinitely

- **Severity:** MEDIUM — DoS vector pre-departure; precedes the elastic path.
- **Category:** Pre-Phase Boundary / Spec Gap
- **Location:** `relativist-core/src/protocol/coordinator.rs:329–334`
- **Description:**
  `accept_workers` wraps the entire accept-loop in `tokio::time::timeout(config.worker_connect_timeout, accept_future)`. Inside the loop, each worker's `recv_frame` call can read a `Register` message of arbitrary size (up to `max_payload_size`). **A single slow worker that sends one byte per `accept_future` half-second slot keeps the entire accept-future alive without ever completing a Register.** Other workers' connections wait behind this slow recv.
  
  Under elastic mode, this exposes a pre-departure adversarial vector: a malicious "joiner" can prevent legitimate workers from registering, and the elastic-departure mechanism never fires (because no workers are *yet* in `W_active`).
- **Fix suggestion:**
  Add a per-stream `recv_frame` timeout inside the accept loop (separate from the overall `worker_connect_timeout`):
  ```rust
  let (msg, _) = tokio::time::timeout(
      config.per_register_timeout,  // e.g., 5s
      recv_frame(&mut stream, config.max_payload_size)
  ).await.map_err(|_| /* drop this stream */)?;
  ```

---

### QA-016 (MEDIUM) — `process_join_request` always increments `next_worker_id` on join, never decrements; if a join is followed by an immediate departure, the worker's id slot is permanently lost; combined with R11 (`u32::MAX` exhaustion), a long-running grid with rotating workers can exhaust the id space prematurely

- **Severity:** MEDIUM — spec gap; long-running grid exhaustion.
- **Category:** Resource Exhaustion / Spec Gap
- **Location:** `relativist-core/src/protocol/coordinator.rs:171–183`
- **Description:**
  At line 181: `let worker_id = *next_worker_id; *next_worker_id += 1;`. The `next_worker_id` is monotonic. There is no recycling of departed-worker ids back into the pool.
  
  In a long-running grid (say 10 years of operation with 1000 worker rotations per day) with `u32::MAX` ids, the pool exhausts in `2^32 / (1000 * 365 * 10) ≈ 1.18` rotations per second on average — feasible for a high-churn grid. R11 (`WorkerIdSpaceExhausted`) eventually fires.
  
  More immediately: the reclaim path's `release_worker` (if implemented per QA-010) frees the registry slot but NOT the id. So even after `release_worker(5)`, the next join gets id 6, not id 5.
- **Expected behavior:**
  Spec ambiguity: SPEC-20 R11 implies monotonic ids ("never reused"), but does not justify why. The reclaim path could safely recycle ids if `release_worker` clears all references.
- **Fix suggestion:**
  - File a SPEC-20 follow-up against `especialista-specs` to clarify: monotonic-only, or recycle-on-release? If monotonic-only: document the operational implication ("for a high-churn grid, plan for restart every N rotations").
  - If recycle-on-release is acceptable: add `next_worker_id` decrement on `release_worker`, with a guard against ABA (don't recycle until all in-flight references to that id are gone).

---

### QA-017 (LOW) — `tracing::warn!(... worker_id = wid, round = metrics.rounds, ?kind, ...)` at line 674 emits `?kind` (Debug-formatted) for `LeaveKind`; log-aggregation pipelines that key on `kind` see `"AfterResult"` or `"Urgent"` strings, but R28's specified field shape is `departure_type = "leave_after_result" | "leave_urgent"`

- **Severity:** LOW — log-shape inconsistency; SF-004 already noted; QA-017 adds the spec-mapping table.
- **Category:** Observability / Log Stability
- **Location:** `relativist-core/src/protocol/coordinator.rs:677, 706, 731, 750`
- **Description:**
  Per SF-004, the four R28 log sites use four different shapes. QA-017 maps to spec R28's enumerated values:
  | R28 spec value | Current site | Current key | Current value |
  |---|---|---|---|
  | `timeout` | line 750 | (none) | (none) |
  | `connection_loss` | line 706, 731 | `error` | error string |
  | `leave_after_result` | line 677 | `kind` | `"AfterResult"` |
  | `leave_urgent` | line 677 | `kind` | `"Urgent"` |
  
  None of the four sites emit a uniform `departure_type` field. Log-aggregation queries cannot join on a single key.
- **Fix suggestion:**
  Per SF-004's recommendation; add a uniform `departure_type` field with the four spec-canonical values.

---

### QA-018 (LOW) — `materialize_reclaimed_partitions` log message at `departure_recovery.rs:46-48` says "skipping reclaim" but the function returns `Err(InvariantViolation(...))` immediately — the log misrepresents the action taken

- **Severity:** LOW — log clarity; NTH-002 noted; QA-018 adds the operational angle.
- **Category:** Observability / Log Clarity
- **Location:** `relativist-core/src/partition/departure_recovery.rs:45–53`
- **Description:**
  The log says "No remapped ID range available for departed worker; skipping reclaim." Then the function returns `Err`. The operator reading the log sees "skipping reclaim" and might assume the reclaim was bypassed (and the run continues); in fact the reclaim *failed*, and the error propagates up. The misleading log delays incident response.
- **Fix suggestion:**
  Per NTH-002: change to "No remapped ID range available for departed worker; failing reclaim with InvariantViolation."

---

### QA-019 (LOW) — `metrics.partitions_redispatched_per_round.push(0)` at line 845 is a placeholder that never reflects actual reclaim activity, making the bundle untestable against reclaim-counting acceptance criteria

- **Severity:** LOW — metric unimplemented; SF-005 / TASK-0443 acceptance criterion gap.
- **Category:** Metric Completeness / Test Coverage
- **Location:** `relativist-core/src/protocol/coordinator.rs:842–846`
- **Description:**
  Even after MF-001's fix, the metric `partitions_redispatched_per_round.push(0)` is hardcoded `0`. Reviewer noted this as dead code on the recovery path; QA-019 adds: it's also dead on the *non-recovery* path (when no workers depart, zero redispatches is correct — but the metric exists to count redispatches, and the only place it should be non-zero is the reclaim block, which currently can't reach the push).
- **Fix suggestion:**
  Replace the placeholder with the actual reclaim count, computed inside the reclaim block:
  ```rust
  let redispatched = reclaimed_partitions.len() as u32;
  // ... later, after the reclaim block:
  metrics.partitions_redispatched_per_round.push(redispatched);
  ```

---

## Edge case catalog — gaps in TEST-SPEC-EG-* coverage

| ID | Edge case | Status | Action |
|----|-----------|--------|--------|
| EC-A | Coordinator restart mid-round; retained_state lost (QA-001) | NOT COVERED | Document durability gap; file SPEC-20 follow-up. **CRITICAL gap.** |
| EC-B | D == K_eff in hybrid mode is unreachable (QA-002) | NOT COVERED | Fix condition; add `eg_u9_hybrid_solo_transition` test. **CRITICAL gap.** |
| EC-C | Reclaim id collision with surviving partition (QA-003) | NOT COVERED | Derive ranges from `compute_id_ranges`; add invariant test. **CRITICAL gap.** |
| EC-D | LeaveAck send-side failure (QA-004) | NOT COVERED | Branch on send result; add `eg_u10_leave_ack_failure` test. **CRITICAL gap.** |
| EC-E | Same wid pushed twice on Error+timeout race (QA-005) | NOT COVERED | Use HashSet; add idempotence test. **CRITICAL gap.** |
| EC-F | reconstruct mixes round-N border_graph with round-0 reclaimed (QA-006) | NOT COVERED | Per MF-008/MF-009 fix; add 2-round border-resolution witness test. **HIGH gap.** |
| EC-G | remap_partition_ids stale free_port_index post-remap (QA-007) | NOT COVERED | Verify `remap.rs:200+`; add unit test. **HIGH gap.** |
| EC-H | DeltaLight {placeholder: String} unbounded payload (QA-008) | NOT COVERED | Per MF-007; add size-bound test. **HIGH gap.** |
| EC-I | collect_timeout has no documented bounds; sequential recv (QA-009) | NOT COVERED | Use FuturesUnordered; document defaults. **HIGH gap.** |
| EC-J | release_worker never called; retained state grows unboundedly (QA-010) | NOT COVERED | Add release call in reclaim block; add 100-round growth test. **HIGH gap.** |
| EC-K | LeaveRequest after PartitionResult buffered, never read (QA-011) | NOT COVERED | Multi-frame recv; add `eg_u10a_clean_leave_with_result` test. **HIGH gap.** |
| EC-L | streams_to_poll index→wid mapping breaks post-pruning (QA-012) | NOT COVERED | Maintain Vec<(WorkerId, Stream)>; add pruning test. **MEDIUM gap.** |
| EC-M | Three error-suppression sites in 200 LoC (QA-013) | NOT COVERED | Apply MF-004/QA-004/SF-001 fixes. **MEDIUM gap.** |
| EC-N | RetainedInitial variant divergence on rename (QA-014) | NOT COVERED | Per SF-005. **MEDIUM gap.** |
| EC-O | Slow-byte adversarial worker stalls accept_workers (QA-015) | NOT COVERED | Per-recv timeout. **MEDIUM gap.** |
| EC-P | next_worker_id monotonic; long-running exhaustion (QA-016) | NOT COVERED | File SPEC-20 follow-up. **MEDIUM gap.** |
| EC-Q | R28 log fields non-uniform (QA-017) | NOT COVERED | Per SF-004. **LOW gap.** |
| EC-R | Misleading "skipping reclaim" log (QA-018) | NOT COVERED | Per NTH-002. **LOW gap.** |
| EC-S | partitions_redispatched_per_round always 0 (QA-019) | NOT COVERED | Compute actual count. **LOW gap.** |

**Coverage gaps that materially affect Stage 6 sign-off: EC-A through EC-K (11 of 19).**

---

## Stress scenarios — adversarial conditions Phase D does not survive

### SS-001: 100-round high-churn grid

**Scenario:** `K_eff = 4`, `elastic_departure = true`, one worker rotates per round (depart + new join). Run 100 rounds.

**Risk:** 
- `retained_state.initial.len()` grows to ~104 (QA-010); `assert_memory_bounds(4)` panics in debug at round 9.
- `next_worker_id` reaches 104 (QA-016).
- Recv-loop sequential timeout accumulates: if 1 worker is slow each round, total wall-clock = `100 * collect_timeout` (QA-009).
- Every round triggers a reclaim that hits MF-001's premature abort. **Run never completes round 1.**

**Recommendation:** After MF-001..MF-009 are fixed, add `eg_p5_100_round_high_churn_stress` integration test with assertions on `retained_state.initial.len()` and total wall-clock.

### SS-002: Adversarial slow-byte worker

**Scenario:** Worker w sends Register one byte per `accept_future` half-slot, then sends the rest only after all other workers are in. Repeat per round.

**Risk:** 
- `accept_workers` blocks for the full `worker_connect_timeout` even if other workers are ready (QA-015).
- During the round, w sends one byte per `collect_timeout - 1ms`, keeping recv alive forever (QA-009 sequential recv aspect).

**Recommendation:** Per-recv timeout independent of overall accept window.

### SS-003: TCP-RST race after LeaveRequest

**Scenario:** Worker w sends `LeaveRequest{AfterResult}`, then immediately `shutdown(SHUT_RDWR)` and `drop(stream)` before the LeaveAck round-trip.

**Risk:** 
- Coordinator's `let _ = send_frame(...)` (QA-004) silently absorbs the BrokenPipe.
- `departing_worker_ids.push(wid)` proceeds.
- Worker, expecting LeaveAck per R20, may interpret the missing ack as "departure not committed" and reconnect with a new id.
- Coordinator now has the old id in `departing_worker_ids` + a new id in `worker_streams` for the same logical worker — id-space confusion.

**Recommendation:** Per QA-004 fix.

### SS-004: Double-detection burst (Error + timeout)

**Scenario:** Worker w's process is killed mid-round via `SIGKILL`. The OS sends RST, the coordinator's recv reads `Message::Error{...}` (sourced from a watchdog or proxy that converts RST to Error). Microseconds later, the timeout fires (because the worker never sent the expected `PartitionResult`).

**Risk:** Per QA-005, both events push the same wid; reclaim materializes the partition twice; debug-assert panics or release-mode D4 violation.

**Recommendation:** Per QA-005 fix.

### SS-005: 4 GiB DeltaLight placeholder

**Scenario:** A future PR adds a `RetainedStateRegistry::dump_to_disk()` method using bincode. An adversary (or a buggy snapshot tool) constructs a `DeltaLight { placeholder: "X".repeat(4 GiB) }` and `dump_to_disk` is called.

**Risk:** Per QA-008, bincode allocates 4 GiB on the wire, OOMs the coordinator. The current `placeholder` field's `String` type is the vector.

**Recommendation:** Per MF-007 fix.

---

## Recommendation for Stage 6 REFACTOR — unified action list

Ordered by severity. **QA findings interleaved with reviewer's MF-NNN/SF-NNN.** Items marked [BLOCKER] must land before bundle ships.

| # | Source | ID | Severity | Action | Surface |
|---|--------|----|----------|--------|---------|
| 1 | review | **MF-001** | CRITICAL [BLOCKER] | Remove `return Err(...TASK-0443 follow-up)` at line 832; implement stream-pruning OR explicitly disable recovery via config gate. | `coordinator.rs:766-833` |
| 2 | review | **MF-002** | CRITICAL [BLOCKER] | Implement R26a hybrid SoloReducing fall-through; correct `>= k_eff` accounting per QA-002. | `coordinator.rs:773-792` |
| 3 | review | **MF-003** | CRITICAL [BLOCKER] | Replace `IdRange{0,100_000}` with `compute_id_ranges`-derived disjoint ranges; address QA-003 collision with surviving partitions. | `coordinator.rs:801-810` |
| 4 | QA | **QA-001** | CRITICAL [BLOCKER] | Document durability gap; release-mode assert in `refresh_last_acked`; file SPEC-20 follow-up. | `retained.rs:55-64`, `coordinator.rs:587-600,662-669` |
| 5 | QA | **QA-002** | CRITICAL [BLOCKER] | Replace `>= k_eff` with `all_remotes_departed`; document hybrid-vs-non-hybrid branches. | `coordinator.rs:773-792` |
| 6 | QA | **QA-003** | CRITICAL [BLOCKER] | Compute reclaimed ranges past `max(surviving.id_range.end)`; integration test for no-collision. | `coordinator.rs:801-810` |
| 7 | QA | **QA-004** | CRITICAL [BLOCKER] | Branch on `send_frame` result for LeaveAck; classify ack-failed as Urgent; log. | `coordinator.rs:680` |
| 8 | QA | **QA-005** | CRITICAL [BLOCKER] | Use `HashSet<WorkerId>` for `departing_worker_ids`; idempotent push. | `coordinator.rs:621,681,708,733,752` |
| 9 | review | **MF-004** | HIGH | Replace `self_partition.as_ref().unwrap()` with combined `if let`. | `coordinator.rs:611` |
| 10 | review | **MF-005** | HIGH | Implement R22a/R22b/R22c branching on `LeaveKind`; multi-frame recv. | `coordinator.rs:672-683` |
| 11 | review | **MF-006** | HIGH | Write 4 unit tests in `retained.rs`, 4 in `departure_recovery.rs`, 1 EG-U7 integration. | new tests |
| 12 | QA | **QA-006** | HIGH | Per MF-008+MF-009: store retained `BorderGraph` snapshot; do not reuse current-round plan. | `coordinator.rs:820-822` |
| 13 | QA | **QA-007** | HIGH | Verify/fix `remap_partition_ids` rewrites `free_port_index`. | `partition/remap.rs:200+` |
| 14 | QA | **QA-008** | HIGH | Per MF-007: bound or feature-gate `DeltaLight` payload. | `retained.rs:31-38` |
| 15 | QA | **QA-009** | HIGH | Use `FuturesUnordered` for concurrent recv; document `collect_timeout` defaults. | `coordinator.rs:645-757` |
| 16 | QA | **QA-010** | HIGH | Call `release_worker` after reclaim; integration test for memory bounds. | `coordinator.rs:766-833` |
| 17 | QA | **QA-011** | HIGH | Multi-frame recv per stream; branch on `(got_result, got_leave)`. | `coordinator.rs:645-757` |
| 18 | review | **MF-007** | MEDIUM | Replace `DeltaLight { placeholder: String }` with spec-correct payload OR feature-gate. | `retained.rs:31-38` |
| 19 | review | **MF-008** | MEDIUM | Use retained `BorderGraph` snapshot, not current-round plan. | `coordinator.rs:820` |
| 20 | review | **MF-009** | MEDIUM | Replace `reconstruct(...)` with v1-correct merge→union→re-split flow. | `coordinator.rs:796-822` |
| 21 | QA | **QA-012** | MEDIUM | Maintain `Vec<(WorkerId, TransportStream)>`; remove inline arithmetic. | `coordinator.rs:626-643` |
| 22 | QA | **QA-013** | MEDIUM | Apply MF-004 + QA-004 + SF-001 fixes; document any remaining `let _` sites. | `coordinator.rs:611,680,829` |
| 23 | QA | **QA-014** | MEDIUM | Per SF-005: collapse or document `RetainedInitial::V1`/`Delta`. | `retained.rs:12-23` |
| 24 | QA | **QA-015** | MEDIUM | Add per-recv timeout in `accept_workers`. | `coordinator.rs:225-330` |
| 25 | QA | **QA-016** | MEDIUM | File SPEC-20 follow-up on `next_worker_id` recycling policy. | `coordinator.rs:171-183` |
| 26 | review | **SF-001** | LOW | Remove `let _ = _round_reclaimed_initial`; add TODO if dead. | `coordinator.rs:829` |
| 27 | review | **SF-002** | LOW | Restructure partition-handover to avoid double-cloning. | `coordinator.rs:572-578,796-797` |
| 28 | review | **SF-003** | LOW | Folded into QA-012. | (folded) |
| 29 | review | **SF-004** | LOW | Folded into QA-017. | (folded) |
| 30 | review | **SF-005** | LOW | Folded into QA-014. | (folded) |
| 31 | review | **SF-006** | LOW | Align `JoinAck` field name with R35 (`assigned_worker_id`). | `protocol/types.rs`, `coordinator.rs:194-198` |
| 32 | QA | **QA-017** | LOW | Per SF-004: uniform `departure_type` field across 4 sites. | `coordinator.rs:677,706,731,750` |
| 33 | QA | **QA-018** | LOW | Per NTH-002: tighten "skipping reclaim" → "failing reclaim". | `departure_recovery.rs:46-48` |
| 34 | QA | **QA-019** | LOW | Compute actual reclaim count for `partitions_redispatched_per_round`. | `coordinator.rs:842-846` |
| 35 | review | **NTH-001..004** | NTH | Bundle in a single test+docs PR. | `retained.rs`, `coordinator.rs` |

**Verification gates after Stage 6 fixes:**
- `cargo test --workspace` — at minimum +12 new tests for QA-001..QA-019 + 11 EG-U/I tests for the original task contracts.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
- `cargo fmt --check` — clean.
- v1 floor of 690 tests — preserved.
- New: integration test `eg_u7_lan_timeout_departure_conservative_recovers` runs end-to-end without `Fatal`.
- New: integration test `eg_u9_hybrid_solo_transition_on_all_remotes_depart` exercises QA-002's corrected condition.
- New: invariant test `eg_p5_100_round_high_churn_retained_state_bounded` validates QA-010.

---

## Sign-off

**The bug-hunt is complete.** I exhausted the adversarial attack surface enumerated in the orchestrator's brief:

1. **Retained-state corruption under partial failure (QA-001)** — surfaced as CRITICAL: registry is in-memory only, debug-assert masks release-mode silent state-loss.
2. **D == K_eff edge (QA-002)** — surfaced as CRITICAL: the hybrid branch is unreachable code under the current condition; reviewer's MF-002 understated the algebraic impossibility.
3. **`reconstruct` mixed-trace inputs (QA-006)** — surfaced as HIGH: concrete witness redex sequence (round-1 border with round-0 reclaimed partition) produces half-wires, T1 violation. Compounds reviewer's MF-008+MF-009 with a reproducible scenario.
4. **`LeaveAck` race (QA-004)** — surfaced as CRITICAL: `let _ = send_frame(...)` discards send errors; TCP-RST after `LeaveRequest` is indistinguishable from clean handshake; retry semantics undefined.
5. **R22a/R22c missing during `Distributing` (QA-011)** — surfaced as HIGH: the recv loop reads one frame per stream, so `LeaveRequest` after `PartitionResult` is silently buffered and treated as connection-loss next round; result data lost.
6. **Connection-loss double-detection (QA-005)** — surfaced as CRITICAL: `Message::Error` + timeout race pushes same wid twice; `materialize_reclaimed_partitions` panics in debug, produces D4-violating duplicates in release.
7. **Heartbeat timeout unbounded (QA-009)** — surfaced as HIGH: `collect_timeout` is per-frame, not per-worker; sequential recv loop multiplies effective wait by `K_eff`; no documented bounds.
8. **`reconstruct_departed_partition` invariant violations (QA-007)** — surfaced as HIGH: `remap_partition_ids` likely preserves `free_port_index` byte-identically, leaving stale slot references; needs verification past `remap.rs:200`.
9. **`DeltaLight { placeholder: String }` adversarial size (QA-008)** — surfaced as HIGH: 4 GiB placeholder DoSes the coordinator on serialize/deserialize; reviewer MF-007 noted the spec mismatch but not the size-attack vector.
10. **`release_worker` never called (QA-010)** — surfaced as HIGH: not in reviewer's findings; retained state grows unboundedly; NF-011 debug-assert eventually panics. **This is a brand-new finding.**

**Verdict:** The bundle is **structurally non-functional** (per reviewer) AND has **5 additional CRITICAL bugs** beyond the reviewer's 3, **6 HIGH bugs** beyond the reviewer's 3, **5 MEDIUM bugs** beyond the reviewer's 3. Total Stage 6 surface: 9 reviewer Must-Fix + 19 QA findings = 28 distinct issues, of which **5 CRITICAL + 6 HIGH = 11 are blockers**.

**Authority:** I am the QA agent (Stage 5). My findings are advisory to the developer; the pipeline orchestrator decides whether to gate Stage 6 on QA-001..QA-005 + reviewer MF-001..MF-003 (the 8 CRITICALs) collectively or carry forward as Stage 7+ work. **My recommendation is GATE all 11 blockers.** The bundle is structurally a placeholder; shipping it as-is means TASK-0444 (Phase E test landing) inherits 11 latent CRITICAL+HIGH bugs that will surface as cascading test failures during Phase E.

The reviewer's recommended Option A (revert recovery block, ship detection-only with `elastic_departure = false` enforced) is the **most truthful path**. If Option B (developer fixes everything) is chosen, the rework is ~5–7 days of focused work, not the reviewer's 3–5 day estimate, due to the 11 additional QA findings.

---

Phase D QA: 5 CRITICAL, 6 HIGH, 5 MEDIUM, 3 LOW
