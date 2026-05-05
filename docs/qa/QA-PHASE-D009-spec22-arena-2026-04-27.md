# QA Report — D-009 SPEC-22 Arena Management — Phase A+B+C
**Date:** 2026-04-27
**Phases audited:** A (specs), B (free-list core), C Waves 1+2+3 (SparseNet + CI + regression)
**Commits:** 01184f1, 47d9bf2, d6411be, c36a999
**Reviewer findings already known:** MF-001, MF-002, SF-001..SF-003 — see `docs/reviews/REVIEW-PHASE-D009-spec22-arena-2026-04-27.md`
**Mindset:** Adversarial — try to break the code; assume reviewer missed important issues.

## Summary

- **Total findings:** 14 (CRITICAL: 5, HIGH: 4, MEDIUM: 3, LOW: 2)
- **Top-3 most dangerous:**
  1. **QA-D009-001** (CRITICAL) — `CompactSubnet` (the wire format used to ship every `Partition.subnet` over TCP) silently DROPS `free_list` on the receive side, defeating the entire R9a wire-format mandate of D-009 in distributed runs.
  2. **QA-D009-002** (CRITICAL) — `Net::from_bytes` does NOT call `validate_free_list`. A peer can ship a serialized Net whose `free_list` references live (`Some`) slots; the next `create_agent` re-issues the live agent's ID, fully aliasing two distinct agents and breaching D4/I3' silently. R9 mandates the post-condition check after deserialization.
  3. **QA-D009-003** (CRITICAL) — `R10b/R10c` protected-tombstone path is dead code in distributed runs: neither `build_subnet` nor `build_subnet_sparse` populate `border_entries_shadow` (both unconditionally set it to `None`). Workers running under delta mode therefore never trigger Strategy A or Strategy B, allowing G1 (BorderGraph slot-id stability) violations under SPEC-19 delta rounds.

The bundle has functioning unit-test coverage of the *isolated primitives* (free-list push/pop, SparseNet ↔ Net conversion) but the *integration* between those primitives and the rest of the system is broken at three critical seams: wire serialization, deserialization validation, and delta-mode wiring. Combined with reviewer's MF-002 (split bypasses sparse path) the SPEC-22 feature is largely ornamental at production scale.

---

## Findings

### QA-D009-001 — CompactSubnet wire serializer drops `free_list` on every cross-network partition transfer [CRITICAL]
**File:** `relativist-core/src/partition/compact.rs:38-58, 88-121` (+ `partition/types.rs:114-115`)
**Severity:** CRITICAL
**Category:** Wire Protocol / Spec Violation

**Reproducer (conceptual):**
```rust
let mut net = Net::new();
let p = net.create_agent(Symbol::Era);
net.remove_agent(p);                         // free_list = [0]
let part = Partition { subnet: net, /* ... */ };

// `Partition` serializes through serialize_subnet_compact, which loses free_list:
let wire = bincode::serialize(&part)?;
let recv: Partition = bincode::deserialize(&wire)?;
assert!(recv.subnet.free_list.is_empty());   // !!! lost across the wire
// Worker on the receive side will never recycle id 0; will allocate fresh
// IDs from next_id only. Tombstones accumulate forever.
```

**Why it breaks:** `Partition::subnet` is wired to serde via
```rust
#[serde(serialize_with = "...::serialize_subnet_compact",
         deserialize_with = "...::deserialize_subnet_compact")]
```
in `partition/types.rs:114-115`. The serializer routes through `CompactSubnet`, whose definition (`compact.rs:38-58`) has *no* `free_list` field at all. `into_net()` (line 113) hard-codes `free_list: Vec::new()`. SPEC-22 R9a says "the `free_list` field MUST be included in serde serialization/deserialization of `Net`". The CompactSubnet path violates this for every cross-worker partition transfer — i.e. the only path that matters in distributed runs.

The reviewer's MF-001 (PROTOCOL_VERSION text) and the bumping to v5 give the false impression that wire-format support landed; in practice the Partition wire encoding is functionally unchanged from v4 with respect to free_list.

The dense Net round-trip via `Net::from_bytes` *does* preserve free_list (because `Net` derives Serialize/Deserialize directly, with `free_list` as a normal field). It is precisely the partition shipping path — the production path — that silently drops it.

**Fix sketch:**
1. Add `free_list: Vec<AgentId>` to `CompactSubnet` (with a versioned default for backward compat across the v4→v5 boundary, mirroring R9a's "v5 deserializers MAY tolerate v4 with empty free_list").
2. Copy in `from_net`: `free_list: net.free_list.clone()`.
3. Restore in `into_net`: `free_list: self.free_list,`.
4. Add a regression test that serializes a Partition whose subnet has a non-empty free_list, deserializes, and asserts `subnet.free_list == original`.
5. Bump CompactSubnet's wire version (or piggyback on PROTOCOL_VERSION = 5).

---

### QA-D009-002 — `Net::from_bytes` does not call `validate_free_list`; a malformed serialized Net poisons the arena and aliases live agents on next `create_agent` [CRITICAL]
**File:** `relativist-core/src/net/core.rs:660-669` (`from_bytes`); R9 contract violated
**Severity:** CRITICAL
**Category:** Wire Protocol / Invariant / Panic Path

**Reproducer:**
```rust
// Adversary crafts a serialized Net where free_list contains an ID
// of a live (Some) slot:
let mut net = Net::new();
let id = net.create_agent(Symbol::Con);    // id = 0
// Manually poison free_list (or via a peer with corrupted state):
net.free_list.push(0);                     // id 0 is LIVE but in free-list
let bytes = net.to_bytes()?;

let recv = Net::from_bytes(&bytes)?;       // does NOT call validate_free_list
assert!(recv.validate_free_list().is_err()); // would catch it but isn't called

// Now use the received net in production:
let new_id = recv.create_agent(Symbol::Era);
// LIFO pop returns 0, the slot is overwritten:
//   debug_assert!(slot.is_none()) PANICS in debug,
//   but in RELEASE the assertion is stripped → recv.agents[0] = Some(Era)
//   even though the original Con was reachable via PortRefs / RedexQueue.
// Result: silent aliasing of agent 0; D4 / I3' breach; possible double-reduction.
```

**Why it breaks:** SPEC-22 R9 mandates "A deserialized net MUST have a valid free-list: every ID in the free-list MUST correspond to a `None` slot". The helper `validate_free_list()` exists for exactly this purpose. But `Net::from_bytes()` (line 660-669) is:
```rust
pub fn from_bytes(bytes: &[u8]) -> Result<Net, ...> {
    crate::protocol::bincode_v2::decode_value(bytes).map_err(...)
}
```
No `validate_free_list` call. R9 is unenforced.

In debug builds, the next `create_agent` *might* trip `debug_assert!(slot.is_none())` at `core.rs:250-257`, but:
1. That assertion is `cfg(debug_assertions)` and stripped in release builds.
2. Even in debug, the panic is a defect in *the consumer*, not at deserialization — the corrupted state leaks into the program before being detected.
3. No release-mode error path returns `Err(NetError::FreeListInvalid)` to the network layer.

This is an unauthenticated remote-code-trigger surface in a future hostile-network deployment (worker is auth'd but its serialized state is coordinator-trusted by default).

**Fix sketch:**
```rust
pub fn from_bytes(bytes: &[u8]) -> Result<Net, NetError> {
    let net: Net = crate::protocol::bincode_v2::decode_value(bytes)
        .map_err(|e| NetError::Deserialize(e.to_string()))?;
    net.validate_free_list()?;          // R9 post-condition (always-on, not debug-only)
    // Also: validate no duplicates, no IDs ≥ agents.len() (currently neither is checked)
    Ok(net)
}
```
Add a separate `validate_free_list_no_duplicates` and `validate_free_list_in_bounds` for a complete R6/R7/R9 enforcement set.

---

### QA-D009-003 — Protected-tombstone (R10b/R10c) path is dead code in production: `build_subnet` never populates `border_entries_shadow` [CRITICAL]
**File:** `relativist-core/src/partition/helpers.rs:368-374` (`build_subnet`), `helpers.rs:519` (`build_subnet_sparse` via `to_dense`), `relativist-core/src/net/sparse.rs:314` (`to_dense`)
**Severity:** CRITICAL
**Category:** Logic / Spec Violation / Concurrency Safety

**Reproducer:**
```rust
// 1. Worker receives a partition + delta-mode round flag from coordinator.
let mut subnet = build_subnet(&net, &agents, &sigma, &borders, w, range);
// build_subnet hard-codes: subnet.border_entries_shadow = None;
//                          subnet.is_in_delta_round = false;
subnet.is_in_delta_round = true;            // worker sets manually...
// ... but border_entries_shadow is STILL None.

// 2. Reduction frees a border-referenced agent:
subnet.remove_agent(border_referenced_id);  // is_border_protected() returns false
                                            // because shadow is None!
// → ID is pushed to free_list.

// 3. Next create_agent recycles the border-referenced ID:
let new = subnet.create_agent(Symbol::Era); // returns border_referenced_id
// → Coordinator's BorderGraph still references the OLD agent at this ID.
//   When CommutationBatch arrives indexing AgentPort(border_referenced_id, 0),
//   it now resolves to a different Symbol → G1 violation.
```

**Why it breaks:**
- `is_border_protected(id)` (`net/core.rs:516-520`) returns `true` only when `border_entries_shadow.contains(&id)`.
- Both `build_subnet` and `build_subnet_sparse` (via `SparseNet::to_dense` line 314) unconditionally set `border_entries_shadow: None`.
- The same applies to `is_in_delta_round`, hard-coded to `false`.
- No code anywhere populates these fields *from the partition's `border_entries`*. The coordinator-side wire never delivers them; the worker-side build path never reconstructs them.
- The R10b unit tests synthesize the shadow manually inside the test harness — they do not exercise any production wiring that puts the data there.

The combination of:
- Reviewer's SF-003 (no `GridConfig.recycle_under_delta` field)
- Reviewer's MF-002 (`split` bypasses `build_subnet_with_config`)
- This finding (no `border_entries_shadow` population at all)

means SPEC-22 §3.8 R10b and R10c are formally landed in code but functionally inert. The threat model (round-N→round-N+1 ID reuse causing G1 violation under delta mode) is not mitigated by this implementation.

**Fix sketch:**
1. In `build_subnet` and `build_subnet_sparse`, accept a `delta_mode: bool` parameter (or a richer `WorkerRoundCtx`) and:
   ```rust
   subnet.border_entries_shadow = Some(
       border_entries.iter().map(|&(id, _, _)| id).collect()
   );
   subnet.is_in_delta_round = delta_mode;
   subnet.recycle_policy = grid_config.recycle_under_delta;
   ```
2. Plumb `delta_mode` from coordinator's round-state through `split()` to `build_subnet`.
3. Add an integration test: split a net with cross-partition borders, set delta_mode=true, remove a border-referenced agent in the worker, assert it does NOT appear in free_list and DOES appear in `protected_tombstones` (debug builds).

---

### QA-D009-004 — `merge` free-list reconciliation does not detect cross-partition duplicates; D4 disjointness is assumed but not asserted on the union [CRITICAL]
**File:** `relativist-core/src/merge/core.rs:198-205`
**Severity:** CRITICAL
**Category:** Invariant / Logic

**Reproducer:**
```rust
// Two partitions with overlapping ID ranges (a coordinator bug, but the
// merge does not defend against it):
let mut p0 = make_partition_with_free_list(0, &live0, &[50], 0..100);
let mut p1 = make_partition_with_free_list(1, &live1, &[50], 50..200);
// Both partitions have id 50 in their free_list.
// Both partitions have agents[50] == None.

let (merged, _) = merge(plan);
// The reconciliation loop pushes 50 from p0 (slot None → push), then again
// from p1 (slot still None → push). merged.free_list contains [50, 50].
assert_eq!(merged.free_list.iter().filter(|&&x| x == 50).count(), 2);

// Now create_agent pops one 50, leaves the duplicate. Next remove_agent for
// some other id panics with R6 violation in debug builds — but in release the
// duplicate persists, and a subsequent create_agent issues id 50 a SECOND
// TIME, even though slot 50 is already Some.
```

**Why it breaks:** The reconciliation loop in `merge::core.rs:198-205` is:
```rust
for partition in &partitions {
    for &id in &partition.subnet.free_list {
        if result.agents.get(id as usize).is_some_and(|s| s.is_none()) {
            result.free_list.push(id);
        }
    }
}
```
It checks the slot is `None` before pushing, but it does NOT check whether `id` has already been pushed by a previous partition. The comment "D4 disjointness guarantees no duplicates" is a RUNTIME assumption masquerading as a static guarantee. The disjointness `debug_assert!` at lines 47-58 only checks `IdRange` overlap, not the free_list contents — and that check is debug-only anyway.

The post-condition `validate_free_list()` (line 228) only checks that every free_list ID maps to a None slot — it does NOT detect duplicates. (Reviewer NTH-002 noted the O(n²) cost of dedup detection but did not flag this as a correctness issue.)

A malformed peer, a bugged coordinator FSM, or a release-build state corruption all silently produce a merged net that fails `R6: free-list no duplicates`. The assertion on the *next* `remove_agent.push` is the only line of defense, and it is debug-only.

**Fix sketch:**
```rust
let mut seen = std::collections::HashSet::new();
for partition in &partitions {
    for &id in &partition.subnet.free_list {
        if result.agents.get(id as usize).is_some_and(|s| s.is_none())
           && seen.insert(id)
        {
            result.free_list.push(id);
        }
    }
}
// Also extend `validate_free_list` to detect duplicates:
pub fn validate_free_list(&self) -> Result<(), NetError> {
    let mut seen = HashSet::new();
    for &id in &self.free_list {
        if !seen.insert(id) { return Err(NetError::FreeListInvalid { id, reason: "duplicate" }); }
        if self.agents.get(id as usize).and_then(|s| s.as_ref()).is_some() {
            return Err(NetError::FreeListInvalid { id, reason: "slot is Some" });
        }
    }
    Ok(())
}
```

---

### QA-D009-005 — `SparseNet::to_dense` allocates `(max_id + 1) * 3` bytes unconditionally; an attacker-controlled `max_id ≈ u32::MAX` produces a ~12 GiB allocation request (DoS surface) [CRITICAL]
**File:** `relativist-core/src/net/sparse.rs:266-275`
**Severity:** CRITICAL
**Category:** Resource Exhaustion / DoS

**Reproducer:**
```rust
// Adversary ships a SparseNet with a single live agent at id u32::MAX - 1.
let mut sn = SparseNet::new();
sn.agents.insert(u32::MAX - 1, Agent { symbol: Symbol::Era, id: u32::MAX - 1 });
sn.next_id = u32::MAX;

let dense = sn.to_dense(None);
// Inside to_dense:
//   max_id = u32::MAX - 1
//   arena_len = (u32::MAX - 1) as usize + 1 = u32::MAX as usize ≈ 4.29 × 10^9
//   vec![None; arena_len]               → ~17 GiB on 64-bit (Option<Agent>)
//   vec![DISCONNECTED; arena_len * 3]   → ~12 GiB (PortRef = 8 bytes)
// Total: ~29 GiB allocation request.
// On 32-bit hosts: arena_len * PORTS_PER_SLOT silently OVERFLOWS usize and
// produces a much smaller allocation; subsequent indexing panics or, worse,
// reads from out-of-bounds memory if the panic is caught.
```

**Why it breaks:**
- Line 271: `let arena_len = (max_id as usize + 1).max(range_end);` has no upper bound.
- Line 275: `vec![DISCONNECTED; arena_len * PORTS_PER_SLOT]` — overflow risk on 32-bit; 12 GiB on 64-bit.
- The function is invoked from `build_subnet_sparse` whenever a partition arrives over the wire with a single live agent at a large ID. SPEC-22 R20 even mandates `max_id + 1` allocation — the spec itself codifies the DoS surface.
- This is the M5 pathology that R22 was supposed to *fix*. But by routing through the dense path *after* sparse, the very last step un-does the sparse benefit when the live set is small but `max_id` is large.

This contradicts the reviewer's "M5 memory safety guarantee" claim (PASS bullet on R20). The sparse path *avoids* the dense allocation only if both `live_count` is small AND `max_id` is small. If a single live agent has a high ID, the dense allocation is back.

**Fix sketch:**
1. Add a hard cap on `arena_len` in `to_dense`. If `arena_len > MAX_ARENA_BYTES / sizeof(Option<Agent>)`, return an `Err(NetError::ArenaTooLarge)` and propagate up.
2. Convert `to_dense` from infallible (`-> Net`) to fallible (`-> Result<Net, NetError>`).
3. Apply the same threshold guard from R30 to `to_dense`'s implicit allocation, OR force the caller of `to_dense` to provide an explicit `id_range` (forbidding `None` outside trusted whole-net contexts) so the partition path always passes a bounded range.
4. Add a regression test: `SparseNet { single live agent at u32::MAX - 1 }.to_dense(None)` returns Err, not a 12 GiB allocation.

---

### QA-D009-006 — `to_dense` panics if `id_range.start > id_range.end` (inverted range slicing) [HIGH]
**File:** `relativist-core/src/net/sparse.rs:297`
**Severity:** HIGH
**Category:** Panic Path

**Reproducer:**
```rust
let mut sn = SparseNet::new();
sn.agents.insert(0, Agent { symbol: Symbol::Era, id: 0 });
sn.to_dense(Some(50..10));
// Inside to_dense:
//   max_id = 0, range_end = 10, arena_len = max(1, 10) = 10
//   lo = 50, hi = 10
//   agents[lo..hi] = agents[50..10] → PANIC: slice index starts at 50 but ends at 10
```

**Why it breaks:** Line 297 — `for (i, slot) in agents[lo..hi].iter().enumerate()` — assumes `lo <= hi <= agents.len()`. The function does not validate this. An inverted range (whether produced by a bug in `compute_id_ranges` under elastic resize, or by a malicious peer) panics the worker thread.

Additionally even with a valid range, if `range.start > arena_len` (e.g., `range = 100..200` but max_id = 5, arena_len = 6), `lo = 100, hi = 200` but `arena_len = max(6, 200) = 200`, so `agents.len() = 200` and `agents[100..200]` is in-bounds. So this only fires on inverted ranges, not gaps.

**Fix sketch:**
```rust
debug_assert!(lo <= hi, "to_dense: inverted id_range {:?}", id_range);
if lo > hi {
    return crate::net::Net::new();   // or Err(...), depending on signature change
}
let hi = hi.min(arena_len);          // belt-and-braces: clamp to arena
for (i, slot) in agents[lo..hi].iter().enumerate() { ... }
```

---

### QA-D009-007 — `is_behaviorally_equal` compares `redex_queue` as a `HashSet`, losing multiplicity; nets that differ only in duplicate redex entries are treated equal [HIGH]
**File:** `relativist-core/src/net/core.rs:1003-1007`
**Severity:** HIGH
**Category:** Logic / Equivalence Relation

**Reproducer:**
```rust
let mut a = Net::new();
let p = a.create_agent(Symbol::Con);
let q = a.create_agent(Symbol::Con);
a.connect(PortRef::AgentPort(p, 0), PortRef::AgentPort(q, 0));
// a.redex_queue = [(p, q)]

let mut b = a.clone();
b.redex_queue.push_back((p, q));
b.redex_queue.push_back((p, q));
// b.redex_queue = [(p, q), (p, q), (p, q)]

assert!(a.is_behaviorally_equal(&b));
// True — but b will perform extra (stale) dequeue work versus a.
```

**Why it breaks:** Lines 1003-1007 collect both queues into `HashSet<(AgentId, AgentId)>` and compare. Set equality discards multiplicity. The doc comment claims this is "redex queue up to element ordering" but the code goes further — set, not multiset.

In production this causes:
1. The R21 round-trip test (`spec22_serde_round_trip_dense_sparse_dense`) silently passes when in reality the redex queue is being mutated by `to_sparse`/`to_dense`.
2. Any property-based test using `is_behaviorally_equal` to verify reduction determinism is undermined: two reductions producing different queue states (one with duplicates, one without) appear equal.

Worse: SPEC-22 R21 says "behaviorally equal iff they reduce to the same normal form". Two nets with `[(a,b)]` and `[(a,b),(a,b)]` reduce to the SAME normal form (the duplicate is stale-rejected at dequeue per SPEC-02 R17), so the *high-level* claim survives. But the implementation's set-equality semantics is sloppier than the doc and silently permits other bugs to slip through tests that rely on it.

**Fix sketch:** Use sorted multiset comparison:
```rust
let mut self_redex: Vec<_> = self.redex_queue.iter().copied().collect();
let mut other_redex: Vec<_> = other.redex_queue.iter().copied().collect();
self_redex.sort_unstable();
other_redex.sort_unstable();
if self_redex != other_redex { return false; }
```
Or, if order-independence is the spec's intent, document it precisely AND add a test that asserts the HashSet semantics is intentional (currently the doc says "set equality" but the spec R21 just says "agree on redex queue"; the spec should be tightened too).

---

### QA-D009-008 — `is_behaviorally_equal` compares `free_list` as a `HashSet`, losing LIFO order; two nets with reversed free-lists return equal but allocate different IDs on next `create_agent` [HIGH]
**File:** `relativist-core/src/net/core.rs:1020-1023`
**Severity:** HIGH
**Category:** Logic / Equivalence Relation

**Reproducer:**
```rust
let mut a = Net::new();
a.free_list = vec![5, 3];     // LIFO: next pop returns 3
let mut b = a.clone();
b.free_list = vec![3, 5];     // LIFO: next pop returns 5
// agents/ports/etc are identical; only free_list ORDER differs.

assert!(a.is_behaviorally_equal(&b));
// → true, but next create_agent on a returns 3, on b returns 5.
// Subsequent reduction traces diverge by id assignment.
```

**Why it breaks:** SPEC-22 R5 says "free-list MUST use LIFO ordering". `is_behaviorally_equal` discarding the order means two nets that disagree on the next allocated ID (and therefore on the entire downstream PortRef encoding) are treated as behaviorally equivalent.

This interacts with the `to_sparse → to_dense(Some)` round-trip: the round-tripped dense net's free_list is reconstructed by ascending arena scan (`sparse.rs:297`), which produces a sorted free_list — NOT the original LIFO order. The R21 test passes only because `is_behaviorally_equal` masks the difference.

**Fix sketch:** Compare as `Vec` (full order-sensitive equality) OR document explicitly that the equivalence relation does not preserve next-ID determinism, and require callers that need next-ID determinism to compare `free_list` explicitly. The current behavior is a lurking nondeterminism source under the marketing of "behavioral equality".

---

### QA-D009-009 — `build_subnet_sparse` returns `Net::new()` for empty partitions, dropping the requested `id_range` and producing a free_list that violates R10a [HIGH]
**File:** `relativist-core/src/partition/helpers.rs:467-469`
**Severity:** HIGH
**Category:** Logic / Spec Violation

**Reproducer:**
```rust
let net = /* a net with workers 0,1; sigma assigns nothing to worker 1 */;
let res = build_subnet_with_config(
    &PartitionConfig { sparse_build: true },
    1, &net,
    &[],                                  // worker 1 owns no agents
    &sigma, &[], 1,
    50..200,                              // id_range for worker 1
);
let subnet = res.unwrap();
assert!(subnet.id_range.is_none());       // !!! should be Some(50..200)
assert!(subnet.free_list.is_empty());     // !!! R10a says all None slots in [50..200) should be present
// Worker 1 then allocates from next_id = 0 (Net::new default), colliding
// with worker 0's range.
```

**Why it breaks:** Line 467-469:
```rust
if worker_agents.is_empty() {
    return crate::net::Net::new();
}
```
This bypasses everything: `id_range`, free_list scoping (R10a), and even `next_id` initialization. The dense path (`build_subnet`) at line 297 has the same shortcut — but is at least documented and split.rs corrects `next_id` after.

Empty partitions are not hypothetical: under elastic-grid resize (SPEC-20) a worker may temporarily own zero agents. R10a is unconditional ("MUST populate the free-list").

**Fix sketch:**
```rust
if worker_agents.is_empty() {
    let mut net = Net::new();
    net.id_range = Some(id_range.clone());
    net.next_id = id_range.start;
    // Populate free_list with all IDs in id_range:
    // Per R10a: the partition's None slots — which is the full range when no live agents.
    // (Implementation choice: either populate eagerly, or defer; current code defers
    //  by not populating, which is OK for a "no live agents" arena since it has no
    //  None slots either. But id_range and next_id MUST be set.)
    return net;
}
```

---

### QA-D009-010 — CI lint `safe-Rust-only audit` regex misses `unsafe fn`, `unsafe impl`, `unsafe trait`, `unsafe extern`, and multi-line `unsafe\n{` forms [MEDIUM]
**File:** `.github/workflows/ci.yml:51-54`
**Severity:** MEDIUM
**Category:** Test Gap / CI

**Reproducer:** Add this code to `relativist-core/src/net/foo.rs`:
```rust
unsafe fn evil() {
    *(0xDEADBEEF as *mut u8) = 0;
}
unsafe impl Send for SomeStruct {}
unsafe trait Bar {}
```
The CI lint regex `unsafe[[:space:]]*{` only matches `unsafe {` (block form). `unsafe fn`, `unsafe impl`, `unsafe trait`, `unsafe extern`, and `unsafe<\n>{` (newline before brace) all bypass it.

**Why it breaks:** SPEC-22 R31 says "all SPEC-22 implementations must be safe Rust". The CI guard intends to enforce this but is too narrow. A future contributor could ship `unsafe fn` in `src/net/` and CI would not flag it.

**Fix sketch:**
```yaml
if grep -rnE \
    -e '\bunsafe\b[[:space:]]*\{' \
    -e '\bunsafe[[:space:]]+(fn|impl|trait|extern)\b' \
    relativist-core/src/net/ \
    relativist-core/src/partition/ \
    relativist-core/src/merge/ \
    relativist-core/src/reduction/; then
  echo "ERROR: unsafe found in SPEC-22 implementation files."
  exit 1
fi
```
Or, better, use `cargo geiger` or `#![forbid(unsafe_code)]` at the crate root for SPEC-22 modules.

---

### QA-D009-011 — CI lint `SparseNet import boundary` misses the `pub use` re-export and qualified path forms [MEDIUM]
**File:** `.github/workflows/ci.yml:34-45`
**Severity:** MEDIUM
**Category:** Test Gap / CI

**Reproducer:** Construct an import that bypasses all three patterns:
```rust
// Pattern 1 not matched: use crate::net::sparse::SparseNet
// Pattern 2 not matched: use crate::net::SparseNet
// Pattern 3 not matched: net::sparse::SparseNet (lacks `crate::`)

// All three of these escape:
use crate::net::*;                        // glob re-export pulls SparseNet in
type S = crate::net::SparseNet;           // type alias — no `use ... SparseNet`
let _ : crate::net::sparse::SparseNet;    // same path but no `use`, just a path expression

// Also escapes if a tracked module re-exports it:
// In crate::reduction::helpers (allowed), use crate::net::SparseNet;
// In crate::reduction::engine, use crate::reduction::helpers::SparseNet;
// → grep finds nothing because it only scans src/reduction/ for the SparseNet path,
//   but the local re-export hides it.
```

**Why it breaks:** R23 says `SparseNet` MUST NOT be imported in `src/reduction/`. The grep guard is text-pattern based and misses:
1. Wildcard imports (`use crate::net::*;`) — SparseNet enters via glob.
2. Type aliases without `use`.
3. Direct path expressions in function signatures or trait impls.
4. Transitive imports through other internal modules.

**Fix sketch:** Use a Rust-aware tool (e.g., a small `cargo-deny` or `crates_check` rule, or a `compile_error!` via a feature flag in `src/net/sparse.rs`) that gates SparseNet visibility through Rust's module system. Or: define a dedicated `SparseNet` newtype that is `pub(in crate::net::sparse)` and `pub(in crate::partition)` only — making the import structurally impossible from `src/reduction/`.

---

### QA-D009-012 — `count_live_agents` is O(arena_len) not O(live); a deserialized Net with `arena_len = u32::MAX` and 1 live agent takes seconds to count [MEDIUM]
**File:** `relativist-core/src/net/core.rs:642-646`
**Severity:** MEDIUM
**Category:** Performance / DoS amplifier

**Reproducer:**
```rust
let net = Net::from_bytes(/* attacker-shipped, 1 live agent at id u32::MAX-1 */)?;
// agents: Vec<Option<Agent>> of length u32::MAX
// count_live_agents iterates all 4 billion slots, returns 1.
let n = net.count_live_agents();   // ~30 seconds on a fast CPU
```

**Why it breaks:** Line 645: `self.agents.iter().filter(|slot| slot.is_some()).count()` is O(arena_len). The doc comment explicitly says "Complexity: O(A) where A is the arena length". Combined with QA-D009-005 (no arena cap on deserialization), an attacker shipping a Net with a few live agents at high IDs forces the receiver into a billion-iteration scan on every `count_live_agents` call (which the reduction engine and protocol layer call repeatedly).

**Fix sketch:** Maintain a `live_count: u32` field on `Net`, updated by `create_agent` (+1 fresh path, no change on recycle), `remove_agent` (-1), and recomputed once on deserialization. `count_live_agents` becomes O(1). Reviewer's NTH-002 (HashSet shadow) is a related optimization but addresses a different path.

Note: this overlaps with QA-D009-005 — the right fix is to bound `agents.len()` at deserialization, after which O(arena_len) is harmless.

---

### QA-D009-013 — `Net::create_agent` and `SparseNet::create_agent` `next_id += 1` panic in debug at u32::MAX, silently wrap in release; ID 0 is re-issued, breaching D4 [MEDIUM]
**File:** `relativist-core/src/net/core.rs:314`, `relativist-core/src/net/sparse.rs:107-108`
**Severity:** MEDIUM
**Category:** Overflow / Invariant

**Reproducer:**
```rust
let mut net = Net::new();
net.next_id = u32::MAX;
// Free-list is empty so we go down the fresh-allocation path:
let id = net.create_agent(Symbol::Era);
// In debug: panics on `self.next_id += 1` overflow.
// In release: id = u32::MAX, next_id wraps to 0.
let id2 = net.create_agent(Symbol::Era);
// id2 = 0, but agents[0] may already exist (if the net was non-empty before).
// Silent ID aliasing → D4 violation.
```

**Why it breaks:**
- `core.rs:314`: `self.next_id += 1` is unchecked.
- `sparse.rs:108`: same pattern.
- The `union` function has a debug-only check (`merged_next_id < u32::MAX`) but `create_agent` itself does not.

While u32::MAX is implausibly large for current scale, the value is reachable through a malformed deserialization (QA-D009-002 surface — a peer ships `next_id = u32::MAX`) or through `union` edge cases.

**Fix sketch:**
```rust
self.next_id = self.next_id.checked_add(1)
    .ok_or(NetError::AgentIdSpaceExhausted)?;
```
Convert `create_agent` to `Result<AgentId, NetError>` (large refactor) OR add a non-debug `assert!` and document the panic.

---

### QA-D009-014 — `SparseNet::write_port` only enforces ERA-arity guard if the agent already exists in `agents`, allowing R17-violating writes if `connect` is called before `create_agent` [LOW]
**File:** `relativist-core/src/net/sparse.rs:382-396`
**Severity:** LOW
**Category:** Logic / Defense in Depth

**Reproducer:**
```rust
let mut sn = SparseNet::new();
// Skip create_agent entirely — directly call connect with an unknown ID:
sn.connect(
    PortRef::AgentPort(99, 1),                         // arbitrary ERA aux port
    PortRef::AgentPort(0, 0),
);
// write_port checks self.agents.get(&id) — returns None for id=99 — falls through
// to the insert call at line 392. The ports HashMap now contains an entry for
// (99, 1) referencing an agent that does not exist.
```

**Why it breaks:** Line 385: `if let Some(agent) = self.agents.get(&id) { ... }`. If the agent does not exist, the ERA guard is skipped and the entry is inserted unconditionally. R17's invariant ("SparseNet MUST NOT store port entries for ERA auxiliary ports") relies on the agent's symbol being known at write time.

This is a low-severity defense-in-depth issue because the production calling pattern always does `create_agent` before `connect`. But `assert_invariants` will then panic in debug builds for the orphaned entry, masking the true root cause (a misordered call sequence).

**Fix sketch:** Add a `debug_assert!(self.agents.contains_key(&id))` at the start of `write_port`, before the symbol lookup, to catch misordered call sequences early.

---

## What was checked but no issue found

- **`Net::union` field handling under D-009 additions** (`core.rs:775-905`): all five new fields (`free_list`, `id_range`, `border_entries_shadow`, `recycle_policy`, `is_in_delta_round`) are explicitly destructured and reset to safe defaults. The `free_list_a` choice (self wins, other discarded) matches the doc comment. Acceptable.
- **R12 protected-tombstone drain in merge** (`merge/core.rs:212-223`): correctly debug-only (`#[cfg(debug_assertions)]`) and idempotent against the existing free_list via `!result.free_list.contains(&id)`.
- **`reconstruct_drain_tombstones`** (`core.rs:556-590`): handles both `border_entries_shadow` (release path) and `protected_tombstones` (debug shadow) correctly. Reviewer NTH-002's O(n²) concern is real but not a correctness issue.
- **`SparseNet::assert_invariants`** (`sparse.rs:338-372`): correctly handles the root-port exception, agent existence, and arity bounds. Tests at lines 696-746 cover the panic paths.
- **R27 family-1 / family-2 / family-3 debug assertions** (`core.rs:466-505, 290-305`): correctly placed at the post-condition moments. The fact they are debug-only is by design (per spec).
- **`PortRef` discriminant ordering** (`net/types.rs:88-93`): manual tagged-byte encoding is stable; `0xFF` reserved for DISCONNECTED, `0x00`/`0x01` for AgentPort/FreePort. No drift introduced by D-009.
- **`PROTOCOL_VERSION = 5` jump from 4** (`protocol/coordinator.rs:182-187`): the version-mismatch path at lines 219-238 cleanly rejects v4. `PREVIOUS_LIVE_VERSION = 4` sentinel test exists (UT-0476-01).
- **Phase A spec text vs Phase B code alignment** for I3', R10b/R10c language: the spec text is faithfully landed in the predecessor specs (modulo MF-001's stale absolute version number).
- **`Net::new` and `Net::with_capacity` initialization of `free_list` and partition fields**: correct; matches R8.
- **CompactSubnet rkyv path**: out of D-009 scope (TASK-0353 from earlier bundle).

---

## Cross-cutting observations

1. **The reviewer's MF-002 (split bypasses sparse path) and this report's QA-D009-001 (CompactSubnet drops free_list) and QA-D009-003 (border_entries_shadow never populated) form a triple compounding gap.** Together they make SPEC-22 in distributed runs functionally a no-op:
   - Sparse path is never taken (MF-002).
   - Free-list never reaches workers across the wire (QA-D009-001).
   - Even if it did, delta-mode protection is inert (QA-D009-003).
   - Stage 6 REFACTOR should treat these three as a single unit of work.
2. **`debug_check_invariants` is documented as called from `reduce_all` but is not** (no callers in `src/reduction/`). The R27 enforcement story is therefore: assertions fire in tests that explicitly call them, never in actual reduction. Consider invoking `debug_check_invariants` at the end of each round in `reduce_all` under `#[cfg(debug_assertions)]`.
3. **R20 spec-vs-code drift**: SPEC-22 R20 (line 142-149) declares `SparseNet::to_dense(&self) -> Net`, while the implementation is `to_dense(&self, id_range: Option<Range<AgentId>>) -> Net`. The spec needs an A-amendment to match the deployed signature, OR the implementation needs to provide a no-arg variant matching the spec. (Reviewer SF-003 covered the GridConfig drift but not this signature drift.)
4. **The reviewer's PASS bullet "`Net::is_behaviorally_equal()` provided and used in regression tests" undercounts the bugs found here** (QA-D009-007, QA-D009-008). The function passes its tests because the tests are constructed within the function's loose semantics. A property-based test (e.g., quickcheck) on `is_behaviorally_equal` reflexivity, symmetry, and transitivity would catch these.

---

## Severity-ordered fix queue (for Stage 6 REFACTOR)

| # | Finding | Severity | Owner | Blocking |
|---|---------|----------|-------|----------|
| 1 | QA-D009-001 (CompactSubnet drops free_list) | CRITICAL | DEVELOPER + CICD (regression test) | Yes |
| 2 | QA-D009-002 (`from_bytes` no validate_free_list) | CRITICAL | DEVELOPER | Yes |
| 3 | QA-D009-003 (border_entries_shadow never populated) | CRITICAL | DEVELOPER | Yes (paired with reviewer MF-002) |
| 4 | QA-D009-004 (merge cross-partition free_list duplicates) | CRITICAL | DEVELOPER | Yes |
| 5 | QA-D009-005 (to_dense unbounded allocation) | CRITICAL | DEVELOPER + ESPECIALISTA EM SPECS | Yes |
| 6 | QA-D009-006 (to_dense panic on inverted range) | HIGH | DEVELOPER | Should fix |
| 7 | QA-D009-007 (is_behaviorally_equal redex multiplicity) | HIGH | DEVELOPER | Should fix |
| 8 | QA-D009-008 (is_behaviorally_equal free_list LIFO) | HIGH | DEVELOPER + ESPECIALISTA EM SPECS | Should fix |
| 9 | QA-D009-009 (build_subnet_sparse drops id_range on empty) | HIGH | DEVELOPER | Should fix |
| 10 | QA-D009-010 (CI unsafe regex too narrow) | MEDIUM | CICD | Discretionary |
| 11 | QA-D009-011 (CI SparseNet import regex too narrow) | MEDIUM | CICD | Discretionary |
| 12 | QA-D009-012 (count_live_agents O(arena_len)) | MEDIUM | DEVELOPER | Discretionary (deferrable to perf bundle) |
| 13 | QA-D009-013 (next_id += 1 overflow) | MEDIUM | DEVELOPER | Discretionary |
| 14 | QA-D009-014 (SparseNet::write_port unguarded) | LOW | DEVELOPER | Discretionary |

---

## Closing

The reviewer's verdict of "PASS WITH NOTES" is over-optimistic for the production deployment surface. The unit-test coverage of *primitives* is solid; the *integration* between primitives and the rest of the system has at least three independent CRITICAL gaps that together render SPEC-22 effectively inert in the distributed run path. Stage 6 REFACTOR should not close D-009 without addressing at least the five CRITICAL findings.

The bundle is, however, low-risk for *isolated* local-mode reduction (single-process, no wire transport), where the free-list works correctly and the sparse path is at least available for testing. v1-feature-complete behaviors are preserved (1450 default / 1493 zero-copy tests pass). v1 floor of 690 is intact.

— QA Stage 5
