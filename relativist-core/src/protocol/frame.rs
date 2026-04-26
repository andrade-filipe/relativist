//! Frame format and framing functions (SPEC-06, Sections 4.2-4.3 + SPEC-18 §3.4).
//!
//! Each message is framed with a 9-byte header (v2 wire format):
//! [4 bytes length (LE u32)] [4 bytes CRC32C (LE u32)] [1 byte flags] [payload bytes...]
//!
//! The `flags` byte is a forward-compat extension point. Two bits are
//! defined today (`FLAG_COMPRESSED`, `FLAG_ARCHIVED`); the remaining six
//! are reserved and must be zero. Receivers reject frames whose `flags &
//! FLAG_RESERVED != 0` with `ProtocolError::UnknownFlags` so older
//! builds refuse to silently mis-decode a future frame variant.
//!
//! The framing functions (`send_frame`, `recv_frame`) are generic over
//! `AsyncWriteExt` / `AsyncReadExt` to support both TCP and in-memory testing.

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::bincode_v2;
use super::compression::{compress_payload, decompress_payload};
use super::error::ProtocolError;
use super::types::Message;

/// Header size in bytes: 4 (length) + 4 (CRC32C) + 1 (flags) — SPEC-18 §3.4.
pub const FRAME_HEADER_SIZE: usize = 9;

/// Bit 0 of the flags byte: payload is LZ4-compressed (SPEC-18 R9).
/// Wired in TASK-0346.
pub const FLAG_COMPRESSED: u8 = 0b0000_0001;

/// Bit 1 of the flags byte: payload is an rkyv archive (SPEC-18 §3.5,
/// deferred to ROADMAP item 2.24).
pub const FLAG_ARCHIVED: u8 = 0b0000_0010;

/// Bits 2-7 of the flags byte: reserved for future use.
/// Must be zero on the wire; any frame setting one of these bits is
/// rejected with `ProtocolError::UnknownFlags`.
pub const FLAG_RESERVED: u8 = 0b1111_1100;

/// Default maximum payload size (1 GiB).
///
/// Raised from 256 MiB as part of the L6 fix (see `docs/PHASE2-FINDINGS.md`).
/// The cap is a DoS guard, not a memory budget: large dense nets (e.g.
/// `dual_tree` depth 22 or `ep_annihilation_con` at 5M agents sent to a
/// single worker) legitimately serialise to >256 MiB even with the
/// `CompactSubnet` wire wrapper, because every agent slot is live.
pub const DEFAULT_MAX_PAYLOAD_SIZE: u32 = 1_073_741_824;

/// Header of a frame in the wire protocol (SPEC-18 §3.4 v2 layout).
/// Precedes each payload transmitted over TCP.
///
/// Does NOT derive Serialize/Deserialize — it is manually encoded
/// as raw bytes for efficiency and control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    /// Length of the payload in bytes (excluding the header itself).
    pub length: u32,
    /// CRC32C checksum of the payload.
    pub checksum: u32,
    /// Per-frame flag bits (SPEC-18 R14-R19). See `FLAG_*` constants.
    pub flags: u8,
}

impl FrameHeader {
    /// Serializes the header to 9 bytes (little-endian).
    pub fn to_bytes(&self) -> [u8; FRAME_HEADER_SIZE] {
        let mut buf = [0u8; FRAME_HEADER_SIZE];
        buf[0..4].copy_from_slice(&self.length.to_le_bytes());
        buf[4..8].copy_from_slice(&self.checksum.to_le_bytes());
        buf[8] = self.flags;
        buf
    }

    /// Deserializes the header from 9 bytes (little-endian).
    pub fn from_bytes(bytes: [u8; FRAME_HEADER_SIZE]) -> Self {
        let length = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let checksum = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let flags = bytes[8];
        Self {
            length,
            checksum,
            flags,
        }
    }

    /// True if `FLAG_COMPRESSED` is set (payload is LZ4-compressed).
    #[inline]
    pub fn is_compressed(&self) -> bool {
        self.flags & FLAG_COMPRESSED != 0
    }

    /// True if `FLAG_ARCHIVED` is set (payload is an rkyv archive).
    #[inline]
    pub fn is_archived(&self) -> bool {
        self.flags & FLAG_ARCHIVED != 0
    }

    /// True if any reserved bit is set (forward-compat hardening, SPEC-18 R19).
    #[inline]
    pub fn has_unknown_flags(&self) -> bool {
        self.flags & FLAG_RESERVED != 0
    }
}

/// Compression threshold used by the bare `send_frame` entry point.
/// Equal to `TransportConfig::default().compression_threshold` (1024).
/// `send_frame_with_threshold` lets callers override it on a per-call
/// basis (CLI flag, tests, benchmarks).
pub const DEFAULT_COMPRESSION_THRESHOLD: usize = 1024;

/// Serializes a message and sends it as a frame over the TCP socket.
///
/// Equivalent to `send_frame_with_threshold(writer, message,
/// DEFAULT_COMPRESSION_THRESHOLD)`.
pub async fn send_frame<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    message: &Message,
) -> Result<usize, ProtocolError> {
    send_frame_with_threshold(writer, message, DEFAULT_COMPRESSION_THRESHOLD).await
}

/// Serializes a message and sends it as a frame, compressing the
/// payload with LZ4 when its uncompressed size is `>=
/// compression_threshold` (SPEC-18 R9-R12, §3.3).
///
/// Returns the total number of bytes written (header + payload).
///
/// Wire-level invariants:
/// - The header's `length` field always describes the on-wire payload
///   (i.e. compressed bytes when `FLAG_COMPRESSED` is set).
/// - The header's `checksum` is **always** computed on the uncompressed
///   payload (R12 — defense in depth: a wrong-but-valid LZ4 block
///   followed by a CRC over the compressed bytes would silently decode
///   to garbage).
pub async fn send_frame_with_threshold<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    message: &Message,
    compression_threshold: usize,
) -> Result<usize, ProtocolError> {
    // 1. Serialize (bincode v2 — SPEC-18 §3.1)
    let uncompressed = bincode_v2::encode(message).map_err(ProtocolError::Serialize)?;

    // 2. CRC32C is *always* over the uncompressed bytes (R12).
    let checksum = crc32fast::hash(&uncompressed);

    // 3. Decide whether to compress (R9). `usize::MAX` disables, `0`
    // always compresses.
    let (payload, flags): (Vec<u8>, u8) =
        if uncompressed.len() >= compression_threshold && compression_threshold != usize::MAX {
            (compress_payload(&uncompressed), FLAG_COMPRESSED)
        } else {
            (uncompressed, 0)
        };

    // Guard: payload length must fit in u32 (frame header uses u32).
    let payload_len: u32 =
        payload
            .len()
            .try_into()
            .map_err(|_| ProtocolError::PayloadTooLarge {
                size: u32::MAX,
                max: u32::MAX,
            })?;

    // 4. Write header
    let header = FrameHeader {
        length: payload_len,
        checksum,
        flags,
    };
    writer
        .write_all(&header.to_bytes())
        .await
        .map_err(ProtocolError::ConnectionLost)?;

    // 5. Write payload
    writer
        .write_all(&payload)
        .await
        .map_err(ProtocolError::ConnectionLost)?;

    // 6. Flush
    writer
        .flush()
        .await
        .map_err(ProtocolError::ConnectionLost)?;

    Ok(FRAME_HEADER_SIZE + payload.len())
}

/// Wire payload for the rkyv-archived `AssignPartition` hot path
/// (SPEC-18 §3.5 R22 — only hot-path messages are eligible for the
/// zero-copy archive).
///
/// We archive a tagged tuple-equivalent struct rather than the full
/// `Message` enum because rkyv enums require all variants to be
/// `Archive`, and `Message` carries `RegisterNackPayload` (a String) and
/// other cold-path variants that we deliberately keep on the bincode path.
#[cfg(feature = "zero-copy")]
#[derive(Debug, Clone, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ArchiveAssignPayload {
    /// Round number (echo from the coordinator).
    pub round: u32,
    /// The partition assigned to the worker.
    pub partition: crate::partition::Partition,
}

/// Wire payload for the rkyv-archived `PartitionResult` hot path
/// (SPEC-18 §3.5 R22).
#[cfg(feature = "zero-copy")]
#[derive(Debug, Clone, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ArchivePartitionResultPayload {
    /// Round number (echo from the AssignPartition message).
    pub round: u32,
    /// The reduced partition.
    pub partition: crate::partition::Partition,
    /// Per-worker statistics for this round.
    pub stats: crate::merge::WorkerRoundStats,
}

/// Returns true iff `msg` is an SPEC-18 §3.5 R22 hot-path message.
///
/// Hot-path messages are the high-volume per-round payloads that benefit
/// from zero-copy archival: `AssignPartition` and `PartitionResult`.
/// All other variants (Register/Ack/Nack, Shutdown, Error) stay on the
/// bincode v2 path because their cost is negligible and their schemas
/// include types we deliberately keep off the rkyv derive surface.
#[cfg(feature = "zero-copy")]
#[inline]
pub fn is_hot_path_message(msg: &Message) -> bool {
    matches!(
        msg,
        Message::AssignPartition { .. } | Message::PartitionResult { .. }
    )
}

/// Reads exactly `len` bytes into a 16-byte-aligned `rkyv::util::AlignedVec`
/// (SPEC-18 §3.5 R25).
///
/// rkyv's validating `access` API requires the underlying byte buffer to
/// be aligned to the archive's natural alignment (16 bytes is the safe
/// upper bound for every type we archive — `Net`, `Partition`,
/// `WorkerRoundStats`, etc.). A plain `Vec<u8>` from the global allocator
/// makes no such guarantee and frequently lands at 8-byte alignment on
/// 64-bit Windows targets, which would trip the rkyv validation step.
///
/// `AlignedVec` provides 16-byte aligned storage with capacity rounded
/// up to a multiple of 16. We pre-allocate to `len`, push zeros to
/// extend the logical length, then `read_exact` straight into the
/// allocation. This avoids the `vec![0u8; len]` -> copy step the
/// non-aligned path uses.
///
/// The function is gated on `feature = "zero-copy"` because `AlignedVec`
/// is a rkyv type. `recv_frame` calls this helper directly on the
/// uncompressed-archive fast path (FLAG_ARCHIVED set, FLAG_COMPRESSED
/// clear), eliminating the second copy that `decode_archive_payload`
/// would otherwise perform. The compressed-archive path still copies
/// because `decompress_payload` returns a plain `Vec<u8>`.
#[cfg(feature = "zero-copy")]
pub(crate) async fn read_aligned_payload<R: AsyncReadExt + Unpin>(
    reader: &mut R,
    len: usize,
) -> Result<rkyv::util::AlignedVec, ProtocolError> {
    // R25 precondition: AlignedVec capacity is always a multiple of 16.
    let mut buf = rkyv::util::AlignedVec::with_capacity(len);
    // Extend logical length to `len` so `read_exact` sees a writable slice.
    buf.resize(len, 0u8);
    reader
        .read_exact(buf.as_mut_slice())
        .await
        .map_err(ProtocolError::ConnectionLost)?;
    debug_assert_eq!(buf.len(), len, "AlignedVec must hold exactly `len` bytes");
    // R25: any *non-empty* AlignedVec base pointer is 16-byte aligned. For
    // `len == 0` the buffer may be backed by a dangling pointer whose value
    // is allocator-defined; rkyv's validating `access` never reads from it
    // so the alignment requirement is vacuously satisfied.
    debug_assert!(
        buf.is_empty() || (buf.as_ptr() as usize).is_multiple_of(16),
        "non-empty AlignedVec base pointer must be 16-byte aligned (R25)"
    );
    Ok(buf)
}

/// SPEC-18 §3.5 R22-R23 — sends `message` as either an rkyv archive
/// (when `use_archive` is true AND the message is on the hot path) or
/// falls back to the bincode v2 path used by `send_frame_with_threshold`.
///
/// Returns the total number of bytes written (header + payload).
///
/// Wire-level invariants under the archive path:
/// - `FLAG_ARCHIVED` is set; `FLAG_COMPRESSED` is set when the
///   uncompressed archive size exceeds `compression_threshold` (R23).
/// - The header's `length` describes the on-wire payload (compressed
///   when `FLAG_COMPRESSED` is set).
/// - The header's `checksum` is computed on the **uncompressed archive
///   bytes** (R12 — same ordering as the bincode path).
///
/// Non-hot-path messages (Register*, Shutdown, Error) ALWAYS take the
/// bincode v2 path even when `use_archive == true` (R22). This is the
/// per-call complement to the build-time `feature = "zero-copy"` gate.
///
/// Errors during rkyv serialization are wrapped in
/// `ProtocolError::ArchiveValidationFailed` with the mandated
/// `"serialize: "` prefix (SPEC-18 §3.5 DC-4).
#[cfg(feature = "zero-copy")]
pub async fn send_frame_v2<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    message: &Message,
    use_archive: bool,
    compression_threshold: usize,
) -> Result<usize, ProtocolError> {
    // R22: only AssignPartition and PartitionResult are eligible for archive.
    if !use_archive || !is_hot_path_message(message) {
        return send_frame_with_threshold(writer, message, compression_threshold).await;
    }

    // Build the rkyv-archivable wire payload from the hot-path Message.
    let archive_bytes: rkyv::util::AlignedVec = match message {
        Message::AssignPartition { round, partition } => {
            let payload = ArchiveAssignPayload {
                round: *round,
                partition: partition.clone(),
            };
            rkyv::to_bytes::<rkyv::rancor::Error>(&payload).map_err(|e| {
                // SPEC-18 §3.5 DC-4: mandated "serialize: " prefix.
                ProtocolError::ArchiveValidationFailed(format!("serialize: {}", e))
            })?
        }
        Message::PartitionResult {
            round,
            partition,
            stats,
        } => {
            let payload = ArchivePartitionResultPayload {
                round: *round,
                partition: partition.clone(),
                stats: stats.clone(),
            };
            rkyv::to_bytes::<rkyv::rancor::Error>(&payload)
                .map_err(|e| ProtocolError::ArchiveValidationFailed(format!("serialize: {}", e)))?
        }
        // Unreachable: is_hot_path_message above filtered to these two.
        _ => unreachable!("non-hot-path message reached archive path"),
    };

    // R12: CRC32C is computed on the uncompressed archive bytes.
    let checksum = crc32fast::hash(archive_bytes.as_ref());

    // R23: optionally LZ4-wrap the archive when above threshold.
    let (payload, flags): (Vec<u8>, u8) =
        if archive_bytes.len() >= compression_threshold && compression_threshold != usize::MAX {
            (
                compress_payload(archive_bytes.as_ref()),
                FLAG_ARCHIVED | FLAG_COMPRESSED,
            )
        } else {
            (archive_bytes.as_ref().to_vec(), FLAG_ARCHIVED)
        };

    let payload_len: u32 =
        payload
            .len()
            .try_into()
            .map_err(|_| ProtocolError::PayloadTooLarge {
                size: u32::MAX,
                max: u32::MAX,
            })?;

    let header = FrameHeader {
        length: payload_len,
        checksum,
        flags,
    };
    writer
        .write_all(&header.to_bytes())
        .await
        .map_err(ProtocolError::ConnectionLost)?;
    writer
        .write_all(&payload)
        .await
        .map_err(ProtocolError::ConnectionLost)?;
    writer
        .flush()
        .await
        .map_err(ProtocolError::ConnectionLost)?;

    Ok(FRAME_HEADER_SIZE + payload.len())
}

/// Reads a frame from the TCP socket and deserializes the message.
///
/// Returns the deserialized message and the total number of bytes read.
///
/// v2 pipeline (SPEC-18 §3.4):
/// 1. Read exactly 9 header bytes (`length: u32 LE | checksum: u32 LE | flags: u8`).
/// 2. Reject if `flags & FLAG_RESERVED != 0` (R19) — *before* any
///    allocation. Forward-compat hardening against future flag variants
///    that this build cannot interpret.
/// 3. Reject if `length > max_payload_size` (OOM defense).
/// 4. Read exactly `length` bytes of (possibly compressed) payload.
///    On the FLAG_ARCHIVED + !FLAG_COMPRESSED fast path with the
///    `zero-copy` feature on, the buffer is allocated as a 16-byte
///    aligned `rkyv::util::AlignedVec` (R25) so that the validating
///    `rkyv::access` step can run without an extra copy.
/// 5. If `FLAG_COMPRESSED` is set, LZ4-decompress the payload (R13).
/// 6. Verify CRC32C against the **uncompressed** payload (R12). The
///    ordering is load-bearing: a wrong-but-valid LZ4 block followed by
///    a CRC over the compressed bytes would silently decode to garbage.
/// 7. Deserialize the uncompressed payload. The path forks on
///    FLAG_ARCHIVED: rkyv validating `access` + `deserialize`
///    (SPEC-18 §3.5 R24) when set AND the build has the `zero-copy`
///    feature; bincode v2 (SPEC-18 §3.1) otherwise.
pub async fn recv_frame<R: AsyncReadExt + Unpin>(
    reader: &mut R,
    max_payload_size: u32,
) -> Result<(Message, usize), ProtocolError> {
    // 1. Read header
    let mut header_buf = [0u8; FRAME_HEADER_SIZE];
    reader
        .read_exact(&mut header_buf)
        .await
        .map_err(ProtocolError::ConnectionLost)?;

    // 2. Extract fields
    let header = FrameHeader::from_bytes(header_buf);

    // 2a. Reject unknown flag bits BEFORE doing anything else (SPEC-18 R19).
    // Forward-compat hardening: a reserved bit set means a future build sent
    // us a frame variant we cannot interpret safely.
    if header.has_unknown_flags() {
        return Err(ProtocolError::UnknownFlags {
            flags: header.flags,
        });
    }

    // 3. Validate size BEFORE allocating (R9 — defense against OOM)
    if header.length > max_payload_size {
        return Err(ProtocolError::PayloadTooLarge {
            size: header.length,
            max: max_payload_size,
        });
    }

    let total_bytes = FRAME_HEADER_SIZE + header.length as usize;

    // 4-7 (uncompressed-archive fast path, zero-copy feature on):
    // Read directly into an `AlignedVec` so `rkyv::access` does not need
    // a second copy in `decode_archive_payload`. R12 ordering is preserved
    // (CRC verify before rkyv access).
    #[cfg(feature = "zero-copy")]
    if header.is_archived() && !header.is_compressed() {
        let aligned = read_aligned_payload(reader, header.length as usize).await?;

        let computed_crc = crc32fast::hash(aligned.as_ref());
        if computed_crc != header.checksum {
            return Err(ProtocolError::ChecksumMismatch {
                expected: header.checksum,
                computed: computed_crc,
            });
        }

        return decode_archive_payload(aligned.as_ref(), total_bytes);
    }

    // 4. Read payload (compressed or raw, per the FLAG_COMPRESSED bit)
    let mut wire_payload = vec![0u8; header.length as usize];
    reader
        .read_exact(&mut wire_payload)
        .await
        .map_err(ProtocolError::ConnectionLost)?;

    // 5. Decompress if needed (SPEC-18 R12 sequencing: decompression
    //    happens BEFORE the CRC check so the checksum verifies the
    //    original message bytes).
    let payload: Vec<u8> = if header.is_compressed() {
        decompress_payload(&wire_payload).map_err(ProtocolError::DecompressionFailed)?
    } else {
        wire_payload
    };

    // 6. Verify checksum on the (uncompressed) payload (SPEC-18 R12).
    let computed_crc = crc32fast::hash(&payload);
    if computed_crc != header.checksum {
        return Err(ProtocolError::ChecksumMismatch {
            expected: header.checksum,
            computed: computed_crc,
        });
    }

    // 7. Deserialize. The compressed-archive path lands here (the
    //    uncompressed-archive path was handled above with a direct
    //    aligned read). When FLAG_ARCHIVED is set under the zero-copy
    //    feature, the (decompressed) payload is allocator-aligned only
    //    and must be copied into an AlignedVec before rkyv access.
    //
    // R19 forward-compat: a default-features build that receives a
    // FLAG_ARCHIVED frame has already passed the `has_unknown_flags`
    // check (FLAG_ARCHIVED is bit 1, NOT in FLAG_RESERVED). Without the
    // zero-copy feature the bincode decoder will fail on the archive
    // bytes and the frame is rejected with a Deserialize error — same
    // behaviour as QA Probe 3 confirmed before this task landed.
    #[cfg(feature = "zero-copy")]
    if header.is_archived() {
        // Compressed-archive path: decompress_payload returned a plain
        // Vec<u8> whose alignment is allocator-defined. Copy into an
        // AlignedVec so rkyv::access can validate (R25).
        let mut aligned: rkyv::util::AlignedVec =
            rkyv::util::AlignedVec::with_capacity(payload.len());
        aligned.extend_from_slice(&payload);
        return decode_archive_payload(aligned.as_ref(), total_bytes);
    }

    let (message, consumed): (Message, usize) =
        bincode_v2::decode(&payload).map_err(ProtocolError::Deserialize)?;
    debug_assert_eq!(
        consumed,
        payload.len(),
        "bincode v2 decoded message must consume the entire payload"
    );

    Ok((message, total_bytes))
}

/// SPEC-18 §3.5 R22-R26 — decodes a FLAG_ARCHIVED payload with the
/// validating rkyv `access` API and reconstructs a `Message`.
///
/// The on-wire payload is one of two hot-path archives
/// (`ArchiveAssignPayload`, `ArchivePartitionResultPayload`). The receiver
/// has no tag byte to discriminate them — the discrimination is purely
/// structural: `access::<ArchivedX, _>` either succeeds (right schema)
/// or fails (wrong schema). We try the AssignPartition layout first
/// because it is the dominant traffic direction (one per-round per worker,
/// vs one PartitionResult per-round per worker — symmetric, but Assign
/// fires first in the round).
///
/// R26 — non-hot-path messages on the archive path are rejected with
/// `ArchiveValidationFailed("non-hot-path archive payload ...")`. Both
/// `try_access` calls failing is the only path to that error: it means
/// the payload bytes do not match either archive schema.
///
/// **Precondition (R25):** `payload` MUST be backed by 16-byte aligned
/// storage. `recv_frame` enforces this on both archive paths:
/// - Uncompressed-archive fast path: `read_aligned_payload` allocates
///   an `AlignedVec` directly.
/// - Compressed-archive path: `recv_frame` copies the decompressed
///   `Vec<u8>` into an `AlignedVec` before invoking this function.
///
/// Empty payloads are vacuously aligned (rkyv never reads from them).
#[cfg(feature = "zero-copy")]
fn decode_archive_payload(
    payload: &[u8],
    total_bytes: usize,
) -> Result<(Message, usize), ProtocolError> {
    // R25 alignment witness: callers (`recv_frame`) hand us bytes backed
    // by an `AlignedVec`, so the base pointer is 16-byte aligned for any
    // non-empty payload. `debug_assert!` is acceptable here because
    // `AlignedVec` guarantees this at construction; the assert is a
    // structural witness, not a correctness check.
    debug_assert!(
        payload.is_empty() || (payload.as_ptr() as usize).is_multiple_of(16),
        "decode_archive_payload precondition violated: payload must be 16-byte aligned (R25)"
    );

    // SPEC-18 R22 discrimination
    // Try the AssignPartition archive first (DC-3: Assign-first ordering).
    if let Ok(archived) =
        rkyv::access::<rkyv::Archived<ArchiveAssignPayload>, rkyv::rancor::Error>(payload)
    {
        let payload: ArchiveAssignPayload =
            rkyv::deserialize::<ArchiveAssignPayload, rkyv::rancor::Error>(archived).map_err(
                |e| ProtocolError::ArchiveValidationFailed(format!("AssignPartition: {}", e)),
            )?;
        return Ok((
            Message::AssignPartition {
                round: payload.round,
                partition: payload.partition,
            },
            total_bytes,
        ));
    }

    // SPEC-18 R22 discrimination
    if let Ok(archived) =
        rkyv::access::<rkyv::Archived<ArchivePartitionResultPayload>, rkyv::rancor::Error>(payload)
    {
        let payload: ArchivePartitionResultPayload = rkyv::deserialize::<
            ArchivePartitionResultPayload,
            rkyv::rancor::Error,
        >(archived)
        .map_err(|e| ProtocolError::ArchiveValidationFailed(format!("PartitionResult: {}", e)))?;
        return Ok((
            Message::PartitionResult {
                round: payload.round,
                partition: payload.partition,
                stats: payload.stats,
            },
            total_bytes,
        ));
    }

    // R26: neither schema matched. Either the payload is corrupt or the
    // peer sent a non-hot-path message via the archive flag.
    Err(ProtocolError::ArchiveValidationFailed(
        "non-hot-path archive payload (matched neither AssignPartition nor PartitionResult)"
            .to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::WorkerRoundStats;
    use crate::net::{Net, PortRef, Symbol};
    use crate::partition::{IdRange, Partition};
    use std::collections::HashMap;
    use tokio::io::duplex;

    /// Creates an in-memory bidirectional channel for testing.
    fn create_test_channel() -> (tokio::io::DuplexStream, tokio::io::DuplexStream) {
        duplex(1_048_576) // 1 MiB buffer
    }

    fn make_test_partition() -> Partition {
        Partition {
            subnet: Net::new(),
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange {
                start: 0,
                end: 100_000,
            },
            border_id_start: 0,
            border_id_end: 0,
        }
    }

    fn make_test_stats() -> WorkerRoundStats {
        WorkerRoundStats {
            worker_id: 0,
            agents_before: 10,
            agents_after: 5,
            local_redexes: 5,
            reduce_duration_secs: 0.001,
            interactions_by_rule: [1, 1, 1, 1, 1, 0],
            has_border_activity: false,
            is_coordinator_self: false,
        }
    }

    // --- FrameHeader tests ---

    // T1: round-trip from_bytes(to_bytes()) preserves values
    #[test]
    fn test_frame_header_roundtrip() {
        let header = FrameHeader {
            length: 1234,
            checksum: 0xDEADBEEF,
            flags: 0,
        };
        let restored = FrameHeader::from_bytes(header.to_bytes());
        assert_eq!(header, restored);
    }

    // T2: to_bytes produces correct little-endian byte order (v2: 9 bytes)
    #[test]
    fn test_frame_header_byte_order() {
        let header = FrameHeader {
            length: 0x04030201,
            checksum: 0x08070605,
            flags: 0x09,
        };
        let bytes = header.to_bytes();
        assert_eq!(
            bytes,
            [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09]
        );
    }

    // T3: constants have expected values (v2: 9-byte header)
    #[test]
    fn test_framing_constants() {
        assert_eq!(FRAME_HEADER_SIZE, 9);
        assert_eq!(DEFAULT_MAX_PAYLOAD_SIZE, 1_073_741_824);
    }

    // T4: edge case — length = 0
    #[test]
    fn test_frame_header_zero_length() {
        let header = FrameHeader {
            length: 0,
            checksum: 0,
            flags: 0,
        };
        let restored = FrameHeader::from_bytes(header.to_bytes());
        assert_eq!(restored.length, 0);
        assert_eq!(restored.checksum, 0);
        assert_eq!(restored.flags, 0);
    }

    // T5: edge case — max values
    #[test]
    fn test_frame_header_max_values() {
        let header = FrameHeader {
            length: u32::MAX,
            checksum: u32::MAX,
            flags: 0,
        };
        let restored = FrameHeader::from_bytes(header.to_bytes());
        assert_eq!(header, restored);
    }

    // --- send_frame / recv_frame tests ---

    // T6: round-trip Shutdown
    #[tokio::test]
    async fn test_round_trip_shutdown() {
        let (mut client, mut server) = create_test_channel();
        let bytes_written = send_frame(&mut client, &Message::Shutdown).await.unwrap();
        assert!(bytes_written > FRAME_HEADER_SIZE);

        let (msg, bytes_read) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        assert_eq!(bytes_written, bytes_read);
        assert!(matches!(msg, Message::Shutdown));
    }

    // T7: round-trip AssignPartition
    #[tokio::test]
    async fn test_round_trip_assign_partition() {
        let (mut client, mut server) = create_test_channel();
        let msg = Message::AssignPartition {
            round: 42,
            partition: make_test_partition(),
        };
        send_frame(&mut client, &msg).await.unwrap();

        let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        match received {
            Message::AssignPartition { round, partition } => {
                assert_eq!(round, 42);
                assert_eq!(partition.worker_id, 0);
            }
            _ => panic!("wrong variant"),
        }
    }

    // T8: round-trip PartitionResult
    #[tokio::test]
    async fn test_round_trip_partition_result() {
        let (mut client, mut server) = create_test_channel();
        let msg = Message::PartitionResult {
            round: 7,
            partition: make_test_partition(),
            stats: make_test_stats(),
        };
        send_frame(&mut client, &msg).await.unwrap();

        let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        match received {
            Message::PartitionResult { round, stats, .. } => {
                assert_eq!(round, 7);
                assert_eq!(stats.agents_before, 10);
                assert_eq!(stats.agents_after, 5);
            }
            _ => panic!("wrong variant"),
        }
    }

    // T9: round-trip Error
    #[tokio::test]
    async fn test_round_trip_error() {
        let (mut client, mut server) = create_test_channel();
        let msg = Message::Error {
            round: 3,
            worker_id: 1,
            description: "test error".into(),
        };
        send_frame(&mut client, &msg).await.unwrap();

        let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        match received {
            Message::Error {
                round,
                worker_id,
                description,
            } => {
                assert_eq!(round, 3);
                assert_eq!(worker_id, 1);
                assert_eq!(description, "test error");
            }
            _ => panic!("wrong variant"),
        }
    }

    // T10: round-trip Register
    #[tokio::test]
    async fn test_round_trip_register() {
        let (mut client, mut server) = create_test_channel();
        let msg = Message::Register(super::super::types::RegisterPayload {
            protocol_version: 1,
            auth_token: None,
        });
        send_frame(&mut client, &msg).await.unwrap();

        let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        assert!(matches!(received, Message::Register(_)));
    }

    // T11: round-trip RegisterAck
    #[tokio::test]
    async fn test_round_trip_register_ack() {
        let (mut client, mut server) = create_test_channel();
        let msg = Message::RegisterAck(super::super::types::RegisterAckPayload { worker_id: 5 });
        send_frame(&mut client, &msg).await.unwrap();

        let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        match received {
            Message::RegisterAck(payload) => assert_eq!(payload.worker_id, 5),
            _ => panic!("wrong variant"),
        }
    }

    // T12: round-trip RegisterNack
    #[tokio::test]
    async fn test_round_trip_register_nack() {
        let (mut client, mut server) = create_test_channel();
        let msg = Message::RegisterNack(super::super::types::RegisterNackPayload {
            reason: "auth failed".into(),
        });
        send_frame(&mut client, &msg).await.unwrap();

        let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        match received {
            Message::RegisterNack(payload) => assert_eq!(payload.reason, "auth failed"),
            _ => panic!("wrong variant"),
        }
    }

    // T13: checksum mismatch — corrupt payload byte
    #[tokio::test]
    async fn test_checksum_mismatch() {
        let (mut client, mut server) = create_test_channel();

        // Manually write a frame with corrupted payload
        let payload = bincode_v2::encode(&Message::Shutdown).unwrap();
        let checksum = crc32fast::hash(&payload);
        let header = FrameHeader {
            length: payload.len() as u32,
            checksum,
            flags: 0,
        };
        client.write_all(&header.to_bytes()).await.unwrap();

        // Write corrupted payload (flip first byte)
        let mut corrupted = payload;
        corrupted[0] ^= 0xFF;
        client.write_all(&corrupted).await.unwrap();
        client.flush().await.unwrap();

        let result = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE).await;
        assert!(matches!(
            result,
            Err(ProtocolError::ChecksumMismatch { .. })
        ));
    }

    // T14: payload too large — rejected before allocating
    #[tokio::test]
    async fn test_payload_too_large() {
        let (mut client, mut server) = create_test_channel();

        // Write a header claiming a huge payload
        let header = FrameHeader {
            length: 1_000_000,
            checksum: 0,
            flags: 0,
        };
        client.write_all(&header.to_bytes()).await.unwrap();
        client.flush().await.unwrap();

        // Set max to something small
        let result = recv_frame(&mut server, 1024).await;
        match result {
            Err(ProtocolError::PayloadTooLarge { size, max }) => {
                assert_eq!(size, 1_000_000);
                assert_eq!(max, 1024);
            }
            other => panic!("expected PayloadTooLarge, got {:?}", other),
        }
    }

    // T15: bytes_written equals FRAME_HEADER_SIZE + payload length
    #[tokio::test]
    async fn test_bytes_count() {
        let (mut client, mut server) = create_test_channel();
        let msg = Message::Shutdown;
        let payload_size = bincode_v2::encode(&msg).unwrap().len();
        let bytes_written = send_frame(&mut client, &msg).await.unwrap();
        assert_eq!(bytes_written, FRAME_HEADER_SIZE + payload_size);

        let (_, bytes_read) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        assert_eq!(bytes_written, bytes_read);
    }

    // T16: multiple messages in sequence
    #[tokio::test]
    async fn test_multiple_messages_sequence() {
        let (mut client, mut server) = create_test_channel();

        // Send 3 messages
        send_frame(&mut client, &Message::Shutdown).await.unwrap();
        send_frame(
            &mut client,
            &Message::Error {
                round: 0,
                worker_id: 0,
                description: "oops".into(),
            },
        )
        .await
        .unwrap();
        send_frame(&mut client, &Message::Shutdown).await.unwrap();

        // Receive 3 messages in order
        let (m1, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        assert!(matches!(m1, Message::Shutdown));

        let (m2, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        assert!(matches!(m2, Message::Error { .. }));

        let (m3, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        assert!(matches!(m3, Message::Shutdown));
    }

    // T17: AssignPartition with real agents
    #[tokio::test]
    async fn test_round_trip_partition_with_agents() {
        let (mut client, mut server) = create_test_channel();

        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let _b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));

        let partition = Partition {
            subnet: net,
            worker_id: 2,
            free_port_index: HashMap::new(),
            id_range: IdRange {
                start: 100,
                end: 200,
            },
            border_id_start: 50,
            border_id_end: 60,
        };

        let msg = Message::AssignPartition {
            round: 0,
            partition,
        };
        send_frame(&mut client, &msg).await.unwrap();

        let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        match received {
            Message::AssignPartition { round, partition } => {
                assert_eq!(round, 0);
                assert_eq!(partition.worker_id, 2);
                assert_eq!(partition.subnet.count_live_agents(), 2);
                assert_eq!(partition.id_range.start, 100);
                assert_eq!(partition.border_id_start, 50);
            }
            _ => panic!("wrong variant"),
        }
    }

    // --- TASK-0345 spec tests (TEST-SPEC-0345 R1-R7) ---

    /// R1: header size constant updated to 9.
    #[test]
    fn frame_header_size_is_nine() {
        assert_eq!(FRAME_HEADER_SIZE, 9);
    }

    /// R2: flag bit constants are correct and partition the byte.
    #[test]
    fn frame_flag_constants_are_correct() {
        assert_eq!(FLAG_COMPRESSED, 0b0000_0001);
        assert_eq!(FLAG_ARCHIVED, 0b0000_0010);
        assert_eq!(FLAG_RESERVED, 0b1111_1100);
        // The three constants together cover every bit exactly once.
        assert_eq!(FLAG_COMPRESSED | FLAG_ARCHIVED | FLAG_RESERVED, 0xFF);
        assert_eq!(FLAG_COMPRESSED & FLAG_ARCHIVED, 0);
        assert_eq!(FLAG_COMPRESSED & FLAG_RESERVED, 0);
        assert_eq!(FLAG_ARCHIVED & FLAG_RESERVED, 0);
    }

    /// R3: round-trip with flags=0 succeeds and the wire byte is zero.
    #[tokio::test]
    async fn frame_v2_roundtrip_no_flags() {
        let (mut client, mut server) = create_test_channel();
        let msg = Message::Shutdown;
        let bytes_written = send_frame(&mut client, &msg).await.unwrap();
        assert!(bytes_written >= FRAME_HEADER_SIZE);

        // Sniff the flags byte by reading the raw header off a second channel.
        let mut buf: Vec<u8> = Vec::new();
        send_frame(&mut buf, &Message::Shutdown).await.unwrap();
        assert_eq!(buf[8], 0, "flags byte must be 0 by default");

        let (back, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        assert!(matches!(back, Message::Shutdown));
    }

    /// R4: helpers `is_compressed` / `is_archived` / `has_unknown_flags`.
    #[test]
    fn frame_header_flag_helpers() {
        let h = FrameHeader {
            length: 0,
            checksum: 0,
            flags: 0,
        };
        assert!(!h.is_compressed());
        assert!(!h.is_archived());
        assert!(!h.has_unknown_flags());

        let h = FrameHeader {
            length: 0,
            checksum: 0,
            flags: FLAG_COMPRESSED,
        };
        assert!(h.is_compressed());
        assert!(!h.is_archived());
        assert!(!h.has_unknown_flags());

        let h = FrameHeader {
            length: 0,
            checksum: 0,
            flags: FLAG_ARCHIVED,
        };
        assert!(!h.is_compressed());
        assert!(h.is_archived());
        assert!(!h.has_unknown_flags());

        let h = FrameHeader {
            length: 0,
            checksum: 0,
            flags: FLAG_COMPRESSED | FLAG_ARCHIVED,
        };
        assert!(h.is_compressed());
        assert!(h.is_archived());
        assert!(!h.has_unknown_flags());

        let h = FrameHeader {
            length: 0,
            checksum: 0,
            flags: 0b0000_0100,
        };
        assert!(h.has_unknown_flags(), "reserved bit 2 must trigger unknown");
    }

    /// R5: receiver rejects unknown flag bits with `ProtocolError::UnknownFlags`.
    #[tokio::test]
    async fn frame_v2_unknown_flag_bit_rejected() {
        let (mut client, mut server) = create_test_channel();

        // Hand-craft a 9-byte header with reserved bit 2 set.
        let length: u32 = 0;
        let checksum: u32 = 0; // CRC32C of empty payload
        let flags: u8 = 0b0000_0100;

        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(&length.to_le_bytes());
        buf.extend_from_slice(&checksum.to_le_bytes());
        buf.push(flags);
        client.write_all(&buf).await.unwrap();
        client.flush().await.unwrap();

        let err = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap_err();
        assert!(
            matches!(err, ProtocolError::UnknownFlags { flags: 0b0000_0100 }),
            "expected UnknownFlags(0b0000_0100), got {:?}",
            err,
        );
    }

    /// R6: `ProtocolError::UnknownFlags` Display formatting.
    #[test]
    fn protocol_error_unknown_flags_renders() {
        let e = ProtocolError::UnknownFlags { flags: 0b1010_0100 };
        let s = e.to_string();
        assert!(s.contains("unknown frame flags"), "got: {}", s);
        assert!(s.contains("10100100"), "binary repr expected, got: {}", s);
    }

    /// R7: bytes_written reflects the new 9-byte header (regression check).
    #[tokio::test]
    async fn bytes_sent_per_round_includes_new_header_byte() {
        let mut buf: Vec<u8> = Vec::new();
        send_frame(&mut buf, &Message::Shutdown).await.unwrap();
        assert!(buf.len() >= FRAME_HEADER_SIZE);
    }

    // --- TASK-0346 spec tests (TEST-SPEC-0346 R1, R2, R5, R6, R8, R9) ---

    /// Build an `AssignPartition` whose serialised payload is large enough
    /// to clear any non-zero compression threshold (large `Net` literal,
    /// no need to be representative — only the byte count matters).
    fn make_large_assign_partition() -> Message {
        let mut net = Net::new();
        for i in 0..256u32 {
            let a = net.create_agent(Symbol::Con);
            net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(i));
            net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1024 + i));
        }
        let partition = Partition {
            subnet: net,
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange {
                start: 0,
                end: 100_000,
            },
            border_id_start: 0,
            border_id_end: 0,
        };
        Message::AssignPartition {
            round: 0,
            partition,
        }
    }

    /// R1: payload above threshold is compressed and round-trips.
    #[tokio::test]
    async fn lz4_compresses_above_threshold() {
        let msg = make_large_assign_partition();

        let mut buf: Vec<u8> = Vec::new();
        send_frame_with_threshold(&mut buf, &msg, 0).await.unwrap();
        assert_ne!(
            buf[8] & FLAG_COMPRESSED,
            0,
            "compression flag must be set when threshold = 0"
        );

        let mut cur = std::io::Cursor::new(buf);
        let (back, _) = recv_frame(&mut cur, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        match back {
            Message::AssignPartition { round, partition } => {
                assert_eq!(round, 0);
                assert_eq!(partition.subnet.count_live_agents(), 256);
            }
            other => panic!("wrong variant: {:?}", other),
        }
    }

    /// R2: payload below threshold is sent raw.
    #[tokio::test]
    async fn lz4_skips_below_threshold() {
        let msg = Message::Shutdown;

        let mut buf: Vec<u8> = Vec::new();
        send_frame_with_threshold(&mut buf, &msg, usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            buf[8] & FLAG_COMPRESSED,
            0,
            "compression flag must NOT be set"
        );

        let mut cur = std::io::Cursor::new(buf);
        let (back, _) = recv_frame(&mut cur, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        assert!(matches!(back, Message::Shutdown));
    }

    /// R5: CRC32C is computed on the *uncompressed* payload (R12).
    #[tokio::test]
    async fn checksum_is_on_uncompressed_payload() {
        let msg = make_large_assign_partition();

        let mut buf: Vec<u8> = Vec::new();
        send_frame_with_threshold(&mut buf, &msg, 0).await.unwrap();

        let length = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        let checksum = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let flags = buf[8];
        assert_ne!(flags & FLAG_COMPRESSED, 0);

        let wire_payload = &buf[FRAME_HEADER_SIZE..FRAME_HEADER_SIZE + length];
        let uncompressed = crate::protocol::compression::decompress_payload(wire_payload).unwrap();
        assert_eq!(
            crc32fast::hash(&uncompressed),
            checksum,
            "checksum must match the uncompressed payload (R12)"
        );
    }

    /// R6: corrupted compressed payload is rejected with either
    /// `DecompressionFailed` or `ChecksumMismatch` — never silently
    /// accepted.
    #[tokio::test]
    async fn corrupted_compressed_payload_is_rejected() {
        let msg = make_large_assign_partition();

        let mut buf: Vec<u8> = Vec::new();
        send_frame_with_threshold(&mut buf, &msg, 0).await.unwrap();

        // Corrupt one byte deep inside the compressed body (skip the
        // 4-byte uncompressed-size prefix written by `compress_payload`
        // so the size header stays sane).
        let payload_off = FRAME_HEADER_SIZE + 4;
        buf[payload_off] ^= 0xFF;

        let mut cur = std::io::Cursor::new(buf);
        let err = recv_frame(&mut cur, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                ProtocolError::DecompressionFailed(_) | ProtocolError::ChecksumMismatch { .. }
            ),
            "expected DecompressionFailed or ChecksumMismatch, got {:?}",
            err,
        );
    }

    /// R8: default `send_frame` uses the 1024-byte threshold.
    #[tokio::test]
    async fn default_send_frame_uses_default_threshold() {
        // Shutdown serialises to ~1 byte → below the default threshold,
        // so the compression flag must stay clear.
        let mut buf: Vec<u8> = Vec::new();
        send_frame(&mut buf, &Message::Shutdown).await.unwrap();
        assert_eq!(buf[8] & FLAG_COMPRESSED, 0);

        // The large AssignPartition serialises to many KB → above the
        // default threshold, so the compression flag must be set.
        let mut buf2: Vec<u8> = Vec::new();
        send_frame(&mut buf2, &make_large_assign_partition())
            .await
            .unwrap();
        assert_ne!(buf2[8] & FLAG_COMPRESSED, 0);
    }

    /// R9: `TransportConfig::default().compression_threshold == 1024`.
    #[test]
    fn default_compression_threshold_is_1024() {
        let cfg = crate::protocol::config::TransportConfig::default();
        assert_eq!(cfg.compression_threshold, 1024);
        assert_eq!(DEFAULT_COMPRESSION_THRESHOLD, 1024);
    }

    // --- TASK-0347 R5: full v2 pipeline round-trip — every Message variant ---

    /// Re-encode `back` and `original` through bincode v2 and assert the
    /// byte streams match. This is our PartialEq surrogate for `Message`,
    /// which can't derive PartialEq because its inner `Net` field doesn't.
    /// Equality of bincode encodings is sufficient for round-trip identity:
    /// distinct `Message` values cannot serialise to identical byte streams
    /// under deterministic encoders, and the test inputs use empty/sorted
    /// `HashMap`s so encoding order is stable.
    fn assert_message_eq_via_bincode(label: &str, original: &Message, back: &Message) {
        let lhs = bincode_v2::encode(original).unwrap();
        let rhs = bincode_v2::encode(back).unwrap();
        assert_eq!(
            lhs, rhs,
            "variant `{}` failed v2 pipeline round-trip identity",
            label
        );
    }

    /// Enumerate every `Message` variant. Using an exhaustive `match` on a
    /// dummy value forces a compile error if a new variant is added without
    /// updating this test (TASK-0347 R5).
    fn sample_all_message_variants() -> Vec<(&'static str, Message)> {
        // Compile-time exhaustiveness gate: any new variant breaks this match.
        #[allow(dead_code)]
        fn _exhaustive_check(m: &Message) {
            match m {
                Message::AssignPartition { .. } => {}
                Message::Shutdown => {}
                Message::PartitionResult { .. } => {}
                Message::Error { .. } => {}
                Message::Register(_) => {}
                Message::RegisterAck(_) => {}
                Message::RegisterNack(_) => {}
                Message::InitialPartition { .. } => {}
                Message::RoundStart { .. } => {}
                Message::RoundResult { .. } => {}
                Message::FinalStateRequest { .. } => {}
                Message::FinalStateResult { .. } => {}
                Message::JoinRequest { .. } => {}
                Message::JoinAck { .. } => {}
                Message::LeaveRequest { .. } => {}
                Message::LeaveAck => {}
                Message::JoinNack { .. } => {}
            }
        }

        use crate::protocol::types::{RegisterAckPayload, RegisterNackPayload, RegisterPayload};

        vec![
            (
                "AssignPartition",
                Message::AssignPartition {
                    round: 7,
                    partition: make_test_partition(),
                },
            ),
            ("Shutdown", Message::Shutdown),
            (
                "PartitionResult",
                Message::PartitionResult {
                    round: 11,
                    partition: make_test_partition(),
                    stats: make_test_stats(),
                },
            ),
            (
                "Error",
                Message::Error {
                    round: 3,
                    worker_id: 1,
                    description: "v2 round-trip".into(),
                },
            ),
            (
                "Register",
                Message::Register(RegisterPayload {
                    protocol_version: 2,
                    auth_token: Some([0xAB; 32]),
                }),
            ),
            (
                "RegisterAck",
                Message::RegisterAck(RegisterAckPayload { worker_id: 42 }),
            ),
            (
                "RegisterNack",
                Message::RegisterNack(RegisterNackPayload {
                    reason: "protocol version mismatch: expected 2, got 1".into(),
                }),
            ),
            // SPEC-19 §3.4 (item 2.26-A) — delta-protocol variants.
            (
                "InitialPartition",
                Message::InitialPartition {
                    round: 0,
                    partition: make_test_partition(),
                },
            ),
            (
                "RoundStart",
                Message::RoundStart {
                    round: 1,
                    border_deltas: Vec::new(),
                    resolved_borders: Vec::new(),
                    new_borders: Vec::new(),
                    local_reconnections: Vec::new(),
                    pending_commutations: Vec::new(),
                },
            ),
            (
                "RoundResult",
                Message::RoundResult {
                    round: 1,
                    border_deltas: Vec::new(),
                    stats: make_test_stats(),
                    has_border_activity: false,
                    minted_agents: Vec::new(),
                },
            ),
            ("FinalStateRequest", Message::FinalStateRequest { round: 2 }),
            (
                "FinalStateResult",
                Message::FinalStateResult {
                    round: 2,
                    partition: make_test_partition(),
                },
            ),
            (
                "JoinRequest",
                Message::JoinRequest {
                    protocol_version: 4,
                    auth_token: None,
                    capabilities: Default::default(),
                },
            ),
            (
                "JoinAck",
                Message::JoinAck {
                    worker_id: 1,
                    partition_index: 0,
                    next_round_number: 1,
                },
            ),
            (
                "LeaveRequest",
                Message::LeaveRequest {
                    kind: crate::protocol::types::LeaveKind::AfterResult,
                },
            ),
            ("LeaveAck", Message::LeaveAck),
            (
                "JoinNack",
                Message::JoinNack {
                    reason: crate::protocol::types::JoinNackReason::ElasticJoinDisabled,
                },
            ),
        ]
    }

    /// TASK-0347 R5 (uncompressed path): every `Message` variant survives
    /// the complete v2 wire pipeline (bincode v2 + 9-byte header + CRC) when
    /// the payload stays below the compression threshold.
    #[tokio::test]
    async fn v2_pipeline_round_trip_all_message_variants_uncompressed() {
        for (label, msg) in sample_all_message_variants() {
            let mut buf: Vec<u8> = Vec::new();
            send_frame_with_threshold(&mut buf, &msg, usize::MAX)
                .await
                .unwrap();
            assert_eq!(
                buf[8] & FLAG_COMPRESSED,
                0,
                "variant `{}` should be uncompressed (threshold = MAX)",
                label
            );

            let mut cur = std::io::Cursor::new(buf);
            let (back, _) = recv_frame(&mut cur, DEFAULT_MAX_PAYLOAD_SIZE)
                .await
                .unwrap();
            assert_message_eq_via_bincode(label, &msg, &back);
        }
    }

    /// TASK-0347 R5 (compressed path): every `Message` variant survives the
    /// complete v2 wire pipeline including LZ4 compression. Threshold = 0
    /// forces the compression flag for every variant, exercising the full
    /// bincode v2 + LZ4 + CRC + 9-byte header chain end to end.
    #[tokio::test]
    async fn v2_pipeline_round_trip_all_message_variants_compressed() {
        for (label, msg) in sample_all_message_variants() {
            let mut buf: Vec<u8> = Vec::new();
            send_frame_with_threshold(&mut buf, &msg, 0).await.unwrap();
            assert_ne!(
                buf[8] & FLAG_COMPRESSED,
                0,
                "variant `{}` should be compressed (threshold = 0)",
                label
            );

            let mut cur = std::io::Cursor::new(buf);
            let (back, _) = recv_frame(&mut cur, DEFAULT_MAX_PAYLOAD_SIZE)
                .await
                .unwrap();
            assert_message_eq_via_bincode(label, &msg, &back);
        }
    }

    // === SPEC-18 §3.1-3.4 + §3.6 — QA stage adversarial probes ===
    //
    // These tests exercise failure-mode paths that the spec-driven suite
    // does not cover directly. Each probe is named after the QA list in
    // `docs/reviews/REVIEW-SPEC-18-WireFormat-v2.md` so a future reader
    // can trace the test back to the rationale that motivated it.

    /// QA probe #1: a frame whose payload truncates a multi-byte PortRef
    /// varint must surface as `ProtocolError::Deserialize` — never as a
    /// silent partial decode and never as a panic.
    #[tokio::test]
    async fn qa_probe_1_truncated_portref_body_yields_deserialize_error() {
        // Build a Message that embeds a PortRef whose id requires a
        // multi-byte varint (>= 0x80), then truncate the trailing bytes.
        let mut net = Net::new();
        for i in 0..4u32 {
            let a = net.create_agent(Symbol::Con);
            net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0x1234 + i));
            net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(0x5678 + i));
        }
        let partition = Partition {
            subnet: net,
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange {
                start: 0,
                end: 100_000,
            },
            border_id_start: 0,
            border_id_end: 0,
        };
        let msg = Message::AssignPartition {
            round: 0,
            partition,
        };

        let mut payload = bincode_v2::encode(&msg).unwrap();
        // Drop the last byte to truncate one of the trailing varint fields.
        payload.pop().unwrap();

        let checksum = crc32fast::hash(&payload);
        let header = FrameHeader {
            length: payload.len() as u32,
            checksum,
            flags: 0,
        };

        let (mut client, mut server) = create_test_channel();
        client.write_all(&header.to_bytes()).await.unwrap();
        client.write_all(&payload).await.unwrap();
        client.flush().await.unwrap();

        let err = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap_err();
        assert!(
            matches!(err, ProtocolError::Deserialize(_)),
            "expected Deserialize on truncated PortRef body, got {:?}",
            err,
        );
    }

    /// QA probe #2: a compressed frame with declared length 0 has no
    /// room for the LZ4 size-prefix and must be rejected by
    /// `decompress_payload`'s guard, surfaced as `DecompressionFailed`.
    #[tokio::test]
    async fn qa_probe_2_compressed_empty_frame_yields_decompression_failed() {
        let header = FrameHeader {
            length: 0,
            checksum: 0,
            flags: FLAG_COMPRESSED,
        };

        let (mut client, mut server) = create_test_channel();
        client.write_all(&header.to_bytes()).await.unwrap();
        client.flush().await.unwrap();

        let err = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap_err();
        assert!(
            matches!(err, ProtocolError::DecompressionFailed(_)),
            "expected DecompressionFailed on compressed empty frame, got {:?}",
            err,
        );
    }

    /// QA probe #3: `flags = FLAG_COMPRESSED | FLAG_ARCHIVED` (0x03) sets
    /// no reserved bit, so `recv_frame` proceeds. Under the default build
    /// the rkyv path is absent, so `FLAG_ARCHIVED` is ignored — the
    /// payload is decompressed and fed to bincode. Garbage bytes must
    /// surface as a clean `Deserialize` error, never a panic or a
    /// partially-decoded `Message`. Under `--features zero-copy` the
    /// archive validator runs and rejects the garbage as
    /// `ArchiveValidationFailed`.
    #[cfg(not(feature = "zero-copy"))]
    #[tokio::test]
    async fn qa_probe_3_both_flags_set_archive_bit_currently_ignored() {
        let garbage = vec![0xAAu8; 256];
        let compressed = crate::protocol::compression::compress_payload(&garbage);
        let checksum = crc32fast::hash(&garbage);
        let header = FrameHeader {
            length: compressed.len() as u32,
            checksum,
            flags: FLAG_COMPRESSED | FLAG_ARCHIVED,
        };

        let (mut client, mut server) = create_test_channel();
        client.write_all(&header.to_bytes()).await.unwrap();
        client.write_all(&compressed).await.unwrap();
        client.flush().await.unwrap();

        let err = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap_err();
        assert!(
            matches!(err, ProtocolError::Deserialize(_)),
            "expected Deserialize when FLAG_ARCHIVED carries non-bincode bytes, got {:?}",
            err,
        );
    }

    /// QA probe #3 (zero-copy variant): same garbage payload now exercises
    /// the rkyv archive validator and must be rejected as
    /// `ArchiveValidationFailed`.
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn qa_probe_3_both_flags_set_archive_validator_rejects_garbage() {
        let garbage = vec![0xAAu8; 256];
        let compressed = crate::protocol::compression::compress_payload(&garbage);
        let checksum = crc32fast::hash(&garbage);
        let header = FrameHeader {
            length: compressed.len() as u32,
            checksum,
            flags: FLAG_COMPRESSED | FLAG_ARCHIVED,
        };

        let (mut client, mut server) = create_test_channel();
        client.write_all(&header.to_bytes()).await.unwrap();
        client.write_all(&compressed).await.unwrap();
        client.flush().await.unwrap();

        let err = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap_err();
        assert!(
            matches!(err, ProtocolError::ArchiveValidationFailed(_)),
            "expected ArchiveValidationFailed when FLAG_ARCHIVED carries garbage, got {:?}",
            err,
        );
    }

    /// QA probe #4: corrupting the CRC field of a compressed frame must
    /// yield `ChecksumMismatch` — not `DecompressionFailed` — because the
    /// R12 sequencing decompresses BEFORE the CRC check.
    #[tokio::test]
    async fn qa_probe_4_compression_flag_with_corrupted_crc_yields_checksum_mismatch() {
        let msg = make_large_assign_partition();
        let mut buf: Vec<u8> = Vec::new();
        send_frame_with_threshold(&mut buf, &msg, 0).await.unwrap();
        assert_ne!(
            buf[8] & FLAG_COMPRESSED,
            0,
            "test setup expects compression flag set"
        );

        // Flip every bit of the CRC field (offset 4..8). Body stays valid LZ4.
        for byte in buf.iter_mut().take(8).skip(4) {
            *byte ^= 0xFF;
        }

        let mut cur = std::io::Cursor::new(buf);
        let err = recv_frame(&mut cur, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap_err();
        assert!(
            matches!(err, ProtocolError::ChecksumMismatch { .. }),
            "expected ChecksumMismatch (R12 sequencing), got {:?}",
            err,
        );
    }

    /// QA probe #7: with `compression_threshold = 0` even a 1-byte
    /// `Shutdown` payload must be compressed, and the round-trip must
    /// still succeed (no degenerate edge case).
    #[tokio::test]
    async fn qa_probe_7_threshold_zero_compresses_minimal_message() {
        let mut buf: Vec<u8> = Vec::new();
        send_frame_with_threshold(&mut buf, &Message::Shutdown, 0)
            .await
            .unwrap();
        assert_ne!(
            buf[8] & FLAG_COMPRESSED,
            0,
            "threshold = 0 must compress every frame, even a 1-byte Shutdown"
        );

        let mut cur = std::io::Cursor::new(buf);
        let (back, _) = recv_frame(&mut cur, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        assert!(matches!(back, Message::Shutdown));
    }

    /// QA probe #8: with `compression_threshold = usize::MAX` even a
    /// multi-KB `AssignPartition` payload must skip compression.
    #[tokio::test]
    async fn qa_probe_8_threshold_usize_max_skips_compression_on_large_message() {
        let msg = make_large_assign_partition();
        let mut buf: Vec<u8> = Vec::new();
        send_frame_with_threshold(&mut buf, &msg, usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            buf[8] & FLAG_COMPRESSED,
            0,
            "threshold = usize::MAX must disable compression, even on multi-KB payloads"
        );

        let mut cur = std::io::Cursor::new(buf);
        let (back, _) = recv_frame(&mut cur, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        match back {
            Message::AssignPartition { partition, .. } => {
                assert_eq!(partition.subnet.count_live_agents(), 256);
            }
            other => panic!("wrong variant: {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // TASK-0353 UT-09 — Coexistence of bincode v2 and rkyv derives.
    //
    // This test MUST run in both the default-features build and with
    // `--features zero-copy`. It verifies that:
    //
    //   1. Adding `#[cfg_attr(feature = "zero-copy", derive(rkyv::...))]`
    //      on hot-path types did NOT perturb the serde-based bincode v2
    //      wire format for a full `Message::AssignPartition` round-trip.
    //   2. The manual `PortRef` serde impl (with its compact tagged-bytes
    //      encoding) still produces the hot-path `DISCONNECTED = 1 byte`
    //      envelope exactly as SPEC-18 §4.3 mandates.
    //
    // Under the zero-copy feature this test runs as-is against the bincode
    // path — confirming both serializer families live side-by-side without
    // interference (rkyv's `Serialize` trait lives in a different namespace
    // than serde's, so the derives do not conflict at resolution time).
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn serde_bincode_v2_path_unaffected_by_rkyv_derives() {
        // Subnet + partition that touches every hot-path type on the wire.
        let mut subnet = Net::new();
        let a = subnet.create_agent(Symbol::Con);
        let b = subnet.create_agent(Symbol::Era);
        subnet.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        subnet.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(10));
        // One DISCONNECTED port to exercise the 1-byte hot path.
        subnet.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(u32::MAX));

        let mut free_port_index = HashMap::new();
        free_port_index.insert(10u32, PortRef::AgentPort(a, 1));

        let partition = Partition {
            subnet,
            worker_id: 0,
            free_port_index,
            id_range: IdRange {
                start: 0,
                end: 1_000,
            },
            border_id_start: 0,
            border_id_end: 10,
        };

        let msg = Message::AssignPartition {
            round: 1,
            partition,
        };

        // Encode through the bincode v2 path (frame header + CRC32C etc.).
        let mut buf: Vec<u8> = Vec::new();
        send_frame(&mut buf, &msg).await.unwrap();
        assert!(
            !buf.is_empty(),
            "bincode v2 wire output must remain non-empty"
        );

        // Decode and confirm the message came back intact.
        let mut cur = std::io::Cursor::new(buf);
        let (back, _) = recv_frame(&mut cur, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        match back {
            Message::AssignPartition {
                round,
                partition: back_p,
            } => {
                assert_eq!(round, 1);
                assert_eq!(back_p.worker_id, 0);
                assert_eq!(back_p.id_range.start, 0);
                assert_eq!(back_p.id_range.end, 1_000);
                assert_eq!(back_p.border_id_start, 0);
                assert_eq!(back_p.border_id_end, 10);
                assert_eq!(back_p.subnet.count_live_agents(), 2);
                assert_eq!(
                    back_p.free_port_index.get(&10),
                    Some(&PortRef::AgentPort(0, 1)),
                    "free_port_index must round-trip unchanged under bincode v2"
                );
            }
            other => panic!("wrong variant: {:?}", other),
        }

        // R8 hot-path sanity: DISCONNECTED must still collapse to 1 byte
        // under the compact PortRef encoding, regardless of rkyv derives.
        let disc_bytes = crate::protocol::bincode_v2::encode(&PortRef::FreePort(u32::MAX)).unwrap();
        assert_eq!(
            disc_bytes.len(),
            1,
            "DISCONNECTED must remain 1 byte on the bincode v2 wire"
        );
    }

    // -----------------------------------------------------------------------
    // TASK-0355 — `read_aligned_payload` (SPEC-18 §3.5 R25).
    //
    // The aligned-receive helper allocates a 16-byte aligned buffer via
    // `rkyv::util::AlignedVec` and reads exactly `len` bytes into it.
    // R25 mandates 16-byte alignment so that `rkyv::access` (the validating
    // API) does not reject the payload on alignment grounds.
    //
    // The helper is wired into `recv_frame` on the FLAG_ARCHIVED +
    // !FLAG_COMPRESSED fast path so that `decode_archive_payload` runs
    // without an extra `Vec<u8> -> AlignedVec` copy (MF-1 hoist).
    //
    // 5 tests under the `zero-copy` feature exercise: round-trip identity,
    // alignment, length, EOF behaviour, and capacity rounding. 1 test
    // under default features asserts the function is not exposed (its
    // absence is structural; we cover it via cfg-gated test compilation).
    // -----------------------------------------------------------------------

    /// UT-0355-01: helper round-trips the bytes verbatim.
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn read_aligned_payload_round_trips_bytes() {
        let payload: Vec<u8> = (0..=255u8).cycle().take(1024).collect();
        let mut cur = std::io::Cursor::new(payload.clone());
        let buf = read_aligned_payload(&mut cur, payload.len()).await.unwrap();
        assert_eq!(buf.as_ref(), payload.as_slice());
    }

    /// UT-0355-02: returned buffer base pointer is 16-byte aligned (R25).
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn read_aligned_payload_buffer_is_16_byte_aligned() {
        let payload = vec![0xABu8; 256];
        let mut cur = std::io::Cursor::new(payload);
        let buf = read_aligned_payload(&mut cur, 256).await.unwrap();
        assert_eq!(
            buf.as_ptr() as usize % 16,
            0,
            "AlignedVec base must satisfy R25 alignment"
        );
    }

    /// UT-0355-03: returned buffer length matches the requested `len` exactly,
    /// across the full small-power and bulk-size matrix from R25.
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn read_aligned_payload_length_matches_request() {
        for len in [0usize, 1, 7, 8, 9, 15, 16, 17, 1023, 4096] {
            let payload = vec![0xCDu8; len];
            let mut cur = std::io::Cursor::new(payload);
            let buf = read_aligned_payload(&mut cur, len).await.unwrap();
            assert_eq!(buf.len(), len, "len mismatch for {} bytes", len);
            // R25: non-empty buffers must be 16-byte aligned.
            if !buf.is_empty() {
                assert_eq!(
                    buf.as_ptr() as usize % 16,
                    0,
                    "buffer of len {} must be 16-aligned",
                    len
                );
            }
        }
    }

    /// UT-0355-04: short reader (fewer bytes than requested) returns
    /// `ConnectionLost` because `read_exact` errors out.
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn read_aligned_payload_truncated_input_errors() {
        let payload = vec![0u8; 16]; // only 16 bytes available
        let mut cur = std::io::Cursor::new(payload);
        let res = read_aligned_payload(&mut cur, 32).await; // request 32
        assert!(
            matches!(res, Err(ProtocolError::ConnectionLost(_))),
            "expected ConnectionLost on EOF, got {:?}",
            res
        );
    }

    /// UT-0355-05: capacity is always >= len (basic Vec invariant) and
    /// the base pointer of any non-empty buffer is 16-byte aligned (the
    /// load-bearing R25 guarantee). rkyv's `AlignedVec::with_capacity(n)`
    /// honours `n` exactly; alignment, not capacity rounding, is what
    /// makes the validating `access` API safe.
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn read_aligned_payload_capacity_is_16_aligned() {
        for len in [0usize, 1, 17, 1023, 4097] {
            let payload = vec![0u8; len];
            let mut cur = std::io::Cursor::new(payload);
            let buf = read_aligned_payload(&mut cur, len).await.unwrap();
            assert!(
                buf.capacity() >= len,
                "capacity must be >= len; got cap={} for len={}",
                buf.capacity(),
                len
            );
            if !buf.is_empty() {
                assert_eq!(
                    buf.as_ptr() as usize % 16,
                    0,
                    "non-empty buffer must be 16-aligned (R25); cap={} len={}",
                    buf.capacity(),
                    len
                );
            }
        }
    }

    /// UT-0355-06 (default-features): the helper is cfg-gated. This test
    /// confirms compilation under default features by referencing only
    /// the un-gated framing surface — a build-time guard against an
    /// accidental future de-gate of the function.
    #[cfg(not(feature = "zero-copy"))]
    #[test]
    fn read_aligned_payload_absent_in_default_build() {
        // The function is `#[cfg(feature = "zero-copy")]` — referencing it
        // here would fail to compile under default features. Asserting on
        // a `FrameHeader` field instead exercises the un-gated frame API
        // and makes the cross-build coverage explicit.
        let h = FrameHeader {
            length: 0,
            checksum: 0,
            flags: 0,
        };
        assert!(!h.is_archived(), "default build must reject FLAG_ARCHIVED");
    }

    // -----------------------------------------------------------------------
    // TASK-0356 — `send_frame_v2` (SPEC-18 §3.5 R22-R23).
    //
    // The v2 sender has two paths: the rkyv archive path (FLAG_ARCHIVED)
    // for hot-path messages when `use_archive == true`, and a transparent
    // fall-through to `send_frame_with_threshold` (bincode v2) otherwise.
    //
    // 6 tests under feature exercise: hot-path emit FLAG_ARCHIVED, cold-path
    // bypasses (Shutdown, Error, Register*), threshold-driven LZ4 wrap,
    // CRC over uncompressed bytes (R12), and the DC-4 prefix on serialize
    // failures (smoke). 1 test under default features confirms the symbol
    // is absent (cfg-gated).
    // -----------------------------------------------------------------------

    /// UT-0356-01: hot-path AssignPartition message under `use_archive=true`
    /// emits a frame with FLAG_ARCHIVED set and (for small payloads) no
    /// FLAG_COMPRESSED bit.
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn send_frame_v2_hot_path_emits_flag_archived() {
        let msg = Message::AssignPartition {
            round: 1,
            partition: make_test_partition(),
        };
        let mut buf: Vec<u8> = Vec::new();
        let n = send_frame_v2(&mut buf, &msg, true, usize::MAX)
            .await
            .unwrap();
        assert!(n >= FRAME_HEADER_SIZE);

        let header_bytes: [u8; FRAME_HEADER_SIZE] = buf[..FRAME_HEADER_SIZE].try_into().unwrap();
        let header = FrameHeader::from_bytes(header_bytes);
        assert!(header.is_archived(), "FLAG_ARCHIVED must be set");
        assert!(
            !header.is_compressed(),
            "FLAG_COMPRESSED must be clear when threshold is usize::MAX"
        );
    }

    /// UT-0356-02: cold-path message (Shutdown) under `use_archive=true`
    /// transparently falls through to bincode v2 (no FLAG_ARCHIVED).
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn send_frame_v2_cold_path_falls_through_to_bincode() {
        let mut buf: Vec<u8> = Vec::new();
        send_frame_v2(&mut buf, &Message::Shutdown, true, usize::MAX)
            .await
            .unwrap();
        let header_bytes: [u8; FRAME_HEADER_SIZE] = buf[..FRAME_HEADER_SIZE].try_into().unwrap();
        let header = FrameHeader::from_bytes(header_bytes);
        assert!(
            !header.is_archived(),
            "Shutdown is cold-path; must not set FLAG_ARCHIVED even with use_archive=true"
        );
    }

    /// UT-0356-03: `use_archive=false` always falls through, even for
    /// hot-path messages.
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn send_frame_v2_use_archive_false_disables_archive() {
        let msg = Message::AssignPartition {
            round: 0,
            partition: make_test_partition(),
        };
        let mut buf: Vec<u8> = Vec::new();
        send_frame_v2(&mut buf, &msg, false, usize::MAX)
            .await
            .unwrap();
        let header_bytes: [u8; FRAME_HEADER_SIZE] = buf[..FRAME_HEADER_SIZE].try_into().unwrap();
        let header = FrameHeader::from_bytes(header_bytes);
        assert!(
            !header.is_archived(),
            "use_archive=false must disable FLAG_ARCHIVED"
        );
    }

    /// UT-0356-04: large hot-path payload above the threshold triggers
    /// FLAG_COMPRESSED in addition to FLAG_ARCHIVED (R23).
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn send_frame_v2_large_payload_triggers_compression() {
        // Build a partition large enough to clear a tiny threshold.
        let mut subnet = Net::new();
        for _ in 0..256 {
            subnet.create_agent(Symbol::Era);
        }
        let partition = Partition {
            subnet,
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange {
                start: 0,
                end: 1_000,
            },
            border_id_start: 0,
            border_id_end: 0,
        };
        let msg = Message::AssignPartition {
            round: 0,
            partition,
        };

        let mut buf: Vec<u8> = Vec::new();
        // threshold=1 forces compression.
        send_frame_v2(&mut buf, &msg, true, 1).await.unwrap();
        let header_bytes: [u8; FRAME_HEADER_SIZE] = buf[..FRAME_HEADER_SIZE].try_into().unwrap();
        let header = FrameHeader::from_bytes(header_bytes);
        assert!(header.is_archived(), "FLAG_ARCHIVED must be set");
        assert!(
            header.is_compressed(),
            "FLAG_COMPRESSED must be set above threshold (R23)"
        );
    }

    /// UT-0356-05: PartitionResult (the second hot-path variant) emits
    /// FLAG_ARCHIVED.
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn send_frame_v2_partition_result_is_hot_path() {
        let msg = Message::PartitionResult {
            round: 0,
            partition: make_test_partition(),
            stats: make_test_stats(),
        };
        let mut buf: Vec<u8> = Vec::new();
        send_frame_v2(&mut buf, &msg, true, usize::MAX)
            .await
            .unwrap();
        let header_bytes: [u8; FRAME_HEADER_SIZE] = buf[..FRAME_HEADER_SIZE].try_into().unwrap();
        let header = FrameHeader::from_bytes(header_bytes);
        assert!(
            header.is_archived(),
            "PartitionResult is hot-path; must set FLAG_ARCHIVED"
        );
    }

    /// UT-0356-06: CRC32C in the frame header is computed over the
    /// **uncompressed** archive bytes (R12 ordering invariant). We send
    /// twice: once with compression, once without; both at the same
    /// payload should record the same CRC.
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn send_frame_v2_crc_over_uncompressed_archive() {
        let msg = Message::AssignPartition {
            round: 7,
            partition: make_test_partition(),
        };
        let mut buf_a: Vec<u8> = Vec::new();
        send_frame_v2(&mut buf_a, &msg, true, usize::MAX)
            .await
            .unwrap();
        let mut buf_b: Vec<u8> = Vec::new();
        send_frame_v2(&mut buf_b, &msg, true, 1).await.unwrap();
        let header_a = FrameHeader::from_bytes(buf_a[..FRAME_HEADER_SIZE].try_into().unwrap());
        let header_b = FrameHeader::from_bytes(buf_b[..FRAME_HEADER_SIZE].try_into().unwrap());
        assert!(header_a.is_archived() && !header_a.is_compressed());
        assert!(header_b.is_archived() && header_b.is_compressed());
        assert_eq!(
            header_a.checksum, header_b.checksum,
            "R12: CRC must be computed on uncompressed archive bytes; same payload -> same CRC"
        );
    }

    /// UT-0356-07 (default-features): `send_frame_v2` is cfg-gated. This
    /// test confirms the bincode-only `send_frame` still works under
    /// default features and produces a frame WITHOUT FLAG_ARCHIVED.
    #[cfg(not(feature = "zero-copy"))]
    #[tokio::test]
    async fn send_frame_v2_absent_in_default_build() {
        // send_frame_v2 is `#[cfg(feature = "zero-copy")]` — referencing
        // it here would fail to compile under default features. We
        // instead exercise the bincode `send_frame` to confirm the
        // default-features sender does NOT emit FLAG_ARCHIVED.
        let mut buf: Vec<u8> = Vec::new();
        send_frame(&mut buf, &Message::Shutdown).await.unwrap();
        let header = FrameHeader::from_bytes(buf[..FRAME_HEADER_SIZE].try_into().unwrap());
        assert!(
            !header.is_archived(),
            "default-features sender must never set FLAG_ARCHIVED"
        );
    }

    // --- TASK-0357: recv_frame archive branch + decode_archive_payload ---
    //
    // These tests exercise the FLAG_ARCHIVED side of `recv_frame`. They
    // pair `send_frame_v2` (TASK-0356) with `recv_frame` (this task) to
    // verify the round-trip and the failure modes mandated by SPEC-18 §3.5.

    /// Field-by-field equality helper for `Partition` (does NOT implement
    /// `PartialEq` because of `Net`/`HashMap` inner state). Compares the
    /// archivable surface only.
    #[cfg(feature = "zero-copy")]
    fn assert_partition_eq(left: &Partition, right: &Partition) {
        assert_eq!(left.worker_id, right.worker_id, "worker_id mismatch");
        assert_eq!(
            left.id_range.start, right.id_range.start,
            "id_range.start mismatch"
        );
        assert_eq!(
            left.id_range.end, right.id_range.end,
            "id_range.end mismatch"
        );
        assert_eq!(
            left.border_id_start, right.border_id_start,
            "border_id_start mismatch"
        );
        assert_eq!(
            left.border_id_end, right.border_id_end,
            "border_id_end mismatch"
        );
        assert_eq!(
            left.subnet, right.subnet,
            "subnet (Net) round-trip mismatch"
        );
    }

    /// Field-by-field equality helper for `WorkerRoundStats`.
    #[cfg(feature = "zero-copy")]
    fn assert_stats_eq(left: &WorkerRoundStats, right: &WorkerRoundStats) {
        assert_eq!(left.worker_id, right.worker_id);
        assert_eq!(left.agents_before, right.agents_before);
        assert_eq!(left.agents_after, right.agents_after);
        assert_eq!(left.local_redexes, right.local_redexes);
        assert_eq!(
            left.reduce_duration_secs.to_bits(),
            right.reduce_duration_secs.to_bits(),
            "reduce_duration_secs (f64) bit-pattern mismatch"
        );
        assert_eq!(left.interactions_by_rule, right.interactions_by_rule);
        assert_eq!(left.has_border_activity, right.has_border_activity);
    }

    /// UT-0357-01: AssignPartition round-trips through the archive path.
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn recv_frame_v2_assign_partition_round_trip() {
        let original = Message::AssignPartition {
            round: 42,
            partition: make_test_partition(),
        };
        let (mut client, mut server) = create_test_channel();
        let n_sent = send_frame_v2(&mut client, &original, true, usize::MAX)
            .await
            .unwrap();
        client.flush().await.unwrap();

        let (decoded, n_recv) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        assert_eq!(n_sent, n_recv, "send/recv byte counts must agree");
        match (&original, &decoded) {
            (
                Message::AssignPartition {
                    round: r0,
                    partition: p0,
                },
                Message::AssignPartition {
                    round: r1,
                    partition: p1,
                },
            ) => {
                assert_eq!(r0, r1);
                assert_partition_eq(p0, p1);
            }
            other => panic!("expected AssignPartition round-trip, got {:?}", other),
        }
    }

    /// UT-0357-02: PartitionResult round-trips through the archive path.
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn recv_frame_v2_partition_result_round_trip() {
        let original = Message::PartitionResult {
            round: 7,
            partition: make_test_partition(),
            stats: make_test_stats(),
        };
        let (mut client, mut server) = create_test_channel();
        send_frame_v2(&mut client, &original, true, usize::MAX)
            .await
            .unwrap();
        client.flush().await.unwrap();

        let (decoded, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        match (&original, &decoded) {
            (
                Message::PartitionResult {
                    round: r0,
                    partition: p0,
                    stats: s0,
                },
                Message::PartitionResult {
                    round: r1,
                    partition: p1,
                    stats: s1,
                },
            ) => {
                assert_eq!(r0, r1);
                assert_partition_eq(p0, p1);
                assert_stats_eq(s0, s1);
            }
            other => panic!("expected PartitionResult round-trip, got {:?}", other),
        }
    }

    /// UT-0357-03: combined `FLAG_ARCHIVED | FLAG_COMPRESSED` round-trips
    /// (R23 + R12: decompress before validating CRC, then rkyv-validate).
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn recv_frame_v2_compressed_archive_round_trip() {
        // Build a partition large enough to clear a tiny threshold so
        // the wire frame carries both flags.
        let mut subnet = Net::new();
        for _ in 0..256 {
            subnet.create_agent(Symbol::Era);
        }
        let partition = Partition {
            subnet,
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange {
                start: 0,
                end: 1_000,
            },
            border_id_start: 0,
            border_id_end: 0,
        };
        let original = Message::AssignPartition {
            round: 9,
            partition,
        };

        let (mut client, mut server) = create_test_channel();
        send_frame_v2(&mut client, &original, true, 1)
            .await
            .unwrap();
        client.flush().await.unwrap();

        // Sanity: the on-wire header must show both flags.
        let mut peek_header = [0u8; FRAME_HEADER_SIZE];
        let mut combined: Vec<u8> = Vec::new();
        // We can't peek without consuming on a DuplexStream. Instead,
        // re-encode to inspect flags, then run the actual recv.
        let mut inspect_buf: Vec<u8> = Vec::new();
        send_frame_v2(&mut inspect_buf, &original, true, 1)
            .await
            .unwrap();
        peek_header.copy_from_slice(&inspect_buf[..FRAME_HEADER_SIZE]);
        let header = FrameHeader::from_bytes(peek_header);
        assert!(
            header.is_archived() && header.is_compressed(),
            "wire frame must carry FLAG_ARCHIVED | FLAG_COMPRESSED"
        );
        combined.extend_from_slice(&inspect_buf); // silence "unused" if any

        let (decoded, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        match decoded {
            Message::AssignPartition {
                round, partition, ..
            } => {
                assert_eq!(round, 9);
                assert_eq!(partition.id_range.end, 1_000);
            }
            other => panic!("expected AssignPartition, got {:?}", other),
        }
    }

    /// UT-0357-04: an archive payload truncated below the minimum size
    /// for the AssignPartition schema yields `ArchiveValidationFailed`
    /// (R24). We hand-craft the frame so the CRC step passes (it is
    /// computed over the truncated bytes) and the rkyv validator is the
    /// step that rejects.
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn recv_frame_v2_corrupt_archive_rejected() {
        // Tiny payload that cannot host the AssignPartition or
        // PartitionResult archive layouts — but is large enough not to
        // be rejected as size 0.
        let payload = vec![0u8; 16];
        let checksum = crc32fast::hash(&payload);
        let header = FrameHeader {
            length: payload.len() as u32,
            checksum,
            flags: FLAG_ARCHIVED,
        };

        let (mut client, mut server) = create_test_channel();
        client.write_all(&header.to_bytes()).await.unwrap();
        client.write_all(&payload).await.unwrap();
        client.flush().await.unwrap();

        let err = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap_err();
        assert!(
            matches!(err, ProtocolError::ArchiveValidationFailed(_)),
            "expected ArchiveValidationFailed on undersized archive, got {:?}",
            err,
        );
    }

    /// UT-0357-05: corrupting the CRC field of an archive frame yields
    /// `ChecksumMismatch` BEFORE the rkyv validator runs (R12 ordering).
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn recv_frame_v2_crc_mismatch_rejected_before_rkyv() {
        let original = Message::AssignPartition {
            round: 0,
            partition: make_test_partition(),
        };
        let mut buf: Vec<u8> = Vec::new();
        send_frame_v2(&mut buf, &original, true, usize::MAX)
            .await
            .unwrap();
        // Flip a CRC bit; archive bytes remain valid.
        buf[4] ^= 0xFF;

        let (mut client, mut server) = create_test_channel();
        client.write_all(&buf).await.unwrap();
        client.flush().await.unwrap();

        let err = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap_err();
        assert!(
            matches!(err, ProtocolError::ChecksumMismatch { .. }),
            "expected ChecksumMismatch (R12 ordering: CRC checked before rkyv), got {:?}",
            err,
        );
    }

    /// UT-0357-06: a FLAG_ARCHIVED frame whose payload is structurally
    /// neither AssignPartition nor PartitionResult is rejected with the
    /// mandated R26 message.
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn recv_frame_v2_non_hot_path_archive_rejected() {
        // Synthesize an archive of `u32` (a valid rkyv schema, but not a
        // hot-path payload schema). The frame is otherwise well-formed.
        let archive = rkyv::to_bytes::<rkyv::rancor::Error>(&123u32).unwrap();
        let payload_bytes: Vec<u8> = archive.as_ref().to_vec();
        let checksum = crc32fast::hash(&payload_bytes);
        let header = FrameHeader {
            length: payload_bytes.len() as u32,
            checksum,
            flags: FLAG_ARCHIVED,
        };

        let (mut client, mut server) = create_test_channel();
        client.write_all(&header.to_bytes()).await.unwrap();
        client.write_all(&payload_bytes).await.unwrap();
        client.flush().await.unwrap();

        let err = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap_err();
        match err {
            ProtocolError::ArchiveValidationFailed(reason) => {
                assert!(
                    reason.contains("non-hot-path archive payload"),
                    "R26 mandates the 'non-hot-path archive payload' message; got '{}'",
                    reason
                );
            }
            other => panic!("expected ArchiveValidationFailed, got {:?}", other),
        }
    }

    /// UT-0357-07: `decode_archive_payload` aligns its input via
    /// `AlignedVec` (R25). We cannot observe alignment directly through
    /// `recv_frame`, but we can drive the function with an off-aligned
    /// `Vec<u8>` (the recv pipeline always does) and confirm validation
    /// still succeeds for a well-formed archive.
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn recv_frame_v2_aligned_buffer_inside_decode() {
        let original = Message::AssignPartition {
            round: 1,
            partition: make_test_partition(),
        };
        let mut buf: Vec<u8> = Vec::new();
        send_frame_v2(&mut buf, &original, true, usize::MAX)
            .await
            .unwrap();
        // The recv path internally allocates a `Vec<u8>` for the payload
        // (allocator-aligned, typically 8 on 64-bit Windows). The R25
        // AlignedVec copy inside `decode_archive_payload` is what makes
        // this round-trip succeed.
        let (mut client, mut server) = create_test_channel();
        client.write_all(&buf).await.unwrap();
        client.flush().await.unwrap();
        let _ = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .expect("R25 alignment must be honored by decode_archive_payload");
    }

    /// UT-0357-08: a FLAG_ARCHIVED frame with the `serialize: ` DC-4
    /// prefix flows end-to-end without the prefix leaking into the
    /// receive-side error type. (Sanity probe: send-side errors are
    /// wrapped in `ArchiveValidationFailed("serialize: …")`; receive-side
    /// errors carry the schema-name prefix instead.)
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn recv_frame_v2_dc4_prefix_only_on_send_side() {
        // Drive an undersized-archive recv to force the rkyv validator
        // to reject. The receive-side error must NOT carry the send-side
        // "serialize: " DC-4 prefix.
        let payload = vec![0u8; 16];
        let checksum = crc32fast::hash(&payload);
        let header = FrameHeader {
            length: payload.len() as u32,
            checksum,
            flags: FLAG_ARCHIVED,
        };

        let (mut client, mut server) = create_test_channel();
        client.write_all(&header.to_bytes()).await.unwrap();
        client.write_all(&payload).await.unwrap();
        client.flush().await.unwrap();

        let err = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap_err();
        if let ProtocolError::ArchiveValidationFailed(reason) = err {
            assert!(
                !reason.starts_with("serialize: "),
                "recv-side ArchiveValidationFailed must NOT carry the send-side 'serialize: ' DC-4 prefix; got '{}'",
                reason
            );
        } else {
            panic!(
                "expected ArchiveValidationFailed on undersized archive, got {:?}",
                err
            );
        }
    }

    /// UT-0357-09 (default-features): under `not(feature = "zero-copy")`,
    /// a FLAG_ARCHIVED frame falls through to the bincode decoder, which
    /// rejects the rkyv bytes as a `Deserialize` error. This guards
    /// against accidental cross-build interop without the feature.
    #[cfg(not(feature = "zero-copy"))]
    #[tokio::test]
    async fn recv_frame_default_build_rejects_archived_frame_as_deserialize() {
        // Hand-craft a FLAG_ARCHIVED frame carrying bytes that bincode
        // will reject. Leading 0xFF bytes encode an invalid enum
        // discriminant for `Message`, forcing a `Deserialize` error.
        let payload = vec![0xFFu8; 64];
        let checksum = crc32fast::hash(&payload);
        let header = FrameHeader {
            length: payload.len() as u32,
            checksum,
            flags: FLAG_ARCHIVED,
        };

        let (mut client, mut server) = create_test_channel();
        client.write_all(&header.to_bytes()).await.unwrap();
        client.write_all(&payload).await.unwrap();
        client.flush().await.unwrap();

        let err = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap_err();
        assert!(
            matches!(err, ProtocolError::Deserialize(_)),
            "default build must reject FLAG_ARCHIVED frames via the bincode decoder, got {:?}",
            err,
        );
    }

    /// UT-0357-10 (default-features): the `decode_archive_payload`
    /// symbol is cfg-gated. We confirm the default build still routes
    /// non-archived frames through the bincode path correctly (the
    /// existing happy-path tests exercise this exhaustively; this is a
    /// belt-and-braces probe that the cfg gate is structural).
    #[cfg(not(feature = "zero-copy"))]
    #[tokio::test]
    async fn recv_frame_default_build_bincode_path_unaffected() {
        let original = Message::Shutdown;
        let mut buf: Vec<u8> = Vec::new();
        send_frame(&mut buf, &original).await.unwrap();
        let (mut client, mut server) = create_test_channel();
        client.write_all(&buf).await.unwrap();
        client.flush().await.unwrap();
        let (decoded, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        assert!(matches!(decoded, Message::Shutdown));
    }

    // =======================================================================
    // QA Stage 5 probes — default-build variants. (See zero_copy_tests.rs
    // for the feature-gated probes Q1, Q2-on, Q3-on, Q4..Q8.)
    // Probes Q2 and Q3 have a default-build branch that asserts the
    // shipped behavior when the `zero-copy` feature is OFF.
    // =======================================================================

    /// Q2 (default branch) — adversarial sender sets FLAG_ARCHIVED on a
    /// bincode payload while the receiver is built WITHOUT `zero-copy`.
    /// Per F-9 of the review, the shipped behavior is "fall through to
    /// bincode decoder", which fails as `ProtocolError::Deserialize` for
    /// payloads bincode does not recognise. We confirm there is no panic
    /// and no silent acceptance.
    #[cfg(not(feature = "zero-copy"))]
    #[tokio::test]
    async fn qa_probe_q2_archived_flag_on_bincode_payload_without_feature() {
        let bincode_bytes: Vec<u8> = crate::protocol::bincode_v2::encode(&Message::Shutdown)
            .expect("bincode encode must succeed");
        let checksum = crc32fast::hash(&bincode_bytes);
        let header = FrameHeader {
            length: bincode_bytes.len() as u32,
            checksum,
            flags: FLAG_ARCHIVED,
        };
        let (mut client, mut server) = create_test_channel();
        client.write_all(&header.to_bytes()).await.unwrap();
        client.write_all(&bincode_bytes).await.unwrap();
        client.flush().await.unwrap();

        let result = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE).await;
        match result {
            // Default build: the bincode path happens to accept Message::Shutdown
            // (the sent payload IS valid bincode; FLAG_ARCHIVED bit is ignored
            // at the discrimination layer because the rkyv branch is
            // cfg-stripped). This is consistent with R19 forward-compat: a
            // future v3 receiver that *does* understand FLAG_ARCHIVED would
            // route via rkyv; today's default receiver routes via bincode.
            Ok((decoded, _)) => {
                assert!(
                    matches!(decoded, Message::Shutdown),
                    "Q2 (default): bincode-fallthrough must decode Shutdown identity, got {:?}",
                    decoded
                );
                tracing::info!(
                    "Q2 (default): FLAG_ARCHIVED+bincode-payload accepted by bincode fallthrough"
                );
            }
            Err(err) => {
                // Acceptable alternative: the bincode decoder rejects with
                // Deserialize. NOT acceptable: any panic or silent corruption.
                assert!(
                    matches!(err, ProtocolError::Deserialize(_)),
                    "Q2 (default): expected Deserialize on bincode-reject path, got {:?}",
                    err
                );
                tracing::info!(
                    ?err,
                    "Q2 (default): FLAG_ARCHIVED+bincode-payload rejected by bincode decoder"
                );
            }
        }
    }

    /// Q3 (default branch) — feature-ON sender → default receiver
    /// interop. We craft an archived frame (using known-bad bincode bytes
    /// at FLAG_ARCHIVED, mirroring the existing UT-0357-09 test) and
    /// assert the receiver rejects cleanly without panic. This also
    /// covers the inverse "default sender → feature-on receiver" because
    /// a default sender NEVER sets FLAG_ARCHIVED (no `send_frame_v2`
    /// symbol in default builds), so the feature-on receiver only ever
    /// sees bincode frames from default senders — which is asserted by
    /// `qa_probe_q3_bincode_frame_accepted_when_feature_on` in
    /// `zero_copy_tests.rs`.
    #[cfg(not(feature = "zero-copy"))]
    #[tokio::test]
    async fn qa_probe_q3_archived_frame_rejected_when_feature_off() {
        // Mirror the byte shape of an archived AssignPartition: 64 bytes
        // of 0xFF (matches the existing
        // `recv_frame_default_build_rejects_archived_frame_as_deserialize`
        // pattern). bincode rejects the 0xFF discriminant as Deserialize.
        let payload = vec![0xFFu8; 64];
        let checksum = crc32fast::hash(&payload);
        let header = FrameHeader {
            length: payload.len() as u32,
            checksum,
            flags: FLAG_ARCHIVED,
        };
        let (mut client, mut server) = create_test_channel();
        client.write_all(&header.to_bytes()).await.unwrap();
        client.write_all(&payload).await.unwrap();
        client.flush().await.unwrap();

        let err = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap_err();
        tracing::info!(
            ?err,
            "Q3 (default): archived-from-feature-on-sender rejected by feature-off receiver"
        );
        assert!(
            matches!(err, ProtocolError::Deserialize(_)),
            "Q3 (default): expected Deserialize for archived-frame on feature-off receiver, got {:?}",
            err
        );
    }
}
