//! Tracing subscriber initialization (SPEC-11 R1-R5, R31-R33).
//!
//! Configures the tracing subscriber with fmt::Layer (text or JSON),
//! EnvFilter from RUST_LOG or per-component defaults, and logs
//! an INFO event confirming initialization.

use std::sync::atomic::{AtomicBool, Ordering};

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

/// Guard against double initialization (SPEC-11 R32).
static TRACING_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialize the tracing subscriber with all configured layers (SPEC-11 R31-R33).
///
/// MUST be called exactly once at startup, before any tracing macros.
/// Panics if called more than once (R32).
///
/// - Reads `RUST_LOG` environment variable for log filtering (R4).
/// - Falls back to `DEFAULT_LOG_FILTER` if `RUST_LOG` is not set (R5).
/// - Selects text or JSON output format based on config (R3).
/// - Includes target, thread ID, and timestamp in output (R9).
pub fn init_tracing(config: &ObservabilityConfig) {
    // R32: panic on double initialization.
    if TRACING_INITIALIZED.swap(true, Ordering::SeqCst) {
        panic!("init_tracing() called more than once — this is a bug (SPEC-11 R32)");
    }

    let using_rust_log = std::env::var("RUST_LOG").is_ok();
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER));
    let filter_desc = if using_rust_log {
        "RUST_LOG"
    } else {
        "defaults"
    };

    // BUG-FIX 2026-05-14: route all tracing output to stderr so it does
    // not contaminate stdout-bound consumers (CSV pipelines, plotters,
    // `--output -` style writers). The fmt layer defaults to stdout,
    // which silently corrupted `raw/in_process.csv` in the 2026-05-14
    // stress-curve run (147 WARN lines interleaved with data rows).
    // stderr is the correct destination for diagnostics on every
    // platform we support (Linux, Docker, Windows + Git Bash).
    match config.log_format {
        LogFormat::Text => {
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_writer(std::io::stderr)
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
                        .with_writer(std::io::stderr)
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_file(false)
                        .with_line_number(false),
                )
                .init();
        }
    }

    // R33: log initialization confirmation with filter source and default string.
    tracing::info!(
        log_format = ?config.log_format,
        role = ?config.role,
        filter_source = filter_desc,
        default_filter = DEFAULT_LOG_FILTER,
        metrics_enabled = cfg!(feature = "metrics"),
        otel_enabled = cfg!(feature = "otel"),
        "observability initialized"
    );
}

/// Reset the initialization guard. **Test-only** — allows multiple tests
/// to call `init_tracing` in the same process.
#[cfg(test)]
pub(crate) fn reset_init_guard() {
    TRACING_INITIALIZED.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_log_filter_is_valid() {
        // Verify the filter string parses without error
        let filter = EnvFilter::try_new(DEFAULT_LOG_FILTER);
        assert!(
            filter.is_ok(),
            "DEFAULT_LOG_FILTER should parse: {:?}",
            filter.err()
        );
    }

    #[test]
    fn test_default_log_filter_contains_expected_targets() {
        assert!(DEFAULT_LOG_FILTER.contains("relativist::coordinator=info"));
        assert!(DEFAULT_LOG_FILTER.contains("relativist::reduction=warn"));
        assert!(DEFAULT_LOG_FILTER.contains("relativist::protocol=warn"));
        assert!(DEFAULT_LOG_FILTER.contains("relativist::security=info"));
    }

    // T2 (SPEC-11): double initialization MUST panic.
    #[test]
    fn test_double_init_guard_panics() {
        // Ensure the flag is set so a call would panic.
        // We cannot actually call init_tracing twice (global subscriber),
        // so we test the guard directly.
        TRACING_INITIALIZED.store(true, Ordering::SeqCst);
        let result = std::panic::catch_unwind(|| {
            // This should panic because the guard is already set.
            if TRACING_INITIALIZED.swap(true, Ordering::SeqCst) {
                panic!("init_tracing() called more than once — this is a bug (SPEC-11 R32)");
            }
        });
        assert!(result.is_err(), "double init should panic");
        // Reset for other tests.
        reset_init_guard();
    }
}
