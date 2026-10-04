use super::{
    buffer::file_ranges,
    models::{DownloadStats, PrefetchStats, TorrentCatalog},
    TorrentEngine,
};
use crate::{
    core::error::{AppError, AppResult},
    media::{
        files::{FileKind, MediaFile},
        playlist::{matching_subtitles, video_queue},
    },
};
use async_trait::async_trait;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, DhtSessionConfig, ListOnlyResponse, Magnet,
    Session, SessionOptions,
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tokio::sync::{Mutex, OnceCell};
use tokio_util::sync::CancellationToken;

struct ResolvedTorrent {
    catalog: TorrentCatalog,
    metadata: ListOnlyResponse,
}
#[derive(Clone)]
struct ActiveDownload {
    id: String,
    file: MediaFile,
    subtitles: Vec<MediaFile>,
    prefetch: Option<MediaFile>,
    handle: Arc<librqbit::ManagedTorrent>,
    cancel: CancellationToken,
}
impl ActiveDownload {
    fn selected(&self) -> HashSet<usize> {
        std::iter::once(self.file.index)
            .chain(self.subtitles.iter().map(|f| f.index))
            .chain(self.prefetch.iter().map(|f| f.index))
            .collect()
    }
}
pub struct RqbitEngine {
    cache_dir: PathBuf,
    session: OnceCell<Arc<Session>>,
    resolved: Mutex<HashMap<String, ResolvedTorrent>>,
    resolution: Mutex<Option<CancellationToken>>,
    active: Mutex<Option<ActiveDownload>>,
    operation: Mutex<()>,
}
impl RqbitEngine {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            session: OnceCell::new(),
            resolved: Mutex::new(HashMap::new()),
            resolution: Mutex::new(None),
            active: Mutex::new(None),
            operation: Mutex::new(()),
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
                        disable_upload: true,
                        peer_limit: Some(80),
                        ..Default::default()
                    },
                )
                .await
                .map_err(|e| AppError::new("TORRENT_INIT", format!("无法启动 torrent 引擎：{e:#}")))
            })
            .await
    }
    async fn read_catalog(&self, source: AddTorrent<'_>) -> AppResult<TorrentCatalog> {
        let response = self
            .session()
            .await?
            .add_torrent(
                source,
                Some(AddTorrentOptions {
                    list_only: true,
                    ..Default::default()
                }),
            )
            .await
            .map_err(|e| AppError::new("METADATA", format!("无法读取元数据：{e:#}")))?;
        let metadata = match response {
            AddTorrentResponse::ListOnly(m) => m,
            AddTorrentResponse::AlreadyManaged(_, h) => {
                return self.catalog(&h.info_hash().as_string()).await
            }
            _ => return Err(AppError::new("METADATA", "目录解析意外启动了下载。")),
        };
        let id = metadata.info_hash.as_string();
        let files: Vec<_> = metadata
            .info
            .iter_file_details()
            .enumerate()
            .map(|(i, f)| MediaFile::new(i, f.filename.to_string(), f.len))
            .collect();
        let catalog = TorrentCatalog {
            id: id.clone(),
            name: metadata
                .info
                .name()
                .map(|n| n.into_owned())
                .unwrap_or_else(|| id.clone()),
            suggested_file_index: video_queue(&files).first().map(|f| f.index),
            files,
        };
        // Durable metadata is separate from the engine's live session. Startup
        // never restores a session or connects to peers.
        let dir = self.cache_dir.join("metadata");
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(AppError::io)?;
        let temporary = dir.join(format!("{id}.{}.tmp", uuid::Uuid::new_v4()));
        tokio::fs::write(&temporary, &metadata.torrent_bytes)
            .await
            .map_err(AppError::io)?;
        if let Err(error) = tokio::fs::rename(&temporary, dir.join(format!("{id}.torrent"))).await {
            let _ = tokio::fs::remove_file(temporary).await;
            return Err(AppError::io(error));
        }
        self.resolved.lock().await.insert(
            id,
            ResolvedTorrent {
                catalog: catalog.clone(),
                metadata,
            },
        );
        Ok(catalog)
    }
    async fn resolve_inner(&self, magnet: &str) -> AppResult<TorrentCatalog> {
        let parsed =
            Magnet::parse(magnet).map_err(|e| AppError::new("INVALID_MAGNET", e.to_string()))?;
        if let Some(hash) = parsed.as_id20() {
            let id = hash.as_string();
            if let Ok(catalog) = self.catalog(&id).await {
                return Ok(catalog);
            }
            if let Ok(bytes) = tokio::fs::read(
                self.cache_dir
                    .join("metadata")
                    .join(format!("{id}.torrent")),
            )
            .await
            {
                let cached = self.read_catalog(AddTorrent::from_bytes(bytes)).await;
                if let Ok(catalog) = cached {
                    if catalog.id == id {
                        return Ok(catalog);
                    }
                }
            }
        }
        self.read_catalog(AddTorrent::from_url(magnet)).await
    }
    async fn selected_handle(
        &self,
        id: &str,
        index: usize,
    ) -> AppResult<Arc<librqbit::ManagedTorrent>> {
        self.active
            .lock()
            .await
            .as_ref()
            .filter(|a| a.id == id && a.selected().contains(&index))
            .map(|a| a.handle.clone())
            .ok_or_else(|| AppError::new("INACTIVE_SOURCE", "媒体源已关闭。"))
    }
    async fn apply_selection(&self, active: &ActiveDownload) -> AppResult<()> {
        let session = self.session().await?;
        let wanted = active.selected();
        session
            .update_only_files(&active.handle, &wanted)
            .await
            .map_err(|e| AppError::new("TORRENT_SELECT", format!("{e:#}")))?;
        let stats = active.handle.stats();
        let needs_data = active
            .handle
            .with_metadata(|m| {
                wanted.iter().any(|i| {
                    stats.file_progress.get(*i).copied().unwrap_or(0) < m.file_infos[*i].len
                })
            })
            .unwrap_or(true);
        if needs_data && active.handle.is_paused() {
            session
                .unpause(&active.handle)
                .await
                .map_err(|e| AppError::new("TORRENT_START", format!("{e:#}")))?;
        } else if !needs_data && !active.handle.is_paused() {
            session
                .pause(&active.handle)
                .await
                .map_err(|e| AppError::new("TORRENT_PAUSE", format!("{e:#}")))?;
        }
        Ok(())
    }
}
#[async_trait]
impl TorrentEngine for RqbitEngine {
    async fn resolve(&self, magnet: &str) -> AppResult<TorrentCatalog> {
        let magnet = validate_magnet(magnet)?;
        let cancel = CancellationToken::new();
        {
            let mut resolution = self.resolution.lock().await;
            if resolution.is_some() {
                return Err(AppError::new("BUSY", "正在解析另一条磁力链接。"));
            }
            *resolution = Some(cancel.clone());
        }
        let result = tokio::select! {
            _ = cancel.cancelled() => Err(AppError::new("CANCELLED","已取消磁力解析。")),
            result = tokio::time::timeout(Duration::from_secs(120),self.resolve_inner(&magnet)) =>
                result.unwrap_or_else(|_|Err(AppError::new("METADATA_TIMEOUT","120 秒内未取得目录，请检查网络或重试。"))),
        };
        *self.resolution.lock().await = None;
        result
    }
    async fn import_torrent(&self, bytes: Vec<u8>) -> AppResult<TorrentCatalog> {
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(AppError::new("TORRENT_SIZE", "种子文件不能超过 16 MiB。"));
        }
        self.read_catalog(AddTorrent::from_bytes(bytes)).await
    }
    async fn cancel_resolution(&self) {
        if let Some(c) = self.resolution.lock().await.as_ref() {
            c.cancel();
        }
    }
    async fn catalog(&self, id: &str) -> AppResult<TorrentCatalog> {
        self.resolved
            .lock()
            .await
            .get(id)
            .map(|r| r.catalog.clone())
            .ok_or_else(|| AppError::new("NOT_FOUND", "请先加载磁力目录。"))
    }
    async fn prepare_file(&self, id: &str, index: usize) -> AppResult<MediaFile> {
        let _operation = self.operation.lock().await;
        let (bytes, peers, file, subtitles) = {
            let guard = self.resolved.lock().await;
            let entry = guard
                .get(id)
                .ok_or_else(|| AppError::new("NOT_FOUND", "请先加载磁力目录。"))?;
            let file = entry
                .catalog
                .files
                .iter()
                .find(|f| f.index == index && f.kind == FileKind::Video && f.size > 0)
                .ok_or_else(|| AppError::new("INVALID_FILE", "请选择非空视频。"))?
                .clone();
            (
                entry.metadata.torrent_bytes.clone(),
                entry.metadata.seen_peers.clone(),
                file.clone(),
                matching_subtitles(&entry.catalog.files, &file),
            )
        };
        // Register paused and empty, so initialization cannot download a season.
        let response = self
            .session()
            .await?
            .add_torrent(
                AddTorrent::from_bytes(bytes),
                Some(AddTorrentOptions {
                    paused: true,
                    only_files: Some(vec![]),
                    initial_peers: Some(peers),
                    sub_folder: Some(id.to_owned()),
                    overwrite: true,
                    ..Default::default()
                }),
            )
            .await
            .map_err(|e| AppError::new("TORRENT_START", format!("{e:#}")))?;
        let handle = response
            .into_handle()
            .ok_or_else(|| AppError::new("TORRENT_START", "引擎没有返回下载任务。"))?;
        let active = ActiveDownload {
            id: id.to_owned(),
            file: file.clone(),
            subtitles,
            prefetch: None,
            handle: handle.clone(),
            cancel: CancellationToken::new(),
        };
        if let Some(previous) = self.active.lock().await.replace(active.clone()) {
            previous.cancel.cancel();
        }
        tokio::time::timeout(Duration::from_secs(60), handle.wait_until_initialized())
            .await
            .map_err(|_| AppError::new("TORRENT_INIT_TIMEOUT", "缓存校验超时。"))?
            .map_err(|e| AppError::new("TORRENT_START", format!("{e:#}")))?;
        self.apply_selection(&active).await?;
        // Separate bounded readers give companion subtitles piece priority, too.
        for subtitle in &active.subtitles {
            let (handle, cancel, index) = (handle.clone(), active.cancel.clone(), subtitle.index);
            tokio::spawn(async move {
                let read = async {
                    if let Ok(mut reader) = handle.stream(index).await {
                        let _ = tokio::io::copy(&mut reader, &mut tokio::io::sink()).await;
                    }
                };
                tokio::select! { _ = cancel.cancelled() => {}, _ = tokio::time::timeout(Duration::from_secs(90),read) => {} }
            });
        }
        Ok(file)
    }
    async fn open_file(&self, id: &str, index: usize) -> AppResult<Box<dyn super::SeekableReader>> {
        let _operation = self.operation.lock().await;
        let handle = self.selected_handle(id, index).await?;
        if handle.is_paused() {
            let active = self
                .active
                .lock()
                .await
                .clone()
                .ok_or_else(|| AppError::new("INACTIVE_SOURCE", "媒体源已关闭。"))?;
            self.apply_selection(&active).await?;
        }
        handle
            .stream(index)
            .await
            .map(|r| Box::new(r) as Box<dyn super::SeekableReader>)
            .map_err(|e| AppError::new("STREAM", format!("{e:#}")))
    }
    async fn prefetch(&self, id: &str, index: Option<usize>) -> AppResult<()> {
        let _operation = self.operation.lock().await;
        let file = match index {
            Some(i) => Some(
                self.catalog(id)
                    .await?
                    .files
                    .into_iter()
                    .find(|f| f.index == i && f.kind == FileKind::Video && f.size > 0)
                    .ok_or_else(|| AppError::new("INVALID_FILE", "无效的预取文件。"))?,
            ),
            None => None,
        };
        let updated = {
            let mut active = self.active.lock().await;
            let active = active
                .as_mut()
                .filter(|a| a.id == id)
                .ok_or_else(|| AppError::new("INACTIVE_SOURCE", "下载已停止。"))?;
            if active.prefetch.as_ref().map(|f| f.index) == index {
                return Ok(());
            }
            active.prefetch = file;
            active.clone()
        };
        self.apply_selection(&updated).await
    }
    async fn subtitle_path(&self, id: &str, index: usize) -> AppResult<Option<PathBuf>> {
        let active = self
            .active
            .lock()
            .await
            .clone()
            .filter(|a| a.id == id)
            .ok_or_else(|| AppError::new("INACTIVE_SOURCE", "下载已停止。"))?;
        let file = active
            .subtitles
            .iter()
            .find(|f| f.index == index)
            .ok_or_else(|| AppError::new("INVALID_FILE", "字幕不属于当前视频。"))?;
        let stats = active.handle.stats();
        if stats.file_progress.get(index).copied().unwrap_or(0) < file.size {
            return Ok(None);
        }
        let path = active
            .handle
            .with_metadata(|m| {
                active
                    .handle
                    .output_folder()
                    .join(&m.file_infos[index].relative_filename)
            })
            .map_err(|e| AppError::new("SUBTITLE", e.to_string()))?;
        // VobSub needs both files; let the .idx load the pair, never .sub twice.
        if path.extension().is_some_and(|e| e == "sub") && path.with_extension("idx").exists() {
            return Ok(None);
        }
        if path.extension().is_some_and(|e| e == "idx") {
            let paired = active.subtitles.iter().find(|f| {
                PathBuf::from(&f.path).with_extension("idx") == std::path::Path::new(&file.path)
                    && f.index != index
            });
            if paired
                .is_some_and(|p| stats.file_progress.get(p.index).copied().unwrap_or(0) < p.size)
            {
                return Ok(None);
            }
        }
        Ok(Some(path))
    }
    async fn download_stats(&self) -> Option<DownloadStats> {
        let active = self.active.lock().await.clone()?;
        let stats = active.handle.stats();
        let buffered = self
            .session
            .get()
            .and_then(|s| {
                librqbit::Api::new(s.clone(), None)
                    .api_dump_haves(librqbit::api::TorrentIdOrHash::Id(active.handle.id()))
                    .ok()
            })
            .and_then(|(bits, count)| {
                active
                    .handle
                    .with_metadata(|m| {
                        let f = &m.file_infos[active.file.index];
                        file_ranges(
                            bits.iter().take(count as usize).map(|b| *b),
                            m.lengths().default_piece_length() as u64,
                            f.offset_in_torrent,
                            f.len,
                        )
                    })
                    .ok()
            })
            .unwrap_or_default();
        let mut selected_files: Vec<_> = active.selected().into_iter().collect();
        selected_files.sort_unstable();
        Some(DownloadStats {
            torrent_id: active.id,
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
                .map(|l| l.download_speed.as_bytes())
                .unwrap_or(0),
            peers: stats
                .live
                .as_ref()
                .map(|l| l.snapshot.peer_stats.live)
                .unwrap_or(0),
            state: stats.state.to_string(),
            error: stats.error,
            buffered,
            selected_files,
            prefetch: active.prefetch.map(|f| PrefetchStats {
                file_index: f.index,
                downloaded: stats
                    .file_progress
                    .get(f.index)
                    .copied()
                    .unwrap_or(0)
                    .min(f.size),
                total: f.size,
            }),
        })
    }
    async fn pause_download(&self) -> AppResult<()> {
        let _operation = self.operation.lock().await;
        let previous = self.active.lock().await.take();
        if let Some(previous) = previous {
            previous.cancel.cancel();
            if !previous.handle.is_paused() {
                if let Some(session) = self.session.get() {
                    session
                        .pause(&previous.handle)
                        .await
                        .map_err(|e| AppError::new("TORRENT_PAUSE", format!("{e:#}")))?;
                }
            }
        }
        Ok(())
    }
    async fn suspend_download(&self) -> AppResult<()> {
        let _operation = self.operation.lock().await;
        let active = {
            let mut active = self.active.lock().await;
            if let Some(active) = active.as_mut() {
                active.prefetch = None;
                active.cancel.cancel();
            }
            active.clone()
        };
        if let (Some(active), Some(session)) = (active, self.session.get()) {
            if !active.handle.is_paused() {
                session
                    .pause(&active.handle)
                    .await
                    .map_err(|e| AppError::new("TORRENT_PAUSE", format!("{e:#}")))?;
            }
        }
        Ok(())
    }
    async fn shutdown(&self) {
        self.cancel_resolution().await;
        let _ = self.pause_download().await;
        if let Some(session) = self.session.get() {
            session.stop().await;
        }
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
