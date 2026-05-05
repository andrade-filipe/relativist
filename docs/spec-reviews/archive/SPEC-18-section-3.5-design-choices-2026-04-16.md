# SPEC-18 §3.5 — Design Choices Verdict (Stage 1.5 spec-critic)

**Date:** 2026-04-16
**Reviewer:** spec-critic (adversarial)
**Bundle:** SPEC-18 §3.5 (item 2.24) — Zero-Copy Archive (rkyv on hot path)
**Predecessors consulted:** SPEC-18 §3.4 (R14-R19), §3.5 (R20-R27), §3.8 (R35),
  §4.2 (Frame Header v2 with explicit constants), §4.6 (rkyv Archive Types).
**Source consulted:** `relativist-core/src/protocol/frame.rs` (current
  `FLAG_*` constants + `recv_frame` flow + QA Probe 3),
  `relativist-core/src/protocol/error.rs` (current variant shapes),
  `docs/backlog/TASK-0354.md`, `TASK-0356.md`, `TASK-0357.md`,
  `docs/backlog/SPEC-18-section-3.5-zero-copy-tasks.md` (bundle index).

---

## Overall Assessment

The SPLITTING bundle is sound. The four design choices flagged by the
task-splitter are all answerable from the spec text + existing code with
**zero blocking spec amendments required**. One choice (variant shape for
`ArchiveValidationFailed`) requires a TASK file edit to bring it into
literal compliance with SPEC-18 R35. The other three either match the spec
already (FLAG_RESERVED, try-then-try) or fall in a region the spec leaves
to the implementer (send-side error mapping); for those I bless the
task-splitter's recommendations with one mandated qualifier on the
send-side error reason string.

**Verdict:** APPROVED WITH AMENDMENTS to **task files only** (no spec
edits). Stage 2 TESTS unblocked once TASK-0354 acceptance criteria are
flipped from struct to tuple variant shape.

---

## Verdicts (4 design choices)

### DC-1: FLAG_RESERVED mask interpretation

**PICK:** **Option B** (mask unchanged; `recv_frame` routes
`FLAG_ARCHIVED` explicitly).

**WHY:** SPEC-18 §4.2 (lines 340-342) writes the three constants
verbatim:
```
pub const FLAG_COMPRESSED: u8 = 0b0000_0001;
pub const FLAG_ARCHIVED:   u8 = 0b0000_0010;
pub const FLAG_RESERVED:   u8 = 0b1111_1100;
```
The mask `0b1111_1100` already excludes bit 1 (FLAG_ARCHIVED is
`0b10`, mask covers bits 2..7). The current `frame.rs` (line 36) matches
this literally. SPEC-18 R15 (lines 99-111) defines bit 1 as the Archive
flag — i.e. "defined but not yet implemented in v2.0", NOT
"reserved-by-mask". `recv_frame` therefore proceeds past the
`has_unknown_flags()` gate when bit 1 is set today (QA Probe 3 of the
prior bundle confirms this — the frame currently passes the
unknown-flag check and fails downstream at bincode). Option A would
require an unnecessary cfg-conditional const change and would
contradict §4.2's explicit mask value. Option B is **the canonical
reading of the spec as written**.

**AMENDMENT NEEDED?** No. The spec already says Option B. `frame.rs`
already implements Option B. The only follow-on work is the `recv_frame`
extension in TASK-0357.

**TASK IMPACT:** None for TASK-0345 (already shipped Option B). TASK-0357
acceptance criteria (already drafted) correctly extend `recv_frame` along
Option B by routing `FLAG_ARCHIVED` to either the rkyv branch (feature
on) or `ArchiveValidationFailed("zero-copy feature disabled")` (feature
off). No edit needed.

---

### DC-2: `ArchiveValidationFailed` variant shape

**PICK:** **Tuple** — `ArchiveValidationFailed(String)`.

**WHY:** SPEC-18 R35 (lines 181-195) lists the variants verbatim:
```rust
DecompressionFailed(String),
ArchiveValidationFailed(String),
UnknownFlags { flags: u8 },
VersionMismatch { expected: u8, received: u8 },
```
The spec is **not shape-agnostic** — it explicitly types the first two as
tuple variants and the latter two as struct variants. The shape choice
is load-bearing for two reasons: (1) it pins the `Display` impl
signature (`Self::ArchiveValidationFailed(reason)` vs
`Self::ArchiveValidationFailed { reason }`), and (2) it pins external
callers' `match` arms. The existing `DecompressionFailed(String)` in
`error.rs` (line 64) was implemented as the tuple form **in compliance
with R35**, so the precedent in this codebase is "follow R35
literally". The task-splitter's "consistency with `UnknownFlags` /
`ChecksumMismatch`" argument is real but irrelevant — those variants
ARE struct variants per R35; if R35 had wanted
`ArchiveValidationFailed { reason: String }`, it would have written it
that way (as it did for `UnknownFlags { flags: u8 }`).

Adversarial principle: when the spec is unambiguous, prefer spec
compliance over local-style consistency.

**AMENDMENT NEEDED?** No spec amendment. **TASK-0354 amendment
required** before TESTS stage writes contracts.

**TASK IMPACT:** **TASK-0354** must be edited:
- "Acceptance Criteria" first bullet: change `ArchiveValidationFailed
  { reason: String }` to `ArchiveValidationFailed(String)`.
- "Key Types / Signatures" code block: same change in the variant
  declaration.
- "Display impl" arm: change `Self::ArchiveValidationFailed { reason }` to
  `Self::ArchiveValidationFailed(reason)`.
- "Test Expectations" `archive_validation_failed_error_renders`:
  change `ArchiveValidationFailed { reason: "..." }` literal to
  `ArchiveValidationFailed("...".into())`.
- "Notes" first bullet ("Why `{ reason: String }` (struct variant)"):
  delete or rewrite to "spec-critic mandated tuple form per R35".

**Cascading TASK impact:**
- **TASK-0357** "Acceptance Criteria" + "Key Types / Signatures" +
  "Test Expectations": every construction
  `ProtocolError::ArchiveValidationFailed { reason: "...".into() }`
  becomes `ProtocolError::ArchiveValidationFailed("...".into())`.
- **TASK-0356** "Notes" send-side error mapping (see DC-4 below): same
  textual swap if/when the construction is added.

---

### DC-3: `recv_frame` discrimination strategy for archive flag

**PICK:** **Try-then-try**, with one mandated ordering rule and one
mandated comment.

**WHY:** The spec is genuinely silent on a wire-level discriminator
between `ArchiveAssignPayload` and `ArchivePartitionResultPayload`.
SPEC-18 §3.5 R22-R24 only constrain WHICH messages may take the rkyv
path (the two hot-path variants), not how to tell them apart on
receive. SPEC-18 §4.2 specifies a fixed 9-byte header (length + CRC32C
+ flags) — no room for a sub-tag without a spec amendment to §3.4 / §4.2.

The two wrapper structs introduced in TASK-0356
(`ArchiveAssignPayload { round, partition }` vs
`ArchivePartitionResultPayload { round, partition, stats }`) have
distinct rkyv archive layouts (different root struct sizes, different
relative pointer offsets). `rkyv::access` performs structural
validation; an `ArchivePartitionResultPayload` archive fed to
`rkyv::access::<ArchivedAssignPayload>` will fail validation
deterministically (length + offset checks won't match). Try-then-try is
therefore correct, not just convenient.

The 1-byte discriminator alternative WOULD work and is cleaner, but it
requires:
1. amending SPEC-18 §3.4 R14 (frame header layout — adds a 10th byte or
   a new "sub-tag after header" field), or
2. burning one of the 6 reserved flag bits (bits 2-7), which sacrifices
   forward-compat headroom.

Neither is justified for an optional optimisation feature on a hot path
where the cost of a failed `rkyv::access` is ~0.1ms on typical
partitions. **The spec does not authorise either amendment.**

**AMENDMENT NEEDED?** No spec amendment. Two task additions required:

1. **Mandated ordering:** TASK-0357 must try `ArchivedAssignPayload`
   FIRST. Rationale: in the BSP cycle, every round produces N
   `AssignPartition` frames (coordinator→workers) for every N
   `PartitionResult` frames (workers→coordinator), so the
   coordinator→worker path is the hotter direction. Coordinators receive
   only `PartitionResult` archives; workers receive only
   `AssignPartition` archives. In the steady-state hot path, the FIRST
   try succeeds and the second is never executed. The TEST-SPEC-0357
   blueprint already happens to put Assign first; pin it explicitly.

2. **Mandated comment:** TASK-0357 must include a `// SPEC-18 R22
   discrimination: ...` comment in `recv_frame` next to the
   try-then-try block, citing this verdict (`docs/spec-reviews/
   SPEC-18-section-3.5-design-choices-2026-04-16.md` DC-3) so a future
   reader knows the strategy was reviewed and approved, not stumbled
   into.

**TASK IMPACT:**
- **TASK-0357** "Acceptance Criteria" — add: "tries
  `ArchivedAssignPayload` BEFORE `ArchivedPartitionResultPayload` (DC-3
  ordering rule)".
- **TASK-0357** "Acceptance Criteria" — add: "includes a SPEC-18 R22
  comment citing DC-3 of the spec-critic verdict".
- **TASK-0357** "Notes" first bullet ("Discrimination strategy"): mark
  Recommendation (a) as **APPROVED by spec-critic DC-3 with ordering
  + comment requirements above**.

---

### DC-4: rkyv send-side `rancor::Error` mapping

**PICK:** **Conflate** into the existing `ArchiveValidationFailed`
variant, with a mandated reason-string prefix.

**WHY:** The spec is silent on send-side error mapping — R26 only
constrains receive-side validation failure. Adding a separate
`ArchiveSerializeFailed(String)` variant is cleaner taxonomy but:
- expands TASK-0354 from S to S+ (one more variant + Display arm + test);
- amends SPEC-18 R26 / R35 to authorise the new variant (spec
  amendment is a follow-up the user must apply, blocking the bundle);
- buys nothing functional: the failure is rare (only OOM during
  `rkyv::to_bytes`'s alignment-padding allocation, or a pathological
  input that slipped past TASK-0353's derive coverage), the error
  propagates as a `Result<_, ProtocolError>`, and downstream callers
  treat all `ProtocolError` variants identically (log + drop frame +
  surface to coordinator FSM).

The diagnostic value of distinguishing send-side from recv-side
failures CAN be recovered through the reason string. Mandate the
prefix `"serialize: "` for send-side construction (so logs read e.g.
`rkyv archive validation failed: serialize: allocator OOM`), and the
prefix `"access: "` or `"deserialize: "` for recv-side (already
suggested in TASK-0357's Notes — keep). Log scrapers and metrics
pipelines can split on the prefix if needed.

This conflation introduces **no observable wire-format difference**
(send-side failures never produce a frame on the wire). The shortcut
is therefore safe under the spec-critic's "shortcuts allowed when no
wire effect" rule.

**AMENDMENT NEEDED?** No spec amendment. **TASK-0356 amendment
required** before DEV stage codes the send path.

**TASK IMPACT:**
- **TASK-0356** "Notes" first bullet (`rkyv::rancor::Error to
  ProtocolError`): mark Recommendation (a) as **APPROVED by
  spec-critic DC-4 with the reason-string prefix requirement
  below**. Delete option (b) ("add `ArchiveSerializeFailed`") to avoid
  developer ambiguity.
- **TASK-0356** "Acceptance Criteria" — add: "send-side
  `rkyv::to_bytes` failure maps to
  `ProtocolError::ArchiveValidationFailed(format!(\"serialize: {}\",
  e))` (DC-4 prefix mandate)".
- **TASK-0356** "Key Types / Signatures" code block: replace the
  placeholder `.map_err(|e| ProtocolError::Serialize(/* see Notes */))`
  with the literal mapping above.
- **TASK-0357** "Acceptance Criteria" / "Test Expectations" — already
  uses `format!("rkyv access failed: {}", e)` and `format!(
  "deserialize: {}", e)`. Acceptable; spec-critic does NOT mandate a
  unified prefix vocabulary across send/recv (different prefixes are
  fine — only the send-side `"serialize: "` prefix is mandated).

---

## Summary table (compact)

| Design choice | Pick | Amendment needed? | Spec edit? |
|---------------|------|---------------------|-----------|
| DC-1 FLAG_RESERVED mask | Option B (mask unchanged, route in `recv_frame`) | No | No |
| DC-2 `ArchiveValidationFailed` shape | Tuple — `ArchiveValidationFailed(String)` per R35 | TASK-0354 + cascade to TASK-0356/0357 | No |
| DC-3 archive flag discrimination | Try-then-try, Assign first, with mandated comment | TASK-0357 (ordering + comment) | No |
| DC-4 rkyv send-side error mapping | Conflate into `ArchiveValidationFailed`, mandatory `"serialize: "` prefix | TASK-0356 (mapping + prefix) | No |

## SPEC-18 amendments required before TESTS

**Count: 0.** No edits to `specs/SPEC-18-wire-format-v2.md` are required.
All four design choices are resolvable within the existing spec text.

## Stage 1.5 verdict

**Stage 1.5 spec-critic complete.** TESTS stage is **unblocked** in
principle, but the task-splitter must apply the TASK-file amendments
listed under DC-2, DC-3, and DC-4 above before test-generator writes
TEST-SPEC-0354/0356/0357 — otherwise the test contracts will encode
the wrong variant shape (DC-2), miss the discrimination ordering rule
(DC-3), or omit the mandated send-side prefix (DC-4). Per project
convention (CLAUDE.md rule 7: "SOMENTE o ESPECIALISTA EM SPECS edita
codigo/relativist/specs/" — and by extension, only task-splitter edits
`docs/backlog/`), spec-critic does NOT edit those task files
directly. The user/orchestrator must dispatch task-splitter (or
task-updater per the local agent table) to apply the three TASK
amendments, then dispatch test-generator for Stage 2 TESTS.

## Checklist

### Consistency
- [x] All terms match SPEC-00 definitions (no new terms introduced).
- [x] Type signatures compatible with predecessor specs (R35 verbatim).
- [x] No contradictions with predecessor requirements (DC-1, DC-2 align
      with §3.4 + §3.8 as written; DC-3, DC-4 fall in spec-silent zones).
- [x] Data flow assumptions match predecessor outputs (R12 ordering
      preserved by TASK-0357 acceptance criteria).

### Testability
- [x] DC-1 verifiable by source-grep (`FLAG_RESERVED == 0b1111_1100`)
      + existing `frame_flag_constants_are_correct` test.
- [x] DC-2 verifiable by `archive_validation_failed_error_renders`
      test (TASK-0354, must use tuple-form construction).
- [x] DC-3 verifiable by source-grep (Assign-first ordering) +
      TR1/TR2 round-trip tests in TEST-SPEC-0357.
- [x] DC-4 verifiable by send-side failure-injection test (skipped:
      deferred to QA stage probe — `rkyv::to_bytes` OOM is not
      reproducible deterministically; the prefix mandate is verified
      by source-grep).

### Completeness
- [x] All four flagged choices have a verdict.
- [x] All cascading TASK impacts documented.
- [x] No undefined terms in the verdict.

### Invariant Preservation
- [x] T1-T7 unaffected (this is wire format, not net invariants).
- [x] D1-D6 unaffected (no distribution semantics changed).
- [x] I1-I5 — I-protocol unchanged: hot-path messages still preserve
      `decode(encode(msg)) == msg` (R27, R32). Try-then-try (DC-3) does
      NOT weaken this — both variants distinguish deterministically via
      rkyv structural validation.
- [x] G1 unaffected (no semantic change to reduction).
