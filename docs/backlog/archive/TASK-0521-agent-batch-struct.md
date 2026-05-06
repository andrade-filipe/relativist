# TASK-0521: Define `AgentBatch` struct

**Spec:** SPEC-21 §4.1 type definitions; SPEC-21 §3.2 R10/R14/R15.
**Requirements:** AgentBatch struct from §4.1; encapsulates the unit of work in the streaming pipeline.
**Priority:** P0 (foundational type for streaming pipeline; blocker for trait + generator + pipeline tasks).
**Status:** TODO
**Depends on:** TASK-0520 (`ConnectionDirective` enum).
**Blocked by:** none
**Estimated complexity:** S (~25 LoC production + ~50 LoC tests).
**Bundle:** SPEC-21 Streaming Generation — Phase B (core types).

## Context

Each batch contains agent definitions and connection directives. The batch is the unit of work in the streaming pipeline: the generator produces one batch, the partitioner assigns its agents, and the pipeline installs agents and connections incrementally.

Per SPEC-21 §4.1 + R14 + R15:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentBatch {
    /// Agent definitions: (id, symbol) pairs.
    /// IDs MUST be globally unique and monotonically increasing
    /// across batches (SPEC-21 R15 — strictly stronger than SPEC-01 I3').
    pub agents: Vec<(AgentId, Symbol)>,

    /// Connection directives for this batch.
    pub connections: Vec<ConnectionDirective>,
}
```

R15 is a **generator-phase** contract: max `AgentId` in batch `k` < min `AgentId` in batch `k+1`. The contract scope is the generation pipeline only; once dispatched, the worker arena MAY recycle slot IDs per SPEC-22 I3' / R1-R10c (see §3.5 closing note + TASK-0571 for the post-dispatch monotonicity-forbidden discipline).

`Symbol` is the SPEC-02 `enum {ERA, CON, DUP}`.

## Acceptance Criteria

- [ ] Define `pub struct AgentBatch` in the same module as `ConnectionDirective` (per TASK-0520).
- [ ] Two fields present: `agents: Vec<(AgentId, Symbol)>` and `connections: Vec<ConnectionDirective>`.
- [ ] Derive `Debug`, `Clone`, `serde::Serialize`, `serde::Deserialize` (per SPEC-21 §4.1; note: `PartialEq`/`Eq` NOT required — `Symbol` Clone semantics may not require deep eq).
- [ ] Document via Rustdoc the IC-concept paragraph from §4.1: AgentBatch represents a fragment of an interaction net which may have dangling ports temporarily.
- [ ] R15 monotonicity contract documented inline as a generator OBLIGATION (not enforced by the type — enforcement is debug-assertion-only in TASK-0544).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/partition/streaming.rs` (or chosen location per TASK-0520) | modify | Add `AgentBatch` struct. |

## Key Types / Signatures

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentBatch {
    pub agents: Vec<(AgentId, Symbol)>,
    pub connections: Vec<ConnectionDirective>,
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0521:
- T2 (AgentBatch construction): create batches with known agents and connections; verify agent IDs are monotonically increasing across batches; verify connection directives are correctly classified (resolved vs pending).
- Serde round-trip equality for AgentBatch (preserves contents).

## Invariants Touched

- R15 (generator-phase monotonicity) — type level: NONE (the type does not enforce; enforcement is in TASK-0544).
- I3' (Uniqueness of AgentIds) — preserved trivially under R15.

## Notes

- The struct does NOT enforce R15 — it is a generator obligation per SPEC-21 §3.2 R15. Enforcement (debug-assert in `make_net_stream` callers) lives in TASK-0544.
- `PartialEq`/`Eq` left off intentionally; equality is rarely meaningful for streaming batches and the derive may collide with `Symbol`'s Eq semantics in some configurations.
- The struct is pure-Core-layer per SPEC-21 R9 / R16.
- Consumed by TASK-0524 (StreamingPartitionStrategy trait), TASK-0540+ (generators), TASK-0554 (pipeline orchestrator).

## DAG Links

- **Predecessors:** TASK-0520.
- **Successors:** TASK-0524, TASK-0540, TASK-0541, TASK-0542, TASK-0554.
