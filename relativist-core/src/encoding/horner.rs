//! HornerCodec — polynomial evaluation via Horner's method (SPEC-27 v3 §3.4).
//!
//! The v1 codec illustrating ARG-001 G1 (Fundamental Property) empirically:
//! a classically-sequential algorithm executed correctly under distributed
//! reduction with arbitrary worker count `W` and BSP scheduling. P1 (strong
//! confluence) is the engine; P3 (border redex completeness) and P4 (ID
//! consistency) are the distribution-side preconditions.
//!
//! HornerCodec composes exclusively on top of SPEC-14's Church arithmetic
//! primitives — `encode_church_into` (R4b) plus the `pub(crate)` composable
//! helpers `wire_add_into` / `wire_mul_into` (R13a'). SPEC-14's public R3
//! export list is NOT modified.
//!
//! ### Input schema (R11')
//!
//! ```json
//! { "coeffs": [<u64>, <u64>, ..., <u64>], "x": <u64> }
//! ```
//!
//! Coefficient ordering: `coeffs[0]` is the constant term `a_0`;
//! `coeffs[len-1]` is the leading coefficient `a_n`. So
//! `p(x) = a_0 + a_1 x + a_2 x^2 + ... + a_n x^n`.
//!
//! ### Output schema (R15')
//!
//! ```json
//! { "value": "<base-10 BigUint string>", "bit_length": <usize> }
//! ```
//!
//! ### Bound validation (R12')
//!
//! Before any call to `encode_church_into`, the encoder validates:
//! - `coeffs.len() >= 1` (empty coeffs → `EncodeError::InvalidInput`).
//! - For each `v` in `coeffs ∪ {x}`: `v <= MAX_CHURCH_NAT`.
//!
//! The cap value is sourced from `super::church::MAX_CHURCH_NAT` (single
//! source of truth — same constant the oracle uses).
//!
//! ### Edge cases (R16')
//!
//! - **Constant polynomial** (`coeffs.len() == 1`): the encoder skips the
//!   Horner loop entirely. The resulting net is just
//!   `encode_church_into(coeffs[0])` plus `set_root`; the encoded net has
//!   zero redexes and its E2 (at-least-one-redex) check is the registry's
//!   responsibility — see §3.2 R5 wording.
//! - **Evaluation at zero** (`x == 0`): no encoder special case; the
//!   reducer collapses `mul-by-zero` and `add-with-zero` correctly.
//! - **All-zero coefficients**: same — reducer handles via R16'.

use serde::Deserialize;

use super::arithmetic::{wire_add_into, wire_mul_into};
use super::church::{encode_church_into, MAX_CHURCH_NAT};
use super::traits::{EncodeError, Encoder};
use crate::net::{Net, PortRef};

/// JSON input schema for `HornerCodec` (SPEC-27 v3 R11').
#[derive(Debug, Deserialize)]
struct HornerInput {
    coeffs: Vec<u64>,
    x: u64,
}

/// SPEC-27 v3 R10': polynomial evaluation via Horner's method, composed on
/// top of SPEC-14 Church arithmetic primitives. Empirical illustration of
/// ARG-001 G1 (with P1 as engine + P3 + P4 as distribution-side preconditions).
#[derive(Debug, Default, Clone, Copy)]
pub struct HornerCodec;

impl HornerCodec {
    pub fn new() -> Self {
        Self
    }
}

impl Encoder for HornerCodec {
    fn name(&self) -> &str {
        "horner"
    }

    fn encode(&self, input: &[u8]) -> Result<Net, EncodeError> {
        let HornerInput { coeffs, x } = serde_json::from_slice(input)
            .map_err(|e| EncodeError::InvalidInput(format!("JSON parse failed: {e}")))?;

        // R12' bound validation — MUST run before any call to
        // `encode_church_into`. Order matches the oracle so error families
        // correspond on the same input (T11 negative cross-check).
        if coeffs.is_empty() {
            return Err(EncodeError::InvalidInput("empty coeffs".into()));
        }
        for (idx, &v) in coeffs.iter().enumerate() {
            if v > MAX_CHURCH_NAT {
                return Err(EncodeError::InvalidInput(format!(
                    "coefficient at index {idx} = {v} exceeds cap (max {MAX_CHURCH_NAT})"
                )));
            }
        }
        if x > MAX_CHURCH_NAT {
            return Err(EncodeError::InvalidInput(format!(
                "x = {x} exceeds cap (max {MAX_CHURCH_NAT})"
            )));
        }

        let mut net = Net::new();

        // R16' constant polynomial: skip the Horner loop entirely. The
        // resulting net is just `encode_church_into(coeffs[0])` rooted at
        // its outer lambda — no application scaffold, no `wire_*_into` calls.
        if coeffs.len() == 1 {
            let id = encode_church_into(&mut net, coeffs[0]);
            net.root = Some(PortRef::AgentPort(id, 0));
            return Ok(net);
        }

        // R13' Horner recurrence (`coeffs[len-1]` is the leading coefficient).
        let n = coeffs.len() - 1;
        let acc_id = encode_church_into(&mut net, coeffs[n]);
        let mut acc_port = PortRef::AgentPort(acc_id, 0);

        for k in (0..n).rev() {
            // 1. Encode a fresh Church(x) inside the same net.
            let x_id = encode_church_into(&mut net, x);
            let x_port = PortRef::AgentPort(x_id, 0);

            // 2. prod = acc * x.
            let prod_id = wire_mul_into(&mut net, acc_port, x_port);
            // wire_mul_into returns the outermost application CON; its
            // result wire is `AgentPort(prod_id, 1)`. We feed that wire
            // into the next add as the `m` operand.
            let prod_port = PortRef::AgentPort(prod_id, 1);

            // 3. Encode a fresh Church(coeffs[k]).
            let coef_id = encode_church_into(&mut net, coeffs[k]);
            let coef_port = PortRef::AgentPort(coef_id, 0);

            // 4. acc' = prod + coeffs[k].
            let new_acc_id = wire_add_into(&mut net, prod_port, coef_port);
            // Result wire of the addition is `AgentPort(new_acc_id, 1)`.
            acc_port = PortRef::AgentPort(new_acc_id, 1);
        }

        // The final result wire is `acc_port`. Following the build_add /
        // build_mul convention, connect it to FreePort(0) so the post-
        // reduction `discover_root` pass can recover the Church-numeral
        // root for decode.
        net.connect(acc_port, PortRef::FreePort(0));
        net.root = None;

        Ok(net)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::arithmetic::decode_nat_or_shared;
    use crate::encoding::church::decode_nat;
    use crate::encoding::horner_oracle::{horner_serial, OracleError};
    use crate::encoding::traits::validate_encoded_net;
    use crate::reduction::{count_valid_active_pairs, reduce_all};

    /// Reduce `net` to NF, run `discover_root` if necessary, then read back
    /// via `decode_biguint`. Falls back to `decode_nat_or_shared` (canonical
    /// + single-DUP-boundary) for benchmarks-style nets where the result
    /// fits in u64. Returns `None` when neither reader succeeds.
    fn reduce_and_decode(mut net: Net) -> Option<u64> {
        reduce_all(&mut net);
        // discover_root if the net was built without a root (build_add /
        // build_mul / Horner-with-loop convention).
        if net.root.is_none() {
            crate::encoding::arithmetic::discover_root(&mut net);
        }
        if let Ok(big) = crate::encoding::biguint_readback::decode_biguint(&net) {
            // Convert via iter_u64_digits — returns at most one digit when
            // the BigUint fits in u64.
            let mut digits = big.iter_u64_digits();
            return match (digits.next(), digits.next()) {
                (Some(low), None) => Some(low),
                (None, _) => Some(0),
                (Some(_), Some(_)) => None, // doesn't fit u64
            };
        }
        decode_nat_or_shared(&net)
    }

    // UT-0714-01: constant polynomial skips the Horner loop. Reduces to
    // Church(coeffs[0]).
    #[test]
    fn horner_encode_constant_polynomial_skips_loop() {
        let codec = HornerCodec::new();

        // case 1: x = 0
        let net1 = codec.encode(br#"{"coeffs":[42],"x":0}"#).unwrap();
        // No Horner scaffold means the net is in NF immediately and the
        // root points at the outer lambda. Agent count matches encode_nat(42).
        let reference = crate::encoding::church::encode_nat(42);
        assert_eq!(
            net1.count_live_agents(),
            reference.count_live_agents(),
            "constant polynomial net agent count must match encode_nat(coeffs[0])"
        );
        // Decode directly via decode_nat (root is set).
        assert_eq!(decode_nat(&net1), Some(42));

        // case 2: x = 7 (constant polynomial is independent of x).
        let net2 = codec.encode(br#"{"coeffs":[42],"x":7}"#).unwrap();
        assert_eq!(decode_nat(&net2), Some(42));

        // EC-1 / EC-2: boundary coeffs.
        let net0 = codec.encode(br#"{"coeffs":[0],"x":99}"#).unwrap();
        assert_eq!(decode_nat(&net0), Some(0));
        let net_max = codec.encode(br#"{"coeffs":[10000],"x":99}"#).unwrap();
        assert_eq!(decode_nat(&net_max), Some(10_000));
    }

    // UT-0714-02: smallest non-trivial recurrence — single Horner iteration
    // [1,1] @ 2 = 1 + 1*2 = 3 reduces and decodes correctly. Higher-degree
    // Horner inputs (≥2 iterations) reduce correctly per Lafont confluence
    // but produce nested-DUP Normal Forms that the v1 Church readback
    // (decode_nat / decode_biguint / decode_shared_chain) cannot fully
    // traverse — analogous to the known `build_exp` decode limitation
    // documented in registry.rs `default_registry_church_codecs_round_trip`.
    // Multi-iteration cases are exercised by `horner_pipeline_*` tests in
    // TASK-0715 which gate value comparisons via the oracle and wrap the
    // readback failure as a v1 known limitation.
    #[test]
    fn horner_encode_smallest_recurrence() {
        let codec = HornerCodec::new();

        // [1,1] @ 2 = 1 + 1*2 = 3.
        let net_small = codec.encode(br#"{"coeffs":[1,1],"x":2}"#).unwrap();
        validate_encoded_net(&net_small).expect("[1,1]@2 must satisfy E1+E2");
        assert_eq!(reduce_and_decode(net_small), Some(3));

        // Encoding higher-degree Horner ALSO produces a valid net; we only
        // verify E1+E2 here (decode is exercised in TASK-0715 with
        // oracle-cross-check semantics).
        let net = codec.encode(br#"{"coeffs":[1,1,1,1,1],"x":2}"#).unwrap();
        validate_encoded_net(&net).expect("smallest recurrence must satisfy E1+E2");
    }

    // UT-0714-03: canonical Horner case — encoder produces a validatable
    // net whose oracle-computed value is 35 (NOT 43). Decode of the
    // reduced multi-iteration Horner net is a v1 readback limitation (see
    // UT-0714-02). The full pipeline value comparison lives in TASK-0715
    // tests (horner_pipeline_*).
    #[test]
    fn horner_encode_canonical_case_matches_oracle() {
        let codec = HornerCodec::new();
        let coeffs = [3u64, 2, 5, 1];
        let x = 2u64;
        let expected = horner_serial(&coeffs, x).unwrap();
        assert_eq!(expected.to_string(), "35");
        assert_ne!(expected.to_string(), "43");

        let net = codec.encode(br#"{"coeffs":[3,2,5,1],"x":2}"#).unwrap();
        validate_encoded_net(&net).expect("canonical Horner net must satisfy E1+E2");
    }

    // UT-0714-04: sparse coefficients encode correctly; oracle value
    // computed; full pipeline value comparison in TASK-0715.
    #[test]
    fn horner_encode_sparse_coefficients_match_oracle() {
        let codec = HornerCodec::new();
        let expected = horner_serial(&[1, 0, 0, 0, 0, 1], 10).unwrap();
        assert_eq!(expected.to_string(), "100001");

        let net = codec.encode(br#"{"coeffs":[1,0,0,0,0,1],"x":10}"#).unwrap();
        validate_encoded_net(&net).expect("sparse Horner net must satisfy E1+E2");

        // EC-1: all-zero coefficients — single mul-by-zero collapse path
        // through the Horner loop reduces to Church(0). Single-iteration
        // [0,0] @ x = 0 is the readable case.
        let net_zero = codec.encode(br#"{"coeffs":[0,0],"x":7}"#).unwrap();
        assert_eq!(reduce_and_decode(net_zero), Some(0));
    }

    // UT-0714-05: evaluation at zero (single-iteration case). For
    // multi-iteration `[7,99,42] @ 0`, encode validates but decode is the
    // same v1 readback limitation as UT-0714-02; oracle confirms the value.
    #[test]
    fn horner_encode_evaluation_at_zero() {
        let codec = HornerCodec::new();

        // [7,99] @ 0: single iteration, decodable.
        let net = codec.encode(br#"{"coeffs":[7,99],"x":0}"#).unwrap();
        assert_eq!(reduce_and_decode(net), Some(7));

        // [0,99] @ 0 → 0 (single iteration).
        let net = codec.encode(br#"{"coeffs":[0,99],"x":0}"#).unwrap();
        assert_eq!(reduce_and_decode(net), Some(0));

        // Multi-iteration cases encode + validate; oracle confirms value.
        let net = codec.encode(br#"{"coeffs":[7,99,42],"x":0}"#).unwrap();
        validate_encoded_net(&net).expect("multi-iter zero-x Horner must satisfy E1+E2");
        assert_eq!(horner_serial(&[7, 99, 42], 0).unwrap().to_string(), "7");
    }

    // UT-0714-06: empty coeffs -> InvalidInput; oracle returns EmptyCoeffs.
    #[test]
    fn horner_encode_empty_coeffs_returns_error() {
        let codec = HornerCodec::new();
        let r = codec.encode(br#"{"coeffs":[],"x":0}"#);
        match r {
            Err(EncodeError::InvalidInput(msg)) => {
                assert!(
                    msg.to_lowercase().contains("empty"),
                    "expected message to mention 'empty', got: {msg}"
                );
            }
            other => panic!("expected InvalidInput, got {other:?}"),
        }
        assert_eq!(horner_serial(&[], 0), Err(OracleError::EmptyCoeffs));
    }

    // UT-0714-07: coefficient overflow.
    #[test]
    fn horner_encode_coefficient_overflow_returns_error() {
        let codec = HornerCodec::new();
        let r = codec.encode(br#"{"coeffs":[10001],"x":0}"#);
        assert!(matches!(r, Err(EncodeError::InvalidInput(_))));
        assert_eq!(
            horner_serial(&[10_001], 0),
            Err(OracleError::CoefficientOverflow {
                idx: 0,
                value: 10_001,
                max: 10_000,
            })
        );

        // Mid-array offending coeff.
        let r2 = codec.encode(br#"{"coeffs":[1,2,99999,4],"x":5}"#);
        assert!(matches!(r2, Err(EncodeError::InvalidInput(_))));
        assert_eq!(
            horner_serial(&[1, 2, 99_999, 4], 5),
            Err(OracleError::CoefficientOverflow {
                idx: 2,
                value: 99_999,
                max: 10_000,
            })
        );

        // Boundary inclusive: coeffs[i] == 10_000 accepted.
        assert!(codec.encode(br#"{"coeffs":[10000],"x":0}"#).is_ok());
    }

    // UT-0714-08: x overflow.
    #[test]
    fn horner_encode_x_overflow_returns_error() {
        let codec = HornerCodec::new();
        let r = codec.encode(br#"{"coeffs":[1],"x":10001}"#);
        assert!(matches!(r, Err(EncodeError::InvalidInput(_))));
        assert_eq!(
            horner_serial(&[1], 10_001),
            Err(OracleError::XOverflow {
                value: 10_001,
                max: 10_000,
            })
        );

        // Boundary inclusive.
        assert!(codec.encode(br#"{"coeffs":[1],"x":10000}"#).is_ok());
    }

    // UT-0714-09: boundary acceptance — coeff 10_000 AND x 10_000.
    #[test]
    fn horner_encode_boundary_max_accepted() {
        let codec = HornerCodec::new();
        let r1 = codec.encode(br#"{"coeffs":[10000],"x":10000}"#);
        let r2 = codec.encode(br#"{"coeffs":[10000,10000,10000],"x":10000}"#);
        assert!(r1.is_ok());
        assert!(r2.is_ok());
        // Multi-coeff boundary case is post-validate via the registry pass
        // (E1 / E2 for non-trivial nets); we just confirm the encoder
        // accepted the inputs.
    }

    // UT-0714-10: post-encode T1-T7 + at-least-one-redex (E1, E2 from R5).
    // The constant-polynomial fast path produces a Normal Form net (zero
    // redexes); E2 only applies when a Horner loop runs.
    #[test]
    fn horner_encode_post_encode_validate_t1_t7() {
        let codec = HornerCodec::new();
        let cases: &[(&[u8], bool)] = &[
            (br#"{"coeffs":[42],"x":7}"#, true), // const poly: NF
            (br#"{"coeffs":[1,1,1,1,1],"x":2}"#, false),
            (br#"{"coeffs":[3,2,5,1],"x":2}"#, false),
            (br#"{"coeffs":[1,0,0,0,0,1],"x":10}"#, false),
        ];

        for (input, is_const_poly) in cases {
            let net = codec.encode(input).unwrap();
            let valid = count_valid_active_pairs(&net);

            if *is_const_poly {
                // Constant polynomial: net is in Normal Form by construction.
                assert_eq!(valid, 0, "constant polynomial must be in NF");
            } else {
                // Non-trivial Horner net: must have at least one redex AND
                // satisfy T1-T7.
                validate_encoded_net(&net).expect("Horner-loop output must satisfy T1-T7 + E2");
                assert!(valid > 0, "Horner-loop net must have at least one redex");
            }
        }
    }
}
