# TEST-SPEC-0126: Implement token file write

**Task:** TASK-0126
**Spec:** SPEC-10
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: write_token_file creates a file at specified path

**Input:** `write_token_file(&AuthToken::generate(), Path::new("/tmp/test-token"))`
**Expected:** `Ok(())` and the file exists at `/tmp/test-token`
**Verifies:** R12 -- token file creation

### T2: File contents match base64-encoded token

**Input:** Generate token, write to file, read file contents
**Expected:** File contents equal `token.to_base64()`
**Verifies:** R12 -- file contains the base64 token

### T3: Unix file permissions are 0600

**Input:** Write token file, check permissions with `std::os::unix::fs::PermissionsExt::mode()`
**Expected:** `mode & 0o777 == 0o600` (owner read/write only)
**Verifies:** R12 -- restricted file permissions (Unix only, `#[cfg(unix)]`)

### T4: Overwriting existing file succeeds

**Input:** Write token file twice to the same path with different tokens
**Expected:** Second write succeeds; file contents match the second token
**Verifies:** File overwrite behavior

### T5: Writing to non-existent directory returns error

**Input:** `write_token_file(&token, Path::new("/nonexistent/dir/token"))`
**Expected:** `Err(SecurityError::Io(...))`
**Verifies:** I/O error propagation

### T6: DEFAULT_TOKEN_FILE constant value

**Input:** `DEFAULT_TOKEN_FILE`
**Expected:** `"./relativist-token"`
**Verifies:** R12 default path

---

## Edge Cases

### E1: display_token logs at INFO level

**Verify:** Calling `display_token(&token)` emits a tracing INFO event containing the base64 token.
**Why:** R11 -- token must be logged exactly once.

### E2: Token file with special characters in path

**Input:** Write token to a path containing spaces, e.g., `/tmp/test dir/token`
**Expected:** Either succeeds (if directory exists) or returns `Err` (if it does not). No panic.
**Why:** Robustness against unusual file paths.
