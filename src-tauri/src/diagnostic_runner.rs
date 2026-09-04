use std::{
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc, Arc, Mutex, MutexGuard, RwLock,
    },
    thread,
    time::{Duration, Instant},
};

use chrono::{SecondsFormat, Utc};
use serde_json::{json, Value};
use tauri::AppHandle;
use uuid::Uuid;

use crate::{
    analysis::AnalysisSnapshot,
    audio::{
        enumerate_audio_sources, spawn_audio_capture, CaptureState, CaptureStatus, PcmRingBuffer,
    },
    diagnostics::{
        self, DiagnosticAudioInfo, DiagnosticMode, DiagnosticReport, DiagnosticStageResult,
        DiagnosticStageStatus, DiagnosticVerdict,
    },
    phrase::PlaybackContext,
    visuals::{
        prepare_renderer_surface, probe_renderer, run_renderer, RendererStatus, VisualSettings,
    },
};

const CAPTURE_INITIALIZE_TIMEOUT: Duration = Duration::from_secs(4);
const FIRST_PACKET_TIMEOUT: Duration = Duration::from_secs(8);
const NON_SILENT_TIMEOUT: Duration = Duration::from_secs(5);
const WORKER_JOIN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Default)]
pub struct DiagnosticCoordinator {
    inner: Arc<DiagnosticCoordinatorInner>,
}

#[derive(Default)]
struct DiagnosticCoordinatorInner {
    operation: Mutex<()>,
    active_cancel: Mutex<Option<Arc<AtomicBool>>>,
}

impl DiagnosticCoordinator {
    pub fn is_active(&self) -> bool {
        lock_unpoisoned(&self.inner.active_cancel).is_some()
    }

    pub fn cancel(&self) -> bool {
        let active = lock_unpoisoned(&self.inner.active_cancel);
        if let Some(cancel) = active.as_ref() {
            cancel.store(true, Ordering::Release);
            true
        } else {
            false
        }
    }

    pub fn run(
        &self,
        app: AppHandle,
        mode: DiagnosticMode,
        settings: VisualSettings,
    ) -> Result<DiagnosticReport, String> {
        let _operation = lock_unpoisoned(&self.inner.operation);
        let cancel = Arc::new(AtomicBool::new(false));
        *lock_unpoisoned(&self.inner.active_cancel) = Some(Arc::clone(&cancel));
        let report = run_diagnostic(&app, mode, settings, cancel);
        *lock_unpoisoned(&self.inner.active_cancel) = None;
        report
    }
}

fn run_diagnostic(
    app: &AppHandle,
    mode: DiagnosticMode,
    settings: VisualSettings,
    cancel: Arc<AtomicBool>,
) -> Result<DiagnosticReport, String> {
    let started = Instant::now();
    let report_id = Uuid::new_v4().to_string();
    diagnostics::set_active_report(Some(&report_id));
    diagnostics::critical_event(
        "info",
        "diagnostic.begin",
        "DIAGNOSTIC_BEGIN",
        "Starting bounded connection diagnostic",
        json!({ "reportId": report_id, "mode": mode }),
    );
    let mut report = DiagnosticReport {
        schema_version: 1,
        report_id,
        session_id: diagnostics::session_id(),
        mode,
        started_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        app: diagnostics::app_info(),
        verdict: DiagnosticVerdict::Pass,
        log_path: diagnostics::log_path()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default(),
        ..Default::default()
    };

    if matches!(
        mode,
        DiagnosticMode::AudioOnly | DiagnosticMode::FullStartup
    ) {
        run_audio_probe(&settings, &cancel, &mut report);
    }
    if !cancel.load(Ordering::Acquire)
        && matches!(
            mode,
            DiagnosticMode::RendererOnly
                | DiagnosticMode::FullStartup
                | DiagnosticMode::SafeRenderer
        )
    {
        run_renderer_probe(mode == DiagnosticMode::SafeRenderer, &cancel, &mut report);
    }
    if !cancel.load(Ordering::Acquire) && mode == DiagnosticMode::FullStartup {
        run_hidden_surface_probe(app, &settings, &cancel, &mut report);
    }
    if cancel.load(Ordering::Acquire) {
        report.verdict = DiagnosticVerdict::Cancelled;
        report.summary =
            "The connection diagnostic was cancelled and its workers were stopped.".to_string();
        if report.failure_code.is_none() {
            report.failure_code = Some("DIAGNOSTIC_CANCELLED".to_string());
        }
    }
    if report.summary.is_empty() {
        report.summary = summary_for(&report);
    }
    report.duration_ms = elapsed_ms(started);
    report.recent_events = diagnostics::recent_events();
    let saved = diagnostics::save_report(report);
    diagnostics::mark_clean_exit("Connection diagnostic completed and cleaned up");
    saved
}

fn run_hidden_surface_probe(
    app: &AppHandle,
    settings: &VisualSettings,
    cancel: &Arc<AtomicBool>,
    report: &mut DiagnosticReport,
) {
    let started = Instant::now();
    let guard = diagnostics::begin_stage(
        "renderer.hiddenSurface",
        "GPU_SURFACE_PROBE_BEGIN",
        "Creating a temporary hidden surface and presenting one frame",
        Value::Null,
    );
    let label = format!("diagnostic-{}", Uuid::new_v4().simple());
    let window = match tauri::window::WindowBuilder::new(app, label)
        .title("PulseBridge renderer diagnostic")
        .visible(false)
        .closable(true)
        .decorations(false)
        .inner_size(640.0, 360.0)
        .build()
    {
        Ok(window) => window,
        Err(error) => {
            let message = format!("DISPLAY_WINDOW_CREATE_FAILED: {error}");
            guard.error("DISPLAY_WINDOW_CREATE_FAILED", &message, Value::Null);
            push_stage(
                report,
                "renderer.hiddenSurface",
                DiagnosticStageStatus::Fail,
                started,
                "DISPLAY_WINDOW_CREATE_FAILED",
                message,
                Value::Null,
            );
            mark_failure(
                report,
                "renderer.hiddenSurface",
                "DISPLAY_WINDOW_CREATE_FAILED",
            );
            return;
        }
    };
    let stop = Arc::new(AtomicBool::new(false));
    let prepared_surface = match prepare_renderer_surface(&window) {
        Ok(surface) => surface,
        Err(error) => {
            let _ = window.close();
            let code = "GPU_SURFACE_CREATE_FAILED";
            guard.error(code, &error, Value::Null);
            push_stage(
                report,
                "renderer.hiddenSurface",
                DiagnosticStageStatus::Fail,
                started,
                code,
                error,
                Value::Null,
            );
            mark_failure(report, "renderer.hiddenSurface", code);
            return;
        }
    };
    let renderer_status = Arc::new(Mutex::new(RendererStatus::default()));
    let analysis = Arc::new(RwLock::new(AnalysisSnapshot::default()));
    let phrase = Arc::new(RwLock::new(PlaybackContext::default()));
    let output_mode = Arc::new(AtomicU8::new(1));
    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
    let render_window = window.clone();
    let worker_stop = Arc::clone(&stop);
    let worker = thread::Builder::new()
        .name("pulsebridge-diagnostic-surface".to_string())
        .spawn({
            let renderer_status = Arc::clone(&renderer_status);
            let settings = Arc::new(RwLock::new(settings.clone()));
            move || {
                let _ = run_renderer(
                    render_window,
                    prepared_surface,
                    worker_stop,
                    settings,
                    analysis,
                    phrase,
                    output_mode,
                    ready_sender,
                    renderer_status,
                );
            }
        });
    let worker = match worker {
        Ok(worker) => worker,
        Err(error) => {
            let _ = window.close();
            let message = format!("GPU_SURFACE_WORKER_START_FAILED: {error}");
            guard.error("GPU_SURFACE_CREATE_FAILED", &message, Value::Null);
            push_stage(
                report,
                "renderer.hiddenSurface",
                DiagnosticStageStatus::Fail,
                started,
                "GPU_SURFACE_CREATE_FAILED",
                message,
                Value::Null,
            );
            mark_failure(
                report,
                "renderer.hiddenSurface",
                "GPU_SURFACE_CREATE_FAILED",
            );
            return;
        }
    };
    let deadline = Instant::now() + Duration::from_secs(8);
    let readiness = loop {
        if cancel.load(Ordering::Acquire) {
            break Err("DIAGNOSTIC_CANCELLED: hidden-surface probe cancelled".to_string());
        }
        match ready_receiver.try_recv() {
            Ok(Ok(info)) => break Ok(info),
            Ok(Err(error)) => break Err(error),
            Err(mpsc::TryRecvError::Disconnected) => {
                break Err("GPU_FIRST_FRAME_TIMEOUT: renderer worker exited".to_string());
            }
            Err(mpsc::TryRecvError::Empty) if Instant::now() >= deadline => {
                break Err(
                    "GPU_FIRST_FRAME_TIMEOUT: no frame was presented within eight seconds"
                        .to_string(),
                );
            }
            Err(mpsc::TryRecvError::Empty) => thread::sleep(Duration::from_millis(25)),
        }
    };
    stop.store(true, Ordering::Release);
    let joined = join_worker_bounded(worker, WORKER_JOIN_TIMEOUT);
    let _ = window.set_cursor_visible(true);
    let _ = window.close();

    match readiness {
        Ok(info) if joined => {
            report.renderer.adapter = Some(info.adapter.clone());
            report.renderer.backend = Some(info.backend.clone());
            report.renderer.driver = Some(info.driver.clone());
            report.renderer.driver_info = Some(info.driver_info.clone());
            report.renderer.device_type = Some(info.device_type.clone());
            report.renderer.surface_format = Some(info.surface_format.clone());
            report.renderer.present_mode = Some(info.present_mode.clone());
            report.renderer.software_fallback = info.software_fallback;
            report.renderer.surface_tested = true;
            guard.pass(
                "GPU_FIRST_FRAME_PRESENTED",
                "A temporary hidden surface presented one frame and was cleaned up.",
                json!({
                    "adapter": info.adapter,
                    "backend": info.backend,
                    "driver": info.driver,
                    "driverInfo": info.driver_info,
                    "deviceType": info.device_type,
                    "surfaceFormat": info.surface_format,
                    "presentMode": info.present_mode,
                    "softwareFallback": info.software_fallback,
                }),
            );
            push_stage(
                report,
                "renderer.hiddenSurface",
                DiagnosticStageStatus::Pass,
                started,
                "GPU_FIRST_FRAME_PRESENTED",
                "A temporary hidden surface presented one frame and was cleaned up.",
                Value::Null,
            );
        }
        Ok(_) => {
            let message = "WORKER_STOP_TIMEOUT: hidden renderer did not stop within two seconds";
            guard.error("WORKER_STOP_TIMEOUT", message, Value::Null);
            push_stage(
                report,
                "renderer.hiddenSurface",
                DiagnosticStageStatus::Fail,
                started,
                "WORKER_STOP_TIMEOUT",
                message,
                Value::Null,
            );
            mark_failure(report, "renderer.hiddenSurface", "WORKER_STOP_TIMEOUT");
        }
        Err(error) if cancel.load(Ordering::Acquire) => {
            guard.cancel(&error);
            push_stage(
                report,
                "renderer.hiddenSurface",
                DiagnosticStageStatus::Cancelled,
                started,
                "DIAGNOSTIC_CANCELLED",
                error,
                Value::Null,
            );
        }
        Err(error) => {
            let code = if error.contains("GPU_FIRST_FRAME_TIMEOUT") {
                "GPU_FIRST_FRAME_TIMEOUT"
            } else if error.contains("GPU_SURFACE") {
                "GPU_SURFACE_CREATE_FAILED"
            } else {
                map_renderer_failure(&error)
            };
            guard.error(code, &error, Value::Null);
            push_stage(
                report,
                "renderer.hiddenSurface",
                DiagnosticStageStatus::Fail,
                started,
                code,
                error,
                Value::Null,
            );
            mark_failure(report, "renderer.hiddenSurface", code);
        }
    }
}

fn run_audio_probe(
    settings: &VisualSettings,
    cancel: &Arc<AtomicBool>,
    report: &mut DiagnosticReport,
) {
    let discovery_started = Instant::now();
    let discovery_guard = diagnostics::begin_stage(
        "rekordbox.processDiscovery",
        "REKORDBOX_PROCESS_DISCOVERY_BEGIN",
        "Enumerating supported audio routes and Rekordbox processes",
        Value::Null,
    );
    let sources = match enumerate_audio_sources() {
        Ok(sources) => sources,
        Err(error) => {
            discovery_guard.error("AUDIO_SOURCE_ENUMERATION_FAILED", &error, Value::Null);
            push_stage(
                report,
                "rekordbox.processDiscovery",
                DiagnosticStageStatus::Fail,
                discovery_started,
                "AUDIO_SOURCE_ENUMERATION_FAILED",
                error,
                Value::Null,
            );
            mark_failure(
                report,
                "rekordbox.processDiscovery",
                "AUDIO_SOURCE_ENUMERATION_FAILED",
            );
            return;
        }
    };
    let selected = sources
        .iter()
        .find(|source| source.id == settings.audio_source_id)
        .or_else(|| sources.iter().find(|source| source.is_default))
        .or_else(|| sources.first());
    let process_detected = sources.iter().any(|source| {
        source.kind == crate::audio::AudioSourceKind::RekordboxProcess && source.detected
    });
    let rekordbox_session_detected = sources.iter().any(|source| {
        source.kind == crate::audio::AudioSourceKind::RekordboxSession && source.detected
    });
    report.audio.process_detected = process_detected;
    report.audio.rekordbox_session_detected = rekordbox_session_detected;
    let selected_requires_rekordbox = selected.is_some_and(|source| {
        matches!(
            source.kind,
            crate::audio::AudioSourceKind::RekordboxProcess
                | crate::audio::AudioSourceKind::RekordboxSession
        )
    });
    let (discovery_status, discovery_code, discovery_message) = if process_detected {
        (
            DiagnosticStageStatus::Pass,
            "REKORDBOX_PROCESS_FOUND",
            "Found a supported Rekordbox process.",
        )
    } else if !selected_requires_rekordbox {
        (
            DiagnosticStageStatus::Pass,
            "REKORDBOX_PROCESS_NOT_REQUIRED",
            "The selected all-app or input route does not require Rekordbox process discovery.",
        )
    } else {
        (
            DiagnosticStageStatus::Degraded,
            "REKORDBOX_PROCESS_NOT_FOUND",
            "Rekordbox is not running. Start it before testing the live connection.",
        )
    };
    if process_detected || !selected_requires_rekordbox {
        discovery_guard.pass(
            discovery_code,
            discovery_message,
            json!({ "sourceCount": sources.len() }),
        );
    } else {
        discovery_guard.degraded(
            discovery_code,
            discovery_message,
            json!({ "sourceCount": sources.len() }),
        );
        report.verdict = DiagnosticVerdict::Degraded;
    }
    push_stage(
        report,
        "rekordbox.processDiscovery",
        discovery_status,
        discovery_started,
        discovery_code,
        discovery_message,
        json!({ "sourceCount": sources.len() }),
    );

    #[cfg(target_os = "windows")]
    {
        let session_started = Instant::now();
        let session_guard = diagnostics::begin_stage(
            "rekordbox.audioSessionDiscovery",
            "REKORDBOX_AUDIO_SESSION_DISCOVERY_BEGIN",
            "Finding the Windows output endpoint used by Rekordbox",
            Value::Null,
        );
        let (session_status, session_code, session_message) = if rekordbox_session_detected {
            (
                DiagnosticStageStatus::Pass,
                "REKORDBOX_AUDIO_SESSION_FOUND",
                "Found the Windows output endpoint carrying Rekordbox audio.",
            )
        } else if !selected_requires_rekordbox {
            (
                DiagnosticStageStatus::Pass,
                "REKORDBOX_AUDIO_SESSION_NOT_REQUIRED",
                "The selected Windows route can be tested without a Rekordbox audio session.",
            )
        } else if process_detected {
            (
                DiagnosticStageStatus::Degraded,
                "REKORDBOX_AUDIO_SESSION_NOT_FOUND",
                "Rekordbox is running but has no capturable Windows audio session. Enable PC MASTER OUT in Performance mode and play a track.",
            )
        } else {
            (
                DiagnosticStageStatus::Degraded,
                "REKORDBOX_AUDIO_SESSION_NOT_FOUND",
                "No Rekordbox audio session can be discovered until Rekordbox is running.",
            )
        };
        if rekordbox_session_detected || !selected_requires_rekordbox {
            session_guard.pass(session_code, session_message, Value::Null);
        } else {
            session_guard.degraded(session_code, session_message, Value::Null);
            report.verdict = DiagnosticVerdict::Degraded;
        }
        push_stage(
            report,
            "rekordbox.audioSessionDiscovery",
            session_status,
            session_started,
            session_code,
            session_message,
            Value::Null,
        );
    }
    let Some(selected) = selected else {
        mark_failure(report, "audio.captureInitialization", "NO_AUDIO_SOURCE");
        return;
    };
    if !selected.available {
        let (code, message) = if selected_requires_rekordbox && !process_detected {
            (
                "REKORDBOX_PROCESS_NOT_FOUND",
                "Start Rekordbox before selecting its audio-session route.",
            )
        } else {
            (
                "AUDIO_PLATFORM_UNSUPPORTED",
                "The selected live-audio route is unavailable on this OS version.",
            )
        };
        push_stage(
            report,
            "audio.captureInitialization",
            DiagnosticStageStatus::Fail,
            Instant::now(),
            code,
            message,
            json!({ "source": selected.name }),
        );
        mark_failure(report, "audio.captureInitialization", code);
        return;
    }

    let ring = Arc::new(PcmRingBuffer::new(settings.pcm_buffer_seconds));
    let stop = Arc::new(AtomicBool::new(false));
    let status = Arc::new(Mutex::new(CaptureStatus::default()));
    let init_started = Instant::now();
    let init_guard = diagnostics::begin_stage(
        "audio.captureInitialization",
        "AUDIO_CAPTURE_INITIALIZATION_BEGIN",
        "Initializing the selected audio route",
        json!({ "sourceId": selected.id, "sourceName": selected.name }),
    );
    let worker = match spawn_audio_capture(
        selected.id.clone(),
        ring,
        Arc::clone(&stop),
        Arc::clone(&status),
    ) {
        Ok(worker) => worker,
        Err(error) => {
            init_guard.error(
                "AUDIO_WORKER_START_FAILED",
                &error,
                json!({ "sourceId": selected.id }),
            );
            push_stage(
                report,
                "audio.captureInitialization",
                DiagnosticStageStatus::Fail,
                init_started,
                "AUDIO_WORKER_START_FAILED",
                error,
                Value::Null,
            );
            mark_failure(
                report,
                "audio.captureInitialization",
                "AUDIO_WORKER_START_FAILED",
            );
            return;
        }
    };

    let initialized = wait_for_status(&status, cancel, CAPTURE_INITIALIZE_TIMEOUT, |current| {
        current.capture_initialized
            || matches!(
                current.state,
                CaptureState::Failed | CaptureState::Unsupported
            )
    });
    let init_snapshot = lock_unpoisoned(&status).clone();
    if initialized && init_snapshot.capture_initialized {
        init_guard.pass(
            "AUDIO_CAPTURE_INITIALIZED",
            "The selected capture client initialized.",
            status_details(&init_snapshot),
        );
        push_stage(
            report,
            "audio.captureInitialization",
            DiagnosticStageStatus::Pass,
            init_started,
            "AUDIO_CAPTURE_INITIALIZED",
            "The selected capture client initialized.",
            status_details(&init_snapshot),
        );
    } else if cancel.load(Ordering::Acquire) {
        init_guard.cancel("Audio initialization was cancelled");
        push_stage(
            report,
            "audio.captureInitialization",
            DiagnosticStageStatus::Cancelled,
            init_started,
            "DIAGNOSTIC_CANCELLED",
            "Audio initialization was cancelled.",
            status_details(&init_snapshot),
        );
    } else {
        let (code, message) = map_audio_failure(&init_snapshot, "AUDIO_CLIENT_START_FAILED");
        init_guard.error(&code, &message, status_details(&init_snapshot));
        push_stage(
            report,
            "audio.captureInitialization",
            DiagnosticStageStatus::Fail,
            init_started,
            &code,
            message,
            status_details(&init_snapshot),
        );
        mark_failure(report, "audio.captureInitialization", &code);
    }

    if init_snapshot.capture_initialized && !cancel.load(Ordering::Acquire) {
        run_sample_flow_probe(&status, cancel, report);
    }
    stop.store(true, Ordering::Release);
    let joined = join_worker_bounded(worker, WORKER_JOIN_TIMEOUT);
    if !joined {
        diagnostics::critical_event(
            "error",
            "diagnostic.audio.joinTimeout",
            "AUDIO_WORKER_STOP_TIMEOUT",
            "The diagnostic audio worker did not acknowledge stop within two seconds",
            Value::Null,
        );
        if report.failure_code.is_none() {
            mark_failure(report, "audio.cleanup", "AUDIO_WORKER_STOP_TIMEOUT");
        }
    }
    let final_status = lock_unpoisoned(&status).clone();
    report.audio = audio_info(&final_status, process_detected);
}

fn run_sample_flow_probe(
    status: &Arc<Mutex<CaptureStatus>>,
    cancel: &Arc<AtomicBool>,
    report: &mut DiagnosticReport,
) {
    let packet_started = Instant::now();
    let packet_guard = diagnostics::begin_stage(
        "audio.firstPacket",
        "FIRST_AUDIO_PACKET_WAIT_BEGIN",
        "Waiting for the first audio packet",
        Value::Null,
    );
    let packets = wait_for_status(status, cancel, FIRST_PACKET_TIMEOUT, |current| {
        current.packets_received || current.state == CaptureState::Failed
    });
    let packet_status = lock_unpoisoned(status).clone();
    if packets && packet_status.packets_received {
        packet_guard.pass(
            "AUDIO_PACKETS_RECEIVED",
            "Audio packets are flowing.",
            status_details(&packet_status),
        );
        push_stage(
            report,
            "audio.firstPacket",
            DiagnosticStageStatus::Pass,
            packet_started,
            "AUDIO_PACKETS_RECEIVED",
            "Audio packets are flowing.",
            status_details(&packet_status),
        );
    } else if cancel.load(Ordering::Acquire) {
        packet_guard.cancel("Waiting for audio packets was cancelled");
        return;
    } else {
        let code = if packet_status.rekordbox_detected {
            "ASIO_BYPASS_SUSPECTED"
        } else {
            "NO_AUDIO_PACKETS"
        };
        let message = packet_status.message.clone().unwrap_or_else(|| {
            "Capture initialized, but no packets arrived before the timeout.".to_string()
        });
        packet_guard.error(code, &message, status_details(&packet_status));
        push_stage(
            report,
            "audio.firstPacket",
            DiagnosticStageStatus::Fail,
            packet_started,
            code,
            message,
            status_details(&packet_status),
        );
        mark_failure(report, "audio.firstPacket", code);
        return;
    }

    let signal_started = Instant::now();
    let signal_guard = diagnostics::begin_stage(
        "audio.nonSilentSamples",
        "NON_SILENT_SAMPLE_WAIT_BEGIN",
        "Waiting for non-silent samples",
        Value::Null,
    );
    let signal = wait_for_status(status, cancel, NON_SILENT_TIMEOUT, |current| {
        current.non_silent_samples_received || current.state == CaptureState::Failed
    });
    let signal_status = lock_unpoisoned(status).clone();
    if signal && signal_status.non_silent_samples_received {
        signal_guard.pass(
            "REACTIVE_READY",
            "Recent non-silent audio is feeding analysis.",
            status_details(&signal_status),
        );
        push_stage(
            report,
            "audio.nonSilentSamples",
            DiagnosticStageStatus::Pass,
            signal_started,
            "REACTIVE_READY",
            "Recent non-silent audio is feeding analysis.",
            status_details(&signal_status),
        );
    } else if cancel.load(Ordering::Acquire) {
        signal_guard.cancel("Waiting for non-silent samples was cancelled");
    } else {
        let code = "NO_NON_SILENT_SAMPLES";
        let message = "Packets arrived, but the route remained silent. Start playback and verify the selected output; with ASIO, enable PC MASTER OUT or select a capturable output.";
        signal_guard.degraded(code, message, status_details(&signal_status));
        push_stage(
            report,
            "audio.nonSilentSamples",
            DiagnosticStageStatus::Degraded,
            signal_started,
            code,
            message,
            status_details(&signal_status),
        );
        if report.failure_code.is_none() {
            report.verdict = DiagnosticVerdict::Degraded;
            report.failure_stage = Some("audio.nonSilentSamples".to_string());
            report.failure_code = Some(code.to_string());
        }
    }
}

fn run_renderer_probe(safe_mode: bool, cancel: &AtomicBool, report: &mut DiagnosticReport) {
    let started = Instant::now();
    let guard = diagnostics::begin_stage(
        "renderer.headlessValidation",
        "GPU_VALIDATION_BEGIN",
        "Validating GPU adapter, device, shader, and pipeline without fullscreen",
        json!({ "safeMode": safe_mode }),
    );
    let (sender, receiver) = mpsc::sync_channel(1);
    if let Err(error) = thread::Builder::new()
        .name("pulsebridge-diagnostic-gpu".to_string())
        .spawn(move || {
            let _ = sender.send(probe_renderer(safe_mode));
        })
    {
        let message = format!("GPU_DIAGNOSTIC_WORKER_START_FAILED: {error}");
        guard.error("GPU_DIAGNOSTIC_FAILED", &message, Value::Null);
        push_stage(
            report,
            "renderer.headlessValidation",
            DiagnosticStageStatus::Fail,
            started,
            "GPU_DIAGNOSTIC_FAILED",
            message,
            Value::Null,
        );
        mark_failure(
            report,
            "renderer.headlessValidation",
            "GPU_DIAGNOSTIC_FAILED",
        );
        return;
    }
    let deadline = Instant::now() + Duration::from_secs(8);
    let result = loop {
        if cancel.load(Ordering::Acquire) {
            break Err("DIAGNOSTIC_CANCELLED: renderer validation cancelled".to_string());
        }
        match receiver.try_recv() {
            Ok(result) => break result,
            Err(mpsc::TryRecvError::Disconnected) => {
                break Err("GPU_DIAGNOSTIC_FAILED: renderer probe exited".to_string());
            }
            Err(mpsc::TryRecvError::Empty) if Instant::now() >= deadline => {
                break Err(
                    "GPU_FIRST_FRAME_TIMEOUT: renderer validation exceeded eight seconds"
                        .to_string(),
                );
            }
            Err(mpsc::TryRecvError::Empty) => thread::sleep(Duration::from_millis(25)),
        }
    };
    match result {
        Ok(renderer) => {
            let details = serde_json::to_value(&renderer).unwrap_or(Value::Null);
            guard.pass(
                "GPU_PIPELINE_VALIDATED",
                "The GPU shader and pipeline validated without entering fullscreen.",
                details.clone(),
            );
            push_stage(
                report,
                "renderer.headlessValidation",
                DiagnosticStageStatus::Pass,
                started,
                "GPU_PIPELINE_VALIDATED",
                "The GPU shader and pipeline validated without entering fullscreen.",
                details,
            );
            report.renderer = renderer;
        }
        Err(error) if cancel.load(Ordering::Acquire) => {
            guard.cancel(&error);
            push_stage(
                report,
                "renderer.headlessValidation",
                DiagnosticStageStatus::Cancelled,
                started,
                "DIAGNOSTIC_CANCELLED",
                error,
                Value::Null,
            );
        }
        Err(error) => {
            let code = map_renderer_failure(&error);
            guard.error(code, &error, Value::Null);
            push_stage(
                report,
                "renderer.headlessValidation",
                DiagnosticStageStatus::Fail,
                started,
                code,
                error,
                Value::Null,
            );
            mark_failure(report, "renderer.headlessValidation", code);
        }
    }
}

fn wait_for_status(
    status: &Mutex<CaptureStatus>,
    cancel: &AtomicBool,
    timeout: Duration,
    predicate: impl Fn(&CaptureStatus) -> bool,
) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if cancel.load(Ordering::Acquire) {
            return false;
        }
        if predicate(&lock_unpoisoned(status)) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn join_worker_bounded(worker: thread::JoinHandle<()>, timeout: Duration) -> bool {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let _ = thread::Builder::new()
        .name("pulsebridge-diagnostic-join".to_string())
        .spawn(move || {
            let result = worker.join().is_ok();
            let _ = sender.send(result);
        });
    receiver.recv_timeout(timeout).unwrap_or(false)
}

fn status_details(status: &CaptureStatus) -> Value {
    serde_json::to_value(status).unwrap_or(Value::Null)
}

fn audio_info(status: &CaptureStatus, process_detected: bool) -> DiagnosticAudioInfo {
    DiagnosticAudioInfo {
        process_detected,
        rekordbox_session_detected: status.rekordbox_session_detected,
        capture_initialized: status.capture_initialized,
        packets_received: status.packets_received,
        non_silent_samples_received: status.non_silent_samples_received,
        reactive_ready: status.reactive_ready,
        route: serde_json::to_value(status.route)
            .ok()
            .and_then(|value| value.as_str().map(ToOwned::to_owned)),
        sample_rate: status.sample_rate,
        channels: status.channels,
        format: status.format.clone(),
        captured_frames: status.captured_frames,
        rms: status.rms,
        peak: status.peak,
    }
}

fn map_audio_failure(status: &CaptureStatus, fallback: &str) -> (String, String) {
    let message = status
        .message
        .clone()
        .unwrap_or_else(|| "Audio capture did not initialize before the timeout.".to_string());
    let stable = [
        "WINDOWS_BUILD_UNSUPPORTED",
        "REKORDBOX_PROCESS_NOT_FOUND",
        "REKORDBOX_AUDIO_SESSION_NOT_FOUND",
        "REKORDBOX_AUDIO_SESSION_CHANGED",
        "REKORDBOX_SESSION_CAPTURE_FAILED",
        "REKORDBOX_ONLY_CAPTURE_FAILED",
        "PROCESS_LOOPBACK_ACTIVATION_FAILED",
        "OUTPUT_LOOPBACK_ACTIVATION_FAILED",
        "UNSUPPORTED_AUDIO_FORMAT",
        "CORE_AUDIO_PROCESS_TAP_CREATE_FAILED",
        "CORE_AUDIO_UNSUPPORTED_FORMAT",
    ]
    .into_iter()
    .find(|code| message.contains(code))
    .unwrap_or(fallback);
    (stable.to_string(), message)
}

fn map_renderer_failure(error: &str) -> &'static str {
    [
        "GPU_ADAPTER_NOT_FOUND",
        "GPU_DEVICE_CREATE_FAILED",
        "GPU_SHADER_VALIDATION_FAILED",
        "GPU_PIPELINE_CREATE_FAILED",
        "GPU_DEVICE_LOST",
    ]
    .into_iter()
    .find(|code| error.contains(code))
    .unwrap_or("GPU_DIAGNOSTIC_FAILED")
}

fn push_stage(
    report: &mut DiagnosticReport,
    stage: &str,
    status: DiagnosticStageStatus,
    started: Instant,
    code: &str,
    message: impl Into<String>,
    details: Value,
) {
    report.stages.push(DiagnosticStageResult {
        stage: stage.to_string(),
        status,
        duration_ms: elapsed_ms(started),
        code: code.to_string(),
        message: message.into(),
        details,
    });
}

fn mark_failure(report: &mut DiagnosticReport, stage: &str, code: &str) {
    if report.verdict != DiagnosticVerdict::Cancelled {
        report.verdict = DiagnosticVerdict::Fail;
    }
    if report.failure_code.is_none() {
        report.failure_stage = Some(stage.to_string());
        report.failure_code = Some(code.to_string());
    }
}

fn summary_for(report: &DiagnosticReport) -> String {
    match report.verdict {
        DiagnosticVerdict::Pass => match report.mode {
            DiagnosticMode::AudioOnly => {
                "Audio capture initialized and non-silent samples reached analysis.".to_string()
            }
            DiagnosticMode::RendererOnly | DiagnosticMode::SafeRenderer => {
                "The GPU adapter, device, shader, and pipeline validated without entering fullscreen."
                    .to_string()
            }
            DiagnosticMode::FullStartup => {
                "Audio sample flow and headless GPU pipeline validation both passed.".to_string()
            }
        },
        DiagnosticVerdict::Degraded => {
            "The diagnostic completed with a usable route, but audio is not yet reactive. Review the staged result."
                .to_string()
        }
        DiagnosticVerdict::Fail => format!(
            "The diagnostic stopped at {} ({}).",
            report.failure_stage.as_deref().unwrap_or("unknown stage"),
            report.failure_code.as_deref().unwrap_or("UNKNOWN_FAILURE")
        ),
        DiagnosticVerdict::Cancelled => {
            "The diagnostic was cancelled and cleaned up.".to_string()
        }
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
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
    fn process_detection_and_sample_flow_remain_separate_facts() {
        let status = CaptureStatus {
            rekordbox_detected: true,
            rekordbox_session_detected: true,
            capture_initialized: true,
            packets_received: false,
            non_silent_samples_received: false,
            reactive_ready: false,
            ..Default::default()
        };
        let info = audio_info(&status, true);
        assert!(info.process_detected);
        assert!(info.rekordbox_session_detected);
        assert!(info.capture_initialized);
        assert!(!info.packets_received);
        assert!(!info.reactive_ready);
    }

    #[test]
    fn representative_failures_keep_stable_codes() {
        assert_eq!(
            map_renderer_failure("GPU_SHADER_VALIDATION_FAILED: invalid binding"),
            "GPU_SHADER_VALIDATION_FAILED"
        );
        let status = CaptureStatus {
            message: Some("WINDOWS_BUILD_UNSUPPORTED: build 19041".to_string()),
            ..Default::default()
        };
        assert_eq!(
            map_audio_failure(&status, "AUDIO_CLIENT_START_FAILED").0,
            "WINDOWS_BUILD_UNSUPPORTED"
        );
    }

    #[test]
    fn cancelled_wait_returns_quickly() {
        let status = Mutex::new(CaptureStatus::default());
        let cancel = AtomicBool::new(true);
        let started = Instant::now();
        assert!(!wait_for_status(
            &status,
            &cancel,
            Duration::from_secs(5),
            |_| false
        ));
        assert!(started.elapsed() < Duration::from_millis(50));
    }
}
