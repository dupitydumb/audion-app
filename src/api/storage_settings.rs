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
    pub azure_connection_string: String, // Masked as "********" if exists
    pub azure_container: String,
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
    pub azure_connection_string: Option<String>,
    pub azure_container: Option<String>,
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
    let azure_container = get_db_setting(&state.pool, "azure_container").await;
    let azure_conn_raw = get_db_setting(&state.pool, "azure_connection_string").await;
    let azure_connection_string = if azure_conn_raw.is_empty() { "" } else { "********" }.to_string();

    Ok(Json(StorageSettingsResponse {
        storage_type,
        s3_endpoint,
        s3_bucket,
        s3_access_key,
        s3_secret_key,
        s3_region,
        s3_force_path_style,
        azure_connection_string,
        azure_container,
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

    // If storage type is s3 or gcs, perform test connection
    if payload.storage_type == "s3" || payload.storage_type == "gcs" {
        let endpoint = payload.s3_endpoint.clone().unwrap_or_default();
        let bucket = payload.s3_bucket.clone().unwrap_or_default();
        let access_key = payload.s3_access_key.clone().unwrap_or_default();
        let region = payload.s3_region.clone().unwrap_or_default();
        let force_path_style = payload.s3_force_path_style.unwrap_or(false);

        // Retrieve existing secret key from DB if the payload contains the mask "********"
        let is_new_key = match payload.s3_secret_key.as_deref() {
            Some("********") | Some("") | None => false,
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

        // Phase 1: Fast TCP/TLS connectivity check using reqwest.
        // This gives a clean error within ~10s if the endpoint is unreachable,
        // before we attempt the full AWS SDK auth handshake.
        // For Cloudflare R2, the endpoint returns 400/403 for unauthenticated requests,
        // which is fine — we just want to confirm the host is reachable.
        if !endpoint_trimmed.is_empty() {
            let probe_url = format!("{}/", endpoint_trimmed.trim_end_matches('/'));
            let http_client = reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .danger_accept_invalid_certs(false)
                .build()
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to build HTTP client: {}", e)))?;

            match http_client.get(&probe_url).send().await {
                Ok(_) => {
                    info!("S3 endpoint connectivity check passed: {}", endpoint_trimmed);
                }
                Err(e) if e.is_connect() || e.is_timeout() => {
                    error!("S3 endpoint connectivity check failed: {}", e);
                    return Err((
                        StatusCode::BAD_REQUEST,
                        format!(
                            "Cannot reach S3 endpoint '{}': {} — Check the Endpoint URL is correct and accessible from the server.",
                            endpoint_trimmed, e
                        ),
                    ));
                }
                Err(_) => {
                    // Non-connect errors (e.g. 4xx/5xx HTTP) are fine — endpoint is reachable
                    info!("S3 endpoint connectivity check: endpoint reachable (got HTTP error response, expected)");
                }
            }
        }

        // Phase 2: Full SDK auth check using head_bucket.
        // Build temporary S3 client to test credentials and bucket access.
        // No-retry client: avoids SDK retry loops that cause 502 from the reverse proxy.
        let s3_client = crate::storage::build_s3_client_no_retry(
            endpoint_trimmed,
            &bucket,
            &access_key,
            &secret_key,
            &region,
            force_path_style,
        ).map_err(|e| (StatusCode::BAD_REQUEST, format!("Failed to create S3 client configuration: {}", e)))?;

        // A 15-second timeout prevents the reverse proxy from issuing a 502 if the
        // SDK hangs unexpectedly. SDK-level timeouts (connect: 10s, read: 12s) are
        // also configured inside build_s3_client for defense-in-depth.
        let head_bucket_result = tokio::time::timeout(
            Duration::from_secs(15),
            s3_client.head_bucket().bucket(&bucket).send(),
        )
        .await
        .map_err(|_| {
            error!("S3 auth test timed out after 15s for bucket: {}", bucket);
            (
                StatusCode::GATEWAY_TIMEOUT,
                format!(
                    "S3 auth test timed out after 15 seconds for bucket '{}'. \
                    The endpoint is reachable but did not respond to authentication. \
                    Check your credentials and bucket name.",
                    bucket
                ),
            )
        })?
        .map_err(|e| {
            let err_str = format!("{:?}", e);
            error!("S3 connection test (head_bucket) failed: {}", err_str);

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
    } else if payload.storage_type == "azure" {
        let container = payload.azure_container.clone().unwrap_or_default();
        let is_new_conn = match payload.azure_connection_string.as_deref() {
            Some("********") | Some("") | None => false,
            _ => true,
        };

        let conn_str = if is_new_conn {
            payload.azure_connection_string.clone().unwrap_or_default()
        } else {
            let db_val = get_db_setting(&state.pool, "azure_connection_string").await;
            if db_val.is_empty() {
                "".to_string()
            } else {
                match crate::auth::decrypt_subsonic_password(&db_val, &state.config.jwt_secret) {
                    Ok(decrypted) => decrypted,
                    Err(_) => db_val,
                }
            }
        };

        if container.trim().is_empty() {
            return Err((StatusCode::BAD_REQUEST, "Container name cannot be empty".to_string()));
        }

        info!("Testing Azure storage connection to container: {}", container);

        let azure_client = crate::storage::build_azure_client(&conn_str)
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("Failed to create Azure client: {}", e)))?;

        let container_client = azure_client.container_client(&container);

        let exists = tokio::time::timeout(
            Duration::from_secs(15),
            container_client.exists(),
        )
        .await
        .map_err(|_| {
            error!("Azure connection test timed out for container: {}", container);
            (
                StatusCode::GATEWAY_TIMEOUT,
                format!("Azure connection test timed out for container '{}'.", container),
            )
        })?
        .map_err(|e| {
            error!("Azure connection test failed: {:?}", e);
            (StatusCode::BAD_REQUEST, format!("Azure connection test failed: {:?}", e))
        })?;

        if !exists {
            return Err((StatusCode::BAD_REQUEST, format!("Container '{}' does not exist.", container)));
        }

        info!("Azure storage connection test succeeded.");
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
        // Skip saving if the value is the mask or empty — preserves the stored encrypted key.
        if secret_key != "********" && !secret_key.is_empty() {
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
    if let Some(azure_container) = payload.azure_container {
        save_db_setting(&state.pool, "azure_container", &azure_container).await;
    }
    if let Some(azure_conn) = payload.azure_connection_string {
        if azure_conn != "********" && !azure_conn.is_empty() {
            let encrypted = crate::auth::encrypt_subsonic_password(&azure_conn, &state.config.jwt_secret)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to encrypt Azure connection string: {}", e)))?;
            save_db_setting(&state.pool, "azure_connection_string", &encrypted).await;
        }
    }

    // Reload active backend client in state
    state.reload_storage_backend().await;
    info!("Storage settings updated and backend reloaded.");

    Ok(StatusCode::OK)
}
