# TEST-SPEC-0376: Integration test — end-to-end `BorderResolver` + `package_resolutions` on 2-partition fixtures

**Task:** TASK-0376
**Spec:** SPEC-19 §3.2 R13 (redex resolution entry), R14 (6-rule
  closure), R15 parts 1-2 (delta dispatch + `remove_border`); SPEC-19
  §3.3 (item 2.26); 2.26-B spec-critic DC-B3 (`local_reconnections`),
  DC-B5 (pending commutations for CON-DUP / CON-ERA / DUP-ERA), DC-B6
  (CON-ERA / DUP-ERA auxiliary-border preservation), DC-B7 (triples).
**Generated:** 2026-04-17
**Baseline before this task:** 1013 lib (default) / 1053 lib
  (`--features zero-copy`) — post-TEST-SPEC-0375.
**Cumulative target after this task:** 1020 lib (default) / 1060 lib
  (`--features zero-copy`) — **+7** new `#[test]` fns (6 per-rule
  integration tests + 1 idempotence/double-resolve test).

---

## Scope note

TASK-0376 ships the integration test file — per TASK acceptance default
(Option (b) of the visibility question), an inner `#[cfg(test)] mod
integration_tests` lives **inside** `relativist-core/src/merge/border_resolver.rs`.
This keeps the resolver's `pub(crate)` items reachable without a
visibility uplift.

Seven integration tests — one per IC rule × the post-resolution
idempotence check — stitch `resolve_border_redex` + `package_resolutions`
end-to-end on 2-partition fixtures with exactly 1 border redex:

```
┌── Partition 0 ──┐         ┌── Partition 1 ──┐
│                 │         │                 │
│   [agent_a]────────bid 0──────────[agent_b] │
│   (p0=principal)          (p0=principal)    │
│                 │         │                 │
└─────────────────┘         └─────────────────┘
```

Each test:
1. Builds the fixture via manual `PartitionPlan` construction (no
   `split()` call — the test controls exact border layout).
2. `BorderGraph::from_partition_plan(&plan)`.
3. Asserts `graph.detect_border_redexes().len() == 1` (precondition).
4. Calls `resolve_border_redex(&mut graph, &plan.partitions, 0)`.
5. Wraps: `let packaged = package_resolutions(vec![resolution], 2);`.
6. Asserts per-rule expected shape of `packaged` (both worker 0 and
   worker 1 dispatches) AND post-graph state.

**Scope matches TASK-0376 acceptance criteria bullet 5 explicitly.**

Seven tests:

- UT-0376-01: CON-CON border redex (Anni cross).
- UT-0376-02: DUP-DUP border redex (Anni parallel).
- UT-0376-03: ERA-ERA border redex (Void).
- UT-0376-04: CON-DUP border redex (Commutation — DC-B5 2-phase).
- UT-0376-05: CON-ERA border redex (Erasure — DC-B6 preservation).
- UT-0376-06: DUP-ERA border redex (Erasure symmetric — DC-B6).
- UT-0376-07: Idempotence — `resolve_border_redex` on an already-
  resolved (now absent) border panics (or returns a defensive-empty
  result per DC-B2 style).

**Out of scope:**
- `run_grid_delta` BSP-loop end-to-end → 2.26-C (worker-lifecycle
  bundle; has its own integration test).
- Wire-format round-trip of resolution outputs → 2.26-A TEST-SPEC-0368
  / 0369.
- Multi-partition (3+) fixtures — "each border has exactly 2 sides"
  (SPEC-19 C3) makes 2-partition fixtures the canonical minimum.

---

## Test target file paths

- `relativist-core/src/merge/border_resolver.rs` — new inner
  `#[cfg(test)] mod integration_tests` block (parallel to the existing
  inline `mod tests` block seeded by TASK-0372). 7 new `#[test]` fns +
  six fixture-builder helper fns (`make_*_fixture`).

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Integration Tests

| Test ID | Name | Reqs covered | File | Preconditions | Assertions | Expected outcome |
|---------|------|--------------|------|---------------|------------|------------------|
| UT-0376-01 | `con_con_border_redex_end_to_end_resolves_and_packages` | R13, R14 (Anni cross), R15 parts 1-2, DC-B3, DC-B7 | `merge/border_resolver.rs::integration_tests` | `make_con_con_fixture()` → `(plan, graph)`. | (i) `graph.detect_border_redexes().len() == 1` pre-call; (ii) `resolve_border_redex` returns a resolution with `resolved_borders == [(0, 0, 1)]`, `new_borders.is_empty()`, `pending_commutations.is_empty()` (CON-CON is pure annihilation — no commutation expansion); (iii) `graph.borders.contains_key(&0) == false`; (iv) `graph.active_redexes` empty; (v) `package_resolutions(vec![resolution], 2)` returns exactly 2 entries; each worker's dispatch has `resolved_borders == [0]`; `local_reconnections.len() == 2` on each worker (cross pattern emits 2 local-aux reconnections per worker per DC-B3). | End-to-end CON-CON flow closes cleanly. |
| UT-0376-02 | `dup_dup_border_redex_end_to_end_resolves_and_packages` | R13, R14 (Anni parallel), R15 parts 1-2, DC-B3, DC-B7 | `merge/border_resolver.rs::integration_tests` | `make_dup_dup_fixture()` → `(plan, graph)`. | Same shape as UT-0376-01 with `Symbol::Dup`; `local_reconnections` uses parallel pattern (not cross). Border removed, packaged dispatch has `resolved_borders == [0]` on both workers. | Parallel pattern end-to-end. |
| UT-0376-03 | `era_era_border_redex_end_to_end_resolves_and_packages` | R13, R14 (Void), R15 parts 1-2, DC-B7 | `merge/border_resolver.rs::integration_tests` | `make_era_era_fixture()` → `(plan, graph)`. | `resolve` returns `resolved_borders == [(0, 0, 1)]`, `new_borders.is_empty()`, `worker_deltas` empty (or present with all-empty inner vectors), `pending_commutations.is_empty()`. Border removed. `package_resolutions` returns 2 entries, each with `resolved_borders == [0]` and ALL other fields empty. | Zero-auxiliary rule takes the shortest path end-to-end. |
| UT-0376-04 | `con_dup_border_redex_end_to_end_emits_pending_commutations` | R13, R14 (Comm), R15 part 2, DC-B5 | `merge/border_resolver.rs::integration_tests` | `make_con_dup_fixture()` → `(plan, graph)`. | `resolve` returns: `resolved_borders == [(0, 0, 1)]`; `new_borders.is_empty()` (DC-B5 defers); `pending_commutations.len() == 2` (one per worker, each minting 2 agents); `pending_new_borders.len() <= 4`. `package_resolutions` output: worker 0 dispatch has `pending_commutations.len() == 1` with `worker == 0`; worker 1 dispatch has `pending_commutations.len() == 1` with `worker == 1`; both have `resolved_borders == [0]`. `graph.borders.contains_key(&0) == false`. | 2-phase coordinator half end-to-end. |
| UT-0376-05 | `con_era_border_redex_end_to_end_preserves_auxiliary_border` | R13, R14 (Eras), R15 part 2, DC-B6 | `merge/border_resolver.rs::integration_tests` | `make_con_era_fixture()` — fixture with P0: Con agent having principal on border 0 and ONE aux port on border 7 (crosses to another agent in P1); P1: Era agent on border 0. P1 ALSO has a Con agent whose principal is the other side of border 7. | (i) `resolve` returns `resolved_borders == [(0, 0, 1)]`; (ii) `new_borders.is_empty()`; (iii) `graph.borders.contains_key(&0) == false`; (iv) **`graph.borders.contains_key(&7) == true`** (DC-B6 preservation); (v) `graph.borders[&7].side_a` is updated to reference the new-ERA's principal port (either concretely — if minted in this round — or a Pending placeholder if DC-B5's 2-phase deferral applies); (vi) `package_resolutions` fans the border-7 update / pending-update to worker 0's dispatch. | DC-B6 `apply_deltas` path — auxiliary border survives with updated endpoint. |
| UT-0376-06 | `dup_era_border_redex_end_to_end_preserves_auxiliary_border` | R13, R14 (Eras), R15 part 2, DC-B6 | `merge/border_resolver.rs::integration_tests` | `make_dup_era_fixture()` — mirror of CON-ERA with Dup replacing Con. | Same assertions as UT-0376-05. | DC-B6 symmetric path. |
| UT-0376-07 | `resolve_border_redex_on_absent_border_panics_per_dc_b2` | DC-B2 (defensive invariant check) | `merge/border_resolver.rs::integration_tests` | `make_con_con_fixture()`; resolve border 0; THEN call `resolve_border_redex(&mut graph, &plan.partitions, 0)` a SECOND time on the now-empty graph. | Second call inside `std::panic::catch_unwind` panics; the payload contains either `"border_resolver: border 0 not present"` or the DC-B2 "agent missing" format (implementation may choose the former for a crisper diagnostic when `graph.borders.get(&0)` is `None` BEFORE `materialize_agent` is reached). At minimum, the second call does NOT silently return `BorderResolution::default()` — a panic or typed error IS required to surface the invariant violation. | Idempotence check — the resolver is NOT a no-op on absent borders; an invariant violation at this call site IS a bug. |

### Fixture builder helpers

Each builder produces a (PartitionPlan, BorderGraph) pair with exactly
1 border redex between 2 partitions. Helpers live in
`mod integration_tests`:

```rust
fn make_con_con_fixture() -> (PartitionPlan, BorderGraph);
fn make_dup_dup_fixture() -> (PartitionPlan, BorderGraph);
fn make_era_era_fixture() -> (PartitionPlan, BorderGraph);
fn make_con_dup_fixture() -> (PartitionPlan, BorderGraph);
fn make_con_era_fixture() -> (PartitionPlan, BorderGraph);  // with aux border 7
fn make_dup_era_fixture() -> (PartitionPlan, BorderGraph);  // with aux border 7
```

Each uses `Net::new()` + `create_agent` + manual port wiring + manual
`free_port_index` construction. `IdRange` and `border_id_start /
_end` are set so each partition owns a distinct, non-overlapping range.

---

## Adversarial / QA coverage map

| Requirement / DC | Covered by |
|---|---|
| R13 — resolver entry point reachable; returns `BorderResolution` | UT-0376-01..06 |
| R14 — all 6 IC rules covered end-to-end | UT-0376-01..06 (one per rule) |
| R15 part 1 — `package_resolutions` fan-out correct | UT-0376-01..06 (each asserts 2-entry packaged output) |
| R15 part 2 — `remove_border` called (border disappears) | UT-0376-01..06 |
| R15 part 3 — NOT called at resolver time for CON-DUP (DC-B5 deferral); NEVER called for CON-ERA/DUP-ERA (DC-B6) | UT-0376-04 (`new_borders.is_empty()`); UT-0376-05, UT-0376-06 (`new_borders.is_empty()` + border 7 preserved) |
| DC-B3 — `local_reconnections` carry the right aux-to-aux pairs | UT-0376-01, UT-0376-02 (count-level assertion; value-level reserved for TEST-SPEC-0373) |
| DC-B5 — `pending_commutations` emission for CON-DUP | UT-0376-04 |
| DC-B5 — `pending_commutations` may also appear for CON-ERA/DUP-ERA (workers mint 2 new ERAs) | UT-0376-05, UT-0376-06 (implicit via the preserved-border's endpoint pointing to a new ERA) |
| DC-B6 — auxiliary border preservation via `apply_deltas` (NOT `add_border_states`) | UT-0376-05, UT-0376-06 |
| DC-B7 — triples fan into both workers' `resolved_borders: Vec<u32>` | UT-0376-01..06 (all assert `result[0].1.resolved_borders == [0]` and `result[1].1.resolved_borders == [0]`) |
| Defensive invariant — double-resolve panics | UT-0376-07 |
| Composition — `resolve_border_redex` + `package_resolutions` interoperate | UT-0376-01..06 (all call both) |

### QA adversarial angles (Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0376-A | Resolver returns a correct `BorderResolution` but `package_resolutions` fails to fold `resolved_borders` triples | UT-0376-01..06 fires on `packaged[w].resolved_borders == [0]` |
| QA-0376-B | CON-DUP's `pending_commutations` pack into the wrong worker's dispatch | UT-0376-04 fires on worker-specific `pending_commutations` assertions |
| QA-0376-C | CON-ERA's auxiliary border (7) is accidentally removed by resolver | UT-0376-05 fires on `graph.borders.contains_key(&7)` |
| QA-0376-D | ERA-ERA emits a spurious `local_reconnections` entry | UT-0376-03 catches via all-empty inner-vector assertion |
| QA-0376-E | Double-resolve silently returns `Default::default()` | UT-0376-07 fires on `catch_unwind` returning `Ok(_)` |
| QA-0376-F | `make_con_dup_fixture()` happens to place all 4 aux targets local (no new cross-partition wires) — `pending_new_borders` empty | This is a LEGAL case — test must accept `pending_new_borders.len() ≤ 4` (not `> 0`). Fixture chosen so at least one wire crosses for the existing `<=` assertion to remain strict. |
| QA-0376-G | `graph.active_redexes` still contains bid 0 after resolution (bookkeeping leak) | UT-0376-01 / 02 / 03 explicit assertion |
| QA-0376-H | `#[cfg(test)] mod integration_tests` accidentally escapes to release binary | `#[cfg(test)]` guard — compile-time eliminated. Not a runtime concern. |
| QA-0376-I | Fixture builder creates a border with a wrong `free_port_index` entry — `BorderGraph::from_partition_plan` panics with C3 violation | Fixture bug, not a resolver bug. Panic at fixture-construction is visible in the test output; easy to fix. |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 1013 → **1020** (+7 new
   `#[test]` fns).
2. `cargo test --workspace --lib --features zero-copy` count: 1053 →
   **1060** (+7).
3. `cargo build --workspace` clean (default features).
4. `cargo build --workspace --features zero-copy` clean.
5. `cargo clippy --workspace --all-targets -- -D warnings` clean.
6. `cargo fmt --check` clean.
7. Manual grep guard still passes on `border_resolver.rs` (the
   integration tests live in a `#[cfg(test)]` block, so `tokio` /
   `protocol` imports in the test block would also trip the guard if
   introduced — but the bundle scope says tests are synchronous `#[test]`
   units; no test introduces those imports).
8. Each fixture builder produces a `BorderGraph` with
   `detect_border_redexes().len() == 1` — tests include this assertion
   as a self-check.

---

## Resolved ambiguities

- **Visibility option.** TASK-0376 defaults to Option (b) — inline
  `#[cfg(test)] mod integration_tests` — and this TEST-SPEC follows
  that default. If the DEV stage escalates to Option (a) (external
  integration test file under `tests/`) + `#[cfg(test)] pub` shim, the
  test names and assertions in this spec remain valid; only the file
  path changes. Prefer Option (b) unless the reviewer asks otherwise.
- **UT-0376-05 / UT-0376-06 auxiliary border endpoint.** Per DC-B5
  2-phase flow, the CON-ERA resolver can either (a) carry a
  `PendingPortRef::Pending` placeholder for the new ERA's AgentId in
  the border-7 update, finalized at round N+2 when `minted_agents`
  arrives, OR (b) require the 2.26-C coordinator loop to wait one round
  before issuing the border-7 update. The TEST-SPEC accepts EITHER
  implementation path: the assertion is that `graph.borders.contains_key(&7)`
  is preserved, and the endpoint update is SOMEWHERE in the resolution
  output (either in `worker_deltas[.].border_deltas` OR in
  `pending_new_borders`). This TEST-SPEC asserts presence, not
  placement.
- **UT-0376-07 panic vs typed-error.** Per DC-B2, a panic is preferred.
  If the DEV stage introduces a typed `ResolverError::BorderAbsent`, the
  TEST-SPEC's `catch_unwind` assertion fails — but the developer MUST
  then also change the caller to handle the Result, which is a larger
  surface change that spec-critic has explicitly rejected (DC-B2
  verdict). Keep the panic.
- **Fixture determinism.** Every `make_*_fixture` helper uses fixed
  AgentIds (0, 1, 2, 3, 4 — per worker) and fixed border IDs (0 for
  the principal redex; 7 for CON-ERA / DUP-ERA's auxiliary). This lets
  assertions reference specific numerics directly.

---

## Test count delta

**+7 tests** (default + zero-copy). Running total after this task:
1020 lib / 1060 lib.
