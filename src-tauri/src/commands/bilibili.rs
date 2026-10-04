use crate::{
    bilibili::auth::{AccountStatus, AuthService, LoginPoll, LoginQr},
    core::error::AppResult,
    danmaku::service::DanmakuService,
};
use tauri::State;
#[tauri::command]
pub fn bilibili_account(auth: State<'_, AuthService>) -> AccountStatus {
    auth.status()
}
#[tauri::command]
pub async fn bilibili_login_start(auth: State<'_, AuthService>) -> AppResult<LoginQr> {
    auth.start().await
}
#[tauri::command]
pub async fn bilibili_login_poll(
    auth: State<'_, AuthService>,
    ticket: String,
) -> AppResult<LoginPoll> {
    auth.poll(&ticket).await
}
#[tauri::command]
pub async fn bilibili_login_cancel(auth: State<'_, AuthService>) -> AppResult<()> {
    auth.cancel_login().await;
    Ok(())
}
#[tauri::command]
pub async fn bilibili_account_verify(auth: State<'_, AuthService>) -> AppResult<AccountStatus> {
    auth.verify().await
}
#[tauri::command]
pub async fn bilibili_logout(
    auth: State<'_, AuthService>,
    dm: State<'_, DanmakuService>,
) -> AppResult<()> {
    dm.cancel();
    auth.logout().await
}
