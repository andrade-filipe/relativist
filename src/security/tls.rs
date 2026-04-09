//! TLS 1.3 support via rustls, feature-gated under `tls` (SPEC-10 R20-R28a).
//!
//! Only compiled when the `tls` Cargo feature is enabled.
//! Server TLS only (no mTLS in v1).

use std::path::Path;
use std::sync::Arc;

use super::error::SecurityError;

/// TLS configuration for the coordinator (server side) (SPEC-10 R25).
#[derive(Clone)]
pub struct TlsServerConfig {
    /// Pre-built rustls ServerConfig from cert + key.
    pub config: Arc<rustls::ServerConfig>,
}

impl std::fmt::Debug for TlsServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TlsServerConfig").finish()
    }
}

impl TlsServerConfig {
    /// Load certificate and private key from PEM files (SPEC-10 R25).
    pub fn from_pem_files(cert_path: &Path, key_path: &Path) -> Result<Self, SecurityError> {
        let cert_data = std::fs::read(cert_path).map_err(|e| {
            SecurityError::Certificate(format!("failed to read cert {:?}: {}", cert_path, e))
        })?;
        let key_data = std::fs::read(key_path).map_err(|e| {
            SecurityError::Certificate(format!("failed to read key {:?}: {}", key_path, e))
        })?;

        let certs = rustls_pemfile::certs(&mut cert_data.as_slice())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| SecurityError::Certificate(format!("invalid cert PEM: {}", e)))?;

        let key = rustls_pemfile::private_key(&mut key_data.as_slice())
            .map_err(|e| SecurityError::Certificate(format!("invalid key PEM: {}", e)))?
            .ok_or_else(|| SecurityError::Certificate("no private key found in PEM file".into()))?;

        // SPEC-10 R22: TLS 1.3 exclusively, no TLS 1.2 fallback.
        let config =
            rustls::ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
                .with_no_client_auth()
                .with_single_cert(certs, key)
                .map_err(|e| SecurityError::TlsConfig(format!("rustls config error: {}", e)))?;

        Ok(Self {
            config: Arc::new(config),
        })
    }
}

/// TLS configuration for workers (client side) (SPEC-10 R26).
#[derive(Clone)]
pub struct TlsClientConfig {
    /// Pre-built rustls ClientConfig from CA cert.
    pub config: Arc<rustls::ClientConfig>,
}

impl std::fmt::Debug for TlsClientConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TlsClientConfig").finish()
    }
}

impl TlsClientConfig {
    /// Load CA certificate from a PEM file (SPEC-10 R26).
    pub fn from_ca_pem(ca_path: &Path) -> Result<Self, SecurityError> {
        let ca_data = std::fs::read(ca_path).map_err(|e| {
            SecurityError::Certificate(format!("failed to read CA cert {:?}: {}", ca_path, e))
        })?;

        let ca_certs = rustls_pemfile::certs(&mut ca_data.as_slice())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| SecurityError::Certificate(format!("invalid CA PEM: {}", e)))?;

        let mut root_store = rustls::RootCertStore::empty();
        for cert in ca_certs {
            root_store
                .add(cert)
                .map_err(|e| SecurityError::Certificate(format!("invalid CA cert: {}", e)))?;
        }

        // SPEC-10 R22: TLS 1.3 exclusively, no TLS 1.2 fallback.
        let config =
            rustls::ClientConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
                .with_root_certificates(root_store)
                .with_no_client_auth();

        Ok(Self {
            config: Arc::new(config),
        })
    }
}
