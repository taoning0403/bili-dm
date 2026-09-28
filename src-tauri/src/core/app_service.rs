use crate::{
    core::error::AppResult,
    database::{models::TorrentRecord, LibraryRepository},
    torrent::{models::TorrentCatalog, TorrentEngine},
};
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub app_name: &'static str,
    pub app_version: &'static str,
    pub platform: &'static str,
    pub architecture: &'static str,
}

/// A small application service; future playback orchestration belongs in Core,
/// not in commands or UI components.
pub struct AppService {
    pub torrent: Arc<dyn TorrentEngine>,
    pub playback: super::playback_service::PlaybackService,
    library: Arc<dyn LibraryRepository>,
}

impl AppService {
    pub fn new(
        torrent: Arc<dyn TorrentEngine>,
        player: Arc<dyn crate::player::PlayerBackend>,
        library: Arc<dyn LibraryRepository>,
    ) -> Self {
        Self {
            playback: super::playback_service::PlaybackService::new(
                torrent.clone(),
                player,
                library.clone(),
            ),
            torrent,
            library,
        }
    }

    pub async fn resolve_magnet(&self, magnet: &str) -> AppResult<TorrentCatalog> {
        let catalog = self.torrent.resolve(magnet).await?;
        self.library.save_catalog(magnet, &catalog).await?;
        Ok(catalog)
    }

    pub async fn recent_torrents(&self) -> AppResult<Vec<TorrentRecord>> {
        self.library.recent_torrents().await
    }

    pub fn runtime_info(&self) -> RuntimeInfo {
        RuntimeInfo {
            app_name: "Bili DM",
            app_version: env!("CARGO_PKG_VERSION"),
            platform: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
        }
    }
}
