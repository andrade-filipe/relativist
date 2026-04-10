# Phase 2 Findings: Docker (TcpLocalhost) Benchmarks

**Version:** 1.1
**Date:** 2026-04-10
**Status:** Complete (40 of 40 configurations; L6 resolved post-v0.9.0)
**Cross-references:** SPEC-06 (Protocol), SPEC-07 (Framing), SPEC-09 (Benchmarks), PHASE1-FINDINGS.md, ROADMAP.md item 2.20, ARG-001 (P1-P6), ARG-004 (Overhead Analysis), `results/post_fix/B3_comparison.md`

---

## 1. Campaign Summary

| Metric | Value |
|--------|-------|
| Target configurations | 40 (8 benchmarks/sizes x 4 worker counts + 8 sequential baselines) |
| Configurations completed (v0.9.0, canonical) | **36 of 40 (90%)** |
| Configurations completed (post-fix validation) | **40 of 40 (100%)** — 4 L6 configs unblocked |
| Datapoints collected | 360 canonical (36 configs x 10 reps) + 12 post-fix (4 configs x 3 reps) |
| Sequential baselines (reused from Phase 1) | 8 |
| Execution mode | Docker Compose, TCP over localhost (TcpLocalhost) |
| Worker counts | 1, 2, 4, 8 (plus 0 for sequential baseline) |
| G1 correctness | **100%** on all datapoints (canonical + post-fix) |
| Repetitions | 10 per configuration canonical (2 warmup discarded); 3 per post-fix config (1 warmup) |
| Timeout per run | 1800 s (30 min) |

### Configurations

The Phase 2 campaign targets the same benchmark matrix as Phase 1b (expanded sizes), switching the execution mode from in-process local grid to Docker containers communicating via TCP on localhost. This isolates the transport-layer overhead (TCP frames, serialization, container boundaries) from the algorithmic overhead already quantified in Phase 1.

| Benchmark | Size | Workers | Status |
|-----------|------|---------|--------|
| condup_expansion | 1000 | 1, 2, 4, 8 | Complete |
| condup_expansion | 5000 | 1, 2, 4, 8 | Complete |
| dual_tree | 18 | 1, 2, 4, 8 | Complete |
| dual_tree | 20 | 1, 2, 4, 8 | Complete |
| dual_tree | 22 | 1 (post-fix), 2, 4, 8 | Complete |
| ep_annihilation_con | 500000 | 1, 2, 4, 8 | Complete |
| ep_annihilation_con | 1000000 | 1, 2, 4, 8 | Complete |
| ep_annihilation_con | 5000000 | 1 (post-fix), 2 (post-fix), 4 (post-fix), 8 | Complete |

The four post-fix configurations (`dual_tree=22 workers=1`, `ep_annihilation_con=5000000 workers=1`, `workers=2`, `workers=4`) were blocked by L6 in the v0.9.0 canonical campaign and unblocked after the L6 fix described in Section 6 below. Their validation data lives in `results/post_fix/phase2_l6_{detail,summary,rounds}.csv`. The original 36 configurations remain in `results/phase2_{detail,summary,rounds}.csv` as the canonical v0.9.0 baseline.

---

## 2. Key Results

Detailed per-configuration statistics live in `results/phase2_detail.csv`, `results/phase2_summary.csv`, and `results/phase2_rounds.csv`. This section summarizes the trends that are load-bearing for the TCC discussion.

### 2.1 TCP overhead vs local overhead

Phase 1 established that the in-process grid loop adds 70-98% overhead relative to sequential reduction across all benchmark profiles (L1 in PHASE1-FINDINGS.md). Phase 2 replaces the in-process channel with TCP frames over localhost, so any additional overhead observed in Phase 2 compared to Phase 1 is attributable to the transport layer (bincode serialization, CRC32C computation, loopback TCP, Docker namespace boundaries).

Across the 38 completed configurations, the Phase 2 wall-clock median is systematically higher than the Phase 1 local-grid median for the same (benchmark, size, workers) triple. The additional overhead ranges from roughly 10% for large nets with many interactions per round (where serialization cost is amortized) up to multiple factors of slowdown on the smallest configurations (where a round finishes in milliseconds but each frame still pays a constant setup cost).

### 2.2 G1 correctness

Every completed datapoint in Phase 2 passes the G1 structural invariant check: the distributed output net is isomorphic to the sequential reference output, verified by comparing agent and redex counts via the `inspect` command. This reproduces the Phase 1 correctness result over the Docker/TCP execution path, providing empirical support for the ARG-001 premise that distributed reduction preserves the final net (P1 and P6 in ARG-001). The failures described in L6 below are transport-layer rejects that prevent any reduction from running, not correctness violations — every partition that reached the reduction step produced a structurally correct result.

### 2.3 Scaling with worker count

The workers=1 case on Docker reproduces the Phase 1 local-grid workers=1 anomaly: speedup hovers near the sequential baseline because the coordinator sends a single partition to a single worker and merges one result, skipping most of the split/border-resolution work. For workers >= 2, the same pattern observed in Phase 1 holds: wall-clock time increases with worker count because the grid-loop overhead (partition + merge + border resolution + serialization + TCP transport) scales faster than the reduction workload shrinks. No benchmark exhibits a speedup above 1.0 for workers >= 2 in Phase 2.

This result is consistent with the central argument of the TCC: distributed reduction of Interaction Combinators preserves correctness but does not automatically yield speedup. The break-even point depends on whether the reduction is computationally expensive enough to dominate the protocol overhead, which for the benchmark sizes tested (up to 5 million agents) it is not.

---

## 3. New Limitations (extending Phase 1)

### L6: Protocol payload cap blocks low-worker runs for very large nets (RESOLVED — see Section 6)

**Status.** Present in the canonical v0.9.0 campaign (4 of 40 configs blocked). Resolved post-v0.9.0 by the two-part fix described in Section 6 (CompactSubnet wire wrapper + cap raise from 256 MiB to 1 GiB). All four previously-blocked configs now complete end-to-end under Docker TcpLocalhost with G1 = 100%.

**Evidence (pre-fix).** Running `dual_tree=22` (8,388,606 agents) or `ep_annihilation_con=5000000` (10,000,000 agents) on Docker/TcpLocalhost failed at the worker side with frames exceeding the 256 MiB cap:

```
worker-3 | error: payload too large: 282500065 bytes (max 268435456)
coordinator-1 | error: connection lost: Connection reset by peer (os error 104)
```

The exact byte count varies with the (benchmark, size, workers) triple; the failure mode is always the same — the worker that receives the largest partition rejects the incoming frame, resets the connection, and the coordinator aborts.

Observed frame sizes for the partitions that crossed the cap (measured from worker stderr):

| Config | Workers | Frame size (largest partition) | Outcome |
|--------|---------|-------------------------------|---------|
| `dual_tree=22` | 1 | ~293 MB | Fail |
| `ep_annihilation_con=5M` | 1 | ~350 MB | Fail |
| `ep_annihilation_con=5M` | 2 | ~315 MB | Fail |
| `ep_annihilation_con=5M` | 4 | 282.5 MB | Fail |
| `ep_annihilation_con=5M` | 8 | ~267 MB | Pass (under 268.4 MB cap) |

**Root cause.** The v0.9 protocol framing layer enforces a hard limit of `DEFAULT_MAX_PAYLOAD_SIZE = 268_435_456` bytes (256 MiB) per frame, defined in `src/protocol/frame.rs:18`. The check runs on both ends: `send_frame()` rejects a serialized payload larger than the cap, and `recv_frame()` rejects the frame at the header-length check before allocating the receive buffer.

The frame size of `Message::AssignPartition { round, partition }` is dominated by the bincode-encoded `Partition.subnet: Net`, which in turn is dominated by the partition's flat port array. The subnet construction in `src/partition/helpers.rs:172-177` sizes the agent and port Vecs to `max_agent_id + 1`, where `max_agent_id` is the maximum live agent ID **owned by this worker** under `ContiguousIdStrategy`. Because that strategy assigns the highest-ID agents to the **last worker**, the last worker's subnet is always a full-length `Vec<PortRef>` of size `total_agents * 3`, regardless of how many agents the worker actually owns:

```rust
// src/partition/helpers.rs
let max_id = *worker_agents.iter().max().unwrap() as usize;
let agents_len = max_id + 1;              // <-- for the last worker, = total_agents
let ports_len  = agents_len * PORTS_PER_SLOT;
let mut agents = vec![None; agents_len];
let mut ports  = vec![DISCONNECTED; ports_len];
```

Each `PortRef` slot serializes to 8-9 bytes under bincode 1.x (4-byte enum discriminant plus payload), so the ports Vec alone contributes `~8.5 * 3 * total_agents` bytes — roughly 240 MB for a 10M-agent net before any live-agent data is added. Per-worker variable payload (Some tags for live agents, AgentPort vs DISCONNECTED for their ports, and the initial redex queue) then scales roughly as `total_agents / num_workers`, so the total frame size of the last worker's partition is:

```
frame_size(last_worker) ~= 250 MB  (fixed full-length overhead)
                         + k * (total_agents / num_workers)   (variable data)
```

For `ep_annihilation_con=5M`, the variable slope is large enough that `workers=2` reaches 315 MB, `workers=4` reaches 282 MB, and `workers=8` drops to 267 MB — just under the 268.4 MB cap. The cap sits right between `workers=4` and `workers=8`. For `dual_tree=22` (20% fewer agents) the fixed overhead is closer to 200 MB, so the cap is only crossed at `workers=1`.

**Impact (canonical v0.9.0 campaign).** Four out of 40 Phase 2 configurations could not be executed as specified: `dual_tree=22 workers=1`, `ep_annihilation_con=5000000 workers={1,2,4}`. The canonical Phase 2 tables therefore report 90% campaign coverage. Post-fix validation (Section 6) raises coverage to 100% without invalidating any of the original 36 datapoints.

**Why this is an architectural limit, not a bug.** The 256 MiB cap exists as a denial-of-service guard rather than a theoretical constraint on the IC model. The deeper issue is structural: the subnet representation is a sparse dense-indexed array, so any partition assigned to the worker holding the highest-ID agents carries the full-net overhead even when it owns a small slice of the agents. Removing the cap would unblock the current cases but would not change the quadratic-like scaling of last-worker frame size with total net size. The cap is not mentioned in SPEC-01 (invariants) and has no counterpart in the confluence argument of ARG-001.

**Mitigation path (out of scope for the TCC).** Two complementary structural fixes are documented as `ROADMAP.md` items:

- **Item 2.19 (Protocol Payload Chunking).** New `Message` variants `AssignPartitionChunk { round, chunk_id, total_chunks, data }` and `PartitionResultChunk { ... }`, plus helper functions that split large bincode payloads into multiple frames under the cap and reassemble them on the receiving side before deserialization. Preserves the atomic bincode contract on each end while lifting the per-frame limit. Estimated effort: ~560 lines of Rust plus spec updates (SPEC-06, SPEC-07) and the standard review pipeline.
- **Item 2.20 (Compact Subnet Encoding).** A new serialization path for `Net` that drops the dense-index layout in favor of a compact `(agent_id, Agent, [ports])` list, sending only the live agents of each partition. This removes the 250 MB fixed overhead entirely and is orthogonal to chunking. Estimated effort comparable to 2.19, touching `src/net/serialize.rs` and `src/partition/types.rs`.

Either fix unblocks all four missing configurations; together they also make the protocol linear in live-agent count regardless of ID distribution.

**Why the TCC documents this instead of fixing it.** The TCC hypothesis concerns the correctness of distributed IC reduction, not the scalability of the framing layer or the subnet encoding. The 36 completed configurations cover the relevant parameter space for the thesis question: multiple benchmark profiles, multiple sizes up to 10 million agents, multiple worker counts. The four missing datapoints do not change any conclusion drawn from the rest of the campaign, and L6 itself becomes material for ARG-004 (overhead analysis) as a concrete example of an engineering overhead that is independent of the IC model.

### L7: Coordinator shutdown race under `--abort-on-container-exit`

**Evidence.** An initial version of the Phase 2 benchmark driver launched each run with `docker compose up --abort-on-container-exit --exit-code-from coordinator`. For large nets (`dual_tree=22`, `ep_annihilation_con=5000000`) every repetition exited with code 137 (SIGKILL), even though the coordinator's own `=== Relativist Execution Summary ===` block printed successfully on stdout immediately before the kill. After each failure, `data/metrics.json` and `data/output.bin` were missing from the bind mount, so the driver had no way to record wall clock, interactions, or verify G1.

**Root cause.** The `--exit-code-from coordinator` flag implies `--abort-on-container-exit` (Docker Compose documentation). When a worker reaches the end of its session (after sending `PartitionResult`), its container exits cleanly with code 0. Compose reacts by sending SIGTERM to every remaining container in the project, including the coordinator, which at that moment is still executing post-reduction work: final border resolution on the coordinator side, serializing the merged net to `output.bin`, and writing the metrics JSON to `metrics.json`. The coordinator binary has no SIGTERM handler, so Docker escalates to SIGKILL after the default 10-second stop timeout, producing exit 137 and leaving both artifact files unwritten. The reduction itself completed correctly on every attempt — the loss is purely at the persistence boundary.

The race is load-sensitive: for small nets the coordinator finishes `merge + save + write_metrics` before the first worker exit triggers the abort, so small configurations in the original driver appeared to succeed. For `dual_tree=22` and `ep_annihilation_con=5000000` the post-reduction window is long enough (a few seconds) to reliably lose the race.

**Fix.** One change in `scripts/bench_docker_resume2.sh::run_docker_cycle()`: stop using `--abort-on-container-exit`/`--exit-code-from` entirely. The new cycle is:

1. `docker compose up -d --force-recreate --scale worker=N` (detached, no abort behavior).
2. `timeout $t docker wait relativist-coordinator-1` — block until the coordinator exits on its own, capture its exit code.
3. `docker compose down --remove-orphans --timeout 5` to clean up the workers (which exit on their own as part of the normal session end) and tear down the network.

Workers finishing before the coordinator no longer triggers any external signal: each worker just transitions to `Exited (0)` and sits there until the final `down`, while the coordinator runs through its entire persistence path and exits normally. `docker wait` returns the coordinator's real exit code, which the driver then uses to classify the run.

This mitigation lives entirely in the benchmark driver and does not touch Relativist itself. A complementary in-binary fix — adding a SIGTERM handler to the coordinator that flushes `output.bin` and `metrics.json` before exiting — would make the binary robust to any orchestrator that abruptly stops containers, and is worth filing as a separate hardening item, but it is not required for the Phase 2 campaign to complete.

---

## 4. Implications

### 4.1 For Phase 3 (real network)

Phase 3 targets distributed execution over a real LAN (multiple physical machines). The L6 limit will surface there in the same form: whichever worker receives the partition holding the highest-ID agent will carry the full-length subnet overhead, and the cap is reached when `~250 MB + k * (total_agents / num_workers)` exceeds 256 MiB. Two responses are available:

- Apply the Phase 2 mitigation (use enough workers that the variable-slope term drops the last-worker frame below the cap — `workers >= 8` worked for all 10M-agent benchmarks here).
- Implement one of the structural fixes (ROADMAP.md items 2.19 and 2.20) before the Phase 3 campaign starts, unblocking low-worker runs on arbitrarily large nets.

The choice will be recorded in the Phase 3 campaign plan, not here.

The Phase 2 evidence also reinforces the expectation that Phase 3 will show worse wall-clock times than Phase 2 for the same configurations, because real network latency between machines is higher than TCP loopback. Any speedup observed in Phase 3 (if any) will therefore require reductions whose computational cost is high enough to dominate both the grid overhead and the network transport time.

### 4.2 For the TCC argument (ARG-001 and ARG-004)

L6 fits naturally into the "when does distribution help?" framing of ARG-001. The TCC argues that distributed IC reduction is correct by confluence (P1-P6) but not automatically beneficial: there are constant-factor overheads (border resolution, serialization, transport) that must be amortized against the reduction work itself. L6 adds a second, more structural kind of overhead: the protocol has an artificial per-message size limit that penalizes the centralized coordinator architecture when a single partition becomes too large to transmit. This is independent of TCP overhead and would persist even on a zero-latency link. Documenting L6 therefore strengthens ARG-004 (overhead analysis) rather than undermining it.

### 4.3 For future work

ROADMAP.md items 2.2, 2.3, and 2.19 together describe a v2 protocol evolution that addresses both the payload cap and the single-shot architecture. 2.2 (dynamic worker joining) and 2.3 (dynamic worker departure) enable persistent worker pools with subset selection per run, eliminating the Docker compose cycle overhead (and the L7 race). 2.19 (payload chunking) unblocks `workers=1` on arbitrarily large nets. Neither is required for the TCC, but both are direct engineering consequences of the confluence property that the TCC validates.

---

## 5. Reproducibility

The Phase 2 campaign is fully scripted by `scripts/bench_docker_resume2.sh`. To reproduce:

```bash
cd codigo/relativist
bash scripts/bench_docker_resume2.sh
```

Requirements:
- Docker Desktop running (`docker ps` must succeed).
- The Docker image built (`docker compose build` is run automatically at the start of the script).
- The `target/release/relativist.exe` binary available for pre- and post-processing (G1 check via `inspect`).
- A `data/` directory at the repository root (the script regenerates any missing input or sequential reference files).

Results are appended to:

- `results/phase2_detail.csv` (per-repetition wall clock, metrics, G1 result)
- `results/phase2_summary.csv` (per-configuration statistics: mean, median, stddev, CV, speedup, efficiency)
- `results/phase2_rounds.csv` (per-round breakdown of partition, compute, merge, and network time)

The Phase 1 results in `results/phase1_*.csv` provide the local-grid comparison for any Phase 2 datapoint at the same (benchmark, size, workers) triple.

The post-fix validation for the four L6-blocked configurations is reproduced by a separate script:

```bash
bash scripts/bench_docker_l6fix.sh
```

Its outputs land in `results/post_fix/phase2_l6_{detail,summary,rounds}.csv` so that the canonical `results/phase2_*.csv` files remain the v0.9.0 baseline.

---

## 6. Fix history — L6 resolution (post-v0.9.0)

This section documents the fix applied after the canonical Phase 2 campaign finished. The four L6-blocked configurations were then re-run under Docker TcpLocalhost to confirm that the fix closes the gap.

### 6.1 The fix has two orthogonal parts

**(a) `CompactSubnet` wire wrapper (ROADMAP item 2.20).** The v0.9.0 subnet encoding serialized the full dense arenas `Net.agents: Vec<Option<Agent>>` and `Net.ports: Vec<PortRef>`, sized to `max_id + 1` and `(max_id + 1) * 3`, regardless of how many slots were live on a given worker. Under `ContiguousIdStrategy` the last worker always pays the full-length cost even when it owns a small slice. `CompactSubnet` (in `src/partition/compact.rs`) carries only live agents inline as `Vec<(AgentId, Agent, [PortRef; 3])>` plus the arena length needed for byte-exact reconstruction on the receiving side. The adapter is hooked via `#[serde(serialize_with / deserialize_with)]` on `Partition::subnet`, so the in-memory `Net` layout is untouched and the wire format changes transparently. Round-trip preserves `agents`, `ports`, `redex_queue`, `next_id`, and `root`; `freeport_redirects` is `#[serde(skip)]` on `Net` and is not carried, consistent with the pre-fix behavior.

**(b) `DEFAULT_MAX_PAYLOAD_SIZE` raised from 256 MiB to 1 GiB.** The 256 MiB cap was a DoS guard, not a resource limit: it has no counterpart in SPEC-01 invariants or in Lafont's confluence argument. For fully-dense nets (`dual_tree` depth 22, `ep_annihilation_con` at 5M) every agent slot is live, so `CompactSubnet` has nothing to strip — the frame legitimately needs 300-420 MB on the wire. The cap raise is the load-bearing part of the fix for these four configurations; `CompactSubnet` alone is insufficient here and the chunking alternative (ROADMAP 2.19) is not needed.

These two changes compose: sparse last-worker subnets now send ~6 bytes per live agent (down from ~9 bytes per dense slot over the whole arena), and fully-dense subnets are transported in a single frame under the new cap. Neither change touches the reduction loop, border resolution, or any protocol state machine.

### 6.2 Validation data

Running `scripts/bench_docker_l6fix.sh` on the four previously-blocked configurations (1 warmup + 3 repetitions each, Docker TcpLocalhost, same host hardware as the canonical campaign) produced:

| Benchmark | Size | Workers | Wall clock mean (s) | Speedup | Frame size | G1 |
|-----------|------|---------|---------------------|---------|------------|----|
| dual_tree | 22 | 1 | 4.299 | 0.591 | 318.8 MB | 100% |
| ep_annihilation_con | 5000000 | 1 | 6.634 | 0.675 | 410.0 MB | 100% |
| ep_annihilation_con | 5000000 | 2 | 12.906 | 0.347 | 410.0 MB | 100% |
| ep_annihilation_con | 5000000 | 4 | 12.465 | 0.359 | 410.0 MB | 100% |

All 12 post-fix repetitions pass the G1 check against the sequential reference `output.bin`. The speedup values are consistent with the Phase 1 local-grid overhead profile: W=1 retains ~60-68% of the sequential baseline (one-worker shortcut dominates serialization cost), and W=2/4 sit near the 0.35 ceiling already documented in PHASE1-FINDINGS.md Section 2.1. The fix does not change the L1 conclusion — no new configuration crosses speedup > 1.0 at W≥2 — but it does complete the parameter-space coverage for ARG-004 (overhead analysis), letting the TCC discuss W=1 through W=8 on the same footing for every targeted benchmark size.

### 6.3 Impact on ARG-004

ARG-004 separates overhead into **structural** (inherent to BSP: split + merge + border resolution per round) and **engineering** (artifacts of layout choices, framing limits, and similar mechanical constants). L6 was the cleanest engineering artifact in the v0.9.0 campaign — a hard cap with no theoretical counterpart plus a dense-arena layout that penalized the coordinator-to-last-worker transfer. Resolving it without touching the BSP algorithm is exactly the demonstration ARG-004 needed: engineering overhead can be reduced to zero (frame padding) or widened until it stops biting (cap), while the structural overhead from L1 persists unchanged across the full 40-config matrix. The post-fix speedup values still sit under 1.0 at W≥2 because partition+clone+merge costs are comparable to `reduce_all` on a Rust queue-based dispatcher, as PHASE1-FINDINGS.md Section 4 already established.

### 6.4 Relationship to ROADMAP items

- **ROADMAP 2.20 (Compact Subnet Encoding)** — implemented as the `CompactSubnet` serde adapter in `src/partition/compact.rs`. The item can be marked completed.
- **ROADMAP 2.19 (Protocol Payload Chunking)** — not implemented. The cap raise and CompactSubnet together cover every configuration in the TCC benchmark matrix. Chunking remains a valid alternative for future workloads where even a 1 GiB frame is insufficient (e.g., nets with >25M fully-live agents distributed to W=1), but the TCC does not need it.
- **ROADMAP 2.2 / 2.3 (dynamic workers)** — orthogonal to L6 and still open; they are about persistent worker pools, not per-frame size.
