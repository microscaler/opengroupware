// Stalwart identity provider — concrete implementation of IdentityProvider.

use types::providers::IdentityProvider;
use types::providers::{AppPassword, SessionInfo, TokenInfo};
use types::{ProviderContext, ProviderError, Uuid};

/// Concrete implementation of IdentityProvider backed by Stalwart Mail Server.
pub struct StalwartIdentityProvider {
    client_url: String,
    admin_api_key: String,
}

impl StalwartIdentityProvider {
    #[must_use]
    pub const fn new(client_url: String, admin_api_key: String) -> Self {
        Self {
            client_url,
            admin_api_key,
        }
    }
}

#[async_trait::async_trait]
impl IdentityProvider for StalwartIdentityProvider {
    async fn authenticate_oidc(&self, access_token: &str) -> Result<TokenInfo, ProviderError> {
        // TODO: Call Stalwart JMAP /auth/oidc endpoint
        // Maps to: STALWART_AUTHENTICATE_OIDC
        let _ = access_token;
        Err(ProviderError::Unavailable(
            "not implemented: real client lands with MVP slice 1 (docs/13)".to_string(),
        ))
    }

    async fn authenticate_password(
        &self,
        email: &str,
        password: &str,
    ) -> Result<(Uuid, TokenInfo), ProviderError> {
        // TODO: Call Stalwart /auth/login endpoint
        // Maps to: STALWART_AUTHENTICATE_PASSWORD
        let _ = (email, password);
        Err(ProviderError::Unavailable(
            "not implemented: real client lands with MVP slice 1 (docs/13)".to_string(),
        ))
    }

    async fn verify_mfa(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        token: &str,
    ) -> Result<bool, ProviderError> {
        // TODO: Verify TOTP code via Stalwart API
        // Maps to: STALWART_VERIFY_MFA
        let _ = (ctx, account_id, token);
        Ok(true)
    }

    async fn create_app_password(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        name: &str,
    ) -> Result<AppPassword, ProviderError> {
        // TODO: Create app-specific password via Stalwart API
        // Maps to: STALWART_CREATE_APP_PASSWORD
        let _ = (ctx, account_id, name);
        Err(ProviderError::Unavailable(
            "not implemented: real client lands with MVP slice 1 (docs/13)".to_string(),
        ))
    }

    async fn revoke_app_password(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        password_id: Uuid,
    ) -> Result<(), ProviderError> {
        // TODO: Revoke app password via Stalwart API
        // Maps to: STALWART_REVOKE_APP_PASSWORD
        let _ = (ctx, account_id, password_id);
        Ok(())
    }

    async fn list_app_passwords(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
    ) -> Result<Vec<AppPassword>, ProviderError> {
        // TODO: List app passwords via Stalwart API
        // Maps to: STALWART_LIST_APP_PASSWORDS
        let _ = (ctx, account_id);
        Ok(vec![])
    }

    async fn create_session(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        device: Option<String>,
        ip: Option<String>,
    ) -> Result<Uuid, ProviderError> {
        // TODO: Create session via Stalwart API
        // Maps to: STALWART_CREATE_SESSION
        let _ = (ctx, account_id, device, ip);
        Ok(Uuid::new_v4())
    }

    async fn validate_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<SessionInfo>, ProviderError> {
        // TODO: Validate session via Stalwart API
        // Maps to: STALWART_VALIDATE_SESSION
        let _ = session_id;
        Err(ProviderError::Unavailable(
            "not implemented: real client lands with MVP slice 1 (docs/13)".to_string(),
        ))
    }

    async fn revoke_session(
        &self,
        ctx: &ProviderContext,
        session_id: Uuid,
    ) -> Result<(), ProviderError> {
        // TODO: Revoke session via Stalwart API
        // Maps to: STALWART_REVOKE_SESSION
        let _ = (ctx, session_id);
        Ok(())
    }

    async fn list_sessions(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
    ) -> Result<Vec<SessionInfo>, ProviderError> {
        // TODO: List active sessions via Stalwart API
        // Maps to: STALWART_LIST_SESSIONS
        let _ = (ctx, account_id);
        Ok(vec![])
    }
}
