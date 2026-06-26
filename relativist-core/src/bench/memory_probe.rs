//! Cross-platform memory probe for the D-014 stress-curve campaign.
//!
//! Provides current + peak Resident Set Size (RSS) plus a fraction-of-total
//! denominator for the campaign's 80%-of-RAM gate (TASK-0700; consumed by
//! TASK-0701 `StopRule`).
//!
//! This module is **additive** to `bench/memory.rs`; the legacy `VmHWM`
//! reader stays in place for SPEC-09 R18 / R18a callers (which assume Linux
//! and a single `u64` return).
//!
//! Platforms in scope: Linux (≥ 3.10) and Windows 10+. On macOS every method
//! returns `BenchError::MemoryProbe("macos unsupported")` — the campaign
//! hosts are Linux + Windows only.

use crate::error::BenchError;

/// Per-process memory probe, opaque outside this module.
///
/// Caches the host's total physical RAM at construction time so the
/// fraction-of-total denominator is read exactly once.
#[derive(Debug, Clone)]
pub struct MemoryProbe {
    /// Total physical RAM in bytes (excluding swap on Linux).
    /// Cached at construction; never re-read.
    total_ram_bytes: u64,
}

impl MemoryProbe {
    /// Construct a new probe.
    ///
    /// On Linux reads `/proc/meminfo` for `MemTotal:`. On Windows calls
    /// `GlobalMemoryStatusEx` for `ullTotalPhys`. On macOS returns
    /// `Err(BenchError::MemoryProbe("macos unsupported"))`.
    pub fn new() -> Result<Self, BenchError> {
        #[cfg(target_os = "linux")]
        {
            let total = read_meminfo_total_bytes()?;
            Ok(Self {
                total_ram_bytes: total,
            })
        }
        #[cfg(target_os = "windows")]
        {
            let total = read_windows_total_phys_bytes()?;
            Ok(Self {
                total_ram_bytes: total,
            })
        }
        #[cfg(target_os = "macos")]
        {
            Err(BenchError::MemoryProbe("macos unsupported".to_string()))
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            Err(BenchError::MemoryProbe(format!(
                "unsupported target: {}",
                std::env::consts::OS
            )))
        }
    }

    /// Current RSS in bytes (`VmRSS` on Linux; `WorkingSetSize` on Windows).
    pub fn current_bytes(&self) -> Result<u64, BenchError> {
        #[cfg(target_os = "linux")]
        {
            read_status_field("VmRSS:")
        }
        #[cfg(target_os = "windows")]
        {
            read_windows_pmc().map(|pmc| pmc.0)
        }
        #[cfg(target_os = "macos")]
        {
            Err(BenchError::MemoryProbe("macos unsupported".to_string()))
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            Err(BenchError::MemoryProbe(format!(
                "unsupported target: {}",
                std::env::consts::OS
            )))
        }
    }

    /// Peak RSS in bytes since process start (`VmHWM` on Linux;
    /// `PeakWorkingSetSize` on Windows). Monotonic non-decreasing.
    pub fn peak_bytes(&self) -> Result<u64, BenchError> {
        #[cfg(target_os = "linux")]
        {
            read_status_field("VmHWM:")
        }
        #[cfg(target_os = "windows")]
        {
            read_windows_pmc().map(|pmc| pmc.1)
        }
        #[cfg(target_os = "macos")]
        {
            Err(BenchError::MemoryProbe("macos unsupported".to_string()))
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            Err(BenchError::MemoryProbe(format!(
                "unsupported target: {}",
                std::env::consts::OS
            )))
        }
    }

    /// Convert a byte count to a fraction of total physical RAM
    /// (excluding swap on Linux). Used by the `StopRule` 80% gate.
    ///
    /// The denominator is the cached `MemTotal` / `ullTotalPhys` from
    /// `new()`. Passing `bytes == 0` returns `0.0` exactly. Passing values
    /// larger than `total_ram_bytes` returns a finite f64 > 1.0 (no
    /// saturation; saturation is the caller's contract).
    pub fn as_fraction_of_total(&self, bytes: u64) -> f64 {
        if self.total_ram_bytes == 0 {
            // Defensive: avoids NaN if the host reports 0 total RAM. Real
            // Linux/Windows hosts never do, but keep the function total.
            return 0.0;
        }
        bytes as f64 / self.total_ram_bytes as f64
    }
}

// ---------------------------------------------------------------------------
// Linux backend
// ---------------------------------------------------------------------------

/// Read a numeric field from `/proc/self/status`. The field is the first
/// whitespace-separated number after the label; the value is reported in kB
/// (we multiply by 1024 to return bytes).
#[cfg(target_os = "linux")]
fn read_status_field(label: &str) -> Result<u64, BenchError> {
    let status = std::fs::read_to_string("/proc/self/status")
        .map_err(|e| BenchError::MemoryProbe(format!("read /proc/self/status: {}", e)))?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix(label) {
            for token in rest.split_whitespace() {
                if let Ok(kb) = token.parse::<u64>() {
                    return Ok(kb.saturating_mul(1024));
                }
            }
        }
    }
    Err(BenchError::MemoryProbe(format!(
        "field {} not found in /proc/self/status",
        label
    )))
}

/// Read `MemTotal` from `/proc/meminfo` and return bytes (excludes swap).
#[cfg(target_os = "linux")]
fn read_meminfo_total_bytes() -> Result<u64, BenchError> {
    let meminfo = std::fs::read_to_string("/proc/meminfo")
        .map_err(|e| BenchError::MemoryProbe(format!("read /proc/meminfo: {}", e)))?;
    for line in meminfo.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            for token in rest.split_whitespace() {
                if let Ok(kb) = token.parse::<u64>() {
                    return Ok(kb.saturating_mul(1024));
                }
            }
        }
    }
    Err(BenchError::MemoryProbe(
        "MemTotal not found in /proc/meminfo".to_string(),
    ))
}

// ---------------------------------------------------------------------------
// Windows backend
// ---------------------------------------------------------------------------

/// Read total physical RAM via `GlobalMemoryStatusEx` (`MEMORYSTATUSEX.ullTotalPhys`).
#[cfg(target_os = "windows")]
fn read_windows_total_phys_bytes() -> Result<u64, BenchError> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        dwMemoryLoad: 0,
        ullTotalPhys: 0,
        ullAvailPhys: 0,
        ullTotalPageFile: 0,
        ullAvailPageFile: 0,
        ullTotalVirtual: 0,
        ullAvailVirtual: 0,
        ullAvailExtendedVirtual: 0,
    };

    // SAFETY: `status` is a valid, fully-initialized `MEMORYSTATUSEX` with
    // `dwLength` set per the Win32 ABI. `GlobalMemoryStatusEx` only writes
    // into the struct; it does not retain the pointer.
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    if ok == 0 {
        return Err(BenchError::MemoryProbe(
            "GlobalMemoryStatusEx failed".to_string(),
        ));
    }
    Ok(status.ullTotalPhys)
}

/// Read `(WorkingSetSize, PeakWorkingSetSize)` for the current process.
#[cfg(target_os = "windows")]
fn read_windows_pmc() -> Result<(u64, u64), BenchError> {
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let mut pmc: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;

    // SAFETY: `GetCurrentProcess` returns a pseudo-handle valid for the
    // lifetime of the call; `pmc` is fully zeroed and `cb` set per the
    // Win32 ABI; `GetProcessMemoryInfo` only writes into the struct.
    let ok = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut pmc, pmc.cb) };
    if ok == 0 {
        return Err(BenchError::MemoryProbe(
            "GetProcessMemoryInfo failed".to_string(),
        ));
    }
    Ok((pmc.WorkingSetSize as u64, pmc.PeakWorkingSetSize as u64))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// UT-0700-01 — Probe construction succeeds on Linux + Windows;
    /// returns `BenchError::MemoryProbe("macos unsupported")` on macOS.
    #[test]
    fn probe_construction_succeeds_on_supported_platforms() {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            let probe =
                MemoryProbe::new().expect("probe construction must succeed on linux/windows");
            // EC-1: repeated construction must also succeed (no global state).
            let probe2 = MemoryProbe::new().expect("repeated probe construction must succeed");
            // Touch both probes so the bindings aren't optimized away in
            // release.
            let _ = (probe, probe2);
        }
        #[cfg(target_os = "macos")]
        {
            match MemoryProbe::new() {
                Err(BenchError::MemoryProbe(s)) => {
                    assert_eq!(s, "macos unsupported")
                }
                other => panic!(
                    "expected MemoryProbe(\"macos unsupported\"), got {:?}",
                    other
                ),
            }
        }
    }

    /// UT-0700-02 — `peak_bytes >= current_bytes` and both > 0 in a live
    /// process.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn peak_is_at_least_current() {
        let probe = MemoryProbe::new().expect("probe construction must succeed");
        let current = probe.current_bytes().expect("current_bytes");
        let peak = probe.peak_bytes().expect("peak_bytes");
        assert!(
            peak >= current,
            "peak ({}) must be >= current ({})",
            peak,
            current
        );
        assert!(current > 0, "current_bytes must be > 0 in a live process");
        assert!(peak > 0, "peak_bytes must be > 0 in a live process");
    }

    /// UT-0700-03 — Allocating ~100 MiB raises `current_bytes` by ≥ 50 MiB.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn allocation_increases_current_bytes() {
        let probe = MemoryProbe::new().expect("probe construction must succeed");
        let before_current = probe.current_bytes().expect("current_bytes pre");
        let frac_before = probe.as_fraction_of_total(before_current);

        // Self-skip on a memory-starved CI runner (≥ 90% RAM already used).
        if frac_before > 0.90 {
            eprintln!(
                "SKIP UT-0700-03: pre-alloc RSS fraction = {} > 0.90 (memory-starved CI)",
                frac_before
            );
            return;
        }

        const SIZE: usize = 100 * 1024 * 1024;
        let mut buf: Vec<u8> = vec![0u8; SIZE];
        // WRITE one byte per page (not read) to force the kernel to back every
        // page with a private resident page. On Linux, `vec![0u8; N]` is backed
        // by the shared zero page (copy-on-write); merely *reading* it never
        // raises RSS, so the probe would see no growth. Writing faults each page
        // in. (4096 = the common page size; smaller real pages are a superset.)
        for i in (0..SIZE).step_by(4096) {
            buf[i] = (i % 251 + 1) as u8;
        }
        let buf = std::hint::black_box(buf);

        let after_current = probe.current_bytes().expect("current_bytes post");
        let delta = after_current.saturating_sub(before_current);
        assert!(
            delta >= 50 * 1024 * 1024,
            "expected current_bytes to rise by >= 50 MiB after allocating 100 MiB; rose by {} bytes ({} MiB)",
            delta,
            delta / (1024 * 1024)
        );
        let peak_after = probe.peak_bytes().expect("peak_bytes post");
        assert!(
            peak_after >= after_current,
            "peak ({}) must be >= post-allocation current ({})",
            peak_after,
            after_current
        );

        drop(buf);
    }

    /// UT-0700-04 — `as_fraction_of_total` is a normalized fraction in
    /// `[0.0, 1.0]` and `fraction(0) == 0.0` exactly.
    #[test]
    fn fraction_of_total_in_unit_interval() {
        let probe = match MemoryProbe::new() {
            Ok(p) => p,
            Err(_) => return, // macOS / unsupported — nothing to assert
        };
        let current = probe.current_bytes().expect("current_bytes");
        let frac_current = probe.as_fraction_of_total(current);
        let frac_zero = probe.as_fraction_of_total(0);

        assert_eq!(frac_zero, 0.0, "fraction(0) must be exactly 0.0");
        assert!(
            frac_current > 0.0,
            "current process must consume some RAM, got fraction {}",
            frac_current
        );
        assert!(
            frac_current <= 1.0,
            "fraction must be <= 1.0, got {}",
            frac_current
        );
        // EC-2: u64::MAX must not panic and must be finite.
        let big = probe.as_fraction_of_total(u64::MAX);
        assert!(
            big.is_finite(),
            "as_fraction_of_total(u64::MAX) must be finite"
        );
    }
}
