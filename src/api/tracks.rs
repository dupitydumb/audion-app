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

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
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

pub async fn get_tracks(
    _claims: Claims,
    State(state): State<AppState>,
    Query(query): Query<PaginatedQuery>,
) -> Result<Json<Vec<TrackResponse>>, (StatusCode, String)> {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(50).max(1);
    let offset = (page - 1) * limit;

    let tracks = sqlx::query_as::<_, TrackResponse>(
        "SELECT id, path, title, artist, album, track_number, disc_number, duration,
                album_id, format, bitrate, source_type, cover_url, external_id,
                local_src, track_cover_path, genre, metadata_json, date_added
         FROM tracks
         ORDER BY artist, album, disc_number, track_number, title
         LIMIT ? OFFSET ?"
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(tracks))
}

pub async fn get_track_by_id(
    _claims: Claims,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<TrackResponse>, (StatusCode, String)> {
    let track = sqlx::query_as::<_, TrackResponse>(
        "SELECT id, path, title, artist, album, track_number, disc_number, duration,
                album_id, format, bitrate, source_type, cover_url, external_id,
                local_src, track_cover_path, genre, metadata_json, date_added
         FROM tracks
         WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Track not found".to_string()))?;

    Ok(Json(track))
}

pub async fn upload_track(
    _claims: Claims,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<TrackResponse>), (StatusCode, String)> {
    let mut file_bytes = Vec::new();
    let mut original_filename = String::new();

    while let Some(field) = multipart.next_field().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))? {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            original_filename = field.file_name().unwrap_or("upload.mp3").to_string();
            file_bytes = field.bytes().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?.to_vec();
        }
    }

    if file_bytes.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Missing file payload".to_string()));
    }

    // Generate unique storage name
    let file_uuid = Uuid::new_v4().to_string();
    let ext = PathBuf::from(&original_filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("mp3")
        .to_string();
    let relative_path = format!("music/{}.{}", file_uuid, ext);
    let full_path = state.config.data_dir.join(&relative_path);

    // Ensure music directory exists
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    // Write file temporarily to run metadata extraction
    std::fs::write(&full_path, &file_bytes).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Extract tags
    let mut metadata = extract_metadata(&full_path.to_string_lossy())
        .ok_or_else(|| {
            let _ = std::fs::remove_file(&full_path);
            (StatusCode::UNSUPPORTED_MEDIA_TYPE, "Failed to extract metadata".to_string())
        })?;

    // If the extracted title matches the generated UUID, it fell back to the disk filename.
    // In this case, override the title using the original filename's stem.
    if let Some(ref title) = metadata.title {
        if title == &file_uuid {
            let orig_path = std::path::Path::new(&original_filename);
            metadata.title = orig_path.file_stem().map(|s| s.to_string_lossy().to_string());
        }
    }

    // Deduplicate by content hash
    if let Some(ref hash) = metadata.content_hash {
        let existing = sqlx::query("SELECT id FROM tracks WHERE content_hash = ?")
            .bind(hash)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| {
                let _ = std::fs::remove_file(&full_path);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            })?;

        if existing.is_some() {
            let _ = std::fs::remove_file(&full_path);
            return Err((StatusCode::CONFLICT, "Duplicate track detected".to_string()));
        }
    }

    // Get or Create Album
    let album_id = if let Some(album_name) = &metadata.album {
        let existing_album = sqlx::query("SELECT id FROM albums WHERE name = ?")
            .bind(album_name)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if let Some(alb) = existing_album {
            Some(alb.get::<i64, _>("id"))
        } else {
            let res = sqlx::query("INSERT INTO albums (name, artist) VALUES (?, ?)")
                .bind(album_name)
                .bind(&metadata.artist)
                .execute(&state.pool)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            Some(res.last_insert_rowid())
        }
    } else {
        None
    };

    // Insert track into DB
    let res = sqlx::query(
        "INSERT INTO tracks (
            path, title, artist, album, track_number, disc_number, duration,
            album_id, format, bitrate, source_type, cover_url, external_id,
            local_src, genre, metadata_json, size
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'server', ?, ?, ?, ?, ?, ?)"
    )
    .bind(&relative_path)
    .bind(&metadata.title)
    .bind(&metadata.artist)
    .bind(&metadata.album)
    .bind(metadata.track_number)
    .bind(metadata.disc_number)
    .bind(metadata.duration)
    .bind(album_id)
    .bind(&metadata.format)
    .bind(metadata.bitrate)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(&metadata.genre)
    .bind(&metadata.metadata_json)
    .bind(file_bytes.len() as i64)
    .execute(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let track_id = res.last_insert_rowid();

    // Ensure artwork directory exists
    let artwork_dir = state.config.artwork_dir();
    std::fs::create_dir_all(&artwork_dir).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Save track cover if present
    if let Some(cover_data) = &metadata.track_cover {
        let relative_cover = format!("artwork/track_{}.jpg", track_id);
        let cover_full = state.config.data_dir.join(&relative_cover);
        if std::fs::write(&cover_full, cover_data).is_ok() {
            sqlx::query("UPDATE tracks SET track_cover_path = ? WHERE id = ?")
                .bind(&relative_cover)
                .bind(track_id)
                .execute(&state.pool)
                .await
                .ok();
        }
    }

    // Save album art if present and album doesn't have art yet
    if let Some(alb_id) = album_id {
        if let Some(art_data) = &metadata.album_art {
            let has_art = sqlx::query("SELECT art_path FROM albums WHERE id = ? AND art_path IS NOT NULL")
                .bind(alb_id)
                .fetch_optional(&state.pool)
                .await
                .map(|o| o.is_some())
                .unwrap_or(false);

            if !has_art {
                let relative_art = format!("artwork/album_{}.jpg", alb_id);
                let art_full = state.config.data_dir.join(&relative_art);
                if std::fs::write(&art_full, art_data).is_ok() {
                    sqlx::query("UPDATE albums SET art_path = ? WHERE id = ?")
                        .bind(&relative_art)
                        .bind(alb_id)
                        .execute(&state.pool)
                        .await
                        .ok();
                }
            }
        }
    }

    // Retrieve full newly created track
    let track = sqlx::query_as::<_, TrackResponse>(
        "SELECT id, path, title, artist, album, track_number, disc_number, duration,
                album_id, format, bitrate, source_type, cover_url, external_id,
                local_src, track_cover_path, genre, metadata_json, date_added
         FROM tracks
         WHERE id = ?"
    )
    .bind(track_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Record Event in DB
    let payload = serde_json::to_value(&track).unwrap_or(serde_json::Value::Null);
    let payload_str = payload.to_string();
    let event_type = "track.added";

    let event_res = sqlx::query("INSERT INTO events (event_type, payload) VALUES (?, ?)")
        .bind(event_type)
        .bind(payload_str)
        .execute(&state.pool)
        .await;

    if let Ok(er) = event_res {
        let event_id = er.last_insert_rowid();
        let time_now = chrono::Utc::now().to_rfc3339();
        
        // Broadcast Event
        state.event_bus.broadcast(ServerEvent {
            id: event_id,
            event_type: event_type.to_string(),
            payload,
            created_at: time_now,
        });
    }

    info!("Successfully uploaded track: {}", track.title.as_deref().unwrap_or(""));
    Ok((StatusCode::CREATED, Json(track)))
}

pub async fn delete_track(
    _claims: Claims,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    let track = sqlx::query("SELECT path, track_cover_path FROM tracks WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Track not found".to_string()))?;

    let path_val = track.get::<String, _>("path");
    let cover_path_val = track.get::<Option<String>, _>("track_cover_path");

    // Delete files
    let full_track_path = state.config.data_dir.join(&path_val);
    if full_track_path.exists() {
        let _ = std::fs::remove_file(full_track_path);
    }
    if let Some(ref cover_path) = cover_path_val {
        let full_cover_path = state.config.data_dir.join(cover_path);
        if full_cover_path.exists() {
            let _ = std::fs::remove_file(full_cover_path);
        }
    }
    let transcoded_path = state.config.data_dir.join("transcoded").join(format!("{}.mp3", id));
    if transcoded_path.exists() {
        let _ = std::fs::remove_file(transcoded_path);
    }

    // Delete from DB
    sqlx::query("DELETE FROM tracks WHERE id = ?")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Record Event in DB
    let payload = serde_json::json!({ "id": id });
    let payload_str = payload.to_string();
    let event_type = "track.deleted";

    let event_res = sqlx::query("INSERT INTO events (event_type, payload) VALUES (?, ?)")
        .bind(event_type)
        .bind(payload_str)
        .execute(&state.pool)
        .await;

    if let Ok(er) = event_res {
        let event_id = er.last_insert_rowid();
        let time_now = chrono::Utc::now().to_rfc3339();

        // Broadcast Event
        state.event_bus.broadcast(ServerEvent {
            id: event_id,
            event_type: event_type.to_string(),
            payload,
            created_at: time_now,
        });
    }

    info!("Successfully deleted track id: {}", id);
    Ok(StatusCode::OK)
}

fn clean_metadata_string(s: &str) -> String {
    let s_lower = s.to_lowercase();
    if let Some(idx) = s_lower.find("(feat.") {
        s[..idx].trim().to_string()
    } else if let Some(idx) = s_lower.find("(feat ") {
        s[..idx].trim().to_string()
    } else if let Some(idx) = s_lower.find("(with ") {
        s[..idx].trim().to_string()
    } else {
        s.trim().to_string()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LyricsResponse {
    pub lyrics: Option<String>,
}

pub async fn get_track_lyrics(
    _claims: Claims,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<LyricsResponse>, (StatusCode, String)> {
    let row = sqlx::query("SELECT title, artist, duration, metadata_json FROM tracks WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Track not found".to_string()))?;

    let title: Option<String> = row.get("title");
    let artist: Option<String> = row.get("artist");
    let duration: Option<i32> = row.get("duration");
    let metadata_json: Option<String> = row.get("metadata_json");

    let mut lyrics = metadata_json.and_then(|json_str| {
        let val: serde_json::Value = serde_json::from_str(&json_str).ok()?;
        val.get("Lyrics").and_then(|v| v.as_str().map(|s| s.to_string()))
    });

    if lyrics.is_none() {
        if let (Some(artist), Some(title)) = (artist, title) {
            let clean_artist = clean_metadata_string(&artist);
            let clean_title = clean_metadata_string(&title);

            let client = reqwest::Client::new();
            let mut request = client.get("https://lrclib.net/api/get")
                .query(&[("artist", &clean_artist), ("track_name", &clean_title)]);

            if let Some(dur) = duration {
                request = request.query(&[("duration", &dur.to_string())]);
            }

            let response = request.timeout(std::time::Duration::from_secs(5))
                .send()
                .await;

            match response {
                Ok(resp) => {
                    if resp.status().is_success() {
                        #[derive(Deserialize)]
                        #[serde(rename_all = "camelCase")]
                        struct LrcResponse {
                            synced_lyrics: Option<String>,
                            plain_lyrics: Option<String>,
                        }

                        if let Ok(lrc_data) = resp.json::<LrcResponse>().await {
                            lyrics = lrc_data.synced_lyrics.or(lrc_data.plain_lyrics);
                        }
                    } else if resp.status() == reqwest::StatusCode::NOT_FOUND {
                        info!("No lyrics found on LRCLIB for: {} - {}", clean_artist, clean_title);
                    } else {
                        error!("LRCLIB API returned status: {}", resp.status());
                    }
                }
                Err(err) => {
                    error!("Failed to fetch lyrics from LRCLIB: {}", err);
                }
            }
        }
    }

    Ok(Json(LyricsResponse { lyrics }))
}

#[derive(Deserialize)]
pub struct UpdateMetadataRequest {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
}

pub async fn update_track_metadata(
    _claims: Claims,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateMetadataRequest>,
) -> Result<Json<TrackResponse>, (StatusCode, String)> {
    // Fetch current track details
    let current_track = sqlx::query("SELECT id, album_id, album, artist, path FROM tracks WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Track not found".to_string()))?;

    let old_album_id: Option<i64> = current_track.get("album_id");
    let old_album_name: Option<String> = current_track.get("album");
    let old_artist: Option<String> = current_track.get("artist");
    let relative_path: String = current_track.get("path");

    // Handle Album ID updates if album name changes
    let mut new_album_id = old_album_id;
    if let Some(ref new_album_name) = payload.album {
        let name_changed = Some(new_album_name) != old_album_name.as_ref();
        if name_changed {
            if new_album_name.trim().is_empty() {
                new_album_id = None;
            } else {
                let existing = sqlx::query("SELECT id FROM albums WHERE name = ?")
                    .bind(new_album_name)
                    .fetch_optional(&state.pool)
                    .await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

                if let Some(alb) = existing {
                    new_album_id = Some(alb.get::<i64, _>("id"));
                } else {
                    let res = sqlx::query("INSERT INTO albums (name, artist) VALUES (?, ?)")
                        .bind(new_album_name)
                        .bind(&payload.artist.as_ref().or(old_artist.as_ref()))
                        .execute(&state.pool)
                        .await
                        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                    new_album_id = Some(res.last_insert_rowid());
                }
            }
        }
    }

    // Update database row
    sqlx::query(
        "UPDATE tracks
         SET title = ?, artist = ?, album = ?, album_id = ?, genre = ?, track_number = ?, disc_number = ?
         WHERE id = ?"
    )
    .bind(&payload.title)
    .bind(&payload.artist)
    .bind(&payload.album)
    .bind(new_album_id)
    .bind(&payload.genre)
    .bind(payload.track_number)
    .bind(payload.disc_number)
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Clean up old empty albums
    if let Some(o_alb_id) = old_album_id {
        if Some(o_alb_id) != new_album_id {
            let tracks_left: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE album_id = ?")
                .bind(o_alb_id)
                .fetch_one(&state.pool)
                .await
                .unwrap_or(0);

            if tracks_left == 0 {
                let art_path_opt: Option<String> = sqlx::query_scalar("SELECT art_path FROM albums WHERE id = ?")
                    .bind(o_alb_id)
                    .fetch_one(&state.pool)
                    .await
                    .unwrap_or(None);

                if let Some(art_path) = art_path_opt {
                    let full_art_path = state.config.data_dir.join(art_path);
                    if full_art_path.exists() {
                        let _ = std::fs::remove_file(full_art_path);
                    }
                }

                sqlx::query("DELETE FROM albums WHERE id = ?")
                    .bind(o_alb_id)
                    .execute(&state.pool)
                    .await
                    .ok();
            }
        }
    }

    // Try writing tags to physical file using lofty
    let full_path = state.config.data_dir.join(&relative_path);
    let payload_title = payload.title.clone();
    let payload_artist = payload.artist.clone();
    let payload_album = payload.album.clone();
    let payload_genre = payload.genre.clone();
    let payload_track_number = payload.track_number;
    let payload_disc_number = payload.disc_number;

    tokio::task::spawn_blocking(move || {
        if full_path.exists() {
            use lofty::prelude::*;
            if let Ok(mut tagged_file) = lofty::probe::Probe::open(&full_path)
                .and_then(|p| p.options(lofty::config::ParseOptions::new().parsing_mode(lofty::config::ParsingMode::Relaxed)).read()) 
            {
                let tag = if tagged_file.primary_tag().is_some() {
                    tagged_file.primary_tag_mut()
                } else {
                    tagged_file.first_tag_mut()
                };
                if let Some(tag) = tag {
                    if let Some(t) = payload_title {
                        tag.insert_text(lofty::tag::ItemKey::TrackTitle, t);
                    }
                    if let Some(a) = payload_artist {
                        tag.insert_text(lofty::tag::ItemKey::TrackArtist, a);
                    }
                    if let Some(al) = payload_album {
                        tag.insert_text(lofty::tag::ItemKey::AlbumTitle, al);
                    }
                    if let Some(g) = payload_genre {
                        tag.insert_text(lofty::tag::ItemKey::Genre, g);
                    }
                    if let Some(tr) = payload_track_number {
                        tag.insert_text(lofty::tag::ItemKey::TrackNumber, tr.to_string());
                    }
                    if let Some(ds) = payload_disc_number {
                        tag.insert_text(lofty::tag::ItemKey::DiscNumber, ds.to_string());
                    }
                    let _ = tag.save_to_path(&full_path, lofty::config::WriteOptions::default());
                }
            }
        }
    });

    // Fetch the updated track
    let track = sqlx::query_as::<_, TrackResponse>(
        "SELECT id, path, title, artist, album, track_number, disc_number, duration,
                album_id, format, bitrate, source_type, cover_url, external_id,
                local_src, track_cover_path, genre, metadata_json, date_added
         FROM tracks
         WHERE id = ?"
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Broadcast track.updated
    let payload_val = serde_json::to_value(&track).unwrap_or(serde_json::Value::Null);
    let payload_str = payload_val.to_string();
    let event_type = "track.updated";

    if let Ok(er) = sqlx::query("INSERT INTO events (event_type, payload) VALUES (?, ?)")
        .bind(event_type)
        .bind(&payload_str)
        .execute(&state.pool)
        .await
    {
        let event_id = er.last_insert_rowid();
        state.event_bus.broadcast(ServerEvent {
            id: event_id,
            event_type: event_type.to_string(),
            payload: payload_val,
            created_at: chrono::Utc::now().to_rfc3339(),
        });
    }

    Ok(Json(track))
}


