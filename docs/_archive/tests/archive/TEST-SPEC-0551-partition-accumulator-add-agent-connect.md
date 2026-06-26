# TEST-SPEC-0551: PartitionAccumulator add_agent and connect operations (Sparse path)

**SPEC-21 §7 ID:** plumbing (gates §4.9 add_agent / connect; T1 / I1 / I2 delegated coverage).
**Owning task:** TASK-0551.
**Parent spec:** SPEC-21 §4.9 PartitionAccumulator (Sparse mutation path); SPEC-22 R14, R15, R26 (SparseNet operations).
**Type:** unit + memory-usage assertion.
**Theory anchor:** AC-010 (HVM4 frame-reuse).

---

## Inputs / Fixtures

- A fresh `PartitionAccumulator::new(WorkerId(0))`.
- 100 agents to add (contiguous IDs 0..99, mixed symbols).
- A non-contiguous insertion case: 2 agents at IDs 0 and 5_000_000.
- A connect with `FreePort(7)` on one side.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0551-01 | `add_100_contiguous_agents_live_count_is_100` | 100 add_agent_at calls with contiguous IDs 0..99 | call `live_agent_count()` | `== 100`. |
| UT-0551-02 | `min_max_assigned_id_after_contiguous` | post UT-0551-01 | inspect `min_assigned_id`, `max_assigned_id` | `Some(0)`, `Some(99)`. |
| UT-0551-03 | `add_two_non_contiguous_ids_live_count_is_2` | add_agent_at(0, CON), add_agent_at(5_000_000, DUP) | live_agent_count | `== 2`; `min == Some(0)`, `max == Some(5_000_000)`. |
| UT-0551-04 | `non_contiguous_does_not_inflate_internal_storage` | post UT-0551-03 | inspect SparseNet's `agents.len()` (delegated SparseNet R14) | `== 2`. CRITICALLY: assert NO Vec allocation of size 5M+ (memory test against SPEC-22 R30 threshold; the Sparse variant MUST NOT pre-size). |
| UT-0551-05 | `connect_freeport_registers_in_free_port_index` | accumulator with 1 agent at id 0; connect `(0, port=0)` ↔ `FreePort(7)` | inspect `free_port_index` | `free_port_index[7]` contains the other endpoint (the (0,0) AgentPort). |
| UT-0551-06 | `connect_internal_wire_does_not_touch_free_port_index` | 2 agents at ids 0 and 1; connect `(0,0) ↔ (1,1)` | inspect `free_port_index` | unchanged from before the connect (no FreePort involved). |
| UT-0551-07 | `r26_t1_port_linearity_preserved_via_sparse_connect` | post UT-0551-05 | inspect both endpoints' port slots | each port is connected to exactly the wire's other endpoint (T1; delegated to SparseNet `connect`, SPEC-22 R26 / SPEC-02 R13). |
| UT-0551-08 | `i3_prime_uniqueness_create_agent_at` | attempt `add_agent_at(0, CON)` twice | the second call | returns `Err(...)` or panics in debug mode (uniqueness; delegated to SparseNet R14 / R29). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `add_agent_at(u32::MAX, CON)` | succeeds; `max_assigned_id == Some(u32::MAX)`; SparseNet does not allocate `u32::MAX + 1` ports (UT-0551-04 generalizes). |
| EC-2 | Connect with both endpoints being `FreePort` (border-to-border in the same accumulator) | rare but allowed; `free_port_index[a]` and `free_port_index[b]` both contain the OTHER FreePort's id. (Documented behavior; downstream merge handles this.) |
| EC-3 | Connect when the agent referenced is NOT in the accumulator | the SparseNet `connect` returns Err / panics in debug; this TEST-SPEC asserts the error propagation through PartitionAccumulator. |
| EC-4 | A future Dense variant call to add_agent | TASK-0550's discipline (default Sparse) means this code path may be unused; if exercised, test MUST cover both variants. UT-0551-04's memory check applies only to Sparse. |

## Invariants asserted

- I3' uniqueness — preserved via SparseNet `create_agent_at` (per SPEC-22 R14/R15).
- T1 (port linearity) — preserved via SparseNet/Net `connect` (per SPEC-22 R26 / SPEC-02 R13).
- C2 (border wires registered) — partially established via free_port_index update.
- §4.9 add_agent / connect contract.

## ARG/DISC/REF citation

- AC-010 (HVM4 frame-reuse — informs the connection-time vs scan-time classification).

## Determinism notes

UT-0551-04 memory assertion uses a heap profiler hook (or `jemalloc_ctl`) — alternatively, the test asserts `SparseNet.agents.len()` AND a synthesized invariant that the underlying HashMap's bucket count is bounded by `2 * agents.len()` (a structural property that holds for `std::HashMap` after construction). The exact mechanism is implementer's choice; the documented intent is "no Vec allocation of size proportional to max_assigned_id".

UT-0551-08 (uniqueness) MAY panic in debug mode and silently overwrite in release mode (per SparseNet R14 / R29 semantics — verify which); the test MUST exercise the documented behavior in BOTH build configs.

## Cross-test dependencies

- TEST-SPEC-0550 (struct construction) — prerequisite.
- **SPEC-22 fixture reuse (mandatory):** TEST-SPEC-0487 (SparseNet operations), TEST-SPEC-T12 (SparseNet bidirectionality), TEST-SPEC-T17 (SparseNet redex detection). Cite them; DO NOT duplicate SparseNet semantic tests.
- TEST-SPEC-0496 (SparseNet T1/I1/I2 debug assertions) — sibling for assertion-layer coverage.
- TEST-SPEC-0552 (finalize) — depends on the post-add accumulator state being correct.
- TEST-SPEC-0553 (install_connection helper) — calls `connect` on accumulators; UT-0551-05/06 are the pre-conditions.
