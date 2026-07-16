// Leptos admin-console SSR entry — axum + leptos_axum.

use axum::{
    routing::{get, post},
    Router,
};
use leptos::config::LeptosOptions;
use leptos::prelude::*;
use leptos_axum::{file_and_error_handler, handle_server_fns, render_app_to_stream};

mod app;

/// Bind on 0.0.0.0 — behind a k8s Service, 127.0.0.1 is unreachable from
/// the kubelet (probes) and from other pods. `PORT` env overrides.
fn listen_addr() -> std::net::SocketAddr {
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3001);
    std::net::SocketAddr::from(([0, 0, 0, 0], port))
}

async fn health() -> &'static str {
    "ok"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = listen_addr();

    let options = LeptosOptions::builder()
        .site_addr(addr)
        .output_name("admin-console")
        .build();

    fn shell(_opts: LeptosOptions) -> impl IntoView {
        view! {
            <!DOCTYPE html>
            <html lang="en">
                <head><meta charset="utf-8"/></head>
                <body>
                    <app::App />
                </body>
            </html>
        }
    }

    let app = Router::new()
        .route("/", get(render_app_to_stream(app::App)))
        .route("/accounts", get(render_app_to_stream(app::App)))
        .route("/quotas", get(render_app_to_stream(app::App)))
        .route("/audit", get(render_app_to_stream(app::App)))
        // axum 0.8 wildcard syntax is `{*name}` — bare `*name` panics at
        // router construction.
        .route("/api/{*fn_name}", post(handle_server_fns))
        .route("/health", get(health))
        .fallback(file_and_error_handler(shell))
        .with_state(options.clone());

    tracing::info!("admin-console listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
