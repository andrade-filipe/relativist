# TEST-SPEC-0136: Verify message size pre-validation in recv_frame (SPEC-06 integration)

**Task:** TASK-0136
**Spec:** SPEC-10
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Frame with declared length 256 MiB + 1 returns PayloadTooLarge

**Input:** Write a frame header with `length = 268_435_457` (256 MiB + 1) to a mock stream; call `recv_frame` with `max_payload_size = 268_435_456`
**Expected:** `Err(ProtocolError::PayloadTooLarge { size: 268_435_457, max: 268_435_456 })`
**Verifies:** T5 -- oversized payload rejection

### T2: Frame with declared length exactly 256 MiB is accepted

**Input:** Write a valid frame with `length = 268_435_456` and matching payload; call `recv_frame` with `max_payload_size = 268_435_456`
**Expected:** `Ok(...)` -- frame accepted
**Verifies:** Boundary value accepted

### T3: Frame with declared length 0 is accepted

**Input:** Write a frame header with `length = 0` and empty payload
**Expected:** `Ok(...)` -- zero-length frame accepted
**Verifies:** Empty frames are valid

### T4: Memory not allocated for oversized payloads

**Input:** Write a frame header with `length = u32::MAX` (4 GiB); call `recv_frame`
**Expected:** Returns `Err(ProtocolError::PayloadTooLarge {...})` without allocating 4 GiB of memory (process does not OOM)
**Verifies:** R29 -- pre-allocation size check

---

## Edge Cases

### E1: Error variant name is PayloadTooLarge (not MessageTooLarge)

**Verify:** `ProtocolError::PayloadTooLarge` is the variant name, matching SPEC-06 Section 4.4.
**Why:** Revised v2 renamed from `MessageTooLarge` to `PayloadTooLarge`.

### E2: No max_message_size in SecurityConfig

**Verify:** `SecurityConfig` does NOT have a `max_message_size` field. The size limit comes from `NodeConfig.max_payload_size` (SPEC-06 R9).
**Why:** SC-004 -- message size is owned by SPEC-06, not SPEC-10.
