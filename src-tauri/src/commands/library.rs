use crate::{
    core::{app_service::AppService, error::AppResult},
    database::models::TorrentRecord,
};
use tauri::State;

#[tauri::command]
pub async fn recent_torrents(service: State<'_, AppService>) -> AppResult<Vec<TorrentRecord>> {
    service.recent_torrents().await
}
