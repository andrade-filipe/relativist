# v2_d011_final_baseline — Campaign Manifest

**Status:** COMPLETE — D-011 perf-fix verification campaign over the
TcpLocalhost transport, 32 configurations × 10 repetitions × 2 warmups
+ 8 sequential native baselines, finished 2026-05-05 11:36 BRT on the
hardware described below. All artifacts are committed together as a
single atomic snapshot.

This baseline is **the post-fix companion** to
`results/locked/v2_pre_fix_baseline_2026-05-04/` (run on the same
machine, same scheme, before the BLOCKER 2026-05-04 fix bundle landed).
Together the two snapshots quantify the impact of the dense
`build_subnet` correctness fixes (TASK-0612 + Bug 1 + Bug 2) on the
public TCP-distributed benchmark.

## Provenance

| Field | Value |
|---|---|
| Repository | `github.com/andrade-filipe/relativist` |
| Branch | `v2-development` |
| HEAD at run | `b079cdc` (`docs(d-011): close BLOCKER 2026-05-04 — paperwork bundle`) |
| Bench-relevant fix commit | `62de30f` (TASK-0612 + Bug 1 + Bug 2 — SPEC-22 v2.4 metric + dense `build_subnet` correctness) |
| Active spec | SPEC-22 v2.4 (R22+R30 effective_arena_size threshold metric) |
| Build profile | `release` (`cargo build --release`, executed inside Docker by `bench_docker_v2.sh`) |
| Rust toolchain (host) | `rustc 1.94.1 (e408947bf 2026-03-25)` / `cargo 1.94.1 (29ea6fb6a 2026-03-24)` |
| Rust toolchain (Docker) | as pinned by `Dockerfile` (image rebuilt fresh from HEAD at start of run; see `run.log` line 13: "Building Docker image…") |
| Operator | Filipe Andrade Nascimento |
| Run start | `2026-05-05 10:46:39 -0300` (per `run.log`) |
| Run end | `2026-05-05 11:36:48 -0300` (per `run.log`, line "Bench Complete") |
| Wall-clock | **50 min 9 s** (8 sequential baselines + 32 distributed configs × 12 binary executions = 392 binaries) |
| Total datapoints | **400** (per `run.log` final line) |

## Hardware & environment

| Field | Value |
|---|---|
| Hostname | `LOMBARDO` |
| Machine | Lenovo ThinkPad T14 Gen 4 (model `21HESHFX00`) — same chassis as v1 baseline |
| CPU model | 13th Gen Intel Core i7-1365U |
| CPU architecture | Hybrid: 2 P-cores HT (up to ~5.2 GHz) + 8 E-cores (up to ~3.9 GHz) |
| Physical cores / logical threads | 10 / 12 |
| RAM | 31.7 GiB DDR5 |
| OS | Windows 11 Pro 10.0.26200 (64-bit) |
| Power scheme | **"Desempenho Máximo"** (Ultimate Performance), GUID `58c19d96-7bb5-497f-9d49-66d9a1860c31`. Activated via `powercfg /setactive` before bench start (per user procedure 2026-05-05). |
| Build flags | `RUSTFLAGS='-C target-cpu=native'` for the host-side release rebuild prior to launching bench. Inside the Docker bench image, the project's default release profile applies (target-cpu not propagated; this is a deliberate trade-off — Docker image is portable, host binary is native). |
| Power source | AC plugged in |
| Docker Desktop | WSL2 backend (per `bench_docker_v2.sh` design) |

### Hardware caveats — comparability vs v1 baseline

This run was executed on the **same machine** as the v1 baseline
(`results/locked/v1_local_baseline/`, 2026-04-11), with identical
toolchain (`rustc 1.94.1`) and identical power scheme (Ultimate
Performance). The only environmental delta from v1 is the elapsed time
(thermal/firmware state), background load, and an extra
`RUSTFLAGS='-C target-cpu=native'` set on the **host-side** release
rebuild — which has **no effect** on the Docker container build.
Therefore, the wall-time deltas observed between v1 and v2-post-fix in
TcpLocalhost mode are attributable to **code-path differences, not
hardware/environment differences**.

The U-series thermal-throttling caveat from the v1 manifest applies
identically here: a 50-minute Docker run will see PL2 turbo collapse
into PL1 (15 W) within the first 1–3 minutes; per-slot wall-times after
that point reflect sustained, not boosted, frequency. This penalises
larger datasets (`*_5000000`, `dual_tree_22`) more than smaller ones
and is the reason CV widens at large input sizes.

## Bench parameters (per `run.log`)

| Field | Value |
|---|---|
| Bench script | `scripts/bench_docker_v2.sh` |
| Phase tag | "D-011 Phase F-2 (Axis 1)" |
| Configs | 8 (4 × `ep_annihilation_con` sizes + 3 × `dual_tree` sizes + 2 × `condup_expansion` sizes) |
| Workers swept | `1, 2, 4, 8` |
| Repetitions per config | 10 |
| Warmup per config | 2 |
| Timeout | 600 s per docker cycle |
| Resume | enabled (`bench_docker_v2.sh -r`) — no prior progress detected at start |
| Sequential baselines | native binary, 10 reps each, 8 datasets |
| Distributed runs | Docker `tcp_localhost` mode (worker containers ↔ coordinator container over TCP) |

## Artifacts

| File | SHA-256 |
|---|---|
| `summary.csv` | `D4880D30A8D0BCB5378667CBB36CBA23D633CFA508DD037BDF85C10B402EC720` |
| `detail.csv` | `61FD26A568469F2DE421731B29F3585FDE07F792B899FF9629FC6B7E941AA94F` |
| `rounds.csv` | `FFF8449B72199711D4519ED8D6A08B80394C9971C4362626150C8F9C8ADC15E3` |
| `run.log` | `1ABC627576E4DDB07293C1619578BA5596C74B1F8280BACA9B66654100C83E24` |

Additional directories:
- `per_rep_metrics/` — 320 per-repetition metric snapshots (32 configs × 10 reps)
- `seq_outputs/` — sequential reference outputs

## Test-floor status at HEAD (`b079cdc`)

| Profile | Status |
|---|---|
| `cargo build --release` | ✅ PASS (15 s) — production binary compiles cleanly |
| `cargo test` (debug profile) | see `docs/analysis/D011-final-baseline-analysis-2026-05-04.md` (recorded at commit time) |
| `cargo test --release` | ❌ FAIL — **pre-existing, unrelated to D-011**: tests in `net/debug.rs` reference symbols gated by `#[cfg(debug_assertions)]`, and `coordinator.rs::ut_0577_08` (release-only) has a non-exhaustive match for `PullCoordinatorError::WorkerIdMismatch` (added by QA-D010-002 later than the test). Filed as a follow-up; does **not** invalidate this bench, which uses `cargo build --release` (production binary, no test code). |

## Known instrumentation defect (does not invalidate wall-time data)

`rounds.csv::network_time_secs` is **structurally 0.0** for every row
in this v2 dataset. Root cause: the `GridMetrics` fields
`network_send_time_per_round` and `network_recv_time_per_round`
(declared in `relativist-core/src/merge/types.rs:69,72`, read by
`relativist-core/src/bench/suite.rs:621-625` to populate the CSV) are
never written by any production code path — only by test fixtures
(`merge/types.rs:1354,1355`). This is a v2 **instrumentation
regression** vs v1 (v1's `phase2_rounds.csv` recorded ~0.29 s of
network time per TCP-localhost round; see line 2 of that file). The
defect does **not** affect wall-time, throughput, byte-counter, or
correctness columns, all of which are independently measured. Filed
as a follow-up bug; tracked in the analysis doc.

## Cross-references

- Investigation arc: `docs/progress.md` § "BLOCKER 2026-05-04"
- Detailed analysis: `docs/analysis/D011-final-baseline-analysis-2026-05-04.md`
- v1 baseline (companion): `results/locked/v1_local_baseline/manifest.md`
- v2 pre-fix baseline (companion): `results/locked/v2_pre_fix_baseline_2026-05-04/`
- Verification micro-bench: `docs/benchmarks/d011-perf-fix-verification-2026-05-04.md`
- Spec at run-time: `specs/SPEC-22-arena-management.md` (v2.4)
