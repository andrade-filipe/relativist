# SPEC-02 -- Round 2: Defender Response

**Date:** 2026-04-05
**Defender:** Especialista em Specs
**Target:** SPEC-02-net-representation.md
**Critic review:** SPEC-02-round1-critic.md
**Spec version:** Revised v2 -> Revised v3

---

## Summary

| Response | Count |
|----------|-------|
| ACCEPTED | 11 |
| PARTIALLY ACCEPTED | 4 |
| NOT ADDRESSED | 0 |
| **Total issues** | **15** |

---

## Responses

### SC-001: Missing `get_agent` public API
**Response:** ACCEPTED
**Action taken:** Added two new requirements: R15a (`get_agent(id: AgentId) -> Option<&Agent>`) and R15b (`get_agent_mut(id: AgentId) -> Option<&mut Agent>`). Both are MUST-level, O(1), and use the `agents.get(id as usize).and_then(|slot| slot.as_ref())` pattern that SPEC-14 had implemented as a workaround. Added Section 4.5.9 (Agent Accessors) with the full implementation. Updated `is_valid_redex` to use `get_agent` instead of the verbose inline pattern. R15a is declared as the canonical accessor, with a note that callers MUST NOT index into `agents` directly for read access.
**Spec sections modified:** Section 3.3 (R15a, R15b), Section 4.5.8 (is_valid_redex updated), Section 4.5.9 (new)

### SC-002: Root port as DISCONNECTED violates invariant T1
**Response:** ACCEPTED
**Action taken:** Adopted option (a) from the critic: formalized the exception. Added R18a which explicitly documents that the root agent's principal port MAY contain DISCONNECTED in the port array as a permanent, structural exception to T1. The rationale is clear: the root's external connection is represented by `net.root`, not the port array. Updated `assert_adjacency_consistent` (Section 4.6) to determine the root agent ID and explicitly skip only the root agent's principal port, rather than generically skipping all DISCONNECTED ports. The assertion now has a named check (`R18a: root agent's principal port is exempt`) that documents why the skip occurs, preventing future confusion.

Option (b) (self-connection sentinel) was rejected because it would introduce a degenerate self-loop on port 0 that could be confused with a valid active pair by code that checks for principal-port connections. Option (c) (ROOT sentinel) was rejected because it introduces a third PortRef variant solely for one edge case, adding complexity to all pattern matches across the codebase.
**Spec sections modified:** Section 3.4 (R18a), Section 4.6 (assert_adjacency_consistent rewritten), Section 4.9 (root behavior notes)

### SC-003: Self-loop policy is unspecified
**Response:** ACCEPTED
**Action taken:** Added R18b which formally resolves the ambiguity between intra-agent connections and same-port self-connections:

1. **Intra-agent connections** (e.g., `connect(AgentPort(x, 1), AgentPort(x, 2))`) are explicitly VALID. They satisfy T1 bidirectionality and are required by Church(0) encoding (SPEC-14 R5).
2. **Same-port self-connections** (e.g., `connect(AgentPort(x, 1), AgentPort(x, 1))`) are explicitly INVALID. A debug assertion in `connect` now rejects them with `assert_ne!(a, b)`.
3. T1's "exactly one other port" is clarified to mean "exactly one distinct port reference" (a different `(AgentId, PortId)` pair).

This resolves the apparent contradiction between SPEC-14 R5 (which allows intra-agent connections) and SPEC-12 R58 (which rejects same-port self-connections). The two specs were always consistent in substance -- they were addressing different operations -- but SPEC-12 R58's language ("Self-loops (port-to-self)") conflated them. SPEC-02 now provides the authoritative definition that both specs can reference.
**Spec sections modified:** Section 3.4 (R18b), Section 4.5.4 (connect -- added debug assertion and doc comments)

### SC-004: `disconnect` does not handle FreePort endpoints correctly
**Response:** ACCEPTED
**Action taken:** Updated R14 with an explicit note documenting the Border Map staleness behavior: "When the target of a disconnected port is a `FreePort(bid)`, the `set_port` call on the FreePort side is a no-op (FreePort has no slot in the port array). The corresponding entry in the external `free_port_index` or Border Map becomes stale. This is by design: SPEC-05 R6 handles missing border entries during merge. The `disconnect` operation does NOT update external maps." This documents the asymmetry and prevents future implementers from flagging it as a bug.
**Spec sections modified:** Section 3.3 (R14)

### SC-005: No `count_live_agents` or iteration API for live agents
**Response:** ACCEPTED
**Action taken:** Added R16a (`count_live_agents(&self) -> usize`, O(A)) and R16b (`live_agents(&self) -> impl Iterator<Item = &Agent>`). Both are MUST-level. Added Section 4.5.10 (Iteration and Counting) with implementations. These formalize the iteration pattern that 6+ successor specs needed. The `live_agents` iterator uses `filter_map(|s| s.as_ref())` to skip None slots, encapsulating the internal representation.
**Spec sections modified:** Section 3.3 (R16a, R16b), Section 4.5.10 (new)

### SC-006: SPEC-12 R56 allows `FreePort` as root but SPEC-14 R9 requires `AgentPort`
**Response:** ACCEPTED
**Action taken:** Added R6a which constrains root to: `None` or `Some(AgentPort(id, 0))` where `id` is a live agent. `FreePort` values are NOT valid for `root`. The rationale is documented: the root field exists precisely to avoid conflation with FreePort sentinels (Section 5.6). SPEC-12 R56's citation of SPEC-14 R9 as evidence for FreePort roots is noted as factually incorrect -- SPEC-14 R9 explicitly mandates `AgentPort` roots.

Regarding SPEC-12 T11c (`root free(0)`): this is a Text DSL parser syntax test. The parser sets `net.root = Some(FreePort(0))` as a parsing operation, but the SPEC-12 parser's T1/I2 validation pass (R11) should reject it as invalid. SPEC-02 documents this as an edge case that SPEC-12 needs to reconcile internally (outside SPEC-02's territory). The cleanest resolution for SPEC-12 would be either (a) T11c tests that the parser correctly rejects FreePort roots with an error, or (b) SPEC-12 allows FreePort roots as a Lafont interface construct but documents that downstream functions (decode_nat, etc.) will not support them.
**Spec sections modified:** Section 3.1 (R6a), Section 4.9 (root doc comments updated)

### SC-007: ERA agents waste 2 port slots with DISCONNECTED
**Response:** ACCEPTED
**Action taken:** Added `assert_era_unused_ports_clean` to Section 4.6 as a debug assertion that verifies ERA agents' unused port slots (ports 1 and 2) contain DISCONNECTED. Added the new assertion to `assert_all_invariants`. This catches silent corruption in ERA slots that the arity-aware iteration in `assert_adjacency_consistent` would otherwise miss.
**Spec sections modified:** Section 4.6 (assert_era_unused_ports_clean, assert_all_invariants)

### SC-008: `connect` with `FreePort` arguments does not detect redexes
**Response:** ACCEPTED
**Action taken:** Added a doc comment on the `connect` function explaining the redex detection limitation: "Redex detection note: redex detection only fires when BOTH endpoints are `AgentPort(_, 0)`. Connections involving `FreePort` never produce redexes; border redexes are detected during merge when `FreePort` sentinels are resolved to `AgentPort` endpoints (SPEC-05 R5)." This is documentation-only, as the behavior is correct.
**Spec sections modified:** Section 4.5.4 (connect doc comments)

### SC-009: Root port behavior during reduction is unspecified
**Response:** ACCEPTED
**Action taken:** Adopted option (c) from the critic. Added an explicit note in Section 4.9: "The `root` field is set once at net construction and is NOT automatically updated by `connect`, `disconnect`, or reduction rules. For Church numeral arithmetic nets (SPEC-14), the root agent's principal port is DISCONNECTED in the port array (not connected to another principal port), so the root agent does NOT participate in any active pair and is never consumed by reduction. The result is extracted by following `net.root -> agent -> auxiliary ports` after reduction completes." Added a forward-looking note: "If a future encoding requires the root agent to be consumed during reduction, the reduction engine or encoding module MUST update `net.root` explicitly."
**Spec sections modified:** Section 4.9

### SC-010: `BorderMap` type is defined but not part of `Net` struct
**Response:** PARTIALLY ACCEPTED
**Action taken:** Removed the `BorderMap` type alias code block from Section 4.10. Replaced it with a prose description pointing to SPEC-04 as the authoritative definition: "The `BorderMap` type alias is defined in SPEC-04 (partition module), where it is stored and used. SPEC-02 documents the concept but defers the type definition to SPEC-04 for proximity to its usage."

The fix differs from the critic's suggestion in that SPEC-02 retains the conceptual description of the Border Map (since it is referenced in the Net's invariant discussion), but no longer defines the type alias. The concept is essential context for understanding FreePort storage; the type alias belongs in the module that owns it.
**Spec sections modified:** Section 4.10

### SC-011: `PartialEq` for Net needed but not required
**Response:** ACCEPTED
**Action taken:** Added R26a requiring Net to derive `PartialEq` and `Eq`. Updated the Net struct code block in Section 4.3 to include `PartialEq, Eq` in the derive list. Added a clarifying note: "structural equality (`==`) requires identical AgentIds; for graph isomorphism (structural equivalence modulo ID renaming), use `nets_isomorphic` (SPEC-08)." This makes R26 testable.
**Spec sections modified:** Section 3.6 (R26a), Section 4.3 (Net derive updated)

### SC-012: `is_reduced` only checks queue emptiness, not actual Normal Form
**Response:** PARTIALLY ACCEPTED
**Action taken:** Added a note in Section 4.9 (adjacent to the `is_reduced` context): "The `is_reduced` function (R16) checks the redex queue, not the net's actual topological state. For a net constructed via the CRUD API with proper use of `connect` (which populates the redex queue incrementally), an empty queue reliably indicates Normal Form. For nets constructed by other means (e.g., deserialization, manual mutation), use `drain_stale()` or scan the net for active pairs to verify Normal Form."

The fix differs from the critic's suggestion in that we do NOT rename `is_reduced` to `is_queue_empty`. The name `is_reduced` is well-established in the codebase (used by SPEC-03, SPEC-05, SPEC-13) and accurately describes the function's intent for its primary use case (after `reduce_all`). The documentation clarifies the semantics for edge cases without requiring a breaking rename across 12+ successor specs.
**Spec sections modified:** Section 4.9

### SC-013: Serialization format includes DISCONNECTED sentinels without documentation
**Response:** ACCEPTED
**Action taken:** Added a note in Section 4.9 under "Serialization": "The port array may contain the sentinel `FreePort(u32::MAX)` (DISCONNECTED) in slots for unused ERA ports (ports 1-2) and the root agent's principal port. Receivers MUST treat `FreePort(u32::MAX)` as an invalid/unconnected sentinel, not as a valid FreePort ID."
**Spec sections modified:** Section 4.9

### SC-014: No requirement for `Net::with_capacity` to satisfy I3
**Response:** PARTIALLY ACCEPTED
**Action taken:** Added a note to R10: "Direct mutation of `agents` or `ports` bypasses invariant checks and may violate I1-I3. Callers MUST use `create_agent`, `connect`, `disconnect`, and `remove_agent` for all mutations." This documents the expectation without making the fields private (which would complicate serialization and iteration).

The fix differs from the critic's suggestion in that we do NOT make `agents` and `ports` fields private. Making them private would require significant API surface expansion (read-only accessors, slice views for serialization, etc.) and is an implementation decision best left to the engineer. The MUST note in R10 establishes the contract; debug assertions (R20) catch violations.
**Spec sections modified:** Section 3.2 (R10)

### SC-015: `connect` does not validate port index against agent arity
**Response:** PARTIALLY ACCEPTED
**Action taken:** The self-loop assertion added to `connect` for SC-003 (`assert_ne!(a, b)`) provides partial validation. Full arity validation in `connect` was NOT added as a MUST requirement because:

1. `connect` is on the hot path (called during every reduction rule) and adding two agent lookups + arity checks per call would impact performance.
2. The post-hoc assertion `assert_refs_valid` (Section 4.6) already catches invalid port references after each operation in debug mode.
3. The reduction engine (SPEC-03) constructs PortRefs from known-valid agents, so invalid port indices would only arise from programmer error, not runtime data.

However, the doc comment on `connect` now documents the expectation: callers must provide valid port references. The existing `assert_refs_valid` assertion provides the safety net in debug mode.
**Spec sections modified:** Section 4.5.4 (connect doc comments, debug assertion for self-loops)

---

## Changes Made to SPEC-02

### Header
- Status changed from "Revised v2" to "Revised v3"

### Section 3.1 (Fundamental Types)
- R6a: Added new requirement constraining `root` to `None` or `Some(AgentPort(id, 0))` with rationale and cross-spec notes

### Section 3.2 (Storage)
- R10: Added note about direct mutation bypassing invariant checks

### Section 3.3 (Operations)
- R14: Extended with FreePort/Border Map staleness documentation
- R15a: Added `get_agent` accessor requirement (MUST, O(1))
- R15b: Added `get_agent_mut` accessor requirement (MUST, O(1))
- R16a: Added `count_live_agents` requirement (MUST, O(A))
- R16b: Added `live_agents` iterator requirement (MUST)

### Section 3.4 (Representation Invariants)
- R18a: Added root port exception to T1 (MUST)
- R18b: Added self-loop policy clarification (MUST)

### Section 3.6 (Serialization)
- R26a: Added `PartialEq`/`Eq` derive requirement for Net (MUST)

### Section 4.3 (Net Structure)
- Updated Net derive to include `PartialEq, Eq`

### Section 4.5.4 (connect)
- Added debug assertion rejecting same-port self-connections (R18b)
- Added doc comments for self-loop policy and FreePort redex detection

### Section 4.5.8 (Redex Validation)
- Updated `is_valid_redex` to use `get_agent` instead of verbose inline pattern

### Section 4.5.9 (Agent Accessors -- new)
- Added `get_agent` and `get_agent_mut` implementations

### Section 4.5.10 (Iteration and Counting -- new)
- Added `count_live_agents` and `live_agents` implementations

### Section 4.6 (Debug Assertions)
- `assert_adjacency_consistent`: Rewritten to explicitly handle root port exception (R18a) by computing `root_agent_id` and skipping only that specific port
- Added `assert_era_unused_ports_clean`: Validates ERA unused port slots contain DISCONNECTED
- Updated `assert_all_invariants` to include ERA port check

### Section 4.9 (Root Port)
- Updated root doc comments to specify `AgentPort(id, 0)` constraint
- Added "Root behavior during reduction" paragraph documenting that root is set-once and root agent is never consumed in Church numeral nets
- Added "Serialization" note about DISCONNECTED sentinel in serialized format
- Added "`is_reduced` semantics note" clarifying queue-based semantics

### Section 4.10 (FreePort Storage)
- Removed `BorderMap` type alias code block; replaced with prose pointing to SPEC-04 as authoritative definition

---

## Residual Risks

### Cross-spec consistency: SPEC-12 R56 and T11c

R6a constrains `root` to `AgentPort` values, which contradicts SPEC-12 R56 (which allows FreePort roots) and T11c (which tests `root free(0)`). SPEC-02 documents the contradiction and declares `AgentPort`-only roots as authoritative. SPEC-12 will need a revision to reconcile R56 and T11c with this constraint. Possible resolutions:
- T11c becomes a negative test: `root free(0)` MUST produce a validation error.
- Or SPEC-12 R56 is amended to remove the FreePort allowance.

This is outside SPEC-02's territory; a future SPEC-12 review should address it.

### Field visibility (`agents`, `ports` are `pub`)

R10 now documents that direct mutation bypasses invariant checks, but the fields remain `pub`. This is a deliberate trade-off: private fields would require extensive accessor API and complicate serde deserialization. The implementer may choose to make fields private as an implementation decision, but the spec does not mandate it. Debug assertions (R20) provide the safety net.

### `is_reduced` naming

The function name `is_reduced` was retained despite the critic's suggestion to rename it to `is_queue_empty`. The documentation now clarifies the semantics. If the name causes persistent confusion during implementation, the engineer may rename it locally, but the spec requirement (R16) defines the semantics precisely.
