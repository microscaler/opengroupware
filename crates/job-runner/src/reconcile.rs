//! Account provisioning reconciliation.
//!
//! admin-api creates accounts `pending_provisioning` and best-effort
//! provisions them into sesame-idam after commit; a sesame outage leaves the
//! row pending (admin-api: "reconciliation can retry"). This worker is that
//! retry: it periodically finds pending accounts and provisions them, flips
//! them `active`, and audits the transition — closing the loop.
//!
//! Network I/O (sesame) happens OUTSIDE any DB transaction: we read a batch
//! in one short tx, provision each over HTTP, then persist each result in its
//! own short tx. No connection is held open across a network call.

use std::future::Future;

use og_db::{record, AuditEntry, Db};
use types::ProviderError;
use uuid::Uuid;

/// Provisions a mail identity into the identity plane. Abstracted so the
/// reconciliation logic is testable with a fake (the production impl is the
/// real sesame HTTP client).
///
/// The desugared `impl Future + Send` form (rather than `async fn`) plus the
/// `Sync` supertrait let `reconcile_once`'s future stay `Send`, so the worker
/// loop can run under `tokio::spawn`.
pub trait Provisioner: Sync {
    fn provision<'a>(
        &'a self,
        email: &'a str,
        display_name: &'a str,
    ) -> impl Future<Output = Result<Uuid, ProviderError>> + Send + 'a;
}

impl Provisioner for wrappers::sesame_client::SesameClient {
    // `async fn` satisfies the trait's `impl Future + Send` bound because the
    // concrete future is `Send`; this form keeps clippy's `manual_async_fn`
    // happy while the trait declaration retains the explicit `Send` bound.
    async fn provision(&self, email: &str, display_name: &str) -> Result<Uuid, ProviderError> {
        self.provision_user(email, display_name).await
    }
}

type PendingRow = (Uuid, Uuid, String, String); // id, tenant_id, email, display_name

/// Run one reconciliation cycle. Returns the number of accounts activated.
///
/// # Errors
/// Returns [`sqlx::Error`] on a database failure; individual sesame failures
/// are logged and left pending for the next cycle (not propagated).
pub async fn reconcile_once<P: Provisioner>(
    db: &Db,
    provisioner: &P,
    batch: i64,
) -> Result<usize, sqlx::Error> {
    // 1. Read a batch of pending accounts (platform scope; short read tx).
    let pending: Vec<PendingRow> = {
        let mut tx = db.platform_tx().await?;
        let rows = sqlx::query_as(
            "SELECT id, tenant_id, email, display_name
             FROM accounts
             WHERE status = 'pending_provisioning'
             ORDER BY created_at
             LIMIT $1",
        )
        .bind(batch)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        rows
    };

    let mut activated = 0usize;
    for (id, tenant_id, email, display_name) in pending {
        // 2. Provision over HTTP — no DB tx held here.
        let sesame_user_id = match provisioner.provision(&email, &display_name).await {
            Ok(uid) => uid,
            Err(e) => {
                tracing::warn!(account = %id, error = %e, "reconcile: provisioning still failing; will retry");
                continue;
            }
        };

        // 3. Persist activation + audit in its own short tx. The status guard
        //    makes this idempotent against a concurrent activation.
        let mut tx = db.platform_tx().await?;
        let updated = sqlx::query(
            "UPDATE accounts
             SET status = 'active', sesame_user_id = $2, updated_at = now()
             WHERE id = $1 AND status = 'pending_provisioning'",
        )
        .bind(id)
        .bind(sesame_user_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        if updated == 1 {
            record(
                &mut tx,
                AuditEntry {
                    tenant_id: Some(tenant_id),
                    actor: "job-runner",
                    action: "account.reconciled",
                    entity_type: "account",
                    entity_id: id.to_string(),
                    payload: serde_json::json!({ "sesame_user_id": sesame_user_id }),
                },
            )
            .await?;
            activated += 1;
        }
        tx.commit().await?;
    }
    Ok(activated)
}
