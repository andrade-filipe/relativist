# Review — Phase D (Departure) of SPEC-20 Elastic Grid bundle (TASK-0438..0443)

**Date:** 2026-04-27
**Stage:** 4 (REVIEW) — unified code-quality + architecture review
**Reviewer:** reviewer agent
**Bundle:** SPEC-20 §3.3 Dynamic Worker Departure (R13-R28, plus retained-state R23/R31, plus FSM amendments §3.8 A4 / §4.1.4)

**Commits reviewed:**
- `8366ef3` — Phase D Wave 1 — TASK-0438 (departure detection: timeout + connloss), TASK-0439 (retained-state bookkeeping)
- `1da2c28` — Phase D Wave 2 — TASK-0440 (v1 reclaim/resplit), TASK-0441 (graceful leave handshake)
- `583a368` — Phase D Wave 3 — TASK-0442 (D == K_eff edge case), TASK-0443 (delta reconstruct/reclaim)

**Files reviewed:**
- `relativist-core/src/protocol/coordinator.rs` (heavy modifications across all 3 waves; 1211 LoC)
- `relativist-core/src/protocol/retained.rs` (NEW — Wave 1; 87 LoC)
- `relativist-core/src/partition/departure_recovery.rs` (NEW — Wave 3; 67 LoC)
- `relativist-core/src/protocol/mod.rs` (Wave 1)
- `relativist-core/src/merge/types.rs` (Wave 1 — minor lint fix)
- Cross-refs: `specs/SPEC-20-elastic-grid.md` §3.3 R13-R28, §3.6 R23/R31, §3.8 A4, §4.1.4
- Task contracts: `docs/backlog/TASK-04{38,39,40,41,42,43}-*.md`
- Test specs: `docs/tests/TEST-SPEC-EG-U7..U13, U19, I3, I5*` (reviewed for coverage gap)

**Code-quality verdict:** NEEDS REFACTORING
**Architecture verdict:** MINOR DRIFT
**Spec compliance:** SPEC-20 §3.3 R13-R28 — partial (R14, R18, R19 wired; R15 violated; R16 conservative-only; R18-id-disjointness violated; R20/R22a-c structurally present; R26a degraded to `Err`)
**Overall verdict:** **REJECT — REQUIRES REWORK BEFORE STAGE 5 QA**

---

## 1. Summary

Phase D delivers the *type surface* and the *detection plumbing* for departure recovery, but the actual **recovery path is non-functional**: every code path that observes a departure terminates the run with `ProtocolError::Fatal`, regardless of whether the reconstruction would have succeeded. The most consequential single line is `coordinator.rs:832`:

```rust
return Err(ProtocolError::Fatal(
    "Departure recovery reconstruction succeeded but stream management is TASK-0443 follow-up".into(),
));
```

This sits *unconditionally* after the `materialize_reclaimed_partitions` call and the `reconstruct(...)` call. The successful reconstruction never reaches the next round. Combined with R26a's hybrid branch also returning `Fatal` (line 784), this means **NO observable departure scenario produces a recovered run** — they all degenerate to abort. SPEC-20 R18 (`elastic_departure = true → recovery instead of fatal`) is therefore not satisfied at the system level, even though the per-event helpers (`handle_connection_loss`, `handle_phase_timeout`) correctly classify the event.

Compounding this, Phase D ships:
- **Zero unit tests** for the two new modules (`retained.rs`, `departure_recovery.rs`).
- **Zero integration tests** for any of the 19 EG-* test specs the bundle's task contracts forward-reference (EG-U7, EG-U7a/b/c, EG-U8, EG-U9, EG-U10/a/b/c, EG-U13, EG-U19, EG-I3/I3-delta, EG-I5a/b, EG-P2, EG-P5, EG-P6).
- A production `unwrap()` on `self_partition.as_ref().unwrap()` (line 611), violating the project's "no `unwrap()` in production code" rule.
- An `IdRange { start: 0, end: 100_000 }` literal as a "placeholder" in the reclaim path (line 805-808) that **structurally violates** SPEC-20 R18/R24d (D3-elastic ID-range disjointness).
- A `RetainedLastAcked::DeltaLight { placeholder: String }` variant in `retained.rs` — a wire-serializable enum carrying a `String` field literally named `placeholder`, with `serde::Serialize` and `rkyv` derives.

The Wave 1+2+3 commit messages note "Test deltas: 1256 → 1256" — the bundle adds **no new tests**. The "structural" framing is accurate for the type surface, but it does not match the task contracts, which explicitly list test acceptance criteria.

What *is* sound:
- Per-event detection helpers (`handle_connection_loss`, `handle_phase_timeout`) are pure, branch correctly on `elastic_departure`, and emit appropriate `tracing::warn!` events with structured fields. R18/R19 detection logic is correct.
- `RetainedStateRegistry` (R23/R31) has the correct shape, atomic-refresh API, debug-assertion bounds, and release semantics. Sole flaw: `DeltaLight` is a stub.
- `materialize_reclaimed_partitions` (R24a conservative path) correctly invokes `remap_partition_ids` and includes a debug-assertion for R24d range disjointness — the helper itself is correct; the **caller** in `coordinator.rs` undermines it by passing a single overlapping range for every departed worker.
- `LeaveRequest`/`LeaveAck` handshake is wired: coordinator handles `Message::LeaveRequest { kind }`, sends `LeaveAck`, and routes to the departing-IDs list (R20, R35a structurally present, but see MF-005 for R22a/c semantics).
- `serde::Serialize` is one-way on FSM-internal enums; `rkyv` archive derives are correctly conditional on `#[cfg(feature = "zero-copy")]`.
- `tracing::error!`/`tracing::warn!` used throughout — no `println!`. The R28 WARN-log requirement is satisfied for all three departure types (graceful, error, timeout).

The bundle should be **rejected back to Stage 3 (DEV)** for completion of the recovery path before Stage 5 QA dispatches. QA cannot meaningfully exercise EG-U7/U8/U9/U10/U13/U19 against a code path that returns `Fatal` on every departure event.

---

## 2. Findings

### Must-Fix (CRITICAL)

#### MF-001 — Successful reconstruction unconditionally returns `Fatal`, defeating R18/R24/R26

**Category:** Spec Violation (SPEC-20 R18, R24a-v1, R24b-v1, R26)
**Principle/Spec:** SPEC-20 §3.3.1 R18 (elastic_departure → recovery, not fatal); §3.3.4 R24a/R24b/R26 (reclaim + re-dispatch); SPEC-13 R28
**File:** `relativist-core/src/protocol/coordinator.rs:766-833`

**Problem:** The reclaim block runs to completion — `materialize_reclaimed_partitions` returns `Ok`, `reconstruct(...)` produces a valid `Net`, `current_net` is updated, and a "Departure recovery reconstruction succeeded" INFO log fires. Then, **unconditionally**, the function returns:

```rust
return Err(ProtocolError::Fatal("Departure recovery reconstruction succeeded but stream management is TASK-0443 follow-up".into()));
```

This converts every successful recovery into an aborted run, fully nullifying R18 (`elastic_departure = true → recovery, not fatal`). The detection plumbing exists (TASK-0438), the retained state exists (TASK-0439), the materialization helper exists (TASK-0443), the `reconstruct` 3-arg call works (TASK-0412) — but the round never advances because the function exits the loop with `Err`.

Worse, this `Err` is fired **after** updating `metrics` (lines 836-846 are dead code on the recovery path — they appear after the early return), so the `partitions_redispatched_per_round` and reclaim counters are never recorded for the departure event that actually happened.

**Before (current):**
```rust
let merged_net = reconstruct(&border_graph, surviving_partitions, reclaimed_partitions);
current_net = merged_net;
tracing::info!(agent_count = current_net.count_live_agents(), "Departure recovery reconstruction succeeded.");

_round_reclaimed_initial += departing_worker_ids.len() as u32;
let _ = _round_reclaimed_initial;

// Remove departed from worker_streams (TODO: TASK-0443 follow-up)
return Err(ProtocolError::Fatal("Departure recovery reconstruction succeeded but stream management is TASK-0443 follow-up".into()));
```

**After (required):** Either (a) implement the stream-removal step (drop the `TransportStream`s for `departing_worker_ids` from `worker_streams` and ensure metric recording proceeds), or (b) split the bundle: open a follow-up task `TASK-0443a-stream-pruning` and **defer the entire `if !departing_worker_ids.is_empty()` block to that task**, falling back to the v1 fatal-on-disconnect behavior for Phase D such that detection remains armed but recovery is opt-in via a feature flag. Shipping Phase D as written means SPEC-20 R18 is structurally violated for every departure-mode integration test.

**Why:** This is the central deliverable of the entire bundle. The detection (TASK-0438), retained state (TASK-0439), v1 reclaim (TASK-0440), and delta reclaim (TASK-0443) all flow through this site. A pre-merge `return Err` short-circuits all of them. None of the EG-U7/U8/U10/I3 tests can pass against this code path; they will all observe `Err(ProtocolError::Fatal(...))` after the first departure event.

---

#### MF-002 — R26a hybrid branch aborts with `Fatal` instead of falling back to `SoloReducing`

**Category:** Spec Violation (SPEC-20 R26a-hybrid, R27)
**Principle/Spec:** SPEC-20 §3.3.4 R26a (D == K_eff hybrid → SoloReducing fallback; non-hybrid → Error); R27 (hybrid + all-remote-departed → solo)
**File:** `relativist-core/src/protocol/coordinator.rs:773-792`

**Problem:** R26a explicitly mandates that under `D == K_eff` with `hybrid_coordinator = true`, the coordinator continues solo with the self-partition (or transitions to `SoloReducing`) and queues reclaimed partitions for the next `AcceptingMembershipChanges` window. The current implementation aborts with `Fatal` on **both** branches:

```rust
if departing_worker_ids.len() >= k_eff {
    tracing::error!("All workers departed! D={}, K_eff={}", ...);
    if grid_config.hybrid_coordinator {
        tracing::warn!("Hybrid mode: falling back to SoloReducing.");
        // In a real implementation we'd reclaim state and continue.
        // For this wave, we'll abort to satisfy P0 safety.
        return Err(ProtocolError::Fatal(
            "All workers departed including self-handle logic".into(),
        ));
    } else {
        return Err(ProtocolError::Fatal(
            "All workers departed and non-hybrid mode".into(),
        ));
    }
}
```

The hybrid branch logs `"Hybrid mode: falling back to SoloReducing"` and then immediately aborts. The comment "For this wave, we'll abort to satisfy P0 safety" is a placeholder, not a defense — TASK-0442's acceptance criteria explicitly require both branches to be implemented:

> - [ ] **Hybrid branch**:
>   - Discard `retained_last_acked` for all D workers; fall back to `retained_initial[w]` (R24a conservative).
>   - Skip `FinalStateRequest` broadcast (no recipients).
>   - Enter `SoloReducing` (or continue self-partition progress) via R27.
>   - Reclaimed partitions queued for next AcceptingMembershipChanges window.

None of these four bullets is implemented; the hybrid branch is functionally identical to the non-hybrid branch.

Additionally, the condition `departing_worker_ids.len() >= k_eff` is sound only if `k_eff` here equals **K_remote**, but the code computes `k_eff = remote_count + if hybrid_coordinator { 1 } else { 0 }`. In hybrid mode, `D == K_remote` (all remotes departed) implies `D == k_eff - 1`, NOT `D == k_eff`. The comparison `>= k_eff` is therefore wrong for the hybrid case — it requires the self-partition to also have "departed" for the branch to fire, but the self-partition cannot depart per R3a (`SelfPartitionPanic → Error`, not elastic-departure path). The hybrid R26a branch is unreachable under correct accounting.

**Before:**
```rust
if departing_worker_ids.len() >= k_eff {
    // ... both branches return Fatal
}
```

**After (R26a hybrid + corrected accounting):**
```rust
let remote_count_before = streams_to_poll.len() - if grid_config.hybrid_coordinator { 1 } else { 0 };
let all_remotes_departed = departing_worker_ids.iter()
    .filter(|&&id| id != 0)  // self is id 0 in hybrid mode (R7a)
    .count() == remote_count_before;

if all_remotes_departed {
    if grid_config.hybrid_coordinator {
        tracing::warn!(D = departing_worker_ids.len(), k_eff, "R26a-hybrid: all remotes departed; entering SoloReducing");
        // Drop departed streams; queue reclaimed partitions; do NOT return Err.
        // Solo loop on next iteration will handle current_net via R5.
        worker_streams.clear();  // all remotes gone
        // Fall through: the `worker_streams.is_empty()` check at the top of the
        // next loop iteration triggers SoloReducing per R5/R5a.
    } else {
        return Err(ProtocolError::Fatal(format!("R26a-non-hybrid: D={}, K_eff={}, no executor remains", ...)));
    }
}
```

**Why:** R26a is the spec's narrowly-defensive case — its purpose is to prevent deadlock on `FinalStateRequest` to an empty recipient set. Aborting in the hybrid branch defeats that purpose: if a hybrid coordinator with one self-partition and three remotes loses all three remotes, it should continue solo, not abort. The current code makes hybrid mode strictly *less* resilient than the spec promises.

---

#### MF-003 — Reclaim caller passes a single overlapping `IdRange{start:0, end:100_000}` for every departed worker, violating R18/R24d D3-elastic disjointness

**Category:** Spec Violation (SPEC-20 R18, R24d, R30; D3-elastic invariant)
**Principle/Spec:** SPEC-20 §3.3.4 R24d (border_id rebase, fresh disjoint ranges); R30 (ID uniqueness); D3-elastic
**File:** `relativist-core/src/protocol/coordinator.rs:801-810`

**Problem:** The reclaim path constructs `reclaimed_id_ranges` as follows:

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
```

Every departed worker gets the **same** `IdRange { start: 0, end: 100_000 }`. If two workers depart in the same round (R26 multi-departure case), both reclaimed partitions are remapped to overlapping ranges — directly violating the R24d acceptance criterion (no overlap between reclaimed partitions). The defensive `assert!` in `materialize_reclaimed_partitions` (lines 32-39) catches this in `debug_assertions`, but only because the second iteration's `new_range` violates the loop invariant; in release builds the assertion is compiled out and the overlap silently corrupts the reconstructed `Net`.

Worse, `start: 0, end: 100_000` is hard-coded and bears no relationship to `compute_id_ranges(K_eff_new)` (the spec's required source of fresh ranges per R30). The IDs in this range will collide with surviving partitions' agent IDs (most fixtures live in `[0, 100)`), violating D4 (ID Uniqueness) on every reclaim.

**Before:**
```rust
let mut reclaimed_id_ranges = std::collections::HashMap::new();
for &id in &departing_worker_ids {
    reclaimed_id_ranges.insert(id, crate::partition::IdRange { start: 0, end: 100_000 }); // placeholder
}
```

**After (required minimum):**
```rust
// R30: fresh, disjoint ID ranges from compute_id_ranges(K_eff_new).
let k_eff_new = (k_eff - departing_worker_ids.len()) as u32;
let max_existing_id = surviving_partitions.iter()
    .map(|p| p.id_range.end)
    .max()
    .unwrap_or(0);
let mut reclaimed_id_ranges = std::collections::HashMap::new();
let mut cursor = max_existing_id;
let chunk_size = ...; // derived from average partition size or compute_id_ranges
for &id in &departing_worker_ids {
    let start = cursor;
    let end = cursor.saturating_add(chunk_size);
    cursor = end;
    reclaimed_id_ranges.insert(id, crate::partition::IdRange { start, end });
}
```

Or, idiomatically, drive ranges through `PartitionPlan::compute_id_ranges(K_eff_new)` and use the indices `K_eff_old - D .. K_eff_new` for the reclaimed slots.

**Why:** R24d/R30 are the spec's defenses for D3 (Border Completeness) and D4 (ID Uniqueness) under at-least-once semantics. The placeholder ranges undermine both: every reclaim is observably wrong on a multi-worker departure, and even a single-worker departure produces colliding IDs with the surviving net.

---

### Must-Fix (HIGH)

#### MF-004 — Production `unwrap()` on `self_partition.as_ref().unwrap()` (line 611) violates project standard

**Category:** Code Quality / Project standard
**Principle/Spec:** Project CLAUDE.md "No `unwrap()` in production code — use `?` or explicit error handling"
**File:** `relativist-core/src/protocol/coordinator.rs:610-617`

**Problem:**

```rust
if let Some(ref mut h) = self_handle {
    let p = self_partition.as_ref().unwrap();  // production unwrap
    let msg = Message::AssignPartition { round: metrics.rounds, partition: p.clone() };
    bytes_sent += send_frame(&mut h.stream, &msg).await?;
}
```

The invariant `self_handle.is_some() ⇒ self_partition.is_some()` holds (lines 580-584 establish it), but the type system does not encode it. The `unwrap()` is a project-rule violation regardless of whether it can panic in practice. This is also the only `unwrap()` in production paths in `coordinator.rs` (the rest are in `#[cfg(test)]` blocks).

**Before:**
```rust
if let Some(ref mut h) = self_handle {
    let p = self_partition.as_ref().unwrap();
    // ...
}
```

**After:**
```rust
if let (Some(ref mut h), Some(ref p)) = (self_handle.as_mut(), self_partition.as_ref()) {
    let msg = Message::AssignPartition { round: metrics.rounds, partition: p.clone() };
    bytes_sent += send_frame(&mut h.stream, &msg).await?;
} else if self_handle.is_some() {
    // unreachable per construction but explicit for readers
    return Err(ProtocolError::Fatal("self_handle is Some but self_partition is None".into()));
}
```

Or refactor `self_handle` to carry the `Partition` directly so the second `Option` disappears.

**Why:** The "no `unwrap()` in production" rule is a hard project standard. This site predates Phase D but is touched by the Wave 1 commit (the surrounding code was refactored), so the bundle is responsible for either fixing it or explicitly noting the QA carry-over.

---

#### MF-005 — `LeaveRequest{AfterResult}` handler does NOT implement R22a/R22c semantics; coordinator unconditionally treats every leave like a timeout

**Category:** Spec Violation (SPEC-20 R22a, R22b, R22c)
**Principle/Spec:** SPEC-20 §3.3.2 R22a (clean leave: store result + remove for next round), R22b (urgent leave: reclaim + remove now), R22c (silent upgrade if no result received)
**File:** `relativist-core/src/protocol/coordinator.rs:672-683`

**Problem:** The `Message::LeaveRequest { kind }` arm pushes `wid` into `departing_worker_ids` regardless of `kind`:

```rust
Message::LeaveRequest { kind } => {
    tracing::warn!(worker_id = wid, round = metrics.rounds, ?kind,
        "Worker left gracefully via LeaveRequest (R28)");
    let _ = send_frame(stream, &Message::LeaveAck).await;
    departing_worker_ids.push(wid);
    round_departed_count += 1;
}
```

Three distinct spec-mandated paths collapse into one:
1. **R22a (`AfterResult`, result already stored):** must `StoreResult(id, prev) + RemoveWorkerForNextRound(id)` — the result is **kept**, only the worker is removed for the *next* round. Current code drops the result.
2. **R22b (`Urgent`):** must `ReclaimPartition(id, auto) + RemoveWorker(id)` — the worker's partition is reclaimed for the *current* round.
3. **R22c (`AfterResult`, no result yet received):** must silently upgrade to `Urgent` and log `WARN`. Current code does not check whether a result was already received.

The `kind` field is destructured but never branched on. `LeaveAck` is sent BEFORE the result is processed — but in the `AfterResult+result-already-received` path, the result should be merged with surviving partitions, not reclaimed.

Additionally, `LeaveRequest` is received in the same recv loop as `PartitionResult`. A worker that follows the spec (send `PartitionResult` then `LeaveRequest`) sends two messages on the same stream; the current loop reads one frame per stream (`recv_frame` once per `(wid, stream)` tuple) and only processes whichever arrives first. If the worker sends `PartitionResult` first, the LeaveRequest is **never read** — it's left buffered on the stream. If the worker sends `LeaveRequest` first (R22a violation but the spec says coordinator must be lenient per R22c), the result is discarded.

**Before:**
```rust
Message::LeaveRequest { kind } => {
    tracing::warn!(worker_id = wid, round = metrics.rounds, ?kind,
        "Worker left gracefully via LeaveRequest (R28)");
    let _ = send_frame(stream, &Message::LeaveAck).await;
    departing_worker_ids.push(wid);
    round_departed_count += 1;
}
```

**After (sketch — full implementation requires the per-stream message order resolution above):**
```rust
Message::LeaveRequest { kind } => {
    let _ = send_frame(stream, &Message::LeaveAck).await;  // R35a
    let result_already_received = collect_results_vec.iter().any(|(p, _)| p.worker_id == wid);
    match kind {
        LeaveKind::AfterResult if result_already_received => {
            // R22a: keep result, remove for next round only.
            tracing::warn!(worker_id = wid, round = metrics.rounds,
                "Worker left clean (R22a); result retained for current round");
            // Mark for next-round removal but do NOT push to departing_worker_ids.
            workers_to_remove_after_round.push(wid);
        }
        LeaveKind::AfterResult => {
            // R22c: silent upgrade to urgent.
            tracing::warn!(worker_id = wid, round = metrics.rounds,
                "Worker sent AfterResult without result; upgrading to Urgent (R22c)");
            departing_worker_ids.push(wid);
            round_departed_count += 1;
        }
        LeaveKind::Urgent => {
            // R22b: reclaim partition for current round.
            tracing::warn!(worker_id = wid, round = metrics.rounds, "Worker urgent leave (R22b)");
            departing_worker_ids.push(wid);
            round_departed_count += 1;
        }
    }
}
```

The R22a-with-pre-recv-result case additionally requires that the recv loop be restructured to pull *both* messages off the stream (or that workers are required to send `LeaveRequest` *before* `PartitionResult` and the coordinator separately recognizes the result on a second recv).

**Why:** R22a is the entire reason `LeaveKind` exists — it distinguishes the "I'm done with this round, please retain my result and remove me for future" path from "I crashed, please reclaim". Collapsing both into the reclaim path discards a worker's completed work on every clean leave, violating the at-least-once + correctness contract (a clean-leave worker's result is *exactly-once*; R22a is the path that preserves that property). The current code makes clean leaves indistinguishable from urgent leaves, defeating R22a's purpose.

---

#### MF-006 — Bundle ships zero unit tests for new modules and zero integration tests for the 19 EG-* test specs the task contracts forward-reference

**Category:** Test Quality / Bundle completeness
**Principle/Spec:** All six task contracts (TASK-0438..0443) list test acceptance criteria; SPEC-20 §7 forward-references EG-U7/U7a/U7b/U7c/U8/U9/U10/U10a/U10b/U10c/U13/U19/I3/I3-delta/I5a/I5b/P2/P5/P6.
**File:** all three Phase D code files; `relativist-core/tests/`

**Problem:**
- `relativist-core/src/protocol/retained.rs` — 0 `#[test]` functions.
- `relativist-core/src/partition/departure_recovery.rs` — 0 `#[test]` functions.
- `relativist-core/src/protocol/coordinator.rs` — 9 `#[test]`/`#[tokio::test]` functions, **all pre-existing handshake/timeout tests** (they predate Phase D; see `coordinator_rejects_v1_worker_with_register_nack`, `qa_probe_5_v0_register_rejected_with_canonical_nack`, etc.). None of them exercise `LeaveRequest`/`LeaveAck`, `RetainedStateRegistry`, `materialize_reclaimed_partitions`, the `D == K_eff` branch, or the reclaim path.
- `relativist-core/tests/` — directory listing shows `cli_integration.rs`, `net_union.rs`, `partition_amendments.rs`. No file matches `departure*`, `leave*`, `retained*`, `elastic*`. None of EG-U7..U19 have a corresponding test file.

The Wave 1, Wave 2, Wave 3 commit messages all report "Default lib: 1256 → 1256" — the bundle adds zero tests. The "structural" framing in the commits is honest, but it does not match the task contracts:
- TASK-0438 acceptance: "Test Expectations: EG-I3, EG-I3-delta, EG-U7" — none present.
- TASK-0439 acceptance: "Test Expectations: EG-U7b, EG-U7c, EG-U13" — none present.
- TASK-0440 acceptance: "Test Expectations: EG-U7, U7a, U7b, U8, U10a, U12, I3, I5a, I5b" — none present.
- TASK-0441 acceptance: "Test Expectations: EG-U10, U10a, U10b, U10c, U19" — none present.
- TASK-0442 acceptance: "Test Expectations: EG-U9 BOTH branches" — none present.
- TASK-0443 acceptance: "Test Expectations: EG-U7c, U10b, I3-delta, P5, P6" — none present.

The new types are also not exercised at the unit level. `RetainedStateRegistry::refresh_last_acked` has a `debug_assert!` for D5 violation but no test that checks it triggers; `materialize_reclaimed_partitions` has a defensive overlap-detection assertion but no test that checks `Err(InvariantViolation)` when no range is provided.

**Before (current):** 1256 tests at the start of the bundle, 1256 tests after.

**After (required minimum):** Each task contract's "Test Expectations" must be addressed before Stage 5 QA. At minimum, unit tests in the new modules:

```rust
// retained.rs
#[test]
fn retained_state_release_worker_removes_both_slots() { ... }

#[test]
fn retained_state_refresh_last_acked_requires_initial_to_exist() { ... }
// (with #[should_panic] for the debug-assert path)

#[test]
fn retained_state_memory_bounds_assertion_2k_eff() { ... }

// departure_recovery.rs
#[test]
fn materialize_returns_err_on_missing_retained_state() { ... }

#[test]
fn materialize_returns_err_on_missing_id_range() { ... }

#[test]
fn materialize_remaps_id_range_correctly_single_worker() { ... }

#[test]
#[should_panic(expected = "R24d violated")]
fn materialize_panics_on_overlapping_ranges_in_debug() { ... }
```

Plus at least one end-to-end integration test exercising `run_coordinator` with `elastic_departure = true` and a worker that drops its connection mid-round (EG-U7-skeleton) — even if the test is `#[ignore]`'d pending MF-001 resolution, it documents the expected wire behavior.

**Why:** Stage 4 reviews and Stage 5 QA both presuppose that the developer has produced executable acceptance evidence. With zero tests, QA has no fixed point of behavior to attack — every adversarial probe will find the `Fatal` return at line 832 and stop. The bundle is structurally untestable in its current state. The 690 v1 floor is preserved only because v1 tests do not exercise `elastic_departure = true`.

---

### Must-Fix (MEDIUM)

#### MF-007 — `RetainedLastAcked::DeltaLight { placeholder: String }` is a serializable stub on the wire-eligible enum

**Category:** Architecture / Spec Mismatch (SPEC-20 R23c-delta)
**Principle/Spec:** SPEC-20 R23c-delta defines `retained_last_acked[w] = (border_graph_snapshot: BorderGraph, last_round_result: RoundResult)`; TASK-0439 specifies `DeltaLight { snapshot: BorderGraphSnapshot, last: RoundResult }`.
**File:** `relativist-core/src/protocol/retained.rs:31-38`

**Problem:** The `DeltaLight` variant is implemented as:

```rust
pub enum RetainedLastAcked {
    V1(Partition),
    /// Placeholder for delta-light (border_graph + last_deltas)
    DeltaLight {
        placeholder: String,
    },
    DeltaCheckpoint(Partition),
}
```

The variant carries `serde::Serialize`, `serde::Deserialize`, and (under `--features zero-copy`) `rkyv::Archive` derives. A wire-format-eligible enum carrying a literally-named `placeholder: String` field is a footgun: the type *will* round-trip through bincode and rkyv, the `String` *will* be allocated, and any future code that constructs a `DeltaLight { placeholder: "TODO".into() }` will compile-and-run silently. There is no compiler signal that this variant is unfinished.

The spec-correct variant per TASK-0439 is `DeltaLight { snapshot: BorderGraphSnapshot, last: RoundResult }`. Even if the delta path is deferred (the conservative path uses `retained_initial[w]` per R29a, which is already CLOSED), the variant should be either (a) implemented with the spec-correct fields, (b) gated behind a feature flag, or (c) marked `#[doc(hidden)]` with a `#[deprecated]` attribute that makes consumers visible.

**Before:**
```rust
DeltaLight {
    placeholder: String,
},
```

**After (option A — implement per spec):**
```rust
DeltaLight {
    snapshot: crate::merge::BorderGraph,
    last: crate::merge::RoundResult,
},
```

**After (option B — gate the unused path):**
```rust
#[cfg(feature = "delta-optimized-reclaim")]
DeltaLight {
    snapshot: crate::merge::BorderGraph,
    last: crate::merge::RoundResult,
},
```

**After (option C — fail-closed stub):**
```rust
/// SPEC-20 R23c-delta optimized path. CONDITIONAL on ARG-005.
/// Constructing this variant is unsupported until TASK-04XX lands.
#[doc(hidden)]
#[deprecated(note = "DeltaLight is not implementable until TASK-04XX; use V1 or DeltaCheckpoint")]
DeltaLight,
```

**Why:** A wire-format-eligible enum is an API surface. Shipping a `placeholder: String` variant means the wire format includes a string slot whose semantics are "we'll figure this out later" — bincode will encode it, rkyv will archive it, and any tooling that decodes `RetainedLastAcked` will see `DeltaLight { placeholder: "..." }` as a legitimate state. This locks in a wire shape that diverges from R23c-delta's specified `(BorderGraph, RoundResult)` payload. Either implement it correctly or make it impossible to construct.

---

#### MF-008 — Reclaim path uses `BorderGraph::from_partition_plan(&plan)` where `plan` is the *current* round's plan, not the *retained* snapshot

**Category:** Spec Mismatch (SPEC-20 R14, R16; D3 border completeness)
**Principle/Spec:** SPEC-20 §3.3.4 R24a/R24b implies the border graph used for `reconstruct` is the one corresponding to the retained partition state, not the round's outgoing plan
**File:** `relativist-core/src/protocol/coordinator.rs:820`

**Problem:** The reclaim path computes the border graph as:

```rust
let border_graph = BorderGraph::from_partition_plan(&plan);
let merged_net = reconstruct(&border_graph, surviving_partitions, reclaimed_partitions);
```

Where `plan` is the *just-dispatched* `PartitionPlan` from `split(current_net, k_eff, strategy)`. This is the plan whose partitions were sent to workers in the round that just timed out. The reclaimed partitions are materialized from `retained_initial`, which is a **prior** snapshot — round 0 in the conservative path. The border graph from the current round's plan does not describe the retained partitions' border structure; it describes the outgoing partitions' border structure.

For the conservative R24a path (always use `retained_initial[w]`), the retained_initial *is* the round-0 partition, and round-0's `PartitionPlan` is what produced its border IDs. Reusing the *current* round's plan introduces a spec-correctness drift: the border graph's `borders` and `next_border_id` reflect this round's split, not round 0's. The two will agree on agent IDs by accident if no border has been allocated since round 0 (which is true for the conservative path with no rejoins), but the construction is incidentally correct, not principally correct.

For the optimized R24b path (use `retained_last_acked` post-rejoin), the staleness is more serious: the retained snapshot is at round N-1 but the plan is round N, and R31's atomic refresh makes them differ in `border_graph` content.

**Before:**
```rust
let border_graph = BorderGraph::from_partition_plan(&plan);
```

**After:**
```rust
// SPEC-20 R14: use the border graph corresponding to the retained snapshot.
// For R24a (round-0): construct from the original round-0 plan, which the
// coordinator must retain alongside retained_initial[w].
// For R24b (round-N-1): use the snapshotted border_graph from retained_last_acked.
let border_graph = match active_mode {
    Mode::V1 => /* a stored round-N-1 BorderGraph or rebuilt from surviving + reclaimed partitions */,
    Mode::Delta => /* retained_last_acked[w].border_graph_snapshot */,
};
```

A simpler v1-only fix: store `last_committed_plan: Option<PartitionPlan>` in the coordinator alongside `retained_state` and use that. The current code "works" for round 0 (where outgoing plan == retained-state plan) by accident; under any non-trivial round count the border graph is silently wrong.

**Why:** R14 says "coordinator must retain departing worker's last `BorderGraph` snapshot." The current code does not retain it; it reconstructs it from the *current* outgoing plan. For round-0 departures (the only scenario this code path actually exercises today, given MF-001's premature return), the coincidence holds. For any future scenario where reclaim happens past round 0, this is silently incorrect.

---

#### MF-009 — `reconstruct` is invoked with `current_net`-derived `surviving_partitions` but those have already been **reduced** by their workers; mixing them with `retained_initial` reclaimed partitions violates D3-elastic R24c

**Category:** Spec Violation (SPEC-20 R24c — D3-elastic clean-boundary rule)
**Principle/Spec:** SPEC-20 §3.3.4 R24c — "reclaimed partition ... MUST NOT be merged with surviving partitions whose evolution diverged from the reclaimed partition's reduction trace."
**File:** `relativist-core/src/protocol/coordinator.rs:796-822`

**Problem:** The reclaim path constructs:

```rust
let surviving_partitions: Vec<Partition> =
    collect_results_vec.iter().map(|(p, _)| p.clone()).collect();
// ...
let merged_net = reconstruct(&border_graph, surviving_partitions, reclaimed_partitions);
```

`collect_results_vec` contains `PartitionResult.partition` from workers that **completed** the round — i.e., post-reduction partitions whose trace has diverged from the round-0 state. The `reclaimed_partitions` come from `retained_initial[w]` — round-0 state. These are *exactly* the two states R24c forbids merging in the same call: "[reclaimed partitions] MUST NOT be merged with surviving partitions whose evolution diverged from the reclaimed partition's reduction trace."

R24c mandates the reclaim go through a re-`split` at a clean round boundary, NOT a `reconstruct(border_graph, evolved_partitions, round_0_reclaimed_partitions)`. The current code violates the central D3-elastic invariant.

The spec-correct flow per R24a-v1 (TASK-0440 Acceptance Criteria) is:
1. `merge()` the surviving (reduced) partitions normally → `merged_net_survivors`.
2. `Net::union(merged_net_survivors, reclaimed_1).union(reclaimed_2)...` (A7).
3. `split(unioned_net, K_eff_new)`.
4. Dispatch via `AssignPartition`.

The implementation skips steps 1-2 and instead invokes `reconstruct` (which is the SPEC-19 delta-mode primitive) on a heterogeneous mix. Even leaving aside the D3-elastic rule, `reconstruct(border_graph, surviving_partitions, reclaimed_partitions)` is the SPEC-19 R38 / SPEC-20 §3.8 A8 3-arg signature whose contract is delta-mode (border-graph stays valid because deltas were applied)— it is not the v1 path.

**Before:**
```rust
let surviving_partitions: Vec<Partition> = collect_results_vec.iter().map(|(p, _)| p.clone()).collect();
// ...
let merged_net = reconstruct(&border_graph, surviving_partitions, reclaimed_partitions);
current_net = merged_net;
```

**After (v1 path per R24a-v1):**
```rust
// 1. Standard merge() of surviving partitions.
let surviving_plan = PartitionPlan {
    partitions: surviving_partitions,
    borders: plan.borders.clone(),
    next_border_id: plan.next_border_id,
};
let (merged_survivors, _border_redex_count) = merge(surviving_plan);

// 2. Union reclaimed partitions in (Net::union per A7).
let mut unioned = merged_survivors;
for reclaimed in reclaimed_partitions {
    let reclaimed_net = build_net_from_partition(&reclaimed);
    unioned = unioned.union(reclaimed_net);
}

// 3. Re-split for K_eff_new at next round's split() call (continue loop).
current_net = unioned;
// (re-split happens at the top of the next loop iteration via split(current_net, K_eff_new, strategy))
```

**Why:** R24c is the foundational D3-elastic rule. The current implementation is doing exactly what the rule forbids: merging (via `reconstruct`) survivor partitions at round N with reclaimed partitions at round 0. The fact that `reconstruct` produces a `Net` is incidental — the result is not spec-valid because the input partitions have incompatible reduction traces. ARG-006's mixed-trace recoverability proof (P10+P11+P12) explicitly requires the **clean-boundary** condition, which this code violates.

---

### Should-Fix

#### SF-001 — `let _ = _round_reclaimed_initial;` (line 829) is a discard-of-discard, not a documented gap marker

**Category:** Code Quality (suppress-warnings idiom misuse)
**File:** `relativist-core/src/protocol/coordinator.rs:622, 828-829`

**Problem:** Line 622 declares `let mut _round_reclaimed_initial = 0;` (the leading underscore already signals "unused-OK"). Line 828 increments it. Line 829 then runs `let _ = _round_reclaimed_initial;` to suppress the warning *again*. This is a double-suppression: the leading underscore on the binding is sufficient.

If the intent is to mark "this variable will be used by TASK-04XX-followup", the idiomatic form is:

```rust
// TODO(TASK-04XX-followup): record this in metrics.retained_initial_reclaims_per_round.
let _ = _round_reclaimed_initial;  // currently dead; see comment above.
```

Currently there is no comment explaining why the discard exists. A future reader will see the increment and the discard and have no signal that the dead-code is intentional. Worse, line 841 *does* push `_round_reclaimed_initial` into `metrics.retained_initial_reclaims_per_round` — but that line is unreachable on the recovery path because of MF-001's early return, so the increment is genuinely dead in all departure scenarios.

**Recommended fix:** Either remove the discard (the leading underscore on the binding is enough) and add a comment marker, or document the deferral with an issue link. As part of MF-001's fix, this becomes naturally resolved when the early return is removed and line 841 starts being reached.

---

#### SF-002 — `surviving_partitions` is built via `iter().map().collect()` followed by `partitions_iter.collect()`, both of which clone

**Category:** Code Quality (unnecessary cloning)
**File:** `relativist-core/src/protocol/coordinator.rs:572-578, 796-797`

**Problem:** `let mut partitions_iter = plan.partitions.iter().cloned()` (line 572) clones the entire partition vec into an iterator. Then line 578 collects again: `let remote_partitions: Vec<Partition> = partitions_iter.collect()`. This is two consecutive clones. Later, line 796-797 does a third clone for `surviving_partitions`.

For TCC-scope grids (K_eff ≤ 8) this is negligible, but the pattern is a readability/idiom issue — the iter-then-collect-then-collect chain obscures that we're moving the partitions out of `plan.partitions`. Idiomatic: `let mut iter = plan.partitions.into_iter();` (consumes plan.partitions, no clone). The code currently relies on `plan` being usable later for `merge_plan.borders = plan.borders` (line 864), so a full move is not directly possible — but `std::mem::take` or splitting `plan` into its pieces upfront would avoid the per-partition clone.

**Recommended fix:** Restructure the partition-handover to avoid double-cloning, or document why `iter().cloned()` is used (e.g., "kept here so plan can be reused for merge_plan.borders below").

---

#### SF-003 — `streams_to_poll: Vec<(WorkerId, &mut TransportStream)>` derives `WorkerId` from index + hybrid offset, not from a stored mapping

**Category:** Architecture (primitive obsession / temporal coupling)
**File:** `relativist-core/src/protocol/coordinator.rs:626-643`

**Problem:** The map from stream-index to `WorkerId` is computed inline via `(i as u32) + offset`:

```rust
let id = if grid_config.hybrid_coordinator { (i as u32) + 1 } else { i as u32 };
```

This works only if `worker_streams[i]` corresponds to the worker registered at index `i` AND no worker has been pruned (R26 multi-departure does not yet prune, so the mapping is stable for now). When MF-001 is fixed and stream-pruning lands, the index will no longer correspond to the original `WorkerId` — this comment-as-documentation ("Simplified: remotes are 1..N if hybrid, 0..N-1 if not.") becomes a latent bug.

The right shape is a sidecar `Vec<WorkerId>` that tracks each stream's identity:

```rust
struct WorkerStreams {
    streams: Vec<TransportStream>,
    ids: Vec<WorkerId>,
}
```

Or a `BTreeMap<WorkerId, TransportStream>`. The current shape forces the same arithmetic at every poll site, and any future divergence (joins, departures) silently corrupts the mapping.

**Recommended fix:** Refactor `worker_streams` to either `Vec<(WorkerId, TransportStream)>` or a dedicated struct. This is a Phase-D-touching prerequisite for any correct implementation of MF-001's stream-pruning step.

---

#### SF-004 — `R28` log key is `kind` (Debug-formatted) for graceful leave but a string for connection-loss/timeout — log key inconsistency

**Category:** Code Quality / Observability
**Principle/Spec:** SPEC-20 R28 — log fields should include departure type (`timeout`, `connection_loss`, `leave_after_result`, `leave_urgent`)
**File:** `relativist-core/src/protocol/coordinator.rs:677, 706, 731, 750`

**Problem:** The four R28 log sites use four different shapes:
- Line 677: `?kind` (Debug-formatted `LeaveKind`) — log shows `kind=AfterResult` or `kind=Urgent`.
- Line 706: `error = description` — log shows error string but no `departure_type`.
- Line 731: `error = e.to_string()` — same.
- Line 750: only `worker_id, round` — no departure_type at all.

R28 explicitly enumerates four departure type values: `timeout`, `connection_loss`, `leave_after_result`, `leave_urgent`. The current logs would yield:
- timeout: `(no departure_type field)`
- connection_loss: `error="..."`
- leave_after_result: `kind=AfterResult`
- leave_urgent: `kind=Urgent`

Log-aggregation tooling cannot key on `departure_type` because the field shape is non-uniform. This is functionally correct (the events fire) but architecturally inconsistent.

**Recommended fix:** Add a uniform `departure_type` field to all four sites:

```rust
tracing::warn!(worker_id = wid, round = metrics.rounds, departure_type = "timeout", ...);
tracing::warn!(worker_id = wid, round = metrics.rounds, departure_type = "connection_loss", error = ..., ...);
tracing::warn!(worker_id = wid, round = metrics.rounds, departure_type = "leave_after_result", ...);
tracing::warn!(worker_id = wid, round = metrics.rounds, departure_type = "leave_urgent", ...);
```

---

#### SF-005 — `RetainedInitial::Delta` and `RetainedInitial::V1` carry the same `Partition` payload with no behavioral distinction

**Category:** Architecture (variant proliferation without justification)
**File:** `relativist-core/src/protocol/retained.rs:12-23`

**Problem:**
```rust
pub enum RetainedInitial {
    V1(Partition),
    Delta(Partition),
}

impl RetainedInitial {
    pub fn partition(&self) -> &Partition {
        match self {
            Self::V1(p) | Self::Delta(p) => p,
        }
    }
}
```

The two variants are payload-identical and the only public method treats them as the same. The spec (R23b-v1, R23b-delta) does specify the variants exist for *semantic* reasons (the delta one comes from `InitialPartition.partition`, the v1 one from `AssignPartition`), but at the type level they are indistinguishable. Three options:

1. Collapse to a single variant (`pub struct RetainedInitial(Partition)`) and recover the v1/delta distinction from elsewhere (e.g., `GridConfig.delta_mode`).
2. Keep both variants but make the distinction useful (e.g., `Delta` carries an additional field — perhaps `originating_round: u32` for telemetry).
3. Document why the variants are kept distinct despite identical payloads (e.g., "future-proofing for R23c-delta optimized path").

The current state is a code-smell: branching on `match` over identical payloads is dead branching. If the spec-cited reason is "reserve for future divergence", a doc comment makes that explicit; otherwise it's variant pollution.

**Recommended fix:** Either collapse or document.

---

#### SF-006 — `process_join_request` passes a `Message::JoinAck { worker_id, partition_index, next_round_number }` that doesn't match SPEC-20 R35's `assigned_worker_id` field name

**Category:** Spec Mismatch (minor — variable naming)
**File:** `relativist-core/src/protocol/coordinator.rs:194-198`

**Problem:** SPEC-20 R35 specifies `JoinAck { assigned_worker_id: WorkerId, partition_index: u32, next_round_number: u32 }`. The implementation uses `worker_id` instead of `assigned_worker_id`:

```rust
let ack = Message::JoinAck {
    worker_id,  // SPEC-20 R35 names this `assigned_worker_id`
    partition_index,
    next_round_number,
};
```

This is a wire-schema deviation. `bincode` encoding of `JoinAck` fields is positional, not named, so the wire format is unaffected — but Rust struct-field syntax is named, so any external code constructing a `JoinAck` will use whatever name the `Message` enum declares (presumably `worker_id`, given this code compiles). The drift is in `protocol/types.rs`, not here, but it surfaces here.

**Recommended fix:** Either align the field name in `protocol/types.rs` to match the spec (`assigned_worker_id`) and update this site, or note the deviation in the spec amendment for SPEC-20 R35.

---

### Nice-to-Have

#### NTH-001 — `RetainedStateRegistry` is `Default` but has no `Default` test demonstrating empty bounds satisfy `assert_memory_bounds(0)`

Add `#[test] fn empty_registry_passes_zero_k_eff_bounds() { RetainedStateRegistry::default().assert_memory_bounds(0); }` — it documents and exercises the boundary case in 3 lines.

#### NTH-002 — `materialize_reclaimed_partitions` log strings are inconsistent ("State loss occurred!" vs "skipping reclaim.")

Line 47 reads `"No remapped ID range available for departed worker; skipping reclaim."` but the function then `return Err(...)` — it's NOT skipping; it's failing. Tighten to `"No remapped ID range available for departed worker; failing reclaim with InvariantViolation."`.

#### NTH-003 — `let mut _round_reclaimed_initial = 0;` could be `i32`-typed via inference, but `metrics.retained_initial_reclaims_per_round` is `Vec<u32>` — make the binding explicit

Line 622: `let mut _round_reclaimed_initial: u32 = 0;` so the `as u32` on line 828 is redundant.

#### NTH-004 — `RetainedLastAcked::DeltaCheckpoint(Partition)` has no docstring

Add a one-line `///` explaining when this variant is constructed (when `GridConfig.checkpoint_partitions = true`, per R23c-delta).

---

## 3. Passed Checks

- [x] No `unsafe` blocks (verified; `safe-rust-only-audit` baseline preserved)
- [x] No `println!` — `tracing` macros only (`error!`, `warn!`, `info!`)
- [x] `thiserror` errors used (`ProtocolError`, `PartitionError` — no `anyhow`)
- [x] `serde::Serialize`/`Deserialize` derives present on `RetainedInitial`, `RetainedLastAcked`
- [x] `rkyv::Archive`/`rkyv::Serialize`/`rkyv::Deserialize` derives correctly gated behind `#[cfg(feature = "zero-copy")]`
- [x] `RetainedStateRegistry` derives `Debug, Clone, Default`
- [x] `RetainedStateRegistry::release_worker` removes both slots (R23a)
- [x] `RetainedStateRegistry::refresh_last_acked` carries debug-assert for D5 invariant
- [x] `RetainedStateRegistry::assert_memory_bounds(k_eff)` enforces `2*k_eff` and `k_eff` bounds (R31, NF-011)
- [x] `materialize_reclaimed_partitions` calls `remap_partition_ids` for each reclaimed (R24d)
- [x] `materialize_reclaimed_partitions` includes a defensive `assert!` for R24d range disjointness in `#[cfg(debug_assertions)]`
- [x] `materialize_reclaimed_partitions` uses `tracing::error!` + `Err(InvariantViolation)` on missing-state, not a panic
- [x] `handle_connection_loss` and `handle_phase_timeout` correctly branch on `elastic_departure` and emit structured `tracing::warn!`
- [x] `Message::LeaveAck` is sent before the worker stream is dropped (line 680 — partial R35a, see MF-005 for the missing R22a path)
- [x] `process_join_request` correctly handles `JoinNackReason::ProtocolVersionMismatch`, `ElasticJoinDisabled`, `WorkerIdSpaceExhausted`, `AuthenticationFailed`
- [x] `accept_workers` distinguishes `Register` (initial) from `JoinRequest` (mid-session) per R37a
- [x] `PROTOCOL_VERSION = 4` per R37
- [x] Module boundaries respected: `retained.rs` lives in `protocol/`, depends on `partition::{Partition, WorkerId}`, no async/tokio in the file itself; `departure_recovery.rs` lives in `partition/`, depends on `protocol::retained::RetainedStateRegistry` (acceptable given retained is a coordinator-side concept; see SF-007 below for the alternative)
- [x] Wire-format additions (`LeaveRequest`, `LeaveAck`, `JoinAck`, `JoinNack`, `JoinRequest`) are in `protocol/types.rs` and are serializable (verified via `Message::JoinAck { ... }` construction in `process_join_request`)
- [x] R28 WARN logs fire on all three departure types (graceful-Leave, error-Disconnect, timeout) — see SF-004 for inconsistency in field shape
- [ ] No `unwrap()` in production code (MF-004: line 611 violation)
- [ ] R18 timeout → recovery path (MF-001: structurally short-circuited by line 832)
- [ ] R26a hybrid → SoloReducing fallback (MF-002: aborts with Fatal instead)
- [ ] R24c D3-elastic clean-boundary rule (MF-009: reconstruct mixes evolved survivors with round-0 reclaimed)
- [ ] R24d/R30 ID range disjointness (MF-003: hard-coded `IdRange{0, 100_000}` for every reclaimed worker)
- [ ] R22a/R22c clean-leave semantics (MF-005: not implemented)
- [ ] R14 retained `BorderGraph` snapshot used in reconstruct (MF-008: uses current-round plan instead)
- [ ] R23c-delta `DeltaLight` payload shape (MF-007: stub `placeholder: String`)
- [ ] Test coverage vs 6 task contracts (MF-006: zero new tests)

---

## 4. Architecture Assessment

### Module Boundaries (SPEC-13)

The new `protocol/retained.rs` and `partition/departure_recovery.rs` modules respect the layering:

- `retained.rs` lives in `protocol/` and depends on `partition::{Partition, WorkerId}`. It is pure (no tokio/async), which is appropriate even though it lives under `protocol/`. Justification: it is the *coordinator's* state, not a wire type, and putting it in `partition/` would force `partition/` to know about `WorkerId → Partition` mappings which are protocol-level. The current placement is defensible.
- `departure_recovery.rs` lives in `partition/` but imports `protocol::retained::RetainedStateRegistry`. This **inverts** the canonical dependency direction (`partition <- merge <- protocol`). The module's docstring says "partition materialization (SPEC-20 §3.3.4)" — materialization is a partition concern, but the *registry* is a protocol concern, so importing the registry pulls protocol up into partition. Two cleaner options:
  1. Move `materialize_reclaimed_partitions` out of `partition/` into `protocol/coordinator/recovery.rs` (or similar), keeping it co-located with the registry it consumes.
  2. Refactor the function to accept a generic `&dyn HasInitialPartition` trait instead of `&RetainedStateRegistry`, so `partition/` doesn't need to import `protocol/`.

The current arrangement compiles only because `partition/` is below `protocol/` and the import goes "downward" — but that's an architectural smell. **SHOULD-FIX:** consider relocating `departure_recovery.rs` to `protocol/`.

### Anti-Patterns

- **Primitive obsession:** `IdRange { start: 0, end: 100_000 }` placeholder (MF-003) — bare numeric ranges where a typed allocator output is required.
- **Temporal coupling:** stream-index-to-WorkerId mapping computed inline via `(i as u32) + offset` (SF-003) — works only if the vec is never mutated post-Register.
- **Stub on a wire enum:** `DeltaLight { placeholder: String }` (MF-007).
- **God function:** `run_coordinator` is now ~510 lines (lines 455-967), with the per-round body itself being ~400 lines and the recovery block in the middle. SHOULD-FIX: the recovery block (lines 766-833) is a natural extract candidate. After Phase D's MF-001/MF-002/MF-003/MF-009 fixes land, the function will exceed 600 lines and become structurally unreviewable. Pre-emptive extraction recommended.
- **Dead code:** `let _ = _round_reclaimed_initial;` (SF-001), and the `partitions_redispatched_per_round.push(0)` placeholder on line 845 (unreachable on the recovery path due to MF-001).

### Wire-Serde Regression Assessment

`RetainedInitial`, `RetainedLastAcked`, `RetainedStateRegistry` derive `Serialize/Deserialize` (and `rkyv` archive variants under feature gate). The registry itself is *not* on the wire (it is coordinator-internal state), but the enums are written with wire-format-eligible derives. `DeltaLight { placeholder: String }` (MF-007) introduces a wire-eligible footgun. No other wire-format issue identified.

`Message::JoinAck`, `LeaveRequest`, `LeaveAck` etc. were added in earlier waves (TASK-0418, presumably; out of Phase D scope). No regression in the Phase D edits.

---

## 5. Spec-Compliance Matrix (R13-R28 + R20-R23 retained)

| Requirement | Status | Evidence |
|-------------|:------:|----------|
| R13 (timeout + connloss detection) | PARTIAL | Helpers correct (`handle_connection_loss`, `handle_phase_timeout`); branching to recovery path is short-circuited (MF-001). |
| R14 (retain last `BorderGraph` snapshot) | NOT IMPLEMENTED | `BorderGraph` is recomputed from current plan (MF-008); no snapshot is retained. |
| R15 (D == K_eff degenerate handling) | PARTIAL | Branch detected but both arms abort; hybrid arm should fall to `SoloReducing` (MF-002). |
| R16 (delta reconstruct from retained snapshot) | DEGRADED | Conservative path uses `retained_initial`; `reconstruct` is invoked but on mixed-trace inputs (MF-009). |
| R17 (observability log) | PARTIAL | WARN logs fire; field shape inconsistent (SF-004); deferred fully to Phase E. |
| R18 (reclaim respects R24d ID-disjointness) | NOT IMPLEMENTED | Hard-coded `IdRange{0, 100_000}` for every reclaimed (MF-003). |
| R20 (`LeaveRequest` ack with `LeaveAck`) | OK | Wire-level handshake present (line 680). |
| R22a (clean leave: store + remove for next round) | NOT IMPLEMENTED | Collapsed into urgent path (MF-005). |
| R22b (urgent leave: reclaim + remove now) | OK (degraded by MF-001) | Pushed into `departing_worker_ids`; reclaim short-circuits at MF-001. |
| R22c (silent upgrade if no result) | NOT IMPLEMENTED | Branch missing (MF-005). |
| R23a (release policy) | OK | `release_worker` removes both slots. |
| R23b/c (per-mode contents) | PARTIAL | `RetainedInitial::V1` correct; `RetainedLastAcked::V1` correct; `DeltaLight` is a stub (MF-007). |
| R23d (priority `last_acked` first) | NOT EXERCISED | No call site queries `last_acked` first then falls back; `materialize_reclaimed_partitions` reads `initial` only. |
| R24a (catastrophic departure → `retained_initial`) | OK (within helper) | `materialize_reclaimed_partitions` correctly reads `registry.initial`. |
| R24b (post-round departure → `retained_last_acked`) | NOT IMPLEMENTED | No call site reads `registry.last_acked` for materialization. |
| R24c (D3-elastic clean boundary) | VIOLATED | `reconstruct` mixes survivor (evolved) with reclaimed (round-0) (MF-009). |
| R24d (border_id rebase, fresh disjoint range) | VIOLATED | Hard-coded overlapping ranges (MF-003). |
| R25 (re-partition for K_eff_new) | NOT IMPLEMENTED | Reclaim path returns `Err` before next-round split runs. |
| R26 (multiple simultaneous departures) | NOT TESTED | Code accepts multiple in `departing_worker_ids` list, but the path returns `Err` before any are processed correctly (MF-001). |
| R26a (D == K_eff edge case) | PARTIALLY IMPLEMENTED | Branch detected; both arms abort (MF-002). |
| R27 (hybrid → solo, non-hybrid → Error) | PARTIAL | Non-hybrid Error is correct; hybrid Solo is broken (MF-002). |
| R28 (WARN log on departure) | OK (with field-shape inconsistency SF-004) | All three departure types log at WARN. |
| R31 (atomic refresh; memory bounds) | OK | `refresh_last_acked` API + `assert_memory_bounds` debug-asserts present. |

---

## 6. Stage 5 QA Readiness

**Conditional — REJECT before QA dispatch.**

Stage 5 QA cannot productively probe Phase D in its current state because:

1. **MF-001** makes every departure scenario abort before recovery completes. QA's adversarial probes (drop a worker, send `LeaveRequest`, time out one stream) will all observe `Fatal("Departure recovery reconstruction succeeded but stream management is TASK-0443 follow-up")` — an artifact, not a behavior. QA will produce a single bug report ("the recovery path is unimplemented") and stop.
2. **MF-006** means there is no test infrastructure for QA to derive adversarial probes from. QA's playbook builds adversarial inputs by negating positive tests; with zero positive tests, there is no anchor.
3. **MF-002, MF-003, MF-009** would each surface as separate bug reports that QA cannot disambiguate from MF-001's blanket abort.

**Recommended path:**

- **Option A (most truthful):** revert Phase D's recovery block, ship Phase D as "detection + retained-state plumbing only" with the recovery path explicitly disabled (`elastic_departure = false` enforced by config validator). Open `TASK-0443a-recovery-path` and `TASK-0443b-stream-pruning` as P0 follow-ups. This makes the bundle's actual scope match its commits.
- **Option B (preferred per task contracts):** developer addresses MF-001..MF-009 (CRITICAL + HIGH + MEDIUM) plus MF-006 (write the EG-U7/U10/U13 unit tests at minimum) before re-dispatching to Stage 4. Estimated effort: ~3-5 days of focused work.

**Do not dispatch Phase D to Stage 5 QA in the current state.** Stage 5 will not produce useful bug reports against this code.

---

## 7. Stage 6 Action List

| # | Severity | Action | File | Lines |
|---|----------|--------|------|-------|
| 1 | CRITICAL (MF-001) | Remove the unconditional `return Err(...)` at line 832; implement stream-pruning OR explicitly disable recovery path with config-validator gate | `coordinator.rs` | 766-833 |
| 2 | CRITICAL (MF-002) | Implement R26a-hybrid SoloReducing fallback; correct the `len() >= k_eff` accounting for hybrid mode | `coordinator.rs` | 773-792 |
| 3 | CRITICAL (MF-003) | Replace `IdRange{0, 100_000}` placeholder with `compute_id_ranges(K_eff_new)`-derived disjoint ranges | `coordinator.rs` | 801-810 |
| 4 | HIGH (MF-004) | Replace `self_partition.as_ref().unwrap()` with combined `if let` pattern or refactor to eliminate the second `Option` | `coordinator.rs` | 611 |
| 5 | HIGH (MF-005) | Implement R22a/R22c branching in `Message::LeaveRequest` arm; differentiate clean-leave-with-result from urgent | `coordinator.rs` | 672-683 |
| 6 | HIGH (MF-006) | Write at minimum: 4 unit tests in `retained.rs`, 4 unit tests in `departure_recovery.rs`, 1 integration test for EG-U7 (timeout-departure-conservative) | `retained.rs`, `departure_recovery.rs`, `tests/` | (new) |
| 7 | MEDIUM (MF-007) | Replace `DeltaLight { placeholder: String }` with spec-correct `(BorderGraph, RoundResult)` payload OR feature-gate behind `delta-optimized-reclaim` | `retained.rs` | 31-38 |
| 8 | MEDIUM (MF-008) | Use a retained `BorderGraph` snapshot instead of recomputing from the current round's plan | `coordinator.rs` | 820 |
| 9 | MEDIUM (MF-009) | Replace `reconstruct(border_graph, evolved_survivors, round_0_reclaimed)` with the v1-correct flow: `merge(survivors)` → `Net::union(reclaimed)` → next-round `split` | `coordinator.rs` | 796-822 |
| 10 | LOW (SF-001) | Remove `let _ = _round_reclaimed_initial;` discard and add TODO comment if dead code is intentional | `coordinator.rs` | 829 |
| 11 | LOW (SF-002) | Restructure partition-handover to avoid double-cloning | `coordinator.rs` | 572-578, 796-797 |
| 12 | LOW (SF-003) | Replace stream-index-to-WorkerId arithmetic with a maintained `Vec<(WorkerId, TransportStream)>` or `BTreeMap` | `coordinator.rs` | 626-643 |
| 13 | LOW (SF-004) | Add uniform `departure_type` field to all four R28 WARN log sites | `coordinator.rs` | 677, 706, 731, 750 |
| 14 | LOW (SF-005) | Either collapse `RetainedInitial::V1`/`Delta` or document the non-divergent variants | `retained.rs` | 12-23 |
| 15 | LOW (SF-006) | Align `Message::JoinAck` field name with SPEC-20 R35 (`assigned_worker_id`) — tracked in `protocol/types.rs` | `protocol/types.rs`, `coordinator.rs:194-198` | (cross-ref) |
| 16 | NTH (NTH-001) | Add `default_registry_passes_zero_bounds` test | `retained.rs` | (new) |
| 17 | NTH (NTH-002) | Tighten misleading "skipping reclaim" log to "failing reclaim" | `departure_recovery.rs` | 47 |
| 18 | NTH (NTH-003) | Type-annotate `_round_reclaimed_initial: u32 = 0` | `coordinator.rs` | 622 |
| 19 | NTH (NTH-004) | Add docstring to `RetainedLastAcked::DeltaCheckpoint` | `retained.rs` | 37 |

Items 1-9 (CRITICAL + HIGH + MEDIUM) MUST be resolved before Stage 5 dispatch. Items 10-15 (LOW) SHOULD be resolved in the same pass. Items 16-19 (NTH) at developer's discretion.

---

## 8. TODO/Follow-Up Watchlist

The reviewer was asked to confirm whether `let _ = _round_reclaimed_initial;` (line 829) and `return Err(...TASK-0443 follow-up)` (line 832) are documented gaps or silent bugs masquerading as features. Verdict:

- **Line 829 (`let _ = _round_reclaimed_initial;`):** documented gap (the variable's leading underscore signals "intentionally unused"), but the documentation is the underscore alone — there is no comment. Treat as silent gap. Resolved by MF-001's metric-recording fix.
- **Line 832 (`return Err(...TASK-0443 follow-up)`):** **silent bug masquerading as a feature.** The string explicitly says "reconstruction succeeded but stream management is TASK-0443 follow-up", but **no such follow-up task exists in `docs/backlog/`** (verified: `TASK-0443-delta-departure-reconstruct-reclaim.md` is the very task this commit closes; there is no `TASK-0443a` or follow-up referenced anywhere). The `Err` therefore terminates the run with a message that points to a task that does not exist. This is the central failure of the bundle.

**Recommended action:** Either open `TASK-0443a-stream-pruning-after-departure-reclaim.md` immediately and link it from the commit message, or fold stream-pruning into MF-001's resolution.

---

## 9. Summary Statement

Phase D is structurally a placeholder bundle: detection plumbing, type surfaces, helpers, and retained-state bookkeeping are in place, but the recovery path is non-functional and untested. The bundle's commit messages accurately note "structural" framing, but the task contracts mandate functional acceptance criteria that the code does not meet (R18, R22a/c, R24c, R24d, R26a-hybrid). Three of nine Must-Fix issues (MF-001, MF-002, MF-009) are blocking spec violations; the remaining six are HIGH/MEDIUM correctness or quality issues. Test coverage is zero against six task contracts that collectively forward-reference 19 EG-* test specs.

**Phase D: 9 Must-Fix, 6 Should-Fix, verdict: REJECT — REQUIRES REWORK BEFORE STAGE 5 QA**
