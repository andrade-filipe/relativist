//! EncoderRegistry — runtime-extensible map of named codecs (SPEC-27 R17-R20).
//!
//! The registry stores `Box<dyn Codec>` keyed by `Codec::name()`, lets the CLI
//! dispatch encoders by name, and centralises encode-contract validation
//! (`validate_encoded_net`, R5/R6) so callers don't have to remember to invoke it.
//!
//! `default_registry()` returns the 5 built-in codecs required by R19.

use std::collections::HashMap;

use super::codec_church::{ChurchArithmeticCodec, ChurchOp};
use super::codec_lambda::LambdaCodec;
use super::traits::{validate_encoded_net, Codec, DecodeError, EncodeError};
use crate::net::Net;

/// Registry of codecs keyed by their `name()` (SPEC-27 R17).
pub struct EncoderRegistry {
    codecs: HashMap<String, Box<dyn Codec>>,
}

/// Errors raised by registry operations (SPEC-27 R20).
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("encoder '{0}' is already registered")]
    DuplicateName(String),
    #[error("encoder '{0}' not found")]
    NotFound(String),
    #[error(transparent)]
    Encode(#[from] EncodeError),
    #[error(transparent)]
    Decode(#[from] DecodeError),
}

impl EncoderRegistry {
    pub fn new() -> Self {
        Self {
            codecs: HashMap::new(),
        }
    }

    /// Register a codec. Returns `DuplicateName` if a codec with the same
    /// `name()` is already present (SPEC-27 R20).
    pub fn register(&mut self, codec: Box<dyn Codec>) -> Result<(), RegistryError> {
        let name = codec.name().to_string();
        if self.codecs.contains_key(&name) {
            return Err(RegistryError::DuplicateName(name));
        }
        self.codecs.insert(name, codec);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&dyn Codec> {
        self.codecs.get(name).map(|b| b.as_ref())
    }

    /// List `(name, description)` pairs sorted by name (deterministic CLI output).
    pub fn list(&self) -> Vec<(&str, &str)> {
        let mut pairs: Vec<(&str, &str)> = self
            .codecs
            .values()
            .map(|c| (c.name(), c.description()))
            .collect();
        pairs.sort_by_key(|(n, _)| *n);
        pairs
    }

    /// Encode + validate (SPEC-27 R18, R5/R6). Returns the encoded `Net` on success.
    pub fn encode_and_validate(&self, name: &str, input: &[u8]) -> Result<Net, RegistryError> {
        let codec = self
            .get(name)
            .ok_or_else(|| RegistryError::NotFound(name.to_string()))?;
        let net = codec.encode(input)?;
        validate_encoded_net(&net)?;
        Ok(net)
    }

    pub fn decode(&self, name: &str, net: &Net) -> Result<serde_json::Value, RegistryError> {
        let codec = self
            .get(name)
            .ok_or_else(|| RegistryError::NotFound(name.to_string()))?;
        Ok(codec.decode(net)?)
    }
}

impl Default for EncoderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns a registry pre-populated with the 5 built-in codecs (SPEC-27 R19).
pub fn default_registry() -> EncoderRegistry {
    let mut r = EncoderRegistry::new();
    // unwrap is safe: each codec has a unique name and the registry starts empty.
    r.register(Box::new(ChurchArithmeticCodec::new(ChurchOp::Add)))
        .expect("church_add registers");
    r.register(Box::new(ChurchArithmeticCodec::new(ChurchOp::Mul)))
        .expect("church_mul registers");
    r.register(Box::new(ChurchArithmeticCodec::new(ChurchOp::Exp)))
        .expect("church_exp registers");
    r.register(Box::new(ChurchArithmeticCodec::new(ChurchOp::SumOfSquares)))
        .expect("church_sum_of_squares registers");
    r.register(Box::new(LambdaCodec::new()))
        .expect("lambda registers");
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reduction::reduce_all;

    // --- Registry struct + ops (TEST-SPEC-0336) ---

    #[test]
    fn empty_registry_has_no_codecs() {
        let r = EncoderRegistry::new();
        assert!(r.list().is_empty());
        assert!(r.get("anything").is_none());
    }

    #[test]
    fn register_and_get_round_trip() {
        let mut r = EncoderRegistry::new();
        r.register(Box::new(LambdaCodec::new())).unwrap();
        let c = r.get("lambda").expect("lambda registered");
        assert_eq!(c.name(), "lambda");
    }

    #[test]
    fn register_duplicate_name_rejected() {
        let mut r = EncoderRegistry::new();
        r.register(Box::new(LambdaCodec::new())).unwrap();
        let err = r.register(Box::new(LambdaCodec::new())).unwrap_err();
        match err {
            RegistryError::DuplicateName(n) => assert_eq!(n, "lambda"),
            other => panic!("expected DuplicateName, got {:?}", other),
        }
    }

    #[test]
    fn list_is_sorted_with_descriptions() {
        let mut r = EncoderRegistry::new();
        r.register(Box::new(LambdaCodec::new())).unwrap();
        r.register(Box::new(ChurchArithmeticCodec::new(ChurchOp::Add)))
            .unwrap();
        let l = r.list();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].0, "church_add");
        assert_eq!(l[1].0, "lambda");
        assert!(!l[0].1.is_empty());
        assert!(!l[1].1.is_empty());
    }

    #[test]
    fn encode_and_validate_returns_net_on_valid_input() {
        let mut r = EncoderRegistry::new();
        r.register(Box::new(LambdaCodec::new())).unwrap();
        // (λx. x) (λy. y) — has a CON-CON redex so validation passes.
        let net = r
            .encode_and_validate("lambda", r#"{"term":"(λx. x) (λy. y)"}"#.as_bytes())
            .unwrap();
        assert!(net.count_live_agents() >= 2);
    }

    #[test]
    fn encode_and_validate_rejects_unknown_encoder() {
        let r = EncoderRegistry::new();
        let err = r.encode_and_validate("nope", b"{}").unwrap_err();
        match err {
            RegistryError::NotFound(n) => assert_eq!(n, "nope"),
            other => panic!("expected NotFound, got {:?}", other),
        }
    }

    #[test]
    fn encode_and_validate_propagates_encoder_errors() {
        let mut r = EncoderRegistry::new();
        r.register(Box::new(LambdaCodec::new())).unwrap();
        let err = r.encode_and_validate("lambda", b"{not json}").unwrap_err();
        assert!(matches!(
            err,
            RegistryError::Encode(EncodeError::InvalidInput(_))
        ));
    }

    #[test]
    fn decode_rejects_unknown_encoder() {
        let r = EncoderRegistry::new();
        let err = r.decode("nope", &Net::new()).unwrap_err();
        assert!(matches!(err, RegistryError::NotFound(_)));
    }

    // --- default_registry (TEST-SPEC-0337) ---

    #[test]
    fn default_registry_has_five_codecs() {
        let r = default_registry();
        assert_eq!(r.list().len(), 5);
    }

    #[test]
    fn default_registry_names_match_spec() {
        let r = default_registry();
        let names: Vec<&str> = r.list().iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            vec![
                "church_add",
                "church_exp",
                "church_mul",
                "church_sum_of_squares",
                "lambda",
            ]
        );
    }

    #[test]
    fn default_registry_church_codecs_round_trip() {
        let r = default_registry();
        // church_exp decode is a known SPEC-14 limitation (DUP-cycle readback
        // unimplemented), so we verify it encodes successfully but only check
        // round-trip on add/mul/sum_of_squares.
        let cases: [(&str, &[u8], u64); 3] = [
            ("church_add", br#"{"a":3,"b":5}"#, 8),
            ("church_mul", br#"{"a":3,"b":4}"#, 12),
            ("church_sum_of_squares", br#"{"a":3}"#, 14),
        ];
        for (name, json, expected) in cases {
            let mut net = r.encode_and_validate(name, json).unwrap();
            reduce_all(&mut net);
            let out = r.decode(name, &net).unwrap();
            assert_eq!(
                out["result"].as_u64().unwrap(),
                expected,
                "{} produced wrong result",
                name
            );
        }
        // church_exp: encode + validate only (decode is known v2 limitation).
        let _ = r
            .encode_and_validate("church_exp", br#"{"a":2,"b":3}"#)
            .expect("church_exp encodes");
    }

    #[test]
    fn default_registry_lambda_round_trips_identity() {
        let r = default_registry();
        let net = r
            .encode_and_validate("lambda", r#"{"term":"(λx. x) (λy. y)"}"#.as_bytes())
            .unwrap();
        let mut n = net;
        reduce_all(&mut n);
        let out = r.decode("lambda", &n).unwrap();
        assert!(out["term"].as_str().unwrap().contains("λ"));
    }

    #[test]
    fn default_registry_descriptions_non_empty() {
        let r = default_registry();
        for (name, desc) in r.list() {
            assert!(!desc.is_empty(), "{} has empty description", name);
        }
    }

    // --- QA adversarial (Stage 5) ---

    // Encoder produces a net with no redex — validate_encoded_net (E2) MUST reject it
    // and encode_and_validate MUST surface the error.
    #[test]
    fn encode_and_validate_rejects_net_with_no_redex() {
        // λx. x — single CON with self-loop, no redex (E2 fails).
        let r = default_registry();
        let err = r
            .encode_and_validate("lambda", r#"{"term":"λx. x"}"#.as_bytes())
            .unwrap_err();
        match err {
            RegistryError::Encode(EncodeError::InvalidNet(_)) => {}
            other => panic!("expected Encode(InvalidNet), got {:?}", other),
        }
    }

    // Codec lookup is case-sensitive — "Lambda" != "lambda".
    #[test]
    fn registry_lookup_is_case_sensitive() {
        let r = default_registry();
        assert!(r.get("lambda").is_some());
        assert!(r.get("Lambda").is_none());
        assert!(r.get("LAMBDA").is_none());
    }

    // After a failed register (collision), original codec remains intact and usable.
    #[test]
    fn failed_register_does_not_corrupt_registry() {
        let mut r = EncoderRegistry::new();
        r.register(Box::new(LambdaCodec::new())).unwrap();
        // Second register fails; original must survive.
        let _ = r.register(Box::new(LambdaCodec::new())).unwrap_err();
        assert_eq!(r.list().len(), 1);
        assert!(r.get("lambda").is_some());
        // And it still works.
        let net = r
            .encode_and_validate("lambda", r#"{"term":"(λx. x) (λy. y)"}"#.as_bytes())
            .expect("registry still functional");
        assert!(net.count_live_agents() >= 2);
    }

    // Empty input bytes — encoder returns InvalidInput, NOT a panic.
    #[test]
    fn empty_input_handled_gracefully() {
        let r = default_registry();
        let err = r.encode_and_validate("lambda", b"").unwrap_err();
        assert!(matches!(
            err,
            RegistryError::Encode(EncodeError::InvalidInput(_))
        ));
    }

    // NotFound carries the exact name asked for, including odd characters.
    #[test]
    fn not_found_preserves_query_name() {
        let r = default_registry();
        let err = r.encode_and_validate("foo bar/baz", b"{}").unwrap_err();
        match err {
            RegistryError::NotFound(n) => assert_eq!(n, "foo bar/baz"),
            other => panic!("expected NotFound(\"foo bar/baz\"), got {:?}", other),
        }
    }

    // Default impl (clippy-friendly) is equivalent to new().
    #[test]
    fn default_impl_equivalent_to_new() {
        let r = EncoderRegistry::default();
        assert!(r.list().is_empty());
    }
}
