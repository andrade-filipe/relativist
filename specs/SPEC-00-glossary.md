# SPEC-00: Canonical Glossary

**Status:** Revised v2
**Depends on:** ---
**Gray zones resolved:** --- (terminology foundation; enables resolution of Z1-Z7 in subsequent specs)
**References consumed:** REF-001, REF-002, REF-003, REF-005, REF-007, REF-013, REF-014, REF-017, REF-018
**Discussions consumed:** DISC-001 v2, DISC-003 v2, DISC-004 v2, DISC-005 v2
**Code analyses consumed:** AC-001, AC-002, AC-003, AC-004, AC-005, AC-015

---

## 1. Purpose

This spec defines the canonical vocabulary for the Relativist project: all technical terms, their formal definitions, the notation used, and the mapping between the academic terminology (Lafont 1990/1997), the Haskell reference prototype, and the Relativist implementation in Rust. This glossary is normative for all other specs.

## 2. Conventions

- **Canonical name** is always in English, as the source code and all specs use English.
- **Greek notation** (gamma, delta, epsilon) is used only in academic/formal context. In code and technical specs, use CON, DUP, ERA.
- **Alias** indicates alternative names found in the literature or in the Haskell prototype.
- **Ref** indicates the primary academic reference or code analysis for the definition.
- Terms marked with **(Relativist)** are specific to Relativist's architecture and do not appear in the original literature.
- Terms marked with **(Framework P1-P5)** refer to the formal argument framework established in DISC-003 v2 and ARG-001.
- RFC 2119 keywords (MUST, SHOULD, MAY) in definitions indicate normative requirements for the implementation.

---

## 3. Domain 1 -- Interaction Combinator Theory

### 3.1 Agent

| Field | Value |
|-------|-------|
| **Canonical name** | Agent |
| **Alias** | Cell (REF-002), Node (colloquial) |
| **Definition** | A labeled vertex in an interaction net. Each agent has exactly one Symbol (which determines its behavior), a unique AgentId, a principal port (port 0), and zero or more auxiliary ports (ports 1..arity). |
| **Ref** | REF-001 p.95 ("agents"), REF-002 p.71 ("cells"), AC-001 |
| **Haskell** | `data Agent = Agent { agentSymbol :: Symbol, agentId :: AgentId }` |
| **Rust (proposed)** | `struct Agent { symbol: Symbol, id: AgentId }` |
| **Related to** | Symbol, Port, AgentId, Net |

### 3.2 Symbol

| Field | Value |
|-------|-------|
| **Canonical name** | Symbol |
| **Alias** | Label, Type (REF-001) |
| **Definition** | The label of an Agent that determines its behavior and arity. In the Interaction Combinators system, there are exactly 3 symbols: CON (gamma), DUP (delta), ERA (epsilon). The symbol determines which interaction rules the agent participates in. |
| **Ref** | REF-002 p.71-72, AC-001 |
| **Haskell** | `data Symbol = CON \| DUP \| ERA` |
| **Rust (proposed)** | `enum Symbol { Con, Dup, Era }` |
| **Related to** | Agent, Interaction Rule, Arity |

### 3.3 CON (gamma / Constructor)

| Field | Value |
|-------|-------|
| **Canonical name** | CON |
| **Alias** | gamma, Constructor |
| **Formal notation** | gamma (Lafont) |
| **Definition** | One of the 3 universal symbols. Arity 2 (2 auxiliary ports + 1 principal port = 3 ports total). Constructs structures in the net. In CON-CON annihilation, auxiliary ports are reconnected in CROSS pattern. In CON-DUP commutation, creates 2 new CON and 2 new DUP agents. |
| **Ref** | REF-002 p.82 (Fig. 2), AC-001 |
| **Related to** | DUP, ERA, Annihilation, Commutation |

### 3.4 DUP (delta / Duplicator)

| Field | Value |
|-------|-------|
| **Canonical name** | DUP |
| **Alias** | delta, Duplicator |
| **Formal notation** | delta (Lafont) |
| **Definition** | One of the 3 universal symbols. Arity 2 (2 auxiliary ports + 1 principal port = 3 ports total). Duplicates structures in the net. In DUP-DUP annihilation, auxiliary ports are reconnected in PARALLEL (straight) pattern. In CON-DUP commutation, creates 2 new CON and 2 new DUP agents. |
| **Ref** | REF-002 p.82 (Fig. 2), AC-001 |
| **Note** | The asymmetry between CON-CON (cross) and DUP-DUP (parallel) in annihilation is essential for the universality of the system (REF-002 p.90). |
| **Related to** | CON, ERA, Annihilation, Commutation |

### 3.5 ERA (epsilon / Eraser)

| Field | Value |
|-------|-------|
| **Canonical name** | ERA |
| **Alias** | epsilon, Eraser |
| **Formal notation** | epsilon (Lafont) |
| **Definition** | One of the 3 universal symbols. Arity 0 (0 auxiliary ports, only 1 principal port). Erases structures in the net. When interacting with CON or DUP, it propagates erasure by creating 2 new ERA agents connected to the auxiliary ports of the partner. In ERA-ERA interaction (void), both are removed without creating anything. |
| **Ref** | REF-002 p.82 (Fig. 2), AC-001 |
| **Related to** | CON, DUP, Erasure, Void |

### 3.6 Arity

| Field | Value |
|-------|-------|
| **Canonical name** | Arity |
| **Alias** | --- |
| **Definition** | The number of auxiliary ports of an Agent. CON and DUP have arity 2. ERA has arity 0. The total number of ports of an agent is arity + 1 (the principal port). |
| **Ref** | REF-001 p.95, REF-002 p.71 |
| **Related to** | Agent, Symbol, Port |

### 3.7 Port

| Field | Value |
|-------|-------|
| **Canonical name** | Port |
| **Alias** | --- |
| **Definition** | A connection point of an Agent. Each agent has exactly one principal port (port 0) and zero or more auxiliary ports (ports 1, 2, ..., arity). Ports are the endpoints of Wires. |
| **Ref** | REF-001 p.95-96, REF-002 p.71 |
| **Haskell** | Represented implicitly in `PortRef = AgentPort AgentId PortId` |
| **Rust (proposed)** | Identified by `(AgentId, PortId)` where `PortId` is `u8` with values 0, 1, or 2 |
| **Related to** | Principal Port, Auxiliary Port, Wire, PortRef |

### 3.8 Principal Port

| Field | Value |
|-------|-------|
| **Canonical name** | Principal Port |
| **Alias** | Port 0, Active Port |
| **Definition** | The distinguished port of each Agent, with index 0. Interactions (reductions) only occur between two agents connected through their principal ports. This restriction guarantees that two distinct Active Pairs are always disjoint, because each agent has exactly one principal port. |
| **Ref** | REF-001 p.96, REF-002 p.71-73 |
| **Related to** | Port, Auxiliary Port, Active Pair, Locality |

### 3.9 Auxiliary Port

| Field | Value |
|-------|-------|
| **Canonical name** | Auxiliary Port |
| **Alias** | Port 1, Port 2 |
| **Definition** | Any port of an Agent other than the principal port. CON and DUP have 2 auxiliary ports (ports 1 and 2). ERA has no auxiliary ports. Auxiliary ports do not participate directly in the formation of Active Pairs; they serve as reconnection points during reduction. |
| **Ref** | REF-001 p.95, REF-002 p.71 |
| **Related to** | Port, Principal Port, Interaction Rule |

### 3.10 Wire

| Field | Value |
|-------|-------|
| **Canonical name** | Wire |
| **Alias** | Edge, Connection (REF-001) |
| **Definition** | An undirected edge connecting exactly two Ports. The linearity property guarantees that each port is the endpoint of at most one wire. Wires are the connectivity elements of the graph. |
| **Ref** | REF-001 p.95, AC-001 |
| **Haskell** | `data Wire = Wire PortRef PortRef` |
| **Rust (proposed)** | Implicit representation via flat port array (confirmed decision: `Vec<Option<Agent>>` + flat port array). Each slot in the port array stores the PortRef of the target port. |
| **Related to** | Port, PortRef, Net |

### 3.11 PortRef

| Field | Value |
|-------|-------|
| **Canonical name** | PortRef |
| **Alias** | Port Reference |
| **Definition** | A reference to a specific port in the net. Can be an agent port (`AgentPort`: identified by AgentId + PortId) or a free port (`FreePort`: identified by an integer index). PortRef is the fundamental type for describing the graph topology. |
| **Ref** | AC-001, AC-002 |
| **Haskell** | `data PortRef = AgentPort AgentId PortId \| FreePort Int` |
| **Rust (proposed)** | Compact encoding: `u32` with `(val << TAG_BITS) \| tag`, where tag distinguishes CON/DUP/ERA/VAR (AC-015 CC-1) |
| **Related to** | Port, Wire, AgentId, FreePort (Lafont), FreePort (Boundary) |

### 3.12 Net

| Field | Value |
|-------|-------|
| **Canonical name** | Net |
| **Alias** | Interaction Net, Network (colloquial) |
| **Definition** | The complete graph of a computation: a set of Agents connected by Wires, with possibly some Free Ports at the interface. Formally, a pair (A, W) where A is the set of agents and W is the set of wires (DISC-004 v2, Section 1.1; adapted from REF-013 p.219 "configuration"). A Net is the fundamental unit of computation in Interaction Combinators. Reduction transforms a Net into another until it reaches Normal Form. |
| **Ref** | REF-001 p.95, REF-002 p.71-73, AC-001, REF-013 p.219 |
| **Haskell** | `data Net = Net { netAgents :: Map AgentId Agent, netWires :: [Wire] }` |
| **Rust (proposed)** | `struct Net { agents: Vec<Option<Agent>>, ports: Vec<PortRef>, redex_queue: VecDeque<(AgentId, AgentId)> }` (confirmed decision) |
| **Related to** | Agent, Wire, Active Pair, Normal Form, Reduced Net |

### 3.13 Active Pair (Redex)

| Field | Value |
|-------|-------|
| **Canonical name** | Active Pair |
| **Alias** | Redex (reducible expression), Alive Pair (REF-001), Cut, Reducible Cut (REF-002) |
| **Definition** | A pair of Agents connected through their Principal Ports (port 0 <-> port 0). It is the atomic unit of computation: each reduction step consists of applying the corresponding Interaction Rule to one Active Pair. Fundamental property: two distinct Active Pairs are always disjoint (because each agent has exactly one principal port), which enables parallel reduction without conflicts. |
| **Ref** | REF-001 p.96 ("alive pair"), REF-002 p.73 ("cut"), AC-001 |
| **Haskell** | `data Redex = Redex AgentId AgentId` (invariant: `aid1 < aid2`) |
| **Rust (proposed)** | Tuple `(AgentId, AgentId)` in the redex queue (`VecDeque<(AgentId, AgentId)>`) |
| **Related to** | Principal Port, Interaction Rule, Reduction Step, Redex Queue |

### 3.14 AgentId

| Field | Value |
|-------|-------|
| **Canonical name** | AgentId |
| **Alias** | Node ID |
| **Definition** | Unique identifier for an Agent within a Net. In Relativist, AgentIds are `u32`, monotonically increasing, never reused within a single execution. In the distributed context, the ID space is statically partitioned among workers to avoid collisions without requiring remapping. IDs are arbitrary labels without semantic meaning; graphs that differ only in agent IDs are isomorphic (ARG-001, P4; DISC-003 v2, Section 2.2, Step 3). |
| **Ref** | AC-001, AC-015 CC-4 |
| **Haskell** | `type AgentId = Int` |
| **Rust (proposed)** | `type AgentId = u32` (confirmed decision) |
| **Related to** | Agent, PortRef, ID Space Partitioning |

---

## 4. Domain 2 -- Interaction Rules

### 4.1 Interaction Rule

| Field | Value |
|-------|-------|
| **Canonical name** | Interaction Rule |
| **Alias** | Reduction Rule, Rewrite Rule |
| **Definition** | A rewrite rule specifying how an Active Pair is transformed. For each ordered pair of Symbols, there exists at most one rule (no-ambiguity condition). In Interaction Combinators, there are exactly 6 rules covering all possible pairs of {CON, DUP, ERA}. Each rule removes the two agents of the Active Pair and reconnects or creates agents according to the specific rule. |
| **Ref** | REF-001 p.96, REF-002 p.82 (Fig. 2) |
| **Related to** | Active Pair, Reduction Step, Annihilation, Commutation, Erasure |

### 4.2 Reduction Step

| Field | Value |
|-------|-------|
| **Canonical name** | Reduction Step |
| **Alias** | Interaction (HVM2/HVM3), Rewrite Step |
| **Definition** | The application of a single Interaction Rule to a single Active Pair, transforming the Net. A reduction step: (1) identifies the two agents of the Active Pair and their neighbors, (2) removes the two agents and their wires, (3) creates new agents and/or wires according to the rule, (4) reconnects the neighbors. It is a constant-time operation (O(1)) due to Locality. |
| **Ref** | REF-001 p.96, REF-002 p.73 |
| **Related to** | Active Pair, Interaction Rule, Locality |

### 4.3 Annihilation (gamma-gamma, delta-delta)

| Field | Value |
|-------|-------|
| **Canonical name** | Annihilation |
| **Alias** | --- |
| **Definition** | Class of interaction rule that occurs when two agents with the SAME symbol interact. Both agents are removed and their auxiliary ports are reconnected directly. There are two sub-rules with distinct topology: |
| **Sub-rules** | |
| | **CON-CON (Annihilate Cross):** Auxiliary ports are reconnected in CROSS pattern: aux1(a) <-> aux2(b) and aux2(a) <-> aux1(b). |
| | **DUP-DUP (Annihilate Parallel):** Auxiliary ports are reconnected in PARALLEL pattern: aux1(a) <-> aux1(b) and aux2(a) <-> aux2(b). |
| **Effect on Net** | Removes 2 agents, creates 0 agents. Decreases the net size. |
| **Ref** | REF-002 p.82 (Fig. 2), AC-001, AC-015 CC-5 |
| **Note** | The cross vs. parallel asymmetry is essential for universality. |
| **Related to** | CON, DUP, Active Pair, Commutation |

### 4.4 Commutation (gamma-delta)

| Field | Value |
|-------|-------|
| **Canonical name** | Commutation |
| **Alias** | --- |
| **Definition** | Interaction rule that occurs when CON and DUP interact (different symbols, both of arity 2). Both original agents are removed. 4 new agents are created: 2 CON and 2 DUP, reconnected in a crossed configuration. This is the ONLY rule that INCREASES the number of agents in the net (from 2 to 4, net gain of +2). |
| **Effect on Net** | Removes 2 agents, creates 4 agents. Increases the net size. |
| **Ref** | REF-002 p.82 (Fig. 2), AC-001, AC-015 CC-5 |
| **Related to** | CON, DUP, Active Pair, Annihilation |

### 4.5 Erasure (gamma-epsilon, delta-epsilon)

| Field | Value |
|-------|-------|
| **Canonical name** | Erasure |
| **Alias** | Erase |
| **Definition** | Class of interaction rule that occurs when ERA interacts with CON or DUP. The ERA agent and the arity-2 agent are removed. 2 new ERA agents are created, one connected to each auxiliary port of the removed agent. This propagates "erasure" through the net. The two sub-rules (CON-ERA and DUP-ERA) are topologically identical. |
| **Sub-rules** | |
| | **CON-ERA:** Removes CON and ERA, creates 2 new ERA at the auxiliary ports of the CON. |
| | **DUP-ERA:** Removes DUP and ERA, creates 2 new ERA at the auxiliary ports of the DUP. |
| **Effect on Net** | Removes 2 agents, creates 2 agents (constant size, but replaces 1 arity-2 agent with 2 arity-0 agents). |
| **Ref** | REF-002 p.82 (Fig. 2), AC-001, AC-015 CC-5 |
| **Related to** | ERA, CON, DUP, Active Pair |

### 4.6 Void (epsilon-epsilon)

| Field | Value |
|-------|-------|
| **Canonical name** | Void |
| **Alias** | ERA-ERA Annihilation |
| **Definition** | Interaction rule when two ERA agents interact. Both are removed without creating any new agent or wire. The simplest rule in the system. Technically a special case of Annihilation for arity-0 agents. |
| **Effect on Net** | Removes 2 agents, creates 0 agents, creates 0 wires. |
| **Ref** | REF-002 p.82, AC-001 |
| **Related to** | ERA, Annihilation |

---

## 5. Domain 3 -- Formal Properties

### 5.1 Strong Confluence

| Field | Value |
|-------|-------|
| **Canonical name** | Strong Confluence |
| **Alias** | Diamond Property (one-step) |
| **Definition** | Property of a rewrite system where, if a Net N reduces in one step to P and in one step to Q (by reductions of distinct Active Pairs, with P != Q), then P and Q each reduce in one step to a common Net R. Formally (REF-002, Proposition 1, p.73): "If a net mu reduces in one step to v and to v', with v != v', then v and v' reduce in one step to a common xi." |
| **Consequences** | (1) All reduction sequences terminate if one terminates (conditional strong normalization). (2) Normal forms are unique. (3) The total number of reduction steps is invariant with respect to reduction order (for terminating nets). (4) Distinct Active Pairs can be reduced in parallel without coordination. |
| **Proof basis** | Follows from: linearity (each variable occurs once on each side of the rule), binary interaction (only principal ports), and no-ambiguity (at most one rule per pair of symbols). Since distinct Active Pairs are disjoint, concurrent reductions never interfere with each other. The proof in REF-002 extends REF-001 to cover self-interaction (3 of the 6 IC rules are self-interaction: gamma-gamma, delta-delta, epsilon-epsilon). |
| **Ref** | REF-001 Proposition 1 p.96, REF-002 Proposition 1 p.73, REF-005 Lemma 2.1, DISC-001 v2 Section 2a, DISC-003 v2 Section 1.1 |
| **Related to** | Locality, Linearity, Normal Form, Determinism, Premises P1-P5 |

### 5.2 Locality

| Field | Value |
|-------|-------|
| **Canonical name** | Locality |
| **Alias** | Locality of Interactions |
| **Definition** | The property that each Reduction Step affects only the two agents of the Active Pair and their immediate connections. No information about the global state of the Net is needed to apply a rule. The reduction of one active pair does not duplicate, erase, or modify any other existing active pair in the net. Direct consequence: each reduction is a constant-time O(1) operation. |
| **Key quotes** | REF-001 p.96: "interactions are purely local and can be performed concurrently." REF-002 p.70: "By 'local synchronization,' we mean that there is no need to consider a global time for computation. In other words, time is relativistic." REF-013 p.219: "the part of the net being reduced can be cut out of the net, and the reduced net connected back in, independently of other possible reductions in the net." |
| **Ref** | REF-001 p.96, REF-002 p.70, REF-013 p.219, DISC-001 v2 Section 2b |
| **Related to** | Strong Confluence, Reduction Step, Principal Port, Partition |

### 5.3 Linearity

| Field | Value |
|-------|-------|
| **Canonical name** | Linearity |
| **Alias** | --- |
| **Definition** | The condition that, within an interaction rule, each variable (representing a context port) occurs exactly twice: once on the left-hand side and once on the right-hand side. Consequences: (1) no implicit duplication of sub-nets, (2) no implicit discarding of sub-nets, (3) no effects at a distance, (4) no garbage collector required. Duplication and erasure are explicit operations via DUP and ERA. Additionally, in the net structure, each port participates in exactly one wire (REF-001, p.95, Condition 1). |
| **Ref** | REF-001 p.95, REF-002, DISC-004 v2 Section 1.1 |
| **Related to** | Strong Confluence, Locality, DUP, ERA |

### 5.4 Universality

| Field | Value |
|-------|-------|
| **Canonical name** | Universality |
| **Alias** | --- |
| **Definition** | The property that any interaction system can be translated into the Interaction Combinators system (3 symbols, 6 rules), preserving complexity (up to a linear constant factor) and degree of parallelism. Formally (REF-002, Theorem 1, p.82): "Any interaction system can be translated into the system of interaction combinators." Analogously to how NAND gates are universal for logic circuits, CON/DUP/ERA are universal for interaction nets. |
| **Ref** | REF-002 Theorem 1 p.82, Proposition 4 (complexity preservation) |
| **Related to** | CON, DUP, ERA, Interaction Rule |

### 5.5 Normal Form

| Field | Value |
|-------|-------|
| **Canonical name** | Normal Form |
| **Alias** | Irreducible Net |
| **Definition** | A Net that contains no Active Pair. No Interaction Rule can be applied. The Normal Form is the final result of a computation. Strong Confluence guarantees that, if a Normal Form exists, it is unique (REF-005, Lemma 2.1). |
| **Ref** | REF-002 p.73, REF-005 Lemma 2.1 |
| **Related to** | Active Pair, Reduced Net, Strong Confluence |

### 5.6 Reduced Net

| Field | Value |
|-------|-------|
| **Canonical name** | Reduced Net |
| **Alias** | --- |
| **Definition** | A Net that contains no cut (Active Pair) and no vicious circle (closed principal path). A Reduced Net is necessarily in Normal Form, but a Net in Normal Form may contain irreducible cuts (pairs without a defined rule -- not applicable to pure IC, where all pairs have a rule). Every Reduced Net with n free ports can be uniquely decomposed into n trees and a wiring (REF-002, Proposition 2, p.79). |
| **Ref** | REF-002 p.77-79 |
| **Related to** | Normal Form, Active Pair, Vicious Circle |

### 5.7 Vicious Circle

| Field | Value |
|-------|-------|
| **Canonical name** | Vicious Circle |
| **Alias** | Principal Cycle |
| **Definition** | A closed path (cycle) in the Net that passes only through principal ports. A Vicious Circle prevents the net from achieving Reduced Net status, even if it contains no Active Pairs. Occurs when an agent is (directly or indirectly) connected to itself through its principal port. |
| **Ref** | REF-002 p.77 |
| **Related to** | Reduced Net, Normal Form, Principal Port |

### 5.8 Terminating Net

| Field | Value |
|-------|-------|
| **Canonical name** | Terminating Net |
| **Alias** | Strongly Normalizing Net |
| **Definition** | A Net that possesses a Normal Form. For terminating nets under strong confluence, ALL reduction sequences terminate and reach the same unique Normal Form (REF-005, Lemma 2.1). The total number of reduction steps is invariant across all strategies. Non-terminating nets exist in the IC system (REF-002, Figure 3) but are out of scope for this project. |
| **Ref** | REF-002 Figure 3, REF-005 Lemma 2.1, REF-018 Abstract, DISC-003 v2 Section 1.3 |
| **Scope note** | Relativist operates exclusively with terminating nets (OBJETIVO_TCC.md). Arrighi et al. (REF-018) argue that confluence alone is insufficient for non-terminating distributed computations. |
| **Related to** | Normal Form, Strong Confluence, Premises P1-P5 (P6) |

---

## 6. Domain 4 -- Distribution and Partitioning

### 6.1 FreePort (Lafont)

| Field | Value |
|-------|-------|
| **Canonical name** | FreePort (Lafont) |
| **Alias** | Free Port, Interface Port |
| **Definition** | A port that is not connected to any Agent within the Net. In a complete Net, free ports represent the external interface of the network -- they are the "inputs and outputs" (REF-001, p.95; REF-002, p.71). A net with n free ports is a function of n arguments. This is a concept from Lafont's original theory. |
| **Ref** | REF-001 p.95 ("free ports" / "interface"), REF-002 p.71, DISC-004 v2 Section 1.4 |
| **Distinction** | MUST NOT be confused with FreePort (Boundary). See Section 6.2 for the boundary variant. DISC-004 v2 explicitly distinguishes these two concepts. |
| **Related to** | Port, PortRef, Net |

### 6.2 FreePort (Boundary)

| Field | Value |
|-------|-------|
| **Canonical name** | FreePort (Boundary) |
| **Alias** | Boundary Sentinel, Border FreePort |
| **Definition** | **(Relativist)** A synthetic marker inserted during partitioning to represent a cut connection between partitions. When a wire between agent a (partition i) and agent b (partition j) is cut, each sub-net receives a FreePort(bid) where bid is a unique border identifier. A boundary FreePort is a reference to the *border wire* identified by bid, NOT to the specific agent that originally occupied that position (ARG-003, R3). This distinction is critical: when an agent is consumed by local reduction and replaced by new agents, the FreePort(bid) remains valid because it references the wire, not the agent. |
| **Ref** | DISC-004 v2 Section 1.4, AC-002, ARG-003 R3 |
| **Haskell** | Constructor `FreePort Int` of `PortRef` (same constructor used for both Lafont and Boundary free ports) |
| **Rust (proposed)** | Dedicated FPORT tag in the compact encoding, or sentinel value in the VAR space (see AC-015 Z7) |
| **Connection to Lafont FreePort** | A boundary FreePort *creates* a new free port in the sub-net. It is effectively a Lafont free port enriched with routing metadata (the borderId). See DISC-004 v2 Section 1.4 for the formal connection. |
| **Related to** | FreePort (Lafont), Port, PortRef, Partition, Boundary, Border Wire |

### 6.3 Partition

| Field | Value |
|-------|-------|
| **Canonical name** | Partition |
| **Alias** | Sub-net |
| **Definition** | A subset of the Net (agents + their internal connections) assigned to a Worker for local reduction. Formally, given an allocation function sigma: A -> {0, 1, ..., n-1}, partition i is the sub-net induced by A_i = {a in A \| sigma(a) = i}. Each Partition contains: (a) a subset of the Agents of the original Net, (b) all Wires whose two endpoints are agents of the partition (local wires), (c) FreePort (Boundary) sentinels where wires cross the partition boundary. The union of all partitions MUST cover all agents of the original Net (Condition C1). |
| **Ref** | AC-002, DISC-004 v2 Section 1.3 |
| **Haskell** | `data Partition = Partition { partId :: Int, partAgents :: Map AgentId Agent, partWires :: [Wire] }` |
| **Related to** | Net, Worker, FreePort (Boundary), Boundary, PartitionPlan, Allocation Function |

### 6.4 Allocation Function (sigma)

| Field | Value |
|-------|-------|
| **Canonical name** | Allocation Function |
| **Alias** | sigma, Partitioning Function |
| **Definition** | **(Relativist)** A function sigma: A -> {0, 1, ..., n-1} that assigns each agent to exactly one worker. The allocation function induces the partition sets A_0, ..., A_{n-1} which form a mathematical partition of A (disjoint and exhaustive). Different allocation functions produce different partitions, which may have different numbers of border wires and thus different performance characteristics, but ALL valid partitions (satisfying C1-C3) produce the same final result due to strong confluence. |
| **Ref** | DISC-004 v2 Section 1.3, ARG-002 Q2 |
| **Note** | The allocation function is defined in this project; it is not a concept from Lafont or Mackie. Mackie (REF-013, p.218): "the way a net is initially distributed over the system can have dramatic effects on the performance." |
| **Related to** | Partition, PartitionPlan, Conditions C1-C3 |

### 6.5 Boundary

| Field | Value |
|-------|-------|
| **Canonical name** | Boundary |
| **Alias** | Border, Frontier |
| **Definition** | The set of Wires that cross two distinct Partitions, i.e., wires whose two endpoints belong to Agents in different Partitions. Each Boundary wire is represented as a pair of FreePort (Boundary) sentinels (one in each Partition), sharing the same borderId. Boundaries are the communication points between Workers: when both endpoints of a boundary wire become Principal Ports, a Cross-Boundary Active Pair (Border Redex) is formed. |
| **Ref** | AC-002, AC-003, DISC-004 v2 Section 1.3-1.4 |
| **Haskell** | `planBorders :: Map Int (PortRef, PortRef)` in `PartitionPlan` |
| **Related to** | Partition, FreePort (Boundary), Border Redex, Merge, Border Wire |

### 6.6 Border Wire

| Field | Value |
|-------|-------|
| **Canonical name** | Border Wire |
| **Alias** | Boundary Wire, Cross-Partition Wire |
| **Definition** | **(Relativist)** A wire in the original Net whose two AgentPort endpoints belong to agents in different partitions (sigma(a1) != sigma(a2)). During partitioning, each border wire w = Wire(AgentPort(a1, p1), AgentPort(a2, p2)) is replaced by two wires with boundary FreePorts: Wire(AgentPort(a1, p1), FreePort(bid)) in partition i and Wire(AgentPort(a2, p2), FreePort(bid)) in partition j. The border map F: borderId -> (PortRef, PortRef) records the original correspondence. |
| **Ref** | DISC-004 v2 Section 1.4, ARG-002 Q3 |
| **Related to** | Boundary, FreePort (Boundary), Partition |

### 6.7 Border Redex

| Field | Value |
|-------|-------|
| **Canonical name** | Border Redex |
| **Alias** | Cross-Boundary Active Pair |
| **Definition** | **(Relativist)** An Active Pair whose two Agents reside in different Partitions. Cannot be reduced by any individual Worker, because each Worker only possesses one of the two agents. Resolution of Border Redexes requires a Merge phase where the Coordinator reunites boundary information and performs the reduction. Border Redexes arise from two distinct sources (ARG-003, R2): (a) **Pre-existing:** The partitioning cuts a wire that already connected two agents by their principal ports -- an artifact of the allocation function. (b) **Emergent:** Local reduction (especially CON-DUP commutation) creates new agents whose principal ports connect to boundary FreePorts, forming new cross-boundary active pairs. Emergent border redexes are only detectable after the next merge. |
| **Ref** | AC-002 (`findBorderRedexes`), DISC-003 v2 Section 2.3, DISC-005 v2 Section 1.3, ARG-003 R2 |
| **Haskell** | Detected by `findBorderRedexes` in `IC.Partition` |
| **Related to** | Active Pair, Boundary, Merge, Coordinator, Round |

### 6.8 PartitionPlan

| Field | Value |
|-------|-------|
| **Canonical name** | PartitionPlan |
| **Alias** | --- |
| **Definition** | **(Relativist)** The complete distribution plan for a Net among Workers. Contains: (a) the list of Partitions (one per Worker), (b) the Boundary map (which FreePort (Boundary) sentinels in each Partition correspond to the same original wire), (c) the AgentId range assigned to each Worker for generating new IDs (ID Space Partitioning). |
| **Ref** | AC-002 (concept), AC-015 CC-4 (ID space partitioning) |
| **Related to** | Partition, Boundary, Worker, ID Space Partitioning |

### 6.9 Conditions C1-C3

| Field | Value |
|-------|-------|
| **Canonical name** | Conditions C1-C3 |
| **Alias** | Partitioning Correctness Conditions |
| **Definition** | **(Relativist / Framework)** Three conditions that a partitioning MUST satisfy for correctness: |
| **C1 (Complete agent coverage)** | The sets A_0, ..., A_{n-1} form a partition (in the mathematical sense) of A. No agent is lost or duplicated. |
| **C2 (Complete wire coverage)** | Every wire in W is classified as either local (preserved entirely in one partition) or border (generates a pair of boundary FreePorts). No wire is lost. |
| **C3 (FreePort bijectivity)** | For each borderId bid, there exists exactly one FreePort(bid) in each of exactly two distinct partitions. The merge can reconnect unambiguously. |
| **Ref** | DISC-004 v2 Section 4.1, ARG-002 Q5 |
| **Related to** | Partition, Allocation Function, Merge |

### 6.10 Merge

| Field | Value |
|-------|-------|
| **Canonical name** | Merge |
| **Alias** | Recombination |
| **Definition** | **(Relativist)** The process of recombining the reduced Partitions from Workers back into a unified Net. Merge involves: (a) collecting the reduced sub-nets from each Worker, (b) substituting the FreePort (Boundary) sentinels with the original endpoints (restoring the border wires), (c) detecting and resolving Border Redexes. The Merge is executed by the Coordinator. The fundamental invariant is that merge(split(net)) = net (identity without reduction), as proven in ARG-002, Part I, Steps 1-4. |
| **Ref** | AC-002 (`mergePartitions`), AC-003, ARG-002 Part I |
| **Related to** | Partition, Boundary, Coordinator, Border Redex |

### 6.11 ID Space Partitioning

| Field | Value |
|-------|-------|
| **Canonical name** | ID Space Partitioning |
| **Alias** | ID Pre-allocation |
| **Definition** | **(Relativist)** Technique to avoid AgentId collisions in a distributed context. The total ID space (`u32`, ~4 billion positions) is statically divided among Workers. Each Worker receives a contiguous, exclusive range for generating new IDs via bump allocation. Example with 8 workers and 30 usable bits: Worker 0 = [0, 128M), Worker 1 = [128M, 256M), etc. Eliminates the need for post-reduction remapping (as `remapAllPartitions` does in the Haskell prototype). This ensures premise P4 (ID consistency) of the formal argument. |
| **Ref** | AC-015 CC-4, inspired by HVM4 (AC-011), ARG-001 P4 |
| **Haskell (reference)** | Absent; used `remapAllPartitions` as a workaround (AC-003) |
| **Related to** | AgentId, Worker, Partition, PartitionPlan |

### 6.12 Isomorphism of IC-nets

| Field | Value |
|-------|-------|
| **Canonical name** | Isomorphism |
| **Alias** | Structural Equivalence (modulo IDs) |
| **Definition** | **(Framework)** Two IC-nets mu = (A, W) and mu' = (A', W') are isomorphic (written mu ~ mu') if there exists a bijection phi: A -> A' that preserves symbols, arities, internal connections, and free ports (up to consistent renaming of free port IDs). If phi is the identity, the nets are structurally equal (mu = mu'). Agent IDs are arbitrary labels; graphs that differ only in IDs are isomorphic. This concept justifies the correctness of ID remapping (ARG-001, P4) and ID Space Partitioning. |
| **Ref** | DISC-004 v2 Section 1.2 |
| **Related to** | Net, AgentId, ID Space Partitioning |

---

## 7. Domain 5 -- Grid Infrastructure

### 7.1 Grid

| Field | Value |
|-------|-------|
| **Canonical name** | Grid |
| **Alias** | Computational Grid |
| **Definition** | The set of distributed computational resources (Nodes) organized to execute the distributed reduction of interaction nets. In Relativist's context, the Grid consists of 1 Coordinator and N Workers communicating via TCP. The Grid is the execution environment, not a data structure. |
| **Ref** | REF-017 (Foster et al. 2001), REF-007 (Casanova 2002) |
| **Related to** | Node, Coordinator, Worker |

### 7.2 Node

| Field | Value |
|-------|-------|
| **Canonical name** | Node |
| **Alias** | Machine, Host |
| **Definition** | A physical or virtual machine participating in the Grid. Each Node runs exactly one process: either the Coordinator or a Worker. Nodes communicate via TCP. Relativist assumes that all Nodes are reachable and operational during the entire execution (no fault tolerance -- Z5, out of scope). |
| **Ref** | REF-017, REF-007 |
| **Related to** | Grid, Coordinator, Worker |

### 7.3 Coordinator

| Field | Value |
|-------|-------|
| **Canonical name** | Coordinator |
| **Alias** | Master (avoid this alias in Relativist) |
| **Definition** | **(Relativist)** The central Node that orchestrates the distributed reduction. Responsibilities: (1) receive or construct the initial Net, (2) create the PartitionPlan and distribute Partitions to Workers, (3) collect results (reduced sub-nets) from Workers, (4) execute the Merge, (5) resolve Border Redexes, (6) decide whether re-partitioning and a new Round is needed, (7) declare termination when the Net reaches Normal Form. The Coordinator does NOT reduce Active Pairs internal to partitions; it only resolves Border Redexes. |
| **Ref** | AC-003 (`runCoordinator`), AC-004 |
| **Related to** | Worker, Grid, Merge, Round, PartitionPlan |

### 7.4 Worker

| Field | Value |
|-------|-------|
| **Canonical name** | Worker |
| **Alias** | Slave (avoid this alias in Relativist) |
| **Definition** | **(Relativist)** A Node that receives a Partition from the Coordinator, executes `reduce_all` on the local sub-net until reaching local Normal Form, and returns the reduced sub-net to the Coordinator. Each Worker operates independently: it does not communicate with other Workers during the reduction phase. The Worker can generate new AgentIds within its pre-allocated range (ID Space Partitioning) without coordination. |
| **Ref** | AC-003 (`runWorker`), AC-004 |
| **Related to** | Coordinator, Partition, reduce_all, ID Space Partitioning |

### 7.5 Round

| Field | Value |
|-------|-------|
| **Canonical name** | Round |
| **Alias** | Iteration, Cycle |
| **Definition** | **(Relativist)** A complete iteration of the distributed reduction cycle. Phases of a Round: (1) **Partition** -- the Coordinator divides the Net into Partitions, (2) **Distribute** -- the Coordinator serializes and sends each Partition to the respective Worker via TCP, (3) **Compute** -- each Worker reduces its Partition locally to local Normal Form, (4) **Collect** -- the Coordinator receives the reduced sub-nets from Workers, (5) **Merge** -- the Coordinator recombines the sub-nets and resolves Border Redexes. If Active Pairs still exist after the Merge, a new Round is initiated. If the Net is in Normal Form, the process terminates. |
| **Ref** | AC-004 (grid loop in `IC.Grid`/`IC.Network`) |
| **Termination** | The cycle terminates in a finite number of rounds for terminating nets (Premise P5). In each round, at least one redex is reduced. The total number of reductions is finite and invariant (DISC-003 v2, Section 2.4). |
| **Related to** | Coordinator, Worker, Partition, Merge, Normal Form |

### 7.6 Redex Queue (Redex Bag)

| Field | Value |
|-------|-------|
| **Canonical name** | Redex Queue |
| **Alias** | Redex Bag, RBag (HVM2), Interaction Queue |
| **Definition** | **(Relativist)** Data structure that maintains the known Active Pairs for incremental reduction. Instead of re-scanning the entire Net to find Active Pairs at each step (as the Haskell prototype does with `findRedexes`), the Redex Queue is updated incrementally: when a wire is created connecting two Principal Ports, the new Active Pair is inserted into the queue. Implemented as `VecDeque<(AgentId, AgentId)>` (confirmed decision). |
| **Ref** | AC-015 CC-2, inspired by HVM2 (AC-006, AC-007) |
| **Haskell (reference)** | Absent; used `findRedexes` with O(w) scan at each step (AC-001) |
| **Related to** | Active Pair, reduce_all, Net |

### 7.7 reduce_all

| Field | Value |
|-------|-------|
| **Canonical name** | reduce_all |
| **Alias** | reduceAll (Haskell) |
| **Definition** | The main reduction function. Given a Net, applies Reduction Steps repeatedly until no more Active Pairs exist (the Net reaches Normal Form). In Relativist, uses the Redex Queue to find the next Active Pair in O(1) amortized time, applies the corresponding Interaction Rule, and inserts any new Active Pairs created by the rule back into the queue. |
| **Ref** | AC-001 (`reduceAll`), AC-015 CC-2 |
| **Haskell** | `reduceAll :: Net -> Net` (linear scan for Active Pair at each step) |
| **Related to** | Reduction Step, Redex Queue, Normal Form |

### 7.8 GridMetrics

| Field | Value |
|-------|-------|
| **Canonical name** | GridMetrics |
| **Alias** | --- |
| **Definition** | **(Relativist)** Aggregate performance measurements collected during a distributed reduction execution. Includes per-round metrics: (a) partitioning time, (b) serialization time, (c) TCP transfer time + latency, (d) local reduction time per worker, (e) merge time, (f) border redex resolution time, (g) number of border redexes per round, (h) number of agents and wires per partition, (i) total rounds. Also includes aggregate metrics: total wall-clock time, total reduction steps (MUST equal sequential count by strong confluence invariance), speedup ratio (sequential time / distributed time). |
| **Ref** | AC-004 (grid metrics in `IC.Grid`), AC-005 (benchmark framework), ARG-004 V2 |
| **Related to** | Round, Coordinator, Benchmarks (SPEC-09) |

---

## 8. Domain 6 -- Formal Argument Framework (P1-P5)

This domain defines the premises of the formal argument for correctness of distributed reduction, as established in DISC-003 v2 and synthesized in ARG-001. These are not data types or runtime concepts, but logical premises that the implementation MUST satisfy.

### 8.1 Premise P1 (Lafont's Conditions)

| Field | Value |
|-------|-------|
| **Canonical name** | Premise P1 |
| **Alias** | Lafont's Conditions |
| **Definition** | **(Framework P1-P5)** The Interaction Combinators system satisfies the three structural conditions that guarantee strong confluence: (a) linearity, (b) binary interaction, (c) no-ambiguity. Consequence: any two disjoint active pairs commute. This is a proven theorem (REF-002, Proposition 1, p.73), not an implementation requirement. |
| **Ref** | REF-002 Proposition 1 p.73, DISC-003 v2 Section 1.1, ARG-001 P1 |
| **Status** | Proven (mathematical theorem). No implementation action required. |
| **Related to** | Strong Confluence, Locality, Linearity |

### 8.2 Premise P2 (Split/Reduce/Remap/Merge Correctness)

| Field | Value |
|-------|-------|
| **Canonical name** | Premise P2 |
| **Alias** | Protocol Correctness |
| **Definition** | **(Framework P1-P5)** The composition of the distribution protocol operations preserves isomorphism with the original net. Specifically: (a) split followed by merge without reduction is the identity: merge(split(mu)) = mu; (b) local reduction of an active pair entirely within a partition produces the same result as reducing that pair in the original net; (c) ID remapping is bijective and preserves all connections; (d) merge after reduction and remap correctly reconstructs border wires. |
| **Ref** | DISC-003 v2 Section 2.2, ARG-001 P2, ARG-002 (full proof) |
| **Status** | Condition on implementation. Verified empirically and by code inspection. |
| **Related to** | Merge, Partition, Isomorphism, Conditions C1-C3 |

### 8.3 Premise P3 (Boundary Resolution Completeness)

| Field | Value |
|-------|-------|
| **Canonical name** | Premise P3 |
| **Alias** | Boundary Completeness |
| **Definition** | **(Framework P1-P5)** All border redexes (both pre-existing and emergent) are identified and resolved in each round of the protocol. No border redex is lost. Emergent border redexes (created when local reduction, especially CON-DUP commutation, reconnects ports to boundary FreePorts) are captured in the subsequent round. |
| **Ref** | DISC-003 v2 Section 2.3, ARG-001 P3, ARG-003 (full proof) |
| **Status** | Condition on implementation. Verified empirically. |
| **Related to** | Border Redex, Merge, Round |

### 8.4 Premise P4 (ID Consistency)

| Field | Value |
|-------|-------|
| **Canonical name** | Premise P4 |
| **Alias** | ID Consistency |
| **Definition** | **(Framework P1-P5)** ID remapping is bijective and updates all references consistently. When two workers execute rules that create new agents (especially CON-DUP, which creates 4 agents), the ID scheme assigns globally unique IDs before merge, avoiding collisions. In Relativist, this is handled by ID Space Partitioning (static pre-allocation), eliminating the need for remapping entirely. |
| **Ref** | DISC-003 v2 Section 2.2 Step 3, ARG-001 P4 |
| **Status** | Condition on implementation. Solved by design (ID Space Partitioning). |
| **Related to** | AgentId, ID Space Partitioning, Isomorphism |

### 8.5 Premise P5 (Protocol Termination)

| Field | Value |
|-------|-------|
| **Canonical name** | Premise P5 |
| **Alias** | Termination |
| **Definition** | **(Framework P1-P5)** The round cycle of the grid protocol terminates in a finite number of rounds for terminating nets. This is not an independent premise: it follows from P1 (total number of reductions is finite and invariant for terminating nets), P2/P4 (split/merge/remap do not create or destroy redexes), and P3 (all border redexes are resolved, ensuring at least one redex is reduced per round). |
| **Ref** | DISC-003 v2 Section 2.4, ARG-001 P5, REF-005 Lemma 2.1 |
| **Status** | Consequence of P1+P2+P3+P4 for terminating nets. |
| **Related to** | Round, Normal Form, Terminating Net |

### 8.6 Premise P6 (Scope Restriction to Terminating Nets)

| Field | Value |
|-------|-------|
| **Canonical name** | Premise P6 |
| **Alias** | Termination Scope |
| **Definition** | **(Framework P1-P5)** The project works exclusively with terminating nets (nets that possess a normal form). Non-terminating nets exist in the IC system (REF-002, Figure 3) but are out of scope. Arrighi et al. (REF-018) argue that confluence alone is insufficient for non-terminating distributed computations; the restriction to terminating nets avoids this problem. |
| **Ref** | OBJETIVO_TCC.md, REF-018 Abstract, DISC-003 v2 Section 1.3 |
| **Status** | Scope decision. |
| **Related to** | Terminating Net, Strong Confluence |

---

## 8b. Domain 6 -- Encoding & Readback (SPEC-14)

### 8b.1 Church Numeral

| Field | Value |
|-------|-------|
| **Canonical name** | Church Numeral |
| **Alias** | Church encoding, Church natural |
| **Definition** | An IC net encoding of a natural number n as the lambda term lambda f. lambda x. f^n(x). Uses CON agents for lambda abstractions and applications, DUP agents for variable sharing, and ERA for erasure. Church numeral n (n >= 2) contains (n + 2) CON + (n - 1) DUP agents. The encoding is in Normal Form (zero redexes). |
| **Ref** | REF-002 (Lafont 1997, Section 4: universality), SPEC-14 |
| **Rust (proposed)** | `encode_nat(n: u64) -> Net` |
| **Related to** | Encoding, Decoding, Arithmetic Net |

### 8b.2 Encoding

| Field | Value |
|-------|-------|
| **Canonical name** | Encoding |
| **Alias** | -- |
| **Definition** | The process of translating a high-level value (natural number) or expression (arithmetic operation applied to operands) into an IC net. The resulting net contains redexes whose reduction computes the result. **(Relativist)** |
| **Ref** | SPEC-14 |
| **Related to** | Church Numeral, Decoding, Arithmetic Net |

### 8b.3 Decoding (Readback)

| Field | Value |
|-------|-------|
| **Canonical name** | Decoding |
| **Alias** | Readback |
| **Definition** | The process of interpreting a Church numeral IC net in Normal Form as a natural number. Performed by traversing the net structure from root, identifying the two lambda CON agents, and counting the application chain length. Inverse of Encoding. **(Relativist)** |
| **Ref** | SPEC-14 |
| **Rust (proposed)** | `decode_nat(net: &Net) -> Option<u64>` |
| **Related to** | Church Numeral, Encoding, Normal Form |

### 8b.4 Arithmetic Net

| Field | Value |
|-------|-------|
| **Canonical name** | Arithmetic Net |
| **Alias** | -- |
| **Definition** | An IC net constructed by composing Church numeral sub-nets with an arithmetic combinator (addition, multiplication, or exponentiation). When reduced to Normal Form via `reduce_all` (SPEC-03), the result is a Church numeral encoding the arithmetic result. Exhibits Profile B overhead behavior (expansion via CON-DUP commutation, then collapse via annihilation). **(Relativist)** |
| **Ref** | SPEC-14, SPEC-09 (Profile B) |
| **Rust (proposed)** | `build_add(a, b) -> Net`, `build_mul(a, b) -> Net`, `build_exp(a, b) -> Net` |
| **Related to** | Church Numeral, Encoding, Overhead Profile |

### 8b.5 Combinator

| Field | Value |
|-------|-------|
| **Canonical name** | Combinator |
| **Alias** | -- |
| **Definition** | A closed lambda term (no free variables) that implements an operation, encoded as an IC net fragment. In Relativist: `add = lambda m. lambda n. lambda f. lambda x. m f (n f x)`, `mul = lambda m. lambda n. lambda f. m (n f)`, `exp = lambda m. lambda n. n m`. Each combinator connects to operand sub-nets via application CON agents, introducing redexes that drive the computation. **(Relativist)** |
| **Ref** | SPEC-14 |
| **Related to** | Arithmetic Net, Church Numeral |

---

## 9. Mapping Table: Lafont --> Haskell --> Rust

This table serves as a quick reference for translating between the three nomenclature layers.

| Lafont (1990/1997) | Haskell (Prototype) | Rust (Relativist) |
|--------------------|---------------------|-------------------|
| gamma | `CON` / `Symbol.CON` | `Symbol::Con` |
| delta | `DUP` / `Symbol.DUP` | `Symbol::Dup` |
| epsilon | `ERA` / `Symbol.ERA` | `Symbol::Era` |
| cell | `Agent` | `Agent` |
| -- | `AgentId` (`Int`) | `AgentId` (`u32`) |
| port | `PortRef` | `PortRef` (encoding `u32`) |
| principal port | port 0 | port 0 |
| auxiliary port | port 1, 2 | port 1, 2 |
| free port (interface) | `FreePort Int` | FPORT tag or VAR sentinel |
| -- (boundary FreePort) | `FreePort Int` (border) | `FreePort` (boundary sentinel) |
| wire / connection | `Wire PortRef PortRef` | Implicit (flat port array) |
| net / interaction net | `Net { netAgents, netWires }` | `Net { agents, ports, redex_queue }` |
| active pair / alive pair / cut | `Redex AgentId AgentId` | `(AgentId, AgentId)` |
| annihilation (gamma-gamma) | `ruleCON_CON` | `interact_anni` (cross) |
| annihilation (delta-delta) | `ruleDUP_DUP` | `interact_anni` (parallel) |
| commutation (gamma-delta) | `ruleCON_DUP` | `interact_comm` |
| erasure (gamma-epsilon) | `ruleErase` | `interact_eras` |
| erasure (delta-epsilon) | `ruleErase` | `interact_eras` |
| void (epsilon-epsilon) | `ruleErase` (ERA case) | `interact_void` |
| reduced net / normal form | `findRedexes net == []` | `redex_queue.is_empty()` |
| strong confluence | (property, not a type) | (property, not a type) |
| -- | `Partition` | `Partition` |
| -- | `FreePort Int` (border) | `FreePort` (boundary sentinel) |
| -- | `PartitionPlan` | `PartitionPlan` |
| -- | `findBorderRedexes` | Border Redex detection |
| -- | `mergePartitions` | `merge` |
| -- | `runCoordinator` | `Coordinator` |
| -- | `runWorker` | `Worker` |
| -- | -- | `GridMetrics` |

---

## 10. Fundamental Property of Relativist

The glossary as a whole serves a single property that Relativist MUST guarantee:

```
reduce_all(net) ~ run_grid(net, n)
```

Where:
- `reduce_all(net)` is the local sequential reduction of the Net to Normal Form.
- `run_grid(net, n)` is the distributed reduction with n Workers, executing Rounds until the Net reaches Normal Form.
- `~` denotes graph isomorphism: structural equality modulo renaming of AgentIds (IDs are arbitrary labels).

This property is enabled by **Strong Confluence** (which guarantees that reduction order does not matter -- P1) and **Locality** (which guarantees that each reduction is independent), conditioned on the implementation satisfying **Premises P2-P5**.

---

## 11. Open Questions

None. Domain 6 (Encoding & Readback) was added by amendment for SPEC-14. Additional terms may be introduced by future specs, provided they are registered here by amendment.
