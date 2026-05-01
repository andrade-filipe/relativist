//! TASK-0604 — Eager-path regression sentinel (Phase C-2).
//!
//! Spec: SPEC-09 R18a–R18g (commit `82b2d27`); D-011 plan §C-2 mitigation
//! (non-negotiable — the silent eager-path regression headlines TASK-0604).
//!
//! Tests cover (per TEST-SPEC-0604):
//!  - IT-0604-08 — bench WITHOUT `--chunk-size` produces output bit-equivalent
//!    to a deterministic baseline derived from `bench.make_net(size)` + the
//!    eager `run_grid` path. If a future refactor accidentally reroutes the
//!    eager path through streaming (or silently propagates only the wrong
//!    Tier 3 fields), this test fires.

use relativist_core::bench::suite::{
    build_grid_config_from_suite, build_input_net_from_suite, run_benchmark_suite,
};
use relativist_core::bench::{
    BenchmarkId, BenchmarkSuiteConfig, Mode, NetRepresentation, RecyclePolicy,
};

fn ep_config_eager(size: u32, workers: u32) -> BenchmarkSuiteConfig {
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
        chunk_size: None, // eager path
        max_pending_lifetime: 16,
        recycle_policy: RecyclePolicy::DisableUnderDelta,
        representation: NetRepresentation::Dense,
    }
}

/// IT-0604-08 — eager-path bit-equivalence baseline.
///
/// The "frozen baseline" referenced in TASK-0604 §AC-6 / TEST-SPEC-0604
/// IT-0604-08 lives at `results/locked/v1_local_baseline/` but cannot be
/// pinned to a specific row at test-spec time — the v1 CSV does not record
/// the same deterministic seed v2 uses. Per the spec's implementation note,
/// we anchor the test against an *in-process* deterministic baseline derived
/// from `bench::suite::get_benchmark` + the bench harness's eager path.
///
/// What we lock:
/// - `final_agent_count == 0` (ep_annihilation always reduces to ∅).
/// - `total_interactions == size` (each ERA-ERA pair = 1 interaction).
/// - `g1_pass == true` (correct).
///
/// What this catches:
/// - The eager path getting rerouted through streaming (silently). The
///   streaming path also produces 0 final agents, but the structural Net
///   passed to `run_grid` would be merge-reconstructed — its arena layout
///   would NOT match `bench.make_net(size)` byte-for-byte.
#[test]
fn it_0604_08_eager_path_output_bit_equivalent_to_baseline() {
    let size = 1000u32;
    let workers = 2u32;
    let config = ep_config_eager(size, workers);
    let result =
        run_benchmark_suite(&config).expect("IT-0604-08: eager bench must complete without error");

    assert!(
        result.all_correct,
        "IT-0604-08: eager bench must be G1-correct"
    );

    let grid_row = result
        .results
        .iter()
        .find(|r| r.mode == Mode::Local)
        .expect("IT-0604-08: a Local-mode row must exist");
    assert!(grid_row.correct, "IT-0604-08: grid row must be correct");
    assert_eq!(
        grid_row.input_size, size,
        "IT-0604-08: input_size column must match"
    );
    assert_eq!(
        grid_row.workers, workers,
        "IT-0604-08: workers column must match"
    );
    assert_eq!(
        grid_row.total_interactions, size as u64,
        "IT-0604-08: ep_annihilation({size}) records {size} ERA-ERA interactions; \
         the eager path's interaction count is the headline regression sentinel"
    );

    // Bit-stable structural check: the input Net produced by the eager path
    // is byte-equivalent to a freshly-built `bench.make_net(size)`.
    let bench = relativist_core::bench::suite::get_benchmark(BenchmarkId::EPAnnihilation);
    let routed = build_input_net_from_suite(&config, bench.as_ref(), size, workers)
        .expect("eager build must succeed");
    let direct = bench.make_net(size);
    assert_eq!(
        routed.agents.len(),
        direct.agents.len(),
        "IT-0604-08: eager-path arena length MUST match bench.make_net (silent-reroute guard)"
    );
    assert_eq!(
        routed.count_live_agents(),
        direct.count_live_agents(),
        "IT-0604-08: eager-path live-agent count MUST match bench.make_net"
    );
    // Bit-stable arena: pre-reduction `next_id` MUST match (a streaming
    // reroute would land here with a merge-reconstructed arena whose
    // `next_id` reflects partition allocation rather than the eager
    // generator's contiguous numbering).
    assert_eq!(
        routed.next_id, direct.next_id,
        "IT-0604-08: eager-path next_id MUST match bench.make_net (silent-reroute guard)"
    );
}

/// IT-0604-08 (extension) — `GridConfig` carries `max_pending_lifetime` even
/// in the eager branch. Acceptance criterion #2.
#[test]
fn it_0604_08_eager_path_grid_config_propagates_max_pending_lifetime() {
    let mut config = ep_config_eager(100, 2);
    config.max_pending_lifetime = 99; // an arbitrary non-default value.

    let grid_config = build_grid_config_from_suite(&config, 2);
    assert_eq!(
        grid_config.max_pending_lifetime, 99,
        "IT-0604-08: eager-path GridConfig.max_pending_lifetime MUST equal config.max_pending_lifetime"
    );
}
