# TEST-SPEC-0131: Define TlsClientConfig (feature-gated)

**Task:** TASK-0131
**Spec:** SPEC-10
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Load valid CA certificate

**Input:** `TlsClientConfig::from_ca_pem(Path::new("tests/certs/ca.crt"))` with a valid CA cert
**Expected:** `Ok(TlsClientConfig { ... })`
**Verifies:** R26 -- CA PEM loading (feature-gated: `#[cfg(feature = "tls")]`)

### T2: Non-existent CA file returns error

**Input:** `TlsClientConfig::from_ca_pem(Path::new("/nonexistent/ca.pem"))`
**Expected:** `Err(SecurityError::...)` (Io or TlsConfig variant)
**Verifies:** Error propagation for missing files

### T3: Invalid PEM content returns error

**Input:** `TlsClientConfig::from_ca_pem` with a file containing `"not a certificate"`
**Expected:** `Err(SecurityError::TlsConfig(...))`
**Verifies:** Error handling for malformed PEM

### T4: TlsClientConfig is Clone

**Input:** `let c = TlsClientConfig::from_ca_pem(...).unwrap(); let c2 = c.clone();`
**Expected:** Compiles and `c2` is usable
**Verifies:** Clone derive via Arc

### T5: No system trust store used

**Input:** Verify that `TlsClientConfig` uses only the provided CA cert, not the system root CAs
**Expected:** Only the specific CA cert is in the root store (R27)
**Verifies:** R27 -- self-signed certificate support

---

## Edge Cases

### E1: Debug output does not reveal internals

**Verify:** `format!("{:?}", tls_client_config)` contains `"[rustls::ClientConfig]"` and does NOT expose internal configuration details.
**Why:** Consistent with TlsServerConfig debug behavior.

### E2: Feature gate exclusion

**Verify:** Without `--features tls`, `TlsClientConfig` is not available.
**Why:** R20 -- entire TLS subsystem is feature-gated.
