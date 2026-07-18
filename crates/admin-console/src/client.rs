//! Read-only HTTP client for admin-api.
//!
//! The console renders desired state; it never mutates. In dev (admin-api with
//! no `OG_AUTH_JWKS_URL`) auth is the `x-actor` header. In production admin-api
//! verifies sesame tokens — the console will need a service token then; that is
//! out of scope for this read-only slice (see the note in `from_env`).

use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct TenantDto {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub plan: String,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DomainDto {
    pub fqdn: String,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountDto {
    pub email: String,
    pub display_name: String,
    pub quota_mb: i32,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PolicyDto {
    pub reject: f64,
    pub add_header: f64,
    pub greylist: f64,
}

#[derive(Clone)]
pub struct AdminApi {
    base: String,
    actor: String,
    http: reqwest::Client,
}

impl AdminApi {
    pub fn from_env() -> Self {
        // ADMIN_API_URL: in-cluster `http://admin-api:8080`; dev `127.0.0.1:8080`.
        let base =
            std::env::var("ADMIN_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
        // Dev auth only. Under JWKS enforcement admin-api ignores x-actor and
        // requires a bearer token — a service-token flow is a later slice.
        let actor =
            std::env::var("ADMIN_CONSOLE_ACTOR").unwrap_or_else(|_| "admin-console".to_string());
        Self {
            base: base.trim_end_matches('/').to_string(),
            actor,
            http: reqwest::Client::new(),
        }
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}{path}", self.base);
        let resp = self
            .http
            .get(&url)
            .header("x-actor", &self.actor)
            .send()
            .await
            .map_err(|e| format!("request to {path} failed: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!("{path} returned {status}"));
        }
        resp.json::<T>()
            .await
            .map_err(|e| format!("decoding {path}: {e}"))
    }

    pub async fn tenants(&self) -> Result<Vec<TenantDto>, String> {
        self.get_json("/api/v1/tenants").await
    }

    pub async fn domains(&self, tenant: Uuid) -> Result<Vec<DomainDto>, String> {
        self.get_json(&format!("/api/v1/tenants/{tenant}/domains"))
            .await
    }

    pub async fn accounts(&self, tenant: Uuid) -> Result<Vec<AccountDto>, String> {
        self.get_json(&format!("/api/v1/tenants/{tenant}/accounts"))
            .await
    }

    pub async fn policy(&self, tenant: Uuid) -> Result<PolicyDto, String> {
        self.get_json(&format!("/api/v1/tenants/{tenant}/policy"))
            .await
    }
}
