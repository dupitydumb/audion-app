use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use crate::auth::Claims;
use crate::api::tracks::TrackResponse;
use crate::api::albums::AlbumResponse;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ArtistResponse {
    pub name: String,
    pub track_count: i32,
    pub album_count: i32,
}

pub async fn get_artists(
    _claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<Vec<ArtistResponse>>, (StatusCode, String)> {
    let artists = sqlx::query_as::<_, ArtistResponse>(
        "SELECT artist as name, COUNT(*) as track_count, COUNT(DISTINCT album) as album_count 
         FROM tracks 
         WHERE artist IS NOT NULL 
         GROUP BY artist 
         ORDER BY artist"
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(artists))
}

pub async fn get_artist_albums(
    _claims: Claims,
    State(state): State<AppState>,
    Path(artist_name): Path<String>,
) -> Result<Json<Vec<AlbumResponse>>, (StatusCode, String)> {
    let albums = sqlx::query_as::<_, AlbumResponse>(
        "SELECT DISTINCT id, name, artist, art_path 
         FROM albums 
         WHERE artist = ? OR id IN (SELECT DISTINCT album_id FROM tracks WHERE artist = ?)
         ORDER BY name"
    )
    .bind(&artist_name)
    .bind(&artist_name)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(albums))
}

pub async fn get_artist_tracks(
    _claims: Claims,
    State(state): State<AppState>,
    Path(artist_name): Path<String>,
) -> Result<Json<Vec<TrackResponse>>, (StatusCode, String)> {
    let tracks = sqlx::query_as::<_, TrackResponse>(
        "SELECT id, path, title, artist, album, track_number, disc_number, duration,
                album_id, format, bitrate, source_type, cover_url, external_id,
                local_src, track_cover_path, genre, metadata_json, date_added
         FROM tracks
         WHERE artist = ?
         ORDER BY album, disc_number, track_number, title"
    )
    .bind(artist_name)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(tracks))
}
