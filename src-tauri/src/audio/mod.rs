mod format;
#[cfg(any(target_os = "windows", test))]
mod reconnect;
mod ring_buffer;
mod types;

#[cfg(target_os = "macos")]
mod coreaudio;
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
mod platform_stub;
#[cfg(target_os = "windows")]
mod wasapi;

use std::{
    sync::{atomic::AtomicBool, Arc, Mutex},
    thread::JoinHandle,
};

#[cfg(target_os = "macos")]
pub(crate) use format::StreamingResampler;
#[cfg(target_os = "windows")]
pub(crate) use format::{
    convert_interleaved_to_mono_into, AudioFormat, SampleFormat, StreamingResampler,
};
#[cfg(target_os = "windows")]
pub(crate) use reconnect::{ReconnectPolicy, RoutePolicy};
pub use ring_buffer::PcmRingBuffer;
pub use ring_buffer::CAPTURE_SAMPLE_RATE;
pub use types::AudioSourceKind;
pub use types::CaptureRoute;
pub use types::{AudioSourceInfo, CaptureState, CaptureStatus, SampleFlowState};

#[cfg(any(target_os = "windows", test))]
#[derive(Default)]
struct ReleaseOnce(bool);

#[cfg(any(target_os = "windows", test))]
impl ReleaseOnce {
    fn claim(&mut self) -> bool {
        if self.0 {
            false
        } else {
            self.0 = true;
            true
        }
    }
}

#[cfg(target_os = "macos")]
use coreaudio as platform;
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
use platform_stub as platform;
#[cfg(target_os = "windows")]
use wasapi as platform;

pub(crate) trait PlatformCaptureBackend {
    fn enumerate_sources() -> Result<Vec<AudioSourceInfo>, String>;

    fn spawn_capture(
        source_id: String,
        ring: Arc<PcmRingBuffer>,
        stop: Arc<AtomicBool>,
        status: Arc<Mutex<CaptureStatus>>,
    ) -> Result<JoinHandle<()>, String>;
}

pub fn enumerate_audio_sources() -> Result<Vec<AudioSourceInfo>, String> {
    platform::Backend::enumerate_sources()
}

pub fn spawn_audio_capture(
    source_id: String,
    ring: Arc<PcmRingBuffer>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<CaptureStatus>>,
) -> Result<JoinHandle<()>, String> {
    platform::Backend::spawn_capture(source_id, ring, stop, status)
}

#[cfg(test)]
mod tests {
    use super::ReleaseOnce;

    #[test]
    fn packet_release_gate_can_be_claimed_exactly_once() {
        let mut release = ReleaseOnce::default();
        assert!(release.claim());
        assert!(!release.claim());
        assert!(!release.claim());
    }
}
