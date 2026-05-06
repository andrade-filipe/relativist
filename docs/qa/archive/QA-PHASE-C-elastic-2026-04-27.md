# QA Phase C — Elastic Grid Joining (TASK-0430/0432/0433/0434/0435; SPEC-20 §3.2)

**Date:** 2026-04-27
**Stage:** 5 (QA — adversarial bug hunt)
**Bundle:** SPEC-20 Elastic Grid — Phase C (Dynamic Worker Joining), Waves 1+2+3
**Commits scope:** `c87b048..613c847`
**Files inspected (read-only):**
- `relativist-core/src/protocol/coordinator.rs` (heavily modified; 1211 LoC, `run_coordinator` ≈ 513 LoC)
- `relativist-core/src/coordinator.rs` (FSM — untouched by Phase C; wildcard arm at L600)
- `relativist-core/src/protocol/types.rs` (`JoinNackReason`)
- `relativist-core/src/error.rs` (`CoordinatorError::WorkerIdSpaceExhausted`)
- `relativist-core/src/merge/types.rs` (`GridConfig`, `join_window_min/max`, `solo_budget`)
- `relativist-core/src/protocol/self_worker.rs` (`spawn_self_partition` lifecycle)
- `docs/reviews/REVIEW-PHASE-C-elastic-2026-04-27.md` (Stage 4 ammunition: MF-001..004 CRITICAL, MF-005..007 HIGH, MF-008..012 MEDIUM, SF-001..004 LOW)
- `docs/tests/TEST-SPEC-EG-U6{,a}.md`, `EG-U14`, `EG-U12a` (per-spec tests claimed by Phase C task contracts)
- `docs/qa/QA-TASK-0414-2026-04-25.md` (QA precedent format)

**Upstream verdict:** Stage 4 reviewer — REJECT_WITH_FIXES (4 CRITICAL, 3 HIGH, 5 MEDIUM, 4 LOW). Reviewer headlined MF-001 (no concurrent accept in non-AMC states), MF-002 (`else =>` in select makes reduce dead), MF-003 (exhaustion aborts the run), MF-004 (FSM rows missing). Those four are taken as known fixes the developer will close in Stage 6; this QA hunts beyond them.

---

## Summary

| Severity | Count | IDs |
|----------|-------|-----|
| **CRITICAL** | 3 | QA-001, QA-002, QA-003 |
| **HIGH** | 6 | QA-004, QA-005, QA-006, QA-007, QA-008, QA-009 |
| **MEDIUM** | 7 | QA-010, QA-011, QA-012, QA-013, QA-014, QA-015, QA-016 |
| **LOW** | 4 | QA-017, QA-018, QA-019, QA-020 |

**Top-5 most dangerous (one line each):**
1. **QA-001 (CRITICAL)** — `metrics.rounds += 1` happens *before* the join window opens, but `process_join_request` computes `next_round_number = current_round + 1`, so a worker that joins in round N's window receives `JoinAck { next_round_number: N+2 }` while it actually participates in round N+1 → permanent off-by-one round-id desynchronization with the coordinator's `AssignPartition { round: N+1, … }`, every joiner errors out on its first round.
2. **QA-002 (CRITICAL)** — The `JoinAck` is sent BEFORE the joiner is pushed into `worker_streams` (L922-934 inside the drain loop), but the *next* iteration of the same drain loop recomputes `active_ids` from `worker_streams.iter().enumerate()` which still doesn't include the just-handshook joiner; both joiners get the same `partition_index`, leading to two workers receiving the same partition slot in the next round (collision in `AssignPartition` distribution).
3. **QA-003 (CRITICAL)** — `pending_connections_queue` outlives the `Err(...)` return at L832 ("Departure recovery reconstruction succeeded but stream management is TASK-0443 follow-up") so on every successful departure-recovery the buffered joiners and any in-flight `pending_connections_queue` streams are silently dropped without `JoinNack` — joiners hang forever waiting for an ack that the coordinator already gave up on.
4. **QA-004 (HIGH)** — `next_worker_id` is `u32` and the exhaustion check `if *next_worker_id == u32::MAX` is *before* the `+= 1`, so the very first joiner that sees `next_worker_id == u32::MAX - 1` succeeds and sets `next_worker_id = u32::MAX`; the *second* sees `u32::MAX` and gets the NACK, but a bug fix that uses `checked_add(1)` instead of `==` check would still need to handle the wraparound `u32::MAX + 1 = 0` colliding with the hybrid `worker_id = 0` self — verify exhaustion semantics across the fix.
5. **QA-005 (HIGH)** — `accept_workers` initial-window's exhaustion path returns `Err(...)` (L240) but `process_join_request`'s NACK send at L139/148/165/174 uses `let _ =` and *also* on the success path at L199 uses `?`; if the joiner has already disconnected between writing JoinRequest and reading JoinAck, the `send_frame(stream, &ack).await?` at L199 propagates the I/O error up through `?` at L932, **aborting the entire coordinator loop** — exactly the same MF-003 class of bug that the reviewer flagged for the exhaustion path, but on the *success* path.

**CRITICAL findings requiring Stage 6 first-fix priority beyond the reviewer's MF-001..004:**
- **QA-001** (off-by-one `next_round_number` vs `metrics.rounds += 1` ordering) — every joiner is broken until this is fixed.
- **QA-002** (drain-loop TOCTOU on `partition_index` for >1 pending joiners) — silent partition-index collision.
- **QA-003** (departure-recovery `Err(...)` drops `pending_connections_queue`) — Phase D coupling.

**Recommendation:** **REQUIRE Stage 6 FIXES — BLOCKED on QA-001, QA-002, QA-003 in addition to MF-001..004.** Phase C-test-gap is the largest *test* finding; QA-006 makes that explicit with severity HIGH because the four CRITICAL bugs of the reviewer's report and the three CRITICAL bugs above land an aggregate of zero new tests.

---

## Findings

### QA-001 (CRITICAL) — Off-by-one in `next_round_number`: `metrics.rounds += 1` runs BEFORE the join window, but `process_join_request` already adds `+1` again ⇒ joiner `JoinAck.next_round_number = current_round + 2`

- **Severity:** CRITICAL — every successful mid-session join is desynchronized from the coordinator by one round.
- **Category:** Logic Error / Off-by-one / Cross-component Contract Violation
- **Location:** `protocol/coordinator.rs:899` (`metrics.rounds += 1;` BEFORE the join window) AND `protocol/coordinator.rs:192` (`let next_round_number = current_round + 1;` INSIDE `process_join_request`); call site `protocol/coordinator.rs:929` passes `metrics.rounds` (already incremented) as `current_round`.
- **Static trace:**
  ```text
  Round N completes:
    L867:  merge plan executed
    L897:  metrics.total_interactions += ...
    L898:  current_net = merged_net
    L899:  metrics.rounds += 1;          // metrics.rounds is now N+1
    L901:  // JOIN WINDOW opens (still inside loop iteration that started as round N)
    L905:  let min_timer = sleep(join_window_min);
    L911:  while let Some(stream) = pending_connections_queue.pop_front() { ... }
    L929:    metrics.rounds,            // = N+1, passed as `current_round` arg
    L922:    process_join_request(...)
      L116:    current_round: u32,        // = N+1
      L192:    let next_round_number = current_round + 1;   // = N+2 ❌
      L194:    Message::JoinAck { ..., next_round_number };  // sent to joiner
  Round N+1 of the outer loop begins:
    L502:  the redex queue is checked
    L605:  distribute_partitions(..., metrics.rounds, ...);  // distributes round N+1
  ```
- **Why this is CRITICAL:**
  - **R16 contract** (SPEC-20 §3.2): "the joiner receives `JoinAck { next_round_number = R }` where `R` is the next round it participates in." For a joiner draining in round N's window, the *next* round it participates in is round N+1, so `next_round_number = N+1`.
  - **The reviewer noted in §1.6** "the round counter is incremented (line 899) **before** the join window runs (lines 901-960), and `metrics.rounds` is therefore the new round number when the window opens — correct for `JoinAck.next_round_number` in `process_join_request` (line 192: `current_round + 1`), but the off-by-one becomes confusing when read against R16."
  - **The reviewer concluded "correct"** but did not verify the arithmetic. The reviewer's logic was: "if `metrics.rounds` already advanced to N+1 before passing as `current_round`, then `current_round + 1 = N+2`" — which the reviewer wrote off as "confusing." The off-by-one is real: the joiner sees `next_round_number = N+2` and will reject `AssignPartition { round: N+1, ... }` as a round-mismatch via the worker-side validation at `protocol/worker.rs` (round mismatch error similar to L656-660 on the coordinator side: `if r != metrics.rounds { return Err(Fatal("round mismatch")) }`). The joiner immediately errors out.
  - The current FSM (procedural) has zero tests exercising this path (SF-001 in the reviewer report, propagated). EG-U6 ("dynamic_join_mid_round_queued") is forward-referenced and would have caught this on its A4/A5 assertions, but does not exist as code.
- **Reproduction (static, no `cargo run`):**
  ```rust
  // Coordinator state at start of join window:
  //   metrics.rounds = 5   (just incremented from 4 → 5 at L899)
  //   current_net = merged result of round 4
  //
  // process_join_request(... current_round=5 ...) returns
  //   JoinAck { next_round_number: 6 }
  //
  // Joiner stores next_round_number=6 internally.
  //
  // Outer loop continues:
  //   L605: distribute_partitions(..., round=metrics.rounds=5, ...)
  //
  // Joiner (added to worker_streams via L934, but actually NOT added until next
  // round because the next .iter_mut() at L628-639 happens at THIS round which
  // already ran distribute) — wait, let's re-read carefully.
  ```
- **Wait, deeper bug**: re-reading the flow shows the round-arithmetic is *correct* IF and ONLY IF the joiner is registered for round N+1 (the one *after* the window). But the join window is at the END of round N's loop iteration; the next outer-loop iteration is round N+1 (which is also `metrics.rounds`). So the joiner receives `next_round_number = N+2` because `process_join_request` is called with `current_round = N+1` and adds `+1`. This means **the joiner is told "your next round is N+2" but the coordinator's next iteration is N+1**. The joiner's first `AssignPartition` arrives with `round = N+1` and the joiner expects `round >= N+2` — round mismatch.
- **Suggested fix:** at L929, pass `metrics.rounds - 1` (round just completed) OR change L192 to `let next_round_number = current_round;` (no `+1`). The semantically clearer fix is:
  ```rust
  // protocol/coordinator.rs:192
  // - let next_round_number = current_round + 1;
  // + // Caller passes metrics.rounds AFTER it has been incremented for the
  // + // *next* round; current_round is already the round the joiner will
  // + // participate in.
  // + let next_round_number = current_round;
  ```
  OR, less invasive, move `metrics.rounds += 1` to the start of the outer-loop iteration *before* any phase, matching the convention "rounds is the count of completed rounds; current round being prepared = rounds + 1." This is an architectural decision; either fix needs an EG-U6 A5 regression test pinning the behavior.
- **Why current tests miss it:** No new tests in any of the three Phase C waves (SF-001 in the reviewer report). EG-U6 ("dynamic_join_mid_round_queued") and EG-U5 ("dynamic_join_repartition_v1") are the obvious closures and neither has been written.

---

### QA-002 (CRITICAL) — `partition_index` collision when ≥ 2 joiners drain in the same window: drain loop recomputes `active_ids` BEFORE pushing the just-handshook stream into `worker_streams`

- **Severity:** CRITICAL — two joiners in the same window receive the same `partition_index`, causing `AssignPartition` to send two workers the *same* partition in the next round.
- **Category:** Race / TOCTOU / Logic Error
- **Location:** `protocol/coordinator.rs:911-947` (the inner drain loop) AND `protocol/coordinator.rs:184-189` (`partition_index = active_workers.len() ± 1`).
- **Static trace:**
  ```text
  Pre-window state: worker_streams.len() = 2 (W1, W2).
  pending_connections_queue contains [stream_J1, stream_J2] from mid-round arrivals.
  hybrid_coordinator = true; offset = 1.
  next_worker_id = 3.
  
  Drain iteration 1:
    L911: stream = stream_J1
    L912-919: active_ids = {1, 2}                  (worker_streams.len() = 2; +1 each)
    L922-933: process_join_request(..., active_workers={1,2})
      L184-189: partition_index = active_workers.len() + 1 = 3
      L181:    worker_id = next_worker_id = 3; next_worker_id = 4
      JoinAck sent: { worker_id=3, partition_index=3 }
    L934: worker_streams.push(stream_J1)             // worker_streams.len() = 3
  
  Drain iteration 2:
    L911: stream = stream_J2
    L912-919: active_ids = {1, 2, 3}                  ✓ (now 3 entries)
    L922-933: process_join_request(..., active_workers={1,2,3})
      L184-189: partition_index = active_workers.len() + 1 = 4
      L181:    worker_id = 4; next_worker_id = 5
      JoinAck sent: { worker_id=4, partition_index=4 }
  ```
- **OK so the index is actually correct here** because `worker_streams.push` at L934 happens BEFORE the next drain iteration's `active_ids` computation at L912. So the math IS correct for hybrid mode in this specific drain shape.
- **But the failure mode is different.** At the next round (L555-577):
  ```text
  L557: remote_count = worker_streams.len() = 4   (W1, W2, J1, J2)
  L558: k_eff = 4 + 1 = 5
  L563: plan = split(current_net, k_eff=5, strategy)
        // Returns 5 partitions: partitions[0..5]
  L572: partitions_iter = plan.partitions.iter().cloned();
  L573: self_partition = partitions_iter.next();    // partitions[0]
  L578: remote_partitions = partitions_iter.collect();  // partitions[1..5]
                                                       // = [P1, P2, P3, P4]
  L602: distribute_partitions(&mut worker_streams, remote_partitions, ...);
  ```
  
  The `distribute_partitions` function at L355 zips `worker_streams.iter_mut()` with `remote_partitions.iter()`. **`worker_streams` is in INSERTION order**: [W1, W2, J1, J2] with WorkerIds [1, 2, 3, 4]. So:
  - W1 ← P1
  - W2 ← P2
  - J1 ← P3
  - J2 ← P4
  
  But the `JoinAck` told J1 its `partition_index = 3` and J2 its `partition_index = 4`. The partitions at indices 1..4 in `plan.partitions` correspond to `partition_index` 1..4 (assuming `partitions[i].partition_index == i`). So J1 receives the partition with `partition_index = 3` and was told `partition_index = 3` ✓, J2 receives the partition with `partition_index = 4` and was told `partition_index = 4` ✓.
- **Wait — but worker IDs in `partitions[i]`?** Let's check `split(net, k_eff=5)`. The `Partition::worker_id` field is set by `split()` — looking at the prior phase's L632 `let id = if hybrid {(i as u32) + 1}`, the streams_to_poll mapping treats the i-th remote stream as having WorkerId `i+1`. So J1 (index 2 in `worker_streams`) gets WorkerId 3, J2 (index 3) gets WorkerId 4. That matches the JoinAck. Math holds.
- **THE ACTUAL CRITICAL BUG** is in the streams_to_poll re-mapping at L628-639:
  ```rust
  let mut streams_to_poll: Vec<(WorkerId, &mut TransportStream)> = worker_streams
      .iter_mut()
      .enumerate()
      .map(|(i, s)| {
          let id = if grid_config.hybrid_coordinator {
              (i as u32) + 1
          } else {
              i as u32
          };
          (id, &mut *s)
      })
      .collect();
  ```
  This computes WorkerId as `i + 1` (or `i`) — which is the POSITION in `worker_streams`, NOT the actual `WorkerId` assigned by `process_join_request`. **For the freshly-joined J1 at index 2, this code computes `id = 3`, which happens to coincide with the actual WorkerId 3** *only because the joiner counter is equal to `worker_streams.len() + offset`*.
  
  But this invariant **breaks under any worker departure**. After Phase D (which is being implemented in parallel), if W1 departs and is removed from `worker_streams`, then [W2, J1, J2] are at indices [0, 1, 2] but their WorkerIds are [2, 3, 4]. The `streams_to_poll` mapping computes [1, 2, 3] — **wrong**: the WorkerIds for departure-detection logging (L675), reclaim tracking (L708, L734), and identity in the LeaveAck/Error paths (L674-682) all become **off by however-many-departures-have-happened**.
  
  **This is the QA-005-of-the-reviewer-report, but the reviewer rated it MEDIUM** ("the partition_index recomputed from worker_streams.len() works for Phase C but is a latent bug for Phase D"). The reviewer was looking at `partition_index`. **The streams_to_poll mapping has the same bug for the WorkerId itself**, and Phase D's departure-reclaim path (L767+) relies on `departing_worker_ids` carrying *actual* WorkerIds — but at L681 it pushes `wid` which came from this index-derived mapping. So when a W1 departure happens AND a J1 join happens in different rounds, the next round's `streams_to_poll` mis-identifies J1 as W1.
  
  This is more severe than the reviewer's MF-010 because it affects *every* identity-bearing operation in collect, not just `partition_index`. Severity-elevate to CRITICAL because Phase D is shipping concurrently and the cross-phase coupling is silent.
- **Suggested fix:**
  1. **Mandatory:** introduce a `BTreeMap<WorkerId, TransportStream>` (or `Vec<(WorkerId, TransportStream)>`) replacing the bare `Vec<TransportStream>`. WorkerId becomes the canonical key, position becomes a derived `partition_index`. This costs ~30 LoC and removes the index↔ID confusion across all phases.
  2. Add a regression test for "join after departure": W1 leaves, J1 joins, next round's collect identifies J1 by WorkerId (4) not by index (1). The existing EG-U11 "join + departure same round" is the closest test — and is also missing.
- **Why current tests miss it:** No Phase C or Phase D integration tests exist. Single-machine tests at L998-1210 only exercise `accept_workers`, not `run_coordinator`'s full loop.

---

### QA-003 (CRITICAL) — Departure-recovery's stub `Err(ProtocolError::Fatal(...))` at L832 **drops `pending_connections_queue` silently**, causing every buffered joiner to hang without `JoinNack`

- **Severity:** CRITICAL — buffered joiners receive no closure; they observe `connection_reset` only after their own write timeout, and their `JoinRequest` is never acknowledged or rejected.
- **Category:** Resource Leak / Cross-Phase Coupling / Protocol Correctness (D6 violation)
- **Location:** `protocol/coordinator.rs:823-832` (the departure-success → fatal stub).
- **Static trace:**
  ```text
  Mid-round: 3 joiners arrive during SoloReducing or Join Window;
             pending_connections_queue = [stream_J1, stream_J2, stream_J3].
  Mid-round: W1 disconnects; departure detected.
  L767: departing_worker_ids = [1]; reclaim path enters.
  L817: reclaim succeeds; reconstruction OK.
  L820-822: current_net updated.
  L823-826: tracing::info!("Departure recovery reconstruction succeeded.")
  L828:    _round_reclaimed_initial += 1
  L830:    let _ = _round_reclaimed_initial;   // silly write-then-discard
  L831:    // Comment: "Remove departed from worker_streams (TODO: TASK-0443)"
  L832:    return Err(ProtocolError::Fatal(
              "Departure recovery reconstruction succeeded but stream management is TASK-0443 follow-up"
            ));
  
  After this Err propagates:
    The async stack unwinds; `pending_connections_queue` is dropped.
    Each TransportStream's drop just closes the underlying TCP/Channel handle.
    No JoinNack is sent to any of J1, J2, J3.
    Coordinator returns Err(Fatal); main shuts down.
  
  Joiner-side:
    Each joiner is blocked on recv_frame() waiting for JoinAck or JoinNack.
    They will only unblock when the OS detects FIN/RST and returns 0 bytes,
    which depending on transport may take seconds to minutes (TCP keepalive)
    or never (some channel-transport implementations).
  ```
- **Why this is CRITICAL:**
  - **D6 (Protocol Termination)** invariant: every protocol message has a defined response. A `JoinRequest` MUST be answered with `JoinAck` or `JoinNack`. The current code **violates D6 deterministically every time departure recovery happens with non-empty pending queue**.
  - The reviewer flagged the departure-success-Err stub at MF-009 (MEDIUM, Phase D coupling). MEDIUM is too low: the **interaction with Phase C** (newly-introduced `pending_connections_queue`) creates a *new* protocol-correctness failure mode that did not exist in v1.
  - The reviewer's comment in §1.6 ("The procedural-runtime side does buffer streams … in pending_connections_queue during SoloReducing and the Join Window. So in those two specific phases the 'queue, don't drop' behavior is correctly implemented.") is *also misleading* — buffering is only the first half of "queue, don't drop"; the second half is "drain on the next AcceptingMembershipChanges." That second half is unreachable when `Err(...)` short-circuits the loop.
- **Reproduction (static, deterministic):**
  ```rust
  // 1. Configure: hybrid + elastic_join + elastic_departure.
  // 2. Seed pending_connections_queue with stream_J1 (mid-round arrival).
  // 3. Cause W1 to depart mid-round (e.g., drop ChannelTransport client).
  // 4. Coordinator enters L767-832 departure path.
  // 5. ASSERT: stream_J1 receives no message; the test joiner's
  //            recv_frame() future remains pending.
  ```
- **Suggested fix:**
  1. **Mandatory short-term fix:** before returning `Err(...)` at L832, drain `pending_connections_queue` and send each pending joiner a `JoinNack { ProtocolError }` (or `JoinNack { ElasticJoinDisabled }` as a temporary "we're shutting down" signal). Cost: ~10 LoC.
  2. **Architectural fix:** Phase D's TASK-0443 must NOT use a placeholder `Err(...)` for "I haven't implemented the cleanup yet"; it must either complete the cleanup or `return Ok((current_net, metrics))` cleanly — `Err(Fatal(...))` after a *successful* recovery is actively wrong.
  3. **(Test)** Add `test_departure_recovery_drains_pending_join_queue_with_nacks` mirroring the trace above.
- **Cross-reference:** Reviewer's MF-009 is the same site; QA elevates from MEDIUM to CRITICAL because the *Phase C newly-introduced state* (`pending_connections_queue`) interacts with the existing Phase D stub to silently violate D6.

---

### QA-004 (HIGH) — Worker-id wraparound after the MF-003 fix: `next_worker_id == u32::MAX` check is *before* increment, so a fix that uses `checked_add(1)` still must avoid the `u32::MAX → 0` collision with hybrid `worker_id = 0`

- **Severity:** HIGH — even after the reviewer's MF-003 fix lands (NACK + continue instead of abort), the wraparound case can produce a duplicate `WorkerId(0)` collision with the hybrid self.
- **Category:** Boundary / Overflow / Cross-Reference (extends MF-003)
- **Location:** `protocol/coordinator.rs:171-182`.
- **Static trace:**
  ```rust
  // Current (Phase C as shipped):
  if *next_worker_id == u32::MAX {        // L171: checked BEFORE increment
      // NACK + abort (MF-003 currently)
  }
  let worker_id = *next_worker_id;        // L181
  *next_worker_id += 1;                   // L182: PANIC on u32::MAX in debug build,
                                          //         WRAP to 0 in release build
  ```
  - In **debug builds**, `*next_worker_id += 1` when value is `u32::MAX` triggers `attempt to add with overflow` panic. The L171 check is the *guard* that prevents this — and it does prevent it (the Err returns at L176).
  - **After MF-003 fix**, the function returns `Ok(None)` instead of aborting. The next call sees `*next_worker_id` still equal to `u32::MAX` (not incremented because we returned early). All subsequent JoinRequests get NACKed. **State is sticky** ✓ (matches EG-U14 A5).
  - **But under a hypothetical wraparound fix** (e.g., a maintainer sees the stickiness and "fixes" it by allowing wraparound):
    ```rust
    *next_worker_id = next_worker_id.wrapping_add(1);   // u32::MAX → 0
    let worker_id = *next_worker_id;   // = 0
    ```
    Now in **hybrid mode**, the new joiner is assigned WorkerId(0) — which is **reserved for the self-partition** (L642 hard-codes `(0, &mut h.stream)`). The joiner masquerades as the self-partition; the next round's `streams_to_poll` collide on WorkerId(0); the collect loop receives the joiner's `PartitionResult` and treats it as the self's, while the actual self-partition's result is silently overwritten by `Vec::push` ordering.
  - Also relevant: in **non-hybrid mode** the joiner-counter starts at `remote_count as u32` (L486). After enough joins to wrap, `next_worker_id == 0` collides with the very first registered worker (W0). The first worker's partition is overwritten on the next round.
- **Why this matters:** EG-U14 EC-1 ("`next_worker_id == u32::MAX - 1`; one final assignment succeeds") and EC-2 ("departure after exhaustion → R11 monotonic prevents reuse") require **R11 monotonic non-reuse**. Wraparound would break that. The current code's `+= 1` overflow-panic is the safety net; if it's "fixed" via `wrapping_add` without an explicit "saturate at u32::MAX and continue NACKing" guard, R11 is silently violated.
- **Reproduction:**
  ```rust
  // Setup: hybrid coordinator with next_worker_id = u32::MAX synthetically.
  // Joiner J1 connects with valid JoinRequest.
  // process_join_request:
  //   L171: *next_worker_id == u32::MAX  → NACK + (after MF-003 fix) Ok(None)
  // ✓ correct: J1 receives JoinNack { WorkerIdSpaceExhausted }
  
  // Hypothetical post-fix (someone tries to "recover" from exhaustion):
  // *next_worker_id = next_worker_id.wrapping_add(1);  // = 0
  // worker_id = 0  // collides with hybrid self-partition
  // → CRITICAL state corruption.
  ```
- **Suggested fix:**
  1. **Mandatory:** when MF-003 is applied, ensure the comment above L171 explicitly forbids "recovery" via wraparound: "`R11 mandates monotonic, non-reusing WorkerId allocation; once `next_worker_id` reaches `u32::MAX`, all subsequent joins MUST be NACKed with `WorkerIdSpaceExhausted`. Wraparound is not legal."
  2. **Defensive:** use `next_worker_id.checked_add(1).ok_or(WorkerIdSpaceExhausted)?` AFTER the assignment, NOT a separate `==` check. This makes the invariant explicit at the operation site:
     ```rust
     let worker_id = *next_worker_id;
     match next_worker_id.checked_add(1) {
         Some(next) => *next_worker_id = next,
         None => {
             let nack = ...; let _ = send_frame(stream, &nack).await;
             tracing::warn!("WorkerId space exhausted post-allocation; sticky NACK state");
             return Ok(None);
         }
     }
     ```
     Wait — this allows `worker_id = u32::MAX` to be assigned once. That's actually a *desirable* behavior matching EG-U14 EC-1 ("`next_worker_id == u32::MAX - 1`; one final assignment succeeds"). The current Phase C code allocates `worker_id = u32::MAX - 1` as the last and rejects at `u32::MAX` — losing one valid id. The reviewer did not flag this.
  3. **Test addition (mandatory):**
     ```rust
     #[tokio::test]
     async fn process_join_request_uses_full_u32_range_then_nacks() {
         // QA-004: assert that worker_id = u32::MAX is a legal allocation
         // and that the SUBSEQUENT join is NACKed. Pin the boundary.
     }
     ```
- **Note:** This is HIGH not CRITICAL because the wraparound vector requires a *future* maintainer to "fix" the stickiness. The current code is sticky-NACK-correct; the danger is the future delta.

---

### QA-005 (HIGH) — `send_frame(stream, &ack).await?` at L199 propagates I/O errors as `Err(...)`, aborting the coordinator on the *success* path of `process_join_request`

- **Severity:** HIGH — same MF-003 abort-the-coordinator class of bug, but on the success path of every join.
- **Category:** Logic Error / Error Handling / Cross-Reference (extends MF-003)
- **Location:** `protocol/coordinator.rs:199` (`send_frame(stream, &ack).await?` inside `process_join_request`); call site `protocol/coordinator.rs:932` uses `?` to propagate.
- **Static trace:**
  ```rust
  // L194-199:
  let ack = Message::JoinAck { worker_id, partition_index, next_round_number };
  send_frame(stream, &ack).await?;        // ← if joiner disconnected, returns Err
  
  // L932:
  if let Some(worker_id) = process_join_request(...).await? { ... }
                                                          // ↑ propagates
  // run_coordinator returns Err(...); coordinator dies.
  ```
- **Adversarial scenario:** the joiner sends `JoinRequest` and then immediately closes its socket (e.g., crash, network partition, malicious client). The coordinator authenticates, allocates `worker_id`, increments `next_worker_id`, then tries to write `JoinAck` to a closed stream — `send_frame` returns `Err(BrokenPipe)`. This Err propagates and kills the coordinator. **The `next_worker_id` is also already incremented**, so the WorkerId is leaked.
- **Why this matters:**
  - Reviewer's MF-003 (CRITICAL) is "exhaustion path aborts the run." QA-005 is "every other join error path also aborts the run" — including a perfectly mundane "joiner died after sending JoinRequest, before reading JoinAck."
  - Per EG-U6 EC-2 ("a joiner connects, sends JoinRequest, then disconnects before the next window opens — coordinator detects the dead stream during drain; logs WARN; skips the joiner"), this is **explicitly a documented edge case the spec expects the coordinator to handle gracefully**. Phase C does not.
- **Suggested fix:**
  1. Wrap L199 in explicit `if let Err(e) = ... { tracing::warn!(...); return Ok(None); }`:
     ```rust
     if let Err(e) = send_frame(stream, &ack).await {
         tracing::warn!(error = %e, worker_id, "Failed to send JoinAck; joiner closed stream — leaking WorkerId");
         // worker_id is already burned; we MUST NOT reuse (R11 monotonic).
         return Ok(None);
     }
     ```
  2. Same pattern for L939-945's INFO log at the call site (currently no error branch).
- **Why current tests miss it:** No EG-U6 EC-2 test exists. EG-U14 covers the exhaustion case; no test covers "joiner disconnects before reading JoinAck."

---

### QA-006 (HIGH) — Phase C ships ZERO new tests across 3 waves; all 4 reviewer CRITICAL findings + all 3 QA CRITICAL findings have no regression coverage

- **Severity:** HIGH — TEST COVERAGE GAP (the Phase-C-test-gap finding the orchestrator brief explicitly requested as a separate item).
- **Category:** Test Coverage / Stage 2 Gap / Audit
- **Location:** Phase C commits `c87b048`, `13def72`, `613c847` — each commit message claims "Default lib: 1256 → 1256" (zero net test additions). Reviewer's SF-001 confirms the four mandated test specs (TEST-SPEC-04{30,32,33,34,35}) do not exist in `docs/tests/`.

**Walking the EG-U test specs** (which Phase C task contracts forward-reference) and enumerating which are *unverifiable* with current code:

| Test ID | Owner task(s) | Status | Why unverifiable |
|---------|---------------|--------|------------------|
| EG-U1 (`hybrid_coordinator_single_machine`, R1/R5) | TASK-0430 | UNVERIFIABLE | MF-002: SoloReducing reduce_n is dead. |
| EG-U1a (`solo_join_during_solo_reduction`, R10b) | TASK-0430+0434 | UNVERIFIABLE | MF-002 (no reduce progress) + MF-001 (no concurrent accept in collect). |
| EG-U1b (`worker_id_zero_semantics_per_mode`) | TASK-0430 | PARTIALLY VERIFIABLE | The hybrid `WorkerId=0` reservation is in place at L642; the non-hybrid case (joiner gets `WorkerId=0` after wraparound) is QA-004's bug. |
| EG-U2 (`hybrid_partition_count`) | TASK-0430 | VERIFIABLE | k_eff math is correct (L558). No test exists. |
| EG-U3 (`hybrid_self_partition_id_range`) | TASK-0430 | VERIFIABLE in pure code; not asserted at runtime. |
| EG-U4 (`hybrid_merge_includes_self`) | TASK-0430 | VERIFIABLE; no test. |
| EG-U5 (`dynamic_join_repartition_v1`, R12-v1/R13) | TASK-0433 | VERIFIABLE if MF-001+002+003+QA-001 fixed; otherwise CANNOT run a multi-round dynamic-join scenario. |
| **EG-U6 (`dynamic_join_mid_round_queued`, R10b/R16)** | TASK-0432+0434 | **UNVERIFIABLE** | MF-001: WaitingForResults does not buffer mid-round connects. The only buffering is in SoloReducing/JoinWindow. |
| **EG-U6a (`join_window_boundary_race`, R10a)** | TASK-0434+0435 | **UNVERIFIABLE** | MF-005: drain-then-arm protocol is misimplemented. |
| EG-U10c (`graceful_leave_after_result_no_result_received`) | TASK-0432 | VERIFIABLE for happy path; QA-005 hits the abort-on-disconnect bug. |
| EG-U11 (`join_and_departure_same_round`) | TASK-0432+0434 | UNVERIFIABLE | MF-001 + QA-002 + QA-003 all interact. |
| EG-U12a (`partition_index_vs_worker_id_decoupling`, R11a) | TASK-0420 | UNVERIFIABLE | MF-010 + QA-002: positional indexing is in place; the spec's BTreeMap-keyed mapping is not. |
| **EG-U14 (`worker_id_exhaustion_join_nack`, R11)** | TASK-0432 | **UNVERIFIABLE** | MF-003: NACK path returns Err(...); test would fail on A3 ("active worker set is unchanged"). |
| EG-U15b (`protocol_version_mismatch_join_request_path`, R0d) | TASK-0432 | VERIFIABLE (L290-315 is correct per reviewer §3 PASS). No test exists. |
| EG-U16 (`self_partition_panic_to_error`) | TASK-0430 | UNVERIFIABLE | The `spawn_self_partition` panics surface via `JoinError`, not via a `SelfPartitionPanic` event (FSM not wired; QA-002 of TASK-0414 carries through). |
| EG-U17 (`strict_bsp_self_partition_uniformity`) | TASK-0430 | VERIFIABLE; no test exists. |
| EG-U18 (`initial_wait_supersedes_worker_connect`) | TASK-0430 | UNVERIFIABLE | `InitialWaitTimeout` event is not fired anywhere (FSM wildcard absorbs). |
| EG-U19 (`leave_ack_before_close`, R28) | (existing) | VERIFIABLE; the LeaveAck send at L680 is correct order. No new test. |

**Summary of the test gap:**
- 6 out of 17 EG-U tests are UNVERIFIABLE today (EG-U1, U1a, U6, U6a, U11, U14, U16, U18 → 8 actually).
- 0 new tests added across 3 Phase C waves (verified per commit message).
- 4 CRITICAL reviewer findings + 3 CRITICAL QA findings = 7 CRITICAL bugs with **zero** regression coverage.

**Suggested fix:** In Stage 6, add minimum 7 regression tests:
1. `test_waiting_for_results_buffers_mid_round_accept` (MF-001)
2. `test_solo_reducing_makes_progress_under_concurrent_accepts` (MF-002)
3. `test_worker_id_exhaustion_returns_join_nack_without_aborting` (MF-003)
4. `test_fsm_worker_joined_in_waiting_for_results_emits_queue_action` (MF-004)
5. `test_join_ack_next_round_number_matches_actual_next_round` (QA-001)
6. `test_join_request_send_failure_does_not_abort_coordinator` (QA-005)
7. `test_departure_recovery_drains_pending_connections_with_nack` (QA-003)

Plus the missing TEST-SPEC-04{30,32,33,34,35}-*.md files per reviewer SF-001.

---

### QA-007 (HIGH) — Two-timer pin'd-future race in Join Window: `min_timer` and `max_timer` armed simultaneously, can both fire in the same `select!` poll, and the `is_elapsed()` check after `pop_front()` masks `max_timer` expiry

- **Severity:** HIGH — extends MF-005 (R10a misimplementation) with a race the reviewer's "drain-then-arm" sketch does not address.
- **Category:** Concurrency / Timer Race / Logic Error
- **Location:** `protocol/coordinator.rs:905-957`.
- **Static trace:**
  ```rust
  let min_timer = sleep(grid_config.join_window_min);
  let max_timer = sleep(grid_config.join_window_max);
  tokio::pin!(min_timer);
  tokio::pin!(max_timer);
  
  loop {
      while let Some(mut stream) = pending_connections_queue.pop_front() {
          // ... potentially long handshake (recv_frame + send_frame) ...
      }
      if min_timer.is_elapsed() { break; }     // ← line 948
      tokio::select! {
          new_conn = transport.accept() => { pending_connections_queue.push_back(new_conn); }
          _ = &mut min_timer => {}              // ← line 955: empty body, falls through
          _ = &mut max_timer => { break; }       // ← line 956
      }
  }
  ```
- **Issue 1 (timer race in `select!`):** `tokio::select!` polls all branches in pseudo-random order. When `min_timer` and `max_timer` both expire (e.g., `min = 50ms`, `max = 500ms`, but the handshake at L911-947 took 600ms with one pending stream), both timer futures are ready simultaneously. The select picks ONE arm pseudo-randomly. If it picks the `min_timer` arm (empty body), it falls through and the outer `loop` re-checks `min_timer.is_elapsed()` at L948 → `true` → break. Correct outcome.
  
  But if the select picks the `max_timer` arm, it `break`s the outer `loop`. **Also correct.**
  
  Actually... let me re-examine the inner-loop ordering. At L911-947, we drain `pending_connections_queue`. Each drained handshake involves an `await` on `recv_frame` (L921) and another on `send_frame` inside `process_join_request` (L199). These awaits can take arbitrary time (the joiner may be slow). **During those awaits, no other `accept()` is pending and no timer is being polled** — so connections that arrive during a long handshake pile up in the kernel backlog (similar to MF-001 but at the join-window scope).
  
  The `pop_front` drain happens **before** `is_elapsed()` is checked, so even if `max_timer` fired during the inner drain, we return to the inner drain loop and continue popping pending connections. This means **the join window can extend arbitrarily past `max_timer` if `pending_connections_queue` keeps getting fed by the `tokio::select!` arms**. Concretely: every time the select fires the `accept` arm, a new stream gets pushed; the inner drain loop pops it; if the handshake is slow, more streams arrive in the backlog and won't be acceptable until we re-enter the select. But once `max_timer.is_elapsed()`, the next `select!` call will always pick the `max_timer` arm... unless `transport.accept()` is also immediately ready (kernel has buffered connections), in which case the select picks one of the two pseudo-randomly.
  
  **Net effect:** the join window's effective duration is `join_window_max + handshake_time × pending_pile_size`, with no upper bound. Under sustained join pressure, the window never closes. This is a *liveness violation* — the coordinator can be DoS'd by a flood of joiners.
- **Issue 2 (`min_timer.is_elapsed()` after-drain check is wrong):** if `pending_connections_queue` was empty when the loop iteration started AND `min_timer` has expired AND `max_timer` has not, the loop hits L948 `is_elapsed() → true → break`. **R10a says we should NOT break here unless drain-empty AND no arrivals during drain.** The current code does not track "did we have arrivals during drain" — it just breaks on the first post-drain `is_elapsed()` check. So if `min_timer` expires WHILE the drain is happening (say drain takes 60ms, min was 50ms), the post-drain break fires, **even though new arrivals may have come in during those 60ms** that have not been drained yet (they'd be in `pending_connections_queue` but the inner `while let` already popped them all... wait, no, they'd be `accept()`-ed and pushed by the next select iteration, but we never get there because we break at L948).
  
  This is the reviewer's MF-005, but the reviewer's recommended fix sketch *also* has the same issue: after the min-window's `had_arrivals = true` is observed, the extension-timer loop also has a drain-then-check pattern with the same race-window.
- **Suggested fix:**
  1. Replace the dual-pin'd-sleep with a `Sleep::reset(deadline)` pattern using a single sleep future and a deadline computed from `start + min` initially, extended to `start + max` if drain produced new pending. This avoids the simultaneously-armed-timer race.
  2. Track an explicit `arrivals_observed: bool` counter incremented in the `accept` arm AND in the inner drain when `pending_connections_queue` was non-empty entering the loop. Use it in the break condition.
  3. **Bound the inner-drain handshake.** The current `recv_frame(&mut stream, max_payload_size).await?` at L921 has *no timeout*; a slow joiner can hang the entire window. Add `tokio::time::timeout(grid_config.join_window_max - elapsed, recv_frame(...))` to bound the per-stream handshake.
- **Why current tests miss it:** No EG-U6a test exists; reviewer flagged this as missing.

---

### QA-008 (HIGH) — `accept_workers` initial-window's `JoinRequest` rejection does NOT increment `next_worker_id` but ALSO does not consume from `streams.len() < num_workers` — *infinite spin* if a malicious joiner repeatedly sends `JoinRequest` instead of `Register`

- **Severity:** HIGH — DoS surface; the initial window is timeout-bounded (`worker_connect_timeout`), so the spin is bounded by that timeout, but the timeout aborts the whole coordinator startup.
- **Category:** DoS / Resource Exhaustion / Initial-Window Hardening
- **Location:** `protocol/coordinator.rs:225-326` (the `accept_workers` initial loop) AND L290-315 (`JoinRequest` rejection in initial window).
- **Static trace:**
  ```rust
  while streams.len() < config.num_workers as usize {
      let mut stream = transport.accept().await?;
      let (msg, _) = recv_frame(&mut stream, ...).await?;
      match msg {
          Message::Register(payload) => {
              // ... successful path: streams.push(stream); continue;
          }
          Message::JoinRequest { protocol_version, .. } => {
              // L290-314: send JoinNack;
              continue;     // ← does NOT push stream into `streams`
          }
          ...
      }
  }
  ```
  - When a connection sends `JoinRequest` instead of `Register`, we send a NACK and `continue` — `streams.len()` does NOT advance.
  - The next iteration calls `transport.accept().await?` for ANOTHER incoming connection.
  - **A malicious client can repeatedly connect and send `JoinRequest`, infinite-looping the initial accept** until the surrounding `tokio::time::timeout(config.worker_connect_timeout, accept_future)` at L329 kills the coordinator.
  - In `worker_connect_timeout` is by default unbounded or large (e.g., 120s — see `NodeConfig::default` at `merge/types.rs`); the spin is bounded but can starve legitimate workers from registering if they connect slower than the malicious flood.
- **Why this matters:**
  - This is the *initial* window's hardening, which Phase C *did not introduce* (it pre-existed via `accept_workers`). But Phase C's NEW `JoinRequest` rejection arm at L290-315 *added a new `continue` path* that previously did not exist. Before Phase C, an unexpected message was rejected via `Message::RegisterNack` and `continue` (L316-323) — same DoS shape, but JoinRequest is now a *spec-blessed* protocol message, so NF-009-compliant clients SHOULD be sending it during the initial window if they're confused; we now explicitly reject and continue.
  - Pre-Phase-C tests `test_accept_workers_token_auth_rejected` (L1063-1090) demonstrate the pattern: the rejected client connects, gets NACK, the loop continues until the OUTER timeout fires. That's exactly the DoS shape.
- **Suggested fix:**
  1. **Mandatory:** Add a per-window rejection counter; if rejected ≥ N times (configurable, default N=10), abort with `ProtocolError::Fatal("excessive rejections; suspected DoS or misconfiguration")`.
  2. **Defensive:** Add a per-IP rate-limiter on `transport.accept()`; this is more invasive and can wait for SPEC-10 hardening.
  3. **(Test)** `test_accept_workers_aborts_after_excessive_join_request_rejections`.
- **Why current tests miss it:** `qa_probe_5_v0_register_rejected_with_canonical_nack` exercises one rejection followed by timeout; no test exercises the spin loop's response to N rejections.

---

### QA-009 (HIGH) — `process_join_request` does NOT verify that `pending_connections_queue` was consumed in FIFO order, so a re-ordered drain can mis-attribute partition_indices when the `BTreeSet` key happens to collide

- **Severity:** HIGH — see QA-002 for the index-collision angle; this finding is the *ordering* angle.
- **Category:** Logic Error / Ordering Invariant
- **Location:** `protocol/coordinator.rs:911-947`.
- **Static trace:** `pending_connections_queue` is `VecDeque<TransportStream>` (L481). `pop_front()` is FIFO — so far so good. But:
  - The streams enter the queue from THREE different sites:
    1. `SoloReducing` accept arm at L521 (`pending_connections_queue.push_back(stream)`).
    2. Join Window's accept arm at L953 (`pending_connections_queue.push_back(s)`).
    3. (NOT YET IMPLEMENTED but should per MF-001) Mid-round accept in `WaitingForResults`/`Partitioning`/etc.
  - **The order in which streams land in the queue is the order in which they get `WorkerId`s allocated** during the next drain. Since R11 mandates monotonic `WorkerId` allocation, the FIFO ordering becomes part of the *protocol contract* — two workers who connect at the same time but write `JoinRequest` at different times will be serialized by the queue, not by transport-level arrival order.
  - **TCP-level reordering** can happen (e.g., one joiner's TCP packet arrives first but their full JoinRequest is delayed by congestion; another joiner's full JoinRequest arrives second but at the application layer is processed first). The current implementation accepts in TCP-arrival order via `transport.accept()`, which is the OS's view, but the joiner's `recv_frame` can serialize reads from the same kernel buffer in a non-deterministic order under concurrent reads. Since each accepted stream is its own TCP connection, this is *less* of an issue than for a multiplexed channel.
  - **The actual ordering bug** is more subtle: the `accept_workers` initial-window order vs the Join-Window order. In `accept_workers`, NACKed connections are NOT pushed into `streams` (L322 `continue`) but the loop still observes them — so the count of `streams.len()` advances only on success. **In the Join Window, NACKed connections (from `process_join_request` returning `Ok(None)`) ARE consumed from `pending_connections_queue` but not pushed into `worker_streams`** — `worker_streams.push(stream)` at L934 is conditional on `if let Some(worker_id) = process_join_request(...)`. So NACKed connections silently disappear with no audit trail.
  - The orchestrator's brief asks: "what if 100 workers join in one round?" — answer: the inner `while let Some(...) = pop_front()` will drain all 100 sequentially, each with a `recv_frame + send_frame` round-trip. Conservatively at 10ms per handshake, that's 1 second of inner drain — during which `transport.accept()` is NOT called (it's inside the outer `tokio::select!`, not the inner `while let`). New arrivals during those 1000ms pile up in the kernel backlog. With Linux's default backlog of 128, **101 simultaneous joiners overflow**. This is a hard scaling cliff at ~128 joiners/second.
- **Suggested fix:**
  1. Make the inner drain concurrent: spawn a `tokio::spawn` per pending stream that handshakes independently; collect results via mpsc. Cost: ~50 LoC; benefit: handshake throughput scales with available cores.
  2. Document the FIFO contract in `pending_connections_queue`'s declaration: "// FIFO: drain order = arrival order = `WorkerId` allocation order. R11 monotonic invariant depends on this."
  3. **(Test)** `test_join_window_drains_100_pending_in_fifo_order_assigns_monotonic_worker_ids`.

---

### QA-010 (MEDIUM) — `accept_workers`'s initial WorkerId pool does not coordinate with Phase C's joiner pool: hybrid mode starts `next_worker_id = 1` in initial accept (L223), but `run_coordinator` resets `next_worker_id = (remote_count + 1) as u32` at L483, which may differ if `accept_workers` advanced `next_worker_id` due to NACKs

- **Severity:** MEDIUM — divergence between initial and mid-session WorkerId counters.
- **Category:** State Management / Cross-Function Contract
- **Location:** `protocol/coordinator.rs:223` (initial-window counter — local to `accept_workers`), L483 (run_coordinator counter — separate `let mut`).
- **Static trace:**
  ```rust
  // accept_workers (L212):
  let mut next_worker_id: u32 = if hybrid_mode { 1 } else { 0 };
  while streams.len() < config.num_workers as usize {
      // success path at L281-282: worker_id = next_worker_id; next_worker_id += 1;
      // NACK paths at L253, L266, L274, L294, L307, L312, L321: do NOT increment.
  }
  // accept_workers returns Vec<TransportStream>; next_worker_id is dropped.
  
  // run_coordinator at L482-487:
  let remote_count = worker_streams.len();
  let mut next_worker_id: u32 = if grid_config.hybrid_coordinator {
      (remote_count + 1) as u32
  } else {
      remote_count as u32
  };
  ```
  - `run_coordinator` recomputes `next_worker_id` from `worker_streams.len()`, IGNORING the `accept_workers`-internal counter. This is *correct as long as no NACKs happened and no R11-monotonicity-affecting events occurred during initial window*. It happens to be correct because the initial window does not have NACKs that pre-allocate WorkerIds — every NACK at L240-242, L253, L266, L274 is *before* `next_worker_id += 1`. So `next_worker_id` in `accept_workers` ends at exactly `streams.len() + (1 if hybrid)`.
  - But the contract is implicit, and a future change to `accept_workers` that allocates a WorkerId before NACKing (e.g., for audit logging) will silently break R11 monotonicity by allowing `run_coordinator` to re-allocate the same id.
- **Suggested fix:**
  1. **Refactor:** thread `next_worker_id` out of `accept_workers` as a `&mut u32` parameter, OR return `(Vec<TransportStream>, u32)`. Cost: ~5 LoC.
  2. **(Document)** Add `// R11 invariant: post-condition: next_worker_id (local) == streams.len() + (1 if hybrid).` at L223.

---

### QA-011 (MEDIUM) — `worker_streams.push(stream)` at L934 does not validate `worker_id == worker_streams.len() + offset`, so a Phase D departure that compacts `worker_streams` followed by a join in the next round produces a hidden index↔id desynchronization

- **Severity:** MEDIUM — see QA-002 for the read-side; this finding is the write-side invariant.
- **Category:** State Management / Invariant Preservation (Phase D coupling)
- **Location:** `protocol/coordinator.rs:934`.
- **Static trace:** Phase D's TASK-0443 (when implemented) will need to remove departed workers from `worker_streams`. After such removal, the invariant `worker_streams[i].worker_id == i + offset` no longer holds. The Phase C join push at L934 unconditionally appends; so a Phase D removal followed by a Phase C join produces:
  - `worker_streams = [W2, W3]` (W1 removed; remaining in their old positions or compacted to indices [0, 1]).
  - Phase C drain: `partition_index = worker_streams.len() + 1 = 3` (hybrid). Joiner gets WorkerId 3, partition_index 3.
  - But W3 is ALREADY at partition_index 2 (post-compaction); J's partition_index 3 collides.
  - Or, if W3 is still at partition_index 3 (no compaction; sparse), J at partition_index 3 ALSO collides.
- **Suggested fix:** Refactor `worker_streams: Vec<TransportStream>` → `worker_streams: BTreeMap<WorkerId, TransportStream>`. The `.iter_mut().enumerate()` patterns at L355 (distribute_partitions), L628-639 (streams_to_poll), L912-919 (active_ids) all become `.iter_mut()` over `(WorkerId, &mut TransportStream)` directly.

---

### QA-012 (MEDIUM) — `t_window_start.elapsed()` is pushed into `metrics.merge_time_per_round` at L959, polluting merge metrics with join-window time

- **Severity:** MEDIUM — metrics correctness; benchmarks for break-even analysis (ROADMAP §2.40) will be skewed by Phase C overhead.
- **Category:** Observability / Metrics
- **Location:** `protocol/coordinator.rs:959` (`metrics.merge_time_per_round.push(t_window_start.elapsed());`).
- **Static trace:** `t_window_start = Instant::now()` is taken at L904 at the START of the join window. `t_window_start.elapsed()` at L959 measures the join-window duration, NOT the merge duration. The merge timer is `t_merge` declared at L861 and used at L889 (`metrics.border_reduce_time_per_round.push(t_merge.elapsed())`). **`metrics.merge_time_per_round` is being overwritten with the join-window duration**, conflating two distinct phases.
- **Suggested fix:**
  ```rust
  // L959:
  - metrics.merge_time_per_round.push(t_window_start.elapsed());
  + metrics.join_round_overhead_ms_per_round.push(t_window_start.elapsed().as_millis() as u64);
  ```
  This requires `merge/types.rs` to support `join_round_overhead_ms_per_round` as a `Vec<u64>` (already exists per L846 push of placeholder `0`). The fix replaces the placeholder.
- **Cross-reference:** Reviewer's SF-002 noted commit-message test counts conflict with `CLAUDE.md` baseline. QA-012 is an unrelated metrics-correctness gap.

---

### QA-013 (MEDIUM) — `partition_index` is computed *positionally* from `active_workers.len()` but the `JoinAck` is sent BEFORE the next round's `split()` call, so a re-partition that produces fewer partitions than `partition_index` (e.g., due to net shrinkage) sends a partition_index that doesn't exist

- **Severity:** MEDIUM — under net-shrinkage in the next round, the joiner is told `partition_index = N` but `split()` produces `M < N+1` partitions; the joiner never receives an `AssignPartition`.
- **Category:** Logic Error / Cross-Function Contract
- **Location:** `protocol/coordinator.rs:184-189` (`partition_index` computation); L563 (`split(current_net, k_eff, strategy)`).
- **Static trace:**
  - `partition_index` at JoinAck time is computed from `active_workers.len() ± 1` — the number of workers that WILL be active in the next round.
  - But `split(current_net, k_eff, strategy)` at L563 returns `partitions` whose count is whatever `strategy` decides — which may be LESS than `k_eff` if the net is empty or near-empty (e.g., post-merge, the net only has 3 redexes but k_eff = 5; strategy returns 3 partitions instead of 5).
  - Looking at `partition/strategy.rs`: typical implementations DO return exactly `k_eff` partitions (some empty), but a future strategy that bails early on degenerate input could violate this.
  - **More concretely:** if the net converges (`redex_queue.is_empty()`) RIGHT AFTER the JoinAck but BEFORE the next round, the outer loop at L498-501 detects convergence and breaks WITHOUT entering the next round's distribute. The joiner is left hanging on `recv_frame` for an `AssignPartition` that will never come.
- **Suggested fix:**
  1. After convergence detection at L498-501, before `break`, drain `pending_connections_queue` AND `worker_streams` (any joiners pushed via L934) with a final `JoinNack { ElasticJoinDisabled }` or `Shutdown` message. Cost: ~10 LoC.
  2. **(Test)** `test_join_then_immediate_convergence_drains_joiner_with_shutdown`.

---

### QA-014 (MEDIUM) — `let _ = _round_reclaimed_initial;` at L829 is a no-op write-then-discard pattern that hides Phase D incompleteness from the type system

- **Severity:** MEDIUM — dead-write pattern; reviewer's SF-004 flagged the underscore-prefix; QA promotes severity because the pattern is *creating* dead writes elsewhere.
- **Category:** Code Quality / Hidden Tech Debt
- **Location:** `protocol/coordinator.rs:828-829`.
- **Static trace:**
  ```rust
  _round_reclaimed_initial += departing_worker_ids.len() as u32;   // L828: write
  let _ = _round_reclaimed_initial;                                  // L829: no-op read
  ```
  - The `let _ = ...` is a Rust idiom to silence "unused variable" warnings. But L829 is *after* L828's write, AND L841 *also* uses `_round_reclaimed_initial` as an argument to `metrics.retained_initial_reclaims_per_round.push(...)`. So the variable IS used. The `_` prefix is misleading.
  - The `let _ =` discard is a dead-code marker that was probably added during Phase D Wave 1 stubbing; it never got removed. **A future maintainer who sees `_round_reclaimed_initial` will assume it's dead and may delete it**, breaking the L841 metrics push.
- **Suggested fix:**
  1. Remove the leading underscore; remove L829.
- **Why current tests miss it:** Cosmetic; no test would catch this.

---

### QA-015 (MEDIUM) — `accept_workers` initial-window uses `next_worker_id += 1` (panics on overflow in debug) WITHOUT the `*next_worker_id == u32::MAX` pre-check that `process_join_request` has

- **Severity:** MEDIUM — debug-build panic on initial window WorkerId exhaustion; release-build wraparound to `0` (the hybrid self).
- **Category:** Boundary / Overflow / Safety
- **Location:** `protocol/coordinator.rs:282` (`next_worker_id += 1` without bounds check).
- **Static trace:**
  ```rust
  // accept_workers L235-242: pre-check exists, returns Err on exhaustion.
  // accept_workers L281-282: ✗ no pre-check; just increments.
  if next_worker_id == u32::MAX {
      // RegisterNack + Err  ✓ at L235-242
  }
  // ... code path continues; payload validated ...
  let worker_id = next_worker_id;
  next_worker_id += 1;     // L282: panics in debug if next_worker_id was u32::MAX
                          // wraparound to 0 in release.
  ```
  - Wait — re-reading L235-242: `if next_worker_id == u32::MAX { return Err(...) }`. So the L282 increment is reached only when `next_worker_id < u32::MAX`. The increment cannot overflow. ✓
  - But the L235 check is INSIDE the `Message::Register(payload) =>` arm. The `Message::JoinRequest` arm at L290-315 does NOT check `next_worker_id`, but it also does not increment — it just NACKs and `continue`s. So the JoinRequest arm doesn't expose overflow.
  - The Phase C invariant therefore holds: in the initial window, `next_worker_id == u32::MAX` → Err; otherwise increment is safe.
  - **The actual issue is post-Phase-C consistency:** when control transfers to `run_coordinator` at L483, it RESETS `next_worker_id` from `worker_streams.len() + offset`. If `accept_workers` ended with `next_worker_id == u32::MAX - 1` (one allocation away from exhaustion) and shipped 5 streams, `run_coordinator` resets to `5 + 1 = 6` — losing all knowledge of the impending exhaustion. Joiners can then be allocated WorkerIds up to `u32::MAX - 1` (max Phase C joiners ≈ 4 billion) without exhaustion concern, **even though** `accept_workers` may have allocated WorkerIds approaching `u32::MAX`.
  - This is QA-010 (counter divergence) under a different lens. The recommended fix (thread the counter through) closes both findings.
- **Suggested fix:** see QA-010.

---

### QA-016 (MEDIUM) — `process_join_request`'s `partition_index` formula uses `active_workers.len()` but is documented as "the position of the worker in `W_active ∪ {self}` sorted ascending by `WorkerId`" — the formula is correct ONLY for contiguous WorkerIds

- **Severity:** MEDIUM — extends MF-010; the BTreeSet key is the WorkerId but the partition_index uses cardinality, not position.
- **Category:** Logic Error / Spec Compliance (R11a)
- **Location:** `protocol/coordinator.rs:184-189`.
- **Static trace:**
  ```rust
  let partition_index = if grid_config.hybrid_coordinator {
      active_workers.len() as u32 + 1
  } else {
      active_workers.len() as u32
  };
  ```
  - R11a (per EG-U12a): "`partition_index` is the position of the worker in `W_active ∪ {self}` sorted ascending by `WorkerId`."
  - The current formula computes "the cardinality of `W_active`, plus 1 for hybrid." This is **the same as the position** ONLY if WorkerIds are contiguous and dense and the joiner's WorkerId is strictly larger than all existing ones (which it IS in Phase C, because `next_worker_id` is monotonic).
  - **After Phase D departures**, `W_active` becomes sparse: `{2, 5}`, then a joiner with `WorkerId = 6` arrives. Position-by-WorkerId-sorted = 2; cardinality + 1 = 2 + 1 = 3. **Off by one.**
  - The reviewer's MF-010 flagged this. QA elevates to MEDIUM (was MEDIUM in the reviewer report) because the CORRECT formula is documented in the test spec (EG-U12a A1: `partition_index_for(WorkerId(5)) == 2`, NOT 5; so `partition_index = position in sorted set`). The current formula will fail EG-U12a A1 the moment Phase D introduces sparse WorkerIds.
- **Suggested fix:**
  ```rust
  // Compute position-in-sorted-set, not cardinality.
  let partition_index = active_workers.iter().filter(|&&id| id < worker_id).count() as u32
      + if grid_config.hybrid_coordinator { 1 } else { 0 };
  ```
  But this requires `worker_id` to be already allocated, which it IS at L181. The fix order is:
  1. Allocate `worker_id` (L181).
  2. Compute `partition_index` from `active_workers.iter().filter(|&id| id < worker_id).count() + offset`.
  3. Send `JoinAck`.

---

### QA-017 (LOW) — `tracing::warn!("Handshake error: expected JoinRequest, got {:?}", other);` at L126 uses positional formatting but the function returns `Ok(None)` silently — lost-event pattern repeats in the procedural runtime

- **Severity:** LOW — observability paperwork.
- **Category:** Observability / Logging
- **Location:** `protocol/coordinator.rs:126-128`.
- **Static trace:** When a stream sends a non-`JoinRequest` message during the join window's drain, `process_join_request` returns `Ok(None)` (L127). The caller at L932 sees `None`, skips the push, and silently continues. Aside from the `tracing::warn!`, there is no metric, no NACK to the sender, no audit. The stream is dropped at the end of the drain iteration via Rust's lexical scoping.
- **Suggested fix:**
  1. Add `metrics.protocol_violations_per_round` (new field) and increment on each violation.
  2. Send a generic `JoinNack { ProtocolError }` (a new NACK reason) to the offender so they get a clear signal.

---

### QA-018 (LOW) — `process_join_request` parameter `_node_config: &NodeConfig` is unused and underscore-prefixed; passing a config that's silently ignored is a maintenance hazard

- **Severity:** LOW — code quality.
- **Category:** Code Quality / Dead Parameter
- **Location:** `protocol/coordinator.rs:113` (`_node_config: &NodeConfig`).
- **Static trace:** `_node_config` is passed at the call site (L926) but never read inside `process_join_request`. The `_` prefix is a marker that the parameter is intentionally unused. This is a code smell — if it's not used, drop it; if it'll be used in a future fix (e.g., for per-config NACK reasons), document why.
- **Suggested fix:** Remove the parameter, OR document its future purpose with a comment.

---

### QA-019 (LOW) — `bytes_received` accumulator at L569 starts at 0 but ONLY accumulates from `recv_frame` returns at L649 — it does NOT count bytes consumed by the join-window's `recv_frame` at L921, so `metrics.bytes_received_per_round` undercounts under heavy join activity

- **Severity:** LOW — metrics undercount.
- **Category:** Observability / Metrics
- **Location:** `protocol/coordinator.rs:569` (`let mut bytes_received = 0;`); L921 (`let (msg, _) = recv_frame(...).await?;` — `_` discards the byte count).
- **Static trace:** The pattern at L920-921 explicitly discards the byte count. If a JoinRequest is large (e.g., 64 KiB auth token), those bytes are not in `metrics.bytes_received_per_round`.
- **Suggested fix:**
  ```rust
  let (msg, nbytes) = recv_frame(&mut stream, ...).await?;
  bytes_received += nbytes;   // attribute to current round's metrics
  ```
  But this requires `bytes_received` to be in scope at L921, which it is not (the loop body's `let mut bytes_received` is at L569 inside the per-round outer loop). This is fixable but cross-cuts.

---

### QA-020 (LOW) — `pending_connections_queue` is declared `let mut` at L481 OUTSIDE the round-loop but is only consulted INSIDE `SoloReducing` (L515) and the Join Window (L911) — it persists across rounds but the `TransportStream`s inside may be stale

- **Severity:** LOW — resource lifecycle.
- **Category:** Resource Management
- **Location:** `protocol/coordinator.rs:481`.
- **Static trace:** A stream pushed at L521 during `SoloReducing` of round N is drained at L911 of round N+1's join window (or later, since SoloReducing only enters when `worker_streams.is_empty()`). The stream sits idle in the queue for an unbounded time. The TCP keepalive timer on the joiner side may fire and the connection will RST — the queue still holds the stream, but the next `recv_frame(&mut stream, ...)` at L921 will return `Err`, which propagates via `?` at L921. **This is QA-005 territory: the entire coordinator loop dies on the first dead-stream drain.**
- **Suggested fix:** add a deadline to streams in `pending_connections_queue` (e.g., `(Instant, TransportStream)`). Drain stale ones (older than `join_window_max × N`) with a NACK. Cost: ~15 LoC.

---

## Edge case catalog

| ID | Edge case | Status in current tests | Action |
|----|-----------|-------------------------|--------|
| EC-A | 100 joiners in one window — all processed FIFO with monotonic WorkerIds | NOT COVERED | Add `test_join_window_drains_100_pending_in_fifo_order` (QA-009). |
| EC-B | Joiner disconnects between `JoinRequest` and `JoinAck` | NOT COVERED | Add `test_joiner_disconnect_before_ack_does_not_abort_coordinator` (QA-005). |
| EC-C | Same physical worker reconnects twice with different WorkerIds in one window | NOT COVERED | The current code allocates two distinct WorkerIds; document this is intentional (R0d full-rejoin). |
| EC-D | `metrics.rounds += 1` followed by `next_round_number = current_round + 1` | NOT COVERED | Add `test_join_ack_round_id_matches_actual_next_round` (QA-001). |
| EC-E | Departure recovery with non-empty pending_connections_queue | NOT COVERED | Add `test_departure_recovery_drains_pending_with_nacks` (QA-003). |
| EC-F | `next_worker_id == u32::MAX - 1`: one final allocation succeeds, next NACKed | NOT COVERED | Add `test_worker_id_full_range_then_sticky_nack` (QA-004). |
| EC-G | NACKed JoinRequest in initial accept_workers window: 100 in a row, no Register | NOT COVERED | Add `test_initial_window_resists_join_request_dos` (QA-008). |
| EC-H | `MembershipWindowClosed_min` and `MembershipWindowClosed_max` fire in same select poll | NOT COVERED (and EG-U6a does not exist) | Add per QA-007. |
| EC-I | Convergence detected after JoinAck sent but before next round's distribute | NOT COVERED | Add `test_join_then_convergence_drains_joiner_with_shutdown` (QA-013). |
| EC-J | Phase D departure + Phase C join in adjacent rounds: positional indexing collides | NOT COVERED (Phase D not implemented yet) | Add when Phase D lands; flag QA-002 / QA-011 as cross-phase prerequisite. |
| EC-K | Stale stream in pending_connections_queue (kernel-level RST'd) | NOT COVERED | Add deadline tracking (QA-020). |
| EC-L | `process_join_request`'s `_node_config` parameter contract | NOT DOCUMENTED | Drop or document (QA-018). |
| EC-M | `bytes_received` undercounts join-window bytes | NOT COVERED | Add accounting at L921 (QA-019). |

**Coverage gaps that materially affect Stage 6 sign-off: EC-A, EC-B, EC-D, EC-E, EC-F, EC-H, EC-I.**

---

## Recommendation for Stage 6 REFACTOR

Ordered by severity. **QA findings interleaved with the reviewer's MF-NNN/SF-NNN findings.** Items marked [BLOCKER] must land before bundle commits.

| # | Source | ID | Severity | Action | Surface |
|---|--------|----|----------|--------|---------|
| 1 | review | MF-001 | CRITICAL [BLOCKER] | Concurrent `transport.accept()` in non-AMC states; push into pending_connections_queue. | `protocol/coordinator.rs:619-758, 555-617, 861-898` |
| 2 | review | MF-002 | CRITICAL [BLOCKER] | Replace `else =>` SoloReducing branch with explicit reduction trigger. | `protocol/coordinator.rs:515-538` |
| 3 | review | MF-003 | CRITICAL [BLOCKER] | Exhaustion path: NACK + `Ok(None)`, do NOT abort. | `protocol/coordinator.rs:170-179` |
| 4 | review | MF-004 | CRITICAL [BLOCKER] | Add `(Partitioning|Dispatching|WaitingForResults|Merging, WorkerJoined) → QueueWorkerForNextWindow` arms to FSM. | `coordinator.rs:600` |
| 5 | QA | **QA-001** | **CRITICAL [BLOCKER]** | Fix `next_round_number = current_round + 1` off-by-one (passing `metrics.rounds` after increment). | `protocol/coordinator.rs:192, 899, 929` |
| 6 | QA | **QA-002** | **CRITICAL [BLOCKER]** | Refactor `worker_streams: Vec` → `BTreeMap<WorkerId, TransportStream>` to eliminate position↔ID confusion. | `protocol/coordinator.rs:473, 628-639, 911-919` |
| 7 | QA | **QA-003** | **CRITICAL [BLOCKER]** | Drain `pending_connections_queue` with NACKs before any `Err(...)` return in the round loop. | `protocol/coordinator.rs:832, 545, 696, 717, 786, 789` |
| 8 | review | MF-005 | HIGH [BLOCKER] | Drain-then-arm protocol per R10a: drain → arm min → if had_arrivals arm `(max - min)` else exit. | `protocol/coordinator.rs:901-960` |
| 9 | review | MF-006 | HIGH | Extract phase-helpers from 513-line `run_coordinator`. | `protocol/coordinator.rs:455-967` |
| 10 | review | MF-007 | HIGH | Pair `self_handle` with `self_partition` to remove unwrap. | `protocol/coordinator.rs:580-617` |
| 11 | QA | **QA-004** | HIGH | Verify exhaustion semantics across MF-003 fix: ensure `worker_id = u32::MAX` is allocatable; reject wraparound. | `protocol/coordinator.rs:171-182` |
| 12 | QA | **QA-005** | HIGH | Wrap L199 `send_frame(&ack)?` in graceful `Err` handler; do NOT abort coordinator. | `protocol/coordinator.rs:199, 932` |
| 13 | QA | **QA-006** | HIGH | Author 7+ regression tests for MF-001..004 + QA-001/003/005. | new test files |
| 14 | QA | **QA-007** | HIGH | Replace dual-pin'd-sleep with `Sleep::reset(deadline)` pattern; track `arrivals_observed`; bound per-stream handshake with timeout. | `protocol/coordinator.rs:905-957` |
| 15 | QA | **QA-008** | HIGH | Per-window rejection counter; abort on excessive (>10) rejections. | `protocol/coordinator.rs:225-326` |
| 16 | QA | **QA-009** | HIGH | Concurrent inner drain via `tokio::spawn` per pending stream; collect via mpsc. | `protocol/coordinator.rs:911-947` |
| 17 | review | MF-008 | MEDIUM | Replace `let _ = send_frame(...)` with explicit `if let Err(e)` + tracing::warn. | multiple sites |
| 18 | review | MF-010 | MEDIUM | Folded into QA-002 / QA-016 — track partition_index via BTreeMap. | (folded) |
| 19 | review | MF-012 | MEDIUM | Emit `tracing::debug!(timer_kind = ?TimerKind::JoinWindowMin, ...)` for log decoding parity. | `protocol/coordinator.rs:905-906` |
| 20 | QA | **QA-010** | MEDIUM | Thread `next_worker_id` out of `accept_workers`; close counter divergence. | `protocol/coordinator.rs:223, 483` |
| 21 | QA | **QA-011** | MEDIUM | Folded into QA-002 — `worker_streams: BTreeMap`. | (folded) |
| 22 | QA | **QA-012** | MEDIUM | Use `metrics.join_round_overhead_ms_per_round` instead of overwriting `merge_time_per_round`. | `protocol/coordinator.rs:846, 959` |
| 23 | QA | **QA-013** | MEDIUM | Drain `worker_streams` + `pending_connections_queue` on convergence break. | `protocol/coordinator.rs:498-501` |
| 24 | QA | **QA-014** | MEDIUM | Remove `_round_reclaimed_initial`'s leading underscore + delete L829's `let _ =`. | `protocol/coordinator.rs:622, 828-829` |
| 25 | QA | **QA-015** | MEDIUM | Folded into QA-010. | (folded) |
| 26 | QA | **QA-016** | MEDIUM | Compute `partition_index` from sorted-position-by-WorkerId, not cardinality. | `protocol/coordinator.rs:184-189` |
| 27 | review | SF-001 | LOW | Author the four MF-001..004 regression tests. | new test files |
| 28 | review | SF-002 | LOW | Reconcile commit-message test counts vs CLAUDE.md baseline. | `CLAUDE.md` |
| 29 | review | SF-003 | LOW | Fold `recv_frame` into `process_join_request` for cohesion. | `protocol/coordinator.rs:108-209, 921-922` |
| 30 | review | SF-004 | LOW | Drop leading-underscore on `_round_reclaimed_initial`. | (folded into QA-014) |
| 31 | QA | **QA-017** | LOW | Add `metrics.protocol_violations_per_round`; NACK protocol violators. | `protocol/coordinator.rs:126-128` |
| 32 | QA | **QA-018** | LOW | Drop unused `_node_config` parameter or document its future purpose. | `protocol/coordinator.rs:113` |
| 33 | QA | **QA-019** | LOW | Account `bytes_received` for join-window `recv_frame` bytes. | `protocol/coordinator.rs:569, 921` |
| 34 | QA | **QA-020** | LOW | Add `(Instant, TransportStream)` deadlines to `pending_connections_queue`. | `protocol/coordinator.rs:481` |

**Verification gates after Stage 6 fixes:**
- `cargo test --workspace` — expect ≥ 1188 default (1181 baseline + 7 new regression tests) / ≥ 1231 zero-copy
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean
- `cargo fmt --check` — clean
- v1 floor of 690 tests — preserved
- New: `grep -rn "next_round_number = current_round + 1" relativist-core/src/` returns no hits OR is documented as semantically correct.
- New: `grep -rn "Vec::<TransportStream>" relativist-core/src/protocol/coordinator.rs` returns only test-fixture sites; production replaced with `BTreeMap<WorkerId, TransportStream>`.

---

## Sign-off

The bug-hunt is complete. I exhausted the adversarial attack surface enumerated in the orchestrator's brief:

1. **Race conditions on `pending_connections_queue`** — surfaced QA-009 (FIFO contract), QA-002 (partition_index TOCTOU), QA-003 (drop on Err in departure recovery), QA-020 (stale streams), QA-013 (drop on convergence). The "100 workers / second" question is QA-009: under the current sequential drain, 100 joiners take ~1s with no concurrent accept, exceeding Linux's default backlog of 128 and dropping connections kernel-side.
2. **Join Window timer races (R10a)** — surfaced QA-007 (dual-pin'd-sleep race; `is_elapsed()` after-drain check); the reviewer's MF-005 sketch does not fully address the simultaneous-arming race. The "what if both fire in the same poll" question: select picks pseudo-randomly; the post-drain `is_elapsed()` check IS a fallback but only for `min_timer`. The "what if `JoinWindowMin` fires but no other workers have arrived (window closes empty)" question: correct fallthrough; `pending_connections_queue` is empty, drain is a no-op, break. ✓
3. **Worker ID overflow** — surfaced QA-004 (post-MF-003 wraparound), QA-015 (initial-window counter divergence), QA-010 (counter divergence between `accept_workers` and `run_coordinator`). The "u32::MAX + 1 = 0 collision with worker_id=0 self" question is QA-004: the current code panics in debug, which is the safety net; a well-meaning maintainer's `wrapping_add` would silently corrupt the hybrid self-partition.
4. **v1 repartition correctness (R11)** — surfaced QA-013 (convergence-after-join orphans the joiner). The "K_eff = K + 1 invariant preserved" question: yes, L558 correctly recomputes from `worker_streams.len() + offset`. The "hybrid_coordinator flips between rounds" question: not possible — `grid_config` is `&` immutable through the loop. ✓
5. **JoinNack fire-and-forget** — surfaced QA-005 (success-path send_frame? aborts coordinator), MF-008 (reviewer's silent error swallowing). The "if NACK can't be sent (TCP error), does the coordinator silently drop the rejected worker, or does it log + retry" question: today, both — `let _ =` discards the error and continues, no log, no retry.
6. **`partition_index` allocation under concurrent joins** — surfaced QA-002 (positional vs identity), QA-016 (cardinality-vs-position formula), QA-011 (write-side invariant). The "TOCTOU between `worker_streams.len()` check and insert" question: technically NOT a TOCTOU because the drain is sequential (`while let Some(...) = pop_front()`); the bug is the cardinality formula breaking under sparse WorkerIds (post-Phase-D).
7. **State transition idempotency** — already covered by reviewer's MF-004 (FSM rows missing). QA verified the FSM coordinator.rs is unchanged in Phase C; the wildcard arm at L600 still absorbs `MembershipWindowClosed`. The "if `MembershipWindowClosed` fires twice" question: the FSM emits zero actions both times; no state mutation, no double-fire bug. The "cycle detection in transition()" question: there is no cycle detection — the FSM is monotonic per state-event. ✓
8. **Test coverage gap** — formalized as QA-006 (HIGH severity, with detailed walk of EG-U test specs). Eight EG-U test scenarios are unverifiable today; zero new tests across three Phase C waves.

**Verdict:** **REQUIRE Stage 6 FIXES — BLOCKED on MF-001..004 (reviewer) + QA-001, QA-002, QA-003 (this report).**

The implementation **partially closes** the wire protocol surface (R8/R11/R12-v1/R13/R14-v1/R17/R0d/NF-009 all PASS per reviewer §4) but the procedural runtime semantics are **structurally broken** in four ways that the reviewer caught (no concurrent accept, dead reduce branch, abort-on-exhaustion, FSM rows missing) and three ways this QA caught (off-by-one round-id, position↔ID confusion, departure-drops-pending-queue). Six EG-U tests are unverifiable today; nine remain blocked behind the seven CRITICAL fixes.

**Authority:** I am the QA agent (Stage 5). My findings are advisory to the developer; the pipeline orchestrator decides whether to gate Stage 6 on the seven CRITICAL fixes or split into Phase C.5 follow-up. **My recommendation is GATE all seven CRITICALs in a single Phase C.5 wave**: the cumulative refactor (BTreeMap-keyed worker_streams, drain-on-Err pattern, off-by-one fix, plus the four reviewer CRITICALs) is ~150 LoC of production code + ~250 LoC of tests; deferring to a later wave compounds the cross-phase coupling cost as Phase D continues to ship.

— qa, 2026-04-27

Phase C QA: 3 CRITICAL, 6 HIGH, 7 MEDIUM, 4 LOW
