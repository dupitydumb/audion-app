use sqlx::SqlitePool;
use crate::config::Config;
use crate::events::EventBus;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Config,
    pub event_bus: EventBus,
    pub has_ffmpeg: bool,
}

impl AppState {
    pub fn new(pool: SqlitePool, config: Config, event_bus: EventBus) -> Self {
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

        Self {
            pool,
            config,
            event_bus,
            has_ffmpeg,
        }
    }
}
