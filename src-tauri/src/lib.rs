pub mod bilibili;
mod commands;
pub mod core;
pub mod danmaku;
pub mod database;
pub mod media;
pub mod player;
pub mod torrent;

use core::app_service::AppService;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::Manager;
use torrent::rqbit::RqbitEngine;

pub fn run() -> tauri::Result<()> {
    // Configure the Vulkan loader before starting Tauri's worker threads.
    #[cfg(target_os = "macos")]
    {
        let resource_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("../Resources")))
            .unwrap_or_default();
        let icd = player::mpv::runtime_directory(&resource_dir).join("MoltenVK_icd.json");
        if icd.is_file() {
            std::env::set_var("VK_DRIVER_FILES", &icd);
            std::env::set_var("VK_ICD_FILENAMES", &icd);
        }
    }
    let closing = Arc::new(AtomicBool::new(false));
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .ok_or("main window missing")?;
            let wid = match window.window_handle()?.as_raw() {
                RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr() as i64,
                RawWindowHandle::Win32(handle) => handle.hwnd.get() as i64,
                RawWindowHandle::Xlib(handle) => handle.window as i64,
                RawWindowHandle::Xcb(handle) => handle.window.get() as i64,
                _ => return Err("此窗口系统暂不支持 libmpv 嵌入。".into()),
            };
            let data_dir = app.path().app_data_dir()?;
            let cache_dir = app.path().app_cache_dir()?;
            #[cfg(debug_assertions)]
            let (data_dir, cache_dir) = std::env::var_os("BILI_DM_TEST_ROOT")
                .map(std::path::PathBuf::from)
                .map(|p| (p.join("data"), p.join("cache")))
                .unwrap_or((data_dir, cache_dir));
            let library = database::sqlite::SqliteLibrary::open(&data_dir.join("library.sqlite3"))?;
            let service = AppService::new(
                Arc::new(RqbitEngine::new(cache_dir.join("torrents"))),
                Arc::new(player::mpv::MpvBackend::embedded(
                    app.path().resource_dir()?,
                    wid,
                )),
                Arc::new(library),
            );
            tauri::async_runtime::spawn(service.playback.clone().monitor());
            let bilibili = Arc::new(bilibili::session::BilibiliClient::new(Some(
                data_dir.join("account/bilibili.json"),
            ))?);
            app.manage(bilibili::auth::AuthService::new(bilibili.clone()));
            app.manage(danmaku::service::DanmakuService::new(
                service.playback.clone(),
                Arc::new(danmaku::bilibili::BilibiliProvider::with_session(bilibili)?),
                app.path().resource_dir()?,
                data_dir.join("danmaku"),
            ));
            app.manage(service);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::runtime::get_runtime_info,
            commands::torrent::resolve_magnet,
            commands::torrent::cancel_magnet,
            commands::torrent::open_torrent_file,
            commands::player::play_torrent,
            commands::player::open_local_video,
            commands::player::control_player,
            commands::player::get_playback_state,
            commands::player::select_queue,
            commands::player::set_queue_options,
            commands::player::set_video_viewport,
            commands::player::set_player_fullscreen,
            commands::player::open_subtitle,
            commands::library::recent_torrents,
            commands::bilibili::bilibili_account,
            commands::bilibili::bilibili_login_start,
            commands::bilibili::bilibili_login_poll,
            commands::bilibili::bilibili_login_cancel,
            commands::bilibili::bilibili_account_verify,
            commands::bilibili::bilibili_logout,
            commands::danmaku::search_danmaku,
            commands::danmaku::get_danmaku_workspace,
            commands::danmaku::resolve_danmaku,
            commands::danmaku::match_danmaku,
            commands::danmaku::apply_danmaku,
            commands::danmaku::preview_danmaku,
            commands::danmaku::get_danmaku_job,
            commands::danmaku::cancel_danmaku,
            commands::danmaku::export_danmaku,
        ])
        .build(tauri::generate_context!())?;
    app.run(move |handle, event| match event {
        tauri::RunEvent::WindowEvent {
            event: tauri::WindowEvent::CloseRequested { api, .. },
            ..
        } => {
            api.prevent_close();
            if !closing.swap(true, Ordering::SeqCst) {
                handle.state::<danmaku::service::DanmakuService>().cancel();
                let playback = handle.state::<AppService>().playback.clone();
                let handle = handle.clone();
                tauri::async_runtime::spawn(async move {
                    playback.shutdown().await;
                    handle.exit(0);
                });
            }
        }
        tauri::RunEvent::ExitRequested { api, .. } if !closing.load(Ordering::SeqCst) => {
            api.prevent_exit();
            if !closing.swap(true, Ordering::SeqCst) {
                handle.state::<danmaku::service::DanmakuService>().cancel();
                let playback = handle.state::<AppService>().playback.clone();
                let handle = handle.clone();
                tauri::async_runtime::spawn(async move {
                    playback.shutdown().await;
                    handle.exit(0);
                });
            }
        }
        _ => {}
    });
    Ok(())
}
