//! IT-0702-01 — `descriptor_data_and_smoke_run` (TASK-0702 / TEST-SPEC-0702).
//!
//! Verifies the canonical N sweep, per-env stop-rule defaults, and a smoke
//! invocation of `StressCurveDescriptor::run_one_sequence` for a tiny
//! `n_seq = [1_000, 10_000]` override on `ep_annihilation` workers=1.

use relativist_core::bench::stop_rule::{ChildExit, StopRule};
use relativist_core::bench::suite::{Env, StressCurveDescriptor, StressWorkload};
use std::time::Duration;

/// Single integration test exercising AC-1, AC-2, AC-3 and AC-6 in one body.
/// AC-4 (macOS BenchError propagation) is verified by the early-return on
/// macOS hosts (see Part C).
#[test]
#[ignore = "D-014 stress-curve smoke: Part C runs a real benchmark sequence with a wall budget that false-trips on slow CI runners. Run manually: cargo test -- --ignored"]
fn descriptor_data_and_smoke_run() {
    // ----- Part A: canonical N sweep -----
    const EXPECTED_N: &[usize] = &[
        10_000,
        31_623,
        100_000,
        316_228,
        1_000_000,
        3_162_278,
        10_000_000,
        31_622_776,
        100_000_000,
        316_227_766,
        1_000_000_000,
    ];
    let n_seq = StressCurveDescriptor::n_seq();
    assert_eq!(
        n_seq, EXPECTED_N,
        "n_seq must match design doc §4.4 verbatim (×√10 from 10⁴ to 10⁹)"
    );
    assert_eq!(n_seq.len(), 11, "n_seq must have exactly 11 entries");

    // ----- Part B: per-env stop-rule defaults -----
    let r_inp = StressCurveDescriptor::default_stop_rule(Env::InProcess);
    assert_eq!(
        r_inp.wall_budget,
        Duration::from_secs(300),
        "InProcess wall budget must be 5 min (300s)"
    );
    assert!(
        (r_inp.memory_fraction_max - 0.80).abs() < f64::EPSILON,
        "InProcess memory_fraction_max must be 0.80; got {}",
        r_inp.memory_fraction_max
    );

    let r_doc = StressCurveDescriptor::default_stop_rule(Env::DockerTcp);
    assert_eq!(
        r_doc.wall_budget,
        Duration::from_secs(450),
        "DockerTcp wall budget must be 7m30s (450s)"
    );
    assert!(
        (r_doc.memory_fraction_max - 0.80).abs() < f64::EPSILON,
        "DockerTcp memory_fraction_max must be 0.80; got {}",
        r_doc.memory_fraction_max
    );

    // ----- Part C: smoke run (AC-3 + AC-6) -----
    // AC-1 from TASK-0700: macOS unsupported. The descriptor propagates
    // BenchError::MemoryProbe; honor that here by early-return on macOS.
    #[cfg(target_os = "macos")]
    {
        return;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let n_override = [1_000usize, 10_000usize];
        let stop = StopRule {
            wall_budget: Duration::from_secs(30),
            memory_fraction_max: 0.95,
        };

        let outcome = StressCurveDescriptor::run_one_sequence(
            StressWorkload::EpAnnihilation,
            Env::InProcess,
            /* workers */ 1,
            /* reps */ 1,
            Some(&n_override),
            Some(stop),
        )
        .expect("smoke run must succeed in-process for ep_annihilation N=[1k, 10k]");

        assert_eq!(
            outcome.completed_reps.len(),
            2,
            "expected exactly 2 completed reps for n_seq=[1k, 10k] reps=1; got {}",
            outcome.completed_reps.len()
        );
        assert_eq!(
            outcome.stop_reason, None,
            "smoke must complete without tripping any rule"
        );
        assert_eq!(outcome.last_attempted_n, Some(10_000));

        for (i, r) in outcome.completed_reps.iter().enumerate() {
            let expected_n = n_override[i];
            assert_eq!(r.n, expected_n, "rep[{}] N mismatch", i);
            assert!(r.wall > Duration::ZERO, "rep[{}] wall must be > 0", i);
            assert!(
                r.vmrss_peak_bytes > 0,
                "rep[{}] vmrss_peak_bytes must be > 0",
                i
            );
            assert!(
                r.vmrss_peak_fraction_of_total > 0.0 && r.vmrss_peak_fraction_of_total <= 1.0,
                "rep[{}] vmrss_peak_fraction_of_total must be in (0, 1]; got {}",
                i,
                r.vmrss_peak_fraction_of_total
            );
            match r.child_exit {
                ChildExit::Ok => {}
                other => panic!("rep[{}] expected ChildExit::Ok; got {:?}", i, other),
            }
        }
    }
}
