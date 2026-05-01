//! TASK-0603 — Tier 3 CLI flags on `BenchArgs`.
//!
//! Spec: SPEC-09 R18a–R18g (commit `82b2d27`); SPEC-21 §3.8 A3.
//!
//! Verifies the four new CLI flags introduced by TASK-0603:
//!
//! ```text
//! --chunk-size <N>             clap default: <none> (eager path)
//! --max-pending-lifetime <N>   clap default: 16
//! --recycle-policy <POLICY>    enum: disable-under-delta | border-clean ; default: disable-under-delta
//! --representation <MODE>      enum: dense | sparse ; default: dense
//! ```
//!
//! Tests cover (per TEST-SPEC-0603):
//!  - UT-0603-01..04 — explicit flag parsing for each Tier 3 flag.
//!  - UT-0603-05 — defaults preserved when flags are omitted (regression
//!    guard: existing bench scripts must not break).
//!  - UT-0603-06..07 — invalid enum values produce a clap error (not a panic).
//!  - UT-0603-08 — the `tier3_into_suite_config` mapping is 1:1 from
//!    `BenchArgs` to the four `BenchmarkSuiteConfig` fields.
//!  - IT-0603-09 — end-to-end CLI smoke (gated `#[ignore]` until TASK-0604
//!    lands the path-selection wiring).

use clap::Parser;
use relativist_core::bench::{NetRepresentation, RecyclePolicy};
use relativist_core::config::{Cli, Command};

/// Helper: parse a CLI invocation and unwrap to the `BenchArgs` payload.
fn parse_bench_args(args: &[&str]) -> relativist_core::config::BenchArgs {
    let cli = Cli::try_parse_from(args).expect("UT-0603: parse must succeed for valid args");
    match cli.command {
        Command::Bench(b) => b,
        other => panic!("UT-0603: expected Command::Bench, got {:?}", other),
    }
}

/// UT-0603-01 — `--chunk-size 1000` parses to `Some(1000)`.
#[test]
fn parse_chunk_size_some_value() {
    let args = parse_bench_args(&[
        "relativist",
        "bench",
        "--benchmark",
        "ep_annihilation",
        "--sizes",
        "1000",
        "--workers",
        "2",
        "--chunk-size",
        "1000",
    ]);
    assert_eq!(
        args.chunk_size,
        Some(1000_u32),
        "UT-0603-01: --chunk-size must parse to Option<u32>, NOT default to 0"
    );
    // Sanity — surrounding flags still work.
    assert_eq!(
        args.benchmark.as_deref().unwrap_or_default(),
        &["ep_annihilation".to_string()][..]
    );
}

/// UT-0603-02 — `--max-pending-lifetime 32` parses to `32`.
#[test]
fn parse_max_pending_lifetime_explicit_value() {
    let args = parse_bench_args(&[
        "relativist",
        "bench",
        "--benchmark",
        "ep_annihilation",
        "--sizes",
        "1000",
        "--workers",
        "2",
        "--max-pending-lifetime",
        "32",
    ]);
    assert_eq!(
        args.max_pending_lifetime, 32_u32,
        "UT-0603-02: --max-pending-lifetime must parse to the explicit value, \
         not silently fall back to the default"
    );
}

/// UT-0603-03 — both `RecyclePolicy` variants parse via clap kebab-case.
#[test]
fn parse_recycle_policy_each_variant() {
    let args = parse_bench_args(&[
        "relativist",
        "bench",
        "--benchmark",
        "ep_annihilation",
        "--sizes",
        "1000",
        "--workers",
        "2",
        "--recycle-policy",
        "disable-under-delta",
    ]);
    assert_eq!(args.recycle_policy, RecyclePolicy::DisableUnderDelta);

    let args = parse_bench_args(&[
        "relativist",
        "bench",
        "--benchmark",
        "ep_annihilation",
        "--sizes",
        "1000",
        "--workers",
        "2",
        "--recycle-policy",
        "border-clean",
    ]);
    assert_eq!(args.recycle_policy, RecyclePolicy::BorderClean);
}

/// UT-0603-04 — both `NetRepresentation` variants parse via kebab-case.
#[test]
fn parse_representation_each_variant() {
    let args = parse_bench_args(&[
        "relativist",
        "bench",
        "--benchmark",
        "ep_annihilation",
        "--sizes",
        "1000",
        "--workers",
        "2",
        "--representation",
        "dense",
    ]);
    assert_eq!(args.representation, NetRepresentation::Dense);

    let args = parse_bench_args(&[
        "relativist",
        "bench",
        "--benchmark",
        "ep_annihilation",
        "--sizes",
        "1000",
        "--workers",
        "2",
        "--representation",
        "sparse",
    ]);
    assert_eq!(args.representation, NetRepresentation::Sparse);
}

/// UT-0603-05 — when the four new flags are omitted, the parsed `BenchArgs`
/// carries the spec-defaults. Regression guard: existing bench scripts that
/// pre-date TASK-0603 must continue to work without modification.
#[test]
fn defaults_when_flags_omitted() {
    let args = parse_bench_args(&[
        "relativist",
        "bench",
        "--benchmark",
        "ep_annihilation",
        "--sizes",
        "1000",
        "--workers",
        "2",
    ]);
    assert_eq!(
        args.chunk_size, None,
        "UT-0603-05: chunk_size default must be None (eager path)"
    );
    assert_eq!(
        args.max_pending_lifetime, 16,
        "UT-0603-05: max_pending_lifetime default must be 16 (matches GridConfig)"
    );
    assert_eq!(
        args.recycle_policy,
        RecyclePolicy::DisableUnderDelta,
        "UT-0603-05: recycle_policy default must be DisableUnderDelta"
    );
    assert_eq!(
        args.representation,
        NetRepresentation::Dense,
        "UT-0603-05: representation default must be Dense"
    );
}

/// UT-0603-06 — an unrecognized `--recycle-policy` value yields a clap
/// `InvalidValue` error (NOT a panic, NOT a silent default fallback).
#[test]
fn invalid_recycle_policy_yields_clap_error() {
    let result = Cli::try_parse_from([
        "relativist",
        "bench",
        "--benchmark",
        "ep_annihilation",
        "--sizes",
        "1000",
        "--workers",
        "2",
        "--recycle-policy",
        "destroy-everything",
    ]);
    assert!(
        result.is_err(),
        "UT-0603-06: invalid value MUST be an error"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.kind(),
        clap::error::ErrorKind::InvalidValue,
        "UT-0603-06: error kind must be InvalidValue (got {:?})",
        err.kind()
    );
    let rendered = err.to_string();
    assert!(
        rendered.contains("destroy-everything"),
        "UT-0603-06: error must mention the offending value; got: {rendered}"
    );
    assert!(
        rendered.contains("disable-under-delta") && rendered.contains("border-clean"),
        "UT-0603-06: error must list valid alternatives; got: {rendered}"
    );
}

/// UT-0603-07 — symmetric to UT-0603-06 for `--representation`.
#[test]
fn invalid_representation_yields_clap_error() {
    let result = Cli::try_parse_from([
        "relativist",
        "bench",
        "--benchmark",
        "ep_annihilation",
        "--sizes",
        "1000",
        "--workers",
        "2",
        "--representation",
        "quantum",
    ]);
    assert!(
        result.is_err(),
        "UT-0603-07: invalid value MUST be an error"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.kind(),
        clap::error::ErrorKind::InvalidValue,
        "UT-0603-07: error kind must be InvalidValue (got {:?})",
        err.kind()
    );
    let rendered = err.to_string();
    assert!(
        rendered.contains("quantum"),
        "UT-0603-07: error must mention the offending value; got: {rendered}"
    );
    assert!(
        rendered.contains("dense") && rendered.contains("sparse"),
        "UT-0603-07: error must list valid alternatives; got: {rendered}"
    );
}

/// UT-0603-08 — `BenchArgs::tier3_into_suite_config` populates a 4-tuple
/// equivalent to the four `BenchmarkSuiteConfig` Tier 3 fields, 1:1 from
/// the parsed `BenchArgs`.
#[test]
fn bench_args_to_suite_config_mapping_is_one_to_one() {
    let args = parse_bench_args(&[
        "relativist",
        "bench",
        "--benchmark",
        "ep_annihilation",
        "--sizes",
        "1000",
        "--workers",
        "2",
        "--chunk-size",
        "500",
        "--max-pending-lifetime",
        "64",
        "--recycle-policy",
        "border-clean",
        "--representation",
        "sparse",
    ]);

    let (chunk_size, max_pending_lifetime, recycle_policy, representation) =
        args.tier3_into_suite_config();

    assert_eq!(
        chunk_size,
        Some(500),
        "UT-0603-08: chunk_size must be forwarded byte-equivalently"
    );
    assert_eq!(
        max_pending_lifetime, 64,
        "UT-0603-08: max_pending_lifetime must be forwarded byte-equivalently"
    );
    assert_eq!(
        recycle_policy,
        RecyclePolicy::BorderClean,
        "UT-0603-08: recycle_policy must be forwarded byte-equivalently"
    );
    assert_eq!(
        representation,
        NetRepresentation::Sparse,
        "UT-0603-08: representation must be forwarded byte-equivalently"
    );
}

/// IT-0603-09 — end-to-end CLI smoke. Originally `#[ignore]` pending TASK-0604;
/// promoted to active after TASK-0604 landed the streaming-path selection
/// wiring in `bench/suite.rs` (commit hash recorded in the dispatch summary).
/// `--chunk-size 100` now exercises the streaming branch end-to-end.
#[test]
fn cli_smoke_chunk_size_100_workers_2_completes_zero() {
    use std::process::Command;

    // Use a small EP annihilation workload so the smoke completes well
    // under the < 60 s budget called out in the test spec.
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
        .expect("IT-0603-09: cargo run --bin relativist must spawn");

    assert!(
        output.status.success(),
        "IT-0603-09: bench smoke must exit 0; status: {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ep_annihilation"),
        "IT-0603-09: stdout must mention the benchmark id; got: {stdout}"
    );
}
