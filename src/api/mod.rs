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
pub mod storage_settings;
pub mod folders;

use axum::{
    routing::{get, post, put, delete},
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    Json,
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use serde::Serialize;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};

use crate::state::AppState;
use crate::auth::Claims;

#[derive(Serialize)]
struct StatsResponse {
    total_tracks: i32,
    total_albums: i32,
    total_artists: i32,
    total_size_bytes: i64,
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
        // Support comma-separated list of origins: "https://app.example.com,https://admin.example.com"
        let origins: Vec<axum::http::HeaderValue> = cors_origin
            .split(',')
            .filter_map(|o| o.trim().parse::<axum::http::HeaderValue>().ok())
            .collect();
        if origins.is_empty() {
            CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any)
        } else {
            CorsLayer::new()
                .allow_origin(origins)
                .allow_methods(Any)
                .allow_headers(Any)
        }
    };

    let public_dir = state.config.public_dir.clone();
    let serve_dir = tower_http::services::ServeDir::new(&public_dir)
        .fallback(tower_http::services::ServeFile::new(public_dir.join("index.html")));

    let governor_conf = std::sync::Arc::new(
        GovernorConfigBuilder::default()
            .per_second(6)
            .burst_size(10)
            .finish()
            .unwrap(),
    );

    Router::new()
        .route("/api/health", get(health))
        .route("/api/stats", get(stats))
        .route("/api/server-info", get(server_info))
        .route("/api/auth/login", post(auth::login).layer(GovernorLayer { config: governor_conf }))
        .route("/api/auth/me", get(auth::me))
        .route("/api/auth/refresh", post(auth::refresh_token))
        .route("/api/auth/profile", put(auth::update_profile))
        .route("/api/admin/users", get(users::list_users).post(users::create_user))
        .route("/api/admin/users/:id", put(users::update_user).delete(users::delete_user))
        .route("/api/admin/users/:id/quota", put(users::update_user_quota))
        .route("/api/admin/stats", get(users::admin_stats))
        .route("/api/admin/tunnel", get(tunnel::get_tunnel_info).put(tunnel::update_tunnel_config))
        .route("/api/admin/tunnel/toggle", post(tunnel::toggle_tunnel))
        .route("/api/admin/storage", get(storage_settings::get_storage_settings).put(storage_settings::update_storage_settings))
        // Subsonic API — both .view and bare paths for client compatibility
        .route("/rest/ping.view", get(subsonic::ping).post(subsonic::ping))
        .route("/rest/ping", get(subsonic::ping).post(subsonic::ping))
        .route("/rest/getLicense.view", get(subsonic::get_license).post(subsonic::get_license))
        .route("/rest/getLicense", get(subsonic::get_license).post(subsonic::get_license))
        .route("/rest/getOpenSubsonicExtensions.view", get(subsonic::get_open_subsonic_extensions).post(subsonic::get_open_subsonic_extensions))
        .route("/rest/getOpenSubsonicExtensions", get(subsonic::get_open_subsonic_extensions).post(subsonic::get_open_subsonic_extensions))
        .route("/rest/getMusicFolders.view", get(subsonic::get_music_folders).post(subsonic::get_music_folders))
        .route("/rest/getMusicFolders", get(subsonic::get_music_folders).post(subsonic::get_music_folders))
        .route("/rest/getIndexes.view", get(subsonic::get_indexes).post(subsonic::get_indexes))
        .route("/rest/getIndexes", get(subsonic::get_indexes).post(subsonic::get_indexes))
        .route("/rest/getArtists.view", get(subsonic::get_artists).post(subsonic::get_artists))
        .route("/rest/getArtists", get(subsonic::get_artists).post(subsonic::get_artists))
        .route("/rest/getArtist.view", get(subsonic::get_artist).post(subsonic::get_artist))
        .route("/rest/getArtist", get(subsonic::get_artist).post(subsonic::get_artist))
        .route("/rest/getAlbum.view", get(subsonic::get_album).post(subsonic::get_album))
        .route("/rest/getAlbum", get(subsonic::get_album).post(subsonic::get_album))
        .route("/rest/getMusicDirectory.view", get(subsonic::get_music_directory).post(subsonic::get_music_directory))
        .route("/rest/getMusicDirectory", get(subsonic::get_music_directory).post(subsonic::get_music_directory))
        .route("/rest/getSong.view", get(subsonic::get_song).post(subsonic::get_song))
        .route("/rest/getSong", get(subsonic::get_song).post(subsonic::get_song))
        .route("/rest/getGenres.view", get(subsonic::get_genres).post(subsonic::get_genres))
        .route("/rest/getGenres", get(subsonic::get_genres).post(subsonic::get_genres))
        .route("/rest/getAlbumList.view", get(subsonic::get_album_list).post(subsonic::get_album_list))
        .route("/rest/getAlbumList", get(subsonic::get_album_list).post(subsonic::get_album_list))
        .route("/rest/getAlbumList2.view", get(subsonic::get_album_list).post(subsonic::get_album_list))
        .route("/rest/getAlbumList2", get(subsonic::get_album_list).post(subsonic::get_album_list))
        .route("/rest/getRandomSongs.view", get(subsonic::get_random_songs).post(subsonic::get_random_songs))
        .route("/rest/getRandomSongs", get(subsonic::get_random_songs).post(subsonic::get_random_songs))
        .route("/rest/getSongsByGenre.view", get(subsonic::get_songs_by_genre).post(subsonic::get_songs_by_genre))
        .route("/rest/getSongsByGenre", get(subsonic::get_songs_by_genre).post(subsonic::get_songs_by_genre))
        .route("/rest/getStarred.view", get(subsonic::get_starred).post(subsonic::get_starred))
        .route("/rest/getStarred", get(subsonic::get_starred).post(subsonic::get_starred))
        .route("/rest/getStarred2.view", get(subsonic::get_starred).post(subsonic::get_starred))
        .route("/rest/getStarred2", get(subsonic::get_starred).post(subsonic::get_starred))
        .route("/rest/star.view", get(subsonic::star).post(subsonic::star))
        .route("/rest/star", get(subsonic::star).post(subsonic::star))
        .route("/rest/unstar.view", get(subsonic::unstar).post(subsonic::unstar))
        .route("/rest/unstar", get(subsonic::unstar).post(subsonic::unstar))
        .route("/rest/search2.view", get(subsonic::search3).post(subsonic::search3))
        .route("/rest/search2", get(subsonic::search3).post(subsonic::search3))
        .route("/rest/search3.view", get(subsonic::search3).post(subsonic::search3))
        .route("/rest/search3", get(subsonic::search3).post(subsonic::search3))
        .route("/rest/getPlaylists.view", get(subsonic::get_playlists).post(subsonic::get_playlists))
        .route("/rest/getPlaylists", get(subsonic::get_playlists).post(subsonic::get_playlists))
        .route("/rest/getPlaylist.view", get(subsonic::get_playlist).post(subsonic::get_playlist))
        .route("/rest/getPlaylist", get(subsonic::get_playlist).post(subsonic::get_playlist))
        .route("/rest/createPlaylist.view", get(subsonic::create_playlist).post(subsonic::create_playlist))
        .route("/rest/createPlaylist", get(subsonic::create_playlist).post(subsonic::create_playlist))
        .route("/rest/updatePlaylist.view", get(subsonic::update_playlist).post(subsonic::update_playlist))
        .route("/rest/updatePlaylist", get(subsonic::update_playlist).post(subsonic::update_playlist))
        .route("/rest/deletePlaylist.view", get(subsonic::delete_playlist).post(subsonic::delete_playlist))
        .route("/rest/deletePlaylist", get(subsonic::delete_playlist).post(subsonic::delete_playlist))
        .route("/rest/getCoverArt", get(subsonic::get_cover_art).post(subsonic::get_cover_art))
        .route("/rest/getCoverArt.view", get(subsonic::get_cover_art).post(subsonic::get_cover_art))
        .route("/rest/stream", get(subsonic::stream).post(subsonic::stream))
        .route("/rest/stream.view", get(subsonic::stream).post(subsonic::stream))
        .route("/rest/getUser.view", get(subsonic::get_user).post(subsonic::get_user))
        .route("/rest/getUser", get(subsonic::get_user).post(subsonic::get_user))
        .route("/rest/getUsers.view", get(subsonic::get_users).post(subsonic::get_users))
        .route("/rest/getUsers", get(subsonic::get_users).post(subsonic::get_users))
        .route("/rest/getScanStatus.view", get(subsonic::get_scan_status).post(subsonic::get_scan_status))
        .route("/rest/getScanStatus", get(subsonic::get_scan_status).post(subsonic::get_scan_status))
        .route("/rest/scrobble.view", get(subsonic::scrobble).post(subsonic::scrobble))
        .route("/rest/scrobble", get(subsonic::scrobble).post(subsonic::scrobble))
        .route("/rest/updateTags.view", get(subsonic::update_tags).post(subsonic::update_tags))
        .route("/rest/updateTags", get(subsonic::update_tags).post(subsonic::update_tags))
        .route("/rest/getArtists", get(subsonic::get_artists).post(subsonic::get_artists))
        .route("/rest/getArtist.view", get(subsonic::get_artist).post(subsonic::get_artist))
        .route("/rest/getArtist", get(subsonic::get_artist).post(subsonic::get_artist))
        .route("/rest/getAlbum.view", get(subsonic::get_album).post(subsonic::get_album))
        .route("/rest/getAlbum", get(subsonic::get_album).post(subsonic::get_album))
        .route("/rest/getStarred.view", get(subsonic::get_starred).post(subsonic::get_starred))
        .route("/rest/getStarred", get(subsonic::get_starred).post(subsonic::get_starred))
        .route("/rest/getStarred2.view", get(subsonic::get_starred).post(subsonic::get_starred))
        .route("/rest/getStarred2", get(subsonic::get_starred).post(subsonic::get_starred))
        .route("/rest/star.view", get(subsonic::star).post(subsonic::star))
        .route("/rest/star", get(subsonic::star).post(subsonic::star))
        .route("/rest/unstar.view", get(subsonic::unstar).post(subsonic::unstar))
        .route("/rest/unstar", get(subsonic::unstar).post(subsonic::unstar))
        .route("/api/tracks", get(tracks::get_tracks).post(tracks::upload_track))
        .route("/api/tracks/:id", get(tracks::get_track_by_id).delete(tracks::delete_track))
        .route("/api/tracks/:id/metadata", put(tracks::update_track_metadata))
        .route("/api/tracks/:id/fetch", post(tracks::fetch_track_metadata))
        .route("/api/tracks/bulk/fetch", post(tracks::bulk_fetch_metadata))
        .route("/api/tracks/bulk/metadata", post(tracks::bulk_update_metadata))
        .route("/api/tracks/bulk/delete", post(tracks::bulk_delete_tracks))
        .route("/api/folders", get(folders::list_folder))
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

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    timestamp: String,
}

async fn health(
    State(state): State<AppState>,
) -> (StatusCode, Json<HealthResponse>) {
    let db_check = sqlx::query("SELECT 1")
        .execute(&state.pool)
        .await;

    let timestamp = chrono::Utc::now().to_rfc3339();
    let version = env!("CARGO_PKG_VERSION").to_string();

    match db_check {
        Ok(_) => (
            StatusCode::OK,
            Json(HealthResponse {
                status: "ok".to_string(),
                version,
                timestamp,
            }),
        ),
        Err(e) => {
            tracing::error!("Health check database connection failed: {:?}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthResponse {
                    status: "degraded".to_string(),
                    version,
                    timestamp,
                }),
            )
        }
    }
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

    Ok(Json(StatsResponse {
        total_tracks,
        total_albums,
        total_artists,
        total_size_bytes,
    }))
}

async fn server_info() -> Json<ServerInfoResponse> {
    Json(ServerInfoResponse {
        version: env!("CARGO_PKG_VERSION"),
        server_name: "Audion Server",
    })
}
