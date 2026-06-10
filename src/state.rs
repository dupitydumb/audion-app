use sqlx::SqlitePool;
use std::sync::{Arc, Mutex};
use serde::Serialize;
use crate::config::Config;
use crate::events::EventBus;
use crate::tunnel::TunnelManager;

#[derive(Debug, Clone, Serialize)]
pub struct ScanStatus {
    pub is_scanning: bool,
    pub files_scanned: usize,
    pub total_files: usize,
    pub current_file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FetcherStatus {
    pub is_running: bool,
    pub tracks_processed: usize,
    pub total_tracks: usize,
    pub current_track: Option<String>,
    pub logs: Vec<String>,
}

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Config,
    pub event_bus: EventBus,
    pub has_ffmpeg: bool,
    pub scan_status: Arc<Mutex<ScanStatus>>,
    pub fetcher_status: Arc<Mutex<FetcherStatus>>,
    pub tunnel_manager: Arc<tokio::sync::Mutex<TunnelManager>>,
    pub user_pools: Arc<tokio::sync::RwLock<std::collections::HashMap<String, SqlitePool>>>,
    pub scan_statuses: Arc<tokio::sync::RwLock<std::collections::HashMap<String, Arc<Mutex<ScanStatus>>>>>,
    pub fetcher_statuses: Arc<tokio::sync::RwLock<std::collections::HashMap<String, Arc<Mutex<FetcherStatus>>>>>,
    pub storage_backend: Arc<tokio::sync::RwLock<crate::storage::StorageBackend>>,
}

impl AppState {
    pub async fn new(pool: SqlitePool, config: Config, event_bus: EventBus) -> Self {
        let has_ffmpeg = std::process::Command::new("ffmpeg")
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if has_ffmpeg {
            tracing::info!("FFmpeg is available. On-the-fly transcoding enabled for FLAC files.");
        } else {
            tracing::warn!("FFmpeg is NOT available. FLAC files will stream untranscoded (might fail on some browsers).");
        }

        let scan_status = Arc::new(Mutex::new(ScanStatus {
            is_scanning: false,
            files_scanned: 0,
            total_files: 0,
            current_file: None,
        }));

        let fetcher_status = Arc::new(Mutex::new(FetcherStatus {
            is_running: false,
            tracks_processed: 0,
            total_tracks: 0,
            current_track: None,
            logs: Vec::new(),
        }));

        let tunnel_manager = Arc::new(tokio::sync::Mutex::new(TunnelManager::new(pool.clone(), config.port).await));

        let storage_backend = Arc::new(tokio::sync::RwLock::new(
            Self::load_storage_backend_from_db(&pool, &config.data_dir).await
        ));

        Self {
            pool,
            config,
            event_bus,
            has_ffmpeg,
            scan_status,
            fetcher_status,
            tunnel_manager,
            user_pools: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            scan_statuses: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            fetcher_statuses: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            storage_backend,
        }
    }

    pub async fn load_storage_backend_from_db(pool: &SqlitePool, data_dir: &std::path::Path) -> crate::storage::StorageBackend {
        let storage_type = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'storage_type'")
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "local".to_string());

        if storage_type == "s3" {
            let endpoint = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 's3_endpoint'")
                .fetch_optional(pool).await.ok().flatten().unwrap_or_default();
            let bucket = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 's3_bucket'")
                .fetch_optional(pool).await.ok().flatten().unwrap_or_default();
            let access_key = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 's3_access_key'")
                .fetch_optional(pool).await.ok().flatten().unwrap_or_default();
            let secret_key = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 's3_secret_key'")
                .fetch_optional(pool).await.ok().flatten().unwrap_or_default();
            let region = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 's3_region'")
                .fetch_optional(pool).await.ok().flatten().unwrap_or_default();
            let force_path_style = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 's3_force_path_style'")
                .fetch_optional(pool).await.ok().flatten()
                .map(|v| v == "true")
                .unwrap_or(false);

            match crate::storage::build_s3_client(
                &endpoint,
                &bucket,
                &access_key,
                &secret_key,
                &region,
                force_path_style,
            ) {
                Ok(client) => crate::storage::StorageBackend::S3 {
                    client,
                    bucket,
                    endpoint_url: endpoint,
                },
                Err(e) => {
                    tracing::error!("Failed to build S3 storage backend: {}. Falling back to local.", e);
                    crate::storage::StorageBackend::Local {
                        data_dir: data_dir.to_path_buf(),
                    }
                }
            }
        } else {
            crate::storage::StorageBackend::Local {
                data_dir: data_dir.to_path_buf(),
            }
        }
    }

    pub async fn reload_storage_backend(&self) {
        let new_backend = Self::load_storage_backend_from_db(&self.pool, &self.config.data_dir).await;
        let mut w = self.storage_backend.write().await;
        *w = new_backend;
    }


    pub async fn get_user_pool(&self, user_id: &str) -> Result<SqlitePool, sqlx::Error> {
        {
            let r = self.user_pools.read().await;
            if let Some(pool) = r.get(user_id) {
                return Ok(pool.clone());
            }
        }
        let mut w = self.user_pools.write().await;
        if let Some(pool) = w.get(user_id) {
            return Ok(pool.clone());
        }
        let db_path = self.config.user_db_path(user_id);
        let pool = crate::db::setup_database(&db_path).await?;
        
        // Sync user to local database for foreign key constraints
        use sqlx::Row;
        if let Ok(Some(row)) = sqlx::query("SELECT username, password_hash, role, is_enabled FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
        {
            let username: String = row.get("username");
            let password_hash: String = row.get("password_hash");
            let role: String = row.get("role");
            let is_enabled: i32 = row.get("is_enabled");
            
            let _ = sqlx::query("INSERT OR REPLACE INTO users (id, username, password_hash, role, is_enabled) VALUES (?, ?, ?, ?, ?)")
                .bind(user_id)
                .bind(username)
                .bind(password_hash)
                .bind(role)
                .bind(is_enabled)
                .execute(&pool)
                .await;
        }

        w.insert(user_id.to_string(), pool.clone());
        Ok(pool)
    }

    pub async fn get_user_scan_status(&self, user_id: &str) -> Arc<Mutex<ScanStatus>> {
        {
            let r = self.scan_statuses.read().await;
            if let Some(status) = r.get(user_id) {
                return status.clone();
            }
        }
        let mut w = self.scan_statuses.write().await;
        if let Some(status) = w.get(user_id) {
            return status.clone();
        }
        let status = Arc::new(Mutex::new(ScanStatus {
            is_scanning: false,
            files_scanned: 0,
            total_files: 0,
            current_file: None,
        }));
        w.insert(user_id.to_string(), status.clone());
        status
    }

    pub async fn get_user_fetcher_status(&self, user_id: &str) -> Arc<Mutex<FetcherStatus>> {
        {
            let r = self.fetcher_statuses.read().await;
            if let Some(status) = r.get(user_id) {
                return status.clone();
            }
        }
        let mut w = self.fetcher_statuses.write().await;
        if let Some(status) = w.get(user_id) {
            return status.clone();
        }
        let status = Arc::new(Mutex::new(FetcherStatus {
            is_running: false,
            tracks_processed: 0,
            total_tracks: 0,
            current_track: None,
            logs: Vec::new(),
        }));
        w.insert(user_id.to_string(), status.clone());
        status
    }

    pub async fn find_artwork_path(&self, album_id: i64) -> Option<String> {
        use sqlx::Row;
        {
            let pools = self.user_pools.read().await;
            for pool in pools.values() {
                let res = sqlx::query("SELECT art_path FROM albums WHERE id = ?")
                    .bind(album_id)
                    .fetch_optional(pool)
                    .await
                    .ok()
                    .flatten();
                if let Some(row) = res {
                    if let Some(art_path) = row.get::<Option<String>, _>("art_path") {
                        return Some(art_path);
                    }
                }
            }
        }
        if let Ok(users) = sqlx::query("SELECT id FROM users")
            .fetch_all(&self.pool)
            .await 
        {
            for u in users {
                let user_id: String = u.get("id");
                if let Ok(pool) = self.get_user_pool(&user_id).await {
                    let res = sqlx::query("SELECT art_path FROM albums WHERE id = ?")
                        .bind(album_id)
                        .fetch_optional(&pool)
                        .await
                        .ok()
                        .flatten();
                    if let Some(row) = res {
                        if let Some(art_path) = row.get::<Option<String>, _>("art_path") {
                            return Some(art_path);
                        }
                    }
                }
            }
        }
        None
    }

    pub async fn find_track_cover_path(&self, track_id: i64) -> Option<String> {
        use sqlx::Row;
        {
            let pools = self.user_pools.read().await;
            for pool in pools.values() {
                let res = sqlx::query("SELECT track_cover_path FROM tracks WHERE id = ?")
                    .bind(track_id)
                    .fetch_optional(pool)
                    .await
                    .ok()
                    .flatten();
                if let Some(row) = res {
                    if let Some(cover_path) = row.get::<Option<String>, _>("track_cover_path") {
                        return Some(cover_path);
                    }
                }
            }
        }
        if let Ok(users) = sqlx::query("SELECT id FROM users").fetch_all(&self.pool).await {
            for u in users {
                let user_id: String = u.get("id");
                if let Ok(pool) = self.get_user_pool(&user_id).await {
                    let res = sqlx::query("SELECT track_cover_path FROM tracks WHERE id = ?")
                        .bind(track_id)
                        .fetch_optional(&pool)
                        .await
                        .ok()
                        .flatten();
                    if let Some(row) = res {
                        if let Some(cover_path) = row.get::<Option<String>, _>("track_cover_path") {
                            return Some(cover_path);
                        }
                    }
                }
            }
        }
        None
    }

    pub async fn find_artwork_path_for_user(&self, user_id: &str, album_id: i64) -> Option<String> {
        use sqlx::Row;
        if let Ok(pool) = self.get_user_pool(user_id).await {
            let res = sqlx::query("SELECT art_path FROM albums WHERE id = ?")
                .bind(album_id)
                .fetch_optional(&pool)
                .await
                .ok()
                .flatten();
            if let Some(row) = res {
                if let Some(art_path) = row.get::<Option<String>, _>("art_path") {
                    return Some(art_path);
                }
            }
        }
        None
    }

    pub async fn find_track_cover_path_for_user(&self, user_id: &str, track_id: i64) -> Option<String> {
        use sqlx::Row;
        if let Ok(pool) = self.get_user_pool(user_id).await {
            let res = sqlx::query("SELECT track_cover_path FROM tracks WHERE id = ?")
                .bind(track_id)
                .fetch_optional(&pool)
                .await
                .ok()
                .flatten();
            if let Some(row) = res {
                if let Some(cover_path) = row.get::<Option<String>, _>("track_cover_path") {
                    return Some(cover_path);
                }
            }
        }
        None
    }
}

