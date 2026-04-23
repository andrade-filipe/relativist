# TEST-SPEC-0394: Worker-side R23/R26 completion — `local_reconnections` + DC-B5 2-phase `pending_commutations → minted_agents`

**See also:** [docs/backlog/TASK-0394.md](../backlog/TASK-0394.md)
  — MF-001 closure; review at `docs/reviews/REVIEW-SPEC-19-section-3.3-3.5-3.6-item-2.26-BCD-2026-04-23.md`.

**Task:** TASK-0394
**Spec:** SPEC-19 §3.3 R23 (full 7-field `RoundStart` payload consumption), R26 (`minted_agents` echo), R48 (protocol-invariant check on stray `request_id`), DC-B5 (2-phase agent allocation — second half on worker side).
**Spec-critic verdicts consumed:**
  - **DC-B5 (ratified 2026-04-17):** coordinator emits `CommutationBatch` with reserved `slot_marker` placeholders; worker allocates from `partition.id_range` and echoes `minted_agents` keyed by `request_id`. The coordinator's round-N+2 finalizer resolves `PendingPortRef::Pending` tokens using the echo.
  - **DC-0394-A (proposed):** `slot_hint` on `PendingCommutation` is advisory; this task ignores it and allocates from `partition.id_range`.
  - **DC-0394-B (proposed):** `minted_agents` preserves `pending_commutations` input order (MUST).
**Generated:** 2026-04-23
**Baseline before this task:** 1039 lib (default) / 1079 lib (`--features zero-copy`) — the 2.26-B/C/D DEV-complete snapshot.
**Cumulative target after this task:** 1050 lib / 1090 lib — **+11** new `#[test]` fns in `worker.rs`.

---

## Scope note

This TEST-SPEC exercises the worker-side completion of the R23/R26 wire contract. Existing UT-0381-01..09 tests remain valid (they pass empty vectors for the two new fields and still assert the 5-field slice). The 11 new tests below (UT-0394-01..11) cover three axes:

1. **`local_reconnections` application** (UT-0394-01..04) — R23 DC-B3 split faithful at the worker.
2. **`pending_commutations → minted_agents` 2-phase echo** (UT-0394-05..09) — R26 DC-B5 second half.
3. **Error paths and invariants** (UT-0394-10..11) — `id_range` exhaustion + protocol invariant preservation.

Integration-level G1 parity is scoped to TASK-0395 (TEST-SPEC-0385 UT-0385-06..08 completion). This TEST-SPEC stays at the unit level and at the worker-handler boundary.

**Backward-compat hook:** all 9 prior UT-0381-01..09 tests MUST be updated mechanically to add `vec![]` for the two new parameters. This is NOT counted toward the `+11` test-count delta — the updates preserve existing test behavior.

---

## Test target file paths

- `relativist-core/src/worker.rs` — inline `#[cfg(test)] mod tests` block:
  - 11 new `#[test]` fns: UT-0394-01..11.
  - Mechanical touch on UT-0381-01..09 to add 2 new `vec![]` parameters.

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Unit Tests — `handle_round_start` extensions

### UT-0394-01: `handle_round_start_empty_local_reconnections_empty_pending_commutations_unchanged`

**Purpose:** Backward-compat sanity: when both new vectors are empty, the handler's output is byte-equivalent to what UT-0381-01 observed.

**Target:** `worker.rs::tests`

**Given:** Worker in `DeltaIdle` with a normalized partition; `previous_border_state == free_port_index`.

**When:** `handle_round_start(&mut ctx, 1, vec![], vec![], vec![], vec![], vec![])`.

**Then:**
- Returned `Message::RoundResult.border_deltas.is_empty() == true`.
- `Message::RoundResult.stats.local_redexes == 0`.
- `Message::RoundResult.has_border_activity == false`.
- `Message::RoundResult.minted_agents.is_empty() == true`.
- `ctx.state == WorkerState::DeltaActive`.
- `ctx.round == 1`.
- No change to `state.partition.subnet` vs. pre-call.

**SPEC-19 R covered:** R23 (empty-payload path), R26 (empty-echo).

---

### UT-0394-02: `handle_round_start_applies_single_local_reconnection`

**Purpose:** R23 DC-B3 `local_reconnections` — a single reconnection pair mutates `state.partition.subnet` before `reduce_all`.

**Target:** `worker.rs::tests`

**Given:** Partition with 2 live agents `a` (CON) and `b` (CON), no wires between them. `local_reconnections = vec![LocalReconnection { left: PortRef::AgentPort(a, 1), right: PortRef::AgentPort(b, 2) }]`. Empty `border_deltas`, `resolved_borders`, `new_borders`, `pending_commutations`.

**When:** `handle_round_start(&mut ctx, 1, vec![], vec![], vec![], local_reconnections, vec![])`.

**Then:**
- `state.partition.subnet.get_target(PortRef::AgentPort(a, 1)) == PortRef::AgentPort(b, 2)`.
- `state.partition.subnet.get_target(PortRef::AgentPort(b, 2)) == PortRef::AgentPort(a, 1)`.
- `Message::RoundResult.stats.local_redexes == 0` (CON.1 ↔ CON.2 is not an active pair).

**Assertions:** The connection is visible post-call AND symmetric.

**SPEC-19 R covered:** R23 (DC-B3 split), R24 step 1.5 (local_reconnections applied between border_deltas and reduce_all).

---

### UT-0394-03: `handle_round_start_applies_multiple_local_reconnections_in_order`

**Purpose:** N-pair `local_reconnections` are applied sequentially; if two pairs touch the same port, the LAST one wins.

**Target:** `worker.rs::tests`

**Given:** Three agents `a`, `b`, `c`. `local_reconnections = vec![ LR{a.1 ↔ b.0}, LR{a.1 ↔ c.0} ]` (two pairs both touching `a.1`; the second overrides).

**When:** `handle_round_start(&mut ctx, 2, vec![], vec![], vec![], local_reconnections, vec![])`.

**Then:**
- `state.partition.subnet.get_target(PortRef::AgentPort(a, 1)) == PortRef::AgentPort(c, 0)` (last-write-wins).
- `state.partition.subnet.get_target(PortRef::AgentPort(b, 0)) == DISCONNECTED` (the first pair's counterpart is orphaned when overridden).
- `state.partition.subnet.get_target(PortRef::AgentPort(c, 0)) == PortRef::AgentPort(a, 1)`.

**Assertions:** Sequential application, not "batch atomic" (DC-B3 resolver contract); last-write-wins is explicit behavior, not accidental.

**SPEC-19 R covered:** R23 (DC-B3), R24 step 1.5.

**Note:** If spec-critic later rules "duplicate pairs are a resolver-side bug, worker should panic", this test flips to assert the panic message instead. Currently: worker is lenient.

---

### UT-0394-04: `handle_round_start_skips_disconnected_and_self_loops_in_local_reconnections`

**Purpose:** Defensive — `DISCONNECTED` sentinel and self-loop pairs in `local_reconnections` are silently skipped with a `tracing::trace!`.

**Target:** `worker.rs::tests`

**Given:** `local_reconnections = vec![
    LR { left: PortRef::AgentPort(a, 1), right: crate::net::DISCONNECTED },
    LR { left: PortRef::AgentPort(a, 1), right: PortRef::AgentPort(a, 1) },  // self-loop
    LR { left: PortRef::AgentPort(b, 0), right: PortRef::AgentPort(c, 0) },  // valid
]`.

**When:** `handle_round_start(&mut ctx, 1, vec![], vec![], vec![], lrs, vec![])`.

**Then:**
- `state.partition.subnet.get_target(PortRef::AgentPort(a, 1))` remains at its pre-call value (first pair skipped — DISCONNECTED endpoint skipped; second pair also skipped — self-loop).
- `state.partition.subnet.get_target(PortRef::AgentPort(b, 0)) == PortRef::AgentPort(c, 0)` (third pair applied).
- No panic fires.

**Assertions:** Silent skip (with tracing trace, NOT panic) is the contract for defensive endpoints from the resolver.

**SPEC-19 R covered:** R23 (robust handling of degenerate resolver output).

---

### UT-0394-05: `handle_round_start_mints_agent_for_single_pending_commutation`

**Purpose:** DC-B5 second half — one `PendingCommutation` yields one minted agent with matching `request_id`.

**Target:** `worker.rs::tests`

**Given:** Empty local state except `partition.id_range = IdRange { start: 100, end: 200 }`. `pending_commutations = vec![PendingCommutation { request_id: CommutationId(42), symbol_type: Symbol::Con, slot_hint: SLOT_MARKER_BASE + 3 }]`.

**When:** `handle_round_start(&mut ctx, 1, vec![], vec![], vec![], vec![], pending_commutations)`.

**Then:**
- `Message::RoundResult.minted_agents.len() == 1`.
- `minted_agents[0].request_id == CommutationId(42)`.
- `minted_agents[0].minted_agent_id == AgentId(100)` (first ID in range).
- `minted_agents[0].symbol_type == Symbol::Con`.
- `state.partition.subnet.get_agent(AgentId(100)).symbol == Symbol::Con`.
- All 3 ports of the new agent are `DISCONNECTED`.
- `state.partition.id_range.next_available == 101`.

**Assertions:** Worker allocates from its own range (DC-B5 non-overlap guarantee); minted agent exists with correct symbol; ports are unwired (round N+2 will wire them).

**SPEC-19 R covered:** R26 (minted_agents echo), DC-B5 (2-phase allocation).

---

### UT-0394-06: `handle_round_start_mints_multiple_agents_in_same_round`

**Purpose:** N `PendingCommutation`s yield N minted agents; IDs are contiguous from `id_range`.

**Target:** `worker.rs::tests`

**Given:** `partition.id_range = IdRange { start: 100, end: 200 }`. `pending_commutations = vec![
    PendingCommutation { request_id: CommutationId(1), symbol_type: Symbol::Con, slot_hint: 0 },
    PendingCommutation { request_id: CommutationId(2), symbol_type: Symbol::Dup, slot_hint: 0 },
    PendingCommutation { request_id: CommutationId(3), symbol_type: Symbol::Era, slot_hint: 0 },
]`.

**When:** `handle_round_start(&mut ctx, 2, vec![], vec![], vec![], vec![], pending_commutations)`.

**Then:**
- `minted_agents.len() == 3`.
- `minted_agents[0] = MintedAgent { request_id: CommutationId(1), minted_agent_id: AgentId(100), symbol_type: Con }`.
- `minted_agents[1] = MintedAgent { request_id: CommutationId(2), minted_agent_id: AgentId(101), symbol_type: Dup }`.
- `minted_agents[2] = MintedAgent { request_id: CommutationId(3), minted_agent_id: AgentId(102), symbol_type: Era }`.
- `state.partition.id_range.next_available == 103`.

**SPEC-19 R covered:** R26, DC-B5.

---

### UT-0394-07: `handle_round_start_minted_agents_allocate_from_partition_id_range_not_global`

**Purpose:** DC-B5 non-overlap — worker 0 minting AgentId(100) does not collide with worker 1 minting AgentId(200).

**Target:** `worker.rs::tests`

**Given:** Two independent `WorkerContext`s: ctx_a with `id_range = [100, 200)`, ctx_b with `id_range = [200, 300)`. Both receive a `PendingCommutation` with `request_id = CommutationId(1)`.

**When:** Call `handle_round_start` on ctx_a and ctx_b independently (not concurrently; test is single-threaded).

**Then:**
- ctx_a produces `minted_agents[0].minted_agent_id == AgentId(100)`.
- ctx_b produces `minted_agents[0].minted_agent_id == AgentId(200)`.
- No collision; both ctxs' `id_range.next_available` advance by 1 independently.

**SPEC-19 R covered:** DC-B5 non-overlap invariant.

---

### UT-0394-08: `handle_round_start_minted_agent_symbol_matches_pending_commutation_symbol`

**Purpose:** Worker does not substitute or transform the `symbol_type` — it mints exactly what was requested.

**Target:** `worker.rs::tests`

**Given:** `pending_commutations = vec![
    PendingCommutation { request_id: CommutationId(1), symbol_type: Con, slot_hint: 0 },
    PendingCommutation { request_id: CommutationId(2), symbol_type: Dup, slot_hint: 0 },
]`.

**When:** `handle_round_start(&mut ctx, 1, vec![], vec![], vec![], vec![], pending_commutations)`.

**Then:**
- `minted_agents[0].symbol_type == Con`; `state.partition.subnet.get_agent(minted_agents[0].minted_agent_id).symbol == Con`.
- `minted_agents[1].symbol_type == Dup`; `state.partition.subnet.get_agent(minted_agents[1].minted_agent_id).symbol == Dup`.

**Assertions:** Symbol transparency — no translation layer between what the resolver requested and what the worker minted.

**SPEC-19 R covered:** R26 (MintedAgent.symbol_type fidelity), DC-B5.

---

### UT-0394-09: `handle_round_start_minted_agents_order_matches_pending_commutations_order`  (DC-0394-B)

**Purpose:** Per DC-0394-B (MUST), `minted_agents` preserves the input order of `pending_commutations`. A coordinator can zip by position or by `request_id`; both must agree.

**Target:** `worker.rs::tests`

**Given:** 5 `PendingCommutation`s with `request_id`s `[7, 3, 11, 2, 5]` (deliberately non-monotonic).

**When:** `handle_round_start(&mut ctx, 3, vec![], vec![], vec![], vec![], pcs)`.

**Then:**
- `minted_agents.iter().map(|m| m.request_id).collect::<Vec<_>>() == vec![7, 3, 11, 2, 5]` (original order preserved byte-for-byte).
- `minted_agents[0].minted_agent_id < minted_agents[1].minted_agent_id < ... < minted_agents[4].minted_agent_id` (IDs are allocated monotonically from `id_range`; this is a byproduct of order-preserving iteration).

**SPEC-19 R covered:** R26, DC-0394-B (proposed ruling).

---

### UT-0394-10: `handle_round_start_id_range_exhaustion_returns_error_action`

**Purpose:** Defensive — if `partition.id_range` is exhausted mid-minting, the handler returns a `WorkerAction::Error(WorkerError::IdRangeExhausted { ... })` WITHOUT a partial `RoundResult`.

**Target:** `worker.rs::tests`

**Given:** `partition.id_range = IdRange { start: 100, end: 102 }` (only 2 IDs available, 100 and 101). `pending_commutations = vec![
    PendingCommutation { request_id: CommutationId(1), symbol_type: Con, slot_hint: 0 },  // gets 100
    PendingCommutation { request_id: CommutationId(2), symbol_type: Dup, slot_hint: 0 },  // gets 101
    PendingCommutation { request_id: CommutationId(3), symbol_type: Era, slot_hint: 0 },  // range exhausted
]`.

**When:** `handle_round_start(&mut ctx, 1, vec![], vec![], vec![], vec![], pcs)`.

**Then:**
- Returned `Vec<WorkerAction>` contains exactly ONE `WorkerAction::Error(WorkerError::IdRangeExhausted { request_id: CommutationId(3) })`.
- No `WorkerAction::SendMessage(Box::new(Message::RoundResult { .. }))` is emitted.
- `state.partition.id_range.next_available == 102` (the first two minted agents ARE committed; the handler does not roll back).
- `state.partition.subnet.get_agent(AgentId(100))` exists with `symbol == Con`.
- `state.partition.subnet.get_agent(AgentId(101))` exists with `symbol == Dup`.
- `AgentId(102)` does not exist.

**Assertions:** Error path fires on exhaustion; no silent skip; no panic; partial progress is allowed (worker does not undo the two already-committed agents — repartitioning is the coordinator's responsibility).

**SPEC-19 R covered:** R26 (error path when resources exhausted); `WorkerError::IdRangeExhausted` variant introduction.

**Note:** If spec-critic rules rollback is a MUST, this test flips: assertions become `id_range.next_available == 100` and neither Con nor Dup exist. Default plan per DC-0394-A: partial progress allowed.

---

### UT-0394-11: `handle_round_start_ignores_slot_hint_per_DC_0394_A`

**Purpose:** DC-0394-A proposal — `slot_hint` is advisory; worker does not honor it.

**Target:** `worker.rs::tests`

**Given:** `partition.id_range = IdRange { start: 100, end: 200 }`. `pending_commutations = vec![
    PendingCommutation { request_id: CommutationId(1), symbol_type: Con, slot_hint: 155 },  // requested slot 155
]`.

**When:** `handle_round_start(&mut ctx, 1, vec![], vec![], vec![], vec![], pcs)`.

**Then:**
- `minted_agents[0].minted_agent_id == AgentId(100)` (worker picked the first from its range, NOT the slot_hint value).
- `state.partition.id_range.next_available == 101`.
- `AgentId(155)` does NOT exist in the subnet.

**Assertions:** `slot_hint` is ignored; allocation follows `id_range` sequentially.

**SPEC-19 R covered:** DC-0394-A (proposed advisory contract).

---

## Existing tests to touch (mechanical update, not counted)

All 9 UT-0381-01..09 tests currently call `handle_round_start(&mut ctx, round, border_deltas, resolved_borders, new_borders)`. Update each to append `vec![]` twice — once for `local_reconnections` and once for `pending_commutations`. The test semantics do not change; only the call site is adjusted. Verify that `cargo test --workspace --lib` count moves from `1039 → 1050` (i.e., `+11` net, NOT `+11 + 9`) — the 9 updates preserve their original assertions.

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| SPEC-19 R23 — full 7-field `RoundStart` consumption | UT-0394-01 (empty path), UT-0394-02..04 (local_reconnections), UT-0394-05..09 (pending_commutations) |
| SPEC-19 R26 — `minted_agents` echo with matching `request_id` | UT-0394-05, UT-0394-06, UT-0394-08, UT-0394-09 |
| DC-B5 — 2-phase agent allocation (worker side) | UT-0394-05, UT-0394-06, UT-0394-07 |
| DC-B5 non-overlap invariant | UT-0394-07 |
| Error path: `id_range` exhaustion | UT-0394-10 (`WorkerError::IdRangeExhausted` variant introduced) |
| DC-0394-A — `slot_hint` advisory | UT-0394-11 |
| DC-0394-B — input-order preservation | UT-0394-09 |
| Defensive skips (DISCONNECTED / self-loops in local_reconnections) | UT-0394-04 |
| Sequential last-write-wins in local_reconnections | UT-0394-03 |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0394-A | `pending_commutations` with duplicate `request_id`s (e.g., both `CommutationId(1)`) | Coordinator-side protocol violation — R48 says coordinator rejects stray `request_id` but what about duplicates? Current worker mints both agents with the same `request_id` in the echo; coordinator would then have ambiguous lookup. **Propose: worker panics on duplicate `request_id` within the same round.** Flagged for QA. |
| QA-0394-B | `pending_commutations` with `symbol_type = /* uninhabited placeholder */` (e.g., an invalid enum discriminant via wire corruption) | Rust's discriminant validation at deserialization should already catch this. If it doesn't (rkyv zero-copy path?), the worker creates an invalid agent. **QA: inject via FrameBuilder in zero-copy path and observe.** |
| QA-0394-C | Concurrent `handle_round_start` calls on the same `WorkerContext` (shouldn't happen in BSP, but race probe) | BSP serializes rounds; no concurrency expected. If someone introduces threading, `id_range.next_available` has no atomics. **QA flag: assert single-threaded.** |
| QA-0394-D | `local_reconnections` pair targets ports outside `partition.id_range` (e.g., AgentId from another worker) | Worker applies the connect blindly; `state.partition.subnet.connect` would silently accept a nonsense AgentId. **Mitigation: `state.partition.subnet.connect` should debug-assert that AgentId targets are within this partition's live agents.** Flagged for QA as a defensive-hardening candidate. |
| QA-0394-E | `local_reconnections` applied AFTER a border delta that erased an endpoint | Ordering bug: if `border_deltas` erased agent `a`, then `local_reconnections` tries to connect `a.1 ↔ b.0`, the connect fires against a non-existent agent. **Mitigation: step 1.5 (`local_reconnections`) should check both endpoints are live agents or DISCONNECTED; mark test to confirm.** Flagged for QA. |
| QA-0394-F | `id_range.next_available` overflow (start near `u32::MAX`) | `AgentId` is `u32`; if `start + len > u32::MAX`, `allocate_agent_id` must return `None` before overflow. **Mitigation: `IdRange::allocate` uses `checked_add`.** Flagged for QA. |
| QA-0394-G | `pending_commutations.len()` > `id_range` capacity — partial exhaustion (UT-0394-10's scenario at a larger scale) | Already covered by UT-0394-10; adversarial variant: 10,000 commutations in one round. Stress test. |
| QA-0394-H | `WorkerError::IdRangeExhausted` returned as `WorkerAction::Error` but coordinator receives no `Message::RoundResult` — coordinator hangs waiting | Protocol-level concern: coordinator's `dispatch_round_start` receives a `RoundResult` per worker per round (R23); if one worker errors, coordinator waits forever. **Mitigation: `WorkerAction::Error` triggers an outbound `Message::Error(ErrorCode::IdRangeExhausted)` AND disconnects.** Out of scope for this task; flag for a follow-up "coordinator error-propagation" task. |
| QA-0394-I | Worker mints agent with `Symbol::Era` (arity 0) but the resolver expected arity > 0 for later wiring | Arity mismatch: `Era` has 0 aux ports, so wiring attempts to its aux ports would fail. Should not happen (resolver knows what symbols it emits), but adversarial: inject a malformed `PendingCommutation`. **Mitigation: `state.partition.subnet.create_agent` debug-asserts that `symbol.arity() == requested arity`.** Flagged for QA. |
| QA-0394-J | Test-only: `cargo test --features some-nonexistent-feature` — does UT-0394 compile under feature permutations? | Build-system robustness; not a logic bug. QA smoke. |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 1039 → **1050** (+11 new `#[test]` fns; 9 prior UT-0381 tests touched mechanically but count unchanged).
2. `cargo test --workspace --lib --features zero-copy` count: 1079 → **1090** (+11).
3. `cargo build --workspace` clean.
4. `cargo build --workspace --features zero-copy` clean.
5. `cargo clippy --workspace --all-targets -- -D warnings` clean (default + zero-copy).
6. `cargo fmt --check` clean.
7. No `unwrap()` in production code; tests use `.expect(...)` with diagnostic messages.
8. All existing UT-0381-01..09 mechanically updated to pass 2 extra `vec![]` — semantics unchanged, all continue to pass.
9. R19 pure-core canary green — this task modifies `worker.rs` (infrastructure layer, allowed to import `protocol::Message` and `tokio`); `merge/` files untouched.
10. `WorkerError` enum gains `IdRangeExhausted` variant; no breaking change to existing variants.

---

## Notes on interaction with other TEST-SPECs

- **TEST-SPEC-0381** (worker round-body) — this TEST-SPEC EXTENDS UT-0381-01..09 with a signature change. The original tests stay but call the new signature with 2 extra `vec![]`. No semantic regression.
- **TEST-SPEC-0385** (coordinator round loop) — TASK-0395's G1 parity integration tests (UT-0385-06..08) will exercise this code end-to-end. A failure at that level implicates either TASK-0394 or TASK-0395.
- **TEST-SPEC-0382** (`compute_outgoing_deltas`) — downstream consumer. The minted agents created here become eligible for border-delta emission in subsequent rounds. No direct interaction at the test level.
- **TEST-SPEC-0377** (pure-core canary) — unaffected; `worker.rs` is infrastructure, not pure-core.

---

## Out of scope

- **Coordinator-side `minted_agents` consumption** — the coordinator's round-N+2 finalizer that maps `request_id → minted_agent_id` and resolves `PendingPortRef::Pending` tokens is a separate piece (may already be present in 2.26-C; re-check during DEV). If absent, that's a NEW task, flagged here but deferred.
- **Rollback on partial exhaustion** — UT-0394-10 asserts partial-progress behavior; if spec-critic rules rollback-MUST, a follow-up task implements it.
- **Concurrent safety on `id_range`** — assumed single-threaded BSP; concurrent worker logic is v3 scope.
- **Wire-level defensiveness** — e.g., rejecting `pending_commutations` with zero-length vectors in the framing layer. Out of scope; wire protocol is shipped 2.26-A.
- **`slot_hint` honoring** — DC-0394-A ignores it. A future optimization task may wire `slot_hint` for cache-locality; not this bundle.
- **G1 parity integration** — TASK-0395 scope.
