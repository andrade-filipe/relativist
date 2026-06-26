# REVIEW — SPEC-19 §3.4 (item 2.26-A) Delta-Only Protocol Wire Extensions

**Reviewer:** SDD pipeline review (manual, Option B precedent)
**Date:** 2026-04-17
**Bundle scope:** SPEC-19 §3.4 wire layer only (TASK-0366..0371).
  Delta-only coordinator/worker dispatch, `GridConfig.delta_mode`,
  `run_grid_delta` BSP loop, and R48 runtime enforcement are **out of
  scope** (ship under items 2.26-B and 2.26-C).

**Files under review**

- `relativist-core/src/merge/border_graph.rs` (L131-199 new structs +
  L1655-1709 new bincode round-trip tests)
- `relativist-core/src/merge/mod.rs` (re-exports)
- `relativist-core/src/protocol/types.rs` (5 new `Message` variants,
  disc 7..=11; 17 new inline tests + extended
  `test_all_variants_serde_roundtrip`; full
  `test_message_discriminant_stability` at 12 variants)
- `relativist-core/src/protocol/mod.rs` (`mod delta_wire_tests;` +
  `pub use crate::merge::{BorderDelta, LocalReconnection,
  MintedAgent, PendingCommutation};`)
- `relativist-core/src/protocol/frame.rs` (TASK-0347 R5
  `_exhaustive_check` + `sample_all_message_variants` extended to
  cover all 12 variants)
- `relativist-core/src/protocol/delta_wire_tests.rs` (new, 466 LoC,
  7 `#[tokio::test]` wire integration tests)

---

## 1. Verdict

**APPROVE** for promotion to Stage 5 (QA).

| Class      | Count | Summary |
|------------|:-----:|---------|
| MUST-FIX   |   0   | No blocking defects. Spec R31..R37 + R48 (wire layer), DC-A1, DC-A2, DC-B3, DC-B5 faithfully implemented. All quality gates GREEN. |
| SHOULD-FIX |   2   | S1 — Missing T8 positive-control test from TEST-SPEC-0370 (`final_state_result_crc_still_valid_no_tamper`). S2 — Stale fixture docstring on `make_large_partition` (`n_agents = 200` comment remains; actual callers pass 2000 after the DEV stage size bump). |
| NICE-TO-HAVE |   4 | See §5. None block promotion. |

Test-count delta: **968 → 995 lib default (+27)**; **1008 → 1035 lib
`--features zero-copy` (+27)**. Floor (per pipeline-state) was +22 —
exceeded by 5.

Pipeline gate evidence (replay from `/docs/pipeline-state.md` Stage 3
acceptance block): `cargo test --workspace --lib` GREEN (995, 1
ignored, 0 failed); `cargo test --workspace --lib --features
zero-copy` GREEN (1035); `cargo test --workspace` GREEN (includes CLI
integration); `cargo clippy --workspace --all-targets -- -D warnings`
GREEN both features; `cargo fmt --check` GREEN; `cargo build
--release` GREEN; smoke `compute add 3 5 → 8`.

---

## 2. Spec Conformance — SPEC-19 §3.4

| Req       | Scope (bundle 2.26-A)                                                                                  | Status | Evidence |
|-----------|--------------------------------------------------------------------------------------------------------|:------:|----------|
| **R31**   | C→W variants `InitialPartition` (disc 7), `RoundStart` (disc 8), `FinalStateRequest` (disc 10) appended. | ✅ | `types.rs:101-106, 114-138, 176-179`. Discriminants pinned by `test_message_discriminant_stability` (`types.rs:1053-1154`, cardinality assertion `cases.len() == 12`). |
| **R32**   | W→C variants `RoundResult` (disc 9), `FinalStateResult` (disc 11) appended.                            | ✅ | `types.rs:144-169, 185-190`. R32 payload for `RoundResult` includes `stats`, `has_border_activity` (DC-A2), `minted_agents` (DC-B5). |
| **R33**   | `BorderDelta`, `LocalReconnection`, `PendingCommutation`, `MintedAgent` defined with serde derives.     | ✅ | `merge/border_graph.rs:131-199`. All four structs `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`. `BorderDelta` additionally `Copy`-able (harmless extension — the R33 source text specifies *minimum* derives). |
| **R34**   | All new variants encode under serde + bincode v2, round-trip identity, CRC32C integrity.                | ✅ | Per-variant round-trips (`types.rs:526-1034`, 17 tests); struct-level round-trips (`border_graph.rs:1654-1709`, 4 tests); wire-layer round-trips (`delta_wire_tests.rs` T1-T6); CRC-negative `delta_wire_tests.rs` T7. |
| **R35**   | `InitialPartition` / `FinalStateResult` benefit from LZ4 ≥ threshold.                                    | ✅ | `delta_wire_tests.rs` T1 (InitialPartition) + T2 (FinalStateResult). Both assert `FLAG_COMPRESSED == 1` AND `frame_len < uncompressed_bincode_len` (Bonus-1). Fixture size `make_large_partition(2000)` guarantees break-even. |
| **R36**   | `RoundStart` / `RoundResult` / `FinalStateRequest` below threshold skip compression (SHOULD).           | ✅ | `delta_wire_tests.rs` T3, T4, T5 assert `FLAG_COMPRESSED == 0` with DEFAULT_COMPRESSION_THRESHOLD. T6 asserts the "SHOULD not MUST-NOT" direction with `threshold=1` forcing compression. |
| **R37**   | Discriminant append-only, coordinated with SPEC-18.                                                      | ✅ | `types.rs:1036-1154` pins byte-0 per variant with cardinality guard. Comment `L1044-1047` documents the SPEC-18 coordination (SPEC-18 appended zero Message variants; §3.4 is the FIRST post-SPEC-06 assignment under R37). |
| **R48**   | Agent-id allocation coordination invariant (DC-B5).                                                     | ⚪ N/A (wire only) | Wire carries `request_id` (C→W) + `minted_agent_id` (W→C) with byte-preserving round-trips. Runtime enforcement (unmatched-request-id → protocol violation) is explicitly scoped to 2.26-C; spec text of R48 acknowledged in doc-comments (`types.rs:133-137, 162-167`; `border_graph.rs:159-198`). |

**SPEC-06 R5 discriminant stability (inherited):** variants 0..=6
untouched both in byte order and in payload shape; only append-only
growth. Verified by the discriminant-stability test.

**SPEC-18 R22 `FLAG_ARCHIVED` whitelist:** no delta variant sets
`FLAG_ARCHIVED`. `delta_wire_tests.rs` T1-T5 each asserts
`header.flags & FLAG_ARCHIVED == 0`; the rkyv fast path remains pinned
to `AssignPartition` / `PartitionResult` only.

---

## 3. Design-Choice Verdicts (R33 Amendments, 2026-04-17)

### DC-A1 — `BorderDelta` gains serde derives + wire re-export

- `border_graph.rs:131` adds `serde::Serialize + Deserialize` to
  `BorderDelta`. Module doc at L126-130 explains the §3.2→§3.4
  rationale.
- `protocol/mod.rs:43` re-exports `BorderDelta` (alongside the three
  new structs) under `crate::protocol::*`. Downstream wire callers
  can now name the full delta-protocol wire surface via a single
  import path.
- **Verdict: CORRECT.** Zero deviations from R33 source text.

### DC-A2 — `RoundResult.has_border_activity` equality with `stats.has_border_activity`

- Wire payload: top-level `has_border_activity: bool` and
  `stats: WorkerRoundStats` (whose own `has_border_activity` field
  is the **canonical source of truth** per the R33 amendment).
  `types.rs:144-169` documents this contract in-line.
- Serde preserves both fields independently — the runtime builder
  invariant (equality at construction) is NOT enforced at the wire
  layer. `types.rs:924-949` (`test_round_result_activity_flag_independence_from_stats`)
  pins that fact by asserting **different top-level values produce
  different byte streams**, ruling out "decoder silently overrides
  top-level from stats". `test_round_result_activity_matches_stats_activity`
  (`types.rs:952-983`) complements by verifying agreement is
  preserved on well-formed pairs.
- Runtime-invariant stub: `types.rs:985-994`
  (`test_round_result_activity_invariant_runtime`) is an
  `#[ignore] #[should_panic]` placeholder for 2.26-C. Body
  `panic!("stub — enable in 2.26-C")` satisfies the
  `#[should_panic(expected = "RoundResult invariant")]` target on
  today's body AS WRITTEN, which is **a bug**: the `should_panic`
  message doesn't match `"stub — enable in 2.26-C"`. See §5 NICE-4.
- **Verdict: CORRECT at the wire layer; the runtime stub mismatch
  is a NICE-TO-HAVE since the test is `#[ignore]` and never runs.**

### DC-B3 — `LocalReconnection`

- Defined at `border_graph.rs:149-157`. Consumed in
  `Message::RoundStart.local_reconnections: Vec<LocalReconnection>`
  (`types.rs:132`). Full field contract (agent_id, port, new_target)
  matches SPEC-19 §3.4 R33.
- Coverage: struct round-trip (`border_graph.rs:1668-1680`), populated
  variant round-trip (`types.rs:733-773`, T5), wire integration via
  T3.
- **Verdict: CORRECT.**

### DC-B5 — 2-phase AgentId allocation (`PendingCommutation` / `MintedAgent`)

- `PendingCommutation` at `border_graph.rs:173-181`,
  `MintedAgent` at `border_graph.rs:190-199`.
  Correlation key `request_id: u32` shared across both, with
  doc-comments cross-referencing R48.
- Order preservation tested at `types.rs:778-817` (multi-request order
  in RoundStart) and `types.rs:998-1034` (multi-request echo order in
  RoundResult).
- **Verdict: CORRECT at the wire layer.** R48 runtime enforcement
  deferred to 2.26-C (documented in `border_graph.rs:166-172`).

---

## 4. MUST-FIX

**None.**

---

## 5. SHOULD-FIX and NICE-TO-HAVE

### S1 (SHOULD-FIX) — Missing T8 positive-control test

TEST-SPEC-0370 §T8 specifies `final_state_result_crc_still_valid_no_tamper`
as a positive-control complement to T7 (catches the failure mode
"T7 'passes' because `recv_frame` always errors for an unrelated
reason"). The test is **not present** in `delta_wire_tests.rs`.

T2 (`final_state_result_wire_roundtrip_compressed_and_beneficial`)
*approximately* covers the positive control via its `.expect("recv")`
at L170-172, but with two deviations from the T8 spec:

1. T2 uses `duplex(1_048_576)` and `make_large_partition(2000)`; T8 was
   spec'd with `duplex(64 * 1024)` and `make_large_partition(200)`.
   With 200 agents, the payload may fall below the LZ4 break-even,
   so T8 also exercises the **uncompressed-but-through-the-CRC-path**,
   which T2 does not.
2. T2's assertion set is compression-centric (`FLAG_COMPRESSED != 0`,
   `frame_len < bincode_len`), not decode-centric. A regression that
   breaks the CRC path but accidentally keeps compression flags stable
   would fail both; one that breaks CRC *and* fixes compression
   signalling simultaneously would slip past T2 but be caught by T8.

**Fix:** add T8 in Stage 6 REFACTOR. Trivial — the spec text includes
the full test body (TEST-SPEC-0370 lines 514-536).

### S2 (SHOULD-FIX) — Stale docstring on `make_large_partition`

`delta_wire_tests.rs:53` states:

```text
/// Partition with `n_agents` live CON agents and trivial port topology,
/// sized to cross `DEFAULT_COMPRESSION_THRESHOLD` when wrapped in
/// `Message::InitialPartition`. `n_agents = 200` empirically exceeds
/// 1024 bytes in bincode v2 varint encoding.
```

The DEV stage bumped all call-sites from 200 to 2000 because 200 agents
produced a payload at the LZ4 break-even (bincode 1217 B → compressed
1236 B — compression NOT beneficial). The docstring still references
200 and is now misleading.

**Fix:** update docstring to name 2000 (or parametrise) and explain the
break-even reasoning.

### NICE-1 — `Copy` derive on `BorderDelta`

`border_graph.rs:131` includes `Copy` in the derive set
(`#[derive(Debug, Clone, Copy, ...)]`). R33's spec text does not
mandate `Copy`, but it is harmless and gives call-sites pass-by-value
ergonomics. No action required.

### NICE-2 — Cardinality assertion comment

`types.rs:1146-1153` asserts `cases.len() == 12`. When the enum grows,
the test will pass **even if the new variant is silently omitted from
`cases`** as long as the cardinality literal is not bumped. The
failure mode is a human forgetting to append both entries at once.
Consider a `const EXPECTED_VARIANT_COUNT: usize = 12;` with a comment
noting the forced compile-break path (`fn _exhaustive_check` on
`frame.rs:1308` is the real guard — maybe cross-reference it in the
test comment).

### NICE-3 — Tamper test error-family documentation

`delta_wire_tests.rs:445-466` accepts
`{ChecksumMismatch | DecompressionFailed | Serialize | Deserialize}`.
The decision was empirical (LZ4 decompress tripped before CRC on
single-byte flips of 2000-agent compressed payloads). Consider
documenting **which error variant is expected on the current code
path** (`DecompressionFailed` for compressed/tampered) and **why the
match is a superset** — the superset is defensive against LZ4
internals changing. This is a comment-only improvement.

### NICE-4 — Ignored-test body mismatch

`types.rs:989-994`:

```rust
#[test]
#[ignore = "TODO(2.26-C): enable once worker_emit_round_result builder lands"]
#[should_panic(expected = "RoundResult invariant")]
fn test_round_result_activity_invariant_runtime() {
    panic!("stub — enable in 2.26-C");
}
```

If someone removes only the `#[ignore]` attribute without wiring up
the 2.26-C builder, the test will fail: the panic message is
`"stub — enable in 2.26-C"` but `#[should_panic]` expects
`"RoundResult invariant"`. That failure is **the intended signal** —
the comment at L988 explicitly calls it out as "fails loudly" on
premature re-enable. The mismatch is the feature, not the bug. Keep
as-is.

---

## 6. Layer / Invariant Check

### R19 (pure-core) — merge/ cannot depend on protocol/

- Canary at `border_graph.rs:1578-1596` scans for `use tokio`,
  `use async_trait`, `use crate::protocol` lines. **PASS.**
- `border_graph.rs:1660, 1662, 1675, 1677, 1690, 1692, 1704, 1706`
  qualify calls as `crate::protocol::bincode_v2::...` inside
  `#[cfg(test)]` bodies. The canary is whitespace-start-of-line +
  `use ` prefix, so fully-qualified paths inside expression bodies
  are R19-canary-safe. Semantically: test-only dependency is a
  well-established precedent (e.g., SPEC-19 §3.2 bundle tests on
  `merge::helpers` already dereference protocol types in identical
  fashion).
- **Verdict: R19 preserved.**

### SPEC-13 R6-R8 layering

No new dependency edges added. The delta-protocol wire structs live
in `merge/` (pure) and are re-exported from `protocol/` (async). The
re-export is zero-cost and does not introduce a cyclic import.

### SPEC-18 R22 `FLAG_ARCHIVED` whitelist

Respected — verified by T1-T5 in `delta_wire_tests.rs` (assert
`header.flags & FLAG_ARCHIVED == 0`) and by the exhaustiveness
contract of `sample_all_message_variants`.

---

## 7. Documentation Quality

- Doc-comments on every new public item (5 `Message` variants, 3 new
  structs, 1 re-export). Each cross-references SPEC-19 §3.4 R33 and
  the relevant DC amendment ID (DC-A1, DC-A2, DC-B3, DC-B5). Good.
- Module-level doc on `types.rs:1-18` is explicit about ownership
  (SPEC-06 vs SPEC-19) and R37 discriminant stability. Good.
- `delta_wire_tests.rs:1-27` module doc is a miniature test plan
  tying each test to its SPEC requirement. Good.

No missing doc-comments on `pub` items; clippy
`-D warnings` GREEN confirms `missing_docs` is not tripped in lib
crate on the new surface.

---

## 8. Test Quality

### Test-spec coverage ledger (per-task)

| Task | Expected tests | Delivered tests | Deviation |
|------|---------------:|----------------:|-----------|
| TASK-0366 (DC-A1 derives + re-export + 4 struct round-trips) | 4 | 4 | — |
| TASK-0367 (InitialPartition + FinalStateResult variants) | 4 | 4 | T1..T4 delivered |
| TASK-0368 (RoundStart + FinalStateRequest + DC-B3 + DC-B5 multi) | 5 | 5 | T1, T2, T3, T5, T6 delivered (numbering from TEST-SPEC-0368) |
| TASK-0369 (RoundResult + DC-A2 + DC-B5 echo) | 7 | 6 runnable + 1 ignored stub | T6 is `#[ignore]` (runtime invariant stub deferred to 2.26-C) |
| TASK-0370 (wire-layer integration tests) | 8 | **7** | **T8 positive-control MISSING** (S1 above) |
| TASK-0371 (discriminant stability R37) | 1 | 1 | `test_message_discriminant_stability`; cardinality gate present |

**Total shortfall:** 1 test (T8 of TEST-SPEC-0370). NOT blocking
(redundant with T2 on the happy path); S1 in §5.

### Test isolation

- No test shares mutable state. `tokio::io::duplex` is per-test.
- No sleeps, no global locks, no `std::env` reads.
- Tests deterministic: fixtures use fixed agent counts, no RNG, no
  time-sensitive code.

### Test readability

Variant-match-and-destructure pattern used throughout is consistent
with SPEC-06 test style; readers unfamiliar with the delta protocol
can still follow because each test names the variant it targets and
the DC / R-number it anchors.

---

## 9. Adversarial Probe Enumeration (QA Stage 5 candidates)

Thirteen probes identified for Stage 5 QA. Each targets a real-bug
surface, not a happy-path restatement.

| Q#  | Target | Hypothesis |
|-----|--------|------------|
| Q1  | Discriminant-byte tamper on compressed frame (flip byte at offset 9 so `RoundStart` decodes as `RoundResult`) | CRC is over uncompressed payload; post-decompression the disc byte is inside the CRC-covered region, so the tamper MUST surface as ChecksumMismatch (not as a silent variant swap). |
| Q2  | Empty `RoundStart` with all 5 vecs empty — byte-count floor | The 5-vec empty case encodes as `disc(1) + round-varint(1..=5) + five-length-prefixes(5×1)` = 7..=11 bytes. Probe pins the floor. |
| Q3  | `RoundResult` with mismatched `has_border_activity` vs `stats.has_border_activity` (top-level=true, stats=false) | Wire layer MUST preserve the mismatch faithfully (no silent "fix"). Already covered by T5-like coverage, but the adversarial framing ("malicious coordinator") is worth a dedicated probe because the 2.26-C builder invariant depends on this reflexive guarantee. |
| Q4  | `PendingCommutation.arity = u8::MAX` (255) | Extreme arity value; bincode should preserve verbatim. |
| Q5  | `LocalReconnection.port = u8::MAX` (255) and `LocalReconnection.agent_id = u32::MAX` | Boundary-value round-trip; ensures no implicit truncation/conversion. |
| Q6  | `MintedAgent.minted_agent_id` inside the coordinator-reserved range `u32::MAX - 10_000 .. u32::MAX` (R48 violation) | Wire layer MUST NOT reject — R48 enforcement is a coordinator-runtime concern (2.26-C). Probe locks this separation. |
| Q7  | `BorderDelta.new_target = AgentPort(u32::MAX, 255)` | DISCONNECTED is `FreePort(u32::MAX)`, NOT `AgentPort(u32::MAX, _)`. This probe verifies a non-DISCONNECTED extreme value round-trips without sentinel confusion. |
| Q8  | `InitialPartition` exactly at `DEFAULT_COMPRESSION_THRESHOLD - 1` bytes | Boundary: threshold is ≥ compare, so this MUST skip compression. Probes the off-by-one. |
| Q9  | `InitialPartition` exactly at `DEFAULT_COMPRESSION_THRESHOLD` bytes | Boundary: threshold equality semantics. Probes the off-by-one. |
| Q10 | 10 000-element `border_deltas` Vec on `RoundStart` | Stress: bincode v2 varint-encoded length prefix; ensures no int-overflow / panic / O(n²) path. |
| Q11 | `FinalStateRequest` with `round = u32::MAX` | Max-value varint edge case (5-byte encoding for `round`). |
| Q12 | Truncated frame (first 8 bytes only — header but zero payload) | `recv_frame` MUST return a short-read error, not panic, not hang. |
| Q13 | Adversarial header-flags combo: `FLAG_COMPRESSED | FLAG_ARCHIVED` on a delta variant | SPEC-18 R22 whitelist: delta variants MUST NOT ride the rkyv fast path. `recv_frame` MUST reject with the SPEC-18 R22 error (or fail to decode via rkyv, surfacing ArchiveValidationFailed). Today no enforcement path — probe may fail, which would be a **real** 2.26-A MUST-FIX. |

**Prioritisation:** Q1, Q3, Q6, Q13 are the highest-leverage (touch
correctness of the delta semantics). Q8/Q9 are standard boundary
hygiene. Q12 is a hang/crash guard. The remainder pin contract edges.

---

## 10. Recommendations

1. **Promote to Stage 5 (QA).** 0 MUST-FIX, 2 SHOULD-FIX that can
   ride in Stage 6 REFACTOR.
2. **Stage 5 QA MUST implement at least Q1, Q3, Q6, Q13** from §9.
   The remainder are highly desirable but not all mandatory.
3. **Stage 6 REFACTOR punch-list:**
   - S1: add T8 `final_state_result_crc_still_valid_no_tamper`.
   - S2: fix `make_large_partition` docstring (200 → 2000 or
     parametrise).
   - NICE-2: consider `EXPECTED_VARIANT_COUNT` const in the
     discriminant-stability test.
   - NICE-3: document the tamper-test error family.

---

## 11. Acceptance Criteria — VERIFIED

1. SPEC-19 R31..R37 (wire layer) implemented and tested. ✅
2. R33 DC-A1/A2/B3/B5 amendments faithful to 2026-04-17 spec text. ✅
3. SPEC-06 R5 + SPEC-19 R37 discriminant stability regression
   guarded at byte level with cardinality assertion. ✅
4. SPEC-18 R22 `FLAG_ARCHIVED` whitelist respected. ✅
5. SPEC-13 R19 pure-core invariant preserved (canary green). ✅
6. All quality gates GREEN (tests, clippy, fmt, release build, smoke). ✅
7. No MUST-FIX defects. ✅
