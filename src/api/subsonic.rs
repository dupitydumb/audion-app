use axum::{
    extract::{Query, State, RawQuery},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
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

#[derive(Deserialize, Debug, Default)]
#[serde(default)]
#[allow(non_snake_case)]
pub struct SubsonicParams {
    pub u: Option<String>, // username
    pub p: Option<String>, // password (plain text or enc:hex)
    pub t: Option<String>, // md5 token
    pub s: Option<String>, // salt
    pub f: Option<String>, // format (xml or json)
    pub v: Option<String>, // version
    pub c: Option<String>, // client name
    pub id: Option<String>, // item ID
    pub musicFolderId: Option<String>,
    pub artistId: Option<String>,
    pub albumId: Option<String>,
    pub songId: Option<String>,
    pub playlistId: Option<String>,
    pub maxBitRate: Option<i32>,
    pub format: Option<String>,
    pub submission: Option<bool>,
    pub query: Option<String>,
    pub q: Option<String>,
    pub artistCount: Option<i32>,
    pub albumCount: Option<i32>,
    pub songCount: Option<i32>,
    pub count: Option<i32>,
    pub size: Option<i32>,
    pub offset: Option<i32>,
    pub genre: Option<String>,
    pub fromYear: Option<i32>,
    pub toYear: Option<i32>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    pub name: Option<String>,
    pub comment: Option<String>,
    pub public: Option<bool>,
    pub rating: Option<i32>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub trackNumber: Option<i32>,
    pub discNumber: Option<i32>,
    pub year: Option<String>,
    pub list_type: Option<String>,
}

// Subsonic Error Codes
const ERROR_GENERIC: i32 = 0;
const ERROR_MISSING_PARAM: i32 = 10;
const ERROR_AUTH: i32 = 40;
const ERROR_NOT_FOUND: i32 = 70;

fn format_artist_id(artist_name: &str) -> String {
    format!("art_{}", hex::encode(artist_name))
}

fn parse_artist_id(id: &str) -> Option<String> {
    if id.starts_with("art_") {
        if let Ok(bytes) = hex::decode(&id[4..]) {
            if let Ok(s) = String::from_utf8(bytes) {
                return Some(s);
            }
        }
    }
    None
}

fn format_album_id(id: i64) -> String {
    format!("alb_{}", id)
}

fn parse_album_id(id: &str) -> Option<i64> {
    if id.starts_with("alb_") {
        id[4..].parse::<i64>().ok()
    } else {
        id.parse::<i64>().ok()
    }
}

fn format_track_id(id: i64) -> String {
    format!("tr_{}", id)
}

fn parse_track_id(id: &str) -> Option<i64> {
    if id.starts_with("tr_") {
        id[3..].parse::<i64>().ok()
    } else {
        id.parse::<i64>().ok()
    }
}

fn format_playlist_id(id: i64) -> String {
    format!("pl_{}", id)
}

fn parse_playlist_id(id: &str) -> Option<i64> {
    if id.starts_with("pl_") {
        id[3..].parse::<i64>().ok()
    } else {
        id.parse::<i64>().ok()
    }
}

fn subsonic_response_json(data: serde_json::Value) -> Response {
    let mut resp = serde_json::json!({
        "subsonic-response": {
            "status": "ok",
            "version": "1.16.1",
            "type": "Audion",
            "serverVersion": env!("CARGO_PKG_VERSION"),
            "openSubsonic": true
        }
    });

    if let Some(obj) = resp.get_mut("subsonic-response").and_then(|v| v.as_object_mut()) {
        if let Some(data_obj) = data.as_object() {
            for (k, v) in data_obj {
                obj.insert(k.clone(), v.clone());
            }
        }
    }

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        Json(resp),
    ).into_response()
}

fn subsonic_response_xml(inner_xml: &str) -> Response {
    let body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response xmlns="http://subsonic.org/restapi" status="ok" version="1.16.1" type="Audion" serverVersion="{}" openSubsonic="true">
{}
</subsonic-response>"#,
        env!("CARGO_PKG_VERSION"),
        inner_xml
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        body,
    ).into_response()
}

fn subsonic_error(f: &str, code: i32, msg: &str) -> Response {
    if f == "json" {
        let body = serde_json::json!({
            "subsonic-response": {
                "status": "failed",
                "version": "1.16.1",
                "type": "Audion",
                "serverVersion": env!("CARGO_PKG_VERSION"),
                "openSubsonic": true,
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
<subsonic-response xmlns="http://subsonic.org/restapi" status="failed" version="1.16.1" type="Audion" serverVersion="{}" openSubsonic="true">
    <error code="{}" message="{}"/>
</subsonic-response>"#,
            env!("CARGO_PKG_VERSION"),
            code, escape_xml(msg)
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

#[derive(Debug)]
struct TrackRow {
    id: i64,
    title: String,
    artist: String,
    album: String,
    album_id: Option<i64>,
    track_number: Option<i32>,
    disc_number: Option<i32>,
    duration: Option<i32>,
    size: Option<i64>,
    format: Option<String>,
    bitrate: Option<i32>,
    path: String,
    genre: Option<String>,
    is_starred: bool,
}

fn row_to_track(r: &sqlx::sqlite::SqliteRow, liked_set: &std::collections::HashSet<i64>) -> TrackRow {
    let id: i64 = r.get("id");
    TrackRow {
        id,
        title: r.get("title"),
        artist: r.get("artist"),
        album: r.get("album"),
        album_id: r.get("album_id"),
        track_number: r.get("track_number"),
        disc_number: r.get("disc_number"),
        duration: r.get("duration"),
        size: r.get("size"),
        format: r.get("format"),
        bitrate: r.get("bitrate"),
        path: r.get("path"),
        genre: r.get("genre"),
        is_starred: liked_set.contains(&id),
    }
}

fn track_to_json(t: &TrackRow) -> serde_json::Value {
    let fmt = t.format.as_deref().unwrap_or("mp3");
    let mime = mime_guess::from_path(fmt).first_or_octet_stream().to_string();
    let parent = t.album_id.map(format_album_id).unwrap_or_else(|| "alb_0".to_string());
    let artist_id = format_artist_id(&t.artist);
    let album_id = t.album_id.map(format_album_id).unwrap_or_else(|| "alb_0".to_string());

    let mut json = serde_json::json!({
        "id": format_track_id(t.id),
        "parent": parent,
        "isDir": false,
        "title": t.title,
        "album": t.album,
        "artist": t.artist,
        "track": t.track_number.unwrap_or(0),
        "discNumber": t.disc_number.unwrap_or(1),
        "duration": t.duration.unwrap_or(0),
        "size": t.size.unwrap_or(0),
        "suffix": fmt,
        "bitRate": t.bitrate.unwrap_or(320000) / 1000,
        "contentType": mime,
        "coverArt": format_track_id(t.id),
        "path": t.path,
        "albumId": album_id,
        "artistId": artist_id,
        "genre": t.genre.as_deref().unwrap_or("")
    });

    if t.is_starred {
        if let Some(obj) = json.as_object_mut() {
            obj.insert("starred".to_string(), serde_json::json!("2024-01-01T00:00:00Z"));
        }
    }

    json
}

fn track_to_xml(t: &TrackRow) -> String {
    let fmt = t.format.as_deref().unwrap_or("mp3");
    let mime = mime_guess::from_path(fmt).first_or_octet_stream().to_string();
    let parent = t.album_id.map(format_album_id).unwrap_or_else(|| "alb_0".to_string());
    let artist_id = format_artist_id(&t.artist);
    let album_id = t.album_id.map(format_album_id).unwrap_or_else(|| "alb_0".to_string());
    let starred_attr = if t.is_starred { r#" starred="2024-01-01T00:00:00Z""# } else { "" };

    format!(
        r#"<child id="{}" parent="{}" isDir="false" title="{}" album="{}" artist="{}" track="{}" discNumber="{}" duration="{}" size="{}" suffix="{}" bitRate="{}" contentType="{}" coverArt="{}" path="{}" albumId="{}" artistId="{}" genre="{}"{}/>"#,
        format_track_id(t.id),
        parent,
        escape_xml(&t.title),
        escape_xml(&t.album),
        escape_xml(&t.artist),
        t.track_number.unwrap_or(0),
        t.disc_number.unwrap_or(1),
        t.duration.unwrap_or(0),
        t.size.unwrap_or(0),
        fmt,
        t.bitrate.unwrap_or(320000) / 1000,
        mime,
        format_track_id(t.id),
        escape_xml(&t.path),
        album_id,
        artist_id,
        escape_xml(t.genre.as_deref().unwrap_or("")),
        starred_attr
    )
}

async fn get_liked_track_ids(pool: &sqlx::SqlitePool, user_id: &str) -> std::collections::HashSet<i64> {
    let rows = sqlx::query_scalar::<_, i64>("SELECT track_id FROM liked_tracks WHERE user_id = ?")
        .bind(user_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    rows.into_iter().collect()
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
        subsonic_response_json(serde_json::json!({}))
    } else {
        subsonic_response_xml("")
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
        subsonic_response_json(serde_json::json!({
            "license": {
                "valid": true,
                "email": "user@audion.local",
                "key": "audion-license-key",
                "date": "2099-01-01T00:00:00Z"
            }
        }))
    } else {
        subsonic_response_xml(
            r#"    <license valid="true" email="user@audion.local" key="audion-license-key" date="2099-01-01T00:00:00Z"/>"#
        )
    }
}

// GET/POST /rest/getOpenSubsonicExtensions.view
pub async fn get_open_subsonic_extensions(
    State(state): State<AppState>,
    Query(params): Query<SubsonicParams>,
) -> Response {
    let f = params.f.as_deref().unwrap_or("xml");
    if let Err(e) = authenticate(&state, &params).await {
        return subsonic_error(f, ERROR_AUTH, &e);
    }

    if f == "json" {
        subsonic_response_json(serde_json::json!({
            "openSubsonicExtensions": [
                {
                    "name": "updateTags",
                    "versions": [1]
                }
            ]
        }))
    } else {
        subsonic_response_xml("    <openSubsonicExtensions>\n        <extension name=\"updateTags\" versions=\"1\"/>\n    </openSubsonicExtensions>")
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
        subsonic_response_json(serde_json::json!({
            "musicFolders": {
                "musicFolder": [
                    { "id": 1, "name": "Music Library" }
                ]
            }
        }))
    } else {
        subsonic_response_xml(
            r#"    <musicFolders>
        <musicFolder id="1" name="Music Library"/>
    </musicFolders>"#
        )
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

    let artists_res = sqlx::query("SELECT DISTINCT artist FROM tracks WHERE artist IS NOT NULL AND artist != '' ORDER BY artist ASC")
        .fetch_all(&user_pool)
        .await;

    let artist_rows = match artists_res {
        Ok(rows) => rows,
        Err(_) => return subsonic_error(f, ERROR_GENERIC, "Database query error"),
    };

    let mut groups: HashMap<char, Vec<String>> = HashMap::new();
    for row in artist_rows {
        let name: String = row.get("artist");
        if let Some(first_char) = name.trim().chars().next() {
            let key = first_char.to_uppercase().next().unwrap_or('A');
            groups.entry(key).or_insert_with(Vec::new).push(name);
        }
    }

    let mut sorted_keys: Vec<char> = groups.keys().cloned().collect();
    sorted_keys.sort();

    if f == "json" {
        let mut index_list = Vec::new();
        for key in sorted_keys {
            let mut artists = Vec::new();
            if let Some(names) = groups.get(&key) {
                for name in names {
                    let id = format_artist_id(name);
                    artists.push(serde_json::json!({
                        "id": id,
                        "name": name,
                        "coverArt": id
                    }));
                }
            }
            index_list.push(serde_json::json!({
                "name": key.to_string(),
                "artist": artists
            }));
        }

        subsonic_response_json(serde_json::json!({
            "indexes": {
                "lastModified": 1700000000000i64,
                "ignoredArticles": "The El La Los Las Le Les",
                "index": index_list
            }
        }))
    } else {
        let mut xml_indexes = String::new();
        for key in sorted_keys {
            xml_indexes.push_str(&format!(r#"<index name="{}">"#, key));
            if let Some(names) = groups.get(&key) {
                for name in names {
                    let id = format_artist_id(name);
                    xml_indexes.push_str(&format!(
                        r#"<artist id="{}" name="{}" coverArt="{}"/>"#,
                        id, escape_xml(name), id
                    ));
                }
            }
            xml_indexes.push_str("</index>");
        }

        subsonic_response_xml(&format!(
            r#"    <indexes lastModified="1700000000000" ignoredArticles="The El La Los Las Le Les">
        {}
    </indexes>"#,
            xml_indexes
        ))
    }
}

// GET/POST /rest/getArtists.view
pub async fn get_artists(
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

    let artists_res = sqlx::query("SELECT artist, COUNT(DISTINCT album_id) as alb_cnt FROM tracks WHERE artist IS NOT NULL AND artist != '' GROUP BY artist ORDER BY artist ASC")
        .fetch_all(&user_pool)
        .await;

    let artist_rows = match artists_res {
        Ok(rows) => rows,
        Err(_) => return subsonic_error(f, ERROR_GENERIC, "Database query error"),
    };

    let mut groups: HashMap<char, Vec<(String, i64)>> = HashMap::new();
    for row in artist_rows {
        let name: String = row.get("artist");
        let alb_cnt: i64 = row.get("alb_cnt");
        if let Some(first_char) = name.trim().chars().next() {
            let key = first_char.to_uppercase().next().unwrap_or('A');
            groups.entry(key).or_insert_with(Vec::new).push((name, alb_cnt));
        }
    }

    let mut sorted_keys: Vec<char> = groups.keys().cloned().collect();
    sorted_keys.sort();

    if f == "json" {
        let mut index_list = Vec::new();
        for key in sorted_keys {
            let mut artists = Vec::new();
            if let Some(items) = groups.get(&key) {
                for (name, alb_cnt) in items {
                    let id = format_artist_id(name);
                    artists.push(serde_json::json!({
                        "id": id,
                        "name": name,
                        "coverArt": id,
                        "albumCount": alb_cnt
                    }));
                }
            }
            index_list.push(serde_json::json!({
                "name": key.to_string(),
                "artist": artists
            }));
        }

        subsonic_response_json(serde_json::json!({
            "artists": {
                "ignoredArticles": "The El La Los Las Le Les",
                "index": index_list
            }
        }))
    } else {
        let mut xml_indexes = String::new();
        for key in sorted_keys {
            xml_indexes.push_str(&format!(r#"<index name="{}">"#, key));
            if let Some(items) = groups.get(&key) {
                for (name, alb_cnt) in items {
                    let id = format_artist_id(name);
                    xml_indexes.push_str(&format!(
                        r#"<artist id="{}" name="{}" coverArt="{}" albumCount="{}"/>"#,
                        id, escape_xml(name), id, alb_cnt
                    ));
                }
            }
            xml_indexes.push_str("</index>");
        }

        subsonic_response_xml(&format!(
            r#"    <artists ignoredArticles="The El La Los Las Le Les">
        {}
    </artists>"#,
            xml_indexes
        ))
    }
}

// GET/POST /rest/getArtist.view
pub async fn get_artist(
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

    let artist_id = match params.id.as_deref().or(params.artistId.as_deref()) {
        Some(id) => id,
        None => return subsonic_error(f, ERROR_MISSING_PARAM, "Missing parameter 'id' or 'artistId'"),
    };

    let artist_name = match parse_artist_id(artist_id) {
        Some(name) => name,
        None => return subsonic_error(f, ERROR_NOT_FOUND, "Artist not found"),
    };

    let album_rows = sqlx::query(
        "SELECT id, name, created_at FROM albums WHERE artist = ? ORDER BY name ASC"
    )
    .bind(&artist_name)
    .fetch_all(&user_pool)
    .await
    .unwrap_or_default();

    if f == "json" {
        let mut albums = Vec::new();
        for r in &album_rows {
            let alb_id: i64 = r.get("id");
            let title: String = r.get("name");
            let created: Option<String> = r.get("created_at");

            let song_cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE album_id = ?")
                .bind(alb_id)
                .fetch_one(&user_pool)
                .await
                .unwrap_or(0);

            let duration: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(duration), 0) FROM tracks WHERE album_id = ?")
                .bind(alb_id)
                .fetch_one(&user_pool)
                .await
                .unwrap_or(0);

            albums.push(serde_json::json!({
                "id": format_album_id(alb_id),
                "name": title,
                "artist": artist_name,
                "artistId": artist_id,
                "coverArt": format_album_id(alb_id),
                "songCount": song_cnt,
                "duration": duration,
                "created": created.unwrap_or_else(|| "2024-01-01T00:00:00Z".to_string())
            }));
        }

        subsonic_response_json(serde_json::json!({
            "artist": {
                "id": artist_id,
                "name": artist_name,
                "coverArt": artist_id,
                "albumCount": albums.len(),
                "album": albums
            }
        }))
    } else {
        let mut xml_albums = String::new();
        for r in &album_rows {
            let alb_id: i64 = r.get("id");
            let title: String = r.get("name");
            let created: Option<String> = r.get("created_at");

            let song_cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE album_id = ?")
                .bind(alb_id)
                .fetch_one(&user_pool)
                .await
                .unwrap_or(0);

            let duration: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(duration), 0) FROM tracks WHERE album_id = ?")
                .bind(alb_id)
                .fetch_one(&user_pool)
                .await
                .unwrap_or(0);

            xml_albums.push_str(&format!(
                r#"<album id="{}" name="{}" artist="{}" artistId="{}" coverArt="{}" songCount="{}" duration="{}" created="{}"/>"#,
                format_album_id(alb_id),
                escape_xml(&title),
                escape_xml(&artist_name),
                artist_id,
                format_album_id(alb_id),
                song_cnt,
                duration,
                created.as_deref().unwrap_or("2024-01-01T00:00:00Z")
            ));
        }

        subsonic_response_xml(&format!(
            r#"    <artist id="{}" name="{}" coverArt="{}" albumCount="{}">
        {}
    </artist>"#,
            artist_id,
            escape_xml(&artist_name),
            artist_id,
            album_rows.len(),
            xml_albums
        ))
    }
}

// GET/POST /rest/getAlbum.view
pub async fn get_album(
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

    let album_id_param = match params.id.as_deref().or(params.albumId.as_deref()) {
        Some(id) => id,
        None => return subsonic_error(f, ERROR_MISSING_PARAM, "Missing parameter 'id' or 'albumId'"),
    };

    let album_id = match parse_album_id(album_id_param) {
        Some(id) => id,
        None => return subsonic_error(f, ERROR_NOT_FOUND, "Album not found"),
    };

    let album_row = sqlx::query("SELECT id, name, artist, created_at FROM albums WHERE id = ?")
        .bind(album_id)
        .fetch_optional(&user_pool)
        .await;

    let (alb_name, alb_artist, created_at) = match album_row {
        Ok(Some(r)) => (
            r.get::<String, _>("name"),
            r.get::<Option<String>, _>("artist").unwrap_or_default(),
            r.get::<Option<String>, _>("created_at"),
        ),
        _ => return subsonic_error(f, ERROR_NOT_FOUND, "Album not found"),
    };

    let liked_set = get_liked_track_ids(&user_pool, &user.id).await;

    let track_rows = sqlx::query(
        "SELECT id, title, artist, album, album_id, track_number, disc_number, duration, size, format, bitrate, path, genre FROM tracks WHERE album_id = ? ORDER BY disc_number, track_number"
    )
    .bind(album_id)
    .fetch_all(&user_pool)
    .await
    .unwrap_or_default();

    let tracks: Vec<TrackRow> = track_rows.iter().map(|r| row_to_track(r, &liked_set)).collect();
    let duration: i32 = tracks.iter().map(|t| t.duration.unwrap_or(0)).sum();

    let artist_id = format_artist_id(&alb_artist);

    if f == "json" {
        let songs_json: Vec<serde_json::Value> = tracks.iter().map(track_to_json).collect();
        subsonic_response_json(serde_json::json!({
            "album": {
                "id": format_album_id(album_id),
                "name": alb_name,
                "artist": alb_artist,
                "artistId": artist_id,
                "coverArt": format_album_id(album_id),
                "songCount": tracks.len(),
                "duration": duration,
                "created": created_at.unwrap_or_else(|| "2024-01-01T00:00:00Z".to_string()),
                "song": songs_json
            }
        }))
    } else {
        let xml_songs: String = tracks.iter().map(track_to_xml).collect();
        subsonic_response_xml(&format!(
            r#"    <album id="{}" name="{}" artist="{}" artistId="{}" coverArt="{}" songCount="{}" duration="{}" created="{}">
        {}
    </album>"#,
            format_album_id(album_id),
            escape_xml(&alb_name),
            escape_xml(&alb_artist),
            artist_id,
            format_album_id(album_id),
            tracks.len(),
            duration,
            created_at.as_deref().unwrap_or("2024-01-01T00:00:00Z"),
            xml_songs
        ))
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
        let artist_name = match parse_artist_id(directory_id) {
            Some(name) => name,
            None => return subsonic_error(f, ERROR_GENERIC, "Invalid artist ID"),
        };

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
                    "id": format_album_id(alb_id),
                    "parent": directory_id,
                    "title": alb_title,
                    "artist": artist_name,
                    "isDir": true,
                    "coverArt": format_album_id(alb_id)
                }));
            }

            subsonic_response_json(serde_json::json!({
                "directory": {
                    "id": directory_id,
                    "name": artist_name,
                    "child": children
                }
            }))
        } else {
            let mut xml_children = String::new();
            for r in album_rows {
                let alb_id: i64 = r.get("album_id");
                let alb_title: String = r.get("album");
                xml_children.push_str(&format!(
                    r#"<child id="{}" parent="{}" title="{}" artist="{}" isDir="true" coverArt="{}"/>"#,
                    format_album_id(alb_id), directory_id, escape_xml(&alb_title), escape_xml(&artist_name), format_album_id(alb_id)
                ));
            }

            subsonic_response_xml(&format!(
                r#"    <directory id="{}" name="{}">
        {}
    </directory>"#,
                directory_id, escape_xml(&artist_name), xml_children
            ))
        }
    } else if directory_id.starts_with("alb_") {
        let album_id = match parse_album_id(directory_id) {
            Some(id) => id,
            None => return subsonic_error(f, ERROR_GENERIC, "Invalid album ID"),
        };

        let album_name: String = sqlx::query_scalar("SELECT name FROM albums WHERE id = ?")
            .bind(album_id)
            .fetch_optional(&user_pool)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "Unknown Album".to_string());

        let liked_set = get_liked_track_ids(&user_pool, &user.id).await;

        let tracks_res = sqlx::query(
            "SELECT id, title, artist, album, album_id, track_number, disc_number, duration, size, format, bitrate, path, genre FROM tracks WHERE album_id = ? ORDER BY disc_number, track_number"
        )
        .bind(album_id)
        .fetch_all(&user_pool)
        .await;

        let track_rows = match tracks_res {
            Ok(rows) => rows,
            Err(_) => return subsonic_error(f, ERROR_GENERIC, "Database query error"),
        };

        let tracks: Vec<TrackRow> = track_rows.iter().map(|r| row_to_track(r, &liked_set)).collect();

        if f == "json" {
            let children: Vec<serde_json::Value> = tracks.iter().map(track_to_json).collect();
            subsonic_response_json(serde_json::json!({
                "directory": {
                    "id": directory_id,
                    "name": album_name,
                    "child": children
                }
            }))
        } else {
            let xml_children: String = tracks.iter().map(track_to_xml).collect();
            subsonic_response_xml(&format!(
                r#"    <directory id="{}" name="{}">
        {}
    </directory>"#,
                directory_id, escape_xml(&album_name), xml_children
            ))
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

    let song_id = match params.id.as_deref().or(params.songId.as_deref()).and_then(parse_track_id) {
        Some(id) => id,
        None => return subsonic_error(f, ERROR_MISSING_PARAM, "Missing track parameter 'id'"),
    };

    let liked_set = get_liked_track_ids(&user_pool, &user.id).await;

    let track_res = sqlx::query(
        "SELECT id, title, artist, album, album_id, track_number, disc_number, duration, size, format, bitrate, path, genre FROM tracks WHERE id = ?"
    )
    .bind(song_id)
    .fetch_optional(&user_pool)
    .await;

    let track = match track_res {
        Ok(Some(row)) => row_to_track(&row, &liked_set),
        _ => return subsonic_error(f, ERROR_NOT_FOUND, "Song not found"),
    };

    if f == "json" {
        subsonic_response_json(serde_json::json!({
            "song": track_to_json(&track)
        }))
    } else {
        subsonic_response_xml(&format!(
            r#"    <song>{}</song>"#,
            track_to_xml(&track)
        ))
    }
}

// GET/POST /rest/getGenres.view
pub async fn get_genres(
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

    let rows = sqlx::query(
        "SELECT genre, COUNT(*) as song_cnt, COUNT(DISTINCT album_id) as alb_cnt FROM tracks WHERE genre IS NOT NULL AND genre != '' GROUP BY genre ORDER BY genre ASC"
    )
    .fetch_all(&user_pool)
    .await
    .unwrap_or_default();

    if f == "json" {
        let mut genres = Vec::new();
        for r in &rows {
            let g: String = r.get("genre");
            let s_cnt: i64 = r.get("song_cnt");
            let a_cnt: i64 = r.get("alb_cnt");
            genres.push(serde_json::json!({
                "value": g,
                "songCount": s_cnt,
                "albumCount": a_cnt
            }));
        }
        subsonic_response_json(serde_json::json!({
            "genres": {
                "genre": genres
            }
        }))
    } else {
        let mut xml_genres = String::new();
        for r in &rows {
            let g: String = r.get("genre");
            let s_cnt: i64 = r.get("song_cnt");
            let a_cnt: i64 = r.get("alb_cnt");
            xml_genres.push_str(&format!(
                r#"<genre value="{}" songCount="{}" albumCount="{}"/>"#,
                escape_xml(&g), s_cnt, a_cnt
            ));
        }
        subsonic_response_xml(&format!(
            r#"    <genres>
        {}
    </genres>"#,
            xml_genres
        ))
    }
}

// GET/POST /rest/getAlbumList.view & getAlbumList2.view
pub async fn get_album_list(
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

    let list_type = params.type_.as_deref().unwrap_or("newest");
    let size = params.size.unwrap_or(10).clamp(1, 500);
    let offset = params.offset.unwrap_or(0).max(0);

    let order_sql = match list_type {
        "newest" | "byYear" => "ORDER BY created_at DESC, id DESC",
        "alphabeticalByName" => "ORDER BY name ASC",
        "alphabeticalByArtist" => "ORDER BY artist ASC, name ASC",
        "random" => "ORDER BY RANDOM()",
        _ => "ORDER BY id DESC",
    };

    let query_str = format!(
        "SELECT id, name, artist, created_at FROM albums {} LIMIT ? OFFSET ?",
        order_sql
    );

    let rows = sqlx::query(&query_str)
        .bind(size)
        .bind(offset)
        .fetch_all(&user_pool)
        .await
        .unwrap_or_default();

    if f == "json" {
        let mut albums = Vec::new();
        for r in &rows {
            let alb_id: i64 = r.get("id");
            let name: String = r.get("name");
            let artist: Option<String> = r.get("artist");
            let artist_name = artist.unwrap_or_default();
            let created: Option<String> = r.get("created_at");

            let song_cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE album_id = ?")
                .bind(alb_id)
                .fetch_one(&user_pool)
                .await
                .unwrap_or(0);

            let duration: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(duration), 0) FROM tracks WHERE album_id = ?")
                .bind(alb_id)
                .fetch_one(&user_pool)
                .await
                .unwrap_or(0);

            albums.push(serde_json::json!({
                "id": format_album_id(alb_id),
                "name": name,
                "artist": artist_name,
                "artistId": format_artist_id(&artist_name),
                "coverArt": format_album_id(alb_id),
                "songCount": song_cnt,
                "duration": duration,
                "created": created.unwrap_or_else(|| "2024-01-01T00:00:00Z".to_string())
            }));
        }

        subsonic_response_json(serde_json::json!({
            "albumList2": {
                "album": albums
            }
        }))
    } else {
        let mut xml_albums = String::new();
        for r in &rows {
            let alb_id: i64 = r.get("id");
            let name: String = r.get("name");
            let artist: Option<String> = r.get("artist");
            let artist_name = artist.unwrap_or_default();
            let created: Option<String> = r.get("created_at");

            let song_cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE album_id = ?")
                .bind(alb_id)
                .fetch_one(&user_pool)
                .await
                .unwrap_or(0);

            let duration: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(duration), 0) FROM tracks WHERE album_id = ?")
                .bind(alb_id)
                .fetch_one(&user_pool)
                .await
                .unwrap_or(0);

            xml_albums.push_str(&format!(
                r#"<album id="{}" name="{}" artist="{}" artistId="{}" coverArt="{}" songCount="{}" duration="{}" created="{}"/>"#,
                format_album_id(alb_id),
                escape_xml(&name),
                escape_xml(&artist_name),
                format_artist_id(&artist_name),
                format_album_id(alb_id),
                song_cnt,
                duration,
                created.as_deref().unwrap_or("2024-01-01T00:00:00Z")
            ));
        }

        subsonic_response_xml(&format!(
            r#"    <albumList2>
        {}
    </albumList2>"#,
            xml_albums
        ))
    }
}

// GET/POST /rest/getRandomSongs.view
pub async fn get_random_songs(
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

    let size = params.size.unwrap_or(10).clamp(1, 500);
    let liked_set = get_liked_track_ids(&user_pool, &user.id).await;

    let track_rows = sqlx::query(
        "SELECT id, title, artist, album, album_id, track_number, disc_number, duration, size, format, bitrate, path, genre FROM tracks ORDER BY RANDOM() LIMIT ?"
    )
    .bind(size)
    .fetch_all(&user_pool)
    .await
    .unwrap_or_default();

    let tracks: Vec<TrackRow> = track_rows.iter().map(|r| row_to_track(r, &liked_set)).collect();

    if f == "json" {
        let songs: Vec<serde_json::Value> = tracks.iter().map(track_to_json).collect();
        subsonic_response_json(serde_json::json!({
            "randomSongs": {
                "song": songs
            }
        }))
    } else {
        let xml_songs: String = tracks.iter().map(track_to_xml).collect();
        subsonic_response_xml(&format!(
            r#"    <randomSongs>
        {}
    </randomSongs>"#,
            xml_songs
        ))
    }
}

// GET/POST /rest/getSongsByGenre.view
pub async fn get_songs_by_genre(
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

    let genre = match &params.genre {
        Some(g) => g,
        None => return subsonic_error(f, ERROR_MISSING_PARAM, "Missing parameter 'genre'"),
    };

    let count = params.count.unwrap_or(params.size.unwrap_or(10)).clamp(1, 500);
    let offset = params.offset.unwrap_or(0).max(0);
    let liked_set = get_liked_track_ids(&user_pool, &user.id).await;

    let track_rows = sqlx::query(
        "SELECT id, title, artist, album, album_id, track_number, disc_number, duration, size, format, bitrate, path, genre FROM tracks WHERE genre = ? ORDER BY id ASC LIMIT ? OFFSET ?"
    )
    .bind(genre)
    .bind(count)
    .bind(offset)
    .fetch_all(&user_pool)
    .await
    .unwrap_or_default();

    let tracks: Vec<TrackRow> = track_rows.iter().map(|r| row_to_track(r, &liked_set)).collect();

    if f == "json" {
        let songs: Vec<serde_json::Value> = tracks.iter().map(track_to_json).collect();
        subsonic_response_json(serde_json::json!({
            "songsByGenre": {
                "song": songs
            }
        }))
    } else {
        let xml_songs: String = tracks.iter().map(track_to_xml).collect();
        subsonic_response_xml(&format!(
            r#"    <songsByGenre>
        {}
    </songsByGenre>"#,
            xml_songs
        ))
    }
}

// GET/POST /rest/getStarred.view & getStarred2.view
pub async fn get_starred(
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

    let liked_set = get_liked_track_ids(&user_pool, &user.id).await;

    let track_rows = sqlx::query(
        "SELECT t.id, t.title, t.artist, t.album, t.album_id, t.track_number, t.disc_number, t.duration, t.size, t.format, t.bitrate, t.path, t.genre FROM tracks t INNER JOIN liked_tracks lt ON t.id = lt.track_id WHERE lt.user_id = ? ORDER BY lt.liked_at DESC"
    )
    .bind(&user.id)
    .fetch_all(&user_pool)
    .await
    .unwrap_or_default();

    let tracks: Vec<TrackRow> = track_rows.iter().map(|r| row_to_track(r, &liked_set)).collect();

    if f == "json" {
        let songs: Vec<serde_json::Value> = tracks.iter().map(track_to_json).collect();
        subsonic_response_json(serde_json::json!({
            "starred2": {
                "song": songs,
                "album": [],
                "artist": []
            }
        }))
    } else {
        let xml_songs: String = tracks.iter().map(track_to_xml).collect();
        subsonic_response_xml(&format!(
            r#"    <starred2>
        {}
    </starred2>"#,
            xml_songs
        ))
    }
}

// GET/POST /rest/star.view
pub async fn star(
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

    let mut track_ids = Vec::new();
    if let Some(ref id) = params.id.or_else(|| params.songId.clone()) {
        if let Some(tr_id) = parse_track_id(id) {
            track_ids.push(tr_id);
        }
    }
    if let Some(ref alb_id_str) = params.albumId {
        if let Some(alb_id) = parse_album_id(alb_id_str) {
            let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM tracks WHERE album_id = ?")
                .bind(alb_id)
                .fetch_all(&user_pool)
                .await
                .unwrap_or_default();
            track_ids.extend(ids);
        }
    }
    if let Some(ref art_id_str) = params.artistId {
        if let Some(art_name) = parse_artist_id(art_id_str) {
            let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM tracks WHERE artist = ?")
                .bind(&art_name)
                .fetch_all(&user_pool)
                .await
                .unwrap_or_default();
            track_ids.extend(ids);
        }
    }

    for tr_id in track_ids {
        let _ = sqlx::query("INSERT OR IGNORE INTO liked_tracks (user_id, track_id) VALUES (?, ?)")
            .bind(&user.id)
            .bind(tr_id)
            .execute(&user_pool)
            .await;
    }

    if f == "json" {
        subsonic_response_json(serde_json::json!({}))
    } else {
        subsonic_response_xml("")
    }
}

// GET/POST /rest/unstar.view
pub async fn unstar(
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

    let mut track_ids = Vec::new();
    if let Some(ref id) = params.id.or_else(|| params.songId.clone()) {
        if let Some(tr_id) = parse_track_id(id) {
            track_ids.push(tr_id);
        }
    }
    if let Some(ref alb_id_str) = params.albumId {
        if let Some(alb_id) = parse_album_id(alb_id_str) {
            let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM tracks WHERE album_id = ?")
                .bind(alb_id)
                .fetch_all(&user_pool)
                .await
                .unwrap_or_default();
            track_ids.extend(ids);
        }
    }

    for tr_id in track_ids {
        let _ = sqlx::query("DELETE FROM liked_tracks WHERE user_id = ? AND track_id = ?")
            .bind(&user.id)
            .bind(tr_id)
            .execute(&user_pool)
            .await;
    }

    if f == "json" {
        subsonic_response_json(serde_json::json!({}))
    } else {
        subsonic_response_xml("")
    }
}

// GET/POST /rest/search2.view & search3.view
pub async fn search3(
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

    let query_term = match params.query.as_deref().or(params.q.as_deref()) {
        Some(q) => q,
        None => return subsonic_error(f, ERROR_MISSING_PARAM, "Missing parameter 'query' or 'q'"),
    };

    let search_pattern = format!("%{}%", query_term);
    let artist_count = params.artistCount.unwrap_or(20).clamp(1, 100);
    let album_count = params.albumCount.unwrap_or(20).clamp(1, 100);
    let song_count = params.songCount.unwrap_or(20).clamp(1, 100);

    // Search Artists
    let artist_rows = sqlx::query(
        "SELECT DISTINCT artist FROM tracks WHERE artist LIKE ? ORDER BY artist ASC LIMIT ?"
    )
    .bind(&search_pattern)
    .bind(artist_count)
    .fetch_all(&user_pool)
    .await
    .unwrap_or_default();

    // Search Albums
    let album_rows = sqlx::query(
        "SELECT id, name, artist, created_at FROM albums WHERE name LIKE ? OR artist LIKE ? ORDER BY name ASC LIMIT ?"
    )
    .bind(&search_pattern)
    .bind(&search_pattern)
    .bind(album_count)
    .fetch_all(&user_pool)
    .await
    .unwrap_or_default();

    // Search Songs
    let liked_set = get_liked_track_ids(&user_pool, &user.id).await;
    let song_rows = sqlx::query(
        "SELECT id, title, artist, album, album_id, track_number, disc_number, duration, size, format, bitrate, path, genre FROM tracks WHERE title LIKE ? OR artist LIKE ? OR album LIKE ? ORDER BY title ASC LIMIT ?"
    )
    .bind(&search_pattern)
    .bind(&search_pattern)
    .bind(&search_pattern)
    .bind(song_count)
    .fetch_all(&user_pool)
    .await
    .unwrap_or_default();

    let tracks: Vec<TrackRow> = song_rows.iter().map(|r| row_to_track(r, &liked_set)).collect();

    if f == "json" {
        let mut artist_list = Vec::new();
        for r in artist_rows {
            let name: String = r.get("artist");
            let id = format_artist_id(&name);
            artist_list.push(serde_json::json!({
                "id": id,
                "name": name,
                "coverArt": id
            }));
        }

        let mut album_list = Vec::new();
        for r in album_rows {
            let alb_id: i64 = r.get("id");
            let name: String = r.get("name");
            let artist: Option<String> = r.get("artist");
            let artist_name = artist.unwrap_or_default();
            album_list.push(serde_json::json!({
                "id": format_album_id(alb_id),
                "name": name,
                "artist": artist_name,
                "artistId": format_artist_id(&artist_name),
                "coverArt": format_album_id(alb_id)
            }));
        }

        let song_list: Vec<serde_json::Value> = tracks.iter().map(track_to_json).collect();

        subsonic_response_json(serde_json::json!({
            "searchResult3": {
                "artist": artist_list,
                "album": album_list,
                "song": song_list
            }
        }))
    } else {
        let mut xml_artists = String::new();
        for r in artist_rows {
            let name: String = r.get("artist");
            let id = format_artist_id(&name);
            xml_artists.push_str(&format!(
                r#"<artist id="{}" name="{}" coverArt="{}"/>"#,
                id, escape_xml(&name), id
            ));
        }

        let mut xml_albums = String::new();
        for r in album_rows {
            let alb_id: i64 = r.get("id");
            let name: String = r.get("name");
            let artist: Option<String> = r.get("artist");
            let artist_name = artist.unwrap_or_default();
            xml_albums.push_str(&format!(
                r#"<album id="{}" name="{}" artist="{}" artistId="{}" coverArt="{}"/>"#,
                format_album_id(alb_id), escape_xml(&name), escape_xml(&artist_name), format_artist_id(&artist_name), format_album_id(alb_id)
            ));
        }

        let xml_songs: String = tracks.iter().map(track_to_xml).collect();

        subsonic_response_xml(&format!(
            r#"    <searchResult3>
        {}
        {}
        {}
    </searchResult3>"#,
            xml_artists, xml_albums, xml_songs
        ))
    }
}

// GET/POST /rest/getPlaylists.view
pub async fn get_playlists(
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

    let playlist_rows = sqlx::query(
        "SELECT id, name, created_at FROM playlists WHERE user_id = ? ORDER BY name ASC"
    )
    .bind(&user.id)
    .fetch_all(&user_pool)
    .await
    .unwrap_or_default();

    if f == "json" {
        let mut list = Vec::new();
        for r in &playlist_rows {
            let pl_id: i64 = r.get("id");
            let name: String = r.get("name");
            let created: Option<String> = r.get("created_at");

            let song_cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?")
                .bind(pl_id)
                .fetch_one(&user_pool)
                .await
                .unwrap_or(0);

            let duration: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(t.duration), 0) FROM playlist_tracks pt INNER JOIN tracks t ON pt.track_id = t.id WHERE pt.playlist_id = ?")
                .bind(pl_id)
                .fetch_one(&user_pool)
                .await
                .unwrap_or(0);

            list.push(serde_json::json!({
                "id": format_playlist_id(pl_id),
                "name": name,
                "comment": "",
                "owner": user.username,
                "public": false,
                "songCount": song_cnt,
                "duration": duration,
                "created": created.unwrap_or_else(|| "2024-01-01T00:00:00Z".to_string()),
                "coverArt": format_playlist_id(pl_id)
            }));
        }

        subsonic_response_json(serde_json::json!({
            "playlists": {
                "playlist": list
            }
        }))
    } else {
        let mut xml_list = String::new();
        for r in &playlist_rows {
            let pl_id: i64 = r.get("id");
            let name: String = r.get("name");
            let created: Option<String> = r.get("created_at");

            let song_cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?")
                .bind(pl_id)
                .fetch_one(&user_pool)
                .await
                .unwrap_or(0);

            let duration: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(t.duration), 0) FROM playlist_tracks pt INNER JOIN tracks t ON pt.track_id = t.id WHERE pt.playlist_id = ?")
                .bind(pl_id)
                .fetch_one(&user_pool)
                .await
                .unwrap_or(0);

            xml_list.push_str(&format!(
                r#"<playlist id="{}" name="{}" comment="" owner="{}" public="false" songCount="{}" duration="{}" created="{}" coverArt="{}"/>"#,
                format_playlist_id(pl_id), escape_xml(&name), escape_xml(&user.username), song_cnt, duration, created.as_deref().unwrap_or("2024-01-01T00:00:00Z"), format_playlist_id(pl_id)
            ));
        }

        subsonic_response_xml(&format!(
            r#"    <playlists>
        {}
    </playlists>"#,
            xml_list
        ))
    }
}

// GET/POST /rest/getPlaylist.view
pub async fn get_playlist(
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

    let pl_id_param = match params.id.as_deref().or(params.playlistId.as_deref()) {
        Some(id) => id,
        None => return subsonic_error(f, ERROR_MISSING_PARAM, "Missing parameter 'id' or 'playlistId'"),
    };

    let pl_id = match parse_playlist_id(pl_id_param) {
        Some(id) => id,
        None => return subsonic_error(f, ERROR_NOT_FOUND, "Playlist not found"),
    };

    let pl_row = sqlx::query("SELECT id, name, created_at FROM playlists WHERE id = ? AND user_id = ?")
        .bind(pl_id)
        .bind(&user.id)
        .fetch_optional(&user_pool)
        .await;

    let (pl_name, created_at) = match pl_row {
        Ok(Some(r)) => (r.get::<String, _>("name"), r.get::<Option<String>, _>("created_at")),
        _ => return subsonic_error(f, ERROR_NOT_FOUND, "Playlist not found"),
    };

    let liked_set = get_liked_track_ids(&user_pool, &user.id).await;

    let track_rows = sqlx::query(
        "SELECT t.id, t.title, t.artist, t.album, t.album_id, t.track_number, t.disc_number, t.duration, t.size, t.format, t.bitrate, t.path, t.genre FROM playlist_tracks pt INNER JOIN tracks t ON pt.track_id = t.id WHERE pt.playlist_id = ? ORDER BY pt.position ASC"
    )
    .bind(pl_id)
    .fetch_all(&user_pool)
    .await
    .unwrap_or_default();

    let tracks: Vec<TrackRow> = track_rows.iter().map(|r| row_to_track(r, &liked_set)).collect();
    let duration: i32 = tracks.iter().map(|t| t.duration.unwrap_or(0)).sum();

    if f == "json" {
        let entries: Vec<serde_json::Value> = tracks.iter().map(track_to_json).collect();
        subsonic_response_json(serde_json::json!({
            "playlist": {
                "id": format_playlist_id(pl_id),
                "name": pl_name,
                "comment": "",
                "owner": user.username,
                "public": false,
                "songCount": tracks.len(),
                "duration": duration,
                "created": created_at.unwrap_or_else(|| "2024-01-01T00:00:00Z".to_string()),
                "coverArt": format_playlist_id(pl_id),
                "entry": entries
            }
        }))
    } else {
        let xml_entries: String = tracks.iter().map(track_to_xml).collect();
        subsonic_response_xml(&format!(
            r#"    <playlist id="{}" name="{}" comment="" owner="{}" public="false" songCount="{}" duration="{}" created="{}" coverArt="{}">
        {}
    </playlist>"#,
            format_playlist_id(pl_id), escape_xml(&pl_name), escape_xml(&user.username), tracks.len(), duration, created_at.as_deref().unwrap_or("2024-01-01T00:00:00Z"), format_playlist_id(pl_id), xml_entries
        ))
    }
}

// GET/POST /rest/createPlaylist.view
pub async fn create_playlist(
    State(state): State<AppState>,
    Query(params): Query<SubsonicParams>,
    RawQuery(raw_q): RawQuery,
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

    let name = match &params.name {
        Some(n) => n,
        None => return subsonic_error(f, ERROR_MISSING_PARAM, "Missing parameter 'name'"),
    };

    let res = sqlx::query("INSERT INTO playlists (user_id, name) VALUES (?, ?)")
        .bind(&user.id)
        .bind(name)
        .execute(&user_pool)
        .await;

    let pl_id = match res {
        Ok(r) => r.last_insert_rowid(),
        Err(e) => return subsonic_error(f, ERROR_GENERIC, &format!("Failed to create playlist: {}", e)),
    };

    let mut song_ids = Vec::new();
    if let Some(ref q_str) = raw_q {
        for (k, v) in q_str.split('&').filter_map(|p| p.split_once('=')) {
            if (k == "songId" || k == "id") && !v.is_empty() {
                if let Some(tr_id) = parse_track_id(v) {
                    song_ids.push(tr_id);
                }
            }
        }
    }
    if song_ids.is_empty() {
        if let Some(ref id) = params.songId.or(params.id) {
            if let Some(tr_id) = parse_track_id(id) {
                song_ids.push(tr_id);
            }
        }
    }

    for (pos, tr_id) in song_ids.iter().enumerate() {
        let _ = sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)")
            .bind(pl_id)
            .bind(tr_id)
            .bind(pos as i32)
            .execute(&user_pool)
            .await;
    }

    if f == "json" {
        subsonic_response_json(serde_json::json!({
            "playlist": {
                "id": format_playlist_id(pl_id),
                "name": name,
                "owner": user.username,
                "songCount": song_ids.len()
            }
        }))
    } else {
        subsonic_response_xml(&format!(
            r#"    <playlist id="{}" name="{}" owner="{}" songCount="{}"/>"#,
            format_playlist_id(pl_id), escape_xml(name), escape_xml(&user.username), song_ids.len()
        ))
    }
}

// GET/POST /rest/updatePlaylist.view
pub async fn update_playlist(
    State(state): State<AppState>,
    Query(params): Query<SubsonicParams>,
    RawQuery(raw_q): RawQuery,
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

    let pl_id_param = match params.playlistId.as_deref().or(params.id.as_deref()) {
        Some(id) => id,
        None => return subsonic_error(f, ERROR_MISSING_PARAM, "Missing parameter 'playlistId'"),
    };

    let pl_id = match parse_playlist_id(pl_id_param) {
        Some(id) => id,
        None => return subsonic_error(f, ERROR_NOT_FOUND, "Playlist not found"),
    };

    if let Some(ref name) = params.name {
        let _ = sqlx::query("UPDATE playlists SET name = ? WHERE id = ? AND user_id = ?")
            .bind(name)
            .bind(pl_id)
            .bind(&user.id)
            .execute(&user_pool)
            .await;
    }

    let mut add_song_ids = Vec::new();
    if let Some(ref q_str) = raw_q {
        for (k, v) in q_str.split('&').filter_map(|p| p.split_once('=')) {
            if k == "songIdToAdd" && !v.is_empty() {
                if let Some(tr_id) = parse_track_id(v) {
                    add_song_ids.push(tr_id);
                }
            }
        }
    }

    if !add_song_ids.is_empty() {
        let max_pos: i32 = sqlx::query_scalar("SELECT COALESCE(MAX(position), -1) FROM playlist_tracks WHERE playlist_id = ?")
            .bind(pl_id)
            .fetch_one(&user_pool)
            .await
            .unwrap_or(-1);

        for (i, tr_id) in add_song_ids.iter().enumerate() {
            let _ = sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)")
                .bind(pl_id)
                .bind(tr_id)
                .bind(max_pos + 1 + i as i32)
                .execute(&user_pool)
                .await;
        }
    }

    if f == "json" {
        subsonic_response_json(serde_json::json!({}))
    } else {
        subsonic_response_xml("")
    }
}

// GET/POST /rest/deletePlaylist.view
pub async fn delete_playlist(
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

    let pl_id_param = match params.id.as_deref().or(params.playlistId.as_deref()) {
        Some(id) => id,
        None => return subsonic_error(f, ERROR_MISSING_PARAM, "Missing parameter 'id' or 'playlistId'"),
    };

    let pl_id = match parse_playlist_id(pl_id_param) {
        Some(id) => id,
        None => return subsonic_error(f, ERROR_NOT_FOUND, "Playlist not found"),
    };

    let _ = sqlx::query("DELETE FROM playlists WHERE id = ? AND user_id = ?")
        .bind(pl_id)
        .bind(&user.id)
        .execute(&user_pool)
        .await;

    if f == "json" {
        subsonic_response_json(serde_json::json!({}))
    } else {
        subsonic_response_xml("")
    }
}

// GET/POST /rest/getCoverArt.view & /rest/getCoverArt
pub async fn get_cover_art(
    State(state): State<AppState>,
    Query(params): Query<SubsonicParams>,
) -> Response {
    let f = params.f.as_deref().unwrap_or("xml");
    let user = match authenticate(&state, &params).await {
        Ok(u) => u,
        Err(e) => return subsonic_error(f, ERROR_AUTH, &e),
    };

    let id = match params.id.as_deref() {
        Some(id) => id,
        None => return subsonic_error(f, ERROR_MISSING_PARAM, "Missing parameter 'id'"),
    };

    let user_pool = match state.get_user_pool(&user.id).await {
        Ok(p) => p,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    let track_id = if let Some(tr_id) = parse_track_id(id) {
        Some(tr_id)
    } else if let Some(alb_id) = parse_album_id(id) {
        sqlx::query_scalar::<_, i64>("SELECT id FROM tracks WHERE album_id = ? ORDER BY disc_number, track_number LIMIT 1")
            .bind(alb_id)
            .fetch_optional(&user_pool)
            .await
            .unwrap_or(None)
    } else if let Some(artist_name) = parse_artist_id(id) {
        sqlx::query_scalar::<_, i64>("SELECT id FROM tracks WHERE artist = ? LIMIT 1")
            .bind(&artist_name)
            .fetch_optional(&user_pool)
            .await
            .unwrap_or(None)
    } else if let Ok(num_id) = id.parse::<i64>() {
        Some(num_id)
    } else {
        None
    };

    if let Some(tr_id) = track_id {
        let claims = crate::auth::Claims {
            sub: user.id,
            username: user.username,
            role: user.role,
            exp: 0,
        };
        return crate::api::stream::get_track_cover(claims, State(state), axum::extract::Path(tr_id)).await.into_response();
    }

    StatusCode::NOT_FOUND.into_response()
}

// GET/POST /rest/stream.view & /rest/stream
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

    let song_id_str = match params.id.as_deref().or(params.songId.as_deref()) {
        Some(id) => id,
        None => return subsonic_error(f, ERROR_MISSING_PARAM, "Missing track parameter 'id'"),
    };

    let song_id = match parse_track_id(song_id_str) {
        Some(id) => id,
        None => return subsonic_error(f, ERROR_GENERIC, "Invalid song ID"),
    };

    let claims = crate::auth::Claims {
        sub: user.id,
        username: user.username,
        role: user.role,
        exp: 0,
    };

    crate::api::stream::stream_track_subsonic(
        claims,
        headers,
        &state,
        song_id,
        params.maxBitRate,
        params.format.as_deref(),
    ).await
}

// GET/POST /rest/getUser.view
pub async fn get_user(
    State(state): State<AppState>,
    Query(params): Query<SubsonicParams>,
) -> Response {
    let f = params.f.as_deref().unwrap_or("xml");
    let user = match authenticate(&state, &params).await {
        Ok(u) => u,
        Err(e) => return subsonic_error(f, ERROR_AUTH, &e),
    };

    let is_admin = user.role == "Admin";

    if f == "json" {
        subsonic_response_json(serde_json::json!({
            "user": {
                "username": user.username,
                "email": "",
                "scrobblingEnabled": true,
                "maxBitRate": 0,
                "adminRole": is_admin,
                "settingsRole": is_admin,
                "downloadRole": true,
                "uploadRole": is_admin,
                "playlistRole": true,
                "coverArtRole": true,
                "commentRole": false,
                "podcastRole": false,
                "streamRole": true,
                "jukeboxRole": false,
                "shareRole": false,
                "videoConversionRole": false,
                "folder": [1]
            }
        }))
    } else {
        subsonic_response_xml(&format!(
            r#"    <user username="{}" email="" scrobblingEnabled="true" maxBitRate="0"
          adminRole="{}" settingsRole="{}" downloadRole="true" uploadRole="{}"
          playlistRole="true" coverArtRole="true" commentRole="false" podcastRole="false"
          streamRole="true" jukeboxRole="false" shareRole="false" videoConversionRole="false">
        <folder>1</folder>
    </user>"#,
            escape_xml(&user.username),
            is_admin,
            is_admin,
            is_admin,
        ))
    }
}

// GET/POST /rest/getUsers.view
pub async fn get_users(
    State(state): State<AppState>,
    Query(params): Query<SubsonicParams>,
) -> Response {
    let f = params.f.as_deref().unwrap_or("xml");
    let user = match authenticate(&state, &params).await {
        Ok(u) => u,
        Err(e) => return subsonic_error(f, ERROR_AUTH, &e),
    };

    let is_admin = user.role == "Admin";

    if f == "json" {
        subsonic_response_json(serde_json::json!({
            "users": {
                "user": [{
                    "username": user.username,
                    "email": "",
                    "scrobblingEnabled": true,
                    "maxBitRate": 0,
                    "adminRole": is_admin,
                    "settingsRole": is_admin,
                    "downloadRole": true,
                    "uploadRole": is_admin,
                    "playlistRole": true,
                    "coverArtRole": true,
                    "commentRole": false,
                    "podcastRole": false,
                    "streamRole": true,
                    "jukeboxRole": false,
                    "shareRole": false,
                    "videoConversionRole": false,
                    "folder": [1]
                }]
            }
        }))
    } else {
        subsonic_response_xml(&format!(
            r#"    <users>
        <user username="{}" email="" scrobblingEnabled="true" maxBitRate="0"
              adminRole="{}" settingsRole="{}" downloadRole="true" uploadRole="{}"
              playlistRole="true" coverArtRole="true" commentRole="false" podcastRole="false"
              streamRole="true" jukeboxRole="false" shareRole="false" videoConversionRole="false">
            <folder>1</folder>
        </user>
    </users>"#,
            escape_xml(&user.username),
            is_admin,
            is_admin,
            is_admin,
        ))
    }
}

// GET/POST /rest/getScanStatus.view
pub async fn get_scan_status(
    State(state): State<AppState>,
    Query(params): Query<SubsonicParams>,
) -> Response {
    let f = params.f.as_deref().unwrap_or("xml");
    let user = match authenticate(&state, &params).await {
        Ok(u) => u,
        Err(e) => return subsonic_error(f, ERROR_AUTH, &e),
    };

    let user_status = state.get_user_scan_status(&user.id).await;
    let (is_scanning, count) = {
        let s = user_status.lock().unwrap();
        (s.is_scanning, s.files_scanned)
    };

    if f == "json" {
        subsonic_response_json(serde_json::json!({
            "scanStatus": {
                "scanning": is_scanning,
                "count": count
            }
        }))
    } else {
        subsonic_response_xml(&format!(
            r#"    <scanStatus scanning="{}" count="{}"/>"#,
            is_scanning, count
        ))
    }
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

    let song_id_str = match params.id.as_deref().or(params.songId.as_deref()) {
        Some(id) => id,
        None => return subsonic_error(f, ERROR_MISSING_PARAM, "Missing track parameter 'id'"),
    };

    let song_id = match parse_track_id(song_id_str) {
        Some(id) => id,
        None => return subsonic_error(f, ERROR_GENERIC, "Invalid song ID"),
    };

    if params.submission.unwrap_or(true) {
        let play_recorded = sqlx::query(
            "INSERT INTO play_log (user_id, track_id, duration_played, client) VALUES (?, ?, ?, ?)"
        )
        .bind(&user.id)
        .bind(song_id)
        .bind(0)
        .bind(params.c.as_deref().unwrap_or("Subsonic Client"))
        .execute(&user_pool)
        .await;

        if play_recorded.is_ok() {
            if let Some(ref token) = user.listenbrainz_token {
                let pool = user_pool.clone();
                let client_token = token.clone();
                tokio::spawn(async move {
                    if let Err(e) = scrobble_to_listenbrainz(&pool, song_id, &client_token).await {
                        tracing::error!("ListenBrainz scrobbling failed: {}", e);
                    }
                });
            }
        }
    }

    if f == "json" {
        subsonic_response_json(serde_json::json!({}))
    } else {
        subsonic_response_xml("")
    }
}

// GET/POST /rest/updateTags.view (OpenSubsonic Extension)
pub async fn update_tags(
    State(state): State<AppState>,
    Query(params): Query<SubsonicParams>,
) -> Response {
    let f = params.f.as_deref().unwrap_or("xml");
    let user = match authenticate(&state, &params).await {
        Ok(u) => u,
        Err(e) => return subsonic_error(f, ERROR_AUTH, &e),
    };

    let song_id_str = match params.id.as_deref().or(params.songId.as_deref()) {
        Some(id) => id,
        None => return subsonic_error(f, ERROR_MISSING_PARAM, "Missing track parameter 'id'"),
    };

    let song_id = match parse_track_id(song_id_str) {
        Some(id) => id,
        None => return subsonic_error(f, ERROR_GENERIC, "Invalid song ID"),
    };

    let req = crate::api::tracks::UpdateMetadataRequest {
        title: params.title.clone(),
        artist: params.artist.clone(),
        album: params.album.clone(),
        album_artist: None,
        composer: None,
        year: params.year.clone(),
        genre: params.genre.clone(),
        track_number: params.trackNumber,
        disc_number: params.discNumber,
        comment: params.comment.clone(),
        bpm: None,
        isrc: None,
        lyrics: None,
    };

    let claims = crate::auth::Claims {
        sub: user.id,
        username: user.username,
        role: "User".to_string(),
        exp: 0,
    };

    match crate::api::tracks::metadata::update_track_metadata_inner(&state, &claims, song_id, req).await {
        Ok(_) => {
            if f == "json" {
                subsonic_response_json(serde_json::json!({}))
            } else {
                subsonic_response_xml("")
            }
        },
        Err((_status, err_msg)) => subsonic_error(f, ERROR_GENERIC, &err_msg),
    }
}

pub async fn scrobble_to_listenbrainz(
    pool: &sqlx::SqlitePool,
    track_id: i64,
    token: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let track = sqlx::query(
        "SELECT title, artist, album FROM tracks WHERE id = ?"
    )
    .bind(track_id)
    .fetch_optional(pool)
    .await?
    .ok_or("Track not found")?;

    let title: String = track.try_get("title").unwrap_or_default();
    let artist: String = track.try_get("artist").unwrap_or_default();
    let album: String = track.try_get("album").unwrap_or_default();

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

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
