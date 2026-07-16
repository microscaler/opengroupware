// Search provider interface — message and content indexing.
//
// Every method takes a `ProviderContext` for multi-tenant scoping.

use crate::{DateTime, Message, Page, ProviderContext, ProviderError, SearchResult, Uuid};
use chrono::Utc;

#[async_trait::async_trait]
pub trait SearchProvider: Send + Sync {
    /// Index a message for full-text search.
    async fn index_message(
        &self,
        ctx: &ProviderContext,
        message: &Message,
        body: &str,
    ) -> Result<(), ProviderError>;

    /// Remove a message from the search index.
    async fn remove_message(
        &self,
        ctx: &ProviderContext,
        message_id: Uuid,
    ) -> Result<(), ProviderError>;

    /// Search messages across a tenant's mailboxes.
    async fn search(
        &self,
        ctx: &ProviderContext,
        query: &str,
        from: Option<&str>,
        to: Option<&str>,
        subject: Option<&str>,
        date_from: Option<DateTime<Utc>>,
        date_to: Option<DateTime<Utc>>,
        limit: u32,
    ) -> Result<Page<SearchResult>, ProviderError>;

    /// Rebuild the search index for a mailbox (used after migration).
    async fn rebuild(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
    ) -> Result<(), ProviderError>;
}
