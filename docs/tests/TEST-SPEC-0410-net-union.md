# TEST-SPEC-0410: `Net::union` structural-concatenation primitive (SPEC-02 A7)

**SPEC-20 §7 ID:** none (plumbing — `Net::union` is exercised end-to-end by EG-I3, EG-I3-delta, EG-I5a per TASK-0410).
**Owning task:** TASK-0410.
**Parent spec:** SPEC-02 (amended via SPEC-20 §3.8 A7); SPEC-20 §4.2.2 v1-mode/delta-mode departure recovery step 4.
**Type:** unit.
**Theory anchor:** none direct (the union itself is purely structural; correctness of the surrounding departure cycle is gated by ARG-006).

---

## Inputs / Fixtures

- `Net::empty()` constructor (must already exist on `Net`).
- Two small hand-built `Net` values with **disjoint** `AgentId` ranges:
  - `net_a`: 2 agents with ids `[0, 1]`, one internal wire, one `FreePort(border_id=10)`.
  - `net_b`: 3 agents with ids `[100, 101, 102]`, two internal wires, two `FreePort(border_id=20, 21)`.
- A "collision" pair where both nets contain `AgentId=5` (used only in the debug-only panic test).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0410-01 | `union_empty_right` | `let a = net_a` | `let n = a.clone().union(Net::empty())` | `n` structurally equals `a` (same agent count, same wires, same FreePort entries — order may differ; canonicalize before equality). |
| UT-0410-02 | `union_empty_left` | `let a = net_a` | `let n = Net::empty().union(a.clone())` | Identical to UT-0410-01. |
| UT-0410-03 | `union_disjoint_ids_preserves_agents` | `net_a`, `net_b` | `let n = net_a.clone().union(net_b.clone())` | `n.agents().count() == 5`; the multiset of `AgentId` values equals `{0,1,100,101,102}`; symbol multiset equals union of input symbol multisets. |
| UT-0410-04 | `union_preserves_freeports_from_both_sides` | same | same | The result's `FreePort` list equals the multiset union of both input lists (`{10, 20, 21}`); no FreePort is silently resolved or removed. |
| UT-0410-05 | `union_preserves_internal_wires` | same | same | Every internal `(AgentId, port) ↔ (AgentId, port)` wire from `net_a` and from `net_b` is present in `n` exactly once. No new cross-net wires materialise (that is `merge`'s responsibility, NOT `union`'s). |
| UT-0410-06 | `union_root_port_follows_self` | `net_a` has `root = AgentPort(0, 0)`, `net_b` has `root = AgentPort(100, 0)` | `let n = net_a.clone().union(net_b.clone())` | `n.root == net_a.root` (`AgentPort(0, 0)`). Documented as `union_root_follows_self` in Rustdoc. |
| UT-0410-07 | `union_panics_on_overlapping_ids_debug_build` | `net_a` and a "collision" net both containing `AgentId=5`; this test runs only under `#[cfg(debug_assertions)]` | `panic::catch_unwind(\|\| net_a.clone().union(collision_net))` | Panic fires; panic payload string contains `SPEC-20 A7` and the offending id. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Both inputs empty | `Net::empty().union(Net::empty()) == Net::empty()`; agent count 0; FreePort count 0. |
| EC-2 | One side has zero internal wires | Result preserves all wires from the other side; no spurious wires. |
| EC-3 | Both sides share a `border_id` value (e.g. both have `FreePort(10)`) | Wire-level OK — `union` is structural; the duplicate is preserved as two distinct entries. (Resolution is the subsequent `split()`'s responsibility.) |

## Invariants asserted

- I1, I2 (per-agent slot validity) — preserved by construction.
- D4 (ID Uniqueness) — caller's precondition; UT-0410-07 guards the debug fence.

## ARG/DISC/REF citation

None directly. `Net::union` is structural plumbing under ARG-002 P2 (split/reduce/remap/merge correctness) — its enclosing departure cycle (TASK-0440, TASK-0443) is what cites ARG-006.

## Determinism notes

Pure synchronous code. No tokio, no async, no scheduling. Tests are fully deterministic.

## Cross-test dependencies

None. UT-0410-07 may be `#[cfg(debug_assertions)]`-gated; the test count for release builds therefore differs by 1.
