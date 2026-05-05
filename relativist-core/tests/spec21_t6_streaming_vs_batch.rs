//! SPEC-21 T6 — Streaming-vs-batch equivalence oracle (TASK-0567).
//!
//! For each benchmark in {ep_annihilation, dual_tree}, runs both the batch
//! path (SPEC-04 `split()`) and the streaming path
//! (`generate_and_partition_chunked`), and asserts that the fully-reduced
//! Normal Form nets are isomorphic per `nets_match_counts` (SPEC-21 R26 /
//! SPEC-00 §6.12).
//!
//! **BSP correctness note**: The full BSP cycle is:
//!   split → reduce_each_partition → merge → reduce_border_redexes → repeat.
//! Both the batch and streaming paths must follow this cycle.  Tests here use
//! `run_grid` for the batch path (which implements the full BSP loop) and
//! explicitly reduce each partition before merging for the streaming path.
//!
//! **Isomorphism, not byte-equality** (per SPEC-21 R26 closure of SC-014).
//! The streaming and batch paths may produce different agent-ID layouts;
//! correctness requires only that the merged nets are structurally isomorphic
//! Normal Forms (same agent-type counts).
//!
//! SPEC-21 §7.2 T6; TASK-0567.

use relativist_core::bench::isomorphism::nets_match_counts;
use relativist_core::bench::streaming::{dual_tree_stream, ep_annihilation_stream};
use relativist_core::io::generators;
use relativist_core::merge::core::merge;
use relativist_core::merge::run_grid;
use relativist_core::merge::types::GridConfig;
use relativist_core::partition::strategy::ContiguousIdStrategy;
use relativist_core::partition::streaming::{
    generate_and_partition_chunked, RoundRobinStreamingStrategy,
};
use relativist_core::reduction::engine::reduce_all;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Run the SPEC-04 batch path on `net` using `run_grid` (full BSP cycle).
///
/// `run_grid` implements split → reduce_each_partition → merge → border_reduce,
/// repeated until Normal Form. This is the correct reference for equivalence.
fn batch_path_result(
    net: relativist_core::net::Net,
    num_workers: u32,
) -> relativist_core::net::Net {
    let config = GridConfig {
        num_workers,
        max_rounds: None,
        ..GridConfig::default()
    };
    let strategy = ContiguousIdStrategy;
    let (result, _metrics) = run_grid(net, &config, &strategy);
    result
}

/// Run the streaming path: generate_and_partition_chunked → BSP reduce step
/// → merge → full convergence via run_grid.
///
/// This mirrors the BSP cycle that `run_grid` performs on the batch path:
/// 1. Each partition is reduced before merge (BSP local-reduce phase).
/// 2. After merge, `run_grid` is applied to the merged net to drive it to
///    full convergence. This handles cases where a single BSP cycle is
///    insufficient (e.g., `dual_tree` which requires multiple rounds to fully
///    reduce via BSP due to FreePort-to-FreePort redirects in partitions).
///
/// The isomorphism claim is: after full convergence, the streaming and batch
/// paths produce the same Normal Form up to agent-ID renaming.
fn streaming_path_result(
    stream: Box<dyn Iterator<Item = relativist_core::partition::streaming::AgentBatch>>,
    num_workers: u32,
) -> relativist_core::net::Net {
    use relativist_core::merge::helpers::rebuild_free_port_index;
    use relativist_core::partition::strategy::ContiguousIdStrategy;

    let mut strategy = RoundRobinStreamingStrategy::new(num_workers);
    let result = generate_and_partition_chunked(stream, num_workers, &mut strategy)
        .expect("streaming must succeed");

    let plan: relativist_core::partition::PartitionPlan = result.into();

    // BSP reduce step: reduce each partition locally before merge, then rebuild
    // free_port_index (mirrors run_grid lines 95-113).
    let mut plan = plan;
    for partition in &mut plan.partitions {
        reduce_all(&mut partition.subnet);
        partition.free_port_index = rebuild_free_port_index(
            &partition.subnet,
            partition.border_id_start,
            partition.border_id_end,
        );
    }

    let (merged, _borders) = merge(plan);

    // Drive to full convergence: `run_grid` handles multi-round BSP reduction.
    // Some reductions (e.g., dual_tree cascading CON-CON chains that pass through
    // multiple partition boundaries) require more than one round.
    let config = GridConfig {
        num_workers,
        max_rounds: None,
        ..GridConfig::default()
    };
    let strategy = ContiguousIdStrategy;
    let (result, _metrics) = run_grid(merged, &config, &strategy);
    result
}

// ---------------------------------------------------------------------------
// T6-01: ep_annihilation streaming ≈ batch (isomorphism)
// ---------------------------------------------------------------------------

/// T6-01: 32 ERA-ERA pairs, 4 workers, chunk_size = 8.
/// Streaming result must be isomorphic to batch result.
/// Isomorphism, not byte-equality per SPEC-21 R26 / SC-014.
#[test]
fn t6_01_ep_annihilation_streaming_isomorphic_to_batch() {
    let size = 32u32;
    let num_workers = 4u32;
    let chunk_size = 8;

    let batch_net = generators::ep_annihilation(size);
    let batch_result = batch_path_result(batch_net, num_workers);

    let stream = ep_annihilation_stream(size, chunk_size);
    let streaming_result = streaming_path_result(stream, num_workers);

    assert!(
        nets_match_counts(&batch_result, &streaming_result),
        "T6-01: ep_annihilation(32) streaming result must have same agent counts as batch result"
    );
}

// ---------------------------------------------------------------------------
// T6-02: dual_tree streaming ≈ batch (isomorphism)
// ---------------------------------------------------------------------------

/// T6-02: dual_tree depth=4 (30 nodes), 3 workers, chunk_size = 5.
/// Streaming result must be isomorphic to batch result.
#[test]
fn t6_02_dual_tree_streaming_isomorphic_to_batch() {
    let depth = 4u32;
    let num_workers = 3u32;
    let chunk_size = 5;

    let batch_net = generators::dual_tree(depth);
    let batch_result = batch_path_result(batch_net, num_workers);

    let stream = dual_tree_stream(depth, chunk_size);
    let streaming_result = streaming_path_result(stream, num_workers);

    assert!(
        nets_match_counts(&batch_result, &streaming_result),
        "T6-02: dual_tree(depth=4) streaming result must have same agent counts as batch result"
    );
}

// ---------------------------------------------------------------------------
// T6-03: R26 short-circuit path ≈ batch (isomorphism)
// ---------------------------------------------------------------------------

/// T6-03: with chunk_size == u32::MAX, the pipeline MUST take the materialise-then-split
/// path (R26). Result must be isomorphic to the normal batch path.
/// This test verifies R26 is exercised (instrumented via the sentinel value).
#[test]
fn t6_03_r26_short_circuit_path_isomorphic_to_batch() {
    use relativist_core::merge::helpers::rebuild_free_port_index;
    use relativist_core::partition::strategy::ContiguousIdStrategy;
    use relativist_core::partition::streaming::{
        generate_and_partition_chunked_with_chunk_size, CHUNK_SIZE_MAX_SENTINEL,
    };

    let size = 16u32;
    let num_workers = 2u32;

    // Batch path (via run_grid)
    let batch_net = generators::ep_annihilation(size);
    let batch_result = batch_path_result(batch_net, num_workers);

    // R26 path: chunk_size = u32::MAX → materialise-then-split
    let stream = ep_annihilation_stream(size, 1024);
    let mut strategy = RoundRobinStreamingStrategy::new(num_workers);
    let result = generate_and_partition_chunked_with_chunk_size(
        stream,
        num_workers,
        &mut strategy,
        CHUNK_SIZE_MAX_SENTINEL,
    )
    .expect("R26 short-circuit must succeed");

    assert_eq!(
        result.stats.chunks_processed, 1,
        "R26: short-circuit path must report exactly 1 chunk (the whole net as a single batch)"
    );

    let plan: relativist_core::partition::PartitionPlan = result.into();

    // BSP reduce step then full convergence via run_grid.
    let mut plan = plan;
    for partition in &mut plan.partitions {
        reduce_all(&mut partition.subnet);
        partition.free_port_index = rebuild_free_port_index(
            &partition.subnet,
            partition.border_id_start,
            partition.border_id_end,
        );
    }

    let (r26_merged, _) = merge(plan);
    let r26_config = GridConfig {
        num_workers,
        max_rounds: None,
        ..GridConfig::default()
    };
    let (r26_result, _) = run_grid(r26_merged, &r26_config, &ContiguousIdStrategy);

    assert!(
        nets_match_counts(&batch_result, &r26_result),
        "T6-03: R26 short-circuit result must be isomorphic to normal batch result"
    );
}

// ---------------------------------------------------------------------------
// T6-04: single-worker streaming ≈ batch (degenerate case)
// ---------------------------------------------------------------------------

/// T6-04: 1 worker — all agents go to worker 0. Streaming ≈ batch.
#[test]
fn t6_04_single_worker_streaming_isomorphic_to_batch() {
    let size = 8u32;
    let num_workers = 1u32;
    let chunk_size = 3;

    let batch_net = generators::ep_annihilation(size);
    let batch_result = batch_path_result(batch_net, num_workers);

    let stream = ep_annihilation_stream(size, chunk_size);
    let streaming_result = streaming_path_result(stream, num_workers);

    assert!(
        nets_match_counts(&batch_result, &streaming_result),
        "T6-04: 1-worker streaming result must match batch result"
    );
}
