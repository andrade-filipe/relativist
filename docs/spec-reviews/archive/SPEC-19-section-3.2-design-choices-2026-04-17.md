# SPEC-19 §3.2 — Design Choices Verdict (Stage 1.5 spec-critic)

**Date:** 2026-04-17
**Reviewer:** spec-critic (adversarial)
**Bundle:** SPEC-19 §3.2 (item 2.35) — BorderGraph data structure (R8-R19)
**Predecessors consulted:** SPEC-19 §3.2 R8-R19 (data-structure scope),
  SPEC-19 §3.3 R20-R36 (delta-protocol scope, out of this bundle),
  SPEC-19 §4.1 (type sketch), §4.2 (BorderGraph pseudocode),
  §4.5 (`compute_border_deltas`).
**Source consulted:**
  `relativist-core/src/net/types.rs` (PortRef + DISCONNECTED const + compact
  serde encoding, TASK-0344),
  `relativist-core/src/partition/types.rs` (Partition, PartitionPlan,
  WorkerId, `free_port_index: HashMap<u32, PortRef>`),
  `relativist-core/src/merge/helpers.rs` (existing `is_principal_pair`),
  `relativist-core/src/merge/core.rs` (existing border-redex call site),
  `relativist-core/src/merge/mod.rs` (module wiring precedent),
  `docs/backlog/TASK-0360.md` .. `TASK-0365.md`,
  `docs/backlog/SPEC-19-section-3.2-border-graph-tasks.md` (bundle index).
**Precedent consulted:**
  `docs/spec-reviews/SPEC-18-section-3.5-design-choices-2026-04-16.md`
  (format + verdict style).

---

## Overall Assessment

The SPLITTING bundle is sound. All four design choices resolve cleanly
against the existing spec text and the existing codebase **with zero
blocking spec amendments required**. In two cases (DC-1, DC-3) the spec
itself is unambiguous once the full text is read — the task-splitter's
flagging was conservative; the decision reduces to "follow spec and
existing code precedent". In one case (DC-4) the spec admits two equally
valid shapes and I pick the safer one (option B — graph recomputes
`is_redex`) against the task-splitter's recommendation (A), on
invariant-preservation grounds. DC-2 is the only genuine trade-off; I
side with the task-splitter (ship `worker_borders` now) but mandate the
doc-comment wording that locks it to item 2.26's future consumer so a
future reviewer cannot prune it as dead code.

**Verdict:** APPROVED WITH AMENDMENTS to **task files only** (no spec
edits). Stage 2 TESTS unblocked once TASK-0364's acceptance criteria
flip from "caller pre-computes `is_redex`" (A) to "graph recomputes
`is_redex` via `is_principal_pair`" (B); TASK-0362's `BorderDelta`
module placement gets a wording amendment; TASK-0361 and TASK-0363 stand
as written.

---

## Verdicts (4 design choices)

### DC-1: `DISCONNECTED` sentinel form (R17)

**PICK:** **Option B** — reuse the existing
`PortRef::FreePort(u32::MAX)` sentinel, imported as
`crate::net::DISCONNECTED`. **No new `BorderTarget` enum. No
`Option<PortRef>` wrapper.**

**WHY:** The spec text at SPEC-19 line 38 defines a border delta as
"`(border_id: u32, new_target: PortRef)`" — the wire-level type is
fixed as `PortRef`, not a fresh enum. Line 107 (R17) writes
"`(border_id, DISCONNECTED)` **or** `(border_id, None)`" — the "or" is
a notational convenience (spec prose hedging between two *ways of
spelling* the same thing), not a mandate for two distinct types. The
spec §4.1 comment at lines 346-350 resolves the ambiguity explicitly:
```
/// `PortRef::FreePort(u32::MAX)` (DISCONNECTED) indicates the
/// agent connected to this border has been erased.
```
and §4.5 pseudocode (`compute_border_deltas`, line 736) constructs
disconnects via `DISCONNECTED` by name. The spec therefore **already
picks Option B**; the task-splitter's flagging was conservative.

The existing code confirms the pick is load-bearing:

1. `net/types.rs` line 228 defines
   `pub const DISCONNECTED: PortRef = PortRef::FreePort(u32::MAX);`
   — the canonical name is already exported from the public API
   (`net::mod.rs` line 11: `pub use types::*;`).
2. `net/types.rs` lines 77-145 (TASK-0344, the SPEC-18 §4.3 compact
   encoding) guarantee that `DISCONNECTED` collapses to **1 wire byte**
   (`PORTREF_TAG_DISCONNECTED = 0xFF`). Test
   `test_portref_compact_disconnected_is_one_byte` (line 509) locks
   this. Under Options A or C, the wire form for a disconnect would
   become a 2-byte tagged enum (`BorderTarget::Disconnected` or
   `Option::None`), burning the TASK-0344 optimisation on the single
   hottest delta case.
3. `reduction/rules.rs` and the entire port-writing discipline already
   use `DISCONNECTED` as the "no-connection" sentinel. A BorderGraph
   that introduces its own disconnect type would diverge from the
   codebase's dominant idiom for zero-connection semantics.
4. R33 (the `BorderDelta` wire type, SPEC-19 line 184-190) hard-codes
   `new_target: PortRef`. R33 is out of scope for THIS bundle (ships
   under item 2.26), but the type we pick for TASK-0362's in-memory
   `BorderDelta` MUST match R33 verbatim to avoid a conversion layer
   when item 2.26 lands wire encoding. Option B matches R33; Options A
   and C do not.

Option A (`BorderTarget` enum) would introduce a new type with no
current consumer and force a `match` at every `apply_deltas` call site
— pure ceremony. Option C (`Option<PortRef>`) would break R33's
hard-coded `new_target: PortRef` and double the disconnect wire cost.

Adversarial principle: when the spec is unambiguous once fully read,
and the existing codebase already ships the picked form, there is no
design choice — only a "don't break it" mandate.

**AMENDMENT NEEDED?** No spec amendment. No task amendment for
TASK-0362 (already imports `DISCONNECTED` from `crate::net`, already
uses `PortRef` as `new_target`). TASK-0360's `//!` doc block (lines
85-98 of that file) must NOT introduce a `BorderTarget` type; verify
the acceptance criteria do not mention one (they don't — only
`crate::net::{PortRef, DISCONNECTED}` is imported). No action required.

**TASK IMPACT:** None. All 6 tasks already assume Option B.

---

### DC-2: `worker_borders` field inclusion

**PICK:** **Option A** — ship `worker_borders: HashMap<WorkerId,
HashSet<u32>>` now, per spec §4.1 sketch. **But mandate a structural
change: replace the `Vec<Vec<u32>>` in TASK-0360 with the
`HashMap<WorkerId, HashSet<u32>>` shape from SPEC-19 line 377.**

**WHY:** The spec §4.1 type sketch (SPEC-19 lines 369-383) is
explicit:
```rust
pub struct BorderGraph {
    pub borders: HashMap<u32, BorderState>,
    pub worker_borders: Vec<Vec<u32>>,   // line 379 — THE AMBIGUITY
    pub active_redexes: HashSet<u32>,
}
```
Note that the spec actually writes `Vec<Vec<u32>>` at line 379 (indexed
by worker_id as usize), NOT `HashMap<WorkerId, HashSet<u32>>`. The
bundle's DC-2 description quotes the `HashMap` form; that is the
**briefing's** quote of what the spec *should* say, not what it *does*
say. After re-reading both:

- **The literal spec (line 379) writes `Vec<Vec<u32>>`.**
- TASK-0360 acceptance criterion line 46 matches literally:
  `worker_borders: Vec<Vec<u32>>`.
- TASK-0361 line 132 seeds it with
  `vec![Vec::new(); plan.partitions.len()]` and indexes by
  `worker_id as usize`.

The `Vec<Vec<u32>>` shape is correct given that `WorkerId = u32` and
worker IDs in a `PartitionPlan` are dense (0..num_workers-1, SPEC-04).
A `HashMap` would add per-lookup hashing overhead for no benefit. The
briefing's `HashMap<WorkerId, HashSet<u32>>` is a mistranscription
that I do NOT propagate to the verdict.

**On shipping vs deferring:**

**FOR shipping now (option A):**
- Spec §4.1 declares the field as part of the struct shape. Deferring
  it means the struct is *not the shape the spec specifies* — a
  deliberate spec deviation that would require amending §4.1.
- TASK-0361's constructor populates it in O(B) during the same pass
  that builds `borders`. Adding it later means ANOTHER O(B) pass over
  `plan.partitions` in item 2.26's bundle, doubling the init cost.
- TASK-0364's `add_border_states` already updates it when new borders
  are added from CON-DUP expansion. Item 2.26 wants the field populated
  from the moment the first border is added, not re-seeded mid-lifecycle.
- Memory cost is negligible: for a 100-worker grid with 10k borders per
  worker-pair, `worker_borders` is `100 * (10k * 4 bytes) = ~4 MB`
  total on the coordinator — noise compared to the 256 MiB frame cap.

**AGAINST shipping now (option B — defer):**
- No method in this bundle READS `worker_borders`. The field is idle
  code from a YAGNI perspective.
- `apply_deltas` (TASK-0362) uses `state.worker_a / worker_b` for
  ownership lookup, not `worker_borders`. The field is only useful to
  item 2.26's "dispatch `RoundStart` only to workers whose borders
  changed" optimisation, which is conjectural.

**Verdict rationale (A wins):**

The spec mandates the field's presence in the struct shape. Deferring
requires spec amendment. Shipping now costs ~15 LoC of constructor
seeding (TASK-0361) and ~8 LoC of add_border_states updating
(TASK-0364) — negligible. The dead-code concern is addressed by:

1. `pub(crate)` visibility (already in TASK-0360 line 136) — the field
   is NOT in the public API, so external users cannot drift against it.
2. A **mandated doc-comment** on the field citing "consumed by item
   2.26 coordinator dispatch" + a direct pointer to the `RoundStart`
   message contract (SPEC-19 R23). This prevents a future refactor
   from pruning the field as unused.

**AMENDMENT NEEDED?** No spec amendment. **TASK-0360 amendment
required** (small — doc comment).

**TASK IMPACT:**
- **TASK-0360** "Key Types / Signatures" — on the `worker_borders`
  field, replace the one-line doc comment with:
  ```rust
  /// Per-worker reverse index of `border_id`s participating in each
  /// worker's partition. Populated by `from_partition_plan` (TASK-0361)
  /// and updated by `add_border_states` (TASK-0364). Read by the future
  /// item 2.26 coordinator dispatch (R23) to address `RoundStart`
  /// messages selectively. Entries become stale after `remove_border`
  /// / R17 double-disconnect (spec §4.2 note); stale entries are
  /// tolerated.
  pub(crate) worker_borders: Vec<Vec<u32>>,
  ```
- **TASK-0360** "Notes" — the final bullet ("Why ship `worker_borders`
  now") should cite this verdict's DC-2 rationale (spec mandate + O(B)
  co-seeding benefit) instead of the briefing's generic "spec §4.1
  includes it" line.
- **TASK-0361, TASK-0364** — no change. Their population / update logic
  already matches.

---

### DC-3: `detect_border_redexes` return shape (R12)

**PICK:** **Option A** — owned `Vec<(u32, BorderState)>` per spec R12
prose (line 93). **Override the task-splitter's §4.2 pseudocode
preference.**

**WHY:** The spec text has a literal contradiction between §3.2 (the
**normative** requirements section) and §4.2 (the **non-normative**
pseudocode section). Line-by-line:

- **§3.2 R12 (line 93 — normative):**
  `detect_border_redexes() -> Vec<(u32, BorderState)>`
  — **owned** `BorderState`.
- **§4.2 pseudocode (line 452 — non-normative):**
  ```rust
  fn detect_border_redexes(&self) -> Vec<(u32, &BorderState)> {
      self.active_redexes.iter()
          .filter_map(|bid| self.borders.get(bid).map(|s| (*bid, s)))
          .collect()
  }
  ```
  — **borrowed** `&BorderState`.

SPEC-00 (Relativist glossary, not quoted here but a project convention)
establishes that §3 "Requirements" sections are normative and §4
"Design" sections are descriptive pseudocode that MAY diverge from the
normative text at the implementer's judgment — so long as the
§3-level behaviour is preserved. **Normative text wins.**

The task-splitter picked the borrowed form (Option B) citing "zero-copy
fits pure-core ethos" and "coordinator can `.cloned()` where needed".
Both arguments are weak:

1. **Coordinator mutation pattern (item 2.26):** the delta BSP loop
   (SPEC-19 §4.3, line 500-514) does:
   ```
   let border_redexes = border_graph.detect_border_redexes();
   for (bid, state) in &border_redexes {
       let resolution = resolve_border_redex_at_coordinator(
           bid, state, &mut border_graph  // <- MUTABLE borrow
       );
       ...
   }
   ```
   If `detect_border_redexes` returns `Vec<(u32, &BorderState)>`, the
   returned references borrow `border_graph` **immutably** for the
   lifetime of the Vec — which makes the `&mut border_graph` argument
   to `resolve_border_redex_at_coordinator` inside the loop
   **impossible to compile**. The coordinator is forced to `.cloned()`
   every single state before the mutable call, which is exactly the
   work that Option B purports to save.
2. **`BorderState` is already `Clone`.** It's 6 fields totalling
   `4 + 8 + 8 + 4 + 4 + 1 = 29 bytes` (padded to probably 32). A
   per-round owned-clone cost for `|active_redexes|` entries is
   `32 × |active_redexes|` bytes — at item 2.26's worst case of 1k
   active borders per round, that's 32 KB per round per coordinator.
   Noise.
3. **§3.2 normative text is load-bearing for callers.** Test
   contracts in Stage 2 will assert against whatever signature this
   bundle ships. If we ship `&BorderState` and item 2.26 later needs
   owned (for the `&mut border_graph` reason above), the entire test
   surface must be rewritten to swap `.iter()` on `(&u32, &&
   BorderState)` for `.iter()` on `(&u32, &BorderState)`. Ship owned
   once, never revisit.

The §4.2 pseudocode was written as illustrative code, not as a
prescriptive API signature. It predates the detailed coordinator loop
in §4.3, which only works with owned states. The contradiction is a
spec-internal inconsistency; resolving it in favour of the normative
text is the only consistent option.

**AMENDMENT NEEDED?** No spec amendment (the §3.2 R12 text already
says owned; §4.2 pseudocode is non-normative). **TASK-0363 amendment
required.**

**TASK IMPACT:**
- **TASK-0363** "Acceptance Criteria" — first bullet: change
  `Vec<(u32, &BorderState)>` to `Vec<(u32, BorderState)>`.
  Remove the mention of "(borrowed form per §4.2 pseudocode)".
- **TASK-0363** "Context" — delete the paragraph justifying DC-3's
  Recommendation B; replace with a one-paragraph note that §3.2 R12
  is normative and §4.2 pseudocode is descriptive; the owned form
  matches the normative text.
- **TASK-0363** "Key Types / Signatures" — the body of
  `detect_border_redexes` changes from:
  ```rust
  self.active_redexes
      .iter()
      .filter_map(|&bid| self.borders.get(&bid).map(|state| (bid, state)))
      .collect()
  ```
  to:
  ```rust
  self.active_redexes
      .iter()
      .filter_map(|&bid| self.borders.get(&bid).map(|state| (bid, state.clone())))
      .collect()
  ```
  (one `.clone()` added per yielded state).
- **TASK-0363** "Notes" — delete the final bullet ("Per bundle DC-3
  (return type): if spec-critic prefers ..."); replace with a one-line
  citation: "Per DC-3 of `docs/spec-reviews/SPEC-19-section-3.2-design-choices-2026-04-17.md`:
  owned `BorderState` per §3.2 R12 normative text."
- **Bundle index** (`SPEC-19-section-3.2-border-graph-tasks.md`) —
  update the DC-3 recommendation line (currently "ship the borrowed
  form per §4.2 pseudocode") to record the spec-critic ruling.
  Wording: "Spec-critic DC-3 ruled: ship owned `Vec<(u32,
  BorderState)>` per §3.2 R12 normative text."

---

### DC-4: `add_border_states` signature (R15 part 3)

**PICK:** **Option B** — graph recomputes `is_redex` via
`is_principal_pair`. **Override the task-splitter's recommendation
(A).**

**WHY:** The spec R15 part 3 (line 102) prescribes the existence of
the primitive but not the exact signature. Both options correctly add
the states; the difference is *who* is responsible for the `is_redex`
derived bit:

- **Option A** (task-splitter's recommendation):
  `add_border_states(&mut self, states: Vec<BorderState>)` —
  caller pre-builds `BorderState.is_redex`.
- **Option B** (my verdict):
  `add_border_states(&mut self, entries: Vec<AddBorderEntry>)` where
  `AddBorderEntry { border_id, side_a, side_b, worker_a, worker_b }`
  — **graph always recomputes** `is_redex = is_principal_pair(side_a,
  side_b)`.

The invariant-preservation argument decides:

**Invariant (from R9):** `state.is_redex == is_principal_pair(state.side_a,
state.side_b)` — ALWAYS, at every observable moment.

Under Option A, a caller can pass `BorderState { side_a: principal,
side_b: principal, is_redex: false }` — the invariant is broken the
moment the struct enters the graph. `apply_deltas` will then *read*
the broken bit via `was_redex = state.is_redex` (TASK-0362 line 151)
and `active_redexes` will drift permanently from the true redex set
until a delta overwrites the corrupted state. The bug surface is
**every future coordinator call site** in item 2.26 — multiple sites
that must each remember to compute `is_redex` correctly.

Under Option B, the invariant is **graph-enforced**: the caller
cannot pass an inconsistent bit because the caller does not write the
bit at all. The primitive is **coupling-safe** — it has no way to be
misused.

The LoC argument (task-splitter's "symmetry with
`from_partition_plan`") is real but inverted: **`from_partition_plan`
itself computes `is_redex` via `is_principal_pair`** at TASK-0361
line 160 — it does NOT trust caller input. Option B matches
`from_partition_plan`'s pattern; Option A diverges from it.

The "caller knows best" argument (if the caller already has the
agents, they know whether it's a redex) also fails: the caller is the
item-2.26 coordinator after a resolution, and the only way to
correctly compute `is_redex` at that site is to call
`is_principal_pair(side_a, side_b)` anyway — exact duplication of the
graph's own check. Shipping Option B means the graph does it once,
correctly, in one place.

**Taxonomy note:** Option B requires a new public type
`AddBorderEntry` (5 fields: `border_id, side_a, side_b, worker_a,
worker_b`). This is cheap (~5 LoC for the struct + derives) and makes
the API signature self-documenting ("the caller provides the
connectivity; the graph provides the redex bit"). Alternative: a
5-tuple `(u32, PortRef, PortRef, WorkerId, WorkerId)` per the
task-splitter's Option (b) wording — I reject the tuple on readability
grounds; a named struct is one line more and pays back at every call
site.

**AMENDMENT NEEDED?** No spec amendment (R15 part 3 is shape-agnostic).
**TASK-0364 amendment required** (moderate — struct change +
signature change).

**TASK IMPACT:**
- **TASK-0364** "Context" — final paragraph ("Per bundle DC-4,
  `add_border_states` takes a pre-built `Vec<BorderState>`...") —
  REPLACE with:
  "Per DC-4 of `docs/spec-reviews/SPEC-19-section-3.2-design-choices-2026-04-17.md`,
  `add_border_states` takes a `Vec<AddBorderEntry>` (5 connectivity
  fields, NO `is_redex`). The graph recomputes `is_redex` via
  `is_principal_pair` to enforce the R9 invariant
  `state.is_redex == is_principal_pair(side_a, side_b)` at the
  primitive boundary."
- **TASK-0364** "Acceptance Criteria" — replace the bullet
  `pub fn add_border_states(&mut self, states: Vec<BorderState>)`
  with:
  ```
  - A new `pub struct AddBorderEntry` in `border_graph.rs`:
        pub border_id: u32,
        pub side_a: PortRef,
        pub side_b: PortRef,
        pub worker_a: WorkerId,
        pub worker_b: WorkerId,
    with derives `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`.
  - `BorderGraph` gets a new method:
    `pub fn add_border_states(&mut self, entries: Vec<AddBorderEntry>)`
    that for each entry:
      1. Computes `is_redex = is_principal_pair(entry.side_a,
         entry.side_b)`.
      2. Constructs `BorderState { border_id: entry.border_id,
         side_a: entry.side_a, side_b: entry.side_b,
         worker_a: entry.worker_a, worker_b: entry.worker_b, is_redex }`.
      3. Applies the insertion semantics documented in the original
         bullet (duplicate panic, out-of-bounds-worker panic,
         active_redexes insertion, worker_borders push).
  ```
- **TASK-0364** "Acceptance Criteria" test list — update each
  `add_border_states_*` test to construct `AddBorderEntry` inputs
  instead of `BorderState` inputs. The assertions on
  `borders.contains_key(...)`, `active_redexes.contains(...)`,
  `worker_borders[...]` are unchanged. Add one NEW test:
  `add_border_states_enforces_is_redex_invariant` — construct a
  principal-principal `AddBorderEntry`, pass it in, assert the stored
  `BorderState.is_redex == true`; then construct a
  principal-auxiliary entry, assert stored `is_redex == false`. This
  test would be **impossible** under Option A (the test would only
  check caller discipline).
- **TASK-0364** "Key Types / Signatures" code block — replace the
  entire `pub fn add_border_states` body with the Option B
  implementation (the two changes from the spec block above: accept
  `Vec<AddBorderEntry>`, derive `is_redex` via `is_principal_pair`).
- **TASK-0364** "Notes" — delete the final bullet ("Per bundle DC-4
  (signature shape): acceptance criteria assume signature (a)...");
  replace with one sentence: "Per DC-4 of the spec-critic verdict,
  shipping Option B (graph recomputes `is_redex`) to enforce the R9
  invariant at the primitive boundary."
- **Test count impact:** TASK-0364's inline test list grows from 10
  to 11. Acceptance criterion line 103 changes from "+10 (932→942)"
  to "+11 (932→943)". The `--features zero-copy` line changes from
  "+10 (972→982)" to "+11 (972→983)". Bundle index Stage 2 test
  estimate (~25 lib tests) grows to ~26 — below the "hard floor"
  margin; no further bundle-index change required.
- **Bundle index** (`SPEC-19-section-3.2-border-graph-tasks.md`) —
  update the DC-4 recommendation line (currently "ship signature
  (a)") to record the spec-critic ruling. Wording: "Spec-critic DC-4
  ruled: ship Option B with new `AddBorderEntry` input struct;
  graph recomputes `is_redex` to enforce R9."
- **TASK-0365** "Key Types / Signatures" `//!` doc block — bullet 6
  under "Coordinator-side lifecycle" (the `add_border_states`
  reference) should be reworded to: "For CON-DUP commutations that
  produce new border wires, calls [`BorderGraph::add_border_states`]
  with `Vec<AddBorderEntry>` inputs; the graph derives `is_redex`
  from side endpoints (R15 part 3 primitive; R9 invariant)." — this
  edit is trivial but prevents the doc block from describing a
  signature that no longer exists.

---

## Summary table (compact)

| Design choice | Pick | Amendment needed? | Spec edit? |
|---------------|------|---------------------|-----------|
| DC-1 `DISCONNECTED` sentinel form | Option B — `PortRef::FreePort(u32::MAX)` (`crate::net::DISCONNECTED`); spec §4.1 line 346-350 and existing TASK-0344 wire encoding already pick this | No | No |
| DC-2 `worker_borders` inclusion | Option A — ship now as `Vec<Vec<u32>>` (literal spec §4.1 line 379) with mandatory doc comment locking it to item 2.26 consumer | TASK-0360 (doc comment) | No |
| DC-3 `detect_border_redexes` return shape | Option A — owned `Vec<(u32, BorderState)>` per §3.2 R12 normative text (§4.2 pseudocode is non-normative and contradicts coordinator's mutable-borrow pattern) | TASK-0363 (signature + body + 1 clone) | No |
| DC-4 `add_border_states` signature | Option B — `Vec<AddBorderEntry>`, graph recomputes `is_redex` to enforce R9 invariant at the primitive boundary | TASK-0364 (new input struct + body + 1 new test) + TASK-0365 (doc bullet) | No |

## SPEC-19 amendments required before TESTS

**Count: 0.** No edits to `specs/SPEC-19-delta-protocol.md` are required.
The §4.2 pseudocode-vs-§3.2 contradiction in DC-3 is resolved in favour
of §3.2 (normative); the §4.2 pseudocode is left as-is (a documented
illustrative reference, not a prescriptive signature).

## Stage 1.5 verdict

**Stage 1.5 spec-critic complete.** TESTS stage is **unblocked** in
principle, but the task-splitter (or task-updater, per local agent
table) MUST apply the TASK-file amendments listed under DC-2
(TASK-0360), DC-3 (TASK-0363), DC-4 (TASK-0364 + TASK-0365), and the
bundle-index amendments (both DC-3 and DC-4 lines) before
test-generator writes TEST-SPEC-0360..0365 — otherwise the test
contracts will encode stale signatures. Per CLAUDE.md rule 7 (specs
editing) and the local convention that only task-splitter /
task-updater edits `docs/backlog/`, spec-critic does NOT edit those
task files directly. The orchestrator should dispatch task-updater to
apply the amendments, then dispatch test-generator for Stage 2 TESTS.

### Additional observations (not DC verdicts but flagged here)

1. **TASK-0360 test fixture bug:** lines 154-155 reference
   `AgentId::new(id)` and `AgentId::new(id), slot`, but `AgentId` is a
   **type alias** (`pub type AgentId = u32;` at `net/types.rs` line
   11), not a newtype with a `::new` constructor. The test helper
   should read `fn p(id: u32) -> PortRef { PortRef::AgentPort(id, 0) }`
   and `fn aux(id: u32, slot: u8) -> PortRef { PortRef::AgentPort(id,
   slot) }`. This is a **pre-existing bug** in TASK-0360, not a DC
   verdict — but if TASK-0360 is being amended anyway for DC-2, the
   task-splitter should fix this at the same time.
2. **TASK-0360 `is_principal_pair` duplication:** a function with the
   same name and signature already exists at
   `relativist-core/src/merge/helpers.rs` line 18
   (`pub(crate) fn is_principal_pair(a: PortRef, b: PortRef) -> bool
   { matches!((a, b), (PortRef::AgentPort(_, 0), PortRef::AgentPort(_,
   0))) }`). TASK-0360 would shadow it with a second definition in
   `border_graph.rs`. This is not a DC ruling but a **code-hygiene
   flag for the developer stage**: either re-export the existing
   helper from `border_graph.rs` (`use super::helpers::is_principal_pair;`)
   or move the helper into `border_graph.rs` and re-export it to
   `helpers` (if still needed by `merge/core.rs` line 156). I
   recommend the first option (reuse existing helper) — zero
   duplication, zero risk of drift. TASK-0360's acceptance criterion
   asking for a fresh definition should be amended to: "reuse the
   existing `pub(crate) fn is_principal_pair` from
   `crate::merge::helpers`; do NOT redefine it." The 7 inline tests
   move to `helpers.rs` if they add useful coverage, or drop if
   redundant with `helpers.rs`'s existing tests (lines 403-444).

Observations 1 and 2 are non-blocking for Stage 2 (both can be fixed
during Stage 3 DEV by the developer) but task-splitter / task-updater
SHOULD fix them now to avoid Stage 3 scope creep.

## Checklist

### Consistency
- [x] All terms match SPEC-00 / SPEC-01 / SPEC-19 definitions (no new
      terms introduced by this verdict).
- [x] Type signatures compatible with predecessor specs (`PortRef`,
      `WorkerId`, `BorderState` per SPEC-19 R9).
- [x] No contradictions with predecessor requirements (DC-1, DC-3
      align with existing code precedent; DC-2 aligns with SPEC-19
      §4.1 literal; DC-4 strengthens R9 enforcement).
- [x] Data flow assumptions match predecessor outputs
      (`from_partition_plan` produces `BorderGraph` with the
      invariants; `apply_deltas` / `add_border_states` preserve them).

### Testability
- [x] DC-1 verifiable by source-grep (no `BorderTarget` type
      introduced) + existing
      `test_portref_compact_disconnected_is_one_byte`.
- [x] DC-2 verifiable by source-grep on `BorderGraph` field shape
      (`worker_borders: Vec<Vec<u32>>`) + doc-comment presence check.
- [x] DC-3 verifiable by compile-time signature check (test calls
      `let vec: Vec<(u32, BorderState)> = graph.detect_border_redexes();`
      — type mismatch would fail to compile).
- [x] DC-4 verifiable by the new
      `add_border_states_enforces_is_redex_invariant` test mandated
      in the TASK-0364 amendment.

### Completeness
- [x] All four flagged choices have a verdict.
- [x] All cascading TASK impacts documented (TASK-0360 doc comment;
      TASK-0363 signature+body+note; TASK-0364 new struct + new test
      + body change; TASK-0365 doc bullet; bundle index DC-3 + DC-4
      lines).
- [x] No undefined terms in the verdict.

### Invariant Preservation
- [x] T1-T7 unaffected (this is coordinator-side metadata, not net
      structure).
- [x] D1-D2, D4-D5 unaffected (no partitioning semantics changed).
- [x] D3 (Border Completeness) **strengthened** by DC-4 Option B —
      the `is_redex` derived bit is now graph-enforced at the
      `add_border_states` boundary, not caller-trusted.
- [x] D6 unaffected (termination bound independent of this data
      structure).
- [x] I1-I5 unaffected (no interaction rules changed).
- [x] G1 unaffected (no reduction semantics changed; this bundle is
      pure-core data-structure scope).
- [x] R18 complexity bounds preserved — DC-3 owned return adds one
      `BorderState.clone()` per yielded entry, still O(|active_redexes|).
- [x] R19 pure-core invariant preserved — no task amendment
      introduces `async`, `tokio`, or `crate::protocol::*` imports.
