# Handoff — SPEC-22 R22+R30 metric amendment (especialista-em-specs)

**Status:** READY TO DISPATCH
**Saved:** 2026-05-04
**Active bundle:** D-011 (SPEC-21/22 hardening). New BLOCKER 2026-05-04 (partition perf regression) requires this amendment as Stage 1 of the 6-stage SDD pipeline.
**Source plan:** `codigo/relativist/docs/plans/2026-05-04-d011-partition-perf-fix.md` (969L, §3 + §5 Task 1 + §8).

> **Per memory policy** (`feedback_especialista_specs_dispatch.md`): `especialista-em-specs` runs from the TCC root session, NOT the relativist subdir. Paste the §3 prompt verbatim into the TCC root Claude Code session.

---

## 1. State of SPEC-22

- Spec: `codigo/relativist/specs/SPEC-22-arena-management.md`, currently `Status: Reviewed v2` (closed via `92145a0` Stage 6 Wave 4 on 2026-04-27).
- D-009 (SPEC-22 implementation) closed 2026-04-27. D-010 (SPEC-21 streaming) closed 2026-04-30. D-011 (hardening) is the active bundle.
- **New finding 2026-05-04** (BLOCKER documented in `docs/next-steps.md`): empirical bisect + probe instrumentation revealed that the threshold metric `id_range > 4 × live_agent_count` (R22 / R30) measures the **planning range** (`id_range.end - id_range.start` from `compute_id_ranges`), not the **actual arena memory** that dense `build_subnet` allocates (which is `max_live_id + 1`). Result: every healthy partition workload is mis-routed to the SPARSE branch, causing a +83% wall-clock regression on `ep_con 5M w=2` (12 s → 22 s).
- Root cause is at the design level: the metric in the spec is wrong. A pure code fix without spec amendment would create spec drift.

## 2. What needs to change in SPEC-22

Three normative requirements need editing/adding:

| ID | Action | Summary |
|---|---|---|
| **R22** | Modify | Replace metric `id_range > 4 × live_agent_count` with `effective_arena_size > 4 × live_agent_count`, where `effective_arena_size := max_live_id + 1`. Add normative note that this metric matches the actual `Vec<Option<Agent>>` size that dense `build_subnet` allocates (line 301-303 of `partition/helpers.rs`). |
| **R30** | Modify | The rejection rule for `sparse_build = false` MUST use the new metric. The error variant `PartitionError::DenseAllocationExceedsThreshold` SHOULD report `effective_arena_size` (not `id_range_size`). |
| **R22a** (new) | Add | Clarification: M5 pathology (recycled-id fragmentation under delta mode) manifests as `max_live_id ≫ live_count` and is still detected by the new metric. Reference D-009 SC-009 closure rationale. |

**Cross-references to check (best-effort grep before editing):**

```
grep -rn "id_range.*4.*live\|id_range_size.*4\|4.*live_agent_count" codigo/relativist/specs/
```

If SPEC-04 §A4 / R10a or SPEC-19 §3.6 R41 mentions the old formulation, update those too.

## 3. Closure expectations

- **Spec status field:** SPEC-22 stays `Reviewed v2` post-amendment (this is a normative-clarity fix, not a re-architecture). Document the amendment date and rationale in the spec's revision history (or add a new §X "D-011 Amendment 2026-05-04" subsection if no revision history exists yet).
- **Closure log:** `codigo/relativist/docs/spec-reviews/SPEC-22-amendment-2026-05-04-d011-blocker.md` — short (50–80 lines) — record (a) the bisect+probe evidence, (b) the metric change, (c) the cross-references audited, (d) the test floor expectation (no change from spec amendment alone).
- **spec-critic Round 1:** REQUESTED for this amendment (small surface, but it touches core threshold semantics; better to have an adversarial second look). Verdict expected: `CONDITIONAL_PASS` (this is a corrective amendment, not a redesign).

## 4. Agent prompt (paste verbatim into especialista-em-specs from TCC root session)

```
Mode: Targeted amendment to SPEC-22 (Arena Management). Status currently
"Reviewed v2" — this amendment corrects a normative metric without
re-opening Round 2. Empirical evidence for the change is in
codigo/relativist/docs/next-steps.md BLOCKER 2026-05-04.

INPUTS (read in this order):
1. codigo/relativist/specs/SPEC-22-arena-management.md — target spec.
   Focus on §3.4 R22, R30. Current text contains the formula
   "id_range > 4 × live_agent_count".
2. codigo/relativist/docs/next-steps.md — read the entire BLOCKER
   2026-05-04 block (top of the file). The "Root cause" subsection
   has the bisect transcript; the "Why this slipped through review"
   subsection has the misalignment analysis.
3. codigo/relativist/docs/plans/2026-05-04-d011-partition-perf-fix.md —
   §1 Background, §3 Handoff brief (DO NOT skip §3, it spells out the
   exact change), §8 Open Questions (predicted spec-critic attack
   vectors with pre-empted answers).
4. codigo/relativist/relativist-core/src/partition/helpers.rs —
   read lines 280-477 to ground the spec text in the actual code:
   - Line 289-307: dense `build_subnet` arena allocation
     (`agents_len = max_id + 1`, NOT `id_range.end`).
   - Line 351-359: free_list iteration clamped to arena_len.
   - Line 422-477: current `build_subnet_with_config` (the call site
     that uses the broken metric).

CHANGES TO MAKE in specs/SPEC-22-arena-management.md:

A. Edit R22: replace the "id_range > 4 × live_agent_count" formula
   with "effective_arena_size > 4 × live_agent_count", where
   effective_arena_size = max_live_id + 1. Cite that this matches
   the dense `build_subnet` allocation behavior at line 301-303 of
   partition/helpers.rs (the actual implementation reality). Keep
   the 4× constant unchanged — only the metric changes.

B. Edit R30: update the rejection rule wording to use the new metric.
   The error variant SHOULD now carry `effective_arena_size`, not
   `id_range_size`. (The code-side rename happens in TASK-0612 of the
   D-011 fix plan; this amendment establishes the normative basis.)

C. Add R22a: a short clarification rule stating that M5 pathology
   (the original motivation of R22 — recycled-id fragmentation under
   delta-mode without compaction) is still detected by the new metric
   because it manifests as max_live_id ≫ live_count. Reference D-009
   SC-009 closure.

D. Update worked example in §3.4 (if any) to derive the threshold
   under the new metric.

E. Cross-reference grep + update — run:
   grep -rn "id_range.*4.*live\|id_range_size.*4" codigo/relativist/specs/
   For every hit outside SPEC-22 (e.g., SPEC-04 R10a, SPEC-19 §3.6 R41
   if present), update to the new metric or add a forwarding note.

F. Add a revision-history entry (or a new §X "D-011 Amendment 2026-05-04"
   subsection) recording: date, motivation (cite docs/next-steps.md
   BLOCKER 2026-05-04), scope (R22+R30+new R22a), impact (no test floor
   change from spec alone; implementation TASKs 0611-0614 follow).

G. Write closure log to
   codigo/relativist/docs/spec-reviews/SPEC-22-amendment-2026-05-04-d011-blocker.md
   following the format of prior amendment closure logs in
   docs/spec-reviews/. Sections: bisect summary, metric diff, cross-refs
   audited, test floor expectation, request for spec-critic Round 1.

OUT OF SCOPE — DO NOT TOUCH:
- compute_id_ranges (`base_next_id × 10` chunk multiplier) — separate
  amendment if ever needed; not part of this fix.
- R23 (CI lint forbidding SparseNet in reduction/) — orthogonal.
- The `4 ×` constant — only the metric the constant multiplies changes.
- Wire format / PROTOCOL_VERSION — this is build-time only, no wire
  field involved.
- SparseNet itself (§4.6 to_dense) — the SPARSE path remains, just
  taken less often.

PRE-EMPTED ATTACK VECTORS (already answered in
docs/plans/2026-05-04-d011-partition-perf-fix.md §8 — read those if you
want to defend the amendment to spec-critic Round 1):
- Q1 Why 4×? — Same calibration as before; metric changes, multiplier
  doesn't.
- Q3 Empty partition? — effective_arena_size = 0, threshold not
  exceeded, dense returns Net::new() (line 297-299).
- Q4 Boundary: single agent at id = u32::MAX-1? — Threshold IS
  exceeded → SPARSE; correct (dense would alloc ~16 GB).
- Q6 T7 determinism? — `worker_agents.iter().max()` is order-
  independent on `&[u32]`; sigma is constructed deterministically by
  split_with_config; T7 holds.

DELIVERABLES:
1. Updated specs/SPEC-22-arena-management.md (R22, R30, +R22a, revision
   history).
2. Closure log
   codigo/relativist/docs/spec-reviews/SPEC-22-amendment-2026-05-04-d011-blocker.md.
3. Cross-reference updates in any other specs that cite the old metric
   (likely SPEC-04, possibly SPEC-19 — verify with grep first).
4. ONE commit, message format (per repo convention `git-commits` skill):

   spec(d-011): amend SPEC-22 R22+R30 — effective_arena_size threshold metric

   Replaces the old `id_range > 4 × live_agent_count` metric with
   `effective_arena_size > 4 × live_agent_count` where
   effective_arena_size = max_live_id + 1, matching dense build_subnet's
   actual allocation. Adds R22a clarifying that M5 pathology is still
   detected. Closes the spec-side of BLOCKER 2026-05-04 (see
   docs/next-steps.md). Implementation in TASKs 0611-0614 follows.

   Co-Authored-By: <maintainer or AI signature per repo convention>

After commit, return control to the user with a one-line summary of:
(a) which files were modified, (b) cross-spec changes (if any), and
(c) whether spec-critic Round 1 should be dispatched immediately.
```

---

## 5. Post-amendment workflow (for the user)

After the agent commits the amendment:

1. **Optional:** dispatch `spec-critic` Round 1 for the amendment (recommended — small surface, low cost, high confidence). Verdict expected `CONDITIONAL_PASS`.
2. Return to relativist subdir; resume the D-011 fix plan from Task 2 onward (`docs/plans/2026-05-04-d011-partition-perf-fix.md` §5).
3. Tasks 2–6 are `developer`-territory and can be executed in the relativist subdir session.
