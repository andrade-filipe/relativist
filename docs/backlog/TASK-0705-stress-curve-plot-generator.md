# TASK-0705 — D-014-PLOT: `scripts/plot_stress_curve.py` (9 PDFs + summary)

**Phase:** D-014 (Stress Curve Campaign) — Stage 3 DEV scope
**Bundle:** D-014 — Stress Curve Campaign
**Status:** TODO
**Priority:** P1 (post-aggregation; not on the critical hot path but blocks campaign artifact closure).
**Spec:** none.
**Depends on:** TASK-0703 (CSV column names — the script reads by column).
**Estimated complexity:** M (~200 LoC python + ~30 LoC integration smoke).

---

## Context

After Phase 3 aggregation produces `aggregated.csv`, the campaign needs IEEE-quality figures for the TCC `artigo/tcc_pt_br.tex` Section 5. Per design doc §6:

| Figure | Content |
|---|---|
| `ep_annihilation_walltime.pdf` | log-log `wall_time` vs N for 4 worker counts, 2 envs |
| `ep_annihilation_mips.pdf` | log-log MIPS vs N for 4 worker counts, 2 envs |
| `ep_annihilation_vmrss.pdf` | log-log `vmrss_peak_mb` vs N |
| (same 3 for `dual_tree`) | |
| (same 3 for `condup_expansion`) | |
| `summary_walls.pdf` | Bar chart: `N_max` per `(workload, env, W)` with `StopReason` color-coded |

Total: 9 + 1 = 10 PDFs.

The script reads `aggregated.csv`, groups by `(workload, env, workers)`, plots geometric-mean wall/mips/vmrss across reps with error bars (CV), and emits PDFs at IEEE column width (3.5 in) using matplotlib + pandas.

## Files in scope (file:line pointers)

| File | Change |
|------|--------|
| `scripts/plot_stress_curve.py` | **CREATE.** Python 3.10+, matplotlib + pandas + numpy. ~200 LoC. |
| `scripts/requirements-stress-curve.txt` | **CREATE.** Pin `matplotlib==3.9.*`, `pandas==2.2.*`, `numpy==2.0.*` (or whatever the project's existing `cv_triage.py` already pulls — match versions). ~3 LoC. |
| `relativist-core/tests/d014_plot_smoke.rs` | **CREATE.** Rust integration test that synthesizes a 12-row fake `aggregated.csv`, invokes `python3 scripts/plot_stress_curve.py --input <csv> --output-dir <tmp>`, and verifies ≥ 4 PDF files produced (1 of each metric for the synthetic single-workload data). ~30 LoC. Skipped on systems without `python3` in PATH. |

## Files explicitly OUT of scope

- The orchestrator script — TASK-0704 invokes this script as a subprocess.
- TikZ figure generation — the design doc says PDFs go to `figures/`; the TCC's TikZ workflow ingests them as `\includegraphics{...}` not native TikZ. Different agent (REDATOR) handles TikZ.
- Statistical analysis (CV gating logic, slope fits) — the campaign reports raw geometric means; CV gating already happens in the bench harness via `cv_above_gate` column.
- Frozen `results/locked/` directories.

## Required CLI

```text
python3 scripts/plot_stress_curve.py [OPTIONS]

OPTIONS:
  --input PATH          Path to aggregated.csv. Required.
  --output-dir DIR      Output directory for PDFs. Required.
  --workloads LIST      Comma-separated subset; default = auto-detect from CSV.
  --dpi INT             Plot DPI; default 300.
  --column-width-in F   IEEE column width in inches; default 3.5.
  --no-summary          Skip summary_walls.pdf.

EXIT CODES:
  0   Plots produced successfully
  1   Input CSV missing or malformed
  2   No data after filtering (e.g., empty subset)
```

## Plot conventions (IEEE single-column figures)

1. Figure size: `(3.5, 2.5)` inches.
2. DPI: 300.
3. Font: Computer Modern (matplotlib default `serif`); size 9 pt for labels, 8 pt for tick labels.
4. Colors: 4 worker counts use 4 distinct colors from a colorblind-safe palette (e.g., matplotlib `tab10`'s 0,2,4,6 indices; document the choice in a comment).
5. Markers: distinct per worker count (`o`, `s`, `^`, `D`).
6. Linestyles: solid for `in_process`, dashed for `docker_tcp`.
7. Axes: log-log for wall/mips/vmrss; linear y / categorical x for `summary_walls.pdf`.
8. Error bars: ± 1 stddev (computed from the 5 reps; if `cv_above_gate=true` paint the bar a brighter shade and add an asterisk to the legend).
9. Title: omitted (TCC adds caption via `\caption{...}` in LaTeX).
10. Save with `bbox_inches='tight'`, `pad_inches=0.02`.

## Acceptance criteria

1. Given an `aggregated.csv` with the post-D-014 schema (TASK-0703 columns) for all 3 workloads × 2 envs × 4 W × 11 N × 5 reps = up to 1320 rows, the script produces exactly 10 PDF files (9 metric × workload + 1 summary).
2. Given a partial CSV (e.g., only `ep_annihilation` rows), the script produces only the 3 PDFs for that workload + the summary; exit 0.
3. Given an empty CSV (header only), the script exits with code 2 and a clear error message.
4. Given a CSV missing a required column (`vmrss_peak_mb`), the script exits with code 1 and a clear error message naming the missing column.
5. Each PDF passes `pdfinfo` with no errors.
6. New integration test `d014_plot_smoke` passes on systems with `python3` (≥ 3.10) in PATH; correctly skipped otherwise.
7. `cargo test` floor: **+1 default = ≥ 1813** (cumulative TASK-0700..0705).
8. `cargo test --features zero-copy` floor: **+1 = ≥ 1857**.
9. `cargo test --features streaming-no-recycle` floor: **+1 = ≥ 1804**.
10. `cargo test --release` floor: **+1 = ≥ 1755**.
11. v1 floor (690) inviolable.
12. `cargo clippy --all-features -- -D warnings` clean.
13. `cargo fmt --check` clean.

## Test floor delta

**+1 default** (one integration test). Cumulative after TASK-0700..0705:
- default ≥ 1813
- zero-copy ≥ 1857
- streaming-no-recycle ≥ 1804
- release ≥ 1755

## Implementation hints

1. Re-use `scripts/cv_triage.py`'s pandas + matplotlib idioms — same project, same dependency versions, same style conventions. Read `cv_triage.py` before authoring; inherit its argparse + main() skeleton.
2. The IEEE column-width 3.5 in is from the IEEE conf template; confirm by checking `Modelo TCC/article.tex` for `\columnwidth` references — if the article ends up double-column, the figures should already fit.
3. Use `numpy.geomean` for the average across reps (geometric mean is appropriate for log-axis data); document in a comment why arithmetic mean is wrong here.
4. For the summary plot: x-axis = `(workload, env, W)` triple as a categorical string label, y-axis = `N_max` (the largest N successfully completed); bar color encodes StopReason (`WallTimeExceeded` blue, `MemoryExceeded` red, `Oom` purple, "no stop, sequence completed" green).
5. Save PDFs with `matplotlib.backends.backend_pdf.PdfPages` if multi-page; here all PDFs are single-page so just `fig.savefig(path)`.
6. Avoid `plt.show()` — script is non-interactive.
7. Reset matplotlib state between figures (`plt.close('all')` after each save) to prevent memory growth on the 10-figure run.

## Estimated LoC

- Python: ~200 LoC.
- Tests: ~30 LoC Rust.
- Total: ~230 LoC. Same caveat as TASK-0704 — Python LoC is low-density (long imports, doc-strings, kwargs); acceptable.

## Cross-references

- Design doc: `docs/superpowers/specs/2026-05-05-stress-test-large-nets-design.md` §4.2 row 6, §6 (figure list), §8 sanity checks (slope expectations).
- Reference style: `scripts/cv_triage.py` (same matplotlib/pandas idioms).
- Consumes: TASK-0703 column names.
- Consumed by: TASK-0704 (script invokes it), TASK-0708 (campaign run produces final figures).
