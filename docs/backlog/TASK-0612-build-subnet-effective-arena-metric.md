# TASK-0612 — `build_subnet_with_config` effective_arena_size metric + error field rename + UT rewrites (D-011 BLOCKER fix)

**Phase:** D-011 BLOCKER 2026-05-04 — Stage 3+ (DEV / REVIEW / QA / REFACTOR)
**Bundle:** D-011 — Tier 3 Hardening + Bench Enablement
**Status:** OPEN
**Priority:** P0 (BLOCKER fix — restores v1 wall-time on `ep_con 5M w=2 local`)
**Spec:** SPEC-22 v2.4 §3.4 R22, R30, R22a (anchor TASK-0611).
**Origin:** `docs/plans/2026-05-04-d011-partition-perf-fix.md` §5 Tasks 3+4 (collapsed; see "Atomicity note" below).
**Estimated complexity:** ~210 LoC total — ~10 LoC production in `error.rs`, ~50 LoC production in `partition/helpers.rs` (lines 402–477 doc + body), ~150 LoC test rewrites in the existing inline test mod (5 tests rewritten at lines 1340–1545; same names, same count — net ±0 tests).

---

## Context

This task implements the SPEC-22 v2.4 amendment (TASK-0611) in code. Three concrete changes ship in a single commit because they are mutually dependent (the field rename creates a transient API inconsistency that cannot be split across commits without breaking the build):

1. **Error variant field rename** in `relativist-core/src/error.rs`: `PartitionError::DenseAllocationExceedsThreshold { partition_index, id_range_size, live_count }` → `{ partition_index, effective_arena_size, live_count }`. Format string updated; doc-comment cites SPEC-22 R30 (D-011 amendment 2026-05-04).
2. **Metric rewrite** in `relativist-core/src/partition/helpers.rs:422-477` (`build_subnet_with_config` body): replace `let id_range_size = (id_range.end as u64).saturating_sub(id_range.start as u64);` with `let effective_arena_size: u64 = worker_agents.iter().copied().max().map(|max_id| max_id as u64 + 1).unwrap_or(0);`. Doc-comment block (lines 402–420) rewritten with the new rationale + reference to dense allocation site at `partition/helpers.rs:301-303`.
3. **Inline UT rewrites** in `relativist-core/src/partition/helpers.rs` test mod (lines 1340–1545): UT-0484-03, UT-0484-04, UT-0484-05 (renamed to `error_field_effective_arena_size_correct`), UT-0492-01, UT-0492-02 — switched from densely-packed live-IDs (which only tripped the OLD metric via `id_range.end`) to scattered live-IDs (which trip the NEW metric via `max_live_id`). All 5 tests retain their original names except UT-0484-05 which renames its function to reflect the renamed field.

Pre-fix probe data: `docs/next-steps.md` BLOCKER 2026-05-04 (lines 9–35 + historical lines 37–155). Post-fix expectation: `ep_con 5M w=2 local` wall returns to v1 baseline of ~12 s (from current 22 s, +83%).

## Atomicity note (~210 LoC, just over the 200 LoC budget)

This task is intentionally **kept as a single atom** despite estimating ~210 LoC, because the field rename in `error.rs` and the call-site update in `helpers.rs` create a transient inconsistency: any commit that lands one without the other fails to compile. Splitting into 0612a (metric+rename) and 0612b (inline-UT rewrites) is mechanically possible — the UT rewrites don't depend on the production change for compilation IF the field name in the new tests matches whatever ships first — but the UT rewrites are the only caller-site validation of the new metric semantics, so committing them separately costs a CI cycle without providing review benefit.

**Recommendation:** keep as one commit. If the reviewer judges atomicity violated, the safe split is:
- 0612a (~60 LoC production): `error.rs` rename + `helpers.rs` body+doc rewrite. Floor: 1683 (5 inline tests still pin OLD metric and FAIL — unacceptable; therefore 0612a alone cannot land).
- 0612b (~150 LoC test): inline UT rewrites. Must follow 0612a in the same review cycle.

The split is therefore artificial; auto-mode should commit as one atom.

## Dependencies

- **Prereq:** TASK-0611 (SPEC-22 v2.4 amendment) — CLOSED. Without R22/R30/R22a in their amended form, this implementation has no normative anchor.
- **Parallel:** TASK-0613 (`tests/d011_partition_perf_witness.rs`) — TDD-RED witness. MUST land BEFORE this task in commit order so the bug is captured before the fix. After TASK-0612 lands, TASK-0613's witness FLIPS from FAIL to PASS — the canonical proof the regression is closed.
- **Downstream:** TASK-0614 (bench verification) — consumes the post-fix binary.

## Files in scope

| File | Change |
|------|--------|
| `relativist-core/src/error.rs` | Rename `id_range_size` → `effective_arena_size` field on `PartitionError::DenseAllocationExceedsThreshold`. Update `#[error(...)]` format string. Add doc-comment citing SPEC-22 R30 (D-011 amendment) + reference to BLOCKER 2026-05-04. (~10 LoC.) |
| `relativist-core/src/partition/helpers.rs` (lines 402–477) | Rewrite `build_subnet_with_config` doc-comment block (lines 402–420) with new rationale and reference to dense allocation at lines 301–303. Replace metric computation (lines 432–434): drop `id_range_size`, compute `effective_arena_size` from `worker_agents.iter().copied().max()`. Update error-construction site (lines 438–444) to pass `effective_arena_size`. (~50 LoC production change.) |
| `relativist-core/src/partition/helpers.rs` (test mod, lines 1340–1545) | Rewrite UT-0484-03 (`sparse_build_false_above_threshold_rejects`), UT-0484-04 (`sparse_build_true_above_threshold_uses_sparse_path`), UT-0484-05 (renamed to `error_field_effective_arena_size_correct`), UT-0492-01 (`sparse_path_taken_above_threshold`), UT-0492-02 (`dense_path_taken_below_threshold`) to use scattered live-ID workloads (50 agents created, every-5th retained → max_live_id ≥ 4 × live_count). Verbatim test bodies in plan §5 Task 3 Steps 1–4. (~150 LoC test rewrite — same test count, same names except UT-0484-05.) |

## Files explicitly OUT of scope

- `relativist-core/tests/d011_partition_perf_witness.rs` — created by TASK-0613.
- `compute_id_ranges` chunk_size formula — separate concern (Option A in plan §2; rejected).
- Historical TASK-0484, TASK-0492, TASK-0552 backlog docs — closed, ship-time historical record (drift by design).
- TEST-SPEC-T16 / TEST-SPEC-0484 / TEST-SPEC-0492 / TEST-SPEC-0552 in `docs/tests/` — historical record (drift by design).
- Cross-references in SPEC-04 §A4 R10a / SPEC-19 §3.6 R41 — verified by especialista-specs in TASK-0611 (no normative drift).
- `docs/tests/TEST-SPEC-0612.md` — produced by Stage 2 test-generator, NOT this task.

## Acceptance criteria

1. `cargo build --release -p relativist-cli` — exit 0.
2. `cargo test` — passes; floor `≥ 1683` default.
3. `cargo test --features zero-copy` — passes; floor `≥ 1726`.
4. `cargo test --features streaming-no-recycle` — passes; floor `≥ 1680`.
5. `cargo clippy --all-targets --all-features -- -D warnings` — clean.
6. `cargo fmt --check` — clean.
7. The TASK-0613 witness `cargo test --release --test d011_partition_perf_witness` — PASSES (was FAILING pre-fix on the witness commit).
8. `grep -rn "id_range_size" relativist-core/src/` returns 0 hits in production paths (the symbol exists nowhere except possibly historical comments). The only remaining occurrence allowed is in the rewritten doc-comment context where it appears in prose explaining the OLD metric.
9. `PartitionError::DenseAllocationExceedsThreshold` payload's second field is named `effective_arena_size` (verified by `grep`).

## Test floor delta

**Net 0** for this task. Five inline tests rewritten (same names except UT-0484-05 renamed), zero added inline. The `+1` regression witness in `tests/d011_partition_perf_witness.rs` ships in TASK-0613, not here.

## Key risks

- **ABI:** `PartitionError::DenseAllocationExceedsThreshold` field rename is a breaking change for any caller that pattern-matches on `id_range_size`. Plan §8 Q5 confirmed via grep: 0 callers outside the variant definition + the inline tests we're rewriting. Safe.
- **Determinism (T7):** `worker_agents.iter().copied().max()` is order-independent on `&[u32]`. T7 preserved (plan §8 Q6).
- **Empty partition:** `worker_agents.is_empty()` → `effective_arena_size = 0`, `threshold_exceeded = false`, dense returns `Net::new()` at lines 297–299. Documented in the new doc-comment block.
- **Single-agent boundary at u32::MAX-1:** `effective_arena_size ≈ u32::MAX`, `4 × 1 = 4`, threshold exceeded → SPARSE. Correct (plan §8 Q4) — dense would allocate ~16 GB.
- **Atomicity:** see "Atomicity note" above. Recommend committing as one.

## Notes

- Verbatim implementation steps (with line numbers, expected `tail -3` outputs, and ready-to-paste code blocks) are in `docs/plans/2026-05-04-d011-partition-perf-fix.md` §5 Task 3 (Steps 1–6) + Task 4 (Steps 1–10).
- The combined commit message template is in plan §5 Task 4 Step 10 (lines 737–779).
- Pre-fix bisect transcript: `docs/next-steps.md` historical BLOCKER block (lines 37–155).
- Verification gates G1–G7 from plan §6 are subsumed into acceptance criteria 1–7 above; G8 (bench) is TASK-0614.
- Rollback procedure: see plan §7 — revert this commit; SPEC-22 amendment (TASK-0611) stays as the corrected design intent.
- Cross-link: TASK-0611 (prereq, CLOSED), TASK-0613 (parallel TDD-RED, ships before this in commit order), TASK-0614 (downstream bench, consumes post-fix HEAD).
