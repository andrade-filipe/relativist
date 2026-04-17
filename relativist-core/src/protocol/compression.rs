//! LZ4 frame-compression helpers (SPEC-18 §3.3 — item 2.23).
//!
//! `compress_payload` produces an LZ4-encoded byte buffer that begins
//! with a little-endian `u32` declaring the uncompressed size; this
//! lets `decompress_payload` allocate exactly the right output buffer
//! and refuse oversized headers (defense against decompression bombs).
//!
//! These helpers are deliberately free functions — they have no
//! coupling to the frame layer, so they can be unit-tested in isolation
//! and reused by any future codepath that needs payload compression
//! (e.g. the persistent pipeline-state cache or the rkyv archive flag).

use lz4_flex::block::{compress, decompress};

/// Hard cap on the declared uncompressed size accepted by
/// `decompress_payload`. 1 GiB matches `DEFAULT_MAX_PAYLOAD_SIZE` and
/// prevents a malicious 4-byte header from forcing a multi-gigabyte
/// allocation before the actual data has been received.
const MAX_DECOMPRESSED_SIZE: usize = 1_073_741_824;

/// Compress an arbitrary byte buffer with LZ4 block format.
///
/// Output layout: `[u32 LE uncompressed_size] [LZ4 block bytes…]`.
pub fn compress_payload(data: &[u8]) -> Vec<u8> {
    let original_len = data.len() as u32;
    let body = compress(data);
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&original_len.to_le_bytes());
    out.extend_from_slice(&body);
    out
}

/// Decompress an LZ4 buffer produced by `compress_payload`.
///
/// Returns the original bytes or an error string suitable for the
/// `ProtocolError::DecompressionFailed` variant.
pub fn decompress_payload(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 4 {
        return Err(format!(
            "payload too short to carry size prefix: {} bytes",
            data.len(),
        ));
    }
    let declared = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    if declared > MAX_DECOMPRESSED_SIZE {
        return Err(format!(
            "declared uncompressed size {} exceeds cap {}",
            declared, MAX_DECOMPRESSED_SIZE,
        ));
    }
    decompress(&data[4..], declared).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R3: `compress_payload` + `decompress_payload` round-trip on
    /// repetitive data and yield a smaller compressed buffer.
    #[test]
    fn lz4_helper_round_trip() {
        let original: Vec<u8> = b"the quick brown fox ".repeat(64); // ~1.3 KB
        let compressed = compress_payload(&original);
        let decompressed = decompress_payload(&compressed).unwrap();
        assert_eq!(decompressed, original);
        assert!(
            compressed.len() < original.len(),
            "expected size reduction, got compressed={} original={}",
            compressed.len(),
            original.len(),
        );
    }

    #[test]
    fn lz4_helper_round_trip_empty() {
        let compressed = compress_payload(&[]);
        let decompressed = decompress_payload(&compressed).unwrap();
        assert!(decompressed.is_empty());
    }

    #[test]
    fn lz4_helper_truncated_buffer_is_error() {
        let err = decompress_payload(&[0x01, 0x02]).unwrap_err();
        assert!(
            err.contains("size prefix"),
            "unexpected error message: {}",
            err,
        );
    }

    #[test]
    fn lz4_helper_oversized_declared_size_is_error() {
        // u32::MAX as declared size — must be rejected before any allocation.
        let mut buf = Vec::new();
        buf.extend_from_slice(&u32::MAX.to_le_bytes());
        buf.push(0); // dummy block byte
        let err = decompress_payload(&buf).unwrap_err();
        assert!(err.contains("exceeds cap"), "got: {}", err);
    }

    /// R4: ratio ≥ 2.0× on a payload built to be highly redundant.
    /// Uses raw bytes (not a serialised partition) to avoid pulling
    /// `partition` and `merge` into this lightweight module.
    #[test]
    fn lz4_ratio_ge_two_on_redundant_payload() {
        let payload = vec![0u8; 16 * 1024];
        let compressed = compress_payload(&payload);
        let ratio = payload.len() as f64 / compressed.len() as f64;
        assert!(
            ratio >= 2.0,
            "ratio {:.2}× below SHOULD threshold (uncompressed={}, compressed={})",
            ratio,
            payload.len(),
            compressed.len(),
        );
    }
}
