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

    let user_pool = state.get_user_pool(&_claims.sub).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

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
    .fetch_all(&user_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(tracks))
}

pub async fn get_track_by_id(
    _claims: Claims,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<TrackResponse>, (StatusCode, String)> {
    let user_pool = state.get_user_pool(&_claims.sub).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let track = sqlx::query_as::<_, TrackResponse>(
        "SELECT id, path, title, artist, album, track_number, disc_number, duration,
                album_id, format, bitrate, source_type, cover_url, external_id,
                local_src, track_cover_path, genre, metadata_json, date_added
         FROM tracks
         WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(&user_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Track not found".to_string()))?;

    Ok(Json(track))
}

pub async fn upload_track(
    claims: Claims,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<TrackResponse>), (StatusCode, String)> {
    claims.require_non_stream_only().map_err(|(s, m)| (s, m.to_string()))?;
    let user_pool = state.get_user_pool(&claims.sub).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
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
    let relative_path = format!("users/{}/music/{}.{}", claims.sub, file_uuid, ext);
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
            .fetch_optional(&user_pool)
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

    // If S3 is active, upload track and delete local temp file
    let upload_to_s3 = {
        let storage = state.storage_backend.read().await;
        if let crate::storage::StorageBackend::S3 { .. } = &*storage {
            let content_type = match ext.to_lowercase().as_str() {
                "mp3" => "audio/mpeg",
                "flac" => "audio/flac",
                "m4a" | "aac" | "alac" => "audio/mp4",
                "ogg" => "audio/ogg",
                "wav" => "audio/wav",
                _ => "application/octet-stream",
            };
            storage.put_object(&relative_path, file_bytes.clone(), content_type).await
                .map_err(|e| {
                    let _ = std::fs::remove_file(&full_path);
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to upload track to S3: {}", e))
                })?;
            true
        } else {
            false
        }
    };
    if upload_to_s3 {
        let _ = std::fs::remove_file(&full_path);
    }

    // Get or Create Album
    let album_id = if let Some(album_name) = &metadata.album {
        let existing_album = sqlx::query("SELECT id FROM albums WHERE name = ?")
            .bind(album_name)
            .fetch_optional(&user_pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if let Some(alb) = existing_album {
            Some(alb.get::<i64, _>("id"))
        } else {
            let res = sqlx::query("INSERT INTO albums (name, artist) VALUES (?, ?)")
                .bind(album_name)
                .bind(&metadata.artist)
                .execute(&user_pool)
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
    .execute(&user_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let track_id = res.last_insert_rowid();

    // Ensure artwork directory exists
    let artwork_dir = state.config.user_artwork_dir(&claims.sub);
    std::fs::create_dir_all(&artwork_dir).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Save track cover if present
    if let Some(cover_data) = &metadata.track_cover {
        let relative_cover = format!("users/{}/artwork/track_{}.jpg", claims.sub, track_id);
        let write_success = {
            let storage = state.storage_backend.read().await;
            match &*storage {
                crate::storage::StorageBackend::Local { .. } => {
                    let cover_full = state.config.data_dir.join(&relative_cover);
                    std::fs::write(&cover_full, cover_data).is_ok()
                }
                crate::storage::StorageBackend::S3 { .. } => {
                    storage.put_object(&relative_cover, cover_data.clone(), "image/jpeg").await.is_ok()
                }
            }
        };
        if write_success {
            sqlx::query("UPDATE tracks SET track_cover_path = ? WHERE id = ?")
                .bind(&relative_cover)
                .bind(track_id)
                .execute(&user_pool)
                .await
                .ok();
        }
    }

    // Save album art if present and album doesn't have art yet
    if let Some(alb_id) = album_id {
        if let Some(art_data) = &metadata.album_art {
            let has_art = sqlx::query("SELECT art_path FROM albums WHERE id = ? AND art_path IS NOT NULL")
                .bind(alb_id)
                .fetch_optional(&user_pool)
                .await
                .map(|o| o.is_some())
                .unwrap_or(false);

            if !has_art {
                let relative_art = format!("users/{}/artwork/album_{}.jpg", claims.sub, alb_id);
                let write_success = {
                    let storage = state.storage_backend.read().await;
                    match &*storage {
                        crate::storage::StorageBackend::Local { .. } => {
                            let art_full = state.config.data_dir.join(&relative_art);
                            std::fs::write(&art_full, art_data).is_ok()
                        }
                        crate::storage::StorageBackend::S3 { .. } => {
                            storage.put_object(&relative_art, art_data.clone(), "image/jpeg").await.is_ok()
                        }
                    }
                };
                if write_success {
                    sqlx::query("UPDATE albums SET art_path = ? WHERE id = ?")
                        .bind(&relative_art)
                        .bind(alb_id)
                        .execute(&user_pool)
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
    .fetch_one(&user_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Record Event in DB
    let payload = serde_json::to_value(&track).unwrap_or(serde_json::Value::Null);
    let payload_str = payload.to_string();
    let event_type = "track.added";

    let event_res = sqlx::query("INSERT INTO events (event_type, payload) VALUES (?, ?)")
        .bind(event_type)
        .bind(payload_str)
        .execute(&user_pool)
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

pub async fn delete_track_inner(
    state: &AppState,
    pool: &sqlx::SqlitePool,
    id: i64,
) -> Result<(), (StatusCode, String)> {
    let track = sqlx::query("SELECT path, track_cover_path FROM tracks WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Track not found".to_string()))?;

    let path_val = track.get::<String, _>("path");
    let cover_path_val = track.get::<Option<String>, _>("track_cover_path");

    // Delete files
    {
        let storage = state.storage_backend.read().await;
        match &*storage {
            crate::storage::StorageBackend::Local { .. } => {
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
            }
            crate::storage::StorageBackend::S3 { .. } => {
                let _ = storage.delete_object(&path_val).await;
                if let Some(ref cover_path) = cover_path_val {
                    let _ = storage.delete_object(cover_path).await;
                }
            }
        }
    }
    let transcoded_path = state.config.data_dir.join("transcoded").join(format!("{}.mp3", id));
    if transcoded_path.exists() {
        let _ = std::fs::remove_file(transcoded_path);
    }

    // Delete from DB
    sqlx::query("DELETE FROM tracks WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Record Event in DB
    let payload = serde_json::json!({ "id": id });
    let payload_str = payload.to_string();
    let event_type = "track.deleted";

    let event_res = sqlx::query("INSERT INTO events (event_type, payload) VALUES (?, ?)")
        .bind(event_type)
        .bind(payload_str)
        .execute(pool)
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
    Ok(())
}

pub async fn delete_track(
    claims: Claims,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    claims.require_non_stream_only().map_err(|(s, m)| (s, m.to_string()))?;
    let user_pool = state.get_user_pool(&claims.sub).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    delete_track_inner(&state, &user_pool, id).await?;
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
    let user_pool = state.get_user_pool(&_claims.sub).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let row = sqlx::query("SELECT title, artist, album, duration, metadata_json FROM tracks WHERE id = ?")
        .bind(id)
        .fetch_optional(&user_pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Track not found".to_string()))?;

    let title: Option<String> = row.get("title");
    let artist: Option<String> = row.get("artist");
    let album: Option<String> = row.get("album");
    let duration: Option<i32> = row.get("duration");
    let metadata_json: Option<String> = row.get("metadata_json");

    let mut lyrics = metadata_json.as_ref().and_then(|json_str| {
        let val: serde_json::Value = serde_json::from_str(json_str).ok()?;
        val.get("Lyrics").and_then(|v| v.as_str().map(|s| s.to_string()))
    });

    if lyrics.is_none() {
        if let (Some(artist), Some(title)) = (artist, title) {
            let clean_artist = clean_metadata_string(&artist);
            let clean_title = clean_metadata_string(&title);

            let client = reqwest::Client::builder()
                .user_agent("Audion/0.1.0 (contact@audion.local)")
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());

            let mut query_params = vec![
                ("artist_name".to_string(), clean_artist.clone()),
                ("track_name".to_string(), clean_title.clone()),
            ];

            if let Some(ref alb) = album {
                if !alb.trim().is_empty() {
                    query_params.push(("album_name".to_string(), clean_metadata_string(alb)));
                }
            }

            if let Some(dur) = duration {
                query_params.push(("duration".to_string(), dur.to_string()));
            }

            let response = client.get("https://lrclib.net/api/get")
                .query(&query_params)
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await;

            match response {
                Ok(resp) => {
                    #[derive(Deserialize)]
                    #[serde(rename_all = "camelCase")]
                    struct LrcResponse {
                        synced_lyrics: Option<String>,
                        plain_lyrics: Option<String>,
                    }

                    if resp.status().is_success() {
                        if let Ok(lrc_data) = resp.json::<LrcResponse>().await {
                            lyrics = lrc_data.synced_lyrics.or(lrc_data.plain_lyrics);
                        }
                    } else if resp.status() == reqwest::StatusCode::NOT_FOUND {
                        info!("No exact match on LRCLIB for: {} - {}, falling back to search", clean_artist, clean_title);
                        
                        let search_req = client.get("https://lrclib.net/api/search")
                            .query(&[("q", &format!("{} {}", clean_title, clean_artist))])
                            .timeout(std::time::Duration::from_secs(5))
                            .send()
                            .await;

                        if let Ok(search_resp) = search_req {
                            if search_resp.status().is_success() {
                                if let Ok(results) = search_resp.json::<Vec<LrcResponse>>().await {
                                    if let Some(best) = results.into_iter().find(|r| r.synced_lyrics.is_some() || r.plain_lyrics.is_some()) {
                                        lyrics = best.synced_lyrics.or(best.plain_lyrics);
                                    }
                                }
                            }
                        }
                    } else {
                        error!("LRCLIB API returned status: {}", resp.status());
                    }
                }
                Err(err) => {
                    error!("Failed to fetch lyrics from LRCLIB: {}", err);
                }
            }

            // Cache successfully fetched lyrics in the DB
            if let Some(ref lrc_text) = lyrics {
                let mut map = metadata_json
                    .and_then(|s| serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&s).ok())
                    .unwrap_or_default();

                if !map.contains_key("Lyrics") {
                    map.insert("Lyrics".to_string(), serde_json::Value::String(lrc_text.clone()));
                    if let Ok(updated_json) = serde_json::to_string(&map) {
                        let _ = sqlx::query("UPDATE tracks SET metadata_json = ? WHERE id = ?")
                            .bind(updated_json)
                            .bind(id)
                            .execute(&user_pool)
                            .await;
                    }
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
    claims: Claims,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateMetadataRequest>,
) -> Result<Json<TrackResponse>, (StatusCode, String)> {
    claims.require_non_stream_only().map_err(|(s, m)| (s, m.to_string()))?;
    let user_pool = state.get_user_pool(&claims.sub).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Fetch current track details
    let current_track = sqlx::query("SELECT id, album_id, album, artist, path, metadata_json FROM tracks WHERE id = ?")
        .bind(id)
        .fetch_optional(&user_pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Track not found".to_string()))?;

    let old_album_id: Option<i64> = current_track.get("album_id");
    let old_album_name: Option<String> = current_track.get("album");
    let old_artist: Option<String> = current_track.get("artist");
    let relative_path: String = current_track.get("path");
    let old_metadata_json: Option<String> = current_track.get("metadata_json");

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
                    .fetch_optional(&user_pool)
                    .await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

                if let Some(alb) = existing {
                    new_album_id = Some(alb.get::<i64, _>("id"));
                } else {
                    let res = sqlx::query("INSERT INTO albums (name, artist) VALUES (?, ?)")
                        .bind(new_album_name)
                        .bind(&payload.artist.as_ref().or(old_artist.as_ref()))
                        .execute(&user_pool)
                        .await
                        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                    new_album_id = Some(res.last_insert_rowid());
                }
            }
        }
    }

    // Merge changes into metadata_json
    let mut metadata_map = old_metadata_json
        .and_then(|s| serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&s).ok())
        .unwrap_or_default();

    if let Some(ref t) = payload.title {
        metadata_map.insert("TrackTitle".to_string(), serde_json::Value::String(t.clone()));
    }
    if let Some(ref a) = payload.artist {
        metadata_map.insert("TrackArtist".to_string(), serde_json::Value::String(a.clone()));
    }
    if let Some(ref al) = payload.album {
        metadata_map.insert("AlbumTitle".to_string(), serde_json::Value::String(al.clone()));
    }
    if let Some(ref g) = payload.genre {
        metadata_map.insert("Genre".to_string(), serde_json::Value::String(g.clone()));
    }
    if let Some(tr) = payload.track_number {
        metadata_map.insert("TrackNumber".to_string(), serde_json::Value::String(tr.to_string()));
    }
    if let Some(ds) = payload.disc_number {
        metadata_map.insert("DiscNumber".to_string(), serde_json::Value::String(ds.to_string()));
    }

    let new_metadata_json = if metadata_map.is_empty() {
        None
    } else {
        serde_json::to_string(&metadata_map).ok()
    };

    // Update database row
    sqlx::query(
        "UPDATE tracks
         SET title = ?, artist = ?, album = ?, album_id = ?, genre = ?, track_number = ?, disc_number = ?, metadata_json = ?
         WHERE id = ?"
    )
    .bind(&payload.title)
    .bind(&payload.artist)
    .bind(&payload.album)
    .bind(new_album_id)
    .bind(&payload.genre)
    .bind(payload.track_number)
    .bind(payload.disc_number)
    .bind(new_metadata_json)
    .bind(id)
    .execute(&user_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Clean up old empty albums
    if let Some(o_alb_id) = old_album_id {
        if Some(o_alb_id) != new_album_id {
            let tracks_left: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE album_id = ?")
                .bind(o_alb_id)
                .fetch_one(&user_pool)
                .await
                .unwrap_or(0);

            if tracks_left == 0 {
                let art_path_opt: Option<String> = sqlx::query_scalar("SELECT art_path FROM albums WHERE id = ?")
                    .bind(o_alb_id)
                    .fetch_one(&user_pool)
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
                    .execute(&user_pool)
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
    .fetch_one(&user_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Broadcast track.updated
    let payload_val = serde_json::to_value(&track).unwrap_or(serde_json::Value::Null);
    let payload_str = payload_val.to_string();
    let event_type = "track.updated";

    if let Ok(er) = sqlx::query("INSERT INTO events (event_type, payload) VALUES (?, ?)")
        .bind(event_type)
        .bind(&payload_str)
        .execute(&user_pool)
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

#[derive(Deserialize)]
pub struct SingleFetchRequest {
    pub provider: String,
}

pub async fn fetch_track_metadata_inner(
    state: &AppState,
    pool: &sqlx::SqlitePool,
    id: i64,
    provider: &str,
) -> Result<TrackResponse, (StatusCode, String)> {
    // 1. Get track path and metadata from DB
    let track_row = sqlx::query("SELECT id, path, title, artist, album, duration FROM tracks WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Track not found".to_string()))?;

    let track_path: String = track_row.get("path");
    let current_title: Option<String> = track_row.get("title");
    let current_artist: Option<String> = track_row.get("artist");
    let current_album: Option<String> = track_row.get("album");
    let duration: Option<i32> = track_row.get("duration");

    let parsed = crate::api::library::parse_track_info(
        &track_path,
        current_title.as_deref(),
        current_artist.as_deref(),
        current_album.as_deref()
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    if provider == "musicbrainz" {
        let mb_match = crate::api::library::fetch_musicbrainz_metadata(
            &client,
            &parsed.title,
            parsed.artist.as_deref(),
            parsed.album.as_deref(),
            duration,
        )
        .await
        .ok_or_else(|| (StatusCode::NOT_FOUND, "No matching metadata found on MusicBrainz".to_string()))?;

        let album_id = crate::api::library::match_or_create_album(pool, &mb_match.album, &mb_match.artist).await;
        let metadata_json = serde_json::to_string(&mb_match).ok();

        // Update database row
        sqlx::query(
            "UPDATE tracks 
             SET title = ?, artist = ?, album = ?, album_id = ?, track_number = ?, disc_number = ?, genre = ?, external_id = ?, metadata_json = ?,
                 duration = CASE WHEN duration IS NULL OR duration <= 0 THEN ? ELSE duration END
             WHERE id = ?"
        )
        .bind(&mb_match.title)
        .bind(&mb_match.artist)
        .bind(&mb_match.album)
        .bind(album_id)
        .bind(mb_match.track_number)
        .bind(mb_match.disc_number)
        .bind(&mb_match.genre)
        .bind(&mb_match.recording_id)
        .bind(metadata_json)
        .bind(mb_match.duration)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Fetch cover art if available
        if let Some(ref release_id) = mb_match.release_id {
            let cover_url = format!("https://coverartarchive.org/release/{}/front-500", release_id);
            if let Ok(c_resp) = client.get(&cover_url)
                .header("User-Agent", "Audion/0.1.0 ( contact@audion.local )")
                .send()
                .await
            {
                if c_resp.status().is_success() {
                    if let Ok(bytes) = c_resp.bytes().await {
                        // Extract user_id from path if possible to store in their folder, or default path.
                        // Actually, since track_path is format "users/<user_id>/music/...", we can infer the user folder!
                        // Let's write helper logic or write directly inside the parent folder of track_path.
                        let file_full_path = state.config.data_dir.join(&track_path);
                        let relative_cover = if let Some(parent) = std::path::Path::new(&track_path).parent() {
                            if let Some(grandparent) = parent.parent() {
                                grandparent.join("artwork").join(format!("track_{}.jpg", id)).to_string_lossy().to_string().replace("\\", "/")
                            } else {
                                format!("artwork/track_{}.jpg", id)
                            }
                        } else {
                            format!("artwork/track_{}.jpg", id)
                        };
                        let cover_full = state.config.data_dir.join(&relative_cover);
                        std::fs::create_dir_all(cover_full.parent().unwrap()).ok();
                        if std::fs::write(&cover_full, &bytes).is_ok() {
                            sqlx::query("UPDATE tracks SET track_cover_path = ? WHERE id = ?")
                                .bind(&relative_cover)
                                .bind(id)
                                .execute(pool)
                                .await
                                .ok();
                        }
                    }
                }
            }
        }

        // Tag writer block (Lofty)
        let file_full_path = state.config.data_dir.join(&track_path);
        let t_title = mb_match.title.clone();
        let t_artist = mb_match.artist.clone();
        let t_album = mb_match.album.clone();
        let t_track_number = mb_match.track_number;
        let t_disc_number = mb_match.disc_number;
        let t_genre = mb_match.genre.clone();
        let t_recording_id = mb_match.recording_id.clone();

        tokio::task::spawn_blocking(move || {
            if file_full_path.exists() {
                use lofty::prelude::*;
                if let Ok(mut tagged_file) = lofty::probe::Probe::open(&file_full_path)
                    .and_then(|p| p.options(lofty::config::ParseOptions::new().parsing_mode(lofty::config::ParsingMode::Relaxed)).read())
                {
                    let tag = if tagged_file.primary_tag().is_some() {
                        tagged_file.primary_tag_mut()
                    } else {
                        tagged_file.first_tag_mut()
                    };
                    if let Some(tag) = tag {
                        tag.insert_text(lofty::tag::ItemKey::TrackTitle, t_title);
                        tag.insert_text(lofty::tag::ItemKey::TrackArtist, t_artist);
                        tag.insert_text(lofty::tag::ItemKey::AlbumTitle, t_album);
                        if let Some(tn) = t_track_number {
                            tag.insert_text(lofty::tag::ItemKey::TrackNumber, tn.to_string());
                        }
                        if let Some(dn) = t_disc_number {
                            tag.insert_text(lofty::tag::ItemKey::DiscNumber, dn.to_string());
                        }
                        if let Some(g) = t_genre {
                            tag.insert_text(lofty::tag::ItemKey::Genre, g);
                        }
                        tag.insert_text(lofty::tag::ItemKey::MusicBrainzTrackId, t_recording_id);
                        let _ = tag.save_to_path(&file_full_path, lofty::config::WriteOptions::default());
                    }
                }
            }
        });
    } else {
        // Deezer fetch
        let url = "https://api.deezer.com/search";
        let query_param = if let Some(ref artist) = parsed.artist {
            format!("{} {}", parsed.title, artist)
        } else {
            parsed.title.clone()
        };

        let resp = client.get(url)
            .query(&[("q", &query_param)])
            .send()
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Deezer request failed: {}", e)))?;

        let search_res = resp.json::<crate::api::library::DeezerSearchResponse>().await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to parse Deezer response: {}", e)))?;

        let mut best_match: Option<(&crate::api::library::DeezerTrack, f64)> = None;
        for candidate in search_res.data.iter() {
            let score = crate::api::library::calculate_match_score(
                &parsed.title,
                parsed.artist.as_deref(),
                parsed.album.as_deref(),
                duration,
                &candidate.title,
                &candidate.artist.name,
                &candidate.album.title,
                candidate.duration,
            );
            if score >= 0.5 {
                if best_match.is_none() || score > best_match.as_ref().unwrap().1 {
                    best_match = Some((candidate, score));
                }
            }
        }

        let match_track = best_match
            .map(|(t, _)| t)
            .ok_or_else(|| (StatusCode::NOT_FOUND, "No matching metadata found on Deezer above confidence threshold".to_string()))?;
        let album_id = crate::api::library::match_or_create_album(pool, &match_track.album.title, &match_track.artist.name).await;
        let metadata_json = serde_json::to_value(&match_track).ok().map(|v| v.to_string());
        let ext_id = Some(match_track.id.to_string());

        sqlx::query(
            "UPDATE tracks 
             SET title = ?, artist = ?, album = ?, album_id = ?, external_id = ?, metadata_json = ?,
                 duration = CASE WHEN duration IS NULL OR duration <= 0 THEN ? ELSE duration END
             WHERE id = ?"
        )
        .bind(&match_track.title)
        .bind(&match_track.artist.name)
        .bind(&match_track.album.title)
        .bind(album_id)
        .bind(ext_id)
        .bind(metadata_json)
        .bind(match_track.duration)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Fetch cover art
        if let Some(ref cover_url) = match_track.album.cover_big {
            if let Ok(c_resp) = client.get(cover_url).send().await {
                if c_resp.status().is_success() {
                    if let Ok(bytes) = c_resp.bytes().await {
                        let relative_cover = if let Some(parent) = std::path::Path::new(&track_path).parent() {
                            if let Some(grandparent) = parent.parent() {
                                grandparent.join("artwork").join(format!("track_{}.jpg", id)).to_string_lossy().to_string().replace("\\", "/")
                            } else {
                                format!("artwork/track_{}.jpg", id)
                            }
                        } else {
                            format!("artwork/track_{}.jpg", id)
                        };
                        let cover_full = state.config.data_dir.join(&relative_cover);
                        std::fs::create_dir_all(cover_full.parent().unwrap()).ok();
                        if std::fs::write(&cover_full, &bytes).is_ok() {
                            sqlx::query("UPDATE tracks SET track_cover_path = ? WHERE id = ?")
                                .bind(&relative_cover)
                                .bind(id)
                                .execute(pool)
                                .await
                                .ok();
                        }
                    }
                }
            }
        }

        // Tag writer block (Lofty)
        let file_full_path = state.config.data_dir.join(&track_path);
        let t_title = match_track.title.clone();
        let t_artist = match_track.artist.name.clone();
        let t_album = match_track.album.title.clone();

        tokio::task::spawn_blocking(move || {
            if file_full_path.exists() {
                use lofty::prelude::*;
                if let Ok(mut tagged_file) = lofty::probe::Probe::open(&file_full_path)
                    .and_then(|p| p.options(lofty::config::ParseOptions::new().parsing_mode(lofty::config::ParsingMode::Relaxed)).read())
                {
                    let tag = if tagged_file.primary_tag().is_some() {
                        tagged_file.primary_tag_mut()
                    } else {
                        tagged_file.first_tag_mut()
                    };
                    if let Some(tag) = tag {
                        tag.insert_text(lofty::tag::ItemKey::TrackTitle, t_title);
                        tag.insert_text(lofty::tag::ItemKey::TrackArtist, t_artist);
                        tag.insert_text(lofty::tag::ItemKey::AlbumTitle, t_album);
                        let _ = tag.save_to_path(&file_full_path, lofty::config::WriteOptions::default());
                    }
                }
            }
        });
    }

    // Get the updated track
    let track = sqlx::query_as::<_, TrackResponse>(
        "SELECT id, path, title, artist, album, track_number, disc_number, duration,
                album_id, format, bitrate, source_type, cover_url, external_id,
                local_src, track_cover_path, genre, metadata_json, date_added
         FROM tracks
         WHERE id = ?"
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Broadcast track.updated
    let payload_val = serde_json::to_value(&track).unwrap_or(serde_json::Value::Null);
    let payload_str = payload_val.to_string();
    let event_type = "track.updated";

    if let Ok(er) = sqlx::query("INSERT INTO events (event_type, payload) VALUES (?, ?)")
        .bind(event_type)
        .bind(&payload_str)
        .execute(pool)
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

    Ok(track)
}

pub async fn fetch_track_metadata(
    claims: Claims,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<SingleFetchRequest>,
) -> Result<Json<TrackResponse>, (StatusCode, String)> {
    claims.require_non_stream_only().map_err(|(s, m)| (s, m.to_string()))?;
    let user_pool = state.get_user_pool(&claims.sub).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let track = fetch_track_metadata_inner(&state, &user_pool, id, &payload.provider).await?;
    Ok(Json(track))
}

async fn broadcast_bulk_event(state: &AppState, pool: &sqlx::SqlitePool, event_type: &str, payload: serde_json::Value) {
    let payload_str = payload.to_string();
    if let Ok(er) = sqlx::query("INSERT INTO events (event_type, payload) VALUES (?, ?)")
        .bind(event_type)
        .bind(&payload_str)
        .execute(pool)
        .await
    {
        let event_id = er.last_insert_rowid();
        state.event_bus.broadcast(ServerEvent {
            id: event_id,
            event_type: event_type.to_string(),
            payload,
            created_at: chrono::Utc::now().to_rfc3339(),
        });
    }
}

#[derive(Deserialize)]
pub struct BulkFetchRequest {
    pub track_ids: Vec<i64>,
    pub provider: String,
}

pub async fn bulk_fetch_metadata(
    claims: Claims,
    State(state): State<AppState>,
    Json(payload): Json<BulkFetchRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    claims.require_non_stream_only().map_err(|(s, m)| (s, m.to_string()))?;
    let user_pool = state.get_user_pool(&claims.sub).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let total = payload.track_ids.len();
    for (idx, id) in payload.track_ids.iter().copied().enumerate() {
        if let Err(e) = fetch_track_metadata_inner(&state, &user_pool, id, &payload.provider).await {
            error!("Failed to fetch metadata for track {}: {:?}", id, e);
        }
        
        let progress_payload = serde_json::json!({
            "action": "fetch",
            "current": idx + 1,
            "total": total,
            "track_id": id,
        });
        broadcast_bulk_event(&state, &user_pool, "bulk.progress", progress_payload).await;

        // Yield execution to avoid rate limits
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    let completed_payload = serde_json::json!({
        "action": "fetch",
        "total": total,
    });
    broadcast_bulk_event(&state, &user_pool, "bulk.completed", completed_payload).await;

    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
pub struct BulkDeleteRequest {
    pub track_ids: Vec<i64>,
}

pub async fn bulk_delete_tracks(
    claims: Claims,
    State(state): State<AppState>,
    Json(payload): Json<BulkDeleteRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    claims.require_non_stream_only().map_err(|(s, m)| (s, m.to_string()))?;
    let user_pool = state.get_user_pool(&claims.sub).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let total = payload.track_ids.len();
    for (idx, id) in payload.track_ids.iter().copied().enumerate() {
        if let Err(e) = delete_track_inner(&state, &user_pool, id).await {
            error!("Failed to delete track {}: {:?}", id, e);
        }
        
        let progress_payload = serde_json::json!({
            "action": "delete",
            "current": idx + 1,
            "total": total,
            "track_id": id,
        });
        broadcast_bulk_event(&state, &user_pool, "bulk.progress", progress_payload).await;
    }

    let completed_payload = serde_json::json!({
        "action": "delete",
        "total": total,
    });
    broadcast_bulk_event(&state, &user_pool, "bulk.completed", completed_payload).await;

    Ok(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bulk_payload_deserialization() {
        let json_data = r#"{"track_ids": [1, 2, 3], "provider": "musicbrainz"}"#;
        let req: BulkFetchRequest = serde_json::from_str(json_data).unwrap();
        assert_eq!(req.track_ids, vec![1, 2, 3]);
        assert_eq!(req.provider, "musicbrainz");

        let json_del = r#"{"track_ids": [10, 20]}"#;
        let req_del: BulkDeleteRequest = serde_json::from_str(json_del).unwrap();
        assert_eq!(req_del.track_ids, vec![10, 20]);
    }
}



