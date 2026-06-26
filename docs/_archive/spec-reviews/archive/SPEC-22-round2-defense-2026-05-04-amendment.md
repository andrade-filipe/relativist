# SPEC-22 D-011 Amendment — Round 2 Defense (Closure of Round 1 Spec-Critic Findings)

**Date:** 2026-05-04
**Author:** especialista-em-specs (TCC root session, Round 2+ defender)
**Spec under amendment:** `codigo/relativist/specs/SPEC-22-arena-management.md` (Reviewed v2.3 → Reviewed v2.4)
**Round 1 review:** `codigo/relativist/docs/spec-reviews/SPEC-22-round1-critic-2026-05-04-amendment.md` (commit `972ce47`)
**Round 1 closure log (v2.3):** `codigo/relativist/docs/spec-reviews/SPEC-22-amendment-2026-05-04-d011-blocker.md`
**Round 1 amendment commit:** `e941273`
**Bundle:** D-011 partition perf fix (Stage 1 of 6, SDD pipeline)

---

## 1. Summary

**Round 1 verdict:** `NEEDS_REVISION` (1 HIGH, 3 MEDIUM, 3 LOW = 7 findings).

**Round 2 disposition:** **6 ACCEPTED** (closed in v2.4) / **1 REJECTED** (with rationale).

**v2.4 status:** **Reviewed** — D-011 corrective amendment is final; no Round 3 expected absent new evidence. Both mandatory-fix findings (SC-022 HIGH, SC-024 MEDIUM) are closed.

| ID | Severity | Decision | One-line rationale |
|----|----------|----------|--------------------|
| SC-022 | HIGH | ACCEPT | R22 third-bullet "MAY" rewritten to "MUST use dense path; SPARSE FORBIDDEN below threshold". |
| SC-023 | MEDIUM | ACCEPT | R22 extended with explicit "Build-time only" paragraph documenting one-shot evaluation scope. |
| SC-024 | MEDIUM | ACCEPT | R22a fragmentation mechanism re-derived (one-way `next_id` drift) + forwarding reference to SC-009 closure. |
| SC-025 | MEDIUM | ACCEPT | R30 SHOULD-rename → MUST-rename, tied to TASK-0612 atomic landing. |
| SC-026 | LOW | ACCEPT | R22 second bullet defines `partition.live_agent_count := worker_agents.len()` inline. |
| SC-027 | LOW | REJECT-WITH-RATIONALE | Pre-existing, orthogonal to D-011 metric correction; the critic explicitly acknowledges this in their finding text. Out of scope for the corrective amendment. |
| SC-028 | LOW | ACCEPT | Change-log shorthand tightened to reference `id_range.end - id_range.start` directly. |

**Both mandatory-fix findings (SC-022, SC-024) closed** — the spec-critic's "must close before v2.4" gate is met.

---

## 2. Per-finding response

### SC-022 (HIGH) — R22 "MAY" clause permits perf regression to recur

**Decision:** ACCEPT.

**Rationale.** The critic is correct that the v2.3 third bullet ("Otherwise, `build_subnet` MAY use the existing dense path or the sparse-then-dense path") permits, by spec letter, the +63% partition+merge regression to re-emerge if a future implementation chooses SPARSE under the `sparse_build = true` default when the threshold is NOT exceeded. The implementation in `helpers.rs:447-475` already enforces the narrowing the critic prescribes; v2.4 aligns the spec letter with the implementation reality. This is the SC-022 "spec letter loophole" the user flagged in the dispatch instructions.

**v2.4 change.** R22 third bullet rewritten:

> Otherwise (i.e., `effective_arena_size <= 4 × partition.live_agent_count`), `build_subnet` MUST use the dense path. The SPARSE path is FORBIDDEN below threshold, regardless of `PartitionConfig.sparse_build` (R30). The `sparse_build = false` flag is honored only as a rejection signal: when `sparse_build = false` AND threshold IS exceeded, `build_subnet` MUST return `PartitionError::DenseAllocationExceedsThreshold` per R30.

Plus an explicit closure note citing the BLOCKER 2026-05-04 evidence base.

### SC-023 (MEDIUM) — R22 silent on build-time-only scope

**Decision:** ACCEPT.

**Rationale.** The pre-empted attack vector Q2 in the v2.3 closure log §5 answers this scope question, but the answer lives in the closure log only — not in the normative spec text. An implementer or test author reading R22 alone could reasonably interpret the threshold as a runtime invariant. v2.4 promotes the closure-log answer to a normative R22 paragraph.

**v2.4 change.** New paragraph appended to R22:

> **Build-time only (closes SC-023).** The metric `effective_arena_size > 4 × partition.live_agent_count` is evaluated EXACTLY ONCE, at `build_subnet` entry. Subsequent `create_agent` calls during local reduction MAY grow the arena past `max_live_id + 1` (e.g., CON-DUP commutation when the free-list is empty). R22 imposes NO post-build re-check; arena growth during reduction is governed by SPEC-02 R11 (next-available-ID via free-list-pop or `next_id` increment) and SPEC-22 R3 / R4 (free-list-first allocation), NOT by the R22 threshold. The threshold's purpose is build-time arena sizing, not runtime memory protection. Under streaming/repartition, R22a's M5-detection guard fires at the NEXT `build_subnet` entry, where the worker's now-fragmented live set produces `max_live_id ≫ live_count` in the post-fragmentation `worker_agents` slice.

### SC-024 (MEDIUM) — R22a M5-detection mechanism asserted, not derived

**Decision:** ACCEPT.

**Rationale.** The critic identifies two real defects in v2.3's R22a:

1. **`max_live_id` source indirection unstated.** v2.3 R22a says the high-numbered slots "leave high-numbered slots occupied while low-numbered slots churn". The source path from "fragmentation during reduction" to "`max_live_id` of the NEXT `build_subnet` call's `worker_agents`" is real but indirect, and v2.3 doesn't name it.
2. **Mental model backwards.** The "low-numbered slots churn" framing contradicts the LIFO free-list ordering documented in §4.7 (which reuses MOST-RECENTLY-FREED first, i.e., low-numbered slots first under typical access patterns). The actual mechanism is one-way `next_id` drift during creation-heavy phases when the free-list is empty (e.g., `RecyclePolicy::DisableUnderDelta` while `delta_mode || streaming_active`).

The critic's Option (a) (explicit fragmentation mechanism) plus Option (b) (forwarding reference) are both adopted in v2.4. Option (c) (instrumented benchmark) is deferred — the explicit derivation is sufficient to make the claim falsifiable; an empirical probe is possible but not required to close the spec finding.

**v2.4 change.** R22a body fully rewritten with three normative blocks:

- **Fragmentation mechanism block** — explicit derivation of `max_live_id ≫ live_count` via one-way `next_id` drift under CON-DUP-dominated phases with free-list disabled, plus explicit `worker_agents` indirection through `split_with_config` (the previous round's `next_id` becomes the next round's `max_live_id` only when the partitioner re-assigns those high-numbered live agents to a worker — the streaming-finalize/repartition pattern, NOT a single round's local reduction).
- **Worked example block** — concrete numbers: worker 0 enters round N with `live_count = 100K`, after 25M CON-DUP fires `next_id ≈ 100M`, next finalize produces `worker_agents` with `max_live_id ≈ 99_999_999` and `live_count = 10M`, threshold fires at `100M > 40M`.
- **Forwarding reference block** — explicit pointer to `docs/spec-reviews/SPEC-REVIEW-22-round-2-2026-04-25.md` SC-009 closure for the HVM2 evidence base coverage assertion. Argues that the corrected `max_live_id`-based metric covers the same evidence base because the original HVM2 pathology was driven by `max_live_id`-style fragmentation, not by `id_range.end`.

### SC-025 (MEDIUM) — R30 SHOULD-rename creates spec/code drift

**Decision:** ACCEPT.

**Rationale.** v2.3 R30 used SHOULD for the `id_range_size → effective_arena_size` field rename, with a parenthetical noting that the rename is "a normative consequence" of the metric correction. The critic correctly observes that "normative consequence" implies MUST while the verb is SHOULD; this leaves room for an implementer to legitimately keep the old name or use ambiguous naming. The v2.3 closure log §4 records that the rename is deferred to TASK-0612, but the normative SHOULD verb does not encode that deferral.

**v2.4 change.** R30 rename clause tightened:

> The `PartitionError::DenseAllocationExceedsThreshold` payload MUST report `effective_arena_size` (renaming the pre-D-011 `id_range_size` field) alongside `live_count` and `partition_index` ... The rename is normative; implementations MUST land the field rename atomically with the threshold metric change. TASK-0612 of the D-011 partition-perf-fix bundle is the canonical implementation hand-off (see `docs/plans/2026-05-04-d011-partition-perf-fix.md` §5 Task 4).

The MUST is calibrated against TASK-0612's atomic landing, not against the in-tree code at the moment v2.4 lands. The spec-ahead-of-code window between v2.4 and TASK-0612 completion is acknowledged explicitly.

### SC-026 (LOW) — R22 lacks `live_agent_count` definition

**Decision:** ACCEPT.

**Rationale.** v2.3 R22 uses `partition.live_agent_count` in the threshold inequality without an inline definition. The closure log §5 implicitly assumes `partition.live_agent_count == worker_agents.len()`, and the implementation at `helpers.rs:433` confirms this binding (`live_count = worker_agents.len() as u64`), but R22 itself does not state it. A literal reader could plausibly bind the term to a `Partition.live_agent_count` field defined elsewhere.

**v2.4 change.** New second bullet inserted into R22:

> Define `partition.live_agent_count := worker_agents.len()` (cast to `u64`) — the count of live agents assigned to this partition, identical to the slice length passed to `build_subnet` per SPEC-04 R15a / `helpers.rs:316-321`'s precondition that `worker_agents` contains exclusively live agent IDs (the `expect("worker_agents should only contain live agent IDs")` at `helpers.rs:319` guards this). This binding closes SC-026.

### SC-027 (LOW) — R10a iteration silently depends on `helpers.rs:351-353` clamp

**Decision:** REJECT-WITH-RATIONALE.

**Rationale.** The critic's own finding text explicitly acknowledges:

> Note this is pre-existing and orthogonal to the D-011 metric correction; it is surfaced here only because the corrected SPARSE path now runs on a different workload distribution and may exercise the latent issue more frequently.

Three reasons to reject in this Round 2:

1. **Out of scope.** The clamp predates D-011. SC-027 is a latent issue surfaced by the D-011 metric correction, not introduced by it. The user's dispatch constraints explicitly state: "Keep changes minimal — surgical wording fixes, not a rewrite." Including SC-027 in v2.4 expands scope beyond the corrective amendment.
2. **Implementation already correct.** The clamp at `helpers.rs:351-353` is correct and tested. The defect is in spec wording (R10a does not mandate the clamp), not in code. A spec/code drift finding for a pre-existing condition is appropriate to file as a separate hardening task, not folded into a corrective amendment.
3. **Frequency of exercise is empirical, not normative.** The critic argues the corrected SPARSE path "may exercise the latent issue more frequently". This is plausible but unverified. v2.4 closes the metric correction; whether the clamp is exercised more or less under the corrected metric is an empirical question the developer agent can investigate during TASK-0613 (integration regression witness) if the in-tree behaviour diverges from spec.

**Recommendation for follow-up.** A separate amendment against R10a (or a §4 design note) should be authored when SPEC-22 next undergoes a non-corrective revision, OR when an in-tree test exposes the clamp dependency. The critic offers two compliant paths in their finding (option (a) tighten R10a, option (b) §4 design note); either is acceptable for the follow-up.

**No v2.4 change.** SC-027 remains open as a tracked-but-deferred finding. The v2.4 change-log explicitly records this rejection.

### SC-028 (LOW) — Change-log shorthand technically loose

**Decision:** ACCEPT.

**Rationale.** The critic correctly observes that the v2.3 change-log line "the pre-D-011 metric `id_range > 4 × live_agent_count` was measuring the planning range produced by `compute_id_ranges` (`base_next_id × 10` chunk multiplier)" conflates the FORMULA `compute_id_ranges` uses (`base_next_id × 10`) with the metric itself (`id_range.end - id_range.start`). Negligible practical impact, but easy to fix.

**v2.4 change.** Change-log line 906 tightened:

> Probe instrumentation in `partition::helpers::build_subnet_with_config` confirmed that the pre-D-011 metric `id_range > 4 × live_agent_count` was measuring the size of the planning range produced by `compute_id_ranges` — specifically `id_range.end - id_range.start`, which under SPEC-04 R18's `ContiguousIdStrategy` evaluates to `max(100_000, base_next_id × 10)` per partition (the `base_next_id × 10` chunk multiplier is the formula `compute_id_ranges` uses to derive the range size, not the metric itself; closes SC-028 — change-log shorthand tightened in v2.4).

---

## 3. Diff summary of SPEC-22 changes (v2.3 → v2.4)

| Surface | Edit | Closes |
|---------|------|--------|
| Frontmatter `Status` line | Extended: Reviewed v2.4 with one-line summary per ACCEPTED finding; v2.3 line preserved as historical. | — |
| §3.2 R22 second bullet (NEW) | Adds `partition.live_agent_count := worker_agents.len()` inline definition. | SC-026 |
| §3.2 R22 third bullet (REWRITTEN) | "MAY use existing dense path or sparse-then-dense path" → "MUST use the dense path. The SPARSE path is FORBIDDEN below threshold, regardless of `PartitionConfig.sparse_build`." Closure note appended. | SC-022 |
| §3.2 R22 — new "Build-time only" paragraph | Documents that the metric is evaluated EXACTLY ONCE at `build_subnet` entry; in-round arena growth is governed by SPEC-02 R11 + SPEC-22 R3/R4, not R22. | SC-023 |
| §3.2 R22a (REWRITTEN) | Three normative blocks: fragmentation-mechanism (one-way `next_id` drift), worked example, forwarding reference to SC-009 closure for HVM2 evidence. | SC-024 |
| §3.4 R30 (TIGHTENED) | SHOULD-rename → MUST-rename, tied to TASK-0612 atomic landing. | SC-025 |
| §11 D-011 Amendment 2026-05-04 motivation paragraph | Shorthand "the planning range produced by `compute_id_ranges` (`base_next_id × 10` chunk multiplier)" → explicit "the size of the planning range ... specifically `id_range.end - id_range.start`, which under SPEC-04 R18's `ContiguousIdStrategy` evaluates to `max(100_000, base_next_id × 10)` per partition". | SC-028 |
| §11 — new "D-011 Round 2 Amendment — 2026-05-04" subsection | Records the seven Round 1 findings, decisions, and v2.4 actions. | — |

**Cross-spec propagation:** none required. SPEC-04 §4.5.1 A7 already forwards to SPEC-22 R22 for the canonical metric ("per SPEC-22 R22 — the canonical metric is normative there"); the v2.4 R22 changes are consumed by reference. SPEC-21 §3.3 / §4.9 / §8 Q6 (5 spots) likewise forward to SPEC-22 R22 transitively. The SPARSE-below-threshold-FORBIDDEN clause (SC-022) is consistent with SPEC-21 §4.9 accumulator design: the streaming pipeline's accumulator selection is a CONSTRUCTION-time choice that finalizes via `to_dense` at the SAME R22 threshold gate; if the threshold is not exceeded at finalize, the dense `Net` materializes via the dense path per the tightened R22, consistent with SPEC-21's "selected at construction time" rule.

**Code-side changes:** none. v2.4 is a spec-only amendment; `relativist-core/src/` is not touched. TASK-0612 remains the canonical implementation hand-off for the field rename + metric implementation.

---

## 4. Test floor expectation

**No change from v2.4 alone.** v2.4 is a normative-only refinement of v2.3 closing Round 1 spec-critic findings. No spec-derived test additions or removals fire from this commit. Floors entering D-011 partition-perf-fix remain:

- `cargo test` (default profile): 1683
- `cargo test --features zero-copy`: 1726
- `cargo test --features streaming-no-recycle`: 1680
- v1 frozen floor (must never regress): 690

Implementation tasks 0612–0613 still target ≥ 1684 default / ≥ 1727 zero-copy / ≥ 1681 streaming-no-recycle (informational projection per partition-perf-fix plan §6 G2/G3/G4).

---

## 5. Final v2.4 status: **Reviewed**

All 6 ACCEPTED findings are closed in v2.4. The 1 REJECTED finding (SC-027) is documented with rationale and remains tracked as a non-blocking deferred item. The two mandatory-fix findings flagged by the spec-critic (SC-022, SC-024) are both closed. The corrective amendment is final; no Round 3 expected absent new evidence.

**Verdict:** **Reviewed v2.4** (D-011 corrective amendment final).

**Stage gate:** v2.4 lands BEFORE Stage 2 (TASK-SPLITTER) of the D-011 partition-perf-fix bundle (`docs/plans/2026-05-04-d011-partition-perf-fix.md` §5). TASK-0611..0614 dispatch can proceed against v2.4 spec text.
