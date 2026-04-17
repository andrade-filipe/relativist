# TEST-SPEC-0383: Worker final-state handler — `handle_final_state_request` (R21.3, R28)

**Task:** TASK-0383
**Spec:** SPEC-19 §3.3 R21 phase 3 (Final State Collection), R28 (worker
  responds with full `Partition` in `FinalStateResult`)
**Spec-critic notes:** No DC-Cn directly amends TASK-0383. The handler
  reuses `WorkerState::Returning` (v1 variant) for outgoing-one-message-
  then-close semantics, consistent with v1 `PartitionResult` path.
**Generated:** 2026-04-17

---

## Scope note

TASK-0383 ships the pure handler that responds to
`Message::FinalStateRequest { round }` at convergence (or at `max_rounds`
cap per R30). The handler:

1. Extracts the stored `Partition` from `ctx.delta_state` via `.take()`
   (clears the state — worker is done).
2. Transitions `ctx.state = WorkerState::Returning` (v1 variant, reused).
3. Returns one `LogTransition` + one `SendMessage(Box::new(Message::FinalStateResult { round, partition }))`.

**Round-0-only convergence path:** the handler MUST accept BOTH
`DeltaIdle` AND `DeltaActive` as legal pre-states. A net that arrives
already in Normal Form converges from `DeltaIdle` without ever entering
`DeltaActive`.

**`.take()` semantics:** the state is moved out of `Option`, leaving
`ctx.delta_state == None` post-call. This frees memory; the worker has
no reason to retain the partition once the coordinator owns the merge
result. UT-0383-03 locks this contract.

---

## Test target file paths

- `relativist-core/src/worker.rs` — inline `#[cfg(test)] mod tests`.
  Five new `#[test]` fns (one with `#[should_panic]`).

All tests are synchronous. No `tokio`, no `async`.

---

## Unit Tests

### UT-0383-01: `handle_final_state_request_from_delta_active`

**Purpose:** Happy-path convergence — worker is in `DeltaActive` (has
processed at least one delta round), receives `FinalStateRequest`,
emits `FinalStateResult` with the stored partition.

**Target:** `worker.rs::tests`

**Given:**
- `ctx.state = WorkerState::DeltaActive`
- `ctx.delta_state = Some(WorkerDeltaState::from_initial_partition(partition.clone()))`
- `ctx.round = 5` (some prior round count)

**When:** `let actions = handle_final_state_request(&mut ctx, 5);`

**Then:**
- `ctx.state == WorkerState::Returning`
- The returned `Vec<WorkerAction>` contains `SendMessage` wrapping
  `Message::FinalStateResult { round: 5, partition: <equal to original> }`.
- The carried `partition` equals the partition originally seeded
  (modulo any in-place mutation across delta rounds — for the fixture
  here we use a partition the worker did NOT mutate; `partition`
  is byte-equal to the seed).

**Assertions:** Final emission carries the worker's full current
partition state.

**SPEC-19 R covered:** R21 phase 3, R28.

---

### UT-0383-02: `handle_final_state_request_from_delta_idle_round_zero_convergence`

**Purpose:** Round-0-only convergence path — input net is already in
Normal Form, coordinator detects this from the seeded `BorderGraph` and
sends `FinalStateRequest` BEFORE any `RoundStart`. Worker is in
`DeltaIdle`. Handler must accept this state, NOT panic.

**Target:** `worker.rs::tests`

**Given:**
- `ctx.state = WorkerState::DeltaIdle`
- `ctx.delta_state = Some(WorkerDeltaState::from_initial_partition(partition.clone()))`
- `ctx.round = 0` (no delta rounds have run)

**When:** `handle_final_state_request(&mut ctx, 0);`

**Then:**
- No panic.
- `ctx.state == WorkerState::Returning`
- Emitted `Message::FinalStateResult.round == 0`
- Emitted `Message::FinalStateResult.partition == partition` (untouched
  Round-0 seed).

**Assertions:** Both `DeltaIdle` and `DeltaActive` are accepted as
pre-states (per task acceptance criteria #2).

**SPEC-19 R covered:** R21 phase 3 (early-convergence edge case).

---

### UT-0383-03: `handle_final_state_request_clears_delta_state_via_take`

**Purpose:** Lock the `.take()` contract — after the handler runs,
`ctx.delta_state == None`. The worker has no reason to retain the
partition; memory is freed.

**Target:** `worker.rs::tests`

**Given:**
- `ctx.state = WorkerState::DeltaActive`
- `ctx.delta_state = Some(_)` (any legal partition)

**When:** Call handler.

**Then:**
- `ctx.delta_state.is_none() == true`.

**Assertions:** Memory cleanup is part of the contract. A future
refactor that uses `.as_ref().clone()` instead of `.take()` would
silently keep the partition resident; this test fires.

**SPEC-19 R covered:** R28 (worker shuts down post-final).

---

### UT-0383-04: `handle_final_state_request_without_delta_state_panics`  `#[should_panic]`

**Purpose:** Caller-invariant violation: receiving `FinalStateRequest`
when `ctx.delta_state == None` means the coordinator sent
`FinalStateRequest` without a prior `InitialPartition` — a protocol
bug. Handler MUST panic with a clear message.

**Target:** `worker.rs::tests`

**Given:**
- `ctx.delta_state = None`
- Any `ctx.state` (e.g. `WorkerState::Idle`).

**When:** `handle_final_state_request(&mut ctx, 0);`

**Then:**
- `#[should_panic(expected = "handle_final_state_request requires prior handle_initial_partition")]`
  fires.

**Assertions:** Panic message names the violated invariant (R21.1
breadcrumb in panic text aids debug).

**SPEC-19 R covered:** Caller invariant (R21 phase 1 prerequisite).

---

### UT-0383-05: `handle_final_state_request_echoes_round`

**Purpose:** Round number on the outgoing message MUST equal the
incoming `round` parameter (so the coordinator can pair reply to
dispatch).

**Target:** `worker.rs::tests`

**Given:**
- `ctx.delta_state = Some(_)`
- `ctx.state = WorkerState::DeltaActive`

**When:** `handle_final_state_request(&mut ctx, 42);`

**Then:**
- Emitted `Message::FinalStateResult.round == 42`.
- `ctx.round == 42` (handler updates `ctx.round` to the incoming value).

**Assertions:** Round echo is exact.

**SPEC-19 R covered:** R28 (round field in `FinalStateResult`).

---

## Coverage mapping

| Requirement / DC | Covered by |
|---|---|
| R21 phase 3 — convergence triggers Final Collection | UT-0383-01, UT-0383-02 |
| R28 — `FinalStateResult` carries full Partition | UT-0383-01, UT-0383-02 |
| R28 — round echo | UT-0383-05 |
| Round-0-only convergence accepted | UT-0383-02 |
| `.take()` semantics — `delta_state == None` post-call | UT-0383-03 |
| Caller invariant — panic on missing `delta_state` | UT-0383-04 |
| State transition `DeltaIdle/DeltaActive → Returning` | UT-0383-01, UT-0383-02 |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|---|---|
| QA-0383-A | Refactor uses `.as_ref().unwrap().partition.clone()` instead of `.take()` | UT-0383-03 fires — state retained, memory leak |
| QA-0383-B | Handler rejects `DeltaIdle` as pre-state | UT-0383-02 fires — Round-0-only convergence broken |
| QA-0383-C | Handler accepts `WorkerState::Idle` (v1 variant) silently | Spec drift; not tested here. QA candidate: add a panic-or-error test for non-delta pre-states |
| QA-0383-D | Round echo gets clobbered (always emits round=0) | UT-0383-05 fires |
| QA-0383-E | Returned Vec contains a second SendMessage (e.g. accidental ack) | Test currently does not strictly count actions; QA candidate to add `actions.len() == 2` assertion |
| QA-0383-F | Handler emits `Message::PartitionResult` (v1 variant) instead of `Message::FinalStateResult` | UT-0383-01 fires (variant mismatch in match arm) |

---

## Acceptance gate

- `cargo test --workspace --lib` floor: +5 new `#[test]` fns
  (4 happy/edge-path + 1 `#[should_panic]`). Gate tolerates +5 to +6.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- No regression on v1 690-test baseline.

---

## Out of scope (deferred)

- Coordinator-side dispatch of `FinalStateRequest` → TEST-SPEC-0387.
- Wire-level integration (async caller in `protocol/worker.rs`) → 2.26-C-wire or 2.26-D.
- Retry semantics if `FinalStateResult` is dropped → SPEC-06 R25 territory.
- `Message::Shutdown` handling post-`Returning` → existing v1 FSM.
