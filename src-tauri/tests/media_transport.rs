use async_trait::async_trait;
use bili_dm_lib::{
    core::error::{AppError, AppResult},
    media::{files::MediaFile, stream_server::MediaServer},
    torrent::{
        models::{DownloadStats, TorrentCatalog},
        SeekableReader, TorrentEngine,
    },
};
use std::{
    io::{Cursor, SeekFrom},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::{Context, Poll},
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncSeek, ReadBuf};

struct MemorySource(Option<Arc<AtomicBool>>);
struct StalledReader(Arc<AtomicBool>);
impl AsyncRead for StalledReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        _: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Poll::Pending
    }
}
impl AsyncSeek for StalledReader {
    fn start_seek(self: Pin<&mut Self>, _: SeekFrom) -> std::io::Result<()> {
        Ok(())
    }
    fn poll_complete(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
        Poll::Ready(Ok(0))
    }
}
impl Drop for StalledReader {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
#[async_trait]
impl TorrentEngine for MemorySource {
    async fn resolve(&self, _: &str) -> AppResult<TorrentCatalog> {
        Err(AppError::new("TEST", "not used"))
    }
    async fn cancel_resolution(&self) {}
    async fn catalog(&self, _: &str) -> AppResult<TorrentCatalog> {
        self.resolve("").await
    }
    async fn import_torrent(&self, _: Vec<u8>) -> AppResult<TorrentCatalog> {
        self.resolve("").await
    }
    async fn prefetch(&self, _: &str, _: Option<usize>) -> AppResult<()> {
        Ok(())
    }
    async fn subtitle_path(&self, _: &str, _: usize) -> AppResult<Option<std::path::PathBuf>> {
        Ok(None)
    }
    async fn prepare_file(&self, _: &str, _: usize) -> AppResult<MediaFile> {
        Ok(MediaFile::new(0, "test.mkv".into(), 100))
    }
    async fn open_file(&self, _: &str, _: usize) -> AppResult<Box<dyn SeekableReader>> {
        if let Some(dropped) = &self.0 {
            return Ok(Box::new(StalledReader(dropped.clone())));
        }
        Ok(Box::new(Cursor::new((0..100u8).collect::<Vec<_>>())))
    }
    async fn download_stats(&self) -> Option<DownloadStats> {
        None
    }
    async fn pause_download(&self) -> AppResult<()> {
        Ok(())
    }
    async fn shutdown(&self) {}
}

#[tokio::test]
async fn real_http_transport_handles_seek_head_and_expired_sources(
) -> Result<(), Box<dyn std::error::Error>> {
    let server = MediaServer::start(Arc::new(MemorySource(None))).await?;
    let url = server
        .source_url("fixture".into(), MediaFile::new(0, "test.mkv".into(), 100))
        .await;
    let client = reqwest::Client::builder().no_proxy().build()?;
    let head = client.head(&url).send().await?;
    assert_eq!(head.status(), 200);
    assert_eq!(head.headers()["content-length"], "100");
    let response = client
        .get(&url)
        .header("Range", "bytes=40-49")
        .send()
        .await?;
    assert_eq!(response.status(), 206);
    assert_eq!(response.headers()["content-range"], "bytes 40-49/100");
    assert_eq!(
        response.bytes().await?.as_ref(),
        &(40..50u8).collect::<Vec<_>>()
    );
    assert_eq!(
        client
            .get(&url)
            .header("Range", "bytes=100-")
            .send()
            .await?
            .status(),
        416
    );
    assert_eq!(
        client.get(format!("{url}-wrong")).send().await?.status(),
        404
    );
    server.clear().await;
    assert_eq!(client.get(&url).send().await?.status(), 404);
    Ok(())
}

#[tokio::test]
async fn revoking_source_releases_a_reader_waiting_on_missing_pieces(
) -> Result<(), Box<dyn std::error::Error>> {
    let dropped = Arc::new(AtomicBool::new(false));
    let server = MediaServer::start(Arc::new(MemorySource(Some(dropped.clone())))).await?;
    let url = server
        .source_url(
            "fixture".into(),
            MediaFile::new(0, "stalled.mkv".into(), 100),
        )
        .await;
    let client = reqwest::Client::builder().no_proxy().build()?;
    let response = tokio::time::timeout(Duration::from_secs(2), client.get(&url).send()).await??;
    assert_eq!(response.status(), 200);
    server.clear().await;
    // Cancellation must end the incomplete HTTP body, even though the reader
    // never wakes itself. Dropping that reader releases rqbit piece priorities.
    assert!(
        tokio::time::timeout(Duration::from_secs(2), response.bytes())
            .await?
            .is_err()
    );
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(client.get(&url).send().await?.status(), 404);
    Ok(())
}
