mod analysis;
mod audio;
mod commands;
mod config;
mod diagnostic_runner;
mod diagnostics;
mod display;
mod phrase;
mod resilience;
mod visuals;

use commands::{
    cancel_connection_diagnostic, get_audio_sources, get_displays, get_latest_diagnostic_report,
    get_previous_run_report, get_runtime_state, open_logs_folder, run_connection_diagnostic,
    set_output_mode, start_visuals, stop_visuals, update_visual_settings,
};
use config::load_settings;
use diagnostic_runner::DiagnosticCoordinator;
use display::PerformanceManager;
use tauri::{Manager, RunEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .setup(|app| {
            let log_path = diagnostics::initialize(app.handle()).ok();
            let settings_stage = diagnostics::begin_stage(
                "settings.load",
                "SETTINGS_LOAD_BEGIN",
                "Loading visual settings",
                serde_json::Value::Null,
            );
            let settings = load_settings(app.handle());
            settings_stage.pass(
                "SETTINGS_LOADED",
                "Visual settings loaded",
                serde_json::Value::Null,
            );
            app.manage(PerformanceManager::new(settings, log_path));
            app.manage(DiagnosticCoordinator::default());
            #[cfg(debug_assertions)]
            if std::env::var_os("PULSEBRIDGE_SMOKE_AUTOSTART").is_some() {
                let smoke_handle = app.handle().clone();
                tauri::async_runtime::spawn_blocking(move || {
                    std::thread::sleep(std::time::Duration::from_millis(750));
                    diagnostics::critical_event(
                        "info",
                        "smoke.autostart.begin",
                        "SMOKE_AUTOSTART_BEGIN",
                        "Starting the debug-only native performance smoke test",
                        serde_json::Value::Null,
                    );
                    let manager = smoke_handle.state::<PerformanceManager>();
                    let outcome = manager.start(&smoke_handle);
                    if outcome.is_ok() {
                        std::thread::sleep(std::time::Duration::from_secs(1));
                    }
                    let _ = manager.stop();
                    let (level, code, message, exit_code) = match outcome {
                        Ok(()) => (
                            "info",
                            "SMOKE_AUTOSTART_PASS",
                            "Debug-only native performance smoke test passed",
                            0,
                        ),
                        Err(ref error) => ("error", "SMOKE_AUTOSTART_FAILED", error.as_str(), 1),
                    };
                    diagnostics::critical_event(
                        level,
                        "smoke.autostart.complete",
                        code,
                        message,
                        serde_json::Value::Null,
                    );
                    smoke_handle.exit(exit_code);
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_displays,
            get_audio_sources,
            get_runtime_state,
            update_visual_settings,
            start_visuals,
            stop_visuals,
            set_output_mode,
            open_logs_folder,
            run_connection_diagnostic,
            cancel_connection_diagnostic,
            get_latest_diagnostic_report,
            get_previous_run_report
        ])
        .build(tauri::generate_context!())
        .unwrap_or_else(|error| {
            diagnostics::event(
                "error",
                "application.runtime.failed",
                &format!("PulseBridge runtime failed: {error}"),
            );
            panic!("PulseBridge runtime failed: {error}");
        });
    app.run(|handle, event| match event {
        RunEvent::ExitRequested { .. } => {
            let manager = handle.state::<PerformanceManager>();
            let _ = manager.stop();
            diagnostics::mark_clean_exit("PulseBridge exited after session cleanup");
        }
        RunEvent::Exit => diagnostics::mark_clean_exit("PulseBridge native runtime exited cleanly"),
        _ => {}
    });
}
