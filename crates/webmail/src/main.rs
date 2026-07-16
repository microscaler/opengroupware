// Leptos webmail SSR entry — axum + leptos_axum.

use axum::{Router, routing::{get, post}};
use leptos::config::LeptosOptions;
use leptos::prelude::*;
use leptos_axum::{render_app_to_stream, handle_server_fns, file_and_error_handler};

mod app;

#[tokio::main]
async fn main() {
    let options = LeptosOptions::builder()
        .site_addr("127.0.0.1:3000".parse::<std::net::SocketAddr>().unwrap())
        .output_name("webmail")
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
        .route("/compose", get(render_app_to_stream(app::App)))
        .route("/contacts", get(render_app_to_stream(app::App)))
        .route("/settings/{*path}", get(render_app_to_stream(app::App)))
        .route("/api/*fn_name", post(handle_server_fns))
        .fallback(file_and_error_handler(shell))
        .with_state(options.clone());

    tracing::info!("webmail listening on http://127.0.0.1:3000");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
