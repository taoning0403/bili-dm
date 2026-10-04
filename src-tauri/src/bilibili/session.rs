use crate::core::error::{AppError, AppResult};
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::Duration,
};
use tokio_util::sync::CancellationToken;
use url::Url;

pub const API: &str = "https://api.bilibili.com";
pub const PASSPORT: &str = "https://passport.bilibili.com";
pub const REFERER: &str = "https://www.bilibili.com/";
pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/131.0.0.0 Safari/537.36";
const COOKIE_NAMES: &[&str] = &[
    "SESSDATA",
    "bili_jct",
    "DedeUserID",
    "DedeUserID__ckMd5",
    "buvid3",
    "buvid4",
    "b_nut",
    "sid",
];

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub uid: u64,
    pub name: String,
    pub vip: bool,
    pub vip_due: u64,
    pub verified_at: u64,
}
#[derive(Clone, Default, Serialize, Deserialize)]
struct SavedSession {
    cookies: BTreeMap<String, String>,
    account: Option<Account>,
}
pub struct BilibiliClient {
    http: Client,
    saved: Mutex<SavedSession>,
    generation: AtomicU64,
    path: Option<PathBuf>,
}
impl BilibiliClient {
    pub fn new(path: Option<PathBuf>) -> AppResult<Self> {
        let mut saved = match path.as_ref().filter(|p| p.is_file()) {
            Some(p) => {
                if std::fs::metadata(p).map_err(AppError::io)?.len() > 65536 {
                    return Err(AppError::new(
                        "ACCOUNT_STORAGE",
                        "本地登录文件过大，请移除后重新扫码。",
                    ));
                }
                serde_json::from_slice::<SavedSession>(&std::fs::read(p).map_err(AppError::io)?)
                    .map_err(|_| {
                        AppError::new("ACCOUNT_STORAGE", "本地登录文件损坏，请移除后重新扫码。")
                    })?
            }
            None => SavedSession::default(),
        };
        saved
            .cookies
            .retain(|k, v| COOKIE_NAMES.contains(&k.as_str()) && valid_cookie_value(v));
        if !saved.cookies.contains_key("SESSDATA") {
            saved.account = None;
        }
        // Device cookie belongs to this application session, never to a browser.
        saved
            .cookies
            .entry("buvid3".into())
            .or_insert_with(|| format!("{}infoc", uuid::Uuid::new_v4()));
        let http = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(25))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| AppError::new("BILIBILI_HTTP", e.without_url().to_string()))?;
        Ok(Self {
            http,
            saved: Mutex::new(saved),
            generation: AtomicU64::new(0),
            path,
        })
    }
    pub fn account(&self) -> Option<Account> {
        self.saved.lock().ok().and_then(|s| s.account.clone())
    }
    pub fn has_login(&self) -> bool {
        self.saved
            .lock()
            .is_ok_and(|s| s.cookies.contains_key("SESSDATA"))
    }
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }
    pub fn cookie_header(&self, url: &Url) -> Option<String> {
        if url.scheme() != "https"
            || !matches!(
                url.host_str(),
                Some("api.bilibili.com" | "passport.bilibili.com")
            )
        {
            return None;
        }
        self.saved.lock().ok().map(|s| {
            s.cookies
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("; ")
        })
    }
    pub async fn bytes(&self, url: &str, cancel: &CancellationToken) -> AppResult<Vec<u8>> {
        let url = Url::parse(url).map_err(|_| AppError::new("BILIBILI_URL", "请求地址无效。"))?;
        if url.scheme() != "https"
            || !matches!(
                url.host_str(),
                Some("api.bilibili.com" | "passport.bilibili.com")
            )
            || url.port().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(AppError::new(
                "BILIBILI_URL",
                "请求不属于受支持的 B 站接口。",
            ));
        }
        let epoch = self.generation();
        let work = async {
            let mut request = self.http.get(url.clone()).header(header::REFERER, REFERER);
            if let Some(cookie) = self.cookie_header(&url) {
                request = request.header(header::COOKIE, cookie);
            }
            let mut response = request
                .send()
                .await
                .map_err(network_error)?
                .error_for_status()
                .map_err(network_error)?;
            // Never follow a redirect while carrying credentials.
            if response.status().is_redirection() {
                return Err(AppError::new(
                    "BILIBILI_REDIRECT",
                    "B 站接口返回了未预期的跳转。",
                ));
            }
            let set_cookies: Vec<String> = response
                .headers()
                .get_all(header::SET_COOKIE)
                .iter()
                .filter_map(|h| h.to_str().ok().map(str::to_owned))
                .collect();
            let mut bytes = vec![];
            while let Some(chunk) = response.chunk().await.map_err(network_error)? {
                if bytes.len() + chunk.len() > 16 * 1024 * 1024 {
                    return Err(AppError::new("BILIBILI_LIMIT", "B 站响应超过 16 MiB。"));
                }
                bytes.extend_from_slice(&chunk);
            }
            if epoch == self.generation() {
                if let Ok(mut saved) = self.saved.lock() {
                    if epoch == self.generation() {
                        for line in set_cookies {
                            if let Some((name, value)) = parse_set_cookie(&line) {
                                if value.is_empty() {
                                    saved.cookies.remove(&name);
                                } else {
                                    saved.cookies.insert(name, value);
                                }
                            }
                        }
                    }
                }
            }
            Ok(bytes)
        };
        tokio::select! { _ = cancel.cancelled() => Err(cancelled()), result = work => result }
    }
    pub async fn json(&self, url: &str, cancel: &CancellationToken) -> AppResult<Value> {
        serde_json::from_slice(&self.bytes(url, cancel).await?).map_err(|_| {
            AppError::new(
                "BILIBILI_RESPONSE",
                "B 站未返回有效 JSON，可能受到访问限制。",
            )
        })
    }
    pub async fn remember(&self, account: Account) -> AppResult<()> {
        let bytes = {
            let mut saved = self
                .saved
                .lock()
                .map_err(|_| AppError::new("ACCOUNT_STATE", "登录状态不可用。"))?;
            saved.account = Some(account);
            serde_json::to_vec(&*saved).map_err(AppError::io)?
        };
        if let Some(path) = &self.path {
            private_write(path, &bytes).await?;
        }
        Ok(())
    }
    pub async fn clear(&self) -> AppResult<()> {
        self.generation.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut saved) = self.saved.lock() {
            *saved = SavedSession::default();
            saved
                .cookies
                .insert("buvid3".into(), format!("{}infoc", uuid::Uuid::new_v4()));
        }
        if let Some(path) = &self.path {
            match tokio::fs::remove_file(path).await {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(AppError::io(e)),
            }
        }
        Ok(())
    }
}
pub fn parse_set_cookie(line: &str) -> Option<(String, String)> {
    let (name, value) = line.split(';').next()?.split_once('=')?;
    if !COOKIE_NAMES.contains(&name) || !valid_cookie_value(value) {
        return None;
    }
    let expired = line
        .split(';')
        .any(|p| p.trim().eq_ignore_ascii_case("max-age=0"));
    Some((
        name.into(),
        if expired { String::new() } else { value.into() },
    ))
}
fn valid_cookie_value(value: &str) -> bool {
    value.len() <= 8192
        && value
            .bytes()
            .all(|b| b.is_ascii_graphic() && b != b';' && b != b',')
}
pub fn cancelled() -> AppError {
    AppError::new("CANCELLED", "已取消任务。")
}
pub fn network_error(e: reqwest::Error) -> AppError {
    AppError::new("BILIBILI_HTTP", e.without_url().to_string())
}
pub fn api_data(value: Value) -> AppResult<Value> {
    if value["code"].as_i64() != Some(0) {
        let code = value["code"].as_i64().unwrap_or(-1);
        let message = match code {
            -101 => "登录已失效，请重新扫码。",
            -10403 | 6002105 => "账号没有此视频的观看权限。",
            -352 | -412 => "请求受到 B 站限制，请稍后再试。",
            _ => "视频不可用或接口暂时不可访问。",
        };
        return Err(AppError::new(
            "BILIBILI_API",
            format!("{message}（{code}）"),
        ));
    }
    let data = value
        .get("data")
        .filter(|v| !v.is_null())
        .or_else(|| value.get("result"));
    data.cloned()
        .ok_or_else(|| AppError::new("BILIBILI_RESPONSE", "B 站数据缺失。"))
}
async fn private_write(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let path = path.to_owned();
    let bytes = bytes.to_vec();
    tokio::task::spawn_blocking(move || {
        use std::io::Write;
        let parent = path
            .parent()
            .ok_or_else(|| AppError::new("ACCOUNT_STORAGE", "登录文件路径无效。"))?;
        std::fs::create_dir_all(parent).map_err(AppError::io)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
                .map_err(AppError::io)?;
        }
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(AppError::io)?;
        file.write_all(&bytes).map_err(AppError::io)?;
        file.as_file().sync_all().map_err(AppError::io)?;
        file.persist(&path).map_err(AppError::io)?;
        Ok(())
    })
    .await
    .map_err(AppError::io)?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credentials_are_scoped_to_api_hosts_and_set_cookie_is_filtered(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let client = BilibiliClient::new(None)?;
        client
            .saved
            .lock()
            .map_err(|_| "session lock")?
            .cookies
            .insert("SESSDATA".into(), "test-session".into());
        assert!(client
            .cookie_header(&Url::parse("https://api.bilibili.com/x/web-interface/nav")?)
            .is_some());
        for host in [
            "https://upos.bilivideo.com/video",
            "https://api.bilibili.com.evil.test/",
            "http://api.bilibili.com/",
        ] {
            assert!(client.cookie_header(&Url::parse(host)?).is_none());
        }
        assert_eq!(
            parse_set_cookie("SESSDATA=sample%2Fvalue; HttpOnly; Secure").map(|p| p.0),
            Some("SESSDATA".into())
        );
        assert_eq!(
            parse_set_cookie("SESSDATA=x; Max-Age=0").map(|p| p.1),
            Some(String::new())
        );
        assert!(parse_set_cookie("OTHER=value").is_none());
        assert!(parse_set_cookie("SESSDATA=x\r\nInjected=y").is_none());
        Ok(())
    }
    #[tokio::test]
    async fn local_account_survives_restart_and_logout_removes_it(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("account/bilibili.json");
        let client = BilibiliClient::new(Some(path.clone()))?;
        client
            .saved
            .lock()
            .map_err(|_| "session lock")?
            .cookies
            .insert("SESSDATA".into(), "test-session".into());
        client
            .remember(Account {
                uid: 123,
                name: "test".into(),
                ..Default::default()
            })
            .await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path)?.permissions().mode() & 0o777,
                0o600
            );
        }
        let reopened = BilibiliClient::new(Some(path.clone()))?;
        assert!(reopened.has_login());
        assert_eq!(reopened.account().map(|a| a.uid), Some(123));
        reopened.clear().await?;
        assert!(!path.exists());
        assert!(!reopened.has_login());
        assert!(reopened.account().is_none());
        Ok(())
    }
}
