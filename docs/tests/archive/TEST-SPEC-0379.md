# TEST-SPEC-0379: `WorkerDeltaState` struct + `from_initial_partition` constructor

**Task:** TASK-0379
**Spec:** SPEC-19 §3.3 R22 (persistent partition across rounds), R25 (`previous_border_state` map seed from `partition.free_port_index`)
**Spec-critic amendment:** DC-C4 (ratified) — `previous_border_state` seeded from `partition.free_port_index` at storage time, NOT empty
**Provenance:** `docs/spec-reviews/SPEC-19-section-3.3-2.26C-design-choices-2026-04-17.md` §DC-C4
**Generated:** 2026-04-17

---

## Scope note

TASK-0379 is a **pure-data** task. It adds the `WorkerDeltaState` struct to
`worker.rs`, its `from_initial_partition` constructor, and inline unit tests.
No wire integration, no FSM changes, no runtime handler plumbing. That is
TASK-0380/0381/0383.

Per DC-C4 ratification (option B): the constructor seeds
`previous_border_state` from `partition.free_port_index.clone()` at storage
time so that Round 1's first delta dispatch reports only border endpoints
that local reduction actually moved (coordinator already knows the initial
map via `BorderGraph::from_partition_plan`).

---

## Test target file paths

- `relativist-core/src/worker.rs` — inline `#[cfg(test)] mod tests` block (existing). Four new `#[test]` fns.

All tests are synchronous. No `tokio`, no `async`.

---

## Unit Tests

### UT-0379-01: `workerdeltastate_struct_shape_locked_per_r22_r25`

**Purpose:** Compile-time contract that `WorkerDeltaState` has exactly the three fields R22/R25 mandate (`partition: Partition`, `previous_border_state: HashMap<u32, PortRef>`, `round: u32`), all `pub`. Breaks compilation if a field is added, renamed, removed, or privatised.

**Target:** `worker.rs::tests`

**Given:** `WorkerDeltaState`, `Partition`, `PortRef` imported; a minimally populated `Partition` fixture.

**When:** Construct a `WorkerDeltaState` via full struct literal.

**Then:** Struct literal compiles; field reads yield written values; types match R22/R25.

**Assertions:**
- Field set locked to three names.
- `round: u32` (not `usize`, not `u64`).
- `previous_border_state: HashMap<u32, PortRef>` — `PortRef` value type (NOT wrapped in `Option`, per DC-C6).

**SPEC-19 R covered:** R22, R25 (shape).

---

### UT-0379-02: `workerdeltastate_from_initial_partition_stores_partition`

**Purpose:** Lock R22 storage: constructor preserves the passed-in partition byte-identically.

**Target:** `worker.rs::tests`

**Given:** A `Partition` with 3 live agents and 2 entries in `free_port_index`.

**When:** `let state = WorkerDeltaState::from_initial_partition(partition.clone());`

**Then:**
- `state.partition.subnet.count_live_agents() == 3`
- `state.round == 0`
- `state.partition == partition` (full struct equality).

**Assertions:** Storage is by-value move; no field drops, no silent re-partitioning.

**SPEC-19 R covered:** R22.

---

### UT-0379-03: `workerdeltastate_from_initial_partition_seeds_previous_border_state_per_dc_c4`

**Purpose:** Lock DC-C4 option B ratification: seed map is a clone of `partition.free_port_index`, NOT empty.

**Target:** `worker.rs::tests`

**Given:** `Partition` with 2 border entries: `free_port_index = {5 → AgentPort(1, 0), 7 → AgentPort(2, 1)}`.

**When:** Build state via `from_initial_partition`.

**Then:**
- `state.previous_border_state.len() == 2`
- `state.previous_border_state == state.partition.free_port_index`
- `state.previous_border_state[&5] == PortRef::AgentPort(AgentId(1), 0)`
- `state.previous_border_state[&7] == PortRef::AgentPort(AgentId(2), 1)`.

**Assertions:** If DC-C4 is ever overridden to option A (empty seed), this test fires immediately.

**SPEC-19 R covered:** R25 + DC-C4 (ratified).

---

### UT-0379-04: `workerdeltastate_from_initial_partition_empty_freeports`

**Purpose:** Edge case — partition with empty `free_port_index` yields state with empty seed map, not a panic.

**Target:** `worker.rs::tests`

**Given:** `Partition` with `free_port_index.is_empty() == true` (monolithic partition, no borders).

**When:** Build state.

**Then:**
- `state.previous_border_state.is_empty() == true`
- `state.round == 0`
- No panic.

**Assertions:** The constructor is total over legal partitions.

**SPEC-19 R covered:** R25 (boundary).

---

### UT-0379-05: `workerdeltastate_clone_is_deep`

**Purpose:** `Clone` on `WorkerDeltaState` must deep-clone the inner `HashMap` so that mutating the clone does not affect the original (safety net for TASK-0381's mutation path).

**Target:** `worker.rs::tests`

**Given:** A seeded state with `previous_border_state.len() == 1`.

**When:** `let mut cloned = state.clone(); cloned.previous_border_state.clear();`

**Then:** `state.previous_border_state.len() == 1` (original untouched).

**Assertions:** `#[derive(Clone)]` on the struct gives a deep clone for `HashMap<u32, PortRef>` (stdlib guarantee; test is the regression guard).

**SPEC-19 R covered:** R22/R25 (ownership semantics).

---

### UT-0379-06: `workerdeltastate_round_initialised_to_zero`

**Purpose:** R22 demands the constructor leaves the worker in Round 0 pre-first-`RoundStart`. Pinned explicitly so TASK-0381 can trust it.

**Target:** `worker.rs::tests`

**Given:** Any legal partition.

**When:** Build state.

**Then:** `state.round == 0u32`.

**Assertions:** No other path sets `round`; it is initialised exclusively here.

**SPEC-19 R covered:** R22 (round initialisation).

---

## Coverage mapping

| Requirement / DC | Covered by |
|---|---|
| R22 — persistent partition stored in worker | UT-0379-02, UT-0379-06 |
| R25 — `previous_border_state` exists + typed | UT-0379-01, UT-0379-03 |
| DC-C4 (ratified) — seed from `free_port_index` | UT-0379-03, UT-0379-04 |
| Struct shape locked to three fields | UT-0379-01 |
| Deep-clone semantics | UT-0379-05 |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|---|---|
| QA-0379-A | Future refactor changes `round: u32` to `round: u64` | UT-0379-01 fails at compile time (struct literal) |
| QA-0379-B | Future refactor drops `#[derive(Clone)]` | UT-0379-05 fails to compile |
| QA-0379-C | Future refactor accidentally seeds empty map (reverts DC-C4) | UT-0379-03 fires |
| QA-0379-D | `Partition.free_port_index` type changes from `HashMap<u32, PortRef>` to `BTreeMap` | UT-0379-01 fires (type mismatch in literal) |

---

## Acceptance gate

- `cargo test --workspace --lib` floor: +4 to +6 `#[test]` fns over the pre-0379 baseline.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- No regression on v1 690-test baseline.

---

## Out of scope (deferred)

- `handle_initial_partition` runtime handler → TEST-SPEC-0380.
- `DeltaIdle` / `DeltaActive` FSM variants → TEST-SPEC-0380 / -0381.
- `compute_outgoing_deltas` diff helper → TEST-SPEC-0382.
