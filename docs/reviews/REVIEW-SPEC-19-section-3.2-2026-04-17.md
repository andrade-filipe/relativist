# REVIEW — SPEC-19 §3.2 (item 2.35) BorderGraph + Delta-Based Merge

**Date:** 2026-04-17
**Reviewer:** reviewer agent (unified code-quality + architecture)
**Bundle:** TASK-0360..TASK-0365 (6 tasks shipped as a single file, `border_graph.rs`)
**Stage:** 4 (REVIEW)
**Branch:** `v2-development` (uncommitted working tree)
**Test counts reported by DEV:** 905 → **955** lib default (+50);
  945 → **995** lib `--features zero-copy` (+50).
**Files reviewed:**
- `relativist-core/src/merge/border_graph.rs` (new; ~1309 LoC incl. tests)
- `relativist-core/src/merge/mod.rs` (re-export edit)
- `relativist-core/src/merge/helpers.rs` (existing `is_principal_pair`, referenced)
- `specs/SPEC-19-delta-protocol.md` §3.2 (R8-R19)
- `docs/spec-reviews/SPEC-19-section-3.2-design-choices-2026-04-17.md`
- `docs/tests/TEST-SPEC-0360.md` .. `TEST-SPEC-0365.md`
- `docs/backlog/TASK-0360.md` .. `TASK-0365.md` (post-amendment)
- `docs/pipeline-state.md` (DEV report)
- `docs/reviews/REVIEW-SPEC-19-section-3.1-2026-04-16.md` (precedent template)

---

## 1. Verdict

**APPROVE.** No MUST-FIX items. 0 SHOULD-FIX items. 3 NICE-TO-HAVE items (non-blocking, listed in §7). All of R8-R19 PASS. All four DC verdicts (DC-1..DC-4) are faithfully implemented. Stage 5 QA may proceed.

The bundle is a well-contained pure-core data-structure addition: ~440 LoC of production code (types + impl block), ~870 LoC of inline tests, one 11-character edit to `merge/mod.rs`. Every decision from the spec-critic verdict is implemented verbatim, the R9 redex-bit invariant is graph-enforced at every mutation boundary (via `is_principal_pair`), the R18 incremental-redex-set contract is honored on every path (`apply_deltas`, `remove_border`, `add_border_states`, and init seeding), and the R19 pure-core contract is guarded by a source-scan test that runs on every `cargo test`.

---

## 2. Requirements Compliance Matrix (R8-R19)

| Req | Verdict | Evidence (file: `border_graph.rs`) |
|-----|---------|------------------------------------|
| **R8** BorderGraph exists in `merge` module | **PASS** | Struct at L100-115; module wired in `merge/mod.rs` L6 (`pub mod border_graph;`) + L13 (`pub use border_graph::{AddBorderEntry, BorderDelta, BorderGraph, BorderState};`). |
| **R9** BorderState has 6 required fields (`border_id`, `side_a`, `side_b`, `worker_a`, `worker_b`, `is_redex`) | **PASS** | L73-86. Exact six-field order matches spec; derives `Debug, Clone, PartialEq, Eq`. Types match (`u32`, `PortRef`, `PortRef`, `WorkerId`, `WorkerId`, `bool`). |
| **R10** Initialize from `PartitionPlan`, exactly-2-sightings invariant | **PASS** | `from_partition_plan` L171-249. Walks `partition.free_port_index` (L180-184); validates C3 bidirectionally (L192-215); populates `borders` + `worker_borders` + `active_redexes` in one O(B) pass (L218-242). |
| **R11** `apply_deltas(worker_id, &[BorderDelta])` dispatches per-side and recomputes `is_redex` | **PASS** | L266-316. Ownership dispatch at L274-279; side update L283-288; `is_redex` recompute L302-303; incremental `active_redexes` update L305-314. Unknown border and wrong-worker paths silently skip (L269-271, L276-279). |
| **R12** `detect_border_redexes` returns `Vec<(u32, BorderState)>` (owned per DC-3) | **PASS** | L334-339. Owned `(u32, BorderState)` via `.clone()` on the state value; iterates `active_redexes` (O(\|active_redexes\|) per R18). |
| **R13** Border redex resolution at coordinator | **N/A** | Explicitly out of scope for this bundle (deferred to item 2.26). Module doc L50 records this. |
| **R14** Coordinator may use `interact_*` functions | **N/A** | Same as R13 — item 2.26 scope. |
| **R15** part 1-2 (port updates after resolution) | **N/A** | item 2.26 scope. |
| **R15** part 3 (add new borders from CON-DUP expansion) | **PASS** | `add_border_states` L398-438. Takes `Vec<AddBorderEntry>` (DC-4); graph recomputes `is_redex` via `is_principal_pair` L422; inserts into `borders`, `worker_borders[worker_a]`, `worker_borders[worker_b]`, and (conditionally) `active_redexes` L431-435. |
| **R16** Border removal on annihilation | **PASS** | `remove_border` L373-379. Returns `Option<BorderState>`; clears `active_redexes` if the removed border was a redex. `apply_deltas` L290-299 also removes via the R17 double-disconnect path. |
| **R17** Border erasure via DISCONNECTED sentinel | **PASS** | `apply_deltas` L290-299. Single-side DISCONNECTED leaves the border alive with `is_redex == false` (L302 `is_principal_pair(DISCONNECTED, _) → false`); double-DISCONNECTED removes the entry and scrubs `active_redexes`. DC-1 sentinel form (`crate::net::DISCONNECTED`) imported at L60, no `BorderTarget` enum anywhere. |
| **R18** Complexity: O(B) init, O(\|deltas\|) apply, O(\|redex_set\|) detect; incremental redex set maintained | **PASS** | See §6 complexity gate below. |
| **R19** Pure-core data structure — no `tokio`, `async`, or I/O | **PASS** | Imports (L58-63): `std::collections::{HashMap, HashSet}`, `crate::net::{PortRef, DISCONNECTED}`, `crate::partition::types::{PartitionPlan, WorkerId}`, `super::helpers::is_principal_pair`. Zero async, zero tokio, zero `crate::protocol`. Enforced by `UT-0365-01` source-scan test at L1511-1528. |

---

## 3. Design-Choice Verdict Compliance (DC-1..DC-4)

| DC | Spec-critic ruling | Implementation location | Verdict |
|----|--------------------|-------------------------|---------|
| **DC-1** DISCONNECTED = `PortRef::FreePort(u32::MAX)` via `crate::net::DISCONNECTED`; NO `BorderTarget` enum, NO `Option<PortRef>` | `BorderDelta.new_target: PortRef` L130; `DISCONNECTED` imported L60; `apply_deltas` compares against `DISCONNECTED` at L291; no `BorderTarget` or `Option<PortRef>` anywhere in the file | **PASS** |
| **DC-2** Ship `worker_borders: Vec<Vec<u32>>` now, with doc-comment locking it to item 2.26 consumer; `pub(crate)` visibility | Field declared `pub(crate)` at L111 with `#[allow(dead_code)]` L110; doc comment L104-108 cites R23 and the item 2.26 worker-dispatch path; seeded in `from_partition_plan` L219, L237-238; updated in `add_border_states` L432-433 | **PASS** |
| **DC-3** `detect_border_redexes` returns owned `Vec<(u32, BorderState)>` | L334 signature `pub fn detect_border_redexes(&self) -> Vec<(u32, BorderState)>`; L337 body uses `s.clone()` to produce owned values; UT-0363-03 at L1133-1152 exercises the `&mut graph` pattern inside the loop body (impossible under borrowed return) | **PASS** |
| **DC-4** `add_border_states` takes `Vec<AddBorderEntry>` (5 connectivity fields, NO `is_redex`); graph recomputes `is_redex` via `is_principal_pair` | `AddBorderEntry` at L142-154 (5 fields, no `is_redex`); `add_border_states` L398 signature; body at L422 computes `is_redex = is_principal_pair(entry.side_a, entry.side_b)`; stores derived bit into `BorderState.is_redex` L429; UT-0364-12 at L1466-1502 proves the invariant is graph-enforced | **PASS** |

All four verdicts are implemented verbatim. No MUST-FIX.

---

## 4. Test Coverage Audit (TEST-SPEC-0360..0365)

Every unit test named in the TEST-SPECs has a matching `#[test]` function in the inline test module. Names match one-for-one; ordering matches the TEST-SPEC grouping.

| TEST-SPEC | Required tests | Shipped | Location | Note |
|-----------|----------------|---------|----------|------|
| 0360 | UT-0360-01..08 (8) | 8 | L540-714 | All names verbatim; struct-shape + derive + module-wiring coverage complete. |
| 0361 | UT-0361-01..08 (8) | 8 | L722-837 | All three panic cases covered with `#[should_panic(expected = ...)]`. |
| 0362 | UT-0362-01..11 (11) | 11 | L845-1094 | Full R11/R17/R18 lattice; includes the disconnect-then-reconnect stress case. |
| 0363 | UT-0363-01..08 (8) | 8 | L1102-1264 | DC-3 load-bearing `&mut self`-inside-loop test present at L1133. |
| 0364 | UT-0364-01..12 (12, incl. DC-4 mandated #12) | 12 | L1272-1502 | UT-0364-12 (`add_border_states_enforces_is_redex_invariant`) present with the exact mandated name. |
| 0365 | UT-0365-01..03 (3) | 3 | L1511-1573 | R19 source scan, doc-presence scan, `Send+Sync` compile-time check. |

**Total: 50 new inline tests, matching DEV's reported +50 on both feature builds.** No test name is missing.

Missing coverage: **none at the TEST-SPEC level.**

Minor test-side observations (not MUST-FIX):
- UT-0362-01 (L845-868) uses a runtime branch (L851-855) to pick the aux-side worker rather than asserting the fixture's construction order. This is robust against `HashMap` iteration non-determinism and aligns with the TEST-SPEC note about set-equality assertions. Good practice.
- UT-0362-09 (L1017-1030) tests the redundant-same-value path by re-writing the same side to itself; the TEST-SPEC wording permits a tighter verbatim check on the same-value target, but the looser check here still covers the invariant path. Acceptable.

---

## 5. Code Quality Audit

| Check | Result | Details |
|-------|--------|---------|
| `.unwrap()` in production code | **CLEAN** | Grep shows 26 `unwrap` hits; **all 26 are inside `#[cfg(test)] mod tests`**. The single production use is `.unwrap_or(0)` at L193 (safe default on a missing `HashMap::get`), which is an infallible variant, not `.unwrap()`. |
| `unsafe` blocks | **CLEAN** | Zero occurrences (grep confirms). |
| `println!` | **CLEAN** | Zero occurrences. Panics in `from_partition_plan` (L195, L204, L210) and `add_border_states` (L402, L410, L416) use `panic!()` with structured messages — consistent with other core modules and the developer brief. |
| Comments explaining IC concepts | **GOOD** | The module-level `//!` block (L1-56) lays out coordinator-side lifecycle + pure-core invariant + out-of-scope frame. `BorderState`'s `is_redex` field has the "counter-intuitive for programmers" rationale at L68-71 ("cached principal-pair flag... so the coordinator can answer 'is this border reducible?' in O(1)"). `BorderDelta` doc at L117-124 explains why `DISCONNECTED` is a sentinel rather than an enum variant. |
| Naming conventions | **GOOD** | `snake_case` methods, `PascalCase` types, `SCREAMING_SNAKE_CASE` constants (via re-export of `DISCONNECTED`). Method names match spec verbatim (`from_partition_plan`, `apply_deltas`, `detect_border_redexes`, `add_border_states`, `remove_border`). |
| `pub(crate)` vs `pub` discipline | **GOOD** | `BorderGraph` fields are all `pub(crate)` (L103, L111, L114) — the struct is public (consumed by the item 2.26 delta coordinator under `run_grid`) but its internal field shape is private to the crate. `BorderState`, `BorderDelta`, `AddBorderEntry` are `pub` with `pub` fields because they cross the public-API boundary as value types (expected for transport/integration). Methods on `BorderGraph` are all `pub` — necessary for the future coordinator to construct + mutate the graph. |
| Dead-code warning silenced | **DELIBERATE** | `#[allow(dead_code)]` at L110 guards `worker_borders`. This is the DC-2 mandated mechanism (field has no READER in this bundle; the READER ships under item 2.26 R23). The doc-comment at L104-108 explicitly cites the future consumer, which satisfies the spec-critic's "prevent a future reviewer from pruning it as dead code" requirement. **No other `#[allow]` attributes present.** |
| Derives on public types | **COMPLETE** | `BorderState`: `Debug, Clone, PartialEq, Eq` (L72). `BorderDelta`: `Debug, Clone, Copy, PartialEq, Eq` (L125). `AddBorderEntry`: `Debug, Clone, Copy, PartialEq, Eq` (L142). `BorderGraph`: `Debug, Clone` (L100) — deliberately NOT `PartialEq, Eq` per spec-critic DC-2 note (equality on a coordinator-side graph is not meaningful without a canonicalization pass). Matches TEST-SPEC-0360 acceptance criteria exactly. |
| No `serde`/`rkyv` derives yet | **CORRECT** | TEST-SPEC-0362 explicitly forbids them in this bundle (item 2.26 adds them non-breakingly). Confirmed absent. |

---

## 6. Architecture Audit

### Pure-core invariant (R19)

**PASS.** Imports at L58-63 are the four lines the spec-critic predicted verbatim: `std::collections`, `crate::net`, `crate::partition::types`, `super::helpers`. No `tokio`, `async`, `async_trait`, `crate::protocol`, `std::io`, `std::fs`, no runtime or I/O. The `UT-0365-01` source-scan at L1511-1528 makes any future regression a compile-time-of-tests failure.

### Dependency direction

**PASS.** The file sits in `merge/` and depends only on `net` and `partition::types` and its sibling `helpers`. The direction `net ← reduction ← partition ← merge ← protocol ← coordinator/worker` is unbroken. The future caller (item 2.26 `run_grid_delta`) will live in `merge/grid.rs` or a sibling; consumption of `BorderGraph` from `protocol/coordinator.rs` is explicitly architected in SPEC-13 R6-R8 (coordinator may reach into core) — not yet active.

### `net ← merge/border_graph` and `partition ← merge` directions

Both correct. `border_graph.rs` imports from `crate::net` (for `PortRef`, `DISCONNECTED`) and `crate::partition::types` (for `PartitionPlan`, `WorkerId`); nothing upstream imports `merge/border_graph`. `merge/mod.rs` re-exports four names at the `crate::merge` level, suitable for the future coordinator entry point.

### Should any type move to `merge/types.rs`?

**No, as shipped is correct.** `merge/types.rs` currently holds `GridConfig`, `GridMetrics`, `WorkerRoundStats` — types that live at the merge-layer API surface and are re-exported from `crate::merge::*`. The four new types (`BorderGraph`, `BorderState`, `BorderDelta`, `AddBorderEntry`) are tightly coupled to `border_graph.rs`'s behavior (derives, `pub(crate)` field discipline, the `from_partition_plan` constructor, the invariant contracts between `BorderGraph.active_redexes` and `BorderState.is_redex`). Moving them to `types.rs` would separate struct from invariants, losing locality. The re-export strategy in `merge/mod.rs` makes them equally reachable from outside. This matches the precedent set by `helpers.rs` (helpers live beside their sole caller in `core.rs`) rather than by `types.rs`.

### Re-export surface (`merge/mod.rs` L13)

```rust
pub use border_graph::{AddBorderEntry, BorderDelta, BorderGraph, BorderState};
```

**Correct granularity.** Four explicit names, alphabetically ordered, matching the existing pattern at L14-17. No `pub use border_graph::*;` glob (which would leak the `is_principal_pair` re-export upward — a visibility violation per SPEC-13). The `is_principal_pair` helper remains `pub(crate)` in `helpers.rs` and is reached inside `border_graph.rs` via `super::helpers::is_principal_pair` (L63). QA-0360-F (glob re-export regression) is architecturally prevented.

---

## 7. Complexity Gate (R18)

R18 mandates O(B) init, O(\|deltas\|) apply, O(\|redex_set\|) detect.

**Init (`from_partition_plan`, L171-249):**
- Pass 1 (L180-185): iterates every `(border_id, port)` in every `partition.free_port_index`. Total work = `sum_i |partitions[i].free_port_index|` = O(B). `HashMap::entry().or_default().push(...)` is amortized O(1).
- Pass 2 (L192-215): two loops over the sighting keyspace, each O(B) with O(1) `HashMap::get` / `contains_key`.
- Pass 3 (L218-242): one `HashMap::with_capacity` pre-allocation (L218) avoids rehashing; the loop body is O(1) per sighting (one `HashMap::insert`, two `Vec::push`, one optional `HashSet::insert`).

Total: **O(B), matches R18.** The `HashMap::with_capacity(sightings.len())` at L218 is a small but real hygiene win.

**Apply (`apply_deltas`, L266-316):**
- Outer loop iterates the `deltas` slice exactly once (L267).
- Body work per delta: one `HashMap::get_mut` (O(1)), two equality checks on `WorkerId` (O(1)), one conditional side assignment (O(1)), at most one `HashMap::remove` + one `HashSet::remove` (O(1) each), one `is_principal_pair` call (O(1)), at most one `HashSet::insert`/`remove` for `active_redexes` (O(1)).

Total: **O(|deltas|), matches R18.**

**Detect (`detect_border_redexes`, L334-339):**
- Iterates `self.active_redexes.iter()` exactly once (L335).
- Per-item: one `self.borders.get(&bid)` (O(1)), one `BorderState.clone()` (O(1) — 6 fields, no heap-backed).
- Returns `Vec<(u32, BorderState)>` (one final `.collect()`, O(|active_redexes|) allocations).

Total: **O(|active_redexes|), matches R18 SHOULD** (incremental form). UT-0363-07 (L1214-1235) empirically pins the property with a 1001-border fixture containing only 1 redex — the output has exactly 1 entry.

**Data-structure choices:** `HashMap<u32, BorderState>` for `borders` (O(1) lookup by `border_id`), `Vec<Vec<u32>>` for `worker_borders` (dense-indexed reverse lookup, matches DC-2 verdict — a `HashMap<WorkerId, HashSet<u32>>` would have added hashing overhead for no benefit given dense `WorkerId = 0..N-1` per SPEC-04), `HashSet<u32>` for `active_redexes` (O(1) insert/remove/lookup, iteration O(|set|)). All three choices are the spec-mandated or minimally-justified shapes. No `BTreeMap` overhead anywhere.

**R18 GATE: PASS.**

---

## 8. Findings

### MUST-FIX

**None.**

### SHOULD-FIX

**None.**

### NICE-TO-HAVE (non-blocking; may land in item 2.26 or a follow-up)

1. **`from_partition_plan` validation redundancy (L192-215).** The current code runs *two* bidirectional passes: one over `plan.borders.keys()` (L192-200) asserting every declared border has exactly 2 sightings, and one over `sightings` (L201-215) asserting every sighting appears in `plan.borders` AND has exactly 2 entries. The "count != 2" check is duplicated across both passes. A minimally-refactored single loop over `sightings.iter()` that also builds a `HashSet<u32>` of seen IDs and afterwards asserts `seen == plan.borders.keys().copied().collect()` would halve the inspection cost and eliminate the duplicated panic branches. Current code is O(B) in the worst case, so this is a constant-factor cleanup; does not change the complexity class. **Not blocking: current form is correct and the panic messages are precise.**

2. **`detect_border_redexes` defensive filter-map (L337).** The `filter_map` silently skips stale `active_redexes` entries whose `borders` entry is missing. The doc-comment at L327-330 correctly flags this as defensive ("this should not happen under a correct implementation"). A `debug_assert!` inside the closure's `None` branch would fail fast under `cfg(debug_assertions)` while still shipping the safe path in release, catching QA-0363-A at the earliest possible point. **Not blocking: the current defensive skip is spec-critic-approved; a debug assert is an additional diagnostic aid.**

3. **`BorderState` memory layout.** Current field order (`border_id: u32, side_a: PortRef, side_b: PortRef, worker_a: WorkerId, worker_b: WorkerId, is_redex: bool`) places the `bool` at the end, after two 4-byte fields — Rust will pad to align, yielding ~24 bytes assuming `PortRef` is 8 bytes (tag + 8) and `u32` fields pack tightly. This matches spec R9 field order verbatim, so the order is immutable. **Observational only** — no change proposed, but for future bundle awareness: packing `is_redex` between two `u32`s could save 3 padding bytes per entry at scale (32 bytes/entry × 1M borders = 32 MB; savings ~3 MB). Not material until hot paths in item 2.26 are profiled. Spec R9 field order wins over micro-packing.

---

## 9. QA Probe Recommendations (for Stage 5)

**10 adversarial angles** for QA. Ordered by priority (1 = highest-signal).

1. **Boundary value: `border_id = u32::MAX`** (but NOT the DISCONNECTED sentinel — use `PortRef::AgentPort(u32::MAX, 0)` as a border endpoint). `DISCONNECTED` is specifically `FreePort(u32::MAX)`, and the code never confuses a `border_id` (HashMap key) with a `PortRef` encoding. Confirm: (a) a real border with `border_id = u32::MAX` round-trips through init → apply_deltas → detect → remove with no interaction with DISCONNECTED sentinel logic. (b) `HashMap::with_capacity(u32::MAX as usize)` is never called (it isn't — capacity comes from `sightings.len()`, not ID value). (c) `active_redexes.contains(&u32::MAX)` behaves identically to any other ID.

2. **Hash-collision adversarial input for `HashMap<u32, BorderState>`.** Default `HashMap` in std uses `SipHash` which is randomized per process, so generating collisions is probabilistic. QA should verify that adding 100k borders with sequential IDs, then randomly ordered IDs, then pathological IDs (e.g., `0, 1, 2, 2^31, 2^31+1, ...`) all yield the same observable post-state (same `len`, same `active_redex_count`). C3 panic on "multiple sightings" must survive the adversarial ordering.

3. **The C3 panic path under a 4+ sighting.** UT-0361-07 covers 3 sightings. QA should craft a plan where the same `border_id` appears in 4, 5, ..., N partitions. The L204 panic message must name the count accurately (not "3", but the true count).

4. **R17 double-disconnect ordering: same worker calls `apply_deltas` twice with DISCONNECTED on each side in turn.** A worker can own only one side; the double-disconnect case requires TWO distinct workers to each report DISCONNECTED. But a bug that lets the same worker's second DISCONNECTED "disconnect the other side" would be silent. QA should craft a sequence `apply_deltas(W0, [disc bid=1])`, then `apply_deltas(W0, [disc bid=1])` again. The border must remain alive (side_b still owned by W1, still non-DISCONNECTED). Invariant cross-check: `active_redexes` unchanged between the two calls.

5. **Ordering dependence from `HashMap` iteration.** `from_partition_plan` L222 iterates `sightings` (a `HashMap`). The `(wa, pa) = sights[0]; (wb, pb) = sights[1]` assignment depends on insertion order within the Vec inside each HashMap entry, NOT on the outer HashMap's iteration. **This is deterministic** because the Vec insertion order follows the sequential partition iteration at L180-184 (stable). However, if a future refactor reshapes `sightings` to use a `HashSet` or shuffles insertion, the `(side_a, worker_a)` vs `(side_b, worker_b)` assignment becomes non-deterministic. QA probe: run `from_partition_plan` 100× on the same input and assert that `borders.get(&bid).unwrap().side_a` is byte-identical across runs. Today, it is. Locks the property.

6. **`add_border_states` mid-batch panic leaves the graph in a partially-mutated state.** If entry 0 succeeds and entry 5 panics on duplicate, entries 1-4 are already in `self.borders` and `self.active_redexes`. `BorderGraph` has no transaction semantics. This is spec-compliant (no spec requirement for atomicity), but QA should confirm: (a) the panic message identifies the offending entry's `border_id` (it does, via L403-404). (b) after catching the panic (`std::panic::catch_unwind`), the graph is still internally consistent (invariant `active_redexes == {bid : borders[bid].is_redex}` still holds over the partial batch). (c) a second `add_border_states` call with the successfully-inserted prefix would panic with "duplicate" — a potential surprise for callers doing retry logic.

7. **`apply_deltas` with empty `&[]` on a graph in an "impossible" state.** Construct a `BorderGraph` by direct struct literal (via the `pub(crate)` field access inside the test module) where `active_redexes` contains a `border_id` NOT in `borders`. Then call `apply_deltas(_, &[])`. The no-op path must not touch `active_redexes` or re-materialize any invariant. `detect_border_redexes` called afterward would exercise the L337 defensive filter-map. This indirectly stresses NICE-TO-HAVE #2 (the `debug_assert!` observation).

8. **DC-2 load-bearing: `worker_borders` staleness after a chain of `remove_border`s.** Build a graph with N borders all owned by worker 0 on side_a. Remove them all via `remove_border`. Assert `worker_borders[0]` still contains all N IDs (the spec §4.2 note + the L371-372 comment). This pins the contract; a future item 2.26 dispatcher that does not cross-check `worker_borders` against `borders` would send spurious round-start messages, but the graph's own invariant is preserved.

9. **`AddBorderEntry` with `side_a == side_b` (self-loop).** QA-0364-C from TEST-SPEC-0364. Spec doesn't forbid it. Stored `is_redex = is_principal_pair(side_a, side_b)`. If both are `AgentPort(x, 0)` with the same `x`, is this a self-loop redex? Probably yes under R9's definition (both are principal), but the semantic of "a worker's agent interacting with itself across a fake border" is nonsense. QA should raise this for spec-critic disposition — may warrant a stronger invariant in a future amendment.

10. **`from_partition_plan` with `WorkerId = u32::MAX`.** `worker_borders: Vec<Vec<u32>>` is sized by `plan.partitions.len()`, indexed by `worker_id as usize`. If a partition declares `worker_id = u32::MAX` but `plan.partitions.len() == 1`, the index `u32::MAX as usize` on L237 panics (out-of-bounds, `thread 'main' panicked at 'index out of bounds'`). The spec assumes dense 0..N-1 per SPEC-04, so this is "user error"; QA probe confirms the panic is graceful (no undefined behavior, no silent Vec re-allocation). Complements QA-0361-B from the TEST-SPEC.

**Concurrency interactions:** Not relevant here — `BorderGraph` is pure sync. QA should NOT probe concurrent access; `Send + Sync` suffices and is already tested (UT-0365-03).

---

## Appendix — Production-code `.unwrap()` grep result

```
L193:    let count = sightings.get(bid).map(|v| v.len()).unwrap_or(0);
```

The only occurrence. `.unwrap_or(0)` is infallible (not `.unwrap()`). All other 25 `unwrap` hits are inside `#[cfg(test)] mod tests`. **Production `.unwrap()` count: 0.** Clean.

---

**End of review.**
