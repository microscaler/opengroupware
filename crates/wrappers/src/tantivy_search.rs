// Tantivy search provider — concrete implementation of SearchProvider.

use async_trait::async_trait;
use chrono::Utc;
use types::{Message, Page, ProviderContext, ProviderError, SearchResult, Uuid};
use types::providers::SearchProvider;
use serde::{Deserialize, Serialize};

/// Configuration for Tantivy search index.
#[derive(Debug, Clone)]
pub struct TantivyIndexConfig {
    pub data_path: String,
    pub max_docs_per_segment: usize,
}

/// Concrete implementation of SearchProvider backed by Tantivy (Rust search library).
pub struct TantivySearchProvider {
    config: TantivyIndexConfig,
}

impl TantivySearchProvider {
    pub fn new(data_path: String, max_docs_per_segment: usize) -> Self {
        Self {
            config: TantivyIndexConfig { data_path, max_docs_per_segment },
        }
    }
}

#[async_trait::async_trait]
impl SearchProvider for TantivySearchProvider {
    async fn index_message(&self, ctx: &ProviderContext, message: &Message, body: &str) -> Result<(), ProviderError> {
        // TODO: Index message in Tantivy index
        // Maps to: TANTIVY_INDEX_MESSAGE
        let _ = (ctx, message, body);
        Ok(())
    }

    async fn remove_message(&self, ctx: &ProviderContext, message_id: Uuid) -> Result<(), ProviderError> {
        // TODO: Remove from Tantivy index
        // Maps to: TANTIVY_REMOVE_MESSAGE
        let _ = (ctx, message_id);
        Ok(())
    }

    async fn search(&self, ctx: &ProviderContext, query: &str, from: Option<&str>, to: Option<&str>, subject: Option<&str>, date_from: Option<chrono::DateTime<Utc>>, date_to: Option<chrono::DateTime<Utc>>, limit: u32) -> Result<Page<SearchResult>, ProviderError> {
        // TODO: Query Tantivy index
        // Maps to: TANTIVY_SEARCH
        let _ = (ctx, query, from, to, subject, date_from, date_to, limit);
        Ok(Page { items: vec![], next_cursor: None, has_more: false })
    }

    async fn rebuild(&self, ctx: &ProviderContext, account_id: Uuid) -> Result<(), ProviderError> {
        // TODO: Rebuild Tantivy index for account
        // Maps to: TANTIVY_REBUILD
        let _ = (ctx, account_id);
        Ok(())
    }
}
