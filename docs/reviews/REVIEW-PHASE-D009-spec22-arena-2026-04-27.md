# Review: D-009 Phase A-C — SPEC-22 Arena Management (Commits 01184f1..c36a999)

**Date:** 2026-04-27
**Reviewer:** REVIEWER agent (Stage 4, unified code quality + architecture)
**Branch:** v2-development
**Commits reviewed:**

| SHA | Subject | Phase |
|-----|---------|-------|
| `01184f1` | docs(spec): land SPEC-22 amendments A1..A10 | A (specs) |
| `47d9bf2` | feat(arena): SPEC-22 free-list core — Phase B | B (core) |
| `d6411be` | feat(arena): SPEC-22 SparseNet + conversions + I3' — Phase C W1+2 | C-W1+2 |
| `c36a999` | feat(spec22): Wave 3 — CI lint guards + regression suite | C-W3 |

**Code quality verdict:** PASS WITH NOTES
**Architecture verdict:** MINOR DRIFT
**Spec compliance:** SPEC-22 R1-R31 mostly implemented; two contractual gaps documented below

**Test floor:** 1450 default / 1493 zero-copy (v1 floor 690 preserved). Clippy clean. fmt clean.

---

## Summary

The D-009 bundle delivers the SPEC-22 arena management feature set in three phases. Phase A lands the 10 predecessor-spec amendments cleanly. Phase B implements the free-list core (R1-R12, R10a-R10c, R28). Phase C adds SparseNet (R13-R23), round-trip conversions (R19-R21), I3' debug assertions (R24-R27a), and the R30 sparse-build threshold guard.

The implementation is broadly correct and well-tested. Two Must-Fix issues are identified: (1) a stale absolute version number in the amended specs SPEC-18 and SPEC-22 that was overtaken by the D-006 elastic-grid bump, and (2) a structural integration gap where `split()` bypasses `build_subnet_with_config`, leaving the R22/R30 sparse-build path unreachable from the production API. Three Should-Fix issues cover a silent `next_id = 0` from the sparse path, a stale I3 doc comment, and the missing `recycle_under_delta` field in `GridConfig`.

---

## Must-Fix Issues

### MF-001: SPEC-18 R28 and SPEC-22 R9a contain stale absolute version numbers

**Category:** Spec Violation
**Principle/Spec:** SPEC-22 R9a, SPEC-18 R28, SPEC-22 §3.8 A9
**Files:** `specs/SPEC-18-wire-format-v2.md:163,539`, `specs/SPEC-22-arena-management.md:71-73`

**Problem:** SPEC-22 R9a mandates "bump `PROTOCOL_VERSION` from `2` to `3`". SPEC-18 R28 (as amended by A9) says the constant becomes `3`. The code correctly implements the relative +1 increment via `PREVIOUS_LIVE_VERSION = 4` and `PROTOCOL_VERSION = 5`, because the D-006 elastic-grid bundle had already bumped the version from 2 to 4 before D-009 landed. The amendment text was written against a stale base value of 2, but the deployed constant is now 5.

The spec-vs-code skew causes two concrete harms:
- A reader of SPEC-18 who trusts the `§4.7` constant table (`PROTOCOL_VERSION: u8 = 3`) and the R28 text ("bump from 2 to 3") gets a false picture of the actual wire protocol version.
- The SPEC-22 R9a clause "from `2` to `3`" is now historically inaccurate — future specs will inherit the wrong base if they copy this wording.

The implementation itself is correct. The fix belongs in the specs, owned by ESPECIALISTA EM SPECS.

**Before (SPEC-18 §4.7 constant table, after A9 amendment):**
```
/// v3: v2 + Net.free_list field (SPEC-22 §3.8 A9 / R9a). Amendment A9.
pub const PROTOCOL_VERSION: u8 = 3;
```

**After (SPEC-18 §4.7 constant table — correct):**
```
/// v4: v3 + SPEC-20 elastic-grid additions (D-006).
/// v5: v4 + Net.free_list field (SPEC-22 §3.8 A9 / R9a). Amendment A9.
pub const PROTOCOL_VERSION: u8 = 5;
```

**Similarly in SPEC-18 R28 and SPEC-22 R9a:** replace "bump from `2` to `3`" with "bump from `4` to `5`", and update R29 to reference version 4 as the rejected predecessor. The `PREVIOUS_LIVE_VERSION = 4` sentinel in coordinator.rs is the correct anchor — the spec text must match it.

**Why:** Stale absolute version numbers in specs are a documentation debt that silently misleads future developers and spec authors. The `PREVIOUS_LIVE_VERSION` mechanism in the code is the right pattern; the specs need to reflect the same +1-relative logic or the correct absolute values.

---

### MF-002: `split()` bypasses `build_subnet_with_config` — R22/R30 sparse path is unreachable from the production API

**Category:** Architecture / Spec Violation
**Principle/Spec:** SPEC-22 R22, R30; SPEC-04 §4.5.1 A7
**Files:** `relativist-core/src/partition/split.rs:9-10,48-55`, `relativist-core/src/partition/helpers.rs:398-444`

**Problem:** `split()` — the sole public entry point for net partitioning — imports and calls `build_subnet` directly (line 48). `build_subnet_with_config`, which wraps `build_subnet` with the R22 threshold check and the R30 error guard, is never called from `split()`. It is only exercised by the unit tests in `helpers.rs` (call sites at lines 1273, 1303, 1332, 1361, 1423, 1448, 1467, 1491, 1520).

As a result:
- A pathological M5 partition (`id_range_size > 4 × live_count`) silently allocates the full dense arena instead of routing to the sparse path.
- `PartitionError::DenseAllocationExceedsThreshold` is never returned in practice.
- The `sparse_build: bool` flag in `PartitionConfig` has no effect on real grid runs.

This is a spec violation: SPEC-22 R22 says `build_subnet` MUST use `SparseNet` internally when the threshold is exceeded; SPEC-04 §4.5.1 (A7) requires the same for the partition build step.

**Before (`split.rs` lines 9-10, 48):**
```rust
use super::helpers::{build_subnet, classify_wires, compute_id_ranges};
// ...
let mut subnet = build_subnet(
    &net,
    &worker_agents[i],
    &sigma,
    &wire_class.border_entries[i],
    i as WorkerId,
    id_range_for_subnet,
);
```

**After:**
```rust
use super::helpers::{build_subnet_with_config, classify_wires, compute_id_ranges};
use super::types::PartitionConfig;
// ...
// SPEC-22 R22/R30: use the config-driven path that applies the sparse
// threshold guard and routes to SparseNet when id_range_size > 4 × live_count.
let config = PartitionConfig::default(); // sparse_build: true
let mut subnet = build_subnet_with_config(
    &config,
    i,
    &net,
    &worker_agents[i],
    &sigma,
    &wire_class.border_entries[i],
    i as WorkerId,
    id_range_for_subnet,
)
.unwrap_or_else(|e| {
    // DenseAllocationExceedsThreshold is unreachable when sparse_build: true
    // (the sparse path is always taken above the threshold). This arm exists
    // only for future callers passing sparse_build: false.
    panic!("build_subnet_with_config failed: {e}");
});
```

Alternatively, split can accept a `PartitionConfig` parameter and propagate `Result` upward, which is cleaner but a larger API change. The minimal fix is the default-config call above.

**Why:** SPEC-22 R22 is a MUST requirement that closes SC-009 (M5 dense pathology). Without wiring the threshold guard into the actual `split()` call, the M5 memory safety guarantee is purely nominal and the feature is effectively a dead code path at production scale.

---

## Should-Fix Issues

### SF-001: `build_subnet_sparse` sets `next_id = 0`, silently broken for callers of `build_subnet_with_config`

**Category:** Code Quality / Correctness
**File:** `relativist-core/src/partition/helpers.rs:520-521`

**Problem:**
```rust
// Determine next_id: max agent ID in range + 1 (consistent with build_subnet).
sparse.next_id = 0; // to_dense will not use this; kept for symmetry.
```

`SparseNet::to_dense` copies `self.next_id` directly to the resulting `Net::next_id`. Any caller of `build_subnet_with_config` on the sparse path receives a `Net` with `next_id = 0`. If that caller then calls `create_agent`, the returned `AgentId = 0` silently collides with agent 0 from the original net (violating I3'/D4).

Currently `split.rs` manually patches `subnet.next_id` after calling `build_subnet` (line 59), but it doesn't call `build_subnet_with_config` at all (MF-002 above). Once MF-002 is fixed and `split.rs` calls `build_subnet_with_config`, it must also patch `next_id` — or `build_subnet_sparse` must set `next_id` correctly itself.

**Fix:** In `build_subnet_sparse`, set `sparse.next_id` to the partition's `id_range.start` (matching the behavior of `build_subnet`'s contract, where `split.rs` sets `subnet.next_id = max(id_range.start, max_agent_id + 1)`):

```rust
// SPEC-22 R10 / R3: next_id must be at least id_range.start so that
// fresh allocations stay within the partition's assigned range.
// The caller (split.rs) may increase this further based on max_agent_id.
sparse.next_id = id_range.start;
```

The comment "to_dense will not use this" is incorrect and should be removed.

**Why:** Silent `next_id = 0` violates I3' and D4 for any caller that creates agents after `build_subnet_with_config` on the sparse path. The fix is one line; the risk of leaving it unfixed grows as more code paths use `build_subnet_with_config` directly (test-driven or coordinator-side).

---

### SF-002: `Net` doc comment on `next_id` references stale invariant I3 instead of I3'

**Category:** Code Quality / Documentation
**File:** `relativist-core/src/net/core.rs:46-47`

**Problem:**
```rust
/// Next AgentId to be assigned. Strictly greater than any AgentId
/// in use. Incremented on each agent creation (SPEC-01 I3).
pub next_id: AgentId,
```

After SPEC-22 Phase A amendments, I3 was replaced by I3'. The doc comment still references `SPEC-01 I3` and the description "incremented on each agent creation" is no longer accurate (recycled-slot creations do not increment `next_id`).

**Fix:**
```rust
/// Monotonic upper bound on assigned `AgentId`s. Strictly greater than
/// any `AgentId` ever assigned (live, in the free-list, or previously
/// freed and re-assigned). Incremented only on fresh allocations (when
/// the free-list is empty or recycling is disabled); recycled-slot
/// creations leave `next_id` unchanged. (SPEC-01 I3', SPEC-22 R3/R10).
pub next_id: AgentId,
```

**Why:** Doc comments on public fields are part of the API contract. An incorrect cross-reference to the superseded I3 invariant misleads contributors and contradicts the formally amended spec.

---

### SF-003: `GridConfig` is missing the `recycle_under_delta: RecyclePolicy` field specified by SPEC-22 R10b / A10

**Category:** Spec Violation (deferred gap)
**Principle/Spec:** SPEC-22 R10b, §3.8 A10; SPEC-19 R12a
**Files:** `relativist-core/src/merge/types.rs:385+`, `relativist-core/src/net/core.rs:96-100`

**Problem:** SPEC-22 R10b mandates `GridConfig.recycle_under_delta: RecyclePolicy` as the configuration knob for choosing between Strategy A (`DisableUnderDelta`) and Strategy B (`BorderClean`). Amendment A10 lands this into SPEC-19 R12a. The implementation instead places `recycle_policy: RecyclePolicy` as a field directly on `Net` (lines 96-100), which is a per-net setting rather than a grid-level configuration.

The practical consequence: there is no way for a user of `GridConfig` (e.g., the coordinator or a test harness) to configure the recycle policy for a grid run without reaching into each `Net` struct individually. The field is effectively inaccessible from the public grid configuration surface.

**Suggested fix:** Add `recycle_under_delta: RecyclePolicy` to `GridConfig`:
```rust
/// SPEC-22 R10b / §3.8 A10 / SPEC-19 R12a: recycle policy for delta-mode rounds.
/// Controls whether workers may pop from the free-list during a delta-mode round.
/// Default: `RecyclePolicy::DisableUnderDelta` (Strategy A, conservative).
#[serde(default)]
pub recycle_under_delta: RecyclePolicy,
```

The per-`Net` `recycle_policy` field can be retained as an implementation detail that is populated from `GridConfig.recycle_under_delta` at `build_subnet` time. The `Net`-level field is not a problem per se — but the spec contract requires the public knob to live on `GridConfig`.

**Why:** SPEC-22 R10b says "a `GridConfig.recycle_under_delta: RecyclePolicy` field (default: `RecyclePolicy::DisableUnderDelta`)" — this is a MUST. The current wiring hides the control surface from the grid-level API, making Strategy B opt-in invisible from the coordinator contract.

---

## Nice-to-Have

### NTH-001: 4× sparse threshold magic literal should be a named constant

**Category:** Code Quality
**File:** `relativist-core/src/partition/helpers.rs:410`

```rust
let threshold_exceeded = id_range_size > 4 * live_count;
```

SPEC-22 R22 names this the "4× threshold" and motivates it clearly. The literal `4` appears 6 times in helpers.rs (lines 381, 392, 410, 454, 1251, 1288). A named constant makes the intended contract visible at a glance and co-locates the SPEC-22 cross-reference.

**Suggestion:**
```rust
/// SPEC-22 R22: sparse arena threshold multiplier.
/// When `id_range_size > SPARSE_THRESHOLD_MULTIPLIER * live_count`, the
/// dense-arena path would waste >75% of memory. The sparse path is used instead.
const SPARSE_THRESHOLD_MULTIPLIER: u64 = 4;
// ...
let threshold_exceeded = id_range_size > SPARSE_THRESHOLD_MULTIPLIER * live_count;
```

---

### NTH-002: `reconstruct_drain_tombstones` uses O(n²) free-list containment check

**Category:** Code Quality
**File:** `relativist-core/src/net/core.rs:564-568, 580-584`

```rust
&& !self.free_list.contains(&id)
```

`Vec::contains` is O(n). In `reconstruct_drain_tombstones`, this is called once per border-shadow ID. For a partition with many border-referenced IDs (e.g., dense delta mode), this is O(border_count × free_list_len). Since SPEC-22 R6 guarantees the free-list has no duplicates, a local `HashSet` built once at the start of the function eliminates the quadratic cost:

```rust
let existing: std::collections::HashSet<AgentId> = self.free_list.iter().copied().collect();
// then replace `!self.free_list.contains(&id)` with `!existing.contains(&id)`
```

This is a quality improvement with no semantic change. Not blocking, but worth fixing before M5 workloads stress this path.

---

### NTH-003: `build_subnet_sparse` documents itself as "called only when threshold exceeded and sparse_build = true"

**Category:** Code Quality (doc accuracy)
**File:** `relativist-core/src/partition/helpers.rs:454-455`

The function-level doc says:
```
/// Called only when `id_range_size > 4 * live_count` (threshold exceeded)
/// and `config.sparse_build == true`.
```

This is imprecise — the function has no access to `config.sparse_build`; that check lives in `build_subnet_with_config`. The doc should say:
```
/// Internal helper for `build_subnet_with_config`.
/// Called when `id_range_size > SPARSE_THRESHOLD_MULTIPLIER * live_count`.
```

---

## Passed Checks

- [x] No `unwrap()` in production code (all `unwrap()` calls are in `#[test]` modules)
- [x] No `unsafe` blocks in `net/`, `partition/`, `merge/`, `reduction/` — SPEC-22 R31 satisfied
- [x] `pub(crate)` discipline: `build_subnet_sparse` is private (`fn`), `build_subnet_with_config` is `pub`, appropriate
- [x] `SparseNet` derives `Debug, Clone, PartialEq, Eq, Serialize, Deserialize` — R18 satisfied
- [x] `Net.free_list: Vec<AgentId>` field present and serde-included — R1, R9 satisfied
- [x] `Net::new()` and `Net::with_capacity()` initialize `free_list = Vec::new()` — R8 satisfied
- [x] `remove_agent` pushes to free-list with `debug_assert!(!self.free_list.contains(&id))` before push — R2, R6 satisfied
- [x] `create_agent` pops from free-list (LIFO), re-initializes slot, does not increment `next_id` — R3, R4, R5 satisfied
- [x] `count_live_agents` uses `iter().filter(|s| s.is_some()).count()` — free-list slots excluded — R7, R11 satisfied
- [x] `merge()` reconciles free-lists per R12 with the walk-check-push-or-discard algorithm — R12 satisfied
- [x] `is_border_protected` predicate wired to `border_entries_shadow` (Strategy B) and `is_in_delta_round` (Strategy A) — R10b satisfied
- [x] R10c protected-tombstone path: slot set to None, ports DISCONNECTED, ID NOT pushed to free-list — R10c satisfied
- [x] `Net::to_sparse()` iterates live agents only, skips None slots and DISCONNECTED ports — R19 satisfied
- [x] `SparseNet::to_dense()` allocates `max_id + 1` arena, sets free-list from `None` slots in `id_range` — R20, R10a satisfied
- [x] `Net::is_behaviorally_equal()` provided and used in regression tests — R21 satisfied
- [x] `SparseNet::assert_invariants()` validates T1/I1 bidirectional consistency and I2 agent existence — R26 (debug) satisfied
- [x] `debug_check_invariants()` covers all four R27 families — R27 satisfied
- [x] `PROTOCOL_VERSION = 5` with `PREVIOUS_LIVE_VERSION = 4` sentinel; relative +1 increment is correct — R9a implementation correct
- [x] `PartitionConfig { sparse_build: bool }` with default `true` — R30 flag present
- [x] `PartitionError::DenseAllocationExceedsThreshold` error variant defined with clear message — R30 error present
- [x] CI lint `Lint — SparseNet import boundary` targets `src/reduction/` with three distinct import patterns — R23 CI guard correct
- [x] CI lint `Lint — safe-Rust-only audit` targets `net/`, `partition/`, `merge/`, `reduction/` — R31 CI guard correct
- [x] `static_assertions::assert_impl_all!(SparseNet: Send, Sync)` present — SPEC-22 §4.4 Send+Sync satisfied
- [x] `static_assertions::assert_impl_all!(Net: Send, Sync)` present — SPEC-22 §4.4 Send+Sync satisfied
- [x] No monotonicity assertions in `reduction/rules.rs` — R27a / A6 satisfied
- [x] TASK-0478 (bitmap free-list fallback, R32 MAY at <10M) deferred — spec-allowed deferral, documented in Phase B commit message
- [x] `RecyclePolicy::DisableUnderDelta` as default — R10b Strategy A default satisfied
- [x] Module dependency direction preserved: `net/sparse.rs` imports nothing from `reduction`, `partition`, `merge`, `protocol` — SPEC-13 satisfied
- [x] `partition::helpers.rs` uses `SparseNet` (import via `use crate::net::SparseNet`) — R23 construction-time use is permitted per spec
- [ ] `split()` wired to `build_subnet_with_config` — NOT satisfied (MF-002)
- [ ] `GridConfig.recycle_under_delta` field present — NOT satisfied (SF-003)
- [ ] SPEC-18 R28 / SPEC-22 R9a absolute version numbers accurate — NOT satisfied (MF-001, spec-layer fix)

---

## Path to Fix

| Finding | Owner | Blocking? |
|---------|-------|-----------|
| MF-001 (stale version in specs) | ESPECIALISTA EM SPECS | Must fix before bundle closure |
| MF-002 (`split()` bypasses threshold) | DEVELOPER | Must fix before bundle closure |
| SF-001 (`next_id = 0` from sparse path) | DEVELOPER | Should fix alongside MF-002 |
| SF-002 (stale I3 doc comment) | DEVELOPER | Low-effort; fix in same PR as SF-001 |
| SF-003 (`GridConfig.recycle_under_delta`) | DEVELOPER + ESPECIALISTA EM SPECS | Should fix; may require small spec addendum |
| NTH-001 through NTH-003 | DEVELOPER | Discretionary |
