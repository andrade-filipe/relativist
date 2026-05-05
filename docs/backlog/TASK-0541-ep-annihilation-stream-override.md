# TASK-0541: Implement `ep_annihilation_stream` native override

**Spec:** SPEC-21 §3.2 R12 (MVP MUST), R13 (informative); SPEC-21 §4.5 example signature.
**Requirements:** R12 MVP — `ep_annihilation` MUST support streaming.
**Priority:** P0 (R12 MVP MUST; only generator REQUIRED to override the default-impl).
**Status:** TODO
**Depends on:** TASK-0540 (Benchmark trait amendment + default_chunked_iter helper).
**Blocked by:** none
**Estimated complexity:** S (~80 LoC production + ~80 LoC tests).
**Bundle:** SPEC-21 Streaming Generation — Phase D (generators).

## Context

Per SPEC-21 R13: ep_annihilation streaming is trivial — each batch emits independent ERA-ERA pairs. No wires connect different pairs, so no forward references are needed and each batch is self-contained.

Per SPEC-21 §4.5 example signature:

```rust
fn ep_annihilation_stream(
    size: u32,
    chunk_size: usize,
) -> Box<dyn Iterator<Item = AgentBatch>>
```

**Pair-batching discipline (per §4.5 doc-comment):** Each pair = 2 agents (ERA, ERA), 1 connection (p0 ↔ p0). Pairs per batch = `chunk_size / 2`. Total batches = `ceil(size / (chunk_size / 2))`. A batch may contain `chunk_size - 1` agents if the last pair would exceed the limit, to avoid splitting a pair across batches.

**R15 monotonicity contract preserved (per §3.2 R15):** AgentIds are assigned in a globally unique, monotonically increasing sequence across all batches (max id in batch k < min id in batch k+1).

**Override placement:** This override is on the SPEC-09 `Benchmark` trait implementation for `EpAnnihilation` (the bench struct in `src/bench/`). The free-function form `ep_annihilation_stream(size, chunk_size)` per §4.5 is the implementation helper; the trait impl wraps it.

## Acceptance Criteria

- [ ] Define `pub fn ep_annihilation_stream(size: u32, chunk_size: usize) -> Box<dyn Iterator<Item = AgentBatch>>` in `relativist-core/src/io/generators.rs` (or `src/bench/streaming.rs` if generators file is unsuitable — DEVELOPER decides).
- [ ] Pair-batching: each batch contains an integer number of pairs; agent count per batch is even (≤ `chunk_size`); last batch may be smaller.
- [ ] Each pair emits exactly 2 agents `(2k, ERA), (2k+1, ERA)` and 1 `Resolved` directive `((2k, 0), (2k+1, 0))` (principal-port connection).
- [ ] AgentIds are monotonically increasing across batches per R15.
- [ ] No `Pending` directives — ep_annihilation has no cross-batch wires (R13 informative).
- [ ] Override `Benchmark::make_net_stream` for the `EpAnnihilation` impl (analogous overrides for `ep_annihilation_con` / `ep_annihilation_dup` per TASK-0513 migration table are deferred per R12 SHOULD — captured in this task's notes but not required).
- [ ] Pure-Core: no async, no tokio, no I/O.
- [ ] T5 (ep_annihilation full pipeline): `generate_and_partition_chunked(ep_annihilation_stream(100, 20), 4, RoundRobin)` produces 200 agents across 4 partitions, no pending refs left at end (verified by TASK-0554).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/io/generators.rs` *(or chosen location)* | modify | Add `ep_annihilation_stream` free function. |
| `relativist-core/src/bench/...` *(EpAnnihilation impl)* | modify | Override `Benchmark::make_net_stream` to call the streaming variant. |

## Key Types / Signatures

```rust
pub fn ep_annihilation_stream(
    size: u32,
    chunk_size: usize,
) -> Box<dyn Iterator<Item = AgentBatch>>;

// Trait impl override:
impl Benchmark for EpAnnihilation {
    fn make_net_stream(
        &self,
        size: u32,
        chunk_size: usize,
    ) -> Box<dyn Iterator<Item = AgentBatch>> {
        ep_annihilation_stream(size, chunk_size)
    }
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0541:
- Pair-batch invariant: every batch has an even agent count.
- Resolved-only: no `Pending` directives in any batch.
- R15 monotonicity: max id in batch k < min id in batch k+1.
- Total agents = `2 * size` after stream exhaustion (per ep_annihilation semantics).
- T5 (streaming pipeline produces valid partitions on ep_annihilation): full pipeline integration test.
- T8 (chunk size independence): vary chunk_size from 2 to size; merged result MUST be isomorphic to sequential baseline (covered jointly with TASK-0567).

## Invariants Touched

- C1 (preserved — every pair's two agents enter exactly one batch).
- C2 (preserved — each pair's principal-port wire is a `Resolved` directive in the same batch).
- C3 (no cross-batch wires → no border wires → bijectivity trivially preserved).
- R15 (monotonicity preserved by construction).

## Notes

- The R12 SHOULD overrides for `ep_annihilation_con` / `ep_annihilation_dup` (per TASK-0513 migration table) are deferred to a future task — they reuse the same pair-batching pattern with CON/DUP symbols. NOT required for SPEC-21 MVP.
- The free-function form is reusable across the CON/DUP variants (parameterize on `Symbol`).
- Consumed by TASK-0554 (pipeline integration), TASK-0567 (T6/T8 isomorphism), TASK-0600 (regression gate).

## DAG Links

- **Predecessors:** TASK-0540.
- **Successors:** TASK-0554, TASK-0567, TASK-0600.
