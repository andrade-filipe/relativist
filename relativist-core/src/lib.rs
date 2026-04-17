//! Relativist — Distributed reduction of Interaction Combinators on Grid Computing.
//!
//! This crate implements a coordinator-worker grid architecture for reducing
//! Interaction Combinator networks across multiple machines, validating that
//! distributed reduction produces identical results to sequential reduction
//! (the Fundamental Property, SPEC-01 G1).

pub mod error;

// Core types: Symbol, Agent, Net, PortRef (SPEC-02)
pub mod net;

// Reduction engine: 6 interaction rules, reduce_all (SPEC-03)
pub mod reduction;

// Partitioning: split net into worker partitions (SPEC-04)
pub mod partition;

// Merge and grid cycle: reunite partitions, resolve borders (SPEC-05)
pub mod merge;

// Wire protocol: TCP messaging, framing, Transport trait (SPEC-06)
pub mod protocol;

// Coordinator FSM (SPEC-13 R19-R23)
pub mod coordinator;

// Worker FSM (SPEC-13 R24-R27)
pub mod worker;

// Configuration and CLI support (SPEC-07, SPEC-13)
pub mod config;

// Command entry points for each CLI subcommand
pub mod commands;

// Security: token auth, optional TLS (SPEC-10)
pub mod security;

// Observability: tracing, metrics (SPEC-11)
pub mod observability;

// User I/O: net formats, generators, examples (SPEC-12)
pub mod io;

// Arithmetic encoding: Church numerals, arithmetic combinators (SPEC-14)
pub mod encoding;

// Benchmark suite: parametric workloads, metrics, CSV output (SPEC-09)
pub mod bench;

#[cfg(test)]
mod tests {
    /// SPEC-18 R20 — `zero-copy` MUST NOT be a default feature. This test
    /// runs ONLY in the default-features build (cfg-gated) and asserts the
    /// feature is OFF. Static manifest check `cargo_toml_default_features_excludes_zero_copy`
    /// is the cross-build complement that runs in both configurations.
    #[cfg(not(feature = "zero-copy"))]
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn zero_copy_feature_is_off_by_default() {
        assert!(
            !cfg!(feature = "zero-copy"),
            "SPEC-18 R20: 'zero-copy' must NOT be a default feature; \
             re-check Cargo.toml [features] table"
        );
    }

    /// SPEC-18 R20 — When `--features zero-copy` is active, the rkyv crate
    /// is in the dep tree. We probe for a known rkyv symbol that only
    /// compiles with the dep present.
    #[cfg(feature = "zero-copy")]
    #[test]
    fn zero_copy_feature_activates_rkyv() {
        // Probe: AlignedVec is the symbol used by TASK-0355 and TASK-0357.
        // If rkyv is not in the dep tree, this would not compile.
        let v: rkyv::util::AlignedVec = rkyv::util::AlignedVec::with_capacity(16);
        assert_eq!(v.len(), 0);
        // R25 precondition: AlignedVec capacity is a multiple of 16.
        assert!(
            v.capacity() == 0 || v.capacity() % 16 == 0,
            "AlignedVec capacity must be 0 or a multiple of 16 (R25 precondition); got {}",
            v.capacity()
        );
    }

    /// Smoke test: rkyv round-trips a primitive `u32` end-to-end via
    /// `to_bytes` -> `access` -> `deserialize`. Confirms the validating API
    /// signature is available with the chosen feature set
    /// (`bytecheck` + `alloc` + `std`).
    #[cfg(feature = "zero-copy")]
    #[test]
    fn rkyv_validating_round_trip_smoke() {
        let v: u32 = 0xDEAD_BEEF;
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&v).expect("serialize");
        let archived = rkyv::access::<rkyv::Archived<u32>, rkyv::rancor::Error>(bytes.as_ref())
            .expect("access");
        let round: u32 =
            rkyv::deserialize::<u32, rkyv::rancor::Error>(archived).expect("deserialize");
        assert_eq!(round, v);
    }

    /// SPEC-18 R20 — Static parse of `Cargo.toml` confirms `default = []`
    /// (or any list that does NOT contain `"zero-copy"`).
    #[test]
    fn cargo_toml_default_features_excludes_zero_copy() {
        let manifest = include_str!("../Cargo.toml");
        // Find the `[features]` section.
        let features_idx = manifest
            .find("[features]")
            .expect("Cargo.toml must have a [features] table");
        let after = &manifest[features_idx..];
        // Look for the `default = ` line.
        let default_idx = after
            .find("default = ")
            .expect("[features] must declare a `default = ` entry");
        // Take that line up to the next newline.
        let default_line_end = after[default_idx..].find('\n').unwrap_or(after.len());
        let default_line = &after[default_idx..default_idx + default_line_end];
        assert!(
            !default_line.contains("zero-copy"),
            "SPEC-18 R20 violated: `default` features list contains \
             'zero-copy'. Got line: {}",
            default_line
        );
    }
}
