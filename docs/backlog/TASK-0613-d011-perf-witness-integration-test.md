# TASK-0613 — D-011 partition perf regression witness (TDD-RED integration test)

**Phase:** D-011 BLOCKER 2026-05-04 — Stage 2 (TDD-RED witness, ships BEFORE the fix)
**Bundle:** D-011 — Tier 3 Hardening + Bench Enablement
**Status:** OPEN
**Priority:** P0 (the empirical proof that the bug exists; flips to PASS after TASK-0612 lands)
**Spec:** SPEC-22 v2.4 §3.4 R22 (anchor TASK-0611). The witness operationalizes the R22 invariant by asserting that healthy partition workloads exercise the DENSE branch.
**Origin:** `docs/plans/2026-05-04-d011-partition-perf-fix.md` §5 Task 2.
**Estimated complexity:** S (~80 LoC test file — one helper `build_dense_packed_net` + one `#[test]` function with two assertions).

---

## Context

This task is the **bug witness** for the D-011 partition performance regression. It is committed BEFORE TASK-0612 (the fix) so the bug is captured on disk. On pre-fix HEAD the test FAILS with the message `partition X took SPARSE branch: arena_len = N == id_range_size = N (live = M)`. After TASK-0612 lands, the test PASSES — this transition is the canonical proof that the fix actually closes the bug class.

The test exercises a healthy workload: 1000 live CON agents densely packed at IDs 0..999, principal ports wired to FreePorts (T1-compliant), partitioned across 2 workers via `FennelStrategy`. Under the OLD R22 metric (`id_range_size > 4 × live_count`), `compute_id_ranges(2, 1000)` yields per-worker chunks of 1000 × 10 = 10_000 IDs, and `id_range_size (10_000) > 4 × live_count (500 × 4 = 2000)` trips the threshold → SPARSE branch (BUG). Under the NEW R22 metric (`effective_arena_size > 4 × live_count`), `effective_arena_size (≈ 500–999) ≤ 4 × live_count (2000)` does not trip → DENSE branch (CORRECT).

The two assertions:

1. **Loose:** `arena_len < id_range_size` for every non-empty partition. SPARSE-via-`to_dense(id_range)` produces `arena_len = id_range_size`; DENSE produces `arena_len = max_live_id + 1` which is strictly less. This is the discriminator.
2. **Tight:** `arena_len == max_live_id + 1` (recomputed by scanning the partition's slot occupancy). DENSE branch invariant.

Verbatim test code (helper + body) is in `docs/plans/2026-05-04-d011-partition-perf-fix.md` §5 Task 2 Step 1 (lines 169–247). It uses the public APIs `Net::new`, `Net::create_agent`, `partition::split_with_config`, `FennelStrategy::new`, `PartitionConfig::default`, `PortRef::FreePort`, and the constant `PORTS_PER_SLOT`.

## Dependencies

- **None** strictly required for compilation. The test compiles against current HEAD because all symbols it references already exist.
- **Sequencing:** This task ships BEFORE TASK-0612 in commit order (TDD-RED). If TASK-0612 ships first, this task degrades from "regression witness with empirical proof on disk" to "passing test that documents the invariant" — still useful, but loses the bisect-replicable proof.
- **Cross-link:** TASK-0611 (spec anchor for what the test is asserting), TASK-0612 (fix that flips this test from FAIL to PASS).

## Files in scope

| File | Change |
|------|--------|
| `relativist-core/tests/d011_partition_perf_witness.rs` | **CREATE.** New integration test file (~80 LoC). Helper `build_dense_packed_net(n: u32) -> Net` + `#[test] fn d011_witness_partition_dense_branch_for_healthy_workload()`. Verbatim body in plan §5 Task 2 Step 1. |

## Files explicitly OUT of scope

- Production code in `relativist-core/src/` — TASK-0612.
- Other inline test rewrites in `helpers.rs` — TASK-0612.
- The bench-level performance verification (`ep_con 5M w=2 local` 12 s vs 22 s) — TASK-0614. This task is a *correctness-level* witness (which branch is taken), not a wall-time gate.

## Acceptance criteria

1. File `relativist-core/tests/d011_partition_perf_witness.rs` exists and compiles on HEAD without TASK-0612.
2. **On pre-fix HEAD** (i.e., the commit immediately preceding TASK-0612): `cargo test --release --test d011_partition_perf_witness` FAILS with the panic message containing `took SPARSE branch`. This failure is **expected** and is the proof that the bug existed.
3. **On post-fix HEAD** (i.e., after TASK-0612 lands): `cargo test --release --test d011_partition_perf_witness` PASSES.
4. Test body asserts both the loose assertion (`arena_len < id_range_size`) and the tight assertion (`arena_len == max_live_id + 1`) for every non-empty partition.
5. Test docstring references `docs/next-steps.md` BLOCKER 2026-05-04 and SPEC-22 R22 amendment.
6. `cargo clippy --all-targets --all-features -- -D warnings` — clean (the new test file does not introduce warnings).
7. `cargo fmt --check` — clean.

## Test floor delta

**+1 default** (one new `#[test]` in a new integration-test binary). New floor expectation after TASK-0612 + TASK-0613 land together: **≥ 1684 default** (1683 inline + 1 integration witness). Zero-copy and streaming-no-recycle floors unchanged (this test is gated on neither feature).

## Key risks

- **TDD-RED sequencing:** if the task is committed AFTER TASK-0612, it loses the "FAIL → PASS" empirical record. Mitigation: this task SHOULD be the first commit in the fix bundle. The plan's §5 Task 2 Step 3 includes the verbatim test-only commit message (lines 264–282).
- **API drift:** `PortRef::FreePort` and `PORTS_PER_SLOT` are publicly re-exported via `relativist_core::net`. Verify with `grep -n "pub use" relativist-core/src/net/mod.rs` if the test fails to import them.
- **Helper realism:** `build_dense_packed_net` only wires principal ports (port index 0 of each agent's slot) to FreePorts. This satisfies T1 (principal ports occupied) but does NOT exercise auxiliary-port routing — adequate for a partition-branch witness; not adequate as a reduction smoke test.

## Notes

- Verbatim test code, including doc-comment, helper body, and assertion text, is in `docs/plans/2026-05-04-d011-partition-perf-fix.md` §5 Task 2 Step 1 (lines 169–247).
- Verbatim commit message for the test-only commit is in plan §5 Task 2 Step 3 (lines 264–282).
- Pre-fix expected panic transcript (sample): `panicked at 'partition 0 took SPARSE branch: arena_len = 10000 == id_range_size = 10000 (live = 500)...'`.
- This test stands alone — no prereq, no concurrent task, just `git add tests/d011_partition_perf_witness.rs && git commit`.
- Cross-link: TASK-0611 (spec anchor), TASK-0612 (fix that flips this test PASS), TASK-0614 (bench verification ships AFTER TASK-0612).
