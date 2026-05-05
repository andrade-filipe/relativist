//! ChurchArithmeticCodec — implements the Codec trait for Church numerals.
//!
//! Wraps the existing `build_add`, `build_mul`, `build_exp`, `build_sum_of_squares`
//! encoders (SPEC-14) and the `decode_nat_or_shared` decoder. Zero changes to
//! the existing public API of `arithmetic.rs` / `church.rs` (SPEC-27 R7).
//!
//! The registry (Phase 4) instantiates four codecs, one per operation:
//! `church_add`, `church_mul`, `church_exp`, `church_sum_of_squares` (SPEC-27 R19).

use serde::{Deserialize, Serialize};

use super::arithmetic::{
    build_add, build_exp, build_mul, build_sum_of_squares, decode_nat_or_shared,
};
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
}
