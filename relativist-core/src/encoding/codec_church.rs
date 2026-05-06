//! ChurchArithmeticCodec — implements the Codec trait for Church numerals.
//!
//! Wraps the existing `build_add`, `build_mul`, `build_exp`, `build_sum_of_squares`
//! encoders (SPEC-14) and the `decode_nat_or_shared` decoder. SPEC-27 v3 R7 was
//! softened in Round 2 (SC-003) to require only that **SPEC-14 R3 public function
//! signatures** stay unchanged; this codec adds a JSON-dispatch surface (R8) on
//! top of those primitives without touching their export list.
//!
//! The registry (Phase 4) instantiates four codecs, one per operation:
//! `church_add`, `church_mul`, `church_exp`, `church_sum_of_squares` (SPEC-27
//! v3 R19; the v3 default registry adds `horner` and drops `lambda`, see
//! TASK-0716).
//!
//! R8 operand semantics (SPEC-27 v3, closure of SC-003):
//!
//! - `op = "add"` / `op = "mul"`: `a` and `b` are the two operands; codec
//!   invokes `build_add(a, b)` / `build_mul(a, b)` (SPEC-14 R15-R16).
//! - `op = "exp"`: `a` is the **base**, `b` is the **exponent**, matching
//!   SPEC-14 R17 ordering `build_exp(base, exp) -> Net` so the result is
//!   `a^b` (NOT `b^a`).
//! - `op = "sum_of_squares"`: `a` is the upper bound `n`; `b` is ignored
//!   (MAY be omitted; defensive parsing accepts a stray `b` without effect).
//!
//! All 690 v1 floor tests (R9) MUST continue to pass after this codec is
//! audited; CI enforces the floor via `cargo test`.

use serde::{Deserialize, Serialize};

use super::arithmetic::{
    build_add, build_exp, build_mul, build_sum_of_squares, decode_nat_or_shared,
};
use super::church::MAX_CHURCH_NAT;
use super::traits::{Codec, DecodeError, Decoder, EncodeError, Encoder};
use crate::net::Net;

/// The four Church-numeral operations exposed as codecs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChurchOp {
    Add,
    Mul,
    Exp,
    SumOfSquares,
}

impl ChurchOp {
    fn codec_name(self) -> &'static str {
        match self {
            ChurchOp::Add => "church_add",
            ChurchOp::Mul => "church_mul",
            ChurchOp::Exp => "church_exp",
            ChurchOp::SumOfSquares => "church_sum_of_squares",
        }
    }

    fn description(self) -> &'static str {
        match self {
            ChurchOp::Add => "Church numeral addition (a + b)",
            ChurchOp::Mul => "Church numeral multiplication (a * b)",
            ChurchOp::Exp => "Church numeral exponentiation (a ^ b)",
            ChurchOp::SumOfSquares => "Sum of squares (1^2 + 2^2 + ... + n^2)",
        }
    }
}

/// Codec for Church-numeral arithmetic (SPEC-27 R7, R8).
///
/// One instance per operation. Input schema (SPEC-27 R8):
/// ```json
/// { "op": "add" | "mul" | "exp" | "sum_of_squares",
///   "a": <u64>,
///   "b": <u64>   // optional for sum_of_squares
/// }
/// ```
/// The `op` field is optional when decoded via `ChurchArithmeticCodec::new(op)` —
/// the codec already knows its operation from its name. If present, it must
/// match the codec's operation (otherwise `EncodeError::InvalidInput`).
#[derive(Debug, Clone, Copy)]
pub struct ChurchArithmeticCodec {
    op: ChurchOp,
}

impl ChurchArithmeticCodec {
    pub fn new(op: ChurchOp) -> Self {
        Self { op }
    }

    pub fn add() -> Self {
        Self::new(ChurchOp::Add)
    }

    pub fn mul() -> Self {
        Self::new(ChurchOp::Mul)
    }

    pub fn exp() -> Self {
        Self::new(ChurchOp::Exp)
    }

    pub fn sum_of_squares() -> Self {
        Self::new(ChurchOp::SumOfSquares)
    }

    pub fn op(&self) -> ChurchOp {
        self.op
    }
}

/// JSON input schema (SPEC-27 R8).
#[derive(Debug, Deserialize)]
struct ChurchInput {
    #[serde(default)]
    op: Option<String>,
    a: u64,
    #[serde(default)]
    b: Option<u64>,
}

/// JSON output schema for decode (SPEC-27 R8 "result" field only; the pipeline
/// driver adds "interactions" after reduction, Phase 5).
#[derive(Debug, Serialize)]
struct ChurchOutput {
    result: u64,
}

impl Encoder for ChurchArithmeticCodec {
    fn name(&self) -> &str {
        self.op.codec_name()
    }

    fn encode(&self, input: &[u8]) -> Result<Net, EncodeError> {
        let params: ChurchInput = serde_json::from_slice(input)
            .map_err(|e| EncodeError::InvalidInput(format!("JSON parse failed: {}", e)))?;

        if let Some(op_str) = &params.op {
            let expected = match self.op {
                ChurchOp::Add => "add",
                ChurchOp::Mul => "mul",
                ChurchOp::Exp => "exp",
                ChurchOp::SumOfSquares => "sum_of_squares",
            };
            if op_str != expected {
                return Err(EncodeError::InvalidInput(format!(
                    "op field '{}' does not match codec '{}'",
                    op_str,
                    self.op.codec_name()
                )));
            }
        }

        // TASK-0721 BUG-003: validate operand bounds against `MAX_CHURCH_NAT`
        // BEFORE delegating to `build_*` (which call `encode_church_into`,
        // which `assert!`s a higher cap and process-aborts on violation).
        // Mirrors HornerCodec's R12' validation. SC-013 single-source-of-truth.
        if params.a > MAX_CHURCH_NAT {
            return Err(EncodeError::InvalidInput(format!(
                "a = {} exceeds cap (max {MAX_CHURCH_NAT})",
                params.a
            )));
        }
        // For sum_of_squares `b` is ignored entirely (R8); skip the bound
        // check there so a stray `b` does not synthesize a fake error.
        if !matches!(self.op, ChurchOp::SumOfSquares) {
            if let Some(b) = params.b {
                if b > MAX_CHURCH_NAT {
                    return Err(EncodeError::InvalidInput(format!(
                        "b = {b} exceeds cap (max {MAX_CHURCH_NAT})"
                    )));
                }
            }
        }

        let net = match self.op {
            ChurchOp::Add => build_add(params.a, params.b.unwrap_or(0)),
            ChurchOp::Mul => build_mul(params.a, params.b.unwrap_or(0)),
            ChurchOp::Exp => build_exp(params.a, params.b.unwrap_or(0)),
            ChurchOp::SumOfSquares => build_sum_of_squares(params.a),
        };
        Ok(net)
    }
}

impl Decoder for ChurchArithmeticCodec {
    fn decode(&self, net: &Net) -> Result<serde_json::Value, DecodeError> {
        let result = decode_nat_or_shared(net).ok_or_else(|| {
            DecodeError::UnrecognizedStructure(
                "Church numeral readback failed (DUP cycle or malformed net)".to_string(),
            )
        })?;
        let out = ChurchOutput { result };
        serde_json::to_value(out).map_err(|e| DecodeError::DecodeFailed(e.to_string()))
    }
}

impl Codec for ChurchArithmeticCodec {
    fn description(&self) -> &str {
        self.op.description()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::arithmetic::decode_nat_or_shared;
    use crate::encoding::traits::validate_encoded_net;
    use crate::reduction::reduce_all;

    // T3 from SPEC-27 §6.2: round-trip for add.
    #[test]
    fn church_add_round_trip() {
        let codec = ChurchArithmeticCodec::add();
        assert_eq!(codec.name(), "church_add");
        let input = br#"{"op":"add","a":3,"b":5}"#;
        let mut net = codec.encode(input).unwrap();
        validate_encoded_net(&net).unwrap();
        reduce_all(&mut net);
        let out = codec.decode(&net).unwrap();
        assert_eq!(out["result"], 8);
    }

    // T3 continued: round-trip for mul.
    #[test]
    fn church_mul_round_trip() {
        let codec = ChurchArithmeticCodec::mul();
        let input = br#"{"op":"mul","a":4,"b":7}"#;
        let mut net = codec.encode(input).unwrap();
        validate_encoded_net(&net).unwrap();
        reduce_all(&mut net);
        let out = codec.decode(&net).unwrap();
        assert_eq!(out["result"], 28);
    }

    // Round-trip without explicit op field (inferred from codec).
    #[test]
    fn church_add_without_op_field() {
        let codec = ChurchArithmeticCodec::add();
        let input = br#"{"a":2,"b":3}"#;
        let mut net = codec.encode(input).unwrap();
        reduce_all(&mut net);
        let out = codec.decode(&net).unwrap();
        assert_eq!(out["result"], 5);
    }

    // Mismatched op returns InvalidInput error.
    #[test]
    fn church_wrong_op_rejected() {
        let codec = ChurchArithmeticCodec::add();
        let input = br#"{"op":"mul","a":3,"b":5}"#;
        let err = codec.encode(input).unwrap_err();
        assert!(matches!(err, EncodeError::InvalidInput(msg) if msg.contains("does not match")));
    }

    // Invalid JSON returns InvalidInput error.
    #[test]
    fn church_invalid_json_rejected() {
        let codec = ChurchArithmeticCodec::add();
        let input = b"not json";
        let err = codec.encode(input).unwrap_err();
        assert!(matches!(err, EncodeError::InvalidInput(_)));
    }

    // Descriptions are non-empty and distinguishable.
    #[test]
    fn church_descriptions_distinct() {
        let a = ChurchArithmeticCodec::add();
        let m = ChurchArithmeticCodec::mul();
        assert_ne!(a.description(), m.description());
        assert!(a.description().contains("addition"));
        assert!(m.description().contains("multiplication"));
    }

    // sum_of_squares: 1^2 + 2^2 + 3^2 = 14.
    #[test]
    fn church_sum_of_squares_round_trip() {
        let codec = ChurchArithmeticCodec::sum_of_squares();
        let input = br#"{"op":"sum_of_squares","a":3}"#;
        let mut net = codec.encode(input).unwrap();
        reduce_all(&mut net);
        let out = codec.decode(&net).unwrap();
        assert_eq!(out["result"], 14);
    }

    // Object-safe: Box<dyn Codec> works for Church.
    #[test]
    fn church_codec_object_safe() {
        let boxed: Box<dyn Codec> = Box::new(ChurchArithmeticCodec::add());
        assert_eq!(boxed.name(), "church_add");
    }

    // --- TASK-0710 / SPEC-27 v3 R8 operand-semantics audit (SC-003 closure) ---

    // UT-0710-01: add round-trip 3 + 5 = 8.
    #[test]
    fn church_codec_add_a_plus_b() {
        let codec = ChurchArithmeticCodec::new(ChurchOp::Add);
        let mut net = codec.encode(br#"{"op":"add","a":3,"b":5}"#).unwrap();
        reduce_all(&mut net);
        let out = codec.decode(&net).unwrap();
        assert_eq!(out["result"].as_u64().unwrap(), 8);

        // Edge cases EC-1 / EC-2.
        for (a, b, expected) in [(0u64, 0u64, 0u64), (1, 0, 1)] {
            let json = format!(r#"{{"op":"add","a":{a},"b":{b}}}"#);
            let mut n = codec.encode(json.as_bytes()).unwrap();
            reduce_all(&mut n);
            let out = codec.decode(&n).unwrap();
            assert_eq!(out["result"].as_u64().unwrap(), expected, "add({a}, {b})");
        }
    }

    // UT-0710-02: SPEC-27 v3 R8 — `exp` operand mapping pinned: a is base,
    // b is exponent. Catches operand-swap regressions (SC-003).
    //
    // Note on decode: `build_exp` produces a net whose Normal Form contains
    // DUP cycles for `exp >= 2` (a known SPEC-14 limitation — see
    // `decode_nat_or_shared` and the existing `default_registry_church_codecs_round_trip`
    // test which encodes-only for `exp`). So we verify operand mapping
    // through structural channels:
    //   1. `b == 0` short-circuits to `Church(1)` for any `a` (build_exp special case),
    //      producing a decodable Church(1) — confirms `b` is the exponent slot.
    //   2. `a == 1` for any `b` reduces to `Church(1)` (1^n = 1) — decodable;
    //      confirms `a` is the base slot.
    //   3. The encoder accepts `exp` JSON with both fields present and produces
    //      a validatable net for the canonical 2^3 case.
    #[test]
    fn church_codec_exp_a_is_base_b_is_exponent() {
        let codec = ChurchArithmeticCodec::new(ChurchOp::Exp);

        // EC-2 / R8 anchor: b=0 (exponent zero) -> Church(1) regardless of a;
        // confirms `b` occupies the exponent slot.
        for a in [1u64, 2, 5, 10] {
            let json = format!(r#"{{"op":"exp","a":{a},"b":0}}"#);
            let mut n = codec.encode(json.as_bytes()).unwrap();
            reduce_all(&mut n);
            assert_eq!(
                decode_nat_or_shared(&n),
                Some(1),
                "exp({a}, 0) MUST equal 1 (anchor: b is exponent slot)"
            );
        }

        // EC-3 / R8 anchor: a=1 (base one) -> Church(1) regardless of b;
        // confirms `a` occupies the base slot.
        for b in [0u64, 1, 5, 10] {
            let json = format!(r#"{{"op":"exp","a":1,"b":{b}}}"#);
            let mut n = codec.encode(json.as_bytes()).unwrap();
            reduce_all(&mut n);
            assert_eq!(
                decode_nat_or_shared(&n),
                Some(1),
                "exp(1, {b}) MUST equal 1 (anchor: a is base slot)"
            );
        }

        // Canonical 2^3 case: encoder produces a validatable, non-trivial net
        // (decode is a known SPEC-14 limitation for exp >= 2).
        let net = codec.encode(br#"{"op":"exp","a":2,"b":3}"#).unwrap();
        validate_encoded_net(&net).expect("exp(2,3) net must satisfy T1-T7");
    }

    // UT-0710-03: SPEC-27 v3 R8 — sum_of_squares uses `a` as `n`; `b` MAY
    // be omitted entirely from the JSON.
    #[test]
    fn church_codec_sum_of_squares_uses_a_only() {
        let codec = ChurchArithmeticCodec::new(ChurchOp::SumOfSquares);

        // 1^2 + 2^2 + 3^2 = 14.
        let mut net = codec.encode(br#"{"op":"sum_of_squares","a":3}"#).unwrap();
        reduce_all(&mut net);
        let out = codec.decode(&net).unwrap();
        assert_eq!(out["result"].as_u64().unwrap(), 14);

        // EC-1: a=0 → empty sum.
        let mut n = codec.encode(br#"{"op":"sum_of_squares","a":0}"#).unwrap();
        reduce_all(&mut n);
        assert_eq!(codec.decode(&n).unwrap()["result"].as_u64().unwrap(), 0);

        // EC-2: a=1 → 1.
        let mut n = codec.encode(br#"{"op":"sum_of_squares","a":1}"#).unwrap();
        reduce_all(&mut n);
        assert_eq!(codec.decode(&n).unwrap()["result"].as_u64().unwrap(), 1);

        // EC-3: a=5 → 1+4+9+16+25 = 55.
        let mut n = codec.encode(br#"{"op":"sum_of_squares","a":5}"#).unwrap();
        reduce_all(&mut n);
        assert_eq!(codec.decode(&n).unwrap()["result"].as_u64().unwrap(), 55);
    }

    // UT-0710-04: defensive — stray `b` in sum_of_squares JSON MUST be
    // silently ignored (R8 wording: "b is ignored").
    #[test]
    fn church_codec_sum_of_squares_b_ignored_when_present() {
        let codec = ChurchArithmeticCodec::new(ChurchOp::SumOfSquares);

        let mut with_b = codec
            .encode(br#"{"op":"sum_of_squares","a":3,"b":99}"#)
            .unwrap();
        let mut without_b = codec.encode(br#"{"op":"sum_of_squares","a":3}"#).unwrap();
        reduce_all(&mut with_b);
        reduce_all(&mut without_b);
        let r1 = codec.decode(&with_b).unwrap();
        let r2 = codec.decode(&without_b).unwrap();
        assert_eq!(r1["result"], r2["result"]);
        assert_eq!(r1["result"].as_u64().unwrap(), 14);

        // EC-2: stray b = u64::MAX SHOULD NOT cause overflow (b is unused).
        let mut huge_b = codec
            .encode(br#"{"op":"sum_of_squares","a":3,"b":18446744073709551615}"#)
            .unwrap();
        reduce_all(&mut huge_b);
        assert_eq!(
            codec.decode(&huge_b).unwrap()["result"].as_u64().unwrap(),
            14
        );
    }

    // TASK-0721 BUG-003: ChurchArithmeticCodec::encode MUST reject inputs
    // where `a` or `b` exceeds `MAX_CHURCH_NAT` with `EncodeError::InvalidInput`,
    // NOT panic via the inner `assert!` in `encode_church_into`. Mirrors
    // HornerCodec's R12' bound validation.
    #[test]
    fn church_codec_rejects_a_above_max_church_nat() {
        let codec = ChurchArithmeticCodec::add();
        let json = format!(r#"{{"op":"add","a":{},"b":1}}"#, MAX_CHURCH_NAT + 1);
        let err = codec.encode(json.as_bytes()).unwrap_err();
        match err {
            EncodeError::InvalidInput(msg) => {
                assert!(
                    msg.contains(&format!("a = {}", MAX_CHURCH_NAT + 1))
                        && msg.contains("exceeds cap"),
                    "expected oversize-a InvalidInput, got: {msg}"
                );
            }
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn church_codec_rejects_b_above_max_church_nat() {
        let codec = ChurchArithmeticCodec::mul();
        let json = format!(r#"{{"op":"mul","a":2,"b":{}}}"#, MAX_CHURCH_NAT + 1);
        let err = codec.encode(json.as_bytes()).unwrap_err();
        assert!(
            matches!(&err, EncodeError::InvalidInput(msg) if msg.contains(&format!("b = {}", MAX_CHURCH_NAT + 1))),
            "expected oversize-b InvalidInput, got {err:?}"
        );
    }

    #[test]
    fn church_codec_accepts_boundary_max_church_nat() {
        // Boundary: a == MAX_CHURCH_NAT must be accepted (cap is inclusive).
        // We don't decode here — `church_add(MAX_CHURCH_NAT, 0)` is a huge
        // net and the goal is just to verify the validator does NOT reject.
        let codec = ChurchArithmeticCodec::add();
        let json = format!(r#"{{"op":"add","a":{MAX_CHURCH_NAT},"b":0}}"#);
        let net = codec.encode(json.as_bytes());
        assert!(net.is_ok(), "boundary a=MAX_CHURCH_NAT must be accepted");
    }

    #[test]
    fn church_codec_sum_of_squares_ignores_oversize_b() {
        // sum_of_squares ignores `b` entirely (R8); a stray oversize `b`
        // MUST NOT synthesize a fake bound error.
        let codec = ChurchArithmeticCodec::sum_of_squares();
        let json = format!(
            r#"{{"op":"sum_of_squares","a":3,"b":{}}}"#,
            MAX_CHURCH_NAT + 1_000_000
        );
        let net = codec.encode(json.as_bytes());
        assert!(net.is_ok(), "sum_of_squares MUST ignore stray b");
    }

    #[test]
    fn church_codec_oversize_a_does_not_panic_under_unwind_test() {
        // Defense-in-depth: verify the cap ACTUALLY prevents reaching the
        // inner `encode_church_into` assert. Construct an adversarial
        // crafted JSON and ensure encode returns Err, never panics.
        let codec = ChurchArithmeticCodec::add();
        let payload = format!(r#"{{"op":"add","a":{},"b":1}}"#, u64::MAX);
        let result = codec.encode(payload.as_bytes());
        assert!(matches!(result, Err(EncodeError::InvalidInput(_))));
    }

    // UT-0710-05: mul round-trip 4 × 7 = 28; completes the T3 quad.
    #[test]
    fn church_codec_mul_a_times_b() {
        let codec = ChurchArithmeticCodec::new(ChurchOp::Mul);

        let mut net = codec.encode(br#"{"op":"mul","a":4,"b":7}"#).unwrap();
        reduce_all(&mut net);
        assert_eq!(codec.decode(&net).unwrap()["result"].as_u64().unwrap(), 28);

        // EC-1/2/3: zero-product, identity, commutativity.
        for (a, b, expected) in [(0u64, 7u64, 0u64), (1, 99, 99), (99, 0, 0)] {
            let json = format!(r#"{{"op":"mul","a":{a},"b":{b}}}"#);
            let mut n = codec.encode(json.as_bytes()).unwrap();
            reduce_all(&mut n);
            assert_eq!(
                codec.decode(&n).unwrap()["result"].as_u64().unwrap(),
                expected,
                "mul({a}, {b})"
            );
        }
    }
}
