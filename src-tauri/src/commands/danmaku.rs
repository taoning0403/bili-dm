use crate::{
    core::error::{AppError, AppResult},
    danmaku::{
        models::{Clip, JobStatus, Workspace},
        service::{DanmakuService, MatchResult, SearchResult},
        strategy::MatchOptions,
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
    options: Option<MatchOptions>,
) -> AppResult<MatchResult> {
    service
        .match_with_options(&session_id, source_ids, options.unwrap_or_default())
        .await
}
#[tauri::command]
pub async fn search_danmaku(
    service: State<'_, DanmakuService>,
    session_id: String,
    query: String,
    page: u32,
    options: MatchOptions,
) -> AppResult<SearchResult> {
    service
        .search_and_match(&session_id, query, page, options)
        .await
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
pub async fn preview_danmaku(
    service: State<'_, DanmakuService>,
    session_id: String,
    source_id: String,
    source_time: f64,
    target_time: f64,
) -> AppResult<crate::danmaku::engine::AlignmentPreview> {
    service
        .preview(&session_id, &source_id, source_time, target_time)
        .await
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
