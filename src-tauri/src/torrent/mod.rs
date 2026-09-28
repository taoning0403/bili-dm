//! Engine-independent torrent contracts. No player or Tauri dependencies.

pub mod models;
pub mod rqbit;

use crate::core::error::AppResult;
use async_trait::async_trait;
use models::TorrentCatalog;

#[async_trait]
pub trait TorrentEngine: Send + Sync {
    async fn resolve(&self, magnet: &str) -> AppResult<TorrentCatalog>;
    async fn cancel_resolution(&self);
    async fn shutdown(&self);
}
