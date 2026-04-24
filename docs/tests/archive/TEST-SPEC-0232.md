# TEST-SPEC-0232: Fix self-loop annihilation in interact_anni

**Task:** TASK-0232
**Spec:** SPEC-03 R25 (extended)
**Generated:** 2026-04-07

---

## Unit Tests — Self-loop on one agent

### T1: CON-CON with b self-loop, a has external ports
Create CON(a) with a.p1<->X, a.p2<->Y. Create CON(b) with b.p1<->b.p2 (self-loop).
Connect a.p0<->b.p0. Call `interact_anni(net, a, b)`.
**Expected:** X<->Y (a's externals connected), both agents removed.

### T2: CON-CON with a self-loop, b has external ports
Create CON(a) with a.p1<->a.p2 (self-loop). Create CON(b) with b.p1<->X, b.p2<->Y.
Connect a.p0<->b.p0. Call `interact_anni(net, a, b)`.
**Expected:** X<->Y (b's externals connected), both agents removed.

### T3: DUP-DUP with b self-loop
Create DUP(a) with a.p1<->X, a.p2<->Y. Create DUP(b) with b.p1<->b.p2 (self-loop).
Connect a.p0<->b.p0. Call `interact_anni(net, a, b)`.
**Expected:** X<->Y, both agents removed. (Same as CON-CON; self-loop acts as identity regardless of CROSS/PARALLEL.)

### T4: DUP-DUP with a self-loop
Mirror of T3 with self-loop on a instead of b.
**Expected:** b's external ports linked together.

## Unit Tests — Self-loop on both agents

### T5: CON-CON both self-loops
Create CON(a) with a.p1<->a.p2, CON(b) with b.p1<->b.p2.
Connect a.p0<->b.p0. Call `interact_anni`.
**Expected:** Both removed. No links made. No residual DISCONNECTED ports (no external ports exist).

### T6: DUP-DUP both self-loops
Same as T5 but with DUP agents.
**Expected:** Both removed. No links made.

## Unit Tests — Normal cases still work (regression)

### T7: CON-CON normal CROSS (no self-loops)
a.p1<->W, a.p2<->X, b.p1<->Y, b.p2<->Z. After interact_anni: W<->Z, X<->Y.
(Same as TEST-SPEC-0024 T1 — verify no regression.)

### T8: DUP-DUP normal PARALLEL (no self-loops)
a.p1<->W, a.p2<->X, b.p1<->Y, b.p2<->Z. After interact_anni: W<->Y, X<->Z.
(Same as TEST-SPEC-0024 T4 — verify no regression.)

### T9: Inter-agent R25 still works
CON-CON where a.p1<->b.p2 and a.p2<->b.p1 (inter-agent cross-ref, not self-loop).
After interact_anni: both removed, no residual.
(Same as TEST-SPEC-0024 T7 — verify no regression.)

## Unit Tests — Integration with Church encoding

### T10: Church(0) annihilation preserves invariants
Build Church(0) via `encode_nat(0)`. Create application CON agent connected to lam_x.
After reduce_step: `net.assert_all_invariants()` does not panic.

### T11: Redex detection after self-loop annihilation
Self-loop agent annihilated, partner's external ports are both principal ports of other agents.
After interact_anni: the new link (external1 <-> external2) forms a new redex.
Verify the redex queue contains the new pair.

## Edge Cases

### E1: Self-loop agent with one external FreePort
CON(a) with a.p1<->FreePort(0), a.p2<->Y. CON(b) with b.p1<->b.p2.
After interact_anni: link(FreePort(0), Y) — FreePort handling per R26.

### E2: Self-loop agent with both externals being FreePort
CON(a) with a.p1<->FreePort(0), a.p2<->FreePort(1). CON(b) with b.p1<->b.p2.
After interact_anni: link(FreePort(0), FreePort(1)) — both FreePort, connect handles this.

### E3: Partial self-loop (only p1 → p2, but p2 → external)
This cannot happen because `connect` is bidirectional: if p1→p2 then p2→p1. So partial self-loops are impossible. This edge case is a non-issue by construction.
