# TEST-SPEC-0169: Implement net_summary computation

**Task:** TASK-0169
**Spec:** SPEC-12 R29, R30, R31
**Generated:** 2026-04-08 (retroactive)

---

## Unit Tests

### T1: net_summary on empty net returns all zeros

**Type:** Unit
**Input:**
```rust
let net = Net::new();
let summary = net_summary(&net);
```
**Expected:**
- `summary.agents == 0`
- `summary.wires == 0`
- `summary.redexes == 0`
- `summary.con == 0`
- `summary.dup == 0`
- `summary.era == 0`
- `summary.free_ports == 0`
- `summary.normal_form == true`
**Verifies:** R29 -- empty net summary; normal form when 0 redexes

### T2: net_summary counts agents correctly

**Type:** Unit
**Input:**
```rust
let mut net = Net::new();
net.create_agent(Symbol::Con);
net.create_agent(Symbol::Con);
net.create_agent(Symbol::Dup);
net.create_agent(Symbol::Era);
let summary = net_summary(&net);
```
**Expected:** `summary.agents == 4`, `summary.con == 2`, `summary.dup == 1`, `summary.era == 1`
**Verifies:** R29 -- per-symbol counting

### T3: net_summary counts wires (AgentPort-AgentPort pairs only)

**Type:** Unit
**Input:**
```rust
let mut net = Net::new();
let a = net.create_agent(Symbol::Con);
let b = net.create_agent(Symbol::Dup);
net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));
let summary = net_summary(&net);
```
**Expected:** `summary.wires == 1` (only the principal-principal connection counts as a wire; FreePort connections are NOT wires)
**Verifies:** R29 v2 -- wire count excludes FreePort connections

### T4: net_summary counts free ports

**Type:** Unit
**Input:** Same net as T3
**Expected:** `summary.free_ports == 4`
**Verifies:** R29 -- free port count from ports connected to `PortRef::FreePort(_)`

### T5: net_summary counts redexes

**Type:** Unit
**Input:**
```rust
let mut net = Net::new();
let a = net.create_agent(Symbol::Con);
let b = net.create_agent(Symbol::Dup);
net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
// ... wire aux ports to free ...
let summary = net_summary(&net);
```
**Expected:** `summary.redexes == 1`
**Verifies:** R29 -- redex count from redex_queue

### T6: net_summary normal_form is true when redexes == 0

**Type:** Unit
**Input:**
```rust
let mut net = Net::new();
net.create_agent(Symbol::Con); // Isolated agent, no redexes
let summary = net_summary(&net);
```
**Expected:** `summary.normal_form == true`, `summary.redexes == 0`
**Verifies:** R29 -- normal form detection

### T7: net_summary normal_form is false when redexes > 0

**Type:** Unit
**Input:** Create a net with at least one principal-principal connection (redex)
**Expected:** `summary.normal_form == false`
**Verifies:** R29 -- non-normal-form detection

### T8: net_summary respects ERA arity (R61)

**Type:** Unit
**Input:**
```rust
let mut net = Net::new();
let e = net.create_agent(Symbol::Era);
net.connect(PortRef::AgentPort(e, 0), PortRef::FreePort(0));
let summary = net_summary(&net);
```
**Expected:** `summary.free_ports == 1` (only port 0 is iterated for ERA, not ports 1 and 2)
**Verifies:** R61 -- iterate only over ports `0..=arity(symbol)` per live agent

### T9: NetSummary serializes to valid JSON (R30)

**Type:** Unit
**Input:**
```rust
let net = Net::new();
let summary = net_summary(&net);
let json = serde_json::to_string(&summary).unwrap();
```
**Expected:** `json` is valid JSON containing `"agents":0`, `"normal_form":true`
**Verifies:** R30 -- JSON format output

### T10: net_summary on ep_annihilation(10) generator

**Type:** Unit
**Input:**
```rust
let net = ep_annihilation(10);
let summary = net_summary(&net);
```
**Expected:** `summary.agents == 20`, `summary.era == 20`, `summary.redexes == 10`, `summary.normal_form == false`, `summary.wires == 10`
**Verifies:** Cross-validation with known generator output

---

## Edge Cases

### E1: Wire count does not double-count bidirectional connections

**Verify:** For a net with 2 CON agents fully connected (3 wires), `summary.wires == 3` (not 6).
**Why:** Each bidirectional pair is counted once using canonical ordering (lower index first).

### E2: Stale redex entries are counted from queue length

**Verify:** The current implementation uses `net.redex_queue.len()`. If the queue contains stale entries (agents removed), the count may overestimate. Document whether stale filtering is implemented.
**Why:** TASK-0169 notes that stale entries should not be counted. The test verifies the chosen behavior.

### E3: Isolated agents (no connections) contribute zero wires and zero free ports

**Verify:** Create 3 agents with no `connect()` calls. `summary.agents == 3`, `summary.wires == 0`, `summary.free_ports == 0` (ports default to `DISCONNECTED`, which may or may not be `FreePort`; depends on initialization).
**Why:** Documents behavior for agents whose ports have not been explicitly connected.

---

## Property Tests

### P1: Agent count equals con + dup + era

**Property:** For any net `N`, `net_summary(N).agents == net_summary(N).con + net_summary(N).dup + net_summary(N).era`.
**Generator:** Use generators with various sizes: `ep_annihilation(n)`, `con_dup_expansion(n)`, `mixed_rules(n)` for `n` in `1..50`.
**Verifies:** R29 -- per-symbol counts partition the total

### P2: normal_form iff redexes == 0

**Property:** For any net `N`, `net_summary(N).normal_form == (net_summary(N).redexes == 0)`.
**Verifies:** Definition of Normal Form
