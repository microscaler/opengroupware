// Stalwart client — HTTP client for Stalwart Mail Server JMAP API.

use chrono::Utc;
use types::providers::{CalendarEvent, Contact, RawMessage, Resource};
use types::{
    Account, ListResponse, Mailbox, Message, MessageFlags, Page, ProviderContext, ProviderError,
    SearchResult, Uuid,
};

/// Configuration for connecting to Stalwart JMAP API.
#[derive(Debug, Clone)]
pub struct StalwartClient {
    api_url: String,
    admin_api_key: String,
}

impl StalwartClient {
    #[must_use]
    pub const fn new(api_url: String, admin_api_key: String) -> Self {
        Self {
            api_url,
            admin_api_key,
        }
    }

    fn tenant_path(&self, tenant_id: Uuid) -> String {
        format!("{}/api/tenants/{}", self.api_url, tenant_id)
    }

    // --- Tenant operations ---

    pub async fn tenant_create(&self, ctx: &ProviderContext) -> Result<(), ProviderError> {
        // TODO: POST /api/tenants with tenant config
        // Maps to: STALWART_TENANT_CREATE
        let _ = ctx;
        Ok(())
    }

    pub async fn tenant_suspend(&self, _ctx: &ProviderContext) -> Result<(), ProviderError> {
        // TODO: PATCH /api/tenants/:id with status=suspended
        // Maps to: STALWART_TENANT_SUSPEND
        Ok(())
    }

    pub async fn tenant_terminate(&self, _ctx: &ProviderContext) -> Result<(), ProviderError> {
        // TODO: DELETE /api/tenants/:id
        // Maps to: STALWART_TENANT_TERMINATE
        Ok(())
    }

    // --- Account operations ---

    pub async fn account_create(
        &self,
        _ctx: &ProviderContext,
        email: &str,
        display_name: &str,
    ) -> Result<Uuid, ProviderError> {
        // TODO: POST /api/accounts with email, name, tenant_id
        // Maps to: STALWART_ACCOUNT_CREATE
        let _ = (email, display_name);
        Ok(Uuid::new_v4())
    }

    pub async fn account_delete(
        &self,
        _ctx: &ProviderContext,
        account_id: Uuid,
    ) -> Result<(), ProviderError> {
        // TODO: DELETE /api/accounts/:id
        // Maps to: STALWART_ACCOUNT_DELETE
        let _ = account_id;
        Ok(())
    }

    pub async fn account_list(
        &self,
        _ctx: &ProviderContext,
        cursor: Option<String>,
        limit: u32,
    ) -> Result<ListResponse<Account>, ProviderError> {
        // TODO: GET /api/accounts?tenant_id=:id&cursor=:c&limit=:n
        // Maps to: STALWART_ACCOUNT_LIST
        let _ = (cursor, limit);
        Ok(ListResponse { items: vec![] })
    }

    // --- Mailbox (folder) operations ---

    pub async fn mailbox_create(
        &self,
        _ctx: &ProviderContext,
        account_id: Uuid,
        name: &str,
        parent_path: Option<&str>,
    ) -> Result<Uuid, ProviderError> {
        // TODO: POST /api/mailboxes with account_id, name, parent
        // Maps to: STALWART_MAILBOX_CREATE
        let _ = (account_id, name, parent_path);
        Ok(Uuid::new_v4())
    }

    pub async fn mailbox_delete(
        &self,
        _ctx: &ProviderContext,
        account_id: Uuid,
        path: &str,
    ) -> Result<(), ProviderError> {
        // TODO: DELETE /api/mailboxes/:account/:path
        // Maps to: STALWART_MAILBOX_DELETE
        let _ = (account_id, path);
        Ok(())
    }

    pub async fn mailbox_list(
        &self,
        _ctx: &ProviderContext,
        account_id: Uuid,
    ) -> Result<Vec<Mailbox>, ProviderError> {
        // TODO: GET /api/mailboxes?account_id=:id
        // Maps to: STALWART_MAILBOX_LIST
        let _ = account_id;
        Ok(vec![])
    }

    // --- Message operations ---

    pub async fn message_store(
        &self,
        _ctx: &ProviderContext,
        raw: RawMessage,
    ) -> Result<Uuid, ProviderError> {
        // TODO: POST /api/messages with raw message bytes
        // Maps to: STALWART_MESSAGE_STORE
        let _ = raw;
        Ok(Uuid::new_v4())
    }

    pub async fn message_get(
        &self,
        _ctx: &ProviderContext,
        account_id: Uuid,
        message_id: Uuid,
    ) -> Result<Message, ProviderError> {
        // TODO: GET /api/messages/:account/:id
        // Maps to: STALWART_MESSAGE_GET
        let _ = (account_id, message_id);
        Err(ProviderError::Unavailable(
            "not implemented: real client lands with MVP slice 1 (docs/13)".to_string(),
        ))
    }

    pub async fn message_update_flags(
        &self,
        _ctx: &ProviderContext,
        account_id: Uuid,
        message_id: Uuid,
        flags: MessageFlags,
    ) -> Result<(), ProviderError> {
        // TODO: PATCH /api/messages/:account/:id/flags
        // Maps to: STALWART_MESSAGE_UPDATE_FLAGS
        let _ = (account_id, message_id, flags);
        Ok(())
    }

    pub async fn message_delete(
        &self,
        _ctx: &ProviderContext,
        account_id: Uuid,
        message_id: Uuid,
    ) -> Result<(), ProviderError> {
        // TODO: DELETE /api/messages/:account/:id
        // Maps to: STALWART_MESSAGE_DELETE
        let _ = (account_id, message_id);
        Ok(())
    }

    pub async fn message_search(
        &self,
        _ctx: &ProviderContext,
        query: &str,
        from: Option<&str>,
        to: Option<&str>,
        subject: Option<&str>,
        date_from: Option<chrono::DateTime<Utc>>,
        date_to: Option<chrono::DateTime<Utc>>,
        limit: u32,
    ) -> Result<Page<SearchResult>, ProviderError> {
        // TODO: GET /api/messages/search with query params
        // Maps to: STALWART_MESSAGE_SEARCH
        let _ = (query, from, to, subject, date_from, date_to, limit);
        Ok(Page {
            items: vec![],
            next_cursor: None,
            has_more: false,
        })
    }

    // --- Attachment operations ---

    pub async fn attachment_list(
        &self,
        _ctx: &ProviderContext,
        message_id: Uuid,
    ) -> Result<Vec<types::Attachment>, ProviderError> {
        // TODO: GET /api/messages/:id/attachments
        // Maps to: STALWART_ATTACHMENT_LIST
        let _ = message_id;
        Ok(vec![])
    }

    pub async fn attachment_get(
        &self,
        _ctx: &ProviderContext,
        blob_key: &str,
    ) -> Result<Vec<u8>, ProviderError> {
        // TODO: GET /api/blobs/:key
        // Maps to: STALWART_ATTACHMENT_GET
        let _ = blob_key;
        Err(ProviderError::Unavailable(
            "not implemented: real client lands with MVP slice 1 (docs/13)".to_string(),
        ))
    }

    // --- Calendar operations ---

    pub async fn event_create(
        &self,
        _ctx: &ProviderContext,
        event: CalendarEvent,
    ) -> Result<Uuid, ProviderError> {
        // TODO: POST /api/events with calendar data
        // Maps to: STALWART_EVENT_CREATE
        let _ = event;
        Ok(Uuid::new_v4())
    }

    pub async fn event_get(
        &self,
        _ctx: &ProviderContext,
        event_id: Uuid,
    ) -> Result<CalendarEvent, ProviderError> {
        // TODO: GET /api/events/:id
        // Maps to: STALWART_EVENT_GET
        let _ = event_id;
        Err(ProviderError::Unavailable(
            "not implemented: real client lands with MVP slice 1 (docs/13)".to_string(),
        ))
    }

    pub async fn event_list(
        &self,
        _ctx: &ProviderContext,
        account_id: Uuid,
        start: chrono::DateTime<Utc>,
        end: chrono::DateTime<Utc>,
    ) -> Result<Vec<CalendarEvent>, ProviderError> {
        // TODO: GET /api/events?account=:id&start=:s&end=:e
        // Maps to: STALWART_EVENT_LIST
        let _ = (account_id, start, end);
        Ok(vec![])
    }

    // --- Contact operations ---

    pub async fn contact_create(
        &self,
        _ctx: &ProviderContext,
        contact: Contact,
    ) -> Result<Uuid, ProviderError> {
        // TODO: POST /api/contacts with contact data
        // Maps to: STALWART_CONTACT_CREATE
        let _ = contact;
        Ok(Uuid::new_v4())
    }

    pub async fn contact_list(
        &self,
        _ctx: &ProviderContext,
        account_id: Uuid,
    ) -> Result<Vec<Contact>, ProviderError> {
        // TODO: GET /api/contacts?account=:id
        // Maps to: STALWART_CONTACT_LIST
        let _ = account_id;
        Ok(vec![])
    }

    // --- Resource booking operations ---

    pub async fn resource_create(
        &self,
        _ctx: &ProviderContext,
        resource: Resource,
    ) -> Result<Uuid, ProviderError> {
        // TODO: POST /api/resources with resource data
        // Maps to: STALWART_RESOURCE_CREATE
        let _ = resource;
        Ok(Uuid::new_v4())
    }

    pub async fn resource_book(
        &self,
        _ctx: &ProviderContext,
        resource_id: Uuid,
        event_id: Uuid,
        start: chrono::DateTime<Utc>,
        end: chrono::DateTime<Utc>,
    ) -> Result<Uuid, ProviderError> {
        // TODO: POST /api/bookings with resource_id, event_id, start, end
        // Maps to: STALWART_RESOURCE_BOOK
        let _ = (resource_id, event_id, start, end);
        Ok(Uuid::new_v4())
    }

    pub async fn booking_cancel(
        &self,
        _ctx: &ProviderContext,
        booking_id: Uuid,
    ) -> Result<(), ProviderError> {
        // TODO: DELETE /api/bookings/:id
        // Maps to: STALWART_BOOKING_CANCEL
        let _ = booking_id;
        Ok(())
    }

    // --- Sieve operations ---

    pub async fn sieve_install(
        &self,
        _ctx: &ProviderContext,
        script: types::SieveScript,
    ) -> Result<(), ProviderError> {
        // TODO: POST /api/sieve with script data
        // Maps to: STALWART_SIEVE_INSTALL
        let _ = script;
        Ok(())
    }

    pub async fn sieve_list(
        &self,
        _ctx: &ProviderContext,
        mailbox_id: Uuid,
    ) -> Result<Vec<types::SieveScript>, ProviderError> {
        // TODO: GET /api/sieve?mailbox=:id
        // Maps to: STALWART_SIEVE_LIST
        let _ = mailbox_id;
        Ok(vec![])
    }
}
