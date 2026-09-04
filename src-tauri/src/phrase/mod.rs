use std::{
    collections::VecDeque,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::analysis::{MusicState, VisualInputFrame};

const OBSERVATION_INTERVAL: Duration = Duration::from_millis(250);
const MAX_OBSERVATIONS: usize = 256;
const FRESH_METADATA: Duration = Duration::from_secs(2);
#[allow(dead_code)]
const HOLD_METADATA: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum PhraseKind {
    Intro,
    Verse,
    Up,
    Chorus,
    Down,
    Bridge,
    Outro,
    Fill,
    #[default]
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum PhraseProvenance {
    Rekordbox,
    CueMarkers,
    AudioInferred,
    #[default]
    Unavailable,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PhraseSegment {
    pub kind: PhraseKind,
    pub index: u64,
    pub start_ms: u64,
    pub end_ms: Option<u64>,
    pub confidence: f32,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct PlaybackContext {
    pub stable_track_id: Option<String>,
    pub structure_signature: Option<u64>,
    pub position_ms: Option<u64>,
    pub playing: bool,
    pub active_deck: Option<u8>,
    pub tempo_bpm: Option<f32>,
    pub beat_confidence: Option<f32>,
    pub bar_phase: Option<f32>,
    pub phrase: Option<PhraseSegment>,
    pub phrase_progress: Option<f32>,
    pub provenance: PhraseProvenance,
    pub updated_at: Instant,
}

impl Default for PlaybackContext {
    fn default() -> Self {
        Self {
            stable_track_id: None,
            structure_signature: None,
            position_ms: None,
            playing: false,
            active_deck: None,
            tempo_bpm: None,
            beat_confidence: None,
            bar_phase: None,
            phrase: None,
            phrase_progress: None,
            provenance: PhraseProvenance::Unavailable,
            updated_at: Instant::now(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhraseStatus {
    pub provenance: PhraseProvenance,
    pub phrase: Option<PhraseKind>,
    pub confidence: Option<f32>,
    pub progress: Option<f32>,
    pub stale: bool,
    pub tempo_bpm: Option<f32>,
    pub beat_confidence: Option<f32>,
    pub bar_phase: Option<f32>,
    pub structure_model_ready: bool,
    pub message: String,
}

impl PlaybackContext {
    pub fn status(&self, now: Instant) -> PhraseStatus {
        let stale = now.saturating_duration_since(self.updated_at) > FRESH_METADATA;
        let message = match (self.provenance, stale) {
            (PhraseProvenance::Rekordbox, false) => "Live Rekordbox phrase metadata".to_string(),
            (PhraseProvenance::CueMarkers, false) => "Cue-derived structure".to_string(),
            (PhraseProvenance::AudioInferred, false) if self.structure_signature.is_some() => {
                "Live structure model ready".to_string()
            }
            (PhraseProvenance::AudioInferred, false) => {
                "Learning this song's structure from live audio".to_string()
            }
            (PhraseProvenance::Unavailable, _) => "Phrase direction unavailable".to_string(),
            (_, true) => "Phrase source is stale; holding the current scene briefly".to_string(),
        };
        PhraseStatus {
            provenance: self.provenance,
            phrase: self.phrase.as_ref().map(|phrase| phrase.kind),
            confidence: self.phrase.as_ref().map(|phrase| phrase.confidence),
            progress: self.phrase_progress,
            stale,
            tempo_bpm: self.tempo_bpm,
            beat_confidence: self.beat_confidence,
            bar_phase: self.bar_phase,
            structure_model_ready: self.structure_signature.is_some(),
            message,
        }
    }
}

pub type SharedPhrase = Arc<RwLock<PlaybackContext>>;

pub trait PhraseProvider: Send {
    fn update(&mut self, now: Instant, frame: VisualInputFrame) -> PlaybackContext;
}

#[derive(Clone, Copy)]
struct Observation {
    energy: f32,
    onset: f32,
    bass: f32,
    mids: f32,
    highs: f32,
}

pub struct AudioInferredPhraseProvider {
    started_at: Instant,
    phrase_started_at: Instant,
    last_observation_at: Instant,
    current_kind: PhraseKind,
    candidate_kind: PhraseKind,
    candidate_since: Instant,
    phrase_index: u64,
    observations: VecDeque<Observation>,
    structure_signature: Option<u64>,
    last_bar_boundary_beat: Option<u64>,
    min_dwell: Duration,
    max_dwell: Duration,
}

impl AudioInferredPhraseProvider {
    pub fn new(now: Instant) -> Self {
        Self::with_dwell(now, Duration::from_secs(8), Duration::from_secs(28))
    }

    fn with_dwell(now: Instant, min_dwell: Duration, max_dwell: Duration) -> Self {
        Self {
            started_at: now,
            phrase_started_at: now,
            last_observation_at: now.checked_sub(OBSERVATION_INTERVAL).unwrap_or(now),
            current_kind: PhraseKind::Intro,
            candidate_kind: PhraseKind::Intro,
            candidate_since: now,
            phrase_index: 0,
            observations: VecDeque::with_capacity(MAX_OBSERVATIONS),
            structure_signature: None,
            last_bar_boundary_beat: None,
            min_dwell,
            max_dwell,
        }
    }
}

impl PhraseProvider for AudioInferredPhraseProvider {
    fn update(&mut self, now: Instant, frame: VisualInputFrame) -> PlaybackContext {
        if now.saturating_duration_since(self.last_observation_at) >= OBSERVATION_INTERVAL {
            if self.observations.len() == MAX_OBSERVATIONS {
                self.observations.pop_front();
            }
            self.observations.push_back(Observation {
                energy: frame.energy,
                onset: frame.onset,
                bass: frame.bass,
                mids: frame.mids,
                highs: frame.highs,
            });
            if self.structure_signature.is_none() && self.observations.len() >= 32 {
                self.structure_signature =
                    Some(structure_signature(&self.observations, frame.tempo_bpm));
            }
            self.last_observation_at = now;
        }

        let suggested = phrase_for_state(frame.state, self.phrase_index);
        if suggested != self.candidate_kind {
            self.candidate_kind = suggested;
            self.candidate_since = now;
        }
        let dwell = now.saturating_duration_since(self.phrase_started_at);
        let candidate_stable =
            now.saturating_duration_since(self.candidate_since) >= Duration::from_secs(2);
        let novelty = self.novelty_score();
        let impact_boundary = frame.impact > 0.82 && novelty > 0.22;
        let bar_boundary = frame.beat_confidence >= 0.42
            && frame.beat_index.is_multiple_of(4)
            && frame.beat_phase < 0.22
            && self.last_bar_boundary_beat != Some(frame.beat_index);
        if bar_boundary {
            self.last_bar_boundary_beat = Some(frame.beat_index);
        }
        let timed_boundary =
            dwell >= self.max_dwell && (bar_boundary || frame.beat_confidence < 0.42);
        let should_change = timed_boundary
            || (dwell >= self.min_dwell
                && suggested != self.current_kind
                && (impact_boundary || (candidate_stable && bar_boundary)));
        if should_change {
            self.current_kind = suggested;
            self.phrase_started_at = now;
            self.phrase_index = self.phrase_index.saturating_add(1);
        }

        let position_ms = now.saturating_duration_since(self.started_at).as_millis() as u64;
        let start_ms = self
            .phrase_started_at
            .saturating_duration_since(self.started_at)
            .as_millis() as u64;
        let phrase_dwell = now.saturating_duration_since(self.phrase_started_at);
        let progress = (phrase_dwell.as_secs_f32() / self.max_dwell.as_secs_f32()).clamp(0.0, 1.0);
        PlaybackContext {
            stable_track_id: None,
            structure_signature: self.structure_signature,
            position_ms: Some(position_ms),
            playing: frame.reactivity > 0.05,
            active_deck: None,
            tempo_bpm: (frame.beat_confidence >= 0.2).then_some(frame.tempo_bpm),
            beat_confidence: Some(frame.beat_confidence),
            bar_phase: Some(frame.bar_phase),
            phrase: Some(PhraseSegment {
                kind: self.current_kind,
                index: self.phrase_index,
                start_ms,
                end_ms: None,
                confidence: (0.48 + novelty * 0.32).clamp(0.35, 0.8),
            }),
            phrase_progress: Some(progress),
            provenance: PhraseProvenance::AudioInferred,
            updated_at: now,
        }
    }
}

impl AudioInferredPhraseProvider {
    fn novelty_score(&self) -> f32 {
        if self.observations.len() < 8 {
            return 0.0;
        }
        let midpoint = self.observations.len() / 2;
        let (older_energy, newer_energy, newer_onset) = self.observations.iter().enumerate().fold(
            (0.0, 0.0, 0.0),
            |(older, newer, onset), (index, observation)| {
                if index < midpoint {
                    (older + observation.energy, newer, onset)
                } else {
                    (older, newer + observation.energy, onset + observation.onset)
                }
            },
        );
        let older = older_energy / midpoint as f32;
        let newer_count = (self.observations.len() - midpoint) as f32;
        let newer = newer_energy / newer_count;
        ((newer - older).abs() * 1.6 + newer_onset / newer_count * 0.4).clamp(0.0, 1.0)
    }
}

fn structure_signature(observations: &VecDeque<Observation>, tempo_bpm: f32) -> u64 {
    let count = observations.len().max(1) as f32;
    let (energy, onset, bass, mids, highs) = observations.iter().fold(
        (0.0, 0.0, 0.0, 0.0, 0.0),
        |(energy, onset, bass, mids, highs), observation| {
            (
                energy + observation.energy,
                onset + observation.onset,
                bass + observation.bass,
                mids + observation.mids,
                highs + observation.highs,
            )
        },
    );
    let values = [
        energy / count,
        onset / count,
        bass / count,
        mids / count,
        highs / count,
        (tempo_bpm / 200.0).clamp(0.0, 1.0),
    ];
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for value in values {
        let quantized = (value.clamp(0.0, 1.0) * 31.0).round() as u8;
        hash ^= u64::from(quantized);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn phrase_for_state(state: MusicState, phrase_index: u64) -> PhraseKind {
    match state {
        MusicState::Quiet => {
            if phrase_index == 0 {
                PhraseKind::Intro
            } else {
                PhraseKind::Down
            }
        }
        MusicState::Flow | MusicState::Groove => PhraseKind::Verse,
        MusicState::Build => PhraseKind::Up,
        MusicState::Impact | MusicState::Peak => PhraseKind::Chorus,
        MusicState::Breakdown => {
            if phrase_index.is_multiple_of(3) {
                PhraseKind::Bridge
            } else {
                PhraseKind::Down
            }
        }
    }
}

#[allow(dead_code)]
pub struct PhraseRouter;

#[allow(dead_code)]
impl PhraseRouter {
    pub fn select(
        external: Option<&PlaybackContext>,
        inferred: &PlaybackContext,
        now: Instant,
    ) -> PlaybackContext {
        if let Some(external) = external {
            if external.provenance != PhraseProvenance::Unavailable
                && now.saturating_duration_since(external.updated_at) <= HOLD_METADATA
            {
                return external.clone();
            }
        }
        inferred.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inferred_provider_uses_bounded_history_and_dwell() {
        let start = Instant::now();
        let mut provider = AudioInferredPhraseProvider::with_dwell(
            start,
            Duration::from_secs(2),
            Duration::from_secs(8),
        );
        let build = VisualInputFrame {
            state: MusicState::Build,
            energy: 0.8,
            onset: 0.7,
            reactivity: 1.0,
            beat_confidence: 0.8,
            beat_index: 3,
            beat_phase: 0.5,
            ..Default::default()
        };
        let early = provider.update(start + Duration::from_secs(1), build);
        assert_eq!(early.phrase.unwrap().kind, PhraseKind::Intro);
        let changed = provider.update(
            start + Duration::from_secs(3),
            VisualInputFrame {
                beat_index: 4,
                beat_phase: 0.05,
                ..build
            },
        );
        assert_eq!(changed.phrase.unwrap().kind, PhraseKind::Up);
        for index in 0..400 {
            let _ = provider.update(start + Duration::from_millis(4_000 + index * 250), build);
        }
        assert_eq!(provider.observations.len(), MAX_OBSERVATIONS);
    }

    #[test]
    fn structure_model_locks_without_claiming_a_rekordbox_track_identity() {
        let start = Instant::now();
        let mut provider = AudioInferredPhraseProvider::new(start);
        let frame = VisualInputFrame {
            energy: 0.7,
            bass: 0.8,
            mids: 0.45,
            highs: 0.3,
            onset: 0.5,
            tempo_bpm: 124.0,
            beat_confidence: 0.8,
            reactivity: 1.0,
            ..Default::default()
        };
        let mut context = PlaybackContext::default();
        for index in 0..40 {
            context = provider.update(start + Duration::from_millis(index * 250), frame);
        }
        assert!(context.stable_track_id.is_none());
        assert!(context.structure_signature.is_some());
        assert!(
            context
                .status(start + Duration::from_secs(10))
                .structure_model_ready
        );
    }

    #[test]
    fn stale_external_metadata_degrades_to_inferred() {
        let now = Instant::now();
        let external = PlaybackContext {
            provenance: PhraseProvenance::Rekordbox,
            updated_at: now - Duration::from_secs(8),
            ..Default::default()
        };
        let inferred = PlaybackContext {
            provenance: PhraseProvenance::AudioInferred,
            ..Default::default()
        };
        assert_eq!(
            PhraseRouter::select(Some(&external), &inferred, now).provenance,
            PhraseProvenance::AudioInferred
        );
    }
}
