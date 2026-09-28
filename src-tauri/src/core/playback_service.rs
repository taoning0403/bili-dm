use super::error::{AppError, AppResult};
use crate::{
    media::{
        files::{classify, FileKind},
        stream_server::MediaServer,
    },
    player::{PlayerBackend, PlayerControl, PlayerSnapshot},
    torrent::{models::DownloadStats, TorrentEngine},
};
use serde::Serialize;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{Mutex, OnceCell, RwLock};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveMedia {
    pub title: String,
    pub source: &'static str,
    pub torrent_id: Option<String>,
    pub file_index: Option<usize>,
    pub local_path: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackState {
    pub media: Option<ActiveMedia>,
    pub player: PlayerSnapshot,
    pub download: Option<DownloadStats>,
}

pub struct PlaybackService {
    torrent: Arc<dyn TorrentEngine>,
    player: Arc<dyn PlayerBackend>,
    server: OnceCell<MediaServer>,
    current: RwLock<Option<ActiveMedia>>,
    operation: Mutex<()>,
}

impl PlaybackService {
    pub fn new(torrent: Arc<dyn TorrentEngine>, player: Arc<dyn PlayerBackend>) -> Self {
        Self {
            torrent,
            player,
            server: OnceCell::new(),
            current: RwLock::new(None),
            operation: Mutex::new(()),
        }
    }

    pub async fn play_torrent(&self, id: &str, index: usize) -> AppResult<ActiveMedia> {
        let _operation = self.operation.lock().await;
        self.stop_inner().await?;
        let result = async {
            let file = self.torrent.prepare_file(id, index).await?;
            let server = self
                .server
                .get_or_try_init(|| MediaServer::start(self.torrent.clone()))
                .await?;
            let url = server.source_url(id.to_owned(), file.clone()).await;
            self.player.load(&url).await?;
            Ok(ActiveMedia {
                title: file.path,
                source: "torrent",
                torrent_id: Some(id.to_owned()),
                file_index: Some(index),
                local_path: None,
            })
        }
        .await;
        match result {
            Ok(media) => {
                *self.current.write().await = Some(media.clone());
                Ok(media)
            }
            Err(error) => {
                let _ = self.stop_inner().await;
                Err(error)
            }
        }
    }

    pub async fn play_local(&self, path: PathBuf) -> AppResult<ActiveMedia> {
        let path = tokio::fs::canonicalize(path).await.map_err(AppError::io)?;
        let info = tokio::fs::metadata(&path).await.map_err(AppError::io)?;
        let path_text = path
            .to_str()
            .ok_or_else(|| AppError::new("INVALID_PATH", "暂不支持非 UTF-8 文件路径。"))?;
        if !info.is_file() || info.len() == 0 || classify(path_text) != FileKind::Video {
            return Err(AppError::new(
                "INVALID_FILE",
                "请选择非空的 mp4、mkv、avi、webm、mov 等视频文件。",
            ));
        }
        let _operation = self.operation.lock().await;
        self.stop_inner().await?;
        self.player.load(path_text).await?;
        let media = ActiveMedia {
            title: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            source: "local",
            torrent_id: None,
            file_index: None,
            local_path: Some(path_text.to_owned()),
        };
        *self.current.write().await = Some(media.clone());
        Ok(media)
    }

    pub async fn control(&self, control: PlayerControl) -> AppResult<()> {
        let _operation = self.operation.lock().await;
        if matches!(control, PlayerControl::Stop) {
            self.stop_inner().await
        } else {
            self.player.control(control).await
        }
    }

    async fn stop_inner(&self) -> AppResult<()> {
        if self.player.control(PlayerControl::Stop).await.is_err() {
            self.player.shutdown().await;
        }
        if let Some(server) = self.server.get() {
            server.clear().await;
        }
        *self.current.write().await = None;
        self.torrent.pause_download().await
    }

    pub async fn snapshot(&self) -> AppResult<PlaybackState> {
        let player = self.player.snapshot().await?;
        // Closing mpv directly must also stop the selected torrent download.
        if !player.running && self.current.read().await.is_some() {
            if let Ok(_operation) = self.operation.try_lock() {
                if !self.player.snapshot().await?.running {
                    self.stop_inner().await?;
                }
            }
        }
        Ok(PlaybackState {
            media: self.current.read().await.clone(),
            player,
            download: self.torrent.download_stats().await,
        })
    }

    pub async fn shutdown(&self) {
        self.player.shutdown().await;
        if let Some(server) = self.server.get() {
            server.clear().await;
        }
        self.torrent.shutdown().await;
    }
}
