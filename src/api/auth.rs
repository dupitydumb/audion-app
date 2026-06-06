use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use crate::auth::{generate_token, verify_password, hash_password, Claims};

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct UpdateProfileRequest {
    pub current_password: String,
    pub new_username: Option<String>,
    pub new_password: Option<String>,
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
    pub role: String,
    pub listenbrainz_token: Option<String>,
}

#[derive(sqlx::FromRow)]
struct DbUser {
    id: String,
    username: String,
    password_hash: String,
    role: String,
    listenbrainz_token: Option<String>,
    is_enabled: i32,
}

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, &'static str)> {
    let user = sqlx::query_as::<_, DbUser>(
        "SELECT id, username, password_hash, role, listenbrainz_token, is_enabled FROM users WHERE username = ?"
    )
    .bind(&payload.username)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
    .ok_or((StatusCode::UNAUTHORIZED, "Invalid username or password"))?;

    if user.is_enabled == 0 {
        return Err((StatusCode::FORBIDDEN, "Account is disabled. Please contact your administrator."));
    }

    if !verify_password(&payload.password, &user.password_hash) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid username or password"));
    }

    let token = generate_token(&user.id, &user.username, &user.role, &state.config.jwt_secret)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Token generation failed"))?;

    Ok(Json(LoginResponse {
        token,
        user: UserResponse {
            id: user.id,
            username: user.username,
            role: user.role,
            listenbrainz_token: user.listenbrainz_token,
        },
    }))
}

pub async fn me(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<UserResponse>, (StatusCode, &'static str)> {
    let user = sqlx::query_as::<_, DbUser>(
        "SELECT id, username, password_hash, role, listenbrainz_token, is_enabled FROM users WHERE id = ?"
    )
    .bind(&claims.sub)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
    .ok_or((StatusCode::UNAUTHORIZED, "User not found"))?;

    if user.is_enabled == 0 {
        return Err((StatusCode::FORBIDDEN, "Account is disabled. Please contact your administrator."));
    }

    Ok(Json(UserResponse {
        id: user.id,
        username: user.username,
        role: user.role,
        listenbrainz_token: user.listenbrainz_token,
    }))
}

pub async fn update_profile(
    claims: Claims,
    State(state): State<AppState>,
    Json(payload): Json<UpdateProfileRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    let user_id = claims.sub;

    // Fetch current user from database
    let user = sqlx::query_as::<_, DbUser>(
        "SELECT id, username, password_hash, role, listenbrainz_token, is_enabled FROM users WHERE id = ?"
    )
    .bind(&user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::UNAUTHORIZED, "User not found".to_string()))?;

    if user.is_enabled == 0 {
        return Err((StatusCode::FORBIDDEN, "Account is disabled.".to_string()));
    }

    // Verify current password
    if !verify_password(&payload.current_password, &user.password_hash) {
        return Err((StatusCode::UNAUTHORIZED, "Incorrect current password".to_string()));
    }

    // Determine new username and password hash
    let updated_username = match payload.new_username {
        Some(ref u) if !u.trim().is_empty() => {
            let exists = sqlx::query("SELECT id FROM users WHERE username = ? AND id != ?")
                .bind(u)
                .bind(&user_id)
                .fetch_optional(&state.pool)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
                .is_some();
            if exists {
                return Err((StatusCode::BAD_REQUEST, "Username already taken".to_string()));
            }
            u.trim().to_string()
        }
        _ => user.username.clone(),
    };

    // Update user row
    if let Some(ref p) = payload.new_password {
        if !p.trim().is_empty() {
            if p.len() < 6 {
                return Err((StatusCode::BAD_REQUEST, "New password must be at least 6 characters long".to_string()));
            }
            let password_hash = hash_password(p)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to hash password: {}", e)))?;
            
            let encrypted_subsonic = crate::auth::encrypt_subsonic_password(p, &state.config.jwt_secret)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to encrypt subsonic password: {}", e)))?;
            
            sqlx::query("UPDATE users SET username = ?, password_hash = ?, subsonic_password = ? WHERE id = ?")
                .bind(&updated_username)
                .bind(&password_hash)
                .bind(&encrypted_subsonic)
                .bind(&user_id)
                .execute(&state.pool)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        } else {
            sqlx::query("UPDATE users SET username = ? WHERE id = ?")
                .bind(&updated_username)
                .bind(&user_id)
                .execute(&state.pool)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
    } else {
        sqlx::query("UPDATE users SET username = ? WHERE id = ?")
            .bind(&updated_username)
            .bind(&user_id)
            .execute(&state.pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }


    // Generate new token
    let token = generate_token(&user_id, &updated_username, &user.role, &state.config.jwt_secret)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Token generation failed".to_string()))?;

    Ok(Json(LoginResponse {
        token,
        user: UserResponse {
            id: user_id,
            username: updated_username,
            role: user.role,
            listenbrainz_token: user.listenbrainz_token,
        },
    }))
}

