//! sesame-idam token verification (PRD-OPENGROUPWARE F4).
//!
//! The sesame-idam-client SDK does NOT verify JWTs (it trusts the
//! BRRTRouter edge). opengroupware runs on tokio/axum outside that edge, so
//! it must verify tokens itself — this crate does that: fetch the sesame
//! JWKS (`OKP`/`Ed25519` keys), cache it, and validate the EdDSA signature +
//! `exp`/`iss` of Bearer tokens.
//!
//! Dev fallback: when `OG_AUTH_JWKS_URL` is unset the [`Authenticator`]
//! trusts the `x-actor` header instead of a token — so local runs and the
//! existing smoke tests keep working without a live sesame.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ring::signature::{UnparsedPublicKey, ED25519};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("missing bearer token")]
    MissingToken,
    #[error("malformed token")]
    Malformed,
    #[error("unknown signing key")]
    UnknownKey,
    #[error("signature verification failed")]
    BadSignature,
    #[error("token expired")]
    Expired,
    #[error("issuer mismatch")]
    IssuerMismatch,
    #[error("jwks unavailable: {0}")]
    JwksUnavailable(String),
}

/// The verified caller identity extracted from a valid token (or the dev
/// `x-actor` fallback).
#[derive(Debug, Clone)]
pub struct Caller {
    pub subject: String,
    pub tenant_id: Option<Uuid>,
    pub roles: Vec<String>,
}

impl Caller {
    /// Audit-actor string.
    #[must_use]
    pub fn actor(&self) -> &str {
        &self.subject
    }
}

#[derive(Clone)]
pub struct AuthConfig {
    /// sesame JWKS URL, e.g. `http://identity-session-service.sesame-idam
    /// .svc.cluster.local:8080/.well-known/jwks.json`. None = dev mode.
    pub jwks_url: Option<String>,
    /// Required `iss` claim (when set).
    pub issuer: Option<String>,
    pub cache_ttl: Duration,
}

impl AuthConfig {
    #[must_use]
    pub fn from_env() -> Self {
        let get = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        Self {
            jwks_url: get("OG_AUTH_JWKS_URL"),
            issuer: get("OG_AUTH_ISSUER"),
            cache_ttl: Duration::from_secs(600),
        }
    }
}

struct CachedKeys {
    keys: HashMap<String, Vec<u8>>, // kid -> raw Ed25519 public key
    fetched_at: Instant,
}

#[derive(Clone)]
pub struct Authenticator {
    cfg: AuthConfig,
    http: reqwest::Client,
    cache: Arc<RwLock<Option<CachedKeys>>>,
}

#[derive(serde::Deserialize)]
struct Jwk {
    kid: String,
    x: String,
    #[serde(default)]
    kty: String,
    #[serde(default)]
    crv: String,
}

#[derive(serde::Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

#[derive(serde::Deserialize)]
struct Header {
    alg: String,
    kid: String,
}

#[derive(serde::Deserialize)]
struct Claims {
    #[serde(default)]
    sub: String,
    #[serde(default)]
    exp: i64,
    #[serde(default)]
    iss: String,
    #[serde(default)]
    tenant_id: Option<String>,
    #[serde(default)]
    sx: Option<Sx>,
}

#[derive(serde::Deserialize)]
struct Sx {
    #[serde(default)]
    roles: Vec<String>,
}

impl Authenticator {
    #[must_use]
    pub fn new(cfg: AuthConfig) -> Self {
        Self {
            cfg,
            http: reqwest::Client::new(),
            cache: Arc::new(RwLock::new(None)),
        }
    }

    /// True when token verification is active (JWKS URL configured).
    #[must_use]
    pub fn enforcing(&self) -> bool {
        self.cfg.jwks_url.is_some()
    }

    /// Resolve the caller from request headers. In enforcing mode a valid
    /// `Authorization: Bearer <jwt>` is required; otherwise falls back to the
    /// `x-actor` header (dev).
    ///
    /// # Errors
    /// Returns [`AuthError`] when enforcing and the token is missing/invalid.
    pub async fn caller(&self, headers: &axum::http::HeaderMap) -> Result<Caller, AuthError> {
        if !self.enforcing() {
            let subject = headers
                .get("x-actor")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("unknown")
                .to_string();
            return Ok(Caller {
                subject,
                tenant_id: None,
                roles: vec![],
            });
        }
        let token = bearer(headers).ok_or(AuthError::MissingToken)?;
        self.verify(token).await
    }

    async fn verify(&self, token: &str) -> Result<Caller, AuthError> {
        let mut parts = token.split('.');
        let (h, p, s) = match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(h), Some(p), Some(s), None) => (h, p, s),
            _ => return Err(AuthError::Malformed),
        };
        let header: Header = decode_json(h).ok_or(AuthError::Malformed)?;
        if header.alg != "EdDSA" {
            return Err(AuthError::BadSignature);
        }
        let pubkey = self.key_for(&header.kid).await?;
        let sig = URL_SAFE_NO_PAD
            .decode(s)
            .map_err(|_| AuthError::Malformed)?;
        let signing_input = format!("{h}.{p}");
        UnparsedPublicKey::new(&ED25519, &pubkey)
            .verify(signing_input.as_bytes(), &sig)
            .map_err(|_| AuthError::BadSignature)?;

        let claims: Claims = decode_json(p).ok_or(AuthError::Malformed)?;
        let now = now_unix();
        if claims.exp != 0 && claims.exp < now {
            return Err(AuthError::Expired);
        }
        if let Some(expected) = &self.cfg.issuer {
            if &claims.iss != expected {
                return Err(AuthError::IssuerMismatch);
            }
        }
        Ok(Caller {
            subject: claims.sub,
            tenant_id: claims.tenant_id.and_then(|t| Uuid::parse_str(&t).ok()),
            roles: claims.sx.map(|s| s.roles).unwrap_or_default(),
        })
    }

    /// Look up a public key by kid, refreshing the JWKS cache on a miss or
    /// when stale.
    async fn key_for(&self, kid: &str) -> Result<Vec<u8>, AuthError> {
        if let Some(k) = self.cached(kid).await {
            return Ok(k);
        }
        self.refresh().await?;
        self.cached(kid).await.ok_or(AuthError::UnknownKey)
    }

    async fn cached(&self, kid: &str) -> Option<Vec<u8>> {
        let guard = self.cache.read().await;
        let cached = guard.as_ref()?;
        if cached.fetched_at.elapsed() > self.cfg.cache_ttl {
            return None;
        }
        cached.keys.get(kid).cloned()
    }

    async fn refresh(&self) -> Result<(), AuthError> {
        let url = self
            .cfg
            .jwks_url
            .as_ref()
            .ok_or_else(|| AuthError::JwksUnavailable("no url".into()))?;
        let set: JwkSet = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| AuthError::JwksUnavailable(e.to_string()))?
            .json()
            .await
            .map_err(|e| AuthError::JwksUnavailable(e.to_string()))?;
        let keys = parse_jwks(&set);
        *self.cache.write().await = Some(CachedKeys {
            keys,
            fetched_at: Instant::now(),
        });
        Ok(())
    }
}

fn parse_jwks(set: &JwkSet) -> HashMap<String, Vec<u8>> {
    let mut keys = HashMap::new();
    for jwk in &set.keys {
        if jwk.kty != "OKP" || jwk.crv != "Ed25519" {
            continue;
        }
        if let Ok(raw) = URL_SAFE_NO_PAD.decode(&jwk.x) {
            keys.insert(jwk.kid.clone(), raw);
        }
    }
    keys
}

fn bearer(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

fn decode_json<T: serde::de::DeserializeOwned>(seg: &str) -> Option<T> {
    let bytes = URL_SAFE_NO_PAD.decode(seg).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use ring::rand::SystemRandom;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    fn b64(data: &[u8]) -> String {
        URL_SAFE_NO_PAD.encode(data)
    }

    /// Build (keypair, kid, public_x_b64) and a signer closure.
    fn keypair() -> (Ed25519KeyPair, String) {
        let rng = SystemRandom::new();
        let doc = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let kp = Ed25519KeyPair::from_pkcs8(doc.as_ref()).unwrap();
        (kp, "test-kid".to_string())
    }

    fn sign_token(kp: &Ed25519KeyPair, kid: &str, exp: i64, iss: &str) -> String {
        let header = serde_json::json!({"alg":"EdDSA","typ":"at+jwt","kid":kid});
        let payload = serde_json::json!({
            "sub":"client:abc","exp":exp,"iss":iss,
            "tenant_id":"11111111-1111-1111-1111-111111111111",
            "sx":{"roles":["admin"]}
        });
        let h = b64(header.to_string().as_bytes());
        let p = b64(payload.to_string().as_bytes());
        let signing_input = format!("{h}.{p}");
        let sig = kp.sign(signing_input.as_bytes());
        format!("{h}.{p}.{}", b64(sig.as_ref()))
    }

    async fn authenticator_with(
        kp: &Ed25519KeyPair,
        kid: &str,
        issuer: Option<&str>,
    ) -> Authenticator {
        let auth = Authenticator::new(AuthConfig {
            jwks_url: Some("http://unused".into()),
            issuer: issuer.map(str::to_string),
            cache_ttl: Duration::from_secs(600),
        });
        // Pre-seed the cache so no network fetch happens.
        let mut keys = HashMap::new();
        keys.insert(kid.to_string(), kp.public_key().as_ref().to_vec());
        *auth.cache.write().await = Some(CachedKeys {
            keys,
            fetched_at: Instant::now(),
        });
        auth
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn valid_token_verifies() {
        let (kp, kid) = keypair();
        let auth = authenticator_with(&kp, &kid, Some("https://idam.example.com")).await;
        let tok = sign_token(&kp, &kid, now_unix() + 300, "https://idam.example.com");
        let caller = auth.verify(&tok).await.expect("valid token");
        assert_eq!(caller.subject, "client:abc");
        assert_eq!(caller.roles, vec!["admin".to_string()]);
        assert!(caller.tenant_id.is_some());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn tampered_payload_rejected() {
        let (kp, kid) = keypair();
        let auth = authenticator_with(&kp, &kid, None).await;
        let tok = sign_token(&kp, &kid, now_unix() + 300, "x");
        let mut parts: Vec<&str> = tok.split('.').collect();
        // Swap the payload for a different one; signature no longer matches.
        let evil = b64(br#"{"sub":"attacker","exp":9999999999}"#);
        parts[1] = &evil;
        let tampered = parts.join(".");
        assert!(matches!(
            auth.verify(&tampered).await,
            Err(AuthError::BadSignature)
        ));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn expired_token_rejected() {
        let (kp, kid) = keypair();
        let auth = authenticator_with(&kp, &kid, None).await;
        let tok = sign_token(&kp, &kid, now_unix() - 10, "x");
        assert!(matches!(auth.verify(&tok).await, Err(AuthError::Expired)));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn wrong_issuer_rejected() {
        let (kp, kid) = keypair();
        let auth = authenticator_with(&kp, &kid, Some("https://expected")).await;
        let tok = sign_token(&kp, &kid, now_unix() + 300, "https://attacker");
        assert!(matches!(
            auth.verify(&tok).await,
            Err(AuthError::IssuerMismatch)
        ));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn unknown_kid_rejected() {
        let (kp, kid) = keypair();
        let auth = authenticator_with(&kp, &kid, None).await;
        let tok = sign_token(&kp, "other-kid", now_unix() + 300, "x");
        // cache has only test-kid; refresh() will try the unused URL and fail.
        assert!(auth.verify(&tok).await.is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn dev_fallback_uses_x_actor() {
        let auth = Authenticator::new(AuthConfig {
            jwks_url: None,
            issuer: None,
            cache_ttl: Duration::from_secs(1),
        });
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("x-actor", "charles@dev".parse().unwrap());
        let caller = auth.caller(&headers).await.unwrap();
        assert_eq!(caller.subject, "charles@dev");
    }
}
