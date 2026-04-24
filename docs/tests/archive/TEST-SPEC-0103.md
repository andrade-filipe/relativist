# TEST-SPEC-0103: Define RelativistError top-level error type

**Task:** TASK-0103
**Spec:** SPEC-13
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Config error has exit code 1

**Type:** Unit test
**Input:** `RelativistError::Config("bad argument".into()).exit_code()`
**Expected:** Returns `1`
**Verifies:** SPEC-07 R43 -- config errors exit with code 1

### T2: Io error has exit code 2

**Type:** Unit test
**Input:** `RelativistError::Io(io::Error::new(ErrorKind::ConnectionRefused, "refused")).exit_code()`
**Expected:** Returns `2`
**Verifies:** Communication errors exit with code 2

### T3: Protocol error has exit code 2

**Type:** Unit test
**Input:** `RelativistError::Protocol(ProtocolError::AuthFailed).exit_code()`
**Expected:** Returns `2`
**Verifies:** Protocol errors classified as communication errors

### T4: Net error has exit code 3

**Type:** Unit test
**Input:** `RelativistError::Net(NetError::some_variant()).exit_code()`
**Expected:** Returns `3`
**Verifies:** Internal errors exit with code 3

### T5: All error enums implement Display

**Type:** Unit test
**Input:** For each variant of `RelativistError`, call `format!("{}", err)`
**Expected:** Each produces a non-empty, descriptive string without panic
**Verifies:** `thiserror::Error` Display implementations

### T6: From conversions compile

**Type:** Compilation test
**Input:**
```
let _: RelativistError = io::Error::new(ErrorKind::Other, "test").into();
let _: RelativistError = ProtocolError::AuthFailed.into();
```
**Expected:** Both compile via `#[from]` conversions
**Verifies:** R17 -- unified error hierarchy with automatic conversions

### T7: Coordinator wrapping Protocol has exit code 2

**Type:** Unit test
**Input:** `RelativistError::Coordinator(CoordinatorError::Protocol(ProtocolError::AuthFailed)).exit_code()`
**Expected:** Returns `2`
**Verifies:** Nested protocol errors are still classified as communication errors

---

## Edge Cases

### E1: Old RelError name still compiles (if alias provided)

**Verify:** If a `pub type RelError = RelativistError;` alias is provided, existing code using `RelError` continues to compile.
**Why:** Backward compatibility during migration from the old error type.

### E2: All 7 module error enums exist and derive Debug + Error

**Verify:** `NetError`, `ReductionError`, `PartitionError`, `MergeError`, `ProtocolError`, `CoordinatorError`, `WorkerError` all exist, derive `Debug`, and implement `std::error::Error`.
**How:** Construct one variant of each and call `format!("{:?}", err)` and `format!("{}", err)`.
**Why:** R15 -- every module has its own error enum.
