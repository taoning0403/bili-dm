use crate::{
    core::error::AppResult,
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
}

impl AppService {
    pub fn new(
        torrent: Arc<dyn TorrentEngine>,
        player: Arc<dyn crate::player::PlayerBackend>,
    ) -> Self {
        Self {
            playback: super::playback_service::PlaybackService::new(torrent.clone(), player),
            torrent,
        }
    }

    pub async fn resolve_magnet(&self, magnet: &str) -> AppResult<TorrentCatalog> {
        self.torrent.resolve(magnet).await
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
