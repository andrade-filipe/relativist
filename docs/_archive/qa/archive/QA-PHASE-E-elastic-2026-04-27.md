# QA Phase E — Stage 5 adversarial review of SPEC-20 Elastic Grid Observability bundle

**Date:** 2026-04-27
**QA agent:** qa (Stage 5, opus 4.7 1M)
**Bundle:** SPEC-20 Phase E (Observability) — TASK-0450, TASK-0451, TASK-0452
**Commits inspected:** `fff0f9e` (Wave 1), `a84cb37` (Wave 2)

**Files inspected (read-only):**
- `relativist-core/src/merge/types.rs` (Wave 1 — 7 new GridMetrics fields, R38a audit comment)
- `relativist-core/src/protocol/coordinator.rs` (Wave 1 — 5 R28 warn sites + R17 info site; +cargo fmt churn)
- `relativist-core/src/merge/core.rs` (Wave 2 — D3 disjointness assertion at L43-55)
- `relativist-core/src/partition/helpers.rs` (Wave 2 — D4-elastic K_eff assertion at L86-99)
- `relativist-core/src/partition/departure_recovery.rs` (Wave 2 — R24d disjointness at L28-40)
- `relativist-core/src/protocol/retained.rs` (Wave 2 — D5 precondition at L56-62)
- `docs/reviews/REVIEW-PHASE-E-elastic-2026-04-27.md` (Stage 4 ammunition: MF-001..MF-006, SF-001..SF-004)
- `docs/qa/QA-TASK-0414-2026-04-25.md` (QA precedent format)

**Upstream verdict:** Stage 4 reviewer — ACCEPT_WITH_FIXES (0 CRITICAL, 6 MEDIUM, 4 LOW). MF-001..MF-006 are treated as known issues that the developer will close in Stage 6. **This QA hunts deeper bugs that the MF-fix list will not reach.**

---

## Summary

| Severity | Count | IDs |
|----------|-------|-----|
| **CRITICAL** | 2 | QA-001, QA-002 |
| **HIGH** | 3 | QA-003, QA-004, QA-005 |
| **MEDIUM** | 5 | QA-006, QA-007, QA-008, QA-009, QA-010 |
| **LOW** | 4 | QA-011, QA-012, QA-013, QA-014 |

**Top-3 most dangerous (one line each):**

1. **QA-001 (CRITICAL)** — `metrics.merge_time_per_round.push(t_window_start.elapsed())` at `coordinator.rs:959` writes the **join-window wall-clock** into a Vec semantically reserved for structural merge timing; the join window can sleep up to `join_window_max` (500 ms default), corrupting c_o/c_r break-even data and producing benchmark output that systematically over-estimates merge cost in elastic mode. Bench code at `bench/csv.rs:68` and `bench/suite.rs:245-246` reads this field directly. **All Phase E benchmark numbers consuming `merge_time_per_round` are now scientifically invalid in elastic mode.**

2. **QA-002 (CRITICAL)** — D5 `debug_assert!` at `protocol/retained.rs:58-62` panics in debug builds whenever `refresh_last_acked` is called for a worker that joined mid-session. The hot path at `protocol/coordinator.rs:662-668` calls `refresh_last_acked(partition.worker_id, ...)` for every `PartitionResult` whenever `retain_partitions` is true; mid-session joiners have NO entry in `retained_state.initial` (the L587-600 init block only inserts for the round-0 cohort). **Any debug-build distributed run with `elastic_join=true` + `retain_partitions=true` panics on the second round after the first joiner returns its first result** — observability code panicking the coordinator in the exact mode the observability is meant to instrument.

3. **QA-003 (HIGH)** — Per-round Vec length invariant is silently violated. `effective_slots_per_round` is pushed at `coordinator.rs:561` BEFORE three `?` early-return points (L608 `distribute_partitions?`, L616 `send_frame?`, L763 `self_handle.await?`). The other six elastic counters (`workers_joined_per_round`, `workers_departed_per_round`, `partitions_redispatched_per_round`, `retained_initial_reclaims_per_round`, `retained_last_acked_reclaims_per_round`, `join_round_overhead_ms_per_round`) are pushed at L835-846 and L961. A network failure mid-distribute leaves `effective_slots_per_round.len() == metrics.rounds + 1` while the others stay at `metrics.rounds`. **No `metrics.rounds` invariant ties these together; CSV/JSON consumers indexing by round will read mismatched data.**

**CRITICAL findings requiring Stage 6 first-fix priority:**
- **QA-001** — fix the misclassified write; either move it to a new `join_window_time_per_round` field or add to a renamed Vec; **must land before any elastic-mode benchmark is run**.
- **QA-002** — debug_assert is reachable through a normal, well-formed elastic run; **must be guarded** (e.g., promote to a soft `tracing::warn!` plus skip, or insert a no-op `RetainedInitial` at join time) before any debug-build elastic test ships.

**Recommendation:** **REQUIRE Stage 6 FIXES** — at minimum QA-001, QA-002, QA-003, QA-004, QA-005 must land before bundle ships, alongside reviewer's MF-001..MF-006.

---

## Findings

### QA-001 (CRITICAL) — `merge_time_per_round` is corrupted by the join-window timer in elastic mode; SPEC-09 benchmark data is scientifically invalid

- **Severity:** CRITICAL — silent benchmark corruption; affects every elastic-mode SPEC-09 c_o/c_r measurement; touches the central break-even claim of the TCC.
- **Category:** Logic Error / Wrong Field Write / Telemetry Lying
- **Location:** `relativist-core/src/protocol/coordinator.rs:959`
- **Reproduction (static trace):**
  ```rust
  // coordinator.rs:903-960 (elastic_join branch):
  if grid_config.elastic_join {
      let t_window_start = Instant::now();           // L904
      // ... 50-500 ms of tokio::sleep + new connection accept ...
      // ... process_join_request handshakes ...
      // L959:
      metrics.merge_time_per_round.push(t_window_start.elapsed());  // <-- WRONG VEC
  }
  ```
  - `merge_time_per_round` is canonically **structural merge time** (SPEC-05 R34, populated in `merge/grid.rs:183, 213, 248, 607, 1205` and used by `bench/csv.rs:68` for the per-round merge cost extraction).
  - In elastic mode, `run_coordinator` writes the **join-window wall-clock duration** to this Vec. With `join_window_min=50ms` and `join_window_max=500ms` (defaults), a typical elastic round adds 50-500 ms of "merge time" that is in fact 100% sleep + I/O + handshake.
  - In **non-elastic** mode (`elastic_join=false`), `run_coordinator` NEVER pushes to `merge_time_per_round` (the only push is inside the L902 elastic branch). So `merge_time_per_round.len() == 0` in non-elastic mode while every other per-round Vec is `metrics.rounds`-long — Vec-length divergence on top of the misclassification.
- **Impact:**
  1. **SPEC-09 break-even (`c_o/c_r`) is broken in elastic mode.** `c_o = sum(merge_time + border_reduce_time + network_*) / sum(local_compute)`. Inflating merge_time by the join-window duration biases `c_o` upward, making elastic mode look more expensive than it is. The break-even target `c_o/c_r < 0.50 for w=2` (per ROADMAP §2.40) cannot be honestly measured.
  2. **EG-B1, EG-B2, EG-B3 elastic benchmark suites** (referenced in TEST-SPEC-0450) consume `merge_time_per_round`. They will systematically over-report merge cost.
  3. **Bench CSV consumers** at `bench/csv.rs:68` and JSON consumers at `bench/suite.rs:245-246` read the field by name with no metadata — there's no in-band signal that the elastic-mode value is polluted.
- **Why current tests miss it:** No elastic-mode benchmark assertion compares `merge_time_per_round[r]` against the actual `merge()` call duration. UT-0450-04 (per TEST-SPEC-0450) checks the *length* of new elastic Vecs, not the *semantic correctness* of `merge_time_per_round`.
- **Suggested fix:**
  1. **(Mandatory)** Remove L959. Add a new `join_window_time_per_round: Vec<Duration>` field to `GridMetrics` (parallel construction to the existing duration Vecs) and push the join-window elapsed there.
  2. **(Mandatory)** Add a separate push for actual structural merge time at L867-889 (currently the only timing push in that block is `border_reduce_time_per_round` at L889; `merge_time_per_round` is never populated in run_coordinator). The reason the current code passes `cargo test` is precisely that nothing tests the run_coordinator elastic path's `merge_time_per_round` for semantic correctness.
  3. **(Test)** Add a regression test that runs a 3-round elastic grid, records the actual wall-clock of the `merge()` call via instrumentation, and asserts `metrics.merge_time_per_round[r]` is within 10% of the instrumented value. The current test would pass against a 500 ms pollution.
- **Why not detected by reviewer:** Reviewer focused on R17/R28/R38 spec compliance and the four Wave 2 assertion sites. The L959 line is in the join window (Wave 1 territory) but the misclassification is hidden by the variable name `t_window_start` — it sounds like merge timing because of the field it writes to. Static trace required to surface.

---

### QA-002 (CRITICAL) — D5 `debug_assert!` panics the coordinator on every mid-session joiner that returns a partition result

- **Severity:** CRITICAL — debug-build coordinator panic in the central elastic flow; observability code (D5 audit) kills the process it is meant to instrument.
- **Category:** Panic Path / Assertion Reachability / Cross-Wave Interaction
- **Location:** `relativist-core/src/protocol/retained.rs:54-64` (`refresh_last_acked` D5 assertion); call site at `relativist-core/src/protocol/coordinator.rs:662-668`
- **Reproduction (static trace):**
  ```rust
  // T=0: coordinator starts with worker_streams = [W1, W2]
  // T=0: round 0 init at coordinator.rs:587-600
  //      retained_state.initial = { 1: V1(p1), 2: V1(p2) }   // R23b for round-0 cohort
  // T=1: worker W3 connects mid-session via JoinRequest in the join window
  // T=1: process_join_request at coordinator.rs:181-208 issues worker_id=3 + JoinAck
  //      worker_streams.push(stream_w3) at coordinator.rs:934
  //      W3's partition is NOT inserted into retained_state.initial
  //      (no init code for joiners exists)
  //
  // T=2: round 1 begins. partitions_iter at L572-578 includes a partition for W3.
  // T=2: distribute at L602 sends partition to W3.
  // T=2: collect at L645-758 receives PartitionResult { partition: P3, .. } from W3.
  // T=2: line 662-668:
  if grid_config.retain_partitions {
      retained_state.refresh_last_acked(
          partition.worker_id,                              // = 3
          RetainedLastAcked::V1(partition.clone()),
      );
  }
  // T=2: protocol/retained.rs:58-62
  debug_assert!(
      self.initial.contains_key(&worker_id),                // FALSE for worker_id=3
      "D5 violated: refreshing last_acked for worker {} without initial state",
      worker_id
  );
  // T=2: PANIC in debug build. Coordinator process dies.
  ```
- **Why this is reachable in a normal flow:**
  1. Mid-session join is the entire point of the `elastic_join` mode (SPEC-20 R9-R17).
  2. `retain_partitions` is auto-enabled when `elastic_departure` is true (`merge/types.rs:559-561`); `elastic_join` is auto-enabled when `elastic_departure` is true. The minimal config for full SPEC-20 use exercises both flags simultaneously.
  3. The grep at `coordinator.rs:587-600` shows ONE init site for `retained_state.initial`, gated by `retain_partitions` and operating only on `self_partition + remote_partitions` — i.e., the round-0 cohort. **No code inserts a `RetainedInitial` for mid-session joiners.**
  4. Therefore: any debug-build elastic run with at least one mid-session join + `retain_partitions=true` panics on round N+1 at the first `PartitionResult` from the new joiner. The full critical-path is L911-947 (join) -> next round L572-578 (dispatch including new joiner) -> L645-755 (collect) -> L662-668 (refresh_last_acked) -> L58-62 (panic).
- **Why release builds are also affected:**
  - `debug_assert!` is stripped, but the missing initial entry means the conservative reclaim path (`materialize_reclaimed_partitions` at `partition/departure_recovery.rs:23` `if let Some(initial) = registry.initial.get(&wid)`) will return `Err(PartitionError::InvariantViolation("State loss for worker {wid}"))` if the joiner subsequently departs. So the bug surfaces as **silent state loss in release builds** and as a **panic in debug builds** — both are critical, debug strictly more loud.
- **Why current tests miss it:**
  - No test exercises the join-window followed by a round followed by `refresh_last_acked` for the new joiner. UT-0450-13 (per TEST-SPEC-0450) is a struct-level invariant; it does not run a multi-round elastic flow.
  - Wave 2 reviewer correctly verified `D5` is "genuinely load-bearing: `refresh_last_acked` requires `initial[w]` to exist" (REVIEW-PHASE-E §3 PASSED CHECKS, line 318) — but did NOT check whether the call sites in `coordinator.rs` actually maintain that precondition for mid-session joiners.
- **Suggested fix (mandatory before bundle ships):**
  1. **At `coordinator.rs:934`** (right after `worker_streams.push(stream)` and before incrementing `round_joined_count`): insert a corresponding `retained_state.initial.insert(worker_id, RetainedInitial::V1(partition_for_next_round.clone()))` IF `retain_partitions` is true. Note: at the time of join, the joiner has NO partition yet (it gets one in round N+1 dispatch). One option is to delay the `initial` entry until after the round-N+1 partition is computed at L578 (`remote_partitions.collect()`).
  2. **(Alternative)** Relax D5 to a `tracing::warn!` rather than panic, for the case where `retain_partitions` was enabled mid-session (or for any mid-session joiner). Drop the `debug_assert!` and keep the warn.
  3. **(Test, mandatory)** Add a regression test that runs a 3-round elastic grid, lets a 4th worker join in round 1, and asserts that round 2's `refresh_last_acked` does not panic and that `retained_state.initial` contains the new worker by the time it might depart.
- **Note:** This is exactly the "observability code that lies is worse than missing observability" pattern from the QA brief. D5 was added to defend an invariant the call sites do not maintain.

---

### QA-003 (HIGH) — Per-round Vec length invariant silently violated by `?` early returns; `effective_slots_per_round` desync vs other elastic counters

- **Severity:** HIGH — telemetry consumers indexing by round see mismatched data; CSV exports become ambiguous; no in-band signal of the desync.
- **Category:** Invariant Violation / Observability Drift / Resource Cleanup on Error
- **Location:** `coordinator.rs:561` (push) vs `coordinator.rs:835-846, 961` (sibling pushes); intervening `?` operators at L608, L616, L763, L817, L832
- **Static trace:**
  ```rust
  // coordinator.rs:551-561 — START of round
  metrics.agents_per_round.push(...);                         // L552  (n+1)
  metrics.partition_time_per_round.push(...);                 // L564  (n+1)
  metrics.effective_slots_per_round.push(k_eff as u32);       // L561  (n+1)

  // L602-608: distribute_partitions(...).await? — early return possible
  // L612-617: self_handle send_frame(...).await? — early return possible
  // L645-758: collect loop (no `?`, but consumes time)
  // L760-763: self_handle.join_handle.await? — early return possible
  // L767-833: departure recovery; L832 unconditional return Err

  // L835-846 — END of round (only reached on success):
  metrics.workers_departed_per_round.push(...);               // L838
  metrics.retained_initial_reclaims_per_round.push(...);      // L841
  metrics.retained_last_acked_reclaims_per_round.push(...);   // L844
  metrics.partitions_redispatched_per_round.push(0);          // L845
  metrics.join_round_overhead_ms_per_round.push(0);           // L846

  // L961 (after merge + join window):
  metrics.workers_joined_per_round.push(round_joined_count);  // L961
  ```
- **Impact:**
  1. **A network failure or worker-stream send timeout at L608 leaves the round's metrics half-pushed.** `effective_slots_per_round` got `k_eff`; `workers_departed_per_round`, `workers_joined_per_round`, `partitions_redispatched_per_round`, `retained_*_reclaims_per_round`, `join_round_overhead_ms_per_round` are all SHORT by 1 entry.
  2. **The L832 deliberate return Err inside the reclaim path** also pushes none of the L835-846 metrics — so even on the *recovery* path the round's metrics are partial. Two failure modes, same lopsided metrics.
  3. **CSV consumers** (`bench/csv.rs`) iterate by `round` index and `unwrap_or(0.0)` — silently substituting zero where data is missing. Charts produced from this data will silently truncate the failing round.
  4. **JSON consumers** (`bench/suite.rs`) serialize the Vecs verbatim. Any downstream analysis assuming `len(workers_joined) == len(effective_slots)` is broken.
- **No invariant ties them.** There is no `assert_eq!(metrics.workers_joined_per_round.len(), metrics.rounds)` anywhere. The `merge_time_per_round` field has the same vulnerability today (QA-001 exposes its corner case).
- **Why reviewer missed:** Reviewer's MF-004 noted that `_round_reclaimed_initial` is unreachable in the L832 path, but did not extend that observation to the wider observation: **every Vec push at L835-846 is unreachable on the L832 early return**. Combined with QA-001's analysis of L959, the per-round invariant is not maintained on at least 4 distinct error paths.
- **Suggested fix:**
  1. **(Strongest)** Replace the per-Vec pushes with a single `push_round_metrics(metrics, round_data)` helper that takes a `RoundMetrics { workers_joined, workers_departed, ... }` struct and pushes all 7+ Vecs in lockstep. Call it once in a `defer`-style block (or restructure the round loop to push at a single bottom-of-loop site). This makes the invariant `len(all per-round Vecs) == metrics.rounds + 1` defensible by construction.
  2. **(Compromise)** After every `?` early return point in the loop, emit a "best-effort" partial-round push for each missing Vec (zero or `Option::None` sentinel, depending on type). Or wrap the round body in a `tokio::select!` + RAII guard that on drop pushes the missing entries.
  3. **(Audit)** Add a debug invariant at the top of every loop iteration: `debug_assert_eq!(metrics.effective_slots_per_round.len(), metrics.workers_joined_per_round.len())` — this would have surfaced QA-001's `merge_time_per_round` desync immediately.
  4. **(Test)** Add a unit test that injects a network failure at L608 and asserts that all 7 elastic Vecs have the same length after coordinator returns Err.

---

### QA-004 (HIGH) — Duplicate / inconsistent R17 INFO log: one structured at coordinator.rs:940, one unstructured at coordinator.rs:201; log scrapers see two events per join

- **Severity:** HIGH — observability cardinality doubled; log analyzers indexing `event=Worker joined the grid (R17)` will count joins as 2x; SPEC-11 OTel mapping will see two structurally-different events for the same R17 trigger.
- **Category:** Spec Violation (logging cardinality) / Cross-File Inconsistency
- **Location:**
  - `coordinator.rs:201-206`: `tracing::info!("Worker joined: id={}, partition_index={}, next_round={}", ...)` — UNSTRUCTURED format string, fired inside `process_join_request`
  - `coordinator.rs:940-945`: `tracing::info!(worker_id, k_eff_new, round, "Worker joined the grid (R17)")` — STRUCTURED, fired in the join window AFTER `process_join_request` returns
- **Static trace:**
  ```rust
  // coordinator.rs:922-933:
  if let Some(worker_id) = process_join_request(...).await? {
      // INSIDE process_join_request, at L201-206, ALREADY logged:
      //   "Worker joined: id={worker_id}, partition_index={pi}, next_round={r+1}"
      // ↑ already an INFO emission, format string, no R17 marker

      worker_streams.push(stream);
      round_joined_count += 1;

      // coordinator.rs:940-945:
      tracing::info!(
          worker_id,
          k_eff_new,
          round = metrics.rounds,
          "Worker joined the grid (R17)"
      );
      // ↑ second INFO emission, structured, with R17 marker
  }
  ```
- **Impact:**
  1. **Cardinality doubled.** Every successful join produces 2 INFO events. Loki / Datadog dashboards that count events-per-second see 2× the actual join rate.
  2. **Format inconsistency.** L201 uses the format-string macro (no structured fields); L940 uses structured fields. SPEC-11 R28 OTel attribute mapping requires structured fields. The first log is non-compliant.
  3. **R17 label drift.** Only the L940 emission carries the `(R17)` marker. The L201 emission is silently anonymous. When the developer fixes MF-001 (add `partition_index` to the L940 site), the L201 emission still won't have the R17 marker, but it WILL have `partition_index` in the format string — confusing the log analyzer about which is the "true" R17 event.
- **Why reviewer missed:** Reviewer focused on the L940 site (the canonical R17 location) and proposed adding `partition_index` there (MF-001). Reviewer did NOT realize that `process_join_request` itself emits an info log at L201. Two emissions, same trigger, no de-duplication.
- **Suggested fix:**
  1. **(Mandatory)** Delete the `tracing::info!` at L201-206. Move the log responsibility entirely to the caller at L940. The L201 site predates the R17 spec contract; it's a remnant that should not coexist with the spec-compliant version.
  2. **(Alternative)** Convert L201 to `tracing::debug!` and rephrase ("JoinAck issued: ...") so it's a developer-debug breadcrumb, not an R17 event. Keep L940 as the canonical R17 emitter.
  3. **(Test)** Add a `tracing-subscriber` test that runs a single join and asserts exactly **one** INFO event with the marker `R17` is captured.

---

### QA-005 (HIGH) — `recv_frame` at coordinator.rs:921 has NO timeout; a slow joiner blocks the join-window past `join_window_max`, defeating SPEC-20 §3.2 R12

- **Severity:** HIGH — spec violation (R12 join_window_max bound); coordinator can hang for minutes on a slow client.
- **Category:** Resource Exhaustion / Timeout Missing / DoS Surface
- **Location:** `coordinator.rs:910-958` (join window loop) — specifically L921
- **Static trace:**
  ```rust
  // coordinator.rs:910-958
  loop {
      while let Some(mut stream) = pending_connections_queue.pop_front() {
          let active_ids: ... = ...;

          // L921 — NO TIMEOUT:
          let (msg, _) = recv_frame(&mut stream, config.max_payload_size).await?;
          //               ^^^^^^^^^^ blocks indefinitely on a slow client

          if let Some(worker_id) = process_join_request(...).await? {
              ...
          }
      }
      if min_timer.is_elapsed() { break; }
      tokio::select! {
          new_conn = transport.accept() => { ... }
          _ = &mut min_timer => {}
          _ = &mut max_timer => { break; }
      }
  }
  ```
- **Why this is a spec violation:** SPEC-20 §3.2 R12 mandates that the join window be bounded by `join_window_max` (default 500 ms). The `tokio::select!` at L951-957 enforces this for the *outer* polling. But once a stream is popped from `pending_connections_queue` and the inner `while` loop calls `recv_frame` (L921), there is no timeout. A malicious or slow client that connects but never sends `JoinRequest` causes the coordinator to block indefinitely, far past `join_window_max`.
- **Impact:**
  1. **DoS surface:** any TCP client can connect, never send data, and stall the grid coordinator for an arbitrary duration. Per-connection. With multiple slow clients in the queue, the stall stacks.
  2. **Spec contract broken:** R12's "bounded join window" guarantee is not met.
  3. **Cascade with QA-001:** the inflated "merge_time_per_round" recorded at L959 will, in this DoS case, capture not just the legitimate join window but also the slow-client stall — multi-second "merge times" appear in benchmark output.
- **Why current tests miss it:** No test injects a TCP client that connects + never sends. The existing tests (`test_accept_workers_*`) use `send_register` immediately; the join-window tests are presumably similar.
- **Suggested fix:**
  1. **(Mandatory)** Wrap L921 in `tokio::time::timeout(join_window_max, recv_frame(...)).await`. On timeout, drop the stream silently and continue the inner loop.
  2. **(Defence-in-depth)** Move the entire `pending_connections_queue` dequeue loop inside the `tokio::select!` so the `&mut max_timer` future is observed during the inner work — making the budget enforcement holistic.
  3. **(Test)** Add an integration test that connects a TCP client without sending Register/JoinRequest and asserts the join window completes within `join_window_max + 100ms` slack.

---

### QA-006 (MEDIUM) — D4-elastic assertion site (helpers.rs:91-99) does not invariant-check the `partition_index` density it claims to defend; per reviewer MF-003, but with a deeper miss

- **Severity:** MEDIUM — defense-in-depth that does not defend (reviewer's MF-003 elaboration)
- **Category:** Spec Violation / Tautological Assertion / Missed Real-Property Check
- **Location:** `partition/helpers.rs:86-99` (Wave 2 D4-elastic block)
- **Why reviewer's MF-003 is incomplete:**
  - Reviewer correctly noted the `expected_k_eff` is computed identically to `k_eff` (tautological).
  - Reviewer's proposed fix moves the assertion to `assert!(map.len() as u32 == k_eff)`. **But that proposed fix STILL does not exercise the spec-named property of D4-elastic.**
  - SPEC-20 R11a says: "every consumed `partition_index` is in `[0, K_eff)` and dense" — a **positional** property. `map.len() == K_eff` is necessary but not sufficient: a `HashMap<WorkerId, IdRange>` could have `len = K_eff` but `partition_index_of(worker, ...)` might return values outside `[0, K_eff)` due to a hybrid-offset bug.
  - The real D4-elastic check needs to be: **for each worker in the active set, its `partition_index_of(...)` is in `[0, K_eff)`, and the set of all consumed indices is exactly `{0, 1, ..., K_eff - 1}`** (no gaps, no duplicates, no out-of-range).
- **Reproduction of the fix gap:** a hypothetical bug where hybrid mode but `WorkerId 0` is also a remote worker (collision) would produce `map.len() == K_eff - 1` (the hybrid `insert(0, ...)` at L110 collides with the `insert(worker_id=0, ...)` at L117) — silently. Reviewer's proposed fix catches this (`map.len() != K_eff`). Good. But a different bug — `compute_id_ranges` with `i+offset > K_eff-1` causing array out-of-bounds — is caught by Rust's bounds check, not by the assertion. So the reviewer's fix is necessary but not the most informative fix.
- **Suggested fix (extending reviewer's MF-003):**
  ```rust
  #[cfg(debug_assertions)]
  {
      // Real D4-elastic property (SPEC-20 R11a):
      let mut consumed_indices: Vec<u32> = Vec::with_capacity(k_eff as usize);
      if config.hybrid_coordinator {
          consumed_indices.push(0);
      }
      for &wid in active_workers {
          if let Some(idx) = partition_index_of(wid, active_workers, config.hybrid_coordinator) {
              consumed_indices.push(idx);
          }
      }
      consumed_indices.sort();
      consumed_indices.dedup();
      debug_assert_eq!(
          consumed_indices.len() as u32, k_eff,
          "D4-elastic violated: dense partition_index set [{}, {}) was not formed: got {:?}",
          0, k_eff, consumed_indices
      );
      for (i, &idx) in consumed_indices.iter().enumerate() {
          debug_assert_eq!(idx, i as u32, "D4-elastic violated: gap or duplicate in partition_index sequence");
      }
  }
  ```

---

### QA-007 (MEDIUM) — `tracing::warn!(worker_id=wid, ...)` field cardinality unbounded in long runs; no SPEC-11 cardinality cap

- **Severity:** MEDIUM — operational risk for long-running grids; Loki/Prometheus cardinality explosion.
- **Category:** Observability / Cardinality / SPEC-11 Compliance
- **Location:** All 4 R28 warn sites (`coordinator.rs:672, 698, 721, 738`) and the R17 info site (`coordinator.rs:940`)
- **Static trace:**
  - All five sites emit structured fields including `worker_id` (the WorkerId, a `u32`).
  - Over a run with high churn (e.g., a benchmark that joins/departs a worker every 100 ms over 24 hours), the unique `worker_id` values can reach ~864,000.
  - SPEC-11 cardinality constraint (R12 / NF-002): high-cardinality fields like `worker_id` should be either bucketed or moved to a span (low-cost) rather than emitted as a structured log field (high-cost in Loki / Prometheus / Datadog labels).
- **Impact:**
  - **Loki / Datadog cardinality explosion.** `worker_id` as a structured field becomes a label; each unique value creates a new time series. 864k unique series per coordinator per day exceeds typical Loki ingestion limits (~100k/day).
  - **Storage cost.** Structured-log indexes balloon.
- **Why reviewer missed:** Reviewer treated structured fields as the *correct* form (correctly so for SPEC-11 OTel attribute mapping). The cardinality nuance — that `worker_id` is high-cardinality and should be emitted to a span/trace, not as a label — is a separate SPEC-11 dimension.
- **Suggested fix:**
  1. **(Compromise)** Document in the R28 warn sites that `worker_id` is intended as an OTel **span attribute** (low-cost, indexed once per span), not a Prometheus **label** (high-cost, indexed per series). Add a SPEC-20 amendment OR a SPEC-11 audit (whichever owns observability cardinality bounds).
  2. **(Defence-in-depth)** Bucket `worker_id` for the warn-log emission: `worker_id_bucket = worker_id / 1000` is a low-cardinality label; keep the raw `worker_id` only in the span body / message. Loses some information, but bounds operational cost.
  3. **(Note)** Surface to `especialista-specs` as a SPEC-11 follow-up: bound `worker_id` cardinality for log fields.

---

### QA-008 (MEDIUM) — `tracing::warn!(error = description, ...)` at coordinator.rs:705 emits worker-supplied `description` verbatim; no length cap, no PII filter

- **Severity:** MEDIUM — log injection / PII surface; complements QA-002 from the TASK-0414 QA precedent.
- **Category:** Adversarial Input / Resource Exhaustion / Information Leak
- **Location:** `coordinator.rs:686-707` (Message::Error arm of the collect loop)
- **Static trace:**
  ```rust
  Message::Error { worker_id, description, .. } => {
      let outcome = handle_connection_loss(worker_id, &description, ...);
      match outcome {
          ConnectionLossOutcome::RecoveryTriggered { worker_id: id, .. } => {
              tracing::warn!(
                  worker_id = id,
                  round = metrics.rounds,
                  error = description,                       // <-- L705: verbatim worker payload
                  "Worker departed due to error; triggering recovery (R28)"
              );
              ...
          }
      }
  }
  ```
- **Impact:**
  1. **Log injection.** A worker (compromised or malicious) sends `Message::Error { description: "fake R28 log\n  R28 BLOCKED EXFIL: secret=AKIA...\n", ... }`. The newline-and-fake-prefix appears verbatim in the log stream, letting the worker forge log entries that look like coordinator-emitted logs.
  2. **Resource exhaustion.** A 4 GiB `description` (no spec-mandated max) is captured into the structured-log subscriber, which may allocate copies for backend formatters.
  3. **PII / secret leakage.** A worker's stack trace may carry env vars, file paths, or auth tokens. The current code emits them verbatim into the log stream without sanitization.
- **Why current tests miss it:** No test sends a `Message::Error` with a >1KB description.
- **Suggested fix:**
  1. **(Mandatory)** Truncate `description` to ≤ 4 KiB before emission. Strip `\n` and `\r` to prevent log injection.
  2. **(Defence-in-depth)** Add a `redact_log_string(s: &str) -> String` helper in `protocol/` that bound-checks length and replaces control chars. Apply at L705 and at any other site that emits worker-supplied strings.
- **Note:** Same root cause as QA-002 from the TASK-0414 precedent (`SelfPartitionPanic(String)` unbounded payload).

---

### QA-009 (MEDIUM) — `pending_connections_queue` is unbounded; stale streams accumulate across rounds; no cleanup of dropped/disconnected entries before drain

- **Severity:** MEDIUM — resource leak; unbounded queue growth.
- **Category:** Resource Exhaustion / Cleanup / State Hygiene
- **Location:** `coordinator.rs:481` (queue creation); `coordinator.rs:911-947` (drain); `coordinator.rs:521, 953` (push)
- **Static trace:**
  - The queue is created at L481 as `VecDeque::<TransportStream>::new()` — no capacity limit.
  - It is fed from two paths: SoloReducing accept (L521) and join-window accept (L953).
  - It is drained (L911) only when `elastic_join` is true. **In `elastic_join=false` mode, the SoloReducing accept (L521) feeds the queue but nothing drains it.** A long SoloReducing run accumulates streams indefinitely.
  - There is no per-stream timeout in the queue. A stream that connected but never sent JoinRequest sits in the queue forever; QA-005 documents how it stalls on dequeue, but here the issue is queue *growth*.
- **Impact:**
  - **In SoloReducing-only mode** (hybrid + no elastic_join — feasible config), queue grows unbounded.
  - **Memory cost.** Each `TransportStream` carries an OS file descriptor + tokio buffers; ~100 KiB/conn typical. 100K stale connections = 10 GiB of FDs.
  - **FD exhaustion.** OS process FD limit (default 1024 on Linux) hit far before memory issue.
- **Suggested fix:**
  1. **(Mandatory)** Bound the queue size (e.g., `MAX_PENDING_CONNECTIONS = 64`). On overflow, drop the oldest entry (or the new one, depending on which is more adversarial-friendly).
  2. **(Cleanup)** On every round entry, drain the queue if `!elastic_join` (otherwise the streams are pure dead weight).

---

### QA-010 (MEDIUM) — `register_initial` happens lazily via `entry().or_insert_with` at coordinator.rs:587-600; an empty round-0 (D=0 active workers, hybrid only) leaves `retained_state.initial` empty, then any later `refresh_last_acked` panics (debug) / silently corrupts (release)

- **Severity:** MEDIUM — degenerate-config panic path; D5 chain extension of QA-002.
- **Category:** Edge Case / Initialization Hygiene
- **Location:** `coordinator.rs:587-600`
- **Static trace:**
  ```rust
  // L587-600 (round init):
  if grid_config.retain_partitions {
      if let Some(ref p) = self_partition {
          retained_state.initial.entry(0).or_insert_with(|| RetainedInitial::V1(p.clone()));
      }
      for p in &remote_partitions {
          retained_state.initial.entry(p.worker_id).or_insert_with(|| RetainedInitial::V1(p.clone()));
      }
  }
  ```
  - In a degenerate but legal config (`hybrid_coordinator=false`, `num_workers=0` initially, waiting for joiners), `self_partition` is None and `remote_partitions` is empty. The init block runs but inserts nothing.
  - Later, when a joiner arrives mid-session and returns a `PartitionResult`, L662-668 calls `refresh_last_acked` for that worker — D5 panics in debug (QA-002), silently fails in release (QA-002 follow-up).
  - The `entry().or_insert_with` pattern is also subtly wrong in a different direction: if the same `worker_id` shows up in subsequent rounds (e.g., the same worker re-joining after a brief disconnect with same id), `or_insert_with` SKIPS the update, retaining the stale round-0 state. SPEC-20 R23b says retained_initial is "the round-0 dispatch state, allocated once per worker." Re-join with same id should arguably re-allocate. The current code semantics are ambiguous.
- **Suggested fix:**
  1. Use `insert` (not `entry().or_insert_with`) and document the round-0-only semantics explicitly.
  2. Add the post-join init block proposed in QA-002 to cover mid-session joiners.
  3. Add a `release_worker(wid)` call when a worker disconnects permanently, freeing the slot — the `release_worker` method exists on the registry (`retained.rs:67-70`) but is **never called** from `coordinator.rs`. Grep confirms zero call sites.

<!-- Verified via Grep: `release_worker` has no call sites in protocol/coordinator.rs -->

---

### QA-011 (LOW) — `bincode::serialize` of long-running `Vec<u64>` `join_round_overhead_ms_per_round` has no overflow protection; legitimate 100k-round bench produces ~800 KiB GridMetrics blob

- **Severity:** LOW — operational note; no correctness break.
- **Category:** Resource Profile / Long-Run Behavior
- **Location:** `merge/types.rs:134` (`join_round_overhead_ms_per_round: Vec<u64>`)
- **Static trace:** Each `u64` is 8 bytes (varint-encoded smaller for small values, but worst case 10 bytes). 100k rounds = ~800 KiB just for this field. Combined with the 6 sibling `Vec<u32>` (~400 KiB) and the existing `worker_stats_per_round: Vec<Vec<WorkerRoundStats>>` (potentially huge), a full `bincode::encode(&grid_metrics)` for a long bench may exceed `max_payload_size` (default 16 MiB).
- **Impact:** None for correctness; bench sidecar serialization may fail with `UnexpectedEnd` on the receiver. Since `GridMetrics` lacks `Deserialize` (per reviewer §4), this is mostly theoretical today.
- **Suggested fix:** None for this bundle. Note in `docs/next-steps.md` that long-run telemetry serialization needs streaming/chunking (SPEC-19 streaming work).

---

### QA-012 (LOW) — All 7 new `Vec<u32>`/`Vec<u64>` fields lack a `with_capacity` hint; many tiny reallocations in a long run

- **Severity:** LOW — micro-perf; no correctness impact.
- **Category:** Performance / Allocation
- **Location:** `merge/types.rs:116-134`; `Default::default` constructs empty Vecs.
- **Static trace:** `GridMetrics::default()` produces 7 empty Vecs. The first `push` triggers an allocation; subsequent pushes follow the standard `2x` doubling. For a 1000-round bench, that's ~10 reallocations per Vec, ~70 reallocations total — minor.
- **Suggested fix:** None for this bundle. Optionally, add a `GridMetrics::with_capacity(rounds: usize)` constructor for benches with known round counts.

---

### QA-013 (LOW) — R17 join log fires AFTER `worker_streams.push(stream)`; if the new stream's connection drops between L934 and L940, the log misleadingly says "Worker joined" for an already-departed worker

- **Severity:** LOW — race window is microseconds; misleading-log impact only.
- **Category:** Race Condition / Log Truthfulness
- **Location:** `coordinator.rs:934-945`
- **Static trace:**
  ```rust
  worker_streams.push(stream);                                  // L934
  round_joined_count += 1;                                       // L935
  let k_eff_new = worker_streams.len() + ...;                    // L938-939
  tracing::info!(worker_id, k_eff_new, ..., "Worker joined ..."); // L940-945
  ```
  - Between L934 (push) and L940 (log), the underlying TCP stream may RST. The next round's collect loop will encounter a dead stream and emit an R28 WARN — but the L940 R17 INFO log already fired claiming the worker successfully joined.
  - Log temporal order: `R17 INFO (joined)` → `R28 WARN (departed)`. Looks legitimate but the worker never actually participated in any reduction. Log analyzers may treat this as "joined and departed in the same round" (which is in fact what happened, but the analyzer may not have a state machine to detect it).
- **Impact:**
  - Trivia for log scrapers; statistical drift in "successful joins" counts vs "completed first round" counts.
  - The `round_joined_count` at L961 includes such ghost joiners — `metrics.workers_joined_per_round` over-counts.
- **Suggested fix:**
  1. **(Compromise)** Move L940 BEFORE L934. The race is then on the other side: log fires, then push happens — log is now "speculative" (worker about to be added). Marginally better than "worker added, then we're checking it's still alive."
  2. **(Stronger)** After L940, do a non-blocking `try_recv_frame` heartbeat to verify the connection is alive before treating the join as final.
  3. **(Doc-only)** Note in the log message that "Worker joined" is the JoinAck-issued state, not "Worker is currently alive": `"Worker JoinAck issued (R17)"`.

---

### QA-014 (LOW) — `next_worker_id` increment at coordinator.rs:182 in `process_join_request` is non-atomic vs concurrent join attempts; in single-task tokio runtime (default), no race; in multi-thread runtime, race possible

- **Severity:** LOW — not a bug today (tokio default is single-thread per task), but a future hazard.
- **Category:** Concurrency / Future Hazard
- **Location:** `coordinator.rs:181-182`
  ```rust
  let worker_id = *next_worker_id;     // L181: read
  *next_worker_id += 1;                 // L182: increment (NOT a fetch_add)
  ```
- **Static trace:** `next_worker_id` is `&mut u32`. The function is `async`. If two `process_join_request` futures execute concurrently (e.g., on separate worker threads in `tokio::runtime::Builder::new_multi_thread`), the read-modify-write is not atomic and two joiners can be assigned the same WorkerId. Today, the join window invokes `process_join_request` sequentially in a `while let` loop (L911-947), so concurrency is only a future hazard. But the function signature accepts `&mut u32`, leaving the door open.
- **Suggested fix:** None for this bundle. Note in `docs/next-steps.md` that any future refactor to fan out `process_join_request` across tasks must promote `next_worker_id` to `Arc<AtomicU32>`.

---

## Edge case catalog — what TEST-SPEC-0450..0452 listed and the developer's tests skipped or covered weakly

| ID | Edge case | Status | Action |
|----|-----------|--------|--------|
| EC-A | `merge_time_per_round` writes are semantically correct in elastic mode | **NOT COVERED** (QA-001) | Mandatory regression test: assert merge_time_per_round[r] ≈ instrumented merge() duration. **CRITICAL gap.** |
| EC-B | `refresh_last_acked` for mid-session joiner does not panic | **NOT COVERED** (QA-002) | Mandatory regression test: 3-round elastic flow with joiner, assert no panic + initial[joiner] populated. **CRITICAL gap.** |
| EC-C | All 7 elastic Vecs have len == metrics.rounds at end of every round | **NOT COVERED** (QA-003) | Mandatory invariant test + maybe a single-helper push. **HIGH gap.** |
| EC-D | Only one R17 INFO log emitted per join | **NOT COVERED** (QA-004) | Tracing-subscriber test: capture INFO events, assert exactly 1 with R17 marker. **HIGH gap.** |
| EC-E | Join window respects `join_window_max` even with slow client | **NOT COVERED** (QA-005) | Integration test: TCP client connects, never sends — assert window completes within budget. **HIGH gap.** |
| EC-F | D4-elastic catches a hybrid-mode collision (worker_id 0 conflict) | **NOT COVERED** (QA-006) | Add the dense-index check in helpers.rs. **MEDIUM gap.** |
| EC-G | `worker_id` cardinality bounded for log emission | **NOT COVERED** (QA-007) | Spec amendment + bucketing. **MEDIUM gap.** |
| EC-H | `description` (worker-supplied) is sanitized before logging | **NOT COVERED** (QA-008) | Inject 1 MiB description, assert truncation. **MEDIUM gap.** |
| EC-I | `pending_connections_queue` is bounded | **NOT COVERED** (QA-009) | Stress test: 1000 idle clients, assert queue stays bounded. **MEDIUM gap.** |
| EC-J | `release_worker` is called when a worker permanently disconnects | **NOT COVERED** (QA-010) | Audit `release_worker` call sites; add to coordinator depart path. **MEDIUM gap.** |
| EC-K | `bincode::encode(GridMetrics)` succeeds for 100k-round bench | **NOT COVERED** (QA-011) | Note in next-steps for streaming serialization. **LOW gap.** |
| EC-L | `GridMetrics` Vecs use capacity hint when round count is known | **NOT COVERED** (QA-012) | Add `with_capacity` constructor. **LOW gap.** |
| EC-M | R17 log temporal ordering verified | **NOT COVERED** (QA-013) | Reorder log vs push, or add a recheck before logging. **LOW gap.** |
| EC-N | `next_worker_id` race-free under multi-thread runtime | **NOT COVERED** (QA-014) | Note as future hazard. **LOW gap.** |

**Coverage gaps that materially affect Stage 6 sign-off: EC-A, EC-B, EC-C, EC-D, EC-E.**

---

## Recommendation for Stage 6 REFACTOR — unified action list

Ordered by severity. **QA findings interleaved with Stage 4 MF-001..MF-006 and SF-001..SF-004.** Items marked [BLOCKER] must land before bundle commits.

| # | Source | ID | Severity | Action | Surface |
|---|--------|----|----------|--------|---------|
| 1 | QA | **QA-001** | **CRITICAL** [BLOCKER] | Remove `metrics.merge_time_per_round.push(t_window_start.elapsed())` at L959; add new `join_window_time_per_round: Vec<Duration>` field to `GridMetrics`; push the join-window elapsed there; AND add a structural-merge time push at L867-889 to populate `merge_time_per_round` correctly. | `coordinator.rs:959`, `merge/types.rs` |
| 2 | QA | **QA-002** | **CRITICAL** [BLOCKER] | After `worker_streams.push(stream)` at L934, insert a corresponding `retained_state.initial.insert(worker_id, ...)` ONCE the joiner's first partition is computed (round N+1 dispatch), OR relax D5 from `debug_assert!` to a `tracing::warn!`. | `coordinator.rs:934`, `protocol/retained.rs:58` |
| 3 | QA | **QA-003** | **HIGH** [BLOCKER] | Restructure round-end metric pushes into a single helper or assert `len(all per-round Vecs) == metrics.rounds` invariant at top of every round; ensure all 7 elastic counters are pushed even on `?` early returns. | `coordinator.rs:561, 835-846, 961` |
| 4 | QA | **QA-004** | **HIGH** [BLOCKER] | Delete the `tracing::info!` at `coordinator.rs:201-206` OR demote to `debug!`; keep L940-945 as the canonical R17 emitter (post MF-001 fix). | `coordinator.rs:201-206` |
| 5 | QA | **QA-005** | **HIGH** [BLOCKER] | Wrap `recv_frame` at `coordinator.rs:921` in `tokio::time::timeout(join_window_max, ...)`. | `coordinator.rs:921` |
| 6 | review | **MF-001** | MEDIUM | Add `partition_index` field to R17 INFO log. | `coordinator.rs:937-945` |
| 7 | review | **MF-002** | MEDIUM | Replace `?kind` with canonical `departure_type` strings + add `retained_slot = "retained_initial"` at all 4 R28 sites. | `coordinator.rs:672-755` |
| 8 | review | **MF-003** | MEDIUM | Replace tautological D4-elastic assertion AND extend with the dense-index property check from QA-006. | `partition/helpers.rs:86-99` |
| 9 | QA | **QA-006** | MEDIUM | Folded into MF-003 — add the `partition_index_of` density invariant. | (folded) |
| 10 | review | **MF-004** | MEDIUM | Add `// FIXME(TASK-0443)` at the metrics push site; raise an entry in `docs/next-steps.md`. | `coordinator.rs:828-841` |
| 11 | review | **MF-005** | MEDIUM | Rename `round` to `first_participating_round` in R17 log. | `coordinator.rs:943` |
| 12 | review | **MF-006** | MEDIUM | Pick one assertion style (`debug_assert!` vs `#[cfg(debug_assertions)]` + `assert!`) and apply consistently. | 4 Wave 2 sites |
| 13 | QA | **QA-007** | MEDIUM | Document `worker_id` cardinality bound (span vs label); raise SPEC-11 follow-up. | `coordinator.rs` warn sites |
| 14 | QA | **QA-008** | MEDIUM | Truncate worker-supplied `description` to 4 KiB; strip `\n`/`\r`. | `coordinator.rs:705` |
| 15 | QA | **QA-009** | MEDIUM | Bound `pending_connections_queue` size. | `coordinator.rs:481, 521, 953` |
| 16 | QA | **QA-010** | MEDIUM | Audit `release_worker` call sites; wire into permanent-disconnect path. | `coordinator.rs`, `protocol/retained.rs:67` |
| 17 | review | **SF-001** | LOW | Delete `let _ = _round_reclaimed_initial;`. | `coordinator.rs:828-829` |
| 18 | review | **SF-002** | LOW | Normalize naming of the three `round_*` counters. | `coordinator.rs:622-624` |
| 19 | review | **SF-003** | LOW | Open follow-up test task for UT-0450-01, UT-0450-10. | `relativist-core/tests/` (new) |
| 20 | review | **SF-004** | LOW | Note `bytes_received` byte attribution in a comment. | `coordinator.rs:648-720` |
| 21 | QA | **QA-011** | LOW | Note long-run serialization in `docs/next-steps.md`. | `merge/types.rs` |
| 22 | QA | **QA-012** | LOW | Optional: add `GridMetrics::with_capacity`. | `merge/types.rs` |
| 23 | QA | **QA-013** | LOW | Reorder R17 log vs push, or add post-log heartbeat. | `coordinator.rs:934-945` |
| 24 | QA | **QA-014** | LOW | Note `next_worker_id` future-hazard for multi-thread runtime. | `coordinator.rs:181-182` |

**Verification gates after Stage 6 fixes:**
- `cargo test --workspace` — all baseline tests pass; new tests for QA-001..QA-005 added.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
- `cargo fmt --check` — clean.
- v1 floor of 690 tests — preserved.
- v2 baseline (1181 default / 1224 zero-copy) — preserved or extended.
- New: `metrics.merge_time_per_round[r]` semantic-correctness regression.
- New: 3-round elastic flow with mid-session joiner + retain_partitions=true does not panic in debug build.
- New: all 7 elastic Vec lengths equal `metrics.rounds` after every successful round AND after every `?` early-return path.

---

## Sign-off

The bug-hunt is complete. I exhausted the adversarial attack surface enumerated in the orchestrator's brief:

1. **Counter increment off-by-one / partial-round push:** surfaced QA-003 (HIGH) — `effective_slots_per_round` desyncs from siblings on `?` early returns; no `metrics.rounds` invariant.
2. **Log field cardinality explosion:** surfaced QA-007 (MEDIUM) — `worker_id` is high-cardinality; SPEC-11 lacks the bound.
3. **`debug_assert!` panic in observability path:** surfaced QA-002 (CRITICAL) — D5 panics on every mid-session joiner returning a partition result; debug-build coordinator dies.
4. **Atomicity claim (D5):** surfaced QA-014 (LOW) for `next_worker_id`; QA-002 for D5's broken precondition contract; the registry IS not Sync in practice but is single-threaded so OK today.
5. **`GridMetrics::Default` correctness for the 7 new fields:** verified empty Vecs, no overflow, `default()` produces `Vec::new()` (not `Vec::with_capacity(0)`); QA-012 notes the missing capacity hint as LOW.
6. **`bincode` overflow on long Vecs:** surfaced QA-011 (LOW) — 100k-round bench produces ~800 KiB blob; below today's max_payload_size but worth noting.
7. **Telemetry leak / log injection:** surfaced QA-008 (MEDIUM) — worker-supplied `description` emitted verbatim; injection + PII surface. Spot-checked the 5 warn sites + 1 info site; no `auth_token` leak found, but `description` is user-controlled.
8. **R17 race with worker disconnect:** surfaced QA-013 (LOW) — log fires after push; race window microseconds, mostly cosmetic.

Plus three additional findings the brief did not enumerate but emerged during static trace:

9. **QA-001 (CRITICAL):** `merge_time_per_round` semantically corrupted in elastic mode — invalidates SPEC-09 break-even data. Highest-impact finding of this review.
10. **QA-004 (HIGH):** duplicate / inconsistent R17 INFO log between `process_join_request` and the join-window caller.
11. **QA-005 (HIGH):** `recv_frame` in the join window has no timeout; spec violation of R12 join_window_max bound; DoS surface.

**Verdict:** **REQUIRE Stage 6 FIXES — BLOCKED on QA-001, QA-002, QA-003, QA-004, QA-005** (and the reviewer's MF-001..MF-006 list). MEDIUM/LOW items should be bundled into the same Stage 6 pass.

The implementation is **functionally complete** for the happy-path elastic flow without retain_partitions+joiner combo, but the observability layer that this Phase E bundle was supposed to deliver is partially **lying** (QA-001 — wrong field), partially **fragile** (QA-002 — panics in debug), partially **inconsistent** (QA-003 — desync on errors, QA-004 — duplicate logs), and partially **unbounded** (QA-005, QA-007, QA-008, QA-009 — DoS / cardinality / unbounded resources). For a Phase E whose stated goal is to instrument the elastic protocol so SPEC-09 break-even can be measured, **shipping QA-001 unfixed makes the entire Phase E benchmark output unusable**.

The CRITICAL fixes are bounded:
- QA-001: ~10 LoC change + 1 new field
- QA-002: ~5 LoC change + 1 new test
- QA-003: ~30 LoC restructure (or 5 LoC for a debug_assert audit hook)

Estimate: 4-6 hours including fixes + regression tests + cargo verification. Strongly recommend gating bundle on QA-001..QA-005 closure before Stage 6 sign-off.

— qa, 2026-04-27

---

Phase E QA: 2 CRITICAL, 3 HIGH, 5 MEDIUM, 4 LOW
