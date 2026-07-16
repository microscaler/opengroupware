// Stalwart backup provider — concrete implementation of BackupProvider.

use async_trait::async_trait;
use chrono::Utc;
use types::{Page, ProviderContext, ProviderError, Uuid};
use types::providers::{BackupJob, BackupStatus, ExportFormat};
use types::providers::BackupProvider;
use serde::{Deserialize, Serialize};

/// Concrete implementation of BackupProvider backed by Stalwart Mail Server.
pub struct StalwartBackupProvider {
    client_url: String,
    admin_api_key: String,
}

impl StalwartBackupProvider {
    pub fn new(client_url: String, admin_api_key: String) -> Self {
        Self { client_url, admin_api_key }
    }
}

#[async_trait::async_trait]
impl BackupProvider for StalwartBackupProvider {
    async fn start_backup(&self, ctx: &ProviderContext, account_id: Option<Uuid>) -> Result<Uuid, ProviderError> {
        // TODO: Call Stalwart export API
        let _ = (ctx, account_id);
        Ok(Uuid::new_v4())
    }

    async fn cancel_backup(&self, ctx: &ProviderContext, job_id: Uuid) -> Result<(), ProviderError> {
        // TODO: Cancel Stalwart export job
        let _ = (ctx, job_id);
        Ok(())
    }

    async fn get_backup_status(&self, ctx: &ProviderContext, job_id: Uuid) -> Result<BackupJob, ProviderError> {
        // TODO: Get Stalwart export job status
        let _ = (ctx, job_id);
        todo!("Not implemented")
    }

    async fn start_restore(&self, ctx: &ProviderContext, account_id: Uuid, backup_job_id: Uuid) -> Result<Uuid, ProviderError> {
        // TODO: Import from Stalwart backup
        let _ = (ctx, account_id, backup_job_id);
        Ok(Uuid::new_v4())
    }

    async fn list_backups(&self, ctx: &ProviderContext) -> Result<Vec<BackupJob>, ProviderError> {
        // TODO: List Stalwart exports
        let _ = ctx;
        Ok(vec![])
    }

    async fn export_mailbox(&self, ctx: &ProviderContext, account_id: Uuid, format: ExportFormat) -> Result<Uuid, ProviderError> {
        // TODO: Export to PST/MBOX/EML via Stalwart
        let _ = (ctx, account_id, format);
        Ok(Uuid::new_v4())
    }

    async fn import_mailbox(&self, ctx: &ProviderContext, account_id: Uuid, source_blob_key: &str) -> Result<Uuid, ProviderError> {
        // TODO: Import from PST/MBOX blob
        let _ = (ctx, account_id, source_blob_key);
        Ok(Uuid::new_v4())
    }
}
