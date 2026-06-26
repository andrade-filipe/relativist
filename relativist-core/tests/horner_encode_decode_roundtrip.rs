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
//
// D-017 SF-004 fix: silent skip-arms on encoder rejection used to risk
// "test passes because nothing was tested" — an envelope tightening that
// rejected 100% of generated inputs would still pass green. The
// companion test `pt_0731_09_acceptance_ratio_above_floor` below drives
// a fresh 32-case sample of the same generator space and fails if the
// encoder rejects more than 50% of inputs, so a future envelope
// tightening that silently nukes PT-0731-09 surfaces as a hard error.

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
        // Acceptance ratio is asserted by the guard test below — a 100%
        // rejection rate (silent green) is impossible.
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

// PT-0731-09-GUARD (D-017 SF-004): assert PT-0731-09 actually exercises
// the codec. Drives a fresh 32-case sample of the same generator space
// independently of PT-0731-09's runtime ordering (cargo can run tests
// in any order), so the guard is robust against shared-state leaks.
//
// With the current envelope (`coeffs.len() in {1, 2}`, `x in [0, 100]`)
// every generated case is in-envelope, so the realistic acceptance
// ratio is ~100%; the 50% floor leaves slack for narrow future envelope
// tightenings without ratcheting QA noise — but flips to RED on a
// regression that rejects every case.
#[test]
fn pt_0731_09_acceptance_ratio_above_floor() {
    use proptest::strategy::{Strategy, ValueTree};
    use proptest::test_runner::{Config, TestRunner};

    let mut runner = TestRunner::new(Config {
        cases: 32,
        ..Config::default()
    });
    let reg = default_registry();
    let strat = (
        proptest::collection::vec(0u64..=10_000, 1..=2),
        0u64..=100u64,
    );

    let mut accepted = 0usize;
    let mut rejected = 0usize;
    let total = 32usize;
    for _ in 0..total {
        let tree = strat
            .new_tree(&mut runner)
            .expect("strategy must produce a value tree");
        let (coeffs, x) = tree.current();
        let input = format!(r#"{{"coeffs":{:?},"x":{}}}"#, coeffs, x);
        match reg.encode_and_validate("horner", input.as_bytes()) {
            Ok(_) => accepted += 1,
            Err(_) => rejected += 1,
        }
    }
    assert_eq!(accepted + rejected, total);
    assert!(
        accepted * 2 >= total,
        "PT-0731-09 generator rejected {}/{} cases (>{}%); envelope drift?",
        rejected,
        total,
        50
    );
}

// IT-0731-11: Docker smoke — invokes
// `reproduce_article/scripts/horner_distributed_demo.sh` and asserts a
// non-empty JSON line on stdout. Ignored by default so `cargo test`
// stays green without a Docker daemon.
//
// D-017 / BUG-D017-007 fix: resolve the script path AND the cwd via
// `env!("CARGO_MANIFEST_DIR")` joined to `../` (the workspace root).
// Cargo invokes integration tests with cwd = the crate manifest dir
// (here, `relativist-core/`) but the script expects the repo root
// because it computes `REPO_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"`
// (the script lives at reproduce_article/scripts/) and
// docker-compose.yml is at the repo root. Without an explicit
// `current_dir`, the script would fail with "No such file or
// directory" silently under `#[ignore]`.
#[test]
#[ignore = "requires Docker + built release binary; run with --ignored"]
fn multi_container_horner_e2e_docker() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR must have a parent (workspace root)")
        .to_path_buf();
    let script = repo_root
        .join("reproduce_article")
        .join("scripts")
        .join("horner_distributed_demo.sh");
    assert!(
        script.exists(),
        "demo script must exist at {} (workspace layout drift?)",
        script.display()
    );

    let output = std::process::Command::new("bash")
        .arg(&script)
        .arg("--workers")
        .arg("2")
        .current_dir(&repo_root)
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

// IT-D017-REFACTOR-03: non-ignored guard for BUG-D017-007 — asserts the
// script path resolution scheme used by IT-0731-11 is sound, so a layout
// drift can't silently break the Docker smoke (per QA TG-D017-03).
#[test]
fn horner_distributed_demo_script_resolves_from_manifest_dir() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR must have a parent");
    let script = repo_root
        .join("reproduce_article")
        .join("scripts")
        .join("horner_distributed_demo.sh");
    assert!(
        script.exists(),
        "horner_distributed_demo.sh must exist at {} (workspace layout drift would silently break IT-0731-11)",
        script.display()
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
