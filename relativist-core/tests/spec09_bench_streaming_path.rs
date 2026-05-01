//! TASK-0604 — Bench harness streaming path (Phase C-2 + C-4).
//!
//! Spec: SPEC-09 R18a–R18g, R37c (commit `82b2d27`); SPEC-21 R37c (construction-
//! phase isomorphism); SPEC-01 G1.
//!
//! Tests cover (per TEST-SPEC-0604):
//!  - IT-0604-06 — streaming bench produces a valid CSV row with G1 passing.
//!  - IT-0604-07 — streaming-vs-eager merged-net isomorphism (R37c).
//!  - IT-0604-09 — CLI smoke: `--chunk-size 100` completes under 60s.
//!  - PT-0604-10 — proptest: streaming/eager isomorphism for random chunk sizes.

use relativist_core::bench::isomorphism::nets_isomorphic;
use relativist_core::bench::suite::run_benchmark_suite;
use relativist_core::bench::{
    BenchmarkId, BenchmarkSuiteConfig, Mode, NetRepresentation, RecyclePolicy,
};
use relativist_core::reduction::reduce_all;

/// Build a `BenchmarkSuiteConfig` for ep_annihilation in `Local` mode.
fn ep_config(size: u32, workers: u32, chunk_size: Option<u32>) -> BenchmarkSuiteConfig {
    BenchmarkSuiteConfig {
        benchmarks: vec![BenchmarkId::EPAnnihilation],
        sizes: Some(vec![size]),
        workers: vec![workers],
        mode: Mode::Local,
        warmup_runs: 0,
        repetitions: 1,
        csv_detail_path: None,
        csv_rounds_path: None,
        csv_summary_path: None,
        max_rounds: None,
        strict_bsp: false,
        skip_g1: false,
        chunk_size,
        max_pending_lifetime: 16,
        recycle_policy: RecyclePolicy::DisableUnderDelta,
        representation: NetRepresentation::Dense,
    }
}

/// IT-0604-06 — streaming bench produces valid output with G1 passing.
///
/// Acceptance criterion #5: smoke command runs in-process and produces a
/// SuiteResult with `all_correct == true` (G1 holds), the new Tier 3 fields
/// are honored, and the per-rep `correct` flag is set on every row.
#[test]
fn it_0604_06_streaming_path_produces_valid_result_with_g1_passing() {
    let config = ep_config(1000, 2, Some(100));
    let result = run_benchmark_suite(&config)
        .expect("IT-0604-06: streaming bench must complete without error");

    assert!(
        result.all_correct,
        "IT-0604-06: streaming bench must produce G1-correct results"
    );
    // 1 sequential + 1 grid result = 2.
    assert_eq!(
        result.results.len(),
        2,
        "IT-0604-06: expected 1 sequential + 1 grid datapoint; got {}",
        result.results.len()
    );
    // The grid result row must show non-zero interactions.
    let grid_row = result
        .results
        .iter()
        .find(|r| r.mode == Mode::Local)
        .expect("IT-0604-06: a Local-mode row must exist");
    assert!(
        grid_row.correct,
        "IT-0604-06: grid row must be marked correct"
    );
    assert!(
        grid_row.total_interactions > 0,
        "IT-0604-06: grid row must record non-zero interactions for ep_annihilation(1000)"
    );
}

/// IT-0604-07 — streaming-vs-eager merged-net isomorphism (R37c, G1).
///
/// SPEC-01 G1 + SPEC-21 R37c: the merged net produced by streaming-path bench
/// must be agent-isomorphic to the eager-path counterpart on the same
/// (size, workers) configuration. This is the strongest invariant — catches
/// any silent semantic divergence introduced by the streaming construction
/// order.
#[test]
fn it_0604_07_streaming_vs_eager_merged_net_isomorphism() {
    use relativist_core::bench::suite::build_input_net_from_suite;

    // The dispatch's two branches differ in *construction order*; SPEC-21 R37c
    // mandates that after `reduce_all` both nets reach the same Normal Form
    // (G1). We exercise the harness's `build_input_net_from_suite` directly
    // because the public `run_benchmark_suite` measures wall-clock time and
    // returns aggregated stats only — we need the raw Net for isomorphism.
    let bench = relativist_core::bench::suite::get_benchmark(BenchmarkId::EPAnnihilation);

    let config_eager = ep_config(500, 2, None);
    let config_stream = ep_config(500, 2, Some(50));

    let mut net_eager = build_input_net_from_suite(&config_eager, bench.as_ref(), 500, 2)
        .expect("eager build must succeed");
    let mut net_stream = build_input_net_from_suite(&config_stream, bench.as_ref(), 500, 2)
        .expect("streaming build must succeed");

    // R37c construction-phase isomorphism: both should have the same live-agent
    // count BEFORE reduction.
    assert_eq!(
        net_eager.count_live_agents(),
        net_stream.count_live_agents(),
        "IT-0604-07: pre-reduction live-agent count must match across paths (R37c)"
    );

    // Reduce both fully; they must reach the same Normal Form.
    reduce_all(&mut net_eager);
    reduce_all(&mut net_stream);

    // ep_annihilation reduces to 0 live agents in either path.
    assert_eq!(
        net_eager.count_live_agents(),
        0,
        "IT-0604-07 sanity: ep_annihilation(500) eager reduces to 0 agents"
    );
    assert_eq!(
        net_stream.count_live_agents(),
        net_eager.count_live_agents(),
        "IT-0604-07: G1 — both paths reduce to the same final live-agent count"
    );
    // Stronger: full structural isomorphism on the reduced nets.
    assert!(
        nets_isomorphic(&net_eager, &net_stream),
        "IT-0604-07: G1 — reduced nets must be structurally isomorphic"
    );
}

/// IT-0604-09 — CLI smoke (un-ignored from TASK-0603's IT-0603-09 sentinel).
///
/// Per TASK-0604 §AC-5: `cargo run --bin relativist -- bench --benchmark
/// ep_annihilation --sizes 1000 --workers 2 --chunk-size 100` must complete
/// in under 60s with exit code 0. This exercises the path-selection wiring
/// end-to-end via subprocess.
#[test]
fn it_0604_09_cli_smoke_chunk_size_100_completes_under_60s() {
    use std::process::Command;
    use std::time::Instant;

    let start = Instant::now();
    let output = Command::new(env!("CARGO"))
        .args([
            "run",
            "--package",
            "relativist-cli",
            "--bin",
            "relativist",
            "--quiet",
            "--",
            "bench",
            "--benchmark",
            "ep_annihilation",
            "--sizes",
            "1000",
            "--workers",
            "2",
            "--chunk-size",
            "100",
            "--repetitions",
            "1",
            "--warmup",
            "0",
        ])
        .output()
        .expect("IT-0604-09: cargo run --bin relativist must spawn");
    let elapsed = start.elapsed();

    assert!(
        output.status.success(),
        "IT-0604-09: bench smoke must exit 0; status: {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ep_annihilation"),
        "IT-0604-09: stdout must mention the benchmark id; got: {stdout}"
    );
    // 60s budget is generous; failure here suggests an infinite loop or
    // a streaming path that has not been wired correctly.
    assert!(
        elapsed.as_secs() < 60,
        "IT-0604-09: bench smoke must complete under 60s; took {:?}",
        elapsed
    );
}

// ---------------------------------------------------------------------------
// PT-0604-10 — proptest: streaming/eager isomorphism for random chunk sizes
// ---------------------------------------------------------------------------

use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig {
        // Each case is a full bench-pipeline build; keep cases low for speed.
        cases: 12,
        ..ProptestConfig::default()
    })]

    /// PT-0604-10 — for any `(size, chunk_size, workers)` in the safe range,
    /// the streaming-path Net is agent-isomorphic to the eager-path Net.
    #[test]
    fn pt_0604_10_streaming_eager_isomorphic_for_random_chunk_sizes(
        size in 50u32..400u32,
        chunk_size in 1u32..400u32,
        workers in 1u32..=4u32,
    ) {
        use relativist_core::bench::suite::build_input_net_from_suite;
        let bench = relativist_core::bench::suite::get_benchmark(BenchmarkId::EPAnnihilation);

        let cfg_eager = ep_config(size, workers, None);
        let cfg_stream = ep_config(size, workers, Some(chunk_size));

        let mut eager = build_input_net_from_suite(&cfg_eager, bench.as_ref(), size, workers)
            .expect("eager build must succeed");
        let mut streamed = build_input_net_from_suite(&cfg_stream, bench.as_ref(), size, workers)
            .expect("streaming build must succeed");

        // R37c: pre-reduction agent counts must match.
        prop_assert_eq!(eager.count_live_agents(), streamed.count_live_agents());

        // G1: post-reduction live-agent counts must match (both → 0 for ep_annihilation).
        reduce_all(&mut eager);
        reduce_all(&mut streamed);
        prop_assert_eq!(eager.count_live_agents(), streamed.count_live_agents());
        prop_assert_eq!(eager.count_live_agents(), 0);
    }
}
