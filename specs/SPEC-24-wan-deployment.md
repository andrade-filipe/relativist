# SPEC-24: WAN Deployment and Security

**Status:** Draft
**Depends on:** SPEC-06 (Wire Protocol), SPEC-10 (Security), SPEC-13 (System Architecture), SPEC-17 (Transport Abstraction)
**Amends:** SPEC-06 (reconnect protocol), SPEC-07 (WAN config), SPEC-10 (strong auth replaces plaintext), SPEC-11 (security metrics)
**ROADMAP items:** 2.21 (WAN / Internet Deployment), 2.21.1 (End-to-End Security Analysis)
**References consumed:** REF-001 (Lafont 1990), REF-013 (Hassan, p.219: grid computing)
**Arguments consumed:** ARG-001 (P1-P6), ARG-004 (viability analysis)
**Briefings consumed:** BRIEF-20260415-v2-tier5-teorica (Section 2.21: D5/D6 attention, P5 wall-clock), BRIEF-20260415-v2-tier5-codebase (Section 2.21: frame.rs generics, session tracking gaps, security scaffolding)

---

## 1. Purpose

This spec lifts Relativist's "same-LAN" assumption (SPEC-09 R27: "TCP over LAN") so that workers can join a coordinator across the public Internet, over typical home/office NATs, with production-grade security, connection resilience, and performance guarantees. This is what transforms Relativist from a LAN-bound research prototype into a grid computing system in the volunteer-computing sense (the title of the TCC).

Seven sub-components are specified:
1. **Mandatory TLS 1.3** for all WAN connections.
2. **Strong authentication** via mTLS (mutual TLS with per-worker certificates).
3. **NAT traversal** via a rendezvous/relay server.
4. **Session-aware reconnection** with partition preservation.
5. **Adaptive timeouts** tuned to observed RTT.
6. **WAN discovery** via rendezvous URLs.
7. **Abuse mitigation** for Internet-facing coordinators.

Additionally, item 2.21.1 (End-to-End Security Analysis) is specified as a deliverable: a STRIDE-based threat model document that validates the security design before implementation.

---

## 2. Definitions

Terms defined in SPEC-00, SPEC-06, SPEC-10, SPEC-13, and SPEC-17 are used without redefinition. Terms introduced in this spec:

| Term | Definition |
|------|-----------|
| **WAN Mode** | A runtime configuration where TLS, strong authentication, session tracking, and adaptive timeouts are enabled. Activated by `--wan` flag or `transport.wan = true` in config. |
| **Relay Server** | A lightweight always-on service (`relativist-relay`) with a public IP that forwards framed messages between coordinator and workers. Enables NAT traversal without hole-punching. |
| **Session ID** | A cryptographically random 128-bit identifier assigned to each worker at registration. Survives TCP connection loss for reconnection. |
| **mTLS** | Mutual TLS: both coordinator and worker present certificates. The coordinator's CA signs per-worker certificates; each party verifies the other. |
| **Certificate Authority (CA)** | The coordinator acts as a local CA, issuing short-lived certificates to workers during an out-of-band enrollment step. |
| **Reconnect Window** | The configurable time (default: 60s) after connection loss during which the coordinator holds the worker's partition assignment and accepts reconnection with the original session ID. |
| **Adaptive Timeout** | A per-worker timeout computed from observed RTT using exponential weighted moving average (EWMA): `timeout = ewma_rtt * multiplier`. Replaces fixed timeouts for WAN. |
| **Rendezvous URL** | A well-known URL (pointing to the relay server or coordinator) that workers use to discover and connect to the coordinator. |
| **STRIDE** | A threat modeling framework: Spoofing, Tampering, Repudiation, Information Disclosure, Denial of Service, Elevation of Privilege. |

---

## 3. Requirements

### 3.1 TLS 1.3 (Mandatory for WAN)

**R1.** When WAN mode is enabled, ALL coordinator↔worker connections MUST use TLS 1.3 via `rustls`. Plaintext TCP MUST be rejected. **(MUST)**

**R2.** The coordinator MUST present a TLS certificate to workers. The certificate MAY be:
- (a) Self-signed, with the fingerprint distributed to workers out-of-band (trust-on-first-use).
- (b) A certificate signed by a public CA (e.g., Let's Encrypt) if the coordinator has a DNS name.
**(MUST)**

**R3.** Workers MUST verify the coordinator's certificate fingerprint (for self-signed) or certificate chain (for CA-signed) before sending any protocol messages. Connection MUST be terminated if verification fails. **(MUST)**

**R4.** The `tls` feature flag (already in `Cargo.toml`) MUST be required for WAN mode. Attempting to enable WAN mode without the `tls` feature MUST produce a compile-time error. **(MUST)**

**R5.** TLS handshake MUST complete within a configurable timeout (default: 10s). Connections that exceed this timeout MUST be dropped before entering the protocol FSM. **(MUST)**

### 3.2 Strong Authentication (mTLS)

**R6.** WAN mode MUST use mutual TLS: both coordinator and worker present certificates. The coordinator validates the worker's certificate; the worker validates the coordinator's certificate. **(MUST)**

**R7.** The coordinator MUST act as a local Certificate Authority (CA): it generates a root CA certificate at first startup and stores it in a configurable directory (default: `~/.relativist/ca/`). **(MUST)**

**R8.** Worker enrollment MUST be a two-step process:
1. The coordinator generates a signed certificate for the worker via `relativist enroll --worker-name <name>`. This produces a `.pem` bundle (cert + key) for the worker.
2. The worker is configured with the `.pem` bundle and the coordinator's CA certificate.
**(MUST)**

**R9.** Certificates MUST have a configurable expiration (default: 24 hours). Expired certificates MUST be rejected at the TLS handshake. **(MUST)**

**R10.** The coordinator MUST support certificate revocation: `relativist revoke --worker-name <name>` adds the certificate serial number to a revocation list. Revoked certificates MUST be rejected at the TLS handshake. **(MUST)**

**R11.** The existing SPEC-10 plaintext token authentication MUST remain available for LAN mode (backward compatibility). In WAN mode, token authentication is superseded by mTLS and MUST NOT be used as the sole authentication mechanism. **(MUST)**

### 3.3 NAT Traversal (Relay Server)

**R12.** Relativist MUST provide a relay server binary (`relativist-relay`) that forwards messages between coordinator and workers. The relay MUST have a stable public IP or DNS name. **(MUST)**

**R13.** The relay server MUST be a minimal binary with dependencies: `tokio`, `rustls`, and the Relativist framing layer (`send_frame`/`recv_frame`). It MUST NOT depend on the core IC modules (`net/`, `reduction/`, `partition/`, `merge/`). **(MUST)**

**R14.** The relay protocol MUST work as follows:
1. Coordinator connects to relay and registers as coordinator (authenticated via mTLS).
2. Workers connect to relay and send a `RelayConnect { coordinator_id }` message.
3. The relay pairs each worker with the coordinator and forwards all subsequent frames bidirectionally.
4. The relay MUST NOT inspect, modify, or cache frame payloads — it is a transparent forwarder.
**(MUST)**

**R15.** The relay MUST support concurrent connections from multiple coordinators and workers. Each coordinator↔worker pair MUST be isolated (no cross-pair message leakage). **(MUST)**

**R16.** The relay server MUST enforce TLS 1.3 on all connections. Plaintext connections MUST be rejected. **(MUST)**

**R17.** Relay latency overhead MUST be documented: one additional network hop per message (relay is in the forwarding path). For messages of size S, relay overhead is approximately `2 × RTT_relay + S/bandwidth_relay`. **(MUST)**

### 3.4 Session-Aware Reconnection

**R18.** At registration, the coordinator MUST assign a cryptographically random 128-bit `SessionId` to each worker. The `SessionId` MUST be included in the `RegisterAck` payload. **(MUST)**

**R19.** The `RegisterAckPayload` (SPEC-10) MUST be amended to include:
```rust
pub struct RegisterAckPayload {
    pub worker_id: WorkerId,
    pub session_id: SessionId,  // NEW
}
```
**(MUST)**

**R20.** If a worker's TCP connection drops mid-round, the coordinator MUST:
1. Start a reconnect timer (configurable, default: 60s).
2. Retain the worker's partition assignment (do NOT re-dispatch to another worker).
3. Accept a new connection from a client presenting the same `SessionId`.
**(MUST)**

**R21.** The `Message` enum MUST gain new variants for reconnection:
```rust
Reconnect { session_id: SessionId },
ReconnectAck { round: u32 },
ReconnectNack { reason: String },
```
**(MUST)**

**R22.** On reconnect, the worker MUST send `Reconnect { session_id }` as the first message. The coordinator MUST verify the session ID and respond with `ReconnectAck` (including the current round number so the worker can resume) or `ReconnectNack` (if the session has expired or been reassigned). **(MUST)**

**R23.** If the reconnect timer expires before the worker reconnects, the coordinator MUST:
1. Mark the session as `Expired`.
2. Re-dispatch the partition to another available worker (if SPEC-20 dynamic departure is implemented) or abort the grid loop with an error.
**(MUST)**

**R24.** If a worker reconnects after its session has been expired and its partition re-dispatched, the coordinator MUST respond with `ReconnectNack { reason: "session expired" }`. The worker MUST NOT be allowed to submit results for the expired session. This preserves D5 (exclusive agent ownership). **(MUST)**

### 3.5 Adaptive Timeouts

**R25.** In WAN mode, fixed timeouts MUST be replaced with adaptive timeouts based on observed RTT. **(MUST)**

**R26.** The coordinator MUST maintain per-worker RTT estimates using exponential weighted moving average (EWMA):
```
rtt_estimate = alpha * latest_rtt + (1 - alpha) * rtt_estimate
```
where `alpha = 0.125` (TCP standard) and `latest_rtt` is the time between sending `RoundStart`/`AssignPartition` and receiving `RoundResult`/`PartitionResult` minus the worker's reported reduction time. **(MUST)**

**R27.** Per-round deadlines for each worker MUST be computed as:
```
deadline = max(rtt_estimate * multiplier, min_deadline)
```
where `multiplier` (default: 4.0) and `min_deadline` (default: 5s) are configurable. **(MUST)**

**R28.** The `worker_connect_timeout` (SPEC-06, currently 120s) MUST be increased to 300s for WAN mode and MUST be configurable via `--wan-connect-timeout`. **(MUST)**

### 3.6 WAN Discovery

**R29.** Workers MUST support connecting to the coordinator via a rendezvous URL: `--coordinator relay://relay.example.com/coordinator-id`. **(MUST)**

**R30.** The `relay://` URL scheme MUST be parsed by the worker's connection logic and routed to the relay server instead of direct TCP. **(MUST)**

**R31.** For direct WAN connections (no relay), workers MAY connect to the coordinator's public IP/hostname as in LAN mode, but TLS and mTLS are mandatory. **(MAY)**

### 3.7 Abuse Mitigation

**R32.** Unauthenticated peers MUST be dropped at the TLS handshake (mTLS certificate validation), before any protocol message is processed. **(MUST)**

**R33.** The coordinator MUST rate-limit connection attempts per source IP: at most `N` connections per minute (default: N=10, configurable). Connections exceeding the rate limit MUST be dropped with a `RejectReason::RateLimited` log entry. **(MUST)**

**R34.** Result frames MUST be validated against `max_payload_size` (SPEC-06 R9) before any deserialization. Frames exceeding the limit MUST be rejected and the connection closed. **(MUST)**

**R35.** The coordinator MUST log all authentication failures and connection rejections via `tracing` at `warn` level, including: source IP, rejection reason, timestamp. **(MUST)**

**R36.** The relay server MUST enforce per-connection bandwidth limits (configurable, default: 100 MB/s) to prevent a single connection from saturating the relay's network capacity. **(MUST)**

### 3.8 End-to-End Security Analysis (2.21.1)

**R37.** Before SPEC-24 implementation begins, a STRIDE-based threat model document MUST be produced as a Markdown file in `docs/security/THREAT-MODEL.md`. **(MUST)**

**R38.** The threat model MUST enumerate:
1. All network-facing endpoints (coordinator, worker, relay).
2. All trust boundaries (Internet↔relay, relay↔coordinator, coordinator↔worker).
3. For each STRIDE category: specific threats, the SPEC-24 requirement that mitigates it, and residual risk.
**(MUST)**

**R39.** The threat model MUST include a trust boundary diagram (TikZ) showing the system's security architecture in WAN mode. **(MUST)**

**R40.** The threat model MUST document a residual risk register: threats NOT mitigated by SPEC-24 (e.g., Byzantine workers, relay compromise). Each entry MUST state why it is deferred and what future work would address it. **(MUST)**

**R41.** Any threats identified in the security analysis that are not covered by existing SPEC-24 requirements MUST trigger requirement amendments to SPEC-24. **(MUST)**

---

## 4. Invariant Amendments

### 4.1 SPEC-06 Amendments

**A1.** SPEC-06 R19 (persistent connections) is amended: connections MAY be lost and re-established within the reconnect window. The "persistent" guarantee applies to the session, not the TCP connection.

**A2.** SPEC-06 R30 (fixed timeouts) is amended: in WAN mode, per-round deadlines are adaptive (R26-R27), not fixed.

### 4.2 SPEC-10 Amendments

**A3.** SPEC-10 R3 (plaintext token) is superseded in WAN mode by mTLS (R6). Token-based auth remains available for LAN mode only.

**A4.** SPEC-10 R14-R17 (Register handshake) is extended: `RegisterAckPayload` gains `session_id` (R19). The handshake now includes TLS certificate validation before the Register message.

### 4.3 SPEC-01 Invariant Notes

**A5.** D5 (Exclusive Agent Ownership) is preserved by R24: a reconnected worker whose session has expired is rejected. At no point do two workers hold the same partition.

**A6.** D6 (Protocol Termination) is preserved by R27: adaptive timeouts have a configurable minimum (`min_deadline`), preventing unbounded delays. The reconnect window (R20, default 60s) is finite. Total wall-clock time per round is bounded by `max(worker_deadline) + reconnect_window`.

### 4.4 New Message Variants

The following variants MUST be appended to the `Message` enum after the last existing discriminant:

| Disc. | Variant | Direction | Purpose |
|------:|---------|-----------|---------|
| TBD | `Reconnect` | W→C | Session-aware reconnection |
| TBD+1 | `ReconnectAck` | C→W | Reconnection accepted |
| TBD+2 | `ReconnectNack` | C→W | Reconnection rejected |

Discriminant numbers depend on ordering with other specs (SPEC-19 adds 5 variants, SPEC-25 adds 1). Coordinate during implementation.

---

## 5. Non-Goals

**NG1.** Byzantine fault tolerance. WAN deployment closes the connectivity gap, not the trust gap. Malicious workers returning fabricated results are out of scope. Production volunteer computing (BOINC, Folding@home) uses redundant execution + result voting; this is a v3+ item.

**NG2.** UDP hole-punching. The relay-based approach (R12-R17) is simpler and works with all NAT types. Hole-punching (STUN/TURN-style) is a performance optimization deferred until relay latency is proven to be the bottleneck.

**NG3.** OAuth2/OIDC integration. mTLS (R6-R10) is sufficient for the TCC. Institutional identity provider integration is a deployment-specific concern deferred to v3+.

**NG4.** Multi-coordinator federation. SPEC-24 assumes a single coordinator. Federating multiple coordinators across institutions is a research problem beyond the TCC scope.

---

## 6. Architecture Diagram

```
┌──────────────────────────────────────────────────────────┐
│                    Internet                               │
│                                                          │
│  ┌─────────┐     TLS 1.3      ┌──────────────┐          │
│  │ Worker A ├────────────────►│              │          │
│  │ (home)   │   mTLS cert A   │    Relay     │          │
│  └─────────┘                  │   Server     │          │
│                               │ (public IP)  │          │
│  ┌─────────┐     TLS 1.3      │              │  TLS 1.3  │
│  │ Worker B ├────────────────►│              ├─────────►│
│  │ (office) │   mTLS cert B   └──────────────┘          │
│  └─────────┘                                            │
│                                          ┌──────────────┐│
│  ┌─────────┐     TLS 1.3 (direct)       │ Coordinator  ││
│  │ Worker C ├───────────────────────────►│ (cloud VM)   ││
│  │ (cloud)  │   mTLS cert C             │ CA + config  ││
│  └─────────┘                            └──────────────┘│
└──────────────────────────────────────────────────────────┘

Trust boundaries:
  [1] Internet ↔ Relay: TLS 1.3 + mTLS (R1, R6, R16)
  [2] Relay ↔ Coordinator: TLS 1.3 + coordinator cert (R2, R14)
  [3] Worker ↔ Coordinator (direct): TLS 1.3 + mTLS (R1, R6)
  [4] Relay internals: transparent forwarder (R14), no payload inspection
```

---

## 7. Test Strategy

### 7.1 TLS Tests

**T1. TLS mandatory in WAN mode.**
- Enable WAN mode. Attempt plaintext TCP connection. Verify connection is rejected.

**T2. Certificate verification.**
- Connect worker with wrong certificate fingerprint. Verify connection rejected at TLS handshake.

**T3. Certificate expiration.**
- Issue a certificate with 1-second expiration. Wait 2 seconds. Attempt connection. Verify rejection.

### 7.2 mTLS Tests

**T4. mTLS handshake.**
- Coordinator issues certificate to worker via `enroll`. Worker connects with issued certificate. Verify successful authentication.

**T5. Revoked certificate.**
- Issue certificate, then revoke. Attempt connection. Verify rejection.

**T6. Unauthorized worker.**
- Worker attempts connection with a self-signed certificate not issued by the coordinator's CA. Verify rejection.

### 7.3 Reconnection Tests

**T7. Successful reconnection.**
- Worker connects, receives partition, connection drops (simulated). Worker reconnects within window. Verify `ReconnectAck` with correct round. Worker completes reduction.

**T8. Expired session.**
- Worker connects, connection drops. Wait for reconnect timeout. Worker attempts reconnect. Verify `ReconnectNack`.

**T9. D5 preservation.**
- Worker A connects, connection drops. Reconnect timeout expires. Partition re-dispatched to Worker B. Worker A reconnects late. Verify `ReconnectNack` for Worker A. Verify Worker B holds the partition exclusively.

### 7.4 Adaptive Timeout Tests

**T10. RTT estimation.**
- Simulate worker with 50ms RTT. Verify EWMA converges to ~50ms after 10 rounds.
- Verify per-round deadline is `50ms * 4.0 = 200ms` (or `min_deadline` if larger).

**T11. Timeout triggers.**
- Worker exceeds adaptive deadline. Verify coordinator logs timeout and initiates reconnect/re-dispatch logic.

### 7.5 Relay Tests

**T12. Relay message forwarding.**
- Coordinator → relay → worker: send `AssignPartition`. Verify worker receives it intact.
- Worker → relay → coordinator: send `PartitionResult`. Verify coordinator receives it intact.

**T13. Relay TLS enforcement.**
- Attempt plaintext connection to relay. Verify rejection.

**T14. Relay isolation.**
- Two coordinator-worker pairs connected to same relay. Verify no cross-pair message leakage.

### 7.6 Abuse Mitigation Tests

**T15. Rate limiting.**
- Attempt 20 connections from same IP in 1 minute (limit: 10). Verify connections 11-20 are rejected.

**T16. Oversized frame rejection.**
- Send a frame exceeding `max_payload_size`. Verify connection closed before deserialization.

### 7.7 End-to-End Tests

**T17. Full WAN grid cycle.**
- Run coordinator on one process, 2 workers on another process, connected via relay (all on localhost with TLS).
  Verify G1: result matches sequential baseline.

---

## 8. Open Questions

**Q1. Relay scalability.** A single relay server is a single point of failure and a bandwidth bottleneck. For production deployment, multiple relay instances behind a load balancer would be needed. This is out of scope for the TCC but should be noted as future work.

**Q2. Certificate distribution UX.** The `enroll` command generates a `.pem` bundle that must be transferred to the worker machine. For the TCC demo, SCP/email is sufficient. For production, an automated enrollment protocol (e.g., ACME-like) would be needed.

**Q3. Relay server deployment.** Where should the relay run? A small cloud VM (e.g., t3.micro) is sufficient. The relay does not perform IC reduction — it only forwards frames. Estimated resource usage: <100 MB RAM, <1 CPU core, bandwidth proportional to frame traffic.

**Q4. IPv6 support.** All networking code in Relativist uses `SocketAddr` which supports both IPv4 and IPv6. No specific IPv6 testing has been done. WAN deployment should verify IPv6 works.

**Q5. Message variant numbering.** SPEC-19 (delta protocol) adds 5 variants (disc. 7-11), SPEC-25 (recipe gen) adds 1 variant. SPEC-24 adds 3 variants (Reconnect, ReconnectAck, ReconnectNack). Total: 16 variants in the Message enum. Coordinate discriminant assignment across all specs before implementation.
