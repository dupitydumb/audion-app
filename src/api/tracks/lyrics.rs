use super::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct LyricsResponse {
    pub lyrics: Option<String>,
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
