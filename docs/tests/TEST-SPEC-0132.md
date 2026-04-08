# TEST-SPEC-0132: Implement TLS handshake integration for coordinator (server side)

**Task:** TASK-0132
**Spec:** SPEC-10
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: TLS accept with valid client succeeds

**Input:** Set up a localhost TLS server+client pair with matching certs; call `tls_accept(tcp_stream, &tls_server_config)`
**Expected:** Returns `Ok(TlsStream<TcpStream>)` ready for communication
**Verifies:** T4 -- valid TLS handshake (feature-gated: `#[cfg(feature = "tls")]`)

### T2: TLS accept with wrong CA fails

**Input:** Client uses a CA cert that did not sign the server's cert; call `tls_accept`
**Expected:** Handshake fails with `Err(SecurityError::TlsConfig(...))`
**Verifies:** T4 -- certificate validation

### T3: Messages can be exchanged over TlsStream

**Input:** After successful `tls_accept`, use `send_frame`/`recv_frame` on the `TlsStream`
**Expected:** Messages serialize, send, receive, and deserialize correctly
**Verifies:** R24 -- transparent encryption layer

### T4: Function signature accepts TcpStream

**Input:** Compile-time check: `tls_accept` takes `TcpStream` and `&TlsServerConfig`
**Expected:** Compiles
**Verifies:** Correct parameter types

---

## Edge Cases

### E1: TLS handshake failure does not terminate accept loop

**Verify:** After a failed TLS handshake, the coordinator can accept subsequent connections.
**Why:** A single bad client must not crash the server.

### E2: Feature-gated compilation

**Verify:** `tls_accept` is only available when compiled with `--features tls`.
**Why:** R20, R28a -- TLS code is feature-gated.
