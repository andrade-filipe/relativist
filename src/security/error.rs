//! Security error types: TokenError and SecurityError (SPEC-10 Section 4.4).

use thiserror::Error;

/// Errors from token parsing and validation (SPEC-10 Section 4.4).
#[derive(Debug, Error)]
pub enum TokenError {
    #[error("invalid base64 encoding: {0}")]
    InvalidBase64(String),

    #[error("invalid token length: expected 32 bytes, got {0}")]
    InvalidLength(usize),
}

/// Errors from the security subsystem (SPEC-10 Section 4.4).
#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("token error: {0}")]
    Token(#[from] TokenError),

    #[error("TLS configuration error: {0}")]
    TlsConfig(String),

    #[error("certificate error: {0}")]
    Certificate(String),

    #[error("authentication failed")]
    AuthFailed,

    #[error("configuration error: {0}")]
    Config(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_error_display() {
        let e = TokenError::InvalidBase64("bad input".into());
        assert!(format!("{}", e).contains("bad input"));

        let e = TokenError::InvalidLength(16);
        assert!(format!("{}", e).contains("16"));
    }

    #[test]
    fn test_security_error_display() {
        let e = SecurityError::AuthFailed;
        assert_eq!(format!("{}", e), "authentication failed");

        let e = SecurityError::Config("missing flag".into());
        assert!(format!("{}", e).contains("missing flag"));
    }

    #[test]
    fn test_security_error_from_token() {
        let te = TokenError::InvalidLength(10);
        let se: SecurityError = te.into();
        assert!(matches!(se, SecurityError::Token(_)));
    }
}
