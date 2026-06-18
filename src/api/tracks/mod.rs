use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::path::PathBuf;
use tracing::{info, error};
use sqlx::Row;

use crate::state::AppState;
use crate::auth::Claims;
use crate::scanner::extract_metadata;
use crate::events::ServerEvent;

pub mod upload;
pub mod metadata;
pub mod query;
pub mod lyrics;

pub use upload::upload_track;
pub use metadata::{update_track_metadata, fetch_track_metadata, bulk_fetch_metadata, UpdateMetadataRequest, bulk_delete_tracks};
pub use query::{get_tracks, get_track_by_id, delete_track, delete_track_inner};
pub use lyrics::get_track_lyrics;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct TrackResponse {
    pub id: i64,
    pub path: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub duration: Option<i32>,
    pub album_id: Option<i64>,
    pub format: Option<String>,
    pub bitrate: Option<i32>,
    pub source_type: Option<String>,
    pub cover_url: Option<String>,
    pub external_id: Option<String>,
    pub local_src: Option<String>,
    pub track_cover_path: Option<String>,
    pub genre: Option<String>,
    pub metadata_json: Option<String>,
    pub date_added: Option<String>,
}

#[derive(Deserialize)]
pub struct PaginatedQuery {
    pub page: Option<u32>,
    pub limit: Option<u32>,
}
