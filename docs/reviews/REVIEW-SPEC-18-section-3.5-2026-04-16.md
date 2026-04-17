# Review: SPEC-18 §3.5 (item 2.24) — Zero-Copy Archive (rkyv on hot path)

**Date:** 2026-04-16
**Reviewer:** reviewer (unified code-quality + architecture, adversarial)
**Bundle:** TASK-0352..0359 (8 tasks, ~600 LoC src + ~340 LoC tests)
**Branch:** `v2-development`, commit `6d16b5c`
**Files reviewed:**
- `relativist-core/Cargo.toml`
- `Cargo.lock` (rkyv 0.8.15 confirmed)
- `relativist-core/src/protocol/error.rs`
- `relativist-core/src/protocol/frame.rs`
- `relativist-core/src/protocol/zero_copy_tests.rs`
- `relativist-core/src/protocol/config.rs`
- `relativist-core/src/protocol/mod.rs`
- `relativist-core/src/protocol/types.rs`
- `relativist-core/src/config.rs`
- `relativist-core/src/net/types.rs`, `net/core.rs`
- `relativist-core/src/partition/types.rs`, `partition/compact.rs`
- `relativist-core/src/merge/types.rs`
- `relativist-core/src/lib.rs`
- `specs/SPEC-18-wire-format-v2.md` §3.5 (R20-R27), §3.8 (R35), §3.9
  (R36/R37), §4.6, §7.2 (T11-T14)
- `docs/spec-reviews/SPEC-18-section-3.5-design-choices-2026-04-16.md`
- `docs/backlog/TASK-0352..0359.md`
- `docs/tests/TEST-SPEC-0352..0359.md`

---

## 1. Verdict

**APPROVE WITH MUST-FIX (1 MUST-FIX, 5 NICE-TO-HAVE).**

The bundle is structurally sound, faithfully implements R20-R27 + R35-R37,
honors all four spec-critic verdicts (DC-1..DC-4), preserves R12 ordering
in the receive path, and respects the `net <- partition <- merge <-
protocol` dependency direction. Tests are comprehensive (16 default-build
unconditional tests + 50 feature-gated tests, all green). Clippy + fmt
clean on both feature configs. No `unsafe`, no `unwrap()` in production
paths, no `println!`, no `access_unchecked`.

The single MUST-FIX is a **dead-code hygiene issue** at the public-helper
seam: `read_aligned_payload` is `pub(crate)`, fully tested under feature
build, but its only caller currently inlines the same logic — the
`#[allow(dead_code)]` is load-bearing today. The choice between
**hoist or strip** is forced by the QA stage's bug-hunting baseline; we
cannot ship with a `pub(crate)` helper documented as "wired by TASK-0357
follow-up" in the same PR that ships TASK-0357. This is a 1-hour fix.

The 5 NICE-TO-HAVE items address surface-level smells (8-arg builder,
wide try-then-try ordering coupling, magic numbers, comment style, missing
`#[must_use]` on a serializer) that should be addressed in a follow-up
sweep but do not block QA.

---

## 2. Per-pre-flagged-smell verdict

### Smell 1: `read_aligned_payload` retains `#[allow(dead_code)]`

**Verdict: MUST-FIX (mandate immediate hoist OR strip).**

The helper is `pub(crate) async fn read_aligned_payload<R: AsyncReadExt
+ Unpin>(reader, len) -> Result<AlignedVec, ProtocolError>` (frame.rs:260)
documented as "wired by TASK-0357 follow-up". The `decode_archive_payload`
function (frame.rs:521-523) does its own AlignedVec copy:

```rust
let mut aligned: rkyv::util::AlignedVec = rkyv::util::AlignedVec::with_capacity(payload.len());
aligned.extend_from_slice(payload);
```

This is the second copy on every uncompressed-archive path. The hoist
the comment promises (allocate aligned in `recv_frame` step 4, skip the
copy on the uncompressed path) is correct and small. Five tests
(`read_aligned_payload_*`) currently exist purely to validate a helper
nothing calls outside tests — that is by definition dead code, even if
the code itself is exercised.

The pipeline cannot ship with a stage marked GREEN whose primary
public-crate helper is dead at runtime. Two acceptable resolutions:

- **(A) Hoist**: in `recv_frame`, when `header.is_archived()` AND NOT
  `header.is_compressed()`, allocate via `read_aligned_payload` instead
  of `vec![0u8; len]`, then pass the `AlignedVec` (or its `as_ref()`)
  through to a refactored `decode_archive_payload` that no longer copies.
  The test surface is unchanged.
- **(B) Strip**: delete `read_aligned_payload` + the 5 R25 tests; rely on
  the in-line `AlignedVec::with_capacity(payload.len()) +
  extend_from_slice` inside `decode_archive_payload`. Document the
  decision in a SPEC-18 §3.5 follow-up note (DEFERRED-WORK or ROADMAP
  amendment) so the hoist isn't silently lost.

I prefer **(A)** because R25 explicitly justifies the SHOULD-use of
`AlignedVec` and the hoist is cheap. **(B)** is acceptable if (A) would
expand the bundle past its size envelope. **Accept-with-deferred-hoist
is NOT acceptable** because it ships a known dead-helper with a
"TASK-0357 follow-up" comment that has no tracked follow-up task.

### Smell 2: QA-Probe-3 expectation split / undersized-payload corruption

**Verdict: ACCEPT — the substitution is load-bearing AND faithful to R26.**

The original ambition (flip arbitrary bytes in a valid archive) failed
because rkyv's archive layout for the empty test partition is
predominantly POD `u32` fields whose individual byte values are all valid.
Single-bit flips landed in fields that re-validated. This is a
**cosmetic test deficiency**, not an evidence of an exploit surface that
the test misses.

Concretely:
- T12 (`t12_undersized_archive_payload_is_rejected`,
  zero_copy_tests.rs:259) and UT-0357-04
  (`recv_frame_v2_corrupt_archive_rejected`, frame.rs:2206) **prove
  R26 correctness**: any payload whose byte length is below the rkyv
  layout minimum for *both* hot-path schemas yields
  `ArchiveValidationFailed`. This covers the legitimate failure mode
  (truncation, wrong message, schema drift) the spec is concerned about.
- T14 (`t14_non_hot_path_archive_yields_r26_message`) covers the orthogonal
  axis: a *structurally valid* archive of a non-hot-path schema. Together
  these two probes pin R22/R26 correctness from both sides.
- The byte-flip failure mode the developer abandoned would only catch a
  rkyv-internal validation bug (e.g. relative-pointer drift past the
  buffer). That is the **rkyv crate's responsibility**, not ours; their
  test suite covers it.

That said, I want one more probe added at QA stage to cover the
high-value middle: **a structurally valid AssignPartition archive where
one byte of a relative-pointer field is corrupted to point outside the
buffer**. See QA Probe Q1 in §7 below.

### Smell 3: `build_transport_config` 8-arg signature

**Verdict: ACCEPT THE LINT for v2; refactor to a builder is NICE-TO-HAVE.**

The function is a private `fn` in `config.rs` (config.rs:536) called
from exactly two siblings (`build_node_config_coordinator`,
`build_node_config_worker`). The `#[allow(clippy::too_many_arguments)]`
is local to the function. The 8 args correspond 1:1 to CLI flags
(`backend_str`, `socket_path`, `no_tcp_nodelay`, `send_buffer`,
`recv_buffer`, `keepalive`, `compression_threshold`, `use_zero_copy`) so
the builder pattern would be ceremony for a function that is used twice
and changes once per spec section.

For v2 ship: the lint suppression is justified. **NICE-TO-HAVE NTH-1**
(see §6) tracks the builder refactor as a follow-up that should land
when the next CLI flag is added (the next time we'd push to 9 args is
the natural inflection point).

The orthogonal observation: **`TransportConfig.use_zero_copy` is
correctly unconditional** (config.rs:90, no `#[cfg]` gate). The
developer's rationale (bit-identical configs across feature builds) is
sound and matches the field's CLI counterpart in `CoordinatorArgs` /
`WorkerArgs`, which are also unconditional. This means a default-build
binary parses `--use-zero-copy` without error and silently no-ops on the
send path (frame.rs:474 — `#[cfg(feature = "zero-copy")] if
header.is_archived()`); a feature-build binary honors the flag. That's
the desired bit-identical CLI surface and is correctly implemented.

### Smell 4: Send-side error mapping conflates serialize + recv errors

**Verdict: SPEC-CRITIC's BLESSING HOLDS in the actual code.**

DC-4 mandates `ArchiveValidationFailed(format!("serialize: {}", e))` on
send-side rkyv failures. I verified:
- frame.rs:325-327: `Message::AssignPartition` send path constructs
  `ProtocolError::ArchiveValidationFailed(format!("serialize: {}", e))`.
- frame.rs:340: `Message::PartitionResult` send path uses the same
  literal pattern.
- frame.rs:531-532, 551: recv-side errors use schema-name prefixes
  `"AssignPartition: "` / `"PartitionResult: "`.
- frame.rs:564-567: R26 non-hot-path rejection uses literal
  `"non-hot-path archive payload (matched neither AssignPartition nor
  PartitionResult)"` (no `serialize:` prefix).

**No leakage**. The send side cannot construct a recv-prefixed error
because the recv side is in a different function (`decode_archive_payload`)
which itself never references `serialize: `.

UT-0357-08 (frame.rs:2335) explicitly asserts the inverse: a recv-side
`ArchiveValidationFailed` MUST NOT start with `"serialize: "`. UT-0354-03
(error.rs:328) asserts the prefix is preserved through `Display`. Both
pass in the green build.

---

## 3. Additional findings

### F-1: `total_bytes` parameter to `decode_archive_payload` is correct but unverified

frame.rs:476 passes `FRAME_HEADER_SIZE + header.length as usize` as
`total_bytes`; frame.rs:539 returns it. The value matches what
`recv_frame` would have returned via the bincode path
(frame.rs:487), and UT-0357-01 asserts `n_sent == n_recv`. This is
correct but I want to call out that NO test directly compares the
returned `total_bytes` against the expected formula on the archive path
when `FLAG_COMPRESSED` is also set — UT-0357-03 round-trips but does not
assert `n_sent == n_recv`. **Add to QA Q5** (§7).

### F-2: `read_aligned_payload` length-zero path uses `debug_assert!` for alignment

frame.rs:277-280: the `is_empty()` short-circuit is correct (rkyv never
reads from a zero-length archive), but `debug_assert!` becomes a no-op
in release. The spec invariant (R25) is "MUST be 16-byte aligned" —
debug_assert is acceptable because the AlignedVec type guarantees this
at construction; the assert is a structural witness, not a correctness
check. Acceptable.

### F-3: `assert_partition_eq` / `assert_stats_eq` duplicated across files

Both helpers live in `frame.rs:2020-2058` (test mod) AND in
`zero_copy_tests.rs:95-127`. Identical bodies. Test code duplication is
NOT a MUST-FIX, but it is a smell — if a Partition field is added,
both must be updated. **NTH-2** (§6).

### F-4: `Partition` does NOT implement `PartialEq`; helpers compensate

This is documented in zero_copy_tests.rs:18-19 and frame.rs:2017-2018.
The "no PartialEq because of Net/HashMap inner state" comment is honest
but invites future bugs: a new PortRef or border field added to
Partition will be silently skipped by both helpers. **NTH-3** —
either derive `PartialEq` on Partition (Net already implements
`PartialEq` per the existing assertion `assert_eq!(left.subnet,
right.subnet)`) or add a `#[non_exhaustive]` reminder.

### F-5: `// SPEC-18 R22 discrimination` comment placement

DC-3 mandates the literal `// SPEC-18 R22 discrimination` comment in the
recv try-then-try block. I verified frame.rs:525 (Assign branch) and
frame.rs:543 (PartitionResult branch). **PASS.** UT-0357-09 was renamed
to UT-0357-10 (default-features test, frame.rs:2407) per the task file;
the source-grep is implicitly covered by the literal comment being
present at compile time. No issue.

### F-6: `is_hot_path_message` is `pub` but only used inside frame.rs

frame.rs:228 is `pub fn is_hot_path_message(msg: &Message) -> bool`.
Used at frame.rs:313 only. Should be `pub(crate)` per the
"visibility is minimal" rule. **NTH-4.**

### F-7: rkyv archive wrappers are `pub` but only used inside frame.rs

frame.rs:199-217: `ArchiveAssignPayload` + `ArchivePartitionResultPayload`
are `pub` structs. They never escape `frame.rs`. Should be `pub(crate)`
or `pub(super)`. **NTH-5.**

### F-8: ALL of these comments are good

The bundle has unusually high-quality comments — they explain WHY
(spec citation + invariant rationale), not WHAT. Examples worth keeping:
- frame.rs:189-196 explains why the rkyv path uses tagged wrapper
  structs instead of `Message` enum.
- frame.rs:235-257 explains AlignedVec rationale and the deferred hoist.
- frame.rs:284-304 explains the send-path invariants and DC-4 prefix.
- frame.rs:491-506 explains try-then-try ordering and DC-3 reasoning.

This sets a good bar for future bundles. No action needed.

### F-9: Default-build behavior on FLAG_ARCHIVED is "fall through to bincode"

frame.rs:474-477: under `cfg(not(feature = "zero-copy"))`, a
FLAG_ARCHIVED frame skips the archive branch entirely and lands at the
bincode decoder (frame.rs:480), which returns `ProtocolError::Deserialize`.
**This deviates from the brief's "must reject cleanly with
ArchiveValidationFailed('zero-copy feature disabled')"** but matches
what was actually shipped per UT-0357-09 default-features test
(frame.rs:2375, asserts `Deserialize` error).

The brief's literal wording ("ArchiveValidationFailed('zero-copy
feature disabled')") was a SUGGESTION in the original task-splitter
brief; the spec-critic's DC-1 verdict explicitly approved Option B
(route in `recv_frame`) but did NOT mandate a particular error variant
for the feature-off branch. The shipped behavior (bincode-fallthrough →
Deserialize error) is **load-bearing for forward-compat**: if a future
v3 redefines bit 1, default-build receivers will reject via Deserialize
rather than mis-routing through a feature-off rkyv handler. R19 is
preserved; R26 only fires under feature ON.

This is acceptable, but **the brief's expectation should be amended in
the pipeline-state followup notes** so future readers don't think the
shipped code drifted from the spec. See pipeline-state update at the
end of this review.

---

## 4. Spec compliance matrix (R20-R27 + R35-R37)

| Req | Verdict | Evidence |
|---|---|---|
| **R20** (`zero-copy` feature-gated, NOT default) | **PASS** | Cargo.toml:96-104. `default = []`; `full = ["tls", "metrics", "otel"]` does NOT include `zero-copy`; `zero-copy = ["dep:rkyv"]` is opt-in. UT-0352-01 (lib.rs:58) asserts `cfg!(feature = "zero-copy") == false` in default build. |
| **R21** (8 types derive Archive/Serialize/Deserialize) | **PASS** | `Net` (net/core.rs:22), `Partition` (partition/types.rs:34), `CompactSubnet` (partition/compact.rs:36), `Agent` (net/types.rs:197), `Symbol` (net/types.rs:36), `PortRef` (net/types.rs:66), `IdRange` (partition/types.rs:16), `WorkerRoundStats` (merge/types.rs:119). All 8 use `#[cfg_attr(feature = "zero-copy", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]`. Per-type round-trip tests exercise each derive. |
| **R22** (hot-path-only: AssignPartition + PartitionResult) | **PASS** | `is_hot_path_message` (frame.rs:228) gates `send_frame_v2` (frame.rs:313). `decode_archive_payload` (frame.rs:508) only constructs Assign/PartitionResult variants and rejects all other archives with R26 message. Control messages (Shutdown/Register/RegisterAck/RegisterNack/Error) ALWAYS take the bincode v2 path even when `use_archive=true` (frame.rs:313-315). T14_cold_path_message_with_archive_flag_falls_through_to_bincode confirms send side; UT-0357-06 + T14 confirm recv side. |
| **R23** (send: rkyv::to_bytes → FLAG_ARCHIVED → optional LZ4) | **PASS** | frame.rs:324, 339 invoke `rkyv::to_bytes::<rkyv::rancor::Error>(&payload)`. frame.rs:347 computes CRC32C on uncompressed archive bytes (R12). frame.rs:350-358 sets FLAG_ARCHIVED unconditionally and adds FLAG_COMPRESSED above threshold. UT-0356-04 (`send_frame_v2_large_payload_triggers_compression`) and UT-0356-06 (`send_frame_v2_crc_over_uncompressed_archive`) verify. |
| **R24** (recv: decompress → CRC verify → rkyv::access validating API) | **PASS** | frame.rs:448-461 decompress + CRC verify happen BEFORE the `is_archived()` branch at frame.rs:475. `decode_archive_payload` calls `rkyv::access::<rkyv::Archived<X>, rkyv::rancor::Error>` (frame.rs:528, 545) — the **validating** API. NO `access_unchecked` anywhere in src/ (verified via grep). UT-0357-05 explicitly proves CRC-before-rkyv ordering by corrupting the CRC byte and asserting `ChecksumMismatch` instead of `ArchiveValidationFailed`. |
| **R25** (16-byte alignment via AlignedVec) | **PASS** | `decode_archive_payload` (frame.rs:521) copies into `rkyv::util::AlignedVec::with_capacity(payload.len())`. `read_aligned_payload` helper (frame.rs:260) provides the same guarantee for the future hoist (currently dead — see MUST-FIX-1). UT-0355-02 asserts `(buf.as_ptr() as usize) % 16 == 0` for non-empty buffers. T13 alignment battery (zero_copy_tests.rs:291-317) round-trips 9 sizes uncompressed + 4 compressed. |
| **R26** (validation failure → ArchiveValidationFailed) | **PASS** | All `rkyv::access` failures map to `ArchiveValidationFailed("non-hot-path archive payload ...")` at frame.rs:564 (DC-3 fall-through path). All `rkyv::deserialize` failures map to `ArchiveValidationFailed(format!("AssignPartition: {}", e))` / `("PartitionResult: {}", e)` at frame.rs:531-532, 551. UT-0357-04, T12, T14 all assert. |
| **R27** (round-trip identity: `deserialize(access(to_bytes(p))) == p`) | **PASS** | T11 series (zero_copy_tests.rs:145-252) covers AssignPartition + PartitionResult, uncompressed + compressed. Per-type round-trip tests in net/types.rs:754-810, partition/types.rs:328-398, partition/compact.rs:291, merge/types.rs:506-525 cover each individual type. |
| **R35** (ProtocolError variant tuple form) | **PASS** | error.rs:93 declares `ArchiveValidationFailed(String)` (tuple). DC-2 mandate honored. UT-0354-02 pattern-matches the tuple form (will fail to compile if struct form regression). |
| **R36** (TransportConfig.compression_threshold + use_zero_copy) | **PASS** | protocol/config.rs:74 (compression_threshold), protocol/config.rs:90 (use_zero_copy). Both fields unconditional, both default to spec values (1024, false). |
| **R37** (CLI flags `--compression-threshold` + `--use-zero-copy` on both subcommands) | **PASS** | config.rs:186-195 (CoordinatorArgs.use_zero_copy with `#[arg(long, default_value_t = false)]`), config.rs:259-262 (WorkerArgs.use_zero_copy). `cli_use_zero_copy_default_is_false_on_both_subcommands` (config.rs:1412) + `cli_use_zero_copy_flag_threads_through_coordinator/worker` (config.rs:1427, 1451) verify the flag and end-to-end flow into TransportConfig. |

**Wire-format compatibility:**
- v1 wire format: **UNCHANGED**. `send_frame` / `recv_frame` are the v2
  entry points (with the v2 9-byte header from prior bundles); the v1
  format is frozen on `v1-feature-complete` and not touched.
- v2 default-feature behavior: **PRESERVED**. A v2 receiver without
  `zero-copy` feature receives FLAG_ARCHIVED frame, passes the
  has_unknown_flags check (FLAG_ARCHIVED is bit 1, NOT in FLAG_RESERVED),
  passes CRC, and falls into the bincode decoder which fails as
  `Deserialize` error. UT-0357-09 (default-features) asserts this.
  This deviates from the prompt's literal expectation
  ("ArchiveValidationFailed('zero-copy feature disabled')") but is
  consistent with DC-1's Option B verdict and R19 forward-compat. See F-9.
- FLAG_RESERVED mask: **UNCHANGED at `0b1111_1100`** (frame.rs:36). DC-1
  Option B verified.

---

## 5. MUST-FIX items

### MF-1: Resolve `read_aligned_payload` dead-code state (BLOCKING)

**Category:** Code Quality (dead code) + Architecture (orphan public seam)
**Principle:** Clean Code "no dead code"; SPEC-13 R44 (visibility minimal)
**File:** `relativist-core/src/protocol/frame.rs:258-282`
**Problem:** Helper is `pub(crate)` and tested under feature build, but
no caller exists outside the test mod. The `#[allow(dead_code)]` is
documented as "wired by TASK-0357 follow-up" — but TASK-0357 itself
shipped, and no follow-up task is tracked. Shipping the bundle in this
state means the helper rots indefinitely.

**Proposed fix (Option A — preferred):**
```rust
// In recv_frame, after CRC verify and BEFORE bincode decode:
#[cfg(feature = "zero-copy")]
if header.is_archived() {
    // Hoist: re-use the aligned read for the uncompressed-archive path.
    // For the compressed path, we already have a `Vec<u8>` from
    // decompress_payload (no way to read directly into AlignedVec without
    // changing decompress signatures), so we still copy in
    // decode_archive_payload.
    return decode_archive_payload(&payload, FRAME_HEADER_SIZE + header.length as usize);
}
```
The honest hoist requires a more substantial recv_frame restructure
(read the header → read aligned if archived+uncompressed → read raw if
compressed-or-bincode). If the developer prefers minimal invasiveness:

**Proposed fix (Option B — strip):**
- Delete frame.rs:258-282 (`read_aligned_payload`).
- Delete the 5 tests `read_aligned_payload_*` in frame.rs:1718-1810.
- Delete the default-build absence test
  `read_aligned_payload_absent_in_default_build` (frame.rs:1814).
- Remove the dead-code-marker comments at frame.rs:251-257.
- Add a single line to `docs/DEFERRED-WORK.md` documenting the hoist
  as a future optimization (with citation back to this review).

Either resolution is acceptable; the developer chooses based on bundle
size budget. Pipeline state must NOT advance to QA until one is applied.

---

## 6. NICE-TO-HAVE items

### NTH-1: Refactor `build_transport_config` to a builder struct
**File:** `relativist-core/src/config.rs:534-588`
**Rationale:** The 8-arg signature is currently justified (only two
callers, 1:1 with CLI flags), but the next CLI flag will push to 9 and
the lint suppression becomes a hand-grenade. Pre-emptive refactor:
```rust
pub(crate) struct TransportConfigInputs<'a> {
    pub backend_str: &'a str,
    pub socket_path: Option<PathBuf>,
    pub no_tcp_nodelay: bool,
    pub send_buffer: usize,
    pub recv_buffer: usize,
    pub keepalive: u64,
    pub compression_threshold: usize,
    pub use_zero_copy: bool,
}

fn build_transport_config(
    inputs: TransportConfigInputs,
) -> Result<TransportConfig, RelativistError> { ... }
```
Callers pass a struct literal with named fields, lint suppression
disappears, future flags land cleanly. ~30 LoC change, zero behavior
change. Defer to next CLI-touching bundle.

### NTH-2: Deduplicate `assert_partition_eq` / `assert_stats_eq`
**Files:** `frame.rs:2020-2058`, `zero_copy_tests.rs:95-127`
**Rationale:** Both copies are identical and serve identical purposes.
Move to a `pub(crate) mod test_helpers` under `#[cfg(all(test, feature
= "zero-copy"))]`. Saves ~80 LoC of duplication and prevents drift.

### NTH-3: Audit `Partition` and `WorkerRoundStats` for `PartialEq` derivation
**Files:** `partition/types.rs`, `merge/types.rs`
**Rationale:** Helpers exist because these types lack `PartialEq`. The
inner `Net` already implements it (the helpers use `assert_eq!` on
`subnet`). HashMap derives `PartialEq` automatically. The only blocker
might be `f64` (in `WorkerRoundStats.reduce_duration_secs`, currently
compared via `to_bits()`). For Partition, derive `PartialEq` directly;
for WorkerRoundStats, derive `PartialEq` and document the f64 NaN
caveat in the doc-comment. Removes the helpers and prevents future
field-add bugs.

### NTH-4: Narrow `is_hot_path_message` visibility
**File:** `relativist-core/src/protocol/frame.rs:228`
**Change:** `pub fn is_hot_path_message` → `pub(crate) fn
is_hot_path_message`. Has only one caller in the same file.

### NTH-5: Narrow archive wrapper struct visibility
**File:** `relativist-core/src/protocol/frame.rs:199, 209`
**Change:** `pub struct ArchiveAssignPayload` → `pub(crate)` (or
`pub(super)`). `pub struct ArchivePartitionResultPayload` → same.
These wrappers are wire-format internals, never observed by callers
outside `protocol::frame`.

---

## 7. QA probes for Stage 5

### Q1 (HIGH PRIORITY): Pointer-corruption probe replacing the abandoned byte-flip approach
**Fixture:** Build a valid `AssignPartition` archive via `rkyv::to_bytes`
on a non-empty partition (4+ agents). Identify the relative-pointer
fields by archive layout inspection (or by binary diff between two
archives of partitions of differing sizes); flip the high bit of one
relative-pointer byte to force the validator to compute an out-of-buffer
offset.
**Expected:** `ProtocolError::ArchiveValidationFailed(reason)` where
`reason.contains("AssignPartition") || reason.contains("non-hot-path")`.
**Why:** Closes the corruption-surface gap the developer pre-flagged.
The undersized-payload test (T12) only exercises the schema-floor path;
this probe exercises the structural-validity path.

### Q2: FLAG_ARCHIVED on bincode payload (adversarial sender misuse)
**Fixture:** Hand-craft a frame: bincode-encode `Message::Shutdown`,
compute CRC, set `flags = FLAG_ARCHIVED` (NOT compressed). Send via
duplex.
**Expected (feature ON):** `ArchiveValidationFailed("non-hot-path
archive payload ...")` because rkyv::access on bincode bytes will fail
the structural check for both ArchiveAssignPayload and
ArchivePartitionResultPayload schemas.
**Expected (default):** `Deserialize` error (bincode discriminant mismatch).
**Why:** Defends against a misbehaving sender that toggles FLAG_ARCHIVED
without invoking the rkyv path.

### Q3: Cross-feature wire interop (the "feature-flip race")
**Fixture:** Two test binaries, one built with `--features zero-copy`,
one default. Use `tokio::io::duplex` to bridge them in-process (or two
processes connected by TcpListener for true wire interop):
- Default sender → feature-ON receiver: works (sends bincode, receiver
  decodes bincode; FLAG_ARCHIVED never set).
- Feature-ON sender (use_archive=true) → default receiver: receiver
  sees FLAG_ARCHIVED, falls into bincode decoder, returns `Deserialize`.
- Feature-ON sender (use_archive=false) → default receiver: works
  (sends bincode, receiver decodes bincode).
**Expected:** Behaviors per spec; default receiver's clean rejection
must NOT panic, must NOT mis-decode, must NOT wedge the channel.
**Why:** Operator misconfiguration (mixed-feature deployment) is a real
failure mode. This probe pins the failure surface.

### Q4: AlignedVec drop semantics on partial read failure
**Fixture:** Use `tokio_test::io::Builder` to construct a reader that
returns half the payload bytes then EOF. Drive `recv_frame` with
header indicating full payload size + FLAG_ARCHIVED.
**Expected:** `ProtocolError::ConnectionLost(io::Error{ ErrorKind::UnexpectedEof, ..})`,
NO panic, NO leak (verify via valgrind/Miri if available). The
partially-read AlignedVec must drop cleanly.
**Why:** Network failures mid-recv are common; rkyv types must not
introduce new failure modes vs. plain Vec<u8>.

### Q5: Round-trip `total_bytes` accounting on compressed-archive path
**Fixture:** Send AssignPartition with `use_archive=true, threshold=1`
(forces FLAG_ARCHIVED + FLAG_COMPRESSED). Capture `n_sent` from
`send_frame_v2`. Recv via `recv_frame`. Capture `n_recv`.
**Expected:** `n_sent == n_recv`. Specifically, `n_recv` should be
`FRAME_HEADER_SIZE + compressed_payload_len`, NOT
`FRAME_HEADER_SIZE + uncompressed_archive_len`.
**Why:** UT-0357-03 round-trips the compressed archive but does not
assert byte-count parity. F-1 in §3 above flags this. Without the
assertion, a future refactor could silently break wire-byte accounting
that downstream metrics rely on.

### Q6: Cross-platform alignment witness (DEFER if no aarch64 CI)
**Fixture:** Run T13 alignment battery on aarch64 (Linux ARM, macOS
M-series, or Windows ARM64). rkyv archive layouts are
endian/alignment-sensitive; we currently test only on x86_64.
**Expected:** All T13 sub-cases pass identically.
**Why:** rkyv's portable-archive guarantees are documented but not
locally verified. If the project ever ships ARM artefacts (likely for
Tailscale-distributed workers per ROADMAP 2.37-2.39), this is a hard
prerequisite. **Defer to Stage 5 if QA env permits; otherwise track in
DEFERRED-WORK.**

### Q7: rkyv 0.8.x patch-version drift simulation
**Fixture:** Pin `Cargo.lock` rkyv to 0.8.15, capture an archive of a
known partition. Then run `cargo update -p rkyv` to bump to 0.8.x
(latest). Re-run T11 with the captured archive bytes.
**Expected (per rkyv's compat policy):** Archive layout is stable across
0.8.x; round-trip succeeds. If it fails, document the rkyv version
constraint as `=0.8.15` in Cargo.toml and open an issue upstream.
**Why:** Defense against silent dependency drift. We carry rkyv via a
non-pinned `version = "0.8"`; if a patch release changes archive layout
(unlikely under semver but possible), every existing on-disk/in-flight
archive becomes invalid.

### Q8: Archive of a partition with `free_port_index` populated
**Fixture:** T11 currently uses `free_port_index = HashMap::new()` in
both `empty_partition` and `populated_partition`. Add a fixture where
`free_port_index` has 4-8 entries pointing to varied PortRef variants
(AgentPort + FreePort + Disconnected). Round-trip via archive path.
**Expected:** Round-trip identity holds for the HashMap; rkyv's HashMap
archive layout is non-trivial and warrants an explicit witness.
**Why:** F-3 / NTH-3 noted that helpers skip fields silently. The
HashMap field is the most likely place for a silent equality failure.

---

## 8. Pipeline-state update note

The brief's literal expectation that default-build receivers reject
FLAG_ARCHIVED with `ArchiveValidationFailed("zero-copy feature
disabled")` does NOT match the shipped behavior (bincode-fallthrough →
`Deserialize` error). The shipped behavior is consistent with DC-1's
Option B verdict and R19 forward-compat principles, but the reviewer
recommends amending the pipeline-state notes (and BACKLOG TASK-0357 if
it carries the literal expectation) to record the actual behavior so
future readers don't perceive drift. See F-9 above.

---

## 9. Summary

- **Verdict:** APPROVE WITH MUST-FIX (1 MUST-FIX, 5 NICE-TO-HAVE)
- **Spec compliance:** R20-R27 + R35-R37 all PASS
- **Architecture:** Dependency direction respected; feature gating
  correct; DC-1..DC-4 spec-critic verdicts honored
- **Code quality:** No `unsafe`, no `unwrap()` in production paths, no
  `println!`, no `access_unchecked`, comments explain WHY not WHAT
- **Test coverage:** 16 unconditional + 50 feature-gated tests, all green
- **MUST-FIX-1:** Resolve `read_aligned_payload` dead-code state (hoist
  preferred, strip acceptable, accept-with-deferred is NOT)
- **QA probes for Stage 5:** 8 enumerated (Q1 high-priority, Q6 defer-if-no-arm-CI)

Stage 4 REVIEW complete. Stage 5 QA dispatchable AFTER MUST-FIX-1 resolution.
