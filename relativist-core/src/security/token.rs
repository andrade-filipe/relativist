//! Token-based authentication for worker registration (SPEC-10 R9-R19).
//!
//! AuthToken is a 256-bit (32-byte) cryptographically random value.
//! It MUST NOT implement PartialEq or Eq — all comparisons MUST go
//! through the `verify()` method which uses constant-time comparison
//! via `subtle::ConstantTimeEq` (SPEC-10 R15).

use base64::Engine;
use rand::rngs::OsRng;
use rand::TryRngCore;
use subtle::ConstantTimeEq;

use super::error::TokenError;

/// 256-bit authentication token (SPEC-10 R9, R19).
///
/// IMPORTANT: Does NOT implement PartialEq/Eq. Use `verify()` for
/// constant-time comparison to prevent timing side-channel attacks.
#[derive(Clone)]
pub struct AuthToken([u8; 32]);

impl std::fmt::Debug for AuthToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AuthToken").field(&"[REDACTED]").finish()
    }
}

impl AuthToken {
    /// Generate a new random token using a CSPRNG (SPEC-10 R9).
    ///
    /// Uses the OS CSPRNG (`OsRng`) via rand 0.9's fallible `TryRngCore`
    /// API. A failure here means the operating system could not provide
    /// entropy — an unrecoverable condition under which minting an auth
    /// token would be unsafe, so we panic rather than emit a weak token.
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        OsRng
            .try_fill_bytes(&mut bytes)
            .expect("OS CSPRNG (OsRng) must be available to mint auth tokens");
        Self(bytes)
    }

    /// Create a token from raw bytes.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Decode a token from a base64-encoded string (SPEC-10 R10).
    pub fn from_base64(s: &str) -> Result<Self, TokenError> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(s)
            .map_err(|e| TokenError::InvalidBase64(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(TokenError::InvalidLength(bytes.len()));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    /// Encode the token as a base64 string (SPEC-10 R10).
    pub fn to_base64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(self.0)
    }

    /// Constant-time comparison to prevent timing attacks (SPEC-10 R15).
    ///
    /// Delegates to `subtle::ConstantTimeEq`. Returns true if both
    /// tokens contain identical bytes.
    pub fn verify(&self, other: &AuthToken) -> bool {
        self.0.ct_eq(&other.0).into()
    }

    /// Return the raw bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_produces_32_bytes() {
        let token = AuthToken::generate();
        assert_eq!(token.as_bytes().len(), 32);
    }

    #[test]
    fn test_generate_is_random() {
        let t1 = AuthToken::generate();
        let t2 = AuthToken::generate();
        // Probabilistically, two random 256-bit values should differ
        assert!(!t1.verify(&t2));
    }

    #[test]
    fn test_base64_roundtrip() {
        let token = AuthToken::generate();
        let encoded = token.to_base64();
        let decoded = AuthToken::from_base64(&encoded).unwrap();
        assert!(token.verify(&decoded));
    }

    #[test]
    fn test_base64_length() {
        let token = AuthToken::generate();
        let encoded = token.to_base64();
        assert_eq!(encoded.len(), 44); // 32 bytes → 44 base64 chars
    }

    #[test]
    fn test_from_base64_invalid() {
        let result = AuthToken::from_base64("not-valid-base64!!!");
        assert!(matches!(result, Err(TokenError::InvalidBase64(_))));
    }

    #[test]
    fn test_from_base64_wrong_length() {
        // 16 bytes encoded as base64
        let short = base64::engine::general_purpose::STANDARD.encode([0u8; 16]);
        let result = AuthToken::from_base64(&short);
        assert!(matches!(result, Err(TokenError::InvalidLength(16))));
    }

    #[test]
    fn test_verify_same_token() {
        let bytes = [0xABu8; 32];
        let t1 = AuthToken::from_bytes(bytes);
        let t2 = AuthToken::from_bytes(bytes);
        assert!(t1.verify(&t2));
    }

    #[test]
    fn test_verify_different_tokens() {
        let t1 = AuthToken::from_bytes([0xAA; 32]);
        let t2 = AuthToken::from_bytes([0xBB; 32]);
        assert!(!t1.verify(&t2));
    }

    #[test]
    fn test_debug_redacted() {
        let token = AuthToken::generate();
        let debug = format!("{:?}", token);
        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains(&token.to_base64()));
    }

    #[test]
    fn test_from_bytes() {
        let bytes = [42u8; 32];
        let token = AuthToken::from_bytes(bytes);
        assert_eq!(token.as_bytes(), &bytes);
    }

    #[test]
    fn test_clone() {
        let token = AuthToken::generate();
        let cloned = token.clone();
        assert!(token.verify(&cloned));
    }
}
