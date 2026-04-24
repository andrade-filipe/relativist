# TEST-SPEC-0128: Implement token validation in coordinator accept flow

**Task:** TASK-0128
**Spec:** SPEC-10
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Tier 1 -- Register with no token accepted

**Input:** Call `authenticate_worker` with `expected_token: None` and `RegisterPayload { protocol_version: 1, auth_token: None }`
**Expected:** Returns `Ok(())` and sends `RegisterAck` to the stream
**Verifies:** T3 -- Tier 1 accepts without token

### T2: Tier 1 -- Register with token accepted (token ignored)

**Input:** Call `authenticate_worker` with `expected_token: None` and `RegisterPayload { protocol_version: 1, auth_token: Some([1u8; 32]) }`
**Expected:** Returns `Ok(())` -- token is ignored in Tier 1
**Verifies:** T3 -- Tier 1 ignores provided tokens

### T3: Tier 2 -- correct token accepted

**Input:** Generate token, call `authenticate_worker` with `expected_token: Some(&token)` and matching `auth_token: Some(token.as_bytes().clone())`
**Expected:** Returns `Ok(())` and sends `RegisterAck { worker_id }` to the stream
**Verifies:** T3 -- correct token in Tier 2

### T4: Tier 2 -- wrong token rejected

**Input:** Call `authenticate_worker` with `expected_token: Some(&token_a)` and `auth_token: Some(token_b_bytes)` where tokens differ
**Expected:** Returns `Err(SecurityError::AuthFailed)` and sends `RegisterNack { reason: "authentication failed" }`
**Verifies:** T3, R16 -- wrong token rejection

### T5: Tier 2 -- missing token rejected

**Input:** Call `authenticate_worker` with `expected_token: Some(&token)` and `auth_token: None`
**Expected:** Returns `Err(SecurityError::AuthFailed)` and sends `RegisterNack`
**Verifies:** T3 -- missing token when required

### T6: Non-Register first message rejected

**Input:** Send a `Message::Shutdown` (or any non-Register variant) as first message
**Expected:** Returns error and sends `RegisterNack { reason: "protocol error" }`
**Verifies:** T7, R35 -- non-Register messages rejected

### T7: Unrecognized protocol version rejected

**Input:** Send `RegisterPayload { protocol_version: 255, auth_token: None }`
**Expected:** Connection closed immediately (no RegisterNack sent)
**Verifies:** R36 -- fast rejection of incompatible clients

---

## Edge Cases

### E1: Rejected worker does not block subsequent connections

**Verify:** After one worker is rejected (wrong token), the accept loop continues and a subsequent worker with the correct token is accepted.
**Why:** T7 -- rejection must not poison the accept flow.

### E2: Logging does not contain token value

**Verify:** When a worker is rejected, the WARN log includes the remote address but NOT the rejected token bytes or base64.
**Why:** R16 -- token values must not appear in logs after rejection.
