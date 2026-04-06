# SPEC-03: Reduction Engine

**Status:** Revised v3
**Depends on:** SPEC-00 (Glossary), SPEC-01 (Invariants), SPEC-02 (Net Representation)
**Gray zones resolved:** ---
**References consumed:** REF-001, REF-002, REF-003, REF-005
**Discussions consumed:** DISC-001 v2 (IC properties, locality, 6 rules), DISC-003 v2 (strong confluence = P1, distributed determinism), DISC-006 v2 (overhead, incremental detection)
**Arguments consumed:** ARG-001 (central argument, P1 = strong confluence)
**Code analyses consumed:** AC-001 (Haskell Core: 6 rules, findRedexes, reduceAll), AC-007 (HVM2: dispatch table, link procedure, interact_* functions), AC-010 (HVM4: WNF evaluator), AC-015 (cross-cutting synthesis: CC-2 incremental redex detection)

---

## 1. Purpose

This spec defines the reduction engine of Relativist: the 6 interaction rules of Lafont's Interaction Combinators with their exact topology, the dispatch mechanism, the reconnection procedure (`link`), the reduction loop (`reduce_step`, `reduce_all`, `reduce_n`), the incremental detection of new redexes during reduction, and the complexity analysis of each operation. The reduction engine is the component that transforms a Net toward its Normal Form, preserving all invariants from SPEC-01.

## 2. Definitions

Terms defined in SPEC-00 (Glossary) are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **Interaction Rule** | One of the 6 topological transformations that consume an Active Pair and produce a new configuration of agents and wires. Each rule is uniquely determined by the pair of symbols of the two agents in the Active Pair. |
| **Dispatch** | The mechanism that, given an Active Pair `(a, b)`, determines which Interaction Rule to apply. In Relativist, implemented as a static 3x3 table indexed by `(a.symbol, b.symbol)`. Inspired by the 8x8 table of HVM2 (AC-007), reduced to the 3 symbols of pure Lafont ICs. |
| **link** | The port reconnection procedure: connects two `PortRef` values via `Net::connect` and detects new redexes on-the-fly. Inspired by the `link` procedure of HVM2 (AC-007), but simplified because Relativist has no VAR (variable) type. |
| **Stale Redex** | See SPEC-02, Section 2. The reduction engine MUST discard stale redexes silently (SPEC-01, I4). |
| **Interaction Counter** | A `u64` counter that records the number of interaction rules applied. Managed by the caller (`reduce_all`, `reduce_n`) via `ReductionStats`, not as a field on `Net`. Incremented by the caller for each `StepResult::Reduced` returned by `reduce_step`. Invariant T7 (SPEC-01) guarantees that the final value is identical for any reduction strategy on the same terminating net. |

---

## 3. Requirements

### 3.1 Interaction Rules

**R1.** The reduction engine MUST implement exactly 6 interaction rules, covering all possible pairs from `{Con, Dup, Era}`. No pair of symbols may be left without a rule. **(MUST)**

**R2.** Each rule MUST produce the exact topology specified by Lafont (REF-002, Fig. 2, p.82), as detailed in Section 4.1. **(MUST)**

**R3.** Each rule MUST preserve invariant T1 (port linearity) from SPEC-01: after applying the rule, every port of every live agent is connected to exactly one other port. **(MUST)**

**R4.** Each rule MUST preserve invariant I1 (bidirectional port array) from SPEC-01: every connection is symmetric. **(MUST)**

**R5.** Each rule MUST preserve invariant I2 (reference validity) from SPEC-01: every `AgentPort(id, p)` reference in the port array points to an existing agent with a valid port index. **(MUST)**

**R6.** Each rule MUST preserve invariant I3 (ID monotonicity) from SPEC-01: `next_id` is strictly greater than any `AgentId` in use. **(MUST)**

**R7.** Each rule MUST preserve invariant T5 (rule correctness) from SPEC-01: the topological result matches Lafont's specification. **(MUST)**

### 3.2 Dispatch

**R8.** The dispatch mechanism MUST map each pair of symbols `(Symbol, Symbol)` to exactly one rule constant in O(1). **(MUST)**

**R9.** The dispatch MUST normalize symmetric pairs before applying the rule: if `(a.symbol, b.symbol)` is a pair where the implementation requires a fixed order (e.g., `(Dup, Con)` normalized to `(Con, Dup)`), normalization MUST occur before the rule function is called. **(MUST)**

### 3.3 Link Procedure

**R10.** The link procedure MUST delegate to `Net::connect(a, b)` (SPEC-02, R13), which establishes a bidirectional connection and detects new redexes incrementally. **(MUST)**

**R11.** New redex detection MUST be on-the-fly: when `connect` creates a connection between two principal ports (`AgentPort(_, 0)` and `AgentPort(_, 0)`), the pair MUST be inserted into the redex queue automatically. **(MUST)**

### 3.4 Reduction Loop

**R12.** The function `reduce_step` MUST: (1) dequeue the next pair from the redex queue, (2) verify that the pair is valid (not stale), (3) if invalid, discard it and try the next, (4) if valid, determine the rule via dispatch and apply it, (5) return the applied rule so that callers can maintain interaction counts via `ReductionStats`. **(MUST)**

- **Note:** The interaction counter is managed by the caller (`reduce_all`, `reduce_n`), not by `reduce_step` itself. `reduce_step` returns `StepResult::Reduced(Rule)` on success, and the caller increments `ReductionStats` accordingly. This avoids adding mutable counter state to `Net`.

**R13.** The function `reduce_all` MUST apply `reduce_step` repeatedly until the redex queue is empty (Normal Form reached). It MUST return the total number of interactions performed. **(MUST)**

**R14.** The function `reduce_n(budget: usize)` MUST apply at most `budget` interactions and return the number of interactions actually performed. If Normal Form is reached before the budget is exhausted, it MUST stop immediately. **(MUST)**

**R15.** In debug mode (`#[cfg(debug_assertions)]`), the reduction functions MUST invoke `Net::assert_all_invariants()` after each interaction rule is applied (cf. SPEC-02, R20). **(MUST)**

**R16.** In release mode, assertions MAY be disabled for performance. **(MAY)**

**R17.** The reduction engine SHOULD discriminate the interaction counter by rule type (annihilation, commutation, erasure, void) to enable workload profiling. **(SHOULD)**

### 3.5 Incremental Redex Detection

**R18.** The reduction engine MUST use incremental redex detection via `Net::connect` rather than a global scan. No `findRedexes`-style O(N) scan per step is permitted. **(MUST)**

- **Justification:** The Haskell prototype's `findRedexes` is O(w) per step, resulting in O(S * w) total (AC-001, Limitation L1). Incremental detection via `connect` is O(1) per connection, resulting in O(S) total. This is the primary complexity optimization identified in AC-015 (CC-2) and the main architectural improvement of Relativist over the prototype.

**R19.** The redex queue MUST tolerate stale entries: when a pair `(a, b)` is dequeued, the engine MUST verify that both agents exist and are still connected through their principal ports (cf. SPEC-01, I4, and SPEC-02, R17). Invalid entries MUST be silently discarded. **(MUST)**

### 3.6 Complexity

**R20.** Each interaction rule MUST execute in O(1) amortized (constant time, independent of net size). The amortized qualifier accounts for `create_agent` potentially triggering Vec reallocation during commutation and erasure rules (cf. SPEC-02, Section 4.5.2). **(MUST)**

**R21.** `reduce_step` MUST execute in O(1) amortized (excluding stale redexes discarded; each stale discard is O(1)). **(MUST)**

**R22.** `reduce_all` MUST execute in O(S) where S is the total number of interactions to Normal Form. Invariant T7 from SPEC-01 guarantees that S is unique for a given terminating net. **(MUST)**

### 3.7 Strong Confluence Preservation

**R23.** The reduction engine MUST preserve the strong confluence property (T4 from SPEC-01, equivalent to Premise P1 from ARG-001). This means: the engine MUST NOT impose any ordering constraint on which redex is selected from the queue. The correctness of the system MUST NOT depend on the order of redex processing. **(MUST)**

- **Justification:** Strong confluence (REF-002, Proposition 1, p.73) guarantees that any two reductions of disjoint Active Pairs commute. The reduction engine leverages this by processing redexes in arbitrary (FIFO) order from the queue. No synchronization or ordering is necessary for correctness. This is Premise P1 of the formal argument framework (DISC-003 v2, Section 1.1; ARG-001, P1).

**R24.** The reduction engine MUST operate by in-place mutation of the Net rather than creating a new Net at each step. **(MUST)**

- **Justification:** The Haskell prototype creates a new immutable Net at each reduction step (AC-001, Limitation L3), causing O(A + W) allocation per step. In-place mutation reduces per-step cost to O(1), which is essential for performance. This follows the approach of HVM2 (AC-007) and HVM4 (AC-009).

### 3.8 Self-Referencing Auxiliary Ports

**R25.** When an annihilation rule (CON-CON or DUP-DUP) has auxiliary ports that point back to the pair being consumed (i.e., `a.1` is connected to `b.2`, or more generally any `target(a.p)` equals `AgentPort(b.id, q)` or `target(b.p)` equals `AgentPort(a.id, q)`), the `link` calls that would reconnect those ports MUST be no-ops. Both agents and all their connections are fully consumed; no ghost entries may remain in the port array. **(MUST)**

- **Justification:** In the "read neighbors, remove, reconnect" pattern, `remove_agent` disconnects all ports and marks the slot as `None`. The saved `PortRef` values (`a1_target`, etc.) still hold `AgentPort` references to the now-removed agents. If `connect` writes to these slots unconditionally, it creates ghost entries that violate I2 (reference validity). The guard ensures that when the entire active pair's auxiliary ports form a closed structure, both agents cleanly vanish with no residual port array entries.

### 3.9 Reduction with FreePort (Boundary) Sentinels

**R26.** During local reduction within a partitioned sub-net, auxiliary ports MAY be connected to `FreePort(bid)` boundary sentinels. The reduction rules treat `FreePort` targets identically to `AgentPort` targets during the `get_target` phase (reading the PortRef from the port array). During the `link` phase, `Net::connect` writes `FreePort(bid)` to the port array of the `AgentPort` side, but `set_port` cannot write back to the `FreePort` side (which has no port array slot). This one-sided write is acceptable because the `free_port_index` is reconstructed post-reduction by scanning the port array (SPEC-04, R14; SPEC-05, Section 4.3 `rebuild_free_port_index`). Invariant I1 (bidirectional consistency) is temporarily violated for `FreePort` connections during local reduction; it is restored after `rebuild_free_port_index`. **(MUST)**

- **Justification:** The lazy reconstruction approach (SPEC-05, Section 5.3) was chosen specifically to avoid modifying the reduction engine to maintain the `free_port_index`. The reduction engine does NOT need to know about partitioning or boundary sentinels. It simply reads and writes `PortRef` values through the standard `get_target`/`connect` interface. The `free_port_index` is a concern of the partitioner (SPEC-04) and merger (SPEC-05), reconstructed before merge.

---

## 4. Design

### 4.1 The 6 Interaction Rules

Each rule transforms an Active Pair (two agents connected through their principal ports) into a new configuration. This specification uses the following notation:

- `a`, `b`: the two agents of the Active Pair (connected port 0 <-> port 0)
- `a.1`, `a.2`: the auxiliary ports of `a` (ports 1 and 2)
- `b.1`, `b.2`: the auxiliary ports of `b` (ports 1 and 2)
- `target(p)`: the PortRef to which port `p` is connected (via `Net::get_target`)
- `link(x, y)`: connect PortRefs `x` and `y` (via `Net::connect`, which detects new redexes)

**Convention:** In all rules, the steps are: (1) read the neighbors of auxiliary ports, (2) remove the agents involved, (3) create new agents if necessary, (4) reconnect ports via `link`.

**Invariant cross-reference:** Each rule preserves the invariants listed. The invariant identifiers refer to SPEC-01. Additionally, all rules preserve T2 (interaction exclusively via principal ports): new redexes inserted by `connect` satisfy T2 by construction, because `connect` only inserts pairs into the redex queue when both endpoints are `AgentPort(_, 0)` (principal ports). T2 is not listed individually per rule to avoid redundancy.

---

#### 4.1.1 Annihilation CON-CON (gamma-gamma) -- Cross

**Active pair:** `Con(a) <port0-port0> Con(b)`
**Rule category:** Annihilation (same symbol, arity 2)

**Topology BEFORE:**
```
  target(a.1) ---[a.1]--- CON(a) ---[a.0 >< b.0]--- CON(b) ---[b.1]--- target(b.1)
                          |                                     |
  target(a.2) ---[a.2]---+                                     +---[b.2]--- target(b.2)
```

**Topology AFTER (cross-connect):**
```
  target(a.1) ------------------------------------------------- target(b.2)
  target(a.2) ------------------------------------------------- target(b.1)
```

**Algorithm:**
```
  let a1_target = net.get_target(AgentPort(a.id, 1))
  let a2_target = net.get_target(AgentPort(a.id, 2))
  let b1_target = net.get_target(AgentPort(b.id, 1))
  let b2_target = net.get_target(AgentPort(b.id, 2))

  net.remove_agent(a.id)
  net.remove_agent(b.id)

  link(a1_target, b2_target)    // CROSS: a.1 <-> b.2
  link(a2_target, b1_target)    // CROSS: a.2 <-> b.1
```

| Metric | Value |
|--------|-------|
| Agents destroyed | 2 |
| Agents created | 0 |
| Agent balance | -2 |
| `link` calls | 2 |

**Invariants preserved:**
- T1 (linearity): The 4 auxiliary ports previously connected to `a` and `b` are reconnected in pairs. No port is left disconnected.
- I1 (bidirectionality): `link` calls `Net::connect`, which writes both sides.
- T3 (disjointness): Agents `a` and `b` are removed. No other Active Pair is affected because T3 guarantees `a` and `b` do not participate in any other pair.

**New redex detection:** Up to 2 new redexes may form. If `a1_target` or `a2_target` (after reconnection) is the principal port of another agent, a new Active Pair is detected by `link` and inserted into the queue.

**Reference:** REF-002 p.82 (Fig. 2, first rule), AC-001 `ruleCON_CON` lines 232-253.

**Note on cross vs. parallel:** The cross pattern (a.1<->b.2, a.2<->b.1) distinguishes CON-CON from DUP-DUP. This asymmetry is essential for the universality of the system (REF-002 p.90; SPEC-00 Sections 3.3 and 3.4).

---

#### 4.1.2 Annihilation DUP-DUP (delta-delta) -- Parallel

**Active pair:** `Dup(a) <port0-port0> Dup(b)`
**Rule category:** Annihilation (same symbol, arity 2)

**Topology BEFORE:**
```
  target(a.1) ---[a.1]--- DUP(a) ---[a.0 >< b.0]--- DUP(b) ---[b.1]--- target(b.1)
                          |                                     |
  target(a.2) ---[a.2]---+                                     +---[b.2]--- target(b.2)
```

**Topology AFTER (parallel-connect):**
```
  target(a.1) ------------------------------------------------- target(b.1)
  target(a.2) ------------------------------------------------- target(b.2)
```

**Algorithm:**
```
  let a1_target = net.get_target(AgentPort(a.id, 1))
  let a2_target = net.get_target(AgentPort(a.id, 2))
  let b1_target = net.get_target(AgentPort(b.id, 1))
  let b2_target = net.get_target(AgentPort(b.id, 2))

  net.remove_agent(a.id)
  net.remove_agent(b.id)

  link(a1_target, b1_target)    // PARALLEL: a.1 <-> b.1
  link(a2_target, b2_target)    // PARALLEL: a.2 <-> b.2
```

| Metric | Value |
|--------|-------|
| Agents destroyed | 2 |
| Agents created | 0 |
| Agent balance | -2 |
| `link` calls | 2 |

**Invariants preserved:** Identical to CON-CON (Section 4.1.1). The only difference is the reconnection topology (parallel vs. cross).

**New redex detection:** Up to 2, same mechanism as CON-CON.

**Reference:** REF-002 p.82 (Fig. 2, second rule), AC-001 `ruleDUP_DUP` lines 267-286.

**Edge case -- self-referencing auxiliary ports (R25):** Both annihilation rules (CON-CON and DUP-DUP) may encounter a scenario where the auxiliary ports of `a` are connected to the auxiliary ports of `b`, forming a closed structure. For example, in a CON-CON pair where `a.1 <-> b.2` and `a.2 <-> b.1`:

```
  a.1 ------- b.2
  CON(a) ---[p0><p0]--- CON(b)
  a.2 ------- b.1
```

After reading neighbors: `a1_target = AgentPort(b, 2)`, `a2_target = AgentPort(b, 1)`, `b1_target = AgentPort(a, 2)`, `b2_target = AgentPort(a, 1)`. After `remove_agent(a)` and `remove_agent(b)`, all saved PortRef values point to removed agents. The `link` calls MUST be guarded: if either endpoint of a `link` call is an `AgentPort` whose agent has been removed, the `link` is a no-op (the wire has been fully consumed). The correct result is: both agents vanish cleanly with no residual port array entries.

The same pattern applies to DUP-DUP when `a.1 <-> b.1` and `a.2 <-> b.2`. A partial self-reference (e.g., only `a.1 <-> b.2`, with `a.2` and `b.1` connected to other agents) is also possible: the self-referencing `link` is a no-op while the other `link` proceeds normally.

See Section 4.5 for the `link` helper implementation that provides this guard.

---

#### 4.1.3 Annihilation ERA-ERA (epsilon-epsilon) -- Void

**Active pair:** `Era(a) <port0-port0> Era(b)`
**Rule category:** Void (same symbol, arity 0)

**Topology BEFORE:**
```
  ERA(a) ---[a.0 >< b.0]--- ERA(b)
```

**Topology AFTER:**
```
  (nothing)
```

**Algorithm:**
```
  net.remove_agent(a.id)
  net.remove_agent(b.id)

  // No reconnection: ERA has arity 0 (no auxiliary ports)
```

| Metric | Value |
|--------|-------|
| Agents destroyed | 2 |
| Agents created | 0 |
| Agent balance | -2 |
| `link` calls | 0 |

**Invariants preserved:**
- T1 (linearity): ERA has no auxiliary ports. Removing the two agents and disconnecting the principal port (done by `remove_agent`) does not leave dangling ports.
- I1/I2: `remove_agent` marks slots as `DISCONNECTED` and removes agents from the arena.

**New redex detection:** 0 new redexes (no reconnections).

**Reference:** REF-002 p.82, AC-001 `ruleERA_ERA` lines 295-297, AC-007 `interact_void`.

**Note:** This is the simplest rule. In HVM2 (AC-007), it is implemented as a near-no-op (`interact_void` returns `true` without node operations) because ERA does not occupy a node slot. In Relativist, agents occupy arena slots and MUST be explicitly removed.

---

#### 4.1.4 Commutation CON-DUP (gamma-delta) -- Expand

**Active pair:** `Con(a) <port0-port0> Dup(b)` (or `Dup <-> Con`, normalized to this order)
**Rule category:** Commutation (different symbols, both arity 2)

This is the ONLY rule that INCREASES the number of agents in the net.

**Topology BEFORE:**
```
  target(a.1) ---[a.1]--- CON(a) ---[a.0 >< b.0]--- DUP(b) ---[b.1]--- target(b.1)
                          |                                     |
  target(a.2) ---[a.2]---+                                     +---[b.2]--- target(b.2)
```

**Topology AFTER:**
```
  target(a.1) ---[p.0]--- DUP(p) ---[p.1]--- CON(r) ---[r.0]--- target(b.1)
                          |                   |
                          +---[p.2]---+       +---[r.2]---+
                                      |                   |
  target(a.2) ---[q.0]--- DUP(q) ---[q.1]--- CON(s) ---[s.0]--- target(b.2)
                          |                   |
                          +---[q.2]-----------+---[s.2]---+
                                                          |
                                              (q.2 <-> s.1)
```

**Detailed connections (8 wires total):**

External wires (4):
```
  DUP(p).port0 <-> target(a.1)    // p inherits the position of a.1
  DUP(q).port0 <-> target(a.2)    // q inherits the position of a.2
  CON(r).port0 <-> target(b.1)    // r inherits the position of b.1
  CON(s).port0 <-> target(b.2)    // s inherits the position of b.2
```

Internal wires (4, in crossed configuration):
```
  DUP(p).port1 <-> CON(r).port1
  DUP(p).port2 <-> CON(s).port1
  DUP(q).port1 <-> CON(r).port2
  DUP(q).port2 <-> CON(s).port2
```

**Algorithm:**
```
  let a1_target = net.get_target(AgentPort(a.id, 1))
  let a2_target = net.get_target(AgentPort(a.id, 2))
  let b1_target = net.get_target(AgentPort(b.id, 1))
  let b2_target = net.get_target(AgentPort(b.id, 2))

  net.remove_agent(a.id)    // Remove original CON
  net.remove_agent(b.id)    // Remove original DUP

  // Create 4 new agents: 2 DUP + 2 CON
  let p = net.create_agent(Symbol::Dup)    // New DUP (inherits side of a.1)
  let q = net.create_agent(Symbol::Dup)    // New DUP (inherits side of a.2)
  let r = net.create_agent(Symbol::Con)    // New CON (inherits side of b.1)
  let s = net.create_agent(Symbol::Con)    // New CON (inherits side of b.2)

  // External wires: principal ports of new agents <-> old neighbors
  link(AgentPort(p, 0), a1_target)
  link(AgentPort(q, 0), a2_target)
  link(AgentPort(r, 0), b1_target)
  link(AgentPort(s, 0), b2_target)

  // Internal wires: auxiliary ports of new agents to each other (crossed)
  link(AgentPort(p, 1), AgentPort(r, 1))
  link(AgentPort(p, 2), AgentPort(s, 1))
  link(AgentPort(q, 1), AgentPort(r, 2))
  link(AgentPort(q, 2), AgentPort(s, 2))
```

| Metric | Value |
|--------|-------|
| Agents destroyed | 2 |
| Agents created | 4 (2 DUP + 2 CON) |
| Agent balance | **+2** |
| `link` calls | 8 (4 external + 4 internal) |

**Invariants preserved:**
- T1 (linearity): All 12 ports of the 4 new agents (4 x 3 ports) are connected: 4 principal ports to old neighbors, 8 auxiliary ports among themselves in pairs. No port is left disconnected.
- I1 (bidirectionality): Each `link` call establishes a bidirectional connection.
- I3 (monotonicity): `create_agent` increments `next_id` for each new agent.
- T3 (disjointness): New redexes may be created by external wires (if `a1_target`, `a2_target`, `b1_target`, or `b2_target` are principal ports of other agents). These new redexes are detected automatically by `link` and inserted into the queue.

**New redex detection:** The 4 external wires connect principal ports of new agents (`AgentPort(p, 0)`, etc.) to ports of the context. If any of those context ports is also a principal port (`AgentPort(_, 0)`), a new redex is formed and inserted into the queue by `link`. Up to 4 new redexes. The 4 internal wires connect auxiliary ports, so they do NOT generate redexes (none is port 0).

**Reference:** REF-002 p.82 (Fig. 2, fourth rule), AC-001 `ruleCON_DUP` lines 329-381, AC-007 `interact_comm` lines 755-794.

**Importance for Grid Computing:** This rule is responsible for the EXPANSION of available work. Without it, nets would only shrink. The granularity of partitioning depends directly on how many CON-DUP expansions have occurred (AC-001, "Semantics" section; DISC-006 v2, Section 1.1).

---

#### 4.1.5 Erasure CON-ERA (gamma-epsilon)

**Active pair:** `Con(a) <port0-port0> Era(b)` (or `Era <-> Con`, normalized)
**Rule category:** Erasure (arity-2 agent meets arity-0 agent)

**Topology BEFORE:**
```
  target(a.1) ---[a.1]--- CON(a) ---[a.0 >< b.0]--- ERA(b)
                          |
  target(a.2) ---[a.2]---+
```

**Topology AFTER:**
```
  target(a.1) ---[e1.0]--- ERA(e1)
  target(a.2) ---[e2.0]--- ERA(e2)
```

**Algorithm:**
```
  let a1_target = net.get_target(AgentPort(a.id, 1))
  let a2_target = net.get_target(AgentPort(a.id, 2))

  net.remove_agent(a.id)    // Remove CON
  net.remove_agent(b.id)    // Remove ERA

  // Create 2 new ERA, one for each auxiliary port of the CON
  let e1 = net.create_agent(Symbol::Era)
  let e2 = net.create_agent(Symbol::Era)

  link(AgentPort(e1, 0), a1_target)    // ERA propagates to a.1
  link(AgentPort(e2, 0), a2_target)    // ERA propagates to a.2
```

| Metric | Value |
|--------|-------|
| Agents destroyed | 2 (1 CON + 1 ERA) |
| Agents created | 2 (2 ERA) |
| Agent balance | 0 |
| `link` calls | 2 |

**Invariants preserved:**
- T1 (linearity): The 2 auxiliary ports of the CON that pointed to the context are replaced by the principal ports of 2 new ERA agents. Each new ERA has arity 0, so only its principal port needs connection.
- I3 (monotonicity): 2 new IDs generated by `create_agent`.

**New redex detection:** If `a1_target` or `a2_target` is the principal port of some agent, the new ERA forms a redex with that agent. This initiates an erasure cascade: ERA propagates through the sub-net until it meets other ERA agents (terminating with ERA-ERA void) or reaches free ports.

**Reference:** REF-002 p.82, AC-001 `ruleErase` lines 383-441.

---

#### 4.1.6 Erasure DUP-ERA (delta-epsilon)

**Active pair:** `Dup(a) <port0-port0> Era(b)` (or `Era <-> Dup`, normalized)
**Rule category:** Erasure (arity-2 agent meets arity-0 agent)

**Topology BEFORE:**
```
  target(a.1) ---[a.1]--- DUP(a) ---[a.0 >< b.0]--- ERA(b)
                          |
  target(a.2) ---[a.2]---+
```

**Topology AFTER:**
```
  target(a.1) ---[e1.0]--- ERA(e1)
  target(a.2) ---[e2.0]--- ERA(e2)
```

**Algorithm:** Identical to CON-ERA (Section 4.1.5), substituting CON for DUP. The topology is identical because the erasure rule depends only on the arity of the non-ERA agent (arity 2 for both CON and DUP).

| Metric | Value |
|--------|-------|
| Agents destroyed | 2 (1 DUP + 1 ERA) |
| Agents created | 2 (2 ERA) |
| Agent balance | 0 |
| `link` calls | 2 |

**New redex detection:** Up to 2, same mechanism as CON-ERA.

**Reference:** REF-002 p.82, AC-001 `ruleErase` lines 383-441 (same function as CON-ERA).

---

### 4.2 Summary Table of the 6 Rules

| Rule | Active Pair | Reconnection | Agents: destr/crea/balance | Links | Function |
|------|-------------|--------------|----------------------------|-------|----------|
| Annihilation (cross) | Con-Con | aux crossed | 2/0/-2 | 2 | `interact_anni` |
| Annihilation (parallel) | Dup-Dup | aux parallel | 2/0/-2 | 2 | `interact_anni` |
| Void | Era-Era | nothing | 2/0/-2 | 0 | `interact_void` |
| Commutation (expand) | Con-Dup | 4 new agents | 2/4/+2 | 8 | `interact_comm` |
| Erasure | Con-Era | 2 new ERA | 2/2/0 | 2 | `interact_eras` |
| Erasure | Dup-Era | 2 new ERA | 2/2/0 | 2 | `interact_eras` |

### 4.3 Dispatch: 3x3 Table

Relativist adopts a static 3x3 dispatch table indexed by `(Symbol, Symbol)`, inspired by the 8x8 table of HVM2 (AC-007) but reduced to Lafont's pure IC system with 3 symbols.

```rust
/// Rule constants for dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Rule {
    /// Annihilation: same symbol, arity 2 (CON-CON or DUP-DUP).
    Anni = 0,
    /// Commutation: CON-DUP or DUP-CON (different symbols, both arity 2).
    Comm = 1,
    /// Erasure: ERA with CON or DUP (erasure propagation).
    Eras = 2,
    /// Void: ERA-ERA (erasers annihilate).
    Void = 3,
}
```

```rust
/// 3x3 dispatch table. Indexed by (Symbol, Symbol).
/// Symmetric: TABLE[a][b] == TABLE[b][a].
///
///          Con    Dup    Era
/// Con  [  ANNI   COMM   ERAS ]
/// Dup  [  COMM   ANNI   ERAS ]
/// Era  [  ERAS   ERAS   VOID ]
///
pub const DISPATCH_TABLE: [[Rule; 3]; 3] = [
    // Con row
    [Rule::Anni, Rule::Comm, Rule::Eras],
    // Dup row
    [Rule::Comm, Rule::Anni, Rule::Eras],
    // Era row
    [Rule::Eras, Rule::Eras, Rule::Void],
];

/// Determines the interaction rule for a pair of symbols.
/// Complexity: O(1) (index lookup).
#[inline]
pub fn get_rule(a: Symbol, b: Symbol) -> Rule {
    DISPATCH_TABLE[a as usize][b as usize]
}
```

### 4.4 Pair Normalization

Before calling an interaction function, the pair MUST be normalized so that the implementation can assume a fixed argument order.

```rust
/// Normalizes the agent pair for dispatch.
/// Guarantees a.symbol <= b.symbol (by Symbol ordering: Con=0, Dup=1, Era=2).
/// Swaps arguments if necessary.
///
/// This allows:
/// - interact_anni to always receive (Con, Con) or (Dup, Dup), never inverted
/// - interact_comm to always receive (Con, Dup), never (Dup, Con)
/// - interact_eras to always receive (X, Era) where X is Con or Dup, never (Era, X)
/// - interact_void to receive (Era, Era)
#[inline]
pub fn normalize_pair(a: AgentId, b: AgentId, net: &Net) -> (AgentId, AgentId) {
    let sym_a = net.agents[a as usize].unwrap().symbol;
    let sym_b = net.agents[b as usize].unwrap().symbol;
    if (sym_a as u8) <= (sym_b as u8) {
        (a, b)
    } else {
        (b, a)
    }
}
```

This normalization is inspired by `should_swap` in HVM2 (AC-007) and by the symmetric pair handling in the Haskell prototype (AC-001, Pattern T2). It allows:
- `interact_eras` to assume without verification that the first argument is the arity-2 agent (CON or DUP) and the second is ERA.
- `interact_comm` to assume the first is CON and the second is DUP.

### 4.5 Interaction Functions

The 6 logical rules are implemented in 4 functions, following the unification of isomorphic rules from the Haskell prototype (AC-001, Pattern T3):

| Function | Rules covered | Parameters |
|----------|--------------|------------|
| `interact_anni` | CON-CON (cross), DUP-DUP (parallel) | `(net, a_id, b_id)` |
| `interact_comm` | CON-DUP | `(net, con_id, dup_id)` |
| `interact_eras` | CON-ERA, DUP-ERA | `(net, node_id, era_id)` |
| `interact_void` | ERA-ERA | `(net, a_id, b_id)` |

```rust
/// Safe link: wraps Net::connect with a guard for removed agents (R25).
///
/// If either endpoint is an AgentPort whose agent has been removed
/// (agents[id] is None), the link is a no-op. This handles the
/// self-referencing auxiliary port edge case in annihilation rules.
///
/// For non-annihilation rules (commutation, erasure), the guard is
/// never triggered because auxiliary ports of the active pair always
/// point to agents outside the pair (or to FreePort sentinels).
fn link(net: &mut Net, a: PortRef, b: PortRef) {
    let is_removed = |net: &Net, p: &PortRef| -> bool {
        if let PortRef::AgentPort(id, _) = p {
            net.agents[*id as usize].is_none()
        } else {
            false // FreePort is not "removed"; connect handles it
        }
    };
    if is_removed(net, &a) || is_removed(net, &b) {
        return;
    }
    net.connect(a, b);
}
```

```rust
/// Annihilation: two agents of the SAME symbol annihilate each other.
/// CON-CON: reconnection in CROSS pattern (a.1<->b.2, a.2<->b.1)
/// DUP-DUP: reconnection in PARALLEL pattern (a.1<->b.1, a.2<->b.2)
///
/// Precondition: both agent IDs MUST refer to live agents
///   (agents[id].is_some()). This precondition is guaranteed by
///   reduce_step's validity check (R12). Calling this function
///   with removed agents is undefined behavior.
/// Postcondition: a and b removed; auxiliary ports reconnected
///   (or no-op'd if self-referencing, per R25).
pub fn interact_anni(net: &mut Net, a_id: AgentId, b_id: AgentId) {
    let sym = net.agents[a_id as usize].unwrap().symbol;

    let a1 = net.get_target(PortRef::AgentPort(a_id, 1));
    let a2 = net.get_target(PortRef::AgentPort(a_id, 2));
    let b1 = net.get_target(PortRef::AgentPort(b_id, 1));
    let b2 = net.get_target(PortRef::AgentPort(b_id, 2));

    net.remove_agent(a_id);
    net.remove_agent(b_id);

    match sym {
        Symbol::Con => {
            // CROSS: a.1 <-> b.2, a.2 <-> b.1
            link(net, a1, b2);
            link(net, a2, b1);
        }
        Symbol::Dup => {
            // PARALLEL: a.1 <-> b.1, a.2 <-> b.2
            link(net, a1, b1);
            link(net, a2, b2);
        }
        _ => unreachable!("interact_anni called with non-arity-2 symbol"),
    }
}
```

```rust
/// Commutation: CON and DUP commute, creating 4 new agents.
/// This is the ONLY rule that INCREASES the number of agents (balance +2).
///
/// Precondition: both agent IDs MUST refer to live agents
///   (agents[id].is_some()). con_id MUST be Con, dup_id MUST be Dup
///   (normalized by normalize_pair). This precondition is guaranteed
///   by reduce_step's validity check (R12).
/// Postcondition: CON and DUP removed; 2 new DUP + 2 new CON created
///   and reconnected. PortRef values are index-based, not pointer-based:
///   Vec reallocation during create_agent does NOT invalidate previously
///   read PortRef values.
pub fn interact_comm(net: &mut Net, con_id: AgentId, dup_id: AgentId) {
    let a1 = net.get_target(PortRef::AgentPort(con_id, 1));
    let a2 = net.get_target(PortRef::AgentPort(con_id, 2));
    let b1 = net.get_target(PortRef::AgentPort(dup_id, 1));
    let b2 = net.get_target(PortRef::AgentPort(dup_id, 2));

    net.remove_agent(con_id);
    net.remove_agent(dup_id);

    // Create 4 new agents
    let p = net.create_agent(Symbol::Dup);   // DUP: inherits side of con.1
    let q = net.create_agent(Symbol::Dup);   // DUP: inherits side of con.2
    let r = net.create_agent(Symbol::Con);   // CON: inherits side of dup.1
    let s = net.create_agent(Symbol::Con);   // CON: inherits side of dup.2

    // External wires: principal ports of new agents <-> old neighbors
    // Note: old neighbors (a1, a2, b1, b2) may be FreePort(bid) in
    // partitioned sub-nets. The link helper handles this correctly:
    // connect writes FreePort to the AgentPort side's port array,
    // and the FreePort side is a no-op in set_port (R26).
    link(net, PortRef::AgentPort(p, 0), a1);
    link(net, PortRef::AgentPort(q, 0), a2);
    link(net, PortRef::AgentPort(r, 0), b1);
    link(net, PortRef::AgentPort(s, 0), b2);

    // Internal wires: auxiliary ports of new agents to each other (crossed)
    // These are always AgentPort-to-AgentPort (never FreePort).
    net.connect(PortRef::AgentPort(p, 1), PortRef::AgentPort(r, 1));
    net.connect(PortRef::AgentPort(p, 2), PortRef::AgentPort(s, 1));
    net.connect(PortRef::AgentPort(q, 1), PortRef::AgentPort(r, 2));
    net.connect(PortRef::AgentPort(q, 2), PortRef::AgentPort(s, 2));
}
```

```rust
/// Erasure: ERA encounters an arity-2 agent (CON or DUP).
/// Propagates erasure: creates 2 new ERA agents on the auxiliary ports.
///
/// Precondition: both agent IDs MUST refer to live agents
///   (agents[id].is_some()). node_id MUST be Con or Dup, era_id
///   MUST be Era (normalized by normalize_pair). This precondition
///   is guaranteed by reduce_step's validity check (R12).
/// Postcondition: both removed; 2 new ERA connected to old neighbors.
pub fn interact_eras(net: &mut Net, node_id: AgentId, era_id: AgentId) {
    let a1 = net.get_target(PortRef::AgentPort(node_id, 1));
    let a2 = net.get_target(PortRef::AgentPort(node_id, 2));

    net.remove_agent(node_id);
    net.remove_agent(era_id);

    // Create 2 new ERA
    let e1 = net.create_agent(Symbol::Era);
    let e2 = net.create_agent(Symbol::Era);

    // Connect new ERA to old neighbors of auxiliary ports
    // Note: a1/a2 may be FreePort(bid) in partitioned sub-nets (R26).
    link(net, PortRef::AgentPort(e1, 0), a1);
    link(net, PortRef::AgentPort(e2, 0), a2);
}
```

```rust
/// Void: two ERA agents annihilate without creating anything.
///
/// Precondition: both agent IDs MUST refer to live agents
///   (agents[id].is_some()) and both MUST be Era. This precondition
///   is guaranteed by reduce_step's validity check (R12).
/// Postcondition: both removed. No agents created, no reconnections.
pub fn interact_void(net: &mut Net, a_id: AgentId, b_id: AgentId) {
    net.remove_agent(a_id);
    net.remove_agent(b_id);
}
```

### 4.6 Reduction Loop

#### 4.6.1 reduce_step

```rust
/// Result of a single reduction step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepResult {
    /// A redex was successfully reduced. Contains the applied rule.
    Reduced(Rule),
    /// The queue is empty: net is in Normal Form.
    NormalForm,
}
```

```rust
/// Executes a single reduction step.
///
/// Dequeues pairs from the redex queue until a valid (non-stale) one is found,
/// applies the corresponding rule, and returns the result.
///
/// Complexity: O(1) amortized (each stale discard is O(1);
/// rule application is O(1)).
pub fn reduce_step(net: &mut Net) -> StepResult {
    loop {
        // 1. Dequeue next pair
        let (a_id, b_id) = match net.redex_queue.pop_front() {
            Some(pair) => pair,
            None => return StepResult::NormalForm,
        };

        // 2. Verify validity (discard stale)
        if !net.is_valid_redex(a_id, b_id) {
            continue;
        }

        // 3. Normalize pair for dispatch
        let (a, b) = normalize_pair(a_id, b_id, net);

        // 4. Determine rule
        let sym_a = net.agents[a as usize].unwrap().symbol;
        let sym_b = net.agents[b as usize].unwrap().symbol;
        let rule = get_rule(sym_a, sym_b);

        // 5. Apply rule
        match rule {
            Rule::Anni => interact_anni(net, a, b),
            Rule::Comm => interact_comm(net, a, b),
            Rule::Eras => interact_eras(net, a, b),
            Rule::Void => interact_void(net, a, b),
        }

        // 6. Verify invariants in debug mode
        #[cfg(debug_assertions)]
        net.assert_all_invariants();

        return StepResult::Reduced(rule);
    }
}
```

#### 4.6.2 reduce_all

```rust
/// Statistics of a completed reduction.
#[derive(Debug, Clone)]
pub struct ReductionStats {
    /// Total number of interactions performed.
    pub total_interactions: u64,
    /// Number of interactions by rule type.
    pub anni_count: u64,
    pub comm_count: u64,
    pub eras_count: u64,
    pub void_count: u64,
}
```

```rust
/// Reduces the net to Normal Form (empty queue).
///
/// WARNING: does not terminate if the net is non-terminating.
/// For potentially non-terminating nets, use reduce_n.
///
/// Complexity: O(S) where S is the total number of interactions to Normal Form
/// (invariant T7 from SPEC-01 guarantees S is unique for the given net).
///
/// Returns reduction statistics.
pub fn reduce_all(net: &mut Net) -> ReductionStats {
    let mut stats = ReductionStats {
        total_interactions: 0,
        anni_count: 0,
        comm_count: 0,
        eras_count: 0,
        void_count: 0,
    };

    loop {
        match reduce_step(net) {
            StepResult::NormalForm => return stats,
            StepResult::Reduced(rule) => {
                stats.total_interactions += 1;
                match rule {
                    Rule::Anni => stats.anni_count += 1,
                    Rule::Comm => stats.comm_count += 1,
                    Rule::Eras => stats.eras_count += 1,
                    Rule::Void => stats.void_count += 1,
                }
            }
        }
    }
}
```

#### 4.6.3 reduce_n

```rust
/// Reduces the net by at most `budget` interactions.
///
/// Useful for:
/// - Granularity control in the grid (workers execute a budget and return
///   a partial result).
/// - Safeguard against non-terminating nets (SPEC-01, I5).
///
/// Returns statistics of the interactions performed (may be < budget
/// if Normal Form is reached before the budget is exhausted).
pub fn reduce_n(net: &mut Net, budget: usize) -> ReductionStats {
    let mut stats = ReductionStats {
        total_interactions: 0,
        anni_count: 0,
        comm_count: 0,
        eras_count: 0,
        void_count: 0,
    };

    for _ in 0..budget {
        match reduce_step(net) {
            StepResult::NormalForm => return stats,
            StepResult::Reduced(rule) => {
                stats.total_interactions += 1;
                match rule {
                    Rule::Anni => stats.anni_count += 1,
                    Rule::Comm => stats.comm_count += 1,
                    Rule::Eras => stats.eras_count += 1,
                    Rule::Void => stats.void_count += 1,
                }
            }
        }
    }

    stats
}
```

### 4.7 Link Procedure (Port Reconnection)

In Relativist, port reconnection during interaction rules uses a `link` helper function (defined in Section 4.5) that wraps `Net::connect` (SPEC-02, Section 4.5.4) with a guard for removed agents (R25). The `connect` function already:

1. Establishes a bidirectional connection in the port array.
2. Detects whether both ports are principal ports (`AgentPort(_, 0)`) and, if so, inserts the pair into the redex queue.

The `link` helper adds a safety check: if either endpoint is an `AgentPort` of a removed agent, the `link` is a no-op. This handles the self-referencing auxiliary port edge case in annihilation rules (R25). For internal wires of commutation (where both endpoints are freshly created agents), `net.connect` is called directly for clarity and efficiency.

**FreePort behavior during link (R26):** When one endpoint of a `link` call is a `FreePort(bid)` (as occurs in partitioned sub-nets), `connect` writes `FreePort(bid)` to the `AgentPort` side's port array slot, but `set_port` is a no-op for the `FreePort` side (which has no port array slot). This one-sided write is acceptable: the `free_port_index` is reconstructed by scanning the port array after local reduction (SPEC-05, Section 4.3). Invariant I1 is temporarily violated for `FreePort` connections during reduction but is restored after reconstruction.

**Difference from HVM2:** In HVM2 (AC-007), the `link` procedure is more complex because it includes variable resolution (VAR), path compression, and atomic ownership via CAS. In Relativist, there are NO variables (VAR): all ports are `AgentPort` or `FreePort`. Therefore, `link` reduces to a guard check + bidirectional `connect` + redex detection.

**Difference from the Haskell prototype:** In the prototype (AC-001), there is no `link` procedure. Each rule constructs wires manually via `Wire a b`. Detection of new redexes is done by the global scan `findRedexes` at the beginning of each step. Relativist eliminates this scan by replacing it with incremental detection in `connect`.

### 4.8 Incremental New Redex Detection

Detection of new redexes during reduction follows the on-the-fly pattern (CC-2 from AC-015):

1. Each rule reconnects ports via `net.connect(a, b)`.
2. `connect` checks: if `a = AgentPort(id_a, 0)` and `b = AgentPort(id_b, 0)`, it inserts `(id_a, id_b)` into the redex queue.
3. `reduce_step` consumes from the queue, verifies validity (stale check), and applies the rule.

**When new redexes appear:**

| Rule | Possible new redexes | Mechanism |
|------|---------------------|-----------|
| CON-CON (anni) | Up to 2 | If `a1_target` or `a2_target` (after reconnection) is a principal port of another agent |
| DUP-DUP (anni) | Up to 2 | Same |
| ERA-ERA (void) | 0 | No reconnections |
| CON-DUP (comm) | Up to 4 | The 4 external wires connect principal ports of new agents to the context |
| CON-ERA (eras) | Up to 2 | The 2 new ERA connect to the context via principal ports |
| DUP-ERA (eras) | Up to 2 | Same |

**Stale redexes:** The queue may contain invalid entries. Scenarios:
- Agent `x` participated in redex `(x, y)`. Agent `x` was consumed by another rule. Pair `(x, y)` is now stale.
- An annihilation rule reconnects ports such that the principal-principal connection changes. The old pair is stale.

The cost of stale redexes is amortized: each stale is discarded in O(1) (lookup in the agent vector + connection verification). In the worst case (many stale), the additional cost is proportional to the number of stale discarded, which is bounded by the total number of reconnections performed.

**Note on `is_reduced()` and stale entries:** The function `is_reduced()` (SPEC-02, R16) checks whether the redex queue is empty. This is a necessary but not sufficient condition for Normal Form when stale entries exist in the queue. The canonical way to verify Normal Form is to call `reduce_all` (which drains all stale entries by processing them in the loop) and then confirm that no new entries were generated. Do NOT use `is_reduced()` as a standalone termination check without first processing stale entries. After `reduce_all` returns, `is_reduced()` is guaranteed to return `true`.

### 4.9 ID Generation During Reduction

The CON-DUP (4 new agents), CON-ERA (2 new agents), and DUP-ERA (2 new agents) rules create new agents via `net.create_agent(symbol)`, which:

1. Assigns the next available ID (`net.next_id`).
2. Increments `net.next_id`.
3. Expands the agent arena and port array if necessary.

**Complexity:** O(1) amortized per `create_agent` (may trigger Vec realloc).

**Vec reallocation safety:** `PortRef` values are index-based (`AgentId` + `PortId`), not pointer-based. Vec reallocation during `create_agent` changes the backing memory address of `agents` and `ports`, but does NOT invalidate previously read `PortRef` values because they store integer indices, not raw pointers. The saved values (`a1`, `a2`, `b1`, `b2`) remain valid after reallocation.

**In the distributed context (SPEC-04, SPEC-05):** Each worker has a pre-allocated ID range. The `next_id` is initialized to the start of the worker's range. The same `create_agent` logic works without modification; the only difference is the initial value of `next_id`. This satisfies invariant D4 (ID uniqueness after distributed reduction, SPEC-01).

### 4.10 Complexity Analysis

#### Per Rule

| Rule | Operations | Complexity |
|------|-----------|-----------|
| CON-CON (anni) | 4 get_target + 2 remove_agent + 2 connect | O(1) |
| DUP-DUP (anni) | 4 get_target + 2 remove_agent + 2 connect | O(1) |
| ERA-ERA (void) | 2 remove_agent | O(1) |
| CON-DUP (comm) | 4 get_target + 2 remove_agent + 4 create_agent + 8 connect | O(1) amortized |
| CON-ERA (eras) | 2 get_target + 2 remove_agent + 2 create_agent + 2 connect | O(1) amortized |
| DUP-ERA (eras) | 2 get_target + 2 remove_agent + 2 create_agent + 2 connect | O(1) amortized |

All operations (`get_target`, `remove_agent`, `create_agent`, `connect`) are O(1) as per SPEC-02. `create_agent` is O(1) amortized due to potential Vec reallocation. Therefore, rules that create agents (comm, eras) are O(1) amortized; rules that do not create agents (anni, void) are O(1) worst-case.

**Comparison with the Haskell prototype (AC-001):** In the prototype, each rule is O(w) where w is the total number of wires, due to the linear scan of `portNeighbor` and `removeAgent`. The improvement from O(w) to O(1) per rule is the primary complexity optimization of Relativist.

#### Per reduce_step

O(1) amortized. Each call executes:
- `pop_front` from VecDeque: O(1)
- `is_valid_redex`: O(1) (two lookups in the agent vector + one get_target)
- `normalize_pair`: O(1) (two lookups in the agent vector)
- `get_rule`: O(1) (table lookup)
- The rule function: O(1) (as above)

Stale redexes add O(1) per stale discarded. The total cost to discard K stale is O(K), amortized over the interactions that created them.

#### For reduce_all

O(S + K) where S is the number of interactions to Normal Form and K is the total number of stale redexes discarded. K is bounded by O(S * c) where c is the maximum number of reconnections per rule (8 for CON-DUP). In practice, K << S for most nets.

Invariant T7 from SPEC-01 guarantees that S is identical for any reduction strategy on the same net.

**Comparison:**

| System | reduce_all cost | Source |
|--------|----------------|--------|
| Haskell prototype | O(S * w) | findRedexes O(w) per step (AC-001 L1) |
| HVM2 | O(S) | On-the-fly detection (AC-007) |
| Relativist | O(S) | Incremental detection via connect |

---

## 5. Rationale

### 5.1 3x3 Table vs. Match vs. Switch

**Decision:** Static 3x3 table.

**Alternatives considered:**
- **Direct pattern match** (as in the Haskell prototype, AC-001): `match (sym_a, sym_b) { (Con, Con) => ..., ... }` with 9 branches. Advantage: readable, exhaustive (the Rust compiler verifies coverage). For 3 symbols and 9 combinations, perfectly adequate.
- **Nested switch** (as in HVM4, AC-010): `switch(tag_frame) { switch(tag_whnf) { ... } }`. Superior for many types (30+ tags in HVM4), but excessive for 3 symbols.

The 3x3 table was chosen because:
- It cleanly separates the dispatch decision (data) from rule implementation (logic), making independent testing easier.
- It is symmetric by construction: `TABLE[a][b] == TABLE[b][a]`.
- It is trivially extensible if additional symbols are ever added (out of TCC scope).
- For 3 symbols, there is no practical performance difference between table and match. The choice is primarily one of architectural clarity.

The implementer MAY replace the table with a direct `match` if preferred; correctness does not depend on the dispatch strategy, only on the correct mapping of pairs to rules.

### 5.2 Four Functions vs. Six Functions

**Decision:** 4 functions (`interact_anni`, `interact_comm`, `interact_eras`, `interact_void`) covering 6 rules.

**Justification:**
- `interact_anni` unifies CON-CON and DUP-DUP because the logic is identical except for the reconnection direction (cross vs. parallel), determined by an internal branch on the symbol. This follows the HVM2 pattern (AC-007, `interact_anni` unifies all annihilations).
- `interact_eras` unifies CON-ERA and DUP-ERA because the topology is identical: propagate ERA through the auxiliary ports of an arity-2 agent. This follows the Haskell prototype pattern (AC-001, `ruleErase`).
- `interact_void` (ERA-ERA) and `interact_comm` (CON-DUP) are unique cases.

**Rejected alternative:** 6 separate functions (one per rule). Advantage: each function is self-contained, easier to test individually. Disadvantage: code duplication between CON-ERA and DUP-ERA, and between CON-CON and DUP-DUP.

### 5.3 Pair Normalization

**Decision:** Normalize to `sym_a <= sym_b` before dispatch.

**Justification:** Allows `interact_eras` to assume without verification that the first argument is the arity-2 agent and the second is ERA. Without normalization, each rule function would need an additional branch to determine which argument is which. The cost of normalization is O(1) (two symbol reads + comparison).

### 5.4 Incremental Detection vs. Global Scan

**Decision:** On-the-fly detection via `Net::connect`.

**Justification:** The Haskell prototype's global scan `findRedexes` is O(w) per step, resulting in O(S * w) total. Incremental detection is O(1) per connection, resulting in O(S) total. For nets with thousands of wires and hundreds of reduction steps, the savings are orders of magnitude. This is the primary bottleneck identified in AC-001 (L1) and the main recommendation of AC-015 (CC-2). DISC-006 v2 confirms that the dominant overhead factor in distributed reduction is communication, not reduction itself -- making O(1)-per-rule reduction essential to ensure that local computation is not the bottleneck.

### 5.5 FIFO vs. Dual-Queue

**Decision:** Simple VecDeque (FIFO), as specified in SPEC-02.

**Alternative considered:** Dual-queue hi/lo as in HVM2 (AC-007): annihilation/erasure/void in the hi queue (high priority), commutation in the lo queue (low priority). Advantage: prioritizing destructive rules (which shrink the net) may reduce peak memory usage.

**Justification for rejection:** Strong confluence (T4, SPEC-01) guarantees that order is irrelevant for correctness. Prioritization is an empirical optimization whose benefit depends on the workload and is only measurable via benchmarks (AC-015, Z8). For the TCC scope, FIFO is sufficient. If benchmarks reveal that peak memory is a problem, migration to dual-queue is possible without altering the reduction logic. This decision aligns with R23 (no ordering constraint imposed on redex selection).

### 5.6 In-Place Mutation vs. Immutable Nets

**Decision:** In-place mutation of the Net.

**Justification:** The Haskell prototype creates a new immutable Net at each reduction step (AC-001, Limitation L3), which entails O(A + W) allocation per step and significant GC pressure. In-place mutation reduces per-step allocation to O(1) (amortized, only when CON-DUP creates new agents that may trigger Vec growth). This follows the approach of HVM2 (AC-007) and HVM4 (AC-009), both of which mutate their nets in-place. The correctness of in-place mutation relies on the disjointness property (T3, SPEC-01): since an Active Pair's agents do not participate in any other Active Pair, consuming them does not corrupt any concurrent reduction.

---

## 6. Haskell Prototype Reference

### 6.1 Function Mapping

| Haskell (AC-001) | Relativist | Change | Reason |
|------------------|------------|--------|--------|
| `applyRule` (pattern match, 9 cases) | `get_rule` (3x3 table) + `match rule` (4 branches) | Separate dispatch from implementation | Clarity and extensibility |
| `ruleCON_CON` (O(6w)) | `interact_anni` (O(1), Con case) | O(w) -> O(1) | Port array eliminates wire scan |
| `ruleDUP_DUP` (O(6w)) | `interact_anni` (O(1), Dup case) | O(w) -> O(1), unified with CON-CON | Same logic, branch by symbol |
| `ruleERA_ERA` (O(2w)) | `interact_void` (O(1)) | O(w) -> O(1) | `remove_agent` is O(1) with port array |
| `ruleCON_DUP` (O(6w + 5 log n)) | `interact_comm` (O(1)) | O(w + log n) -> O(1) | Port array + bump counter |
| `ruleErase` (CON-ERA/DUP-ERA, O(5w + 3 log n)) | `interact_eras` (O(1)) | O(w + log n) -> O(1) | Port array + bump counter |
| `findRedexes` (O(w) per step) | Incremental detection via `connect` (O(1) per connection) | O(S*w) -> O(S) total | Primary optimization |
| `reduceAll` (O(S*w)) | `reduce_all` (O(S)) | O(S*w) -> O(S) | All optimizations combined |
| `reduceN` (O(min(n,S)*w)) | `reduce_n` (O(min(n,S))) | Same | Same |
| `nextAgentId` (O(log n)) | `net.next_id` (O(1)) | Map.findMax -> bump counter | SPEC-02 decision |

### 6.2 What Worked in the Prototype

1. **"Read neighbors, remove old, create new, reconnect" pattern** (AC-001, Pattern T7): Preserved in Relativist. The difference is that each operation is O(1) instead of O(w).

2. **Symmetric pair normalization** (AC-001, Pattern T2): Preserved and formalized as `normalize_pair`.

3. **Unified `ruleErase`** (AC-001, Pattern T3): Preserved as `interact_eras`.

4. **`reduceN` as granularity control** (AC-001, Pattern T5): Preserved as `reduce_n`.

### 6.3 What Relativist Changes

1. **From global scan to incremental detection:** `findRedexes` O(w) eliminated. Detection is O(1) per `connect`. Total savings: O(S * w) -> O(S).

2. **From pattern match over wires to O(1) port array lookup:** `portNeighbor` O(w) replaced by `get_target` O(1). Savings per rule: from 4-6 scans of O(w) to 4-6 lookups of O(1).

3. **From new Net creation per step to in-place mutation:** The Haskell prototype creates a new immutable Net at each reduction (AC-001, L3). Relativist mutates the Net in-place, eliminating allocation and GC pressure.

4. **From `removeAgent` with filter O(w) to `remove_agent` O(1):** The prototype filters ALL wires to remove those referencing the agent (AC-001, L4). Relativist disconnects ports directly in the port array.

5. **From `nextAgentId = findMax + 1` O(log n) to bump counter O(1):** (AC-001, L6). SPEC-02 decision.

6. **Added: interaction count by rule type.** The prototype does not count interactions. Relativist returns `ReductionStats` with a breakdown by type, following the recommendation of AC-007 and AC-015 (CC-8).

---

## 7. Open Questions

None. All design decisions necessary to implement the reduction engine are covered:

- The 6 rules are specified with exact topology and pseudocode.
- The dispatch mechanism is defined (3x3 table).
- The reduction loop is defined (reduce_step, reduce_all, reduce_n).
- Redex detection is defined (incremental via connect).
- Pair normalization is defined.
- Self-referencing auxiliary ports in annihilation are handled by the `link` guard (R25).
- FreePort boundary sentinels during local reduction are documented (R26).
- Interaction counter management is clarified (caller-managed via ReductionStats).
- Preconditions for all interact_* functions are explicitly documented.
- Complexity is analyzed.
- Preserved invariants are identified for each rule.
- The relationship between each rule and the formal invariants (T1-T7, I1-I4, D4) is established.
- The relationship to the strong confluence property (P1) is made explicit in R23.
