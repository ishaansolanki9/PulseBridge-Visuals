use std::{
    collections::HashSet,
    mem::size_of,
    slice,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use windows::{
    core::{Interface, GUID, HRESULT, HSTRING},
    Win32::{
        Foundation::{CloseHandle, PROPERTYKEY},
        Media::{
            Audio::{
                eMultimedia, eRender, IAudioCaptureClient, IAudioClient, IAudioSessionControl2,
                IAudioSessionManager2, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
                AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
                DEVICE_STATE_ACTIVE, WAVEFORMATEX, WAVEFORMATEXTENSIBLE, WAVE_FORMAT_PCM,
            },
            KernelStreaming::{KSDATAFORMAT_SUBTYPE_PCM, WAVE_FORMAT_EXTENSIBLE},
            Multimedia::{KSDATAFORMAT_SUBTYPE_IEEE_FLOAT, WAVE_FORMAT_IEEE_FLOAT},
        },
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
                StructuredStorage::{PropVariantClear, PropVariantToString},
                CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ,
            },
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            SystemInformation::{GetVersionExW, OSVERSIONINFOW},
        },
    },
};

use super::{
    convert_interleaved_to_mono_into, AudioFormat, AudioSourceInfo, AudioSourceKind, CaptureRoute,
    CaptureState, CaptureStatus, PcmRingBuffer, PlatformCaptureBackend, ReconnectPolicy,
    ReleaseOnce, RoutePolicy, SampleFlowState, SampleFormat, StreamingResampler,
    CAPTURE_SAMPLE_RATE,
};

const DEVICE_PREFIX: &str = "device:";
const PROCESS_PREFIX: &str = "process:";
const REKORDBOX_SESSION_SOURCE_ID: &str = "rekordbox:auto";
const AUTOMATIC_OUTPUT_SOURCE_ID: &str = "output:auto";
const FRIENDLY_NAME_KEY: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
    pid: 14,
};

#[derive(Debug)]
struct OutputEndpoint {
    id: String,
    name: String,
    is_default: bool,
    has_rekordbox_session: bool,
}

#[derive(Clone, Debug)]
struct ProcessEntry {
    pid: u32,
    parent_pid: u32,
    executable: String,
}

pub struct Backend;

impl PlatformCaptureBackend for Backend {
    fn enumerate_sources() -> Result<Vec<AudioSourceInfo>, String> {
        enumerate_sources()
    }

    fn spawn_capture(
        source_id: String,
        ring: Arc<PcmRingBuffer>,
        stop: Arc<AtomicBool>,
        status: Arc<Mutex<CaptureStatus>>,
    ) -> Result<JoinHandle<()>, String> {
        spawn_capture(source_id, ring, stop, status)
    }
}

pub fn enumerate_sources() -> Result<Vec<AudioSourceInfo>, String> {
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).is_ok() };
    let result = enumerate_sources_inner().map_err(|error| error.to_string());
    if initialized {
        unsafe { CoUninitialize() };
    }
    result
}

fn enumerate_sources_inner() -> windows::core::Result<Vec<AudioSourceInfo>> {
    let process_ids = rekordbox_process_ids();
    let detected = !process_ids.is_empty();
    let endpoints = active_output_endpoints_for(&process_ids)?;
    let rekordbox_endpoint = endpoints
        .iter()
        .find(|endpoint| endpoint.has_rekordbox_session);
    let mut sources = vec![
        AudioSourceInfo {
            id: format!("{PROCESS_PREFIX}auto"),
            name: if detected {
                "Rekordbox detected (process capture disabled for stability)".to_string()
            } else {
                "Rekordbox not running (process capture disabled)".to_string()
            },
            kind: AudioSourceKind::RekordboxProcess,
            detected,
            is_default: false,
            available: false,
        },
        AudioSourceInfo {
            id: REKORDBOX_SESSION_SOURCE_ID.to_string(),
            name: match (detected, rekordbox_endpoint) {
                (true, Some(endpoint)) => {
                    format!("Rekordbox audio on {} (recommended)", endpoint.name)
                }
                (true, None) => "Rekordbox detected · waiting for PC MASTER OUT".to_string(),
                (false, _) => "Rekordbox audio · start Rekordbox first".to_string(),
            },
            kind: AudioSourceKind::RekordboxSession,
            detected: rekordbox_endpoint.is_some(),
            is_default: detected,
            available: detected,
        },
        AudioSourceInfo {
            id: AUTOMATIC_OUTPUT_SOURCE_ID.to_string(),
            name: if let Some(endpoint) = rekordbox_endpoint {
                format!("Automatic output · Rekordbox found on {}", endpoint.name)
            } else {
                "Automatic Windows output (all apps)".to_string()
            },
            kind: AudioSourceKind::OutputDevice,
            detected: true,
            is_default: !detected,
            available: true,
        },
    ];

    for endpoint in endpoints {
        sources.push(AudioSourceInfo {
            id: format!("{DEVICE_PREFIX}{}", endpoint.id),
            name: if endpoint.has_rekordbox_session {
                format!("{} · Rekordbox audio session", endpoint.name)
            } else {
                endpoint.name
            },
            kind: AudioSourceKind::OutputDevice,
            detected: true,
            is_default: endpoint.is_default,
            available: true,
        });
    }
    Ok(sources)
}

pub fn spawn_capture(
    source_id: String,
    ring: Arc<PcmRingBuffer>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<CaptureStatus>>,
) -> Result<JoinHandle<()>, String> {
    thread::Builder::new()
        .name("pulsebridge-wasapi-capture".to_string())
        .spawn(move || {
            let worker_stop = Arc::clone(&stop);
            let worker_status = Arc::clone(&status);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_capture(source_id, ring, stop, status)
            }));
            if result.is_err() {
                crate::diagnostics::event(
                    "error",
                    "worker.audio.panic",
                    "The WASAPI capture worker panicked",
                );
                set_status(
                    &worker_status,
                    CaptureState::Failed,
                    SampleFlowState::Unavailable,
                    None,
                    None,
                    Some("Audio capture worker failed unexpectedly".to_string()),
                );
                worker_stop.store(true, Ordering::Release);
            }
            crate::diagnostics::event("info", "worker.audio.exit", "Audio worker exited");
        })
        .map_err(|error| error.to_string())
}

fn run_capture(
    source_id: String,
    ring: Arc<PcmRingBuffer>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<CaptureStatus>>,
) {
    crate::diagnostics::event(
        "info",
        "audio.worker.start",
        &format!(
            "platform=windows os_build={} arch={}",
            windows_version(),
            std::env::consts::ARCH
        ),
    );
    if let Err(error) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() } {
        let message = format!("COM initialization failed: {error}");
        crate::diagnostics::event("error", "audio.com.failed", &message);
        set_status(
            &status,
            CaptureState::Failed,
            SampleFlowState::Unavailable,
            None,
            None,
            Some(message),
        );
        return;
    }

    while !stop.load(Ordering::Acquire) {
        let detected = !rekordbox_process_ids().is_empty();
        if let Ok(mut current) = status.lock() {
            current.rekordbox_detected = detected;
            current.capture_initialized = false;
            current.reactive_ready = false;
        }
        set_status(
            &status,
            CaptureState::Connecting,
            SampleFlowState::Waiting,
            None,
            None,
            None,
        );
        let capture_result = capture_selected_source(&source_id, &ring, &stop, &status);
        if stop.load(Ordering::Acquire) {
            break;
        }
        let message = capture_result
            .err()
            .unwrap_or_else(|| "The selected audio stream ended".to_string());
        crate::diagnostics::event("warn", "audio.capture.recovering", &message);
        set_status(
            &status,
            CaptureState::Recovering,
            SampleFlowState::Unavailable,
            None,
            None,
            Some(format!("{message}. Reconnecting…")),
        );
        for _ in 0..8 {
            if stop.load(Ordering::Acquire) {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    unsafe { CoUninitialize() };
    set_status(
        &status,
        CaptureState::Stopped,
        SampleFlowState::Unavailable,
        None,
        None,
        None,
    );
}

fn windows_version() -> String {
    let (_, _, build) = windows_version_parts();
    let mut version = OSVERSIONINFOW {
        dwOSVersionInfoSize: size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    if unsafe { RtlGetVersion(&mut version) } >= 0 {
        return format!(
            "{}.{}.{}",
            version.dwMajorVersion, version.dwMinorVersion, build
        );
    }
    let mut version = OSVERSIONINFOW {
        dwOSVersionInfoSize: size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    match unsafe { GetVersionExW(&mut version) } {
        Ok(()) => format!(
            "{}.{}.{}",
            version.dwMajorVersion, version.dwMinorVersion, version.dwBuildNumber
        ),
        Err(error) => format!("unknown({error})"),
    }
}

fn windows_version_parts() -> (u32, u32, u32) {
    let mut version = OSVERSIONINFOW {
        dwOSVersionInfoSize: size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    let detected = unsafe { RtlGetVersion(&mut version) } >= 0
        || unsafe { GetVersionExW(&mut version) }.is_ok();
    if detected {
        (
            version.dwMajorVersion,
            version.dwMinorVersion,
            version.dwBuildNumber,
        )
    } else {
        (0, 0, 0)
    }
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn RtlGetVersion(version: *mut OSVERSIONINFOW) -> i32;
}

fn capture_selected_source(
    source_id: &str,
    ring: &PcmRingBuffer,
    stop: &AtomicBool,
    status: &Mutex<CaptureStatus>,
) -> Result<(), String> {
    if source_id.strip_prefix(PROCESS_PREFIX).is_some() {
        crate::diagnostics::critical_event(
            "warn",
            "audio.process.redirected",
            "PROCESS_LOOPBACK_DISABLED_AFTER_NATIVE_CRASH",
            "A legacy Rekordbox-only selection was redirected to safe audio-session-guided endpoint capture",
            serde_json::Value::Null,
        );
        capture_rekordbox_session_output(ring, stop, status)
    } else if source_id == REKORDBOX_SESSION_SOURCE_ID {
        capture_rekordbox_session_output(ring, stop, status)
    } else if source_id == AUTOMATIC_OUTPUT_SOURCE_ID {
        capture_automatic_output(ring, stop, status)
    } else if let Some(device_id) = source_id.strip_prefix(DEVICE_PREFIX) {
        let client = activate_endpoint_client(Some(device_id))?;
        let name = selected_device_name(device_id).unwrap_or_else(|_| "Windows output".to_string());
        capture_client(
            client,
            &name,
            None,
            CaptureRoute::SelectedOutput,
            ring,
            stop,
            status,
            RoutePolicy::SelectedOutput,
        )
    } else {
        Err("Unknown audio source".to_string())
    }
}

fn capture_rekordbox_session_output(
    ring: &PcmRingBuffer,
    stop: &AtomicBool,
    status: &Mutex<CaptureStatus>,
) -> Result<(), String> {
    set_status(
        status,
        CaptureState::Connecting,
        SampleFlowState::Waiting,
        Some(CaptureRoute::RekordboxSessionOutput),
        Some("Rekordbox audio session".to_string()),
        Some("Finding the Windows output used by Rekordbox…".to_string()),
    );
    let process_ids = rekordbox_process_ids();
    if process_ids.is_empty() {
        if let Ok(mut current) = status.lock() {
            current.rekordbox_detected = false;
            current.rekordbox_session_detected = false;
        }
        return Err(
            "REKORDBOX_PROCESS_NOT_FOUND: start Rekordbox before connecting PulseBridge"
                .to_string(),
        );
    }
    if let Ok(mut current) = status.lock() {
        current.rekordbox_detected = true;
    }

    crate::diagnostics::critical_event(
        "info",
        "audio.rekordboxSession.discovery",
        "REKORDBOX_AUDIO_SESSION_DISCOVERY_BEGIN",
        "Finding the Windows audio endpoint that owns the Rekordbox session",
        serde_json::json!({ "processCount": process_ids.len() }),
    );
    let endpoints = active_output_endpoints_for(&process_ids).map_err(|error| {
        format!("OUTPUT_ENDPOINT_ENUMERATION_FAILED: could not list Windows outputs: {error}")
    })?;
    let matches = endpoints
        .into_iter()
        .filter(|endpoint| endpoint.has_rekordbox_session)
        .collect::<Vec<_>>();
    if matches.is_empty() {
        if let Ok(mut current) = status.lock() {
            current.rekordbox_session_detected = false;
        }
        crate::diagnostics::critical_event(
            "warn",
            "audio.rekordboxSession.missing",
            "REKORDBOX_AUDIO_SESSION_NOT_FOUND",
            "Rekordbox is running but has no capturable Windows audio session",
            serde_json::Value::Null,
        );
        return Err("REKORDBOX_AUDIO_SESSION_NOT_FOUND: Rekordbox is running, but Windows cannot see its audio stream. In Rekordbox Performance mode, enable PC MASTER OUT and play a track; keep the controller ASIO device as the primary output.".to_string());
    }

    if let Ok(mut current) = status.lock() {
        current.rekordbox_session_detected = true;
    }
    let endpoint_count = matches.len();
    let mut failures = Vec::new();
    for (index, endpoint) in matches.into_iter().enumerate() {
        if stop.load(Ordering::Acquire) {
            return Ok(());
        }
        let position = index + 1;
        let message = format!(
            "Connected to Rekordbox audio session on {} ({position}/{endpoint_count})",
            endpoint.name
        );
        set_status(
            status,
            CaptureState::Connecting,
            SampleFlowState::Waiting,
            Some(CaptureRoute::RekordboxSessionOutput),
            Some(endpoint.name.clone()),
            Some(message.clone()),
        );
        crate::diagnostics::critical_event(
            "info",
            "audio.rekordboxSession.found",
            "REKORDBOX_AUDIO_SESSION_FOUND",
            &message,
            serde_json::json!({
                "deviceName": endpoint.name,
                "isDefault": endpoint.is_default,
                "candidateCount": endpoint_count,
            }),
        );
        let client = match activate_endpoint_client(Some(&endpoint.id)) {
            Ok(client) => client,
            Err(error) => {
                failures.push(format!("{}: {error}", endpoint.name));
                continue;
            }
        };
        match capture_client(
            client,
            &endpoint.name,
            Some(&endpoint.id),
            CaptureRoute::RekordboxSessionOutput,
            ring,
            stop,
            status,
            RoutePolicy::RekordboxSession,
        ) {
            Ok(()) => return Ok(()),
            Err(error) if stop.load(Ordering::Acquire) => return Err(error),
            Err(error) => failures.push(format!("{}: {error}", endpoint.name)),
        }
    }
    Err(format!(
        "REKORDBOX_SESSION_CAPTURE_FAILED: Rekordbox audio endpoints were found but did not provide live audio ({})",
        failures.join("; ")
    ))
}

fn capture_automatic_output(
    ring: &PcmRingBuffer,
    stop: &AtomicBool,
    status: &Mutex<CaptureStatus>,
) -> Result<(), String> {
    set_status(
        status,
        CaptureState::Connecting,
        SampleFlowState::Waiting,
        Some(CaptureRoute::AutomaticOutput),
        Some("Finding active Windows output…".to_string()),
        Some("Checking Windows outputs for live audio from any application".to_string()),
    );
    crate::diagnostics::critical_event(
        "info",
        "audio.output.autoActivate",
        "OUTPUT_LOOPBACK_ACTIVATION_BEGIN",
        "Checking active Windows outputs for live system audio",
        serde_json::Value::Null,
    );
    let endpoints = active_output_endpoints().map_err(|error| {
        format!("OUTPUT_ENDPOINT_ENUMERATION_FAILED: could not list Windows outputs: {error}")
    })?;
    if endpoints.is_empty() {
        return Err(
            "OUTPUT_ENDPOINT_ENUMERATION_FAILED: Windows reported no active audio outputs"
                .to_string(),
        );
    }
    let rekordbox_session_detected = endpoints
        .iter()
        .any(|endpoint| endpoint.has_rekordbox_session);
    if let Ok(mut current) = status.lock() {
        current.rekordbox_session_detected = rekordbox_session_detected;
    }

    let endpoint_count = endpoints.len();
    let mut failures = Vec::new();
    for (index, endpoint) in endpoints.into_iter().enumerate() {
        if stop.load(Ordering::Acquire) {
            return Ok(());
        }
        let position = index + 1;
        let message = if endpoint.has_rekordbox_session {
            format!(
                "Rekordbox audio session found; checking {position}/{endpoint_count}: {}",
                endpoint.name
            )
        } else {
            format!(
                "Checking Windows output {position}/{endpoint_count}: {}",
                endpoint.name
            )
        };
        set_status(
            status,
            CaptureState::Connecting,
            SampleFlowState::Waiting,
            Some(CaptureRoute::AutomaticOutput),
            Some(endpoint.name.clone()),
            Some(message.clone()),
        );
        crate::diagnostics::event("info", "audio.output.probe", &message);
        let stage = crate::diagnostics::begin_stage(
            "audio.outputLoopbackActivation",
            "OUTPUT_LOOPBACK_ACTIVATION_BEGIN",
            "Activating a Windows output loopback candidate",
            serde_json::json!({
                "deviceId": endpoint.id,
                "deviceName": endpoint.name,
                "isDefault": endpoint.is_default,
                "rekordboxSession": endpoint.has_rekordbox_session,
                "probeIndex": position,
                "probeCount": endpoint_count,
            }),
        );
        let client = match activate_endpoint_client(Some(&endpoint.id)) {
            Ok(client) => {
                stage.pass(
                    "OUTPUT_LOOPBACK_ACTIVATED",
                    "Windows output loopback client activated",
                    serde_json::json!({
                        "deviceName": endpoint.name,
                        "isDefault": endpoint.is_default,
                    }),
                );
                client
            }
            Err(error) => {
                stage.error(
                    "OUTPUT_LOOPBACK_ACTIVATION_FAILED",
                    &error,
                    serde_json::json!({ "deviceName": endpoint.name }),
                );
                failures.push(format!("{}: {error}", endpoint.name));
                continue;
            }
        };
        match capture_client(
            client,
            &endpoint.name,
            None,
            CaptureRoute::AutomaticOutput,
            ring,
            stop,
            status,
            RoutePolicy::AutomaticOutput,
        ) {
            Ok(()) => return Ok(()),
            Err(error) if stop.load(Ordering::Acquire) => return Err(error),
            Err(error) => {
                crate::diagnostics::event(
                    "warn",
                    "audio.output.probeSilent",
                    &format!("device={} error={error}", endpoint.name),
                );
                failures.push(format!("{}: {error}", endpoint.name));
            }
        }
    }
    Err(format!(
        "NO_ACTIVE_OUTPUT_SIGNAL: checked {endpoint_count} Windows output(s) without finding live audio ({})",
        failures.join("; ")
    ))
}

fn active_output_endpoints() -> windows::core::Result<Vec<OutputEndpoint>> {
    let process_ids = rekordbox_process_ids();
    active_output_endpoints_for(&process_ids)
}

fn active_output_endpoints_for(
    rekordbox_process_ids: &HashSet<u32>,
) -> windows::core::Result<Vec<OutputEndpoint>> {
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
    let default_device = unsafe {
        enumerator
            .GetDefaultAudioEndpoint(eRender, eMultimedia)
            .ok()
    };
    let default_id = default_device
        .as_ref()
        .and_then(|device| device_id(device).ok());
    let devices = unsafe { enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)? };
    let count = unsafe { devices.GetCount()? };
    let mut endpoints = Vec::with_capacity(count as usize);
    for index in 0..count {
        let device = unsafe { devices.Item(index)? };
        let id = device_id(&device)?;
        let name = device_name(&device).unwrap_or_else(|_| format!("Audio output {}", index + 1));
        endpoints.push(OutputEndpoint {
            is_default: default_id.as_ref().is_some_and(|default| default == &id),
            has_rekordbox_session: endpoint_has_process_session(&device, rekordbox_process_ids),
            id,
            name,
        });
    }
    endpoints.sort_by_key(|endpoint| {
        (
            !endpoint.has_rekordbox_session,
            !endpoint.is_default,
            endpoint.name.to_lowercase(),
        )
    });
    Ok(endpoints)
}

fn endpoint_has_process_session(device: &IMMDevice, process_ids: &HashSet<u32>) -> bool {
    if process_ids.is_empty() {
        return false;
    }
    let manager = unsafe { device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) };
    let Ok(manager) = manager else {
        return false;
    };
    let Ok(sessions) = (unsafe { manager.GetSessionEnumerator() }) else {
        return false;
    };
    let Ok(count) = (unsafe { sessions.GetCount() }) else {
        return false;
    };
    for index in 0..count {
        let Ok(control) = (unsafe { sessions.GetSession(index) }) else {
            continue;
        };
        let Ok(control2) = control.cast::<IAudioSessionControl2>() else {
            continue;
        };
        let Ok(process_id) = (unsafe { control2.GetProcessId() }) else {
            continue;
        };
        if process_ids.contains(&process_id) {
            return true;
        }
    }
    false
}

fn activate_endpoint_client(device_id: Option<&str>) -> Result<IAudioClient, String> {
    let enumerator: IMMDeviceEnumerator = unsafe {
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .map_err(|error| error.to_string())?
    };
    let device = match device_id {
        Some(id) => unsafe {
            enumerator
                .GetDevice(&HSTRING::from(id))
                .map_err(|error| error.to_string())?
        },
        None => unsafe {
            enumerator
                .GetDefaultAudioEndpoint(eRender, eMultimedia)
                .map_err(|error| error.to_string())?
        },
    };
    unsafe {
        device
            .Activate::<IAudioClient>(CLSCTX_ALL, None)
            .map_err(|error| error.to_string())
    }
}

#[allow(clippy::too_many_arguments)]
fn capture_client(
    client: IAudioClient,
    source_name: &str,
    rekordbox_endpoint_id: Option<&str>,
    route: CaptureRoute,
    ring: &PcmRingBuffer,
    stop: &AtomicBool,
    status: &Mutex<CaptureStatus>,
    route_policy: RoutePolicy,
) -> Result<(), String> {
    let format_stage = crate::diagnostics::begin_stage(
        "audio.format",
        "AUDIO_FORMAT_QUERY_BEGIN",
        "Querying and validating the WASAPI shared-mode format",
        serde_json::json!({ "route": route }),
    );
    let raw_format = match unsafe { client.GetMixFormat() } {
        Ok(format) => format,
        Err(error) => {
            let message = windows_error("WASAPI_MIX_FORMAT_QUERY_FAILED", &error);
            format_stage.error(
                "UNSUPPORTED_AUDIO_FORMAT",
                &message,
                serde_json::Value::Null,
            );
            return Err(message);
        }
    };
    if raw_format.is_null() {
        return Err("WASAPI returned no shared-mode mix format".to_string());
    }
    let format_result = unsafe { parse_wave_format(raw_format) };
    let format = match format_result {
        Ok(format) => format,
        Err(error) => {
            unsafe { CoTaskMemFree(Some(raw_format.cast())) };
            let message = format!("UNSUPPORTED_AUDIO_FORMAT: {error}");
            format_stage.error(
                "UNSUPPORTED_AUDIO_FORMAT",
                &message,
                serde_json::Value::Null,
            );
            return Err(message);
        }
    };
    format_stage.pass(
        "AUDIO_FORMAT_SUPPORTED",
        "WASAPI format validated",
        serde_json::json!({
            "sampleRate": format.sample_rate,
            "channels": format.channels,
            "format": format.label(),
            "blockAlign": format.block_align,
        }),
    );
    let initialize_stage = crate::diagnostics::begin_stage(
        "audio.clientInitialization",
        "AUDIO_CLIENT_INITIALIZATION_BEGIN",
        "Initializing and starting the WASAPI capture client",
        serde_json::json!({ "route": route }),
    );
    let initialization = unsafe {
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK,
            0,
            0,
            &*raw_format,
            None,
        )
    };
    unsafe { CoTaskMemFree(Some(raw_format.cast())) };
    if let Err(error) = initialization {
        let message = format!(
            "AUDIO_CLIENT_INITIALIZATION_FAILED: {}",
            windows_error("WASAPI_INITIALIZE_FAILED", &error)
        );
        initialize_stage.error(
            "AUDIO_CLIENT_INITIALIZATION_FAILED",
            &message,
            serde_json::Value::Null,
        );
        return Err(message);
    }

    crate::diagnostics::event(
        "info",
        "audio.format.selected",
        &format!(
            "route={route:?} sample_rate={} channels={} format={} block_align={}",
            format.sample_rate,
            format.channels,
            format.label(),
            format.block_align
        ),
    );
    let capture: IAudioCaptureClient =
        unsafe { client.GetService().map_err(|error| error.to_string())? };
    if let Err(error) = unsafe { client.Start() } {
        let message = windows_error("AUDIO_CLIENT_START_FAILED", &error);
        initialize_stage.error(
            "AUDIO_CLIENT_START_FAILED",
            &message,
            serde_json::Value::Null,
        );
        return Err(message);
    }
    initialize_stage.pass(
        "AUDIO_CLIENT_STARTED",
        "WASAPI capture client initialized and started",
        serde_json::json!({ "route": route }),
    );
    if let Ok(mut current) = status.lock() {
        current.state = CaptureState::Connecting;
        current.route = route;
        current.sample_flow = SampleFlowState::Waiting;
        current.source_name = Some(source_name.to_string());
        current.capture_initialized = true;
        current.sample_rate = Some(format.sample_rate);
        current.channels = Some(format.channels);
        current.format = Some(format.label().to_string());
        current.message = Some("Audio client initialized; waiting for samples".to_string());
    }
    crate::diagnostics::event("info", "audio.client.started", &format!("route={route:?}"));

    let started_at = Instant::now();
    let mut last_signal_at = None;
    let mut packets = 0_u32;
    let mut captured_samples = 0_u64;
    let mut mono = Vec::with_capacity(8_192);
    let mut converted = Vec::with_capacity(8_192);
    let mut resampler = StreamingResampler::new(format.sample_rate, CAPTURE_SAMPLE_RATE)?;
    let mut logged_first_packet = false;
    let mut logged_first_signal = false;
    let mut next_session_validation = Instant::now() + Duration::from_secs(2);
    while !stop.load(Ordering::Acquire) {
        loop {
            let packet_frames = unsafe {
                capture
                    .GetNextPacketSize()
                    .map_err(|error| error.to_string())?
            };
            if packet_frames == 0 {
                break;
            }
            let packet = WasapiPacket::acquire(&capture)?;
            let frames = packet.frames();
            let silent = packet.is_silent();
            if silent {
                mono.clear();
                mono.resize(frames as usize, 0.0);
            } else {
                let byte_count = frames as usize * usize::from(format.block_align);
                let bytes = unsafe { slice::from_raw_parts(packet.data(), byte_count) };
                convert_interleaved_to_mono_into(bytes, frames as usize, format, &mut mono)?;
            }
            resampler.process(&mono, &mut converted);
            let has_signal = converted.iter().any(|sample| sample.abs() > 0.0001);
            let peak = converted
                .iter()
                .map(|sample| sample.abs())
                .fold(0.0_f32, f32::max);
            let rms = if converted.is_empty() {
                0.0
            } else {
                (converted.iter().map(|sample| sample * sample).sum::<f32>()
                    / converted.len() as f32)
                    .sqrt()
            };
            for &sample in &converted {
                ring.push(sample);
            }
            packet.release()?;
            packets = packets.saturating_add(1);
            captured_samples = captured_samples.saturating_add(frames as u64);
            if !logged_first_packet {
                logged_first_packet = true;
                crate::diagnostics::critical_event(
                    "info",
                    "audio.packet.first",
                    "FIRST_AUDIO_PACKET",
                    "Received the first WASAPI packet",
                    serde_json::json!({ "route": route, "frames": frames }),
                );
            }
            if has_signal {
                last_signal_at = Some(Instant::now());
                if !logged_first_signal {
                    logged_first_signal = true;
                    crate::diagnostics::critical_event(
                        "info",
                        "audio.signal.first",
                        "FIRST_NON_SILENT_SAMPLE",
                        "Received the first non-silent WASAPI sample",
                        serde_json::json!({ "route": route, "rms": rms, "peak": peak }),
                    );
                }
                set_status(
                    status,
                    CaptureState::Listening,
                    SampleFlowState::Flowing,
                    Some(route),
                    Some(source_name.to_string()),
                    None,
                );
            }
            if let Ok(mut current) = status.lock() {
                current.capture_initialized = true;
                current.packets_received = true;
                current.non_silent_samples_received |= has_signal;
                if has_signal {
                    current.reactive_ready = true;
                }
                current.captured_samples = captured_samples;
                current.captured_frames = captured_samples;
                current.dropped_samples = ring.dropped_samples();
                current.rms = rms;
                current.peak = peak;
            }
        }

        let now = Instant::now();
        if route_policy == RoutePolicy::RekordboxSession && now >= next_session_validation {
            next_session_validation = now + Duration::from_secs(2);
            let process_ids = rekordbox_process_ids();
            if process_ids.is_empty() {
                if let Ok(mut current) = status.lock() {
                    current.rekordbox_detected = false;
                    current.rekordbox_session_detected = false;
                }
                let _ = unsafe { client.Stop() };
                return Err(
                    "REKORDBOX_PROCESS_NOT_FOUND: Rekordbox exited; waiting for it to restart"
                        .to_string(),
                );
            }
            if let Some(expected_id) = rekordbox_endpoint_id {
                match active_output_endpoints_for(&process_ids) {
                    Ok(endpoints)
                        if !endpoints.iter().any(|endpoint| {
                            endpoint.id == expected_id && endpoint.has_rekordbox_session
                        }) =>
                    {
                        if let Ok(mut current) = status.lock() {
                            current.rekordbox_detected = true;
                            current.rekordbox_session_detected = false;
                        }
                        let _ = unsafe { client.Stop() };
                        return Err("REKORDBOX_AUDIO_SESSION_CHANGED: Rekordbox moved or closed its Windows audio session; refreshing the endpoint".to_string());
                    }
                    Err(error) => crate::diagnostics::event(
                        "warn",
                        "audio.rekordboxSession.validationFailed",
                        &format!("Could not refresh Rekordbox audio sessions: {error}"),
                    ),
                    _ => {}
                }
            }
        }
        let silence_age = last_signal_at
            .map(|last| now.saturating_duration_since(last))
            .unwrap_or_else(|| now.saturating_duration_since(started_at));
        if silence_age >= Duration::from_secs(2) {
            if let Ok(mut current) = status.lock() {
                current.reactive_ready = false;
            }
            set_status(
                status,
                CaptureState::Recovering,
                SampleFlowState::Silent,
                Some(route),
                Some(source_name.to_string()),
                Some(match route {
                    CaptureRoute::RekordboxSessionOutput => format!(
                        "Connected to Rekordbox on {source_name}, but its Windows stream is silent. Play a track and verify PC MASTER OUT."
                    ),
                    CaptureRoute::AutomaticOutput => format!(
                        "No music detected on {source_name}; checking the other Windows outputs. If Rekordbox uses ASIO, enable PC MASTER OUT."
                    ),
                    _ => "Audio client is connected but the selected output is silent".to_string(),
                }),
            );
        }
        if ReconnectPolicy::default().should_retry(
            route_policy,
            last_signal_at.is_some(),
            silence_age,
        ) {
            let _ = unsafe { client.Stop() };
            return Err(match route_policy {
                RoutePolicy::RekordboxSession => {
                    "Rekordbox audio session remained silent; refreshing its output endpoint"
                        .to_string()
                }
                RoutePolicy::AutomaticOutput => {
                    "Windows output remained silent; checking another output".to_string()
                }
                RoutePolicy::SelectedOutput => {
                    unreachable!("selected outputs do not auto-retry on silence")
                }
            });
        }
        thread::sleep(Duration::from_millis(8));
    }
    unsafe { client.Stop().map_err(|error| error.to_string())? };
    Ok(())
}

struct WasapiPacket<'a> {
    capture: &'a IAudioCaptureClient,
    data: *mut u8,
    frames: u32,
    flags: u32,
    release_once: ReleaseOnce,
}

impl<'a> WasapiPacket<'a> {
    fn acquire(capture: &'a IAudioCaptureClient) -> Result<Self, String> {
        let mut data = std::ptr::null_mut();
        let mut frames = 0;
        let mut flags = 0;
        unsafe {
            capture
                .GetBuffer(&mut data, &mut frames, &mut flags, None, None)
                .map_err(|error| windows_error("WASAPI_GET_BUFFER_FAILED", &error))?;
        }
        Ok(Self {
            capture,
            data,
            frames,
            flags,
            release_once: ReleaseOnce::default(),
        })
    }

    fn data(&self) -> *const u8 {
        self.data.cast_const()
    }

    fn frames(&self) -> u32 {
        self.frames
    }

    fn is_silent(&self) -> bool {
        self.flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || self.data.is_null()
    }

    fn release(mut self) -> Result<(), String> {
        self.release_inner()
    }

    fn release_inner(&mut self) -> Result<(), String> {
        if !self.release_once.claim() {
            return Ok(());
        }
        unsafe {
            self.capture
                .ReleaseBuffer(self.frames)
                .map_err(|error| windows_error("WASAPI_RELEASE_BUFFER_FAILED", &error))
        }
    }
}

impl Drop for WasapiPacket<'_> {
    fn drop(&mut self) {
        if let Err(error) = self.release_inner() {
            crate::diagnostics::event("error", "audio.buffer.release.failed", &error);
        }
    }
}

fn windows_error(code: &str, error: &windows::core::Error) -> String {
    format!(
        "{code}: HRESULT 0x{:08X}: {}",
        error.code().0 as u32,
        error.message()
    )
}

unsafe fn parse_wave_format(raw: *const WAVEFORMATEX) -> Result<AudioFormat, String> {
    let base = unsafe { *raw };
    let tag = base.wFormatTag;
    let bits = base.wBitsPerSample;
    let sample_format = if u32::from(tag) == WAVE_FORMAT_IEEE_FLOAT && bits == 32 {
        SampleFormat::Float32
    } else if u32::from(tag) == WAVE_FORMAT_PCM {
        pcm_format(bits)?
    } else if u32::from(tag) == WAVE_FORMAT_EXTENSIBLE {
        if usize::from(base.cbSize) < size_of::<WAVEFORMATEXTENSIBLE>() - size_of::<WAVEFORMATEX>()
        {
            return Err("WASAPI returned a truncated extensible audio format".to_string());
        }
        let extensible = unsafe { *raw.cast::<WAVEFORMATEXTENSIBLE>() };
        let sub_format = extensible.SubFormat;
        if sub_format == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT && bits == 32 {
            SampleFormat::Float32
        } else if sub_format == KSDATAFORMAT_SUBTYPE_PCM {
            pcm_format(bits)?
        } else {
            return Err(format!(
                "Unsupported WASAPI extensible subtype: {sub_format:?}"
            ));
        }
    } else {
        return Err(format!(
            "Unsupported WASAPI format tag 0x{tag:04x} ({bits} bits)"
        ));
    };
    AudioFormat {
        sample_format,
        channels: base.nChannels,
        sample_rate: base.nSamplesPerSec,
        block_align: base.nBlockAlign,
    }
    .validate()
}

fn pcm_format(bits: u16) -> Result<SampleFormat, String> {
    match bits {
        16 => Ok(SampleFormat::Pcm16),
        24 => Ok(SampleFormat::Pcm24),
        32 => Ok(SampleFormat::Pcm32),
        _ => Err(format!("Unsupported PCM bit depth: {bits}")),
    }
}

fn rekordbox_process_ids() -> HashSet<u32> {
    let processes = process_snapshot();
    let roots = processes
        .iter()
        .filter(|process| process.executable.eq_ignore_ascii_case("rekordbox.exe"))
        .map(|process| process.pid)
        .collect::<HashSet<_>>();
    expand_process_tree(&processes, roots)
}

fn expand_process_tree(processes: &[ProcessEntry], mut included: HashSet<u32>) -> HashSet<u32> {
    loop {
        let before = included.len();
        for process in processes {
            if included.contains(&process.parent_pid) {
                included.insert(process.pid);
            }
        }
        if included.len() == before {
            return included;
        }
    }
}

fn process_snapshot() -> Vec<ProcessEntry> {
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return Vec::new();
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut processes = Vec::new();
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|character| *character == 0)
                    .unwrap_or(entry.szExeFile.len());
                processes.push(ProcessEntry {
                    pid: entry.th32ProcessID,
                    parent_pid: entry.th32ParentProcessID,
                    executable: String::from_utf16_lossy(&entry.szExeFile[..end]),
                });
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
        processes
    }
}

fn selected_device_name(id: &str) -> Result<String, String> {
    let enumerator: IMMDeviceEnumerator = unsafe {
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .map_err(|error| error.to_string())?
    };
    let device = unsafe {
        enumerator
            .GetDevice(&HSTRING::from(id))
            .map_err(|error| error.to_string())?
    };
    device_name(&device).map_err(|error| error.to_string())
}

fn device_id(device: &IMMDevice) -> windows::core::Result<String> {
    let raw = unsafe { device.GetId()? };
    let result = unsafe { raw.to_string() }
        .map_err(|_| windows::core::Error::from_hresult(HRESULT(0x80004005_u32 as i32)));
    unsafe { CoTaskMemFree(Some(raw.0.cast())) };
    result
}

fn device_name(device: &IMMDevice) -> windows::core::Result<String> {
    let store = unsafe { device.OpenPropertyStore(STGM_READ)? };
    let mut value = unsafe { store.GetValue(&FRIENDLY_NAME_KEY)? };
    let mut buffer = [0_u16; 256];
    let conversion = unsafe { PropVariantToString(&value, &mut buffer) };
    let _ = unsafe { PropVariantClear(&mut value) };
    conversion?;
    let end = buffer
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(buffer.len());
    Ok(String::from_utf16_lossy(&buffer[..end]))
}

fn set_status(
    status: &Mutex<CaptureStatus>,
    state: CaptureState,
    sample_flow: SampleFlowState,
    route: Option<CaptureRoute>,
    source_name: Option<String>,
    message: Option<String>,
) {
    if let Ok(mut current) = status.lock() {
        current.state = state;
        current.sample_flow = sample_flow;
        if let Some(route) = route {
            current.route = route;
        }
        if source_name.is_some() {
            current.source_name = source_name;
        }
        current.message = message;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rekordbox_process_tree_includes_nested_audio_helpers() {
        let processes = vec![
            ProcessEntry {
                pid: 10,
                parent_pid: 1,
                executable: "rekordbox.exe".to_string(),
            },
            ProcessEntry {
                pid: 11,
                parent_pid: 10,
                executable: "rekordboxAgent.exe".to_string(),
            },
            ProcessEntry {
                pid: 12,
                parent_pid: 11,
                executable: "audio-helper.exe".to_string(),
            },
            ProcessEntry {
                pid: 20,
                parent_pid: 1,
                executable: "unrelated.exe".to_string(),
            },
        ];
        let included = expand_process_tree(&processes, HashSet::from([10]));

        assert_eq!(included, HashSet::from([10, 11, 12]));
    }
}
