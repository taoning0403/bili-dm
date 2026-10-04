use super::session::{api_data, Account, BilibiliClient, API, PASSPORT};
use crate::core::error::{AppError, AppResult};
use qrcode::{Color, QrCode};
use serde::Serialize;
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    pub state: String,
    pub account: Option<Account>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginQr {
    pub ticket: String,
    pub pixels: Vec<Vec<bool>>,
    pub expires_at: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginPoll {
    pub state: String,
    pub account: Option<Account>,
}
struct Challenge {
    ticket: String,
    key: String,
    expires: u64,
}
pub struct AuthService {
    client: Arc<BilibiliClient>,
    challenge: Mutex<Option<Challenge>>,
}
impl AuthService {
    pub fn new(client: Arc<BilibiliClient>) -> Self {
        Self {
            client,
            challenge: Mutex::new(None),
        }
    }
    pub fn status(&self) -> AccountStatus {
        AccountStatus {
            state: if self.client.has_login() {
                "saved"
            } else {
                "signedOut"
            }
            .into(),
            account: self.client.account(),
        }
    }
    pub async fn start(&self) -> AppResult<LoginQr> {
        let mut challenge = self.challenge.lock().await;
        let data = api_data(
            self.client
                .json(
                    &format!("{PASSPORT}/x/passport-login/web/qrcode/generate"),
                    &CancellationToken::new(),
                )
                .await?,
        )?;
        let key = data["qrcode_key"]
            .as_str()
            .filter(|v| v.len() <= 128)
            .ok_or_else(invalid)?;
        let url = data["url"].as_str().ok_or_else(invalid)?;
        let parsed = url::Url::parse(url).map_err(|_| invalid())?;
        if parsed.scheme() != "https"
            || !matches!(
                parsed.host_str(),
                Some("passport.bilibili.com" | "account.bilibili.com")
            )
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.port().is_some()
        {
            return Err(invalid());
        }
        let qr = QrCode::new(url).map_err(|_| invalid())?;
        let width = qr.width();
        let pixels = qr
            .to_colors()
            .chunks(width)
            .map(|row| row.iter().map(|v| *v == Color::Dark).collect())
            .collect();
        let ticket = uuid::Uuid::new_v4().to_string();
        let expires = now() + 180;
        *challenge = Some(Challenge {
            ticket: ticket.clone(),
            key: key.into(),
            expires,
        });
        Ok(LoginQr {
            ticket,
            pixels,
            expires_at: expires,
        })
    }
    pub async fn poll(&self, ticket: &str) -> AppResult<LoginPoll> {
        let mut pending = self.challenge.lock().await;
        let challenge = pending
            .as_ref()
            .filter(|c| c.ticket == ticket)
            .ok_or_else(|| AppError::new("LOGIN_EXPIRED", "二维码已更换，请重新扫码。"))?;
        if now() >= challenge.expires {
            *pending = None;
            return Ok(LoginPoll {
                state: "expired".into(),
                account: None,
            });
        }
        let mut url = url::Url::parse(&format!("{PASSPORT}/x/passport-login/web/qrcode/poll"))
            .map_err(|_| invalid())?;
        url.query_pairs_mut()
            .append_pair("qrcode_key", &challenge.key);
        let data = api_data(
            self.client
                .json(url.as_str(), &CancellationToken::new())
                .await?,
        )?;
        let state = match data["code"].as_i64() {
            Some(86101) => "waiting",
            Some(86090) => "scanned",
            Some(86038) => "expired",
            Some(0) => "signedIn",
            _ => return Err(invalid()),
        };
        let account = if state == "signedIn" {
            Some(self.verify_account().await?)
        } else {
            None
        };
        if matches!(state, "signedIn" | "expired") {
            *pending = None;
        }
        Ok(LoginPoll {
            state: state.into(),
            account,
        })
    }
    async fn verify_account(&self) -> AppResult<Account> {
        let data = api_data(
            self.client
                .json(
                    &format!("{API}/x/web-interface/nav"),
                    &CancellationToken::new(),
                )
                .await?,
        )?;
        if data["isLogin"].as_bool() != Some(true) || !self.client.has_login() {
            return Err(AppError::new("LOGIN_EXPIRED", "登录已失效，请重新扫码。"));
        }
        let account = Account {
            uid: data["mid"].as_u64().unwrap_or(0),
            name: data["uname"].as_str().unwrap_or("B 站用户").into(),
            vip: data["vipStatus"]
                .as_u64()
                .or(data["vip"]["status"].as_u64())
                == Some(1),
            vip_due: data["vipDueDate"]
                .as_u64()
                .or(data["vip"]["due_date"].as_u64())
                .unwrap_or(0),
            verified_at: now(),
        };
        if account.uid == 0 {
            return Err(invalid());
        }
        self.client.remember(account.clone()).await?;
        Ok(account)
    }
    pub async fn verify(&self) -> AppResult<AccountStatus> {
        let _guard = self.challenge.lock().await;
        match self.verify_account().await {
            Ok(account) => Ok(AccountStatus {
                state: "signedIn".into(),
                account: Some(account),
            }),
            Err(e) if e.code == "LOGIN_EXPIRED" || e.message.contains("（-101）") => {
                self.client.clear().await?;
                Ok(AccountStatus {
                    state: "expired".into(),
                    account: None,
                })
            }
            Err(e) => Err(e),
        }
    }
    pub async fn logout(&self) -> AppResult<()> {
        let mut challenge = self.challenge.lock().await;
        *challenge = None;
        self.client.clear().await
    }
    pub async fn cancel_login(&self) {
        *self.challenge.lock().await = None;
    }
}
fn invalid() -> AppError {
    AppError::new("LOGIN_RESPONSE", "B 站登录响应无效，请重新获取二维码。")
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
