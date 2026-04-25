# TEST-SPEC-0524: StreamingPartitionStrategy trait

**SPEC-21 §7 ID:** plumbing (T1 / T9 verification deferred to TASK-0530 / TASK-0531).
**Owning task:** TASK-0524.
**Parent spec:** SPEC-21 §3.1 R1, R2, R3, R7, R8, R9; §4.2 trait definition.
**Type:** unit (compile-time trait surface verification).
**Theory anchor:** ARG-002 Q2 (σ allocation function).

---

## Inputs / Fixtures

- The trait definition `StreamingPartitionStrategy` in `src/partition/streaming.rs`.
- A test-only `IdentityStrategy` impl that always returns `WorkerId(0)` (used to verify the trait is implementable).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0524-01 | `trait_compiles_standalone` | the trait | invoke `cargo doc -p relativist-core --no-deps` and inspect rendered HTML | both methods present: `allocate_batch(&mut self, batch: &AgentBatch, num_workers: u32) -> Vec<WorkerId>` and `finalize(self) -> StreamingPartitionStats` (or whatever the §4.2 signature is). |
| UT-0524-02 | `trait_uses_mut_self` | the trait `allocate_batch` signature | grep | `&mut self` (R2: stateful operation). |
| UT-0524-03 | `trait_object_constructible` | `IdentityStrategy` | `let _: Box<dyn StreamingPartitionStrategy> = Box::new(IdentityStrategy::default());` | compiles (trait is object-safe). |
| UT-0524-04 | `pure_core_no_async_no_tokio` | the trait + impls in `src/partition/` | grep for `async fn`, `tokio::`, `await` keywords | NONE (R9: pure-Core layer). |
| UT-0524-05 | `pure_core_no_io` | impls in `src/partition/` | grep for `std::io`, `std::fs`, `std::net` | NONE (R9: no I/O). |
| UT-0524-06 | `r7_contract_documented_in_rustdoc` | the `allocate_batch` Rustdoc | grep for "C1" or "complete coverage" or "every agent assigned exactly once" | substring present (R7 contract on the trait surface). |
| UT-0524-07 | `r8_determinism_documented_in_rustdoc` | the trait's Rustdoc | grep for "deterministic" or "identical across invocations" | substring present (R8 contract). |
| UT-0524-08 | `finalize_consumes_self` | the `finalize` method signature | grep | `self` (not `&self`, not `&mut self`); the strategy is consumed at finalize. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | A future strategy that holds a tokio handle internally | UT-0524-04/05 fail; the strategy violates R9. The grep MUST be strict: `tokio::sync::*` types, async functions, `.await`. |
| EC-2 | A strategy that uses interior mutability (`RefCell`) instead of `&mut self` | UT-0524-02 still passes (the trait signature is `&mut self`); the strategy is internally re-entrant but the trait surface enforces the discipline. |
| EC-3 | A strategy that returns a `Vec<WorkerId>` of length != `batch.agents.len()` | trait surface allows this; downstream pipeline (TASK-0553) MUST detect the mismatch and error out. The trait test does NOT enforce length parity. |

## Invariants asserted

- R1, R2, R3 (trait definition + signatures).
- R7 (C1 contract on the trait surface).
- R8 (determinism contract).
- R9 (pure Core; no async, no tokio, no I/O).
- C1 (Complete Agent Coverage) — preserved by R7 contract.
- D1 (extended for streaming) — preserved by R7 + downstream pipeline.

## ARG/DISC/REF citation

- ARG-002 Q2 (σ allocation function — the streaming trait IS the σ in the streaming regime).

## Determinism notes

Pure synchronous compile-time test. UT-0524-04/05 are CI-lint patterns (rg-grep). The grep MUST exclude doc comments (only check actual code lines) to avoid false positives from words like "tokio" appearing in Rustdoc explanatory text.

## Cross-test dependencies

- TEST-SPEC-0530 (RoundRobinStreamingStrategy) — concrete impl exercising T1.
- TEST-SPEC-0531 (FennelStreamingStrategy) — concrete impl exercising T9.
- TEST-SPEC-T1 (assignment correctness) — behavioral coverage of R7 / R8.
