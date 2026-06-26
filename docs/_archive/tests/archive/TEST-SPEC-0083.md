# TEST-SPEC-0083: Define FrameHeader struct and framing constants

**Task:** TASK-0083
**Spec:** SPEC-06
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Round-trip header serialization

**Type:** Unit test
**Input:**
```
let header = FrameHeader { length: 1024, checksum: 0xDEADBEEF };
let bytes = header.to_bytes();
let decoded = FrameHeader::from_bytes(bytes);
```
**Expected:** `decoded.length == 1024`, `decoded.checksum == 0xDEADBEEF`
**Verifies:** `from_bytes(to_bytes(h)) == h`

### T2: Little-endian byte order

**Type:** Unit test
**Input:**
```
let header = FrameHeader { length: 0x04030201, checksum: 0x08070605 };
let bytes = header.to_bytes();
```
**Expected:** `bytes == [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]`
**Verifies:** R6 -- little-endian encoding for both length and checksum fields

### T3: Constants have expected values

**Type:** Unit test
**Input:** Read `FRAME_HEADER_SIZE` and `DEFAULT_MAX_PAYLOAD_SIZE`
**Expected:** `FRAME_HEADER_SIZE == 8`, `DEFAULT_MAX_PAYLOAD_SIZE == 268_435_456`
**Verifies:** R7 (8-byte header) and R9 (256 MiB max payload)

### T4: FrameHeader derives Debug, Clone, Copy

**Type:** Unit test
**Input:**
```
let h = FrameHeader { length: 0, checksum: 0 };
let h2 = h;  // Copy
let h3 = h.clone();  // Clone
let _ = format!("{:?}", h);  // Debug
```
**Expected:** All operations succeed without error
**Verifies:** Derive attributes on FrameHeader

---

## Edge Cases

### E1: Zero-length payload header

**Type:** Unit test
**Input:** `FrameHeader { length: 0, checksum: 0 }.to_bytes()`
**Expected:** `[0, 0, 0, 0, 0, 0, 0, 0]` -- all zeros, round-trips correctly
**Why:** Shutdown message may have very small payload; length 0 is a valid edge case.

### E2: Maximum values

**Type:** Unit test
**Input:** `FrameHeader { length: u32::MAX, checksum: u32::MAX }.to_bytes()`
**Expected:** `[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]` -- round-trips correctly
**Why:** Ensures no overflow or truncation at the u32 boundary.

### E3: FrameHeader does NOT derive Serialize/Deserialize

**Verify:** `serde::Serialize` and `serde::Deserialize` are NOT derived on `FrameHeader`.
**Why:** Header is manually encoded as raw bytes, not via bincode/serde (per TASK-0083 notes).
