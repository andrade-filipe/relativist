# v1_local_baseline — Frozen Local Baseline for Phase 3 LAN Comparison

This directory contains the **frozen local-only baseline** for Relativist,
produced on the tagged binary `v0.10.0-bench`. It is the reference the
upcoming Phase 3 LAN campaign will subtract from to isolate network cost.

**Do not modify any file in this directory.** To reproduce the campaign on
a different machine, run `scripts/reproduce_local_baseline.sh` — wall-clock
values will diverge (different hardware), but row counts, correctness
flags, and round structure must match.

## Contents

| File | Description |
|---|---|
| `manifest.md` | Full provenance: commit SHA, tag, hardware, timestamps, sha256 checksums, campaign knobs |
| `phase1_lenient_detail.csv` | Phase 1 (in-process) per-repetition rows, lenient BSP (default) |
| `phase1_lenient_rounds.csv` | Phase 1 per-round rows, lenient BSP |
| `phase1_lenient_summary.csv` | Phase 1 aggregate stats (mean, std, median, CV), lenient BSP |
| `phase1_strict_detail.csv` | Phase 1 per-repetition rows, strict BSP (cascade_cross, dual_tree subset) |
| `phase1_strict_rounds.csv` | Phase 1 per-round rows, strict BSP — multi-round data for Phase 3 |
| `phase1_strict_summary.csv` | Phase 1 aggregate stats, strict BSP |
| `phase2_detail.csv` | Phase 2 (Docker / TcpLocalhost) per-repetition rows |
| `phase2_rounds.csv` | Phase 2 per-round rows |
| `phase2_summary.csv` | Phase 2 aggregate stats |
| `cv_triage.md` | CV > 0.15 datapoints flagged with keep/rerun/exclude disposition |
| `raw/phase1/` | Per-benchmark stdout/stderr logs and raw CSV fragments |
| `raw/phase2/` | Per-run metrics.json snapshots from Docker runs |

## Scope note

- **Correctness method:** Full G1 isomorphism for all benchmarks except
  `condup_expansion` at sizes 10k and 50k, where the `--skip-g1` weak
  check (agent count equality) is used by default (abordagem A, see
  `USAGE_GUIDE.md` Section 11.5). The optional full-G1 overnight
  verification (abordagem B) is documented there for voluntary replication.
- **Strict BSP data:** Only `cascade_cross` (all default sizes) and
  `dual_tree` (sizes 6, 10, 14) have strict-mode rows; all other Phase 1
  benchmarks run in the default lenient mode.
- **Phase 2 L6:** Configs that were previously blocked by the 256 MiB
  frame cap (`dual_tree=22` at w=1, `ep_annihilation_con=5M` at w∈{1,2,4})
  now complete within the 1800s timeout thanks to the CompactSubnet fix
  shipped with v0.10.0-bench.
