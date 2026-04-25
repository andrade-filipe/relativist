# TASK-0487: SparseNet operations — create/remove/connect/disconnect/get_target/get_agent/is_reduced/count_live (R14, R15, R16, R17)

**Spec:** SPEC-22 §3.2 R14, R15, R16, R17; §4.5 (operations bodies).
**Requirements:** R14 (full operation list mirroring Net), R15 (O(1) amortized complexity), R16 (no tombstones — `remove_agent` removes from HashMap and purges port entries), R17 (no port entries for ERA auxiliary slots — sparse equivalent of I6).
**Priority:** P0 (foundational; required for to_sparse/to_dense and build_subnet integration).
**Status:** TODO
**Depends on:** TASK-0486 (struct exists).
**Blocked by:** none
**Estimated complexity:** M (~150 LoC production + ~120 LoC tests)
**Bundle:** SPEC-22 Arena Management — Phase D (SparseNet).

## Context

SPEC-22 §4.5 provides the complete bodies for SparseNet operations:
- `create_agent(symbol)`: insert into HashMap, increment `next_id`. No port entries created (created by `connect`).
- `remove_agent(id)`: remove from HashMap, disconnect all ports.
- `connect(a, b)`: insert bidirectional entries (only for AgentPort endpoints — FreePort is implicit in absence). Detect new redex if both are principal (port 0).
- `disconnect(port)`: remove bidirectional entries.
- `get_target(port)`: HashMap lookup; FreePort returns DISCONNECTED.
- `get_agent` / `get_agent_mut`: HashMap lookup.
- `is_reduced`: redex queue empty.
- `count_live_agents`: `self.agents.len()` (R14, R15 — O(1)).
- `live_agents`: iterate over HashMap values.

R16 mandates no tombstones; R17 mandates no ERA auxiliary port entries (sparse equivalent of SPEC-01 I6).

## Acceptance Criteria

- [ ] Implement all 9 operations on `SparseNet` per SPEC-22 §4.5 reference bodies.
- [ ] R14 signature parity with Net.
- [ ] R15 complexity contracts: all O(1) amortized.
- [ ] R16: `remove_agent` removes the agent's HashMap entry AND all `(id, p)` port entries for `p in 0..total_ports(symbol)`. No tombstones.
- [ ] R17: `connect` does NOT insert port entries for ERA auxiliary ports (port 1, 2 of an ERA agent — ERA has arity 0 so principal port is the only valid port).
- [ ] `connect` detects new redex when both endpoints are AgentPort with port 0 (principal-principal).
- [ ] `disconnect` is bidirectional: when `(a_id, a_port) -> b` is removed, the reverse entry from `b` (if `b` is AgentPort) is also removed.
- [ ] `get_target` for FreePort returns DISCONNECTED (sparse representation; redirects handled separately in `freeport_redirects`).
- [ ] `live_agents()` returns `impl Iterator<Item = &Agent>`.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/sparse.rs` | modify | Add `impl SparseNet { ... }` block with the 9 operations per §4.5. |

## Key Types / Signatures

(Per SPEC-22 §4.5 — copy verbatim.)

## Test Expectations (forward-ref)

TEST-SPEC-0487 — covers SPEC-22 §7.2:
- T11: Construction and agent count (5 agents → 5 live; remove 2 → 3 live; no tombstones).
- T12: Bidirectionality (I1 for SparseNet — for every `(id, p) -> target`, reverse exists unless target is FreePort).
- T13: ERA cleanliness (I6 for SparseNet — `ports.get(&(era_id, 1))` returns `None`; same for port 2).
- T17: Redex detection (principal-principal connect adds to redex queue; aux-aux does not).

## Invariants Touched

- R26 (T1, I1, I2 hold for SparseNet with adapted verification — implemented here, asserted in TASK-0496).
- Sparse equivalent of I6 (ERA cleanliness) — R17.

## Notes

- `total_ports(symbol)` already exists from SPEC-02 / TASK-0006.
- The `connect` redex-detection logic mirrors Net's existing logic — same predicates, different storage backend.

## DAG Links

- **Predecessors:** TASK-0486.
- **Successors:** TASK-0489 (to_sparse), TASK-0490 (to_dense), TASK-0496 (T1/I1/I2 SparseNet debug assertions).
