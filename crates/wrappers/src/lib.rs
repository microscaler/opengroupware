// Concrete provider implementations.
//
// Each submodule implements one or more trait impls from types::providers.
// The stalwart module provides the shared HTTP client for Stalwart Mail Server.

pub mod minio_storage;
pub mod rspamd_abuse;
pub mod stalwart;
pub mod stalwart_backup;
pub mod stalwart_identity;
pub mod stalwart_mail;
pub mod tantivy_search;
