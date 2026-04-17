# TEST-SPEC-0349: Populate `has_border_activity` at every WorkerRoundStats build site

**Task:** TASK-0349
**Spec:** SPEC-19 §3.1 R2 (item 2.34)
**Generated:** 2026-04-16
**Baseline before this task:** 856+ (post-TASK-0348)
**Cumulative target after this task:** 860+ (≥ +4 new tests)

---

## Scope note

TASK-0348 added the `has_border_activity` field and the helper, defaulting
the field to `false` at every existing construction site. THIS task wires
the real value at the **three** production build sites:

1. **Distributed worker** (`relativist-core/src/worker.rs`)
2. **Wire-protocol worker loop** (`relativist-core/src/protocol/worker.rs`)
3. **Local in-process simulation** (`relativist-core/src/merge/grid.rs`,
   inside `run_grid`'s per-worker loop, after `rebuild_free_port_index`)

The contract is fixed by R1 ordering: `reduce_all → rebuild_free_port_index
→ compute_border_activity → build stats`. Reordering breaks R1.

R7 compatibility: the change is purely additive on an existing
`Message::PartitionResult` payload — no new variants, no FSM changes.
Both v1 and v2 protocol carriers convey the field unchanged.

---

## Test target file paths

- `relativist-core/src/worker.rs` — inline `#[cfg(test)] mod tests`
  block for the per-round build path.
- `relativist-core/src/merge/grid.rs` — inline `#[cfg(test)] mod tests`
  block for the local-simulation round build.
- `relativist-core/src/protocol/worker.rs` — only if a separate stats
  build site exists; otherwise this file is exercised transitively by the
  worker.rs test.

All tests are synchronous `#[test]` units except IT-0349-03 which is
`#[test]` running `run_grid` synchronously (no tokio needed — `run_grid`
is in the pure `merge/` layer).

---

## Unit Tests

### UT-0349-01: `worker_round_build_sets_has_border_activity_true` (R2 positive)

**Purpose:** Driving the worker round-build with a partition whose
`free_port_index` contains a principal-port endpoint MUST yield
`stats.has_border_activity == true`.
**Target file:** `worker.rs::tests` (or wherever the per-round build
helper currently lives — call it `build_round_stats` for the spec).
**Preconditions:** Construct a small `Partition` with one agent and one
`AgentPort(_, 0)` entry in `free_port_index`. Run only the build step (no
real reduction needed if the helper is independently callable).

**Input:**
```rust
let mut partition = make_partition_with_one_principal_border();
let stats = build_round_stats(&partition, /* timing, etc. */);
```

**Expected output:**
```rust
assert!(stats.has_border_activity,
        "principal-port border MUST set has_border_activity = true");
assert_eq!(stats.worker_id, partition.worker_id);
```

**SPEC-19 R covered:** R2 (positive carrier).

---

### UT-0349-02: `worker_round_build_sets_has_border_activity_false_when_no_borders` (R2 negative)

**Purpose:** A partition with empty `free_port_index` MUST yield
`stats.has_border_activity == false`.
**Target file:** `worker.rs::tests`

**Input:**
```rust
let partition = make_partition_with_no_borders();   // empty free_port_index
let stats = build_round_stats(&partition, /* timing */);
```

**Expected output:**
```rust
assert!(!stats.has_border_activity,
        "no border endpoints ⇒ activity must be false");
```

**SPEC-19 R covered:** R2 (negative carrier).

---

### UT-0349-03: `compute_border_activity_called_after_rebuild_free_port_index` (R1+R2 ordering)

**Purpose:** The activity flag MUST reflect the **post-`rebuild_free_port_index`**
state. Catches the bug of computing on a stale index.
**Target file:** `merge/grid.rs::tests`
**Preconditions:** Construct a `Partition` whose `free_port_index` is
intentionally **stale** (empty) but whose underlying `Net` has an agent
whose principal port is exposed as a free wire (would, after rebuild,
yield an `AgentPort(_, 0)` entry).

**Input:**
```rust
let mut partition = make_partition_with_stale_index_and_principal_free_port();
assert!(partition.free_port_index.is_empty(), "precondition: stale");
// run the per-round body up to and including rebuild + activity check
let stats = run_one_round_for_test(&mut partition);
```

**Expected output:**
```rust
assert!(stats.has_border_activity,
        "must compute activity AFTER rebuild_free_port_index, not on the stale index");
assert!(!partition.free_port_index.is_empty(),
        "rebuild must have populated the index");
```

**SPEC-19 R covered:** R1 + R2 (ordering invariant).

---

## Integration Tests

### IT-0349-04: `run_grid_records_per_worker_activity_in_round_zero` (R2 end-to-end)

**Purpose:** End-to-end check that `run_grid` propagates the new field
into per-round metrics for both polarities simultaneously.
**Target file:** `merge/grid.rs::tests` (or `tests/run_grid_smoke.rs`
if integration-style placement is preferred).
**Preconditions:** Build a small net partitioned across exactly 2 workers.
Worker 0's partition has at least one principal-port border endpoint;
worker 1's partition has none.

**Input:**
```rust
let net = build_two_worker_net_one_with_principal_border();
let config = GridConfig {
    num_workers: 2,
    max_rounds: Some(1),
    strict_bsp: true,
    ..GridConfig::default()
};
let (result_net, metrics) = run_grid(net, config);
```

**Expected output:**
```rust
let round0 = &metrics.worker_stats_per_round[0];
assert_eq!(round0.len(), 2);
let activity: Vec<bool> =
    round0.iter().map(|s| s.has_border_activity).collect();
// Order is deterministic (workers are appended in WorkerId order).
assert_eq!(activity, vec![true, false],
           "round 0 must carry one true and one false");
```

**Edge cases proven:**
- Mixed polarity in the same round is supported.
- Per-worker stats vector is in `WorkerId` order.

**SPEC-19 R covered:** R2 (multi-worker end-to-end).

---

## Adversarial probes (QA candidates for Stage 5)

| # | Scenario | Why dangerous | Stage |
|---|----------|---------------|-------|
| QA-0349-A | Worker that oscillates `has_border_activity` round-to-round (true → false → true) | Verifies the helper is **idempotent on identical input** and **reactive to mutation** between rounds — no caching across rounds | QA |
| QA-0349-B | Race where `has_border_activity` would change between local reduction and stats emission | The contract pins the value to **post-rebuild_free_port_index**; UT-0349-03 covers the deterministic ordering, but QA should adversarially probe with concurrent mutation | QA |
| QA-0349-C | `WorkerRoundStats` shipped via `Message::PartitionResult` through the full v2 wire pipeline (LZ4 + 9-byte header + bincode v2) carrying `has_border_activity = true` | Confirms R7 wire compatibility round-trips the field intact when an actual coordinator-worker pair runs | QA |
| QA-0349-D | Empty workers list (`num_workers = 0`) | Should not panic; `run_grid` either errors clearly or returns immediately with empty metrics | QA |
| QA-0349-E | Single worker (always 0 borders by construction) | `has_border_activity` MUST be `false` for every round; the skip path in TASK-0351 will trigger trivially | QA |

---

## Hard-to-write-deterministically tests (FLAGGED)

- **QA-0349-B (race)** is **non-deterministic** by nature. The
  test-generator recommends covering the ordering invariant
  *deterministically* via UT-0349-03 (intentional stale-index fixture),
  and leaving QA-0349-B as an adversarial probe documented for QA but
  **not implemented as a `#[test]`**. The R1 ordering contract is
  enforced by code structure (helper called immediately after rebuild),
  not by a flaky timing test.

---

## Acceptance Gate

1. `cargo test --workspace` count: 856 → **860+** (≥ +4: UT-01, UT-02,
   UT-03, IT-04).
2. All previously passing tests still pass (no regression).
3. Existing `WorkerRoundStats` literals in test fixtures across the
   workspace continue to compile (TASK-0348 already wired the default
   `false`; this task replaces the production-path defaults with the
   real call).
4. `cargo clippy --workspace --all-targets -- -D warnings` clean.
5. `cargo fmt --check` clean.
6. Release smoke `compute add 3 5 → 8` works.

## Out of Scope

- Coordinator-side consumption of the field (TASK-0351).
- `coordinator_free_rounds` config / metric (TASK-0350).
- Real-wire FSM in `protocol/coordinator.rs` — untouched in this bundle (R7).
