use tauri::{AppHandle, State};

use crate::{
    audio::{enumerate_audio_sources, AudioSourceInfo},
    config::save_settings,
    diagnostic_runner::DiagnosticCoordinator,
    diagnostics::{self, DiagnosticMode, DiagnosticReport},
    display::{enumerate_displays, DisplayInfo, OutputMode, PerformanceManager, RuntimeSnapshot},
    visuals::VisualSettings,
};

#[tauri::command]
pub async fn run_connection_diagnostic(
    app: AppHandle,
    manager: State<'_, PerformanceManager>,
    coordinator: State<'_, DiagnosticCoordinator>,
    mode: DiagnosticMode,
) -> Result<DiagnosticReport, String> {
    if manager.snapshot().running {
        return Err("Stop live visuals before running a connection diagnostic".to_string());
    }
    let settings = manager.settings();
    let coordinator = coordinator.inner().clone();
    tauri::async_runtime::spawn_blocking(move || coordinator.run(app, mode, settings))
        .await
        .map_err(|error| format!("Diagnostic worker failed: {error}"))?
}

#[tauri::command]
pub fn cancel_connection_diagnostic(coordinator: State<'_, DiagnosticCoordinator>) -> bool {
    coordinator.cancel()
}

#[tauri::command]
pub fn get_latest_diagnostic_report() -> Option<DiagnosticReport> {
    diagnostics::latest_report()
}

#[tauri::command]
pub fn get_previous_run_report() -> Option<DiagnosticReport> {
    diagnostics::previous_report()
}

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
    coordinator: State<'_, DiagnosticCoordinator>,
    settings: VisualSettings,
) -> Result<(), String> {
    if coordinator.is_active() {
        return Err("Cancel the connection diagnostic before starting live visuals".to_string());
    }
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

#[tauri::command]
pub fn open_logs_folder() -> Result<(), String> {
    let path = diagnostics::log_path()
        .and_then(|path| path.parent().map(ToOwned::to_owned))
        .ok_or_else(|| "The diagnostic log folder is unavailable".to_string())?;
    diagnostics::event("info", "logs.open", "Opening diagnostic log folder");
    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new("explorer.exe");
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");
    command
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Unable to open the diagnostic log folder: {error}"))
}
