use async_trait::async_trait;
use bili_dm_lib::{
    core::error::{AppError, AppResult},
    media::{files::MediaFile, stream_server::MediaServer},
    torrent::{
        models::{DownloadStats, TorrentCatalog},
        SeekableReader, TorrentEngine,
    },
};
use std::{io::Cursor, sync::Arc};

struct MemorySource;
#[async_trait]
impl TorrentEngine for MemorySource {
    async fn resolve(&self, _: &str) -> AppResult<TorrentCatalog> {
        Err(AppError::new("TEST", "not used"))
    }
    async fn cancel_resolution(&self) {}
    async fn prepare_file(&self, _: &str, _: usize) -> AppResult<MediaFile> {
        Ok(MediaFile::new(0, "test.mkv".into(), 100))
    }
    async fn open_file(&self, _: &str, _: usize) -> AppResult<Box<dyn SeekableReader>> {
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
    let server = MediaServer::start(Arc::new(MemorySource)).await?;
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
