# TEST-SPEC-0344: Compact PortRef serde encoding

**Task:** TASK-0344
**Spec:** SPEC-18 R5, R6, R7, R8 (item 2.23, §3.2)
**Generated:** 2026-04-16
**Baseline before this task:** 801+ (post-TASK-0343)

---

## Scope note

After TASK-0343, every `PortRef` is encoded by bincode v2's default
varint. This task **further** shrinks the most-common variants by
applying a custom serde adapter (`portref_compact`) that:

```
0xFF                        -> DISCONNECTED          (1 byte)
0x00 + varint(id) + u8(pid) -> AgentPort(id, pid)    (3-7 bytes typical)
0x01 + varint(border)       -> FreePort(border)      (2-6 bytes)
```

Tests verify (a) every variant round-trips, (b) the high-frequency
shapes meet the size budget, and (c) malformed payloads are rejected.

---

## R1: round-trip identity for every variant in the SPEC-18 §7.1 T2 list

```rust
#[test]
fn portref_compact_round_trip_t2_set() {
    let cfg = bincode::config::standard();
    let cases = [
        PortRef::AgentPort(AgentId(0), 0),
        PortRef::AgentPort(AgentId(16383), 2),
        PortRef::AgentPort(AgentId(16384), 1),
        PortRef::AgentPort(AgentId(u32::MAX - 1), 2),
        PortRef::FreePort(0),
        PortRef::FreePort(1000),
        PortRef::FreePort(u32::MAX),  // DISCONNECTED sentinel
    ];
    for p in cases {
        let bytes = bincode::serde::encode_to_vec(&p, cfg).unwrap();
        let (back, n): (PortRef, usize) =
            bincode::serde::decode_from_slice(&bytes, cfg).unwrap();
        assert_eq!(n, bytes.len(), "{:?}: bytes not fully consumed", p);
        assert_eq!(back, p, "{:?}: round-trip mismatch", p);
    }
}
```

## R2: size budget — DISCONNECTED is exactly 1 byte (R8 hot path)

```rust
#[test]
fn portref_compact_disconnected_is_one_byte() {
    let cfg = bincode::config::standard();
    let bytes = bincode::serde::encode_to_vec(
        &PortRef::FreePort(u32::MAX),
        cfg,
    ).unwrap();
    assert_eq!(bytes.len(), 1, "DISCONNECTED must collapse to 1 byte");
    assert_eq!(bytes[0], 0xFF);
}
```

## R3: size budget — small AgentPort fits in ≤ 4 bytes

```rust
#[test]
fn portref_compact_small_agent_port_le_4_bytes() {
    let cfg = bincode::config::standard();
    let bytes = bincode::serde::encode_to_vec(
        &PortRef::AgentPort(AgentId(100), 0),
        cfg,
    ).unwrap();
    assert!(
        bytes.len() <= 4,
        "AgentPort(100,0) compact encoding was {} bytes (expected ≤ 4)",
        bytes.len(),
    );
}
```

## R4: size budget — AgentPort(0,0) is exactly 3 bytes

```rust
#[test]
fn portref_compact_zero_agent_port_is_three_bytes() {
    let cfg = bincode::config::standard();
    let bytes = bincode::serde::encode_to_vec(
        &PortRef::AgentPort(AgentId(0), 0),
        cfg,
    ).unwrap();
    assert_eq!(bytes.len(), 3, "AgentPort(0,0) tag+varint+pid = 3 bytes");
    assert_eq!(bytes[0], 0x00, "tag for AgentPort");
}
```

## R5: malformed payload — unknown tag rejected

```rust
#[test]
fn portref_compact_unknown_tag_is_error() {
    let cfg = bincode::config::standard();
    let res: Result<(PortRef, usize), _> =
        bincode::serde::decode_from_slice(&[0x42, 0x00], cfg);
    assert!(res.is_err(), "tag 0x42 must be rejected");
}
```

## R6: malformed payload — truncated varint rejected

```rust
#[test]
fn portref_compact_truncated_varint_is_error() {
    let cfg = bincode::config::standard();
    // 0x00 = AgentPort tag, then a continuation byte but no terminator.
    let bytes = [0x00, 0x80];
    let res: Result<(PortRef, usize), _> =
        bincode::serde::decode_from_slice(&bytes, cfg);
    assert!(res.is_err(), "truncated varint must produce a serde error");
}
```

## R7: composition with `CompactSubnet` (SPEC-18 §8 Q3)

`CompactSubnet`'s existing serde adapter must continue to work with the
new PortRef encoding inside it. The simplest test is to round-trip a
realistic `Partition` (which contains `Net` which contains `PortRef`)
through bincode v2 and assert structural equality.

```rust
#[test]
fn compact_subnet_composes_with_portref_compact() {
    let cfg = bincode::config::standard();
    let p = build_partition_for_tests();  // contains AgentPort + FreePort + DISCONNECTED
    let bytes = bincode::serde::encode_to_vec(&p, cfg).unwrap();
    let (back, n): (Partition, usize) =
        bincode::serde::decode_from_slice(&bytes, cfg).unwrap();
    assert_eq!(n, bytes.len());
    assert_eq!(back, p);
}
```

## R8: hot-path size shrink (SHOULD, not enforced — logged in REVIEW)

Measure: encode `build_partition_for_tests()` post-TASK-0343 (record
`baseline_bytes`), then post-TASK-0344 (record `compact_bytes`). The
REVIEWER notes the ratio. We expect ≥ 30% shrink for typical
partitions, but this is documented in REVIEW, not asserted, because
exact ratios depend on the test fixture.

```rust
// Optional development-time check; gate behind ignore so CI does not
// enforce a size that may change with fixture tweaks.
#[test]
#[ignore = "size measurement, run manually"]
fn portref_compact_hot_path_shrink() {
    let cfg = bincode::config::standard();
    let p = build_partition_for_tests();
    let bytes = bincode::serde::encode_to_vec(&p, cfg).unwrap();
    eprintln!("partition bytes (compact PortRef + bincode v2): {}", bytes.len());
}
```

## Acceptance

1. `cargo test --workspace` count: 801 → **805+** (R1 counts as 1
   parameterized test; R2/R3/R4/R5/R6/R7 each count as 1).
2. All previously passing tests continue to pass.
3. `cargo clippy --workspace --all-targets -- -D warnings` clean.
4. `cargo fmt --check` clean.
5. `rg 'serde.*portref_compact' relativist-core/src/net/types.rs` finds
   the field annotation, confirming the adapter is wired in (not just
   defined).

## Out of Scope

- Frame header changes (TASK-0345).
- Compression (TASK-0346).
- Version bump (TASK-0347).
- rkyv (deferred to 2.24).
- JSON shape compatibility for `PortRef` (custom encoding is wire-only).
