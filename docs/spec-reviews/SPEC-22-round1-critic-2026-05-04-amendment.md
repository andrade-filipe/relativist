# SPEC-22 v2.3 D-011 Amendment — Round 1: Spec Critic Review

**Date:** 2026-05-04
**Reviewer:** Spec Critic (adversarial)
**Target:** SPEC-22-arena-management.md (status: Reviewed v2.3 — D-011 Amendment 2026-05-04). Surface under review: §3.2 R22 (modified), §3.2 R22a (new), §3.4 R30 (modified), §11 Change Log entry "D-011 Amendment — 2026-05-04". Cross-ref propagation in SPEC-04 §4.5.1 A7 and SPEC-21 §3.3 / §4.9 (5 spots).
**Predecessors consulted:** SPEC-00, SPEC-01 (T7, I3', D4), SPEC-02 (R2/R10/R11/R12 post-A2..A5), SPEC-03 (post-A6), SPEC-04 §4.5.1 (post-A7), SPEC-05 §4.2 (post-A8), SPEC-18 R28 (post-A9), SPEC-19 §3.2 (post-A10).
**Code under review:** `relativist-core/src/partition/helpers.rs:280-477` (specifically lines 297-303 dense allocation, 351-359 free-list scan, 422-477 `build_subnet_with_config` — the call site that still carries the OLD metric `id_range_size > 4 * live_count`, deferred to TASK-0612).
**Closure log consulted:** `docs/spec-reviews/SPEC-22-amendment-2026-05-04-d011-blocker.md`.
**Handoff brief consulted:** `docs/handoffs/2026-05-04-SPEC-22-R22-R30-metric-amendment-handoff.md`.

---

## Overall Assessment

The amendment does what it claims at the surface level: the new metric `effective_arena_size := max_live_id + 1` exactly matches the dense `build_subnet`'s `agents_len = max_id + 1` allocation at `helpers.rs:301-303`, and the worked example in R22a is arithmetically sound (100_000_001 > 40_000_000 → SPARSE, preserving the M5 guard). The T7 determinism note is correct (`max()` is order-independent on `&[u32]`). Empty-partition handling is consistent across R22 ("`worker_agents` is empty ⇒ `effective_arena_size := 0`"), R22a, and `helpers.rs:297-299`.

That said, the surface IS small but it touches a build-time gate that controls which representation is used; mis-stating the metric a second time would either re-introduce the perf regression or open a memory pathology. Three substantive concerns remain after adversarial review:

1. **The R22 "MAY" clause is dangerously permissive post-correction.** The spec still says "Otherwise, `build_subnet` MAY use the existing dense path or the sparse-then-dense path." With the corrected metric, this clause now permits — by the spec's own letter — the perf regression to recur if any future implementation chooses the sparse-then-dense path when threshold is NOT exceeded. The implementation in `helpers.rs:447-475` correctly narrows to "sparse only when threshold exceeded", but the spec does not enforce this narrowing.

2. **R22 is silent on whether the metric is build-time-only.** The pre-empted attack-vector Q2 in the closure log answers this ("post-build agent creation does NOT re-trigger the check"), but R22 itself does not say so. An implementer reading R22 could reasonably interpret the threshold as a runtime invariant maintained across reduction. Under CON-DUP-heavy workloads the dense arena grows past `max_live_id + 1` (workers `create_agent` from `next_id`), and the threshold's protective property degrades — without the spec being explicit that this is intentional and out-of-scope for R22.

3. **R22a's claim "M5 pathology manifests as `max_live_id ≫ live_count`" is asserted but not derived.** The closure log §5 acknowledges this is a question the spec-critic should pressure-test. Under the only fragmentation source documented in the SPEC-22 design rationale — recycled-id fragmentation under delta-mode reduction — the high-numbered slot pattern arises because the worker's `next_id` advances during reduction past `max_live_id` and consumed slots in low IDs go to the free-list. But `max_live_id` is taken from `worker_agents` at split time (the input to `build_subnet`), NOT from the worker's previous-round `next_id`. R22a conflates "max_live_id grows" (true within a single worker's local arena across rounds) with "max_live_id at the next split's `worker_agents` is high" (true only if those high-numbered live agents are re-assigned to this partition by `sigma`). The path from "fragmentation during reduction" to "`max_live_id` of the NEXT `build_subnet` call's `worker_agents`" is real but indirect; R22a should make it explicit, or downgrade the M5-still-detected claim to a streaming/repartition context only.

A fourth concern is R30's normative-vs-implementation timing: R30 specifies the error-variant field rename to `effective_arena_size` as SHOULD, but the code still carries `id_range_size` (TASK-0612 deferred). This produces brief but real spec/code drift. The closure log handles this as expected, but R30's wording should mark the rename as MUST conditional on TASK-0612 landing, not SHOULD.

Beyond these, the amendment is consistent with predecessor specs, preserves T1-T7 / D1-D6 / I1-I7 / G1, and the cross-references in SPEC-04 §4.5.1 A7 and SPEC-21 (5 spots) are propagated correctly.

**Verdict:** **NEEDS_REVISION** — the corrective intent is sound but R22's `MAY` clause and silent build-time scope, plus R22a's under-derived M5 claim, are gaps an adversarial reader can exercise. None block immediate dispatch of TASK-0612 (the perf fix); all should be closed before SPEC-22 reaches Reviewed v2.4.

---

## Issues

### SC-022: R22 "MAY" clause permits the just-fixed perf regression to recur

**Severity:** HIGH
**Axis:** Consistency / Testability
**Section:** §3.2 R22
**Requirement:** R22

**Problem:**
The amendment leaves intact the bullet "Otherwise, `build_subnet` MAY use the existing dense path or the sparse-then-dense path; the choice is a `PartitionConfig.sparse_build: bool` flag with default `true` (R30)."

After the metric correction, the SPARSE path is fast in absolute terms ONLY when the dense path is pathological. When the threshold is not exceeded, sparse-then-dense incurs:
- One `SparseNet` HashMap-build pass O(live_count) with hash overhead
- One `to_dense(Some(id_range))` materialization O(max_id) + O(live_count)

versus the dense direct-build O(arena_len) + O(live_count) which has better constant factors. Section §5.2 of SPEC-22 itself argues this point in justifying that SparseNet is NOT for the reduction hot path; the same constant-factor argument applies to construction.

The bisect transcript (closure log §1) shows that even WITH the metric fix, choosing SPARSE on a healthy workload added +63% wall-clock to partition+merge (~13.8 s out of 22 s on `ep_con 5M w=2`). The current implementation in `helpers.rs:447-475` correctly bypasses SPARSE when threshold is not exceeded, but R22's spec text does not require this bypass. A future implementer reading R22 could legitimately set the SPARSE path under the `sparse_build = true` default even when threshold is NOT exceeded — re-introducing the regression.

**Impact if unresolved:**
- Spec drift between R22 and the perf-fix implementation: the fix lives in code only, with no normative basis.
- A future refactor that "simplifies" `build_subnet_with_config` by always taking the SPARSE branch would be SPEC-22-compliant but performance-regressed.
- Tests written against R22 cannot detect this drift because R22 permits the regression by letter.

**Suggested resolution:**
Tighten the second bullet of R22 to forbid the SPARSE path below threshold:

> Otherwise (i.e., `effective_arena_size <= 4 × partition.live_agent_count`), `build_subnet` MUST use the dense path. The SPARSE path is forbidden when threshold is not exceeded, regardless of `PartitionConfig.sparse_build`. The `sparse_build = false` flag is honored only as a rejection signal: when `sparse_build = false` AND threshold IS exceeded, `build_subnet` MUST return `PartitionError::DenseAllocationExceedsThreshold` per R30.

This aligns the spec with the BLOCKER 2026-05-04 evidence base (the SPARSE path is slow in absolute terms on healthy workloads) and removes the spec-permitted re-regression vector.

---

### SC-023: R22 does not state that the metric is build-time-only

**Severity:** MEDIUM
**Axis:** Completeness / Testability
**Section:** §3.2 R22
**Requirement:** R22, R22a

**Problem:**
R22 defines `effective_arena_size := max_live_id + 1` where `max_live_id := worker_agents.iter().copied().max()`. This computation runs once at `build_subnet` entry. During subsequent reduction, the dense arena grows past `max_live_id + 1` whenever CON-DUP fires and the free-list is empty (e.g., expansion-dominated workloads). The spec is silent on this point.

Closure log §5 ("Pre-empted attack vectors" Q2) does answer it:
> Q2 — Workloads where `max_live_id ≪ id_range.end` AND in-round `create_agent` calls push past `4 × live`? `create_agent` allocates from `next_id` upward within the partition's `id_range`; this happens DURING reduction, AFTER `build_subnet`. The threshold check is build-time. Post-build agent creation does NOT re-trigger the check.

But this scope qualification is in the closure log, NOT in the normative spec text. An implementer or a test author reading R22 alone cannot infer that "post-build growth is intentionally unprotected by R22." That is a load-bearing assumption — the threshold's purpose is build-time arena sizing, not runtime memory protection.

**Impact if unresolved:**
- Tests of R22 may incorrectly fail/pass based on whether the test runs reduction after `build_subnet` and then re-checks the threshold.
- A future spec-author may add a post-build re-check under the assumption that R22 demands one, complicating the hot path needlessly.
- The R22a claim "M5 pathology is still detected" is partially weakened: it is detected at re-partition / streaming-finalize time (where `max_live_id` of the live worker_agents is high after fragmentation), but NOT during a single round's local reduction — which is fine, but should be explicit.

**Suggested resolution:**
Add a single normative bullet to R22 immediately after the empty-partition convention:

> **Build-time only.** The metric `effective_arena_size > 4 × live_agent_count` is evaluated exactly once, at `build_subnet` entry. Subsequent `create_agent` calls during local reduction MAY grow the arena past `max_live_id + 1` (e.g., CON-DUP commutation when free-list is empty). R22 imposes no post-build re-check; arena growth during reduction is governed by SPEC-02 R11 and SPEC-22 R3 (free-list-first allocation), not by the R22 threshold.

This closes the scope ambiguity without weakening the M5 protection (M5 fragmentation manifests at the NEXT split, where `max_live_id` of the worker's now-fragmented live set is high).

---

### SC-024: R22a's M5-detection claim is asserted, not derived from the SC-009 evidence base

**Severity:** MEDIUM
**Axis:** Completeness / Consistency
**Section:** §3.2 R22a
**Requirement:** R22a

**Problem:**
R22a states: "Under M5 fragmentation, although `live_count` may remain modest (e.g., 10M live agents per partition), the `max_live_id` in `worker_agents` grows because consumed-then-recycled IDs leave high-numbered slots occupied while low-numbered slots churn through `None`/`Some` cycles."

The closure log §5 (closure rationale) admits this needs adversarial pressure-testing:
> An adversarial reader should pressure-test the worked example (10M live, 100M max_live_id ⇒ 100_000_001 > 40_000_000) against the original SC-009 evidence base (HVM2 / AC-006 — was the original "200K agents at 800 MiB" pathology really `max_live_id`-driven, or could there be edge cases where `id_range`-style fragmentation matters?).

Two problems with R22a's derivation:

1. **`max_live_id` source.** `max_live_id` is taken from `worker_agents`, the input to `build_subnet`. `worker_agents` is the OUTPUT of `split_with_config` for partition i, derived from `sigma`. Under streaming/repartition, the worker's previous-round `next_id` (which can be high after fragmentation) does NOT directly become `max_live_id`; it becomes `max_live_id` only if the partitioner re-assigns those high-numbered live agents to the same worker. This indirection is real but unstated.

2. **Fragmentation model.** R22a's mental model — "consumed-then-recycled IDs leave high-numbered slots occupied while low-numbered slots churn" — is backwards relative to the free-list LIFO ordering documented in §4.7. The free-list pops the most recently freed (LIFO), so under steady-state recycling the LOW-numbered slots get reused first, not last. The pattern that produces `max_live_id ≫ live_count` is ONE-WAY DRIFT (creation-heavy phases that bump `next_id` past consumed-and-recycled IDs), not "low-numbered slots churn." The R22a wording suggests a different mechanism than the actual one.

The original SC-009 evidence base (HVM2 / AC-006 — closure log §5 explicitly invites this check) describes "200K agents at 800 MiB" — which is `200_000 × ~4 KB/agent` arena, suggesting the pathology was driven by arena pre-sizing to a high upper bound (i.e., `id_range.end` in the OLD metric language, or `max_live_id + 1` if those high-numbered IDs were live). If HVM2's pathology was driven by allocator-reserved high IDs that were NEVER actually populated, then the new metric `max_live_id + 1` may UNDER-count vs the old `id_range`, and the M5-protection claim is over-stated.

**Impact if unresolved:**
- The "M5 pathology is still detected" claim is the load-bearing rationale for the corrective amendment; if it does not hold across all M5 scenarios, the amendment trades one pathology for another.
- Tests targeting M5 fragmentation may fail to exercise the actual mechanism R22a describes.
- The amendment's audit trail in §11 cites SC-009's HVM2 evidence by reference but does not verify that the new metric covers the SAME evidence base.

**Suggested resolution:**
Replace R22a's asserted derivation with one of:

(a) **Explicit fragmentation model:** State precisely the scenario where `max_live_id ≫ live_count` arises:

> Under streaming/repartition with a fragmented prior round, the next `split_with_config` call may assign a worker `worker_agents = [10, 50, ..., 99_999_990, 99_999_999]` — 10M live agents whose IDs were drawn from a `next_id` that advanced to ~100M during the prior round's CON-DUP commutation. `max_live_id = 99_999_999`, `effective_arena_size = 100_000_000`, threshold fires at `> 4 × 10M = 40M`. SPARSE path taken.

This makes the mechanism explicit and falsifiable.

(b) **Defer to SC-009 closure log:** Add a forwarding reference: "See `docs/spec-reviews/SPEC-REVIEW-22-round-2-2026-04-25.md` SC-009 closure for the fragmentation-pattern derivation; R22a asserts that the same evidence base remains covered under the corrected metric."

(c) **Audit the original evidence:** If neither (a) nor (b) is sufficient, run a probe-instrumented benchmark on a fragmented workload (e.g., post-CON-DUP-cascade `dual_tree` repartition) and record the empirical `max_live_id` / `live_count` ratio in the closure log §1. This is what closure log §5 invites.

Option (a) is the lowest-cost fix; option (c) is the most rigorous.

---

### SC-025: R30 SHOULD-rename of `id_range_size` field creates spec/code drift

**Severity:** MEDIUM
**Axis:** Consistency / Testability
**Section:** §3.4 R30
**Requirement:** R30

**Problem:**
R30 says:
> The `PartitionError::DenseAllocationExceedsThreshold` payload SHOULD report `effective_arena_size` (renaming the pre-D-011 `id_range_size` field) alongside `live_count` and `partition_index`, so that diagnostics directly identify the actual `Vec<Option<Agent>>` size that would have been allocated; the field rename is a normative consequence of the R22 metric correction (D-011 Amendment 2026-05-04).

Two issues:

1. **SHOULD vs MUST.** The rename is described as "a normative consequence" — implying MUST — but the verb is SHOULD. SHOULD is permissive; an implementation could legitimately keep `id_range_size` AND name it `effective_arena_size` in the field's documentation, or vice versa. The spec leaves room for ambiguous naming.

2. **Spec/code timing.** The current code at `relativist-core/src/error.rs` and `helpers.rs:432-444` carries `id_range_size`. The closure log defers the rename to TASK-0612. Between the spec amendment landing (now) and TASK-0612 implementation, the spec's R30 is ahead of the code. A test asserting `effective_arena_size` in the error payload would fail until TASK-0612 lands. The closure log §4 ("Test floor expectation") notes "no change from spec amendment alone" — but R30's SHOULD-with-deferred-implementation creates a window where the spec's normative wording does not match the in-tree code.

**Impact if unresolved:**
- A reviewer of TASK-0612 cannot use R30 alone to determine whether the rename is required (SHOULD) or optional.
- Tests written against the SPEC text may diverge from tests written against the in-tree code.
- The SHOULD verb invites future drift: a future implementer might decide the rename is "stylistic" and skip it, leaving the diagnostic field name forever misaligned with the metric.

**Suggested resolution:**
Tighten R30 to MUST and tie the rename explicitly to TASK-0612:

> The `PartitionError::DenseAllocationExceedsThreshold` payload MUST report `effective_arena_size` (renaming the pre-D-011 `id_range_size` field) alongside `live_count` and `partition_index`. The rename is normative; implementations MUST land the field rename atomically with the threshold metric change. (TASK-0612 of the D-011 partition-perf-fix bundle is the canonical implementation hand-off.)

If the timing constraint cannot be met (i.e., the spec amendment must land before TASK-0612), the closure log should record an explicit "spec ahead of code" status rather than leaving R30 as SHOULD.

---

### SC-026: R22 lacks a definition of `live_agent_count` in the threshold expression

**Severity:** LOW
**Axis:** Completeness
**Section:** §3.2 R22

**Problem:**
R22 uses `partition.live_agent_count` in the threshold inequality `effective_arena_size > 4 × partition.live_agent_count`. The term is used without explicit definition in §3.2; implementers must infer that it equals `worker_agents.len()` (which is what the implementation at `helpers.rs:433` uses: `live_count = worker_agents.len() as u64`).

The closure log §5 implicitly assumes this equivalence ("Q4 — single live agent at id = u32::MAX-1"; `4 × 1 = 4`). The handoff brief does too. But §3.2 R22 itself does not state the definition.

This is a minor terminology drift — `partition.live_agent_count` is plausibly a `Partition` struct field (SPEC-04 R15a), not the `worker_agents` slice length. If `Partition.live_agent_count` is computed differently (e.g., by counting `Some(_)` slots in the dense `agents` Vec post-build), the threshold value could differ from the implementation reality.

**Impact if unresolved:**
- A literal reader of R22 may bind `partition.live_agent_count` to a different code construct than `worker_agents.len()`.
- Test authors may use `partition.subnet.count_live_agents()` (SPEC-22 R11) post-build, which equals `worker_agents.len()` only when `worker_agents` contains exclusively live agents (which is asserted by `helpers.rs:319` `expect("worker_agents should only contain live agent IDs")`, but never spec'd in R22 itself).

**Suggested resolution:**
Add a one-line definition immediately after the metric definition in R22:

> `partition.live_agent_count := worker_agents.len()` — the count of live agents assigned to this partition, identical to the slice length passed to `build_subnet` (per SPEC-04 R15a / `helpers.rs:316-321`'s precondition that `worker_agents` contains exclusively live agent IDs).

---

### SC-027: R22 silently inherits the `helpers.rs:351-353` clamping behavior without spec mandate

**Severity:** LOW
**Axis:** Consistency
**Section:** §3.2 R22, §3.1 R10a (cross-ref)

**Problem:**
R22's SPARSE path calls `to_dense(Some(partition.id_range.clone()))`. The resulting `Net.agents` Vec is sized to `max_id + 1` per `to_dense` at `helpers.rs:676`. R10a then says the free-list MUST contain "all `None` slots in `[partition.id_range.start, partition.id_range.end)`."

But if `max_id < partition.id_range.end` (which is the common case, since `id_range.end` is allocated as `max(100_000, base_next_id × 10)` per SPEC-04 R18), iterating `agents.get(i)` for `i in [id_range.start, id_range.end)` accesses indices past `agents.len()`. The implementation at `helpers.rs:351-353` clamps `range_end = id_range.end.min(arena_len)` — but this clamping is NOT mandated by R10a or R22.

Pre-D-011, this was a latent issue too — the metric correction does not introduce it, but the corrected SPARSE path is now taken on a different (smaller) set of workloads, so the clamping correctness is more frequently exercised. Spec should make the clamp explicit, OR R10a should narrow its iteration bound to `[id_range.start, min(id_range.end, max_id + 1))`.

**Impact if unresolved:**
- An implementation that follows R10a literally (without clamping) would panic with index-out-of-bounds.
- A test that constructs `partition` with `id_range.end > max_id + 1` would expose the clamp dependency.

**Suggested resolution:**
Either:

(a) Tighten R10a to:
> The partition's `free_list` MUST contain all `None` slots in `[partition.id_range.start, min(partition.id_range.end, partition.subnet.agents.len() as AgentId))`. The upper bound clamp ensures iteration stays within the allocated arena (`helpers.rs:351-353` is the canonical implementation).

(b) Make the clamp a §4 design note (less normative, but at least documented).

Note this is pre-existing and orthogonal to the D-011 metric correction; it is surfaced here only because the corrected SPARSE path now runs on a different workload distribution and may exercise the latent issue more frequently.

---

### SC-028: Closure-log claim about "pre-D-011 metric measured `compute_id_ranges` chunk multiplier" is technically loose

**Severity:** LOW
**Axis:** Consistency
**Section:** §11 Change Log entry "D-011 Amendment — 2026-05-04"

**Problem:**
The change log line 906 states:
> the pre-D-011 metric `id_range > 4 × live_agent_count` was measuring the planning range produced by `compute_id_ranges` (`base_next_id × 10` chunk multiplier)

This is shorthand. Strictly, the metric in `helpers.rs:432` measures `id_range_size = id_range.end as u64 - id_range.start as u64`, where `id_range` is the per-partition output of `compute_id_ranges`. The `base_next_id × 10` chunk multiplier is the FORMULA `compute_id_ranges` uses to derive `id_range.end - id_range.start` (per SPEC-04 R18), but the metric itself is just the size of the range, not the multiplier.

A pedantic reader will note that "measuring the chunk multiplier" is a one-step indirection from "measuring the range size produced by applying the chunk multiplier."

**Impact if unresolved:**
- A future spec-archaeologist tracing the regression's root cause may briefly conflate `compute_id_ranges`'s implementation choice with the metric definition.
- Negligible practical impact.

**Suggested resolution:**
Tighten the change log line:

> the pre-D-011 metric `id_range > 4 × live_agent_count` was measuring the size of the planning range produced by `compute_id_ranges` (specifically `id_range.end - id_range.start`, which under SPEC-04 R18's `ContiguousIdStrategy` evaluates to `max(100_000, base_next_id × 10)` per partition)

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 0 |
| HIGH     | 1 (SC-022) |
| MEDIUM   | 3 (SC-023, SC-024, SC-025) |
| LOW      | 3 (SC-026, SC-027, SC-028) |
| **Total** | **7** |

## Mandatory (must fix before SPEC-22 reaches Reviewed v2.4)
- **SC-022:** Tighten R22's "MAY use the existing dense path or the sparse-then-dense path" to forbid SPARSE below threshold. Spec-permitted re-regression vector.
- **SC-024:** Replace R22a's asserted M5-detection mechanism with an explicit fragmentation derivation OR a forwarding reference to the SC-009 closure log. As written, the M5-still-detected claim is the load-bearing rationale for the amendment but is asserted without traceable derivation.

## Recommended (should fix before TASK-0612 closes; may fix in a follow-up)
- **SC-023:** Add an explicit "build-time only" bullet to R22.
- **SC-025:** Tighten R30 from SHOULD-rename to MUST-rename, tied to TASK-0612 atomic landing.
- **SC-026:** Add explicit definition of `partition.live_agent_count := worker_agents.len()` to R22.

## Optional (LOW, polish-only)
- **SC-027:** Make the `min(id_range.end, arena_len)` clamp explicit in R10a, OR document as §4 design note. Pre-existing, not introduced by D-011.
- **SC-028:** Tighten the change-log shorthand "measuring the planning range produced by `compute_id_ranges`" to reference the actual `id_range.end - id_range.start` computation.

## Non-blockers for TASK-0612 dispatch

None of the 7 findings block the dispatch of TASK-0612 (the perf fix). The corrective intent of the amendment is sound; the metric `effective_arena_size := max_live_id + 1` correctly aligns with `helpers.rs:301-303`. The findings are about SPEC-22's normative completeness post-amendment, not about the perf fix itself. Stages 2-6 of the SDD pipeline (`docs/plans/2026-05-04-d011-partition-perf-fix.md` §5) can proceed with the v2.3 spec text as-is; the fixes for the findings above can land in a separate amendment commit before SPEC-22 is moved to Reviewed v2.4 status.

---

## Checklist

### Consistency
- [x] All terms match SPEC-00 definitions (`AgentId`, `PortId`, `Net`, `Partition`)
- [x] Type signatures compatible with predecessor specs (R22 metric is a `u64`-typed predicate; SPEC-04 R15a `Partition.live_agent_count` is implicitly `u64` / `usize`; SPEC-22 §3.8 A4 SPEC-02 R11 amendment compatible)
- [~] No contradictions with predecessor requirements — **caveat:** SC-022 flags that R22's MAY clause permits a behavior the perf-fix implementation forbids; spec/code drift inside R22 itself
- [~] Data flow assumptions match predecessor outputs — **caveat:** SC-026 (live_agent_count term undefined); SC-027 (R10a iteration silently depends on clamp)

### Testability
- [x] Every MUST requirement has a testable criterion (R22 threshold inequality is directly testable; R22a worked example is verified arithmetically)
- [x] Boundary conditions defined (0 agents → effective_arena_size = 0 → DENSE; 1 agent at id=u32::MAX-1 → SPARSE; verified in closure log §5 Q3, Q4)
- [~] Error conditions specified — **caveat:** SC-025 (R30's SHOULD-rename leaves error variant naming ambiguous in tests)
- [~] R22's "MAY" branch is not pinpointable to a single test (SC-022)

### Completeness
- [~] Pseudocode provided for non-trivial operations — **caveat:** SC-024 (R22a's fragmentation mechanism is asserted, not derived)
- [~] All edge cases documented — **caveat:** SC-023 (build-time-only scope is in closure log only); SC-027 (clamp behavior not in spec)
- [x] Rust type signatures preserved (no signature changes from this amendment)
- [~] No undefined terms or dangling references — **caveat:** SC-026 (`live_agent_count`)

### Invariant Preservation
- [x] T1-T7 maintained (T7 specifically: R22's `max()` is order-independent, asserted in spec text)
- [x] D1-D6 maintained (no changes to split/merge identity, ID uniqueness, exclusive ownership; the metric correction does not affect any D-layer invariant)
- [x] I1-I7 maintained (free-list semantics unchanged; I3' uniqueness intact)
- [x] G1 not violatable by any valid operation sequence (the metric is a build-time representation choice; both SPARSE and DENSE paths produce isomorphic Nets per §4.6 conversion semantics; G1 is independent of representation)

---

## Final note

This is a corrective amendment, not a redesign. Verdict `NEEDS_REVISION` is calibrated against the spec's own quality bar (Reviewed v2 → Reviewed v2.3 status implies near-publication-ready normative text), not against the urgency of the perf fix. The spec-critic agrees with the closure log's expectation of `CONDITIONAL_PASS`-level severity (closure log §5 verdict) but elevates two findings (SC-022, SC-024) to mandatory-fix because they affect the spec's normative completeness, not just the perf-fix dispatch. Stages 2-6 of TASK-0611..0614 can dispatch on v2.3 as-is; SC-022 and SC-024 can be closed in a small follow-up amendment that lands SPEC-22 at Reviewed v2.4.
