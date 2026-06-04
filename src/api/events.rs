use axum::{
    extract::State,
    http::HeaderMap,
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::stream::StreamExt;
use std::convert::Infallible;
use tracing::info;
use sqlx::Row;

use crate::state::AppState;
use crate::auth::Claims;
use crate::events::ServerEvent;

pub async fn handle_events(
    _claims: Claims,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let last_event_id: Option<i64> = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok());

    info!("Client connected to SSE. Last-Event-ID: {:?}", last_event_id);

    let mut replayed_events = Vec::new();
    if let Some(last_id) = last_event_id {
        if let Ok(rows) = sqlx::query(
            "SELECT id, event_type, payload, created_at FROM events WHERE id > ? ORDER BY id ASC"
        )
        .bind(last_id)
        .fetch_all(&state.pool)
        .await {
            for row in rows {
                let id: i64 = row.get("id");
                let event_type: String = row.get("event_type");
                let payload_str: String = row.get("payload");
                let created_at: Option<String> = row.get("created_at");
                if let Ok(payload_val) = serde_json::from_str::<serde_json::Value>(&payload_str) {
                    replayed_events.push(ServerEvent {
                        id,
                        event_type,
                        payload: payload_val,
                        created_at: created_at.unwrap_or_default(),
                    });
                }
            }
        }
    }

    let rx = state.event_bus.subscribe();
    let live_stream = tokio_stream::wrappers::BroadcastStream::new(rx)
        .filter_map(|res| std::future::ready(match res {
            Ok(event) => Some(event),
            Err(_) => None, // Ignore lag errors
        }));

    let replayed_stream = tokio_stream::iter(replayed_events);

    let stream = replayed_stream.chain(live_stream)
        .map(|event| {
            let event_id = event.id.to_string();
            let event_type = event.event_type.clone();
            let event_json = serde_json::to_string(&event).unwrap_or_default();
            
            Ok(Event::default()
                .id(event_id)
                .event(event_type)
                .data(event_json))
        });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
