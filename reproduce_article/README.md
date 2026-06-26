# `reproduce_article/` — Reproducibility package for the TCC article

This directory exists **only because of the academic article** that introduced Relativist:

> *Interaction Combinators as a Formal Model for Distributed Reduction in Grid Computing* —
> Filipe Andrade Nascimento, advised by Yuri Faro Dantas de Sant'Anna (UNIT, 2026).

Everything required to reproduce the empirical claims of that article — the frozen benchmark
evidence, the scripts that produced it, and the checksums that pin it — lives here, isolated from
the evolving source tree. **The application code (`relativist-core`, `relativist-cli`) will keep
changing; this folder is a frozen snapshot tied to the paper.** If a future version of Relativist
no longer reproduces these numbers bit-for-bit, that is expected — pin to the tag below.

> **Reproduce against the tagged binary, not `main`.** The canonical evidence was produced at
> tag **`v0.10.0-bench`** (v1) and the post-D-012 v2 baseline. Wall-clock numbers depend on
> hardware; **structural** numbers (correctness, rounds, bytes) are reproducible exactly.

## Layout

```
reproduce_article/
├── README.md                 ← you are here
├── scripts/                  ← the exact scripts that produced the locked data
│   ├── reproduce_local_baseline.sh   ← headline: re-run Phase 1+2 and diff vs frozen
│   ├── bench_phase1_locked.sh        ← Phase 1 (in-process) campaign
│   ├── bench_phase2_locked.sh        ← Phase 2 (Docker/TCP localhost) campaign
│   ├── bench_phase3_lan.sh           ← Phase 3 (real LAN) — pending run
│   ├── bench_docker*.sh              ← Docker bench drivers / resume helpers
│   ├── bench_phase{1,2}_stress_locked.sh ← extended stress campaigns
│   ├── stress_curve.sh, plot_stress_curve.py, requirements-stress-curve.txt
│   ├── horner_demo.sh, horner_distributed_demo.sh, horner_live_demo.sh
│   └── cv_triage.py
└── results/
    ├── locked/               ← FROZEN, checksummed evidence (do not edit)
    │   ├── v1_local_baseline/                  ← v1 Phase 1+2 (4600 reps, 0 failures)
    │   ├── v2_post_d012_baseline_2026-05-05/   ← CANONICAL v2 baseline (cited by the paper)
    │   ├── v2_d011_final_baseline_2026-05-04/  ← intermediate v2 baseline
    │   ├── v2_pre_fix_baseline_2026-05-04/     ← pre-D-011-fix baseline (for contrast)
    │   └── v2_stress_curve_2026-05-15_*/       ← stress-curve campaign outputs
    ├── extended/             ← extended v1 stress data
    └── post_fix/             ← post-fix re-run comparison data
```

Each `locked/<campaign>/` directory carries a `MANIFEST.md` with SHA-256 checksums. Those
checksums are the source of truth; `.gitattributes` pins these files to LF so Windows checkouts
don't silently invalidate them.

## Prerequisites

| Requirement | Version / note |
|---|---|
| Rust toolchain | stable, edition 2021 (`cargo build --release`) |
| OS | Linux/macOS for the bash scripts; Windows via Git Bash works (paths handled) |
| Docker | Docker Desktop / engine + compose — required for Phase 2 (TCP localhost) |
| Python | 3.x + `pip install -r scripts/requirements-stress-curve.txt` (only for stress-curve plots) |
| Git tag | check out `v0.10.0-bench` for exact v1 reproduction |

## Quick reproduction (headline)

From the **repository root** (scripts resolve the repo root themselves, so the working directory
only needs to be inside the repo):

```bash
# 1. Build the release binary
cargo build --release

# 2. Dry-run to see exactly what would execute
bash reproduce_article/scripts/reproduce_local_baseline.sh --dry-run

# 3. Reproduce Phase 1 (in-process) and diff against the frozen baseline
bash reproduce_article/scripts/reproduce_local_baseline.sh --phase 1

# 4. Reproduce Phase 2 (Docker/TCP) — requires Docker running
bash reproduce_article/scripts/reproduce_local_baseline.sh --phase 2
```

Fresh outputs land in `reproduce_article/results/reproduction/<DATE>/` (gitignored) alongside a
`comparison.md` that checks **row counts** and **`correct=true` ratios** against the frozen
reference. Those two MUST match exactly; wall-clock columns are expected to differ by hardware.

## What maps to what (article claim → evidence)

| Article claim | Where it comes from |
|---|---|
| Distributed reduction = sequential reduction (G1, 0 correctness failures across 4490+ executions) | `results/locked/v1_local_baseline/` + `results/locked/v2_post_d012_baseline_2026-05-05/` (`all_correct=true` on every slot) |
| Strict-BSP round predictions (`cascade_cross(N)=N`, `dual_tree(d)=d` rounds, w≥2) | `bench_phase1_locked.sh` rounds CSVs |
| Break-even / overhead model `c_o/c_r ≈ 2.2` (needs < 0.5 for speedup at w=2) | Phase 1 vs Phase 2 paired measurements; analysis in [`../docs/ROADMAP.md`](../docs/ROADMAP.md) §2.40 |
| v2 sends ~30% fewer bytes/round (delta protocol) yet is wall-slower on localhost | `results/locked/v2_post_d012_baseline_2026-05-05/` (D-012 per-component instrumentation) |
| Per-component decomposition (wall ≈ network + compute + merge + framing) | `v2_post_d012_baseline_2026-05-05/` `rounds.csv` + `per_rep_metrics/` |
| Church-arithmetic / Horner end-to-end encode→reduce→decode demo | `horner_*` scripts + `results/horner_demo_*.csv` |

## Reproducibility table

| Axis | Value |
|---|---|
| Reference tag | `v0.10.0-bench` (v1); post-D-012 commit for the v2 canonical baseline |
| Canonical v2 baseline | `results/locked/v2_post_d012_baseline_2026-05-05/` (32 distributed slots × 10 reps, all `all_correct=true`) |
| Determinism guarantee | structural columns (`rounds`, `bytes_sent`, `bytes_received`, interaction counts, `correct`) reproduce exactly |
| Non-deterministic by design | wall-clock (`wall_clock_secs`, `mean`, `median`, `cv`) — hardware-dependent, not compared |
| Integrity | SHA-256 per file in each `locked/<campaign>/MANIFEST.md`; LF pinned via `.gitattributes` |
| Verification policy | `reproduce_local_baseline.sh` fails (blocker) on any row-count or `correct=true` mismatch |

## Notes on the larger picture

- **Phase 3 LAN is pending.** `bench_phase3_lan.sh` is the harness; the real cross-machine run is
  the next empirical milestone (it is where v2's delta protocol is expected to cross break-even —
  see [`../docs/reference/next-steps.md`](../docs/reference/next-steps.md)).
- The headline scientific result is a **clean negative result on local/localhost hardware**
  (distribution overhead structurally exceeds the parallel gain), which is *why* the break-even
  analysis and Phase 3 LAN matter. The article is honest about this; so is this package.
