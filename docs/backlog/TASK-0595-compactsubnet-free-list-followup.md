# TASK-0595 — CompactSubnet wire format must include free_list (QA-D009-001 followup)

**Status:** TODO
**Spec:** SPEC-22 §3.7 R9a
**Bundle:** D-009 Stage 6 deferred (explicitly out of scope per user directive)
**Created:** 2026-04-27
**Origin:** QA-D009-001 from QA-PHASE-D009-spec22-arena-2026-04-27.md

---

## Background

`Partition::subnet` is serialized through `CompactSubnet` (wired via `#[serde(serialize_with, deserialize_with)]` in `partition/types.rs`). The `CompactSubnet` struct has **no `free_list` field**, so `into_net()` hard-codes `free_list: Vec::new()`. Every cross-worker partition transfer therefore drops the free_list silently.

SPEC-22 R9a mandates: "the `free_list` field MUST be included in serde serialization/deserialization of `Net`". The `CompactSubnet` path violates this for every cross-worker partition transfer — the only path that matters in distributed runs.

The bump to `PROTOCOL_VERSION = 5` in coordinator.rs gives the false impression that the wire-format support landed; in practice the Partition wire encoding is functionally unchanged from v4 with respect to free_list.

---

## Acceptance Criteria

1. `CompactSubnet` has a `free_list: Vec<AgentId>` field.
2. `CompactSubnet::from_net` sets `free_list: net.free_list.clone()`.
3. `CompactSubnet::into_net` sets `free_list: self.free_list`.
4. `CompactSubnet` has a versioned default for the field (backward compat across v4→v5 boundary): missing `free_list` in a v4 wire frame deserializes to `Vec::new()`.
5. A regression test serializes a `Partition` whose `subnet` has a non-empty `free_list`, deserializes, and asserts `subnet.free_list == original`.
6. PROTOCOL_VERSION comment updated to reflect free_list wire inclusion.
7. All 690 v1 floor tests pass. No regression against current baseline.

---

## Files to modify

- `relativist-core/src/partition/compact.rs` — add `free_list` field to `CompactSubnet`
- `relativist-core/src/partition/types.rs` — verify serde wiring picks up the new field
- `relativist-core/tests/` — add regression test for free_list round-trip through CompactSubnet
- `relativist-core/src/protocol/coordinator.rs` — update comment near PROTOCOL_VERSION

---

## Dependencies

None. Can be implemented independently of other D-009 tasks.

---

## Notes

This is the most impactful single fix for SPEC-22 correctness in distributed runs. The reason it was deferred from D-009 Stage 6 is that it touches the wire protocol (requires protocol compatibility analysis) and the test scope was out of the explicit Stage 6 mandate. It should be prioritized as a standalone bundle or the first task in the next D-009 followup bundle.

QA severity: CRITICAL (QA-D009-001).
