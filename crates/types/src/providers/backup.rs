// Backup provider interface — message and mailbox-level backup/restore.
//
// Every method takes a `ProviderContext` for multi-tenant scoping.

use crate::{DateTime, ProviderContext, ProviderError, Uuid};
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// A backup job tracked by the product.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupJob {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub account_id: Option<Uuid>, // None = full tenant backup
    pub status: BackupStatus,
    pub progress_percent: u8,
    pub message_count: u64,
    pub message_progress: u64,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackupStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[async_trait::async_trait]
pub trait BackupProvider: Send + Sync {
    /// Start a backup of a mailbox or full tenant.
    async fn start_backup(
        &self,
        ctx: &ProviderContext,
        account_id: Option<Uuid>,
    ) -> Result<Uuid, ProviderError>;

    /// Cancel an in-progress backup.
    async fn cancel_backup(
        &self,
        ctx: &ProviderContext,
        job_id: Uuid,
    ) -> Result<(), ProviderError>;

    /// Get backup job status.
    async fn get_backup_status(
        &self,
        ctx: &ProviderContext,
        job_id: Uuid,
    ) -> Result<BackupJob, ProviderError>;

    /// Start a restore of a mailbox from a backup.
    async fn start_restore(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        backup_job_id: Uuid,
    ) -> Result<Uuid, ProviderError>;

    /// List backups for a tenant.
    async fn list_backups(
        &self,
        ctx: &ProviderContext,
    ) -> Result<Vec<BackupJob>, ProviderError>;

    /// Export a mailbox to PST/MBOX format.
    async fn export_mailbox(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        format: ExportFormat,
    ) -> Result<Uuid, ProviderError>;

    /// Import a mailbox from PST/MBOX.
    async fn import_mailbox(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        source_blob_key: &str,
    ) -> Result<Uuid, ProviderError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Pst,
    Mbox,
    Eml,
}
