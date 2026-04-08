# TEST-SPEC-0081: Define ProtocolError enum

**Task:** TASK-0081
**Spec:** SPEC-06
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: All 8 variants constructible

**Type:** Unit test
**Input:** Construct each variant:
- `ProtocolError::ConnectionLost(io::Error::new(ErrorKind::ConnectionRefused, "refused"))`
- `ProtocolError::PayloadTooLarge { size: 500_000_000, max: 268_435_456 }`
- `ProtocolError::ChecksumMismatch { expected: 0xAABBCCDD, computed: 0x11223344 }`
- `ProtocolError::Deserialize(bincode_err)`
- `ProtocolError::Serialize(bincode_err)`
- `ProtocolError::UnexpectedMessage { expected: "PartitionResult", received: "Shutdown".to_string() }`
- `ProtocolError::Timeout { phase: "collect", elapsed: Duration::from_secs(600) }`
- `ProtocolError::AuthFailed`
**Expected:** All 8 variants compile and can be constructed without error
**Verifies:** R8, R9, R25, R29, R30, R31 -- all SPEC-06 v3 variants present

### T2: Debug output for each variant

**Type:** Unit test
**Input:** `format!("{:?}", variant)` for each of the 8 variants
**Expected:** Each produces a non-empty string containing the variant name
**Verifies:** `#[derive(Debug)]` works for all variants

### T3: From<std::io::Error> conversion

**Type:** Unit test
**Input:** `let err: ProtocolError = io::Error::new(ErrorKind::BrokenPipe, "broken").into();`
**Expected:** `matches!(err, ProtocolError::ConnectionLost(_))` is true
**Verifies:** `impl From<std::io::Error>` converts to `ConnectionLost`, NOT `Io`

### T4: Display messages are descriptive

**Type:** Unit test
**Input:** `format!("{}", ProtocolError::PayloadTooLarge { size: 300_000_000, max: 268_435_456 })`
**Expected:** String contains both `300000000` and `268435456` (or human-readable equivalents)
**Verifies:** Display implementation provides actionable error messages

### T5: AuthFailed has no fields

**Type:** Unit test
**Input:** `let err = ProtocolError::AuthFailed;`
**Expected:** Compiles; `format!("{:?}", err)` contains "AuthFailed"
**Verifies:** AuthFailed is a unit variant with no associated data

### T6: PayloadTooLarge uses `size` field, not `declared`

**Type:** Compilation test
**Input:** `ProtocolError::PayloadTooLarge { size: 100, max: 50 }`
**Expected:** Compiles successfully. Using `declared` instead of `size` would fail to compile.
**Verifies:** SPEC-06 v3 field rename from `declared` to `size`

---

## Edge Cases

### E1: WorkerError and WorkerCountMismatch are absent

**Verify:** `ProtocolError::WorkerError` and `ProtocolError::WorkerCountMismatch` do NOT exist as variants.
**How:** Attempting to construct `ProtocolError::WorkerError { .. }` must fail to compile.
**Why:** These were moved to `CoordinatorError` per SPEC-13 R16.

### E2: ProtocolError does not derive Clone

**Verify:** `ProtocolError` does NOT implement `Clone`.
**How:** `let clone = err.clone();` should fail to compile for any `ProtocolError` instance.
**Why:** `std::io::Error` and `bincode::Error` are not `Clone`.
