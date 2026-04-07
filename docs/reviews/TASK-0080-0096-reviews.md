# Review: Phase 5 — Wire Protocol (SPEC-06)

**Tasks:** TASK-0080 through TASK-0096 (17 done, TASK-0212 deferred)
**Spec:** SPEC-06 (Revised v3)
**Date:** 2026-04-06
**Tests:** 55 new tests (341 -> 396)

---

## Summary

Implemented the complete wire protocol for distributed coordinator-worker communication. The protocol uses length-delimited framing with CRC32 checksums over TCP, with bincode serialization for all message types.

## Architecture

```
src/protocol/
  mod.rs          — Module facade with re-exports
  types.rs        — Message enum (7 variants), RegisterPayload/Ack/Nack structs
  error.rs        — ProtocolError enum (8 variants)
  frame.rs        — FrameHeader, send_frame, recv_frame, constants
  config.rs       — NodeConfig (bind, timeouts, payload limits)
  coordinator.rs  — accept_workers, distribute_partitions, collect_results,
                    shutdown_workers, run_coordinator
  worker.rs       — connect_with_retry, run_worker
```

## Key Design Decisions

1. **Message enum has 7 variants** (R1-R3): 4 core (AssignPartition, Shutdown, PartitionResult, Error) + 3 registration (Register, RegisterAck, RegisterNack). New variants MUST be appended for bincode discriminant stability (R5).

2. **8-byte framing** (R6): [4B length LE][4B CRC32 LE][payload]. CRC32C via `crc32fast` with hardware acceleration on x86.

3. **Generic async I/O** (R20): `send_frame` and `recv_frame` are generic over `AsyncWriteExt`/`AsyncReadExt`, enabling in-memory testing via `tokio::io::duplex`.

4. **Concurrent distribution** (R21): `distribute_partitions` uses `futures::join_all` to send all partitions in parallel, improving on the Haskell prototype's sequential sends.

5. **Size validation before allocation** (R9): `recv_frame` checks `length > max_payload_size` BEFORE calling `vec![0u8; length]` to prevent OOM attacks.

6. **GridMetrics network extensions** (R33-R35): Added `bytes_sent/received_per_round`, `network_send/recv_time_per_round`, plus `total_network_bytes()` and `network_overhead_fraction()` convenience methods.

## Spec Requirement Coverage

| Req | Status | Implementation |
|-----|--------|----------------|
| R1-R5 | DONE | Message enum with 7 variants, serde derives, extensibility note |
| R6-R10 | DONE | FrameHeader, 8-byte framing, CRC32C, configurable max payload |
| R11-R15 | DONE | bincode v1 serialize/deserialize, identity property tested |
| R16-R22 | DONE | TCP transport, persistent connections, concurrent sends |
| R23 | DONE | connect_with_retry: 10 attempts, 1s-16s exponential backoff |
| R24 | DONE | accept_workers with configurable timeout |
| R25 | DONE | Connection loss returns ProtocolError::ConnectionLost |
| R29 | DONE | Checksum verified before deserializing |
| R30-R31 | DONE | distribute_timeout (60s), collect_timeout (600s) |
| R33-R35 | DONE | Network metrics in GridMetrics + overhead fraction |
| R36-R37 | DONE | NodeConfig with 6 fields and defaults |
| R39-R40 | DONE | O(n) framing, O(sum) communication per round |

## Distributed G1 Tests

Three integration tests validate the Fundamental Property via real TCP:
- **test_g1_distributed_era_era**: ERA-ERA annihilation, 1 worker
- **test_g1_distributed_con_con_2_workers**: CON-CON annihilation, 2 workers
- **test_distributed_already_normal_form**: Empty net, 0 rounds, clean shutdown

## Deferred

- **TASK-0212** (SerializingChannelTransport): Requires `Transport` trait and `ChannelTransport` from SPEC-13. Will be implemented in Phase 6.

## Dependencies Added

- `futures = "0.3"` — for `join_all` in concurrent partition distribution
