# TEST-SPEC-0345: Frame header v2 (9 bytes + flags)

**Task:** TASK-0345
**Spec:** SPEC-18 R14, R15, R16, R17, R18, R19, R35 partial (item 2.23, §3.4)
**Generated:** 2026-04-16
**Baseline before this task:** 805+ (post-TASK-0344)

---

## Scope note

`FrameHeader` grows from 8 → 9 bytes by appending a `flags: u8` field.
Two flag bits are reserved (`FLAG_COMPRESSED = 0b01`, `FLAG_ARCHIVED =
0b10`); the other 6 are reserved for future use and must be rejected
when set on input (forward-compat hardening).

This task **does not** set any flag bits — `send_frame` always writes
`flags = 0`. The compression bit is wired in TASK-0346; the archive bit
is deferred to ROADMAP item 2.24.

---

## R1: header size constant updated

```rust
#[test]
fn frame_header_size_is_nine() {
    assert_eq!(FRAME_HEADER_SIZE, 9);
}
```

## R2: flag bit constants defined

```rust
#[test]
fn frame_flag_constants_are_correct() {
    assert_eq!(FLAG_COMPRESSED, 0b0000_0001);
    assert_eq!(FLAG_ARCHIVED,   0b0000_0010);
    assert_eq!(FLAG_RESERVED,   0b1111_1100);
    // Mutually exclusive partition: the three constants together cover
    // exactly 0xFF (every bit accounted for).
    assert_eq!(FLAG_COMPRESSED | FLAG_ARCHIVED | FLAG_RESERVED, 0xFF);
}
```

## R3: round-trip with flags=0 succeeds

```rust
#[tokio::test]
async fn frame_v2_roundtrip_no_flags() {
    let mut buf: Vec<u8> = Vec::new();
    let msg = Message::Heartbeat;  // smallest variant
    send_frame(&mut buf, &msg).await.unwrap();
    assert_eq!(buf.len() >= FRAME_HEADER_SIZE, true);
    assert_eq!(buf[8], 0, "flags byte must be 0 by default");

    let mut cur = std::io::Cursor::new(buf);
    let back = recv_frame(&mut cur).await.unwrap();
    assert_eq!(back, msg);
}
```

## R4: helpers `is_compressed()` / `is_archived()` / `has_unknown_flags()`

```rust
#[test]
fn frame_header_flag_helpers() {
    let h = FrameHeader { length: 0, checksum: 0, flags: 0b00 };
    assert!(!h.is_compressed());
    assert!(!h.is_archived());
    assert!(!h.has_unknown_flags());

    let h = FrameHeader { length: 0, checksum: 0, flags: 0b01 };
    assert!(h.is_compressed());
    assert!(!h.is_archived());
    assert!(!h.has_unknown_flags());

    let h = FrameHeader { length: 0, checksum: 0, flags: 0b10 };
    assert!(!h.is_compressed());
    assert!(h.is_archived());
    assert!(!h.has_unknown_flags());

    let h = FrameHeader { length: 0, checksum: 0, flags: 0b11 };
    assert!(h.is_compressed());
    assert!(h.is_archived());
    assert!(!h.has_unknown_flags());

    let h = FrameHeader { length: 0, checksum: 0, flags: 0b0000_0100 };
    assert!(h.has_unknown_flags(), "reserved bit 2 must trigger unknown");
}
```

## R5: receiver rejects unknown flag bits (R19)

The receiver must reject any frame whose `flags & FLAG_RESERVED != 0`
with `ProtocolError::UnknownFlags { flags }`.

```rust
#[tokio::test]
async fn frame_v2_unknown_flag_bit_rejected() {
    // Hand-craft a 9-byte header with reserved bit set.
    let length: u32 = 0;
    let checksum: u32 = 0;  // CRC32C of empty payload is 0
    let flags: u8 = 0b0000_0100;  // reserved bit 2

    let mut buf: Vec<u8> = Vec::new();
    buf.extend_from_slice(&length.to_le_bytes());
    buf.extend_from_slice(&checksum.to_le_bytes());
    buf.push(flags);
    // payload of length 0 — nothing to append

    let mut cur = std::io::Cursor::new(buf);
    let err = recv_frame(&mut cur).await.unwrap_err();
    assert!(
        matches!(err, ProtocolError::UnknownFlags { flags: 0b0000_0100 }),
        "expected UnknownFlags(0b0000_0100), got {:?}",
        err,
    );
}
```

## R6: `ProtocolError::UnknownFlags` Display formatting

```rust
#[test]
fn protocol_error_unknown_flags_renders() {
    let e = ProtocolError::UnknownFlags { flags: 0b1010_0100 };
    let s = e.to_string();
    assert!(s.contains("unknown frame flags"), "got: {}", s);
    assert!(s.contains("0b10100100"), "binary repr expected, got: {}", s);
}
```

## R7: `bytes_sent_per_round` reflects 9-byte header

This is a regression check: any test that previously asserted a
specific byte count for a single frame's wire size must increase by
exactly 1 byte (the new flags byte).

```rust
#[tokio::test]
async fn bytes_sent_per_round_includes_new_header_byte() {
    let mut buf: Vec<u8> = Vec::new();
    send_frame(&mut buf, &Message::Heartbeat).await.unwrap();
    // Heartbeat payload after bincode v2 + LZ4-disabled is small;
    // the *minimum* on the wire is FRAME_HEADER_SIZE + payload_len.
    assert!(buf.len() >= FRAME_HEADER_SIZE);
}
```

## R8: existing 8-byte assertions are updated, not deleted

Any test in the codebase asserting `header.len() == 8` or comparable
must be updated to 9 and kept. Static check via grep:

```bash
# After TASK-0345, no test should assert == 8 for header size.
rg -n '== 8.*HEADER' relativist-core/src relativist-core/tests
rg -n 'FRAME_HEADER_SIZE.*= 8' relativist-core/src relativist-core/tests
```

Both must return zero matches.

## Acceptance

1. `cargo test --workspace` count: 805 → **809+** (≥ +4 new tests
   covering R1, R3, R4/R5, R6).
2. All existing tests still pass (header-size assertions updated, not
   removed).
3. `cargo clippy --workspace --all-targets -- -D warnings` clean.
4. `cargo fmt --check` clean.
5. Wire-incompatibility note: between TASK-0345 and TASK-0347 commits,
   intermediate v2-development builds are not interoperable with each
   other — accepted per task spec.

## Out of Scope

- Setting `FLAG_COMPRESSED` (TEST-SPEC-0346).
- Setting `FLAG_ARCHIVED` (deferred to 2.24).
- PROTOCOL_VERSION bump (TEST-SPEC-0347).
