mod config;
mod db;
mod state;
mod auth;
mod events;
mod scanner;
mod api;

use std::sync::Arc;
use tracing::{info, warn, error};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

use crate::config::Config;
use crate::db::setup_database;
use crate::state::AppState;
use crate::events::EventBus;
use crate::api::create_router;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    info!("Starting Audion Server...");

    // Load configuration
    let config = Config::load();
    info!("Data directory: {:?}", config.data_dir);
    info!("Port: {}", config.port);

    // Setup database
    let pool = setup_database(&config.db_path()).await?;

    // Bootstrap admin user
    let user_count: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    if user_count == 0 {
        info!("No users found in database. Bootstrapping admin user...");
        let admin_id = Uuid::new_v4().to_string();
        
        let hash = auth::hash_password(&config.admin_password_raw)
            .map_err(|e| format!("Failed to hash password: {}", e))?;
        
        sqlx::query("INSERT INTO users (id, username, password_hash) VALUES (?, ?, ?)")
            .bind(&admin_id)
            .bind(&config.admin_user)
            .bind(&hash)
            .execute(&pool)
            .await?;

        info!("Admin user '{}' bootstrapped successfully.", config.admin_user);
    } else {
        info!("Database contains existing users. Skipping admin bootstrap.");
    }

    // Initialize Event Bus
    let event_bus = EventBus::new();

    // Create AppState
    let state = AppState::new(pool, config.clone(), event_bus);

    // Create router
    let router = create_router(state);

    // Start server
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("Server listening on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
