use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use super::{AudioSourceInfo, CaptureState, CaptureStatus, PcmRingBuffer, PlatformCaptureBackend};

pub struct Backend;

impl PlatformCaptureBackend for Backend {
    fn enumerate_sources() -> Result<Vec<AudioSourceInfo>, String> {
        Ok(Vec::new())
    }

    fn spawn_capture(
        _source_id: String,
        _ring: Arc<PcmRingBuffer>,
        stop: Arc<AtomicBool>,
        status: Arc<Mutex<CaptureStatus>>,
    ) -> Result<JoinHandle<()>, String> {
        thread::Builder::new()
            .name("pulsebridge-audio-unavailable".to_string())
            .spawn(move || {
                crate::diagnostics::event(
                    "info",
                    "audio.unsupported",
                    "Live audio capture is unavailable on this platform",
                );
                if let Ok(mut current) = status.lock() {
                    current.state = CaptureState::Unsupported;
                    current.message = Some(
                        "Live audio capture is unavailable on this operating system".to_string(),
                    );
                }
                while !stop.load(Ordering::Acquire) {
                    thread::sleep(Duration::from_millis(100));
                }
            })
            .map_err(|error| error.to_string())
    }
}
