# SPEC-19 §3.4 — Design Choices Verdict (Stage 1.5 spec-critic)

**Date:** 2026-04-17
**Reviewer:** spec-critic (adversarial)
**Bundle:** SPEC-19 §3.4 (item 2.26-A) — Wire Protocol Extensions (R31-R37)
**Predecessors consulted:**
  SPEC-19 §3.2 R8-R19 (BorderGraph; `BorderDelta` ships in
  `merge/border_graph.rs`), SPEC-19 §3.3 R20-R30 (delta protocol —
  worker-side semantics, R26's `RoundResult` shape in particular),
  SPEC-19 §3.4 R31-R37 (this bundle),
  SPEC-06 R5 (append-only discriminants),
  SPEC-13 R6-R8 (layer dependency direction: `protocol/` → `merge/`
  allowed; reverse illegal),
  SPEC-18 R9 (compression threshold at the frame layer),
  SPEC-18 R20 (feature-gating precedent for the opt-in `zero-copy`
  dependency).
**Source consulted:**
  `relativist-core/src/protocol/types.rs` (7 variants at discriminants
  0..=6 confirmed),
  `relativist-core/src/protocol/frame.rs` (`send_frame`,
  `send_frame_with_threshold`, `DEFAULT_COMPRESSION_THRESHOLD`,
  `FLAG_COMPRESSED`),
  `relativist-core/src/merge/border_graph.rs` L117-131
  (`BorderDelta` struct with `#[derive(Debug, Clone, Copy, PartialEq,
  Eq)]`),
  `relativist-core/src/merge/mod.rs` L13
  (`pub use border_graph::{AddBorderEntry, BorderDelta, BorderGraph,
  BorderState};`),
  `relativist-core/src/merge/types.rs` L149
  (`WorkerRoundStats.has_border_activity: bool` — already shipped in
  SPEC-19 §3.1 bundle),
  `relativist-core/src/merge/grid.rs` L32, L127-136, L1599
  (canonical convergence read path:
  `stats.iter().all(|s| !s.has_border_activity)` — the per-worker
  stats field is the authoritative source of truth),
  `relativist-core/Cargo.toml` L100-104 (the `zero-copy` feature gate
  precedent: opt-in ONLY because it pulls an optional `dep:rkyv`;
  no precedent for feature-gating internal variants),
  `docs/backlog/TASK-0366.md` .. `TASK-0371.md`,
  `docs/backlog/SPEC-19-section-3.4-wire-extensions-tasks.md` (bundle
  index).
**Precedent consulted:**
  `docs/spec-reviews/SPEC-19-section-3.2-design-choices-2026-04-17.md`
  (format + verdict style; DC-4 Option B ruling on invariant-at-
  boundary — the same principle governs DC-A2 here).

---

## Overall Assessment

The SPLITTING bundle is sound. All three design choices resolve
cleanly against the existing spec text and the existing codebase
**with zero blocking spec amendments required**. DC-A1 (placement)
and DC-A3 (feature gating) are settled by SPEC-13 R6-R8 (layering)
and SPEC-18 R20 precedent (feature gates are for opt-in external
deps, not internal variant pruning) respectively — the task-
splitter's defaults are correct. DC-A2 (duplication) is the only
genuine trade-off. I side with the task-splitter on **keeping the
duplicated field per R26 verbatim**, but add a mandatory
**graph-enforced invariant assertion** (Option C) that closes the
drift-vulnerability the task-splitter left open — direct analogue of
DC-4's Option B ruling in the §3.2 verdict.

**Verdict:** APPROVED WITH AMENDMENTS to **task files only** (no spec
edits). Stage 2 TESTS unblocked once TASK-0369 carries the new
`test_round_result_activity_matches_stats_activity` regression test
and the doc-comment wording for the duplicated field is tightened to
mark `stats.has_border_activity` as the canonical source of truth.

---

## Verdicts (3 design choices)

### DC-A1: `BorderDelta` placement (merge vs protocol)

**PICK:** **Option A** — keep `BorderDelta` in `merge/border_graph.rs`;
add a `pub use crate::merge::BorderDelta;` re-export from
`protocol/types.rs` (or `protocol/mod.rs`) so the new `Message`
variants can name it as `crate::protocol::BorderDelta`.

**WHY:** This decision is forced by SPEC-13's layering rule, not by
ergonomics or taste:

1. **SPEC-13 R6-R8 (dependency direction).** `merge/` is pure-core.
   `protocol/` is the async/tokio layer. The allowed edge is
   `protocol/` → `merge/`. The reverse edge is illegal. `BorderDelta`
   is already consumed by `BorderGraph::apply_deltas` inside `merge/`
   (TASK-0362, shipped). If we moved the struct definition to
   `protocol/`, `merge/` would have to import from `protocol/` — a
   direct SPEC-13 violation. There is no way to "cleanly" relocate the
   struct without inverting the forbidden edge.

2. **Existing code already picked Option A.** `merge/border_graph.rs`
   L125-131 defines the struct with `#[derive(Debug, Clone, Copy,
   PartialEq, Eq)]` and a rich doc comment tying the field to
   SPEC-19 R11/R17 and the `DISCONNECTED` sentinel; `merge/mod.rs`
   L13 already publicly re-exports it as `crate::merge::BorderDelta`.
   R33's struct shape (with `serde::Serialize, serde::Deserialize`)
   is achievable by adding the two serde derives at the definition
   site — a trivial amendment to TASK-0366 (see "Task impact" below).

3. **Semantic reading of R33.** The task-splitter's flag asks whether
   R33's phrasing "The `BorderDelta` type MUST be defined as: ..."
   mandates `protocol/` as the definition site. It does not. R33
   mandates **shape**, not **module**. SPEC-19 §3.2 already uses
   `BorderDelta` as the payload of `apply_deltas` (pure-core
   primitive) — the shape exists to serve BOTH the pure-core
   `apply_deltas` consumer AND the wire format. A re-export from
   `merge/` to `protocol/` is the only topology that serves both
   without violating SPEC-13.

4. **Idiomatic Rust.** `pub use` re-exports are zero-runtime-cost;
   rustdoc renders them as first-class items at the re-export path;
   downstream users can name either path. The "ergonomics" argument
   Option B implies ("wire type should live in `protocol/`") is
   folkore, not a real cost.

Option B (move to `protocol/`) was never viable — it would require
a SPEC-13 layering amendment, which would cascade through every
other pure-core module. Decisively rejected.

**AMENDMENT NEEDED?** No spec amendment. **TASK-0366 amendment
required** (small): ensure the re-export + serde derive story is
explicit, and fix a latent defect in the existing struct definition.

**TASK IMPACT:**

- **`merge/border_graph.rs` L125**. The existing `BorderDelta`
  definition derives `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`
  but **NOT `Serialize, Deserialize`** — yet R33 mandates both serde
  derives. This is a pre-existing defect in TASK-0362's output that
  blocks TASK-0368 / TASK-0369 compilation (the variants carrying
  `Vec<BorderDelta>` fields will fail `#[derive(Serialize,
  Deserialize)]` on `Message`). **TASK-0366 acceptance criteria must
  amend the struct derives:**

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq,
           serde::Serialize, serde::Deserialize)]
  pub struct BorderDelta {
      pub border_id: u32,
      pub new_target: PortRef,
  }
  ```

  Both `u32` and `PortRef` already implement `Serialize + Deserialize`
  (PortRef via SPEC-18's compact encoding, TASK-0344). No further
  work.

- **TASK-0366 acceptance criterion "No change to `BorderDelta`'s
  definition in `merge/border_graph.rs`"** — this bullet is WRONG
  as written. Replace with: "Extend `BorderDelta`'s `#[derive(...)]`
  to add `serde::Serialize, serde::Deserialize` per R33. All other
  fields and doc comments remain unchanged."

- **TASK-0366 acceptance criterion "No new tests in this task"** —
  amend to: "One new test in `merge::border_graph::tests`:
  `test_border_delta_bincode_roundtrip` — construct a `BorderDelta`
  with a non-trivial `PortRef::AgentPort(...)` target, encode via
  `bincode_v2::encode`, decode, assert equality. This locks R33's
  round-trip property at the struct level; the variant-level
  round-trips in TASK-0368 / 0369 are additive evidence. Test count
  goes from +0 to +1; final count: **969 default / 1009 zero-copy**."

- **Bundle index test-count table:** shift every downstream row by
  +1: TASK-0366 ends at 969/1009 (not 968/1008); TASK-0367 at
  973/1013; TASK-0368 at 976/1016; TASK-0369 at 980/1020; TASK-0370
  at 988/1028; TASK-0371 at 989/1029. The "Expected final counts"
  line becomes: **989 lib default / 1029 `--features zero-copy`**.

- **TASK-0367 "Test Expectations"** — note that the `BorderDelta`
  round-trip test has been lifted into TASK-0366; TASK-0367's
  `Partition`-shaped round-trips remain unchanged.

---

### DC-A2: `RoundResult.has_border_activity` redundancy

**PICK:** **Option C** — duplicate per spec literal (R26 enumerates
the field at the `RoundResult` level), **BUT** add a
graph-enforced invariant: `RoundResult::has_border_activity MUST
equal stats.has_border_activity` at encode time, enforced by a
debug_assert in a constructor helper, and verified by a new
regression test. Mark `stats.has_border_activity` as the canonical
source of truth in the doc comment.

**WHY:** This is the only DC where the task-splitter's default
(Option A — blind duplication per spec literal) leaves a latent
drift vulnerability. The §3.2 DC-4 verdict set the precedent:
**derived bits must be graph-enforced at the primitive boundary, not
caller-trusted.** The same reasoning applies here.

**Argument for shipping duplication (not Option B):**

1. **R26 is normative and unambiguous.** R26 (line 146-151) literally
   enumerates four fields at the same indentation level: `round`,
   `border_deltas`, `stats`, `has_border_activity`. The SPEC-19 §3.1
   bundle amended `WorkerRoundStats` (merge/types.rs L149) to add
   the same `has_border_activity` field after R26 was written;
   without reading §3.1 history, R26 looks naturally-written, not
   vestigial. The bundle index's hypothesis that R26 "predates §3.1"
   is plausible but **not provable** from the spec text alone — and
   the adversarial principle here is: when the spec is explicit,
   follow it; do not second-guess.

2. **Wire cost is negligible.** bincode v2 encodes a `bool` as a
   single byte. One extra byte per `RoundResult` per round per
   worker. For a 100-worker grid running 10 rounds, this is 1 KB
   over the entire protocol — noise compared to the MiB-scale
   payloads of `InitialPartition` / `FinalStateResult`.

3. **Ergonomics favour the duplication.** Coordinator code in
   sub-bundle 2.26-B will want to match
   `Message::RoundResult { has_border_activity, .. }` directly
   without destructuring the nested `stats` field. The §3.1 bundle's
   convergence check (`merge/grid.rs` L32, L1599) currently reads
   `stats.iter().all(|s| !s.has_border_activity)` — when the
   coordinator-side delta loop ships in 2.26-B, it will need the
   same check on the incoming `RoundResult` messages. Having the
   field at the top level saves one `.stats` access per iteration.

**Argument AGAINST blind duplication (the drift vulnerability):**

1. The worker code path that constructs `RoundResult` is two lines
   apart in the source (`protocol/worker.rs` L247-256 pattern from
   the existing `PartitionResult` emission). If a maintainer
   computes `has_border_activity = compute_border_activity(...)`
   once and writes it into `stats.has_border_activity` but then
   forgets to copy it to the top-level field (or vice versa), the
   two copies disagree silently. The coordinator's convergence
   detector will then read `top_level=false` while
   `stats.has_border_activity=true` (or the inverse), causing a
   false-positive or false-negative convergence — exactly the
   symmetry-breaking bug the BSP loop cannot tolerate (SPEC-05 R5,
   T4 strong confluence depends on per-worker activity flags being
   correct).

2. This is **structurally identical** to §3.2 DC-4's concern:
   `BorderState.is_redex` can drift from `is_principal_pair(side_a,
   side_b)` if a caller writes it manually. That verdict shipped
   Option B (graph recomputes; caller cannot set the bit). The same
   principle here: if duplication is mandated by R26, the
   duplication must be graph-enforced — not trust-based.

**The Option C resolution:**

Instead of Option B (drop the top-level field, divergence from spec
literal) or raw Option A (trust-based duplication, drift-vulnerable),
ship Option C:

1. Keep the top-level `has_border_activity: bool` field on
   `Message::RoundResult` per R26 verbatim.
2. Mark `stats.has_border_activity` as the **canonical source of
   truth** in the variant's doc comment, and state that the
   top-level field is a cache for ergonomics.
3. Add a constructor helper on `Message` (or an inline
   `debug_assert!` at the worker construction site) that enforces
   the equality at construction:
   ```rust
   debug_assert_eq!(
       stats.has_border_activity, has_border_activity,
       "RoundResult invariant: top-level has_border_activity MUST \
        equal stats.has_border_activity (SPEC-19 R26, §3.1 amendment)"
   );
   ```
4. Add a serde-layer regression test
   (`test_round_result_activity_matches_stats_activity`) that
   constructs a `RoundResult` with deliberately mismatched fields and
   asserts the debug-assert fires (via `std::panic::catch_unwind` or
   a compile-time-gated `#[should_panic]`).

The cost is ~10 LoC (the debug-assert + the test), and it closes
the drift vulnerability completely. In release builds the assert
vanishes (no runtime cost on the hot path); in debug + CI builds,
any maintainer-introduced drift is caught immediately.

**AMENDMENT NEEDED?** No spec amendment (R26 is preserved; the
debug_assert is a codebase-internal invariant, not a wire-protocol
change). **TASK-0369 amendment required.**

**TASK IMPACT:**

- **TASK-0369 "Context" paragraph** — replace the task-splitter's
  "follow verbatim" language with:
  > "Per DC-A2 of `docs/spec-reviews/SPEC-19-section-3.4-design-
  > choices-2026-04-17.md`: R26's top-level `has_border_activity`
  > field is duplicated on the wire per spec literal, BUT the
  > duplication is graph-enforced at construction — the worker-side
  > `RoundResult` builder MUST satisfy
  > `top_level == stats.has_border_activity` via debug_assert, and a
  > regression test verifies the invariant. The canonical source of
  > truth is `stats.has_border_activity` (per §3.1 amendment,
  > merge/types.rs L149); the top-level field exists for coordinator
  > pattern-match ergonomics only."

- **TASK-0369 variant doc comment** — the inline doc on
  `has_border_activity` changes from the current spec-neutral
  wording to:
  ```rust
  /// Cache of `stats.has_border_activity` for coordinator
  /// pattern-match ergonomics (R26). The canonical source of truth
  /// is `stats.has_border_activity` (SPEC-19 §3.1 amendment;
  /// `merge/types.rs` L149). The two copies MUST agree at
  /// construction — enforced by debug_assert in the worker-side
  /// builder and regression-tested in `protocol::types::tests`.
  has_border_activity: bool,
  ```

- **TASK-0369 "Acceptance Criteria" — add one test:**

  - `test_round_result_activity_matches_stats_activity` —
    construct two `RoundResult`s:
      1. `top_level = true`, `stats.has_border_activity = true` —
         bincode round-trip succeeds; decoded fields agree.
      2. `top_level = false`, `stats.has_border_activity = false` —
         same; fields agree.
    Then construct a **mismatched** pair (`top_level = true`,
    `stats.has_border_activity = false`) **inside a
    `#[should_panic(expected = "RoundResult invariant")]` test**
    that invokes the worker-side builder (once it exists;
    placeholder until 2.26-C ships the builder). **Note:** the
    builder doesn't exist yet; the spec-critic recommendation is to
    land the debug_assert invariant inline at the construction site
    when 2.26-C ships the worker lifecycle, and to pre-write the
    regression test stub here (marked `#[ignore]` with a
    `// TODO(2.26-C): enable once worker_emit_round_result lands`
    comment). This keeps the invariant discoverable without blocking
    the wire-layer ship.

- **TASK-0369 test-count row** (post DC-A1 shift): goes from 980 to
  **981 default / 1021 zero-copy** (one new `#[test]` + one ignored
  regression stub — `#[ignore]` tests count toward the total only
  when run with `--ignored`; for default `cargo test`, they are
  listed but not executed, and DO contribute to the `test result:
  ok. X passed; Y failed; Z ignored` line. The cargo test counter
  counts them in the total, so the count goes +2 not +1 for
  TASK-0369). Adjusted trajectory:
  | After task | default lib | zero-copy lib |
  |-----------|------------:|--------------:|
  | baseline (after DC-A1 fix) | 969 | 1009 |
  | TASK-0367 | 973 | 1013 |
  | TASK-0368 | 976 | 1016 |
  | TASK-0369 (with +2 DC-A2 tests) | 982 | 1022 |
  | TASK-0370 | 990 | 1030 |
  | TASK-0371 | 991 | 1031 |

  Update the bundle index test-count table accordingly.

- **Bundle index "Expected final counts"** — **991 lib default /
  1031 `--features zero-copy`** (+23 default / +23 zero-copy across
  6 tasks).

---

### DC-A3: Cargo feature gating for new variants

**PICK:** **Option A** — new variants are **always compiled** into the
`Message` enum. No `delta-mode` cargo feature. The runtime flag
`GridConfig.delta_mode` (shipping in sub-bundle 2.26-D, R20/R41) gates
the **behaviour** (whether the coordinator dispatches the variants);
the variants themselves are unconditionally present in the enum.

**WHY:** This is forced by SPEC-18 R20's feature-gate precedent and by
the nature of cargo features:

1. **SPEC-18 R20 precedent.** The `zero-copy` feature is feature-gated
   **because it pulls an optional dependency** (`dep:rkyv`), and the
   spec explicitly says `zero-copy` MUST NOT be a default feature
   (Cargo.toml L102-104). The motivation is **external dependency
   opt-in**, not variant-count reduction. The 5 new delta-protocol
   variants introduce NO new dependency — they compose existing
   types (`Partition`, `BorderDelta`, `WorkerRoundStats`, `PortRef`).
   There is no feature-gate motivation.

2. **Cargo-feature-gated variants are a public-API viral hazard.** A
   `#[cfg(feature = "delta-mode")] InitialPartition { ... }` on
   `Message` creates feature-conditional match arms throughout the
   codebase. Every `match msg { ... }` site has to decide: either
   (a) add `#[cfg(feature = "delta-mode")]` arms, duplicating every
   match block; or (b) make the match non-exhaustive and rely on a
   wildcard `_ =>`, which defeats Rust's exhaustiveness checking for
   `Message` entirely. Option (b) is worse than useless — it
   silently hides real unhandled-variant bugs. Option (a) is a
   maintenance-cost multiplier on every pattern site.

3. **Code-size cost is negligible.** 5 variants = ~100 bytes of
   discriminant + ~200 bytes of serde codegen per variant. Total:
   ~1.5 KB compiled overhead. On a Relativist binary that already
   clocks ~7 MiB release (per SPEC-13 targets), this is invisible.

4. **Runtime flag already provides the gate.** `GridConfig.delta_mode
   : bool` (default `false`, per R41) means that in a non-delta
   deployment, the variants are simply not sent over the wire. They
   exist in the enum, but the coordinator's dispatch code never
   produces them and the worker's handler never receives them. Dead
   code on the wire does not cost bytes.

5. **Discriminant-stability test (TASK-0371) becomes feature-
   conditional under Option B.** The byte-level stability test would
   have to be split into two versions (`#[cfg(feature =
   "delta-mode")]` testing 12 variants vs `#[cfg(not(feature =
   "delta-mode"))]` testing 7). This is avoidable complexity.

6. **Cargo-feature drift risk.** Under Option B, a developer can
   build the binary with `--features delta-mode` in one deployment
   and without it in another. Two binaries with different Message
   enums in the SAME cluster is catastrophic: the
   discriminant-stability contract protects against variant
   reordering but NOT against variant absence. A mixed-feature
   cluster would see a discriminant-7 message arrive at a binary
   whose decoder thinks discriminant 7 doesn't exist —
   `ProtocolError::UnknownVariant`, connection reset, confusing
   symptoms. This is a real operational footgun.

The task-splitter's default (Option A) is correct and the reasoning
in the bundle index is solid. No change required.

**AMENDMENT NEEDED?** No spec amendment. No task amendment.

**TASK IMPACT:** None. The bundle index's DC-A3 section already
documents the correct choice and the rationale. The spec-critic's
only addition is the "mixed-feature cluster" risk (reason 6 above),
which the task-splitter may OPTIONALLY cite in the bundle index for
future reference — not required.

---

## Summary table (compact)

| Design choice | Pick | Amendment needed? | Spec edit? |
|---------------|------|-------------------|-----------|
| DC-A1 `BorderDelta` placement | Option A — stay in `merge/`, re-export via `pub use` to `protocol/`. SPEC-13 R6-R8 forces this. | TASK-0366 (add serde derives on struct + 1 round-trip test; fix test-count trajectory) | No |
| DC-A2 `has_border_activity` duplication | Option C — duplicate per R26 literal, BUT graph-enforce equality via debug_assert + regression test; mark `stats.has_border_activity` as canonical. | TASK-0369 (doc comment + 1 new test + 1 ignored stub + test-count shift) + bundle index test-count table | No |
| DC-A3 Cargo feature gating | Option A — no `delta-mode` feature; runtime flag suffices; mixed-feature clusters are an operational footgun. | None | No |

## SPEC-19 amendments required before TESTS

**Count: 0.** No edits to `specs/SPEC-19-delta-protocol.md` are
required.

---

## Bonus: under-specified R31-R37 requirements flagged

The task-splitter's decomposition is thorough, but two requirements
are under-specified in the spec text and should be pinned before
Stage 2 TESTS writes test contracts against them.

### Bonus-1: R35 does not quantify "large-payload benefits from
`CompactSubnet` + varint + LZ4"

**R35 text:** "The `InitialPartition` and `FinalStateResult` variants
carry full `Partition` payloads and MUST benefit from all wire
optimizations: `CompactSubnet` encoding (SPEC-04), bincode v2 varint
encoding (SPEC-18 R1-R3), and optional LZ4 compression (SPEC-18
R8-R11)."

**Problem:** "MUST benefit" is not observable. The three listed
optimisations are all **transparently applied by the frame layer** —
there is no opt-in switch to flip. The requirement as written cannot
fail. It is effectively a comment, not a testable MUST.

**TASK-0370's mitigation:** the task-splitter translated R35 into a
concrete testable criterion ("`FLAG_COMPRESSED` SET for a 200-agent
partition") — this is the right move but it conflates "compression
was applied" with "compression was beneficial". A 200-agent payload
that LZ4 compresses to 98% of original size still satisfies the
test but does NOT benefit from compression.

**Recommendation for Stage 2 TESTS:** add one assertion to
`test_initial_partition_wire_roundtrip_compressed`:
```rust
let original_len = bincode_v2::encode(&msg)
    .expect("encode")
    .len();
let frame_len = /* from the recv side */;
// R35 "benefit" criterion: compressed payload must be strictly
// smaller than uncompressed. If LZ4 hit 98% of original, the wire
// "benefit" is illusory — the test should fail.
assert!(
    (frame_len as usize) < original_len,
    "R35 benefit: compressed frame ({} bytes) must be < \
     uncompressed bincode ({} bytes)",
    frame_len, original_len
);
```
This is a **non-blocking Stage 2 addition** — the test-generator
agent should add it to TEST-SPEC-0370 without a task amendment. Flag
here for the spec-critic record.

**Spec consequence:** not worth a spec amendment for v2; the language
"benefit" is informal-but-understandable. If the test ever fails
(compression didn't actually help for a realistic partition size),
the real action is to revisit SPEC-18's threshold default (1024
bytes), not SPEC-19.

### Bonus-2: R37 discriminant numbering tied to SPEC-18, but SPEC-18
added zero variants

**R37 text:** "The discriminant assignments in R31 and R32 MUST be
coordinated with SPEC-18. If SPEC-18 assigns different discriminants
to these variants, SPEC-19 defers to the coordinated numbering. The
requirement is that all delta protocol variants are appended at the
end of the enum (SPEC-06 R5) and assigned stable discriminants."

**Problem:** The bundle index correctly notes that SPEC-18 added NO
`Message` variants (verified by grep on `specs/SPEC-18-wire-format-
v2.md`). So "coordination with SPEC-18" is a no-op for this bundle.
R37's hedge language is dead text — but a future spec-critic or a
future bundle that reads R37 standalone (without reading the bundle
index) might get confused by the conditional.

**Recommendation:** no spec edit required (R37 is safe as a
future-proofing hedge), but TASK-0371's doc comment should state
explicitly: "SPEC-18 did not append variants; discriminants 7..=11
assigned here are fresh under R37's coordination rule." This is a
one-sentence addition, no new tests.

**Task impact:** tiny — TASK-0371 test body comment block (the
`TODO` reminder already there) should include one line:
```rust
// R37 coordination note: SPEC-18 (wire format v2) appended NO
// Message variants. Discriminants 7..=11 are fresh assignments
// for SPEC-19 §3.4 per R37's deferred-to-coordinated-numbering rule.
```

Not a TASK-0371 amendment (acceptance criteria unchanged); just a
documentation hygiene note for the developer.

### Bonus-3: R34 silence on `FLAG_ARCHIVED` for delta variants

**R34 text:** "All new message variants MUST satisfy the same
serialization requirements as existing variants: serde + bincode
(SPEC-06 R4, R11), round-trip identity (SPEC-06 R14), and CRC32
integrity (SPEC-06 R6-R10)."

**Problem:** R34 says nothing about `FLAG_ARCHIVED`. SPEC-18 R22
restricts `FLAG_ARCHIVED` to `AssignPartition` / `PartitionResult`
only (the rkyv fast path). The task-splitter correctly interprets
this in TASK-0370 ("Assert `FLAG_ARCHIVED` is UNSET (this variant
does NOT use rkyv per R22)"), but the interpretation is not
spec-pinned — it is implied by SPEC-18 R22's whitelist.

**Recommendation:** no SPEC-19 amendment needed. SPEC-18 R22 is
normative and TASK-0370's tests correctly encode the exclusion. For
future reviewer clarity, the bundle index's "Scope (in vs out)"
line item "rkyv zero-copy archive path for the 5 new variants —
SPEC-18 R22 explicitly restricts `FLAG_ARCHIVED` to `AssignPartition`
/ `PartitionResult`; the delta variants ride the bincode path"
already captures this. No action required.

---

## Checklist

### Consistency
- [x] All terms match SPEC-00 / SPEC-19 / SPEC-13 / SPEC-18
      definitions.
- [x] Type signatures compatible with predecessor specs
      (`BorderDelta` shape matches R33; `Partition`,
      `WorkerRoundStats`, `PortRef` all already Serialize +
      Deserialize).
- [x] No contradictions with predecessor requirements (DC-A1 aligns
      with SPEC-13 R6-R8; DC-A3 aligns with SPEC-18 R20 precedent).
- [x] Data flow assumptions match (`apply_deltas` consumes
      `BorderDelta` in pure-core; wire transport adds serde derives
      at the same struct).

### Testability
- [x] DC-A1 verifiable by source-grep (no `BorderDelta` in
      `protocol/types.rs` definition site; `pub use` re-export
      present) + new `test_border_delta_bincode_roundtrip`.
- [x] DC-A2 verifiable by new
      `test_round_result_activity_matches_stats_activity`
      (serde-layer equality of the two copies) + the stub for the
      debug_assert regression (to be enabled when 2.26-C ships the
      worker builder).
- [x] DC-A3 verifiable by negative test: grep the codebase post-
      ship for `#[cfg(feature = "delta-mode")]` — expected count: 0.

### Completeness
- [x] All three flagged choices have a verdict.
- [x] All cascading TASK impacts documented (TASK-0366 serde derive
      + test; TASK-0369 doc comment + 2 tests; bundle index
      test-count trajectory).
- [x] All bonus findings flagged (R35 benefit quantification, R37
      fresh-discriminant comment, R34/R22 whitelist note).
- [x] No undefined terms in the verdict.

### Invariant Preservation
- [x] T1-T7 unaffected (wire-protocol extension; no net structural
      changes).
- [x] D1-D2, D4-D5 unaffected (no partitioning semantics changed).
- [x] D3 (Border Completeness) **strengthened** by DC-A2 Option C
      — `has_border_activity` duplication is now graph-enforced,
      not caller-trusted.
- [x] D6 unaffected.
- [x] I1-I5 unaffected.
- [x] G1 unaffected (no reduction semantics changed).
- [x] R18 complexity bounds preserved.
- [x] R19 pure-core invariant preserved — `BorderDelta` stays in
      `merge/` (pure-core); the re-export into `protocol/` does NOT
      invert the allowed dependency edge (SPEC-13 R6-R8).
- [x] SPEC-06 R5 (append-only discriminants) preserved —
      verifiable by TASK-0371's byte-level stability test.

---

## Stage 1.5 verdict

**Stage 1.5 spec-critic complete.** TESTS stage is **unblocked** in
principle, but the task-splitter (or task-updater) MUST apply the
TASK-file amendments listed under DC-A1 (TASK-0366 + bundle index
test-count table) and DC-A2 (TASK-0369 + bundle index test-count
table) before test-generator writes TEST-SPEC-0366..0371 — otherwise
the test contracts will encode stale signatures and stale counts.

Per CLAUDE.md rule and local convention that only task-splitter /
task-updater edits `docs/backlog/`, spec-critic does NOT edit those
task files directly. The orchestrator should dispatch task-updater
to apply the amendments, then dispatch test-generator for Stage 2
TESTS.

### Additional observations (not DC verdicts but flagged here)

1. **Pre-existing defect in TASK-0362's BorderDelta derives**
   (surfaced by DC-A1): the struct is missing `serde::Serialize,
   serde::Deserialize`. This blocks TASK-0368 / TASK-0369 compilation
   and MUST be fixed in TASK-0366. If the developer discovers this
   during Stage 3 DEV rather than task-update time, it is a
   **Stage 3 blocker, not a Stage 1.5 blocker** — but landing the
   fix pre-emptively in TASK-0366's scope is strictly cheaper.

2. **Worker construction site for `RoundResult`** (surfaced by
   DC-A2): the debug_assert must live where `RoundResult` is built
   by the worker — which does not exist yet (ships in 2.26-C). The
   spec-critic recommendation is to **land the invariant-enforcing
   test stub here as `#[ignore]`** and enable it in 2.26-C. If the
   2.26-C bundle drops or reorders the builder, the ignored test
   will flag the regression at the next `cargo test --ignored` run.
   Alternative: land the `debug_assert!` as a free function
   `Message::round_result_builder(...) -> Message` in TASK-0369 and
   require 2.26-C to use it. Marginal LoC difference; the free
   function is cleaner and self-documenting. Task-updater's choice
   which to pin in the amendment.

Observations 1 and 2 are non-blocking for Stage 2 TESTS (both
addressable in the TASK-0366 + TASK-0369 amendments) but
task-splitter / task-updater SHOULD fix them now to avoid Stage 3
scope creep.
