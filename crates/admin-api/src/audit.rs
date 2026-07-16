//! Append-only audit trail. Every mutation writes exactly one row inside
//! the same transaction as the change it describes.

use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub struct AuditEntry<'a> {
    pub tenant_id: Option<Uuid>,
    pub actor: &'a str,
    pub action: &'a str,
    pub entity_type: &'a str,
    pub entity_id: String,
    pub payload: serde_json::Value,
}

pub async fn record(
    tx: &mut Transaction<'_, Postgres>,
    entry: AuditEntry<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO audit_log (tenant_id, actor, action, entity_type, entity_id, payload)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(entry.tenant_id)
    .bind(entry.actor)
    .bind(entry.action)
    .bind(entry.entity_type)
    .bind(entry.entity_id)
    .bind(entry.payload)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
