//! API error type — maps domain/database failures onto HTTP responses.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("validation: {0}")]
    Validation(String),
    #[error("database error")]
    Db(#[from] sqlx::Error),
}

impl ApiError {
    fn status(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Db(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // Unique-violation → 409 with a stable shape; never leak SQL detail.
        let (status, message) = match &self {
            Self::Db(sqlx::Error::Database(dbe)) if dbe.is_unique_violation() => {
                (StatusCode::CONFLICT, "already exists".to_string())
            }
            Self::Db(e) => {
                tracing::error!(error = %e, "database error");
                (self.status(), "internal error".to_string())
            }
            other => (other.status(), other.to_string()),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
