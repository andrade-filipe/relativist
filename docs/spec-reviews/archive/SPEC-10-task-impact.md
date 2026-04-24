# SPEC-10 Task Impact Report

**Date:** 2026-04-05
**Trigger:** SPEC-10 revised from Draft v1 to Revised v2 (adversarial review)
**Source:** `SPEC-10-round2-defender.md` (14 issues addressed)

---

## 1. Summary

| Category | Count |
|----------|-------|
| Tasks updated | 18 |
| Tasks created | 0 |
| Tasks obsoleted | 0 |
| Tasks unchanged | 2 (TASK-0117, TASK-0118) |
| **Total Phase 7 tasks** | **20** |

All existing tasks were sufficient to cover the revised requirements. No new tasks were needed because the changes primarily refined existing requirements (renumbering, type signature changes, behavior clarifications) rather than introducing fundamentally new functionality. The new requirements (R28a, T10, Section 4.3-4.5) all map to existing tasks.

---

## 2. Updated Tasks

### TASK-0120: Convert security module to directory structure
**Change:** Added `error.rs` to the module tree (SPEC-10 Section 4.1 now includes `error.rs` for `TokenError` and `SecurityError`).
**Trigger:** SC-003 (SecurityError and TokenError types now defined as separate enums in a dedicated file).
**Sections modified:** Context, Acceptance Criteria, Files to Create/Modify, Key Types / Signatures.

### TASK-0121: Define TokenError and SecurityError enums
**Change:** Complete rewrite. Now defines TWO enums (`TokenError` and `SecurityError`) matching Section 4.4 exactly. Variants renamed: `InvalidBase64` -> `InvalidBase64(String)`, `InvalidTokenLength` -> `InvalidLength(usize)`, `AuthenticationFailed` -> `AuthFailed`, `ConfigurationError(String)` -> `Config(String)`, `TlsError(String)` -> `TlsConfig(String)` + `Certificate(String)`. Added `Token(#[from] TokenError)` variant. File location changed from `mod.rs` to `error.rs`. Note about `RelativistError` extension added.
**Trigger:** SC-003 (error types fully defined in Section 4.4).
**Sections modified:** Title, Context, Acceptance Criteria, Files to Create/Modify, Key Types / Signatures, Test Expectations, Notes.

### TASK-0122: Define SecurityTier enum and tier detection logic
**Change:** Added "purely flag-based with no inference" clause to context (R1 wording). Added note about R8 change (MUST-warn-and-proceed, handled in TASK-0129).
**Trigger:** SC-007 (tier detection now strictly flag-based), SC-007 (R8 behavior change).
**Sections modified:** Context, Notes.

### TASK-0123: Define AuthToken struct with generation and serialization
**Change:** Major update. Removed `PartialEq`, `Eq` from derives (now ONLY `Clone`). Changed return type of `from_base64` from `Result<Self, SecurityError>` to `Result<Self, TokenError>`. Updated test expectations to use `verify()` instead of `==`. Added `verify()` method signature to the struct (per spec's R19 type definition). Removed `serde::Serialize/Deserialize` (wire protocol uses raw `[u8; 32]` in `RegisterPayload`, not `AuthToken`). Updated R34 reference (was R36). Added dependency notes for `base64` 0.22 and `subtle` 2.x.
**Trigger:** SC-009 (PartialEq/Eq removed), SC-003 (TokenError separate enum), SC-005 (crate dependencies specified), SC-014 (R36 renumbered from R38).
**Sections modified:** Acceptance Criteria, Key Types / Signatures, Test Expectations, Dependencies Context, Notes.

### TASK-0124: Implement AuthToken constant-time verification
**Change:** Updated to reflect that `PartialEq` and `Eq` have been REMOVED (not just discouraged). Updated doc comment and notes to state `verify()` is the ONLY way to compare tokens, including in tests.
**Trigger:** SC-009 (PartialEq/Eq removed from AuthToken).
**Sections modified:** Acceptance Criteria, Key Types / Signatures, Notes.

### TASK-0125: Define SecurityConfig struct
**Change:** Removed `max_message_size` field and `DEFAULT_MAX_MESSAGE_SIZE` constant. Updated requirement references (R32->R31, R33->R32). Removed test expectation for `max_message_size`. Added note about SC-004 rationale.
**Trigger:** SC-004 (max_message_size removed, delegated to SPEC-06 R9).
**Sections modified:** Requirements, Context, Acceptance Criteria, Key Types / Signatures, Test Expectations, Notes.

### TASK-0126: Implement token file write
**Change:** Updated R12 cross-platform language to match revised spec (explicit `#[cfg(unix)]` mention, non-Unix platform documentation requirement). Updated error type reference to `src/security/error.rs`.
**Trigger:** SC-010 (R12 cross-platform behavior specified).
**Sections modified:** Acceptance Criteria, Dependencies Context.

### TASK-0127: Extend Message enum with Register, RegisterAck, RegisterNack
**Change:** Major update. Messages now use separate payload structs (`RegisterPayload`, `RegisterAckPayload`, `RegisterNackPayload`) instead of inline fields. `RegisterPayload` includes `protocol_version: u8` (current: 1). Enum variants use tuple syntax: `Register(RegisterPayload)` instead of `Register { auth_token }`. Updated test expectations. Updated notes about 7 total variants and R35 (was R37).
**Trigger:** SC-001 (registration messages fully defined), SC-014 (protocol_version in Register).
**Sections modified:** Context, Acceptance Criteria, Key Types / Signatures, Test Expectations, Notes.

### TASK-0128: Implement token validation in coordinator accept flow
**Change:** Added R36 to requirements. Added protocol_version check step (before token validation). Updated message type references to use payload structs. Updated section references (4.6, 4.8 instead of 4.3, 4.5). Added `PROTOCOL_VERSION` constant. Updated R35 reference (was R37).
**Trigger:** SC-014 (protocol_version check), SC-001 (message struct names).
**Sections modified:** Requirements, Context, Acceptance Criteria, Key Types / Signatures, Dependencies Context, Notes.

### TASK-0129: Implement network binding security
**Change:** Major update. Default bind address changed from `"127.0.0.1"` to `"127.0.0.1:9000"` (matching SPEC-13 R44). R8 behavior changed from SHOULD-refuse to MUST-warn-and-proceed (no refusal). CLI flag is `--bind` (not `--host`), superseding SPEC-07 R3. `--insecure` suppresses warnings but does not control refusal. Updated all test expectations.
**Trigger:** SC-002 (bind address contradiction), SC-007 (R8 behavior change), SC-008 (CLI extensions).
**Sections modified:** Context, Acceptance Criteria, Key Types / Signatures, Test Expectations, Notes.

### TASK-0130: Define TlsServerConfig (feature-gated)
**Change:** Error variant renamed from `SecurityError::TlsError` to `SecurityError::TlsConfig`. Added `SecurityError::Certificate` for invalid cert content. Updated dependencies context.
**Trigger:** SC-003 (error types redefined in Section 4.4).
**Sections modified:** Acceptance Criteria, Test Expectations, Dependencies Context.

### TASK-0131: Define TlsClientConfig (feature-gated)
**Change:** Error variant renamed from `SecurityError::TlsError` to `SecurityError::TlsConfig`. Added `SecurityError::Certificate`. Updated dependencies context.
**Trigger:** SC-003 (error types redefined in Section 4.4).
**Sections modified:** Acceptance Criteria, Dependencies Context.

### TASK-0132: Implement TLS handshake integration for coordinator
**Change:** Added R28a to requirements. Added note about ChannelTransport MUST NOT attempt TLS handshakes. Error variant renamed to `TlsConfig`.
**Trigger:** SC-011 (TLS/ChannelTransport interaction specified).
**Sections modified:** Requirements, Key Types / Signatures, Notes.

### TASK-0133: Implement TLS handshake integration for worker
**Change:** Added R28a to requirements. Error variant renamed to `TlsConfig`.
**Trigger:** SC-011 (TLS/ChannelTransport interaction specified).
**Sections modified:** Requirements, Key Types / Signatures.

### TASK-0134: Implement connection limits
**Change:** Requirement reference updated from R32 to R31 (renumbering due to R31 removal in Section 3.5).
**Trigger:** SC-004 (requirement renumbering).
**Sections modified:** Requirements, Context, Key Types / Signatures, Notes.

### TASK-0135: Implement idle connection timeout
**Change:** Requirement reference updated from R33 to R32 (renumbering). Added note about SHOULD language softening (SC-013).
**Trigger:** SC-004 (requirement renumbering), SC-013 (SHOULD language).
**Sections modified:** Requirements, Context, Notes.

### TASK-0136: Verify message size pre-validation in recv_frame
**Change:** Major rewrite. Removed all references to `SecurityConfig.max_message_size` (field no longer exists). Changed error variant from `MessageTooLarge` to `PayloadTooLarge` (matching SPEC-06 Section 4.4). Removed `--max-message-size` CLI flag reference. Changed dependency from TASK-0125 to TASK-0084 (NodeConfig). Requirements updated from R29/R30/R31 to R29/R30. Title updated to reflect verification nature.
**Trigger:** SC-004 (max_message_size removed from SecurityConfig, delegated to SPEC-06).
**Sections modified:** Title, Requirements, Depends on, Context, Acceptance Criteria, Key Types / Signatures, Test Expectations, Dependencies Context, Notes.

### TASK-0138: Implement SecurityConfig builder from CLI flags
**Change:** Removed `max_message_size` field from `SecurityArgs`. Removed `--max-message-size` CLI flag references. Added notes about Section 4.5 CLI extensions, `--bind` vs `--host`, and R8 behavior (no refusal).
**Trigger:** SC-004 (max_message_size removed), SC-002 (bind flag), SC-007 (R8 behavior), SC-008 (CLI extensions).
**Sections modified:** Context, Key Types / Signatures, Notes.

### TASK-0139: Security integration tests
**Change:** Added T10 (AuthToken Debug redaction test). Updated T2 to use `verify()` instead of `==`. Updated T1 to use `!t1.verify(&t2)` instead of `assert_ne!`. Updated T5 to reference `max_payload_size` (SPEC-06 R9). Updated total from 9 to 10 test requirements.
**Trigger:** SC-009 (PartialEq removed, verify() required), SC-006 (T10 for Debug redaction), SC-004 (max_payload_size).
**Sections modified:** Requirements, Context, Acceptance Criteria, Key Types / Signatures, Test Expectations.

---

## 3. Unchanged Tasks

### TASK-0117: Enforce Core/Infrastructure layer boundary
**Reason:** This is a SPEC-13 task. Its mention of SPEC-10 is only in the module classification comment (`/// Security: token auth, optional TLS (SPEC-10)`), which remains accurate.

### TASK-0118: Feature-gated module stubs for tls, metrics, otel
**Reason:** This is a SPEC-13 task. Its reference to SPEC-10 is contextual (Phase 7 implementation). The stub signatures are placeholders that will be replaced by the real implementation. No type signature changes affect this task.

---

## 4. Requirement Coverage Verification

Every MUST requirement in SPEC-10 Revised v2 maps to at least one task:

| Requirement | Level | Task(s) | Verified |
|-------------|-------|---------|----------|
| R1 (three tiers, flag-based) | MUST | TASK-0122 | Yes |
| R2 (no config for Tier 1) | MUST | TASK-0122, TASK-0125, TASK-0138 | Yes |
| R3 (tier detection from flags) | MUST | TASK-0122, TASK-0138 | Yes |
| R4 (TLS without token SHOULD reject) | SHOULD | TASK-0122, TASK-0138 | Yes |
| R5 (bind 127.0.0.1:9000 default) | MUST | TASK-0129 | Yes |
| R6 (--bind for non-localhost) | MUST | TASK-0129 | Yes |
| R7 (WARN on 0.0.0.0 bind) | MUST | TASK-0129 | Yes |
| R8 (WARN + proceed on 0.0.0.0 no token) | MUST | TASK-0129 | Yes |
| R9 (token generation via OsRng) | MUST | TASK-0123, TASK-0137 | Yes |
| R10 (base64 encoding, base64 crate) | MUST | TASK-0123, TASK-0137 | Yes |
| R11 (log token once at INFO) | MUST | TASK-0126 | Yes |
| R12 (token file 0600, cross-platform) | SHOULD | TASK-0126 | Yes |
| R13 (worker token from CLI/env) | MUST | TASK-0138 | Yes |
| R14 (token in Register message) | MUST | TASK-0127, TASK-0128 | Yes |
| R15 (constant-time verify, subtle crate) | MUST | TASK-0124, TASK-0137 | Yes |
| R16 (RegisterNack on auth failure) | MUST | TASK-0128 | Yes |
| R17 (RegisterAck on success) | MUST | TASK-0128 | Yes |
| R18 (per-session token) | MUST | TASK-0123 | Yes |
| R19 (AuthToken type, Message extension) | MUST | TASK-0123, TASK-0127 | Yes |
| R20 (TLS feature-gated) | MUST | TASK-0130, TASK-0131, TASK-0137 | Yes |
| R21 (rustls + tokio-rustls) | MUST | TASK-0130, TASK-0131, TASK-0137 | Yes |
| R22 (TLS 1.3 only) | MUST | TASK-0130, TASK-0131 | Yes |
| R23 (server TLS only, no mTLS) | MUST | TASK-0130, TASK-0131 | Yes |
| R24 (TLS wraps wire protocol) | MUST | TASK-0132, TASK-0133 | Yes |
| R25 (coordinator TLS CLI flags) | MUST | TASK-0130, TASK-0138 | Yes |
| R26 (worker TLS CLI flag) | MUST | TASK-0131, TASK-0138 | Yes |
| R27 (self-signed certs) | MUST | TASK-0130, TASK-0131 | Yes |
| R28 (TLS handshake timing) | MUST | TASK-0132, TASK-0133 | Yes |
| R28a (ChannelTransport no TLS) | MUST | TASK-0132, TASK-0133 | Yes |
| R29 (payload size check, SPEC-06 R9) | MUST | TASK-0136 | Yes |
| R30 (PayloadTooLarge error) | MUST | TASK-0136 | Yes |
| R31 (max connections SHOULD) | SHOULD | TASK-0134 | Yes |
| R32 (idle timeout SHOULD) | SHOULD | TASK-0135 | Yes |
| R33 (rate limiting MAY) | MAY | Not tasked (MAY) | N/A |
| R34 (token not in logs/Debug) | MUST | TASK-0123 | Yes |
| R35 (generic error messages) | MUST | TASK-0121, TASK-0128 | Yes |
| R36 (protocol_version in Register) | SHOULD | TASK-0127, TASK-0128 | Yes |
| R37 (NOT in v1 exclusions) | MUST NOT | All tasks (implicit) | Yes |
| T1 (token uniqueness) | MUST | TASK-0139 | Yes |
| T2 (token roundtrip via verify) | MUST | TASK-0139 | Yes |
| T3 (token validation scenarios) | MUST | TASK-0139 | Yes |
| T4 (TLS handshake tests) | MUST | TASK-0139 | Yes |
| T5 (message size limit) | MUST | TASK-0139 | Yes |
| T6 (idle timeout test) | SHOULD | TASK-0139 | Yes |
| T7 (rejected worker isolation) | MUST | TASK-0139 | Yes |
| T8 (localhost binding test) | SHOULD | TASK-0139 | Yes |
| T9 (tier detection test) | MUST | TASK-0139 | Yes |
| T10 (AuthToken Debug redaction) | MUST | TASK-0139 | Yes |

**Coverage:** All MUST requirements have at least one task. All SHOULD requirements have tasks (P1-P2 priority). MAY requirement R33 is intentionally untasked (optional). All test requirements (T1-T10) are covered by TASK-0139.

---

## 5. Cross-Spec Consistency Notes

The following inconsistencies are documented in SPEC-10 but cannot be resolved by task updates alone (they require spec revisions to SPEC-06, SPEC-07, SPEC-13):

1. **SPEC-06 Message enum** needs 3 new variants (Register, RegisterAck, RegisterNack). Covered by TASK-0127.
2. **SPEC-07 R3/R12** bind address default (`--host 0.0.0.0`) superseded by SPEC-10 R5 (`--bind 127.0.0.1:9000`). Covered by TASK-0129.
3. **SPEC-13 R11** dependency table needs `base64` and `subtle`. Covered by TASK-0137.
4. **SPEC-13 R17** `RelativistError` needs `Security(#[from] SecurityError)` variant. Covered by TASK-0103 (Phase 6).
5. **SPEC-13 R16** `ProtocolError` should use `PayloadTooLarge` (not `MessageTooLarge`). Covered by TASK-0136.

These will be addressed when the dependent specs (SPEC-06, SPEC-07, SPEC-13) go through their own revision cycles.
