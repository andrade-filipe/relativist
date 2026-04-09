//! Peak memory measurement (SPEC-09 R21, Section 4.4).

/// Obtain the peak memory usage (resident set size) of the current process.
///
/// On Linux: reads `/proc/self/status`, parses VmHWM (peak RSS in kB),
/// and converts to bytes.
/// On other OSes: returns 0 (metric unavailable).
pub fn get_peak_memory_bytes() -> u64 {
    #[cfg(target_os = "linux")]
    {
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

    #[cfg(not(target_os = "linux"))]
    {
        0
    }
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
}
