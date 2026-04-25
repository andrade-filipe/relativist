# TASK-0520: Define `ConnectionDirective` enum (Resolved / Pending)

**Spec:** SPEC-21 §4.1 type definitions; SPEC-21 §3.2 R14 (forward references via `PendingConnection`).
**Requirements:** `ConnectionDirective` enum from §4.1 — supports forward references per R14.
**Priority:** P0 (foundational type for AgentBatch; blocker for all generator and pipeline tasks).
**Status:** TODO
**Depends on:** none.
**Blocked by:** none
**Estimated complexity:** S (~30 LoC production + ~40 LoC tests).
**Bundle:** SPEC-21 Streaming Generation — Phase B (core types).

## Context

The streaming pipeline emits batches of agents with connection directives. Some directives reference agents in the current/previous batch (resolved); others reference agents that will appear in a future batch (pending / forward references).

Per SPEC-21 §4.1:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ConnectionDirective {
    Resolved {
        source: (AgentId, PortId),
        target: (AgentId, PortId),
    },
    Pending {
        source: (AgentId, PortId),
        target_agent_id: AgentId,
        target_port: PortId,
    },
}
```

`AgentId` and `PortId` are defined in SPEC-02 (per §4.1 type-origins paragraph): `AgentId` is `u32` newtype, `PortId` is `0..=2` per arity (ERA: 0 aux; CON/DUP: 2 aux).

## Acceptance Criteria

- [ ] Define `pub enum ConnectionDirective` in `relativist-core/src/partition/streaming.rs` (or `relativist-core/src/io/streaming.rs` per the SPEC-21 R10 module suggestion — DEVELOPER chooses the canonical home; both locations are documented in the spec; coordinate with TASK-0524 trait location).
- [ ] Both variants present: `Resolved { source, target }` and `Pending { source, target_agent_id, target_port }`.
- [ ] Derive `Debug`, `Clone`, `PartialEq`, `Eq`, `serde::Serialize`, `serde::Deserialize` (per SPEC-21 §4.1 type definitions).
- [ ] Document via Rustdoc the IC-concept comment from §4.1 (forward-ref scenario, `dual_tree` example).
- [ ] No `unwrap()` / no `unsafe` — pure data type.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/partition/streaming.rs` *(or `src/io/streaming.rs` — DEVELOPER choice)* | create OR modify | Add `ConnectionDirective` enum. |
| `relativist-core/src/partition/mod.rs` *(if streaming.rs is new in partition/)* | modify | Re-export `ConnectionDirective`. |

## Key Types / Signatures

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ConnectionDirective {
    Resolved {
        source: (AgentId, PortId),
        target: (AgentId, PortId),
    },
    Pending {
        source: (AgentId, PortId),
        target_agent_id: AgentId,
        target_port: PortId,
    },
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0520:
- Construct `Resolved` and `Pending` variants; assert serde round-trip equality (covers SPEC-21 §4.1 derive set).
- Assert `Pending::target_port` accepts the full `PortId` range (0, 1, 2) — out-of-range values are caller's responsibility, not enum's.
- T2 partial (AgentBatch construction depends on this).

## Invariants Touched

- None at type level (purely structural).

## Notes

- Module location: SPEC-21 §3.3 R17 specifies `src/partition/streaming.rs` OR `src/merge/grid.rs`; SPEC-21 §3.2 R10 specifies `src/bench/streaming.rs` OR `src/io/streaming.rs` for the helper. The streaming-pipeline types (this task + TASK-0521 + TASK-0524) belong with the pipeline orchestrator — DEVELOPER decision, but `src/partition/streaming.rs` is the recommended home (closest to SPEC-04 split() which the streaming pipeline parallels).
- The enum is pure-Core-layer per SPEC-21 R9 / R16 (no async, no tokio, no I/O).
- Forward-ref variant carries `target_agent_id` separately because the target agent does not yet exist when the directive is emitted — there is no addressable port pair to reference.

## DAG Links

- **Predecessors:** none.
- **Successors:** TASK-0521 (AgentBatch consumes), TASK-0555 (pending connection store consumes).
