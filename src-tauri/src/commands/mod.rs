use crate::{
    adb,
    bot::{self, Controller, Status},
    config::{self, Settings},
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{AppHandle, Manager, State};
fn dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path().app_config_dir().map_err(|e| e.to_string())
}
#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<Settings, String> {
    config::settings(&dir(&app)?)
}
#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    ctl: State<Controller>,
    adb_path: String,
) -> Result<Settings, String> {
    let i = ctl.inner.lock().map_err(|e| e.to_string())?;
    if i.status.state != "stopped" && i.status.state != "error" {
        return Err("หยุดบอทก่อนเปลี่ยนการตั้งค่า".into());
    }
    config::save(&dir(&app)?, adb_path)
}
#[tauri::command]
pub async fn list_devices(app: AppHandle) -> Result<Vec<adb::DeviceInfo>, String> {
    let s = config::settings(&dir(&app)?)?;
    tauri::async_runtime::spawn_blocking(move || adb::devices_with_names(&s.adb_path))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn get_bot_status(ctl: State<Controller>) -> Result<Status, String> {
    Ok(ctl.inner.lock().map_err(|e| e.to_string())?.status.clone())
}
#[tauri::command]
pub async fn start_bot(app: AppHandle, device: String) -> Result<(), String> {
    if device.trim().is_empty() {
        return Err("กรุณาเลือกอุปกรณ์".into());
    }
    let s = config::settings(&dir(&app)?)?;
    let cfg = config::load(&s.config_path)?;
    let path = s.adb_path.clone();
    let list = tauri::async_runtime::spawn_blocking(move || adb::devices(&path))
        .await
        .map_err(|e| e.to_string())??;
    if !list.contains(&device) {
        return Err("ไม่พบอุปกรณ์ที่เลือก".into());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let ctl = app.state::<Controller>();
        let mut i = ctl.inner.lock().map_err(|e| e.to_string())?;
        if i.status.state != "stopped" && i.status.state != "error" {
            return Err("บอทกำลังทำงานอยู่".into());
        }
        if let Some(previous) = i.worker.take() {
            // The previous worker has already published its terminal state.
            let _ = previous.join();
        }
        i.status = Status {
            state: "starting".into(),
            device: Some(device.clone()),
            message: None,
        };
        i.cancel = Some(cancel.clone());
        use tauri::Emitter;
        let _ = app.emit("bot-status", i.status.clone());
    }
    bot::start(app, device, s.adb_path, cfg, cancel);
    Ok(())
}
#[tauri::command]
pub fn stop_bot(app: AppHandle, ctl: State<Controller>) -> Result<(), String> {
    let mut i = ctl.inner.lock().map_err(|e| e.to_string())?;
    if let Some(c) = &i.cancel {
        c.store(true, Ordering::SeqCst)
    }
    if let Some(worker) = &i.worker {
        worker.thread().unpark();
    }
    if i.status.state == "running" || i.status.state == "starting" {
        i.status.state = "stopping".into();
        use tauri::Emitter;
        let _ = app.emit("bot-status", i.status.clone());
    }
    Ok(())
}

#[tauri::command]
pub async fn get_cycle_logs(app: AppHandle) -> Result<Vec<crate::logging::Entry>, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<crate::logging::Logger>().read())
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn clear_logs(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<crate::logging::Logger>().clear())
        .await
        .map_err(|e| e.to_string())?
}
