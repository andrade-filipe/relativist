# TEST-SPEC-0026: Implement interact_comm (CON-DUP)

**Task:** TASK-0026
**Spec:** SPEC-03 Section 4.1.4, 4.5
**Generated:** 2026-04-06

---

## Unit Tests

### T1: CON-DUP creates 4 new agents (2 DUP + 2 CON)
Create CON(a)<->DUP(b) active pair. After interact_comm: a and b removed, 4 new agents: p=DUP, q=DUP, r=CON, s=CON.

### T2: Agent balance is +2 (removes 2, creates 4)
count_live_agents increases by 2 relative to the pair (net gains 2 agents after removing 2 and creating 4).

### T3: External wires correct -- p.p0<->a1_target, q.p0<->a2_target, r.p0<->b1_target, s.p0<->b2_target
Verify the 4 external wire connections from new agents' principal ports to old neighbors.

### T4: Internal wires correct -- crossed pattern
Verify: p.1<->r.1, p.2<->s.1, q.1<->r.2, q.2<->s.2.

### T5: New agents have correct symbols
p and q are DUP, r and s are CON.

### T6: New redex detection -- up to 4 external wires can form redexes
If a1_target is a principal port of another agent, linking p.p0<->a1_target forms a new redex.

### T7: Internal wires do NOT generate redexes (all aux-to-aux)
The 4 internal wire connections are between auxiliary ports, so no redex detection.

## Edge Cases

### E1: FreePort aux targets (boundary sentinel, R26)
CON(a)<->DUP(b) where a.1 target is FreePort(0). After interact_comm, p.p0 should be connected to FreePort(0).

### E2: Other agents in the net are unaffected
Verify unrelated agents remain intact after interaction.

### E3: PortRef values survive Vec reallocation during create_agent
Pre-read PortRef values (a1, a2, b1, b2) are index-based, not pointer-based. Vec reallocation during 4x create_agent does NOT invalidate them.
