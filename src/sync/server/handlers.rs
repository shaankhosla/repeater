use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::sync::types::{PullResponse, PushRequest, PushResponse, SyncStatus};

use super::AppState;

#[derive(Deserialize)]
pub struct PullQuery {
    pub since_version: i64,
}

async fn extract_user_id(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Uuid, (StatusCode, String)> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "Missing Authorization header".to_string(),
        ))?;

    let token = auth_header.strip_prefix("Token ").ok_or((
        StatusCode::UNAUTHORIZED,
        "Invalid Authorization header format".to_string(),
    ))?;

    state
        .db
        .get_user_id_by_token(token)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error".to_string(),
            )
        })?
        .ok_or((StatusCode::UNAUTHORIZED, "Invalid token".to_string()))
}

pub async fn push(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<PushRequest>,
) -> impl IntoResponse {
    let user_id = match extract_user_id(&state, &headers).await {
        Ok(id) => id,
        Err(e) => return e.into_response(),
    };

    match state.db.push_cards(user_id, &req.cards).await {
        Ok(updated) => Json(PushResponse { updated }).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Push failed: {}", e),
        )
            .into_response(),
    }
}

pub async fn pull(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PullQuery>,
) -> impl IntoResponse {
    let user_id = match extract_user_id(&state, &headers).await {
        Ok(id) => id,
        Err(e) => return e.into_response(),
    };

    match state.db.pull_cards(user_id, query.since_version).await {
        Ok((cards, latest_version)) => Json(PullResponse {
            cards,
            latest_version,
        })
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Pull failed: {}", e),
        )
            .into_response(),
    }
}

pub async fn status(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let user_id = match extract_user_id(&state, &headers).await {
        Ok(id) => id,
        Err(e) => return e.into_response(),
    };

    match state.db.get_status(user_id).await {
        Ok((card_count, latest_version)) => Json(SyncStatus {
            card_count,
            latest_version,
        })
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Status failed: {}", e),
        )
            .into_response(),
    }
}
