# Review: Phase E — Code Quality & Architecture

**Date:** 2026-04-27
**Stage:** 4 (REVIEW) — unified code-quality + architecture review
**Reviewer:** reviewer agent
**Commits:** `fff0f9e` (Wave 1 — TASK-0450, TASK-0451), `a84cb37` (Wave 2 — TASK-0452)
**Files reviewed:**
- `relativist-core/src/merge/types.rs` (Wave 1: +30, Wave 2: −1 whitespace)
- `relativist-core/src/protocol/coordinator.rs` (Wave 1: +162; Wave 2: +238/−66 — almost entirely `cargo fmt`)
- `relativist-core/src/merge/core.rs` (Wave 2: +17 — D3 disjointness assertion)
- `relativist-core/src/partition/helpers.rs` (Wave 2: +12 — D4-elastic `K_eff` self-consistency)
- `relativist-core/src/partition/departure_recovery.rs` (Wave 2: +44 — R24d disjointness + import reorder)
- `relativist-core/src/protocol/retained.rs` (Wave 2: +7 — D5 precondition assertion)

**Code-quality verdict:** NEEDS REFACTORING
**Architecture verdict:** ALIGNED
**Spec compliance:** SPEC-20 R17 (PARTIAL — missing `partition_index` field), R28 (PARTIAL — missing retained-slot field), R38 (COMPLETE — 7 fields, types match), R31 / R39 / D3-elastic / D4-elastic / D5 (PARTIAL — assertions present but several misnamed or non-load-bearing)
**Overall verdict:** **ACCEPT_WITH_FIXES** — 0 CRITICAL, 6 MEDIUM, 4 LOW

---

## 1. Summary

Wave 1 lands the seven `GridMetrics` elastic fields with correct types (`Vec<u32>` × 6 + `Vec<u64>` × 1) and the spec-mandated R38a non-collision audit committed as a comment block. R17 and R28 logging is wired in five departure paths plus the join-success path, all using `tracing::info!`/`tracing::warn!` (no `println!`). Wave 2 lands four `debug_assert!`/`#[cfg(debug_assertions)]` blocks across `merge/core.rs`, `partition/helpers.rs`, `partition/departure_recovery.rs`, and `protocol/retained.rs`. The four core files compile cleanly; module boundaries are respected; the v1 floor (690) and the v2 baseline (1256/1299) are reportedly preserved.

However, the implementation falls short of the spec contract in three concrete ways and has one significant integrity issue:

1. **R17 omits `partition_index`** (spec MUST: `WorkerId`, `K_eff_new`, `partition_index`, round). The new joiner's `partition_index` is computable on the spot (`active_ids.len()` at insert time, accounting for hybrid offset) but is not emitted.
2. **R28 omits the retained-slot field** (spec MUST: `WorkerId`, departure type, round, and *which retained slot* — `retained_initial` | `retained_last_acked` — was consumed). All four `tracing::warn!` sites lack this field.
3. **D4-elastic assertion is tautological.** In `partition/helpers.rs:89-99`, the `expected_k_eff` is computed from the *same* expression as `k_eff` two lines above (`active_workers.len() as u32 + (1 if hybrid)`). The assertion `k_eff == expected_k_eff` cannot fail by construction; it is a dead-code defense. The intent of D4-elastic per R11a is to catch a *positional* disagreement between `partition_index` allocation and slot count — which this assertion does not exercise.
4. **The `retained_initial_reclaims_per_round` counter is unreachable.** The reclaim branch in `coordinator.rs:767-833` increments `_round_reclaimed_initial` and then unconditionally `return`s an error (line 832: `"Departure recovery reconstruction succeeded but stream management is TASK-0443 follow-up"`). The counter is therefore *only* pushed in the no-departure branch (where it is always `0`). This is a known gap from the underlying TASK-0443, but it means `retained_initial_reclaims_per_round` is dead-on-arrival until that gap closes — TEST-SPEC-0450 UT-0450-06 cannot pass against the real coordinator path.

The Wave 2 commit message claims "+238 LoC in coordinator.rs — heavy invariant assertions integration." This is misleading. The actual semantic addition in `coordinator.rs` for Wave 2 is zero (no `assert!` or `debug_assert!` was added there); the +238/−66 delta is entirely `cargo fmt` reformatting that should have been a separate commit. The four real assertions all live in the four other files.

The four checks land are otherwise clean Rust: no `unwrap()` introduced in production paths (the `self_partition.as_ref().unwrap()` at line 611 is pre-existing, not new), no `unsafe`, no `println!`. Module boundaries are respected.

---

## 2. Findings

### Must-Fix (MEDIUM)

#### MF-001 — R17 INFO log omits `partition_index` (spec MUST field)

**Category:** Spec Violation
**Principle/Spec:** SPEC-20 R17, SPEC-11 R28 (OTel attribute completeness)
**File:** `relativist-core/src/protocol/coordinator.rs:937-945`

**Problem:** SPEC-20 R17 explicitly enumerates four required fields on the join INFO log: `K_eff_new`, joining worker's `WorkerId`, `partition_index` it will occupy in the next round, and the round number. The current implementation emits only three (`worker_id`, `k_eff_new`, `round`) — `partition_index` is missing.

**Before:**
```rust
let k_eff_new = worker_streams.len()
    + if grid_config.hybrid_coordinator { 1 } else { 0 };
tracing::info!(
    worker_id,
    k_eff_new,
    round = metrics.rounds,
    "Worker joined the grid (R17)"
);
```

**After:**
```rust
let offset = if grid_config.hybrid_coordinator { 1 } else { 0 };
let partition_index = (worker_streams.len() as u32 - 1) + offset;
let k_eff_new = worker_streams.len() + offset as usize;
tracing::info!(
    worker_id,
    partition_index,
    k_eff_new,
    first_participating_round = metrics.rounds,
    "Worker joined the grid (R17)"
);
```

**Why:** SPEC-20 R17 enumerates the field list authoritatively. Log analyzers downstream of NF-008 / SPEC-11 R28 OTel attribute mapping will index on `partition_index`; missing it forces the analyzer to recompute from `WorkerId` ordering, undermining the whole reason it was decoupled from `WorkerId` in R11a / D4-elastic.

---

#### MF-002 — R28 WARN log omits "retained slot consumed" field at all four call sites

**Category:** Spec Violation
**Principle/Spec:** SPEC-20 R28
**File:** `relativist-core/src/protocol/coordinator.rs:672-682, 698-710, 721-735, 738-755`

**Problem:** SPEC-20 R28 enumerates four required fields on the departure WARN log: `WorkerId`, departure type (`timeout | connection_loss | leave_after_result | leave_urgent`), round number, and "which retained slot (`retained_initial` or `retained_last_acked`) was consumed." The implementation omits the retained-slot field at all four warn sites. Furthermore, the `LeaveRequest` site emits `?kind` (a `Debug` repr of `LeaveKind`) instead of one of the four canonical strings R28 enumerates — `LeaveKind::AfterResult` does not stringify to `leave_after_result`.

The reclaim path itself happens *later* in the function (`coordinator.rs:767+`); the slot consumed is implicit from the SPEC-20 R24a/R24b decision tree. Today it is always R24a (conservative, `retained_initial`) because the optimized R24b path is not yet implemented. Therefore the canonical departure-type-to-slot mapping is currently fixed: all four sites consume `retained_initial`.

**Before (LeaveRequest site, line 672–682):**
```rust
Message::LeaveRequest { kind } => {
    tracing::warn!(
        worker_id = wid,
        round = metrics.rounds,
        ?kind,
        "Worker left gracefully via LeaveRequest (R28)"
    );
    let _ = send_frame(stream, &Message::LeaveAck).await;
    departing_worker_ids.push(wid);
    round_departed_count += 1;
}
```

**After:**
```rust
Message::LeaveRequest { kind } => {
    let departure_type = match kind {
        LeaveKind::AfterResult => "leave_after_result",
        LeaveKind::Urgent      => "leave_urgent",
    };
    tracing::warn!(
        worker_id = wid,
        round = metrics.rounds,
        departure_type,
        retained_slot = "retained_initial",
        "Worker left gracefully via LeaveRequest (R28)"
    );
    let _ = send_frame(stream, &Message::LeaveAck).await;
    departing_worker_ids.push(wid);
    round_departed_count += 1;
}
```

For the other three sites, `departure_type = "connection_loss"` (the `Message::Error` arm and the `Ok(Err(e))` arm) and `departure_type = "timeout"` (the `Err(_)` timeout arm). All three should emit `retained_slot = "retained_initial"` until R24b lands.

**Why:** R28 explicitly enumerates the four field values. `?kind` is implementation-private debug formatting and will silently change format if `LeaveKind` is ever moved or augmented. SPEC-11 R28 OTel attribute mapping requires a stable string-typed `departure_type` field. The retained-slot field is a future-tracking signal: when R24b lands, log analyzers must distinguish R24a from R24b reclaims; emitting the field as a literal `"retained_initial"` today closes that schema gap without code churn at the R24b landing point.

---

#### MF-003 — D4-elastic assertion is tautological — does not exercise the invariant

**Category:** Spec Violation (defense does not defend the named property)
**Principle/Spec:** SPEC-20 R11a / D4-elastic, TASK-0452 acceptance criterion 2
**File:** `relativist-core/src/partition/helpers.rs:86-99`

**Problem:** The asserted equality cannot fail because `expected_k_eff` is computed by the same expression as `k_eff` two lines above, with identical operands:

```rust
let k = active_workers.len() as u32;
let k_eff = k + if config.hybrid_coordinator { 1 } else { 0 };

#[cfg(debug_assertions)]
{
    let expected_k_eff =
        active_workers.len() as u32 + if config.hybrid_coordinator { 1 } else { 0 };
    assert_eq!(
        k_eff, expected_k_eff,
        "D4 violated: K_eff calculation mismatch"
    );
}
```

This compiles to a comparison of two values produced by identical code — it is dead-code defense. TASK-0452 asks for a real D4-elastic check: that *every consumed `partition_index` is in `[0, K_eff)` and dense*, i.e., the resulting `HashMap<WorkerId, IdRange>` has exactly `K_eff` entries with the expected indices. R11a (the source of D4-elastic) is explicitly about the partition_index *allocation* matching `[0, K_eff)`.

**After:** Move the assertion to *after* the `map` is built, and assert the actual D4-elastic property:
```rust
debug_assert_eq!(
    map.len() as u32,
    k_eff,
    "D4-elastic violated: |map| = {} but K_eff = {}",
    map.len(),
    k_eff
);
#[cfg(debug_assertions)]
for (&wid, range) in &map {
    debug_assert!(
        range.start < range.end,
        "D4-elastic violated: degenerate IdRange for worker {}: {:?}",
        wid,
        range
    );
}
```

**Why:** The defense as written cannot fail; the defense as proposed catches a real class of bug — e.g., if hybrid mode is true but the self-partition entry is accidentally not inserted at `WorkerId 0` (which would produce a `map.len() == k_eff - 1`). This is precisely what D4-elastic is meant to detect.

---

#### MF-004 — `_round_reclaimed_initial` counter is unreachable in the reclaim path

**Category:** Spec Violation (R38 metric is dead code) / Test Coverage
**Principle/Spec:** SPEC-20 R38 (`retained_initial_reclaims_per_round`), TEST-SPEC-0450 UT-0450-06
**File:** `relativist-core/src/protocol/coordinator.rs:622-832, 835-846`

**Problem:** The reclaim branch increments `_round_reclaimed_initial` at line 828 and then unconditionally returns an error at line 832 (`"Departure recovery reconstruction succeeded but stream management is TASK-0443 follow-up"`). Therefore the metric push at line 841 only ever runs in the *no-departure* branch, where the counter is always `0`. Symmetrically, `round_reclaimed_last_acked` is bound at line 623 to `0` and never reassigned; it pushes `0` every round.

This means:
- `metrics.retained_initial_reclaims_per_round` is dead-on-arrival — always pushes `0` for any non-error round.
- `metrics.retained_last_acked_reclaims_per_round` is dead-on-arrival — always pushes `0`.
- TEST-SPEC-0450 UT-0450-06 (`retained_initial_reclaims_increment_on_r24a`) cannot pass against the real coordinator path; it can only pass against a stub.

The underlying limitation comes from TASK-0443 (the unconditional early return), which is acknowledged in the task contract as a follow-up. But TASK-0450's own R38 contract is therefore not closed today.

**After (minimum):** Add a `// FIXME(TASK-0443): retained_initial_reclaims_per_round / retained_last_acked_reclaims_per_round are unreachable today because of the early return below. Until TASK-0443 wires reclaim back into the round loop, these counters always push 0.` comment at the metric-push site (line 839), AND raise a corresponding entry in `docs/next-steps.md`. Also remove `let _ = _round_reclaimed_initial;` (line 829) — that "use the variable" silence-the-warning idiom telegraphs that the value is going to be discarded; clippy already accepts the leading underscore.

**Why:** The R38 fields are landed but inert. Benchmarks (EG-B1, EG-B2, EG-B3) that consume them per the test spec will silently report `0` reclaims regardless of churn, leading to misinterpretation of the break-even analysis. A `// FIXME` plus an entry in `next-steps.md` makes the gap visible.

---

#### MF-005 — R17 round number ambiguity: log emits `metrics.rounds` (current) but spec says "round at which it will *first participate*"

**Category:** Spec Violation (off-by-one)
**Principle/Spec:** SPEC-20 R17 ("the round number at which it will first participate")
**File:** `relativist-core/src/protocol/coordinator.rs:943`

**Problem:** R17 specifies "the round number at which [the joining worker] will first participate." The join window runs *after* the merge of round N (lines 901–960), and `metrics.rounds` is incremented at line 899 *before* the join window (`metrics.rounds += 1;`). So at the point of the log, `metrics.rounds` is already `N+1` — the round about to start is `N+1`, in which the new worker will participate. This appears correct by accident of placement: `metrics.rounds + 1` would be wrong here.

However, this depends on a non-obvious ordering: the `metrics.rounds += 1` at line 899 *just before* the join window is what makes `metrics.rounds` "the next round" rather than "the current round." If a future refactor moves the increment, R17's semantic shifts silently.

**After:** Either rename the field to make the semantics explicit, or add a comment:
```rust
tracing::info!(
    worker_id,
    k_eff_new,
    first_participating_round = metrics.rounds,
    "Worker joined the grid (R17)"
);
```

**Why:** Field renames are cheap; the cost of a silent semantic drift after a future refactor is high. SPEC-11 OTel mapping benefits from a self-documenting field name.

---

#### MF-006 — D3 disjointness assertion in `merge/core.rs:48` uses bare `assert!` instead of `debug_assert!`

**Category:** Code Quality / Spec Compliance
**Principle/Spec:** TASK-0452 acceptance criterion ("Release builds strip assertions (standard Rust `debug_assert!` behavior)"), Phase A precedent (`merge/grid.rs::reconstruct` uses bare `assert!` for release-safety-critical ID overlap)
**File:** `relativist-core/src/merge/core.rs:43-55`

**Problem:** The block is gated by `#[cfg(debug_assertions)]` and uses bare `assert!`. The pattern is correct for the goal but unidiomatic and inconsistent with TASK-0452's stated mechanism (`debug_assert!`). Same issue at `partition/departure_recovery.rs:30-40`. The mixing of `#[cfg(debug_assertions)] { … assert!(…) … }` (used in `core.rs`, `helpers.rs`, `departure_recovery.rs`) with `debug_assert!` (used in `retained.rs`) is not a bug but inconsistent.

Compare to the Phase A precedent: `merge/grid.rs::reconstruct` uses bare `assert!` *without* a cfg gate because the ID-range overlap there is release-safety-critical. Wave 2's D3 site is the same kind of merge precondition — a violation produces a malformed `Net`. There is a defensible argument to drop the `#[cfg(debug_assertions)]` gate at `core.rs:43` and let the `assert!` fire in release builds too, matching `reconstruct`.

**After (option B — match TASK-0452 verbatim, debug-only):**
```rust
let mut sorted_ranges: Vec<_> = partitions.iter().map(|p| p.id_range).collect();
sorted_ranges.sort_by_key(|r| r.start);
for pair in sorted_ranges.windows(2) {
    debug_assert!(
        pair[0].end <= pair[1].start,
        "D3 violated: overlapping ID ranges in merge: {:?} vs {:?}",
        pair[0],
        pair[1]
    );
}
```

Pick one. The ESPECIALISTA EM SPECS should adjudicate D3-elastic's release-safety bar; the test spec frames it as defensive against developer error in DEV, suggesting B. The Phase A precedent suggests A. Either way, do not leave both styles in the same bundle. Apply the same fix to `partition/departure_recovery.rs:30-40` (option B is more consistent with the TASK-0452 contract).

**Why:** Consistency. The `#[cfg(debug_assertions)] { … assert! … }` pattern is correct but obscures intent — a reader wonders whether `debug_assert!` was rejected for a reason. Picking one form and using it everywhere documents the choice.

---

### Should-Fix (LOW)

#### SF-001 — `let _ = _round_reclaimed_initial;` at line 829 is a smell

**Category:** Code Quality (dead idiom)
**File:** `relativist-core/src/protocol/coordinator.rs:828-829`

The leading underscore in `_round_reclaimed_initial` already silences the unused-variable warning; the explicit `let _ = …` is redundant *and* misleading (it suggests intentional drop). The variable's value flows into the metric push at line 841 in the no-departure branch, so the `+=` is also dead in the early-return path. Resolving MF-004 likely removes this idiom organically.

**After:** Delete line 829.

---

#### SF-002 — Variable naming inconsistency: `_round_reclaimed_initial` vs `round_reclaimed_last_acked`

**Category:** Code Quality (naming)
**File:** `relativist-core/src/protocol/coordinator.rs:622-624`

The leading underscore on the first variable signals "this may be unused"; the second variable (`round_reclaimed_last_acked`) has no leading underscore but is also assigned `0` and never reassigned. They are both "unused except in the metrics push" and should follow the same convention.

**After:** Either remove both leading underscores and rely on the future write that wires reclaim metrics into the loop (alongside MF-004), or — pragmatically for today — replace both `let` bindings with `0u32` literals at the metric push sites and add a `// TODO(TASK-0443): wire reclaim counts here` comment.

---

#### SF-003 — No regression test exercises any of the seven new GridMetrics fields end-to-end

**Category:** Test Coverage
**Spec/Task:** TEST-SPEC-0450 UT-0450-01..13

A grep for `workers_joined_per_round` (and the six sibling fields) finds them only in `merge/types.rs` (the definitions) and `coordinator.rs` (the writes). No test asserts that any of them grow, that the lengths agree across rounds, or that the per-round invariant holds. TEST-SPEC-0450 enumerates UT-0450-01 through UT-0450-13 with a mandatory `nf_004_field_name_disjointness_with_spec19_r45` test (UT-0450-10) — none of these are present. A `cargo test` regression that introduces a typo on a field name (e.g., renaming `workers_joined_per_round` to `workers_joined`) would not fail any test.

This is the same lesson Phase A QA recorded: positional/field-name discriminants need pinning tests; invariants need targeted regression tests. `debug_assert!`s exercised opportunistically through unrelated test paths are not equivalent.

**After:** Open a follow-up test task (or fold into TASK-0455 / Phase E completion) for at least:
- UT-0450-01 (struct has all 7 fields) — compiles to a Default + field-name check
- UT-0450-10 (R45 disjointness, the NF-004 anchor)
- UT-0450-11 (serde roundtrip — note: blocked because `GridMetrics` only derives `Serialize`, not `Deserialize`; either add `Deserialize` or relax UT-0450-11 to "serializes without panic")

---

#### SF-004 — `bytes_received` is not segmented by message type in the metric push

**Category:** Code Quality (consistency)
**File:** `relativist-core/src/protocol/coordinator.rs:649-720`

Each `Ok(Ok((msg, nbytes)))` branch attributes `nbytes` to `bytes_received`. The `LeaveRequest` and `Error` arms record the `nbytes` but the byte attribution is not segmented: a `LeaveRequest` byte is counted as a "result byte" in `bytes_received_per_round`. In the SPEC-20 churn benchmarks this distorts the per-result byte budget. Not a bug for v1; flag it as a benchmark-affecting note.

---

## 3. Passed Checks

- [x] No `unwrap()` introduced in production paths (the one at coordinator.rs:611 is pre-existing)
- [x] No `unsafe` blocks
- [x] No `println!` / `eprintln!` — `tracing::info!` and `tracing::warn!` only
- [x] `tracing` is unconditionally available; the new logs compile in all configurations
- [x] All 7 new `GridMetrics` fields have correct types per SPEC-20 R38: 6 × `Vec<u32>` + 1 × `Vec<u64>`
- [x] All 7 new fields have `///` Rustdoc comments
- [x] `GridMetrics` `Default + Clone + Debug + serde::Serialize` derives still apply
- [x] R38a NF-004 audit comment block is committed at `merge/types.rs:107-114`
- [x] R38b `is_coordinator_self: bool` already exists on `WorkerRoundStats` (TASK-0420)
- [x] D3-elastic assertion (merge/core.rs) exercises a real property (sorted ranges, no overlap) — modulo MF-006 style choice
- [x] R24d assertion (departure_recovery.rs) exercises a real property (no remapped overlap with prior reclaims)
- [x] D5 assertion (retained.rs) is genuinely load-bearing: `refresh_last_acked` requires `initial[w]` to exist
- [x] Module boundaries respected: no `merge -> protocol` inversions
- [x] Wave 2 assertions all gated by `#[cfg(debug_assertions)]` or `debug_assert!`
- [x] No new error variants; `thiserror` chain unaffected
- [ ] R17 emits `partition_index` (MF-001: missing)
- [ ] R28 emits `retained_slot` (MF-002: missing at all 4 sites)
- [ ] R28 emits departure_type as one of the four canonical strings (MF-002: emits `?kind` debug fmt)
- [ ] D4-elastic assertion exercises a non-tautological property (MF-003: tautological)
- [ ] `_round_reclaimed_initial` reaches the metric push (MF-004: blocked by early return)
- [ ] Wave 2 assertion style is consistent across all four files (MF-006: mixed)
- [ ] Tests exist for any of the 7 new GridMetrics fields (SF-003: none)

---

## 4. Wire-Serde Regression Assessment

`GridMetrics` derives `Default, Clone, Debug, serde::Serialize` only — no `Deserialize`, no `bincode`-specific derive. This was the case before Wave 1 and remains so after. The 7 new fields are all `Vec<u32>` / `Vec<u64>` which serialize cleanly via serde. No wire-format regression.

`Message`, `Partition`, `LeaveKind`, `RetainedInitial`, `RetainedLastAcked`, and `RetainedStateRegistry` are unchanged in derive surface. No new variant added; no positional discriminant churn. Wave 2 assertions are observation-only and have no wire impact.

UT-0450-11's `bincode::serialize → deserialize` round-trip is *not* directly testable today because `GridMetrics` lacks `Deserialize`. This is a pre-existing gap, not a Wave 1 regression.

---

## 5. Wave 2 Coordinator Diff Note (informational)

The Wave 2 commit message states "+238 LoC in coordinator.rs — heavy invariant assertions integration." The diff confirms this is `cargo fmt` reformatting (single-line statements broken across multiple lines, plus four blank-line removals), with **zero** semantic invariant additions in `coordinator.rs`. The actual Wave 2 invariants are in `merge/core.rs` (+17), `partition/helpers.rs` (+12), `partition/departure_recovery.rs` (+44 of which most is also `cargo fmt`), and `protocol/retained.rs` (+7).

This is not a code-quality issue per se, but the commit message is misleading. For future bundles: `cargo fmt`-only changes should land in their own commit, separate from the semantic change.

---

## 6. Stage 6 Action List (for Developer after Stage 5 QA)

| # | Severity | Action | File | Lines |
|---|----------|--------|------|-------|
| 1 | MEDIUM (MF-001) | Add `partition_index` field to R17 INFO log | `coordinator.rs` | 937–945 |
| 2 | MEDIUM (MF-002) | Replace `?kind` with canonical `departure_type` string + add `retained_slot = "retained_initial"` at all 4 R28 warn sites | `coordinator.rs` | 672–682, 698–710, 721–735, 738–755 |
| 3 | MEDIUM (MF-003) | Replace tautological D4-elastic assertion with a real `partition_index` density check (post-`map`-build) | `partition/helpers.rs` | 86–99 |
| 4 | MEDIUM (MF-004) | Add `// FIXME(TASK-0443)` at the metrics push site; raise an entry in `docs/next-steps.md` | `coordinator.rs` | 828–841 + `docs/next-steps.md` |
| 5 | MEDIUM (MF-005) | Rename `round = metrics.rounds` to `first_participating_round = metrics.rounds` in R17 log | `coordinator.rs` | 943 |
| 6 | MEDIUM (MF-006) | Pick one assertion style and apply consistently across all 4 Wave 2 sites | `merge/core.rs:43–55`, `partition/departure_recovery.rs:30–40`, `partition/helpers.rs:89–99`, `protocol/retained.rs:56–62` |
| 7 | LOW (SF-001) | Remove `let _ = _round_reclaimed_initial;` | `coordinator.rs` | 829 |
| 8 | LOW (SF-002) | Normalize naming of the three `round_*` counters | `coordinator.rs` | 622–624 |
| 9 | LOW (SF-003) | Open a follow-up test task to land UT-0450-01, UT-0450-10, etc. | `relativist-core/tests/` (new file) |
| 10 | LOW (SF-004) | Note `bytes_received` byte-attribution behavior in a comment | `coordinator.rs` | 648–720 |

Items 1–6 (MEDIUM) MUST be resolved before this bundle is considered complete. Items 7–10 (LOW) SHOULD be resolved in the same pass.

---

## 7. Stage 5 QA Readiness

**Conditional.** The implementation compiles, the assertions are wired, and the tests reportedly pass at 1256/1299. However, the spec contract for R17 and R28 is not fully met (MF-001, MF-002), which means QA will likely write a test against the documented field list and find it failing. MF-003 (tautological D4 assertion) will not surface in QA — by definition the assertion cannot fail — but it leaves the named invariant un-defended; a future SPEC-20 violation that D4-elastic is supposed to catch will not panic.

**Recommended path:** Developer applies MF-001, MF-002, MF-003 (and ideally MF-006) before Stage 5. MF-004 / MF-005 can be deferred to Stage 6 alongside the LOW items.

---

**Phase E: 6 Must-Fix, 4 Should-Fix, verdict: ACCEPT_WITH_FIXES**
