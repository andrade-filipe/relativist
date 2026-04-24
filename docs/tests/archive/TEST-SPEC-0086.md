# TEST-SPEC-0086: Implement recv_frame function

**Task:** TASK-0086
**Spec:** SPEC-06
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Round-trip send then receive

**Type:** Unit test (async)
**Input:**
```
let (mut client, mut server) = tokio::io::duplex(1_048_576);
send_frame(&mut client, &Message::Shutdown).await.unwrap();
drop(client);
let (msg, total_bytes) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE).await.unwrap();
```
**Expected:** `msg` matches `Message::Shutdown`; `total_bytes == FRAME_HEADER_SIZE + payload_len`
**Verifies:** R6, R14 -- `recv_frame(send_frame(msg)) == msg`

### T2: Corrupted payload returns ChecksumMismatch

**Type:** Unit test (async)
**Input:** Write a valid header manually, then write payload bytes with one byte flipped
**Expected:** `Err(ProtocolError::ChecksumMismatch { expected, computed })` where `expected != computed`
**Verifies:** R29 -- CRC32 checksum verification rejects corrupted payloads

### T3: Payload exceeding max_payload_size returns PayloadTooLarge

**Type:** Unit test (async)
**Input:** Write a header with `length = 500_000_000` (> 268_435_456) followed by minimal bytes
**Expected:** `Err(ProtocolError::PayloadTooLarge { size: 500_000_000, max: 268_435_456 })`
**Verifies:** R9 -- size check BEFORE allocation to prevent OOM

### T4: Truncated header returns ConnectionLost

**Type:** Unit test (async)
**Input:** Write only 4 bytes (less than 8-byte header), then close the stream
**Expected:** `Err(ProtocolError::ConnectionLost(_))`
**Verifies:** Graceful handling of incomplete header reads

### T5: Truncated payload returns ConnectionLost

**Type:** Unit test (async)
**Input:** Write a valid 8-byte header declaring `length = 100`, then write only 50 bytes and close
**Expected:** `Err(ProtocolError::ConnectionLost(_))`
**Verifies:** `read_exact` detects incomplete payload

### T6: Total bytes returned matches header + length

**Type:** Unit test (async)
**Input:** Send a `Message::AssignPartition` and receive it
**Expected:** `total_bytes == FRAME_HEADER_SIZE + declared_length`
**Verifies:** R34 -- bytes count includes header for network metrics

---

## Edge Cases

### E1: PayloadTooLarge field is `size`, not `declared`

**Verify:** The error uses `size` as the field name: `PayloadTooLarge { size: _, max: _ }`.
**How:** Pattern match on the error variant using `size` field name.
**Why:** SPEC-06 v3 renamed `declared` to `size`.

### E2: Zero-length payload round-trips correctly

**Verify:** A message that serializes to an empty or minimal payload (e.g., `Shutdown`) can be received correctly with `length` near 0.
**Why:** Zero-length payloads test the boundary condition of CRC32 on empty data.
