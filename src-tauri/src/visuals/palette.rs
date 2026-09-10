use crate::analysis::MusicState;

use super::PaletteName;

pub type Palette = [[f32; 4]; 4];

pub fn palette_for(name: PaletteName, state: MusicState) -> Palette {
    let resolved = match name {
        PaletteName::Auto => match state {
            MusicState::Quiet | MusicState::Flow | MusicState::Breakdown => PaletteName::Ocean,
            MusicState::Groove => PaletteName::Electric,
            MusicState::Build => PaletteName::Sunset,
            MusicState::Impact | MusicState::Peak => PaletteName::Neon,
        },
        selected => selected,
    };

    match resolved {
        PaletteName::Auto => unreachable!("auto palettes are resolved by musical state"),
        PaletteName::Electric => [
            [0.01, 0.08, 0.16, 1.0],
            [0.0, 0.95, 0.88, 1.0],
            [0.14, 0.24, 1.0, 1.0],
            [0.74, 0.05, 1.0, 1.0],
        ],
        PaletteName::Neon => [
            [0.04, 0.0, 0.12, 1.0],
            [1.0, 0.02, 0.5, 1.0],
            [0.08, 0.96, 0.86, 1.0],
            [0.58, 0.1, 1.0, 1.0],
        ],
        PaletteName::Sunset => [
            [0.12, 0.01, 0.12, 1.0],
            [1.0, 0.12, 0.08, 1.0],
            [1.0, 0.62, 0.02, 1.0],
            [0.8, 0.04, 0.48, 1.0],
        ],
        PaletteName::Ocean => [
            [0.0, 0.03, 0.1, 1.0],
            [0.0, 0.28, 0.68, 1.0],
            [0.0, 0.8, 0.78, 1.0],
            [0.22, 0.08, 0.72, 1.0],
        ],
        PaletteName::Infrared => [
            [0.06, 0.0, 0.0, 1.0],
            [0.68, 0.0, 0.02, 1.0],
            [1.0, 0.16, 0.0, 1.0],
            [1.0, 0.62, 0.04, 1.0],
        ],
        PaletteName::PurpleBlue => [
            [0.015, 0.0, 0.09, 1.0],
            [0.18, 0.08, 0.82, 1.0],
            [0.48, 0.12, 1.0, 1.0],
            [0.02, 0.5, 1.0, 1.0],
        ],
        PaletteName::Warm => [
            [0.09, 0.015, 0.0, 1.0],
            [0.82, 0.08, 0.01, 1.0],
            [1.0, 0.38, 0.02, 1.0],
            [1.0, 0.78, 0.18, 1.0],
        ],
        PaletteName::Monochrome => [
            [0.005, 0.008, 0.012, 1.0],
            [0.08, 0.1, 0.13, 1.0],
            [0.5, 0.56, 0.62, 1.0],
            [0.92, 0.96, 1.0, 1.0],
        ],
        PaletteName::RainbowFlow => [
            [0.08, 0.0, 0.18, 1.0],
            [0.95, 0.05, 0.45, 1.0],
            [0.02, 0.86, 0.75, 1.0],
            [0.98, 0.62, 0.03, 1.0],
        ],
    }
}

pub fn smooth_palette(current: &mut Palette, target: Palette, delta_seconds: f32) {
    let amount = 1.0 - (-delta_seconds / 0.85).exp();
    for (current_color, target_color) in current.iter_mut().zip(target) {
        for (current_channel, target_channel) in current_color.iter_mut().zip(target_color) {
            *current_channel += (target_channel - *current_channel) * amount;
        }
    }
}

/// Integrating speed avoids hue jumps when energy or the color control changes.
#[derive(Default)]
pub(super) struct ColorMotion {
    phase: f32,
}
impl ColorMotion {
    pub fn update(&mut self, delta: f32, drive: f32, onset: f32, amount: f32) -> f32 {
        if delta.is_finite() && delta > 0.0 {
            let speed = amount.clamp(0.0, 1.5)
                * (0.055 + drive.clamp(0.0, 1.0) * 0.17 + onset.clamp(0.0, 1.0) * 0.08);
            self.phase = (self.phase + delta * speed).rem_euclid(1.0);
        }
        self.phase
    }
}

#[cfg(test)]
fn cyclic_palette_sample(palette: Palette, value: f32) -> [f32; 4] {
    let wrapped = value.rem_euclid(1.0);
    let scaled = wrapped * 4.0;
    let segment = scaled.floor() as usize;
    let local = scaled.fract();
    let eased = local * local * (3.0 - 2.0 * local);
    let start = palette[segment.min(3)];
    let end = palette[(segment + 1) % 4];
    std::array::from_fn(|channel| start[channel] + (end[channel] - start[channel]) * eased)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_phase_is_continuous_and_zero_control_freezes_it() {
        let mut clock = ColorMotion::default();
        let first = clock.update(1.0, 0.5, 0.0, 1.0);
        let next = clock.update(1.0 / 60.0, 1.0, 1.0, 1.5);
        assert!(next > first && next - first < 0.01);
        assert_eq!(clock.update(4.0, 1.0, 1.0, 0.0), next);
        let mut slow = ColorMotion::default();
        let mut fast = ColorMotion::default();
        for _ in 0..30 {
            slow.update(1.0 / 30.0, 0.7, 0.1, 1.0);
        }
        for _ in 0..120 {
            fast.update(1.0 / 120.0, 0.7, 0.1, 1.0);
        }
        assert!((slow.phase - fast.phase).abs() < 0.00001);
    }

    #[test]
    fn automatic_palette_changes_with_musical_state() {
        assert_ne!(
            palette_for(PaletteName::Auto, MusicState::Flow),
            palette_for(PaletteName::Auto, MusicState::Peak)
        );
    }

    #[test]
    fn palette_changes_are_interpolated() {
        let mut current = palette_for(PaletteName::Ocean, MusicState::Flow);
        let target = palette_for(PaletteName::Sunset, MusicState::Build);
        let starting_channel = current[1][0];
        smooth_palette(&mut current, target, 0.1);
        assert!(current[1][0] > starting_channel);
        assert!(current[1][0] < target[1][0]);
    }

    #[test]
    fn cyclic_palette_is_continuous_across_angle_wrap() {
        let palette = palette_for(PaletteName::Neon, MusicState::Peak);
        let left = cyclic_palette_sample(palette, -0.000_001);
        let right = cyclic_palette_sample(palette, 0.000_001);
        for channel in 0..3 {
            assert!((left[channel] - right[channel]).abs() < 0.0001);
        }
        assert_eq!(cyclic_palette_sample(palette, 0.0), palette[0]);
        assert_eq!(cyclic_palette_sample(palette, 1.0), palette[0]);
    }
}
