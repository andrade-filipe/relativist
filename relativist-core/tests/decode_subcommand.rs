//! D-017 / TASK-0729 integration tests for `relativist decode`.
//!
//! Covers the runtime contract of `run_decode_command`:
//!   - Round-trip equivalence with the in-process compute pipeline.
//!   - Post-load root recovery (HornerCodec leaves `root = None`).
//!   - Error propagation on un-reduced nets, unknown codecs, corrupt
//!     bincode payloads.
//!   - Stdout vs `--output <path>` sink branches.

use relativist_core::commands::run_decode_command;
use relativist_core::config::DecodeArgs;
use relativist_core::encoding::{default_registry, discover_root};
use relativist_core::error::RelativistError;
use relativist_core::io::binary::{load_bin, save_bin};
use relativist_core::reduction::reduce_all;
use serde_json::Value;
use tempfile::NamedTempFile;

const HORNER_INPUT: &[u8] = br#"{"coeffs":[10000,500,1],"x":100}"#;

fn make_reduced_horner_bin() -> NamedTempFile {
    let reg = default_registry();
    let mut net = reg.encode_and_validate("horner", HORNER_INPUT).unwrap();
    let _ = reduce_all(&mut net);
    let tmp = NamedTempFile::new().unwrap();
    save_bin(&net, tmp.path()).unwrap();
    tmp
}

fn make_unreduced_horner_bin() -> NamedTempFile {
    let reg = default_registry();
    let net = reg.encode_and_validate("horner", HORNER_INPUT).unwrap();
    let tmp = NamedTempFile::new().unwrap();
    save_bin(&net, tmp.path()).unwrap();
    tmp
}

// IT-0729-05: decode a reduced .bin → JSON equals in-process pipeline output.
#[test]
fn decode_reduced_horner_bin_matches_inproc_pipeline() {
    let bin = make_reduced_horner_bin();
    let out = NamedTempFile::new().unwrap();
    let args = DecodeArgs {
        codec: Some("horner".to_string()),
        encoder: None,
        input: bin.path().to_path_buf(),
        output: Some(out.path().to_path_buf()),
    };
    run_decode_command(args).expect("decode must succeed");
    let written = std::fs::read_to_string(out.path()).unwrap();
    let decoded: Value = serde_json::from_str(&written).unwrap();

    // Cross-check against the in-process baseline.
    let reg = default_registry();
    let mut ref_net = reg.encode_and_validate("horner", HORNER_INPUT).unwrap();
    let _ = reduce_all(&mut ref_net);
    if ref_net.root.is_none() {
        discover_root(&mut ref_net);
    }
    let ref_json = reg.decode("horner", &ref_net).unwrap();

    assert_eq!(decoded, ref_json);
    // SPEC-27 v3 R15' schema sanity checks.
    assert!(
        decoded.get("value").is_some(),
        "JSON must have 'value' field"
    );
    assert!(
        decoded.get("bit_length").is_some(),
        "JSON must have 'bit_length' field"
    );
}

// IT-0729-06: nets with root=None on disk decode after automatic
// `discover_root` recovery.
#[test]
fn decode_recovers_root_when_missing_post_load() {
    let reg = default_registry();
    let mut net = reg.encode_and_validate("horner", HORNER_INPUT).unwrap();
    let _ = reduce_all(&mut net);
    // Force root=None to simulate the post-merge state (HornerCodec already
    // does this naturally, but be explicit to defend against silent changes).
    net.root = None;
    let tmp = NamedTempFile::new().unwrap();
    save_bin(&net, tmp.path()).unwrap();

    // Sanity: load preserves root=None.
    let loaded_check = load_bin(tmp.path()).unwrap();
    assert!(
        loaded_check.root.is_none(),
        "saved net must round-trip root=None"
    );

    let args = DecodeArgs {
        codec: Some("horner".to_string()),
        encoder: None,
        input: tmp.path().to_path_buf(),
        output: None,
    };
    run_decode_command(args).expect("decode must succeed even when root=None on disk");
}

// IT-0729-07: decoding an un-reduced .bin returns a clear error (no panic).
#[test]
fn decode_on_unreduced_bin_returns_clear_error_no_panic() {
    let bin = make_unreduced_horner_bin();
    let args = DecodeArgs {
        codec: Some("horner".to_string()),
        encoder: None,
        input: bin.path().to_path_buf(),
        output: None,
    };
    let res = run_decode_command(args);
    match res {
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            // SPEC-27 v3 R4 NotNormalForm-style failure. Accept any wording
            // that points at the right cause (redexes / normal form / not
            // reduced / structure).
            assert!(
                msg.contains("normal form")
                    || msg.contains("redex")
                    || msg.contains("not reduced")
                    || msg.contains("structure"),
                "expected a NotNormalForm-style error, got: {e}"
            );
        }
        Ok(()) => panic!("decoding an un-reduced net MUST error"),
    }
}

// IT-0729-08: if the registry exposes another codec besides `horner`, try
// decoding a Horner .bin with the wrong codec — must error, not panic.
#[test]
fn decode_with_wrong_codec_returns_clear_error() {
    let names: Vec<String> = default_registry()
        .list()
        .iter()
        .map(|(n, _)| (*n).to_string())
        .collect();
    let alt = names.iter().find(|n| n.as_str() != "horner").cloned();
    if let Some(other) = alt {
        let bin = make_reduced_horner_bin();
        let args = DecodeArgs {
            codec: Some(other.clone()),
            encoder: None,
            input: bin.path().to_path_buf(),
            output: None,
        };
        let res = run_decode_command(args);
        assert!(
            res.is_err(),
            "decoding horner bytes as `{other}` must error, not panic"
        );
    }
    // If only `horner` is registered post-D-017, this test degenerates to a
    // no-op; the path is also covered by `decode_with_unknown_codec_...`.
}

// IT-0729-09: unknown codec name returns Config error mentioning the name
// or "codec" / "not found".
#[test]
fn decode_with_unknown_codec_name_returns_config_error() {
    let bin = make_reduced_horner_bin();
    let args = DecodeArgs {
        codec: Some("nonexistent_codec_xyz".to_string()),
        encoder: None,
        input: bin.path().to_path_buf(),
        output: None,
    };
    let res = run_decode_command(args);
    match res {
        Err(RelativistError::Encoding(msg)) | Err(RelativistError::Config(msg)) => {
            let low = msg.to_lowercase();
            assert!(
                low.contains("nonexistent_codec_xyz")
                    || low.contains("codec")
                    || low.contains("not found")
                    || low.contains("unknown"),
                "error must reference the unknown name or 'codec', got: {msg}"
            );
        }
        Ok(()) => panic!("unknown codec name MUST error"),
        Err(other) => panic!("expected Config/Encoding error, got {other:?}"),
    }
}

// IT-0729-10: corrupt .bin returns Config error mentioning the path or
// the underlying bincode failure (no panic).
#[test]
fn decode_corrupt_bin_returns_config_error_with_path() {
    let tmp = NamedTempFile::new().unwrap();
    std::fs::write(
        tmp.path(),
        b"this is not bincode at all, just garbage bytes",
    )
    .unwrap();
    let args = DecodeArgs {
        codec: Some("horner".to_string()),
        encoder: None,
        input: tmp.path().to_path_buf(),
        output: None,
    };
    let res = run_decode_command(args);
    match res {
        Err(e) => {
            let s = e.to_string().to_lowercase();
            let path_str = tmp.path().display().to_string().to_lowercase();
            assert!(
                s.contains("bincode")
                    || s.contains("deserialize")
                    || s.contains("deserialization")
                    || s.contains("corrupt")
                    || s.contains("load")
                    || s.contains("failed")
                    || s.contains(&path_str),
                "corrupt .bin error must mention parse failure or path, got: {e}"
            );
        }
        Ok(()) => panic!("corrupt .bin must not decode silently"),
    }
}

// IT-0729-11: when --output is absent, the JSON sink is stdout — the
// handler returns Ok and writes no file. (Stdout capture is deliberately
// out of scope to avoid coupling to formatting.)
#[test]
fn decode_stdout_path_when_output_absent() {
    let bin = make_reduced_horner_bin();
    let args = DecodeArgs {
        codec: Some("horner".to_string()),
        encoder: None,
        input: bin.path().to_path_buf(),
        output: None,
    };
    run_decode_command(args).expect("decode to stdout must succeed");
}
