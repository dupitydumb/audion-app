pub mod auth;
pub mod tracks;
pub mod albums;
pub mod artists;
pub mod playlists;
pub mod liked;
pub mod search;
pub mod stream;
pub mod events;
pub mod library;
pub mod users;
pub mod subsonic;
pub mod tunnel;

use axum::{
    routing::{get, post, put, delete},
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    Json,
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use serde::Serialize;

use crate::state::AppState;
use crate::auth::Claims;

#[derive(Serialize)]
struct StatsResponse {
    total_tracks: i32,
    total_albums: i32,
    total_artists: i32,
    total_size_bytes: i64,
    data_dir: String,
}

#[derive(Serialize)]
struct ServerInfoResponse {
    version: &'static str,
    server_name: &'static str,
}

pub fn create_router(state: AppState) -> Router {
    let cors_origin = std::env::var("AUDION_CORS_ORIGIN").unwrap_or_else(|_| "*".to_string());
    let cors = if cors_origin == "*" {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        match cors_origin.parse::<axum::http::HeaderValue>() {
            Ok(origin) => CorsLayer::new()
                .allow_origin(origin)
                .allow_methods(Any)
                .allow_headers(Any),
            Err(_) => CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        }
    };

    let public_dir = state.config.public_dir.clone();
    let serve_dir = tower_http::services::ServeDir::new(&public_dir)
        .fallback(tower_http::services::ServeFile::new(public_dir.join("index.html")));

    Router::new()
        .route("/api/health", get(health))
        .route("/api/stats", get(stats))
        .route("/api/server-info", get(server_info))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/me", get(auth::me))
        .route("/api/auth/profile", put(auth::update_profile))
        .route("/api/admin/users", get(users::list_users).post(users::create_user))
        .route("/api/admin/users/:id", put(users::update_user).delete(users::delete_user))
        .route("/api/admin/stats", get(users::admin_stats))
        .route("/api/admin/tunnel", get(tunnel::get_tunnel_info).put(tunnel::update_tunnel_config))
        .route("/api/admin/tunnel/toggle", post(tunnel::toggle_tunnel))
        .route("/rest/ping.view", get(subsonic::ping).post(subsonic::ping))
        .route("/rest/getLicense.view", get(subsonic::get_license).post(subsonic::get_license))
        .route("/rest/getMusicFolders.view", get(subsonic::get_music_folders).post(subsonic::get_music_folders))
        .route("/rest/getIndexes.view", get(subsonic::get_indexes).post(subsonic::get_indexes))
        .route("/rest/getMusicDirectory.view", get(subsonic::get_music_directory).post(subsonic::get_music_directory))
        .route("/rest/getSong.view", get(subsonic::get_song).post(subsonic::get_song))
        .route("/rest/stream", get(subsonic::stream).post(subsonic::stream))
        .route("/rest/stream.view", get(subsonic::stream).post(subsonic::stream))
        .route("/rest/scrobble.view", get(subsonic::scrobble).post(subsonic::scrobble))
        .route("/api/tracks", get(tracks::get_tracks).post(tracks::upload_track))
        .route("/api/tracks/:id", get(tracks::get_track_by_id).delete(tracks::delete_track))
        .route("/api/tracks/:id/metadata", put(tracks::update_track_metadata))
        .route("/api/tracks/:id/fetch", post(tracks::fetch_track_metadata))
        .route("/api/tracks/bulk/fetch", post(tracks::bulk_fetch_metadata))
        .route("/api/tracks/bulk/delete", post(tracks::bulk_delete_tracks))
        .route("/api/library/scan", post(library::start_scan))
        .route("/api/library/scan-status", get(library::get_scan_status))
        .route("/api/library/fetch", post(library::start_metadata_fetcher))
        .route("/api/library/fetch-status", get(library::get_fetch_status))
        .route("/api/library/clean", post(library::clean_library))
        .route("/api/library/reset", post(library::reset_library))
        .route("/api/albums", get(albums::get_albums))
        .route("/api/albums/:id", get(albums::get_album_by_id))
        .route("/api/albums/:id/tracks", get(albums::get_album_tracks))
        .route("/api/albums/:id/artwork", get(albums::get_album_artwork))
        .route("/api/artists", get(artists::get_artists))
        .route("/api/artists/:name/albums", get(artists::get_artist_albums))
        .route("/api/artists/:name/tracks", get(artists::get_artist_tracks))
        .route("/api/playlists", get(playlists::get_playlists).post(playlists::create_playlist))
        .route("/api/playlists/:id", get(playlists::get_playlist_by_id).put(playlists::update_playlist).delete(playlists::delete_playlist))
        .route("/api/playlists/:id/tracks", get(playlists::get_playlist_tracks).post(playlists::add_track_to_playlist))
        .route("/api/playlists/:id/tracks/bulk", post(playlists::bulk_add_tracks_to_playlist))
        .route("/api/playlists/:playlist_id/tracks/:track_id", delete(playlists::remove_track_from_playlist))
        .route("/api/playlists/:id/tracks/reorder", put(playlists::reorder_playlist_tracks))
        .route("/api/liked", get(liked::get_liked_tracks))
        .route("/api/liked/:track_id", post(liked::like_track).delete(liked::unlike_track))
        .route("/api/search", get(search::search))
        .route("/api/tracks/:id/stream", get(stream::stream_track))
        .route("/api/tracks/:id/cover", get(stream::get_track_cover))
        .route("/api/tracks/:id/lyrics", get(tracks::get_track_lyrics))
        .route("/api/events", get(events::handle_events))
        .fallback_service(serve_dir)
        .layer(cors)
        .layer(DefaultBodyLimit::max(
            std::env::var("AUDION_MAX_BODY_SIZE")
                .ok()
                .and_then(|val| val.parse::<usize>().ok())
                .unwrap_or(250 * 1024 * 1024) // 250MB default
        ))
        .with_state(state)
}

async fn health() -> &'static str {
    "OK"
}

async fn stats(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<StatsResponse>, (StatusCode, String)> {
    let user_pool = state.get_user_pool(&claims.sub).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total_tracks: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&user_pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total_albums: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM albums")
        .fetch_one(&user_pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total_artists: i32 = sqlx::query_scalar("SELECT COUNT(DISTINCT artist) FROM tracks")
        .fetch_one(&user_pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total_size_bytes: i64 = sqlx::query_scalar::<_, Option<i64>>("SELECT SUM(size) FROM tracks")
        .fetch_one(&user_pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .unwrap_or(0);

    let data_dir = state.config.data_dir.to_string_lossy().to_string();

    Ok(Json(StatsResponse {
        total_tracks,
        total_albums,
        total_artists,
        total_size_bytes,
        data_dir,
    }))
}

async fn server_info() -> Json<ServerInfoResponse> {
    Json(ServerInfoResponse {
        version: env!("CARGO_PKG_VERSION"),
        server_name: "Audion Server",
    })
}
