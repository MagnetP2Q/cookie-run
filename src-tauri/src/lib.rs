mod adb;
mod bot;
mod commands;
mod config;
mod games;
mod logging;
mod ocr;
use commands::*;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;
#[derive(Default)]
struct Closing(AtomicBool);
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(bot::Controller::default())
        .manage(Closing::default())
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            app.manage(logging::Logger::new(app.handle().clone(), dir));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if window
                    .app_handle()
                    .state::<Closing>()
                    .0
                    .swap(true, Ordering::SeqCst)
                {
                    return;
                }
                let window = window.clone();
                std::thread::spawn(move || {
                    let app = window.app_handle().clone();
                    bot::stop_and_join(&app);
                    if let Err(error) = app.state::<logging::Logger>().shutdown() {
                        eprintln!("Could not flush log on close: {error}");
                        app.state::<Closing>().0.store(false, Ordering::SeqCst);
                    } else {
                        let _ = window.destroy();
                    }
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            list_devices,
            get_bot_status,
            start_bot,
            stop_bot,
            get_cycle_logs,
            clear_logs
        ])
        .run(tauri::generate_context!())
        .expect("Tauri failed");
}
