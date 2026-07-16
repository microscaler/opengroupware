// Stalwart mail provider — concrete implementation of MailProvider.

use chrono::Utc;
use types::providers::MailProvider;
use types::providers::{Attachment, CalendarEvent, Contact, SieveScript};
use types::{
    Account, ListResponse, Mailbox, Message, MessageFlags, Page, ProviderContext, ProviderError,
    RawMessage, Resource, SearchResult, Uuid,
};

use crate::stalwart::StalwartClient;

/// Concrete implementation of MailProvider backed by a Stalwart Mail Server.
pub struct StalwartMailProvider {
    client: StalwartClient,
}

impl StalwartMailProvider {
    #[must_use]
    pub const fn new(api_url: String, admin_api_key: String) -> Self {
        Self {
            client: StalwartClient::new(api_url, admin_api_key),
        }
    }
}

#[async_trait::async_trait]
impl MailProvider for StalwartMailProvider {
    async fn create_tenant(&self, ctx: &ProviderContext) -> Result<(), ProviderError> {
        self.client.tenant_create(ctx).await
    }

    async fn suspend_tenant(&self, ctx: &ProviderContext) -> Result<(), ProviderError> {
        self.client.tenant_suspend(ctx).await
    }

    async fn terminate_tenant(&self, ctx: &ProviderContext) -> Result<(), ProviderError> {
        self.client.tenant_terminate(ctx).await
    }

    async fn create_account(
        &self,
        ctx: &ProviderContext,
        email: &str,
        display_name: &str,
    ) -> Result<Uuid, ProviderError> {
        self.client.account_create(ctx, email, display_name).await
    }

    async fn delete_account(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
    ) -> Result<(), ProviderError> {
        self.client.account_delete(ctx, account_id).await
    }

    async fn list_accounts(
        &self,
        ctx: &ProviderContext,
        cursor: Option<String>,
        limit: u32,
    ) -> Result<ListResponse<Account>, ProviderError> {
        self.client.account_list(ctx, cursor, limit).await
    }

    async fn create_mailbox(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        name: &str,
        parent_path: Option<&str>,
    ) -> Result<Uuid, ProviderError> {
        self.client
            .mailbox_create(ctx, account_id, name, parent_path)
            .await
    }

    async fn delete_mailbox(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        path: &str,
    ) -> Result<(), ProviderError> {
        self.client.mailbox_delete(ctx, account_id, path).await
    }

    async fn list_mailboxes(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
    ) -> Result<Vec<Mailbox>, ProviderError> {
        self.client.mailbox_list(ctx, account_id).await
    }

    async fn store_message(
        &self,
        ctx: &ProviderContext,
        raw: RawMessage,
    ) -> Result<Uuid, ProviderError> {
        self.client.message_store(ctx, raw).await
    }

    async fn get_message(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        message_id: Uuid,
    ) -> Result<Message, ProviderError> {
        self.client.message_get(ctx, account_id, message_id).await
    }

    async fn update_message_flags(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        message_id: Uuid,
        flags: MessageFlags,
    ) -> Result<(), ProviderError> {
        self.client
            .message_update_flags(ctx, account_id, message_id, flags)
            .await
    }

    async fn delete_message(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        message_id: Uuid,
    ) -> Result<(), ProviderError> {
        self.client
            .message_delete(ctx, account_id, message_id)
            .await
    }

    async fn search_messages(
        &self,
        ctx: &ProviderContext,
        query: &str,
        from: Option<&str>,
        to: Option<&str>,
        subject: Option<&str>,
        date_from: Option<chrono::DateTime<Utc>>,
        date_to: Option<chrono::DateTime<Utc>>,
        limit: u32,
    ) -> Result<Page<SearchResult>, ProviderError> {
        self.client
            .message_search(ctx, query, from, to, subject, date_from, date_to, limit)
            .await
    }

    async fn list_attachments(
        &self,
        ctx: &ProviderContext,
        message_id: Uuid,
    ) -> Result<Vec<Attachment>, ProviderError> {
        self.client.attachment_list(ctx, message_id).await
    }

    async fn get_attachment(
        &self,
        ctx: &ProviderContext,
        blob_key: &str,
    ) -> Result<Vec<u8>, ProviderError> {
        self.client.attachment_get(ctx, blob_key).await
    }

    async fn create_event(
        &self,
        ctx: &ProviderContext,
        event: CalendarEvent,
    ) -> Result<Uuid, ProviderError> {
        self.client.event_create(ctx, event).await
    }

    async fn get_event(
        &self,
        ctx: &ProviderContext,
        event_id: Uuid,
    ) -> Result<CalendarEvent, ProviderError> {
        self.client.event_get(ctx, event_id).await
    }

    async fn list_events(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        start: chrono::DateTime<Utc>,
        end: chrono::DateTime<Utc>,
    ) -> Result<Vec<CalendarEvent>, ProviderError> {
        self.client.event_list(ctx, account_id, start, end).await
    }

    async fn create_contact(
        &self,
        ctx: &ProviderContext,
        contact: Contact,
    ) -> Result<Uuid, ProviderError> {
        self.client.contact_create(ctx, contact).await
    }

    async fn list_contacts(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
    ) -> Result<Vec<Contact>, ProviderError> {
        self.client.contact_list(ctx, account_id).await
    }

    async fn create_resource(
        &self,
        ctx: &ProviderContext,
        resource: Resource,
    ) -> Result<Uuid, ProviderError> {
        self.client.resource_create(ctx, resource).await
    }

    async fn book_resource(
        &self,
        ctx: &ProviderContext,
        resource_id: Uuid,
        event_id: Uuid,
        start: chrono::DateTime<Utc>,
        end: chrono::DateTime<Utc>,
    ) -> Result<Uuid, ProviderError> {
        self.client
            .resource_book(ctx, resource_id, event_id, start, end)
            .await
    }

    async fn cancel_booking(
        &self,
        ctx: &ProviderContext,
        booking_id: Uuid,
    ) -> Result<(), ProviderError> {
        self.client.booking_cancel(ctx, booking_id).await
    }

    async fn install_sieve_script(
        &self,
        ctx: &ProviderContext,
        script: SieveScript,
    ) -> Result<(), ProviderError> {
        self.client.sieve_install(ctx, script).await
    }

    async fn list_sieve_scripts(
        &self,
        ctx: &ProviderContext,
        mailbox_id: Uuid,
    ) -> Result<Vec<SieveScript>, ProviderError> {
        self.client.sieve_list(ctx, mailbox_id).await
    }
}
