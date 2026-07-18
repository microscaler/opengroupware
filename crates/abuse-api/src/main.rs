//! abuse-api service entrypoint — abuse/quarantine workflow (slice 2).
//!
//! Requires DATABASE_URL. MIGRATE_ONLY=1 migrates and exits (k8s bootstrap
//! Job); otherwise migrates-then-serves in dev when RUN_MIGRATIONS=1.

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
    let db = og_db::Db::connect(&url).await?;
    let flag = |k: &str| std::env::var(k).map(|v| v == "1").unwrap_or(false);
    if flag("RUN_MIGRATIONS") || flag("MIGRATE_ONLY") {
        db.run_migrations(&sqlx::migrate!("./migrations")).await?;
        tracing::info!("migrations applied");
    }
    if flag("MIGRATE_ONLY") {
        return Ok(());
    }
    tracing::info!("database connected");

    let auth = og_auth::Authenticator::new(og_auth::AuthConfig::from_env());
    if !auth.enforcing() {
        tracing::warn!("OG_AUTH_JWKS_URL unset — trusting x-actor header (dev only)");
    }

    let app = routes::router(AppState { db, auth });
    service_kit::run("abuse-api", 8081, app).await?;
    Ok(())
}
