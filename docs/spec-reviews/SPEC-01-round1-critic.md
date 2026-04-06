# SPEC-01 -- Round 1: Spec Critic Review

**Date:** 2026-04-05
**Reviewer:** Spec Critic (adversarial)
**Target:** SPEC-01-invariantes.md (status: Revised v2)
**Successors consulted:** SPEC-02, SPEC-03, SPEC-04, SPEC-05, SPEC-06, SPEC-07, SPEC-08, SPEC-09, SPEC-10, SPEC-11, SPEC-12, SPEC-13, SPEC-14

---

## Overall Assessment

SPEC-01 is a well-structured, thorough invariant catalog that correctly maps the formal argument framework (P1-P6) to concrete implementation properties. However, since its last revision, specs 10 through 14 (all now at Revised v2) have introduced new concepts, types, and implicit invariants that SPEC-01 does not account for. The most significant gaps are: (1) T1's formal statement does not handle the `DISCONNECTED` sentinel, `root` port, or self-loop distinction, leading to ambiguity in implementation; (2) the `encoding` module (SPEC-14) introduces a new Core Layer module with its own invariant requirements that should be referenced in SPEC-01; (3) SPEC-12's text DSL parser introduces a new entry point for net construction that must enforce invariants, yet is not mentioned in the "When to verify" clauses; (4) several security and observability concepts from SPEC-10/SPEC-11 introduce operational invariants that, while not theoretical, should at least be acknowledged as out-of-scope. The invariant set T1-T7/D1-D6/I1-I5/G1 is still correct and sufficient for the formal argument, but implementation-level gaps could cause bugs if not addressed.

**Verdict:** NEEDS REVISION

---

## Issues

### SC-001: T1 formal statement does not account for DISCONNECTED sentinel
**Severity:** CRITICAL
**Axis:** Internal Consistency | Completeness
**Section:** 3.1 (T1)
**Requirement:** T1
**Problem:** T1's formal statement says: "For every agent `a` in the net and every port index `p` in `0..=arity(a.symbol)`, there exists exactly one `PortRef q` such that `ports[agent_port(a.id, p)] == q` and `ports[port_index(q)] == agent_port(a.id, p)`."

SPEC-02 (Section 4.4) defines a `DISCONNECTED` sentinel (`PortRef::FreePort(u32::MAX)`) that is used transiently during reduction. SPEC-02 also states: "A slot containing DISCONNECTED violates invariant T1 if it persists after a reduction rule completes." However, T1's formal statement has no clause for `DISCONNECTED`. The bidirectional property `ports[port_index(q)] == agent_port(a.id, p)` is undefined when `q == DISCONNECTED` because `port_index(FreePort(u32::MAX))` has no meaningful slot.

Furthermore, SPEC-14 R5/R9 note that the root agent's principal port stores `DISCONNECTED` in the port array (the external reference is provided by `net.root`). This means that for the root agent, port 0 contains `DISCONNECTED` as a permanent state, NOT a transient one. T1 as currently stated would classify this as a violation.

**Impact if unresolved:** The implementer writing `assert_ports_consistent` will face a dilemma: should the assertion accept `DISCONNECTED` at the root agent's principal port? If it rejects it, all nets with a root port (including Church numerals from SPEC-14) will fail debug assertions. If it accepts it without qualification, the assertion loses power to detect real disconnected ports.

**Suggested resolution:** Add a clause to T1's formal statement: "Exception: if `net.root == Some(AgentPort(a.id, 0))`, the principal port of agent `a` MAY contain `DISCONNECTED` in the port array, and the bidirectional check is waived for that port. All other ports of live agents MUST satisfy the bidirectional property." Also add a note that `DISCONNECTED` is a transient state during rule application and MUST NOT persist after a rule completes (except at the root port).

---

### SC-002: T1 does not distinguish port-to-self from cross-port self-loop
**Severity:** HIGH
**Axis:** Completeness | Internal Consistency
**Section:** 3.1 (T1)
**Requirement:** T1
**Problem:** T1 states that each port must be connected to "exactly one other port." The word "other" is ambiguous. SPEC-14 R5 explicitly documents that `CON_1.p1 <-> CON_1.p2` (a cross-port self-loop, where two different ports of the same agent are connected to each other) is valid and satisfies T1 and I1. SPEC-14 even warns: "Implementers MUST NOT add assertions that reject self-loops." SPEC-12 R58, conversely, says: "Self-loops (port-to-self) violate T1's 'exactly one other port' requirement" and mandates rejection of `wire a.left a.left`.

The distinction is:
- **Port-to-self** (`a.p1 -> a.p1`): port connected to itself. `ports[p] == p`. This violates T1 because the port is not connected to a *different* port.
- **Cross-port self-loop** (`a.p1 -> a.p2`): port connected to a different port of the same agent. `ports[p1] == p2` and `ports[p2] == p1`. This satisfies T1 because each port is connected to a different port (just on the same agent).

T1's formal statement uses "one other port" but does not define "other." Informally, "other" means "a different port index" (not necessarily on a different agent). But without explicit definition, an implementer could misinterpret "other port" as "port on a different agent" and reject valid self-loops.

**Impact if unresolved:** An implementer could write a T1 assertion that rejects cross-port self-loops (which are valid, per SPEC-14), breaking Church(0) encoding. Or they could accept port-to-self connections (which are invalid), allowing corrupt nets.

**Suggested resolution:** Clarify T1's statement: "Each port MUST be connected to exactly one port at a different index. That is, `ports[p] == q` where `p != q`. Two different ports of the same agent MAY be connected to each other (cross-port self-loop); a port connected to itself (`ports[p] == p`) violates T1."

---

### SC-003: I-layer missing invariant for ERA unused port slots
**Severity:** HIGH
**Axis:** Completeness
**Section:** 3.3 (Layer I)
**Requirement:** (missing)
**Problem:** The port array allocates 3 slots per agent (`id * 3 + port_id`). ERA agents have arity 0, meaning slots at indices `id*3+1` and `id*3+2` are unused. SPEC-12 R61 explicitly states: "Port array slots at indices `id*3+1` and `id*3+2` for ERA agents are unused and MUST NOT be validated against T1." SPEC-02 Section 4.4 initializes all new slots to `DISCONNECTED`.

This creates an implicit invariant: **ERA agents' auxiliary port slots MUST contain `DISCONNECTED` and MUST NOT be used by any port reference.** If an implementation bug wrote a valid `PortRef` into an ERA's auxiliary slot, the T1 assertion (if it correctly skips ERA auxiliary ports per SPEC-12 R61) would not detect it, yet the port array would be semantically corrupt.

This is not covered by T1 (which only iterates `0..=arity(a.symbol)`), I1 (which checks bidirectionality for existing entries but does not check unused slots), or I2 (which checks that references point to existing agents but does not check that non-referenced slots are clean).

**Impact if unresolved:** A bug that writes a valid `PortRef` into an ERA's auxiliary slot could create phantom connections that are invisible to all invariant assertions. During merge or split, these phantom connections could corrupt the net.

**Suggested resolution:** Add invariant **I6 (ERA Auxiliary Slot Cleanliness):** "For every ERA agent `a` in the net, `ports[agent_port(a.id, 1)] == DISCONNECTED` and `ports[agent_port(a.id, 2)] == DISCONNECTED`. In debug mode, this MUST be verified alongside I1." Alternatively, integrate this into the existing I2 as a sub-clause.

---

### SC-004: No invariant for root port consistency
**Severity:** HIGH
**Axis:** Completeness
**Section:** 3.3 (Layer I)
**Requirement:** (missing)
**Problem:** SPEC-02 R6 defines `root: Option<PortRef>` as part of the Net struct. SPEC-14 R9 specifies how `root` must be set for Church numerals. SPEC-12 R56 requires the parser to validate that root references a valid port. However, SPEC-01 has no invariant governing `net.root`.

Consider these failure modes:
1. `net.root = Some(AgentPort(dead_id, 0))` where `dead_id` is a removed agent -- dangling root reference.
2. `net.root = Some(AgentPort(id, 3))` where agent `id` has arity 2 -- invalid port index on root.
3. `net.root = Some(AgentPort(id, 0))` but the port array at `id*3+0` is NOT `DISCONNECTED` -- the root port is also internally connected, meaning the port is effectively connected to two things (the external observer and an internal agent), violating the spirit of T1.

These are implementation-level concerns that belong in Layer I.

**Impact if unresolved:** A net with a dangling root reference could panic when `decode_nat` follows `net.root`, or produce garbage results. A net with a root port that is also internally connected would have ambiguous semantics.

**Suggested resolution:** Add invariant **I7 (Root Port Consistency):** "If `net.root == Some(ref)`, then: (a) `ref` MUST point to a valid port (I2-compatible: if `AgentPort(id, p)`, then agent `id` exists and `p <= arity(agent.symbol)`), (b) if `ref == AgentPort(id, 0)`, then `ports[agent_port(id, 0)] == DISCONNECTED` (the root port is NOT also internally connected)."

---

### SC-005: SPEC-10 introduces Register/RegisterAck messages without security invariants in SPEC-01
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 4.4 (Failure Model)
**Requirement:** (missing)
**Problem:** SPEC-10 introduces token-based authentication, three security tiers, and new message types (Register, RegisterAck, RegisterNack). SPEC-10 R15 mandates constant-time token comparison to prevent timing attacks. SPEC-10 R34 mandates that token values never appear in logs after initial display. These are operational security invariants.

SPEC-01's Failure Model (Section 4.4) assumes "cooperative workers" and "reliable TCP." SPEC-10's three-tier model (Development, Private Network, Production) now means the system can operate under assumptions weaker than "cooperative workers" (a worker must prove authorization). This is a change in the trust model that SPEC-01's failure model does not reflect.

Additionally, SPEC-10 R19 extends the `Message` enum from SPEC-06 with three new variants. SPEC-01 D3 (border redex resolution) and D6 (protocol termination) assume the protocol terminates correctly; the new authentication handshake adds a phase before the grid loop that could fail (rejected token, timeout), and SPEC-01 does not acknowledge this.

**Impact if unresolved:** The failure model in SPEC-01 Section 4.4 is incomplete. An implementer reading only SPEC-01 would believe all workers are inherently trusted and that the grid loop is the first phase of communication, when in reality there is now a registration/authentication phase that can fail.

**Suggested resolution:** Update Section 4.4 to add a bullet: "**Worker authentication:** When token authentication is enabled (SPEC-10), workers MUST present a valid token before participating in the grid loop. Failed authentication does not affect the invariants (the worker is rejected and never receives a partition). The invariants T1-G1 apply to the grid loop proper, after successful authentication." This acknowledges SPEC-10 without adding formal invariants for security.

---

### SC-006: "When to verify" clauses do not mention encoding or I/O entry points
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 3.1 (T1), 3.3 (I1-I5)
**Requirement:** T1, I1, I2, I3
**Problem:** T1's "When to verify" says: "After each reduction step (in debug mode), after each merge, after each split." The I-layer invariants have similar clauses. However, since SPEC-01 was written, two new entry points for net construction have been added:

1. **SPEC-14 `encode_nat` / `encode_church_into` / `build_add` / `build_mul` / `build_exp`:** These functions construct nets from scratch. SPEC-14 R8 says: "The function MUST validate the output net in debug mode." But SPEC-01 does not list encoding as a verification point.

2. **SPEC-12 `parse_ic` (text DSL parser):** This function constructs nets from text input. SPEC-12 R11 says: "The parser MUST validate the parsed net against invariants T1 (port linearity) and I2 (reference validity)." But SPEC-01 does not list parsing as a verification point.

3. **SPEC-12 generators (`ep_annihilation`, `con_dup_expansion`, `dual_tree`, etc.):** SPEC-12 R34 says: "Each generator MUST produce a valid Net satisfying invariants T1 through T7." Not mentioned in SPEC-01.

**Impact if unresolved:** The "When to verify" clauses in SPEC-01 are incomplete documentation. No correctness impact (the successor specs already mandate verification), but an implementer reading SPEC-01 as the authoritative invariant document would miss these verification points.

**Suggested resolution:** Update T1's "When to verify" to: "After each reduction step (in debug mode), after each merge, after each split, after each net construction by encoding functions (SPEC-14), after parsing from text DSL (SPEC-12), and after workload generation (SPEC-12)."

---

### SC-007: G1 formal statement does not mention the `compute` CLI workflow
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 3.4 (G1)
**Requirement:** G1
**Problem:** SPEC-14 introduces the `compute` subcommand, which combines encoding, reduction (possibly distributed via `--workers`), and decoding into a single workflow. The fundamental property G1 (`reduce_all(net) ~ extract_result(run_grid(net, n))`) must hold for arithmetic nets just as for any other net. SPEC-14 R18 mandates:

```rust
let mut net = build_op(a, b);
reduce_all(&mut net);
assert_eq!(decode_nat(&net), Some(expected_result));
```

And the distributed version (SPEC-14 ET-11) tests:
```
build_add(a, b) -> run_grid -> decode_nat == a + b
```

These are specific instances of G1 applied to arithmetic nets. G1 as stated is generic enough to cover them. However, SPEC-01 Section 3.4 says "This is Relativist's fundamental test" and describes verification as: "construct the net, reduce sequentially, reduce distributedly, compare by isomorphism." The encoding/decoding workflow adds a third dimension: the decoded numeric result MUST also be correct.

This is not a violation of G1 (isomorphic nets will decode to the same number), but it is a usability gap: the "How to verify" section of G1 should mention that for encoded arithmetic nets, correctness can also be verified by `decode_nat` comparison, which is simpler than graph isomorphism.

**Impact if unresolved:** Minor. No correctness issue. But SPEC-01 G1 would benefit from acknowledging SPEC-14's contribution to the test strategy.

**Suggested resolution:** Add to G1's "How to verify": "For arithmetic nets (SPEC-14), correctness can also be verified by comparing decoded results: `decode_nat(reduce_all(build_op(a, b))) == decode_nat(extract_result(run_grid(build_op(a, b), n)))` for all valid operands."

---

### SC-008: I3 does not account for static ID space partitioning ranges
**Severity:** MEDIUM
**Axis:** Internal Consistency
**Section:** 3.3 (I3)
**Requirement:** I3
**Problem:** I3 states: "`next_id` MUST be strictly greater than any `AgentId` currently in use." This is correct for a single-net context. However, SPEC-04 R16-R19 define static ID space partitioning where each worker receives a contiguous range `[start, end)` and uses a local `next_id` within that range. After partitioning, a worker's `next_id` may be much less than the global maximum `AgentId` (because another worker has a higher range).

Consider: net has `next_id = 1000`. After partitioning into 2 workers with ID ranges `[0, 500)` and `[500, 1000)`, worker 0 has `next_id = 500` and worker 1 has `next_id = 1000`. Worker 0's `next_id` is 500, but agent IDs up to 999 exist globally. I3 as stated ("next_id MUST be strictly greater than any AgentId currently in use") would be violated in the context of the global net.

SPEC-05 R8 handles the merge case: "The `next_id` of the result net MUST be the maximum of all `next_id` values from the partitions." But I3 does not explicitly state that it applies per-partition during distributed execution.

**Impact if unresolved:** Ambiguity about whether I3 applies globally or per-partition. The assertion `assert_next_id_valid` could spuriously fail on partitions.

**Suggested resolution:** Add a scope qualifier to I3: "In the context of a single Net (whether the global net or a partition), `next_id` MUST be strictly greater than any `AgentId` in that Net's agent arena. During distributed execution, each partition satisfies I3 independently with respect to its own agent set. After merge, I3 is restored globally by taking the maximum `next_id` (SPEC-05 R8)."

---

### SC-009: SPEC-12 introduces a 12th module not in SPEC-13, with invariant implications
**Severity:** MEDIUM
**Axis:** Consistency with successors
**Section:** 4.3 (Debug Assertions)
**Requirement:** (cross-cutting)
**Problem:** SPEC-12 (v2) introduces an `io/` module (12th module) and explicitly notes that "SPEC-13 R5 MUST be updated to include the `io` module." The `io` module contains a text DSL parser that MUST enforce T1 and I2 (SPEC-12 R11), and generators that MUST enforce T1-T7 (SPEC-12 R34). However, SPEC-01's Section 4.3 ("Debug Assertions in Rust") only shows assertion functions for I1, I2, and I3. It does not show or mention assertions for I4 (redex queue validity), I5 (local termination), or any encoding/I/O assertions.

Additionally, SPEC-12 R52 introduces `FileIoError` with an `InvariantViolation` variant, and SPEC-12 R11 says the parser MUST return an error (not panic) when invariants are violated. This is a different failure mode than the `#[cfg(debug_assertions)]` approach in SPEC-01 Section 4.3, which uses panicking assertions.

**Impact if unresolved:** Two different invariant failure modes exist (panic via `debug_assert!` vs. `Result::Err` via parser validation) without SPEC-01 acknowledging both. An implementer may not realize that invariant checking must work in both modes.

**Suggested resolution:** Add a note to Section 4.3: "Invariant verification occurs in two modes: (a) runtime assertions via `debug_assert!`, which panic on failure (used in the reduction engine, partition, merge), and (b) validation returning `Result::Err`, which propagates errors gracefully (used in the text DSL parser, SPEC-12 R11; and in encoding functions that may be called with external input). Both modes verify the same invariants; they differ only in the failure mechanism."

---

### SC-010: SPEC-13 error types include `InvariantViolation(String)` but SPEC-01 invariants have no error codes
**Severity:** LOW
**Axis:** Completeness | Testability
**Section:** 3.1-3.4 (all invariants)
**Requirement:** T1-G1
**Problem:** SPEC-13 R16 defines per-module error enums with `InvariantViolation(String)` variants for `NetError`, `ReductionError`, `PartitionError`, and `MergeError`. The `String` carries a free-text description. However, SPEC-01's invariants are identified by codes (T1, T2, ..., I5, G1), and the "How to verify" sections reference these codes. There is no requirement that the `InvariantViolation` string include the invariant code.

If the `InvariantViolation("some description")` error does not include the invariant code, correlating a runtime error to its SPEC-01 definition will require manual grep through code comments.

**Impact if unresolved:** Debugging invariant violations is slightly harder. No correctness impact.

**Suggested resolution:** Recommend (SHOULD) that `InvariantViolation` strings start with the invariant code: e.g., `InvariantViolation("T1: port 42 is dangling")`, `InvariantViolation("I3: next_id 5 <= max agent id 7")`. This enables automated log analysis and direct cross-reference to SPEC-01.

---

### SC-011: Section 5.2 "Net size only decreases" rejected alternative is incomplete
**Severity:** LOW
**Axis:** Completeness
**Section:** 5.2 (Alternatives Considered)
**Requirement:** (rationale)
**Problem:** Section 5.2 rejects a "Net size only decreases" invariant because "CON-DUP commutation rule INCREASES the number of agents (balance +2)." This is correct. However, it does not mention a related property that IS always true: **the total interaction count decreases monotonically** (each step reduces the remaining interactions by exactly 1). This is T7 (invariant step count for terminating nets), but stated differently.

More importantly, SPEC-14's complexity bounds (R20) show that `build_mul(a, b)` requires O(a*b) interactions and `build_exp(a, b)` requires O(a^b) interactions. These large interaction counts create nets where the number of agents first grows dramatically (during commutation phases) before collapsing to normal form. The rejected alternative should note this practical consequence.

**Impact if unresolved:** Minor documentation gap. No correctness impact.

**Suggested resolution:** Add to the "Net size only decreases" rejection: "In fact, arithmetic nets (SPEC-14) demonstrate dramatic growth before collapse: `build_mul(10, 10)` generates a net with ~21 agents that grows to hundreds during reduction before collapsing to the normal form (~21 agents). T7 guarantees that the total number of remaining interactions decreases, even as the agent count temporarily increases."

---

### SC-012: Section 6.3 "What Relativist Changes" is stale with respect to SPEC-10/SPEC-11/SPEC-14
**Severity:** LOW
**Axis:** Completeness
**Section:** 6.3 (What Relativist Changes)
**Requirement:** (informative)
**Problem:** Section 6.3 lists 5 changes from the Haskell prototype. Since Revised v2, several new capabilities have been added that represent further changes:

6. **Token authentication (SPEC-10):** The Haskell prototype has no authentication. Relativist adds token-based worker authentication with constant-time comparison.
7. **Structured observability (SPEC-11):** The Haskell prototype has no structured logging or metrics. Relativist adds tracing-based logging, Prometheus metrics, and optional OpenTelemetry.
8. **Arithmetic encoding (SPEC-14):** The Haskell prototype has no encoding/decoding layer. Relativist adds Church numerals, arithmetic operations, and a `compute` CLI.
9. **Text DSL (SPEC-12):** The Haskell prototype reads/writes only binary `[Int]` format. Relativist adds a human-readable `.ic` text format.
10. **Multi-format I/O (SPEC-12):** Binary, text DSL, and JSON input/output.

**Impact if unresolved:** Section 6.3 is incomplete documentation. No correctness impact.

**Suggested resolution:** Add items 6-10 to Section 6.3 for completeness.

---

### SC-013: Verification frequency resolved question (7.2) conflicts with SPEC-14 self-loop requirement
**Severity:** LOW
**Axis:** Internal Consistency
**Section:** 7.2 (Resolved Questions)
**Requirement:** Resolved Question 2
**Problem:** Resolved Question 2 says: "Invariant verification (I1, I2, I3, I4) MUST be configurable at three levels: (a) every reduction, (b) every N reductions, (c) disabled." This is sensible for runtime performance.

However, SPEC-14 R8 says: "The function MUST validate the output net in debug mode." SPEC-14 R5 note says: "Implementers MUST NOT add assertions that reject self-loops." These two requirements interact: if the verification functions are configurable and the implementer initially writes a T1 assertion that rejects self-loops, the assertion will only trigger in debug mode (per RQ2 level (a)). The encoding functions will pass in release mode but fail in debug mode. This is a trap.

The root cause is that the invariant assertions must be consistent with T1's definition, which (per SC-002 above) does not clearly define "other port." If T1 is clarified per SC-002, this issue is resolved automatically.

**Impact if unresolved:** Covered by SC-002. This is the practical consequence.

**Suggested resolution:** After resolving SC-002 (clarifying T1 re: self-loops), add a note in Section 7.2: "Invariant verification functions MUST be consistent with T1's definition, including acceptance of cross-port self-loops (cf. SPEC-14 R5)."

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 1 |
| HIGH | 3 |
| MEDIUM | 5 |
| LOW | 4 |

## Mandatory (must fix before implementation)

- **SC-001:** T1 formal statement does not handle DISCONNECTED sentinel or root port exception -- clarify with explicit exception clause
- **SC-002:** T1 "other port" ambiguity re: port-to-self vs. cross-port self-loop -- define precisely what "other" means
- **SC-003:** Missing I-layer invariant for ERA unused port slots -- add I6 or integrate into I2
- **SC-004:** Missing invariant for root port consistency -- add I7

## Recommended (should fix)

- **SC-005:** Failure model does not acknowledge SPEC-10 authentication phase
- **SC-006:** "When to verify" clauses missing encoding/I/O entry points
- **SC-007:** G1 verification should mention decode_nat for arithmetic nets
- **SC-008:** I3 scope ambiguity during distributed execution (per-partition vs. global)
- **SC-009:** Two invariant failure modes (panic vs. Result::Err) not acknowledged

---

## Checklist

### Consistency with Successors
- [x] SPEC-02 references T1, I1, I2, I3 correctly (R8, R18, R19, R20)
- [x] SPEC-03 references T1, T5, I1, I2, I3 correctly (R3-R7)
- [x] SPEC-04 references D1 (C1-C3), D5 correctly (R6-R10)
- [x] SPEC-05 references D1, D3, G1 correctly (R10, R12-R17)
- [x] SPEC-06 references serialization identity (R14) -- not an invariant per se
- [x] SPEC-07 references grid loop termination (via --max-rounds for D6)
- [x] SPEC-08 references all invariants (T1-T7, D1-D6, I1-I5, G1) correctly
- [x] SPEC-09 references G1 (R4) correctly
- [x] SPEC-10 does NOT introduce new invariants requiring SPEC-01 formalization (security is operational)
- [x] SPEC-11 does NOT introduce new invariants requiring SPEC-01 formalization (observability is operational)
- [ ] **PARTIAL:** SPEC-12 introduces implicit invariants (ERA slot cleanliness R61, root validation R56, self-loop rejection R58) not formalized in SPEC-01 (SC-003, SC-004)
- [ ] **PARTIAL:** SPEC-14 identifies self-loop validity (R5) that conflicts with ambiguous T1 wording (SC-002)
- [x] SPEC-13 R3 termination condition consistent with SPEC-01 G1 (border + local redexes = 0)

### Testability
- [x] T1 (linearity): testable via `assert_ports_consistent` -- BUT needs DISCONNECTED/root exception (SC-001)
- [x] T2 (principal port interaction): testable via redex detection verification
- [x] T3 (disjointness): testable via HashSet during redex enumeration
- [x] T4 (strong confluence): testable by construction (verified via G1 empirically)
- [x] T5 (6 rules): testable via unit tests per rule
- [x] T6 (unique normal form): testable via multi-strategy reduction + isomorphism check
- [x] T7 (invariant step count): testable via multi-strategy reduction + counter comparison
- [x] D1 (split/merge identity): testable via round-trip test
- [x] D2 (local reduction equivalence): testable by comparing partition reduction with global reduction
- [x] D3 (border redex completeness): testable by post-merge verification of no border redexes
- [x] D4 (ID uniqueness): testable by post-merge duplicate check
- [x] D5 (exclusive ownership): testable by post-split union/disjointness check
- [x] D6 (protocol termination): testable by monitoring redex count across rounds
- [x] I1 (bidirectional port array): testable via `assert_ports_consistent`
- [x] I2 (reference validity): testable via `assert_refs_valid`
- [x] I3 (monotonic IDs): testable via `assert_next_id_valid` -- BUT scope needs clarification (SC-008)
- [x] I4 (redex queue validity): testable via stale redex detection
- [x] I5 (local termination): testable for known terminating/non-terminating nets
- [x] G1 (fundamental property): testable via sequential vs. distributed comparison

### Completeness
- [ ] **FAIL:** No invariant for DISCONNECTED sentinel handling (SC-001)
- [ ] **FAIL:** No invariant for ERA auxiliary slot cleanliness (SC-003)
- [ ] **FAIL:** No invariant for root port consistency (SC-004)
- [ ] **PARTIAL:** "When to verify" missing SPEC-12/SPEC-14 entry points (SC-006)
- [ ] **PARTIAL:** Section 6.3 stale re: SPEC-10/11/12/14 changes (SC-012)
- [x] All formal argument premises (P1-P6) mapped to invariants (Section 4.2)
- [x] All partitioning conditions (C1-C3) mapped to D1 sub-requirements
- [x] Dependency hierarchy complete and correct (Section 4.1)
- [x] Failure model documented (Section 4.4) -- BUT needs SPEC-10 update (SC-005)
- [x] Alternatives considered section well-argued (Section 5.2)

### Internal Consistency
- [ ] **FAIL:** T1 "other port" ambiguity (SC-002)
- [ ] **PARTIAL:** I3 scope ambiguity per-partition vs. global (SC-008)
- [x] T4 correctly identified as P1
- [x] D1-D6 correctly map to P2-P5
- [x] I1-I5 correctly map to T/D layer invariants
- [x] G1 correctly depends on T4 + D1-D6
- [x] Section 4.2 mapping table is complete and accurate
- [x] No contradictions between invariants themselves
