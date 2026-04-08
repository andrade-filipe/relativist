# TEST-SPEC-0164: Text DSL parser - lexing and declaration collection (Pass 1)

**Task:** TASK-0164
**Spec:** SPEC-12 R6, R7, R8, R10, R12
**Generated:** 2026-04-08 (retroactive)

---

## Unit Tests

### T1: Parse blank line

**Type:** Unit
**Input:** `parse_ic("")`
**Expected:** Returns `Ok(Net)` with 0 agents
**Verifies:** R12 -- blank lines are handled gracefully

### T2: Parse comment line

**Type:** Unit
**Input:** `parse_ic("# this is a comment\n")`
**Expected:** Returns `Ok(Net)` with 0 agents
**Verifies:** R12 -- comments starting with `#` are ignored

### T3: Parse multiple comments and blanks

**Type:** Unit
**Input:** `parse_ic("# comment\n\n# another comment\n\n")`
**Expected:** Returns `Ok(Net)` with 0 agents
**Verifies:** R12 -- mixed blank and comment lines

### T4: Parse agent declaration CON

**Type:** Unit
**Input:** `parse_ic("agent x CON")`
**Expected:** Returns `Ok(Net)` with 1 agent, agent has symbol `Symbol::Con`
**Verifies:** R7 -- agent declaration grammar, CON symbol

### T5: Parse agent declaration DUP

**Type:** Unit
**Input:** `parse_ic("agent y DUP")`
**Expected:** Returns `Ok(Net)` with 1 agent, agent has symbol `Symbol::Dup`
**Verifies:** R7 -- DUP symbol recognized

### T6: Parse agent declaration ERA

**Type:** Unit
**Input:** `parse_ic("agent z ERA")`
**Expected:** Returns `Ok(Net)` with 1 agent, agent has symbol `Symbol::Era`
**Verifies:** R7 -- ERA symbol recognized

### T7: Sequential AgentId assignment (R10)

**Type:** Unit
**Input:**
```
agent a CON
agent b DUP
agent c ERA
```
**Expected:** Agent `a` gets ID 0, `b` gets ID 1, `c` gets ID 2
**Verifies:** R10 -- IDs assigned in declaration order starting from 0

### T8: Invalid symbol name produces error

**Type:** Unit
**Input:** `parse_ic("agent x FOO")`
**Expected:** Returns `Err(...)` containing `"unknown symbol 'FOO'"`
**Verifies:** R7 -- only CON, DUP, ERA are valid symbols

### T9: Duplicate agent name produces error with line number

**Type:** Unit
**Input:**
```
agent x CON
agent x DUP
```
**Expected:** Returns `Err(...)` containing `"duplicate"` and a line number reference
**Verifies:** TASK-0164 AC -- duplicate names rejected with line info

### T10: Port name alias principal maps to port 0

**Type:** Unit
**Input:**
```
agent a CON
agent b CON
wire a.principal b.principal
```
**Expected:** Net has agents connected at port 0
**Verifies:** R8 -- `principal` maps to PortId 0

### T11: Port name alias p0 maps to port 0

**Type:** Unit
**Input:**
```
agent a CON
agent b CON
wire a.p0 b.p0
```
**Expected:** Parses successfully, same result as `principal`
**Verifies:** R8 -- `p0` is an alias for `principal`

### T12: Port name alias left/p1 maps to port 1

**Type:** Unit
**Input:**
```
agent a CON
agent b CON
wire a.left b.p1
```
**Expected:** Parses successfully, both sides map to port 1
**Verifies:** R8 -- `left`/`p1` map to PortId 1

### T13: Port name alias right/p2 maps to port 2

**Type:** Unit
**Input:**
```
agent a CON
agent b CON
wire a.right b.p2
```
**Expected:** Parses successfully, both sides map to port 2
**Verifies:** R8 -- `right`/`p2` map to PortId 2

### T14: Invalid port name produces error

**Type:** Unit
**Input:** `parse_ic("agent a CON\nwire a.top free(0)")`
**Expected:** Returns `Err(...)` containing `"unknown port name 'top'"`
**Verifies:** R8 -- only valid port names accepted

### T15: Free port syntax free(N) is recognized

**Type:** Unit
**Input:**
```
agent a CON
wire a.principal free(3)
```
**Expected:** Parses successfully, the connection target is `PortRef::FreePort(3)`
**Verifies:** R7 -- free port syntax in grammar

### T16: Agent declaration with missing symbol produces error

**Type:** Unit
**Input:** `parse_ic("agent a")`
**Expected:** Returns `Err(...)` containing `"requires name and symbol"`
**Verifies:** R7 -- malformed agent declaration is rejected

### T17: Wire with missing port ref produces error

**Type:** Unit
**Input:** `parse_ic("agent a CON\nwire a.principal")`
**Expected:** Returns `Err(...)` containing `"requires two port references"`
**Verifies:** R7 -- malformed wire declaration is rejected

### T18: Unknown keyword produces error

**Type:** Unit
**Input:** `parse_ic("node x CON")`
**Expected:** Returns `Err(...)` containing `"unknown keyword 'node'"`
**Verifies:** R7 -- only agent, wire, root keywords are recognized

---

## Edge Cases

### E1: Whitespace-only lines are treated as blank

**Verify:** `parse_ic("   \t  \n")` returns `Ok(Net)` with 0 agents.
**Why:** Lines with only whitespace should be treated as blank, not produce parse errors.

### E2: Leading/trailing whitespace on declarations is trimmed

**Verify:** `parse_ic("  agent a CON  \n  wire a.principal free(0)  \n")` parses correctly.
**Why:** The parser should `trim()` each line before parsing.

### E3: Agent name can contain underscores and digits

**Verify:** `parse_ic("agent my_agent_42 CON")` succeeds.
**Why:** Per R7, IDENT = `[a-zA-Z_][a-zA-Z0-9_]*`, so underscores and digits (after first char) are valid.
