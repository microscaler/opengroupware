//! admin-console SSR entry — axum, read-only views over admin-api.
//!
//! Each page fetches from admin-api and renders a Leptos view to HTML server
//! side (no hydration/wasm). Env:
//!   * `PORT`               — listen port (default 3001).
//!   * `ADMIN_API_URL`      — admin-api base (default http://127.0.0.1:8080).
//!   * `ADMIN_CONSOLE_ACTOR`— dev x-actor header value.

mod app;
mod client;

use std::net::SocketAddr;

use axum::extract::{Path, State};
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use uuid::Uuid;

use client::AdminApi;

#[derive(Clone)]
struct AppState {
    api: AdminApi,
}

/// Bind on 0.0.0.0 — behind a k8s Service, 127.0.0.1 is unreachable from the
/// kubelet (probes) and other pods. `PORT` overrides.
fn listen_addr() -> SocketAddr {
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3001);
    SocketAddr::from(([0, 0, 0, 0], port))
}

async fn health() -> &'static str {
    "ok"
}

async fn tenants_page(State(state): State<AppState>) -> Html<String> {
    let html = match state.api.tenants().await {
        Ok(tenants) => app::render_tenants_page(tenants),
        Err(e) => app::render_error_page("Tenants", &e),
    };
    Html(html)
}

async fn tenant_detail_page(State(state): State<AppState>, Path(id): Path<Uuid>) -> Html<String> {
    let tenant = match state.api.tenants().await {
        Ok(tenants) => tenants.into_iter().find(|t| t.id == id),
        Err(e) => return Html(app::render_error_page("Tenant", &e)),
    };
    let Some(tenant) = tenant else {
        return Html(app::render_error_page(
            "Tenant not found",
            "No tenant with that id.",
        ));
    };

    // Independent reads — fetch concurrently.
    let (domains, accounts, policy) = tokio::join!(
        state.api.domains(id),
        state.api.accounts(id),
        state.api.policy(id)
    );

    let policy = match policy {
        Ok(p) => p,
        Err(e) => return Html(app::render_error_page(&tenant.name, &e)),
    };
    Html(app::render_detail_page(
        tenant,
        domains.unwrap_or_default(),
        accounts.unwrap_or_default(),
        policy,
    ))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = listen_addr();
    let state = AppState {
        api: AdminApi::from_env(),
    };

    let router = Router::new()
        .route("/", get(tenants_page))
        .route("/tenants/{id}", get(tenant_detail_page))
        .route("/health", get(health))
        .with_state(state);

    tracing::info!("admin-console listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;
    Ok(())
}
