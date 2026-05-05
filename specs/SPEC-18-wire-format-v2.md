# SPEC-18: Wire Format v2

**Status:** Draft — R28 amended per SPEC-22 §3.8 A9 (PROTOCOL_VERSION 2 → 3)
**Depends on:** SPEC-06 (Wire Protocol), SPEC-02 (Net Representation), SPEC-04 (Partitioning), SPEC-17 (Transport Abstraction)
**Amends:** SPEC-06 R4, R5, R6, R7, R11, R14, R15; SPEC-22 §3.8 A9 (R28 — PROTOCOL_VERSION bump 2 → 3 for Net.free_list wire layout)
**ROADMAP items:** 2.23 (Wire Format Compaction), 2.24 (Zero-Copy Archive)
**References:** bincode v2 spec (docs.rs/bincode), lz4_flex crate, rkyv docs (rkyv.org), rust_serialization_benchmark (GitHub)

---

## 1. Purpose

This spec defines the second version of the wire format for communication between the coordinator and workers in Relativist. Wire Format v2 replaces the bincode v1 fixed-int encoding with bincode v2 variable-length integer encoding, introduces a custom compact serialization for `PortRef`, adds optional LZ4 frame compression for large payloads, updates the frame header to carry a compression flag, and provides an optional zero-copy deserialization path via rkyv for the hot-path messages (`AssignPartition`, `PartitionResult`).

The goal is to reduce per-round serialization cost (CPU time) and payload size (bytes on the wire) without changing protocol semantics. All invariants from SPEC-01 are preserved: the wire format is a serialization concern, and round-trip correctness (SPEC-06 R14, SPEC-02 R26) remains the fundamental contract.

This is a **breaking wire change**: v1 workers cannot communicate with a v2 coordinator (or vice versa). The `PROTOCOL_VERSION` constant is bumped from 1 to 2, and the existing Register handshake rejects version mismatches with a clear error message.

---

## 2. Definitions

Terms defined in SPEC-00 (Glossary) and SPEC-06 (Wire Protocol) are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **bincode v2** | Version 2.x of the bincode crate. Uses variable-length integer encoding (varint) by default. API surface differs from v1: `bincode::serde::encode_to_vec` / `bincode::serde::decode_from_slice` replace `bincode::serialize` / `bincode::deserialize`. |
| **Varint** | Variable-length integer encoding where small values use fewer bytes. bincode v2 encodes integers using 1 byte for values 0-250, 3 bytes for values 251-65535, 5 bytes for values up to 2^32-1, and 9 bytes for u64. Enum discriminants use the same encoding. |
| **Compact PortRef** | A custom serde encoding for `PortRef` that uses a 1-byte tag (`0x00` for `AgentPort`, `0x01` for `FreePort`, `0xFF` for `DISCONNECTED` sentinel) followed by varint-encoded payload fields. Reduces the common case (`AgentPort` with small ID) from 9 bytes (v1) to 3-4 bytes. |
| **LZ4 Frame Compression** | Block compression using the LZ4 algorithm (via `lz4_flex` crate) applied to the serialized payload before framing. LZ4 sustains 500-3500 MB/s throughput and achieves 3-10x compression ratios on IC net data (repeated `DISCONNECTED` sentinels, identical `Symbol` tags, regular port triples). |
| **Compression Threshold** | A configurable payload size (in bytes) below which compression is skipped. Default: 1024 bytes (1 KiB). Payloads at or above this threshold are LZ4-compressed; payloads below are sent uncompressed. |
| **rkyv** | A zero-copy serialization framework for Rust. The receiver can access fields of the serialized data directly from the received byte buffer without running a deserialization pass. Used optionally on the hot path (`AssignPartition`, `PartitionResult`) to eliminate deserialization CPU cost. |
| **Archived Type** | An rkyv-generated type (e.g., `ArchivedPartition`) that provides safe accessor methods into a byte buffer without materializing the original Rust type. Requires the buffer to be aligned (typically 16-byte alignment). |
| **Zero-Copy Path** | The code path where the receiver accesses an rkyv archive directly from the receive buffer, without allocating or copying the data into a standard Rust struct. Feature-gated under `#[cfg(feature = "zero-copy")]`. |

---

## 3. Requirements

### 3.1 bincode v2 Migration

**R1.** The wire format MUST use bincode v2 (crate `bincode = "2"`) with the default configuration, which uses varint integer encoding and little-endian byte order. The v1 configuration (`bincode::config::standard().with_little_endian().with_fixed_int_encoding()`) is superseded. **(MUST)**

**R2.** All call sites that use `bincode::serialize(&value)` MUST be migrated to `bincode::serde::encode_to_vec(&value, bincode::config::standard())`. All call sites that use `bincode::deserialize::<T>(&bytes)` MUST be migrated to `bincode::serde::decode_from_slice::<T, _>(&bytes, bincode::config::standard())`. **(MUST)**

**R3.** Enum discriminants under bincode v2 MUST use varint encoding. For the `Message` enum (7 variants in v1, discriminants 0-6), each discriminant encodes as a single byte (values 0-250 use 1 byte in bincode v2 varint). This supersedes SPEC-06 R5's note about 4-byte u32 discriminants. **(MUST)**

**R4.** New `Message` variants MUST continue to be appended at the end of the enum to preserve discriminant stability across protocol versions (SPEC-06 R5 principle). The encoding of discriminants changes (fixed-int to varint), but the ordering contract remains. **(MUST)**

### 3.2 Custom PortRef Serialization

**R5.** The `PortRef` type MUST implement a custom compact serde encoding via `#[serde(with = "portref_compact")]` in `src/net/types.rs`. The compact encoding MUST use the following wire format: **(MUST)**

```
Tag byte (1 byte):
  0x00 = AgentPort
  0x01 = FreePort
  0xFF = DISCONNECTED sentinel (FreePort(u32::MAX))

Payload (variable length, depends on tag):
  AgentPort: varint(AgentId) + u8(PortId)    -- 1-5 bytes + 1 byte
  FreePort:  varint(border_id)               -- 1-5 bytes
  DISCONNECTED: (no payload)                 -- 0 bytes
```

**R6.** The `DISCONNECTED` sentinel (`FreePort(u32::MAX)`) MUST be encoded as a single byte (`0xFF`) with no payload. This is a special case that avoids the 5-byte varint encoding of `u32::MAX`. **(MUST)**

**R7.** The compact encoding MUST satisfy round-trip correctness: for every valid `PortRef` value `p`, `decode(encode(p)) == p`. **(MUST)**

**R8.** The compact encoding SHOULD reduce the serialized size of `AgentPort` with typical agent IDs (< 2^14) from 9 bytes (v1) to 3-4 bytes. **(SHOULD)**

### 3.3 LZ4 Frame Compression

**R9.** The framing layer MUST support optional LZ4 compression of the serialized payload. Compression MUST be applied when the serialized payload size is greater than or equal to the configurable compression threshold (default: 1024 bytes). Payloads below the threshold MUST be sent uncompressed. **(MUST)**

**R10.** The compression algorithm MUST be LZ4 block compression, provided by the `lz4_flex` crate. The choice of LZ4 is justified by its throughput (500-3500 MB/s sustained on a single core) and suitability for IC net data, which contains high redundancy (repeated `DISCONNECTED` sentinels, identical `Symbol` tags, regular port triples). **(MUST)**

**R11.** The compression threshold MUST be configurable via the `TransportTuning` configuration struct (SPEC-17). The field MUST be named `compression_threshold: usize` with a default value of 1024 bytes. **(MUST)**

**R12.** Decompression MUST occur before CRC32C verification. The checksum in the frame header MUST be computed over the **uncompressed** payload, so that the receiver can verify integrity after decompression. This means: sender computes CRC32C of the uncompressed payload, then compresses the payload and writes the compressed bytes; receiver reads the compressed bytes, decompresses, then verifies CRC32C against the uncompressed result. **(MUST)**

> **Rationale:** Computing CRC32C on the uncompressed payload ensures that the checksum validates the logical content, not the compression output. A bit flip in the compressed stream will either (a) cause LZ4 decompression to fail (caught by the decompressor), or (b) produce incorrect decompressed bytes (caught by CRC32C mismatch). This provides defense-in-depth.

**R13.** If LZ4 decompression fails (invalid compressed data), the receiver MUST return `ProtocolError::DecompressionFailed` and reject the frame. **(MUST)**

### 3.4 Frame Header Update

**R14.** The frame header MUST be extended from 8 bytes to 9 bytes. The new format is: **(MUST)**

```
+------------------+------------------+---------+-----------------------------+
| Length (4 bytes)  | CRC32 (4 bytes)  | Flags   | Payload (length bytes)      |
| little-endian u32 | little-endian u32 | (1 byte)| bincode v2 payload          |
+------------------+------------------+---------+-----------------------------+
```

**Total on the wire:** `9 + length` bytes per frame.

**R15.** The `Flags` byte MUST use the following bit layout: **(MUST)**

```
Bit 0 (LSB): Compression flag
  0 = payload is uncompressed
  1 = payload is LZ4-compressed

Bit 1: Archive flag
  0 = payload is bincode-encoded
  1 = payload is rkyv-archived (zero-copy path)

Bits 2-7: Reserved (MUST be 0)
```

**R16.** The `Length` field MUST represent the number of bytes of the payload as transmitted on the wire. When compression is enabled, `Length` is the compressed payload size (the number of bytes the receiver must read from the socket). The uncompressed size is not stored in the header; the receiver decompresses into a dynamically-sized buffer. **(MUST)**

**R17.** The CRC32C checksum MUST be computed over the **uncompressed** payload (R12). The `Flags` byte is NOT included in the checksum. **(MUST)**

**R18.** The `FrameHeader` struct MUST be updated to include the flags field: **(MUST)**

```rust
#[derive(Debug, Clone, Copy)]
pub struct FrameHeader {
    /// Length of the payload in bytes (as transmitted: compressed if applicable).
    pub length: u32,
    /// CRC32C checksum of the uncompressed payload.
    pub checksum: u32,
    /// Flags byte: bit 0 = compressed, bit 1 = archived.
    pub flags: u8,
}

/// Header size in bytes (updated from 8 to 9 for v2).
pub const FRAME_HEADER_SIZE: usize = 9;
```

**R19.** The receiver MUST reject frames where reserved flag bits (bits 2-7) are non-zero, returning `ProtocolError::UnknownFlags`. This ensures forward compatibility: a future v3 that defines new flag bits will be rejected by v2 receivers rather than silently misinterpreted. **(MUST)**

### 3.5 rkyv Zero-Copy Archive (Optional)

**R20.** The rkyv zero-copy path MUST be feature-gated under `#[cfg(feature = "zero-copy")]`. When the feature is not enabled, all serialization uses bincode v2 exclusively. The `zero-copy` feature MUST NOT be a default feature. **(MUST)**

**R21.** When the `zero-copy` feature is enabled, the following types MUST derive rkyv's `Archive`, `rkyv::Serialize`, and `rkyv::Deserialize` traits (in addition to the existing serde derives): `Net`, `Partition`, `CompactSubnet`, `Agent`, `Symbol`, `PortRef`, `IdRange`, `WorkerRoundStats`. **(MUST)**

**R22.** The rkyv archive path MUST be used only for hot-path messages: `AssignPartition` and `PartitionResult`. Control messages (`Shutdown`, `Error`, `Register`, `RegisterAck`, `RegisterNack`) MUST always use bincode v2, because rkyv's alignment padding overhead is not justified for small payloads. **(MUST)**

**R23.** When sending a hot-path message via the rkyv path, the sender MUST: **(MUST)**
1. Serialize the `Partition` (or `CompactSubnet`) using `rkyv::to_bytes::<rkyv::rancor::Error>(&value)`.
2. Wrap the resulting bytes as the payload of a frame with the archive flag (bit 1) set in the `Flags` byte.
3. Apply LZ4 compression to the archived bytes if the payload exceeds the compression threshold (compression and archive flags may both be set).

**R24.** When receiving a frame with the archive flag set, the receiver MUST: **(MUST)**
1. Decompress the payload if the compression flag is also set.
2. Validate the CRC32C checksum against the uncompressed payload.
3. Call `rkyv::access::<ArchivedPartition>(&payload)` (the safe, validating API) to obtain a reference into the buffer. The receiver MUST NOT use `rkyv::access_unchecked` in production code.
4. Use the `ArchivedPartition` reference directly for read access where possible, or call `rkyv::deserialize::<Partition, rkyv::rancor::Error>(&archived)` to materialize a full `Partition` when mutation is required.

**R25.** The receive buffer for rkyv-archived frames MUST be 16-byte aligned. The implementation SHOULD use `rkyv::util::AlignedVec` (or equivalent) instead of `vec![0u8; len]` for the payload read buffer when the archive flag is set. **(MUST for alignment; SHOULD for `AlignedVec`)**

**R26.** If rkyv validation fails (malformed archive, alignment error, out-of-bounds access), the receiver MUST return `ProtocolError::ArchiveValidationFailed` and reject the frame. **(MUST)**

**R27.** The rkyv path MUST satisfy the same round-trip correctness guarantee as bincode: for every valid `Partition` value `p`, `deserialize(access(to_bytes(p))) == p`. Tests MUST verify this property. **(MUST)**

### 3.6 Protocol Version

**R28.** The `PROTOCOL_VERSION` constant MUST be bumped from `2` to `3` upon SPEC-22 landing in the wire-relevant `Net` payload. v2 deserializers MUST reject v3 nets with `UnsupportedVersion`. The wire break is justified by the `free_list` field addition; v1/v2 binaries cannot deserialize v3 nets without producing length-mismatch errors. Migration path documented in SPEC-22 §6. v3 deserializers MAY ALSO reject v2 nets, OR MAY tolerate them as nets with an empty `free_list` (deserializer-defined; document the chosen path in §6 of SPEC-22). Persisted v1/v2 `.bin` files (e.g., `results/locked/v1_local_baseline/`) become unreadable by v3 binaries; this is acceptable because v1 baseline binaries are frozen and not consumed by v2/v3 code paths. **(MUST)**

> **Amendment A9 (SPEC-22 §3.8 A9 / R9a):** The previous "bump from 1 to 2" clause is superseded by the 2 → 3 bump upon SPEC-22 landing. Closes SC-007. See SPEC-22 R9a for the formal statement.

> **Amendment (D-011 Phase A — SPEC-19 R35a / SPEC-22 §3.8 A11):** The `PROTOCOL_VERSION` constant MUST additionally bump from `PREVIOUS_LIVE_VERSION` to `PREVIOUS_LIVE_VERSION + 1` upon SPEC-19 R35a landing. The wire break is justified by the `CompactSubnet` struct gaining a new `free_list: Vec<AgentId>` field positioned after `root` (R35a clause (a)); bincode does not tolerate trailing fields on the decode side, so pre-bump receivers cannot decode post-bump payloads without a length-mismatch error. Defensive `PREVIOUS_LIVE_VERSION + 1` language is mandated (NOT a hardcoded absolute integer) in line with SPEC-06 R3b, SPEC-19 R37, and SPEC-22 R9a precedents. At the time of writing the live constant is `6` (post D-009 + D-010, verified at `relativist-core/src/protocol/coordinator.rs:197`); R35a's bump therefore lands at `7`, but implementers MUST read the current value at amendment time. Pre-bump deserializers MUST reject post-bump `CompactSubnet` payloads via the existing `UnsupportedVersion` reject path (mirroring SPEC-22 R9a's v2-vs-v3 reject pattern). Closes QA-D009-001. See SPEC-19 R35a clause (e) for the formal statement.

**R29.** A v3 coordinator that receives a `Register` message with `protocol_version == 2` (or `1`) MUST respond with `RegisterNack` containing the reason `"protocol version mismatch: expected 3, got <received>"` and close the connection. **(MUST)**

**R30.** A v2 worker that receives a `RegisterAck` or `RegisterNack` from a coordinator whose prior `Register` response does not match the expected protocol flow MUST log an error and terminate. The worker SHOULD include the expected and received protocol versions in the error message for diagnostics. **(MUST for termination; SHOULD for diagnostic message)**

**R31.** There is no backward-compatible bridge between v1 and v2 wire formats. The entire grid deployment MUST run the same protocol version. Mixed v1/v2 deployments are unsupported and rejected at the Register handshake. **(MUST)**

### 3.7 Round-Trip Correctness

**R32.** The fundamental serialization identity MUST hold for Wire Format v2: `decode(encode(msg)) == msg` for every valid `Message` value `msg`, where `encode` is the full send pipeline (bincode v2 serialization + optional LZ4 compression + framing) and `decode` is the full receive pipeline (deframing + optional LZ4 decompression + bincode v2 deserialization). This supersedes and strengthens SPEC-06 R14 for the v2 format. **(MUST)**

**R33.** Round-trip correctness MUST additionally hold through the `CompactSubnet` layer: for every valid `Partition` value `p`, `recv(send(p)) == p`, where the partition's subnet passes through `serialize_subnet_compact` / `deserialize_subnet_compact` (SPEC-04) as part of the bincode v2 serialization. Round-trip correctness MUST additionally hold for `Net.free_list` through the `CompactSubnet` layer (SPEC-19 R35a clauses (a)-(b); SPEC-22 §3.8 A11): `(Net -> CompactSubnet -> Net).free_list == Net.free_list` byte-for-byte, for both empty and populated `free_list` source values. This is verified by extending the `nets_equivalent` test helper at `relativist-core/src/partition/compact.rs:152-158` to compare `free_list` alongside `agents`, `ports`, `redex_queue`, `next_id`, and `root`; the unextended helper would silently mark divergent free-lists as equivalent (the original D-009 defect mode). **(MUST)**

**R34.** Round-trip correctness MUST hold for the custom `PortRef` compact encoding: for every valid `PortRef` value `pr` (including `DISCONNECTED`), `portref_compact::deserialize(portref_compact::serialize(pr)) == pr`. **(MUST)**

### 3.8 Error Types

**R35.** The `ProtocolError` enum MUST be extended with the following variants: **(MUST)**

```rust
/// LZ4 decompression failed (invalid compressed data).
DecompressionFailed(String),

/// rkyv archive validation failed (malformed archive, alignment error).
ArchiveValidationFailed(String),

/// Frame flags contain unknown bits (forward compatibility rejection).
UnknownFlags { flags: u8 },

/// Protocol version mismatch during registration.
VersionMismatch { expected: u8, received: u8 },
```

### 3.9 Configuration

**R36.** The `TransportTuning` struct (SPEC-17) MUST include the following wire format v2 fields: **(MUST)**

```rust
/// Payload size threshold for LZ4 compression (bytes).
/// Payloads >= this size are compressed. Default: 1024.
pub compression_threshold: usize,

/// Whether to use the rkyv zero-copy path for hot-path messages.
/// Only effective when the `zero-copy` feature is enabled.
/// Default: false.
pub use_zero_copy: bool,
```

**R37.** Both fields MUST be configurable via CLI arguments (extending the `--transport-*` flag namespace from SPEC-17). **(MUST)**

### 3.10 Metrics Integration

**R38.** The `bytes_sent_per_round` and `bytes_received_per_round` metrics (SPEC-06 R33) MUST reflect the actual bytes on the wire, i.e., the compressed payload size (if compression was applied) plus the 9-byte header. **(MUST)**

**R39.** When observability is enabled (SPEC-11), the following additional metrics SHOULD be emitted per round: **(SHOULD)**
- `compression_ratio_per_round: Vec<f64>` -- ratio of uncompressed to compressed payload size (1.0 if uncompressed).
- `compression_time_per_round: Vec<Duration>` -- CPU time spent on LZ4 compression.
- `decompression_time_per_round: Vec<Duration>` -- CPU time spent on LZ4 decompression.

### 3.11 Complexity

**R40.** LZ4 compression and decompression MUST be O(n) where n is the payload size. The constant factor MUST be small enough that compression does not become the bottleneck: LZ4 throughput SHOULD exceed 500 MB/s on commodity hardware. **(MUST for O(n); SHOULD for throughput)**

**R41.** The varint encoding/decoding for individual integers MUST be O(1) (bounded by the maximum encoded size of 9 bytes for u64). The custom `PortRef` compact encoding MUST be O(1). **(MUST)**

---

## 4. Design

### 4.1 Serialization Pipeline

The v2 send pipeline processes a `Message` through the following stages:

```
Message
  │
  ├──[hot-path + zero-copy feature enabled]──► rkyv::to_bytes(partition)
  │                                              │
  │                                              ▼
  │                                           raw archive bytes
  │                                              │
  └──[all other messages]──► bincode::serde::encode_to_vec(msg, config)
                               │
                               ▼
                          serialized payload (uncompressed)
                               │
                          ┌────┴────┐
                          │ CRC32C  │ ◄── computed on uncompressed payload
                          └────┬────┘
                               │
                    ┌──────────┴──────────┐
                    │ size >= threshold?   │
                    └──────────┬──────────┘
                       yes     │     no
                        │      │      │
                  ┌─────▼─────┐│ ┌────▼────┐
                  │ LZ4 block ││ │ as-is   │
                  │ compress  ││ │         │
                  └─────┬─────┘│ └────┬────┘
                        │      │      │
                        ▼      │      ▼
                  compressed   │  uncompressed
                  payload      │  payload
                        │      │      │
                        └──────┴──────┘
                               │
                    ┌──────────┴──────────┐
                    │   Frame Header      │
                    │ length | crc | flags │
                    └──────────┬──────────┘
                               │
                               ▼
                          TCP write
```

The v2 receive pipeline is the exact inverse:

```
TCP read
  │
  ▼
Read 9-byte header → extract length, checksum, flags
  │
  ├── Reject if reserved flag bits (2-7) are non-zero
  │
  ▼
Read `length` bytes of payload
  │
  ├──[flags.bit(0) == 1]──► LZ4 decompress → uncompressed payload
  │
  └──[flags.bit(0) == 0]──► payload is already uncompressed
                               │
                               ▼
                          Verify CRC32C(uncompressed) == header.checksum
                               │
                    ┌──────────┴──────────┐
                    │ flags.bit(1) == 1?  │
                    └──────────┬──────────┘
                       yes     │     no
                        │      │      │
                  ┌─────▼──────┐ ┌───▼────────────────────────┐
                  │ rkyv::     │ │ bincode::serde::            │
                  │ access()   │ │ decode_from_slice()         │
                  │ (validate) │ │                             │
                  └─────┬──────┘ └───┬────────────────────────┘
                        │            │
                        ▼            ▼
                  ArchivedPartition   Message
```

### 4.2 Frame Header v2

```
Byte offset:  0       1       2       3       4       5       6       7       8
            ┌───────┬───────┬───────┬───────┬───────┬───────┬───────┬───────┬───────┐
            │         Length (LE u32)        │       CRC32C (LE u32)         │ Flags │
            └───────┴───────┴───────┴───────┴───────┴───────┴───────┴───────┴───────┘

Flags byte layout:
  Bit 0: compressed (LZ4)
  Bit 1: archived (rkyv)
  Bits 2-7: reserved (must be 0)
```

```rust
/// Frame header for Wire Format v2.
#[derive(Debug, Clone, Copy)]
pub struct FrameHeader {
    pub length: u32,
    pub checksum: u32,
    pub flags: u8,
}

pub const FRAME_HEADER_SIZE: usize = 9;

/// Flag bit constants.
pub const FLAG_COMPRESSED: u8 = 0b0000_0001;
pub const FLAG_ARCHIVED:   u8 = 0b0000_0010;
pub const FLAG_RESERVED:   u8 = 0b1111_1100;

impl FrameHeader {
    pub fn is_compressed(&self) -> bool {
        self.flags & FLAG_COMPRESSED != 0
    }

    pub fn is_archived(&self) -> bool {
        self.flags & FLAG_ARCHIVED != 0
    }

    pub fn has_unknown_flags(&self) -> bool {
        self.flags & FLAG_RESERVED != 0
    }
}
```

### 4.3 Compact PortRef Encoding

The `portref_compact` module provides a custom serde serializer/deserializer for `PortRef`:

```rust
/// Custom compact serde encoding for PortRef.
///
/// Wire format:
///   0xFF                        -> DISCONNECTED (FreePort(u32::MAX))
///   0x00 + varint(id) + u8(pid) -> AgentPort(id, pid)
///   0x01 + varint(bid)          -> FreePort(bid) where bid != u32::MAX
///
/// The DISCONNECTED sentinel is the most common PortRef in sparse nets
/// (every None slot's ports are DISCONNECTED), so encoding it as a single
/// byte provides significant savings.
pub mod portref_compact {
    use super::PortRef;
    use serde::{Deserializer, Serializer};

    const TAG_AGENT_PORT: u8 = 0x00;
    const TAG_FREE_PORT: u8 = 0x01;
    const TAG_DISCONNECTED: u8 = 0xFF;

    pub fn serialize<S: Serializer>(
        value: &PortRef,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        // Implementation writes tag byte + varint payload
        // using serde's serialize_tuple or serialize_bytes
        todo!()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<PortRef, D::Error> {
        // Implementation reads tag byte, dispatches on tag,
        // reads varint payload for AgentPort/FreePort
        todo!()
    }
}
```

**Size comparison (typical `AgentPort` with ID < 16384, port 0):**

| Encoding | Tag | AgentId | PortId | Total |
|----------|-----|---------|--------|-------|
| bincode v1 (fixed-int) | 4 bytes | 4 bytes | 1 byte | **9 bytes** |
| bincode v2 (default varint) | 1 byte | 1-2 bytes | 1 byte | **3-4 bytes** |
| Compact PortRef (R5) | 1 byte | 1-2 bytes | 1 byte | **3-4 bytes** |
| Compact DISCONNECTED (R6) | 1 byte | -- | -- | **1 byte** |

**Size comparison (`DISCONNECTED` sentinel):**

| Encoding | Total |
|----------|-------|
| bincode v1 | **8 bytes** (4-byte tag + 4-byte u32::MAX) |
| bincode v2 (default varint) | **6 bytes** (1-byte tag + 5-byte varint for u32::MAX) |
| Compact PortRef (R6) | **1 byte** |

### 4.4 LZ4 Compression Wrapper

```rust
use lz4_flex::{compress_prepend_size, decompress_size_prepended};

/// Compresses a payload using LZ4 block compression.
/// The compressed output is prepended with a 4-byte little-endian
/// uncompressed size (lz4_flex convention) to facilitate decompression.
///
/// Returns the compressed bytes.
pub fn compress_payload(payload: &[u8]) -> Vec<u8> {
    compress_prepend_size(payload)
}

/// Decompresses an LZ4-compressed payload.
///
/// Returns the uncompressed bytes, or an error if decompression fails.
pub fn decompress_payload(
    compressed: &[u8],
) -> Result<Vec<u8>, ProtocolError> {
    decompress_size_prepended(compressed)
        .map_err(|e| ProtocolError::DecompressionFailed(e.to_string()))
}
```

> **Note:** `lz4_flex::compress_prepend_size` prepends a 4-byte little-endian uncompressed size to the compressed block. This is internal to the compressed payload and distinct from the frame header's `Length` field (which stores the compressed size). The receiver calls `decompress_size_prepended` which reads this internal size to allocate the decompression buffer.

### 4.5 Send and Receive Functions (v2)

```rust
/// Serializes, optionally compresses, and sends a message as a v2 frame.
///
/// Returns the total number of bytes written (header + payload).
pub async fn send_frame_v2<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    message: &Message,
    tuning: &TransportTuning,
) -> Result<usize, ProtocolError> {
    // 1. Serialize with bincode v2
    let config = bincode::config::standard();
    let payload = bincode::serde::encode_to_vec(message, config)
        .map_err(|e| ProtocolError::Serialize(e))?;

    // 2. Compute CRC32C on uncompressed payload
    let checksum = crc32fast::hash(&payload);

    // 3. Optionally compress
    let (wire_payload, compressed) = if payload.len() >= tuning.compression_threshold {
        (compress_payload(&payload), true)
    } else {
        (payload, false)
    };

    // 4. Build flags
    let flags = if compressed { FLAG_COMPRESSED } else { 0 };

    // 5. Write header (9 bytes) + payload
    let header = FrameHeader {
        length: wire_payload.len() as u32,
        checksum,
        flags,
    };
    // ... write header bytes + wire_payload ...

    Ok(FRAME_HEADER_SIZE + wire_payload.len())
}

/// Reads a v2 frame, optionally decompresses, and deserializes the message.
///
/// Returns the deserialized message and total bytes read.
pub async fn recv_frame_v2<R: AsyncReadExt + Unpin>(
    reader: &mut R,
    max_payload_size: u32,
) -> Result<(Message, usize), ProtocolError> {
    // 1. Read 9-byte header
    // 2. Reject unknown flags
    // 3. Reject if length > max_payload_size
    // 4. Read `length` bytes
    // 5. Decompress if FLAG_COMPRESSED is set
    // 6. Verify CRC32C on uncompressed payload
    // 7. Deserialize with bincode v2 (or rkyv if FLAG_ARCHIVED)
    todo!()
}
```

### 4.6 rkyv Archive Types

When the `zero-copy` feature is enabled:

```rust
#[cfg(feature = "zero-copy")]
use rkyv::{Archive, rancor};

/// Partition type with rkyv derives for zero-copy deserialization.
/// The serde derives coexist -- bincode v2 is used for control messages,
/// rkyv for hot-path partition messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "zero-copy", derive(Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct Partition {
    #[serde(
        serialize_with = "crate::partition::compact::serialize_subnet_compact",
        deserialize_with = "crate::partition::compact::deserialize_subnet_compact"
    )]
    pub subnet: Net,
    pub worker_id: WorkerId,
    pub free_port_index: HashMap<u32, PortRef>,
    pub id_range: IdRange,
}
```

> **Note on CompactSubnet and rkyv:** The `CompactSubnet` wire/memory separation layer (SPEC-04) is a serde adapter. Under the rkyv path, `CompactSubnet` is NOT used for the archive payload. Instead, rkyv serializes the `Partition` directly (including the full `Net` with arena and port array). This is acceptable because rkyv's zero-copy access eliminates the deserialization cost that `CompactSubnet` was designed to mitigate. The rkyv archive size may be larger than the bincode + `CompactSubnet` payload, but the CPU saving on the receive side compensates. When LZ4 compression is applied on top, the wire size difference is negligible.

### 4.7 Protocol Version and Registration

```rust
/// Wire protocol version.
/// v1: bincode v1, fixed-int, 8-byte frame header.
/// v2: bincode v2, varint, 9-byte frame header, optional LZ4 + rkyv.
/// v3: v2 + Net.free_list field (SPEC-22 §3.8 A9 / R9a). Amendment A9.
pub const PROTOCOL_VERSION: u8 = 3;
```

The Register handshake (SPEC-10) uses this constant. When a v3 coordinator receives `RegisterPayload { protocol_version: 2, .. }` (or `1`), it responds with:

```rust
Message::RegisterNack(RegisterNackPayload {
    reason: format!(
        "protocol version mismatch: expected {}, got {}",
        PROTOCOL_VERSION, payload.protocol_version
    ),
})
```

---

## 5. Rationale

### 5.1 Why bincode v2 over postcard/other codecs

bincode v2 was chosen for continuity: the entire codebase already uses bincode v1 with serde derives on all types. The migration is a crate version bump + API signature changes at ~20 call sites, not a full codec replacement. bincode v2 with varint provides comparable encoding efficiency to postcard (the other leading varint serde codec) while maintaining the `bincode` crate namespace and community familiarity. The `rust_serialization_benchmark` project shows bincode v2 and postcard within 10% of each other on both encode/decode speed and encoded size for typical Rust structs.

### 5.2 Why LZ4 over zstd

LZ4 was chosen for the default compression algorithm because:

1. **Throughput dominates ratio.** For localhost and LAN scenarios (the primary v2 targets), compression throughput matters more than compression ratio. LZ4 sustains 500-3500 MB/s on a single core; zstd-1 (the fastest zstd level) sustains 300-600 MB/s. At 1 GB payloads, the difference is 200-500 ms per frame.

2. **IC net data has high redundancy.** Repeated `DISCONNECTED` sentinels, long runs of identical `Symbol` tags (especially in homogeneous nets like `ep_annihilation_con`), and regular port triples all compress well even with LZ4's simpler algorithm. Typical ratios on IC data are 3-10x for LZ4 vs 5-15x for zstd, but the throughput difference outweighs the ratio difference.

3. **Pure Rust implementation.** `lz4_flex` is a pure-Rust LZ4 implementation with no C dependencies, simplifying cross-compilation and deployment (consistent with Relativist's current dependency profile).

> **Note:** For future WAN deployments (ROADMAP 2.21), zstd may be a better choice due to its superior compression ratio. The `TransportTuning` struct can be extended with a `compression_algorithm` field. This spec does not mandate a single algorithm permanently; it mandates LZ4 as the initial default.

### 5.3 Why rkyv is feature-gated

rkyv adds complexity: alignment requirements on receive buffers, stricter derive constraints (no `#[serde(with = "...")]` composability), and a less inspectable archive format compared to bincode. For most users and all v2 development/testing, bincode v2 + LZ4 provides sufficient improvement. The rkyv path is an optimization for production deployments processing very large partitions (>100 MB) where the deserialization CPU cost is a measurable fraction of round time. Feature-gating keeps the default build simple and the rkyv dependency optional.

### 5.4 Why the header grew by 1 byte

Adding a single flags byte to the frame header (8 -> 9 bytes) is the minimal change that communicates compression and archive mode to the receiver without ambiguity. Alternatives considered:

- **Stealing a bit from the CRC32 field:** Weakens integrity checking. Rejected.
- **Using a magic number prefix per frame:** Adds 2-4 bytes and complicates alignment. Rejected.
- **Encoding the mode in the payload itself:** Requires the receiver to speculatively decompress before knowing the mode. Rejected.

The 1-byte overhead per frame is negligible: for a 1 MB payload, it is 0.0001% of the frame size.

### 5.5 Checksum on uncompressed payload

Computing CRC32C on the uncompressed payload (rather than the compressed payload) provides defense-in-depth: any corruption in the compressed stream will be caught either by the LZ4 decompressor (format error) or by the CRC32C mismatch (logical content error). Computing it on the compressed stream would only catch corruption that survives decompression, which is a strictly weaker guarantee.

---

## 6. Migration Path

### 6.1 bincode v1 to v2

**Step 1: Cargo.toml.** Change `bincode = "1"` to `bincode = "2"`. Both versions cannot coexist in the same crate without renaming. This is a clean replacement.

**Step 2: API migration.** Find all call sites:

| v1 API | v2 API |
|--------|--------|
| `bincode::serialize(&value)` | `bincode::serde::encode_to_vec(&value, bincode::config::standard())` |
| `bincode::deserialize::<T>(&bytes)` | `bincode::serde::decode_from_slice::<T, _>(&bytes, bincode::config::standard())` |
| `bincode::serialized_size(&value)` | `bincode::serde::encode_to_vec(&value, config).len()` (compute after encoding) |

**Affected files (from briefing):**
- `src/protocol/frame.rs` (send_frame, recv_frame)
- `src/net/types.rs` (unit tests)
- `src/partition/types.rs` (unit tests)
- `src/partition/compact.rs` (CompactSubnet tests)
- `src/protocol/types.rs` (unit tests)
- ~20 call sites total across test modules

**Step 3: Verify.** All 690 existing tests must pass after migration. Bincode v2 with `config::standard()` uses varint by default. The encoded sizes will differ from v1, so any test that asserts exact byte counts must be updated.

### 6.2 PROTOCOL_VERSION Bump

**Step 1 (original v1→v2 bump, already done):** Change `pub const PROTOCOL_VERSION: u8 = 1;` to `pub const PROTOCOL_VERSION: u8 = 2;` in `src/protocol/coordinator.rs`.

**Step 1a (SPEC-22 amendment A9 — v2→v3 bump):** Upon SPEC-22 landing (Net.free_list serde layout change), change `pub const PROTOCOL_VERSION: u8 = 2;` to `pub const PROTOCOL_VERSION: u8 = 3;` in `src/protocol/coordinator.rs`. See SPEC-22 R9a, §3.8 A9.

**Step 2:** Update the rejection message in the Register handshake to include both expected and received versions (R29).

**Step 3:** Update `FRAME_HEADER_SIZE` from 8 to 9 in `src/protocol/frame.rs`. Update `send_frame` and `recv_frame` to write/read the 9-byte header.

**Step 4:** Update `bytes_sent_per_round` and `bytes_received_per_round` accounting to use the 9-byte header (R38).

### 6.3 Ordering

The migration MUST follow this order:

1. **bincode v2 migration (R1-R4)** -- standalone, no new features, all tests pass.
2. **Custom PortRef encoding (R5-R8)** -- apply `#[serde(with = "portref_compact")]` on `PortRef`.
3. **Frame header update (R14-R19)** -- extend to 9 bytes, add flags byte (initially always 0).
4. **LZ4 compression (R9-R13)** -- implement compression pipeline, set compression flag.
5. **PROTOCOL_VERSION bump (R28-R31)** -- commit the wire break.
6. **rkyv zero-copy (R20-R27)** -- optional, feature-gated, can ship independently.

Steps 1-5 form a single coherent release. Step 6 is independent and may follow later.

---

## 7. Test Strategy

### 7.1 Unit Tests

**T1. bincode v2 round-trip.** For each type reachable from `Message` (`Net`, `Partition`, `Agent`, `Symbol`, `PortRef`, `IdRange`, `WorkerRoundStats`, `RegisterPayload`, `RegisterAckPayload`, `RegisterNackPayload`), verify that `decode(encode(value)) == value` using bincode v2 with `config::standard()`.

**T2. Compact PortRef round-trip.** For every `PortRef` variant:
- `AgentPort(0, 0)` -- minimal values
- `AgentPort(16383, 2)` -- largest 2-byte varint AgentId
- `AgentPort(16384, 1)` -- first 3-byte varint AgentId
- `AgentPort(u32::MAX - 1, 2)` -- near-maximum AgentId
- `FreePort(0)` -- minimal border ID
- `FreePort(1000)` -- typical border ID
- `FreePort(u32::MAX)` -- DISCONNECTED sentinel

Verify `portref_compact::deserialize(portref_compact::serialize(pr)) == pr` for each.

**T3. Compact PortRef size.** Assert that:
- `AgentPort(100, 0)` encodes to <= 4 bytes.
- `DISCONNECTED` encodes to exactly 1 byte.
- `AgentPort(0, 0)` encodes to exactly 3 bytes (1 tag + 1 varint + 1 port).

**T4. LZ4 round-trip.** For a realistic `Partition` built from `build_partition_for_tests()`:
- Serialize with bincode v2.
- Compress with LZ4.
- Decompress.
- Verify decompressed bytes == original serialized bytes.

**T5. LZ4 compression ratio.** For the L6 test cases (large nets with many `DISCONNECTED` sentinels), measure the compression ratio and assert it is >= 2.0x.

**T6. Frame v2 round-trip.** For each `Message` variant, send through `send_frame_v2` and receive through `recv_frame_v2` (using an in-memory transport or `tokio_test::io`). Verify the received message equals the sent message.

**T7. Frame v2 with compression.** Same as T6, but with `compression_threshold = 0` (force compression on all messages). Verify round-trip correctness and that the compressed flag is set in the header.

**T8. Frame v2 without compression.** Same as T6, but with `compression_threshold = usize::MAX` (never compress). Verify round-trip correctness and that the compressed flag is NOT set.

**T9. Unknown flags rejection.** Send a frame with reserved flag bits set (e.g., `flags = 0b0000_0100`). Verify the receiver returns `ProtocolError::UnknownFlags`.

**T10. Protocol version rejection.** Simulate a v1 worker sending `RegisterPayload { protocol_version: 1, .. }` to a v2 coordinator. Verify the coordinator responds with `RegisterNack` containing a version mismatch message.

### 7.2 rkyv Tests (feature-gated)

**T11. rkyv round-trip.** For a realistic `Partition`, verify `deserialize(access(to_bytes(partition))) == partition`.

**T12. rkyv + LZ4 round-trip.** Same as T11, but with LZ4 compression applied to the archived bytes. Verify round-trip correctness.

**T13. rkyv validation rejection.** Feed a corrupted byte buffer to `rkyv::access::<ArchivedPartition>`. Verify the receiver returns `ProtocolError::ArchiveValidationFailed`.

**T14. Archive flag in frame.** Send a hot-path message with the archive flag set. Verify the receiver can decode it via the rkyv path. Verify that a non-hot-path message with the archive flag set is rejected.

### 7.3 Integration Tests

**T15. Full grid loop with v2 wire format.** Run a complete grid loop (coordinator + 2 workers) using the v2 wire format with compression enabled. Verify the final result matches the sequential reduction (G1).

**T16. v1/v2 version mismatch.** Start a v2 coordinator and attempt to connect a simulated v1 worker (sending `protocol_version: 1`). Verify the connection is rejected with `RegisterNack`.

### 7.4 Size Regression Tests

**T17. Payload size improvement.** For each benchmark profile in SPEC-09, serialize a representative partition with both v1 encoding (fixed-int, no compression) and v2 encoding (varint + compact PortRef + LZ4). Assert that v2 payload size is at most 50% of v1 payload size. This validates SPEC-06 R15's target of ~50% reduction, which v2 should exceed.

---

## 8. Open Questions

**Q1. Should the compression algorithm be configurable (LZ4 vs zstd)?**
For v2, LZ4 is the only supported algorithm. A future `compression_algorithm: CompressionAlgorithm` field in `TransportTuning` could support zstd for WAN deployments. Deferred to avoid scope creep.

**Q2. Should the rkyv path use `access_unchecked` behind a `--trust-peers` flag?**
`rkyv::access` runs a validation pass that costs CPU (though much less than full deserialization). For trusted LAN deployments, `access_unchecked` skips validation entirely. However, skipping validation opens a safety hole if a malicious or corrupted payload is received. Deferred to SPEC-10 (Security) scope.

**Q3. Should `CompactSubnet` be updated for the custom PortRef encoding?**
`CompactSubnet` stores `Vec<(AgentId, Agent, [PortRef; 3])>`. The custom `PortRef` compact encoding (R5) applies at the serde level, so `CompactSubnet`'s existing serde derives will automatically benefit from it. No changes to `CompactSubnet` itself are needed, but the round-trip tests (T1) must verify this interaction.

**Q4. What is the maximum decompression buffer size?**
`lz4_flex::decompress_size_prepended` reads the uncompressed size from the first 4 bytes of the compressed stream and allocates accordingly. A malicious payload could declare a very large uncompressed size. The receiver SHOULD impose a maximum decompression buffer size (e.g., `max_payload_size * 10`) and reject payloads that exceed it. Implementation detail, not spec'd here.

**Q5. Should the flags byte support negotiation (feature advertisement)?**
Currently, flags are per-frame (each frame declares its own compression/archive mode). An alternative is to negotiate capabilities during the Register handshake (e.g., "I support LZ4" / "I support rkyv"). This would allow the coordinator to adapt per-worker. Deferred: for v2, all workers are assumed to support the same capabilities as the coordinator.
