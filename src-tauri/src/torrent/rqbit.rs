use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, DhtSessionConfig, ListOnlyResponse, Magnet,
    Session, SessionOptions,
};
use tokio::sync::{Mutex, OnceCell};
use tokio_util::sync::CancellationToken;

use super::{models::TorrentCatalog, TorrentEngine};
use crate::core::error::{AppError, AppResult};
use crate::media::files::{preferred_video, MediaFile};

pub struct ResolvedTorrent {
    pub catalog: TorrentCatalog,
    pub metadata: ListOnlyResponse,
}

pub struct RqbitEngine {
    cache_dir: PathBuf,
    session: OnceCell<Arc<Session>>,
    resolved: Mutex<HashMap<String, ResolvedTorrent>>,
    resolution: Mutex<Option<CancellationToken>>,
    active: Mutex<Option<ActiveDownload>>,
}

struct ActiveDownload {
    id: String,
    file: MediaFile,
    handle: Arc<librqbit::ManagedTorrent>,
}

impl RqbitEngine {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            session: OnceCell::new(),
            resolved: Mutex::new(HashMap::new()),
            resolution: Mutex::new(None),
            active: Mutex::new(None),
        }
    }

    async fn session(&self) -> AppResult<&Arc<Session>> {
        self.session
            .get_or_try_init(|| async {
                tokio::fs::create_dir_all(&self.cache_dir)
                    .await
                    .map_err(AppError::io)?;
                Session::new_with_opts(
                    self.cache_dir.clone(),
                    SessionOptions {
                        dht: Some(DhtSessionConfig {
                            persistence: None,
                            ..Default::default()
                        }),
                        disable_local_service_discovery: true,
                        peer_limit: Some(80),
                        ..Default::default()
                    },
                )
                .await
                .map_err(|e| AppError::new("TORRENT_INIT", format!("无法启动 torrent 引擎：{e:#}")))
            })
            .await
    }

    async fn resolve_inner(&self, magnet: &str) -> AppResult<TorrentCatalog> {
        let session = self.session().await?;
        let response = session
            .add_torrent(
                AddTorrent::from_url(magnet),
                Some(AddTorrentOptions {
                    list_only: true,
                    ..Default::default()
                }),
            )
            .await
            .map_err(|e| AppError::new("METADATA", format!("无法读取磁力元数据：{e:#}")))?;
        let metadata = match response {
            AddTorrentResponse::ListOnly(metadata) => metadata,
            AddTorrentResponse::AlreadyManaged(_, handle) => {
                return self
                    .resolved
                    .lock()
                    .await
                    .get(&handle.info_hash().as_string())
                    .map(|entry| entry.catalog.clone())
                    .ok_or_else(|| AppError::new("METADATA", "任务已存在，但目录信息不可用。"));
            }
            AddTorrentResponse::Added(_, _) => {
                return Err(AppError::new("METADATA", "引擎意外启动了下载任务。"))
            }
        };
        let id = metadata.info_hash.as_string();
        let files: Vec<_> = metadata
            .info
            .iter_file_details()
            .enumerate()
            .map(|(index, file)| MediaFile::new(index, file.filename.to_string(), file.len))
            .collect();
        let catalog = TorrentCatalog {
            id: id.clone(),
            name: metadata
                .info
                .name()
                .map(|name| name.into_owned())
                .unwrap_or_else(|| id.clone()),
            suggested_file_index: preferred_video(&files),
            files,
        };
        self.resolved.lock().await.insert(
            id,
            ResolvedTorrent {
                catalog: catalog.clone(),
                metadata,
            },
        );
        Ok(catalog)
    }
}

pub fn validate_magnet(value: &str) -> AppResult<String> {
    let value = value.trim();
    if value.len() > 32_768 || !value.starts_with("magnet:?") {
        return Err(AppError::new(
            "INVALID_MAGNET",
            "请输入有效的 magnet:?xt=urn:btih:… 链接。",
        ));
    }
    let mut url = url::Url::parse(value)
        .map_err(|_| AppError::new("INVALID_MAGNET", "磁力链接格式错误。"))?;
    // Selection comes from the user, never from an eagerly expanded URI `so` range.
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| matches!(key.as_ref(), "xt" | "tr" | "dn"))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    url.set_query(None);
    url.query_pairs_mut().extend_pairs(pairs);
    let normalized = url.to_string();
    let parsed = Magnet::parse(&normalized)
        .map_err(|_| AppError::new("INVALID_MAGNET", "磁力链接缺少有效的 btih 信息哈希。"))?;
    if parsed.as_id20().is_none() {
        return Err(AppError::new(
            "UNSUPPORTED_MAGNET",
            "当前支持 BT v1 / hybrid 磁力，暂不支持纯 BT v2。",
        ));
    }
    Ok(normalized)
}

#[async_trait]
impl TorrentEngine for RqbitEngine {
    async fn resolve(&self, magnet: &str) -> AppResult<TorrentCatalog> {
        let magnet = validate_magnet(magnet)?;
        let cancel = CancellationToken::new();
        {
            let mut pending = self.resolution.lock().await;
            if pending.is_some() {
                return Err(AppError::new("BUSY", "正在解析另一条磁力链接，请先取消。"));
            }
            *pending = Some(cancel.clone());
        }
        let result = tokio::select! {
            _ = cancel.cancelled() => Err(AppError::new("CANCELLED", "已取消磁力解析。")),
            result = tokio::time::timeout(Duration::from_secs(120), self.resolve_inner(&magnet)) => {
                result.unwrap_or_else(|_| Err(AppError::new("METADATA_TIMEOUT", "120 秒内未取得元数据。请检查网络或稍后重试，资源可能暂时没有在线节点。")))
            }
        };
        *self.resolution.lock().await = None;
        result
    }

    async fn cancel_resolution(&self) {
        if let Some(cancel) = self.resolution.lock().await.as_ref() {
            cancel.cancel();
        }
    }

    async fn prepare_file(&self, id: &str, index: usize) -> AppResult<MediaFile> {
        let (bytes, peers, file) = {
            let entries = self.resolved.lock().await;
            let entry = entries
                .get(id)
                .ok_or_else(|| AppError::new("NOT_FOUND", "请先加载磁力目录。"))?;
            let file = entry
                .catalog
                .files
                .iter()
                .find(|file| file.index == index)
                .filter(|file| file.kind == crate::media::files::FileKind::Video && file.size > 0)
                .ok_or_else(|| AppError::new("INVALID_FILE", "请选择非空的视频文件。"))?
                .clone();
            (
                entry.metadata.torrent_bytes.clone(),
                entry.metadata.seen_peers.clone(),
                file,
            )
        };
        let session = self.session().await?;
        let response = session
            .add_torrent(
                AddTorrent::from_bytes(bytes),
                Some(AddTorrentOptions {
                    only_files: Some(vec![index]),
                    initial_peers: Some(peers),
                    sub_folder: Some(id.to_owned()),
                    overwrite: true,
                    ..Default::default()
                }),
            )
            .await
            .map_err(|error| AppError::new("TORRENT_START", format!("无法开始下载：{error:#}")))?;
        let handle = response
            .into_handle()
            .ok_or_else(|| AppError::new("TORRENT_START", "引擎没有返回下载任务。"))?;
        *self.active.lock().await = Some(ActiveDownload {
            id: id.to_owned(),
            file: file.clone(),
            handle: handle.clone(),
        });
        tokio::time::timeout(Duration::from_secs(60), handle.wait_until_initialized())
            .await
            .map_err(|_| AppError::new("TORRENT_INIT_TIMEOUT", "缓存校验超时，请稍后重试。"))?
            .map_err(|error| {
                AppError::new("TORRENT_START", format!("缓存初始化失败：{error:#}"))
            })?;
        session
            .update_only_files(&handle, &std::collections::HashSet::from([index]))
            .await
            .map_err(|error| {
                AppError::new("TORRENT_SELECT", format!("无法切换下载文件：{error:#}"))
            })?;
        if matches!(handle.stats().state, librqbit::TorrentStatsState::Paused) {
            session
                .unpause(&handle)
                .await
                .map_err(|error| AppError::new("TORRENT_START", error.to_string()))?;
        }
        Ok(file)
    }

    async fn open_file(&self, id: &str, index: usize) -> AppResult<Box<dyn super::SeekableReader>> {
        let handle = {
            let active = self.active.lock().await;
            active
                .as_ref()
                .filter(|active| active.id == id && active.file.index == index)
                .map(|active| active.handle.clone())
                .ok_or_else(|| AppError::new("INACTIVE_SOURCE", "媒体源已关闭。"))?
        };
        handle
            .stream(index)
            .await
            .map(|reader| Box::new(reader) as Box<dyn super::SeekableReader>)
            .map_err(|error| AppError::new("STREAM", format!("无法读取视频数据：{error:#}")))
    }

    async fn download_stats(&self) -> Option<super::models::DownloadStats> {
        let guard = self.active.lock().await;
        let active = guard.as_ref()?;
        let stats = active.handle.stats();
        Some(super::models::DownloadStats {
            torrent_id: active.id.clone(),
            file_index: active.file.index,
            downloaded: stats
                .file_progress
                .get(active.file.index)
                .copied()
                .unwrap_or(0)
                .min(active.file.size),
            total: active.file.size,
            bytes_per_second: stats
                .live
                .as_ref()
                .map(|live| live.download_speed.as_bytes())
                .unwrap_or(0),
            peers: stats
                .live
                .as_ref()
                .map(|live| live.snapshot.peer_stats.live)
                .unwrap_or(0),
            state: stats.state.to_string(),
            error: stats.error,
        })
    }

    async fn pause_download(&self) -> AppResult<()> {
        let previous = self.active.lock().await.take();
        if let (Some(previous), Some(session)) = (previous, self.session.get()) {
            if matches!(
                previous.handle.stats().state,
                librqbit::TorrentStatsState::Live
                    | librqbit::TorrentStatsState::Initializing { .. }
            ) {
                session
                    .pause(&previous.handle)
                    .await
                    .map_err(|error| AppError::new("TORRENT_PAUSE", error.to_string()))?;
            }
        }
        Ok(())
    }

    async fn shutdown(&self) {
        self.cancel_resolution().await;
        if let Some(session) = self.session.get() {
            session.stop().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_magnets_and_bad_hashes() {
        for value in [
            "",
            "https://example.com/video",
            "magnet:?xt=urn:btih:abc",
            "magnet:?dn=movie",
        ] {
            assert!(validate_magnet(value).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn accepts_trimmed_btih() -> AppResult<()> {
        let normalized =
            validate_magnet("  magnet:?xt=urn:btih:e07ed7410558567358b92e455a6a226e68a96a09 \n")?;
        assert!(Magnet::parse(&normalized).is_ok());
        Ok(())
    }

    #[test]
    fn removes_untrusted_selection_ranges() -> AppResult<()> {
        let normalized = validate_magnet("magnet:?xt=urn:btih:e07ed7410558567358b92e455a6a226e68a96a09&so=0-18446744073709551615")?;
        assert!(!normalized.contains("so="));
        Ok(())
    }
}
