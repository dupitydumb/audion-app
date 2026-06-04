use sqlx::SqlitePool;
use crate::config::Config;
use crate::events::EventBus;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Config,
    pub event_bus: EventBus,
}

impl AppState {
    pub fn new(pool: SqlitePool, config: Config, event_bus: EventBus) -> Self {
        Self {
            pool,
            config,
            event_bus,
        }
    }
}
