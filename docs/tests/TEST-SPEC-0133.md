# TEST-SPEC-0133: Implement TLS handshake integration for worker (client side)

**Task:** TASK-0133
**Spec:** SPEC-10
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: TLS connect with correct CA succeeds

**Input:** Set up a localhost TLS server; call `tls_connect(tcp_stream, &tls_client_config, "localhost")`
**Expected:** Returns `Ok(TlsStream<TcpStream>)` ready for communication
**Verifies:** T4 -- valid client TLS handshake (feature-gated: `#[cfg(feature = "tls")]`)

### T2: TLS connect with wrong CA cert fails

**Input:** Client uses a CA cert that did not sign the server's cert
**Expected:** Handshake fails with `Err(SecurityError::TlsConfig(...))`
**Verifies:** T4 -- certificate validation on client side

### T3: Messages exchangeable over TLS connection

**Input:** After successful `tls_connect`, use `send_frame`/`recv_frame` over the TlsStream
**Expected:** Bidirectional communication works
**Verifies:** R24 -- transparent encryption layer

### T4: Invalid server name returns error

**Input:** `tls_connect(stream, &config, "invalid..name")`
**Expected:** `Err(SecurityError::TlsConfig(...))` containing "invalid server name"
**Verifies:** SNI name validation

---

## Edge Cases

### E1: IP address as server name

**Input:** `tls_connect(stream, &config, "127.0.0.1")` when the cert has `127.0.0.1` as SAN
**Expected:** Handshake succeeds (ServerName::IpAddress variant)
**Why:** Grid computing may use IP addresses rather than DNS names.

### E2: Feature-gated compilation

**Verify:** `tls_connect` is only available when compiled with `--features tls`.
**Why:** R20, R28a -- TLS code is feature-gated.
