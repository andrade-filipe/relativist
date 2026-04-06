# SPEC-02 -- Round 1: Spec Critic Review

**Date:** 2026-04-05
**Reviewer:** Spec Critic (adversarial)
**Target:** SPEC-02-net-representation.md (status: Revised v2)
**Predecessors consulted:** SPEC-00, SPEC-01
**Successors consulted:** SPEC-03, SPEC-04, SPEC-05, SPEC-06, SPEC-07, SPEC-08, SPEC-09, SPEC-10, SPEC-11, SPEC-12, SPEC-13, SPEC-14

---

## Overall Assessment

SPEC-02 is a solid, well-justified spec that defines the core data structures of the entire Relativist system. The fundamental types (Symbol, AgentId, PortRef, Agent, Net), CRUD operations, and storage design are all clearly specified with thorough rationale and invariant traceability. However, as the foundation spec consumed by 12 successor specs, it has accumulated several gaps and inconsistencies that were exposed when successor specs attempted to use its API. The most critical issue is the absence of a `get_agent` accessor, which SPEC-14 had to work around with a local helper. Other issues involve the root port's interaction with invariant T1 (DISCONNECTED in the port array violates "every port connected to exactly one other port"), an unspecified self-loop policy (SPEC-14 allows them, SPEC-12 rejects them, SPEC-02 is silent), and missing support for iteration patterns that successor specs require.

**Verdict:** NEEDS REVISION

---

## Issues

### SC-001: Missing `get_agent` public API
**Severity:** CRITICAL
**Axis:** Completeness
**Section:** 3.3 (Operations), 4.5 (CRUD Operations)
**Requirement:** (missing -- no R-number covers this)
**Problem:** SPEC-02 defines `create_agent`, `remove_agent`, `connect`, `disconnect`, `get_target`, `set_port`, `is_reduced`, and `is_valid_redex` as Net operations. There is NO `get_agent(id: AgentId) -> Option<&Agent>` operation. Yet this operation is needed by virtually every successor spec:

- **SPEC-03** (reduction): accesses `net.agents[a as usize].unwrap().symbol` directly (lines 514-515, 547, 682-683) -- raw indexing with unwrap, no bounds check, panics on invalid ID.
- **SPEC-14** (encoding): defines a local `get_agent` helper (line 632) because the public API does not expose one: `fn get_agent(net: &Net, id: AgentId) -> Option<&Agent> { net.agents.get(id as usize).and_then(|slot| slot.as_ref()) }`. SPEC-14 explicitly notes: "This helper is NOT part of SPEC-02's public API" (line 637).
- **SPEC-01** (invariants): accesses `net.agents.get(agent_id as usize)` in pseudocode for the invariant checker (line 414).
- **SPEC-02 itself**: `is_valid_redex` uses `self.agents.get(a as usize).map_or(false, |s| s.is_some())` -- a verbose 1-liner that `get_agent` would simplify.

The field `agents` is `pub`, so callers can index into it directly. But direct indexing is unsafe (panics on out-of-bounds or unwrap of None), inconsistent (SPEC-03 uses `.unwrap()`, SPEC-14 uses `.get().and_then()`), and breaks encapsulation.

**Impact if unresolved:** Every consumer of SPEC-02 must reinvent agent lookup with ad-hoc patterns. This creates inconsistency: some callers panic on missing agents (SPEC-03), others return Option (SPEC-14). If the internal representation ever changes (e.g., from `Vec<Option<Agent>>` to a `SlotMap`), every direct access site breaks.
**Suggested resolution:** Add requirement R-NEW-1: "The `get_agent(id: AgentId) -> Option<&Agent>` operation MUST return a reference to the agent with the given ID, or None if the ID is out of range or the slot is empty. Expected complexity: O(1)." Add the corresponding design code in Section 4.5. This formalizes what SPEC-14 already implemented as a workaround.

---

### SC-002: Root port as DISCONNECTED violates invariant T1
**Severity:** CRITICAL
**Axis:** Invariant Preservation
**Section:** 4.9 (Root Port), 3.4 (Representation Invariants)
**Requirement:** R18 (bidirectionality), R19 (reference validity)
**Problem:** SPEC-02 Section 4.9 states the root is an `Option<PortRef>` stored separately from the port array. SPEC-14 makes the consequence explicit: "The principal port of the root agent has no internal peer in the port array (its slot contains `DISCONNECTED`)." (SPEC-14 line 119, line 447, line 453).

This means for a net with a root agent, `ports[root_agent_id * 3 + 0] == DISCONNECTED`. But SPEC-02 Section 4.4 states: "A slot containing DISCONNECTED violates invariant T1 (linearity) if it persists after a reduction rule completes." And SPEC-01 T1 states: "Every port of every live agent in the net MUST be connected to exactly one other port."

The root agent's principal port is permanently DISCONNECTED. This is a standing T1 violation. The debug assertion `assert_adjacency_consistent` (SPEC-02 Section 4.6) skips DISCONNECTED ports with `if target == DISCONNECTED { continue; // Allowed transiently }`, but the comment says "transiently" -- the root port is not transient, it is permanent.

Additionally, R19 states "Every reference `AgentPort(id, p)` in the port array MUST point to an existing agent" -- but the root agent's port slot contains DISCONNECTED (which is `FreePort(u32::MAX)`), not an AgentPort, so R19 is technically not violated. However, R18 (bidirectionality: "if `ports[idx(a)] == b`, then `ports[idx(b)] == a`") cannot hold for the root port because there is no reverse entry.

**Impact if unresolved:** The invariant checker will either (a) silently skip the root port's T1 violation (masking real bugs), or (b) correctly report it as a violation (causing false positives in debug mode). Neither is acceptable. SPEC-14 already had to document the exception extensively (3 separate notes). Every future spec that checks T1 must know about this exception.
**Suggested resolution:** Either:
(a) **Formalize the exception in T1/R18:** Add a clause: "T1 linearity applies to all ports of live agents EXCEPT the principal port of the root agent (when `net.root == Some(AgentPort(id, 0))`), which MAY be DISCONNECTED in the port array." Update `assert_adjacency_consistent` to explicitly skip the root port (not all DISCONNECTED ports). OR
(b) **Wire the root to itself:** Store `ports[root_id * 3 + 0] = PortRef::AgentPort(root_id, 0)` (a self-connection sentinel for the root). This satisfies T1 bidirectionality without an exception. `net.root` still provides the external reference. OR
(c) **Use a dedicated ROOT sentinel:** Define `pub const ROOT: PortRef = PortRef::FreePort(u32::MAX - 1);` as a distinct sentinel from DISCONNECTED. Store `ports[root_id * 3 + 0] = ROOT`. Update T1 to allow ROOT as a valid target for the root agent's principal port.

Option (a) is simplest. Option (c) is most explicit.

---

### SC-003: Self-loop policy is unspecified -- SPEC-14 and SPEC-12 contradict
**Severity:** HIGH
**Axis:** Consistency | Completeness
**Section:** 3.4 (Representation Invariants), 4.5 (connect)
**Requirement:** R13 (connect), R18 (bidirectionality)
**Problem:** SPEC-02 is silent on whether self-loops (connecting a port to itself, e.g., `connect(AgentPort(x, 1), AgentPort(x, 2))`) are valid. Two successor specs disagree:

- **SPEC-14 R5** (encoding): Self-loops are mandatory for Church(0): "`CON_1.p1 <-> CON_1.p2` is a valid self-loop representing the identity function (`lambda x. x`). Implementers MUST NOT add assertions that reject self-loops, as they would break Church(0)."
- **SPEC-12 R58** (user I/O): "`wire a.left a.left` MUST be rejected with a parse error: 'port cannot be connected to itself at line {line}'. Self-loops (port-to-self) violate T1's 'exactly one *other* port' requirement."

Note that SPEC-14's self-loop connects two DIFFERENT ports of the SAME agent (`p1 <-> p2`), while SPEC-12 R58 rejects connections of a port to ITSELF (`p <-> p`). These are semantically different operations, but SPEC-12's language "Self-loops (port-to-self)" conflates them. The T1 text says "exactly one other port" which is ambiguous: does "other" mean a different port index, or a different agent?

Additionally, SPEC-01 T1's formal statement says `ports[agent_port(a.id, p)] == q` and `ports[port_index(q)] == agent_port(a.id, p)` -- this allows `p` and `q` to be ports of the same agent (as long as bidirectionality holds). So T1 permits intra-agent connections but the informal text ("exactly one *other* port") might be read as prohibiting them.

**Impact if unresolved:** If the implementer adds an assertion in `connect` that rejects same-agent connections, Church(0) encoding breaks. If the implementer allows all self-connections, `connect(AgentPort(x, 1), AgentPort(x, 1))` would write `ports[idx] = ports[idx]` (a no-op on the same slot), which is degenerate.
**Suggested resolution:** SPEC-02 MUST explicitly clarify:
1. **Intra-agent connections** (e.g., `connect(AgentPort(x, 1), AgentPort(x, 2))`) are VALID. They satisfy T1 bidirectionality: `ports[x*3+1] == AgentPort(x, 2)` and `ports[x*3+2] == AgentPort(x, 1)`.
2. **Same-port self-connections** (e.g., `connect(AgentPort(x, 1), AgentPort(x, 1))`) are INVALID (degenerate: would require `ports[idx] == ports[idx]` pointing to itself, which satisfies bidirectionality formally but represents a port connected to itself -- a structural impossibility in IC theory).
3. Update T1's informal text to say "exactly one other port (a different port reference)" instead of ambiguous "other port."

---

### SC-004: `disconnect` does not handle FreePort endpoints correctly
**Severity:** HIGH
**Axis:** Completeness | Invariant Preservation
**Section:** 4.5.5 (disconnect)
**Requirement:** R14
**Problem:** The `disconnect` implementation:
```rust
pub fn disconnect(&mut self, port: PortRef) {
    let target = self.get_target(port);
    if target != DISCONNECTED {
        self.set_port(target, DISCONNECTED);
    }
    self.set_port(port, DISCONNECTED);
}
```

If `port` is an `AgentPort` that is connected to a `FreePort(bid)` (i.e., `ports[agent_port_idx] == FreePort(bid)`), then `target == FreePort(bid)`. The code calls `self.set_port(FreePort(bid), DISCONNECTED)`, which is a no-op because `set_port` ignores FreePort inputs. Then it sets the AgentPort slot to DISCONNECTED. The result is correct for the AgentPort side, but the FreePort -> AgentPort mapping in the external Border Map is now stale (the Border Map still thinks `FreePort(bid)` points to this agent).

Similarly, `remove_agent` calls `disconnect` for each port. If an agent had a port connected to a FreePort (boundary), `remove_agent` disconnects the port array entry but leaves the Border Map / `free_port_index` with a dangling reference.

**Impact if unresolved:** During merge (SPEC-05), the `free_port_index` may reference an agent that no longer exists. SPEC-05 R6 handles this gracefully ("If during the merge one side of a border wire cannot be found [...], the border wire MUST be discarded silently"), so the system does not crash. But the gap in SPEC-02 means it does not acknowledge or document this asymmetry. A future implementer adding assertions in `disconnect` might flag the stale Border Map as a bug.
**Suggested resolution:** Add a note in R14 or Section 4.5.5: "When the target of a disconnected port is a `FreePort(bid)`, the corresponding entry in the external `free_port_index` or Border Map becomes stale. This is by design: SPEC-05 R6 handles missing border entries during merge. The `disconnect` operation does NOT update external maps."

---

### SC-005: No `count_live_agents` or iteration API for live agents
**Severity:** HIGH
**Axis:** Completeness
**Section:** 3.3 (Operations)
**Requirement:** (missing)
**Problem:** Multiple successor specs need to iterate over live agents or count them:

- **SPEC-04** (partition): The split function must iterate all live agents to classify them by partition.
- **SPEC-05** (merge): TASK-0073 in the backlog explicitly creates a `count_live_agents` helper.
- **SPEC-08** (test): Tests need to compare agent counts for correctness verification.
- **SPEC-09** (benchmarks): Metrics include agent counts.
- **SPEC-12** (user I/O): The `inspect` subcommand reports agent counts and type distribution (CON/DUP/ERA).
- **SPEC-14** (encoding): The `encode_nat` function specifies exact agent counts per Church numeral.

SPEC-02 provides no API for this. All callers must manually iterate `net.agents.iter().filter(|s| s.is_some())`, which is verbose and leaks the internal representation.

**Impact if unresolved:** Code duplication across 6+ modules. If the representation changes, all iteration sites break.
**Suggested resolution:** Add requirements:
- R-NEW-2: `count_live_agents(&self) -> usize` MUST return the number of live agents. O(A) where A is the arena size.
- R-NEW-3: `live_agents(&self) -> impl Iterator<Item = &Agent>` MUST provide an iterator over all live agents.

---

### SC-006: SPEC-12 R56 allows `FreePort` as root but SPEC-14 R9 requires `AgentPort`
**Severity:** HIGH
**Axis:** Consistency
**Section:** 4.9 (Root Port)
**Requirement:** R6(e) (root: Option<PortRef>)
**Problem:** SPEC-02 R6 defines `root: Option<PortRef>` -- the type allows both `AgentPort` and `FreePort` values. SPEC-02 Section 4.9 shows the example `net.root = Some(PortRef::AgentPort(root_agent, 0))` and says "the AgentPort connected to the external observation point," implying `AgentPort` is canonical. However:

- **SPEC-12 R56** explicitly says: "Both `AgentPort` and `FreePort` references are valid for `root` (FreePort is a valid use case for Lafont interface ports, cf. SPEC-14 R9 where `net.root = Some(FreePort(0))`)."
- **SPEC-14 R9** says the OPPOSITE: `net.root = Some(PortRef::AgentPort(lam_f, 0))` and explicitly states "This is NOT a `FreePort(0)` stored in the port array."

SPEC-12 R56 cites SPEC-14 R9 as evidence that `FreePort` roots are valid, but SPEC-14 R9 actually mandates `AgentPort` roots. The SPEC-12 citation is factually wrong.

**Impact if unresolved:** The implementer does not know whether `FreePort` roots are valid. If they are, `decode_nat` (SPEC-14) would fail on them (line 651: `PortRef::AgentPort(id, 0) => id, _ => return None`). If they are not, SPEC-12's Text DSL test T11c (`root free(0)` MUST set `net.root = Some(FreePort(0))`) would produce a root that downstream functions cannot use.
**Suggested resolution:** SPEC-02 MUST clarify the semantic constraints on `root`:
- Either: "root MUST be `Some(AgentPort(id, 0))` where `id` is a live agent, or `None`." (Matches SPEC-14's usage.)
- Or: "root MAY be `Some(FreePort(bid))` for Lafont interface nets, or `Some(AgentPort(id, p))` for standard nets." (Matches SPEC-12's intent but requires SPEC-14 `decode_nat` to handle FreePort roots.)

The cleanest resolution is to constrain root to `AgentPort` and fix SPEC-12 R56's erroneous citation.

---

### SC-007: `ERA` agents waste 2 port slots with DISCONNECTED -- interaction with assertions unclear
**Severity:** MEDIUM
**Axis:** Completeness | Testability
**Section:** 4.3 (Net Structure), 4.6 (Debug Assertions)
**Requirement:** R8, R20
**Problem:** SPEC-02 acknowledges that ERA agents waste 2 slots: "ERA wastes 2 slots (ports 1 and 2 are unused)." These unused slots are initialized to DISCONNECTED (via `ports.resize(required_len, DISCONNECTED)`). The resolved question in Section 7 confirms: "ERA agents occupy 3 slots in the port array but only use 1."

However, the debug assertions in Section 4.6 iterate over `0..total_ports(agent.symbol)`. For ERA, `total_ports(Era) = arity(Era) + 1 = 0 + 1 = 1`, so only port 0 is checked. The unused slots (ports 1 and 2) are never validated.

This is actually correct (validated by SPEC-12 R61 which explicitly requires arity-aware iteration). But the assertion `assert_adjacency_consistent` has a latent issue: if some code accidentally writes to ERA port slots 1 or 2, the assertion will not catch it. These slots could contain stale references to deleted agents, and nothing would detect the corruption.

**Impact if unresolved:** Stale data in ERA port slots 1-2 could cause subtle bugs if any code accidentally reads those slots (e.g., a generic "iterate all 3 ports" loop that does not check arity). Low probability but hard to diagnose.
**Suggested resolution:** Add a SHOULD-level assertion: "In debug mode, port array slots at indices `id*3+1` and `id*3+2` for ERA agents SHOULD contain DISCONNECTED. If they contain any other value, it indicates a bug in a prior operation."

---

### SC-008: `connect` with `FreePort` arguments does not detect redexes
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 4.5.4 (connect)
**Requirement:** R13
**Problem:** The `connect` implementation only detects redexes when BOTH ports are `AgentPort(_, 0)`:
```rust
if let (PortRef::AgentPort(id_a, 0), PortRef::AgentPort(id_b, 0)) = (a, b) {
    self.redex_queue.push_back((id_a, id_b));
}
```

If a reduction rule reconnects a port to a `FreePort(bid)`, no redex is detected. This is correct in the local context (FreePort boundaries have no partner until merge). But SPEC-05 R5 says the merge uses `Net::connect` to restore boundary connections. When merge calls `connect(AgentPort(a, 0), AgentPort(b, 0))`, redex detection works. So the overall system is correct.

However, SPEC-02 does not document this limitation of `connect`. A future reader might wonder why `connect(AgentPort(x, 0), FreePort(5))` does not create a redex even though port 0 is involved.

**Impact if unresolved:** Confusion for implementers. No functional bug.
**Suggested resolution:** Add a comment in R13 or the design section: "Redex detection only fires when BOTH endpoints are `AgentPort(_, 0)`. Connections involving `FreePort` never produce redexes; border redexes are detected during merge when `FreePort` sentinels are resolved to `AgentPort` endpoints (SPEC-05 R5)."

---

### SC-009: Root port behavior during reduction is unspecified
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 4.9 (Root Port)
**Requirement:** R6(e)
**Problem:** SPEC-02 specifies that `root: Option<PortRef>` stores the observation point. But it does not specify what happens to `root` when the root agent participates in a reduction:

1. If the root agent's principal port is connected to another agent's principal port (forming a redex), the root agent will be consumed by the reduction rule. After reduction, `net.root` still points to the consumed agent's ID. The root is now stale.
2. The reduction engine (SPEC-03) does not mention `root` at all -- no rule updates `net.root` after consuming the root agent.
3. For Church numeral arithmetic (SPEC-14), the initial net has its root set to a lambda agent. When beta-reduction occurs, this agent may be consumed. The resulting Normal Form has a different root structure.

In the Haskell prototype, the root was a FreePort in the wire list. When reduction reconnected ports, the FreePort naturally tracked the result because wires were re-linked. In Relativist's design, `root` is a field disconnected from the port array, so it is never automatically updated by `connect`/`disconnect`.

**Impact if unresolved:** After reducing a net whose root agent is consumed, `net.root` points to a dead agent. `decode_nat` and other readback functions would fail (return None) even though the net is correctly reduced. The user cannot extract the result.
**Suggested resolution:** SPEC-02 MUST address root tracking during reduction. Options:
(a) Document that `net.root` is set once at construction and is NOT updated by reduction. The readback algorithm must locate the result by other means (e.g., traversing remaining agents).
(b) Specify that reduction rules MUST update `net.root` when the root agent is consumed. This requires the reduction engine to check `net.root` during each rule application.
(c) Document that for Church numeral arithmetic nets, the root agent is NOT the initial redex (the root agent's principal port is DISCONNECTED, not connected to another principal port). Therefore the root agent is never consumed directly. Instead, the result is found by following `net.root -> agent -> auxiliary ports` after reduction. This is the actual behavior implied by SPEC-14's port tables, but it should be documented explicitly.

Option (c) is likely the correct interpretation but needs to be stated explicitly.

---

### SC-010: `BorderMap` type is defined but not part of `Net` struct
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 2 (Definitions), 4.10 (FreePort Storage)
**Requirement:** R6
**Problem:** SPEC-02 Section 2 defines `Border Map` and Section 4.10 defines the type alias `pub type BorderMap = std::collections::HashMap<u32, PortRef>;`. But the `Net` struct (R6) does not include a `BorderMap` field. Section 4.10 states it is "Maintained externally to the Net struct (in PartitionPlan, SPEC-04)."

This is architecturally sound (the Border Map is a partitioning concern, not a Net concern). However, defining the type alias in SPEC-02 but storing it in SPEC-04 creates confusion about ownership. Should `BorderMap` be defined in the `net` module (SPEC-02) or in the `partition` module (SPEC-04)?

SPEC-13 R5 places partitioning types in the `partition/` module. Having `BorderMap` defined in `net/` but used exclusively by `partition/` and `merge/` violates the principle of proximity.

**Impact if unresolved:** The type alias is in the wrong module. Minor organizational issue.
**Suggested resolution:** Move the `BorderMap` type alias definition to SPEC-04 (partition), which is where it is stored and used. SPEC-02 should only reference it as an external concept in Section 4.10, not define it.

---

### SC-011: `PartialEq` for `Net` is listed in backlog (TASK-0018) but not in spec requirements
**Severity:** MEDIUM
**Axis:** Completeness | Testability
**Section:** 3.6 (Serialization)
**Requirement:** R26
**Problem:** R26 states: "`deserialize(serialize(net)) == net` (structural equality)." This requires `Net` to implement `PartialEq`. The backlog (TASK-0018) lists "Implement PartialEq for Net" as a P1 task. But SPEC-02 does not have a requirement that `Net` MUST implement `PartialEq`. The `Net` struct definition in Section 4.3 derives `Debug, Clone, Serialize, Deserialize` but NOT `PartialEq` or `Eq`.

For `VecDeque<(AgentId, AgentId)>` (the redex queue), `PartialEq` is available. For `Vec<Option<Agent>>`, `PartialEq` requires `Agent: PartialEq` (which it derives). For `Vec<PortRef>`, `PartialEq` requires `PortRef: PartialEq` (which it derives). So `#[derive(PartialEq)]` on `Net` would work. But it is not specified.

Additionally, structural equality via `==` may not be the correct semantics for all comparisons. Two nets that are graph-isomorphic (same structure, different AgentIds) would NOT be equal under `PartialEq`. SPEC-08 defines `nets_isomorphic` for graph isomorphism comparison. `PartialEq` provides a stricter comparison (exact AgentId equality).

**Impact if unresolved:** R26 is untestable as written (no `==` on Net). The implementer must add `PartialEq` without spec backing, or use a different comparison method.
**Suggested resolution:** Add a requirement: "The `Net` struct MUST derive `PartialEq` (and `Eq` if applicable) to enable structural equality comparison for serialization round-trip tests (R26) and debug comparisons. Note: structural equality (`==`) requires identical AgentIds; for graph isomorphism, use `nets_isomorphic` (SPEC-08)."

---

### SC-012: `is_reduced` only checks queue emptiness, not actual Normal Form
**Severity:** LOW
**Axis:** Testability
**Section:** 4.5.7 (is_reduced)
**Requirement:** R16
**Problem:** R16 states: "`is_reduced` MUST return `true` if and only if the redex queue is empty (all redexes have been consumed or discarded)." The implementation comments acknowledge the gap: "the redex queue may contain stale entries. This function only checks whether the queue is empty. For rigorous verification [...], use drain_stale() + is_reduced()."

The problem is that `is_reduced` can return `false` when the net IS in Normal Form (because the queue has stale entries), or return `true` when the net is NOT in Normal Form (theoretically impossible if the incremental detection is correct, but there is no proof of this in the spec).

SPEC-05 R27 uses this function as the termination condition: "the net is in Normal Form (redex queue empty after `reduce_all`)." If `reduce_all` correctly drains all redexes and stale entries, this works. But the name `is_reduced` suggests it checks the net's semantic state, while it actually checks a queue implementation detail.

**Impact if unresolved:** Semantically misleading function name. Could cause test confusion: a test checking `is_reduced()` after manually constructing a net (without using `reduce_all`) would get incorrect results because the redex queue was not populated by `connect`.
**Suggested resolution:** (a) Rename to `is_queue_empty()` to clarify the semantics. OR (b) Add a note: "This function checks the redex queue, not the net's actual state. For a net constructed via the CRUD API with proper use of `connect` (which populates the redex queue incrementally), an empty queue reliably indicates Normal Form. For nets constructed by other means (e.g., deserialization, manual mutation), use `drain_stale()` or scan the net for active pairs."

---

### SC-013: Serialization format includes DISCONNECTED sentinels without documentation
**Severity:** LOW
**Axis:** Completeness
**Section:** 4.11 (Serialization Format)
**Requirement:** R24, R25, R27
**Problem:** R25 states the serialized format must be "self-contained." The port array may contain `DISCONNECTED` sentinels (for unused ERA ports, transient states, and the root agent's principal port). These are serialized as `PortRef::FreePort(u32::MAX)`.

A receiver deserializing the net will see `FreePort(u32::MAX)` in port array slots. Without knowledge of the DISCONNECTED convention, the receiver might interpret `u32::MAX` as a valid FreePort ID and attempt to resolve it in the Border Map.

**Impact if unresolved:** Interoperability issue if a non-Relativist consumer deserializes the format. Low risk for v1 (only Relativist reads its own format), but the format documentation should be complete per R25.
**Suggested resolution:** Add to Section 4.11: "The port array may contain the sentinel `FreePort(u32::MAX)` (DISCONNECTED) in slots for unused ERA ports (ports 1-2) and the root agent's principal port. Receivers MUST treat `FreePort(u32::MAX)` as an invalid/unconnected sentinel, not as a valid FreePort ID."

---

### SC-014: No requirement for `Net::with_capacity` to satisfy I3
**Severity:** LOW
**Axis:** Invariant Preservation
**Section:** 4.5.1 (Construction)
**Requirement:** R10
**Problem:** `Net::with_capacity(capacity)` pre-allocates vectors but sets `next_id: 0`. If a caller creates a net with capacity 100 and then manually sets `agents[50] = Some(agent)` without going through `create_agent`, `next_id` would still be 0, violating I3 ("next_id > max(active agent ids)").

This is a misuse scenario (callers should use `create_agent`, not direct mutation). But the `agents` field is `pub`, so direct mutation is allowed by the API.

**Impact if unresolved:** A caller who bypasses `create_agent` can violate I3. Debug assertions would catch this, but the API design invites the mistake.
**Suggested resolution:** Consider making `agents` and `ports` fields non-pub (private) and exposing them only through the defined CRUD operations. If pub access is needed for serialization or iteration, provide read-only accessors. Alternatively, add a note: "Direct mutation of `agents` or `ports` bypasses invariant checks and may violate I1-I3. Callers MUST use `create_agent`, `connect`, `disconnect`, and `remove_agent` for all mutations."

---

### SC-015: `connect` does not validate port index against agent arity
**Severity:** LOW
**Axis:** Invariant Preservation
**Section:** 4.5.4 (connect)
**Requirement:** R13, R19
**Problem:** `connect(a, b)` calls `set_port(a, b)` and `set_port(b, a)` without verifying that the port indices are valid for the agents' arities. For example, `connect(AgentPort(era_id, 2), AgentPort(con_id, 1))` would write to ERA's slot at index `era_id * 3 + 2`, even though ERA has arity 0 and only port 0 is valid.

R19 states: "Every reference `AgentPort(id, p)` in the port array MUST point to an existing agent with `p <= arity(agents[id].symbol)`." But `connect` does not enforce this precondition.

The debug assertion `assert_refs_valid` (Section 4.6) would catch this after the fact, but only in debug mode and only if the assertion is called.

**Impact if unresolved:** Invalid connections can be silently created. Debug assertions catch them post-hoc but not at the point of error.
**Suggested resolution:** Add a debug assertion in `connect` that validates both port references: "In debug mode, `connect(a, b)` SHOULD verify that both `a` and `b` reference valid ports (existing agents with port index within arity)."

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 2 |
| HIGH | 4 |
| MEDIUM | 4 |
| LOW | 5 |

## Mandatory (must fix before implementation)

- **SC-001:** Missing `get_agent` public API -- add as a formal requirement and design code
- **SC-002:** Root port as DISCONNECTED violates T1 -- formalize the exception or use a distinct sentinel
- **SC-003:** Self-loop policy unspecified -- clarify intra-agent connections vs same-port connections
- **SC-004:** `disconnect` with FreePort endpoints -- document the Border Map staleness behavior
- **SC-005:** Missing iteration/count APIs for live agents -- add `count_live_agents` and `live_agents` iterator
- **SC-006:** Root port type constraint unclear -- SPEC-12 vs SPEC-14 contradiction on FreePort roots

## Recommended (should fix)

- **SC-007:** ERA unused port slot assertion -- add SHOULD-level validation
- **SC-008:** `connect` with FreePort -- document redex detection limitation
- **SC-009:** Root port behavior during reduction -- document root tracking policy
- **SC-010:** `BorderMap` type alias in wrong module -- move to SPEC-04
- **SC-011:** `PartialEq` for Net needed but not required -- add requirement
- **SC-012:** `is_reduced` name is semantically misleading -- clarify or rename

---

## Checklist

### Consistency
- [x] Types match SPEC-00 glossary (Symbol, AgentId, PortRef, Agent, Net)
- [x] PortRef encoding matches SPEC-00 Sections 3.11, 6.1, 6.2
- [x] Invariant references (T1-T7, I1-I5) align with SPEC-01
- [ ] **FAIL:** Root port type unconstrained -- SPEC-12 allows FreePort, SPEC-14 requires AgentPort (SC-006)
- [ ] **FAIL:** Self-loop policy unspecified -- SPEC-14 allows, SPEC-12 rejects (SC-003)
- [x] Serialization format matches SPEC-06 R12 (types derive Serialize/Deserialize)
- [x] CRUD operations used consistently by SPEC-03 (connect, disconnect, create_agent, remove_agent)
- [x] Redex queue type matches SPEC-03's usage
- [x] Port array indexing consistent across all specs (`id * 3 + p`)

### Testability
- [x] R1-R6 (fundamental types): testable by type-level assertions and construction tests
- [x] R7-R10 (storage): testable by creating agents and verifying arena/port array state
- [x] R11-R17 (operations): testable by CRUD operation tests (SPEC-08 N1-N18)
- [x] R18-R21 (invariants): testable via debug assertions
- [ ] **PARTIAL:** R22-R27 (serialization): R26 requires `PartialEq` for Net which is not derived (SC-011)
- [x] All MUST requirements have corresponding test IDs in SPEC-08

### Completeness
- [ ] **FAIL:** No `get_agent` accessor -- needed by SPEC-03, SPEC-14, SPEC-01 (SC-001)
- [ ] **FAIL:** No `count_live_agents` or iteration API -- needed by SPEC-04, SPEC-05, SPEC-08, SPEC-09, SPEC-12 (SC-005)
- [ ] **FAIL:** Root port semantic constraints not documented -- what values are valid? (SC-006)
- [ ] **FAIL:** Self-loop policy not specified (SC-003)
- [ ] **PARTIAL:** `disconnect` FreePort behavior not documented (SC-004)
- [ ] **PARTIAL:** Root tracking during reduction not specified (SC-009)
- [x] All fundamental types defined (Symbol, AgentId, PortId, PortRef, Agent, Net)
- [x] All CRUD operations defined (create, remove, connect, disconnect, get_target)
- [x] Serialization strategy defined (serde + bincode)
- [x] Debug assertion strategy defined (R20-R21)
- [x] FreePort storage mechanism documented (Border Map)

### Invariant Preservation
- [ ] **FAIL:** T1 violated by root port DISCONNECTED (SC-002)
- [x] T1 (linearity/bidirectionality): enforced by connect/disconnect symmetry and debug assertions
- [x] I1 (bidirectional port array): implemented by R18 and assert_adjacency_consistent
- [x] I2 (reference validity): implemented by R19 and assert_refs_valid
- [x] I3 (ID monotonicity): implemented by R10 and assert_next_id_valid
- [x] I4 (redex queue validity): implemented by R17 and is_valid_redex
- [x] T2 (principal port interaction): connect correctly detects redexes on port 0
- [x] T5 (rule correctness): not directly enforced by SPEC-02 (delegated to SPEC-03)
