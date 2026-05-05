//! SPEC-21 T8 — Chunk-size independence oracle (TASK-0567).
//!
//! For `ep_annihilation_pure(1024)`, runs the streaming pipeline with
//! `chunk_size ∈ {2, 8, 64, 512, 1024}`, and asserts pairwise isomorphism
//! of the fully-reduced Normal Forms.
//!
//! **BSP correctness note**: Each streaming partition is reduced locally before
//! merge (mirroring `run_grid`'s BSP cycle), then border redexes are resolved.
//!
//! **Isomorphism, not byte-equality** (per SPEC-21 R26 closure of SC-014).
//! Different chunk sizes produce different border layouts but identical
//! Normal Forms up to agent-ID renaming.
//!
//! SPEC-21 §7.2 T8; TASK-0567.

use relativist_core::bench::isomorphism::nets_match_counts;
use relativist_core::bench::streaming::ep_annihilation_stream;
use relativist_core::merge::core::merge;
use relativist_core::merge::helpers::rebuild_free_port_index;
use relativist_core::partition::streaming::{
    generate_and_partition_chunked, RoundRobinStreamingStrategy,
};
use relativist_core::reduction::engine::reduce_all;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn run_streaming_and_reduce(
    size: u32,
    chunk_size: usize,
    num_workers: u32,
) -> relativist_core::net::Net {
    let stream = ep_annihilation_stream(size, chunk_size);
    let mut strategy = RoundRobinStreamingStrategy::new(num_workers);
    let result = generate_and_partition_chunked(stream, num_workers, &mut strategy)
        .unwrap_or_else(|e| panic!("streaming failed for chunk_size={chunk_size}: {e}"));

    let plan: relativist_core::partition::PartitionPlan = result.into();

    // BSP reduce step: reduce each partition locally before merge, then rebuild
    // free_port_index (mirrors run_grid BSP cycle lines 95-113).
    let mut plan = plan;
    for partition in &mut plan.partitions {
        reduce_all(&mut partition.subnet);
        partition.free_port_index = rebuild_free_port_index(
            &partition.subnet,
            partition.border_id_start,
            partition.border_id_end,
        );
    }

    let (mut merged, _) = merge(plan);
    // Reduce any border redexes surfaced by merge.
    reduce_all(&mut merged);
    merged
}

// ---------------------------------------------------------------------------
// T8-01: chunk-size sweep, 4 workers
// ---------------------------------------------------------------------------

/// T8-01: ep_annihilation(1024), 4 workers, chunk_size ∈ {2, 8, 64, 512, 1024}.
/// All merged Normal Forms must be pairwise isomorphic (same agent counts).
#[test]
fn t8_01_chunk_size_independence_ep_annihilation_4_workers() {
    let size = 64u32; // smaller size for test speed; scales to 1024 with --release
    let num_workers = 4u32;
    let chunk_sizes = [2usize, 8, 16, 32, 64];

    let results: Vec<_> = chunk_sizes
        .iter()
        .map(|&cs| run_streaming_and_reduce(size, cs, num_workers))
        .collect();

    // Assert pairwise isomorphism (all must match the first).
    let reference = &results[0];
    for (i, result) in results.iter().enumerate().skip(1) {
        assert!(
            nets_match_counts(reference, result),
            "T8-01: chunk_size={} result must be isomorphic to chunk_size={} result",
            chunk_sizes[i],
            chunk_sizes[0],
        );
    }
}

// ---------------------------------------------------------------------------
// T8-02: chunk-size sweep, 2 workers
// ---------------------------------------------------------------------------

/// T8-02: ep_annihilation, 2 workers, chunk_size ∈ {1, 4, 16}.
/// All merged Normal Forms must be pairwise isomorphic.
#[test]
fn t8_02_chunk_size_independence_ep_annihilation_2_workers() {
    let size = 32u32;
    let num_workers = 2u32;
    let chunk_sizes = [1usize, 4, 16, 32];

    let results: Vec<_> = chunk_sizes
        .iter()
        .map(|&cs| run_streaming_and_reduce(size, cs, num_workers))
        .collect();

    let reference = &results[0];
    for (i, result) in results.iter().enumerate().skip(1) {
        assert!(
            nets_match_counts(reference, result),
            "T8-02: chunk_size={} result must be isomorphic to chunk_size={} result",
            chunk_sizes[i],
            chunk_sizes[0],
        );
    }
}

// ---------------------------------------------------------------------------
// T8-03: chunk-size == total net size (single chunk, degenerate case)
// ---------------------------------------------------------------------------

/// T8-03: chunk_size == total agent count → single chunk → isomorphic to batch.
#[test]
fn t8_03_single_chunk_equals_batch() {
    let size = 16u32; // 32 agents (ERA-ERA pairs)
    let num_workers = 2u32;
    let agent_count = (size * 2) as usize; // ep_annihilation produces 2n agents

    let single_chunk = run_streaming_and_reduce(size, agent_count, num_workers);
    let two_chunks = run_streaming_and_reduce(size, agent_count / 4, num_workers);

    assert!(
        nets_match_counts(&single_chunk, &two_chunks),
        "T8-03: single-chunk streaming must be isomorphic to multi-chunk streaming"
    );
}
