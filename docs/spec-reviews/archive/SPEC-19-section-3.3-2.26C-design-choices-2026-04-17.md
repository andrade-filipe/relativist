# SPEC-19 §3.3 (2.26-C) — Design Choices Verdict (Stage 1.5 spec-critic)

**Date:** 2026-04-17
**Reviewer:** spec-critic (adversarial)
**Bundle:** SPEC-19 §3.3 (item 2.26-C) — Stateful Worker Lifecycle + delta
  BSP loop (R20-R30)
**Predecessors consulted:**
  SPEC-19 §3.1 R1-R7 (coordinator-free round optimization + the canonical
  activity/GNF split in `merge/grid.rs::check_global_normal_form`),
  SPEC-19 §3.2 R8-R19 (BorderGraph primitives — shipped),
  SPEC-19 §3.3 R20-R30 (this bundle),
  SPEC-19 §3.4 R31-R37 (wire variants — bundle 2.26-A, concurrent),
  SPEC-19 §3.5 R38-R40 (invariant amendments — bundle 2.26-D),
  SPEC-19 §3.6 R41-R44 (config amendments — bundle 2.26-D),
  SPEC-13 R6-R8 (layer dependency direction: `protocol/` → `merge/`
  allowed; reverse illegal),
  SPEC-06 R5 (append-only discriminants, discriminants 7..=11 fresh).
**Source consulted:**
  `relativist-core/src/worker.rs` (v1 FSM: Idle → Reducing → Returning
  via `ReceivePartition` / `ReductionComplete` — fire-and-forget w.r.t.
  coordinator),
  `relativist-core/src/merge/grid.rs` L15-35 (canonical
  `check_global_normal_form` pair: BOTH `has_border_activity == false`
  AND `local_redexes == 0` required; L168-176 shows the branch uses
  `&&`), L45-68 (pure-core `run_grid` — no tokio, takes
  `&dyn PartitionStrategy`),
  `relativist-core/src/merge/types.rs` L129, L149 (`WorkerRoundStats`
  with `local_redexes: usize` AND `has_border_activity: bool` —
  distinct, not derivable from each other), L179
  (`GridConfig.strict_bsp: bool`), L200 (`coordinator_free_rounds: bool`),
  `relativist-core/src/merge/border_graph.rs` L59-63 (`DISCONNECTED`
  sentinel import from `crate::net::DISCONNECTED`), L125-131
  (`BorderDelta.new_target: PortRef` — struct shape already shipped and
  matching R33 literal), L117-124 (DISCONNECTED-in-BorderDelta semantic
  already authoritative),
  `relativist-core/src/net/types.rs` L228
  (`pub const DISCONNECTED: PortRef = PortRef::FreePort(u32::MAX);`),
  `relativist-core/src/net/types.rs` L77-145 + L509
  (SPEC-18 §4.3 compact encoding — DISCONNECTED collapses to 1 wire
  byte via `PORTREF_TAG_DISCONNECTED = 0xFF`),
  `docs/backlog/TASK-0379.md` .. `TASK-0388.md`,
  `docs/backlog/SPEC-19-section-3.3-worker-lifecycle-tasks.md`
  (bundle index).
**Precedent consulted:**
  `docs/spec-reviews/SPEC-19-section-3.2-design-choices-2026-04-17.md`
  DC-1 (DISCONNECTED sentinel ruling — the same principle governs DC-C6
  here) and DC-4 (graph-enforced invariants at primitive boundary),
  `docs/spec-reviews/SPEC-19-section-3.4-design-choices-2026-04-17.md`
  DC-A1 (SPEC-13 layering forces pure-core residency; same principle
  governs DC-C2).

---

## Overall Assessment

The SPLITTING bundle is thorough. Four of six design choices resolve
cleanly to the task-splitter's defaults. **Two choices require
correction** — DC-C3 (which contradicts SPEC-19 R40's explicit
definition of BOTH strict AND lenient delta modes) and DC-C5 (which
contradicts SPEC-19 R40's explicit termination condition "all workers
report zero local redexes AND the BorderGraph contains zero active
pairs" and also contradicts the v1 `check_global_normal_form`
precedent at `merge/grid.rs` L31-34). These are the only blockers.

**Verdict:** APPROVED WITH AMENDMENTS to **task files only** (no spec
edits). Stage 2 TESTS unblocked once TASK-0385 and TASK-0386 are
updated per DC-C3 and DC-C5 rulings. DC-C1, DC-C2, DC-C4, and DC-C6
stand at the task-splitter's default.

---

## Verdicts (6 design choices)

### DC-C1: Round 0 → Round 1 transition signal

**PICK:** **Option B** — arrival-triggered transition; no new
`Message` variant; the worker's first `Message::RoundResult` (round=1)
serves as the implicit "partition stored and round 1 complete"
acknowledgement.

**WHY:**

1. **R21.1 literal.** "The worker stores this partition in its local
   state." The requirement is a storage obligation, not a protocol
   obligation to confirm storage. The spec does not say "and
   acknowledges storage" or "and emits a confirmation". Adversarial
   principle: when the spec omits an ack, the default is fire-and-
   forget; forcing an ack requires positive evidence in the text.

2. **v1 precedent (canonical).** The v1 protocol is already fire-and-
   forget between `AssignPartition` and the first round's
   `PartitionResult`. `worker.rs` L108-117 shows the FSM transitions
   `Idle → Reducing` on `ReceivePartition(partition)` without sending
   any message back to the coordinator; the coordinator learns the
   partition was received ONLY when it receives `PartitionResult`
   (worker.rs L123). R21.1/R21.2 mirror this exactly: `InitialPartition`
   is the analogue of `AssignPartition`; the first `RoundResult` is the
   analogue of `PartitionResult`. SPEC-19 §3.3 was clearly designed to
   preserve this pattern.

3. **Option A (`RoundStartAck`) is a NEW wire variant.** Adding a 6th
   coordinator-bound variant to sub-bundle 2.26-A mid-flight would
   require amending `Message` to include discriminant 12, re-running
   2.26-A's TASK-0371 byte-level stability test, and expanding
   TASK-0368/0369 serde round-trip coverage. This is a cross-bundle
   cascade for zero benefit: the coordinator can already detect a
   failed Round 0 via TCP FIN / timeout / malformed-frame errors
   (SPEC-06 R18-R19 error paths exist and ship in v1).

4. **Option C (empty `RoundResult` as ack) requires a SPEC-19 §3.3
   amendment** because R26 does not enumerate "empty-round
   acknowledgement semantics" among the `RoundResult` shapes. Adding
   such semantics is more expensive than Option A and buys nothing
   over Option B.

**AMENDMENT NEEDED?** No spec amendment. **TASK-0380 / TASK-0385
doc-comment hardening recommended** — pin the fire-and-forget
contract explicitly so a future maintainer does not reintroduce an
ack.

**TASK IMPACT:**

- **TASK-0380 "Context"** — add one sentence: "Per DC-C1 of
  `docs/spec-reviews/SPEC-19-section-3.3-2.26C-design-choices-2026-04-17.md`:
  Round 0 is fire-and-forget. The worker stores the partition and
  transitions to round-1 state on arrival of the first
  `Message::RoundStart`. No ack message is sent after
  `InitialPartition` — the coordinator learns storage succeeded only
  when the worker's first `RoundResult` (round=1) arrives. This
  mirrors the v1 `AssignPartition` → `PartitionResult` contract."

- **TASK-0385 "Context"** — analogous sentence on the coordinator
  side: "Per DC-C1: after dispatching `InitialPartition` to every
  worker, the coordinator does NOT block on an ack. It immediately
  enters the round-1 dispatch loop (`RoundStart` to every worker),
  then blocks on `RoundResult` collection. A worker whose
  `InitialPartition` was dropped will be detected via TCP
  disconnection or Round 1 timeout (SPEC-06 R18-R19)."

---

### DC-C2: Wire plumbing for `run_grid_delta`

**PICK:** **Option C** — pure-core `run_grid_delta` in `merge/grid.rs`
takes a `&dyn WorkerDispatch` trait; the async/tokio implementation
of the trait lives in `coordinator.rs` (binary-layer); tests use a
synchronous `LocalDeltaDispatch` mock.

**WHY:** This decision is forced by SPEC-13 R6-R8 (layer dependency
direction), not by ergonomics:

1. **SPEC-13 R6-R8 (inviolable).** `merge/` is pure-core.
   `protocol/` / `coordinator.rs` / `worker.rs` are the async/tokio
   layer. The allowed edge is `protocol/` → `merge/`. The reverse
   edge is illegal. `run_grid_delta` is the delta-mode analogue of
   `run_grid` — it orchestrates the BSP loop (split already happened;
   per-round dispatch to workers + collection + graph update + GNF
   check). It MUST live in `merge/` for parity with `run_grid`
   (`merge/grid.rs::run_grid` is already pure-core — `merge/grid.rs`
   L45-68 confirms no tokio import).

2. **Option A (async plumbing shared with v1 coordinator FSM in
   `protocol/coordinator.rs`) would pull tokio into `merge/grid.rs`.**
   This inverts SPEC-13's forbidden edge direction. Decisively
   rejected — same argument as §3.4 DC-A1's `BorderDelta` placement
   ruling.

3. **Option B (everything in `coordinator.rs`) is feasible but
   violates the established v1 pattern.** v1 runs the BSP loop in
   `merge/grid.rs::run_grid` and the async wire FSM in
   `protocol/coordinator.rs`; the two are cleanly separated.
   Duplicating this pattern for delta mode means `run_grid_delta`
   has no local-simulation counterpart — developers cannot exercise
   the delta BSP loop in unit tests without tokio and real TCP.
   This breaks the "pure-core is in-process testable" invariant that
   SPEC-13 was designed to enforce.

4. **Precedent.** `PartitionStrategy` is the exact same pattern
   (`merge/grid.rs::run_grid` takes `&dyn PartitionStrategy` as a
   pure-trait abstraction; the strategy implementations are synchronous
   and pure-core). Applying the same pattern to `WorkerDispatch` is
   architecturally consistent.

5. **Pure-core helpers already assume Option C.** TASK-0381 /
   TASK-0382 factor out `apply_border_deltas_to_partition` and
   `compute_outgoing_deltas` as pure-core functions. These are called
   from `worker.rs` (async) AND from `LocalDeltaDispatch` mock
   (synchronous). Option C is the only shape where both call sites
   compile without tokio leaking into pure-core helpers.

**AMENDMENT NEEDED?** No spec amendment. No TASK-0384 amendment —
TASK-0384 already documents Option C as the default. Spec-critic
ratifies.

**TASK IMPACT:**

- **TASK-0384 acceptance criteria** — reinforce the `WorkerDispatch`
  trait signature pins: the trait methods must be **synchronous**
  (the async binding is the `impl WorkerDispatch for CoordinatorConnection`
  block in `coordinator.rs`, OUTSIDE this bundle; inside the bundle
  only the trait declaration and the `LocalDeltaDispatch` mock).

- **TASK-0385 "Files FORBIDDEN" list** — explicitly add
  "`relativist-core/src/protocol/coordinator.rs` async
  `impl WorkerDispatch` body (deferred to 2.26-D or a follow-up
  wire bundle)". Current task already treats `coordinator.rs` as
  READ-ONLY; pin the reason in the note.

---

### DC-C3: `strict_bsp` × `delta_mode` interaction

**PICK:** **Option C** — fully orthogonal. `delta_mode` with
`strict_bsp = true` AND `delta_mode` with `strict_bsp = false` are
BOTH defined semantics per SPEC-19 R40. No hard assert; no silent
override.

**WHY:** This reverses the task-splitter's default, but the reversal
is forced by the spec text:

1. **SPEC-19 R40 (line 238-247) defines BOTH modes explicitly:**
   ```
   - Delta mode, lenient behavior: The coordinator resolves border
     redexes immediately upon detection (R13). [...] R_delta_lenient
     = 1 in the absence of cross-partition cascades.
   - Delta mode, strict behavior: Border redexes are dispatched to
     workers for resolution in subsequent rounds [...]. R_delta_strict
     ≤ N.
   ```
   These are NOT alternative descriptions of the same behavior — they
   are TWO distinct semantics that depend on `strict_bsp`. R40 makes
   `delta_mode × strict_bsp` a four-cell matrix where all four cells
   are defined:
   | delta_mode | strict_bsp | semantics |
   |:----------:|:----------:|-----------|
   | false | false | v1 lenient (SPEC-05 R30a) |
   | false | true | v1 strict (SPEC-05 R30a) |
   | true | false | **delta lenient** (R40 line 242-243) |
   | true | true | **delta strict** (R40 line 244) |

2. **The task-splitter's default (Option A, hard-assert
   `delta_mode ⇒ strict_bsp`) DELETES row 3 of this matrix.**
   Implementing `assert!(config.delta_mode implies config.strict_bsp)`
   at `run_grid_delta` entry would panic on a configuration SPEC-19
   R40 explicitly permits. This is a **spec violation**.

3. **Option B (soft warn, silent fallback) also deviates from R40
   — it elides the documented different semantics.**

4. **Option C consequences for the bundle:** TASK-0385 must branch
   on `config.strict_bsp` inside the round loop:
   - `strict_bsp = true`: the coordinator resolves border redexes
     BUT defers their dispatch — they are sent to workers as
     `RoundStart.border_deltas` in round k+1, not resolved inline
     this round. This matches v1 strict mode semantics.
   - `strict_bsp = false`: the coordinator resolves border redexes
     inline via `BorderResolver::resolve` (2.26-B) and includes the
     freshly-resolved deltas in THIS round's `RoundStart`. Result:
     in the absence of cross-partition cascades, 1 delta round
     suffices — matching `R_delta_lenient = 1`.

5. **v1 precedent.** `merge/grid.rs::run_grid` already handles both
   `strict_bsp` values WITHOUT asserting. Delta mode has no grounds
   to be more restrictive than v1 mode.

**AMENDMENT NEEDED?** No spec amendment (R40 already defines both
cells). **TASK-0384 and TASK-0385 require amendment.**

**TASK IMPACT:**

- **TASK-0384 acceptance criteria** — REMOVE any language that
  asserts `config.strict_bsp == true`. `run_grid_delta` MUST accept
  both values. Replace with: "Per DC-C3 ruling
  (`docs/spec-reviews/SPEC-19-section-3.3-2.26C-design-choices-
  2026-04-17.md`): `run_grid_delta` MUST support both
  `strict_bsp = true` (deferred border dispatch — delta strict
  semantics per R40) and `strict_bsp = false` (inline border
  resolution — delta lenient semantics per R40). No `assert!` on
  the flag combination. No silent override."

- **TASK-0385 acceptance criteria** — add branch on
  `config.strict_bsp` inside the round loop:
  - **Delta lenient branch** (`strict_bsp = false`): after collecting
    `RoundResult`s and applying deltas to `BorderGraph`, IMMEDIATELY
    invoke `BorderResolver::resolve(border_graph.detect_border_redexes())`
    and merge the resolution outputs into THIS round's next-dispatch
    `RoundStart` payload. Convergence check runs after resolution.
  - **Delta strict branch** (`strict_bsp = true`): after collecting
    `RoundResult`s and applying deltas, run `BorderResolver::resolve`
    and STORE the resolution outputs; dispatch them as
    `RoundStart.border_deltas` in round k+1. Convergence check runs
    after apply-only (pre-resolution) on `BorderGraph::detect_border_redexes`
    for strict semantics.

- **TASK-0385 "Acceptance Criteria" test additions (Stage 2 TESTS
  to pin):**
  - `run_grid_delta_lenient_converges_in_one_round_absent_cascades`:
    a partition pair whose only inter-partition redex resolves in a
    single resolution cycle — both `strict_bsp = false` AND
    `delta_mode = true` → 1 delta round. Matches
    `R_delta_lenient = 1`.
  - `run_grid_delta_strict_multi_round_cascade`: same partition
    pair under `strict_bsp = true` + `delta_mode = true` → ≥ 2
    delta rounds (deferred dispatch).
  - `run_grid_delta_result_matches_run_grid_under_both_strict_modes`:
    final net from `run_grid_delta` is isomorphic to `run_grid`'s
    output under the SAME `strict_bsp` value (G1 preservation
    per SPEC-19 R38 amendment).

- **Bundle index "Design choices flagged for spec-critic"** — the
  current text of DC-C3 must be updated to cite SPEC-19 R40 and flip
  the picked option from (A) to (C). This is a bundle-index-level
  documentation fix; no source code touched.

- **Cross-bundle coupling (2.26-D):** `GridConfig.delta_mode` +
  `strict_bsp` validation in 2.26-D MUST NOT reject the
  `delta_mode = true, strict_bsp = false` combination. If 2.26-D's
  current splitting adds such rejection, flag for amendment.

---

### DC-C4: `previous_border_state` initial value

**PICK:** **Option B** — seed `previous_border_state` from
`partition.free_port_index` at the moment `InitialPartition` is
stored (round 0). Round 1's delta computation emits deltas only for
borders Round 1's local reduction actually moved.

**WHY:**

1. **Coordinator-side knowledge parity.** The coordinator's
   `BorderGraph` is initialized by `BorderGraph::from_partition_plan`
   (`merge/border_graph.rs` L171) from the SAME `PartitionPlan` that
   produced the workers' initial partitions. At Round 0, coordinator
   and worker therefore have identical knowledge of border endpoints.
   There is NO information the worker could emit in a hypothetical
   "Round 1 full re-dispatch" (Option A) that the coordinator doesn't
   already hold. Option A's bandwidth expenditure is pure waste.

2. **R25 literal.** "The worker MUST maintain a `previous_border_state:
   HashMap<u32, PortRef>` that records the last-reported endpoint for
   each border ID." The phrase "last-reported" is key — at round 0,
   the worker has NOT yet reported anything, but the coordinator has
   already SEEN the initial state via `InitialPartition.partition`.
   The "last-reported" state from the coordinator's perspective IS
   the `InitialPartition`'s `free_port_index`. Seeding Option B is
   therefore the semantic identity: "coordinator's most recent view
   of this border."

3. **Option A (empty seed) wastes bandwidth.** For a partition with
   B borders, Option A sends B redundant `BorderDelta`s in Round 1
   regardless of how few actually changed. For typical Relativist
   workloads (few tens of borders per worker, most stable within a
   round), Option A blows up the Round 1 payload by 1-2 orders of
   magnitude for zero information gain.

4. **Option C (seeded from R23's `RoundStart` in round 1) is
   self-referential** — the Round 1 `RoundStart` may itself carry
   deltas (new_borders from CON-DUP dispatched at coordinator
   pre-round-1), and those deltas should update `previous_border_state`
   AFTER arrival, not serve as the seed. Mixing "seed" and "apply"
   semantics at Round 1 creates a race the protocol doesn't need.

5. **TASK-0379's current code already implements Option B** (L119):
   `let previous_border_state = partition.free_port_index.clone();`.
   The spec-critic ratifies — no task amendment required.

**AMENDMENT NEEDED?** No spec amendment. No task amendment —
TASK-0379 already implements Option B.

**TASK IMPACT:**

- **TASK-0379 "Notes" section** — tighten the DC-C4 rationale from
  "may amend this task" to "ratified by spec-critic per DC-C4 of
  `docs/spec-reviews/SPEC-19-section-3.3-2.26C-design-choices-
  2026-04-17.md`; seeding from `free_port_index` is the semantic
  identity with the coordinator's `BorderGraph::from_partition_plan`
  initialization."

---

### DC-C5: Termination predicate

**PICK:** **Option B** — `has_border_activity == false` for all workers
AND `BorderGraph::detect_border_redexes().is_empty()` AND
`all stats.local_redexes == 0` for all workers. **Three predicates
required, not two.**

**WHY:** This reverses the task-splitter's default. The reversal is
forced by TWO independent spec references AND by v1 precedent:

1. **SPEC-19 R40 (line 245) is LITERAL:**
   > "Termination condition (Global Normal Form): The protocol
   > terminates when **all workers report zero local redexes AND the
   > BorderGraph contains zero active pairs** (R4)."

   The conjunction is explicit. Dropping the `local_redexes == 0`
   conjunct is a direct spec deviation.

2. **v1 precedent (inviolable).** `merge/grid.rs` L15-35 ships
   `check_global_normal_form` returning BOTH booleans:
   ```rust
   pub(crate) fn check_global_normal_form(stats: &[WorkerRoundStats])
       -> (bool, bool) {
       let all_no_border = stats.iter().all(|s| !s.has_border_activity);
       let all_no_redexes = stats.iter().all(|s| s.local_redexes == 0);
       (all_no_border, all_no_redexes)
   }
   ```
   And the GNF branch at L172-176 requires BOTH via `&&`. The
   delta-mode predicate's relationship to its v1 counterpart should
   be "delta mode replaces v1 mode, with the SAME predicate plus the
   `BorderGraph::detect_border_redexes().is_empty()` conjunct."
   Dropping `local_redexes == 0` for delta mode would silently
   diverge from v1's invariants for no stated reason.

3. **The bundle-index's argument is factually wrong.** The bundle
   index claims `stats.local_redexes` records "interactions that
   HAPPENED during the round, not remaining work" and therefore is
   "always ≥ 0" even on a net that ends in Normal Form. The v1 code
   at `merge/grid.rs` L133 confirms the field is populated as
   `reduction_stats.total_interactions as usize` — interactions
   performed. BUT the v1 GNF check USES this value anyway, reading
   it as a Normal-Form signal: `all_no_redexes = stats.iter().all(|s|
   s.local_redexes == 0)` means "every worker performed zero
   interactions this round". In v1, a worker that performed zero
   local interactions AND has no border activity cannot contribute
   further to reduction — this IS the Normal-Form signal. The same
   reasoning applies in delta mode: a worker that performed zero
   local interactions in round k AND has `has_border_activity=false`
   has reached its local fixed point. Dropping the check loses a
   legitimate signal.

4. **Edge case not covered by two-predicate version.** Consider a
   pathological round where `reduce_all` reaches a local fixed point
   immediately but `rebuild_free_port_index` reports
   `has_border_activity == false` because all live agents have their
   principal ports consumed. In v1, this is caught by
   `local_redexes == 0` AND `has_border_activity == false`. In
   delta mode under the bundle's two-predicate version, this is
   caught only by `has_border_activity == false`, which is fine IF
   and ONLY IF the implementation guarantees that
   `has_border_activity = false` implies `reduce_all` reached a
   fixed point. That implication is not proven — it is a folklore
   assumption. The three-predicate version does not rely on it.

5. **Cost of the third predicate is negligible.** One extra
   `stats.iter().all(|r| r.stats.local_redexes == 0)` call per
   round, O(W) where W = workers per grid (typically 2..100). Wall
   time: microseconds. Bug-safety gain: closes the folklore gap.

**AMENDMENT NEEDED?** No spec amendment (R40 already mandates three
predicates). **TASK-0386 REQUIRES amendment.**

**TASK IMPACT:**

- **TASK-0386 "Context"** — replace "DC-C5 (bundle index): the
  current draft uses a two-predicate version" with:
  > "Per DC-C5 ruling
  > (`docs/spec-reviews/SPEC-19-section-3.3-2.26C-design-choices-
  > 2026-04-17.md`): the predicate is **three-conjunct**, matching
  > SPEC-19 R40 literal ('all workers report zero local redexes
  > AND the BorderGraph contains zero active pairs') and v1
  > `check_global_normal_form`'s shape."

- **TASK-0386 Acceptance Criteria Semantics block** — replace:
  ```
  all_no_border_activity = results.iter().all(|r| !r.has_border_activity);
  graph_has_no_redexes = border_graph.detect_border_redexes().is_empty();
  return all_no_border_activity && graph_has_no_redexes;
  ```
  with:
  ```
  all_no_border_activity = results.iter().all(|r| !r.has_border_activity);
  all_no_local_redexes = results.iter().all(|r| r.stats.local_redexes == 0);
  graph_has_no_redexes = border_graph.detect_border_redexes().is_empty();
  return all_no_border_activity && all_no_local_redexes && graph_has_no_redexes;
  ```

- **TASK-0386 Unit tests** — the `check_delta_convergence_ignores_local_redexes`
  test (Acceptance Criteria line 70-73) INVERTS: rename to
  `check_delta_convergence_requires_no_local_redexes`, change
  expected value from `true` to `false`, doc-comment updated to
  cite R40 literal.

- **TASK-0386 Key Types / Signatures block** — rewrite doc-comment
  of `check_delta_convergence` from "The `local_redexes == 0` check
  from R4 is omitted per DC-C5" to "The predicate is three-conjunct
  per SPEC-19 R40 and the v1 `check_global_normal_form` precedent
  at `merge/grid.rs` L31-34. Delta mode adds
  `BorderGraph::detect_border_redexes().is_empty()` as a third
  conjunct on top of v1's two."

- **TASK-0386 test-count row** — unchanged count (+6 tests) but one
  assertion polarity flips. Stage 2 TESTS writes the flipped test.

---

### DC-C6: Disconnection encoding in `BorderDelta`

**PICK:** **Option C** — reuse the existing `DISCONNECTED` sentinel
(`PortRef::FreePort(u32::MAX)`). No `Option<PortRef>` wrapper;
no `disconnected: bool` field.

**WHY:** This is forced by both the spec text AND by the §3.2 DC-1
precedent:

1. **R33 is LITERAL** (SPEC-19 line 184-190):
   ```rust
   pub struct BorderDelta {
       pub border_id: u32,
       pub new_target: PortRef,
   }
   ```
   The field type is `PortRef`, not `Option<PortRef>`. Option A
   (change to `Option<PortRef>`) is a direct spec deviation that
   would require a SPEC-19 amendment.

2. **§3.2 DC-1 precedent.** The `BorderGraph` bundle's DC-1 ruling
   (`docs/spec-reviews/SPEC-19-section-3.2-design-choices-2026-04-17.md`)
   picked Option B (reuse `DISCONNECTED` sentinel) against candidate
   options A (new `BorderTarget` enum) and C (`Option<PortRef>`
   wrapper). Four reasons given in that ruling (spec line 38
   hard-codes `new_target: PortRef`, line 107 disconnect spelling,
   SPEC-18 §4.3 compact-encoding single-byte optimisation at
   `net/types.rs` line 228/509, existing `reduction/rules.rs`
   DISCONNECTED discipline). **All four reasons apply identically
   to 2.26-C's on-wire `BorderDelta`** — in fact §3.2's in-memory
   `BorderDelta` IS THE SAME STRUCT that 2.26-A will serialize over
   the wire (with added `serde::Serialize, serde::Deserialize`
   derives per §3.4 DC-A1 amendment). Divergence in disconnection
   encoding between in-memory and on-wire would force a conversion
   layer, break R33 literal, and duplicate the §3.2 DC-1 analysis.
   Decisively rejected.

3. **Bincode wire cost verification.** The task-splitter's flag
   asked whether `PortRef::FreePort(u32::MAX)` is safe on the wire
   versus `Option<PortRef>::None`. Under SPEC-18 §4.3 compact
   encoding (confirmed at `net/types.rs` L77-145, test
   `test_portref_compact_disconnected_is_one_byte` at L509):
   - `PortRef::FreePort(u32::MAX)` → 1 wire byte (tag
     `PORTREF_TAG_DISCONNECTED = 0xFF`).
   - `Option<PortRef>::Some(PortRef::AgentPort(...))` → 1 presence
     byte + encoded PortRef (≥ 2 bytes).
   - `Option<PortRef>::None` → 1 presence byte + no payload (1 byte).
     BUT every `Some` case pays an extra byte vs. Option C's raw
     PortRef.
   Delta payloads consist overwhelmingly of `Some` cases (reconnects
   dominate disconnects). Option A therefore costs one extra wire
   byte per reconnect delta for no information gain — a regression
   on the hottest delta shape.

4. **Option B (separate `disconnected: bool` field) is redundant.**
   The bool IS the sentinel: `new_target == DISCONNECTED` already
   answers "was this border disconnected?". Adding a bool doubles
   the source of truth and creates a drift vulnerability (structurally
   identical to §3.4 DC-A2's `has_border_activity` duplication
   concern). Decisively rejected.

5. **Cross-bundle coupling check.** `merge/border_graph.rs` L117-131
   already defines `BorderDelta` with `new_target: PortRef` and the
   DISCONNECTED semantics documented in the doc-comment. 2.26-A's
   TASK-0366 adds `serde::Serialize, serde::Deserialize` derives
   (per §3.4 DC-A1 amendment) without changing the struct shape.
   Option C therefore requires ZERO code changes in 2.26-C — the
   struct already exists in the shape Option C mandates. Options A
   and B would force a cross-bundle cascade into 2.26-A (struct shape
   change → TASK-0366 amendment → test-count re-shift).

**AMENDMENT NEEDED?** No spec amendment. No task amendment for
2.26-C (worker delta-emission code in TASK-0382 simply constructs
`BorderDelta { border_id, new_target: DISCONNECTED }` for erased
borders and `BorderDelta { border_id, new_target: PortRef::AgentPort(...) }`
for reconnects). **One doc-comment hardening recommended on
TASK-0382 to pin the convention.**

**TASK IMPACT:**

- **TASK-0382 "Context"** — add one paragraph:
  > "Per DC-C6 of
  > `docs/spec-reviews/SPEC-19-section-3.3-2.26C-design-choices-
  > 2026-04-17.md`: disconnected borders (freed by local reduction
  > erasing the agent connected to the border) are emitted as
  > `BorderDelta { border_id, new_target: DISCONNECTED }` where
  > `DISCONNECTED = PortRef::FreePort(u32::MAX)` (`crate::net::DISCONNECTED`).
  > This reuses the §3.2 DC-1 ruling's sentinel and relies on
  > SPEC-18 §4.3 compact encoding to collapse the disconnect wire
  > form to a single byte. No `Option<PortRef>` wrapper; no
  > `disconnected: bool` field on `BorderDelta`."

- **TASK-0382 Unit tests** — add one test:
  - `compute_outgoing_deltas_encodes_disconnect_as_sentinel`:
    construct a partition where local reduction erases an agent
    connected to a `FreePort(border_id)`; run
    `compute_outgoing_deltas`; assert the emitted `BorderDelta`
    has `new_target == crate::net::DISCONNECTED`.
  - `compute_outgoing_deltas_encodes_reconnect_as_agentport`:
    construct a partition where local reduction connects a new
    agent's principal port to a `FreePort(border_id)`; assert the
    emitted `BorderDelta.new_target` is a `PortRef::AgentPort(_, _)`
    matching the new endpoint.

- **Cross-bundle coupling flag for 2.26-A:** ratify §3.4 DC-A1's
  amendment (TASK-0366 adds `serde::Serialize, serde::Deserialize`
  on `BorderDelta`). 2.26-C's wire-facing code depends on this
  amendment landing; no additional cross-bundle amendment needed
  since §3.4 DC-A1 already flagged it.

---

## Summary table (compact)

| Design choice | Pick | Amendment needed? | Spec edit? |
|---------------|------|-------------------|-----------|
| DC-C1 Round-0 transition signal | **B** — arrival-triggered, no ack (v1 fire-and-forget precedent) | TASK-0380 + TASK-0385 doc-comment hardening only | No |
| DC-C2 Wire plumbing for `run_grid_delta` | **C** — pure-core + `WorkerDispatch` trait (SPEC-13 R6-R8 forces this) | TASK-0384 reinforce sync trait; TASK-0385 pin `coordinator.rs` as out of scope | No |
| DC-C3 `strict_bsp` × `delta_mode` | **C** — fully orthogonal per SPEC-19 R40 (BOTH cells defined) | TASK-0384 remove `strict_bsp` assert; TASK-0385 add strict/lenient branch + 3 tests; bundle-index flip A→C | No |
| DC-C4 `previous_border_state` seed | **B** — seed from `free_port_index` at storage (coordinator parity) | TASK-0379 "Notes" wording only | No |
| DC-C5 Termination predicate | **A → B (three-predicate)** — `has_border_activity=false` AND `local_redexes=0` AND graph empty (SPEC-19 R40 literal + v1 precedent) | TASK-0386 change predicate + invert one test + rewrite doc-comment | No |
| DC-C6 Disconnection encoding | **C** — reuse `DISCONNECTED` sentinel (§3.2 DC-1 precedent + R33 literal + SPEC-18 compact-encoding) | TASK-0382 doc-comment + 2 new tests (no struct change) | No |

## SPEC-19 amendments required before TESTS

**Count: 0.** No edits to `specs/SPEC-19-delta-protocol.md` are
required. All verdicts align with R20-R30, R33, R40 literal.

---

## Cross-bundle coupling concerns flagged

### 2.26-A (wire variants)
- **Confirmed no cascade.** DC-C1 Option B does NOT add a new
  `Message` variant, so 2.26-A's 5-variant shape (discriminants
  7..=11) stands.
- **Confirmed no cascade.** DC-C6 Option C reuses the sentinel that
  2.26-A's `BorderDelta` serde wiring already encodes via SPEC-18
  compact format; TASK-0366's `serde::Serialize, serde::Deserialize`
  amendment (per §3.4 DC-A1) covers 2.26-C's usage without further
  change.

### 2.26-B (coordinator resolver)
- **Confirmed no cascade.** DC-C2 Option C's `WorkerDispatch` trait
  lives in `merge/types.rs` (pure-core), consumed by 2.26-C's
  `run_grid_delta`. The `BorderResolver` API (2.26-B)
  is consumed inside `run_grid_delta` at TASK-0385 step 3f — the
  trait does NOT change `BorderResolver`'s signature.
- **DC-C3 flag for 2.26-B:** the `BorderResolver::resolve` API
  must be callable in BOTH lenient (inline) and strict (deferred)
  modes. If 2.26-B's current splitting tied the resolver to a single
  mode, re-open and verify. Low risk — the resolver's pseudocode
  in SPEC-19 §4.4 is mode-agnostic.

### 2.26-D (config + invariants)
- **Cascade:** `GridConfig.delta_mode` validation in 2.26-D MUST
  accept all four `(delta_mode, strict_bsp)` cells, including
  `delta_mode = true, strict_bsp = false` (delta lenient). If
  2.26-D's splitting currently adds a validator that rejects this
  combination, the validator must be relaxed per DC-C3 ruling.
  **ACTION:** spec-critic for 2.26-D should cross-reference this
  verdict before ruling on that bundle's config validation.

- **Cascade:** 2.26-D ships `GridMetrics` extensions (R45). The
  delta-mode convergence predicate (DC-C5 three-conjunct form)
  consumes `WorkerRoundStats.local_redexes` — this field is already
  shipped (v1, L129). No cascade.

- **Soft cascade:** `coordinator_free_rounds` flag (v1, already
  shipped) interacts with delta mode. SPEC-19 R43 says:
  "`coordinator_free_rounds` MUST default to `true` when `delta_mode`
  is `true`." Verify 2.26-D honours this default. Not a 2.26-C
  concern, but flagged here for completeness.

---

## Checklist

### Consistency
- [x] All terms match SPEC-00 / SPEC-13 / SPEC-19 / SPEC-06
      definitions.
- [x] Type signatures compatible with predecessor specs
      (`BorderDelta` shape matches R33 verbatim; `WorkerRoundStats`
      shape matches §3.1 amendment at `merge/types.rs` L129/L149).
- [x] No contradictions with predecessor requirements (DC-C2 aligns
      with SPEC-13 R6-R8; DC-C3 aligns with SPEC-19 R40's four-cell
      matrix; DC-C5 aligns with v1 `check_global_normal_form`;
      DC-C6 aligns with §3.2 DC-1 precedent).
- [x] Data flow assumptions match (Round 0 fire-and-forget mirrors
      v1 `AssignPartition`; `previous_border_state` seed mirrors
      `BorderGraph::from_partition_plan`).

### Testability
- [x] DC-C1 verifiable by grep: no new `Message::RoundStartAck`
      variant; TASK-0371 byte-stability test confirms 5-variant
      append.
- [x] DC-C2 verifiable by pure-core source-file scan on
      `merge/grid.rs` (no tokio import).
- [x] DC-C3 verifiable by TASK-0385's three added round-count
      tests (lenient 1-round, strict multi-round, G1 parity).
- [x] DC-C4 verifiable by TASK-0379's
      `workerdeltastate_from_initial_partition_seeds_previous_border_state`
      test (already specified in acceptance criteria).
- [x] DC-C5 verifiable by TASK-0386's three-conjunct predicate
      tests (including the inverted
      `check_delta_convergence_requires_no_local_redexes`).
- [x] DC-C6 verifiable by TASK-0382's two new sentinel-encoding
      tests and grep for absence of `Option<PortRef>` / `disconnected:
      bool` in `BorderDelta` definition site (unchanged from §3.2).

### Completeness
- [x] All six flagged choices have a verdict.
- [x] All cascading TASK impacts documented (TASK-0379 wording;
      TASK-0380 wording; TASK-0382 wording + 2 tests; TASK-0384
      remove assert; TASK-0385 strict/lenient branch + 3 tests;
      TASK-0386 predicate + test inversion + doc-comment).
- [x] All cross-bundle cascades flagged (2.26-A: none; 2.26-B:
      resolver mode-agnosticism verification; 2.26-D: config
      validator must permit all four matrix cells).
- [x] No undefined terms in the verdict.

### Invariant Preservation
- [x] T1-T7 unaffected (delta protocol is orchestration; local
      reduction is still `reduce_all`, preserving T1-T7).
- [x] D1-D2, D4-D5 unaffected (partitioning semantics unchanged).
- [x] D3 (Border Completeness) preserved — DC-C5's three-predicate
      termination predicate STRENGTHENS D3 relative to the
      two-predicate draft.
- [x] D6 (Protocol Termination) preserved — DC-C3 Option C honours
      R40's four-cell matrix; `R_delta_lenient = 1`,
      `R_delta_strict ≤ N` both ship.
- [x] I1-I5 unaffected (isomorphism invariants; no net structural
      changes).
- [x] G1 preserved — DC-C3 ensures `run_grid_delta` matches
      `run_grid`'s output under the SAME `strict_bsp` value
      (test added to TASK-0385).
- [x] R18 complexity bounds preserved — DC-C4 Option B avoids
      a redundant O(B) Round 1 dispatch.
- [x] R19 pure-core invariant preserved — DC-C2 Option C keeps
      `run_grid_delta` and its helpers in `merge/` pure-core;
      async layer lives outside this bundle.
- [x] SPEC-13 R6-R8 preserved — DC-C2 Option C is the only option
      that does not invert the forbidden `protocol/ ← merge/` edge.

---

## Stage 1.5 verdict

**Stage 1.5 spec-critic complete.** TESTS stage is **unblocked** in
principle, but the task-updater MUST apply the amendments listed
under DC-C3 (TASK-0384 remove assert; TASK-0385 add strict/lenient
branch + 3 tests + bundle-index flip A→C) and DC-C5 (TASK-0386
three-predicate change + test inversion + doc-comment rewrite)
before test-generator writes TEST-SPEC-0379..0388 — otherwise the
test contracts will encode the WRONG semantics for `strict_bsp`
handling and for GNF termination.

Amendments under DC-C1 (wording), DC-C2 (wording), DC-C4 (wording
only — code already correct), and DC-C6 (wording + 2 tests) are
non-blocking for Stage 2 TESTS but SHOULD be applied in the same
task-updater pass to avoid Stage 3 DEV scope creep.

Per CLAUDE.md rule and local convention that only task-splitter /
task-updater edits `docs/backlog/`, spec-critic does NOT edit those
task files directly. The orchestrator should dispatch task-updater
to apply the amendments, then dispatch test-generator for Stage 2
TESTS.

### Additional observations (not DC verdicts but flagged here)

1. **R40's "four-cell matrix" is under-specified.** R40 defines
   `R_delta_lenient = 1` (absence of cascades) and `R_delta_strict
   ≤ N`, but does NOT specify the round-count upper bound under
   delta lenient WITH cross-partition cascades. The implicit bound
   is "cascade depth + 1", which for Relativist's typical nets is
   small but unproven. If 2.26-D's splitting adds a round-count
   metric for cascade-depth observability, this is a natural
   counterpart. Not blocking.

2. **DC-C3's "deferred dispatch" semantics for strict mode (round
   k+1 RoundStart carries round-k resolutions) is not pinned in
   SPEC-19 §3.3 text.** R40 line 244 says "Border redexes are
   dispatched to workers for resolution in subsequent rounds" but
   does not fix WHICH subsequent round. One-round defer is the
   natural choice (matches v1 strict mode); TASK-0385's amendment
   should pin this explicitly. If a future bundle wants to batch
   resolutions across multiple rounds, a SPEC-19 amendment is
   required first.

3. **The `WorkerDispatch` trait (DC-C2 Option C) has NO concrete
   async implementation in this bundle.** The bundle index correctly
   notes this ("real async implementation lives OUTSIDE this
   bundle"). Stage 6 REFACTOR / follow-up bundle must land the
   `impl WorkerDispatch for CoordinatorConnection` block in
   `coordinator.rs` before 2.26-C is production-ready. Not a
   Stage 1.5 blocker but a known deferred-work item.

Observations 1, 2, 3 are non-blocking for Stage 2 TESTS (all three
addressable in follow-up bundles) but task-updater MAY reference
them in TASK-0385 / TASK-0388 notes for developer awareness.
