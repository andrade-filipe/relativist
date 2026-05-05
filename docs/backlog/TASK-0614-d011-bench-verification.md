# TASK-0614 — D-011 bench verification: confirm v1 wall-time restored on `ep_con 5M w=2 local`

**Phase:** D-011 BLOCKER 2026-05-04 — Stage 6 (post-fix verification, gates BLOCKER closure)
**Bundle:** D-011 — Tier 3 Hardening + Bench Enablement
**Status:** OPEN
**Priority:** P0 (the empirical gate that closes the BLOCKER — without this, the fix is unproven)
**Spec:** SPEC-22 v2.4 §3.4 R22 (operationalized via the 12 s wall-time target derived from v1 baseline). No new spec authority required.
**Origin:** `docs/plans/2026-05-04-d011-partition-perf-fix.md` §5 Task 5 + §6 Gate G8.
**Estimated complexity:** S — no code changes; ~30 LoC of bench harness shell + ~40 lines of markdown verification report. Wall-clock budget: ~10 min runtime (3 reps × 2 builds × ~22 s + setup).

---

## Context

This task is the **empirical proof** that the TASK-0612 fix actually restores the v1 wall-time on the BLOCKER's canonical workload (`ep_annihilation-con 5M w=2 local`, 3-rep median). Without this measurement, the BLOCKER cannot be marked CLOSED — the unit tests in TASK-0612/0613 prove the *correctness* of the metric (which branch is taken), but only the bench proves the *performance* (wall-time at the v1 baseline).

The methodology mirrors the bisect transcript in `docs/next-steps.md` BLOCKER 2026-05-04 (historical lines 79–155). Two binaries are timed under matched conditions:

| Binary | Path | Pre-built? |
|---|---|---|
| v1 baseline | `/tmp/r-bisect/v1/target/release/relativist.exe` | YES — already built during the bisect |
| HEAD post-fix | `target/release/relativist.exe` (built fresh after TASK-0612 lands) | NO — must rebuild on the post-fix HEAD |

Each binary runs `local --workers=2 -i <ep5m_input.bin>` 3 times; the median is the reported wall-time. Inputs are regenerated on HEAD post-fix to ensure they match the current binary format (the input format has not changed in D-011, but a regenerate+match is the safe convention used in the bisect protocol).

Pass criterion (plan §6 G8): **HEAD post-fix wall ≤ 1.10 × v1 wall** (within 10 %). v1 baseline was measured at ~12 s during the bisect; current HEAD pre-fix is 22.88 s (+83 %). Expected post-fix: ~12–13 s.

If the gate fails (HEAD post-fix is still >1.10 × v1), the fix is incomplete. STOP, do NOT close the BLOCKER, and re-open the investigation. Possible follow-ups: re-examine `compute_id_ranges` (Option A in plan §2 — previously rejected; would become the next candidate), or hunt for a second performance regression hidden behind the partition fix.

## Dependencies

- **Prereq:** TASK-0612 (fix lands first — this task measures its impact). Bench cannot run before the fix exists.
- **Prereq (artifact):** `/tmp/r-bisect/v1/target/release/relativist.exe` exists from the original bisect. If it has been deleted, rebuild from `v1-feature-complete` (tag `v0.10.0-bench`) per the bisect protocol — adds ~3 min to the runtime.
- **Indirect prereq:** TASK-0611 (spec anchor) and TASK-0613 (correctness witness) — both should be CLOSED before this task runs, otherwise the bench is being run against an incomplete chain.

## Files in scope

| File | Change |
|------|--------|
| `docs/benchmarks/d011-perf-fix-verification-2026-05-04.md` | **CREATE.** Verification report. Table with rows for `v1-feature-complete`, `HEAD pre-fix (commit 2b6528b)`, `HEAD post-fix (commit <NEW>)`, columns for `ep_con 5M w=2 wall (median of 3)` and `partition_time_per_round`. Includes Verdict (PASS/FAIL + ±N% deviation from v1), hardware/conditions block (Windows 11, performance mode, build flags), and reference back to `docs/next-steps.md` BLOCKER 2026-05-04 transcript. Template in plan §5 Task 5 Step 3 (lines 822–836). |
| (no code changes) | — |

## Files explicitly OUT of scope

- `docs/next-steps.md` — closing the BLOCKER block is plan §5 Task 6 (separate task; not included in this 4-file batch).
- `docs/progress.md` — same reason.
- Any `relativist-core/`, `relativist-cli/`, or `relativist-net/` source — this is a measurement task only.
- Test files — TASK-0612 + TASK-0613 cover correctness.
- The bisect itself — already performed (`docs/next-steps.md` historical block); this task **re-runs** the same workload on the post-fix binary.

## Acceptance criteria

1. File `docs/benchmarks/d011-perf-fix-verification-2026-05-04.md` exists.
2. Report contains a table with three rows (v1, HEAD pre-fix, HEAD post-fix) and at minimum the wall-time column populated for all three (the `partition_time_per_round` column may be `-` if the bench harness does not surface it; not blocking).
3. **Gate G8 (plan §6):** HEAD post-fix median wall-time ≤ 1.10 × v1 median wall-time on `ep_annihilation-con 5M w=2 local`.
4. Hardware/conditions block records: OS (`Windows 11`), performance-mode setting, `cargo` build profile (`--release`), and any `CARGO_PROFILE_RELEASE_DEBUG` setting used (matches the bisect protocol — `line-tables-only` was used pre-fix for parity).
5. Verdict line reads `**Verdict:** PASS — HEAD post-fix is within ±N % of v1` (or `FAIL` with explanation).
6. Reference link to `docs/next-steps.md` BLOCKER 2026-05-04 + cross-link to TASK-0611 / TASK-0612 / TASK-0613 commit hashes (the SPEC-amend chain `e941273` / `972ce47` / `5cca7e6` plus the TASK-0612 implementation hash, plus TASK-0613 witness hash).

## Test floor delta

**0** — no test files added, no inline test changes. This task is documentation + measurement only. The unit-test floor (1683 default / 1726 zero-copy / 1680 streaming-no-recycle) is unchanged from TASK-0612 + TASK-0613 net delta (+1 default).

## Key risks

- **Runner variability:** Windows 11 wall-time measurements have ±5–10 % noise even with performance-mode pinned. The 10 % gate (G8) is calibrated for this; tightening to <5 % would require deeper environmental control (CPU pinning, frequency lock, background process kill) that is out of scope for a single bench run.
- **v1 binary missing:** if `/tmp/r-bisect/v1/target/release/relativist.exe` was cleaned, the rebuild from `v1-feature-complete` adds ~3 min and a separate verification step (re-confirm v1 wall ≈ 12 s on the rebuilt binary before comparing HEAD).
- **Input drift:** if the binary input format silently changed in D-011 (it shouldn't have; verify with `grep -rn "PROTOCOL_VERSION" relativist-core/src/io/` to confirm no bumps), the v1 binary cannot read the HEAD-generated input. Mitigation already in plan §5 Task 5 Step 1: regenerate the HEAD input separately (`/tmp/r-bisect/ep5m_HEAD_postfix.bin`); v1 keeps its own input (`/tmp/r-bisect/ep5m_v1.bin`).
- **Gate failure:** if G8 fails, do NOT close the BLOCKER. Re-open investigation; first candidate is `compute_id_ranges` chunk multiplier (plan §2 Option A, previously rejected).

## Notes

- Verbatim bench shell harness (Python-via-anaconda subprocess timer for portability with the user's environment) is in plan §5 Task 5 Step 2 (lines 800–814).
- Verbatim verification-report markdown template is in plan §5 Task 5 Step 3 (lines 820–837).
- Verbatim commit message for the bench-only commit is in plan §5 Task 5 Step 4 (lines 841–849).
- Bisect transcript reference: `docs/next-steps.md` BLOCKER 2026-05-04 historical detail (lines 37–155 of current `next-steps.md`).
- Cross-link: TASK-0611 (spec, CLOSED), TASK-0612 (fix, prereq), TASK-0613 (correctness witness, parallel-prereq), and the next-steps closure (plan §5 Task 6 — out of scope for this 4-file batch).
- After this task PASSES G8, the BLOCKER block in `docs/next-steps.md` can be moved to `docs/progress.md` per project policy. That move is a separate task (not included in this batch).
