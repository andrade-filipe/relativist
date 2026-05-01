# TASK-0602 — Extend `BenchmarkSuiteConfig` with Tier 3 fields (Phase C-1)

**Phase:** C-1 (D-011 bench harness wiring — foundation struct extension)
**Bundle:** D-011 — Tier 3 Hardening + Bench Enablement
**Status:** PENDING
**Priority:** P0 (foundation for all Phase C work; blocks C-2/C-3/C-4/C-5)
**Spec:** SPEC-09 R18a–R18g, R37c (committed `82b2d27`); SPEC-21 §3.8 A3 (`GridConfig` chunk_size + max_pending_lifetime); SPEC-22 R10b (recycle_under_delta).
**Origin:** D-011 plan §C-1 — bench harness has no Tier 3 flags today.
**Estimated complexity:** S (~50 LoC production + ~40 LoC tests)
**Estimated stages duration:** Stages 2→3→4→5→6 over ~0.5 day.

---

## Context

Per D-011 plan: `BenchmarkSuiteConfig` (`relativist-core/src/bench/mod.rs:231-251`) carries 11 fields today, none of which expose Tier 3 (streaming + recycling + sparse) configuration. The bench harness therefore always invokes `run_grid` with `GridConfig::default()` — eager path. SPEC-09 R18a–R18g require the bench harness to support the chunked streaming path with measurable peak-construction memory.

This task extends the struct with 4 new fields. It is the smallest, most foundational task of Phase C; downstream tasks consume these fields.

## Dependencies

- **SPEC commit `82b2d27`** — SPEC-09 R18a–R18g + R37c amendment defining the new bench metrics. (Hard prerequisite — already landed.)
- TASK-0596 (B-1) recommended landed first (so TCP path is known good before bench wires it), but not strictly required for C-1 itself.

## Files in scope

| File | Change |
|------|--------|
| `relativist-core/src/bench/mod.rs:231-251` | Add 4 fields to `BenchmarkSuiteConfig`. |
| `relativist-core/src/bench/mod.rs` (related: enum defs) | Define `RecyclePolicy` enum (`DisableUnderDelta` \| `BorderClean`) and `NetRepresentation` enum (`Dense` \| `Sparse`) if not already present. |
| `relativist-core/tests/bench_suite_config_serde.rs` (new) | Round-trip test: build config → debug/clone → assert fields preserved. (Note: `BenchmarkSuiteConfig` derives Debug + Clone, not Serialize — so test stays at struct-level, not serde JSON.) |

## Files explicitly OUT of scope

- Path selection logic (`bench/suite.rs`) — that's TASK-0604 (C-2/C-4).
- CLI parsing — that's TASK-0603 (C-3).
- Memory probe — that's TASK-0605 (C-5).
- The actual `GridConfig` plumbing in `merge::run_grid` — already in tree from D-010.

## Key types / signatures

```rust
// In relativist-core/src/bench/mod.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecyclePolicy {
    DisableUnderDelta,
    BorderClean,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetRepresentation {
    Dense,
    Sparse,
}

pub struct BenchmarkSuiteConfig {
    // ... existing 11 fields ...
    /// None → eager path (status quo); Some(N) → streaming with chunk size N. (SPEC-09 R18a, SPEC-21 §3.8 A3.)
    pub chunk_size: Option<u32>,
    /// SPEC-21 R37g pending-store memory bound. Default: 16.
    pub max_pending_lifetime: u32,
    /// SPEC-22 R10b recycling policy under delta+streaming. Default: DisableUnderDelta.
    pub recycle_policy: RecyclePolicy,
    /// Net representation during construction. Default: Dense. SPEC-22 R12 (sparse net).
    pub representation: NetRepresentation,
}
```

## Acceptance criteria

1. `BenchmarkSuiteConfig` carries the 4 new fields with the exact types and defaults as specified above.
2. Defaults match the eager-path status quo (`chunk_size: None`, `representation: Dense`).
3. The two new enums (`RecyclePolicy`, `NetRepresentation`) derive `Debug, Clone, Copy, PartialEq, Eq`.
4. Bench suite construction sites (current callers of `BenchmarkSuiteConfig::default()`-equivalent or struct-literal creation) compile against the extended struct (additive change).
5. Cargo build + tests pass with zero regression.
6. New unit test asserts the defaults are exactly as specified (regression guard against accidental default drift).

## Test floor delta expected

**+2 to +3 tests** added.

## Notes

- Pure additive change; backward-compat for downstream code that uses struct-update syntax (`..config`).
- The 4 fields are decoupled — downstream tasks (C-2/C-3/C-4/C-5) wire them independently.
