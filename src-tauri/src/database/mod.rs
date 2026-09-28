//! Local library boundary. SQL and migrations are private to the SQLite adapter.
pub mod models;
pub mod sqlite;

use crate::{core::error::AppResult, torrent::models::TorrentCatalog};
use async_trait::async_trait;
use models::{MediaRecord, TorrentRecord};

#[async_trait]
pub trait LibraryRepository: Send + Sync {
    async fn save_catalog(&self, magnet: &str, catalog: &TorrentCatalog) -> AppResult<()>;
    async fn recent_torrents(&self) -> AppResult<Vec<TorrentRecord>>;
    async fn save_media(&self, media: MediaRecord) -> AppResult<()>;
    async fn pause_task(&self, torrent_id: &str) -> AppResult<()>;
}
