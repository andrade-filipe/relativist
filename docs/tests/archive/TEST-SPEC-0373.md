# TEST-SPEC-0373: `resolve_border_redex` dispatcher + annihilation/void paths (CON-CON, DUP-DUP, ERA-ERA)

**Task:** TASK-0373
**Spec:** SPEC-19 §3.2 R13 (dispatch via IC rules), R14 (6-rule closure —
  annihilation/void half), R15 part 2 (`remove_border` post-resolution);
  SPEC-19 §3.3 (item 2.26); 2.26-B spec-critic DC-B2 (`assert_agent`
  helper + panic format), DC-B3 (split `WorkerDeltas` — `border_deltas`
  vs `local_reconnections`), DC-B7 (`resolved_borders` triples).
**Generated:** 2026-04-17
**Baseline before this task:** 996 lib (default) / 1036 lib
  (`--features zero-copy`) — post-TEST-SPEC-0372.
**Cumulative target after this task:** 1002 lib (default) / 1042 lib
  (`--features zero-copy`) — **+6** new `#[test]` fns in
  `merge::border_resolver::tests`.

---

## Scope note

TASK-0373 adds the top-level `resolve_border_redex` entry point plus
three private rule bodies `resolve_con_con`, `resolve_dup_dup`,
`resolve_era_era`, and the private helper `assert_agent` (DC-B2
caller-side panic). The `BorderResolution` struct is introduced in its
DC-B3 + DC-B7 shape:

```rust
pub(crate) struct WorkerDeltas {
    pub(crate) border_deltas: Vec<BorderDelta>,
    pub(crate) local_reconnections: Vec<(PortRef, PortRef)>,
}
pub(crate) struct BorderResolution {
    pub(crate) worker_deltas: Vec<(WorkerId, WorkerDeltas)>,
    pub(crate) resolved_borders: Vec<(u32, WorkerId, WorkerId)>,  // DC-B7 triples
    pub(crate) new_borders: Vec<AddBorderEntry>,                  // empty for CON-CON/DUP-DUP/ERA-ERA
}
```

**Three contracts under test:**

1. **Rule dispatch.** `resolve_border_redex` reads both sides via
   `materialize_agent` + `assert_agent`, pattern-matches on `(sym_a,
   sym_b)`, and routes to the correct private body. Asymmetric pairs
   (e.g. `(Con, Dup)`) are out of scope for this task; they land in
   TASK-0374. The dispatcher MUST compile with a `match` arm that
   `todo!()`/panics for the three asymmetric pairs OR delegates to a
   stub that TASK-0374 fills in. This TEST-SPEC does not assert on
   asymmetric dispatch behaviour (it is TASK-0374's deliverable).
2. **Same-symbol rule topology.** CON-CON uses the cross pattern
   (`a.1 ↔ b.2`, `a.2 ↔ b.1`); DUP-DUP uses the parallel pattern
   (`a.1 ↔ b.1`, `a.2 ↔ b.2`); ERA-ERA has zero auxiliary ports and
   yields empty delta vectors. The resolver simulates `interact_anni`
   / `interact_void` on read-only partition views, emitting the wire
   updates each worker must apply locally.
3. **BorderGraph side-effects.** After every same-symbol resolution,
   `graph.borders.contains_key(&bid) == false`,
   `graph.active_redexes.contains(&bid) == false`, and
   `resolution.resolved_borders == [(bid, worker_a, worker_b)]`
   (exactly one triple per resolved border). `resolution.new_borders`
   MUST be empty for all three same-symbol rules.

**Out of scope:**
- CON-DUP 2-phase `pending_commutations` path (DC-B5) → TEST-SPEC-0374.
- CON-ERA / DUP-ERA `apply_deltas` preservation (DC-B6) → TEST-SPEC-0374.
- `package_resolutions` fan-out → TEST-SPEC-0375.
- `assert_agent`'s panic in the "agent missing" branch is partially
  exercised here (UT-0373-06) with a cache-desync fixture; the exact
  panic message format follows the DC-B2 spec-critic directive.

---

## Test target file paths

- `relativist-core/src/merge/border_resolver.rs::tests` — 6 new
  `#[test]` fns appended to the existing `#[cfg(test)] mod tests` block
  (seeded by TASK-0372).

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Unit Tests

| Test ID | Name | Reqs covered | File | Preconditions | Assertions | Expected outcome |
|---------|------|--------------|------|---------------|------------|------------------|
| UT-0373-01 | `resolve_con_con_removes_border_and_emits_cross_pattern_deltas` | R13, R14 (Anni), R15 part 2, DC-B3, DC-B7 | `merge/border_resolver.rs::tests` | 2-partition fixture: Con at `AgentId(0)` in P0 with aux targets `AgentPort(1,0)` (local), `AgentPort(2,0)` (local); Con at `AgentId(0)` in P1 with aux targets `AgentPort(3,0)`, `AgentPort(4,0)`; border 0 connects `P0.agent0.0` ↔ `P1.agent0.0`. | `resolve_border_redex(&mut graph, &[p0,p1], 0)` returns a `BorderResolution` where (i) `resolved_borders == [(0, 0, 1)]`; (ii) `new_borders.is_empty()`; (iii) worker 0's `WorkerDeltas.local_reconnections` contains 2 entries pairing P0's former aux-targets with P1's former aux-targets under the cross pattern (`P0.a.1's target ↔ P1.b.2's target`, `P0.a.2's target ↔ P1.b.1's target`); (iv) `graph.borders.contains_key(&0) == false`; (v) `graph.active_redexes.contains(&0) == false`. | Cross pattern reconnects correctly; border removed. |
| UT-0373-02 | `resolve_dup_dup_removes_border_and_emits_parallel_pattern_deltas` | R13, R14 (Anni parallel), R15 part 2, DC-B3, DC-B7 | `merge/border_resolver.rs::tests` | Same shape as UT-0373-01 but both agents are `Dup`. | Same structural assertions as UT-0373-01 except the pairing pattern is PARALLEL: `P0.a.1's target ↔ P1.b.1's target` and `P0.a.2's target ↔ P1.b.2's target`. `resolved_borders == [(0, 0, 1)]`, border removed. | Parallel pattern; border removed; no new borders. |
| UT-0373-03 | `resolve_era_era_removes_border_with_zero_deltas` | R13, R14 (Void), R15 part 2, DC-B3, DC-B7 | `merge/border_resolver.rs::tests` | 2-partition fixture: Era at `AgentId(0)` in P0, Era at `AgentId(0)` in P1, border 0 wires their principals. Era has arity 0 — no auxiliary-port targets to carry. | `resolution.resolved_borders == [(0, 0, 1)]`, `resolution.new_borders.is_empty()`, BOTH `worker_deltas[0].1.border_deltas.is_empty()` AND `worker_deltas[0].1.local_reconnections.is_empty()` (symmetrically for worker 1; empty entries may be absent from `worker_deltas` by convention — assert `worker_deltas` slice if present has all-empty inner vectors). `graph.borders.contains_key(&0) == false`. | ERA-ERA carries no auxiliary-port payload; border disappears with zero reconnection work. |
| UT-0373-04 | `resolve_border_redex_dispatches_by_symbol_pair_and_normalizes_order` | R13, R14 (dispatch table), DC-B2 (helper) | `merge/border_resolver.rs::tests` | Build CON-CON fixture; then build a MIRROR fixture where the two agents are swapped between partitions (P1 now owns `worker_a` role, P0 owns `worker_b`). | Dispatcher reaches the same private body regardless of which worker's side is labelled `side_a` vs `side_b`. Resolution outputs are isomorphic modulo the `(worker_a, worker_b)` triple in `resolved_borders` (which reflects the ACTUAL `BorderState.worker_a / worker_b`). Both cases: border removed, resolved_borders has one triple, new_borders empty. | `normalize_pair` / symmetric handling works — no crash, correct routing. |
| UT-0373-05 | `resolve_border_redex_on_con_con_preserves_graph_non_redex_borders` | R15 part 2 (targeted removal, not global clear) | `merge/border_resolver.rs::tests` | 2-partition fixture with two borders: border 0 is a CON-CON redex (both principal); border 1 is a non-redex (one principal, one auxiliary). Call `resolve_border_redex(&mut graph, &parts, 0)`. | After resolution: `graph.borders.contains_key(&0) == false`; `graph.borders.contains_key(&1) == true`; `graph.active_redexes.is_empty()` (border 1 was never a redex). `resolution.resolved_borders` has exactly `(0, _, _)` — not `(1, _, _)`. | Targeted removal — the resolver touches ONLY the passed `border_id`. |
| UT-0373-06 | `resolve_border_redex_panics_with_dc_b2_message_on_missing_agent` | DC-B2 (caller-side `assert_agent` panic format) | `merge/border_resolver.rs::tests` | 2-partition fixture with a CON-CON border; then DELIBERATELY vacate `P0.agent0` via `remove_agent` AFTER `BorderGraph::from_partition_plan` (simulates cache desync). Call `resolve_border_redex(&mut graph, &[p0,p1], 0)` inside `std::panic::catch_unwind`. | `catch_unwind` returns `Err(_)`; the panic payload (downcast to `&str` or `String`) contains substrings `"border_resolver: agent missing for border 0"`, `"side "`, and `"cache desync"` AND `"DC-B1"` (DC-B2 verdict format). | Panic is uniform, grep-able, and names the fix direction (check cache maintenance per DC-B1). |

### Detailed fixture and assertion sketches

**UT-0373-01** — CON-CON cross pattern.
```text
Fixture P0 (worker_id=0):
  agent_0: Con, ports = [FreePort(0) /*border*/, AgentPort(1, 0), AgentPort(2, 0)]
  agent_1: Era (local target for P0.agent_0.1)
  agent_2: Era (local target for P0.agent_0.2)
  free_port_index = { 0 -> AgentPort(0, 0) }

Fixture P1 (worker_id=1):
  agent_0: Con, ports = [FreePort(0) /*border*/, AgentPort(3, 0), AgentPort(4, 0)]
  agent_3: Era (local target for P1.agent_0.1)
  agent_4: Era (local target for P1.agent_0.2)
  free_port_index = { 0 -> AgentPort(0, 0) }

plan.borders = { 0 -> (P0's view, P1's view) }  // exact PortRefs from split

BorderGraph after from_partition_plan:
  borders[0] = BorderState { side_a = AgentPort(0,0) /*P0*/, side_b = AgentPort(0,0) /*P1*/,
                              worker_a = 0, worker_b = 1, is_redex = true }
  active_redexes = {0}

Call:
  let r = resolve_border_redex(&mut graph, &[p0, p1], 0);

Assertions:
  assert_eq!(r.resolved_borders, vec![(0, 0, 1)]);
  assert!(r.new_borders.is_empty());

  // CON-CON cross pattern: P0.agent_0.1 now connects to P1.agent_0.2's
  // former target (P1.agent_4), and P0.agent_0.2 now connects to
  // P1.agent_0.1's former target (P1.agent_3). Symmetric on worker 1.
  let w0 = r.worker_deltas.iter().find(|(w, _)| *w == 0).unwrap();
  // Cross-partition aux connections stay as BorderDeltas / local reconnections
  // per whether the paired endpoint is remote or local.
  assert_eq!(
      w0.1.local_reconnections.len(),
      2,
      "CON-CON P0: 2 new aux-to-aux links land as local reconnections \
       when both remote targets resolve through cross-partition wires \
       (see DC-B3 for the rule)"
  );

  // Border 0 removed from the graph.
  assert!(!graph.borders.contains_key(&0));
  assert!(!graph.active_redexes.contains(&0));
```

**UT-0373-02** — DUP-DUP parallel pattern. Identical shape to UT-0373-01
but the private helper `resolve_dup_dup` pairs (a.1, b.1) and (a.2, b.2)
rather than the cross pattern. Assertions mirror UT-0373-01 with
`Symbol::Dup` fixtures and the parallel-pattern pairing in
`local_reconnections` entries.

**UT-0373-03** — ERA-ERA void.
```text
Fixture P0: agent_0: Era, ports = [FreePort(0)]
Fixture P1: agent_0: Era, ports = [FreePort(0)]

Call:
  let r = resolve_border_redex(&mut graph, &[p0, p1], 0);

Assertions:
  assert_eq!(r.resolved_borders, vec![(0, 0, 1)]);
  assert!(r.new_borders.is_empty());
  // All worker_deltas entries (if present) have empty inner vectors.
  for (_w, wd) in &r.worker_deltas {
      assert!(wd.border_deltas.is_empty());
      assert!(wd.local_reconnections.is_empty());
  }
  assert!(!graph.borders.contains_key(&0));
```

**UT-0373-04** — dispatch + normalize. Build TWO variants of the CON-CON
fixture with `worker_a` and `worker_b` swapped in the `BorderState`;
assert resolution succeeds in both, with matching `resolved_borders`
triple that reflects the CURRENT `BorderState` orientation.

**UT-0373-05** — targeted removal.
```text
Graph has two borders: (0) CON-CON principal-principal, (1) CON-Era
principal-auxiliary (non-redex). resolve_border_redex(_, _, 0) removes
ONLY border 0.
```

**UT-0373-06** — DC-B2 panic format.
```text
Fixture CON-CON. After BorderGraph::from_partition_plan, mutate P0 to
remove agent 0 (simulating cache desync with respect to the graph):
  p0.subnet.remove_agent(0);

Call inside catch_unwind:
  let result = std::panic::catch_unwind(
      std::panic::AssertUnwindSafe(|| resolve_border_redex(&mut graph, &[p0, p1], 0))
  );

Assertions:
  let err = result.expect_err("resolver must panic on cache desync");
  let msg = err.downcast_ref::<&str>()
      .map(|s| (*s).to_string())
      .or_else(|| err.downcast_ref::<String>().cloned())
      .unwrap_or_default();
  assert!(msg.contains("border_resolver: agent missing for border 0"),
      "panic message missing header: {msg}");
  assert!(msg.contains("cache desync"), "missing cache-desync hint: {msg}");
  assert!(msg.contains("DC-B1"),
      "panic message must reference DC-B1 per spec-critic DC-B2 verdict: {msg}");
```

---

## Adversarial / QA coverage map

| Requirement / DC | Covered by |
|---|---|
| R13 — dispatch reaches the correct rule body per `(sym_a, sym_b)` | UT-0373-01, UT-0373-02, UT-0373-03, UT-0373-04 |
| R14 — CON-CON annihilation (cross) topology | UT-0373-01 |
| R14 — DUP-DUP annihilation (parallel) topology | UT-0373-02 |
| R14 — ERA-ERA void (no auxiliaries) | UT-0373-03 |
| R15 part 2 — `remove_border(bid)` post-resolution | UT-0373-01, UT-0373-02, UT-0373-03, UT-0373-05 |
| R15 part 2 — `active_redexes` bookkeeping | UT-0373-01, UT-0373-02, UT-0373-03 |
| DC-B2 — `assert_agent` panic format (header + cache-desync hint + DC-B1 pointer) | UT-0373-06 |
| DC-B3 — `WorkerDeltas` shape (split `border_deltas` / `local_reconnections`) | UT-0373-01, UT-0373-02, UT-0373-03 |
| DC-B7 — `resolved_borders: Vec<(u32, WorkerId, WorkerId)>` triples | UT-0373-01, UT-0373-02, UT-0373-03, UT-0373-04, UT-0373-05 |
| DC-B1 (cache consistency — negative path) | UT-0373-06 |
| Non-scope: asymmetric dispatch (`(Con, Dup)`, `(Con, Era)`, `(Dup, Era)`) | covered in TEST-SPEC-0374 |

### QA adversarial angles (Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0373-A | `resolve_con_con` swaps its pattern to parallel | UT-0373-01 asserts cross pattern by target identity; fails loudly |
| QA-0373-B | `resolve_dup_dup` reuses the cross pattern | UT-0373-02 fails by symmetric inspection |
| QA-0373-C | `resolve_era_era` mistakenly emits `BorderDelta` entries (e.g. copying CON-CON logic by accident) | UT-0373-03 asserts empty inner vectors |
| QA-0373-D | `remove_border` called on wrong `bid` (typo — passes state's border_id directly instead of the input parameter) | UT-0373-05 (two-border fixture) catches: wrong border survives |
| QA-0373-E | Panic message drifts (e.g. translator changes "desync" to "mismatch") | UT-0373-06 substring assertions fail |
| QA-0373-F | Dispatcher swaps `(Con, Dup)` and `(Dup, Con)` to different helpers | Not in scope here; TEST-SPEC-0374 catches |
| QA-0373-G | Resolver mutates input partitions (writes back to `&[Partition]`) | Compile-time guard — `&[Partition]` is immutable; structural violation would fail to compile. No runtime test needed. |
| QA-0373-H | `assert_agent` helper swallows `None` into a `unwrap_or_default()` (returns `(AgentId(0), Symbol::Con)`) | UT-0373-06 fires — no panic means the test fails |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 996 → **1002** (+6 new
   `#[test]` fns).
2. `cargo test --workspace --lib --features zero-copy` count: 1036 →
   **1042** (+6).
3. `cargo build --workspace` clean (default features).
4. `cargo build --workspace --features zero-copy` clean.
5. `cargo clippy --workspace --all-targets -- -D warnings` clean.
6. `cargo fmt --check` clean.
7. Manual grep guard still passes on `border_resolver.rs`.
8. No `unwrap()` in production code within `border_resolver.rs`
   (the DC-B2 panic site uses `expect(...)` or `panic!(...)` with the
   prescribed message).

---

## Resolved ambiguities

- **UT-0373-01 / UT-0373-02 `local_reconnections` vs `border_deltas`
  split.** The fixtures place BOTH auxiliary-port targets of each
  border-principal agent in the OWN partition (`AgentPort(1, 0)`,
  `AgentPort(2, 0)` locally). Per DC-B3, the correct delta type here
  is `local_reconnections` (the aux-to-aux reconnection is purely local
  to each worker). If a future fixture places the aux target across a
  border, the same-index entry is instead a `BorderDelta`. The test
  SPEC fixes the local-only topology to pin the common-path behaviour;
  cross-partition aux targets are exercised in TEST-SPEC-0376's
  integration fixtures.
- **UT-0373-03 `worker_deltas` shape.** The developer MAY include
  `(0, empty_WorkerDeltas)` and `(1, empty_WorkerDeltas)` entries for
  symmetry OR omit them entirely (an empty outer vector). The assertion
  is "every present entry has empty inner vectors"; the test does NOT
  force presence/absence.
- **UT-0373-06 panic plumbing.** `std::panic::catch_unwind` + downcast
  to `&str` then `String` is the standard test pattern in
  `relativist-core`. If the DEV stage chooses `expect(...)` on an
  `Option`, the panic payload is a `&'static str` from `expect`; if
  `panic!("...")`, it is a `String`. The test must handle both downcast
  paths.

---

## Test count delta

**+6 tests** (default + zero-copy). Running total after this task:
1002 lib / 1042 lib.
