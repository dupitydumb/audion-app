use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use crate::auth::{generate_token, verify_password, Claims};

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: String,
    pub username: String,
}

#[derive(sqlx::FromRow)]
struct DbUser {
    id: String,
    username: String,
    password_hash: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, &'static str)> {
    let user = sqlx::query_as::<_, DbUser>(
        "SELECT id, username, password_hash FROM users WHERE username = ?"
    )
    .bind(&payload.username)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
    .ok_or((StatusCode::UNAUTHORIZED, "Invalid username or password"))?;

    if !verify_password(&payload.password, &user.password_hash) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid username or password"));
    }

    let token = generate_token(&user.id, &user.username, &state.config.jwt_secret)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Token generation failed"))?;

    Ok(Json(LoginResponse {
        token,
        user: UserResponse {
            id: user.id,
            username: user.username,
        },
    }))
}

pub async fn me(
    claims: Claims,
) -> Json<UserResponse> {
    Json(UserResponse {
        id: claims.sub,
        username: claims.username,
    })
}
