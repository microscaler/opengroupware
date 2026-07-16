// Storage provider interface — blob/object storage for messages and attachments.
//
// Every method takes a `ProviderContext` for multi-tenant scoping.

use crate::{BlobRef, Page, ProviderContext, ProviderError, Uuid};

#[async_trait::async_trait]
pub trait StorageProvider: Send + Sync {
    /// Upload a blob. Returns the blob key (S3 key / MinIO object key).
    async fn upload(
        &self,
        ctx: &ProviderContext,
        content: &[u8],
        content_type: &str,
        filename: Option<&str>,
    ) -> Result<Uuid, ProviderError>;

    /// Download a blob by key.
    async fn download(
        &self,
        ctx: &ProviderContext,
        blob_key: &str,
    ) -> Result<Vec<u8>, ProviderError>;

    /// Delete a blob by key.
    async fn delete(
        &self,
        ctx: &ProviderContext,
        blob_key: &str,
    ) -> Result<(), ProviderError>;

    /// List blobs for a tenant (with optional prefix and pagination).
    async fn list(
        &self,
        ctx: &ProviderContext,
        prefix: Option<&str>,
        cursor: Option<String>,
        limit: u32,
    ) -> Result<Page<BlobRef>, ProviderError>;

    /// Generate a signed URL for a blob (for temporary browser access).
    async fn signed_url(
        &self,
        ctx: &ProviderContext,
        blob_key: &str,
        expiry_seconds: u32,
    ) -> Result<String, ProviderError>;

    /// Get blob metadata by key.
    async fn metadata(
        &self,
        ctx: &ProviderContext,
        blob_key: &str,
    ) -> Result<BlobRef, ProviderError>;
}
