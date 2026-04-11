# v1_local_baseline — Campaign Manifest

**Status:** PARTIAL — hardware, toolchain and provenance fields are
populated from the operator's machine inventory. Timestamps, checksums
and row counts remain `<FILL>` until the campaign finishes; commit this
file together with the frozen CSVs in a single atomic commit after that.

## Provenance

| Field | Value |
|---|---|
| Repository | `github.com/andrade-filipe/relativist` |
| Git tag | `v0.10.0-bench` |
| Commit SHA | `8529dd572a06bb32de80b8ce8b9eb2c67a63f20f` |
| Build profile | `release` (`cargo build --release`) |
| Rust toolchain | `rustc 1.94.1 (e408947bf 2026-03-25)` / `cargo 1.94.1 (29ea6fb6a 2026-03-24)` |
| Operator | Filipe Andrade Nascimento |
| Start date | `<FILL: YYYY-MM-DD HH:MM -03:00>` |
| End date | `<FILL: YYYY-MM-DD HH:MM -03:00>` |

## Hardware & environment

| Field | Value |
|---|---|
| Machine | Lenovo ThinkPad T14 Gen 4 (model `21HESHFX00`) |
| CPU model | Intel Core i7-1365U (13th gen, Raptor Lake-U, 15 W base TDP / 55 W turbo) |
| CPU architecture | Hybrid: 2 P-cores (hyperthreaded, up to ~5.2 GHz) + 8 E-cores (up to ~3.9 GHz) |
| Physical cores / logical threads | 10 / 12 |
| Cache | L2 = 6.5 MB (aggregated), L3 = 12 MB |
| RAM | 32 GB DDR5 @ 5200 MT/s (2 × 16 GB, Samsung `M425R1GB4DB0-CWMOL` + A-Data `AI1V56WCSV1-B1ES`) |
| Storage backing `data/` | Kingston `SNV3S1000G` 1 TB NVMe SSD |
| OS | Windows 11 Pro 10.0.26200 (64-bit) |
| Docker Desktop version | 29.3.1 (build c2be9cc), engine kernel `6.6.87.2-microsoft-standard-WSL2` |
| Docker engine resources | 12 CPUs, 15.45 GiB RAM allocated to the WSL2 VM |
| Power source | AC (plugged in, battery at 98 %) |
| Power plan / mode | Windows **"Desempenho Máximo"** (Ultimate Performance) scheme, activated via `powercfg -duplicatescheme e9a42b02-d5df-448d-aa00-03f14749eb61 && powercfg /setactive <new GUID>`. The Settings → Power & battery UI was non-functional (error `0x80040905` from `ms-settings:powersleep`, a known Windows 11 preview-build Settings app bug), so the scheme was unhidden and activated via CLI instead. Hardware PL1 of 15 W is unchanged (firmware limit, not software). |
| Background load posture | Browsers closed, Windows Update paused, no IDE running; only a bash terminal foreground. Docker Desktop closed during Phase 1 and restarted immediately before Phase 2. |

### Hardware constraints (disclosure)

This campaign was run on a 14-inch business ultrabook, not a dedicated
benchmark machine. Three hardware-specific caveats shape how the
wall-clock numbers in the frozen CSVs should be interpreted. They are
documented here rather than papered over, because transparency about
hardware context is the price of reproducibility on consumer silicon.

1. **U-series thermal throttling is expected over a 4–6 h run.** The
   i7-1365U sustains its 15 W PL1 indefinitely, but the 55 W PL2 turbo
   budget is exhausted within seconds. Longer benchmarks (Phase 2
   `dual_tree=22`, `ep_annihilation_con=5M`) run largely in the
   throttled regime, whereas short benchmarks (`ep_annihilation` small
   sizes) may briefly hit turbo. As a consequence, wall-clock scaling
   across sizes is not linear in the way a desktop workstation would
   produce — the ratio of "long benchmark / short benchmark" is biased
   toward the long benchmark taking longer than a non-throttled
   machine would predict. This is acceptable for the baseline because
   Phase 3 LAN comparisons subtract the *same-hardware* local
   reference, not an idealized one.

2. **Hybrid P/E core scheduling adds per-repetition variance.** With
   `--workers 8`, the 8 reduction threads are distributed by the
   Windows scheduler across 2 P-cores (~5.2 GHz max) and 8 E-cores
   (~3.9 GHz max). Which threads land on P vs. E cores changes
   between repetitions, injecting variance that cannot be eliminated
   without core pinning (which would itself compromise the
   representativeness of the baseline). High-CV datapoints flagged
   by `scripts/cv_triage.py` are reviewed manually and either kept
   with a footnote in the article or excluded; see `cv_triage.md`.

3. **Docker Desktop oversubscribes the physical CPU.** The WSL2 VM
   is configured with 12 vCPUs while the host has only 10 physical
   cores (12 logical threads total). During Phase 2 this is the
   intended operating point — the Docker containers *are* the
   workload. Docker Desktop was closed during Phase 1 to avoid the
   ~2 – 4 GB background WSL2 footprint contaminating the in-process
   measurements, and reopened immediately before Phase 2 launched.

These constraints are why `scripts/cv_triage.py` exists and why the
article reports CV alongside mean wall-clock: they turn a limitation
into disclosable methodology rather than a hidden confounder.

## Campaign knobs

### Phase 1 — in-process (`scripts/bench_phase1_locked.sh`)

| Knob | Value |
|---|---|
| Repetitions | 10 |
| Warmup runs | 2 |
| Worker counts | `1,2,4,8` (plus sequential baseline auto-added by the suite) |
| Lenient benchmarks | `ep_annihilation`, `ep_annihilation_con`, `ep_annihilation_dup`, `dual_tree`, `tree_sum`, `tree_sum_balanced`, `mixed_net`, `erasure_propagation`, `church_add`, `church_mul`, `cascade_cross` |
| `condup_expansion` sizes with full G1 | `100, 500, 1000, 5000` |
| `condup_expansion` sizes with weak check (`--skip-g1`) | `10000, 50000` (abordagem A default) |
| Strict BSP benchmarks | `cascade_cross` (default sizes), `dual_tree` (sizes `6, 10, 14`) |

### Phase 2 — Docker / TcpLocalhost (`scripts/bench_phase2_locked.sh`)

| Knob | Value |
|---|---|
| Repetitions | 10 |
| Warmup runs | 2 |
| Worker counts | `1, 2, 4, 8` |
| Per-run timeout | 1800s (30 minutes) |
| Benchmarks × sizes | `ep_annihilation_con × {500k, 1M, 5M}`, `dual_tree × {18, 20, 22}`, `condup_expansion × {1000, 5000}` |
| Total Docker configs | 32 (8 × 4 workers) |
| Plus native sequential baselines | 8 (one per bench × size) |
| **Total Phase 2 datapoints** | **40** |

## Correctness methodology

| Benchmark(s) | Sizes | Method | Notes |
|---|---|---|---|
| `ep_annihilation*`, `dual_tree`, `tree_sum*`, `mixed_net`, `erasure_propagation`, `church_*`, `cascade_cross` | all default sizes | Full G1 (`nets_isomorphic`) | Deterministic structural equivalence |
| `condup_expansion` | `100, 500, 1000, 5000` | Full G1 | Completes within minutes |
| `condup_expansion` | `10000, 50000` | Weak check (`--skip-g1`) | Abordagem A — agent count + rule tally equality. Abordagem B overnight full-G1 is optional, see `USAGE_GUIDE.md` §11.5 |
| All Phase 2 Docker runs | — | Structural check via `relativist inspect` (agent count + redex count equality to sequential reference output) | Identical to `bench_docker.sh` canonical method |

## Strict BSP campaign

| Benchmark | Sizes | Workers | Repetitions | Purpose |
|---|---|---|---|---|
| `cascade_cross` | `10, 50, 100, 500, 1000` | `1, 2, 4, 8` | 10 | Per-round data for cross-partition cascades (Phase 3 RTT cost isolation) |
| `dual_tree` | `6, 10, 14` | `1, 2, 4, 8` | 10 | Expected ~d rounds under strict mode (theoretical validation) |

All other Phase 1 benchmarks run with `strict_bsp=false` (lenient default).
See `specs/SPEC-05-*.md` §Lenient-vs-Strict for the formal definition and
`specs/SPEC-09-benchmarks.md` for the updated property tables.

## Checksums (sha256)

Computed over the final concatenated CSVs. Fill after the campaign ends:

```
<FILL: sha256sum phase1_lenient_detail.csv>
<FILL: sha256sum phase1_lenient_rounds.csv>
<FILL: sha256sum phase1_lenient_summary.csv>
<FILL: sha256sum phase1_strict_detail.csv>
<FILL: sha256sum phase1_strict_rounds.csv>
<FILL: sha256sum phase1_strict_summary.csv>
<FILL: sha256sum phase2_detail.csv>
<FILL: sha256sum phase2_rounds.csv>
<FILL: sha256sum phase2_summary.csv>
```

Regenerate on a reproduction machine and compare row counts and
correctness columns — wall-clock columns will differ by hardware.

## Row counts (sanity check)

Fill after the campaign ends. Expected (approximate):

| File | Expected rows | Actual |
|---|---|---|
| `phase1_lenient_detail.csv` | ~12 benchmarks × default_sizes × {seq+1+2+4+8} × 10 reps + header | `<FILL: wc -l>` |
| `phase1_strict_detail.csv` | (cascade_cross 5 sizes + dual_tree 3 sizes) × 5 workers × 10 reps + header | `<FILL: wc -l>` |
| `phase2_detail.csv` | 8 sequential + (8 × 4 workers × 10 reps) = 328 + header = 329 | `<FILL: wc -l>` |
| `phase2_summary.csv` | 8 sequential + 32 Docker + header = 41 | `<FILL: wc -l>` |

`awk -F, 'NR>1 && $6=="false"'` on any `*_detail.csv` must return zero
lines: every repetition must pass the configured correctness check.
Any `correct=false` row invalidates the snapshot and must be
investigated before the manifest is signed off.
