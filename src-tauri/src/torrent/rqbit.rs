use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, DhtSessionConfig, ListOnlyResponse, Magnet,
    Session, SessionOptions,
};
use tokio::sync::{Mutex, OnceCell};
use tokio_util::sync::CancellationToken;

use super::{
    models::{TorrentCatalog, TorrentFile},
    TorrentEngine,
};
use crate::core::error::{AppError, AppResult};

pub struct ResolvedTorrent {
    pub catalog: TorrentCatalog,
    pub metadata: ListOnlyResponse,
}

pub struct RqbitEngine {
    cache_dir: PathBuf,
    session: OnceCell<Arc<Session>>,
    resolved: Mutex<HashMap<String, ResolvedTorrent>>,
    resolution: Mutex<Option<CancellationToken>>,
}

impl RqbitEngine {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            session: OnceCell::new(),
            resolved: Mutex::new(HashMap::new()),
            resolution: Mutex::new(None),
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
        let catalog = TorrentCatalog {
            id: id.clone(),
            name: metadata
                .info
                .name()
                .map(|name| name.into_owned())
                .unwrap_or_else(|| id.clone()),
            files: metadata
                .info
                .iter_file_details()
                .enumerate()
                .map(|(index, file)| TorrentFile {
                    index,
                    path: file.filename.to_string(),
                    size: file.len,
                })
                .collect(),
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
