use std::time::Instant;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MusicState {
    #[default]
    Quiet,
    Flow,
    Groove,
    Build,
    Impact,
    Peak,
    Breakdown,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FeatureFrame {
    pub timestamp_ms: u64,
    pub loudness: f32,
    pub sub_energy: f32,
    pub bass_energy: f32,
    pub mid_energy: f32,
    pub high_energy: f32,
    pub spectral_flux: f32,
    pub onset_strength: f32,
    pub beat_strength: f32,
    pub beat_phase: f32,
    pub tempo_bpm: f32,
    pub beat_confidence: f32,
    pub beat_index: u64,
    pub bar_phase: f32,
    pub energy_fast: f32,
    pub energy_slow: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct VisualInputFrame {
    pub state: MusicState,
    pub energy: f32,
    pub sub: f32,
    pub bass: f32,
    pub mids: f32,
    pub highs: f32,
    pub beat_phase: f32,
    pub beat_pulse: f32,
    pub tempo_bpm: f32,
    pub beat_confidence: f32,
    pub beat_index: u64,
    pub bar_phase: f32,
    pub onset: f32,
    pub impact: f32,
    pub reactivity: f32,
}

impl Default for VisualInputFrame {
    fn default() -> Self {
        Self {
            state: MusicState::Quiet,
            energy: 0.16,
            sub: 0.12,
            bass: 0.14,
            mids: 0.18,
            highs: 0.08,
            beat_phase: 0.0,
            beat_pulse: 0.0,
            tempo_bpm: 0.0,
            beat_confidence: 0.0,
            beat_index: 0,
            bar_phase: 0.0,
            onset: 0.0,
            impact: 0.0,
            reactivity: 0.0,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AnalysisSnapshot {
    pub frame: VisualInputFrame,
    pub last_audio_at: Option<Instant>,
    pub feature_frames: usize,
}

impl AnalysisSnapshot {
    pub fn visual_input(&self, now: Instant) -> VisualInputFrame {
        let reactivity = self
            .last_audio_at
            .map(|last| {
                let age = now.saturating_duration_since(last).as_secs_f32();
                if age <= 0.3 {
                    1.0
                } else if age < 2.0 {
                    1.0 - (age - 0.3) / 1.7
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let ambient = VisualInputFrame::default();
        let mut output = self.frame;
        output.energy = mix(ambient.energy, output.energy, reactivity);
        output.sub = mix(ambient.sub, output.sub, reactivity);
        output.bass = mix(ambient.bass, output.bass, reactivity);
        output.mids = mix(ambient.mids, output.mids, reactivity);
        output.highs = mix(ambient.highs, output.highs, reactivity);
        output.beat_pulse *= reactivity;
        output.beat_confidence *= reactivity;
        output.onset *= reactivity;
        output.impact *= reactivity;
        output.reactivity = reactivity;
        if reactivity < 0.05 {
            output.state = MusicState::Quiet;
        }
        output
    }

    pub fn audio_age_ms(&self, now: Instant) -> Option<u64> {
        self.last_audio_at
            .map(|last| now.saturating_duration_since(last).as_millis() as u64)
    }
}

fn mix(start: f32, end: f32, amount: f32) -> f32 {
    start + (end - start) * amount
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn missing_audio_fades_to_ambient_without_freezing() {
        let now = Instant::now();
        let snapshot = AnalysisSnapshot {
            frame: VisualInputFrame {
                state: MusicState::Peak,
                energy: 1.0,
                impact: 1.0,
                ..Default::default()
            },
            last_audio_at: Some(now - Duration::from_secs(3)),
            feature_frames: 12,
        };
        let output = snapshot.visual_input(now);
        assert_eq!(output.state, MusicState::Quiet);
        assert!((output.energy - VisualInputFrame::default().energy).abs() < 0.001);
        assert_eq!(output.impact, 0.0);
    }
}
