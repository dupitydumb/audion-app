use super::*;

pub async fn clean_library(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    claims.require_non_stream_only().map_err(|(s, m)| (s, m.to_string()))?;
    let user_pool = state.get_user_pool(&claims.sub).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("Pruning orphan records from database library...");
    let tracks = sqlx::query("SELECT id, path, track_cover_path FROM tracks")
        .fetch_all(&user_pool)
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
                .execute(&user_pool)
                .await
                .ok();

            let payload = serde_json::json!({ "id": id });
            let payload_str = payload.to_string();
            let event_type = "track.deleted";

            if let Ok(er) = sqlx::query("INSERT INTO events (event_type, payload) VALUES (?, ?)")
                .bind(event_type)
                .bind(&payload_str)
                .execute(&user_pool)
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
    .execute(&user_pool)
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
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    claims.require_non_stream_only().map_err(|(s, m)| (s, m.to_string()))?;
    let user_pool = state.get_user_pool(&claims.sub).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("Resetting library database...");

    sqlx::query("DELETE FROM tracks").execute(&user_pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query("DELETE FROM albums").execute(&user_pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query("DELETE FROM playlists").execute(&user_pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query("DELETE FROM liked_tracks").execute(&user_pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let artwork_dir = state.config.user_artwork_dir(&claims.sub);
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
