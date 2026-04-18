# TEST-SPEC-0374: Commutation + asymmetric erasure — CON-DUP, CON-ERA, DUP-ERA border-redex resolution

**Task:** TASK-0374
**Spec:** SPEC-19 §3.2 R13, R14 (commutation/erasure half of the 6-rule
  closure), R15 parts 2-3 (`remove_border` + `add_border_states`);
  SPEC-19 §3.3 R48 (AgentId reserved range for correlation IDs);
  2.26-B spec-critic DC-B5 (2-phase AgentId allocation via
  `pending_commutations` / `pending_new_borders`), DC-B6 (CON-ERA /
  DUP-ERA preserve auxiliary borders via `apply_deltas` — NO new
  border IDs).
**Generated:** 2026-04-17
**Baseline before this task:** 1002 lib (default) / 1042 lib
  (`--features zero-copy`) — post-TEST-SPEC-0373.
**Cumulative target after this task:** 1008 lib (default) / 1048 lib
  (`--features zero-copy`) — **+6** new `#[test]` fns in
  `merge::border_resolver::tests`.

---

## Scope note

TASK-0374 closes the 6-rule dispatch table by adding `resolve_con_dup`,
`resolve_con_era`, `resolve_dup_era`, extending the dispatcher in
`resolve_border_redex` to route the three asymmetric pairs (and their
reverse forms via `normalize_pair`), and introducing the `CommutationId`
allocator + `BorderIdAllocator` per the DC-B5 amendment.

Under DC-B5 Option (B) (workers allocate AgentIds, 2-phase echo), CON-DUP
at the resolver produces:

- `pending_commutations: Vec<PendingCommutation>` — one entry per
  involved worker describing the symbols to mint + the per-agent local
  wiring hints.
- `pending_new_borders: Vec<PendingNewBorder>` — placeholder border
  entries with `PendingPortRef::Pending { commutation_id, agent_slot,
  port_slot }` tokens; their concrete resolution is 2.26-C's territory
  (round N+2 finalization).
- `resolved_borders: [(bid, worker_a, worker_b)]` — the original border
  IS removed in this round (DC-B5 flow step 1).
- `new_borders: Vec<AddBorderEntry>` — EMPTY (deferred until round N+2).
- `graph.add_border_states(...)` — NOT called at this resolver stage.

Under DC-B6 Option (A), CON-ERA / DUP-ERA:

- Produce 2 new ERA agents per resolution (via `pending_commutations` —
  workers mint locally).
- Existing auxiliary-port borders on the consumed non-ERA agent are
  PRESERVED: the resolver emits `BorderDelta { border_id, new_target:
  AgentPort(new_era_id, 0) }` via `apply_deltas` semantics — NO new
  border ID allocated.
- The principal-port border that triggered the resolution IS removed
  (`resolved_borders` triple).
- `new_borders` is empty.

**Four contracts under test:**

1. **CON-DUP pending-commutation emission.** UT-0374-01 / UT-0374-02
   pin the two-phase protocol's coordinator half: `pending_commutations`
   has entries for both workers (typically 2 per worker for 2-CON + 2-
   DUP expansion); `pending_new_borders` has ≤ 4 entries carrying
   `Pending` placeholders; `new_borders.is_empty()`; the original border
   is removed.
2. **CON-ERA border preservation (DC-B6).** UT-0374-03 exercises the
   `apply_deltas` path: the consumed CON has one auxiliary port
   connected to an existing border `bid_aux`; that border survives with
   endpoint re-pointed to the new ERA's principal port.
3. **DUP-ERA symmetric path.** UT-0374-04 mirrors UT-0374-03 with DUP
   replacing CON.
4. **Allocator + dispatch completeness.** UT-0374-05 checks `BorderIdAllocator`
   monotonicity across back-to-back CON-DUP resolutions (border-ID
   allocator is coordinator-owned — u32 counter — and remains in scope
   per the DC-B5 verdict); UT-0374-06 checks the dispatcher reaches
   `resolve_con_dup` / `resolve_con_era` / `resolve_dup_era` for both
   orderings of the symbol pair.

**Out of scope:**
- Round N+2 finalization (concrete AgentIds substituted in via
  `minted_agents`) → 2.26-C memo.
- Wire-level `PendingCommutation` serde round-trip → TEST-SPEC-0366
  TASK-0366 T8 (already shipped in 2.26-A).
- `package_resolutions` fan-out of `pending_commutations` →
  TEST-SPEC-0375.
- End-to-end `BorderResolver` + `package_resolutions` wiring →
  TEST-SPEC-0376.

---

## Test target file paths

- `relativist-core/src/merge/border_resolver.rs::tests` — 6 new
  `#[test]` fns appended to the `#[cfg(test)] mod tests` block.

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Unit Tests

| Test ID | Name | Reqs covered | File | Preconditions | Assertions | Expected outcome |
|---------|------|--------------|------|---------------|------------|------------------|
| UT-0374-01 | `resolve_con_dup_emits_pending_commutations_for_both_workers` | R13, R14 (Comm), R15 part 2, DC-B5 | `merge/border_resolver.rs::tests` | 2-partition CON-DUP fixture: `P0.agent0 = Con` with aux targets local; `P1.agent0 = Dup` with aux targets local; border 0 connects principals. | `resolve_border_redex(&mut graph, &[p0,p1], 0)` returns a `BorderResolution` with (i) `pending_commutations.len() == 2` (one entry per worker); (ii) each `PendingCommutation.target_symbols.len() == 2` (each worker mints 2 agents — typically one Con + one Dup per DC-B5 notes); (iii) `pending_new_borders.len() <= 4`; (iv) `new_borders.is_empty()`; (v) `resolved_borders == [(0, 0, 1)]`; (vi) `graph.borders.contains_key(&0) == false`. | 2-phase protocol half 1: request AgentIds from workers, original border removed. |
| UT-0374-02 | `resolve_con_dup_pending_new_borders_carry_placeholder_refs` | DC-B5 (PendingPortRef enum shape) | `merge/border_resolver.rs::tests` | CON-DUP fixture where AT LEAST ONE of the 4 new cross-partition wires would cross worker boundaries (e.g. `P0.agent0.1 → AgentPort(local, 0)` but `P0.agent0.2 → FreePort(existing_border)`). | For every entry `pnb` in `resolution.pending_new_borders`, at least one of `pnb.side_a` / `pnb.side_b` is `PendingPortRef::Pending { commutation_id, agent_slot, port_slot }` referring to a concrete `CommutationId` that appears in `resolution.pending_commutations`. Both `worker_a` and `worker_b` fields are valid. | Placeholder tokens correlate coherently with pending commutations; round N+2 can resolve them. |
| UT-0374-03 | `resolve_con_era_preserves_auxiliary_border_via_apply_deltas` | R13, R14 (Eras), R15 part 2, DC-B6 | `merge/border_resolver.rs::tests` | 2-partition fixture: `P0.agent0 = Con` with `ports[0] = FreePort(0)` (principal border), `ports[1] = FreePort(7)` (auxiliary border — call it `bid_aux`), `ports[2] = AgentPort(1, 0)` (local). `P1.agent0 = Era`, `ports[0] = FreePort(0)`. `plan.borders` includes BOTH border 0 and border 7. `BorderGraph` after init has both entries with `worker_a=0, worker_b=1` for 0 and border 7's other side inferred from the fixture. | After `resolve_border_redex(&mut graph, &[p0,p1], 0)`: (i) `resolution.resolved_borders == [(0, 0, 1)]` — ONLY the principal border; (ii) `resolution.new_borders.is_empty()` — DC-B6 forbids `add_border_states` for CON-ERA; (iii) `resolution.pending_commutations.len() >= 1` — workers mint 2 new ERA agents (one per aux port of the consumed CON); (iv) one `BorderDelta { border_id: 7, new_target: AgentPort(... pending new ERA id ..., 0) }` appears in the appropriate worker's `border_deltas` OR is marked as pending via the resolution's placeholder mechanism (implementation detail; see DC-B5 note — if the new ERA's id is not yet known at coordinator time, the delta carries a `PendingPortRef::Pending` token); (v) `graph.borders.contains_key(&0) == false`; (vi) `graph.borders.contains_key(&7) == true` (border 7 survives per DC-B6). | Principal border disappears; auxiliary border preserved, endpoint updated via `apply_deltas` semantics — NOT `add_border_states`. |
| UT-0374-04 | `resolve_dup_era_mirror_of_con_era_preserves_auxiliary_border` | R13, R14 (Eras), R15 part 2, DC-B6 | `merge/border_resolver.rs::tests` | Mirror of UT-0374-03 with `P0.agent0 = Dup` in place of `Con`. | Same structural assertions as UT-0374-03. | Symmetry — DUP-ERA and CON-ERA take the same `apply_deltas` preservation path. |
| UT-0374-05 | `border_id_allocator_produces_unique_ids_across_back_to_back_con_dups` | R15 part 3 (border-ID allocator); DC-B5 note that border IDs remain coordinator-owned | `merge/border_resolver.rs::tests` | Build a graph with two independent CON-DUP borders (0 and 1) on disjoint agent pairs. Allocate a `BorderIdAllocator::from_graph(&graph)` before resolving; resolve border 0, then resolve border 1. Collect all `pending_new_borders.border_id` values. | Every emitted new-border id is strictly greater than `max(existing_border_ids in plan.borders)`; no two emitted ids collide. If the implementation chooses to increment the allocator THROUGH resolutions (shared `&mut allocator`), the second resolution's ids are strictly greater than the first's. | Border-ID allocator monotonicity — required so that round-N+2 finalization does not collide with existing borders. |
| UT-0374-06 | `resolve_border_redex_dispatches_asymmetric_pairs_regardless_of_order` | R13 (dispatch), R14 (6-rule closure), DC-B2 (`assert_agent` reached symmetrically) | `merge/border_resolver.rs::tests` | Three fixtures — (Con on worker_a, Dup on worker_b), (Dup on worker_a, Con on worker_b), (Con on worker_a, Era on worker_b) — plus the three symbol-reversed variants. | For each fixture: `resolve_border_redex` returns a non-empty `BorderResolution` — for CON-DUP variants, `pending_commutations.is_empty() == false`; for CON-ERA/ DUP-ERA variants, `resolved_borders.len() == 1`. No panics; dispatch reaches the correct helper independent of which side was `worker_a`. | Dispatch table is complete for asymmetric pairs, and normalize_pair handles reversed ordering. |

### Detailed assertion sketches

**UT-0374-01** — CON-DUP pending emission.
```text
Fixture:
  P0 (worker 0): agent_0 = Con; ports = [FreePort(0), AgentPort(1,0), AgentPort(2,0)]
                 agent_1 = Era; agent_2 = Era
  P1 (worker 1): agent_0 = Dup; ports = [FreePort(0), AgentPort(3,0), AgentPort(4,0)]
                 agent_3 = Era; agent_4 = Era

Call:
  let r = resolve_border_redex(&mut graph, &[p0, p1], 0);

Assertions:
  assert_eq!(r.pending_commutations.len(), 2, "one PendingCommutation per worker");
  let w0_pc = r.pending_commutations.iter().find(|pc| pc.worker == 0).unwrap();
  let w1_pc = r.pending_commutations.iter().find(|pc| pc.worker == 1).unwrap();
  assert_eq!(w0_pc.target_symbols.len(), 2);
  assert_eq!(w1_pc.target_symbols.len(), 2);
  // SPEC-03 commutation topology: each worker mints one Con + one Dup.
  let mut s = w0_pc.target_symbols.clone();
  s.sort();
  assert_eq!(s, vec![Symbol::Con, Symbol::Dup]);

  // up to 4 new cross-partition wires; at least one for a non-trivial
  // fixture where none of the aux targets already crossed.
  assert!(r.pending_new_borders.len() <= 4);

  assert!(r.new_borders.is_empty(), "DC-B5: concrete new borders are \
      NOT finalized at resolver time — deferred to round N+2");
  assert_eq!(r.resolved_borders, vec![(0, 0, 1)]);
  assert!(!graph.borders.contains_key(&0));
```

**UT-0374-02** — placeholder tokens coherent.
```text
Fixture: same as UT-0374-01 but P0.agent_0.2 points to FreePort(99)
(an existing border). This ensures at least one new cross-partition
wire reuses the existing border structure OR creates a new one with
placeholder ports.

Assertions:
  for pnb in &r.pending_new_borders {
      let sides_are_pending_or_concrete = [&pnb.side_a, &pnb.side_b]
          .iter()
          .any(|s| matches!(s, PendingPortRef::Pending { .. }));
      assert!(sides_are_pending_or_concrete,
          "at least one side of a pending new border must be a \
           Pending token referring to a commutation_id");

      // Every Pending token's commutation_id must match an entry in
      // r.pending_commutations.
      for side in [&pnb.side_a, &pnb.side_b] {
          if let PendingPortRef::Pending { commutation_id, agent_slot, port_slot: _ } = side {
              assert!(
                  r.pending_commutations.iter().any(|pc| pc.commutation_id == *commutation_id),
                  "Pending token's commutation_id {} orphan — no matching \
                   entry in pending_commutations", commutation_id
              );
              // Agent slot must be within the target symbols list for that commutation.
              let pc = r.pending_commutations.iter()
                  .find(|pc| pc.commutation_id == *commutation_id).unwrap();
              assert!(
                  (*agent_slot as usize) < pc.target_symbols.len(),
                  "agent_slot {} out of bounds for commutation with {} symbols",
                  agent_slot, pc.target_symbols.len()
              );
          }
      }
  }
```

**UT-0374-03** — CON-ERA auxiliary preservation.
```text
Fixture: P0 CON with ports [FreePort(0), FreePort(7), AgentPort(1,0)]
         P1 ERA with ports [FreePort(0)]
         P_other (worker 2) provides the other side of border 7 — for
         this test, we reuse P1 if the fixture allows (border 7's
         other side is `P1.agent_0.1` — but Era has no aux ports.
         Adjust fixture so border 7's other side lives in a third
         partition, OR on P1 as a FreePort(7) in agent_1's auxiliary
         port.) — For simplicity the test builds a 2-partition fixture
         where border 7's other side is `AgentPort(1, 0)` on P1 (a
         Con agent whose principal connects to border 7).

After resolution of border 0:
  assert_eq!(r.resolved_borders, vec![(0, 0, 1)]);
  assert!(r.new_borders.is_empty());
  assert!(r.pending_commutations.len() >= 1,
      "worker 0 mints 2 new ERA agents to replace the consumed CON's \
       auxiliary-port neighbours (one ERA per aux port)");

  // Border 7 must still be in graph.borders.
  assert!(graph.borders.contains_key(&7),
      "DC-B6: auxiliary border preserved via apply_deltas, NOT removed");

  // The resolver must have emitted a BorderDelta or pending update
  // for border 7 with the new ERA's principal port as endpoint.
  let found_update_for_7 = r.worker_deltas.iter()
      .flat_map(|(_, wd)| wd.border_deltas.iter())
      .any(|d| d.border_id == 7);
  assert!(
      found_update_for_7
          || r.pending_new_borders.iter().any(|pnb| pnb.border_id == 7),
      "CON-ERA must update (not recreate) the auxiliary border"
  );
```

**UT-0374-04** — DUP-ERA mirror. Same as UT-0374-03 with `Symbol::Dup`.

**UT-0374-05** — allocator monotonicity.
```text
Given: graph with 2 CON-DUP borders (0, 1).
  let mut alloc = BorderIdAllocator::from_graph(&graph);
  let r0 = resolve_border_redex(&mut graph, &parts, 0);
  let r1 = resolve_border_redex(&mut graph, &parts, 1);

Collect:
  let all_new_ids: Vec<u32> = r0.pending_new_borders.iter()
      .chain(r1.pending_new_borders.iter())
      .map(|pnb| pnb.border_id)
      .collect();

Assertions:
  let max_existing = plan.borders.keys().copied().max().unwrap_or(0);
  for id in &all_new_ids {
      assert!(*id > max_existing,
          "new border id {} must be > max existing {}", id, max_existing);
  }
  // Uniqueness.
  let mut sorted = all_new_ids.clone();
  sorted.sort_unstable();
  sorted.dedup();
  assert_eq!(sorted.len(), all_new_ids.len(),
      "back-to-back resolutions produced duplicate border ids");
```

**UT-0374-06** — dispatch symmetry.
```text
For each (sym_a, sym_b) in [(Con,Dup), (Dup,Con), (Con,Era), (Era,Con),
                             (Dup,Era), (Era,Dup)]:
  let r = resolve_border_redex(&mut graph, &parts, 0);
  if matches!((sym_a, sym_b), (Symbol::Con, Symbol::Dup) | (Symbol::Dup, Symbol::Con)) {
      assert!(!r.pending_commutations.is_empty(),
          "CON-DUP dispatch reaches resolve_con_dup");
  } else {
      // CON-ERA / DUP-ERA path
      assert_eq!(r.resolved_borders.len(), 1);
  }
  assert!(!graph.borders.contains_key(&0));
```

---

## Adversarial / QA coverage map

| Requirement / DC | Covered by |
|---|---|
| R13 — dispatch reaches CON-DUP / CON-ERA / DUP-ERA helpers | UT-0374-01, UT-0374-03, UT-0374-04, UT-0374-06 |
| R14 — commutation topology (2 new CONs + 2 new DUPs) | UT-0374-01 |
| R14 — erasure topology (2 new ERAs inheriting aux targets) | UT-0374-03, UT-0374-04 |
| R15 part 2 — original border removed | UT-0374-01, UT-0374-03, UT-0374-04, UT-0374-05, UT-0374-06 |
| R15 part 3 — `BorderIdAllocator` monotonicity (new IDs for CON-DUP) | UT-0374-05 |
| DC-B5 — `pending_commutations` emission (one per worker, correct symbols) | UT-0374-01 |
| DC-B5 — `pending_new_borders` placeholder tokens referencing commutation_ids | UT-0374-02 |
| DC-B5 — `new_borders` empty at resolver time (finalization deferred) | UT-0374-01, UT-0374-02 |
| DC-B6 — CON-ERA / DUP-ERA preserve auxiliary-port borders (NO `add_border_states`) | UT-0374-03, UT-0374-04 |
| DC-B2 — asymmetric dispatch reaches `assert_agent` both ways | UT-0374-06 |

### QA adversarial angles (Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0374-A | `resolve_con_dup` calls `graph.add_border_states` (ignoring DC-B5 2-phase) | UT-0374-01 fires on `new_borders.is_empty()` assertion |
| QA-0374-B | `resolve_con_era` removes the auxiliary border (forgets DC-B6) | UT-0374-03 fires on `graph.borders.contains_key(&7)` |
| QA-0374-C | `resolve_dup_era` uses CON topology (swapped symbols for new agents) | UT-0374-04 catches via inherited assertions |
| QA-0374-D | `BorderIdAllocator::from_graph` uses `borders.len()` as seed instead of `max(id) + 1` | UT-0374-05 fires — allocated id can collide with existing |
| QA-0374-E | `PendingCommutation.target_symbols` has wrong multiplicity (e.g. 4 per worker instead of 2) | UT-0374-01 catches (`target_symbols.len() == 2`) |
| QA-0374-F | `PendingPortRef::Pending` tokens reference non-existent commutation_ids (stale/typo) | UT-0374-02 fires on the orphan-check assertion |
| QA-0374-G | CON-ERA emits `add_border_states` by mistake (future refactor confuses DC-B6 with DC-B5) | UT-0374-03's `new_borders.is_empty()` fires |
| QA-0374-H | Dispatcher falls through to `unimplemented!()` for `(Era, Con)` — same-rule but mirrored args | UT-0374-06 catches via reverse-ordered fixture |
| QA-0374-I | Resolver stores `resolved_borders: Vec<u32>` (pre-DC-B7 shape) | UT-0374-01 fires at the `vec![(0, 0, 1)]` equality assertion |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 1002 → **1008** (+6 new
   `#[test]` fns).
2. `cargo test --workspace --lib --features zero-copy` count: 1042 →
   **1048** (+6).
3. `cargo build --workspace` clean (default features).
4. `cargo build --workspace --features zero-copy` clean.
5. `cargo clippy --workspace --all-targets -- -D warnings` clean.
6. `cargo fmt --check` clean.
7. Manual grep guard still passes.
8. No `unwrap()` in production code within `border_resolver.rs`.
9. The `BorderIdAllocator` struct has `pub(crate)` visibility; its
   state is not leaked outside `merge/`.

---

## Resolved ambiguities

- **UT-0374-03 border 7 fixture.** CON-ERA with ONE auxiliary-port
  border is the minimum fixture to exercise DC-B6. A two-auxiliary-
  border fixture would be redundant for the contract (the rule applies
  independently per aux port). The TEST-SPEC fixes a single-aux fixture;
  a future regression test MAY extend to both.
- **Pending tokens on the "concrete side" of a `PendingNewBorder`.**
  When one side of a new cross-partition wire is a live agent (not part
  of the commutation — e.g., a border re-inheriting an existing aux
  target), that side is `PendingPortRef::Concrete(PortRef)`. UT-0374-02
  asserts AT LEAST one side is `Pending`; both sides Pending is
  permitted (when both endpoints are newly-minted agents).
- **UT-0374-06 dispatch completeness.** The test does NOT exercise
  `(Con, Con)` / `(Dup, Dup)` / `(Era, Era)` — those are covered by
  TEST-SPEC-0373. Together the two TEST-SPECs cover all 9 symbol-pair
  dispatch arms (3 × 3).
- **CON-ERA's pending_commutations count.** Per SPEC-03, CON-ERA
  produces 2 new ERA agents on ONE worker (the worker owning the
  consumed CON). DUP-ERA is symmetric. The `pending_commutations`
  vector therefore has exactly 1 entry in UT-0374-03 (not 2 as in
  CON-DUP).
- **Fixture builder utility.** The developer MAY factor out common
  fixture construction into private helpers inside
  `#[cfg(test)] mod tests` (e.g. `fn make_two_partition_fixture(sym_a:
  Symbol, sym_b: Symbol) -> (PartitionPlan, Vec<Partition>, BorderGraph)`);
  this TEST-SPEC does not mandate the factoring but permits it.

---

## Test count delta

**+6 tests** (default + zero-copy). Running total after this task:
1008 lib / 1048 lib.
