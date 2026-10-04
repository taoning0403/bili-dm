use crate::{
    core::error::{AppError, AppResult},
    danmaku::{
        models::{Clip, JobStatus, Workspace},
        service::{DanmakuService, MatchResult},
    },
};
use tauri::State;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn get_danmaku_workspace(
    service: State<'_, DanmakuService>,
    session_id: String,
) -> AppResult<Workspace> {
    service.workspace(&session_id).await
}
#[tauri::command]
pub async fn resolve_danmaku(
    service: State<'_, DanmakuService>,
    session_id: String,
    inputs: Vec<String>,
) -> AppResult<Workspace> {
    service.resolve(&session_id, inputs).await
}
#[tauri::command]
pub async fn match_danmaku(
    service: State<'_, DanmakuService>,
    session_id: String,
    source_ids: Vec<String>,
) -> AppResult<MatchResult> {
    service.auto_match(&session_id, source_ids).await
}
#[tauri::command]
pub async fn apply_danmaku(
    service: State<'_, DanmakuService>,
    session_id: String,
    clips: Vec<Clip>,
) -> AppResult<Workspace> {
    service.apply(&session_id, clips).await
}
#[tauri::command]
pub fn get_danmaku_job(service: State<'_, DanmakuService>) -> JobStatus {
    service.status()
}
#[tauri::command]
pub fn cancel_danmaku(service: State<'_, DanmakuService>) {
    service.cancel();
}
#[tauri::command]
pub async fn export_danmaku(
    app: tauri::AppHandle,
    service: State<'_, DanmakuService>,
    session_id: String,
) -> AppResult<Option<String>> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Bilibili 弹幕 XML", &["xml"])
        .set_file_name("mixed-danmaku.xml")
        .save_file(move |path| {
            let _ = sender.send(path);
        });
    let path = receiver
        .await
        .map_err(|_| AppError::new("DIALOG", "保存窗口意外关闭。"))?;
    match path {
        Some(path) => Ok(Some(
            service
                .export(
                    &session_id,
                    path.into_path()
                        .map_err(|_| AppError::new("INVALID_PATH", "请选择本地文件。"))?,
                )
                .await?,
        )),
        None => Ok(None),
    }
}
