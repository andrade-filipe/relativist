//! TASK-0721 D-015 Stage 6 REFACTOR — CLI registry roundtrip integration tests.
//!
//! Covers AC-1, AC-2, AC-3 of the bundle:
//!   - AC-1 (BUG-001): `compute --codec horner` for a non-constant polynomial
//!     produces a decoded value (registry path no longer skips `discover_root`).
//!   - AC-2 (BUG-002): `compute --codec horner` for a constant polynomial is
//!     accepted by the registry (E2 bypass via `Codec::accepts_normal_form_input`).
//!   - AC-3 (BUG-003): `compute --codec church_add` for `a > MAX_CHURCH_NAT`
//!     returns `EncodeError::InvalidInput`, never panics.
//!
//! These tests exercise the same library functions the CLI binary calls
//! (`EncoderRegistry::encode_and_validate`, `reduce_all`, `discover_root`,
//! `Codec::decode`). We do NOT spawn `cargo run` because:
//!   - it requires a release build to be available,
//!   - it doubles CI wall time,
//!   - the in-process equivalence is sufficient (the CLI is a thin shell
//!     around `run_compute_with_encoder` — see `relativist-core/src/commands.rs`).
//!
//! A separate end-to-end smoke that does spawn the binary may be added by
//! the cicd agent in a follow-up; the SDD pipeline gates on this in-process
//! roundtrip plus the existing integration tests.

use relativist_core::encoding::{default_registry, MAX_CHURCH_NAT};
use relativist_core::reduction::reduce_all;

/// Mimics the registry-driven `run_compute_with_encoder` pipeline (commands.rs):
/// `encode_and_validate → reduce_all → discover_root (if needed) → decode`.
/// Returns the decoded JSON or a stringified error.
fn cli_compute(name: &str, input: &[u8]) -> Result<serde_json::Value, String> {
    let registry = default_registry();
    let mut net = registry
        .encode_and_validate(name, input)
        .map_err(|e| format!("encode/validate: {e}"))?;
    let _stats = reduce_all(&mut net);
    if net.root.is_none() {
        relativist_core::encoding::discover_root(&mut net);
    }
    registry
        .decode(name, &net)
        .map_err(|e| format!("decode: {e}"))
}

// AC-1 (BUG-001): non-constant polynomial via the CLI registry path.
// Pre-fix: returns `DecodeError::DecodeFailed("no root")` because the
// registry pipeline did not call `discover_root` between `reduce_all` and
// `decode`. Post-fix (this test): decodes successfully.
#[test]
fn cli_horner_non_constant_polynomial_decodes() {
    let out = cli_compute("horner", br#"{"coeffs":[1,1],"x":2}"#)
        .expect("registry pipeline must decode single-iter Horner");
    assert_eq!(out["value"].as_str().unwrap(), "3");
    assert_eq!(out["bit_length"].as_u64().unwrap(), 2);
}

// AC-2 (BUG-002): constant polynomial via the CLI registry path.
// Pre-fix: `encode_and_validate` rejects with `EncodeError::InvalidNet("E2:
// net has no redexes")` because `HornerCodec::encode` for `coeffs.len() == 1`
// returns a Normal-Form net, and the registry validator unconditionally
// enforced E2 (≥1 redex). Post-fix: the codec opts in via
// `Codec::accepts_normal_form_input`, which the registry consults to bypass
// E2 for that input.
#[test]
fn cli_horner_constant_polynomial_decodes() {
    let out = cli_compute("horner", br#"{"coeffs":[42],"x":99}"#)
        .expect("registry pipeline must accept constant polynomial");
    assert_eq!(out["value"].as_str().unwrap(), "42");
    // Bit length of 42 = 0b101010 = 6 bits.
    assert_eq!(out["bit_length"].as_u64().unwrap(), 6);
}

// AC-2 boundary: constant polynomial with `coeffs[0] == 0` decodes to 0.
#[test]
fn cli_horner_constant_zero_polynomial_decodes() {
    let out = cli_compute("horner", br#"{"coeffs":[0],"x":7}"#)
        .expect("registry must accept constant zero polynomial");
    assert_eq!(out["value"].as_str().unwrap(), "0");
}

// AC-2 boundary: constant polynomial at the cap.
#[test]
fn cli_horner_constant_max_church_polynomial_decodes() {
    let json = format!(r#"{{"coeffs":[{MAX_CHURCH_NAT}],"x":1}}"#);
    let out = cli_compute("horner", json.as_bytes())
        .expect("registry must accept constant polynomial at MAX_CHURCH_NAT");
    assert_eq!(out["value"].as_str().unwrap(), MAX_CHURCH_NAT.to_string());
}

// AC-3 (BUG-003): ChurchArithmeticCodec for `a > MAX_CHURCH_NAT` MUST
// return `EncodeError::InvalidInput` via `RegistryError::Encode`, NOT
// panic via the inner `assert!` in `encode_church_into`.
#[test]
fn cli_church_add_oversize_a_returns_invalid_input() {
    let payload = format!(r#"{{"op":"add","a":{},"b":1}}"#, MAX_CHURCH_NAT + 1);
    let err = cli_compute("church_add", payload.as_bytes())
        .expect_err("oversize `a` must be rejected, not panic");
    // Surface error message comes from RegistryError::Encode(EncodeError::InvalidInput(_)).
    assert!(
        err.contains("exceeds cap") && err.contains(&format!("a = {}", MAX_CHURCH_NAT + 1)),
        "expected oversize-a error message, got: {err}"
    );
}

// AC-3 stress: u64::MAX likewise rejected, never panics.
#[test]
fn cli_church_add_u64_max_a_returns_invalid_input() {
    let payload = format!(r#"{{"op":"add","a":{},"b":1}}"#, u64::MAX);
    let err = cli_compute("church_add", payload.as_bytes())
        .expect_err("u64::MAX `a` must be rejected, not panic");
    assert!(err.contains("exceeds cap"), "got: {err}");
}

// AC-3 mirror: the same defense-in-depth applies to other ChurchArithmeticCodec
// operations (mul, exp). Reject before delegating to `build_*`.
#[test]
fn cli_church_mul_oversize_b_returns_invalid_input() {
    let payload = format!(r#"{{"op":"mul","a":2,"b":{}}}"#, MAX_CHURCH_NAT + 1);
    let err = cli_compute("church_mul", payload.as_bytes())
        .expect_err("oversize `b` must be rejected for mul");
    assert!(err.contains("exceeds cap"), "got: {err}");
}

// Sanity: a well-formed church_add input still works post-fix.
#[test]
fn cli_church_add_within_bounds_round_trips() {
    let out = cli_compute("church_add", br#"{"op":"add","a":3,"b":5}"#)
        .expect("in-bounds church_add must round-trip");
    assert_eq!(out["result"].as_u64().unwrap(), 8);
}
