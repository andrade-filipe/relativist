# Review — Phase C (Joining): TASK-0430 / 0432 / 0433 / 0434 / 0435 (SPEC-20 §3.2)

**Date:** 2026-04-27
**Stage:** 4 (REVIEW) — unified code-quality + architecture review
**Reviewer:** reviewer agent
**Bundle:** SPEC-20 Elastic Grid — Phase C, Dynamic Worker Joining (3 waves)

**Commits reviewed:**
- `c87b048` — Phase C Wave 1: TASK-0430 (Hybrid dispatch orchestrator), TASK-0432 (JoinRequest handshake handler)
- `13def72` — Phase C Wave 2: TASK-0433 (v1 repartition on join), TASK-0434 (pending_connections_queue R10b)
- `613c847` — Phase C Wave 3: TASK-0435 (Join Window drain + Min/Max timers)

**Files reviewed:**
- `relativist-core/src/protocol/coordinator.rs` (modified across all three waves; reaches 1211 LoC, with `run_coordinator` itself at 513 LoC)
- `docs/backlog/TASK-04{30,32,33,34,35}-*.md` (task contracts)
- Cross-refs: `specs/SPEC-20-elastic-grid.md` §3.1, §3.2, §3.6, §4.1.1–§4.1.4; `specs/SPEC-13-system-architecture.md` R28; `relativist-core/src/coordinator.rs` (FSM, untouched by Phase C)

**Code-quality verdict:** **NEEDS REFACTORING**
**Architecture verdict:** **NEEDS RESTRUCTURING**
**Spec compliance:** SPEC-20 R8/R9/R10/R10a/R10b/R11/R12-v1/R13 — partial; multiple normative violations
**Overall verdict:** **REJECT_WITH_FIXES** — 4 CRITICAL, 3 HIGH, 5 MEDIUM, 4 LOW

---

## 1. Summary

Phase C adds the production code-path for hybrid dispatch (TASK-0430), the `JoinRequest`/`JoinAck`/`JoinNack` handshake (TASK-0432), v1 re-partition on join (TASK-0433), the `pending_connections_queue` field (TASK-0434), and a Join Window with `JoinWindowMin` / `JoinWindowMax` timers (TASK-0435). All five tasks land **entirely inside the procedural async `run_coordinator` function** in `protocol/coordinator.rs`. The pure-FSM in `coordinator.rs` (TASK-0414) is **not touched** — the wildcard arm of `transition()` continues to silently absorb every `WorkerJoined`, `WorkerLeft`, `MembershipWindowClosed`, `InitialWaitTimeout`, etc., with the explanatory NOTE pointing to TASK-0436 still in place.

This split between "procedural live runtime" and "formal FSM" is intentional per the bundle DAG (TASK-0436 is the FSM-wiring task and is **not** in Phase C), but the consequence is that the procedural runtime has accumulated several independent correctness bugs that the FSM contract would have caught:

1. **R10b is silently violated for `WaitingForResults`** (CRITICAL — exactly the QA-001 Phase A pattern). The collect loop polls worker streams sequentially with no concurrent `transport.accept()`; mid-round TCP arrivals sit in the OS backlog and may be dropped under load. Connections are buffered into `pending_connections_queue` ONLY during `SoloReducing` (line 521) and during the Join Window (line 953) — never during the main dispatch/collect path. This is the same class of bug QA-001 flagged in Phase A.
2. **The `SoloReducing` `tokio::select!` block uses an `else =>` branch for the reduction work** (line 528), which fires only when **all** other branches are disabled. Since `transport.accept()` has no guard, it is always enabled, and `else` will **never** be entered — the SoloReducing loop will accept incoming connections forever and never make any reduction progress. This is dead-on-arrival; SoloReducing cannot converge.
3. **`process_join_request` returns `Err(...)` on `WorkerIdSpaceExhausted`** (line 176), which propagates through `?` and aborts the entire `run_coordinator`. The spec (R11, SC-023, EG-U14) requires the offender to receive `JoinNack { WorkerIdSpaceExhausted }` while the coordinator continues serving other workers. Behavior diverges from acceptance criterion of TASK-0432.
4. **R10a's drain-then-arm protocol is misimplemented.** Lines 905–906 arm `JoinWindowMin` and `JoinWindowMax` simultaneously, then drain in a loop. The spec requires: drain first, arm `min`, on `min`-expiry **if** drain produced new pending arm `(max - min)` and continue, **else** transition. The current behavior is roughly "drain, race min vs accept vs max, exit on min unless max already fired" — closes the window prematurely whenever a connection arrives mid-min; arrivals after `min`-expiry but during a still-in-flight handshake on the previous batch can be lost.
5. **`run_coordinator` is now a 513-line god-function** with deep concerns (accept, solo-reduce, partition, dispatch, collect, departure-reclaim, merge, post-merge reduce, join-window) all interleaved. SRP is heavily violated. Concrete refactoring to phase-level helper functions is recommended before TASK-0436 (FSM wiring) lands, otherwise the FSM transitions cannot be cleanly hooked.

A further critical concern is **TASK-0434's stated FSM acceptance criterion was not met**: the task explicitly required four FSM transition rows (`Partitioning|Dispatching|WaitingForResults|Merging × WorkerJoined → same state, action QueueWorkerForNextWindow(id)`). None of these rows were added to `transition()`. The wildcard arm silently absorbs `WorkerJoined` in those states. This is the literal pattern QA-001 already filed once.

The procedural-runtime side does buffer streams (the raw TCP streams) into `pending_connections_queue` during `SoloReducing` and the Join Window. So in those two specific phases the "queue, don't drop" behavior is correctly implemented. The CRITICAL gap is the dispatch/collect/merge phase (where the runtime is busy with results and never accepts).

The handshake protocol (R8 / R11 / R0d) reuses wire variants from TASK-0418 (`Message::JoinRequest` / `JoinAck` / `JoinNack` with `JoinNackReason` enum). No duplicate type definitions were introduced. JoinNack reason coverage is correct (ProtocolVersionMismatch, ElasticJoinDisabled, AuthenticationFailed, WorkerIdSpaceExhausted). The version-mismatch path on `accept_workers`'s `JoinRequest` arm (line 290–315) is a clean R0d implementation including NF-009 shape.

The v1 repartition path (R12-v1 / R13) is implemented procedurally by simply continuing to the next `loop {}` iteration after the join window, where `k_eff` is recomputed and `split()` is called over the merged net. This is correct in principle. However, the round counter is incremented (line 899) **before** the join window runs (lines 901-960), and `metrics.rounds` is therefore the new round number when the window opens — correct for `JoinAck.next_round_number` in `process_join_request` (line 192: `current_round + 1`), but the off-by-one becomes confusing when read against R16 ("the next round").

---

## 2. Findings

### Must-Fix (CRITICAL — block merge)

#### MF-001 (CRITICAL) — `WaitingForResults` does NOT accept new connections; mid-round joins are dropped at the OS level

**Category:** Spec Violation (R10b)
**Principle/Spec:** SPEC-20 §3.6 R10b ("TCP `accept()` completions during any non-`AcceptingMembershipChanges` state … MUST be buffered in a coordinator-local `pending_connections_queue`")
**File:** `relativist-core/src/protocol/coordinator.rs:619–758` (collect-results phase of `run_coordinator`)

**Problem:** The collect-results phase iterates `streams_to_poll` sequentially with `tokio::time::timeout(collect_timeout, recv_frame(...))` per worker (line 645–757). There is **no concurrent `transport.accept()` future**. New TCP connections during this phase are not accepted at all — they pile up in the kernel's TCP listen backlog. With a default Linux backlog of 128, a sufficiently busy round will silently lose connections. Per R10b, every state outside `AcceptingMembershipChanges` MUST buffer accepts into `pending_connections_queue`. The same gap applies to `Partitioning` (line 555–564, no select), `Dispatching` (line 602–617, no select), `Merging` (line 861–898, no select), and `CheckTermination` (lines 540–549).

**Before (current — collect loop without accept):**
```rust
for (wid, stream) in streams_to_poll {
    let recv_future = recv_frame(stream, config.max_payload_size);
    match tokio::time::timeout(config.collect_timeout, recv_future).await {
        // ... worker reply handling ...
    }
}
```

**After (recommended — collect with concurrent accept):**
```rust
// Collect-results phase MUST race result-frames against transport.accept()
// to satisfy R10b. The accept-future is wrapped to push into the queue.
loop {
    if streams_to_poll.is_empty() { break; }
    tokio::select! {
        accept_result = transport.accept() => {
            match accept_result {
                Ok(stream) => pending_connections_queue.push_back(stream),
                Err(e) => tracing::warn!(error = %e, "accept failed during collect"),
            }
        }
        // Pick first pending stream and try recv on it; replace with a polled
        // FuturesUnordered<...> for true concurrent collection if needed.
        recv_result = next_collect(&mut streams_to_poll, ...) => {
            // ... handle result, departure, error ...
        }
    }
}
```

A FuturesUnordered (or a per-stream tokio::spawn collecting into mpsc) is the cleanest fix; the tokio::select! sketch above is the minimum-change fix. The same pattern must be applied to the partition/dispatch/merge phases or, alternately, a long-lived `accept()` task on a tokio::spawn that sends accepted streams into a `mpsc` consumed by the main loop in every select. The latter is the cleanest architectural answer and decouples accept from any phase.

**Why:** This is the exact correctness regression QA-001 filed for Phase A. R10b is the requirement that closes SC-012; missing it leaves the system vulnerable to dropping joining workers under any non-trivial network arrival pattern, and the test that would catch it (EG-U6 `test_dynamic_join_mid_round_queued`) was forward-referenced from TASK-0432/0434 but not run because the test files don't exist yet (`docs/tests/TEST-SPEC-04{30..35}-*.md` are missing — see SF-001 below).

---

#### MF-002 (CRITICAL) — `SoloReducing` `tokio::select!` uses `else =>` for reduction work; reduction never runs

**Category:** Logic Bug (dead branch)
**Principle/Spec:** SPEC-20 R5 (`SoloReducing` MUST advance reduction in `reduce_n(solo_budget)` batches)
**File:** `relativist-core/src/protocol/coordinator.rs:515–538`

**Problem:** The `tokio::select!` is structured as:
```rust
while !current_net.redex_queue.is_empty() {
    tokio::select! {
        new_conn = transport.accept() => {
            // queue stream
        }
        else => {
            let stats = crate::reduction::reduce_n(&mut current_net, ...);
            // reduce
        }
    }
}
```

The `else =>` branch in `tokio::select!` runs **only when all other branches are disabled** (per tokio docs: "behaves like the default branch of a match expression — runs only if no branches in the select can match"). `transport.accept()` has no `if`-guard, so it is always enabled. Therefore `else` is **never** entered, and `reduce_n` is **never** called. `SoloReducing` becomes an infinite accept loop that makes zero reduction progress. The outer loop's `while !current_net.redex_queue.is_empty()` will never terminate from inside the block, only via external panic or shutdown.

**Before:**
```rust
tokio::select! {
    new_conn = transport.accept() => { ... }
    else => {
        let stats = crate::reduction::reduce_n(&mut current_net, grid_config.solo_budget as usize);
        ...
        if stats.total_interactions == 0 { break; }
    }
}
```

**After (recommended — use a `biased` select with a non-blocking try-accept, OR alternate via outer-loop turn-taking):**
```rust
// Option A: use try_accept (non-blocking) before each batch
loop {
    while let Ok(stream) = transport.try_accept() {
        pending_connections_queue.push_back(stream);
    }
    let stats = crate::reduction::reduce_n(&mut current_net, grid_config.solo_budget as usize);
    metrics.total_interactions += stats.total_interactions;
    // ... per-rule accumulation ...
    if stats.total_interactions == 0 || current_net.redex_queue.is_empty() { break; }
    tokio::task::yield_now().await;
}

// Option B: race accept against an explicit timeout/yield (cleaner async)
loop {
    tokio::select! {
        biased;
        new_conn = transport.accept() => { /* queue */ }
        _ = tokio::task::yield_now() => {
            let stats = crate::reduction::reduce_n(...);
            // ...
            if stats.total_interactions == 0 { break; }
        }
    }
}
```

Note that Option A requires `Transport` to expose a non-blocking try-accept (currently it does not; this would be an additive trait method). Option B works with the existing API.

**Why:** The current code is dead-on-arrival for `SoloReducing` — any test or real run that enters this state cannot make progress. The reason no test caught this is that no SoloReducing integration test exists in Phase C (test specs for 0430/0432/0433/0434/0435 are missing — see SF-001). EG-U1 (`test_hybrid_coordinator_single_machine`, R1/R5) was forward-referenced by TASK-0430 but appears never to have been written.

---

#### MF-003 (CRITICAL) — `WorkerIdSpaceExhausted` aborts the coordinator instead of NACKing the offender

**Category:** Spec Violation (R11 / SC-023)
**Principle/Spec:** SPEC-20 §3.2 R11 — "if `next_worker_id` would exceed `u32::MAX`, the coordinator MUST reject the join with `JoinNack { reason: WorkerIdSpaceExhausted }`". The coordinator SHOULD continue to serve other workers.
**File:** `relativist-core/src/protocol/coordinator.rs:170–179`

**Problem:**
```rust
if *next_worker_id == u32::MAX {
    let nack = Message::JoinNack {
        reason: crate::protocol::types::JoinNackReason::WorkerIdSpaceExhausted,
    };
    let _ = send_frame(stream, &nack).await;
    return Err(ProtocolError::Coordinator(Box::new(
        crate::error::CoordinatorError::WorkerIdSpaceExhausted,
    )));
}
```

The function sends `JoinNack` correctly, then returns `Err(...)`. The caller at line 922 uses `?` (line 932), which propagates and aborts `run_coordinator` entirely. The R11 contract is "reject this join" not "abort the whole run." TASK-0432 acceptance criterion 5 says "If `next_worker_id` would overflow `u32::MAX` → `JoinNack { WorkerIdSpaceExhausted }`" — without "abort." Test EG-U14 (`test_worker_id_exhaustion_join_nack`) is forward-referenced and explicitly checks the JoinNack-then-coordinator-still-running shape.

**Before:**
```rust
if *next_worker_id == u32::MAX {
    let nack = ...;
    let _ = send_frame(stream, &nack).await;
    return Err(ProtocolError::Coordinator(...));
}
```

**After:**
```rust
if *next_worker_id == u32::MAX {
    let nack = Message::JoinNack {
        reason: crate::protocol::types::JoinNackReason::WorkerIdSpaceExhausted,
    };
    let _ = send_frame(stream, &nack).await;
    tracing::warn!("WorkerId space exhausted — rejecting join (R11/SC-023)");
    return Ok(None); // refuse this join, keep the coordinator running
}
```

The `accept_workers` initial-handshake path (lines 234–243) has the same shape and arguably is correct *there* (during the initial-window enrollment, exhausting WorkerId space genuinely is fatal because every active worker counts), but the mid-session path *must* be soft.

**Why:** This is a direct violation of the explicit acceptance criterion. EG-U14 will fail.

---

#### MF-004 (CRITICAL) — TASK-0434's required FSM transitions for `WorkerJoined` in non-AMC states are missing

**Category:** Spec Violation (R10b FSM totality) / Acceptance criterion not met
**Principle/Spec:** SPEC-20 §3.6 R10b — "The FSM transitions for these states MUST include explicit handlers for `WorkerJoined(id)` events: `WaitingForResults × WorkerJoined(id) → WaitingForResults` with action `QueueWorkerForNextWindow(id)`, and similarly for `Partitioning`, `Dispatching`, `Merging`. This eliminates the FSM-totality gap flagged by SC-012."
**File:** `relativist-core/src/coordinator.rs:429–610` (the `transition()` function); TASK-0434 acceptance criterion 3
**Discovery path:** `grep -n QueueWorkerForNextWindow coordinator.rs` returns only the variant declaration (line 321) and a single test fixture (line 1088); no transition arm produces it.

**Problem:** The `QueueWorkerForNextWindow` action is defined but never emitted. Every `WorkerJoined` event in `Partitioning`/`Dispatching`/`WaitingForResults`/`Merging` falls through to the wildcard `(state, event) => { tracing::warn!(...) }` arm at line 600 and is **silently absorbed** with zero actions. The TASK-0414 reviewer (REVIEW-TASK-0414, SF-001) flagged exactly this risk: "any new SPEC-20 event that is accidentally omitted from the match arms will silently fall through to this same WARN — indistinguishable from a genuinely unexpected event." TASK-0434 did not address it.

The defense from the procedural-runtime side is "we buffer raw streams in `pending_connections_queue` so behavior is correct" — but R10b is **also** a formal FSM totality requirement (the spec language is explicit: "The FSM transitions for these states MUST include explicit handlers"), and TASK-0436 (the future FSM-wiring task) is not in scope for Phase C. Worse, until TASK-0436 lands, *anyone* who fires `WorkerJoined` into the FSM in any state silently sees zero actions, making FSM-driven testing impossible.

**Before:** No arm for `WorkerJoined` in any of the five states. Wildcard absorbs it.

**After:** Add four arms (or a single arm with state-list matching):
```rust
// SPEC-20 R10b — mid-state buffering of joins
(
    CoordinatorState::Partitioning
    | CoordinatorState::Dispatching
    | CoordinatorState::WaitingForResults
    | CoordinatorState::Merging,
    CoordinatorEvent::WorkerJoined(id),
) => {
    actions.push(CoordinatorAction::QueueWorkerForNextWindow(id));
    actions.push(CoordinatorAction::LogJoin(id));
    // state unchanged
}
```

This is the minimum-LoC fix that satisfies TASK-0434's third acceptance criterion verbatim. It is independent of the procedural runtime fix in MF-001 — both are required.

**Why:** TASK-0434 acceptance criterion 3 was not met. The QA-001 Phase A "wildcard absorbs critical events" pattern is reproduced verbatim. The fix is small (~10 LoC) and well-scoped. There is no reason it should be deferred to TASK-0436.

---

### Must-Fix (HIGH)

#### MF-005 (HIGH) — R10a drain-then-arm protocol is misimplemented; `JoinWindowMin` and `JoinWindowMax` are armed simultaneously

**Category:** Spec Violation (R10a)
**Principle/Spec:** SPEC-20 §3.2 R10a
**File:** `relativist-core/src/protocol/coordinator.rs:901–960`

**Problem:** Per R10a:
> "When the coordinator transitions into `AcceptingMembershipChanges`, it MUST first drain all pending TCP connections … Each drained connection completes the `Register` handshake. The coordinator **then** arms the `join_window_min` timer; on `MembershipWindowClosed_min`, **if no further pending connections have queued during the drain**, it transitions to `Partitioning`. If new pending connections **did** queue during the drain, it arms `join_window_max - join_window_min` and transitions to `Partitioning` on either the next drain-empty observation or the timer expiry, whichever comes first."

Current code:
```rust
let min_timer = tokio::time::sleep(grid_config.join_window_min);
let max_timer = tokio::time::sleep(grid_config.join_window_max);
tokio::pin!(min_timer);
tokio::pin!(max_timer);

loop {
    while let Some(mut stream) = pending_connections_queue.pop_front() { ... }
    if min_timer.is_elapsed() { break; }
    tokio::select! {
        new_conn = transport.accept() => { ... }
        _ = &mut min_timer => {}
        _ = &mut max_timer => { break; }
    }
}
```

The two timers are armed **simultaneously**. The drain is performed once per outer-loop iteration. There is no notion of "did new connections arrive during the drain" — once the drain runs, the loop checks `min_timer.is_elapsed()` and exits if so. The critical guarantee from R10a — "if drain produced no new pending, exit immediately at min" vs. "if drain produced new pending, extend to max" — is collapsed into "exit at min always (unless an accept-during-select interleaves)".

**Recommended fix (sketch):**
```rust
// 1. Drain all currently-queued streams (handshake them).
let new_arrivals_during_drain = drain_and_handshake(&mut pending_connections_queue, ...).await?;

// 2. Arm JoinWindowMin and accept concurrently.
tokio::pin! {
    let min_timer = tokio::time::sleep(grid_config.join_window_min);
}
let mut had_arrivals = new_arrivals_during_drain;
loop {
    tokio::select! {
        biased;
        _ = &mut min_timer => break,
        new_conn = transport.accept() => {
            pending_connections_queue.push_back(new_conn?);
            had_arrivals = true;
        }
    }
}

// 3. If anything arrived during drain or during min-window, extend to max.
if had_arrivals {
    drain_and_handshake(&mut pending_connections_queue, ...).await?;
    tokio::pin! {
        let extension_timer = tokio::time::sleep(
            grid_config.join_window_max - grid_config.join_window_min,
        );
    }
    loop {
        tokio::select! {
            biased;
            _ = &mut extension_timer => break,
            new_conn = transport.accept() => {
                pending_connections_queue.push_back(new_conn?);
            }
        }
        // Drain after each accept; exit on drain-empty observation.
        if pending_connections_queue.is_empty() { break; }
        drain_and_handshake(&mut pending_connections_queue, ...).await?;
    }
}
```

The key invariants are: (a) drain BEFORE arming `min`; (b) only arm `max` if drain or min-window saw arrivals; (c) exit on drain-empty observation OR `max` expiry. The current implementation violates (a), (b), and (c).

**Why:** This is a normative requirement. SC-007 closure is at stake. The test that would catch the divergence is EG-U6a (`test_join_window_boundary_race`) — also missing from Phase C test specs.

---

#### MF-006 (HIGH) — `run_coordinator` is a 513-LoC god-function; SRP heavily violated, blocks future FSM wiring

**Category:** Code Quality / Architecture (SRP, future-readiness for TASK-0436)
**Principle/Spec:** Clean Code (functions < 20 lines, single level of abstraction)
**File:** `relativist-core/src/protocol/coordinator.rs:455–967` (513 lines in one function)

**Problem:** `run_coordinator` now interleaves: `accept_workers` setup → SoloReducing branch → metrics setup → split → self-partition spawn → state-retention bookkeeping → distribute → collect (with departure detection) → departure-reclaim (with v1 reconstruct) → metrics finalization → merge → post-merge reduce_all → join window with timers and drain. There are eleven distinct phases. Variable shadowing is rampant (`remote_count` declared twice, lines 482 and 557; `k_eff` referenced in both metrics and partition contexts).

This god-function is the direct reason MF-001, MF-002, MF-005 went undetected: visual inspection of a 513-line function is hard, and unit-testing individual phases is impossible.

**Recommended refactoring (sketch):**
- Extract phase-helpers: `solo_reduce_phase(...)`, `partition_phase(...)`, `dispatch_phase(...)`, `collect_phase(...)`, `departure_reclaim_phase(...)`, `merge_phase(...)`, `join_window_phase(...)`. Each takes mutable references to the shared state structures (`current_net`, `metrics`, `worker_streams`, `pending_connections_queue`, `retained_state`, `next_worker_id`) and returns a small typed result (e.g. `JoinWindowOutcome { joined: u32 }`).
- Extract a `RunState` struct holding `current_net`, `metrics`, `worker_streams`, `next_worker_id`, `pending_connections_queue`, `retained_state` — currently all top-level locals in `run_coordinator`. This both shrinks the signatures of phase-helpers and provides a natural injection point for the FSM (TASK-0436 will need to drive `run_coordinator` via FSM events; that requires structured state).
- Use the FSM `transition()` to drive phase choice (MF-004 fix is a prerequisite). The procedural loop becomes "fire next event → transition → execute returned actions → goto top." This is the architecture SPEC-13 §4 already mandates.

**Why:** Without extraction, TASK-0436 (FSM wiring) will require a near-full rewrite of `run_coordinator`. Doing the extraction now, *before* the FSM is wired, is much cheaper than after. Furthermore, three of the four CRITICAL bugs above would have been visible at a glance in 50-line phase-helpers but were obscured in the 513-line monolith.

---

#### MF-007 (HIGH) — Production `unwrap()` at line 611

**Category:** Code Quality (no `unwrap()` in production paths — project standard)
**File:** `relativist-core/src/protocol/coordinator.rs:611`

**Problem:**
```rust
if let Some(ref mut h) = self_handle {
    let p = self_partition.as_ref().unwrap();   // <-- unwrap in production
    let msg = Message::AssignPartition { round: metrics.rounds, partition: p.clone() };
    bytes_sent += send_frame(&mut h.stream, &msg).await?;
}
```

The two are kept in sync earlier (lines 580–584: `self_handle = Some(...)` IFF `self_partition.is_some()`), so the unwrap is *currently* safe — but the invariant is implicit. A small refactor sets the contract explicitly:

**Before:**
```rust
let mut self_handle = if let Some(ref _p) = self_partition {
    Some(crate::protocol::self_worker::spawn_self_partition(config.max_payload_size).await)
} else { None };
// ... later ...
if let Some(ref mut h) = self_handle {
    let p = self_partition.as_ref().unwrap();
    ...
}
```

**After:**
```rust
let mut self_handle = if let Some(ref p) = self_partition {
    let h = crate::protocol::self_worker::spawn_self_partition(config.max_payload_size).await;
    Some((h, p.clone()))
} else { None };
// ... later ...
if let Some((ref mut h, ref p)) = self_handle {
    let msg = Message::AssignPartition { round: metrics.rounds, partition: p.clone() };
    bytes_sent += send_frame(&mut h.stream, &msg).await?;
}
```

This tightens the invariant and removes the unwrap.

**Why:** Project standard is "no `unwrap()` in production code." The current site is *technically* sound but the implicit invariant is an accident waiting to happen if the `self_handle`-construction branch ever changes.

---

### Must-Fix (MEDIUM)

#### MF-008 (MEDIUM) — Error-path send-frame results are discarded (`let _ =`)

**Category:** Code Quality (silent error swallowing)
**File:** `relativist-core/src/protocol/coordinator.rs:139, 148, 165, 174, 239, 253, 266, 273, 307, 312, 321, 680`

**Problem:** Every `send_frame(... &nack)` and `send_frame(... &Message::LeaveAck)` call uses `let _ = ...` to discard the I/O error. While it is reasonable not to escalate a NACK send failure to a fatal error (the connection is being terminated anyway), the systematic discard means a network failure during NACK is not even logged. This is hard to debug under churn.

**Before:**
```rust
let _ = send_frame(stream, &nack).await;
return Ok(None);
```

**After:**
```rust
if let Err(e) = send_frame(stream, &nack).await {
    tracing::warn!(error = %e, reason = ?nack_reason_for_log, "failed to send NACK to rejected joiner");
}
return Ok(None);
```

**Why:** `let _ =` on a `Result` is a code smell — at minimum the error should be logged so observability tooling can correlate "rejected joiner with no NACK delivered" against worker-side timeouts.

---

#### MF-009 (MEDIUM) — Departure path returns `Err(ProtocolError::Fatal(...))` after successful reconstruction (line 832)

**Category:** Code Quality / Logic (terminal abort on the success path)
**File:** `relativist-core/src/protocol/coordinator.rs:825–832`

**Problem:**
```rust
tracing::info!(
    agent_count = current_net.count_live_agents(),
    "Departure recovery reconstruction succeeded."
);
_round_reclaimed_initial += departing_worker_ids.len() as u32;
let _ = _round_reclaimed_initial;
// Remove departed from worker_streams (TODO: TASK-0443 follow-up)
return Err(ProtocolError::Fatal("Departure recovery reconstruction succeeded but stream management is TASK-0443 follow-up".into()));
```

This is a known stub from Phase D Wave 1 (TASK-0438), but it is **on the success path** of departure recovery: every successful departure recovery aborts the run. As a TODO this might be acceptable in the right context (Phase D is reviewed separately), but Phase C touches this region by introducing the `next_worker_id` state shared across the loop, which compounds the dependency. The `_ = _round_reclaimed_initial;` write-then-discard shows the field is unused-but-incremented.

**Why:** This is mostly a Phase D bug (out of scope for this review per the prompt), but it interacts with Phase C's correctness because the `pending_connections_queue` is dropped on the abort, losing any buffered joiners. Worth flagging here so the cross-Phase reviewers see the coupling.

---

#### MF-010 (MEDIUM) — `process_join_request` recomputes `partition_index` based on `active_workers.len()` but the caller passes a freshly-built `BTreeSet`

**Category:** Code Quality (primitive obsession + possibly wrong index)
**File:** `relativist-core/src/protocol/coordinator.rs:184–189` and call site lines 911–920

**Problem:** `partition_index` is computed inside `process_join_request` as `active_workers.len() as u32 + (1 if hybrid)`. But the call site (lines 912–919) constructs `active_ids` from `worker_streams` in *index order*, which is dense (0..N) regardless of WorkerId. Worse, this is computed once per drained pending connection inside the inner loop — but `worker_streams` doesn't yet include the connection currently being handshook, so the index is correct for the FIRST drained connection; the SECOND drained connection sees the same `worker_streams.len()` because the prior one hasn't been pushed yet.

Wait — the push happens AFTER `process_join_request` returns (line 934). So by the time the second pending connection is processed, `worker_streams` has grown by one. Good — the index advances correctly. But the BTreeSet is rebuilt every iteration; the iteration order of `enumerate()` over `worker_streams.iter()` is dense by Rust's `Vec` semantics, so the BTreeSet is `{1, 2, 3, ..., N}` in hybrid mode (or `{0, 1, ..., N-1}` non-hybrid). The `+1` in `partition_index` calculation already accounts for the self-partition.

So the math is correct under the current naming convention. The CONCERN is: per R11a (D4-elastic), `partition_index` is the *position* of the worker in `W_active ∪ {self}` sorted ascending by `WorkerId`. `WorkerId`-as-index assumes WorkerIds are contiguous and dense. After multiple join/leave cycles WorkerIds become sparse (0, 3, 7, ...), and `worker_streams[i].partition_index = i + offset` no longer matches "sorted-by-WorkerId position". For Phase C alone (joins only, no leaves) this is OK, but it will fail Phase D (departures) tests immediately.

**Recommended:** Don't recompute `partition_index` from `worker_streams.len()`. Track it explicitly as a `BTreeMap<WorkerId, partition_index>` rebuilt at each window close. The current code happens to give the right answer for Phase C in isolation but is a latent bug for any Phase D scenario.

**Why:** Primitive-obsession + position-vs-id confusion. R11a specifically warns about this (closes SC-006). The code "works" for Phase C-only tests but breaks under any departure cycle.

---

#### MF-011 (MEDIUM) — `accept_workers` initial-window discards the `JoinRequest` after NACK without dropping the stream

**Category:** Code Quality (resource leak on rejected initial joiner)
**File:** `relativist-core/src/protocol/coordinator.rs:290–315`

**Problem:** When a worker arrives at the initial window with a `JoinRequest` instead of `Register`, we send a `JoinNack` and `continue` (line 314). The stream is never explicitly dropped — it's discarded with the loop iteration. That's correct via Rust's drop semantics, but the half-closed TCP state (we wrote a NACK frame, then drop without close-shutdown) is not graceful. The kernel will FIN-RST the peer.

The comparable Register-NACK path at lines 244–254/263–267/318–321 has the same shape. This is a long-standing pattern (not introduced in Phase C), so it is below the must-fix bar — but worth noting because Phase C added one more such site.

**Why:** Cosmetic / observability. Future graceful-shutdown work (SPEC-08?) should add an explicit `stream.shutdown()` after NACK.

---

#### MF-012 (MEDIUM) — `join_window` Min/Max timer literals do not use `TimerKind` discriminants

**Category:** Spec Violation (NF-008 spirit) / Code Quality
**Principle/Spec:** SPEC-20 §4.1.3 — "All transition rows in §4.1.4 use `StartTimer(TimerKind::X, duration)` and `CancelTimer(TimerKind::X)`."
**File:** `relativist-core/src/protocol/coordinator.rs:905–906`

**Problem:** The Phase C join-window does not use `CoordinatorAction::StartTimer(TimerKind::JoinWindowMin, ...)` because the procedural loop bypasses the FSM. It uses `tokio::time::sleep(grid_config.join_window_min)` directly. While that is fine for the procedural loop (which doesn't go through the action enum), the consequence is that when TASK-0436 wires the FSM, the timer arming/cancelling MUST be re-implemented because `tokio::time::sleep` produces a future, not an action. The transition from "procedural sleep futures" to "FSM-driven `StartTimer(TimerKind::JoinWindowMin)` actions" is a non-trivial rewrite.

The fix is preventive: make the procedural loop also go through `TimerKind::JoinWindowMin as TimerId` and `TimerKind::JoinWindowMax as TimerId` for log/trace purposes, even if the procedural side uses `tokio::time::sleep`. This costs essentially zero and gives observability tooling a stable handle.

**Recommended:** Add `tracing::debug!(timer_kind = ?TimerKind::JoinWindowMin, ...)` at the points the futures are pinned, so logs can correlate. Then the procedural→FSM transition is purely structural.

**Why:** NF-008 is about "log analysis tooling can decode `TimerId -> TimerKind` without per-build metadata." Bypassing the enum entirely loses that benefit. This is MEDIUM rather than HIGH because the procedural runtime is itself transitional (TASK-0436 will rework it).

---

### Should-Fix (LOW)

#### SF-001 (LOW) — Phase C test specs (`TEST-SPEC-04{30,32,33,34,35}`) do not exist; tests forward-referenced but never written

**Category:** Test Coverage / Stage 2 (test-generator) gap
**File:** `docs/tests/` directory listing

**Problem:** The prompt's mandatory-check item 7 requested verification of "Test coverage vs the 5 test specs." None of the five test spec files exist in `docs/tests/`. The TASK files forward-reference EG-U1, EG-U2, EG-U3, EG-U4, EG-U5, EG-U6, EG-U6a, EG-U14, EG-U15b, EG-I1, EG-I2 — those EG-U* test specs DO exist (in `docs/tests/`), but they are spec-level tests for SPEC-20, not the per-task TEST-SPEC-0430 etc. that Stage 2 is meant to produce.

Either (a) Stage 2 was skipped for Phase C entirely, or (b) the per-task TEST-SPECs were merged into the EG-U* files. Either is OK in principle but the bookkeeping is broken. The commit log shows "Test deltas: Default lib: 1256 → 1256 (structural orchestration wave)" — meaning **zero new tests** were added in Wave 1, **zero** in Wave 2, **zero** in Wave 3. The acceptance criteria of TASK-0432 explicitly list "EG-U5, EG-U6, EG-U14, EG-U15b" as expected tests; if those are existing EG-U* files, none were *populated* with the new behavior in Phase C.

In short: no new tests were added across Phase C, despite the task contracts each listing 80–150 LoC of test additions.

**Recommended:** Add at minimum the four CRITICAL-bug regression tests:
- `test_waiting_for_results_buffers_mid_round_accept` (MF-001 closure).
- `test_solo_reducing_makes_progress_under_concurrent_accepts` (MF-002 closure).
- `test_worker_id_exhaustion_returns_join_nack_without_aborting` (MF-003 closure).
- `test_fsm_worker_joined_in_waiting_for_results_emits_queue_action` (MF-004 closure).

**Why:** "Tests must accompany code changes" is project standard. Three structural waves with zero net test additions is a flag.

---

#### SF-002 (LOW) — Test-count claim in commit messages does not match the CLAUDE.md baseline

**Category:** Documentation accuracy
**File:** Commit messages of `c87b048`, `13def72`, `613c847`

**Problem:** Each commit claims "Default lib: 1256 → 1256." `relativist/CLAUDE.md` baseline says "1181 default / 1224 zero-copy." Either a previous bundle moved the floor up to 1256, or the commit message numbers are wrong. Reviewer cannot run `cargo test` from this review, but the discrepancy should be reconciled in a follow-up commit (update CLAUDE.md or correct the commit messages). Not a Phase C concern strictly — flagging because it affects regression-floor enforcement.

---

#### SF-003 (LOW) — `process_join_request`'s caller does the `recv_frame` but the function signature accepts `Message`, splitting concerns awkwardly

**Category:** Code Quality (cohesion)
**File:** `relativist-core/src/protocol/coordinator.rs:108–209` (function) and 921–932 (call site)

**Problem:** The caller does:
```rust
let (msg, _) = recv_frame(&mut stream, config.max_payload_size).await?;
if let Some(worker_id) = process_join_request(&mut stream, msg, ...).await? { ... }
```

`process_join_request` then pattern-matches `msg` and rejects "expected JoinRequest, got X" (lines 119–129). This is an odd split: the function name suggests "I will read and process," but the read is external. Either name it `process_join_message(stream, msg, ...)` or fold the `recv_frame` inside.

**Recommended (folding the recv inside):**
```rust
pub async fn process_join_request(
    stream: &mut TransportStream,
    grid_config: &GridConfig,
    ...
) -> Result<Option<WorkerId>, ProtocolError> {
    let (msg, _) = recv_frame(stream, ...).await?;
    let (protocol_version, auth_token) = match msg { Message::JoinRequest { .. } => (...), _ => ... };
    // ... rest unchanged
}
```

Then the caller is simply:
```rust
if let Some(worker_id) = process_join_request(&mut stream, ...).await? { ... }
```

**Why:** Single-level-of-abstraction (Clean Code). Cosmetic; not blocking.

---

#### SF-004 (LOW) — `_round_reclaimed_initial` and `round_reclaimed_last_acked` are mutated then discarded

**Category:** Code Quality (dead writes)
**File:** `relativist-core/src/protocol/coordinator.rs:622, 829`

**Problem:** `_round_reclaimed_initial` is incremented (line 828) then read once into `let _ = _round_reclaimed_initial;` (line 829). The read does nothing. `round_reclaimed_last_acked` is declared `0` (line 623) and never assigned. Both feed `metrics` lines 840–844. This is from Phase D Wave 1 stubbing but lives in the code that Phase C now compiles into. The patterns look like "I need to push something into metrics; placeholder until Phase D," which is fine — but the underscore-prefixed names misrepresent intent (`_round_reclaimed_initial` reads as "intentionally unused," yet it IS used in line 841).

**Recommended:** Drop the leading underscore (it IS used) or wrap the metrics push in a guard. Cosmetic.

---

## 3. Passed Checks

- [x] No `unsafe` blocks added in Phase C
- [x] No `println!` — `tracing::info!` / `warn!` / `error!` only
- [x] `thiserror` errors propagated through `?` (apart from MF-003 abort-vs-nack)
- [x] `Message::JoinRequest` / `Message::JoinAck` / `Message::JoinNack` reused from TASK-0418 (`protocol/types.rs`); no duplicate variant definitions
- [x] `JoinNackReason` enum reused; new reasons `ProtocolVersionMismatch`, `ElasticJoinDisabled`, `AuthenticationFailed`, `WorkerIdSpaceExhausted` correctly emitted (R0d, R9, SPEC-10, R11/SC-023)
- [x] `accept_workers` rejects `JoinRequest` during the initial window with a `JoinNack` that includes correct `expected/got` shape per NF-009 / SPEC-20 R37a
- [x] `accept_workers` rejects v1 `Register` with `RegisterNack { reason: "protocol version mismatch: expected 4, got 1" }` shape (qa_probe_5 / qa_probe_9 confirm)
- [x] R12-v1 (re-partition over merged net at next round) is implemented procedurally — `current_net` after merge feeds the next iteration's `split(current_net, k_eff_new, strategy)` (line 563)
- [x] R13 (id-range recomputation) correct — `split()` invokes `compute_id_ranges(K_eff_new)` internally
- [x] `pending_connections_queue: VecDeque<TransportStream>` field added to coordinator state (line 481)
- [x] Connections accepted during `SoloReducing` are pushed into the queue (line 521) — but see MF-002 (the surrounding loop never reduces)
- [x] Connections accepted during the Join Window's idle-wait are pushed into the queue (line 953)
- [x] `partition_index` derives from `active_workers.len() + (1 if hybrid)` (line 184–189) — correct for Phase C-only flow (caveat MF-010 for Phase D coupling)
- [x] `next_round_number` computed as `current_round + 1` (line 192) — matches R16 for joiners arriving between rounds
- [x] Hybrid mode partitions `partitions[0]` → self, `partitions[1..]` → remote (line 572–578) — TASK-0430 acceptance criterion 2
- [x] Hybrid mode self-partition `WorkerId = 0` reserved (line 642 — `streams_to_poll.push((0, ...))`) — R7a
- [x] Hybrid mode joiner counter starts at `1` (line 483: `(remote_count + 1) as u32`) — R7a
- [x] R28 graceful-departure logging path present (line 672–683)
- [x] `LeaveAck` sent before stream removal (line 680) — R28 / EG-U19
- [x] `TimerKind` discriminants pinned in tests (the FSM `coordinator.rs` test `qa_003_timerkind_discriminants_pinned`)
- [ ] R10b mid-state buffering covers ALL non-AcceptingMembershipChanges states (MF-001: `WaitingForResults`/`Partitioning`/`Dispatching`/`Merging`/`CheckTermination` do NOT accept)
- [ ] `SoloReducing` state advances reduction concurrently with accepts (MF-002: `else =>` branch is dead)
- [ ] `WorkerIdSpaceExhausted` returns `JoinNack` and continues the run (MF-003: aborts via `Err(...)`)
- [ ] FSM transitions for `WorkerJoined` in `Partitioning`/`Dispatching`/`WaitingForResults`/`Merging` emit `QueueWorkerForNextWindow(id)` (MF-004: wildcard absorbs)
- [ ] R10a drain-then-arm protocol matches spec sequence (MF-005: timers armed simultaneously)
- [ ] `run_coordinator` is decomposed into phase-helper functions (MF-006: 513-line god-function)
- [ ] No production `unwrap()` (MF-007: line 611)

---

## 4. Spec Compliance Summary (SPEC-20 §3.2 R8–R17)

| Requirement | Status | Notes |
|-------------|--------|-------|
| R8 (id-range from `compute_id_ranges(K_eff)`) | PASS | `split()` → `compute_id_ranges` is invoked uniformly per round. |
| R9 (accept mid-run TCP) | PARTIAL | Procedural runtime only accepts during `SoloReducing` and Join Window — MF-001. |
| R10 (Join Window state) | PASS structurally | `AcceptingMembershipChanges` is reachable in the FSM (TASK-0414) but not driven by the procedural runtime. |
| R10a (drain-then-arm) | FAIL | Timers armed simultaneously — MF-005. |
| R10b (mid-state pending queue) | PARTIAL | Field exists; queue used in 2 of 7 states; FSM totality missing — MF-001 + MF-004. |
| R11 (WorkerId allocation, no reuse) | PASS for the join path; FAIL for exhaustion | MF-003: exhaustion aborts the coordinator instead of NACKing. |
| R11a (partition_index decoupling) | PASS for Phase C-only | Latent bug for Phase D (sparse WorkerIds) — MF-010. |
| R12-v1 (re-partition at next round) | PASS | `split(current_net, k_eff_new, ...)` is invoked uniformly. |
| R13 (id-range recompute) | PASS | inherited from R8. |
| R14-v1 (`AssignPartition` to joiner) | PASS | At the next round's distribute phase. |
| R15 (Solo→grid transition) | FAIL | MF-002: SoloReducing reduce_n never fires. |
| R16 (mid-round joins queued, registered next window) | PARTIAL | Buffering works in Solo/JoinWindow only — MF-001. |
| R17 (INFO log on join) | PASS | `tracing::info!` at lines 200–206 and 940–945 emits worker_id, partition_index, k_eff_new, round. |
| R0d / NF-009 (version mismatch shape) | PASS | `JoinNack { ProtocolVersionMismatch { expected, got } }` correctly produced. |
| R35a (NACK reasons) | PASS | All four reasons are emitted at correct sites. |

---

## 5. Architecture Assessment

**Module dependencies:** `protocol/coordinator.rs` imports from `merge`, `partition`, `reduction`, `security`, `protocol::*`. All within the allowed direction per SPEC-13 R28 (`net <- reduction <- partition <- merge <- protocol`). No regressions.

**Core layer purity:** `protocol/coordinator.rs` is correctly above the core layer; no core-layer file in Phase C reaches up into `protocol`. Confirmed.

**FSM-vs-runtime split:** This is the central architectural concern. Phase C operates entirely on the procedural side; the formal FSM (`coordinator.rs::transition`) is unchanged. The result is:

1. **Two parallel implementations of the join-state-machine semantics**: one in `transition()` (incomplete, wildcard-absorbed), one in `run_coordinator` (procedural, with the bugs above).
2. **No way to test FSM totality from `run_coordinator`** because the procedural loop never fires events into `transition()`.
3. **TASK-0436 will need to reconcile these two implementations** by having the procedural loop fire events into `transition()` and execute the resulting actions. This is the standard architecture for FSM-driven async runtimes, and it is the only way to satisfy R10b's "FSM transitions MUST include explicit handlers" wording.

The split is *temporally* valid (TASK-0414 wrote the FSM enums; Phase C writes the runtime; TASK-0436 will wire them) but the gap is filling up with bugs and the eventual reconciliation cost is rising. The MF-006 refactoring recommendation is the cheapest path to closing the gap before TASK-0436 lands.

---

## 6. Stage 5 QA Readiness

**REJECT_WITH_FIXES.** The four CRITICAL findings are all behavior-level correctness bugs that QA will hit on the first non-trivial test: MF-001 will surface in any test that exercises mid-round connect (EG-U6); MF-002 will surface in any test that enters `SoloReducing` (EG-U1); MF-003 will surface in any test that triggers WorkerId exhaustion (EG-U14); MF-004 will surface in any FSM-driven test that fires `WorkerJoined` outside `AcceptingMembershipChanges`. None of those tests exist yet (SF-001), so the Phase C wave landed without exercising any of the spec requirements it claims to implement — a serious Stage 2 gap.

**Recommended path:**
1. **Block on MF-001 through MF-004** — fix all four CRITICAL bugs before any further wave depending on Phase C lands.
2. **Block on MF-005** — R10a is a normative MUST.
3. **Concurrently address MF-006** — extract phase-helpers from `run_coordinator` to make MF-001/002/003 fixes localized rather than further-burying them in the god-function.
4. **Add the four regression tests listed in SF-001.**
5. **Then re-dispatch to Stage 5 QA.**

The HIGH (MF-005, MF-006, MF-007), MEDIUM (MF-008–012), and LOW (SF-001–004) findings collectively suggest Phase C should be considered partially complete: the type-and-wire-protocol surfaces are correct (R8/R11 path/R12-v1/R13/R14-v1/R17/R0d/NF-009 all pass), but the runtime semantics (R10a/R10b/R15) and the FSM totality (R10b FSM) are unimplemented or incorrect. A sub-bundle "Phase C.5" addressing the four CRITICALs and MF-005 would be a clean follow-up.

---

## 7. Stage 6 Action List (for Developer)

| # | Severity | Action | File | Lines |
|---|----------|--------|------|-------|
| 1 | CRITICAL (MF-001) | Add concurrent `transport.accept()` arm to the collect-results phase (and Partitioning/Dispatching/Merging) so mid-round arrivals push into `pending_connections_queue` | `protocol/coordinator.rs` | 619–758 (and surrounding phases) |
| 2 | CRITICAL (MF-002) | Replace `else =>` branch in SoloReducing select with a real reduction trigger (Option B sketch above using `tokio::task::yield_now()` as the second arm, OR rewrite to do try-accept + reduce_n in alternation) | `protocol/coordinator.rs` | 515–538 |
| 3 | CRITICAL (MF-003) | `process_join_request` exhaustion path: send `JoinNack { WorkerIdSpaceExhausted }`, return `Ok(None)`, do NOT abort run | `protocol/coordinator.rs` | 170–179 |
| 4 | CRITICAL (MF-004) | Add four FSM transition arms `(Partitioning|Dispatching|WaitingForResults|Merging, WorkerJoined(id)) → QueueWorkerForNextWindow(id) + LogJoin(id)` to `transition()` | `coordinator.rs` (FSM, NOT protocol/coordinator.rs) | 433–608 |
| 5 | HIGH (MF-005) | Reimplement Join Window per R10a: drain → arm min → on min if had_arrivals arm `(max - min)` else exit; exit on drain-empty observation OR max | `protocol/coordinator.rs` | 901–960 |
| 6 | HIGH (MF-006) | Extract phase-helpers (`solo_reduce_phase`, `partition_phase`, `dispatch_phase`, `collect_phase`, `merge_phase`, `join_window_phase`); introduce `RunState` struct | `protocol/coordinator.rs` | 455–967 |
| 7 | HIGH (MF-007) | Pair `self_handle` with `self_partition` clone in a tuple to remove the `unwrap()` at line 611 | `protocol/coordinator.rs` | 580–617 |
| 8 | MEDIUM (MF-008) | Replace `let _ = send_frame(...)` with explicit `if let Err(e) = ... { tracing::warn!(...); }` on all NACK send sites | `protocol/coordinator.rs` | 139, 148, 165, 174, 239, 253, 266, 273, 307, 312, 321, 680 |
| 9 | MEDIUM (MF-010) | Track `partition_index` via an explicit `BTreeMap<WorkerId, partition_index>` rebuilt at each join window close, not via `worker_streams.len()` | `protocol/coordinator.rs` | 184–189, 911–920 |
| 10 | MEDIUM (MF-012) | Emit `tracing::debug!(timer_kind = ?TimerKind::JoinWindowMin, ...)` at the procedural-sleep pin sites for log decoding parity with NF-008 | `protocol/coordinator.rs` | 905–906 |
| 11 | LOW (SF-001) | Author the four regression tests for MF-001/002/003/004 listed in SF-001 | `relativist-core/tests/` (new) | new |
| 12 | LOW (SF-002) | Reconcile commit-message test counts vs `CLAUDE.md` baseline (1181 vs 1256) | `CLAUDE.md` or commit log | — |
| 13 | LOW (SF-003) | Optionally fold `recv_frame` into `process_join_request` body for cohesion | `protocol/coordinator.rs` | 108–209 + 921–922 |
| 14 | LOW (SF-004) | Drop leading-underscore prefix on `_round_reclaimed_initial` (it IS read into metrics) | `protocol/coordinator.rs` | 622, 828–829 |

Items 1–4 are merge-blockers. Item 5 should ship in the same PR. Items 6–7 should ship in the same wave; deferring them past TASK-0436 will multiply the FSM-wiring cost. Items 8–14 may ship in a follow-up.

---

## 8. Cross-Reference to QA-001 Phase A

The prompt notes "(lesson from QA-001 Phase A — they found a CRITICAL exactly here)." That CRITICAL was `WorkerJoined` falling through the wildcard arm of `transition()`. **The same bug is present in Phase C** (MF-004), in the literal same arm of the literal same function. The TASK-0414 reviewer SF-001 flagged the wildcard's silent-absorption risk for any future task that adds new transition rows; QA-001 confirmed the risk had materialized; TASK-0434 was tasked with closing it ("FSM transitions for `WorkerJoined(id)` ... → same state + action `QueueWorkerForNextWindow(id)`"); TASK-0434 instead added only the runtime-side queue field and never wrote the four FSM rows. The recurrence of the pattern across Phase A and Phase C indicates a process gap (Stage 4 review of a task should mechanically check that every "FSM transition" item in the acceptance list shows up as a literal arm in `transition()`). Worth raising at the SDD-pipeline level.

---

Phase C: 4 Must-Fix (CRITICAL), 3 Must-Fix (HIGH), 5 Must-Fix (MEDIUM), 4 Should-Fix (LOW), verdict: REJECT_WITH_FIXES
