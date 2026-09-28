use crate::{
    core::{
        app_service::AppService,
        error::{AppError, AppResult},
        playback_service::{ActiveMedia, PlaybackState},
    },
    player::PlayerControl,
};
use tauri::State;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn play_torrent(
    service: State<'_, AppService>,
    torrent_id: String,
    file_index: usize,
) -> AppResult<ActiveMedia> {
    service.playback.play_torrent(&torrent_id, file_index).await
}

#[tauri::command]
pub async fn open_local_video(
    app: tauri::AppHandle,
    service: State<'_, AppService>,
) -> AppResult<Option<ActiveMedia>> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter(
            "视频",
            &["mp4", "mkv", "avi", "webm", "mov", "m4v", "ts", "m2ts"],
        )
        .pick_file(move |path| {
            let _ = sender.send(path);
        });
    let path = receiver
        .await
        .map_err(|_| AppError::new("DIALOG", "文件选择窗口意外关闭。"))?;
    match path {
        Some(path) => Ok(Some(
            service
                .playback
                .play_local(
                    path.into_path()
                        .map_err(|_| AppError::new("INVALID_PATH", "请选择本地文件。"))?,
                )
                .await?,
        )),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn control_player(
    service: State<'_, AppService>,
    control: PlayerControl,
) -> AppResult<()> {
    service.playback.control(control).await
}

#[tauri::command]
pub async fn get_playback_state(service: State<'_, AppService>) -> AppResult<PlaybackState> {
    service.playback.snapshot().await
}
