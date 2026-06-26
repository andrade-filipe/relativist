# TEST-SPEC-0130: Define TlsServerConfig (feature-gated)

**Task:** TASK-0130
**Spec:** SPEC-10
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Load valid self-signed cert and key

**Input:** `TlsServerConfig::from_pem_files(Path::new("tests/certs/server.crt"), Path::new("tests/certs/server.key"))` with valid test certificates
**Expected:** `Ok(TlsServerConfig { ... })` with a valid `Arc<rustls::ServerConfig>`
**Verifies:** R25 -- PEM file loading (feature-gated: `#[cfg(feature = "tls")]`)

### T2: Non-existent cert file returns error

**Input:** `TlsServerConfig::from_pem_files(Path::new("/nonexistent/cert.pem"), Path::new("tests/certs/server.key"))`
**Expected:** `Err(SecurityError::...)` (Io or TlsConfig variant)
**Verifies:** Error propagation for missing files

### T3: Invalid PEM content returns error

**Input:** `TlsServerConfig::from_pem_files` with a file containing `"not a certificate"`
**Expected:** `Err(SecurityError::Certificate(...))` or `Err(SecurityError::TlsConfig(...))`
**Verifies:** Error handling for malformed PEM

### T4: TlsServerConfig is Clone

**Input:** `let c = TlsServerConfig::from_pem_files(...).unwrap(); let c2 = c.clone();`
**Expected:** Compiles and `c2` is usable
**Verifies:** Clone derive via Arc

### T5: TLS 1.3 enforced

**Input:** Inspect the built `rustls::ServerConfig` versions
**Expected:** Only TLS 1.3 is accepted (R22)
**Verifies:** R22 -- TLS 1.3 exclusively

---

## Edge Cases

### E1: Debug output does not reveal private key

**Verify:** `format!("{:?}", tls_config)` contains `"[rustls::ServerConfig]"` and does NOT contain PEM key material.
**Why:** Security -- private keys must not leak into logs.

### E2: Feature gate exclusion

**Verify:** Without `--features tls`, `TlsServerConfig` is not available (does not compile if referenced).
**Why:** R20 -- entire TLS subsystem is feature-gated.
