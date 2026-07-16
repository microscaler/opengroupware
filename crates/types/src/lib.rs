// Domain types shared across all crates.

pub use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
pub use uuid::Uuid;

// ---------------------------------------------------------------------------
// Provider traits (re-exported for downstream crates)
// ---------------------------------------------------------------------------

pub mod providers;

// Re-export provider-side domain types at crate root for convenience.
pub use providers::{
    AppPassword, Attachment, BackupJob, BookingStatus, CalendarEvent, Contact, DmarcResult,
    ExportFormat, ScoringAction, ScoringResult, SearchProvider, SessionInfo, SieveScript,
    ThreatIndicatorType, ThreatIntelObservation, ThreatSeverity, TokenInfo,
};
// Mail-specific types
pub use providers::mail::{RawMessage, Resource, ResourceBooking};

// ---------------------------------------------------------------------------
// Tenant model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tenant {
    pub id: Uuid,
    pub domain: String,
    pub status: TenantStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TenantStatus {
    Active,
    Suspended,
    Terminated,
}

// ---------------------------------------------------------------------------
// Account / User model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub status: AccountStatus,
    pub mfa_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccountStatus {
    Active,
    Disabled,
    Locked,
}

// ---------------------------------------------------------------------------
// Mailbox model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mailbox {
    pub id: Uuid,
    pub account_id: Uuid,
    pub tenant_id: Uuid,
    pub name: String, // folder name, e.g. "Inbox", "Sent"
    pub path: String, // full IMAP path, e.g. "Inbox", "Archive/2024"
    pub flags: MailboxFlags,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MailboxFlags {
    pub has_children: bool,
    pub no_inferiors: bool,
    pub no_select: bool,
    pub marked: bool,
    pub unanswered: bool,
    pub deleted: bool,
}

// ---------------------------------------------------------------------------
// Message model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub tenant_id: Uuid,
    pub uid: u32, // IMAP UID within mailbox
    pub from: Vec<String>,
    pub to: Vec<String>,
    pub subject: String,
    pub received_at: DateTime<Utc>,
    pub size_bytes: u64,
    pub has_attachments: bool,
    pub flags: MessageFlags,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageFlags {
    pub seen: bool,
    pub answered: bool,
    pub flagged: bool,
    pub deleted: bool,
    pub draft: bool,
}

// ---------------------------------------------------------------------------
// Abuse / Quarantine model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineItem {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub mailbox_id: Uuid,
    pub message_id: Uuid,
    pub spam_score: f64,
    pub virus: bool,
    pub status: QuarantineStatus,
    pub created_at: DateTime<Utc>,
    pub decision: Option<QuarantineDecision>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuarantineStatus {
    Pending,
    Released,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuarantineDecision {
    AdminRelease,
    AdminDelete,
    UserRelease,
    UserDelete,
}

// ---------------------------------------------------------------------------
// Search model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub message_id: Uuid,
    pub mailbox_id: Uuid,
    pub tenant_id: Uuid,
    pub subject: String,
    pub from: String,
    pub received_at: DateTime<Utc>,
    pub snippet: String,
}

// ---------------------------------------------------------------------------
// Blob model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobRef {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub content_type: String,
    pub size_bytes: u64,
    pub etag: String,
}

// ---------------------------------------------------------------------------
// Provider context — the multi-tenant scope for every provider call
// ---------------------------------------------------------------------------

/// Carries the tenant context that must accompany every provider method call.
/// This is how the product plane enforces multi-tenant isolation at the
/// provider boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderContext {
    pub tenant_id: Uuid,
    /// Optional account_id for mailbox-scoped operations.
    pub account_id: Option<Uuid>,
}

impl ProviderContext {
    #[must_use]
    pub const fn new(tenant_id: Uuid) -> Self {
        Self {
            tenant_id,
            account_id: None,
        }
    }

    #[must_use]
    pub const fn for_account(tenant_id: Uuid, account_id: Uuid) -> Self {
        Self {
            tenant_id,
            account_id: Some(account_id),
        }
    }
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("tenant mismatch: expected {expected}, got {actual}")]
    TenantMismatch { expected: Uuid, actual: Uuid },
    #[error("tenant suspended: {0}")]
    TenantSuspended(String),
    #[error("quota exceeded")]
    QuotaExceeded,
    #[error("internal error: {0}")]
    Internal(String),
    #[error("backend unavailable: {0}")]
    Unavailable(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

// ---------------------------------------------------------------------------
// Common request/response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
}

// ---------------------------------------------------------------------------
// Config model — what the config compiler reads from the product DB
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantConfig {
    pub tenant_id: Uuid,
    pub domain: String,
    pub policy_profile: String,
    pub smtp_config: SmtpConfig,
    pub mailbox_config: MailboxConfig,
    pub rspamd_config: RspamdConfig,
    pub tenant_resource_quota: TenantResourceQuota,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    pub max_message_size: u64,
    pub rate_limit_per_minute: u32,
    pub require_auth_for_outbound: bool,
    pub enable_dmarc_reject: bool,
    pub shadow_copy_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailboxConfig {
    pub max_storage_bytes: u64,
    pub enable_imap: bool,
    pub enable_jmap: bool,
    pub enable_caldav: bool,
    pub enable_carddav: bool,
    pub enable_sieve: bool,
    pub default_quota_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RspamdConfig {
    pub bayes_enabled: bool,
    pub neutral_on_spam: bool,
    pub quarantine_on_virus: bool,
    pub threat_intel_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantResourceQuota {
    pub max_storage_bytes: u64,
    pub max_accounts: u32,
    pub max_domains: u32,
    pub max_api_requests_per_minute: u32,
    pub max_concurrent_connections: u32,
}
