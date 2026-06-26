# SPEC-10 -- Round 2: Defender Response

**Date:** 2026-04-05
**Defender:** Especialista em Specs
**Target:** SPEC-10-security.md
**Critic review:** SPEC-10-round1-critic.md
**Spec version:** Draft v1 -> Revised v2

---

## Summary

| Response | Count |
|----------|-------|
| ACCEPTED | 10 |
| PARTIALLY ACCEPTED | 4 |
| NOT ADDRESSED | 0 |
| **Total issues** | **14** |

---

## Responses

### SC-001: Register/RegisterAck/RegisterNack messages not defined in SPEC-06 Message enum
**Severity:** CRITICAL
**Response:** ACCEPTED
**Action taken:** SPEC-10 now owns the full definition of `Register`, `RegisterAck`, and `RegisterNack` message structures with complete Rust type signatures in a new Section 4.3 ("Registration Messages"). The three variants are defined as `RegisterPayload`, `RegisterAckPayload`, and `RegisterNackPayload` structs, with a clear note that they extend (not replace) the four variants in SPEC-06 Section 4.1, making a total of seven `Message` variants. The `Register` message includes both `protocol_version: u8` (addressing SC-014 simultaneously) and `auth_token: Option<[u8; 32]>`. `RegisterAck` includes the assigned `WorkerId`. `RegisterNack` includes a generic `reason: String`. The spec explicitly notes that these extend the SPEC-06 enum and provides the exact variant syntax to be added.
**Spec sections modified:** Section 3.3 (R14, R19 reworded), Section 4.3 (new), Section 4.6 (updated lifecycle to reference `RegisterPayload`), Section 4.8 (updated connection flow to include protocol version check)

### SC-002: Default bind address contradicts SPEC-07
**Severity:** CRITICAL
**Response:** ACCEPTED
**Action taken:** SPEC-10 R5 now explicitly states that the default bind address is `127.0.0.1:9000` (matching SPEC-13 R44) and includes a supersession note: "This requirement supersedes SPEC-07 R3 and R12 for the default bind address; SPEC-07's `--host 0.0.0.0` default is overridden to `--bind 127.0.0.1:9000`." A new Section 4.5 ("CLI Extensions") clarifies that the CLI flag is `--bind` (matching SPEC-13 R44), not `--host` (SPEC-07 R3), with an explicit note that SPEC-07 R3's `--host` is superseded. SPEC-07 itself is not modified (territory restriction), but the supersession is clearly documented in SPEC-10.
**Spec sections modified:** Section 3.2 (R5 reworded with supersession note), Section 4.5 (new, CLI flag table with --bind)

### SC-003: SecurityError and TokenError types not defined
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** A new Section 4.4 ("Error Types") defines both `TokenError` and `SecurityError` enums with concrete Rust code using `thiserror::Error`. `TokenError` has two variants: `InvalidBase64(String)` and `InvalidLength(usize)`. `SecurityError` has four variants: `Token(#[from] TokenError)`, `TlsConfig(String)`, `Certificate(String)`, `AuthFailed`, and `Config(String)`. The section also specifies that `RelativistError` (SPEC-13 R17) MUST be extended with a `Security(#[from] SecurityError)` variant, and explains the relationship between `SecurityError::AuthFailed` (security module internal) and `ProtocolError::AuthFailed` (protocol-level propagation). A new module file `error.rs` has been added to the security module structure in Section 4.1.
**Spec sections modified:** Section 4.1 (added `error.rs` to module tree), Section 4.4 (new)

### SC-004: max_message_size conflicts with SPEC-06 max_payload_size
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** Removed `max_message_size` from `SecurityConfig` and the `--max-message-size` CLI flag entirely. The message size limit is now fully delegated to SPEC-06 R9 (`NodeConfig.max_payload_size`). The old R29/R30/R31 (three requirements) have been consolidated into R29/R30 (two requirements) that reference SPEC-06 as the canonical owner. R29 now states: "The maximum payload size enforcement is defined in SPEC-06 R9... SPEC-10 does not define its own message size configuration; SPEC-06 is the canonical owner." R30 now uses the error variant name `ProtocolError::PayloadTooLarge` (matching SPEC-06 Section 4.4), not `MessageTooLarge`. A new Section 5.6 in Rationale explains why the message size limit is owned by SPEC-06. Requirement numbering has been adjusted: old R32-R39 are now R31-R37.
**Spec sections modified:** Section 2 (removed "Message Size Limit" definition), Section 3.5 (R29/R30 rewritten, R31 removed), Section 4.2 (removed `max_message_size` from `SecurityConfig`), Section 4.5 (no `--max-message-size` flag), Section 5.6 (new rationale subsection)

### SC-005: Missing base64 and subtle crate dependencies
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** R10 now explicitly states: "The `base64` crate (version 0.22) MUST be added to the always-on dependencies (extending SPEC-13 R11)." R15 now explicitly states: "The `subtle` crate (version 2.x) MUST be added to the always-on dependencies (extending SPEC-13 R11). Constant-time comparison MUST be implemented via `subtle::ConstantTimeEq`." Additionally, `PartialEq` and `Eq` have been removed from the `AuthToken` derive list (addressing SC-009 simultaneously). The spec notes that SPEC-13 R11's dependency table requires extension with these two crates (cannot edit SPEC-13 directly due to territory restriction).
**Spec sections modified:** Section 3.3 (R10, R15, R19 updated)

### SC-006: Zeroize mentioned but not specified or dependency-listed
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** The ambiguous parenthetical "use `Zeroize` or avoid including token in panic paths" has been removed. The approach is now definitive: token leakage is prevented by the redacted `Debug` implementation on `AuthToken` (which prints `AuthToken([REDACTED])`). R34 (formerly R36) now explicitly states the `Debug` implementation requirement. The `zeroize` crate is listed in the "What is NOT in v1" table (R37) as explicitly excluded with justification: "Token leakage is prevented by redacted Debug impl; zeroize MAY be added in v2 but is not required." This eliminates the ambiguity completely.
**Spec sections modified:** Section 3.7 (R34 rewritten), Section 3.8 (R37 table: added Zeroize row)

### SC-007: --token auto vs no --token ambiguity in tier detection
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** Tier detection is now strictly and exclusively flag-based. R1 adds: "Tier detection MUST be purely flag-based with no inference from bind address or other context." R3 has been rewritten with explicit bullet points and the clause "No other context (bind address, environment, etc.) MUST influence tier detection." R9 has been simplified: it now defines three cases (`--token auto`, `--token <base64>`, no `--token`) with no reference to "when token auth is required (Tier 2/3)," eliminating the circular dependency. R8 has been simplified from a SHOULD-refuse to a MUST-warn-and-proceed: "The coordinator MUST emit a WARN-level log message... The coordinator MUST proceed (no refusal)." The `--insecure` flag is retained as a coordinator CLI extension (Section 4.5) for suppressing the warning, but refusal is no longer the default behavior.
**Spec sections modified:** Section 2 (SecurityTier definition updated), Section 3.1 (R1, R3 rewritten), Section 3.2 (R8 rewritten), Section 3.3 (R9 rewritten), Section 4.5 (--insecure in CLI table)

### SC-008: SPEC-13 R44 omits --token for coordinator CLI
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** A new Section 4.5 ("CLI Extensions") explicitly documents that SPEC-10 extends SPEC-13 R44 and SPEC-07 R3 with additional CLI flags. The section provides a table of additional flags for both `CoordinatorArgs` (`--token`, `--token-file`, `--insecure`) and `WorkerArgs` (`--token`), with types, defaults, and descriptions. A note clarifies that TLS flags (`--tls-cert`, `--tls-key`, `--tls-ca`) are already defined in SPEC-13 R44-R45 and not repeated.
**Spec sections modified:** Section 4.5 (new)

### SC-009: AuthToken derives PartialEq but R15 requires constant-time comparison
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** `PartialEq` and `Eq` have been removed from the `AuthToken` derive list. The `AuthToken` struct now derives only `Clone`. A comment block in the type definition explicitly states: "AuthToken MUST NOT implement PartialEq or Eq. All comparisons MUST go through the verify() method, which uses constant-time comparison via subtle::ConstantTimeEq." A custom `Debug` implementation is provided that prints `AuthToken([REDACTED])`. T2 (test requirement) has been updated to use `verify()` instead of `==` for roundtrip comparison.
**Spec sections modified:** Section 3.3 (R19 type definition rewritten), Section 6 (T2 updated)

### SC-010: Token file mode 0600 is POSIX-only, no Windows equivalent
**Severity:** LOW
**Response:** ACCEPTED
**Action taken:** R12 now explicitly addresses cross-platform behavior: "with mode `0600` (owner read/write only) on Unix-like systems. On non-Unix platforms, the file SHOULD be created in the current working directory with the platform's default permissions, and the limitation MUST be documented. The implementation SHOULD use `#[cfg(unix)]` for the permission-setting call."
**Spec sections modified:** Section 3.3 (R12 rewritten)

### SC-011: TLS integration with ChannelTransport unspecified
**Severity:** LOW
**Response:** ACCEPTED
**Action taken:** Added R28a after R28: "TLS applies only to TcpTransport. ChannelTransport (SPEC-13, R31) does not support TLS and MUST NOT attempt TLS handshakes. Integration tests using ChannelTransport MAY run at Tier 1 or Tier 2 regardless of the tls feature flag. Tier 3 integration tests MUST use TcpTransport (localhost)."
**Spec sections modified:** Section 3.4 (R28a added)

### SC-012: R35 is a documentation requirement, not a testable security requirement
**Severity:** LOW
**Response:** PARTIALLY ACCEPTED
**Action taken:** The requirement has been moved from Section 3.7 (Defensive Measures) to Section 5.5 (Rationale) as a design note titled "Why Bincode Deserialization is a Security Advantage." The content has been preserved and expanded. The MUST requirement about documentation has been removed from the requirements section. However, this is classified as PARTIALLY ACCEPTED rather than fully ACCEPTED because the content was relocated and expanded rather than simply downgraded to SHOULD -- the information is valuable context that belongs in the Rationale section, not as a testable requirement.
**Spec sections modified:** Section 3.7 (old R35 removed), Section 5.5 (new rationale subsection)

### SC-013: T6 uses SHOULD but describes MUST behavior
**Severity:** LOW
**Response:** PARTIALLY ACCEPTED
**Action taken:** T6 has been rewritten to consistently use SHOULD throughout, matching R32 (the requirement it verifies, formerly R33): "Connection idle timeout SHOULD be tested: a connection that sends no messages for longer than `idle_timeout` SHOULD be closed by the coordinator." The internal MUST language has been softened to SHOULD. However, the fix differs slightly from the suggestion because the requirement R32 (formerly R33) itself was also softened: the internal "MUST be closed" within R32's description was changed to "SHOULD be closed" since the entire requirement is SHOULD-level.
**Spec sections modified:** Section 3.6 (R32 softened), Section 6 (T6 rewritten)

### SC-014: R38 magic bytes / protocol version unspecified
**Severity:** LOW
**Response:** PARTIALLY ACCEPTED
**Action taken:** Rather than removing R38 (now R36) entirely or specifying standalone magic bytes, the requirement has been concretized by integrating the protocol version into the `Register` message. The `RegisterPayload` struct (Section 4.3) includes a `protocol_version: u8` field (current version: 1). R36 now states: "The Register message defined in Section 4.3 includes a protocol_version: u8 field that serves this purpose. When the protocol version is unrecognized, the coordinator MUST close the connection immediately." This avoids changing the SPEC-06 frame format (no magic bytes before the length-prefix header) while achieving the same goal: fast rejection of non-Relativist connections. The approach differs from the suggestion because (a) it avoids the complexity of separate magic bytes and (b) it leverages the `Register` message that must already be the first message on every connection per the coordinator FSM.
**Spec sections modified:** Section 3.7 (R36 rewritten), Section 4.3 (protocol_version in RegisterPayload), Section 4.6 (lifecycle updated), Section 4.8 (connection flow step 6 added)

---

## Changes Made to SPEC-10

### Header
- Status changed from "Draft v1" to "Revised v2"

### Section 1 (Purpose)
- Updated to reference message size enforcement as "delegated to SPEC-06" instead of being owned by SPEC-10

### Section 2 (Definitions)
- Removed "Message Size Limit" definition (now fully owned by SPEC-06)
- Updated "Security Tier" definition to emphasize flag-based detection
- Removed reference to "implicit" selection

### Section 3.1 (Three-Tier Security Model)
- R1: Added "purely flag-based with no inference" clause
- R3: Rewritten with explicit bullet points and "No other context... MUST influence tier detection"

### Section 3.2 (Network Binding)
- R5: Added `127.0.0.1:9000` (matching SPEC-13 R44), added supersession note for SPEC-07
- R8: Changed from SHOULD-refuse to MUST-warn-and-proceed

### Section 3.3 (Token Authentication)
- R9: Removed circular "when token auth is required (Tier 2/3)" clause
- R10: Added `base64` crate dependency note
- R12: Added cross-platform behavior note (Unix vs non-Unix)
- R15: Added `subtle` crate dependency note, explicit `ConstantTimeEq` reference
- R19: Complete rewrite of `AuthToken` type:
  - Removed `PartialEq`, `Eq` from derives
  - Removed `Debug` from derives, replaced with custom impl printing `[REDACTED]`
  - Added MUST NOT comment for PartialEq
  - Reworded to reference new Section 4.3 for message definitions

### Section 3.4 (TLS Support)
- R28a: New requirement specifying ChannelTransport behavior under TLS

### Section 3.5 (Message Size Limits)
- R29: Rewritten to reference SPEC-06 R9 as canonical owner, not define its own config
- R30: Uses `ProtocolError::PayloadTooLarge` (SPEC-06 name), not `MessageTooLarge`
- Old R31 (enforcement details): Consolidated into R30
- Old R30 (SecurityConfig.max_message_size): Removed entirely

### Section 3.6 (Connection Limits)
- Renumbered from R32-R34 to R31-R33
- R32 (formerly R33): Internal "MUST be closed" softened to "SHOULD be closed"

### Section 3.7 (Defensive Measures)
- Old R35 (bincode documentation): Moved to Section 5.5 (Rationale)
- R34 (formerly R36): Rewritten to specify redacted Debug as the definitive approach, removed Zeroize ambiguity
- R36 (formerly R38): Rewritten to reference protocol_version in Register message
- Renumbered from R35-R38 to R34-R36

### Section 3.8 (What is NOT in v1)
- Renumbered from R39 to R37
- Added "Zeroize crate" row to exclusion table

### Section 4.1 (Security Module Structure)
- Added `error.rs` to module tree

### Section 4.2 (Security Configuration)
- Removed `max_message_size: u32` field from `SecurityConfig`
- Removed `--max-message-size` mention

### Section 4.3 (Registration Messages) -- NEW
- Full definitions of `RegisterPayload`, `RegisterAckPayload`, `RegisterNackPayload`
- Exact syntax for Message enum extension variants
- Note about total 7 variants (4 from SPEC-06 + 3 from SPEC-10)

### Section 4.4 (Error Types) -- NEW
- Full `TokenError` enum definition
- Full `SecurityError` enum definition
- Note about `RelativistError` extension
- Explanation of `SecurityError::AuthFailed` vs `ProtocolError::AuthFailed`

### Section 4.5 (CLI Extensions) -- NEW
- Table of additional coordinator CLI flags
- Table of additional worker CLI flags
- Notes about flag naming (--bind not --host) and TLS flag references

### Section 4.6 (Token Lifecycle) -- RENAMED (was 4.3)
- Updated to reference `RegisterPayload` struct
- Updated to include protocol_version check

### Section 4.7 (TLS Integration) -- RENAMED (was 4.4)
- No content changes

### Section 4.8 (Connection Acceptance Flow) -- RENAMED (was 4.5)
- Added step 6: protocol_version check
- Updated step 8 to reference `RegisterAckPayload`

### Section 5 (Rationale)
- Section 5.5 (new): "Why Bincode Deserialization is a Security Advantage" (moved from R35)
- Section 5.6 (new): "Why Message Size Limit is Owned by SPEC-06"

### Section 6 (Test Requirements)
- T2: Updated to use `verify()` instead of `==` for roundtrip test
- T5: Updated to reference `max_payload_size` (SPEC-06 R9) instead of `max_message_size`
- T6: Softened internal language from MUST to SHOULD
- T10 (new): Test for redacted Debug output on AuthToken

---

## Residual Risks

No issues were NOT ADDRESSED. All 14 issues from the critic review have been either ACCEPTED (10) or PARTIALLY ACCEPTED (4) with concrete fixes applied to the spec.

The following cross-spec inconsistencies are documented in SPEC-10 but cannot be fixed by SPEC-10 alone (territory restriction):

1. **SPEC-06 Message enum:** SPEC-06 Section 4.1 defines 4 variants. SPEC-10 Section 4.3 defines 3 additional variants that must be added. SPEC-06 will need revision to incorporate these. SPEC-10 provides the complete type definitions to minimize ambiguity.

2. **SPEC-07 R3/R12 bind address default:** SPEC-07 uses `--host 0.0.0.0` as default. SPEC-10 R5 supersedes this with `--bind 127.0.0.1:9000`. SPEC-07 will need revision to align. The supersession is clearly documented.

3. **SPEC-13 R11 dependency table:** SPEC-13's always-on dependencies do not include `base64` or `subtle`. SPEC-10 R10 and R15 note that these must be added. SPEC-13 will need revision.

4. **SPEC-13 R17 RelativistError:** Does not include a `Security(#[from] SecurityError)` variant. SPEC-10 Section 4.4 notes this extension requirement. SPEC-13 will need revision.

5. **ProtocolError variant naming:** SPEC-06 uses `PayloadTooLarge`, SPEC-13 uses `MessageTooLarge`. SPEC-10 now consistently uses `PayloadTooLarge` (matching SPEC-06 as the canonical wire protocol spec). SPEC-13 R16's `ProtocolError` definition will need revision to align.

These are tracked as cross-spec consistency items for the next revision cycle of SPEC-06, SPEC-07, and SPEC-13.
