//! Bilibili UGC/PGC provider backed by the application-local authenticated session.
use super::models::{Comment, ParsedSource, SourceInfo, MAX_COMMENTS, MAX_DURATION};
use crate::bilibili::{
    discovery::{self, SearchPage},
    session::{api_data, BilibiliClient},
};
use crate::core::error::{AppError, AppResult};
use async_trait::async_trait;
use prost::Message;
use reqwest::{header, Client};
use serde_json::Value;
use std::sync::Arc;
use std::{
    collections::{BTreeMap, HashSet},
    time::Duration,
};
use tokio_util::sync::CancellationToken;
use url::Url;

const API: &str = "https://api.bilibili.com";
pub const REFERER: &str = "https://www.bilibili.com/";
pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/131.0.0.0 Safari/537.36";

#[derive(Debug, PartialEq, Eq)]
pub struct VideoId {
    pub bvid: String,
    pub page: Option<u32>,
}

pub fn parse_input(input: &str) -> AppResult<VideoId> {
    let input = input.trim();
    let (id, page) = if input.starts_with("BV") {
        (input.to_owned(), None)
    } else {
        let input = if input.starts_with("www.bilibili.com/") || input.starts_with("bilibili.com/")
        {
            format!("https://{input}")
        } else {
            input.to_owned()
        };
        let url = Url::parse(&input)
            .map_err(|_| AppError::new("BILIBILI_INPUT", "请输入 BV 号或 B 站视频链接。"))?;
        if !matches!(url.scheme(), "https" | "http")
            || !matches!(
                url.host_str(),
                Some("www.bilibili.com" | "bilibili.com" | "m.bilibili.com")
            )
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
        {
            return Err(AppError::new(
                "BILIBILI_INPUT",
                "仅支持 bilibili.com/video/ 视频链接和 b23.tv 短链接。",
            ));
        }
        let parts: Vec<_> = url.path().trim_matches('/').split('/').collect();
        if parts.len() != 2 || parts[0] != "video" {
            return Err(AppError::new(
                "BILIBILI_INPUT",
                "请使用包含 BV 号的视频链接；番剧 ep/ss 和直播链接暂不支持。",
            ));
        }
        let page = url
            .query_pairs()
            .find(|(key, _)| key == "p")
            .map(|(_, value)| value.parse::<u32>())
            .transpose()
            .map_err(|_| AppError::new("BILIBILI_INPUT", "分 P 参数无效。"))?;
        (parts[1].to_owned(), page)
    };
    if id.len() != 12
        || !id.starts_with("BV1")
        || !id.bytes().all(|b| b.is_ascii_alphanumeric())
        || page == Some(0)
    {
        return Err(AppError::new("BILIBILI_INPUT", "BV 号或分 P 参数无效。"));
    }
    Ok(VideoId { bvid: id, page })
}

#[derive(Clone)]
pub struct VideoStream {
    pub url: String,
    pub audio_url: Option<String>,
    pub duration: f64,
}

#[async_trait]
pub trait DanmakuProvider: Send + Sync {
    async fn search(
        &self,
        _query: &str,
        _page: u32,
        _cancel: &CancellationToken,
    ) -> AppResult<SearchPage> {
        Err(AppError::new("SEARCH_UNAVAILABLE", "该来源暂不支持搜索。"))
    }
    async fn videos(&self, input: &str, cancel: &CancellationToken) -> AppResult<Vec<SourceInfo>>;
    async fn comments(
        &self,
        source: SourceInfo,
        cancel: &CancellationToken,
    ) -> AppResult<ParsedSource>;
    async fn stream(
        &self,
        source: &SourceInfo,
        cancel: &CancellationToken,
    ) -> AppResult<VideoStream>;
}

pub struct BilibiliProvider {
    client: Client,
    session: Arc<BilibiliClient>,
}
impl BilibiliProvider {
    pub fn new() -> AppResult<Self> {
        Self::with_session(Arc::new(BilibiliClient::new(None)?))
    }
    pub fn with_session(session: Arc<BilibiliClient>) -> AppResult<Self> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(25))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| AppError::new("BILIBILI_HTTP", e.without_url().to_string()))?;
        Ok(Self { client, session })
    }
    async fn bytes(&self, url: &str, cancel: &CancellationToken) -> AppResult<Vec<u8>> {
        self.session.bytes(url, cancel).await
    }

    async fn json(&self, path: &str, cancel: &CancellationToken) -> AppResult<Value> {
        serde_json::from_slice(&self.bytes(&format!("{API}{path}"), cancel).await?).map_err(|_| {
            AppError::new(
                "BILIBILI_RESPONSE",
                "B 站没有返回有效 JSON，可能受到网络或访问限制。",
            )
        })
    }
    async fn identify(&self, input: &str, cancel: &CancellationToken) -> AppResult<String> {
        let input = input.trim();
        let expanded = if input.starts_with("b23.tv/") {
            format!("https://{input}")
        } else {
            input.to_owned()
        };
        if let Ok(url) = Url::parse(&expanded) {
            if url.host_str() == Some("b23.tv")
                && matches!(url.scheme(), "https" | "http")
                && url.port().is_none()
                && url.username().is_empty()
                && url.password().is_none()
            {
                let mut current = url;
                current
                    .set_scheme("https")
                    .map_err(|_| AppError::new("BILIBILI_INPUT", "短链接无效。"))?;
                for _ in 0..4 {
                    let response = tokio::select! {
                        _ = cancel.cancelled() => return Err(cancelled()),
                        response = self.client.get(current.clone()).header(header::REFERER, REFERER).send() => response.map_err(network_error)?,
                    };
                    if !response.status().is_redirection() {
                        break;
                    }
                    let location = response
                        .headers()
                        .get(header::LOCATION)
                        .and_then(|v| v.to_str().ok())
                        .ok_or_else(|| AppError::new("BILIBILI_INPUT", "短链接没有有效目标。"))?;
                    current = current
                        .join(location)
                        .map_err(|_| AppError::new("BILIBILI_INPUT", "短链接跳转无效。"))?;
                    if current.host_str() != Some("b23.tv") {
                        return Ok(current.into());
                    }
                    if current.scheme() != "https"
                        || current.port().is_some()
                        || !current.username().is_empty()
                        || current.password().is_some()
                    {
                        break;
                    }
                }
                return Err(AppError::new(
                    "BILIBILI_INPUT",
                    "短链接无法解析，请复制完整 BV 链接。",
                ));
            }
        }
        Ok(expanded)
    }
    async fn signed_playurl(
        &self,
        source: &SourceInfo,
        cancel: &CancellationToken,
    ) -> AppResult<Value> {
        let nav = self.json("/x/web-interface/nav", cancel).await?;
        let mut lookup = String::new();
        for field in ["img_url", "sub_url"] {
            let value = nav["data"]["wbi_img"][field].as_str().unwrap_or_default();
            lookup.push_str(
                value
                    .rsplit('/')
                    .next()
                    .unwrap_or_default()
                    .split('.')
                    .next()
                    .unwrap_or_default(),
            );
        }
        let indexes = [
            46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42,
            19, 29, 28, 14, 39, 12, 38, 41, 13,
        ];
        if lookup.len() != 64 || !lookup.is_ascii() {
            return Err(AppError::new(
                "BILIBILI_RESPONSE",
                "无法获取 B 站视频请求签名参数。",
            ));
        }
        let key: String = indexes
            .iter()
            .map(|i| lookup.as_bytes()[*i] as char)
            .collect();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(AppError::io)?
            .as_secs();
        let params = BTreeMap::from([
            ("bvid", source.bvid.clone()),
            ("cid", source.cid.to_string()),
            ("qn", "16".into()),
            ("fnval", "16".into()),
            ("fourk", "0".into()),
            ("wts", now.to_string()),
        ]);
        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(params)
            .finish();
        let signature = format!("{:x}", md5::compute(format!("{query}{key}")));
        self.json(
            &format!("/x/player/wbi/playurl?{query}&w_rid={signature}"),
            cancel,
        )
        .await
    }
}

fn network_error(error: reqwest::Error) -> AppError {
    let message = match error.status().map(|s| s.as_u16()) {
        Some(403 | 412 | 429) => "B 站限制了当前请求，请稍后重试；已有弹幕仍可手动混合。".into(),
        _ if error.is_timeout() => "B 站请求超时，请检查网络后重试。".into(),
        _ => format!("B 站请求失败：{}", error.without_url()),
    };
    AppError::new("BILIBILI_HTTP", message)
}
pub fn cancelled() -> AppError {
    AppError::new("CANCELLED", "已取消弹幕任务。")
}
fn data(value: Value) -> AppResult<Value> {
    api_data(value)
}

#[async_trait]
impl DanmakuProvider for BilibiliProvider {
    async fn search(
        &self,
        query: &str,
        page: u32,
        cancel: &CancellationToken,
    ) -> AppResult<SearchPage> {
        discovery::search(&self.session, query, page, cancel).await
    }
    async fn videos(&self, input: &str, cancel: &CancellationToken) -> AppResult<Vec<SourceInfo>> {
        let input = self.identify(input, cancel).await?;
        if let Some((episode, id)) = discovery::pgc_id(&input) {
            return discovery::episodes(&self.session, episode, id, cancel).await;
        }
        let video = parse_input(&input)?;
        let view = data(
            self.json(
                &format!("/x/web-interface/view?bvid={}", video.bvid),
                cancel,
            )
            .await?,
        )?;
        let pages = view["pages"]
            .as_array()
            .ok_or_else(|| AppError::new("BILIBILI_RESPONSE", "视频没有可读取的分 P 信息。"))?;
        let mut result = Vec::new();
        for item in pages {
            let page = item["page"].as_u64().unwrap_or_default() as u32;
            if video.page.is_some_and(|p| p != page) {
                continue;
            }
            let duration = item["duration"].as_f64().unwrap_or_default();
            let cid = item["cid"].as_u64().unwrap_or_default();
            if cid == 0 || !(0.1..=MAX_DURATION).contains(&duration) {
                return Err(AppError::new(
                    "BILIBILI_RESPONSE",
                    "视频 CID 或时长无效（最长支持 24 小时）。",
                ));
            }
            let title = view["title"].as_str().unwrap_or(&video.bvid);
            let part = item["part"].as_str().unwrap_or_default();
            result.push(SourceInfo {
                id: format!("{}:{cid}", video.bvid),
                bvid: video.bvid.clone(),
                cid,
                page,
                title: if pages.len() > 1 {
                    format!("{title} · P{page} {part}")
                } else {
                    title.into()
                },
                duration,
                comment_count: 0,
                warnings: vec![],
                episode_id: None,
            });
        }
        if result.is_empty() {
            return Err(AppError::new("BILIBILI_INPUT", "指定的分 P 不存在。"));
        }
        Ok(result)
    }
    async fn comments(
        &self,
        mut source: SourceInfo,
        cancel: &CancellationToken,
    ) -> AppResult<ParsedSource> {
        let total = (source.duration / 360.0).ceil() as usize;
        let mut comments = Vec::new();
        let mut ids = HashSet::new();
        let mut skipped = 0;
        for segment in 1..=total {
            let bytes = self
                .bytes(
                    &format!(
                        "{API}/x/v2/dm/web/seg.so?type=1&oid={}&segment_index={segment}",
                        source.cid
                    ),
                    cancel,
                )
                .await?;
            // An empty protobuf message is a valid empty segment; HTML/JSON is not.
            let (items, unsupported) = decode_segment(&bytes)?;
            skipped += unsupported;
            for item in items {
                if ids.insert(item.id.clone()) {
                    comments.push(item);
                }
            }
            if comments.len() > MAX_COMMENTS {
                return Err(AppError::new(
                    "DANMAKU_LIMIT",
                    "单视频弹幕超过 20 万条，已停止解析。",
                ));
            }
        }
        comments.sort_by(|a, b| a.time.total_cmp(&b.time));
        source.comment_count = comments.len();
        if skipped > 0 {
            source.warnings.push(format!(
                "跳过 {skipped} 条高级、代码或无效弹幕，仅混合普通滚动和顶部/底部弹幕。"
            ));
        }
        Ok(ParsedSource {
            info: source,
            comments,
        })
    }
    async fn stream(
        &self,
        source: &SourceInfo,
        cancel: &CancellationToken,
    ) -> AppResult<VideoStream> {
        let play = if let Some(episode) = source.episode_id {
            let value = data(
                self.json(
                    &format!(
                        "/pgc/player/web/v2/playurl?ep_id={episode}&cid={}&qn=16&fnval=16&fourk=0",
                        source.cid
                    ),
                    cancel,
                )
                .await?,
            )?;
            let video = value.get("video_info").unwrap_or(&value);
            if value["is_preview"].as_bool() == Some(true)
                || video["is_preview"].as_bool() == Some(true)
                || value["play_check"]["play_detail"].as_str() == Some("PLAY_PREVIEW")
                || value["play_video_type"].as_str() == Some("preview")
            {
                return Err(AppError::new(
                    "BILIBILI_PREVIEW",
                    "当前账号仅能观看预览，请登录有观看权益的账号。",
                ));
            }
            video.clone()
        } else {
            let response = self
                .json(
                    &format!(
                        "/x/player/playurl?bvid={}&cid={}&qn=16&fnval=16&fourk=0",
                        source.bvid, source.cid
                    ),
                    cancel,
                )
                .await;
            match response.and_then(data) {
                Ok(play) => play,
                Err(e) if e.code == "CANCELLED" => return Err(e),
                Err(_) => data(self.signed_playurl(source, cancel).await?)?,
            }
        };
        let duration = play["timelength"].as_f64().unwrap_or_default() / 1000.0;
        if duration <= 0.0 || (duration - source.duration).abs() > 2.0 {
            return Err(AppError::new(
                "BILIBILI_PREVIEW",
                "B 站返回的视频时长不符，可能只有预览片段；请使用手动区间。",
            ));
        }
        let dash = play["dash"]["video"]
            .as_array()
            .and_then(|videos| {
                videos
                    .iter()
                    .filter(|v| v["codecs"].as_str().is_some_and(|s| s.starts_with("avc")))
                    .min_by_key(|v| v["bandwidth"].as_u64().unwrap_or(u64::MAX))
            })
            .or_else(|| play["dash"]["video"].as_array().and_then(|v| v.first()));
        let video = if let Some(video) = dash {
            video
        } else {
            let parts = play["durl"].as_array().ok_or_else(|| {
                AppError::new(
                    "BILIBILI_VIDEO",
                    "没有可读取的视频流，可能需要登录或视频已下架。",
                )
            })?;
            if parts.len() != 1 {
                return Err(AppError::new(
                    "BILIBILI_VIDEO",
                    "该视频使用旧版分段视频流，暂请手动指定弹幕区间。",
                ));
            }
            &parts[0]
        };
        Ok(VideoStream {
            audio_url: if play["dash"].is_object() {
                play["dash"]["audio"]
                    .as_array()
                    .and_then(|a| {
                        a.iter()
                            .min_by_key(|v| v["bandwidth"].as_u64().unwrap_or(u64::MAX))
                    })
                    .map(select_stream_url)
                    .transpose()?
            } else {
                Some(select_stream_url(video)?)
            },
            url: select_stream_url(video)?,
            duration,
        })
    }
}

pub fn select_stream_url(video: &Value) -> AppResult<String> {
    let primary = ["baseUrl", "base_url", "url"]
        .iter()
        .filter_map(|key| video[key].as_str());
    let backups = ["backupUrl", "backup_url"]
        .iter()
        .flat_map(|key| video[key].as_array().into_iter().flatten())
        .filter_map(Value::as_str);
    for candidate in primary.chain(backups) {
        let Ok(mut url) = Url::parse(candidate) else {
            continue;
        };
        let host = url.host_str().unwrap_or_default();
        if ![
            "bilivideo.com",
            "bilivideo.cn",
            "bilivideo.net",
            "akamaized.net",
        ]
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
            || !matches!(url.scheme(), "https" | "http")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
        {
            continue;
        }
        url.set_scheme("https")
            .map_err(|_| AppError::new("BILIBILI_VIDEO", "视频流协议无效。"))?;
        return Ok(url.into());
    }
    Err(AppError::new(
        "BILIBILI_VIDEO",
        "主地址与备用地址均不是受支持的 B 站 CDN 地址。",
    ))
}

#[derive(Clone, PartialEq, Message)]
struct Segment {
    #[prost(message, repeated, tag = "1")]
    elems: Vec<Element>,
}
#[derive(Clone, PartialEq, Message)]
struct Element {
    #[prost(int64, tag = "1")]
    id: i64,
    #[prost(int32, tag = "2")]
    progress: i32,
    #[prost(int32, tag = "3")]
    mode: i32,
    #[prost(int32, tag = "4")]
    fontsize: i32,
    #[prost(uint32, tag = "5")]
    color: u32,
    #[prost(string, tag = "7")]
    content: String,
    #[prost(string, tag = "12")]
    id_str: String,
}
pub fn decode_segment(bytes: &[u8]) -> AppResult<(Vec<Comment>, usize)> {
    let segment = Segment::decode(bytes).map_err(|_| {
        AppError::new(
            "BILIBILI_DANMAKU",
            "B 站弹幕分段格式无效；未保存不完整的弹幕源。",
        )
    })?;
    let mut skipped = 0;
    let comments = segment
        .elems
        .into_iter()
        .filter_map(|c| {
            if !(1..=6).contains(&c.mode)
                || c.progress < 0
                || c.content.trim().is_empty()
                || c.content.len() > 6000
                || (c.id <= 0 && c.id_str.is_empty())
            {
                skipped += 1;
                return None;
            }
            Some(Comment {
                id: if c.id_str.is_empty() {
                    c.id.to_string()
                } else {
                    c.id_str
                },
                time: c.progress as f64 / 1000.0,
                mode: c.mode as u32,
                color: c.color & 0xffffff,
                size: c.fontsize.clamp(12, 64) as u32,
                text: c.content,
            })
        })
        .collect();
    Ok((comments, skipped))
}
