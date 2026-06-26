# SPEC-01 -- Round 2: Defender Response

**Date:** 2026-04-05
**Defender:** Especialista em Specs
**Target:** SPEC-01-invariantes.md
**Critic review:** SPEC-01-round1-critic.md
**Spec version:** Revised v2 -> Revised v3

---

## Summary

| Response | Count |
|----------|-------|
| ACCEPTED | 10 |
| PARTIALLY ACCEPTED | 3 |
| NOT ADDRESSED | 0 |
| **Total issues** | **13** |

---

## Responses

### SC-001: T1 formal statement does not account for DISCONNECTED sentinel
**Response:** ACCEPTED
**Action taken:** Rewrote T1's formal statement with three explicit clauses:

1. **Root port exception:** Added: "If `net.root == Some(AgentPort(a.id, p_root))`, then port `p_root` of agent `a` MAY contain `DISCONNECTED` in the port array, and the bidirectional check is waived for that port." This resolves the dilemma for `assert_ports_consistent`: the assertion now explicitly checks whether a DISCONNECTED port is the root port before flagging a violation.

2. **DISCONNECTED transience:** Added: "The sentinel `DISCONNECTED` (SPEC-02, Section 4.4) is used internally during reduction rule application. A slot containing `DISCONNECTED` violates T1 if it persists after a reduction rule completes, except at the root port."

3. **Assertion update:** The `assert_ports_consistent` pseudocode in Section 4.3 now explicitly handles DISCONNECTED at the root port (continue with no error) and rejects DISCONNECTED at non-root ports ("T1 violated: port is DISCONNECTED but is not the root port").

Additionally, added invariant **I7 (Root Port Consistency)** to formalize the requirements for `net.root` itself (see SC-004 response). T1's root port exception and I7's root consistency requirement work in tandem: T1 waives the bidirectional check for the root port, and I7 ensures that waiver is only applied to valid, DISCONNECTED root ports.
**Spec sections modified:** Section 3.1 (T1 -- formal statement, justification, how to verify, when to verify), Section 4.3 (assert_ports_consistent rewritten)

### SC-002: T1 does not distinguish port-to-self from cross-port self-loop
**Response:** ACCEPTED
**Action taken:** Added two explicit clauses to T1's formal statement:

1. **Different index requirement:** "The port ref `q` MUST be at a different port array index than `agent_port(a.id, p)`. That is, `port_index(q) != agent_port_index(a.id, p)`. A port connected to itself (`ports[p] == p`) violates T1."

2. **Cross-port self-loop validity:** "Two different ports of the same agent MAY be connected to each other. For example, `ports[agent_port(a.id, 1)] == agent_port(a.id, 2)` and `ports[agent_port(a.id, 2)] == agent_port(a.id, 1)` satisfies T1 because the two ports are at different indices. This pattern occurs in Church(0) encoding (SPEC-14 R5)."

The word "other" in T1's English statement has been replaced with "at a different index," removing all ambiguity. The assertion pseudocode in Section 4.3 now includes an explicit `assert_ne!(port_index(target), port_idx, ...)` check for port-to-self connections while naturally accepting cross-port self-loops (which have different indices). The SC-013 concern (verification functions rejecting valid self-loops) is now resolved as a consequence: the assertion is structurally incapable of rejecting cross-port self-loops.

Added a note to Resolved Question 2 (Section 7): "Invariant verification functions MUST be consistent with T1's definition, including acceptance of cross-port self-loops (cf. SPEC-14 R5)."
**Spec sections modified:** Section 3.1 (T1 -- formal statement, how to verify, consequence of violation), Section 4.3 (assert_ports_consistent), Section 7 (RQ2)

### SC-003: I-layer missing invariant for ERA unused port slots
**Response:** ACCEPTED
**Action taken:** Added invariant **I6 (ERA Auxiliary Slot Cleanliness)** as a new entry in Section 3.3 (Layer I):

- **Formal statement:** "For every ERA agent `a` in the net: `ports[agent_port(a.id, 1)] == DISCONNECTED` and `ports[agent_port(a.id, 2)] == DISCONNECTED`. Furthermore, no port entry in the port array may contain `AgentPort(a.id, 1)` or `AgentPort(a.id, 2)`."
- **Justification:** T1 iterates only `0..=arity(a.symbol)` (for ERA: only port 0), leaving slots 1 and 2 outside scope. SPEC-12 R61 confirms these slots must not be validated against T1. A bug writing a valid PortRef into an ERA auxiliary slot would be invisible to T1/I1 assertions.
- **Relationship:** I6 complements T1 and I1 by covering the gap in unused slots.

The critic suggested integrating into I2 as a sub-clause; I chose a separate invariant (I6) because it covers a fundamentally different concern (unused slot cleanliness vs. reference validity for active slots). I2 checks that references in the port array point to valid agents; I6 checks that certain slots contain NO references at all.

Added `assert_era_slots_clean` assertion to Section 4.3 and added I6 to the dependency hierarchy in Section 4.1.
**Spec sections modified:** Section 3.3 (added I6), Section 4.1 (dependency hierarchy), Section 4.3 (added assert_era_slots_clean)

### SC-004: No invariant for root port consistency
**Response:** ACCEPTED
**Action taken:** Added invariant **I7 (Root Port Consistency)** as a new entry in Section 3.3 (Layer I):

- **Formal statement:** Three sub-requirements: (a) `ref` MUST point to a valid port (I2-compatible), (b) if `ref == AgentPort(id, p)`, then `ports[agent_port(id, p)] == DISCONNECTED` (root port not also internally connected), (c) if `ref == FreePort(bid)`, the FreePort is a valid Lafont interface port (SPEC-12 R56).

This addresses all three failure modes identified by the critic:
1. Dangling root reference -> caught by I7(a): agent id must exist.
2. Invalid port index on root -> caught by I7(a): `p <= arity(agent.symbol)`.
3. Root port also internally connected -> caught by I7(b): port must contain DISCONNECTED.

Added `assert_root_consistent` assertion to Section 4.3 and added I7 to the dependency hierarchy in Section 4.1.
**Spec sections modified:** Section 3.3 (added I7), Section 4.1 (dependency hierarchy), Section 4.3 (added assert_root_consistent)

### SC-005: SPEC-10 introduces Register/RegisterAck messages without security invariants
**Response:** ACCEPTED
**Action taken:** Added a bullet to Section 4.4 (Failure Model): "**Worker authentication:** When token authentication is enabled (SPEC-10), workers MUST present a valid token before participating in the grid loop. Failed authentication does not affect the invariants (the worker is rejected and never receives a partition). The invariants T1-G1 apply to the grid loop proper, after successful authentication. SPEC-10's security tiers (Development, Private Network, Production) do not alter the trust model for invariant purposes: once a worker is authenticated, it is treated as cooperative."

This acknowledges SPEC-10's authentication phase without introducing formal security invariants, which would be outside SPEC-01's scope.
**Spec sections modified:** Section 4.4 (Failure Model)

### SC-006: "When to verify" clauses do not mention encoding or I/O entry points
**Response:** ACCEPTED
**Action taken:** Updated T1's "When to verify" clause to: "After each reduction step (in debug mode), after each merge, after each split, after each net construction by encoding functions (SPEC-14), after parsing from text DSL (SPEC-12), and after workload generation (SPEC-12)."

The I-layer invariants (I1-I5) inherit these verification points implicitly because they implement T-layer invariants. The verification frequency is governed by Resolved Question 2 (configurable and disableable). The successor specs (SPEC-12 R11/R34, SPEC-14 R8) already mandate verification at their respective entry points; this change makes SPEC-01 consistent by listing all verification points in one place.
**Spec sections modified:** Section 3.1 (T1 -- "When to verify")

### SC-007: G1 formal statement does not mention the `compute` CLI workflow
**Response:** ACCEPTED
**Action taken:** Updated G1's "How to verify" to add: "For arithmetic nets (SPEC-14), correctness can also be verified by comparing decoded results: `decode_nat(reduce_all(build_op(a, b))) == decode_nat(extract_result(run_grid(build_op(a, b), n)))` for all valid operands. This is simpler than graph isomorphism and directly validates the end-to-end encoding/reduction/decoding workflow."

G1's formal statement remains generic (isomorphism-based), which is correct. The decode_nat comparison is a practical convenience for arithmetic nets that follows from G1 (isomorphic nets decode to the same number).
**Spec sections modified:** Section 3.4 (G1 -- "How to verify")

### SC-008: I3 does not account for static ID space partitioning ranges
**Response:** ACCEPTED
**Action taken:** Added a **Scope** paragraph to I3: "In the context of a single Net (whether the global net or a partition), `next_id` MUST be strictly greater than any `AgentId` in that Net's agent arena. During distributed execution, each partition satisfies I3 independently with respect to its own agent set and its own ID range (SPEC-04 R16-R19). After merge, I3 is restored globally by taking the maximum `next_id` across all partitions (SPEC-05 R8)."

Updated "How to verify" to clarify: "During distributed execution, the assertion applies per-partition."
**Spec sections modified:** Section 3.3 (I3 -- scope, how to verify)

### SC-009: SPEC-12 introduces a 12th module not in SPEC-13, with invariant implications
**Response:** ACCEPTED
**Action taken:** Added a paragraph to Section 4.3 (before the code blocks) explaining the two invariant failure modes:

1. Runtime assertions via `debug_assert!`, which panic on failure (reduction engine, partition, merge, encoding functions).
2. Validation returning `Result::Err`, which propagates errors gracefully (text DSL parser via SPEC-12 R11, `FileIoError::InvariantViolation`).

Both modes verify the same invariants; they differ only in the failure mechanism. This makes SPEC-01 explicit about the dual verification pattern that successor specs use.
**Spec sections modified:** Section 4.3 (added introductory paragraph on two verification modes)

### SC-010: SPEC-13 error types include InvariantViolation(String) but invariants have no error codes
**Response:** PARTIALLY ACCEPTED
**Action taken:** Added a SHOULD-level recommendation to Section 4.3: "Invariant error codes: `InvariantViolation` strings SHOULD start with the invariant code (e.g., `'T1: port 42 is dangling'`, `'I3: next_id 5 <= max agent id 7'`). This enables automated log analysis and direct cross-reference to SPEC-01."

This is a SHOULD rather than a MUST because the invariant code prefix is a debugging convenience, not a correctness requirement. The assertion pseudocode in Section 4.3 already uses invariant codes in its error messages (e.g., `"I1 violated: ..."`, `"T1 violated: ..."`), setting the pattern for implementers.
**Spec sections modified:** Section 4.3 (added error code recommendation)

### SC-011: Section 5.2 "Net size only decreases" rejected alternative is incomplete
**Response:** ACCEPTED
**Action taken:** Expanded the "Net size only decreases" rejection in Section 5.2 to mention the practical consequence of arithmetic nets: "In fact, arithmetic nets (SPEC-14) demonstrate dramatic growth before collapse: `build_mul(10, 10)` generates a net with ~21 agents that grows to hundreds during reduction before collapsing to the normal form (~21 agents). T7 guarantees that the total number of remaining interactions decreases, even as the agent count temporarily increases."
**Spec sections modified:** Section 5.2 (Alternatives Considered)

### SC-012: Section 6.3 "What Relativist Changes" is stale
**Response:** ACCEPTED
**Action taken:** Added items 6-10 to Section 6.3:

6. Token authentication (SPEC-10)
7. Structured observability (SPEC-11)
8. Arithmetic encoding (SPEC-14)
9. Text DSL (SPEC-12)
10. Multi-format I/O (SPEC-12)
**Spec sections modified:** Section 6.3 (What Relativist Changes)

### SC-013: Verification frequency resolved question conflicts with SPEC-14 self-loop requirement
**Response:** PARTIALLY ACCEPTED
**Action taken:** This issue is fully resolved as a consequence of SC-002 (T1 clarification). The root cause -- ambiguous definition of "other port" in T1 -- has been eliminated. T1 now explicitly states that cross-port self-loops are valid and that only port-to-self connections are rejected. The `assert_ports_consistent` pseudocode structurally cannot reject cross-port self-loops.

As an additional safeguard, added a note to Resolved Question 2: "Invariant verification functions MUST be consistent with T1's definition, including acceptance of cross-port self-loops (cf. SPEC-14 R5). Writing a T1 assertion that rejects cross-port self-loops would cause encoding functions to fail in debug mode while passing in release mode."

No separate fix needed beyond SC-002; marking PARTIALLY ACCEPTED because the note in RQ2 is a minor addition beyond what SC-002 already provides.
**Spec sections modified:** Section 7 (RQ2 -- added self-loop consistency note)

---

## Changes Made to SPEC-01

### Header
- Status changed from "Revised v2" to "Revised v3"

### Section 3.1 (Theoretical Invariants -- Layer T)
- **T1 (Port Linearity):** Complete rewrite of the formal statement with four explicit sub-clauses:
  - "Different index" requirement replacing ambiguous "other port" wording
  - Cross-port self-loop validity clause with SPEC-14 R5 reference
  - Root port exception clause for DISCONNECTED at the root
  - DISCONNECTED transience clause referencing SPEC-02 Section 4.4
- T1 justification: Added references to SPEC-02 Section 4.9, SPEC-14 R9 (root port), SPEC-14 R5 (self-loops), SPEC-12 R58 (port-to-self rejection)
- T1 "How to verify": Updated to mention root port skipping, cross-port self-loop acceptance, port-to-self rejection
- T1 "Consequence of violation": Added port-to-self consequence
- T1 "When to verify": Expanded with encoding functions (SPEC-14), text DSL parser (SPEC-12), and workload generation (SPEC-12)

### Section 3.3 (Implementation Invariants -- Layer I)
- **I3 (Monotonicity):** Added "Scope" paragraph clarifying per-partition vs. global semantics during distributed execution, referencing SPEC-04 R16-R19 and SPEC-05 R8. Updated "How to verify" for per-partition context.
- **I6 (ERA Auxiliary Slot Cleanliness):** New invariant. Formal statement, justification, relationship (complements T1/I1), how to verify, consequence of violation.
- **I7 (Root Port Consistency):** New invariant. Three sub-requirements: (a) valid port reference, (b) DISCONNECTED at root port, (c) FreePort validity. Formal statement, justification referencing SPEC-02 R6, SPEC-14 R9, SPEC-12 R56.

### Section 3.4 (Fundamental Property -- Layer G)
- **G1:** Added `decode_nat` comparison as alternative verification method for arithmetic nets (SPEC-14)

### Section 4.1 (Invariant Dependency Hierarchy)
- Added I6 and I7 to the dependency graph:
  - `I6 (ERA Auxiliary Slot Cleanliness) --> T1 (complements linearity for unused slots)`
  - `I7 (Root Port Consistency) ----------> T1 (formalizes root port exception)`

### Section 4.3 (Debug Assertions in Rust)
- Added introductory paragraph explaining two invariant verification modes (panic via debug_assert vs. Result::Err via parser validation)
- Added SHOULD-level recommendation for invariant error codes in InvariantViolation strings
- **assert_ports_consistent:** Complete rewrite. Now iterates per-agent (over `0..total_ports(agent.symbol)`), handles DISCONNECTED at root port, checks port-to-self, verifies bidirectionality for non-root ports. Cross-port self-loops pass naturally.
- **assert_refs_valid:** Rewritten to iterate per-agent instead of per-port-array-index, consistent with other assertions.
- **assert_era_slots_clean:** New assertion function for I6.
- **assert_root_consistent:** New assertion function for I7.

### Section 4.4 (Failure Model)
- Added "Worker authentication" bullet acknowledging SPEC-10's authentication phase and clarifying that invariants T1-G1 apply after successful authentication

### Section 5.2 (Alternatives Considered)
- Expanded "Net size only decreases" rejection with SPEC-14 arithmetic net example (build_mul growth/collapse)

### Section 6.3 (What Relativist Changes)
- Added items 6-10: token authentication (SPEC-10), structured observability (SPEC-11), arithmetic encoding (SPEC-14), text DSL (SPEC-12), multi-format I/O (SPEC-12)

### Section 7 (Resolved Questions)
- RQ2: Updated to include I6 and I7 in the configurable invariant list. Added note about self-loop consistency requirement for verification functions.

---

## Residual Risks

None. All 13 issues have been addressed (10 ACCEPTED, 3 PARTIALLY ACCEPTED, 0 NOT ADDRESSED). The PARTIALLY ACCEPTED issues (SC-010, SC-013) received lighter-weight fixes than proposed (SHOULD instead of MUST for error codes; RQ2 note instead of standalone requirement for self-loop consistency) but the underlying concerns are fully resolved.

### Cross-spec consistency notes

1. **T1's root port exception and I7 cross-reference SPEC-02 Section 4.9 and SPEC-14 R9.** These successor specs already document the root port behavior; SPEC-01 now formalizes the invariant that they rely on. No changes to SPEC-02 or SPEC-14 are needed.

2. **I6 cross-references SPEC-12 R61.** SPEC-12 R61 already states that ERA auxiliary slots must not be validated against T1. I6 formalizes the complementary invariant: those slots must contain DISCONNECTED. No changes to SPEC-12 are needed.

3. **The `assert_ports_consistent` pseudocode in Section 4.3 now differs from SPEC-02 Section 4.6's `assert_adjacency_consistent`.** SPEC-02's version iterates per-agent and skips DISCONNECTED (treating it as "allowed transiently"). SPEC-01's version is stricter: DISCONNECTED is only allowed at the root port. The implementer SHOULD use SPEC-01's version as authoritative; SPEC-02's version may need a minor update for consistency.

4. **Invariant count is now T1-T7, D1-D6, I1-I7, G1.** SPEC-08 (test strategy) references "I1-I5" in several places. A future consistency pass should update SPEC-08 to reference "I1-I7."
