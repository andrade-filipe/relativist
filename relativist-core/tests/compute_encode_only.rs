//! D-017 / TASK-0728 integration tests for `compute --encode-only`.
//!
//! Validates the short-circuit branch of `run_compute_with_encoder` added
//! in TASK-0728: `--encode-only --output <path>` writes the un-reduced net
//! via `io::binary::save_bin` and skips both `reduce_all` and the decode
//! stage. The produced `.bin` is what the multi-container demo
//! (TASK-0730) feeds into the coordinator service.
//!
//! Acceptance criteria covered: AC1, AC2, AC3, AC4, AC5 of TASK-0728.

use relativist_core::commands::run_compute_command;
use relativist_core::config::{ArithmeticOp, ComputeArgs};
use relativist_core::encoding::default_registry;
use relativist_core::error::RelativistError;
use relativist_core::io::binary::load_bin;
use tempfile::NamedTempFile;

/// Build a `ComputeArgs` for an `--encode-only` Horner invocation.
fn encode_only_args(input_json: &str, output: std::path::PathBuf) -> ComputeArgs {
    ComputeArgs {
        operation: None,
        a: None,
        b: None,
        encoder: None,
        codec: Some("horner".to_string()),
        input: Some(input_json.to_string()),
        workers: None,
        output: Some(output),
        metrics: None,
        encode_only: true,
    }
}

// IT-0728-05: encode-only writes a non-empty bincode-valid `.bin`.
#[test]
fn encode_only_writes_nonempty_bin_for_horner_degree_2() {
    let tmp = NamedTempFile::new().unwrap();
    let args = encode_only_args(
        r#"{"coeffs":[10000,500,1],"x":100}"#,
        tmp.path().to_path_buf(),
    );
    run_compute_command(args).expect("encode-only must succeed");

    let meta = std::fs::metadata(tmp.path()).expect("output file must exist");
    assert!(meta.len() > 0, "encoded .bin must be non-empty");
    let loaded = load_bin(tmp.path()).expect("bincode must be valid");
    assert!(
        loaded.count_live_agents() > 0,
        "loaded net must have agents"
    );
}

// IT-0728-06: smallest valid Horner net (constant polynomial) encodes
// without panicking and round-trips through `load_bin`.
#[test]
fn encode_only_constant_polynomial_smallest_net() {
    let tmp = NamedTempFile::new().unwrap();
    let args = encode_only_args(r#"{"coeffs":[42],"x":7}"#, tmp.path().to_path_buf());
    run_compute_command(args).expect("constant polynomial must encode");

    let loaded = load_bin(tmp.path()).unwrap();
    assert!(loaded.count_live_agents() > 0);
}

// IT-0728-07: max-envelope degree-2 polynomial encodes deterministically
// — two runs of the same input MUST yield the same live agent count.
#[test]
fn encode_only_degree_2_max_envelope_loads_cleanly() {
    let input = r#"{"coeffs":[10000,500,1],"x":100}"#;
    let tmp1 = NamedTempFile::new().unwrap();
    let tmp2 = NamedTempFile::new().unwrap();

    run_compute_command(encode_only_args(input, tmp1.path().to_path_buf()))
        .expect("envelope-max input must encode (run 1)");
    run_compute_command(encode_only_args(input, tmp2.path().to_path_buf()))
        .expect("envelope-max input must encode (run 2)");

    let loaded1 = load_bin(tmp1.path()).unwrap();
    let loaded2 = load_bin(tmp2.path()).unwrap();
    assert!(loaded1.count_live_agents() > 0);
    assert_eq!(
        loaded1.count_live_agents(),
        loaded2.count_live_agents(),
        "HornerCodec must be deterministic across runs"
    );
}

// IT-0728-08 (CANARY): encode-only must NOT call reduce_all.
// Proven by checking that the persisted net's redex_queue is still
// populated after load.
#[test]
fn encode_only_does_not_reduce_redexes_remain_in_queue() {
    let tmp = NamedTempFile::new().unwrap();
    let args = encode_only_args(
        r#"{"coeffs":[10000,500,1],"x":100}"#,
        tmp.path().to_path_buf(),
    );
    run_compute_command(args).unwrap();

    // Precondition cross-check: a fresh encode of the same input has
    // non-empty redex_queue. If it didn't, the canary would be vacuous.
    let fresh = default_registry()
        .encode_and_validate("horner", br#"{"coeffs":[10000,500,1],"x":100}"#)
        .unwrap();
    assert!(
        !fresh.redex_queue.is_empty(),
        "precondition: HornerCodec must produce non-empty redex_queue for non-trivial input"
    );

    let loaded = load_bin(tmp.path()).unwrap();
    assert!(
        !loaded.redex_queue.is_empty(),
        "encode-only must NOT reduce; redex_queue must be non-empty for non-trivial input"
    );
}

// IT-0728-09: legacy positional `compute add 3 5 --encode-only` rejects
// with a clear Config error (no panic).
#[test]
fn legacy_positional_compute_rejects_encode_only() {
    let tmp = NamedTempFile::new().unwrap();
    let args = ComputeArgs {
        operation: Some(ArithmeticOp::Add),
        a: Some(3),
        b: Some(5),
        encoder: None,
        codec: None,
        input: None,
        workers: None,
        output: Some(tmp.path().to_path_buf()),
        metrics: None,
        encode_only: true,
    };
    let res = run_compute_command(args);
    match res {
        Err(RelativistError::Config(msg)) => {
            let low = msg.to_lowercase();
            assert!(
                low.contains("encoder") || low.contains("codec") || low.contains("encode-only"),
                "config error must mention encoder/codec/encode-only, got: {msg}"
            );
        }
        Ok(()) => panic!("legacy positional + --encode-only must error"),
        Err(other) => panic!("expected Config error, got {other:?}"),
    }
}

// IT-0728-10: backward compat — without `--encode-only`, the encode→reduce
// →decode path keeps working.
#[test]
fn backward_compat_no_encode_only_still_reduces_and_decodes() {
    let args = ComputeArgs {
        operation: None,
        a: None,
        b: None,
        encoder: None,
        codec: Some("horner".to_string()),
        input: Some(r#"{"coeffs":[10000,500,1],"x":100}"#.to_string()),
        workers: None,
        output: None,
        metrics: None,
        encode_only: false,
    };
    run_compute_command(args).expect("legacy encoder path must still work");
}
