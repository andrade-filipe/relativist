# TASK-0523: Define `ChunkedPartitionResult` struct

**Spec:** SPEC-21 §4.1 type definitions; SPEC-21 §3.3 R20, R21.
**Requirements:** ChunkedPartitionResult struct from §4.1; structurally compatible with PartitionPlan (SPEC-04).
**Priority:** P0 (foundational return type for the pipeline orchestrator).
**Status:** TODO
**Depends on:** TASK-0522 (StreamingPartitionStats).
**Blocked by:** none
**Estimated complexity:** S (~20 LoC production + ~40 LoC tests).
**Bundle:** SPEC-21 Streaming Generation — Phase B (core types).

## Context

The result of the chunked generation + partitioning pipeline. Structurally equivalent to `PartitionPlan` (SPEC-04) but produced incrementally; directly consumable by the merge protocol (SPEC-05) per R21.

Per SPEC-21 §4.1:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChunkedPartitionResult {
    pub partitions: Vec<Partition>,
    pub borders: HashMap<u32, (PortRef, PortRef)>,
    pub stats: StreamingPartitionStats,
}
```

`Partition` is the SPEC-04 type (carries `subnet`, `worker_id`, `free_port_index`, `id_range`, `border_id_start`, `border_id_end`); `PortRef` is the SPEC-02 type.

R21 mandates structural compatibility with `split()`-produced `PartitionPlan`; R26 mandates that `chunk_size = u32::MAX` short-circuits to `split()` and produces an isomorphic (NOT bit-identical, per SC-014 closure) result. Per SPEC-21 §6.2 (Coexistence with SPEC-04), the result is convertible to `PartitionPlan`:

```rust
PartitionPlan { partitions: result.partitions, borders: result.borders }
```

## Acceptance Criteria

- [ ] Define `pub struct ChunkedPartitionResult` in `relativist-core/src/partition/streaming.rs`.
- [ ] Three fields present: `partitions: Vec<Partition>`, `borders: HashMap<u32, (PortRef, PortRef)>`, `stats: StreamingPartitionStats`.
- [ ] Derive `Debug`, `Clone`, `serde::Serialize`, `serde::Deserialize`.
- [ ] Provide a `From<ChunkedPartitionResult> for PartitionPlan` conversion (or equivalent helper) per §6.2 — drops `stats` and lifts `partitions` + `borders`.
- [ ] Document Rustdoc per SPEC-21 §4.1: structurally equivalent to PartitionPlan but produced incrementally.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/partition/streaming.rs` | modify | Add `ChunkedPartitionResult` struct + `From` conversion to `PartitionPlan`. |

## Key Types / Signatures

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChunkedPartitionResult {
    pub partitions: Vec<Partition>,
    pub borders: HashMap<u32, (PortRef, PortRef)>,
    pub stats: StreamingPartitionStats,
}

impl From<ChunkedPartitionResult> for PartitionPlan {
    fn from(result: ChunkedPartitionResult) -> Self {
        PartitionPlan {
            partitions: result.partitions,
            borders: result.borders,
        }
    }
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0523:
- Construct ChunkedPartitionResult; verify serde round-trip.
- Verify `From` conversion preserves `partitions` and `borders` 1:1; `stats` is dropped (no information loss for downstream merge).
- T6 partial (streaming vs batch isomorphism — verifies the full `From` round-trip in the actual pipeline run, TASK-0567).

## Invariants Touched

- None at type level.

## Notes

- The `stats` field is dropped by the `From` conversion because `PartitionPlan` (SPEC-04) does not carry observability data; `stats` is consumed via SPEC-21's own observability surface (TASK-0584 / SPEC-11).
- Consumed by TASK-0554 (pipeline assembly), TASK-0556 (final result assembly), TASK-0567 (R26 short-circuit → comparison with `split()` output).

## DAG Links

- **Predecessors:** TASK-0522.
- **Successors:** TASK-0554, TASK-0556, TASK-0567.
