# v2_pre_fix_baseline — Campaign Manifest

**Status:** FROZEN reference — pre-D-011-fix snapshot, retained as the
companion delta against `v2_d011_final_baseline_2026-05-04/`. This is
the v2 dataset that surfaced the partition-perf regression which led
to the D-011 BLOCKER investigation.

## Provenance

| Field | Value |
|---|---|
| Repository | `github.com/andrade-filipe/relativist` |
| Branch | `v2-development` |
| HEAD at run | pre-`62de30f` (i.e. before TASK-0612 + Bug 1 + Bug 2 fix bundle landed; see `docs/progress.md` 2026-05-04 entry) |
| Run date | 2026-05-02 (per CSV mtime) |
| Hardware | Lenovo ThinkPad T14 Gen 4 (i7-1365U), 31.7 GiB RAM — same as v1 + post-fix baselines |
| Toolchain | rustc 1.94.1, cargo 1.94.1 |
| Power scheme | (not recorded at the time; the post-fix run used Ultimate Performance — this run was likely Balanced or Better Battery, contributing to the relative slowdown but **not explaining** the 4 failed slots) |
| Mode | Docker `tcp_localhost` via `bench_docker_v2.sh` + native sequential |
| Reps × configs | 10 × 32 dist + 10 × 8 seq |
| Failures observed | **4 slots had reduced rep counts or `all_correct=false`:** `ep_annihilation_con 5M w=1` (3/10 reps, false), `5M w=2` (8/10), `5M w=4` (8/10), `dual_tree 22 w=2` (9/10, false). These failures are the empirical signature of Bug 1 (`freeport_redirects` lost) + Bug 2 (`next_id = 0` causing cross-partition AgentId collisions) in the dense `build_subnet` path. |

## Why this is preserved

The wall-time deltas (post-fix / pre-fix) form the `post/pre` column in
the analysis at `docs/analysis/D011-final-baseline-analysis-2026-05-04.md`.
That column is the empirical proof that the D-011 fix bundle improves
both correctness (RF-08) and wall-time (median ratio 0.55) on every
distributed slot. Discarding this snapshot would erase the only
controlled measurement of the bug's wall-clock cost.

## Cross-references

- Final post-fix baseline: `results/locked/v2_d011_final_baseline_2026-05-04/MANIFEST.md`
- D-011 investigation arc: `docs/progress.md` 2026-05-04 entry
- Cold post-mortem analysis: `docs/analysis/D011-final-baseline-analysis-2026-05-04.md`
- Spec at run-time: `specs/SPEC-22-arena-management.md` v2.3 (pre-amendment)
- Fix commit: `62de30f`
