# TEST-SPEC-0375: `RoundStartDispatch` + `package_resolutions` — group deltas by worker

**Task:** TASK-0375
**Spec:** SPEC-19 §3.2 R15 part 1 (send port-update deltas to affected
  workers); SPEC-19 §3.3 R23 (coordinator sends `RoundStart` to EVERY
  worker every round — includes workers with empty payload);
  2.26-B spec-critic DC-B3 (`local_reconnections` parallel field on
  `RoundStartDispatch`), DC-B5 (`pending_commutations` fanned per worker),
  DC-B7 (`resolved_borders` triples fanned into both workers' `Vec<u32>`).
**Generated:** 2026-04-17
**Baseline before this task:** 1008 lib (default) / 1048 lib
  (`--features zero-copy`) — post-TEST-SPEC-0374.
**Cumulative target after this task:** 1013 lib (default) / 1053 lib
  (`--features zero-copy`) — **+5** new `#[test]` fns in
  `merge::border_resolver::tests`.

---

## Scope note

TASK-0375 lands the pure-core packaging layer that groups a batch of
`BorderResolution` outputs into a per-worker `RoundStartDispatch`. Shape
under DC-B3 + DC-B5 + DC-B7 amendments:

```rust
#[derive(Debug, Clone, Default)]
pub(crate) struct RoundStartDispatch {
    pub(crate) border_deltas: Vec<BorderDelta>,
    pub(crate) local_reconnections: Vec<(PortRef, PortRef)>,     // DC-B3
    pub(crate) resolved_borders: Vec<u32>,                        // DC-B7 fan-out
    pub(crate) new_borders: Vec<(u32, PortRef)>,                  // DC-B5: empty for CON-DUP (deferred)
    pub(crate) pending_commutations: Vec<PendingCommutation>,     // DC-B5
}

pub(crate) fn package_resolutions(
    resolutions: Vec<BorderResolution>,
    num_workers: usize,
) -> Vec<(WorkerId, RoundStartDispatch)>;
```

**Five contracts under test:**

1. **Empty input.** `package_resolutions(vec![], N)` returns `N` entries,
   each with `RoundStartDispatch::default()` (all vectors empty), keys
   `(0..N)` ascending.
2. **Fan-out by worker.** A single `BorderResolution` with
   `worker_deltas` for workers 0 and 1 (both `WorkerDeltas` populated)
   lands in `result[0]` and `result[1]` respectively; other workers
   receive empty dispatches.
3. **DC-B7 triple fan.** A resolution with `resolved_borders = [(0, 0,
   1)]` produces `result[0].resolved_borders == [0]` AND
   `result[1].resolved_borders == [0]` — both workers remove the
   `FreePort` entry.
4. **DC-B5 `pending_commutations` fan by worker.** Per DC-B5 Option (B),
   each `PendingCommutation` has a `worker: WorkerId` field; fanning
   sends each entry to exactly one worker's dispatch.
5. **Determinism.** Calling `package_resolutions` twice with identical
   inputs yields byte-identical outputs (`Vec` ordering stable by
   `WorkerId` ascending; within a dispatch, per-worker order preserved
   from resolution input).

**Out of scope:**
- Wire mapping `RoundStartDispatch → Message::RoundStart` → 2.26-C memo.
- Integration with `resolve_border_redex` → TEST-SPEC-0376.

---

## Test target file paths

- `relativist-core/src/merge/border_resolver.rs::tests` — 5 new
  `#[test]` fns appended to the `#[cfg(test)] mod tests` block.

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Unit Tests

| Test ID | Name | Reqs covered | File | Preconditions | Assertions | Expected outcome |
|---------|------|--------------|------|---------------|------------|------------------|
| UT-0375-01 | `package_resolutions_empty_produces_one_default_per_worker` | R23 (every worker addressed even when empty) | `merge/border_resolver.rs::tests` | `package_resolutions(vec![], 3)`. | Returns exactly 3 entries: `[(0, default), (1, default), (2, default)]`; each inner `RoundStartDispatch` has `border_deltas.is_empty()`, `local_reconnections.is_empty()`, `resolved_borders.is_empty()`, `new_borders.is_empty()`, `pending_commutations.is_empty()`. Keys are `0, 1, 2` in order. | Per R23, coordinator STILL sends `RoundStart` to every worker — packaging preserves this by defaulting absent workers. |
| UT-0375-02 | `package_resolutions_fans_worker_deltas_to_correct_dispatch` | R15 part 1, DC-B3 | `merge/border_resolver.rs::tests` | Build one `BorderResolution` with `worker_deltas = vec![(0, WorkerDeltas { border_deltas: vec![BorderDelta { border_id: 0, new_target: AgentPort(7,0) }], local_reconnections: vec![(AgentPort(1,1), AgentPort(2,1))] }), (1, WorkerDeltas { border_deltas: vec![BorderDelta { border_id: 0, new_target: AgentPort(9,0) }], local_reconnections: vec![] })]`, `resolved_borders = vec![(0, 0, 1)]`, `new_borders = vec![]`, `pending_commutations = vec![]`. Call `package_resolutions(vec![resolution], 3)`. | `result[0].1.border_deltas == [BorderDelta{0, AgentPort(7,0)}]`; `result[0].1.local_reconnections == [(AgentPort(1,1), AgentPort(2,1))]`; `result[1].1.border_deltas == [BorderDelta{0, AgentPort(9,0)}]`; `result[1].1.local_reconnections.is_empty()`; `result[2].1` is entirely default. | Worker-keyed fan-out preserves DC-B3 two-vector split. |
| UT-0375-03 | `package_resolutions_fans_resolved_border_triples_to_both_workers` | DC-B7 (triples fan) | `merge/border_resolver.rs::tests` | One `BorderResolution` with `resolved_borders = vec![(0, 0, 1), (5, 1, 2)]` and otherwise empty. Call `package_resolutions(vec![resolution], 3)`. | `result[0].1.resolved_borders == vec![0]` (worker 0 is side_a of border 0); `result[1].1.resolved_borders == vec![0, 5]` (worker 1 is side_b of 0 AND side_a of 5); `result[2].1.resolved_borders == vec![5]` (worker 2 is side_b of 5). Order within each dispatch preserves the input order from `resolutions` iteration. | Every worker whose side was consumed receives the `bid` so it can remove its `FreePort` entry per R23 semantics. |
| UT-0375-04 | `package_resolutions_fans_pending_commutations_to_owning_worker` | DC-B5 (`PendingCommutation.worker` fan) | `merge/border_resolver.rs::tests` | One `BorderResolution` with `pending_commutations = vec![PendingCommutation { commutation_id: 1, worker: 0, target_symbols: vec![Symbol::Con, Symbol::Dup], local_wiring: vec![] }, PendingCommutation { commutation_id: 2, worker: 1, target_symbols: vec![Symbol::Con, Symbol::Dup], local_wiring: vec![] }]` and otherwise empty. Call `package_resolutions(vec![resolution], 3)`. | `result[0].1.pending_commutations.len() == 1` and `result[0].1.pending_commutations[0].commutation_id == 1`; `result[1].1.pending_commutations.len() == 1` and `result[1].1.pending_commutations[0].commutation_id == 2`; `result[2].1.pending_commutations.is_empty()`. | Each pending_commutation travels to exactly its owning worker; no duplication across workers. |
| UT-0375-05 | `package_resolutions_is_deterministic_and_ordered_by_worker_id` | R15 part 1 (deterministic packaging) | `merge/border_resolver.rs::tests` | Two `BorderResolution`s: one affects workers 2 and 0 (in that insertion order), the other affects worker 1. Build input `resolutions = vec![r_a, r_b]`. Call `package_resolutions` twice with clones; compare outputs. | Output from both calls is bitwise identical (element-equal via `PartialEq` on the entire `Vec<(WorkerId, RoundStartDispatch)>`). Output keys are `0, 1, 2` ascending regardless of input insertion order. Within each dispatch, `border_deltas` / `local_reconnections` / `pending_commutations` preserve source-order from input resolutions. | Determinism is essential for reproducible coordinator dispatch under a replayable BSP loop. |

### Detailed fixture and assertion sketches

**UT-0375-01** — empty input.
```text
let result = package_resolutions(vec![], 3);
assert_eq!(result.len(), 3);
for (i, (wid, dispatch)) in result.iter().enumerate() {
    assert_eq!(*wid, i as WorkerId);
    assert!(dispatch.border_deltas.is_empty());
    assert!(dispatch.local_reconnections.is_empty());
    assert!(dispatch.resolved_borders.is_empty());
    assert!(dispatch.new_borders.is_empty());
    assert!(dispatch.pending_commutations.is_empty());
}
```

**UT-0375-02** — DC-B3 fan-out.
```text
let resolution = BorderResolution {
    worker_deltas: vec![
        (0, WorkerDeltas {
            border_deltas: vec![BorderDelta { border_id: 0, new_target: PortRef::AgentPort(7, 0) }],
            local_reconnections: vec![(PortRef::AgentPort(1, 1), PortRef::AgentPort(2, 1))],
        }),
        (1, WorkerDeltas {
            border_deltas: vec![BorderDelta { border_id: 0, new_target: PortRef::AgentPort(9, 0) }],
            local_reconnections: vec![],
        }),
    ],
    resolved_borders: vec![(0, 0, 1)],
    new_borders: vec![],
    pending_commutations: vec![],
    pending_new_borders: vec![],
};
let result = package_resolutions(vec![resolution], 3);

// Worker 0
assert_eq!(result[0].0, 0);
assert_eq!(result[0].1.border_deltas.len(), 1);
assert_eq!(result[0].1.border_deltas[0].border_id, 0);
assert_eq!(result[0].1.border_deltas[0].new_target, PortRef::AgentPort(7, 0));
assert_eq!(result[0].1.local_reconnections,
    vec![(PortRef::AgentPort(1, 1), PortRef::AgentPort(2, 1))]);
// Worker 0 is side_a of the resolved border => bid fans here.
assert_eq!(result[0].1.resolved_borders, vec![0]);

// Worker 1 symmetric.
assert_eq!(result[1].0, 1);
assert_eq!(result[1].1.border_deltas.len(), 1);
assert_eq!(result[1].1.border_deltas[0].new_target, PortRef::AgentPort(9, 0));
assert!(result[1].1.local_reconnections.is_empty());
assert_eq!(result[1].1.resolved_borders, vec![0]);

// Worker 2 untouched.
assert!(result[2].1.border_deltas.is_empty());
assert!(result[2].1.local_reconnections.is_empty());
assert!(result[2].1.resolved_borders.is_empty());
```

**UT-0375-03** — triple fan.
```text
let resolution = BorderResolution {
    worker_deltas: vec![],  // not the focus here
    resolved_borders: vec![(0, 0, 1), (5, 1, 2)],
    new_borders: vec![],
    pending_commutations: vec![],
    pending_new_borders: vec![],
};
let result = package_resolutions(vec![resolution], 3);

assert_eq!(result[0].1.resolved_borders, vec![0]);
assert_eq!(result[1].1.resolved_borders, vec![0, 5]); // order: 0 first (fanned from triple 1), 5 next
assert_eq!(result[2].1.resolved_borders, vec![5]);
```

**UT-0375-04** — pending fan.
```text
let resolution = BorderResolution {
    worker_deltas: vec![],
    resolved_borders: vec![],
    new_borders: vec![],
    pending_commutations: vec![
        PendingCommutation { commutation_id: 1, worker: 0, target_symbols: vec![Symbol::Con, Symbol::Dup], local_wiring: vec![] },
        PendingCommutation { commutation_id: 2, worker: 1, target_symbols: vec![Symbol::Con, Symbol::Dup], local_wiring: vec![] },
    ],
    pending_new_borders: vec![],
};
let result = package_resolutions(vec![resolution], 3);

assert_eq!(result[0].1.pending_commutations.len(), 1);
assert_eq!(result[0].1.pending_commutations[0].commutation_id, 1);
assert_eq!(result[1].1.pending_commutations.len(), 1);
assert_eq!(result[1].1.pending_commutations[0].commutation_id, 2);
assert!(result[2].1.pending_commutations.is_empty());
```

**UT-0375-05** — determinism.
```text
let r_a = BorderResolution { /* affects workers 2, 0 via worker_deltas */ };
let r_b = BorderResolution { /* affects worker 1 */ };
let input = vec![r_a, r_b];

let out1 = package_resolutions(input.clone(), 3);
let out2 = package_resolutions(input.clone(), 3);

assert_eq!(out1, out2);  // RoundStartDispatch + WorkerId implement PartialEq
assert_eq!(out1[0].0, 0);
assert_eq!(out1[1].0, 1);
assert_eq!(out1[2].0, 2);
```

---

## Adversarial / QA coverage map

| Requirement / DC | Covered by |
|---|---|
| R15 part 1 — deltas grouped by owning worker | UT-0375-02 |
| R23 — every worker addressed each round (empty dispatch is still present) | UT-0375-01, UT-0375-02 (worker 2 empty branch) |
| DC-B3 — `local_reconnections` fanned per worker in parallel to `border_deltas` | UT-0375-02 |
| DC-B5 — `pending_commutations` fanned to owning worker via `.worker` field | UT-0375-04 |
| DC-B5 — `new_borders: Vec<(u32, PortRef)>` preserved in dispatch (empty for this bundle) | UT-0375-02, UT-0375-04 (empty-field persistence) |
| DC-B7 — triples `(bid, wa, wb)` fanned to BOTH workers' `resolved_borders: Vec<u32>` | UT-0375-03 |
| Determinism — output ordering stable by WorkerId | UT-0375-01, UT-0375-05 |

### QA adversarial angles (Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0375-A | `package_resolutions` omits workers with empty dispatches | UT-0375-01 fires (len != 3) |
| QA-0375-B | DC-B7 triple fanned to only one worker (implementation forgets to fan to both sides) | UT-0375-03 fires — one of the two workers missing the `bid` |
| QA-0375-C | `PendingCommutation` cloned to every worker (over-broadcast) | UT-0375-04 fires — worker 2 would have entries |
| QA-0375-D | Output ordering relies on HashMap iteration (non-deterministic) | UT-0375-05 fires on second call yielding different `Vec` |
| QA-0375-E | `worker_deltas` entry with `WorkerId >= num_workers` panics at packaging time | Not explicitly tested here — the contract is that resolver produces valid WorkerIds. A defensive panic would be appropriate; this TEST-SPEC does NOT specify it. |
| QA-0375-F | Implementation loses the order of `border_deltas` within a worker (sort by `border_id`) | UT-0375-05 catches via `assert_eq!` on entire output Vec — if sort accidentally applied, the Vec differs from input insertion order |
| QA-0375-G | Triple `(bid, wa, wb)` with `wa == wb` (self-border edge case) inserts twice into the same worker | UT-0375-03 does not exercise; implementation note says "push once" per TASK spec — regression to be QA'd later |
| QA-0375-H | `new_borders` field accidentally dropped by packaging (future refactor removes it because 2.26-B never populates it concretely) | UT-0375-02's empty-field persistence assertion fires — the field must still exist in `RoundStartDispatch` |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 1008 → **1013** (+5 new
   `#[test]` fns).
2. `cargo test --workspace --lib --features zero-copy` count: 1048 →
   **1053** (+5).
3. `cargo build --workspace` clean (default features).
4. `cargo build --workspace --features zero-copy` clean.
5. `cargo clippy --workspace --all-targets -- -D warnings` clean.
6. `cargo fmt --check` clean.
7. Manual grep guard still passes on `border_resolver.rs`.
8. `RoundStartDispatch` derives `PartialEq` and `Eq` (required by
   UT-0375-05's `assert_eq!` on the full output Vec).

---

## Resolved ambiguities

- **Ordering within a dispatch.** UT-0375-05 asserts the OUTPUT is
  deterministic across re-invocations. It does NOT prescribe whether
  the output order within a worker's `border_deltas` is "by source
  resolution order" or "by border_id ascending". Per TASK-0375 spec
  ("Output is sorted by `WorkerId` ascending for determinism" — applies
  to the outer Vec only), the implementation is free to choose within-
  dispatch ordering provided it is reproducible. The test uses
  `assert_eq!` on full `Vec` equality which locks WHICHEVER order the
  implementation settles on, preventing future drift.
- **Empty resolution shape.** UT-0375-01 fixes num_workers=3 but
  applies equally at num_workers=0 (empty Vec out) — the developer
  MAY add a 0-workers assertion as an inline helper, but it is not
  required by this TEST-SPEC.
- **`resolved_borders` fan order.** UT-0375-03 asserts
  `result[1].resolved_borders == vec![0, 5]` (border 0 before 5) —
  order mirrors the source triple order. If the developer accidentally
  uses a HashMap keyed by worker, the order may become non-deterministic
  and UT-0375-05 would fire on the second call; UT-0375-03 provides an
  additional positive ordering assertion.
- **DC-B7 self-border edge case (`wa == wb`).** Not exercised in this
  TEST-SPEC; TASK-0375 acceptance criterion says "push once" — a
  separate regression test may cover it but is NOT required by
  spec-critic per DC-B7 verdict.

---

## Test count delta

**+5 tests** (default + zero-copy). Running total after this task:
1013 lib / 1053 lib.
