// Mailbox provider interface — operations on the mail store.
//
// Every method takes a `ProviderContext` for multi-tenant scoping.
// The implementation (Stalwart, Dovecot, etc.) is responsible for ensuring
// tenant_id isolation. The product plane must never construct a call without
// a valid ProviderContext.

use crate::{
    Account, DateTime, ListResponse, Mailbox, Message, MessageFlags, Page, ProviderContext,
    ProviderError, SearchResult, Uuid,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Message data as stored in the mailbox backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawMessage {
    pub id: Uuid,
    pub from: Vec<String>,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_html: Option<String>,
    pub body_text: Option<String>,
    pub headers: HashMap<String, String>,
    pub received_at: DateTime<Utc>,
}

/// Attachment stored in the mailbox backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: Uuid,
    pub message_id: Uuid,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: u64,
    pub blob_key: String, // S3/blob storage key
}

/// Calendar event stored in the mailbox backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub organizer: String,
    pub attendees: Vec<String>,
}

/// Contact stored in the mailbox backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: Option<String>,
}

/// Resource (conference room, equipment) stored in the mailbox backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub capacity: u32,
    pub location: Option<String>,
}

/// Calendar booking (reservation of a resource).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceBooking {
    pub id: Uuid,
    pub resource_id: Uuid,
    pub event_id: Uuid,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub status: BookingStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BookingStatus {
    Confirmed,
    Pending,
    Cancelled,
}

/// Sieve script stored in the mailbox backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SieveScript {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub name: String,
    pub script: String, // Sieve DSL text
    pub active: bool,
}

#[async_trait::async_trait]
pub trait MailProvider: Send + Sync {
    // --- Tenant operations ---

    /// Create a tenant in the mailbox backend. This allocates storage prefixes,
    /// creates tenant-scoped namespaces, and sets up initial configuration.
    async fn create_tenant(&self, ctx: &ProviderContext) -> Result<(), ProviderError>;

    /// Suspend a tenant: block new mail, freeze updates, keep existing data.
    async fn suspend_tenant(&self, ctx: &ProviderContext) -> Result<(), ProviderError>;

    /// Terminate a tenant: delete all data for a tenant (called after export).
    async fn terminate_tenant(&self, ctx: &ProviderContext) -> Result<(), ProviderError>;

    // --- Account operations ---

    /// Create a user account in the mailbox backend.
    async fn create_account(
        &self,
        ctx: &ProviderContext,
        email: &str,
        display_name: &str,
    ) -> Result<Uuid, ProviderError>;

    /// Delete a user account and all its mailboxes.
    async fn delete_account(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
    ) -> Result<(), ProviderError>;

    /// List accounts for a tenant (with optional pagination).
    async fn list_accounts(
        &self,
        ctx: &ProviderContext,
        cursor: Option<String>,
        limit: u32,
    ) -> Result<ListResponse<Account>, ProviderError>;

    // --- Mailbox (folder) operations ---

    /// Create a mailbox (folder) for an account.
    async fn create_mailbox(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        name: &str,
        parent_path: Option<&str>,
    ) -> Result<Uuid, ProviderError>;

    /// Delete a mailbox (folder).
    async fn delete_mailbox(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        path: &str,
    ) -> Result<(), ProviderError>;

    /// List mailboxes (folders) for an account.
    async fn list_mailboxes(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
    ) -> Result<Vec<Mailbox>, ProviderError>;

    // --- Message operations ---

    /// Store a raw message in the mailbox backend. Returns the message ID.
    async fn store_message(
        &self,
        ctx: &ProviderContext,
        raw: RawMessage,
    ) -> Result<Uuid, ProviderError>;

    /// Retrieve a message by ID.
    async fn get_message(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        message_id: Uuid,
    ) -> Result<Message, ProviderError>;

    /// Update message flags (seen, answered, flagged, etc.).
    async fn update_message_flags(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        message_id: Uuid,
        flags: MessageFlags,
    ) -> Result<(), ProviderError>;

    /// Delete a message.
    async fn delete_message(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        message_id: Uuid,
    ) -> Result<(), ProviderError>;

    /// Search messages in a tenant's mailboxes.
    async fn search_messages(
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

    // --- Attachment operations ---

    /// List attachments for a message.
    async fn list_attachments(
        &self,
        ctx: &ProviderContext,
        message_id: Uuid,
    ) -> Result<Vec<Attachment>, ProviderError>;

    /// Get attachment data by blob key.
    async fn get_attachment(
        &self,
        ctx: &ProviderContext,
        blob_key: &str,
    ) -> Result<Vec<u8>, ProviderError>;

    // --- Calendar operations ---

    /// Create a calendar event.
    async fn create_event(
        &self,
        ctx: &ProviderContext,
        event: CalendarEvent,
    ) -> Result<Uuid, ProviderError>;

    /// Get a calendar event by ID.
    async fn get_event(
        &self,
        ctx: &ProviderContext,
        event_id: Uuid,
    ) -> Result<CalendarEvent, ProviderError>;

    /// List events for an account within a date range.
    async fn list_events(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<CalendarEvent>, ProviderError>;

    // --- Contact operations ---

    /// Create a contact.
    async fn create_contact(
        &self,
        ctx: &ProviderContext,
        contact: Contact,
    ) -> Result<Uuid, ProviderError>;

    /// List contacts for an account.
    async fn list_contacts(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
    ) -> Result<Vec<Contact>, ProviderError>;

    // --- Resource booking operations ---

    /// Create a resource (conference room, etc.).
    async fn create_resource(
        &self,
        ctx: &ProviderContext,
        resource: Resource,
    ) -> Result<Uuid, ProviderError>;

    /// Book a resource for an event.
    async fn book_resource(
        &self,
        ctx: &ProviderContext,
        resource_id: Uuid,
        event_id: Uuid,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Uuid, ProviderError>;

    /// Cancel a resource booking.
    async fn cancel_booking(
        &self,
        ctx: &ProviderContext,
        booking_id: Uuid,
    ) -> Result<(), ProviderError>;

    // --- Sieve operations ---

    /// Install a Sieve script for a mailbox.
    async fn install_sieve_script(
        &self,
        ctx: &ProviderContext,
        script: SieveScript,
    ) -> Result<(), ProviderError>;

    /// List Sieve scripts for a mailbox.
    async fn list_sieve_scripts(
        &self,
        ctx: &ProviderContext,
        mailbox_id: Uuid,
    ) -> Result<Vec<SieveScript>, ProviderError>;
}
