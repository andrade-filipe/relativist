# SPEC-REVIEW-20 — Round 3 (continuation): NF closure pass

**Date:** 2026-04-24
**Author:** ESPECIALISTA EM SPECS (Round 3 closure; not an adversarial review)
**Target:** `specs/SPEC-20-elastic-grid.md` post-NF-closure pass
**Predecessor reviews:**
- Round 1: `docs/spec-reviews/archive/SPEC-REVIEW-20-round-1-2026-04-24.md` (30 findings, gate BLOCK)
- Round 2: `docs/spec-reviews/archive/SPEC-REVIEW-20-round-2-2026-04-24.md` (CONDITIONAL_PASS, 11 fresh NFs + 2 polish items)
- Round 3 partial (housekeeping, commit d891d63): closed NF-002 only — recorded as the existing "Round 3 — 2026-04-24 (housekeeping after TCC-root theoretical work)" entry in §11 of SPEC-20.

This document is the **continuation** of that partial Round 3 — it closes the remaining 10 NFs and the 1 amendment-target polish item flagged by the Round-2 review's "Specialist-em-specs TODO list". It is structured as a per-NF audit trail (verdict, evidence, diff pointer) modelled on `SPEC-REVIEW-19-section-3.4-D-005-2026-04-23-REREVIEW-R3.md`.

---

## 1. Round-2 NF closure status

| ID | Severity | Round-2 verdict | Round-3 verdict | Pass that closed it |
|---|---|---|---|---|
| NF-001 | HIGH | Open | **CLOSED** | This pass |
| NF-002 | MEDIUM | Open | **CLOSED** | Round 3 housekeeping (commit d891d63) |
| NF-003 | HIGH | Open | **CLOSED** | This pass |
| NF-004 | MEDIUM | Open | **CLOSED** | This pass |
| NF-005 | MEDIUM | Open | **CLOSED** | This pass |
| NF-006 | MEDIUM | Open | **CLOSED** | This pass |
| NF-007 | MEDIUM | Open | **CLOSED** | This pass |
| NF-008 | LOW | Open | **CLOSED** | This pass |
| NF-009 | LOW | Open | **CLOSED** | This pass |
| NF-010 | LOW | Open | **CLOSED** | This pass |
| NF-011 | MEDIUM | Open | **CLOSED** | This pass |

**Summary:** 11 / 11 Round-2 NFs CLOSED. Polish item 12 (A3 amendment-target mismatch) FIXED. Polish item 13 (proof-sketch polish in §5.1) DEFERRED with rationale (cost > value at this pipeline stage).

---

## 2. Per-NF audit

### NF-001 — `Net::union` undefined in SPEC-02

**Severity:** HIGH
**Round-2 location:** §4.2.2 v1-mode step 4; §4.2.2 delta-mode step 4
**Verdict:** **CLOSED.**

**Evidence.** §3.8 amendment **A7** added (preferred Round-2 option). The amendment formally specifies:

> `Net::union(self, other: Net) -> Net` under the precondition that the caller has already ensured **disjoint `AgentId` ranges** ... Border `FreePort` entries on either side are preserved in the resulting net's free-port list; the operation does NOT detect or resolve cross-net `FreePort` matches — that is `merge()`'s responsibility (SPEC-05).

The disjointness precondition is established by SPEC-20's existing call-site discipline: reclaimed partitions are renumbered via `remap_partition_ids` (SPEC-04 R30 / amendment A4) BEFORE union, and freshly-split partitions occupy a disjoint `IdRange` per `compute_id_ranges(K_eff_new)` (SPEC-20 R13). §4.2.2 v1-mode step 4 prose was updated to call out this precondition-establishment step explicitly.

**Diff pointer.** `specs/SPEC-20-elastic-grid.md` §3.8 A7 (new); §4.2.2 v1-mode step 4 (prose extended).

**Why not the alternative.** The Round-2 review offered an alternative: rewrite §4.2.2 to use `merge()` over a synthetic single-partition `PartitionPlan`. This was rejected because (a) `merge()` consults `FreePort` cross-references (SPEC-05 R20-R23), and the SPEC-20 use case does NOT need cross-net FreePort resolution at union time — that resolution is the responsibility of the subsequent `split()`'s border-id allocation; (b) the alternative would force `merge()` to accept partitions with mismatched round-state, which is the exact concern that SC-021 / R24c was designed to forbid.

---

### NF-003 — R4-delta self-worker symmetry under-specified

**Severity:** HIGH
**Round-2 location:** R4-delta clause (§3.1)
**Verdict:** **CLOSED.**

**Evidence.** Added new requirement **R4-delta-self-symmetry** appended to R4-delta. The clause MUSTs that the in-process self-worker execute SPEC-19 R24's full worker-side delta state machine: (i) maintain its own `partition` and `border_id` endpoint state; (ii) receive `RoundStart(deltas)` via `ChannelTransport`; (iii) apply deltas via `apply_border_deltas(partition, deltas)`; (iv) reduce via `reduce_all(partition)`; (v) emit `RoundResult(deltas)` back through the channel. The clause includes an explicit prohibition on short-circuiting `reduce_all` over the full self-partition without the delta loop, with two failure-mode cited rationales (a) BorderGraph consistency invariant break, (b) silent D2 violation.

Test EG-U4-delta-wire-symmetry added to §7.1: runs the same partition through the in-process self-worker and a remote worker, captures both `RoundResult.border_deltas` payloads via instrumented channel transport, and asserts structural equivalence.

**Diff pointer.** §3.1 R4-delta-self-symmetry (new bulleted clause); §7.1 EG-U4-delta-wire-symmetry (new test row).

**D2 (Local Reduction Equivalence) consequence.** The clause re-establishes D2 for the self-worker by construction: identical code path → identical reduction trace shape on the wire.

---

### NF-004 — Metric non-collision audit not committed to written record

**Severity:** MEDIUM
**Round-2 location:** R38a (§3.6)
**Verdict:** **CLOSED.**

**Evidence.** R38a was rewritten from prose-only to an exhaustive enumeration. The audit lists all 8 SPEC-19 R45 fields by name (`coordinator_free_rounds`, `border_deltas_received_per_round`, `border_deltas_sent_per_round`, `coordinator_border_resolutions_per_round`, `delta_bytes_sent_per_round`, `delta_bytes_received_per_round`, `coordinator_resolve_time_per_round`, `border_graph_update_time_per_round`) and all 7 SPEC-20 R38 fields. Disjointness is argued by-inspection on field-name prefixes: every SPEC-20 field uses one of `workers_`, `effective_`, `partitions_`, `retained_`, `join_round_`; no SPEC-19 field uses any of those prefixes.

Source verification: SPEC-19 R45 enumeration cross-checked against `specs/SPEC-19-delta-protocol.md` lines 464-494 at the time of this revision (2026-04-24); committed verbatim.

**Diff pointer.** §3.6 R38a (rewritten).

---

### NF-005 — `reconstruct` 3-argument signature unamended

**Severity:** MEDIUM
**Round-2 location:** §4.2.2 delta-mode step 4; Open Issue #4 (§9, informal)
**Verdict:** **CLOSED.**

**Evidence.** §3.8 amendment **A8** added. The amendment promotes the previously informal Open Issue #4 to a formal cross-spec amendment with explicit signature:

> SPEC-19 R38 `reconstruct` MUST accept an OPTIONAL third argument `reclaimed_partitions: Vec<Partition>` defaulting to the empty vector. ... When `reclaimed_partitions` is empty, behavior is identical to the existing 2-argument signature; downstream SPEC-19 callers (SPEC-19 R29 final merge) are unaffected.

§4.2.2 delta-mode step 4 prose was rewritten to reference A8 explicitly and to specify how reclaimed partitions are materialized from `retained_last_acked` (apply snapshot deltas to `retained_initial`, then `Net::union` per A7, with the disjointness precondition established by R30 / A4 renumbering). Special case for `checkpoint_partitions = true` is called out.

Open Issue #4 in §9 was updated from "needs a small amendment to SPEC-19 R29" (incorrect target — should be R38) to "RESOLVED 2026-04-24 by promotion to formal §3.8 amendment A8".

**Diff pointer.** §3.8 A8 (new); §4.2.2 delta-mode step 4 (prose rewritten); §9 Open Issue 4 (status updated).

---

### NF-006 — A3/A4 lacked error-return signatures

**Severity:** MEDIUM
**Round-2 location:** §3.8 A3, A4
**Verdict:** **CLOSED.**

**Evidence.** Both amendments rewritten with `Result` return types and named `PartitionError` variants:

- **A3:** `PartitionPlan::allocate_border_ids(count: u32) -> Result<Range<u32>, PartitionError>`, error variant `PartitionError::BorderIdSpaceExhausted { requested: u32, available: u32 }` when `count > u32::MAX - next_border_id`.
- **A4:** `remap_partition_ids(partition: Partition, new_range: IdRange) -> Result<Partition, PartitionError>`, error variant `PartitionError::NewRangeTooSmall { partition_size: u32, range_size: u32 }` when `partition.agent_count() > new_range.len()`.

The variants follow the existing `PartitionError` shape in SPEC-04 (the Round-2 review confirmed this enum already exists and these variants compose cleanly).

**Diff pointer.** §3.8 A3, A4 (signatures revised).

---

### NF-007 — D == K_eff edge case unspecified

**Severity:** MEDIUM
**Round-2 location:** R26 (§3.3.4)
**Verdict:** **CLOSED.**

**Evidence.** Added new requirement **R26a** with two branches:

- **Hybrid mode.** Coordinator discards `retained_last_acked` for all D reclaimed slots, falls back per R27 with the still-progressing self-partition, and SKIPS the `FinalStateRequest` step of §4.2.2 (no recipients). Reclaimed `retained_initial[w]` snapshots are queued for re-introduction at the next `AcceptingMembershipChanges` window via R5a + R15 if any worker rejoins.
- **Non-hybrid mode.** Coordinator transitions to `Error` per R27. `FinalStateRequest` SKIPPED (no survivors AND no executor). Reclaimed snapshots released as part of `Error` cleanup.

R27's preamble was updated to refine that R26a covers the single-round D == K_eff specialization. EG-U9 test description was extended to require both branches (hybrid SoloReducing fallback + non-hybrid Error path).

**Diff pointer.** §3.3.4 R26a (new); §3.3.4 R27 (precondition refined); §7.1 EG-U9 (description expanded).

---

### NF-008 — TimerKind derivation said "e.g." instead of MUST

**Severity:** LOW
**Round-2 location:** §4.1.3 TimerKind block
**Verdict:** **CLOSED.**

**Evidence.** §4.1.3 now mandates `#[repr(u32)]` on the enum and assigns explicit discriminants 0-3 (`InitialWait = 0`, `JoinWindowMin = 1`, `JoinWindowMax = 2`, `Collect = 3`). The Rust comment block above the enum was rewritten to require this MUST and to spell out the rationale: stable, portable, well-defined cast for `TimerId = kind as u32`; portable test assertions on `TimerId` values; log-tooling decode independence from per-build metadata.

**Diff pointer.** §4.1.3 TimerKind block.

---

### NF-009 — `JoinNackReason::ProtocolVersionMismatch` shape vs SPEC-19 R37 `RegisterNack`

**Severity:** LOW
**Round-2 location:** R35 / §4.1.4 SPEC-19 cross-reference
**Verdict:** **CLOSED.**

**Evidence.** Added a new requirement **R35-cross-spec-version-shape** between R35 and R35a. The requirement pins the canonical payload `{ coordinator: u32, worker: u32 }` for both rejection paths and obligates SPEC-19 R37's next revision to either (a) reuse SPEC-20's `JoinNackReason::ProtocolVersionMismatch` variant inside `RegisterNack`'s reason field, OR (b) inline a shape-equivalent `RegisterNackReason::ProtocolVersionMismatch { coordinator: u32, worker: u32 }`. The variant's docstring in R35 cross-references this requirement.

The current SPEC-19 R37 text does NOT pin a payload shape (it defers to the existing `coordinator.rs` rejection path). This SPEC-20 requirement is therefore the canonical shape definition for both protocols.

**Diff pointer.** §3.5 R35 `JoinNackReason` (docstring extended); §3.5 R35-cross-spec-version-shape (new).

---

### NF-010 — EG-U15 conflated Register and JoinRequest paths

**Severity:** LOW
**Round-2 location:** §7.1 EG-U15
**Verdict:** **CLOSED.**

**Evidence.** Replaced the single EG-U15 row with two rows:

- **EG-U15a** `test_protocol_version_mismatch_register_path`: initial `Register` handshake (worker is part of the initial `WaitingForWorkers` cohort); v3 worker connects to v4 coordinator; rejected with `RegisterNack { reason: ProtocolVersionMismatch { coordinator: 4, worker: 3 } }` per SPEC-19 R37.
- **EG-U15b** `test_protocol_version_mismatch_join_request_path`: mid-session `JoinRequest` handshake (worker connects after the BSP loop has started); v3 worker connects to v4 coordinator; rejected with `JoinNack { reason: JoinNackReason::ProtocolVersionMismatch { coordinator: 4, worker: 3 } }` per SPEC-20 R35. Additionally asserts shape-identity between EG-U15a's and EG-U15b's rejection payloads (closes NF-009 + NF-010 jointly).

**Diff pointer.** §7.1 EG-U15a / EG-U15b (replaces former EG-U15 row).

---

### NF-011 — R23a retention policy + R31 memory bound

**Severity:** MEDIUM
**Round-2 location:** R23a (§3.3.3); R31 (§3.3.6)
**Verdict:** **CLOSED.**

**Evidence — R23a.** Release-policy clause rewritten to enumerate three release conditions:
- (a) graceful completion via `LeaveRequest{kind: AfterResult}` (R22a);
- (b) coordinator-initiated `Shutdown`;
- (c) reclaim consumes the slot — when R24a/R24b/R26/R26a reclaim renumbers and re-introduces the partition, the original slot is freed and `w` is removed from `W_active`.

Case (c) was the explicit gap in the Round-2 wording. The new prose also notes that R11's no-WorkerId-reuse rule guarantees a worker cannot be reclaimed twice within a run, so the reclaim-consumes-slot rule is unambiguous.

**Evidence — R31 memory bound.** The Round-2 wording `O(sum_{w in W_ever_active} |partition_w|)` was incorrect because `W_ever_active` is monotonically growing under churn. The rewritten R31 introduces two well-defined sets: `W_currently_active` (presently in `W_active`) and `W_pending_reclaim` (departure observed, retained slot not yet consumed). Both are bounded by current concurrency, NOT by cumulative join/leave history. The corrected bounds are:

- `retained_initial` memory: `O(sum_{w in W_currently_active ∪ W_pending_reclaim} |partition_w|)` — bounded by `K_eff + D_pending_reclaim ≤ 2 · K_eff` slots at any instant.
- `retained_last_acked` memory: `O(sum_{w in W_currently_active} |partition_w|)` at any instant; transient overlap during R31's atomic refresh window adds at most one extra slot per worker.

For TCC scope (K_eff ≤ 8), these bounds are trivially small.

**Diff pointer.** §3.3.3 R23a (clause (c) added; reclaim-consumes-slot rule); §3.3.6 R31 (bounds rewritten with correct sets).

---

### Item 12 (polish) — A3 amendment-target mismatch

**Verdict:** **FIXED.**

**Evidence.** Round-2 review observation: "A3 currently targets 'SPEC-04 R15' but R15 is about FreePort distinction; border_id allocation is R16-R18." The fix re-attributes A3 to a new requirement number (`R18a`) following SPEC-04's R16-R18 cluster, and the amendment heading explicitly notes the correction with a rationale clause: "(NOT R15; R15 is about FreePort Lafont vs Boundary distinction and is unrelated to border_id allocation)".

**Diff pointer.** §3.8 A3 (amendment heading + target re-numbered to R18a).

---

### Item 13 (polish) — proof sketch in §5.1

**Verdict:** **DEFERRED.**

**Rationale.** The Round-2 review labeled this OPTIONAL ("Skip if cost > value"). The existing R39-G1-elastic-departure breakdown in §3.7 already articulates the "reduce to ARG-001 Passo 11 via clean-boundary re-split" argument with sufficient precision (per-mode breakdown + ARG-006 P10/P11/P12 citation pattern). Pulling that argument into §5.1 would be a presentation polish, not a correctness improvement, and the spec is already at 1300+ lines after this NF closure pass. Cost > value at this stage. Deferred to a future polish-only edit if a downstream consumer asks for it; not blocking Stage 1.

---

## 3. Cross-spec amendment debt summary

The §3.8 amendment list now stands at A1-A8 (was A1-A6 before this pass). The cross-spec patches that ESPECIALISTA EM SPECS owns:

| Amendment | Target spec | Target requirement | Diff in this pass | Outstanding |
|---|---|---|---|---|
| A1 | SPEC-06 | R25 conditional | (no change) | Cross-ref patch in SPEC-06 next revision |
| A2 | SPEC-13 | R21 transitions | (no change) | Cross-ref patch in SPEC-13 next revision |
| A3 | SPEC-04 | new R18a (border_id allocator) | NF-006 + item 12 | Cross-ref patch in SPEC-04 next revision |
| A4 | SPEC-04 | new R19a (id remap) | NF-006 | Cross-ref patch in SPEC-04 next revision |
| A5 | SPEC-05 | GridConfig | (no change) | Cross-ref patch in SPEC-05 next revision |
| A6 | SPEC-19 | R45 metric coexistence | (audit committed in R38a) | Cross-ref patch in SPEC-19 next revision |
| **A7 (new)** | SPEC-02 | new `Net::union` primitive | NF-001 | Cross-ref patch in SPEC-02 next revision |
| **A8 (new)** | SPEC-19 | R38 signature extension | NF-005 | Cross-ref patch in SPEC-19 next revision |

The patches are all forward-references; they do not block Stage 1 of the SPEC-20 SDD pipeline. Stage 1 TASK-SPLITTER MAY proceed; the implementer relies on §3.8 as the canonical specification of these primitives until the predecessor specs catch up.

---

## 4. Test count guard

The closure pass added the following test rows to §7.1:
- `EG-U4-delta-wire-symmetry` (NF-003)
- `EG-U15a` and `EG-U15b` (replacing former `EG-U15`; NF-010)

`EG-U9` description was extended (NF-007). Net test count delta in §7.1: **+2** rows (one new test EG-U4-delta-wire-symmetry; one row split EG-U15 → EG-U15a + EG-U15b is +1 net). Correction: +2 rows total (1 new test + 1 row from the split).

This is a **specification-side** delta. No `src/` or `tests/` edits are made in this pass; test counts (`1181 default / 1224 zero-copy`) are unchanged on disk. The new test rows become tasks for Stage 1 TASK-SPLITTER and Stage 2 TEST-GENERATOR.

---

## 5. Status field update

Bumped `Status:` field in SPEC-20 frontmatter from:

> Draft — Round 2 (post-SPEC-REVIEW-20-round-1-2026-04-24)

to:

> Reviewed v2 — Round 3 closure landed 2026-04-24 (Round-2 verdict CONDITIONAL_PASS; Round-3 NF closure pass closed NF-001, NF-003 through NF-011; NF-002 was closed earlier in Round 3 housekeeping commit d891d63; per Round-2 review §"Gate decision", Round 3 is AVOIDABLE).

**Rationale for "Reviewed v2" (not "Draft — Round 3 (closure landed; pending spec-critic Round 3 review)").** The Round-2 verdict explicitly said "Round 3 is AVOIDABLE — the residual work is small-surface, has a clear owner, and does not re-scope any requirement." None of the NFs closed in this pass are structurally non-trivial:

- NF-001, NF-003, NF-005, NF-007 are localized requirement additions/clarifications (single-clause MUSTs or §3.8 amendment rows).
- NF-004, NF-006, NF-008, NF-009 are wording / signature tightening (no new behavior, no new states, no new wire format).
- NF-010 is a test-row split.
- NF-011 is a memory-bound correction + retention-policy clause clarification.
- Item 12 is a misnumbered amendment target.

No new requirement R-NN is renamed, no FSM state is added, no wire discriminant is introduced, no invariant is reframed. The escalation criterion ("structurally non-trivial enough to warrant a fresh adversarial pass") is not met for any individual NF, so the spec advances to "Reviewed v2" without a fresh spec-critic round. If, during Stage 1 TASK-SPLITTER or any subsequent stage, a specific closure is judged non-trivial in retrospect, that closure escalates individually to a Round 3 spec-critic re-review — the spec-review log structure supports per-finding escalation.

---

## 6. Stage gate

**SPEC-20 advances to Stage 1 TASK-SPLITTER (after this pass).**

Pre-Stage-1 hygiene check:
- All §3.8 amendments (A1-A8) are formally listed with target specs and target requirement numbers. (Pass.)
- All Round-2 NFs are CLOSED (NF-002 in commit d891d63; NF-001, NF-003-011 in this pass). (Pass.)
- Polish items: item 12 FIXED, item 13 DEFERRED with rationale. (Pass.)
- Status field bumped. (Pass.)
- §11 Change Log updated with both the housekeeping entry (already present) and the NF closure entry (added in this pass). (Pass.)
- Closure log committed at this path. (Pass — this file.)
- Theory-bridge IDs (ARG-001, ARG-005, ARG-006) all present in `docs/theory-bridge.md` last-updated 2026-04-24. (Pass — verified before edits.)
- No new ARG / DISC / REF cited in this pass that is not already in the bridge. (Pass — only ARG-005 / ARG-006 / ARG-001 are referenced, all CLOSED.)

**Open items carried forward** (NOT blockers for Stage 1):
- §9 Open Issue 5 (possible SPEC-20 split into Hybrid+Joining vs Departure) remains open as an organizational improvement; with ARG-006 CLOSED, the original splitting rationale is weakened. No commitment.
- A1-A8 cross-ref patches in predecessor specs (SPEC-02, SPEC-04, SPEC-05, SPEC-06, SPEC-13, SPEC-19) are forward-references; they do not block SPEC-20's pipeline progression.

---

## 7. Sign-off

This Round 3 (continuation) closure pass discharges all 11 Round-2 NFs and 1 polish item. The SPEC-20 spec is **Reviewed v2** and ready for Stage 1 TASK-SPLITTER.

No item is escalated to spec-critic Round 3.

**End of Round 3 (continuation) closure log.**
