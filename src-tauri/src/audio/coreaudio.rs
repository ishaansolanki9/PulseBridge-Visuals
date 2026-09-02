use std::{
    ffi::c_void,
    mem::{size_of, transmute},
    ptr, slice,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use core_foundation::{
    array::CFArray, base::TCFType, boolean::CFBoolean, dictionary::CFDictionary, string::CFString,
};
use coreaudio_sys as ca;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, Sample, SampleFormat as CpalSampleFormat, SizedSample,
};
use objc2::{
    msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject},
};
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSArray, NSNumber, NSUUID};

use super::{
    AudioSourceInfo, AudioSourceKind, CaptureRoute, CaptureState, CaptureStatus, PcmRingBuffer,
    PlatformCaptureBackend, SampleFlowState, StreamingResampler, CAPTURE_SAMPLE_RATE,
};

const PROCESS_SOURCE_ID: &str = "process:auto";
const SYSTEM_OUTPUT_SOURCE_ID: &str = "output:default";
const INPUT_SOURCE_PREFIX: &str = "input:";
const MIN_MACOS_MAJOR: u32 = 14;
const MIN_MACOS_MINOR: u32 = 2;
const MAX_AUDIO_BUFFERS: usize = 32;
const MAX_CALLBACK_SAMPLES: usize = 131_072;

type CreateProcessTap =
    unsafe extern "C" fn(*mut AnyObject, *mut ca::AudioObjectID) -> ca::OSStatus;
type DestroyProcessTap = unsafe extern "C" fn(ca::AudioObjectID) -> ca::OSStatus;

pub struct Backend;

impl PlatformCaptureBackend for Backend {
    fn enumerate_sources() -> Result<Vec<AudioSourceInfo>, String> {
        let supported = macos_support().is_ok();
        let process = find_rekordbox_process();
        let host = cpal::default_host();
        let default_output_name = host
            .default_output_device()
            .map(|device| device.to_string())
            .unwrap_or_else(|| "Default speakers".to_string());
        let mut sources = vec![
            AudioSourceInfo {
                id: PROCESS_SOURCE_ID.to_string(),
                name: if !supported {
                    "Rekordbox process (requires macOS 14.2+)".to_string()
                } else if process.is_some() {
                    "Rekordbox process (Detected)".to_string()
                } else {
                    "Rekordbox process (Not running)".to_string()
                },
                kind: AudioSourceKind::RekordboxProcess,
                detected: process.is_some(),
                is_default: true,
                available: supported,
            },
            AudioSourceInfo {
                id: SYSTEM_OUTPUT_SOURCE_ID.to_string(),
                name: if supported {
                    format!("System audio · {default_output_name}")
                } else {
                    "System audio (requires macOS 14.2+)".to_string()
                },
                kind: AudioSourceKind::OutputDevice,
                detected: true,
                is_default: false,
                available: supported,
            },
        ];

        let default_input_id = host
            .default_input_device()
            .and_then(|device| device.id().ok())
            .map(|id| id.to_string());
        match host.input_devices() {
            Ok(devices) => {
                let mut inputs = devices
                    .filter_map(|device| {
                        let id = device.id().ok()?.to_string();
                        let name = device.to_string();
                        Some(AudioSourceInfo {
                            id: format!("{INPUT_SOURCE_PREFIX}{id}"),
                            name: format!("Input · {name}"),
                            kind: AudioSourceKind::InputDevice,
                            detected: true,
                            is_default: default_input_id.as_deref() == Some(id.as_str()),
                            available: true,
                        })
                    })
                    .collect::<Vec<_>>();
                inputs.sort_by(|left, right| {
                    right
                        .is_default
                        .cmp(&left.is_default)
                        .then_with(|| left.name.cmp(&right.name))
                });
                sources.extend(inputs);
            }
            Err(error) => crate::diagnostics::event(
                "warn",
                "audio.coreAudio.inputEnumeration",
                &format!("Unable to enumerate macOS input devices: {error}"),
            ),
        }
        Ok(sources)
    }

    fn spawn_capture(
        source_id: String,
        ring: Arc<PcmRingBuffer>,
        stop: Arc<AtomicBool>,
        status: Arc<Mutex<CaptureStatus>>,
    ) -> Result<JoinHandle<()>, String> {
        if source_id != PROCESS_SOURCE_ID
            && source_id != SYSTEM_OUTPUT_SOURCE_ID
            && !source_id.starts_with(INPUT_SOURCE_PREFIX)
        {
            return Err("Unknown macOS audio source".to_string());
        }
        thread::Builder::new()
            .name("pulsebridge-coreaudio-capture".to_string())
            .spawn(move || {
                let worker_stop = Arc::clone(&stop);
                let worker_status = Arc::clone(&status);
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_capture(&source_id, ring, stop, status)
                }));
                if result.is_err() {
                    crate::diagnostics::critical_event(
                        "error",
                        "audio.coreAudio.panic",
                        "CORE_AUDIO_WORKER_PANIC",
                        "The Core Audio capture worker panicked",
                        serde_json::Value::Null,
                    );
                    update_failure(
                        &worker_status,
                        CaptureState::Failed,
                        "Core Audio capture failed unexpectedly",
                    );
                    worker_stop.store(true, Ordering::Release);
                }
                crate::diagnostics::event("info", "worker.audio.exit", "Audio worker exited");
            })
            .map_err(|error| error.to_string())
    }
}

fn run_capture(
    source_id: &str,
    ring: Arc<PcmRingBuffer>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<CaptureStatus>>,
) {
    if source_id == SYSTEM_OUTPUT_SOURCE_ID {
        run_system_output_capture(ring, stop, status);
        return;
    }
    if source_id.starts_with(INPUT_SOURCE_PREFIX) {
        run_input_capture(source_id, ring, stop, status);
        return;
    }
    run_process_capture(ring, stop, status);
}

fn run_process_capture(
    ring: Arc<PcmRingBuffer>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<CaptureStatus>>,
) {
    if let Err(error) = macos_support() {
        update_failure(&status, CaptureState::Unsupported, &error);
        return;
    }

    crate::diagnostics::critical_event(
        "info",
        "audio.coreAudio.start",
        "CORE_AUDIO_PROCESS_TAP_START",
        "Starting private Core Audio process-tap capture",
        serde_json::json!({ "minimumMacOS": "14.2" }),
    );
    while !stop.load(Ordering::Acquire) {
        let discovery_stage = crate::diagnostics::begin_stage(
            "audio.processDiscovery",
            "REKORDBOX_PROCESS_DISCOVERY_BEGIN",
            "Discovering Rekordbox with supported macOS workspace APIs",
            serde_json::Value::Null,
        );
        let Some((pid, name)) = find_rekordbox_process() else {
            discovery_stage.degraded(
                "REKORDBOX_PROCESS_NOT_FOUND",
                "Rekordbox is not running; waiting without claiming a connection",
                serde_json::Value::Null,
            );
            if let Ok(mut current) = status.lock() {
                current.state = CaptureState::Recovering;
                current.sample_flow = SampleFlowState::Waiting;
                current.rekordbox_detected = false;
                current.message = Some("Waiting for Rekordbox to start".to_string());
            }
            interruptible_delay(&stop, Duration::from_millis(800));
            continue;
        };
        discovery_stage.pass(
            "REKORDBOX_PROCESS_FOUND",
            "Found a supported Rekordbox process",
            serde_json::json!({ "pid": pid }),
        );
        if let Ok(mut current) = status.lock() {
            current.state = CaptureState::Connecting;
            current.sample_flow = SampleFlowState::Waiting;
            current.rekordbox_detected = true;
            current.source_name = Some(name.clone());
            current.message = Some("Requesting macOS system-audio permission".to_string());
        }
        let result = capture_process(
            pid,
            &name,
            Arc::clone(&ring),
            Arc::clone(&stop),
            Arc::clone(&status),
        );
        if stop.load(Ordering::Acquire) {
            break;
        }
        let message = result.err().unwrap_or_else(|| {
            "The Rekordbox Core Audio process tap ended unexpectedly".to_string()
        });
        crate::diagnostics::critical_event(
            "warn",
            "audio.coreAudio.recovering",
            "CORE_AUDIO_CAPTURE_INTERRUPTED",
            &message,
            serde_json::json!({ "pid": pid }),
        );
        if let Ok(mut current) = status.lock() {
            current.state = CaptureState::Recovering;
            current.sample_flow = SampleFlowState::Unavailable;
            current.message = Some(format!("{message}. Retrying…"));
        }
        interruptible_delay(&stop, Duration::from_millis(800));
    }
    if let Ok(mut current) = status.lock() {
        current.state = CaptureState::Stopped;
        current.route = CaptureRoute::None;
        current.sample_flow = SampleFlowState::Unavailable;
        current.message = None;
    }
}

fn run_system_output_capture(
    ring: Arc<PcmRingBuffer>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<CaptureStatus>>,
) {
    if let Err(error) = macos_support() {
        update_failure(&status, CaptureState::Unsupported, &error);
        return;
    }
    crate::diagnostics::critical_event(
        "info",
        "audio.coreAudio.systemStart",
        "CORE_AUDIO_SYSTEM_TAP_START",
        "Starting Core Audio capture for all outgoing system audio",
        serde_json::json!({ "minimumMacOS": "14.2" }),
    );
    while !stop.load(Ordering::Acquire) {
        let source_name = default_output_source_name();
        if let Ok(mut current) = status.lock() {
            current.state = CaptureState::Connecting;
            current.route = CaptureRoute::SelectedOutput;
            current.sample_flow = SampleFlowState::Waiting;
            current.rekordbox_detected = find_rekordbox_process().is_some();
            current.source_name = Some(source_name.clone());
            current.message = Some("Requesting macOS system-audio permission".to_string());
        }
        let result = capture_system_output(
            &source_name,
            Arc::clone(&ring),
            Arc::clone(&stop),
            Arc::clone(&status),
        );
        if stop.load(Ordering::Acquire) {
            break;
        }
        let message = result
            .err()
            .unwrap_or_else(|| "The system-audio tap ended unexpectedly".to_string());
        if let Ok(mut current) = status.lock() {
            current.state = CaptureState::Recovering;
            current.sample_flow = SampleFlowState::Unavailable;
            current.message = Some(format!("{message}. Retrying…"));
        }
        interruptible_delay(&stop, Duration::from_millis(800));
    }
    mark_capture_stopped(&status);
}

fn run_input_capture(
    source_id: &str,
    ring: Arc<PcmRingBuffer>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<CaptureStatus>>,
) {
    while !stop.load(Ordering::Acquire) {
        let result = capture_input_device(
            source_id,
            Arc::clone(&ring),
            Arc::clone(&stop),
            Arc::clone(&status),
        );
        if stop.load(Ordering::Acquire) {
            break;
        }
        let message = result
            .err()
            .unwrap_or_else(|| "The selected input stream ended unexpectedly".to_string());
        crate::diagnostics::critical_event(
            "warn",
            "audio.coreAudio.inputRecovering",
            "CORE_AUDIO_INPUT_INTERRUPTED",
            &message,
            serde_json::json!({ "sourceId": source_id }),
        );
        if let Ok(mut current) = status.lock() {
            current.state = CaptureState::Recovering;
            current.route = CaptureRoute::SelectedInput;
            current.sample_flow = SampleFlowState::Unavailable;
            current.message = Some(format!("{message}. Retrying…"));
        }
        interruptible_delay(&stop, Duration::from_millis(800));
    }
    mark_capture_stopped(&status);
}

fn capture_process(
    pid: libc::pid_t,
    process_name: &str,
    ring: Arc<PcmRingBuffer>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<CaptureStatus>>,
) -> Result<(), String> {
    let translation_stage = crate::diagnostics::begin_stage(
        "audio.processTranslation",
        "CORE_AUDIO_PROCESS_TRANSLATION_BEGIN",
        "Translating the Rekordbox PID to a Core Audio process object",
        serde_json::json!({ "pid": pid }),
    );
    let process_object = match translate_pid(pid) {
        Ok(object) => {
            translation_stage.pass(
                "CORE_AUDIO_PROCESS_TRANSLATED",
                "Resolved the Rekordbox Core Audio process object",
                serde_json::json!({ "pid": pid }),
            );
            object
        }
        Err(error) => {
            translation_stage.error(
                "CORE_AUDIO_PROCESS_TRANSLATION_FAILED",
                &error,
                serde_json::json!({ "pid": pid }),
            );
            return Err(error);
        }
    };
    let uuid = NSUUID::new();
    let tap_uid = uuid.UUIDString().to_string();
    let process_number = NSNumber::new_u32(process_object);
    let processes = NSArray::from_retained_slice(&[process_number]);
    let class = tap_description_class()?;
    let allocated: *mut AnyObject = unsafe { msg_send![class, alloc] };
    let raw_description: *mut AnyObject =
        unsafe { msg_send![allocated, initStereoMixdownOfProcesses: &*processes] };
    let description = configure_tap_description(raw_description, &uuid, "Rekordbox")?;
    capture_tap(
        description,
        &tap_uid,
        &format!("{process_name} process tap"),
        CaptureRoute::RekordboxProcess,
        true,
        Some(pid),
        ring,
        stop,
        status,
    )
}

fn capture_system_output(
    source_name: &str,
    ring: Arc<PcmRingBuffer>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<CaptureStatus>>,
) -> Result<(), String> {
    let uuid = NSUUID::new();
    let tap_uid = uuid.UUIDString().to_string();
    let excluded: Retained<NSArray<NSNumber>> = NSArray::from_retained_slice(&[]);
    let class = tap_description_class()?;
    let allocated: *mut AnyObject = unsafe { msg_send![class, alloc] };
    let raw_description: *mut AnyObject =
        unsafe { msg_send![allocated, initStereoGlobalTapButExcludeProcesses: &*excluded] };
    let description = configure_tap_description(raw_description, &uuid, "System audio")?;
    capture_tap(
        description,
        &tap_uid,
        source_name,
        CaptureRoute::SelectedOutput,
        find_rekordbox_process().is_some(),
        None,
        ring,
        stop,
        status,
    )
}

fn tap_description_class() -> Result<&'static AnyClass, String> {
    AnyClass::get(c"CATapDescription")
        .ok_or_else(|| "Core Audio process taps are unavailable on this macOS build".to_string())
}

fn configure_tap_description(
    raw_description: *mut AnyObject,
    uuid: &NSUUID,
    label: &str,
) -> Result<Retained<AnyObject>, String> {
    let description = unsafe { Retained::from_raw(raw_description) }
        .ok_or_else(|| format!("Core Audio could not create the {label} tap description"))?;
    unsafe {
        let _: () = msg_send![&*description, setPrivate: true];
        let _: () = msg_send![&*description, setMuteBehavior: 0_isize];
        let _: () = msg_send![&*description, setUUID: uuid];
    }
    Ok(description)
}

#[allow(clippy::too_many_arguments)]
fn capture_tap(
    description: Retained<AnyObject>,
    tap_uid: &str,
    source_name: &str,
    route: CaptureRoute,
    rekordbox_detected: bool,
    monitored_pid: Option<libc::pid_t>,
    ring: Arc<PcmRingBuffer>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<CaptureStatus>>,
) -> Result<(), String> {
    let (create_tap, destroy_tap) = process_tap_symbols()?;
    let tap_stage = crate::diagnostics::begin_stage(
        "audio.processTapActivation",
        "CORE_AUDIO_PROCESS_TAP_CREATE_BEGIN",
        "Creating a private non-mutating Core Audio tap",
        serde_json::json!({ "source": source_name, "pid": monitored_pid }),
    );
    let mut tap_id = ca::kAudioObjectUnknown;
    let create_status =
        unsafe { create_tap(Retained::as_ptr(&description).cast_mut(), &mut tap_id) };
    if let Err(error) = check_status(
        create_status,
        "CORE_AUDIO_PROCESS_TAP_CREATE_FAILED",
        "Unable to create the audio tap. Allow PulseBridge in System Settings > Privacy & Security > Screen & System Audio Recording, then retry",
    ) {
        tap_stage.error(
            "CORE_AUDIO_PROCESS_TAP_CREATE_FAILED",
            &error,
            serde_json::json!({ "source": source_name, "pid": monitored_pid }),
        );
        return Err(error);
    }
    tap_stage.pass(
        "CORE_AUDIO_PROCESS_TAP_CREATED",
        "Private Core Audio tap created",
        serde_json::json!({ "source": source_name, "pid": monitored_pid }),
    );
    let format_stage = crate::diagnostics::begin_stage(
        "audio.format",
        "CORE_AUDIO_FORMAT_QUERY_BEGIN",
        "Querying and validating the Core Audio tap stream format",
        serde_json::Value::Null,
    );
    let format = match tap_format(tap_id) {
        Ok(format) => {
            format_stage.pass(
                "AUDIO_FORMAT_SUPPORTED",
                "Core Audio tap format validated",
                serde_json::json!({
                    "sampleRate": format.sample_rate,
                    "channels": format.channels,
                    "nonInterleaved": format.non_interleaved,
                }),
            );
            format
        }
        Err(error) => {
            format_stage.error("UNSUPPORTED_AUDIO_FORMAT", &error, serde_json::Value::Null);
            unsafe { destroy_tap(tap_id) };
            return Err(error);
        }
    };
    let client_stage = crate::diagnostics::begin_stage(
        "audio.clientInitialization",
        "CORE_AUDIO_CLIENT_INITIALIZATION_BEGIN",
        "Creating the private aggregate and Core Audio IO callback",
        serde_json::Value::Null,
    );
    let aggregate = match create_aggregate_device(tap_uid, source_name) {
        Ok(device) => device,
        Err(error) => {
            client_stage.error(
                "CORE_AUDIO_AGGREGATE_CREATE_FAILED",
                &error,
                serde_json::Value::Null,
            );
            unsafe { destroy_tap(tap_id) };
            return Err(error);
        }
    };
    let callback = match CallbackState::new(Arc::clone(&ring), format) {
        Ok(callback) => Arc::new(callback),
        Err(error) => {
            unsafe {
                ca::AudioHardwareDestroyAggregateDevice(aggregate);
                destroy_tap(tap_id);
            }
            client_stage.error(
                "CORE_AUDIO_RESAMPLER_CREATE_FAILED",
                &error,
                serde_json::Value::Null,
            );
            return Err(error);
        }
    };
    let callback_ptr = Arc::into_raw(Arc::clone(&callback))
        .cast_mut()
        .cast::<c_void>();
    let mut io_proc = None;
    let io_status = unsafe {
        ca::AudioDeviceCreateIOProcID(
            aggregate,
            Some(audio_io_callback),
            callback_ptr,
            &mut io_proc,
        )
    };
    if let Err(error) = check_status(
        io_status,
        "CORE_AUDIO_IO_PROC_CREATE_FAILED",
        "Unable to install the Core Audio tap callback",
    ) {
        unsafe {
            drop(Arc::from_raw(callback_ptr.cast::<CallbackState>()));
            ca::AudioHardwareDestroyAggregateDevice(aggregate);
            destroy_tap(tap_id);
        }
        client_stage.error(
            "CORE_AUDIO_IO_PROC_CREATE_FAILED",
            &error,
            serde_json::Value::Null,
        );
        return Err(error);
    }
    let mut session = CoreAudioSession {
        tap_id,
        aggregate_device: aggregate,
        io_proc,
        callback_ptr,
        destroy_tap,
        started: false,
        _description: description,
    };
    if let Err(error) = check_status(
        unsafe { ca::AudioDeviceStart(aggregate, io_proc) },
        "CORE_AUDIO_CLIENT_START_FAILED",
        "Core Audio created the tap but could not start it",
    ) {
        client_stage.error(
            "CORE_AUDIO_CLIENT_START_FAILED",
            &error,
            serde_json::Value::Null,
        );
        return Err(error);
    }
    session.started = true;
    client_stage.pass(
        "AUDIO_CLIENT_STARTED",
        "Core Audio tap client initialized and started",
        serde_json::Value::Null,
    );
    if let Ok(mut current) = status.lock() {
        initialize_capture_status(
            &mut current,
            route,
            rekordbox_detected,
            source_name,
            format.sample_rate,
            format.channels,
            "32-bit float Core Audio",
        );
    }
    crate::diagnostics::critical_event(
        "info",
        "audio.coreAudio.initialized",
        "CORE_AUDIO_CAPTURE_INITIALIZED",
        "Core Audio tap initialized",
        serde_json::json!({
            "source": source_name,
            "route": route,
            "sampleRate": format.sample_rate,
            "channels": format.channels,
            "nonInterleaved": format.non_interleaved,
        }),
    );

    let mut last_signal_at = None;
    let mut observed_packets = 0;
    let mut process_check_counter = 0_u8;
    while !stop.load(Ordering::Acquire) {
        publish_callback_status(
            &callback,
            &status,
            source_name,
            route,
            rekordbox_detected,
            &mut observed_packets,
            &mut last_signal_at,
        );
        process_check_counter = process_check_counter.wrapping_add(1);
        if monitored_pid.is_some()
            && process_check_counter.is_multiple_of(10)
            && find_rekordbox_process().is_none()
        {
            return Err("Rekordbox exited; the process tap was released".to_string());
        }
        interruptible_delay(&stop, Duration::from_millis(100));
    }
    drop(session);
    Ok(())
}

fn capture_input_device(
    source_id: &str,
    ring: Arc<PcmRingBuffer>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<CaptureStatus>>,
) -> Result<(), String> {
    let selected_id = source_id
        .strip_prefix(INPUT_SOURCE_PREFIX)
        .ok_or_else(|| "Invalid macOS input source".to_string())?;
    let host = cpal::default_host();
    let device = host
        .input_devices()
        .map_err(|error| format!("Unable to enumerate input devices: {error}"))?
        .find(|device| device.id().is_ok_and(|id| id.to_string() == selected_id))
        .ok_or_else(|| "The selected input device is no longer connected".to_string())?;
    let source_name = format!("Input · {device}");
    let rekordbox_detected = find_rekordbox_process().is_some();
    if let Ok(mut current) = status.lock() {
        current.state = CaptureState::Connecting;
        current.route = CaptureRoute::SelectedInput;
        current.sample_flow = SampleFlowState::Waiting;
        current.rekordbox_detected = rekordbox_detected;
        current.source_name = Some(source_name.clone());
        current.message = Some("Requesting microphone or line-in permission".to_string());
    }

    let supported = device
        .default_input_config()
        .map_err(|error| format!("Unable to read the selected input format: {error}"))?;
    let sample_format = supported.sample_format();
    let sample_rate = supported.sample_rate();
    let channels = supported.channels();
    if channels == 0 || channels > 32 || !(8_000..=384_000).contains(&sample_rate) {
        return Err(format!(
            "Unsupported input layout: {sample_rate} Hz, {channels} channels"
        ));
    }
    let callback = Arc::new(CallbackState::new(
        Arc::clone(&ring),
        CoreAudioFormat {
            sample_rate,
            channels,
            non_interleaved: false,
        },
    )?);
    let stream_error = Arc::new(Mutex::new(None::<String>));
    let config = supported.into();
    let stream = match sample_format {
        CpalSampleFormat::I8 => build_input_stream::<i8>(
            &device,
            config,
            Arc::clone(&callback),
            Arc::clone(&stream_error),
        ),
        CpalSampleFormat::I16 => build_input_stream::<i16>(
            &device,
            config,
            Arc::clone(&callback),
            Arc::clone(&stream_error),
        ),
        CpalSampleFormat::I24 => build_input_stream::<cpal::I24>(
            &device,
            config,
            Arc::clone(&callback),
            Arc::clone(&stream_error),
        ),
        CpalSampleFormat::I32 => build_input_stream::<i32>(
            &device,
            config,
            Arc::clone(&callback),
            Arc::clone(&stream_error),
        ),
        CpalSampleFormat::I64 => build_input_stream::<i64>(
            &device,
            config,
            Arc::clone(&callback),
            Arc::clone(&stream_error),
        ),
        CpalSampleFormat::U8 => build_input_stream::<u8>(
            &device,
            config,
            Arc::clone(&callback),
            Arc::clone(&stream_error),
        ),
        CpalSampleFormat::U16 => build_input_stream::<u16>(
            &device,
            config,
            Arc::clone(&callback),
            Arc::clone(&stream_error),
        ),
        CpalSampleFormat::U24 => build_input_stream::<cpal::U24>(
            &device,
            config,
            Arc::clone(&callback),
            Arc::clone(&stream_error),
        ),
        CpalSampleFormat::U32 => build_input_stream::<u32>(
            &device,
            config,
            Arc::clone(&callback),
            Arc::clone(&stream_error),
        ),
        CpalSampleFormat::U64 => build_input_stream::<u64>(
            &device,
            config,
            Arc::clone(&callback),
            Arc::clone(&stream_error),
        ),
        CpalSampleFormat::F32 => build_input_stream::<f32>(
            &device,
            config,
            Arc::clone(&callback),
            Arc::clone(&stream_error),
        ),
        CpalSampleFormat::F64 => build_input_stream::<f64>(
            &device,
            config,
            Arc::clone(&callback),
            Arc::clone(&stream_error),
        ),
        unsupported => Err(format!("Unsupported input sample format: {unsupported}")),
    }?;
    stream
        .play()
        .map_err(|error| format!("Unable to start the selected input: {error}"))?;
    if let Ok(mut current) = status.lock() {
        initialize_capture_status(
            &mut current,
            CaptureRoute::SelectedInput,
            rekordbox_detected,
            &source_name,
            sample_rate,
            channels,
            &sample_format.to_string(),
        );
    }
    crate::diagnostics::critical_event(
        "info",
        "audio.coreAudio.inputInitialized",
        "CORE_AUDIO_INPUT_INITIALIZED",
        "Selected macOS input initialized",
        serde_json::json!({
            "source": source_name,
            "sampleRate": sample_rate,
            "channels": channels,
            "format": sample_format.to_string(),
        }),
    );
    let mut last_signal_at = None;
    let mut observed_packets = 0;
    while !stop.load(Ordering::Acquire) {
        if let Ok(mut error) = stream_error.lock() {
            if let Some(error) = error.take() {
                return Err(error);
            }
        }
        publish_callback_status(
            &callback,
            &status,
            &source_name,
            CaptureRoute::SelectedInput,
            rekordbox_detected,
            &mut observed_packets,
            &mut last_signal_at,
        );
        interruptible_delay(&stop, Duration::from_millis(100));
    }
    drop(stream);
    Ok(())
}

fn build_input_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    callback: Arc<CallbackState>,
    stream_error: Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream, String>
where
    T: Sample + SizedSample + Copy,
    f32: FromSample<T>,
{
    device
        .build_input_stream::<T, _, _>(
            config,
            move |samples, _| process_input_samples(samples, &callback),
            move |error| {
                if let Ok(mut current) = stream_error.lock() {
                    *current = Some(format!("The selected input stream failed: {error}"));
                }
            },
            Some(Duration::from_secs(3)),
        )
        .map_err(|error| {
            format!(
                "Unable to open the selected input. Allow PulseBridge in System Settings > Privacy & Security > Microphone, then retry: {error}"
            )
        })
}

fn process_input_samples<T>(samples: &[T], state: &CallbackState)
where
    T: Sample + Copy,
    f32: FromSample<T>,
{
    let Ok(mut pipeline) = state.pipeline.try_lock() else {
        return;
    };
    let channels = usize::from(pipeline.format.channels);
    if channels == 0 {
        return;
    }
    let frames = samples.len() / channels;
    if frames == 0 || frames > MAX_CALLBACK_SAMPLES {
        return;
    }
    pipeline.mono.clear();
    let mono_capacity = pipeline.mono.capacity();
    pipeline.mono.reserve(frames.saturating_sub(mono_capacity));
    for frame in samples.chunks_exact(channels) {
        let total = frame.iter().copied().map(f32::from_sample).sum::<f32>();
        pipeline
            .mono
            .push((total / channels as f32).clamp(-1.0, 1.0));
    }
    let mono = std::mem::take(&mut pipeline.mono);
    let CallbackPipeline {
        resampler,
        converted,
        ..
    } = &mut *pipeline;
    resampler.process(&mono, converted);
    pipeline.mono = mono;
    for &sample in &pipeline.converted {
        state.ring.push(sample);
    }
    let peak = pipeline
        .converted
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0_f32, f32::max);
    let rms = if pipeline.converted.is_empty() {
        0.0
    } else {
        (pipeline
            .converted
            .iter()
            .map(|sample| sample * sample)
            .sum::<f32>()
            / pipeline.converted.len() as f32)
            .sqrt()
    };
    state.packets.fetch_add(1, Ordering::Relaxed);
    state.frames.fetch_add(frames as u64, Ordering::Relaxed);
    state.rms_bits.store(rms.to_bits(), Ordering::Relaxed);
    state.peak_bits.store(peak.to_bits(), Ordering::Relaxed);
    if peak > 0.0001 {
        state.non_silent.store(true, Ordering::Release);
    }
}

fn initialize_capture_status(
    current: &mut CaptureStatus,
    route: CaptureRoute,
    rekordbox_detected: bool,
    source_name: &str,
    sample_rate: u32,
    channels: u16,
    format: &str,
) {
    current.state = CaptureState::Connecting;
    current.route = route;
    current.sample_flow = SampleFlowState::Waiting;
    current.rekordbox_detected = rekordbox_detected;
    current.capture_initialized = true;
    current.source_name = Some(source_name.to_string());
    current.sample_rate = Some(sample_rate);
    current.channels = Some(channels);
    current.format = Some(format.to_string());
    current.message = Some("Audio capture initialized; waiting for samples".to_string());
}

fn default_output_source_name() -> String {
    cpal::default_host()
        .default_output_device()
        .map(|device| format!("System audio · {device}"))
        .unwrap_or_else(|| "System audio · Default speakers".to_string())
}

fn mark_capture_stopped(status: &Mutex<CaptureStatus>) {
    if let Ok(mut current) = status.lock() {
        current.state = CaptureState::Stopped;
        current.route = CaptureRoute::None;
        current.sample_flow = SampleFlowState::Unavailable;
        current.message = None;
    }
}

#[derive(Clone, Copy)]
struct CoreAudioFormat {
    sample_rate: u32,
    channels: u16,
    non_interleaved: bool,
}

fn tap_format(tap_id: ca::AudioObjectID) -> Result<CoreAudioFormat, String> {
    let address = ca::AudioObjectPropertyAddress {
        mSelector: ca::kAudioTapPropertyFormat,
        mScope: ca::kAudioObjectPropertyScopeGlobal,
        mElement: ca::kAudioObjectPropertyElementMain,
    };
    let mut format = ca::AudioStreamBasicDescription::default();
    let mut size = size_of::<ca::AudioStreamBasicDescription>() as u32;
    check_status(
        unsafe {
            ca::AudioObjectGetPropertyData(
                tap_id,
                &address,
                0,
                ptr::null(),
                &mut size,
                (&mut format as *mut ca::AudioStreamBasicDescription).cast(),
            )
        },
        "CORE_AUDIO_FORMAT_QUERY_FAILED",
        "Unable to read the Core Audio tap stream format",
    )?;
    if format.mFormatID != ca::kAudioFormatLinearPCM
        || format.mFormatFlags & ca::kAudioFormatFlagIsFloat == 0
        || format.mBitsPerChannel != 32
        || format.mChannelsPerFrame == 0
        || format.mChannelsPerFrame > 32
        || !format.mSampleRate.is_finite()
        || !(8_000.0..=384_000.0).contains(&format.mSampleRate)
    {
        return Err(format!(
            "CORE_AUDIO_UNSUPPORTED_FORMAT: format={} flags=0x{:x} rate={} channels={} bits={}",
            fourcc(format.mFormatID as i32),
            format.mFormatFlags,
            format.mSampleRate,
            format.mChannelsPerFrame,
            format.mBitsPerChannel
        ));
    }
    Ok(CoreAudioFormat {
        sample_rate: format.mSampleRate.round() as u32,
        channels: format.mChannelsPerFrame as u16,
        non_interleaved: format.mFormatFlags & ca::kAudioFormatFlagIsNonInterleaved != 0,
    })
}

fn translate_pid(pid: libc::pid_t) -> Result<ca::AudioObjectID, String> {
    let address = ca::AudioObjectPropertyAddress {
        mSelector: ca::kAudioHardwarePropertyTranslatePIDToProcessObject,
        mScope: ca::kAudioObjectPropertyScopeGlobal,
        mElement: ca::kAudioObjectPropertyElementMain,
    };
    let mut process_object = ca::kAudioObjectUnknown;
    let mut output_size = size_of::<ca::AudioObjectID>() as u32;
    check_status(
        unsafe {
            ca::AudioObjectGetPropertyData(
                ca::kAudioObjectSystemObject,
                &address,
                size_of::<libc::pid_t>() as u32,
                (&pid as *const libc::pid_t).cast(),
                &mut output_size,
                (&mut process_object as *mut ca::AudioObjectID).cast(),
            )
        },
        "CORE_AUDIO_PROCESS_TRANSLATION_FAILED",
        "Rekordbox was detected but Core Audio could not resolve its process-audio object",
    )?;
    if process_object == ca::kAudioObjectUnknown {
        return Err(
            "CORE_AUDIO_PROCESS_TRANSLATION_FAILED: Core Audio returned no process object"
                .to_string(),
        );
    }
    Ok(process_object)
}

fn create_aggregate_device(tap_uid: &str, source_name: &str) -> Result<ca::AudioObjectID, String> {
    let tap = CFDictionary::<CFString, core_foundation::base::CFType>::from_CFType_pairs(&[
        (
            CFString::from_static_string("uid"),
            CFString::new(tap_uid).as_CFType(),
        ),
        (
            CFString::from_static_string("drift"),
            CFBoolean::true_value().as_CFType(),
        ),
    ]);
    let taps = CFArray::from_CFTypes(&[tap.as_CFType()]);
    let aggregate_uid = format!("com.pulsebridge.tap.{}", uuid::Uuid::new_v4());
    let aggregate = CFDictionary::<CFString, core_foundation::base::CFType>::from_CFType_pairs(&[
        (
            CFString::from_static_string("uid"),
            CFString::new(&aggregate_uid).as_CFType(),
        ),
        (
            CFString::from_static_string("name"),
            CFString::new(&format!("PulseBridge private {source_name} tap")).as_CFType(),
        ),
        (
            CFString::from_static_string("private"),
            CFBoolean::true_value().as_CFType(),
        ),
        (
            CFString::from_static_string("tapautostart"),
            CFBoolean::false_value().as_CFType(),
        ),
        (CFString::from_static_string("taps"), taps.as_CFType()),
    ]);
    let mut device = ca::kAudioObjectUnknown;
    check_status(
        unsafe {
            ca::AudioHardwareCreateAggregateDevice(
                aggregate.as_concrete_TypeRef().cast::<ca::__CFDictionary>(),
                &mut device,
            )
        },
        "CORE_AUDIO_AGGREGATE_CREATE_FAILED",
        "Unable to create the private Core Audio aggregate used to read the process tap",
    )?;
    Ok(device)
}

struct CoreAudioSession {
    tap_id: ca::AudioObjectID,
    aggregate_device: ca::AudioObjectID,
    io_proc: ca::AudioDeviceIOProcID,
    callback_ptr: *mut c_void,
    destroy_tap: DestroyProcessTap,
    started: bool,
    _description: Retained<AnyObject>,
}

impl Drop for CoreAudioSession {
    fn drop(&mut self) {
        unsafe {
            if self.started {
                let status = ca::AudioDeviceStop(self.aggregate_device, self.io_proc);
                log_cleanup_status("audio.coreAudio.stop", status);
            }
            let status = ca::AudioDeviceDestroyIOProcID(self.aggregate_device, self.io_proc);
            log_cleanup_status("audio.coreAudio.destroyIoProc", status);
            let status = ca::AudioHardwareDestroyAggregateDevice(self.aggregate_device);
            log_cleanup_status("audio.coreAudio.destroyAggregate", status);
            let status = (self.destroy_tap)(self.tap_id);
            log_cleanup_status("audio.coreAudio.destroyTap", status);
            drop(Arc::from_raw(self.callback_ptr.cast::<CallbackState>()));
        }
    }
}

struct CallbackState {
    ring: Arc<PcmRingBuffer>,
    pipeline: Mutex<CallbackPipeline>,
    packets: AtomicU64,
    frames: AtomicU64,
    non_silent: AtomicBool,
    rms_bits: AtomicU32,
    peak_bits: AtomicU32,
}

impl CallbackState {
    fn new(ring: Arc<PcmRingBuffer>, format: CoreAudioFormat) -> Result<Self, String> {
        Ok(Self {
            ring,
            pipeline: Mutex::new(CallbackPipeline {
                format,
                mono: Vec::with_capacity(32_768),
                converted: Vec::with_capacity(32_768),
                resampler: StreamingResampler::new(format.sample_rate, CAPTURE_SAMPLE_RATE)?,
            }),
            packets: AtomicU64::new(0),
            frames: AtomicU64::new(0),
            non_silent: AtomicBool::new(false),
            rms_bits: AtomicU32::new(0.0_f32.to_bits()),
            peak_bits: AtomicU32::new(0.0_f32.to_bits()),
        })
    }
}

struct CallbackPipeline {
    format: CoreAudioFormat,
    mono: Vec<f32>,
    converted: Vec<f32>,
    resampler: StreamingResampler,
}

impl CallbackPipeline {
    unsafe fn process(&mut self, input: *const ca::AudioBufferList, ring: &PcmRingBuffer) -> usize {
        if input.is_null() {
            return 0;
        }
        let count = unsafe { (*input).mNumberBuffers as usize };
        if count == 0 || count > MAX_AUDIO_BUFFERS {
            return 0;
        }
        let buffers = unsafe {
            slice::from_raw_parts(
                ptr::addr_of!((*input).mBuffers).cast::<ca::AudioBuffer>(),
                count,
            )
        };
        let mut frames = usize::MAX;
        for buffer in buffers {
            let channels = if self.format.non_interleaved {
                buffer.mNumberChannels.max(1) as usize
            } else {
                self.format.channels as usize
            };
            let available = buffer.mDataByteSize as usize / (size_of::<f32>() * channels);
            frames = frames.min(available);
        }
        if frames == usize::MAX || frames == 0 || frames > MAX_CALLBACK_SAMPLES {
            return 0;
        }
        self.mono.clear();
        self.mono.resize(frames, 0.0);
        if self.format.non_interleaved {
            let mut channels_seen = 0_usize;
            for buffer in buffers {
                if buffer.mData.is_null() {
                    continue;
                }
                let channel_count = buffer.mNumberChannels.max(1) as usize;
                let samples = unsafe {
                    slice::from_raw_parts(buffer.mData.cast::<f32>(), frames * channel_count)
                };
                for frame in 0..frames {
                    for channel in 0..channel_count {
                        self.mono[frame] += samples[frame * channel_count + channel];
                    }
                }
                channels_seen += channel_count;
            }
            if channels_seen == 0 {
                return 0;
            }
            for sample in &mut self.mono {
                *sample = (*sample / channels_seen as f32).clamp(-1.0, 1.0);
            }
        } else {
            let Some(buffer) = buffers.first() else {
                return 0;
            };
            if buffer.mData.is_null() {
                return 0;
            }
            let channels = self.format.channels as usize;
            let samples =
                unsafe { slice::from_raw_parts(buffer.mData.cast::<f32>(), frames * channels) };
            for frame in 0..frames {
                let start = frame * channels;
                let total = samples[start..start + channels]
                    .iter()
                    .copied()
                    .sum::<f32>();
                self.mono[frame] = (total / channels as f32).clamp(-1.0, 1.0);
            }
        }
        self.resampler.process(&self.mono, &mut self.converted);
        for &sample in &self.converted {
            ring.push(sample);
        }
        frames
    }
}

unsafe extern "C" fn audio_io_callback(
    _device: ca::AudioObjectID,
    _now: *const ca::AudioTimeStamp,
    input: *const ca::AudioBufferList,
    _input_time: *const ca::AudioTimeStamp,
    _output: *mut ca::AudioBufferList,
    _output_time: *const ca::AudioTimeStamp,
    client_data: *mut c_void,
) -> ca::OSStatus {
    if client_data.is_null() {
        return 0;
    }
    let state = unsafe { &*client_data.cast::<CallbackState>() };
    let Ok(mut pipeline) = state.pipeline.try_lock() else {
        return 0;
    };
    let frames = unsafe { pipeline.process(input, &state.ring) };
    if frames == 0 {
        return 0;
    }
    let samples = &pipeline.converted;
    let peak = samples
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0_f32, f32::max);
    let rms = if samples.is_empty() {
        0.0
    } else {
        (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
    };
    state.packets.fetch_add(1, Ordering::Relaxed);
    state.frames.fetch_add(frames as u64, Ordering::Relaxed);
    state.rms_bits.store(rms.to_bits(), Ordering::Relaxed);
    state.peak_bits.store(peak.to_bits(), Ordering::Relaxed);
    if peak > 0.0001 {
        state.non_silent.store(true, Ordering::Release);
    }
    0
}

fn publish_callback_status(
    callback: &CallbackState,
    status: &Mutex<CaptureStatus>,
    source_name: &str,
    route: CaptureRoute,
    rekordbox_detected: bool,
    observed_packets: &mut u64,
    last_signal_at: &mut Option<Instant>,
) {
    let packets = callback.packets.load(Ordering::Acquire);
    let frames = callback.frames.load(Ordering::Acquire);
    let non_silent = callback.non_silent.load(Ordering::Acquire);
    let peak = f32::from_bits(callback.peak_bits.load(Ordering::Relaxed));
    if packets != *observed_packets && peak > 0.0001 {
        *last_signal_at = Some(Instant::now());
    }
    *observed_packets = packets;
    let reactive_ready = last_signal_at.is_some_and(|last| last.elapsed() < Duration::from_secs(2));
    if let Ok(mut current) = status.lock() {
        current.state = if reactive_ready {
            CaptureState::Listening
        } else {
            CaptureState::Connecting
        };
        current.route = route;
        current.sample_flow = if reactive_ready {
            SampleFlowState::Flowing
        } else if packets > 0 {
            SampleFlowState::Silent
        } else {
            SampleFlowState::Waiting
        };
        current.rekordbox_detected = rekordbox_detected;
        current.capture_initialized = true;
        current.packets_received = packets > 0;
        current.non_silent_samples_received = non_silent;
        current.reactive_ready = reactive_ready;
        current.source_name = Some(source_name.to_string());
        current.captured_samples = frames;
        current.captured_frames = frames;
        current.dropped_samples = callback.ring.dropped_samples();
        current.rms = f32::from_bits(callback.rms_bits.load(Ordering::Relaxed));
        current.peak = peak;
        current.message = if reactive_ready {
            None
        } else if packets > 0 {
            Some(format!("{source_name} is initialized but currently silent"))
        } else {
            Some("Core Audio tap initialized; waiting for the first packet".to_string())
        };
    }
}

fn find_rekordbox_process() -> Option<(libc::pid_t, String)> {
    let workspace = NSWorkspace::sharedWorkspace();
    let applications = workspace.runningApplications();
    let mut matches = Vec::new();
    for index in 0..applications.count() {
        let application = applications.objectAtIndex(index);
        let name = application.localizedName().map(|name| name.to_string());
        let bundle = application
            .bundleIdentifier()
            .map(|bundle| bundle.to_string());
        let is_rekordbox = name
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("rekordbox"))
            || bundle.as_deref().is_some_and(|value| {
                let value = value.to_ascii_lowercase();
                value.contains("rekordbox") || value.contains("pioneer.dj")
            });
        let pid = application.processIdentifier();
        if is_rekordbox && pid > 0 {
            matches.push((pid, name.unwrap_or_else(|| "Rekordbox".to_string())));
        }
    }
    matches.sort_by_key(|(pid, _)| *pid);
    matches.into_iter().next()
}

fn macos_support() -> Result<(), String> {
    let output = std::process::Command::new("sw_vers")
        .args(["-productVersion"])
        .output()
        .map_err(|error| format!("Unable to determine the macOS version: {error}"))?;
    let version = String::from_utf8_lossy(&output.stdout);
    let mut parts = version.trim().split('.');
    let major = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    if (major, minor) < (MIN_MACOS_MAJOR, MIN_MACOS_MINOR) {
        return Err(format!(
            "Live process audio requires macOS 14.2 or later; this Mac reports {}",
            version.trim()
        ));
    }
    Ok(())
}

fn process_tap_symbols() -> Result<(CreateProcessTap, DestroyProcessTap), String> {
    let create = unsafe {
        libc::dlsym(
            libc::RTLD_DEFAULT,
            c"AudioHardwareCreateProcessTap".as_ptr(),
        )
    };
    let destroy = unsafe {
        libc::dlsym(
            libc::RTLD_DEFAULT,
            c"AudioHardwareDestroyProcessTap".as_ptr(),
        )
    };
    if create.is_null() || destroy.is_null() {
        return Err(
            "Core Audio process-tap symbols are unavailable; macOS 14.2 or later is required"
                .to_string(),
        );
    }
    Ok(unsafe {
        (
            transmute::<*mut c_void, CreateProcessTap>(create),
            transmute::<*mut c_void, DestroyProcessTap>(destroy),
        )
    })
}

fn check_status(status: ca::OSStatus, code: &str, message: &str) -> Result<(), String> {
    if status == 0 {
        Ok(())
    } else {
        Err(format!(
            "{code}: {message} (OSStatus {status}, '{}')",
            fourcc(status)
        ))
    }
}

fn log_cleanup_status(event: &str, status: ca::OSStatus) {
    if status != 0 {
        crate::diagnostics::event(
            "warn",
            event,
            &format!(
                "Core Audio cleanup returned OSStatus {status} ('{}')",
                fourcc(status)
            ),
        );
    }
}

fn fourcc(status: i32) -> String {
    let bytes = status.to_be_bytes();
    if bytes
        .iter()
        .all(|byte| byte.is_ascii_graphic() || *byte == b' ')
    {
        String::from_utf8_lossy(&bytes).into_owned()
    } else {
        "non-printable".to_string()
    }
}

fn interruptible_delay(stop: &AtomicBool, duration: Duration) {
    let slices = (duration.as_millis() / 50).max(1) as usize;
    for _ in 0..slices {
        if stop.load(Ordering::Acquire) {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn update_failure(status: &Mutex<CaptureStatus>, state: CaptureState, message: &str) {
    if let Ok(mut current) = status.lock() {
        current.state = state;
        current.sample_flow = SampleFlowState::Unavailable;
        current.capture_initialized = false;
        current.reactive_ready = false;
        current.message = Some(message.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_sources_always_expose_rekordbox_and_system_audio() {
        let sources = Backend::enumerate_sources().expect("macOS source enumeration should work");
        assert!(sources.iter().any(|source| source.id == PROCESS_SOURCE_ID));
        assert!(sources
            .iter()
            .any(|source| source.id == SYSTEM_OUTPUT_SOURCE_ID));
        assert!(sources
            .iter()
            .filter(|source| source.kind == AudioSourceKind::InputDevice)
            .all(|source| source.id.starts_with(INPUT_SOURCE_PREFIX)));
    }

    #[test]
    fn core_audio_errors_keep_numeric_and_fourcc_status() {
        let error = check_status(i32::from_be_bytes(*b"perm"), "PERMISSION_DENIED", "Denied")
            .expect_err("non-zero status should fail");
        assert!(error.contains("PERMISSION_DENIED"));
        assert!(error.contains("perm"));
    }

    #[test]
    fn fourcc_handles_non_printable_statuses() {
        assert_eq!(fourcc(-50), "non-printable");
    }
}
