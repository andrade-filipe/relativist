# TEST-SPEC-0129: Implement network binding security

**Task:** TASK-0129
**Spec:** SPEC-10
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Default bind address constant

**Input:** `DEFAULT_BIND_ADDRESS`
**Expected:** `"127.0.0.1:9000"`
**Verifies:** R5, SPEC-13 R44

### T2: Localhost with Development tier succeeds

**Input:** `validate_bind_address("127.0.0.1:9000", SecurityTier::Development, false)`
**Expected:** `Ok(())`
**Verifies:** Default config is valid

### T3: 0.0.0.0 with Development tier succeeds (warn and proceed)

**Input:** `validate_bind_address("0.0.0.0:9000", SecurityTier::Development, false)`
**Expected:** `Ok(())` -- function returns Ok, not Err (R8: MUST-warn-and-proceed)
**Verifies:** R8 -- no refusal on 0.0.0.0 without auth

### T4: 0.0.0.0 with Development and insecure flag succeeds

**Input:** `validate_bind_address("0.0.0.0:9000", SecurityTier::Development, true)`
**Expected:** `Ok(())` -- warnings suppressed
**Verifies:** --insecure suppresses SHOULD-level warnings

### T5: 0.0.0.0 with PrivateNetwork tier succeeds

**Input:** `validate_bind_address("0.0.0.0:9000", SecurityTier::PrivateNetwork, false)`
**Expected:** `Ok(())`
**Verifies:** Auth enabled, binding to all interfaces is fine

### T6: 0.0.0.0 with Production tier succeeds

**Input:** `validate_bind_address("0.0.0.0:9000", SecurityTier::Production, false)`
**Expected:** `Ok(())`
**Verifies:** Full security, binding to all interfaces is fine

### T7: Specific non-localhost IP with PrivateNetwork succeeds

**Input:** `validate_bind_address("192.168.1.10:9000", SecurityTier::PrivateNetwork, false)`
**Expected:** `Ok(())`
**Verifies:** R6 -- explicit bind to non-localhost with auth

---

## Edge Cases

### E1: Function never returns Err for valid addresses

**Verify:** `validate_bind_address` returns `Ok(())` for all tier/address combinations (R8 removed refusal).
**Why:** R8 was revised to MUST-warn-and-proceed. The function emits warnings but never refuses.

### E2: IPv6 unspecified address

**Input:** `validate_bind_address("[::]:9000", SecurityTier::Development, false)`
**Expected:** `Ok(())` -- should handle IPv6 unspecified similarly to 0.0.0.0
**Why:** Robustness for IPv6 environments.
