// MinIO storage provider — concrete implementation of StorageProvider.

use async_trait::async_trait;
use chrono::Utc;
use types::{BlobRef, Page, ProviderContext, ProviderError, Uuid};
use serde::{Deserialize, Serialize};

/// Concrete implementation of StorageProvider backed by MinIO/S3-compatible storage.
pub struct MinioStorageProvider {
    endpoint: String,
    bucket: String,
    access_key: String,
    secret_key: String,
    use_ssl: bool,
}

impl MinioStorageProvider {
    pub fn new(endpoint: String, bucket: String, access_key: String, secret_key: String, use_ssl: bool) -> Self {
        Self { endpoint, bucket, access_key, secret_key, use_ssl }
    }
}

#[async_trait::async_trait]
impl types::providers::StorageProvider for MinioStorageProvider {
    async fn upload(&self, ctx: &ProviderContext, content: &[u8], content_type: &str, filename: Option<&str>) -> Result<Uuid, ProviderError> {
        // TODO: Upload to S3/MinIO with tenant prefix
        // Maps to: MINIO_STORAGE_UPLOAD
        let _ = (content, content_type, filename);
        Ok(Uuid::new_v4())
    }

    async fn download(&self, ctx: &ProviderContext, blob_key: &str) -> Result<Vec<u8>, ProviderError> {
        // TODO: Download from S3/MinIO
        // Maps to: MINIO_STORAGE_DOWNLOAD
        let _ = blob_key;
        Ok(vec![])
    }

    async fn delete(&self, ctx: &ProviderContext, blob_key: &str) -> Result<(), ProviderError> {
        // TODO: Delete from S3/MinIO
        // Maps to: MINIO_STORAGE_DELETE
        let _ = blob_key;
        Ok(())
    }

    async fn list(&self, ctx: &ProviderContext, prefix: Option<&str>, cursor: Option<String>, limit: u32) -> Result<Page<BlobRef>, ProviderError> {
        // TODO: List objects from S3/MinIO with prefix
        // Maps to: MINIO_STORAGE_LIST
        let _ = (prefix, cursor, limit);
        Ok(Page { items: vec![], next_cursor: None, has_more: false })
    }

    async fn signed_url(&self, ctx: &ProviderContext, blob_key: &str, expiry_seconds: u32) -> Result<String, ProviderError> {
        // TODO: Generate presigned URL
        // Maps to: MINIO_STORAGE_SIGNED_URL
        let _ = (blob_key, expiry_seconds);
        Ok(format!("https://{}.{}.com/{}", self.endpoint, self.bucket, blob_key))
    }

    async fn metadata(&self, ctx: &ProviderContext, blob_key: &str) -> Result<BlobRef, ProviderError> {
        // TODO: Get object metadata from S3/MinIO
        // Maps to: MINIO_STORAGE_METADATA
        let _ = blob_key;
        todo!("Not implemented")
    }
}
