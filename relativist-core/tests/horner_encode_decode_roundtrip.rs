//! D-017 / TASK-0731 integration tests — end-to-end roundtrip of the
//! TASK-0728 (`compute --encode-only`) + TASK-0729 (`decode`) pair plus
//! optional distributed smokes.
//!
//! These tests certify the contract the multi-container Horner demo
//! (TASK-0730) relies on: `save_bin` + `load_bin` is semantically
//! transparent across the reduction boundary, and the `decode`
//! subcommand returns the same JSON whether the reduction happened
//! in-process or out-of-process.
//!
//! - IT-0731-01..05: default-running roundtrip + boundary cases.
//! - IT-0731-06..07: negative paths duplicated from TEST-SPEC-0729 so
//!   the D-017 contract is self-contained for the demo script.
//! - PT-0731-09 (optional): property-based roundtrip over the working
//!   envelope.
//! - IT-0731-11: `#[ignore]` Docker smoke driving `horner_distributed_demo.sh`.

use relativist_core::commands::run_decode_command;
use relativist_core::config::DecodeArgs;
use relativist_core::encoding::{default_registry, discover_root};
use relativist_core::error::RelativistError;
use relativist_core::io::binary::{load_bin, save_bin};
use relativist_core::reduction::reduce_all;
use serde_json::Value;
use tempfile::NamedTempFile;

/// Reference input matching `scripts/horner_live_demo.sh` working envelope.
const HORNER_INPUT: &[u8] = br#"{"coeffs":[10000,500,1],"x":100}"#;

fn inproc_reference_json() -> Value {
    let reg = default_registry();
    let mut net = reg.encode_and_validate("horner", HORNER_INPUT).unwrap();
    let _ = reduce_all(&mut net);
    if net.root.is_none() {
        discover_root(&mut net);
    }
    reg.decode("horner", &net).unwrap()
}

// IT-0731-01: in-process baseline — JSON shape and parseability.
#[test]
fn encode_reduce_decode_inproc_returns_expected_value() {
    let json = inproc_reference_json();
    assert!(json.get("value").is_some(), "schema R15': missing 'value'");
    assert!(
        json.get("bit_length").is_some(),
        "schema R15': missing 'bit_length'"
    );
    let value_str = json["value"]
        .as_str()
        .expect("value must be a decimal string");
    assert!(
        value_str.parse::<u128>().is_ok(),
        "value must be a parseable decimal integer, got: {value_str}"
    );
}

// IT-0731-02: the load-bearing roundtrip — encode → save → load → reduce
// → decode equals the pure in-process baseline. This is the single most
// important test of D-017.
#[test]
fn encode_save_load_reduce_decode_matches_inproc() {
    let reg = default_registry();
    let net = reg.encode_and_validate("horner", HORNER_INPUT).unwrap();

    let tmp = NamedTempFile::new().unwrap();
    save_bin(&net, tmp.path()).unwrap();

    let mut loaded = load_bin(tmp.path()).unwrap();
    let _ = reduce_all(&mut loaded);
    if loaded.root.is_none() {
        discover_root(&mut loaded);
    }
    let json_loaded = reg.decode("horner", &loaded).unwrap();

    let json_ref = inproc_reference_json();
    assert_eq!(
        json_loaded, json_ref,
        "save/load roundtrip must produce identical JSON to in-process pipeline"
    );
}

// IT-0731-03 (CANARY for TASK-0728 short-circuit): encode-only `.bin`
// preserves the redex_queue across save/load.
#[test]
fn encode_only_bin_preserves_redex_queue() {
    let reg = default_registry();
    let net = reg.encode_and_validate("horner", HORNER_INPUT).unwrap();
    assert!(
        !net.redex_queue.is_empty(),
        "canary precondition: HornerCodec must emit redexes pre-reduce"
    );

    let tmp = NamedTempFile::new().unwrap();
    save_bin(&net, tmp.path()).unwrap();
    let loaded = load_bin(tmp.path()).unwrap();
    assert_eq!(
        loaded.redex_queue.len(),
        net.redex_queue.len(),
        "save/load must preserve redex_queue length"
    );
    assert_eq!(
        loaded.count_live_agents(),
        net.count_live_agents(),
        "save/load must preserve live agent count"
    );
}

// IT-0731-04: constant polynomial roundtrip — the only test in D-017 that
// hardcodes a specific decoded integer (42, independent of x).
#[test]
fn constant_polynomial_roundtrip_does_not_panic() {
    let input = br#"{"coeffs":[42],"x":7}"#;
    let reg = default_registry();
    let net = reg.encode_and_validate("horner", input).unwrap();
    let tmp = NamedTempFile::new().unwrap();
    save_bin(&net, tmp.path()).unwrap();
    let mut loaded = load_bin(tmp.path()).unwrap();
    let _ = reduce_all(&mut loaded);
    if loaded.root.is_none() {
        discover_root(&mut loaded);
    }
    let json = reg
        .decode("horner", &loaded)
        .expect("constant polynomial must decode");
    let v: u64 = json["value"].as_str().unwrap().parse().unwrap();
    assert_eq!(
        v, 42,
        "constant polynomial coeffs=[42] must decode to 42 regardless of x"
    );
}

// IT-0731-05: envelope-max degree-2 input — same assertion as IT-0731-02
// against an explicitly different (here identical to HORNER_INPUT) input
// pinned to the envelope ceiling per task notes.
#[test]
fn degree_2_envelope_roundtrip_matches_inproc() {
    let reg = default_registry();
    let net = reg.encode_and_validate("horner", HORNER_INPUT).unwrap();
    let tmp = NamedTempFile::new().unwrap();
    save_bin(&net, tmp.path()).unwrap();
    let mut loaded = load_bin(tmp.path()).unwrap();
    let _ = reduce_all(&mut loaded);
    if loaded.root.is_none() {
        discover_root(&mut loaded);
    }
    let json_loaded = reg.decode("horner", &loaded).unwrap();
    assert_eq!(json_loaded, inproc_reference_json());
}

// IT-0731-06: D-017 contract suite — `decode` on garbage input errors.
#[test]
fn decode_corrupt_bin_returns_error_no_panic() {
    let tmp = NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), b"corrupt").unwrap();
    let args = DecodeArgs {
        codec: Some("horner".to_string()),
        encoder: None,
        input: tmp.path().to_path_buf(),
        output: None,
    };
    let res = run_decode_command(args);
    assert!(res.is_err(), "corrupt .bin must error, not panic");
}

// IT-0731-07: D-017 contract suite — unknown codec returns a Config or
// Encoding error (RegistryError flows through `From<RegistryError>` into
// `RelativistError::Encoding`).
#[test]
fn decode_unknown_codec_returns_config_error() {
    let reg = default_registry();
    let mut ref_net = reg.encode_and_validate("horner", HORNER_INPUT).unwrap();
    let _ = reduce_all(&mut ref_net);
    let tmp = NamedTempFile::new().unwrap();
    save_bin(&ref_net, tmp.path()).unwrap();
    let args = DecodeArgs {
        codec: Some("does_not_exist".to_string()),
        encoder: None,
        input: tmp.path().to_path_buf(),
        output: None,
    };
    let res = run_decode_command(args);
    match res {
        Err(RelativistError::Config(_)) | Err(RelativistError::Encoding(_)) => {}
        other => panic!("expected Config or Encoding error for unknown codec, got {other:?}"),
    }
}

// PT-0731-09 (optional): for all envelope-bounded Horner inputs,
// save/load is semantically transparent.
//
// Generates `coeffs ∈ [0, 10000]^k`, `k ∈ {1, 2}`, `x ∈ [0, 100]` to stay
// within the v1 readback envelope (degree-2 requires `c2 == 1` per
// HornerCodec — outside the property generator). Capped at 12 cases to
// keep debug builds under ~60s (each case re-encodes + re-reduces +
// re-decodes twice over a non-trivial HornerCodec input space). Release
// builds are ~20× faster and complete in well under 10s.
proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config { cases: 12, .. proptest::test_runner::Config::default() })]
    #[test]
    fn save_load_roundtrip_preserves_decoded_value(
        coeffs in proptest::collection::vec(0u64..=10_000, 1..=2),
        x in 0u64..=100u64,
    ) {
        let input = format!(r#"{{"coeffs":{:?},"x":{}}}"#, coeffs, x);
        let reg = default_registry();
        let a = reg.encode_and_validate("horner", input.as_bytes());
        // Skip cases the encoder rejects (e.g., out-of-envelope shapes).
        let Ok(mut a) = a else { return Ok(()); };
        let _ = reduce_all(&mut a);
        if a.root.is_none() { discover_root(&mut a); }
        let json_a = reg.decode("horner", &a);

        let net = reg
            .encode_and_validate("horner", input.as_bytes())
            .expect("re-encode of same input must succeed");
        let tmp = NamedTempFile::new().unwrap();
        save_bin(&net, tmp.path()).unwrap();
        let mut b = load_bin(tmp.path()).unwrap();
        let _ = reduce_all(&mut b);
        if b.root.is_none() { discover_root(&mut b); }
        let json_b = reg.decode("horner", &b);

        // Compare the Result variants by debug formatting — both sides
        // must agree on success/failure AND on the inner JSON when both
        // succeed.
        let s_a = format!("{:?}", json_a);
        let s_b = format!("{:?}", json_b);
        proptest::prop_assert_eq!(s_a, s_b);
    }
}

// IT-0731-11: Docker smoke — invokes `scripts/horner_distributed_demo.sh`
// and asserts a non-empty JSON line on stdout. Ignored by default so
// `cargo test` stays green without a Docker daemon.
#[test]
#[ignore = "requires Docker + built release binary; run with --ignored"]
fn multi_container_horner_e2e_docker() {
    let output = std::process::Command::new("bash")
        .arg("scripts/horner_distributed_demo.sh")
        .arg("--workers")
        .arg("2")
        .output()
        .expect("invoke demo script");
    assert!(
        output.status.success(),
        "demo script must exit 0; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_line = stdout
        .lines()
        .find(|l| l.trim_start().starts_with('{'))
        .expect("stdout must contain a JSON line from `decode`");
    let json: Value = serde_json::from_str(json_line).expect("decoded JSON must parse");
    assert!(
        json.get("value").is_some(),
        "decoded JSON must have 'value' field"
    );
}

// IT-0731-10: TCP-localhost smoke deferred — TASK-0731 explicitly allows
// the developer to mark this `#[ignore]` if the in-process harness from
// `tests/horner_distributed_g1.rs` cannot be reused without significant
// scaffolding (NG note: "Note: If implementation is non-trivial, …").
// Documenting the gap here keeps the contract visible.
#[test]
#[ignore = "TCP localhost smoke; deferred to a future bundle. See docs/next-steps.md."]
fn tcp_localhost_distributed_matches_inproc() {
    // Placeholder: see TEST-SPEC-TASK-0731 IT-0731-10 skeleton. Coverage
    // of the same contract is provided by IT-0731-11 (Docker smoke) and
    // by `tests/horner_distributed_g1.rs::*` (in-process G1 witness).
}
