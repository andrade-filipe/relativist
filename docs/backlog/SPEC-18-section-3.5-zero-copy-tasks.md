# Bundle: SPEC-18 §3.5 — Zero-Copy Archive (rkyv on hot path) (item 2.24)

**Created:** 2026-04-16
**Owner:** task-splitter (orchestrated by sdd-pipeline)
**Stage:** 1.5 SPEC-CRITIC — complete (2026-04-16, 0 spec amendments,
  3 task amendments applied + 2 cascades; verdict at
  `docs/spec-reviews/SPEC-18-section-3.5-design-choices-2026-04-16.md`).
  **Stage 2 TESTS unblocked.**
**Test baseline before bundle:** 887 lib + 4 integration (post-SPEC-19 §3.1 ship).
**Hard floor (CLAUDE.md):** 887 lib tests post-bundle, with AND without `--features zero-copy`.
**Estimated total LoC:** ~600 across 8 atomic tasks (each <200 LoC).
**Tier 1 break-even path (V2-FEATURE-MATRIX):** confirmed next after item 2.34
  (`2.22 → 2.23 → 2.34 → 2.24 → 2.25 → 2.35 → 2.26`).
**Closes:** DEFERRED-WORK row D-002 (acceptance signal: hot-path `AssignPartition`
  with `--features zero-copy` produces a frame with `FLAG_ARCHIVED` set, the
  receiver decodes via `rkyv::access` (validating API), and round-trip identity
  `deserialize(access(to_bytes(p))) == p` holds).

## Scope (in vs out)

**In scope (SPEC-18 §3.5, R20-R27 + §3.9 R36-R37 + §7.2 T11-T14):**
- R20 — `zero-copy` cargo feature gates the rkyv path. **MUST NOT** be a default
  feature. When the feature is off, all serialization uses bincode v2 only
  (current behaviour, no regression).
- R21 — derive `Archive` / `rkyv::Serialize` / `rkyv::Deserialize` on the 8
  listed types under `#[cfg_attr(feature = "zero-copy", derive(...))]`:
  `Net`, `Partition`, `CompactSubnet`, `Agent`, `Symbol`, `PortRef`, `IdRange`,
  `WorkerRoundStats`.
- R22 — rkyv archive path is **only** for hot-path messages (`AssignPartition`,
  `PartitionResult`). Control messages (`Shutdown`, `Error`, `Register`,
  `RegisterAck`, `RegisterNack`) MUST always use bincode v2.
- R23 — `send_frame` rkyv path: `rkyv::to_bytes::<rkyv::rancor::Error>(&value)`
  → set `FLAG_ARCHIVED` → optional LZ4 compression on top (compression and
  archive flags MAY both be set).
- R24 — `recv_frame` rkyv path: when `FLAG_ARCHIVED` is set, (1) decompress if
  `FLAG_COMPRESSED` is also set, (2) validate CRC32C against the **uncompressed**
  payload (R12 invariant preserved), (3) call `rkyv::access::<ArchivedPartition>(&payload)`
  — the **safe, validating** API; **MUST NOT** use `rkyv::access_unchecked` in
  production code. (4) Use the archived ref directly OR call `rkyv::deserialize`.
- R25 — receive buffer for archived frames MUST be 16-byte aligned. SHOULD use
  `rkyv::util::AlignedVec` instead of `vec![0u8; len]` when `FLAG_ARCHIVED` is set.
- R26 — rkyv validation failure (malformed archive, alignment error, OOB) MUST
  surface as `ProtocolError::ArchiveValidationFailed(String)`. Non-hot-path
  messages with `FLAG_ARCHIVED` set MUST also be rejected with this variant.
- R27 — round-trip identity: for every valid `Partition` value `p`,
  `deserialize(access(to_bytes(p))) == p`. T11-T14 verify this.
- R36, R37 — `TransportConfig.use_zero_copy: bool` (default `false`) +
  `--use-zero-copy` CLI flag, gated under `#[cfg(feature = "zero-copy")]`.

**Hard scope boundaries (out of scope — task-splitter MUST NOT generate tasks
for these):**
- Modifying v1 wire format or v1 protocol path.
- Touching SPEC-19 delta protocol code (§3.2/§3.3 are items 2.35/2.26).
- Making `zero-copy` a default feature (R20 explicit MUST NOT).
- Using `rkyv::access_unchecked` in production code (R24 step 3 explicit MUST NOT).

## R12 ordering invariant (load-bearing — preserved across rkyv path)

CRC32C is **always** computed on the uncompressed payload, even when both
`FLAG_ARCHIVED` and `FLAG_COMPRESSED` are set. The recv pipeline order:
**decompress → CRC verify → rkyv access**. QA Probe 4 from the SPEC-18 §3.1-3.4
ship pinned this for the bincode path; TASK-0357 carries the same ordering into
the rkyv branch and TEST-SPEC-0357 will assert it explicitly.

## Wire compatibility (additive, not breaking)

v2 wire format is shipped (item 2.23). `FLAG_ARCHIVED` (bit 1) is currently
defined as a const but reserved (QA Probe 3 confirmed it errors out as
`ProtocolError::Deserialize` when no rkyv path exists, because the bytes are
fed to bincode and fail). Activating it is purely additive:
- v2 receivers WITH `--features zero-copy` accept frames with `FLAG_ARCHIVED`
  set (and route them through `rkyv::access`).
- v2 receivers WITHOUT `--features zero-copy` MUST cleanly reject frames with
  `FLAG_ARCHIVED` set, with `ProtocolError::ArchiveValidationFailed`. See the
  `FLAG_RESERVED` design choice section below.

## Design choice flagged for spec-critic — `FLAG_RESERVED` mask

The current `FLAG_RESERVED = 0b1111_1100` includes bit 1 (FLAG_ARCHIVED) by
design — that bit was reserved when SPEC-18 §3.1-3.4 shipped. With this
bundle activating bit 1, the mask must change. Two options:

**Option A — feature-conditional FLAG_RESERVED:**
```rust
#[cfg(feature = "zero-copy")]
pub const FLAG_RESERVED: u8 = 0b1111_1100; // bit 1 stays in reserved? NO — this is wrong
// Correct A: bit 1 is NEVER reserved; mask is always 0b1111_1100 with both
// FLAG_COMPRESSED + FLAG_ARCHIVED defined.
```
On reflection, Option A as originally drafted in the brief is incoherent. The
clean version of A is: **the mask is unchanged (bit 1 is already excluded from
0b1111_1100, since 0b1111_1100 = bits 2..7 only)**. Re-reading SPEC-18 §4.2:
`FLAG_RESERVED = 0b1111_1100` already excludes bits 0 (FLAG_COMPRESSED) and
1 (FLAG_ARCHIVED). So bit 1 is **NOT** in the reserved mask — it is a defined
flag with no implementation behind it yet (a "reserved-by-implementation" bit
masquerading as a defined flag).

**Option B — mask unchanged, recv_frame routes FLAG_ARCHIVED explicitly:**
- Keep `FLAG_RESERVED = 0b1111_1100` (already excludes bit 1).
- `recv_frame` checks: if `header.is_archived()`:
  - When `cfg(feature = "zero-copy")`: route to rkyv path.
  - When `cfg(not(feature = "zero-copy"))`: return
    `ProtocolError::ArchiveValidationFailed("zero-copy feature disabled".into())`
    immediately, before allocating the payload buffer.
- This means current behaviour for QA Probe 3 (`Deserialize` error when
  `FLAG_ARCHIVED + FLAG_COMPRESSED` set) **changes** to
  `ArchiveValidationFailed`. The probe must be updated as part of TASK-0357.

**Recommendation: Option B.** Rationale: SPEC-18 §4.2 already wrote
`FLAG_ARCHIVED` and `FLAG_RESERVED` as mutually-exclusive constants; the
existing mask is already correct. Option B requires zero const changes and
yields a clearer typed error (`ArchiveValidationFailed` distinguishes "we
understand this flag but cannot honour it" from "we don't know this flag").
QA Probe 3 of the prior bundle becomes a positive test for the new error
path under `cfg(not(feature = "zero-copy"))`.

**Spec-critic decision needed before TASK-0357 lands.** If spec-critic
prefers a different shape (e.g., `UnknownFlags` instead of
`ArchiveValidationFailed` when feature is off), TASK-0357 acceptance criteria
will be updated and TEST-SPEC-0357 regenerated.

## Task graph (DAG)

```
                          TASK-0352 (S, ~30)
                                │
              ┌─────────────────┼─────────────────┐
              ▼                 ▼                 ▼
          TASK-0353         TASK-0354         TASK-0358
          (M, ~150)         (S, ~30)          (S, ~50)
              │                 │
              │   ┌─────────────┘
              │   │
              ▼   │
          TASK-0355 (S, ~50)
              │
              │   ┌─────────────┐
              │   │             │
              ▼   ▼             ▼
          TASK-0356        TASK-0357
          (M, ~100)        (M, ~120)
              │                 │
              └────────┬────────┘
                       ▼
                  TASK-0359 (S, ~70)
```

| ID | Title | Spec Reqs | Size | LoC est. | Depends |
|------|-------|-----------|------|----------|---------|
| 0352 | `Cargo.toml`: rkyv optional dep + `zero-copy` feature gate | R20 | S | ~30 | none |
| 0353 | Derive `Archive`/`rkyv::Serialize`/`rkyv::Deserialize` on 8 types | R21 | M | ~150 | 0352 |
| 0354 | `ProtocolError::ArchiveValidationFailed(String)` tuple variant (per spec-critic DC-2, 2026-04-16) | R26, R35 | S | ~30 | 0352 |
| 0355 | 16-byte aligned receive buffer (`rkyv::util::AlignedVec`) | R25 | S | ~50 | 0352, 0353 |
| 0356 | `send_frame` archive path (hot-path only, optional LZ4) | R22, R23 | M | ~100 | 0353, 0354 |
| 0357 | `recv_frame` archive path (decompress → CRC → `rkyv::access`) | R12, R22, R24, R26 | M | ~120 | 0354, 0355 |
| 0358 | `TransportConfig.use_zero_copy` + `--use-zero-copy` CLI flag | R36, R37 | S | ~50 | 0352 |
| 0359 | T11-T14 test suite (round-trip, corrupt rejection, alignment, hot-path-only) | R27, T11-T14 | S | ~70 | 0356, 0357 |

**Total:** ~600 LoC, all <200 per task. No cycles. Implementable in topological
order: 0352 → {0353, 0354, 0358} → 0355 → {0356, 0357} → 0359.

## Per-task cargo-feature compatibility

Every task individually preserves the dual-build contract: the codebase MUST
compile and pass all tests both with `cargo test --workspace` (default
features) and with `cargo test --workspace --features zero-copy`.

- **TASK-0352:** opt-in feature flag. Without `--features zero-copy`, no rkyv
  code path is reachable. Default feature set unchanged.
- **TASK-0353:** all derives gated on `#[cfg_attr(feature = "zero-copy", ...)]`.
  Without the feature, only existing serde derives compile.
- **TASK-0354:** `ArchiveValidationFailed` variant is unconditional in the
  enum (so error pattern matches stay exhaustive in both builds), but it is
  only constructed by code paths that themselves are `#[cfg(feature = "zero-copy")]`
  — in the no-feature build the variant is unreachable but not dead-code-warned
  (variants are public API).
- **TASK-0355:** `AlignedVec` use is gated on `#[cfg(feature = "zero-copy")]`;
  the no-feature build keeps the existing `vec![0u8; len]` branch.
- **TASK-0356, TASK-0357:** all rkyv-path code branches are
  `#[cfg(feature = "zero-copy")]`. Without the feature, the branches are
  not compiled and `recv_frame` rejects `FLAG_ARCHIVED` with
  `ArchiveValidationFailed("zero-copy feature disabled")` (Option B above).
- **TASK-0358:** CLI flag is gated on `#[cfg(feature = "zero-copy")]` so it
  does not appear in `--help` for default builds. `TransportConfig.use_zero_copy`
  field is unconditional but defaults to `false` and is only read by the rkyv
  send path (gated).
- **TASK-0359:** all T11-T14 tests are gated on `#[cfg(feature = "zero-copy")]`
  so the default test count stays at 887. With `--features zero-copy`, target is
  905+ (T11-T14 yields ~18 tests).

## Spec ambiguities flagged for spec-critic — RESOLVED 2026-04-16

All four flagged choices were resolved by spec-critic on 2026-04-16
with **0 spec amendments**. Verdict file:
`docs/spec-reviews/SPEC-18-section-3.5-design-choices-2026-04-16.md`.

| DC | Choice | Verdict | Task amendment |
|----|--------|---------|----------------|
| DC-1 | `FLAG_RESERVED` mask interpretation | Option B (mask unchanged; `recv_frame` routes `FLAG_ARCHIVED` explicitly) — spec already says this; `frame.rs` already implements it | None (TASK-0345 already shipped Option B; TASK-0357 acceptance criteria already extend `recv_frame` along Option B) |
| DC-2 | `ArchiveValidationFailed` variant shape | **Tuple** `ArchiveValidationFailed(String)` per R35 verbatim — spec is explicit, not shape-agnostic | TASK-0354 + cascade to TASK-0356/0357/0358/0359 |
| DC-3 | `recv_frame` archive flag discrimination | Try-then-try with **Assign-first** ordering + mandated `// SPEC-18 R22 discrimination` source comment | TASK-0357 (ordering rule + comment requirement added to acceptance criteria) |
| DC-4 | rkyv send-side `rancor::Error` mapping | **Conflate** into `ArchiveValidationFailed` with mandatory `"serialize: "` reason-string prefix | TASK-0356 (mapping pinned + prefix mandate added to acceptance criteria) |

The §4.6 note on `CompactSubnet` and rkyv ("rkyv serializes the `Partition`
directly") makes it clear that the rkyv path bypasses the
`serialize_subnet_compact` adapter and operates on the full `Net` including
arena. TASK-0353 records this and the rkyv derives go on `Net` itself, not
on `CompactSubnet`'s wire form.

## Acceptance gate for the whole bundle

- All 8 tasks shipped GREEN through Stages 3-6.
- `cargo test --workspace` test count ≥ 887 lib tests (CLAUDE.md hard floor)
  and ≥ 4 integration tests.
- `cargo test --workspace --features zero-copy` test count ≥ 905 lib tests
  (target +18 from T11-T14).
- `cargo clippy --workspace --all-targets -- -D warnings` clean both with and
  without `--features zero-copy`.
- `cargo fmt --check` clean.
- Release smoke `compute add 3 5 → 8` works (default features).
- Release smoke with `cargo build --release --features zero-copy` succeeds.
- Round-trip `deserialize(access(to_bytes(p))) == p` holds for every test
  partition (T11, R27).
- DEFERRED-WORK row D-002 closed (move from "Active Deferrals" to "Resolved
  Deferrals").

## Stage advancement

- **Stage 1 SPLITTING:** complete (this file + 8 TASK-NNNN.md files).
- **Stage 1.5 SPEC-CRITIC:** complete (2026-04-16) — 0 spec amendments,
  3 task amendments applied (TASK-0354 tuple variant per DC-2;
  TASK-0356 send-side error mapping with `"serialize: "` prefix per DC-4;
  TASK-0357 try-then-try with Assign-first ordering + mandated source
  comment per DC-3) + 2 cascade documentation amendments (TASK-0358 and
  TASK-0359 narrative references to the variant brought into tuple-form
  alignment). Verdict at
  `docs/spec-reviews/SPEC-18-section-3.5-design-choices-2026-04-16.md`.
  Each amended task carries a "## Spec-Critic Amendments (2026-04-16)"
  section recording the change-set and rationale.
- **Stage 2 TESTS:** **UNBLOCKED** — ready to dispatch `test-generator`
  with bundle spec = SPEC-18 §3.5 (R20-R27, R36-R37) + §7.2 (T11-T14),
  task list = TASK-0352..0359, deliverable = `docs/tests/TEST-SPEC-0352.md`
  … `TEST-SPEC-0359.md`. Test contracts MUST encode:
  - Tuple variant form `ArchiveValidationFailed(String)` everywhere
    (DC-2 mandate).
  - Send-side error mapping `format!("serialize: {}", e)` with the
    literal `"serialize: "` prefix (DC-4 mandate, source-grep
    verifiable).
  - Assign-first try-then-try ordering in `recv_frame` discrimination
    + the literal `// SPEC-18 R22 discrimination` source comment
    (DC-3 mandate, source-grep verifiable).
- Orchestrator pause requested by parent: STOP after Stage 1.5 and
  confirm before Stage 2 dispatch.
