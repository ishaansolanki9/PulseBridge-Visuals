use serde::{Deserialize, Serialize};

use crate::analysis::{MusicState, VisualInputFrame};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VisualStyle {
    #[default]
    Auto,
    Fluid,
    Waves,
    Pulse,
    Tunnel,
    Burst,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IntensityProfile {
    Chill,
    #[default]
    Balanced,
    Wild,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FlashProfile {
    #[default]
    Off,
    Moderate,
    High,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PaletteName {
    #[default]
    Auto,
    Electric,
    Neon,
    Sunset,
    Ocean,
    Infrared,
    PurpleBlue,
    Warm,
    Monochrome,
    RainbowFlow,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VisualSettings {
    pub display_id: usize,
    pub audio_source_id: String,
    pub pcm_buffer_seconds: u8,
    pub style: VisualStyle,
    pub intensity: IntensityProfile,
    pub palette: PaletteName,
    pub flash: FlashProfile,
    pub topmost: bool,
    pub music_reactivity: f32,
    pub motion: f32,
    pub brightness: f32,
    pub color_change: f32,
    pub flash_strength: f32,
}

impl Default for VisualSettings {
    fn default() -> Self {
        Self {
            display_id: 0,
            audio_source_id: "process:auto".to_string(),
            pcm_buffer_seconds: 10,
            style: VisualStyle::Auto,
            intensity: IntensityProfile::Balanced,
            palette: PaletteName::Auto,
            flash: FlashProfile::Off,
            topmost: false,
            music_reactivity: 1.0,
            motion: 1.0,
            brightness: 1.0,
            color_change: 1.0,
            flash_strength: 1.0,
        }
    }
}

impl VisualSettings {
    pub fn sanitized(mut self) -> Self {
        self.pcm_buffer_seconds = self.pcm_buffer_seconds.clamp(5, 30);
        self.music_reactivity = self.music_reactivity.clamp(0.0, 1.5);
        self.motion = self.motion.clamp(0.25, 1.75);
        self.brightness = self.brightness.clamp(0.25, 1.25);
        self.color_change = self.color_change.clamp(0.0, 1.5);
        self.flash_strength = self.flash_strength.clamp(0.0, 1.0);
        if self.audio_source_id.is_empty() {
            self.audio_source_id = "process:auto".to_string();
        }
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SmoothedVisualState {
    pub energy: f32,
    pub sub: f32,
    pub bass: f32,
    pub mids: f32,
    pub highs: f32,
    pub beat_pulse: f32,
    pub onset: f32,
    pub impact: f32,
    pub reactivity: f32,
    pub style_weights: [f32; 5],
}

impl Default for SmoothedVisualState {
    fn default() -> Self {
        Self {
            energy: 0.16,
            sub: 0.12,
            bass: 0.14,
            mids: 0.18,
            highs: 0.08,
            beat_pulse: 0.0,
            onset: 0.0,
            impact: 0.0,
            reactivity: 0.0,
            style_weights: [1.0, 0.0, 0.0, 0.0, 0.0],
        }
    }
}

impl SmoothedVisualState {
    pub fn update(&mut self, frame: VisualInputFrame, style: VisualStyle, delta_seconds: f32) {
        self.energy = envelope(self.energy, frame.energy, delta_seconds, 0.055, 0.42);
        self.sub = envelope(self.sub, frame.sub, delta_seconds, 0.045, 0.5);
        self.bass = envelope(self.bass, frame.bass, delta_seconds, 0.035, 0.3);
        self.mids = envelope(self.mids, frame.mids, delta_seconds, 0.08, 0.48);
        self.highs = envelope(self.highs, frame.highs, delta_seconds, 0.045, 0.28);
        self.beat_pulse = envelope(
            self.beat_pulse,
            frame.beat_pulse,
            delta_seconds,
            0.018,
            0.22,
        );
        self.onset = envelope(self.onset, frame.onset, delta_seconds, 0.015, 0.18);
        self.impact = envelope(self.impact, frame.impact, delta_seconds, 0.012, 0.34);
        self.reactivity = envelope(self.reactivity, frame.reactivity, delta_seconds, 0.6, 1.4);

        let target_weights = style_weights(style, frame.state);
        for (current, target) in self.style_weights.iter_mut().zip(target_weights) {
            *current = envelope(*current, target, delta_seconds, 0.45, 1.1);
        }
        let total = self.style_weights.iter().sum::<f32>().max(0.001);
        self.style_weights
            .iter_mut()
            .for_each(|value| *value /= total);
    }
}

fn envelope(current: f32, target: f32, delta_seconds: f32, attack: f32, release: f32) -> f32 {
    let time_constant = if target > current { attack } else { release };
    let amount = 1.0 - (-delta_seconds / time_constant.max(0.001)).exp();
    current + (target - current) * amount
}

fn style_weights(style: VisualStyle, state: MusicState) -> [f32; 5] {
    match style {
        VisualStyle::Fluid => [1.0, 0.0, 0.0, 0.0, 0.0],
        VisualStyle::Waves => [0.0, 1.0, 0.0, 0.0, 0.0],
        VisualStyle::Pulse => [0.0, 0.0, 1.0, 0.0, 0.0],
        VisualStyle::Tunnel => [0.0, 0.0, 0.0, 1.0, 0.0],
        VisualStyle::Burst => [0.0, 0.0, 0.0, 0.0, 1.0],
        VisualStyle::Auto => match state {
            MusicState::Quiet => [0.94, 0.03, 0.02, 0.01, 0.0],
            MusicState::Flow => [0.88, 0.08, 0.02, 0.02, 0.0],
            MusicState::Groove => [0.16, 0.68, 0.08, 0.06, 0.02],
            MusicState::Build => [0.42, 0.18, 0.06, 0.3, 0.04],
            MusicState::Impact => [0.04, 0.12, 0.14, 0.1, 0.6],
            MusicState::Peak => [0.1, 0.43, 0.14, 0.11, 0.22],
            MusicState::Breakdown => [0.82, 0.06, 0.07, 0.04, 0.01],
        },
    }
}

pub fn intensity_values(profile: IntensityProfile) -> [f32; 4] {
    match profile {
        IntensityProfile::Chill => [0.66, 0.62, 0.72, 0.45],
        IntensityProfile::Balanced => [0.9, 0.92, 1.0, 0.78],
        IntensityProfile::Wild => [1.12, 1.25, 1.2, 1.0],
    }
}

pub struct FlashEnvelope {
    value: f32,
    last_trigger_seconds: f32,
    was_above_threshold: bool,
}

impl Default for FlashEnvelope {
    fn default() -> Self {
        Self {
            value: 0.0,
            last_trigger_seconds: -10.0,
            was_above_threshold: false,
        }
    }
}

impl FlashEnvelope {
    pub fn update(
        &mut self,
        seconds: f32,
        impact: f32,
        profile: FlashProfile,
        strength: f32,
        intensity_scale: f32,
        delta_seconds: f32,
    ) -> f32 {
        let above_threshold = impact >= 0.72;
        let (cap, cooldown, release) = match profile {
            FlashProfile::Off => (0.0, f32::INFINITY, 0.025),
            FlashProfile::Moderate => (0.34, 1.6, 0.045),
            FlashProfile::High => (0.62, 1.25, 0.065),
        };
        if above_threshold
            && !self.was_above_threshold
            && seconds - self.last_trigger_seconds >= cooldown
        {
            self.value = (cap * strength * intensity_scale).clamp(0.0, cap);
            self.last_trigger_seconds = seconds;
        }
        self.was_above_threshold = above_threshold;
        self.value *= (-delta_seconds / release).exp();
        if profile == FlashProfile::Off {
            self.value = 0.0;
        }
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attack_is_faster_than_release() {
        let rise = envelope(0.0, 1.0, 0.05, 0.05, 0.5);
        let fall = envelope(1.0, 0.0, 0.05, 0.05, 0.5);
        assert!(rise > 1.0 - fall);
    }

    #[test]
    fn auto_style_blends_tunnel_into_a_build() {
        let weights = style_weights(VisualStyle::Auto, MusicState::Build);
        assert!(weights[3] > weights[1]);
        assert!(weights.iter().sum::<f32>() > 0.99);
    }

    #[test]
    fn settings_are_sanitized_before_runtime_use() {
        let settings = VisualSettings {
            pcm_buffer_seconds: 60,
            brightness: 4.0,
            ..Default::default()
        }
        .sanitized();
        assert_eq!(settings.pcm_buffer_seconds, 30);
        assert_eq!(settings.brightness, 1.25);
    }

    #[test]
    fn flash_is_opt_in_short_and_cooldown_limited() {
        let mut flash = FlashEnvelope::default();
        assert_eq!(
            flash.update(1.0, 1.0, FlashProfile::Off, 1.0, 1.0, 0.016),
            0.0
        );
        flash.update(1.1, 0.0, FlashProfile::High, 1.0, 1.0, 0.016);
        let first = flash.update(2.0, 1.0, FlashProfile::High, 1.0, 1.0, 0.016);
        assert!(first > 0.4 && first <= 0.62);
        flash.update(2.1, 0.0, FlashProfile::High, 1.0, 1.0, 0.1);
        let blocked = flash.update(2.2, 1.0, FlashProfile::High, 1.0, 1.0, 0.1);
        assert!(blocked < first * 0.25);
    }
}
