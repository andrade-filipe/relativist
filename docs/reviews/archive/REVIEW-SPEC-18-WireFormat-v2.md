# REVIEW: SPEC-18 §3.1-3.4 + §3.6 — Wire Format v2 (atomic break)

**Reviewed:** 2026-04-16
**Scope:** TASK-0343 (bincode v2), TASK-0344 (Compact PortRef), TASK-0345 (Frame header v2), TASK-0346 (LZ4 compression pipeline), TASK-0347 (PROTOCOL_VERSION 1 → 2). Together these constitute the atomic v2 wire break (item 2.23). §3.5 (rkyv R20-R27) and the SHOULD field R39 are explicitly deferred — see DEFERRED-WORK D-002 / D-003.
**Verdict:** **APPROVE WITH MUST-FIX** (two corrections required before QA).

---

## What was reviewed

| File | Lines added (approx) | Owner task |
|------|---------------------|------------|
| `relativist-core/Cargo.toml` | +2 deps (bincode 2 + serde feature; lz4_flex 0.11 safe-encode/safe-decode) | 0343, 0346 |
| `relativist-core/src/protocol/bincode_v2.rs` | NEW (~46 LoC inc. inline helpers) | 0343 |
| `relativist-core/src/net/types.rs` | manual `Serialize` / `Deserialize` for `PortRef` (+ 7 inline tests) | 0344 |
| `relativist-core/src/protocol/frame.rs` | +9-byte header, FLAG_*, send/recv pipeline rewrite (+24 inline tests across 0345/0346/0347) | 0345, 0346, 0347 |
| `relativist-core/src/protocol/compression.rs` | NEW (~115 LoC inc. 4 tests) | 0346 |
| `relativist-core/src/protocol/config.rs` | TransportConfig.compression_threshold | 0346 |
| `relativist-core/src/config.rs` | `--compression-threshold` flag on coordinator + worker (+ 3 CLI tests) | 0346 |
| `relativist-core/src/protocol/error.rs` | UnknownFlags, DecompressionFailed, VersionMismatch (+ 2 Display tests) | 0345, 0346, 0347 |
| `relativist-core/src/protocol/coordinator.rs` | PROTOCOL_VERSION = 2; nack reason rewrite (+ 3 spec tests) | 0347 |
| `relativist-core/src/protocol/worker.rs` | `parse_version_mismatch_nack`; VersionMismatch routing (+ 3 spec tests) | 0347 |

---

## Spec compliance matrix

| Req | Status | Evidence / notes |
|-----|--------|------------------|
| R1-R4 (bincode v2 migration) | ✅ | `Cargo.toml: bincode = { version = "2", features = ["serde"] }`. All 13 prior call sites routed through `bincode_v2::{encode, decode, decode_value}`. Discriminant ordering preserved on `Message`. |
| R5 (PortRef tag layout) | ✅ | `PORTREF_TAG_DISCONNECTED = 0xFF`, `_AGENT_PORT = 0x00`, `_FREE_PORT = 0x01`. Manual `serialize_tuple(N)` bypasses bincode v2's enum discriminant, exposing the raw tag byte. |
| R6 (DISCONNECTED = 1 byte) | ✅ | `net::types::tests` asserts `bytes.len() == 1` and `bytes[0] == 0xFF`. |
| R7 (round-trip) | ✅ | `portref_*_round_trip` covers all three variants. |
| R8 (size SHOULD ≤ 4 B) | ✅ | `agent_port_small_id_under_four_bytes` asserts 3-byte encoding for `AgentPort(0, 0)`. |
| R9 (LZ4 above threshold) | ✅ | `frame::tests::lz4_compresses_above_threshold` proves `FLAG_COMPRESSED` is set when payload ≥ threshold. |
| R10 (LZ4 lib choice + redundancy) | ✅ | `lz4_flex` chosen with `default-features = false`, `safe-encode`, `safe-decode` (no `unsafe`). Ratio ≥ 2× on redundant data verified in `compression::tests::lz4_ratio_ge_two_on_redundant_payload`. |
| R11 (`compression_threshold: usize`) | ✅ | Field on `TransportConfig` (named `TransportConfig` rather than spec's `TransportTuning` — naming preserved from existing SPEC-17 implementation; documented at struct level). |
| **R12 (CRC on uncompressed)** | ✅ | `recv_frame` decompresses *before* CRC check. `send_frame_with_threshold` computes CRC on `uncompressed` and compresses afterwards. Explicit invariant test `checksum_is_on_uncompressed_payload`. |
| R13 (DecompressionFailed) | ✅ | New variant; `recv_frame` maps `decompress_payload` errors via `.map_err(ProtocolError::DecompressionFailed)`. |
| R14 (9-byte header) | ✅ | `FRAME_HEADER_SIZE = 9`. `to_bytes`/`from_bytes` round-trip including flags. |
| R15 (flag bit layout) | ✅ | `FLAG_COMPRESSED = 0b01`, `FLAG_ARCHIVED = 0b10`, `FLAG_RESERVED = 0b1111_1100` form a mutually-exclusive partition of `0xFF` (asserted in `frame_v2_constants_partition_byte`). |
| R16 (length is on-wire size) | ✅ | `payload_len = payload.len()` after compression; receiver `read_exact(header.length)` from socket. |
| R17 (CRC excludes flags) | ✅ | Header bytes are not part of `crc32fast::hash(&uncompressed)`. |
| R18 (FrameHeader struct) | ✅ | Matches spec layout exactly. |
| R19 (UnknownFlags rejection) | ✅ | `recv_frame` rejects with `ProtocolError::UnknownFlags` *before* allocating payload buffer — verified in `frame_v2_unknown_flag_bit_rejected`. |
| R20-R27 (rkyv) | ⚠️ Deferred | DEFERRED-WORK D-002 → ROADMAP item 2.24. `FLAG_ARCHIVED` reserved on the wire but no encoder/decoder wired (intentional). |
| R28 (PROTOCOL_VERSION = 2) | ✅ | `coordinator.rs:31`. Canary test `protocol_version_is_two` guards against rollback during merge. |
| **R29 (nack reason literal)** | ⚠️ **DEVIATION → MUST-FIX #1** | Spec mandates `"protocol version mismatch: expected 2, got 1"`. Implementation emits `"protocol version mismatch: expected 2, received 1"`. Tests assert `"received"` so they pass, but the on-wire literal deviates. See Must-Fix #1 below. |
| R30 (worker terminates with diagnostic) | ✅ | Worker returns `ProtocolError::VersionMismatch { expected, received }`; `Display` includes both versions. Non-version nacks remain `AuthFailed` (preserves SPEC-10 contract). |
| R31 (no bridge) | ✅ | No fallback codepath; v1 and v2 cannot interoperate by construction. |
| R32 (full-pipeline round-trip) | ✅ | `v2_pipeline_round_trip_all_message_variants_uncompressed` and `..._compressed` exhaustively iterate all 7 `Message` variants through bincode v2 + 9-byte header + (optional) LZ4 + CRC. Compile-time `match` exhaustiveness ensures new variants must update the test. |
| R33 (round-trip via CompactSubnet) | ✅ | Inherited — `Partition`'s `subnet` field uses `CompactSubnet` serde wrapper (SPEC-04). Covered by the `AssignPartition` and `PartitionResult` cases of R32 tests. |
| R34 (PortRef round-trip) | ✅ | `portref_round_trip_t2_set` (TASK-0344). |
| R35 (ProtocolError variants) | ✅ Partial | `DecompressionFailed`, `UnknownFlags`, `VersionMismatch` added. `ArchiveValidationFailed` deferred with rkyv (D-002). |
| R36 (config fields) | ✅ Partial | `compression_threshold` shipped. `use_zero_copy` deferred with rkyv (D-002). |
| R37 (CLI flags) | ✅ Partial | `--compression-threshold` added to both coordinator and worker subcommands; threaded through `build_transport_config`. |
| R38 (`bytes_*_per_round` reflects wire) | ✅ | `send_frame_with_threshold` returns `FRAME_HEADER_SIZE + payload.len()` (post-compression). `recv_frame` returns `FRAME_HEADER_SIZE + header.length`. Coordinator's `distribute_partitions` and `collect_results` aggregate these directly. |
| **R39 (compression metrics SHOULD)** | ⚠️ Deferred | DEFERRED-WORK D-003 — `compression_ratio_per_round`, `compression_time_per_round`, `decompression_time_per_round` not yet on `WorkerRoundStats`. SPEC says SHOULD, not MUST; deferral is explicitly recorded. |
| R40-R41 (complexity) | ✅ | LZ4 throughput O(n) inherited from `lz4_flex` (>500 MB/s on commodity hardware per crate docs). Varint + PortRef compact O(1). |

---

## Code quality

**Strengths**
- **Zero unsafe.** `lz4_flex` selected with `default-features = false` plus explicit `safe-encode` and `safe-decode` features — verified by reading `Cargo.toml:41`.
- **Defense in depth:**
  - `decompress_payload` rejects declared sizes > 1 GiB (`MAX_DECOMPRESSED_SIZE`) before any allocation, blocking decompression-bomb amplification.
  - `recv_frame` rejects `FLAG_RESERVED` bits *before* allocating the payload buffer, blocking forward-incompatible frames from forcing OOM.
  - CRC32C is computed on the *uncompressed* payload (R12), so a wrong-but-valid LZ4 block followed by a CRC over compressed bytes cannot silently decode to garbage.
- **Backward-compat surface preserved:** `send_frame` is now a thin delegate to `send_frame_with_threshold`, so all existing call sites (coordinator, worker, channel transport tests) compile and behave identically with the default 1024-byte threshold.
- **Bincode v2 surface centralized:** single `bincode_v2` module wraps `bincode::config::standard()`. Future v3 upgrade has one touch point.
- **No `unwrap()` in production code paths.** All fallible operations propagate via `?`.
- **Tracing, not `println!`** consistently used across coordinator and worker.
- **Compile-time exhaustiveness** in `frame::tests::sample_all_message_variants`: a hidden `match` on `&Message` will refuse to build if a new variant is added without updating the test, preventing R32 coverage drift.

**Issues**

### Must-Fix #1 — R29 wire string deviates from spec literal

**File:** `relativist-core/src/protocol/coordinator.rs:69-77`

The spec mandates the literal nack reason `"protocol version mismatch: expected 2, got 1"` (R29). Implementation currently emits `"protocol version mismatch: expected 2, received 1"`. Three downstream effects:

1. The on-wire string deviates from the spec literal — any external monitoring, log scraper, or reference implementation that follows R29 verbatim will fail to match.
2. `worker::parse_version_mismatch_nack` parses on the substring `"received "`, so it would not recognise a spec-conformant nack from a different implementation.
3. The TASK-0347 R2 test in `coordinator_rejects_v1_worker_with_register_nack` asserts `"received 1"` — which makes the test internally consistent with the bug, but inconsistent with TEST-SPEC-0347 R2 which requires `"got 1"`.

**Fix (REFACTOR stage):**
- Change coordinator nack format to `"protocol version mismatch: expected {N}, got {M}"`.
- Change `parse_version_mismatch_nack` to look for `"got "` instead of `"received "`.
- Update the `coordinator_rejects_v1_worker_with_register_nack` and `worker_terminates_on_version_mismatch_nack` test assertions to match `"got 1"`.
- **Keep** `ProtocolError::VersionMismatch` field name as `received` and the Display impl as `"... received {M}"` — the field name documents the local variable, and TEST-SPEC-0347 R4 explicitly asserts `s.contains("received 1")`. Field name and wire string can legitimately differ.

### Must-Fix #2 — `recv_frame` doc-comment is stale

**File:** `relativist-core/src/protocol/frame.rs:189-199`

The doc-comment still describes the v1 pipeline:
```
/// Steps:
/// 1. Read exactly 8 header bytes.        ← actually 9
/// 2. Extract length and checksum from the header.   ← also flags now
/// 3. Reject if length > max_payload_size (defense against OOM).
/// 4. Read exactly `length` bytes of payload.
/// 5. Verify CRC32C of payload against header checksum.   ← decompression happens BEFORE this
/// 6. Deserialize payload with bincode -> Message.
```

The implementation correctly does the v2 sequence (header → unknown-flag check → payload read → decompress → CRC → decode), but the doc-comment lies about the contract. Future readers will misunderstand the R12 ordering invariant.

**Fix (REFACTOR stage):** Rewrite the step list to:
1. Read 9-byte header.
2. Reject if `flags & FLAG_RESERVED != 0` (R19) — *before* allocating.
3. Reject if `length > max_payload_size` (OOM defense).
4. Read exactly `length` bytes of (possibly compressed) payload.
5. If `FLAG_COMPRESSED`, LZ4-decompress (R13).
6. Verify CRC32C against the *uncompressed* payload (R12).
7. Deserialize via bincode v2.

**Should-Fix:** None.

**Nice-to-have (defer):**
- R29 spec text uses *"got"* but R35's error variant field name is *"received"*. The spec itself is mildly inconsistent here; consider an editorial clarification in SPEC-18 §3.6 to either reconcile the two terms or explicitly note that the wire string and the error field name use different words. Out of scope for this REVIEW; flag for the next spec-revision pass.

---

## Architecture review

- **Module boundaries respected.** `compression.rs` is a free-function module with zero coupling to `frame`/`Message`/`Net`. `bincode_v2.rs` exposes only thin wrappers over a serde-trait surface. Both can be unit-tested in isolation (and are).
- **Dependency direction inviolate.** `protocol/` still depends on `net/`, `partition/`, `merge/` — never the reverse. The new `protocol/compression.rs` and `protocol/bincode_v2.rs` are leaf modules within `protocol/`.
- **No new async surface in core layers.** `compress_payload` and `decompress_payload` are pure synchronous functions; the only `async` additions are in `frame.rs` (which is the appropriate layer per CLAUDE.md).
- **CLI flag wiring is symmetric** between `CoordinatorArgs` and `WorkerArgs` — same default (1024), same semantics. `LocalArgs` does not carry the flag because in-memory `ChannelTransport` paths skip framing.

---

## Test summary

| Task | Tests added | Cumulative |
|------|------------|------------|
| baseline | — | 690 |
| TASK-0343 (bincode v2) | indirect (existing tests re-validate) | 801 |
| TASK-0344 (Compact PortRef) | 7 | 808 |
| TASK-0345 (Frame header v2) | 7 | 815 |
| TASK-0346 (LZ4 pipeline) | 15 (9 spec + 4 compression + 1 Display + 1 default) | 830 |
| TASK-0347 (PROTOCOL_VERSION) | 9 (R1, R2, R3, R3-negative, R3-parser, R4, R5×2, R7) | **839** |

Plus 4 integration tests (`tests/cli_integration.rs`) — unchanged.

All gates clean:
- `cargo test --workspace` — 839 lib + 4 integration, 0 failed
- `cargo clippy --workspace --all-targets -- -D warnings` — clean
- `cargo fmt --check` — clean
- `cargo build --release` — clean
- Smoke: `target/release/relativist.exe compute add 3 5 → Result: 8`

---

## Adversarial probes for QA stage

The QA stage should specifically attempt these (most have not been exercised):

1. **Truncated PortRef body.** Send an `AgentPort` tag (`0x00`) followed by a one-byte truncated varint id. Verify `bincode::serde::decode_from_slice` returns `DecodeError::UnexpectedEnd`, surfaced as `ProtocolError::Deserialize` (not panic, not silent recovery).
2. **Compressed empty frame.** Send a frame with `FLAG_COMPRESSED` set and `length = 0`. `decompress_payload(&[])` should hit the size-prefix guard and return an error → `DecompressionFailed`.
3. **Both flags set: compressed + archived (`flags = 0x03`).** No reserved bit is set, so current `recv_frame` would proceed to bincode-decode an rkyv archive → guaranteed `Deserialize` error, but verify the failure mode is graceful (no panic, no partial state).
4. **Compression flag with wrong CRC.** Send a compressed payload and corrupt the CRC. Should yield `ChecksumMismatch` (not `DecompressionFailed`) because R12 sequencing decompresses first.
5. **`PROTOCOL_VERSION = 0` Register payload.** Spec only mentions `1` vs `2`; verify a `0` arriving against a v2 coordinator is also rejected with the canonical nack.
6. **Worker receives a `RegisterAck` first, then a `RegisterNack` later.** Worker is in the main loop reading `AssignPartition` — what happens if a stray nack arrives mid-stream? (Should surface as `UnexpectedMessage`.)
7. **CLI: `--compression-threshold 0`** — must compress every frame; verify `cargo run --release -- coordinator --compression-threshold 0 ...` works end-to-end with the smoke test.
8. **CLI: `--compression-threshold 18446744073709551615`** (`usize::MAX` literal) — must skip compression on every frame; verify smoke succeeds.
9. **Concurrent receive during coordinator-side rejection.** Two workers connect simultaneously; one is v1, one is v2. Confirm v1 gets nack and v2 gets ack, regardless of arrival order.

---

## Deferred work — recorded

The deferral is documented in three independent locations so it cannot be silently lost:
1. `docs/DEFERRED-WORK.md` — D-002 (rkyv portion R20-R27) and D-003 (compression metrics R39).
2. `docs/ROADMAP.md` — item 2.24 (zero-copy archive) carries the rkyv work.
3. `docs/pipeline-state.md` SPEC-18 entry — explicit `Deferred` block at the bottom of the §3.1-3.4 section.

---

## Conclusion

The atomic v2 wire break (bincode v2 + Compact PortRef + 9-byte frame header + LZ4 + PROTOCOL_VERSION bump) is correctly implemented end-to-end. The R12 CRC-on-uncompressed invariant is respected with an explicit test; the FLAG_RESERVED rejection is hardened against OOM amplification; the PortRef compact encoding hits the SHOULD size targets. Test count went 690 → 839 with zero regression.

Two Must-Fix items must be addressed in the REFACTOR stage:
- **#1:** Coordinator nack reason should use `"got"` (R29 literal), not `"received"`. Worker parser, two tests, and the test asserting wire string must be updated together; the `VersionMismatch` field name and Display are intentionally separate and must remain `"received"` to keep TEST-SPEC-0347 R4 satisfied.
- **#2:** `recv_frame` doc-comment must be rewritten to describe the v2 pipeline accurately, including the R12 ordering invariant.

Both are mechanical edits with no functional regression risk. After QA's adversarial pass and these two fixes, the bundle is ready to ship.

**APPROVE WITH MUST-FIX.**
