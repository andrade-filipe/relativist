# TEST-SPEC-0124: Implement AuthToken constant-time verification

**Task:** TASK-0124
**Spec:** SPEC-10
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: verify returns true for cloned token

**Input:** `let t = AuthToken::generate(); t.verify(&t.clone())`
**Expected:** `true`
**Verifies:** Identical tokens pass verification

### T2: verify returns false for different token

**Input:** `let t1 = AuthToken::generate(); let t2 = AuthToken::generate(); t1.verify(&t2)`
**Expected:** `false`
**Verifies:** Different tokens fail verification

### T3: verify detects single-byte difference at last position

**Input:** Construct two tokens identical in bytes 0-30, differing only in byte 31. Call `t1.verify(&t2)`.
**Expected:** `false`
**Verifies:** Constant-time comparison checks all bytes

### T4: verify detects single-byte difference at first position

**Input:** Construct two tokens identical in bytes 1-31, differing only in byte 0. Call `t1.verify(&t2)`.
**Expected:** `false`
**Verifies:** Constant-time comparison checks all bytes including the first

---

## Edge Cases

### E1: AuthToken MUST NOT implement PartialEq

**Verify:** `token1 == token2` does not compile for `AuthToken` values.
**Why:** SC-009 -- all comparisons MUST go through `verify()` which uses `subtle::ConstantTimeEq`.

### E2: Verify uses subtle::ConstantTimeEq (code review)

**Verify:** Source code of `verify()` calls `self.0.ct_eq(&other.0).into()` or equivalent `subtle` API.
**Why:** R15 mandates constant-time comparison to prevent timing side-channel attacks.
