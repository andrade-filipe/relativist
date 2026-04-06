# TEST-SPEC-0024: Implement interact_anni (CON-CON, DUP-DUP)

**Task:** TASK-0024
**Spec:** SPEC-03 Sections 4.1.1, 4.1.2, 4.5
**Generated:** 2026-04-06

---

## Unit Tests — CON-CON (cross reconnection)

### T1: CON-CON cross reconnection topology
Create CON(a) <-> CON(b) active pair with a.1<->X, a.2<->Y, b.1<->Z, b.2<->W.
After interact_anni: X<->W (cross: a.1 target <-> b.2 target), Y<->Z (cross: a.2 target <-> b.1 target).

### T2: CON-CON removes both agents
After interact_anni on CON-CON pair, both agent slots are None.

### T3: CON-CON agent count decreases by 2
count_live_agents drops by exactly 2 after interact_anni.

## Unit Tests — DUP-DUP (parallel reconnection)

### T4: DUP-DUP parallel reconnection topology
Create DUP(a) <-> DUP(b) active pair with a.1<->X, a.2<->Y, b.1<->Z, b.2<->W.
After interact_anni: X<->Z (parallel: a.1 target <-> b.1 target), Y<->W (parallel: a.2 target <-> b.2 target).

### T5: DUP-DUP removes both agents
After interact_anni on DUP-DUP pair, both agent slots are None.

## Unit Tests — New Redex Detection

### T6: New redex detected when reconnection links two principal ports
Create CON(a)<->CON(b) where a.1<->CON(c).p0 and b.2<->CON(d).p0. After interact_anni, c.p0<->d.p0 forms a new redex.

## Unit Tests — Self-referencing (R25)

### T7: CON-CON fully self-referencing (a.1<->b.2, a.2<->b.1)
All aux ports of pair point to each other. After interact_anni, both removed, no residual entries. link no-ops per R25.

### T8: DUP-DUP fully self-referencing (a.1<->b.1, a.2<->b.2)
Same as T7 but with DUP-DUP parallel pattern.

### T9: Partial self-reference (one link is no-op, other proceeds normally)
CON-CON where a.1<->b.2 (self-ref) but a.2<->X and b.1<->Y (external). After interact_anni: X<->Y link proceeds, self-ref link is no-op.

## Edge Cases

### E1: Aux ports connected to FreePort (boundary sentinel, R26)
CON-CON where a.1 target is FreePort(0). After interact_anni, reconnection links FreePort to the other target.

### E2: Other agents in the net are unaffected
Create CON-CON pair plus unrelated agents. After interact_anni, unrelated agents still live and connected.
