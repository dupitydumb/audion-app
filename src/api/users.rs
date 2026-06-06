use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::AppState;
use crate::auth::{hash_password, Claims};

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub role: String,
    pub listenbrainz_token: Option<String>,
    pub is_enabled: i32,
    pub created_at: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: String,
}

#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub username: Option<String>,
    pub password: Option<String>,
    pub role: Option<String>,
    pub listenbrainz_token: Option<String>,
    pub is_enabled: Option<i32>,
}

// Check if requester has Admin role
fn require_admin(claims: &Claims) -> Result<(), (StatusCode, &'static str)> {
    if claims.role != "Admin" {
        return Err((StatusCode::FORBIDDEN, "Administrator privileges required"));
    }
    Ok(())
}

pub async fn list_users(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<Vec<UserInfo>>, (StatusCode, &'static str)> {
    require_admin(&claims)?;

    let users = sqlx::query_as::<_, UserInfo>(
        "SELECT id, username, role, listenbrainz_token, is_enabled, created_at FROM users ORDER BY username ASC"
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to retrieve users"))?;

    Ok(Json(users))
}

pub async fn create_user(
    claims: Claims,
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserInfo>), (StatusCode, String)> {
    require_admin(&claims).map_err(|(s, m)| (s, m.to_string()))?;

    if payload.username.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Username cannot be empty".to_string()));
    }
    if payload.password.trim().len() < 6 {
        return Err((StatusCode::BAD_REQUEST, "Password must be at least 6 characters".to_string()));
    }

    let valid_roles = vec!["Admin", "User", "StreamOnly"];
    if !valid_roles.contains(&payload.role.as_str()) {
        return Err((StatusCode::BAD_REQUEST, "Invalid role specified".to_string()));
    }

    // Check if user already exists
    let exists = sqlx::query("SELECT id FROM users WHERE username = ?")
        .bind(&payload.username)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .is_some();

    if exists {
        return Err((StatusCode::CONFLICT, "Username already exists".to_string()));
    }

    let user_id = Uuid::new_v4().to_string();
    let password_hash = hash_password(&payload.password)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Password hashing failed: {}", e)))?;

    let encrypted_subsonic_password = crate::auth::encrypt_subsonic_password(&payload.password, &state.config.jwt_secret)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Subsonic password encryption failed: {}", e)))?;

    sqlx::query(
        "INSERT INTO users (id, username, password_hash, role, is_enabled, subsonic_password) VALUES (?, ?, ?, ?, 1, ?)"
    )
    .bind(&user_id)
    .bind(&payload.username)
    .bind(&password_hash)
    .bind(&payload.role)
    .bind(&encrypted_subsonic_password)
    .execute(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let user_info = UserInfo {
        id: user_id,
        username: payload.username,
        role: payload.role,
        listenbrainz_token: None,
        is_enabled: 1,
        created_at: Some(chrono::Utc::now().to_rfc3339()),
    };

    Ok((StatusCode::CREATED, Json(user_info)))
}

pub async fn update_user(
    claims: Claims,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<Json<UserInfo>, (StatusCode, String)> {
    // A user can update their own ListenBrainz token, but other changes require admin
    if claims.sub != id {
        require_admin(&claims).map_err(|(s, m)| (s, m.to_string()))?;
    }

    // Standard users are only allowed to update their listenbrainz_token
    if claims.role != "Admin" {
        if payload.username.is_some() || payload.password.is_some() || payload.role.is_some() || payload.is_enabled.is_some() {
            return Err((StatusCode::FORBIDDEN, "Non-administrator users cannot update protected fields (username, password, role, or active status) here".to_string()));
        }
    }

    // Get current user details
    let mut current_user = sqlx::query_as::<_, UserInfo>(
        "SELECT id, username, role, listenbrainz_token, is_enabled, created_at FROM users WHERE id = ?"
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "User not found".to_string()))?;

    // Admins can change username, role, and enabled status. Standard users can't change their own role/enabled status.
    let updated_username = if claims.role == "Admin" {
        if let Some(ref u) = payload.username {
            if u.trim().is_empty() {
                return Err((StatusCode::BAD_REQUEST, "Username cannot be empty".to_string()));
            }
            // Check if username taken
            let exists = sqlx::query("SELECT id FROM users WHERE username = ? AND id != ?")
                .bind(u)
                .bind(&id)
                .fetch_optional(&state.pool)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
                .is_some();
            if exists {
                return Err((StatusCode::CONFLICT, "Username already taken".to_string()));
            }
            u.trim().to_string()
        } else {
            current_user.username.clone()
        }
    } else {
        current_user.username.clone()
    };

    let updated_role = if claims.role == "Admin" {
        if let Some(ref r) = payload.role {
            let valid_roles = vec!["Admin", "User", "StreamOnly"];
            if !valid_roles.contains(&r.as_str()) {
                return Err((StatusCode::BAD_REQUEST, "Invalid role specified".to_string()));
            }
            r.clone()
        } else {
            current_user.role.clone()
        }
    } else {
        current_user.role.clone()
    };

    let updated_enabled = if claims.role == "Admin" {
        if let Some(e) = payload.is_enabled {
            // Can't disable yourself if you're the only admin
            if id == claims.sub && e == 0 {
                return Err((StatusCode::BAD_REQUEST, "You cannot disable your own administrator account".to_string()));
            }
            e
        } else {
            current_user.is_enabled
        }
    } else {
        current_user.is_enabled
    };

    let updated_lb_token = if let Some(ref token) = payload.listenbrainz_token {
        if token.trim().is_empty() {
            None
        } else {
            Some(token.trim().to_string())
        }
    } else {
        current_user.listenbrainz_token.clone()
    };

    // Update DB
    if let Some(ref p) = payload.password {
        if p.trim().len() < 6 {
            return Err((StatusCode::BAD_REQUEST, "Password must be at least 6 characters".to_string()));
        }
        let password_hash = hash_password(p)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Password hashing failed: {}", e)))?;
        
        let encrypted_subsonic_password = crate::auth::encrypt_subsonic_password(p, &state.config.jwt_secret)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Subsonic password encryption failed: {}", e)))?;
        
        sqlx::query(
            "UPDATE users SET username = ?, password_hash = ?, role = ?, listenbrainz_token = ?, is_enabled = ?, subsonic_password = ? WHERE id = ?"
        )
        .bind(&updated_username)
        .bind(&password_hash)
        .bind(&updated_role)
        .bind(&updated_lb_token)
        .bind(updated_enabled)
        .bind(&encrypted_subsonic_password)
        .bind(&id)
        .execute(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    } else {
        sqlx::query(
            "UPDATE users SET username = ?, role = ?, listenbrainz_token = ?, is_enabled = ? WHERE id = ?"
        )
        .bind(&updated_username)
        .bind(&updated_role)
        .bind(&updated_lb_token)
        .bind(updated_enabled)
        .bind(&id)
        .execute(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    current_user.username = updated_username;
    current_user.role = updated_role;
    current_user.is_enabled = updated_enabled;
    current_user.listenbrainz_token = updated_lb_token;

    Ok(Json(current_user))
}

pub async fn delete_user(
    claims: Claims,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    require_admin(&claims).map_err(|(s, m)| (s, m.to_string()))?;

    if id == claims.sub {
        return Err((StatusCode::BAD_REQUEST, "You cannot delete your own account".to_string()));
    }

    let rows_affected = sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(&id)
        .execute(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .rows_affected();

    if rows_affected == 0 {
        return Err((StatusCode::NOT_FOUND, "User not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}
