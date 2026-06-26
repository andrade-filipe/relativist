# TEST-SPEC-0051: Redex queue population for partitions

**Task:** TASK-0051
**Spec:** SPEC-04
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Internal redex included
Original net has redex `(0, 1)`. Agent set for partition: `{0, 1}`. Call `populate_redex_queue`. Verify subnet's redex_queue contains `(0, 1)`.

### T2: Border redex excluded
Original net has redex `(0, 1)`. Agent set for partition: `{0}` (agent `1` is in another partition). Call `populate_redex_queue`. Verify subnet's redex_queue is empty.

### T3: Stale redex excluded
Original net has redex `(0, 3)` but agent `3` has been removed (slot is `None`). Agent set: `{0}`. Call `populate_redex_queue`. Verify subnet's redex_queue does not contain `(0, 3)`.

### T4: Mixed internal and border redexes -- only internal kept
Original net has redexes `[(0, 1), (2, 3), (0, 3)]`. Agent set: `{0, 1}`. Expected: only `(0, 1)` in subnet's queue. `(2, 3)` has neither agent in set; `(0, 3)` is a border redex.

### T5: Multiple internal redexes preserved in order
Original net has redexes `[(0, 1), (2, 3)]`. Agent set: `{0, 1, 2, 3}`. Verify subnet's queue contains both `(0, 1)` and `(2, 3)` in that order.

## Edge Cases

### E1: Empty original redex queue
Original net's redex_queue is empty. Agent set: `{0, 1}`. Verify subnet's redex_queue is empty.

### E2: All redexes are border redexes
Original net has redexes `[(0, 1), (0, 2)]`. Agent set: `{0}` (agents 1 and 2 in other partitions). Verify subnet's redex_queue is empty.
