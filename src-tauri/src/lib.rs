mod adb;
mod bot;
mod commands;
mod config;
mod games;
mod ocr;
use commands::*;
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(bot::Controller::default())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            list_devices,
            get_bot_status,
            start_bot,
            stop_bot
        ])
        .run(tauri::generate_context!())
        .expect("Tauri failed");
}
