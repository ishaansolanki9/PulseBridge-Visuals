use std::collections::VecDeque;

pub struct RollingNormalizer {
    history: VecDeque<f32>,
    capacity: usize,
    low: f32,
    median: f32,
    high: f32,
    frames_since_refresh: u8,
}

impl RollingNormalizer {
    pub fn new(capacity: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(capacity),
            capacity,
            low: 0.0,
            median: 0.05,
            high: 0.1,
            frames_since_refresh: 0,
        }
    }

    pub fn normalize(&mut self, value: f32) -> f32 {
        if self.history.len() == self.capacity {
            self.history.pop_front();
        }
        self.history.push_back(value.max(0.0));
        self.frames_since_refresh += 1;
        if self.frames_since_refresh >= 10 || self.history.len() < 20 {
            self.refresh();
            self.frames_since_refresh = 0;
        }
        let floor = self.low * 0.72 + self.median * 0.28;
        ((value - floor) / (self.high - floor).max(0.000_01)).clamp(0.0, 1.0)
    }

    fn refresh(&mut self) {
        let mut values = self.history.iter().copied().collect::<Vec<_>>();
        values.sort_by(f32::total_cmp);
        if values.is_empty() {
            return;
        }
        self.low = percentile(&values, 0.1);
        self.median = percentile(&values, 0.5);
        self.high = percentile(&values, 0.92).max(self.median * 1.08 + 0.000_01);
    }
}

fn percentile(values: &[f32], fraction: f32) -> f32 {
    let index = ((values.len() - 1) as f32 * fraction).round() as usize;
    values[index.min(values.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_is_relative_to_recent_material() {
        let mut normalizer = RollingNormalizer::new(128);
        for index in 0..100 {
            normalizer.normalize(0.01 + index as f32 * 0.000_1);
        }
        let quiet_track_peak = normalizer.normalize(0.03);
        assert!(quiet_track_peak > 0.8);
    }
}
