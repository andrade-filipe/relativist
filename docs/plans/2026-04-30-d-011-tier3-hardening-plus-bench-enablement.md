# D-011 — Tier 3 Hardening + Bench Enablement (Consolidated Plan)

**Date:** 2026-04-30
**Branch:** `v2-development`
**Status:** APPROVED — ready to dispatch
**Predecessor:** D-010 (SPEC-21 streaming generation, closed `4d52597`)
**Successor:** D-012/D-013 (master plan original) or Tier 4 bundles, depending on D-011 outcomes

---

## Context

D-009 (SPEC-22 arena recycling + SparseNet) and D-010 (SPEC-21 streaming generation + chunked partitioning) closed all 6 SDD stages. Tier 1 and Tier 2 (D-006 elastic grid, detection-only) are already on the default path. **However, the bench harness does not actively exercise Tier 3** — `BenchmarkSuiteConfig` invokes `run_grid` with `GridConfig::default()` (eager path), so the three memory optimizations (free-list, streaming, chunked) ship "compiled but not measured."

**User's goal (2026-04-30):** Re-run the same suite of 13 benchmarks (same problems — `ep_annihilation`, `dual_tree`, `cascade_cross`, etc.) in `--mode local` AND `--mode tcp` (docker), with Tier 1+2+3 all active. Compare **aggregated** against `results/locked/v1_local_baseline/` — single narrative "three optimizations combined." The pre-release v2 with this bench feeds TCC data; Tiers 4 (UX) and 5 (internet) will only be touched after the bench validates M5.

**Blockers mapped via Explore (file:line citations):**

| Blocker | Evidence |
|---|---|
| Bench harness has no Tier 3 flags | `relativist-core/src/config.rs:571-629` (`BenchArgs` has 12 fields, none for Tier 3); `relativist-core/src/bench/mod.rs:231-251` (`BenchmarkSuiteConfig`) |
| QA-D009-001 (CompactSubnet wire drops `free_list`) | Blocks `--mode tcp`; requires SPEC-19 amendment first |
| QA-D010-009 residual (`max_pending_lifetime` not threaded) | Free-list grows unbounded in legacy callers; biases memory measurement |
| Docker broken post workspace refactor | `Dockerfile:5-6` does `COPY src/ src/` and `COPY benches/ benches/`, but repo is now workspace (`relativist-core/`, `relativist-cli/`) |
| SPEC-09 silent on Tier 3 metrics | No `peak_memory_during_construction`, no sparse net watermark; needs §3.3.X / §3.6 / §4.9 amendments |

## Pre-Bundle State (Read-Only)

- **Branch:** `v2-development` (clean working tree, last commit `4d52597`)
- **Test floor to preserve:** ≥ **1683 default / 1726 zero-copy / 1680 streaming-no-recycle**. v1 floor: 690 (inviolable).
- **Commits ahead of `origin/v2-development`:** 9 (D-010 closure unpushed). User will push manually after D-011 closes.
- **Mandatory gates after every commit:** `cargo test`, `cargo test --features zero-copy`, `cargo clippy --all-features -- -D warnings`, `cargo fmt --check` — all via `/c/Users/Filipe/.cargo/bin/cargo.exe` (cargo not on PATH on this Windows host).

## Scope: Phases A–F (single bundle D-011)

Mirrors the D-009 structure (Phase A spec → B core → C integration → D CI → E close). One closure commit at the end.

### Phase A — SPEC-19 amendment (QA-D009-001)

**Owner:** ESPECIALISTA EM SPECS (handoff brief; user dispatches from TCC root session per memory `feedback_especialista_specs_dispatch.md`).

**Scope:** Amendment R35a in `specs/SPEC-19-delta-protocol.md` §3.4 specifying that CompactSubnet wire format MUST encode `free_list: Vec<AgentId>` as a suffix after the agent array. Reason: today's serialization loses free-list across deserialization, making `next_id` diverge between coordinator and worker (violates SPEC-22 R10b/R12a).

**Stages:** spec-only (Round 1 spec-critic + Round 2 defense + closure log in `docs/spec-reviews/`).

**Deliverable:** SPEC-19 commit `spec(d-011): amend R35a — CompactSubnet free_list encoding`.

**Gate:** spec-critic Round 2 closes with no objections. Blocks Phase B item B-1.

### Phase B — Hardening (D-011 audit deferrals)

**Owner:** developer (Sonnet) → reviewer → qa (Opus) → developer refactor.

**Scope (full 6-stage SDD):**

| Item | Severity | File |
|---|---|---|
| **B-1** QA-D009-001 fix (CompactSubnet wire — depends on Phase A) | CRITICAL | `relativist-core/src/net/sparse.rs:308`, wire serialization path |
| **B-2** QA-D010-009 residual: thread `GridConfig.max_pending_lifetime` through legacy callers (`generate_and_partition_chunked_with_delta`) | HIGH | `relativist-core/src/merge/helpers.rs` |
| **B-3** QA-D010-014: debug_assertions ABI drift | MEDIUM | counter fields gated in streaming/arena |
| **B-4** QA-D010-010..013, 016 (placeholder semantics, IT-0591 strengthening, parallel state collapse, LIFO stalemate) | MEDIUM/LOW | `relativist-core/src/partition/streaming.rs`, tests |

**Out of scope:** QA-D010-011 (`streaming-no-recycle` debug_asserts) — only relevant if bench runs with that feature; default doesn't. Document deferral in `docs/next-steps.md`.

**Test floor delta expected:** +30 to +60 tests (RED→GREEN cycles for each finding).

**Deliverable:** granular commits `fix(streaming): QA-D010-NNN — <description>`, one per finding.

### Phase C — Bench harness wiring

**Owner:** developer.

**Scope (full 6-stage SDD):**

**C-1: Extend `BenchmarkSuiteConfig`** (`relativist-core/src/bench/mod.rs:231-251`):
```rust
pub chunk_size: Option<u32>,           // None → eager path (status quo); Some(N) → streaming
pub max_pending_lifetime: u32,         // default: 16 (matches coordinator CLI default)
pub recycle_policy: RecyclePolicy,     // enum: DisableUnderDelta | BorderClean
pub representation: NetRepresentation, // enum: Dense | Sparse
```

**C-2: Path selection in `bench/suite.rs:313-438`:**
- If `chunk_size.is_some()` → call `merge::generate_and_partition_chunked_with_chunk_size_and_lifetime` (wired in `5a54111`) with the benchmark's stream (`benchmark.make_net_stream(size, chunk_size)`).
- Otherwise → keep `run_grid(net, GridConfig{..max_pending_lifetime: config.max_pending_lifetime, ..default()}, &strategy)` (current path, now with lifetime threaded).

**C-3: CLI flags in `relativist-core/src/config.rs:571-629`** (`BenchArgs`):
```
--chunk-size <N>             default: <none> (eager)
--max-pending-lifetime <N>   default: 16
--recycle-policy <POLICY>    default: disable-under-delta
--representation <MODE>      default: dense
```

**C-4: Streaming generators wiring** — only `ep_annihilation` for now (per user decision; the other 12 benchmarks use the default-impl path via `default_chunked_iter` already working since `ec81eb3`).

**C-5: Pre-reduction memory measurement** — in `relativist-core/src/bench/memory.rs:8-30`, add `get_peak_memory_at_construction_complete()` called between net construction and `reduce_all` in `bench/suite.rs:102` (sequential) and ~221 (grid). Required to validate M5 (peak coordinator memory).

**Deliverable:** ~400 LoC production + ~100 LoC test in ~3-4 commits. No change to existing CSVs (new fields are additional columns; backward-compatible with `v1_local_baseline`).

**Gate:** smoke run `cargo run --bin relativist -- bench --benchmark ep_annihilation --sizes 1000 --workers 2 --chunk-size 100 --representation dense` produces a valid CSV with new columns populated.

### Phase D — SparseNet micro-bench (validate 2.32)

**Owner:** developer.

**Scope (full 6-stage SDD, micro):**

**D-1:** Add `--representation sparse` path in `bench/suite.rs` that builds `SparseNet` instead of `Net` during `make_net()`, measures peak memory **before** any reduction, then converts via `SparseNet::to_dense(id_range)` (`relativist-core/src/net/sparse.rs:308`) for the rest of the pipeline (reduction continues on dense `Net`).

**D-2:** Limit to 1 benchmark (`dual_tree`) and 2 sizes (small + large to validate the acceptance gate "<30% dense in ep_construct(5M)" — adapted to dual_tree).

**D-3:** Separate sub-CSV `sparse_construction_memory.csv` (does not pollute main CSVs). Columns: `benchmark, size, representation, peak_construction_bytes, ratio_to_dense`.

**Out of scope:** SparseNet across all benchmarks; SparseNet during reduction (dominant memory is dense arena post-construction).

**Deliverable:** ~150 LoC + 1 page in DATA-COLLECTION-PLAN.

### Phase E — Docker fix + TCP smoke

**Owner:** cicd subagent (Dockerfile/CI specialist) or developer.

**Scope (full 6-stage SDD, light):**

**E-1: `Dockerfile` fix** — replace `COPY src/ src/` and `COPY benches/ benches/` with workspace-aware:
```dockerfile
COPY Cargo.toml Cargo.lock ./
COPY relativist-core/ relativist-core/
COPY relativist-cli/ relativist-cli/
RUN cargo build --release -p relativist-cli
```
Binary still at `/app/target/release/relativist`.

**E-2: `docker-compose.yml`** — add a `bench-tcp` profile/service that orchestrates coordinator + N workers and runs bench in TCP mode, parameterized via env vars (`CHUNK_SIZE`, `MAX_PENDING_LIFETIME`, `RECYCLE_POLICY`). Maintain backward-compat with the existing coordinator/worker profile.

**E-3: Validate post-D-006 hybrid coordinator** — coordinator now reduces a local partition; verify healthcheck/logs in compose still work. Document the difference vs v1 docker bench in `docs/DOCKER.md` (create if missing, ~50 lines).

**E-4: Smoke test TCP** — `docker compose run bench-tcp --benchmark ep_annihilation --sizes 1000 --workers 2 --chunk-size 100 --mode tcp` must complete in <60s and produce a valid CSV with G1 isomorphism passing. **This is the test that validates QA-D009-001 fix (Phase B-1) actually closed the TCP path.**

**Deliverable:** Dockerfile + compose + DOCKER.md + smoke test in CI (`.github/workflows/`).

### Phase F — SPEC-09 amendment + close-out + bench run

**Owner:** ESPECIALISTA EM SPECS (handoff brief; user dispatches from TCC root) → user → developer (close-out).

**F-1 (ESPECIALISTA EM SPECS):** SPEC-09 amendment covering Tier 3:
- §3.3.X "Streaming Representation Metrics": `peak_memory_during_construction`, `sparse_net_agent_count_watermark`
- §3.6 new clause: "Construction-phase isomorphism" (when `representation=sparse`, the `AgentBatch` stream MUST reconstruct a net agent-isomorphic to `make_net(size)` before any reduction)
- §4.9 "Streaming Architecture": chunk lifecycle, pending-reference bounds, recycle policy contracts

**F-2 (user / sdd-pipeline):** run the bench rodada — local mode first, then TCP/docker. Explicit commands:
```bash
# Local — same v1 suite, with Tier 3 active
cargo run --release --bin relativist -- bench \
  --workers 1,2,4,8 \
  --chunk-size 10000 \
  --max-pending-lifetime 16 \
  --representation dense \
  --csv-detail results/v2_local_full_detail.csv \
  --csv-rounds  results/v2_local_full_rounds.csv \
  --csv-summary results/v2_local_full_summary.csv

# Docker TCP
docker compose --profile bench-tcp run \
  -e CHUNK_SIZE=10000 \
  -e MAX_PENDING_LIFETIME=16 \
  bench-tcp --workers 2,4,8 --csv-summary /data/v2_tcp_summary.csv

# SparseNet micro
cargo run --release --bin relativist -- bench \
  --benchmark dual_tree --sizes 5000,50000 --representation sparse --workers 1 \
  --csv-summary results/sparse_construction_memory.csv
```

**F-3 (developer):** close-out commit `chore(d-011): close Tier 3 hardening + bench enablement`:
- `docs/progress.md`: entry consolidating phases A-F with SHAs
- `docs/next-steps.md`: D-011 → CLOSED; open D-012/D-013 (master plan original) or mark as absorbed; open Tier 4 as NEXT
- Locked baseline: `results/locked/v2_local_full_baseline/` (copy of new CSVs, marked as TCC reference)
- `.gitattributes`: ensure CSVs in `results/locked/` are read-only by convention

## Dependency Graph

```
A (SPEC-19 amend) ─→ B-1 (CompactSubnet fix) ─┐
                                                ├─→ E (Docker TCP smoke) ─→ F-2 (bench TCP)
                              B-2 (lifetime) ─┐ │
                              B-3 (ABI)       │ │
                              B-4 (medium)   ─┤ │
                                              ↓ ↓
                                        C (bench wiring) ─→ F-2 (bench local) ─→ F-3 (close-out)
                                              ↓
                                        D (sparse micro)
F-1 (SPEC-09 amend) is independent; can run in parallel with B-D, must close before F-3.
```

**Possible parallelism:** Phase A + F-1 simultaneously (both ESPECIALISTA EM SPECS, but distinct specs). Phase B items 2/3/4 in parallel with C (but B-1 blocks E).

## Estimate

| Phase | LoC prod | LoC test | Days |
|---|---:|---:|---:|
| A (SPEC-19) | – | – | 0.5 |
| B (hardening) | ~200 | ~100 | 1.5 |
| C (bench wiring) | ~400 | ~100 | 2.0 |
| D (sparse micro) | ~150 | ~50 | 1.0 |
| E (docker) | ~80 | ~50 | 1.0 |
| F (spec + bench + close) | ~50 | – | 1.0 + bench run (~4-6h cpu) |
| **Total** | **~880** | **~300** | **~7 days dev + bench rodada** |

Test floor at end: estimated **1750–1820 default** (D-009/D-010 brought ~+250 tests; B/C/D should bring ~+70).

## Verification (per-phase gates)

| Gate | Command | Criterion |
|---|---|---|
| Build | `cargo build --release --workspace` | exit 0 |
| Tests default | `cargo test --workspace` | ≥ 1683 |
| Tests zero-copy | `cargo test --workspace --features zero-copy` | ≥ 1726 |
| Lint | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | exit 0 |
| Format | `cargo fmt --check` | exit 0 |
| Phase A | spec-critic Round 2 closure log | no objections |
| Phase B | findings inventory cleared | 0 CRITICAL + 0 HIGH residuals |
| Phase C | smoke `bench --chunk-size=100 --benchmark ep_annihilation` | CSV produced, new columns populated |
| Phase D | sparse vs dense memory comparison | sparse < 80% dense at dual_tree(5000) (relaxed vs spec acceptance — informative micro-bench) |
| Phase E | `docker compose run bench-tcp --benchmark ep_annihilation` | smoke passes, G1 OK |
| Phase F-2 | full bench rodada complete | all 13 benchmarks complete, CSVs generated, no speedup regression vs `v1_local_baseline` |

## Critical Files To Modify

| File | Phase | Reason |
|---|---|---|
| `specs/SPEC-19-delta-protocol.md` | A | R35a free_list encoding amendment |
| `specs/SPEC-09-benchmarks.md` | F-1 | §3.3.X + §3.6 + §4.9 amendments |
| `relativist-core/src/net/sparse.rs:308` | B-1 | `to_dense` populates `Net.free_list` from sparse gaps |
| `relativist-core/src/merge/helpers.rs` | B-2 | Thread `max_pending_lifetime` in legacy callers |
| `relativist-core/src/partition/streaming.rs` | B-3, B-4 | counter ABI fix, placeholder semantics, IT-0591 |
| `relativist-core/src/bench/mod.rs:231-251` | C-1 | `BenchmarkSuiteConfig` 4 new fields |
| `relativist-core/src/bench/suite.rs:313-438` | C-2 | path selection (eager vs streaming-chunked) |
| `relativist-core/src/config.rs:571-629` | C-3 | `BenchArgs` 4 new flags |
| `relativist-core/src/bench/memory.rs` | C-5 | `get_peak_memory_at_construction_complete` |
| `relativist-core/src/bench/streaming.rs` | C-4 | wire `ep_annihilation_stream` into bench path |
| `Dockerfile` | E-1 | workspace-aware COPY |
| `docker-compose.yml` | E-2 | `bench-tcp` profile |
| `docs/DOCKER.md` (new) | E-3 | post-D-006 changes |
| `docs/DATA-COLLECTION-PLAN.md` | D, F | Tier 3 matrix + sparse micro |
| `docs/progress.md` | F-3 | D-011 close entry |
| `docs/next-steps.md` | F-3 | D-011 → CLOSED + open Tier 4 |
| `results/locked/v2_local_full_baseline/` (new dir) | F-3 | new frozen baseline for the TCC |

## Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| Phase A spec amendment stalls in spec-critic Round 2 | MEDIUM | blocks B-1 and E | prepare lean handoff brief; user dispatches early in bundle |
| C-2 path selection introduces silent regression in default eager path | MEDIUM | masks TCC baseline | regression test: run `bench --benchmark ep_annihilation --workers 2` WITHOUT `--chunk-size` and compare to last known number bit-for-bit |
| QA-D009-001 fix changes wire format and breaks existing workers | LOW (no deploy) | none (single-version v2) | none — it's the correct fix |
| Bench TCP/docker uncovers post-D-006 hybrid-coordinator regression | MEDIUM | delays F-2 | Phase E-3 isolated to diagnose before bench rodada |
| Full bench rodada takes >12h and blocks TCC work | LOW | 1-day delay | run overnight; F-3 close-out only after results |
| SparseNet micro-bench reveals bug not detected in D-009 | LOW | delays B-1/Phase D | treat as new QA finding, escalate to D-012 if grave |

## Out of Scope (Deferred)

- **Tier 4 (UX):** Tailscale, installer, mDNS — bundles D-014..D-016 from master plan, opened after D-011 closes.
- **Tier 5 (advanced):** D-017 (intelligent partitioning), D-018 (rayon), D-019 (streaming arithmetic encoding) — decision deferred post-bench.
- **v2.1 deferrals (Tier 2):** full `elastic_departure=true` reclaim, `worker_streams: Vec→BTreeMap`, RetainedStateRegistry persistence — do not block this bench.
- **SparseNet across all 13 benchmarks:** Phase D is limited to `dual_tree`; expansion is v2.1.
- **Tier-by-tier decomposed comparison:** user chose aggregated.
- **Push to origin:** after F-3, user pushes manually.

## Reference Inputs

- `docs/next-steps.md` — D-011 inventory (already authored 2026-04-30)
- `docs/plans/2026-04-24-tier-4-master-plan.md` — original Tier 3 bundle layout (D-011 here SUPERSEDES the master plan's "D-011 Streaming Net Generation" since that scope was absorbed by D-010)
- `docs/reviews/REVIEW-PHASE-D010-spec21-streaming-2026-04-28.md`
- `docs/qa/QA-PHASE-D010-spec21-streaming-2026-04-28.md`
- `docs/reviews/REVIEW-PHASE-D009-spec22-arena-2026-04-27.md`
- `docs/qa/QA-PHASE-D009-spec22-arena-2026-04-27.md`
- `specs/SPEC-19-delta-protocol.md` (§3.4 R31-R36 — wire format)
- `specs/SPEC-21-streaming-generation.md`
- `specs/SPEC-22-arena-management.md`
- `specs/SPEC-09-benchmarks.md` (§3 Requirements, §4 Design)
