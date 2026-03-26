# SPEC-01: System Invariants

**Status:** Revised v2
**Depends on:** SPEC-00 (Glossary)
**Gray zones resolved:** Z1 (strong confluence local to distributed determinism)
**References consumed:** REF-001, REF-002, REF-003, REF-005, REF-013, REF-014, REF-018
**Discussions consumed:** DISC-001 v2, DISC-003 v2, DISC-004 v2
**Arguments consumed:** ARG-001 (central argument, P1-P6), ARG-002 (partitioning, C1-C3)
**Code analyses consumed:** AC-001, AC-002, AC-003, AC-004, AC-015

---

## 1. Purpose

This spec defines the formal invariants that Relativist MUST preserve across every execution, from the local reduction of a single net to distributed reduction across multiple workers. The invariants are the fundamental contract of the system: any component (reduction engine, partitioner, merge protocol, wire protocol) that violates an invariant produces incorrect results.

The invariants are organized in four layers:

- **Layer T (Theoretical):** Formal properties of Interaction Combinators (Lafont 1990/1997) that Relativist inherits by construction. Violating a Layer T invariant means the implementation is not a valid implementation of ICs.
- **Layer D (Distribution):** Properties that the distributed reduction protocol MUST satisfy so that strong confluence is preserved in the grid scenario. Derived from the P1-P5 formal argument framework (DISC-003 v2, ARG-001) and the C1-C3 partitioning conditions (DISC-004 v2, ARG-002).
- **Layer I (Implementation):** Properties specific to the Rust implementation that guarantee operational correctness. Derived from design decisions (AC-001, AC-015, confirmed technical decisions).
- **Layer G (Global):** The Fundamental Property -- the single end-to-end guarantee that the entire system exists to uphold.

---

## 2. Definitions

All terms used in this spec are defined in SPEC-00 (Glossary). The following cross-references are provided for convenience:

| Term | SPEC-00 Section | Key aspect for this spec |
|------|-----------------|--------------------------|
| Agent | 3.1 | Node in the interaction net; has Symbol, AgentId, ports |
| Principal Port | 3.8 | Port 0; agents interact ONLY through principal ports |
| Wire | 3.10 | Undirected edge connecting two ports |
| PortRef | 3.11 | Reference to a port: `AgentPort(AgentId, PortId)` or `FreePort(bid)` |
| Active Pair (Redex) | 3.13 | Two agents connected through their principal ports |
| Normal Form | 5.5 | Net with no active pairs |
| Strong Confluence | 5.1 | Diamond property at one step (REF-002, Proposition 1) |
| Terminating Net | 5.8 | Net that possesses a normal form; all reduction sequences terminate |
| FreePort (Lafont) | 6.1 | Interface port in the original theory |
| FreePort (Boundary) | 6.2 | Synthetic marker for cut connections between partitions |
| Border Redex | 6.7 | Active pair whose agents reside in different partitions |
| Conditions C1-C3 | 6.9 | Correctness conditions for partitioning |
| Isomorphism | 6.12 | Structural equivalence modulo agent ID renaming |
| Premises P1-P6 | 8.1-8.6 | Formal argument framework for distributed correctness |

---

## 3. Requirements

### 3.1. Theoretical Invariants (Layer T)

These invariants capture the formal properties of Interaction Combinators that Relativist inherits by construction. They correspond to the structural conditions underlying Premise P1 (SPEC-00 Section 8.1) and their consequences.

---

**T1. Port Linearity**

Every port of every live agent in the net MUST be connected to exactly one other port. There are no dangling ports (unconnected) and no fan-out ports (multiply connected).

- **Formal statement:** For every agent `a` in the net and every port index `p` in `0..=arity(a.symbol)`, there exists exactly one `PortRef q` such that `ports[agent_port(a.id, p)] == q` and `ports[port_index(q)] == agent_port(a.id, p)`.
- **Justification:** REF-001, p.95, Condition 1: "Inside a rule, each variable occurs exactly twice." REF-002, p.71 (net structure). Linearity is the structural condition that prevents pointer sharing, which is the basis of strong confluence (DISC-001 v2, Section 3a; ARG-001, P1).
- **How to verify:** Assertion that traverses all live agents and verifies that each port has exactly one entry in the port array and that the reverse entry is consistent (bidirectionality).
- **Consequence of violation:** If a port is dangling, the rule consuming it will produce undefined results. If a port has multiple connections, reducing one may corrupt the other, destroying strong confluence.
- **When to verify:** After each reduction step (in debug mode), after each merge, after each split.

---

**T2. Interaction Exclusively via Principal Ports**

An Active Pair exists if and only if two agents are connected through their principal ports (port 0 of both). Connections between auxiliary ports, or between a principal and an auxiliary port, do NOT form Active Pairs.

- **Formal statement:** `(a, b)` is an Active Pair if and only if `ports[agent_port(a.id, 0)] == agent_port(b.id, 0)`.
- **Justification:** REF-001, p.96: "Agents interact through their principal port only." REF-002, p.73: definition of "cut" (Active Pair). DISC-001 v2, Section 2b (locality depends on this restriction).
- **How to verify:** The redex detection function MUST verify that both endpoints are port 0. Unit tests for each combination of port indices.
- **Consequence of violation:** False redexes would be detected and rules applied to non-active pairs, producing spurious results. The resulting net would not be equivalent to a valid IC reduction.

---

**T3. Disjointness of Active Pairs**

Two distinct Active Pairs share no agents. If `(a, b)` and `(c, d)` are distinct Active Pairs, then `{a, b} intersection {c, d} = empty`.

- **Formal statement:** Follows directly from T2 and the uniqueness of the principal port. Each agent has exactly one principal port. If the principal port of `a` is connected to the principal port of `b`, then the principal port of `a` cannot simultaneously be connected to the principal port of `c` (by T1, linearity). Therefore, `a` participates in at most one Active Pair.
- **Justification:** REF-001, p.96; REF-002, p.73 (immediate consequence). DISC-001 v2, Section 2c: "each agent belongs to exactly one connection. There are no shared pointers." This property is the structural enabler of parallel reduction without conflicts (ARG-001, Passo 2).
- **How to verify:** When building the list of Active Pairs, verify that no `AgentId` appears in more than one pair. Can be done with a `HashSet<AgentId>` during iteration.
- **Consequence of violation:** If two Active Pairs shared an agent, reducing them "simultaneously" would cause a conflict: both rules would attempt to consume the shared agent. Strong confluence would cease to hold.

---

**T4. Strong Confluence**

For any net `mu` with two distinct Active Pairs `r1` and `r2`: if `mu` reduces in one step to `v` (by `r1`) and to `v'` (by `r2`), then there exists `xi` such that `v` reduces in one step to `xi` (by the residual of `r2`) and `v'` reduces in one step to `xi` (by the residual of `r1`).

- **Formal statement:** If `mu ->r1 v` and `mu ->r2 v'` with `r1 != r2`, then there exists `xi` such that `v ->r2' xi` and `v' ->r1' xi`, where `r1'` and `r2'` are the residuals of `r1` and `r2` respectively.
- **Justification:** REF-001, Proposition 1 (p.96, without self-interaction); REF-002, Proposition 1 (p.73, with self-interaction -- the proof that applies to ICs, since 3 of the 6 rules are self-interaction: CON-CON, DUP-DUP, ERA-ERA). DISC-001 v2, Section 2a (precise attribution of the proof). The proof depends on T1 (linearity), T2 (principal port interaction), and T3 (disjointness). This property IS Premise P1 of the formal argument framework (SPEC-00 Section 8.1; ARG-001, P1).
- **How to verify:** By construction, not by direct testing (enumerating all redex pairs is impractical). Guaranteed if T1, T2, and T3 are satisfied and the 6 reduction rules are correctly implemented (T5). Empirical verification is via the Fundamental Property (G1).
- **Consequence of violation:** Different reductions would lead to different results. The system would lose determinism. The Fundamental Property would be false.

---

**T5. Correctness of the 6 Interaction Rules**

Each interaction rule MUST produce the topology specified by Lafont (REF-002, Fig. 2, p.82). The 6 rules are:

| Rule | Active Pair | Result | Agent balance | Reference |
|------|-------------|--------|---------------|-----------|
| Annihilation (cross) | CON-CON | Auxiliary ports cross-connected: aux1(a)<->aux2(b), aux2(a)<->aux1(b) | -2 | REF-002 p.82 |
| Annihilation (parallel) | DUP-DUP | Auxiliary ports parallel-connected: aux1(a)<->aux1(b), aux2(a)<->aux2(b) | -2 | REF-002 p.82 |
| Void | ERA-ERA | No residue: both agents removed, no new wires | -2 | REF-002 p.82 |
| Commutation | CON-DUP | 2 new DUP + 2 new CON (crossed configuration) | +2 | REF-002 p.82 |
| Erasure (CON) | CON-ERA | 2 new ERA (one per auxiliary port of the CON) | 0 | REF-002 p.82 |
| Erasure (DUP) | DUP-ERA | 2 new ERA (one per auxiliary port of the DUP) | 0 | REF-002 p.82 |

- **Formal statement:** For each rule, the topology of the right-hand side (agents created, ports reconnected) MUST correspond exactly to Fig. 2 of REF-002 (p.82). The topology of each rule is detailed in AC-001 (section "The 6 Reduction Rules").
- **Justification:** REF-002, pp.71-72, 82. Rule correctness is a necessary condition for strong confluence (T4). DISC-001 v2, Section 2a. The cross vs. parallel asymmetry in annihilation is essential for universality (SPEC-00 Sections 3.3, 3.4).
- **How to verify:** Unit tests for each rule, verifying: (a) correct agents created/destroyed, (b) auxiliary ports reconnected in the correct topology, (c) invariant T1 (linearity) preserved after the rule.
- **Consequence of violation:** If a rule produces incorrect topology, the resulting net will not be isomorphic to the net Lafont specifies. Subsequent reductions will produce incorrect results. Strong confluence (T4) will be violated.

---

**T6. Uniqueness of Normal Form (for Terminating Nets)**

If a net `mu` is terminating (SPEC-00 Section 5.8), its Normal Form is unique: if `mu` reduces (in zero or more steps) to `N'` and to `N''`, both Normal Forms, then `N' = N''` (modulo isomorphism of agent IDs).

- **Formal statement:** Consequence of T4 (strong confluence). See REF-005, Lemma 2.1.
- **Justification:** REF-002, Corollary of Proposition 1; REF-005, Lemma 2.1 (Mackie & Pinto): "If N has normal form N', then N is strongly normalising" and "normal forms are unique." DISC-003 v2, Section 1.3.
- **Scope qualification:** This invariant holds exclusively for terminating nets. Non-terminating nets (REF-002, Figure 3) do not possess a normal form, and confluence alone is insufficient for determinism in non-terminating computations (REF-018, Arrighi et al.; DISC-003 v2, Perspective 3). Relativist operates exclusively with terminating nets (Premise P6, SPEC-00 Section 8.6).
- **How to verify:** Execute `reduce_all` with different reduction strategies (first redex, last redex, random) and compare normal forms by graph isomorphism (SPEC-00 Section 6.12).
- **Consequence of violation:** The system would produce non-deterministic results.

---

**T7. Invariance of Total Interaction Count (for Terminating Nets)**

The total number of reduction steps to reach Normal Form is invariant with respect to reduction strategy.

- **Formal statement:** If `mu` is terminating, then for any two reduction sequences that lead `mu` to Normal Form, both have the same length (number of steps).
- **Justification:** REF-003 (Taelin/HVM2, p.8): "the number of reductions is invariant to the order in which interaction rules are applied." Consequence of the diamond property (strong confluence): at any fork `mu -> v` and `mu -> v'`, convergence occurs in exactly one step. By induction over the length of the reduction sequence, any two paths to normal form have exactly the same length. DISC-003 v2, Section 1.3.
- **Scope qualification:** Like T6, this holds exclusively for terminating nets. For non-terminating nets, there is no total step count (reduction never terminates).
- **How to verify:** Count interactions in `reduce_all` with different strategies. The counters MUST be equal.
- **Consequence of violation:** If different strategies produced different interaction counts, the system's cost model would be invalid. Distributed overhead benchmarks would be unreliable.

---

### 3.2. Distribution Protocol Invariants (Layer D)

These invariants correspond to the premises P2-P4 of the formal argument framework (DISC-003 v2, ARG-001). Together with strong confluence (T4 = P1), they form the sufficient conditions for distributed determinism. They also incorporate the partitioning correctness conditions C1-C3 (DISC-004 v2, ARG-002).

---

**D1. Split/Merge Identity (Premise P2a, Conditions C1-C3)**

The operation of partitioning a net into sub-nets and then merging them without any reduction MUST produce a net isomorphic (SPEC-00 Section 6.12) to the original.

- **Formal statement:** For any net `mu` and any valid PartitionPlan `P`:
  ```
  merge(split(mu, P)) ~ mu
  ```
  where `~` denotes isomorphism (structural equality modulo ID renaming).
- **Justification:** DISC-003 v2, Section 2.2, Step 1. ARG-002, Part I, Steps 1-4 (full derivation). The split cuts border wires substituting endpoints with FreePort (Boundary) sentinels; the merge restores the original wires using the border IDs. If the round-trip is not the identity, the protocol has altered the net.
- **Requirements (derived from C1-C3, SPEC-00 Section 6.9; ARG-002, Q5):**
  - D1a. **(MUST -- C1, Complete agent coverage):** The allocation function sigma MUST assign each agent to exactly one worker. The sets A_0, ..., A_{n-1} MUST form a mathematical partition of A (disjoint and exhaustive). No agent is lost or duplicated.
  - D1b. **(MUST -- C2, Complete wire coverage):** Every wire in the net MUST be classified as either local (both endpoints in the same partition, preserved entirely) or border (endpoints in different partitions, generates a pair of FreePort (Boundary) sentinels). No wire is lost.
  - D1c. **(MUST -- C3, FreePort bijectivity):** For each border ID `bid`, there MUST exist exactly one FreePort(bid) in each of exactly two distinct partitions. The merge can reconnect unambiguously.
  - D1d. **(MUST):** No internal wire (both endpoints in the same partition) MUST be altered by the split.
  - D1e. **(MUST):** The split operation MUST be a pure function of the PartitionPlan and the net; it MUST NOT depend on external state.
- **How to verify:** Round-trip test: `assert(merge(split(net, plan)) ~ net)` for nets of various sizes and partition plans. Property-based testing with randomly generated nets.
- **Consequence of violation:** If `merge(split(mu)) != mu`, then agents or connections have been lost or spuriously created. Distributed reduction would operate on a different net from the original, producing incorrect results.

---

**D2. Equivalence of Local Reduction (Premise P2b)**

The reduction of an Active Pair entirely within a partition MUST produce the same topological changes as reducing that same pair in the global net.

- **Formal statement:** For any Active Pair `(a, b)` with `sigma(a) = sigma(b) = i`, reducing `(a, b)` in partition `mu_i` produces the same agents, wires, and reconnections as reducing `(a, b)` in the original net `mu`.
- **Justification:** By Locality (SPEC-00 Section 5.2; DISC-001 v2, Section 2b), reducing an Active Pair depends exclusively on the two agents and their immediate port connections. All of these are present in `mu_i` by construction: internal connections are local wires, and boundary connections are represented by FreePort (Boundary) sentinels that inherit the original connection identity (ARG-002, Part II, Steps 5-6; ARG-003, R3: FreePort references the wire, not the agent).
- **Requirements:**
  - D2a. **(MUST):** When a reduction rule reconnects ports, any port connected to a FreePort (Boundary) MUST correctly inherit the boundary connection. If a consumed agent had a boundary connection, the newly created agent MUST connect to the same FreePort(bid).
  - D2b. **(MUST):** Redexes internal to different partitions are disjoint by construction (no agent belongs to two partitions, by D1a/C1). The reduction engine MUST NOT assume any ordering between reductions in different partitions.
- **How to verify:** Construct a test net with known redexes. Partition it. Reduce internally. Merge. Compare the result to reducing the same redexes in the global net.
- **Consequence of violation:** Workers would reduce incorrectly. The merged net would not be isomorphic to the expected intermediate state of the global reduction.

---

**D3. Completeness of Border Redex Resolution (Premise P3)**

After each merge phase, ALL border redexes (Active Pairs whose agents were in different partitions) MUST be identified and resolved.

- **Formal statement:** Let `mu'` be the net after merge. Let `B = {(a, b) | (a, b) is an Active Pair in mu' and sigma(a) != sigma(b) before the merge}`. After the border redex resolution phase, all pairs in B MUST have been reduced.
- **Justification:** DISC-003 v2, Section 2.3. ARG-001, P3. ARG-003 (full proof). Strong confluence guarantees that the order of resolution is irrelevant (reducing internal redexes first, then border redexes, is a valid strategy -- ARG-001, Passo 4), but completeness is essential: an unresolved border redex is a "lost" reduction step.
- **Requirements:**
  - D3a. **(MUST):** Border redex detection MUST examine all wires that crossed partition boundaries (all pairs of restored FreePort (Boundary) sentinels).
  - D3b. **(MUST):** Each identified border redex MUST be reduced before the next round begins (or before the final result is returned).
  - D3c. **(MUST):** If reducing a border redex creates new border redexes (possible with the CON-DUP commutation rule, which creates 4 new agents that may inherit boundary connections), these emergent border redexes MUST be treated in the same round or in subsequent rounds. The protocol MUST iterate until no border redexes remain. (DISC-003 v2, Section 2.3, note on emergent border redexes; ARG-003, R2.)
  - D3d. **(MUST):** Detection of emergent border redexes: a new agent created by local reduction whose principal port is connected to a FreePort (Boundary) that, after merge, reconnects to the principal port of an agent in another partition, forms an emergent border redex. The merge protocol MUST detect these (DISC-003 v2, Perspective 5 analysis; ARG-003, R2b).
- **How to verify:** After the resolution phase, verify that no Active Pair in the merged net has agents that originated from different partitions. Test specifically with nets that generate emergent border redexes (via CON-DUP rule across boundaries).
- **Consequence of violation:** Unresolved border redexes mean the final result will contain Active Pairs that should have been reduced. The result will differ from the Normal Form of the original net.

---

**D4. ID Uniqueness After Distributed Reduction (Premise P4)**

When workers create new agents (via CON-DUP, CON-ERA, DUP-ERA rules), the generated IDs MUST be globally unique after the merge.

- **Formal statement:** Let `A_i` be the set of agents in partition `i` after local reduction. For all `i != j`: `{a.id | a in A_i} intersection {a.id | a in A_j} = empty`.
- **Justification:** DISC-003 v2, Section 2.2, Step 3. ARG-001, P4. IDs are arbitrary labels without semantic meaning (SPEC-00 Section 3.14, Section 6.12), but ID collisions would cause two distinct agents to be confused during merge, corrupting the net topology.
- **Requirements:**
  - D4a. **(MUST):** Each worker MUST generate IDs within a range that does not collide with the ranges of other workers.
  - D4b. **(SHOULD, recommended):** ID Space Partitioning (SPEC-00 Section 6.11): the total ID space (`u32`, ~4 billion positions) is statically divided among workers at partition time. Each worker receives a contiguous, exclusive range for bump allocation. This eliminates the need for post-reduction remapping (as `remapAllPartitions` does in the Haskell prototype -- AC-003).
  - D4c. **(MUST, if remapping is used instead of pre-allocation):** The remapping MUST be bijective: each old ID maps to exactly one new ID, and distinct new IDs correspond to distinct old IDs. The remapping MUST consistently update ALL references to the renamed ID: entries in the port array, the redex queue, and the agent's `id` field.
  - D4d. **(MUST):** The net resulting from merge with unique IDs MUST be isomorphic to the net that would result from reducing the same redexes in the original net (with "correct" IDs).
- **How to verify:** (a) After merge, verify that no duplicate IDs exist in the agent set. (b) Verify that every `AgentId` referenced in any port exists in the agent arena. (c) Round-trip test: reduce the same net sequentially (with "natural" IDs) and distributedly (with ID space partitioning), and compare normal forms by isomorphism.
- **Consequence of violation:** ID collision would cause the merge to confuse distinct agents, corrupting wires and creating false Active Pairs or losing real ones. Incorrect results and potential crashes.

---

**D5. Exclusive Agent Ownership**

Each live agent belongs to exactly one partition at any given moment. No agent exists simultaneously in two partitions.

- **Formal statement:** Given a net `mu` partitioned into `mu_0, ..., mu_{n-1}`, for every agent `a` in `mu`: there exists exactly one `i` such that `a in mu_i`. Equivalently, the allocation function sigma: A -> {0, ..., n-1} is total and well-defined.
- **Justification:** Consequence of D1a (C1, complete agent coverage with disjointness) and the linearity invariant (T1). If an agent existed in two partitions, its principal port would be duplicated, violating T1. DISC-001 v2, Section 2c: "Each agent belongs to exactly one connection. There are no shared pointers."
- **Requirements:**
  - D5a. **(MUST):** The partitioner MUST assign each agent to exactly one partition.
  - D5b. **(MUST):** During local reduction, a worker MUST NOT create agents with IDs that reference agents in other partitions.
  - D5c. **(MUST):** During border redex resolution, both agents of the pair MUST be reunited in a single context before applying the rule.
- **How to verify:** After split, verify that the union of partition agent sets equals the original net's agent set with no duplicates and no losses. `sum(|mu_i|) == |mu|` and `union(agents(mu_i)) == agents(mu)`.
- **Consequence of violation:** A duplicated agent in two partitions could be consumed twice (once in each worker), creating dangling ports. A lost agent in the split means an Active Pair will never be reduced.

---

**D6. Protocol Termination (Premise P5)**

For terminating nets, the round cycle of the grid protocol MUST terminate in a finite number of rounds.

- **Formal statement:** If `mu` is a Terminating Net, `run_grid(mu, n)` terminates in at most `R` rounds, where `R <= N` and `N` is the total number of reduction steps to reach Normal Form (invariant by T7).
- **Justification:** DISC-003 v2, Section 2.4. ARG-001, P5 (derived from P1+P2+P3+P4). The argument:
  1. The initial net is terminating (Premise P6, scope restriction).
  2. By strong confluence (T4, P1) and REF-005 Lemma 2.1, the total number of reductions to Normal Form is finite and invariant (T7).
  3. Split/merge/remap do not create or destroy redexes (D1, D4 -- P2, P4).
  4. In each round, at least one redex is reduced: either at least one is internal to some partition (reduced locally), or all are border redexes (resolved in the merge phase by D3 -- P3).
  5. Therefore, the number of remaining reductions decreases strictly at each round. The protocol terminates in at most N rounds.
- **In practice:** Each round reduces many redexes simultaneously (all internal redexes of all partitions + all border redexes), so the number of rounds is much smaller than N.
- **How to verify:** For known terminating test nets, verify that `run_grid` terminates. Monitor the redex count across rounds to confirm strict decrease.
- **Consequence of violation:** The grid protocol would loop indefinitely for a terminating net, indicating a bug in border redex detection (D3) or in the split/merge cycle (D1).

---

### 3.3. Implementation Invariants (Layer I)

These invariants are properties of Relativist's concrete data structures that guarantee the theoretical and distribution invariants are upheld at runtime.

---

**I1. Bidirectional Consistency of the Port Array**

The port array MUST be bidirectional and consistent: if `ports[p] == q`, then `ports[q] == p`.

- **Formal statement:** For every port index `p` in the port array such that `ports[p]` is defined: `ports[ports[p]] == p`.
- **Justification:** The port array is the concrete representation of T1 (linearity). An inconsistency (p points to q, but q does not point back to p) is a violation of linearity.
- **Relationship:** I1 implements T1.
- **How to verify:** Debug assertion that traverses all entries in the port array and verifies the reverse property.
- **Consequence of violation:** "Ghost" or "orphan" ports that corrupt reduction rules.

---

**I2. Reference Validity**

Every reference `AgentPort(id, port)` in the port array or the redex queue MUST point to an existing agent with a valid port index.

- **Formal statement:** For every reference `AgentPort(id, port)` in the port array: `agents[id]` is `Some(agent)` and `port <= arity(agent.symbol)`. For every `(a, b)` in the redex queue: `agents[a]` is `Some(_)` and `agents[b]` is `Some(_)`.
- **Justification:** Dangling references are semantic undefined behavior (the rule would attempt to access a nonexistent agent).
- **Relationship:** I2 is a precondition for T5 (rule correctness).
- **How to verify:** Debug assertion after each reduction. In release mode, absence of panic when accessing `Vec<Option<Agent>>` (no `None` where `Some` is expected).
- **Consequence of violation:** Panic in debug mode; silent incorrect results in release mode.

---

**I3. Monotonicity of AgentIds**

The `next_id` field of Net MUST be strictly greater than any `AgentId` currently in use. IDs are never reused.

- **Formal statement:** For every agent `a` in the net: `a.id < net.next_id`. After each operation that creates agents, `next_id` is incremented by the number of agents created.
- **Justification:** AC-001, "Design Decisions" section. Monotonically increasing IDs prevent reuse and simplify distribution (each worker receives a disjoint range -- SPEC-00 Section 6.11, ID Space Partitioning).
- **Relationship:** I3 is a precondition for D4 (ID uniqueness).
- **How to verify:** Assertion that `next_id > max(active agent ids)` after each operation.
- **Consequence of violation:** ID collision, which violates D4 (ID uniqueness) in the distributed scenario and corrupts the port array in the local scenario.

---

**I4. Redex Queue Validity**

Every pair `(a, b)` in the redex queue MUST correspond to a real Active Pair when it is dequeued for reduction: both agents exist and are connected through their principal ports.

- **Formal statement:** For every `(a, b)` at the moment of dequeue: `agents[a]` is `Some(_)`, `agents[b]` is `Some(_)`, and `ports[agent_port(a, 0)] == agent_port(b, 0)`.
- **Justification:** The redex queue is an incremental optimization (AC-015 CC-2) that replaces `findRedexes`. If the queue contains invalid entries (agents already consumed, or pairs no longer connected), the reduction engine would attempt to apply rules to nonexistent pairs.
- **Relationship:** I4 is a precondition for T2 (redexes involve principal ports).
- **Design note:** Redexes can become stale when an agent has been consumed by another reduction. The reduction engine MUST tolerate stale redexes in the queue: verify validity at dequeue time and silently discard stale entries. **(MUST)**
- **How to verify:** In debug mode, before reducing a dequeued redex, verify that it is still valid. Count discarded stale redexes as a diagnostic metric.
- **Consequence of violation:** Application of a rule to nonexistent agents (panic or silent corruption).

---

**I5. Termination of Local Reduction**

For terminating nets, `reduce_all` MUST terminate in finite time. For non-terminating nets (out of scope but defensively handled), `reduce_n` MUST respect the step budget.

- **Formal statement:** If `mu` is terminating, `reduce_all(mu)` returns after `N` steps, where `N` is the unique interaction count (by T7). If `mu` is not terminating, `reduce_n(mu, budget)` returns after exactly `min(budget, available)` steps, where `available` is the number of redexes found up to the budget.
- **Justification:** REF-002, Fig. 3 (p.76) shows non-terminating nets. REF-005, Lemma 2.1: strong confluence implies that nets with normal form are strongly normalizing (every reduction sequence terminates). AC-001: `reduceN` exists as a "safety valve."
- **Relationship:** I5 implements T6 (uniqueness via termination).
- **How to verify:** For known terminating nets, verify that `reduce_all` returns. For known non-terminating nets, verify that `reduce_n(budget)` returns after exactly `budget` steps.
- **Consequence of violation:** Infinite loop in `reduce_all` for a non-terminating net (expected, documented limitation -- Premise P6 restricts scope to terminating nets). A real violation would be `reduce_all` not terminating for a terminating net, indicating a bug in the rule implementation.

---

### 3.4. Fundamental Property (Layer G)

**G1. Equivalence Between Local and Distributed Reduction**

```
reduce_all(net) ~ extract_result(run_grid(net, n))
```

for any Terminating Net `net` and any number of workers `n >= 1`, where `~` denotes isomorphism (SPEC-00 Section 6.12: structural equality modulo agent ID renaming).

- **Formal statement:** Let `mu` be a Terminating Net. Let `NF = reduce_all(mu)` be the Normal Form obtained by sequential reduction. Let `NF_d = run_grid(mu, n)` be the result of distributed reduction with `n` workers. Then `NF ~ NF_d`.
- **Justification:** This is the central thesis of the TCC (OBJETIVO_TCC.md). The logical chain, as established in DISC-003 v2 (Section 3.1) and synthesized in ARG-001:
  - T4 (strong confluence = P1) guarantees that reduction order is irrelevant.
  - D1 (split/merge identity = P2a, via C1-C3) guarantees that partitioning and merging does not alter the net.
  - D2 (local reduction equivalence = P2b) guarantees that workers reduce correctly.
  - D3 (border redex completeness = P3) guarantees that all redexes are eventually reduced.
  - D4 (ID uniqueness = P4) guarantees that merge produces a net isomorphic to the expected one.
  - D5 (exclusive ownership) guarantees no agent duplication across partitions.
  - D6 (protocol termination = P5) guarantees the cycle converges.
  - P6 (scope restriction to terminating nets) is the precondition for T6, T7, and the proof by induction (ARG-001, Section "Induction Proof").
- **How to verify:** This is Relativist's fundamental test. For each test case: (a) construct the net, (b) reduce sequentially with `reduce_all`, (c) reduce distributedly with `run_grid` for various values of `n`, (d) compare normal forms by graph isomorphism.
- **Consequence of violation:** The TCC hypothesis would be refuted. Distributed reduction would not preserve IC determinism.

---

## 4. Design

### 4.1. Invariant Dependency Hierarchy

```
T1 (Linearity) --------+
                        |
T2 (Principal Port) ----+----> T3 (Disjointness) ----> T4 (Strong Confluence = P1)
                        |                                       |
T5 (6 Rules) -----------+                                      |
                                                                |
T6 (Unique Normal Form) <--------------------------------------+-- [for Terminating Nets, P6]
T7 (Invariant Step Count) <------------------------------------+-- [for Terminating Nets, P6]
                                                                |
D1 (Split/Merge Identity = P2a + C1-C3) ----+                  |
D2 (Local Reduction Equiv = P2b) -----------+                   |
D3 (Border Redex Completeness = P3) --------+----> G1 (Fundamental Property)
D4 (ID Uniqueness = P4) -------------------+                   |
D5 (Exclusive Ownership) ------------------+                   |
D6 (Protocol Termination = P5) ------------+-------------------+
                                                                |
I1 (Bidirectional Port Array) --------> T1 (implements linearity)
I2 (Reference Validity) --------------> T5 (precondition for rules)
I3 (Monotonic IDs) -------------------> D4 (precondition for uniqueness)
I4 (Redex Queue Validity) ------------> T2 (precondition for redexes)
I5 (Local Termination) ---------------> T6 (implements uniqueness)
```

### 4.2. Mapping to Formal Argument Framework (P1-P6)

This table maps each invariant to the premises P1-P6 established in DISC-003 v2 and ARG-001, and to the conditions C1-C3 from ARG-002. This ensures that the invariants are both necessary and sufficient for the Fundamental Property.

| Premise (ARG-001) | Invariant(s) | How satisfied |
|--------------------|-------------|---------------|
| P1 (Lafont's Conditions) | T1, T2, T3, T4, T5 | By construction: correct implementation of the 6 rules |
| P2a (split/merge = identity) | D1 (via C1-C3) | Empirical verification via round-trip tests |
| P2b (local reduction = global) | D2 | By construction: locality (T1, T2) + rule correctness (T5) |
| P2c (remap preserves isomorphism) | D4c | By construction: bijective remapping (or eliminated by ID space partitioning, D4b) |
| P2d (merge after reduction is correct) | D2a, D3d | Design: FreePort (Boundary) references wires, not agents (ARG-003, R3) |
| P3 (boundary completeness) | D3 | Design: iterate until no border redexes remain |
| P4 (ID consistency) | D4 | ID Space Partitioning (pre-allocation of ranges per worker) |
| P5 (protocol termination) | D6 | Consequence of P1+P2+P3+P4 for terminating nets |
| P6 (terminating nets only) | T6, T7 scope qualifications | Scope decision (OBJETIVO_TCC.md) |

### 4.3. Debug Assertions in Rust

The implementation MUST include verifiable assertions for implementation invariants (Layer I) and, optionally, for theoretical invariants (Layer T) in debug mode.

```rust
/// Verifies invariant I1: bidirectional port array.
/// MUST be called after each reduction in debug mode.
#[cfg(debug_assertions)]
fn assert_ports_consistent(net: &Net) {
    for (idx, &target) in net.ports.iter().enumerate() {
        if target != NONE {
            assert_eq!(
                net.ports[port_index(target)],
                idx_to_port_ref(idx),
                "I1 violated: ports[{:?}] = {:?}, but ports[{:?}] != {:?}",
                idx, target, target, idx
            );
        }
    }
}

/// Verifies invariant I2: all references point to existing agents.
#[cfg(debug_assertions)]
fn assert_refs_valid(net: &Net) {
    for (idx, &target) in net.ports.iter().enumerate() {
        if let Some((agent_id, port_id)) = decode_agent_port(target) {
            assert!(
                net.agents.get(agent_id as usize)
                    .is_some_and(|slot| slot.is_some()),
                "I2 violated: reference to nonexistent agent {:?}",
                target
            );
            let agent = net.agents[agent_id as usize].as_ref().unwrap();
            assert!(
                port_id <= arity(agent.symbol),
                "I2 violated: port {} exceeds arity of {:?}",
                port_id, agent.symbol
            );
        }
    }
}

/// Verifies invariant I3: next_id > max(active IDs).
#[cfg(debug_assertions)]
fn assert_next_id_valid(net: &Net) {
    for (i, slot) in net.agents.iter().enumerate() {
        if slot.is_some() {
            assert!(
                (i as u32) < net.next_id,
                "I3 violated: agent with id {} >= next_id {}",
                i, net.next_id
            );
        }
    }
}
```

### 4.4. Failure Model

Relativist operates under an ideal scenario (no network or worker failures), as specified in OBJETIVO_TCC.md. The invariants assume:

- **Reliable TCP:** Messages arrive intact, in order, without loss. Guaranteed by TCP under the no-failure scenario.
- **Cooperative workers:** No worker sends corrupted data or acts maliciously.
- **Sufficient memory:** No worker exhausts memory during reduction (nets that expand via CON-DUP may require significant memory).
- **Clock irrelevance:** Strong confluence makes temporal ordering irrelevant. There is no dependency on a global clock or temporal synchronization. As Lafont states: "time is relativistic" (REF-002, p.70).

Fault tolerance is OUT OF SCOPE (Z5, per OBJETIVO_TCC.md; Premise P6 scope restriction). If any of the above assumptions is violated (e.g., TCP connection interrupted), behavior is undefined.

---

## 5. Rationale

### 5.1. Why These Invariants and Not Others

The selection of invariants follows three principles:

1. **Theoretical closure:** Invariants T1-T7 capture all properties of ICs that the TCC utilizes. T1-T3 are the structural premises of the strong confluence proof. T4 is the central property (= P1). T5 is the rule correctness condition. T6-T7 are consequences for terminating nets (qualified by P6). There is no additional IC property needed for the TCC argument.

2. **Distribution sufficiency:** Invariants D1-D6 correspond exactly to premises P2-P5 of the formal argument (DISC-003 v2, ARG-001), plus the partitioning conditions C1-C3 (ARG-002). DISC-003 v2, Section 3.1, argues that P1 + P2 + P3 + P4 are sufficient for G1; therefore T4 + D1 + D2 + D3 + D4 (+ D5 as precondition for D1, + D6 as consequence of P1-P4) are sufficient for G1.

3. **Implementation grounding:** Invariants I1-I5 are the concrete implementation properties that ensure T1-T7 and D1-D6 hold at runtime. Each Layer I invariant maps to exactly one Layer T or Layer D invariant (see dependency hierarchy in Section 4.1).

### 5.2. Alternatives Considered and Rejected

- **"Constant number of free ports" invariant:** Rejected. Free ports (FreePort Boundary) are temporary artifacts of partitioning, not a permanent net property. Their count varies between phases.

- **"Net size only decreases" invariant:** Rejected. The CON-DUP commutation rule INCREASES the number of agents (balance +2). Net size can grow before converging to Normal Form.

- **"Fixed number of rounds" invariant (strong protocol determinism):** Rejected. The number of rounds depends on partitioning and the distribution of border redexes. Strong confluence guarantees determinism of the RESULT, not the PATH. DISC-003 v2, Perspective 6.

- **Separate invariants for each C1-C3 condition:** Integrated into D1 instead, since C1-C3 are sub-requirements of the split/merge identity. This avoids proliferation and maintains the clear mapping to P2a.

### 5.3. Scope Qualification: Terminating Nets

Invariants T6, T7, D6, and G1 are explicitly qualified with "for terminating nets" (P6). This qualification is essential because:

- Arrighi et al. (REF-018, Abstract) argue that "confluence alone is too weak a property" for non-terminating distributed computations.
- Non-terminating nets (REF-002, Figure 3) have no Normal Form, so uniqueness of result is not meaningful.
- The TCC scope (OBJETIVO_TCC.md) explicitly restricts to terminating nets.

The DISC-003 v2 (Perspective 3) reconciliation: for terminating nets, strong confluence IS sufficient (the P1-P5 framework applies fully). For non-terminating nets, the Arrighi et al. framework would be needed as an extension, which is out of scope.

---

## 6. Haskell Prototype Reference

### 6.1. Invariants Implicitly Assumed by the Prototype

The Haskell prototype (AC-001) implicitly assumes all T1-T7 invariants but does NOT verify them:

- **T1 (Linearity):** Assumed but not enforced. `mkNet` and `addWire` do not validate that each port has exactly one connection (AC-001, Limitation L5).
- **T2 (Principal Ports):** Correctly implemented in `findRedexes` (checks `AgentPort _ 0` for both endpoints).
- **T5 (6 Rules):** Implemented in 4 functions (`ruleCON_CON`, `ruleDUP_DUP`, `ruleERA_ERA`/`ruleErase`) with symmetric pair normalization (AC-001, pattern T2).
- **T7 (Invariant count):** Not verified in the prototype. No interaction counter exists.

### 6.2. Distribution Invariants D1-D6 in the Prototype

- **D1 (split/merge identity):** Implemented in IC.Partition via `FreePort`. The prototype tests the round-trip empirically (110 datapoints, 0 failures -- AC-005).
- **D2 (local reduction equivalence):** Implicit; no explicit verification.
- **D3 (border redexes):** Implemented in the `go` loop of IC.Grid, which iterates while `findRedexes` returns a non-empty list.
- **D4 (ID uniqueness):** Implemented via `remapAllPartitions` with per-worker offsets. `nextAgentId` guarantees local monotonicity.
- **D5 (exclusive ownership):** Implemented by contiguous ID ranges in partitioning.
- **D6 (protocol termination):** Implicit; the grid loop terminates when `findRedexes` returns empty.

### 6.3. What Relativist Changes

1. **Explicit verification:** Relativist will include assertions for invariants I1-I5, absent in the prototype. In debug mode, each reduction will be verified.
2. **Optimized representation:** The prototype uses `[Wire]` (wire list). Relativist will use a flat port array (`Vec<PortRef>`), making I1 verifiable in O(n) instead of O(w^2).
3. **Incremental redex queue:** The prototype rescans all wires to find redexes (AC-001, L1). Relativist maintains an incremental queue, which requires I4 (queue validity) and supports stale entry detection.
4. **Pre-allocated IDs:** The prototype performs post-reduction remapping (`remapAllPartitions`). Relativist will pre-allocate ID ranges per worker (SPEC-00 Section 6.11), avoiding remapping entirely and simplifying D4.
5. **Explicit FreePort distinction:** Relativist will distinguish FreePort (Lafont) from FreePort (Boundary) in the type system (SPEC-00 Sections 6.1, 6.2; DISC-004 v2, Section 1.4), preventing confusion between interface ports and boundary sentinels.

---

## 7. Open Questions

1. **Graph isomorphism comparison in G1:** The Fundamental Property compares Normal Forms "modulo isomorphism of IDs." Which comparison algorithm to use? Options: (a) ID canonicalization (renumber sequentially by BFS) and direct comparison, (b) graph structure hashing, (c) counting + topology verification. Option (a) is simplest and sufficient for small/medium nets. Decision deferred to the implementer (SPEC-08 will specify test strategy).

2. **Assertion verification frequency:** In debug mode, verifying I1+I2 after EACH reduction may be costly for large nets. It SHOULD be possible to configure the frequency (e.g., every N reductions, or only at the end). Decision deferred to the implementer.

3. **Non-terminating net handling in the grid protocol:** I5 covers `reduce_n` as a safeguard, but the grid protocol MUST have a round limit to prevent infinite looping for accidentally non-terminating nets. The round limit will be specified in SPEC-05 (merge protocol) or SPEC-07 (deployment). Not yet decided.
