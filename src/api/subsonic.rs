use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use sqlx::Row;

use crate::state::AppState;

#[derive(sqlx::FromRow, Debug)]
struct DbUser {
    id: String,
    username: String,
    password_hash: String,
    role: String,
    listenbrainz_token: Option<String>,
    is_enabled: i32,
    subsonic_password: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct SubsonicParams {
    pub u: Option<String>, // username
    pub p: Option<String>, // password (plain text or enc:hex)
    pub t: Option<String>, // md5 token
    pub s: Option<String>, // salt
    pub f: Option<String>, // format (xml or json)
    pub v: Option<String>, // version
    pub c: Option<String>, // client name
    pub id: Option<String>, // item ID (for directory / song / stream)
    pub maxBitRate: Option<i32>, // max bitrate for streaming
    pub format: Option<String>, // target format for streaming
    pub submission: Option<bool>, // for scrobble
}

// Subsonic Error Codes
const ERROR_GENERIC: i32 = 0;
const ERROR_MISSING_PARAM: i32 = 10;
const ERROR_AUTH: i32 = 40;

// Helper to write subsonic error response
fn subsonic_error(f: &str, code: i32, msg: &str) -> Response {
    if f == "json" {
        let body = serde_json::json!({
            "subsonic-response": {
                "status": "failed",
                "version": "1.16.1",
                "error": {
                    "code": code,
                    "message": msg
                }
            }
        });
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            Json(body),
        ).into_response()
    } else {
        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response xmlns="http://subsonic.org/restapi" status="failed" version="1.16.1">
    <error code="{}" message="{}"/>
</subsonic-response>"#,
            code, msg
        );
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
            body,
        ).into_response()
    }
}

// Authenticate subsonic requests
async fn authenticate(state: &AppState, params: &SubsonicParams) -> Result<DbUser, String> {
    let username = params.u.as_deref().ok_or_else(|| "Missing username 'u'".to_string())?;

    let user = sqlx::query_as::<_, DbUser>(
        "SELECT id, username, password_hash, role, listenbrainz_token, is_enabled, subsonic_password FROM users WHERE username = ?"
    )
    .bind(username)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "User not found".to_string())?;

    if user.is_enabled == 0 {
        return Err("Account is disabled".to_string());
    }

    // Check role limits: StreamOnly and User are allowed to stream and read.
    // If we have granular roles, they are all allowed to use subsonic client for playback.
    
    // Check plain password parameter 'p'
    if let Some(p) = &params.p {
        let mut actual_password = p.clone();
        if actual_password.starts_with("enc:") {
            if let Ok(bytes) = hex::decode(&actual_password[4..]) {
                if let Ok(decoded) = String::from_utf8(bytes) {
                    actual_password = decoded;
                }
            }
        }

        if let Some(ref db_pwd) = user.subsonic_password {
            let decrypted_pwd = crate::auth::decrypt_subsonic_password(db_pwd, &state.config.jwt_secret)
                .unwrap_or_else(|_| db_pwd.clone());
            if decrypted_pwd == actual_password {
                return Ok(user);
            }
        }

        if crate::auth::verify_password(&actual_password, &user.password_hash) {
            return Ok(user);
        }

        return Err("Invalid credentials".to_string());
    }

    // Check token 't' and salt 's'
    if let (Some(token), Some(salt)) = (&params.t, &params.s) {
        if let Some(ref db_pwd) = user.subsonic_password {
            let decrypted_pwd = crate::auth::decrypt_subsonic_password(db_pwd, &state.config.jwt_secret)
                .unwrap_or_else(|_| db_pwd.clone());
            let combined = format!("{}{}", decrypted_pwd, salt);
            let computed = format!("{:x}", md5::compute(combined));
            if &computed == token {
                return Ok(user);
            }
        }
        return Err("Invalid token credentials".to_string());
    }


    Err("Missing authentication credentials".to_string())
}

// GET/POST /rest/ping.view
pub async fn ping(
    State(state): State<AppState>,
    Query(params): Query<SubsonicParams>,
) -> Response {
    let f = params.f.as_deref().unwrap_or("xml");
    if let Err(e) = authenticate(&state, &params).await {
        return subsonic_error(f, ERROR_AUTH, &e);
    }

    if f == "json" {
        let body = serde_json::json!({
            "subsonic-response": {
                "status": "ok",
                "version": "1.16.1"
            }
        });
        (StatusCode::OK, [(header::CONTENT_TYPE, "application/json")], Json(body)).into_response()
    } else {
        let body = r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response xmlns="http://subsonic.org/restapi" status="ok" version="1.16.1"/>"#;
        (StatusCode::OK, [(header::CONTENT_TYPE, "application/xml; charset=utf-8")], body).into_response()
    }
}

// GET/POST /rest/getLicense.view
pub async fn get_license(
    State(state): State<AppState>,
    Query(params): Query<SubsonicParams>,
) -> Response {
    let f = params.f.as_deref().unwrap_or("xml");
    if let Err(e) = authenticate(&state, &params).await {
        return subsonic_error(f, ERROR_AUTH, &e);
    }

    if f == "json" {
        let body = serde_json::json!({
            "subsonic-response": {
                "status": "ok",
                "version": "1.16.1",
                "license": {
                    "valid": true,
                    "email": "user@audion.local",
                    "key": "audion-license-key",
                    "date": "2099-01-01T00:00:00Z"
                }
            }
        });
        (StatusCode::OK, [(header::CONTENT_TYPE, "application/json")], Json(body)).into_response()
    } else {
        let body = r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response xmlns="http://subsonic.org/restapi" status="ok" version="1.16.1">
    <license valid="true" email="user@audion.local" key="audion-license-key" date="2099-01-01T00:00:00Z"/>
</subsonic-response>"#;
        (StatusCode::OK, [(header::CONTENT_TYPE, "application/xml; charset=utf-8")], body).into_response()
    }
}

// GET/POST /rest/getMusicFolders.view
pub async fn get_music_folders(
    State(state): State<AppState>,
    Query(params): Query<SubsonicParams>,
) -> Response {
    let f = params.f.as_deref().unwrap_or("xml");
    if let Err(e) = authenticate(&state, &params).await {
        return subsonic_error(f, ERROR_AUTH, &e);
    }

    if f == "json" {
        let body = serde_json::json!({
            "subsonic-response": {
                "status": "ok",
                "version": "1.16.1",
                "musicFolders": {
                    "musicFolder": [
                        { "id": 1, "name": "Music Library" }
                    ]
                }
            }
        });
        (StatusCode::OK, [(header::CONTENT_TYPE, "application/json")], Json(body)).into_response()
    } else {
        let body = r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response xmlns="http://subsonic.org/restapi" status="ok" version="1.16.1">
    <musicFolders>
        <musicFolder id="1" name="Music Library"/>
    </musicFolders>
</subsonic-response>"#;
        (StatusCode::OK, [(header::CONTENT_TYPE, "application/xml; charset=utf-8")], body).into_response()
    }
}

// GET/POST /rest/getIndexes.view
pub async fn get_indexes(
    State(state): State<AppState>,
    Query(params): Query<SubsonicParams>,
) -> Response {
    let f = params.f.as_deref().unwrap_or("xml");
    let user = match authenticate(&state, &params).await {
        Ok(u) => u,
        Err(e) => return subsonic_error(f, ERROR_AUTH, &e),
    };
    let user_pool = match state.get_user_pool(&user.id).await {
        Ok(p) => p,
        Err(e) => return subsonic_error(f, ERROR_GENERIC, &format!("User database error: {}", e)),
    };

    // Fetch distinct artist names from tracks
    let artists_res = sqlx::query("SELECT DISTINCT artist FROM tracks WHERE artist IS NOT NULL AND artist != '' ORDER BY artist ASC")
        .fetch_all(&user_pool)
        .await;

    let artist_rows = match artists_res {
        Ok(rows) => rows,
        Err(_) => return subsonic_error(f, ERROR_GENERIC, "Database query error"),
    };

    // Group artists by first uppercase letter
    let mut groups: HashMap<char, Vec<String>> = HashMap::new();
    for row in artist_rows {
        let name: String = row.get("artist");
        if let Some(first_char) = name.trim().chars().next() {
            let key = first_char.to_uppercase().next().unwrap_or('A');
            groups.entry(key).or_insert_with(Vec::new).push(name);
        }
    }

    // Sort group keys
    let mut sorted_keys: Vec<char> = groups.keys().cloned().collect();
    sorted_keys.sort();

    if f == "json" {
        let mut index_list = Vec::new();
        for key in sorted_keys {
            let mut artists = Vec::new();
            if let Some(names) = groups.get(&key) {
                for name in names {
                    let id = format!("art_{}", hex::encode(name));
                    artists.push(serde_json::json!({
                        "id": id,
                        "name": name
                    }));
                }
            }
            index_list.push(serde_json::json!({
                "name": key.to_string(),
                "artist": artists
            }));
        }

        let body = serde_json::json!({
            "subsonic-response": {
                "status": "ok",
                "version": "1.16.1",
                "indexes": {
                    "lastModified": 1700000000000i64,
                    "index": index_list
                }
            }
        });
        (StatusCode::OK, [(header::CONTENT_TYPE, "application/json")], Json(body)).into_response()
    } else {
        let mut xml_indexes = String::new();
        for key in sorted_keys {
            xml_indexes.push_str(&format!(r#"<index name="{}">"#, key));
            if let Some(names) = groups.get(&key) {
                for name in names {
                    let id = format!("art_{}", hex::encode(name));
                    xml_indexes.push_str(&format!(
                        r#"<artist id="{}" name="{}"/>"#,
                        id, escape_xml(name)
                    ));
                }
            }
            xml_indexes.push_str("</index>");
        }

        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response xmlns="http://subsonic.org/restapi" status="ok" version="1.16.1">
    <indexes lastModified="1700000000000">
        {}
    </indexes>
</subsonic-response>"#,
            xml_indexes
        );
        (StatusCode::OK, [(header::CONTENT_TYPE, "application/xml; charset=utf-8")], body).into_response()
    }
}

// GET/POST /rest/getMusicDirectory.view
pub async fn get_music_directory(
    State(state): State<AppState>,
    Query(params): Query<SubsonicParams>,
) -> Response {
    let f = params.f.as_deref().unwrap_or("xml");
    let user = match authenticate(&state, &params).await {
        Ok(u) => u,
        Err(e) => return subsonic_error(f, ERROR_AUTH, &e),
    };
    let user_pool = match state.get_user_pool(&user.id).await {
        Ok(p) => p,
        Err(e) => return subsonic_error(f, ERROR_GENERIC, &format!("User database error: {}", e)),
    };

    let directory_id = match &params.id {
        Some(id) => id,
        None => return subsonic_error(f, ERROR_MISSING_PARAM, "Missing directory parameter 'id'"),
    };

    if directory_id.starts_with("art_") {
        // ID is an artist. List their albums.
        let artist_hex = &directory_id[4..];
        let artist_bytes = match hex::decode(artist_hex) {
            Ok(b) => b,
            Err(_) => return subsonic_error(f, ERROR_GENERIC, "Invalid artist ID"),
        };
        let artist_name = match String::from_utf8(artist_bytes) {
            Ok(s) => s,
            Err(_) => return subsonic_error(f, ERROR_GENERIC, "Invalid artist name UTF8"),
        };

        // Fetch distinct albums for this artist
        let albums_res = sqlx::query(
            "SELECT DISTINCT album_id, album FROM tracks WHERE artist = ? AND album_id IS NOT NULL ORDER BY album ASC"
        )
        .bind(&artist_name)
        .fetch_all(&user_pool)
        .await;

        let album_rows = match albums_res {
            Ok(rows) => rows,
            Err(_) => return subsonic_error(f, ERROR_GENERIC, "Database query error"),
        };

        if f == "json" {
            let mut children = Vec::new();
            for r in album_rows {
                let alb_id: i64 = r.get("album_id");
                let alb_title: String = r.get("album");
                children.push(serde_json::json!({
                    "id": format!("alb_{}", alb_id),
                    "parent": directory_id,
                    "title": alb_title,
                    "artist": artist_name,
                    "isDir": true
                }));
            }

            let body = serde_json::json!({
                "subsonic-response": {
                    "status": "ok",
                    "version": "1.16.1",
                    "directory": {
                        "id": directory_id,
                        "name": artist_name,
                        "child": children
                    }
                }
            });
            (StatusCode::OK, [(header::CONTENT_TYPE, "application/json")], Json(body)).into_response()
        } else {
            let mut xml_children = String::new();
            for r in album_rows {
                let alb_id: i64 = r.get("album_id");
                let alb_title: String = r.get("album");
                xml_children.push_str(&format!(
                    r#"<child id="alb_{}" parent="{}" title="{}" artist="{}" isDir="true"/>"#,
                    alb_id, directory_id, escape_xml(&alb_title), escape_xml(&artist_name)
                ));
            }

            let body = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response xmlns="http://subsonic.org/restapi" status="ok" version="1.16.1">
    <directory id="{}" name="{}">
        {}
    </directory>
</subsonic-response>"#,
                directory_id, escape_xml(&artist_name), xml_children
            );
            (StatusCode::OK, [(header::CONTENT_TYPE, "application/xml; charset=utf-8")], body).into_response()
        }
    } else if directory_id.starts_with("alb_") {
        // ID is an album. List its tracks.
        let album_id_str = &directory_id[4..];
        let album_id = match album_id_str.parse::<i64>() {
            Ok(id) => id,
            Err(_) => return subsonic_error(f, ERROR_GENERIC, "Invalid album ID"),
        };

        // Query album name
        let album_name: String = match sqlx::query_scalar("SELECT name FROM albums WHERE id = ?")
            .bind(album_id)
            .fetch_optional(&user_pool)
            .await
        {
            Ok(Some(n)) => n,
            _ => "Unknown Album".to_string(),
        };

        // Query tracks for this album
        let tracks_res = sqlx::query(
            "SELECT id, title, artist, album, track_number, duration, size, format, bitrate FROM tracks WHERE album_id = ? ORDER BY disc_number, track_number"
        )
        .bind(album_id)
        .fetch_all(&user_pool)
        .await;

        let track_rows = match tracks_res {
            Ok(rows) => rows,
            Err(_) => return subsonic_error(f, ERROR_GENERIC, "Database query error"),
        };

        if f == "json" {
            let mut children = Vec::new();
            for r in track_rows {
                let tr_id: i64 = r.get("id");
                let title: String = r.get("title");
                let artist: String = r.get("artist");
                let album: String = r.get("album");
                let tr_num: Option<i32> = r.get("track_number");
                let duration: Option<i32> = r.get("duration");
                let format: Option<String> = r.get("format");
                let format_str = format.as_deref().unwrap_or("mp3");
                let bitrate: Option<i32> = r.get("bitrate");
                let size: Option<i64> = r.get("size");

                children.push(serde_json::json!({
                    "id": format!("tr_{}", tr_id),
                    "parent": directory_id,
                    "title": title,
                    "artist": artist,
                    "album": album,
                    "isDir": false,
                    "track": tr_num.unwrap_or(0),
                    "duration": duration.unwrap_or(0),
                    "size": size.unwrap_or(0),
                    "suffix": format_str,
                    "bitRate": bitrate.unwrap_or(320000) / 1000,
                    "contentType": mime_guess::from_path(format_str).first_or_octet_stream().to_string()
                }));
            }

            let body = serde_json::json!({
                "subsonic-response": {
                    "status": "ok",
                    "version": "1.16.1",
                    "directory": {
                        "id": directory_id,
                        "name": album_name,
                        "child": children
                    }
                }
            });
            (StatusCode::OK, [(header::CONTENT_TYPE, "application/json")], Json(body)).into_response()
        } else {
            let mut xml_children = String::new();
            for r in track_rows {
                let tr_id: i64 = r.get("id");
                let title: String = r.get("title");
                let artist: String = r.get("artist");
                let album: String = r.get("album");
                let tr_num: Option<i32> = r.get("track_number");
                let duration: Option<i32> = r.get("duration");
                let format: Option<String> = r.get("format");
                let format_str = format.as_deref().unwrap_or("mp3");
                let bitrate: Option<i32> = r.get("bitrate");
                let size: Option<i64> = r.get("size");

                xml_children.push_str(&format!(
                    r#"<child id="tr_{}" parent="{}" title="{}" artist="{}" album="{}" isDir="false" track="{}" duration="{}" size="{}" suffix="{}" bitRate="{}" contentType="{}"/>"#,
                    tr_id,
                    directory_id,
                    escape_xml(&title),
                    escape_xml(&artist),
                    escape_xml(&album),
                    tr_num.unwrap_or(0),
                    duration.unwrap_or(0),
                    size.unwrap_or(0),
                    format_str,
                    bitrate.unwrap_or(320000) / 1000,
                    mime_guess::from_path(format_str).first_or_octet_stream().to_string()
                ));
            }

            let body = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response xmlns="http://subsonic.org/restapi" status="ok" version="1.16.1">
    <directory id="{}" name="{}">
        {}
    </directory>
</subsonic-response>"#,
                directory_id, escape_xml(&album_name), xml_children
            );
            (StatusCode::OK, [(header::CONTENT_TYPE, "application/xml; charset=utf-8")], body).into_response()
        }
    } else {
        subsonic_error(f, ERROR_GENERIC, "Unsupported directory ID format")
    }
}

// GET/POST /rest/getSong.view
pub async fn get_song(
    State(state): State<AppState>,
    Query(params): Query<SubsonicParams>,
) -> Response {
    let f = params.f.as_deref().unwrap_or("xml");
    let user = match authenticate(&state, &params).await {
        Ok(u) => u,
        Err(e) => return subsonic_error(f, ERROR_AUTH, &e),
    };
    let user_pool = match state.get_user_pool(&user.id).await {
        Ok(p) => p,
        Err(e) => return subsonic_error(f, ERROR_GENERIC, &format!("User database error: {}", e)),
    };

    let song_id_str = match &params.id {
        Some(id) if id.starts_with("tr_") => &id[3..],
        _ => return subsonic_error(f, ERROR_MISSING_PARAM, "Missing track parameter 'id'"),
    };

    let song_id = match song_id_str.parse::<i64>() {
        Ok(id) => id,
        Err(_) => return subsonic_error(f, ERROR_GENERIC, "Invalid song ID"),
    };

    // Query track
    let track_res = sqlx::query(
        "SELECT id, title, artist, album, album_id, track_number, duration, size, format, bitrate FROM tracks WHERE id = ?"
    )
    .bind(song_id)
    .fetch_optional(&user_pool)
    .await;

    let track_row = match track_res {
        Ok(Some(row)) => row,
        _ => return subsonic_error(f, ERROR_GENERIC, "Song not found"),
    };

    let title: String = track_row.get("title");
    let artist: String = track_row.get("artist");
    let album: String = track_row.get("album");
    let alb_id: Option<i64> = track_row.get("album_id");
    let tr_num: Option<i32> = track_row.get("track_number");
    let duration: Option<i32> = track_row.get("duration");
    let size: Option<i64> = track_row.get("size");
    let format: Option<String> = track_row.get("format");
    let format_str = format.as_deref().unwrap_or("mp3");
    let bitrate: Option<i32> = track_row.get("bitrate");

    if f == "json" {
        let body = serde_json::json!({
            "subsonic-response": {
                "status": "ok",
                "version": "1.16.1",
                "song": {
                    "id": format!("tr_{}", song_id),
                    "parent": format!("alb_{}", alb_id.unwrap_or(0)),
                    "title": title,
                    "artist": artist,
                    "album": album,
                    "isDir": false,
                    "track": tr_num.unwrap_or(0),
                    "duration": duration.unwrap_or(0),
                    "size": size.unwrap_or(0),
                    "suffix": format_str,
                    "bitRate": bitrate.unwrap_or(320000) / 1000,
                    "contentType": mime_guess::from_path(format_str).first_or_octet_stream().to_string()
                }
            }
        });
        (StatusCode::OK, [(header::CONTENT_TYPE, "application/json")], Json(body)).into_response()
    } else {
        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response xmlns="http://subsonic.org/restapi" status="ok" version="1.16.1">
    <song id="tr_{}" parent="alb_{}" title="{}" artist="{}" album="{}" isDir="false" track="{}" duration="{}" size="{}" suffix="{}" bitRate="{}" contentType="{}"/>
</subsonic-response>"#,
            song_id,
            alb_id.unwrap_or(0),
            escape_xml(&title),
            escape_xml(&artist),
            escape_xml(&album),
            tr_num.unwrap_or(0),
            duration.unwrap_or(0),
            size.unwrap_or(0),
            format_str,
            bitrate.unwrap_or(320000) / 1000,
            mime_guess::from_path(format_str).first_or_octet_stream().to_string()
        );
        (StatusCode::OK, [(header::CONTENT_TYPE, "application/xml; charset=utf-8")], body).into_response()
    }
}

// GET/POST /rest/stream.view
pub async fn stream(
    State(state): State<AppState>,
    Query(params): Query<SubsonicParams>,
    headers: HeaderMap,
) -> Response {
    let f = params.f.as_deref().unwrap_or("xml");
    let user = match authenticate(&state, &params).await {
        Ok(u) => u,
        Err(e) => return subsonic_error(f, ERROR_AUTH, &e),
    };

    let song_id_str = match &params.id {
        Some(id) if id.starts_with("tr_") => &id[3..],
        _ => return subsonic_error(f, ERROR_MISSING_PARAM, "Missing track parameter 'id'"),
    };

    let song_id = match song_id_str.parse::<i64>() {
        Ok(id) => id,
        Err(_) => return subsonic_error(f, ERROR_GENERIC, "Invalid song ID"),
    };

    // Call our streaming API. To do this, we can construct mock Axum Claims and call the streaming module directly!
    let claims = crate::auth::Claims {
        sub: user.id,
        username: user.username,
        role: user.role,
        exp: 0,
    };

    // Forward to stream_track implementation with custom query params mapping
    crate::api::stream::stream_track_subsonic(
        claims,
        headers,
        &state,
        song_id,
        params.maxBitRate,
        params.format.as_deref(),
    ).await
}

// GET/POST /rest/scrobble.view
pub async fn scrobble(
    State(state): State<AppState>,
    Query(params): Query<SubsonicParams>,
) -> Response {
    let f = params.f.as_deref().unwrap_or("xml");
    let user = match authenticate(&state, &params).await {
        Ok(u) => u,
        Err(e) => return subsonic_error(f, ERROR_AUTH, &e),
    };
    let user_pool = match state.get_user_pool(&user.id).await {
        Ok(p) => p,
        Err(e) => return subsonic_error(f, ERROR_GENERIC, &format!("User database error: {}", e)),
    };

    let song_id_str = match &params.id {
        Some(id) if id.starts_with("tr_") => &id[3..],
        _ => return subsonic_error(f, ERROR_MISSING_PARAM, "Missing track parameter 'id'"),
    };

    let song_id = match song_id_str.parse::<i64>() {
        Ok(id) => id,
        Err(_) => return subsonic_error(f, ERROR_GENERIC, "Invalid song ID"),
    };

    // Record the play event if it's a submission scrobble
    if params.submission.unwrap_or(true) {
        let play_recorded = sqlx::query(
            "INSERT INTO play_log (user_id, track_id, duration_played, client) VALUES (?, ?, ?, ?)"
        )
        .bind(&user.id)
        .bind(song_id)
        .bind(0) // Duration info not strictly provided in standard scrobble.view, can default to 0
        .bind(params.c.as_deref().unwrap_or("Subsonic Client"))
        .execute(&user_pool)
        .await;

        if play_recorded.is_ok() {
            // Trigger automatic scrobbling to ListenBrainz if user token is configured!
            if let Some(ref token) = user.listenbrainz_token {
                let pool = user_pool.clone();
                let client_token = token.clone();
                tokio::spawn(async move {
                    if let Err(e) = crate::api::subsonic::scrobble_to_listenbrainz(&pool, song_id, &client_token).await {
                        tracing::error!("ListenBrainz scrobbling failed: {}", e);
                    }
                });
            }
        }
    }

    if f == "json" {
        let body = serde_json::json!({
            "subsonic-response": {
                "status": "ok",
                "version": "1.16.1"
            }
        });
        (StatusCode::OK, [(header::CONTENT_TYPE, "application/json")], Json(body)).into_response()
    } else {
        let body = r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response xmlns="http://subsonic.org/restapi" status="ok" version="1.16.1"/>"#;
        (StatusCode::OK, [(header::CONTENT_TYPE, "application/xml; charset=utf-8")], body).into_response()
    }
}

// Scrobble helper to ListenBrainz API
pub async fn scrobble_to_listenbrainz(
    pool: &sqlx::SqlitePool,
    track_id: i64,
    token: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Query track meta
    let track = sqlx::query(
        "SELECT title, artist, album FROM tracks WHERE id = ?"
    )
    .bind(track_id)
    .fetch_optional(pool)
    .await?
    .ok_or("Track not found")?;

    let title: String = track.get("title");
    let artist: String = track.get("artist");
    let album: String = track.get("album");

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "listen_type": "single",
        "payload": [
            {
                "listened_at": chrono::Utc::now().timestamp(),
                "track_metadata": {
                    "artist_name": artist,
                    "track_name": title,
                    "release_name": album
                }
            }
        ]
    });

    let res = client.post("https://api.listenbrainz.org/1/submit-listens")
        .header("Authorization", format!("Token {}", token))
        .json(&body)
        .send()
        .await?;

    if !res.status().is_success() {
        return Err(format!("ListenBrainz returned status: {}", res.status()).into());
    }

    tracing::info!("Successfully scrobbled track '{}' to ListenBrainz", title);
    Ok(())
}

// Simple XML string escape utility
fn escape_xml(s: &str) -> String {
    s.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
        .replace("'", "&apos;")
}
