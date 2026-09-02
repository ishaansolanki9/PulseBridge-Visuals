use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

use tauri::{AppHandle, Manager};

use crate::{diagnostics, visuals::VisualSettings};

pub fn load_settings(app: &AppHandle) -> VisualSettings {
    let Ok(path) = settings_path(app) else {
        diagnostics::event(
            "warn",
            "settings.path.unavailable",
            "Using default settings because the config path is unavailable",
        );
        return VisualSettings::default();
    };
    match fs::read_to_string(path) {
        Ok(contents) => match serde_json::from_str::<VisualSettings>(&contents) {
            Ok(settings) => settings.sanitized(),
            Err(error) => {
                diagnostics::event(
                    "warn",
                    "settings.parse.failed",
                    &format!("Using defaults: {error}"),
                );
                VisualSettings::default()
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => VisualSettings::default(),
        Err(error) => {
            diagnostics::event(
                "warn",
                "settings.read.failed",
                &format!("Using defaults: {error}"),
            );
            VisualSettings::default()
        }
    }
}

pub fn save_settings(app: &AppHandle, settings: &VisualSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    let parent = path
        .parent()
        .ok_or_else(|| "Settings path has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary_path = path.with_extension("json.tmp");
    let payload = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
    let mut file = File::create(&temporary_path).map_err(|error| error.to_string())?;
    file.write_all(&payload)
        .map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    if path.exists() {
        fs::remove_file(&path).map_err(|error| error.to_string())?;
    }
    fs::rename(temporary_path, path).map_err(|error| error.to_string())
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|path| path.join("visual-settings.json"))
        .map_err(|error| error.to_string())
}
