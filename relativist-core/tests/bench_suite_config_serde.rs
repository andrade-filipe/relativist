//! TASK-0602 — Integration test for `BenchmarkSuiteConfig` Tier 3 fields.
//!
//! Spec: SPEC-09 R18a–R18g (commit `82b2d27`); SPEC-21 §3.8 A3; SPEC-22 R10b.
//!
//! TEST-SPEC-0602 §UT-0602-04 specifies a struct-level "round-trip" test
//! using `Clone` + `Debug` (the struct itself does NOT derive Serialize, per
//! the test spec's own out-of-scope §). This file mirrors the in-tree unit
//! test in `bench/mod.rs::tests::struct_clone_and_debug_round_trip_preserves_tier3_fields`
//! and additionally exercises the field via the public re-exports, ensuring
//! the type is reachable from outside the crate and that the new enums
//! (`RecyclePolicy`, `NetRepresentation`) are part of the public API surface.

use relativist_core::bench::{
    BenchmarkId, BenchmarkSuiteConfig, Mode, NetRepresentation, RecyclePolicy,
    DEFAULT_BENCH_MAX_PENDING_LIFETIME,
};

/// IT-0602-04: Build a `BenchmarkSuiteConfig` via the public API, set all 4
/// Tier 3 fields to non-default values, clone, and verify the new fields
/// round-trip via `Clone` + `Debug`. This proves the new fields are wired
/// into the derive pipeline (not orphaned by a manual `impl Clone`).
#[test]
fn struct_clone_and_debug_round_trip_preserves_tier3_fields() {
    let config = BenchmarkSuiteConfig {
        benchmarks: vec![BenchmarkId::EPAnnihilation],
        sizes: None,
        workers: vec![1],
        mode: Mode::Sequential,
        warmup_runs: 0,
        repetitions: 1,
        csv_detail_path: None,
        csv_rounds_path: None,
        csv_summary_path: None,
        max_rounds: None,
        strict_bsp: false,
        skip_g1: false,
        chunk_size: Some(123),
        max_pending_lifetime: 42,
        recycle_policy: RecyclePolicy::BorderClean,
        representation: NetRepresentation::Sparse,
    };

    let cloned = config.clone();
    let debug_str = format!("{:?}", config);

    assert_eq!(
        cloned.chunk_size,
        Some(123),
        "IT-0602-04: Clone must preserve chunk_size"
    );
    assert_eq!(
        cloned.max_pending_lifetime, 42,
        "IT-0602-04: Clone must preserve max_pending_lifetime"
    );
    assert_eq!(
        cloned.recycle_policy,
        RecyclePolicy::BorderClean,
        "IT-0602-04: Clone must preserve recycle_policy"
    );
    assert_eq!(
        cloned.representation,
        NetRepresentation::Sparse,
        "IT-0602-04: Clone must preserve representation"
    );

    assert!(
        debug_str.contains("chunk_size: Some(123)"),
        "IT-0602-04: Debug must render chunk_size; got {debug_str}"
    );
    assert!(
        debug_str.contains("max_pending_lifetime: 42"),
        "IT-0602-04: Debug must render max_pending_lifetime; got {debug_str}"
    );
    assert!(
        debug_str.contains("BorderClean"),
        "IT-0602-04: Debug must render recycle_policy variant; got {debug_str}"
    );
    assert!(
        debug_str.contains("Sparse"),
        "IT-0602-04: Debug must render representation variant; got {debug_str}"
    );
}

/// IT-0602: the public `DEFAULT_BENCH_MAX_PENDING_LIFETIME` constant equals
/// the spec-mandated value (16). Pinned here so external crates that depend
/// on this crate's bench API observe a stable default.
#[test]
fn public_default_max_pending_lifetime_matches_spec() {
    assert_eq!(
        DEFAULT_BENCH_MAX_PENDING_LIFETIME, 16,
        "TASK-0602 / SPEC-21 R37g: bench harness max_pending_lifetime default \
         must be 16 (matches coordinator GridConfig default)"
    );
}
