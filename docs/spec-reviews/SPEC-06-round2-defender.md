# SPEC-06 -- Round 2: Defender Response

**Date:** 2026-04-05
**Defender:** Especialista em Specs
**Target:** SPEC-06-wire-protocol.md
**Critic review:** SPEC-06-round1-critic.md
**Spec version:** Revised v2 -> Revised v3

---

## Summary

| Response | Count |
|----------|-------|
| ACCEPTED | 12 |
| PARTIALLY ACCEPTED | 5 |
| NOT ADDRESSED | 0 |
| **Total issues** | **17** |

---

## Responses

### SC-001: Message enum is missing Register/RegisterAck/RegisterNack variants
**Severity:** CRITICAL
**Response:** ACCEPTED
**Action taken:** Extended the `Message` enum in Section 4.1 to include all 7 variants. The three registration variants (`Register`, `RegisterAck`, `RegisterNack`) are documented with their payload structures as defined by SPEC-10 Section 4.3. A note clarifies that SPEC-10 is the canonical owner of these variants' semantics and that SPEC-06 includes them for completeness of the enum definition.

Regarding the contradiction between SPEC-10 (explicit Register messages) and SPEC-13 R21 note (implicit registration): the resolution is tier-dependent. In Tier 1 (no auth), the coordinator MAY accept connections implicitly without requiring a Register message -- this is the behavior described by SPEC-13 R21's note. In Tier 2/3 (auth enabled), the Register/RegisterAck/RegisterNack handshake is mandatory per SPEC-10 R14-R17. This distinction is documented in the new R2a requirement and in the Message enum notes.

Updated R2 and R3 to include the registration variants. Added R2a to clarify the tier-dependent registration behavior.
**Spec sections modified:** Section 3.1 (R2, R3, added R2a), Section 4.1 (Message enum)

### SC-002: FSM states (R26-R28) are superseded by SPEC-13 R19-R22 but not marked as such
**Severity:** CRITICAL
**Response:** ACCEPTED
**Action taken:** Added a prominent supersession note at the beginning of Section 3.6 stating that R26-R28 have been superseded by SPEC-13 R19-R25. The note includes a complete state name mapping between SPEC-06 and SPEC-13. R26, R27, and R28 are demoted from normative MUST to historical documentation, retained only for traceability. The original FSM diagrams and pseudocode in Sections 4.6 and 4.7 are annotated with a note that SPEC-13 state names are authoritative.

The contradition between R28 ("MAY be implemented implicitly via control flow") and SPEC-13 R22 ("MUST be enum-based") is resolved by the supersession: SPEC-13 R22 is authoritative.
**Spec sections modified:** Section 3.6 (R26, R27, R28 -- added supersession notes and demoted to historical), Section 4.6 (coordinator pseudocode annotation), Section 4.7 (worker pseudocode annotation)

### SC-003: ProtocolError::PayloadTooLarge vs SPEC-13 ProtocolError::MessageTooLarge naming conflict
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** Standardized on `PayloadTooLarge` as the canonical name since SPEC-06 is the wire protocol owner and the error specifically refers to the payload portion of a frame (header is always 8 bytes and is not subject to size limits). Retained `u32` field types since the `length` field in the frame header is `u32`. Updated the SPEC-06 ProtocolError definition to be declared as canonical with a note that SPEC-13 R16's `MessageTooLarge` variant should reference SPEC-06's definition during SPEC-13's next revision cycle.

Renamed field `declared` to `size` for clarity (the value is the declared size from the frame header), aligning with SPEC-13's field naming convention while keeping the `u32` type consistent with the frame header.
**Spec sections modified:** Section 4.4 (ProtocolError enum -- `PayloadTooLarge` field renamed `declared` -> `size`, added canonicality note)

### SC-004: ProtocolError enum structure differs significantly between SPEC-06 and SPEC-13
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** SPEC-06 is now declared as the canonical owner of `ProtocolError` with all 10 variants (the original 8 plus `AuthFailed` from SPEC-10 and `ConnectionLost` replacing `Io`). The reconciliation:

1. `Io(std::io::Error)` renamed to `ConnectionLost(std::io::Error)` to match SPEC-13 naming (more descriptive).
2. `PayloadTooLarge` retained with `u32` fields (canonical, per SC-003).
3. `ChecksumMismatch` retains structured fields `{ expected: u32, computed: u32 }` for diagnostics (richer than SPEC-13's fieldless variant).
4. `Deserialize` and `Serialize` retained (SPEC-13 omitted them but they are needed for the framing layer).
5. `UnexpectedMessage` retained with structured fields (more informative than SPEC-13's `InvalidMessage(String)`).
6. `Timeout` retains structured fields `{ phase, elapsed }` (more informative than SPEC-13's `Timeout(Duration)`).
7. `WorkerError` moved to `CoordinatorError` (consistent with SPEC-13 R16 -- it is a coordinator-level concern, not a protocol-level concern).
8. `WorkerCountMismatch` moved to `CoordinatorError` (same rationale).
9. `AuthFailed` added from SPEC-10.

A note references SPEC-13 R16's statement that "individual variants MAY be added or renamed during implementation" and declares SPEC-06 Section 4.4 as the canonical detailed specification.
**Spec sections modified:** Section 4.4 (ProtocolError enum -- restructured, added AuthFailed, moved WorkerError and WorkerCountMismatch to a separate note, renamed Io to ConnectionLost, added canonicality note)

### SC-005: NodeConfig structure is incompatible with SPEC-13 CLI design
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** Replaced `host: String` + `port: u16` with `bind: SocketAddr` in `NodeConfig`. Added default value `127.0.0.1:9000` consistent with SPEC-10 R5 and SPEC-13 R44. Added a note that SPEC-13 R44-R45 defines separate `CoordinatorArgs` and `WorkerArgs` for the CLI layer; `NodeConfig` is an internal configuration struct used after CLI parsing, not a CLI struct itself. Removed the `role: NodeRole` field and `NodeRole` enum since SPEC-13's architecture uses separate `CoordinatorArgs`/`WorkerArgs` structs, making the role discriminant unnecessary -- the coordinator and worker binaries (or subcommands) know their role from the CLI subcommand.

Updated `connect_with_retry` pseudocode (Section 4.8) to accept `SocketAddr` instead of separate host/port.
**Spec sections modified:** Section 3.9 (R36, R37), Section 4.5 (NodeConfig -- replaced host+port with bind SocketAddr, removed NodeRole, added defaults), Section 4.6 (coordinator pseudocode updated), Section 4.8 (connect_with_retry updated)

### SC-006: WorkerRoundStats in SPEC-06 pseudocode is outdated relative to SPEC-11 extension
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** Updated the worker pseudocode in Section 4.7 to construct the full 6-field `WorkerRoundStats` as defined by SPEC-11 Section 4.4, including `reduce_duration_secs` and `interactions_by_rule`. Added timing instrumentation around `reduce_all` and noted that `interactions_by_rule` requires the reduction engine to return per-rule counts (cross-reference to SPEC-03). Added a note to R12 that the SPEC-11 extended definition is normative for the complete `WorkerRoundStats` type.
**Spec sections modified:** Section 3.3 (R12 -- added note about SPEC-11 extension), Section 4.7 (worker pseudocode updated to 6-field WorkerRoundStats)

### SC-007: No worker-to-coordinator registration handshake in the protocol flow
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** Updated the coordinator pseudocode Phase 0 in Section 4.6 to include the registration handshake. The handshake is tier-dependent: in Tier 1 (no auth), the coordinator accepts TCP connections and optionally processes a Register message (if the worker sends one); in Tier 2/3, the Register/RegisterAck/RegisterNack handshake is mandatory. Added a note referencing SPEC-10 for the full authentication flow and R2a for the tier-dependent behavior.
**Spec sections modified:** Section 4.6 (coordinator pseudocode Phase 0 -- added registration handshake), Section 4.11 (sequence diagram -- added Register/RegisterAck)

### SC-008: R17 uses "configurable port" but specifies no default
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** Updated R17 to specify the default bind address as `127.0.0.1:9000`, consistent with SPEC-10 R5 and SPEC-13 R44. The default is also reflected in the `NodeConfig` definition (Section 4.5).
**Spec sections modified:** Section 3.4 (R17 -- added default bind address)

### SC-009: R12 lists types for serde derives but omits Registration types from SPEC-10
**Severity:** MEDIUM
**Response:** PARTIALLY ACCEPTED
**Action taken:** Rather than maintaining an exhaustive list that must be manually updated every time a new type is added, R12 is rephrased to: "All types transitively reachable from `Message` MUST derive `serde::Serialize` and `serde::Deserialize`." A non-exhaustive list is provided for guidance. This approach is more maintainable and avoids the staleness issue.

The fix differs from the critic's suggestion in that we do not add an exhaustive list of Registration types. The "transitively reachable" phrasing covers all current and future payload types automatically, including `RegisterPayload`, `RegisterAckPayload`, and `RegisterNackPayload`.
**Spec sections modified:** Section 3.3 (R12 -- rephrased to transitive reachability)

### SC-010: R25 abort-on-connection-loss is normatively correct but has no recovery mechanism
**Severity:** MEDIUM
**Response:** PARTIALLY ACCEPTED
**Action taken:** Added a clarifying note to R25 specifying that on connection loss: (a) the coordinator transitions to the Error state (SPEC-13 R21), (b) the coordinator sends Shutdown to all remaining connected workers before aborting (best-effort, errors ignored), and (c) the error is propagated as `ProtocolError::ConnectionLost` wrapping the underlying I/O error.

The fix does not specify whether partial results are available because this is an implementation concern: the coordinator's error handling may or may not retain partial state. The TCC scope explicitly excludes fault tolerance (Z5), so partial result recovery is out of scope.
**Spec sections modified:** Section 3.5 (R25 -- added cleanup behavior note)

### SC-011: Coordinator pseudocode Phase 2b collects results sequentially, contradicting R22 MAY for concurrent
**Severity:** LOW
**Response:** PARTIALLY ACCEPTED
**Action taken:** Added a comment in the pseudocode at Phase 2b: "// Note: concurrent collection via FuturesUnordered is permitted by R22 and may improve performance." No structural change to the pseudocode itself, as R22 already documents the MAY and Open Question 3 acknowledges this as an implementation decision. The sequential pseudocode is simpler and correct; the concurrent option is the implementer's discretion.
**Spec sections modified:** Section 4.6 (Phase 2b -- added inline comment)

### SC-012: R9 maximum payload default (256 MiB) has no justification
**Severity:** LOW
**Response:** ACCEPTED
**Action taken:** Added a rationale note to R9: "The 256 MiB default accommodates nets up to approximately 8M agents per partition (at ~32 bytes per agent+ports). For the benchmark suite (SPEC-09), typical partition sizes are under 10 MB. The generous default prevents false rejections for exploratory workloads beyond the benchmark suite while still providing defense against unbounded allocation."
**Spec sections modified:** Section 3.2 (R9 -- added rationale note)

### SC-013: R5 extensibility SHOULD conflicts with bincode's default behavior
**Severity:** LOW
**Response:** ACCEPTED
**Action taken:** Added a note to R5: "New variants MUST be appended at the end of the enum to preserve bincode discriminant stability. Inserting variants in the middle changes the discriminants of subsequent variants, breaking backward compatibility with any previously serialized messages."
**Spec sections modified:** Section 3.1 (R5 -- added append-only note)

### SC-014: R30 timeout values are SHOULD but R31 abort on timeout is MUST
**Severity:** LOW
**Response:** PARTIALLY ACCEPTED
**Action taken:** Upgraded `collect_timeout` in R30 from SHOULD to MUST, since stuck workers are the primary failure mode in the failure-free scenario (e.g., a worker that deadlocks internally). `distribute_timeout` remains SHOULD because send operations are bounded by TCP buffer sizes and are unlikely to hang indefinitely.

The fix differs from the critic's suggestion in that `distribute_timeout` is kept as SHOULD rather than being upgraded. The critical path is collection, not distribution.
**Spec sections modified:** Section 3.7 (R30 -- upgraded collect_timeout to MUST)

### SC-015: send_frame return type does not include bytes count for shutdown
**Severity:** LOW
**Response:** PARTIALLY ACCEPTED
**Action taken:** Added a note in the shutdown section (Section 4.12) clarifying that shutdown message bytes are not included in per-round metrics since they occur outside the grid loop. This is the only change needed; the `let _ =` pattern in the pseudocode is intentional for shutdown (best-effort, errors ignored).
**Spec sections modified:** Section 4.12 (added clarifying note about shutdown metrics)

### SC-016: No specification of byte order for PortRef tag in bincode
**Severity:** LOW
**Response:** ACCEPTED
**Action taken:** Updated R11 to explicitly specify the bincode configuration: "bincode with configuration equivalent to `bincode::config::standard().with_little_endian().with_fixed_int_encoding()`." Added a note that bincode v2.x default configuration uses variable-length encoding, which differs from v1 defaults, so explicit configuration is required. Updated the note in Section 4.9 to reference this explicit configuration.
**Spec sections modified:** Section 3.3 (R11 -- explicit bincode configuration), Section 4.9 (updated note)

### SC-017: SPEC-06 does not reference SPEC-13's Transport trait abstraction
**Severity:** LOW
**Response:** ACCEPTED
**Action taken:** Added a note in Section 4.3 clarifying that `send_frame` and `recv_frame` are internal implementation details of `TcpTransport` (SPEC-13 R29). External callers use the `Transport` trait (SPEC-13 R28) which abstracts over TCP and in-memory channels. The framing functions are the low-level building blocks; the `Transport` trait is the public API.
**Spec sections modified:** Section 4.3 (added Transport trait reference note)

---

## Changes Made to SPEC-06

### Header
- Status changed from "Revised v2" to "Revised v3"

### Section 3.1 (Protocol Messages)
- R2: Added `Register`, `RegisterAck`, `RegisterNack` to coordinator-to-worker / worker-to-coordinator message lists with tier dependency notes
- R2a: New requirement documenting tier-dependent registration behavior (Tier 1 implicit, Tier 2/3 explicit handshake per SPEC-10)
- R3: Added `Register` to worker-to-coordinator messages
- R5: Added append-only enum discipline note for bincode discriminant stability

### Section 3.2 (Framing Format)
- R9: Added rationale note for 256 MiB default

### Section 3.3 (Serialization)
- R11: Explicit bincode configuration specified (standard + little-endian + fixed-int)
- R12: Rephrased from exhaustive list to "transitively reachable from Message" with non-exhaustive guidance list and SPEC-11 extension note

### Section 3.4 (TCP Transport)
- R17: Added default bind address `127.0.0.1:9000`

### Section 3.5 (Connection and Retry)
- R25: Added cleanup behavior on connection loss (Error state, Shutdown to remaining workers, ConnectionLost error)

### Section 3.6 (Finite State Machines)
- Added supersession note at top of section
- R26: Demoted from normative MUST to historical documentation with state name mapping
- R27: Demoted from normative MUST to historical documentation with state name mapping
- R28: Demoted from normative MUST to historical documentation; superseded by SPEC-13 R22

### Section 3.7 (Integrity and Timeouts)
- R30: `collect_timeout` upgraded from SHOULD to MUST

### Section 3.9 (Configuration)
- R36: `NodeConfig` replaced host+port with `bind: SocketAddr`, removed `NodeRole`, added defaults
- R37: No change (CLI via clap)

### Section 4.1 (Message Catalog)
- Extended `Message` enum from 4 to 7 variants (added Register, RegisterAck, RegisterNack)
- Added registration payload struct definitions from SPEC-10 Section 4.3
- Added note on tier-dependent registration and canonical ownership

### Section 4.3 (Framing Functions)
- Added note about Transport trait abstraction (SPEC-13 R28-R29)

### Section 4.4 (Protocol Error Types)
- `Io` renamed to `ConnectionLost`
- `PayloadTooLarge.declared` renamed to `size`
- `AuthFailed` added from SPEC-10
- `WorkerError` removed (moved to CoordinatorError per SPEC-13 R16)
- `WorkerCountMismatch` removed (moved to CoordinatorError per SPEC-13 R16)
- Added canonicality note declaring SPEC-06 Section 4.4 as the authoritative ProtocolError definition

### Section 4.5 (Node Configuration)
- Removed `NodeRole` enum
- `NodeConfig`: replaced `host: String` + `port: u16` with `bind: SocketAddr`, removed `role` field, added default values

### Section 4.6 (Coordinator FSM)
- Phase 0: Added registration handshake (Register/RegisterAck/RegisterNack) with tier-dependent note
- Phase 2b: Added inline comment about concurrent collection per R22
- Added annotation noting SPEC-13 state names are authoritative

### Section 4.7 (Worker FSM)
- Updated `WorkerRoundStats` construction to 6 fields (added `reduce_duration_secs`, `interactions_by_rule`)
- Added timing instrumentation around `reduce_all`
- Added annotation noting SPEC-13 state names are authoritative

### Section 4.8 (Connect with Retry)
- Updated to use `SocketAddr` instead of separate host/port

### Section 4.9 (Serialized Message Size Estimates)
- Updated note to reference explicit bincode configuration

### Section 4.11 (Round Sequence Diagram)
- Added Register/RegisterAck in the initial connection phase

### Section 4.12 (Shutdown Protocol)
- Added note clarifying shutdown bytes are not included in per-round metrics
