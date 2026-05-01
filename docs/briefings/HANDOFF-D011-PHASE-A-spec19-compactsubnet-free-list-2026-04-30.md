# Handoff Brief — D-011 Phase A: SPEC-19 R35a Amendment (CompactSubnet free_list)

**Date:** 2026-04-30
**Target agent:** ESPECIALISTA EM SPECS (camada 1, root do TCC)
**Bundle:** D-011 (Tier 3 hardening + bench enablement)
**Plan:** `codigo/relativist/docs/plans/2026-04-30-d-011-tier3-hardening-plus-bench-enablement.md` Phase A
**Origin:** QA finding `QA-D009-001` (CRITICAL) deferred from D-009 closure (commit `92145a0`, 2026-04-27); pre-tracked as `codigo/relativist/docs/backlog/TASK-0595-compactsubnet-free-list-followup.md`.

---

## Why this amendment exists (1-paragraph context)

D-009 introduced `Net.free_list: Vec<AgentId>` (SPEC-22 R1) and the SparseNet free-list invariants (R10a/R10b/R10c). The wire format for partitions, however, goes through `CompactSubnet` (`relativist-core/src/partition/compact.rs:38`) — a serde adapter that strips down `Net` to live agents only for transmission. The current implementation **drops the `free_list` entirely on serialization** (`from_net()` doesn't capture it, `compact.rs:64-83`) and **resets it to `Vec::new()` on deserialization** (`into_net()`, `compact.rs:113`). On TCP transport, this produces silent divergence: coordinator sends a `Partition` with a populated free-list, worker deserializes one with an empty free-list, and the next `create_agent` call on the worker side allocates from `next_id` instead of recycling — which violates SPEC-22 R10b/R10c (protected tombstone semantics) and SPEC-22 R12a (free-list reconciliation across merge). Bench Phase E (`--mode tcp`/docker smoke) is blocked until this is fixed at the spec level. SPEC-19 R35 (line 403) lists `CompactSubnet` as a wire optimization for `InitialPartition`/`FinalStateResult` payloads but does not specify the wire encoding of `free_list`. SPEC-04 §A7 (line 409) specifies that `build_subnet` MUST populate `free_list`, but says nothing about wire round-trip preservation.

## What you MUST do

Author one new requirement **R35a** in `specs/SPEC-19-delta-protocol.md` §3.4 (immediately after R35, line 403), and propagate cross-references to:
- `specs/SPEC-04-partition.md` §A7 (existing amendment block at line 409 — extend with a new clause covering wire round-trip)
- `specs/SPEC-22-arena-management.md` §3.8 (Amendments) — add A11 noting the SPEC-19 R35a dependency
- `specs/SPEC-18-wire-format-v2.md` §R33 (round-trip correctness, line 177 — extend to mention `free_list` field)

## R35a — exact authoring guidance (NOT final text; you decide the wording, but cover every clause below)

The new R35a MUST require:

1. **Wire-form preservation:** `CompactSubnet` MUST encode `free_list: Vec<AgentId>` as a new struct field, positioned after `root` (the current last field at `compact.rs:57`). Both `from_net()` and `into_net()` MUST round-trip this field byte-for-byte.

2. **Bincode/rkyv backward layout impact:** This is a `Net` wire layout change. PROTOCOL_VERSION is currently `3` (per SPEC-22 R9a, bumped from 2→3 on the SPEC-22 landing). R35a's change to `CompactSubnet` ALSO requires a PROTOCOL_VERSION bump → `4`. Justify the bump explicitly in the requirement: "v3 deserializers MUST reject v4 nets via the existing `UnsupportedVersion` reject path (SPEC-18 R31)". v3 binaries cannot decode v4 `CompactSubnet` payloads because bincode does not tolerate trailing fields on the decode side — exactly the same justification SPEC-19 R37 used for the `LocalWiringHint` field addition (line 407).

3. **Round-trip invariant:** R35a MUST state explicitly that `Net -> CompactSubnet -> Net` preserves `free_list` in addition to the existing `agents`, `ports`, `redex_queue`, `next_id`, `root`. Reference `relativist-core/src/partition/compact.rs:152-158` (`nets_equivalent` test helper) — those tests MUST be extended to include `free_list` equality. Estimated test floor delta: +2 round-trip tests (empty free-list, populated free-list).

4. **rkyv (`zero-copy` feature) coverage:** Per SPEC-18 R21 (line 140), `CompactSubnet` derives rkyv. The new `free_list: Vec<AgentId>` field is `Vec<u32>` (since `AgentId = u32`); rkyv handles `Vec<u32>` natively with 4-byte alignment. R35a MUST require that `--features zero-copy` round-trip tests for `CompactSubnet` cover the new field; estimated +1 archived round-trip test.

5. **Empty/non-empty boundary:** R35a MUST explicitly state that `Net::new()` produces `free_list: Vec::new()` and that this MUST round-trip as an empty `Vec<AgentId>` (not as missing/None). This closes the boundary case that today's bug masks (today, sender's empty free-list and sender's populated free-list both arrive as empty on the wire — the test that would catch the bug needs to use a populated source).

6. **Reconciliation interaction:** R35a MUST reference SPEC-22 R12a (merge free-list reconciliation, `specs/SPEC-22-arena-management.md:270`) and assert that the wire round-trip is what makes R12a soundness hold over the network: without R35a, the coordinator merges partitions whose free-lists were silently emptied by the wire, producing a merged `Net` whose `next_id` is correct but whose ID-reuse opportunities are permanently lost (memory waste compounds across rounds).

7. **Acceptance criterion:** R35a SHOULD reference Phase E smoke test in `codigo/relativist/docs/plans/2026-04-30-d-011-tier3-hardening-plus-bench-enablement.md` Phase E-4 — `docker compose run bench-tcp --benchmark ep_annihilation --sizes 1000 --workers 2 --chunk-size 100 --mode tcp` — as the integration-level acceptance gate. The amendment is "shipped" only when this smoke test produces a CSV with G1 isomorphism passing AND the worker's post-deserialization `free_list` is non-empty for at least one partition (verifiable via tracing).

## Cross-reference propagation

After R35a is authored, edit (in the same commit):

- **SPEC-04 §A7 amendment block** (`specs/SPEC-04-partition.md:409`) — append: "Wire round-trip of the partition's `free_list` is governed by SPEC-19 R35a. `build_subnet`'s populated `free_list` MUST survive serialization to and deserialization from the wire form via `CompactSubnet`."
- **SPEC-22 §3.8 (Amendments)** — append A11: "SPEC-19 R35a (CompactSubnet wire encoding of free_list) MUST be implemented atomically with SPEC-22's free-list across the wire path; without R35a, R10b/R10c semantics degrade silently on multi-worker TCP transport. PROTOCOL_VERSION bump: 3 → 4."
- **SPEC-18 R33** (`specs/SPEC-18-wire-format-v2.md:177`) — extend: "Round-trip correctness MUST additionally hold for `Net.free_list` through the `CompactSubnet` layer (SPEC-19 R35a). This is verified by extending the `nets_equivalent` test helper at `relativist-core/src/partition/compact.rs:152-158` to compare `free_list`."
- **SPEC-18 §R31 PROTOCOL_VERSION** — bump the documented value from 3 to 4, with cross-reference to SPEC-19 R35a as the justification.

## Stages

This is a **spec-only** amendment (no code in this dispatch). The pipeline is:

1. **Round 1 (you, ESPECIALISTA EM SPECS):** author R35a + cross-references. Output: edited specs + closure log entry queued.
2. **spec-critic Round 1:** adversarial review — does R35a fully eliminate the silent-drop failure mode? Does the PROTOCOL_VERSION bump conflict with anything? Is the rkyv interaction specified correctly? Output: `codigo/relativist/docs/spec-reviews/SPEC-19-R35a-round1-2026-04-30.md`.
3. **Round 2 (you again, defender):** address findings, revise spec, write closure log `codigo/relativist/docs/spec-reviews/SPEC-19-R35a-closure-2026-04-30.md`.
4. **Commit:** `spec(d-011): amend R35a — CompactSubnet free_list wire encoding (PROTOCOL_VERSION 3 → 4)` on branch `v2-development`.

**Gate:** spec-critic Round 2 closes with no objections. After this commit, Phase B-1 (developer fix in `relativist-core/src/partition/compact.rs`) is unblocked.

## Files in scope (this dispatch)

- `codigo/relativist/specs/SPEC-19-delta-protocol.md` — add R35a after R35 (~line 403)
- `codigo/relativist/specs/SPEC-04-partition.md` — append clause to A7 amendment block (~line 409)
- `codigo/relativist/specs/SPEC-22-arena-management.md` — append A11 to Amendments section
- `codigo/relativist/specs/SPEC-18-wire-format-v2.md` — extend R33; bump PROTOCOL_VERSION 3→4 in §R31
- `codigo/relativist/docs/spec-reviews/SPEC-19-R35a-closure-2026-04-30.md` — closure log (you author after Round 2)

## Files explicitly OUT of scope (deferred to Phase B-1)

- `codigo/relativist/relativist-core/src/partition/compact.rs` — implementation fix
- `codigo/relativist/relativist-core/src/protocol/coordinator.rs:570` — `PROTOCOL_VERSION` constant bump
- Any test additions

These ship in the next D-011 phase under the developer agent. Do NOT touch them in this dispatch.

## Reference reading (in this order)

1. `codigo/relativist/specs/SPEC-19-delta-protocol.md` lines 228-407 (§3.4 wire protocol, including R35 and R37 PROTOCOL_VERSION bump precedent)
2. `codigo/relativist/specs/SPEC-22-arena-management.md` lines 1-100 (free-list intro), 250-275 (build_subnet/merge amendments), 800-870 (test plan + status closures)
3. `codigo/relativist/specs/SPEC-18-wire-format-v2.md` lines 140-180 (rkyv R21 + round-trip R33), 530-540 (CompactSubnet/rkyv interaction note)
4. `codigo/relativist/relativist-core/src/partition/compact.rs` lines 1-145 (the actual buggy code)
5. `codigo/relativist/docs/qa/QA-PHASE-D009-spec22-arena-2026-04-27.md` — search for `QA-D009-001` for the original adversarial finding
6. `codigo/relativist/docs/backlog/TASK-0595-compactsubnet-free-list-followup.md` — pre-tracked task

## Test floor reminder

After this Phase A spec commit, code-side test floor remains: **1683 default / 1726 zero-copy / 1680 streaming-no-recycle**. The Phase B-1 implementation will add ≥3 new tests (round-trip empty, round-trip populated, rkyv round-trip). The new floor target after B-1 ships: ≥1686 default / ≥1729 zero-copy.

## Output expected back to user

1. Edits applied to the 4 spec files above
2. Closure log at `codigo/relativist/docs/spec-reviews/SPEC-19-R35a-closure-2026-04-30.md`
3. Single commit on `v2-development`: `spec(d-011): amend R35a — CompactSubnet free_list wire encoding (PROTOCOL_VERSION 3 → 4)`
4. Brief status report (under 150 words) summarizing: what R35a says, what spec-critic Round 1 raised, how Round 2 addressed it, what Phase B-1 (developer) needs to do next.

---

**Done. User: copy this entire file content (or the relevant subset) as the prompt to ESPECIALISTA EM SPECS in the TCC root session. The agent has its own access to read all spec files referenced.**
