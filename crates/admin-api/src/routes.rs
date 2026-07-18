//! Provisioning routes, slice 1: tenants, domains, accounts.
//!
//! Identity: the `og_auth::require_auth` layer verifies every `/api` request
//! (sesame EdDSA token, PRD F4) and injects the [`Caller`]; handlers read it
//! via `Extension<Caller>`. In dev (no `OG_AUTH_JWKS_URL`) the layer falls
//! back to the `x-actor` header. Accounts are created `pending_provisioning`;
//! the sesame-idam + Stalwart provisioning calls attach here.

use axum::extract::{Path, State};
use axum::middleware;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use uuid::Uuid;

use og_auth::Caller;
use og_db::{record, ApiError, AuditEntry, Db};

use crate::models::{
    validate_fqdn, validate_local_part, validate_policy, validate_slug, AbusePolicy, Account,
    CreateAccount, CreateDomain, CreateTenant, Domain, SetAbusePolicy, Tenant,
};

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    /// None when SESAME_* env is absent (dev without an identity stack):
    /// accounts then stay `pending_provisioning` for later reconciliation.
    pub sesame: Option<std::sync::Arc<wrappers::sesame_client::SesameClient>>,
    /// Verifies sesame EdDSA tokens (PRD F4). In dev (no OG_AUTH_JWKS_URL)
    /// it trusts the `x-actor` header instead.
    pub auth: og_auth::Authenticator,
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
        .route(
            "/api/v1/tenants/{tenant_id}/policy",
            get(get_policy).put(put_policy),
        )
        .layer(middleware::from_fn_with_state(
            state.auth.clone(),
            og_auth::require_auth,
        ))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Tenants (platform scope)
// ---------------------------------------------------------------------------

async fn create_tenant(
    State(state): State<AppState>,
    Extension(caller): Extension<Caller>,
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
            actor: caller.actor(),
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
    Extension(caller): Extension<Caller>,
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
            actor: caller.actor(),
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
    Extension(caller): Extension<Caller>,
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
            actor: caller.actor(),
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
            provision_into_sesame(&state, sesame, tenant_id, account, caller.actor()).await
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

// ---------------------------------------------------------------------------
// Abuse policy (tenant scope) — desired state consumed by config-compiler
// ---------------------------------------------------------------------------

/// Return the tenant's effective abuse policy: its stored row, or the platform
/// defaults when it has never set one.
async fn get_policy(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
) -> Result<Json<AbusePolicy>, ApiError> {
    let mut tx = state.db.tenant_tx(tenant_id).await?;
    let policy: Option<AbusePolicy> = sqlx::query_as(
        "SELECT tenant_id, reject, add_header, greylist, updated_at
         FROM abuse_policy WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(
        policy.unwrap_or_else(|| AbusePolicy::defaults(tenant_id)),
    ))
}

/// Upsert the tenant's abuse policy. config-compiler picks it up on its next
/// compile cycle.
async fn put_policy(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    Extension(caller): Extension<Caller>,
    Json(req): Json<SetAbusePolicy>,
) -> Result<Json<AbusePolicy>, ApiError> {
    validate_policy(&req)?;
    let mut tx = state.db.tenant_tx(tenant_id).await?;
    let policy: AbusePolicy = sqlx::query_as(
        "INSERT INTO abuse_policy (tenant_id, reject, add_header, greylist)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (tenant_id) DO UPDATE
           SET reject = $2, add_header = $3, greylist = $4, updated_at = now()
         RETURNING tenant_id, reject, add_header, greylist, updated_at",
    )
    .bind(tenant_id)
    .bind(req.reject)
    .bind(req.add_header)
    .bind(req.greylist)
    .fetch_one(&mut *tx)
    .await?;

    record(
        &mut tx,
        AuditEntry {
            tenant_id: Some(tenant_id),
            actor: caller.actor(),
            action: "policy.set",
            entity_type: "abuse_policy",
            entity_id: tenant_id.to_string(),
            payload: serde_json::json!({
                "reject": req.reject, "add_header": req.add_header, "greylist": req.greylist
            }),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(policy))
}
