# SPEC-02: Net Representation

**Status:** Revised v3
**Depends on:** SPEC-00 (Glossary), SPEC-01 (Invariants)
**Gray zones resolved:** ---
**References consumed:** REF-001, REF-002, REF-003, REF-013
**Discussions consumed:** DISC-001 v2, DISC-004 v2
**Arguments consumed:** ARG-001 (P1-P6 framework), ARG-002 (partitioning, C1-C3)
**Code analyses consumed:** AC-001, AC-006, AC-009, AC-015

---

## 1. Purpose

This spec defines the in-memory representation of an Interaction Combinator net in Relativist: the fundamental types (Symbol, AgentId, PortRef, Agent, Net), the storage structures (agent arena, flat port array, redex queue), the CRUD operations on the net, the invariants the representation must satisfy, and the serialization format for transmission between nodes. Every design decision is justified with reference to code analyses (AC-001, AC-006, AC-009, AC-015), discussions (DISC-004 v2), and formal arguments (ARG-001, ARG-002).

## 2. Definitions

Terms defined in SPEC-00 (Glossary) are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **Port Array** | A flat linear array where each slot stores the `PortRef` to which a port is connected. The port of agent `id` at slot `p` lives at index `id * 3 + p`. Replaces the `[Wire]` list of the Haskell prototype (AC-001) and the dual `node[]`/`vars[]` buffers of HVM2 (AC-006). |
| **Agent Slot** | A position in the `Vec<Option<Agent>>` indexed by `AgentId`. `Some(agent)` indicates a live agent; `None` indicates a free slot (agent removed or never created). |
| **Stale Redex** | An entry in the redex queue whose agents have already been consumed by another reduction, or whose principal port connection has changed. Stale redexes MUST be silently discarded by the reduction engine (cf. SPEC-01, I4). |
| **Root Port** | The port representing the final result of the computation. Implemented as an `Option<PortRef>` field in the Net struct that points to the external observation point of the net. |
| **Border Map** | A `HashMap<u32, PortRef>` that maps `FreePort(bid)` identifiers to the `AgentPort` they are connected to. Required because FreePort (Boundary) sentinels have no slot in the port array. Used by the partitioner (SPEC-04) and merger (SPEC-05). |

---

## 3. Requirements

### 3.1 Fundamental Types

**R1.** The `Symbol` type MUST be an enum with exactly 3 variants: `Con`, `Dup`, `Era`, corresponding to the 3 universal symbols of Lafont (REF-002, p.71-72). **(MUST)**

**R2.** The `AgentId` type MUST be `u32`, monotonically increasing, never reused within an execution (cf. SPEC-01, I3). **(MUST)**

**R3.** The `PortId` type MUST be `u8` with values in the range `[0, 2]`: 0 for the principal port, 1 and 2 for auxiliary ports. **(MUST)**

**R4.** The `PortRef` type MUST represent a reference to a port in the net. It MUST distinguish between agent ports (`AgentPort(AgentId, PortId)`) and boundary free ports (`FreePort(u32)`). **(MUST)**
- Note on FreePort disambiguation: the `FreePort(u32)` variant represents both Lafont free ports (interface ports of the original net) and Boundary free ports (synthetic markers inserted during partitioning). The distinction is semantic, not structural: Lafont free ports belong to the original net; Boundary free ports are created by the split operation and carry a `borderId` for merge resolution (SPEC-00 Sections 6.1 and 6.2; DISC-004 v2 Section 1.4). A single `FreePort(u32)` variant suffices because both are resolved the same way at the type level.

**R5.** The `Agent` type MUST contain at least the fields `symbol: Symbol` and `id: AgentId`. **(MUST)**

**R6.** The `Net` type MUST contain: (a) an agent arena `Vec<Option<Agent>>`, (b) a port array `Vec<PortRef>`, (c) a redex queue `VecDeque<(AgentId, AgentId)>`, (d) an ID generator `next_id: AgentId`, (e) an optional root port `root: Option<PortRef>`. **(MUST)**

**R6a.** The `root` field MUST be constrained to the following valid values: `None` (the net has no root, e.g., partition sub-nets), or `Some(AgentPort(id, 0))` where `id` is a live agent in the arena. `FreePort` values are NOT valid for `root`. The root observation point is represented by an explicit field precisely to avoid conflation with boundary `FreePort` sentinels (Section 5.6). SPEC-12 R56's erroneous citation of SPEC-14 R9 as evidence for `FreePort` roots is incorrect; SPEC-14 R9 explicitly mandates `AgentPort` roots. SPEC-12 T11c (`root free(0)`) is a Text DSL syntax test that sets `net.root = Some(FreePort(0))`; this is a parser-level operation before validation. The SPEC-12 parser MUST reject `FreePort` roots during T1/I2 validation (R11) or document the exception as a Lafont interface net construct. **(MUST)**

### 3.2 Storage

**R7.** Agents MUST be stored in a `Vec<Option<Agent>>` indexed by `AgentId`. The index in the vector MUST correspond to the `AgentId` of the agent. **(MUST)**

**R8.** Connections (wires) MUST be represented implicitly by a port array of size `agents.len() * 3`, where slot `id * 3 + port_id` stores the `PortRef` to which port `(id, port_id)` is connected. The adjacency map MUST be bidirectional (cf. SPEC-01, T1 and I1). **(MUST)**

**R9.** The redex queue MUST be a `VecDeque<(AgentId, AgentId)>` updated incrementally: new redexes are inserted when a `connect` operation creates a connection between two principal ports. **(MUST)**

**R10.** The field `next_id` MUST be strictly greater than any `AgentId` in use in the net (cf. SPEC-01, I3). After creating `k` agents, `next_id` MUST be incremented by `k`. Direct mutation of `agents` or `ports` bypasses invariant checks and may violate I1-I3. Callers MUST use `create_agent`, `connect`, `disconnect`, and `remove_agent` for all mutations. **(MUST)**

### 3.3 Operations

**R11.** The `create_agent` operation MUST create a new agent with the next available ID, insert it into the agent arena (expanding if necessary), and return the assigned `AgentId`. Expected complexity: O(1) amortized. **(MUST)**

**R12.** The `remove_agent` operation MUST mark the agent's slot as `None`, disconnect all its ports from the port array, and NOT reuse the ID. Expected complexity: O(1). **(MUST)**

**R13.** The `connect(a: PortRef, b: PortRef)` operation MUST establish a bidirectional connection between two ports in the port array. If both ports are principal ports of agents (`AgentPort(_, 0)`), it MUST insert the pair into the redex queue. Expected complexity: O(1). **(MUST)**

**R14.** The `disconnect(port: PortRef)` operation MUST remove the bidirectional connection of the port in the port array. Expected complexity: O(1). When the target of a disconnected port is a `FreePort(bid)`, the `set_port` call on the FreePort side is a no-op (FreePort has no slot in the port array). The corresponding entry in the external `free_port_index` or Border Map becomes stale. This is by design: SPEC-05 R6 handles missing border entries during merge. The `disconnect` operation does NOT update external maps. **(MUST)**

**R15.** The `get_target(port: PortRef) -> PortRef` operation MUST return the PortRef to which the given port is connected, via lookup in the port array. Expected complexity: O(1). **(MUST)**

**R15a.** The `get_agent(id: AgentId) -> Option<&Agent>` operation MUST return a reference to the agent with the given ID, or `None` if the ID is out of range or the slot is empty. Expected complexity: O(1). This is the canonical accessor for agent lookup; callers MUST NOT index into `agents` directly for read access. **(MUST)**

**R15b.** The `get_agent_mut(id: AgentId) -> Option<&mut Agent>` operation MUST return a mutable reference to the agent with the given ID, or `None` if the ID is out of range or the slot is empty. Expected complexity: O(1). **(MUST)**

**R16.** The `is_reduced(net: &Net) -> bool` function MUST return `true` if and only if the redex queue is empty (all redexes have been consumed or discarded). **(MUST)**

**R16a.** The `count_live_agents(&self) -> usize` operation MUST return the number of live agents in the net (slots where `agents[i].is_some()`). Expected complexity: O(A) where A is the arena size. **(MUST)**

**R16b.** The `live_agents(&self) -> impl Iterator<Item = &Agent>` operation MUST provide an iterator over all live agents in the net. The iterator MUST skip `None` slots. **(MUST)**

**R17.** The reduction engine MUST tolerate stale redexes in the queue: when consuming a pair `(a, b)`, it MUST verify that both agents exist and that `get_target(AgentPort(a, 0)) == AgentPort(b, 0)`. If the verification fails, the redex MUST be silently discarded (cf. SPEC-01, I4). **(MUST)**

### 3.4 Representation Invariants

**R18.** The port array MUST maintain bidirectionality: if `ports[idx(a)] == b`, then `ports[idx(b)] == a` (implements SPEC-01, T1 and I1). **(MUST)**

**R18a. Root port exception to T1:** The principal port of the root agent (when `net.root == Some(AgentPort(id, 0))`) MAY contain `DISCONNECTED` in the port array. This is a permanent, structural exception to T1 linearity: the root agent's principal port connects to the external observation point (represented by the `net.root` field), not to another port in the port array. Debug assertions MUST explicitly skip the root agent's principal port when checking T1/I1, rather than skipping all DISCONNECTED ports generically. **(MUST)**

**R18b. Self-loop policy:** Intra-agent connections (connecting two DIFFERENT ports of the SAME agent, e.g., `connect(AgentPort(x, 1), AgentPort(x, 2))`) are VALID. They satisfy T1 bidirectionality: `ports[x*3+1] == AgentPort(x, 2)` and `ports[x*3+2] == AgentPort(x, 1)`. These are required by Church(0) encoding (SPEC-14 R5). Same-port self-connections (connecting a port to ITSELF, e.g., `connect(AgentPort(x, 1), AgentPort(x, 1))`) are INVALID and MUST be rejected with a debug assertion. T1's "exactly one other port" means "exactly one distinct port reference" (a different `(AgentId, PortId)` pair). **(MUST)**

**R19.** Every reference `AgentPort(id, p)` in the port array MUST point to an existing agent (`agents[id].is_some()`) with `p <= arity(agents[id].symbol)` (implements SPEC-01, I2). **(MUST)**

**R20.** In debug mode (`#[cfg(debug_assertions)]`), the functions `create_agent`, `remove_agent`, `connect`, and the reduction rules MUST execute assertions for T1 (linearity/bidirectionality) and I2 (reference validity) after each operation. **(MUST)**

**R21.** In release mode, assertions MAY be disabled for performance. **(MAY)**

### 3.5 Formal Net Model

**R22.** The `Net` struct MUST be understood as a concrete representation of the formal pair `(A, W)` from DISC-004 v2 Section 1.1, where `A` is the set of agents (represented by the agent arena) and `W` is the set of wires (represented implicitly by the port array). **(MUST)**

**R23.** The representation MUST support the distinction between:
- **Internal wires** (both endpoints are `AgentPort` within the same net), and
- **Border wires** (one endpoint is an `AgentPort`, the other is a `FreePort(bid)` boundary sentinel).

This distinction is essential for the partitioning protocol (SPEC-04, conditions C1-C3 from ARG-002). **(MUST)**

### 3.6 Serialization

**R24.** The Net MUST be serializable for transmission between nodes using serde + bincode, per the confirmed technical decision. **(MUST)**

**R25.** The serialized format MUST be self-contained: a receiver with no prior knowledge of the net MUST be able to reconstruct the complete Net from the received bytes. **(MUST)**

**R26.** Serialization MUST preserve identity: `deserialize(serialize(net)) == net` (structural equality). **(MUST)**

**R26a.** The `Net` struct MUST derive `PartialEq` and `Eq` to enable structural equality comparison for serialization round-trip tests (R26) and debug comparisons. Note: structural equality (`==`) requires identical AgentIds; for graph isomorphism (structural equivalence modulo ID renaming), use `nets_isomorphic` (SPEC-08). **(MUST)**

**R27.** The format SHOULD use fixed size per element (inspired by HVM2, AC-006) to facilitate aligned access and size estimation. **(SHOULD)**

---

## 4. Design

### 4.1 Fundamental Types

```rust
/// The 3 universal symbols of Lafont (REF-002, p.71-72).
/// CON = gamma (constructor, arity 2)
/// DUP = delta (duplicator, arity 2)
/// ERA = epsilon (eraser, arity 0)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum Symbol {
    Con = 0,
    Dup = 1,
    Era = 2,
}
```

- **Formal definition:** The set of symbols Sigma = {gamma, delta, epsilon} from REF-002, p.71-72.
- **Invariants satisfied:** T5 (rule correctness requires exactly 3 symbols covering all 6 rules).
- **Reference:** REF-002 p.71-72; AC-001 (`data Symbol = CON | DUP | ERA`).

```rust
/// Unique identifier for an agent. Monotonically increasing, never reused.
/// u32 allows ~4 billion agents over the lifetime of an execution.
pub type AgentId = u32;

/// Port index within an agent. 0 = principal, 1-2 = auxiliary.
pub type PortId = u8;
```

- **Formal definition:** AgentId is a label from the set of natural numbers, injectively assigned to agents. PortId is an index in `{0, 1, ..., arity(symbol)}`.
- **Invariants satisfied:** I3 (monotonicity of IDs), T2 (port 0 is the principal port).
- **Reference:** AC-001 (`type AgentId = Int`, `type PortId = Int`); AC-015 CC-4 (ID space).

```rust
/// A reference to a specific port in the net.
///
/// AgentPort: port of a live agent in the net.
/// FreePort: interface port (Lafont) or boundary sentinel (partitioning).
///
/// The FreePort variant serves dual purpose (SPEC-00 Sections 6.1, 6.2):
/// - Lafont free ports: the external interface of the net (inputs/outputs).
/// - Boundary free ports: synthetic markers inserted by the partitioner,
///   carrying a borderId for merge resolution (DISC-004 v2 Section 1.4).
/// Both are structurally identical at the type level; the distinction is
/// semantic and resolved by context (the border map in SPEC-04/SPEC-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PortRef {
    /// Port of an agent: (agent_id, port_id).
    AgentPort(AgentId, PortId),
    /// Free port identified by a unique integer index.
    FreePort(u32),
}
```

- **Formal definition:** PortRef is the union type `AgentPort(AgentId, PortId) | FreePort(u32)`. Formally, a PortRef names a specific endpoint in the net's wiring relation `W` (DISC-004 v2 Section 1.1).
- **Invariants satisfied:** T1 (each port appears in exactly one wire; the port array maps each PortRef to exactly one target), I2 (every AgentPort reference is valid).
- **Reference:** AC-001 (`data PortRef = AgentPort AgentId PortId | FreePort Int`); DISC-004 v2 Section 1.4 (FreePort disambiguation).

```rust
/// An agent (node) in the interaction net.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Agent {
    pub symbol: Symbol,
    pub id: AgentId,
}
```

- **Formal definition:** An agent `a` in `A` with `symbol(a) in Sigma` and a unique label `id(a)`.
- **Invariants satisfied:** T5 (symbol determines the applicable interaction rule), I3 (id is unique and monotonic).
- **Reference:** AC-001 (`data Agent = Agent { agentSymbol :: Symbol, agentId :: AgentId }`).

### 4.2 Arity Function

The arity determines the number of auxiliary ports of an agent. The total number of ports is `arity + 1` (including the principal port).

```rust
/// Returns the arity (number of auxiliary ports) of a symbol.
/// CON and DUP: 2 auxiliary ports. ERA: 0 auxiliary ports.
pub const fn arity(symbol: Symbol) -> u8 {
    match symbol {
        Symbol::Con => 2,
        Symbol::Dup => 2,
        Symbol::Era => 0,
    }
}

/// Returns the total number of ports of a symbol (arity + 1).
pub const fn total_ports(symbol: Symbol) -> u8 {
    arity(symbol) + 1
}
```

- **Reference:** REF-001 p.95 (arity definition); REF-002 p.71 (gamma/delta arity 2, epsilon arity 0).

### 4.3 Net Structure

The Net struct is the concrete representation of the formal pair `(A, W)` from DISC-004 v2 Section 1.1, where:
- `A` (the set of agents) is represented by `agents: Vec<Option<Agent>>`
- `W` (the set of wires) is represented implicitly by `ports: Vec<PortRef>` (the flat port array)

Additionally, the Net contains operational state (redex queue, ID generator, root) that is not part of the formal model but is essential for efficient reduction.

```rust
use std::collections::VecDeque;

/// The complete interaction net.
///
/// Formally, a Net is a pair (A, W) where A is the set of agents and W
/// is the set of wires (DISC-004 v2, Section 1.1; REF-013, p.219).
///
/// Agents are stored in an arena indexed by AgentId.
/// Connections are represented implicitly by a flat port array.
/// The redex queue maintains known active pairs for incremental reduction.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Net {
    /// Agent arena. agents[id] == Some(agent) if the agent is live.
    /// agents[id] == None if the slot is free.
    pub agents: Vec<Option<Agent>>,

    /// Flat port array. The slot for port (id, port_id) is at index
    /// id * PORTS_PER_SLOT + port_id. Each slot stores the PortRef to
    /// which the port is connected.
    ///
    /// PORTS_PER_SLOT = 3 (maximum ports per agent: principal + 2 auxiliary).
    /// ERA agents waste 2 slots (ports 1 and 2 are unused).
    pub ports: Vec<PortRef>,

    /// Queue of active pairs (redexes) for incremental reduction.
    /// May contain stale entries (agents already consumed); the reduction
    /// engine MUST verify validity before reducing (R17).
    pub redex_queue: VecDeque<(AgentId, AgentId)>,

    /// Next AgentId to be assigned. Strictly greater than any AgentId
    /// in use. Incremented on each agent creation.
    pub next_id: AgentId,

    /// Root port: the AgentPort connected to the external observation point.
    /// None if the net has no root (e.g., a partition sub-net).
    pub root: Option<PortRef>,
}
```

- **Invariants satisfied:**
  - T1 (linearity) via the port array's bidirectional consistency (I1)
  - I2 (reference validity) via the correspondence between agents and port array entries
  - I3 (ID monotonicity) via the `next_id` field
  - I4 (redex queue validity) via the stale-check protocol (R17)
- **Reference:** AC-001 (`data Net = Net { netAgents :: Map AgentId Agent, netWires :: [Wire] }`); AC-006 (`GNet` with `node[]`/`vars[]` arena); DISC-004 v2 Section 1.1.

**Layout constant:**

```rust
/// Number of port slots per agent in the port array.
/// 3 = 1 (principal) + 2 (maximum auxiliary).
/// ERA wastes 2 slots, but uniformity simplifies indexing.
pub const PORTS_PER_SLOT: usize = 3;
```

**Index function:**

```rust
/// Computes the index in the port array for port (agent_id, port_id).
#[inline]
pub fn port_index(agent_id: AgentId, port_id: PortId) -> usize {
    (agent_id as usize) * PORTS_PER_SLOT + (port_id as usize)
}
```

### 4.4 Sentinel for Disconnected Ports

The port array needs a sentinel value to indicate that a slot is not connected to anything (e.g., the auxiliary slots of a freshly created ERA before connection, or a slot temporarily disconnected during a reduction rule).

```rust
/// Sentinel for disconnected or invalid port slots.
/// Used internally during reduction operations.
/// A slot containing DISCONNECTED violates invariant T1 (linearity)
/// if it persists after a reduction rule completes.
pub const DISCONNECTED: PortRef = PortRef::FreePort(u32::MAX);
```

Note: `DISCONNECTED` is a transient state. After each reduction rule, no port of a live agent may contain `DISCONNECTED` (verifiable by assertion in debug mode). This is a concrete representation concern, not a concept from Lafont's theory.

### 4.5 CRUD Operations

#### 4.5.1 Construction

```rust
impl Net {
    /// Creates an empty Net with no agents, wires, or redexes.
    pub fn new() -> Self {
        Net {
            agents: Vec::new(),
            ports: Vec::new(),
            redex_queue: VecDeque::new(),
            next_id: 0,
            root: None,
        }
    }

    /// Creates a Net with pre-allocated capacity for `capacity` agents.
    pub fn with_capacity(capacity: usize) -> Self {
        Net {
            agents: Vec::with_capacity(capacity),
            ports: Vec::with_capacity(capacity * PORTS_PER_SLOT),
            redex_queue: VecDeque::new(),
            next_id: 0,
            root: None,
        }
    }
}
```

#### 4.5.2 create_agent

```rust
impl Net {
    /// Creates a new agent with the given symbol.
    /// Returns the assigned AgentId.
    ///
    /// Complexity: O(1) amortized (may trigger Vec reallocation).
    /// Postcondition: agents[id] == Some(Agent { symbol, id }), next_id == id + 1.
    pub fn create_agent(&mut self, symbol: Symbol) -> AgentId {
        let id = self.next_id;
        self.next_id += 1;

        let agent = Agent { symbol, id };

        // Expand arena to contain index id
        if self.agents.len() <= id as usize {
            self.agents.resize((id as usize) + 1, None);
        }
        self.agents[id as usize] = Some(agent);

        // Expand port array for the new agent's ports
        let required_len = (id as usize + 1) * PORTS_PER_SLOT;
        if self.ports.len() < required_len {
            self.ports.resize(required_len, DISCONNECTED);
        }

        id
    }
}
```

#### 4.5.3 remove_agent

```rust
impl Net {
    /// Removes an agent from the net. The slot is marked as None.
    /// All of the agent's ports are disconnected in the port array.
    /// The AgentId is NOT reused.
    ///
    /// Complexity: O(1) (disconnects at most 3 ports).
    /// Precondition: agents[id].is_some().
    pub fn remove_agent(&mut self, id: AgentId) {
        if let Some(agent) = self.agents[id as usize] {
            let num_ports = total_ports(agent.symbol);
            for p in 0..num_ports {
                let port = PortRef::AgentPort(id, p);
                self.disconnect(port);
            }
            self.agents[id as usize] = None;
        }
    }
}
```

#### 4.5.4 connect

```rust
impl Net {
    /// Establishes a bidirectional connection between two ports.
    /// If both are principal ports (port_id == 0), inserts the pair
    /// into the redex queue.
    ///
    /// Complexity: O(1).
    /// Postcondition: get_target(a) == b && get_target(b) == a.
    ///
    /// Self-loop policy (R18b): intra-agent connections (different ports
    /// of the same agent) are valid. Same-port self-connections (a == b)
    /// are invalid and rejected with a debug assertion.
    ///
    /// Redex detection note: redex detection only fires when BOTH
    /// endpoints are `AgentPort(_, 0)`. Connections involving `FreePort`
    /// never produce redexes; border redexes are detected during merge
    /// when `FreePort` sentinels are resolved to `AgentPort` endpoints
    /// (SPEC-05 R5).
    pub fn connect(&mut self, a: PortRef, b: PortRef) {
        #[cfg(debug_assertions)]
        {
            // R18b: same-port self-connections are invalid.
            assert_ne!(a, b, "Same-port self-connection is invalid: {:?}", a);
        }

        self.set_port(a, b);
        self.set_port(b, a);

        // Incremental redex detection: if both are principal ports,
        // an active pair is formed.
        if let (PortRef::AgentPort(id_a, 0), PortRef::AgentPort(id_b, 0)) = (a, b) {
            self.redex_queue.push_back((id_a, id_b));
        }
    }
}
```

#### 4.5.5 disconnect

```rust
impl Net {
    /// Removes the bidirectional connection of a port.
    /// The slot of the port and the slot of the target are set to DISCONNECTED.
    ///
    /// Complexity: O(1).
    pub fn disconnect(&mut self, port: PortRef) {
        let target = self.get_target(port);
        if target != DISCONNECTED {
            self.set_port(target, DISCONNECTED);
        }
        self.set_port(port, DISCONNECTED);
    }
}
```

#### 4.5.6 get_target and set_port (internal helpers)

```rust
impl Net {
    /// Returns the PortRef to which the given port is connected.
    ///
    /// Complexity: O(1).
    pub fn get_target(&self, port: PortRef) -> PortRef {
        match port {
            PortRef::AgentPort(id, p) => {
                let idx = port_index(id, p);
                if idx < self.ports.len() {
                    self.ports[idx]
                } else {
                    DISCONNECTED
                }
            }
            PortRef::FreePort(_) => {
                // FreePort targets are resolved during merge (SPEC-05).
                // In this context, return DISCONNECTED.
                DISCONNECTED
            }
        }
    }

    /// Writes the target of a port in the port array.
    /// Only operates on AgentPort; FreePort is ignored (resolved in SPEC-05).
    fn set_port(&mut self, port: PortRef, target: PortRef) {
        if let PortRef::AgentPort(id, p) = port {
            let idx = port_index(id, p);
            if idx < self.ports.len() {
                self.ports[idx] = target;
            }
        }
    }
}
```

#### 4.5.7 is_reduced

```rust
impl Net {
    /// Returns true if the net is in normal form (no valid active pairs remain).
    ///
    /// Note: the redex queue may contain stale entries. This function only
    /// checks whether the queue is empty. For rigorous verification (that
    /// discards stale entries), use drain_stale() + is_reduced(), or simply
    /// run reduce_all until it returns (SPEC-03).
    pub fn is_reduced(&self) -> bool {
        self.redex_queue.is_empty()
    }
}
```

#### 4.5.8 Redex Validation (stale check)

```rust
impl Net {
    /// Checks whether a pair (a, b) in the redex queue is valid (non-stale).
    ///
    /// A redex is valid if:
    /// 1. Both agents exist.
    /// 2. The principal ports of a and b are connected to each other.
    pub fn is_valid_redex(&self, a: AgentId, b: AgentId) -> bool {
        let a_exists = self.get_agent(a).is_some();
        let b_exists = self.get_agent(b).is_some();
        if !a_exists || !b_exists {
            return false;
        }
        self.get_target(PortRef::AgentPort(a, 0)) == PortRef::AgentPort(b, 0)
    }
}
```

#### 4.5.9 Agent Accessors

```rust
impl Net {
    /// Returns a reference to the agent with the given ID.
    /// Returns None if the ID is out of range or the slot is empty.
    ///
    /// Complexity: O(1).
    /// This is the canonical accessor for agent lookup (R15a).
    pub fn get_agent(&self, id: AgentId) -> Option<&Agent> {
        self.agents.get(id as usize).and_then(|slot| slot.as_ref())
    }

    /// Returns a mutable reference to the agent with the given ID.
    /// Returns None if the ID is out of range or the slot is empty.
    ///
    /// Complexity: O(1).
    pub fn get_agent_mut(&mut self, id: AgentId) -> Option<&mut Agent> {
        self.agents.get_mut(id as usize).and_then(|slot| slot.as_mut())
    }
}
```

#### 4.5.10 Iteration and Counting

```rust
impl Net {
    /// Returns the number of live agents in the net.
    ///
    /// Complexity: O(A) where A is the arena size.
    pub fn count_live_agents(&self) -> usize {
        self.agents.iter().filter(|s| s.is_some()).count()
    }

    /// Returns an iterator over all live agents in the net.
    /// Skips None slots.
    pub fn live_agents(&self) -> impl Iterator<Item = &Agent> {
        self.agents.iter().filter_map(|s| s.as_ref())
    }
}
```

### 4.6 Debug Assertions

The assertions below implement the checks required by R20, corresponding to invariants T1, I1, I2, and I3 of SPEC-01.

```rust
#[cfg(debug_assertions)]
impl Net {
    /// Verifies I1: bidirectionality of the port array.
    /// For each live agent, each connected port must have the correct reverse.
    ///
    /// The root agent's principal port is exempt from T1 (R18a): it MAY
    /// contain DISCONNECTED because its external connection is represented
    /// by `net.root`, not by the port array.
    pub fn assert_adjacency_consistent(&self) {
        // Determine the root agent ID (if any) for the T1 exception.
        let root_agent_id = match self.root {
            Some(PortRef::AgentPort(id, 0)) => Some(id),
            _ => None,
        };

        for slot in self.agents.iter() {
            if let Some(agent) = slot {
                let num_ports = total_ports(agent.symbol);
                for p in 0..num_ports {
                    let port = PortRef::AgentPort(agent.id, p);
                    let target = self.get_target(port);
                    if target == DISCONNECTED {
                        // R18a: root agent's principal port is exempt.
                        if p == 0 && root_agent_id == Some(agent.id) {
                            continue; // Permanent DISCONNECTED for root (R18a)
                        }
                        continue; // Transient DISCONNECTED during reduction
                    }
                    let reverse = self.get_target(target);
                    assert_eq!(
                        reverse, port,
                        "I1 violated: port {:?} -> {:?}, but {:?} -> {:?}",
                        port, target, target, reverse
                    );
                }
            }
        }
    }

    /// Verifies I2: every AgentPort reference in the port array points to
    /// an existing agent with a valid port index.
    pub fn assert_refs_valid(&self) {
        for slot in self.agents.iter() {
            if let Some(agent) = slot {
                let num_ports = total_ports(agent.symbol);
                for p in 0..num_ports {
                    let target = self.get_target(PortRef::AgentPort(agent.id, p));
                    if target == DISCONNECTED {
                        continue;
                    }
                    if let PortRef::AgentPort(tid, tp) = target {
                        assert!(
                            self.agents.get(tid as usize).map_or(false, |s| s.is_some()),
                            "I2 violated: port {:?} of agent {} points to nonexistent agent {}",
                            p, agent.id, tid
                        );
                        let target_agent = self.agents[tid as usize].unwrap();
                        assert!(
                            tp <= arity(target_agent.symbol),
                            "I2 violated: port {} exceeds arity of {:?} (agent {})",
                            tp, target_agent.symbol, tid
                        );
                    }
                    // FreePort targets are valid (used in partitioning)
                }
            }
        }
    }

    /// Verifies I3: next_id > max(IDs in use).
    pub fn assert_next_id_valid(&self) {
        for (i, slot) in self.agents.iter().enumerate() {
            if slot.is_some() {
                assert!(
                    (i as u32) < self.next_id,
                    "I3 violated: agent at slot {} but next_id is {}",
                    i, self.next_id
                );
            }
        }
    }

    /// Counts stale redexes in the queue.
    /// Note: stale redexes are tolerated by the reduction engine,
    /// but this function reports how many exist (diagnostic metric).
    pub fn count_stale_redexes(&self) -> usize {
        self.redex_queue
            .iter()
            .filter(|(a, b)| !self.is_valid_redex(*a, *b))
            .count()
    }

    /// Verifies that ERA agents' unused port slots (ports 1 and 2)
    /// contain DISCONNECTED. If they contain any other value, it
    /// indicates a bug in a prior operation.
    pub fn assert_era_unused_ports_clean(&self) {
        for slot in self.agents.iter() {
            if let Some(agent) = slot {
                if agent.symbol == Symbol::Era {
                    for p in 1..=2u8 {
                        let idx = port_index(agent.id, p);
                        if idx < self.ports.len() {
                            assert_eq!(
                                self.ports[idx], DISCONNECTED,
                                "ERA agent {} has non-DISCONNECTED value at unused port {}",
                                agent.id, p
                            );
                        }
                    }
                }
            }
        }
    }

    /// Runs all invariant checks.
    pub fn assert_all_invariants(&self) {
        self.assert_adjacency_consistent();
        self.assert_refs_valid();
        self.assert_next_id_valid();
        self.assert_era_unused_ports_clean();
    }
}
```

### 4.7 Incremental Redex Detection

Redex detection is the primary divergence between Relativist and the Haskell prototype.

**Haskell prototype (AC-001):** `findRedexes` traverses ALL wires at every step, O(w) per step. For a net with 1000 wires and 1 redex, that is 1000 comparisons to find it. Total cost of a full reduction is O(steps * w).

**HVM2 (AC-006, AC-007):** On-the-fly detection via `link`: when both ports are nodes (non-VAR), `push_redex` is called. Amortized cost is O(1) per step.

**Relativist:** Adopts the incremental model inspired by HVM2. The `connect` function detects when both ports are principal ports and inserts into the queue. This transforms redex detection from O(w) to O(1) amortized per connection operation.

The redex queue may contain stale entries because:
1. Reducing a pair `(a, b)` removes agents `a` and `b`, but other pairs in the queue that reference `a` or `b` remain.
2. Reconnections during a rule may alter the neighborhood of a principal port without updating queue entries.

The reduction engine (SPEC-03) handles stale entries via the "validate-or-skip" pattern:

```
loop:
    (a, b) = redex_queue.pop_front()
    if (a, b) is None: break  // queue empty -> normal form
    if !is_valid_redex(a, b): continue  // stale -> discard
    apply_rule(a, b)
```

### 4.8 ID Generation

The `next_id` field is a bump-only counter. Each call to `create_agent` consumes an ID and increments the counter.

In the distributed context (SPEC-04, SPEC-05), the ID space is statically partitioned among workers to avoid collisions:

```
Worker 0: [0,              chunk_size)
Worker 1: [chunk_size,     2 * chunk_size)
...
Worker N: [N * chunk_size, (N+1) * chunk_size)
```

Where `chunk_size = u32::MAX / num_workers`. Each worker initializes `next_id` with `worker_id * chunk_size`. This eliminates the need for post-reduction remapping (`remapAllPartitions` in the Haskell prototype, AC-003), removing an entire phase from the grid cycle.

- **Invariants satisfied:** I3 (monotonicity), D4 (ID uniqueness in distributed context, Premise P4).
- **Reference:** AC-015 CC-4 (ID space partitioning); AC-011 (HVM4 heap partitioning by thread, analogous approach).

The full specification of ID space partitioning is in SPEC-04.

### 4.9 Root Port

The net needs an observation point: the external port from which the computation result is extracted. In HVM2, this is the `ROOT` sentinel (AC-006). In the Haskell prototype, free ports serve as the interface.

In Relativist, the root is an `Option<PortRef>` field on the Net:

```rust
/// The root field in Net stores the AgentPort connected to the
/// external observation point.
/// - Some(AgentPort(id, 0)): the result is the agent at this port.
///   The root MUST always reference port 0 (principal port) of a
///   live agent (R6a).
/// - None: the net has no root (e.g., partition sub-net without
///   a designated output).
///
/// During net construction, the root is set via:
///   net.root = Some(PortRef::AgentPort(root_agent, 0));
///
/// After reduction, the result is extracted by reading net.root.
```

Note: Unlike the port array (which only stores connections for `AgentPort` entries), the root is stored separately because it represents an external observation point, not a connection between two agents. In the Haskell prototype, this role was played by `FreePort(0)` as interface port (AC-001); in Relativist, the explicit `root` field is clearer and avoids conflation with boundary FreePorts used by the partitioner.

**Root behavior during reduction:** The `root` field is set once at net construction and is NOT automatically updated by `connect`, `disconnect`, or reduction rules. For Church numeral arithmetic nets (SPEC-14), the root agent's principal port is DISCONNECTED in the port array (not connected to another principal port), so the root agent does NOT participate in any active pair and is never consumed by reduction. The result is extracted by following `net.root -> agent -> auxiliary ports` after reduction completes. If a future encoding requires the root agent to be consumed during reduction, the reduction engine or encoding module MUST update `net.root` explicitly.

**Serialization:** The port array may contain the sentinel `FreePort(u32::MAX)` (DISCONNECTED) in slots for unused ERA ports (ports 1-2) and the root agent's principal port. Receivers MUST treat `FreePort(u32::MAX)` as an invalid/unconnected sentinel, not as a valid FreePort ID.

**`is_reduced` semantics note:** The `is_reduced` function (R16) checks the redex queue, not the net's actual topological state. For a net constructed via the CRUD API with proper use of `connect` (which populates the redex queue incrementally), an empty queue reliably indicates Normal Form. For nets constructed by other means (e.g., deserialization, manual mutation), use `drain_stale()` or scan the net for active pairs to verify Normal Form.

### 4.10 FreePort Storage and the Border Map

Free ports (`FreePort(bid)`) arise from two sources (SPEC-00 Sections 6.1, 6.2; DISC-004 v2 Section 1.4):

1. **Lafont free ports:** External interface of the original net. In a complete program, only the root is typically a free port.
2. **Boundary free ports:** Synthetic markers inserted during partitioning to represent cut connections between partitions.

Because the port array is indexed by `(AgentId, PortId)`, a FreePort has no direct slot. When a connection involves a FreePort, it is stored on the AgentPort side:

```
connect(AgentPort(42, 1), FreePort(7))
  -> ports[42 * 3 + 1] = FreePort(7)
  // The reverse (FreePort -> AgentPort) cannot be stored in the port array.
```

To resolve the reverse direction, the partitioner (SPEC-04) and merger (SPEC-05) maintain a **Border Map**: a `HashMap<u32, PortRef>` that maps `FreePort(bid)` identifiers to the `AgentPort` they are connected to. The `BorderMap` type alias is defined in SPEC-04 (partition module), where it is stored and used. SPEC-02 documents the concept but defers the type definition to SPEC-04 for proximity to its usage.

The invariant T1 (bidirectionality) is relaxed for FreePort connections: bidirectionality is maintained by the external Border Map, not by the port array alone. Within the port array, the connection is one-directional (AgentPort -> FreePort). The reverse mapping (FreePort -> AgentPort) is available through the Border Map.

- **Invariants satisfied:** T1 (bidirectionality, with relaxation for FreePorts via Border Map), D1c (C3, FreePort bijectivity).
- **Reference:** AC-002 (`planBorders` map); DISC-004 v2 Section 1.4; ARG-002 Q3 (FreePort mechanism).

### 4.11 Serialization Format

Relativist uses serde + bincode (confirmed technical decision). Serialization is performed on the `Net` struct directly via `#[derive(Serialize, Deserialize)]`.

The bincode-serialized format of a `Net` contains:

```
[agents_len: u64][agents: (Option<Agent>)*][ports_len: u64][ports: (PortRef)*]
[redex_queue_len: u64][redexes: (u32, u32)*][next_id: u32][root: Option<PortRef>]
```

Where each `Option<Agent>` is serialized as `[tag: u8][payload: Agent?]` (tag 0 = None, tag 1 = Some), and each `PortRef` as `[tag: u8][payload: (u32, u8) | u32]`.

**TCP framing:** Each message is prefixed by a 4-byte header (big-endian `u32`) indicating the payload size in bytes. This follows the length-prefixed pattern of the Haskell prototype (AC-003) and is detailed in SPEC-06.

**Size estimate:** For a net with `A` agents and `R` pending redexes:
- Agents: `A * (1 + 1 + 4)` = `6A` bytes (Option tag + Symbol + AgentId)
- Port array: `A * 3 * 6` = `18A` bytes (3 ports per agent, ~6 bytes per PortRef with tag)
- Redex queue: `R * 8` bytes (two u32 per redex)
- Overhead: ~28 bytes (lengths, next_id, root)
- **Approximate total:** `24A + 8R + 28` bytes

For a typical net with 10,000 agents and 100 redexes: ~240 KB.

- **Invariants satisfied:** R26 (round-trip identity).
- **Reference:** AC-003 (Haskell serialization as `[Int]`); AC-015 CC-7 (fixed-size recommendation).

### 4.12 Comparison of Approaches: Trade-off Table

| Aspect | Haskell (AC-001) | HVM2 (AC-006) | Relativist |
|--------|------------------|---------------|------------|
| Agent arena | `Map AgentId Agent` (O(log n) lookup) | `node[]: APair[]` (O(1) lookup, index = ID) | `Vec<Option<Agent>>` (O(1) lookup, index = ID) |
| Wire representation | `[Wire]` (list, O(w) scan) | Implicit in `node[]` (aux ports) and `vars[]` (linking) | Flat port array `Vec<PortRef>` (O(1) lookup) |
| Redex detection | `findRedexes` scan O(w) | On-the-fly via `link`, RBag hi/lo | Incremental via `connect`, `VecDeque` |
| ID generation | `findMax + 1`, O(log n) | Arena index (scan for free slot) | Bump counter `next_id`, O(1) |
| Serialization | `[Int]` variable (AC-003) | N/A (shared memory) | serde + bincode (fixed size per element) |
| Wasted space | 0 (Map) | Large (entire arena pre-allocated) | Moderate (None slots for removed agents) |
| Formal model | Explicit (A, W) as (Map, [Wire]) | Implicit (packed arrays) | Explicit (A, W) as (Vec, port array) |

---

## 5. Rationale

### 5.1 Vec<Option<Agent>> Instead of HashMap<AgentId, Agent>

**Decision:** Dense arena (`Vec<Option<Agent>>`) instead of hash map.

**Rationale:**
- O(1) guaranteed lookup (direct index) vs. O(1) amortized for HashMap (with hashing overhead and possible cache misses).
- Agents are created with monotonically increasing IDs, producing a dense array without significant fragmentation. Most slots will be `Some`.
- Trivial serialization: the vector is a linear sequence.
- Established pattern from HVM2 (AC-006): `node[]` indexed by ID.

**Discarded alternative:** `HashMap<AgentId, Agent>` (as the Haskell prototype). Advantage: no wasted slots for removed agents. Disadvantage: hashing overhead, cache unfriendly, non-deterministic iteration order.

**Reference:** AC-001 (`Map.lookup` as O(log n) in Haskell), AC-006 (arena `node[]` as O(1)).

### 5.2 Flat Port Array Instead of Wire List

**Decision:** Linear port array `Vec<PortRef>` indexed by `(agent_id * 3 + port_id)` instead of a wire list `Vec<(PortRef, PortRef)>`.

**Rationale:**
- O(1) lookup by direct index vs. O(w) linear scan in the Haskell prototype (AC-001).
- The `portNeighbor` function in the prototype traverses ALL wires to find the neighbor of a port (AC-001, lines 141-148). In Relativist, `get_target` is a vector access.
- Bidirectionality is maintained by invariant (R18): `connect` always writes both sides.
- Space: 3 slots per agent (wastes 2 slots for ERA). For a net with 50% ERAs, the overhead is ~33%. Acceptable given the performance gain.

**Discarded alternative:** Two separate buffers `node[]` (for auxiliary port pairs) and `vars[]` (for linking), as in HVM2 (AC-006). Advantage: less waste for ERA (ERA does not occupy a slot in `node[]`). Disadvantage: additional complexity from two address spaces, the need for compact encoding with LSB tags to distinguish node-port from var-port, and harder to serialize/understand. For the scope of this project, the simplicity of a single flat array outweighs the memory savings.

**Discarded alternative:** Compact `u32` port encoding with `(val << 3) | tag` as in HVM2 (AC-006, AC-015 CC-1). Advantage: 4 bytes per reference, trivially serializable, fits in a register. Disadvantage: requires bit manipulation for each access, less readable, more prone to bugs in an academic prototype. Relativist prioritizes clarity over micro-optimization. If benchmarks reveal a bottleneck in memory or serialization, migration to compact encoding is possible without changing the public API.

**Reference:** AC-001 (O(w) scan as primary bottleneck, lines 141-148), AC-006 (arena as efficient implementation pattern), AC-015 CC-1 (compact encoding recommendation).

### 5.3 Incremental VecDeque Instead of Global Scan

**Decision:** `VecDeque<(AgentId, AgentId)>` with incremental insertion in `connect`, instead of `findRedexes` with global scan.

**Rationale:**
- The Haskell prototype calls `findRedexes` BEFORE each reduction step, at cost O(w) per step. For a net with 1000 wires and 500 reduction steps, that is 500,000 comparisons. Total cost is O(steps * w) (AC-001, lines 170-177).
- HVM2 eliminates this cost with on-the-fly detection via `link` and RBag (AC-006, AC-007, AC-015 CC-2). Each connection tests in O(1) whether a redex was formed.
- Relativist adopts the same principle: `connect` checks whether both ports are principal ports and inserts into the queue. Total cost: O(steps) instead of O(steps * w).
- The VecDeque (FIFO) is chosen over HVM2's dual-queue hi/lo for simplicity. Redex prioritization (AC-006 D6) is an optimization that MAY be added later (AC-015 Z8).

**Discarded alternative:** Dual-queue hi/lo as in HVM2 (AC-006): annihilation/erasure in hi, commutation in lo. Advantage: may reduce peak memory by prioritizing destructive rules. Disadvantage: additional complexity, and the benefit is only measurable via empirical benchmarks (AC-015 Z8). For this project, FIFO is sufficient; strong confluence guarantees that order is irrelevant for correctness.

**Reference:** AC-001 (findRedexes as bottleneck), AC-006 (RBag with priority), AC-007 (atomic link), AC-015 CC-2 (comparison of approaches).

### 5.4 Enum PortRef Instead of Compact u32 Encoding

**Decision:** `enum PortRef { AgentPort(AgentId, PortId), FreePort(u32) }` instead of `u32` with `(val << 3) | tag`.

**Rationale:**
- The Rust enum is type-safe: the compiler guarantees exhaustive pattern matching, eliminating entire classes of bugs (wrong tag, val out of range).
- With serde, serializes to a self-describing format (tag byte + payload).
- Readability and debuggability are superior: `println!("{:?}", port)` shows `AgentPort(42, 0)` instead of a hexadecimal number.
- The space overhead (~6 bytes vs. 4 bytes per PortRef) is acceptable for the project scope.

**Trade-off:** In production, the compact `u32` encoding of HVM2 (AC-006) would be preferable for memory savings and cache locality. Future migration is possible by encapsulating PortRef in a newtype with conversion methods.

**Reference:** AC-006 (u32 port encoding), AC-015 CC-1 (compact encoding recommendation for production).

### 5.5 serde + bincode Instead of Custom Format

**Decision:** Use serde + bincode for serialization, per the confirmed technical decision, instead of a custom binary format.

**Rationale:**
- serde + bincode is idiomatic in Rust: derive macros generate serialization code automatically.
- bincode produces a compact binary format, little-endian, without schema overhead (unlike JSON or MessagePack).
- The round-trip property (`deserialize(serialize(x)) == x`) is guaranteed by construction for types with `Serialize + Deserialize`.
- The Haskell prototype used a custom `[Int]` format with TLV tags (AC-003), which was prone to parsing bugs and wasted space with per-element tags.

**Trade-off:** bincode does not produce a fixed-size-per-element format (Option and enum have variable tags). For memcpy-style serialization (as recommended in AC-015 CC-7), a custom layout with fixed width would be superior. However, the additional complexity is not justified for this project.

**Reference:** AC-003 (Haskell `[Int]` serialization, verbosity problems), AC-015 CC-7 (fixed-format recommendation for production).

### 5.6 Root as Explicit Field Instead of FreePort Convention

**Decision:** Store the root as an explicit `Option<PortRef>` field on Net, instead of using `FreePort(0)` as a convention.

**Rationale:**
- Avoids conflation of the root observation point with boundary FreePorts used by the partitioner (DISC-004 v2 Section 1.4).
- The Haskell prototype used free ports as the external interface (AC-001), which is theoretically clean but creates ambiguity in the distributed context where FreePort is overloaded with boundary semantics.
- An explicit field is self-documenting and simplifiable for sub-nets that have no root (partitions).
- HVM2 uses a dedicated `ROOT` sentinel (AC-006), which serves the same purpose as this field.

**Reference:** AC-001 (FreePort as interface), AC-006 (ROOT sentinel), DISC-004 v2 Section 1.4 (FreePort disambiguation).

---

## 6. Haskell Prototype Reference

### 6.1 Type Mapping

| Haskell (AC-001) | Relativist | Change | Reason |
|------------------|------------|--------|--------|
| `data Symbol = CON \| DUP \| ERA` | `enum Symbol { Con, Dup, Era }` | None (isomorphic) | Direct mapping of the 3 symbols |
| `type AgentId = Int` | `type AgentId = u32` | Fixed-size type | Haskell Int is arbitrarily large; u32 is sufficient and efficient |
| `type PortId = Int` | `type PortId = u8` | Smaller type | Values 0-2 fit in u8 |
| `data PortRef = AgentPort AgentId PortId \| FreePort Int` | `enum PortRef { AgentPort(AgentId, PortId), FreePort(u32) }` | Isomorphic | Direct mapping |
| `data Agent = Agent { agentSymbol, agentId }` | `struct Agent { symbol, id }` | Isomorphic | Direct mapping |
| `data Wire = Wire PortRef PortRef` | Implicit in `ports: Vec<PortRef>` | Eliminated | Wires are the representation; Relativist uses an adjacency map |
| `data Net = Net { netAgents :: Map, netWires :: [Wire] }` | `struct Net { agents: Vec<Option<Agent>>, ports: Vec<PortRef>, ... }` | Redesigned | Map -> Vec (O(1) lookup), [Wire] -> port array (O(1) lookup) |
| `data Redex = Redex AgentId AgentId` | `(AgentId, AgentId)` in VecDeque | Simplified | Tuple is sufficient; incremental queue replaces findRedexes |

### 6.2 What Worked in the Prototype

1. **Conceptual model Agent/Wire/Net** (AC-001): the clean separation between agents and connections is preserved. Relativist only changes the representation from O(w) to O(1).

2. **FreePort as boundary sentinel** (AC-001, AC-002): the concept of FreePort to mark partition boundaries is elegant and reused integrally. The distinction between Lafont free ports and boundary free ports (DISC-004 v2 Section 1.4) is now explicit in the spec.

3. **Invariant `aid1 < aid2` for canonical redex ordering** (AC-001): Relativist does not require this invariant in the redex queue (strong confluence makes order irrelevant), but MAY adopt it for deterministic debug output.

### 6.3 What Relativist Changes

1. **From `[Wire]` to flat port array:** The O(w) bottleneck of `portNeighbor` (AC-001, lines 141-148) is eliminated. Neighbor lookup goes from O(w) to O(1).

2. **From `findRedexes` to incremental redex queue:** The O(w) per-step bottleneck (AC-001, lines 170-177) is eliminated. Redex detection goes from O(steps * w) to O(steps).

3. **From `Map AgentId Agent` to `Vec<Option<Agent>>`:** Lookup goes from O(log n) to O(1). The trade-off is possible because IDs are monotonically increasing (not sparse as in a general Map).

4. **From `removeAgent` with O(w) filter to `remove_agent` O(1):** The prototype filters ALL wires to remove those referencing the agent (AC-001, lines 119-123). Relativist disconnects the 3 ports directly (O(1) per port).

5. **From `nextAgentId = findMax + 1` to bump counter:** The prototype computes O(log n) at each ID generation (AC-001, lines 159-162). Relativist increments a counter in O(1).

6. **From implicit interface via FreePort to explicit `root` field:** Avoids conflation with boundary FreePorts in the distributed context (Section 5.6).

---

## 7. Resolved Questions

*All questions resolved during Human Check review (2026-04-04).*

1. **Port array sizing for ERA.** **RESOLVED: Keep uniform (NO optimization).** ERA agents occupy 3 slots in the port array but only use 1. The ~67% waste in ERA slots is accepted in favor of uniform allocation, which simplifies the implementation. If benchmarks later show this is a problem, variable-arity allocation can be revisited as a post-v1 optimization.

2. **Arena compaction after many removals.** **RESOLVED: Out of scope.** The `Vec<Option<Agent>>` accumulates `None` slots after annihilation reductions. Relativist does not compact the arena (IDs are never reused). This is accepted for v1. If memory becomes a problem for very long reductions, a compaction operation with ID remapping could be implemented between grid rounds as a future optimization.

3. **FreePort in the port array.** **RESOLVED: Handled by SPEC-04/SPEC-05.** The asymmetry introduced by FreePort (no slot in the port array, requires external Border Map) is fully addressed by SPEC-04 (partitioning, Border Map construction) and SPEC-05 (merge, Border Map resolution). No further action needed in SPEC-02.
