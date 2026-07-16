// Identity provider interface — OIDC/LDAP user authentication and directory.
//
// Every method takes a `ProviderContext` for multi-tenant scoping.

use crate::{DateTime, ProviderContext, ProviderError, Uuid};
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// OIDC access token info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub tenant_id: Uuid,
    pub account_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub is_admin: bool,
    pub mfa_enabled: bool,
}

/// App password (hashed token for legacy IMAP auth).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPassword {
    pub id: Uuid,
    pub account_id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
}

/// Active session for a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: Uuid,
    pub account_id: Uuid,
    pub tenant_id: Uuid,
    pub device: Option<String>,
    pub ip: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub is_active: bool,
}

#[async_trait::async_trait]
pub trait IdentityProvider: Send + Sync {
    /// Authenticate with OIDC token. Returns user info + tenant context.
    async fn authenticate_oidc(&self, access_token: &str) -> Result<TokenInfo, ProviderError>;

    /// Authenticate with email + password (or app password).
    /// Returns session token + user info.
    async fn authenticate_password(
        &self,
        email: &str,
        password: &str,
    ) -> Result<(Uuid, TokenInfo), ProviderError>;

    /// Verify MFA token for an account.
    async fn verify_mfa(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        token: &str,
    ) -> Result<bool, ProviderError>;

    /// Create an app password for an account (for legacy IMAP/DAV clients).
    async fn create_app_password(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        name: &str,
    ) -> Result<AppPassword, ProviderError>;

    /// Revoke an app password.
    async fn revoke_app_password(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        password_id: Uuid,
    ) -> Result<(), ProviderError>;

    /// List app passwords for an account.
    async fn list_app_passwords(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
    ) -> Result<Vec<AppPassword>, ProviderError>;

    /// Start a session (login). Returns session ID.
    async fn create_session(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        device: Option<String>,
        ip: Option<String>,
    ) -> Result<Uuid, ProviderError>;

    /// Validate a session token.
    async fn validate_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<SessionInfo>, ProviderError>;

    /// Revoke a session (logout or device removal).
    async fn revoke_session(
        &self,
        ctx: &ProviderContext,
        session_id: Uuid,
    ) -> Result<(), ProviderError>;

    /// List active sessions for an account.
    async fn list_sessions(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
    ) -> Result<Vec<SessionInfo>, ProviderError>;
}
