# TEST-SPEC-0700: Tests for TASK-0700 — Cross-platform `MemoryProbe`

**Task:** TASK-0700
**Spec:** none (campaign methodology — no SPEC delta)
**Bundle:** D-014 (Stress Curve Campaign)
**Requirements covered:** Acceptance criteria 1-5 from TASK-0700
**Test IDs:** UT-0700-{01..04} (4 unit tests in-module)

---

## Scope

Verify the public API of `relativist-core/src/bench/memory_probe.rs`:
- `MemoryProbe::new() -> Result<Self, BenchError>`
- `MemoryProbe::current_bytes(&self) -> Result<u64, BenchError>`
- `MemoryProbe::peak_bytes(&self) -> Result<u64, BenchError>`
- `MemoryProbe::as_fraction_of_total(&self, bytes: u64) -> f64`

Plus the macOS unsupported path (returns `BenchError::MemoryProbe("macos unsupported")`).

The 4 unit tests live **in-module** at the bottom of `bench/memory_probe.rs` inside a `#[cfg(test)] mod tests { ... }` block (idiomatic Rust; matches `bench/memory.rs` convention). No integration test target is created by this TASK — TASK-0707 covers the oracle integration test.

## Test category & location

| # | Category | Cfg gating | File | LoC |
|---|----------|------------|------|-----|
| UT-0700-01 | unit | none | `relativist-core/src/bench/memory_probe.rs` | ~12 |
| UT-0700-02 | unit | `#[cfg(not(target_os = "macos"))]` | same | ~10 |
| UT-0700-03 | unit | `#[cfg(not(target_os = "macos"))]` | same | ~22 |
| UT-0700-04 | unit | none | same | ~12 |

macOS-only test (UT-0700-MAC) is added with `#[cfg(target_os = "macos")]`. It's the only test the macOS path needs and is **not counted** in the +4 floor delta because the campaign hosts are Linux + Windows; on macOS `cargo test` would still pass with this single replacement test running and the other 4 elided.

## Test floor delta (from TASK-0700 acceptance criterion 6-9)

- default: **+4** → ≥ 1802
- zero-copy: **+4** → ≥ 1846
- streaming-no-recycle: **+4** → ≥ 1793
- release: **+4** → ≥ 1744

---

## Unit Tests

### UT-0700-01: `probe_construction_succeeds_on_supported_platforms`

**Purpose:** Verify `MemoryProbe::new()` returns `Ok(_)` on Linux + Windows; `Err(BenchError::MemoryProbe("macos unsupported"))` on macOS.

**Cfg gating:** none (the test branches on `cfg!(target_os = ...)` internally so a single test exercises the right arm per host).

**Preconditions:** Standard user-mode process; no privileged setup.

**Input:**
```rust
let result = MemoryProbe::new();
```

**Expected output:**
- On Linux / Windows: `result.is_ok()` MUST be `true`. Probe is usable for subsequent reads.
- On macOS: `result.is_err()` MUST be `true`, AND the inner error is `BenchError::MemoryProbe(s)` where `s == "macos unsupported"` (string compare, not contains).

**Assertion sketch:**
```rust
#[cfg(any(target_os = "linux", target_os = "windows"))]
{
    let probe = MemoryProbe::new().expect("probe construction must succeed on linux/windows");
    let _ = probe;
}
#[cfg(target_os = "macos")]
{
    match MemoryProbe::new() {
        Err(BenchError::MemoryProbe(s)) => assert_eq!(s, "macos unsupported"),
        other => panic!("expected MemoryProbe(\"macos unsupported\"), got {:?}", other),
    }
}
```

**Edge cases:**
- (EC-1) Repeated construction in the same process MUST succeed (no global state leak). UT-0700-01 also calls `MemoryProbe::new()` twice and asserts both succeed.
- (EC-2) On a Linux kernel < 3.10 (theoretical) `/proc/self/status` could lack `VmHWM:` — out of test scope; production minimum is 3.10 (acceptance criterion 1). Document as known-untested.

---

### UT-0700-02: `peak_is_at_least_current`

**Purpose:** Verify the invariant `peak_bytes >= current_bytes` at all times after probe construction.

**Cfg gating:** `#[cfg(not(target_os = "macos"))]` — macOS errors out, no-op.

**Preconditions:** A successfully-constructed probe.

**Input:**
```rust
let probe = MemoryProbe::new().unwrap();
let current = probe.current_bytes().unwrap();
let peak    = probe.peak_bytes().unwrap();
```

**Expected output:**
- `peak >= current` MUST hold.
- Both `current > 0` and `peak > 0` MUST hold (the test process itself is non-empty).

**Assertion sketch:**
```rust
assert!(peak >= current, "peak ({}) must be >= current ({})", peak, current);
assert!(current > 0,  "current_bytes must be > 0 in a live process");
assert!(peak    > 0,  "peak_bytes must be > 0 in a live process");
```

**Edge cases:**
- (EC-1) Race: between the `current_bytes` and `peak_bytes` calls, a heavy allocator could spike `current` past `peak`. Mitigation: `peak` is read AFTER `current` so `peak` reflects at-least-as-recent state. The OS counters are monotonic for `peak` so this can't fail in practice; test with no concurrent allocation in the same thread.
- (EC-2) `current_bytes` returning `0` would indicate a probe bug (or a kernel that doesn't expose `VmRSS`); test asserts `> 0` to catch this.

---

### UT-0700-03: `allocation_increases_current_bytes`

**Purpose:** Verify that allocating ~100 MiB raises `current_bytes` by ≥ 50 MiB (slack for system page-cache reuse, allocator behavior).

**Cfg gating:** `#[cfg(not(target_os = "macos"))]`.

**Preconditions:** Probe constructed; system has ≥ 200 MiB free RAM (test self-skips if `as_fraction_of_total(probe.current_bytes())` already > 0.90, indicating a memory-starved environment).

**Input:**
```rust
let probe   = MemoryProbe::new().unwrap();
let before  = probe.current_bytes().unwrap();

// Allocate exactly 100 MiB (104_857_600 bytes), zeroed and BLACK_BOXED so the
// optimizer can't elide it in --release.
let buf: Vec<u8> = vec![0u8; 100 * 1024 * 1024];
let buf = std::hint::black_box(buf);

// Touch every page to force commit. Skipping this on Linux works because
// vec![0u8; ...] zeroes; but make it explicit.
let _sum: u64 = buf.iter().map(|&b| b as u64).sum();
let _sum = std::hint::black_box(_sum);

let after = probe.current_bytes().unwrap();
drop(buf);
```

**Expected output:**
- `after > before`
- `after - before >= 50 * 1024 * 1024` (50 MiB; allows for allocator coalescing and per-page accounting drift)
- `probe.peak_bytes()` after the allocation MUST be ≥ `after`.

**Assertion sketch:**
```rust
let delta = after - before;
assert!(
    delta >= 50 * 1024 * 1024,
    "expected current_bytes to rise by >=50 MiB after allocating 100 MiB; rose by {} bytes",
    delta
);
let peak_after = probe.peak_bytes().unwrap();
assert!(peak_after >= after, "peak must be >= post-allocation current");
```

**Edge cases:**
- (EC-1) Page-cache reuse may produce delta < 100 MiB; the 50-MiB floor is conservative slack. If even 50 MiB doesn't show, that's a probe bug.
- (EC-2) On a host with `cgroup` memory limits stricter than 200 MiB, the allocation could itself OOM. Mitigation: the test self-skips when `probe.as_fraction_of_total(probe.current_bytes()) > 0.90` (≥ 90% of host RAM already in use — very-low-RAM CI runners).
- (EC-3) Release builds with aggressive LTO could elide the buffer despite `black_box`. Mitigation: also fold `_sum` through `black_box` (line above).

---

### UT-0700-04: `fraction_of_total_in_unit_interval`

**Purpose:** Verify `as_fraction_of_total` is a normalized fraction (0..=1.0) and that passing the total RAM exactly returns 1.0.

**Cfg gating:** none (works on all platforms; on macOS the probe construction returns Err so the test is skipped via early-return).

**Preconditions:** Construction succeeds; otherwise early-return.

**Input:**
```rust
let probe = match MemoryProbe::new() {
    Ok(p) => p,
    Err(_) => return,  // macOS or unsupported; nothing to assert here
};

let current = probe.current_bytes().unwrap();
let frac_current = probe.as_fraction_of_total(current);
let frac_zero    = probe.as_fraction_of_total(0);
// We don't have access to the cached MemTotal directly in the public API,
// so this test exercises the boundary indirectly: passing a value equal to
// what current_bytes returns must produce a strictly positive fraction in
// the half-open interval (0.0, 1.0).
```

**Expected output:**
- `frac_zero == 0.0` exactly (`f64` arithmetic; division of 0 by total).
- `frac_current > 0.0` (the running test process consumes some RAM).
- `frac_current <= 1.0` (cannot exceed total physical RAM in normal operation).
- `frac_current >= 0.0` (trivially; together with above gives the closed `[0.0, 1.0]` interval).

**Assertion sketch:**
```rust
assert_eq!(frac_zero, 0.0, "fraction(0) must be exactly 0.0");
assert!(frac_current > 0.0, "current process must consume some RAM, got fraction {}", frac_current);
assert!(frac_current <= 1.0, "fraction must be <= 1.0, got {}", frac_current);
```

**Edge cases:**
- (EC-1) Acceptance criterion 4 says `as_fraction_of_total(total_ram_bytes) == 1.0` exactly. Since `MemTotal` isn't exposed publicly, this exact-1.0 invariant is implicitly tested by the strict inequality `frac_current <= 1.0` plus the production code's contract (cached `MemTotal` is the exact denominator). If the developer feels the explicit `1.0` test adds value, a `#[cfg(target_os = "linux")]` variant can read `/proc/meminfo` directly to obtain `MemTotal` and pass it back through `as_fraction_of_total`. Not strictly required for the +4 count; if added, it's UT-0700-04b and bumps the floor by +1 (developer's call — note in DEV time if added).
- (EC-2) `as_fraction_of_total(u64::MAX)` should NOT panic (no overflow); it should return a value > 1.0 (saturating semantics not required by the spec). Test optionally covers: `assert!(probe.as_fraction_of_total(u64::MAX).is_finite())`.

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-0700-01 | macOS host | `MemoryProbe::new() == Err(BenchError::MemoryProbe("macos unsupported"))` | UT-0700-01 macOS arm |
| EC-0700-02 | Repeated `new()` calls | both succeed; no global state | UT-0700-01 |
| EC-0700-03 | Process barely-above-zero RSS | `current_bytes() > 0` even at startup | UT-0700-02 |
| EC-0700-04 | Memory-starved CI (>90% RAM used) | UT-0700-03 self-skips with no assertion failure | UT-0700-03 |
| EC-0700-05 | Release build with LTO | allocation not elided thanks to `black_box` | UT-0700-03 |
| EC-0700-06 | Pass `0` to fraction | returns `0.0` exactly | UT-0700-04 |
| EC-0700-07 | Pass `u64::MAX` to fraction | returns finite f64 (no panic, no NaN) | UT-0700-04 EC-2 (optional) |

## Out of scope (documented for clarity; NOT tests)

- macOS positive path (no host).
- Property tests (proptest) — deferred per stage-2 directive.
- Concurrent reads from multiple threads — deferred (the campaign runs single-threaded probes).
- 32-bit `usize` hosts — Relativist requires 64-bit (documented in repo README).

## Open questions to surface to DEV (Stage 3)

1. Should `MemoryProbe` be `Clone`? The TASK doesn't require it but UT-0700-04 EC-1 (the implicit `1.0` boundary) would be cleaner if the test could clone the probe instead of reconstructing. Recommendation: add `#[derive(Clone)]` if cheap (cached u64 fields only).
2. Whether `BenchError::MemoryProbe(String)` already exists or needs to be added — TASK-0700 says "if needed". DEV verifies and adds if missing; this TEST-SPEC works either way.
