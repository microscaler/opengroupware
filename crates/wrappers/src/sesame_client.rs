//! sesame-idam client (ADR-0006 v2) — the ONLY identity integration path.
//!
//! Authenticates machine-to-machine with the `client_credentials` grant
//! (sesame login-service `/auth/token`, PRD-OPENGROUPWARE F1) and provisions
//! users via user-mgmt `/admin/users` (idempotent by email).
//!
//! Provisioned users carry no password: mail-protocol credentials arrive via
//! sesame app passwords (PRD F2), which Stalwart verifies through the
//! rp_directory bridge (F3). Web sign-in uses sesame OIDC.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use types::ProviderError;
use uuid::Uuid;

/// Environment-driven configuration.
///
/// - `SESAME_LOGIN_URL` — login-service base (e.g. `http://sesame-idam-login:8080`)
/// - `SESAME_USER_MGMT_URL` — user-mgmt base
/// - `SESAME_TENANT` — sesame tenant slug this deployment provisions into
/// - `SESAME_CLIENT_ID` / `SESAME_CLIENT_SECRET` — api-key name + `sk_` secret
#[derive(Clone, Debug)]
pub struct SesameConfig {
    pub login_url: String,
    pub user_mgmt_url: String,
    pub tenant: String,
    pub client_id: String,
    pub client_secret: String,
}

impl SesameConfig {
    /// Returns `None` when the integration is not configured (dev without
    /// a sesame stack) — callers degrade to `pending_provisioning`.
    pub fn from_env() -> Option<Self> {
        let get = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        Some(Self {
            login_url: get("SESAME_LOGIN_URL")?,
            user_mgmt_url: get("SESAME_USER_MGMT_URL")?,
            tenant: get("SESAME_TENANT")?,
            client_id: get("SESAME_CLIENT_ID")?,
            client_secret: get("SESAME_CLIENT_SECRET")?,
        })
    }
}

struct CachedToken {
    access_token: String,
    valid_until: Instant,
}

pub struct SesameClient {
    cfg: SesameConfig,
    http: reqwest::Client,
    token: Mutex<Option<CachedToken>>,
}

#[derive(serde::Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i32,
}

#[derive(serde::Deserialize)]
struct CreatedUser {
    user_id: Uuid,
}

impl SesameClient {
    #[must_use]
    pub fn new(cfg: SesameConfig) -> Self {
        Self {
            cfg,
            http: reqwest::Client::new(),
            token: Mutex::new(None),
        }
    }

    fn cached_token(&self) -> Option<String> {
        let guard = self.token.lock().ok()?;
        guard
            .as_ref()
            .filter(|t| t.valid_until > Instant::now())
            .map(|t| t.access_token.clone())
    }

    fn store_token(&self, access_token: String, expires_in: i32) {
        if let Ok(mut guard) = self.token.lock() {
            // Refresh 60s before expiry.
            let ttl = Duration::from_secs(u64::try_from(expires_in.max(61) - 60).unwrap_or(60));
            *guard = Some(CachedToken {
                access_token,
                valid_until: Instant::now() + ttl,
            });
        }
    }

    /// M2M access token via client_credentials, cached until near expiry.
    async fn token(&self) -> Result<String, ProviderError> {
        if let Some(tok) = self.cached_token() {
            return Ok(tok);
        }
        let url = format!("{}/auth/token", self.cfg.login_url.trim_end_matches('/'));
        let resp = self
            .http
            .post(&url)
            .header("X-Tenant-ID", &self.cfg.tenant)
            .json(&serde_json::json!({
                "grant_type": "client_credentials",
                "client_id": self.cfg.client_id,
                "client_secret": self.cfg.client_secret,
            }))
            .send()
            .await
            .map_err(|e| ProviderError::Unavailable(format!("sesame token request: {e}")))?;

        if !resp.status().is_success() {
            return Err(ProviderError::Unavailable(format!(
                "sesame token endpoint returned {}",
                resp.status()
            )));
        }
        let body: TokenResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::Internal(format!("sesame token decode: {e}")))?;
        if body.access_token.is_empty() {
            return Err(ProviderError::Unavailable(
                "sesame issued an empty token (invalid client credentials?)".to_string(),
            ));
        }
        self.store_token(body.access_token.clone(), body.expires_in);
        Ok(body.access_token)
    }

    /// Provision (or fetch, idempotently) a sesame user for a mail account.
    /// Returns the sesame user id.
    pub async fn provision_user(
        &self,
        email: &str,
        display_name: &str,
    ) -> Result<Uuid, ProviderError> {
        let token = self.token().await?;
        let url = format!(
            "{}/admin/users",
            self.cfg.user_mgmt_url.trim_end_matches('/')
        );
        let mut body = serde_json::json!({
            "email": email,
            "email_confirmed": true,
        });
        if !display_name.trim().is_empty() {
            body["first_name"] = serde_json::json!(display_name);
        }
        let resp = self
            .http
            .post(&url)
            .bearer_auth(token)
            .header("X-Tenant-ID", &self.cfg.tenant)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Unavailable(format!("sesame create_user: {e}")))?;

        let status = resp.status();
        if !(status.is_success()) {
            return Err(ProviderError::Unavailable(format!(
                "sesame create_user returned {status}"
            )));
        }
        let user: CreatedUser = resp
            .json()
            .await
            .map_err(|e| ProviderError::Internal(format!("sesame create_user decode: {e}")))?;
        Ok(user.user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_absent_when_env_unset() {
        // Not set in the test environment.
        std::env::remove_var("SESAME_LOGIN_URL");
        assert!(SesameConfig::from_env().is_none());
    }
}
