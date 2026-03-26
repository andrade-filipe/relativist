---
pesq_id: PESQ-018
title: "Token Authentication & Trust Models"
category: Security in Distributed Systems
date: 2026-03-26
status: Complete
cross_references:
  specs: [SPEC-10, SPEC-06, SPEC-13]
  pesqs: [PESQ-005, PESQ-017, PESQ-019]
  discs: [DISC-007]
---

# PESQ-018: Token Authentication & Trust Models

**Category:** Security in Distributed Systems
**Status:** Complete

---

## 1. Subject Overview

Authentication in distributed systems answers: "Is this worker allowed to participate?" For Relativist, this means verifying that workers connecting to the coordinator are authorized nodes, not attackers.

### 1.1 Authentication Approaches

| Approach | Complexity | Security | Relativist Fit |
|----------|-----------|----------|---------------|
| No auth | None | None | Dev/testing only |
| Shared secret (token) | Low | Medium | **v1 recommendation** |
| HMAC-signed messages | Medium | High | With TLS disabled |
| mTLS (client certs) | High | Very High | v2 consideration |
| OAuth2/JWT | High | High | Overkill |

---

## 2. Token-Based Authentication for Relativist

### 2.1 Model

1. **Coordinator generates a token** at startup (random 256-bit, base64-encoded)
2. Token is displayed in coordinator logs / written to file
3. **Operator distributes token** to workers (CLI flag, env var, or Docker secret)
4. **Worker presents token** in `Register` message
5. **Coordinator validates** token and accepts/rejects registration

```
Coordinator startup:
  token = random_bytes(32) |> base64_encode
  log::info!("Worker token: {}", token)
  write_file("./relativist-token", token)  // optional

Worker startup:
  token = env::var("RELATIVIST_TOKEN") || cli_flag("--token")
  send(Register { token, capabilities })

Coordinator on Register:
  if msg.token != self.token:
    send(RegisterNack { reason: "invalid token" })
    close_connection()
  else:
    send(RegisterAck { worker_id, config })
```

### 2.2 Token Properties

| Property | Value |
|----------|-------|
| Length | 256 bits (32 bytes) |
| Encoding | Base64 (44 characters) |
| Generation | `rand::rngs::OsRng` (CSPRNG) |
| Lifetime | Single coordinator session |
| Rotation | Restart coordinator = new token |
| Storage | Environment variable or file |
| Wire transmission | In `Register` message only (not every message) |

### 2.3 Trust Model

**Trusted Network (default):** No TLS, no token. Suitable for:
- Local development
- Private LAN / VPN
- Docker Compose on single host

**Token Auth (recommended for multi-node):** Token + no TLS. Suitable for:
- Private network but want identity verification
- Basic protection against accidental connections

**Token + TLS (production):** Token auth over TLS. Suitable for:
- Untrusted network
- Cloud deployments
- Any production use

### 2.4 HMAC Message Integrity (when TLS disabled)

When TLS is disabled, messages need integrity protection beyond CRC32 (which is not cryptographic). Option:

```
Message frame (SPEC-06):
  [length: u32] [payload: bytes] [crc32: u32]

With HMAC:
  [length: u32] [payload: bytes] [hmac-sha256: [u8; 32]]
```

HMAC-SHA256 using the shared token as key provides:
- Message integrity (tampering detected)
- Authentication (only holders of the token can produce valid HMACs)

**Decision:** Make HMAC optional. CRC32 for trusted networks (fast, catches corruption). HMAC-SHA256 for untrusted without TLS. TLS supersedes both.

---

## 3. Comparison: HTCondor IDTOKENS (PESQ-005)

HTCondor's IDTOKENS model (PESQ-005) is more sophisticated:
- Tokens are signed by the central manager using a signing key
- Tokens contain claims (identity, capabilities, expiry)
- Tokens can be revoked
- Multiple tokens can coexist

Relativist's model is simpler (single shared secret) but sufficient for v1 where:
- There's one coordinator, one computation, one token
- Workers are homogeneous (no capability-based access control)
- Sessions are short-lived (no rotation needed)

---

## 4. Lessons for Relativist

### L1: Shared Token Authentication [ADOPT]
Coordinator generates random token, workers present it on registration. Simple, effective, sufficient for v1.
→ Informs: SPEC-10, SPEC-06

### L2: Token via Env Var [ADOPT]
Workers receive token via `RELATIVIST_TOKEN` env var. This is Docker-friendly (env vars, secrets) and avoids command-line exposure in process lists.
→ Informs: SPEC-07, SPEC-10

### L3: HMAC-SHA256 as Optional Integrity [ADAPT]
When TLS is disabled and network is untrusted, use HMAC-SHA256 (keyed by token) instead of CRC32 for message integrity. Feature-flagged alongside `tls`.
→ Informs: SPEC-06, SPEC-10

### L4: No Token Rotation in v1 [ADOPT]
Token is per-session (coordinator restart = new token). Rotation adds complexity for no benefit when sessions last minutes to hours.
→ Informs: SPEC-10

### L5: Three Security Tiers [ADOPT]
Clearly document three tiers:
1. **Development:** No auth, no TLS
2. **Private network:** Token auth, no TLS (CRC32 or HMAC)
3. **Production:** Token auth + TLS
→ Informs: SPEC-10

---

## 5. Sources

| Source | URL | Accessed |
|--------|-----|----------|
| Token Authentication overview | https://www.loginradius.com/blog/identity/what-is-token-authentication | 2026-03-26 |
| HMAC vs Token auth comparison | https://towardsdev.com/tech-stack-101-ep-03-api-security-token-based-vs-hmac-authentication-b1fb70c1f8bb | 2026-03-26 |
| JWT introduction | https://www.jwt.io/introduction | 2026-03-26 |
| HTCondor IDTOKENS (PESQ-005) | internal | — |
