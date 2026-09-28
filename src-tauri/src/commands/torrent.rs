use crate::{
    core::{app_service::AppService, error::AppResult},
    torrent::models::TorrentCatalog,
};
use tauri::State;

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
