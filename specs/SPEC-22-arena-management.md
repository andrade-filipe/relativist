# SPEC-22: Arena Management and Memory Efficiency

**Status:** Draft
**Depends on:** SPEC-02 (Net Representation), SPEC-01 (Invariants), SPEC-03 (Reduction Engine)
**Amends:** SPEC-01 I3 (Monotonicity of AgentIds — relaxed for free-list variant), SPEC-02 R12 (remove_agent — extended for free-list)
**ROADMAP items:** 2.33 (Arena Recycling / GC During Reduction — free-list variant only), 2.32 (Sparse Net Representation)
**References:** REF-002 (Lafont 1997), REF-003 (HVM2 — arena management), REF-014 (Kahl — GC impact on parallel reduction)

---

## 1. Purpose

This spec defines two independent mechanisms for improving memory efficiency in Relativist's net representation:

1. **Arena Recycling via Free-List (ROADMAP 2.33, free-list variant):** A free-list of recycled `AgentId` slots that allows `create_agent` to reuse slots vacated by consumed agents, preventing the agent arena from growing indefinitely during long reductions. This is the free-list variant only — no renumbering, no compaction, no remapping.

2. **Sparse Net Representation (ROADMAP 2.32):** An alternative net representation using `HashMap<AgentId, Agent>` and `HashMap<(AgentId, PortId), PortRef>` instead of the dense `Vec<Option<Agent>>` and flat `Vec<PortRef>`. This eliminates wasted memory for consumed agent slots and is useful for construction and partitioning phases where proportionality to live agents matters.

These two features are **independent**. Arena recycling operates on the existing `Vec`-based `Net`. The sparse net is a completely new representation type. Both MAY be implemented separately and neither depends on the other.

---

## 2. Definitions

Terms defined in SPEC-00 (Glossary) and SPEC-02 (Net Representation) are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **Free-List** | A stack (LIFO) of `AgentId` values corresponding to agent slots that have been freed by `remove_agent`. The free-list enables slot reuse without renumbering. Implemented as `Vec<AgentId>` within the `Net` struct. |
| **Recycled Slot** | An agent slot whose `AgentId` was previously assigned to a live agent, which was subsequently consumed by a reduction rule and added to the free-list. A recycled slot's `agents[id]` entry is `None` and its `AgentId` is present in the free-list. |
| **Tombstone** | An agent slot in the `Vec<Option<Agent>>` arena where `agents[id] == None` and the slot was previously occupied. In the absence of a free-list, tombstones persist for the lifetime of the net. With a free-list, tombstones are reclaimed when `create_agent` pops their ID from the free-list. |
| **Tombstone Ratio** | The fraction of tombstone slots over total arena length: `(arena_len - live_count) / arena_len`. A high tombstone ratio (>0.5) indicates significant memory waste. |
| **Arena Recycling** | The process of reusing tombstone slots via the free-list, as opposed to always allocating new IDs via `next_id` increment. Recycling prevents arena growth when agent creation and destruction are balanced (e.g., commutation rules that destroy 2 agents and create 4). |
| **SparseNet** | An alternative net representation where agents and ports are stored in `HashMap` collections. Memory is proportional to live agents only. No tombstone slots, no padding, no wasted port array entries. |
| **Dense-to-Sparse Conversion** | The operation `Net::to_sparse() -> SparseNet` that converts a dense `Vec`-based net to a sparse `HashMap`-based net, skipping `None` slots. Complexity: O(live_agents). |
| **Sparse-to-Dense Conversion** | The operation `SparseNet::to_dense() -> Net` that converts a sparse net back to a dense `Vec`-based net. The resulting dense net's arena has no tombstones. Complexity: O(live_agents). |
| **Hybrid Approach** | The strategy of using `SparseNet` for construction and partitioning (where memory proportionality matters), then converting to dense `Net` before the reduction loop (where O(1) indexed access matters). |

---

## 3. Requirements

### 3.1 Arena Recycling / Free-List (ROADMAP 2.33)

**R1.** The `Net` struct MUST contain a free-list field: `free_list: Vec<AgentId>`. The free-list stores the `AgentId` values of slots that have been freed by `remove_agent` and are available for reuse by `create_agent`. **(MUST)**

**R2.** When `remove_agent(id)` is called, after marking `agents[id] = None` and disconnecting all ports (as per SPEC-02 R12), the operation MUST push `id` onto the free-list. **(MUST)**

**R3.** When `create_agent(symbol)` is called, it MUST first check the free-list:
- If the free-list is non-empty, pop an `AgentId` from the free-list, reuse that slot in the arena, and return the recycled ID. The `next_id` field MUST NOT be incremented.
- If the free-list is empty, allocate a new slot by using `next_id` and incrementing it (existing behavior per SPEC-02 R11).
**(MUST)**

**R4.** When reusing a recycled slot, `create_agent` MUST:
- (a) Set `agents[id] = Some(Agent { symbol, id })` where `id` is the popped free-list value.
- (b) Ensure the port array slots for the recycled ID (`id * 3 + 0`, `id * 3 + 1`, `id * 3 + 2`) are initialized to `DISCONNECTED`.
- (c) NOT expand the arena or port array (the slot already exists within bounds).
**(MUST)**

**R5.** The free-list MUST use LIFO (stack) ordering. Popping from the end of a `Vec<AgentId>` is O(1). The LIFO policy maximizes temporal locality: recently freed slots are reused first, which tends to keep hot cache lines active. **(MUST)**

**R6.** The free-list MUST NOT contain duplicate `AgentId` values. Each freed ID appears in the free-list at most once. The `remove_agent` operation MUST NOT push an ID that is already in the free-list. In debug mode, this invariant SHOULD be verified by assertion. **(MUST)**

**R7.** An `AgentId` in the free-list MUST NOT be referenced by any `PortRef::AgentPort(id, _)` in the port array, the redex queue, or the `root` field. The slots for a free-list ID in the port array MUST contain `DISCONNECTED`. This is guaranteed by `remove_agent` disconnecting all ports before pushing to the free-list. **(MUST)**

**R8.** The `Net::new()` and `Net::with_capacity()` constructors MUST initialize the free-list as empty (`Vec::new()`). **(MUST)**

**R9.** The free-list MUST be included in serde serialization/deserialization of `Net` (SPEC-02 R24-R26). A deserialized net MUST have a valid free-list: every ID in the free-list MUST correspond to a `None` slot in the arena, and every `None` slot in the arena SHOULD be in the free-list (slots that were `None` before free-list introduction MAY not be in the free-list for backward compatibility). **(MUST)**

**R10.** The free-list MUST be compatible with ID space partitioning (SPEC-04 R16-R19, SPEC-01 D4). When a worker creates agents during reduction, it MUST draw recycled IDs from the free-list only if those IDs fall within the worker's assigned ID range `[start, end)`. IDs in the free-list that fall outside the worker's range MUST NOT be used by that worker. **(MUST)**

**R10a.** The `build_subnet()` operation (SPEC-04) SHOULD populate the free-list of each partition with the `None` slots that fall within that partition's ID range. This ensures partitions start with a pre-populated free-list for immediate recycling during local reduction. **(SHOULD)**

**R11.** The `count_live_agents()` operation (SPEC-02 R16a) MUST NOT count free-list entries as live agents. The free-list does not change the semantics of `count_live_agents`; it only affects which `None` slots are available for reuse. **(MUST)**

**R12.** The `merge()` operation (SPEC-05) MUST handle free-lists from multiple partitions. After merging, the resulting net's free-list MUST contain only IDs that correspond to `None` slots in the merged arena. IDs that were in a partition's free-list but whose slots are now occupied in the merged net MUST NOT appear in the merged free-list. **(MUST)**

### 3.2 Sparse Net Representation (ROADMAP 2.32)

**R13.** A `SparseNet` type MUST be defined (in `src/net/sparse.rs`) with the following fields:
- `agents: HashMap<AgentId, Agent>` — maps each live agent's ID to the agent.
- `ports: HashMap<(AgentId, PortId), PortRef>` — maps each port to its connected target.
- `redex_queue: VecDeque<(AgentId, AgentId)>` — queue of active pairs (same semantics as `Net`).
- `next_id: AgentId` — next available ID (same semantics as `Net`).
- `root: Option<PortRef>` — root observation port (same semantics as `Net`).
**(MUST)**

**R14.** `SparseNet` MUST implement the same logical operations as `Net` (SPEC-02 R11-R16b):
- `create_agent(symbol: Symbol) -> AgentId` — insert into the HashMap, increment `next_id`. **(MUST)**
- `remove_agent(id: AgentId)` — remove from the HashMap, remove port entries. **(MUST)**
- `connect(a: PortRef, b: PortRef)` — insert bidirectional entries in the port HashMap; enqueue redex if both are principal ports. **(MUST)**
- `disconnect(port: PortRef)` — remove bidirectional entries from the port HashMap. **(MUST)**
- `get_target(port: PortRef) -> PortRef` — lookup in the port HashMap. **(MUST)**
- `get_agent(id: AgentId) -> Option<&Agent>` — lookup in the agent HashMap. **(MUST)**
- `get_agent_mut(id: AgentId) -> Option<&mut Agent>` — mutable lookup. **(MUST)**
- `is_reduced(&self) -> bool` — redex queue empty check. **(MUST)**
- `count_live_agents(&self) -> usize` — `self.agents.len()`. **(MUST)**
- `live_agents(&self) -> impl Iterator<Item = &Agent>` — iterate over HashMap values. **(MUST)**
**(MUST)**

**R15.** `SparseNet` operations MUST have the following complexity:
- `create_agent`: O(1) amortized (HashMap insert).
- `remove_agent`: O(1) amortized (HashMap remove + up to 3 port removals).
- `connect`: O(1) amortized (2 HashMap inserts + conditional redex enqueue).
- `disconnect`: O(1) amortized (2 HashMap removals).
- `get_target`: O(1) amortized (HashMap lookup).
- `get_agent` / `get_agent_mut`: O(1) amortized (HashMap lookup).
- `count_live_agents`: O(1) (`HashMap::len()`).
**(MUST)**

**R16.** `SparseNet` MUST NOT store entries for consumed agents. When `remove_agent(id)` is called, the agent is removed from `self.agents` and all associated port entries `(id, 0)`, `(id, 1)`, `(id, 2)` are removed from `self.ports`. There are no tombstone slots. Memory is strictly proportional to live agents. **(MUST)**

**R17.** `SparseNet` MUST NOT store port entries for ERA auxiliary ports. Since ERA agents have arity 0, entries `(era_id, 1)` and `(era_id, 2)` MUST NOT exist in the port HashMap. This is the sparse equivalent of SPEC-01 I6 (ERA Auxiliary Slot Cleanliness). **(MUST)**

**R18.** `SparseNet` MUST derive or implement `Debug`, `Clone`, `PartialEq`, `Eq`, `Serialize`, and `Deserialize`. **(MUST)**

**R19.** A conversion function `Net::to_sparse(&self) -> SparseNet` MUST be provided. It MUST:
- Iterate over the dense arena, inserting only `Some(agent)` entries into the sparse agents HashMap.
- For each live agent, copy port entries from the flat port array into the sparse ports HashMap, skipping ERA auxiliary slots and `DISCONNECTED` entries.
- Copy the redex queue, `next_id`, and `root` directly.
- Complexity: O(A) where A is the dense arena length.
**(MUST)**

**R20.** A conversion function `SparseNet::to_dense(&self) -> Net` MUST be provided. It MUST:
- Determine the maximum `AgentId` in the sparse agents HashMap.
- Allocate a `Vec<Option<Agent>>` of size `max_id + 1`, initialized to `None`.
- Allocate a `Vec<PortRef>` of size `(max_id + 1) * 3`, initialized to `DISCONNECTED`.
- Insert all agents and port entries from the sparse representation into the dense vectors.
- Copy the redex queue, `next_id`, and `root` directly.
- Complexity: O(max_id) for allocation + O(live_agents) for insertion.
**(MUST)**

**R21.** Round-trip conversions MUST preserve structural identity:
- `Net::to_sparse().to_dense()` MUST produce a net structurally equal to the original (modulo `None` slots that are trimmed: the resulting dense net has no trailing `None` slots beyond `max_id`).
- `SparseNet::to_dense().to_sparse()` MUST produce a sparse net structurally equal to the original.
**(MUST)**

**R22.** `SparseNet` SHOULD be used as the representation during subnet construction in `build_subnet()` (SPEC-04, `src/partition/helpers.rs`). Under the current `ContiguousIdStrategy`, the last worker's dense subnet allocates `vec![None; max_id + 1]` even when it owns a fraction of the agents. Using `SparseNet` for construction and converting to `Net` before the reduction loop avoids this memory inflation. **(SHOULD)**

**R23.** `SparseNet` MUST NOT be used in the reduction hot path. The reduction engine (SPEC-03) relies on O(1) guaranteed indexed access to `agents[id]` and `ports[id * 3 + port]`. HashMap lookup has O(1) amortized complexity but with a 5-10x worse constant factor due to hashing and cache misses. The hybrid approach (R22: sparse for construction, dense for reduction) avoids this. **(MUST NOT)**

### 3.3 Invariant Amendments

**R24.** Invariant I3 (Monotonicity of AgentIds, SPEC-01) MUST be relaxed for nets with an active free-list. The amended invariant is:

> **I3' (Uniqueness of AgentIds):** Each `AgentId` in use belongs to exactly one live agent in the arena. `AgentId` values in the free-list belong to no agent (`agents[id] == None`). No two live agents share the same `AgentId`. The `next_id` field MUST be strictly greater than any `AgentId` ever assigned (whether currently live, in the free-list, or previously freed and re-assigned).

The change: monotonicity (IDs always increase) is relaxed to uniqueness (IDs are never shared). The `next_id` field remains monotonic and serves as an upper bound on the ID space, but `create_agent` MAY return IDs less than `next_id` when recycling from the free-list. **(MUST)**

**R25.** The relaxation of I3 to I3' MUST NOT violate D4 (ID Uniqueness After Distributed Reduction, SPEC-01). In distributed execution:
- Free-list IDs are bound to the worker's assigned ID range `[start, end)` (R10).
- A recycled ID is reused only within the same partition where it was freed.
- The free-list is partition-local; no cross-partition ID reuse occurs.
- Therefore, D4 (disjointness of ID sets across partitions) is preserved.
**(MUST)**

**R26.** Invariants T1 (Port Linearity), I1 (Bidirectional Consistency), and I2 (Reference Validity) MUST hold for `SparseNet` with adapted verification:
- T1/I1: For every port entry `(a_id, p) -> q` in the sparse ports HashMap, there MUST exist a reverse entry `q -> AgentPort(a_id, p)` (unless `q` is a `FreePort`). The root port exception (SPEC-01 T1) applies unchanged.
- I2: For every `AgentPort(id, p)` value in the sparse ports HashMap, `self.agents.contains_key(&id)` MUST be `true` and `p <= arity(agents[id].symbol)`.
**(MUST)**

**R27.** Debug assertions (SPEC-01, Section 4.3; SPEC-02 R20) MUST be extended to verify free-list consistency:
- After `remove_agent`: the freed ID is in the free-list, `agents[id] == None`, and port slots are `DISCONNECTED`.
- After `create_agent` with recycled ID: the ID is no longer in the free-list, `agents[id] == Some(_)`, and the free-list contains no duplicates.
- Periodic (or per-step in debug mode): no free-list ID is referenced by any `PortRef` in the port array.
**(MUST)**

### 3.4 Configuration

**R28.** Arena recycling MUST be always-on by default (no feature gate). The free-list adds negligible overhead (one `Vec<AgentId>` field, one `pop`/`push` per create/remove) and provides significant memory savings for workloads with high agent turnover. **(MUST)**

**R29.** The `SparseNet` type MUST be always available (no feature gate). It is a data structure, not an optional behavior. Whether a particular code path uses `SparseNet` or `Net` is a design choice at each call site, not a runtime configuration. **(MUST)**

**R30.** The hybrid approach (R22: `SparseNet` for `build_subnet`, `Net` for reduction) SHOULD be configurable via a `sparse_build: bool` flag in `PartitionConfig` or equivalent. Default: `true` (use sparse construction). Setting `false` reverts to the current dense construction for backward compatibility or when the overhead of conversion is undesirable. **(SHOULD)**

---

## 4. Design

### 4.1 Free-List Data Structure

The free-list is a `Vec<AgentId>` used as a stack (LIFO). It is a field of the `Net` struct:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Net {
    pub agents: Vec<Option<Agent>>,
    pub ports: Vec<PortRef>,
    pub redex_queue: VecDeque<(AgentId, AgentId)>,
    pub next_id: AgentId,
    pub root: Option<PortRef>,
    /// Free-list of recycled AgentId slots available for reuse.
    /// LIFO ordering: `create_agent` pops from the end, `remove_agent` pushes to the end.
    /// Invariant: every ID in this list has `agents[id] == None` and port slots == DISCONNECTED.
    pub free_list: Vec<AgentId>,
}
```

### 4.2 Modified `create_agent`

```rust
impl Net {
    /// Creates a new agent with the given symbol.
    /// Returns the assigned AgentId.
    ///
    /// If the free-list is non-empty, reuses a recycled slot (O(1)).
    /// Otherwise, allocates a new slot at next_id (O(1) amortized).
    ///
    /// Postcondition: agents[id] == Some(Agent { symbol, id }).
    /// Invariant I3' (uniqueness): the returned ID is not held by any other live agent.
    pub fn create_agent(&mut self, symbol: Symbol) -> AgentId {
        if let Some(id) = self.free_list.pop() {
            // Reuse a recycled slot
            debug_assert!(
                self.agents[id as usize].is_none(),
                "Free-list invariant violated: slot {} is not None",
                id
            );

            let agent = Agent { symbol, id };
            self.agents[id as usize] = Some(agent);

            // Port slots already exist and should be DISCONNECTED.
            // Re-initialize them defensively.
            let base = (id as usize) * PORTS_PER_SLOT;
            for offset in 0..PORTS_PER_SLOT {
                debug_assert!(
                    self.ports[base + offset] == DISCONNECTED,
                    "Free-list invariant violated: port slot {} is not DISCONNECTED",
                    base + offset
                );
                self.ports[base + offset] = DISCONNECTED;
            }

            id
        } else {
            // No recycled slots available; allocate a new one
            let id = self.next_id;
            self.next_id += 1;

            let agent = Agent { symbol, id };

            if self.agents.len() <= id as usize {
                self.agents.resize((id as usize) + 1, None);
            }
            self.agents[id as usize] = Some(agent);

            let required_len = (id as usize + 1) * PORTS_PER_SLOT;
            if self.ports.len() < required_len {
                self.ports.resize(required_len, DISCONNECTED);
            }

            id
        }
    }
}
```

### 4.3 Modified `remove_agent`

```rust
impl Net {
    /// Removes an agent from the net and adds the slot to the free-list.
    ///
    /// Postcondition: agents[id] == None, port slots == DISCONNECTED, id is in free_list.
    pub fn remove_agent(&mut self, id: AgentId) {
        if let Some(agent) = self.agents[id as usize] {
            let num_ports = total_ports(agent.symbol);
            for p in 0..num_ports {
                let port = PortRef::AgentPort(id, p);
                self.disconnect(port);
            }
            self.agents[id as usize] = None;

            // Push the freed slot onto the free-list for recycling
            self.free_list.push(id);
        }
    }
}
```

### 4.4 SparseNet Type

```rust
use std::collections::{HashMap, VecDeque};

/// Sparse interaction net representation.
///
/// Uses HashMaps instead of Vecs for agents and ports.
/// Memory is proportional to live agents only (no tombstone slots).
/// Intended for construction and partitioning phases (R22, R23).
///
/// NOT intended for the reduction hot path — use Net::to_dense()
/// before reduce_all/reduce_step.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SparseNet {
    /// Live agents, keyed by AgentId.
    pub agents: HashMap<AgentId, Agent>,

    /// Port connections, keyed by (AgentId, PortId).
    /// Only stores entries for live ports (no ERA auxiliary slots,
    /// no DISCONNECTED entries).
    pub ports: HashMap<(AgentId, PortId), PortRef>,

    /// Queue of active pairs (same semantics as Net.redex_queue).
    pub redex_queue: VecDeque<(AgentId, AgentId)>,

    /// Next available AgentId (monotonically increasing).
    pub next_id: AgentId,

    /// Root observation port (same semantics as Net.root).
    pub root: Option<PortRef>,
}
```

### 4.5 SparseNet Operations

```rust
impl SparseNet {
    pub fn new() -> Self {
        SparseNet {
            agents: HashMap::new(),
            ports: HashMap::new(),
            redex_queue: VecDeque::new(),
            next_id: 0,
            root: None,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        SparseNet {
            agents: HashMap::with_capacity(capacity),
            ports: HashMap::with_capacity(capacity * PORTS_PER_SLOT),
            redex_queue: VecDeque::new(),
            next_id: 0,
            root: None,
        }
    }

    pub fn create_agent(&mut self, symbol: Symbol) -> AgentId {
        let id = self.next_id;
        self.next_id += 1;

        let agent = Agent { symbol, id };
        self.agents.insert(id, agent);
        // No port entries created yet; they are created by connect().

        id
    }

    pub fn remove_agent(&mut self, id: AgentId) {
        if let Some(agent) = self.agents.remove(&id) {
            let num_ports = total_ports(agent.symbol);
            for p in 0..num_ports {
                self.disconnect(PortRef::AgentPort(id, p));
            }
        }
    }

    pub fn connect(&mut self, a: PortRef, b: PortRef) {
        // Set bidirectional entries (only for AgentPort endpoints)
        if let PortRef::AgentPort(a_id, a_port) = a {
            self.ports.insert((a_id, a_port), b);
        }
        if let PortRef::AgentPort(b_id, b_port) = b {
            self.ports.insert((b_id, b_port), a);
        }

        // Detect new redex
        if let (PortRef::AgentPort(a_id, 0), PortRef::AgentPort(b_id, 0)) = (a, b) {
            self.redex_queue.push_back((a_id, b_id));
        }
    }

    pub fn disconnect(&mut self, port: PortRef) {
        if let PortRef::AgentPort(id, p) = port {
            if let Some(target) = self.ports.remove(&(id, p)) {
                if let PortRef::AgentPort(t_id, t_port) = target {
                    self.ports.remove(&(t_id, t_port));
                }
            }
        }
    }

    pub fn get_target(&self, port: PortRef) -> PortRef {
        match port {
            PortRef::AgentPort(id, p) => {
                self.ports.get(&(id, p)).copied().unwrap_or(DISCONNECTED)
            }
            PortRef::FreePort(_) => DISCONNECTED,
        }
    }

    pub fn get_agent(&self, id: AgentId) -> Option<&Agent> {
        self.agents.get(&id)
    }

    pub fn get_agent_mut(&mut self, id: AgentId) -> Option<&mut Agent> {
        self.agents.get_mut(&id)
    }

    pub fn is_reduced(&self) -> bool {
        self.redex_queue.is_empty()
    }

    pub fn count_live_agents(&self) -> usize {
        self.agents.len()
    }

    pub fn live_agents(&self) -> impl Iterator<Item = &Agent> {
        self.agents.values()
    }
}
```

### 4.6 Conversion Functions

```rust
impl Net {
    /// Converts this dense Net to a SparseNet.
    /// Skips None slots in the arena and DISCONNECTED entries in the port array.
    /// Complexity: O(arena_len).
    pub fn to_sparse(&self) -> SparseNet {
        let live_count = self.count_live_agents();
        let mut sparse = SparseNet::with_capacity(live_count);

        for agent in self.agents.iter().flatten() {
            sparse.agents.insert(agent.id, *agent);

            let num_ports = total_ports(agent.symbol);
            for p in 0..num_ports {
                let idx = port_index(agent.id, p);
                let target = self.ports[idx];
                if target != DISCONNECTED {
                    sparse.ports.insert((agent.id, p), target);
                }
            }
        }

        sparse.redex_queue = self.redex_queue.clone();
        sparse.next_id = self.next_id;
        sparse.root = self.root;

        sparse
    }
}

impl SparseNet {
    /// Converts this SparseNet to a dense Net.
    /// Allocates arena and port array sized to max_id + 1.
    /// Complexity: O(max_id) for allocation + O(live_agents) for insertion.
    pub fn to_dense(&self) -> Net {
        let max_id = self.agents.keys().max().copied().unwrap_or(0);
        let arena_len = (max_id as usize) + 1;

        let mut net = Net {
            agents: vec![None; arena_len],
            ports: vec![DISCONNECTED; arena_len * PORTS_PER_SLOT],
            redex_queue: self.redex_queue.clone(),
            next_id: self.next_id,
            root: self.root,
            free_list: Vec::new(),
        };

        for (&id, &agent) in &self.agents {
            net.agents[id as usize] = Some(agent);
        }

        for (&(id, port), &target) in &self.ports {
            let idx = port_index(id, port);
            net.ports[idx] = target;
        }

        // Populate free-list with None slots that fall within the ID range
        for i in 0..arena_len {
            if net.agents[i].is_none() {
                net.free_list.push(i as AgentId);
            }
        }

        net
    }
}
```

### 4.7 Free-List Behavior During Reduction

During reduction, the 6 interaction rules call `remove_agent` (which pushes to the free-list) and `create_agent` (which pops from the free-list). The agent balance per rule (SPEC-01 T5):

| Rule | Agents Removed | Agents Created | Free-List Effect |
|------|---------------|----------------|-----------------|
| CON-CON Annihilation | 2 | 0 | +2 to free-list (net grows by 0) |
| DUP-DUP Annihilation | 2 | 0 | +2 to free-list (net grows by 0) |
| ERA-ERA Void | 2 | 0 | +2 to free-list (net grows by 0) |
| CON-DUP Commutation | 2 | 4 | -2 from free-list, then +2 new IDs if free-list was depleted |
| CON-ERA Erasure | 2 | 2 | 0 net change (2 freed, 2 recycled) |
| DUP-ERA Erasure | 2 | 2 | 0 net change (2 freed, 2 recycled) |

For annihilation-dominated workloads (e.g., `ep_annihilation`), the free-list accumulates slots that are never reused — the arena doesn't shrink but also doesn't grow. For commutation-dominated workloads (e.g., `con_dup_expansion`), the free-list provides immediate recycling: the 2 destroyed agents' slots are reused for 2 of the 4 new agents, reducing arena growth by 50%.

---

## 5. Rationale

### 5.1 Why Free-List, Not Full Compaction

ROADMAP 2.33 describes two variants: (a) full compaction with ID renumbering, and (b) free-list recycling without renumbering. This spec implements only the free-list variant because:

1. **No renumbering complexity.** Full compaction requires building a remapping table `old_id -> new_id` and updating every `PortRef::AgentPort(old_id, port)` in the port array, redex queue, `freeport_redirects`, and `root`. This is O(live_agents) and invalidates any external references to old IDs (e.g., the border map during distributed execution). The free-list avoids all of this.

2. **No external reference invalidation.** Free-list recycling is purely local: the ID value stays the same, only the agent occupying that slot changes. No external data structure is invalidated.

3. **Minimal code change.** ~150 LoC: one new field on `Net`, modified `create_agent`, modified `remove_agent`, updated constructors, updated serialization, debug assertions. Full compaction would be ~700 LoC with significantly more surface area for bugs.

4. **Sufficient for the TCC scope.** The free-list prevents unbounded arena growth for all workloads where agent creation and destruction are balanced. For workloads where agents are only created (pure expansion with no annihilation), the arena must grow regardless — neither free-list nor compaction helps.

### 5.2 Why SparseNet is Not Used for Reduction

The reduction hot path (`reduce_step` in SPEC-03) accesses `net.agents[id]` and `net.ports[id * 3 + port]` with O(1) guaranteed indexed access. HashMap provides O(1) amortized access, but:

- **Constant factor.** HashMap lookup involves computing a hash, probing the bucket array, and following a pointer. For `u32` keys, this is ~5-10x slower than a direct array index.
- **Cache misses.** HashMap entries are heap-allocated and scattered in memory. Dense array access benefits from spatial locality and hardware prefetching.
- **Benchmarks.** HVM2 (AC-006) uses a flat array (`node[]`/`vars[]`) specifically for reduction speed. The Haskell prototype (AC-001) uses `Map AgentId Agent` (tree-based, O(log n)) and this is identified as a performance bottleneck.

The hybrid approach (sparse for construction/partitioning, dense for reduction) provides the best of both worlds: memory proportionality during construction and raw speed during reduction.

### 5.3 I3 Relaxation Is Safe

The original I3 (monotonicity) was designed to prevent ID collisions. The free-list preserves the essential property (uniqueness) while relaxing the non-essential property (monotonicity). Specifically:

- **Uniqueness is preserved.** A free-list ID is only reused after the previous occupant has been fully removed (all ports disconnected, agent slot set to `None`). No two live agents ever share an ID.
- **D4 compatibility.** Free-list IDs are constrained to the worker's ID range (R10), so cross-partition collisions cannot occur.
- **`next_id` remains monotonic.** The `next_id` field is only incremented, never decremented. It serves as the upper bound of the ID space for a given net. This is important for `build_subnet` and ID range allocation.

---

## 6. Migration Path

### 6.1 Arena Recycling (Free-List)

1. **Add `free_list: Vec<AgentId>` to `Net` struct** in `src/net/core.rs`. Update `Net::new()`, `Net::with_capacity()`, and `#[derive(Serialize, Deserialize)]`.
2. **Modify `remove_agent`** to push the freed ID onto the free-list after disconnecting ports.
3. **Modify `create_agent`** to pop from the free-list before falling back to `next_id` increment.
4. **Update `build_subnet`** in `src/partition/helpers.rs` to populate partitions' free-lists with `None` slots within each partition's ID range.
5. **Update `merge`** in `src/merge/engine.rs` to construct a valid free-list for the merged net.
6. **Add debug assertions** for free-list invariants (R6, R7, R27).
7. **Update existing tests.** All 690 v1 tests MUST pass without modification (free-list is backward compatible; it simply starts empty and accumulates naturally during reduction).
8. **Add new tests** for free-list-specific scenarios (Section 7).

### 6.2 SparseNet

1. **Create `src/net/sparse.rs`** with the `SparseNet` type and its operations.
2. **Add `mod sparse;` to `src/net/mod.rs`** and export `SparseNet`.
3. **Implement conversion functions** `Net::to_sparse()` and `SparseNet::to_dense()`.
4. **Optionally integrate with `build_subnet`** (R22) behind a configuration flag (R30).
5. **Add tests** for SparseNet operations, invariant preservation, and conversion round-trips.

### 6.3 Ordering

The free-list (6.1) SHOULD be implemented first because it is simpler (~150 LoC vs ~600 LoC), directly benefits the reduction phase (which is the hot path), and does not require a new type. SparseNet (6.2) MAY be implemented independently afterward.

---

## 7. Test Strategy

### 7.1 Free-List Tests

**T1. Basic recycling.** Create 3 agents, remove the middle one, create a new agent. Assert the new agent reuses the middle ID. Assert `agents[middle_id]` is `Some` with the new symbol. Assert the free-list is empty after reuse.

**T2. LIFO ordering.** Create 5 agents (IDs 0-4), remove IDs 1, 3, 2 (in that order). Create 3 new agents. Assert they receive IDs 2, 3, 1 (LIFO pop order). Assert `next_id` is unchanged at 5.

**T3. Free-list exhaustion.** Create 3 agents, remove all 3 (free-list has 3 entries). Create 5 agents. Assert first 3 reuse recycled IDs, last 2 allocate new IDs from `next_id`.

**T4. Port slot reinitialization.** Create a CON agent (ID=0), connect its ports, remove it (pushed to free-list). Create a new ERA agent. Assert the recycled ERA's auxiliary port slots are `DISCONNECTED`. Assert no stale connections from the previous CON agent exist in the port array.

**T5. Reduction with recycling.** Build a net with 100 CON-CON annihilation pairs. Run `reduce_all`. Assert `next_id` is 200 (no new IDs allocated — all annihilations are -2 agents). Assert the free-list contains 200 entries. Assert `count_live_agents() == 0`.

**T6. Commutation recycling.** Build a CON-DUP active pair. Reduce it. Assert the free-list has 0 entries (2 removed, 2 of the 4 created agents reused the freed slots). Assert `next_id == 4` (original 2 + 2 new IDs for the remaining 2 agents).

**T7. Invariant T1 after recycling.** Build a non-trivial net (e.g., Church(3) + Church(2) addition). Run `reduce_all` with free-list enabled. Assert the normal form passes T1 (bidirectionality), I2 (reference validity), and I3' (uniqueness) assertions.

**T8. Serialization round-trip.** Build a net, reduce partially (some free-list entries), serialize, deserialize. Assert the deserialized net's free-list matches the original. Assert continuing reduction produces the same normal form.

**T9. Distributed ID range compliance.** Create a net, partition it with ID ranges `[0, 100)` and `[100, 200)`. During reduction on partition 0, verify that recycled IDs are all in `[0, 100)`. Verify that partition 1's recycled IDs are all in `[100, 200)`.

**T10. Free-list no-duplicate invariant.** Remove the same agent twice (e.g., via direct manipulation in test). Assert that the debug assertion catches the duplicate free-list entry.

### 7.2 SparseNet Tests

**T11. Construction and agent count.** Create a `SparseNet`, add 5 agents of various symbols. Assert `count_live_agents() == 5`. Remove 2 agents. Assert `count_live_agents() == 3`. Assert no tombstones exist (memory is proportional to live agents).

**T12. Bidirectionality (I1 for SparseNet).** Create 3 agents in a SparseNet, connect them in a chain. For every port entry `(id, p) -> target` in the ports HashMap, assert the reverse entry exists. Assert T1 holds.

**T13. ERA cleanliness (I6 for SparseNet).** Create an ERA agent in a SparseNet. Assert that `ports.get(&(era_id, 1))` and `ports.get(&(era_id, 2))` return `None`. Connect the ERA's principal port. Assert auxiliary port entries still do not exist.

**T14. Conversion round-trip (Dense -> Sparse -> Dense).** Build a dense `Net` with 10 agents (including some `None` slots from removals). Convert to `SparseNet`, then back to `Net`. Assert the result is structurally equal to the original (modulo trailing `None` slots and free-list population).

**T15. Conversion round-trip (Sparse -> Dense -> Sparse).** Build a `SparseNet` with 10 agents. Convert to `Net`, then back to `SparseNet`. Assert structural equality.

**T16. Sparse build_subnet.** Build a large net (1000 agents), partition it for 4 workers using `SparseNet`-based construction. Convert each partition to `Net`. Reduce all partitions. Merge. Assert the result is isomorphic to sequential reduction of the original net (G1).

**T17. Redex detection in SparseNet.** Create 2 agents in a SparseNet, connect their principal ports. Assert the redex queue contains the pair. Create 2 more agents, connect auxiliary-to-auxiliary. Assert the redex queue still has exactly 1 entry.

**T18. Serialization round-trip for SparseNet.** Build a SparseNet, serialize with serde + bincode, deserialize. Assert structural equality.

---

## 8. Open Questions

**Q1. Should `SparseNet` support `freeport_redirects`?** The current `Net` has a `freeport_redirects: HashMap<u32, PortRef>` field (used during merge for FreePort resolution). If `SparseNet` is used for partition construction, it may need this field. Alternatively, `freeport_redirects` could be external to both `Net` and `SparseNet`. **Decision deferred to implementation.**

**Q2. Should `SparseNet` implement a trait shared with `Net`?** A `NetOps` trait with `create_agent`, `connect`, `get_target`, etc. would allow polymorphic code. However, SPEC-02 does not define such a trait, and adding one is a larger architectural change. The trait approach is cleaner but adds indirection. **Decision deferred to SPEC review. If adopted, it should be a separate spec amendment.**

**Q3. Free-list memory overhead for very large nets.** For a net with 10M agents that are all annihilated, the free-list holds 10M `u32` entries = 40 MB. This is small compared to the arena itself (10M * `Option<Agent>` = ~80 MB), but not negligible. Should the free-list have a maximum size, beyond which freed IDs are simply discarded? **Tentatively: no cap. 40 MB for 10M agents is acceptable. Revisit if benchmarks show otherwise.**

**Q4. Should the free-list be sorted for deterministic ID allocation?** LIFO ordering means the order of ID reuse depends on the order of `remove_agent` calls, which depends on reduction strategy. This does not affect correctness (I3' guarantees uniqueness regardless of allocation order), but it means two runs with different reduction orders may assign different IDs to the same logical agents. This is already true with the current monotonic allocation (different reduction orders create agents at different `next_id` values). **No sorting needed. Determinism of IDs is not a requirement; determinism of normal form topology is (T6/G1).**
