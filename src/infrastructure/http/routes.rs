//! axum route handlers for the read-only `/api` endpoints.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::domain::model::sms_message::SmsMessage;
use crate::domain::port::sms_repository::{
    MessageFilter, MessagePage, ModemEventRecord, ModemStatusRecord, StatusCounts,
};
use crate::infrastructure::http::error::ApiError;
use crate::infrastructure::http::state::AppState;

/// Build the `/api` sub-router mounted by [super::app_router].
pub fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/stats", get(stats))
        .route("/messages", get(list_messages))
        .route("/messages/{id}", get(get_message))
        .route("/modem/status", get(modem_status))
        .route("/modem/events", get(modem_events))
}

// ── handlers ──────────────────────────────────────────────────────────────

/// Query params for `GET /api/messages`.
#[derive(Debug, Default, Deserialize)]
pub struct ListParams {
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    /// Exact status: pending | sending | sent | failed | decode_failed.
    pub status: Option<String>,
    /// Free-text search across sender + content.
    pub q: Option<String>,
}

async fn list_messages(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Json<MessagePage>, ApiError> {
    let filter = MessageFilter {
        limit: params.limit.unwrap_or(0),
        offset: params.offset.unwrap_or(0),
        status: params.status.and_then(|s| {
            let t = s.trim().to_lowercase();
            if t.is_empty() { None } else { Some(t) }
        }),
        query: params.q.and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() { None } else { Some(t) }
        }),
    };
    let page = state.repo.list_messages(filter).await?;
    Ok(Json(page))
}

async fn get_message(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Option<SmsMessage>>, ApiError> {
    let msg = state.repo.get_message(id).await?;
    Ok(Json(msg))
}

async fn stats(State(state): State<AppState>) -> Result<Json<StatusCounts>, ApiError> {
    let counts = state.repo.count_by_status().await?;
    Ok(Json(counts))
}

async fn modem_status(
    State(state): State<AppState>,
) -> Result<Json<Option<ModemStatusRecord>>, ApiError> {
    let s = state.repo.latest_modem_status().await?;
    Ok(Json(s))
}

/// Query params for `GET /api/modem/events`.
#[derive(Debug, Default, Deserialize)]
pub struct EventsParams {
    pub limit: Option<u64>,
}

async fn modem_events(
    State(state): State<AppState>,
    Query(params): Query<EventsParams>,
) -> Result<Json<Vec<ModemEventRecord>>, ApiError> {
    let events = state.repo.recent_modem_events(params.limit.unwrap_or(50)).await?;
    Ok(Json(events))
}

/// `GET /api/health` — lightweight liveness probe used by the frontend to
/// show an online/offline banner.
#[derive(Debug, Serialize)]
struct HealthBody {
    status: &'static str,
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, Json(HealthBody { status: "ok" }))
}
