# TEST-SPEC-T7: Invariants T1, I2, I3' on a non-trivial reduced net

**SPEC-22 §7.1 ID:** T7.
**Owning task:** TASK-0495 (I3' debug assertions; covers R27 family 1-4).
**Parent spec:** SPEC-22 §3.3 R24 (I3'), R27; SPEC-01 T1 / I1 / I2.
**Type:** integration.
**Theory anchor:** REF-002 (Lafont 1997 — Church numerals as IC encoding); AC-001 (Haskell IC.Core baseline).

---

## Inputs / Fixtures

- A non-trivial net: Church(3) + Church(2) addition encoded per SPEC-14 (Church numerals + add).
  - `Church(3)` is the IC encoding of the numeral 3 (a tree of CON/ERA/DUP per the spec).
  - `Church(2)` is the IC encoding of the numeral 2.
  - The `add` combinator wires them together to compute Church(5).
- Pre-reduction state: ~tens of agents, multiple redexes in the queue.
- Run `net.reduce_all()` to normal form.
- Post-reduction state: a smaller net representing Church(5), plus accumulated free-list entries from the consumed agents.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T7-01 | `t1_bidirectionality_post_reduction` | post-reduction net | iterate over every `port` slot in `net.ports`; for each `AgentPort(a, p)`, verify `net.ports[port_index(target_a, target_p)] == AgentPort(a, p)` (root-port exception per SPEC-01 T1) | passes for every live port. |
| UT-T7-02 | `i2_reference_validity_post_reduction` | post-reduction net | for every `PortRef::AgentPort(id, p)` value in `ports`, verify `net.agents[id as usize].is_some()` AND `p < total_ports(net.agents[id].unwrap().symbol)` | passes for every entry. |
| UT-T7-03 | `i3_prime_uniqueness_post_reduction` | post-reduction net | for every live agent `agents[i] = Some(a)`, verify `a.id == i as AgentId`; for every `id in &net.free_list`, verify `net.agents[id as usize].is_none()`; assert no overlap between live-id set and free-list set | passes. |
| UT-T7-04 | `r27_family_4_no_free_list_port_refs` | post-reduction net | call `net.assert_no_free_list_port_refs()` (debug-only helper from TASK-0495) | does not panic. R27 family (4): no port slot references a free-list ID. |
| UT-T7-05 | `church_5_topology_recovered` | post-reduction net | structural check that the result encodes Church(5) per SPEC-14 (e.g., 5 nested CON layers terminating at a known marker) | matches the expected Church(5) shape. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Church(0) + Church(0) (degenerate base case) | Reduces to Church(0); no agents created via commutation; free-list reflects only annihilations. T1/I2/I3' all hold. |
| EC-2 | Church(10) + Church(10) (larger workload, exercises more recycling) | Reduces to Church(20); same invariant checks pass. |
| EC-3 | Reduction halted mid-way (call `reduce_step()` 5 times instead of `reduce_all()`) | Even at intermediate state, T1/I2/I3' MUST hold. Free-list contains some IDs; live agents reference no free-list IDs. |

## Invariants asserted

- T1 (Port Linearity).
- I1 (Bidirectional Consistency — implied by T1 verification).
- I2 (Reference Validity).
- I3' (Uniqueness of AgentIds — RELAXED from I3 monotonicity per §3.8 A1).
- R27 family (1)-(4) — verified at the end of `reduce_all` by `Net::debug_check_invariants` (helper composed of the four families).

## ARG/DISC/REF citation

- REF-002 (Lafont 1997 §4 — Church-encoded data and the universality of γ/δ/ε for arithmetic).
- AC-001 (Haskell IC.Core baseline — Church arithmetic reference for invariant validation).

## Determinism notes

Reduction strategy may visit redexes in any order, but the normal form (Church(5)) is unique by confluence (REF-002 Proposition 1). The post-state assertions are confluence-preserved (T1/I2/I3' are invariants of the reduction, not of the strategy). Pure single-threaded `reduce_all`; no tokio.

## Cross-test dependencies

- Uses the `Church(n)` and `add` constructors from SPEC-14 (existing v1 code).
- Composes with T7a (CON-DUP under partial free-list) — T7 is the broad sweep, T7a is the targeted assertion.
- TEST-SPEC-0495 covers the same R27 families with synthetic minimal fixtures.
