use std::collections::VecDeque;

const MAX_INTERVALS: usize = 24;
const MIN_BEAT_INTERVAL: f32 = 60.0 / 180.0;
const MAX_BEAT_INTERVAL: f32 = 60.0 / 70.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct RhythmEstimate {
    pub beat_phase: f32,
    pub beat_strength: f32,
    pub tempo_bpm: f32,
    pub beat_confidence: f32,
    pub beat_index: u64,
    pub bar_phase: f32,
}

pub struct RhythmTracker {
    last_onset_seconds: Option<f32>,
    intervals: VecDeque<f32>,
    estimated_interval: f32,
    beat_origin_seconds: Option<f32>,
    confidence: f32,
}

impl RhythmTracker {
    pub fn new() -> Self {
        Self {
            last_onset_seconds: None,
            intervals: VecDeque::with_capacity(MAX_INTERVALS),
            estimated_interval: 0.5,
            beat_origin_seconds: None,
            confidence: 0.0,
        }
    }

    pub fn update(&mut self, seconds: f32, onset: f32) -> RhythmEstimate {
        let since_onset = self
            .last_onset_seconds
            .map(|last| seconds - last)
            .unwrap_or(f32::MAX);
        if onset > 0.62 && since_onset > 0.2 {
            if since_onset.is_finite() {
                let interval = normalize_interval(since_onset);
                if (MIN_BEAT_INTERVAL..=MAX_BEAT_INTERVAL).contains(&interval) {
                    if self.intervals.len() == MAX_INTERVALS {
                        self.intervals.pop_front();
                    }
                    self.intervals.push_back(interval);
                    self.update_tempo();
                }
            }
            self.align_origin(seconds);
            self.last_onset_seconds = Some(seconds);
        } else if since_onset > 3.0 {
            self.confidence *= 0.995;
        }

        let origin = self.beat_origin_seconds.unwrap_or(seconds);
        let beat_position = ((seconds - origin) / self.estimated_interval.max(0.001)).max(0.0);
        let beat_index = beat_position.floor() as u64;
        let beat_phase = beat_position.fract();
        let phase_pulse = (-beat_phase * 7.2).exp() * self.confidence;
        let onset_pulse = if onset > 0.62 { onset } else { 0.0 };
        RhythmEstimate {
            beat_phase,
            beat_strength: phase_pulse.max(onset_pulse).clamp(0.0, 1.0),
            tempo_bpm: 60.0 / self.estimated_interval.max(0.001),
            beat_confidence: self.confidence.clamp(0.0, 1.0),
            beat_index,
            bar_phase: ((beat_index % 4) as f32 + beat_phase) / 4.0,
        }
    }

    fn update_tempo(&mut self) {
        let mut sorted = self.intervals.iter().copied().collect::<Vec<_>>();
        sorted.sort_by(f32::total_cmp);
        let median = sorted[sorted.len() / 2];
        let inliers = sorted
            .iter()
            .filter(|interval| ((**interval - median) / median).abs() <= 0.12)
            .count();
        let consistency = inliers as f32 / sorted.len() as f32;
        let evidence = (sorted.len() as f32 / 8.0).clamp(0.0, 1.0);
        let target_confidence = consistency * evidence;
        self.confidence += (target_confidence - self.confidence) * 0.35;
        self.estimated_interval += (median - self.estimated_interval) * 0.28;
    }

    fn align_origin(&mut self, seconds: f32) {
        let Some(origin) = self.beat_origin_seconds else {
            self.beat_origin_seconds = Some(seconds);
            return;
        };
        let beat = ((seconds - origin) / self.estimated_interval.max(0.001)).round();
        let predicted = origin + beat * self.estimated_interval;
        let error = seconds - predicted;
        if error.abs() <= self.estimated_interval * 0.3 {
            self.beat_origin_seconds = Some(origin + error * 0.2);
        }
    }
}

fn normalize_interval(mut interval: f32) -> f32 {
    while interval < MIN_BEAT_INTERVAL {
        interval *= 2.0;
    }
    while interval > MAX_BEAT_INTERVAL {
        interval *= 0.5;
    }
    interval
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locks_to_a_regular_120_bpm_pulse() {
        let mut tracker = RhythmTracker::new();
        let mut estimate = RhythmEstimate::default();
        for frame in 0..500 {
            let seconds = frame as f32 * 0.02;
            let onset = if frame % 25 == 0 { 0.95 } else { 0.05 };
            estimate = tracker.update(seconds, onset);
        }
        assert!((estimate.tempo_bpm - 120.0).abs() < 1.0);
        assert!(estimate.beat_confidence > 0.8);
        assert!(estimate.beat_index >= 18);
        assert!((0.0..1.0).contains(&estimate.bar_phase));
    }

    #[test]
    fn normalizes_fast_subdivisions_into_a_dance_tempo() {
        assert!((normalize_interval(0.25) - 0.5).abs() < 0.001);
        assert!((normalize_interval(1.0) - 0.5).abs() < 0.001);
    }

    #[test]
    fn interval_history_is_bounded() {
        let mut tracker = RhythmTracker::new();
        for index in 0..1_000 {
            let _ = tracker.update(index as f32 * 0.5, 1.0);
        }
        assert_eq!(tracker.intervals.len(), MAX_INTERVALS);
    }
}
