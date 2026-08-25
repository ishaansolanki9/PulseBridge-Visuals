use std::collections::VecDeque;

use super::{FeatureFrame, MusicState};

const HISTORY_FRAMES: usize = 600;
const RECENT_FRAMES: usize = 75;

#[derive(Clone, Copy)]
struct TrendPoint {
    energy: f32,
    highs: f32,
    onset: f32,
}

pub struct MusicalStateTracker {
    current: MusicState,
    candidate: MusicState,
    candidate_since: f32,
    impact_until: f32,
    last_impact: f32,
    previous_energy: f32,
    previous_bass: f32,
    history: VecDeque<TrendPoint>,
}

impl MusicalStateTracker {
    pub fn new() -> Self {
        Self {
            current: MusicState::Quiet,
            candidate: MusicState::Quiet,
            candidate_since: 0.0,
            impact_until: 0.0,
            last_impact: -10.0,
            previous_energy: 0.0,
            previous_bass: 0.0,
            history: VecDeque::with_capacity(HISTORY_FRAMES),
        }
    }

    pub fn update(&mut self, seconds: f32, frame: &FeatureFrame) -> (MusicState, f32) {
        if self.history.len() == HISTORY_FRAMES {
            self.history.pop_front();
        }
        self.history.push_back(TrendPoint {
            energy: frame.energy_fast,
            highs: frame.high_energy,
            onset: frame.onset_strength,
        });
        let (history_trend, high_trend, onset_density) = self.history_context(frame);
        let energy_jump = (frame.energy_fast - self.previous_energy).max(0.0);
        let bass_jump = (frame.bass_energy - self.previous_bass).max(0.0);
        let build_context = if self.current == MusicState::Build || history_trend > 0.08 {
            1.0
        } else {
            0.0
        };
        let impact_score = (frame.onset_strength * 0.3
            + frame.spectral_flux * 0.04
            + energy_jump * 0.25
            + bass_jump * 0.2
            + frame.beat_strength * 0.11
            + build_context * 0.18)
            .clamp(0.0, 1.0);
        self.previous_energy = frame.energy_fast;
        self.previous_bass = frame.bass_energy;

        if impact_score > 0.72 && seconds - self.last_impact > 1.25 {
            self.current = MusicState::Impact;
            self.impact_until = seconds + 0.42;
            self.last_impact = seconds;
            return (self.current, impact_score);
        }
        if seconds < self.impact_until {
            let release = ((self.impact_until - seconds) / 0.42).clamp(0.0, 1.0);
            return (MusicState::Impact, impact_score.max(release));
        }

        let trend = if self.history.len() >= RECENT_FRAMES * 2 {
            history_trend
        } else {
            frame.energy_fast - frame.energy_slow
        };
        let desired = if frame.loudness < 0.000_01
            || (frame.energy_fast < 0.13 && frame.onset_strength < 0.18)
        {
            MusicState::Quiet
        } else if trend < -0.13 && frame.energy_slow > 0.5 {
            MusicState::Breakdown
        } else if trend > 0.08
            && (high_trend > 0.035
                || frame.high_energy > 0.42
                || frame.onset_strength > 0.38
                || onset_density > 0.11)
        {
            MusicState::Build
        } else if frame.energy_fast > 0.72 && frame.beat_strength > 0.18 {
            MusicState::Peak
        } else if frame.energy_fast > 0.4 && frame.beat_strength > 0.12 {
            MusicState::Groove
        } else {
            MusicState::Flow
        };

        if desired != self.candidate {
            self.candidate = desired;
            self.candidate_since = seconds;
        }
        let dwell = match desired {
            MusicState::Quiet => 0.55,
            MusicState::Build => 0.9,
            MusicState::Breakdown => 0.65,
            MusicState::Peak => 0.55,
            _ => 0.8,
        };
        if desired != self.current && seconds - self.candidate_since >= dwell {
            self.current = desired;
        }
        (self.current, 0.0)
    }

    fn history_context(&self, frame: &FeatureFrame) -> (f32, f32, f32) {
        if self.history.len() < RECENT_FRAMES * 2 {
            return (
                frame.energy_fast - frame.energy_slow,
                frame.high_energy - 0.35,
                0.0,
            );
        }
        let recent_start = self.history.len() - RECENT_FRAMES;
        let mut baseline_energy = 0.0;
        let mut baseline_highs = 0.0;
        let mut baseline_count = 0.0;
        let mut recent_energy = 0.0;
        let mut recent_highs = 0.0;
        let mut strong_onsets = 0.0;
        for (index, point) in self.history.iter().enumerate() {
            if index >= recent_start {
                recent_energy += point.energy;
                recent_highs += point.highs;
                strong_onsets += (point.onset > 0.4) as u8 as f32;
            } else {
                baseline_energy += point.energy;
                baseline_highs += point.highs;
                baseline_count += 1.0;
            }
        }
        let recent_count = RECENT_FRAMES as f32;
        (
            recent_energy / recent_count - baseline_energy / baseline_count,
            recent_highs / recent_count - baseline_highs / baseline_count,
            strong_onsets / recent_count,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(energy: f32, slow: f32, high: f32, onset: f32) -> FeatureFrame {
        FeatureFrame {
            loudness: 0.1,
            energy_fast: energy,
            energy_slow: slow,
            high_energy: high,
            onset_strength: onset,
            bass_energy: energy,
            beat_strength: onset,
            ..Default::default()
        }
    }

    #[test]
    fn state_changes_require_dwell_time() {
        let mut tracker = MusicalStateTracker::new();
        assert_eq!(
            tracker.update(0.0, &frame(0.5, 0.48, 0.3, 0.2)).0,
            MusicState::Quiet
        );
        assert_eq!(
            tracker.update(0.4, &frame(0.5, 0.48, 0.3, 0.2)).0,
            MusicState::Quiet
        );
        assert_eq!(
            tracker.update(0.9, &frame(0.5, 0.48, 0.3, 0.2)).0,
            MusicState::Groove
        );
    }

    #[test]
    fn impact_has_a_cooldown() {
        let mut tracker = MusicalStateTracker::new();
        tracker.current = MusicState::Build;
        let impact = frame(1.0, 0.4, 0.9, 1.0);
        assert_eq!(tracker.update(2.0, &impact).0, MusicState::Impact);
        tracker.impact_until = 0.0;
        let (_, score) = tracker.update(2.5, &impact);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn musical_history_is_bounded() {
        let mut tracker = MusicalStateTracker::new();
        for index in 0..1_000 {
            tracker.update(index as f32 * 0.02, &frame(0.4, 0.38, 0.3, 0.2));
        }
        assert_eq!(tracker.history.len(), HISTORY_FRAMES);
    }
}
