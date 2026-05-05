# TEST-SPEC-0615 — Tests for TASK-0615 — D-011-FU-NETMETRIC: restore per-round network time instrumentation

**Task:** TASK-0615 (D-012 Instrumentation Restore — Stage 3 DEV scope).
**Spec:** none (instrumentation-only; no spec change). Production-side fields already declared at `relativist-core/src/merge/types.rs:69,72`. Bench harness consumer already wired at `relativist-core/src/bench/suite.rs:621-625`. CSV writer already wired at `relativist-core/src/bench/csv.rs:159`.
**Closes red flag:** RF-04 (`docs/analysis/D011-final-baseline-analysis-2026-05-04.md` §3 RF-04, lines 142–146) — `network_time_secs = 0.0` everywhere on v2 due to a pre-existing producer-side plumbing gap.
**Origin:** `docs/handoffs/2026-05-05-D012-instrumentation-restore-handoff.md` §3 D-011-FU-NETMETRIC + `docs/backlog/TASK-0615-d011-fu-netmetric-restore-network-time.md` Acceptance criteria.
**Test floor delta:** **+1 default** (one new integration-test binary `relativist-core/tests/d012_network_time_witness.rs`, holding 1–2 `#[tokio::test]`s). Zero-copy and streaming-no-recycle floors unchanged.
**Prerequisites:**
- TASK-0617 (release-tests) is **not** a hard prerequisite, but landing it first lets this test run cleanly under `cargo test --release`. Recommended sequencing: 0617 → 0615.
- No spec prerequisite (instrumentation-only; no R-rule pinned).
- No code prerequisite for compilation: the test references existing public APIs (`GridConfig`, in-process `run_grid` or distributed coordinator/worker harness, `GridMetrics`).

---

## Test inventory

| test_id | level | target file::test_name | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| IT-0615-01 | integration | `relativist-core/tests/d012_network_time_witness.rs::tcp_round_records_nonzero_network_time` | none | none |
| IT-0615-02 | integration | `relativist-core/tests/d012_network_time_witness.rs::tcp_round_records_send_and_recv_separately` | none | none |
| IT-0615-03 | integration | `relativist-core/tests/d012_network_time_witness.rs::in_process_round_keeps_network_time_zero` | none | none |
| IT-0615-04 | integration | `relativist-core/tests/d012_network_time_witness.rs::heartbeat_only_round_records_measurable_send_recv` | none | none |

**Totals:** 0 UT, 4 IT, 0 PT. Net floor delta: **+1 default** (single new integration binary; the four `#[test]` functions live in it).

---

## Per-test specifications

### IT-0615-01 — `tcp_round_records_nonzero_network_time`

**Purpose.** Headline acceptance witness for RF-04 closure. After a 1-round TCP-mode benchmark (or equivalent in-test coordinator/worker dyad) completes, `metrics.network_send_time_per_round[0] > Duration::ZERO` AND `metrics.network_recv_time_per_round[0] > Duration::ZERO`. Pre-fix HEAD: FAILS (Vec is empty → indexing panics OR length-check fails). Post-fix HEAD: PASSES.

**Setup.**
- Bring up an in-test coordinator + 1 worker over a localhost TCP listener (use the existing distributed-test harness pattern from `relativist-core/tests/` — e.g., the harness used by `tests/d011_partition_perf_witness.rs` for `local` mode if it covers TCP, OR a fresh `tokio::net::TcpListener::bind("127.0.0.1:0")` ephemeral-port pair).
- Workload: a small but non-trivial net (e.g., `dual_tree(depth = 6)` or `ep_annihilation(64)`) — enough to force at least one non-zero-byte round-trip but not so much that the test is slow. Target wall-time < 5 s.
- `GridConfig` with `workers = 1`, `transport = TCP` (or whatever enum variant selects TCP). `sparse_build = false` is fine; not load-bearing.

**Action.**
1. Run `run_grid` (or distributed equivalent) for 1 BSP round.
2. Capture the returned `GridMetrics`.

**Assertions.**
1. `metrics.network_send_time_per_round.len() >= 1` — the producer pushed at least one entry.
2. `metrics.network_recv_time_per_round.len() >= 1` — same on the recv side.
3. `metrics.network_send_time_per_round[0] > Duration::ZERO`.
4. `metrics.network_recv_time_per_round[0] > Duration::ZERO`.

**Failure message contract.** On (3)/(4) failure, the panic message MUST cite "RF-04" and `docs/analysis/D011-final-baseline-analysis-2026-05-04.md §3 RF-04` so future debuggers find the historical context. Example:
```
network_send_time_per_round[0] = 0 — RF-04 regression. The producer-side push site in protocol/coordinator.rs (or worker.rs) is missing. See docs/analysis/D011-final-baseline-analysis-2026-05-04.md §3 RF-04 and docs/handoffs/2026-05-05-D012-instrumentation-restore-handoff.md §3 D-011-FU-NETMETRIC.
```

**Boundary case coverage.** The "1-round" workload is the smallest non-trivial case. A 0-round bench (degenerate: net with 0 redexes) would short-circuit before any TCP traffic and is covered separately by IT-0615-03 conceptually (in-process == zero) and IT-0615-04 (heartbeat-only round).

**Why it must exist.** Direct closure of RF-04 acceptance criterion 1 in TASK-0615.md: "every row of `rounds.csv` from a non-trivial round (≥ 1 redex reduced) has `network_time_secs > 0.0`."

---

### IT-0615-02 — `tcp_round_records_send_and_recv_separately`

**Purpose.** Defends the **separation** of send vs recv timing. The handoff §3 explicitly recommends two distinct accumulators (`network_send_time_per_round` and `network_recv_time_per_round`). Asserts both are recorded independently, neither is silently merged into the other, and the values are not literally equal (which would suggest one was copied from the other).

**Setup.** Identical to IT-0615-01 — same coordinator/worker dyad, same workload.

**Action.** Same as IT-0615-01.

**Assertions.**
1. `metrics.network_send_time_per_round[0] > Duration::ZERO`.
2. `metrics.network_recv_time_per_round[0] > Duration::ZERO`.
3. `metrics.network_send_time_per_round[0] != metrics.network_recv_time_per_round[0]` — at sub-microsecond timer resolution, exact equality is statistically impossible if both are independently measured. If the implementation accidentally copies the same `Instant` delta into both, this assertion fires.
   - **Tolerance note:** if the developer's instrumentation legitimately produces equal values on a deterministic mock transport (e.g., a future LoopbackTransport), document it in the commit and weaken the assertion to `>= Duration::from_nanos(1)` for both. The default expectation is real-clock TCP localhost, where ns-level jitter guarantees inequality.
4. The order of magnitude of both values is "reasonable" — at least 1 µs (`>= Duration::from_micros(1)`) and at most 10 s (`<= Duration::from_secs(10)`). This guards against unit-confusion bugs (e.g., recording nanoseconds into a `Duration::from_secs(...)` field).

**Boundary case coverage.** Equal-values mode (assertion 3) is the boundary that catches a "copy-paste" bug where a developer wires the same accumulator into both fields.

**Why it must exist.** Handoff §3 explicitly mandates "send and recv timings are recorded separately"; without this assertion a single accumulator pushed twice would silently pass IT-0615-01.

---

### IT-0615-03 — `in_process_round_keeps_network_time_zero`

**Purpose.** Negative control: in-process (non-TCP) rounds MUST continue to report zero network time. This metric is TCP-mode-only by definition (no wire → no network time). If a developer wires the timing instrumentation too aggressively (e.g., into `merge/grid.rs`'s in-process loop), this test fires.

**Setup.**
- In-process `run_grid` (no TCP listener; `transport = InProcess` or equivalent).
- Same workload as IT-0615-01.

**Action.** Run 1 BSP round in-process. Capture `GridMetrics`.

**Assertions.**
1. EITHER `metrics.network_send_time_per_round.is_empty()` OR all entries equal `Duration::ZERO`. Both are acceptable: the first matches "Vec was never populated for this code path" (spec-compliant); the second matches "Vec was populated with zeroes" (also spec-compliant, slightly less clean).
2. Same for `metrics.network_recv_time_per_round`.

**Boundary case coverage.** This is the in-process boundary — the metric exists in `GridMetrics` but is semantically zero/empty.

**Why it must exist.** TASK-0615 acceptance criterion 2: "Pre-existing in-process bench rows (no TCP) remain `0.0` for `network_time_secs` (this metric is TCP-mode-only by definition)." Without this test, an over-eager instrumentation patch could populate the in-process path with non-zero values and silently pass IT-0615-01/02.

---

### IT-0615-04 — `heartbeat_only_round_records_measurable_send_recv`

**Purpose.** Edge-case witness for **zero-byte content rounds**. The constraint statement on this task explicitly asks: "one edge-case for zero-byte rounds (e.g., heartbeat-only or final-ack-only round) — do they record measurable time, and what does the spec say should happen? Document the answer."

**Spec answer (documented here, since no formal spec text covers it).** A round in which the only wire traffic is protocol-level framing (heartbeat, final-ack, RequestWork with no data, or empty `PartitionResult`) is **still a round with measurable `Instant::now()` deltas around the recv/send `await` points**. The instrumentation MUST therefore record a small but non-zero duration. Rationale: `network_time_secs` is wall-time spent on the wire, not bytes transferred. Per-round bytes-sent/received columns (already populated) cover the byte side; this column covers the wall-time side. A heartbeat-only round still costs syscall + scheduler latency.

**Setup.**
- Coordinator + 1 worker over TCP localhost.
- Workload: a net with 0 redexes (e.g., a single isolated CON agent with all three ports wired to FreePorts). The first round will run, find no work, and emit a heartbeat / NoMoreWork / final-ack pattern.
- Configure for **exactly 1 round** (the bench harness should detect "no redexes" and emit the no-work termination after one round-trip).

**Action.** Run 1 round. Capture `GridMetrics`.

**Assertions.**
1. `metrics.network_send_time_per_round.len() >= 1` AND `metrics.network_recv_time_per_round.len() >= 1` — even for a no-redex round, the protocol exchange happened.
2. `metrics.network_send_time_per_round[0] >= Duration::from_nanos(1)` — non-zero (some traffic, even if just heartbeat).
3. `metrics.network_recv_time_per_round[0] >= Duration::from_nanos(1)`.
4. **Documentation assertion (in test body comment, not runtime):** the test file MUST contain a comment block explaining the "zero-byte rounds still record measurable time" decision and citing this TEST-SPEC's IT-0615-04 entry. Future readers grepping the codebase for "heartbeat" or "zero-byte round" find the rationale immediately.

**Boundary case coverage.** This is the "0-redex round" boundary — the smallest workload that still triggers a wire round-trip. Distinct from IT-0615-01's "1-redex round" boundary.

**Why it must exist.** Spec answer is now nailed down in code, not in prose. If a future developer adds a "skip metric when round had 0 bytes payload" optimization, this test fires.

---

## Notes

### Determinism strategy

All four IT tests run on `tokio::time::Instant` (or `std::time::Instant`) which is a real-clock measurement. Per-test wall-clock is bounded < 5 s; CI flake risk is negligible. The `>= Duration::from_nanos(1)` assertions are the only real-clock-dependent ones, and ns-level granularity is below any plausible scheduler floor on Linux/Windows runners — they'll always be satisfied as long as the producer-side push site is wired.

### What this test does NOT cover

- **Per-round CSV row generation** — the CSV writer (`bench/csv.rs:159`) is already wired and tested elsewhere; this TEST-SPEC pins the producer side, not the writer side.
- **Wall-clock proportionality to network bytes.** A future test could correlate `network_time_secs` with `bytes_sent + bytes_received`; that's a separate enhancement (TASK-0615 acceptance does not require it).
- **Worker-side timing.** TASK-0615 explicitly scopes acceptance to coordinator-side rows. If the developer instruments worker-side too, that's optional and OUT of this TEST-SPEC's scope (see TASK-0616 for compute-side worker→coordinator aggregation).
- **MIPS / total_interactions.** TASK-0618 territory.
- **Compute time.** TASK-0616 territory.

### Coverage of constraints from the operator prompt

| Constraint | Where |
|---|---|
| At least one assertion `network_time_secs > 0` for any TCP-mode bench round | IT-0615-01 assertions (3)+(4) |
| Send and recv recorded separately, both > 0 | IT-0615-02 assertions (1)+(2)+(3) |
| Edge-case for zero-byte rounds (heartbeat/final-ack-only) — document spec answer | IT-0615-04 (full test, plus mandatory documentation comment) |

### Cfg gates

None. Tests run on the default profile (`cargo test`). Recommend re-running under `cargo test --release` post-TASK-0617 to verify release-mode timing is also non-zero (release builds may inline harder; IT-0615-04's ns-level threshold is the most exposed).

---

## Cross-references

- **Source task:** `docs/backlog/TASK-0615-d011-fu-netmetric-restore-network-time.md`.
- **Bundle handoff:** `docs/handoffs/2026-05-05-D012-instrumentation-restore-handoff.md` §2 row 1, §3 D-011-FU-NETMETRIC subsection.
- **Red flag:** `docs/analysis/D011-final-baseline-analysis-2026-05-04.md` §3 RF-04 (lines 142–146).
- **MANIFEST citation:** `results/locked/v2_d011_final_baseline_2026-05-04/MANIFEST.md` "Known instrumentation defect" section.
- **Companion TEST-SPEC:** TEST-SPEC-0616 (compute-time companion; same test-binary location but separate concerns).

---

## Coverage matrix

| test_id | RF-04 (network_time_secs > 0) | Send/recv separation | TCP-only (in-process zero) | Zero-byte round edge case |
|---|---|---|---|---|
| IT-0615-01 | ✅ | partial (both > 0 but not separated) | — | — |
| IT-0615-02 | ✅ | ✅ (load-bearing) | — | — |
| IT-0615-03 | — | — | ✅ (load-bearing) | — |
| IT-0615-04 | ✅ (zero-byte case) | partial | — | ✅ (load-bearing) |

---

## Out-of-scope (explicitly NOT specified here)

- Worker-side compute-time aggregation — TEST-SPEC-0616.
- Release-mode test compilation — TEST-SPEC-0617.
- MIPS / total_interactions — TEST-SPEC-0618.
- TCC artigo edits (REDATOR territory).
- Frozen baselines under `results/locked/`.
- Any wire-format change.
