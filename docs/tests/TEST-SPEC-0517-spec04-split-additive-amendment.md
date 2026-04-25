# TEST-SPEC-0517: SPEC-04 split() additive amendment (chunk_size short-circuit + fallback path)

**SPEC-21 §7 ID:** plumbing only (gates T6 short-circuit equivalence).
**Owning task:** TASK-0517.
**Parent spec:** SPEC-21 §3.8 A8; §6.2 R26 (v1 backward-compat short-circuit).
**Type:** unit (spec-text grep gate + structural-compatibility verification).
**Theory anchor:** ARG-002 (partitioning preserves structure; split/merge identity).

---

## Inputs / Fixtures

- The amended SPEC-04 §6 / §3 prose declaring `split()` UNCHANGED and the chunked pipeline as an alternative entry point.
- The R26 short-circuit policy text in SPEC-21 §6.2.
- A reference net of size 100 generated via `make_net(ep_annihilation_pure(50))`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0517-01 | `split_function_unchanged` | `cargo doc -p relativist-core` rendered SPEC-04 § split() prose | diff against pre-SPEC-21 baseline | byte-for-byte unchanged. The amendment is purely additive at the prose level. |
| UT-0517-02 | `split_signature_byte_unchanged` | the public signature of `pub fn split(net: Net, num_workers: u32, strategy: ...) -> Result<PartitionPlan, _>` (or whatever the v1 signature is) | grep + diff | signature unchanged. |
| UT-0517-03 | `chunk_size_u32_max_routes_to_split` | `GridConfig { chunk_size: u32::MAX, .. default() }` and the dispatch helper | invoke top-level partition entry point | the call dispatches to `split()`, NOT to `generate_and_partition_chunked` (R26 short-circuit). |
| UT-0517-04 | `split_output_structurally_compatible_with_chunked` | `split(net, 4, ContiguousIdStrategy)` and `generate_and_partition_chunked(stream, 4, RoundRobin, chunk_size=25)` on the same generator | compare output types | both produce a `Vec<Partition>` (or `PartitionPlan` and a `From<ChunkedPartitionResult>` conversion); each `Partition` has identical fields: `subnet`, `free_port_index`, `id_range`, `border_id_start`, `border_id_end`, `worker_id` (R21). |
| UT-0517-05 | `split_v1_backward_compat_amendment_documented` | SPEC-04 §6 prose | grep for "fallback for the v1 backward-compat path (R26)" | substring present (SPEC-21 §6.2 cross-reference). |
| UT-0517-06 | `chunked_path_explicitly_named_as_alternative_entry_point` | SPEC-04 §6 prose | grep for "ALTERNATIVE entry point" or "selected by `GridConfig.chunk_size != u32::MAX`" | substring present. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `chunk_size = u32::MAX` AND `dispatch_mode = Pull` | dispatch helper logs a warning (TEST-SPEC-0512 EC-3); short-circuit wins; routes to `split()`. |
| EC-2 | A future amendment that mutates `split()` signature | UT-0517-02 fails immediately (regression gate — `split()` signature is part of the v1 backward-compat surface). |
| EC-3 | A new `Partition` field added by another spec | UT-0517-04 list MUST be updated AND the `From<ChunkedPartitionResult>` conversion in TASK-0523 MUST be updated synchronously; this is a sibling-spec coordination point. |

## Invariants asserted

- §3.8 A8 (SPEC-04 split() UNCHANGED; chunked pipeline is alternative entry point).
- R26 (v1 backward-compat path).
- D1 (Split/Merge Identity) — preserved unchanged for the `split()` path.
- C1, C2, C3 — preserved unchanged for the `split()` path.

## ARG/DISC/REF citation

- ARG-002 (partitioning preserves structure) — the amendment is a no-op for the original ARG-002 path.

## Determinism notes

Pure synchronous spec-text and signature gate. No tokio, no RNG. UT-0517-04 builds two equivalent partitionings; the comparison is structural (same field set, same types), NOT semantic (semantic equivalence is TEST-SPEC-T6's job).

## Cross-test dependencies

- TEST-SPEC-T6 (streaming vs batch isomorphism) — behavioral coverage of UT-0517-04's structural-compat claim.
- TEST-SPEC-0567 (R26 short-circuit) — forward-referenced from TASK-0517 but NOT in scope for Stage 2 wave 1 (TASK-0567 not yet authored). Flagged.
- TEST-SPEC-0500 (SPEC-22 v1 backward-compat regression) — share the regression-gate pattern; UT-0517-01/02 are the SPEC-21 mirror.
- All existing SPEC-04 split() tests UNCHANGED — regression gate via TASK-0600 (out of scope wave 2).
