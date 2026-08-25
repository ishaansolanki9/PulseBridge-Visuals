mod ring_buffer;
mod types;

#[cfg(not(target_os = "windows"))]
mod platform_stub;
#[cfg(target_os = "windows")]
mod wasapi;

use std::{
    sync::{atomic::AtomicBool, Arc, Mutex},
    thread::JoinHandle,
};

pub use ring_buffer::PcmRingBuffer;
#[cfg(target_os = "windows")]
pub use ring_buffer::CAPTURE_SAMPLE_RATE;
#[cfg(target_os = "windows")]
pub use types::AudioSourceKind;
pub use types::{AudioSourceInfo, CaptureState, CaptureStatus};

#[cfg(not(target_os = "windows"))]
use platform_stub as platform;
#[cfg(target_os = "windows")]
use wasapi as platform;

pub fn enumerate_audio_sources() -> Result<Vec<AudioSourceInfo>, String> {
    platform::enumerate_sources()
}

pub fn spawn_audio_capture(
    source_id: String,
    ring: Arc<PcmRingBuffer>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<CaptureStatus>>,
) -> Result<JoinHandle<()>, String> {
    platform::spawn_capture(source_id, ring, stop, status)
}
