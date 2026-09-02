mod fft;
mod musical_state;
mod normalization;
mod rhythm;
mod types;

use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::audio::PcmRingBuffer;
use crate::phrase::{AudioInferredPhraseProvider, PhraseProvider, SharedPhrase};
use fft::{AudioAnalyzer, ANALYSIS_HOP, FFT_SIZE};
use musical_state::MusicalStateTracker;

pub use types::{AnalysisSnapshot, FeatureFrame, MusicState, VisualInputFrame};

pub type SharedAnalysis = Arc<RwLock<AnalysisSnapshot>>;

pub fn spawn_analysis(
    ring: Arc<PcmRingBuffer>,
    stop: Arc<AtomicBool>,
    shared: SharedAnalysis,
    phrase: SharedPhrase,
) -> Result<JoinHandle<()>, String> {
    thread::Builder::new()
        .name("pulsebridge-music-analysis".to_string())
        .spawn(move || {
            let worker_stop = Arc::clone(&stop);
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_analysis(ring, Arc::clone(&stop), shared, phrase)
            }));
            if outcome.is_err() {
                crate::diagnostics::event(
                    "error",
                    "worker.analysis.panic",
                    "The analysis worker panicked",
                );
                worker_stop.store(true, Ordering::Release);
            }
            crate::diagnostics::event("info", "worker.analysis.exit", "Analysis worker exited");
        })
        .map_err(|error| error.to_string())
}

fn run_analysis(
    ring: Arc<PcmRingBuffer>,
    stop: Arc<AtomicBool>,
    shared: SharedAnalysis,
    phrase: SharedPhrase,
) {
    let started_at = Instant::now();
    let mut phrase_provider = AudioInferredPhraseProvider::new(started_at);
    let mut analyzer = AudioAnalyzer::new();
    let mut state_tracker = MusicalStateTracker::new();
    let mut window = VecDeque::with_capacity(FFT_SIZE);
    let mut feature_history = VecDeque::with_capacity(6_000);
    let mut samples_since_analysis = 0;

    while !stop.load(Ordering::Acquire) {
        if ring.len() > 24_000 {
            while ring.len() > FFT_SIZE * 2 {
                let _ = ring.pop();
            }
        }

        let mut received = 0;
        while let Some(sample) = ring.pop() {
            if window.len() == FFT_SIZE {
                window.pop_front();
            }
            window.push_back(sample);
            received += 1;
        }
        samples_since_analysis += received;

        if received > 0 {
            let mut snapshot = shared
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            snapshot.last_audio_at = Some(Instant::now());
        }

        if window.len() == FFT_SIZE && samples_since_analysis >= ANALYSIS_HOP {
            samples_since_analysis %= ANALYSIS_HOP;
            let samples = window.iter().copied().collect::<Vec<_>>();
            let timestamp_ms = started_at.elapsed().as_millis() as u64;
            let features = analyzer.analyze(&samples, timestamp_ms);
            let seconds = features.timestamp_ms as f32 / 1000.0;
            let (state, impact) = state_tracker.update(seconds, &features);
            if feature_history.len() == 6_000 {
                feature_history.pop_front();
            }
            feature_history.push_back(features);
            let frame = VisualInputFrame {
                state,
                energy: features.energy_fast,
                sub: features.sub_energy,
                bass: features.bass_energy,
                mids: features.mid_energy,
                highs: features.high_energy,
                beat_phase: features.beat_phase,
                beat_pulse: features.beat_strength,
                onset: features.onset_strength,
                impact,
                reactivity: 1.0,
            };
            *phrase
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                phrase_provider.update(Instant::now(), frame);
            let mut snapshot = shared
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            snapshot.frame = frame;
            snapshot.feature_frames = feature_history.len();
        } else {
            thread::sleep(Duration::from_millis(5));
        }
    }
}
