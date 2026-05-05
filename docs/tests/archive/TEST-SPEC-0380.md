# TEST-SPEC-0380: Worker Round 0 handler — `handle_initial_partition` + `DeltaIdle` state

**Task:** TASK-0380
**Spec:** SPEC-19 §3.3 R21 phase 1 (Initial Dispatch), R22 (persistent partition state)
**Spec-critic amendments incorporated:**
- DC-C1 (ratified, option B) — fire-and-forget: NO ack `SendMessage` after `InitialPartition`
- DC-C4 (ratified, option B) — `previous_border_state` seeded from `partition.free_port_index`
**Provenance:** `docs/spec-reviews/SPEC-19-section-3.3-2.26C-design-choices-2026-04-17.md` §DC-C1, §DC-C4
**Generated:** 2026-04-17

---

## Scope note

TASK-0380 wires `Message::InitialPartition { round, partition }` into the
worker FSM. It extends `WorkerContext` with an `Option<WorkerDeltaState>`
(the struct from TASK-0379), adds a new `WorkerState::DeltaIdle` variant,
and introduces the pure handler `handle_initial_partition`.

**DC-C1 firewall:** the handler emits exactly ONE `WorkerAction::LogTransition`
and NO `WorkerAction::SendMessage`. Any regression that reintroduces an ack
would break T4 below.

**v1 untouched:** existing v1 FSM variants (`Init`, `Idle`, `Reducing`,
`Returning`, `Error`, `Done`) are NOT renumbered or removed. All existing
tests that construct `WorkerContext { ... }` literals must compile via
`..Default::default()` spread once the `delta_state` field is added.

---

## Test target file paths

- `relativist-core/src/worker.rs` — inline `#[cfg(test)] mod tests` block.
  Six new `#[test]` fns (one gated on `#[cfg(debug_assertions)]`).

All tests are synchronous. No `tokio`, no `async`.

---

## Unit Tests

### UT-0380-01: `handle_initial_partition_stores_state`

**Purpose:** Lock R22: after the handler runs, `ctx.delta_state` is
`Some(_)` and carries the exact `Partition` that was passed in.

**Target:** `worker.rs::tests`

**Given:** A default `WorkerContext` (state = `Idle`, round = 0,
`delta_state = None`); a `Partition` fixture with 3 live agents and
2 entries in `free_port_index`.

**When:** `let actions = handle_initial_partition(&mut ctx, 0, partition.clone());`

**Then:**
- `ctx.delta_state.is_some() == true`
- `ctx.delta_state.as_ref().unwrap().partition == partition`
- `ctx.delta_state.as_ref().unwrap().partition.subnet.count_live_agents() == 3`
- `ctx.delta_state.as_ref().unwrap().round == 0`

**Assertions:** Partition is moved into the state by value; no silent
re-partitioning, no field drops.

**SPEC-19 R covered:** R21 phase 1, R22.

---

### UT-0380-02: `handle_initial_partition_transitions_to_delta_idle`

**Purpose:** Lock the state transition `Idle → DeltaIdle` per R21 phase 1.

**Target:** `worker.rs::tests`

**Given:** `ctx.state == WorkerState::Idle` pre-call.

**When:** Call `handle_initial_partition(&mut ctx, 0, partition)`.

**Then:** `ctx.state == WorkerState::DeltaIdle`.

**Assertions:** v1 transitions (`Idle → Reducing`, `Reducing → Returning`)
are unaffected. `DeltaIdle` is distinct from every v1 variant.

**SPEC-19 R covered:** R21 phase 1.

---

### UT-0380-03: `handle_initial_partition_emits_log_transition_only`

**Purpose:** DC-C1 firewall — the returned `Vec<WorkerAction>` contains
exactly ONE `LogTransition { from: Idle, to: DeltaIdle }` and ZERO
`SendMessage` variants. Regression guard against future reintroduction
of an ack message.

**Target:** `worker.rs::tests`

**Given:** Default `WorkerContext`, any legal partition.

**When:** `let actions = handle_initial_partition(&mut ctx, 0, partition);`

**Then:**
- `actions.len() == 1`
- `matches!(actions[0], WorkerAction::LogTransition { .. })` is `true`
- The single `LogTransition` has `from == WorkerState::Idle` and
  `to == WorkerState::DeltaIdle`
- `actions.iter().any(|a| matches!(a, WorkerAction::SendMessage(_))) == false`

**Assertions:** NO `SendMessage` action under any circumstance (DC-C1
fire-and-forget contract). If a future refactor adds an ack path, this
test fires.

**SPEC-19 R covered:** R21 phase 1 + DC-C1 (ratified).

---

### UT-0380-04: `handle_initial_partition_seeds_previous_border_state`

**Purpose:** Lock DC-C4 — after the handler runs, `delta_state.previous_border_state`
equals `partition.free_port_index` (not empty, not re-derived).

**Target:** `worker.rs::tests`

**Given:** `Partition` with 2 border entries:
`free_port_index = {5 → AgentPort(1, 0), 7 → AgentPort(2, 1)}`.

**When:** Call handler.

**Then:**
- `ctx.delta_state.as_ref().unwrap().previous_border_state.len() == 2`
- `ctx.delta_state.as_ref().unwrap().previous_border_state == partition.free_port_index`
- `ctx.delta_state.as_ref().unwrap().previous_border_state[&5] == PortRef::AgentPort(AgentId(1), 0)`

**Assertions:** If DC-C4 is ever flipped to option A (empty seed), this
test fires immediately.

**SPEC-19 R covered:** R25 + DC-C4 (ratified).

---

### UT-0380-05: `handle_initial_partition_round_nonzero_panics_in_debug`  `#[cfg(debug_assertions)]`

**Purpose:** Enforce R21.1's guarantee that `InitialPartition` MUST arrive
at Round 0 only. Ship as `debug_assert!` so release builds accept the
value silently (caller contract violation, not a user-facing error).

**Target:** `worker.rs::tests`

**Given:** Any legal partition.

**When:** `handle_initial_partition(&mut ctx, 1, partition);` — note
`round == 1` (invalid).

**Then:** `#[should_panic(expected = "InitialPartition MUST arrive at round 0")]`
fires under `#[cfg(debug_assertions)]`.

**Assertions:** Panic message carries R21.1 breadcrumb (aids debug).

**SPEC-19 R covered:** R21 phase 1 (caller invariant).

**Note:** Test is gated on `#[cfg(debug_assertions)]` so it only runs in
debug mode. Release mode accepts round != 0 silently (per task spec's
`debug_assert!` choice).

---

### UT-0380-06: `worker_context_default_has_none_delta_state`

**Purpose:** Lock backwards compatibility — `WorkerContext::default()`
produces `delta_state = None`. Any v1 test that uses `..Default::default()`
spread continues to compile and behave.

**Target:** `worker.rs::tests`

**Given:** Call `WorkerContext::default()`.

**When:** Inspect fields.

**Then:**
- `ctx.state == WorkerState::Init`
- `ctx.round == 0`
- `ctx.delta_state.is_none() == true`

**Assertions:** All three v1 defaults preserved; new field defaults to
`None`. Regression guard on the `Default` impl.

**SPEC-19 R covered:** R22 (opt-in delta state; v1 unchanged).

---

## Coverage mapping

| Requirement / DC | Covered by |
|---|---|
| R21 phase 1 — InitialPartition handling | UT-0380-01, UT-0380-02, UT-0380-05 |
| R22 — persistent partition state stored | UT-0380-01 |
| R25 — `previous_border_state` seeded | UT-0380-04 |
| DC-C1 (ratified) — no ack `SendMessage` | UT-0380-03 |
| DC-C4 (ratified) — seed from `free_port_index` | UT-0380-04 |
| `WorkerState::DeltaIdle` variant added | UT-0380-02 |
| `delta_state: Option<WorkerDeltaState>` default `None` | UT-0380-06 |
| v1 variants / `Default` preserved | UT-0380-06 |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|---|---|
| QA-0380-A | Future refactor adds `SendMessage(InitialPartitionAck)` | UT-0380-03 fires — DC-C1 firewall |
| QA-0380-B | `Default for WorkerContext` accidentally drops `delta_state` field | UT-0380-06 compile-time failure or runtime `None` check fires |
| QA-0380-C | `WorkerState::DeltaIdle` accidentally merged with `Idle` (variant removal) | UT-0380-02 fails — distinct variants required |
| QA-0380-D | Handler called with round=1 in release mode | No panic (debug_assert); silently overwrites state. Flag at Stage 5 for spec-critic if defensive check is wanted in release |
| QA-0380-E | Double-invocation of the handler (coordinator retry) | Current draft silently overwrites `ctx.delta_state`. Not tested here; noted as open question for spec-critic |

---

## Acceptance gate

- `cargo test --workspace --lib` floor: +5 `#[test]` fns unconditionally
  (+1 more under `#[cfg(debug_assertions)]`); gate tolerates +5 to +6.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- No regression on v1 690-test baseline. All existing `WorkerContext { ... }`
  literals in the workspace must compile (via `..Default::default()` spread
  if needed).

---

## Out of scope (deferred)

- `handle_round_start` (delta-round body) → TEST-SPEC-0381.
- `compute_outgoing_deltas` helper → TEST-SPEC-0382.
- `handle_final_state_request` → TEST-SPEC-0383.
- Wire-layer integration (async `protocol/worker.rs` read-loop) → 2.26-C-wire or 2.26-D.
- Double-invocation / retry semantics → spec-critic open question.
