//! axum error type that maps [AppError] into JSON `{ code, message }` responses.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use tracing::warn;

use crate::domain::error::AppError;

/// Wrapper carrying an [AppError] so handlers can `?`-propagate it while still
/// mapping to a sensible HTTP status at the response boundary.
#[derive(Debug)]
pub struct ApiError(pub AppError);

impl From<AppError> for ApiError {
    fn from(e: AppError) -> Self {
        ApiError(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // Read-only endpoints only ever surface database / not-found errors,
        // but map defensively so a future mutating endpoint is also covered.
        let (status, code) = match &self.0 {
            AppError::Database { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "database_error"),
            AppError::Config { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "config_error"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        };
        warn!(code = code, error = %self.0, "api error");
        let body = Json(json!({ "code": code, "message": self.0.to_string() }));
        (status, body).into_response()
    }
}
