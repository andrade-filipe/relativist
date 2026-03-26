---
pesq_id: PESQ-017
title: "TLS 1.3 / rustls / mTLS for Rust Distributed Systems"
category: Security in Distributed Systems
date: 2026-03-26
status: Complete
cross_references:
  specs: [SPEC-10, SPEC-13]
  pesqs: [PESQ-005, PESQ-018]
  discs: [DISC-007]
---

# PESQ-017: TLS 1.3 / rustls / mTLS

**Category:** Security in Distributed Systems
**Status:** Complete

---

## 1. Subject Overview

rustls is a pure-Rust TLS implementation supporting TLS 1.2 and 1.3. It uses `ring` for cryptography and `webpki` for certificate validation. Combined with `tokio-rustls`, it provides async TLS for tokio-based applications.

### 1.1 Why rustls Over OpenSSL

| Dimension | rustls | openssl (via rust-openssl) |
|-----------|--------|---------------------------|
| Memory safety | Pure Rust, no C code | C library, CVE history |
| Build complexity | No system dependency | Requires libssl-dev |
| Performance | Competitive, sometimes faster | Mature optimization |
| TLS 1.3 | Full support | Full support |
| mTLS | Supported | Supported |
| FIPS compliance | No (ring doesn't have FIPS cert) | Yes (with FIPS module) |
| Docker image size | No extra libs needed | Needs libssl in runtime image |

**Decision: rustls** — no C dependencies, simpler Docker images, memory-safe.

---

## 2. Architecture for Relativist

### 2.1 TLS Modes

| Mode | When | Security Level |
|------|------|---------------|
| **No TLS** | Trusted LAN, development, testing | None |
| **Server TLS** | Coordinator has cert, workers verify | Encryption + server auth |
| **Mutual TLS (mTLS)** | Both sides have certs | Encryption + mutual auth |

**Recommendation for v1:** Support **No TLS** (default) and **Server TLS** (with `--tls` flag). mTLS is too complex for v1 (requires PKI infrastructure).

### 2.2 Integration with tokio

```rust
// Server (Coordinator)
use tokio_rustls::TlsAcceptor;
use rustls::ServerConfig;

let config = ServerConfig::builder()
    .with_no_client_auth()  // or .with_client_cert_verifier() for mTLS
    .with_single_cert(certs, key)?;
let acceptor = TlsAcceptor::from(Arc::new(config));

// For each incoming connection:
let tls_stream = acceptor.accept(tcp_stream).await?;

// Client (Worker)
use tokio_rustls::TlsConnector;
use rustls::ClientConfig;

let config = ClientConfig::builder()
    .with_root_certificates(root_store)
    .with_no_client_auth();  // or .with_client_auth_cert() for mTLS
let connector = TlsConnector::from(Arc::new(config));
let tls_stream = connector.connect(server_name, tcp_stream).await?;
```

### 2.3 Certificate Management

For v1, keep it simple:
- **Self-signed certificates:** Generated with `rcgen` crate or external tool
- **Coordinator generates its own cert:** Stored in configurable path
- **Workers trust coordinator's CA:** CA cert distributed out-of-band (file copy, Docker secret)

No automatic certificate rotation in v1. Certificate lifetime: 365 days (configurable).

### 2.4 Feature Flag

```toml
[features]
default = []
tls = ["rustls", "tokio-rustls", "rustls-pemfile"]
```

When `tls` is disabled, all TLS code is compiled out. The binary is smaller and has no crypto dependencies.

---

## 3. Crate Dependencies

| Crate | Purpose | Version |
|-------|---------|---------|
| `rustls` | TLS implementation | 0.23+ |
| `tokio-rustls` | Async TLS for tokio | 0.26+ |
| `rustls-pemfile` | PEM file parsing | 2.0+ |
| `rcgen` | Certificate generation (dev/test) | 0.13+ |
| `webpki-roots` | Mozilla CA bundle (for verifying public certs) | 0.26+ |

---

## 4. Lessons for Relativist

### L1: rustls + tokio-rustls [ADOPT]
Use rustls for TLS. Pure Rust, no system dependencies, TLS 1.3 by default. Combined with tokio-rustls for async.
→ Informs: SPEC-10, SPEC-13

### L2: TLS as Feature Flag [ADOPT]
`--features tls` enables TLS. Without it, no crypto dependencies are compiled. This keeps the default build fast and simple.
→ Informs: SPEC-10, SPEC-13

### L3: Server TLS Only for v1 [ADOPT]
Start with server-only TLS (coordinator has cert, workers verify). mTLS adds certificate distribution complexity that's not justified for v1.
→ Informs: SPEC-10

### L4: Self-Signed Certs for Development [ADOPT]
Provide `relativist generate-cert` subcommand (or use `rcgen` in tests) for easy self-signed cert generation. No PKI infrastructure needed for development.
→ Informs: SPEC-10, SPEC-12

### L5: TLS Wraps Existing Protocol [ADOPT]
TLS is a transport-layer concern. The wire protocol (SPEC-06) is unchanged — same messages, same framing. TLS wraps the TCP stream transparently.
→ Informs: SPEC-06, SPEC-10

---

## 5. Sources

| Source | URL | Accessed |
|--------|-----|----------|
| rustls GitHub | https://github.com/rustls/rustls | 2026-03-26 |
| tokio-rustls GitHub | https://github.com/rustls/tokio-rustls | 2026-03-26 |
| tokio-rustls docs | https://docs.rs/tokio-rustls | 2026-03-26 |
| Rust TLS guide (developerlife) | https://developerlife.com/2024/11/28/rust-tls-rustls/ | 2026-03-26 |
| Tokio+Rustls server series | https://medium.com/@alfred.weirich/tokio-tower-hyper-and-rustls-building-high-performance-and-secure-servers-in-rust-part-4-59a8320a1f7f | 2026-03-26 |
