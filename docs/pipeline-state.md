# Pipeline State

**Last updated:** 2026-04-16 (SPEC-18 §3.5 / item 2.24 — **all 6 stages complete; bundle shipped at 905 default / 945 zero-copy tests, 0 bugs**)
**Maintained by:** sdd-pipeline agent (do not edit manually)

---

## Active Bundle

**Bundle:** SPEC-18 §3.5 (item 2.24) — Zero-Copy Archive (rkyv on hot path)
**Stage:** 5 of 6 — **QA COMPLETE** (verdict: 10 probes added / 10 PASS; 0 bugs). Stage 6 REFACTOR is no-op, ready to ship.
**Branch:** `v2-development`
**Test baseline:** 887 lib + 4 integration
**Current test counts:** 905 lib default (+18) / 945 lib `--features zero-copy` (+58)
**See:** "SPEC-18 §3.5 (item 2.24)" section below for full brief.

## Stage 5 QA Summary (SPEC-18 §3.5 / item 2.24)

- **Probes implemented:** 10 (Q1, Q2-on, Q2-off, Q3-on, Q3-off, Q4, Q5, Q6, Q7, Q8) per
  `docs/reviews/REVIEW-SPEC-18-section-3.5-2026-04-16.md` §7.
- **Location:**
  - Feature-gated (8 probes): `relativist-core/src/protocol/zero_copy_tests.rs` under
    `#[cfg(all(test, feature = "zero-copy"))]`.
  - Default-build (2 probes: Q2-off, Q3-off): `relativist-core/src/protocol/frame.rs`
    inside the existing `#[cfg(test)]` module under `#[cfg(not(feature = "zero-copy"))]`.
- **Q1 tightening:** Initial "mutate trailing 4 bytes" strategy landed on POD u32 fields
  rather than relative pointers (rkyv 0.8 places the root struct at the buffer end, so
  the tail bytes are fields, not the root RelPtr). Switched to a deterministic
  front-half truncation (`payload.drain(0..len/2)`) which moves the buffer base so every
  internal RelPtr now references pre-buffer memory — the rkyv validator rejects
  reliably (`ArchiveValidationFailed`).
- **Bug count:** 0. All 10 probes PASS.
- **Gate status:** `cargo test --workspace` GREEN (905+4), `cargo test --workspace
  --features zero-copy` GREEN (945+4), `cargo clippy --workspace --all-targets -- -D
  warnings` GREEN (both feature configs), `cargo fmt --check` GREEN.
- **Q6 status:** Intentional no-op traceability test (ARM alignment witness deferred —
  no ARM CI runner). Q7 documents rkyv 0.8.x cross-patch fixture coverage as deferred.
- **Stage 6 REFACTOR:** No-op (0 bugs). Ready to ship.

---

## Current Work

**Current spec:** SPEC-26 §3.1 (Cargo Workspace Restructure — Layer 0) — **COMPLETE**
**Current stage:** DONE (mechanical refactor, no functional changes)
**v2 branch:** v2-development
**v1 tests baseline:** 690 passing
**Final test count:** 716 (712 unit + 4 integration) — unchanged
**Clippy status:** clean (0 warnings, including --all-targets)
**Formatting:** clean (cargo fmt --check passes)

## Stage History (SPEC-26 §3.1)

- [x] SPLITTING: 2026-04-16 — 5 atomic tasks (workspace skeleton, move src/, thin CLI, verify, docs)
- [x] DEV: 2026-04-16 — workspace created (relativist-core + relativist-cli), src/tests/benches moved via git mv
- [x] VERIFY: 2026-04-16 — 716 tests pass, clippy --workspace --all-targets clean, fmt clean, release binary at target/release/relativist.exe (scripts + Dockerfile path preserved)
- [x] REVIEW: 2026-04-16 — refactor-only change, all 7 requirements (R1-R7) satisfied
- [x] QA: 2026-04-16 — smoke test `compute add 3 5 = 8` passes
- [x] REFACTOR: 2026-04-16 — fixed pre-existing clippy warning in protocol/config.rs (bool_assert_comparison)

## Layer 0 Verification (SPEC-26 R1-R7)

| Req | Status | Evidence |
|-----|--------|----------|
| R1 | ✅ | Layout: relativist/{Cargo.toml, relativist-core/, relativist-cli/} |
| R2 | ✅ | Workspace manifest with members + resolver = "2" |
| R3 | ✅ | All src/ files moved verbatim to relativist-core/src/ via git mv |
| R4 | ✅ | All deps + features (tls, metrics, otel, full) preserved in relativist-core/Cargo.toml |
| R5 | ✅ | relativist-cli/src/main.rs delegates to relativist_core::commands; binary name "relativist" preserved |
| R6 | ✅ | 716 tests pass in relativist-core; [[bench]] section in relativist-core/Cargo.toml |
| R7 | ✅ | `cargo test --workspace` runs all tests across both crates |

## Backward Compatibility

- Binary path unchanged: `target/release/relativist.exe`
- All 6 bench scripts (`scripts/bench_*.sh`) work without modification
- Dockerfile `COPY` line works unchanged
- CLI flags and subcommands identical to pre-restructure

## SPEC-27 Progress

**Phase 1: Traits — COMPLETE** (2026-04-16)
- `relativist_core::encoding::traits` module created (~150 LoC inc. tests)
- `Encoder`, `Decoder`, `Codec` traits + `EncodeError`, `DecodeError` types (R1-R4)
- `validate_encoded_net()` checks E1 (T1 linearity subset) + E2 (at least one redex) (R5, R6)
- 6 inline tests; test count 716 → 722 (+6)

**Phase 2: Church refactoring — COMPLETE** (2026-04-16, commit `a601c5e`)
- `ChurchArithmeticCodec` (4 ops: add/mul/exp/sum_of_squares) implements `Codec`
- Wraps existing `build_*` + `decode_nat_or_shared`; zero changes to public API (R7)
- 8 inline tests (round-trips, schema validation, object safety); test count 722 → 726 (+4)

**Phase 3: LambdaCodec — COMPLETE** (2026-04-16, all 6 stages)
- TASK-0333 (encoder), TASK-0334 (decoder), TASK-0335 (edge cases) created in `docs/backlog/`.
- TEST-SPEC-0333/0334/0335 specified in `docs/tests/`.
- `relativist-core/src/encoding/codec_lambda.rs` implements parser + pretty-printer +
  Mackie/Pinto encoder + port-directed readback decoder; ~900 LoC inc. 32 inline tests
  (26 spec'd + 6 QA adversarial).
- Engine fix: `interact_comm` self-loop guard added (mirrors `interact_anni` /
  `interact_eras`); required for sound reduction of identity-typed arguments
  (e.g., `(λx. x x) (λy. y)`). No regression on the 690 v1 tests.
- Final test count: 726 → 758 (+32). Clippy + fmt clean.
- REVIEW notes in `docs/reviews/REVIEW-SPEC-27-Phase3-LambdaCodec.md`.
- Stage 6 REFACTOR: no Must-Fix items from REVIEW. Should-Fix items
  (free-var name preservation, DUP-cycle readback algorithm) deferred as
  out of T5-T9 scope; will be re-evaluated when consumed by Phase 5 CLI.
- Driven by Option B: SDD pipeline managed manually from TCC root
  (relativist-local agents not invocable from TCC cwd).

**Phase 4: EncoderRegistry — COMPLETE** (2026-04-16, all 6 stages)
- TASK-0336 (registry struct + ops, R17/R18/R20), TASK-0337 (default_registry, R19) created.
- TEST-SPEC-0336 (8 cases R1-R8) and TEST-SPEC-0337 (5 cases D1-D5) specified.
- `relativist-core/src/encoding/registry.rs` (~310 LoC inc. 19 inline tests):
  - `EncoderRegistry` (HashMap<String, Box<dyn Codec>>) with 6 ops: new, register,
    get, list (sorted), encode_and_validate (calls validate_encoded_net), decode.
  - `RegistryError` enum (DuplicateName, NotFound, Encode #[from], Decode #[from]).
  - `default_registry()` registering 4 ChurchArithmeticCodec ops + LambdaCodec.
  - 13 spec-driven tests + 6 QA adversarial tests (case-sensitive lookup,
    failed-register doesn't poison, empty input, NotFound preserves name,
    Default impl, no-redex net rejected by E2).
- Re-exported from `encoding/mod.rs` as `EncoderRegistry`, `RegistryError`,
  `default_registry`.
- Note: round-trip test for `church_exp` reduced to encode-only because decode
  hits the SPEC-14 DUP-cycle readback limitation (pre-existing, not caused by
  this work). add/mul/sum_of_squares fully round-trip.
- Final test count: 758 → 777 (+19). Clippy + fmt clean.
- REVIEW notes in `docs/reviews/REVIEW-SPEC-27-Phase4-EncoderRegistry.md` (APPROVE).
- Stage 5 QA: 6 adversarial tests added, all passed first try — no Must-Fix items.
- Stage 6 REFACTOR: no code changes needed (zero bugs found in QA).

**Phase 5: CLI integration — COMPLETE** (2026-04-16, all 6 stages)
- TASK-0338 (compute --encoder dispatch, R21/R23), TASK-0339 (encoders list, R22) created.
- TEST-SPEC-0338 (5 cases C1-C5) and TEST-SPEC-0339 (3 cases E1-E3) specified.
- `relativist-core/src/config.rs`: ComputeArgs extended (operation/a/b → Option,
  added `--encoder` and `--input` with clap `requires`); `Encoders(EncodersArgs)`
  variant with `EncodersAction::List` subcommand added.
- `relativist-core/src/commands.rs`:
  - `run_compute_command` early-returns through `run_compute_with_encoder` when
    `--encoder` is set; legacy SPEC-14 path validates positional args at runtime
    and is otherwise unchanged.
  - `run_compute_with_encoder` runs the full SPEC-27 R23 pipeline:
    `encode_and_validate → reduce_all → decode → serde_json pretty-print`.
  - `run_encoders_command` enumerates `default_registry().list()` aligned with
    the longest name (deterministic, sorted output).
- `relativist-core/src/error.rs`: added `RelativistError::Encoding(String)` and
  `From<RegistryError>` impl so `?` propagates registry errors with exit code 3.
- `relativist-cli/src/main.rs`: `Command::Encoders(args)` dispatch wired.
- Smoke (release binary): `compute add 3 5 → Result: 8`;
  `compute --encoder church_add --input '{"a":3,"b":5}' → {"result":8}`;
  `compute --encoder lambda --input '{"term":"(λx. x) (λy. y)"}' → λv0. v0`;
  unknown encoder → exit 3 with clear message; `encoders list` → 5 codecs.
- Final test count: 777 → 789 (+12: 4 spec-driven C/E + 4 QA arg-parsing + 4
  dispatch unit tests). Clippy + fmt clean.
- REVIEW notes in `docs/reviews/REVIEW-SPEC-27-Phase5-CLI.md` (APPROVE).
- Stage 5 QA: 8 adversarial tests added (parsing edge cases + dispatch error
  paths), all passed first try — no Must-Fix items.
- Stage 6 REFACTOR: no code changes needed.

**Phase 6: RecipeEncoder mínimo (R24+R25 only) — COMPLETE** (2026-04-16, all 6 stages)
- TASK-0340 created with explicit "minimal scope" note: ships only R24 (trait
  definition) and R25 (non-coupling with the `Codec` registry). R26/R27/R28
  deferred until SPEC-25 (item 2.29, milestone M7) is implemented.
- TEST-SPEC-0340 (R1-R8) specified.
- `relativist-core/src/encoding/recipe.rs` (~250 LoC inc. 12 inline tests):
  - `RecipeEncoder: Encoder` trait with `type Recipe: Serialize + DeserializeOwned + Send + Sync`
    and 3 methods (`is_decomposable`, `make_recipes`, `generate_partition`).
  - `MinimalRecipe` struct + `MinimalRecipeEncoder` demo implementation (Con/Era pairs).
  - Demo encoder is **not** registered in `default_registry()` — it exists only to
    exercise the trait in tests.
- 8 spec-driven tests + 4 QA adversarial tests (more-workers-than-size, zero-pair
  recipe, Send+Sync compile-check, centralized-encode validation).
- Final test count: 789 → **801** (+12). Clippy + fmt clean.
- REVIEW notes in `docs/reviews/REVIEW-SPEC-27-Phase6-RecipeEncoder.md` (APPROVE).
- Stage 5 QA: 4 adversarial tests added, all passed first try — no Must-Fix items.
- Stage 6 REFACTOR: no code changes needed.
- **Deferral tracker:** see `docs/DEFERRED-WORK.md` row D-001 for the unblock
  checklist that must be processed when SPEC-25 ships in milestone M7.

**SPEC-27 status:** Phases 1-5 fully complete. Phase 6 partially complete
(R24+R25 shipped; R26-R28 deferred to M7). M10 milestone in V2-FEATURE-MATRIX
remains open until D-001 is resolved.

**Next focus (per V2-FEATURE-MATRIX Tier 1 break-even path):**
- ~~Item 2.22 — TCP Transport Tuning~~ DONE (commit c360fe5, 2026-04-15)
- ~~Item 2.25 — Unix Domain Socket Fast Path~~ DONE (commit c360fe5, 2026-04-15)
- **Item 2.23 — Wire Format v2 (M1, ~450 LoC, 2-3d) — IN PROGRESS**
- Item 2.34 — Coordinator-Free Round (next after 2.23)
- Item 2.24 — rkyv zero-copy (deferred per DEFERRED-WORK D-002, sequenced after 2.34)
- Decision point after M1: measure tcp_localhost/seq ratio and decide on M3-M4.

---

## SPEC-18 §3.1-3.4 + §3.6 (Wire Format v2, item 2.23) — IN PROGRESS

**Started:** 2026-04-16
**Test baseline before this work:** 801

### Stage History

- [x] **SPLITTING** (2026-04-16): 5 atomic tasks created in `docs/backlog/`
  following SPEC-18 §6.3 mandated migration order.
  - TASK-0343 — bincode v2 migration (M, ~150 LoC, R1-R4)
  - TASK-0344 — Compact PortRef encoding (S, ~150 LoC, R5-R8) depends on 0343
  - TASK-0345 — Frame header v2 (S, ~150 LoC, R14-R19) depends on 0343
  - TASK-0346 — LZ4 compression pipeline (M, ~180 LoC, R9-R13, R36-R39) depends on 0345
  - TASK-0347 — PROTOCOL_VERSION bump 1→2 (S, ~80 LoC, R28-R32) depends on 0343-0346
- [x] **TESTS** (2026-04-16): 5 TEST-SPEC files created in `docs/tests/`
  specifying acceptance criteria for each task before implementation.
  - TEST-SPEC-0343, 0344, 0345, 0346, 0347 — all enumerate R-numbered
    requirements with concrete `#[test]` blueprints.
  - Cumulative test target: 801 → 819+ across the 5 tasks.
- [~] **DEV** — TDD RED → GREEN → REFACTOR per task, in dependency order.
  - [x] **TASK-0343 (bincode v2 migration) — GREEN** (2026-04-16)
    - Cargo.toml: `bincode = "1"` → `bincode = { version = "2", features = ["serde"] }`
    - Created `src/protocol/bincode_v2.rs` thin wrapper module
      (`encode`, `decode`, `decode_value` over `bincode::config::standard()`).
    - Migrated 13 call sites across `protocol/{frame,types,error}.rs`,
      `net/{core,types}.rs`, `partition/{types,compact}.rs`,
      `merge/types.rs`, `io/binary.rs`.
    - `ProtocolError::Serialize`/`Deserialize` switched from `bincode::Error`
      to `bincode::error::{EncodeError, DecodeError}`.
    - 801 lib + 4 integration tests pass (no regression). Clippy clean,
      fmt clean. Release smoke `compute add 3 5 → 8` works.
  - [x] **TASK-0344 (Compact PortRef encoding) — GREEN** (2026-04-16)
    - Removed `serde::{Serialize,Deserialize}` derive from `PortRef`;
      added manual impls using `serialize_tuple(N)` / `deserialize_tuple(3, …)`
      to bypass bincode v2's enum discriminant overhead.
    - Tag bytes (public): `PORTREF_TAG_DISCONNECTED = 0xFF`,
      `PORTREF_TAG_AGENT_PORT = 0x00`, `PORTREF_TAG_FREE_PORT = 0x01`.
      Wire shapes: DISCONNECTED = 1 B; FreePort(b) = 1 + varint(b);
      AgentPort(id, pid) = 1 + varint(id) + varint(pid).
    - 7 spec-driven tests added in `net::types::tests` (R1-R7):
      round-trip on T2 set; DISCONNECTED = 1 byte; AgentPort(0,0) = 3 bytes;
      small AgentPort ≤ 4 bytes; FreePort(0) = 2 bytes; unknown tag rejected;
      truncated payload rejected.
    - 801 → **808** lib tests + 4 integration tests (no regression).
      Clippy clean, fmt clean. Release smoke `compute add 3 5 → 8` works.
  - [x] **TASK-0345 (Frame header v2) — GREEN** (2026-04-16)
    - `FRAME_HEADER_SIZE` 8 → **9** bytes (added `flags: u8`).
    - New public consts: `FLAG_COMPRESSED = 0b01`, `FLAG_ARCHIVED = 0b10`,
      `FLAG_RESERVED = 0b1111_1100` (mutually exclusive partition of 0xFF).
    - `FrameHeader` gains helpers `is_compressed()`, `is_archived()`,
      `has_unknown_flags()`. `to_bytes()` / `from_bytes()` round-trip the flags.
    - `send_frame` always writes `flags = 0` (compression bit lights up in
      TASK-0346, archive bit deferred to ROADMAP item 2.24).
    - `recv_frame` rejects any frame with `flags & FLAG_RESERVED != 0`
      via new `ProtocolError::UnknownFlags { flags }` BEFORE allocating the
      payload buffer (forward-compat hardening per SPEC-18 R19).
    - 7 spec-driven tests added (R1-R7): size const, flag-const partition,
      flags=0 round-trip, helper truth tables, reserved-bit rejection,
      `UnknownFlags` Display, header-byte regression check.
    - Existing tests updated (not deleted): `test_frame_header_byte_order`
      now asserts the 9-byte LE layout including flags; `test_framing_constants`
      asserts `FRAME_HEADER_SIZE == 9`; all `FrameHeader { … }` literals
      gained `flags: 0`.
    - 808 → **815** lib tests + 4 integration tests (no regression).
      Clippy clean, fmt clean. Release smoke `compute add 3 5 → 8` works.
  - [x] **TASK-0346 (LZ4 compression pipeline) — GREEN** (2026-04-16)
    - Added `lz4_flex = "0.11"` (default-features off, safe-encode +
      safe-decode features) — pure-Rust, no `unsafe`.
    - New module `protocol/compression.rs` (~115 LoC inc. 4 tests):
      `compress_payload(&[u8]) -> Vec<u8>` writes a `[u32 LE size]
      [LZ4 block]` envelope; `decompress_payload(&[u8])` rejects
      truncated buffers and declared sizes >1 GiB before allocating.
    - `protocol/error.rs`: new `DecompressionFailed(String)` variant
      (Display: `"LZ4 decompression failed: <reason>"`).
    - `protocol/config.rs`: `TransportConfig.compression_threshold:
      usize` (default 1024) — `usize::MAX` disables, `0` always
      compresses.
    - `protocol/frame.rs`:
      - New `send_frame_with_threshold(writer, msg, threshold)`. The
        bare `send_frame` now delegates with
        `DEFAULT_COMPRESSION_THRESHOLD = 1024`.
      - `recv_frame` decompresses BEFORE the CRC check so the checksum
        verifies the uncompressed message bytes (R12 invariant).
    - `config.rs`: `--compression-threshold <BYTES>` CLI flag on both
      `coordinator` and `worker` subcommands; threaded through
      `build_transport_config` into `TransportConfig`.
    - 9 spec-driven tests added (R1, R2, R3, R4, R5, R6, R7, R8 ×2, R9):
      LZ4 round-trip, ratio ≥ 2× on redundant data, above/below
      threshold, CRC-on-uncompressed invariant, corrupted payload
      yields `DecompressionFailed | ChecksumMismatch`, default-threshold
      behaviour for both `send_frame` and `TransportConfig::default`,
      and CLI-flag round-trip on coordinator + worker.
    - Existing test count 815 → **830** (+15: 9 spec + 4 new in
      `compression.rs` + 1 error Display + 1 default const).
      Clippy clean, fmt clean. Release smoke `compute add 3 5 → 8` works.
    - **R10 deferral:** `compression_ratio_per_round` field on
      `WorkerRoundStats` (R39 SHOULD) deliberately not added in this
      task — would require coordinator-side aggregation work that the
      atomic wire-break does not need. Recorded as DEFERRED-WORK D-003
      (track for SPEC-11 metrics evolution).
  - [x] **TASK-0347 (PROTOCOL_VERSION bump 1 → 2) — GREEN** (2026-04-16)
    - `protocol/coordinator.rs`: `PROTOCOL_VERSION` bumped 1 → 2 (R28).
      Nack reason on version mismatch now uses the canonical phrase
      `"protocol version mismatch: expected N, received M"` so workers
      can parse it deterministically (R29).
    - `protocol/error.rs`: new `VersionMismatch { expected: u8,
      received: u8 }` variant (R30); Display renders
      `"protocol version mismatch: expected N, received M"` (R4).
    - `protocol/worker.rs`: new private `parse_version_mismatch_nack()`
      helper extracts the received-version `u8` from a nack reason; the
      `RegisterNack` arm in `run_worker_inner` now returns
      `VersionMismatch` for version-shaped nacks and `AuthFailed` for
      everything else (R30, preserves SPEC-10 contract).
    - 7 spec-driven tests added (R1, R2, R3, R3-negative, R3-parser,
      R4, R5×2, R7):
      - `coordinator::tests::protocol_version_is_two` (R1 canary)
      - `coordinator::tests::coordinator_rejects_v1_worker_with_register_nack` (R2)
      - `coordinator::tests::smoke_v2_coordinator_v2_worker_handshake_succeeds` (R7)
      - `worker::tests::worker_terminates_on_version_mismatch_nack` (R3)
      - `worker::tests::worker_returns_auth_failed_for_non_version_nack` (R3 negative)
      - `worker::tests::parse_version_mismatch_nack_recognises_canonical_phrase` (R3 unit)
      - `error::tests::version_mismatch_error_renders` (R4)
      - `frame::tests::v2_pipeline_round_trip_all_message_variants_uncompressed` (R5 — all 7 variants through bincode v2 + 9-byte header + CRC, threshold = MAX)
      - `frame::tests::v2_pipeline_round_trip_all_message_variants_compressed` (R5 — all 7 variants through bincode v2 + LZ4 + 9-byte header + CRC, threshold = 0)
      - Compile-time exhaustive `match` in `sample_all_message_variants` so adding a new `Message` variant breaks the test.
    - Test count 830 → **839** lib tests + 4 integration tests
      (no regression). Clippy clean, fmt clean. Release smoke
      `compute add 3 5 → 8` works.
    - **R6 deferral:** raw-TCP integration test with hand-crafted v1
      bytes (`legacy_v1_register_bytes`) deliberately omitted — the v1
      wire format used bincode v1 + 8-byte header, neither of which
      this build can emit, and the spec explicitly permits gating it
      under `#[ignore]` and relying on R2's ChannelTransport coverage
      for the contract. Recorded as in-spec deferral, no follow-up needed.
- [x] **REVIEW** (2026-04-16) — full-bundle pass over TASK-0343..0347 in
      `docs/reviews/REVIEW-SPEC-18-WireFormat-v2.md`. Verdict: **APPROVE
      WITH MUST-FIX** (2 mechanical edits queued for REFACTOR).
- [x] **QA** (2026-04-16) — 9 adversarial probes from the REVIEW added as
      `qa_probe_*` tests:
      - `frame::tests::qa_probe_1_truncated_portref_body_yields_deserialize_error`
      - `frame::tests::qa_probe_2_compressed_empty_frame_yields_decompression_failed`
      - `frame::tests::qa_probe_3_both_flags_set_archive_bit_currently_ignored`
      - `frame::tests::qa_probe_4_compression_flag_with_corrupted_crc_yields_checksum_mismatch`
      - `coordinator::tests::qa_probe_5_v0_register_rejected_with_canonical_nack`
      - `worker::tests::qa_probe_6_stray_nack_mid_stream_surfaces_unexpected_message`
      - `frame::tests::qa_probe_7_threshold_zero_compresses_minimal_message`
        + `config::tests::qa_probe_7_cli_threshold_zero_threads_through_coordinator_and_worker`
      - `frame::tests::qa_probe_8_threshold_usize_max_skips_compression_on_large_message`
        + `config::tests::qa_probe_8_cli_threshold_usize_max_threads_through_coordinator_and_worker`
      - `coordinator::tests::qa_probe_9_v1_then_v2_workers_v1_nacked_v2_acked`
      Test count 839 → **850** lib + 4 integration. Clippy clean
      (one `needless_range_loop` fixed during the pass), fmt clean,
      release smoke `compute add 3 5 → 8` works. **No new bugs surfaced** —
      every failure mode probed (truncation, decompression bombs, stray
      flag bits, CRC tampering, version edge cases, mid-stream message
      ordering, CLI extremes, concurrent v1/v2 registration) is handled
      gracefully via typed `ProtocolError` variants.
- [x] **REFACTOR** (2026-04-16) — Both REVIEW Must-Fix items applied:
      - **#1 (R29 wire literal):** `coordinator::accept_workers` nack
        format changed from `"… received {M}"` to `"… got {M}"` (matches
        SPEC-18 R29 verbatim). `worker::parse_version_mismatch_nack` now
        keys on `"got "` instead of `"received "`. The
        `ProtocolError::VersionMismatch { received: u8 }` field name and
        the Display impl `"… received N"` are intentionally kept (R35,
        TEST-SPEC-0347 R4 — wire string and Rust field name use different
        terms by design). Test fixtures and assertions in
        `coordinator_rejects_v1_worker_with_register_nack`,
        `qa_probe_5_v0_register_rejected_with_canonical_nack`,
        `worker_terminates_on_version_mismatch_nack`,
        `parse_version_mismatch_nack_recognises_canonical_phrase`, and
        the R5 `sample_all_message_variants` fixture were updated
        together to keep the suite internally consistent.
      - **#2 (`recv_frame` doc-comment):** rewritten to the v2 7-step
        sequence (header → unknown-flag check → length check → payload
        read → decompress → CRC → bincode v2 decode), explicitly noting
        the load-bearing R12 ordering invariant.
      Final gate: 850 lib + 4 integration tests pass, clippy clean, fmt
      clean, release build clean, smoke `compute add 3 5 → 8` works.
      **SPEC-18 §3.1-3.4 + §3.6 (item 2.23) ships green.**

### Deferred (already tracked)

- §3.5 (rkyv zero-copy archive, R20-R27) — DEFERRED-WORK row D-002,
  sequenced after item 2.34 in M1.

---

## SPEC-19 §3.1 (item 2.34) — Coordinator-Free Round (Merge Avoidance) — SHIPPED 2026-04-16

**Started:** 2026-04-16
**Test baseline before this work:** 850 lib + 4 integration (post-SPEC-18 ship)
**Bundle scope:** SPEC-19 §3.1 only — requirements R1, R2, R3, R4, R5, R6, R7.
  Explicitly **OUT of scope** for this bundle: §3.2 BorderGraph (item 2.35),
  §3.3 full Delta-Only Protocol (item 2.26), §3.4-§3.7 (delta wire variants,
  invariant amendments, delta config, delta metrics).
**Estimated size:** ~300 LoC across atomic tasks (<200 LoC each).
**Tier 1 break-even path:** confirmed next after item 2.23 in
  `docs/V2-FEATURE-MATRIX.md` (Implementation order:
  2.22 → 2.23 → **2.34** → 2.24 → 2.25 → 2.35 → 2.26).
**Dependencies:** none beyond what already shipped (strict BSP from v1,
  v1 wire protocol, v2 wire format from SPEC-18).

### Bundle Goal (single sentence)

Detect when **all** workers report `has_border_activity: false` after a round
of local reduction and skip the merge-redistribute cycle for that round; if
they additionally report `local_redexes == 0`, declare Global Normal Form
(R4) and terminate. Stay compatible with both v1 protocol and v2 wire
format (R7) — the `has_border_activity` field is a **new addition to the
worker round result that crosses the wire**, so a bincode v2
message-format change is in scope for this bundle and MUST NOT break any
existing serde round-trip tests (TASK-0343..0347 baseline).

### Stage History

- [x] **SPLITTING** (2026-04-16): complete — 4 atomic tasks delivered to
      `docs/backlog/`, all <200 LoC, ~300 LoC total, every task documents
      v1+v2 protocol compatibility per R7. Bundle index file:
      `docs/backlog/SPEC-19-section-3.1-coordinator-free-round-tasks.md`.
  - TASK-0348 — Add `has_border_activity` field + `compute_border_activity` helper (S, ~80 LoC, R1+R2)
  - TASK-0349 — Populate `has_border_activity` at every WorkerRoundStats build site (S, ~50 LoC, R2) depends on 0348
  - TASK-0350 — Add `coordinator_free_rounds` config flag + metrics counter (S, ~40 LoC, R6+R41 partial+R45 partial)
  - TASK-0351 — Coordinator skip-merge logic + Global Normal Form termination (M, ~130 LoC, R3+R4+R5+R6+R7) depends on 0348+0349+0350
  - DAG: 0348 → 0349 ‖ 0350 → 0351. Cumulative test target 850 → ~865+.
- [x] **TESTS** (2026-04-16): complete — 4 TEST-SPEC files delivered to
      `docs/tests/`, one per task, each enumerating concrete `#[test]`
      blueprints with target file paths, exact inputs/expected outputs,
      and explicit SPEC-19 R-number coverage.
  - `docs/tests/TEST-SPEC-0348.md` — 6 new tests (UT-01..05 cover
    `compute_border_activity` against R1; UT-06 covers
    `WorkerRoundStats` bincode v2 round-trip carrier for R2).
    Adversarial probes A-D referenced as QA candidates (large-N scan,
    `AgentId(u32::MAX)`, full v2 wire pipeline carrier, legacy bytes).
  - `docs/tests/TEST-SPEC-0349.md` — 4 new tests (UT-01/02 positive +
    negative round-build for R2; UT-03 ordering check —
    `compute_border_activity` after `rebuild_free_port_index`;
    IT-04 `run_grid` per-worker activity propagation end-to-end).
    Adversarial probes A-E referenced as QA candidates (oscillation,
    race window flagged non-deterministic, full v2 wire ride, empty
    workers list, single worker).
  - `docs/tests/TEST-SPEC-0350.md` — 2 new tests (UT-01 default flag
    `false` per R43 SHOULD; UT-02 metric counter starts at 0 per R45p).
    Cross-task probes A-C reference TEST-SPEC-0351 for the gating /
    increment behaviour.
  - `docs/tests/TEST-SPEC-0351.md` — 8+ new tests: UT-01..04 exhaustive
    2×2 truth table for `check_global_normal_form` (R3+R4); UT-05
    skip-merge under strict BSP (R3+R6); UT-06 GNF early termination
    (R4); UT-07 default config bit-identical v1 behaviour (R7); UT-08
    lenient mode does not skip (R6 SHOULD interpretation); UT-09
    structural assertion that `protocol/coordinator.rs` FSM is untouched
    by the bundle (R7 wire compat); UT-10 / UT-11 result-equivalence
    on skip-triggered AND skip-NOT-triggered workloads (R5 confluence);
    UT-12 G1 spot check on `church_add(2, 3)` at w=2 strict BSP.
    Adversarial probes A-G referenced as QA candidates (empty / single
    worker, oscillation, lenient + flag combo, `max_rounds = 0`,
    `ep_annihilation_con(100)` per spec T11, async race flagged as
    out-of-bundle for item 2.26).
  - **Cumulative bundle test target:** 850 → **870** lib tests (+20),
    with conservative floor **866+** to absorb implementation
    negotiation. Inside the orchestrator's `+12 to +18` hint.
  - **Hard-to-write-deterministically tests flagged:** UT-0351-09 is
    a source-grep structural assertion (acceptable but flagged for
    future migration to a parsed-AST check); QA-0349-B / QA-0351-G
    (race) are **non-deterministic** and explicitly NOT implemented as
    `#[test]` — left as documented adversarial probes for QA.
  - **No spec ambiguities surfaced** during TEST-SPEC drafting; the R3
    MAY → opt-in flag, R6 SHOULD → strict-BSP gating, and R7 v1 compat
    → wire-FSM-untouched mappings all hold. No spec-critic dispatch
    needed.
- [x] **DEV**: COMPLETE (2026-04-17) — 4 tasks GREEN in sequence on `v2-development`.
  - **TASK-0348 (S, ~80 LoC):** GREEN — added `WorkerRoundStats.has_border_activity: bool` field + `merge::helpers::compute_border_activity(&Partition) -> bool` (O(|free_port_index|), reads `AgentPort(_, 0)`). Test count 850 → 856 (+6: UT-0348-01..05 in `helpers.rs`, UT-06 serde-true round-trip in `types.rs`). All 9 build sites updated to `false` placeholder. Files: `merge/types.rs`, `merge/helpers.rs`, `worker.rs` (FSM stub), `protocol/{worker,types,frame,coordinator}.rs`, `merge/grid.rs`.
  - **TASK-0349 (S, ~50 LoC):** GREEN — wired `compute_border_activity(&partition)` at production sites in `protocol/worker.rs` (after `rebuild_free_port_index`) and `merge/grid.rs` (per-worker reduction loop). R1 ordering preserved (rebuild → compute). Test count 856 → 861 (+5: UT-0349-01..03 in `helpers.rs`, IT-0349-04/05 in `grid.rs`). Files: `merge/{grid,helpers}.rs`, `protocol/worker.rs`.
  - **TASK-0350 (S, ~40 LoC):** GREEN — added `GridConfig.coordinator_free_rounds: bool` (default `false`) + `GridMetrics.coordinator_free_rounds: u32` (Default-derived `0`). Test count 861 → 863 (+2 effective: UT-01 default-disables, UT-02 metric-default-zero is included in existing `test_grid_metrics_default`, UT-03 settable). Fixed 4 explicit `GridConfig` constructors in `config.rs`, `bench/suite.rs`, `bench/benchmarks/church_sum_of_squares.rs` to use `..GridConfig::default()`. Files: `merge/types.rs`, `config.rs`, `bench/suite.rs`, `bench/benchmarks/church_sum_of_squares.rs`.
  - **TASK-0351 (M, ~130 LoC + tests):** GREEN — added `pub(crate) fn check_global_normal_form(&[WorkerRoundStats]) -> (bool, bool)` and the R3/R4 skip-merge / GNF early-exit branches in `run_grid` after stats collection (gated on `coordinator_free_rounds && strict_bsp`). Wire FSM in `protocol/coordinator.rs` untouched (R7). Test count 863 → 874 (+11: UT-0351-01..04 truth table, UT-06 GNF, UT-07 default-v1, UT-08 lenient-no-skip, UT-09 FSM-untouched grep with self-grep mitigation, UT-10/11 R5 equivalences, UT-12 G1 spot check `church_add(2,3) → 5`). Files: `merge/grid.rs` (helper + skip branch + 9 tests), `protocol/coordinator.rs` (1 structural test).
  - **Final gate:** 874 lib + 4 integration = 878 total tests passing (baseline 850 + 24); clippy `--workspace --all-targets -- -D warnings` clean; `cargo fmt --check` clean; release smoke `compute add 3 5 = 8` passes.
- [x] **REVIEW** (2026-04-16) — full-bundle pass over TASK-0348..0351 in
      `docs/reviews/REVIEW-SPEC-19-section-3.1-2026-04-16.md`. Verdict:
      **APPROVE** — 0 MUST-FIX, 2 NICE-TO-HAVE (CLI flag exposure, stub
      unification), R1..R7 all PASS. All four pre-flagged developer smells
      explicitly accepted with rationale (UT-0351-09 self-grep mitigation
      OK; skip-still-merges faithful to §3.1; GNF counter increment correct
      semantics; vacuous-true on empty unreachable in core path). 8 QA
      probes enumerated for Stage 5 (5 documented by developer + 3 added
      by reviewer: empty-partition guard, two-worker round-boundary mix,
      strict-BSP telemetry audit). Architecture clean — `compute_border_activity`
      correctly placed in `merge/helpers.rs`, dependency direction preserved,
      no `unwrap`/`unsafe`/`println!` in production deltas, all GridConfig
      constructors use `..GridConfig::default()` for forward-compat.
- [x] **QA** (2026-04-16) — 9 adversarial probes from §7 of the REVIEW added
      as `qa_probe_*` tests on `merge/{helpers,grid}.rs`:
      - `merge::helpers::tests::qa_probe_a_empty_workers_returns_false_not_vacuous_true`
      - `merge::helpers::tests::qa_probe_f_empty_and_non_principal_border_maps_return_false`
      - `merge::grid::tests::qa_probe_a_run_grid_panics_on_zero_workers`
      - `merge::grid::tests::qa_probe_b_single_worker_always_skips_merge`
      - `merge::grid::tests::qa_probe_c_oscillating_border_activity_no_carryover`
      - `merge::grid::tests::qa_probe_d_lenient_mode_ignores_coordinator_free_flag`
      - `merge::grid::tests::qa_probe_e_race_window_in_local_sim_is_not_applicable` (N/A doc)
      - `merge::grid::tests::qa_probe_g_skip_transition_preserves_correctness`
      - `merge::grid::tests::qa_probe_h_exact_coordinator_free_count`
      Test count 878 → **887** lib (883) + integration (4). All probes PASS
      first try. Clippy `--workspace --all-targets -- -D warnings` clean,
      `cargo fmt --check` clean. **No new bugs surfaced** — the 4 pre-flagged
      developer smells and 3 reviewer-added probes all behave as specified:
      defense-in-depth `assert!(num_workers >= 1)` triggers on zero workers,
      single-worker fast path correctly bypasses skip counter, oscillating
      and skip→non-skip transitions preserve R5 (byte-equal totals + decoded
      results), lenient mode silently ignores the flag (R6 SHOULD), GNF exit
      counts EXACTLY 1 (no double-count of skip + GNF), and helper-direct
      empty / non-principal-only inputs all return false (vacuous-true contract
      pinned at the helper level).
- [x] **REFACTOR** (2026-04-16): no-op — REVIEW returned 0 MUST-FIX, QA found
      0 bugs. The 2 NICE-TO-HAVE items from REVIEW (CLI flag exposure for
      `--coordinator-free-rounds`, deduplicate `build_round_stats` between
      `worker.rs` FSM stub and `protocol/worker.rs` real path) are deferred
      to follow-up backlog items, not blocking. Final gate held at 887 lib +
      4 integration tests, clippy/fmt clean, release smoke green. **Bundle
      shipped.**

### Bundle Shipped — 2026-04-16

SPEC-19 §3.1 (item 2.34) — Coordinator-Free Round (Merge Avoidance) closed
green through all 6 SDD stages. Test count 850 → 887 (+37 across DEV +24
and QA +13 lib counted via 9 probe tests). All R1..R7 PASS. Wire FSM in
`protocol/coordinator.rs` untouched (R7 v1/v2 compat preserved). Next
bundle on Tier 1 break-even path: **item 2.24** (per
`docs/V2-FEATURE-MATRIX.md` order 2.22 → 2.23 → **2.34 ✓** → 2.24 → 2.25
→ 2.35 → 2.26).

### Developer Brief (Stage 3 Dispatch — 2026-04-16)

**Agent:** `developer` (see `.claude/agents/developer.md`)
**Branch:** `v2-development`
**Scope:** 4 tasks in strict sequence, ~300 LoC total, TDD RED→GREEN→REFACTOR per task.

**Inputs per task (read in this order):**
1. `docs/backlog/TASK-03{48,49,50,51}.md` — what to implement + acceptance criteria
2. `docs/tests/TEST-SPEC-03{48,49,50,51}.md` — exact `#[test]` blueprints
3. `specs/SPEC-19-coordinator-free-round.md` — R1..R7 full context
4. `docs/backlog/SPEC-19-section-3.1-coordinator-free-round-tasks.md` — bundle index

**Per-task TDD flow:**
- **TASK-0348 (S):** pragmatic TDD-RED — field + helper + their tests land together
  (trivial additive change; red-phase is a formality for S tasks per user feedback
  `feedback_tdd_red_pragmatic.md`).
- **TASK-0349 (S):** pragmatic TDD-RED; populate 3 build sites (worker.rs,
  protocol/worker.rs if present, merge/grid.rs). Maintain R1 ordering:
  `reduce_all → rebuild_free_port_index → compute_border_activity`.
- **TASK-0350 (S):** pragmatic TDD-RED; additive config field + metric counter.
- **TASK-0351 (M):** REAL TDD-RED — write all tests first with stubs
  (`check_global_normal_form` returning `(false,false)` and no skip branch in
  `run_grid`), confirm reds, then implement.

**Gate after EACH task (order-enforced):**
- `cargo test --workspace` — all green; test count never below 850 lib + 4 integration.
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- `cargo fmt --check` — clean.
- Record test count delta in pipeline-state.md after each task.

**Hard constraints (v2 development rules):**
- No `unwrap()` in production; no `unsafe` without `// SAFETY:`; no `println!`
  (use `tracing`).
- No changes to the wire FSM in `protocol/coordinator.rs` or to any `Message`
  variant in `protocol/mod.rs` beyond the additive
  `has_border_activity: bool` on the `WorkerRoundStats` payload (R7).
- v1 frame format (`protocol/frame.rs`) is untouched; verify by reading the
  file if in doubt — the `has_border_activity` field lives in the bincode v2
  payload of `Message::PartitionResult`, not in the frame header.
- `compute_border_activity` must be O(|free_port_index|) using the existing
  `HashMap<u32, PortRef>` (no net scan).
- For TASK-0351 R5 equivalence test:
  - skip-triggered side: `church_add(2, 3) → 5` at w=2 strict BSP (no border
    activity by construction in the Church encoding).
  - skip-NOT-triggered side: use a multi-partition workload with border
    activity (e.g., `cascade_cross` from benches if exported as a test
    helper; otherwise the smallest existing `run_grid` test workload that
    exhibits non-empty border activity — TASK-0351's TEST-SPEC UT-10/UT-11
    will already name one).
- For TASK-0351 UT-09 (wire-FSM-untouched assertion): read
  `protocol/coordinator.rs` source as a string and assert no new `Message::*`
  variants or new match arms were added in this bundle. Source-grep form is
  acceptable (flagged in TEST-SPEC-0351 for future AST-parse migration).

**Bundle size guardrail:** if any single task exceeds its LoC estimate by >50%,
STOP and report — signal that the spec needs amendment.

**Report-back contract (developer → orchestrator after each task):**
- Task ID + GREEN/RED status
- Test count before → after (+delta)
- Files touched + LoC delta vs estimate
- Any adversarial-probe trip (belong in QA but document if encountered)
- Any design choice that REVIEW should explicitly evaluate

**Orchestrator checkpoint:** after all 4 tasks GREEN, STOP. Do NOT auto-invoke
reviewer — the parent orchestrator checkpoints between stages.

### Hard Gate Carried Forward

Test count must stay **>= 850 lib tests** after the bundle ships
(post-SPEC-18 baseline). Clippy + fmt clean. Release smoke
`compute add 3 5 -> 8` must continue to work.

---

## SPEC-18 §3.5 (item 2.24) — Zero-Copy Archive (rkyv on hot path) — SHIPPED 2026-04-16

**Started:** 2026-04-16
**Test baseline before this work:** 887 lib + 4 integration (post-SPEC-19 §3.1 ship)
**Bundle scope:** SPEC-18 §3.5 only — requirements R20-R27 + tests T11-T14.
  Activates the FLAG_ARCHIVED bit (currently reserved per QA Probe 3 of the
  SPEC-18 §3.1-3.4 ship) under a `#[cfg(feature = "zero-copy")]` gate.
**Explicitly OUT of scope for this bundle:**
  - v1 wire format / v1 protocol path (untouched).
  - SPEC-19 delta protocol code (§3.2/§3.3 are items 2.35/2.26 — separate bundles).
  - Making `zero-copy` a default feature (R20 explicit MUST NOT).
  - `rkyv::access_unchecked` in production code (R24 step 3 explicit MUST NOT).
**Estimated size:** ~600 LoC across atomic tasks (<200 LoC each), 3-5 days,
  Medium-High difficulty (per V2-FEATURE-MATRIX item 2.24 + DEFERRED-WORK D-002).
**Tier 1 break-even path:** confirmed next after item 2.34 in
  `docs/V2-FEATURE-MATRIX.md` (Implementation order:
  2.22 → 2.23 → 2.34 ✓ → **2.24** → 2.25 ✓ → 2.35 → 2.26).
**Dependencies:** SPEC-18 §3.1-3.4 + §3.6 (item 2.23, shipped) + SPEC-19 §3.1
  (item 2.34, shipped). No new external blockers.
**Tracker:** DEFERRED-WORK row D-002 (7-step action plan; this bundle
  fulfils its acceptance signal — close D-002 row when the bundle ships).

### Bundle Goal (single sentence)

When the `zero-copy` cargo feature is enabled, hot-path messages
(`AssignPartition`, `PartitionResult`) MAY be sent as rkyv archives with
`FLAG_ARCHIVED` set in the frame flags byte; receivers decompress (if
`FLAG_COMPRESSED` is also set) → verify CRC32C against the uncompressed
payload (R12 invariant preserved) → call `rkyv::access` (validating API,
NEVER `access_unchecked`) → either use the `ArchivedPartition` reference
directly or `rkyv::deserialize` it; non-hot-path messages with
`FLAG_ARCHIVED` set MUST be rejected with `ProtocolError::ArchiveValidationFailed`;
v2 receivers WITHOUT the `zero-copy` feature MUST cleanly reject
`FLAG_ARCHIVED` frames.

### Stage History

- [x] **SPLITTING** (2026-04-16): complete — 8 atomic tasks delivered
      (TASK-0352..0359, ~600 LoC total). Bundle index at
      `docs/backlog/SPEC-18-section-3.5-zero-copy-tasks.md`. FLAG_RESERVED
      design choice flagged for spec-critic (Option B recommended: route
      `FLAG_ARCHIVED` via `recv_frame` branch; mask `0b1111_1100` already
      excludes bit 1, so zero const changes). Other ambiguities flagged:
      (1) struct vs tuple variant for `ArchiveValidationFailed` (chose
      struct for consistency with `UnknownFlags`/`ChecksumMismatch`),
      (2) try-then-try discrimination strategy for hot-path R22 enforcement
      in `recv_frame`, (3) rkyv send-side error mapping to
      `ArchiveValidationFailed`. SPEC-18 §3.9 R36/R37 confirmed in scope
      (mandates `TransportConfig.use_zero_copy` field + matching CLI flag);
      TASK-0358 covers both. BACKLOG total grew 240 → 248.
- [x] **SPEC-CRITIC** (2026-04-16): complete — 0 spec amendments,
      3 task amendments applied (TASK-0354 tuple variant, TASK-0356
      send-side error prefix, TASK-0357 try-then-try ordering + mandated
      comment). Verdict at
      `docs/spec-reviews/SPEC-18-section-3.5-design-choices-2026-04-16.md`.
      DC-1 (FLAG_RESERVED) already implemented as Option B in
      `frame.rs` — no edit needed. DC-2 mandates tuple form
      `ArchiveValidationFailed(String)` per R35 verbatim — TASK-0354
      flipped struct→tuple, with cascading documentation updates in
      TASK-0356/0357/0358/0359. DC-3 mandates Assign-first try-then-try
      ordering + literal `// SPEC-18 R22 discrimination` source comment
      in `recv_frame` — TASK-0357 acceptance criteria + Key Types code
      block updated. DC-4 mandates conflation of rkyv send-side errors
      into `ArchiveValidationFailed` with `"serialize: "` reason prefix
      — TASK-0356 acceptance criteria + Key Types code block updated
      (placeholder `.map_err(|e| ProtocolError::Serialize(/* see Notes */))`
      replaced with the canonical mapping). TASK-0352, TASK-0353,
      TASK-0355 unaffected (no variant references). Each amended task
      file carries a `## Spec-Critic Amendments (2026-04-16)` section
      recording the change-set and rationale. Bundle index
      `docs/backlog/SPEC-18-section-3.5-zero-copy-tasks.md` updated to
      reflect the resolved design choices. **Stage 2 TESTS unblocked.**
- [x] **TESTS** (2026-04-16): complete — 8 TEST-SPEC files delivered to
      `docs/tests/` (TEST-SPEC-0352 through TEST-SPEC-0359), one per task,
      each enumerating concrete `#[test]` blueprints with target file
      paths, exact fixtures, R-mappings, and feature gates
      (`#[cfg(feature = "zero-copy")]` / unconditional). Spec-critic
      verdicts DC-2/DC-3/DC-4 all pinned by load-bearing tests:
      - **DC-2 (tuple variant)**: UT-0354-04 pattern-matches
        `ProtocolError::ArchiveValidationFailed(s)` — fails to compile if
        struct form is restored.
      - **DC-3 (Assign-first try-then-try + literal source comment)**:
        UT-0357-08 fall-through ordering proof + UT-0357-09 source-grep
        for `// SPEC-18 R22 discrimination` literal in `frame.rs`.
      - **DC-4 (`"serialize: "` prefix mandate)**: UT-0356-07 source-grep
        for the prefix string near `rkyv::to_bytes` in `frame.rs`.
      - **R12 ordering invariant**: UT-0357-04 corrupts CRC bytes of an
        archive+LZ4 frame and asserts `Err(ChecksumMismatch)` — proves
        decompress → CRC → rkyv access ordering.
      - **R22 hot-path-only**: UT-0357-06 rejects bare `u32` archive
        (control-shaped) with `ArchiveValidationFailed`; UT-0359-T14-02
        rejects forged FLAG_ARCHIVED on `Shutdown` variant.
      - **R20 default-OFF**: UT-0352-01 asserts `cfg!(feature = "zero-copy")
        == false` in default build; UT-0352-03 parses `Cargo.toml` via
        `include_str!` to confirm `default = []` does not contain `zero-copy`.
      - **Cumulative bundle test target:** 887 → **904+** lib tests in
        default build (+17), 887 → **939+** lib tests with `--features
        zero-copy` (+52). Conservative floor 887 lib never breached.
      - **Test count exceeds orchestrator hint** (+18 default / +24 feature):
        documented in TEST-SPEC-0359 §"Bundle-wide aggregate test count"
        — variant + CLI tests are unconditional per TASK-0354/0358 design;
        feature-build delta exceeds hint because each per-task TEST-SPEC
        enumerates 5-9 sub-cases covering R-numbers, edge cases, and
        DC-mandates explicitly. Trim path noted: fold UT-0354-04 and
        UT-0357-09 into helper assertions if developer prefers.
      - **Hard-to-write-deterministically tests flagged:**
        UT-0356-07/UT-0357-09 (source-grep self-mitigation, mirrors
        UT-0351-09 pattern); QA-0355-C/D (allocator-misbehavior +
        concurrent-read race, both non-deterministic and NOT implemented
        as `#[test]`); QA-0357-A (premature EOF mid-archive, flagged
        for QA stage with `tokio_test::io::Builder`).
      - **No spec ambiguities surfaced** during TEST-SPEC drafting —
        the 4 DCs already resolved by spec-critic on 2026-04-16 are
        sufficient. No further spec-critic dispatch needed.
- [x] **DEV**: TASK-0352..0359 all GREEN (2026-04-16).
  - **TASK-0352 (S, ~30 LoC):** GREEN (2026-04-16) — `relativist-core/Cargo.toml`
    adds `rkyv = { version = "0.8", optional = true, default-features = false,
    features = ["bytecheck", "alloc", "std"] }` + `zero-copy = ["dep:rkyv"]`.
    Smoke test in `relativist-core/src/lib.rs` confirms u32 round-trip via
    `rkyv::to_bytes`/`access`/`deserialize` under feature. Test count
    883 default / 887 zero-copy.
  - **TASK-0353 (M, ~150 LoC):** GREEN (2026-04-16) — Archive/Serialize/Deserialize
    derives on `Symbol`, `PortRef`, `Agent`, `Net` (`#[rkyv(with = rkyv::with::Skip)]`
    on `freeport_redirects`), `IdRange`, `Partition`, `CompactSubnet`,
    `WorkerRoundStats`. UT-09 cross-build serde-bincode-v2 path test in
    `frame.rs` verifies bincode v2 unaffected by rkyv derives. Test count
    883 default / 893 zero-copy.
  - **TASK-0354 (S, ~30 LoC):** GREEN (2026-04-16) — `protocol/error.rs` adds
    `ArchiveValidationFailed(String)` tuple variant (DC-2) with Display
    `"rkyv archive validation failed: {reason}"`. 5 unconditional tests
    + extended `test_all_variants_debug`. Test count 888 default / 898
    zero-copy.
  - **TASK-0355 (S, ~50 LoC):** GREEN (2026-04-16) — `frame.rs` adds
    `pub(crate) async fn read_aligned_payload<R>(reader, len) -> Result<AlignedVec, _>`
    under `#[cfg(feature = "zero-copy")]` + `#[allow(dead_code)]`
    (wired by TASK-0357 follow-up). 5 zero-copy tests + 1 default
    (cfg-gate symbol absence). R25: AlignedVec base pointer is 16-byte
    aligned for non-empty buffers (debug_assert tolerant of empty case).
    Test count 889 default / 904 zero-copy.
  - **TASK-0356 (M, ~120 LoC):** GREEN (2026-04-16) — `frame.rs` adds
    `ArchiveAssignPayload`, `ArchivePartitionResultPayload` wire wrappers
    (R21 hot-path types), `is_hot_path_message(&Message) -> bool` (R22),
    `pub async fn send_frame_v2<W>(writer, message, use_archive,
    compression_threshold) -> Result<usize, _>` under
    `#[cfg(feature = "zero-copy")]`. R12 CRC over uncompressed archive
    bytes; R23 LZ4 wrap above threshold; non-hot-path messages fall
    through to bincode. DC-4: send-side rkyv errors → `ArchiveValidationFailed(format!("serialize: {}", e))`.
    6 zero-copy tests + 1 default. Test count 890 default / 911 zero-copy.
  - **TASK-0357 (M, ~150 LoC):** GREEN (2026-04-16) — `frame.rs` adds
    `recv_frame` archive branch (`#[cfg(feature = "zero-copy")] if
    header.is_archived() { return decode_archive_payload(&payload, ...); }`)
    after R12 CRC verify, and `decode_archive_payload(payload, total_bytes)
    -> Result<(Message, usize), _>`. DC-3: try-then-try Assign-first
    ordering with mandated `// SPEC-18 R22 discrimination` comments;
    schema mismatch yields R26 error
    `"non-hot-path archive payload (matched neither AssignPartition nor
    PartitionResult)"`. R25 satisfied via `AlignedVec::with_capacity(len)
    + extend_from_slice(payload)` copy at top of decode (recv pipeline
    holds a `Vec<u8>`, allocator-aligned only). QA Probe 3 split
    cfg-gated (default: bincode Deserialize; zero-copy: ArchiveValidationFailed).
    8 zero-copy tests (round-trip Assign+Result, compressed-archive,
    corrupt-archive rejection, R12 CRC-before-rkyv ordering, R26
    non-hot-path rejection, R25 alignment honored, DC-4 prefix
    asymmetry) + 2 default tests (FLAG_ARCHIVED → Deserialize, bincode
    path unaffected). Test count 895 default / 919 zero-copy. Clippy
    + fmt clean both feature configs.
  - **TASK-0358 (S, ~50 LoC):** GREEN (2026-04-16) — `protocol/config.rs`
    adds `TransportConfig.use_zero_copy: bool` (default `false`,
    unconditional field so configs are bit-identical across feature
    builds). `config.rs` adds `--use-zero-copy` flag to both
    `CoordinatorArgs` and `WorkerArgs` (default `false`); threaded
    through `build_transport_config` (now `#[allow(clippy::too_many_arguments)]`
    with 8 params). Test fixtures `make_coordinator_args` /
    `make_worker_args` + 2 `TransportConfig` literals updated. 5
    unconditional tests in `protocol::config::tests` (default false,
    settable, Debug includes field, Clone preserves, NodeConfig chain)
    + 3 CLI tests in `config::tests` (default false on both subcommands,
    flag threads through coordinator, flag threads through worker).
    Test count 903 default / 927 zero-copy.
  - **TASK-0359 (S, ~70 LoC + ~340 LoC test suite):** GREEN (2026-04-16)
    — new module `relativist-core/src/protocol/zero_copy_tests.rs`
    declared in `protocol/mod.rs` under `#[cfg(all(test, feature =
    "zero-copy"))]`. T11..T14 acceptance suite (10 tests):
    - **T11 (round-trip identity):** Assign + Result, both
      uncompressed and compressed; field-by-field equality on
      Partition / WorkerRoundStats (no PartialEq derives).
    - **T12 (corrupt archive rejection):** undersized FLAG_ARCHIVED
      payload yields ArchiveValidationFailed.
    - **T13 (alignment correctness):** R25 round-trip battery at
      sizes [0, 1, 8, 17, 64, 128, 256, 1023, 4096] uncompressed
      + [4, 32, 256, 4096] compressed.
    - **T14 (hot-path-only enforcement):** bare `u32` archive yields
      R26 phrase `"non-hot-path archive payload"`; cold-path
      `Shutdown` with `use_archive=true` falls through to bincode
      (no FLAG_ARCHIVED ever emitted).
    - **T11/T13 cross-cut matrix:** 4 hot-path messages × 2
      compression states = 8 round-trips; variant identity check.
    Test count 903 default / 937 zero-copy. Clippy + fmt clean both
    feature configs.
  - **Final gate:** 903 lib default + 4 integration (+16 from
    baseline 887) / 937 lib zero-copy + 4 integration (+50 from
    baseline). Clippy `--workspace --all-targets -- -D warnings`
    clean both feature configs. `cargo fmt --check` clean. Release
    smoke `compute add 3 5 → 8` works. Bundle ships GREEN through
    Stage 3.
  - [x] **MF-1 RESOLVED (2026-04-16):** Option A (hoist) — `recv_frame`
    now calls `read_aligned_payload` directly on the FLAG_ARCHIVED +
    !FLAG_COMPRESSED fast path, eliminating the second `Vec<u8> →
    AlignedVec` copy that `decode_archive_payload` previously
    performed. The compressed-archive path still copies (decompression
    yields a plain `Vec<u8>`); `decode_archive_payload` is now a pure
    R25-precondition consumer with a `debug_assert!` alignment witness.
    `#[allow(dead_code)]` removed. R12 ordering preserved on both
    paths (decompress → CRC verify on uncompressed payload → rkyv
    access). Doc comments amended to reflect actual behavior. Tests
    now 903 lib default / 937 lib `--features zero-copy` (unchanged
    counts; the 5 helper tests stay valid because the function is
    still called from production code). Clippy + fmt clean both
    feature configs.
- [x] **REVIEW** (2026-04-16): **APPROVE** (MF-1 resolved) — verdict at
      `docs/reviews/REVIEW-SPEC-18-section-3.5-2026-04-16.md`.
      1 MUST-FIX (blocking): **MF-1** resolve `read_aligned_payload`
      dead-code state. The helper at `frame.rs:258-282` is `pub(crate)`,
      fully tested (5 tests), but no caller exists outside tests — the
      `#[allow(dead_code)]` is load-bearing today. Acceptable fixes:
      **(A)** hoist into `recv_frame` uncompressed-archive fast path
      (preferred — saves one copy); **(B)** strip the helper + 5 tests
      and document the deferred hoist in `docs/DEFERRED-WORK.md`.
      "Accept-with-deferred-hoist" is NOT acceptable (no follow-up
      task tracked). 5 NICE-TO-HAVE items deferrable (NTH-1 through
      NTH-5: builder refactor, test-helper dedup, PartialEq audit,
      visibility narrowing × 2). Spec compliance matrix: R20-R27 +
      R35-R37 all PASS. Architecture: dependency direction respected;
      feature gating correct; DC-1..DC-4 spec-critic verdicts honored
      in code. No `unsafe`, no `unwrap()` in production paths, no
      `println!`, no `access_unchecked`. 8 QA probes enumerated for
      Stage 5 (Q1 high-priority pointer-corruption; Q3 cross-feature
      interop; Q6 ARM alignment defer-if-no-ARM-CI). One pipeline
      note (F-9): default-build FLAG_ARCHIVED falls through to bincode
      `Deserialize` error (NOT `ArchiveValidationFailed("zero-copy
      feature disabled")` as the original brief suggested) — behavior
      is consistent with DC-1 Option B and R19 forward-compat; brief
      wording should be amended on next sdd-pipeline pass.
      **Update (2026-04-16):** MF-1 RESOLVED via Option A hoist —
      `read_aligned_payload` is now invoked from `recv_frame` on the
      FLAG_ARCHIVED + !FLAG_COMPRESSED fast path; `#[allow(dead_code)]`
      removed; helper is no longer dead. Verdict transitions to
      **APPROVE** (no longer with MUST-FIX). All gates re-greened on
      both feature configs (test 903/937, clippy clean, fmt clean).
- [ ] **QA**: **READY TO DISPATCH** (qa). MF-1 resolved (2026-04-16);
      gates re-greened: `cargo test --workspace` 903 lib + 4 integration
      default / 937 lib + 4 integration `--features zero-copy`; `cargo
      clippy --workspace --all-targets -- -D warnings` clean both
      feature configs; `cargo fmt --check` clean. Probes Q1-Q8 from
      the review (`docs/reviews/REVIEW-SPEC-18-section-3.5-2026-04-16.md`
      §7) define the QA bug-hunt scope.
- [x] **REFACTOR** (2026-04-16): no-op — Stage 4 REVIEW MF-1 already
      resolved (Option A hoist of `read_aligned_payload` into `recv_frame`);
      Stage 5 QA found 0 bugs across 10 probes. The 5 NICE-TO-HAVE items
      from REVIEW (NTH-1 builder-struct refactor, NTH-2 dedup test helpers,
      NTH-3 PartialEq audit, NTH-4/5 visibility narrowing) are deferred
      as follow-up backlog items. Final gate: 905 lib + 4 integration
      (default), 945 lib + 4 integration (`--features zero-copy`); clippy
      clean both feature configs; fmt clean; release smoke green.
      **Bundle shipped.**

### Bundle Shipped — 2026-04-16

SPEC-18 §3.5 (item 2.24) — Zero-Copy Archive (rkyv on hot path) closed
green through all 6 SDD stages (plus Stage 1.5 spec-critic). Test count
887 → 905 (default, +18) / 887 → 945 (`--features zero-copy`, +58). All
R20-R27 PASS. DEFERRED-WORK D-002 closed and moved to Resolved
Deferrals. Tier 1 break-even path next: **item 2.35** (Delta-Based
Merge with BorderGraph, SPEC-19 §3.2 — item 2.25 already DONE per
commit c360fe5).

### Current Stage

**Stage 4 REVIEW — COMPLETE.** Verdict: **APPROVE** (MF-1 resolved
via Option A hoist on 2026-04-16). Bundle is structurally sound,
spec-compliant, and the previously flagged dead-code seam is now wired
through production. Test counts: default features **903 lib** (+16 from
baseline 887), `--features zero-copy` **937 lib** (+50 from baseline).
Bundle floor 887 lib never breached. Stage 5 QA ready to dispatch.

### Hard Gate Carried Forward

Test count must stay **>= 887 lib tests** after the bundle ships
(post-SPEC-19 §3.1 baseline). Clippy + fmt clean. Release smoke
`compute add 3 5 -> 8` must continue to work. With `--features zero-copy`,
all 887+ tests MUST also pass.

### Task-Splitter Brief (Stage 1 Dispatch — 2026-04-16)

**Agent:** `task-splitter` (see `.claude/agents/task-splitter.md`)
**Branch:** `v2-development`
**Output:** `docs/backlog/SPEC-18-section-3.5-zero-copy-tasks.md` (bundle index)
  + per-task files starting at TASK-0352 (next available; TASK-0351 was the
  last assigned in SPEC-19 §3.1 bundle).

**Pre-flight reading (in this order):**
1. `specs/SPEC-18-wire-format-v2.md` §3.5 (R20-R27), §4.6 (rkyv archive
   types and `rkyv::access` validating API), §7.2 (T11-T14 test list).
2. `docs/DEFERRED-WORK.md` row D-002 — 7-step action plan + files-to-revisit
   list + acceptance signal. **The 7-step plan is a hint, NOT a substitute
   for atomic task decomposition** (each step may map to 1-2 atomic tasks).
3. `relativist-core/Cargo.toml` — current feature flags (none for rkyv yet);
   confirm no rkyv dep already lurks.
4. `relativist-core/src/protocol/frame.rs` — current `FLAG_ARCHIVED` handling
   (the bit IS already defined as a const + reserved-mask partition; QA
   Probe 3 of the SPEC-18 §3.1-3.4 ship pinned that frames with
   `FLAG_ARCHIVED` currently fail bincode v2 deserialization — this bundle
   activates that bit).
5. `relativist-core/src/protocol/types.rs` (Message enum — identify hot-path
   variants `AssignPartition` + `PartitionResult`) and
   `relativist-core/src/partition/types.rs` (Partition, IdRange, free_port_index
   HashMap, CompactSubnet serde adapter — note: rkyv path bypasses
   CompactSubnet per SPEC-18 §4.6).

**Hard scope boundaries (out of scope — task-splitter MUST NOT generate
tasks for these):**
- Modifying v1 wire format or v1 protocol path.
- Touching SPEC-19 delta protocol code (§3.2/§3.3 are items 2.35/2.26).
- Making `zero-copy` a default feature (R20).
- Using `rkyv::access_unchecked` in production code (R24 step 3).

**Suggested task boundaries (task-splitter may refine; ~600 LoC total):**
1. **TASK-0352 — Cargo.toml: rkyv optional dep + `zero-copy` feature gate.**
   ~30 LoC, S. Adds `rkyv = { version = "0.8", optional = true,
   features = ["validation"] }` (or current stable, task-splitter to
   verify) and `zero-copy = ["dep:rkyv"]`. No source changes yet.
2. **TASK-0353 — Derive Archive/rkyv::Serialize/rkyv::Deserialize on the
   8 types listed in R21** (`Net`, `Partition`, `CompactSubnet`, `Agent`,
   `Symbol`, `PortRef`, `IdRange`, `WorkerRoundStats`) under
   `#[cfg_attr(feature = "zero-copy", derive(rkyv::Archive,
   rkyv::Serialize, rkyv::Deserialize))]`. ~150 LoC across multiple
   files (net/types.rs, partition/types.rs, partition/compact.rs,
   merge/types.rs). M. Depends on TASK-0352.
3. **TASK-0354 — `ProtocolError::ArchiveValidationFailed(String)` variant
   + Display impl (R26 + R35)** under `#[cfg(feature = "zero-copy")]` (or
   unconditional — task-splitter to choose). ~30 LoC, S. Depends on
   TASK-0352.
4. **TASK-0355 — Aligned receive buffer using `rkyv::util::AlignedVec`
   (R25)** wired into recv_frame when FLAG_ARCHIVED is set. ~50 LoC, S.
   Depends on TASK-0352, TASK-0353.
5. **TASK-0356 — send_frame archive path: `rkyv::to_bytes`, set
   FLAG_ARCHIVED, optional LZ4 wrap (R23)** restricted to hot-path
   messages only (R22). ~100 LoC, M. Depends on TASK-0353, TASK-0354.
6. **TASK-0357 — recv_frame archive path: detect FLAG_ARCHIVED,
   decompress if combined with FLAG_COMPRESSED, validate CRC against
   uncompressed payload (R12 invariant preserved), call `rkyv::access`
   (validating API), reject non-hot-path messages with
   `ArchiveValidationFailed` (R26).** ~120 LoC, M. Depends on TASK-0354,
   TASK-0355.
7. **TASK-0358 — CLI `--use-zero-copy` flag (R36, R37)** gated on
   `#[cfg(feature = "zero-copy")]`; thread through `TransportTuning` /
   `TransportConfig`. ~50 LoC, S. Depends on TASK-0352. Task-splitter to
   verify whether SPEC-18 §3.9 R36/R37 apply or whether
   `TransportConfig` already has the field.
8. **TASK-0359 — T11-T14 test suite** (round-trip identity, rejection
   of corrupt archives, alignment correctness, hot-path-only enforcement)
   gated under `#[cfg(feature = "zero-copy")]`. ~70 LoC, S — but
   task-splitter to confirm boundary; T11-T14 may yield 4-6 tests each
   (16-24 total).

**R12 ordering invariant (load-bearing — must be preserved):** CRC32C is
ALWAYS computed on the uncompressed payload. The recv pipeline order when
both FLAG_ARCHIVED + FLAG_COMPRESSED are set: **decompress → CRC verify →
rkyv access**. QA Probe 4 from the SPEC-18 §3.1-3.4 ship pinned this; the
rkyv path MUST honor it. Task-splitter to bake this ordering into TASK-0357
acceptance criteria.

**Wire compatibility (additive, not breaking):** v2 wire format is shipped
(item 2.23). FLAG_ARCHIVED bit is currently reserved (QA Probe 3 confirmed
it errors out as bincode Deserialize when no rkyv path exists). Activating
it is purely additive — v2 receivers WITHOUT `zero-copy` feature should
reject FLAG_ARCHIVED frames cleanly. **DESIGN CHOICE FOR SPEC-CRITIC:** the
FLAG_RESERVED check in `recv_frame` currently treats bit 1 as reserved
(via `FLAG_RESERVED = 0b1111_1100` partition); when feature is enabled, bit
1 must NOT be in the reserved mask. Two options:
  - (a) Adjust FLAG_RESERVED conditionally per-feature (
    `#[cfg(feature = "zero-copy")] const FLAG_RESERVED: u8 = 0b1111_1100;`
    vs feature-off);
  - (b) Keep FLAG_RESERVED constant and have the reserved-flag check
    explicitly route FLAG_ARCHIVED through a different code path that
    errors with `ArchiveValidationFailed("zero-copy feature disabled")`
    when the feature is off.
Task-splitter MUST flag this choice for spec-critic review BEFORE the TESTS
stage — the reviewer needs to pick one canonically.

**Test count baseline:** 887 lib + 4 integration. **Target after bundle
ships:** 905+ lib (+18 estimated; T11-T14 each yields 4-6 tests). Hard
floor: never below 887 lib at any point during DEV.

**Per-task acceptance criteria template (task-splitter to expand per task):**
- File paths to create/modify (relativist-core/...).
- R-numbers covered.
- Acceptance bullet: `cargo test --workspace` green; `cargo clippy
  --workspace --all-targets -- -D warnings` clean (with AND without
  `--features zero-copy`); `cargo fmt --check` clean.
- LoC budget; STOP if exceeded by >50%.

**Hard constraints (v2 development rules):**
- No `unwrap()` in production; no `unsafe` without `// SAFETY:`; no
  `println!` (use `tracing`).
- All deliverables in English (CLAUDE.md rule).
- Specs MUST NOT be modified by task-splitter (specialista-em-specs only).
- Source files MUST NOT be modified by task-splitter (developer only).

**Report-back contract (task-splitter → orchestrator):**
- Bundle index file path.
- Per-task: ID, title, complexity (S/M/L), LoC estimate, R-number coverage,
  dependencies.
- DAG diagram (text).
- Cumulative LoC vs ~600 LoC budget.
- Cumulative test target vs 887 → 905+ baseline.
- Any spec ambiguities flagged for spec-critic (esp. FLAG_RESERVED design
  choice).
- Whether D-002 7-step plan was followed verbatim or refined.

**Orchestrator checkpoint:** after task-splitter delivers the bundle, STOP.
Do NOT auto-invoke test-generator — the parent orchestrator checkpoints
between stages.
