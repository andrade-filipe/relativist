# Reviews -- TASK-0120 to TASK-0139: Security Module (Phase 7)

**Date:** 2026-04-08
**Spec:** SPEC-10 (Security)
**Files reviewed:** `src/security/mod.rs`, `src/security/error.rs`, `src/security/tls.rs`, `src/security/token.rs`

---

## Stage 4: Code Cleaner

### Findings

**CC-1 (NTH): Unused import `std::net::SocketAddr` visible scope**
`mod.rs` imports `std::path::Path` at line 19 but `Path` is not directly used in the module body (only used transitively via `write_token_file`). This is fine since it IS used -- no action needed. All imports are justified.

**CC-2 (NTH): `check_bind_warnings` takes `&SocketAddr` rather than value**
`SocketAddr` is `Copy`, so passing by reference is slightly unconventional but not incorrect. Changing to by-value would be marginally more idiomatic but would break callers. NTH.

**CC-3 (NTH): `build_security_config` token logging format**
Line 119: `tracing::info!(token = %t.to_base64(), "Worker authentication token")` uses structured logging with the token as a field. This is correct per SPEC-10 R11 (log once at INFO). The format is consistent with the project's tracing conventions.

**CC-4 (NTH): `from_base64` could use `TryInto` for array conversion**
Lines 48-50 in `token.rs` use manual `copy_from_slice`. Could use `bytes.try_into().map_err(...)` but current code is clear and correct. NTH.

**CC-5 (NTH): `TlsServerConfig` and `TlsClientConfig` custom Debug impls**
These correctly elide the inner `Arc<rustls::*Config>` from debug output. Good practice for security-sensitive config objects.

### Verdict: **PASS** -- No MF or SF issues. Code is clean, idiomatic, well-documented.

---

## Stage 5: Architecture Review

### Spec Compliance Matrix (SPEC-10 MUST requirements)

| Req | Description | Status | Notes |
|-----|-------------|--------|-------|
| R1 | Three security tiers, flag-based detection | PASS | `detect_tier()` + `SecurityTier` enum matches spec exactly |
| R2 | No config required for Tier 1 | PASS | `build_security_config(None, false)` returns Development tier with defaults |
| R3 | Tier detection from CLI flags, logged at INFO | PASS | `detect_tier()` is pure flag-based; `build_security_config` logs tier at INFO |
| R5 | Default bind 127.0.0.1:9000 | N/A | Bind address is in protocol/config, not security module |
| R7 | WARN on 0.0.0.0 binding | PASS | `check_bind_warnings()` emits exact spec wording |
| R8 | WARN on 0.0.0.0 without token | PASS | Second warning in `check_bind_warnings()` |
| R9 | Token generation via OsRng, 256-bit | PASS | `AuthToken::generate()` uses `OsRng.fill_bytes()` on 32-byte array |
| R10 | Base64 encoding, 44 chars | PASS | Uses `base64::engine::general_purpose::STANDARD`, test verifies 44-char output |
| R11 | Token logged once at INFO, never again | PASS | Logged in `build_security_config` during auto-generation; `Debug` impl is redacted |
| R14 | Token in Register as raw bytes | PASS | `RegisterPayload.auth_token: Option<[u8; 32]>` in protocol/types.rs |
| R15 | Constant-time comparison via subtle | PASS | `AuthToken::verify()` uses `self.0.ct_eq(&other.0).into()` |
| R16 | Failed validation: RegisterNack + close | PASS | Coordinator sends RegisterNack with "authentication failed" |
| R17 | Successful validation: RegisterAck | PASS | Coordinator sends RegisterAck with worker_id |
| R18 | Per-session token, no rotation | PASS | Token generated once in `build_security_config`, no refresh logic |
| R19 | AuthToken MUST NOT implement PartialEq/Eq | PASS | Only `Clone` derived; `Debug` prints `[REDACTED]` |
| R20 | TLS feature-gated under `tls` | PASS | `#[cfg(feature = "tls")] pub mod tls;` in mod.rs |
| R22 | TLS 1.3 exclusively, no 1.2 fallback | **MF** | See AR-1 below |
| R23 | Server TLS only (no mTLS) | PASS | `with_no_client_auth()` on both server and client configs |
| R29 | Max payload size from SPEC-06 | N/A | Owned by SPEC-06, not security module |
| R34 | Token never in logs/errors/debug after initial display | PASS | `Debug` impl prints `[REDACTED]`, no other leaks found |
| R35 | Error messages generic, no internal state | PASS | "authentication failed", "protocol error" -- no details |
| R37 | Excluded features NOT implemented | PASS | No mTLS, no rotation, no BFT, no HMAC |

### Architecture Issues

**AR-1 (MF): TLS 1.3 not explicitly enforced (R22 violation)**
In `tls.rs`, both `TlsServerConfig::from_pem_files()` (line 42) and `TlsClientConfig::from_ca_pem()` (line 84) use `rustls::ServerConfig::builder()` / `rustls::ClientConfig::builder()` without specifying protocol versions. In rustls 0.23, the default `builder()` creates a config that accepts BOTH TLS 1.2 and TLS 1.3 (via `DEFAULT_VERSIONS`). SPEC-10 R22 states: "TLS MUST use TLS 1.3 exclusively. TLS 1.2 fallback MUST NOT be permitted."

**Fix:** Use `builder_with_protocol_versions(&[&rustls::version::TLS13])` instead of `builder()`.

**NOTE:** TLS tasks (TASK-0132 through 0135) are DEFERRED. However, since this code exists behind the feature gate and the fix is non-breaking (only restricts the version set), this is safe to fix. Leaving it unfixed would mean the code violates a MUST requirement if ever compiled with `--features tls`.

**AR-2 (NTH): SecurityConfig missing TLS fields when `tls` feature disabled**
The spec (Section 4.2) shows `SecurityConfig` with `tls_server` and `tls_client` fields behind `#[cfg(feature = "tls")]`. The current implementation omits these fields entirely, which is correct for now since TLS tasks are deferred. When TLS is implemented, these fields will need to be added. This is expected and not an issue.

**AR-3 (NTH): Module boundary correctness**
The security module correctly owns: `AuthToken`, `SecurityConfig`, `SecurityTier`, `SecurityError`, `TokenError`, `TlsServerConfig`, `TlsClientConfig`, `detect_tier`, `build_security_config`, `write_token_file`, `check_bind_warnings`. Protocol-level registration messages (`RegisterPayload`, etc.) are correctly in `protocol::types` since they are wire protocol structures. The `AuthToken` is imported by `protocol::coordinator` and `protocol::worker` for authentication logic. Module boundaries are clean.

### Verdict: **1 MF** (AR-1: TLS 1.3 enforcement), otherwise PASS.

---

## Stage 6: QA Bug Hunt

### Security Vulnerability Analysis

**QA-1 (MF): TLS 1.2 fallback permitted (same as AR-1)**
If the `tls` feature is enabled, a downgrade attack to TLS 1.2 is possible because the rustls config does not restrict to TLS 1.3. This is a security vulnerability. Cross-referenced with AR-1.

**QA-2 (SF): Token double-log risk analysis**
Reviewed all paths where the token value could appear in logs:
- `build_security_config` with `Some("auto")`: logs `token = %t.to_base64()` at INFO once. PASS.
- `build_security_config` with `Some(value)`: logs "Using provided authentication token" (no value). PASS.
- `AuthToken::Debug`: prints `[REDACTED]`. PASS.
- Coordinator's `accept_workers`: logs "Rejected: authentication failed" with addr but NOT the token. PASS.
- No path logs the token value more than once. **No double-log risk found.**

**QA-3 (NTH): Error message information leakage review**
- `SecurityError::AuthFailed` displays "authentication failed" -- generic, PASS.
- `TokenError::InvalidBase64(String)` includes the base64 library's error message. This is only used internally during coordinator startup (parsing `--token <base64>`) and does not reach unauthenticated clients. PASS.
- `TokenError::InvalidLength(usize)` reveals expected vs actual length. Same as above -- internal only. PASS.
- `SecurityError::TlsConfig(String)` and `SecurityError::Certificate(String)` include internal details. These are configuration errors at startup, not sent to clients. PASS.
- `RegisterNackPayload.reason` in coordinator code: always "authentication failed", "unsupported protocol version", or "protocol error". None leak internal state. PASS per R35.

**QA-4 (NTH): Panic analysis**
- `AuthToken::generate()`: `OsRng.fill_bytes()` -- can panic if the OS RNG fails (extremely rare, system-level failure). Acceptable.
- `AuthToken::from_base64()`: returns `Result`, no panics.
- `AuthToken::verify()`: `ct_eq` on fixed-size arrays, no panics.
- `write_token_file()`: returns `Result`, no panics.
- `build_security_config()`: returns `Result`, no panics.
- `detect_tier()`: exhaustive match, no panics.
- TLS `from_pem_files` / `from_ca_pem`: return `Result`, no panics.
- **No unexpected panic paths found.**

**QA-5 (NTH): TLS config edge cases (documented, TLS tasks deferred)**
- `from_pem_files`: If cert file contains zero certificates, `certs()` returns empty vec, `with_single_cert(vec![], key)` will fail with a rustls error caught by the `map_err`. PASS.
- `from_ca_pem`: If CA file contains zero certs, `root_store` will be empty. `ClientConfig::builder().with_root_certificates(empty_store)` will succeed but all TLS connections will fail validation. This is a user configuration error, not a code bug. Could add a check for empty root store, but since TLS tasks are deferred, this is NTH.
- Missing: no validation that cert and key match (rustls does this internally in `with_single_cert`). PASS.

**QA-6 (NTH): `detect_tier(false, true)` returns Development**
When TLS flags are provided without a token, `detect_tier` returns `Development`. This is handled correctly because `build_security_config` rejects this combination with an error BEFORE returning a config (R4 check at line 133-139). The tier detection function is deliberately simple (pure flag mapping); validation is separate. This is correct design.

### Verdict: **1 MF** (same as AR-1), otherwise PASS. No panics, no leaks, no logic errors.

---

## Summary

| Stage | Verdict | MF | SF | NTH |
|-------|---------|----|----|-----|
| Code Cleaner | PASS | 0 | 0 | 5 |
| Architecture | 1 MF | 1 | 0 | 2 |
| QA Bug Hunt | 1 MF (shared) | 1 | 0 | 4 |

**Total unique issues:** 1 MF (TLS 1.3 enforcement), 0 SF, ~8 NTH (informational).

### MF Fix Plan

**AR-1/QA-1:** In `src/security/tls.rs`, replace `rustls::ServerConfig::builder()` with `rustls::ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])` and `rustls::ClientConfig::builder()` with `rustls::ClientConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])`. This is a one-line change per config builder that restricts negotiation to TLS 1.3 only, satisfying R22.
