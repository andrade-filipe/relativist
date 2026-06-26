# v1_local_baseline — Campaign Manifest

**Status:** COMPLETE — Phase 1 and Phase 2 campaigns finished
successfully on 2026-04-11 on the hardware described below. All
timestamps, checksums and row counts are filled. This manifest is
committed together with the frozen CSVs as a single atomic snapshot.

## Provenance

| Field | Value |
|---|---|
| Repository | `github.com/andrade-filipe/relativist` |
| Git tag | `v0.10.0-bench` |
| Commit SHA | `8529dd572a06bb32de80b8ce8b9eb2c67a63f20f` |
| Build profile | `release` (`cargo build --release`) |
| Rust toolchain | `rustc 1.94.1 (e408947bf 2026-03-25)` / `cargo 1.94.1 (29ea6fb6a 2026-03-24)` |
| Operator | Filipe Andrade Nascimento |
| Phase 1 start | `2026-04-11 11:44:55 -0300` |
| Phase 1 end | `2026-04-11 11:56:34 -0300` (wall clock: **11 min 39 s**) |
| Phase 2 start | `2026-04-11 12:22:37 -0300` |
| Phase 2 end | `2026-04-11 13:06:19 -0300` (wall clock: **43 min 42 s**) |

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

Computed over the final concatenated CSVs.

```
26e33178446bd58df8ccc14187c0a8a8f2666e5c844186df4f6d0dcc86646fb5  phase1_lenient_detail.csv
32e6f07fd820171d689c5484f416234593e76b97fc75af65cecce8ad2c356d85  phase1_lenient_rounds.csv
9bd9056c19f9a1c15393a6853fe150b2132c4a31cc1700cf4d34db931ad44bfe  phase1_lenient_summary.csv
749d0c86d51ee69a49a65d722d60296f4369656f1f10df2f5bf018749374444d  phase1_strict_detail.csv
f685c3d22a7e91369a88cb441f7374eb334c9b9604ab0f29a6b528d454ba474c  phase1_strict_rounds.csv
001f5a5cdb4c5a4d5ca6f3c460322092c14c7fd75319fd9052c6788643139265  phase1_strict_summary.csv
174fd2fca5dd1deda420d545ebe43e7d587fa13dac16c60569cf752d6d02acee  phase2_detail.csv
6948cd3f36825cec30fdc13c4a9bf4f61e49fdeab70c78906f9f1ec6179092b8  phase2_rounds.csv
de07fc8c357f39202008fd4d92e0644ec317b15b831fbb5df7f456824b93e218  phase2_summary.csv
```

Regenerate on a reproduction machine and compare row counts and
correctness columns — wall-clock columns will differ by hardware.

## Row counts (sanity check)

| File | Expected | Actual |
|---|---|---|
| `phase1_lenient_detail.csv`  | 12 benchmarks × default_sizes × {seq+1+2+4+8} × 10 reps + header | **3401** |
| `phase1_lenient_rounds.csv`  | one per grid round (lenient = 1 round per run) + header | **2721** |
| `phase1_lenient_summary.csv` | one per (bench, size, mode, workers) + header | **341** |
| `phase1_strict_detail.csv`   | (5 cascade_cross + 3 dual_tree) sizes × 5 workers × 10 reps + header | **401** |
| `phase1_strict_rounds.csv`   | one per strict round (empirically `rounds = N` for cascade_cross, `rounds = d` for dual_tree) + header | **50781** |
| `phase1_strict_summary.csv`  | one per (bench, size, workers) + header | **41** |
| `phase2_detail.csv`          | 8 bench×size × 5 worker configs × 10 reps = 400 + header | **401** |
| `phase2_rounds.csv`          | 32 Docker configs × 10 reps = 320 + header (sequential emits no grid round) | **321** |
| `phase2_summary.csv`         | 8 sequential + 32 Docker + header = 41 | **41** |

`awk -F, 'NR>1 && $6=="false"'` on any `*_detail.csv` returns zero lines
for both phases (0 of 3800 Phase 1 measurements and 0 of 400 Phase 2
measurements failed G1 / weak-check / structural equality). Any
`correct=false` row would invalidate the snapshot.

## Phase 1 results summary

- **Wall clock:** 11 min 39 s total (much faster than the 4-6 h
  planning estimate, because the U-series sustained-load concern was
  mostly offset by DDR5-5200 memory bandwidth and because the
  `--skip-g1` weak check on `condup_expansion` 10k/50k eliminates the
  O(N!) verification cost that dominates worst-case runs).
- **Correctness:** 0 of 3800 measurement repetitions failed G1 /
  weak-check. All 12 Phase 1 benchmarks (lenient) and both strict
  benchmarks (cascade_cross, dual_tree) produced stable, reduced nets.
- **Strict BSP validation (SPEC-09 theoretical predictions confirmed):**
  empirical `rounds` exactly matches the theoretical expectation in
  every config — `cascade_cross(N)` terminates in `N` rounds with
  `workers ≥ 2`, and `dual_tree(d)` terminates in `d` rounds with
  `workers ≥ 2`. L2 (BSP loop collapse) is resolved in data, not just
  in code.
- **CV triage:** 62 of 340 lenient summary rows flagged with
  `cv > 0.15`. `scripts/cv_triage.py` classified **all 62 as `keep`
  (0 rerun, 0 exclude)**: 58 are sub-millisecond timer noise, and 4
  are genuine P/E core scheduling variance in the 0.155 – 0.193 range
  (well below the 0.30 rerun threshold). These 4 will be footnoted in
  the article — see `cv_triage.md` for the list.
- **`raw/phase1/`:** per-benchmark stdout + raw detail/rounds/summary
  CSVs preserved for forensic review.

## Phase 2 results summary

- **Wall clock:** 43 min 42 s total (`12:22:37` → `13:06:19`). Much
  faster than the 1.5 – 3 h planning estimate because the Docker
  per-run overhead (compose up / coordinator hand-off / compose down)
  is roughly constant ~3 – 5 s regardless of benchmark size, and the
  ThinkPad's NVMe SSD kept container image start-up cheap. The
  `dual_tree=22` and `ep_annihilation_con=5M` configs accounted for
  most of the wall-clock weight (8 – 10 s per rep at `workers=8`).
- **Correctness:** 0 of 400 measurement repetitions failed the
  structural check (`relativist inspect`-based agent + redex count
  equality to sequential reference output). All 8 benchmark × size
  combos and all 4 Docker worker counts (1, 2, 4, 8) produced nets
  indistinguishable from the sequential baseline.
- **Rounds:** every Docker run terminated in exactly 1 round, as
  expected under the lenient BSP mode that Phase 2 uses by design.
  Phase 2 is deliberately *not* a strict-BSP campaign — its purpose
  is to characterize the distributed-local baseline on the same
  hardware that Phase 3 LAN will subtract, not to count rounds.
  Strict-BSP data lives in the Phase 1 `phase1_strict_*.csv` files
  instead.
- **CV triage (Phase 2):** 1 of 40 Docker summary rows flagged with
  `cv > 0.15` — `condup_expansion, size=1000, workers=1` with
  CV = 0.172 at a 1.99 ms mean wall-clock. Automatic disposition:
  `keep` (timer noise at sub-5 ms scale, not genuine variance).
  Combined Phase 1 + Phase 2 CV triage now reports **63 flagged /
  63 keep / 0 rerun / 0 exclude** — see `cv_triage.md`.
- **`raw/phase2/`:** 320 per-repetition `metrics_*.json` files (one
  per Docker run, not produced by the sequential baselines) preserved
  for forensic review.
