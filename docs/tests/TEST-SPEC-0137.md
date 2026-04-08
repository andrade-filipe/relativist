# TEST-SPEC-0137: Add security crate dependencies to Cargo.toml

**Task:** TASK-0137
**Spec:** SPEC-10
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: cargo check succeeds with default features

**Input:** Run `cargo check` (no feature flags)
**Expected:** Compilation succeeds
**Verifies:** Always-on deps (rand, base64, subtle) are available

### T2: cargo check succeeds with TLS feature

**Input:** Run `cargo check --features tls`
**Expected:** Compilation succeeds
**Verifies:** TLS deps (rustls-pemfile) added to tls feature

### T3: rand crate is importable

**Input:** `use rand::rngs::OsRng; use rand::RngCore;` compiles
**Expected:** Compiles without error
**Verifies:** R9 -- CSPRNG dependency available

### T4: subtle crate is importable

**Input:** `use subtle::ConstantTimeEq;` compiles
**Expected:** Compiles without error
**Verifies:** R15 -- constant-time comparison dependency

### T5: base64 crate is importable

**Input:** `use base64::Engine;` compiles
**Expected:** Compiles without error
**Verifies:** R10 -- base64 encoding dependency

---

## Edge Cases

### E1: rustls-pemfile is optional and gated under tls

**Verify:** `Cargo.toml` has `rustls-pemfile = { version = "2", optional = true }` and the `tls` feature includes `"rustls-pemfile"`.
**Why:** PEM parsing is only needed for TLS certificate loading.

### E2: Always-on deps are not optional

**Verify:** `rand`, `base64`, and `subtle` are listed as non-optional dependencies.
**Why:** Token generation and validation are needed in Tier 2 even without TLS.
