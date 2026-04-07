//! Tracing subscriber initialization (SPEC-11 R1-R5, R31-R33).
//!
//! Configures the tracing subscriber with fmt::Layer (text or JSON),
//! EnvFilter from RUST_LOG or per-component defaults, and logs
//! an INFO event confirming initialization.

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use super::types::{LogFormat, ObservabilityConfig};

/// Default per-component log levels (SPEC-11 R5).
///
/// Hot paths (reduction, protocol) default to WARN; lifecycle
/// modules (coordinator, worker, partition) default to INFO.
/// The trailing `warn` is the catch-all for unmatched targets.
pub const DEFAULT_LOG_FILTER: &str = "\
    relativist::coordinator=info,\
    relativist::worker=info,\
    relativist::reduction=warn,\
    relativist::protocol=warn,\
    relativist::partition=info,\
    relativist::net=warn,\
    relativist::observability=info,\
    relativist::security=info,\
    warn";

/// Initialize the tracing subscriber with all configured layers (SPEC-11 R31-R33).
///
/// MUST be called exactly once at startup, before any tracing macros.
/// Panics if called more than once (tracing_subscriber limitation).
///
/// - Reads `RUST_LOG` environment variable for log filtering (R4).
/// - Falls back to `DEFAULT_LOG_FILTER` if `RUST_LOG` is not set (R5).
/// - Selects text or JSON output format based on config (R3).
/// - Includes target, thread ID, and timestamp in output (R9).
pub fn init_tracing(config: &ObservabilityConfig) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER));

    match config.log_format {
        LogFormat::Text => {
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_file(false)
                        .with_line_number(false),
                )
                .init();
        }
        LogFormat::Json => {
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    tracing_subscriber::fmt::layer()
                        .json()
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_file(false)
                        .with_line_number(false),
                )
                .init();
        }
    }

    // R33: log initialization confirmation
    tracing::info!(
        log_format = ?config.log_format,
        role = ?config.role,
        metrics_enabled = cfg!(feature = "metrics"),
        otel_enabled = cfg!(feature = "otel"),
        "observability initialized"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_log_filter_is_valid() {
        // Verify the filter string parses without error
        let filter = EnvFilter::try_new(DEFAULT_LOG_FILTER);
        assert!(filter.is_ok(), "DEFAULT_LOG_FILTER should parse: {:?}", filter.err());
    }

    #[test]
    fn test_default_log_filter_contains_expected_targets() {
        assert!(DEFAULT_LOG_FILTER.contains("relativist::coordinator=info"));
        assert!(DEFAULT_LOG_FILTER.contains("relativist::reduction=warn"));
        assert!(DEFAULT_LOG_FILTER.contains("relativist::protocol=warn"));
        assert!(DEFAULT_LOG_FILTER.contains("relativist::security=info"));
    }
}
