# CLOSURE-D010 — Spec Amendments A1..A8 Landing Log

**Date:** 2026-04-27
**Author:** especialista-specs
**Bundle:** D-010 Phase A — SPEC-21 Streaming Generation predecessor-spec amendment cluster (TASK-0510..0517)
**Branch:** v2-development
**Source of truth:** `specs/SPEC-21-streaming-generation.md` §3.8 (A1..A8 New text — read-only)

---

## Summary

8 amendments to 7 predecessor spec files, all verbatim (in normative content) from SPEC-21 §3.8. No code changes. No test changes. Mirrors the D-009 Phase A bundle (`01184f1`, 2026-04-27) in style, formatting, and process. SPEC-04 takes both A1 and A8 in the same edit pass on that file.

| Amendment | Task | Spec file | Edit location | Status |
|-----------|------|-----------|---------------|--------|
| A1 | TASK-0510 | `specs/SPEC-04-partition.md` | R12 replacement (line 85), header bump | LANDED |
| A2 | TASK-0511 | `specs/SPEC-06-wire-protocol.md` | R3a + R3b inserted after R3 (~line 60); `Message` enum gains `RequestWork`/`NoMoreWork` (after `RegisterNack` per R5) in §4.1 catalog | LANDED |
| A3 | TASK-0512 | `specs/SPEC-07-deployment.md` | R11 amended (line 96): GridConfig CLI surface gains `chunk_size`, `streaming_strategy`, `dispatch_mode`, `max_pending_lifetime` | LANDED |
| A4 | TASK-0513 | `specs/SPEC-09-benchmarks.md` | R2 amended (line 45): `Benchmark` trait gains `make_net_stream` with default impl + 9-row migration table | LANDED |
| A5 | TASK-0514 | `specs/SPEC-13-system-architecture.md` | R23a+R23b+R23c+R23d inserted after R23 (~line 378); pull-mode coord/worker FSMs + push-mode-unchanged guarantee + BSP-barrier semantics | LANDED |
| A6 | TASK-0515 | `specs/SPEC-22-arena-management.md` | R10b amendment block inserted after closure paragraph; R10c trigger broadened (~line 87..89) | LANDED |
| A7 | TASK-0516 | `specs/SPEC-19-delta-protocol.md` | R12b inserted after R12a (~line 110): `extend_with_chunk_borders` method signature + semantics + call-site discipline note | LANDED |
| A8 | TASK-0517 | `specs/SPEC-04-partition.md` | §4.5 head amendment paragraph (~line 282); split() unchanged + chunked pipeline additive clarification | LANDED |

---

## Amendment-by-Amendment Detail

### A1 — TASK-0510: SPEC-04 R12 — Border-id allocation policy for streaming pipeline

**File:** `specs/SPEC-04-partition.md`
**Status header:** Revised v3.2 (was v3.1)
**Edit:** R12 (line 85) replaced with the dual-path policy verbatim from SPEC-21 §3.8 A1 New text. The batch path retains the original `max_existing_freeport_id + 1` clause (AC-002 backward compatibility); the streaming path mandates either `0`-base monotonic increment (no Lafont FreePorts) or `max_lafont_freeport_id_in_first_batch + 1` (Lafont FreePorts in first batch). `Partition.border_id_start`/`border_id_end` (R15a) MUST be set to the global range. Cross-path test note about distinct-but-non-overlapping ranges added. Backward-pointer `> Amendment A1 (SPEC-21 §3.8 A1 / R29b)` appended below R12 with explicit cross-references to SPEC-21 §3.3 R17 (`generate_and_partition_chunked`) and §4.8 (partition border-id-range propagation).
**Backward-pointer:** SPEC-21 §3.8 A1 / R29b.
**Acceptance criteria check:**
- [x] R12 explicitly states the dual-path policy (batch vs streaming).
- [x] Cross-references SPEC-21 R29b and §4.8.
- [x] §4.5 documents the `Partition.border_id_start`/`border_id_end` global-range assignment.
- [x] Cross-path test discipline noted.

---

### A2 — TASK-0511: SPEC-06 `Message` enum — `RequestWork`/`NoMoreWork` variants + PROTOCOL_VERSION defensive sequencing

**File:** `specs/SPEC-06-wire-protocol.md`
**Status header:** Revised v3.1 (was v3); `Amends:` line added.
**Edit (R3a — variants):** New requirement R3a inserted after R3 (~line 60) declaring the two pull-dispatch variants `RequestWork { worker_id: WorkerId }` and `NoMoreWork`, with mode-agnostic-at-wire / mode-specific-at-FSM scoping clause and explicit reference to SPEC-21 R37e push-mode-emission prohibition.
**Edit (R3b — version bump):** New requirement R3b inserted immediately after R3a, mandating defensive `PREVIOUS_LIVE_VERSION + 1` language for the PROTOCOL_VERSION bump rather than a hardcoded absolute integer. Explicitly notes "= 6 at the time of writing, given the current value 5 from SPEC-22 D-009 Phase A landing" — same pattern as TEST-SPEC-0511 / TEST-SPEC-0576. `ProtocolError::UnsupportedVersion` rejection clause included. Cross-references SPEC-22 R9a / TASK-0476 precedent and SPEC-20 R37 v3-vs-v4 pattern.
**Edit (§4.1 catalog):** Two new variants appended at end of `Message` enum (after `RegisterNack`) per R5 discriminant-stability rule, with comment block flagging pull-dispatch scoping.
**Backward-pointer:** SPEC-21 §3.8 A2 / R31, R37c, R37e.
**Acceptance criteria check:**
- [x] `Message` enum text includes `RequestWork { worker_id: WorkerId }` and `NoMoreWork` variants (appended at end per R5).
- [x] PROTOCOL_VERSION clause uses defensive `PREVIOUS_LIVE_VERSION + 1` language (NOT a hardcoded integer).
- [x] Pre-bump deserializers reject post-bump payloads with `UnsupportedVersion` — explicit clause added.
- [x] Cross-references SPEC-21 R31, R37c; SPEC-22 R9a / TASK-0476 precedent.
- [x] `grep -c "RequestWork\|NoMoreWork" SPEC-06` returns 9 (was 0).

---

### A3 — TASK-0512: SPEC-07 R11 — GridConfig CLI surface gains streaming-pipeline fields

**File:** `specs/SPEC-07-deployment.md`
**Status header:** Revised v3.1 (was v3); `Amends:` line added.
**Edit:** R11 (line 96) amended to enumerate four additional GridConfig fields in the CLI-to-config mapping bullet:
- `chunk_size: u32` (SPEC-21 R24, default `10_000`)
- `streaming_strategy: StreamingStrategyConfig` (SPEC-21 R25, default `RoundRobin`)
- `dispatch_mode: DispatchMode` (SPEC-21 R34, default `Auto`)
- `max_pending_lifetime: u32` (SPEC-21 R37g, optional, default `16`)

Backward-pointer block clarifies that the canonical struct definition lives in SPEC-05 §4.1 (this requirement governs the CLI mapping only) and that the supporting enums `StreamingStrategyConfig` (variants `RoundRobin`, `Fennel`) and `DispatchMode` (variants `Push`, `Pull`, `Auto`) are declared in SPEC-21 §4.x and re-exported from `src/config.rs`. `chunk_size = 10_000` and `max_pending_lifetime = 16` defaults are flagged as benchmark-TBD per SPEC-21 SC-024.
**Backward-pointer:** SPEC-21 §3.8 A3 / R24, R25, R34, R37g.
**Acceptance criteria check:**
- [x] All four field bullets present with normative defaults and R-number references.
- [x] Benchmark-TBD calibration disposition noted for `chunk_size` and `max_pending_lifetime`.
- [x] Strategy enum normative variants enumerated.
- [x] DispatchMode enum normative variants enumerated.
- [x] Additive-change posture preserved (no break to SPEC-05 R-N MUSTs).

---

### A4 — TASK-0513: SPEC-09 R2 — `Benchmark` trait gains `make_net_stream` with default impl

**File:** `specs/SPEC-09-benchmarks.md`
**Status header:** Revised v3.1.2 (was v3.1.1); `Amends:` line added.
**Edit:** R2 (line 45) amended verbatim per SPEC-21 §3.8 A4 New text. Trait body now contains `fn make_net_stream(...)` with default implementation that wraps `make_net` via `default_chunked_iter` (lives in `src/bench/streaming.rs` per SPEC-21 §3.2 R10). Doc-comment threads R10/R11/R12 cross-references; T6 isomorphism contract noted. Migration table (9 rows) appended in amendment block enumerating per-benchmark override status (MUST/SHOULD/DEFAULT) per SPEC-21 R12. Total Phase B effort estimate (~30 LoC + per-generator overrides) preserved.
**Backward-pointer:** SPEC-21 §3.8 A4 / R10, R11, R12.
**Acceptance criteria check:**
- [x] R2 trait amended with `make_net_stream` default-impl method.
- [x] Default impl materializes via `make_net` and slices via `default_chunked_iter`.
- [x] Cross-references SPEC-21 R10, R11, R12.
- [x] Documents that all 13 existing implementations remain valid without per-implementation edits.
- [x] Migration table present with override status per benchmark.
- [x] R10 ↔ R11 isomorphism contract (T6, SPEC-21 §7.2) cited.

---

### A5 — TASK-0514: SPEC-13 §3.5/§3.6 — Coordinator + Worker FSMs gain pull-mode states

**File:** `specs/SPEC-13-system-architecture.md`
**Status header:** Revised v2.1 (was v2); `Amends:` line added.
**Edit (R23a — coordinator pull-mode states):** Inserted after R23 (~line 378). Adds 5 new states (`DispatchingFirst`, `AwaitingResults`, `GeneratingNext`, `SendingNoMoreWork`, `AwaitingFinalResults`) with explicit transition table (7 transitions) verbatim per SPEC-21 §3.8 A5 New text. State enum extension shown as Rust comment block. Gating on `DispatchMode::Pull` (`GridConfig.dispatch_mode == DispatchMode::Pull`, SPEC-21 R34 / SPEC-07 R11) explicitly stated.
**Edit (R23b — worker pull-mode states):** Inserted as part of the same block. Adds 2 new states (`AwaitingChunkAfterResult`, `FinalReduction`) with 4-row transition table verbatim per SPEC-21 §3.8 A5 New text.
**Edit (R23c — push-mode-unchanged guarantee):** Inserted as part of the same block. Mandates that R19/R21 coordinator transitions and R24/R25 worker transitions are UNCHANGED in push mode; workers MUST NOT add defensive `NoMoreWork` handling to push-mode tables; coordinators MUST NOT emit `NoMoreWork` in push mode. Critical for backward compatibility (1181-test scenario preservation).
**Edit (R23d — BSP-barrier semantics):** Inserted as part of the same block. Workers MAY emit `PartitionResult` individually but coordinator MUST NOT begin `merge()` until all post-`NoMoreWork` final `PartitionResult` messages received. Reduces pull dispatch to single logical BSP round, preserving D6 and G1.
**Backward-pointer:** SPEC-21 §3.8 A5 / R30, R32, R37d, R37e.
**Acceptance criteria check:**
- [x] Coordinator FSM extended with 5 pull-mode states + transitions.
- [x] Worker FSM extended with 2 pull-mode states + transitions.
- [x] Push-mode-unchanged guarantee (R37e) explicit.
- [x] Pull-only states gated on `DispatchMode::Pull` per SPEC-21 R34 / SPEC-07 R11.
- [x] BSP-barrier semantics under pull dispatch (R37d) explicit.
- [x] Cross-references SPEC-06 R3a (wire-level variants).

---

### A6 — TASK-0515: SPEC-22 R10b/R10c — Trigger condition broadened to `(delta_mode || streaming_active)`

**File:** `specs/SPEC-22-arena-management.md`
**Status header:** Reviewed v2.1 (was v2); `Amends:` line gains SPEC-21 §3.8 A6 entry.
**Edit (R10b broadening):** Amendment block inserted immediately after the closing `R10b prevents the recycle in step 2.` paragraph. New text broadens the trigger from `delta_mode == true` to `(delta_mode || streaming_active) && id ∈ border_referenced_set`. Threat model under streaming-alone (no delta) explicitly documented (border_map populated by the streaming pipeline serves as the same active-tracking surface). Strategy A and Strategy B both extend identically: under A, workers MUST NOT pop while `streaming_active`; under B, `border_referenced_set` is sourced from streaming `border_map` when `streaming_active && !delta_mode`. Conceptual rename (`DisableUnderBorderTracking` / `BorderClean`) noted, but wire-level enum name `RecyclePolicy::DisableUnderDelta` preserved for backward compatibility (SC-013 / DISC-012 stale-tag precedent). Alternative one-liner closure via `streaming-no-recycle` cargo feature gate documented. Closes SC-007.
**Edit (R10c extension):** Statement extended verbatim to OR-in the streaming border_map as a triggering surface, matching R10b's broadened scope.
**Backward-pointer:** SPEC-21 §3.8 A6 / R37b.
**Acceptance criteria check:**
- [x] R10b trigger condition broadened from `delta_mode == true` to `(delta_mode || streaming_active) && id ∈ border_referenced_set`.
- [x] Wire-level enum name `RecyclePolicy::DisableUnderDelta` preserved.
- [x] `streaming-no-recycle` cargo feature gate documented as valid one-liner closure.
- [x] R10c protected-tombstone semantics extend identically to streaming-active context.
- [x] Cross-references SPEC-21 R37b, §3.8 A6.

---

### A7 — TASK-0516: SPEC-19 §3.2 — `BorderGraph::extend_with_chunk_borders` method addition

**File:** `specs/SPEC-19-delta-protocol.md`
**Status header:** Draft, dual amendment marker added (`§3.2 amended per SPEC-21 §3.8 A7 (BorderGraph gains extend_with_chunk_borders method)`); `Amends:` line gains SPEC-21 entry.
**Edit:** New requirement R12b inserted immediately after R12a (~line 110). Method signature `pub fn extend_with_chunk_borders(&mut self, new_borders: &HashMap<u32, (PortRef, PortRef)>)` declared verbatim per SPEC-21 §3.8 A7 New text. Semantic clauses: merge new entries (one fresh `BorderState` per pair, matching R10 initialization convention); MUST be called by coordinator after each `install_connection` invocation (SPEC-21 §4.6) yielding a border wire under `delta_mode && streaming_active`, BEFORE next chunk's `AssignPartition`; idempotent on previously-seen border IDs (preserves R11-applied deltas); no-op on empty input. Ownership-split note (SPEC-19 owns implementation; SPEC-21 owns call-site discipline; production task is TASK-0588) included.
**Backward-pointer:** SPEC-21 §3.8 A7 / R37f.
**Acceptance criteria check:**
- [x] Method signature `extend_with_chunk_borders(&mut self, &HashMap<u32, (PortRef, PortRef)>)` declared.
- [x] Idempotency on previously-seen border IDs documented.
- [x] No-op semantics on empty input documented.
- [x] Call-site discipline note pointing to SPEC-21 R37f present.
- [x] Cross-references SPEC-21 R37f, §3.8 A7.

---

### A8 — TASK-0517: SPEC-04 §4.5 — split() UNCHANGED, chunked pipeline additive (clarifying amendment)

**File:** `specs/SPEC-04-partition.md`
**Status header:** combined with A1 in v3.2 bump.
**Edit:** Single-paragraph amendment block inserted at the head of §4.5 (immediately after the section heading, before the `fn split(...)` signature) clarifying that `split()` is UNCHANGED (same semantics, same R-numbers R6/R12/R16-R18/R28); the chunked pipeline (`generate_and_partition_chunked`, SPEC-21 §3.3 R17) is an ALTERNATIVE entry point selected by `GridConfig.chunk_size != u32::MAX`; the two paths produce structurally compatible output (`PartitionPlan` from `split()`, `ChunkedPartitionResult { partitions, borders, stats }` from streaming, convertible to `PartitionPlan` per SPEC-21 R20-R21); `split()` is the fallback for the v1 backward-compat path under SPEC-21 R26 short-circuit. Closes SC-001 part 4.
**Backward-pointer:** SPEC-21 §3.8 A8 / SC-001 part 4.
**Acceptance criteria check:**
- [x] §4.5 explicitly states that `split()` is UNCHANGED.
- [x] §4.5 cross-references SPEC-21 §3.3 R17, R20-R21, R26.
- [x] §4.5 documents non-overlapping but coexistent entry points.
- [x] R-numbers R6/R12/R16-R18/R28 explicitly preserved.

---

## Backward-Pointer Summary (all 8 amendments carry backward-pointers)

| Amendment | Backward-pointer to SPEC-21 |
|-----------|----------------------------|
| A1 | §3.8 A1 / R29b |
| A2 | §3.8 A2 / R31, R37c, R37e (variants) + R37c (PROTOCOL_VERSION) |
| A3 | §3.8 A3 / R24, R25, R34, R37g |
| A4 | §3.8 A4 / R10, R11, R12 |
| A5 | §3.8 A5 / R30, R32, R37d, R37e |
| A6 | §3.8 A6 / R37b |
| A7 | §3.8 A7 / R37f |
| A8 | §3.8 A8 / SC-001 part 4 |

---

## Defensive Version-Bump Compliance (TASK-0511 specific)

The PROTOCOL_VERSION bump in SPEC-06 R3b uses defensive sequencing language verbatim per task constraint:

> "PROTOCOL_VERSION ← PREVIOUS_LIVE_VERSION + 1 (= 6 at the time of writing, given the current value 5 from SPEC-22 D-009 Phase A landing)."

This matches the precedent in TEST-SPEC-0511 / TEST-SPEC-0576 (defensive style), and prevents merge-order reshuffling between SPEC-20 / SPEC-21 / SPEC-22 from silently producing wrong absolute version numbers. The constant declaration site (SPEC-18 §4.7 / R28) is **not** edited in this bundle — that is a wire-format-spec-owned decision and is captured by TASK-0576 (production wave, Phase B).

---

## Invariant Table / Cross-Reference Updates

| Invariant / Surface | Location | Change |
|---------------------|----------|--------|
| SPEC-04 R12 (border-id allocation) | line 85 | Dual-path policy: batch unchanged, streaming added |
| SPEC-04 §4.5 | line ~282 | Additive note: split() unchanged, chunked pipeline alternative |
| SPEC-06 `Message` enum | §4.1 catalog | `RequestWork`, `NoMoreWork` appended (R5 discriminant stability) |
| SPEC-06 PROTOCOL_VERSION | R3b (new) | Defensive `PREVIOUS_LIVE_VERSION + 1` language |
| SPEC-07 R11 (CLI-to-config) | line 96 | 4 new GridConfig fields enumerated |
| SPEC-09 R2 (Benchmark trait) | line 45 | `make_net_stream` with default impl added |
| SPEC-13 §3.5 coord FSM | R23a (new) | 5 pull-mode states + transitions |
| SPEC-13 §3.6 worker FSM | R23b (new) | 2 pull-mode states + transitions |
| SPEC-13 push-mode posture | R23c (new) | UNCHANGED guarantee + gating on `DispatchMode::Pull` |
| SPEC-13 BSP barrier (pull) | R23d (new) | Single-logical-round barrier semantics (D6, G1) |
| SPEC-19 §3.2 BorderGraph | R12b (new) | `extend_with_chunk_borders` method signature + semantics |
| SPEC-22 R10b trigger | line ~87 | `delta_mode` → `(delta_mode || streaming_active)` |
| SPEC-22 R10c trigger | line 89 | OR-in streaming border_map surface |

---

## Verification

- All 8 amendments landed verbatim (verified by grep against SPEC-21 §3.8 New text).
- 7 spec files modified: SPEC-04, SPEC-06, SPEC-07, SPEC-09, SPEC-13, SPEC-19, SPEC-22 (SPEC-04 takes both A1 and A8).
- SPEC-21 itself NOT modified (canonical source, read-only — task constraint).
- No files outside `specs/` or `docs/spec-reviews/` modified — `git diff --stat` confirms.
- No `relativist-core/src/` or `relativist-core/tests/` changes — task constraint observed.
- Status headers updated on all 7 files; `Amends:` frontmatter added/extended on all 7 files referencing SPEC-21 §3.8.
- `grep -c "RequestWork\|NoMoreWork" specs/SPEC-06-wire-protocol.md` returns 9 (was 0; success criterion ≥ 2 met).
- `grep -c "PREVIOUS_LIVE_VERSION" specs/SPEC-06-wire-protocol.md` returns 2 (defensive language verified).
- `grep -c "extend_with_chunk_borders" specs/SPEC-19-delta-protocol.md` returns 3 (signature + 2 prose mentions).
- `grep -c "streaming_active" specs/SPEC-22-arena-management.md` returns 4.
- Total diff: 7 files, 180 insertions, 13 deletions.

---

## Recommended Next Action

D-010 Phase A (TASK-0510..0517) is complete. Proceed to Phase B: Streaming Generation Core (TASK-0520..0524 + Phase B/C/D/E onwards) — developer agent, TDD RED→GREEN→REFACTOR. Test baseline entering Phase B: 1464 default / 1507 zero-copy (unchanged; these are spec-only edits). The PROTOCOL_VERSION bump itself happens in TASK-0576 (production), depending on TASK-0476 (SPEC-22 wire-version-bump precedent already landed in D-009 Phase B).
