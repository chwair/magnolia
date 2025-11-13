// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod torrent;

use std::sync::Arc;
use torrent::TorrentManager;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_dir = app.path().app_data_dir()
                .expect("Failed to get app data directory");
            let torrent_dir = app_dir.join("torrents");

            let manager = tauri::async_runtime::block_on(async {
                TorrentManager::new(torrent_dir)
                    .await
                    .expect("Failed to initialize torrent manager")
            });

            app.manage(Arc::new(manager));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            torrent::add_torrent,
            torrent::get_torrent_info,
            torrent::list_torrents,
            torrent::start_stream,
            torrent::stop_stream,
            torrent::pause_torrent,
            torrent::resume_torrent,
            torrent::remove_torrent,
            torrent::get_download_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
