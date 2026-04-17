# TEST-SPEC-0348: `has_border_activity` field + `compute_border_activity` helper

**Task:** TASK-0348
**Spec:** SPEC-19 §3.1 R1, R2 (item 2.34)
**Generated:** 2026-04-16
**Baseline before this task:** 850 lib + 4 integration (post-SPEC-18 ship)
**Cumulative target after this task:** 856+ (≥ +6 new tests)

---

## Scope note

This is the **type-foundation commit** for the SPEC-19 §3.1 bundle.
After this task lands, the canonical `WorkerRoundStats` carries a
`has_border_activity: bool` field defaulting to `false` at every existing
construction site, and a pure helper `compute_border_activity(&Partition)`
returns the SPEC-19 R1 boolean from a partition's `free_port_index`.

No behavior change is observable at runtime; subsequent tasks (0349/0351)
wire the helper into the round-build path and the coordinator decision.
Both v1 protocol and v2 wire format (SPEC-18) keep working because the
field is additive inside the existing `Message::PartitionResult` payload
(R7) — bincode v2 with `config::standard()` round-trips struct field
appends safely as long as both sides are rebuilt together (which holds
inside this workspace).

---

## Test target file paths

- `relativist-core/src/merge/helpers.rs` — inline `#[cfg(test)] mod tests`
  block for the new `compute_border_activity` helper (R1).
- `relativist-core/src/merge/types.rs` — extend the existing
  `#[cfg(test)] mod tests` block with one new serde round-trip test for
  the new field (R2 carrier).

All tests are `#[test]` synchronous units; no `tokio` runtime needed.

---

## Unit Tests

### UT-0348-01: `compute_border_activity_returns_false_for_empty_index` (R1)

**Purpose:** Empty `free_port_index` ⇒ no border endpoints ⇒ no activity.
**Target file:** `merge/helpers.rs::tests`
**Preconditions:** None — construct `Partition` with empty
`free_port_index: HashMap::new()`.

**Input:**
```rust
let partition = Partition {
    id: PartitionId(0),
    net: Net::new(),
    free_port_index: HashMap::new(),
    border_map: HashMap::new(),
    // ...other fields per Partition struct...
};
```

**Expected output:**
```rust
assert!(!compute_border_activity(&partition),
        "empty free_port_index must yield false");
```

**SPEC-19 R covered:** R1.

---

### UT-0348-02: `compute_border_activity_returns_false_when_only_aux_ports` (R1)

**Purpose:** Verify the helper specifically tests for principal port slot
(slot index 0); auxiliary ports (slots 1, 2) MUST NOT trigger activity.
**Target file:** `merge/helpers.rs::tests`
**Preconditions:** Build a `Partition` whose `free_port_index` has 3
entries, all `AgentPort(_, 1)` or `AgentPort(_, 2)`.

**Input:**
```rust
let mut idx = HashMap::new();
idx.insert(0, PortRef::AgentPort(AgentId(0), 1));
idx.insert(1, PortRef::AgentPort(AgentId(1), 2));
idx.insert(2, PortRef::AgentPort(AgentId(2), 1));
let partition = make_partition_with_index(idx);
```

**Expected output:**
```rust
assert!(!compute_border_activity(&partition),
        "auxiliary-only ports must yield false");
```

**Edge cases proven:**
- Slot 1 alone → false
- Slot 2 alone → false
- Mix of slots 1 and 2 → false

**SPEC-19 R covered:** R1.

---

### UT-0348-03: `compute_border_activity_returns_true_for_single_principal_port` (R1)

**Purpose:** A single principal-port border endpoint MUST flip the flag.
**Target file:** `merge/helpers.rs::tests`
**Preconditions:** Build a `Partition` whose `free_port_index` has 1
entry: `AgentPort(_, 0)`.

**Input:**
```rust
let mut idx = HashMap::new();
idx.insert(0, PortRef::AgentPort(AgentId(0), 0));
let partition = make_partition_with_index(idx);
```

**Expected output:**
```rust
assert!(compute_border_activity(&partition),
        "single principal-port endpoint must yield true");
```

**SPEC-19 R covered:** R1.

---

### UT-0348-04: `compute_border_activity_returns_true_when_all_principal` (R1)

**Purpose:** Sanity — every entry being a principal port still yields true
(no XOR semantics).
**Target file:** `merge/helpers.rs::tests`
**Preconditions:** 4 entries, all `AgentPort(_, 0)`.

**Input:**
```rust
let mut idx = HashMap::new();
for i in 0..4 {
    idx.insert(i, PortRef::AgentPort(AgentId(i), 0));
}
let partition = make_partition_with_index(idx);
```

**Expected output:**
```rust
assert!(compute_border_activity(&partition));
```

**SPEC-19 R covered:** R1.

---

### UT-0348-05: `compute_border_activity_returns_true_for_mixed_with_principal` (R1)

**Purpose:** Existence semantics — one principal among non-principals is
sufficient. Also verifies the helper accepts `FreePort` variants without
panicking (they MUST count as non-principal).
**Target file:** `merge/helpers.rs::tests`

**Input:**
```rust
let mut idx = HashMap::new();
idx.insert(0, PortRef::FreePort(7));
idx.insert(1, PortRef::AgentPort(AgentId(0), 1));
idx.insert(2, PortRef::AgentPort(AgentId(1), 0));   // <- the one principal
idx.insert(3, PortRef::FreePort(8));
let partition = make_partition_with_index(idx);
```

**Expected output:**
```rust
assert!(compute_border_activity(&partition),
        "mixed with one principal must yield true");
```

**Edge cases proven:**
- `FreePort(_)` alone → false (already covered by negative shape)
- `FreePort(_)` + principal → true (shown here)
- Auxiliary + principal → true (shown here)

**SPEC-19 R covered:** R1.

---

### UT-0348-06: `worker_round_stats_serde_roundtrip_with_activity_true` (R2)

**Purpose:** End-to-end verification that the new field survives bincode
v2 encode/decode under the existing `WorkerRoundStats` `serde` derive,
which is the **wire carrier** inside `Message::PartitionResult` (R7).
**Target file:** `merge/types.rs::tests` (extends the existing
`test_worker_round_stats_serde` neighbour, do NOT delete that test —
add this one beside it).
**Preconditions:** Use `bincode_v2::encode` / `decode` (the post-SPEC-18
TASK-0343 thin wrapper).

**Input:**
```rust
let stats = WorkerRoundStats {
    worker_id: WorkerId(7),
    agents_before: 12,
    agents_after: 10,
    local_redexes: 3,
    reduce_duration_secs: 0.005,
    interactions_by_rule: [1, 2, 3, 4, 5, 6],
    has_border_activity: true,   // <- new field, exercised
};
let bytes = bincode_v2::encode(&stats).expect("encode");
```

**Expected output:**
```rust
let (decoded, _): (WorkerRoundStats, _) =
    bincode_v2::decode(&bytes).expect("decode");
assert_eq!(decoded, stats);
assert!(decoded.has_border_activity,
        "round-tripped value must preserve has_border_activity = true");
```

**Edge case:** Adjacent test `worker_round_stats_serde_roundtrip_with_activity_false`
constructed with `has_border_activity: false` (default for v1 callers)
— may be added as UT-0348-07 if the existing
`test_worker_round_stats_serde` is updated to set the field to `false`
explicitly (R2 contract: the field is always serialized, both polarities).

**SPEC-19 R covered:** R2.

---

## Adversarial probes (QA candidates for Stage 5 — referenced here, not implemented)

| # | Scenario | Why dangerous | Stage |
|---|----------|---------------|-------|
| QA-0348-A | `free_port_index` with 100 000 entries, all `AgentPort(_, 1)` | Helper iterates all on `false` cases; verify O(N) and no allocation | QA |
| QA-0348-B | `PortRef::AgentPort(AgentId(u32::MAX), 0)` | Boundary on AgentId; principal slot detection must still match `0` | QA |
| QA-0348-C | `PartitionResult` carrying `WorkerRoundStats { has_border_activity: true, .. }` shipped through full `send_frame` + `recv_frame` (LZ4 + 9-byte header + bincode v2) | Confirms the field rides the wire intact end-to-end (R7); supersedes UT-0348-06 with the full v2 pipeline | QA |
| QA-0348-D | Decode a `WorkerRoundStats` payload produced before the field was added (synthetic legacy bytes) | Per spec note: bincode v2 struct-append is forward-compat only when both sides are rebuilt; this probe documents the failure mode for future deserialise-from-disk scenarios | QA |

---

## Acceptance Gate

1. `cargo test --workspace` count: 850 → **856+** (≥ +6: UT-01..06).
2. All previously passing tests still pass (no regression).
3. `cargo clippy --workspace --all-targets -- -D warnings` clean.
4. `cargo fmt --check` clean.
5. `protocol/types.rs` `make_test_stats()` fixture (line ~128) updated
   so existing `protocol::types::tests::*` keep passing.
6. `protocol/frame.rs` `make_test_stats()` fixture (line ~303) updated
   for the same reason — verify by running the v2 wire round-trip tests
   (`v2_pipeline_round_trip_all_message_variants_*`).

## Out of Scope

- Wiring `compute_border_activity` into the round build path (TASK-0349).
- Adding `coordinator_free_rounds` config / metric (TASK-0350).
- Coordinator skip-merge logic + GNF termination (TASK-0351).
- Real-wire FSM modifications in `protocol/coordinator.rs` — explicitly
  forbidden in this bundle (R7).
