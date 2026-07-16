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
    if std::env::var("RUN_MIGRATIONS")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        db.migrate().await?;
        tracing::info!("migrations applied");
    }
    tracing::info!("database connected");

    let app = routes::router(AppState { db });
    service_kit::run("admin-api", 8080, app).await?;
    Ok(())
}
