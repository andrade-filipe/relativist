# TEST-SPEC-0085: Implement send_frame function

**Task:** TASK-0085
**Spec:** SPEC-06
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Send Shutdown to in-memory buffer and verify header

**Type:** Unit test (async)
**Input:**
```
let (mut client, mut server) = tokio::io::duplex(1_048_576);
let bytes_written = send_frame(&mut client, &Message::Shutdown).await.unwrap();
```
**Expected:**
- `bytes_written == FRAME_HEADER_SIZE + serialized_shutdown_len`
- First 4 bytes of the buffer (length field, LE) match the serialized payload length
- Next 4 bytes (checksum field, LE) match `crc32fast::hash(&serialized_payload)`
**Verifies:** R6, R7, R10 -- correct header format with CRC32C

### T2: CRC32C checksum is correct

**Type:** Unit test (async)
**Input:** Send `Message::Shutdown`, read raw bytes from the other end of duplex stream
**Expected:** Manually compute CRC32C of the payload bytes; it matches the checksum in the header
**Verifies:** R10 -- CRC32C checksum computation

### T3: Total bytes returned equals header + payload

**Type:** Unit test (async)
**Input:** Send `Message::AssignPartition { round: 0, partition: test_partition() }`
**Expected:** `bytes_written == 8 + bincode::serialize(&msg).unwrap().len()`
**Verifies:** R34 -- return value includes header bytes for metrics

### T4: Serialization failure returns ProtocolError::Serialize

**Type:** Unit test (async)
**Input:** If possible, trigger a bincode serialization error (e.g., with a type that fails to serialize). Otherwise, verify the error path exists in the code.
**Expected:** `Err(ProtocolError::Serialize(_))`
**Verifies:** Error handling for bincode failures

### T5: Write failure returns ProtocolError::ConnectionLost

**Type:** Unit test (async)
**Input:** Drop the reader side of the duplex stream before calling `send_frame`
**Expected:** `Err(ProtocolError::ConnectionLost(_))`
**Verifies:** R25 -- I/O errors map to ConnectionLost (renamed from Io in SPEC-06 v3)

---

## Edge Cases

### E1: Flush is called after write

**Verify:** The function calls `writer.flush().await` after writing header + payload.
**How:** Use a mock writer that tracks flush calls, or verify via code review.
**Why:** Without flush, data may remain buffered and the receiver will hang.

### E2: Bincode uses explicit configuration

**Verify:** Serialization uses `bincode::config::standard().with_little_endian().with_fixed_int_encoding()` (or the v1 equivalent `bincode::serialize`).
**How:** Code review or test that integers are encoded as fixed-width LE bytes.
**Why:** R11 -- SPEC-06 v3 mandates explicit bincode config.
