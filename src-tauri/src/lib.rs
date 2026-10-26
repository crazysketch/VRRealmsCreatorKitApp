mod applog;
mod commands;
mod config;
mod engine;
mod epic;
mod error;
mod ini;
mod kit;
mod net;
mod news;
mod steam;
mod winutil;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if let Ok(dir) = app.path().app_data_dir() {
                let _ = std::fs::create_dir_all(&dir);
                applog::init(&dir);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::set_preferences,
            commands::get_overview,
            commands::set_engine_path,
            commands::install_epic_launcher,
            commands::open_unreal_page,
            commands::install_steamcmd,
            commands::steam_login,
            commands::list_kits,
            commands::add_kit,
            commands::forget_kit,
            commands::set_current_kit,
            commands::discover_kits,
            commands::suggest_kit_folder,
            commands::install_kit,
            commands::check_kit_update,
            commands::update_kit,
            commands::verify_kit,
            commands::repair_kit,
            commands::open_kit,
            commands::show_in_explorer,
            commands::get_news,
            commands::get_log,
            commands::app_info,
        ])
        .run(tauri::generate_context!())
        .expect("the app could not start");
}
