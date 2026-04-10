# v1_local_baseline — Campaign Manifest

**Status:** TEMPLATE — fields marked `<FILL>` must be populated by the
operator running the campaign. Once the campaign completes, commit this
file together with the frozen CSVs in a single atomic commit.

## Provenance

| Field | Value |
|---|---|
| Repository | `github.com/andrade-filipe/relativist` |
| Git tag | `v0.10.0-bench` |
| Commit SHA | `<FILL: git rev-parse v0.10.0-bench>` |
| Build profile | `release` (`cargo build --release`) |
| Rust toolchain | `<FILL: rustc --version>` |
| Operator | Filipe Andrade Nascimento |
| Start date | `<FILL: YYYY-MM-DD HH:MM (local)>` |
| End date | `<FILL: YYYY-MM-DD HH:MM (local)>` |

## Hardware & environment

| Field | Value |
|---|---|
| Machine | `<FILL: hostname or label>` |
| CPU model | `<FILL>` |
| Physical cores / logical threads | `<FILL>` |
| RAM | `<FILL: GB>` |
| Storage backing `data/` | `<FILL: SSD/NVMe/HDD>` |
| OS | `<FILL: Windows 11 Pro 10.0.XXXXX>` |
| Docker Desktop version | `<FILL: docker --version>` |
| Docker engine memory limit | `<FILL: GB>` |
| Power plan | `High performance` |
| Background load posture | Browsers closed, Windows Update paused, no IDE running |

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
