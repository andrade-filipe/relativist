//! HTTP endpoints for health, readiness, and Prometheus metrics (SPEC-11 R19-R24a).
//!
//! Feature-gated under `metrics`. Runs as a background tokio task
//! on a dedicated port (default 9090), separate from the grid protocol.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use prometheus_client::registry::Registry;

/// Shared state for the HTTP endpoints.
#[derive(Clone)]
pub struct AppState {
    /// Prometheus registry for /metrics encoding.
    pub registry: Arc<Registry>,
    /// Coordinator readiness flag (SPEC-11 R22a).
    /// Set to true when coordinator FSM leaves Init state.
    pub is_ready: Arc<AtomicBool>,
}

/// Build the axum Router with /health, /ready, /metrics routes (SPEC-11 R21).
pub fn metrics_router(registry: Arc<Registry>, is_ready: Arc<AtomicBool>) -> Router {
    let state = AppState { registry, is_ready };

    Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .route("/metrics", get(metrics_handler))
        .with_state(state)
}

/// GET /health — liveness check (SPEC-11 R21).
///
/// Returns 200 "ok" if the process is alive.
async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

/// GET /ready — readiness check (SPEC-11 R22, R22a).
///
/// Returns 200 "ready" when coordinator is past Init state,
/// 503 "not ready" otherwise.
async fn ready_handler(State(state): State<AppState>) -> impl IntoResponse {
    if state.is_ready.load(Ordering::Relaxed) {
        (StatusCode::OK, "ready")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "not ready")
    }
}

/// GET /metrics — Prometheus scrape endpoint (SPEC-11 R21).
///
/// Encodes all registered metrics in OpenMetrics text format.
async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    let mut buf = String::new();
    if let Err(e) = prometheus_client::encoding::text::encode(&mut buf, &state.registry) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(axum::http::header::CONTENT_TYPE, "text/plain")],
            format!("encoding error: {}", e),
        );
    }
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "application/openmetrics-text; version=1.0.0; charset=utf-8",
        )],
        buf,
    )
}

/// Spawn the HTTP server as a background tokio task (SPEC-11 R23).
///
/// Returns a handle that can be used to await the server.
/// The server shuts down when the `shutdown` signal resolves.
pub async fn spawn_metrics_server(
    bind_addr: std::net::SocketAddr,
    registry: Arc<Registry>,
    is_ready: Arc<AtomicBool>,
    shutdown: tokio::sync::oneshot::Receiver<()>,
) -> Result<tokio::task::JoinHandle<()>, std::io::Error> {
    let app = metrics_router(registry, is_ready);
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    let actual_addr = listener.local_addr()?;

    tracing::info!(addr = %actual_addr, "metrics HTTP server listening");

    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown.await;
            })
            .await
            .ok();
    });

    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn make_router() -> (Router, Arc<AtomicBool>) {
        let registry = Arc::new(Registry::default());
        let is_ready = Arc::new(AtomicBool::new(false));
        let router = metrics_router(registry, is_ready.clone());
        (router, is_ready)
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let (app, _) = make_router();
        let response = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_ready_not_ready() {
        let (app, _is_ready) = make_router();
        // is_ready defaults to false
        let response = app
            .oneshot(Request::get("/ready").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_ready_when_ready() {
        let (app, is_ready) = make_router();
        is_ready.store(true, Ordering::Relaxed);
        let response = app
            .oneshot(Request::get("/ready").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_metrics_endpoint() {
        let mut registry = Registry::default();
        // Register a test counter
        let counter = prometheus_client::metrics::counter::Counter::<u64>::default();
        registry.register("test_counter", "A test counter", counter.clone());
        counter.inc();

        let registry = Arc::new(registry);
        let is_ready = Arc::new(AtomicBool::new(true));
        let app = metrics_router(registry, is_ready);

        let response = app
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let content_type = response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(content_type.contains("openmetrics-text"));

        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("test_counter"));
    }

    #[tokio::test]
    async fn test_spawn_metrics_server() {
        let registry = Arc::new(Registry::default());
        let is_ready = Arc::new(AtomicBool::new(true));
        let (tx, rx) = tokio::sync::oneshot::channel();

        let handle = spawn_metrics_server("127.0.0.1:0".parse().unwrap(), registry, is_ready, rx)
            .await
            .unwrap();

        // Signal shutdown
        let _ = tx.send(());
        handle.await.unwrap();
    }
}
