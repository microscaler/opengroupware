//! Abuse/quarantine routes, slice 2.
//!
//! The product owns the quarantine WORKFLOW and the abuse DECISION record
//! (D6). Rspamd scoring and the actual mail-plane release (re-inject to the
//! mailbox via Stalwart) are integration points that land with the real
//! Rspamd/Stalwart clients; here the workflow + audit are real and tested.
//!
//! Actor identity: temporary `x-actor` header until the sesame JWKS
//! middleware lands (PRD-OPENGROUPWARE F4).

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use og_db::{record, ApiError, AuditEntry, Db};
use uuid::Uuid;

use crate::models::{
    AbuseDecision, QuarantineItem, RecordDecision, ReportFeedback, VALID_ACTIONS, VALID_VERDICTS,
};

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route(
            "/api/v1/tenants/{tenant_id}/abuse/decisions",
            post(record_decision),
        )
        .route(
            "/api/v1/tenants/{tenant_id}/quarantine",
            get(list_quarantine),
        )
        .route(
            "/api/v1/tenants/{tenant_id}/quarantine/{item_id}/release",
            post(release_item),
        )
        .route(
            "/api/v1/tenants/{tenant_id}/quarantine/{item_id}/report",
            post(report_item),
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
// Record a scan decision (called by the mail pipeline)
// ---------------------------------------------------------------------------

async fn record_decision(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<RecordDecision>,
) -> Result<(StatusCode, Json<AbuseDecision>), ApiError> {
    if !VALID_ACTIONS.contains(&req.action.as_str()) {
        return Err(ApiError::Validation(format!(
            "action must be one of {VALID_ACTIONS:?}"
        )));
    }
    if !VALID_VERDICTS.contains(&req.verdict.as_str()) {
        return Err(ApiError::Validation(format!(
            "verdict must be one of {VALID_VERDICTS:?}"
        )));
    }
    let symbols = if req.symbols.is_null() {
        serde_json::json!([])
    } else {
        req.symbols.clone()
    };

    let mut tx = state.db.tenant_tx(tenant_id).await?;
    let decision: AbuseDecision = sqlx::query_as(
        "INSERT INTO abuse_decision
           (tenant_id, message_ref, direction, recipient, sender, score, action, verdict, symbols)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
         RETURNING id, tenant_id, message_ref, direction, recipient, sender, score, action,
                   verdict, symbols, scanned_at",
    )
    .bind(tenant_id)
    .bind(&req.message_ref)
    .bind(&req.direction)
    .bind(&req.recipient)
    .bind(&req.sender)
    .bind(req.score)
    .bind(&req.action)
    .bind(&req.verdict)
    .bind(&symbols)
    .fetch_one(&mut *tx)
    .await?;

    // Held messages get a quarantine workflow row.
    if req.action == "quarantine" {
        sqlx::query(
            "INSERT INTO quarantine_item
               (tenant_id, decision_id, message_ref, recipient, sender, subject, reason)
             VALUES ($1,$2,$3,$4,$5,$6,$7)
             ON CONFLICT (tenant_id, message_ref) DO NOTHING",
        )
        .bind(tenant_id)
        .bind(decision.id)
        .bind(&req.message_ref)
        .bind(&req.recipient)
        .bind(&req.sender)
        .bind(&req.subject)
        .bind(format!("{} (score {:.1})", req.verdict, req.score))
        .execute(&mut *tx)
        .await?;
    }

    record(
        &mut tx,
        AuditEntry {
            tenant_id: Some(tenant_id),
            actor: &actor(&headers),
            action: "abuse.decision",
            entity_type: "abuse_decision",
            entity_id: decision.id.to_string(),
            payload: serde_json::json!({
                "action": req.action, "verdict": req.verdict, "score": req.score,
                "message_ref": req.message_ref,
            }),
        },
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(decision)))
}

// ---------------------------------------------------------------------------
// Quarantine console
// ---------------------------------------------------------------------------

async fn list_quarantine(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
) -> Result<Json<Vec<QuarantineItem>>, ApiError> {
    let mut tx = state.db.tenant_tx(tenant_id).await?;
    let items: Vec<QuarantineItem> = sqlx::query_as(
        "SELECT id, tenant_id, decision_id, message_ref, recipient, sender, subject, reason,
                status, reported_as, held_at, resolved_at, resolved_by
         FROM quarantine_item
         WHERE status = 'held'
         ORDER BY held_at DESC",
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(items))
}

async fn release_item(
    State(state): State<AppState>,
    Path((tenant_id, item_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<QuarantineItem>, ApiError> {
    let mut tx = state.db.tenant_tx(tenant_id).await?;
    // Mail-plane re-injection to the recipient mailbox happens via the
    // Stalwart client once wired; here we transition the workflow state.
    let item: Option<QuarantineItem> = sqlx::query_as(
        "UPDATE quarantine_item
         SET status = 'released', resolved_at = now(), resolved_by = $2
         WHERE id = $1 AND status = 'held'
         RETURNING id, tenant_id, decision_id, message_ref, recipient, sender, subject, reason,
                   status, reported_as, held_at, resolved_at, resolved_by",
    )
    .bind(item_id)
    .bind(actor(&headers))
    .fetch_optional(&mut *tx)
    .await?;
    let item = item.ok_or(ApiError::NotFound)?;

    record(
        &mut tx,
        AuditEntry {
            tenant_id: Some(tenant_id),
            actor: &actor(&headers),
            action: "quarantine.release",
            entity_type: "quarantine_item",
            entity_id: item.id.to_string(),
            payload: serde_json::json!({ "message_ref": item.message_ref }),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(item))
}

async fn report_item(
    State(state): State<AppState>,
    Path((tenant_id, item_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(req): Json<ReportFeedback>,
) -> Result<Json<QuarantineItem>, ApiError> {
    if req.verdict != "spam" && req.verdict != "ham" {
        return Err(ApiError::Validation(
            "verdict must be 'spam' or 'ham'".to_string(),
        ));
    }
    let mut tx = state.db.tenant_tx(tenant_id).await?;
    // The training signal (feed Rspamd Bayes learn spam/ham) is dispatched by
    // job-runner once the Rspamd client lands; here we record the feedback.
    let item: Option<QuarantineItem> = sqlx::query_as(
        "UPDATE quarantine_item
         SET reported_as = $2
         WHERE id = $1
         RETURNING id, tenant_id, decision_id, message_ref, recipient, sender, subject, reason,
                   status, reported_as, held_at, resolved_at, resolved_by",
    )
    .bind(item_id)
    .bind(&req.verdict)
    .fetch_optional(&mut *tx)
    .await?;
    let item = item.ok_or(ApiError::NotFound)?;

    record(
        &mut tx,
        AuditEntry {
            tenant_id: Some(tenant_id),
            actor: &actor(&headers),
            action: "quarantine.report",
            entity_type: "quarantine_item",
            entity_id: item.id.to_string(),
            payload: serde_json::json!({ "reported_as": req.verdict, "message_ref": item.message_ref }),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(item))
}
