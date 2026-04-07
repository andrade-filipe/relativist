//! Observability: tracing and metrics (SPEC-11).
//!
//! Configurable logging via tracing + tracing-subscriber,
//! optional Prometheus metrics and OpenTelemetry export.

use tracing_subscriber::EnvFilter;

/// Initialize the tracing subscriber with env-filter (SPEC-07 R35).
///
/// Reads the `RUST_LOG` environment variable. If unset, defaults to `info`.
/// Must be called exactly once at program startup, before any tracing macros.
pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();
}
