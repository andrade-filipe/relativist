# SPEC-03 -- Round 1: Spec Critic Review

**Date:** 2026-04-05
**Reviewer:** Spec Critic (adversarial)
**Target:** SPEC-03-reduction.md (status: Revised v2)
**Predecessors consulted:** SPEC-00, SPEC-01, SPEC-02
**Successors consulted:** SPEC-04, SPEC-05, SPEC-06, SPEC-07, SPEC-08, SPEC-09, SPEC-10, SPEC-11, SPEC-12, SPEC-13, SPEC-14

---

## Overall Assessment

SPEC-03 is one of the strongest specs in the Relativist project. The 6 interaction rules are specified with precise topology diagrams, pseudocode, agent balance tables, and invariant cross-references. The dispatch table, pair normalization, reduction loop, and incremental redex detection are all well-motivated and thoroughly documented. The mapping to the Haskell prototype is comprehensive. However, the spec has notable gaps in edge case coverage -- particularly around self-linking scenarios (annihilation where auxiliary ports point back to the same pair), behavior when `link` targets are `FreePort` (Boundary) sentinels, the under-specification of `remove_agent` port disconnection interaction with `get_target` ordering, and a discrepancy in the `is_reduced` function between SPEC-02 and SPEC-03's actual termination semantics.

**Verdict:** NEEDS MINOR REVISION

---

## Issues

### SC-001: Annihilation self-linking edge case not addressed
**Severity:** HIGH
**Axis:** Completeness | Testability
**Section:** 4.1.1, 4.1.2
**Requirement:** R2, R3, R7
**Problem:** Consider the annihilation rules (CON-CON, DUP-DUP) applied to an active pair where the auxiliary ports of `a` are connected to auxiliary ports of `b`. For example, in a CON-CON pair where `a.1` is connected to `b.2` and `a.2` is connected to `b.1` (the auxiliary ports form a closed structure):

```
  a.1 ------- b.2
  CON(a) ---[p0><p0]--- CON(b)
  a.2 ------- b.1
```

After reading neighbors:
```
a1_target = AgentPort(b.id, 2)
a2_target = AgentPort(b.id, 1)
b1_target = AgentPort(a.id, 2)
b2_target = AgentPort(a.id, 1)
```

After `remove_agent(a.id)` and `remove_agent(b.id)`, all these PortRefs now point to removed agents. Then:
- `link(a1_target, b2_target)` = `link(AgentPort(b.id, 2), AgentPort(a.id, 1))` -- both agents are removed.
- `link(a2_target, b1_target)` = `link(AgentPort(b.id, 1), AgentPort(a.id, 2))` -- both agents are removed.

The `connect` function (SPEC-02, Section 4.5.4) will call `set_port` on `AgentPort(b.id, 2)`, which writes to `ports[b.id * 3 + 2]`. But agent `b` has been removed. The port array slot still exists (it is not freed), but:
1. Invariant I2 (reference validity) is violated: a port array entry references a non-existent agent.
2. `connect` checks if both ports are `AgentPort(_, 0)` for redex detection -- neither is port 0, so no false redex is created. But the bidirectional entries now point to deleted agents.

The same scenario applies to DUP-DUP (parallel pattern) when `a.1 <-> b.1` and `a.2 <-> b.2`. After removal, `link(a1_target=AgentPort(b,1), b1_target=AgentPort(a,1))` writes to slots of removed agents.

The spec's "read neighbors, remove, reconnect" pattern (Section 4, convention) implicitly assumes that `a1_target`, `a2_target`, etc., are always ports of LIVE agents other than `a` and `b`. When they point back to `a` or `b`, the reconnection creates ghost entries in the port array.

For this specific case, the correct behavior is: the net should have nothing left (both agents and all their wires disappear cleanly). The `link` calls should be no-ops or should be guarded.

**Impact if unresolved:** An implementer following the pseudocode literally will write port array entries for removed agents. In debug mode, the invariant checker (`assert_all_invariants`) should catch this as an I2 violation -- but the spec does not document this edge case or the expected behavior. The implementer may add ad-hoc guards that inadvertently break other cases.
**Suggested resolution:** Add a subsection "Edge Case: Self-Referencing Auxiliary Ports" after each annihilation rule (or consolidated in Section 4.8). Specify that: (a) if `a1_target` is `AgentPort(a.id, _)` or `AgentPort(b.id, _)`, the port belongs to a removed agent and the `link` is a no-op (the wire is already fully consumed), or (b) the `link`/`connect` function should guard against writing to removed agent slots. Provide a concrete test scenario. SPEC-08 should add a test case `RE_SELF_AUX` for this.

---

### SC-002: Behavior of `link`/`connect` when target is `FreePort(bid)` is undefined
**Severity:** HIGH
**Axis:** Completeness | Consistency
**Section:** 4.1.4, 4.1.5, 4.1.6, 4.7
**Requirement:** R10, R11
**Problem:** SPEC-03 states that `link` is implemented by `Net::connect` (Section 4.7). SPEC-02 Section 4.5.4 shows that `connect` calls `set_port(a, b)` and `set_port(b, a)`. SPEC-02 Section 4.5.6 shows that `set_port` ignores `FreePort` targets: "Only operates on AgentPort; FreePort is ignored."

Now consider a reduction rule in a partitioned sub-net. After partitioning (SPEC-04), some ports of agents near the boundary are connected to `FreePort(bid)` sentinels. When a rule is applied:

1. `get_target(AgentPort(a.id, 1))` could return `FreePort(bid)` if `a.1` is connected to a boundary sentinel.
2. The rule saves `a1_target = FreePort(bid)`.
3. After removing agents, the rule calls `link(a1_target, ...)` = `connect(FreePort(bid), some_port)`.
4. `set_port(FreePort(bid), some_port)` is a no-op (FreePort is ignored).
5. `set_port(some_port, FreePort(bid))` writes `FreePort(bid)` to the port array of `some_port`.

This means the connection is half-written: `some_port -> FreePort(bid)` but `FreePort(bid) -> ???` (not stored anywhere in the port array). Invariant I1 (bidirectional consistency) is violated.

SPEC-03 does not acknowledge or handle this case. The spec says "all ports are `AgentPort` or `FreePort`" (Section 4.7) but does not specify what happens when `FreePort` appears in the reduction rule's neighbor list.

SPEC-05 (Section 4.3) handles this with the `free_port_index` (a `HashMap<u32, PortRef>` per partition), and SPEC-04 mentions that partitions maintain this mapping. But SPEC-03's reduction rules do not mention updating the `free_port_index` when a FreePort endpoint is reconnected during local reduction in a partition.

**Impact if unresolved:** When a worker reduces a net containing boundary FreePorts, the reduction engine will silently create half-connections. The `free_port_index` will become stale (it still maps `bid -> old_agent_port`). After merge, the coordinator will use the stale `free_port_index` to reconnect boundaries, producing incorrect wires.
**Suggested resolution:** SPEC-03 should add a subsection "Reduction with FreePort (Boundary) Sentinels" specifying one of:
- (a) The `free_port_index` MUST be updated by `connect` when one endpoint is a `FreePort(bid)`: `free_port_index[bid] = other_port`. This requires `connect` to have access to the `free_port_index`.
- (b) The `free_port_index` is lazily reconstructed after local reduction by scanning the port array for entries containing `FreePort(bid)`. SPEC-04 R30 / SPEC-05 already mentions this ("rebuild_free_port_index"), but SPEC-03 should note that the reduction engine does NOT need to maintain the `free_port_index` because it is reconstructed post-reduction.
Option (b) is simpler and is consistent with the existing SPEC-05 Section 4.3 (`rebuild_free_port_index`). But SPEC-03 should explicitly state this. Add a note: "During local reduction within a partition, auxiliary ports may be connected to FreePort (Boundary) sentinels. The reduction rules treat FreePort targets identically to AgentPort targets in the `get_target` phase (reading the PortRef). During the `link` phase, `connect` will write the FreePort to the port array of the AgentPort side, but cannot write back to the FreePort (which has no port array slot). This one-sided write is acceptable because the `free_port_index` is reconstructed post-reduction (SPEC-05, Section 4.3). Invariant I1 is temporarily violated for FreePort connections during reduction; it is restored after `rebuild_free_port_index`."

---

### SC-003: `interact_anni` reads symbol from agent arena after validity check, but spec does not guarantee agent still exists at that point
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 4.5 (`interact_anni`)
**Requirement:** R12
**Problem:** In the `interact_anni` pseudocode (Section 4.5):
```rust
let sym = net.agents[a_id as usize].unwrap().symbol;
```
This assumes `agents[a_id]` is `Some(...)`. The `reduce_step` function (Section 4.6.1) performs validity checking via `is_valid_redex(a_id, b_id)` before calling the interact function, so at that point both agents are guaranteed to exist. However, the spec does not explicitly state as a precondition that `interact_*` functions are only called on live agents. The `unwrap()` will panic if called on a removed agent.

This is a minor issue because the calling code (`reduce_step`) guarantees validity. But the precondition should be formalized.

**Impact if unresolved:** An implementer who calls `interact_anni` directly (e.g., in unit tests) without verifying that agents exist will get a panic. Minor.
**Suggested resolution:** Add explicit preconditions to each `interact_*` function docstring: "Precondition: both agent IDs MUST refer to live agents (`agents[id].is_some()`). This precondition is guaranteed by `reduce_step`'s validity check (R12). Calling an interact function with removed agents is undefined behavior."

---

### SC-004: `reduce_step` does not increment the interaction counter
**Severity:** MEDIUM
**Axis:** Consistency
**Section:** 4.6.1
**Requirement:** R12
**Problem:** R12 states: "reduce_step MUST: ... (5) increment the interaction counter." The term "interaction counter" implies a field on `Net` (consistent with SPEC-00 Section 4.2, "Interaction Counter: A u64 counter..."). However, the `reduce_step` pseudocode (Section 4.6.1) does NOT increment any counter. Instead, it returns a `StepResult::Reduced(Rule)`, and the caller (`reduce_all`, `reduce_n`) is responsible for counting.

The spec defines `ReductionStats` as a local struct in `reduce_all`/`reduce_n`, not as a field of `Net`. The "interaction counter" from SPEC-00 Section 3 (as a `u64` counter on the Net) is not present in SPEC-02's `Net` struct definition, and is not incremented by `reduce_step`.

This is not necessarily wrong -- counting in the caller is a valid design. But R12 says `reduce_step` MUST increment the interaction counter, which contradicts the pseudocode.

SPEC-01 T7 says "the total number of reduction steps to reach Normal Form is invariant." If the counter is caller-managed (in `ReductionStats`), this invariant is verifiable. But SPEC-00 defines "Interaction Counter" as "A u64 counter that records the number of interaction rules applied. Incremented on each successful reduce_step." and SPEC-03 Section 2 says "Invariant T7 (SPEC-01) guarantees that the final value is identical for any reduction strategy." These definitions imply a persistent counter on Net, not a local variable.

**Impact if unresolved:** The implementer must decide between: (a) adding an `interaction_count: u64` field to `Net` and incrementing it in `reduce_step`, or (b) keeping the counter in `ReductionStats` and trusting callers. The spec text (R12) and the pseudocode disagree.
**Suggested resolution:** Either (a) add `interaction_count: u64` to `Net` (SPEC-02) and increment it in `reduce_step`, or (b) amend R12 to say "reduce_step MUST return the applied rule so that callers can maintain interaction counts." and note that the interaction counter is managed externally via `ReductionStats`. Option (b) matches the pseudocode and avoids adding state to `Net`. Update the SPEC-00 definition accordingly.

---

### SC-005: `is_reduced` checks only queue emptiness, but stale entries can make it return false when the net is actually in Normal Form
**Severity:** MEDIUM
**Axis:** Consistency | Testability
**Section:** 4.6.1, 4.6.2
**Requirement:** R13, R16 (SPEC-02)
**Problem:** SPEC-02 R16 defines `is_reduced` as returning `true` iff the redex queue is empty. SPEC-02 Section 4.5.7 documents this and notes: "the redex queue may contain stale entries." It suggests "drain_stale() + is_reduced()" for rigorous verification.

SPEC-03's `reduce_all` (Section 4.6.2) loops until `reduce_step` returns `StepResult::NormalForm`, which happens when the queue is empty. This correctly drains stale entries through the loop. But SPEC-05 R27 says: "The termination condition of the grid loop MUST be: the net is in Normal Form (redex queue empty after reduce_all)." SPEC-05 Section 4.5 calls `net.is_reduced()` after `reduce_all` to confirm.

The issue: after `reduce_all` returns, the queue IS empty (all entries consumed), so `is_reduced()` will return `true`. This is correct. But if someone calls `is_reduced()` without first calling `reduce_all` (e.g., immediately after merge, before border redex resolution), the result is unreliable because of stale entries.

SPEC-03 does not explicitly state that `is_reduced()` should NOT be used as a standalone termination check without first draining stale entries. SPEC-05 relies on `is_reduced()` after `reduce_all`, which works. But the gap is in documentation, not correctness.

**Impact if unresolved:** An implementer might use `is_reduced()` as a standalone check (e.g., in merge phase before calling `reduce_all`), getting a false negative (stale entries present, but no real redexes). Minor.
**Suggested resolution:** Add a note in Section 4.6 or Section 4.8: "The function `is_reduced()` (SPEC-02 R16) is a necessary but not sufficient condition for Normal Form when stale entries exist in the queue. The canonical way to verify Normal Form is to call `reduce_all` (which drains all stale entries) and then check that no new entries were generated. Do NOT use `is_reduced()` as a standalone termination check without first processing stale entries."

---

### SC-006: No specification of behavior when `create_agent` causes Vec reallocation during `interact_comm`
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 4.1.4, 4.5 (`interact_comm`)
**Requirement:** R20
**Problem:** The `interact_comm` function creates 4 new agents via `net.create_agent(Symbol::Dup/Con)`. SPEC-02 Section 4.5.2 notes that `create_agent` may trigger Vec reallocation ("`resize`" calls on `agents` and `ports`). If reallocation occurs, previously saved `PortRef::AgentPort(id, port)` values (like `a1`, `a2`, `b1`, `b2`) are still valid because they store agent IDs, not pointers -- the Vec reallocation changes the backing memory but the indexing logic remains correct.

However, SPEC-03 does not explicitly state this safety property. An implementer unfamiliar with Rust's ownership model might worry that reallocation invalidates the saved PortRef values.

Additionally, R20 says each rule executes in O(1), but `create_agent` is O(1) AMORTIZED due to Vec growth. This means `interact_comm` is O(1) amortized, not O(1) worst-case. The spec's complexity table (Section 4.10) correctly notes "O(1)" for the rule but does not add the "amortized" qualifier.

**Impact if unresolved:** Minor. The O(1) vs O(1)-amortized distinction does not affect correctness, only benchmarking of individual operations.
**Suggested resolution:** (a) Add a note in Section 4.1.4 or Section 4.9: "PortRef values are index-based, not pointer-based. Vec reallocation during `create_agent` does not invalidate previously read PortRef values." (b) Qualify R20 as "O(1) amortized" to be consistent with the `create_agent` amortized complexity from SPEC-02.

---

### SC-007: `reduce_step` interaction counter increment says (5) but pseudocode has no step (5)
**Severity:** LOW
**Axis:** Consistency
**Section:** 3.4 (R12), 4.6.1
**Requirement:** R12
**Problem:** R12 describes 5 steps for `reduce_step`. Step (5) is "increment the interaction counter." The pseudocode in Section 4.6.1 has 6 numbered comments: (1) dequeue, (2) verify validity, (3) normalize, (4) determine rule, (5) apply rule, (6) verify invariants. There is no "increment the interaction counter" step. The interaction counter is managed by the caller (`reduce_all`, `reduce_n`).

This is related to SC-004 but specifically about the numbering mismatch between R12's description and the pseudocode.

**Impact if unresolved:** Confusion about which step does what. Minor.
**Suggested resolution:** Amend R12 to remove step (5) or change it to "(5) return the applied rule for caller-managed counting."

---

### SC-008: `interact_anni` uses a single function with internal branch, but dispatch table maps to `Rule::Anni` for both CON-CON and DUP-DUP without distinguishing cross vs parallel
**Severity:** LOW
**Axis:** Testability
**Section:** 4.3, 4.5
**Requirement:** R8, R17
**Problem:** The dispatch table maps both CON-CON and DUP-DUP to `Rule::Anni`. The `ReductionStats` struct counts `anni_count` as a single counter. R17 says the engine SHOULD discriminate by rule type (annihilation, commutation, erasure, void). Since both CON-CON and DUP-DUP map to `Anni`, the stats cannot distinguish between cross-annihilation and parallel-annihilation.

For benchmarking and profiling (SPEC-09), knowing the mix of CON-CON vs DUP-DUP may be valuable (e.g., a net dominated by CON-CON has different characteristics from one dominated by DUP-DUP). SPEC-09 Section 4.5 lists "interactions_by_rule" as a metric and references SPEC-03 R16 (note: R16 is `is_reduced` in SPEC-02; the cross-reference appears to be R17).

**Impact if unresolved:** Cannot separately profile CON-CON vs DUP-DUP annihilations. This limits the detail of workload characterization in benchmarks.
**Suggested resolution:** Either (a) split `Rule::Anni` into `Rule::AnniCon` and `Rule::AnniDup` in the dispatch table, and add separate counters to `ReductionStats`, or (b) add a note that the SHOULD requirement R17 is satisfied at the category level (annihilation/commutation/erasure/void) and sub-category discrimination (CON-CON vs DUP-DUP, CON-ERA vs DUP-ERA) is deferred. Option (b) is simpler for v1.

---

### SC-009: SPEC-09 cross-references SPEC-03 R16 for per-rule counter, but R16 is about `is_reduced` in SPEC-02
**Severity:** LOW
**Axis:** Consistency
**Section:** (cross-spec)
**Requirement:** R17
**Problem:** SPEC-09 Section 4.5 says: "Interactions by rule | interactions_by_rule | count per type | SPEC-03, R16 (per-rule counter)". But SPEC-03 R16 says: "In release mode, assertions MAY be disabled for performance." The per-rule counter is R17, not R16. Additionally, SPEC-02 R16 defines `is_reduced`, adding to the cross-reference confusion.

**Impact if unresolved:** An implementer following the SPEC-09 cross-reference to SPEC-03 R16 will land on the assertion config requirement, not the counter requirement.
**Suggested resolution:** Fix the cross-reference in SPEC-09 to point to SPEC-03 R17.

---

### SC-010: No explicit handling of `reduce_all` called on a net with only stale redexes in the queue
**Severity:** LOW
**Axis:** Completeness | Testability
**Section:** 4.6.2
**Requirement:** R13
**Problem:** Consider a net where the redex queue has 5 entries, all stale (agents already consumed). `reduce_all` calls `reduce_step` 5 times; each call discards a stale entry and tries the next. After all 5 are discarded, `pop_front` returns `None`, and `reduce_step` returns `NormalForm`. `reduce_all` then returns with `total_interactions: 0`.

This is correct behavior, but SPEC-08 has no explicit test for this case. RE13 tests `reduce_all` on an empty net (0 entries in queue), and RE12 tests a single stale redex in `reduce_step`. But neither tests `reduce_all` on a net with ONLY stale redexes (non-empty queue, zero valid redexes).

**Impact if unresolved:** A subtle bug where `reduce_step` fails to properly loop through stale entries (e.g., an off-by-one in the discard loop) would not be caught by existing tests.
**Suggested resolution:** Add to SPEC-08 a test `RE_ALL_STALE`: "reduce_all on a net with 3 stale redexes and no valid redexes: returns with 0 interactions and the net is unchanged." This is implicitly a consequence of RE12 being applied in a loop, but an explicit test strengthens confidence.

---

### SC-011: SPEC-03 Section 2 defines "Stale Redex" as "Defined in SPEC-02" but SPEC-02 defines it differently
**Severity:** LOW
**Axis:** Consistency
**Section:** 2 (Definitions)
**Requirement:** ---
**Problem:** SPEC-03 Section 2 says: "Stale Redex: An entry in the redex queue whose agents have already been consumed by another reduction, or whose principal port connection has changed. Defined in SPEC-02." SPEC-02 Section 2 says: "Stale Redex: An entry in the redex queue whose agents have already been consumed by another reduction, or whose principal port connection has changed." Both definitions are identical. The phrase "Defined in SPEC-02" in SPEC-03 is a cross-reference, but the definition is then re-stated verbatim. This is not harmful but is redundant -- the convention in other specs is to say "Terms defined in SPEC-00 (Glossary) are used without redefinition."

**Impact if unresolved:** Stylistic inconsistency. No correctness issue.
**Suggested resolution:** Remove the re-definition from SPEC-03 Section 2 and replace with: "Stale Redex: See SPEC-02, Section 2."

---

### SC-012: T2 (Interaction Exclusively via Principal Ports) preservation not explicitly listed for each rule
**Severity:** LOW
**Axis:** Invariant Preservation
**Section:** 4.1.1 through 4.1.6
**Requirement:** R7
**Problem:** Each rule section lists invariants preserved (e.g., T1, I1, T3 for annihilation; T1, I1, I3, T3 for commutation). None of the 6 rules lists T2 (Interaction Exclusively via Principal Ports) as preserved. T2 is about the correctness of redex detection, not about the topology of the rule result. It is preserved by construction: the incremental redex detection in `connect` only inserts pairs where both ports are port 0. But for completeness of the invariant cross-reference, T2 should be listed.

Similarly, T5 (Correctness of the 6 Interaction Rules) is not listed as preserved -- it IS the rule itself, not something preserved by it. T6 and T7 are consequences of T4 and are not listed. These are fine to omit.

But T2 is directly relevant: after a rule applies, the redex queue may have new entries. Those entries must satisfy T2. This is guaranteed by `connect`, but should be noted.

**Impact if unresolved:** Incomplete invariant cross-reference. An auditor checking which invariants are preserved by each rule will not see T2 listed.
**Suggested resolution:** Add "T2 (principal port interaction): New redexes inserted by `connect` satisfy T2 by construction (only `AgentPort(_, 0)` pairs are inserted)." to the invariants-preserved section of each rule (or once in a consolidated note in Section 4.8).

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 0 |
| HIGH | 2 |
| MEDIUM | 4 |
| LOW | 6 |

## Mandatory (must fix before implementation)

- **SC-001:** Self-linking edge case in annihilation rules -- specify behavior when auxiliary ports of an active pair point to each other
- **SC-002:** FreePort (Boundary) targets during reduction -- specify that `free_port_index` is reconstructed post-reduction, and that I1 is temporarily violated for FreePort connections

## Recommended (should fix)

- **SC-003:** Add explicit preconditions to `interact_*` functions
- **SC-004:** Reconcile R12 step (5) "increment counter" with the actual pseudocode (counter is in caller)
- **SC-005:** Document that `is_reduced()` should not be used as standalone termination check
- **SC-006:** Qualify R20 as O(1) amortized and add note about Vec reallocation safety

## Optional (nice to have)

- **SC-007:** Fix R12 step numbering
- **SC-008:** Consider splitting `Rule::Anni` for finer-grained profiling
- **SC-009:** Fix SPEC-09 cross-reference from R16 to R17
- **SC-010:** Add SPEC-08 test for `reduce_all` on net with only stale redexes
- **SC-011:** Remove redundant Stale Redex definition
- **SC-012:** Add T2 to invariant preservation lists

---

## Checklist

### Consistency
- [x] Types match predecessor specs (Symbol, AgentId, PortRef, Net, Agent, Rule)
- [x] PortRef encoding consistent with SPEC-00/SPEC-02
- [x] The 6 rules match SPEC-01 T5 table (agent balance, reconnection topology)
- [x] SPEC-00 glossary definitions for Annihilation/Commutation/Erasure/Void match SPEC-03 topology
- [x] `connect` behavior consistent with SPEC-02 R13 (bidirectional write + redex detection)
- [x] `remove_agent` behavior consistent with SPEC-02 R12 (mark None, disconnect ports)
- [x] `get_target` behavior consistent with SPEC-02 R15 (returns target PortRef)
- [x] Stale redex handling consistent with SPEC-01 I4 and SPEC-02 R17
- [x] Successor specs reference SPEC-03 correctly (SPEC-04, SPEC-05, SPEC-06, SPEC-08, SPEC-09, SPEC-12, SPEC-13, SPEC-14)
- [ ] **PARTIAL:** R12 "increment counter" step contradicts pseudocode (SC-004)
- [ ] **FAIL:** SPEC-09 cross-references SPEC-03 R16 but means R17 (SC-009)

### Testability
- [x] Each of the 6 rules is testable via topology verification (SPEC-08 RE1-RE6)
- [x] Dispatch symmetry is testable (SPEC-08 RE7-RE10)
- [x] Reduction loop is testable (SPEC-08 RE11-RE17)
- [x] Incremental redex detection is testable (SPEC-08 RE18-RE20)
- [x] Per-rule counters testable (SPEC-08 RE21)
- [ ] **FAIL:** Self-linking annihilation edge case has no test (SC-001)
- [ ] **FAIL:** Reduction with FreePort boundary targets has no explicit test in SPEC-08 (SC-002)
- [ ] **PARTIAL:** `reduce_all` on net with only stale redexes not explicitly tested (SC-010)

### Completeness
- [x] All 6 rules specified with topology diagrams, pseudocode, and metrics
- [x] Dispatch table covers all 9 (Symbol, Symbol) pairs
- [x] Pair normalization fully specified
- [x] `reduce_step`, `reduce_all`, `reduce_n` fully specified with pseudocode
- [x] Incremental redex detection mechanism fully specified
- [x] Complexity analysis per rule and per loop
- [x] Rationale section covers all major design decisions (5 decisions)
- [x] Haskell prototype mapping is comprehensive (Section 6)
- [x] Open questions section declares "None" with justification
- [ ] **FAIL:** Self-linking edge case not addressed (SC-001)
- [ ] **FAIL:** FreePort targets during reduction not addressed (SC-002)
- [ ] **PARTIAL:** Vec reallocation safety not stated (SC-006)
- [ ] **PARTIAL:** `is_reduced` vs stale entries caveat not documented in SPEC-03 (SC-005)

### Invariant Preservation
- [x] T1 (linearity): explicitly verified for each rule
- [ ] **PARTIAL:** T2 (principal port interaction): not listed in any rule's invariants-preserved section (SC-012)
- [x] T3 (disjointness): explicitly verified for annihilation and commutation
- [x] T4 (strong confluence): R23 explicitly preserves; no ordering constraint
- [x] T5 (rule correctness): each rule specified per Lafont Fig. 2
- [x] T6/T7 (uniqueness/invariant count): preserved by construction via T4
- [x] I1 (bidirectional port array): explicitly verified via `connect` for each rule
- [x] I2 (reference validity): explicitly mentioned for relevant rules
- [x] I3 (monotonicity): explicitly verified for rules that create agents (comm, eras)
- [x] I4 (redex queue validity): R19 and reduce_step stale check
- [x] I5 (local termination): reduce_all terminates for terminating nets; reduce_n respects budget
- [x] D2 (local reduction equivalence): not SPEC-03's responsibility directly, but SPEC-03's rule correctness is a precondition
- [x] D4 (ID uniqueness): create_agent uses monotonic IDs; distributed ID ranges noted in Section 4.9
- [ ] **PARTIAL:** I1 is temporarily violated during reduction in partitioned sub-nets with FreePort targets (SC-002)
