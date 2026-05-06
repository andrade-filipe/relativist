# D-014 Stress Curve Campaign — SENTINEL

This directory is a **sentinel** for the D-014 stress-curve campaign
(TASK-0708). It is NOT a locked dataset; the actual locked dataset
will live at `results/locked/v2_stress_curve_<YYYY-MM-DD>/` once the
operator runs the campaign.

## Why this sentinel exists

TASK-0708 is the **execution + lock** task of the D-014 bundle. The
infrastructure (TASK-0700..0707) is committed and tested; the
overnight ~7-8h campaign run is intentionally NOT performed by an
agent — the operator (Filipe) presses "go" on a workstation that
satisfies the pre-condition gate documented in
`docs/benchmarks/campaigns/stress-curve.md` §5.1, then audits the
result before locking.

This `SENTINEL.md` exists so:
1. The TCC root `progress.md` can refer to "D-014 ready, run pending"
   without committing a fake locked dir.
2. `docs/INDEX.md` and `docs/ROADMAP.md` §2.16 already reference the
   stress-curve methodology; readers landing here see the canonical
   pointer to "how to run the real campaign".
3. The operator does not have to remember the invocation —
   §"How to invoke" below is the canonical command list.

## How to invoke the real campaign

From a clean checkout on `feature/stress-and-encoder` (or whatever
branch the bundle merges onto):

```bash
# 1. Pre-flight (each step exit 0):
cd codigo/relativist
cargo test --release
cargo test
cargo test --features zero-copy
cargo test --features streaming-no-recycle
cargo clippy --all-features -- -D warnings
cargo fmt --check
cargo build --release
scripts/stress_curve.sh --smoke

# 2. Full campaign (~7-8h overnight). Pick a fresh day-stamp:
scripts/stress_curve.sh \
  --output-dir results/locked/v2_stress_curve_$(date -I)

# 3. Audit (after wake-up):
LOCKED=results/locked/v2_stress_curve_$(date -I)
ls "$LOCKED/figures/"
cat "$LOCKED/MANIFEST.md"
sha256sum -c "$LOCKED/checksums.sha256"

# 4. Sanity checks (manual; design doc §8) — see §8 of
#    docs/benchmarks/campaigns/stress-curve.md for the 6-item list.
#    If any fails: STOP, file as a QA blocker, do NOT lock the dir.

# 5. Lock the directory:
git add "$LOCKED"
git commit -m "feat(bench): TASK-0708 stress-curve campaign locked dataset $(date -I)

Locked output of the D-014 stress-curve campaign. Methodology in
docs/benchmarks/campaigns/stress-curve.md. MANIFEST.md inside the
directory carries the git SHA, rustc/cargo versions, environment
snapshot, and any operator-noted anomalies.
"

# 6. Update the documentation pointers:
#    - docs/INDEX.md: replace the methodology link with the locked
#      dir under Benchmark Results.
#    - docs/ROADMAP.md §2.16: amend the "PENDING run" paragraph
#      with observed N_max ± noise per (workload, env, W).
#    - docs/next-steps.md: move D-014 from Active to Closed.
#    - CHANGELOG.md: add a "Locked: v2_stress_curve_$(date -I)"
#      sub-bullet under [Unreleased] D-014.
#    - docs/backlog/BACKLOG.md: move all 9 D-014 TASKs to a "D-014
#      delivered" row in the Cumulative bundles section.
#    - git mv docs/backlog/TASK-070*.md docs/backlog/archive/

# 7. The merge to `main` is the OPERATOR's call (not the agent's).
#    Push the branch, eyeball the figures, decide if the dataset
#    matches expectations, then merge.
```

## What is OUT of scope for this sentinel

- The script does NOT auto-`git add` or `git commit` — the operator
  audits before any commit.
- The agent does NOT run the campaign. ~7-8h on real hardware is
  not an agent task; the operator owns the wall-clock budget.
- This sentinel directory itself can be removed in the same commit
  that lands the locked directory (it serves no purpose afterwards).

## Files in this sentinel

- `SENTINEL.md` — this file.

(Empty otherwise; the orchestrator never writes here.)

## Cross-references

- TASK-0708: `docs/backlog/TASK-0708-stress-curve-campaign-run-and-lock.md`
- Methodology: `docs/benchmarks/campaigns/stress-curve.md`
- Orchestrator: `scripts/stress_curve.sh`
- Plot generator: `scripts/plot_stress_curve.py`
- Design doc: `docs/superpowers/specs/2026-05-05-stress-test-large-nets-design.md`
