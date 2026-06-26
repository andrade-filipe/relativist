# SPEC-06: Wire Protocol

**Status:** Revised v3.1 — `Message` enum amended per SPEC-21 §3.8 A2 (`RequestWork`/`NoMoreWork` variants + PROTOCOL_VERSION sequencing)
**Depends on:** SPEC-00 (Glossary), SPEC-01 (Invariants), SPEC-02 (Net Representation), SPEC-04 (Partitioning), SPEC-05 (Merge and Grid Cycle)
**Amends:** SPEC-21 §3.8 A2 (`Message` enum gains `RequestWork { worker_id: WorkerId }` and `NoMoreWork`; PROTOCOL_VERSION bump per defensive `PREVIOUS_LIVE_VERSION + 1` language; SPEC-21 R31, R37c)
**Gray zones resolved:** ---
**References consumed:** REF-001, REF-002, REF-003, REF-004, REF-013, REF-014
**Discussions consumed:** DISC-005 v2 (cross-boundary protocol, serialization format), DISC-006 v2 (communication overhead, granularity, bincode vs [Int] comparison), DISC-008 v2 (shared-to-distributed transition, serialization as operational cost)
**Arguments consumed:** ARG-004 (practical viability, overhead decomposition, break-even analysis)
**Code analyses consumed:** AC-003 (Haskell Protocol/Network: length-prefixed TCP, GridState FSM, sendMessage/recvMessage, connectWithRetry, gridLoop, workerLoop), AC-015 (CC-7 serialization of nets: Haskell [Int] vs HVM2 compact encoding)
**Spec reviews consumed:** SPEC-06-round1-critic.md (SC-001 through SC-017)

---

## 1. Purpose

This spec defines the wire protocol for distributed communication between the coordinator and workers in Relativist when operating in grid mode over TCP. It covers: the message catalog (including registration variants from SPEC-10), the TCP framing format, the serialization strategy (serde + bincode), the integrity and timeout policies, the network metrics integrated into `GridMetrics` (SPEC-05), and the connection configuration. This spec transforms the local grid loop of SPEC-05 into a real distributed grid loop, where Phase 2 (local reduction) occurs in remote processes connected via TCP.

> **Note on FSM ownership:** The finite state machines for coordinator and worker are documented in this spec for historical context (Section 3.6). The authoritative FSM definitions, including state names, transition tables, and the enum-based implementation requirement, are in SPEC-13 R19-R25. See the supersession note in Section 3.6.

## 2. Definitions

Terms defined in SPEC-00 (Glossary) are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **Wire Protocol** | The binary protocol over TCP that defines how coordinator and workers exchange messages (partitions, results, registration, status, shutdown). (Relativist) |
| **Frame** | A unit of transmission in the wire protocol: an 8-byte header (4 bytes length + 4 bytes checksum) followed by a variable-length payload. |
| **Length-Delimited Framing** | A framing technique where each message is preceded by its length in bytes, allowing the receiver to know exactly how many bytes to read. Used by the Haskell prototype (AC-003) with 4 bytes big-endian. Relativist uses 4 bytes little-endian (bincode convention). |
| **Coordinator** | The central process that orchestrates the grid cycle: partitions the net, sends partitions to workers, receives results, executes merge, and resolves border redexes. Corresponds to `runCoordinator` in the Haskell prototype (AC-003). |
| **Worker** | A remote process that receives a partition, reduces locally via `reduce_all` (SPEC-03), reconstructs the `free_port_index` (SPEC-05, Section 4.3), and returns the reduced partition to the coordinator. Corresponds to `workerLoop` in the Haskell prototype (AC-003). |
| **Persistent Connection** | A TCP connection kept open between the coordinator and each worker for the entire duration of the grid loop. Avoids TCP handshake overhead at each round. |
| **Registration Handshake** | The initial exchange of `Register`/`RegisterAck` (or `RegisterNack`) messages when a worker connects to the coordinator. Required when authentication is enabled (SPEC-10, Tier 2/3). In Tier 1 (no auth), registration is implicit upon TCP connection acceptance. |

---

## 3. Requirements

### 3.1 Protocol Messages

**R1.** The protocol MUST define an enum `Message` with variants covering all communication between coordinator and workers, including the registration handshake (SPEC-10). **(MUST)**

**R2.** The coordinator-to-worker messages MUST include at least:
- `AssignPartition`: sends a partition for local reduction.
- `Shutdown`: signals the worker to close its connection.
- `RegisterAck`: confirms worker registration (sent after successful authentication or implicit acceptance). Defined by SPEC-10 Section 4.3.
- `RegisterNack`: rejects worker registration (sent on authentication failure). Defined by SPEC-10 Section 4.3.
**(MUST)**

**R2a.** The registration handshake behavior MUST be tier-dependent (SPEC-10 R1):
- **Tier 1 (no auth):** The coordinator MAY accept connections implicitly without requiring a `Register` message. If a worker sends a `Register` message in Tier 1, the coordinator MUST accept it unconditionally (ignoring `auth_token`). This is consistent with SPEC-13 R21's note on implicit registration.
- **Tier 2/3 (auth enabled):** The worker MUST send a `Register` message containing the authentication token, and the coordinator MUST respond with `RegisterAck` or `RegisterNack` per SPEC-10 R14-R17.
**(MUST)**

**R3.** The worker-to-coordinator messages MUST include at least:
- `PartitionResult`: returns the reduced partition.
- `Error`: reports an irrecoverable error during reduction.
- `Register`: worker registration request with optional authentication token. Defined by SPEC-10 Section 4.3.
**(MUST)**

**R3a. Pull-Dispatch Variants (Amendment A2 — SPEC-21 §3.8 A2 / R31).** When the streaming pipeline (SPEC-21 §3.3 R17) is active under `DispatchMode::Pull` (SPEC-21 R34), the `Message` enum MUST include two additional variants:

```rust
RequestWork { worker_id: WorkerId },  // Worker -> Coordinator
NoMoreWork,                            // Coordinator -> Worker
```

`RequestWork` is sent by the worker to indicate readiness for a new chunk. `NoMoreWork` is sent by the coordinator when the generator stream is exhausted. Both variants MUST be appended at the end of the `Message` enum (after `RegisterNack`) per R5 discriminant-stability rule, MUST serialize through SPEC-18 wire-format-v2 serde without modification to the framing layer (length-prefixed, bincode-encoded), and are mode-agnostic at the wire layer (per SPEC-21 R37e) but mode-specific at the FSM layer (push mode MUST NOT emit them; coordinators MUST NOT emit `NoMoreWork` in push mode; workers MUST NOT add defensive `NoMoreWork` handling to push-mode transitions). **(MUST when `DispatchMode::Pull`; ABSENT-FROM-WIRE when `DispatchMode::Push`)**

> **Amendment A2 (SPEC-21 §3.8 A2 / R31, R37c, R37e):** Closes SC-001 part 1. The variants are required by R30-R32 pull dispatch. FSM-level scoping discipline lives in SPEC-13 §3.5/§3.6 (per SPEC-21 §3.8 A5).

**R3b. PROTOCOL_VERSION Sequencing (Amendment A2 — SPEC-21 §3.8 A2 / R37c).** The `PROTOCOL_VERSION` constant (canonically owned by SPEC-18 §4.7 / R28; surfaced via `RegisterPayload.protocol_version`) MUST be bumped using **defensive `PREVIOUS_LIVE_VERSION + 1` language** rather than a hardcoded absolute integer. Concretely: `PROTOCOL_VERSION ← PREVIOUS_LIVE_VERSION + 1` (= 6 at the time of writing, given the current value 5 from SPEC-22 D-009 Phase A landing). This defensive sequencing prevents merge-order reshuffling between SPEC-20 / SPEC-21 / SPEC-22 from silently producing wrong absolute version numbers (mirrors SPEC-22 R9a / TASK-0476 precedent and the SPEC-20 R37 v3-vs-v4 pattern). Pre-bump deserializers MUST reject post-bump payloads with `ProtocolError::UnsupportedVersion`, mirroring SPEC-22 R10b's rejection clause. **(MUST)**

> **Amendment A2 (SPEC-21 §3.8 A2 / R37c):** Closes SC-001 part 1 (version-bump half). The version bump itself happens in TASK-0576 (production), which depends on TASK-0476 (SPEC-22 wire-version-bump precedent). See also SPEC-18 R28 / §4.7 for the canonical constant declaration site.

**R4.** Every variant of the `Message` enum MUST be serializable and deserializable via serde + bincode (confirmed technical decision, SPEC-02 R24). **(MUST)**

**R5.** The `Message` enum SHOULD be extensible: adding new variants in future versions should not break deserialization of existing variants, provided the receiver discards unknown variants with a graceful error. New variants MUST be appended at the end of the enum to preserve bincode discriminant stability. Inserting variants in the middle changes the discriminants of subsequent variants, breaking backward compatibility with any previously serialized messages. **(SHOULD)**

### 3.2 Framing Format

**R6.** Each message transmitted over TCP MUST be framed with an 8-byte header: 4 bytes payload length (little-endian u32) followed by 4 bytes CRC32 checksum of the payload (little-endian u32). **(MUST)**

**R7.** The payload length in the header MUST represent the exact number of bytes of the bincode-serialized payload that follows the header. **(MUST)**

**R8.** The receiver MUST read exactly the 8 header bytes, extract the length and checksum, read exactly `length` bytes of payload, verify the checksum before deserializing, and reject the message if the checksum does not match. **(MUST)**

**R9.** The maximum payload size MUST be configurable, with a default of 256 MiB (`268_435_456` bytes). Messages with a declared length above the maximum MUST be rejected before allocating memory. The 256 MiB default accommodates nets up to approximately 8M agents per partition (at ~32 bytes per agent+ports). For the benchmark suite (SPEC-09), typical partition sizes are under 10 MB. The generous default prevents false rejections for exploratory workloads beyond the benchmark suite while still providing defense against unbounded allocation. **(MUST)**

**R10.** The checksum MUST use CRC32 (CRC32C/Castagnoli variant, available via crate `crc32fast`). CRC32C is chosen because: (a) it detects common transmission errors in TCP (bit flips, truncation), (b) it has hardware acceleration on x86 (SSE4.2 `crc32` instruction), (c) its computational overhead is negligible compared to bincode serialization. **(MUST)**

### 3.3 Serialization

**R11.** The serialization of each `Message` payload MUST use bincode with configuration equivalent to `bincode::config::standard().with_little_endian().with_fixed_int_encoding()`. Note: bincode v2.x default configuration uses variable-length integer encoding, which differs from bincode v1 defaults. Explicit configuration with fixed-int encoding is required for predictable wire sizes and for the size estimates in Section 4.9 to be accurate. **(MUST)**

**R12.** All types transitively reachable from `Message` MUST derive `serde::Serialize` and `serde::Deserialize`. This includes (non-exhaustive): `Net`, `Partition`, `IdRange`, `PortRef`, `Agent`, `Symbol`, `WorkerRoundStats`, `RegisterPayload`, `RegisterAckPayload`, and `RegisterNackPayload` (types defined in SPEC-02, SPEC-04, SPEC-05, and SPEC-10). The complete `WorkerRoundStats` type is defined in SPEC-05 R37 with observability extensions in SPEC-11 Section 4.4 (canonical for the full 6-field definition). **(MUST)**

**R13.** The serialized format MUST be self-contained: a receiver with the same type schema MUST be able to reconstruct the complete message from the received bytes. **(MUST)**

**R14.** Serialization MUST preserve identity: `deserialize(serialize(msg)) == msg` for every valid message `msg`. **(MUST)**

**R15.** The bincode format SHOULD result in payloads approximately 50% smaller than the `[Int]` format of the Haskell prototype (AC-003: ~2x overhead), since bincode encodes Symbol as 1 byte (u8), PortId as 1 byte (u8), and AgentId as 4 bytes (u32), as analyzed in DISC-006 v2 Section 1.3. **(SHOULD)**

### 3.4 TCP Transport

**R16.** The transport MUST use TCP as the network protocol, with persistent connections between coordinator and each worker. **(MUST)**

**R17.** The coordinator MUST open a TCP listener socket on a configurable address and wait for connections from workers. Default bind address: `127.0.0.1:9000` (consistent with SPEC-10 R5 and SPEC-13 R44). **(MUST)**

**R18.** Each worker MUST connect to the coordinator via TCP upon startup, using the coordinator address configured via CLI (SPEC-13 R45: `--coordinator` flag). **(MUST)**

**R19.** Connections MUST remain open for the entire duration of the grid loop. A connection is only closed after the coordinator sends `Shutdown` or after an irrecoverable error. **(MUST)**

**R20.** All network I/O MUST be asynchronous, using `tokio` as the async runtime (confirmed technical decision). **(MUST)**

**R21.** The coordinator MUST send partitions to workers concurrently (not sequentially): all `AssignPartition` writes MUST be initiated before awaiting any `PartitionResult`. **(MUST)**

**R22.** The coordinator MUST await `PartitionResult` from all workers before proceeding to merge. The collection of results MAY be concurrent (each recv in parallel) or sequential (recv one by one). **(MUST for awaiting all; MAY for ordering)**

### 3.5 Connection and Retry

**R23.** Each worker MUST implement retry with exponential backoff to connect to the coordinator. The backoff MUST start at 1 second and double at each attempt, up to a maximum of 16 seconds, with at most 10 attempts. This behavior is identical to `connectWithRetry` in the Haskell prototype (AC-003, lines 331-352). **(MUST)**

> **Note (SPEC-16):** In daemon mode (SPEC-16 R4), `connect_with_retry` retries indefinitely (no maximum attempts). The exponential backoff and 16-second cap still apply.

**R24.** The coordinator MUST wait for all `num_workers` workers to connect (and complete the registration handshake per R2a, if authentication is enabled) before starting the first round of the grid loop. A configurable timeout (default: 120 seconds) MUST abort execution if not all workers have registered in time. **(MUST)**

**R25.** If a connection with a worker is lost during execution, the coordinator MUST abort the grid loop and return an error. Fault tolerance is out of scope (OBJETIVO_TCC.md, Z5). On connection loss, the coordinator MUST: (a) transition to the Error state (SPEC-13 R21), (b) send `Shutdown` to all remaining connected workers (best-effort, errors ignored), and (c) return `ProtocolError::ConnectionLost` wrapping the underlying I/O error. **(MUST)**

### 3.6 Finite State Machines (FSM)

> **SUPERSESSION NOTE:** The FSM definitions in R26-R28 have been superseded by SPEC-13 R19-R25. The state names and transition tables in SPEC-13 are authoritative. R26-R28 are retained below for historical context and traceability only. Where the implementer encounters a conflict between SPEC-06 R26-R28 and SPEC-13 R19-R25, SPEC-13 is authoritative.
>
> **State name mapping (SPEC-06 -> SPEC-13):**
>
> Coordinator:
> - `WaitingWorkers` -> `WaitingForWorkers`
> - `Idle` -> replaced by `CheckTermination`
> - `Partitioning` -> `Partitioning` (same)
> - `Distributing` -> `Dispatching`
> - `WaitingResults` -> `WaitingForResults`
> - `Merging` -> `Merging` (same)
> - `ShuttingDown` -> subsumed by `Done` + `ShutdownAll` action
> - `Done` -> `Done` (same)
> - *(new in SPEC-13)* -> `Init`, `CheckTermination`, `Error`
>
> Worker:
> - `Connecting` -> `Init`
> - `Idle` -> `Idle` (same)
> - `Reducing` -> `Reducing` (same)
> - `Sending` -> `Returning`
> - `Done` -> `Done` (same)
> - *(new in SPEC-13)* -> `Error`
>
> Additionally, R28's "MAY be implemented implicitly via control flow" is superseded by SPEC-13 R22's "MUST be enum-based."

**R26.** *(Historical -- superseded by SPEC-13 R19, R21)* The coordinator operates according to the following FSM:

| State | Description | Transition |
|-------|-------------|-----------|
| `WaitingWorkers` | Waiting for worker connections | All connected -> `Idle` |
| `Idle` | Net ready to process | Redexes exist -> `Partitioning`; Normal Form -> `ShuttingDown` |
| `Partitioning` | Executing `partition()` | Partitions ready -> `Distributing` |
| `Distributing` | Sending partitions to workers | All sent -> `WaitingResults` |
| `WaitingResults` | Awaiting `PartitionResult` from all workers | All received -> `Merging` |
| `Merging` | Executing merge + resolve borders | Merge complete -> `Idle` |
| `ShuttingDown` | Sending `Shutdown` to all workers | All notified -> `Done` |
| `Done` | Execution complete | (terminal) |

**R27.** *(Historical -- superseded by SPEC-13 R24, R25)* Each worker operates according to the following FSM:

| State | Description | Transition |
|-------|-------------|-----------|
| `Connecting` | Attempting to connect to coordinator | Connected -> `Idle` |
| `Idle` | Awaiting message from coordinator | Received `AssignPartition` -> `Reducing`; Received `Shutdown` -> `Done` |
| `Reducing` | Executing `reduce_all` on the partition | Reduction complete -> `Sending` |
| `Sending` | Sending `PartitionResult` to coordinator | Sent -> `Idle` |
| `Done` | Received shutdown, terminating | (terminal) |

**R28.** *(Historical -- superseded by SPEC-13 R22)* The coordinator and worker FSMs were originally specified as MAY for explicit implementation. SPEC-13 R22 requires enum-based FSMs. The FSMs documented here serve as historical specification of expected behavior.

### 3.7 Integrity and Timeouts

**R29.** The CRC32 checksum (R10) MUST be verified by the receiver before deserializing the payload. If the checksum does not match, the message MUST be rejected with a `ChecksumMismatch` error. **(MUST)**

**R30.** The coordinator MUST impose a `collect_timeout` per round (default: 600 seconds -- large nets may take a long time). The coordinator SHOULD impose a `distribute_timeout` per round (default: 60 seconds, maximum time to send all partitions). **(MUST for collect_timeout; SHOULD for distribute_timeout)**

**R31.** If a timeout is exceeded, the coordinator MUST abort the grid loop and return an error. Retry of rounds is not implemented in v1 (out of scope: Z5). **(MUST)**

**R32.** The wire protocol does NOT require per-message acknowledgments (ACKs). The semantics are fire-and-wait: the coordinator sends `AssignPartition` and awaits `PartitionResult`. TCP guarantees reliable and ordered delivery in the failure-free scenario (scope of this TCC). **(MUST)**

### 3.8 Network Metrics

**R33.** The wire protocol MUST collect per-round communication metrics and integrate them into `GridMetrics` (SPEC-05, R36). The additional metrics MUST include:
- `bytes_sent_per_round: Vec<usize>` -- total bytes sent by the coordinator in the round (headers + payloads of all partitions).
- `bytes_received_per_round: Vec<usize>` -- total bytes received by the coordinator in the round (headers + payloads of all results).
- `network_send_time_per_round: Vec<Duration>` -- wall-clock time to send all partitions (start of first send to completion of last send).
- `network_recv_time_per_round: Vec<Duration>` -- wall-clock time to collect all results (start of first recv to completion of last recv).
**(MUST)**

**R34.** The bytes counted in `bytes_sent_per_round` and `bytes_received_per_round` MUST include the 8 header bytes (length + checksum) of each frame, in addition to the payload. This reflects the real cost on the wire. **(MUST)**

**R35.** The network metrics MUST be sufficient to calculate the communication overhead as a fraction of total time, per the formula from DISC-006 v2 Section 1.1:
```
communication_overhead = (network_send_time + network_recv_time) / total_time
```
**(MUST)**

### 3.9 Configuration

**R36.** The coordinator MUST be configurable via a `NodeConfig` structure containing at least:
- `bind: SocketAddr` -- address and port for the TCP listener. Default: `127.0.0.1:9000` (consistent with SPEC-10 R5 and SPEC-13 R44).
- `num_workers: u32` -- expected number of workers.
- `max_payload_size: u32` -- maximum accepted payload size. Default: `268_435_456` (256 MiB).
- `worker_connect_timeout: Duration` -- timeout for waiting for all workers to connect. Default: 120 seconds.
- `distribute_timeout: Duration` -- timeout for distributing partitions. Default: 60 seconds.
- `collect_timeout: Duration` -- timeout for collecting results. Default: 600 seconds.

Note: SPEC-13 R44-R45 defines separate `CoordinatorArgs` and `WorkerArgs` structs for the CLI layer. `NodeConfig` is an internal configuration struct populated after CLI parsing, not a CLI argument struct. The coordinator and worker know their role from the CLI subcommand (SPEC-13 R43), so a `role` discriminant is unnecessary.
**(MUST)**

**R37.** Configuration MUST be provided via CLI arguments (using `clap`, confirmed technical decision). **(MUST)**

**R38.** Communication logs (messages sent/received, sizes, errors) SHOULD use the `tracing` crate with configured levels:
- `info`: start/end of each round, number of workers connected.
- `debug`: size of each message sent/received, duration of each network operation.
- `trace`: dump of serialized content (development only).
**(SHOULD)**

### 3.10 Complexity

**R39.** The cost of framing (header serialization, CRC32) MUST be O(n) where n is the payload size in bytes. **(MUST)**

**R40.** The total communication cost per round MUST be O(sum(P_i) + sum(R_i)) where P_i is the serialized size of partition i and R_i is the serialized size of result i. There MUST NOT be super-linear communication overhead. **(MUST)**

---

## 4. Design

### 4.1 Message Catalog

```rust
use serde::{Serialize, Deserialize};

/// Messages in the communication protocol between coordinator and workers.
///
/// The enum covers all possible communication. Each variant is annotated
/// with the direction (Coordinator->Worker or Worker->Coordinator) and the
/// FSM state in which it is expected.
///
/// The enum has 7 variants: 4 defined by SPEC-06 (core protocol) and 3
/// defined by SPEC-10 (registration/authentication). SPEC-06 is the canonical
/// owner of the Message enum definition; SPEC-10 defines the registration
/// variant semantics and payload structures.
///
/// IMPORTANT: New variants MUST be appended at the end of this enum to
/// preserve bincode discriminant stability (R5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    // === Coordinator -> Worker (SPEC-06 core) ===

    /// Sends a partition for the worker to reduce locally.
    /// Sent in the coordinator's Dispatching state (SPEC-13 R19).
    /// The worker transitions from Idle to Reducing upon receipt.
    AssignPartition {
        /// Current round number (0-indexed). Used for correlation and logging.
        round: u32,
        /// The complete partition to be reduced. Contains the sub-net, the worker_id,
        /// the free_port_index, and the id_range (SPEC-04).
        partition: Partition,
    },

    /// Signals the worker to terminate. The worker MUST close the connection
    /// after receiving this message.
    /// Sent when the coordinator transitions to Done state (SPEC-13 R19).
    Shutdown,

    // === Worker -> Coordinator (SPEC-06 core) ===

    /// Returns the reduced partition to the coordinator.
    /// Sent by the worker after completing reduce_all + rebuild_free_port_index.
    PartitionResult {
        /// Round number (echo of the value received in AssignPartition).
        round: u32,
        /// The partition with the locally-reduced sub-net and reconstructed free_port_index.
        partition: Partition,
        /// Local reduction statistics for this worker in this round.
        /// Full type definition with 6 fields: SPEC-05 R37 + SPEC-11 Section 4.4.
        stats: WorkerRoundStats,
    },

    /// Reports an irrecoverable error in the worker.
    /// The coordinator MUST abort the grid loop upon receipt.
    Error {
        /// Round number in which the error occurred.
        round: u32,
        /// Identifier of the worker that reported the error.
        worker_id: WorkerId,
        /// Textual description of the error.
        description: String,
    },

    // === Registration (SPEC-10 Section 4.3) ===
    //
    // These three variants implement the worker registration handshake.
    // In Tier 1 (no auth), registration is implicit upon TCP connection
    // acceptance and the Register message is optional (R2a).
    // In Tier 2/3 (auth enabled), the Register/RegisterAck handshake
    // is mandatory (SPEC-10 R14-R17).

    /// Worker registration request. First message on every new connection
    /// when authentication is enabled (Tier 2/3).
    /// Worker -> Coordinator.
    Register(RegisterPayload),

    /// Registration accepted. Coordinator -> Worker.
    RegisterAck(RegisterAckPayload),

    /// Registration rejected. Coordinator -> Worker.
    /// Connection MUST be closed after sending this.
    RegisterNack(RegisterNackPayload),

    // === Pull-Dispatch (SPEC-21 §3.8 A2 / R31) ===
    //
    // Active only under DispatchMode::Pull (SPEC-21 R34). Push-mode coordinators
    // MUST NOT emit NoMoreWork; push-mode workers MUST NOT emit RequestWork.
    // Variants are appended at end per R5 discriminant-stability rule.

    /// Worker requests a new chunk after sending `PartitionResult`.
    /// Worker -> Coordinator.
    /// Sent in the worker's `AwaitingChunkAfterResult` state (SPEC-13 §3.6).
    RequestWork { worker_id: WorkerId },

    /// Coordinator signals that the generator stream is exhausted.
    /// Coordinator -> Worker.
    /// Sent in the coordinator's `SendingNoMoreWork` state (SPEC-13 §3.5).
    /// The worker transitions to `FinalReduction` upon receipt.
    NoMoreWork,
}

/// Registration payload. Defined by SPEC-10 Section 4.3.
/// Direction: Worker -> Coordinator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterPayload {
    /// Protocol version for fast rejection of incompatible clients.
    /// Current version: 1.
    pub protocol_version: u8,
    /// Authentication token. None when running in Tier 1 (no auth).
    /// Some(raw_bytes) when running in Tier 2 or 3.
    pub auth_token: Option<[u8; 32]>,
}

/// Registration accepted payload. Defined by SPEC-10 Section 4.3.
/// Direction: Coordinator -> Worker (success).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterAckPayload {
    /// The WorkerId assigned to this worker by the coordinator.
    pub worker_id: WorkerId,
}

/// Registration rejected payload. Defined by SPEC-10 Section 4.3.
/// Direction: Coordinator -> Worker (failure).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterNackPayload {
    /// Human-readable reason for rejection.
    /// MUST be generic (e.g., "authentication failed") and MUST NOT
    /// reveal internal state (SPEC-10 R35).
    pub reason: String,
}
```

**Note on extensibility:** Future versions may add variants such as `RequestStatus` (coordinator -> worker for diagnostics) and `StatusReport` (worker -> coordinator with heartbeat). These variants are omitted in v1 because the TCC scope assumes a failure-free scenario (OBJETIVO_TCC.md). The enum structure allows adding them without breaking compatibility, provided they are appended at the end (R5).

### 4.2 Frame Format

Each message transmitted over TCP is encapsulated in a frame with the following structure:

```
+------------------+------------------+-----------------------------+
| Length (4 bytes)  | CRC32 (4 bytes)  | Payload (length bytes)      |
| little-endian u32 | little-endian u32 | bincode-serialized Message  |
+------------------+------------------+-----------------------------+
```

**Total on the wire:** `8 + length` bytes per frame.

```rust
/// Header of a frame in the wire protocol.
/// Precedes each payload transmitted over TCP.
#[derive(Debug, Clone, Copy)]
pub struct FrameHeader {
    /// Length of the payload in bytes (excluding the header itself).
    pub length: u32,
    /// CRC32C checksum of the payload.
    pub checksum: u32,
}

/// Header size in bytes.
pub const FRAME_HEADER_SIZE: usize = 8;

/// Default maximum payload size (256 MiB).
pub const DEFAULT_MAX_PAYLOAD_SIZE: u32 = 268_435_456;
```

### 4.3 Framing Functions

> **Note on Transport abstraction:** The `send_frame` and `recv_frame` functions defined here are internal implementation details of `TcpTransport` (SPEC-13 R29). External callers use the `Transport` trait (SPEC-13 R28), which abstracts over TCP and in-memory channels. The framing functions are the low-level building blocks; the `Transport` trait is the public API.

```rust
use crc32fast::Hasher;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Serializes a message and sends it as a frame over the TCP socket.
///
/// Returns the total number of bytes written (header + payload).
///
/// Steps:
/// 1. Serialize Message with bincode -> payload: Vec<u8>.
/// 2. Compute CRC32C of the payload.
/// 3. Write header (length + checksum) as 8 bytes little-endian.
/// 4. Write payload.
/// 5. Flush the buffer.
pub async fn send_frame<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    message: &Message,
) -> Result<usize, ProtocolError>
```

```rust
/// Reads a frame from the TCP socket and deserializes the message.
///
/// Returns the deserialized message and the total number of bytes read.
///
/// Steps:
/// 1. Read exactly 8 header bytes.
/// 2. Extract length and checksum from the header.
/// 3. Reject if length > max_payload_size (defense against OOM).
/// 4. Read exactly `length` bytes of payload.
/// 5. Verify CRC32C of payload against header checksum.
/// 6. Deserialize payload with bincode -> Message.
pub async fn recv_frame<R: AsyncReadExt + Unpin>(
    reader: &mut R,
    max_payload_size: u32,
) -> Result<(Message, usize), ProtocolError>
```

**Pseudocode for `recv_frame`:**

```
async fn recv_frame(reader, max_payload_size) -> Result<(Message, usize)>:
    // 1. Read header
    let mut header_buf = [0u8; FRAME_HEADER_SIZE]
    reader.read_exact(&mut header_buf).await?

    // 2. Extract fields
    let length = u32::from_le_bytes(header_buf[0..4])
    let checksum = u32::from_le_bytes(header_buf[4..8])

    // 3. Validate size
    if length > max_payload_size:
        return Err(ProtocolError::PayloadTooLarge { size: length, max: max_payload_size })

    // 4. Read payload
    let mut payload = vec![0u8; length as usize]
    reader.read_exact(&mut payload).await?

    // 5. Verify checksum
    let computed_crc = crc32fast::hash(&payload)
    if computed_crc != checksum:
        return Err(ProtocolError::ChecksumMismatch {
            expected: checksum,
            computed: computed_crc,
        })

    // 6. Deserialize
    let message: Message = bincode::deserialize(&payload)?

    let total_bytes = FRAME_HEADER_SIZE + length as usize
    Ok((message, total_bytes))
```

### 4.4 Protocol Error Types

> **Canonicality note:** SPEC-06 Section 4.4 is the canonical owner of the `ProtocolError` enum definition. SPEC-13 R16 defines a high-level sketch of per-module error enums; where the two differ, SPEC-06 is authoritative for `ProtocolError`. SPEC-13 R16's note that "individual variants MAY be added or renamed during implementation" permits the richer variant set defined here.

```rust
use std::time::Duration;

/// Possible errors in the wire protocol.
///
/// This is the canonical definition. SPEC-13 R16 provides a high-level
/// sketch; this definition is authoritative for field names and types.
#[derive(Debug)]
pub enum ProtocolError {
    /// Connection lost (I/O error in TCP communication).
    /// Named `ConnectionLost` (SPEC-13 convention) rather than `Io`
    /// for clarity.
    ConnectionLost(std::io::Error),

    /// Declared payload exceeds the maximum allowed size.
    /// Field types are `u32` consistent with the frame header's `length` field.
    PayloadTooLarge {
        size: u32,
        max: u32,
    },

    /// CRC32 checksum of the payload does not match the header declaration.
    /// Structured fields enable diagnostics; SPEC-13's fieldless variant
    /// is a simplification that loses diagnostic information.
    ChecksumMismatch {
        expected: u32,
        computed: u32,
    },

    /// Bincode deserialization error.
    Deserialize(bincode::Error),

    /// Bincode serialization error.
    Serialize(bincode::Error),

    /// Unexpected message for the current FSM state.
    /// E.g., worker received PartitionResult, or coordinator received AssignPartition.
    UnexpectedMessage {
        expected: &'static str,
        received: String,
    },

    /// Timeout exceeded in an operation.
    Timeout {
        phase: &'static str,
        elapsed: Duration,
    },

    /// Authentication failed (SPEC-10).
    /// Emitted when a worker's `Register` message contains an invalid
    /// or missing auth token.
    AuthFailed,
}
```

> **Note on variants moved to CoordinatorError:** The following variants from SPEC-06 Revised v2 have been moved to `CoordinatorError` (SPEC-13 R16), as they are coordinator-level concerns rather than wire protocol errors:
> - `WorkerError { worker_id, round, description }` -> `CoordinatorError::WorkerFailed(WorkerId, String)`
> - `WorkerCountMismatch { expected, connected }` -> `CoordinatorError::NoWorkers` (or equivalent)

### 4.5 Node Configuration

```rust
use std::net::SocketAddr;
use std::time::Duration;

/// Configuration of the coordinator node.
///
/// Note: SPEC-13 R44-R45 defines separate `CoordinatorArgs` and `WorkerArgs`
/// for the CLI layer. `NodeConfig` is an internal configuration struct
/// populated after CLI argument parsing. The coordinator and worker know
/// their role from the CLI subcommand (SPEC-13 R43), so a `role` field
/// is unnecessary.
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// Address and port for the TCP listener (coordinator) or
    /// coordinator address (worker).
    /// Default: 127.0.0.1:9000 (SPEC-10 R5, SPEC-13 R44).
    pub bind: SocketAddr,

    /// Expected number of workers (relevant only for coordinator).
    pub num_workers: u32,

    /// Maximum accepted payload size, in bytes.
    /// Default: DEFAULT_MAX_PAYLOAD_SIZE (256 MiB).
    pub max_payload_size: u32,

    /// Timeout for waiting for all workers to connect (coordinator).
    /// Default: 120 seconds.
    pub worker_connect_timeout: Duration,

    /// Timeout for distributing partitions in a round (SHOULD).
    /// Default: 60 seconds.
    pub distribute_timeout: Duration,

    /// Timeout for collecting results in a round (MUST).
    /// Default: 600 seconds.
    pub collect_timeout: Duration,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:9000".parse().unwrap(),
            num_workers: 1,
            max_payload_size: DEFAULT_MAX_PAYLOAD_SIZE,
            worker_connect_timeout: Duration::from_secs(120),
            distribute_timeout: Duration::from_secs(60),
            collect_timeout: Duration::from_secs(600),
        }
    }
}
```

### 4.6 Coordinator Pseudocode

> **Note on state names:** The pseudocode below uses SPEC-06's original state names for readability. The authoritative state names are defined in SPEC-13 R19. See the mapping in Section 3.6.

The coordinator orchestrates the grid loop in distributed mode. The difference from the local grid loop (SPEC-05, Section 4.4) is in Phase 2: instead of reducing locally in-process, the coordinator serializes and sends each partition to a remote worker, awaits results, and reconstructs the partitions for merge.

```
                   +=====================+
                   |  WaitingForWorkers  |
                   | (accept + register) |
                   +=========+==========+
                             |
                             | All N workers registered
                             v
                   +=====================+
            +----->| CheckTermination   |
            |      | (check redexes)    |
            |      +=========+==========+
            |                |                      |
            |       +--------+--------+             |
            |       |                 |             |
            |  Has redexes      Normal Form        |
            |       |                 |             |
            |       v                 v             |
            |  +==========+   +==============+     |
            |  |Partitioning| |    Done      |     |
            |  +=====+=====+  | (ShutdownAll)|     |
            |        |        +=======+======+     |
            |        v                |             |
            |  +==========+          |             |
            |  |Dispatching|         |             |
            |  | (send_frame |       |             |
            |  |  x N)      |       |             |
            |  +=====+=====+        |             |
            |        |                             |
            |        | All sent                    |
            |        v                             |
            |  +================+                  |
            |  |WaitingForResults|                 |
            |  | (recv_frame     |                 |
            |  |  x N)           |                 |
            |  +=====+===========+                 |
            |        |                             |
            |        | All received                |
            |        v                             |
            |  +==========+                        |
            |  | Merging   |                       |
            |  | (merge +  |                       |
            |  | reduce_all)|                      |
            |  +=====+=====+                       |
            |        |                             |
            +--------+                             |
```

**Pseudocode for the distributed coordinator:**

```
async fn run_coordinator(
    net: Net,
    config: &NodeConfig,
    grid_config: &GridConfig,
    strategy: &dyn PartitionStrategy,
) -> Result<(Net, GridMetrics), ProtocolError>:

    // PHASE 0: Accept worker connections + registration handshake
    let listener = TcpListener::bind(config.bind).await?
    let mut worker_streams: Vec<TcpStream> = Vec::new()

    with_timeout(config.worker_connect_timeout):
        while worker_streams.len() < config.num_workers as usize:
            let (stream, addr) = listener.accept().await?
            tracing::info!("Worker connected from {}", addr)

            // Registration handshake (tier-dependent, R2a):
            // If auth is enabled (Tier 2/3), expect Register message
            // and validate token per SPEC-10 R14-R17.
            // If no auth (Tier 1), registration is implicit.
            // See SPEC-10 Section 4.8 for the full acceptance flow.
            if auth_enabled:
                let (msg, _) = recv_frame(&mut stream, config.max_payload_size).await?
                match msg:
                    Message::Register(payload):
                        if !validate_token(payload.auth_token):
                            send_frame(&mut stream, &Message::RegisterNack(
                                RegisterNackPayload { reason: "authentication failed".into() }
                            )).await?
                            stream.shutdown().await?
                            continue
                        let worker_id = worker_streams.len() as WorkerId
                        send_frame(&mut stream, &Message::RegisterAck(
                            RegisterAckPayload { worker_id }
                        )).await?
                    _:
                        stream.shutdown().await?
                        continue

            worker_streams.push(stream)

    // GRID LOOP (distributed)
    let mut current_net = net
    let mut metrics = GridMetrics::default()
    let start_time = Instant::now()

    loop:
        // Check Normal Form
        drain_stale_redexes(&mut current_net)
        if current_net.redex_queue.is_empty():
            metrics.converged = true
            break

        // Check round limit
        if let Some(max) = grid_config.max_rounds:
            if metrics.rounds >= max:
                metrics.converged = false
                break

        metrics.agents_per_round.push(count_live_agents(&current_net))

        // === PHASE 1: PARTITION ===
        let t_partition = Instant::now()
        let plan = partition(&current_net, config.num_workers, strategy)
        metrics.partition_time_per_round.push(t_partition.elapsed())

        // === PHASE 2a: DISTRIBUTE (send partitions) ===
        let t_send = Instant::now()
        let mut bytes_sent: usize = 0

        // Send concurrently to all workers (R21)
        let send_futures = plan.partitions.iter().zip(worker_streams.iter_mut())
            .map(|(partition, stream)| async {
                let msg = Message::AssignPartition {
                    round: metrics.rounds,
                    partition: partition.clone(),
                }
                send_frame(stream, &msg).await
            })
        let send_results = join_all(send_futures).await  // tokio::join or futures::join_all
        for result in send_results:
            bytes_sent += result?

        metrics.network_send_time_per_round.push(t_send.elapsed())
        metrics.bytes_sent_per_round.push(bytes_sent)

        // === PHASE 2b: COLLECT (receive results) ===
        // Note: concurrent collection via FuturesUnordered is permitted by R22
        // and may improve performance when workers have unbalanced reduction times.
        let t_recv = Instant::now()
        let mut bytes_received: usize = 0
        let mut reduced_partitions: Vec<Partition> = Vec::with_capacity(config.num_workers as usize)
        let mut worker_stats: Vec<WorkerRoundStats> = Vec::new()

        // Collect from all workers (may be concurrent)
        with_timeout(config.collect_timeout):
            for stream in &mut worker_streams:
                let (msg, nbytes) = recv_frame(stream, config.max_payload_size).await?
                bytes_received += nbytes
                match msg:
                    Message::PartitionResult { round, partition, stats }:
                        assert_eq!(round, metrics.rounds)
                        reduced_partitions.push(partition)
                        worker_stats.push(stats)
                    Message::Error { worker_id, round, description }:
                        return Err(ProtocolError::UnexpectedMessage {
                            expected: "PartitionResult",
                            received: format!("Error from worker {} in round {}: {}", worker_id, round, description),
                        })
                    _:
                        return Err(ProtocolError::UnexpectedMessage {
                            expected: "PartitionResult",
                            received: format!("{:?}", msg),
                        })

        metrics.network_recv_time_per_round.push(t_recv.elapsed())
        metrics.bytes_received_per_round.push(bytes_received)
        metrics.worker_stats_per_round.push(worker_stats)

        // === PHASE 3: MERGE + RESOLVE BORDERS ===
        let t_merge = Instant::now()
        let (mut merged_net, border_redex_count) = merge(reduced_partitions, &plan.borders)
        metrics.border_redexes_per_round.push(border_redex_count)

        let local_interactions: u64 = metrics.worker_stats_per_round.last().unwrap()
            .iter().map(|s| s.local_redexes as u64).sum()
        metrics.local_interactions_per_round.push(local_interactions)

        let border_interactions = reduce_all(&mut merged_net)
        metrics.merge_time_per_round.push(t_merge.elapsed())
        metrics.border_interactions_per_round.push(border_interactions as u64)
        metrics.total_interactions += local_interactions + border_interactions as u64

        current_net = merged_net
        metrics.rounds += 1

    // SHUTDOWN
    for stream in &mut worker_streams:
        let _ = send_frame(stream, &Message::Shutdown).await

    metrics.total_time = start_time.elapsed()
    Ok((current_net, metrics))
```

### 4.7 Worker Pseudocode

> **Note on state names:** The pseudocode below uses SPEC-06's original state names for readability. The authoritative state names are defined in SPEC-13 R24. See the mapping in Section 3.6.

```
async fn run_worker(config: &NodeConfig) -> Result<(), ProtocolError>:

    // Connect with retry
    let mut stream = connect_with_retry(config).await?

    // WORKER LOOP
    loop:
        let (msg, _nbytes) = recv_frame(&mut stream, config.max_payload_size).await?

        match msg:
            Message::AssignPartition { round, mut partition }:
                tracing::info!("Round {}: received partition worker_id={}", round, partition.worker_id)

                // Local reduction with timing (SPEC-11 Section 4.4)
                let agents_before = count_live_agents(&partition.subnet)
                let t_reduce = Instant::now()
                let (interactions, interactions_by_rule) = reduce_all_with_rule_counts(&mut partition.subnet)
                let reduce_duration = t_reduce.elapsed()
                let agents_after = count_live_agents(&partition.subnet)

                // Reconstruct free_port_index (SPEC-05, Section 4.3)
                partition.free_port_index = rebuild_free_port_index(&partition.subnet)

                // Send result with full 6-field WorkerRoundStats
                // (SPEC-05 R37 canonical + SPEC-11 Section 4.4 extension)
                let stats = WorkerRoundStats {
                    worker_id: partition.worker_id,
                    agents_before,
                    agents_after,
                    local_redexes: interactions,
                    reduce_duration_secs: reduce_duration.as_secs_f64(),
                    interactions_by_rule,
                }
                let result_msg = Message::PartitionResult {
                    round,
                    partition,
                    stats,
                }
                send_frame(&mut stream, &result_msg).await?

            Message::Shutdown:
                tracing::info!("Received shutdown, terminating worker.")
                break

            _:
                return Err(ProtocolError::UnexpectedMessage {
                    expected: "AssignPartition or Shutdown",
                    received: format!("{:?}", msg),
                })

    Ok(())
```

> **Note on `reduce_all_with_rule_counts`:** The `interactions_by_rule` field requires the reduction engine to return per-rule interaction counts. SPEC-03 defines `reduce_all` returning the total interaction count. The worker pseudocode assumes an extended variant `reduce_all_with_rule_counts` that also returns `[u64; 6]` indexed by rule ordinal (0=CON-CON, 1=CON-DUP, 2=CON-ERA, 3=DUP-DUP, 4=DUP-ERA, 5=ERA-ERA). The implementer MAY implement this as a separate function or as an option on `reduce_all`. This is an implementation decision left to the ENGINEER.

### 4.8 Connect with Retry Function

```rust
/// Connects to the coordinator with exponential backoff.
///
/// Parameters:
/// - config: NodeConfig with coordinator address (config.bind).
///
/// Backoff: 1s, 2s, 4s, 8s, 16s, 16s, 16s, 16s, 16s, 16s (10 attempts).
/// Identical to connectWithRetry in the Haskell prototype (AC-003, lines 331-352).
///
/// Returns: connected TcpStream, or ProtocolError after 10 attempts.
pub async fn connect_with_retry(config: &NodeConfig) -> Result<TcpStream, ProtocolError>
```

**Pseudocode:**

```
async fn connect_with_retry(config) -> Result<TcpStream>:
    let max_attempts = 10
    let mut delay = Duration::from_secs(1)
    let max_delay = Duration::from_secs(16)

    for attempt in 1..=max_attempts:
        match TcpStream::connect(config.bind).await:
            Ok(stream):
                tracing::info!("Connected to coordinator on attempt {}", attempt)
                return Ok(stream)
            Err(e):
                tracing::warn!("Attempt {}/{} failed: {}. Retrying in {:?}",
                    attempt, max_attempts, e, delay)
                tokio::time::sleep(delay).await
                delay = min(delay * 2, max_delay)

    Err(ProtocolError::ConnectionLost(io::Error::new(
        io::ErrorKind::ConnectionRefused,
        format!("Failed to connect after {} attempts", max_attempts),
    )))
```

### 4.9 Serialized Message Size Estimates (bincode)

Based on the analysis from DISC-006 v2 Section 1.3 and AC-003 (size tables), the following table estimates the serialized size with bincode for Relativist types:

| Type | Fields | Bincode size (bytes) | Comparison with Haskell [Int] |
|------|--------|---------------------|-------------------------------|
| Symbol | u8 enum | 1 | 4 (4x smaller) |
| AgentId | u32 | 4 | 4 (equal) |
| PortId | u8 | 1 | 4 (4x smaller) |
| PortRef::AgentPort | tag(u32) + AgentId + PortId | 9 | 12 (1.3x smaller) |
| PortRef::FreePort | tag(u32) + u32 | 8 | 8 (equal) |
| Agent | Symbol + AgentId | 5 | 8 (1.6x smaller) |
| Net (A agents, P ports) | Vec\<Option\<Agent\>\> + Vec\<PortRef\> + VecDeque + u32 | ~9A + 9P + 8Q + 12 | ~(8A + 24W + 8) |
| Partition | Net + WorkerId + HashMap + IdRange | Net + 4 + index + 8 | Net + 4 |

**Note:** These size estimates assume bincode configured with `bincode::config::standard().with_little_endian().with_fixed_int_encoding()` per R11. With this configuration, enum tags consume 4 bytes (u32). With varint encoding (bincode v2 default), tags < 128 consume 1 byte, which would reduce sizes slightly.

**Comparison with HVM2 (AC-006, AC-015 CC-7):** HVM2 uses an even more compact encoding: Port = u32 (4 bytes) with tag embedded in 3 bits via `(val << 3) | tag`, Pair = u64 (8 bytes). This format eliminates parsing overhead (direct memcpy) but requires changing the in-memory representation (SPEC-02). Relativist v1 does not adopt this format because serde+bincode already provides acceptable overhead and greater flexibility for type evolution. However, this encoding is noted as a viable optimization path for v2 (see Section 5.1).

### 4.10 Integration with GridMetrics

The `GridMetrics` defined in SPEC-05 (Section 4.1) already provides fields for network metrics (SPEC-05 R36). This spec concretizes those fields:

```rust
/// Extension of GridMetrics with network metrics.
/// These fields are populated by the coordinator in distributed mode.
/// In local mode (simulation without network), they remain as empty Vecs.
impl GridMetrics {
    // Fields already defined in SPEC-05:
    // pub network_send_time_per_round: Vec<Duration>,
    // pub network_recv_time_per_round: Vec<Duration>,
    // pub bytes_sent_per_round: Vec<usize>,
    // pub bytes_received_per_round: Vec<usize>,

    /// Returns the total bytes transferred across all rounds (sent + received).
    pub fn total_network_bytes(&self) -> usize {
        self.bytes_sent_per_round.iter().sum::<usize>()
            + self.bytes_received_per_round.iter().sum::<usize>()
    }

    /// Returns the communication overhead as a fraction of total time.
    /// Formula: (sum(send_time) + sum(recv_time)) / total_time
    /// Cf. DISC-006 v2, Section 1.1.
    pub fn network_overhead_fraction(&self) -> f64 {
        let send_total: Duration = self.network_send_time_per_round.iter().sum();
        let recv_total: Duration = self.network_recv_time_per_round.iter().sum();
        let network_total = send_total + recv_total;
        if self.total_time.is_zero() {
            0.0
        } else {
            network_total.as_secs_f64() / self.total_time.as_secs_f64()
        }
    }
}
```

### 4.11 Round Sequence Diagram

```
  Coordinator                        Worker 0                    Worker 1
      |                                 |                           |
      |<== TCP Connect =================|                           |
      |<== TCP Connect ===========================================>|
      |                                 |                           |
      |  [Tier 2/3 only: Registration handshake per SPEC-10]        |
      |<-- Register(token) -------------|                           |
      |--- RegisterAck(worker_id=0) --->|                           |
      |<-- Register(token) -------------|---------------------------|
      |--- RegisterAck(worker_id=1) ----|-------------------------->|
      |                                 |                           |
      |  [Grid Loop starts]             |                           |
      |                                 |                           |
      |--- AssignPartition(round=R) --->|                           |
      |--- AssignPartition(round=R) ----|-------------------------->|
      |                                 |                           |
      |                         [reduce_all]              [reduce_all]
      |                         [rebuild_fpi]             [rebuild_fpi]
      |                                 |                           |
      |<-- PartitionResult(round=R) ----|                           |
      |<-- PartitionResult(round=R) ----|---------------------------|
      |                                 |                           |
      |  [merge + reduce_all]           |                           |
      |  [update metrics]               |                           |
      |                                 |                           |
      |  (next round or shutdown)       |                           |
```

**Observations on the diagram:**

1. The registration handshake occurs once, before the grid loop. In Tier 1, the handshake is implicit (no Register/RegisterAck exchange).
2. The sending of `AssignPartition` is concurrent (R21): both workers receive the partition approximately at the same time.
3. Workers reduce in parallel (separate processes).
4. The collection of `PartitionResult` may be sequential or concurrent, at the implementer's discretion (R22).
5. Merge and border redex resolution occur on the coordinator.

### 4.12 Shutdown Protocol

The shutdown follows the same pattern as the Haskell prototype (AC-003): the coordinator sends an explicit `Shutdown` message to each worker.

```
  Coordinator                        Worker 0              Worker 1
      |                                 |                     |
      |-- Shutdown -------------------->|                     |
      |-- Shutdown ----------------------|-------------------->|
      |                                 |                     |
      |                         [close connection]    [close connection]
      |                                 |                     |
      | [close listener]                |                     |
```

**Note on metrics:** Shutdown messages are sent outside the grid loop and are NOT included in per-round metrics (`bytes_sent_per_round`, etc.). The `let _ =` pattern in the pseudocode (Section 4.6) is intentional: shutdown sends are best-effort, and errors are ignored since the grid loop has already completed.

**Contrast with the Haskell prototype:** The prototype uses an empty message (`[]`) as the shutdown signal (AC-003, `sendMessage(sock, [])` in `gridLoop`). Relativist uses an explicit enum variant (`Message::Shutdown`), which is clearer and type-safe.

---

## 5. Rationale

### 5.1 Bincode Instead of Custom Format

**Decision:** Use serde + bincode for message serialization.

**Rationale:** The technical decision to use serde + bincode was confirmed at the beginning of the project (SPEC-02, R24). The advantages over the `[Int]` format of the Haskell prototype (AC-003) are:

1. **Type safety:** serde guarantees that serialization and deserialization are consistent with Rust types at compile-time. The `[Int]` format in Haskell lost type information and used silent sentinels (`FreePort (-1)`) for errors.
2. **Compactness:** Bincode encodes each field at its natural size (u8 as 1 byte, u32 as 4 bytes), resulting in payloads ~50% smaller than `[Int]` (DISC-006 v2, Section 1.3).
3. **Zero boilerplate:** Deriving `Serialize`/`Deserialize` eliminates manual serialization/deserialization code (which in the Haskell prototype was ~100 lines in Protocol.hs).
4. **Performance:** Bincode is one of the fastest binary formats for Rust, with near-zero parsing overhead for simple types.

**Alternative considered:** HVM2-style format with Port = u32 tagged via `(val << 3) | tag` (AC-006, AC-015 CC-7). Rejected for v1 because: (a) it would require changing the in-memory representation of PortRef (SPEC-02), (b) the compactness gain (~30% over bincode) does not justify the additional complexity for the TCC scope, (c) bincode already halves the payload compared to the prototype. This format remains a viable optimization target for v2: if benchmarks (SPEC-09) reveal serialization as a bottleneck, the Port encoding can be changed to the HVM2-style u32 tagged representation without protocol-level changes (the frame format remains identical; only the payload encoding changes).

### 5.2 CRC32 Instead of Blind Trust in TCP

**Decision:** Include CRC32 checksum in each frame header.

**Rationale:** TCP guarantees reliable and ordered delivery via transport-layer checksums. However:

1. **Defense in depth:** Implementation errors (buffer overflow in send, off-by-one in recv) can corrupt data without violating TCP. CRC32 detects these errors at the application layer.
2. **Negligible cost:** CRC32C with hardware acceleration (SSE4.2) processes ~20 GB/s. For typical payloads (< 50 MB), the cost is < 3 ms, insignificant compared to TCP latency.
3. **Diagnosability:** A `ChecksumMismatch` is a clear, diagnosable error, whereas silently corrupted data would cause incorrect results that are difficult to trace.
4. **Standard practice:** Protocols such as gRPC (HTTP/2), Kafka, and CockroachDB use application-layer checksums over TCP.

**Alternative considered:** Trust TCP alone (no checksum). Rejected based on the defense-in-depth argument. The TCC operates in an ideal scenario, but the implementation should be robust enough to diagnose errors during development and testing.

### 5.3 Persistent Connections Instead of Per-Round Connections

**Decision:** Keep TCP connections open for the entire execution.

**Rationale:** The grid loop can execute multiple rounds (e.g., DualTree with 14 rounds). Opening and closing connections at each round would add TCP handshake latency (~3-way handshake: ~1 RTT) and `connectWithRetry` overhead. Persistent connections eliminate this cost. The Haskell prototype uses the same approach (AC-003: `runCoordinator` and `workerLoop` operate over the same socket throughout the grid loop).

### 5.4 Explicit Shutdown Instead of Empty Message

**Decision:** Use `Message::Shutdown` variant instead of empty payload.

**Rationale:** The Haskell prototype uses `sendMessage(sock, [])` as the shutdown signal (AC-003). This works because `[]` is not a valid `PartitionPlan`. However, it is a fragile convention: it depends on `deserializePartition([])` failing gracefully. Relativist uses a typed enum, making shutdown a first-class citizen of the protocol. This is more robust and self-documenting.

### 5.5 No Per-Message ACKs

**Decision:** No per-message acknowledgments.

**Rationale:** The protocol follows a strict request-response pattern: coordinator sends `AssignPartition`, worker responds with `PartitionResult`. There is no ambiguity about which message the receiver should send. Additional ACKs would increase latency (additional round-trip per message) without benefit in the failure-free scenario (TCC scope). If fault tolerance were needed (Z5), ACKs and retransmissions would be relevant but are out of scope.

### 5.6 Concurrent Partition Sending

**Decision:** The coordinator sends all partitions concurrently.

**Rationale:** Sending sequentially would mean the last worker starts reducing only after all previous workers have received their partitions. With N workers and latency L per send, the last worker would wait (N-1)*L before starting. Concurrent sending (via `tokio::join!` or `futures::join_all`) reduces this wait to ~L (all writes occur in parallel). The Haskell prototype sends sequentially (`forall (sock, part): sendMessage`, AC-003 lines 388-391), but Relativist improves upon this. Result collection may be sequential without significant penalty because workers finish at different times and `recv` simply waits for the next available message.

### 5.7 Overhead Analysis Summary (from DISC-006 v2 and ARG-004)

The per-round communication overhead can be decomposed into six phases (DISC-006 v2, Section 1.1; ARG-004, V2):

| Phase | Complexity | Expected cost |
|-------|-----------|---------------|
| Partitioning | O(A + W) | < 1% of round time |
| Serialization | O(A + W) per partition | Included in network send time |
| TCP transfer | O(bytes) + latency | 0.8% for EP 300K x 8w; dominates for small nets |
| ID remapping | **Eliminated** (SPEC-04 R19: static ID space partitioning) | 0% |
| Merge | O(sum(A_i) + sum(W_i) + B) | < 1% typical; ~21.5% for tree workloads |
| Border redex resolution | O(B_redex) reductions | 0% for EP; up to ~31% for DualTree |

The break-even condition for distributed reduction (ARG-004, Steps 1-2) is:

```
speedup > 1  iff  T_seq > max_i(T_worker_i) + R * T_overhead_per_round
```

Where `T_seq` is the sequential time, `max_i(T_worker_i)` is the slowest worker, `R` is the number of rounds, and `T_overhead_per_round` is the total overhead cost per round. Nets with high internal parallelism and few border wires (Profile A in ARG-004) achieve significant speedup; nets with sequential dependency and many border redexes (Profile C) experience slowdown.

---

## 6. Haskell Prototype Reference

### 6.1 IC.Protocol (Protocol.hs)

The IC.Protocol module defines:

- **`GridState`:** Enum with 7 constructors representing FSM states (AC-003). NOT used as an active runtime state machine -- exists as documentation. Relativist documents the FSM as specification (Sections 4.6, 4.7) with authoritative definitions in SPEC-13, requiring explicit enum-based implementation (SPEC-13 R22).

- **`[Int]` serialization:** Functions `serializeNet`, `serializePartition`, `serializePlan` convert types into integer lists (AC-003, "Serialization Format" section). Relativist replaces all of this with `#[derive(Serialize, Deserialize)]` and bincode, eliminating ~100 lines of manual code and reducing payload by ~50%.

- **`protocolRound`:** High-level function that executes one protocol round. In Relativist, the round is implemented directly in the coordinator's grid loop (Section 4.6).

### 6.2 IC.Network (Network.hs)

The IC.Network module implements:

- **`sendMessage`/`recvMessage`:** Length-prefixed framing with a 4-byte header (AC-003, lines 91-108). Relativist extends this to 8 bytes (4 length + 4 CRC32).

- **`recvExact`:** Loop to handle TCP fragmentation, limited to 65536 bytes per `recv` (AC-003, lines 81-89). Uses `BS.append acc chunk` with O(n^2) worst-case cost. Relativist uses `tokio::io::AsyncReadExt::read_exact`, which is implemented efficiently by the async runtime.

- **`connectWithRetry`:** Exponential backoff with 10 attempts and 16s maximum (AC-003, lines 331-352). Relativist replicates this behavior (Section 4.8).

- **`gridLoop` (coordinator):** Recursive loop that sends partitions sequentially, receives sequentially, remaps IDs, merges, and resolves borders (AC-003, lines 155-273). Relativist: (a) sends concurrently, (b) eliminates ID remapping (SPEC-04), (c) uses `reduce_all` after merge instead of selective resolution (SPEC-05).

- **`workerLoop`:** Recursive loop that receives a partition, reduces, sends the result, and repeats until receiving an empty message (AC-003, lines 355-379). Relativist: (a) receives typed `Message::Shutdown` instead of `[]`, (b) sends `WorkerRoundStats` along with the result.

### 6.3 Network Metrics in the Prototype

The prototype collects `bytesSent`, `bytesRecv`, `tSend`, `tRecv` per round (AC-003, "Coordinator Loop" section). Relativist collects the same data via `GridMetrics` (Section 4.10), with the addition of convenience methods (`total_network_bytes`, `network_overhead_fraction`).

---

## 7. Open Questions

1. **Payload compression.** For very large nets (> 10 MB per partition), compressing the payload with LZ4 or zstd before sending could significantly reduce transfer time. The current framing format allows this extension (compress between serialization and sending, decompress between receiving and deserialization). The decision to implement compression MUST be based on benchmarks (SPEC-09) and is left to the ENGINEER's discretion.

2. **Sequential vs concurrent result collection.** The pseudocode in Section 4.6 collects results sequentially (for loop). A concurrent implementation with `tokio::select!` or `FuturesUnordered` could allow starting to process one worker's result while awaiting others. The actual impact depends on how imbalanced reduction times are between workers.

3. **Protocol version negotiation.** If Relativist evolves to v2 with changes to the message format, a version handshake on initial connection will be necessary. In v1, the `protocol_version` field in `RegisterPayload` (SPEC-10) provides a basic version check. The extensibility of the `Message` enum (R5) partially mitigates this risk.

> **Resolved:** OQ-1 from Revised v2 (bincode fixed-int vs varint encoding) is now resolved: R11 explicitly specifies `bincode::config::standard().with_little_endian().with_fixed_int_encoding()`. The ENGINEER MAY benchmark varint encoding as an optimization, but fixed-int is the default for predictability.
