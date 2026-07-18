//! Mock sesame identity provider for CI / integration tests.
//!
//! A bug like sesame's non-RFC-8037 JWKS casing (`kty:"okp"` instead of
//! `"OKP"`) broke token verification for every consumer and reached
//! production because nothing in CI exercised the real fetch+verify path
//! against a realistic JWKS. [`MockIdp`] closes that gap: it generates an
//! Ed25519 key, produces a JWKS document with the *exact* RFC 8037 shape a
//! standards-compliant verifier expects, and mints valid EdDSA tokens.
//!
//! Enable with the `mock` feature. Two modes:
//!  * [`MockIdp::jwks_json`] + [`MockIdp::mint`] — pre-seed a cache or assert
//!    on the wire shape without any network.
//!  * [`MockIdp::serve`] — bind a real HTTP JWKS endpoint so the full
//!    [`Authenticator`](crate::Authenticator) fetch path is tested.

// Test/CI helper: expect/unwrap are acceptable here.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair};

/// In-process stand-in for sesame-idam's signing + JWKS.
pub struct MockIdp {
    keypair: Ed25519KeyPair,
    kid: String,
    issuer: String,
}

impl MockIdp {
    /// Generate a fresh mock IdP with the given `kid` and `iss`.
    ///
    /// # Panics
    /// Panics only if the system RNG fails (test-only helper).
    #[must_use]
    pub fn new(kid: &str, issuer: &str) -> Self {
        let rng = SystemRandom::new();
        let doc = Ed25519KeyPair::generate_pkcs8(&rng).expect("rng");
        let keypair = Ed25519KeyPair::from_pkcs8(doc.as_ref()).expect("pkcs8");
        Self {
            keypair,
            kid: kid.to_string(),
            issuer: issuer.to_string(),
        }
    }

    #[must_use]
    pub fn kid(&self) -> &str {
        &self.kid
    }

    /// The JWKS document as sesame *should* serve it — RFC 8037 casing:
    /// `kty:"OKP"`, `crv:"Ed25519"`. This is the regression guard: if a
    /// verifier passes against this, it will pass against a compliant sesame.
    #[must_use]
    pub fn jwks_json(&self) -> String {
        let x = URL_SAFE_NO_PAD.encode(self.keypair.public_key().as_ref());
        serde_json::json!({
            "keys": [{
                "kty": "OKP",
                "crv": "Ed25519",
                "use": "sig",
                "alg": "EdDSA",
                "kid": self.kid,
                "x": x,
            }]
        })
        .to_string()
    }

    /// Mint a signed EdDSA access token (header `{alg:EdDSA,typ:at+jwt,kid}`).
    #[must_use]
    pub fn mint(&self, subject: &str, tenant_id: &str, roles: &[&str], ttl_secs: i64) -> String {
        let now = now_unix();
        let header = serde_json::json!({"alg":"EdDSA","typ":"at+jwt","kid":self.kid});
        let payload = serde_json::json!({
            "sub": subject,
            "iss": self.issuer,
            "aud": ["sesame-idam"],
            "iat": now,
            "nbf": now,
            "exp": now + ttl_secs,
            "tenant_id": tenant_id,
            "sx": { "roles": roles },
        });
        let h = URL_SAFE_NO_PAD.encode(header.to_string());
        let p = URL_SAFE_NO_PAD.encode(payload.to_string());
        let signing_input = format!("{h}.{p}");
        let sig = self.keypair.sign(signing_input.as_bytes());
        format!("{signing_input}.{}", URL_SAFE_NO_PAD.encode(sig.as_ref()))
    }

    /// Serve the JWKS on an ephemeral localhost port; returns the base URL
    /// (e.g. `http://127.0.0.1:PORT`). The JWKS is at `{base}/.well-known/
    /// jwks.json`. The server runs until the returned handle is dropped.
    ///
    /// # Errors
    /// Returns an IO error if the listener cannot bind.
    pub async fn serve(&self) -> std::io::Result<(String, tokio::task::JoinHandle<()>)> {
        use axum::routing::get;
        use axum::Router;
        let body = self.jwks_json();
        let app = Router::new().route(
            "/.well-known/jwks.json",
            get(move || {
                let body = body.clone();
                async move { ([("content-type", "application/json")], body) }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Ok((format!("http://{addr}"), handle))
    }
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
    use crate::{AuthConfig, Authenticator};
    use std::time::Duration;

    /// Full fetch+parse+verify path through a real HTTP JWKS — the CI guard
    /// that would have caught sesame's `kty:"okp"` casing bug. If the mock
    /// served the wrong casing, the Authenticator would skip the key and
    /// this test would fail.
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn end_to_end_fetch_and_verify() {
        let idp = MockIdp::new("ci-kid", "https://idam.example.com");
        let (base, _handle) = idp.serve().await.unwrap();
        let auth = Authenticator::new(AuthConfig {
            jwks_url: Some(format!("{base}/.well-known/jwks.json")),
            issuer: Some("https://idam.example.com".to_string()),
            cache_ttl: Duration::from_secs(600),
        });
        let token = idp.mint(
            "client:ci",
            "11111111-1111-1111-1111-111111111111",
            &["admin"],
            300,
        );
        let caller = auth.caller_bearer(&token).await.expect("token verifies");
        assert_eq!(caller.subject, "client:ci");
        assert_eq!(caller.roles, vec!["admin".to_string()]);
    }

    /// The JWKS the mock serves MUST use the RFC 8037 casing verifiers expect.
    #[test]
    fn jwks_uses_rfc8037_casing() {
        let idp = MockIdp::new("k", "iss");
        let doc: serde_json::Value = serde_json::from_str(&idp.jwks_json()).unwrap();
        assert_eq!(doc["keys"][0]["kty"], "OKP");
        assert_eq!(doc["keys"][0]["crv"], "Ed25519");
    }
}
