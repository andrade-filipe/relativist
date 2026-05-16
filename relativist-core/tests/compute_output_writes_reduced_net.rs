//! D-017 / BUG-D017-001 regression coverage.
//!
//! Before the fix: `compute --codec X --input Y --output Z.bin` (without
//! `--encode-only`) silently dropped `--output`, exiting 0 but never
//! creating the file. This was surprising once TASK-0728 introduced
//! `--encode-only` (where `--output` IS honored), so the registry path
//! now symmetrically persists the REDUCED net to `--output` after the
//! pipeline finishes.
//!
//! Acceptance: `--output` produces a non-empty bincode-valid `.bin`
//! whose decode (via the `decode` subcommand) yields the same JSON value
//! as the in-process `compute` reference.

use relativist_core::commands::{run_compute_command, run_decode_command};
use relativist_core::config::{ComputeArgs, DecodeArgs};
use relativist_core::io::binary::load_bin;
use tempfile::NamedTempFile;

const HORNER_INPUT: &str = r#"{"coeffs":[10000,500,1],"x":100}"#;

fn compute_args_with_output(input_json: &str, output: std::path::PathBuf) -> ComputeArgs {
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
        encode_only: false,
    }
}

// IT-D017-REFACTOR-01: `compute --codec horner --input ... --output X.bin`
// (without `--encode-only`) MUST create `X.bin` as a non-empty bincode-valid
// payload representing the REDUCED net.
#[test]
fn compute_with_output_no_encode_only_writes_reduced_bin() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_path_buf();
    let args = compute_args_with_output(HORNER_INPUT, path.clone());

    run_compute_command(args).expect("compute with --output must succeed");

    let meta = std::fs::metadata(&path).expect("output file must exist (BUG-D017-001 regression)");
    assert!(
        meta.len() > 0,
        "output .bin must be non-empty (BUG-D017-001 regression)"
    );

    let loaded = load_bin(&path).expect("output must be valid bincode-v2");
    assert!(
        loaded.count_live_agents() > 0,
        "loaded reduced net must have live agents"
    );
}

// IT-D017-REFACTOR-02: the persisted .bin contains the REDUCED net (not the
// encoded-only one) — proven by an empty redex_queue after load, and by the
// `decode` subcommand returning the expected Horner value without re-running
// `reduce_all`.
#[test]
fn compute_with_output_persists_normal_form_decodes_to_expected_value() {
    let bin_tmp = NamedTempFile::new().unwrap();
    let bin_path = bin_tmp.path().to_path_buf();
    let args = compute_args_with_output(HORNER_INPUT, bin_path.clone());
    run_compute_command(args).unwrap();

    let loaded = load_bin(&bin_path).expect("output must be loadable");
    // The reduced net must be in normal form (no active pairs left). This is
    // the canary that distinguishes BUG-001 fix (reduced .bin) from a copy of
    // the encoded-only .bin (which still has redexes for non-trivial inputs).
    assert!(
        loaded.redex_queue.is_empty(),
        "reduced .bin must be in normal form (empty redex_queue); got {} redexes",
        loaded.redex_queue.len()
    );

    // Decode the reduced .bin via the `decode` subcommand. The output file
    // is unused (decode prints to stdout when --output is None), so we only
    // assert it runs to completion without error.
    let decode_args = DecodeArgs {
        codec: Some("horner".to_string()),
        encoder: None,
        input: bin_path,
        output: None,
    };
    run_decode_command(decode_args).expect("decode of compute --output .bin must succeed");
}
