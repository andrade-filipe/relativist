# TEST-SPEC-0486: `SparseNet` struct + constructors (R13, R18, R29)

**SPEC-22 §7 ID:** T11..T18 (all consume this primitive); plus this plumbing file.
**Owning task:** TASK-0486.
**Parent spec:** SPEC-22 §3.2 R13, R18, R29; §4.4.
**Type:** unit.

---

## Inputs / Fixtures

- `SparseNet::new()` and `SparseNet::with_capacity(N)`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0486-01 | `sparse_new_initializes_empty` | `let sn = SparseNet::new();` | inspect all fields | `agents.is_empty()`, `ports.is_empty()`, `redex_queue.is_empty()`, `next_id == 0`, `root.is_none()`, `freeport_redirects.is_empty()`. |
| UT-0486-02 | `sparse_with_capacity_pre_allocates_buckets` | `let sn = SparseNet::with_capacity(100);` | inspect | `sn.agents.capacity() >= 100`; `sn.ports.capacity() >= 100 * PORTS_PER_SLOT` (or implementation-defined hint propagation). |
| UT-0486-03 | `sparse_derives_debug` | `let sn = SparseNet::new();` | `format!("{:?}", sn)` | does not panic; output is non-empty. |
| UT-0486-04 | `sparse_derives_clone` | `let sn = SparseNet::new();` then add 5 agents | `let sn2 = sn.clone();` | `sn == sn2`. |
| UT-0486-05 | `sparse_derives_partial_eq_eq` | two SparseNet with identical state | `==` | `true`. Two with different state (one extra agent) | `!=`. |
| UT-0486-06 | `sparse_derives_serialize_deserialize` | sparse with 5 agents | bincode round-trip | preserved (smoke; full coverage in T18). |
| UT-0486-07 | `freeport_redirects_field_present_with_serde_skip` | compile-time | grep / inspect | the field is present with `#[serde(skip)]`. (R13 closes Q1 / SC-011.) |
| UT-0486-08 | `no_feature_gate_around_sparse_net` | grep `src/net/sparse.rs` | search for `#[cfg(feature = ...)]` around the struct or impl | none. (R29 always-available.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `with_capacity(0)` | Both HashMaps initialized with default capacity. |
| EC-2 | `with_capacity(usize::MAX)` | Allocation may fail; bincode panics OR `with_capacity` panics — implementation-defined; document. |

## Invariants asserted

- R13 (full field list, including `freeport_redirects`).
- R18 (derive set).
- R29 (always available).

## ARG/DISC/REF citation

- AC-001 (Haskell IC.Core baseline `Map AgentId Agent`).

## Determinism notes

Pure synchronous; no tokio.

## Cross-test dependencies

- T11..T18 all build on this fixture.
- TEST-SPEC-0488 (Send + Sync compile-time assertion) is a separate compile-time check.
