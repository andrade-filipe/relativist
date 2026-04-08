# TEST-SPEC-0095: Implement in-memory transport for testing

**Task:** TASK-0095
**Spec:** SPEC-06
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Round-trip Shutdown through in-memory channel

**Type:** Unit test (async)
**Input:**
```
let (mut client, mut server) = create_test_channel();
send_frame(&mut client, &Message::Shutdown).await.unwrap();
drop(client);
let (msg, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE).await.unwrap();
```
**Expected:** `matches!(msg, Message::Shutdown)`
**Verifies:** Basic in-memory transport round-trip

### T2: Round-trip AssignPartition with test data

**Type:** Unit test (async)
**Input:**
```
let partition = test_partition_with_3_agents();
let msg = Message::AssignPartition { round: 42, partition: partition.clone() };
send_frame(&mut client, &msg).await.unwrap();
let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE).await.unwrap();
```
**Expected:** Received message contains `round == 42` and partition matching the original
**Verifies:** Complex message types survive serialization round-trip

### T3: Round-trip all 7 Message variants

**Type:** Unit test (async)
**Input:** For each variant (`AssignPartition`, `Shutdown`, `PartitionResult`, `Error`, `Register`, `RegisterAck`, `RegisterNack`), send through the channel and receive
**Expected:** All 7 variants round-trip successfully
**Verifies:** SPEC-06 v3 -- all message types work through in-memory transport

### T4: Checksum mismatch detection via corrupted payload

**Type:** Unit test (async)
**Input:**
```
// Write valid header manually, then write corrupted payload (one byte flipped)
let payload = bincode::serialize(&Message::Shutdown).unwrap();
let checksum = crc32fast::hash(&payload);
let header = FrameHeader { length: payload.len() as u32, checksum };
writer.write_all(&header.to_bytes()).await.unwrap();
let mut corrupted = payload.clone();
corrupted[0] ^= 0xFF;  // flip first byte
writer.write_all(&corrupted).await.unwrap();
```
**Expected:** `recv_frame` returns `Err(ProtocolError::ChecksumMismatch { .. })`
**Verifies:** R29 -- CRC32 integrity check catches corruption

### T5: PayloadTooLarge rejection via in-memory channel

**Type:** Unit test (async)
**Input:** Write a header declaring `length = 500_000_000`, pass `max_payload_size = 268_435_456`
**Expected:** `Err(ProtocolError::PayloadTooLarge { size: 500_000_000, max: 268_435_456 })`
**Verifies:** R9 -- size validation before allocation

---

## Edge Cases

### E1: Multiple messages in sequence through same channel

**Verify:** Send 5 different messages sequentially through the same duplex stream pair; all 5 are received correctly in order.
**Why:** Tests that framing correctly delineates message boundaries.

### E2: Large payload near the max size limit

**Verify:** A message with payload size close to `DEFAULT_MAX_PAYLOAD_SIZE` (e.g., 250 MiB) can be sent and received through the in-memory channel (with a sufficiently large duplex buffer).
**Why:** Tests that the framing handles large payloads without truncation.
