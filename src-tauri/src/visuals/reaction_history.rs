/// A little over one second of independent reactive lanes, sampled at 30 Hz.
/// Fixed-time sampling makes propagation independent of presentation frame rate.
const PERIOD: f32 = 1.0 / 30.0;
#[derive(Default)]
pub(super) struct ReactionHistory {
    past: [[f32; 4]; 32],
    current: [f32; 4],
    phase: f32,
}
impl ReactionHistory {
    pub fn update(&mut self, value: [f32; 4], delta: f32) {
        if !delta.is_finite() || delta <= 0.0 {
            return;
        }
        let value = value.map(|v| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        });
        if delta > 1.0 {
            self.past.fill(value);
            self.current = value;
            self.phase = 0.0;
            return;
        }
        let mut boundary = PERIOD - self.phase;
        while boundary <= delta + 0.000001 {
            let fraction = (boundary / delta).clamp(0.0, 1.0);
            let sample = std::array::from_fn(|lane| {
                self.current[lane] + (value[lane] - self.current[lane]) * fraction
            });
            self.past.copy_within(0..31, 1);
            self.past[0] = sample;
            boundary += PERIOD;
        }
        self.phase = (self.phase + delta) % PERIOD;
        if self.phase > PERIOD - 0.000001 {
            self.phase = 0.0;
        }
        self.current = value;
    }
    pub fn snapshot(&self) -> [[f32; 4]; 32] {
        let mut result = [[0.0; 4]; 32];
        result[0] = self.current;
        result[1..].copy_from_slice(&self.past[..31]);
        result
    }
    pub fn fraction_seconds(&self) -> f32 {
        self.phase
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_hit_travels_into_history_then_expires_without_leaking_between_bands() {
        let mut history = ReactionHistory::default();
        history.update([1.0, 0.0, 0.0, 0.0], PERIOD);
        for _ in 0..5 {
            history.update([0.0; 4], PERIOD);
        }
        assert_eq!(history.snapshot()[0], [0.0; 4]);
        assert!(history.snapshot()[6][0] > 0.99);
        assert!(history
            .snapshot()
            .iter()
            .all(|sample| sample[1..] == [0.0; 3]));
        for _ in 0..35 {
            history.update([0.0; 4], PERIOD);
        }
        assert_eq!(history.snapshot(), [[0.0; 4]; 32]);
    }
    #[test]
    fn frame_rate_does_not_change_the_sampled_ramp() {
        fn ramp(hz: usize) -> [[f32; 4]; 32] {
            let mut history = ReactionHistory::default();
            for frame in 1..=hz {
                history.update([frame as f32 / hz as f32; 4], 1.0 / hz as f32);
            }
            history.snapshot()
        }
        let a = ramp(30);
        let b = ramp(120);
        for (left, right) in a.iter().zip(b) {
            assert!((left[0] - right[0]).abs() < 0.001);
        }
    }
    #[test]
    fn silence_and_invalid_values_cannot_generate_motion() {
        let mut history = ReactionHistory::default();
        history.update([0.0; 4], 0.016);
        history.update([f32::NAN, -1.0, f32::NEG_INFINITY, 0.0], 0.1);
        assert_eq!(history.snapshot(), [[0.0; 4]; 32]);
    }
}
