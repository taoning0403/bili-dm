//! Local library boundary. SQL and migrations are private to the SQLite adapter.
pub mod models;
pub mod sqlite;

use crate::{core::error::AppResult, torrent::models::TorrentCatalog};
use async_trait::async_trait;
use models::{MediaRecord, PlaybackPreferences, PlaybackProgress, TorrentRecord};

#[async_trait]
pub trait LibraryRepository: Send + Sync {
    async fn save_catalog(&self, magnet: &str, catalog: &TorrentCatalog) -> AppResult<()>;
    async fn recent_torrents(&self) -> AppResult<Vec<TorrentRecord>>;
    async fn save_media(&self, media: MediaRecord) -> AppResult<()>;
    async fn pause_task(&self, torrent_id: &str) -> AppResult<()>;
    async fn progress(&self, path: &str) -> AppResult<Option<PlaybackProgress>>;
    async fn save_progress(&self, progress: PlaybackProgress) -> AppResult<()>;
    async fn preferences(&self) -> AppResult<PlaybackPreferences>;
    async fn save_preferences(&self, preferences: PlaybackPreferences) -> AppResult<()>;
}
