# TASK-0513: [SPEC-09 amendment A4] `Benchmark` trait gains `make_net_stream` with default impl

**Spec:** SPEC-21 §3.8 A4 (closes SC-008); SPEC-21 §3.2 R10, R11.
**Requirements:** A4 (formal SPEC-09 R2 amendment with default-impl-bearing trait extension).
**Priority:** P0 (blocker for TASK-0540 trait method production and ALL streaming-generator tasks).
**Status:** TODO
**Depends on:** none (Phase A).
**Blocked by:** none
**Estimated complexity:** S (~25 LoC SPEC-09 next-revision diff; spec text only).
**Bundle:** SPEC-21 Streaming Generation — predecessor-spec amendment cluster (Phase A).
**Tag:** `[SPEC-09 amendment]`

## Context

SPEC-09 R2 declares the `Benchmark` trait with the required method `fn make_net(&self, size: u32) -> Net`. SPEC-21 R10 mandates a new method `fn make_net_stream(&self, size: u32, chunk_size: usize) -> Box<dyn Iterator<Item = AgentBatch>>` — with a **default implementation** so that all 13 existing SPEC-09 implementations remain valid without per-implementation edits (the SC-008 closure decision in Round 2):

```rust
fn make_net_stream(
    &self,
    size: u32,
    chunk_size: usize,
) -> Box<dyn Iterator<Item = AgentBatch>> {
    // Default: collect the eager net and slice it into chunks.
    Box::new(default_chunked_iter(self.make_net(size), chunk_size))
}
```

The default-impl path materializes the net then slices it (memory-equivalent to v1; no streaming benefit, but no break). Generators that benefit from native streaming MUST override (R12 — `ep_annihilation` MUST; others SHOULD).

R11 reframes existing `make_net` as the **source-of-truth materialization path** — it remains a required method (no default impl) so the streaming default has a fallback. R10 ↔ R11 isomorphism contract: when the streaming variant is collected, it MUST produce a net isomorphic to `make_net(size)` (T6, §7.2).

**Migration path enumeration (per SC-008 / Phase B effort estimate ~30 LoC vs ~520 LoC).** The 13 existing benchmark implementations affected by the default-impl decision are documented for downstream task TASK-0540 (~ all under `src/bench/` directory). The default-impl choice means none require per-implementation edits; only those that derive native-streaming benefit override:

| Benchmark | Native streaming benefit | Override status |
|-----------|--------------------------|-----------------|
| `ep_annihilation_pure` (SPEC-09 R10 family) | YES — independent ERA-ERA pairs, no cross-batch wires | OVERRIDE per R12 MUST (TASK-0541) |
| `ep_annihilation_con` | YES — independent CON-CON pairs | OVERRIDE per R12 SHOULD (deferred — captured in TASK-0541's notes) |
| `ep_annihilation_dup` | YES — independent DUP-DUP pairs | OVERRIDE per R12 SHOULD (deferred) |
| `dual_tree` | YES — but requires forward references | OVERRIDE per R12 SHOULD (TASK-0542) |
| `mixed_net` | NO — small fixed sizes; default-impl path acceptable | DEFAULT (no override) |
| `church_add` (SPEC-14) | TBD — depends on encoder API stability | DEFAULT (no override) |
| `church_mul` (SPEC-14) | TBD | DEFAULT (no override) |
| `m5_*` (M5 milestone family) | YES — large nets benefit most | OVERRIDE per R12 SHOULD (deferred to M5 tasks) |
| Remaining 5 benchmarks | NO (small / synthetic) | DEFAULT (no override) |

Total Phase B effort estimate: ~30 LoC for the trait amendment + `default_chunked_iter` helper, plus per-generator overrides on opt-in basis (TASK-0541 ~50 LoC, TASK-0542 ~80 LoC). The alternative (no default impl) would have forced ~520 LoC of mechanical implementation across 13 benchmarks even for those that derive no benefit.

## Acceptance Criteria

- [ ] ESPECIALISTA EM SPECS lands the SPEC-09 next-revision diff amending R2 to add the `make_net_stream` method with default impl (verbatim per SPEC-21 §3.8 A4 *New text*).
- [ ] R2 explicitly states the default-impl path materializes via `make_net` and slices via `default_chunked_iter`.
- [ ] R2 cross-references SPEC-21 R10 (default-impl mandate), R11 (`make_net` remains source-of-truth), R12 (override expectations).
- [ ] R2 documents that all 13 existing implementations remain valid without per-implementation edits.
- [ ] §6 (SPEC-09 implementation list) annotated to flag which benchmarks SHOULD override (per the migration table above) and which use the default path.
- [ ] The `default_chunked_iter` helper signature documented in §4 of SPEC-09 (or referenced as living in `src/bench/streaming.rs` per SPEC-21 §3.2 R10).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `codigo/relativist/specs/SPEC-09-benchmarks.md` | modify (by ESPECIALISTA EM SPECS only) | R2 amended — adds `make_net_stream` with default impl. §6 implementation list annotated with override status table. |

## Test Expectations (forward-ref)

TEST-SPEC-0513 — covered by:
- TEST-SPEC-0540 default-impl path equivalence (T6 isomorphism, §7.2).
- TEST-SPEC-0541 ep_annihilation native-streaming override.
- TEST-SPEC-0542 dual_tree native-streaming override (forward references).
- T6 (streaming vs batch isomorphism — global gate via TASK-0567).

## Invariants Touched

- D1 (Split/Merge Identity, extended for streaming) — preserved by R10 ↔ R11 isomorphism contract.

## Notes

- This is a spec-text-only task (no production code).
- The 13-benchmark migration table is informative; the actual override decisions per-benchmark live in TASK-0541 / TASK-0542 / future M5 tasks.
- No regression risk: existing 1181 / 1224 tests pass via the default-impl path until a benchmark explicitly opts into override.
- Consumed by TASK-0540 (trait method + helper production), TASK-0541 (ep_annihilation override), TASK-0542 (dual_tree override).

## DAG Links

- **Predecessors:** none.
- **Successors:** TASK-0540, TASK-0541, TASK-0542.
