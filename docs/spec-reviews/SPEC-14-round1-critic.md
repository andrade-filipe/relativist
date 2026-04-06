# SPEC-14 -- Round 1: Spec Critic Review

**Date:** 2026-04-05
**Reviewer:** Spec Critic (adversarial)
**Target:** SPEC-14-encoding.md (status: Draft v1)
**Predecessors consulted:** SPEC-00, SPEC-01, SPEC-02, SPEC-03

---

## Overall Assessment

SPEC-14 is ambitious and covers a lot of ground -- Church numeral encoding/decoding, arithmetic combinators, CLI integration, benchmarks, and generator integration. The high-level design is sound: Church encoding is well-motivated and the encoding/decoding approach is reasonable. However, the spec contains a critical internal contradiction in how the root port is represented (R9 vs. the Section 4.1 algorithm), references multiple APIs that do not exist in predecessor specs, and leaves two of three arithmetic operations (multiplication, exponentiation) critically underspecified.

**Verdict:** MAJOR REVISION

---

## Issues

### SC-001: R9 contradicts the Section 4.1 algorithm and SPEC-02 root port design
**Severity:** CRITICAL
**Axis:** Consistency
**Section:** 3.2 (R9) vs. 4.1 (algorithm) vs. 4.4 (decode algorithm)
**Requirement:** R9
**Problem:** R9 states: "The net produced by `encode_nat` MUST have a root port set to `FreePort(0)`, representing the external interface of the Church numeral." However, in Section 4.1, the construction algorithm calls `net.set_root(PortRef::AgentPort(lam_f, 0))`, which sets the root to an `AgentPort`, not a `FreePort(0)`. Furthermore, SPEC-02 Section 4.9 defines root as `Option<PortRef>` where the canonical usage is `net.root = Some(PortRef::AgentPort(root_agent, 0))`. The decode algorithm (Section 4.4) also expects root to be an `AgentPort` (matching on `PortRef::AgentPort(id, 0)`).

The port connection tables in Section 4.2 list `CON_0.p0 = FreePort(0) [root]`, suggesting that the principal port of the outer lambda is connected TO a `FreePort(0)` -- but this is a different concept than the net's root field being `FreePort(0)`. These are two separate things that the spec conflates:
1. `net.root` = the Net struct field (should be `Some(AgentPort(lam_f, 0))` per SPEC-02)
2. `ports[lam_f * 3 + 0]` = what the principal port of lam_f is connected to (this could be `FreePort(0)` as a Lafont interface port)

But if lam_f.p0 is connected to FreePort(0), then the port array has a one-directional entry (`FreePort -> AgentPort` cannot be stored in the port array per SPEC-02 Section 4.10). This means T1 (port linearity) and I1 (bidirectionality) cannot be fully satisfied for this port. The decode algorithm would also fail because `net.get_target(root)` where root = `FreePort(0)` returns `DISCONNECTED` per SPEC-02's `get_target` implementation.

**Impact if unresolved:** An implementer cannot know whether to set root as `FreePort(0)` or `AgentPort(lam_f, 0)`. If they follow R9, the decode algorithm breaks. If they follow Section 4.1, R9 is violated.
**Suggested resolution:** Remove R9 or rewrite it to: "The net produced by `encode_nat` MUST have `net.root = Some(PortRef::AgentPort(lam_f, 0))` where `lam_f` is the outer lambda agent." Update the port connection tables to show `CON_0.p0` as the root observation point (perhaps `root -> CON_0.p0`) rather than `FreePort(0)`. If `FreePort(0)` is needed in the port array as a Lafont interface marker, explicitly document how this interacts with T1/I1 and the border map.

---

### SC-002: `get_agent` method not defined in SPEC-02
**Severity:** CRITICAL
**Axis:** Consistency
**Section:** 4.4 (decode algorithm)
**Requirement:** R11, R12
**Problem:** The decode algorithm in Section 4.4 calls `net.get_agent(lam_f)?` and `net.get_agent(era_id)?` repeatedly. This method is not defined anywhere in SPEC-02, which defines only `create_agent`, `remove_agent`, `connect`, `disconnect`, `get_target`, and `set_port`. The `?` operator on the return value implies a signature like `fn get_agent(&self, id: AgentId) -> Option<&Agent>`, but this is not specified.

An implementer reading SPEC-02 would have no guidance on this API. The return type matters: `Option<&Agent>` (reference to Agent in the arena slot) vs `Option<Agent>` (copy, since Agent is Copy) have different implications.

**Impact if unresolved:** Decode algorithm references an API that does not exist in the Net contract. Implementer must invent the API.
**Suggested resolution:** Either (a) add a `get_agent` requirement to SPEC-02 (e.g., R15b: "The `get_agent(id: AgentId) -> Option<&Agent>` operation MUST return a reference to the agent if it exists"), or (b) rewrite the decode algorithm in SPEC-14 to use only SPEC-02's existing API: `net.agents[id as usize].as_ref()`.

---

### SC-003: `get_target` return type mismatch -- `PortRef` vs `Option<PortRef>`
**Severity:** HIGH
**Axis:** Consistency
**Section:** 4.4 (decode algorithm)
**Requirement:** R12
**Problem:** SPEC-02 (Section 4.5.6, R15) defines `get_target(&self, port: PortRef) -> PortRef` (returns `PortRef`, returning `DISCONNECTED` for invalid/out-of-range ports). But the SPEC-14 decode algorithm uses `net.get_target(...)? ` with the `?` operator throughout (lines 512, 523, 524, 525, 549), which requires a return type of `Option<PortRef>`. These are fundamentally different signatures.

**Impact if unresolved:** The decode algorithm as written will not compile against SPEC-02's API.
**Suggested resolution:** Either (a) change the decode algorithm to check for `DISCONNECTED` instead of using `?`, or (b) note that `get_target` should be wrapped in a helper that converts `DISCONNECTED` to `None`. Clarify which approach the implementer should use.

---

### SC-004: `reduce_all_and_return` function undefined
**Severity:** HIGH
**Axis:** Consistency
**Section:** 7 (Test Requirements T6-T8, T10)
**Requirement:** T6, T7, T8, T10
**Problem:** Tests T6-T8 and T10 reference `reduce_all_and_return(build_add(a, b))` and similar calls. This function does not exist in SPEC-03, which defines `reduce_all(net: &mut Net) -> ReductionStats`. Since `reduce_all` takes a mutable reference and mutates in place, the net IS the result after calling it. There is no function that takes ownership and returns the net.

Meanwhile, T11 uses `reduce_all(build_add(a, b))` as if it takes ownership -- but SPEC-03's signature takes `&mut Net`.

**Impact if unresolved:** Test authors will not know what API to call. The test pseudocode is not implementable as written.
**Suggested resolution:** Replace all occurrences of `reduce_all_and_return(expr)` with a two-step pattern: `let mut net = expr; reduce_all(&mut net); decode_nat(&net)`. Alternatively, define `reduce_all_and_return` as a test helper and note it explicitly.

---

### SC-005: Test label collision with SPEC-01 invariants
**Severity:** HIGH
**Axis:** Consistency
**Section:** 7 (Test Requirements)
**Requirement:** T1-T12
**Problem:** SPEC-14 labels its test requirements T1 through T12. SPEC-01 labels its theoretical invariants T1 through T7. When SPEC-14 Section 7 says "T9. Invariant preservation: for all encodings and arithmetic operations in T1-T8, the generated nets MUST satisfy invariants T1-T7 from SPEC-01", it is ambiguous whether "T1-T8" refers to SPEC-14's test labels or SPEC-01's invariants. Within SPEC-14 itself, "T1" could mean "SPEC-14 test T1" or "SPEC-01 invariant T1".

This is not merely a style issue -- it creates concrete ambiguity in T9, which cross-references both systems.

**Impact if unresolved:** Confusion during test implementation. Wrong tests may be attributed to wrong requirements during traceability.
**Suggested resolution:** Rename SPEC-14's test labels to use a distinct prefix, e.g., `E-T1` through `E-T12` (Encoding Tests), or `T14.1` through `T14.12`.

---

### SC-006: Multiplication and exponentiation net construction critically underspecified
**Severity:** CRITICAL
**Axis:** Completeness
**Section:** 4.3.2 (Multiplication), 4.3.3 (Exponentiation)
**Requirement:** R16, R17
**Problem:** Section 4.3.1 (Addition) provides detailed construction steps (5 numbered steps with wire connections) and even mentions an alternative direct construction approach. In contrast, Section 4.3.2 (Multiplication) says only "Similar pattern using `mul = lambda m. lambda n. lambda f. m (n f)`" with one sentence of explanation. Section 4.3.3 (Exponentiation) provides only two sentences.

There are no port connection tables for `mul` or `exp` arithmetic nets. There is no pseudocode. There are no diagrams. The multiplication combinator involves composition of functions that requires DUP agents in the combinator itself (as the spec briefly notes) -- this is non-trivial and an implementer cannot derive the correct wiring from "Similar pattern."

Multiplication is the canonical Profile B benchmark for the TCC. Getting it wrong invalidates the experimental results.

**Impact if unresolved:** An implementer must independently derive the lambda-to-IC-net translation for `mul` and `exp` combinators, risking incorrect encodings that would silently produce wrong results (detected only at test time, if the tests are correct).
**Suggested resolution:** Provide the same level of detail for `build_mul` and `build_exp` as for `build_add`: numbered construction steps, wire connections, and ideally a port connection table for a small example (e.g., `mul(2, 2)` and `exp(2, 2)`).

---

### SC-007: Sub-net ID space composition not specified for arithmetic net construction
**Severity:** HIGH
**Axis:** Completeness
**Section:** 4.3 (Arithmetic Net Construction)
**Requirement:** R15, R16, R17
**Problem:** `build_add(a, b)` creates two Church numeral sub-nets (via `encode_nat(a)` and `encode_nat(b)`) and then composes them. But each call to `encode_nat` produces a `Net` with `next_id` starting at 0. If both sub-nets have agents with IDs 0, 1, 2, ..., how are they merged into a single net without ID collisions?

Section 4.3.1 describes the composition at a high level (steps 1-5) but does not explain the mechanics of combining two independently constructed nets. The spec must address:
- Do we remap IDs from one sub-net?
- Do we construct everything in a single `Net` from the start (calling `create_agent` sequentially)?
- Do we use a `merge_into` operation?

SPEC-01 invariant I3 (ID monotonicity) requires all IDs to be unique within a net.

**Impact if unresolved:** An implementer may produce nets with duplicate agent IDs, violating I3 and causing corrupt reductions.
**Suggested resolution:** Explicitly state one of: (a) `build_add` constructs everything in a single `Net` by calling `create_agent` sequentially (recommended -- simplest approach), or (b) `build_add` creates sub-nets and remaps IDs before merging. Option (a) means `encode_nat` should also accept a `&mut Net` parameter rather than returning a new `Net`, or there should be a separate internal builder API.

---

### SC-008: Duplicate Section 9 numbering
**Severity:** MEDIUM
**Axis:** Consistency
**Section:** 9
**Requirement:** N/A (structural)
**Problem:** The spec has two sections numbered "## 9.": "9. Open Questions" (line 718) and "9. Cross-References (Specs Affected)" (line 734). This violates basic document structure and could cause confusion when referencing sections by number.

**Impact if unresolved:** Ambiguous section references.
**Suggested resolution:** Renumber "Cross-References" to Section 10.

---

### SC-009: Church(0) self-loop on CON_1 and T1/I1 implications
**Severity:** MEDIUM
**Axis:** Invariant Preservation
**Section:** 3.2 (R5), 4.1, 4.2
**Requirement:** R5, R8
**Problem:** Church(0) encoding specifies a self-loop: `CON_1.p1 <-> CON_1.p2` (the inner lambda's auxiliary ports connected to each other). While self-loops are not prohibited by Lafont's theory (they represent the identity function `lambda x. x` correctly), the spec should explicitly confirm that this satisfies T1 and I1.

T1's formal statement says: "for every agent `a` and every port `p`, there exists exactly one `PortRef q` such that `ports[agent_port(a.id, p)] == q` and `ports[port_index(q)] == agent_port(a.id, p)`." For a self-loop where p1 -> p2 and p2 -> p1 of the same agent, this IS satisfied (p1 maps to p2 and p2 maps back to p1). But I1 says `ports[ports[p]] == p` -- here `ports[p1] = p2` and `ports[p2] = p1`, so `ports[ports[p1]] = ports[p2] = p1`. This holds.

However, this pattern will cause the decode algorithm to match the `n=0` special case at line 528-530 (checking `x_bind == PortRef::AgentPort(lam_x, 2)` and `x_body == PortRef::AgentPort(lam_x, 1)`). The spec should explicitly note that self-loops are a valid encoding pattern, as programmers unfamiliar with IC may see them as bugs.

**Impact if unresolved:** An implementer might add assertions that reject self-loops, breaking Church(0).
**Suggested resolution:** Add a note to R5 or the Church(0) entry in Section 4.2 explicitly stating: "The self-loop on CON_1 auxiliary ports is correct and satisfies T1/I1. Self-loops represent the identity function (lambda x. x) in the IC encoding of lambda calculus."

---

### SC-010: `encode_nat` returns `Net` but `build_add` needs to compose nets
**Severity:** HIGH
**Axis:** Completeness
**Section:** 3.2 (R4), 3.4 (R15-R17)
**Requirement:** R4, R15
**Problem:** R4 specifies `encode_nat(n: u64) -> Net`, which returns a complete, independent `Net`. R15 specifies `build_add(a: u64, b: u64) -> Net`, which must compose two Church numerals with a combinator into a single net. But there is no API for merging two `Net` structs or building a Church numeral into an existing net.

If `build_add` calls `encode_nat(a)` and `encode_nat(b)`, it gets two separate `Net` objects. Merging them requires: (1) remapping all agent IDs from one net to avoid collisions, (2) merging the agent arenas, (3) merging the port arrays, (4) reconnecting the interface ports. None of this is specified.

The simpler approach -- having `build_add` construct everything in a single `Net` using raw `create_agent`/`connect` calls -- makes `encode_nat(n: u64) -> Net` the wrong API for internal composition. An `encode_nat_into(net: &mut Net, n: u64) -> AgentId` (returning the root agent ID) would be more composable.

**Impact if unresolved:** The public API makes internal composition awkward or impossible without ID remapping machinery not specified anywhere.
**Suggested resolution:** Add a requirement for an internal builder variant: `encode_church_into(net: &mut Net, n: u64) -> AgentId` that constructs the Church numeral inside an existing net and returns the root agent ID. `encode_nat(n) -> Net` can be a convenience wrapper that creates a new `Net`, calls `encode_church_into`, and sets the root. `build_add` would use `encode_church_into` internally.

---

### SC-011: `set_root` method not defined in SPEC-02
**Severity:** MEDIUM
**Axis:** Consistency
**Section:** 4.1 (construction algorithm)
**Requirement:** R9
**Problem:** The construction algorithm calls `net.set_root(PortRef::AgentPort(lam_f, 0))`. SPEC-02 shows root being set via direct field assignment: `net.root = Some(PortRef::AgentPort(root_agent, 0))` (Section 4.9). There is no `set_root` method defined in SPEC-02. While this is a minor API discrepancy (a setter method vs direct field access), it should be consistent across specs.

**Impact if unresolved:** Minor inconsistency; implementer can infer intent.
**Suggested resolution:** Either use `net.root = Some(...)` in the algorithm (matching SPEC-02), or add `set_root` to SPEC-02 as a convenience method.

---

### SC-012: No edge case specification for `encode_nat` overflow or max range boundary
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 3.2 (R4)
**Requirement:** R4
**Problem:** R4 states `encode_nat(n: u64)` MUST work for `n in [0, 10_000]`. The spec does not specify:
1. What happens when `n > 10_000`? Should it panic, return an error, or silently succeed?
2. Why 10,000 specifically? Church(10000) requires 2*10000+1 = 20,001 agents, consuming 20001 * 3 * 4 bytes = ~240 KB in the port array. This is tiny. The real constraint is reduction time for arithmetic operations.
3. The function signature returns `Net` (not `Result<Net, _>`), so it cannot return an error. If it should reject inputs > 10,000, it must panic.

**Impact if unresolved:** Unclear behavior at the boundary. An implementer may allow any `u64` and silently produce a net with 2^64 agents, causing OOM.
**Suggested resolution:** Either (a) specify that `encode_nat` MUST panic with a descriptive message for `n > 10_000`, or (b) change the signature to `Result<Net, EncodingError>`, or (c) remove the upper bound and document that the practical limit is determined by available memory.

---

### SC-013: Complexity claim for `build_add` reduction interactions is O(a+b) -- needs justification
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 3.5 (R20)
**Requirement:** R20
**Problem:** R20 claims `build_add(a, b)` requires O(a + b) reduction interactions. The lambda calculus definition of addition is `add = lambda m. lambda n. lambda f. lambda x. m f (n f x)`. When applied to church(a) and church(b), the beta-reduction unfolds f^a(f^b(x)) = f^(a+b)(x). But the IC net reduction involves:
- Beta-reductions to apply the add combinator to both arguments (at least 2-4 interactions per combinator application)
- If using standard combinator composition (Section 4.3.1, standard approach): additional beta-reductions for the add combinator itself (4 lambdas in add = at least 4 more interactions)
- DUP-CON commutations if f needs to be shared

The O(a+b) claim may be correct but is not derived or justified. The benchmark table in Section 8.2 gives `~100` interactions for `add(50, 50)`, which is consistent with O(a+b) = O(100). But this is an assertion without proof.

**Impact if unresolved:** If the complexity claim is wrong, benchmark expectations will be miscalibrated. The spec should either derive the bound or cite a reference.
**Suggested resolution:** Add a brief derivation in the Rationale section or in Section 4.5 showing how the O(a+b) bound arises from the specific IC net reduction of Church addition. Alternatively, note that exact counts will be determined empirically and the O notation is an estimate.

---

### SC-014: SPEC-14 test T9 references "invariants T1-T7 from SPEC-01" but SPEC-14 tests are also named T1-T8
**Severity:** LOW
**Axis:** Consistency
**Section:** 7 (T9)
**Requirement:** T9
**Problem:** This is a specific instance of SC-005. T9 says "for all encodings and arithmetic operations in T1-T8" -- here T1-T8 means SPEC-14's own test labels. Then it says "MUST satisfy invariants T1-T7 from SPEC-01" -- here T1-T7 means SPEC-01's invariants. The qualifier "from SPEC-01" disambiguates, but only if the reader is paying close attention.

**Impact if unresolved:** Potential misinterpretation during test implementation.
**Suggested resolution:** Addressed by SC-005 (rename SPEC-14 test labels).

---

### SC-015: Test T11 mixes `reduce_all` signatures inconsistently
**Severity:** MEDIUM
**Axis:** Testability
**Section:** 7 (T11)
**Requirement:** T11
**Problem:** T11 states: `decode_nat(reduce_all(build_add(a, b)))` MUST equal `decode_nat(extract_result(run_grid(build_add(a, b), k)))`. The left side uses `reduce_all(build_add(a, b))` -- passing the net by value (ownership), but SPEC-03 defines `reduce_all(net: &mut Net) -> ReductionStats`. The right side uses `extract_result(run_grid(...))` which is from SPEC-05/SPEC-13 -- plausible but not fully specified here. Also, `decode_nat` expects `&Net`, but `reduce_all` returns `ReductionStats`, not a `Net`.

**Impact if unresolved:** Test T11 as written is not implementable.
**Suggested resolution:** Rewrite T11 as a multi-step test:
```
let mut local_net = build_add(50, 50);
reduce_all(&mut local_net);
let local_result = decode_nat(&local_net);

let distributed_net = run_grid(build_add(50, 50), k);
let distributed_result = decode_nat(&distributed_net);

assert_eq!(local_result, distributed_result);
```

---

### SC-016: Spec deviates from template -- missing standard sections
**Severity:** LOW
**Axis:** Consistency
**Section:** Document structure
**Requirement:** N/A (template compliance)
**Problem:** The spec template requires Section 7 to be "Open Questions" (currently Section 9). The spec has added non-template sections (7: Test Requirements, 8: Arithmetic Benchmark Scenarios) that push Open Questions to Section 9 and add a duplicate Section 9. While the content is valuable, the deviation from the standard template makes cross-spec navigation inconsistent.

**Impact if unresolved:** Minor friction when navigating between specs.
**Suggested resolution:** Consider restructuring to match the template: move Test Requirements into Section 4 (Design) as a subsection, or into a dedicated appendix. Rename sections to match the standard numbering.

---

### SC-017: `build_exp` correctness requirement tension between SHOULD (R17) and MUST (R18)
**Severity:** MEDIUM
**Axis:** Consistency
**Section:** 3.4 (R17, R18)
**Requirement:** R17, R18
**Problem:** R17 says `build_exp` SHOULD be exposed (not MUST -- it is optional). R18 says "All arithmetic nets MUST reduce to a valid Church numeral Normal Form when processed by `reduce_all`." If `build_exp` is not implemented (SHOULD allows this), then R18's "all arithmetic nets" is vacuously true for `exp`. But the phrasing is confusing: R18 appears to mandate correctness for all three operations including exp, even though exp is optional.

Additionally, T8 (exponentiation correctness test) is a MUST, but R17 (the function itself) is a SHOULD. A MUST test for a SHOULD feature is contradictory.

**Impact if unresolved:** If the implementer skips `build_exp` (which SHOULD permits), they would violate T8 (MUST).
**Suggested resolution:** Either (a) promote R17 to MUST (exponentiation is needed for benchmarks ARITH-EXP-*), or (b) downgrade T8 to SHOULD to match R17. Given that Section 8 includes ARITH-EXP benchmarks as primary evaluation scenarios, promoting to MUST seems appropriate.

---

### SC-018: No specification of how `build_add` creates redexes at application boundaries
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 4.3.1
**Requirement:** R15, R18
**Problem:** Section 4.3.1 states: "After connecting the sub-nets, new active pairs emerge at the application boundaries -- these are the redexes that drive the computation." But it does not specify exactly which connections form these redexes. The 5-step construction plan mentions `@_1.p0 = add_root`, which would be a principal-to-principal connection between the application CON agent and the outermost CON of the add combinator, forming a redex. But the spec does not verify that the correct redexes form, does not count them, and does not show that the resulting reduction produces church(a+b).

**Impact if unresolved:** An implementer cannot verify correctness of the constructed net without manually tracing the reduction. A worked example (e.g., `build_add(1, 1)` with full port table showing the 2 initial redexes) would make this implementable.
**Suggested resolution:** Provide a worked example for `build_add(1, 1)` or `build_add(2, 1)`: show the complete port connection table, identify the initial redexes, and trace a few reduction steps to demonstrate that the result is church(a+b).

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 3 |
| HIGH | 4 |
| MEDIUM | 7 |
| LOW | 2 |

## Mandatory (must fix before implementation)

- **SC-001:** R9 root port contradiction (`FreePort(0)` vs `AgentPort(lam_f, 0)`)
- **SC-002:** `get_agent` method undefined in SPEC-02
- **SC-006:** Multiplication and exponentiation construction critically underspecified
- **SC-003:** `get_target` return type mismatch (`PortRef` vs `Option<PortRef>`)
- **SC-004:** `reduce_all_and_return` function undefined
- **SC-005:** Test label collision T1-T12 vs SPEC-01 T1-T7
- **SC-007:** Sub-net ID composition for arithmetic construction unspecified

## Recommended (should fix)

- **SC-008:** Duplicate Section 9 numbering
- **SC-009:** Self-loop documentation for Church(0)
- **SC-010:** `encode_nat` API not composable for arithmetic construction
- **SC-011:** `set_root` not in SPEC-02
- **SC-012:** Missing edge case for `n > 10_000`
- **SC-013:** O(a+b) interaction count unjustified
- **SC-015:** Test T11 not implementable as written
- **SC-017:** SHOULD/MUST tension between R17 and T8
- **SC-018:** No worked example for arithmetic net redex verification

---

## Checklist

### Consistency
- [ ] All type names match SPEC-00 and SPEC-02 definitions
- [x] `Symbol`, `PortRef`, `AgentId`, `PortId`, `Net` types used correctly
- [ ] `get_agent` referenced but not defined in predecessor specs (SC-002)
- [ ] `get_target` return type inconsistent with SPEC-02 (SC-003)
- [ ] `reduce_all_and_return` not in SPEC-03 (SC-004)
- [ ] `set_root` not in SPEC-02 (SC-011)
- [ ] R9 contradicts Section 4.1 and SPEC-02 root design (SC-001)
- [ ] Test labels T1-T12 collide with SPEC-01 invariant labels (SC-005)
- [ ] Duplicate section numbering (SC-008)
- [x] Terminology (Church Numeral, Encoding, Decoding) is well-defined in Section 2
- [ ] R17 (SHOULD) conflicts with T8 (MUST) (SC-017)

### Testability
- [x] R4 (encode_nat) testable via structure checks and roundtrip
- [x] R5-R7 (Church 0, 1, n) testable via port connection table verification
- [x] R10 (Normal Form) testable via empty redex queue check
- [x] R11 (decode_nat) testable via roundtrip with encode
- [x] R18 (arithmetic correctness) testable via decode after reduction
- [ ] T6-T8 reference undefined `reduce_all_and_return` (SC-004)
- [ ] T11 mixing value/reference semantics (SC-015)
- [x] T12 (decode rejection) testable with known non-Church nets
- [x] R20 (complexity) testable by measuring agent counts

### Completeness
- [x] Church(0) fully specified with diagram and table
- [x] Church(1) fully specified with table
- [x] Church(2) fully specified with table
- [x] Church(n) general pattern specified
- [x] Addition construction partially specified (steps but no worked example)
- [ ] Multiplication construction underspecified (SC-006)
- [ ] Exponentiation construction underspecified (SC-006)
- [ ] Sub-net composition mechanics unspecified (SC-007, SC-010)
- [ ] Behavior for n > 10,000 unspecified (SC-012)
- [ ] Interaction count derivation missing (SC-013)
- [ ] No worked example for arithmetic redex formation (SC-018)
- [x] Decode algorithm fully specified with pseudocode
- [x] CLI integration fully specified with output format
- [x] Generator integration specified
- [x] Benchmark table comprehensive

### Invariant Preservation
- [x] R8 explicitly requires T1-T7 and I1, I3 preservation
- [x] Church(0) self-loop satisfies T1/I1 (verified in SC-009, but needs documentation)
- [x] Church(n) encoding produces zero redexes (R10) -- confirmed by redex verification in tables
- [x] Decode does not modify net (R13) -- `&Net` reference enforced
- [ ] Root port representation unclear -- T1/I1 status for the FreePort(0) connection not analyzed (SC-001)
- [x] Agent counts match formula (2n+1 for n>=2; 3 for n=0,1)
- [x] DUP chain structure connects principal ports of DUPs to auxiliary ports of other agents (no spurious redexes)
- [x] T9 explicitly mandates invariant verification for all test cases
