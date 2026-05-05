# TEST-SPEC-0166: Text DSL serializer (format_ic)

**Task:** TASK-0166
**Spec:** SPEC-12 R15, R51
**Generated:** 2026-04-08 (retroactive)

---

## Unit Tests

### T1: format_ic on empty net

**Type:** Unit
**Input:**
```rust
let net = Net::new();
let text = format_ic(&net);
```
**Expected:** `text` is empty (or contains only whitespace/newline)
**Verifies:** Empty net produces no declarations

### T2: format_ic emits agent declarations in order

**Type:** Unit
**Input:**
```rust
let mut net = Net::new();
net.create_agent(Symbol::Con);
net.create_agent(Symbol::Dup);
net.create_agent(Symbol::Era);
let text = format_ic(&net);
```
**Expected:** `text` contains lines `"agent a0 CON"`, `"agent a1 DUP"`, `"agent a2 ERA"` in that order
**Verifies:** TASK-0166 AC -- agents named `a<id>`, emitted in AgentId order

### T3: format_ic uses human-readable port names

**Type:** Unit
**Input:**
```rust
let mut net = Net::new();
let a = net.create_agent(Symbol::Con);
let b = net.create_agent(Symbol::Con);
net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));
let text = format_ic(&net);
```
**Expected:** `text` contains `"principal"`, `"left"`, `"right"` (not `p0`, `p1`, `p2`)
**Verifies:** TASK-0166 AC -- human-readable port name forms

### T4: format_ic emits ERA agents with only principal

**Type:** Unit
**Input:**
```rust
let mut net = Net::new();
let e = net.create_agent(Symbol::Era);
let f = net.create_agent(Symbol::Era);
net.connect(PortRef::AgentPort(e, 0), PortRef::AgentPort(f, 0));
let text = format_ic(&net);
```
**Expected:** Output contains `"agent a0 ERA"`, `"agent a1 ERA"`, one wire line with `"a0.principal"` and `"a1.principal"`. No `"left"` or `"right"` appears.
**Verifies:** TASK-0166 AC -- ERA agents emit only principal port references

### T5: format_ic emits free ports as free(id)

**Type:** Unit
**Input:**
```rust
let mut net = Net::new();
let a = net.create_agent(Symbol::Con);
net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));
net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(1));
net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(2));
let text = format_ic(&net);
```
**Expected:** `text` contains `"free(0)"`, `"free(1)"`, `"free(2)"`
**Verifies:** TASK-0166 AC -- free port syntax in output

### T6: Each wire appears exactly once (no duplicates)

**Type:** Unit
**Input:**
```rust
let mut net = Net::new();
let a = net.create_agent(Symbol::Con);
let b = net.create_agent(Symbol::Con);
net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));
let text = format_ic(&net);
```
**Expected:** Count of lines starting with `"wire"` is exactly 3 (one per connection, not 6 for both directions)
**Verifies:** TASK-0166 AC -- bidirectional pairs emitted exactly once

### T7: format_ic emits root when net.root is Some

**Type:** Unit
**Input:**
```rust
let mut net = Net::new();
let a = net.create_agent(Symbol::Con);
net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
net.root = Some(PortRef::AgentPort(a, 0));
let text = format_ic(&net);
```
**Expected:** `text` contains the line `"root a0.principal"`
**Verifies:** TASK-0166 AC -- root line emitted

### T8: format_ic omits root when net.root is None

**Type:** Unit
**Input:**
```rust
let mut net = Net::new();
net.create_agent(Symbol::Con);
let text = format_ic(&net);
```
**Expected:** `text` does NOT contain the word `"root"`
**Verifies:** TASK-0166 AC -- no root line when root is None

### T9: Roundtrip: parse_ic(format_ic(net)) == net

**Type:** Unit
**Input:**
```rust
let mut net = Net::new();
let a = net.create_agent(Symbol::Con);
let b = net.create_agent(Symbol::Era);
net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
let text = format_ic(&net);
let reparsed = parse_ic(&text).unwrap();
```
**Expected:** `reparsed.count_live_agents() == 2`, agents and connections match the original
**Verifies:** R15 -- roundtrip identity

### T10: Roundtrip with root preservation

**Type:** Unit
**Input:**
```rust
let mut net = Net::new();
let a = net.create_agent(Symbol::Con);
net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));
net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(1));
net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(2));
net.root = Some(PortRef::FreePort(99));
let text = format_ic(&net);
let reparsed = parse_ic(&text).unwrap();
```
**Expected:** `reparsed.root == Some(PortRef::FreePort(99))`
**Verifies:** R15 -- roundtrip preserves root declaration

---

## Edge Cases

### E1: Dead agents (holes in arena) are skipped

**Verify:** Create a net with 3 agents, remove agent 1 (leave a hole), then `format_ic`. The output should contain `"agent a0"` and `"agent a2"` but NOT `"agent a1"`.
**Why:** The serializer must handle sparse agent arenas gracefully.

### E2: Canonical wire ordering is deterministic

**Verify:** Format the same net twice. The output strings are identical byte-for-byte.
**Why:** Without canonical ordering, iteration order could vary and break determinism.

### E3: Output ends with newline

**Verify:** `format_ic(&non_empty_net).ends_with('\n')` is true.
**Why:** TASK-0166 AC specifies trailing newline.

---

## Property Tests

### P1: Roundtrip identity for generated nets

**Property:** For any net `N` from `ep_annihilation(n)` where `n` in `1..50`, `parse_ic(format_ic(N))` succeeds and produces a net with the same agent count and redex count.
**Verifies:** R15 -- roundtrip identity as a universal property
