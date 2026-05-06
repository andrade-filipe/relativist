# CLOSURE-D009 — Spec Amendments A1..A10 Landing Log

**Date:** 2026-04-27
**Author:** especialista-specs
**Bundle:** D-009 Phase A — SPEC-22 Arena Management predecessor-spec amendment cluster (TASK-0460..0469)
**Branch:** v2-development
**Source of truth:** `specs/SPEC-22-arena-management.md` §3.8 (A1..A10 New text — read-only)

---

## Summary

10 amendments to 7 predecessor spec files, all verbatim from SPEC-22 §3.8. No code changes. No test changes.

| Amendment | Task | Spec file | Edit location | Status |
|-----------|------|-----------|---------------|--------|
| A1 | TASK-0460 | `specs/SPEC-01-invariantes.md` | Lines 290–319 (I3 → I3') | LANDED |
| A2 | TASK-0461 | `specs/SPEC-02-net-representation.md` | Line 37 (R2 replacement) | LANDED |
| A3 | TASK-0462 | `specs/SPEC-02-net-representation.md` | Line 58 (R10 replacement) | LANDED |
| A4 | TASK-0463 | `specs/SPEC-02-net-representation.md` | Line 62 (R11 replacement) | LANDED |
| A5 | TASK-0464 | `specs/SPEC-02-net-representation.md` | Line 64 (R12 replacement) | LANDED |
| A6 | TASK-0465 | `specs/SPEC-03-reduction.md` | §4.3.0 inserted before §4.3.1 (line 529) | LANDED |
| A7 | TASK-0466 | `specs/SPEC-04-partition.md` | §4.5.1 inserted after §4.5 complexity (line 398) | LANDED |
| A8 | TASK-0467 | `specs/SPEC-05-merge.md` | §4.2.1 inserted after §4.2 algorithm (line 433) | LANDED |
| A9 | TASK-0468 | `specs/SPEC-18-wire-format-v2.md` | R28 replacement (line 163) + constant §4.7 (line 538) | LANDED |
| A10 | TASK-0469 | `specs/SPEC-19-delta-protocol.md` | R12a inserted after R12 (line 96) | LANDED |

---

## Amendment-by-Amendment Detail

### A1 — TASK-0460: SPEC-01 I3 → I3' (Monotonicity → Uniqueness)

**File:** `specs/SPEC-01-invariantes.md`
**Status header:** Revised v3.2
**Edit:** Replaced the `**I3. Monotonicity of AgentIds**` block (lines 289–299) with `**I3' (Uniqueness of AgentIds)**` block. New text is verbatim from SPEC-22 §3.8 A1 New text. Added backward-pointer `> Amendment A1 (SPEC-22 §3.8 A1 / R24)` prefixing the invariant. Cross-reference to SPEC-22 R10b/R10c (protected tombstones) included in the I3' statement. Invariant table D-layer/I-layer cross-reference: `I3' is a precondition for D4` retained in the Relationship bullet. Revision history updated at line 11: `v3.2 (2026-04-27): I3 relaxed to I3' ...`.
**Backward-pointer:** SPEC-22 R24 cited in amendment block header.
**Acceptance criteria check:**
- [x] I3' text verbatim from SPEC-22 §3.8 A1 New text.
- [x] Invariant table D-layer/I-layer cross-reference updated (I3' → D4 relationship preserved).
- [x] Cross-reference to SPEC-22 R10b/R10c in statement.
- [x] No code change.

---

### A2 — TASK-0461: SPEC-02 R2 — AgentId reuse via free-list

**File:** `specs/SPEC-02-net-representation.md`
**Status header:** Revised v3.1
**Edit:** R2 (line 37) replaced with SPEC-22 §3.8 A2 New text verbatim. Cross-reference updated from `SPEC-01, I3` to `SPEC-01, I3'`. Backward-pointer `> Amendment A2 (SPEC-22 §3.8 A2)` appended. Clearing protocol triple `(a)/(b)` explicitly enumerated. SPEC-22 R1-R10c cited as reuse mechanism.
**Backward-pointer:** SPEC-22 §3.8 A2 + R1-R10c.
**Acceptance criteria check:**
- [x] R2 verbatim from SPEC-22 §3.8 A2 New text.
- [x] Cross-reference updated to I3'.
- [x] R2 cites SPEC-22 R1-R10c.
- [x] Clearing protocol triple (a)/(b) enumerated.

---

### A3 — TASK-0462: SPEC-02 R10 — next_id increment by f = k - r

**File:** `specs/SPEC-02-net-representation.md`
**Edit:** R10 (line 58) replaced with SPEC-22 §3.8 A3 New text verbatim. `next_id` upper-bound updated from "in use" to "ever assigned". Cross-reference updated to `SPEC-01, I3'`. Fresh-vs-recycle decomposition (`r`, `f = k - r`) defined. Backward-pointer appended.
**Backward-pointer:** SPEC-22 §3.8 A3 + R3.
**Acceptance criteria check:**
- [x] "strictly greater than any AgentId ever assigned" clause in place.
- [x] Cross-reference updated to I3'.
- [x] `r` and `f = k - r` defined.

---

### A4 — TASK-0463: SPEC-02 R11 — "next available ID" subsumes free-list pop

**File:** `specs/SPEC-02-net-representation.md`
**Edit:** R11 (line 62) replaced with SPEC-22 §3.8 A4 New text verbatim. Free-list pop path (with R10b/R10c protected-tombstone check) and next_id path both subsumed under "next available ID". Arena expansion only on fresh-allocation path stated explicitly. O(1) amortized complexity preserved. Backward-pointer appended.
**Backward-pointer:** SPEC-22 §3.8 A4.
**Acceptance criteria check:**
- [x] Both paths (free-list pop, next_id) subsumed under "next available ID".
- [x] Arena expansion only on fresh-allocation path.
- [x] O(1) amortized preserved.
- [x] R10b/R10c protected-tombstone check included.

---

### A5 — TASK-0464: SPEC-02 R12 — remove_agent pushes free-list, purges freeport_redirects

**File:** `specs/SPEC-02-net-representation.md`
**Edit:** R12 (line 64) replaced with SPEC-22 §3.8 A5 New text verbatim. `freeport_redirects` purge step added. Protected-tombstone exception (SPEC-22 R10c) threaded in. Free-list push (SPEC-22 R2/R6) stated. "NOT reuse the ID" clause lifted. Backward-pointer appended with SPEC-22 R2, R6, R10c cross-references.
**Backward-pointer:** SPEC-22 §3.8 A5 + R2 (push) + R6 (no duplicates) + R10c (protected tombstones).
**Acceptance criteria check:**
- [x] `freeport_redirects` purge step in place.
- [x] Protected-tombstone exception clause present.
- [x] Cross-references to SPEC-22 R2, R6, R10c.

---

### A6 — TASK-0465: SPEC-03 §4.3 — debug-assertion allowlist/denylist (I3'-compatible)

**File:** `specs/SPEC-03-reduction.md`
**Status header:** Revised v3.1
**Edit (primary):** New subsection `#### 4.3.0 Debug Assertion Language — I3'-Compatible Patterns (Amendment A6)` inserted immediately before §4.3.1 (line 529). Contains verbatim SPEC-22 §3.8 A6 New text: allowlist (`debug_assert!(self.agents[new_id as usize].is_some())`, `debug_assert!(self.next_id > new_id)`), denylist (`assert!(new_id > old_max_id)`, `assert!(new_id == self.next_id - 1)`), CON-DUP load-bearing case noted, `assert_next_id_valid` preserved note. SPEC-22 R27a cross-referenced.
**Edit (secondary):** R6 updated to reference I3' instead of I3. Two inline `I3 (monotonicity)` references in rule descriptions (CON-DUP at line 376, CON-ERA at line 430) updated to `I3' (uniqueness)`.
**Backward-pointer:** SPEC-22 §3.8 A6 + R27a.
**Acceptance criteria check:**
- [x] §4.3 amended with allowlist/denylist verbatim.
- [x] SPEC-22 R27a cross-referenced.
- [x] `assert_next_id_valid` preservation noted.
- [x] CON-DUP flagged as load-bearing case.

---

### A7 — TASK-0466: SPEC-04 §4.5 build_subnet — per-partition free-list + 4× sparse threshold

**File:** `specs/SPEC-04-partition.md`
**Status header:** Revised v3.1
**Edit:** New subsection `#### 4.5.1 build_subnet — Free-List Population and Sparse-Build Threshold (Amendment A7)` inserted after the §4.5 complexity statement (line 398). Contains verbatim SPEC-22 §3.8 A7 New text: `free_list` population with `None` slots in `[id_range.start, id_range.end)`, 4× threshold → SparseNet path, exposed signature MAY remain `Net`, `PartitionError::DenseAllocationExceedsThreshold` rejection when `sparse_build=false` at threshold. Cross-references SPEC-22 R10a, R22, R30.
**Backward-pointer:** SPEC-22 §3.8 A7 + R10a (per-partition free-list) + R22 (sparse threshold) + R30 (sparse_build flag).
**Acceptance criteria check:**
- [x] Per-partition free-list population requirement stated.
- [x] 4× threshold rule and sparse-build fallback stated.
- [x] SPEC-22 R10a, R22, R30 cross-referenced.
- [x] `PartitionError::DenseAllocationExceedsThreshold` rejection noted.

---

### A8 — TASK-0467: SPEC-05 §4.2 merge — free-list reconciliation across partitions

**File:** `specs/SPEC-05-merge.md`
**Status header:** Revised v3.2
**Edit:** New subsection `#### 4.2.1 merge — Free-List Reconciliation (Amendment A8)` inserted after the §4.2 algorithm's complexity statement (line 433). Contains verbatim SPEC-22 §3.8 A8 New text: walk-check-push-or-discard reconciliation algorithm, O(sum of |partition.free_list|) complexity, no-duplicates guarantee via partition disjointness (D4). Cross-references SPEC-22 R12, R6, SPEC-01 D4.
**Backward-pointer:** SPEC-22 §3.8 A8 + R12 + R6 + SPEC-01 D4.
**Acceptance criteria check:**
- [x] Reconciliation algorithm (walk → check → push-or-discard) stated.
- [x] SPEC-22 R12, R6, SPEC-01 D4 cross-referenced.
- [x] Complexity O(sum of |partition.free_list|) declared.

---

### A9 — TASK-0468: SPEC-18 R28 — PROTOCOL_VERSION 2 → 3

**File:** `specs/SPEC-18-wire-format-v2.md`
**Status header:** Updated to note R28 amended per A9.
**Edit (R28):** R28 (line 163) replaced with SPEC-22 §3.8 A9 New text verbatim: 2 → 3 bump upon SPEC-22 landing, v2 deserializer rejection via `UnsupportedVersion`, v1/v2 `.bin` file unreadability acceptable, migration path in SPEC-22 §6. Backward-pointer `> Amendment A9 (SPEC-22 §3.8 A9 / R9a)` appended.
**Edit (constant §4.7 line 538):** `PROTOCOL_VERSION: u8 = 2` → `PROTOCOL_VERSION: u8 = 3`. Added `v3:` line to the comment block.
**Edit (§6.2 line 619):** Added Step 1a documenting the v2→v3 bump procedure.
**Backward-pointer:** SPEC-22 §3.8 A9 + R9a.
**Acceptance criteria check:**
- [x] R28 explicitly mandates 2 → 3 bump.
- [x] v2 deserializer rejection via `UnsupportedVersion` path stated.
- [x] v1/v2 `.bin` file unreadability documented as acceptable.
- [x] SPEC-22 §6 migration path cross-referenced.
- [x] Constant table updated to `PROTOCOL_VERSION = 3`.

---

### A10 — TASK-0469: SPEC-19 §3.2 BorderGraph — recycle-policy-aware contract

**File:** `specs/SPEC-19-delta-protocol.md`
**Status header:** Updated to note §3.2 amended per A10.
**Edit:** New requirement `**R12a. BorderGraph — Recycle-Policy-Aware Contract (Amendment A10)**` inserted after R12 (line 96). Contains verbatim SPEC-22 §3.8 A10 New text: Strategy A (`RecyclePolicy::DisableUnderDelta`, default), Strategy B (`RecyclePolicy::BorderClean`), `GridConfig.recycle_under_delta: RecyclePolicy` field, protected-tombstone semantics (R10c). Threat model paragraph included verbatim. Cross-references SPEC-22 R10b, R10c.
**Backward-pointer:** SPEC-22 §3.8 A10 + R10b + R10c.
**Acceptance criteria check:**
- [x] Strategy A (`DisableUnderDelta`) and Strategy B (`BorderClean`) enumerated.
- [x] `RecyclePolicy::DisableUnderDelta` as conservative default.
- [x] Protected-tombstone semantics (R10c) present.
- [x] SPEC-22 R10b, R10c and `GridConfig.recycle_under_delta` field cross-referenced.

---

## Backward-Pointer Summary (all 10 amendments carry backward-pointers)

| Amendment | Backward-pointer to SPEC-22 |
|-----------|----------------------------|
| A1 | §3.8 A1 / R24 |
| A2 | §3.8 A2 / R1-R10c |
| A3 | §3.8 A3 / R3 |
| A4 | §3.8 A4 |
| A5 | §3.8 A5 / R2, R6, R10c |
| A6 | §3.8 A6 / R27a |
| A7 | §3.8 A7 / R10a, R22, R30 |
| A8 | §3.8 A8 / R12, R6, D4 |
| A9 | §3.8 A9 / R9a |
| A10 | §3.8 A10 / R10b, R10c |

---

## Invariant Table / Cross-Reference Updates

| Invariant | Location | Change |
|-----------|----------|--------|
| SPEC-01 I3 → I3' | SPEC-01 line 290 | Full replacement; D4 relationship preserved |
| SPEC-01 I3' | SPEC-02 R2, R10 | Cross-references updated from I3 to I3' |
| SPEC-01 I3' | SPEC-03 R6 | Reference updated from I3 to I3' |
| SPEC-01 D4 | SPEC-05 §4.2.1 | Cited as no-duplicate guarantor for merged free-list |
| SPEC-22 R10b/R10c | SPEC-01 I3', SPEC-02 R12, SPEC-19 R12a | Protected-tombstone cross-references in place |

---

## Verification

- All 10 amendments landed verbatim (verified by comparing grep output to SPEC-22 §3.8 New text).
- 7 spec files modified: SPEC-01, SPEC-02, SPEC-03, SPEC-04, SPEC-05, SPEC-18, SPEC-19.
- SPEC-22 itself NOT modified (canonical source, read-only).
- No files outside `specs/` or `docs/spec-reviews/` modified.
- No `src/` or `tests/` changes.
- Status headers updated on all 7 files.
- `Amends:` frontmatter added to all 7 files referencing SPEC-22 §3.8.

---

## Recommended Next Action

D-009 Phase A (TASK-0460..0469) is complete. Proceed to Phase B: Free-List Core (TASK-0471..0484) — developer agent, TDD RED→GREEN→REFACTOR. Test baseline entering Phase B: 1308 default / 1351 zero-copy (unchanged; these are spec-only edits).
