use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use uuid::Uuid;

use crate::sync::types::{AuthRequest, AuthResponse};

use super::AppState;

fn generate_token() -> String {
    Uuid::new_v4().to_string()
}

fn hash_password(password: &str) -> Result<String, StatusCode> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(hash.to_string())
}

fn verify_password(password: &str, hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<AuthRequest>,
) -> impl IntoResponse {
    if !state.config.open_registration {
        return (StatusCode::FORBIDDEN, "Registration is closed".to_string()).into_response();
    }

    if req.username.is_empty() || req.password.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            "Username and password required".to_string(),
        )
            .into_response();
    }

    if !req
        .username
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-')
    {
        return (
            StatusCode::BAD_REQUEST,
            "Username must be alphanumeric (hyphens allowed)".to_string(),
        )
            .into_response();
    }

    if req.password.len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            "Password must be at least 8 characters".to_string(),
        )
            .into_response();
    }

    let password_hash = match hash_password(&req.password) {
        Ok(h) => h,
        Err(status) => return (status, "Failed to hash password".to_string()).into_response(),
    };

    let user_id = match state.db.create_user(&req.username, &password_hash).await {
        Ok(id) => id,
        Err(_) => {
            return (StatusCode::CONFLICT, "Username already taken".to_string()).into_response();
        }
    };

    let token = generate_token();
    if let Err(_) = state.db.create_session(user_id, &token).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to create session".to_string(),
        )
            .into_response();
    }

    Json(AuthResponse { token }).into_response()
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<AuthRequest>,
) -> impl IntoResponse {
    let (user_id, password_hash) = match state.db.get_user_by_username(&req.username).await {
        Ok(Some(row)) => row,
        Ok(None) => {
            return (StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()).into_response();
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error".to_string(),
            )
                .into_response();
        }
    };

    if !verify_password(&req.password, &password_hash) {
        return (StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()).into_response();
    }

    let token = generate_token();
    if let Err(_) = state.db.create_session(user_id, &token).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to create session".to_string(),
        )
            .into_response();
    }

    Json(AuthResponse { token }).into_response()
}
