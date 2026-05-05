//! Peak memory measurement (SPEC-09 R21, Section 4.4; R18a § Tier 3 D-011).
//!
//! MF-004 (D-011 review): the original API had two byte-identical functions
//! (`get_peak_memory_bytes` and `get_peak_memory_during_construction`) — only
//! their doc-comments documented different call-site semantics. The DRY
//! violation has been collapsed: both functions are now thin wrappers around
//! a single private `sample_vmhwm` reader, preserving the spec-mandated
//! API surface (R18 / R18a both name-resolve) while keeping a single source
//! of truth for the actual VmHWM probe logic.

/// Obtain the peak memory usage (resident set size) of the current process.
///
/// On Linux: reads `/proc/self/status`, parses VmHWM (peak RSS in kB),
/// and converts to bytes.
/// On other OSes: returns 0 (metric unavailable).
///
/// SPEC-09 R18 (legacy): sampled AT END-OF-RUN by the bench harness.
pub fn get_peak_memory_bytes() -> u64 {
    sample_vmhwm()
}

/// SPEC-09 R18a — sample `VmHWM` at the construction-complete program point.
///
/// Per SPEC-09 §4.9 R18a (commit `82b2d27`), the framework MUST capture this
/// value at a single, well-defined program point:
/// - **Eager path** (`chunk_size == None`): AFTER `bench.make_net(size)`
///   returns AND BEFORE any `reduce_all` / `run_grid` invocation.
/// - **Streaming path** (`chunk_size == Some(N)`): AFTER the chunked partition
///   pipeline returns AND BEFORE the first `AssignPartition` is dispatched.
/// - **Sparse path** (`representation == Sparse`): AFTER `to_dense(id_range)`
///   returns AND BEFORE any `reduce_all` invocation.
///
/// This function shares the underlying VmHWM reader with
/// [`get_peak_memory_bytes`] (the legacy R18 metric); the two are
/// distinguished ONLY by their CALL SITE in `bench/suite.rs`. On non-Linux
/// targets returns `0` (matches `get_peak_memory_bytes` convention). Both
/// rely on `VmHWM` being monotonic non-decreasing, so the construction-time
/// snapshot is a valid lower bound for the eventual end-of-run reading.
pub fn get_peak_memory_during_construction() -> u64 {
    sample_vmhwm()
}

/// MF-004 (D-011 review): the single VmHWM probe. Linux-only; returns 0
/// elsewhere. Kept private — callers should use the named wrappers above so
/// the call-site discipline is documented at the type-system level (the
/// only public surface is the two named functions).
fn sample_vmhwm() -> u64 {
    #[cfg(target_os = "linux")]
    {
        read_vmhwm_bytes()
    }

    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}

#[cfg(target_os = "linux")]
fn read_vmhwm_bytes() -> u64 {
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("VmHWM:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<u64>() {
                        return kb * 1024;
                    }
                }
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_peak_memory_does_not_panic() {
        let _bytes = get_peak_memory_bytes();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_get_peak_memory_nonzero_on_linux() {
        assert!(get_peak_memory_bytes() > 0);
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn test_get_peak_memory_zero_on_non_linux() {
        assert_eq!(get_peak_memory_bytes(), 0);
    }

    // -----------------------------------------------------------------------
    // TASK-0605 — get_peak_memory_during_construction probe (SPEC-09 R18a)
    //
    // Per TEST-SPEC-0605:
    //   UT-0605-01 — Linux: returns non-zero VmHWM (>= 1024 bytes)
    //   UT-0605-02 — non-Linux: returns 0
    //   UT-0605-03 — does not panic on any OS
    // -----------------------------------------------------------------------

    /// UT-0605-03 — Cross-platform: function is callable without panic.
    /// Mirrors the existing `test_get_peak_memory_does_not_panic` discipline.
    #[test]
    fn ut_0605_03_probe_does_not_panic_any_os() {
        let _v = get_peak_memory_during_construction();
    }

    /// UT-0605-01 — Linux: reads `/proc/self/status` VmHWM and returns a
    /// non-zero value (the test process is at least a few MiB resident).
    #[cfg(target_os = "linux")]
    #[test]
    fn ut_0605_01_probe_returns_nonzero_on_linux() {
        let v = get_peak_memory_during_construction();
        assert!(
            v > 0,
            "UT-0605-01: probe must return non-zero on Linux; got {v}"
        );
        assert!(
            v >= 1024,
            "UT-0605-01: VmHWM is reported in kB; the test process is at least 1 page; got {v}"
        );
    }

    /// UT-0605-02 — Non-Linux (Windows, macOS): probe returns 0 (matches
    /// existing convention with `get_peak_memory_bytes`).
    #[cfg(not(target_os = "linux"))]
    #[test]
    fn ut_0605_02_probe_returns_zero_on_non_linux() {
        let v = get_peak_memory_during_construction();
        assert_eq!(
            v, 0,
            "UT-0605-02: probe must return 0 on non-Linux targets; got {v}"
        );
    }
}
