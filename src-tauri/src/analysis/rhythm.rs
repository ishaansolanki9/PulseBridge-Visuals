use std::collections::VecDeque;

pub struct RhythmTracker {
    last_beat_seconds: Option<f32>,
    intervals: VecDeque<f32>,
    estimated_interval: f32,
}

impl RhythmTracker {
    pub fn new() -> Self {
        Self {
            last_beat_seconds: None,
            intervals: VecDeque::with_capacity(12),
            estimated_interval: 0.5,
        }
    }

    pub fn update(&mut self, seconds: f32, onset: f32) -> (f32, f32) {
        let since_last = self
            .last_beat_seconds
            .map(|last| seconds - last)
            .unwrap_or(f32::MAX);
        if onset > 0.68 && since_last > 0.24 {
            if (0.24..=1.1).contains(&since_last) {
                if self.intervals.len() == 12 {
                    self.intervals.pop_front();
                }
                self.intervals.push_back(since_last);
                let mut intervals = self.intervals.iter().copied().collect::<Vec<_>>();
                intervals.sort_by(f32::total_cmp);
                self.estimated_interval = intervals[intervals.len() / 2];
            }
            self.last_beat_seconds = Some(seconds);
        }

        let elapsed = self
            .last_beat_seconds
            .map(|last| seconds - last)
            .unwrap_or(10.0);
        let phase = (elapsed / self.estimated_interval.max(0.24)).fract();
        let pulse = (-elapsed.max(0.0) * 7.2).exp() * onset.max(0.45);
        (phase, pulse.clamp(0.0, 1.0))
    }
}
