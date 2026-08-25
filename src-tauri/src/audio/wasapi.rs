use std::{
    mem::{size_of, ManuallyDrop},
    slice,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
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
                AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
                AUDCLNT_STREAMFLAGS_LOOPBACK, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
                AUDIOCLIENT_ACTIVATION_PARAMS, AUDIOCLIENT_ACTIVATION_PARAMS_0,
                AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK, AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS,
                DEVICE_STATE_ACTIVE, PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
                VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, WAVEFORMATEX,
            },
            Multimedia::WAVE_FORMAT_IEEE_FLOAT,
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
            Variant::VT_BLOB,
        },
    },
};

use super::{
    AudioSourceInfo, AudioSourceKind, CaptureState, CaptureStatus, PcmRingBuffer,
    CAPTURE_SAMPLE_RATE,
};

const DEVICE_PREFIX: &str = "device:";
const PROCESS_PREFIX: &str = "process:";
const CHANNELS: u16 = 2;
const FRIENDLY_NAME_KEY: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
    pid: 14,
};

pub fn enumerate_sources() -> Result<Vec<AudioSourceInfo>, String> {
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).is_ok() };
    let result = enumerate_sources_inner().map_err(|error| error.to_string());
    if initialized {
        unsafe { CoUninitialize() };
    }
    result
}

fn enumerate_sources_inner() -> windows::core::Result<Vec<AudioSourceInfo>> {
    let rekordbox_pid = find_rekordbox_process();
    let mut sources = vec![AudioSourceInfo {
        id: format!("{PROCESS_PREFIX}auto"),
        name: if rekordbox_pid.is_some() {
            "Rekordbox (Detected)".to_string()
        } else {
            "Rekordbox (Not running)".to_string()
        },
        kind: AudioSourceKind::RekordboxProcess,
        detected: rekordbox_pid.is_some(),
        is_default: false,
        available: rekordbox_pid.is_some(),
    }];

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
    for index in 0..count {
        let device = unsafe { devices.Item(index)? };
        let id = device_id(&device)?;
        let name = device_name(&device).unwrap_or_else(|_| format!("Audio output {}", index + 1));
        sources.push(AudioSourceInfo {
            id: format!("{DEVICE_PREFIX}{id}"),
            name,
            kind: AudioSourceKind::OutputDevice,
            detected: true,
            is_default: default_id.as_ref().is_some_and(|default| default == &id),
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
        .spawn(move || run_capture(source_id, ring, stop, status))
        .map_err(|error| error.to_string())
}

fn run_capture(
    source_id: String,
    ring: Arc<PcmRingBuffer>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<CaptureStatus>>,
) {
    if let Err(error) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() } {
        set_status(&status, CaptureState::Failed, None, Some(error.to_string()));
        return;
    }

    while !stop.load(Ordering::Acquire) {
        set_status(&status, CaptureState::Connecting, None, None);
        let capture_result = capture_selected_source(&source_id, &ring, &stop, &status);
        if stop.load(Ordering::Acquire) {
            break;
        }
        let message = capture_result
            .err()
            .unwrap_or_else(|| "The selected audio stream ended".to_string());
        set_status(
            &status,
            CaptureState::Recovering,
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
    set_status(&status, CaptureState::Stopped, None, None);
}

fn capture_selected_source(
    source_id: &str,
    ring: &PcmRingBuffer,
    stop: &AtomicBool,
    status: &Mutex<CaptureStatus>,
) -> Result<(), String> {
    if source_id.strip_prefix(PROCESS_PREFIX).is_some() {
        let pid = find_rekordbox_process().ok_or_else(|| "Rekordbox is not running".to_string())?;
        match activate_process_client(pid)
            .and_then(|client| capture_client(client, "Rekordbox", ring, stop, status))
        {
            Ok(()) => Ok(()),
            Err(process_error) if !stop.load(Ordering::Acquire) => {
                set_status(
                    status,
                    CaptureState::Recovering,
                    Some("Default Windows output".to_string()),
                    Some(format!(
                        "Rekordbox-only capture was unavailable ({process_error}); using output loopback"
                    )),
                );
                let client = activate_endpoint_client(None).map_err(|fallback_error| {
                    format!(
                        "Rekordbox capture failed: {process_error}; output fallback failed: {fallback_error}"
                    )
                })?;
                capture_client(client, "Default Windows output", ring, stop, status)
            }
            Err(error) => Err(error),
        }
    } else if let Some(device_id) = source_id.strip_prefix(DEVICE_PREFIX) {
        let client = activate_endpoint_client(Some(device_id))?;
        let name = selected_device_name(device_id).unwrap_or_else(|_| "Windows output".to_string());
        capture_client(client, &name, ring, stop, status)
    } else {
        Err("Unknown audio source".to_string())
    }
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
        .map_err(|error| error.to_string())?
    };
    receiver
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| "Timed out activating Rekordbox process audio".to_string())?
}

fn capture_client(
    client: IAudioClient,
    source_name: &str,
    ring: &PcmRingBuffer,
    stop: &AtomicBool,
    status: &Mutex<CaptureStatus>,
) -> Result<(), String> {
    let format = WAVEFORMATEX {
        wFormatTag: WAVE_FORMAT_IEEE_FLOAT as u16,
        nChannels: CHANNELS,
        nSamplesPerSec: CAPTURE_SAMPLE_RATE,
        nAvgBytesPerSec: CAPTURE_SAMPLE_RATE * CHANNELS as u32 * size_of::<f32>() as u32,
        nBlockAlign: CHANNELS * size_of::<f32>() as u16,
        wBitsPerSample: 32,
        cbSize: 0,
    };
    unsafe {
        client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK
                    | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                    | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
                0,
                0,
                &format,
                None,
            )
            .map_err(|error| error.to_string())?;
    }
    let capture: IAudioCaptureClient =
        unsafe { client.GetService().map_err(|error| error.to_string())? };
    unsafe { client.Start().map_err(|error| error.to_string())? };
    set_status(
        status,
        CaptureState::Listening,
        Some(source_name.to_string()),
        None,
    );

    let mut packets = 0_u32;
    let mut captured_samples = 0_u64;
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
            let mut data = std::ptr::null_mut();
            let mut frames = 0;
            let mut flags = 0;
            unsafe {
                capture
                    .GetBuffer(&mut data, &mut frames, &mut flags, None, None)
                    .map_err(|error| error.to_string())?;
            }
            let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0;
            if silent || data.is_null() {
                for _ in 0..frames {
                    ring.push(0.0);
                }
            } else {
                let samples = unsafe {
                    slice::from_raw_parts(data.cast::<f32>(), frames as usize * CHANNELS as usize)
                };
                for frame in samples.chunks_exact(CHANNELS as usize) {
                    ring.push((frame[0] + frame[1]) * 0.5);
                }
            }
            unsafe {
                capture
                    .ReleaseBuffer(frames)
                    .map_err(|error| error.to_string())?;
            }
            packets += 1;
            captured_samples += frames as u64;
            if packets.is_multiple_of(50) {
                if let Ok(mut current) = status.lock() {
                    current.captured_samples = captured_samples;
                    current.dropped_samples = ring.dropped_samples();
                }
            }
        }
        thread::sleep(Duration::from_millis(8));
    }
    unsafe { client.Stop().map_err(|error| error.to_string())? };
    Ok(())
}

fn find_rekordbox_process() -> Option<u32> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut found = None;
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|character| *character == 0)
                    .unwrap_or(entry.szExeFile.len());
                let executable = String::from_utf16_lossy(&entry.szExeFile[..end]).to_lowercase();
                if executable == "rekordbox.exe" {
                    found = Some(entry.th32ProcessID);
                    break;
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
        found
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
    source_name: Option<String>,
    message: Option<String>,
) {
    if let Ok(mut current) = status.lock() {
        current.state = state;
        if source_name.is_some() {
            current.source_name = source_name;
        }
        current.message = message;
    }
}
