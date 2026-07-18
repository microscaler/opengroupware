//! config-compiler service entrypoint.
//!
//! Translates control-plane desired state into Rspamd `settings` config
//! (ADR-0003: no hand-edited production configs), validating structure against
//! a JSON Schema before writing (docs/02 pipeline step 2). Serves `/health` +
//! `/metrics` via service-kit alongside the compile loop.
//!
//! Env:
//!   * `DATABASE_URL` — control-plane DB (same as admin-api).
//!   * `RSPAMD_SETTINGS_PATH` — output UCL path
//!     (default `/etc/opengroupware/rspamd/settings.conf`).
//!   * `RENDER_INTERVAL_SECS` — compile cadence (default 60).
//!   * `RENDER_ONCE=1` — compile once, then exit (smoke).

mod render;

use std::time::Duration;

use axum::Router;
use og_db::Db;

#[derive(Debug, thiserror::Error)]
enum StartupError {
    #[error("DATABASE_URL must be set")]
    NoDatabaseUrl,
    #[error("database: {0}")]
    Db(#[from] sqlx::Error),
    #[error("compile: {0}")]
    Compile(#[from] render::CompileError),
    #[error("service: {0}")]
    Service(#[from] service_kit::ServiceError),
}

#[tokio::main]
async fn main() -> Result<(), StartupError> {
    service_kit::init_tracing();

    let database_url = std::env::var("DATABASE_URL").map_err(|_| StartupError::NoDatabaseUrl)?;
    let db = Db::connect(&database_url).await?;
    let out_path = std::env::var("RSPAMD_SETTINGS_PATH")
        .unwrap_or_else(|_| "/etc/opengroupware/rspamd/settings.conf".to_string());
    let once = std::env::var("RENDER_ONCE").is_ok_and(|v| v == "1" || v == "true");

    if once {
        let n = render::compile_once(&db, &out_path).await?;
        tracing::info!(tenants = n, path = %out_path, "config-compiler: one-shot compile complete");
        return Ok(());
    }

    let interval = Duration::from_secs(env_parse::<u64>("RENDER_INTERVAL_SECS", 60));
    {
        let db = db.clone();
        let out_path = out_path.clone();
        tokio::spawn(async move {
            loop {
                match render::compile_once(&db, &out_path).await {
                    Ok(n) => {
                        tracing::info!(tenants = n, "config-compiler: compiled rspamd settings");
                    }
                    Err(e) => tracing::error!(error = %e, "config-compiler: compile cycle failed"),
                }
                tokio::time::sleep(interval).await;
            }
        });
    }

    let app = Router::new();
    service_kit::run("config-compiler", 8083, app).await?;
    Ok(())
}

/// Parse an env var into `T`, falling back to `default` when unset or invalid.
fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
