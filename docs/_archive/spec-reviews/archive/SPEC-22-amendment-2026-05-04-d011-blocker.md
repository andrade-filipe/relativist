# SPEC-22 R22+R30 Amendment — Closure Log (D-011 BLOCKER 2026-05-04)

**Date:** 2026-05-04
**Author:** especialista-em-specs (TCC root session)
**Spec under amendment:** `codigo/relativist/specs/SPEC-22-arena-management.md` §3.2 R22, R30, +R22a (new), revision history extended
**Cross-references propagated:** `codigo/relativist/specs/SPEC-04-partition.md` §4.5.1 Amendment A7; `codigo/relativist/specs/SPEC-21-streaming-generation.md` §4.9 (intro, doc-comments at PartitionAccumulator, finalize-time threshold contract) + §8 Q6
**Branch:** v2-development
**Bundle:** D-011 partition perf fix (Stage 1 of 6, SDD pipeline)
**Plan reference:** `codigo/relativist/docs/plans/2026-05-04-d011-partition-perf-fix.md` §3 (handoff brief), §8 (pre-empted attack vectors)
**Origin defect:** `codigo/relativist/docs/next-steps.md` BLOCKER 2026-05-04 (partition perf regression — root cause: design-level metric error)
**Handoff brief:** `codigo/relativist/docs/handoffs/2026-05-04-SPEC-22-R22-R30-metric-amendment-handoff.md`

---

## 1. Bisect summary (evidence base)

**Symptom.** `ep_annihilation_con 5M w=2` `local --workers=2` wall-time on `v2-development`: 22.88 s (HEAD) vs 11.99 s (`v1-feature-complete`). +83% regression on the canonical Tier 1 break-even workload.

**Bisect (7-point, median of 3 reps, native release build, same machine, see `docs/next-steps.md` BLOCKER 2026-05-04 lines 51-99):**

| Point | wall (s) | partition+merge (s) | Δ vs v1 |
|---|---:|---:|---:|
| v1-feature-complete (501b901) | 11.99 | 7.03 | — |
| D-009 Phase B end (47d9bf2) — free-list only | 11.96 | 6.28 | −0.3% (noise) |
| **D-009 Phase C end (d6411be)** — SparseNet + I3' assertions | **21.73** | **~16.3** | **+81%** |
| D-009 Stage 6 W1 (0d7c4a8) | 21.61 | 15.93 | +80% |
| D-010 final (61e86a1) | 22.50 | 16.94 | +88% |
| D-011 task596 (abd2976) | 22.96 | 17.53 | +91% |
| HEAD (2b6528b) | 22.88 | 17.49 | +91% |

**Probe instrumentation (HEAD, ep_con 5M w=2, two partitions):**

| Worker | id_range | size | live | ratio | branch | wall |
|---|---|---:|---:|---:|---|---:|
| 0 | 10M..110M | 100 M | 5 M | 20× | SPARSE | 7.05 s |
| 1 | 110M..4.29B | 4.18 B | 5 M | 837× | SPARSE | 6.78 s |

Both partitions take the SPARSE branch even though they are *healthy* workloads (live agents densely packed in their assigned id_range). Total partition cost ≈ 13.8 s (~63% of total wall), matching the bisect delta.

**Root cause.** The pre-D-011 metric `id_range > 4 × live_agent_count` measures the **planning range** produced by `partition::helpers::compute_id_ranges` (`max(100_000, base_next_id × 10)`). The dense `build_subnet` (line 289 of `partition/helpers.rs`) does not allocate by `id_range.end`; it allocates `vec![None; max_id + 1]` where `max_id = worker_agents.iter().max()` (line 301-303). For healthy workloads under `ContiguousIdStrategy`, `id_range.end` is 5–800× larger than `max_id + 1`, so the threshold mis-fires and routes every healthy partition through the slow SPARSE path. The metric is measuring the wrong signal: it should measure the actual `Vec<Option<Agent>>` size that the dense path would allocate, not the planning range that `compute_id_ranges` happens to choose.

This is a design-level defect. A pure code fix without spec amendment would create spec drift (SPEC-22 R22's normative metric would no longer match the implementation). Hence this Stage-1 spec amendment.

## 2. Metric diff

### R22 — before (Reviewed v2.2)

> If `partition.id_range.end - partition.id_range.start > 4 * partition.live_agent_count`, `build_subnet` MUST use `SparseNet` and then call `to_dense(Some(partition.id_range.clone()))` before the reduction loop. This is the M5-target case (SC-009).

### R22 — after (Reviewed v2.3 — D-011 Amendment 2026-05-04)

- Defines `effective_arena_size := max_live_id + 1` where `max_live_id := worker_agents.iter().copied().max()` (cast to `u64`); empty `worker_agents` ⇒ `effective_arena_size := 0`.
- Threshold rule: if `effective_arena_size > 4 * partition.live_agent_count`, `build_subnet` MUST use `SparseNet` and then call `to_dense(Some(partition.id_range.clone()))` before the reduction loop. M5-target case (SC-009) AND M5 ID-fragmentation pathology (R22a).
- Cites `relativist-core/src/partition/helpers.rs:301-303` as the implementation reality the metric matches.
- Documents that the `4×` constant is unchanged from the pre-D-011 formulation; only the metric the constant multiplies has been corrected.
- Adds an explicit determinism-T7 note: `worker_agents.iter().copied().max()` is order-independent on `&[u32]`; the slice itself is constructed deterministically by `split_with_config`; therefore SPEC-01 T7 is preserved.
- Cites the BLOCKER 2026-05-04 evidence base (bisect + probe) and the +83% wall regression as the empirical motivation.

### R30 — diff

The rejection rule is rewritten to refer to the new metric (`effective_arena_size > 4 × live_agent_count` instead of `id_range > 4 × live_agent_count`). The error variant `PartitionError::DenseAllocationExceedsThreshold` SHOULD report `effective_arena_size` (renaming the pre-D-011 `id_range_size` field) alongside `live_count` and `partition_index`. The field rename is a normative consequence of the R22 metric correction; the actual rename in `relativist-core/src/error.rs` lands in TASK-0612 of the partition-perf-fix bundle.

### R22a — new

Clarification rule: M5 pathology (recycled-id fragmentation under delta-mode operation without compaction) is still detected by the corrected metric. Concrete worked example: `live_count = 10M`, `max_live_id = 100M` (heavy fragmentation) ⇒ `effective_arena_size = 100_000_001 > 4 × 10_000_000 = 40_000_000`, threshold fires, SPARSE path taken, prevents 800 MB `vec![None; 100M]` allocation. Empty-partition convention preserved (`worker_agents.is_empty() ⇒ effective_arena_size = 0 ⇒ dense path returns `Net::new()`` per `partition/helpers.rs:297-299`). The rule confirms that the metric correction does NOT lose the M5-pathology guard that originally motivated R22 / SC-009; the pre-D-011 metric was an over-approximation that happened to subsume M5 while ALSO mis-routing healthy workloads, and the post-D-011 metric is exact.

### Revision history

`§11 Change Log` extended with a new subsection "D-011 Amendment — 2026-05-04 — `effective_arena_size` threshold metric (closes BLOCKER 2026-05-04)" recording: motivation (BLOCKER 2026-05-04, +83% wall regression bisect), scope table (R22 modified / R30 modified / R22a added), cross-references audited (SPEC-04 §4.5.1 A7 + SPEC-21 §3.3 / §4.9 / §8 Q6), out-of-scope items (`compute_id_ranges` chunk multiplier preserved; R23 orthogonal; wire format / `PROTOCOL_VERSION` unaffected; SparseNet itself unchanged), implementation hand-off paths (`error.rs`, `partition/helpers.rs:432-444`, in-file tests, integration witness), test-floor impact (none from amendment alone; floors entering D-011: 1683 default / 1726 zero-copy / 1680 streaming-no-recycle; v1 floor 690), files cross-referenced for the audit, spec-critic Round 1 status (REQUESTED, expected `CONDITIONAL_PASS`), pre-empted attack vectors per §8 of the partition-perf-fix plan, closure log path (this document), status transition Reviewed v2.2 → Reviewed v2.3.

The frontmatter `Status` line was extended in-place to record the v2.3 transition with a one-paragraph summary preceding the v2.2 line.

## 3. Cross-references audited

Best-effort grep over `codigo/relativist/specs/`:

```
grep -rn "id_range.*4.*live\|id_range_size.*4\|id_range > 4" codigo/relativist/specs/
```

Findings outside SPEC-22 + remediation:

| Spec | Line(s) | Surface | Action |
|------|--------:|---------|--------|
| SPEC-04 | 409 | §4.5.1 Amendment A7 — cites the threshold formula in the A7 callout block ("dense-arena threshold check fires (`id_range.end - id_range.start > 4 × live_agent_count`)") | Updated: A7 now defers to "per SPEC-22 R22 — the canonical metric is normative there"; the historical formula citation is replaced with a forwarding reference + a parenthetical noting the D-011 supersession. |
| SPEC-21 | 752 | §3.3 — accumulator paragraph, "selected at construction time using the same `id_range > 4 × live_agent_count` threshold rule that SPEC-22 R10a/R22 mandates" | Updated to `effective_arena_size > 4 × live_agent_count` with a parenthetical citing SPEC-22 R22 D-011 Amendment 2026-05-04 and the supersession. |
| SPEC-21 | 764 | §4.9 — `AccumulatorNet` enum doc-comment ("Dense is used only when the threshold check at construction time confirms that id_range <= 4 * expected_live_count") | Updated to `effective_arena_size <= 4 * expected_live_count` with the same SPEC-22 R22 D-011 forward reference. |
| SPEC-21 | 782 | §4.9 — `PartitionAccumulator` `min_assigned_id` / `max_assigned_id` field doc-comment ("the id_range > 4 * live_agent_count threshold check at finalize()") | Updated to `effective_arena_size > 4 * live_agent_count` with the same forward reference. |
| SPEC-21 | 852 | §4.9 — *Threshold contract* bullet ("when `id_range > 4 × live_agent_count` at finalize-time, SparseNet is mandatory ...") | Updated to `effective_arena_size > 4 × live_agent_count` with the supersession parenthetical. |
| SPEC-21 | 1068 | §8 Q6 (RESOLVED note) — "The dense path SHALL be rejected with `PartitionError::DenseAllocationExceedsThreshold` (SPEC-22 R30) if `id_range > 4 × live_agent_count` at finalize-time." | Updated to `effective_arena_size > 4 × live_agent_count` with the supersession parenthetical. |

Findings inside SPEC-22 + remediation:

| Surface | Action |
|---------|--------|
| SPEC-22 §3.2 R22 (line 173) | **Modified** — see §2 metric diff above. |
| SPEC-22 §3.4 R30 (line 223) | **Modified** — see §2 metric diff above. |
| SPEC-22 §3.8 A7 line 267 — predecessor-amendment narrative for SPEC-04 §4.5 build_subnet | UNCHANGED — A7 is a historical record of the SPEC-22 → SPEC-04 amendment authored at SPEC-22 v2 review time; it cites the original metric phrasing as part of the historical "New text" snapshot. The current normative cross-reference flows through SPEC-04 §4.5.1 (which I updated) and SPEC-22 R22 (which I updated). Re-writing A7's historical snapshot would distort the audit trail. |
| SPEC-22 §11 Change Log line 870 — SC-009 closure row ("R10a + R22 upgraded SHOULD → MUST under the `id_range > 4 × live_agent_count` threshold") | UNCHANGED — frozen Round 2 closure log entry, audit history. Same precedent as SPEC-21 line 1085 SC-006 closure row. The current normative R22 in §3.2 is the source of truth; SC-009 row is preserved as-was. |

Findings outside `codigo/relativist/specs/` were not in scope for this amendment (the brief explicitly limits propagation to spec-internal cross-references). Code-side identifier `id_range_size` in `relativist-core/src/error.rs:138-143` and `relativist-core/src/partition/helpers.rs:432-449` will be renamed in TASK-0612 of the partition-perf-fix bundle; this closure log establishes the normative basis.

## 4. Test floor expectation

**No change from spec amendment alone.** The amendment is normative-only; no spec-derived test additions or removals fire from this commit. Floors entering the D-011 partition-perf-fix bundle remain:

- `cargo test` (default profile): 1683
- `cargo test --features zero-copy`: 1726
- `cargo test --features streaming-no-recycle`: 1680
- v1 frozen floor (must never regress): 690

The implementation tasks 0612-0613 (in `docs/plans/2026-05-04-d011-partition-perf-fix.md` §5) WILL move the floor by:

- Net 0 from in-file UT rewrites (UT-0484-02..05 + UT-0492-01..02 are 1:1 reformulated against the new metric — see plan §5 Task 3).
- +1 from the new integration regression witness `relativist-core/tests/d011_partition_perf_witness.rs` (plan §5 Task 2).

So the post-implementation floor target is ≥ 1684 default / ≥ 1727 zero-copy / ≥ 1681 streaming-no-recycle (informational projection; the developer is the source of truth for actual counts post `cargo test`).

## 5. Spec-critic Round 1 — request

**Verdict requested:** `CONDITIONAL_PASS` (corrective amendment, not redesign).

**Rationale for requesting Round 1 despite the small surface:**

1. The amendment touches a core threshold decision that gates the build_subnet representation choice (SPARSE vs DENSE). Mis-stating the metric a second time would re-introduce the perf regression or, worse, regress correctness on M5 fragmentation cases.
2. The R22 amendment narrative claims the corrected metric "exactly" matches dense allocation behavior (`max_live_id + 1`). An adversarial second look should verify that this claim survives all R22-cited code paths, not just the one observed in the bisect.
3. The R22a clarification asserts that M5 pathology is still detected. An adversarial reader should pressure-test the worked example (10M live, 100M max_live_id ⇒ 100_000_001 > 40_000_000) against the original SC-009 evidence base (HVM2 / AC-006 — was the original "200K agents at 800 MiB" pathology really `max_live_id`-driven, or could there be edge cases where `id_range`-style fragmentation matters?).

**Pre-empted attack vectors** (from `docs/plans/2026-05-04-d011-partition-perf-fix.md` §8, replicated here for the spec-critic's convenience):

- **Q1 — Why `4×`?** Same calibration as before; the constant is unchanged; only the metric the constant multiplies changes. R22 narrative explicitly preserves the calibration.
- **Q2 — Workloads where `max_live_id ≪ id_range.end` AND in-round `create_agent` calls push past `4 × live`?** `create_agent` allocates from `next_id` upward within the partition's `id_range`; this happens DURING reduction, AFTER `build_subnet`. The threshold check is build-time. Post-build agent creation does NOT re-trigger the check. So in-round growth does not regress correctness or memory bounds beyond what was already true under the pre-D-011 metric.
- **Q3 — Boundary: empty partition (`live_count = 0`)?** R22a explicitly preserves the convention `worker_agents.is_empty() ⇒ effective_arena_size = 0 ⇒ threshold not exceeded ⇒ dense path returns `Net::new()`` (per `partition/helpers.rs:297-299`). No regression.
- **Q4 — Boundary: single live agent at `id = u32::MAX - 1`?** `effective_arena_size = u32::MAX as u64`; `4 × 1 = 4`; threshold is exceeded ⇒ SPARSE path. Correct: dense would allocate ~16 GB. Behavior matches intent.
- **Q5 — ABI break: error variant field rename `id_range_size → effective_arena_size`?** Spec-only amendment names the rename as a normative consequence (R30 SHOULD clause); the actual ABI change lands in TASK-0612 implementation. Grep audit for callers pattern-matching on `id_range_size` returns zero hits outside the variant definition + the in-file tests being rewritten in TASK-0612 Task 3 (per partition-perf-fix plan §8 Q5).
- **Q6 — Determinism (T7)?** R22 narrative includes an explicit T7 note: `worker_agents.iter().copied().max()` is order-independent on `&[u32]`; the slice is constructed deterministically by `split_with_config`. T7 holds.

**Brief expects** Round 1 to surface 0–2 LOW findings; CRITICAL/HIGH findings would indicate the amendment as written is insufficient and would block dispatch of TASK-0612 implementation work.

## 6. Files modified in this dispatch

| File | Edit summary |
|------|--------------|
| `codigo/relativist/specs/SPEC-22-arena-management.md` | Frontmatter `Status` line extended (Reviewed v2.2 → v2.3 with D-011 Amendment 2026-05-04 summary). §3.2 R22 modified (metric replaced; T7 note added; BLOCKER 2026-05-04 evidence cited). §3.2 R22a added (clarification — M5 pathology still detected; empty-partition convention preserved). §3.4 R30 modified (rejection rule wording + error variant SHOULD clause). §11 Change Log extended with new subsection "D-011 Amendment — 2026-05-04 — `effective_arena_size` threshold metric (closes BLOCKER 2026-05-04)". |
| `codigo/relativist/specs/SPEC-04-partition.md` | §4.5.1 Amendment A7 callout block: threshold-formula citation replaced with forwarding reference to SPEC-22 R22 (the canonical normative source) plus a parenthetical noting the D-011 supersession of the original `id_range`-based formulation. |
| `codigo/relativist/specs/SPEC-21-streaming-generation.md` | §3.3 / §4.9 PartitionAccumulator narrative + 3 doc-comment citations + §4.9 *Threshold contract* bullet + §8 Q6 (RESOLVED note): all five spots updated to use `effective_arena_size > 4 × live_agent_count` with a forwarding reference to SPEC-22 R22 D-011 Amendment 2026-05-04 and a parenthetical noting the supersession of the pre-D-011 `id_range`-based formulation. |
| `codigo/relativist/docs/spec-reviews/SPEC-22-amendment-2026-05-04-d011-blocker.md` | This closure log. |

## 7. Files explicitly NOT edited (deferred to D-011 partition-perf-fix Task 2 onward)

- `codigo/relativist/relativist-core/src/error.rs` — `DenseAllocationExceedsThreshold` field rename `id_range_size → effective_arena_size` + `#[error(...)]` format-string update. **Owner:** developer agent, TASK-0612 (plan §5 Task 4 step 2).
- `codigo/relativist/relativist-core/src/partition/helpers.rs:402-477` — `build_subnet_with_config` body + doc-comment rewrite per the new metric. **Owner:** developer agent, TASK-0612 (plan §5 Task 4 step 3).
- `codigo/relativist/relativist-core/src/partition/helpers.rs` (test mod) — UT-0484-02..05 + UT-0492-01..02 rewrite for scattered-ID workloads. **Owner:** developer agent, TASK-0612 (plan §5 Task 3).
- `codigo/relativist/relativist-core/tests/d011_partition_perf_witness.rs` — new integration regression witness. **Owner:** developer agent, TASK-0613 (plan §5 Task 2).
- `codigo/relativist/docs/benchmarks/d011-perf-fix-verification-2026-05-04.md` — bench verification post-fix. **Owner:** developer agent, TASK-0614 (plan §5 Task 5).
- `codigo/relativist/docs/next-steps.md` BLOCKER block migration to `progress.md`. **Owner:** developer agent, plan §5 Task 6.

These are developer-agent territory in subsequent stages of the bundle.

## 8. Brief status report (for parent agent)

R22 + R30 amended in-place; R22a added immediately after R22 with the M5-pathology-still-detected clarification; revision history entry "D-011 Amendment — 2026-05-04" appended to §11. Cross-references propagated to SPEC-04 §4.5.1 A7 (forwarding reference + supersession parenthetical) and SPEC-21 (5 spots: §3.3 paragraph, §4.9 enum doc-comment, §4.9 PartitionAccumulator field doc-comment, §4.9 *Threshold contract* bullet, §8 Q6). The two historical surfaces inside SPEC-22 (§3.8 A7 narrative snapshot, §11 SC-009 closure-log row) are intentionally left intact as audit trail. The closure log is this file. Spec-critic Round 1 is REQUESTED on this amendment with `CONDITIONAL_PASS` expected. Implementation hand-off TASK-0611..0614 is documented in `docs/plans/2026-05-04-d011-partition-perf-fix.md` §5; Stages 2-6 of the SDD pipeline pick up from there. No test-floor change from this dispatch alone; post-implementation floor target ≥ 1684 default / ≥ 1727 zero-copy / ≥ 1681 streaming-no-recycle (informational projection per plan §6 G2/G3/G4).
