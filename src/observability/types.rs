//! Observability types: LogFormat, ProcessRole, ObservabilityConfig (SPEC-11 R3, R31).

/// Log output format (SPEC-11 R3).
///
/// Selected at startup via `--log-format` or auto-detected from TTY.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// Human-readable output (tracing_subscriber::fmt::format::Full).
    Text,
    /// Machine-parseable JSON output.
    Json,
}

/// Process role, used for OTel service.name and init behavior (SPEC-11 R28, R33a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessRole {
    /// Coordinator process — starts HTTP endpoints (metrics feature).
    Coordinator,
    /// Worker process — no HTTP endpoints.
    Worker,
    /// Local reduction mode (SPEC-13 R41: `relativist reduce` / `relativist local`).
    /// Logging is initialized; HTTP endpoints are NOT started.
    Local,
}

/// Configuration for the observability subsystem (SPEC-11 R31).
#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    /// Log output format (text or JSON).
    pub log_format: LogFormat,
    /// Whether this process is coordinator, worker, or local.
    pub role: ProcessRole,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            log_format: LogFormat::Text,
            role: ProcessRole::Local,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_format_debug() {
        assert_eq!(format!("{:?}", LogFormat::Text), "Text");
        assert_eq!(format!("{:?}", LogFormat::Json), "Json");
    }

    #[test]
    fn test_process_role_debug() {
        assert_eq!(format!("{:?}", ProcessRole::Coordinator), "Coordinator");
        assert_eq!(format!("{:?}", ProcessRole::Worker), "Worker");
        assert_eq!(format!("{:?}", ProcessRole::Local), "Local");
    }

    #[test]
    fn test_observability_config_default() {
        let config = ObservabilityConfig::default();
        assert_eq!(config.log_format, LogFormat::Text);
        assert_eq!(config.role, ProcessRole::Local);
    }

    #[test]
    fn test_log_format_equality() {
        assert_eq!(LogFormat::Text, LogFormat::Text);
        assert_ne!(LogFormat::Text, LogFormat::Json);
    }

    #[test]
    fn test_process_role_equality() {
        assert_eq!(ProcessRole::Coordinator, ProcessRole::Coordinator);
        assert_ne!(ProcessRole::Coordinator, ProcessRole::Worker);
    }
}
