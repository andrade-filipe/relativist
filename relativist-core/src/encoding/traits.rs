//! Encoder, Decoder, and Codec traits — extension API for problem domains.
//!
//! This module defines the trait boundary between problem-domain code (Church
//! arithmetic, lambda-calculus, future user-defined encoders) and the IC
//! reduction infrastructure. Any type that implements `Codec` can be
//! registered (SPEC-27 R17) and invoked via the CLI (SPEC-27 R21).
//!
//! Specified in SPEC-27 R1-R6 (Phase 1: Traits).

use crate::net::{Net, PortRef};

/// Encoder converts domain-specific problems into IC nets ready for reduction.
///
/// SPEC-27 R1.
pub trait Encoder: Send + Sync {
    /// Human-readable name (e.g., "church_add", "lambda").
    fn name(&self) -> &str;

    /// Encode a problem (JSON bytes) into an IC net with redexes.
    ///
    /// `input` is opaque JSON bytes. Each encoder defines its own input schema.
    fn encode(&self, input: &[u8]) -> Result<Net, EncodeError>;
}

/// Decoder interprets a normal-form IC net and extracts a domain answer.
///
/// SPEC-27 R2.
pub trait Decoder: Send + Sync {
    /// Decode an IC net in normal form into a JSON-serializable answer.
    fn decode(&self, net: &Net) -> Result<serde_json::Value, DecodeError>;
}

/// Codec combines Encoder and Decoder for a single problem domain.
///
/// SPEC-27 R3. All built-in encoders MUST implement Codec; the registry
/// (SPEC-27 R17) stores `Box<dyn Codec>`.
pub trait Codec: Encoder + Decoder {
    /// Short description of what this codec encodes/decodes.
    fn description(&self) -> &str;
}

/// Errors raised by `Encoder::encode()` and the encode-contract validator.
///
/// SPEC-27 R4.
#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("encoding produced invalid net: {0}")]
    InvalidNet(String),
    #[error("input too large: {size} exceeds limit {limit}")]
    InputTooLarge { size: usize, limit: usize },
}

/// Errors raised by `Decoder::decode()`.
///
/// SPEC-27 R4.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("net is not in normal form (has {redexes} redexes)")]
    NotNormalForm { redexes: usize },
    #[error("unrecognized net structure: {0}")]
    UnrecognizedStructure(String),
    #[error("decode failed: {0}")]
    DecodeFailed(String),
}

/// Validates an encoded net before reduction (SPEC-27 R5, R6: encode contract).
///
/// Checks:
/// - **E1** (subset of T1-T7): structural integrity — every wire is symmetric,
///   every AgentPort connection points to a live agent, and no port connects
///   to itself at the same index.
/// - **E2**: net has at least one redex (otherwise there is nothing to reduce).
///
/// The error message names the violated invariant (R6).
///
/// Phase 1 implements a runtime-checkable subset of T1-T7 (T1 linearity).
/// T2 (interaction via principal ports) and T3 (disjointness) follow from T1
/// when `connect()`/`disconnect()` are used correctly. T4 (strong confluence)
/// and T5 (rule correctness) are proof-level — not runtime-checkable. T6/T7
/// concern wire and symbol well-formedness, which are guaranteed by the type
/// system (`Symbol` is a `#[repr(u8)]` enum, wires are `PortRef`).
pub fn validate_encoded_net(net: &Net) -> Result<(), EncodeError> {
    for slot in net.agents.iter() {
        let Some(agent) = slot else { continue };
        for p in 0u8..3 {
            let port = PortRef::AgentPort(agent.id, p);
            let target = net.get_target(port);

            if let PortRef::AgentPort(other_id, _) = target {
                if net.get_agent(other_id).is_none() {
                    return Err(EncodeError::InvalidNet(format!(
                        "T1: agent {} port {} targets dead agent {}",
                        agent.id, p, other_id
                    )));
                }
                if target == port {
                    return Err(EncodeError::InvalidNet(format!(
                        "T1: port {:?} connects to itself",
                        port
                    )));
                }
                if net.get_target(target) != port {
                    return Err(EncodeError::InvalidNet(format!(
                        "T1: asymmetric wire {:?} -> {:?} (reverse points to {:?})",
                        port,
                        target,
                        net.get_target(target)
                    )));
                }
            }
        }
    }

    if net.is_reduced() {
        return Err(EncodeError::InvalidNet(
            "E2: net has no redexes (nothing to reduce)".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::Symbol;

    // T1 from SPEC-27 §6.1: validation catches invalid net (broken symmetry).
    #[test]
    fn validation_catches_broken_symmetry() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.redex_queue.push_back((a, b));

        // Manually break symmetry: redirect a.p0 to b.p1 without updating b.p1.
        let a_p0_idx = (a as usize) * 3;
        net.ports[a_p0_idx] = PortRef::AgentPort(b, 1);

        let result = validate_encoded_net(&net);
        assert!(
            matches!(&result, Err(EncodeError::InvalidNet(msg)) if msg.contains("T1")),
            "expected T1 violation, got {:?}",
            result
        );
    }

    // T2 from SPEC-27 §6.1: validation catches nets with no redex.
    #[test]
    fn validation_catches_no_redex() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Era);
        // Connect a.p1 (auxiliary) to b.p0 (principal): not an active pair.
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 0));

        let result = validate_encoded_net(&net);
        assert!(
            matches!(&result, Err(EncodeError::InvalidNet(msg)) if msg.contains("E2")),
            "expected E2 violation, got {:?}",
            result
        );
    }

    // Validator catches dead-agent reference (T1 sub-check).
    #[test]
    fn validation_catches_dead_agent_target() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.redex_queue.push_back((a, b));

        // Kill agent b, but leave a.p0 pointing at it.
        net.agents[b as usize] = None;

        let result = validate_encoded_net(&net);
        assert!(
            matches!(&result, Err(EncodeError::InvalidNet(msg)) if msg.contains("dead agent")),
            "expected dead-agent violation, got {:?}",
            result
        );
    }

    // Validator accepts a well-formed net with at least one redex.
    #[test]
    fn validation_accepts_valid_redex() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.redex_queue.push_back((a, b));

        assert!(validate_encoded_net(&net).is_ok());
    }

    // Object safety: Box<dyn Codec> compiles, allowing registry storage.
    #[test]
    fn codec_is_object_safe() {
        struct Dummy;
        impl Encoder for Dummy {
            fn name(&self) -> &str {
                "dummy"
            }
            fn encode(&self, _: &[u8]) -> Result<Net, EncodeError> {
                Err(EncodeError::InvalidInput("dummy".into()))
            }
        }
        impl Decoder for Dummy {
            fn decode(&self, _: &Net) -> Result<serde_json::Value, DecodeError> {
                Ok(serde_json::Value::Null)
            }
        }
        impl Codec for Dummy {
            fn description(&self) -> &str {
                "dummy codec"
            }
        }
        let boxed: Box<dyn Codec> = Box::new(Dummy);
        assert_eq!(boxed.name(), "dummy");
        assert_eq!(boxed.description(), "dummy codec");
    }

    // Error types render readable messages.
    #[test]
    fn error_messages_render() {
        let e = EncodeError::InputTooLarge {
            size: 1000,
            limit: 100,
        };
        assert!(e.to_string().contains("1000"));
        assert!(e.to_string().contains("100"));

        let d = DecodeError::NotNormalForm { redexes: 7 };
        assert!(d.to_string().contains("7"));
    }
}
