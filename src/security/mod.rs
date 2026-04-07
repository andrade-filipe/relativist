//! Security: token authentication and optional TLS (SPEC-10).
//!
//! Implements the 3-tier security model:
//! - Tier 1 (Development): No auth, no TLS, localhost only
//! - Tier 2 (Private Network): Token auth, no TLS
//! - Tier 3 (Production): Token auth + TLS 1.3

pub mod error;
pub mod token;

#[cfg(feature = "tls")]
pub mod tls;

pub use error::{SecurityError, TokenError};
pub use token::AuthToken;

use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

/// The three security tiers (SPEC-10 R1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum SecurityTier {
    /// Tier 1: No auth, no TLS, localhost only.
    Development,
    /// Tier 2: Token auth, no TLS.
    PrivateNetwork,
    /// Tier 3: Token auth + TLS 1.3.
    Production,
}

/// Security configuration, assembled from CLI flags (SPEC-10 Section 4.2).
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// The active security tier (detected from CLI flags, not directly configured).
    pub tier: SecurityTier,
    /// Authentication token. None for Tier 1.
    pub token: Option<AuthToken>,
    /// Maximum concurrent connections (SPEC-10 R31).
    pub max_connections: usize,
    /// Connection idle timeout (SPEC-10 R32).
    pub idle_timeout: Duration,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            tier: SecurityTier::Development,
            token: None,
            max_connections: 1024,
            idle_timeout: Duration::from_secs(30),
        }
    }
}

/// Detect the security tier from CLI flags (SPEC-10 R3).
///
/// - No `--token` and no TLS flags → Tier 1 (Development)
/// - `--token` present, no TLS flags → Tier 2 (PrivateNetwork)
/// - `--token` present + TLS flags → Tier 3 (Production)
pub fn detect_tier(has_token: bool, has_tls: bool) -> SecurityTier {
    match (has_token, has_tls) {
        (false, _) => SecurityTier::Development,
        (true, false) => SecurityTier::PrivateNetwork,
        (true, true) => SecurityTier::Production,
    }
}

/// Write the generated token to a file (SPEC-10 R12).
///
/// On Unix, sets file permissions to 0600 (owner read/write only).
/// On other platforms, uses default permissions.
pub fn write_token_file(token: &AuthToken, path: &Path) -> Result<(), SecurityError> {
    let encoded = token.to_base64();
    std::fs::write(path, encoded.as_bytes())
        .map_err(|e| SecurityError::Config(format!("failed to write token file {:?}: {}", path, e)))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms).map_err(|e| {
            SecurityError::Config(format!("failed to set permissions on {:?}: {}", path, e))
        })?;
    }

    tracing::info!(path = ?path, "wrote token file");
    Ok(())
}

/// Emit security warnings for network binding (SPEC-10 R7, R8).
pub fn check_bind_warnings(bind: &SocketAddr, has_token: bool) {
    if bind.ip().is_unspecified() {
        tracing::warn!(
            "Binding to all interfaces (0.0.0.0). \
             Ensure authentication is enabled for non-trusted networks."
        );
        if !has_token {
            tracing::warn!(
                "No authentication configured while binding to all interfaces. \
                 Use --token for production deployments."
            );
        }
    }
}

/// Build a SecurityConfig from CLI flags (SPEC-10 R1-R3, TASK-0138).
///
/// `token_flag`: value of --token (None = absent, Some("auto") = generate, Some(base64) = decode)
/// `has_tls`: whether TLS flags were provided
pub fn build_security_config(
    token_flag: Option<&str>,
    has_tls: bool,
) -> Result<SecurityConfig, SecurityError> {
    let token = match token_flag {
        None => None,
        Some("auto") => {
            let t = AuthToken::generate();
            tracing::info!(token = %t.to_base64(), "Worker authentication token");
            Some(t)
        }
        Some(value) => {
            let t = AuthToken::from_base64(value)?;
            tracing::info!("Using provided authentication token");
            Some(t)
        }
    };

    let has_token = token.is_some();
    let tier = detect_tier(has_token, has_tls);

    // SPEC-10 R4: TLS without token SHOULD be rejected
    if has_tls && !has_token {
        return Err(SecurityError::Config(
            "TLS flags provided without --token. \
             TLS without token auth is insecure (any host can register)."
                .into(),
        ));
    }

    tracing::info!(tier = ?tier, "security tier detected");

    Ok(SecurityConfig {
        tier,
        token,
        max_connections: 1024,
        idle_timeout: Duration::from_secs(30),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_tier_development() {
        assert_eq!(detect_tier(false, false), SecurityTier::Development);
    }

    #[test]
    fn test_detect_tier_development_tls_without_token() {
        // TLS without token → still Development (SPEC-10 R4 says SHOULD reject,
        // but detection returns Development; validation happens elsewhere)
        assert_eq!(detect_tier(false, true), SecurityTier::Development);
    }

    #[test]
    fn test_detect_tier_private_network() {
        assert_eq!(detect_tier(true, false), SecurityTier::PrivateNetwork);
    }

    #[test]
    fn test_detect_tier_production() {
        assert_eq!(detect_tier(true, true), SecurityTier::Production);
    }

    #[test]
    fn test_security_tier_serialize() {
        let json = serde_json::to_string(&SecurityTier::Development).unwrap();
        assert!(json.contains("Development"));
    }

    #[test]
    fn test_security_config_default() {
        let config = SecurityConfig::default();
        assert_eq!(config.tier, SecurityTier::Development);
        assert!(config.token.is_none());
        assert_eq!(config.max_connections, 1024);
        assert_eq!(config.idle_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_build_security_config_no_token() {
        let config = build_security_config(None, false).unwrap();
        assert_eq!(config.tier, SecurityTier::Development);
        assert!(config.token.is_none());
    }

    #[test]
    fn test_build_security_config_auto_token() {
        let config = build_security_config(Some("auto"), false).unwrap();
        assert_eq!(config.tier, SecurityTier::PrivateNetwork);
        assert!(config.token.is_some());
    }

    #[test]
    fn test_build_security_config_explicit_token() {
        let token = AuthToken::generate();
        let b64 = token.to_base64();
        let config = build_security_config(Some(&b64), false).unwrap();
        assert_eq!(config.tier, SecurityTier::PrivateNetwork);
        assert!(config.token.is_some());
        assert!(config.token.unwrap().verify(&token));
    }

    #[test]
    fn test_build_security_config_tls_without_token_rejected() {
        let result = build_security_config(None, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_security_config_tls_with_token() {
        let config = build_security_config(Some("auto"), true).unwrap();
        assert_eq!(config.tier, SecurityTier::Production);
    }

    #[test]
    fn test_build_security_config_invalid_token() {
        let result = build_security_config(Some("not-valid-base64!!!"), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_write_token_file() {
        let token = AuthToken::generate();
        let path = std::env::temp_dir().join("relativist_test_token");
        write_token_file(&token, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let restored = AuthToken::from_base64(&content).unwrap();
        assert!(token.verify(&restored));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_check_bind_warnings_localhost() {
        // Should not panic — just verify it runs
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        check_bind_warnings(&addr, false);
    }

    #[test]
    fn test_check_bind_warnings_all_interfaces() {
        // Should not panic
        let addr: SocketAddr = "0.0.0.0:9000".parse().unwrap();
        check_bind_warnings(&addr, true);
        check_bind_warnings(&addr, false);
    }
}
