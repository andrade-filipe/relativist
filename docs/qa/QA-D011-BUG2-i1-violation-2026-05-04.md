# QA Bug Report — D-011 Bug 2: dense `build_subnet` ID-range violation produces I1 asymmetry post-merge

**Date:** 2026-05-04
**Branch:** `v2-development` (working tree includes Phase B uncommitted changes)
**Severity:** CRITICAL (correctness, distributed reduction silently corrupts merged Net)
**Status:** Root cause identified. Fix shape recommended below. NO production code modified.

---

## 1. Summary

`partition::helpers::build_subnet` (dense path) returns a per-worker `Net` with `next_id = 0` (line `helpers.rs:382`). The split orchestrator (`split.rs:96-98`) then sets `subnet.next_id = max(subnet.next_id, max_agent_id + 1)`, where `max_agent_id` is the **largest live AgentId of THIS worker**. That value is bounded by the strategy-supplied agent IDs (e.g., 4 for worker 0 holding agents 0..4), but it is COMPLETELY DECOUPLED from the worker's assigned `id_range` (e.g., `[10..100_010)`). Worker 0 therefore begins fresh allocation at ID 5, which is INSIDE worker 1's `id_range`. After local CON-DUP commutation, worker 0 has new agents at IDs 5,6,7,8 — colliding with worker 1's pre-existing original agents 5..9. Merge concatenates both partitions into one `result.agents` arena; the later partition wins each colliding slot. Wires from worker 0's new agents that point to "AgentPort(5, _)" land on worker 1's original Dup@5 in the merged arena, producing the I1 asymmetry observed at `merge::core::merge → assert_all_invariants`.

The sparse path (used for ALL distributed runs prior to D-011 Phase B) is immune because `build_subnet_sparse` initializes `sparse.next_id = id_range.start` (line `helpers.rs:616`); the subsequent `split.rs:96-98` `max(id_range.start, max_agent_id + 1)` then yields a value **inside** the worker's range. Phase B's metric correction (effective_arena_size threshold) was the first change to route real reduction workloads through the dense path, exposing a latent bug that has lived in `build_subnet` since SPEC-22 R10a was added.

## 2. Reproducer

```bash
RUST_BACKTRACE=1 /c/Users/Filipe/.cargo/bin/cargo.exe test --lib \
  bench::suite::tests::test_suite_correctness_holds_for_all_benchmarks 2>&1 | tail -20
```

Expected panic:

```
thread '...' panicked at relativist-core\src\net\debug.rs:49:17:
assertion `left == right` failed: I1 violated:
  AgentPort(0, 1) -> AgentPort(5, 2),
  but AgentPort(5, 2) -> FreePort(11)
  left: FreePort(11)
  right: AgentPort(0, 1)
```

Reproducer call stack (from RUST_BACKTRACE=1):

```
4: Net::assert_adjacency_consistent          (debug.rs:49)
5: Net::assert_all_invariants                (debug.rs:165)
6: merge::core::merge                        (merge/core.rs:314)
7: merge::grid::run_grid                     (merge/grid.rs:260)
8: bench::suite::measure_grid                (bench/suite.rs:494)
9: bench::suite::run_benchmark_suite         (bench/suite.rs:954)
10: test_suite_correctness_holds_for_all_benchmarks
```

Sanity confirm (sparse path passes): `git stash` Phase B, run the same command — passes. `git stash pop` — fails again. Confirmed.

## 3. Hypotheses tested

| # | Hypothesis | Test performed | Result | Conclusion |
|---|-----------|----------------|--------|------------|
| H0 | Bug 2 is a downstream consequence of Bug 1 (`freeport_redirects` drop on dense, `helpers.rs:384`). | Previous DEV applied `freeport_redirects: net.freeport_redirects.clone()`. | Same panic, identical positions. | REJECTED (per dispatch brief). |
| H1 | Asymmetric `border_entries` from `classify_wires`. | Read `classify_wires` (`helpers.rs:213-261`); confirmed lines 252-253 push entries for BOTH workers symmetrically when `agent_id < other_id`. | Symmetric — never single-sided. | REJECTED. |
| H2 | Dense `build_subnet` fails to fix up the partner's port for border overrides. | Same applies to sparse (lines 580-590 use the same one-sided write). Both correctly rely on the `border_overrides` map being populated for THE LOCAL agent only; the remote partner's port lives in a DIFFERENT subnet and receives ITS OWN border override via the symmetric `border_entries` push above. | Same logic shape on both paths — not the asymmetry. | REJECTED. |
| H3 | Reduction introduces asymmetry via `freeport_redirects`. | Even with H0's fix applied, the panic persists with identical positions. | The asymmetry is structural, not redirect-driven. | REJECTED. |
| H4 | Merge fails to symmetrize border-FreePort resolution under dense path. | Diagnostic dump shows merge correctly wires `(4,0)↔(5,0)` for the border (the only border in this run). The asymmetry is NOT on the border wire — it's on AgentPort(0,1)↔AgentPort(5,2), an INTERNAL wire produced by worker 0's local reduction. | Merge logic is correct. | REJECTED. |
| H5 | `condup_expansion` topology triggers a unique code path. | The bench creates 5 CON-DUP redex pairs. With workers=2 and `ContiguousIdStrategy`, redex (4,5) is the SOLE border redex; redexes (0,1),(2,3) → worker 0; (6,7),(8,9) → worker 1. Each worker performs 2 CON-DUP commutations LOCALLY (each creates 4 new agents). This is the only bench in the suite that does CON-DUP commutation locally + has uneven freeport IDs. The other passing benches (`ep_annihilation*`) annihilate (no new agents) — never trigger fresh allocation in workers. | The bench shape exposes the bug because CON-DUP commutation is the only rule that creates new agents during local reduction, which is the precondition for the bug. | KEY OBSERVATION — explains why other benches in the same suite passed. |
| **H6** | **Dense `build_subnet` returns `next_id=0`. Combined with `split.rs:96-98` `min_next_id = max_agent_id + 1`, worker 0 begins fresh allocation OUTSIDE its assigned `id_range`, colliding with another worker's range.** | Diagnostic dump (added then reverted) of partitions BEFORE merge for `condup_expansion`: worker 0's agents.len()=9 with `agent[5] = Con` AND worker 1's agents.len()=14 with `agent[5] = Dup`. **Both workers materialized agents at ID 5.** Worker 0's Con@5 has wires `(5,1)→AgentPort(1,1)` and `(5,2)→AgentPort(0,1)` (CON-DUP commutation pattern). Worker 1's original Dup@5 has wires `(5,1)→FreePort(10)` and `(5,2)→FreePort(11)` (the original Lafont free-ports). After `merge::core::merge` Step 2 (lines 109-151) iterates partition 0 then partition 1, the SECOND partition's writes overwrite slot 5 → result.agents[5] = Dup, ports[15..18] take Dup's values. But result.agents[0] = Worker 0's Dup@0 with port `(0,1)→AgentPort(5,2)` (pointing to what WAS Con@5 in worker 0), now landing on Dup@5 from worker 1. The reverse port `(5,2)` reads `FreePort(11)`. **Asymmetry confirmed.** | CONFIRMED — root cause. |

## 4. Root cause

### File / line

- **`relativist-core/src/partition/helpers.rs:382`** — dense `build_subnet` returns `next_id: 0`.
- The downstream override at `relativist-core/src/partition/split.rs:96-98` only widens `next_id` to `max_agent_id + 1`, NOT to `id_range.start`.

### Faulty logic

```rust
// helpers.rs:378-399 (dense build_subnet return value)
Net {
    agents,
    ports,
    redex_queue,
    next_id: 0, // Caller sets this based on ID range          // <-- BUG: misleading comment
    root: None,
    freeport_redirects: std::collections::HashMap::new(),       // (Bug 1, separate)
    free_list,
    id_range: Some(id_range),
    border_entries_shadow,
    ...
}
```

```rust
// split.rs:93-98 (orchestrator override)
// Set next_id: max(id_range.start, max_agent_id + 1)   <-- comment LIES; code does NOT max with range.start
// sparse path sets next_id = id_range.start; dense path sets next_id = 0.
// Ensure max_agent_id + 1 is also respected (I3' upper bound).
let max_agent_id = worker_agents[i].iter().copied().max();
let min_next_id = max_agent_id.map(|m| m + 1).unwrap_or(0);
subnet.next_id = std::cmp::max(subnet.next_id, min_next_id);
```

The comment on line 93 says "max(id_range.start, max_agent_id + 1)" but the implementation only computes `max(subnet.next_id, max_agent_id + 1)` — it relies on `subnet.next_id` ALREADY being `>= id_range.start` upon return from `build_subnet*`. The sparse path satisfies this (line 616: `sparse.next_id = id_range.start`); the dense path violates it (line 382: `next_id: 0`).

### Trace for `condup_expansion` size=5, workers=2

- `compute_id_ranges(2, 10)` (since original `net.next_id = 10`):
  - worker 0: `IdRange { start: 10, end: 100_010 }`
  - worker 1: `IdRange { start: 100_010, end: u32::MAX }`
- Worker 0 dense: `build_subnet` returns `next_id = 0`. split.rs override: `min_next_id = 4 + 1 = 5`. `subnet.next_id = max(0, 5) = 5`. **5 is OUTSIDE worker 0's range [10..100_010), and INSIDE worker 1's range [100_010..) — wait no, 5 < 100_010, so it's not in worker 1's range either; it's in NO worker's assigned range, but it overlaps with worker 1's PRE-EXISTING agent IDs 5..9.**
- Worker 1 dense: `build_subnet` returns `next_id = 0`. split.rs override: `min_next_id = 9 + 1 = 10`. `subnet.next_id = max(0, 10) = 10`. **10 is also OUTSIDE worker 1's range [100_010..), but doesn't matter for this benchmark because worker 1's reduction never creates an agent (the bench's worker 1 redexes (6,7) and (8,9) commute creating new agents starting at ID 10 — these may collide with future allocations if the scenario grew).** In the dump, worker 1's allocations went 10..13 (4 new agents from one commutation? Actually the dump shows agents 10..13, all live, and 6..9 also live — meaning worker 1 created 4 new agents at IDs 10..13, while pair (6,7) and (8,9) consumed and the commutation re-used ports etc. Verifying exactly is incidental — the key smoking gun is worker 0's collision with IDs 5..8.)
- During merge `Step 2`, the iteration order is `for partition in &partitions` — worker 0 first (writes to slots 0..8), then worker 1 (overwrites slots 5..9 with its own values, leaves slots 10..13 untouched in the result so far).
- Final merged `result.agents[5] = Dup` (from worker 1, original agent), `result.ports[5*3..5*3+3] = [DISCONNECTED, FreePort(10), FreePort(11)]` (worker 1's port values; principal cleared because border bid=20 was unrouted by Step 2 then fixed up in Step 3 to `AgentPort(4, 0)`).
- But `result.agents[0] = Dup` (from worker 0's commutation, agent 0 reused via free_list) with port `(0,1) = AgentPort(5, 2)` — pointing to what worker 0 thought was its own Con@5 but is actually now worker 1's Dup@5 with `(5,2) = FreePort(11)`. **I1 asymmetry.**

## 5. Why pre-Phase-B (sparse path) works

Pre-Phase-B, `id_range_size > 4 * live_count` (the OLD metric used `id_range.end - id_range.start` ≈ 100_000 vs 4 × 5 = 20) was always true for these small benches → `build_subnet_with_config` routed through SPARSE. `build_subnet_sparse` line 616 sets `sparse.next_id = id_range.start = 10` for worker 0. After `to_dense`, the resulting `Net.next_id = 10`. split.rs override: `max(10, 5) = 10`. **Worker 0 allocates new agents at IDs 10, 11, 12, 13 — INSIDE worker 0's range and DISJOINT from worker 1's pre-existing IDs 5..9.** No collision, no asymmetry.

Phase B's metric correction (`effective_arena_size = max_live_id + 1` ≈ 5–10 for these benches) makes `5 > 20` false → DENSE path now selected. The dense path was previously dead code in any non-trivial distributed scenario; the correctness bug it harbors had no exposure.

## 6. Recommended fix shape

### Primary fix (1 line, smallest possible change)

In `relativist-core/src/partition/helpers.rs`, change line 382 of `build_subnet`:

```rust
//   from
next_id: 0, // Caller sets this based on ID range
//   to
next_id: id_range.start,
```

This mirrors the sparse path (line 616). With this change, split.rs's `max(subnet.next_id, max_agent_id + 1)` will yield `max(id_range.start, max_agent_id + 1)`, which lives in worker_i's assigned range as the comment promises.

The **comment** on `split.rs:93` ("Set next_id: max(id_range.start, max_agent_id + 1)") then becomes truthful.

### Optional secondary defense (recommended)

Add a runtime guard in `Net::create_agent` (`net/core.rs:525-540`, fresh allocation path) that, when `id_range = Some(r)` and `next_id >= r.end`, panics with a descriptive `D3` message. Currently there's a debug-only check in the RECYCLE path (lines 442-451) but NONE in the FRESH path. Without this guard, ANY future regression in `next_id` initialization (e.g., a new build path or a refactor) silently re-introduces the same cross-partition collision. Suggested:

```rust
#[cfg(debug_assertions)]
if let Some(ref range) = self.id_range {
    debug_assert!(
        self.next_id < range.end,
        "SPEC-22 R10 / D3 violation: fresh next_id {} reached id_range.end {} (would allocate outside partition range)",
        self.next_id, range.end
    );
    debug_assert!(
        self.next_id >= range.start,
        "SPEC-22 R10 / D3 violation: fresh next_id {} below id_range.start {} (cross-partition collision risk)",
        self.next_id, range.start
    );
}
```

Place this immediately before `let id = self.next_id;` (around line 532).

### Alternative (more invasive — NOT recommended unless other surfaces emerge)

Change `split.rs:96-98` to `subnet.next_id = std::cmp::max(id_ranges[i].start, min_next_id);`, ignoring `subnet.next_id` from the build path. This is more defensive but masks any other build paths that might also have a similar bug. The primary fix above keeps each `build_*` path responsible for its own `next_id` initialization, which is more honest.

### Tradeoffs

| Option | Pros | Cons |
|--------|------|------|
| **Primary (1 line)** | Minimal diff; makes the dense path symmetric with sparse; satisfies the truthful interpretation of the split.rs comment. | Does not catch FUTURE re-introductions of the same class of bug. |
| Primary + defensive guard | Catches the bug class going forward; cheap (debug-only); paired with regression test pins it permanently. | One more debug assertion to maintain. |
| Alternative | Defends against ALL build paths setting wrong next_id. | Hides bugs in build paths; weakens the implicit contract that build paths should return a useful next_id. |

**Recommendation: Primary fix + defensive guard, plus the regression witness in §7.**

## 7. Test specification — regression witness

Add to `relativist-core/src/partition/helpers.rs` test module (or split.rs tests):

```rust
/// QA-D011-BUG2 regression: dense `build_subnet` MUST initialize `next_id`
/// to `id_range.start` so that subsequent `create_agent` calls allocate
/// inside the partition's assigned ID range.
///
/// Witness: a 2-worker split of a net where worker 0's `max_agent_id <
/// id_range.start`. Worker 0 must NOT be able to create an agent at any
/// ID inside worker 1's id_range.
#[test]
fn qa_d011_bug2_dense_build_subnet_next_id_in_range() {
    let mut net = Net::new();
    // 4 agents 0..3, 1 cross-partition redex (1↔2)
    let a = net.create_agent(Symbol::Con); // 0
    let b = net.create_agent(Symbol::Dup); // 1
    let c = net.create_agent(Symbol::Con); // 2
    let d = net.create_agent(Symbol::Dup); // 3
    net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));
    net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
    net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));
    net.connect(PortRef::AgentPort(b, 0), PortRef::AgentPort(c, 0)); // border redex
    net.connect(PortRef::AgentPort(c, 1), PortRef::AgentPort(d, 1));
    net.connect(PortRef::AgentPort(c, 2), PortRef::AgentPort(d, 2));
    net.connect(PortRef::AgentPort(d, 0), PortRef::FreePort(1));

    let worker_agents_0 = vec![0u32, 1u32];
    let sigma: HashMap<AgentId, WorkerId> =
        [(0, 0), (1, 0), (2, 1), (3, 1)].into_iter().collect();
    let border_entries: Vec<(AgentId, PortId, u32)> = vec![(1, 0, 100)]; // bid=100

    // Use a non-trivial id_range to expose the bug.
    let id_range = 1000u32..2000u32;

    let subnet = build_subnet(&net, &worker_agents_0, &sigma, &border_entries, 0, id_range.clone());
    // PRIMARY ASSERTION: next_id must be at id_range.start (not 0).
    assert_eq!(
        subnet.next_id, id_range.start,
        "QA-D011-BUG2: dense build_subnet must initialize next_id = id_range.start, got {}",
        subnet.next_id
    );

    // Secondary: end-to-end through split + create_agent must allocate inside range.
    let mut subnet2 = subnet;
    subnet2.next_id = std::cmp::max(subnet2.next_id, /* max_agent_id+1 */ 2);
    let new_id = subnet2.create_agent(Symbol::Era);
    assert!(
        id_range.contains(&new_id),
        "QA-D011-BUG2: create_agent allocated id {} OUTSIDE id_range {:?}",
        new_id, id_range
    );
}
```

And/or an end-to-end witness through `run_grid`:

```rust
/// QA-D011-BUG2 end-to-end: condup_expansion(5) with workers=2 must produce
/// a merged net whose I1 invariant holds (no AgentPort/FreePort asymmetry).
#[test]
fn qa_d011_bug2_condup_expansion_e2e() {
    let net = crate::io::generators::con_dup_expansion(5);
    let cfg = crate::merge::GridConfig {
        num_workers: 2,
        max_rounds: Some(20),
        ..Default::default()
    };
    let strategy = crate::partition::ContiguousIdStrategy;
    let (result, _metrics) = crate::merge::run_grid(net, &cfg, &strategy);
    // The debug-only invariant check inside merge would have already panicked.
    // This explicit call is belt-and-braces and reads naturally as a witness.
    #[cfg(debug_assertions)]
    result.assert_all_invariants();
    let _ = result; // suppress unused in release
}
```

Both should fail BEFORE the primary fix and pass AFTER.

## 8. Ancillary findings

### AF-1 (LOW, observation): `split.rs:93` comment is misleading

The comment claims `Set next_id: max(id_range.start, max_agent_id + 1)` but the code does `max(subnet.next_id, max_agent_id + 1)`. After applying the primary fix, the values coincide for both build paths (both set `subnet.next_id = id_range.start`), and the comment becomes truthful. Suggest also tightening the comment to:

```rust
// Set next_id = max(subnet.next_id, max_agent_id + 1).
// Both build_subnet (dense) and build_subnet_sparse initialize
// subnet.next_id = id_range.start, so this widens to ensure I3'.
```

### AF-2 (MEDIUM, latent): `Net::create_agent` fresh path has NO `id_range` guard

`net/core.rs:525-540` (fresh allocation) does NOT debug-assert that the freshly chosen `id` lies in `id_range`. The recycle path DOES (lines 442-451). This asymmetry is what allowed the bug to be silent — `create_agent` happily handed out IDs outside the partition's range. Recommend the defensive guard from §6 as a separate hardening task (independent of this fix). Without it, ANY future build_* path that forgets to seed `next_id` correctly will silently re-introduce a cross-partition collision.

### AF-3 (MEDIUM, related but out of scope): `merge::core::merge` Step 2 silently overwrites colliding agent IDs

`merge/core.rs:109-151` iterates partitions, writing each agent into `result.agents[i]`. There is a `D3` debug check at the TOP (lines 50-62) that `id_range`s are disjoint, but NO check that the live AgentIds across partitions are disjoint. The bug above demonstrates the failure mode: two partitions can have overlapping LIVE agent sets (NOT overlapping id_ranges) when one partition's `next_id` strays. Recommend adding to merge a debug assertion: before each `result.agents[i] = Some(*agent)`, assert that the slot is currently `None`. Cheap, debug-only, would have caught Bug 2 at the merge boundary instead of waiting for the I1 panic at the end.

```rust
// In merge/core.rs around line 116:
debug_assert!(
    result.agents[i].is_none(),
    "merge: agent ID {} appears in multiple partitions (D3 live-set violation)",
    i
);
```

### AF-4 (LOW): build_subnet's `border_entries_shadow` is built only when `border_entries` is non-empty

`helpers.rs:367` — `if border_entries.is_empty() { None } else { Some(...) }`. This is consistent with sparse (line 620) and not a bug, but worth flagging: if a subnet has zero borders but its `recycle_policy` is `BorderClean`, `is_border_protected` returns `false` for all IDs, which is correct in this case (no borders → nothing to protect). Just noting consistency.

---

**Working tree state at report time:** clean of any QA debug additions. `git status` shows ONLY the pre-existing Phase B + scaffolding changes (per the dispatch brief manifest).

**Reproducer verification at report time:** `cargo test --lib bench::suite::tests::test_suite_correctness_holds_for_all_benchmarks` still fails with the SAME I1 panic, confirming the bug is unaffected by my investigation.
