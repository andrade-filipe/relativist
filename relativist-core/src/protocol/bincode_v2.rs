//! Bincode v2 thin wrappers (SPEC-18 §3.1).
//!
//! bincode v2 dropped the `serialize` / `deserialize` free functions in favor
//! of an explicit configuration object and a `bincode::serde::*` module path.
//! Every call site repeating `bincode::config::standard()` adds noise, so we
//! centralize the configuration here and expose two thin wrappers.
//!
//! The configuration is `bincode::config::standard()`:
//! - little-endian byte order,
//! - varint integer encoding (smaller wire size for typical small values),
//! - no length limit (limits are enforced at the frame layer).

use bincode::config::Configuration;
use serde::{de::DeserializeOwned, Serialize};

/// Standard bincode v2 configuration used everywhere in the protocol layer.
#[inline]
pub fn config() -> Configuration {
    bincode::config::standard()
}

/// Encode a serde-serializable value to bytes via bincode v2.
#[inline]
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, bincode::error::EncodeError> {
    bincode::serde::encode_to_vec(value, config())
}

/// Decode a serde-deserializable value from bytes via bincode v2.
///
/// Returns the decoded value and the number of bytes consumed. The protocol
/// layer asserts the consumed length equals the input length where appropriate
/// (e.g. inside a frame, after the CRC has already validated the byte count).
#[inline]
pub fn decode<T: DeserializeOwned>(
    bytes: &[u8],
) -> Result<(T, usize), bincode::error::DecodeError> {
    bincode::serde::decode_from_slice(bytes, config())
}

/// Decode a value, discarding the consumed-length sidecar.
/// Convenience for tests that only care about the value.
#[inline]
pub fn decode_value<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, bincode::error::DecodeError> {
    decode(bytes).map(|(v, _)| v)
}
