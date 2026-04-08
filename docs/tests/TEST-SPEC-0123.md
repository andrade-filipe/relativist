# TEST-SPEC-0123: Define AuthToken struct with generation and serialization

**Task:** TASK-0123
**Spec:** SPEC-10
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: AuthToken::generate produces 32-byte value

**Input:** `let token = AuthToken::generate(); token.as_bytes().len()`
**Expected:** `32`
**Verifies:** R9 -- token is 256-bit (32 bytes)

### T2: Two generate() calls produce different tokens

**Input:** `let t1 = AuthToken::generate(); let t2 = AuthToken::generate(); t1.verify(&t2)`
**Expected:** `false` (with overwhelming probability)
**Verifies:** T1 -- CSPRNG produces unique tokens

### T3: Base64 roundtrip via verify

**Input:** `let t = AuthToken::generate(); let encoded = t.to_base64(); let decoded = AuthToken::from_base64(&encoded).unwrap(); t.verify(&decoded)`
**Expected:** `true`
**Verifies:** T2 -- roundtrip identity through base64

### T4: to_base64 produces 44-character string

**Input:** `AuthToken::generate().to_base64().len()`
**Expected:** `44`
**Verifies:** R10 -- standard base64 encoding of 32 bytes

### T5: from_base64 rejects invalid base64

**Input:** `AuthToken::from_base64("not-valid-base64!!!")`
**Expected:** `Err(TokenError::InvalidBase64(...))`
**Verifies:** Error handling for malformed input

### T6: from_base64 rejects wrong length

**Input:** `AuthToken::from_base64(&base64::engine::general_purpose::STANDARD.encode(&[0u8; 16]))`
**Expected:** `Err(TokenError::InvalidLength(16))`
**Verifies:** Token must be exactly 32 bytes

### T7: Debug output is redacted

**Input:** `let t = AuthToken::generate(); format!("{:?}", t)`
**Expected:** Contains `"[REDACTED]"` and does NOT contain raw byte values
**Verifies:** R34 -- token never appears in debug output

### T8: as_bytes returns the inner array

**Input:** `let t = AuthToken::generate(); t.as_bytes().len()`
**Expected:** `32` and the bytes are deterministic for the same token instance
**Verifies:** R19 -- raw bytes available for wire protocol

---

## Edge Cases

### E1: AuthToken does not implement PartialEq

**Verify:** Code like `assert_eq!(t1, t2)` on two `AuthToken` values fails to compile.
**Why:** SC-009 mandates all comparisons go through `verify()`.

### E2: from_base64 with empty string

**Input:** `AuthToken::from_base64("")`
**Expected:** Returns an error (either `InvalidBase64` or `InvalidLength(0)`)
**Why:** Empty input must not panic.

### E3: from_base64 with padding variations

**Input:** `AuthToken::from_base64(&base64::engine::general_purpose::STANDARD.encode(&[42u8; 32]))`
**Expected:** `Ok(...)` -- valid base64 with standard padding accepted
**Why:** Ensures standard base64 alphabet is used.
