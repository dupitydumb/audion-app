pub mod auth;
pub mod tracks;
pub mod albums;
pub mod artists;
pub mod playlists;
pub mod liked;
pub mod search;
pub mod stream;
pub mod events;

use axum::{
    routing::{get, post, put, delete},
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/me", get(auth::me))
        .route("/api/tracks", get(tracks::get_tracks).post(tracks::upload_track))
        .route("/api/tracks/:id", get(tracks::get_track_by_id).delete(tracks::delete_track))
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
        .route("/api/playlists/:playlist_id/tracks/:track_id", delete(playlists::remove_track_from_playlist))
        .route("/api/playlists/:id/tracks/reorder", put(playlists::reorder_playlist_tracks))
        .route("/api/liked", get(liked::get_liked_tracks))
        .route("/api/liked/:track_id", post(liked::like_track).delete(liked::unlike_track))
        .route("/api/search", get(search::search))
        .route("/api/tracks/:id/stream", get(stream::stream_track))
        .route("/api/tracks/:id/cover", get(stream::get_track_cover))
        .route("/api/events", get(events::handle_events))
        .layer(cors)
        .with_state(state)
}

async fn health() -> &'static str {
    "OK"
}
