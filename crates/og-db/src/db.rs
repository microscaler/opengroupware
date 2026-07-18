//! Database access — the ONLY way service code touches Postgres.
//!
//! D4 (docs/13): RLS is keyed off a per-transaction GUC. `set_config(...,
//! true)` == `SET LOCAL`, so the setting dies with the transaction and can
//! never leak across PgBouncer transaction-pooled connections. Handlers must
//! obtain connections exclusively through [`Db::tenant_tx`] /
//! [`Db::platform_tx`] — never from a raw pool.
//!
//! Safety guard: RLS does not apply to superusers or BYPASSRLS roles, so a
//! privileged connection silently disables tenant isolation (verified live
//! in the slice-1 smoke test). [`Db::connect`] fails closed on a privileged
//! role unless OPENGROUPWARE_ALLOW_UNSAFE_DB=1 (dev escape hatch only).

use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Clone)]
pub struct Db {
    pool: PgPool,
}

impl Db {
    /// Connect and verify the role is safe for RLS enforcement.
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(url)
            .await?;
        let db = Self { pool };
        db.assert_rls_safe_role().await?;
        Ok(db)
    }

    /// Run the caller's migrations. Each service passes its own
    /// `sqlx::migrate!("./migrations")` (the macro resolves relative to the
    /// calling crate). Invoked only in the migration Job / dev bootstrap.
    pub async fn run_migrations(&self, migrator: &Migrator) -> Result<(), sqlx::Error> {
        migrator
            .run(&self.pool)
            .await
            .map_err(|e| sqlx::Error::Migrate(Box::new(e)))
    }

    async fn assert_rls_safe_role(&self) -> Result<(), sqlx::Error> {
        let row =
            sqlx::query("SELECT rolsuper, rolbypassrls FROM pg_roles WHERE rolname = current_user")
                .fetch_one(&self.pool)
                .await?;
        let superuser: bool = row.try_get("rolsuper")?;
        let bypass: bool = row.try_get("rolbypassrls")?;
        if !(superuser || bypass) {
            return Ok(());
        }
        let allowed = std::env::var("OPENGROUPWARE_ALLOW_UNSAFE_DB")
            .map(|v| v == "1")
            .unwrap_or(false);
        if allowed {
            tracing::warn!(
                superuser,
                bypass,
                "UNSAFE: privileged DB role — RLS tenant isolation is INACTIVE \
                 (OPENGROUPWARE_ALLOW_UNSAFE_DB=1)"
            );
            return Ok(());
        }
        Err(sqlx::Error::Configuration(
            format!(
                "connecting role is privileged (superuser={superuser}, bypassrls={bypass}); \
                 RLS would be bypassed. Connect as a role granted opengroupware_app, or set \
                 OPENGROUPWARE_ALLOW_UNSAFE_DB=1 in development only."
            )
            .into(),
        ))
    }

    /// Begin a transaction scoped to one tenant. Every query inside sees
    /// only that tenant's rows (RLS policies).
    pub async fn tenant_tx(
        &self,
        tenant_id: Uuid,
    ) -> Result<Transaction<'_, Postgres>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT set_config('app.current_tenant_id', $1, true)")
            .bind(tenant_id.to_string())
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }

    /// Begin a platform-admin transaction (cross-tenant reads / lifecycle).
    /// Audited by callers; the flag is transaction-local.
    pub async fn platform_tx(&self) -> Result<Transaction<'_, Postgres>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT set_config('app.is_platform_admin', 'true', true)")
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }
}
