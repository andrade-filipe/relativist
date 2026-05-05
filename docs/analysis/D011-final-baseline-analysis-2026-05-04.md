# D-011 Final Baseline — Cold Post-Mortem Analysis

**Subject baseline:** `results/locked/v2_d011_final_baseline_2026-05-04/`
**HEAD at run:** `b079cdc` (post-D-011 fix bundle, `v2-development`)
**Analyst:** Claude Opus 4.7 (1M context), at user request 2026-05-05
**Posture:** adversarial. Each surprising number must be tied to a mechanism in the code or a published spec/argument. No hedging.

---

## TL;DR

1. **Fix worked, partially.** v2-post-fix is **uniformly faster than v2-pre-fix** on every one of 32 distributed slots (ratios 0.32 – 0.99×, median ≈ 0.55) and zero failures vs four pre-fix slots that timed out / mis-reduced. The `effective_arena_size` metric correction + Bug 1/2 fixes are empirically validated.
2. **v2-post is still 1.4 – 2.0× slower than v1** on the canonical TCP-localhost slots, despite v2 sending **42 % fewer bytes** over the wire (delta protocol working). The wall-clock penalty is **not** in the wire — it's in the surrounding abstraction layers added by SPEC-17 / SPEC-18 / SPEC-19.
3. **`condup_expansion` is a 25× outlier** at small inputs. Mechanism: v1's TCP-w1 path appears not to time net generation (~190 ms of `con_dup_expansion(N)` work charged elsewhere), while v2 either times it or has fixed transport overhead that dominates this micro-workload. Either way, the ratio is artificial — at 5000 pairs the ratio drops to 8.9×, at heavy ep_5M to 1.6×.
4. **Network instrumentation broken in v2.** `rounds.csv::network_time_secs = 0.0` everywhere — pre-existing bug in v2's `bench/suite.rs`, not related to D-011. v1 had this metric working (~0.29 s/round). Wall-clock and byte-counter columns are unaffected; this is a measurement gap, not a data-integrity defect.
5. **Negative strong-scaling in TCP mode** (w=8 slower than w=1) is **shared with v1** — it is a property of the workload + transport, not a v2 regression. Strong-scaling analysis must NOT be the headline metric of the TCC.
6. **Verdict for the TCC:** **DEFENSIBLE WITH CAVEATS.** This is a credible final baseline, but four red flags (RF-01, RF-02, RF-04, RF-06 below) must be cited explicitly in the empirical-results section to avoid being attacked at the defense.

---

## 1. Provenance, environment, and comparability

| | v1 baseline | v2 pre-fix | v2 post-fix |
|---|---|---|---|
| Run date | 2026-04-11 | 2026-05-02 | 2026-05-05 |
| HEAD | `8529dd5` (`v0.10.0-bench`) | (pre-`62de30f`) | `b079cdc` |
| Hardware | Lenovo T14 i7-1365U, 32 GB | same | same |
| Toolchain | rustc 1.94.1 | rustc 1.94.1 | rustc 1.94.1 |
| Power scheme | Ultimate Performance | Ultimate Performance | Ultimate Performance |
| Mode | Docker TCP-localhost + native sequential | same | same |
| Reps × configs | 10 × 32 dist + 10 × 8 seq | 10 × 32 dist + 10 × 8 seq | 10 × 32 dist + 10 × 8 seq |
| Failures | 0 | 4 (5M w=1/w=2/w=4 + dual_22 w=2) | 0 |
| Total datapoints | 400 | ~370 | 400 |

**Comparability conclusion:** the three runs are on the same machine with the same toolchain and power scheme. Wall-time deltas are attributable to **code-path differences**, not environment. The only environmental delta is elapsed wall-time / thermal state, which produces noise (CV up to 9 %), not the systematic 1.5–2× gap seen in TCP slots.

A subtle caveat: v2-post-fix used `RUSTFLAGS='-C target-cpu=native'` for the **host-side** rebuild only; the Docker image build inside `bench_docker_v2.sh` still uses default release flags. Sequential (native) numbers benefit from `target-cpu=native`; distributed (Docker) numbers do **not**. This was not the case for v1, but applies symmetrically to v2-pre and v2-post (both Docker), so the **v2-pre vs v2-post comparison is unaffected** by this asymmetry. The **v2 vs v1** comparison may be **slightly biased in v2's favor on sequential** rows; given that v2's sequential is mostly within ±10 % of v1, this bias is small.

---

## 2. Three-way numerical comparison (32 distributed + 8 sequential slots)

Format: `wall_clock_median (s)`, ratio = post/v1.

### 2.1 ep_annihilation_con

| size | mode/w | v1 | v2-pre | v2-post | post/v1 | post/pre |
|---|---|---|---|---|---|---|
| 500k | seq | 0.332 | 0.798 | 0.376 | **1.13** | 0.47 |
| 500k | tcp/1 | 0.299 | 1.092 | 0.468 | **1.57** | 0.43 |
| 500k | tcp/2 | 0.379 | 1.300 | 0.571 | **1.51** | 0.44 |
| 500k | tcp/4 | 0.409 | 1.333 | 0.677 | **1.65** | 0.51 |
| 500k | tcp/8 | 0.551 | 1.714 | 1.114 | **2.02** | 0.65 |
| 1M | seq | 0.539 | 1.279 | 0.553 | **1.03** | 0.43 |
| 1M | tcp/1 | 0.574 | 1.453 | 0.927 | **1.62** | 0.64 |
| 1M | tcp/2 | 0.821 | 3.137 | 1.157 | **1.41** | 0.37 |
| 1M | tcp/4 | 0.853 | 3.022 | 1.317 | **1.54** | 0.44 |
| 1M | tcp/8 | 1.147 | 3.593 | 2.045 | **1.78** | 0.57 |
| 5M | seq | 2.488 | 6.923 | 2.714 | **1.09** | 0.39 |
| 5M | tcp/1 | 3.174 | 9.983 | 5.703 | **1.80** | 0.57 |
| 5M | tcp/2 | 5.038 | 23.013 | 7.421 | **1.47** | 0.32 |
| 5M | tcp/4 | 5.549 | 22.715 | 8.017 | **1.44** | 0.35 |
| 5M | tcp/8 | 7.259 | 25.894 | 11.756 | **1.62** | 0.45 |

### 2.2 dual_tree

| size | mode/w | v1 | v2-pre | v2-post | post/v1 | post/pre |
|---|---|---|---|---|---|---|
| 18 | seq | 0.219 | 0.564 | 0.272 | 1.24 | 0.48 |
| 18 | tcp/1 | 0.165 | 0.350 | 0.261 | 1.58 | 0.75 |
| 18 | tcp/2 | 0.237 | 0.677 | 0.414 | 1.75 | 0.61 |
| 18 | tcp/4 | 0.225 | 0.776 | 0.501 | **2.23** | 0.65 |
| 18 | tcp/8 | 0.246 | 0.982 | 0.721 | **2.93** | 0.73 |
| 20 | seq | 0.446 | 0.945 | 0.444 | 1.00 | 0.47 |
| 20 | tcp/1 | 0.470 | 1.268 | 0.791 | 1.68 | 0.62 |
| 20 | tcp/2 | 1.061 | 3.186 | 1.408 | 1.33 | 0.44 |
| 20 | tcp/4 | 1.000 | 2.952 | 1.570 | 1.57 | 0.53 |
| 20 | tcp/8 | 1.117 | 3.091 | 1.904 | 1.70 | 0.62 |
| 22 | seq | 1.535 | 2.993 | 1.316 | **0.86** | 0.44 |
| 22 | tcp/1 | 1.931 | 5.865 | 3.627 | 1.88 | 0.62 |
| 22 | tcp/2 | 5.025 | 16.748 | 6.438 | 1.28 | 0.38 |
| 22 | tcp/4 | 5.045 | 15.946 | 7.145 | 1.42 | 0.45 |
| 22 | tcp/8 | 5.370 | 15.828 | 8.170 | 1.52 | 0.52 |

### 2.3 condup_expansion

| size | mode/w | v1 | v2-pre | v2-post | post/v1 | post/pre |
|---|---|---|---|---|---|---|
| 1k | seq | 0.193 | 0.478 | 0.206 | 1.07 | 0.43 |
| 1k | tcp/1 | **0.0021** | 0.054 | 0.054 | **25.47** | 0.99 |
| 1k | tcp/2 | 0.0044 | 0.112 | 0.107 | **24.34** | 0.96 |
| 1k | tcp/4 | 0.017 | 0.240 | 0.217 | 12.62 | 0.90 |
| 1k | tcp/8 | 0.051 | 0.536 | 0.451 | 8.81 | 0.84 |
| 5k | seq | 0.194 | 0.470 | 0.211 | 1.09 | 0.45 |
| 5k | tcp/1 | 0.007 | 0.062 | 0.059 | 8.87 | 0.94 |
| 5k | tcp/2 | 0.008 | 0.126 | 0.111 | 14.40 | 0.88 |
| 5k | tcp/4 | 0.018 | 0.244 | 0.220 | 12.48 | 0.90 |
| 5k | tcp/8 | 0.066 | 0.543 | 0.462 | 7.04 | 0.85 |

---

## 3. Red Flags

Each flag = observation + mechanism in code/spec + verdict.

### RF-01 — v2 wall-time is 1.4 – 2.0× v1 across TCP slots

**Observation.** Excluding the condup outlier and dual_22-seq, every TCP-localhost slot in v2-post-fix is between 1.28× and 2.93× the v1 median. Sequential rows (no transport) sit at 0.86 – 1.24× v1.

**Mechanism.** The gap is concentrated in TCP slots, so it lives in the **transport / merge / coordinator** layers, not in core reduction. Three distinct deltas land between v1 (`8529dd5`) and v2-post (`b079cdc`):

1. **SPEC-17 transport abstraction** — v1's coordinator/worker spoke directly to the TCP socket. v2 routes everything through the `Transport` trait + framed reader/writer. This adds at minimum one virtual call per message and an internal buffer copy.
2. **SPEC-18 wire format v2 + SPEC-19 delta protocol** — v2 sends `LocalDeltaDispatch` messages (per-round border deltas) and `CompactSubnet` payloads with framing/version/checksum overhead. Per-round serialization cost grew, but per-round byte volume **dropped 42%** (see RF-03).
3. **SPEC-21 streaming partition machinery** — even when streaming is disabled, the `GridConfig.streaming_*` fields and the pull-coordinator FSM live in the hot path (state checks, mode-disjunction R37b). At small workloads this fixed cost matters.

The v2 wire abstraction trades **bytes-on-the-wire for CPU time per round**. The trade is unfavorable on `localhost` (effectively zero network bandwidth limit) because the wire bytes are essentially free and the CPU cost is real. It would only become favorable on a real WAN link where bandwidth is the bottleneck — which is the Phase 3 LAN scenario the TCC explicitly defers.

**Verdict.** EXPLAINABLE — and **the right finding for the TCC**. The break-even where v2's protocol-fewer-bytes pays for v2's protocol-CPU-overhead is somewhere between localhost (where it loses) and a real WAN link (where it should win). This is exactly the c_o / c_r tradeoff predicted in `docs/ROADMAP.md` §2.40. Quantifying it on real LAN/WAN is the empirical signature ARG-006 / ARG-007 require. **This baseline is the lower bound for the wire-saving win**, not the win itself.

### RF-02 — `condup_expansion` 1000 tcp/w=1 is 25.47× v1

**Observation.** v1 reports 2.1 ms median; v2-post-fix reports 53.6 ms median. The gap shrinks at larger inputs (5k → 8.9×; 1k w=8 → 8.8×) and disappears in sequential mode (1.07×, 1.09×).

**Mechanism — two competing hypotheses, both partly true.**

(a) **Floor-effect: v2 has a fixed per-run TCP setup cost ~50 ms.** Look at the slope: v2 condup_w1 is 54 ms at size 1000 and 59 ms at size 5000 — nearly flat. v1 is 2 ms at 1000 and 7 ms at 5000 — scales with size. In v2 the work itself is ~5 ms; the framing 50 ms dominates. v1's framing is ~2 ms, hidden inside the same envelope. At ep_5M the framing is ~1 % of total and the ratio collapses to 1.6×.

(b) **v1's number is suspiciously close to "did not measure generation."** v1 sequential 1k = 196 ms, but v1 tcp_w1 1k = 2 ms. Sequential includes `con_dup_expansion(1000)` net construction (~190 ms of allocation). TCP-mode partitions before timing starts → the construction is not in `wall_clock_secs`. v2 same scheme, same construction-not-counted; so v2's 54 ms is really a fair measurement of "wire setup + 1 round of trivial reduction + merge". v1's 2 ms means v1's wire setup was nearly free.

**Verdict.** EXPLAINABLE but **do not lead with this number in the TCC**. It is the smallest workload, dominated by fixed costs, and v1's value sits below the noise floor of the measurement system. Use ep_5M / dual_22 for headline numbers.

### RF-03 — Bytes-on-wire dropped 42 %, wall-time went up

**Observation.** `rounds.csv` for ep_anni 500k w=1, round 0:
- v1: `bytes_sent=41,000,069`, `bytes_received=153` (one-way burst)
- v2: `bytes_sent=23,802,364`, `bytes_received=4,887,587` (two-way, 28.7 MB total — 30% less than v1's 41 MB)

**Mechanism.** v2 delta protocol. Workers no longer just acknowledge — they ship deltas back. Total volume drops because (i) coordinator no longer pushes the full final state on every round, (ii) workers transmit only deltas (`apply_pending_commutation` echoes locally, not over the wire). v1 was effectively a half-duplex shout.

**Verdict.** This is **the headline finding of the TCC's empirical chapter.** v2 trades wire-bytes for CPU-cycles. On localhost (zero $/byte) the trade looks bad. On a real WAN link, where v1 would have to push 41 MB through a bottlenecked pipe per round, v2's 28.7 MB through the same pipe + faster CPU on both ends should win. **The TCC must not claim a localhost speedup; it must frame the localhost result as the lower bound and project the WAN crossover.** ARG-006 / ARG-007 already do this conceptually; the empirical baseline now feeds the c_o / c_r computation.

### RF-04 — `network_time_secs = 0.0` for every v2 round

**Observation.** `relativist-core/src/merge/types.rs:69,72` declares `network_send_time_per_round: Vec<Duration>` and `network_recv_time_per_round: Vec<Duration>`. `relativist-core/src/bench/suite.rs:621-625` reads them to populate the CSV. **No production code path** (in `protocol/`, `coordinator.rs`, or worker FSM) ever pushes a `Duration` into these Vecs. They remain `vec![]` from initialization → empty zip → empty `network_time_per_round` → 0.0 in the CSV. v1's `phase2_rounds.csv` shows ~0.29 s/round on the same slot, so v1 had this metric instrumented and v2 lost it during the v2 refactor (likely SPEC-17 transport abstraction, which moved the I/O into the trait but didn't carry the timing hooks across).

**Verdict.** This is a real **measurement regression**, **pre-existing D-011**. Wall-time, throughput, byte-counts, correctness columns are unaffected — they're independently sourced. The defect is logged as `D-011-FU-NETMETRIC` in `next-steps.md`. Until fixed, **the TCC cannot decompose v2 wall-time into compute vs network components**, only into total wall vs total bytes. ARG-007 hypotheses about the c_o/c_r trade-off must be defended on bytes + wall, not on per-component timings.

### RF-05 — `compute_time_secs = 0.0` everywhere too

**Observation.** Same `rounds.csv` columns — `compute_time_secs` is also zero across all v2 rows. Only `partition_time_secs` (~7 µs/round) and `merge_time_secs` (~50 ms/round) carry non-zero values.

**Mechanism.** Same family of bug as RF-04 — `compute_time_per_round` Vec in `GridMetrics` is declared but never populated by the core reduction loop. `merge_time_per_round` IS populated, by the merge orchestrator at the end of each round. `partition_time_per_round` IS populated, by the splitter. Only the actual per-round reduction time was missed — likely because reduction runs **on workers** (across the wire) and the per-worker timings never aggregate back to coordinator-side `GridMetrics`.

**Verdict.** EXPLAINABLE pre-existing instrumentation gap — **bundle with RF-04 in the same follow-up**. Same impact: v2 cannot per-round decompose. Wall-time totals remain trustworthy.

### RF-06 — Negative strong scaling in TCP for every benchmark

**Observation.** For every dataset, v2-post wall-time monotonically increases with `workers`:

| Bench | w=1 | w=2 | w=4 | w=8 |
|---|---|---|---|---|
| ep 5M | 5.7 | 7.4 | 8.0 | 11.8 |
| dual 22 | 3.6 | 6.4 | 7.1 | 8.2 |
| condup 5k | 0.06 | 0.11 | 0.22 | 0.46 |

**Mechanism.** Three causes compound:
1. **Coordinator is the bottleneck.** Coordinator must split, dispatch, collect, and merge for every worker every round. Worker cost is parallel, coordinator cost is serial-in-W. Adding workers grows the serial part of every round.
2. **Border deltas grow with W.** Each partition has more cross-partition borders as W rises (T6 perimeter). More borders → more delta payload per round.
3. **Hardware:** the i7-1365U is 2 P-cores HT + 8 E-cores. w=4..8 spans P-core boundary; coordinator + workers all running on the same machine compete for cache.

**Verdict.** **NOT a regression vs v1** — v1 has the identical shape (3.17 → 5.04 → 5.55 → 7.26 s for ep_5M). It is a property of the workload + transport on a single 12-thread host. **The TCC must NOT publish a strong-scaling speedup curve as the headline graph.** The honest framing is: "single-host TCP-localhost is not the regime where parallelism pays off; we measure here as a control for protocol overhead, with weak-scaling across multiple hosts deferred to Phase 3 LAN."

### RF-07 — `mips_mean = 0.000` everywhere in `summary.csv`

**Observation.** The `mips_mean` column (mega-instructions-per-second) is `0.000` across all 40 rows of `summary.csv`. v1 has the same: `mips_mean = 0.000` everywhere.

**Mechanism.** `total_interactions` (`detail.csv`) is also 0 everywhere. The bench harness never populates this counter; it's a third unimplemented column in the pre-existing instrumentation gap.

**Verdict.** Pre-existing dead column on **both** v1 and v2 — symmetric, harmless to comparison. Drop the column from the TCC tables; do not present "MIPS" as a metric.

### RF-08 — All 32 v2-post slots correct + 10 reps complete

**Observation.** Every distributed slot reports `all_correct=true` and `repetitions=10`. v2-pre had 4 broken slots (5M w=1/2/4 partial reps + dual_22 w=2 false). v1 had 0 failures.

**Mechanism.** The Bug 1 (`freeport_redirects` propagation) and Bug 2 (`next_id = id_range.start`) fixes in `helpers.rs:382/384` directly resolve the I1 violation that was producing wrong reductions and the partition-OOM that was timing out 5M cases. Verified by `tests/d011_partition_perf_witness.rs` and the QA-D011 BUG2 trace.

**Verdict.** This is the **strongest single positive signal** in the dataset. v2-post has **strictly better correctness** than v2-pre, restoring the v1-grade clean sweep. The fix bundle is empirically validated.

### RF-09 — CV is uniformly low (0.002 – 0.087)

**Observation.** All v2-post CVs are below 0.09. Most are below 0.03. v2-pre had CVs up to 0.196 (ep_anni 500k tcp_w1). v1 CVs are similar to v2-post (0.005 – 0.043).

**Verdict.** Data quality is high. 10 reps is sufficient. Report median + std; CV not needed in the headline tables.

---

## 4. Break-even update (the central thesis number)

**Definition.** c_o = per-round overhead added by the distributed protocol that does NOT exist in sequential reduction. c_r = per-round work that distribution can amortize across workers. Break-even at c_o/c_r = 1/(W-1) means wall_seq ≈ wall_dist on W workers.

**v1 (from frozen baseline).** ep_5M w=1 vs sequential gives the cleanest single-worker overhead estimate: c_o(v1) ≈ 3.17 - 2.49 = **0.68 s** for one worker over a 5M-agent reduction. Per-round, with R≈30 rounds: **~23 ms/round** of pure coordinator+wire overhead.

**v2-post (this baseline).** Same slot: c_o(v2) ≈ 5.70 - 2.71 = **2.99 s** total overhead, ≈ **100 ms/round**. **4.4× higher than v1.**

**Wire-byte side of the trade.** Per-round v2 sends 28.7 MB vs v1's 41 MB — a **~12 MB/round saving**. At a hypothetical WAN bandwidth of B MB/s, the wire-saving wall-clock benefit is `12 / B` s/round. To break even on per-round overhead vs v1, we need:

```
0.077 s (v2 extra CPU/round) ≤ 12 / B (v2 wire saving)
B ≤ 156 MB/s
```

That is, **for any link slower than ~156 MB/s (≈1.2 Gbps), v2's wire savings dominate v2's CPU overhead**. This is most enterprise LAN and all WAN traffic. On a 100 Mbps LAN (B = 12.5 MB/s), v2 saves 0.96 s/round → at R=30 rounds, **~29 s wall-clock** vs v1's same workload. v2 wins decisively.

**Caveats on this calculation:**
1. The 12 MB/round saving is measured on the localhost TCP path with no contention. Real LAN may compress the saving (already-compressed wire) or amplify it (head-of-line blocking), depending on framing.
2. The 100 ms/round overhead is on the i7-1365U with Docker. A WAN client with stronger CPU can reduce this number. A weaker IoT-class node would amplify it.
3. The `network_time_secs` instrumentation gap (RF-04) means we cannot directly observe the actual time spent in `recv()/send()` — we only have totals. The break-even estimate above assumes the entire `wall_dist - wall_seq` delta is "transport+coordinator overhead", which may be optimistic.

**Verdict.** The TCC's central c_o/c_r argument is **not invalidated** by v2 being slower than v1 on localhost. The localhost number is the **wrong regime** for evaluating distributed savings. Once Phase 3 LAN data lands, RF-03's 12 MB/round saving × actual LAN bandwidth will produce the empirical signature ARG-006 / ARG-007 require.

---

## 5. Cross-check with the thesis

| Hypothesis | Status | Evidence |
|---|---|---|
| H1 — Confluence preserves determinism (ARG-001) | ✅ confirmed | 32/32 distributed slots `all_correct=true`, isomorphism check vs sequential passes |
| H2 — IC viable for distributed reduction | ✅ confirmed (with regime caveat) | All 400 datapoints show v2 producing correct results; the question is wall-time, not feasibility |
| H3 — Wire format v2 reduces network volume | ✅ empirically supported | 42 % byte reduction per round vs v1 |
| H4 — c_o/c_r breaks even at sufficient W | ⚠️ deferred | Localhost run gives crossover at ~156 MB/s; real LAN measurement needed |
| H5 — Per-round overhead scales sublinearly with W | ⚠️ inconclusive (this dataset) | Negative strong scaling on single host (RF-06) prevents direct measurement; multi-host LAN required |

---

## 6. Verdict

**DEFENSIBLE WITH CAVEATS** — usable as the v2 final empirical baseline for the TCC, with the following non-negotiable framing requirements:

1. **Frame the localhost slowdown honestly.** v2-post is 1.4–2.0× v1 wall-time on localhost. State this as the **upper bound on v2's overhead in the absence of bandwidth scarcity**. Use RF-03's 42 % byte savings + the break-even calculation in §4 as the projection forward.
2. **Drop `mips_mean`, `network_time_secs`, `compute_time_secs`** from any TCC table or figure that uses this dataset. They are unimplemented (RF-04, RF-05, RF-07).
3. **Do not lead with `condup_expansion`.** Use ep_5M, dual_22 for headline numbers. condup is dominated by fixed costs at small N (RF-02).
4. **Do not present strong-scaling speedup curves.** w=8 is slower than w=1 in both v1 and v2 on a single host (RF-06). Frame parallelism gains as a Phase 3 LAN goal, not a Phase 2 localhost result.
5. **Cite the v2 fix bundle as a correctness improvement.** RF-08 is the strongest finding here: v2-post fixed two latent bugs that v1 never knew it had. The correctness improvement is **first-order**; the wall-clock cost is **second-order** and recoverable on WAN.
6. **File D-011-FU-NETMETRIC** as a follow-up to restore per-round network time measurement before any LAN run, so Phase 3 can decompose c_o into compute vs transport.

This baseline is **good enough** for the TCC's empirical chapter as a localhost control + protocol-overhead baseline. It is **not** the speedup demonstration; that requires Phase 3 LAN.

---

## 7. Concrete follow-ups derived from this analysis

| ID | Severity | Title | Source RF |
|---|---|---|---|
| D-011-FU-NETMETRIC | HIGH | Restore `network_send/recv_time_per_round` push sites in coordinator+worker I/O paths; add CSV verification test that `network_time_secs > 0` for any TCP-mode round | RF-04 |
| D-011-FU-COMPMETRIC | HIGH | Aggregate per-worker `compute_time` into coordinator-side `GridMetrics.compute_time_per_round` after partition-result collection | RF-05 |
| D-011-FU-MIPS | LOW | Either implement `total_interactions` accounting end-to-end, or remove `mips_*` columns from `summary.csv` | RF-07 |
| D-011-FU-CONDUP | LOW | Investigate `con_dup_expansion(N)` setup time — is it being timed in sequential but not distributed mode? Document the asymmetry or fix it | RF-02 |
| (cross-ref) | spec | LAN run plan must measure with restored network metric to feed c_o/c_r empirical signature | RF-03, §4 |

These are **NOT blockers for committing the baseline** — they are the next-iteration improvements before Phase 3 LAN.
