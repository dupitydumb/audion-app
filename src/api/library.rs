use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, error, warn};
use sqlx::Row;
use lofty::prelude::*;
use lofty::file::TaggedFileExt;

use crate::state::{AppState, ScanStatus, FetcherStatus};
use crate::auth::Claims;
use crate::scanner::extract_metadata;
use crate::events::ServerEvent;
use crate::api::tracks::TrackResponse;

#[derive(Serialize)]
pub struct LibraryStatusResponse {
    pub is_scanning: bool,
    pub files_scanned: usize,
    pub total_files: usize,
    pub current_file: Option<String>,
}

#[derive(Serialize)]
pub struct FetchStatusResponse {
    pub is_running: bool,
    pub tracks_processed: usize,
    pub total_tracks: usize,
    pub current_track: Option<String>,
    pub logs: Vec<String>,
}

#[derive(Deserialize, Serialize)]
pub struct FetcherRequest {
    pub provider: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct DeezerArtist {
    name: String,
}

#[derive(Deserialize, Serialize)]
struct DeezerAlbum {
    title: String,
    #[serde(rename = "cover_big")]
    cover_big: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct DeezerTrack {
    id: i64,
    title: String,
    artist: DeezerArtist,
    album: DeezerAlbum,
}

#[derive(Deserialize, Serialize)]
struct DeezerSearchResponse {
    data: Vec<DeezerTrack>,
}

// MusicBrainz structs
#[derive(Deserialize, Debug)]
struct MbArtist {
    name: String,
}

#[derive(Deserialize, Debug)]
struct MbArtistCredit {
    artist: MbArtist,
}

#[derive(Deserialize, Debug, Clone)]
struct MbTrack {
    number: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct MbMedia {
    #[serde(rename = "track-count")]
    _track_count: Option<i32>,
    tracks: Option<Vec<MbTrack>>,
}

#[derive(Deserialize, Debug, Clone)]
struct MbRelease {
    id: String,
    title: String,
    date: Option<String>,
    media: Option<Vec<MbMedia>>,
}

#[derive(Deserialize, Debug)]
struct MbTag {
    name: String,
    count: i32,
}

#[derive(Deserialize, Debug)]
struct MbRecording {
    id: String,
    title: String,
    #[serde(rename = "artist-credit")]
    artist_credit: Option<Vec<MbArtistCredit>>,
    releases: Option<Vec<MbRelease>>,
    tags: Option<Vec<MbTag>>,
}

#[derive(Deserialize, Debug)]
struct MbSearchResponse {
    recordings: Vec<MbRecording>,
}

#[derive(Serialize, Debug)]
struct MusicBrainzMatch {
    title: String,
    artist: String,
    album: String,
    track_number: Option<i32>,
    disc_number: Option<i32>,
    genre: Option<String>,
    release_id: Option<String>,
    recording_id: String,
}

async fn fetch_musicbrainz_metadata(
    client: &reqwest::Client,
    search_term: &str,
) -> Option<MusicBrainzMatch> {
    let url = "https://musicbrainz.org/ws/2/recording";
    let query_param = search_term
        .replace(":", " ")
        .replace("-", " ")
        .replace("\"", " ");
    
    let response = client.get(url)
        .header("User-Agent", "Audion/0.1.0 ( contact@audion.local )")
        .query(&[("query", query_param.as_str()), ("fmt", "json")])
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let search_res = response.json::<MbSearchResponse>().await.ok()?;
    if search_res.recordings.is_empty() {
        return None;
    }

    let recording = &search_res.recordings[0];
    
    let title = recording.title.clone();
    
    let artist = recording.artist_credit.as_ref()
        .and_then(|ac| ac.first())
        .map(|c| c.artist.name.clone())
        .unwrap_or_else(|| "Unknown Artist".to_string());
        
    let release = recording.releases.as_ref().and_then(|r| r.first());
    let album = release.map(|r| r.title.clone()).unwrap_or_else(|| "Unknown Album".to_string());
    let release_id = release.map(|r| r.id.clone());
    
    let mut track_number = None;
    let mut disc_number = None;
    
    if let Some(r) = release {
        if let Some(ref media) = r.media {
            for (m_idx, m) in media.iter().enumerate() {
                if let Some(ref tracks) = m.tracks {
                    for t in tracks {
                        if let Some(ref num) = t.number {
                            if let Ok(n) = num.parse::<i32>() {
                                track_number = Some(n);
                                disc_number = Some((m_idx + 1) as i32);
                                break;
                            }
                        }
                    }
                }
                if track_number.is_some() {
                    break;
                }
            }
        }
    }
    
    let genre = recording.tags.as_ref()
        .and_then(|tags| {
            tags.iter()
                .max_by_key(|t| t.count)
                .map(|t| t.name.clone())
        });

    Some(MusicBrainzMatch {
        title,
        artist,
        album,
        track_number,
        disc_number,
        genre,
        release_id,
        recording_id: recording.id.clone(),
    })
}

pub async fn get_scan_status(
    _claims: Claims,
    State(state): State<AppState>,
) -> Json<LibraryStatusResponse> {
    let status = state.scan_status.lock().unwrap();
    Json(LibraryStatusResponse {
        is_scanning: status.is_scanning,
        files_scanned: status.files_scanned,
        total_files: status.total_files,
        current_file: status.current_file.clone(),
    })
}

pub async fn get_fetch_status(
    _claims: Claims,
    State(state): State<AppState>,
) -> Json<FetchStatusResponse> {
    let status = state.fetcher_status.lock().unwrap();
    Json(FetchStatusResponse {
        is_running: status.is_running,
        tracks_processed: status.tracks_processed,
        total_tracks: status.total_tracks,
        current_track: status.current_track.clone(),
        logs: status.logs.clone(),
    })
}

pub async fn start_scan(
    _claims: Claims,
    State(state): State<AppState>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut status = state.scan_status.lock().unwrap();
    if status.is_scanning {
        return Err((StatusCode::CONFLICT, "Scan already in progress".to_string()));
    }

    status.is_scanning = true;
    status.files_scanned = 0;
    status.total_files = 0;
    status.current_file = None;

    let pool = state.pool.clone();
    let config = state.config.clone();
    let event_bus = state.event_bus.clone();
    let scan_status = state.scan_status.clone();

    tokio::spawn(async move {
        info!("Starting background music directory scan...");
        let music_dir = config.music_dir();
        
        let mut audio_files = Vec::new();
        collect_audio_files(&music_dir, &mut audio_files);
        
        {
            let mut status = scan_status.lock().unwrap();
            status.total_files = audio_files.len();
        }

        broadcast_library_event(&event_bus, "scan.started", serde_json::json!({
            "total_files": audio_files.len()
        }));

        for (index, file_path) in audio_files.iter().enumerate() {
            let relative_path_res = file_path.strip_prefix(&config.data_dir);
            if relative_path_res.is_err() {
                continue;
            }
            let relative_path = relative_path_res.unwrap().to_string_lossy().to_string();

            // Update scan status
            {
                let mut status = scan_status.lock().unwrap();
                status.files_scanned = index + 1;
                status.current_file = Some(file_path.file_name().unwrap_or_default().to_string_lossy().to_string());
            }

            // Periodically broadcast progress
            if (index + 1) % 5 == 0 || index + 1 == audio_files.len() {
                let status_clone = {
                    let status = scan_status.lock().unwrap();
                    status.clone()
                };
                broadcast_library_event(&event_bus, "scan.progress", serde_json::to_value(&status_clone).unwrap_or(serde_json::Value::Null));
            }

            // Check if track is already in database
            let path_exists = sqlx::query("SELECT id FROM tracks WHERE path = ?")
                .bind(&relative_path)
                .fetch_optional(&pool)
                .await
                .map(|o| o.is_some())
                .unwrap_or(false);

            if path_exists {
                continue;
            }

            if let Some(mut metadata) = extract_metadata(&file_path.to_string_lossy()) {
                let mut is_duplicate = false;
                if let Some(ref hash) = metadata.content_hash {
                    let hash_exists = sqlx::query("SELECT id FROM tracks WHERE content_hash = ?")
                        .bind(hash)
                        .fetch_optional(&pool)
                        .await
                        .map(|o| o.is_some())
                        .unwrap_or(false);
                    if hash_exists {
                        is_duplicate = true;
                    }
                }

                if is_duplicate {
                    continue;
                }

                let album_id = if let Some(album_name) = &metadata.album {
                    let existing_album = sqlx::query("SELECT id FROM albums WHERE name = ?")
                        .bind(album_name)
                        .fetch_optional(&pool)
                        .await
                        .unwrap_or(None);

                    if let Some(alb) = existing_album {
                        Some(alb.get::<i64, _>("id"))
                    } else {
                        let res = sqlx::query("INSERT INTO albums (name, artist) VALUES (?, ?)")
                            .bind(album_name)
                            .bind(&metadata.artist)
                            .execute(&pool)
                            .await;
                        
                        match res {
                            Ok(r) => Some(r.last_insert_rowid()),
                            Err(_) => None,
                        }
                    }
                } else {
                    None
                };

                let file_size = std::fs::metadata(file_path).map(|m| m.len() as i64).unwrap_or(0);

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
                .bind(file_size)
                .execute(&pool)
                .await;

                if let Ok(r) = res {
                    let track_id = r.last_insert_rowid();
                    let artwork_dir = config.artwork_dir();
                    std::fs::create_dir_all(&artwork_dir).ok();

                    if let Some(cover_data) = &metadata.track_cover {
                        let relative_cover = format!("artwork/track_{}.jpg", track_id);
                        let cover_full = config.data_dir.join(&relative_cover);
                        if std::fs::write(&cover_full, cover_data).is_ok() {
                            sqlx::query("UPDATE tracks SET track_cover_path = ? WHERE id = ?")
                                .bind(&relative_cover)
                                .bind(track_id)
                                .execute(&pool)
                                .await
                                .ok();
                        }
                    }

                    if let Some(alb_id) = album_id {
                        if let Some(art_data) = &metadata.album_art {
                            let has_art = sqlx::query("SELECT art_path FROM albums WHERE id = ? AND art_path IS NOT NULL")
                                .bind(alb_id)
                                .fetch_optional(&pool)
                                .await
                                .map(|o| o.is_some())
                                .unwrap_or(false);

                            if !has_art {
                                let relative_art = format!("artwork/album_{}.jpg", alb_id);
                                let art_full = config.data_dir.join(&relative_art);
                                if std::fs::write(&art_full, art_data).is_ok() {
                                    sqlx::query("UPDATE albums SET art_path = ? WHERE id = ?")
                                        .bind(&relative_art)
                                        .bind(alb_id)
                                        .execute(&pool)
                                        .await
                                        .ok();
                                }
                            }
                        }
                    }

                    if let Ok(track) = sqlx::query_as::<_, TrackResponse>(
                        "SELECT id, path, title, artist, album, track_number, disc_number, duration,
                                album_id, format, bitrate, source_type, cover_url, external_id,
                                local_src, track_cover_path, genre, metadata_json, date_added
                         FROM tracks
                         WHERE id = ?"
                    )
                    .bind(track_id)
                    .fetch_one(&pool)
                    .await {
                        let payload = serde_json::to_value(&track).unwrap_or(serde_json::Value::Null);
                        broadcast_library_event(&event_bus, "track.added", payload);
                    }
                }
            }
        }

        {
            let mut status = scan_status.lock().unwrap();
            status.is_scanning = false;
        }

        let final_status = {
            let status = scan_status.lock().unwrap();
            status.clone()
        };

        broadcast_library_event(&event_bus, "scan.completed", serde_json::to_value(&final_status).unwrap_or(serde_json::Value::Null));
        info!("Background music directory scan completed.");
    });

    Ok(StatusCode::ACCEPTED)
}

pub async fn start_metadata_fetcher(
    _claims: Claims,
    State(state): State<AppState>,
    payload: Option<Json<FetcherRequest>>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut status = state.fetcher_status.lock().unwrap();
    if status.is_running {
        return Err((StatusCode::CONFLICT, "Metadata fetcher already running".to_string()));
    }

    let provider = payload
        .and_then(|Json(p)| p.provider)
        .unwrap_or_else(|| "deezer".to_string());

    status.is_running = true;
    status.tracks_processed = 0;
    status.total_tracks = 0;
    status.current_track = None;
    status.logs = vec![format!("Worker started (Provider: {}). Analyzing database library...", provider)];

    let pool = state.pool.clone();
    let config = state.config.clone();
    let event_bus = state.event_bus.clone();
    let fetcher_status = state.fetcher_status.clone();

    let provider_clone = provider.clone();
    tokio::spawn(async move {
        // Query tracks missing metadata (unknown titles or missing artists/albums)
        let tracks_res = sqlx::query(
            "SELECT id, path, title, artist, album FROM tracks 
             WHERE artist IS NULL OR album IS NULL OR title IS NULL OR title = '' OR title LIKE 'music/%'"
        )
        .fetch_all(&pool)
        .await;

        let tracks = match tracks_res {
            Ok(t) => t,
            Err(e) => {
                let err_msg = format!("Database error while querying tracks: {}", e);
                log_fetcher_message(&fetcher_status, &event_bus, &err_msg);
                let mut status = fetcher_status.lock().unwrap();
                status.is_running = false;
                return;
            }
        };

        let total = tracks.len();
        {
            let mut status = fetcher_status.lock().unwrap();
            status.total_tracks = total;
        }

        let start_msg = format!("Found {} tracks needing metadata completion.", total);
        log_fetcher_message(&fetcher_status, &event_bus, &start_msg);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        for (idx, row) in tracks.iter().enumerate() {
            let track_id: i64 = row.get("id");
            let track_path: String = row.get("path");
            let current_title: Option<String> = row.get("title");

            let filename = Path::new(&track_path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            let target_title = current_title.filter(|t| !t.trim().is_empty() && !t.starts_with("music/")).unwrap_or(filename);
            let search_term = clean_search_term(&target_title);

            {
                let mut status = fetcher_status.lock().unwrap();
                status.tracks_processed = idx + 1;
                status.current_track = Some(search_term.clone());
            }

            let search_msg = format!("[{}/{}] Searching for: {}", idx + 1, total, search_term);
            log_fetcher_message(&fetcher_status, &event_bus, &search_msg);

            let provider_run = provider_clone.clone();

            if provider_run == "musicbrainz" {
                let mb_match_opt = fetch_musicbrainz_metadata(&client, &search_term).await;

                match mb_match_opt {
                    Some(mb_match) => {
                        let found_msg = format!(
                            "  -> Match found (MusicBrainz): \"{}\" by \"{}\" (Album: \"{}\")",
                            mb_match.title, mb_match.artist, mb_match.album
                        );
                        log_fetcher_message(&fetcher_status, &event_bus, &found_msg);

                        // Match album
                        let album_id = match_or_create_album(&pool, &mb_match.album, &mb_match.artist).await;

                        // Serialize full raw response as metadata_json
                        let metadata_json = serde_json::to_string(&mb_match).ok();

                        // Update track metadata in DB (including new fields: track_number, disc_number, genre, external_id, metadata_json)
                        let update_res = sqlx::query(
                            "UPDATE tracks SET title = ?, artist = ?, album = ?, album_id = ?, track_number = ?, disc_number = ?, genre = ?, external_id = ?, metadata_json = ? WHERE id = ?"
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
                        .bind(track_id)
                        .execute(&pool)
                        .await;

                        if update_res.is_ok() {
                            // Try downloading album artwork from Cover Art Archive
                            if let Some(ref release_id) = mb_match.release_id {
                                let cover_url = format!("https://coverartarchive.org/release/{}/front-500", release_id);
                                let c_resp_res = client.get(&cover_url)
                                    .header("User-Agent", "Audion/0.1.0 ( contact@audion.local )")
                                    .send()
                                    .await;

                                if let Ok(c_resp) = c_resp_res {
                                    if c_resp.status().is_success() {
                                        if let Ok(bytes) = c_resp.bytes().await {
                                            let relative_cover = format!("artwork/track_{}.jpg", track_id);
                                            let cover_full = config.data_dir.join(&relative_cover);
                                            std::fs::create_dir_all(cover_full.parent().unwrap()).ok();
                                            if std::fs::write(&cover_full, &bytes).is_ok() {
                                                sqlx::query("UPDATE tracks SET track_cover_path = ? WHERE id = ?")
                                                    .bind(&relative_cover)
                                                    .bind(track_id)
                                                    .execute(&pool)
                                                    .await
                                                    .ok();
                                            }

                                            if let Some(alb_id) = album_id {
                                                let has_art = sqlx::query("SELECT art_path FROM albums WHERE id = ? AND art_path IS NOT NULL")
                                                    .bind(alb_id)
                                                    .fetch_optional(&pool)
                                                    .await
                                                    .map(|o| o.is_some())
                                                    .unwrap_or(false);

                                                if !has_art {
                                                    let relative_art = format!("artwork/album_{}.jpg", alb_id);
                                                    let art_full = config.data_dir.join(&relative_art);
                                                    if std::fs::write(&art_full, &bytes).is_ok() {
                                                        sqlx::query("UPDATE albums SET art_path = ? WHERE id = ?")
                                                            .bind(&relative_art)
                                                            .bind(alb_id)
                                                            .execute(&pool)
                                                            .await
                                                            .ok();
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Try writing tags back to file asynchronously
                            let file_full_path = config.data_dir.join(&track_path);
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

                            // Broadcast track.updated
                            if let Ok(track) = sqlx::query_as::<_, TrackResponse>(
                                "SELECT id, path, title, artist, album, track_number, disc_number, duration,
                                        album_id, format, bitrate, source_type, cover_url, external_id,
                                        local_src, track_cover_path, genre, metadata_json, date_added
                                 FROM tracks
                                 WHERE id = ?"
                            )
                            .bind(track_id)
                            .fetch_one(&pool)
                            .await {
                                let payload = serde_json::to_value(&track).unwrap_or(serde_json::Value::Null);
                                broadcast_library_event(&event_bus, "track.updated", payload);
                            }
                        }
                    }
                    None => {
                        log_fetcher_message(&fetcher_status, &event_bus, "  -> No match found in MusicBrainz.");
                    }
                }
            } else {
                // Fetch from Deezer API
                let url = "https://api.deezer.com/search";
                let response = client.get(url)
                    .query(&[("q", &search_term)])
                    .send()
                    .await;

                match response {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            if let Ok(search_res) = resp.json::<DeezerSearchResponse>().await {
                                if !search_res.data.is_empty() {
                                    let match_track = &search_res.data[0];
                                    let found_msg = format!(
                                        "  -> Match found: \"{}\" by \"{}\" (Album: \"{}\")",
                                        match_track.title, match_track.artist.name, match_track.album.title
                                    );
                                    log_fetcher_message(&fetcher_status, &event_bus, &found_msg);

                                    // Match album
                                    let album_id = match_or_create_album(&pool, &match_track.album.title, &match_track.artist.name).await;

                                    let metadata_json = serde_json::to_value(&match_track).ok().map(|v| v.to_string());
                                    let ext_id = Some(match_track.id.to_string());

                                    // Update track metadata in DB
                                    let update_res = sqlx::query(
                                        "UPDATE tracks SET title = ?, artist = ?, album = ?, album_id = ?, external_id = ?, metadata_json = ? WHERE id = ?"
                                    )
                                    .bind(&match_track.title)
                                    .bind(&match_track.artist.name)
                                    .bind(&match_track.album.title)
                                    .bind(album_id)
                                    .bind(ext_id)
                                    .bind(metadata_json)
                                    .bind(track_id)
                                    .execute(&pool)
                                    .await;

                                    if update_res.is_ok() {
                                        // Try downloading album artwork
                                        if let Some(ref cover_url) = match_track.album.cover_big {
                                            if let Ok(c_resp) = client.get(cover_url).send().await {
                                                if c_resp.status().is_success() {
                                                    if let Ok(bytes) = c_resp.bytes().await {
                                                        let relative_cover = format!("artwork/track_{}.jpg", track_id);
                                                        let cover_full = config.data_dir.join(&relative_cover);
                                                        std::fs::create_dir_all(cover_full.parent().unwrap()).ok();
                                                        if std::fs::write(&cover_full, &bytes).is_ok() {
                                                            sqlx::query("UPDATE tracks SET track_cover_path = ? WHERE id = ?")
                                                                .bind(&relative_cover)
                                                                .bind(track_id)
                                                                .execute(&pool)
                                                                .await
                                                                .ok();
                                                        }

                                                        if let Some(alb_id) = album_id {
                                                            let has_art = sqlx::query("SELECT art_path FROM albums WHERE id = ? AND art_path IS NOT NULL")
                                                                .bind(alb_id)
                                                                .fetch_optional(&pool)
                                                                .await
                                                                .map(|o| o.is_some())
                                                                .unwrap_or(false);

                                                            if !has_art {
                                                                let relative_art = format!("artwork/album_{}.jpg", alb_id);
                                                                let art_full = config.data_dir.join(&relative_art);
                                                                if std::fs::write(&art_full, &bytes).is_ok() {
                                                                    sqlx::query("UPDATE albums SET art_path = ? WHERE id = ?")
                                                                        .bind(&relative_art)
                                                                        .bind(alb_id)
                                                                        .execute(&pool)
                                                                        .await
                                                                        .ok();
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // Try writing tags back to file asynchronously
                                        let file_full_path = config.data_dir.join(&track_path);
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

                                        // Broadcast track.updated
                                        if let Ok(track) = sqlx::query_as::<_, TrackResponse>(
                                            "SELECT id, path, title, artist, album, track_number, disc_number, duration,
                                                    album_id, format, bitrate, source_type, cover_url, external_id,
                                                    local_src, track_cover_path, genre, metadata_json, date_added
                                             FROM tracks
                                             WHERE id = ?"
                                        )
                                        .bind(track_id)
                                        .fetch_one(&pool)
                                        .await {
                                            let payload = serde_json::to_value(&track).unwrap_or(serde_json::Value::Null);
                                            broadcast_library_event(&event_bus, "track.updated", payload);
                                        }
                                    }
                                } else {
                                    log_fetcher_message(&fetcher_status, &event_bus, "  -> No match found.");
                                }
                            }
                        } else {
                            let err_log = format!("  -> API returned status error: {}", resp.status());
                            log_fetcher_message(&fetcher_status, &event_bus, &err_log);
                        }
                    }
                    Err(err) => {
                        let err_log = format!("  -> API request failed: {}", err);
                        log_fetcher_message(&fetcher_status, &event_bus, &err_log);
                    }
                }
            }

            // Throttle requests slightly based on provider
            let sleep_ms = if provider_run == "musicbrainz" { 1000 } else { 500 };
            tokio::time::sleep(std::time::Duration::from_millis(sleep_ms)).await;
        }

        {
            let mut status = fetcher_status.lock().unwrap();
            status.is_running = false;
        }
        log_fetcher_message(&fetcher_status, &event_bus, "Worker complete. All tracks processed.");
        
        let final_status = {
            let status = fetcher_status.lock().unwrap();
            status.clone()
        };
        broadcast_library_event(&event_bus, "fetch.completed", serde_json::to_value(&final_status).unwrap_or(serde_json::Value::Null));
    });

    Ok(StatusCode::ACCEPTED)
}

pub async fn clean_library(
    _claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    info!("Pruning orphan records from database library...");
    let tracks = sqlx::query("SELECT id, path, track_cover_path FROM tracks")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut pruned_count = 0;

    for row in tracks {
        let id: i64 = row.get("id");
        let path_val: String = row.get("path");
        let cover_path_val: Option<String> = row.get("track_cover_path");

        let full_track_path = state.config.data_dir.join(&path_val);
        if !full_track_path.exists() {
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

            sqlx::query("DELETE FROM tracks WHERE id = ?")
                .bind(id)
                .execute(&state.pool)
                .await
                .ok();

            let payload = serde_json::json!({ "id": id });
            let payload_str = payload.to_string();
            let event_type = "track.deleted";

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
                    payload,
                    created_at: chrono::Utc::now().to_rfc3339(),
                });
            }

            pruned_count += 1;
        }
    }

    let prune_albums_res = sqlx::query(
        "DELETE FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL)"
    )
    .execute(&state.pool)
    .await;

    if let Ok(r) = prune_albums_res {
        info!("Pruned {} empty albums.", r.rows_affected());
    }

    info!("Pruned {} orphan tracks from library database.", pruned_count);

    Ok(Json(serde_json::json!({
        "status": "success",
        "pruned_count": pruned_count
    })))
}

pub async fn reset_library(
    _claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    info!("Resetting library database...");

    sqlx::query("DELETE FROM tracks").execute(&state.pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query("DELETE FROM albums").execute(&state.pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query("DELETE FROM playlists").execute(&state.pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query("DELETE FROM liked_tracks").execute(&state.pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let artwork_dir = state.config.artwork_dir();
    if artwork_dir.exists() {
        let _ = std::fs::remove_dir_all(&artwork_dir);
        let _ = std::fs::create_dir_all(&artwork_dir);
    }
    
    let transcoded_dir = state.config.data_dir.join("transcoded");
    if transcoded_dir.exists() {
        let _ = std::fs::remove_dir_all(&transcoded_dir);
    }

    broadcast_library_event(&state.event_bus, "library.reset", serde_json::json!({}));

    info!("Library database reset complete.");
    Ok(Json(serde_json::json!({
        "status": "success",
        "message": "Library has been completely reset."
    })))
}

fn collect_audio_files(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_audio_files(&path, files);
            } else if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    match ext.to_lowercase().as_str() {
                        "mp3" | "flac" | "m4a" | "m4b" | "ogg" | "wav" | "alac" | "aac" => {
                            files.push(path);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn broadcast_library_event(event_bus: &crate::events::EventBus, event_type: &str, payload: serde_json::Value) {
    let time_now = chrono::Utc::now().to_rfc3339();
    event_bus.broadcast(ServerEvent {
        id: 0,
        event_type: event_type.to_string(),
        payload,
        created_at: time_now,
    });
}

fn log_fetcher_message(fetcher_status: &Arc<std::sync::Mutex<FetcherStatus>>, event_bus: &crate::events::EventBus, message: &str) {
    let mut status = fetcher_status.lock().unwrap();
    status.logs.push(message.to_string());
    if status.logs.len() > 30 {
        status.logs.remove(0);
    }

    let payload = serde_json::json!({
        "is_running": status.is_running,
        "tracks_processed": status.tracks_processed,
        "total_tracks": status.total_tracks,
        "current_track": status.current_track.clone(),
        "logs": status.logs.clone(),
    });
    
    broadcast_library_event(event_bus, "fetch.progress", payload);
}

fn clean_search_term(input: &str) -> String {
    let mut clean = input.replace(".mp3", "")
        .replace(".flac", "")
        .replace(".m4a", "")
        .replace(".ogg", "")
        .replace(".wav", "")
        .replace(".alac", "")
        .replace(".aac", "")
        .replace('_', " ")
        .replace('-', " ");
    
    let parts: Vec<&str> = clean.split_whitespace().collect();
    if !parts.is_empty() {
        let first = parts[0];
        let is_num = first.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-');
        if is_num && parts.len() > 1 {
            clean = parts[1..].join(" ");
        }
    }
    
    clean.trim().to_string()
}

async fn match_or_create_album(pool: &sqlx::SqlitePool, name: &str, artist: &str) -> Option<i64> {
    if name.trim().is_empty() {
        return None;
    }

    let existing = sqlx::query("SELECT id FROM albums WHERE name = ?")
        .bind(name)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

    if let Some(alb) = existing {
        Some(alb.get::<i64, _>("id"))
    } else {
        let res = sqlx::query("INSERT INTO albums (name, artist) VALUES (?, ?)")
            .bind(name)
            .bind(artist)
            .execute(pool)
            .await;
        
        match res {
            Ok(r) => Some(r.last_insert_rowid()),
            Err(_) => None,
        }
    }
}
