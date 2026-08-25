use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

use tauri::{AppHandle, Manager};

use crate::visuals::VisualSettings;

pub fn load_settings(app: &AppHandle) -> VisualSettings {
    let Ok(path) = settings_path(app) else {
        return VisualSettings::default();
    };
    fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str::<VisualSettings>(&contents).ok())
        .unwrap_or_default()
        .sanitized()
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
