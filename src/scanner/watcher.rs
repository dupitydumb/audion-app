use notify::{Watcher, RecommendedWatcher, RecursiveMode, EventKind};
use crate::state::AppState;

pub fn start_file_watcher(state: AppState) {
    let music_dir = state.config.music_dir();
    
    // Ensure music directory exists
    if !music_dir.exists() {
        std::fs::create_dir_all(&music_dir).ok();
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
            if w.watch(&music_dir, RecursiveMode::Recursive).is_err() {
                tracing::error!("Failed to watch directory {:?}", music_dir);
                return;
            }
            tracing::info!("Directory watcher started for {:?}", music_dir);
        } else {
            tracing::error!("Failed to create directory watcher");
            return;
        }
        
        // Debounce logic: wait for 5 seconds of inactivity before launching library scan
        let mut timer = Box::pin(tokio::time::sleep(tokio::time::Duration::from_secs(0)));
        let mut pending = false;
        
        loop {
            tokio::select! {
                Some(event) = rx.recv() => {
                    let is_relevant = match event.kind {
                        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(_) => true,
                        _ => false,
                    };
                    if is_relevant {
                        pending = true;
                        timer = Box::pin(tokio::time::sleep(tokio::time::Duration::from_secs(5)));
                    }
                }
                _ = &mut timer, if pending => {
                    pending = false;
                    tracing::info!("Filesystem change detected. Triggering library auto-scan...");
                    crate::api::library::trigger_auto_scan(state_clone.clone());
                }
            }
        }
    });
}
