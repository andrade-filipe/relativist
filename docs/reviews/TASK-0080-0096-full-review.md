# Full Review: Phase 5 -- Wire Protocol (SPEC-06)

**Tasks:** TASK-0080 through TASK-0096
**Spec:** SPEC-06 (Revised v3)
**Date:** 2026-04-08
**Tests:** 57 tests across 6 files (all passing)
**Reviewer:** Development Pipeline Stages 4-6 + Stage 7 refactoring

---

## Stage 4: Code Cleaner

### MF (Must-Fix) Issues

**MF-01: `payload.len() as u32` truncation in `send_frame` (frame.rs:72)**
- `bincode::serialize()` returns a `Vec<u8>` whose `.len()` is `usize` (64-bit on x86_64).
  Casting to `u32` silently truncates payloads larger than 4 GiB. While `recv_frame` defends
  against oversized payloads via `max_payload_size`, `send_frame` had no corresponding guard
  on the outbound path. A serialized message exceeding u32::MAX would produce a corrupt frame
  with the wrong length in the header.
- **Status: FIXED.** Added `try_into()` with `PayloadTooLarge` error before the cast.

**MF-02: Unused import `RegisterPayload` in coordinator.rs (line 14)**
- Clippy `-D warnings` flags this as an error. `RegisterPayload` was imported in non-test code
  but only used inside `#[cfg(test)] mod tests`.
- **Status: FIXED.** Removed from the non-test import; added `use crate::protocol::types::RegisterPayload;`
  inside the test module.

### SF (Should-Fix) Issues

**SF-01: `total_interactions as usize` cast in worker.rs:137**
- `ReductionStats::total_interactions` is `u64`, `WorkerRoundStats::local_redexes` is `usize`.
  On 64-bit platforms this is lossless, but on hypothetical 32-bit targets it would truncate
  counts above ~4 billion.
- **Assessment:** Relativist targets 64-bit only (tokio async runtime, benchmark workloads).
  The risk is theoretical. A `try_into().expect()` or type change to `u64` in `WorkerRoundStats`
  would be cleaner, but touches `merge::types` which is outside `src/protocol/`.
- **Status: DOCUMENTED, not fixed.** The fix requires changing `WorkerRoundStats.local_redexes`
  from `usize` to `u64` in `src/merge/types.rs`, which is outside scope.

**SF-02: `unreachable!()` at end of `connect_with_retry` (worker.rs:62)**
- The for loop `1..=MAX_ATTEMPTS` always returns inside the loop body (either `Ok` on success
  or `Err` on the final attempt). The trailing `unreachable!()` is logically correct but
  introduces a panic path in non-test code. Replacing the for loop with a more idiomatic
  structure (or removing the `unreachable!()` and letting the compiler prove exhaustiveness)
  would be marginally safer.
- **Assessment:** The for loop logic is clear and correct. The `unreachable!()` documents intent.
  Risk is negligible.
- **Status: Not fixed (risk too low).**

### NTH (Nice-to-Have) Issues

**NTH-01: `"127.0.0.1:9000".parse().unwrap()` in NodeConfig::default() (config.rs:48)**
- The string is a compile-time constant, so `.parse()` will never fail. However, a `const`
  SocketAddr or `unwrap_or_else(|_| unreachable!())` with a comment would make the intent
  clearer.
- **Status: Not fixed.** This is a standard Rust pattern for constant addresses.

**NTH-02: No `PartialEq` derive on `Message` enum**
- The test comment in types.rs:172 notes "Can't use PartialEq (Net doesn't derive it)".
  This limits test expressiveness but is an upstream limitation (Net is in SPEC-02).
- **Status: Not applicable (upstream dependency).**

**NTH-03: `coordinator` and `worker` not re-exported from mod.rs**
- `mod.rs` re-exports `config::*`, `error::*`, `frame::*`, `types::*` but not `coordinator`
  or `worker`. This is intentional -- they are larger modules with many internal functions,
  and callers use `crate::protocol::coordinator::run_coordinator` explicitly.
- **Status: Correct as-is.**

### Naming and Idioms

- All public functions use `snake_case`, types use `PascalCase`, constants use `SCREAMING_SNAKE`.
- Function sizes are well-contained: `accept_workers` (~90 lines including nested async block),
  `run_coordinator` (~115 lines), `run_worker` (~97 lines), `send_frame`/`recv_frame` (~35 lines each).
- The coordinator's grid loop in `run_coordinator` is the longest function but follows a
  well-documented phase structure (Phase 1/2a/2b/3) matching the spec pseudocode.
- Error handling consistently uses `?` propagation with `map_err` for I/O errors.
- Ownership is clean: `send_frame` borrows `&Message`, `recv_frame` returns owned `Message`.
- Iterators are used appropriately (`worker_stats.iter().map(...)`, `zip`, `join_all`).

---

## Stage 5: Architecture Review

### Module Boundaries

The module structure matches the spec design (SPEC-06 Section 4):

| File | Responsibility | SPEC Section |
|------|---------------|-------------|
| `types.rs` | Message enum, payload structs | 4.1 |
| `error.rs` | ProtocolError enum | 4.4 |
| `frame.rs` | FrameHeader, send_frame, recv_frame | 4.2, 4.3 |
| `config.rs` | NodeConfig | 4.5 |
| `coordinator.rs` | accept_workers, distribute, collect, shutdown, run_coordinator | 4.6, 4.12 |
| `worker.rs` | connect_with_retry, run_worker | 4.7, 4.8 |

**SPEC-13 compliance note:** SPEC-13 R28-R29 require a `Transport` trait with `TcpTransport` and
`ChannelTransport` implementations. This is noted as deferred (TASK-0212) for Phase 6.
The current implementation uses raw `TcpStream` directly, which is correct for Phase 5 but will
need refactoring when the Transport abstraction is introduced.

### Dependency Direction

```
coordinator.rs --> frame.rs, types.rs, error.rs, config.rs
                   crate::merge, crate::partition, crate::reduction, crate::security
worker.rs -------> frame.rs, types.rs, error.rs, config.rs, coordinator.rs (PROTOCOL_VERSION)
                   crate::merge, crate::reduction, crate::security
frame.rs --------> types.rs, error.rs
config.rs -------> frame.rs (DEFAULT_MAX_PAYLOAD_SIZE)
types.rs --------> crate::merge, crate::partition
error.rs --------> (no protocol-internal deps)
```

Dependencies flow downward: coordinator/worker -> frame -> types/error. No circular dependencies.
External dependencies are on Core Layer modules (merge, partition, reduction, net) as required
by SPEC-13 R7.

### Design Patterns

1. **Generic I/O traits:** `send_frame`/`recv_frame` accept `AsyncWriteExt`/`AsyncReadExt`,
   enabling testing with `tokio::io::duplex` without TCP overhead.
2. **Timeout wrapping:** Both `accept_workers` and `collect_results` wrap their async block
   in `tokio::time::timeout()`, consistent with R30-R31.
3. **Concurrent send via join_all:** `distribute_partitions` sends all partitions in parallel
   using `futures::join_all`, satisfying R21.
4. **Best-effort shutdown:** `shutdown_workers` logs but doesn't propagate individual send
   failures, matching the spec's "best-effort" shutdown design (Section 4.12).

### Spec Compliance Matrix

| Requirement | Status | Evidence | Notes |
|------------|--------|----------|-------|
| **R1** Message enum | PASS | `types.rs`: 7-variant `Message` enum | |
| **R2** Coordinator->Worker messages | PASS | AssignPartition, Shutdown, RegisterAck, RegisterNack | |
| **R2a** Tier-dependent handshake | PASS | `accept_workers` handles Tier 1 (no auth) and Tier 2/3 (token) | |
| **R3** Worker->Coordinator messages | PASS | PartitionResult, Error, Register | |
| **R4** serde+bincode serialization | PASS | All types derive Serialize/Deserialize; bincode v1 used | |
| **R5** Extensibility (append-only) | PASS | Comment on enum documents this requirement | |
| **R6** 8-byte header framing | PASS | `FrameHeader` with length(u32 LE) + checksum(u32 LE) | |
| **R7** Length = exact payload bytes | PASS | `payload.len()` set as header length | |
| **R8** Receiver reads header, length, checksum, validates | PASS | `recv_frame` follows 6-step process per spec | |
| **R9** Max payload size, reject before allocation | PASS | Check at recv_frame line 121; sender guard added (MF-01) | |
| **R10** CRC32C via crc32fast | PASS | `crc32fast::hash(&payload)` | |
| **R11** bincode config | PASS/NOTE | bincode v1 `serialize`/`deserialize` uses fixed-int LE by default | R11 text references bincode v2 API but v1 is equivalent |
| **R12** All transitive types derive Serialize/Deserialize | PASS | Net, Partition, IdRange, PortRef, Agent, Symbol, WorkerRoundStats all derive | |
| **R13** Self-contained serialized format | PASS | bincode produces self-contained bytes | |
| **R14** Serialization identity | PASS | Round-trip tests in types.rs and frame.rs | |
| **R16** TCP transport | PASS | `TcpListener`/`TcpStream` used throughout | |
| **R17** Configurable listener address | PASS | `NodeConfig.bind`, default 127.0.0.1:9000 | |
| **R18** Worker connects via TCP | PASS | `connect_with_retry` in worker.rs | |
| **R19** Persistent connections | PASS | Streams stored in `Vec<TcpStream>`, reused across rounds | |
| **R20** Async I/O via tokio | PASS | All functions are `async`, use `tokio::net` | |
| **R21** Concurrent partition sending | PASS | `join_all` in `distribute_partitions` | |
| **R22** Await all results before merge | PASS | `collect_results` reads from all streams before returning | |
| **R23** Exponential backoff | PASS | `connect_with_retry`: 1s->2s->4s->8s->16s (capped), 10 attempts | |
| **R24** Wait for all workers + timeout | PASS | `accept_workers` with `tokio::time::timeout` | |
| **R25** Abort on connection loss | PASS | All I/O errors propagate as `ConnectionLost` | Note: no explicit shutdown of remaining workers on error |
| **R29** Checksum verification before deser | PASS | `recv_frame` checks CRC32 at step 5, before bincode deser at step 6 | |
| **R30** collect_timeout + distribute_timeout | PASS | Both implemented with `tokio::time::timeout` | |
| **R31** Timeout aborts grid loop | PASS | Timeout errors propagate through `?` to caller | |
| **R32** No per-message ACKs | PASS | Fire-and-wait pattern used | |
| **R33** Per-round network metrics | PASS | `bytes_sent/received_per_round`, `network_send/recv_time_per_round` | |
| **R34** Bytes include header | PASS | `send_frame` returns `FRAME_HEADER_SIZE + payload.len()` | |
| **R35** Metrics sufficient for overhead calc | PASS | `network_overhead_fraction()` method on GridMetrics | |
| **R36** NodeConfig fields and defaults | PASS | 6 fields, all defaults match spec | |
| **R38** tracing for communication logs | PASS | `tracing::info!`/`warn!` at appropriate points | |
| **R39** O(n) framing cost | PASS | CRC32 is O(n), header encode/decode is O(1) | |
| **R40** O(sum) total communication | PASS | No super-linear overhead | |

### Minor Spec Deviations (Non-Blocking)

1. **R25 partial:** The spec says "send Shutdown to all remaining connected workers (best-effort)
   on connection loss." The `run_coordinator` function propagates the error via `?` without
   explicitly shutting down remaining workers on error. This is acceptable in the failure-free
   scope (Z5) but should be addressed for robustness.

2. **R11 text vs implementation:** R11 references `bincode::config::standard().with_little_endian()
   .with_fixed_int_encoding()` which is a bincode v2 API. The code uses bincode v1's
   `serialize`/`deserialize` which defaults to LE fixed-int encoding. Functionally equivalent.

---

## Stage 6: QA Bug Hunt

### Panic Sources in Non-Test Code

| Location | Pattern | Risk | Assessment |
|----------|---------|------|-----------|
| config.rs:48 | `.parse().unwrap()` | Zero | Constant string "127.0.0.1:9000" always parses |
| worker.rs:62 | `unreachable!()` | Zero | Provably unreachable (for loop exhaustive) |
| frame.rs:72 | `as u32` truncation | **Was Medium** | **FIXED (MF-01):** Added `try_into()` guard |

### Logic Error Analysis

1. **Round number validation in `collect_results`** (coordinator.rs:206-209): If a worker
   echoes the wrong round number, the coordinator returns `UnexpectedMessage`. This is correct
   behavior per spec. No bug.

2. **Worker ID assignment** (coordinator.rs:100): `streams.len() as u32` is safe because
   `num_workers` is `u32` and the loop exits when `streams.len() == num_workers`. The cast
   cannot overflow.

3. **Metric accumulation** (coordinator.rs:356): `s.local_redexes as u64` casts `usize` to
   `u64`. On 64-bit, lossless. On 32-bit, also lossless (usize <= u64). Safe.

4. **`header.length as usize`** (frame.rs:129, 147): Casts `u32` to `usize`. On all supported
   platforms (64-bit), this is a widening cast. On 32-bit, u32 fits in usize. Safe.

### Concurrency Analysis

1. **`distribute_partitions` parallel sends:** Uses `join_all` on independent `send_frame`
   futures, each operating on a distinct `&mut TcpStream`. No shared mutable state. Safe.

2. **`collect_results` sequential reads:** Iterates `worker_streams.iter_mut()` sequentially.
   No concurrency issue. Note: R22 permits this to be concurrent, but sequential is correct.

3. **`accept_workers` single-threaded accept loop:** Accepts connections sequentially inside
   a timeout. No race conditions possible.

4. **`run_coordinator` single-threaded grid loop:** All phases execute sequentially within
   the coordinator task. Workers operate independently over TCP. No shared memory.

### IC-Specific Correctness

1. **Worker reduction:** `run_worker` calls `reduce_all(&mut partition.subnet)` and
   `rebuild_free_port_index(...)`, matching the spec pseudocode (Section 4.7). The
   `reduce_all` function guarantees confluence (SPEC-01 T1-T2), so the worker always
   produces a valid reduced partition.

2. **Border handling:** After merge, `run_coordinator` calls `reduce_all(&mut merged_net)`
   to resolve border redexes, consistent with SPEC-05's grid loop design.

3. **ID range preservation:** Partitions carry `id_range` through serialization/deserialization.
   bincode round-trip tests confirm `IdRange` fields are preserved.

### Untested Edge Cases

| Edge Case | Risk | Notes |
|-----------|------|-------|
| Payload exactly at `max_payload_size` boundary | Low | Tested: max_payload_size validation works, but no test for payload == max |
| Zero-length payload | Low | Not possible with bincode (enum discriminant is always > 0 bytes) |
| Worker sends `PartitionResult` for a future round | Low | `collect_results` rejects with round mismatch |
| Connection reset mid-frame (partial header) | Low | `read_exact` returns `ConnectionLost` on incomplete read |
| Multiple Register attempts from same worker | Medium | `accept_workers` handles this: rejected workers don't increment the stream count |
| Concurrent workers registering simultaneously | Low | TCP listener serializes accepts; no race |
| Very large net causing serialization > 256 MiB | Low | `recv_frame` rejects with `PayloadTooLarge`; `send_frame` now also guards (MF-01) |

---

## Stage 7: Refactoring Applied

### Changes Made

1. **frame.rs:** Added `try_into()` guard before `payload.len() as u32` cast in `send_frame`.
   Returns `ProtocolError::PayloadTooLarge` if payload exceeds u32::MAX. (MF-01)

2. **coordinator.rs:** Removed `RegisterPayload` from the non-test import on line 14. Added
   `use crate::protocol::types::RegisterPayload;` inside `#[cfg(test)] mod tests`. (MF-02)

### Verification

- All 57 protocol tests pass.
- 547 total library tests pass (5 known failures in `encoding::arithmetic` are pre-existing).
- No clippy warnings in `src/protocol/` files.
- Pre-existing clippy errors in `src/worker.rs` and `src/io/text_dsl.rs` are outside scope.

---

## Overall Assessment

The Phase 5 wire protocol implementation is **high quality**. It faithfully implements SPEC-06
with clean code organization, comprehensive tests (57 tests covering all message variants,
framing edge cases, and full distributed G1 integration), and proper error handling. The two
must-fix issues found (MF-01: truncation guard, MF-02: unused import) have been resolved.
The codebase is ready for Phase 6 (Transport abstraction per SPEC-13 R28-R29).
