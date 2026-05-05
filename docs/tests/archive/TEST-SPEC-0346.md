# TEST-SPEC-0346: LZ4 compression pipeline

**Task:** TASK-0346
**Spec:** SPEC-18 R9, R10, R11, R12, R13, R35 partial, R36, R37, R38, R39 partial (item 2.23, §3.3 + §3.9 + §3.10)
**Generated:** 2026-04-16
**Baseline before this task:** 809+ (post-TASK-0345)

---

## Scope note

LZ4 frame compression is wired into `send_frame` / `recv_frame`:
- Payloads ≥ `tuning.compression_threshold` (default 1024) are
  compressed and the `FLAG_COMPRESSED` bit is set on the header.
- CRC32C is computed on the **uncompressed** payload (R12 — defense in
  depth: a checksum on compressed bytes would catch transport
  corruption but not silent decompression of valid-but-wrong bytes).
- Smaller payloads are sent uncompressed.
- A new `--compression-threshold <BYTES>` CLI flag (R37) overrides the
  default per-process.

A compression-ratio metric is added per round (R39 SHOULD); compression
*time* is deferred to a follow-up.

---

## R1: round-trip — payload above threshold is compressed

```rust
#[tokio::test]
async fn lz4_compresses_above_threshold() {
    let tuning = TransportTuning { compression_threshold: 0, ..default_tuning() };
    let big = build_partition_for_tests();
    let msg = Message::AssignPartition(AssignPartitionPayload {
        partition: big.clone(),
        ..default_assign_payload()
    });

    let mut buf: Vec<u8> = Vec::new();
    send_frame_with_tuning(&mut buf, &msg, &tuning).await.unwrap();

    // Header byte 8 is the flags byte. Bit 0 is FLAG_COMPRESSED.
    assert_ne!(buf[8] & FLAG_COMPRESSED, 0, "compression flag must be set");

    let mut cur = std::io::Cursor::new(buf);
    let back = recv_frame(&mut cur).await.unwrap();
    assert_eq!(back, msg);
}
```

## R2: round-trip — payload below threshold is sent raw

```rust
#[tokio::test]
async fn lz4_skips_below_threshold() {
    let tuning = TransportTuning { compression_threshold: usize::MAX, ..default_tuning() };
    let msg = Message::Heartbeat;

    let mut buf: Vec<u8> = Vec::new();
    send_frame_with_tuning(&mut buf, &msg, &tuning).await.unwrap();
    assert_eq!(buf[8] & FLAG_COMPRESSED, 0, "compression flag must NOT be set");

    let mut cur = std::io::Cursor::new(buf);
    let back = recv_frame(&mut cur).await.unwrap();
    assert_eq!(back, msg);
}
```

## R3: helper round-trip — `compress_payload` + `decompress_payload`

```rust
#[test]
fn lz4_helper_round_trip() {
    let original = b"the quick brown fox ".repeat(64);  // ~1.3 KB
    let compressed = compress_payload(&original);
    let decompressed = decompress_payload(&compressed).unwrap();
    assert_eq!(decompressed, original);
    assert!(compressed.len() < original.len(),
            "expected size reduction, got compressed={} original={}",
            compressed.len(), original.len());
}
```

## R4: compression ratio ≥ 2.0× for high-redundancy partition (R10 SHOULD)

```rust
#[test]
fn lz4_ratio_ge_two_for_redundant_partition() {
    let p = build_partition_with_many_disconnected();  // helper to add
    let cfg = bincode::config::standard();
    let payload = bincode::serde::encode_to_vec(&p, cfg).unwrap();
    let compressed = compress_payload(&payload);
    let ratio = payload.len() as f64 / compressed.len() as f64;
    assert!(ratio >= 2.0,
            "ratio {:.2}× below SHOULD threshold (uncompressed={}, compressed={})",
            ratio, payload.len(), compressed.len());
}
```

## R5: CRC32C is computed on uncompressed payload (R12)

```rust
#[tokio::test]
async fn checksum_is_on_uncompressed_payload() {
    let tuning = TransportTuning { compression_threshold: 0, ..default_tuning() };
    let msg = Message::AssignPartition(/* sufficiently large */);

    let mut buf: Vec<u8> = Vec::new();
    send_frame_with_tuning(&mut buf, &msg, &tuning).await.unwrap();

    // Manually parse the header.
    let length = u32::from_le_bytes(buf[0..4].try_into().unwrap()) as usize;
    let checksum = u32::from_le_bytes(buf[4..8].try_into().unwrap());
    let flags = buf[8];
    assert_ne!(flags & FLAG_COMPRESSED, 0);
    let wire_payload = &buf[FRAME_HEADER_SIZE..FRAME_HEADER_SIZE + length];
    let uncompressed = decompress_payload(wire_payload).unwrap();
    assert_eq!(crc32c::crc32c(&uncompressed), checksum,
               "checksum must match the uncompressed payload (R12)");
}
```

## R6: corrupted compressed payload — DecompressionFailed OR ChecksumMismatch (R13)

The receiver must never silently accept a corrupted compressed
payload. Either the decompressor rejects it (`DecompressionFailed`) or
the post-decompression CRC catches it (`ChecksumMismatch`).

```rust
#[tokio::test]
async fn corrupted_compressed_payload_is_rejected() {
    let tuning = TransportTuning { compression_threshold: 0, ..default_tuning() };
    let msg = Message::AssignPartition(/* sufficiently large */);

    let mut buf: Vec<u8> = Vec::new();
    send_frame_with_tuning(&mut buf, &msg, &tuning).await.unwrap();

    // Corrupt one byte in the compressed payload area.
    let payload_off = FRAME_HEADER_SIZE + 4;  // skip lz4_flex's size prefix
    buf[payload_off] ^= 0xFF;

    let mut cur = std::io::Cursor::new(buf);
    let err = recv_frame(&mut cur).await.unwrap_err();
    assert!(
        matches!(
            err,
            ProtocolError::DecompressionFailed(_) | ProtocolError::ChecksumMismatch { .. },
        ),
        "expected DecompressionFailed or ChecksumMismatch, got {:?}",
        err,
    );
}
```

## R7: bare `ProtocolError::DecompressionFailed` renders correctly

```rust
#[test]
fn decompression_failed_error_renders() {
    let e = ProtocolError::DecompressionFailed("invalid block".into());
    let s = e.to_string();
    assert!(s.contains("LZ4 decompression failed"), "got: {}", s);
    assert!(s.contains("invalid block"), "inner reason missing: {}", s);
}
```

## R8: `--compression-threshold` CLI flag wires into `TransportTuning` (R37)

```rust
#[test]
fn cli_compression_threshold_flag_threads_through() {
    use crate::config::parse_args_for_test;
    let args = ["relativist", "--compression-threshold", "2048", "compute", "add", "1", "2"];
    let cfg = parse_args_for_test(&args).unwrap();
    assert_eq!(cfg.transport_tuning.compression_threshold, 2048);
}
```

If `parse_args_for_test` does not exist, add a minimal helper that
exposes `clap::Command::try_get_matches_from(args)` for tests.

## R9: default threshold is 1024

```rust
#[test]
fn default_compression_threshold_is_1024() {
    assert_eq!(TransportTuning::default().compression_threshold, 1024);
}
```

## R10: `compression_ratio_per_round` populated in `WorkerRoundStats` (R39)

```rust
#[test]
fn worker_round_stats_records_compression_ratio() {
    let mut stats = WorkerRoundStats::default();
    stats.record_compression(uncompressed_bytes: 4096, compressed_bytes: 1024);
    assert_eq!(stats.compression_ratio_per_round.last().copied(), Some(4.0));
}
```

If `record_compression` does not exist with this signature, design it
to fit the existing `WorkerRoundStats` style — the test specifies the
behavior, not the exact API.

## Acceptance

1. `cargo test --workspace` count: 809 → **815+** (≥ +6).
2. All previously passing tests still pass.
3. `cargo clippy --workspace --all-targets -- -D warnings` clean.
4. `cargo fmt --check` clean.
5. Manual smoke (release): `target/release/relativist compute add 3 5`
   still prints `Result: 8`.
6. Optional bench (REVIEW only, not enforced): one `tcp_localhost`
   round with `--compression-threshold 0` produces a positive
   `compression_ratio_per_round` entry.

## Out of Scope

- `compression_time_per_round` / `decompression_time_per_round`
  (follow-up, only if measurable impact justifies the cost of wiring
  them in).
- rkyv archive flag handling (deferred to 2.24, DEFERRED-WORK D-002).
- PROTOCOL_VERSION bump (TEST-SPEC-0347).
