//! Engine-independent torrent contracts. No player or Tauri dependencies.

pub mod models;
pub mod rqbit;

use crate::core::error::AppResult;
use crate::media::files::MediaFile;
use async_trait::async_trait;
use models::TorrentCatalog;
use tokio::io::{AsyncRead, AsyncSeek};

pub trait SeekableReader: AsyncRead + AsyncSeek + Send + Unpin {}
impl<T: AsyncRead + AsyncSeek + Send + Unpin> SeekableReader for T {}

#[async_trait]
pub trait TorrentEngine: Send + Sync {
    async fn resolve(&self, magnet: &str) -> AppResult<TorrentCatalog>;
    async fn cancel_resolution(&self);
    async fn prepare_file(&self, id: &str, index: usize) -> AppResult<MediaFile>;
    async fn open_file(&self, id: &str, index: usize) -> AppResult<Box<dyn SeekableReader>>;
    async fn download_stats(&self) -> Option<models::DownloadStats>;
    async fn pause_download(&self) -> AppResult<()>;
    async fn shutdown(&self);
}
