use tokio::sync::broadcast;
use serde::{Serialize, Deserialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEvent {
    pub id: i64,
    pub event_type: String, // 'track.added', 'track.deleted', 'playlist.updated', 'liked.changed', etc.
    pub payload: Value,
    pub created_at: String,
}

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<ServerEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        // Create a broadcast channel with a buffer size of 100
        let (sender, _) = broadcast::channel(100);
        Self { sender }
    }

    pub fn broadcast(&self, event: ServerEvent) {
        // If there are no active receivers, sending will return an Err.
        // We can safely ignore this error.
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.sender.subscribe()
    }
}
