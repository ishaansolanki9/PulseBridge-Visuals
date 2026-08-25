use tauri::{AppHandle, State};

use crate::{
    audio::{enumerate_audio_sources, AudioSourceInfo},
    config::save_settings,
    display::{enumerate_displays, DisplayInfo, OutputMode, PerformanceManager, RuntimeSnapshot},
    visuals::VisualSettings,
};

#[tauri::command]
pub fn get_displays(app: AppHandle) -> Result<Vec<DisplayInfo>, String> {
    enumerate_displays(&app)
}

#[tauri::command]
pub fn get_audio_sources() -> Result<Vec<AudioSourceInfo>, String> {
    enumerate_audio_sources()
}

#[tauri::command]
pub fn get_runtime_state(manager: State<'_, PerformanceManager>) -> RuntimeSnapshot {
    manager.snapshot()
}

#[tauri::command]
pub fn update_visual_settings(
    app: AppHandle,
    manager: State<'_, PerformanceManager>,
    settings: VisualSettings,
) -> Result<(), String> {
    let settings = settings.sanitized();
    manager.update_settings(settings.clone());
    save_settings(&app, &settings)
}

#[tauri::command]
pub async fn start_visuals(
    app: AppHandle,
    manager: State<'_, PerformanceManager>,
    settings: VisualSettings,
) -> Result<(), String> {
    let settings = settings.sanitized();
    manager.update_settings(settings.clone());
    save_settings(&app, &settings)?;
    manager.start(&app)
}

#[tauri::command]
pub async fn stop_visuals(manager: State<'_, PerformanceManager>) -> Result<(), String> {
    manager.stop()
}

#[tauri::command]
pub fn set_output_mode(manager: State<'_, PerformanceManager>, mode: OutputMode) {
    manager.set_output_mode(mode);
}
