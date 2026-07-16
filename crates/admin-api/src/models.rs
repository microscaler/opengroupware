//! Request/response types + input validation for the provisioning API.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;

// ---------------------------------------------------------------------------
// Rows
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Tenant {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub plan: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Domain {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub fqdn: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Account {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub domain_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub quota_mb: i32,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ---------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateTenant {
    pub slug: String,
    pub name: String,
    #[serde(default = "default_plan")]
    pub plan: String,
}

fn default_plan() -> String {
    "standard".to_string()
}

#[derive(Debug, Deserialize)]
pub struct CreateDomain {
    pub fqdn: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateAccount {
    pub domain_id: Uuid,
    pub local_part: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default = "default_quota")]
    pub quota_mb: i32,
}

fn default_quota() -> i32 {
    1024
}

// ---------------------------------------------------------------------------
// Validation (DB CHECKs are the backstop; these produce friendly 422s)
// ---------------------------------------------------------------------------

pub fn validate_slug(slug: &str) -> Result<(), ApiError> {
    let ok = (2..=63).contains(&slug.len())
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !slug.starts_with('-')
        && !slug.ends_with('-');
    if ok {
        Ok(())
    } else {
        Err(ApiError::Validation(
            "slug must be 2-63 chars of [a-z0-9-], not starting/ending with '-'".to_string(),
        ))
    }
}

pub fn validate_fqdn(fqdn: &str) -> Result<(), ApiError> {
    let ok = fqdn.len() <= 253
        && fqdn.contains('.')
        && fqdn.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                && !label.starts_with('-')
                && !label.ends_with('-')
        });
    if ok {
        Ok(())
    } else {
        Err(ApiError::Validation("invalid fqdn".to_string()))
    }
}

pub fn validate_local_part(local: &str) -> Result<(), ApiError> {
    let ok = (1..=64).contains(&local.len())
        && local.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_' | '+')
        })
        && !local.starts_with('.')
        && !local.ends_with('.');
    if ok {
        Ok(())
    } else {
        Err(ApiError::Validation(
            "local part must be 1-64 chars of [a-z0-9.+_-], not starting/ending with '.'"
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_rules() {
        assert!(validate_slug("acme").is_ok());
        assert!(validate_slug("acme-corp-2").is_ok());
        assert!(validate_slug("a").is_err());
        assert!(validate_slug("-acme").is_err());
        assert!(validate_slug("acme-").is_err());
        assert!(validate_slug("Acme").is_err());
        assert!(validate_slug("acme corp").is_err());
    }

    #[test]
    fn fqdn_rules() {
        assert!(validate_fqdn("example.com").is_ok());
        assert!(validate_fqdn("mail.acme-corp.co.uk").is_ok());
        assert!(validate_fqdn("localhost").is_err());
        assert!(validate_fqdn("-bad.com").is_err());
        assert!(validate_fqdn("bad-.com").is_err());
        assert!(validate_fqdn("").is_err());
    }

    #[test]
    fn local_part_rules() {
        assert!(validate_local_part("charles").is_ok());
        assert!(validate_local_part("charles.sibbald+test").is_ok());
        assert!(validate_local_part("").is_err());
        assert!(validate_local_part(".charles").is_err());
        assert!(validate_local_part("charles.").is_err());
        assert!(validate_local_part("Charles").is_err());
    }
}
