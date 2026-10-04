use super::session::{api_data, BilibiliClient, API};
use crate::{
    core::error::{AppError, AppResult},
    danmaku::models::{SourceInfo, MAX_DURATION},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub input: String,
    pub title: String,
    pub kind: String,
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub hits: Vec<SearchHit>,
    pub has_more: bool,
    pub warnings: Vec<String>,
}

pub fn pgc_id(input: &str) -> Option<(bool, u64)> {
    let input = input.trim();
    let id = if input.starts_with("ep") || input.starts_with("ss") {
        input.to_owned()
    } else {
        let expanded =
            if input.starts_with("www.bilibili.com/") || input.starts_with("bilibili.com/") {
                format!("https://{input}")
            } else {
                input.into()
            };
        let url = Url::parse(&expanded).ok()?;
        if !matches!(url.scheme(), "http" | "https")
            || !matches!(
                url.host_str(),
                Some("www.bilibili.com" | "bilibili.com" | "m.bilibili.com")
            )
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
        {
            return None;
        }
        url.path()
            .trim_end_matches('/')
            .strip_prefix("/bangumi/play/")?
            .to_owned()
    };
    let (prefix, digits) = id.split_at_checked(2)?;
    if !matches!(prefix, "ep" | "ss") || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let number = digits.parse::<u64>().ok().filter(|n| *n > 0)?;
    Some((prefix == "ep", number))
}
pub async fn episodes(
    client: &BilibiliClient,
    episode: bool,
    id: u64,
    cancel: &CancellationToken,
) -> AppResult<Vec<SourceInfo>> {
    let key = if episode { "ep_id" } else { "season_id" };
    let result = api_data(
        client
            .json(&format!("{API}/pgc/view/web/season?{key}={id}"), cancel)
            .await?,
    )?;
    parse_episodes(&result, episode.then_some(id))
}
pub fn parse_episodes(value: &Value, selected: Option<u64>) -> AppResult<Vec<SourceInfo>> {
    let title = value["title"].as_str().unwrap_or("番剧 / 影视");
    let mut seen = std::collections::HashSet::new();
    let items = value["episodes"].as_array().into_iter().flatten().chain(
        value["section"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|s| s["episodes"].as_array().into_iter().flatten()),
    );
    let mut sources = vec![];
    for item in items {
        let id = item["id"].as_u64().or(item["ep_id"].as_u64()).unwrap_or(0);
        let cid = item["cid"].as_u64().unwrap_or(0);
        let duration = item["duration"].as_f64().unwrap_or(0.0) / 1000.0;
        if id == 0 || cid == 0 || selected.is_some_and(|s| s != id) || !seen.insert(id) {
            continue;
        }
        if !(0.1..=MAX_DURATION).contains(&duration) {
            continue;
        }
        let number = item["title"].as_str().unwrap_or("");
        let name = item["long_title"].as_str().unwrap_or("");
        sources.push(SourceInfo {
            id: format!("ep{id}:{cid}"),
            bvid: item["bvid"].as_str().unwrap_or_default().into(),
            cid,
            page: number.parse().unwrap_or(sources.len() as u32 + 1),
            title: format!("{title} · 第{number}集 {name}"),
            duration,
            comment_count: 0,
            warnings: vec![],
            episode_id: Some(id),
        });
    }
    if sources.is_empty() {
        return Err(AppError::new(
            "BILIBILI_EPISODE",
            "没有取得该剧集的 CID 和完整时长，可能需要登录或没有观看权限。",
        ));
    }
    Ok(sources)
}
pub async fn search(
    client: &BilibiliClient,
    query: &str,
    page: u32,
    cancel: &CancellationToken,
) -> AppResult<SearchPage> {
    if query.trim().is_empty() || query.len() > 300 || !(1..=20).contains(&page) {
        return Err(AppError::new(
            "SEARCH_INPUT",
            "搜索词须为 1～300 字节，页码须为 1～20。",
        ));
    }
    let mut result = SearchPage::default();
    for kind in ["video", "media_bangumi", "media_ft"] {
        let mut url =
            Url::parse(&format!("{API}/x/web-interface/search/type")).map_err(AppError::io)?;
        url.query_pairs_mut()
            .append_pair("keyword", query.trim())
            .append_pair("search_type", kind)
            .append_pair("page", &page.to_string())
            .append_pair("page_size", "20");
        let data = match client.json(url.as_str(), cancel).await.and_then(api_data) {
            Ok(data) => data,
            Err(e) if e.code == "CANCELLED" => return Err(e),
            Err(e) => {
                result.warnings.push(format!("{kind} 搜索：{}", e.message));
                continue;
            }
        };
        result.has_more |= data["numPages"].as_u64().is_some_and(|n| n > page as u64);
        for item in data["result"].as_array().into_iter().flatten() {
            let input = if kind == "video" {
                item["bvid"]
                    .as_str()
                    .filter(|v| v.starts_with("BV1") && v.len() == 12)
                    .map(str::to_owned)
            } else {
                item["season_id"].as_u64().map(|s| format!("ss{s}"))
            };
            if let Some(input) = input {
                if !result.hits.iter().any(|h| h.input == input) {
                    result.hits.push(SearchHit {
                        input,
                        title: clean_title(item["title"].as_str().unwrap_or("未命名视频")),
                        kind: kind.into(),
                    });
                }
            }
        }
    }
    // Relevance is only discovery order, never evidence that content matches.
    let lower = query.to_lowercase();
    result.hits.sort_by_key(|hit| {
        (
            !hit.title.to_lowercase().contains(&lower),
            hit.kind == "video",
        )
    });
    Ok(result)
}
fn clean_title(value: &str) -> String {
    let mut inside = false;
    value
        .chars()
        .filter(|c| match c {
            '<' => {
                inside = true;
                false
            }
            '>' => {
                inside = false;
                false
            }
            _ => !inside,
        })
        .collect::<String>()
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}
