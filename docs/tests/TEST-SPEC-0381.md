# TEST-SPEC-0381: Worker delta-round handler — `handle_round_start` + `apply_border_deltas_to_partition`

**Task:** TASK-0381
**Spec:** SPEC-19 §3.3 R23 (`RoundStart` payload), R24 (apply → reduce →
  rebuild → diff → report pipeline), R26 (`RoundResult` payload shape),
  R25 (update `previous_border_state`).
**Spec-critic amendments incorporated:**
- DC-C4 (ratified) — round-1 diff runs against the Round 0 seed from `free_port_index`
- DC-C6 (locked in, option C) — disconnection = `crate::net::DISCONNECTED` sentinel
- DC-C3 strict-only note — this task's `reduce_all` path does NOT depend on `strict_bsp`;
  lenient/strict branching lives at the coordinator (TASK-0385)
**Provenance:** `docs/spec-reviews/SPEC-19-section-3.3-2.26C-design-choices-2026-04-17.md`
**Generated:** 2026-04-17

---

## Scope note

TASK-0381 ships the "round body" handler that runs the R24 five-step
pipeline: apply deltas → `reduce_all` → rebuild `free_port_index` → diff
via TASK-0382's helper → emit `Message::RoundResult`.

The task ALSO ships the pure-core helper
`apply_border_deltas_to_partition` in `merge/helpers.rs`, exercised by
its own inline tests below (UT-0381-10 through UT-0381-13).

The handler MUST emit exactly two actions: one `LogTransition` and one
`SendMessage(Box::new(Message::RoundResult { .. }))`. The round field on
the outgoing message MUST match the incoming `round`.

**DC-C6 pin:** any border that local reduction erased (agent consumed,
no endpoint remains) MUST surface as
`BorderDelta { border_id, new_target: crate::net::DISCONNECTED }`.
The sentinel is `PortRef::FreePort(u32::MAX)`; tests MUST NOT use
`PortRef::FreePort(u32::MAX)` literally — use the named constant.

---

## Test target file paths

- `relativist-core/src/worker.rs` — inline `#[cfg(test)] mod tests`.
  Nine new `#[test]` fns (UT-0381-01..09) for `handle_round_start`.
- `relativist-core/src/merge/helpers.rs` — inline `#[cfg(test)] mod tests`.
  Four new `#[test]` fns (UT-0381-10..13) for `apply_border_deltas_to_partition`.

All tests are synchronous. No `tokio`, no `async`.

---

## Unit Tests — `handle_round_start` (`worker.rs`)

### UT-0381-01: `handle_round_start_empty_deltas_no_reduction`

**Purpose:** Happy-path quiet round. A partition with no local redexes,
empty `border_deltas / resolved_borders / new_borders`, pre-state
`DeltaIdle`, seeded `previous_border_state` identical to current
`free_port_index`. Expected: `outgoing_deltas.is_empty()`,
`stats.local_redexes == 0`, `has_border_activity == false`.

**Target:** `worker.rs::tests`

**Given:** Worker in `DeltaIdle` with a normalized partition (no redexes);
`previous_border_state == free_port_index`.

**When:** `handle_round_start(&mut ctx, 1, vec![], vec![], vec![])`.

**Then:**
- Returned `Message::RoundResult.border_deltas.is_empty() == true`
- `Message::RoundResult.stats.local_redexes == 0`
- `Message::RoundResult.has_border_activity == false`
- `ctx.state == WorkerState::DeltaActive`
- `ctx.round == 1`

**Assertions:** Quiet round generates zero outgoing delta traffic
(matches DC-C5 convergence signal).

**SPEC-19 R covered:** R24 full pipeline with no-op inputs, R25
(unchanged `previous_border_state` → no diff), R26 (shape).

---

### UT-0381-02: `handle_round_start_applies_border_delta_reconnect`

**Purpose:** R23 case 1 (`border_deltas`) — a single reconnect.

**Target:** `worker.rs::tests`

**Given:** Partition where `border_id = 5` is currently connected to
`AgentPort(a, 1)`; `border_deltas = vec![BorderDelta { border_id: 5,
new_target: PortRef::AgentPort(AgentId(b), 0) }]`; empty
`resolved_borders`, `new_borders`.

**When:** Call handler.

**Then:**
- After call, `ctx.delta_state.as_ref().unwrap().partition.free_port_index[&5]
  == PortRef::AgentPort(AgentId(b), 0)`.

**Assertions:** The reconnect is applied pre-reduction (R24.1 ordering).

**SPEC-19 R covered:** R23 case 1, R24.1.

---

### UT-0381-03: `handle_round_start_removes_resolved_border`

**Purpose:** R23 case 2 (`resolved_borders`) — a border ID is erased
from `free_port_index`.

**Target:** `worker.rs::tests`

**Given:** Partition with `border_id = 7` present in `free_port_index`;
`resolved_borders = vec![7]`; other input vecs empty.

**When:** Call handler.

**Then:**
- `ctx.delta_state.as_ref().unwrap().partition.free_port_index.contains_key(&7) == false`.

**Assertions:** Erasure applied pre-reduction. Any wire carrying
`FreePort(7)` is cleaned up (structural — exact subnet state is
implementation-defined, the public contract is `free_port_index`).

**SPEC-19 R covered:** R23 case 2, R24.1.

---

### UT-0381-04: `handle_round_start_adds_new_border`

**Purpose:** R23 case 3 (`new_borders`) — a newly-created border from
coordinator-side CON-DUP expansion is inserted.

**Target:** `worker.rs::tests`

**Given:** `new_borders = vec![(9, PortRef::AgentPort(AgentId(c), 0))]`;
other input vecs empty.

**When:** Call handler.

**Then:**
- `ctx.delta_state.as_ref().unwrap().partition.free_port_index[&9]
  == PortRef::AgentPort(AgentId(c), 0)`.

**Assertions:** Insertion applied pre-reduction. A `FreePort(9)` wire
is present in the subnet post-apply (structural).

**SPEC-19 R covered:** R23 case 3, R24.1.

---

### UT-0381-05: `handle_round_start_reduces_and_reports_interactions`

**Purpose:** R24.2 locked: `reduce_all` fires and `WorkerRoundStats`
accumulates interactions.

**Target:** `worker.rs::tests`

**Given:** Partition with exactly one CON-CON active pair (both agents
live, connected principal-to-principal); empty delta inputs.

**When:** Call handler.

**Then:**
- `Message::RoundResult.stats.local_redexes == 1`
- `Message::RoundResult.stats.interactions_by_rule[CON_CON_INDEX] == 1`
  (where CON_CON_INDEX is the rule index per SPEC-03).

**Assertions:** Pipeline runs in order apply → reduce; `interactions_by_rule`
accumulates correctly.

**SPEC-19 R covered:** R24.2, R26.

---

### UT-0381-06: `handle_round_start_updates_previous_border_state`

**Purpose:** R25 — after one round, `previous_border_state` equals the
post-rebuild `free_port_index`.

**Target:** `worker.rs::tests`

**Given:** Worker with seeded state; a round where at least one border
changes.

**When:** Call handler.

**Then:**
- `ctx.delta_state.as_ref().unwrap().previous_border_state ==
   ctx.delta_state.as_ref().unwrap().partition.free_port_index`.

**Assertions:** Next round's diff operates from this new baseline.
Regression guard against a forgotten update (would emit duplicate
deltas next round).

**SPEC-19 R covered:** R25.

---

### UT-0381-07: `handle_round_start_transitions_to_delta_active`

**Purpose:** State transition `DeltaIdle → DeltaActive` on the first
round; idempotent `DeltaActive → DeltaActive` on subsequent rounds.

**Target:** `worker.rs::tests`

**Given:** (a) Worker pre-state = `DeltaIdle`; (b) second invocation
pre-state = `DeltaActive`.

**When:** Call handler twice with `round = 1` then `round = 2`.

**Then:**
- After first call: `ctx.state == WorkerState::DeltaActive`, `ctx.round == 1`.
- After second call: `ctx.state == WorkerState::DeltaActive`, `ctx.round == 2`.

**Assertions:** No regression from `DeltaActive` back to `DeltaIdle`.

**SPEC-19 R covered:** R21 phase 2.

---

### UT-0381-08: `handle_round_start_emits_log_transition_and_send_round_result`

**Purpose:** DC-C1 echo + R26 — returned `Vec<WorkerAction>` contains
exactly two items: one `LogTransition`, one `SendMessage` wrapping
`Message::RoundResult { .. }` with the correct round.

**Target:** `worker.rs::tests`

**Given:** Any legal round input.

**When:** Call handler with `round = 7`.

**Then:**
- `actions.len() == 2`
- `matches!(actions[0], WorkerAction::LogTransition { .. }) == true`
- `matches!(actions[1], WorkerAction::SendMessage(msg) if matches!(**msg, Message::RoundResult { round: 7, .. })) == true`

**Assertions:** Round number is echoed on the outgoing message (so the
coordinator can pair reply to dispatch).

**SPEC-19 R covered:** R26.

---

### UT-0381-09: `handle_round_start_emits_disconnect_sentinel_for_erased_border`  (DC-C6)

**Purpose:** Lock DC-C6 option C — when local reduction erases an
agent connected to a border, the outgoing `BorderDelta` has
`new_target == crate::net::DISCONNECTED`. Regression guard against
accidental use of `PortRef::FreePort(K)` (border-id-indexed) or
`u32::MAX` literal.

**Target:** `worker.rs::tests`

**Given:** Partition with one agent A whose principal port is
connected to `FreePort(42)` and whose aux port forms an active pair
with another erase-producing configuration (so `reduce_all` removes
A). `previous_border_state[&42]` is set to `AgentPort(A, 0)`.

**When:** Call handler with empty delta inputs.

**Then:**
- Outgoing `Message::RoundResult.border_deltas` contains at least one
  entry with `border_id == 42` and
  `new_target == crate::net::DISCONNECTED`.
- The literal `PortRef::FreePort(u32::MAX)` does NOT appear anywhere
  in the assertion — the NAMED constant MUST be used.

**Assertions:** The outgoing sentinel is the DISCONNECTED constant
from `crate::net`, not a hand-rolled `FreePort(u32::MAX)`.

**SPEC-19 R covered:** R25 + DC-C6 (locked in, option C).

---

## Unit Tests — `apply_border_deltas_to_partition` (`merge/helpers.rs`)

### UT-0381-10: `apply_border_deltas_to_partition_empty_inputs_preserves_agent_count`

**Purpose:** All-empty inputs are a no-op on the subnet; agent count
unchanged.

**Target:** `merge/helpers.rs::tests`

**Given:** Partition with 5 live agents, 3 border entries; all three
input slices empty.

**When:** `apply_border_deltas_to_partition(&mut p, &[], &[], &[])`.

**Then:**
- `p.subnet.count_live_agents() == 5`
- `p.free_port_index.len() == 3` (unchanged)

**Assertions:** Empty inputs are pure identity on the partition.

**SPEC-19 R covered:** R23 (identity).

---

### UT-0381-11: `apply_border_deltas_to_partition_reconnect_updates_free_port_index`

**Purpose:** R23 case 1 at the helper level — reconnect updates
`free_port_index[bid]` to the new target.

**Target:** `merge/helpers.rs::tests`

**Given:** Partition with `free_port_index = {5 → AgentPort(1, 0)}`.

**When:** `apply_border_deltas_to_partition(&mut p,
&[BorderDelta { border_id: 5, new_target: PortRef::AgentPort(AgentId(9), 1) }],
&[], &[])`.

**Then:**
- `p.free_port_index[&5] == PortRef::AgentPort(AgentId(9), 1)`.

**Assertions:** Index map is updated exactly, no other key touched.

**SPEC-19 R covered:** R23 case 1.

---

### UT-0381-12: `apply_border_deltas_to_partition_resolved_borders_removes_entry`

**Purpose:** R23 case 2 at the helper level — resolved IDs are
removed from `free_port_index`.

**Target:** `merge/helpers.rs::tests`

**Given:** Partition with `free_port_index = {5 → X, 7 → Y, 9 → Z}`.

**When:** `apply_border_deltas_to_partition(&mut p, &[], &[7, 9], &[])`.

**Then:**
- `p.free_port_index.contains_key(&7) == false`
- `p.free_port_index.contains_key(&9) == false`
- `p.free_port_index[&5] == X` (unchanged)
- `p.free_port_index.len() == 1`.

**Assertions:** Multiple removals in one call; unrelated keys preserved.

**SPEC-19 R covered:** R23 case 2.

---

### UT-0381-13: `apply_border_deltas_to_partition_new_borders_inserts_entry`

**Purpose:** R23 case 3 at the helper level — new borders are added.

**Target:** `merge/helpers.rs::tests`

**Given:** Partition with empty `free_port_index`.

**When:** `apply_border_deltas_to_partition(&mut p, &[], &[],
&[(11, PortRef::AgentPort(AgentId(2), 0)),
   (12, PortRef::FreePort(12))])`.

**Then:**
- `p.free_port_index.len() == 2`
- `p.free_port_index[&11] == PortRef::AgentPort(AgentId(2), 0)`
- `p.free_port_index[&12] == PortRef::FreePort(12)`.

**Assertions:** Insertion idempotent (repeat insert of same key
overwrites; not asserted here — out of scope for DC-C6).

**SPEC-19 R covered:** R23 case 3.

---

## Coverage mapping

| Requirement / DC | Covered by |
|---|---|
| R23 case 1 (border_deltas reconnect) | UT-0381-02, UT-0381-11 |
| R23 case 2 (resolved_borders erase) | UT-0381-03, UT-0381-12 |
| R23 case 3 (new_borders insert) | UT-0381-04, UT-0381-13 |
| R24 pipeline (apply → reduce → rebuild → diff → report) | UT-0381-01, UT-0381-05 |
| R25 (previous_border_state update) | UT-0381-06 |
| R26 (RoundResult shape + round echo) | UT-0381-01, UT-0381-08 |
| R21 phase 2 (state transition) | UT-0381-07 |
| DC-C6 option C (DISCONNECTED sentinel) | UT-0381-09 |
| Identity over empty inputs | UT-0381-10 |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|---|---|
| QA-0381-A | Pipeline order swap (reduce before apply) | UT-0381-02/03/04 fail — deltas missed |
| QA-0381-B | `previous_border_state` not updated | UT-0381-06 fires — next round emits duplicate deltas |
| QA-0381-C | Release mode reorders `apply → rebuild` → `reduce` | R24 ordering broken; UT-0381-05 likely still passes, UT-0381-06 fires |
| QA-0381-D | DISCONNECTED changed to `PortRef::FreePort(K)` (border-id-indexed) | UT-0381-09 fires — sentinel drift |
| QA-0381-E | Handler accidentally emits `SendMessage(Message::Error)` on normal path | UT-0381-08 fires — wrong variant |
| QA-0381-F | `new_borders` tuple ordering (`(PortRef, u32)` vs `(u32, PortRef)`) flipped | UT-0381-13 compile-time failure |
| QA-0381-G | `ctx.state` accidentally reset to `DeltaIdle` on repeat call | UT-0381-07 fires |

---

## Acceptance gate

- `cargo test --workspace --lib` floor: +13 new `#[test]` fns
  (9 in `worker.rs`, 4 in `merge/helpers.rs`). Gate tolerates +13 to +16
  if developer splits any case for readability.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- No regression on v1 690-test baseline.

---

## Out of scope (deferred)

- `compute_outgoing_deltas` helper (called in step 4 of R24) → TEST-SPEC-0382.
- `handle_final_state_request` → TEST-SPEC-0383.
- Coordinator-side round loop driving this handler → TEST-SPEC-0385.
- Wire-level integration (async `protocol/worker.rs` caller) → 2.26-C-wire or 2.26-D.
- `strict_bsp × delta_mode` branching (DC-C3) → TEST-SPEC-0384 / TEST-SPEC-0385.
