mod analysis;
mod audio;
mod commands;
mod config;
mod display;
mod resilience;
mod visuals;

use commands::{
    get_audio_sources, get_displays, get_runtime_state, set_output_mode, start_visuals,
    stop_visuals, update_visual_settings,
};
use config::load_settings;
use display::PerformanceManager;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let settings = load_settings(app.handle());
            app.manage(PerformanceManager::new(settings));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_displays,
            get_audio_sources,
            get_runtime_state,
            update_visual_settings,
            start_visuals,
            stop_visuals,
            set_output_mode
        ])
        .run(tauri::generate_context!())
        .expect("PulseBridge Visuals failed to start");
}
