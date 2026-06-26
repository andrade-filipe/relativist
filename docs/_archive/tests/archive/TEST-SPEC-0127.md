# TEST-SPEC-0127: Extend Message enum with Register, RegisterAck, RegisterNack

**Task:** TASK-0127
**Spec:** SPEC-10
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Register with no token roundtrips via bincode

**Input:** `Message::Register(RegisterPayload { protocol_version: 1, auth_token: None })` -- serialize with bincode then deserialize
**Expected:** Deserialized message matches original
**Verifies:** R14, R19 -- wire format serialization

### T2: Register with token roundtrips via bincode

**Input:** `Message::Register(RegisterPayload { protocol_version: 1, auth_token: Some([0u8; 32]) })` -- serialize then deserialize
**Expected:** Deserialized message matches original with all 32 bytes preserved
**Verifies:** Token transmission as raw bytes

### T3: RegisterAck roundtrips via bincode

**Input:** `Message::RegisterAck(RegisterAckPayload { worker_id: 42 })` -- serialize then deserialize
**Expected:** Deserialized `worker_id` is `42`
**Verifies:** R17 -- worker ID assignment

### T4: RegisterNack roundtrips via bincode

**Input:** `Message::RegisterNack(RegisterNackPayload { reason: "authentication failed".into() })` -- serialize then deserialize
**Expected:** Deserialized `reason` is `"authentication failed"`
**Verifies:** R16, R35 -- generic rejection message

### T5: Message enum has exactly 7 variants

**Input:** Count the variants in the `Message` enum (4 from SPEC-06 + 3 from SPEC-10)
**Expected:** 7 total variants
**Verifies:** No accidental variants added or removed

### T6: RegisterPayload serialized size is compact

**Input:** Serialize `RegisterPayload { protocol_version: 1, auth_token: None }` with bincode
**Expected:** Serialized size < 20 bytes (enum tag + u8 + option discriminant)
**Verifies:** Compact wire format

---

## Edge Cases

### E1: RegisterNack reason is always generic for auth failures

**Verify:** The `RegisterNackPayload` documentation specifies that `reason` MUST be `"authentication failed"` for auth errors, never revealing whether the token was wrong, missing, or malformed.
**Why:** R35 -- no internal state leakage.

### E2: auth_token uses raw bytes, not AuthToken struct

**Verify:** `RegisterPayload.auth_token` is `Option<[u8; 32]>`, NOT `Option<AuthToken>`.
**Why:** R14 -- wire protocol uses raw bytes; AuthToken is a domain type.
