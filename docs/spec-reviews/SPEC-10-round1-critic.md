# SPEC-10 -- Round 1: Spec Critic Review

**Date:** 2026-04-05
**Reviewer:** Spec Critic (adversarial)
**Target:** SPEC-10-security.md (status: Draft v1)
**Predecessors consulted:** SPEC-00, SPEC-01, SPEC-06, SPEC-07, SPEC-13

---

## Overall Assessment

SPEC-10 provides a well-structured three-tier security model with clear rationale and practical design choices. The separation of concerns (development vs. private network vs. production) is sound, and the decision to exclude mTLS, HMAC, and token rotation for v1 is well-justified. However, the spec has a critical dependency on `Register`/`RegisterAck`/`RegisterNack` message variants that do not exist in SPEC-06's `Message` enum, introduces a CLI flag naming conflict with SPEC-07 (`--bind` vs `--host`), a default bind address contradiction with SPEC-07 (`127.0.0.1` vs `0.0.0.0`), references error types (`SecurityError`, `TokenError`) that are not defined anywhere, omits the `base64` and `subtle` crates from SPEC-13's dependency table, and has a `SecurityConfig.max_message_size` field that duplicates and conflicts with SPEC-06's `NodeConfig.max_payload_size`. Multiple MUST requirements lack sufficient detail for implementation without guessing.

**Verdict:** NEEDS REVISION

---

## Issues

### SC-001: Register/RegisterAck/RegisterNack messages not defined in SPEC-06 Message enum
**Severity:** CRITICAL
**Axis:** Consistency
**Section:** 3.3 (Token Authentication)
**Requirement:** R14, R16, R17, R19
**Problem:** SPEC-10 R14 says: "Workers MUST include the token in the `Register` message sent to the coordinator upon connection (SPEC-13, R25)." R16 references `RegisterNack` and R17 references `RegisterAck`. R19 says "The `Register` message (SPEC-06 Message enum) MUST be extended to include an `auth_token: Option<[u8; 32]>` field." However, SPEC-06's `Message` enum (Section 4.1) contains exactly four variants: `AssignPartition`, `Shutdown`, `PartitionResult`, `Error`. There are no `Register`, `RegisterAck`, or `RegisterNack` variants. This was already identified in the SPEC-13 review (SC-002 there) as a cross-spec gap: SPEC-13's FSM references these messages but SPEC-06 never defines them. SPEC-10 now compounds the problem by building its entire authentication flow on top of messages that do not exist in the canonical wire protocol spec.

**Impact if unresolved:** The implementer cannot build token authentication without first inventing the `Register`/`RegisterAck`/`RegisterNack` messages. Since SPEC-06 is the canonical wire protocol spec, the implementer has no authoritative definition of these message structures, their serialization format, or their place in the FSM transitions.

**Suggested resolution:** Either (a) SPEC-10 MUST define the full `Register`, `RegisterAck`, and `RegisterNack` message structures with complete Rust type signatures (including all fields, not just `auth_token`), their serialization requirements, and a note that they extend SPEC-06's `Message` enum, OR (b) SPEC-06 MUST be revised first to include these three variants, and SPEC-10 should reference them. Option (a) is recommended because SPEC-10 already has the security-relevant fields and can own the full definition.

---

### SC-002: Default bind address contradicts SPEC-07
**Severity:** CRITICAL
**Axis:** Consistency
**Section:** 3.2 (Network Binding)
**Requirement:** R5, R6
**Problem:** SPEC-10 R5 states: "The coordinator MUST bind to `127.0.0.1` by default." SPEC-10 R6 states: "To bind to a non-localhost address, the operator MUST explicitly provide the `--bind` CLI flag." However, SPEC-07 R3 defines the coordinator's `--host` flag with `default_value = "0.0.0.0"`, and SPEC-07 R12 explicitly lists `host: 0.0.0.0 (coordinator)` as the default. SPEC-13 R44 uses `--bind` with default `127.0.0.1:9000`, which agrees with SPEC-10 but disagrees with SPEC-07. There are TWO conflicts:
1. **Flag name:** SPEC-07 uses `--host`, SPEC-10 uses `--bind`, SPEC-13 uses `--bind`.
2. **Default value:** SPEC-07 defaults to `0.0.0.0`, SPEC-10 and SPEC-13 default to `127.0.0.1`.

**Impact if unresolved:** The implementer does not know whether the coordinator binds to localhost or all interfaces by default. If `0.0.0.0` (SPEC-07), the coordinator is accidentally exposed on all interfaces with no auth in Tier 1, which is precisely what SPEC-10 R5 tries to prevent. If `127.0.0.1` (SPEC-10/SPEC-13), existing Docker Compose configurations in SPEC-07 that rely on workers connecting from other containers will fail.

**Suggested resolution:** SPEC-07 R3 and R12 MUST be revised to use `--bind` with default `127.0.0.1:9000`, matching SPEC-10 and SPEC-13. SPEC-10 should add an explicit note: "This requirement supersedes SPEC-07 R3 and R12 for the default bind address." The Docker Compose configuration in SPEC-07 R38 should use `--bind 0.0.0.0:9000` explicitly.

---

### SC-003: SecurityError and TokenError types not defined
**Severity:** HIGH
**Axis:** Completeness
**Section:** 4.1 (Security Module Structure), 3.3 (Token Authentication)
**Requirement:** R19 (AuthToken type), R25 (TlsServerConfig), R26 (TlsClientConfig)
**Problem:** The `AuthToken::from_base64()` method returns `Result<Self, TokenError>`. `TlsServerConfig::from_pem_files()` and `TlsClientConfig::from_ca_pem()` both return `Result<Self, SecurityError>`. Neither `TokenError` nor `SecurityError` are defined anywhere -- not in SPEC-10, not in SPEC-13 (which defines `NetError`, `ReductionError`, `PartitionError`, `MergeError`, `ProtocolError`, `CoordinatorError`, `WorkerError`). Additionally, SPEC-13 R17's `RelativistError` does not include a `Security(#[from] SecurityError)` variant. The security module's errors have no integration path to the top-level error type.

**Impact if unresolved:** The implementer must invent both error types and decide which variants they contain. Without a `SecurityError` in `RelativistError`, security-related errors cannot propagate through the standard error handling chain defined in SPEC-13 R15-R17.

**Suggested resolution:** SPEC-10 MUST define both error enums with concrete Rust code:
```rust
#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("invalid base64 encoding: {0}")]
    InvalidBase64(String),
    #[error("invalid token length: expected 32 bytes, got {0}")]
    InvalidLength(usize),
}

#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("token error: {0}")]
    Token(#[from] TokenError),
    #[error("TLS configuration error: {0}")]
    TlsConfig(String),
    #[error("certificate error: {0}")]
    Certificate(String),
    #[error("authentication failed")]
    AuthFailed,
}
```
SPEC-10 MUST also note that `RelativistError` (SPEC-13 R17) requires a `Security(#[from] SecurityError)` variant, and that `ProtocolError` (SPEC-13 R16) should move `AuthFailed` to `SecurityError` or reference it.

---

### SC-004: max_message_size conflicts with SPEC-06 max_payload_size
**Severity:** HIGH
**Axis:** Consistency
**Section:** 3.5 (Message Size Limits), 4.2 (SecurityConfig)
**Requirement:** R29, R30, R31
**Problem:** SPEC-10 introduces `SecurityConfig.max_message_size: u32` (R30: "default maximum message size MUST be 256 MiB") and a CLI flag `--max-message-size`. SPEC-06 already defines `NodeConfig.max_payload_size: u32` (R9: "maximum payload size MUST be configurable, with a default of 256 MiB") and `ProtocolError::PayloadTooLarge`. These are the same concept with different names and different owners. The field names differ (`max_message_size` vs `max_payload_size`), they live in different config structs (`SecurityConfig` vs `NodeConfig`), and they have slightly different semantics: "message size" could include the 8-byte frame header, while "payload size" excludes it.

Additionally, SPEC-10 R31 references `ProtocolError::MessageTooLarge` while SPEC-06 uses `ProtocolError::PayloadTooLarge`, and SPEC-13 R16 uses `ProtocolError::MessageTooLarge`. The error variant name is inconsistent across three specs.

**Impact if unresolved:** Two config values controlling the same enforcement point. If they disagree (e.g., `SecurityConfig.max_message_size = 128M` but `NodeConfig.max_payload_size = 256M`), which one is used by `recv_frame()`? The implementer must resolve this at coding time.

**Suggested resolution:** The message size limit is a wire protocol concern, not a security concern. Remove `max_message_size` from `SecurityConfig` and `--max-message-size` from SPEC-10. The canonical owner is SPEC-06 `NodeConfig.max_payload_size`. SPEC-10 should reference it: "The max payload size limit defined in SPEC-06 R9 serves as a security defense (R29). See SPEC-06 for configuration." Unify the error variant name to one canonical form across all specs.

---

### SC-005: Missing base64 and subtle crate dependencies
**Severity:** HIGH
**Axis:** Completeness
**Section:** 3.3 (Token Authentication), Section 4.3 (Token Lifecycle)
**Requirement:** R10, R15
**Problem:** R10 requires base64 encoding/decoding of tokens. R15 requires constant-time comparison. SPEC-13 R11 lists the always-on dependencies and R12 lists feature-gated dependencies. Neither list includes:
- A `base64` crate (needed for `AuthToken::to_base64()` and `AuthToken::from_base64()`).
- The `subtle` crate (the standard Rust crate for constant-time comparison, providing `ConstantTimeEq`).

The `AuthToken` struct in SPEC-10 derives `PartialEq`, which uses standard comparison, not constant-time. Yet R15 mandates constant-time comparison for `verify()`. The `PartialEq` derive is actively dangerous: any code using `==` on `AuthToken` would bypass the constant-time `verify()` method.

**Impact if unresolved:** (1) No `base64` crate means the implementer must either write their own base64 or add an undocumented dependency. (2) Without `subtle`, implementing constant-time comparison correctly is error-prone (manual byte-by-byte XOR is needed). (3) The `PartialEq` derive enables accidental non-constant-time comparison.

**Suggested resolution:** (1) Add `base64` (version 0.22) to SPEC-13 R11's always-on dependency table. (2) Add `subtle` (version 2.x) to SPEC-13 R11's always-on dependency table (it has zero dependencies and is tiny). (3) In SPEC-10, remove `PartialEq` from `AuthToken`'s derive list and note: "`AuthToken` MUST NOT implement `PartialEq` or `Eq`. All comparisons MUST use the `verify()` method which delegates to `subtle::ConstantTimeEq`."

---

### SC-006: Zeroize mentioned but not specified or dependency-listed
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 3.7 (Defensive Measures)
**Requirement:** R36
**Problem:** R36 says: "Core dumps or panic messages (use `Zeroize` or avoid including token in panic paths)." The `Zeroize` trait is from the `zeroize` crate (by RustCrypto), but this crate appears nowhere in SPEC-13's dependency tables. Furthermore, if `AuthToken` should implement `Zeroize`, this has implications for its `Drop` implementation and for how `SecurityConfig` handles the token field. The requirement is stated parenthetically as an option, not as a firm directive: "use `Zeroize` OR avoid including token in panic paths." This ambiguity leaves the implementer unsure whether to add the dependency.

**Impact if unresolved:** If the implementer chooses the "avoid including token in panic paths" approach, there is no test to verify this. If they choose `Zeroize`, they add an unlisted dependency.

**Suggested resolution:** Pick one approach and make it explicit. Recommended: "The `AuthToken` MUST NOT implement `Debug` with full content display. The `Debug` implementation MUST print `AuthToken([REDACTED])`. This eliminates token leakage via panic messages and debug logging. The `zeroize` crate MAY be used but is not required for v1." Remove the ambiguous parenthetical.

---

### SC-007: --token auto vs no --token ambiguity in tier detection
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 3.3 (Token Authentication), 3.1 (Three-Tier Security Model)
**Requirement:** R3, R9
**Problem:** R9 states: "The coordinator MUST generate a 256-bit authentication token when the `--token auto` flag is provided **or when `--token <value>` is not specified but token auth is required (Tier 2/3)**." But R3 says: "No `--token` and no TLS flags: Tier 1." This creates a circular dependency: the tier is determined by whether `--token` is present (R3), but whether to auto-generate a token depends on the tier (R9). Specifically, if the user runs `relativist coordinator --bind 0.0.0.0:9000 --workers 2 --input net.bin` (no `--token`, no TLS), is this Tier 1 (R3: no token, no TLS) or should a token be auto-generated because binding to `0.0.0.0` implies a non-development scenario?

R8 attempts to address this: "The coordinator SHOULD refuse to bind to `0.0.0.0` without token authentication... This MAY be overridden with a `--insecure` flag." But this is a SHOULD, not a MUST, and the `--insecure` flag is not defined in the CLI args anywhere.

**Impact if unresolved:** Ambiguous behavior for `--bind 0.0.0.0` without `--token`. The implementer must decide: warn-and-continue? refuse? auto-generate?

**Suggested resolution:** Make tier detection strictly flag-based with no inference. Restate R3 more precisely:
- `--token` absent: Tier 1.
- `--token auto` or `--token <value>`, no TLS flags: Tier 2.
- `--token` present AND `--tls-cert`/`--tls-key` present: Tier 3.
Remove the "when token auth is required (Tier 2/3)" clause from R9. Add: "If binding to `0.0.0.0` without `--token`, the coordinator MUST emit a WARN log (R7) but MUST proceed (no refusal). R8 is downgraded from SHOULD to MAY."

---

### SC-008: SPEC-13 R44 omits --token for coordinator CLI
**Severity:** MEDIUM
**Axis:** Consistency
**Section:** 3.3 (Token Authentication)
**Requirement:** R9, R11, R12
**Problem:** SPEC-10 R9 requires the coordinator to accept `--token auto` or `--token <value>`. SPEC-10 R12 requires `--token-file`. But SPEC-13 R44, which defines the canonical coordinator CLI arguments, lists only: `--bind`, `--workers`, `--input`, `--output`, `--tls-cert`, `--tls-key`. No `--token`, no `--token-file`, no `--insecure`, no `--max-message-size`. SPEC-07 R3 similarly omits these flags.

**Impact if unresolved:** The implementer following SPEC-13 R44 or SPEC-07 R3 as the authoritative CLI spec will not implement token authentication on the coordinator side.

**Suggested resolution:** SPEC-10 MUST note that it extends SPEC-13 R44 and SPEC-07 R3 with the following additional coordinator CLI flags: `--token`, `--token-file`, and `--insecure`. Alternatively, SPEC-13 R44 should be amended (requires a SPEC-13 revision).

---

### SC-009: AuthToken derives PartialEq but R15 requires constant-time comparison
**Severity:** MEDIUM
**Axis:** Invariant Preservation
**Section:** 3.3 (Token Authentication)
**Requirement:** R15, R19
**Problem:** The `AuthToken` struct definition in R19 derives `PartialEq` and `Eq`. This means `token_a == token_b` compiles and works, but uses standard short-circuit byte comparison, which is NOT constant-time. R15 mandates "constant-time comparison to prevent timing side-channel attacks" via the `verify()` method. The coexistence of `PartialEq` and a constant-time `verify()` method is a footgun: any code path that accidentally uses `==` instead of `verify()` silently introduces a timing side-channel.

**Impact if unresolved:** A security-sensitive comparison can be trivially bypassed by using `==` instead of `verify()`, and the compiler will not warn about it.

**Suggested resolution:** Remove `PartialEq` and `Eq` from the `AuthToken` derive list. If equality comparison is needed for testing, provide it only in `#[cfg(test)]` blocks. The spec should state: "`AuthToken` MUST NOT implement `PartialEq`. All comparisons MUST go through `AuthToken::verify()`."

---

### SC-010: Token file mode 0600 is POSIX-only, no Windows equivalent
**Severity:** LOW
**Axis:** Completeness
**Section:** 3.3 (Token Authentication)
**Requirement:** R12
**Problem:** R12 says: "The coordinator SHOULD write the generated token to a file with mode `0600` (owner read/write only)." File mode `0600` is a Unix/POSIX concept. On Windows (which is the development platform per the environment info), there is no direct equivalent. Rust's `std::fs::Permissions` on Windows does not support Unix permission bits. The spec does not address cross-platform behavior.

**Impact if unresolved:** The implementer will either (a) use `#[cfg(unix)]` to conditionally set permissions (leaving Windows files world-readable), or (b) skip the permission setting entirely, or (c) add a platform-specific crate. None of these are specified.

**Suggested resolution:** Add a note: "File permission `0600` applies to Unix-like systems only. On non-Unix platforms, the file SHOULD be created in the current user's home directory or a platform-appropriate secure location. The implementation SHOULD use `#[cfg(unix)]` for the `chmod` call and document the limitation on other platforms."

---

### SC-011: TLS integration with ChannelTransport unspecified
**Severity:** LOW
**Axis:** Completeness
**Section:** 3.4 (TLS Support)
**Requirement:** R24, R28
**Problem:** R24 states TLS wraps the wire protocol transparently. R28 says TLS handshake occurs on `TcpTransport`. SPEC-13 R29-R31 define two Transport implementations: `TcpTransport` and `ChannelTransport`. SPEC-10 only addresses TLS for `TcpTransport`. When running integration tests via `ChannelTransport` (in-memory, SPEC-13 R31), TLS is not applicable. However, the spec does not state this explicitly. If `SecurityConfig` has `tier = Production`, but the test uses `ChannelTransport`, what happens? Does the system require TLS and fail? Or does `ChannelTransport` bypass TLS silently?

**Impact if unresolved:** Integration tests in Tier 3 mode may fail if the security layer expects TLS on all transports, or silently bypass security if `ChannelTransport` ignores TLS config.

**Suggested resolution:** Add a note in Section 3.4: "TLS applies only to `TcpTransport`. `ChannelTransport` (SPEC-13 R31) does not support TLS. Integration tests using `ChannelTransport` MAY run at Tier 1 or Tier 2 regardless of the TLS feature flag. Tier 3 integration tests MUST use `TcpTransport` (localhost)."

---

### SC-012: R35 is a documentation requirement, not a testable security requirement
**Severity:** LOW
**Axis:** Testability
**Section:** 3.7 (Defensive Measures)
**Requirement:** R35
**Problem:** R35 says: "The use of bincode for deserialization MUST be documented as a security advantage." This is a MUST requirement about documentation, not about behavior. It is not testable via automated tests. It would be more appropriate as a SHOULD or as a note in the Rationale section.

**Impact if unresolved:** This requirement inflates the MUST count without adding any testable guarantee. An implementer could satisfy it by adding a comment in the code, but there is no way to verify compliance automatically.

**Suggested resolution:** Move R35 to Section 5 (Rationale) as a design note. If it must remain as a requirement, downgrade to SHOULD and reframe: "The security module documentation SHOULD note that bincode deserialization into fixed Rust structs eliminates arbitrary-code-execution deserialization attacks."

---

### SC-013: T6 uses SHOULD but describes MUST behavior
**Severity:** LOW
**Axis:** Testability
**Section:** 6 (Test Requirements)
**Requirement:** T6
**Problem:** T6 says: "Connection idle timeout MUST be tested: a connection that sends no messages for longer than `idle_timeout` MUST be closed by the coordinator. **(SHOULD)**" The test description uses MUST language for the behavior but the test requirement itself is SHOULD. Meanwhile, R33 (which defines idle timeout) is also SHOULD. The test requirement level should match the requirement level it verifies.

**Impact if unresolved:** Minor confusion about whether idle timeout testing is mandatory or optional.

**Suggested resolution:** Since R33 is SHOULD, T6 should also be SHOULD. The internal MUST language ("MUST be closed") should be softened to: "a connection that sends no messages for longer than `idle_timeout` SHOULD be closed." This is already the case but the formatting is confusing. Clarify.

---

### SC-014: R38 magic bytes / protocol version unspecified
**Severity:** LOW
**Axis:** Completeness
**Section:** 3.7 (Defensive Measures)
**Requirement:** R38
**Problem:** R38 is a SHOULD requirement that says: "The `Message` enum SHOULD include version or magic bytes as the first field to allow fast rejection of non-Relativist connections." However, it provides no concrete specification: what magic bytes? How many bytes? Where in the frame (before the length-prefix header, or as the first field of the first message)? Is this per-connection or per-message? The parenthetical "This MAY be implemented as a protocol version prefix in the first message of each connection" suggests per-connection, but this contradicts "as the first field" of the Message enum (which would be per-message).

**Impact if unresolved:** If implemented, it could break framing compatibility with SPEC-06 (which defines the frame header as exactly 8 bytes: 4 length + 4 CRC32). Adding magic bytes before the header changes the frame format; adding them inside the payload changes the Message serialization.

**Suggested resolution:** Either (a) specify concretely: "The first message on each new connection MUST be a `Register` (or `Hello`) message containing a 4-byte magic (`b"RLVT"`) and a protocol version (`u8`). This is checked before any further message processing." Or (b) remove R38 entirely and note it as a v2 candidate, since the current protocol only accepts `Register` as the first message anyway, which serves a similar purpose.

---

## Summary Table

| ID | Severity | Axis | Requirement | Title |
|----|----------|------|-------------|-------|
| SC-001 | CRITICAL | Consistency | R14, R16, R17, R19 | Register/RegisterAck/RegisterNack not in SPEC-06 Message enum |
| SC-002 | CRITICAL | Consistency | R5, R6 | Default bind address contradicts SPEC-07 |
| SC-003 | HIGH | Completeness | R19, R25, R26 | SecurityError and TokenError types not defined |
| SC-004 | HIGH | Consistency | R29, R30, R31 | max_message_size conflicts with SPEC-06 max_payload_size |
| SC-005 | HIGH | Completeness | R10, R15 | Missing base64 and subtle crate dependencies |
| SC-006 | MEDIUM | Completeness | R36 | Zeroize mentioned but not specified or dependency-listed |
| SC-007 | MEDIUM | Completeness | R3, R9 | --token auto vs no --token ambiguity in tier detection |
| SC-008 | MEDIUM | Consistency | R9, R11, R12 | SPEC-13 R44 omits --token for coordinator CLI |
| SC-009 | MEDIUM | Invariant Preservation | R15, R19 | AuthToken PartialEq vs constant-time comparison footgun |
| SC-010 | LOW | Completeness | R12 | Token file mode 0600 is POSIX-only |
| SC-011 | LOW | Completeness | R24, R28 | TLS + ChannelTransport interaction unspecified |
| SC-012 | LOW | Testability | R35 | Documentation requirement masquerading as MUST |
| SC-013 | LOW | Testability | T6 | Test level (SHOULD) vs behavior level (MUST) mismatch |
| SC-014 | LOW | Completeness | R38 | Magic bytes / protocol version unspecified |

---

## Mandatory Fixes (must resolve before approving SPEC-10)

1. **SC-001:** Define `Register`, `RegisterAck`, `RegisterNack` message structures with full Rust types and integrate them into SPEC-06's `Message` enum (or own them in SPEC-10 with a cross-reference).
2. **SC-002:** Resolve the `--bind` vs `--host` naming and `127.0.0.1` vs `0.0.0.0` default contradiction between SPEC-07, SPEC-10, and SPEC-13.
3. **SC-003:** Define `SecurityError` and `TokenError` enums with variants and integrate into `RelativistError`.
4. **SC-004:** Deduplicate `max_message_size` / `max_payload_size`. One canonical field, one canonical config location, one canonical error variant name.
5. **SC-005:** Add `base64` and `subtle` crates to SPEC-13 R11 dependency table (or to SPEC-10's own dependency section).

## Recommended Fixes (should resolve before approving)

6. **SC-006:** Decide on Zeroize vs Debug-redaction for token leakage prevention. Remove ambiguity.
7. **SC-007:** Remove circular dependency in tier detection logic. Make it purely flag-based.
8. **SC-008:** Document that SPEC-10 extends SPEC-13 R44 and SPEC-07 R3 with additional CLI flags.
9. **SC-009:** Remove `PartialEq`/`Eq` from `AuthToken` derive list.

## Optional Fixes (may resolve or defer)

10. **SC-010:** Document cross-platform behavior for token file permissions.
11. **SC-011:** Specify ChannelTransport behavior under Tier 3 configuration.
12. **SC-012:** Move R35 to Rationale or downgrade to SHOULD.
13. **SC-013:** Align T6 test level with R33 requirement level.
14. **SC-014:** Specify magic bytes concretely or defer to v2.

---

## Checklist for SPEC-10 Author

- [ ] `Register`, `RegisterAck`, `RegisterNack` messages fully defined with Rust type signatures
- [ ] `SecurityError` and `TokenError` error enums defined with variants
- [ ] `RelativistError` extended with `Security(#[from] SecurityError)` variant (cross-ref to SPEC-13)
- [ ] `max_message_size` removed from `SecurityConfig` (delegated to SPEC-06 `NodeConfig.max_payload_size`)
- [ ] `--max-message-size` CLI flag removed (already exists as SPEC-06/SPEC-07 config)
- [ ] `base64` crate listed as always-on dependency (or noted as requirement for SPEC-13 update)
- [ ] `subtle` crate listed as always-on dependency (or noted as requirement for SPEC-13 update)
- [ ] `PartialEq` and `Eq` removed from `AuthToken` derive list
- [ ] `Debug` implementation for `AuthToken` redacts token content
- [ ] Tier detection logic rewritten without circular dependency
- [ ] CLI flag extensions to SPEC-13 R44 and SPEC-07 R3 explicitly documented
- [ ] Zeroize vs Debug-redaction decision made and documented
- [ ] Cross-platform note for R12 file permissions added
- [ ] ChannelTransport + TLS interaction documented
- [ ] R35 moved to Rationale or downgraded
- [ ] R38 magic bytes either specified concretely or deferred
- [ ] `ProtocolError` variant name unified: `MessageTooLarge` vs `PayloadTooLarge` resolved
