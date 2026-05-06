# TASK-0700 — D-014-MEMPROBE: cross-platform `MemoryProbe` (current + peak + RAM fraction)

**Phase:** D-014 (Stress Curve Campaign) — Stage 3 DEV scope
**Bundle:** D-014 — Stress Curve Campaign
**Status:** TODO
**Priority:** P0 (foundational dependency for StopRule TASK-0701 and CSV schema TASK-0703)
**Spec:** none (campaign methodology — no SPEC delta). Adds new module under `relativist-core/src/bench/`.
**Depends on:** none (independent, ship first).
**Estimated complexity:** S–M (~120 LoC production + ~60 LoC unit tests).

---

## Context

The stress-curve campaign requires per-rep peak and current Resident Set Size (RSS) plus the fraction of total host RAM that pico represents. The existing `relativist-core/src/bench/memory.rs` only exposes `VmHWM` reads via `/proc/self/status` (Linux-only) and returns `0` on every other target — this is insufficient for the campaign because the campaign also runs on the developer's Windows host (per design doc §4.4 matrix in-process arm) and needs the **current** RSS at end-of-rep (not just monotonic peak) and a **fraction-of-total** denominator for the `StopRule` 80%-of-RAM gate.

Per design doc §4.3 the new struct is platform-specific opaque, with `current_bytes`, `peak_bytes`, and `as_fraction_of_total` methods. **It does not replace** `bench/memory.rs::sample_vmhwm` — that path stays for SPEC-09 R18/R18a callers. The new probe is additive.

Justification for `peak_bytes` using `VmHWM` (Linux) / `PeakWorkingSetSize` (Windows): both are process-wide monotonic non-decreasing counters. The campaign runs each rep in a child process (per design doc §5 Phase 1, `Command::spawn`), so the peak read at end-of-rep IS that rep's peak. The "sparse-vs-dense indistinguishable" failure mode logged in D-011/D-012 does NOT apply here because the comparison axis is `(workload, W, N)` across distinct child processes, not representations sharing a single process.

## Files in scope (file:line pointers)

| File | Change |
|------|--------|
| `relativist-core/src/bench/memory_probe.rs` | **CREATE.** New module with the public `MemoryProbe` struct + `BenchError` variants for probe errors. ~120 LoC. |
| `relativist-core/src/bench/mod.rs` | **MODIFY.** Add `pub mod memory_probe;` (1 line). |
| `relativist-core/src/bench/memory.rs` | **DO NOT TOUCH.** Existing legacy probe stays for SPEC-09 R18/R18a. |
| `relativist-core/src/error.rs` (or wherever `BenchError` lives) | **MODIFY (if needed).** Add `MemoryProbe(String)` variant with `thiserror::Error` plumbing if a new variant is required. ~5 LoC. |

## Files explicitly OUT of scope

- `bench/memory.rs` — unchanged, keeps Linux VmHWM legacy path.
- `bench/suite.rs` — wiring into the suite is TASK-0703's territory.
- `bench/stop_rule.rs` — consumer; that is TASK-0701.
- Any I/O in `protocol/`, `merge/`, `partition/`, `net/`, `reduction/` — pure-core inviolable.
- macOS support — explicit non-goal for this campaign (host hardware is Linux + Windows; document the macOS path as `unsupported` with `cfg(target_os = "macos")` returning `BenchError::MemoryProbe("macos unsupported")`).

## Required public API

```rust
// relativist-core/src/bench/memory_probe.rs
pub struct MemoryProbe { /* opaque, platform-specific cache */ }

impl MemoryProbe {
    /// Construct a new probe. Captures total system RAM (denominator for
    /// `as_fraction_of_total`).
    pub fn new() -> Result<Self, BenchError>;

    /// Current RSS in bytes (VmRSS on Linux; WorkingSetSize on Windows).
    pub fn current_bytes(&self) -> Result<u64, BenchError>;

    /// Peak RSS in bytes since process start (VmHWM on Linux;
    /// PeakWorkingSetSize on Windows). Monotonic non-decreasing.
    pub fn peak_bytes(&self) -> Result<u64, BenchError>;

    /// Convert a byte count to a fraction of total physical RAM
    /// (excluding swap on Linux). Used by StopRule for the 80% gate.
    pub fn as_fraction_of_total(&self, bytes: u64) -> f64;
}
```

## Acceptance criteria

1. `MemoryProbe::new()` succeeds on Linux (any kernel ≥ 3.10) and Windows 10+ in a normal user-mode process.
2. `current_bytes` and `peak_bytes` return monotonically reasonable values: `peak >= current` always; both > 0 after the test allocates and **black_boxes** a 100 MiB buffer.
3. On macOS, all methods return `BenchError::MemoryProbe(...)` (graceful failure, not panic).
4. `as_fraction_of_total(total_ram_bytes) == 1.0` exactly (within `f64::EPSILON * 4`) — by definition.
5. Unit tests cover: (a) probe construction succeeds, (b) `peak >= current`, (c) allocating ~100 MiB raises `current` by ≥ 50 MiB (slack for system page-cache reuse), (d) fraction-of-total is in `[0.0, 1.0]` and strictly positive.
6. `cargo test` floor: **+4 default = ≥ 1802 default** (4 new unit tests in this module).
7. `cargo test --features zero-copy` floor: **+4 = ≥ 1846**.
8. `cargo test --features streaming-no-recycle` floor: **+4 = ≥ 1793**.
9. `cargo test --release` floor: **+4 (matching the debug count for the new tests) = ≥ 1744**.
10. v1 floor (690) inviolable.
11. `cargo clippy --all-features -- -D warnings` clean.
12. `cargo fmt --check` clean.

## Test floor delta

**+4 default** (one new module test target with 4 unit tests). New floor expectations:
- default ≥ 1802
- zero-copy ≥ 1846
- streaming-no-recycle ≥ 1793
- release ≥ 1744

## Implementation hints

1. On Linux read `/proc/self/status` and parse `VmRSS:` (current) and `VmHWM:` (peak). Both are kB → multiply by 1024.
2. On Linux read `/proc/meminfo` `MemTotal:` for the fraction denominator. **Exclude swap** per design doc §5 Phase 2 — do NOT add `SwapTotal:`.
3. On Windows, link `kernel32` and call `GetCurrentProcess` + `GetProcessMemoryInfo` (`PROCESS_MEMORY_COUNTERS` struct). Fields: `WorkingSetSize` (current), `PeakWorkingSetSize` (peak). For total RAM use `GlobalMemoryStatusEx` (`MEMORYSTATUSEX.ullTotalPhys`).
4. The Windows path needs a `[target.'cfg(target_os = "windows")'.dependencies]` entry — prefer the `windows-sys` crate (lightweight, no proc-macro) over `winapi`. Add to `relativist-core/Cargo.toml` only the features `Win32_System_ProcessStatus` and `Win32_System_SystemInformation`.
5. Cache `MemTotal` / `ullTotalPhys` in `MemoryProbe::new` — re-reading per call is wasteful and never changes within a process lifetime.
6. Keep all `unsafe` Windows FFI inside ONE function with a `// SAFETY: ...` comment per CLAUDE.md coding standards.
7. Do NOT use `tracing` macros for diagnostics inside probe (cold path; could be called from a panic handler later); return errors instead.

## Estimated LoC

- Production: ~120 LoC (Linux ~40, Windows ~50, common ~30).
- Tests: ~60 LoC (4 unit tests, no integration).
- Total: ~180 LoC. Under the 200 LoC ceiling.

## Cross-references

- Design doc: `docs/superpowers/specs/2026-05-05-stress-test-large-nets-design.md` §4.2 row 2, §4.3 (the `MemoryProbe` interface).
- Existing legacy probe (do NOT touch): `relativist-core/src/bench/memory.rs`.
- Will be consumed by: TASK-0701 (StopRule), TASK-0703 (CSV schema), TASK-0707 (integration tests), TASK-0704 (script).
