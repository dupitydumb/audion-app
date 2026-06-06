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

        Self {
            pool,
            config,
            event_bus,
            has_ffmpeg,
            scan_status,
            fetcher_status,
            tunnel_manager,
        }
    }
}

