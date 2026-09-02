use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, MutexGuard, OnceLock},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

const MAX_LOG_BYTES: u64 = 1_000_000;
const ROTATED_FILES: usize = 3;
const RECENT_EVENT_LIMIT: usize = 128;
const LOG_FILE_NAME: &str = "pulsebridge.log";
const MARKER_FILE_NAME: &str = "last-session.json";
const LATEST_REPORT_FILE_NAME: &str = "latest-diagnostic.json";
const PREVIOUS_REPORT_FILE_NAME: &str = "previous-run-report.json";

static LOG: OnceLock<DiagnosticLog> = OnceLock::new();

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticMode {
    #[default]
    AudioOnly,
    RendererOnly,
    FullStartup,
    SafeRenderer,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticVerdict {
    Pass,
    Degraded,
    #[default]
    Fail,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticStageStatus {
    #[default]
    Pending,
    Running,
    Pass,
    Degraded,
    Fail,
    Cancelled,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DiagnosticStageResult {
    pub stage: String,
    pub status: DiagnosticStageStatus,
    pub duration_ms: u64,
    pub code: String,
    pub message: String,
    pub details: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DiagnosticAppInfo {
    pub version: String,
    pub platform: String,
    pub os_version: String,
    pub arch: String,
    pub target: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DiagnosticAudioInfo {
    pub process_detected: bool,
    pub capture_initialized: bool,
    pub packets_received: bool,
    pub non_silent_samples_received: bool,
    pub reactive_ready: bool,
    pub route: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub format: Option<String>,
    pub captured_frames: u64,
    pub rms: f32,
    pub peak: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DiagnosticRendererInfo {
    pub adapter: Option<String>,
    pub backend: Option<String>,
    pub driver: Option<String>,
    pub driver_info: Option<String>,
    pub device_type: Option<String>,
    pub surface_format: Option<String>,
    pub present_mode: Option<String>,
    pub shader_validated: bool,
    pub pipeline_created: bool,
    pub surface_tested: bool,
    pub software_fallback: bool,
    pub safe_mode: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DiagnosticReport {
    pub schema_version: u32,
    pub report_id: String,
    pub session_id: Option<String>,
    pub mode: DiagnosticMode,
    pub started_at: String,
    pub duration_ms: u64,
    pub app: DiagnosticAppInfo,
    pub verdict: DiagnosticVerdict,
    pub failure_stage: Option<String>,
    pub failure_code: Option<String>,
    pub summary: String,
    pub stages: Vec<DiagnosticStageResult>,
    pub audio: DiagnosticAudioInfo,
    pub renderer: DiagnosticRendererInfo,
    pub recent_events: Vec<LogRecord>,
    pub log_path: String,
    pub report_path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRecord {
    pub schema_version: u32,
    pub timestamp: String,
    pub timestamp_ms: u128,
    pub monotonic_ms: u64,
    pub session_id: String,
    pub level: String,
    pub event: String,
    pub stage: Option<String>,
    pub status: String,
    pub code: String,
    pub duration_ms: Option<u64>,
    pub message: String,
    pub details: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum SessionExitState {
    InProgress,
    CleanExit,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LastSessionMarker {
    schema_version: u32,
    session_id: String,
    started_at: String,
    updated_at: String,
    state: SessionExitState,
    active_report_id: Option<String>,
    last_stage: String,
    last_status: String,
    last_code: String,
    last_message: String,
    log_path: String,
}

pub struct DiagnosticLog {
    path: PathBuf,
    marker_path: PathBuf,
    session_id: String,
    started: Instant,
    write_lock: Mutex<()>,
    marker: Mutex<LastSessionMarker>,
    recent: Mutex<VecDeque<LogRecord>>,
    last_report: Mutex<Option<DiagnosticReport>>,
    previous_report: Mutex<Option<DiagnosticReport>>,
    max_bytes: u64,
    rotated_files: usize,
}

impl DiagnosticLog {
    fn new(path: PathBuf, previous_marker: Option<LastSessionMarker>) -> Self {
        let session_id = Uuid::new_v4().to_string();
        let started_at = now_rfc3339();
        let marker_path = path.with_file_name(MARKER_FILE_NAME);
        let marker = LastSessionMarker {
            schema_version: 1,
            session_id: session_id.clone(),
            started_at: started_at.clone(),
            updated_at: started_at.clone(),
            state: SessionExitState::InProgress,
            active_report_id: None,
            last_stage: "app.runtimeStart".to_string(),
            last_status: "begin".to_string(),
            last_code: "APP_RUNTIME_START".to_string(),
            last_message: "PulseBridge native runtime starting".to_string(),
            log_path: path.to_string_lossy().into_owned(),
        };
        let previous_report = previous_marker
            .filter(|previous| previous.state == SessionExitState::InProgress)
            .map(|previous| reconstruct_previous_report(previous, &path));
        Self {
            path,
            marker_path,
            session_id,
            started: Instant::now(),
            write_lock: Mutex::new(()),
            marker: Mutex::new(marker),
            recent: Mutex::new(VecDeque::with_capacity(RECENT_EVENT_LIMIT)),
            last_report: Mutex::new(None),
            previous_report: Mutex::new(previous_report),
            max_bytes: MAX_LOG_BYTES,
            rotated_files: ROTATED_FILES,
        }
    }

    #[cfg(test)]
    fn with_limits(path: PathBuf, max_bytes: u64, rotated_files: usize) -> Self {
        let mut log = Self::new(path, None);
        log.max_bytes = max_bytes;
        log.rotated_files = rotated_files;
        log
    }

    fn write_record(&self, mut record: LogRecord, critical: bool) {
        record.session_id.clone_from(&self.session_id);
        record.monotonic_ms = duration_ms(self.started.elapsed());
        let _guard = lock_unpoisoned(&self.write_lock);
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if self
            .path
            .metadata()
            .is_ok_and(|metadata| metadata.len() >= self.max_bytes)
        {
            self.rotate();
        }
        if let Ok(payload) = serde_json::to_vec(&record) {
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
            {
                let _ = file.write_all(&payload);
                let _ = file.write_all(b"\n");
                if critical {
                    let _ = file.sync_data();
                }
            }
        }
        let mut recent = lock_unpoisoned(&self.recent);
        if recent.len() == RECENT_EVENT_LIMIT {
            recent.pop_front();
        }
        recent.push_back(record);
    }

    fn rotate(&self) {
        for index in (1..=self.rotated_files).rev() {
            let destination = rotated_path(&self.path, index);
            let source = if index == 1 {
                self.path.clone()
            } else {
                rotated_path(&self.path, index - 1)
            };
            if source.exists() {
                let _ = fs::remove_file(&destination);
                let _ = fs::rename(source, destination);
            }
        }
    }

    fn persist_marker(
        &self,
        stage: &str,
        status: &str,
        code: &str,
        message: &str,
    ) -> Result<(), String> {
        let mut marker = lock_unpoisoned(&self.marker);
        marker.updated_at = now_rfc3339();
        marker.last_stage = stage.to_string();
        marker.last_status = status.to_string();
        marker.last_code = code.to_string();
        marker.last_message = message.to_string();
        atomic_write_json(&self.marker_path, &*marker)
    }
}

fn rotated_path(path: &Path, index: usize) -> PathBuf {
    path.with_file_name(format!("{LOG_FILE_NAME}.{index}"))
}

pub fn initialize(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_log_dir()
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let path = directory.join(LOG_FILE_NAME);
    let marker_path = directory.join(MARKER_FILE_NAME);
    let previous_marker = read_json::<LastSessionMarker>(&marker_path).ok();
    let log = DiagnosticLog::new(path.clone(), previous_marker);
    let previous_report = lock_unpoisoned(&log.previous_report).clone();
    atomic_write_json(&marker_path, &*lock_unpoisoned(&log.marker))?;
    LOG.set(log)
        .map_err(|_| "Diagnostic logging was initialized more than once".to_string())?;
    critical_event(
        "info",
        "application.startup",
        "APP_RUNTIME_START",
        "PulseBridge native runtime starting",
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "target": env!("PULSEBRIDGE_TARGET"),
            "arch": std::env::consts::ARCH,
            "osVersion": os_version(),
        }),
    );
    if let Some(mut report) = previous_report {
        let report_path = directory.join(PREVIOUS_REPORT_FILE_NAME);
        report.report_path = Some(report_path.to_string_lossy().into_owned());
        let _ = atomic_write_json(&report_path, &report);
        *lock_unpoisoned(&global().previous_report) = Some(report.clone());
        critical_event(
            "error",
            "application.previousExit",
            "UNEXPECTED_PROCESS_EXIT",
            &report.summary,
            json!({
                "previousSessionId": report.session_id,
                "reportId": report.report_id,
                "failureStage": report.failure_stage,
            }),
        );
    }
    install_panic_hook();
    Ok(path)
}

pub fn log_path() -> Option<PathBuf> {
    LOG.get().map(|log| log.path.clone())
}

pub fn report_directory() -> Option<PathBuf> {
    log_path().and_then(|path| path.parent().map(ToOwned::to_owned))
}

pub fn session_id() -> Option<String> {
    LOG.get().map(|log| log.session_id.clone())
}

pub fn app_info() -> DiagnosticAppInfo {
    DiagnosticAppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: std::env::consts::OS.to_string(),
        os_version: os_version(),
        arch: std::env::consts::ARCH.to_string(),
        target: env!("PULSEBRIDGE_TARGET").to_string(),
    }
}

pub fn event(level: &str, name: &str, message: &str) {
    event_with_code(level, name, "EVENT", message, Value::Null, false);
}

pub fn critical_event(level: &str, name: &str, code: &str, message: &str, details: Value) {
    event_with_code(level, name, code, message, details, true);
}

pub fn event_with_code(
    level: &str,
    name: &str,
    code: &str,
    message: &str,
    details: Value,
    critical: bool,
) {
    let Some(log) = LOG.get() else {
        return;
    };
    log.write_record(
        LogRecord {
            schema_version: 1,
            timestamp: now_rfc3339(),
            timestamp_ms: unix_timestamp_ms(),
            monotonic_ms: 0,
            session_id: log.session_id.clone(),
            level: level.to_string(),
            event: name.to_string(),
            stage: None,
            status: "event".to_string(),
            code: code.to_string(),
            duration_ms: None,
            message: message.to_string(),
            details,
        },
        critical,
    );
}

pub struct StageGuard {
    stage: String,
    code: String,
    started: Instant,
    finished: bool,
}

impl StageGuard {
    pub fn pass(mut self, code: &str, message: &str, details: Value) {
        self.finish("info", "pass", code, message, details);
    }

    pub fn degraded(mut self, code: &str, message: &str, details: Value) {
        self.finish("warn", "degraded", code, message, details);
    }

    pub fn error(mut self, code: &str, message: &str, details: Value) {
        self.finish("error", "error", code, message, details);
    }

    pub fn cancel(mut self, message: &str) {
        self.finish(
            "warn",
            "cancelled",
            "DIAGNOSTIC_CANCELLED",
            message,
            Value::Null,
        );
    }

    fn finish(&mut self, level: &str, status: &str, code: &str, message: &str, details: Value) {
        if self.finished {
            return;
        }
        self.finished = true;
        write_stage_record(
            level,
            &self.stage,
            status,
            code,
            message,
            Some(duration_ms(self.started.elapsed())),
            details,
        );
    }
}

impl Drop for StageGuard {
    fn drop(&mut self) {
        if !self.finished {
            write_stage_record(
                "error",
                &self.stage,
                "error",
                "STAGE_INTERRUPTED",
                "Stage ended without a terminal result",
                Some(duration_ms(self.started.elapsed())),
                json!({ "beginCode": self.code }),
            );
        }
    }
}

pub fn begin_stage(stage: &str, code: &str, message: &str, details: Value) -> StageGuard {
    write_stage_record("info", stage, "begin", code, message, None, details);
    StageGuard {
        stage: stage.to_string(),
        code: code.to_string(),
        started: Instant::now(),
        finished: false,
    }
}

fn write_stage_record(
    level: &str,
    stage: &str,
    status: &str,
    code: &str,
    message: &str,
    duration_ms: Option<u64>,
    details: Value,
) {
    let Some(log) = LOG.get() else {
        return;
    };
    log.write_record(
        LogRecord {
            schema_version: 1,
            timestamp: now_rfc3339(),
            timestamp_ms: unix_timestamp_ms(),
            monotonic_ms: 0,
            session_id: log.session_id.clone(),
            level: level.to_string(),
            event: format!("stage.{stage}.{status}"),
            stage: Some(stage.to_string()),
            status: status.to_string(),
            code: code.to_string(),
            duration_ms,
            message: message.to_string(),
            details,
        },
        true,
    );
    let _ = log.persist_marker(stage, status, code, message);
}

pub fn set_active_report(report_id: Option<&str>) {
    let Some(log) = LOG.get() else {
        return;
    };
    let mut marker = lock_unpoisoned(&log.marker);
    marker.active_report_id = report_id.map(ToOwned::to_owned);
    if report_id.is_some() {
        marker.state = SessionExitState::InProgress;
    }
    marker.updated_at = now_rfc3339();
    let _ = atomic_write_json(&log.marker_path, &*marker);
}

pub fn mark_in_progress(stage: &str, code: &str, message: &str) {
    let Some(log) = LOG.get() else {
        return;
    };
    let mut marker = lock_unpoisoned(&log.marker);
    marker.state = SessionExitState::InProgress;
    marker.updated_at = now_rfc3339();
    marker.last_stage = stage.to_string();
    marker.last_status = "begin".to_string();
    marker.last_code = code.to_string();
    marker.last_message = message.to_string();
    let _ = atomic_write_json(&log.marker_path, &*marker);
}

pub fn save_report(mut report: DiagnosticReport) -> Result<DiagnosticReport, String> {
    let directory = report_directory()
        .ok_or_else(|| "The diagnostic report directory is unavailable".to_string())?;
    let path = directory.join(format!("diagnostic-{}.json", report.report_id));
    report.report_path = Some(path.to_string_lossy().into_owned());
    atomic_write_json(&path, &report)?;
    atomic_write_json(&directory.join(LATEST_REPORT_FILE_NAME), &report)?;
    *lock_unpoisoned(&global().last_report) = Some(report.clone());
    set_active_report(None);
    critical_event(
        "info",
        "diagnostic.report.saved",
        "DIAGNOSTIC_REPORT_SAVED",
        "Connection diagnostic report saved",
        json!({ "reportId": report.report_id, "verdict": report.verdict }),
    );
    Ok(report)
}

pub fn latest_report() -> Option<DiagnosticReport> {
    let log = LOG.get()?;
    lock_unpoisoned(&log.last_report)
        .clone()
        .or_else(|| lock_unpoisoned(&log.previous_report).clone())
}

pub fn previous_report() -> Option<DiagnosticReport> {
    LOG.get()
        .and_then(|log| lock_unpoisoned(&log.previous_report).clone())
}

pub fn recent_events() -> Vec<LogRecord> {
    LOG.get()
        .map(|log| lock_unpoisoned(&log.recent).iter().cloned().collect())
        .unwrap_or_default()
}

pub fn mark_clean_exit(message: &str) {
    let Some(log) = LOG.get() else {
        return;
    };
    critical_event(
        "info",
        "application.cleanExit",
        "CLEAN_EXIT",
        message,
        Value::Null,
    );
    let mut marker = lock_unpoisoned(&log.marker);
    marker.updated_at = now_rfc3339();
    marker.state = SessionExitState::CleanExit;
    marker.last_stage = "application.cleanExit".to_string();
    marker.last_status = "pass".to_string();
    marker.last_code = "CLEAN_EXIT".to_string();
    marker.last_message = message.to_string();
    let _ = atomic_write_json(&log.marker_path, &*marker);
}

fn reconstruct_previous_report(
    marker: LastSessionMarker,
    current_log_path: &Path,
) -> DiagnosticReport {
    let prior_log = PathBuf::from(&marker.log_path);
    let recent_events = load_recent_records(&prior_log);
    let stage = DiagnosticStageResult {
        stage: marker.last_stage.clone(),
        status: DiagnosticStageStatus::Fail,
        duration_ms: 0,
        code: "UNEXPECTED_PROCESS_EXIT".to_string(),
        message: format!(
            "The previous run ended while '{}' was {}.",
            marker.last_stage, marker.last_status
        ),
        details: json!({
            "lastCode": marker.last_code,
            "lastMessage": marker.last_message,
            "previousLogPath": marker.log_path,
        }),
    };
    DiagnosticReport {
        schema_version: 1,
        report_id: marker
            .active_report_id
            .unwrap_or_else(|| Uuid::new_v4().to_string()),
        session_id: Some(marker.session_id),
        mode: DiagnosticMode::FullStartup,
        started_at: marker.started_at,
        duration_ms: 0,
        app: app_info(),
        verdict: DiagnosticVerdict::Fail,
        failure_stage: Some(marker.last_stage),
        failure_code: Some("UNEXPECTED_PROCESS_EXIT".to_string()),
        summary:
            "Previous run ended unexpectedly. The last durable startup stage is included below."
                .to_string(),
        stages: vec![stage],
        audio: DiagnosticAudioInfo::default(),
        renderer: DiagnosticRendererInfo::default(),
        recent_events,
        log_path: if prior_log.as_os_str().is_empty() {
            current_log_path.to_string_lossy().into_owned()
        } else {
            prior_log.to_string_lossy().into_owned()
        },
        report_path: None,
    }
}

fn load_recent_records(path: &Path) -> Vec<LogRecord> {
    fs::read_to_string(path)
        .ok()
        .map(|contents| {
            let mut records = contents
                .lines()
                .rev()
                .take(64)
                .filter_map(|line| serde_json::from_str::<LogRecord>(line).ok())
                .collect::<Vec<_>>();
            records.reverse();
            records
        })
        .unwrap_or_default()
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}

fn atomic_write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Diagnostic path has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("pulsebridge"),
        Uuid::new_v4()
    ));
    let payload = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    let result = (|| {
        let mut file = File::create(&temp).map_err(|error| error.to_string())?;
        file.write_all(&payload)
            .map_err(|error| error.to_string())?;
        file.write_all(b"\n").map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        drop(file);
        replace_file(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(not(target_os = "windows"))]
fn replace_file(source: &Path, destination: &Path) -> Result<(), String> {
    fs::rename(source, destination).map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
fn replace_file(source: &Path, destination: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        core::PCWSTR,
        Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MOVE_FILE_FLAGS,
        },
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVE_FILE_FLAGS(MOVEFILE_REPLACE_EXISTING.0 | MOVEFILE_WRITE_THROUGH.0),
        )
        .map_err(|error| error.to_string())
    }
}

fn global() -> &'static DiagnosticLog {
    LOG.get().expect("diagnostic logging should be initialized")
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn unix_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn duration_ms(duration: std::time::Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn os_version() -> String {
    #[cfg(target_os = "macos")]
    let output = Command::new("sw_vers").args(["-productVersion"]).output();
    #[cfg(target_os = "windows")]
    let output = Command::new("cmd").args(["/C", "ver"]).output();
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let output = Command::new("uname").args(["-sr"]).output();

    output
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|version| version.trim().to_string())
        .filter(|version| !version.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|location| format!("{}:{}", location.file(), location.line()))
            .unwrap_or_else(|| "unknown location".to_string());
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("non-string panic");
        critical_event(
            "error",
            "process.panic",
            "PROCESS_PANIC",
            &format!("{payload} at {location}"),
            json!({ "location": location }),
        );
        previous(info);
    }));
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_rotation_stays_bounded() {
        let directory =
            std::env::temp_dir().join(format!("pulsebridge-log-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let path = directory.join(LOG_FILE_NAME);
        let log = DiagnosticLog::with_limits(path.clone(), 80, 2);
        for index in 0..12 {
            log.write_record(
                LogRecord {
                    schema_version: 1,
                    timestamp: now_rfc3339(),
                    timestamp_ms: unix_timestamp_ms(),
                    monotonic_ms: 0,
                    session_id: String::new(),
                    level: "info".to_string(),
                    event: "test".to_string(),
                    stage: None,
                    status: "event".to_string(),
                    code: "TEST".to_string(),
                    duration_ms: None,
                    message: format!("bounded record {index}"),
                    details: Value::Null,
                },
                false,
            );
        }
        assert!(path.exists());
        assert!(rotated_path(&path, 1).exists());
        assert!(!rotated_path(&path, 3).exists());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn diagnostic_schema_serializes_and_accepts_missing_optional_v1_fields() {
        let report = DiagnosticReport {
            schema_version: 1,
            report_id: "report".to_string(),
            verdict: DiagnosticVerdict::Pass,
            ..Default::default()
        };
        let encoded = serde_json::to_string(&report).expect("serialize report");
        assert!(encoded.contains("schemaVersion"));
        let parsed: DiagnosticReport =
            serde_json::from_str(r#"{"schemaVersion":1,"reportId":"old","verdict":"fail"}"#)
                .expect("backward-compatible parse");
        assert_eq!(parsed.report_id, "old");
        assert_eq!(parsed.verdict, DiagnosticVerdict::Fail);
    }

    #[test]
    fn unfinished_marker_reconstructs_an_unexpected_exit_report() {
        let marker = LastSessionMarker {
            schema_version: 1,
            session_id: "session".to_string(),
            started_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:01Z".to_string(),
            state: SessionExitState::InProgress,
            active_report_id: Some("report".to_string()),
            last_stage: "renderer.device".to_string(),
            last_status: "begin".to_string(),
            last_code: "GPU_DEVICE_CREATE_BEGIN".to_string(),
            last_message: "Creating device".to_string(),
            log_path: "missing.log".to_string(),
        };
        let report = reconstruct_previous_report(marker, Path::new("pulsebridge.log"));
        assert_eq!(
            report.failure_code.as_deref(),
            Some("UNEXPECTED_PROCESS_EXIT")
        );
        assert_eq!(report.failure_stage.as_deref(), Some("renderer.device"));
        assert_eq!(report.report_id, "report");
    }

    #[test]
    fn stage_records_are_ordered_and_terminal_records_include_duration() {
        if LOG.get().is_none() {
            let directory =
                std::env::temp_dir().join(format!("pulsebridge-stage-test-{}", std::process::id()));
            let _ = fs::create_dir_all(&directory);
            let _ = LOG.set(DiagnosticLog::with_limits(
                directory.join(LOG_FILE_NAME),
                MAX_LOG_BYTES,
                ROTATED_FILES,
            ));
        }

        let stage = format!("test.stage.{}", Uuid::new_v4());
        begin_stage(&stage, "TEST_BEGIN", "Starting test stage", Value::Null).pass(
            "TEST_PASS",
            "Finished test stage",
            Value::Null,
        );
        let records = recent_events()
            .into_iter()
            .filter(|record| record.stage.as_deref() == Some(stage.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].status, "begin");
        assert_eq!(records[0].code, "TEST_BEGIN");
        assert!(records[0].duration_ms.is_none());
        assert_eq!(records[1].status, "pass");
        assert_eq!(records[1].code, "TEST_PASS");
        assert!(records[1].duration_ms.is_some());
        assert!(records[1].monotonic_ms >= records[0].monotonic_ms);
    }
}
