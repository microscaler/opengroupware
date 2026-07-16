// Provider trait definitions and provider-side domain types.
//
// The traits defined here are the API boundary between the product plane
// (admin-api, abuse-api, job-runner) and the concrete infrastructure
// implementations in the `wrappers` crate.
//
// Every trait method takes a `ProviderContext` to enforce multi-tenant
// isolation at the call site.

pub mod abuse;
pub mod backup;
pub mod identity;
pub mod mail;
pub mod search;
pub mod storage;

// Re-export traits and types for convenient access from other crates.
pub use abuse::{
    AbuseProvider, DmarcResult, ScoringAction, ScoringResult, ThreatIndicatorType,
    ThreatIntelObservation, ThreatSeverity,
};
pub use backup::{BackupJob, BackupProvider, BackupStatus, ExportFormat};
pub use identity::{AppPassword, IdentityProvider, SessionInfo, TokenInfo};
pub use mail::{
    Attachment, BookingStatus, CalendarEvent, Contact, MailProvider, RawMessage, Resource,
    ResourceBooking, SieveScript,
};
pub use search::SearchProvider;
pub use storage::StorageProvider;
