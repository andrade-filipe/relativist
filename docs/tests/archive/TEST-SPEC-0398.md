# TEST-SPEC-0398: Pure-core plumbing for D-004 coordinator-side round-N+2 finalizer

**See also:** [docs/backlog/TASK-0398.md](../backlog/TASK-0398.md) — D-004 plumbing; unblocker for TASK-0399.

**Task:** TASK-0398
**Spec:** SPEC-19 §3.3 R23 / R26 / R48 / DC-B5 (2-phase AgentId allocation, coordinator-side half).
**Spec-critic verdicts consumed:**
  - **DC-0398-A (proposed, option a):** commutation_id low-28-bit truncation in encode_request_id/decode_request_id; debug_assert guards.
  - **DC-0398-B (proposed, option a):** LENIENT duplicate-request_id handling at coordinator (consistent with QA-0394-A).
  - **DC-0398-C (proposed, option a):** `enqueue_pending_borders` / `register_minted_agents` are `pub(crate)` (private_interfaces lint constraint from TASK-0396).
**Generated:** 2026-04-23
**Baseline before this task:** 1138 lib default / 1178 lib `--features zero-copy` (post refactor 2026-04-23, commit `08722e0`).
**Cumulative target after this task:** 1146 / 1186 — **+8** new `#[test]` fns in `border_graph.rs::tests`.

---

## Scope note

This TEST-SPEC verifies the pure-core primitives that TASK-0399 will wire into `run_grid_delta_inner`. Tests are BorderGraph-local (no grid loop, no dispatch mocks); they exercise:
1. `encode_request_id` / `decode_request_id` roundtrip invariants (UT-0398-01).
2. `enqueue_pending_borders` passive storage (UT-0398-02).
3. `register_minted_agents` happy path — single mint → single border promoted (UT-0398-03).
4. `register_minted_agents` multi-mint happy path — N mints → N borders promoted (UT-0398-04).
5. `register_minted_agents` partial resolution — one side Pending stays queued (UT-0398-05).
6. `register_minted_agents` duplicate request_id LENIENT (DC-0398-B, UT-0398-06).
7. `register_minted_agents` R48 stray request_id rejection (UT-0398-07).
8. `register_minted_agents` DC-B6 preserve-existing-border path (UT-0398-08).

Integration-level exercises (LocalDeltaDispatch end-to-end, G1 parity on asymmetric fixtures) are scoped to TASK-0399.

---

## Test target file paths

- `relativist-core/src/merge/border_graph.rs` — inline `#[cfg(test)] mod tests`. 8 new `#[test]` fns: UT-0398-01..08.

All tests synchronous. No tokio, no async.

---

## Unit Tests

### UT-0398-01: `encode_decode_request_id_is_roundtrip_for_small_ids`

**Purpose:** encode followed by decode returns the original (commutation_id_low28, agent_slot) pair for all values within the supported range.

**Target:** `border_graph.rs::tests` (import via `use crate::merge::border_resolver::{encode_request_id, decode_request_id};`).

**Given:** A matrix of (commutation_id, agent_slot) pairs:
- `(0, 0)`, `(0, 15)` (slot boundaries)
- `(1, 0)`, `(100, 7)`, `(0x0FFF_FFFF, 15)` (commutation_id boundary at 2^28 - 1)
- `(0xFF_FFFF, 3)` (mid-range)

**When:** For each pair, `let r = encode_request_id(cid, slot); let (cid_back, slot_back) = decode_request_id(r);`.

**Then:**
- `slot_back == slot` for all cases.
- `cid_back == (cid as u32) & 0x0FFF_FFFF` (low-28-bit truncation — documented).
- For cid < 2^28: `cid_back == cid as u32`.

**Assertions:** bit-packing invertible within the <2^28 range; truncation explicit for larger.

**SPEC-19 R covered:** R48 wire-correlation; DC-0398-A encoding contract.

---

### UT-0398-02: `enqueue_pending_borders_is_passive_storage`

**Purpose:** `enqueue_pending_borders` only stores; it does not mutate self.borders, self.active_redexes, self.worker_borders.

**Target:** `border_graph.rs::tests`.

**Given:** Fresh `BorderGraph::from_partition_plan(...)` with 2 workers. Build a `Vec<PendingNewBorder>` with 2 entries referencing commutation_id=7, agent_slots 0 and 1 (constructed via `PendingPortRef::Pending { commutation_id: 7, agent_slot: 0, port_slot: 0 }` for side_a and a Concrete side_b each).

**When:** `graph.enqueue_pending_borders(borders);`.

**Then:**
- `graph.pending_new_borders.len() == 2`.
- `graph.pending_new_borders` contains both entries in input order.
- `graph.borders` unchanged (still only contains the original borders from the partition plan, if any).
- `graph.active_redexes` unchanged.
- `graph.worker_borders[w_id]` unchanged for each worker.
- `graph.resolved_mints.is_empty()`.

**Assertions:** Pure storage; no side effect on public graph state.

**SPEC-19 R covered:** DC-B5 coordinator-side state model (lazy round-N+2 finalization).

---

### UT-0398-03: `register_minted_agents_single_mint_promotes_fully_resolved_border`

**Purpose:** A PendingNewBorder with side_a=Pending and side_b=Concrete, once side_a's commutation resolves via a MintedAgent, gets promoted to an `AddBorderEntry` and added to `self.borders` via `add_border_states`.

**Target:** `border_graph.rs::tests`.

**Given:**
- `BorderGraph` with 2 workers.
- Enqueue 1 PendingNewBorder: `border_id=100`, side_a=Pending{commutation_id=7, agent_slot=0, port_slot=1}, side_b=Concrete(AgentPort(5, 0)), worker_a=0, worker_b=1.
- Prepare `mints: &[MintedAgent]` = `[{request_id: encode_request_id(7, 0), minted_agent_id: 42}]`.

**When:** `graph.register_minted_agents(/* worker_id */ 0, &mints);`.

**Then:**
- Result is `Ok(())`.
- `graph.pending_new_borders.is_empty()` — the entry was fully resolved and removed.
- `graph.borders.contains_key(&100)` — the promoted border is now in the map.
- `graph.borders[&100].side_a == PortRef::AgentPort(42, 1)` (Pending resolved to concrete using minted_agent_id=42 and port_slot=1 from the Pending token).
- `graph.borders[&100].side_b == PortRef::AgentPort(5, 0)` (pass-through).
- `graph.worker_borders[0]` contains 100 AND `graph.worker_borders[1]` contains 100.
- `graph.resolved_mints[(7, 0)] == 42` (still in the cache; flush happens when no more pending references exist — current design keeps entries for debug/audit).

**SPEC-19 R covered:** R26 minted_agents consumption; DC-B5 round-N+2 promotion; `add_border_states` integration.

---

### UT-0398-04: `register_minted_agents_multiple_mints_preserve_input_order_of_pending_borders`

**Purpose:** N PendingNewBorders resolved simultaneously by N MintedAgents; add_border_states called with entries in the input order of pending_new_borders.

**Target:** `border_graph.rs::tests`.

**Given:**
- 3 PendingNewBorders enqueued in order: border_id=100 (cid=7, slot=0), border_id=101 (cid=7, slot=1), border_id=102 (cid=7, slot=2). All with side_a=Pending, side_b=Concrete(distinct FreePort sinks).
- `mints` in DELIBERATELY different order: `[{slot=2, id=14}, {slot=0, id=12}, {slot=1, id=13}]` — each request_id encoded from cid=7 + respective slot.

**When:** `graph.register_minted_agents(0, &mints)`.

**Then:**
- All 3 pending borders resolved; `pending_new_borders.is_empty()`.
- `graph.borders` contains keys 100, 101, 102.
- `graph.borders[&100].side_a == AgentPort(12, port_slot_100)` (matching cid=7, slot=0 → id=12).
- `graph.borders[&101].side_a == AgentPort(13, port_slot_101)`.
- `graph.borders[&102].side_a == AgentPort(14, port_slot_102)`.
- `graph.resolved_mints` contains all 3 entries.

**Assertions:** Order of mints doesn't matter (HashMap-keyed lookup); order of promoted borders follows pending_new_borders input order (stable).

---

### UT-0398-05: `register_minted_agents_partial_resolution_keeps_unresolved_borders_queued`

**Purpose:** If a PendingNewBorder has BOTH side_a=Pending and side_b=Pending, and only side_a's commutation is minted this round, the border STAYS in pending_new_borders. A subsequent register_minted_agents call that completes side_b promotes it.

**Target:** `border_graph.rs::tests`.

**Given:**
- 1 PendingNewBorder: border_id=200, side_a=Pending{cid=7, slot=0, port_slot=1}, side_b=Pending{cid=7, slot=1, port_slot=2}. Both pending.

**When (round 1):** `graph.register_minted_agents(0, &[{cid=7, slot=0, id=50}])`.

**Then (after round 1):**
- Result `Ok(())`.
- `pending_new_borders.len() == 1` — NOT promoted because side_b is still Pending.
- `graph.resolved_mints[(7, 0)] == 50`.
- `graph.borders` does NOT contain 200.

**When (round 2):** `graph.register_minted_agents(0, &[{cid=7, slot=1, id=51}])`.

**Then (after round 2):**
- Result `Ok(())`.
- `pending_new_borders.is_empty()` — NOW promoted.
- `graph.borders.contains_key(&200)`.
- `graph.borders[&200].side_a == AgentPort(50, 1)` (resolved in round 1).
- `graph.borders[&200].side_b == AgentPort(51, 2)` (resolved in round 2).

**Assertions:** Multi-round resolution via persistent `self.resolved_mints`; stable partial state across round boundaries.

**SPEC-19 R covered:** DC-B5 round-N+2 ≠ round-N+k for k>2 edge — resolution may span multiple rounds if different commutations mint at different round cadences.

---

### UT-0398-06: `register_minted_agents_duplicate_request_id_is_lenient_per_DC_0398_B`

**Purpose:** DC-0398-B (option a, LENIENT) — if mints slice contains two entries with the same request_id (same (cid, slot) pair), the coordinator accepts both; the second overwrites `self.resolved_mints` entry; one minted_agent_id becomes canonical, the other is silently dropped at coordinator level (the worker's arena has both agents but the coordinator cache lazy-syncs on FinalStateResult).

**Target:** `border_graph.rs::tests`.

**Given:**
- 1 PendingNewBorder: border_id=300, side_a=Pending{cid=7, slot=0, port_slot=0}, side_b=Concrete(FreePort(999)).
- `mints = [{request_id: encode(7,0), minted_id: 60}, {request_id: encode(7,0), minted_id: 61}]` — duplicate request_id, different minted_ids.

**When:** `graph.register_minted_agents(0, &mints)`.

**Then:**
- Result `Ok(())` — LENIENT.
- `graph.resolved_mints[(7, 0)] == 61` (second mint wins, last-write).
- `graph.borders.contains_key(&300)`.
- `graph.borders[&300].side_a == AgentPort(61, 0)` — uses the canonical (second) minted id.

**Assertions:** Lenient policy pinned; QA-0394-A's worker-side lenient counterpart consistent.

**Note:** If spec-critic later rules DC-0398-B option (b) STRICT, this test flips to assert `Err(GridError::ProtocolViolation)` with anchor substring "duplicate request_id".

---

### UT-0398-07: `register_minted_agents_stray_request_id_returns_protocol_violation`

**Purpose:** R48 invariant — if a MintedAgent.request_id decodes to a (cid, slot) pair that matches NO outstanding PendingPortRef::Pending in self.pending_new_borders, return `GridError::ProtocolViolation`.

**Target:** `border_graph.rs::tests`.

**Given:**
- 1 PendingNewBorder expecting cid=7, slot=0.
- mints contains cid=99, slot=0 (stray — never requested).

**When:** `graph.register_minted_agents(0, &mints)`.

**Then:**
- Result `Err(GridError::ProtocolViolation(msg))`.
- `msg` contains substring "R48", "stray", and "worker 0".
- `pending_new_borders` unchanged (border stays queued).
- `graph.borders` unchanged.
- `graph.resolved_mints` — may or may not contain the stray entry depending on implementation; asserting the error is sufficient.

**Assertions:** R48 enforcement; error carries actionable diagnostic.

**SPEC-19 R covered:** R48 primary path.

---

### UT-0398-08: `register_minted_agents_DC_B6_preserve_existing_border_path`

**Purpose:** DC-B6 CON-ERA / DUP-ERA — `PendingNewBorder.border_id` equals an EXISTING border_id in self.borders. In this case, `register_minted_agents` must REMOVE the existing border first, then add_border_states with the new entry. (This preserves the "update existing border" semantics the resolver emits for asymmetric erasure rules.)

**Target:** `border_graph.rs::tests`.

**Given:**
- Seed `graph.borders` with an existing entry at border_id=400 (via `graph.add_border_states` in setup).
- Enqueue 1 PendingNewBorder: border_id=400 (SAME as existing), side_a=Pending{cid=7, slot=0, port_slot=1}, side_b=Concrete(FreePort(888)), worker_a=0, worker_b=1.
- mints = [{cid=7, slot=0, id=70}].

**When:** `graph.register_minted_agents(0, &mints)`.

**Then:**
- Result `Ok(())`.
- `pending_new_borders.is_empty()`.
- `graph.borders[&400].side_a == AgentPort(70, 1)` — new side_a.
- `graph.borders[&400].side_b == FreePort(888)` — new side_b.
- The OLD border at 400 has been REPLACED (old side_a / side_b no longer present).
- No duplicate in `graph.worker_borders[0]` or `graph.worker_borders[1]` (remove_border cleans up the old worker-side indices before add_border_states re-adds).
- `graph.active_redexes` reflects the new is_redex status (potentially different from the old entry — is_principal_pair re-evaluated).

**Assertions:** DC-B6 replace semantics functional; worker_borders index stays consistent.

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| SPEC-19 R26 minted_agents echo consumption | UT-0398-03, 04 |
| SPEC-19 R48 stray request_id rejection | UT-0398-07 |
| DC-B5 round-N+2 promotion | UT-0398-03, 04 |
| DC-B5 multi-round partial resolution | UT-0398-05 |
| DC-B6 CON-ERA / DUP-ERA preserve-border path | UT-0398-08 |
| DC-0398-A encoding roundtrip | UT-0398-01 |
| DC-0398-B duplicate request_id lenient | UT-0398-06 |
| enqueue_pending_borders passive storage | UT-0398-02 |

---

## Adversarial angles (for TASK-0399's QA, not covered here)

| # | Scenario | Covered by |
|---|----------|---|
| A | 12-case G1 parity across 6 fixtures × 2 strict modes | TASK-0399 UT-0385-08 (SKIP_ASYMMETRIC=false flip) |
| B | Multi-round commutation pipeline with 3+ rounds of Pending accumulation | TASK-0399 smoke test |
| C | BorderGraph serialization preserves pending state across rkyv archive roundtrip | Deferred (not blocking D-003 closure) |
| D | Exhaustion: 2^28 commutation_ids | Deferred (debug_assert guard) |
| E | MintedAgent list with mixed valid + stray (partial success) | TEST-SPEC flags for Stage 5 QA if needed; current impl rejects the whole slice on first stray |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 1138 → **1146** (+8).
2. `cargo test --workspace --lib --features zero-copy` count: 1178 → **1186** (+8).
3. `cargo clippy --workspace --all-targets -- -D warnings` clean (default + zero-copy).
4. `cargo fmt --check` clean.
5. No regression on any existing test (R42 smoke, UT-0385-06/07 symmetric G1 parity, UT-0394-* worker tests).
6. `GridError::ProtocolViolation` variant available in error.rs (added if missing).
7. `package_resolutions` still works (companion `package_resolutions_with_pending` is additive).

---

## Notes on interaction with other TEST-SPECs

- **TEST-SPEC-0394 UT-0394-10** (id_range exhaustion): unchanged. `register_minted_agents` doesn't touch id_range semantics.
- **TEST-SPEC-0394 QA-0394-A** (duplicate request_id LENIENT at worker): mirrored by UT-0398-06 at coordinator. Matched policy.
- **TEST-SPEC-0385 UT-0385-06/07/08**: the 3 tests will continue to pass after this task lands (symmetric rules don't exercise register_minted_agents because resolver emits no pending_new_borders for them). TASK-0399 flips SKIP_ASYMMETRIC and those same tests grow to exercise UT-0385-08 asymmetric branches via register_minted_agents.

---

## Out of scope

- **Full loop integration** — TASK-0399 scope.
- **Flip SKIP_ASYMMETRIC=false** — TASK-0399 scope.
- **Performance optimization** of the O(N*M) scan in register_minted_agents — current implementation is linear-in-pending_borders × linear-in-mints; sufficient for the round-level granularity (mints/round ~O(commutations), pending_borders ~O(commutations) → O(N²) per round, but N is per-round and small).
- **Garbage collection of `resolved_mints`** — entries persist across rounds; cleared only when a new BSP run starts (via `BorderGraph::from_partition_plan`). If this becomes a memory concern for long-running grids, add `resolved_mints.shrink_to_fit()` or periodic cleanup in TASK-0399.
- **rkyv serialization** of the new state fields — `pending_new_borders` and `resolved_mints` do NOT need to cross the wire (coordinator-local state). Do NOT add `#[cfg_attr(feature = "zero-copy", derive(rkyv::Archive, ...))]` to them.
