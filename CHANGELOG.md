# Changelog

All notable changes to Relativist are recorded here. The narrative is
**condensed per release** — for the full bundle-level history, see
`docs/progress.md`. For active and future work, see
`docs/next-steps.md`.

The format is loosely inspired by [Keep a Changelog][1] but adapted to
the project's bundle-driven development model (each `D-NNN` bundle is
a discrete unit of work that goes through the 6-stage SDD pipeline).
Versioning follows [Semantic Versioning][2] from v0.20.0-pre.1 onward;
earlier v0.x releases predate the formal version policy.

[1]: https://keepachangelog.com/en/1.1.0/
[2]: https://semver.org/spec/v2.0.0.html

## [Unreleased]

Active queue tracked in `docs/next-steps.md`. Highlights of work
that landed on `v2-development` after `v0.20.0-pre.1` will be summarised
here as bundles close.

### Added (D-014, infrastructure for the stress-curve campaign)

- D-014 stress-curve campaign infrastructure: `MemoryProbe`
  (TASK-0700), `StopRule` (TASK-0701), `StressCurveDescriptor`
  (TASK-0702), 4-column CSV schema extension (TASK-0703), bash
  orchestrator `scripts/stress_curve.sh` (TASK-0704), Python plot
  generator `scripts/plot_stress_curve.py` (TASK-0705), methodology
  page `docs/benchmarks/campaigns/stress-curve.md` (TASK-0706),
  6 dedicated integration tests (TASK-0707).
- The campaign dataset itself (locked directory under
  `results/locked/v2_stress_curve_<DATE>/`) lands when TASK-0708 is
  executed by the operator (overnight, ~7-8h; sentinel at
  `results/locked/v2_stress_curve_TEMPLATE/SENTINEL.md`).

---

## [v0.20.0-pre.1] — 2026-05-05 — first v2 pre-release for LAN testing

The first pre-release of the v2 series. v2 is the rewrite that
introduces a transport abstraction, a new wire format with optional
zero-copy serialisation, an explicit delta protocol for cross-partition
state, streaming partition generation for huge nets, and a free-list
arena for memory reuse. This pre-release exists so the project can
collect Phase 3 LAN data on real-network hardware before the v0.20.0
stable cut.

### What is new in v2 versus the v1 line (v0.10.x)

- **Transport abstraction** (SPEC-17). Coordinator and worker no longer
  bind to TCP directly — they speak through a `Transport` trait, with
  TCP, Unix-socket, and in-memory `ChannelTransport` implementations.
  Tests now exercise wire-level behaviour without binding ports;
  switching protocols on a real LAN is a flag, not a rewrite.
- **Wire format v2 + optional zero-copy** (SPEC-18). The default wire
  is bincode + framing + CRC32. With `--features zero-copy` and the
  `--use-zero-copy` runtime flag, payloads ship as `rkyv` archives,
  trading bincode's encode/decode CPU cost for byte-aligned reads on
  high-bandwidth links. Wire format is gated by `PROTOCOL_VERSION`;
  current value is 5.
- **Delta protocol** (SPEC-19). Between rounds, workers ship only the
  border state that actually changed since the last commutation. v2 on
  TCP-localhost sends ~42 % fewer bytes per round than v1 did on the
  same workload. The full state still ships at round 0 (cold start)
  and on-demand reconstruction; only the steady-state rounds use
  deltas.
- **Streaming partition generation** (SPEC-21). The coordinator no
  longer materialises the full input net before partitioning — it
  partitions in chunks of `--chunk-size` agents, with `--max-pending-
  lifetime` controlling how long a deferred chunk may live in the
  pending queue before forced dispatch. This unblocks reduction on
  inputs that exceed coordinator RAM. Two `docker compose` profiles
  exercise both paths in CI: `bench-tcp` for streaming, `bench-tcp-
  eager` for the v1-equivalent eager path. The `streaming-no-recycle`
  cargo feature ships an alternative build that disables free-list
  recycling under streaming mode for diagnostic comparison.
- **Arena management** (SPEC-22 v2.4). `Net.free_list` is a LIFO of
  reusable agent slots populated by `remove_agent` and consumed by
  `create_agent`. A `SparseNet` representation backed by `HashMap`
  serves as a fallback when dense allocation would overflow the
  effective arena threshold (`max_live_id + 1 > 4 × live_agent_count`,
  the metric corrected by D-011 — see below). The `--recycle-policy`
  flag controls when recycling activates; `--representation` forces a
  particular layout for benchmarking.
- **Elastic grid** (SPEC-20). Workers can join and leave during a
  reduction. The coordinator tracks a hybrid in-process + remote
  worker set, accepts join requests within a windowed admission
  protocol (`--join-window-min-ms` / `--join-window-max-ms`), and
  retains finished partitions for late-joining workers
  (`--retain-partitions`). Mid-session catastrophic departure recovery
  is currently exercised by tests but not by locked benchmarks; flagged
  experimental outside `local` mode in guide 08.
- **Spec-Driven Development pipeline.** Every change goes through six
  stages: SPLITTING (task-splitter) → TESTS (test-generator) → DEV
  (developer, TDD RED→GREEN→REFACTOR) → REVIEW (reviewer) → QA
  (adversarial) → REFACTOR. The `sdd-pipeline` agent orchestrates
  stage transitions and pinning. See `docs/WORKFLOWS.md`.

### Bundles closed since v0.10.1

This section sketches what each bundle delivered. Numbering follows
the order they ran on `v2-development`; for full per-bundle narratives
see `docs/progress.md`.

- **D-005 — Worker-side `CommutationBatch.local_wiring`**
  (SPEC-19 §3.4 Shape A). Mint-then-wire ordering, `MalformedLocalWiring`
  error variant, `PROTOCOL_VERSION 2 → 3`. 11 of 12 G1 parity gate
  cases passed at Stage 3; the remaining asymmetric CON-DUP strict=false
  case was closed in Stages 4-6.
- **D-006 — Hybrid coordinator + elastic grid skeleton** (SPEC-20
  §3.1-3.3, Option A). The 12 LLM-authored Stage 3 commits required a
  full audit (14 CRITICAL + 23 HIGH bugs); REFACTOR shipped per-phase
  on top. The reclaim path was deliberately removed and
  `GridConfig.elastic_departure` defaults to `false`; full elastic
  reconstruct deferred to v2.1.
- **D-009 — SPEC-22 Arena Management.** `Net.free_list`, SparseNet,
  R22/R30 sparse threshold, R27 debug-assertion families, integration
  regression suite. 5 CRITICALs found by adversarial QA were resolved
  in Stage 6 (CompactSubnet wire format issue deferred to TASK-0595).
- **D-010 — SPEC-21 Streaming Generation.** Pull-dispatch FSM,
  `RequestWork`/`NoMoreWork` wire variants (PROTOCOL_VERSION 5→6),
  R10b Strategy A streaming gate, `BorderClean` precision recycling,
  `streaming-no-recycle` cargo feature. 4 CRITICAL + 5 HIGH QA findings
  resolved.
- **D-011 — Partition perf regression BLOCKER + final v2 baseline.**
  Empirical bisect identified `partition::helpers::build_subnet_with_
  config` threshold metric as wrong: `id_range > 4 × live_count`
  measured the planning range, not the actual arena memory. Result:
  every healthy partition was mis-routed to SPARSE, causing +83 % wall
  on `ep_con 5M w=2`. SPEC-22 amended to v2.4 (R22 metric →
  `effective_arena_size = max_live_id + 1`, R30 reworded). Two latent
  dense-path bugs surfaced during the fix and were closed: Bug 1
  (`freeport_redirects` not propagated; SC-001 second surface) and
  Bug 2 (`next_id = 0` causing cross-partition AgentId collision; I1
  violation post-merge). Defensive guards AF-2 (`Net::create_agent`
  fresh-allocation) and AF-3 (`merge::core::merge` collision check)
  added. Bench verified perf restored within noise (~1.11× v1 on
  ep_con 5M w=2 local). Final TCP/Docker baseline locked at
  `results/locked/v2_d011_final_baseline_2026-05-04/`.
- **D-012 — Instrumentation Restore.** Three CSV columns surfaced as
  red flags in the D-011 cold post-mortem (`network_time_secs`,
  `compute_time_secs`, `mips_mean`) were structurally zero across all
  v2 datasets — not regressions of D-011, but pre-existing v2 refactor
  losses. TASK-0615 wired `Instant::now()` around recv/send sites in
  `protocol/coordinator.rs`. TASK-0616 had workers report per-round
  duration in `WorkerRoundStats.reduce_duration_secs` and the
  coordinator aggregates with **MAX** across workers (BSP critical-
  path; QA-D012-001 found that SUM produces `compute_time > wall_clock`
  for parallel workers). TASK-0618 traced the `mips_mean` literal zero
  to `scripts/bench_docker_v2.sh:283` Python-in-bash hardcode; patched
  to recompute from per-rep `total_interactions`. TASK-0617 also
  unblocked `cargo test --release` (was broken pre-D-011 by debug-only
  test imports + a non-exhaustive match for `WorkerIdMismatch`). The
  canonical post-D-012 baseline is now
  `results/locked/v2_post_d012_baseline_2026-05-05/`; sample headline
  decomposition: `ep_500k w=1` round 0 → wall 0.460 s = compute 0.10
  + network 0.39 + merge 0.04 + ~0.03 framing.

### Test floor

| Profile | v0.10.1 | v0.20.0-pre.1 | Δ |
|---|---|---|---|
| `cargo test` (default debug) | 690 | 1798 | +1108 |
| `cargo test --features zero-copy` | n/a | 1842 | NEW |
| `cargo test --features streaming-no-recycle` | n/a | 1789 | NEW |
| `cargo test --release` | broken | 1740 | NEW (was uncompilable) |
| v1 inviolable floor (frozen on `v1-feature-complete`) | 690 | 690 | invariant |

### Known limitations and follow-ups carried into v0.20.0-pre.1

- **Sequential `mips_mean` still 0.000.** `total_interactions` is wired
  in the worker→coordinator path only; sequential `reduce_all` does not
  increment the counter. Tracked as `D-012-FU-SEQ-MIPS`. Cosmetic —
  does not affect distributed analysis.
- **`condup_expansion` floor effect.** At `N=1000 w=1` v2 reports
  ~54 ms while v1 reported ~2 ms. Setup-time asymmetry between
  sequential (which times generation) and distributed (which does not)
  is the dominant cause. Tracked as `D-011-FU-CONDUP`.
- **Bench subcommand has no real TCP path.** `relativist bench`
  hardcodes `Mode::Local`; the docker-compose `bench-tcp` /
  `bench-tcp-eager` profiles are the canonical TCP path for the
  benchmark harness. Tracked as `TASK-0620`.
- **`cfg(debug_assertions)` audit incomplete.** `TASK-0617` fixed four
  instances of debug-only symbols leaking into `#[cfg(test)]`-but-
  non-`debug_assertions` code; broader manifestations may still exist.
  Tracked as `TASK-0621`.
- **Hardening backlog.** D-013 inherits the SPEC-21/22 hardening
  follow-ups originally numbered D-011 before the BLOCKER 2026-05-04
  redirect consumed that slot. Inventory in `docs/next-steps.md`.

### Specs added or amended in this release

| Spec | Status |
|---|---|
| SPEC-17 transport abstraction | new |
| SPEC-18 wire format v2 (optional zero-copy) | new; PROTOCOL_VERSION up to 5 |
| SPEC-19 delta protocol | new; CompactSubnet free_list deferred to TASK-0595 |
| SPEC-20 elastic grid | new (Reviewed v2; §3.1-3.3 shipped, full §3.4-3.6 reclaim deferred) |
| SPEC-21 streaming generation | new (Reviewed v2) |
| SPEC-22 arena management | new (Reviewed v2.4 after D-011 amendment) |
| SPEC-23..27 | drafted; not yet shipping |
| SPEC-01 invariants | amended (I3 → I3' Uniqueness via D-009 §3.8 A1) |
| SPEC-02 net representation | amended (Net::union via SPEC-20 §3.8 A7) |
| SPEC-04 partition | amended (R10a/R22 via SPEC-22 §3.8 A4) |
| SPEC-13 system architecture | amended (FSM via SPEC-20 §3.8 A5) |
| SPEC-18 wire format | amended (R28 + PROTOCOL_VERSION via SPEC-22 §3.8 A6) |
| SPEC-19 delta protocol | amended (R12a slot-id stability via SPEC-22 §3.8 A8) |

### How to use this pre-release

This pre-release is intended for **Phase 3 LAN testing on real
networked hardware**. It is not the v0.20.0 stable cut.

- `git checkout v0.20.0-pre.1` and follow `docs/benchmarks/phase-3-lan.md`
  for the dual-axis (bincode-only / zero-copy) protocol.
- All `--release` builds work; `cargo build --release --features zero-copy`
  builds the Axis 2 binary.
- File any LAN-specific issues in the GitHub tracker; they will be
  triaged for v0.20.0-pre.2 or v0.20.0.

---

## [v0.10.1] — 2026-04-11 — v1 frozen baseline (last v1 release)

This is the formal v1 freeze. The branch `v1-feature-complete` and
the tag `v0.10.0-bench` capture the same commit; `v0.10.1` is a
trivial follow-up with the v1 phase 2 baseline data. Test floor: 690.
Frozen baseline lives at `results/locked/v1_local_baseline/`.

### Earlier v0.x releases

For the v0.2..v0.10 progression (initial reduction implementation,
file format work, deployment skeleton, security primitives, observability,
benchmarks suite, Church arithmetic), consult `git log v0.2.0..v0.10.1`
and the corresponding tag annotations on the GitHub release page. The
v1 line is now in maintenance mode; new work happens on `v2-development`.

[Unreleased]: https://github.com/andrade-filipe/relativist/compare/v0.20.0-pre.1...HEAD
[v0.20.0-pre.1]: https://github.com/andrade-filipe/relativist/compare/v0.10.1...v0.20.0-pre.1
[v0.10.1]: https://github.com/andrade-filipe/relativist/releases/tag/v0.10.1
