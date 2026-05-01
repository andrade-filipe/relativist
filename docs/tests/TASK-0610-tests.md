# TEST-SPEC-0610 — Tests for TASK-0610 — TCP smoke test in CI + hybrid coordinator validation

**Task:** TASK-0610 (Phase E-4 + remainder of E-3, P0)
**Spec:** SPEC-19 R35a (committed `c4c80b8`); SPEC-09 R18a–R18g (committed `82b2d27`); SPEC-22 R10b/R12a (free-list / `next_id` consistency); SPEC-01 G1 (graph-isomorphism invariant).
**Origin:** D-011 plan §E-4 + §E-3 — TCP smoke + hybrid coordinator runtime validation. **This is the test that closes QA-D009-001 on the TCP path.**
**Test floor delta:** **+3 default cargo tests** (Rust-level integration in `relativist-net/tests/`) + **+3 CI smoke tests** (not cargo).
**Prerequisites:** TASK-0596, TASK-0603, TASK-0604, TASK-0608, TASK-0609; SPEC commits `c4c80b8` and `82b2d27` (both already landed).

---

## Test inventory

| test_id | level | target file | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| IT-0610-01 | docker-smoke (CI) | `.github/workflows/docker-bench-smoke.yml::tcp_smoke_completes_under_60s_with_g1_pass` | all listed | CI only |
| IT-0610-02 | docker-smoke (CI) | same file::`csv_row_for_ep_annihilation_1000_workers_2_g1_pass` | all listed | CI only |
| IT-0610-03 | docker-smoke (CI) | same file::`tracing_logs_witness_compactsubnet_free_list_round_trip_non_empty` | TASK-0596 | CI only |
| IT-0610-04 | rust-integration | `relativist-net/tests/integration_tcp_bench_smoke.rs::g1_isomorphism_passes_after_two_worker_tcp_run` | TASK-0596 | none |
| IT-0610-05 | rust-integration | `relativist-net/tests/integration_tcp_bench_smoke.rs::next_id_matches_across_coordinator_and_worker_post_partition_transfer` | TASK-0596 | none |
| IT-0610-06 | rust-integration | `relativist-net/tests/integration_tcp_bench_smoke.rs::hybrid_coordinator_reduces_local_partition_under_tcp` | none | none |

Total: **3 cargo integration tests + 3 CI docker-smoke tests**. Cargo floor delta: **+3 default**.

---

## Per-test specifications

### IT-0610-01 — `tcp_smoke_completes_under_60s_with_g1_pass`

**Purpose.** Headline E-4 acceptance: `docker compose --profile bench-tcp run bench-tcp --benchmark ep_annihilation --sizes 1000 --workers 2 --chunk-size 100 --mode tcp` completes in <60 s with G1 isomorphism passing.
**Setup.**
- The bench-tcp profile from TASK-0609 must be live.
- Image from TASK-0608 must be built.
**Action.** Invoke the docker-compose run; capture wall-clock and exit code.
**Assertions.**
- Exit code == 0.
- Wall-clock <= 90 s (gate at 90 s with margin; reported metric is the actual time, with a target of <60 s per the plan).
- The bench output (stdout or CSV) contains a row with `g1_isomorphism: pass` (or `pass-weak` if `skip_g1` is configured — the test asserts exact-match against whichever the production code emits for a passing G1 check).
- No `panic`, `error[E`, or `Err(` strings in coordinator/worker logs.
**Boundary case coverage.** Catches a TCP path that runs but fails G1 (silent correctness regression).
**Why it must exist.** Acceptance criteria #1 + #2 of TASK-0610. THE smoke test.

---

### IT-0610-02 — `csv_row_for_ep_annihilation_1000_workers_2_g1_pass`

**Purpose.** Acceptance criterion #3: the CSV produced by the smoke run has a row with `benchmark = ep_annihilation`, `size = 1000`, `workers = 2`, `g1_isomorphism = pass`.
**Setup.** Same as IT-0610-01; bind-mount a host directory for CSV output.
**Action.** Read the CSV file from the host post-run; parse via `csv` crate or grep.
**Assertions.**
- The CSV has at least one data row matching: `benchmark == "ep_annihilation"`, `size == 1000`, `workers == 2`.
- That row has `g1_isomorphism == "pass"` (exact string match).
- That row has a non-zero `interaction_count`.
- That row's `mode` column == `"tcp"` (proving TCP path exercised).
**Boundary case coverage.** Catches a smoke that exits 0 but with no CSV row emitted (silent failure).
**Why it must exist.** Acceptance criterion #3 of TASK-0610.

---

### IT-0610-03 — `tracing_logs_witness_compactsubnet_free_list_round_trip_non_empty`

**Purpose.** **The QA-D009-001 fix witness on the TCP path.** Tracing logs from the smoke run must contain a record showing that a `CompactSubnet` with a non-empty `free_list` was sent over the TCP wire AND the receiving worker reconstructed it intact.
**Setup.** Same as IT-0610-01 but with tracing log level set to capture partition-transfer events (`RUST_LOG=relativist_core::partition::compact=debug,relativist_core::protocol=debug` or equivalent).
**Action.** Run the smoke; capture all coordinator and worker logs; grep for the witness lines.
**Assertions.**
- Coordinator logs contain at least one event with `compact_subnet.free_list.len() >= 1` (or equivalent structured field) — proving a non-empty free_list was actually present and serialized at some point during the run.
- Worker logs contain a corresponding deserialization event with the SAME `free_list` length value (round-trip witness).
- No log line says `free_list reconstructed empty after non-empty send` (the bug signature) or any equivalent divergence error.
- The number of agents recycled (per the worker's free_list pop_count) > 0 — proving the recycled-id ledger was actually consumed downstream, not just transmitted.
**Boundary case coverage.** This is the END-TO-END regression for QA-D009-001 on the production TCP path. Without this assertion, the unit-level UT-0596-02 could pass while the TCP path silently regresses (wire format vs. application-layer divergence).
**Why it must exist.** Acceptance criterion #4 of TASK-0610 (SPEC-22 R10b/R12a free_list-consistency check); the dispatch brief explicit instruction ("CompactSubnet free_list round-trip non-empty in tracing logs (witness for QA-D009-001 fix)").

**Implementation note.** This test relies on the `ep_annihilation` benchmark at size 1000 actually producing recycled ids during reduction. Empirically, this should be true (annihilation produces dead agents → tombstones → free_list entries). If it doesn't, the test must use a different benchmark (e.g. `dual_tree`) where free_list usage is guaranteed. Stage 3 developer must verify.

---

### IT-0610-04 — `g1_isomorphism_passes_after_two_worker_tcp_run`

**Purpose.** Rust-level integration: spin up an in-process coordinator + 2 workers via `tokio::spawn` (no docker), run `ep_annihilation` at size 1000 with 2 workers, assert G1 invariant after reduction.
**Setup.**
- Use the existing `relativist-net` test scaffold for in-process TCP coordinator/worker.
- Configure `chunk_size=100`, `mode=tcp`.
- Build a reference dense-local reduction of the same input as the G1 oracle.
**Action.** Run the distributed reduction; capture the final reduced net; compare against the local oracle.
**Assertions.**
- The distributed reduced net is graph-isomorphic to the local-reduced oracle (`nets_isomorphic(&distributed, &local) == true`).
- Total interaction count is identical (or, if the spec allows reduction-order divergence, within the documented tolerance — assert exact equality first; if it fails, document the tolerance).
- Final agent count is identical.
- The test completes in <30 s on the developer's box (CI margin allowed).
**Boundary case coverage.** Catches a TCP-path reduction that produces a structurally different result than local — the most fundamental G1 violation.
**Why it must exist.** Acceptance criteria #2 + #3 of TASK-0610 from the Rust-test side. This is the finer-grained debugging vehicle when the docker smoke fails — runs without docker so it can be debugged in IDE.

---

### IT-0610-05 — `next_id_matches_across_coordinator_and_worker_post_partition_transfer`

**Purpose.** SPEC-22 R10b/R12a regression guard: after a partition transfer, the coordinator and worker agree on the next agent id that `create_agent` would allocate. This is the QA-D009-001 application-layer assertion.
**Setup.**
- In-process coordinator + 1 worker via `tokio::spawn`.
- Configure a partition with `subnet.free_list = vec![AgentId(7), AgentId(3), AgentId(1)]` and `subnet.next_id = AgentId(10)`.
- Coordinator sends the partition.
**Action.** After the worker confirms receipt, both sides compute `next_create_id`:
  - Coordinator: pop top of `free_list` → AgentId(7) (LIFO per SPEC-22 R10c).
  - Worker: same.
- Compare both sides' computed next id.
**Assertions.**
- `coordinator.next_create_id == worker.next_create_id == AgentId(7)`.
- After both sides simulate one `create_agent`, both have `next_create_id == AgentId(3)` (the next on the LIFO stack).
- After exhausting the free_list (3 simulated creates), both sides have `next_create_id == AgentId(10)` (fall-through to `next_id`).
- `coordinator.free_list_len() == worker.free_list_len()` at every step.
**Boundary case coverage.** Catches a TCP-path implementation that transmits the free_list bytes but the worker's deserializer ignores them (drops to `Vec::new()`) — the original QA-D009-001 bug — application-visible as divergent next-id.
**Why it must exist.** Acceptance criterion #4 verbatim ("next_id matches between coordinator and worker post-transfer — this is the regression guard for QA-D009-001"). The strongest application-layer assertion in the entire D-011 bundle.

---

### IT-0610-06 — `hybrid_coordinator_reduces_local_partition_under_tcp`

**Purpose.** E-3 runtime validation: the post-D-006 hybrid coordinator (coordinator now reduces a LOCAL partition, contrasting with v1's pure dispatcher) works under TCP.
**Setup.** In-process coordinator + 2 workers; configure the run such that the coordinator is assigned a non-empty local partition.
**Action.** Run a small reduction; capture per-worker (and coordinator-as-worker) interaction counts.
**Assertions.**
- The coordinator's local partition is non-empty: `coordinator_local_partition.agent_count > 0`.
- The coordinator emits at least one `local_interaction_count > 0` per round (proving it actually reduces, not just dispatches).
- The two remote workers also emit `interaction_count > 0` (the workload is actually distributed, not all-local).
- Final reduced net is G1-isomorphic to a pure-dispatcher v1 baseline (tolerated: agent-id renumbering).
- Coordinator's healthcheck stays `ready` throughout the run.
**Boundary case coverage.** Catches a regression where the post-D-006 hybrid coordinator silently degrades to v1 pure-dispatcher mode (coordinator's local partition is always empty).
**Why it must exist.** Acceptance criterion #5 of TASK-0610 (hybrid coordinator's healthcheck/logs verified sane). This is the runtime test for the E-3 documentation point in TASK-0609.

---

## Coverage matrix

| test_id | AC-1 (CI runs on PR) | AC-2 (<60s) | AC-3 (CSV row + g1 pass) | AC-4 (next_id consistency) | AC-5 (hybrid coord ready) | AC-6 (PR-blocking) |
|---|---|---|---|---|---|---|
| IT-0610-01 | ✅ | ✅ | ✅ | | | ✅ |
| IT-0610-02 | | | ✅ | | | |
| IT-0610-03 | | | | ✅ | | |
| IT-0610-04 | | | ✅ | | | |
| IT-0610-05 | | | | ✅ | | |
| IT-0610-06 | | | | | ✅ | |

Every acceptance criterion has ≥1 test.

---

## Implementation guidance for the developer

- IT-0610-01, 02, 03 → CI smoke (`.github/workflows/docker-bench-smoke.yml`); not cargo tests.
- IT-0610-04, 05, 06 → Rust integration tests (`relativist-net/tests/integration_tcp_bench_smoke.rs`); cargo `#[test]`.
- Cargo floor delta: **+3 default** (only the Rust-level tests).
- The CI workflow trigger filter MUST include: `Dockerfile`, `docker-compose.yml`, `relativist-net/`, `relativist-core/src/protocol/`, `relativist-core/src/partition/compact.rs` (per task §Files in scope).

---

## Out-of-scope tests (deferred to other tasks)

- The Dockerfile build itself → TASK-0608.
- The compose profile definition → TASK-0609.
- The wire format unit tests → TASK-0596.
- Sparse-path tests → TASK-0606 + 0607.
- Other benchmarks beyond `ep_annihilation` for the smoke → out of scope; D-011 plan §E-4 names `ep_annihilation` specifically.

---

## Known spec ambiguity (adversarial flag)

- The "g1_isomorphism: pass" exact column value depends on production CSV emit choices. Some emits use `pass`/`fail`; others may use `true`/`false` or `1`/`0`. IT-0610-01/02 assume `pass` per the dispatch brief language ("G1 isomorphism passing"); Stage 3 must verify against the actual CSV writer.
- "<60 s" is the plan-stated target; the test gate is set at 90 s for runner variability margin. The discrepancy is documented but should be flagged: if the run consistently takes >60 s, that's an early signal that the TCP path has perf debt to investigate (out of scope for this test, but worth tracing).
- The "ep_annihilation produces non-empty free_list" assumption in IT-0610-03 is empirical — if it doesn't hold for the chosen size/workers configuration, the witness test fails for benign reasons. Stage 3 must verify by running once and confirming free_list usage > 0; if not, switch to `dual_tree` for the smoke.
- SPEC-22 R10c specifies LIFO recycle order. IT-0610-05 hard-codes `AgentId(7)` as the first popped value (top of the stack). If a future SPEC amendment changes recycle order to FIFO, the test must be regenerated with the popped value swapped.
