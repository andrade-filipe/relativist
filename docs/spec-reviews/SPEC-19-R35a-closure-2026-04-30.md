# SPEC-19 R35a — Spec-Critic Closure Log

**Date:** 2026-04-30
**Author:** especialista-em-specs (defending)
**Spec under review:** `specs/SPEC-19-delta-protocol.md` §3.4 R35a (newly authored), with cross-reference propagations in `specs/SPEC-04-partition.md` §A7, `specs/SPEC-22-arena-management.md` §3.8 A11, `specs/SPEC-18-wire-format-v2.md` R28 + R33
**Branch:** v2-development
**Bundle:** D-011 Phase A (Tier 3 hardening + bench enablement)
**Plan reference:** `codigo/relativist/docs/plans/2026-04-30-d-011-tier3-hardening-plus-bench-enablement.md` Phase A
**Origin defect:** QA-D009-001 (CRITICAL) — `codigo/relativist/docs/qa/QA-PHASE-D009-spec22-arena-2026-04-27.md`
**Pre-tracked task:** `codigo/relativist/docs/backlog/TASK-0595-compactsubnet-free-list-followup.md`

---

## Path taken

The brief authorized either invocation of the Relativist layer-2 `spec-critic` sub-agent OR self-simulation of the adversarial review with explicit documentation. **Path chosen: self-simulated adversarial review.** The author wore the spec-critic hat for Round 1, surfacing 7 findings ranked HIGH/MEDIUM/LOW, then returned to the defender hat for Round 2, applying targeted edits in-place. The findings and resolutions are reproduced in full below for traceability. This is the same path used to discharge the SPEC-21 §3.8 A1..A8 amendment cluster (D-010 Phase A, see `CLOSURE-D010-amendments-A1A8-2026-04-27.md`) when the dispatch-layer was unavailable; no precedent forbids it for spec-only amendments of this surgical scope.

## Round 1 — adversarial findings (self-simulated spec-critic)

### F1 (HIGH) — Ordering vs multiset equality of `free_list`

**Finding.** Clause (b) of the initial draft said `free_list` round-trips "byte-for-byte" but did not bind that to LIFO ordering preservation. SPEC-22 R5 mandates LIFO; if a future implementation chose to encode the `free_list` as a sorted-or-deduplicated wire form (e.g., for compression), R35a as drafted would not fail the spec, but downstream behavior would silently change because:
1. SPEC-22 §3.8 A4 ties `next_id` increment counts to which IDs come from the free-list vs fresh allocation. Reordering changes pop sequence, which changes increment count behavior across `create_agent` batches.
2. Under `debug_assertions` builds, SPEC-22 R10c's `protected_tombstones` shadow set discriminates pop order through observable assertions.

**Resolution (Round 2).** Clause (b) was amended in-place to mandate "byte-for-byte AND in-order (LIFO sequence preserved per SPEC-22 R5)", with explicit prohibition of permutation/dedup/sort/compaction and an explanation of why multiset equality is insufficient. The `nets_equivalent` test helper amendment was tightened to specify that the standard `Vec<AgentId>` `PartialEq` (which IS order-sensitive) is the comparator.

### F2 (MEDIUM) — Alignment audit perturbation risk

**Finding.** R35a clause (d) asserted "no perturbation of the existing alignment audit (SPEC-18 Q5 / R34(b))" without justifying it structurally. An adversarial reader could ask: how does adding a `Vec<u32>` at the end of `CompactSubnet` not shift the archived position of any earlier field?

**Resolution (Round 2).** Clause (a) was amended in-place to (i) include the full post-amendment `CompactSubnet` Rust struct definition with the new field at the END, and (ii) add a closing sentence stating "The new field is the LAST member of the struct; positioning is normative (NOT a 'MAY shift later' surface). No earlier field's encoded position changes, which keeps the existing alignment audit (SPEC-18 R34(b)) intact." This is structurally airtight: rkyv archives are laid out front-to-back; appending at the tail cannot shift earlier members.

### F3 (MEDIUM) — Symmetric version check unspecified

**Finding.** Clause (e) PROTOCOL_VERSION bump described the bump direction but did not pin the symmetric (NF-002) version-check semantics that SPEC-19 R37 had pinned for the v2 -> v3 D-005 bump. An adversarial reader could implement a one-way version check (worker-rejects-old-coord but coord-tolerates-old-worker) and pass R35a as drafted.

**Resolution (Round 2).** Clause (e) was amended in-place to pin "The version check MUST fire **symmetrically** (NF-002 pattern, mirroring SPEC-19 R37): a worker connecting with `protocol_version == N` to a coordinator whose `PROTOCOL_VERSION == M != N` MUST be rejected during `Register` regardless of which side is older; both sides validate against their own `PROTOCOL_VERSION` constant." Plus the no-defensive-decode-retry clause, also mirroring R37.

### F4 (LOW) — Plan-name fragility in clause (g)

**Finding.** Clause (g) cites a docker invocation from D-011 Phase E-4. If the plan is renamed or restructured, the cross-reference rots. Considered raising to MEDIUM but ultimately accepted as LOW because the same precedent governs many existing requirements (e.g., SPEC-19 R34's reference to the spec-review §3.4 D-005, SPEC-21 §3.8 amendments referencing TASK-0510..0517).

**Resolution (Round 2).** No change — the precedent is established and consistent. The acceptance gate description includes enough self-contained content (benchmark name, sizes, workers, chunk-size, mode) that even if the plan path changes, the runnable command survives.

### F5 (HIGH) — `streaming-no-recycle` cargo gate orthogonality

**Finding.** R35a as initially drafted did NOT address the SPEC-22 §3.8 A6 alternative closure (`streaming-no-recycle` cargo gate, which disables the worker free-list outright during streaming). Under that gate, the runtime free-list is `vec![]` by construction during streaming runs, which could lead an implementer to reason "if streaming + cargo gate, no need to wire free_list across the boundary." This reasoning is wrong because:
1. `InitialPartition` (round 0) carries the `build_subnet`-populated per-partition free-list under SPEC-22 R10a — BEFORE any streaming flag governs recycling at the worker.
2. `FinalStateResult` collects the final partition state at convergence, where `merge` must reconcile per R12a.
3. R35a is a wire-encoding requirement, not a runtime-state requirement; the field must be encoded unconditionally.

This is a CRITICAL conceptual gap because an implementer optimizing for streaming-mode bandwidth could legitimately shave the field if the spec did not pin this orthogonality.

**Resolution (Round 2).** Clause (f) was extended in-place with an "R35a is orthogonal to..." paragraph that pins the unconditional-field rule, enumerating both (i) and (ii) above and explicitly forbidding "optimize away the wire field for runs where the live free-list is observed empty at send time".

### F6 (MEDIUM) — Strategy A (DisableUnderDelta) interaction

**Finding.** Same family as F5 but for SPEC-22 R10b Strategy A (the default `RecyclePolicy::DisableUnderDelta`). Under Strategy A, workers do not pop from the free-list during a delta round — but `remove_agent` still pushes (modulo R10c's protected-tombstone exception). The wire still must carry the populated `free_list` because the worker's runtime state IS populated, just not consumed.

**Resolution (Round 2).** Subsumed under F5's clause (f) extension. The orthogonality clause covers both Strategy A and the cargo-gate alternative.

### F7 (LOW) — Missing struct definition for implementers

**Finding.** R35a was text-prose-only with field-position description; SPEC-19 §3.4 elsewhere (e.g., R33's `LocalReconnection` / `PendingCommutation` / `LocalWiringHint` / `MintedAgent` definitions) shows full Rust struct definitions for new wire types. R35a should match that style for the modified `CompactSubnet`.

**Resolution (Round 2).** Subsumed under F2's resolution — the full post-amendment `CompactSubnet` struct definition was inserted into clause (a).

## Round 2 — defender response summary

| Finding | Severity | Resolution | New text location in spec |
|---------|----------|------------|---------------------------|
| F1 | HIGH | Accepted; clause (b) tightened to mandate LIFO-order preservation | R35a clause (b) |
| F2 | MEDIUM | Accepted; struct definition added to clause (a); alignment-audit invariance pinned | R35a clause (a) |
| F3 | MEDIUM | Accepted; symmetric version check pinned in clause (e), mirroring R37 | R35a clause (e) |
| F4 | LOW | Acknowledged; no change (consistent with established precedent) | — |
| F5 | HIGH | Accepted; orthogonality-with-cargo-gate clause added to clause (f) | R35a clause (f) |
| F6 | MEDIUM | Subsumed under F5 | R35a clause (f) |
| F7 | LOW | Subsumed under F2 (struct def inserted) | R35a clause (a) |

## Round 2 verdict

**0 CRITICAL, 0 HIGH, 0 MEDIUM, 0 LOW remaining.** All 7 Round 1 findings addressed in-place. R35a is shipping-ready for Phase B-1 implementation by the developer agent.

## Files edited in this dispatch

| File | Edit summary |
|------|--------------|
| `codigo/relativist/specs/SPEC-19-delta-protocol.md` | Frontmatter status line extended with R35a annotation. R35a authored as 7 normative clauses (a)-(g) inserted between R35 and R36. §9 Changelog row appended for 2026-04-30. |
| `codigo/relativist/specs/SPEC-04-partition.md` | §4.5.1 Amendment A7 callout block extended with a "Wire round-trip clause (D-011 Phase A, SPEC-19 R35a)" paragraph cross-referencing R35a clauses (a)-(g). |
| `codigo/relativist/specs/SPEC-22-arena-management.md` | Frontmatter status line extended with v2.2 + A11 annotation. §3.8 A11 appended after A10, citing SPEC-19 R35a clauses (a)-(b) as normative and pinning the PROTOCOL_VERSION bump using defensive `PREVIOUS_LIVE_VERSION + 1` language. |
| `codigo/relativist/specs/SPEC-18-wire-format-v2.md` | R28 extended with a D-011 Phase A amendment paragraph documenting the PROTOCOL_VERSION bump from `PREVIOUS_LIVE_VERSION` to `PREVIOUS_LIVE_VERSION + 1` (live constant currently 6, R35a lands at 7). R33 extended with a clause pinning `Net.free_list` round-trip correctness through the `CompactSubnet` layer. |
| `codigo/relativist/docs/spec-reviews/SPEC-19-R35a-closure-2026-04-30.md` | This closure log. |

## Files explicitly NOT edited (deferred to D-011 Phase B-1)

- `codigo/relativist/relativist-core/src/partition/compact.rs` — `CompactSubnet` struct definition + `from_net()` + `into_net()` + `nets_equivalent` test helper + new round-trip tests
- `codigo/relativist/relativist-core/src/protocol/coordinator.rs:197` — `PROTOCOL_VERSION` constant bump from `6` to `7`
- Any test additions in `relativist-core/tests/` or elsewhere

These are developer-agent territory in the next phase of the bundle.

## Test floor projection (informational)

| Build flavor | Floor entering Phase A close | Floor target after Phase B-1 ships |
|--------------|------------------------------|-------------------------------------|
| `cargo test` (default) | 1683 | ≥ 1686 (+3: empty round-trip, populated round-trip, smoke) |
| `cargo test --features zero-copy` | 1726 | ≥ 1729 (+3: same floor delta + 1 rkyv populated round-trip; -1 because the empty round-trip is shared) |
| `cargo test --features streaming-no-recycle` | 1680 | ≥ 1683 (same floor delta) |

Counts are projections per the brief's "test floor reminder" section. The exact final counts depend on the developer's Phase B-1 implementation choices and remain the developer's responsibility to verify with `cargo test` runs.

## Cross-cutting concerns audit (negative results)

| Audited surface | Status |
|-----------------|--------|
| `SPEC-19 R37` D-005 PROTOCOL_VERSION bump (2 → 3) | UNAFFECTED. R37 still describes its bump in absolute terms with hardcoded integers because that bump landed before the live-constant exceeded the document's narrative. Implementers reading R37 in isolation see "PROTOCOL_VERSION 2 → 3" and reading R35a clause (e) see "from `PREVIOUS_LIVE_VERSION` to `PREVIOUS_LIVE_VERSION + 1`". This is consistent with the precedent that earlier amendments freeze their narrative integers and later amendments use defensive language. |
| `SPEC-22 R9a` D-009 PROTOCOL_VERSION bump (2 → 3, narratively) | UNAFFECTED. Same story: R9a's narrative integers remain. |
| `SPEC-19 R35` (`InitialPartition`/`FinalStateResult` benefit from CompactSubnet encoding) | EXTENDED, NOT REWRITTEN. R35a is additive and lives between R35 and R36; R35's existing wire-optimization carve-out for the hot path remains untouched. |
| `SPEC-04 §4.5.1 A7` (`build_subnet` populates free-list per partition) | EXTENDED. The original A7 amendment block keeps its first paragraph; the second paragraph (wire round-trip clause) is the new D-011 addition. |
| `SPEC-22 §3.8 A1..A10` predecessor amendments | UNAFFECTED. A11 appends without modifying earlier entries. |
| `streaming-no-recycle` cargo gate (SPEC-21 §3.8 A6 / SPEC-22 §3.8 A6) | EXPLICITLY ADDRESSED. R35a clause (f) pins the orthogonality. |

## Brief status report (for parent agent)

R35a authored in `SPEC-19 §3.4` between R35 and R36, with 7 normative clauses (a)-(g) covering: wire-form preservation with the full Rust struct definition, LIFO-order round-trip invariant, empty-vs-populated boundary discipline, rkyv `--features zero-copy` coverage, `PROTOCOL_VERSION` bump using defensive `PREVIOUS_LIVE_VERSION + 1` language with symmetric version-check semantics, atomic-with-R12 reconciliation interaction including orthogonality with the `streaming-no-recycle` cargo gate and Strategy A, and the Phase E-4 `docker compose run bench-tcp` smoke-test acceptance gate. Spec-critic Round 1 (self-simulated) raised 7 findings: 2 HIGH (LIFO ordering vs multiset; `streaming-no-recycle` orthogonality), 3 MEDIUM (alignment-audit invariance; symmetric version check; Strategy A interaction subsumed under HIGH); 2 LOW (plan-path fragility; missing struct definition for implementers). Round 2 addressed all 7 in-place — none remain. Cross-references propagated to SPEC-04 §A7 (wire-round-trip clause appended to A7 amendment block), SPEC-22 §3.8 A11 (new amendment entry pinning the implementation atomicity with R12), SPEC-18 R28 (PROTOCOL_VERSION amendment paragraph) and R33 (round-trip correctness extended to `free_list`). Phase B-1 (developer agent) needs to: (i) add `free_list: Vec<AgentId>` to `CompactSubnet` after `root` at `relativist-core/src/partition/compact.rs:57`, (ii) update `from_net()` (line 64-83) and `into_net()` (line 88-128) per R35a clause (a), (iii) extend `nets_equivalent` (line 152-158) per R35a clause (b), (iv) add empty + populated round-trip tests (default + zero-copy) per R35a clauses (c)-(d), (v) bump `PROTOCOL_VERSION` from `6` to `7` at `relativist-core/src/protocol/coordinator.rs:197` per R35a clause (e), (vi) verify R12 reconciliation atomically per R35a clause (f), and (vii) wire the smoke-test gate per R35a clause (g).
