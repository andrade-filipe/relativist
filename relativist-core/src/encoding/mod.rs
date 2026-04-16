//! Arithmetic encoding layer: Church numerals, arithmetic combinators, decoding (SPEC-14).
//!
//! This module bridges abstract IC net reduction and practical computation by
//! encoding natural numbers as Church numerals and arithmetic operations as
//! IC net constructions. When reduced, these nets compute real arithmetic.
//!
//! - `church`: Church numeral encoding (`encode_nat`, `encode_church_into`) and
//!   decoding/readback (`decode_nat`)
//! - `arithmetic`: Arithmetic operation combinators (`build_add`, `build_mul`, `build_exp`)

pub mod arithmetic;
pub mod church;
pub mod codec_church;
pub mod traits;

pub use arithmetic::{
    build_add, build_exp, build_mul, build_sum_of_squares, compute_arithmetic,
    decode_nat_or_shared, discover_root,
};
pub use church::{decode_nat, encode_church_into, encode_nat};
pub use codec_church::{ChurchArithmeticCodec, ChurchOp};
pub use traits::{validate_encoded_net, Codec, DecodeError, Decoder, EncodeError, Encoder};
