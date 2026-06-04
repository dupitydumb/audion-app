use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    body::Body,
};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
use tokio_util::io::ReaderStream;
use std::path::PathBuf;

use sqlx::Row;
use crate::state::AppState;
use crate::auth::Claims;
use crate::events::ServerEvent;

fn parse_range(range_header: &str, file_size: u64) -> Option<(u64, u64)> {
    if !range_header.starts_with("bytes=") {
        return None;
    }
    let range_str = &range_header[6..];
    let parts: Vec<&str> = range_str.split('-').collect();
    if parts.len() != 2 {
        return None;
    }
    
    let start_str = parts[0].trim();
    let end_str = parts[1].trim();

    let start = if start_str.is_empty() {
        0
    } else {
        start_str.parse::<u64>().ok()?
    };

    let end = if end_str.is_empty() {
        file_size - 1
    } else {
        end_str.parse::<u64>().ok()?
    };

    if start > end || start >= file_size {
        return None;
    }

    let end = end.min(file_size - 1);
    Some((start, end))
}

fn mime_for_format(format: Option<&str>) -> &'static str {
    match format {
        Some(f) => match f.to_lowercase().as_str() {
            "mp3" => "audio/mpeg",
            "flac" => "audio/flac",
            "alac" | "m4a" | "aac" => "audio/mp4",
            "ogg" | "vorbis" => "audio/ogg",
            "wav" => "audio/wav",
            _ => "audio/mpeg",
        },
        None => "audio/mpeg",
    }
}

pub async fn stream_track(
    _claims: Claims,
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Response {
    let (path, format) = match sqlx::query("SELECT path, format FROM tracks WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(row)) => {
            let path: String = row.get("path");
            let format: Option<String> = row.get("format");
            (path, format)
        }
        _ => return StatusCode::NOT_FOUND.into_response(),
    };

    let mut stream_path = state.config.data_dir.join(&path);
    let mut mime_type = mime_for_format(format.as_deref());

    if format.as_deref().map(|f| f.to_lowercase()) == Some("flac".to_string()) && state.has_ffmpeg {
        let cache_dir = state.config.data_dir.join("transcoded");
        if std::fs::create_dir_all(&cache_dir).is_ok() {
            let cache_path = cache_dir.join(format!("{}.mp3", id));
            let mut transcode_success = cache_path.exists();

            if !transcode_success {
                tracing::info!("Transcoding FLAC track {} to MP3 on-demand...", id);

                // Broadcast that transcoding has started
                state.event_bus.broadcast(ServerEvent {
                    id: 0,
                    event_type: "track.transcoding".to_string(),
                    payload: serde_json::json!({ "id": id, "status": "started" }),
                    created_at: chrono::Utc::now().to_rfc3339(),
                });

                let temp_path = cache_dir.join(format!("{}.temp.mp3", id));
                let status = tokio::process::Command::new("ffmpeg")
                    .args(&[
                        "-y",
                        "-i", &stream_path.to_string_lossy(),
                        "-codec:a", "libmp3lame",
                        "-b:a", "320k",
                        &temp_path.to_string_lossy(),
                    ])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .await;

                let rename_success = match status {
                    Ok(s) if s.success() => {
                        if tokio::fs::rename(&temp_path, &cache_path).await.is_ok() {
                            tracing::info!("Transcoding of track {} complete.", id);
                            transcode_success = true;
                            true
                        } else {
                            tracing::error!("Failed to rename temp transcoded file for track {}", id);
                            let _ = tokio::fs::remove_file(&temp_path).await;
                            false
                        }
                    }
                    _ => {
                        tracing::error!("FFmpeg transcoding failed for track {}", id);
                        let _ = tokio::fs::remove_file(&temp_path).await;
                        false
                    }
                };

                // Broadcast final transcoding status
                let final_status = if rename_success { "complete" } else { "failed" };
                state.event_bus.broadcast(ServerEvent {
                    id: 0,
                    event_type: "track.transcoding".to_string(),
                    payload: serde_json::json!({ "id": id, "status": final_status }),
                    created_at: chrono::Utc::now().to_rfc3339(),
                });
            }

            if transcode_success {
                stream_path = cache_path;
                mime_type = "audio/mpeg";
            }
        }
    }

    let mut file = match File::open(&stream_path).await {
        Ok(f) => f,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    let file_size = match file.metadata().await {
        Ok(m) => m.len(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    // Check for Range header
    let range = headers.get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| parse_range(s, file_size));

    match range {
        Some((start, end)) => {
            let len = end - start + 1;
            if file.seek(SeekFrom::Start(start)).await.is_err() {
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }

            let stream = ReaderStream::new(file.take(len));
            let body = Body::from_stream(stream);

            Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_RANGE, format!("bytes {}-{}/{}", start, end, file_size))
                .header(header::CONTENT_LENGTH, len)
                .header(header::ACCEPT_RANGES, "bytes")
                .header(header::CONTENT_TYPE, mime_type)
                .body(body)
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        None => {
            let stream = ReaderStream::new(file);
            let body = Body::from_stream(stream);

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_LENGTH, file_size)
                .header(header::ACCEPT_RANGES, "bytes")
                .header(header::CONTENT_TYPE, mime_type)
                .body(body)
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
    }
}

pub async fn get_track_cover(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let cover_path = match sqlx::query("SELECT track_cover_path FROM tracks WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(row)) => row.get::<Option<String>, _>("track_cover_path"),
        _ => None,
    };

    if let Some(ref path) = cover_path {
        let full_path = state.config.data_dir.join(path);
        if full_path.exists() {
            if let Ok(bytes) = std::fs::read(&full_path) {
                let mime = mime_guess::from_path(&full_path).first_or_octet_stream();
                return (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, mime.to_string())],
                    bytes,
                ).into_response();
            }
        }
    }

    StatusCode::NOT_FOUND.into_response()
}
