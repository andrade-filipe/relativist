# TEST-SPEC-0367: `Message::InitialPartition` (disc 7) + `Message::FinalStateResult` (disc 11) — bincode round-trip

**Task:** TASK-0367
**Spec:** SPEC-19 §3.4 (R31 InitialPartition, R32 FinalStateResult, R34
  serde + bincode round-trip + CRC32 integrity, R35 CompactSubnet +
  varint benefit, R37 discriminant stability)
**Generated:** 2026-04-17
**Baseline before this task:** 969 lib (default) / 1009 lib
  (`--features zero-copy`) — post TASK-0366 per DC-A1 amended trajectory.
**Cumulative target after this task:** 973 lib / 1013 lib — **+4** new
  `#[test]` fns.

---

## Scope note

This task lands the two **large-payload** delta-protocol variants —
`InitialPartition` at discriminant 7 (C→W, round 0 only) and
`FinalStateResult` at discriminant 11 (W→C, final state collection).
Both carry a full `Partition` payload (which already serialises via
SPEC-04 `CompactSubnet` + SPEC-18 varint).

**This TEST-SPEC covers only the bincode layer.** Wire-layer tests
(`send_frame_with_threshold`, `recv_frame`, `FLAG_COMPRESSED`, CRC32,
compression-benefit `frame_len < bincode_len` assertion) are in
TEST-SPEC-0370.

Per TASK-0367 acceptance note re DC-A1: the `BorderDelta` struct-level
round-trip test has been lifted to TASK-0366; this TEST-SPEC's tests
remain `Partition`-shaped.

---

## Test target file paths

- `relativist-core/src/protocol/types.rs` — `#[cfg(test)] mod tests`
  block (existing). All 4 new `#[test]` fns live here.
- Helpers `make_test_partition()` / `make_test_stats()` already exist
  in the test module and are re-used.

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Unit Tests

### T1: `initial_partition_bincode_roundtrip_identity`

**Purpose:** R34 round-trip identity for `Message::InitialPartition`.
Encode → decode → match variant → assert round-tripped fields equal
inputs. Pin `round: 0` (R21.1; InitialPartition is round-0-only per
R31 table "always 0").

**Target file:** `protocol/types.rs::tests`

**Given:** `Message::InitialPartition { round: 0, partition:
make_test_partition() }` — use the module's existing `make_test_partition()`
helper (same pattern as `test_message_with_agents`).

**When:** `bincode_v2::encode(&msg)` → `bincode_v2::decode_value::<Message>(...)`.

**Then:**
```rust
let original = Message::InitialPartition {
    round: 0,
    partition: make_test_partition(),
};
let bytes = bincode_v2::encode(&original).expect("encode");
let (decoded, _) = bincode_v2::decode_value::<Message>(&bytes)
    .expect("decode");
match decoded {
    Message::InitialPartition { round, partition } => {
        assert_eq!(round, 0, "R21.1: InitialPartition round MUST be 0");
        assert_eq!(partition.worker_id,
                   make_test_partition().worker_id,
                   "partition.worker_id must round-trip");
        // Further field checks via Debug-format comparison:
        let expected_dbg = format!("{:?}", make_test_partition());
        let actual_dbg = format!("{:?}", partition);
        assert_eq!(actual_dbg, expected_dbg,
                   "full partition Debug-format must match");
    }
    other => panic!("expected InitialPartition, got {:?}", other),
}
```

**Assertions:**
- Variant dispatch succeeds (the decoded `Message` is
  `InitialPartition` — R37 discriminant 7 is wired).
- `round` field round-trips and is 0 (R21.1).
- `partition` field round-trips (via `Debug` formatting since
  `Partition` / `Net` don't derive `PartialEq` — see TASK-0367 note).

**SPEC-19 R covered:** R31 (variant exists at disc 7), R34
(serde + bincode round-trip), R21.1 (round=0).

---

### T2: `final_state_result_bincode_roundtrip_preserves_worker_id`

**Purpose:** R32 round-trip identity for `Message::FinalStateResult`.
Use a non-default `worker_id` (e.g. 7) so a future refactor that
silently loses the field is caught.

**Target file:** `protocol/types.rs::tests`

**Given:** A `Partition` with `worker_id = 7` (construct via the
existing helper or by destructuring / rebuilding `make_test_partition()`
with worker_id override).

**When:** Encode the `FinalStateResult` variant, decode, match.

**Then:**
```rust
let mut partition = make_test_partition();
partition.worker_id = 7;
let original = Message::FinalStateResult {
    round: 42,
    partition,
};
let bytes = bincode_v2::encode(&original).expect("encode");
let (decoded, _) = bincode_v2::decode_value::<Message>(&bytes)
    .expect("decode");
match decoded {
    Message::FinalStateResult { round, partition } => {
        assert_eq!(round, 42);
        assert_eq!(partition.worker_id, 7,
                   "FinalStateResult must preserve partition.worker_id");
    }
    other => panic!("expected FinalStateResult, got {:?}", other),
}
```

**Assertions:**
- Variant dispatch succeeds (disc 11 is wired).
- `round: u32` is preserved (value 42, non-zero — catches any
  bad default).
- `partition.worker_id` is preserved exactly.

**SPEC-19 R covered:** R32 (variant exists at disc 11), R34.

---

### T3: `initial_partition_bincode_size_monotone_in_agent_count`

**Purpose:** R35 sanity — varint encoding actually scales with content.
Assert that encoding a `Partition` with 2N agents produces a strictly
larger bincode output than a partition with N agents. Prevents a silent
regression where the variant accidentally gets fixed-width encoding
(SPEC-06 R4.1 `legacy()` config path).

**Target file:** `protocol/types.rs::tests`

**Given:** Two `InitialPartition` messages with partition agent counts
N = 10 and 2N = 20 (or whatever small N the fixture helpers produce;
the size-monotone property holds for any N ≥ 1).

**When:** Encode both; compare lengths.

**Then:**
```rust
fn make_partition_with_n_agents(n: usize) -> Partition {
    // Helper (test-only) that builds a Partition with `n` CON agents.
    // Developer uses the existing test fixture pattern from
    // `merge::types::tests` (`stats_with_activity_variants` area) or
    // constructs via `Net::new()` + `add_agent(Symbol::Con)` x n.
    // ...
}
let small = Message::InitialPartition {
    round: 0,
    partition: make_partition_with_n_agents(10),
};
let large = Message::InitialPartition {
    round: 0,
    partition: make_partition_with_n_agents(20),
};
let small_bytes = bincode_v2::encode(&small).expect("encode small");
let large_bytes = bincode_v2::encode(&large).expect("encode large");
assert!(
    small_bytes.len() < large_bytes.len(),
    "bincode must scale with agent count (small={} bytes, large={} bytes)",
    small_bytes.len(), large_bytes.len()
);
```

**Assertions:**
- Varint encoding is active (a fixint config would still scale with
  count since agents are a `Vec`, so this test is a scaling-sanity
  check, not a strict varint check — the varint check is in
  TEST-SPEC-0366 T5).
- No silent regression to a fixed-capacity encoding path.

**SPEC-19 R covered:** R35 (CompactSubnet + varint benefit — scaling
sanity); informal but testable. The stronger wire-level
"`frame_len < bincode_len`" assertion lands in TEST-SPEC-0370.

---

### T4: `final_state_result_preserves_non_trivial_partition`

**Purpose:** Symmetric large-payload test — ensure a `FinalStateResult`
carrying a partition with actual agents + wires (not just an empty
partition) round-trips structurally. Catches any serde customisation
that silently normalises `Net.agents: Vec<Option<Agent>>` — e.g. a
future "compact" serialisation that drops `None` slots would break
index invariants.

**Target file:** `protocol/types.rs::tests`

**Given:** `make_test_partition()` (builder already includes agents
per the `test_message_with_agents` precedent).

**When:** Round-trip a `FinalStateResult` carrying that partition.

**Then:**
```rust
let original_partition = make_test_partition();
let original_agent_count = original_partition.subnet.count_live_agents();
let msg = Message::FinalStateResult {
    round: 99,
    partition: original_partition,
};
let bytes = bincode_v2::encode(&msg).expect("encode");
let (decoded, _) = bincode_v2::decode_value::<Message>(&bytes)
    .expect("decode");
match decoded {
    Message::FinalStateResult { round, partition } => {
        assert_eq!(round, 99);
        assert_eq!(partition.subnet.count_live_agents(),
                   original_agent_count,
                   "agent count must be preserved through round-trip");
    }
    other => panic!("expected FinalStateResult, got {:?}", other),
}
```

**Assertions:**
- `subnet.count_live_agents()` is preserved — both count AND live
  slots. If the serde layer silently compacts `None` slots, this
  test fires because `Net`'s internal indexing (by `AgentId(u32)`)
  would shift.
- `round` field correctness is re-confirmed.

**SPEC-19 R covered:** R32, R34, R35.

---

### T5 (list-extension, not a new `#[test]`): extend `test_all_variants_serde_roundtrip`

**Purpose:** The existing blanket test iterates a `Vec<Message>` of
all variants. TASK-0367's variant additions MUST be reflected in the
list. This is NOT a new `#[test]` fn — it is a modification to the
existing blanket test. (Counts toward +0 in the test-count delta;
TASK-0368/0369 will also extend the list.)

**Target file:** `protocol/types.rs::tests`
(inside `test_all_variants_serde_roundtrip`).

**Change:** Append two entries:
```rust
Message::InitialPartition { round: 0, partition: make_test_partition() },
Message::FinalStateResult { round: 0, partition: make_test_partition() },
```

**Assertions:**
- Blanket test continues passing for all variants.

**SPEC-19 R covered:** R34 (as blanket coverage).

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| R31 — `InitialPartition` variant at disc 7 | T1 (variant dispatch succeeds) |
| R31 — round=0 for InitialPartition (R21.1) | T1 |
| R32 — `FinalStateResult` variant at disc 11 | T2, T4 (variant dispatch succeeds) |
| R34 — serde + bincode v2 round-trip identity | T1, T2, T4, T5 (blanket) |
| R34 — CRC32C integrity | DEFERRED to TEST-SPEC-0370 (frame-layer) |
| R35 — CompactSubnet + varint | T3 (size-monotone scaling sanity); stronger `frame_len < bincode_len` assertion in TEST-SPEC-0370 |
| R37 — discriminant stability (variants appended) | DEFERRED to TEST-SPEC-0371 (byte-level test) |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0367-A | `round: u32::MAX` on `FinalStateResult` | Varint edge — 5-byte encoding; does it round-trip? (T2 covers round=42; adversarial extension) |
| QA-0367-B | Partition with 0 live agents (empty net) | Does the blanket bincode path encode empty `Vec<Option<Agent>>` as 0-length and recover it? T1's `make_test_partition` may or may not hit this — add empty-partition case |
| QA-0367-C | Partition with a `Net` carrying an erasure redex (`Era`) | Does `CompactSubnet` handle `Era` symbol correctly under the delta path? Regression on SPEC-04 TASK-0344 |
| QA-0367-D | Two `InitialPartition` messages with identical agent count but different wire topologies | Does bincode round-trip preserve wire structure? (The blanket `Debug`-format assertion catches this) |
| QA-0367-E | Ordering drift in a future refactor: append `InitialPartition` AFTER other variants | Compiles, but byte-level discriminant test (TEST-SPEC-0371) fires |
| QA-0367-F | `round: u32` accidentally widened to `u64` | Type-level compile break — but T1 enforces the `u32` literal |
| QA-0367-G | `Partition` derive cascade breaks when a sub-field loses serde | Round-trip decode fails with a clear serde error; T1/T2/T4 fire |
| QA-0367-H | Extreme payload (1M agents) — does the encoded size overflow `DEFAULT_MAX_PAYLOAD_SIZE`? | Wire-layer concern; TEST-SPEC-0370 covers the frame path; not this TEST-SPEC |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 969 → **973** (+4 new
   `#[test]` fns).
2. `cargo test --workspace --lib --features zero-copy` count: 1009 →
   **1013** (+4).
3. `cargo build --workspace` clean, default features.
4. `cargo build --workspace --features zero-copy` clean.
5. `cargo clippy --workspace --all-targets -- -D warnings` clean both
   ways.
6. `cargo fmt --check` clean.
7. `cargo doc --workspace --no-deps` clean (new `///` docs on the
   variants render without broken intra-doc links).

---

## Out of scope (deferred to later TEST-SPECs in the bundle)

- `RoundStart` + `FinalStateRequest` bincode round-trips → TEST-SPEC-0368.
- `RoundResult` bincode round-trip + DC-A2 activity invariant → TEST-SPEC-0369.
- CRC32 integrity, `FLAG_COMPRESSED` behavior, R35 benefit assertion → TEST-SPEC-0370.
- Byte-level discriminant stability → TEST-SPEC-0371.
