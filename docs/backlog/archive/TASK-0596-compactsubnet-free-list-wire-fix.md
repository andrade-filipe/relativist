# TASK-0596 — CompactSubnet wire format must round-trip `free_list` (QA-D009-001 fix)

**Phase:** B-1 (D-011 hardening — CRITICAL fix for cross-worker partition transfers)
**Bundle:** D-011 — Tier 3 Hardening + Bench Enablement
**Status:** PENDING
**Priority:** P0 (CRITICAL — blocks Phase E TCP smoke)
**Spec:** SPEC-19 §3.4 R35a (amended in `c4c80b8`); SPEC-22 §3.7 R9a (free_list serde MUST), R10b/R12a (free-list / next_id consistency across coordinator and worker).
**Origin:** QA-D009-001 (deferred from D-009 Stage 6); supersedes pre-existing TASK-0595.
**Estimated complexity:** M (~120 LoC production + ~80 LoC tests)
**Estimated stages duration:** Stages 2→3→4→5→6 over ~1.5 days (RED→GREEN→REFACTOR for wire format change).

---

## Context

`Partition::subnet` is serialized through `CompactSubnet` (wired via `#[serde(serialize_with, deserialize_with)]` in `relativist-core/src/partition/types.rs`). Today's `CompactSubnet` carries no `free_list` field, so `into_net()` hard-codes `free_list: Vec::new()`. Every cross-worker partition transfer therefore drops the free-list silently → `next_id` diverges between coordinator and worker → SPEC-22 R10b/R12a violated; the `--mode tcp` benchmark path is functionally broken until this lands.

SPEC-19 R35a (committed `c4c80b8`) now mandates that the wire encoding of `CompactSubnet` MUST encode `free_list: Vec<AgentId>` as a suffix after the agent array. PROTOCOL_VERSION was bumped (`PREVIOUS_LIVE_VERSION + 1`) in the same amendment.

This task is the *implementation* of R35a. It blocks Phase E (TCP smoke) and Phase F-2 (TCP bench rodada).

## Dependencies

- **SPEC commit `c4c80b8`** — SPEC-19 R35a amendment defining the `free_list` wire suffix and PROTOCOL_VERSION bump. (Hard prerequisite — already landed.)
- No upstream task dependency in D-011.
- Replaces / absorbs the pre-existing TASK-0595 placeholder (which can be marked OBSOLETED-by-0596 in BACKLOG.md).

## Files in scope

| File | Change |
|------|--------|
| `relativist-core/src/partition/compact.rs` (~lines 38-145) | Add `free_list: Vec<AgentId>` field to `CompactSubnet`; populate in `from_net`; restore in `into_net`. |
| `relativist-core/src/partition/types.rs` | Verify `#[serde(serialize_with/deserialize_with)]` wiring picks up the new field; no API surface change to `Partition`. |
| `relativist-core/src/protocol/coordinator.rs` | Update PROTOCOL_VERSION constant (already amended in spec; must match wire) + comment block. |
| `relativist-core/tests/spec19_compactsubnet_free_list_roundtrip.rs` (new) | Regression test: `Partition` with non-empty `free_list` → serialize → deserialize → assert equality. Also: cross-version backward-compat probe (v4 wire frame missing free_list deserializes to `Vec::new()`). |

## Files explicitly OUT of scope

- `relativist-core/src/net/sparse.rs` (`SparseNet::to_dense` is not the wire path).
- All other Message variants in `relativist-core/src/protocol/messages.rs` — only Partition-bearing variants are affected.
- Any change to `Net::free_list` semantics (already correct per D-009).

## Acceptance criteria

1. `CompactSubnet` carries a `free_list: Vec<AgentId>` field (SPEC-19 R35a).
2. `CompactSubnet::from_net` sets `free_list: net.free_list.clone()`.
3. `CompactSubnet::into_net` sets `free_list: self.free_list` (per SPEC-22 R9a).
4. Round-trip preserves `Net.free_list` exactly: `CompactSubnet::from_net(&net).into_net() ≡ net` modulo agent ordering (per SPEC-22 R9a, SPEC-19 R35a).
5. `next_id` is consistent across coordinator and worker after partition transfer (SPEC-22 R10b/R12a).
6. PROTOCOL_VERSION constant in `coordinator.rs` matches the spec-mandated bumped value, with comment citing SPEC-19 R35a + commit `c4c80b8`.
7. New regression test `spec19_compactsubnet_free_list_roundtrip.rs` covers: (a) non-empty free_list round-trip; (b) empty free_list round-trip (regression for v4 baseline); (c) free_list with sparse / non-monotonic AgentIds.
8. All existing tests (1683 default / 1726 zero-copy) continue to pass — zero regression.

## Test floor delta expected

**+5 to +8 tests** added. New floor target after this task: **≥1688 default / ≥1731 zero-copy**.

## Key risks

- Wire format break is intentional (single-version v2; no live deployments). No backward-compat shim required.
- `bincode`/serde length-prefixed Vec encoding is straightforward — no manual framing logic needed in this task.
- Coupled with Phase E-4: the docker TCP smoke test is the integration validation that this task actually closes the cross-worker path.

## Notes

- Reuse the existing TASK-0595 acceptance criteria as a starting point; this task supersedes 0595 with the now-landed SPEC-19 R35a authority and the explicit PROTOCOL_VERSION bump.
- Closure log: append a note in `docs/spec-reviews/` referencing SPEC-19 R35a + this TASK in the closing of QA-D009-001.
