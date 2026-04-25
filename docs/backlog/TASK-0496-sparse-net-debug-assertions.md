# TASK-0496: T1 / I1 / I2 SparseNet debug assertions (R26)

**Spec:** SPEC-22 §3.3 R26.
**Requirements:** R26 (T1 / I1: for every port entry `(a_id, p) -> q` in sparse `ports` HashMap, reverse entry `q -> AgentPort(a_id, p)` MUST exist unless `q` is a `FreePort`. Root port exception per SPEC-01 T1 applies. I2: for every `AgentPort(id, p)` value in sparse ports, `self.agents.contains_key(&id)` MUST be true and `p <= arity(agents[id].symbol)`).
**Priority:** P1 (defensive; SparseNet correctness during construction).
**Status:** TODO
**Depends on:** TASK-0486 (SparseNet), TASK-0487 (SparseNet ops).
**Blocked by:** none
**Estimated complexity:** S (~60 LoC production + ~50 LoC tests)
**Bundle:** SPEC-22 Arena Management — Phase E (invariant amendments).

## Context

R26 mandates that SparseNet preserve T1 (Port Linearity), I1 (Bidirectional Consistency), and I2 (Reference Validity) with adapted verification semantics. The sparse representation differs from dense: ports are stored in a HashMap keyed by `(AgentId, PortId)`, so the verification is structurally adapted but semantically equivalent.

## Acceptance Criteria

- [ ] Implement `SparseNet::assert_invariants(&self)` in `relativist-core/src/net/sparse.rs` covering R26's three families:
  - **T1/I1 bidirectional:** for each `((a_id, p), q)` in `self.ports`, if `q` is `AgentPort(b_id, b_p)`, assert `self.ports.get(&(b_id, b_p)) == Some(&AgentPort(a_id, p))`. Root-port exception: skip if `q == self.root.unwrap_or(DISCONNECTED)`.
  - **I2 reference validity (agent existence):** for every `AgentPort(id, p)` value in `self.ports`, assert `self.agents.contains_key(&id)`.
  - **I2 reference validity (port arity):** for every `AgentPort(id, p)` value, assert `p < total_ports(self.agents[&id].symbol)`.
- [ ] Wrap the entire helper in `#[cfg(debug_assertions)]` for zero release-build cost.
- [ ] Cite SPEC-22 R26 + SPEC-01 T1, I1, I2 in Rustdoc.
- [ ] Test: build a SparseNet, intentionally violate I1 by removing one direction of a bidirectional port pair, assert `assert_invariants` panics.
- [ ] Test: build a valid SparseNet (3 chained agents), assert `assert_invariants` does not panic.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/sparse.rs` | modify | Add `impl SparseNet { pub fn assert_invariants(&self) }`. |

## Key Types / Signatures

```rust
#[cfg(debug_assertions)]
impl SparseNet {
    /// SPEC-22 R26: verify T1 (Port Linearity), I1 (Bidirectional Consistency),
    /// I2 (Reference Validity) on the sparse representation.
    pub fn assert_invariants(&self);
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0496:
- `sparse_assert_invariants_passes_on_valid_net`.
- `sparse_assert_invariants_catches_one_way_port_violation`.
- `sparse_assert_invariants_catches_dangling_agent_reference`.
- `sparse_assert_invariants_catches_oob_port_arity`.

## Invariants Touched

- T1 / I1 / I2 (SparseNet equivalents — R26).

## Notes

- The root-port exception is per SPEC-01 T1 (root may legitimately be a one-sided endpoint).
- ERA cleanliness (R17) is a separate concern; that's the I6 sparse equivalent and is verified at construction time in TASK-0487, not here.

## DAG Links

- **Predecessors:** TASK-0486, TASK-0487.
- **Successors:** TASK-0500 (regression).
