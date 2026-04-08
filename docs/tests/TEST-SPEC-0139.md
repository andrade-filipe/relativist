# TEST-SPEC-0139: Security integration tests

**Task:** TASK-0139
**Spec:** SPEC-10, SPEC-08
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Token generation uniqueness (SPEC-10 T1)

**Input:** `let t1 = AuthToken::generate(); let t2 = AuthToken::generate(); t1.verify(&t2)`
**Expected:** `false`
**Verifies:** SEC-1 -- CSPRNG uniqueness

### T2: Token base64 roundtrip (SPEC-10 T2)

**Input:** `let t = AuthToken::generate(); let d = AuthToken::from_base64(&t.to_base64()).unwrap(); t.verify(&d)`
**Expected:** `true`
**Verifies:** SEC-2 -- encoding roundtrip

### T3: Token validation scenarios (SPEC-10 T3)

**Input:** Multiple scenarios: correct token -> RegisterAck; wrong token -> RegisterNack; missing token when required -> RegisterNack; token in Tier 1 -> accepted
**Expected:** Each scenario produces the expected result
**Verifies:** SEC-3 -- authentication flow

### T4: TLS handshake (SPEC-10 T4, feature-gated)

**Input:** Valid cert+CA -> success; wrong CA -> failure
**Expected:** Handshake succeeds/fails as expected
**Verifies:** SEC-4 -- TLS integration (`#[cfg(feature = "tls")]`)

### T5: Oversized message rejected (SPEC-10 T5)

**Input:** Frame with declared length exceeding `max_payload_size`
**Expected:** `Err(ProtocolError::PayloadTooLarge {...})` without reading the payload
**Verifies:** SEC-5 -- SPEC-06 integration

### T6: Idle connection timeout (SPEC-10 T6)

**Input:** Connection with no messages for longer than `idle_timeout`
**Expected:** Connection is closed
**Verifies:** SEC-6 -- timeout enforcement

### T7: Rejected worker does not block others (SPEC-10 T7)

**Input:** Reject one worker (wrong token), then accept another (correct token)
**Expected:** Second worker succeeds
**Verifies:** SEC-7 -- isolation

### T8: Default bind is localhost (SPEC-10 T8)

**Input:** Coordinator started without `--bind` flag
**Expected:** Binds to `127.0.0.1:9000`
**Verifies:** SEC-8 -- default binding

### T9: Tier detection for all flag combinations (SPEC-10 T9)

**Input:** Test all 4 combinations of `(has_token, has_tls)`
**Expected:** Development, PrivateNetwork, Production, Error respectively
**Verifies:** SEC-9 -- tier detection

### T10: Token debug is redacted (SPEC-10 T10)

**Input:** `format!("{:?}", AuthToken::generate())`
**Expected:** Contains `"[REDACTED]"`, does NOT contain raw bytes or base64
**Verifies:** SEC-10 -- redaction

---

## Edge Cases

### E1: TLS tests only run with feature flag

**Verify:** T4 tests are annotated with `#[cfg(feature = "tls")]` and skipped otherwise.
**Why:** TLS code is only compiled with the feature.

### E2: Self-signed test certificates

**Verify:** Integration tests use self-signed certificates (generated or pre-built), not real CA certificates.
**Why:** Tests must run offline without external CA dependencies.
