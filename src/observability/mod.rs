//! Observability: tracing, metrics, and HTTP endpoints (SPEC-11).
//!
//! - `types`: LogFormat, ProcessRole, ObservabilityConfig
//! - `tracing_init`: Subscriber initialization with fmt::Layer + EnvFilter
//! - `metrics`: CoordinatorMetrics (feature-gated under `metrics`)
//! - `http`: /health, /ready, /metrics endpoints (feature-gated under `metrics`)

pub mod tracing_init;
pub mod types;

#[cfg(feature = "metrics")]
pub mod http;

#[cfg(feature = "metrics")]
pub mod metrics;

pub use tracing_init::{init_tracing, DEFAULT_LOG_FILTER};
pub use types::{LogFormat, ObservabilityConfig, ProcessRole};
