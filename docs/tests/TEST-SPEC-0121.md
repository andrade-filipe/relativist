# TEST-SPEC-0121: Define TokenError and SecurityError enums

**Task:** TASK-0121
**Spec:** SPEC-10
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: TokenError::InvalidBase64 Display output

**Input:** `TokenError::InvalidBase64("abc".to_string()).to_string()`
**Expected:** `"invalid base64 encoding: abc"`
**Verifies:** Display impl matches spec (R25)

### T2: TokenError::InvalidLength Display output

**Input:** `TokenError::InvalidLength(16).to_string()`
**Expected:** `"invalid token length: expected 32 bytes, got 16"`
**Verifies:** Display impl matches spec

### T3: SecurityError::AuthFailed Display is generic

**Input:** `SecurityError::AuthFailed.to_string()`
**Expected:** `"authentication failed"` -- no internal state revealed (R35)
**Verifies:** Generic error message for security (R35)

### T4: TokenError auto-converts to SecurityError via From

**Input:** `let se: SecurityError = TokenError::InvalidBase64("x".into()).into();`
**Expected:** `se` is `SecurityError::Token(TokenError::InvalidBase64("x"))`
**Verifies:** `#[from]` derive works correctly

### T5: Both enums implement std::error::Error

**Input:** Compile-time check: `fn assert_error<T: std::error::Error>() {}; assert_error::<TokenError>(); assert_error::<SecurityError>();`
**Expected:** Compiles without error
**Verifies:** Both enums satisfy the `Error` trait bound

### T6: SecurityError::TlsConfig carries message

**Input:** `SecurityError::TlsConfig("bad cert".to_string()).to_string()`
**Expected:** Contains `"TLS configuration error: bad cert"`

### T7: SecurityError::Certificate carries message

**Input:** `SecurityError::Certificate("expired".to_string()).to_string()`
**Expected:** Contains `"certificate error: expired"`

---

## Edge Cases

### E1: AuthFailed does not reveal token information

**Verify:** `SecurityError::AuthFailed.to_string()` does NOT contain "token", "wrong", "missing", "malformed", or any byte values.
**Why:** R35 requires generic error messages that do not leak internal state.

### E2: Config variant with empty string

**Input:** `SecurityError::Config("".to_string()).to_string()`
**Expected:** `"configuration error: "` -- no panic on empty string
**Why:** Ensures graceful handling of edge-case error messages.
