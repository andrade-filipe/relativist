# TEST-SPEC-0370: Wire integration — `send_frame_with_threshold` / `recv_frame` round-trip, CRC32, compression thresholds for all 5 delta variants

**Task:** TASK-0370
**Spec:** SPEC-19 §3.4 (R34 CRC32 integrity on v2 wire, R35 compression
  engaged + beneficial for large variants, R36 compression skipped
  below threshold for small variants).
**Spec-critic verdict:** Bonus-1 (R35 "benefit" made observable via
  `frame_len < bincode_len` assertion); Bonus-3 (R34 `FLAG_ARCHIVED`
  explicitly UNSET for delta variants per SPEC-18 R22 whitelist).
**Generated:** 2026-04-17
**Baseline before this task:** 982 lib (default) / 1022 lib
  (`--features zero-copy`) — post TASK-0369 per DC-A1+DC-A2 amended
  trajectory.
**Cumulative target after this task:** 990 lib / 1030 lib — **+8** new
  `#[tokio::test]` fns.

---

## Scope note

This TEST-SPEC exercises the FRAME LAYER (SPEC-18) for the 5 new
delta-protocol variants. Bincode-layer correctness is covered in
TEST-SPEC-0367/0368/0369.

Three SPEC-19 §3.4 requirements are observable at the wire:
- **R34** — CRC32C integrity: `recv_frame` rejects tampered payloads
  with a recognisable error.
- **R35** — large-payload variants (`InitialPartition`,
  `FinalStateResult`) benefit from LZ4 compression when encoded size
  ≥ `DEFAULT_COMPRESSION_THRESHOLD` (1024 bytes). Per Bonus-1, the
  testable "benefit" criterion is `frame_len(compressed) <
  frame_len(uncompressed_via_bincode)`.
- **R36** — small-payload variants (`RoundStart`, `RoundResult`,
  `FinalStateRequest`) MUST skip compression when encoded size <
  threshold (`FLAG_COMPRESSED` UNSET in the frame header), but MAY
  compress if the threshold is forced low (spec says SHOULD, not
  MUST-NOT).

Per Bonus-3 and SPEC-18 R22: `FLAG_ARCHIVED` MUST be UNSET for all 5
new variants (rkyv fast path is whitelisted to `AssignPartition` /
`PartitionResult` only; delta variants ride the plain bincode path).

All tests use the `tokio::io::duplex(64 * 1024)` in-memory pair
pattern precedent from `zero_copy_tests.rs`. Tests are
`#[tokio::test]` (async). `send_frame_with_threshold` (not
`send_frame_v2`) is used throughout — the v2 rkyv/archive path is
SPEC-18 R22 out of scope.

---

## Test target file paths

- `relativist-core/src/protocol/delta_wire_tests.rs` — **NEW FILE**
  (≈180 LoC). Contains:
  - `make_large_partition(n_agents: usize) -> Partition` fixture.
  - 8 `#[tokio::test]` fns (T1..T8 below).
  - Inline helper `async fn duplex_pair()` or direct
    `tokio::io::duplex(64 * 1024)` usage at each test site.
- `relativist-core/src/protocol/mod.rs` — **MODIFY** (add
  `#[cfg(test)] mod delta_wire_tests;` line, alongside the existing
  `zero_copy_tests` wiring).

---

## Test fixture

### `make_large_partition(n_agents: usize) -> Partition`

**Purpose:** Produce a partition whose bincode-encoded
`Message::InitialPartition { round: 0, partition }` exceeds 1024 bytes
(the `DEFAULT_COMPRESSION_THRESHOLD`). Parameter `n_agents = 200`
empirically produces > 1024 bytes; developer SHOULD verify via a
`debug_assert!(bincode_v2::encode(&msg).expect("encode").len() > 1024,
...)` during fixture development.

**Fixture shape (developer-facing, illustrative — not test code):**
```rust
#[cfg(test)]
fn make_large_partition(n_agents: usize) -> Partition {
    use crate::net::{Net, Symbol, PortRef};
    use crate::partition::Partition;
    let mut net = Net::new();
    for _ in 0..n_agents {
        let _ = net.add_agent(Symbol::Con); // auxiliary-filled by default
    }
    // Wire some pairs to produce non-trivial serialised output.
    // The precise topology is unimportant; size is.
    Partition {
        worker_id: 0,
        subnet: net,
        free_port_index: Default::default(),
    }
}
```
(Developer adapts to the actual `Partition` constructor / wire helper
shape in the current codebase.)

---

## Unit Tests

### T1: `initial_partition_wire_roundtrip_compressed_and_beneficial`

**Purpose:** R35 for `InitialPartition` — large payload crosses the
compression threshold, `FLAG_COMPRESSED` is SET, `FLAG_ARCHIVED` is
UNSET, payload round-trips, AND the compressed frame is strictly
smaller than the uncompressed bincode encoding (Bonus-1).

**Target file:** `protocol/delta_wire_tests.rs`

**Given:** `Message::InitialPartition { round: 0, partition:
make_large_partition(200) }`. Bincode-encoded size > 1024 bytes.

**When:** `send_frame_with_threshold(&mut client, &msg,
DEFAULT_COMPRESSION_THRESHOLD).await?` → `recv_frame(&mut server, ...)`.

**Then:**
```rust
#[tokio::test]
async fn initial_partition_wire_roundtrip_compressed_and_beneficial() {
    let msg = Message::InitialPartition {
        round: 0,
        partition: make_large_partition(200),
    };
    let uncompressed_len = bincode_v2::encode(&msg)
        .expect("encode")
        .len();
    assert!(uncompressed_len > DEFAULT_COMPRESSION_THRESHOLD,
            "fixture precondition: encoded size must exceed threshold");

    let (mut client, mut server) = tokio::io::duplex(64 * 1024);

    // Send
    send_frame_with_threshold(&mut client, &msg, DEFAULT_COMPRESSION_THRESHOLD)
        .await
        .expect("send");

    // Recv — inspect header flags AND decode payload.
    let received: Message = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
        .await
        .expect("recv");

    // R34: round-trip identity.
    match received {
        Message::InitialPartition { round, partition } => {
            assert_eq!(round, 0);
            assert_eq!(partition.subnet.count_live_agents(), 200);
        }
        other => panic!("expected InitialPartition, got {:?}", other),
    }

    // R35 flag assertions via a second send into a Vec<u8> buffer,
    // inspecting the raw header bytes (because recv_frame unwraps the
    // payload). Test developer pattern is to call send_frame_with_threshold
    // into a Vec<u8> (impl AsyncWriteExt) and read the first
    // FRAME_HEADER_SIZE bytes.
    let mut buf: Vec<u8> = Vec::new();
    send_frame_with_threshold(&mut buf, &msg, DEFAULT_COMPRESSION_THRESHOLD)
        .await
        .expect("send to buf");
    let header = FrameHeader::from_bytes(
        buf[..FRAME_HEADER_SIZE].try_into().expect("header slice")
    );
    assert_ne!(header.flags & FLAG_COMPRESSED, 0,
        "R35: InitialPartition above threshold MUST set FLAG_COMPRESSED");
    assert_eq!(header.flags & FLAG_ARCHIVED, 0,
        "SPEC-18 R22: delta variants MUST NOT set FLAG_ARCHIVED");

    // Bonus-1: R35 "benefit" — compressed frame strictly smaller.
    let frame_len = buf.len();
    assert!(
        frame_len < uncompressed_len,
        "R35 benefit (spec-critic Bonus-1): compressed frame \
         ({} bytes) must be < uncompressed bincode ({} bytes). \
         If LZ4 hit ~98% of original, re-examine \
         DEFAULT_COMPRESSION_THRESHOLD (SPEC-18), not SPEC-19.",
        frame_len, uncompressed_len
    );
}
```

**Assertions:**
- Fixture precondition: `uncompressed_len > 1024`.
- `FLAG_COMPRESSED` is SET on the received header.
- `FLAG_ARCHIVED` is UNSET (SPEC-18 R22 whitelist exclusion).
- Payload round-trips (variant dispatch + `count_live_agents`).
- Compressed frame size < uncompressed bincode size (Bonus-1 benefit
  criterion).

**SPEC-19 R covered:** R34 (round-trip + CRC32 — CRC is enforced by
`recv_frame`'s internal check; implicit in a successful recv), R35
(compression engaged + beneficial).

---

### T2: `final_state_result_wire_roundtrip_compressed_and_beneficial`

**Purpose:** R35 mirror-test for the W→C large-payload variant.

**Target file:** `protocol/delta_wire_tests.rs`

**Given:** `Message::FinalStateResult { round: 42, partition:
make_large_partition(200) }`.

**When:** `send_frame_with_threshold` → `recv_frame`.

**Then:** Same pattern as T1 — assert `FLAG_COMPRESSED` SET,
`FLAG_ARCHIVED` UNSET, round-trip identity (`round == 42`,
`count_live_agents == 200`), and Bonus-1 `frame_len <
uncompressed_len`.

**Assertions:** (same list as T1, adjusted for `FinalStateResult`
variant).

**SPEC-19 R covered:** R34, R35 (symmetric to T1).

---

### T3: `round_start_wire_roundtrip_skips_compression_below_threshold`

**Purpose:** R36 for `RoundStart` — small payload, `FLAG_COMPRESSED`
MUST be UNSET.

**Target file:** `protocol/delta_wire_tests.rs`

**Given:**
- `border_deltas = vec![BorderDelta { border_id: 5, new_target: PortRef::AgentPort(3, 0) }, BorderDelta { border_id: 7, new_target: PortRef::FreePort(9) }, BorderDelta { border_id: 8, new_target: PortRef::FreePort(u32::MAX) }]`
- `resolved_borders = vec![1, 3]`
- `new_borders = vec![(20, PortRef::FreePort(20)), (21, PortRef::FreePort(21))]`
- `round = 3`

**When:** `send_frame_with_threshold(..., DEFAULT_COMPRESSION_THRESHOLD)`.

**Then:**
```rust
#[tokio::test]
async fn round_start_wire_roundtrip_skips_compression_below_threshold() {
    let msg = Message::RoundStart {
        round: 3,
        border_deltas: vec![
            BorderDelta { border_id: 5, new_target: PortRef::AgentPort(3, 0) },
            BorderDelta { border_id: 7, new_target: PortRef::FreePort(9) },
            BorderDelta { border_id: 8, new_target: PortRef::FreePort(u32::MAX) },
        ],
        resolved_borders: vec![1, 3],
        new_borders: vec![
            (20, PortRef::FreePort(20)),
            (21, PortRef::FreePort(21)),
        ],
    };
    // Fixture precondition: encoded size WELL under threshold.
    let enc_len = bincode_v2::encode(&msg).expect("encode").len();
    assert!(enc_len < 512,
            "fixture: encoded size must be < 512 bytes (got {})", enc_len);

    let mut buf: Vec<u8> = Vec::new();
    send_frame_with_threshold(&mut buf, &msg, DEFAULT_COMPRESSION_THRESHOLD)
        .await
        .expect("send");

    let header = FrameHeader::from_bytes(
        buf[..FRAME_HEADER_SIZE].try_into().expect("header slice")
    );
    assert_eq!(header.flags & FLAG_COMPRESSED, 0,
        "R36: RoundStart below threshold MUST NOT set FLAG_COMPRESSED");
    assert_eq!(header.flags & FLAG_ARCHIVED, 0,
        "SPEC-18 R22: delta variants MUST NOT set FLAG_ARCHIVED");

    // Round-trip via recv_frame for correctness.
    let (mut client, mut server) = tokio::io::duplex(64 * 1024);
    send_frame_with_threshold(&mut client, &msg, DEFAULT_COMPRESSION_THRESHOLD)
        .await
        .expect("send");
    let received: Message = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
        .await
        .expect("recv");
    match received {
        Message::RoundStart { round, border_deltas, resolved_borders, new_borders } => {
            assert_eq!(round, 3);
            assert_eq!(border_deltas.len(), 3);
            assert_eq!(resolved_borders, vec![1, 3]);
            assert_eq!(new_borders.len(), 2);
        }
        other => panic!("expected RoundStart, got {:?}", other),
    }
}
```

**Assertions:**
- Fixture precondition: encoded size < 512 bytes.
- `FLAG_COMPRESSED` UNSET.
- `FLAG_ARCHIVED` UNSET.
- Payload round-trips (field values preserved).

**SPEC-19 R covered:** R36 (compression skip), R34 (round-trip), R22
(FLAG_ARCHIVED exclusion).

---

### T4: `round_result_wire_roundtrip_skips_compression_below_threshold`

**Purpose:** R36 for `RoundResult` — small payload, `FLAG_COMPRESSED`
UNSET.

**Target file:** `protocol/delta_wire_tests.rs`

**Given:**
- `border_deltas = vec![BorderDelta { border_id: 5, new_target: PortRef::FreePort(6) }]`
- `stats = make_test_stats_with_activity(false)`
- `has_border_activity = false`
- `round = 3`

**When:** `send_frame_with_threshold(..., DEFAULT_COMPRESSION_THRESHOLD)`.

**Then:** Pattern as T3 — assert encoded size < 512, `FLAG_COMPRESSED`
UNSET, `FLAG_ARCHIVED` UNSET, round-trip preserves all fields
(including both activity flags agreeing at `false`).

**Assertions:**
- Encoded size < 512.
- `FLAG_COMPRESSED` UNSET (R36).
- `FLAG_ARCHIVED` UNSET.
- `round == 3`; `border_deltas.len() == 1`;
  `has_border_activity == false`;
  `stats.has_border_activity == false`.

**SPEC-19 R covered:** R36, R34.

---

### T5: `final_state_request_wire_roundtrip_minimal_frame`

**Purpose:** R36 for the smallest variant. `FinalStateRequest { round
}` is a single-u32 payload; total frame must be small (`< 32` bytes
including header + payload + CRC).

**Target file:** `protocol/delta_wire_tests.rs`

**Given:** `Message::FinalStateRequest { round: 99 }`.

**When:** `send_frame_with_threshold(..., DEFAULT_COMPRESSION_THRESHOLD)`.

**Then:**
```rust
let msg = Message::FinalStateRequest { round: 99 };
let mut buf: Vec<u8> = Vec::new();
send_frame_with_threshold(&mut buf, &msg, DEFAULT_COMPRESSION_THRESHOLD)
    .await
    .expect("send");
assert!(buf.len() < 32,
        "FinalStateRequest total frame size must be < 32 bytes (got {})",
        buf.len());
let header = FrameHeader::from_bytes(
    buf[..FRAME_HEADER_SIZE].try_into().expect("header slice")
);
assert_eq!(header.flags & FLAG_COMPRESSED, 0,
    "R36: FinalStateRequest MUST NOT compress a 1-byte varint");
assert_eq!(header.flags & FLAG_ARCHIVED, 0);

// Recv round-trip.
let (mut client, mut server) = tokio::io::duplex(64 * 1024);
send_frame_with_threshold(&mut client, &msg, DEFAULT_COMPRESSION_THRESHOLD)
    .await
    .expect("send");
let received: Message = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
    .await
    .expect("recv");
match received {
    Message::FinalStateRequest { round } => assert_eq!(round, 99),
    other => panic!("expected FinalStateRequest, got {:?}", other),
}
```

**Assertions:**
- Total frame < 32 bytes.
- `FLAG_COMPRESSED` UNSET.
- `FLAG_ARCHIVED` UNSET.
- Round-trip preserves `round = 99`.

**SPEC-19 R covered:** R36, R34.

---

### T6: `round_start_forced_compression_when_threshold_is_one`

**Purpose:** R36 "SHOULD" wording — the layer IS capable of
compressing a small payload if the threshold is forced low. Asserts
that `FLAG_COMPRESSED` is SET when the threshold is 1, even for a
payload that would otherwise skip compression.

**Target file:** `protocol/delta_wire_tests.rs`

**Given:** Same `RoundStart` fixture as T3.

**When:** `send_frame_with_threshold(..., 1)` — force-compress.

**Then:**
```rust
let msg = /* same RoundStart as T3 */;
let mut buf: Vec<u8> = Vec::new();
send_frame_with_threshold(&mut buf, &msg, 1)
    .await
    .expect("send");
let header = FrameHeader::from_bytes(
    buf[..FRAME_HEADER_SIZE].try_into().expect("header slice")
);
assert_ne!(header.flags & FLAG_COMPRESSED, 0,
    "threshold=1 forces FLAG_COMPRESSED even on small payloads");

// Still round-trips correctly.
let (mut client, mut server) = tokio::io::duplex(64 * 1024);
send_frame_with_threshold(&mut client, &msg, 1)
    .await
    .expect("send");
let received: Message = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
    .await
    .expect("recv");
match received {
    Message::RoundStart { round, .. } => assert_eq!(round, 3),
    other => panic!("expected RoundStart, got {:?}", other),
}
```

**Assertions:**
- `FLAG_COMPRESSED` SET when threshold == 1.
- Payload still decodes correctly (decompress path covers small
  inputs).

**SPEC-19 R covered:** R36 "SHOULD not MUST-NOT" nuance.

---

### T7: `initial_partition_crc_tamper_rejected`

**Purpose:** R34 CRC32C integrity — flipping one byte in the encoded
payload causes `recv_frame` to return an error containing `"checksum"`
or `"CRC"`. Mirrors the SPEC-18 `zero_copy_tests.rs` CRC-tamper
precedent.

**Target file:** `protocol/delta_wire_tests.rs`

**Given:** `Message::InitialPartition { round: 0, partition:
make_large_partition(200) }` — encode into a `Vec<u8>` buffer, flip
one byte in the payload region (post-header, pre-CRC).

**When:** Pipe tampered bytes back into `recv_frame`.

**Then:**
```rust
#[tokio::test]
async fn initial_partition_crc_tamper_rejected() {
    let msg = Message::InitialPartition {
        round: 0,
        partition: make_large_partition(200),
    };
    let mut buf: Vec<u8> = Vec::new();
    send_frame_with_threshold(&mut buf, &msg, DEFAULT_COMPRESSION_THRESHOLD)
        .await
        .expect("send");

    // Flip one byte in the payload region. Skip the first
    // FRAME_HEADER_SIZE bytes (header), avoid the last 4 bytes (CRC
    // trailer location depends on frame layout — match
    // zero_copy_tests.rs precedent for the exact offset; the offset
    // of "clearly in payload" is FRAME_HEADER_SIZE + 8).
    let tamper_off = FRAME_HEADER_SIZE + 8;
    assert!(tamper_off < buf.len() - 4,
            "fixture: tamper offset must be in payload region");
    buf[tamper_off] ^= 0xFF;

    let (mut client, mut server) = tokio::io::duplex(64 * 1024);
    // Write the tampered bytes to the pipe.
    use tokio::io::AsyncWriteExt;
    client.write_all(&buf).await.expect("write tampered frame");
    drop(client);

    let err = recv_frame::<Message, _>(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
        .await
        .expect_err("tampered frame MUST be rejected");

    let err_str = format!("{}", err).to_lowercase();
    assert!(
        err_str.contains("checksum") || err_str.contains("crc"),
        "R34: tampered frame error MUST mention checksum/CRC; got: {}",
        err_str
    );
}
```

**Assertions:**
- `recv_frame` returns `Err` (not `Ok`).
- Error message contains `"checksum"` or `"CRC"` (case-insensitive).
- Exercises R34 end-to-end for the largest new variant.

**SPEC-19 R covered:** R34 (CRC32C integrity).

---

### T8: `final_state_result_crc_still_valid_no_tamper` (positive control)

**Purpose:** Positive control for T7 — an un-tampered
`FinalStateResult` frame MUST decode successfully under the same
infrastructure. Catches a false-positive failure mode where T7 "passes"
because `recv_frame` always errors (for an unrelated reason).

**Target file:** `protocol/delta_wire_tests.rs`

**Given:** `Message::FinalStateResult { round: 42, partition:
make_large_partition(200) }`.

**When:** `send_frame_with_threshold` → `recv_frame` (no tamper).

**Then:**
```rust
#[tokio::test]
async fn final_state_result_crc_still_valid_no_tamper() {
    let msg = Message::FinalStateResult {
        round: 42,
        partition: make_large_partition(200),
    };
    let (mut client, mut server) = tokio::io::duplex(64 * 1024);
    send_frame_with_threshold(&mut client, &msg, DEFAULT_COMPRESSION_THRESHOLD)
        .await
        .expect("send");
    let received: Message = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
        .await
        .expect("recv must succeed without tampering");
    match received {
        Message::FinalStateResult { round, partition } => {
            assert_eq!(round, 42);
            assert_eq!(partition.subnet.count_live_agents(), 200);
        }
        other => panic!("expected FinalStateResult, got {:?}", other),
    }
}
```

**Assertions:**
- `recv_frame` returns `Ok`.
- Payload round-trips correctly.

**SPEC-19 R covered:** R34 (positive-control complement to T7).

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| R34 — CRC32C integrity (positive) | T1, T2, T3, T4, T5, T6, T8 (all successful recv_frame calls exercise the CRC check) |
| R34 — CRC32C integrity (negative / tamper rejection) | T7 |
| R34 — round-trip identity via the full frame layer | T1, T2, T3, T4, T5, T6, T8 |
| R35 — compression engaged for large variants | T1 (InitialPartition), T2 (FinalStateResult) |
| R35 — compression BENEFIT (Bonus-1: frame_len < bincode_len) | T1, T2 |
| R36 — compression skipped below threshold | T3, T4, T5 |
| R36 — "SHOULD not MUST-NOT" (force compression path works) | T6 |
| SPEC-18 R22 — FLAG_ARCHIVED UNSET for delta variants (Bonus-3) | T1, T2, T3, T4, T5 |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0370-A | Truncate the frame mid-payload (e.g. send first 32 bytes only) | `recv_frame` must return a short-read error; not CRC, but a deterministic error (UnexpectedEof or similar) |
| QA-0370-B | Tamper the frame header (flags byte) | Flipping `FLAG_COMPRESSED` causes recv to try decompressing a non-compressed payload — error path may differ from T7 |
| QA-0370-C | Bit-flip in LZ4-compressed region (T7's tamper is post-header but pre-CRC; what if the LZ4 decompress fails first?) | Error propagates as CRC failure OR as LZ4 decompress error; T7 accepts either via the "checksum OR CRC" match. QA should confirm real-world error message |
| QA-0370-D | Varint overflow — manually construct a payload with a 10-byte varint for `round` | Decoder error; catch path |
| QA-0370-E | `DEFAULT_MAX_PAYLOAD_SIZE` boundary — send 1 GiB + 1 byte payload | Expected UpperBound error; R34 indirect |
| QA-0370-F | Concurrent sends on shared writer (two tasks calling `send_frame_with_threshold` simultaneously) | Frame interleaving; writer must be `&mut` and hence exclusive — catch-all for data races |
| QA-0370-G | `make_large_partition(200)` happens to compress to exactly `uncompressed_len` (0% benefit) | T1's Bonus-1 assertion fires; real action: revisit SPEC-18 threshold default |
| QA-0370-H | `DEFAULT_COMPRESSION_THRESHOLD` halved (to 512) — does the skip/apply boundary shift as expected? | QA-configurability probe |
| QA-0370-I | LZ4 compression fails catastrophically on a pathological input | send_frame error path — tests don't exercise it; spec-level concern |
| QA-0370-J | Endianness drift: same frame read on a big-endian target | SPEC-06 R4.1 (now SPEC-18 R2) mandates LE; test environment is always LE in CI |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 982 → **990** (+8 new
   `#[tokio::test]` fns).
2. `cargo test --workspace --lib --features zero-copy` count: 1022 →
   **1030** (+8).
3. `cargo build --workspace` clean, default features.
4. `cargo build --workspace --features zero-copy` clean — the tests
   use `send_frame_with_threshold` (feature-agnostic), not
   `send_frame_v2` (which has rkyv-feature-gated paths).
5. `cargo clippy --workspace --all-targets -- -D warnings` clean.
6. `cargo fmt --check` clean.
7. No `unwrap()` in production code; tests use `.expect(...)`.
8. No `println!`; `tracing::debug!` if needed during development;
   remove before commit.

---

## Out of scope (deferred to later TEST-SPECs in the bundle)

- Byte-level discriminant stability → TEST-SPEC-0371.
- Worker-side `RoundResult` builder debug_assert invariant → sub-bundle 2.26-C.
- Coordinator dispatch loop → sub-bundle 2.26-B.
- rkyv FLAG_ARCHIVED path for delta variants → explicitly OUT per SPEC-18 R22.
