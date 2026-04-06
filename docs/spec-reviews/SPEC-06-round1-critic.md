# SPEC-06 -- Round 1: Spec Critic Review

**Date:** 2026-04-05
**Reviewer:** Spec Critic (adversarial)
**Target:** SPEC-06-wire-protocol.md (status: Revised v2)
**Predecessors consulted:** SPEC-00, SPEC-01, SPEC-02, SPEC-05
**Successors consulted:** SPEC-10 (Security, Revised v2), SPEC-11 (Observability, Revised v2), SPEC-13 (System Architecture, Revised v2)
**Backlog consulted:** BACKLOG.md (Phase 5: Wire Protocol, TASK-0080 through TASK-0096)

---

## Overall Assessment

SPEC-06 is a well-structured wire protocol spec that covers framing, serialization, FSMs, metrics, and configuration. However, its Revised v2 status is misleading: multiple successor specs (SPEC-10, SPEC-11, SPEC-13) have been revised to v2 and introduce requirements that directly contradict or supersede SPEC-06, yet SPEC-06 has not been updated to reflect these changes. The result is a spec that is internally consistent but externally stale -- the `Message` enum is incomplete, the FSM states are superseded, the error types conflict, the `NodeConfig` structure is incompatible with the CLI design, and the `WorkerRoundStats` type is out of date. An implementer following SPEC-06 alone will produce code that is incompatible with SPEC-10, SPEC-11, and SPEC-13.

**Verdict:** NEEDS REVISION

---

## Issues

### SC-001: Message enum is missing Register/RegisterAck/RegisterNack variants
**Severity:** CRITICAL
**Axis:** Consistency
**Section:** 3.1 (R1-R3), Section 4.1
**Requirements affected:** R1, R2, R3
**Problem:** SPEC-06 Section 4.1 defines the `Message` enum with exactly 4 variants: `AssignPartition`, `Shutdown`, `PartitionResult`, `Error`. However, SPEC-10 R19 (Revised v2) explicitly mandates:

> "The `Message` enum (SPEC-06 Section 4.1) MUST be extended with `Register`, `RegisterAck`, and `RegisterNack` variants for worker registration and authentication."

SPEC-10 Section 4.3 provides full struct definitions (`RegisterPayload`, `RegisterAckPayload`, `RegisterNackPayload`) and states: "The complete `Message` enum has seven variants in total."

SPEC-13 Revised v2 R21 note already acknowledges this by saying "Worker registration is implicit upon TCP connection acceptance, consistent with SPEC-06 R24." But SPEC-13 Revised v2 R21 note also says there are no explicit Register/RegisterAck messages -- which now contradicts SPEC-10 Revised v2 R14 ("Workers MUST include the token in the `Register` message") and R17 ("The coordinator MUST send a `RegisterAck` message").

So SPEC-13 Revised v2 says registration is implicit (no Register message), while SPEC-10 Revised v2 says registration requires an explicit Register message with an auth token. SPEC-06 is caught in the middle, defining neither.

**Impact if unresolved:** The implementer cannot build TASK-0082 (Define Message enum) or TASK-0127 (Extend Message enum with Register variants) because the canonical definition of Message is ambiguous. SPEC-06 says 4 variants, SPEC-10 says 7, and SPEC-13's FSM notes say registration is implicit (i.e., 4 variants are enough). Three specs give three different answers.

**Suggested resolution:** SPEC-06 MUST be updated to include all 7 Message variants. The `Register`, `RegisterAck`, and `RegisterNack` definitions from SPEC-10 Section 4.3 should be incorporated into SPEC-06 Section 4.1. SPEC-13 R21's note about implicit registration must be updated to reference the SPEC-10 registration handshake. Add a note that `Register`/`RegisterAck`/`RegisterNack` are defined by SPEC-10 and documented here for completeness of the enum.

---

### SC-002: FSM states (R26-R28) are superseded by SPEC-13 R19-R22 but not marked as such
**Severity:** CRITICAL
**Axis:** Consistency
**Section:** 3.6 (R26, R27, R28)
**Requirements affected:** R26, R27, R28
**Problem:** SPEC-13 Revised v2 R19 contains an explicit supersession note:

> "SPEC-13 R19-R22 supersede SPEC-06 R26-R28 for the coordinator FSM definition. SPEC-06's FSM was a behavioral specification using informal state names; SPEC-13 provides the concrete enum-based FSM that the implementation MUST use."

However, SPEC-06 itself still presents R26-R28 as normative MUST requirements with no indication that they have been superseded. The state names differ significantly:

| SPEC-06 R26 (Coordinator) | SPEC-13 R19 (Coordinator) | Status |
|---|---|---|
| `WaitingWorkers` | `WaitingForWorkers` | Renamed |
| `Idle` | *(removed)* | Replaced by `CheckTermination` |
| `Partitioning` | `Partitioning` | Same |
| `Distributing` | `Dispatching` | Renamed |
| `WaitingResults` | `WaitingForResults` | Renamed |
| `Merging` | `Merging` | Same |
| `ShuttingDown` | *(removed)* | Subsumed by `Done` + `ShutdownAll` action |
| `Done` | `Done` | Same |
| *(none)* | `Init` | New |
| *(none)* | `CheckTermination` | New |
| *(none)* | `Error` | New |

| SPEC-06 R27 (Worker) | SPEC-13 R24 (Worker) | Status |
|---|---|---|
| `Connecting` | `Init` | Renamed |
| `Idle` | `Idle` | Same |
| `Reducing` | `Reducing` | Same |
| `Sending` | `Returning` | Renamed |
| `Done` | `Done` | Same |
| *(none)* | `Error` | New |

Additionally, R28 says the FSM "MAY be implemented implicitly via control flow," while SPEC-13 R22 says it "MUST be enum-based." These are contradictory normative requirements.

**Impact if unresolved:** The implementer reading SPEC-06 will build against the wrong FSM states. TASK-0107 (Define CoordinatorState enum) and TASK-0109 (Define WorkerState enum) reference the SPEC-13 states, but TASK-0088/0089/0090 (Phase 5 coordinator logic) reference SPEC-06 pseudocode that uses the old state names. The pseudocode in Sections 4.6 and 4.7 becomes misleading.

**Suggested resolution:** Add a prominent note at the beginning of Section 3.6: "The FSM definitions in R26-R28 have been superseded by SPEC-13 R19-R25. The state names and transition tables in SPEC-13 are authoritative. R26-R28 are retained for historical context only." Alternatively, update R26-R28 to match SPEC-13 exactly.

---

### SC-003: ProtocolError::PayloadTooLarge vs SPEC-13 ProtocolError::MessageTooLarge naming conflict
**Severity:** HIGH
**Axis:** Consistency
**Section:** Section 4.4 (ProtocolError enum)
**Requirements affected:** R9 (references PayloadTooLarge behavior)
**Problem:** SPEC-06 Section 4.4 defines the error variant:

```rust
PayloadTooLarge {
    declared: u32,
    max: u32,
}
```

SPEC-13 R16 defines the same error concept with a different name and different field types:

```rust
MessageTooLarge { size: usize, max: usize },
```

SPEC-10 R30 explicitly references SPEC-06's naming: "the receiver MUST reject it with a `ProtocolError::PayloadTooLarge` error (SPEC-06 Section 4.4)."

The differences are:
1. **Name:** `PayloadTooLarge` (SPEC-06, SPEC-10) vs `MessageTooLarge` (SPEC-13)
2. **Field names:** `declared`/`max` (SPEC-06) vs `size`/`max` (SPEC-13)
3. **Field types:** `u32` (SPEC-06) vs `usize` (SPEC-13)

**Impact if unresolved:** The implementer must choose one variant name and field signature. TASK-0081 (Define ProtocolError enum) will be ambiguous. If SPEC-06's naming is chosen, SPEC-13's error definitions are wrong. If SPEC-13's naming is chosen, SPEC-10's explicit reference to `PayloadTooLarge` is broken.

**Suggested resolution:** Standardize on one name. Since SPEC-06 is the canonical owner of the wire protocol error types, `PayloadTooLarge` should be authoritative. SPEC-13 R16 should be updated to match. The field type should be `u32` (consistent with `length` in the frame header being `u32`), or `usize` with a documented conversion. Either way, SPEC-06 and SPEC-13 MUST agree.

---

### SC-004: ProtocolError enum structure differs significantly between SPEC-06 and SPEC-13
**Severity:** HIGH
**Axis:** Consistency
**Section:** Section 4.4
**Requirements affected:** (design section, but implementer must choose one)
**Problem:** Beyond the PayloadTooLarge/MessageTooLarge issue (SC-003), the entire ProtocolError enum differs between the two specs:

| SPEC-06 Section 4.4 | SPEC-13 R16 |
|---|---|
| `Io(std::io::Error)` | `ConnectionLost(#[source] std::io::Error)` |
| `PayloadTooLarge { declared: u32, max: u32 }` | `MessageTooLarge { size: usize, max: usize }` |
| `ChecksumMismatch { expected: u32, computed: u32 }` | `ChecksumMismatch` (no fields) |
| `Deserialize(bincode::Error)` | *(missing)* |
| `Serialize(bincode::Error)` | *(missing)* |
| `UnexpectedMessage { expected, received }` | `InvalidMessage(String)` |
| `Timeout { phase, elapsed }` | `Timeout(std::time::Duration)` |
| `WorkerError { worker_id, round, description }` | *(in CoordinatorError, not ProtocolError)* |
| `WorkerCountMismatch { expected, connected }` | *(missing)* |
| *(missing)* | `AuthFailed` |

SPEC-06 has 8 variants with rich structured fields. SPEC-13 has 5 variants with minimal fields. The two are fundamentally incompatible.

**Impact if unresolved:** TASK-0081 (Define ProtocolError enum) cannot proceed without reconciliation. The rich SPEC-06 variants are more useful for diagnostics but SPEC-13 moved some concerns (WorkerError) to CoordinatorError and added AuthFailed (from SPEC-10). The implementer needs one canonical enum.

**Suggested resolution:** SPEC-13 R16 note already says "individual variants MAY be added or renamed during implementation." SPEC-06 should be treated as the canonical detailed specification of ProtocolError since it is the wire protocol owner. SPEC-13 R16 should reference SPEC-06 Section 4.4 for the canonical ProtocolError definition and only add variants not already covered (e.g., `AuthFailed`). SPEC-06 should add `AuthFailed` from SPEC-10.

---

### SC-005: NodeConfig structure is incompatible with SPEC-13 CLI design
**Severity:** HIGH
**Axis:** Consistency
**Section:** 3.9 (R36, R37), Section 4.5
**Requirements affected:** R36, R37
**Problem:** SPEC-06 R36 defines `NodeConfig` with:
- `role: NodeRole` (Coordinator or Worker)
- `host: String` -- address for bind/connect
- `port: u16` -- TCP port
- `num_workers: u32`

SPEC-13 R44 specifies the coordinator CLI as `--bind` (a socket address like `127.0.0.1:9000`), which is a `SocketAddr` combining host and port. SPEC-10 R5 specifies the default bind address as `127.0.0.1:9000` and uses `--bind` for override. SPEC-06's separate `host: String` + `port: u16` is inconsistent with the `--bind <SocketAddr>` pattern adopted by SPEC-10 and SPEC-13.

Additionally, SPEC-06 does not specify a default bind address. SPEC-10 R5 and SPEC-13 R44 both specify `127.0.0.1:9000`. SPEC-06 R17 says "configurable port" but provides no default.

Furthermore, SPEC-06's `NodeConfig` conflates coordinator and worker config into one struct (`role` field determines which fields are relevant), while SPEC-13 R44-R45 uses separate `CoordinatorArgs` and `WorkerArgs` structs. This is a design-level incompatibility.

**Impact if unresolved:** TASK-0084 (Define NodeConfig and NodeRole types) will produce a struct that does not match SPEC-13's CLI args or SPEC-10's binding defaults.

**Suggested resolution:** SPEC-06 should update `NodeConfig` to use `bind: SocketAddr` (or separate `bind_addr: IpAddr` + `bind_port: u16`) with default `127.0.0.1:9000`, consistent with SPEC-10 R5 and SPEC-13 R44. Alternatively, SPEC-06 should note that `NodeConfig` is a design suggestion and the canonical CLI configuration is defined in SPEC-13 R44-R45. Either way, the default port must be specified.

---

### SC-006: WorkerRoundStats in SPEC-06 pseudocode is outdated relative to SPEC-11 extension
**Severity:** MEDIUM
**Axis:** Consistency
**Section:** Section 4.7 (worker pseudocode, line 644-649)
**Requirements affected:** R12 (types that MUST derive Serialize/Deserialize)
**Problem:** SPEC-06 Section 4.7 constructs `WorkerRoundStats` with 4 fields:
```
let stats = WorkerRoundStats {
    worker_id: partition.worker_id,
    agents_before,
    agents_after,
    local_redexes: interactions,
}
```

SPEC-11 Section 4.4 extends `WorkerRoundStats` with 2 additional fields:
- `reduce_duration_secs: f64`
- `interactions_by_rule: [u64; 6]`

SPEC-11 OQ-1 explicitly notes: "This extension SHOULD be reflected in SPEC-05 R37 and SPEC-06 R12 during their next revision cycle." SPEC-06 R12 lists `WorkerRoundStats` as a type that MUST derive `Serialize`/`Deserialize`, but the type definition is not in SPEC-06 -- it's in SPEC-05 R37 (canonical) with SPEC-11 extension. SPEC-06's pseudocode constructs the old 4-field version.

**Impact if unresolved:** TASK-0152 (Extend WorkerRoundStats with observability fields) and TASK-0093 (Implement run_worker) will need the 6-field version, but the SPEC-06 pseudocode shows the 4-field version. Minor confusion but worth fixing for consistency.

**Suggested resolution:** Update SPEC-06 Section 4.7 pseudocode to construct the full 6-field `WorkerRoundStats` as defined by SPEC-11 Section 4.4, or add a note that the pseudocode shows the SPEC-05 R37 canonical fields and SPEC-11 adds additional fields.

---

### SC-007: No worker-to-coordinator registration handshake in the protocol flow
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 3.4 (R16-R19), Section 4.6 (coordinator pseudocode)
**Requirements affected:** R16, R17, R18, R24
**Problem:** SPEC-06's coordinator pseudocode (Section 4.6) shows Phase 0 as:
```
while worker_streams.len() < config.num_workers as usize:
    let (stream, addr) = listener.accept().await?
    worker_streams.push(stream)
```

This is raw TCP accept with no handshake. But SPEC-10 R14 requires: "Workers MUST include the token in the `Register` message sent to the coordinator upon connection." SPEC-10 R17 requires the coordinator to send `RegisterAck` on success. SPEC-13 Revised v2 R25 worker FSM shows `Init -> Connected -> Idle` with implicit registration.

The flow should be:
1. Worker connects (TCP accept)
2. Worker sends `Register(RegisterPayload)` with optional auth token
3. Coordinator validates token (if configured)
4. Coordinator sends `RegisterAck(RegisterAckPayload)` with assigned WorkerId, OR `RegisterNack` + close
5. Worker enters Idle state

None of this is in SPEC-06. The coordinator pseudocode accepts raw TCP connections and immediately starts the grid loop without any handshake.

**Impact if unresolved:** TASK-0088 (Implement coordinator worker-accept phase) must implement the registration handshake per SPEC-10, but the SPEC-06 pseudocode shows no handshake. The implementer will need to reconcile.

**Suggested resolution:** Update the coordinator pseudocode Phase 0 to include the registration handshake. At minimum, add a note: "The accept phase shown here is simplified. When authentication is enabled (SPEC-10), each accepted connection must complete a Register/RegisterAck handshake before being added to the worker pool."

---

### SC-008: R17 uses "configurable port" but specifies no default, conflicting with SPEC-10/SPEC-13 defaults
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 3.4 (R17)
**Requirements affected:** R17
**Problem:** R17 says: "The coordinator MUST open a TCP listener socket on a configurable port and wait for connections from workers." No default port is specified. SPEC-10 R5 says the default is `127.0.0.1:9000`. SPEC-13 R44 says the default `--bind` is `127.0.0.1:9000`. SPEC-06 NodeConfig (Section 4.5) defines `port: u16` but provides no default value.

**Impact if unresolved:** SPEC-06 is incomplete as a standalone spec for the wire protocol. The implementer must cross-reference SPEC-10 and SPEC-13 for the default port, which is `9000`.

**Suggested resolution:** Add `Default: 9000` to R17 or to the `NodeConfig` definition in Section 4.5.

---

### SC-009: R12 lists types for serde derives but omits Registration types from SPEC-10
**Severity:** MEDIUM
**Axis:** Consistency
**Section:** 3.3 (R12)
**Requirements affected:** R12
**Problem:** R12 says: "All types contained in `Message` MUST derive `serde::Serialize` and `serde::Deserialize`. This includes `Net`, `Partition`, `IdRange`, `PortRef`, `Agent`, `Symbol`, and `WorkerRoundStats`."

With SPEC-10's extension, the `Message` enum now contains `RegisterPayload`, `RegisterAckPayload`, and `RegisterNackPayload`. These types also need serde derives. Additionally, SPEC-10 R14 specifies that `AuthToken` is transmitted as raw bytes `[u8; 32]` inside `RegisterPayload.auth_token: Option<[u8; 32]>`, which is inherently serializable, but the `AuthToken` wrapper type itself does NOT derive `Serialize`/`Deserialize` (it's a security type with custom Debug).

R12's enumeration of types is incomplete.

**Impact if unresolved:** If the implementer follows R12 as an exhaustive list, the Registration payload types may not get serde derives. In practice this is a minor issue since `#[derive(Serialize, Deserialize)]` is shown on the Registration structs in SPEC-10, but R12's claim to be comprehensive is misleading.

**Suggested resolution:** Add `RegisterPayload`, `RegisterAckPayload`, `RegisterNackPayload` to R12's list, or rephrase R12 to "All types transitively reachable from `Message` MUST derive `serde::Serialize` and `serde::Deserialize`" instead of providing an exhaustive list.

---

### SC-010: R25 abort-on-connection-loss is normatively correct but has no recovery mechanism
**Severity:** MEDIUM
**Axis:** Completeness | Testability
**Section:** 3.5 (R25)
**Requirements affected:** R25
**Problem:** R25 says: "If a connection with a worker is lost during execution, the coordinator MUST abort the grid loop and return an error." This is consistent with the TCC scope (Z5: fault tolerance out of scope). However, the spec does not specify:
1. What error type is returned (is it `ProtocolError::Io`? `CoordinatorError::WorkerFailed`?)
2. Whether the coordinator sends `Shutdown` to the remaining connected workers before aborting
3. Whether partial results (from workers that already returned) are available or discarded

SPEC-13 R21 provides more detail: `FatalError(e) -> Error -> LogTransition, ShutdownAll`. But SPEC-06 does not reference this resolution.

**Impact if unresolved:** TASK-0092 (Implement run_coordinator) must handle connection loss, but SPEC-06 gives only "abort and return error" with no specifics about cleanup behavior.

**Suggested resolution:** Clarify that on connection loss: (a) the coordinator transitions to Error state (SPEC-13), (b) sends `Shutdown` to all remaining connected workers, (c) returns `ProtocolError::Io` wrapping the connection error.

---

### SC-011: Coordinator pseudocode Phase 2b collects results sequentially, contradicting R22 MAY for concurrent
**Severity:** LOW
**Axis:** Testability
**Section:** Section 4.6 (coordinator pseudocode, Phase 2b)
**Requirements affected:** R22
**Problem:** R22 says collection "MAY be concurrent (each recv in parallel) or sequential (recv one by one)." The pseudocode in Section 4.6 shows a sequential `for` loop:
```
for stream in &mut worker_streams:
    let (msg, nbytes) = recv_frame(stream, config.max_payload_size).await?
```

This is correct per R22 (sequential is allowed), but it means the coordinator blocks on each worker in order. If Worker 1 finishes later than Worker 0, Worker 0's result sits in the TCP buffer while the coordinator waits for Worker 1 (assuming the loop order is fixed). The pseudocode doesn't demonstrate the concurrent option, which may be more performant.

This is not a correctness issue, but the pseudocode may mislead the implementer into always using sequential collection. Open Question 3 at the end of the spec acknowledges this.

**Impact if unresolved:** Minor performance concern. The implementer may naively follow the sequential pseudocode.

**Suggested resolution:** No change needed in the spec. The SHOULD/MAY in R22 and Open Question 3 are sufficient. Optionally, add a comment in the pseudocode: "// Note: concurrent collection via FuturesUnordered is permitted by R22."

---

### SC-012: R9 maximum payload default (256 MiB) has no justification or sizing analysis
**Severity:** LOW
**Axis:** Completeness
**Section:** 3.2 (R9)
**Requirements affected:** R9
**Problem:** R9 sets the default maximum payload at 256 MiB (268,435,456 bytes). No justification is provided for this specific value. For context:
- SPEC-09 benchmarks target nets up to 300K agents (EqualPartition)
- A partition of 300K agents with bincode: ~300K * (5 bytes agent + 9 bytes * 3 ports) = ~9.6 MB per partition
- Even with 1M agents, the serialized size would be ~32 MB

A 256 MiB limit seems generous but arbitrary. A lower default (e.g., 64 MiB) might be more appropriate for defense-in-depth (SPEC-10), while a 256 MiB default allows extremely large nets.

**Impact if unresolved:** No correctness issue. The default is configurable. But the value should be justified.

**Suggested resolution:** Add a brief rationale in R9 or Section 5: "The 256 MiB default accommodates nets up to ~8M agents per partition. For the benchmark suite (SPEC-09), typical partition sizes are < 10 MB."

---

### SC-013: R5 extensibility SHOULD conflicts with bincode's default behavior
**Severity:** LOW
**Axis:** Testability
**Section:** 3.1 (R5)
**Requirements affected:** R5
**Problem:** R5 says the `Message` enum "SHOULD be extensible: adding new variants in future versions should not break deserialization of existing variants." However, R11 mandates "bincode with default configuration (fixed-int encoding)." Bincode's default configuration assigns enum discriminants sequentially (0, 1, 2, ...). If a new variant is inserted between existing variants (rather than appended at the end), the discriminants shift and existing serialized messages become incompatible.

The extensibility SHOULD in R5 is only achievable if new variants are always appended at the end of the enum. This constraint is not documented.

**Impact if unresolved:** A future developer who inserts a variant in the middle of the `Message` enum will break wire compatibility without realizing it.

**Suggested resolution:** Add a note to R5: "New variants MUST be appended at the end of the enum to preserve bincode discriminant stability. Inserting variants in the middle changes the discriminants of subsequent variants, breaking backward compatibility."

---

### SC-014: R30 timeout values are SHOULD but R31 abort on timeout is MUST -- inconsistent severity
**Severity:** LOW
**Axis:** Consistency
**Section:** 3.7 (R30, R31)
**Requirements affected:** R30, R31
**Problem:** R30 says phase timeouts "SHOULD" be imposed with specified defaults. R31 says if a timeout is exceeded, the coordinator "MUST abort." This means: if you optionally implement timeouts (SHOULD), then when they fire you must abort (MUST). This creates an odd conditional MUST: the abort is mandatory only if the optional timeout was implemented.

This is technically correct RFC 2119 usage (SHOULD = recommended but not required; MUST within the context of having implemented it). But it may confuse the implementer.

**Impact if unresolved:** The implementer might skip timeouts entirely (since R30 is SHOULD) and then have no mechanism for detecting stuck workers.

**Suggested resolution:** Consider upgrading R30 from SHOULD to MUST for `collect_timeout` at least, since stuck workers are the most likely failure mode. `distribute_timeout` can remain SHOULD.

---

### SC-015: send_frame return type does not include bytes count for `send_frame` calls in shutdown
**Severity:** LOW
**Axis:** Completeness
**Section:** Section 4.6 (shutdown), Section 4.12
**Requirements affected:** R33, R34
**Problem:** The shutdown code in Section 4.6 shows:
```
for stream in &mut worker_streams:
    let _ = send_frame(stream, &Message::Shutdown).await
```

The `let _ =` discards the byte count returned by `send_frame`. R33 requires `bytes_sent_per_round` to include all coordinator bytes. Shutdown messages are sent outside the round loop, so they would not be counted in any round's metrics. This is likely intentional (shutdown is not part of a round), but R34 says "MUST include the 8 header bytes of each frame" -- it's not clear whether shutdown messages should be included in any metric at all.

**Impact if unresolved:** Minor. Shutdown byte counts are likely irrelevant for benchmarks.

**Suggested resolution:** Add a note clarifying that shutdown messages are not included in per-round metrics since they occur outside the grid loop.

---

### SC-016: No specification of byte order for PortRef tag in bincode
**Severity:** LOW
**Axis:** Completeness
**Section:** 3.3 (R11), Section 4.9
**Requirements affected:** R11
**Problem:** R11 mandates "bincode with default configuration (little-endian, fixed-int encoding)." Section 4.9's size estimates assume the enum tag consumes 4 bytes (u32) with fixed-int. This means the `PortRef` enum discriminant is 4 bytes, making `PortRef::AgentPort` = 4 (tag) + 4 (AgentId) + 1 (PortId) = 9 bytes.

However, bincode v2 (listed in SPEC-13 R11 as version 2.x) has a different default configuration than bincode v1. Bincode 2's `DefaultConfig` uses variable-length integer encoding, not fixed-int. The fixed-int configuration requires explicit `bincode::config::standard().with_fixed_int_encoding()`. If the implementer uses bincode 2 defaults, the size estimates in Section 4.9 will be wrong.

**Impact if unresolved:** Size estimates may be incorrect. The communication overhead analysis (Section 5.7, ARG-004) depends on accurate size estimates.

**Suggested resolution:** Clarify in R11 exactly which bincode configuration to use. If bincode 2.x, specify: `bincode::config::standard().with_little_endian().with_fixed_int_encoding()` or `bincode::config::legacy()`. Add a note that size estimates in Section 4.9 assume fixed-int encoding.

---

### SC-017: SPEC-06 does not reference SPEC-13's Transport trait abstraction
**Severity:** LOW
**Axis:** Consistency
**Section:** 3.4 (R16-R22), Section 4.3
**Requirements affected:** R16, R20
**Problem:** SPEC-06 defines `send_frame` and `recv_frame` as free functions operating on `AsyncReadExt`/`AsyncWriteExt` streams. SPEC-13 R28 defines a `Transport` trait with `send(&mut self, msg: &Message)` and `recv(&mut self)` methods that abstract over TCP and in-memory channels. The `TcpTransport` implementation (SPEC-13 R29) would call `send_frame`/`recv_frame` internally.

SPEC-06 has no mention of the `Transport` trait or the abstraction layer. Its pseudocode directly uses `TcpStream`, which is the underlying concrete type. The relationship between SPEC-06's framing functions and SPEC-13's Transport trait is not documented in either spec.

**Impact if unresolved:** The implementer must figure out that `send_frame`/`recv_frame` are internal implementation details of `TcpTransport`, not the public API. The public API is `Transport::send`/`Transport::recv`.

**Suggested resolution:** Add a note in Section 4.3: "The `send_frame` and `recv_frame` functions are internal to the `TcpTransport` implementation (SPEC-13 R29). External callers use the `Transport` trait (SPEC-13 R28)."

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 2 |
| HIGH | 3 |
| MEDIUM | 4 |
| LOW | 8 |

## Mandatory (must fix before implementation)

- **SC-001:** Message enum missing Register/RegisterAck/RegisterNack variants required by SPEC-10
- **SC-002:** FSM states R26-R28 superseded by SPEC-13 R19-R25 but not marked as such
- **SC-003:** ProtocolError::PayloadTooLarge naming conflicts with SPEC-13 ProtocolError::MessageTooLarge
- **SC-004:** Entire ProtocolError enum structure differs between SPEC-06 and SPEC-13
- **SC-005:** NodeConfig structure (host+port) incompatible with SPEC-13/SPEC-10 --bind SocketAddr pattern

## Recommended (should fix)

- **SC-006:** WorkerRoundStats pseudocode outdated relative to SPEC-11 extension
- **SC-007:** No registration handshake in coordinator pseudocode (required by SPEC-10)
- **SC-008:** No default port specified (SPEC-10/SPEC-13 say 9000)
- **SC-009:** R12 type list incomplete (missing Registration types from SPEC-10)

## Optional (nice to have)

- **SC-010:** R25 abort-on-connection-loss lacks cleanup details
- **SC-011:** Pseudocode shows only sequential collection despite R22 MAY concurrent
- **SC-012:** R9 default payload limit (256 MiB) has no sizing justification
- **SC-013:** R5 extensibility requires append-only enum discipline (undocumented)
- **SC-014:** R30 SHOULD timeout + R31 MUST abort creates conditional MUST
- **SC-015:** Shutdown message bytes not accounted in metrics
- **SC-016:** Bincode v2 default config differs from Section 4.9 size estimates
- **SC-017:** No reference to SPEC-13's Transport trait abstraction

---

## Checklist

### Consistency
- [x] Types match predecessor specs (Symbol, AgentId, PortRef, Net, Agent)
- [x] Serialization strategy matches SPEC-02 R24 (serde + bincode)
- [x] GridMetrics extension matches SPEC-05 R36
- [ ] **FAIL:** Message enum missing 3 variants required by SPEC-10 R19 (SC-001)
- [ ] **FAIL:** FSM states R26-R28 superseded by SPEC-13 R19-R25 (SC-002)
- [ ] **FAIL:** ProtocolError variant naming conflicts with SPEC-13 R16 (SC-003, SC-004)
- [ ] **FAIL:** NodeConfig structure (host+port) incompatible with SPEC-13 R44 --bind pattern (SC-005)
- [ ] **PARTIAL:** WorkerRoundStats pseudocode shows 4 fields, SPEC-11 extends to 6 (SC-006)
- [ ] **PARTIAL:** No registration handshake in coordinator pseudocode despite SPEC-10 R14-R17 (SC-007)
- [ ] **PARTIAL:** No default port specified, SPEC-10/SPEC-13 both say 9000 (SC-008)
- [ ] **PARTIAL:** R12 type list incomplete for Registration types from SPEC-10 (SC-009)
- [x] CRC32C checksum approach is internally consistent (R6, R8, R10, R29)
- [x] Framing format is fully specified (R6-R10)
- [x] Complexity bounds (R39, R40) are consistent with overhead analysis (DISC-006)

### Testability
- [x] R1 (Message enum): testable by verifying variant count
- [x] R4 (serde round-trip): testable with `deserialize(serialize(msg)) == msg`
- [x] R6-R8 (framing): testable with byte-level assertions
- [x] R9 (max payload): testable by sending oversize message
- [x] R10 (CRC32): testable by corrupting payload and verifying rejection
- [x] R11 (bincode config): testable by verifying serialized sizes
- [x] R14 (serialization identity): testable with property-based testing
- [x] R21 (concurrent send): testable by timing concurrent vs sequential sends
- [x] R23 (exponential backoff): testable with mock server that delays accept
- [x] R24 (wait for all workers): testable with timeout assertion
- [x] R29 (checksum verification): testable with corruption injection
- [x] R32 (no per-message ACKs): testable by verifying protocol has no ACK messages
- [x] R33-R34 (metrics): testable by comparing byte counts to frame sizes
- [x] R39-R40 (complexity): testable with benchmarks at varying net sizes

### Completeness
- [x] Frame format fully specified (header + payload structure)
- [x] Serialization strategy fully specified (bincode + serde derives)
- [x] Error handling specified (ProtocolError enum with all variants)
- [x] Configuration specified (NodeConfig)
- [x] Metrics integration specified (GridMetrics extension)
- [x] Connect-with-retry specified (backoff parameters)
- [x] Shutdown protocol specified
- [x] Rationale section covers all design decisions with alternatives
- [x] Haskell prototype comparison provided
- [ ] **PARTIAL:** Registration handshake not specified (delegated to SPEC-10 but not referenced)
- [ ] **PARTIAL:** Default port not specified (SC-008)
- [ ] **PARTIAL:** Cleanup behavior on connection loss not specified (SC-010)

### Invariant Preservation
- [x] T1 (linearity): wire protocol does not modify net structure; serialization round-trip preserves topology (R14)
- [x] T4 (strong confluence): not affected by wire protocol
- [x] T5 (6 rules): not affected by wire protocol
- [x] D1 (split/merge identity): serialization identity (R14) ensures transmitted partitions are faithful
- [x] D3 (border redex completeness): SPEC-06 references SPEC-05 merge+reduce_all; the protocol correctly transmits all partitions and results
- [x] D4 (ID uniqueness): IdRange is included in Partition (SPEC-04), transmitted faithfully
- [x] D5 (exclusive ownership): each partition sent to exactly one worker (R21, R22)
- [x] G1 (fundamental property): preserved if serialization is correct (R14) and all partitions are transmitted and collected (R21, R22)
- [x] I1 (bidirectional port array): serialization preserves port array structure (R14)
