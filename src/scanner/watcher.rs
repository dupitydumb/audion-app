use notify::{Watcher, RecommendedWatcher, RecursiveMode, EventKind};
use crate::state::AppState;
use std::collections::HashSet;

pub fn start_file_watcher(state: AppState) {
    let users_root = state.config.data_dir.join("users");
    
    // Ensure users directory exists
    if !users_root.exists() {
        std::fs::create_dir_all(&users_root).ok();
    }
    
    let state_clone = state.clone();
    tokio::spawn(async move {
        // Channel for communicating filesystem events
        let (tx, mut rx) = tokio::sync::mpsc::channel(100);
        
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            },
            notify::Config::default(),
        );
        
        if let Ok(ref mut w) = watcher {
            if w.watch(&users_root, RecursiveMode::Recursive).is_err() {
                tracing::error!("Failed to watch directory {:?}", users_root);
                return;
            }
            tracing::info!("Directory watcher started for {:?}", users_root);
        } else {
            tracing::error!("Failed to create directory watcher");
            return;
        }
        
        // Debounce logic: wait for 5 seconds of inactivity before launching library scan.
        // We track a set of pending user IDs to scan.
        let mut timer = Box::pin(tokio::time::sleep(tokio::time::Duration::from_secs(0)));
        let mut pending_users = HashSet::new();
        
        loop {
            tokio::select! {
                Some(event) = rx.recv() => {
                    let is_relevant = match event.kind {
                        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(_) => true,
                        _ => false,
                    };
                    if is_relevant {
                        for path in event.paths {
                            if let Ok(rel_path) = path.strip_prefix(&users_root) {
                                let mut components = rel_path.components();
                                if let Some(std::path::Component::Normal(user_id_os)) = components.next() {
                                    if let Some(user_id) = user_id_os.to_str() {
                                        if let Some(std::path::Component::Normal(music_os)) = components.next() {
                                            if music_os == "music" {
                                                pending_users.insert(user_id.to_string());
                                                timer = Box::pin(tokio::time::sleep(tokio::time::Duration::from_secs(5)));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                _ = &mut timer, if !pending_users.is_empty() => {
                    let users_to_scan: Vec<String> = pending_users.drain().collect();
                    for user_id in users_to_scan {
                        tracing::info!("Filesystem change detected for user {}. Triggering library auto-scan...", user_id);
                        let state_cloned = state_clone.clone();
                        tokio::spawn(async move {
                            crate::api::library::trigger_auto_scan(state_cloned, user_id).await;
                        });
                    }
                }
            }
        }
    });
}
