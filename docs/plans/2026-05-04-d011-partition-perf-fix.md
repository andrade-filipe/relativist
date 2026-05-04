# D-011 Partition Performance Fix — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **For Relativist developer agent:** this plan respects the 6-stage SDD pipeline. Stage 1 (SPLITTING) is encoded as the task list below. Stage 2 (TESTS) specifications are inlined per task. Stages 3–6 (DEV/REVIEW/QA/REFACTOR) follow the standard `developer → reviewer → qa → developer` flow. SPEC amendments (Phase A) MUST be performed by the `especialista-specs` agent — a handoff brief is included in §3.

**Goal:** Restore `local --workers=N` partition performance to v1 levels (≈ 12 s for `ep_con 5M w=2`, currently 22 s) by correcting the SPARSE-branch threshold metric in `partition::helpers::build_subnet_with_config`. Zero correctness regressions; all 1683/1726/1680 tests must remain green and the v1 floor of 690 must be preserved.

**Architecture:** Three changes in concert: (1) SPEC-22 R22 + R30 amendment to redefine the threshold metric from `id_range_size` to `effective_arena_size = max_live_id + 1` (the actual memory dense `build_subnet` would allocate); (2) `build_subnet_with_config` rewrite to use the new metric; (3) test suite rewrite — existing UT-0484-02..04 and UT-0492-01..02 pin the OLD metric and must be reformulated, plus one new regression test (`d011_perf_witness_partition_takes_dense_branch_for_healthy_workload`) that catches this bug class.

**Tech Stack:** Rust 2021 edition, `cargo build --release -p relativist-cli`, `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`. SDD pipeline: `task-splitter → test-generator → developer → reviewer → qa → developer`. Spec edits: `especialista-specs`.

---

## §1 — Background & Root Cause Recap

This plan implements the fix identified in `docs/next-steps.md` BLOCKER section (investigation 2026-05-04). Briefly:

- **Symptom:** `ep_con 5M w=2` `local` mode wall: v1 = 12 s, HEAD = 22 s (+83%).
- **Bisect:** entire +9.7 s regression entered at commit `d6411be` (D-009 Phase C Wave 1+2 — SparseNet + I3' assertions).
- **Root cause (probe-confirmed 2026-05-04):** `compute_id_ranges` allocates per-worker chunks of `max(100_000, base_next_id × 10)`. For 10M agents this yields chunks of 100M IDs and the last worker extends to `u32::MAX` (4.29B). Phase C added `id_range_size > 4 × live_count → SPARSE branch`; for healthy workloads the ratio is 5–800×, so the SPARSE (HashMap-based) path is taken every time. The SPARSE path costs ≈ 7 s per partition on 5M agents. Two partitions → ≈ 14 s of partition time (matches the +10 s gap).
- **Why the metric is wrong:** dense `build_subnet` (line 289 of `partition/helpers.rs`) allocates `Vec<Option<Agent>>` of size `max_live_id + 1`, NOT `id_range.end`. The clamp at line 353 (`range_end = id_range.end.min(arena_len)`) bounds the free_list iteration. Therefore, the *actual* memory cost of the dense path depends on `max_live_id`, not on the planning range. The threshold check is measuring the wrong signal.

**Consequence in v1:** dense path was taken even with id_range = 100M because the SPARSE branch did not yet exist. v1 allocated arena = 5M + 1 ≈ 5M slots = ~120 MiB per worker. No M5 pathology. The "M5 800 MiB at 200 K agents" pathology that motivated R22/R30 only occurs when `max_live_id` itself is pathologically larger than `live_count` (e.g., heavy ID fragmentation from delta-mode recycling without compaction). The new metric still catches that.

---

## §2 — Design Decision (4 options compared)

| Option | Description | Pros | Cons | Decision |
|---|---|---|---|---|
| **A** | Reduce `compute_id_ranges` chunk multiplier 10× → 2-3× | Mechanical; small diff | Risk: workloads with high in-round agent creation may exhaust per-worker range; hard-to-estimate safe multiplier; doesn't fix the wrong-metric bug | **Reject** — masks the bug |
| **B** | Raise threshold constant 4× → 15× in `build_subnet_with_config` | One-line change; preserves intent | Magic number drift; doesn't fix root cause (still measures `id_range_size`); first delta-mode workload with ID fragmentation will trip again | **Reject** — patches symptom |
| **C** | Make dense `build_subnet` aware of `id_range.end`, allocate by `id_range.end` instead of `max_live_id+1` | Aligns dense with planning range | Catastrophic memory regression (would allocate 4.29B-slot Vec for last worker); contradicts current behavior | **Reject** — breaks reality |
| **D** | Change threshold metric: `effective_arena_size = max_live_id + 1`. Take SPARSE only when dense WOULD actually inflate beyond budget | Single-call-site fix; correct semantics; matches what dense actually does; preserves M5 guard for true ID fragmentation | Requires SPEC-22 R22 + R30 amendment; existing UT-0484/0492 tests must be rewritten | **ACCEPT** ✅ |

**Chosen: Option D.** It corrects the metric to match dense's actual allocation behavior. M5 pathology (the original motivation of R22/R30) is still caught — it appears as `max_live_id >> live_count`, which the new metric detects. Healthy workloads (where `max_live_id ≈ live_count`) take the dense path as v1 did.

**Why not also Option A as a defense-in-depth?** Reducing the chunk_size has independent appeal (smaller default ID space per worker, less wasteful in `compute_id_ranges` itself). However, it introduces a separate question (what is the safe minimum for high-creation workloads?) that requires its own evidence base. Keep this for a follow-up bundle if needed; this plan stays surgical.

---

## §3 — SPEC-22 Amendment (Handoff Brief for `especialista-specs`)

**This section MUST be implemented by the `especialista-specs` agent before any code change.** Per project policy (memory: ESPECIALISTA EM SPECS dispatch policy), the user runs the agent from the TCC root session. The brief below tells the agent exactly what to change.

### Handoff brief

> **Task:** Amend SPEC-22 §3.4 (R22, R30) to correct the threshold metric used by `build_subnet` / `build_subnet_with_config` to decide between the dense and sparse construction paths. The current metric `id_range > 4 × live_agent_count` measures the planning range — a quantity decoupled from the actual memory dense allocation would consume. The corrected metric `effective_arena_size > 4 × live_agent_count` measures `max_live_id + 1` — the actual `Vec<Option<Agent>>` size dense build will allocate.
>
> **Motivation:** Empirical evidence (`docs/next-steps.md` BLOCKER 2026-05-04) shows the current metric routes 100% of healthy partition workloads through the SPARSE path, causing a +83% wall-clock regression on `ep_con 5M w=2`. Dense's actual arena cost matches the new metric, not the old one.
>
> **Scope of change:**
>
> 1. **R22** — change the threshold formula from `id_range > 4 × live_agent_count` to `effective_arena_size > 4 × live_agent_count`, where `effective_arena_size := max(live_agents.iter().map(|a| a.id).max().unwrap_or(0) as u64 + 1, 1)`. Add a normative note that this metric matches the actual `Vec<Option<Agent>>` size that dense `build_subnet` would allocate (i.e., the M5 budget is computed against the real allocation, not against the planning range).
>
> 2. **R30** — change the rejection rule for `sparse_build = false` to use the new metric. The error variant `PartitionError::DenseAllocationExceedsThreshold` SHOULD report both `effective_arena_size` and `live_count` (rename or extend payload as preferred — see §4 for the exact field-rename ABI consideration).
>
> 3. **Add R22a (new)** — clarification: M5 pathology (recycled-id fragmentation under delta mode) manifests as `max_live_id >> live_count` and is still detected by the new metric. Reference Phase D-009 SC-009 closure rationale.
>
> 4. **Update §3.4 worked example** (if any) — re-derive the example with the new metric so the spec stays internally consistent.
>
> 5. **Cross-reference:** if SPEC-04 §A4 / R10a or SPEC-19 §3.6 R41 mentions `id_range > 4 × live_agent_count`, update to the new metric. (Best-effort grep: `grep -rn "id_range.*4.*live" specs/` before editing.)
>
> **Out of scope:**
>
> - Do NOT change `compute_id_ranges` formula (`base_next_id × 10`) — that is a separate concern (Option A above) and would be a different amendment.
> - Do NOT change `R23` (CI lint forbidding `SparseNet` in `reduction/`) — orthogonal.
> - Do NOT change the `4 ×` constant — only the metric the constant multiplies.
>
> **Round 2 review (Spec-Critic):** expect attack surfaces around:
> - **Determinism (T7):** the new metric depends on `worker_agents.iter().max()` which is order-independent — keep this property explicit in the amendment.
> - **Boundary on empty partition:** `worker_agents.is_empty()` — `max(...).unwrap_or(0)` and `effective_arena_size = 1` is the safe convention; spell this out.
> - **Wire compatibility:** none — this is a build-time decision, not a wire-format field.
>
> **Test floor impact:** none from spec amendment alone (Stage 2 tests will live in implementation tasks below).

---

## §4 — File Structure

| File | Status | Responsibility |
|---|---|---|
| `specs/SPEC-22-arena-management.md` | **Modify** (via `especialista-specs`) | R22 + R30 metric amendment + new R22a |
| `specs/SPEC-04-partitioning.md` | **Modify if needed** (via `especialista-specs`) | Cross-reference update if it cites the old metric |
| `relativist-core/src/partition/helpers.rs:422-477` | **Modify** | `build_subnet_with_config` body: replace `id_range_size` computation with `effective_arena_size = worker_agents.iter().max().unwrap_or(&0) + 1`; update doc comment; update `PartitionError::DenseAllocationExceedsThreshold` field naming |
| `relativist-core/src/error.rs` | **Modify** | Rename `id_range_size` field of `DenseAllocationExceedsThreshold` to `effective_arena_size` (or add `effective_arena_size` and deprecate `id_range_size`; see Task 4 for rationale). |
| `relativist-core/src/partition/helpers.rs` (test mod, lines 1330–1545) | **Modify** | Rewrite UT-0484-02, UT-0484-03, UT-0484-04, UT-0484-05, UT-0492-01, UT-0492-02 to use scattered IDs that exercise the NEW metric. |
| `relativist-core/src/partition/helpers.rs` (test mod, append) | **Add** | New regression test `d011_perf_witness_partition_takes_dense_branch_for_healthy_workload` (witnesses the original bug). |
| `relativist-core/tests/d011_partition_perf_witness.rs` | **Create** | Integration test mirroring the production scenario (10M agents, 2 workers, expect dense branch). |
| `docs/tests/TASK-0612-tests.md` | **Create** | TEST-SPEC-0612 — full inventory + per-test spec for this bundle. |
| `docs/backlog/TASK-0611-spec22-r22-r30-metric-amendment.md` | **Create** | Atomic task: SPEC-22 amendment (especialista-specs handoff). |
| `docs/backlog/TASK-0612-build-subnet-effective-arena-metric.md` | **Create** | Atomic task: code change + tests. |
| `docs/backlog/TASK-0613-d011-perf-witness-integration-test.md` | **Create** | Atomic task: integration regression witness. |
| `docs/backlog/TASK-0614-bench-verification.md` | **Create** | Atomic task: re-run bisect timings to confirm 12 s wall. |
| `docs/next-steps.md` | **Modify** | Move BLOCKER block to closed; record fix landing. |

---

## §5 — Task Breakdown (TDD-first, 1 commit per task)

> **Coding standards** (from `CLAUDE.md`):
> - No `unwrap()` in production code — use `?` or explicit error handling. (Tests MAY use `unwrap()`.)
> - No `unsafe` without `// SAFETY:` comment.
> - No `println!` — use `tracing` macros only.
> - `thiserror` for errors, not `anyhow`.
> - `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]` on public types where applicable.
> - All commits must pass: `cargo test && cargo clippy -- -D warnings && cargo fmt --check`.
> - Tests floor: 1683 default / 1726 zero-copy / 1680 streaming-no-recycle. Floor MUST NOT regress.
> - **Every commit message** ends with `Co-Authored-By: <name> <email>` if collaborating; the convention used in this repo is `Co-Authored-By: Claude <noreply@anthropic.com>` for AI-assisted commits — keep it.

---

### Task 1: SPEC-22 amendment (handoff to `especialista-specs`)

**Files:**
- Modify: `specs/SPEC-22-arena-management.md` (R22, R30, new R22a)
- Modify (if needed): `specs/SPEC-04-partitioning.md` (cross-references)

- [ ] **Step 1: Save the handoff brief to a draft file**

```bash
cat > docs/handoffs/2026-05-04-spec22-r22-r30-metric-amendment.md <<'EOF'
# Handoff: SPEC-22 R22 + R30 metric amendment

(Paste the full §3 handoff brief from docs/plans/2026-05-04-d011-partition-perf-fix.md verbatim.)
EOF
```

- [ ] **Step 2: Hand off to `especialista-specs`**

From the TCC root session (per project policy: ESPECIALISTA EM SPECS dispatch policy), invoke:

```
Subagent: especialista-specs
Mode: amend existing spec
Target: specs/SPEC-22-arena-management.md (R22, R30, +R22a)
Brief: docs/handoffs/2026-05-04-spec22-r22-r30-metric-amendment.md
```

- [ ] **Step 3: After agent completes, verify changes**

```bash
git diff specs/SPEC-22-arena-management.md
grep -n "effective_arena_size\|max_live_id" specs/SPEC-22-arena-management.md   # Expected: ≥ 3 hits
grep -n "id_range > 4" specs/SPEC-22-arena-management.md                         # Expected: 0 hits in normative text
```

- [ ] **Step 4: Run `spec-critic` Round 1**

Per project workflow (`docs/WORKFLOWS.md` spec-review pipeline). Verify Round 1 verdict; if `ACCEPT_WITH_FIXES`, return to `especialista-specs` for Round 2 closure log.

- [ ] **Step 5: Commit (via especialista-specs)**

```
spec(d-011): amend SPEC-22 R22+R30 — effective_arena_size threshold metric (closes BLOCKER 2026-05-04)
```

---

### Task 2: New regression test FAILS (bug witness)

**Files:**
- Create: `relativist-core/tests/d011_partition_perf_witness.rs`

This test witnesses the bug: a "healthy" workload (live agents densely packed in their id_range) MUST take the dense path. With current code, it takes sparse — test FAILS. After Task 4, it passes.

- [ ] **Step 1: Write the failing integration test**

> **2026-05-04 plan correction:** the original draft of this test used `arena_len < id_range_size` as the discriminator, on the assumption that `SparseNet::to_dense` allocates by `id_range.end`. **That is no longer true** — QA-D009-005 (already landed on `v2-development`) changed `to_dense` to allocate by `max_id + 1` regardless of the supplied `id_range` (see `relativist-core/src/net/sparse.rs:325-336`). Both branches now produce the SAME `arena_len`. Reliable discriminator: `subnet.next_id` after `split_with_config`. From `partition/split.rs:93-98`, the override `subnet.next_id = max(subnet.next_id, max_agent_id + 1)` resolves to:
> - **DENSE** (build_subnet sets `subnet.next_id = 0` initially) → final = `max(0, max_agent_id + 1)` = `max_agent_id + 1`.
> - **SPARSE** (build_subnet_sparse sets `sparse.next_id = id_range.start`, propagated by to_dense) → final = `max(id_range.start, max_agent_id + 1)` = `id_range.start` whenever `id_range.start > max_agent_id` (which is always the case for non-first partitions: `compute_id_ranges` yields `id_range.start = base_next_id + i × chunk_size ≥ base_next_id` for worker `i`).
>
> The test below uses this discriminator. Rationale documented inline in the test file.

```rust
// relativist-core/tests/d011_partition_perf_witness.rs
//! D-011 BLOCKER 2026-05-04 — partition performance regression witness.
//!
//! For a healthy workload (live agents densely packed inside their id_range),
//! `partition::split_with_config` MUST route every partition through the DENSE
//! `build_subnet` path. Using the SPARSE path here is a 5–7× wall-clock
//! regression (proven empirically by the bisect in `docs/next-steps.md`
//! BLOCKER 2026-05-04).
//!
//! Regression witness for SPEC-22 v2.4 R22 amendment (effective_arena_size
//! metric, replacing the broken id_range_size metric).
//!
//! ## Discriminator: `subnet.next_id`
//!
//! After QA-D009-005, `SparseNet::to_dense` sizes the dense arena by
//! `max_id + 1` regardless of the requested `id_range`. Both branches
//! therefore produce the same `subnet.agents.len()`, so arena size cannot
//! discriminate. We use `subnet.next_id` instead. From `partition/split.rs:93-98`
//! the post-build override is `subnet.next_id = max(subnet.next_id, max_agent_id + 1)`:
//!
//! - DENSE: `build_subnet` initializes `next_id = 0` → final = `max_agent_id + 1`.
//! - SPARSE: `build_subnet_sparse` initializes `next_id = id_range.start` →
//!           final = `max(id_range.start, max_agent_id + 1)` = `id_range.start`
//!           (because `compute_id_ranges` always assigns `id_range.start ≥ base_next_id ≥ live_count`).

use relativist_core::net::{Net, PortRef, Symbol, PORTS_PER_SLOT};
use relativist_core::partition::{self, FennelStrategy, PartitionConfig};

/// Build a small healthy net: N live CON agents at IDs 0..N (densely packed),
/// principal ports wired to FreePorts (T1 compliance).
fn build_dense_packed_net(n: u32) -> Net {
    let mut net = Net::new();
    for _ in 0..n {
        net.create_agent(Symbol::Con);
    }
    for i in 0..n {
        let port_idx = i as usize * PORTS_PER_SLOT;
        net.ports[port_idx] = PortRef::FreePort(1_000_000 + i);
    }
    net
}

#[test]
fn d011_witness_partition_dense_branch_for_healthy_workload() {
    // 1000 live CON agents densely packed at IDs 0..999. After FENNEL split into
    // 2 workers, each partition gets ≈ 500 live agents. `compute_id_ranges(2, 1000)`
    // yields chunk_size = max(100_000, 1000 × 10) = 100_000, so:
    //   worker 0: id_range = [1000, 101_000), live ≈ 500, max_agent_id ≈ 499.
    //   worker 1: id_range = [101_000, u32::MAX), live ≈ 500, max_agent_id ≈ 999.
    //
    // OLD metric (pre-v2.4): id_range_size (100_000) > 4 × 500 = 2000 → SPARSE for both
    //                        → subnet.next_id = id_range.start (1000 / 101_000) → BUG.
    // NEW metric (v2.4): effective_arena_size = max_live_id + 1 (≈ 500–1000) ≤ 4 × 500
    //                    → DENSE for both → subnet.next_id = max_agent_id + 1 → CORRECT.
    let net = build_dense_packed_net(1000);
    let strategy = FennelStrategy::new();
    let cfg = PartitionConfig::default();

    let plan = partition::split_with_config(net, 2, &strategy, &cfg);

    for (i, partition) in plan.partitions.iter().enumerate() {
        let live_count = partition.subnet.count_live_agents();
        if live_count == 0 {
            continue; // empty partition: both branches return Net::new()
        }
        let max_live_id = partition.subnet.agents.iter()
            .enumerate()
            .filter_map(|(idx, slot)| slot.as_ref().map(|_| idx as u32))
            .max()
            .expect("non-empty partition has at least one live agent");
        let expected_dense_next_id = max_live_id + 1;
        let id_range_start = partition.id_range.start;

        assert_eq!(
            partition.subnet.next_id, expected_dense_next_id,
            "partition {i} took SPARSE branch: subnet.next_id = {} (= id_range.start = {}); \
             expected DENSE branch: subnet.next_id = max_agent_id + 1 = {}. \
             See docs/next-steps.md BLOCKER 2026-05-04.",
            partition.subnet.next_id, id_range_start, expected_dense_next_id,
        );
        // Sanity: confirm the discriminator is real (id_range.start would be a different value).
        assert_ne!(
            partition.subnet.next_id, id_range_start,
            "discriminator collapse: max_agent_id + 1 == id_range.start for partition {i}; \
             test setup must scatter live IDs differently to maintain the SPARSE/DENSE distinction",
        );
    }
}
```

- [ ] **Step 2: Confirm the test FAILS on current HEAD**

```bash
cargo test --release --test d011_partition_perf_witness -- --nocapture
```

Expected output (current behavior, BEFORE fix):
```
---- d011_witness_partition_dense_branch_for_healthy_workload stdout ----
panicked at 'partition 0 took SPARSE branch: arena_len = 10000 == id_range_size = 10000 (live = 500)...'
```

If the test panics with the SPARSE-branch message → bug witnessed correctly. Proceed to Task 3.
If the test passes → STOP. Re-read the assertions; the test is not catching the bug.

- [ ] **Step 3: Commit (test-only — proves bug exists)**

```bash
git add relativist-core/tests/d011_partition_perf_witness.rs
git commit -m "$(cat <<'EOF'
test(d-011): add d011 partition perf regression witness (FAILS on HEAD)

Captures the production bug isolated by the bisect in docs/next-steps.md
BLOCKER 2026-05-04. The test asserts that healthy workloads (live agents
densely packed) take the DENSE build_subnet branch. On HEAD, every
partition takes SPARSE due to the id_range_size > 4 × live_count metric
(SPEC-22 R22) measuring the planning range instead of the actual arena
size. Test will pass after TASK-0612 (build_subnet_with_config metric
correction).

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: Rewrite UT-0484-02..05 + UT-0492-01..02 to exercise the NEW metric

**Files:**
- Modify: `relativist-core/src/partition/helpers.rs` (test mod, lines 1340–1545)

These existing tests pin the OLD metric — they construct nets with 10 live agents at IDs 0..9 and rely on `id_range_size > 40` to trigger the SPARSE branch. Under the new metric, `effective_arena_size = 10`, which never trips. We rewrite them to use **scattered IDs** so `max_live_id` itself trips the threshold (matches the M5 pathology rationale).

- [ ] **Step 1: Rewrite UT-0484-03 `sparse_build_false_above_threshold_rejects`**

Replace lines 1372–1404 with:

```rust
    // UT-0484-03 (REVISED 2026-05-04 — D-011 BLOCKER fix):
    // sparse_build=false with effective_arena_size > 4 × live_count must reject.
    // Under the NEW SPEC-22 R22 metric, "above threshold" requires scattered live IDs
    // (not just a generous id_range.end). 10 live agents at scattered IDs whose
    // max ≥ 40 forces effective_arena_size ≥ 41 > 4 × 10.
    #[test]
    fn sparse_build_false_above_threshold_rejects() {
        use crate::error::PartitionError;
        use crate::partition::PartitionConfig;

        // 10 live agents, scattered so max_live_id = 49 → effective_arena_size = 50.
        // 50 > 4 × 10 = 40 → threshold exceeded under new metric.
        let mut net = Net::new();
        // Force next_id high before creating agents so they get scattered IDs.
        // Use a controlled approach: create at deterministic positions.
        for _ in 0..50 {
            net.create_agent(Symbol::Era); // creates IDs 0..49
        }
        // Remove every 5th agent to leave 10 live at IDs {0, 5, 10, 15, ..., 45}.
        // After removal: live agents = {0, 5, 10, 15, 20, 25, 30, 35, 40, 45}.
        // max_live_id = 45, live_count = 10, effective_arena_size = 46 > 40 → exceeded.
        // (We use `remove_agent` which puts removed IDs in free_list — that's fine.)
        let live: Vec<u32> = (0..50).step_by(5).collect();
        let to_remove: Vec<u32> = (0..50).filter(|i| !live.contains(i)).collect();
        for id in to_remove {
            net.remove_agent(id);
        }
        for &i in &live {
            let port_idx = i as usize * PORTS_PER_SLOT;
            net.ports[port_idx] = PortRef::FreePort(1_000_000 + i);
        }
        let agents = live.clone();
        let sigma: HashMap<AgentId, WorkerId> = agents.iter().map(|&id| (id, 0u32)).collect();

        let cfg = PartitionConfig {
            sparse_build: false,
            ..PartitionConfig::default()
        };
        // id_range can be ANY value here — what matters is effective_arena_size.
        // Use 0..50 (matches max_live_id + 1).
        let result = build_subnet_with_config(&cfg, 0, &net, &agents, &sigma, &[], 0, 0..50);
        assert!(
            matches!(
                result,
                Err(PartitionError::DenseAllocationExceedsThreshold { .. })
            ),
            "scattered live IDs (max={}, live=10): expected DenseAllocationExceedsThreshold under new metric, got {:?}",
            45, result
        );
    }
```

- [ ] **Step 2: Rewrite UT-0484-04 `sparse_build_true_above_threshold_uses_sparse_path`**

Replace lines 1406–1433 with the same scattered-ID setup, just changing `sparse_build` to `true` and asserting `result.is_ok()`:

```rust
    // UT-0484-04 (REVISED 2026-05-04 — D-011 BLOCKER fix):
    // sparse_build=true with effective_arena_size > 4 × live_count must succeed
    // via SPARSE path (no rejection).
    #[test]
    fn sparse_build_true_above_threshold_uses_sparse_path() {
        use crate::partition::PartitionConfig;

        let mut net = Net::new();
        for _ in 0..50 {
            net.create_agent(Symbol::Era);
        }
        let live: Vec<u32> = (0..50).step_by(5).collect();
        let to_remove: Vec<u32> = (0..50).filter(|i| !live.contains(i)).collect();
        for id in to_remove {
            net.remove_agent(id);
        }
        for &i in &live {
            let port_idx = i as usize * PORTS_PER_SLOT;
            net.ports[port_idx] = PortRef::FreePort(1_000_000 + i);
        }
        let agents = live.clone();
        let sigma: HashMap<AgentId, WorkerId> = agents.iter().map(|&id| (id, 0u32)).collect();

        let cfg = PartitionConfig {
            sparse_build: true,
            ..PartitionConfig::default()
        };
        let result = build_subnet_with_config(&cfg, 0, &net, &agents, &sigma, &[], 0, 0..50);
        assert!(
            result.is_ok(),
            "sparse_build=true above threshold (scattered live IDs): must not reject, got {:?}",
            result
        );
    }
```

- [ ] **Step 3: Rewrite UT-0484-05 `error_field_id_range_size_correct`**

Find lines 1436+ (the `error_field_id_range_size_correct` test) and rewrite it to assert the NEW field name `effective_arena_size`. The error variant rename happens in Task 4 — this test will FAIL until then. The test code:

```rust
    // UT-0484-05 (REVISED 2026-05-04 — D-011 BLOCKER fix):
    // error variant carries effective_arena_size (renamed from id_range_size in Task 4).
    #[test]
    fn error_field_effective_arena_size_correct() {
        use crate::error::PartitionError;
        use crate::partition::PartitionConfig;

        // Scattered: 100 live agents at IDs 0, 5, 10, ..., 495. max = 495, eff_arena = 496.
        // 496 > 4 × 100 = 400 → exceeded.
        let mut net = Net::new();
        for _ in 0..500 {
            net.create_agent(Symbol::Era);
        }
        let live: Vec<u32> = (0..500).step_by(5).collect();
        let to_remove: Vec<u32> = (0..500).filter(|i| !live.contains(i)).collect();
        for id in to_remove {
            net.remove_agent(id);
        }
        for &i in &live {
            let port_idx = i as usize * PORTS_PER_SLOT;
            net.ports[port_idx] = PortRef::FreePort(1_000_000 + i);
        }
        let agents = live.clone();
        let sigma: HashMap<AgentId, WorkerId> = agents.iter().map(|&id| (id, 0u32)).collect();

        let cfg = PartitionConfig {
            sparse_build: false,
            ..PartitionConfig::default()
        };
        let result = build_subnet_with_config(&cfg, 0, &net, &agents, &sigma, &[], 0, 0..1000);
        match result {
            Err(PartitionError::DenseAllocationExceedsThreshold {
                partition_index,
                effective_arena_size,
                live_count,
            }) => {
                assert_eq!(partition_index, 0);
                assert_eq!(effective_arena_size, 496, "effective_arena_size = max_live_id + 1");
                assert_eq!(live_count, 100);
            }
            other => panic!("expected DenseAllocationExceedsThreshold, got {:?}", other),
        }
    }
```

- [ ] **Step 4: Rewrite UT-0492-01 + UT-0492-02 (sparse/dense path tests)**

These tests pin the same metric. Apply the scattered-ID treatment to UT-0492-01 (`sparse_path_taken_above_threshold`, line 1504) and the inverse to UT-0492-02 (`dense_path_taken_below_threshold`, line 1532). Code:

```rust
    /// UT-0492-01 (REVISED 2026-05-04 — D-011): sparse path taken when
    /// effective_arena_size > 4 × live_count.
    #[test]
    fn sparse_path_taken_above_threshold() {
        // 10 live at IDs 0, 6, 12, ..., 54 → max_live_id = 54, eff_arena = 55, 55 > 40.
        let mut net = Net::new();
        for _ in 0..55 {
            net.create_agent(Symbol::Era);
        }
        let live: Vec<u32> = (0..55).step_by(6).take(10).collect();
        assert_eq!(live.len(), 10, "test setup expects exactly 10 live agents");
        let to_remove: Vec<u32> = (0..55).filter(|i| !live.contains(i)).collect();
        for id in to_remove {
            net.remove_agent(id);
        }
        for &i in &live {
            let port_idx = i as usize * PORTS_PER_SLOT;
            net.ports[port_idx] = PortRef::FreePort(1_000_000 + i);
        }
        let agents = live.clone();
        let sigma: HashMap<AgentId, WorkerId> = agents.iter().map(|&id| (id, 0u32)).collect();
        let cfg = crate::partition::PartitionConfig::default(); // sparse_build = true

        let result = build_subnet_with_config(&cfg, 0, &net, &agents, &sigma, &[], 0, 0..55);
        assert!(
            result.is_ok(),
            "sparse_build=true + threshold exceeded should succeed, got {:?}",
            result
        );
        let subnet = result.unwrap();
        // Sparse path produces an arena sized by id_range.end - id_range.start (via to_dense),
        // not by max_live_id + 1.
        // Dense path would produce arena_len = 55. Sparse via to_dense(0..55) ALSO produces 55.
        // Discriminate by checking free_list: sparse populates free_list to span the id_range
        // minus live, dense populates similarly but ordering may differ.
        // For this test, just assert success — the regression witness in tests/ does the
        // tighter discrimination.
        assert_eq!(subnet.count_live_agents(), 10);
    }

    /// UT-0492-02 (REVISED 2026-05-04 — D-011): dense path taken when
    /// effective_arena_size ≤ 4 × live_count.
    #[test]
    fn dense_path_taken_below_threshold() {
        // 10 live agents densely packed at IDs 0..10. max_live_id = 9, eff_arena = 10, 10 < 40.
        let mut net = Net::new();
        for _ in 0..10 {
            net.create_agent(Symbol::Era);
        }
        for i in 0..10u32 {
            let port_idx = i as usize * PORTS_PER_SLOT;
            net.ports[port_idx] = PortRef::FreePort(1_000_000 + i);
        }
        let agents: Vec<u32> = (0..10).collect();
        let sigma: HashMap<AgentId, WorkerId> = agents.iter().map(|&id| (id, 0u32)).collect();
        let cfg = crate::partition::PartitionConfig::default(); // sparse_build = true

        // id_range = 0..1000 (NOT relevant under new metric; what matters is max_live_id).
        let result = build_subnet_with_config(&cfg, 0, &net, &agents, &sigma, &[], 0, 0..1000);
        assert!(result.is_ok());
        let subnet = result.unwrap();
        // Dense path sizes arena to max_live_id + 1 = 10.
        assert_eq!(
            subnet.agents.len(),
            10,
            "dense branch: arena = max_live_id + 1 = 10 (NOT id_range.end = 1000)"
        );
        assert_eq!(subnet.count_live_agents(), 10);
    }
```

- [ ] **Step 5: Run the rewritten tests on HEAD (before code change)**

```bash
cargo test --release --lib partition::helpers::tests::sparse_build_false_above_threshold_rejects 2>&1 | tail -3
cargo test --release --lib partition::helpers::tests::sparse_build_true_above_threshold_uses_sparse_path 2>&1 | tail -3
cargo test --release --lib partition::helpers::tests::error_field_effective_arena_size_correct 2>&1 | tail -3
cargo test --release --lib partition::helpers::tests::sparse_path_taken_above_threshold 2>&1 | tail -3
cargo test --release --lib partition::helpers::tests::dense_path_taken_below_threshold 2>&1 | tail -3
```

Expected results (BEFORE Task 4 fix):
- `sparse_build_false_above_threshold_rejects`: **PASS** (scattered IDs trip both old and new metric — old: id_range_size=50>40, new: eff_arena=46>40)
- `sparse_build_true_above_threshold_uses_sparse_path`: **PASS** (same)
- `error_field_effective_arena_size_correct`: **FAIL TO COMPILE** (field `effective_arena_size` does not exist yet)
- `sparse_path_taken_above_threshold`: **PASS**
- `dense_path_taken_below_threshold`: **FAIL** (subnet.agents.len() == 1000 because SPARSE branch via to_dense(0..1000); we expect 10)

The compile failure of UT-0484-05 is expected: the field rename is part of Task 4. The runtime failure of UT-0492-02 is exactly the bug we're fixing. Do NOT commit Task 3 yet — the test file does not compile.

- [ ] **Step 6: Skip commit until Task 4 lands** — Task 3 + Task 4 must commit together because the field rename creates a transient inconsistency.

---

### Task 4: Implement the metric change + error field rename

**Files:**
- Modify: `relativist-core/src/error.rs`
- Modify: `relativist-core/src/partition/helpers.rs:422-477` (`build_subnet_with_config` body + doc)

- [ ] **Step 1: Locate the error variant**

```bash
grep -n "DenseAllocationExceedsThreshold" relativist-core/src/error.rs relativist-core/src/partition/helpers.rs
```

Expected: 1 definition in `error.rs`, ≥ 3 references in `helpers.rs`.

- [ ] **Step 2: Rename `id_range_size` → `effective_arena_size` in the error variant**

In `relativist-core/src/error.rs`, find the variant:

```rust
    DenseAllocationExceedsThreshold {
        partition_index: usize,
        id_range_size: u64,
        live_count: u64,
    },
```

Replace with:

```rust
    /// SPEC-22 R30 (D-011 amendment 2026-05-04): rejected because the dense
    /// arena would allocate `effective_arena_size = max_live_id + 1` slots,
    /// which exceeds `4 × live_count` (M5 budget). The previous formulation
    /// measured `id_range.end - id_range.start` and routed all healthy
    /// workloads through SPARSE — see `docs/next-steps.md` BLOCKER
    /// 2026-05-04 for the bisect transcript.
    DenseAllocationExceedsThreshold {
        partition_index: usize,
        effective_arena_size: u64,
        live_count: u64,
    },
```

Update the `#[error("...")]` format string accordingly:

```rust
    #[error(
        "dense allocation in partition {partition_index} would size arena to {effective_arena_size} slots \
         (> 4 × live_count = {live_count}); see SPEC-22 R30 (M5 budget)"
    )]
```

- [ ] **Step 3: Update `build_subnet_with_config` body**

In `relativist-core/src/partition/helpers.rs`, replace the metric computation at the top of `build_subnet_with_config` (lines 432–434):

```rust
    let id_range_size = (id_range.end as u64).saturating_sub(id_range.start as u64);
    let live_count = worker_agents.len() as u64;
    let threshold_exceeded = id_range_size > 4 * live_count;
```

With:

```rust
    // SPEC-22 R22 (D-011 amendment 2026-05-04): threshold metric matches the
    // actual `Vec<Option<Agent>>` size that dense `build_subnet` would allocate.
    // The previous metric used `id_range.end - id_range.start`, which is the
    // PLANNING range from `compute_id_ranges` and is decoupled from real arena
    // memory (dense allocates by `max_live_id + 1`, see line 301-303). Using
    // the planning range routed every healthy workload through SPARSE; see
    // `docs/next-steps.md` BLOCKER 2026-05-04 for the bisect transcript.
    let live_count = worker_agents.len() as u64;
    let effective_arena_size: u64 = worker_agents
        .iter()
        .copied()
        .max()
        .map(|max_id| max_id as u64 + 1)
        .unwrap_or(0);
    let threshold_exceeded = effective_arena_size > 4 * live_count;
```

- [ ] **Step 4: Update the error construction site**

In the same function (around line 438–444), replace:

```rust
        return Err(
            crate::error::PartitionError::DenseAllocationExceedsThreshold {
                partition_index,
                id_range_size,
                live_count,
            },
        );
```

With:

```rust
        return Err(
            crate::error::PartitionError::DenseAllocationExceedsThreshold {
                partition_index,
                effective_arena_size,
                live_count,
            },
        );
```

- [ ] **Step 5: Update the doc comment of `build_subnet_with_config`**

Replace the doc-comment block (lines 402–420) with:

```rust
/// SPEC-22 §3.4 R30 (D-011 amendment 2026-05-04): `build_subnet` with
/// effective-arena threshold guard.
///
/// Wraps `build_subnet` with a threshold check. Routes between dense and
/// sparse construction based on the metric:
///
/// ```text
/// effective_arena_size = max_live_id + 1   (matches dense allocation, line 301-303)
/// threshold_exceeded   = effective_arena_size > 4 × live_count
/// ```
///
/// - If `threshold_exceeded && !config.sparse_build` → returns
///   `Err(PartitionError::DenseAllocationExceedsThreshold { ... })`.
/// - If `threshold_exceeded && config.sparse_build` → SPARSE path
///   (`build_subnet_sparse` → `to_dense(id_range)`). M5 budget honored.
/// - Otherwise → DENSE path (`build_subnet`).
///
/// The `partition_index` parameter is passed through to the error payload
/// for diagnostic purposes.
///
/// # Threshold rationale
///
/// The 4× factor (SPEC-22 R22) is a fixed safety margin against the M5
/// pathology (`>75%` dead slots in the arena). The PRE-D-011 formulation
/// measured `id_range_size` (the planning range from `compute_id_ranges`),
/// which is decoupled from actual arena memory: dense `build_subnet`
/// allocates `Vec<Option<Agent>>` of size `max_live_id + 1` regardless of
/// `id_range.end`. Using the planning range routed every healthy workload
/// through SPARSE — a 5–7× wall-clock regression on `ep_con 5M w=2`.
/// `docs/next-steps.md` BLOCKER 2026-05-04 for the bisect transcript.
///
/// # Empty partition
///
/// `worker_agents.is_empty()` ⇒ `effective_arena_size = 0`, `threshold_exceeded
/// = false`, dense path returns `Net::new()` (line 297–299).
// 8 params — justified: config, partition_index, net, agents, sigma, borders, worker_id, id_range.
```

- [ ] **Step 6: Run the test suite from Task 3 + new witness from Task 2**

```bash
cargo build --release -p relativist-cli 2>&1 | tail -3
cargo test --release --lib partition::helpers::tests::sparse_build 2>&1 | tail -10
cargo test --release --lib partition::helpers::tests::error_field 2>&1 | tail -3
cargo test --release --lib partition::helpers::tests::dense_path_taken_below_threshold 2>&1 | tail -3
cargo test --release --test d011_partition_perf_witness 2>&1 | tail -3
```

Expected: all PASS.

- [ ] **Step 7: Run the FULL test suite to detect collateral damage**

```bash
cargo test 2>&1 | tail -20
```

Expected output line:
```
test result: ok. <N> passed; 0 failed; ...
```

`<N>` MUST be ≥ **1683** (default profile floor). If less, a test was broken — investigate.

- [ ] **Step 8: Run zero-copy + streaming-no-recycle profiles**

```bash
cargo test --features zero-copy 2>&1 | tail -3   # ≥ 1726
cargo test --features streaming-no-recycle 2>&1 | tail -3   # ≥ 1680
```

- [ ] **Step 9: Lint + fmt**

```bash
cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -5
cargo fmt --check
```

Both MUST pass cleanly.

- [ ] **Step 10: Commit (Task 3 + Task 4 together)**

```bash
git add relativist-core/src/error.rs \
        relativist-core/src/partition/helpers.rs \
        relativist-core/tests/d011_partition_perf_witness.rs
git commit -m "$(cat <<'EOF'
fix(d-011): SPEC-22 R22 effective_arena_size threshold (closes BLOCKER 2026-05-04)

PROBLEM
  build_subnet_with_config used id_range_size > 4 × live_count to decide
  between dense and sparse construction. id_range_size is the PLANNING
  range from compute_id_ranges (next_id × 10), not the actual memory
  dense build_subnet would allocate (which is max_live_id + 1, line 301).
  For healthy workloads the planning range is 5-800× the live count, so
  every partition took the slow SPARSE branch. Empirically 7s per
  partition on ep_con 5M (vs <1s for dense in v1) → +83% wall regression.

FIX
  Replace the metric with effective_arena_size = max_live_id + 1 — the
  exact size dense build_subnet allocates. M5 pathology (recycled-id
  fragmentation under delta mode) is still detected (it manifests as
  max_live_id ≫ live_count and trips the same constant 4×).

CHANGES
- specs/SPEC-22 R22 + R30 amended (commit <SPEC AMEND COMMIT>) — metric
  redefined; new R22a clarifies M5 still caught.
- error::PartitionError::DenseAllocationExceedsThreshold renamed
  id_range_size → effective_arena_size.
- partition::helpers::build_subnet_with_config rewrites the metric
  computation; doc rewritten with the rationale.
- 5 existing tests rewritten to exercise scattered-ID workloads that
  trip the new metric (UT-0484-03, UT-0484-04, UT-0484-05 — renamed
  to error_field_effective_arena_size_correct, UT-0492-01, UT-0492-02).
- 1 new regression witness in tests/d011_partition_perf_witness.rs
  (asserts dense branch for healthy workload).

VERIFICATION
- cargo test: <N> default / <M> zero-copy / <K> streaming-no-recycle
- cargo clippy --all-targets --all-features -- -D warnings: clean
- cargo fmt --check: clean
- d011_partition_perf_witness: PASS (was failing pre-fix)

PERF (TASK-0614 verifies):
- ep_con 5M w=2 local: expected 12s (v1), measured <TBD>

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: Bench verification — confirm v1 wall-time restored

**Files:**
- Read: `/tmp/r-bisect/v1/target/release/relativist.exe` (existing)
- Read: `target/release/relativist.exe` (HEAD post-fix)
- Create: `docs/benchmarks/d011-perf-fix-verification-2026-05-04.md`

- [ ] **Step 1: Re-generate ep_con 5M input on HEAD (post-fix)**

```bash
target/release/relativist.exe generate ep-annihilation-con -n 5000000 -o /tmp/r-bisect/ep5m_HEAD_postfix.bin
```

- [ ] **Step 2: Run timing on v1 + HEAD-postfix in matched conditions**

```bash
for bin_label in "v1:/tmp/r-bisect/v1/target/release/relativist.exe:/tmp/r-bisect/ep5m_v1.bin" \
                 "HEAD-postfix:target/release/relativist.exe:/tmp/r-bisect/ep5m_HEAD_postfix.bin"; do
  IFS=':' read -r label bin inp <<< "$bin_label"
  echo "=== $label ==="
  for i in 1 2 3; do
    "$USERPROFILE/anaconda3/python.exe" -c "
import subprocess, time
t0 = time.perf_counter()
subprocess.run([r'$bin', 'local', '--workers=2', '-i', r'$inp'],
               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
print(f'  rep $i: {time.perf_counter()-t0:.3f}s')
"
  done
done
```

Expected: HEAD-postfix median ≈ v1 median (within 10%). If HEAD-postfix is still 2× slower than v1 → fix is incomplete; STOP and investigate.

- [ ] **Step 3: Document the verification**

Create `docs/benchmarks/d011-perf-fix-verification-2026-05-04.md`:

```markdown
# D-011 Perf Fix Verification — 2026-05-04

| Build | ep_con 5M w=2 wall (median of 3) | partition_time_per_round |
|---|---|---|
| v1-feature-complete | <X.XX> s | <Y.YY> s |
| HEAD pre-fix (commit 2b6528b) | 22.88 s | ~16.5 s |
| HEAD post-fix (commit <NEW>) | <Z.ZZ> s | <W.WW> s |

**Verdict:** <PASS|FAIL> — HEAD post-fix is within <±N%> of v1.

**Hardware/conditions:** Windows 11, performance mode <ON|OFF>, build with
`CARGO_PROFILE_RELEASE_DEBUG=line-tables-only` (matching pre-fix bisect).

**Bisect transcript reference:** docs/next-steps.md BLOCKER 2026-05-04.
```

- [ ] **Step 4: Commit**

```bash
git add docs/benchmarks/d011-perf-fix-verification-2026-05-04.md
git commit -m "$(cat <<'EOF'
bench(d-011): verify partition perf fix restores v1 wall-time on ep_con 5M w=2

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

---

### Task 6: Move BLOCKER block in next-steps.md to closed state

**Files:**
- Modify: `docs/next-steps.md`
- Append to: `docs/progress.md` (per project policy — historical entries move to progress)

- [ ] **Step 1: Cut the BLOCKER block (lines 9–95 of current `next-steps.md`)**

Per project policy ("Once a task, bundle, or milestone is DONE, its record MUST be moved to `progress.md`"), move the entire BLOCKER block from `next-steps.md` to `progress.md` under a new heading:

```markdown
## D-011 BLOCKER 2026-05-04 — partition perf regression — CLOSED <DATE>

(paste the entire BLOCKER block here, with a new top-line:)

**Closure:** fixed in commit `<HASH>` via SPEC-22 R22 amendment (commit `<SPEC HASH>`)
+ build_subnet_with_config metric correction. Verification:
docs/benchmarks/d011-perf-fix-verification-2026-05-04.md.
```

- [ ] **Step 2: Update D-011 active bundle status**

In `docs/next-steps.md` D-011 section, add a row:

```markdown
| 2026-05-04 BLOCKER | partition perf regression (eff_arena_size metric) | CLOSED commit <HASH> |
```

- [ ] **Step 3: Run the test floor verification one more time**

```bash
cargo test 2>&1 | tail -3
cargo test --features zero-copy 2>&1 | tail -3
cargo test --features streaming-no-recycle 2>&1 | tail -3
```

All three MUST report `<N> passed; 0 failed`.

- [ ] **Step 4: Commit**

```bash
git add docs/next-steps.md docs/progress.md
git commit -m "$(cat <<'EOF'
docs(d-011): close BLOCKER 2026-05-04 — partition perf fix landed

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

---

## §6 — Verification Gates (must all pass before merge)

| Gate | Command | Pass criterion |
|---|---|---|
| **G1** | `cargo build --release -p relativist-cli` | exit 0 |
| **G2** | `cargo test` | `<N> passed; 0 failed`, `N ≥ 1683` |
| **G3** | `cargo test --features zero-copy` | `<M> passed; 0 failed`, `M ≥ 1726` |
| **G4** | `cargo test --features streaming-no-recycle` | `<K> passed; 0 failed`, `K ≥ 1680` |
| **G5** | `cargo clippy --all-targets --all-features -- -D warnings` | no warnings |
| **G6** | `cargo fmt --check` | exit 0 |
| **G7** | `cargo test --test d011_partition_perf_witness` | PASS (regression witness) |
| **G8** | Bench (Task 5) | HEAD post-fix wall ≤ 1.10 × v1 wall on `ep_con 5M w=2` |

---

## §7 — Rollback Plan

If any of G1–G8 fails after Task 4 commit (and the cause cannot be fixed in the same commit cycle):

1. `git revert <Task 4 commit hash>` — restores HEAD to pre-fix state.
2. `git revert <Task 5 commit hash>` if it had landed.
3. Notify in `docs/next-steps.md` BLOCKER entry: "Rollback applied; investigation continues."
4. The SPEC-22 amendment (Task 1) is **NOT** reverted — it stands as the corrected design intent. Rollback only the implementation; re-attempt with refined approach.

---

## §8 — Open Questions (for spec-critic Round 1)

These are the predictable Round 1 attack vectors. Pre-empted answers in `[brackets]` are the design rationale.

- **Q1.** Why `4 ×`? [Same constant as before — preserves SPEC-22 R22 calibration. Only the metric changes, not the multiplier.]
- **Q2.** What about workloads where `max_live_id ≪ id_range.end` (the original case) AND in-round `create_agent` calls push past `4 × live`? [`create_agent` allocates from `next_id` upward within the partition's `id_range`; this happens during reduction, AFTER `build_subnet`. The threshold check is build-time. Post-build agent creation does not re-trigger the check.]
- **Q3.** Boundary: empty partition (`live_count = 0`). [`effective_arena_size = 0`, `threshold_exceeded = false`, dense returns `Net::new()` at line 297–299. No regression.]
- **Q4.** Boundary: single live agent at `id = u32::MAX - 1`. [`effective_arena_size = u32::MAX as u64`, `4 × 1 = 4`, threshold exceeded → SPARSE. Correct: dense would allocate ~16 GB. Behavior matches intent.]
- **Q5.** ABI: error variant field rename (Task 4 step 2) is breaking for any caller that pattern-matches on `id_range_size`. [Grep: `grep -rn "id_range_size" relativist-core/src/ relativist-cli/src/`. Found 0 callers outside the variant definition + tests we're rewriting in Task 3.]
- **Q6.** Determinism (T7): does `worker_agents.iter().max()` preserve T7? [Yes — `max()` is order-independent on `&[AgentId]` (= `&[u32]`); for any fixed input slice the result is deterministic. The slice itself is constructed from `sigma` deterministically by `split_with_config`. T7 holds.]

---

## §9 — Self-Review Checklist (per writing-plans skill)

- [x] **Spec coverage:** §3 Handoff brief covers R22, R30, +R22a. §5 Task 1 enforces it via `especialista-specs`. §6 G2/G3/G4 enforce floor.
- [x] **No placeholders:** all TBD fields above are `<placeholder>` markers in COMMIT MESSAGES (filled at commit time), not in instructions. Test code, error variant code, doc comment text all complete.
- [x] **Type consistency:** `effective_arena_size: u64`, `live_count: u64` — same type throughout (error variant + helper computation + tests).
- [x] **Function names:** `build_subnet_with_config`, `compute_id_ranges`, `Net::create_agent`, `Net::remove_agent`, `Net::count_live_agents` — all real, all consistent across tasks.
- [x] **TDD ordering:** Task 2 writes failing test FIRST (witnesses bug). Task 3 prepares passing tests for new metric. Task 4 implements change so all tests pass. Task 5 verifies perf. Task 6 closes documentation.
- [x] **Frequent commits:** 6 separate commits (Task 1 SPEC, Task 2 witness, Task 3+4 implementation, Task 5 bench, Task 6 docs).
- [x] **Reference accuracy:** `partition/helpers.rs:289` (build_subnet), `:301-303` (arena allocation), `:353` (clamp), `:422` (build_subnet_with_config), `:432-434` (current metric) — all line numbers verified against current HEAD.

---

## §10 — Execution Handoff

**Plan complete and saved to `docs/plans/2026-05-04-d011-partition-perf-fix.md`.**

Two execution options:

1. **Subagent-Driven (recommended for Relativist's SDD pipeline)** — task-by-task, fresh subagent per task, review between tasks. The natural agent assignment:
   - Task 1 → `especialista-specs` (from TCC root session — see project policy).
   - Tasks 2, 3, 4, 5, 6 → `developer` (sequential).
   - After Task 4: dispatch `reviewer` then `qa` per the standard 6-stage pipeline.

2. **Inline Execution** — execute Tasks 2–6 in the current session via `superpowers:executing-plans` (Task 1 still needs the spec agent — cannot be inlined here). Useful for fast iteration but skips the inter-stage review checkpoints.

**Recommended path:** Option 1. Start with Task 1 (SPEC amendment) — without it, the implementation tasks lack the normative anchor.
