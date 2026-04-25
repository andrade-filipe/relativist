# TEST-SPEC EG-U17: strict BSP self-partition uniformity (R3c)

**SPEC-20 §7.1 ID:** EG-U17
**Owning task(s):** TASK-0424.
**Type:** unit.
**Test name:** `test_strict_bsp_self_partition_uniformity`.

---

## Inputs / Fixtures

- Hybrid; K_remote=1; `strict_bsp = true`.
- A small net containing a redex whose origin can be classified as "border-origin" within the self-partition (e.g. a CON-DUP cascade that crosses the self/remote boundary).

## Expected behaviour

R3c: in strict_bsp mode, border-origin redexes inside the self-partition are deferred to the NEXT round identically to remote workers. The self-partition does NOT have privileged access to "reduce now" semantics that remote workers lack.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | In round N, the self-partition reduces all NON-border redexes; emits its border-origin redex into the round-N+1 cycle (does NOT reduce it in-round). |
| A2 | A remote worker with an analogous border-origin redex defers it identically. |
| A3 | The self-partition's `border_deltas` for round N has the same _count_ of deferred border-origin redexes as a hypothetical remote running the same partition would. |
| A4 | Final result: `canonicalise(final) == canonicalise(reduce_all(input))`. |

## Edge / negative cases

- EC-1: `strict_bsp = false` (lenient) — the self-partition MAY reduce border-origin redexes immediately (faster); not this test's territory; cross-link.
- EC-2: net with no border-origin redexes — uniformity assertion is trivially true (both self and remote defer 0 redexes).

## Invariants asserted

- G1 PRESERVED at every K_eff value.

## ARG/DISC/REF citation

None.

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. Pre-computed partitions; reduction is synchronous within the spawn_blocking task.

## Cross-test dependencies

EG-U4-delta-wire-symmetry (the wire-shape symmetry; this test asserts the *behavioural* symmetry).
