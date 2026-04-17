# Bundle: SPEC-19 §3.4 — Wire Protocol Extensions (item 2.26-A)

**Created:** 2026-04-17
**Owner:** task-splitter (orchestrated by sdd-pipeline)
**Stage:** 1.5 SPEC-CRITIC — complete; see
  [`docs/spec-reviews/SPEC-19-section-3.4-design-choices-2026-04-17.md`](../spec-reviews/SPEC-19-section-3.4-design-choices-2026-04-17.md).
  Task-updater amendments required (TASK-0366 serde derive + test,
  TASK-0369 invariant guard + 2 tests, test-count trajectory
  refresh); then Stage 2 TESTS.
**Test baseline before bundle:** 968 lib default / 1008 `--features
  zero-copy` (post SPEC-19 §3.2 ship, pipeline-state 2026-04-17).
**Hard floor (CLAUDE.md):** 690 lib tests post-bundle; v1 baseline
  must never decrease.
**Expected final counts:** 988 lib default / 1028 `--features
  zero-copy` (+20 default / +20 zero-copy across 6 tasks).
**Estimated total LoC:** ~685 across 6 atomic tasks (15 + 150 + 130 +
  130 + 180 + 80).
**Tier 1 break-even path (V2-FEATURE-MATRIX):** first of 4 sub-bundles
  under item 2.26; unblocks coordinator dispatch (2.26-B), worker
  lifecycle (2.26-C), and invariant amendments + config (2.26-D).

## Scope (in vs out)

**In scope (SPEC-19 §3.4, R31-R37):**
- R31 — three new C→W `Message` variants: `InitialPartition` (disc 7),
  `RoundStart` (disc 8), `FinalStateRequest` (disc 10).
- R32 — two new W→C `Message` variants: `RoundResult` (disc 9),
  `FinalStateResult` (disc 11).
- R33 — `BorderDelta` struct shape (already shipped by TASK-0362 in
  `merge/border_graph.rs`; this bundle re-exports it to `protocol/`).
- R34 — serde + bincode v2 round-trip identity + CRC32C integrity
  for all 5 variants.
- R35 — large-payload variants (`InitialPartition`,
  `FinalStateResult`) benefit from CompactSubnet + varint + LZ4
  compression via the existing `send_frame_with_threshold` path.
- R36 — small-payload variants (`RoundStart`, `RoundResult`,
  `FinalStateRequest`) skip compression below threshold (SHOULD).
- R37 — discriminant stability: new variants appended after
  `RegisterNack` (disc 6); byte-level regression test added.

**Out of scope (separate sub-bundles under item 2.26):**
- 2.26-B — coordinator dispatch loop invoking the new variants
  (R13-R15 wire-protocol call sites; the protocol-layer dispatcher
  that calls `send_frame_with_threshold` with each variant; border
  graph update on `RoundResult` receipt).
- 2.26-C — stateful worker lifecycle (R20-R30 worker-side delta
  emission, `previous_border_state` tracking, `reduce_all` + diff
  loop under delta mode).
- 2.26-D — `GridConfig.delta_mode` flag (R20), invariant amendments
  (R38 / G1 reformulation, R40 / D3/D6), operational requirements
  (R41-R42).
- rkyv zero-copy archive path for the 5 new variants — SPEC-18 R22
  explicitly restricts `FLAG_ARCHIVED` to `AssignPartition` /
  `PartitionResult`; the delta variants ride the bincode path.
- The delta BSP loop (`run_grid_delta`); SPEC-19 §3.3 + §4.3.
- `WorkerRoundStats.has_border_activity` field — already shipped in
  §3.1 bundle (TASK-0348).

## Wire change in scope

This bundle is a **breaking additive change** to the `Message` enum:
5 new variants at discriminants 7..=11, appended after SPEC-10's
registration variants (disc 4..=6). Both sides of the wire (coordinator
and worker) are rebuilt together, so the additive change does not break
existing tests. The discriminant positions are fresh (SPEC-18 did NOT
add variants; current enum still has 7 variants at discriminants 0..=6,
confirmed by reading `relativist-core/src/protocol/types.rs` lines
23-74 on 2026-04-17).

**Protocol version bump:** `PROTOCOL_VERSION` remains at 2 (SPEC-18's
bump). The delta-protocol variants are ONLY sent when
`GridConfig.delta_mode == true` (to land in sub-bundle 2.26-D); absent
that flag the full-partition v1 variants still flow. A mixed deployment
runs the same PROTOCOL_VERSION but with `delta_mode` coordinated at
startup by the coordinator. Pre-startup compatibility check ships
under 2.26-D.

## Tasks

| ID | Title | Deps | LoC |
|----|-------|------|-----|
| [TASK-0366](TASK-0366.md) | Re-export `BorderDelta` from `protocol` + module doc alignment | — | 15 |
| [TASK-0367](TASK-0367.md) | `Message::InitialPartition` (disc 7) + `Message::FinalStateResult` (disc 11) — large-payload variants | TASK-0366 | 150 |
| [TASK-0368](TASK-0368.md) | `Message::RoundStart` (disc 8) + `Message::FinalStateRequest` (disc 10) — small C→W variants | TASK-0366, TASK-0367 | 130 |
| [TASK-0369](TASK-0369.md) | `Message::RoundResult` (disc 9) — W→C small-payload variant | TASK-0366, TASK-0367, TASK-0368 | 130 |
| [TASK-0370](TASK-0370.md) | Wire integration — `send_frame` round-trip + CRC32 + compression thresholds for all 5 variants | TASK-0366, TASK-0367, TASK-0368, TASK-0369 | 180 |
| [TASK-0371](TASK-0371.md) | Discriminant stability lock-in test — byte-level assertions for variants 0..=11 | TASK-0367, TASK-0368, TASK-0369 | 80 |

Total: 685 LoC across 6 tasks.

## Task DAG

```
TASK-0366 (re-export)
    |
    v
TASK-0367 (disc 7 + 11)
    |
    v
TASK-0368 (disc 8 + 10)
    |
    v
TASK-0369 (disc 9) -------+
    |                     |
    v                     v
TASK-0370 (wire)    TASK-0371 (discriminant lock-in)
    |                     |
    +----------+----------+
               v
         Stage 4 REVIEW
```

TASK-0370 and TASK-0371 are parallelisable after TASK-0369 lands.

## Test-count trajectory

| After task | default lib | zero-copy lib |
|-----------|------------:|--------------:|
| baseline  |         968 |          1008 |
| TASK-0366 |         968 |          1008 |
| TASK-0367 |         972 |          1012 |
| TASK-0368 |         975 |          1015 |
| TASK-0369 |         979 |          1019 |
| TASK-0370 |         987 |          1027 |
| TASK-0371 |         988 |          1028 |

## Flagged design choices for Stage 1.5 spec-critic

Three items flagged at SPLITTING time; all three ruled on in
[`docs/spec-reviews/SPEC-19-section-3.4-design-choices-2026-04-17.md`](../spec-reviews/SPEC-19-section-3.4-design-choices-2026-04-17.md).

**Spec-critic verdicts (summary):**
- **DC-A1** — Option A (`BorderDelta` stays in `merge/`, re-exported).
  Forced by SPEC-13 R6-R8 layering. TASK-0366 amendment: add
  `serde::Serialize, serde::Deserialize` derives to the existing
  struct in `merge/border_graph.rs` (pre-existing defect in
  TASK-0362) + one new round-trip test.
- **DC-A2** — Option C (duplicate per R26 literal, but
  graph-enforce equality via debug_assert + regression test; mark
  `stats.has_border_activity` as canonical source of truth).
  TASK-0369 amendment: doc comment tightening + 1 equality test +
  1 `#[ignore]` regression stub to enable in sub-bundle 2.26-C.
- **DC-A3** — Option A (no cargo feature gate). SPEC-18 R20
  precedent only covers opt-in external deps; feature-gating
  internal variants would create match-arm cfg viral hazard and
  mixed-feature cluster footgun. No task amendment.

**Test-count trajectory refresh (post DC-A1 + DC-A2 amendments):**
| After task | default lib | zero-copy lib |
|-----------|------------:|--------------:|
| baseline  |         968 |          1008 |
| TASK-0366 |         969 |          1009 |
| TASK-0367 |         973 |          1013 |
| TASK-0368 |         976 |          1016 |
| TASK-0369 |         982 |          1022 |
| TASK-0370 |         990 |          1030 |
| TASK-0371 |         991 |          1031 |

**Expected final counts (post-amendment):** 991 lib default / 1031
`--features zero-copy` (+23 default / +23 zero-copy across 6 tasks).

**Bonus findings** (see spec-review §"Bonus: under-specified
R31-R37 requirements flagged"):
1. R35 "benefit" is not observable; Stage 2 TESTS should add one
   `frame_len < bincode_len` assertion on the large-payload
   compression test (non-blocking).
2. R37 "coordinated with SPEC-18" is effectively a no-op (SPEC-18
   added zero `Message` variants); TASK-0371 doc comment should
   note the fresh assignment.
3. R34 silence on `FLAG_ARCHIVED` for delta variants is already
   correctly handled by TASK-0370 (SPEC-18 R22 whitelist).

---

**Original task-splitter flags preserved below for reference:**

### DC-A1: `BorderDelta` lives in `merge/`, re-exported to `protocol/`

**Options:**
- **A (chosen):** keep `BorderDelta` in `merge/border_graph.rs` (where
  `BorderGraph` already consumes it) and add a `pub use` re-export
  `pub use crate::merge::BorderDelta;` to the protocol layer.
- **B:** move the struct definition into `protocol/types.rs` (or a
  new `protocol/border_delta.rs`) and re-export backwards to `merge/`.

**Rationale for A:** `merge/` is pure-core (SPEC-13, SPEC-19 §3.2 R19).
`BorderGraph` consumes `BorderDelta` as a primitive input; forcing
`merge/` to depend on `protocol/` would invert the allowed dependency
direction. Re-export is idiomatic Rust and costs zero runtime.

**Spec-critic flag:** please confirm this reading of R19 / SPEC-13
R6-R8. If the policy is stricter (e.g. "struct MUST live in protocol"),
TASK-0366 inverts the re-export direction — LoC impact negligible.

### DC-A2: `has_border_activity` duplicated on `RoundResult` + `WorkerRoundStats`

**Options:**
- **A (chosen, follows R26 verbatim):** `RoundResult` carries both a
  top-level `has_border_activity: bool` AND `stats.has_border_activity`
  (the §3.1-bundle field on `WorkerRoundStats`).
- **B:** drop the top-level field; coordinator reads
  `stats.has_border_activity`.

**Rationale for A:** R26 enumerates the top-level field. Cost: 1 extra
bincode-encoded byte per `RoundResult`. Benefit: coordinator can match
`Message::RoundResult { has_border_activity, .. }` without pulling
`stats` out of the pattern. Developer ergonomics > 1 byte.

**Spec-critic flag:** please confirm whether the duplication is
intentional or whether R26 is a pre-§3.1-bundle draft that predates
the `stats.has_border_activity` addition. If B, TASK-0369 drops the
field — LoC impact -5.

### DC-A3: No feature-gating on the new variants

**Options:**
- **A (chosen):** new variants are always compiled into the `Message`
  enum. The `delta_mode` config flag (2.26-D) controls whether the
  coordinator sends them; absent the flag, the variants are
  unreachable on the wire but still in the enum.
- **B:** feature-gate the variants behind a `delta-protocol` cargo
  feature.

**Rationale for A:** the variants are cheap (empty discriminant slots
in the enum; no runtime overhead for un-sent variants). Feature-gating
would split the build matrix (+1 feature combination to test),
complicate the discriminant-stability test (variants 7..=11 would
conditionally disappear), and offer no benefit because the flag
already gates the behaviour.

**Spec-critic flag:** please confirm. If B is required, TASK-0366 adds
a `delta-protocol` feature to `relativist-core/Cargo.toml` and all
variant additions gain `#[cfg(feature = "delta-protocol")]`. LoC
impact: +10 on TASK-0366; complicates TASK-0371 (requires feature-gated
test bodies).

## Constraints

- All 6 tasks are atomic (each <200 LoC per CLAUDE.md).
- No `unwrap()` in production code.
- `#[derive(Debug, Clone, Serialize, Deserialize)]` on payload types
  (matches existing `Message` enum; no `PartialEq`/`Eq` on `Message`
  itself — see TASK-0367 note).
- Discriminant assignments 7..=11 match SPEC-19 R31-R32 exactly. No
  collision with existing discriminants 0..=6 (verified against
  `protocol/types.rs` on 2026-04-17).
- 690-test hard floor per CLAUDE.md; 968-test current floor per
  pipeline-state. Zero regression expected.
- Pure-core invariant (R19 from §3.2) preserved: `BorderDelta` stays
  in `merge/`; protocol layer re-exports without inversion.

## Discriminant verification (as of 2026-04-17)

`relativist-core/src/protocol/types.rs` has 7 variants:
| Disc | Variant |
|:---:|---------|
| 0 | `AssignPartition` |
| 1 | `Shutdown` |
| 2 | `PartitionResult` |
| 3 | `Error` |
| 4 | `Register` |
| 5 | `RegisterAck` |
| 6 | `RegisterNack` |

SPEC-18 §3.5 did NOT add any `Message` variants (confirmed by
`grep -n 'Register\|Round\|Final\|InitialPartition' specs/SPEC-18-wire-format-v2.md`:
all hits are references to existing variants or the discriminant-encoding
rule). High-water-mark is **6**; this bundle assigns 7..=11 without
collision.

## Pipeline handoff

After all 6 tasks ship, Stage 2 TESTS (test-generator) produces
TEST-SPEC-0366..0371, Stage 3 DEV (developer) implements with TDD,
Stage 4 REVIEW (reviewer), Stage 5 QA (qa), Stage 6 REFACTOR complete
the 6-stage SDD loop.

**Next sub-bundle:** 2.26-B (coordinator dispatch, R13-R15) — consumes
the wire variants shipped here.
