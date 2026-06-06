use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::Serialize;
use crate::state::AppState;
use crate::auth::Claims;
use crate::tunnel::{TunnelConfig, TunnelStatus};

#[derive(Serialize)]
pub struct TunnelInfoResponse {
    pub config: TunnelConfig,
    pub status: TunnelStatus,
}

pub async fn get_tunnel_info(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<TunnelInfoResponse>, (StatusCode, String)> {
    if claims.role != "Admin" {
        return Err((StatusCode::FORBIDDEN, "Admin privileges required".to_string()));
    }

    let manager = state.tunnel_manager.lock().await;
    Ok(Json(TunnelInfoResponse {
        config: manager.get_config(),
        status: manager.get_status(),
    }))
}

pub async fn toggle_tunnel(
    claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<TunnelInfoResponse>, (StatusCode, String)> {
    if claims.role != "Admin" {
        return Err((StatusCode::FORBIDDEN, "Admin privileges required".to_string()));
    }

    // Clone the lock so we don't hold the lock across the .await boundary
    // Because .await yields the thread, holding a std::sync::MutexGuard across it is not Send and causes compiler errors!
    // Since TunnelManager's toggle takes &mut self, we should use tokio::sync::Mutex or simply lock, do what we need, or run it.
    // Wait! Let's lock, clone the manager or just use a standard async-safe approach.
    // Wait! If AppState.tunnel_manager is Arc<tokio::sync::Mutex<TunnelManager>>, then we can easily hold the guard across .await!
    // Let's use `Arc<tokio::sync::Mutex<TunnelManager>>` instead of `std::sync::Mutex`. That is Async-safe and prevents standard borrow checker/lock issues.
    // Yes! Let's check state.rs: we used std::sync::Mutex for ScanStatus and FetcherStatus because they are only read/written in brief sync calls.
    // But TunnelManager does process spawning and TCP lookup, so it is async. So using `Arc<tokio::sync::Mutex<TunnelManager>>` is the perfect async-safe design!
    
    let mut manager = state.tunnel_manager.lock().await;
    manager.toggle().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(TunnelInfoResponse {
        config: manager.get_config(),
        status: manager.get_status(),
    }))
}

pub async fn update_tunnel_config(
    claims: Claims,
    State(state): State<AppState>,
    Json(new_config): Json<TunnelConfig>,
) -> Result<Json<TunnelInfoResponse>, (StatusCode, String)> {
    if claims.role != "Admin" {
        return Err((StatusCode::FORBIDDEN, "Admin privileges required".to_string()));
    }

    let mut manager = state.tunnel_manager.lock().await;
    let was_active = manager.get_status().active;

    manager.update_config(new_config).await;

    // If it was active, restart it with new config
    if was_active {
        let _ = manager.toggle().await; // Stop
        let _ = manager.toggle().await; // Start
    }

    Ok(Json(TunnelInfoResponse {
        config: manager.get_config(),
        status: manager.get_status(),
    }))
}
