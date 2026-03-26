---
pesq_id: PESQ-019
title: "Security Lessons from Distributed System CVEs"
category: Security in Distributed Systems
date: 2026-03-26
status: Complete
cross_references:
  specs: [SPEC-10, SPEC-06, SPEC-13]
  pesqs: [PESQ-017, PESQ-018]
  discs: [DISC-007]
---

# PESQ-019: Security Lessons from Distributed System CVEs

**Category:** Security in Distributed Systems
**Status:** Complete

---

## 1. Subject Overview

This document reviews common security vulnerabilities in distributed computing systems and extracts defensive lessons for Relativist. Rather than analyzing specific CVEs (which would require access to proprietary systems), we focus on vulnerability classes relevant to coordinator-worker architectures.

---

## 2. Vulnerability Classes

### 2.1 Unauthenticated Access

**Pattern:** Distributed system services listen on network ports without authentication. Anyone who can reach the port can submit jobs, read data, or control the cluster.

**Historical examples:**
- Apache Spark: Default configuration had no authentication; any user on the network could submit arbitrary code
- Redis: Default bind to 0.0.0.0 with no password; led to widespread cryptomining infections
- Elasticsearch: Default no-auth REST API; mass data leaks

**Relativist mitigation:**
- Default bind to `127.0.0.1` (localhost only)
- Token authentication for multi-node deployments
- `--bind` flag required for non-localhost listening

### 2.2 Deserialization Attacks

**Pattern:** Untrusted data is deserialized into complex objects, triggering arbitrary code execution or resource exhaustion.

**Historical examples:**
- Java: Apache Commons Collections deserialization CVEs
- Python: pickle deserialization RCE
- Distributed systems using Java serialization (Hadoop, Spark) were particularly vulnerable

**Relativist mitigation:**
- **bincode is not exploitable for RCE** — it deserializes into fixed Rust structs, not arbitrary objects. Rust's type system prevents the object-graph-based attacks that plague Java/Python.
- **However, resource exhaustion is possible:** A malicious message could claim to contain billions of agents, causing OOM during deserialization.
- **Defense:** Validate message size before deserialization. SPEC-06 already specifies length-prefixed framing — enforce maximum message size (configurable, default 256 MB).

### 2.3 Denial of Service (DoS)

**Pattern:** Attacker sends malformed/excessive requests to exhaust coordinator resources.

**Relevant vectors for Relativist:**
| Vector | Risk | Mitigation |
|--------|------|------------|
| Connection flood | Exhaust file descriptors | Max connections limit (e.g., 1024) |
| Registration spam | Fill worker list with fake workers | Token auth required for registration |
| Large message | OOM on deserialization | Max message size check |
| Slow loris | Hold connections open indefinitely | Connection timeout (30s idle) |
| Heartbeat flood | CPU on heartbeat processing | Rate limit per connection |

### 2.4 Man-in-the-Middle (MITM)

**Pattern:** Attacker intercepts communication between coordinator and workers, modifying or reading messages.

**Relativist mitigation:**
- TLS (PESQ-017) prevents MITM when enabled
- HMAC-SHA256 (PESQ-018) detects message tampering when TLS is disabled
- Without either: only CRC32 (no security, just corruption detection)

### 2.5 Malicious Worker

**Pattern:** A compromised worker returns incorrect results, poisoning the computation.

**Relativist context:**
- IC reduction is deterministic (P1). A malicious worker returning wrong results will produce a net that doesn't reduce correctly.
- **Detection:** Not feasible in v1 (would require redundant computation or result verification).
- **Mitigation:** Token auth limits workers to authorized nodes. In trusted environments, this is sufficient.
- **Future:** Redundant reduction (dispatch same partition to 2 workers, compare) for Byzantine fault tolerance. Not v1.

### 2.6 Information Disclosure

**Pattern:** Metrics, health endpoints, or error messages leak sensitive information.

**Relativist mitigation:**
- `/metrics` endpoint: Only exposes aggregate statistics, no net data
- Error messages: Never include raw net data or token values
- Logs: Token value is logged once at startup (coordinator only); never in worker logs
- Health endpoint: Only "ok" / "not ready" — no internal state

---

## 3. Security Checklist for Relativist v1

| # | Control | Priority | Spec |
|---|---------|----------|------|
| S1 | Default bind to localhost | MUST | SPEC-10 |
| S2 | Token authentication for registration | MUST | SPEC-10 |
| S3 | Maximum message size enforcement | MUST | SPEC-06, SPEC-10 |
| S4 | Connection limit (max workers) | SHOULD | SPEC-10 |
| S5 | Connection timeout (idle) | SHOULD | SPEC-06, SPEC-10 |
| S6 | TLS support (feature-flagged) | SHOULD | SPEC-10 |
| S7 | HMAC integrity (when no TLS) | MAY | SPEC-10 |
| S8 | Rate limiting per connection | MAY | SPEC-10 |
| S9 | No sensitive data in logs | MUST | SPEC-11 |
| S10 | No sensitive data in metrics | MUST | SPEC-11 |

---

## 4. Lessons for Relativist

### L1: Default to Secure (Localhost Bind) [ADOPT]
Bind to 127.0.0.1 by default. Require explicit `--bind 0.0.0.0` for network access. This prevents accidental exposure.
→ Informs: SPEC-10, SPEC-07

### L2: Enforce Message Size Limits [ADOPT]
Reject messages exceeding configurable maximum (default 256 MB). Check length prefix before allocating memory for deserialization.
→ Informs: SPEC-06, SPEC-10

### L3: bincode is Safe Against RCE [ADOPT]
Rust's type system + bincode's fixed-schema deserialization eliminates the class of deserialization RCE attacks. Document this as a security advantage.
→ Informs: SPEC-10

### L4: Connection Limits and Timeouts [ADOPT]
Limit max connections and enforce idle timeouts. This prevents resource exhaustion from connection floods or slow-loris attacks.
→ Informs: SPEC-10, SPEC-06

### L5: No Byzantine Fault Tolerance in v1 [REJECT for v1]
Detecting malicious workers requires redundant computation. Not justified for a research system where all workers are operated by the same user.
→ Informs: SPEC-10

---

## 5. Sources

| Source | URL | Accessed |
|--------|-----|----------|
| OWASP Deserialization | https://owasp.org/www-project-web-security-testing-guide/latest/4-Web_Application_Security_Testing/07-Input_Validation_Testing/16-Testing_for_HTTP_Incoming_Requests | 2026-03-26 |
| Redis unauthorized access | Common knowledge, documented in Redis security docs | — |
| Spark security | https://spark.apache.org/docs/latest/security.html | 2026-03-26 |
| PESQ-017, PESQ-018 | Internal | — |
