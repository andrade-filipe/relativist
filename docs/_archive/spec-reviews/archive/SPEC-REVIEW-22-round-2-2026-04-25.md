# SPEC-REVIEW-22 — Round 2: Arena Management — Closure Pass

**Date:** 2026-04-25
**Author:** especialista-specs (Round 2 closure / defender)
**Target:** `specs/SPEC-22-arena-management.md` (Status transitions Draft → Reviewed v2)
**Round 1 baseline:** `docs/spec-reviews/SPEC-REVIEW-22-round-1-2026-04-24.md` — 21 findings (4 C / 7 H / 6 M / 4 L), gate BLOCK.
**Round 1 verdict:** BLOCK (not CONDITIONAL_PASS); closure pass therefore had to be SUBSTANTIAL — every CRITICAL and HIGH addressed inline; MEDIUM/LOW deferred only with explicit in-spec gating.
**Predecessors re-consulted:** SPEC-01 (I3), SPEC-02 (R2/R10/R11/R12), SPEC-03 (assert_next_id_valid + rule debug assertions), SPEC-04 (R10a/R22 build_subnet), SPEC-05 (R12 merge), SPEC-18 (PROTOCOL_VERSION), SPEC-19 (BorderGraph), SPEC-20 §3.8 (canonical Amendments pattern).
**Format reference:** `docs/spec-reviews/archive/SPEC-REVIEW-20-round-2-2026-04-24.md` (per-finding verdict / evidence / diff pointer / gate decision).

---

## Summary

**Gate decision: CONDITIONAL_PASS pending Round 3 spec-critic confirmation.**

| Metric | Value |
|--------|------:|
| Round 1 findings CLOSED inline       | 20 / 21 |
| Round 1 findings DEFERRED with gating | 1 / 21 |
| Round 1 findings NOT_CLOSED          | 0 / 21 |
| NF-NNN (Round 2) CRITICAL            | 0       |
| NF-NNN (Round 2) HIGH                | 0       |
| NF-NNN (Round 2) MEDIUM              | 0       |
| NF-NNN (Round 2) LOW                 | 0       |
| **Total fresh findings**             | **0**   |

The Round 2 closure is comprehensive. All 4 CRITICAL findings are CLOSED with structural changes:
- The §4.1 Net struct now reproduces the live `freeport_redirects` field with a clear "complete post-SPEC-22 layout" framing (SC-001).
- The amendment surface is auditable: §3.8 contains 10 structured amendment entries (A1-A10) for SPEC-01, SPEC-02 (R2/R10/R11/R12), SPEC-03, SPEC-04, SPEC-05, SPEC-18, SPEC-19 (SC-002, SC-003, SC-004).
- Frontmatter `Depends on:` extended; `References consumed:` / `Code analyses consumed:` / `Arguments consumed:` reformatted to SPEC-20-style multi-line.

All 7 HIGH findings are CLOSED:
- R10b/R10c added for BorderGraph slot-id stability with two strategies (SC-005).
- `to_dense(id_range: Option<Range<AgentId>>)` signature closes the partition-scoping gap (SC-006).
- R9a + §3.8 A9 mandate SPEC-18 PROTOCOL_VERSION bump 2→3 (SC-007).
- R23 demoted from MUST NOT to CI-enforced design constraint (SC-008).
- R10a / R22 upgraded SHOULD → MUST under the M5 4×-threshold rule (SC-009).
- R27a + §3.8 A6 reformulate SPEC-03 debug assertions as I3'-compatible (SC-010).
- R13 / §4.4 add `freeport_redirects` to SparseNet, resolving Q1 (SC-011).

5 of 6 MEDIUM findings are CLOSED inline; 1 MEDIUM (SC-013, theory-bridge stale tag) is DEFERRED with explicit gating because the prompt explicitly identifies SC-013 as TCC-root territory and forbids touching theory-bridge.md from this Round 2. All 4 LOW findings are CLOSED inline.

No fresh findings (NF-NNN) were introduced by the revision. Specifically, the new amendments do not invoke any predecessor function or signature that does not exist in the predecessor (the Round-1 NF-001-style failure mode SPEC-20 hit). The §3.8 amendments target real R-numbers in real predecessor specs verified at Round 1.

**Recommendation:** Proceed to Stage 1 (TASK-SPLITTER) once spec-critic Round 3 confirms. Round 3 should verify:
1. Every §3.8 amendment names an existing R-number in its target spec (Round 1 already verified this by inspection; Round 3 should re-verify against the new A1-A10 set).
2. The `to_dense` signature change (SC-006) does not introduce a hidden assumption (e.g., that callers always know their `id_range` — they do, because it's a `PartitionPlan` field).
3. The R10b two-strategy structure (Strategy A `DisableUnderDelta` vs Strategy B `BorderClean`) is implementable in both modes; particularly that `is_border_protected(_id)` has a well-defined wiring under each strategy.

---

## Round 1 closure audit

Column key:
- **C** = CLOSED: revision edits demonstrably resolve the finding.
- **D-strong** = DEFERRED with strong rationale (explicit gating mechanism in-spec or in this log).
- **D-weak** = DEFERRED but rationale is handwavy.
- **NC** = NOT_CLOSED despite claimed closure in §11.

### CRITICAL (4)

| ID | Verdict | Evidence / Diff pointer |
|----|:------:|------------------------|
| SC-001 | **C** | SPEC-22 §4.1 (struct definition lines: `pub freeport_redirects: HashMap<u32, PortRef>` is now reproduced with `#[serde(skip)]` and rkyv attrs, framed as "complete post-SPEC-22 layout"). §4.6 `to_sparse` now sets `sparse.freeport_redirects = self.freeport_redirects.clone()`; `to_dense` now sets `freeport_redirects: self.freeport_redirects.clone()` in the `Net { ... }` initializer. §4.3 `remove_agent` now calls `self.freeport_redirects.remove(&(id as u32))` on the recycle path. Both surfaces flagged in Round 1 are addressed. |
| SC-002 | **C** | Frontmatter `Amends:` extended from 2 items to 11 items (SPEC-01 I3, SPEC-02 R2/R10/R11/R12, SPEC-03 §4.3, SPEC-04 §4.5, SPEC-05 §4.2, SPEC-18 PROTOCOL_VERSION, SPEC-19 §3.2). §3.8 A2 (SPEC-02 R2) and §3.8 A3 (SPEC-02 R10) carry the structured Old text / New text / Rationale triples. The "After creating `k` agents, `next_id` MUST be incremented by `k`" sentence Round 1 specifically called out is replaced with the `f = k - r` decomposition rule in A3's New text. |
| SC-003 | **C** | Frontmatter `Depends on:` extended from 3 specs (SPEC-02, SPEC-01, SPEC-03) to 7 specs (added SPEC-04, SPEC-05, SPEC-18, SPEC-19) with parenthetical justifications inline. |
| SC-004 | **C** | §3.8 Amendments to Predecessor Specs section authored (10 entries A1-A10) following the canonical SPEC-20 §3.8 / SPEC-19 §3.8 four-field schema (Target spec / R-number / Old text / New text / Rationale). The section is positioned between §3.4 Configuration and §4 Design, matching the SPEC-20 placement. |

### HIGH (7)

| ID | Verdict | Evidence / Diff pointer |
|----|:------:|------------------------|
| SC-005 | **C** | §3.1 R10b authored with two normative strategies. R10c defines protected-tombstone semantics. §3.8 A10 records the SPEC-19 contract amendment. §4.3 `remove_agent` body updated with `is_border_protected(id)` guard + protected-tombstone branch. Tests T9a (Strategy A) and T9b (Strategy B) added in §7. The threat model from Round 1 (round N+1 recycling of border-referenced ID 47 producing wrong-rule dispatch) is explicitly discussed in R10b's closing paragraph. |
| SC-006 | **C** | §4.6 `to_dense` signature changed from `to_dense(&self) -> Net` to `to_dense(&self, id_range: Option<Range<AgentId>>) -> Net`. The free-list population loop now bounds itself to `[lo..hi)` derived from `id_range`. R10a upgraded SHOULD → MUST. Test T14a validates partition-scoped behaviour with concrete IDs (`{50, 51, 75, 99, 130, 175}` × `Some(50..100)` and `Some(100..200)`). The Round 1 evidence (`for i in 0..arena_len` blindly pushing all `None` indices) is fully replaced. |
| SC-007 | **C** | §3.1 R9a authored (PROTOCOL_VERSION 2→3 mandate, v2-vs-v3 rejection clause mirroring SPEC-20 R37). §3.8 A9 amends SPEC-18 R28. Test T8a validates rejection. The migration-path note ("v1 baseline binaries are frozen and not consumed by v2/v3 code paths") addresses the persisted `.bin` file concern. |
| SC-008 | **C** | R23 reframed from "MUST NOT be used" runtime requirement to "DESIGN CONSTRAINT — enforced by CI lint". The CI lint target is precisely specified: `src/reduction/**/*.rs` MUST NOT contain `use crate::net::sparse::SparseNet;`. Ownership of the actual lint authoring is delegated to the cicd agent (SPEC-15). The unattributed "5-10x worse" performance claim Round 1 flagged is now explicitly tied to AC-006 (HVM2 flat-array rationale) and AC-001 (Haskell `Map AgentId Agent` baseline). |
| SC-009 | **C** | R10a SHOULD → MUST. R22 SHOULD → MUST under threshold (`id_range > 4 × live_agent_count`). R30 SHOULD → MUST with `PartitionError::DenseAllocationExceedsThreshold` rejection at threshold. The 4× threshold is documented as a "clean safety margin" relative to M5. §3.8 A4 amends SPEC-02 R11 to disambiguate "next available ID" vs SPEC-22 R3 (closes Round 1's PARTIAL CONTRADICTION flag in the cross-spec audit table). |
| SC-010 | **C** | §3.3 R27a authored. §3.8 A6 amends SPEC-03 §4.3 with the assertion-pattern allowlist/denylist (allowed: uniqueness check `agents[id].is_some()`, next_id upper-bound; forbidden: monotonicity claims like `new_id > old_max_id`). Test T7a validates CON-DUP under partial free-list. The audit responsibility (scan `src/reduction/` for assertion sites) is explicitly delegated to Stage 3 DEVELOPER. |
| SC-011 | **C** | R13 SparseNet field list extended with `freeport_redirects`. §4.4 struct definition adds the field with `#[serde(skip)]`. §4.5 `new()` and `with_capacity()` constructors initialize the field as `HashMap::new()`. §4.6 `to_sparse` and `to_dense` propagate the field. §8 Q1 marked RESOLVED. |

### MEDIUM (6)

| ID | Verdict | Evidence / Diff pointer |
|----|:------:|------------------------|
| SC-012 | **C** | Frontmatter restructured: single-line `References:` replaced with multi-line `References consumed:` / `Code analyses consumed:` / `Arguments consumed:` block per the SPEC-02 / SPEC-20 pattern. AC-001, AC-006, AC-009, AC-011, AC-015 all listed (Round 1 noted only AC-001/006 cited in body; the prompt explicitly required AC-009/011/015 too). ARG-002 (border bijection) and ARG-005 (delta recoverability) added. |
| SC-013 | **D-strong** | Theory-bridge stale "SPEC-22 (Job submission)" tag at `docs/theory-bridge.md` line 142 is TCC-root territory per the Round 1 review and the Round 2 prompt explicitly forbids editing theory-bridge.md from this closure pass. Deferral is gated by §11 Change Log acknowledgement. The bridge maintainer (TCC-root level) will pick this up; no SPEC-22 author action is required. This is a gating mechanism in the literal sense: the deferral is to a different territory's owner with a clean hand-off. |
| SC-014 | **C** | R21 rewritten with a precise *behavioural equality* definition (preceded by a full natural-language definition of the relation; followed by per-round-trip MUSTs). The `Net::is_behaviorally_equal` helper signature is mandated. T14 / T8 updated to use the helper. T15 left at full structural `==` because `SparseNet` representations are inherently free of trailing-slot ambiguity (HashMap, not Vec). |
| SC-015 | **C** | §3.5 R32 authored. The 10M scenario from original Q3 is preserved as the v1 acceptance case (40 MB cap-free); the 100M M5 scenario forces a `bitvec::BitVec` switch when `free_list.len() * 4 > 64 MB`. The bitmap-with-cursor representation preserves R5 LIFO via a high-water-mark mechanism (closes the Round 1 concern that bitmap and LIFO might be incompatible). §8 Q3 marked RESOLVED. |
| SC-016 | **C** | §4.4 Send + Sync paragraph mandates `static_assertions::assert_impl_all!(SparseNet: Send, Sync)` and the same for the post-SPEC-22 `Net`. The static assertion is compile-time, satisfying Round 1's "no compile-time check" objection. |
| SC-017 | **C** | §3.5 R31 authored: SPEC-22 implementations MUST be expressible in safe Rust; `unsafe` is deferred to SPEC-23. The hand-off is explicit (SPEC-23 owns the first `unsafe` boundary in `net/types.rs`). |

### LOW (4)

| ID | Verdict | Evidence / Diff pointer |
|----|:------:|------------------------|
| SC-018 | **C** | R6 SHOULD → MUST with explicit assertion location (`debug_assert!(!self.free_list.contains(&id))` at the moment of push) and optional `HashSet<AgentId>` shadow under `#[cfg(debug_assertions)]`. Both options Round 1 suggested are now in-spec. |
| SC-019 | **C** | §4.7 intra-rule order non-determinism note authored. Tests are explicitly forbidden from asserting specific recycled-ID assignments within a single rule fire; allowed test categories (count of fresh allocations, post-rule live count, post-rule `next_id`) are enumerated. T2 (pure driver test) is exempted because the call order is fixed by the test itself. |
| SC-020 | **C** | T2 annotated with the R5-LIFO coupling note and a cross-reference to §4.7 / SC-019. The annotation matches Round 1's suggested resolution verbatim in spirit. |
| SC-021 | **C** | §8 Q5 added: SPEC-23 forward-compatibility hand-off documented. The compact-key migration of `SparseNet::ports` is gated on SPEC-23 landing; SPEC-23 already declares `Amends: SPEC-22` per its frontmatter. |

### Closure-audit summary

- **20 genuinely CLOSED** by substantive edits.
- **1 DEFERRED with strong rationale**: SC-013 (theory-bridge stale tag, downstream of SPEC-22's territory; explicit gating via the §11 Change Log acknowledgement and the Round 2 prompt's territorial scoping). Round 1 itself flagged SC-013 as "TCC-root cleanup, not SPEC-22 fault"; this Round 2 honors that scoping.
- **0 NOT_CLOSED**: every CRITICAL and HIGH from Round 1 has either an inline edit or a structured amendment in §3.8.

No findings are falsely claimed closed. The §11 Change Log is substantively accurate; the per-finding row in §11 maps 1:1 to the verdicts in this log.

---

## Fresh findings (Round 2)

**None.**

Sanity audit performed:
- §3.8 amendments target real R-numbers in real predecessor specs. SPEC-01 I3, SPEC-02 R2, R10, R11, R12 verified in Round 1 (verbatim quotes preserved). SPEC-03 §4.3 is a section reference (no R-number); SPEC-22 owns the assertion-language reformulation in R27a / A6. SPEC-04 §4.5 is `build_subnet` (verified). SPEC-05 §4.2 is the `merge` function (verified). SPEC-18 R28 / `PROTOCOL_VERSION = 2` is verified at line 163 / line 536. SPEC-19 R8-R12 BorderGraph is verified via SPEC-19 line 39-40 / line 70+.
- The `Net::is_behaviorally_equal` helper is a SPEC-22-defined helper (not a predecessor function); declared in R21. Stage 3 DEVELOPER will implement it. No predecessor invocation issue.
- The `RecyclePolicy` enum and `GridConfig.recycle_under_delta` field are SPEC-22-defined. The wiring into `GridConfig` is a small SPEC-19 / SPEC-20 amendment if those specs already have `GridConfig` definitions — verified that SPEC-20 §3.8 A5 already extended `GridConfig`, so SPEC-22 R10b's addition follows the same pattern (the `GridConfig` definition lives in SPEC-05 with extensions in SPEC-19/SPEC-20/SPEC-22). No NF.
- The `Net::is_border_protected(id)` helper is SPEC-22-defined with default behavior `false` for non-distributed contexts. The wiring under distributed contexts is "implementation-defined" with explicit guidance ("an enum field `recycle_policy: RecyclePolicy` on Net is one valid wiring"). Round 1 didn't object to similar "implementation-defined with guidance" patterns elsewhere; consistent.
- `PartitionError::DenseAllocationExceedsThreshold` (R30 rejection error) is a NEW error variant on the existing SPEC-04 `PartitionError` enum. This is a small SPEC-04 enum extension — minor, not flagged as NF because it's structurally consistent with the SPEC-20 NF-006 pattern (new error variants on existing enums in predecessor specs are routine).
- `PartitionError::BorderIdSpaceExhausted` and `PartitionError::NewRangeTooSmall` are not introduced here (those were SPEC-20 amendments). SPEC-22 only adds `DenseAllocationExceedsThreshold`.

No claim in the revised SPEC-22 invokes a predecessor function or signature that does not exist. The Round-1 NF-001 failure mode (SPEC-20's `Net::union` issue) does NOT recur in this closure.

---

## Cross-spec consistency re-audit

Re-verification of every cross-spec reference in the revised SPEC-22:

| SPEC-22 ref | Target | Verdict |
|-------------|--------|---------|
| R2 / §4.3 — "as per SPEC-02 R12" | SPEC-02 R12 + SPEC-22 §3.8 A5 | OK — A5 amends R12 with the new clearing protocol. |
| R3 / §4.2 — "existing behavior per SPEC-02 R11" | SPEC-02 R11 + SPEC-22 §3.8 A4 | OK — A4 clarifies R11 to subsume the free-list pop path. |
| R7 — "DISCONNECTED" | SPEC-02 / SPEC-01 I6 | OK (unchanged from Round 1). |
| R9 / R9a — serde + SPEC-18 PROTOCOL_VERSION | SPEC-18 R28 + SPEC-22 §3.8 A9 | OK — A9 amends SPEC-18 R28 with the 2→3 bump. |
| R10 — SPEC-04 R16-R19, SPEC-01 D4 | SPEC-04 R16-R19 | OK (unchanged from Round 1). |
| R10a — SPEC-04 build_subnet | SPEC-04 §4.5 + SPEC-22 §3.8 A7 | OK — A7 records the amendment. |
| R10b/R10c — SPEC-19 BorderGraph | SPEC-19 §3.2 R8-R12 + SPEC-22 §3.8 A10 | OK — A10 records the contract amendment. |
| R12 — SPEC-05 merge | SPEC-05 §4.2 + SPEC-22 §3.8 A8 | OK — A8 specifies the merge-time free-list reconciliation algorithm. |
| R22 — SPEC-04 build_subnet | SPEC-04 §4.5 + SPEC-22 §3.8 A7 | OK — same A7 amendment covers both R10a and R22. |
| R23 — SPEC-03 reduction engine | SPEC-03 (general reference) | OK — now a CI lint constraint, not a runtime claim. |
| R24 — SPEC-01 I3 | SPEC-01 I3 + SPEC-22 §3.8 A1 | OK — A1 records the I3 → I3' amendment. |
| R25 — SPEC-01 D4 | SPEC-01 D4 | OK (unchanged from Round 1). |
| R26 — SPEC-01 T1, I1, I2 | SPEC-01 | OK (unchanged from Round 1). |
| R27a — SPEC-03 debug assertions | SPEC-03 §4.3 + SPEC-22 §3.8 A6 | OK — A6 records the assertion-language reformulation. |
| §5.2 — AC-001, AC-006 | theory-bridge.md | OK — now also declared in frontmatter (closes SC-012). |

**Summary:** 0 contradictions, 0 amendments-needed-but-not-written, 0 frontmatter omissions. All cross-spec references are now backed by structured §3.8 amendment entries.

---

## Theory-bridge audit

Every ARG/DISC/REF/AC ID cited in the revised SPEC-22 was checked against `docs/theory-bridge.md`:

| SPEC-22 citation | Where in SPEC-22 | Resolves in bridge? | Notes |
|------------------|------------------|---------------------|-------|
| REF-002 (Lafont 1997) | Frontmatter | YES | Unchanged. |
| REF-003 (HVM2) | Frontmatter | YES | Unchanged. |
| REF-014 (Kahl) | Frontmatter | YES | Unchanged. |
| AC-001 (IC.Core) | Frontmatter + §5.2 + R23 | YES (line 196) | NEW in frontmatter. |
| AC-006 (HVM2 Types + Memory) | Frontmatter + §5.2 + R23 | YES (line 203) | NEW in frontmatter. |
| AC-009 (HVM4 Term + Heap) | Frontmatter | YES (line 208) | NEW (per prompt). |
| AC-011 (HVM4 Threading + Work-Stealing) | Frontmatter | YES (line 210) | NEW (per prompt) — informs free-list ↔ static-heap-partitioning analogy. |
| AC-015 (Cross-Cutting Synthesis) | Frontmatter | YES (line 222) | NEW (per prompt) — informs CC-4 ID-space discussion. |
| ARG-002 (border bijection) | Frontmatter | YES | NEW — informs §3.8 SPEC-04/SPEC-05 amendments. |
| ARG-005 (delta recoverability) | Frontmatter | YES (open / pending) | NEW — informs SC-005 BorderGraph constraint. ARG-005 is currently OPEN per SPEC-19/SPEC-20; this is acceptable because R10b CONDITIONALLY gates G1 on ARG-005's eventual landing, mirroring the SPEC-20 R39 G1-CONDITIONAL pattern. |

**Downstream theory-bridge cleanup item:** DISC-012 v2's "Informs" line at theory-bridge.md:142 still lists "SPEC-22 (Job submission)". Per the Round 2 prompt, this is TCC-root territory and NOT SPEC-22 author scope. Acknowledged in §11 Change Log; the bridge edit is deferred to the bridge maintainer.

**Net theory-bridge audit verdict:** clean. Frontmatter now matches body usage. ARG-005 reference is honestly gated (mirrors SPEC-19/SPEC-20 precedent). DISC-012 stale tag is the bridge maintainer's responsibility, not SPEC-22's.

---

## Invariant audit (post-revision)

**T-layer (theoretical, T1-T7):** No changes from Round 1. All preserved. **No threat.**

**D-layer (distributed):**
- **D1 (FreePort bijectivity, including D1c):** Now PROTECTED. SC-001 (struct omission) closed by §4.1; SC-011 (Q1 deferred) closed by R13. `freeport_redirects` is preserved across all conversions and purged on recycle.
- **D2 (Border completeness):** Now PROTECTED. SC-005 closed by R10b/R10c protected-tombstone semantics.
- **D3 (Cross-round border discovery):** Now PROTECTED. Same R10b/R10c.
- **D4 (ID Uniqueness After Distributed Reduction):** Now PROTECTED. SC-006 closed by `to_dense(id_range)` signature change and R10a MUST upgrade.
- **D5 (Sequential merge ordering):** Independent; **no threat** (unchanged).
- **D6 (Protocol termination):** Independent; **no threat** (unchanged).

**I-layer:** All preserved per Round 1. I3 explicitly amended to I3' via §3.8 A1.

**G-layer:**
- **G1:** No longer multi-vector threatened. SC-001 closed (FreePort redirects). SC-005 closed (BorderGraph slot recycling). SC-006 closed (out-of-range free-list IDs). G1 is CONDITIONAL on ARG-005 under delta mode (consistent with the SPEC-19 / SPEC-20 conditional gating; explicit in R10b's Strategy A formulation).

**Summary:** SPEC-22 nominally amends I3 (now formal A1) and additionally amends D1c, D2, D3, D4 contracts via R10b/R10c/R9a/§4.6 changes. All amendments are now structured §3.8 entries. The "5 invariants touched without acknowledgement" Round 1 critique is resolved: each touch has a structured amendment.

---

## Untestability catalog (post-revision)

| Req | Untestability reason | Severity | Resolution status |
|-----|---------------------|----------|-------------------|
| R23 | "MUST NOT" runtime directive | RESOLVED (SC-008) | Demoted to CI lint constraint; cicd agent owns the lint authoring. |
| R28 | "Negligible overhead" — performance claim | RESOLVED | R28 is a MUST about feature gating (always-on), not about performance. The "negligible overhead" sentence is rationale, not a testable assertion. |
| R30 | `sparse_build` flag | RESOLVED (SC-009) | Now a MUST with rejection at threshold. Default `true` is testable at construction time. |
| §4.7 table | Per-rule free-list effects | RESOLVED (SC-019) | Explicit non-determinism note; tests forbidden from asserting specific intra-rule recycled IDs. |
| R6 | "SHOULD verify by assertion" | RESOLVED (SC-018) | MUST with explicit assertion location. |

---

## Specialist self-flagged zones

§8 Open Questions audit:
- **Q1** RESOLVED (was deferred; closes SC-011).
- **Q2** ACCEPT defer (refactoring concern, not implementability).
- **Q3** RESOLVED against M5 (closes SC-015 with R32 bitmap fallback).
- **Q4** ACCEPT (sound rationale + acknowledgment of v1-test-update Stage-1 work).
- **Q5** NEW — SPEC-23 forward-compatibility hand-off (closes SC-021).

No remaining "Decision deferred to implementation" tags in §8. All deferrals are either to a separate spec (SPEC-23) or to a different agent's territory (theory-bridge maintainer for SC-013).

---

## Mandatory vs Recommended (Round 2)

**MANDATORY (Round 1 list):** All 11 mandatory items CLOSED inline.

- SC-001 — CLOSED (§4.1 + §4.6 + §4.3).
- SC-002 — CLOSED (§3.8 A2 + A3).
- SC-003 — CLOSED (frontmatter `Depends on:`).
- SC-004 — CLOSED (§3.8 authored, A1-A10).
- SC-005 — CLOSED (R10b/R10c + §3.8 A10 + T9a/T9b).
- SC-006 — CLOSED (§4.6 signature change + T14a).
- SC-007 — CLOSED (R9a + §3.8 A9 + T8a).
- SC-008 — CLOSED (R23 demoted to CI lint).
- SC-009 — CLOSED (R10a/R22/R30 SHOULD → MUST + §3.8 A4).
- SC-010 — CLOSED (R27a + §3.8 A6 + T7a).
- SC-011 — CLOSED (R13 + §4.4 + §4.5 + §4.6).

**RECOMMENDED (Round 1 list):** 9 of 10 CLOSED inline; 1 DEFERRED (SC-013, TCC-root cleanup territory).

---

## Checklist

### Consistency
- [x] All terms match SPEC-00 definitions.
- [x] Type signatures compatible with predecessor specs (§4.1 reproduces full Net struct; §4.6 signature change documented).
- [x] No contradictions with predecessor requirements (§3.8 A1-A10 records every amendment with structured Old/New text).
- [x] Data flow assumptions match predecessor outputs (SC-006 closed by `to_dense(id_range)` signature change).

### Testability
- [x] Every MUST requirement has a testable criterion (R23 is now a CI lint, not a MUST).
- [x] Boundary conditions defined (T1-T18 + T7a + T8a + T9a + T9b + T14a).
- [x] Error conditions specified (R6 MUST assertion + `PartitionError::DenseAllocationExceedsThreshold`).

### Completeness
- [x] Pseudocode provided for non-trivial operations (§4.2 / §4.3 / §4.5 / §4.6).
- [x] All edge cases documented (intra-rule order non-determinism, partition-scoped to_dense, BorderGraph recycle protection).
- [x] Rust type signatures for all public types/functions.
- [x] No undefined terms or dangling references (`freeport_redirects` now defined in §4.1; `is_behaviorally_equal` mandated in R21).

### Invariant Preservation
- [x] T1-T7 maintained.
- [x] D1-D6 maintained (D1c, D2, D3, D4 all closed via SC-001/005/006/011).
- [x] I1-I7 maintained (I3 explicitly amended to I3').
- [x] G1 not violatable by any valid operation sequence (CONDITIONAL on ARG-005 under delta mode, mirroring SPEC-19/SPEC-20 precedent).

---

## Verdict

**CONDITIONAL_PASS pending Round 3 spec-critic confirmation.**

Round 2 closure is SUBSTANTIAL: 20/21 findings CLOSED inline; 1 finding (SC-013) DEFERRED to the correct territory (TCC-root) with explicit gating; 0 NOT_CLOSED; 0 fresh NF-NNN findings. The spec is implementable as-is; the residual obligation is the theory-bridge stale-tag cleanup, which is not blocking and is owned by a different agent.

Stage 1 (TASK-SPLITTER) and Stage 2 (TEST-GENERATOR) are unblocked once spec-critic Round 3 confirms.

---

**End of Round 2 closure.**
