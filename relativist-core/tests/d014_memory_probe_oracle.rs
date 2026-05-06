//! IT-0707-01 (a) — `memory_probe_vs_oracle_100mib` (TASK-0707).
//!
//! Allocate exactly 100 MiB, verify probe reports `current_bytes` rises by
//! ≥ 80 MiB and `peak_bytes` ≥ 100 MiB. Self-skips on memory-starved CI
//! (frac > 0.90) per AC-3.

#![cfg(not(target_os = "macos"))]

use relativist_core::bench::memory_probe::MemoryProbe;

#[test]
fn memory_probe_vs_oracle_100mib() {
    let probe = match MemoryProbe::new() {
        Ok(p) => p,
        Err(e) => panic!("MemoryProbe::new must succeed on linux/windows: {:?}", e),
    };

    let cur0 = probe.current_bytes().expect("current_bytes pre-alloc");
    let frac0 = probe.as_fraction_of_total(cur0);
    if frac0 > 0.90 {
        eprintln!(
            "SKIP IT-0707-01: pre-alloc RSS fraction = {} > 0.90; CI is memory-starved",
            frac0
        );
        return;
    }

    const SIZE: usize = 100 * 1024 * 1024;
    let buf: Vec<u8> = vec![0u8; SIZE];
    // Force commit by touching every 4 KiB page.
    let mut sum: u64 = 0;
    for chunk in buf.chunks(4096) {
        sum = sum.wrapping_add(chunk[0] as u64);
    }
    let buf = std::hint::black_box(buf);
    let _sum = std::hint::black_box(sum);

    let cur1 = probe.current_bytes().expect("current_bytes post-alloc");
    let peak1 = probe.peak_bytes().expect("peak_bytes post-alloc");
    let delta = cur1.saturating_sub(cur0);

    assert!(
        delta >= 80 * 1024 * 1024,
        "expected current_bytes to rise by >= 80 MiB; rose by {} bytes ({} MiB)",
        delta,
        delta / (1024 * 1024)
    );
    assert!(
        peak1 >= 100 * 1024 * 1024,
        "expected peak_bytes >= 100 MiB; got {} bytes ({} MiB)",
        peak1,
        peak1 / (1024 * 1024)
    );

    drop(buf);
}
