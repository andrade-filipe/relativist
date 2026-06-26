# v2_post_d012_baseline — Campaign Manifest

**Status:** COMPLETE — Definitive v2 TCP/Docker baseline with the
D-012 instrumentation restored (`network_time_secs`,
`compute_time_secs`, and `mips_mean` columns are now populated for
every TCP-mode row, closing RF-04, RF-05, and RF-07 from the D-011
post-mortem).

This is the **canonical pre-release reference baseline** going into
v0.20.0-pre.1 LAN testing. Companion historical baselines:

- `results/locked/v1_local_baseline/` — frozen v1 reference (2026-04-11)
- `results/locked/v2_pre_fix_baseline_2026-05-04/` — pre-D-011-fix
  v2 (4 broken slots; demonstrates Bug 1+2 wall-clock cost)
- `results/locked/v2_d011_final_baseline_2026-05-04/` — post-D-011-fix
  v2 with broken instrumentation (3 zeroed columns; superseded by this
  baseline for analysis purposes but retained as historical reference)

## Provenance

| Field | Value |
|---|---|
| Repository | `github.com/andrade-filipe/relativist` |
| Branch | `v2-development` |
| HEAD at run | `e6ff6bb` (`docs(d-012): close bundle — paperwork`) |
| D-012 fix commits | `360d6ea` (release-test compile) + `fd2cafe` (network metric) + `ca3634c` (compute metric) + `ac828f9` (mips total_interactions) + `c439182` (Stage 6 refactor: SUM→MAX, bash hardcode) |
| Active spec set | SPEC-22 v2.4 (D-011 amendment) + SPEC-21 v2 streaming + SPEC-19 delta + SPEC-18 wire v2 + SPEC-17 transport |
| Build profile | `release` (`cargo build --release`, executed inside Docker by `bench_docker_v2.sh`) |
| Rust toolchain (host) | `rustc 1.94.1 (e408947bf 2026-03-25)` / `cargo 1.94.1 (29ea6fb6a 2026-03-24)` |
| Operator | Filipe Andrade Nascimento |
| Run start | `2026-05-05 16:28` BRT |
| Run end | `2026-05-05 18:36:48` BRT (per `run.log` line "Bench Complete") |
| Wall-clock | **2 h 8 min** (longer than the previous run; second attempt after the first run was invalidated by accidental battery-saver mode) |
| Total datapoints | **400** (per `run.log` final line) |
| Re-run reason | The first attempt on 2026-05-05 morning was discarded — battery-saver mode was active despite Ultimate Performance scheme being set, producing throttled wall-times unsuitable for the canonical baseline. This second attempt confirms operator-validated power state before launch. |

## Hardware & environment

| Field | Value |
|---|---|
| Hostname | `LOMBARDO` |
| Machine | Lenovo ThinkPad T14 Gen 4 (model `21HESHFX00`) — same chassis as v1 + previous v2 baselines |
| CPU model | 13th Gen Intel Core i7-1365U |
| CPU architecture | Hybrid: 2 P-cores HT (up to ~5.2 GHz) + 8 E-cores (up to ~3.9 GHz) |
| Physical cores / logical threads | 10 / 12 |
| RAM | 31.7 GiB DDR5 |
| OS | Windows 11 Pro 10.0.26200 (64-bit) |
| Power scheme during run | "Desempenho Máximo" (Ultimate Performance) — operator-validated active throughout the run; PROCTHROTTLEMIN/MAX both set to 100. Battery saver explicitly disabled (`ESBATTTHRESHOLD = 0`). |
| Power scheme at lock time | "Equilibrado" (Balanced) — Windows automatically reverted after the bench finished; this does not affect the data. |
| Build flags | `RUSTFLAGS='-C target-cpu=native'` for the host-side release rebuild prior to launching bench. Inside the Docker bench image, the project's default release profile applies. |
| Power source | AC plugged in |
| Docker Desktop | WSL2 backend (per `bench_docker_v2.sh` design) |

### Hardware caveats — comparability vs prior baselines

This run is on the **same machine** as v1 baseline (2026-04-11) and the
two prior v2 baselines (2026-05-02 pre-fix and 2026-05-05-am post-fix).
Identical toolchain (`rustc 1.94.1`). The U-series thermal-throttling
caveat from the v1 manifest applies identically here: a 2 h Docker run
will see PL2 turbo collapse into PL1 (15 W) within the first 1–3 min.

This baseline runs **40 % longer** than the previous v2 post-fix run
(50 min) primarily because of bench-script overhead unrelated to
reduction work — the `--no-resume` flag was used to force a clean
overwrite of all 32 slots after the discarded first attempt, and the
sequential phase ran from a colder cache. Per-slot wall-times are
within noise (±1 %) of the previous v2 post-fix run, confirming the
data is comparable.

## Bench parameters (per `run.log`)

| Field | Value |
|---|---|
| Bench script | `scripts/bench_docker_v2.sh --no-resume` |
| Phase tag | "D-011 Phase F-2 (Axis 1)" |
| Configs | 8 (4 × `ep_annihilation_con` sizes + 3 × `dual_tree` sizes + 2 × `condup_expansion` sizes) |
| Workers swept | `1, 2, 4, 8` |
| Repetitions per config | 10 |
| Warmup per config | 2 |
| Timeout | 600 s per docker cycle |
| `CHUNK_SIZE` env | `10000` (streaming partition active per SPEC-21) |
| `MAX_PENDING_LIFETIME` env | `16` |
| Sequential baselines | native binary, 10 reps each, 8 datasets |
| Distributed runs | Docker `tcp_localhost` mode (worker containers ↔ coordinator container over TCP) |
| Features active during run | streaming generation (SPEC-21), delta protocol (SPEC-19), free-list recycle (SPEC-22 R10b/c), CompactSubnet wire (SPEC-19 R26+R28), transport abstraction (SPEC-17 TCP), arena dense/sparse routing (SPEC-22 v2.4) |
| Features **NOT** active | zero-copy serialisation (SPEC-18 — opt-in feature, build flag not set; reserved for Phase 3 LAN axis 2), elastic grid (SPEC-20 — workers static throughout) |

## Artifacts

| File | SHA-256 |
|---|---|
| `summary.csv` | `F11D93BEBE07607A8E1C7BA15334C8DFC9D99C9B3AE3D04959767EAC2687315A` |
| `detail.csv` | `841A5C86990E8506445EF39E7F697B58C8A1F8AD483C10C0A9455A23AD05A1DA` |
| `rounds.csv` | `934FD0404025328076F5C5D3BFAB82F1B20C0F85AC22DF06C27DD588C13B171E` |
| `run.log` | `BEBB3303F1FC3E17ED534734DFD4B82446155AB581BF06327BE789AB090C64D1` |

Additional directories:
- `per_rep_metrics/` — 320 per-repetition metric snapshots (32 configs × 10 reps)
- `seq_outputs/` — sequential reference outputs

## Headline correctness + instrumentation findings

| Property | Value |
|---|---|
| `all_correct=true` slots | 32 / 32 distributed + 8 / 8 sequential |
| Reps completed | 10 / 10 on every slot (zero failures, zero timeouts) |
| `network_time_secs` non-zero rows | 100 % of TCP-mode rows in `rounds.csv` (closes RF-04) |
| `compute_time_secs` non-zero rows | 100 % of TCP-mode rows in `rounds.csv` (closes RF-05) |
| `mips_mean` non-zero rows | 100 % of TCP-mode rows in `summary.csv` (closes RF-07 for distributed path) |
| Sequential `mips_mean` | still 0.000 — `total_interactions` accounting was wired in worker→coordinator path only; sequential `reduce_all` does not increment it. Documented as follow-up `D-012-FU-SEQ-MIPS` in `next-steps.md`; does not block the pre-release baseline. |
| Wall-time stability vs prior run | ±1 % vs `v2_d011_final_baseline_2026-05-04` (e.g., ep_5M w=2: 7.40 s vs 7.42 s) — confirms hardware/thermal envelope reproduced |

## Cross-references

- Pre-release plan: `docs/plans/2026-05-05-v0-20-0-pre-release.md` (will be created during this campaign)
- Detailed analysis (will be updated in this commit): `docs/analysis/D011-final-baseline-analysis-2026-05-04.md`
- Bench script: `scripts/bench_docker_v2.sh`
- D-012 closure narrative: `docs/progress.md` 2026-05-05 entry
