use crate::{
    core::{
        app_service::AppService,
        error::{AppError, AppResult},
        playback_service::{ActiveMedia, PlaybackState, QueueOptions},
    },
    player::{PlayerControl, VideoViewport},
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
pub async fn get_playback_state(
    window: tauri::WebviewWindow,
    service: State<'_, AppService>,
) -> AppResult<PlayerViewState> {
    Ok(PlayerViewState {
        playback: service.playback.snapshot().await?,
        fullscreen: window
            .is_fullscreen()
            .map_err(|e| AppError::new("WINDOW", e.to_string()))?,
    })
}

#[derive(serde::Serialize)]
pub struct PlayerViewState {
    #[serde(flatten)]
    playback: PlaybackState,
    fullscreen: bool,
}

#[tauri::command]
pub async fn select_queue(service: State<'_, AppService>, index: usize) -> AppResult<ActiveMedia> {
    service.playback.select_queue(index).await
}
#[tauri::command]
pub async fn set_queue_options(
    service: State<'_, AppService>,
    options: QueueOptions,
) -> AppResult<()> {
    service.playback.queue_options(options).await
}
#[tauri::command]
pub async fn set_video_viewport(
    service: State<'_, AppService>,
    viewport: VideoViewport,
) -> AppResult<()> {
    service.playback.viewport(viewport).await
}
#[tauri::command]
pub async fn set_player_fullscreen(
    window: tauri::WebviewWindow,
    fullscreen: bool,
) -> AppResult<()> {
    window
        .set_fullscreen(fullscreen)
        .map_err(|e| AppError::new("WINDOW", e.to_string()))
}
#[tauri::command]
pub async fn open_subtitle(app: tauri::AppHandle, service: State<'_, AppService>) -> AppResult<()> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("字幕", &["srt", "ass", "ssa", "vtt", "sub", "idx"])
        .pick_file(move |path| {
            let _ = sender.send(path);
        });
    if let Some(path) = receiver
        .await
        .map_err(|_| AppError::new("DIALOG", "字幕选择窗口已关闭。"))?
    {
        service
            .playback
            .add_subtitle(
                path.into_path()
                    .map_err(|_| AppError::new("INVALID_PATH", "请选择本地字幕。"))?,
            )
            .await?;
    }
    Ok(())
}
