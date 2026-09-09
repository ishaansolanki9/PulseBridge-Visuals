/// Sustained missed presentation deadlines lower detail/resolution. Recovery is
/// deliberately much slower, so a single drop or resize does not cause pumping.
#[derive(Default)]
pub(super) struct AdaptiveQuality {
    tier: u8,
    slow_seconds: f32,
    stable_seconds: f32,
}
impl AdaptiveQuality {
    pub fn observe(&mut self, seconds: f32) {
        if !seconds.is_finite() || !(0.001..1.0).contains(&seconds) {
            return;
        }
        if seconds > 0.019 {
            self.slow_seconds += seconds.min(0.1);
            self.stable_seconds = 0.0;
        } else {
            self.slow_seconds = (self.slow_seconds - seconds * 0.5).max(0.0);
            if seconds < 0.018 {
                self.stable_seconds += seconds;
            }
        }
        if self.slow_seconds >= 1.5 && self.tier < 2 {
            self.tier += 1;
            self.slow_seconds = 0.0;
            self.stable_seconds = 0.0;
        } else if self.stable_seconds >= 20.0 && self.tier > 0 {
            self.tier -= 1;
            self.stable_seconds = 0.0;
        }
    }
    pub fn detail(&self, minimum_tier: u8) -> f32 {
        match self.tier.max(minimum_tier) {
            0 => 1.0,
            1 => 0.5,
            _ => 0.0,
        }
    }
    pub fn render_size(&self, width: u32, height: u32, minimum_tier: u8) -> (u32, u32) {
        let (width, height) = super::renderer::performance_render_size(width, height);
        let scale = match self.tier.max(minimum_tier) {
            0 => 1.0,
            1 => 2.0 / 3.0,
            _ => 0.5,
        };
        (
            (width as f32 * scale).round().max(1.0) as u32,
            (height as f32 * scale).round().max(1.0) as u32,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sustained_overload_falls_back_and_recovers_slowly() {
        let mut quality = AdaptiveQuality::default();
        quality.observe(0.080);
        assert_eq!(quality.detail(0), 1.0);
        for _ in 0..120 {
            quality.observe(0.03);
        }
        assert_eq!(quality.render_size(3840, 2160, 0), (960, 540));
        for _ in 0..600 {
            quality.observe(1.0 / 60.0);
        }
        assert_eq!(quality.detail(0), 0.0);
        for _ in 0..650 {
            quality.observe(1.0 / 60.0);
        }
        assert_eq!(quality.detail(0), 0.5);
        assert_eq!(quality.render_size(3840, 2160, 0), (1280, 720));
    }
    #[test]
    fn transitions_bound_work_and_invalid_timing_cannot_change_quality() {
        let mut quality = AdaptiveQuality::default();
        for value in [f32::NAN, f32::INFINITY, -1.0, 10.0] {
            quality.observe(value);
        }
        assert_eq!(quality.detail(1), 0.5);
        assert_eq!(quality.detail(2), 0.0);
        assert_eq!(quality.render_size(3840, 2160, 2), (960, 540));
        assert_eq!(quality.render_size(3840, 2160, 1), (1280, 720));
        assert_eq!(quality.render_size(1920, 1080, 0), (1920, 1080));
        assert_eq!(quality.render_size(0, 0, 0), (1, 1));
    }
}
