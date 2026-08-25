use std::{
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc, Mutex, MutexGuard, RwLock,
    },
    thread::{self, JoinHandle},
    time::Instant,
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, PhysicalPosition, PhysicalSize, Window};

use crate::{
    analysis::{spawn_analysis, AnalysisSnapshot, SharedAnalysis},
    audio::{spawn_audio_capture, CaptureStatus, PcmRingBuffer},
    resilience::PerformancePowerGuard,
    visuals::{run_renderer, VisualSettings},
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    pub id: usize,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    pub is_primary: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutputMode {
    #[default]
    Reactive,
    Ambient,
    Black,
}

impl OutputMode {
    fn value(self) -> u8 {
        match self {
            Self::Reactive => 0,
            Self::Ambient => 1,
            Self::Black => 2,
        }
    }

    fn from_value(value: u8) -> Self {
        match value {
            1 => Self::Ambient,
            2 => Self::Black,
            _ => Self::Reactive,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSnapshot {
    pub running: bool,
    pub last_error: Option<String>,
    pub settings: VisualSettings,
    pub audio: CaptureStatus,
    pub output_mode: OutputMode,
    pub reactive: bool,
    pub audio_age_ms: Option<u64>,
}

struct PerformanceSession {
    window: Window,
    stop_requested: Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
}

pub struct PerformanceManager {
    session: Mutex<Option<PerformanceSession>>,
    settings: Arc<RwLock<VisualSettings>>,
    last_error: Arc<Mutex<Option<String>>>,
    capture_status: Arc<Mutex<CaptureStatus>>,
    analysis: SharedAnalysis,
    output_mode: Arc<AtomicU8>,
}

impl PerformanceManager {
    pub fn new(settings: VisualSettings) -> Self {
        Self {
            session: Mutex::new(None),
            settings: Arc::new(RwLock::new(settings.sanitized())),
            last_error: Arc::new(Mutex::new(None)),
            capture_status: Arc::new(Mutex::new(CaptureStatus::default())),
            analysis: Arc::new(RwLock::new(AnalysisSnapshot::default())),
            output_mode: Arc::new(AtomicU8::new(OutputMode::Reactive.value())),
        }
    }

    pub fn snapshot(&self) -> RuntimeSnapshot {
        let running = lock_unpoisoned(&self.session)
            .as_ref()
            .is_some_and(|session| !session.stop_requested.load(Ordering::Acquire));
        let now = Instant::now();
        let analysis = self
            .analysis
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let visual_input = analysis.visual_input(now);
        RuntimeSnapshot {
            running,
            last_error: lock_unpoisoned(&self.last_error).clone(),
            settings: self.settings(),
            audio: lock_unpoisoned(&self.capture_status).clone(),
            output_mode: OutputMode::from_value(self.output_mode.load(Ordering::Acquire)),
            reactive: visual_input.reactivity > 0.05,
            audio_age_ms: analysis.audio_age_ms(now),
        }
    }

    pub fn settings(&self) -> VisualSettings {
        self.settings
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn update_settings(&self, settings: VisualSettings) {
        let settings = settings.sanitized();
        if let Some(session) = lock_unpoisoned(&self.session).as_ref() {
            let _ = session.window.set_always_on_top(settings.topmost);
        }
        *self
            .settings
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = settings;
    }

    pub fn set_output_mode(&self, mode: OutputMode) {
        self.output_mode.store(mode.value(), Ordering::Release);
    }

    pub fn start(&self, app: &AppHandle) -> Result<(), String> {
        self.stop()?;
        *lock_unpoisoned(&self.last_error) = None;
        *lock_unpoisoned(&self.capture_status) = CaptureStatus::default();
        *self
            .analysis
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = AnalysisSnapshot::default();
        self.output_mode
            .store(OutputMode::Reactive.value(), Ordering::Release);

        let monitors = app
            .available_monitors()
            .map_err(|error| error.to_string())?;
        if monitors.is_empty() {
            return Err("No connected displays were found".to_string());
        }
        let settings = self.settings();
        let monitor = monitors
            .get(settings.display_id)
            .or_else(|| monitors.first())
            .ok_or_else(|| "The selected display is no longer connected".to_string())?;
        let position = *monitor.position();
        let size = *monitor.size();

        let window = tauri::window::WindowBuilder::new(app, "performance")
            .title("PulseBridge Visuals")
            .decorations(false)
            .resizable(false)
            .minimizable(false)
            .maximizable(false)
            .closable(false)
            .always_on_top(settings.topmost)
            .skip_taskbar(true)
            .visible(false)
            .inner_size(800.0, 450.0)
            .build()
            .map_err(|error| error.to_string())?;

        window
            .set_position(PhysicalPosition::new(position.x, position.y))
            .map_err(|error| error.to_string())?;
        window
            .set_size(PhysicalSize::new(size.width, size.height))
            .map_err(|error| error.to_string())?;
        window
            .set_cursor_visible(false)
            .map_err(|error| error.to_string())?;
        window
            .set_fullscreen(true)
            .map_err(|error| error.to_string())?;
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;

        let stop_requested = Arc::new(AtomicBool::new(false));
        let ring = Arc::new(PcmRingBuffer::new(settings.pcm_buffer_seconds));
        let analysis_worker = spawn_analysis(
            Arc::clone(&ring),
            Arc::clone(&stop_requested),
            Arc::clone(&self.analysis),
        )?;
        let audio_worker = match spawn_audio_capture(
            settings.audio_source_id.clone(),
            ring,
            Arc::clone(&stop_requested),
            Arc::clone(&self.capture_status),
        ) {
            Ok(worker) => worker,
            Err(error) => {
                stop_requested.store(true, Ordering::Release);
                let _ = analysis_worker.join();
                let _ = window.close();
                return Err(error);
            }
        };

        let worker_stop = Arc::clone(&stop_requested);
        let worker_settings = Arc::clone(&self.settings);
        let worker_analysis = Arc::clone(&self.analysis);
        let worker_output_mode = Arc::clone(&self.output_mode);
        let worker_error = Arc::clone(&self.last_error);
        let render_window = window.clone();
        let renderer_worker = match thread::Builder::new()
            .name("pulsebridge-visual-renderer".to_string())
            .spawn(move || {
                let _power_guard = PerformancePowerGuard::acquire();
                if let Err(error) = run_renderer(
                    render_window,
                    Arc::clone(&worker_stop),
                    worker_settings,
                    worker_analysis,
                    worker_output_mode,
                ) {
                    *lock_unpoisoned(&worker_error) = Some(error);
                }
                worker_stop.store(true, Ordering::Release);
            }) {
            Ok(worker) => worker,
            Err(error) => {
                stop_requested.store(true, Ordering::Release);
                let _ = audio_worker.join();
                let _ = analysis_worker.join();
                let _ = window.close();
                return Err(error.to_string());
            }
        };

        *lock_unpoisoned(&self.session) = Some(PerformanceSession {
            window,
            stop_requested,
            workers: vec![renderer_worker, audio_worker, analysis_worker],
        });
        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        let Some(mut session) = lock_unpoisoned(&self.session).take() else {
            return Ok(());
        };
        session.stop_requested.store(true, Ordering::Release);
        for worker in session.workers.drain(..) {
            let _ = worker.join();
        }
        let _ = session.window.set_cursor_visible(true);
        let _ = session.window.set_fullscreen(false);
        session.window.close().map_err(|error| error.to_string())?;
        *lock_unpoisoned(&self.capture_status) = CaptureStatus::default();
        self.output_mode
            .store(OutputMode::Reactive.value(), Ordering::Release);
        Ok(())
    }
}

impl Drop for PerformanceManager {
    fn drop(&mut self) {
        if let Ok(session) = self.session.get_mut() {
            if let Some(mut session) = session.take() {
                session.stop_requested.store(true, Ordering::Release);
                for worker in session.workers.drain(..) {
                    let _ = worker.join();
                }
                let _ = session.window.set_cursor_visible(true);
                let _ = session.window.close();
            }
        }
    }
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn enumerate_displays(app: &AppHandle) -> Result<Vec<DisplayInfo>, String> {
    let monitors = app
        .available_monitors()
        .map_err(|error| error.to_string())?;
    let primary = app.primary_monitor().map_err(|error| error.to_string())?;
    Ok(monitors
        .iter()
        .enumerate()
        .map(|(id, monitor)| {
            let is_primary = primary.as_ref().is_some_and(|primary| {
                primary.position() == monitor.position() && primary.size() == monitor.size()
            });
            DisplayInfo {
                id,
                name: monitor
                    .name()
                    .cloned()
                    .unwrap_or_else(|| format!("Display {}", id + 1)),
                width: monitor.size().width,
                height: monitor.size().height,
                scale_factor: monitor.scale_factor(),
                is_primary,
            }
        })
        .collect())
}
