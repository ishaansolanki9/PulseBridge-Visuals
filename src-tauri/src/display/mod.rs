use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc, Arc, Mutex, MutexGuard, RwLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow, Window, WindowEvent,
};

use crate::{
    analysis::{spawn_analysis, AnalysisSnapshot, SharedAnalysis},
    audio::{spawn_audio_capture, CaptureState, CaptureStatus, PcmRingBuffer, SampleFlowState},
    diagnostics,
    phrase::{PhraseStatus, PlaybackContext, SharedPhrase},
    resilience::PerformancePowerGuard,
    visuals::{
        prepare_renderer_surface, run_renderer, RendererLifecycle, RendererStatus, VisualSettings,
    },
};

const RENDERER_START_TIMEOUT: Duration = Duration::from_secs(12);
const WORKER_JOIN_TIMEOUT: Duration = Duration::from_secs(2);

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeLifecycle {
    #[default]
    Stopped,
    Starting,
    Running,
    Recovering,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LifecycleEvent {
    Start,
    Ready,
    Recover,
    Recovered,
    Fail,
    Stop,
}

fn transition_lifecycle(current: RuntimeLifecycle, event: LifecycleEvent) -> RuntimeLifecycle {
    match event {
        LifecycleEvent::Start => RuntimeLifecycle::Starting,
        LifecycleEvent::Ready if current == RuntimeLifecycle::Starting => RuntimeLifecycle::Running,
        LifecycleEvent::Recover if current == RuntimeLifecycle::Running => {
            RuntimeLifecycle::Recovering
        }
        LifecycleEvent::Recovered if current == RuntimeLifecycle::Recovering => {
            RuntimeLifecycle::Running
        }
        LifecycleEvent::Fail => RuntimeLifecycle::Failed,
        LifecycleEvent::Stop => RuntimeLifecycle::Stopped,
        _ => current,
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSnapshot {
    pub lifecycle: RuntimeLifecycle,
    pub running: bool,
    pub last_error: Option<String>,
    pub log_path: Option<String>,
    pub settings: VisualSettings,
    pub audio: CaptureStatus,
    pub phrase: PhraseStatus,
    pub renderer: RendererStatus,
    pub output_mode: OutputMode,
    pub reactive: bool,
    pub audio_age_ms: Option<u64>,
}

struct PerformanceSession {
    window: Window,
    controller: Option<WebviewWindow>,
    stop_requested: Arc<AtomicBool>,
    allow_close: Arc<AtomicBool>,
    workers: Vec<(&'static str, JoinHandle<()>)>,
}

pub struct PerformanceManager {
    operation: Mutex<()>,
    session: Mutex<Option<PerformanceSession>>,
    lifecycle: Arc<Mutex<RuntimeLifecycle>>,
    settings: Arc<RwLock<VisualSettings>>,
    last_error: Arc<Mutex<Option<String>>>,
    capture_status: Arc<Mutex<CaptureStatus>>,
    renderer_status: Arc<Mutex<RendererStatus>>,
    analysis: SharedAnalysis,
    phrase: SharedPhrase,
    output_mode: Arc<AtomicU8>,
    log_path: Option<PathBuf>,
}

impl PerformanceManager {
    pub fn new(settings: VisualSettings, log_path: Option<PathBuf>) -> Self {
        Self {
            operation: Mutex::new(()),
            session: Mutex::new(None),
            lifecycle: Arc::new(Mutex::new(RuntimeLifecycle::Stopped)),
            settings: Arc::new(RwLock::new(settings.sanitized())),
            last_error: Arc::new(Mutex::new(None)),
            capture_status: Arc::new(Mutex::new(CaptureStatus::default())),
            renderer_status: Arc::new(Mutex::new(RendererStatus::default())),
            analysis: Arc::new(RwLock::new(AnalysisSnapshot::default())),
            phrase: Arc::new(RwLock::new(PlaybackContext::default())),
            output_mode: Arc::new(AtomicU8::new(OutputMode::Reactive.value())),
            log_path,
        }
    }

    pub fn snapshot(&self) -> RuntimeSnapshot {
        self.reap_finished_session();
        let now = Instant::now();
        let analysis = self
            .analysis
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let visual_input = analysis.visual_input(now);
        let audio = lock_unpoisoned(&self.capture_status).clone();
        match (audio.state, audio.sample_flow) {
            (CaptureState::Recovering, _) | (_, SampleFlowState::Silent) => {
                self.apply_lifecycle(LifecycleEvent::Recover);
            }
            (CaptureState::Listening, SampleFlowState::Flowing) => {
                self.apply_lifecycle(LifecycleEvent::Recovered);
            }
            _ => {}
        }
        let stored_lifecycle = *lock_unpoisoned(&self.lifecycle);
        let lifecycle = match (stored_lifecycle, audio.state, audio.sample_flow) {
            (RuntimeLifecycle::Running, CaptureState::Recovering, _)
            | (RuntimeLifecycle::Running, _, SampleFlowState::Silent) => {
                RuntimeLifecycle::Recovering
            }
            (RuntimeLifecycle::Recovering, CaptureState::Listening, SampleFlowState::Flowing) => {
                RuntimeLifecycle::Running
            }
            _ => stored_lifecycle,
        };
        let session_present = lock_unpoisoned(&self.session).is_some();
        let phrase_status = self
            .phrase
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .status(now);
        RuntimeSnapshot {
            lifecycle,
            running: session_present
                && matches!(
                    lifecycle,
                    RuntimeLifecycle::Running | RuntimeLifecycle::Recovering
                ),
            last_error: lock_unpoisoned(&self.last_error).clone(),
            log_path: self
                .log_path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            settings: self.settings(),
            audio,
            phrase: phrase_status,
            renderer: lock_unpoisoned(&self.renderer_status).clone(),
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
        diagnostics::event("info", "output.mode", &format!("mode={mode:?}"));
    }

    pub fn start(&self, app: &AppHandle) -> Result<(), String> {
        let _operation = lock_unpoisoned(&self.operation);
        self.stop_inner()?;
        self.apply_lifecycle(LifecycleEvent::Start);
        *lock_unpoisoned(&self.last_error) = None;
        *lock_unpoisoned(&self.capture_status) = CaptureStatus::default();
        *lock_unpoisoned(&self.renderer_status) = RendererStatus {
            state: RendererLifecycle::Initializing,
            message: Some("Preparing performance output".to_string()),
            ..Default::default()
        };
        *self
            .analysis
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = AnalysisSnapshot::default();
        *self
            .phrase
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = PlaybackContext::default();
        self.output_mode
            .store(OutputMode::Reactive.value(), Ordering::Release);
        diagnostics::event(
            "info",
            "performance.start.begin",
            "Starting performance output",
        );
        diagnostics::mark_in_progress(
            "performance.start",
            "PERFORMANCE_START_BEGIN",
            "Starting performance output",
        );

        let result = self.start_transaction(app);
        if let Err(error) = &result {
            self.record_failure("performance.start.failed", error);
            diagnostics::mark_clean_exit("Recoverable performance startup failure was cleaned up");
        }
        result
    }

    fn start_transaction(&self, app: &AppHandle) -> Result<(), String> {
        let display_stage = diagnostics::begin_stage(
            "display.enumeration",
            "DISPLAY_ENUMERATION_BEGIN",
            "Enumerating output displays",
            serde_json::Value::Null,
        );
        let monitors = match app.available_monitors() {
            Ok(monitors) if !monitors.is_empty() => {
                display_stage.pass(
                    "DISPLAYS_ENUMERATED",
                    "Output displays enumerated",
                    serde_json::json!({ "displayCount": monitors.len() }),
                );
                monitors
            }
            Ok(_) => {
                display_stage.error(
                    "NO_DISPLAYS_FOUND",
                    "No connected displays were found",
                    serde_json::Value::Null,
                );
                return Err("No connected displays were found".to_string());
            }
            Err(error) => {
                let message = format!("Unable to enumerate displays: {error}");
                display_stage.error(
                    "DISPLAY_ENUMERATION_FAILED",
                    &message,
                    serde_json::Value::Null,
                );
                return Err(message);
            }
        };
        if monitors.is_empty() {
            return Err("No connected displays were found".to_string());
        }
        let settings = self.settings();
        let (selected_index, monitor) = monitors
            .get(settings.display_id)
            .map(|monitor| (settings.display_id, monitor))
            .or_else(|| monitors.first().map(|monitor| (0, monitor)))
            .ok_or_else(|| "The selected display is no longer connected".to_string())?;
        let position = *monitor.position();
        let size = *monitor.size();
        diagnostics::event(
            "info",
            "display.selected",
            &format!(
                "index={selected_index} size={}x{} scale={}",
                size.width,
                size.height,
                monitor.scale_factor()
            ),
        );

        let window_stage = diagnostics::begin_stage(
            "display.windowCreate",
            "DISPLAY_WINDOW_CREATE_BEGIN",
            "Creating hidden performance window",
            serde_json::json!({ "displayIndex": selected_index }),
        );
        let window = match tauri::window::WindowBuilder::new(app, "performance")
            .title("PulseBridge Visuals")
            .decorations(false)
            .resizable(false)
            .minimizable(false)
            .maximizable(false)
            .closable(true)
            .always_on_top(settings.topmost)
            .skip_taskbar(true)
            .visible(false)
            .inner_size(800.0, 450.0)
            .build()
        {
            Ok(window) => {
                window_stage.pass(
                    "DISPLAY_WINDOW_CREATED",
                    "Hidden performance window created",
                    serde_json::Value::Null,
                );
                window
            }
            Err(error) => {
                let message = format!("Performance window creation failed: {error}");
                window_stage.error(
                    "DISPLAY_WINDOW_CREATE_FAILED",
                    &message,
                    serde_json::Value::Null,
                );
                return Err(message);
            }
        };

        let configure_stage = diagnostics::begin_stage(
            "display.windowConfigure",
            "DISPLAY_WINDOW_CONFIGURE_BEGIN",
            "Positioning and sizing the hidden performance window",
            serde_json::json!({
                "x": position.x,
                "y": position.y,
                "width": size.width,
                "height": size.height,
            }),
        );
        if let Err(error) = prepare_hidden_window(&window, position, size) {
            configure_stage.error(
                "DISPLAY_WINDOW_CONFIGURE_FAILED",
                &error,
                serde_json::Value::Null,
            );
            let _ = window.close();
            return Err(error);
        }
        configure_stage.pass(
            "DISPLAY_WINDOW_CONFIGURED",
            "Hidden performance window configured",
            serde_json::Value::Null,
        );

        let prepared_surface = match prepare_renderer_surface(&window) {
            Ok(surface) => surface,
            Err(error) => {
                let _ = window.close();
                return Err(error);
            }
        };

        let stop_requested = Arc::new(AtomicBool::new(false));
        let allow_close = Arc::new(AtomicBool::new(false));
        install_output_close_handler(&window, &stop_requested, &allow_close);
        let ring = Arc::new(PcmRingBuffer::new(settings.pcm_buffer_seconds));
        let mut workers = Vec::with_capacity(3);
        let analysis_worker = match spawn_analysis(
            Arc::clone(&ring),
            Arc::clone(&stop_requested),
            Arc::clone(&self.analysis),
            Arc::clone(&self.phrase),
        ) {
            Ok(worker) => worker,
            Err(error) => {
                rollback_startup(window, stop_requested, allow_close, workers);
                return Err(format!("Analysis worker failed to start: {error}"));
            }
        };
        workers.push(("analysis", analysis_worker));

        let audio_worker = match spawn_audio_capture(
            settings.audio_source_id.clone(),
            ring,
            Arc::clone(&stop_requested),
            Arc::clone(&self.capture_status),
        ) {
            Ok(worker) => worker,
            Err(error) => {
                rollback_startup(window, stop_requested, allow_close, workers);
                return Err(format!("Audio worker failed to start: {error}"));
            }
        };
        workers.push(("audio", audio_worker));

        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let worker_stop = Arc::clone(&stop_requested);
        let worker_settings = Arc::clone(&self.settings);
        let worker_analysis = Arc::clone(&self.analysis);
        let worker_phrase = Arc::clone(&self.phrase);
        let worker_output_mode = Arc::clone(&self.output_mode);
        let worker_error = Arc::clone(&self.last_error);
        let worker_lifecycle = Arc::clone(&self.lifecycle);
        let worker_renderer_status = Arc::clone(&self.renderer_status);
        let render_window = window.clone();
        let panic_sender = ready_sender.clone();
        let renderer_worker = match thread::Builder::new()
            .name("pulsebridge-visual-renderer".to_string())
            .spawn(move || {
                let _power_guard = PerformancePowerGuard::acquire();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_renderer(
                        render_window,
                        prepared_surface,
                        Arc::clone(&worker_stop),
                        worker_settings,
                        worker_analysis,
                        worker_phrase,
                        worker_output_mode,
                        ready_sender,
                        Arc::clone(&worker_renderer_status),
                    )
                }));
                match result {
                    Ok(Ok(())) if worker_stop.load(Ordering::Acquire) => {}
                    Ok(Ok(())) => {
                        let error = "Renderer exited unexpectedly".to_string();
                        *lock_unpoisoned(&worker_error) = Some(error.clone());
                        set_arc_lifecycle(&worker_lifecycle, LifecycleEvent::Fail);
                        diagnostics::event("error", "worker.renderer.exit", &error);
                    }
                    Ok(Err(error)) => {
                        *lock_unpoisoned(&worker_error) = Some(error.clone());
                        set_arc_lifecycle(&worker_lifecycle, LifecycleEvent::Fail);
                        diagnostics::event("error", "worker.renderer.failed", &error);
                    }
                    Err(_) => {
                        let error =
                            "Renderer worker panicked during GPU setup or drawing".to_string();
                        let _ = panic_sender.try_send(Err(error.clone()));
                        *lock_unpoisoned(&worker_error) = Some(error.clone());
                        *lock_unpoisoned(&worker_renderer_status) = RendererStatus {
                            state: RendererLifecycle::Failed,
                            message: Some(error.clone()),
                            ..Default::default()
                        };
                        set_arc_lifecycle(&worker_lifecycle, LifecycleEvent::Fail);
                        diagnostics::event("error", "worker.renderer.panic", &error);
                    }
                }
                worker_stop.store(true, Ordering::Release);
                diagnostics::event("info", "worker.renderer.cleanup", "Renderer worker exited");
            }) {
            Ok(worker) => worker,
            Err(error) => {
                rollback_startup(window, stop_requested, allow_close, workers);
                return Err(format!("Renderer worker failed to start: {error}"));
            }
        };
        workers.push(("renderer", renderer_worker));

        let readiness = ready_receiver.recv_timeout(RENDERER_START_TIMEOUT);
        let info = match readiness {
            Ok(Ok(info)) => info,
            Ok(Err(error)) => {
                rollback_startup(window, stop_requested, allow_close, workers);
                return Err(format!("Performance renderer failed: {error}"));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                rollback_startup(window, stop_requested, allow_close, workers);
                return Err(format!(
                    "Performance renderer did not produce a frame within {} seconds",
                    RENDERER_START_TIMEOUT.as_secs()
                ));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                rollback_startup(window, stop_requested, allow_close, workers);
                return Err("Renderer worker exited before startup completed".to_string());
            }
        };

        if stop_requested.load(Ordering::Acquire) {
            rollback_startup(window, stop_requested, allow_close, workers);
            return Err("Renderer stopped during startup".to_string());
        }
        let reveal_stage = diagnostics::begin_stage(
            "display.fullscreenReveal",
            "FULLSCREEN_REVEAL_BEGIN",
            "Revealing the performance output after GPU readiness",
            serde_json::Value::Null,
        );
        if let Err(error) = reveal_window(&window) {
            reveal_stage.error("FULLSCREEN_REVEAL_FAILED", &error, serde_json::Value::Null);
            rollback_startup(window, stop_requested, allow_close, workers);
            return Err(error);
        }
        reveal_stage.pass(
            "FULLSCREEN_REVEALED",
            "Performance output revealed and focused",
            serde_json::Value::Null,
        );
        diagnostics::event(
            "info",
            "performance.window.visible",
            &format!(
                "First frame ready on adapter={} backend={} software_fallback={}",
                info.adapter, info.backend, info.software_fallback
            ),
        );
        *lock_unpoisoned(&self.session) = Some(PerformanceSession {
            window,
            controller: app.get_webview_window("main"),
            stop_requested,
            allow_close,
            workers,
        });
        self.apply_lifecycle(LifecycleEvent::Ready);
        diagnostics::event(
            "info",
            "performance.start.complete",
            "Performance output is running",
        );
        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        let _operation = lock_unpoisoned(&self.operation);
        self.stop_inner()
    }

    fn stop_inner(&self) -> Result<(), String> {
        let session = lock_unpoisoned(&self.session).take();
        if let Some(session) = session {
            diagnostics::event(
                "info",
                "performance.stop.begin",
                "Stopping performance output",
            );
            cleanup_session(session);
            diagnostics::event(
                "info",
                "performance.stop.complete",
                "Performance output stopped",
            );
            diagnostics::mark_clean_exit("Performance output stopped and cleanup completed");
        }
        *lock_unpoisoned(&self.capture_status) = CaptureStatus::default();
        *lock_unpoisoned(&self.renderer_status) = RendererStatus::default();
        self.output_mode
            .store(OutputMode::Reactive.value(), Ordering::Release);
        self.apply_lifecycle(LifecycleEvent::Stop);
        Ok(())
    }

    fn reap_finished_session(&self) {
        let should_reap = lock_unpoisoned(&self.session)
            .as_ref()
            .is_some_and(|session| {
                should_reap_session(
                    session.stop_requested.load(Ordering::Acquire),
                    session
                        .workers
                        .iter()
                        .any(|(_, worker)| worker.is_finished()),
                )
            });
        if !should_reap {
            return;
        }
        let Some(session) = lock_unpoisoned(&self.session).take() else {
            return;
        };
        diagnostics::event(
            "warn",
            "performance.session.stale",
            "Cleaning a performance session after an unexpected worker exit",
        );
        cleanup_session(session);
        if *lock_unpoisoned(&self.lifecycle) != RuntimeLifecycle::Failed {
            self.record_failure(
                "performance.session.failed",
                "A performance worker exited unexpectedly; the session was cleaned up",
            );
        }
    }

    fn apply_lifecycle(&self, event: LifecycleEvent) {
        set_arc_lifecycle(&self.lifecycle, event);
    }

    fn record_failure(&self, event: &str, error: &str) {
        *lock_unpoisoned(&self.last_error) = Some(error.to_string());
        self.apply_lifecycle(LifecycleEvent::Fail);
        diagnostics::event("error", event, error);
    }
}

impl Drop for PerformanceManager {
    fn drop(&mut self) {
        if let Ok(session) = self.session.get_mut() {
            if let Some(session) = session.take() {
                cleanup_session(session);
            }
        }
    }
}

fn prepare_hidden_window(
    window: &Window,
    position: tauri::PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
) -> Result<(), String> {
    window
        .set_position(PhysicalPosition::new(position.x, position.y))
        .map_err(|error| format!("Unable to position performance window: {error}"))?;
    window
        .set_size(PhysicalSize::new(size.width, size.height))
        .map_err(|error| format!("Unable to size performance window: {error}"))?;
    window
        .set_fullscreen(true)
        .map_err(|error| format!("Unable to make performance window fullscreen: {error}"))
}

fn reveal_window(window: &Window) -> Result<(), String> {
    window
        .set_cursor_visible(false)
        .map_err(|error| format!("Unable to hide the performance cursor: {error}"))?;
    window
        .show()
        .map_err(|error| format!("Unable to show the performance window: {error}"))?;
    window
        .set_focus()
        .map_err(|error| format!("Unable to focus the performance window: {error}"))
}

fn rollback_startup(
    window: Window,
    stop_requested: Arc<AtomicBool>,
    allow_close: Arc<AtomicBool>,
    workers: Vec<(&'static str, JoinHandle<()>)>,
) {
    diagnostics::event(
        "warn",
        "performance.start.rollback",
        "Rolling back incomplete performance startup",
    );
    cleanup_session(PerformanceSession {
        window,
        controller: None,
        stop_requested,
        allow_close,
        workers,
    });
}

fn cleanup_session(mut session: PerformanceSession) {
    stop_and_join_workers(&session.stop_requested, &mut session.workers);
    let _ = session.window.set_cursor_visible(true);
    let _ = session.window.set_always_on_top(false);
    let _ = session.window.set_fullscreen(false);
    session.allow_close.store(true, Ordering::Release);
    let close_stage = diagnostics::begin_stage(
        "display.windowClose",
        "DISPLAY_WINDOW_CLOSE_BEGIN",
        "Closing only the performance output window",
        serde_json::Value::Null,
    );
    match session.window.close() {
        Ok(()) => close_stage.pass(
            "DISPLAY_WINDOW_CLOSED",
            "Performance output window closed",
            serde_json::Value::Null,
        ),
        Err(error) => {
            close_stage.error(
                "DISPLAY_WINDOW_CLOSE_FAILED",
                &error.to_string(),
                serde_json::Value::Null,
            );
        }
    }
    if let Some(controller) = session.controller {
        let _ = controller.show();
        let _ = controller.set_focus();
    }
    diagnostics::critical_event(
        "info",
        "performance.cleanup.complete",
        "PERFORMANCE_CLEANUP_COMPLETE",
        "Cursor, fullscreen, topmost state, workers, and controller focus were restored",
        serde_json::Value::Null,
    );
}

fn stop_and_join_workers(
    stop_requested: &AtomicBool,
    workers: &mut Vec<(&'static str, JoinHandle<()>)>,
) {
    stop_requested.store(true, Ordering::Release);
    for (name, worker) in workers.drain(..) {
        let join_stage = diagnostics::begin_stage(
            "worker.stopJoin",
            "WORKER_STOP_JOIN_BEGIN",
            &format!("Stopping and joining the {name} worker"),
            serde_json::json!({ "worker": name }),
        );
        let (sender, receiver) = mpsc::sync_channel(1);
        let joiner = thread::Builder::new()
            .name(format!("pulsebridge-{name}-join"))
            .spawn(move || {
                let _ = sender.send(worker.join().is_ok());
            });
        match joiner {
            Ok(_) => match receiver.recv_timeout(WORKER_JOIN_TIMEOUT) {
                Ok(true) => join_stage.pass(
                    "WORKER_STOPPED",
                    &format!("The {name} worker stopped"),
                    serde_json::json!({ "worker": name }),
                ),
                Ok(false) => join_stage.error(
                    "WORKER_JOIN_PANIC",
                    &format!("The {name} worker terminated with a panic"),
                    serde_json::json!({ "worker": name }),
                ),
                Err(_) => join_stage.error(
                    "WORKER_STOP_TIMEOUT",
                    &format!("The {name} worker did not stop within two seconds"),
                    serde_json::json!({ "worker": name }),
                ),
            },
            Err(error) => join_stage.error(
                "WORKER_JOIN_SUPERVISOR_FAILED",
                &format!("Unable to supervise the {name} worker join: {error}"),
                serde_json::json!({ "worker": name }),
            ),
        }
    }
}

fn install_output_close_handler(
    window: &Window,
    stop_requested: &Arc<AtomicBool>,
    allow_close: &Arc<AtomicBool>,
) {
    let stop = Arc::clone(stop_requested);
    let close_allowed = Arc::clone(allow_close);
    window.on_window_event(move |event| match event {
        WindowEvent::CloseRequested { api, .. } if !close_allowed.load(Ordering::Acquire) => {
            api.prevent_close();
            request_session_stop(&stop, "OS close request");
        }
        WindowEvent::Destroyed => request_session_stop(&stop, "performance window destroyed"),
        _ => {}
    });
}

fn request_session_stop(stop_requested: &AtomicBool, reason: &str) {
    if !stop_requested.swap(true, Ordering::AcqRel) {
        diagnostics::critical_event(
            "info",
            "performance.stop.requested",
            "PERFORMANCE_STOP_REQUESTED",
            &format!("Stopping performance output: {reason}"),
            serde_json::json!({ "reason": reason }),
        );
    }
}

fn should_reap_session(stop_requested: bool, worker_finished: bool) -> bool {
    stop_requested || worker_finished
}

fn set_arc_lifecycle(lifecycle: &Mutex<RuntimeLifecycle>, event: LifecycleEvent) {
    let mut current = lock_unpoisoned(lifecycle);
    *current = transition_lifecycle(*current, event);
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn enumerate_displays(app: &AppHandle) -> Result<Vec<DisplayInfo>, String> {
    let monitors = app.available_monitors().map_err(|error| {
        diagnostics::event("error", "display.enumerate.failed", &error.to_string());
        error.to_string()
    })?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_transitions_are_explicit_and_recoverable() {
        let mut state = transition_lifecycle(RuntimeLifecycle::Stopped, LifecycleEvent::Start);
        assert_eq!(state, RuntimeLifecycle::Starting);
        state = transition_lifecycle(state, LifecycleEvent::Ready);
        assert_eq!(state, RuntimeLifecycle::Running);
        state = transition_lifecycle(state, LifecycleEvent::Recover);
        assert_eq!(state, RuntimeLifecycle::Recovering);
        state = transition_lifecycle(state, LifecycleEvent::Recovered);
        assert_eq!(state, RuntimeLifecycle::Running);
        state = transition_lifecycle(state, LifecycleEvent::Fail);
        assert_eq!(state, RuntimeLifecycle::Failed);
        state = transition_lifecycle(state, LifecycleEvent::Stop);
        assert_eq!(state, RuntimeLifecycle::Stopped);
    }

    #[test]
    fn unrelated_events_do_not_claim_readiness() {
        assert_eq!(
            transition_lifecycle(RuntimeLifecycle::Stopped, LifecycleEvent::Ready),
            RuntimeLifecycle::Stopped
        );
        assert_eq!(
            transition_lifecycle(RuntimeLifecycle::Failed, LifecycleEvent::Recovered),
            RuntimeLifecycle::Failed
        );
    }

    #[test]
    fn transactional_worker_rollback_signals_and_joins_everything() {
        let stop = Arc::new(AtomicBool::new(false));
        let exited = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker_exited = Arc::clone(&exited);
        let worker = thread::spawn(move || {
            while !worker_stop.load(Ordering::Acquire) {
                thread::yield_now();
            }
            worker_exited.store(true, Ordering::Release);
        });
        let mut workers = vec![("test", worker)];
        stop_and_join_workers(&stop, &mut workers);
        assert!(stop.load(Ordering::Acquire));
        assert!(exited.load(Ordering::Acquire));
        assert!(workers.is_empty());
    }

    #[test]
    fn stale_session_cleanup_triggers_for_stop_or_worker_exit() {
        assert!(!should_reap_session(false, false));
        assert!(should_reap_session(true, false));
        assert!(should_reap_session(false, true));
    }

    #[test]
    fn native_close_requests_only_stop_the_visual_session() {
        let stop = AtomicBool::new(false);
        request_session_stop(&stop, "test close");
        assert!(stop.load(Ordering::Acquire));
        request_session_stop(&stop, "duplicate close");
        assert!(stop.load(Ordering::Acquire));
    }
}
