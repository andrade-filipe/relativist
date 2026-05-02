# Design — `bench_docker_v2.sh` (D-011 Phase F-2 docker TCP rodada)

**Date:** 2026-05-02
**Author:** Filipe Andrade Nascimento (with Claude)
**Branch:** `v2-development`
**Bundle:** D-011 Phase F-2 (extended)
**Status:** Approved — ready for implementation

## 1. Context

D-011 Phase F-2 originally planned a "docker TCP rodada" via the new `bench-tcp` profile in `docker-compose.yml`. End-to-end smoke (after fixing 3 latent docker-path bugs: stub binary cache, socket2 `with_retries` feature, GLIBC mismatch) revealed that `relativist bench --mode tcp_localhost` does **not** dispatch over TCP — it parses the flag but always runs the harness in-process. Confirmed in `docs/benchmarks/DATA-COLLECTION-PLAN.md:152` and verified by code search (no `Mode::TcpLocalhost` dispatch in any binary path).

The v1 had a working TCP rodada produced by `scripts/bench_docker.sh` orchestrating `docker compose up coordinator worker --scale worker=N`. Frozen baseline at `results/locked/v1_local_baseline/phase2_*.csv` (400 datapoints, 8 configs, 22-column schema).

This design adapts the v1 script for v2 with Tier 3 active in the coordinator, extended scope to also bridge the v2-local rodada.

## 2. Goals

- Produce TCP rodada CSVs comparable directly to `phase2_*` (Axis 1: v1-TCP vs v2-TCP).
- Produce TCP rodada CSVs covering the workload of `v2_local_full_summary.csv` (Axis 2: v2-local vs v2-TCP), filtered to viable configs.
- Tier 3 optimizations active in coordinator path (free-list recycling, CompactSubnet wire fix, `--chunk-size`, `--max-pending-lifetime`).
- Resume capability mandatory — partial CSVs survive interruption.
- All in a single user-runnable script, no code changes to `relativist` binary required.

## 3. Non-goals

- No TCP dispatch implementation inside the `bench` subcommand (deferred to D-012 candidate).
- No 7 extra v2 schema columns (`peak_memory_during_construction`, etc.) — N/A in coord+worker path. Schema stays at v1 22 columns.
- No `--recycle-policy` exposure on coordinator (hardcoded `DisableUnderDelta`, matches local rodada default).
- No TLS/auth on the rodada — TCP loopback is trusted-network by construction (matches v1 posture).
- No fault injection, chaos benchmarks, or Phase 3 LAN coverage.
- No refactor of v1 `bench_docker.sh` (kept as historical reference).

## 4. Design

### 4.1 Language & environment

Bash + Python 3, matching v1 `bench_docker.sh` (520 lines). Runs under Git Bash on Windows (uses `cygpath` for path conversion). Avoids re-implementing in PowerShell.

### 4.2 Output schema

**v1 22-column format** for TCP CSVs. Columns:

```
benchmark, input_size, mode, workers, repetition, correct, wall_clock_secs,
total_interactions, mips, rounds, speedup, efficiency, overhead_ratio,
peak_memory_bytes, bytes_sent, bytes_received,
con_con, dup_dup, era_era, con_dup, con_era, dup_era
```

**Rationale:** the 7 extra columns in v2 29-column schema (`peak_memory_during_construction`, `peak_memory_during_reduction`, `agent_count_at_construction_complete`, `live_agent_count_watermark`, `representation`, `chunk_size`, `recycle_policy`) are partially N/A in coord+worker path because (a) coordinator loads net from disk (no construction phase), (b) `metrics.json` does not expose post-reduction VmHWM or live-agent watermark (verified in `merge/types.rs:18`). Filling with zero placeholders pollutes the CSV. The Axis 2 comparison (v2-local vs v2-TCP) operates on the 22 shared columns.

### 4.3 Tier 3 in coordinator path — what applies

| Optimization | Applied? | Source of truth |
|---|---|---|
| Free-list recycling (D-009 SPEC-22) | Yes — runtime-resident in `Net::create_agent` | n/a |
| CompactSubnet wire fix (D-011 B-1, QA-D009-001) | Yes — protocol-layer fix | `relativist-core/src/net/sparse.rs` |
| `--chunk-size` flag wiring | Yes — `CoordinatorArgs.chunk_size`, default 10000 | `config.rs:293` |
| `--max-pending-lifetime` flag wiring | Yes — `CoordinatorArgs.max_pending_lifetime`, default 16 | `config.rs:311` |
| Streaming chunked partitioning | **No** — `coordinator.rs:842` calls eager `split()`. Streaming benefit only realizes when generating net inline; coord loads from disk. | `coordinator.rs:842` |
| `representation=sparse` | **No** — bench-only flag for construction memory measurement; coord transports dense `Net`. | n/a |
| `recycle_policy` | Hardcoded `DisableUnderDelta` in `commands.rs:785`. Safe default, matches local rodada. | `commands.rs:785` |

### 4.4 Configuration profiles

**Axis 1 — v1 parity (8 configs hardcoded):**

```
ep_annihilation_con × {500_000, 1_000_000, 5_000_000}
dual_tree           × {18, 20, 22}
condup_expansion    × {1_000, 5_000}
```

Same as `bench_docker.sh` v1 line 62-71.

**Axis 2 — v2-local bridge (filtered subset):**

Loaded dynamically from `results/v2_local_full_summary.csv` at script startup. Filter rule:

```
keep (benchmark, size) where mode=sequential AND wall_clock_mean >= 0.050
```

Rationale: Docker compose startup overhead is ~5s/cycle. A 50ms filter ensures the actual reduction time is ≥1% of total wall-clock — the docker overhead is bounded contamination. Configs faster than 50ms produce ratio-of-noise data points.

Expected after filtering: ~30 configs (most cascade_cross, church_*, dual_tree<10, ep_ann<5K drop out; ep_ann_con/dup at ≥10K, dual_tree≥10, condup_expansion≥1K, mixed_net≥5K, erasure_propagation≥5K survive).

**Workers:** `(1, 2, 4, 8)` for both axes.

**Repetitions:** 10 measurements + 2 warmups per (bench, size, workers) tuple.

**Per-cycle timeout:** 600s (10 min).

### 4.5 Output layout

```
results/v2_tcp_baseline/
├── detail.csv                         # 22 cols, all reps (sequential + tcp_localhost)
├── summary.csv                        # 16 cols, mean/std/cv/speedup per (bench, size, mode, workers)
├── rounds.csv                         # 15 cols, per-round breakdown for TCP rows
├── per_rep_metrics/                   # forensic dumps
│   └── ${bench}_${size}_w${w}_r${r}.json
├── seq_outputs/
│   └── ${bench}_${size}.bin           # for G1 cross-check
└── run.log                            # tee of stdout, timestamped
```

`run.log` is `tee`'d during execution; per-rep metrics are copies of `data/metrics.json` after each successful TCP run.

Promotion to `results/locked/v2_tcp_baseline/` happens manually in F-3 close-out.

### 4.6 Script architecture (`scripts/bench_docker_v2.sh`)

~700 lines, sectioned:

1. **Configuration** — top-of-file: `BENCHMARKS_AXIS1[]`, worker counts, warmup, repetitions, timeout, output paths, flag parsing.
2. **Library functions** — `log()`, `winpath()`, `join_comma()`, `parse_metrics_to_detail()`, `parse_metrics_to_rounds()`, `extract_wall_clock()`, `write_summary_row()`, `verify_g1()`, `run_docker_cycle()`, `load_axis2_configs()`, `checkpoint_completed_set()`.
3. **Main** — argparse → mkdir → docker build → CSV init → Phase A (sequential baselines, native) → Phase B (TCP rodada, docker per-cycle) → final report.

### 4.7 Resume capability (mandatory)

`--resume` is **default ON**. On startup, the script reads existing `detail.csv` (if present) and builds a set of `(benchmark, size, mode, workers)` tuples that already have ≥10 reps recorded. Each Phase A and Phase B inner loop tests the current tuple against the set and skips if complete.

`detail.csv` is line-appended via shell `>>`, which is unbuffered in append mode → every successful rep is durably on disk. Interruption (Ctrl+C, laptop sleep, docker daemon hang) is safe; re-invocation resumes.

### 4.8 G1 verification (correctness gate)

Per-rep: `verify_g1(seq_output, output.bin)` runs `relativist inspect -i <file>` on both, greps `^Agents:` and `^Redexes:` lines, compares strings.

**Limitation:** count-based G1 is weaker than `nets_graph_isomorphic` used in the bench harness. Two nets with same Agent+Redex counts but different topology pass. This is the v1 gate, kept verbatim for methodological parity. Topological correctness for Tier 3 is independently validated by the local rodada (360 datapoints all_correct=true) and by `cargo test` (1619 tests).

If a config consistently fails G1, the script logs `Rep N: G1 FAILED!`, sets `correct=false` on the row, and `all_correct=false` on the summary. The rodada continues — one bad config does not abort.

### 4.9 docker-compose.yml update

Existing `coordinator` service `command:` gets two added flags:

```yaml
- --chunk-size=${CHUNK_SIZE:-10000}
- --max-pending-lifetime=${MAX_PENDING_LIFETIME:-16}
```

Defaults match the v2 local rodada baseline (`00f9ce8`). Env-var override allows the script to sweep different values if a follow-up experiment is needed. The existing `bench-tcp` profile is **not** modified — it remains the in-process smoke.

`.env.example` gets a one-line comment update noting these env vars now also affect the `coordinator` service.

## 5. Pre-flight smoke tests (~30 min)

Before the full ~30h rodada, four quick tests:

1. **Compose update works** (~2 min) — manual `docker compose up coordinator worker --scale worker=2 --abort-on-container-exit` with a small pre-generated input. Verify `metrics.json` written, exit 0.
2. **Script syntactic** (~1 min) — `bash scripts/bench_docker_v2.sh --dry-run`. Verify all configs printed.
3. **One-config end-to-end** (~5 min) — temporarily restrict to `condup_expansion:1000`, 1 worker, 1 rep, 0 warmup. Verify CSV row written, parse OK, `correct=true`, `mode=tcp_localhost`.
4. **Resume capability** (~2 min) — re-invoke after smoke #3. Verify `[skipped] condup_expansion:1000:w=2 (already complete)` appears.

If any fails, fix before the full rodada. Cost of pre-flight: <30 min vs downside of 30h on a broken script.

## 6. Risks

| Risk | Probability | Impact | Mitigation | Plan B |
|---|---|---|---|---|
| Hybrid coordinator breaks with `--scale worker=N>1` | Low | Blocks rodada | Smoke #3 catches | Add `--no-hybrid` to coord command |
| Frame cap (256 MiB legacy L6) on 5M sizes | Medium | Configs lose data | CompactSubnet fix should resolve; monitor smoke #3 with 5M | Cap at 1M for Axis 1, document as limitation |
| `relativist inspect` API drift v1→v2 | Low | G1 doesn't run | Verify before script ships | Replace with `cmp seq_output.bin output.bin` |
| Wall-clock overshoot (>50h) | Medium | Schedule slip | Per-cycle 600s timeout limits explosion | Cut Axis 2 to ~15 core configs |
| `metrics.json` schema drift v1→v2 incompatible with parser | Medium | All rows fail G1 | Smoke #3 catches on first row | Update parser (~10 lines) |
| Per-rep metrics dumps disk usage (~3744 × ~50KB ≈ 180 MB) | Low | Disk pressure | Document in header | Cleanup post-rodada |
| Docker Desktop hang on Windows after many hours | Medium | Rodada interrupted | Resume capability covers | n/a |
| Tier 3 introduces silent G1 regression in TCP path | Low | Rodada invalid | Local rodada (360 datapoints) all_correct=true validates Tier 3 algorithmically | Mark config, report in close-out |

## 7. Cronograma

| Step | Time | Owner |
|---|---|---|
| Write `bench_docker_v2.sh` + compose update + commit | ~2h | Claude |
| Pre-flight 4 smokes | ~30min | User+Claude |
| Axis 1 — 8 configs × 4 workers × 12 reps (~384 cycles) | ~8h | Background |
| Axis 2 batch A — ~15 larger configs | ~10h | Background |
| Axis 2 batch B — ~15 remaining | ~10h | Background |
| F-3 close-out (move to `results/locked/v2_tcp_baseline/`, `progress.md`, `next-steps.md`, open D-012 if needed) | ~1h | Claude |
| **Total wall-clock** | **~3-4 days** | |
| **Total active user time** | **~2h** | |

## 8. Files modified

| File | Lines added | Notes |
|---|---|---|
| `scripts/bench_docker_v2.sh` | ~700 (new) | Main deliverable |
| `docker-compose.yml` | +2 in coordinator service | Tier 3 flags |
| `.env.example` | +1 comment | Cross-reference for coord env vars |
| `docs/plans/2026-05-02-bench-docker-v2-tcp-rodada-design.md` | this file | Design ref |

## 9. Acceptance criteria

- All 4 pre-flight smokes pass.
- Axis 1 produces ≥384 datapoints (32 configs × 12 reps) with `all_correct=true` ratio ≥ 0.95.
- Axis 2 produces ≥240 datapoints (≥20 surviving configs × 12 reps) with `all_correct=true` ratio ≥ 0.95.
- `summary.csv` rows for `mode=tcp_localhost` have non-zero `speedup_mean` and finite `cv`.
- Re-invocation after partial run skips completed configs (resume verified).
- Schema parses cleanly with `pandas.read_csv` (no quoting issues, no truncated rows).

## 10. Out-of-scope follow-ups (potential D-012)

- TCP dispatch inside `bench` subcommand (Path A from the discussion).
- Strong G1 (`nets_graph_isomorphic`) exposed via a `relativist verify` subcommand.
- Streaming chunked partitioning in coordinator load path (would require changing `coordinator.rs:842` to detect the chunk_size flag and route through the streaming partitioner).
- Phase 3 LAN rodada (real network latency, not loopback).
- Sparse representation in coordinator transport path.
