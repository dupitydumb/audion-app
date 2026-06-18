use axum::{
    extract::{State, Query},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use crate::auth::Claims;
use tracing::{info, error};
use std::time::Duration;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StorageSettingsResponse {
    pub storage_type: String,
    pub s3_endpoint: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String, // Masked as "********" if exists
    pub s3_region: String,
    pub s3_force_path_style: bool,
}

#[derive(Deserialize, Debug)]
pub struct StorageSettingsUpdate {
    pub storage_type: String,
    pub s3_endpoint: Option<String>,
    pub s3_bucket: Option<String>,
    pub s3_access_key: Option<String>,
    pub s3_secret_key: Option<String>,
    pub s3_region: Option<String>,
    pub s3_force_path_style: Option<bool>,
}

#[derive(Deserialize, Debug)]
pub struct StorageSettingsQuery {
    pub test_only: Option<bool>,
}

async fn get_db_setting(pool: &sqlx::SqlitePool, key: &str) -> String {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
}

async fn save_db_setting(pool: &sqlx::SqlitePool, key: &str, value: &str) {
    let _ = sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)")
        .bind(key)
        .bind(value)
        .execute(pool)
        .await;
}

pub async fn get_storage_settings(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<StorageSettingsResponse>, (StatusCode, String)> {
    claims.require_admin().map_err(|(s, m)| (s, m.to_string()))?;

    let storage_type = get_db_setting(&state.pool, "storage_type").await;
    let storage_type = if storage_type.is_empty() { "local".to_string() } else { storage_type };
    let s3_endpoint = get_db_setting(&state.pool, "s3_endpoint").await;
    let s3_bucket = get_db_setting(&state.pool, "s3_bucket").await;
    let s3_access_key = get_db_setting(&state.pool, "s3_access_key").await;
    let s3_secret_key_raw = get_db_setting(&state.pool, "s3_secret_key").await;
    let s3_secret_key = if s3_secret_key_raw.is_empty() { "" } else { "********" }.to_string();
    let s3_region = get_db_setting(&state.pool, "s3_region").await;
    let s3_force_path_style = get_db_setting(&state.pool, "s3_force_path_style").await == "true";

    Ok(Json(StorageSettingsResponse {
        storage_type,
        s3_endpoint,
        s3_bucket,
        s3_access_key,
        s3_secret_key,
        s3_region,
        s3_force_path_style,
    }))
}

pub async fn update_storage_settings(
    claims: Claims,
    State(state): State<AppState>,
    Query(query): Query<StorageSettingsQuery>,
    Json(payload): Json<StorageSettingsUpdate>,
) -> Result<StatusCode, (StatusCode, String)> {
    claims.require_admin().map_err(|(s, m)| (s, m.to_string()))?;

    let test_only = query.test_only.unwrap_or(false);

    // If storage type is s3, perform test connection
    if payload.storage_type == "s3" {
        let endpoint = payload.s3_endpoint.clone().unwrap_or_default();
        let bucket = payload.s3_bucket.clone().unwrap_or_default();
        let access_key = payload.s3_access_key.clone().unwrap_or_default();
        let region = payload.s3_region.clone().unwrap_or_default();
        let force_path_style = payload.s3_force_path_style.unwrap_or(false);

        // Retrieve existing secret key from DB if the payload contains the mask "********"
        let is_new_key = match payload.s3_secret_key.as_deref() {
            Some("********") | None => false,
            _ => true,
        };

        let secret_key = if is_new_key {
            payload.s3_secret_key.clone().unwrap_or_default()
        } else {
            let db_val = get_db_setting(&state.pool, "s3_secret_key").await;
            if db_val.is_empty() {
                "".to_string()
            } else {
                match crate::auth::decrypt_subsonic_password(&db_val, &state.config.jwt_secret) {
                    Ok(decrypted) => decrypted,
                    Err(_) => db_val, // Fallback to plaintext if decryption fails (migration path)
                }
            }
        };

        if bucket.trim().is_empty() {
            return Err((StatusCode::BAD_REQUEST, "Bucket name cannot be empty".to_string()));
        }

        info!("Testing S3 storage connection to bucket: {}", bucket);

        // Validate endpoint URL format
        let endpoint_trimmed = endpoint.trim();
        if !endpoint_trimmed.is_empty() {
            if !endpoint_trimmed.starts_with("https://") && !endpoint_trimmed.starts_with("http://") {
                return Err((StatusCode::BAD_REQUEST, "Endpoint URL must start with https:// or http://".to_string()));
            }
            // Warn if endpoint contains bucket name (common Cloudflare R2 mistake)
            if endpoint_trimmed.contains(&bucket) {
                return Err((StatusCode::BAD_REQUEST, format!(
                    "Endpoint URL should NOT contain the bucket name. \
                    For Cloudflare R2, use: https://<ACCOUNT_ID>.r2.cloudflarestorage.com \
                    (without the bucket name '{}' in the URL)", bucket
                )));
            }
        }

        // Build temporary S3 client to test
        let client = crate::storage::build_s3_client(
            endpoint_trimmed,
            &bucket,
            &access_key,
            &secret_key,
            &region,
            force_path_style,
        ).map_err(|e| (StatusCode::BAD_REQUEST, format!("Failed to create S3 client configuration: {}", e)))?;

        // Use head_bucket as a lightweight read-only connectivity test.
        // This avoids issues with Cloudflare WAF blocking PUT requests during testing,
        // and works correctly with R2, B2, MinIO, and AWS S3.
        //
        // A 15-second timeout prevents the reverse proxy from issuing a 502 if the
        // S3 endpoint is unreachable and the SDK would otherwise hang indefinitely.
        let head_bucket_result = tokio::time::timeout(
            Duration::from_secs(15),
            client.head_bucket().bucket(&bucket).send(),
        )
        .await
        .map_err(|_| {
            error!("S3 connection test timed out after 15s for bucket: {}", bucket);
            (
                StatusCode::GATEWAY_TIMEOUT,
                format!(
                    "S3 connection test timed out after 15 seconds. \
                    The endpoint '{}' did not respond in time. \
                    Check that the endpoint URL is correct and reachable.",
                    endpoint_trimmed
                ),
            )
        })?
        .map_err(|e| {
            let err_str = format!("{:?}", e);
            error!("S3 connection test (head_bucket) failed: {}", err_str);

            // Provide a user-friendly hint for common Cloudflare R2 issues
            let hint = if err_str.contains("InvalidRequest") || err_str.contains("AuthorizationHeaderMalformed") {
                " — Hint: For Cloudflare R2, set region to 'auto' and enable 'Force Path Style'."
            } else if err_str.contains("NoSuchBucket") {
                " — Hint: The bucket was not found. Check the bucket name."
            } else if err_str.contains("InvalidAccessKeyId") || err_str.contains("SignatureDoesNotMatch") {
                " — Hint: Invalid credentials. Check your Access Key ID and Secret Key."
            } else if err_str.contains("dispatch failure") || err_str.contains("ConnectorError") {
                " — Hint: Could not reach the endpoint. Check the Endpoint URL."
            } else {
                ""
            };

            (StatusCode::BAD_REQUEST, format!("S3 connection test failed: {}{}", err_str, hint))
        });

        head_bucket_result?;

        info!("S3 storage connection test succeeded.");
    }

    if test_only {
        return Ok(StatusCode::OK);
    }

    // Persist settings to DB
    save_db_setting(&state.pool, "storage_type", &payload.storage_type).await;
    
    if let Some(endpoint) = payload.s3_endpoint {
        save_db_setting(&state.pool, "s3_endpoint", &endpoint).await;
    }
    if let Some(bucket) = payload.s3_bucket {
        save_db_setting(&state.pool, "s3_bucket", &bucket).await;
    }
    if let Some(access_key) = payload.s3_access_key {
        save_db_setting(&state.pool, "s3_access_key", &access_key).await;
    }
    if let Some(secret_key) = payload.s3_secret_key {
        if secret_key != "********" {
            let encrypted_key = crate::auth::encrypt_subsonic_password(&secret_key, &state.config.jwt_secret)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to encrypt S3 secret key: {}", e)))?;
            save_db_setting(&state.pool, "s3_secret_key", &encrypted_key).await;
        }
    }
    if let Some(region) = payload.s3_region {
        save_db_setting(&state.pool, "s3_region", &region).await;
    }
    if let Some(force_path_style) = payload.s3_force_path_style {
        save_db_setting(&state.pool, "s3_force_path_style", if force_path_style { "true" } else { "false" }).await;
    }

    // Reload active backend client in state
    state.reload_storage_backend().await;
    info!("Storage settings updated and backend reloaded.");

    Ok(StatusCode::OK)
}
