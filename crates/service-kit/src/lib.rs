//! Shared service bootstrap for all OpenGroupware services.
//!
//! Every service binary calls [`run`] with its name, default port, and
//! application router. `run` provides, uniformly:
//! - `GET /health` (readiness/liveness — the k8s manifests probe this)
//! - `GET /metrics` (Prometheus exposition; minimal until real metrics land)
//! - binding on `0.0.0.0` (services run behind a k8s Service; `127.0.0.1`
//!   is unreachable from the kubelet and other pods)
//! - `PORT` env override, tracing init, graceful shutdown on SIGTERM/ctrl-c

use axum::{routing::get, Router};

/// Errors from service bootstrap.
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("failed to bind {addr}: {source}")]
    Bind {
        addr: String,
        source: std::io::Error,
    },
    #[error("server error: {0}")]
    Serve(#[from] std::io::Error),
}

/// Health endpoint payload — deliberately boring.
async fn health() -> &'static str {
    "ok"
}

/// Prometheus exposition. Minimal `up` gauge until real metrics are wired;
/// exists so scrape configs and annotations are truthful from day one.
async fn metrics() -> ([(&'static str, &'static str); 1], String) {
    (
        [("content-type", "text/plain; version=0.0.4")],
        "# TYPE opengroupware_up gauge\nopengroupware_up 1\n".to_string(),
    )
}

/// Initialise tracing from `RUST_LOG` (default `info`).
pub fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    // `try_init` so tests that call `run` twice don't panic.
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

/// Resolve the port: `PORT` env var wins, otherwise the service default.
fn resolve_port(default_port: u16) -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(default_port)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if tokio::signal::ctrl_c().await.is_err() {
            tracing::warn!("failed to install ctrl-c handler");
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => tracing::warn!(error = %e, "failed to install SIGTERM handler"),
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutdown signal received");
}

/// Run a service: mounts `/health` + `/metrics` alongside `app`, binds
/// `0.0.0.0:{PORT|default_port}`, serves until SIGTERM/ctrl-c.
pub async fn run(service_name: &str, default_port: u16, app: Router) -> Result<(), ServiceError> {
    init_tracing();
    let port = resolve_port(default_port);
    let addr = format!("0.0.0.0:{port}");

    let router = app
        .route("/health", get(health))
        .route("/metrics", get(metrics));

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|source| ServiceError::Bind {
            addr: addr.clone(),
            source,
        })?;

    tracing::info!(service = service_name, %addr, "listening");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}
