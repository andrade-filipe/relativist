# TEST-SPEC-0122: Define SecurityTier enum and tier detection logic

**Task:** TASK-0122
**Spec:** SPEC-10
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: detect_tier(false, false) returns Development

**Input:** `detect_tier(false, false)`
**Expected:** `Ok(SecurityTier::Development)`
**Verifies:** R2 -- no flags means Tier 1

### T2: detect_tier(true, false) returns PrivateNetwork

**Input:** `detect_tier(true, false)`
**Expected:** `Ok(SecurityTier::PrivateNetwork)`
**Verifies:** Token without TLS is Tier 2

### T3: detect_tier(true, true) returns Production

**Input:** `detect_tier(true, true)`
**Expected:** `Ok(SecurityTier::Production)`
**Verifies:** Token + TLS is Tier 3

### T4: detect_tier(false, true) returns Config error

**Input:** `detect_tier(false, true)`
**Expected:** `Err(SecurityError::Config(...))` containing a message about TLS without token
**Verifies:** R4 -- TLS without token is rejected

### T5: SecurityTier::Display produces expected strings

**Input:** `format!("{}", SecurityTier::Development)`
**Expected:** `"Development (Tier 1)"`
**Input:** `format!("{}", SecurityTier::PrivateNetwork)`
**Expected:** `"Private Network (Tier 2)"`
**Input:** `format!("{}", SecurityTier::Production)`
**Expected:** `"Production (Tier 3)"`
**Verifies:** Human-readable display names

### T6: SecurityTier derives required traits

**Input:** Compile-time check: `let t = SecurityTier::Development; let t2 = t; let t3 = t.clone(); assert_eq!(t, t3);`
**Expected:** Compiles and passes -- verifies Copy, Clone, PartialEq, Eq
**Verifies:** Required trait derives

---

## Edge Cases

### E1: SecurityTier is serializable

**Verify:** `serde_json::to_string(&SecurityTier::Development)` succeeds and produces a valid JSON string.
**Why:** Required for structured logging (R3).

### E2: All four flag combinations are covered

**Verify:** The function handles all 4 combinations of `(has_token, has_tls)`: `(F,F)`, `(T,F)`, `(T,T)`, `(F,T)`. No panic on any combination.
**Why:** Ensures exhaustive handling of all possible CLI flag combinations.
