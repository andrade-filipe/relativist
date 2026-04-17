# TEST-SPEC-0343: bincode v2 migration

**Task:** TASK-0343
**Spec:** SPEC-18 R1, R2, R3, R4 (item 2.23, §3.1)
**Generated:** 2026-04-16
**Baseline before this task:** 801 passing tests

---

## Scope note

This is a mechanical refactor: bincode v1 → bincode v2. Semantic
neutrality is the contract — every value `v` such that
`decode(encode(v)) == v` held under v1 must still hold under v2.

What changes observably:
- Encoded **byte counts shrink** because v2's `config::standard()` uses
  varint by default (where v1 used fixint).
- Call-site signatures change (`serialize` → `encode_to_vec`,
  `deserialize` → `decode_from_slice`).

What does NOT change:
- Decoded values.
- Wire compatibility *within* v2 (the version bump is TASK-0347).
- Any non-bincode serde codepath (JSON for codec input, etc.).

---

## R1: every `bincode::serialize` / `bincode::deserialize` call site is migrated

**Test type:** static — verified by grep.

```bash
# After migration, both must return zero matches in src/, tests/, benches/:
rg -n 'bincode::serialize\b' relativist-core/src relativist-core/tests
rg -n 'bincode::deserialize\b' relativist-core/src relativist-core/tests
rg -n 'bincode::serialized_size\b' relativist-core/src relativist-core/tests
```

The CI sanity check is captured implicitly by `cargo build --workspace`
succeeding — v1 functions no longer compile against bincode v2.

## R2: round-trip identity holds for `Net` after migration

```rust
#[test]
fn bincode_v2_net_round_trip() {
    let net = build_net_for_tests();  // existing helper
    let bytes = bincode::serde::encode_to_vec(&net, bincode::config::standard()).unwrap();
    let (back, n): (Net, usize) =
        bincode::serde::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
    assert_eq!(n, bytes.len(), "all bytes consumed");
    assert_eq!(back, net);
}
```

## R3: round-trip identity holds for `Partition`

```rust
#[test]
fn bincode_v2_partition_round_trip() {
    let p = build_partition_for_tests();
    let bytes = bincode::serde::encode_to_vec(&p, bincode::config::standard()).unwrap();
    let (back, n): (Partition, usize) =
        bincode::serde::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
    assert_eq!(n, bytes.len());
    assert_eq!(back, p);
}
```

## R4: round-trip identity holds for every `Message` variant

This is the broadest contract. For each variant of the existing
`protocol::Message` enum (Register, RegisterAck, RegisterNack,
AssignPartition, PartitionResult, BorderUpdate, Heartbeat, Shutdown,
…), build a representative value and round-trip it.

```rust
#[test]
fn bincode_v2_message_round_trip_all_variants() {
    let cfg = bincode::config::standard();
    for msg in sample_all_message_variants() {
        let bytes = bincode::serde::encode_to_vec(&msg, cfg).unwrap();
        let (back, n): (Message, usize) =
            bincode::serde::decode_from_slice(&bytes, cfg).unwrap();
        assert_eq!(n, bytes.len(), "variant {:?} not fully consumed", msg);
        assert_eq!(back, msg, "variant {:?} round-trip mismatch", msg);
    }
}
```

If `sample_all_message_variants()` does not exist as a helper, build it
inline — exhaustive `match` to make the compiler reject any missing
variant when new ones are added.

## R-byte-count: shrink expectation is recorded, not asserted

bincode v2 shrinks payloads. The exact post-migration sizes are not
known in advance and are not part of the public contract — but tests
that previously asserted exact v1 byte counts must be updated to v2
counts (not deleted). Those updated tests serve as a regression net
against accidental encoding-config changes.

Files known to have such assertions (per SPEC-18 §6.1):
- `src/protocol/types.rs`
- `src/partition/types.rs`
- `src/partition/compact.rs`
- `src/net/types.rs`
- `src/protocol/frame.rs`

Each updated test must keep its name and structure; only the expected
byte-count constant changes.

## R-frame-still-roundtrips: end-to-end frame still works

Smoke test that `send_frame` + `recv_frame` (bincode v2 inside, v1
8-byte header still in place — header changes in TASK-0345) round-trip
every variant:

```rust
#[tokio::test]
async fn frame_roundtrip_post_bincode_v2() {
    for msg in sample_all_message_variants() {
        let mut buf: Vec<u8> = Vec::new();
        send_frame(&mut buf, &msg).await.unwrap();
        let mut cur = std::io::Cursor::new(buf);
        let back = recv_frame(&mut cur).await.unwrap();
        assert_eq!(back, msg);
    }
}
```

## Acceptance

1. `cargo build --workspace` succeeds.
2. `cargo test --workspace` count: 801 → **801+** (zero regression; new
   `bincode_v2_*` tests add coverage; updated byte-count tests do not
   change the count).
3. `cargo clippy --workspace --all-targets -- -D warnings` clean.
4. `cargo fmt --check` clean.
5. `rg 'bincode::serialize\b'`, `rg 'bincode::deserialize\b'`,
   `rg 'bincode::serialized_size\b'` all return zero hits in
   `relativist-core/`.
6. Manual smoke: `target/release/relativist compute add 3 5` still
   prints `Result: 8` (sequential mode unaffected).

## Out of Scope

- Custom PortRef compact encoding (TEST-SPEC-0344).
- Frame header changes (TEST-SPEC-0345).
- LZ4 (TEST-SPEC-0346).
- Version bump (TEST-SPEC-0347).
