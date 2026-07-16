//! Provisioning routes, slice 1: tenants, domains, accounts.
//!
//! Actor identity: temporary `x-actor` header until sesame-idam OIDC/JWKS
//! middleware lands in service-kit (PRD-OPENGROUPWARE-RELYING-PARTY F1/F4).
//! Accounts are created `pending_provisioning`; the sesame-idam + Stalwart
//! provisioning calls attach here once the sesame OIDC milestone ships.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::routing::post;
use axum::{Json, Router};
use uuid::Uuid;

use crate::audit::{record, AuditEntry};
use crate::db::Db;
use crate::error::ApiError;
use crate::models::{
    validate_fqdn, validate_local_part, validate_slug, Account, CreateAccount, CreateDomain,
    CreateTenant, Domain, Tenant,
};

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    /// None when SESAME_* env is absent (dev without an identity stack):
    /// accounts then stay `pending_provisioning` for later reconciliation.
    pub sesame: Option<std::sync::Arc<wrappers::sesame_client::SesameClient>>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/tenants", post(create_tenant).get(list_tenants))
        .route(
            "/api/v1/tenants/{tenant_id}/domains",
            post(create_domain).get(list_domains),
        )
        .route(
            "/api/v1/tenants/{tenant_id}/accounts",
            post(create_account).get(list_accounts),
        )
        .with_state(state)
}

fn actor(headers: &HeaderMap) -> String {
    headers
        .get("x-actor")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string()
}

// ---------------------------------------------------------------------------
// Tenants (platform scope)
// ---------------------------------------------------------------------------

async fn create_tenant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateTenant>,
) -> Result<(axum::http::StatusCode, Json<Tenant>), ApiError> {
    validate_slug(&req.slug)?;
    let mut tx = state.db.platform_tx().await?;
    let tenant: Tenant = sqlx::query_as(
        "INSERT INTO tenants (slug, name, plan)
         VALUES ($1, $2, $3)
         RETURNING id, slug, name, plan, status, created_at",
    )
    .bind(&req.slug)
    .bind(&req.name)
    .bind(&req.plan)
    .fetch_one(&mut *tx)
    .await?;

    record(
        &mut tx,
        AuditEntry {
            tenant_id: Some(tenant.id),
            actor: &actor(&headers),
            action: "tenant.create",
            entity_type: "tenant",
            entity_id: tenant.id.to_string(),
            payload: serde_json::json!({ "slug": tenant.slug, "plan": tenant.plan }),
        },
    )
    .await?;
    tx.commit().await?;
    Ok((axum::http::StatusCode::CREATED, Json(tenant)))
}

async fn list_tenants(State(state): State<AppState>) -> Result<Json<Vec<Tenant>>, ApiError> {
    let mut tx = state.db.platform_tx().await?;
    let tenants: Vec<Tenant> = sqlx::query_as(
        "SELECT id, slug, name, plan, status, created_at FROM tenants ORDER BY created_at",
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(tenants))
}

// ---------------------------------------------------------------------------
// Domains (tenant scope — RLS enforced by tenant_tx)
// ---------------------------------------------------------------------------

async fn create_domain(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<CreateDomain>,
) -> Result<(axum::http::StatusCode, Json<Domain>), ApiError> {
    validate_fqdn(&req.fqdn)?;
    let mut tx = state.db.tenant_tx(tenant_id).await?;
    let domain: Domain = sqlx::query_as(
        "INSERT INTO domains (tenant_id, fqdn)
         VALUES ($1, $2)
         RETURNING id, tenant_id, fqdn, status, created_at",
    )
    .bind(tenant_id)
    .bind(&req.fqdn)
    .fetch_one(&mut *tx)
    .await?;

    record(
        &mut tx,
        AuditEntry {
            tenant_id: Some(tenant_id),
            actor: &actor(&headers),
            action: "domain.create",
            entity_type: "domain",
            entity_id: domain.id.to_string(),
            payload: serde_json::json!({ "fqdn": domain.fqdn }),
        },
    )
    .await?;
    tx.commit().await?;
    Ok((axum::http::StatusCode::CREATED, Json(domain)))
}

async fn list_domains(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
) -> Result<Json<Vec<Domain>>, ApiError> {
    let mut tx = state.db.tenant_tx(tenant_id).await?;
    let domains: Vec<Domain> = sqlx::query_as(
        "SELECT id, tenant_id, fqdn, status, created_at FROM domains ORDER BY created_at",
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(domains))
}

// ---------------------------------------------------------------------------
// Accounts (tenant scope)
// ---------------------------------------------------------------------------

async fn create_account(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<CreateAccount>,
) -> Result<(axum::http::StatusCode, Json<Account>), ApiError> {
    validate_local_part(&req.local_part)?;
    let mut tx = state.db.tenant_tx(tenant_id).await?;

    // Domain must exist within this tenant (RLS hides other tenants' rows,
    // so a foreign domain_id 404s here rather than leaking existence).
    let fqdn: Option<String> =
        sqlx::query_scalar("SELECT fqdn FROM domains WHERE id = $1 AND status <> 'suspended'")
            .bind(req.domain_id)
            .fetch_optional(&mut *tx)
            .await?;
    let fqdn = fqdn.ok_or(ApiError::NotFound)?;
    let email = format!("{}@{}", req.local_part, fqdn);

    let account: Account = sqlx::query_as(
        "INSERT INTO accounts (tenant_id, domain_id, email, display_name, quota_mb)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, tenant_id, domain_id, email, display_name, quota_mb, status, created_at",
    )
    .bind(tenant_id)
    .bind(req.domain_id)
    .bind(&email)
    .bind(&req.display_name)
    .bind(req.quota_mb)
    .fetch_one(&mut *tx)
    .await?;

    record(
        &mut tx,
        AuditEntry {
            tenant_id: Some(tenant_id),
            actor: &actor(&headers),
            action: "account.create",
            entity_type: "account",
            entity_id: account.id.to_string(),
            payload: serde_json::json!({ "email": email, "quota_mb": account.quota_mb }),
        },
    )
    .await?;
    tx.commit().await?;

    // Provision the identity in sesame-idam (ADR-0006 v2). Deliberately
    // after commit: the row exists as pending_provisioning regardless, and
    // a sesame outage must not fail account creation — reconciliation can
    // retry. Success flips the account active in a second transaction.
    let account = match &state.sesame {
        Some(sesame) => {
            provision_into_sesame(&state, sesame, tenant_id, account, &actor(&headers)).await
        }
        None => account,
    };

    Ok((axum::http::StatusCode::CREATED, Json(account)))
}

/// Best-effort sesame provisioning; returns the (possibly updated) account.
async fn provision_into_sesame(
    state: &AppState,
    sesame: &wrappers::sesame_client::SesameClient,
    tenant_id: Uuid,
    account: Account,
    actor: &str,
) -> Account {
    let provisioned = sesame
        .provision_user(&account.email, &account.display_name)
        .await;
    let sesame_user_id = match provisioned {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!(
                account = %account.id, error = %e,
                "sesame provisioning failed — account stays pending_provisioning"
            );
            return account;
        }
    };

    let update = async {
        let mut tx = state.db.tenant_tx(tenant_id).await?;
        let updated: Account = sqlx::query_as(
            "UPDATE accounts
             SET status = 'active', sesame_user_id = $2, updated_at = now()
             WHERE id = $1
             RETURNING id, tenant_id, domain_id, email, display_name, quota_mb,
                       status, created_at",
        )
        .bind(account.id)
        .bind(sesame_user_id)
        .fetch_one(&mut *tx)
        .await?;
        record(
            &mut tx,
            AuditEntry {
                tenant_id: Some(tenant_id),
                actor,
                action: "account.provisioned",
                entity_type: "account",
                entity_id: updated.id.to_string(),
                payload: serde_json::json!({ "sesame_user_id": sesame_user_id }),
            },
        )
        .await?;
        tx.commit().await?;
        Ok::<Account, sqlx::Error>(updated)
    };

    match update.await {
        Ok(updated) => updated,
        Err(e) => {
            tracing::error!(
                account = %account.id, error = %e,
                "sesame user created but local activation failed — reconciliation needed"
            );
            account
        }
    }
}

async fn list_accounts(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
) -> Result<Json<Vec<Account>>, ApiError> {
    let mut tx = state.db.tenant_tx(tenant_id).await?;
    let accounts: Vec<Account> = sqlx::query_as(
        "SELECT id, tenant_id, domain_id, email, display_name, quota_mb, status, created_at
         FROM accounts ORDER BY created_at",
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(accounts))
}
