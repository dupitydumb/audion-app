pub mod metadata;
pub mod watcher;

pub use metadata::{extract_metadata, TrackMetadata};
pub use watcher::start_file_watcher;
