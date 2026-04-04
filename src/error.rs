//! Relativist error types.
//!
//! Centralized error handling using `thiserror` (PESQ-023 D2).
//! Errors are classified as transient (retryable) or fatal (abort).

use thiserror::Error;

/// Top-level error type for Relativist operations.
#[derive(Debug, Error)]
pub enum RelError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialize(String),

    #[error("Deserialization error: {0}")]
    Deserialize(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Configuration error: {0}")]
    Config(String),
}
