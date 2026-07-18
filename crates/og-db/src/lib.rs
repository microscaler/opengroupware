//! Shared Postgres access for opengroupware API services.
//!
//! Consolidates the RLS transaction wrapper (D4, docs/13), the
//! privileged-role startup guard, the API error→HTTP mapping, and the
//! append-only audit helper so admin-api, abuse-api, and future services
//! share one correct implementation.

pub mod audit;
pub mod db;
pub mod error;

pub use audit::{record, AuditEntry};
pub use db::Db;
pub use error::ApiError;
