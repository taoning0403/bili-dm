mod commands;
pub mod core;
pub mod database;
pub mod media;
pub mod player;
pub mod torrent;

use core::app_service::AppService;

pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .manage(AppService::default())
        .invoke_handler(tauri::generate_handler![
            commands::runtime::get_runtime_info
        ])
        .run(tauri::generate_context!())
}
