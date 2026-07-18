//! job-runner service entrypoint.
//!
//! Runs background reconciliation for the control plane while serving
//! `/health` + `/metrics` (from service-kit) so the k8s manifests stay
//! truthful. The first real job is account-provisioning reconciliation: it
//! retries sesame provisioning for accounts admin-api left
//! `pending_provisioning` after a best-effort provision failed on the request
//! path. See `reconcile.rs`.
//!
//! Env:
//!   * `DATABASE_URL`               — control-plane DB (same as admin-api).
//!   * `SESAME_*`                   — identity plane (see wrappers::SesameConfig).
//!   * `RECONCILE_INTERVAL_SECS`    — cycle period (default 30).
//!   * `RECONCILE_BATCH`            — accounts per cycle (default 50).
//!   * `RECONCILE_ONCE=1`           — run exactly one cycle, then exit (smoke).

mod reconcile;

use std::time::Duration;

use axum::Router;
use og_db::Db;
use wrappers::sesame_client::{SesameClient, SesameConfig};

#[derive(Debug, thiserror::Error)]
enum StartupError {
    #[error("DATABASE_URL must be set")]
    NoDatabaseUrl,
    #[error("database: {0}")]
    Db(#[from] sqlx::Error),
    #[error("service: {0}")]
    Service(#[from] service_kit::ServiceError),
}

#[tokio::main]
async fn main() -> Result<(), StartupError> {
    service_kit::init_tracing();

    let database_url = std::env::var("DATABASE_URL").map_err(|_| StartupError::NoDatabaseUrl)?;
    let db = Db::connect(&database_url).await?;

    // sesame is required for reconciliation; without it we still serve
    // health/metrics but do no work (and say so, loudly).
    let sesame = SesameConfig::from_env().map(SesameClient::new);

    let batch: i64 = env_parse("RECONCILE_BATCH", 50);
    let once = std::env::var("RECONCILE_ONCE").is_ok_and(|v| v == "1" || v == "true");

    match sesame {
        Some(sesame) if once => {
            // One-shot mode: run a single cycle and exit (used by the smoke).
            let n = reconcile::reconcile_once(&db, &sesame, batch).await?;
            tracing::info!(reconciled = n, "reconcile: one-shot cycle complete");
            return Ok(());
        }
        Some(sesame) => {
            // Move the (non-Clone) client into the worker; clone the DB handle.
            let db = db.clone();
            let interval = Duration::from_secs(env_parse::<u64>("RECONCILE_INTERVAL_SECS", 30));
            tokio::spawn(async move {
                loop {
                    match reconcile::reconcile_once(&db, &sesame, batch).await {
                        Ok(0) => {}
                        Ok(n) => tracing::info!(reconciled = n, "reconcile: activated accounts"),
                        Err(e) => tracing::error!(error = %e, "reconcile: cycle failed"),
                    }
                    tokio::time::sleep(interval).await;
                }
            });
        }
        None if once => {
            tracing::warn!("RECONCILE_ONCE set but SESAME_* unconfigured; nothing to do");
            return Ok(());
        }
        None => {
            tracing::warn!("SESAME_* unconfigured — reconciliation disabled; serving health only");
        }
    }

    let app = Router::new();
    service_kit::run("job-runner", 8082, app).await?;
    Ok(())
}

/// Parse an env var into `T`, falling back to `default` when unset or invalid.
fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
