# TASK-0540: Add `Benchmark::make_net_stream` default impl + `default_chunked_iter` helper

**Spec:** SPEC-21 §3.2 R10, R11, R16; SPEC-21 §3.8 A4 (consumer of TASK-0513 amendment).
**Requirements:** R10 (default-impl-bearing trait method), R11 (`make_net` UNCHANGED — source-of-truth materialization), R16 (pure-Core iterator + R10/R11 produce isomorphic nets when collected per T6).
**Priority:** P0 (blocker for streaming generators and the entire chunked pipeline).
**Status:** TODO
**Depends on:** TASK-0513 (SPEC-09 amendment landed), TASK-0521 (AgentBatch type exists), TASK-0520 (ConnectionDirective).
**Blocked by:** none
**Estimated complexity:** S (~30 LoC trait amendment + helper; ~80 LoC tests).
**Bundle:** SPEC-21 Streaming Generation — Phase D (generators).

## Context

Per SPEC-21 R10, the `Benchmark` trait (SPEC-09 R2, in `src/bench/mod.rs`) gains a new method WITH A DEFAULT IMPLEMENTATION:

```rust
fn make_net_stream(
    &self,
    size: u32,
    chunk_size: usize,
) -> Box<dyn Iterator<Item = AgentBatch>> {
    Box::new(default_chunked_iter(self.make_net(size), chunk_size))
}
```

The default implementation MUST be sufficient to keep all 13 SPEC-09 benchmark implementations compiling unchanged (closes SC-008 per Round 2). Generators that benefit from native streaming (`ep_annihilation` per R12 MUST; others SHOULD) override.

**`default_chunked_iter` helper (per SPEC-21 §3.2 R10).** Lives in `src/bench/streaming.rs` (or `src/io/streaming.rs` — SPEC-21 §3.2 R10 explicitly notes either location):

```rust
pub fn default_chunked_iter(net: Net, chunk_size: usize) -> impl Iterator<Item = AgentBatch>
```

Walks `net.agents` in id order, emitting `AgentBatch` values whose connection directives are all `Resolved` (no forward references arise from a fully materialized net). The default path forfeits the memory benefit of streaming but preserves the API contract.

R10/R11 isomorphism contract (per R11): `make_net_stream(size, chunk_size).flatten() ~ make_net(size)` (up to T6 isomorphism per §7.2). Verified by T6 in TASK-0567.

**Migration table (per TASK-0513 / SC-008 closure):** the 13 existing benchmark implementations remain valid without per-implementation edits — they all use the default impl. Only TASK-0541 (ep_annihilation R12 MUST) and TASK-0542 (dual_tree R12 SHOULD) override.

## Acceptance Criteria

- [ ] Add `make_net_stream` method with default impl to the `Benchmark` trait in `relativist-core/src/bench/mod.rs` per SPEC-09 amendment landed in TASK-0513.
- [ ] Default impl returns `Box::new(default_chunked_iter(self.make_net(size), chunk_size))`.
- [ ] Implement `pub fn default_chunked_iter(net: Net, chunk_size: usize) -> Box<dyn Iterator<Item = AgentBatch>>` in `relativist-core/src/bench/streaming.rs` (DEVELOPER chooses bench/ vs io/; mirror SPEC-21 §3.2 R10's flexibility).
- [ ] `default_chunked_iter` walks `net.agents` in `AgentId` order; emits batches of size `chunk_size`; all directives are `Resolved` (no forward refs from materialized net).
- [ ] All 13 existing benchmark implementations compile unchanged — verified by `cargo build` and `cargo test` passing the 1181/1224 baseline.
- [ ] Doc-test or unit test: collect `make_net_stream(...)` into a `Net` and verify `nets_isomorphic` (SPEC-00 §6.12) with `make_net(...)` for at least 2 representative benchmarks (e.g., `ep_annihilation_pure(20)` and `dual_tree(8)`).
- [ ] Pure-Core constraint preserved: no `tokio`, no `async`, no I/O imports in `bench/streaming.rs`.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/bench/mod.rs` | modify | Add `make_net_stream` method to `Benchmark` trait with default impl. |
| `relativist-core/src/bench/streaming.rs` | create | Add `default_chunked_iter` helper. |

## Key Types / Signatures

```rust
pub trait Benchmark {
    // ... existing methods (make_net, etc.) UNCHANGED per R11 ...

    fn make_net_stream(
        &self,
        size: u32,
        chunk_size: usize,
    ) -> Box<dyn Iterator<Item = AgentBatch>> {
        Box::new(default_chunked_iter(self.make_net(size), chunk_size))
    }
}

pub fn default_chunked_iter(
    net: Net,
    chunk_size: usize,
) -> Box<dyn Iterator<Item = AgentBatch>>;
```

## Test Expectations (forward-ref)

TEST-SPEC-0540:
- Default-impl path: collect `make_net_stream(20, 5)` into a Net and assert `nets_isomorphic` with `make_net(20)` for `ep_annihilation_pure` and `dual_tree`.
- All 13 benchmarks compile unchanged: regression gate via TASK-0600.
- T6 partial (full equivalence test in TASK-0567).

## Invariants Touched

- D1 (extended for streaming) — preserved by R10/R11 isomorphism contract.
- R16 pure-Core — preserved.

## Notes

- Module location decision: SPEC-21 R10 explicitly accepts both `src/bench/streaming.rs` and `src/io/streaming.rs`. Recommendation: `src/bench/streaming.rs` (closer to the `Benchmark` trait definition); however if `src/io/streaming.rs` already houses generator-related streaming code (TASK-0541 / TASK-0542 may add to io/), keep them co-located. DEVELOPER decides.
- The default path materializes the net then slices; this is **memory-equivalent to v1**, providing no streaming benefit. The benefit is unlocked by per-generator overrides (TASK-0541 / TASK-0542).
- Consumed by TASK-0541 (ep_annihilation override), TASK-0542 (dual_tree override), TASK-0554 (pipeline calls `make_net_stream`).

## DAG Links

- **Predecessors:** TASK-0513, TASK-0520, TASK-0521.
- **Successors:** TASK-0541, TASK-0542, TASK-0554.
