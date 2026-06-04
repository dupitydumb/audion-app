use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use crate::state::AppState;
use crate::auth::Claims;
use crate::api::tracks::TrackResponse;
use crate::events::ServerEvent;

pub async fn get_liked_tracks(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<Vec<TrackResponse>>, (StatusCode, String)> {
    let tracks = sqlx::query_as::<_, TrackResponse>(
        "SELECT t.id, t.path, t.title, t.artist, t.album, t.track_number, t.disc_number, t.duration,
                t.album_id, t.format, t.bitrate, t.source_type, t.cover_url, t.external_id,
                t.local_src, t.track_cover_path, t.genre, t.metadata_json, t.date_added
         FROM tracks t
         JOIN liked_tracks lt ON lt.track_id = t.id
         WHERE lt.user_id = ?
         ORDER BY lt.liked_at DESC"
    )
    .bind(&claims.sub)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(tracks))
}

pub async fn like_track(
    claims: Claims,
    State(state): State<AppState>,
    Path(track_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Verify track exists
    let _exists = sqlx::query("SELECT id FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Track not found".to_string()))?;

    sqlx::query(
        "INSERT OR IGNORE INTO liked_tracks (user_id, track_id) VALUES (?, ?)"
    )
    .bind(&claims.sub)
    .bind(track_id)
    .execute(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Record and broadcast event
    let payload = serde_json::json!({ "track_id": track_id, "liked": true });
    let payload_str = payload.to_string();
    let event_type = "liked.changed";

    if let Ok(res) = sqlx::query(
        "INSERT INTO events (event_type, payload) VALUES (?, ?)"
    )
    .bind(event_type)
    .bind(&payload_str)
    .execute(&state.pool)
    .await
    {
        let event_id = res.last_insert_rowid();
        let time_now = chrono::Utc::now().to_rfc3339();
        state.event_bus.broadcast(ServerEvent {
            id: event_id,
            event_type: event_type.to_string(),
            payload,
            created_at: time_now,
        });
    }

    Ok(StatusCode::OK)
}

pub async fn unlike_track(
    claims: Claims,
    State(state): State<AppState>,
    Path(track_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    sqlx::query(
        "DELETE FROM liked_tracks WHERE user_id = ? AND track_id = ?"
    )
    .bind(&claims.sub)
    .bind(track_id)
    .execute(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Record and broadcast event
    let payload = serde_json::json!({ "track_id": track_id, "liked": false });
    let payload_str = payload.to_string();
    let event_type = "liked.changed";

    if let Ok(res) = sqlx::query(
        "INSERT INTO events (event_type, payload) VALUES (?, ?)"
    )
    .bind(event_type)
    .bind(&payload_str)
    .execute(&state.pool)
    .await
    {
        let event_id = res.last_insert_rowid();
        let time_now = chrono::Utc::now().to_rfc3339();
        state.event_bus.broadcast(ServerEvent {
            id: event_id,
            event_type: event_type.to_string(),
            payload,
            created_at: time_now,
        });
    }

    Ok(StatusCode::OK)
}
