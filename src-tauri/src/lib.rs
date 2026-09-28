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
        .setup(|app| {
            let cache = app.path().app_cache_dir()?.join("torrents");
            app.manage(AppService::new(Arc::new(RqbitEngine::new(cache))));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::runtime::get_runtime_info,
            commands::torrent::resolve_magnet,
            commands::torrent::cancel_magnet
        ])
        .build(tauri::generate_context!())?;
    app.run(|handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            if let Some(service) = handle.try_state::<AppService>() {
                tauri::async_runtime::block_on(service.torrent.shutdown());
            }
        }
    });
    Ok(())
}
