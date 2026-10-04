use crate::{
    core::{app_service::AppService, error::AppResult},
    torrent::models::TorrentCatalog,
};
use tauri::State;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn resolve_magnet(
    service: State<'_, AppService>,
    magnet: String,
) -> AppResult<TorrentCatalog> {
    service.resolve_magnet(&magnet).await
}

#[tauri::command]
pub async fn cancel_magnet(service: State<'_, AppService>) -> AppResult<()> {
    service.torrent.cancel_resolution().await;
    Ok(())
}

#[tauri::command]
pub async fn open_torrent_file(
    app: tauri::AppHandle,
    service: State<'_, AppService>,
) -> AppResult<Option<TorrentCatalog>> {
    use crate::core::error::AppError;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("种子", &["torrent"])
        .pick_file(move |path| {
            let _ = sender.send(path);
        });
    let Some(path) = receiver
        .await
        .map_err(|_| AppError::new("DIALOG", "种子选择窗口已关闭。"))?
    else {
        return Ok(None);
    };
    let path = path
        .into_path()
        .map_err(|_| AppError::new("INVALID_PATH", "请选择本地种子文件。"))?;
    if tokio::fs::metadata(&path)
        .await
        .map_err(AppError::io)?
        .len()
        > 16 * 1024 * 1024
    {
        return Err(AppError::new("TORRENT_SIZE", "种子文件不能超过 16 MiB。"));
    }
    Ok(Some(
        service
            .import_torrent(tokio::fs::read(path).await.map_err(AppError::io)?)
            .await?,
    ))
}
