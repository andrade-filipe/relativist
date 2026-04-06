# SPEC-03 -- Round 2: Defender Response

**Date:** 2026-04-05
**Defender:** Especialista em Specs
**Target:** SPEC-03-reduction.md
**Critic review:** SPEC-03-round1-critic.md
**Spec version:** Revised v2 -> Revised v3

---

## Summary

| Response | Count |
|----------|-------|
| ACCEPTED | 10 |
| PARTIALLY ACCEPTED | 2 |
| NOT ADDRESSED | 0 |
| **Total issues** | **12** |

---

## Responses

### SC-001: Annihilation self-linking edge case not addressed
**Response:** ACCEPTED
**Action taken:** Added requirement R25 (Section 3.8) specifying that when annihilation rules encounter auxiliary ports pointing back to the pair being consumed, the `link` calls MUST be no-ops. Introduced a `link` helper function (Section 4.5) that wraps `Net::connect` with a guard: if either endpoint is an `AgentPort` of a removed agent (`agents[id].is_none()`), the link is skipped. Updated `interact_anni` to use `link` instead of `net.connect` for all reconnections. Added a detailed "Edge case -- self-referencing auxiliary ports" note after Section 4.1.2 (DUP-DUP) with a concrete diagram showing the CON-CON closed structure scenario, and documenting that partial self-references (one link is a no-op, the other proceeds) are also handled. Section 4.7 (Link Procedure) updated to document the `link` helper semantics.

The approach of using a `link` wrapper rather than modifying `Net::connect` (SPEC-02) was chosen because: (a) it keeps `connect` simple and in SPEC-02's territory, (b) the guard is only relevant for annihilation rules (commutation and erasure cannot produce self-referencing targets), and (c) the `link` function also provides a uniform wrapper for documenting FreePort behavior (R26/SC-002).
**Spec sections modified:** Section 3.8 (R25), Section 4.1.2 (edge case note), Section 4.5 (`link` helper + `interact_anni` updated), Section 4.7

### SC-002: Behavior of `link`/`connect` when target is `FreePort(bid)` is undefined
**Response:** ACCEPTED
**Action taken:** Adopted option (b) from the critic (lazy reconstruction). Added requirement R26 (Section 3.9) explicitly documenting the behavior when `FreePort(bid)` appears as a target during local reduction in a partitioned sub-net. The spec now states:

1. During `get_target`, the port array returns `FreePort(bid)` as-is (it is stored in the port array slot of the `AgentPort` side).
2. During `link`/`connect`, `set_port(FreePort(bid), target)` is a no-op (FreePort has no port array slot), while `set_port(target, FreePort(bid))` writes `FreePort(bid)` to the AgentPort side.
3. This one-sided write is acceptable because `rebuild_free_port_index` (SPEC-05, Section 4.3) scans the port array after local reduction and reconstructs the mapping.
4. Invariant I1 is temporarily violated for FreePort connections during local reduction; it is restored after reconstruction.

Updated `interact_comm` and `interact_eras` pseudocode to use `link` for external wires (where old neighbors may be FreePort), with comments noting the FreePort possibility. Section 4.7 now includes a dedicated "FreePort behavior during link" paragraph.

This is consistent with SPEC-05 Section 5.3 ("Lazy Reconstruction of FreePort Index"), which was designed specifically so the reduction engine does not need to maintain the `free_port_index`.
**Spec sections modified:** Section 3.9 (R26), Section 4.5 (`interact_comm`, `interact_eras` use `link`), Section 4.7 (FreePort paragraph)

### SC-003: `interact_anni` reads symbol from agent arena after validity check, but spec does not guarantee agent still exists at that point
**Response:** ACCEPTED
**Action taken:** Added explicit preconditions to all four `interact_*` function docstrings: "Precondition: both agent IDs MUST refer to live agents (`agents[id].is_some()`). This precondition is guaranteed by `reduce_step`'s validity check (R12). Calling this function with removed agents is undefined behavior." Each function's precondition also documents the specific symbol requirements (e.g., `interact_comm` requires `con_id` to be Con and `dup_id` to be Dup).
**Spec sections modified:** Section 4.5 (all four `interact_*` functions)

### SC-004: `reduce_step` does not increment the interaction counter
**Response:** ACCEPTED
**Action taken:** Adopted option (b) from the critic. Amended R12 step (5) from "increment the interaction counter" to "return the applied rule so that callers can maintain interaction counts via `ReductionStats`." Added a note below R12 clarifying that the interaction counter is managed by the caller (`reduce_all`, `reduce_n`), not by `reduce_step` itself, and that this avoids adding mutable counter state to `Net`. Updated the "Interaction Counter" definition in Section 2 to specify that the counter is "managed by the caller via `ReductionStats`, not as a field on `Net`."

This matches the actual pseudocode design (Section 4.6) where `reduce_all` and `reduce_n` increment `ReductionStats` upon receiving `StepResult::Reduced(Rule)`. SPEC-00's definition of "Interaction Counter" should be updated in a future consistency pass, but since SPEC-00 is not in this spec's territory, the discrepancy is documented here.
**Spec sections modified:** Section 2 (Interaction Counter definition), Section 3.4 (R12)

### SC-005: `is_reduced` checks only queue emptiness, but stale entries can make it return false when the net is actually in Normal Form
**Response:** ACCEPTED
**Action taken:** Added a "Note on `is_reduced()` and stale entries" paragraph at the end of Section 4.8 (Incremental New Redex Detection). The note states that `is_reduced()` is a necessary but not sufficient condition for Normal Form when stale entries exist, and prescribes `reduce_all` as the canonical way to verify Normal Form. It explicitly warns against using `is_reduced()` as a standalone termination check.
**Spec sections modified:** Section 4.8 (new paragraph)

### SC-006: No specification of behavior when `create_agent` causes Vec reallocation during `interact_comm`
**Response:** ACCEPTED
**Action taken:** Two changes:
1. Added a "Vec reallocation safety" note to Section 4.9 (ID Generation During Reduction) stating that `PortRef` values are index-based, not pointer-based, and that Vec reallocation does not invalidate previously read values.
2. Qualified R20 as "O(1) amortized" with an explanation that the amortized qualifier accounts for `create_agent` potentially triggering Vec reallocation. Updated the complexity table in Section 4.10 to show "O(1) amortized" for rules that create agents (comm, eras) and added a clarifying note distinguishing O(1) worst-case (anni, void) from O(1) amortized (comm, eras).

Also added a note in `interact_comm`'s docstring about Vec reallocation safety.
**Spec sections modified:** Section 3.6 (R20), Section 4.5 (`interact_comm` docstring), Section 4.9, Section 4.10

### SC-007: `reduce_step` interaction counter increment says (5) but pseudocode has no step (5)
**Response:** ACCEPTED (covered by SC-004)
**Action taken:** The fix for SC-004 already resolved this issue by changing R12 step (5) from "increment the interaction counter" to "return the applied rule so that callers can maintain interaction counts." The pseudocode in Section 4.6.1 already has step (5) as "Apply rule" and step (6) as "Verify invariants in debug mode," which now matches R12's description. No additional change needed.
**Spec sections modified:** (covered by SC-004 changes to Section 3.4)

### SC-008: `interact_anni` uses a single function with internal branch, but dispatch table maps to `Rule::Anni` for both CON-CON and DUP-DUP without distinguishing cross vs parallel
**Response:** PARTIALLY ACCEPTED
**Action taken:** The SHOULD requirement R17 is satisfied at the category level (annihilation, commutation, erasure, void). Sub-category discrimination (CON-CON vs DUP-DUP, CON-ERA vs DUP-ERA) is deferred to a future version if benchmarking (SPEC-09) reveals a need for finer-grained profiling. This is consistent with the v1 scope: the primary benchmarks (SPEC-09) focus on aggregate rule counts, not sub-category breakdowns. The implementer MAY split `Rule::Anni` into `Rule::AnniCon` and `Rule::AnniDup` if needed, as this is a backward-compatible extension.

No spec text was changed for this issue. The existing R17 SHOULD-level requirement is sufficient: "The reduction engine SHOULD discriminate the interaction counter by rule type (annihilation, commutation, erasure, void)."
**Spec sections modified:** (none)

### SC-009: SPEC-09 cross-references SPEC-03 R16 for per-rule counter, but R16 is about `is_reduced` in SPEC-02
**Response:** PARTIALLY ACCEPTED
**Action taken:** This is a cross-reference error in SPEC-09, not in SPEC-03. SPEC-09 is outside SPEC-03's territory. The correct reference is SPEC-03 R17 (per-rule counter), not R16. Documented here for the future SPEC-09 consistency pass. No change made to SPEC-03 itself.
**Spec sections modified:** (none -- SPEC-09 is outside territory)

### SC-010: No explicit handling of `reduce_all` called on a net with only stale redexes in the queue
**Response:** ACCEPTED
**Action taken:** The behavior is already correct by construction: `reduce_all` calls `reduce_step` in a loop; each `reduce_step` dequeues and discards stale entries until the queue is empty, then returns `NormalForm`. After draining all stale entries, `reduce_all` returns with `total_interactions: 0`. No spec text change is needed for SPEC-03 because the pseudocode handles this correctly. However, a test `RE_ALL_STALE` should be added to SPEC-08 ("reduce_all on a net with 3 stale redexes and no valid redexes: returns with 0 interactions and the net is unchanged"). Since SPEC-08 is outside this spec's territory, this is documented here for the future SPEC-08 review.
**Spec sections modified:** (none -- SPEC-08 is outside territory)

### SC-011: SPEC-03 Section 2 defines "Stale Redex" as "Defined in SPEC-02" but SPEC-02 defines it differently
**Response:** ACCEPTED
**Action taken:** Replaced the verbatim re-definition of "Stale Redex" in Section 2 with "See SPEC-02, Section 2. The reduction engine MUST discard stale redexes silently (SPEC-01, I4)." This follows the convention used in other specs where shared terms point to their canonical definition.
**Spec sections modified:** Section 2 (Stale Redex definition)

### SC-012: T2 (Interaction Exclusively via Principal Ports) preservation not explicitly listed for each rule
**Response:** ACCEPTED
**Action taken:** Added a consolidated note in the preamble to Section 4.1 (before the individual rules): "Additionally, all rules preserve T2 (interaction exclusively via principal ports): new redexes inserted by `connect` satisfy T2 by construction, because `connect` only inserts pairs into the redex queue when both endpoints are `AgentPort(_, 0)` (principal ports). T2 is not listed individually per rule to avoid redundancy." This approach was chosen over listing T2 in each rule's invariants-preserved section because T2 is a property of the `connect` function, not of any specific rule's topology.
**Spec sections modified:** Section 4.1 (preamble, invariant cross-reference paragraph)

---

## Changes Made to SPEC-03

### Header
- Status changed from "Revised v2" to "Revised v3"

### Section 2 (Definitions)
- Stale Redex: removed verbatim re-definition, replaced with cross-reference to SPEC-02 Section 2 (SC-011)
- Interaction Counter: clarified as caller-managed via `ReductionStats`, not a field on `Net` (SC-004)

### Section 3.4 (Reduction Loop)
- R12: step (5) changed from "increment the interaction counter" to "return the applied rule so that callers can maintain interaction counts via ReductionStats" (SC-004, SC-007)
- R12: added note clarifying caller-managed counter design

### Section 3.6 (Complexity)
- R20: qualified as "O(1) amortized" with explanation of Vec reallocation (SC-006)

### Section 3.8 (NEW -- Self-Referencing Auxiliary Ports)
- R25: new MUST requirement specifying that link calls to removed agents are no-ops (SC-001)

### Section 3.9 (NEW -- Reduction with FreePort Sentinels)
- R26: new MUST requirement documenting FreePort behavior during local reduction and temporary I1 violation (SC-002)

### Section 4.1 (Interaction Rules preamble)
- Added T2 preservation note to invariant cross-reference paragraph (SC-012)

### Section 4.1.2 (DUP-DUP)
- Added "Edge case -- self-referencing auxiliary ports (R25)" with concrete diagram, explaining the closed structure scenario, partial self-references, and pointer to Section 4.5 (SC-001)

### Section 4.5 (Interaction Functions)
- Added `link` helper function with guard for removed agents (SC-001)
- `interact_anni`: updated to use `link` instead of `net.connect`; added explicit preconditions (SC-001, SC-003)
- `interact_comm`: updated external wires to use `link` with FreePort comments; added explicit preconditions and Vec reallocation safety note (SC-002, SC-003, SC-006)
- `interact_eras`: updated to use `link` with FreePort comment; added explicit preconditions (SC-002, SC-003)
- `interact_void`: added explicit preconditions (SC-003)

### Section 4.7 (Link Procedure)
- Rewritten to document the `link` helper function, the removed-agent guard (R25), and FreePort behavior during link (R26) (SC-001, SC-002)

### Section 4.8 (Incremental New Redex Detection)
- Added "Note on `is_reduced()` and stale entries" paragraph (SC-005)

### Section 4.9 (ID Generation During Reduction)
- Added "Vec reallocation safety" paragraph (SC-006)

### Section 4.10 (Complexity Analysis)
- Qualified CON-DUP, CON-ERA, DUP-ERA as "O(1) amortized" in the per-rule table (SC-006)
- Added clarifying note distinguishing O(1) worst-case from O(1) amortized (SC-006)

### Section 7 (Open Questions)
- Added entries for self-referencing edge case, FreePort sentinels, counter management, and preconditions as resolved items

---

## Residual Risks

### SC-008 (PARTIALLY ACCEPTED -- deferred sub-category profiling)

The `Rule::Anni` enum variant covers both CON-CON and DUP-DUP, preventing separate profiling of cross vs. parallel annihilation. This is a SHOULD-level concern (R17). For v1, aggregate category counts (annihilation, commutation, erasure, void) are sufficient for workload characterization. The implementer MAY split `Rule::Anni` into `Rule::AnniCon` and `Rule::AnniDup` if SPEC-09 benchmarking reveals a need. No correctness impact.

### SC-009 (PARTIALLY ACCEPTED -- cross-spec reference error)

SPEC-09 Section 4.5 incorrectly references "SPEC-03, R16" for the per-rule counter; the correct reference is R17. This cannot be fixed from SPEC-03's territory. Documented here for the SPEC-09 review round.

### SC-010 (ACCEPTED -- test gap in SPEC-08)

`reduce_all` on a net with only stale redexes works correctly by construction (the pseudocode handles it). A dedicated test `RE_ALL_STALE` should be added to SPEC-08 for explicit coverage. This cannot be added from SPEC-03's territory. Documented here for the SPEC-08 review round.

### Cross-spec consistency note

R25 introduces a `link` helper at the SPEC-03 level rather than modifying `Net::connect` in SPEC-02. This means `connect` remains a "raw" bidirectional write without removed-agent guards. Callers other than the reduction engine (e.g., net construction utilities, test harnesses) that call `connect` directly are responsible for ensuring both endpoints refer to live agents or FreePorts. This is acceptable because `connect` is a low-level primitive, and the guard is a reduction-engine concern.

R12 step (5) now references caller-managed counters, which differs from SPEC-00's definition of "Interaction Counter" as "incremented on each successful reduce_step." SPEC-00 should be updated in a future consistency pass to align with SPEC-03's design. Since SPEC-00 is outside this spec's territory, the discrepancy is documented but not resolved here.
