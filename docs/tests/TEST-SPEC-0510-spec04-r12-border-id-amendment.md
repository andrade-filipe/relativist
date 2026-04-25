# TEST-SPEC-0510: SPEC-04 R12 border-id amendment (streaming-path policy)

**SPEC-21 §7 ID:** plumbing only (no T-id; transitively covered by T5/T6).
**Owning task:** TASK-0510 (spec-text amendment).
**Parent spec:** SPEC-21 §3.8 A1; SPEC-21 §3.5 R29b; SPEC-21 §4.8.
**Type:** unit (spec-language assertion only — no production code; verified via doc-test / grep gate).
**Theory anchor:** ARG-002 Q3 (bidirectional FreePort), Q5/C3 (border bijection); ARG-003 R3 (FreePort as wire-reference).

---

## Inputs / Fixtures

- The SPEC-04 next-revision diff produced by ESPECIALISTA EM SPECS for §4.5 / R12.
- A `cargo test --doc -p relativist-core` runner with the amended R12 prose loaded into a doc comment of the public `border_id_start` struct field (or equivalent indirect anchor when SPEC-04's design exposes it).
- Two reference generator fixtures: (a) `ep_annihilation_pure(20)` (no Lafont FreePorts), (b) a synthetic generator with 3 Lafont FreePorts in batch 0 only.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0510-01 | `r12_text_contains_dual_path_clause` | the rendered `specs/SPEC-04-partition.md` § 4.5 / R12 | grep for both `chunk_size = u32::MAX` AND `generate_and_partition_chunked` AND `SPEC-21 §3.8 A1` AND `SPEC-21 R29b` | all four substrings present (single-pass scan over the file). |
| UT-0510-02 | `r12_streaming_clause_starts_at_zero_no_freeports` | amended R12 prose | grep for "`border IDs start at 0`" or "`[0, border_id_counter)`" near the streaming-path branch | substring present. |
| UT-0510-03 | `r12_streaming_clause_lafont_offset` | amended R12 prose | grep for `max_lafont_freeport_id` and `+ 1` in the streaming-path branch | both substrings present. |
| UT-0510-04 | `r12_cross_path_disjointness_note` | amended §4.5 closing-note | grep for the sentence "tests exercising both `split()` and `generate_and_partition_chunked` MUST account for the distinct border-id ranges" | substring present (verbatim or paraphrase containing all four anchor tokens). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | A future amendment relocates the streaming clause to §4.8 only | UT-0510-01 fails; the spec MUST keep the cross-reference in §4.5 to keep R12's textual surface complete (this is the gate against regressing SC-018 closure). |
| EC-2 | Generator emits Lafont FreePorts in batch 1 (not batch 0) | Behavioral test deferred to TEST-SPEC-T5; this TEST-SPEC only covers the spec-text gate. The §4.5 prose explicitly forbids this layout (generators "SHOULD emit ALL Lafont FreePorts in the first batch"). |
| EC-3 | `split()` and `generate_and_partition_chunked` invoked on the same generator emit IDs that overlap | This is a SPEC-21 R29b / SPEC-04 R12 violation; behavioral coverage in the cross-path test (TEST-SPEC-T6); this spec-text test does not cover the runtime path. |

## Invariants asserted

- C3 (FreePort Bijectivity) — preserved across both paths; the amendment only changes the absolute integer, not the bijection contract.
- T1 (Port Linearity) — unaffected.
- SC-018 closure (border-id allocation defined for the streaming path) — frozen via UT-0510-01.

## ARG/DISC/REF citation

- ARG-002 Q3, Q5/C3.
- ARG-003 R3.

## Determinism notes

Pure spec-text grep test. No tokio, no async, no RNG. Deterministic by inspection. The doc-test runner MUST use a stable file path resolution (`env!("CARGO_MANIFEST_DIR")` rooted) so the test is reproducible across CI runners.

## Cross-test dependencies

- TEST-SPEC-T5 (streaming pipeline produces valid partitions) — behavioral coverage of the streaming-path C3 bijection.
- TEST-SPEC-T6 (streaming vs batch isomorphism) — behavioral coverage of the dual-path output equivalence.
- TEST-SPEC-0517 (`split()` additive amendment) — sibling spec-text gate.
