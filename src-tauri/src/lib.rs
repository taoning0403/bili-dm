mod commands;
pub mod core;
pub mod database;
pub mod media;
pub mod player;
pub mod torrent;

use core::app_service::AppService;
use std::sync::Arc;
use tauri::Manager;
use torrent::rqbit::RqbitEngine;

pub fn run() -> tauri::Result<()> {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let cache = app.path().app_cache_dir()?.join("torrents");
            let library = database::sqlite::SqliteLibrary::open(
                &app.path().app_data_dir()?.join("library.sqlite3"),
            )?;
            app.manage(AppService::new(
                Arc::new(RqbitEngine::new(cache)),
                Arc::new(player::mpv::MpvBackend::new(app.path().resource_dir()?)),
                Arc::new(library),
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::runtime::get_runtime_info,
            commands::torrent::resolve_magnet,
            commands::torrent::cancel_magnet,
            commands::player::play_torrent,
            commands::player::open_local_video,
            commands::player::control_player,
            commands::player::get_playback_state,
            commands::library::recent_torrents
        ])
        .build(tauri::generate_context!())?;
    app.run(|handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            if let Some(service) = handle.try_state::<AppService>() {
                tauri::async_runtime::block_on(service.playback.shutdown());
            }
        }
    });
    Ok(())
}
