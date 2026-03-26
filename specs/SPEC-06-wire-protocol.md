# SPEC-06: Wire Protocol

**Status:** Revised v2
**Depends on:** SPEC-00 (Glossary), SPEC-01 (Invariants), SPEC-02 (Net Representation), SPEC-04 (Partitioning), SPEC-05 (Merge and Grid Cycle)
**Gray zones resolved:** ---
**References consumed:** REF-001, REF-002, REF-003, REF-004, REF-013, REF-014
**Discussions consumed:** DISC-005 v2 (cross-boundary protocol, serialization format), DISC-006 v2 (communication overhead, granularity, bincode vs [Int] comparison), DISC-008 v2 (shared-to-distributed transition, serialization as operational cost)
**Arguments consumed:** ARG-004 (practical viability, overhead decomposition, break-even analysis)
**Code analyses consumed:** AC-003 (Haskell Protocol/Network: length-prefixed TCP, GridState FSM, sendMessage/recvMessage, connectWithRetry, gridLoop, workerLoop), AC-015 (CC-7 serialization of nets: Haskell [Int] vs HVM2 compact encoding)

---

## 1. Purpose

This spec defines the wire protocol for distributed communication between the coordinator and workers in Relativist when operating in grid mode over TCP. It covers: the message catalog, the TCP framing format, the serialization strategy (serde + bincode), the finite state machines (FSMs) of coordinator and worker, the integrity and timeout policies, the network metrics integrated into `GridMetrics` (SPEC-05), and the connection configuration. This spec transforms the local grid loop of SPEC-05 into a real distributed grid loop, where Phase 2 (local reduction) occurs in remote processes connected via TCP.

## 2. Definitions

Terms defined in SPEC-00 (Glossary) are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **Wire Protocol** | The binary protocol over TCP that defines how coordinator and workers exchange messages (partitions, results, status, shutdown). (Relativist) |
| **Frame** | A unit of transmission in the wire protocol: an 8-byte header (4 bytes length + 4 bytes checksum) followed by a variable-length payload. |
| **Length-Delimited Framing** | A framing technique where each message is preceded by its length in bytes, allowing the receiver to know exactly how many bytes to read. Used by the Haskell prototype (AC-003) with 4 bytes big-endian. Relativist uses 4 bytes little-endian (bincode convention). |
| **Coordinator** | The central process that orchestrates the grid cycle: partitions the net, sends partitions to workers, receives results, executes merge, and resolves border redexes. Corresponds to `runCoordinator` in the Haskell prototype (AC-003). |
| **Worker** | A remote process that receives a partition, reduces locally via `reduce_all` (SPEC-03), reconstructs the `free_port_index` (SPEC-05, Section 4.3), and returns the reduced partition to the coordinator. Corresponds to `workerLoop` in the Haskell prototype (AC-003). |
| **Persistent Connection** | A TCP connection kept open between the coordinator and each worker for the entire duration of the grid loop. Avoids TCP handshake overhead at each round. |

---

## 3. Requirements

### 3.1 Protocol Messages

**R1.** The protocol MUST define an enum `Message` with variants covering all communication between coordinator and workers. **(MUST)**

**R2.** The coordinator-to-worker messages MUST include at least:
- `AssignPartition`: sends a partition for local reduction.
- `Shutdown`: signals the worker to close its connection.
**(MUST)**

**R3.** The worker-to-coordinator messages MUST include at least:
- `PartitionResult`: returns the reduced partition.
- `Error`: reports an irrecoverable error during reduction.
**(MUST)**

**R4.** Every variant of the `Message` enum MUST be serializable and deserializable via serde + bincode (confirmed technical decision, SPEC-02 R24). **(MUST)**

**R5.** The `Message` enum SHOULD be extensible: adding new variants in future versions should not break deserialization of existing variants, provided the receiver discards unknown variants with a graceful error. **(SHOULD)**

### 3.2 Framing Format

**R6.** Each message transmitted over TCP MUST be framed with an 8-byte header: 4 bytes payload length (little-endian u32) followed by 4 bytes CRC32 checksum of the payload (little-endian u32). **(MUST)**

**R7.** The payload length in the header MUST represent the exact number of bytes of the bincode-serialized payload that follows the header. **(MUST)**

**R8.** The receiver MUST read exactly the 8 header bytes, extract the length and checksum, read exactly `length` bytes of payload, verify the checksum before deserializing, and reject the message if the checksum does not match. **(MUST)**

**R9.** The maximum payload size MUST be configurable, with a default of 256 MiB (`268_435_456` bytes). Messages with a declared length above the maximum MUST be rejected before allocating memory. **(MUST)**

**R10.** The checksum MUST use CRC32 (CRC32C/Castagnoli variant, available via crate `crc32fast`). CRC32C is chosen because: (a) it detects common transmission errors in TCP (bit flips, truncation), (b) it has hardware acceleration on x86 (SSE4.2 `crc32` instruction), (c) its computational overhead is negligible compared to bincode serialization. **(MUST)**

### 3.3 Serialization

**R11.** The serialization of each `Message` payload MUST use bincode with default configuration (little-endian, fixed-int encoding). **(MUST)**

**R12.** All types contained in `Message` MUST derive `serde::Serialize` and `serde::Deserialize`. This includes `Net`, `Partition`, `IdRange`, `PortRef`, `Agent`, `Symbol`, and `WorkerRoundStats` (types defined in SPEC-02 and SPEC-04). **(MUST)**

**R13.** The serialized format MUST be self-contained: a receiver with the same type schema MUST be able to reconstruct the complete message from the received bytes. **(MUST)**

**R14.** Serialization MUST preserve identity: `deserialize(serialize(msg)) == msg` for every valid message `msg`. **(MUST)**

**R15.** The bincode format SHOULD result in payloads approximately 50% smaller than the `[Int]` format of the Haskell prototype (AC-003: ~2x overhead), since bincode encodes Symbol as 1 byte (u8), PortId as 1 byte (u8), and AgentId as 4 bytes (u32), as analyzed in DISC-006 v2 Section 1.3. **(SHOULD)**

### 3.4 TCP Transport

**R16.** The transport MUST use TCP as the network protocol, with persistent connections between coordinator and each worker. **(MUST)**

**R17.** The coordinator MUST open a TCP listener socket on a configurable port and wait for connections from workers. **(MUST)**

**R18.** Each worker MUST connect to the coordinator via TCP upon startup, using host and port configured via CLI or configuration file. **(MUST)**

**R19.** Connections MUST remain open for the entire duration of the grid loop. A connection is only closed after the coordinator sends `Shutdown` or after an irrecoverable error. **(MUST)**

**R20.** All network I/O MUST be asynchronous, using `tokio` as the async runtime (confirmed technical decision). **(MUST)**

**R21.** The coordinator MUST send partitions to workers concurrently (not sequentially): all `AssignPartition` writes MUST be initiated before awaiting any `PartitionResult`. **(MUST)**

**R22.** The coordinator MUST await `PartitionResult` from all workers before proceeding to merge. The collection of results MAY be concurrent (each recv in parallel) or sequential (recv one by one). **(MUST for awaiting all; MAY for ordering)**

### 3.5 Connection and Retry

**R23.** Each worker MUST implement retry with exponential backoff to connect to the coordinator. The backoff MUST start at 1 second and double at each attempt, up to a maximum of 16 seconds, with at most 10 attempts. This behavior is identical to `connectWithRetry` in the Haskell prototype (AC-003, lines 331-352). **(MUST)**

**R24.** The coordinator MUST wait for all `num_workers` workers to connect before starting the first round of the grid loop. A configurable timeout (default: 120 seconds) MUST abort execution if not all workers have connected in time. **(MUST)**

**R25.** If a connection with a worker is lost during execution, the coordinator MUST abort the grid loop and return an error. Fault tolerance is out of scope (OBJETIVO_TCC.md, Z5). **(MUST)**

### 3.6 Finite State Machines (FSM)

**R26.** The coordinator MUST operate according to the following FSM:

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

**(MUST)**

**R27.** Each worker MUST operate according to the following FSM:

| State | Description | Transition |
|-------|-------------|-----------|
| `Connecting` | Attempting to connect to coordinator | Connected -> `Idle` |
| `Idle` | Awaiting message from coordinator | Received `AssignPartition` -> `Reducing`; Received `Shutdown` -> `Done` |
| `Reducing` | Executing `reduce_all` on the partition | Reduction complete -> `Sending` |
| `Sending` | Sending `PartitionResult` to coordinator | Sent -> `Idle` |
| `Done` | Received shutdown, terminating | (terminal) |

**(MUST)**

**R28.** The coordinator and worker FSMs do NOT need to be implemented as explicit state machines with a state enum. The FSM MAY be implemented implicitly via control flow (if/else + loop), as in the Haskell prototype (AC-003, critical observation about `GridState`). The FSMs documented here serve as a specification of expected behavior, not an implementation prescription. **(MAY for explicit implementation)**

### 3.7 Integrity and Timeouts

**R29.** The CRC32 checksum (R10) MUST be verified by the receiver before deserializing the payload. If the checksum does not match, the message MUST be rejected with a `ChecksumMismatch` error. **(MUST)**

**R30.** The coordinator SHOULD impose a timeout per phase of the round, configurable with default values:
- `distribute_timeout`: 60 seconds (maximum time to send all partitions).
- `collect_timeout`: 600 seconds (maximum time to receive all results -- large nets may take a long time).
**(SHOULD)**

**R31.** If a timeout is exceeded, the coordinator MUST abort the grid loop and return an error. Retry of rounds is not implemented in v1 (out of scope: Z5). **(MUST for abort; SHOULD for default values)**

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

**R36.** The coordinator and workers MUST be configurable via a `NodeConfig` structure containing at least:
- `role: NodeRole` -- role of the node (Coordinator or Worker).
- `host: String` -- address for bind (coordinator) or connect (worker).
- `port: u16` -- TCP port.
- `num_workers: u32` -- expected number of workers (coordinator only).
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    // === Coordinator -> Worker ===

    /// Sends a partition for the worker to reduce locally.
    /// Sent in the coordinator's Distributing state.
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
    /// Sent in the coordinator's ShuttingDown state.
    Shutdown,

    // === Worker -> Coordinator ===

    /// Returns the reduced partition to the coordinator.
    /// Sent by the worker after completing reduce_all + rebuild_free_port_index.
    PartitionResult {
        /// Round number (echo of the value received in AssignPartition).
        round: u32,
        /// The partition with the locally-reduced sub-net and reconstructed free_port_index.
        partition: Partition,
        /// Local reduction statistics for this worker in this round.
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
}
```

**Note on extensibility:** Future versions may add variants such as `RequestStatus` (coordinator -> worker for diagnostics) and `StatusReport` (worker -> coordinator with heartbeat). These variants are omitted in v1 because the TCC scope assumes a failure-free scenario (OBJETIVO_TCC.md). The enum structure allows adding them without breaking compatibility.

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
        return Err(ProtocolError::PayloadTooLarge { declared: length, max: max_payload_size })

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

```rust
use std::time::Duration;

/// Possible errors in the wire protocol.
#[derive(Debug)]
pub enum ProtocolError {
    /// I/O error in TCP communication.
    Io(std::io::Error),

    /// Declared payload exceeds the maximum allowed size.
    PayloadTooLarge {
        declared: u32,
        max: u32,
    },

    /// CRC32 checksum of the payload does not match the header declaration.
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

    /// Worker reported an error.
    WorkerError {
        worker_id: WorkerId,
        round: u32,
        description: String,
    },

    /// Number of connected workers does not match the expected count.
    WorkerCountMismatch {
        expected: u32,
        connected: u32,
    },
}
```

### 4.5 Node Configuration

```rust
/// Role of a node in the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    Coordinator,
    Worker,
}

/// Configuration of a node (coordinator or worker).
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// Role of this node.
    pub role: NodeRole,

    /// Address for bind (coordinator) or connect (worker).
    pub host: String,

    /// TCP port.
    pub port: u16,

    /// Expected number of workers (relevant only for coordinator).
    pub num_workers: u32,

    /// Maximum accepted payload size, in bytes.
    /// Default: DEFAULT_MAX_PAYLOAD_SIZE (256 MiB).
    pub max_payload_size: u32,

    /// Timeout for waiting for all workers to connect (coordinator).
    /// Default: 120 seconds.
    pub worker_connect_timeout: Duration,

    /// Timeout for distributing partitions in a round.
    /// Default: 60 seconds.
    pub distribute_timeout: Duration,

    /// Timeout for collecting results in a round.
    /// Default: 600 seconds.
    pub collect_timeout: Duration,
}
```

### 4.6 Coordinator FSM

The coordinator orchestrates the grid loop in distributed mode. The difference from the local grid loop (SPEC-05, Section 4.4) is in Phase 2: instead of reducing locally in-process, the coordinator serializes and sends each partition to a remote worker, awaits results, and reconstructs the partitions for merge.

```
                   +=====================+
                   |  WaitingWorkers     |
                   | (accept connections)|
                   +=========+==========+
                             |
                             | All N workers connected
                             v
                   +=====================+
            +----->|       Idle          |
            |      | (check redexes)    |
            |      +=========+==========+
            |                |                      |
            |       +--------+--------+             |
            |       |                 |             |
            |  Has redexes      Normal Form        |
            |       |                 |             |
            |       v                 v             |
            |  +==========+   +==============+     |
            |  |Partitioning| | ShuttingDown |     |
            |  +=====+=====+  +=======+======+     |
            |        |                |             |
            |        v                | Shutdown sent
            |  +==========+          | to all
            |  |Distributing|        v             |
            |  | (send_frame |  +==========+       |
            |  |  x N)      |  |   Done    |       |
            |  +=====+=====+  +==========+        |
            |        |                             |
            |        | All sent                    |
            |        v                             |
            |  +=============+                     |
            |  |WaitingResults|                    |
            |  | (recv_frame  |                    |
            |  |  x N)        |                    |
            |  +=====+========+                    |
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

    // PHASE 0: Accept worker connections
    let listener = TcpListener::bind((config.host, config.port)).await?
    let mut worker_streams: Vec<TcpStream> = Vec::new()

    with_timeout(config.worker_connect_timeout):
        while worker_streams.len() < config.num_workers as usize:
            let (stream, addr) = listener.accept().await?
            tracing::info!("Worker {} connected from {}", worker_streams.len(), addr)
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

        // Send concurrently to all workers
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
                        return Err(ProtocolError::WorkerError { worker_id, round, description })
                    _:
                        return Err(ProtocolError::UnexpectedMessage { ... })

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

### 4.7 Worker FSM

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

                // Local reduction
                let agents_before = count_live_agents(&partition.subnet)
                let interactions = reduce_all(&mut partition.subnet)
                let agents_after = count_live_agents(&partition.subnet)

                // Reconstruct free_port_index (SPEC-05, Section 4.3)
                partition.free_port_index = rebuild_free_port_index(&partition.subnet)

                // Send result
                let stats = WorkerRoundStats {
                    worker_id: partition.worker_id,
                    agents_before,
                    agents_after,
                    local_redexes: interactions,
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

### 4.8 Connect with Retry Function

```rust
/// Connects to the coordinator with exponential backoff.
///
/// Parameters:
/// - config: NodeConfig with coordinator host and port.
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
        match TcpStream::connect((config.host, config.port)).await:
            Ok(stream):
                tracing::info!("Connected to coordinator on attempt {}", attempt)
                return Ok(stream)
            Err(e):
                tracing::warn!("Attempt {}/{} failed: {}. Retrying in {:?}",
                    attempt, max_attempts, e, delay)
                tokio::time::sleep(delay).await
                delay = min(delay * 2, max_delay)

    Err(ProtocolError::Io(io::Error::new(
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

**Note:** Exact sizes depend on the bincode configuration (fixed-int vs varint). With fixed-int encoding (recommended for predictability), the bincode enum tag consumes 4 bytes (u32). With varint, tags < 128 consume 1 byte. The table above assumes fixed-int.

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

1. The sending of `AssignPartition` is concurrent (R21): both workers receive the partition approximately at the same time.
2. Workers reduce in parallel (separate processes).
3. The collection of `PartitionResult` may be sequential or concurrent, at the implementer's discretion (R22).
4. Merge and border redex resolution occur on the coordinator.

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

- **`GridState`:** Enum with 7 constructors representing FSM states (AC-003). NOT used as an active runtime state machine -- exists as documentation. Relativist also documents the FSM as specification (Sections 4.6, 4.7), without requiring explicit implementation as a state enum (R28).

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

1. **Bincode fixed-int vs varint encoding.** The default bincode configuration uses fixed-int encoding (enum tags as u32). Varint encoding would reduce enum tags to 1 byte, shrinking the `Message` payload by ~3 bytes per enum. The decision depends on serialization/deserialization benchmarks and is left to the ENGINEER's discretion.

2. **Payload compression.** For very large nets (> 10 MB per partition), compressing the payload with LZ4 or zstd before sending could significantly reduce transfer time. The current framing format allows this extension (compress between serialization and sending, decompress between receiving and deserialization). The decision to implement compression MUST be based on benchmarks (SPEC-09) and is left to the ENGINEER's discretion.

3. **Sequential vs concurrent result collection.** The pseudocode in Section 4.6 collects results sequentially (for loop). A concurrent implementation with `tokio::select!` or `FuturesUnordered` could allow starting to process one worker's result while awaiting others. The actual impact depends on how imbalanced reduction times are between workers.

4. **Protocol version negotiation.** If Relativist evolves to v2 with changes to the message format, a version handshake on initial connection will be necessary. In v1, there is no need. The extensibility of the `Message` enum (R5) partially mitigates this risk.
