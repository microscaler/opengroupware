//! admin-api service entrypoint — control-plane provisioning (slice 1).
//!
//! Requires DATABASE_URL. Migrations run at startup (idempotent; the k8s
//! bootstrap migration-job runs the same set).

mod audit;
mod db;
mod error;
mod models;
mod routes;

use routes::AppState;

#[derive(Debug, thiserror::Error)]
enum MainError {
    #[error("DATABASE_URL is not set")]
    MissingDatabaseUrl,
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Service(#[from] service_kit::ServiceError),
}

#[tokio::main]
async fn main() -> Result<(), MainError> {
    service_kit::init_tracing();
    let url = std::env::var("DATABASE_URL").map_err(|_| MainError::MissingDatabaseUrl)?;
    let db = db::Db::connect(&url).await?;
    let flag = |k: &str| std::env::var(k).map(|v| v == "1").unwrap_or(false);
    if flag("RUN_MIGRATIONS") || flag("MIGRATE_ONLY") {
        db.migrate().await?;
        tracing::info!("migrations applied");
    }
    if flag("MIGRATE_ONLY") {
        // k8s bootstrap Job mode: migrate and exit 0 — never serve.
        return Ok(());
    }
    tracing::info!("database connected");

    let sesame = wrappers::sesame_client::SesameConfig::from_env().map(|cfg| {
        tracing::info!(tenant = %cfg.tenant, "sesame-idam integration enabled");
        std::sync::Arc::new(wrappers::sesame_client::SesameClient::new(cfg))
    });
    if sesame.is_none() {
        tracing::warn!(
            "SESAME_* env not set — accounts will stay pending_provisioning (no identity plane)"
        );
    }

    let app = routes::router(AppState { db, sesame });
    service_kit::run("admin-api", 8080, app).await?;
    Ok(())
}
