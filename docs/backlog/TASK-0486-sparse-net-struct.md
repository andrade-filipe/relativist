# TASK-0486: Define `SparseNet` struct + constructors (R13, R18, R29)

**Spec:** SPEC-22 §3.2 R13, R18, R29; §4.4 (struct definition); §4.5 (`new`/`with_capacity` constructors).
**Requirements:** R13 (SparseNet field list — agents, ports, redex_queue, next_id, root, freeport_redirects), R18 (Debug, Clone, PartialEq, Eq, Serialize, Deserialize derives), R29 (always available, no feature gate).
**Priority:** P0 (foundational; every SparseNet operation depends on the type).
**Status:** TODO
**Depends on:** none (pure type definition).
**Blocked by:** none
**Estimated complexity:** S (~50 LoC production + ~30 LoC tests)
**Bundle:** SPEC-22 Arena Management — Phase D (SparseNet).

## Context

SPEC-22 §4.4 defines the complete `SparseNet` struct in `src/net/sparse.rs`. Fields (R13):
- `agents: HashMap<AgentId, Agent>`
- `ports: HashMap<(AgentId, PortId), PortRef>` (only live ports — no ERA aux, no DISCONNECTED)
- `redex_queue: VecDeque<(AgentId, AgentId)>`
- `next_id: AgentId`
- `root: Option<PortRef>`
- `freeport_redirects: HashMap<u32, PortRef>` (closes Q1 / SC-011)

R18 mandates the derive set; R29 mandates always-available (no feature gate). Memory is strictly proportional to live agents (no tombstones).

## Acceptance Criteria

- [ ] Create `relativist-core/src/net/sparse.rs` with the `SparseNet` struct per SPEC-22 §4.4.
- [ ] All 6 fields per R13.
- [ ] Derives: `Debug, Clone, PartialEq, Eq, Serialize, Deserialize` (R18).
- [ ] `freeport_redirects` field has `#[serde(skip)]` (mirrors Net's pattern; not on the wire).
- [ ] Implement `SparseNet::new()` and `SparseNet::with_capacity(capacity: usize)` per §4.5 — both initialize all fields to empty/default; `next_id = 0`; `root = None`.
- [ ] `with_capacity` allocates `HashMap::with_capacity(capacity)` for `agents` and `HashMap::with_capacity(capacity * PORTS_PER_SLOT)` for `ports`.
- [ ] No feature gate (R29).
- [ ] Add `pub mod sparse;` to `relativist-core/src/net/mod.rs` and export `pub use sparse::SparseNet`.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/sparse.rs` | create | New file: `SparseNet` struct + `new()` + `with_capacity()`. |
| `relativist-core/src/net/mod.rs` | modify | `pub mod sparse;` and re-export. |

## Key Types / Signatures

```rust
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SparseNet {
    pub agents: HashMap<AgentId, Agent>,
    pub ports: HashMap<(AgentId, PortId), PortRef>,
    pub redex_queue: VecDeque<(AgentId, AgentId)>,
    pub next_id: AgentId,
    pub root: Option<PortRef>,
    #[serde(skip)]
    pub freeport_redirects: HashMap<u32, PortRef>,
}

impl SparseNet {
    pub fn new() -> Self;
    pub fn with_capacity(capacity: usize) -> Self;
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0486:
- `sparse_new_initializes_empty`.
- `sparse_with_capacity_pre_allocates_buckets`.
- `sparse_derives_debug_clone_eq` (smoke).

## Invariants Touched

- R29 (always available).
- D1c (FreePort bijectivity preserved by `freeport_redirects` field).

## Notes

- This task does NOT include rkyv derives (SparseNet is NOT on the zero-copy hot path; it's a construction-time type). If future work adds rkyv to SparseNet, add the conditional derive then.
- `Send + Sync` compile-time assertion is in TASK-0488.

## DAG Links

- **Predecessors:** none.
- **Successors:** TASK-0487 (SparseNet ops), TASK-0488 (Send+Sync), TASK-0489 (to_sparse), TASK-0490 (to_dense).
