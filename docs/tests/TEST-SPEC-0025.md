# TEST-SPEC-0025: Implement interact_eras (CON-ERA, DUP-ERA)

**Task:** TASK-0025
**Spec:** SPEC-03 Sections 4.1.5, 4.1.6, 4.5
**Generated:** 2026-04-06

---

## Unit Tests

### T1: CON-ERA removes both agents and creates 2 new ERA
Create CON(a)<->ERA(b) with a.1<->X and a.2<->Y. After interact_eras: a and b removed, 2 new ERA(e1) and ERA(e2) created, e1.p0<->X and e2.p0<->Y.

### T2: DUP-ERA removes both and creates 2 new ERA (identical topology)
Same as T1 but with DUP instead of CON.

### T3: Agent balance is 0 (removes 2, creates 2)
count_live_agents unchanged (excluding the pair: context agents + 2 new ERA = context agents + 2 original).

### T4: New ERA agents have Symbol::Era
Verify the created agents have the correct symbol.

### T5: Erasure cascade -- new redex detected when a1_target is a principal port
CON(a)<->ERA(b) where a.1<->CON(c).p0. After interact_eras, new ERA.p0<->c.p0 forms a new redex.

### T6: FreePort aux target (boundary sentinel, R26)
CON(a)<->ERA(b) where a.1 target is FreePort(0). After interact_eras, new ERA.p0 is connected to FreePort(0).

## Edge Cases

### E1: Other agents in the net are unaffected
Create CON-ERA pair plus unrelated agents. After interact_eras, unrelated agents still live.

### E2: ERA's unused auxiliary slots remain clean after removal
After removing ERA(b), its slots 1 and 2 are DISCONNECTED (I6 compliance).
