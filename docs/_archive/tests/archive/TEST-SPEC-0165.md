# TEST-SPEC-0165: Text DSL parser - net construction and validation (Pass 2)

**Task:** TASK-0165
**Spec:** SPEC-12 R7, R8, R9, R10, R11, R51, R54, R55, R56, R57, R58, R59
**Generated:** 2026-04-08 (retroactive)

---

## Unit Tests

### T1: CON-CON annihilation example (R13)

**Type:** Unit
**Input:**
```
agent a CON
agent b CON
wire a.principal b.principal
wire a.left b.left
wire a.right b.right
```
**Expected:** Net has 2 agents, 3 wires, 1 redex (principals connected), `redex_queue.len() == 1`
**Verifies:** R13, R10 -- correct construction from the canonical example

### T2: CON-DUP commutation example (R14)

**Type:** Unit
**Input:**
```
agent c CON
agent d DUP
wire c.principal d.principal
wire c.left free(0)
wire c.right free(1)
wire d.left free(2)
wire d.right free(3)
```
**Expected:** Net has 2 agents, 4 free ports, 1 redex
**Verifies:** R14 -- commutation example with free ports

### T3: ERA agents wired at principals succeeds

**Type:** Unit
**Input:**
```
agent e ERA
agent f ERA
wire e.principal f.principal
```
**Expected:** Net has 2 ERA agents, 1 redex
**Verifies:** R9 -- ERA principal port wiring is allowed

### T4: ERA agent with auxiliary port reference is rejected (R9)

**Type:** Unit
**Input:** `"agent e ERA\nwire e.left free(0)"`
**Expected:** Returns `Err(...)` containing `"no auxiliary ports"` and a line number
**Verifies:** R9 -- ERA agents cannot use left/right/p1/p2

### T5: ERA agent with p1 alias also rejected (R9)

**Type:** Unit
**Input:** `"agent e ERA\nwire e.p1 free(0)"`
**Expected:** Returns `Err(...)` containing `"no auxiliary ports"`
**Verifies:** R9 -- ERA rejection applies to p1/p2 aliases too

### T6: Unknown agent name in wire produces error with line number

**Type:** Unit
**Input:** `"agent a CON\nwire a.principal unknown.principal"`
**Expected:** Returns `Err(...)` containing `"unknown agent 'unknown'"` and line number
**Verifies:** R11 -- reference validity check

### T7: parse_ic("") returns empty net

**Type:** Unit
**Input:** `parse_ic("")`
**Expected:** Returns `Ok(Net)` with 0 agents, 0 wires, `net.root == None`
**Verifies:** R55 -- empty input is valid

### T8: parse_ic on R13 example produces net with 1 redex

**Type:** Unit
**Input:** Full R13 example text
**Expected:** `net.redex_queue.len() == 1`
**Verifies:** Pass 2 correctly detects principal-principal connections as redexes

### T9: Duplicate root declaration produces error (R54)

**Type:** Unit
**Input:**
```
agent a CON
root a.principal
root a.left
```
**Expected:** Returns `Err(...)` containing `"duplicate root declaration at line"`
**Verifies:** R54 -- at most one root declaration per file

### T10: No root declaration means net.root is None (R55)

**Type:** Unit
**Input:** `"agent a CON"`
**Expected:** `net.root == None`
**Verifies:** R55 -- absence of root declaration

### T11: Root with agent port reference (R56)

**Type:** Unit
**Input:**
```
agent a CON
wire a.left free(0)
wire a.right free(1)
root a.principal
```
**Expected:** `net.root == Some(PortRef::AgentPort(0, 0))`
**Verifies:** R56 -- root can be an agent port

### T12: Root with free port reference (R56)

**Type:** Unit
**Input:** `"root free(0)"`
**Expected:** `net.root == Some(PortRef::FreePort(0))`
**Verifies:** R56 -- root can be a free port

### T13: Self-loop wire rejected (R58)

**Type:** Unit
**Input:** `"agent a CON\nwire a.left a.left"`
**Expected:** Returns `Err(...)` containing `"port cannot be connected to itself"`
**Verifies:** R58 -- self-loop detection

### T14: Free-to-free wire rejected (R59)

**Type:** Unit
**Input:** `"agent a CON\nwire free(0) free(1)"`
**Expected:** Returns `Err(...)` containing `"free-to-free wires are not supported"`
**Verifies:** R59 -- both endpoints must not be free ports

### T15: Root declaration with missing port ref produces error

**Type:** Unit
**Input:** `"root"`
**Expected:** Returns `Err(...)` containing `"requires exactly one port reference"`
**Verifies:** R7 -- malformed root declaration

---

## Edge Cases

### E1: Root is not counted as a wire (R57)

**Verify:** Parse a net with one agent and a root declaration but no wire declarations. The net should have 0 wires (root is metadata, not a connection).
```
agent a CON
root a.principal
```
**Why:** R57 explicitly states root is not a wire and not counted in wire count.

### E2: Multiple wire declarations for the same port pair

**Verify:** Wiring the same port twice (e.g., `wire a.left free(0)` then `wire a.left free(1)`) should result in the second wire overwriting the first, or producing a validation error (depending on implementation). The test documents whichever behavior the implementation chose.
**Why:** Port linearity (T1, SPEC-01) implies each port connects to exactly one target.

### E3: Very large free port ID

**Verify:** `parse_ic("agent a CON\nwire a.principal free(999999)")` succeeds and creates `PortRef::FreePort(999999)`.
**Why:** Free port IDs are u32; large values should work.

### E4: Agent names are case-sensitive

**Verify:** `parse_ic("agent A CON\nagent a DUP\n")` creates two distinct agents (A and a).
**Why:** Per R7, IDENT uses `[a-zA-Z_]` so upper and lower case are distinct.
