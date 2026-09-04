use std::{
    mem::{size_of, ManuallyDrop},
    slice,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use windows::{
    core::{implement, Interface, Ref, GUID, HRESULT, HSTRING},
    Win32::{
        Foundation::{CloseHandle, PROPERTYKEY},
        Media::{
            Audio::{
                eMultimedia, eRender, ActivateAudioInterfaceAsync,
                IActivateAudioInterfaceAsyncOperation, IActivateAudioInterfaceCompletionHandler,
                IActivateAudioInterfaceCompletionHandler_Impl, IAudioCaptureClient, IAudioClient,
                IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, AUDCLNT_BUFFERFLAGS_SILENT,
                AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
                AUDIOCLIENT_ACTIVATION_PARAMS, AUDIOCLIENT_ACTIVATION_PARAMS_0,
                AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK, AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS,
                DEVICE_STATE_ACTIVE, PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
                VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
                WAVE_FORMAT_PCM,
            },
            KernelStreaming::{KSDATAFORMAT_SUBTYPE_PCM, WAVE_FORMAT_EXTENSIBLE},
            Multimedia::{KSDATAFORMAT_SUBTYPE_IEEE_FLOAT, WAVE_FORMAT_IEEE_FLOAT},
        },
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
                StructuredStorage::{
                    PropVariantClear, PropVariantToString, PROPVARIANT, PROPVARIANT_0,
                    PROPVARIANT_0_0, PROPVARIANT_0_0_0,
                },
                BLOB, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ,
            },
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            SystemInformation::{GetVersionExW, OSVERSIONINFOW},
            Variant::VT_BLOB,
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
const MIN_PROCESS_LOOPBACK_BUILD: u32 = 20_348;
const EXPERIMENTAL_PROCESS_LOOPBACK_ENV: &str = "PULSEBRIDGE_EXPERIMENTAL_PROCESS_LOOPBACK";
const PROCESS_ACTIVATION_RELEASE_DELAY: Duration = Duration::from_millis(50);
const FRIENDLY_NAME_KEY: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
    pid: 14,
};

#[derive(Clone, Copy, Debug)]
struct ProcessCandidate {
    pid: u32,
    parent_pid: u32,
    root: bool,
}

#[derive(Debug)]
struct OutputEndpoint {
    id: String,
    name: String,
    is_default: bool,
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
    let candidates = find_rekordbox_processes();
    let detected = !candidates.is_empty();
    let experimental_process_capture = experimental_process_loopback_enabled();
    let mut sources = vec![AudioSourceInfo {
        id: format!("{PROCESS_PREFIX}auto"),
        name: if detected && experimental_process_capture {
            "Rekordbox process (Detected · experimental capture)".to_string()
        } else if detected {
            "Rekordbox detected (Automatic Windows-output capture)".to_string()
        } else {
            "Rekordbox (Automatic Windows-output capture)".to_string()
        },
        kind: AudioSourceKind::RekordboxProcess,
        detected,
        is_default: false,
        // Process capture has an explicit default-output fallback, so a closed Rekordbox
        // process does not make startup invalid.
        available: true,
    }];

    for endpoint in active_output_endpoints()? {
        sources.push(AudioSourceInfo {
            id: format!("{DEVICE_PREFIX}{}", endpoint.id),
            name: endpoint.name,
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
        let detected = !find_rekordbox_processes().is_empty();
        if let Ok(mut current) = status.lock() {
            current.rekordbox_detected = detected;
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
        let discovery_stage = crate::diagnostics::begin_stage(
            "audio.processDiscovery",
            "REKORDBOX_PROCESS_DISCOVERY_BEGIN",
            "Enumerating deterministic Rekordbox process candidates",
            serde_json::Value::Null,
        );
        let candidates = find_rekordbox_processes();
        if candidates.is_empty() {
            discovery_stage.degraded(
                "REKORDBOX_PROCESS_NOT_FOUND",
                "Rekordbox is not running; trying the explicit output fallback",
                serde_json::json!({ "candidateCount": 0 }),
            );
        } else {
            discovery_stage.pass(
                "REKORDBOX_PROCESS_FOUND",
                "Found deterministic Rekordbox process candidates",
                serde_json::json!({ "candidateCount": candidates.len() }),
            );
        }
        if let Ok(mut current) = status.lock() {
            current.rekordbox_detected = !candidates.is_empty();
        }
        crate::diagnostics::event(
            "info",
            "audio.process.candidates",
            &format!(
                "count={} candidates={}",
                candidates.len(),
                candidates
                    .iter()
                    .map(|candidate| format!(
                        "pid:{} parent:{} root:{}",
                        candidate.pid, candidate.parent_pid, candidate.root
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        );

        // Process-specific ActivateAudioInterfaceAsync repeatedly terminated the affected
        // Windows machine with STATUS_HEAP_CORRUPTION before its completion callback returned.
        // Keep the Rekordbox-aware selection useful, but route it through the proven stable
        // Windows output loopback unless a developer explicitly opts into the native process
        // API for an isolated hardware test.
        if !experimental_process_loopback_enabled() {
            let reason = if candidates.is_empty() {
                "PROCESS_LOOPBACK_SAFE_FALLBACK: Rekordbox is not running; using stable Windows-output loopback"
                    .to_string()
            } else {
                "PROCESS_LOOPBACK_SAFE_FALLBACK: Rekordbox was detected, but process-specific capture is disabled after a native heap-corruption failure; using stable Windows-output loopback"
                    .to_string()
            };
            crate::diagnostics::critical_event(
                "warn",
                "audio.process.safeFallback",
                "PROCESS_LOOPBACK_DISABLED_SAFE_MODE",
                &reason,
                serde_json::json!({
                    "candidateCount": candidates.len(),
                    "route": "defaultOutputFallback",
                }),
            );
            return capture_output_fallback(&reason, ring, stop, status);
        }

        let (_, _, build) = windows_version_parts();
        if build < MIN_PROCESS_LOOPBACK_BUILD {
            let reason = format!(
                "WINDOWS_BUILD_UNSUPPORTED: process loopback requires Windows build {MIN_PROCESS_LOOPBACK_BUILD} or later; detected build {build}"
            );
            crate::diagnostics::critical_event(
                "warn",
                "audio.process.unsupported",
                "WINDOWS_BUILD_UNSUPPORTED",
                &reason,
                serde_json::json!({
                    "detectedBuild": build,
                    "minimumBuild": MIN_PROCESS_LOOPBACK_BUILD,
                }),
            );
            return capture_output_fallback(&reason, ring, stop, status);
        }

        let mut process_errors = Vec::new();
        for candidate in candidates {
            crate::diagnostics::event(
                "info",
                "audio.process.activate",
                &format!("pid={}", candidate.pid),
            );
            let activation_stage = crate::diagnostics::begin_stage(
                "audio.processLoopbackActivation",
                "PROCESS_LOOPBACK_ACTIVATION_BEGIN",
                "Activating Rekordbox process loopback",
                serde_json::json!({ "pid": candidate.pid }),
            );
            let client = match activate_process_client(candidate.pid) {
                Ok(client) => {
                    activation_stage.pass(
                        "PROCESS_LOOPBACK_ACTIVATED",
                        "Rekordbox process-loopback client activated",
                        serde_json::json!({ "pid": candidate.pid }),
                    );
                    client
                }
                Err(error) => {
                    activation_stage.error(
                        "PROCESS_LOOPBACK_ACTIVATION_FAILED",
                        &error,
                        serde_json::json!({ "pid": candidate.pid }),
                    );
                    crate::diagnostics::event(
                        "warn",
                        "audio.process.failed",
                        &format!("pid={} error={error}", candidate.pid),
                    );
                    process_errors.push(format!("PID {}: {error}", candidate.pid));
                    continue;
                }
            };
            match capture_client(
                client,
                "Rekordbox process",
                CaptureRoute::RekordboxProcess,
                ring,
                stop,
                status,
                RoutePolicy::Process,
            ) {
                Ok(()) => return Ok(()),
                Err(error) if !stop.load(Ordering::Acquire) => {
                    crate::diagnostics::event(
                        "warn",
                        "audio.process.failed",
                        &format!("pid={} error={error}", candidate.pid),
                    );
                    process_errors.push(format!("PID {}: {error}", candidate.pid));
                }
                Err(error) => return Err(error),
            }
        }

        let reason = if process_errors.is_empty() {
            "Rekordbox is not running".to_string()
        } else {
            process_errors.join("; ")
        };
        capture_output_fallback(&reason, ring, stop, status)
    } else if let Some(device_id) = source_id.strip_prefix(DEVICE_PREFIX) {
        let client = activate_endpoint_client(Some(device_id))?;
        let name = selected_device_name(device_id).unwrap_or_else(|_| "Windows output".to_string());
        capture_client(
            client,
            &name,
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

fn capture_output_fallback(
    reason: &str,
    ring: &PcmRingBuffer,
    stop: &AtomicBool,
    status: &Mutex<CaptureStatus>,
) -> Result<(), String> {
    if let Ok(mut current) = status.lock() {
        current.fallback_attempted = true;
        current.preferred_route_failure = Some(reason.to_string());
    }
    set_status(
        status,
        CaptureState::Recovering,
        SampleFlowState::Waiting,
        Some(CaptureRoute::DefaultOutputFallback),
        Some("Finding active Windows output…".to_string()),
        Some(format!(
            "Rekordbox process audio was unavailable ({reason}); checking Windows outputs for live music"
        )),
    );
    crate::diagnostics::critical_event(
        "warn",
        "audio.fallback.activate",
        "OUTPUT_LOOPBACK_ACTIVATION_BEGIN",
        "Checking active Windows outputs after process capture was unavailable",
        serde_json::json!({ "preferredRouteFailure": reason }),
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

    let endpoint_count = endpoints.len();
    let mut failures = Vec::new();
    for (index, endpoint) in endpoints.into_iter().enumerate() {
        if stop.load(Ordering::Acquire) {
            return Ok(());
        }
        let position = index + 1;
        let message = format!(
            "Checking Windows output {position}/{endpoint_count}: {}",
            endpoint.name
        );
        set_status(
            status,
            CaptureState::Connecting,
            SampleFlowState::Waiting,
            Some(CaptureRoute::DefaultOutputFallback),
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
                "probeIndex": position,
                "probeCount": endpoint_count,
                "preferredRouteFailure": reason,
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
            CaptureRoute::DefaultOutputFallback,
            ring,
            stop,
            status,
            RoutePolicy::DefaultFallback,
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
            id,
            name,
        });
    }
    endpoints.sort_by_key(|endpoint| !endpoint.is_default);
    Ok(endpoints)
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

#[implement(IActivateAudioInterfaceCompletionHandler)]
struct ActivationHandler {
    sender: Mutex<Option<mpsc::Sender<Result<IAudioClient, String>>>>,
}

impl IActivateAudioInterfaceCompletionHandler_Impl for ActivationHandler_Impl {
    fn ActivateCompleted(
        &self,
        activate_operation: Ref<IActivateAudioInterfaceAsyncOperation>,
    ) -> windows::core::Result<()> {
        let result = (|| -> windows::core::Result<IAudioClient> {
            let operation = activate_operation.ok()?;
            let mut activation_result = HRESULT::default();
            let mut interface = None;
            unsafe { operation.GetActivateResult(&mut activation_result, &mut interface)? };
            activation_result.ok()?;
            interface
                .ok_or_else(|| windows::core::Error::from_hresult(HRESULT(0x80004005_u32 as i32)))?
                .cast::<IAudioClient>()
        })()
        .map_err(|error| error.to_string());
        if let Ok(mut sender) = self.sender.lock() {
            if let Some(sender) = sender.take() {
                let _ = sender.send(result);
            }
        }
        Ok(())
    }
}

fn activate_process_client(pid: u32) -> Result<IAudioClient, String> {
    let params = AUDIOCLIENT_ACTIVATION_PARAMS {
        ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
        Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
            ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                TargetProcessId: pid,
                ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
            },
        },
    };
    let inner = PROPVARIANT_0_0 {
        vt: VT_BLOB,
        Anonymous: PROPVARIANT_0_0_0 {
            blob: BLOB {
                cbSize: size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
                pBlobData: &params as *const _ as *mut u8,
            },
        },
        ..Default::default()
    };
    let activation_params = PROPVARIANT {
        Anonymous: PROPVARIANT_0 {
            Anonymous: ManuallyDrop::new(inner),
        },
    };
    let (sender, receiver) = mpsc::channel();
    let handler: IActivateAudioInterfaceCompletionHandler = ActivationHandler {
        sender: Mutex::new(Some(sender)),
    }
    .into();
    let _operation = unsafe {
        ActivateAudioInterfaceAsync(
            VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
            &IAudioClient::IID,
            Some(&activation_params),
            &handler,
        )
        .map_err(|error| {
            format!(
                "PROCESS_LOOPBACK_ACTIVATION_FAILED: {}",
                windows_error("ACTIVATE_AUDIO_INTERFACE_ASYNC", &error)
            )
        })?
    };
    // ActivateAudioInterfaceAsync has no cancellation API. Keep both COM objects on this MTA
    // thread until Windows calls ActivateCompleted, exactly as the reference sample waits for
    // its completion event. The old five-second timeout could drop them while a late native
    // callback was still pending, violating the documented lifetime contract.
    let result = receiver.recv().map_err(|_| {
        "PROCESS_LOOPBACK_ACTIVATION_FAILED: Windows closed the activation callback unexpectedly"
            .to_string()
    })?;
    // The callback sends immediately before returning. Give it time to leave native callback
    // dispatch before this thread releases the operation and its final handler reference.
    thread::sleep(PROCESS_ACTIVATION_RELEASE_DELAY);
    result.map_err(|error| format!("PROCESS_LOOPBACK_ACTIVATION_FAILED: {error}"))
}

fn experimental_process_loopback_enabled() -> bool {
    process_loopback_opted_in(
        std::env::var(EXPERIMENTAL_PROCESS_LOOPBACK_ENV)
            .ok()
            .as_deref(),
    )
}

fn process_loopback_opted_in(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn capture_client(
    client: IAudioClient,
    source_name: &str,
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
                Some(if route == CaptureRoute::RekordboxProcess {
                    "Rekordbox audio client is connected but no signal is flowing. If using ASIO, enable PC MASTER OUT or select its Windows output.".to_string()
                } else if route == CaptureRoute::DefaultOutputFallback {
                    format!("No music detected on {source_name}; checking the other Windows outputs. If Rekordbox uses ASIO, enable PC MASTER OUT.")
                } else {
                    "Audio client is connected but the selected output is silent".to_string()
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
                RoutePolicy::Process => {
                    "Rekordbox process capture produced no live samples".to_string()
                }
                RoutePolicy::DefaultFallback => {
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

fn find_rekordbox_processes() -> Vec<ProcessCandidate> {
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return Vec::new();
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut matches = Vec::new();
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|character| *character == 0)
                    .unwrap_or(entry.szExeFile.len());
                let executable = String::from_utf16_lossy(&entry.szExeFile[..end]).to_lowercase();
                if executable == "rekordbox.exe" {
                    matches.push(ProcessCandidate {
                        pid: entry.th32ProcessID,
                        parent_pid: entry.th32ParentProcessID,
                        root: true,
                    });
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
        let pids = matches
            .iter()
            .map(|candidate| candidate.pid)
            .collect::<Vec<_>>();
        for candidate in &mut matches {
            candidate.root = !pids.contains(&candidate.parent_pid);
        }
        matches.sort_by_key(|candidate| (!candidate.root, candidate.pid));
        matches
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
    use super::process_loopback_opted_in;

    #[test]
    fn process_loopback_requires_an_explicit_developer_opt_in() {
        assert!(!process_loopback_opted_in(None));
        assert!(!process_loopback_opted_in(Some("")));
        assert!(!process_loopback_opted_in(Some("false")));
        assert!(process_loopback_opted_in(Some("1")));
        assert!(process_loopback_opted_in(Some(" TRUE ")));
        assert!(process_loopback_opted_in(Some("yes")));
    }
}
