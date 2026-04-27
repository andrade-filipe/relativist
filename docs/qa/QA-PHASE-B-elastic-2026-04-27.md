# QA — Phase B (Foundations) of SPEC-20 Elastic Grid (TASK-0415..0426)

**Date:** 2026-04-27
**Stage:** 5 (QA — adversarial bug hunting)
**QA agent:** qa
**Bundle:** D-006 Phase B Waves 1-4 (commits `4fb77bc`, `ba846ad`, `5c6d8b6`, `21cbfb4`)
**Files inspected (post-commit `21cbfb4`):**
- `relativist-core/src/protocol/coordinator.rs` (run_coordinator + process_join_request + accept_workers)
- `relativist-core/src/protocol/self_worker.rs` (NEW)
- `relativist-core/src/protocol/timers.rs` (NEW)
- `relativist-core/src/protocol/types.rs` (Message + LeaveKind + JoinNackReason + WorkerCapabilities)
- `relativist-core/src/protocol/error.rs` (thiserror migration)
- `relativist-core/src/protocol/channel.rs` (ChannelTransport semantics)
- `relativist-core/src/partition/types.rs` (LeaveKind canonical site)
- `relativist-core/src/partition/helpers.rs` (compute_round_id_ranges + partition_index_of)
- `relativist-core/src/merge/types.rs` (GridConfig + ExecutionMode)
- `relativist-core/src/config.rs` (CLI elastic flags)

**Upstream review verdict:** ACCEPT_WITH_FIXES (4 Must-Fix, 6 Should-Fix in `docs/reviews/REVIEW-PHASE-B-elastic-2026-04-27.md`).

**Adversarial mindset:** Phase B introduces a `tokio::select!` event loop, in-process self-worker spawning, mid-session JoinRequest handling, and elastic-departure plumbing. Every new code path was probed for: panic surfaces, infinite-hang surfaces, off-by-one errors in round/ID arithmetic, FSM event-swallow gaps, wire-format silent breaks, race conditions, and resource exhaustion attack vectors.

---

## Summary

| Severity | Count |
|----------|-------|
| **CRITICAL** | 4 |
| **HIGH** | 5 |
| **MEDIUM** | 4 |
| **LOW** | 3 |

**Recommendation for Stage 6:** **HOLD** — at least the 4 CRITICALs MUST land before any further integration, and the wave-3/wave-4 production paths must NOT be exercised in any benchmark or integration test until QA-001 (SoloReducing infinite hang) and QA-002 (no recv_frame timeout in join window) are fixed. The reviewer's MF-003 (departure premature-abort) will, by design, mask QA-004 (placeholder `IdRange { 0, 100_000 }` collision in reclaim) — the masking goes away the instant Phase D enables departure, so QA-004 must be triaged in tandem with MF-003.

---

## Top-3 most dangerous

1. **QA-001 [CRITICAL] SoloReducing event-loop deadlock — `tokio::select! { ...accept... else => reduce_n(...) }`** at `coordinator.rs:516-536`. The `else` arm fires only when ALL other branches are disabled; `transport.accept()` is always-active so the `else` branch is **unreachable**. The coordinator hangs forever in SoloReducing without ever reducing.
2. **QA-002 [CRITICAL] No timeout on `recv_frame` in join-window pending-connection drain** at `coordinator.rs:921`. A malicious or unresponsive client that connects but never sends `JoinRequest` stalls the entire coordinator indefinitely; `join_window_max` is bypassed for buffered connections.
3. **QA-004 [CRITICAL] Hardcoded placeholder `IdRange { start: 0, end: 100_000 }` in departure recovery** at `coordinator.rs:803-810`. If departure recovery ever fires (Phase B today aborts after this point — see reviewer MF-003 — but Phase D will activate it), the reclaimed partition is materialized with an ID range that **collides with surviving workers' agent IDs**, corrupting the reconstructed net.

---

## Findings

### QA-001 — `tokio::select!` `else =>` arm is unreachable; SoloReducing hangs forever

**Severity:** CRITICAL
**Category:** Logic Error / Infinite hang / `tokio::select!` else-branch trap
**Location:** `relativist-core/src/protocol/coordinator.rs:516-536`

**Reproduction (static trace):**
```rust
while !current_net.redex_queue.is_empty() {
    tokio::select! {
        new_conn = transport.accept() => { ... }      // always-active branch
        else => {                                     // ← UNREACHABLE
            let stats = crate::reduction::reduce_n(&mut current_net, ...);
            ...
            if stats.total_interactions == 0 { break; }
        }
    }
    tokio::task::yield_now().await;
}
```

In `tokio::select!`, the `else =>` arm fires **only when every other branch is structurally disabled** (i.e., guarded with `if false` or pre-completed). Here `transport.accept()` is unguarded — its future is always pollable. Therefore the `else` arm **cannot ever be selected** while `accept()` returns `Pending` (no incoming connection) or `Ready` (a connection arrives — taken, queued, loop iterates). `reduce_n` is never invoked.

**Impact:**
- The redex queue is never drained → `while !current_net.redex_queue.is_empty()` is always true.
- The coordinator hangs forever in SoloReducing whenever it enters the state with `worker_streams.is_empty() && hybrid_coordinator = true`.
- This is precisely the failure mode the reviewer identified (and patched) in Phase C MF-002. Phase B has the SAME defect.
- `pending_connections_queue` grows unboundedly (every accepted connection is buffered, never reduced).
- `solo_budget` is dead config: `reduce_n(&mut current_net, grid_config.solo_budget as usize)` never executes.
- `reduce_solo_batch` (`self_worker.rs:104`) is defined but has **zero callers** (verified by `grep -r reduce_solo_batch`) — strongly suggests this code path was never exercised end-to-end.

**Mitigation:**
Replace the `else =>` arm with an explicit `biased; reduce-with-deadline` pattern, OR alternate accept and reduce explicitly:
```rust
while !current_net.redex_queue.is_empty() {
    // Always do at least one reduce batch per iteration.
    let stats = crate::reduction::reduce_n(&mut current_net, grid_config.solo_budget as usize);
    metrics.total_interactions += stats.total_interactions;
    for (i, &count) in stats.interactions_by_rule.iter().enumerate() {
        metrics.total_interactions_by_rule[i] += count;
    }
    if stats.total_interactions == 0 { break; }

    // Drain newly-arrived connections without blocking.
    loop {
        match futures::future::poll_immediate(transport.accept()).await {
            Some(Ok(stream)) => pending_connections_queue.push_back(stream),
            Some(Err(e)) => { tracing::warn!(?e, "accept error in SoloReducing"); break; }
            None => break,
        }
    }
    tokio::task::yield_now().await;
}
```
Or use `tokio::select! { biased; ... new_conn = transport.accept() => ..., _ = std::future::ready(()) => reduce_step() }`. After the fix, add a unit test that asserts SoloReducing converges on a non-empty net with no incoming connections within `solo_budget * O(1)` polls.

---

### QA-002 — `recv_frame` in join-window has NO timeout; one slow client stalls the coordinator

**Severity:** CRITICAL
**Category:** DoS / Infinite hang / Resource exhaustion
**Location:** `relativist-core/src/protocol/coordinator.rs:921`

**Reproduction (static trace):**
```rust
// inside the elastic_join window loop
while let Some(mut stream) = pending_connections_queue.pop_front() {
    ...
    let (msg, _) = recv_frame(&mut stream, config.max_payload_size).await?;  // ← no timeout
    if let Some(worker_id) = process_join_request(...) { ... }
}
```

Compare with `accept_workers` (line 230) which is wrapped in `tokio::time::timeout(config.worker_connect_timeout, accept_future)` AT THE OUTER LEVEL. Once a connection is accepted in the join-window path, the per-message wait has **no per-stream deadline**.

**Reproduction scenario:**
1. A malicious worker (or a network-stalled one) opens a TCP connection during the join window.
2. The coordinator's outer `tokio::select!` (line 951) accepts it → pushes onto `pending_connections_queue`.
3. The min-timer fires; the inner `while let` drains the queue → calls `recv_frame` on the malicious stream.
4. The malicious worker never sends `JoinRequest` → `recv_frame` blocks indefinitely.
5. **`max_timer` is BYPASSED** because we are stuck inside the inner `while let`, not in the outer `select!`.
6. The coordinator hangs forever, all subsequent rounds blocked.

**Impact:**
- One unresponsive client blocks the entire grid indefinitely. CRITICAL DoS surface for any `--bind 0.0.0.0` deployment.
- `join_window_max` becomes a lie: it bounds only the *between-connections* idle time, not the *per-connection handshake* time.
- Same defect on `accept_workers` is mitigated by the outer 30-second `worker_connect_timeout`; the join-window path has no analogous outer guard.

**Mitigation:**
Wrap `recv_frame` in `tokio::time::timeout(config.handshake_timeout, recv_frame(...))` and on timeout drop the stream and continue:
```rust
let recv_with_deadline = tokio::time::timeout(
    grid_config.join_window_max,  // or a dedicated per-handshake budget
    recv_frame(&mut stream, config.max_payload_size),
);
match recv_with_deadline.await {
    Ok(Ok((msg, _))) => { /* process_join_request */ }
    Ok(Err(e)) => {
        tracing::warn!(error = %e, "join handshake recv error; dropping stream");
        continue;
    }
    Err(_) => {
        tracing::warn!("join handshake timed out; dropping stream");
        // stream drops here, TCP closed
        continue;
    }
}
```
Add `NodeConfig::handshake_timeout: Duration` (default 5s) to keep this independent of `join_window_max`.

---

### QA-003 — `transport.accept()` NOT in select during `Distributing`/`WaitingForResults`/`Merging`; mid-session connections silently dropped

**Severity:** CRITICAL
**Category:** Logic error / Lost connections / Phase C MF-001 regression check
**Location:** `relativist-core/src/protocol/coordinator.rs:602-758`

**Reproduction (static trace):**
Phase C reviewer flagged in MF-001 that during `WaitingForResults`/`Distributing`/`Merging`, mid-session `transport.accept()` must be in the same `select!` as the result-collection branch, otherwise inbound connections during the round are dropped (or worse: deferred until after merge then race-handled). Phase B's `run_coordinator` does NOT poll `transport.accept()` during:
- `distribute_partitions` (line 602): pure write-side, no accept polling.
- `collect_results` (line 645-758): per-stream `recv_frame`-with-timeout loop, no accept polling.
- merge phase (line 861-898): pure-CPU, no accept polling.

A worker that connects between `t_partition` and the start of the join window will sit in the OS TCP backlog. If the OS backlog fills up (small backlogs are common in default tokio TcpListener configs), additional connections are RST. If the backlog is large enough, the connections wait until the next accept call — which is in the join-window's `select!` (line 952) — at which point the connection is accepted. So this is a "deferred accept", not strictly "dropped" — but the deferral can violate the spec's join-window semantics: a client that connected at round-start is racing against `min_timer`.

**Impact:**
- Connection acceptance is **non-uniform across the round**: connections during the merge phase wait until join-window opens; connections during the join-window are accepted promptly. Operator metrics for "connection→join latency" become bimodal.
- If Phase D adds a *during-round* join handler (it should — see SPEC-20 §3.2 R10), the missing accept in the result-collection select! means the join handler is never reached during the round.
- TCP backlog overflow under load → silent client RSTs.

**Mitigation:**
Restructure `collect_results` to use a `tokio::select!` that includes `transport.accept()`:
```rust
loop {
    tokio::select! {
        biased;
        // 1. Drain results from already-connected workers
        result = recv_from_active_workers(&mut streams_to_poll) => { ... }
        // 2. Accept mid-session connections (queue for later JoinRequest processing)
        new_conn = transport.accept() => {
            if let Ok(s) = new_conn { pending_connections_queue.push_back(s); }
        }
        // 3. Phase timeout guard
        _ = tokio::time::sleep(config.collect_timeout) => break with phase_timeout,
    }
    if all_results_received { break; }
}
```
Add a unit test that simulates a connection arriving during `WaitingForResults` and asserts it's queued for the next join window.

---

### QA-004 — Departure recovery uses hardcoded placeholder `IdRange { 0, 100_000 }` that collides with surviving worker IDs

**Severity:** CRITICAL
**Category:** Latent logic bug / Net corruption (silent in Phase B because of MF-003 abort, ACTIVE in Phase D)
**Location:** `relativist-core/src/protocol/coordinator.rs:801-817`

**Reproduction (static trace):**
```rust
let mut reclaimed_id_ranges = std::collections::HashMap::new();
for &id in &departing_worker_ids {
    reclaimed_id_ranges.insert(
        id,
        crate::partition::IdRange {
            start: 0,
            end: 100_000,
        },
    ); // placeholder
}

let reclaimed_partitions = materialize_reclaimed_partitions(
    &departing_worker_ids,
    &retained_state,
    &reclaimed_id_ranges,
).map_err(|e| ProtocolError::Fatal(e.to_string()))?;

// Reconstruct the net
let border_graph = BorderGraph::from_partition_plan(&plan);
let merged_net = reconstruct(&border_graph, surviving_partitions, reclaimed_partitions);
current_net = merged_net;
```

The `IdRange { 0, 100_000 }` is wired into every reclaimed partition. Surviving workers in the same round were assigned ID ranges via `compute_id_ranges` based on the round's `K_eff` — ranges that almost certainly overlap with `[0, 100_000)`.

**Impact:**
- After `materialize_reclaimed_partitions` + `reconstruct`, the net contains agents whose `AgentId` values are duplicated across reclaimed and surviving partitions.
- Every downstream operation that assumes ID disjointness (T1, T2, T7 in SPEC-01) is violated.
- `current_net = merged_net` plants the corruption into the next round's input.
- **Phase B masks this** because line 832 immediately returns `ProtocolError::Fatal` (the reviewer's MF-003), so the corrupted net is discarded before any operation observes it. The instant MF-003 is fixed (Phase D landed), the corruption ships into round N+1.

**Mitigation:**
Compute proper ID ranges via the `compute_round_id_ranges` helper that already exists in `partition/helpers.rs:81`. The function takes `(grid_config, active_workers, base_next_id)`, returns `HashMap<WorkerId, IdRange>` covering exactly the active set with disjoint ranges. Replace the placeholder block:
```rust
// Compute the post-departure active set and assign fresh ID ranges.
let post_departure_active: BTreeSet<WorkerId> = active_workers_pre_round
    .iter()
    .filter(|id| !departing_worker_ids.contains(id))
    .copied()
    .collect();

// Use a `base_next_id` derived from current_net's max live AgentId + 1, to
// avoid collision with the surviving subnets' agent IDs.
let base_next_id = current_net
    .all_live_agent_ids()
    .max()
    .map(|id| id + 1)
    .unwrap_or(0);

let reclaimed_id_ranges = compute_round_id_ranges(
    grid_config,
    &post_departure_active,
    base_next_id,
);
```
Add a unit test that simulates a 3-worker grid where worker 2 departs; assert that `reconstruct(...)` produces a net with no duplicate `AgentId`s.

---

### QA-005 — `next_round_number` in `JoinAck` is off-by-one (advertises N+1 when worker actually joins at round N)

**Severity:** HIGH
**Category:** Off-by-one / Wire-format spec violation
**Location:** `relativist-core/src/protocol/coordinator.rs:191-198` and the call site at `coordinator.rs:929`

**Reproduction (static trace):**
The outer loop increments `metrics.rounds += 1` at line 899, **before** the join window opens at line 901-961. Inside the join window, `process_join_request` is called with `current_round = metrics.rounds` (line 929). Since `metrics.rounds` was already incremented to reflect the just-completed round count, `current_round` already equals `R + 1` where `R` is the just-finished round.

`process_join_request` at line 192:
```rust
let next_round_number = current_round + 1;
```

So `next_round_number = R + 2`. But the next call to the outer `loop` body executes round `R + 1` (because `metrics.rounds` is now `R + 1` and the round number for the upcoming round IS `metrics.rounds`'s current value — see line 605: `distribute_partitions(..., metrics.rounds, ...)`).

Therefore the worker is told **"you'll be in round R+2"** but actually receives `AssignPartition { round: R+1, ... }` in the very next round.

**Impact:**
- The worker's `current_round` tracking diverges from the coordinator's by 1.
- A worker that does `if msg.round != self.expected_round { reject }` will reject the first `AssignPartition` after joining — looks like a protocol violation, triggers `Message::Error`, which the coordinator routes to `handle_connection_loss` → spurious "departure" → cascading recovery (or abort under non-elastic).
- The inline test `qa_probe_9` and `smoke_v2_coordinator_v2_worker_handshake_succeeds` don't exercise the join-during-loop path so this off-by-one is not caught.

**Mitigation:**
Either (a) call `process_join_request` with `current_round = metrics.rounds.saturating_sub(1)`, or (b) move the `metrics.rounds += 1` increment to AFTER the join window closes, or (c) pass `next_round_number = current_round` (since `current_round` already equals the next round to execute due to the prior increment). Add an integration test:
```rust
#[tokio::test]
async fn join_ack_advertises_correct_next_round_number() {
    // Run 3 rounds, join a worker during round 2's join-window, assert
    // first AssignPartition received by joiner has round == JoinAck.next_round_number.
}
```

---

### QA-006 — `streams_to_poll` and `active_ids` index→WorkerId mapping breaks after any departure (latent for Phase D)

**Severity:** HIGH
**Category:** Logic error / Index-based ID arithmetic
**Location:** `relativist-core/src/protocol/coordinator.rs:628-639` and `:912-919`

**Reproduction (static trace):**
Both code blocks reconstruct WorkerIds from `worker_streams` indices:
```rust
// line 628 (collect phase)
let mut streams_to_poll: Vec<(WorkerId, &mut TransportStream)> = worker_streams
    .iter_mut()
    .enumerate()
    .map(|(i, s)| {
        let id = if grid_config.hybrid_coordinator { (i as u32) + 1 } else { i as u32 };
        (id, &mut *s)
    })
    .collect();

// line 912 (join-window active_ids)
let active_ids: BTreeSet<WorkerId> = worker_streams
    .iter()
    .enumerate()
    .map(|(i, _)| {
        let offset = if grid_config.hybrid_coordinator { 1 } else { 0 };
        (i as u32) + offset
    })
    .collect();
```

These both assume `worker_streams[i].worker_id == i + offset`, i.e., **dense, contiguous, monotone** WorkerIds rooted at offset. This holds at startup (assigned via `accept_workers`) and survives appends from joins (line 934 push at the end of `worker_streams`, with new ID = `next_worker_id` which monotonically increments). But it BREAKS the moment any departure removes an entry from `worker_streams`:
- Suppose IDs 1, 2, 3 active. Worker 2 departs → `worker_streams` shrinks to 2 entries. New mapping: index 0 → ID 1 (correct), index 1 → ID 2 (WRONG; the entry there is actually ID 3).
- The `active_ids` set passed to `process_join_request` will contain {1, 2} when actually {1, 3} are alive.
- `process_join_request` uses `active_workers.len() + (1 if hybrid)` as the new partition_index — this is fine because it's a count — but downstream `compute_round_id_ranges` (TASK-0421) relies on the ACTUAL set membership to assign disjoint ID ranges. A wrong active_ids set breaks ID-range disjointness.

**Impact:**
- Phase B masks this because `elastic_departure = false` (default) aborts on any departure, AND `elastic_departure = true` aborts after the recovery block (MF-003). The bug is dormant.
- The instant Phase D properly handles departures by removing entries from `worker_streams`, the index-based reconstruction silently mis-maps WorkerIds, and `compute_round_id_ranges` is fed a wrong active set.

**Mitigation:**
Track WorkerId alongside each stream. Replace `Vec<TransportStream>` with `Vec<(WorkerId, TransportStream)>` or `BTreeMap<WorkerId, TransportStream>`. The latter naturally preserves iteration order and supports O(log n) removal. Index-based arithmetic should be eliminated entirely from the run loop. Add a unit test that simulates: register 3, depart middle one, join 1 new, and assert the resulting `active_ids` and `streams_to_poll` mappings.

---

### QA-007 — `JoinNackReason` and `LeaveKind` discriminants are NOT pinned per-variant; reorder silently breaks wire (Phase A QA-003 regression)

**Severity:** HIGH
**Category:** Wire-format silent break / Discriminant pinning gap
**Location:** `relativist-core/src/protocol/types.rs:240-251` (LeaveKind), `:269-283` (JoinNackReason)

**Reproduction (static trace):**
The Phase A QA-003 finding (HIGH severity) explicitly required per-variant discriminant tests for new enums. Phase B introduces:
- `LeaveKind::AfterResult`, `LeaveKind::Urgent`
- `JoinNackReason::ProtocolVersionMismatch{...}`, `ElasticJoinDisabled`, `WorkerIdSpaceExhausted`, `AuthenticationFailed`

The `test_message_discriminant_stability` test (line 1164-1294) exercises only **one variant of each**: `LeaveKind::AfterResult` (line 1260) and `JoinNackReason::ElasticJoinDisabled` (line 1267). The `test_all_variants_serde_roundtrip` (line 386-464) likewise covers only those two variants.

Therefore a developer who reorders `JoinNackReason` (e.g., adds `RateLimited` at position 0, demoting `ProtocolVersionMismatch` to position 1) will have:
- `cargo test` PASSES (because the only tested variant `ElasticJoinDisabled` rides on a different discriminant byte position inside the bincode payload, but the test only looks at `bytes[0]` of the OUTER `Message` enum, not the inner enum's discriminant).
- A v3 worker that receives `JoinNack { reason: bytes encoding old discriminant 0 (= ProtocolVersionMismatch) }` will decode it as the new variant 0 (= RateLimited).

For `TimerKind`, the discriminants ARE explicitly pinned (`#[repr(u32)]` with `= 0`, `= 1`, ...) and unit-tested at `protocol/timers.rs:46-51`. `LeaveKind` and `JoinNackReason` lack both `#[repr(u8)]` and per-variant `bincode_v2::encode → assert_eq!(bytes[X], expected_disc)` tests.

**Impact:**
- The wire-format invariant for `LeaveKind` and `JoinNackReason` is unenforced.
- An accidental reorder during a future SPEC revision lands a silent v4-to-v4 wire break.
- The MF-001 follow-up (u8→u32 type change for `JoinRequest.protocol_version` / `JoinNackReason::ProtocolVersionMismatch`) WILL touch this struct again — exactly the time when an inadvertent reorder is most likely.

**Mitigation:**
Add explicit discriminant tests:
```rust
#[test]
fn leave_kind_discriminants_are_stable() {
    use crate::protocol::bincode_v2;
    // Wrap in Message::LeaveRequest so the inner enum's discriminant is at byte[1]
    // (byte[0] is Message::LeaveRequest's discriminant = 14).
    let bytes_after = bincode_v2::encode(&Message::LeaveRequest { kind: LeaveKind::AfterResult }).unwrap();
    let bytes_urgent = bincode_v2::encode(&Message::LeaveRequest { kind: LeaveKind::Urgent }).unwrap();
    assert_eq!(bytes_after[1], 0, "LeaveKind::AfterResult MUST encode to discriminant 0");
    assert_eq!(bytes_urgent[1], 1, "LeaveKind::Urgent MUST encode to discriminant 1");
}

#[test]
fn join_nack_reason_discriminants_are_stable() {
    // Same pattern for all 4 JoinNackReason variants.
    // Use bytes[1] (byte[0] is JoinNack's discriminant = 16).
    ...
}
```
Also add `#[repr(u8)]` to both enums so the LLVM-level layout is pinned, defending against future zero-copy/rkyv consumers.

---

### QA-008 — Unbounded `String` payloads on `Message::Error.description`, `RegisterNackPayload.reason`, `ProtocolError::Fatal/AuthFailed` (Phase A QA-002 regression)

**Severity:** HIGH
**Category:** Resource exhaustion / DoS surface
**Location:** `relativist-core/src/protocol/types.rs:78` (Error.description), `:312` (RegisterNackPayload.reason), `relativist-core/src/protocol/error.rs:52, 60` (Fatal, AuthFailed)

**Reproduction (static trace):**
None of the new SPEC-20 wire variants introduced unbounded String fields, but the diagnostic strings on existing variants ride the same wire path. On the **decode side**, `recv_frame` does enforce `max_payload_size` (default 256 MiB, see `NodeConfig::default()`) so a single `Message::Error { description: "A".repeat(255 MiB) }` would be rejected at the frame layer. On the **encode side**, the coordinator's own logic constructs a `Fatal(String)` at multiple call-sites (`coordinator.rs:545, 657, 696, 714, 763, 784, 788, 832`), and the strings are unclamped:
```rust
return Err(ProtocolError::Fatal(format!(
    "Departure of workers {:?} detected; ...",  // ← unclamped
    departing_worker_ids
)));
```
If `departing_worker_ids` is large (say, 100 000 workers — extreme but allocatable since `WorkerId = u32`), the formatted string is multi-MB. `tracing::error!(?err)` will then allocate and emit a multi-MB log line, possibly OOM the coordinator or silently truncate logs.

Phase A QA-002 already flagged this for HIGH severity. Phase B added MORE `Fatal(format!(...))` call sites without clamping.

**Impact:**
- Coordinator OOM under pathological departure scenarios.
- Log shipping pipelines (Loki, journald) drop or fragment giant log lines, losing forensics on the actual failure.
- The HIGH-volume `Message::Error { description }` from a malicious worker is bounded by `max_payload_size` (256 MiB default) but still allocates 256 MiB on the coordinator side — exploitable by a single connection.

**Mitigation:**
1. Introduce `truncate_diag(s: String) -> String` that clamps to e.g. 1024 chars with a `"... [truncated]"` suffix.
2. Apply at every `Fatal(format!(...))`, `RegisterNackPayload { reason }`, `Message::Error { description }` construction site.
3. Reduce `NodeConfig::max_payload_size` default from 256 MiB to 16 MiB unless a benchmark requires the larger value (Wire format has been bounded by SPEC-18 frame budgets).
4. Add a unit test that constructs a `Message::Error { description: "A".repeat(2 MiB) }`, sends it through `recv_frame`, and asserts the truncate-on-decode path fires (today: it doesn't truncate, just allocates).

---

### QA-009 — `Message` enum lacks `#[non_exhaustive]` (mirrors reviewer's MF-004; QA confirmation)

**Severity:** HIGH
**Category:** Code quality / Forward-compat hazard
**Location:** `relativist-core/src/protocol/types.rs:42-43`

This is the same defect the reviewer flagged as MF-004. Independently verified via direct inspection of the enum declaration. The reviewer's mitigation (one-line `#[non_exhaustive]` attribute) is correct and sufficient. Cross-listed here so the developer's Stage 6 fix list is complete.

**Mitigation:** See reviewer's MF-004.

---

### QA-010 — `pending_connections_queue` leaked on coordinator return (resource leak)

**Severity:** MEDIUM
**Category:** Resource leak
**Location:** `relativist-core/src/protocol/coordinator.rs:481, 964-966`

**Reproduction (static trace):**
The `pending_connections_queue: VecDeque<TransportStream>` is created at line 481 but never explicitly drained on the success or error exit paths. `shutdown_workers` (line 964) only signals workers in `worker_streams`, not pending. Streams in the queue go out of scope and Drop closes them — but **without sending Shutdown or LeaveAck**. Clients waiting on those streams see ECONNRESET / unexpected EOF and may interpret the disconnect as a coordinator crash rather than a graceful end.

**Impact:**
- Pending workers see a non-graceful close, may emit confusing logs.
- TCP RST instead of FIN means the kernel may keep ephemeral ports in TIME_WAIT longer.
- Test assertion gap: `qa_probe_9` and similar tests only check `worker_streams` shutdown, not pending.

**Mitigation:**
Before `shutdown_workers`, drain `pending_connections_queue` and send `RegisterNack { reason: "coordinator shutting down" }` or `Shutdown` on each:
```rust
while let Some(mut stream) = pending_connections_queue.pop_front() {
    let _ = send_frame(&mut stream, &Message::Shutdown).await;
}
```

---

### QA-011 — `process_join_request` silently consumes `JoinRequest` and returns `Ok(None)` on every rejection path (caller cannot distinguish "rejected — try later" from "permanent fail")

**Severity:** MEDIUM
**Category:** API design / Error propagation
**Location:** `relativist-core/src/protocol/coordinator.rs:109-209`

**Reproduction (static trace):**
The function returns `Result<Option<WorkerId>, ProtocolError>`. Looking at the four rejection branches:
- Wrong message type (line 125-128): logs warning, returns `Ok(None)`.
- Protocol version mismatch (line 132-141): sends JoinNack, returns `Ok(None)`.
- elastic_join disabled (line 144-150): sends JoinNack, returns `Ok(None)`.
- Auth failed (line 161-167): sends JoinNack, returns `Ok(None)`.

The caller (line 922) treats `Ok(None)` uniformly — does NOT push the stream into `worker_streams`. Good for happy-path. But:
- The caller cannot distinguish "permanent rejection — close the connection" from "soft rejection — could retry". For example, "elastic_join disabled" is a config-level rejection (permanent until coordinator restart); "auth failed" is permanent for THIS auth token but a different token might succeed.
- The stream is **dropped on `Ok(None)`** (because the `if let Some(...) = process_join_request(...)` arm is the only one that pushes to `worker_streams`). The TCP stream's Drop closes it. The client sees the JoinNack, then the close, can't distinguish reasons without parsing the JoinNack payload.

**Impact:**
- Less actionable client-side error handling.
- Caller can't emit different metrics for different rejection types.

**Mitigation:**
Refactor the return type to a structured outcome:
```rust
pub enum JoinOutcome {
    Accepted(WorkerId),
    RejectedSoft(JoinNackReason),    // try later, e.g., backpressure
    RejectedHard(JoinNackReason),    // never retry, e.g., elastic_join disabled
}
pub async fn process_join_request(...) -> Result<JoinOutcome, ProtocolError> { ... }
```
Caller branches on `JoinOutcome` for metrics + logging.

---

### QA-012 — `WorkerCapabilities` is empty struct; serde-roundtrips as zero bytes; future addition is a wire break

**Severity:** MEDIUM
**Category:** Wire-format forward-compat / Spec violation
**Location:** `relativist-core/src/protocol/types.rs:259-261`

**Reproduction (static trace):**
```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerCapabilities {
    // Reserved for SPEC-31 (intra-worker parallelism, rayon).
}
```

Empty struct. bincode v2 encodes this as zero bytes. When SPEC-31 lands and adds e.g. `pub max_threads: u32`, the field will be added at offset 0 of WorkerCapabilities's payload, but every existing v4-encoded `JoinRequest` lacks those bytes — UnexpectedEnd on decode. `WorkerCapabilities` is NOT `#[non_exhaustive]` (which would only help external consumers anyway).

**Impact:**
- The "reserved for future use" intent is contradicted by the wire format: any addition is a hard break, the same as if the field were absent.
- A v4 client that learns `max_threads` would have to bump to v5.

**Mitigation:**
Either (a) add a placeholder field NOW (e.g., `pub _reserved: u32`) so future fields can be added compatibly via `Vec<u8>` or `Option<...>`; or (b) document this as a hard wire break for SPEC-31 and bump PROTOCOL_VERSION at that time. Option (a) is preferable.

---

### QA-013 — `Message::LeaveAck` wire size: variant has zero data but bincode v2 always emits the full discriminant byte (no size optimization for unit variants)

**Severity:** MEDIUM
**Category:** Code-quality observation
**Location:** `relativist-core/src/protocol/types.rs:228-229`

`LeaveAck` is a unit variant. After reading the discriminant byte (15), the decoder expects no further bytes — correct, no risk. However, the spec text on `LeaveAck` says "The coordinator MUST send this before closing the TCP stream." The implementation at line 680 sends LeaveAck with `let _ = send_frame(...)` — **dropping any error**. If the LeaveAck send fails (broken pipe), the close still happens; the client never receives ack. The "MUST" is violated under partial connection failure.

**Impact:**
- Spec MUST violation under TCP failure.
- A racing client that disconnects before the coordinator's LeaveAck send will see no ack and may log a phantom error.

**Mitigation:** Replace `let _ = send_frame(...)` with `if let Err(e) = send_frame(...)` and at least emit `tracing::warn!`. Document that LeaveAck is best-effort under broken pipe. Optionally retry once.

---

### QA-014 — `accept_workers` loop has no cap on `JoinRequest`-mistakenly-sent retries; a flapping client can spin the registration loop forever within `worker_connect_timeout`

**Severity:** LOW
**Category:** DoS attenuation
**Location:** `relativist-core/src/protocol/coordinator.rs:290-314`

When a v3 client (or buggy client) sends `JoinRequest` to `accept_workers`'s initial registration window, the code at line 290-314 sends a `JoinNack` and `continue`s — the loop iteration count is uncapped. A misbehaving client that keeps reconnecting and sending JoinRequest will exhaust the coordinator's CPU on the registration loop within the `worker_connect_timeout` budget (default 30s).

**Impact:** Mild — outer timeout caps the impact; no correctness violation. But under load, a buggy client can crowd out legitimate ones.

**Mitigation:** Add a per-source-IP retry counter (track in a `HashMap<IpAddr, u32>`, reject after 3 attempts). Out of scope for Phase B; document as Phase D follow-up.

---

### QA-015 — `is_coordinator_self` flag is never validated; a malicious worker can claim `is_coordinator_self = true` in its `WorkerRoundStats` payload

**Severity:** LOW
**Category:** Trust boundary / Metric integrity
**Location:** `relativist-core/src/merge/types.rs:198-200`, decoded at `protocol/coordinator.rs:651-655`

**Reproduction (static trace):**
`WorkerRoundStats.is_coordinator_self` is part of the wire payload (it's serialized inside `Message::PartitionResult`). The coordinator at line 651-655 trusts the worker's own claim about whether it's the coordinator. A misconfigured or malicious worker could set this to `true`, and the coordinator's accumulator code would treat it as a self-partition contribution — corrupting metrics that filter by `is_coordinator_self == true`.

**Impact:**
- Metrics integrity issue, not correctness.
- No code path currently special-cases `is_coordinator_self` for control-flow purposes (verified by grep), so the trust violation is currently opt-in metric pollution.

**Mitigation:** On the coordinator side, after `recv_frame` decodes a `PartitionResult` from a remote worker, override `stats.is_coordinator_self = false` unconditionally before accumulating. The self-worker's stats (line 75 in `self_worker.rs`) are constructed coordinator-side, so they are trustworthy by construction.

---

### QA-016 — `Message::Error` from a worker triggers `handle_connection_loss` regardless of whether the error is recoverable

**Severity:** LOW
**Category:** Error categorization
**Location:** `relativist-core/src/protocol/coordinator.rs:684-711`

```rust
Message::Error { worker_id, description, .. } => {
    let outcome = handle_connection_loss(worker_id, &description, grid_config.elastic_departure);
    ...
}
```

Treating any worker-reported `Error` as a connection-loss event is overbroad. SPEC-06 says `Error` is "irrecoverable in the worker". The coordinator's reaction is to either abort (non-elastic) or trigger recovery (elastic) — but a worker that reports a transient error (e.g., "OOM, retry") is treated as permanently departed. There's no recovery hook for transient errors.

**Impact:** Lost flexibility for future transient-error semantics; not a correctness break given the SPEC-06 contract today.

**Mitigation:** Out of scope for Phase B; document for Phase D when departure semantics are formalized.

---

## Stress scenarios (informational; out-of-scope for Stage 6 fix list)

### SS-001 — TASK-0423 self-worker spawn under panic injection

**Scenario:** Spawn 100 self-partition tasks back-to-back, randomly inject panics in the spawned task body.
**Risk:** `panic_rx.try_recv()` may not propagate the panic to the coordinator if the panic happens in the runtime, not in user code.
**Recommendation:** The reviewer's SF-004 noted no tests exist for `self_worker.rs`. Add panic-injection tests as part of the Stage 6 follow-up.

### SS-002 — Concurrent `transport.accept()` and join-window timer race

**Scenario:** A connection arrives at the exact instant `min_timer` fires.
**Risk:** Either branch of the `tokio::select!` at line 951 may win — both are valid, but the code path differs (the connection is queued and processed in next iteration's `while let`, but `min_timer.is_elapsed()` at line 948 may return true depending on which select arm fired).
**Recommendation:** Acceptable nondeterminism per spec, but worth documenting in `process_join_request` rustdoc.

### SS-003 — `WorkerId` exhaustion (u32::MAX joins)

**Scenario:** A coordinator runs for years; cumulative joins exceed u32::MAX.
**Risk:** `next_worker_id == u32::MAX` returns `WorkerIdSpaceExhausted` correctly. Verified at line 171 (`process_join_request`) and line 235 (`accept_workers`). Both paths are covered.
**Recommendation:** No action needed; coverage is adequate.

---

## Recommendation for Stage 6

**Stage 6 must NOT proceed with integration testing or benchmarking until at least the 4 CRITICALs are closed.** Sequenced fix order:

1. **QA-001** (SoloReducing hang) — replace `else =>` with explicit reduce-then-poll-accept structure. **MUST be unit-tested** with a non-empty net + zero pending connections, asserting the loop converges within `solo_budget × O(K)` iterations.
2. **QA-002** (no recv_frame timeout) — wrap the join-window `recv_frame` in `tokio::time::timeout` with a dedicated `handshake_timeout` config field.
3. **QA-003** (accept not in collect_results select) — add `transport.accept()` to the collect-phase select; queue mid-round connections.
4. **QA-004** (placeholder IdRange) — replace with `compute_round_id_ranges` call. Add a unit test asserting reconstructed-net AgentId disjointness.
5. Reviewer's MF-001..MF-004 should land in the same Stage 6 patch.
6. **QA-005** (off-by-one in next_round_number) — fix the increment ordering; add an integration test asserting the joiner's first AssignPartition matches JoinAck.next_round_number.
7. **QA-006** (index-based WorkerId mapping) — refactor `worker_streams: Vec<TransportStream>` → `BTreeMap<WorkerId, TransportStream>`. Out of scope if Phase D restructures; add a TODO + assert that today no entry is removed mid-run.
8. **QA-007** (LeaveKind / JoinNackReason discriminant pinning) — add per-variant byte-level tests.
9. **QA-008** (unbounded String diag) — introduce `truncate_diag` helper, apply to all Fatal/RegisterNack/Error sites.
10. **QA-009** (`#[non_exhaustive]` on Message) — same as reviewer MF-004; one-line attribute.
11. **QA-010..QA-016** are MEDIUM/LOW; Stage 6 may defer with a tracking issue but each must have a docs/backlog entry.

After all CRITICAL+HIGH fixes land, **re-run the full Phase B test suite** and add the explicit invariant checks called out above. Phase C dispatch should NOT proceed until QA-001 and QA-002 are confirmed closed by integration tests, because Phase C will exercise both code paths under elastic membership churn.

---

Phase B QA: 4 CRITICAL, 5 HIGH, 4 MEDIUM, 3 LOW
