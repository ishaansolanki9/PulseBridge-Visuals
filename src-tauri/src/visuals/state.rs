use serde::{Deserialize, Serialize};

use crate::analysis::VisualInputFrame;

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
        self.style = VisualStyle::Auto;
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
    pub drive: f32,
    pub bass_hit: f32,
    pub mid_motion: f32,
    pub high_hit: f32,
    pub energy_rise: f32,
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
            drive: 0.0,
            bass_hit: 0.0,
            mid_motion: 0.0,
            high_hit: 0.0,
            energy_rise: 0.0,
        }
    }
}

impl SmoothedVisualState {
    pub fn update(&mut self, frame: VisualInputFrame, delta_seconds: f32) {
        let energy_rise = (frame.energy - self.energy).max(0.0);
        let sub_rise = (frame.sub - self.sub).max(0.0);
        let bass_rise = (frame.bass - self.bass).max(0.0);
        let mid_change = (frame.mids - self.mids).abs();
        let high_rise = (frame.highs - self.highs).max(0.0);
        let onset_accent = ((frame.onset - 0.22) / 0.78).clamp(0.0, 1.0);
        let inferred_beat = onset_accent * (0.38 + frame.bass * 0.62);
        let visual_beat = frame.beat_pulse.max(inferred_beat).clamp(0.0, 1.0);

        self.energy = envelope(self.energy, frame.energy, delta_seconds, 0.055, 0.42);
        self.sub = envelope(self.sub, frame.sub, delta_seconds, 0.045, 0.5);
        self.bass = envelope(self.bass, frame.bass, delta_seconds, 0.035, 0.3);
        self.mids = envelope(self.mids, frame.mids, delta_seconds, 0.08, 0.48);
        self.highs = envelope(self.highs, frame.highs, delta_seconds, 0.045, 0.28);
        self.beat_pulse = envelope(self.beat_pulse, visual_beat, delta_seconds, 0.012, 0.18);
        self.onset = envelope(
            self.onset,
            frame.onset.powf(0.82),
            delta_seconds,
            0.01,
            0.15,
        );
        self.impact = envelope(self.impact, frame.impact, delta_seconds, 0.012, 0.34);
        self.reactivity = envelope(self.reactivity, frame.reactivity, delta_seconds, 0.6, 1.4);
        let reactive = frame.reactivity.clamp(0.0, 1.0);
        let bass_hit_target =
            (visual_beat * 0.58 + onset_accent * 0.16 + bass_rise * 2.35 + sub_rise * 1.45)
                .clamp(0.0, 1.0)
                * reactive;
        let mid_motion_target = (((frame.mids - 0.08) / 0.82).clamp(0.0, 1.0) * 0.52
            + mid_change * 2.4
            + onset_accent * 0.3)
            .clamp(0.0, 1.0)
            * reactive;
        let high_hit_target = (high_rise * 3.1
            + onset_accent * 0.62
            + ((frame.highs - 0.16) / 0.84).clamp(0.0, 1.0) * 0.24)
            .clamp(0.0, 1.0)
            * reactive;
        let energy_rise_target = (energy_rise * 3.4 + onset_accent * 0.32 + frame.impact * 0.42)
            .clamp(0.0, 1.0)
            * reactive;
        self.bass_hit = envelope(self.bass_hit, bass_hit_target, delta_seconds, 0.012, 0.2);
        self.mid_motion = envelope(
            self.mid_motion,
            mid_motion_target,
            delta_seconds,
            0.035,
            0.28,
        );
        self.high_hit = envelope(self.high_hit, high_hit_target, delta_seconds, 0.009, 0.14);
        self.energy_rise = envelope(
            self.energy_rise,
            energy_rise_target,
            delta_seconds,
            0.012,
            0.26,
        );
        let tonal_energy =
            frame.energy * 0.48 + frame.bass * 0.22 + frame.mids * 0.12 + frame.highs * 0.18;
        let continuous = ((tonal_energy - 0.1) / 0.78).clamp(0.0, 1.0);
        let target_drive =
            (continuous * 0.78 + frame.beat_pulse * 0.12 + frame.onset * 0.1 + frame.impact * 0.2)
                .clamp(0.0, 1.0)
                * frame.reactivity.clamp(0.0, 1.0);
        self.drive = envelope(self.drive, target_drive, delta_seconds, 0.055, 0.34);
    }
}

fn envelope(current: f32, target: f32, delta_seconds: f32, attack: f32, release: f32) -> f32 {
    let time_constant = if target > current { attack } else { release };
    let amount = 1.0 - (-delta_seconds / time_constant.max(0.001)).exp();
    current + (target - current) * amount
}

pub fn intensity_values(profile: IntensityProfile) -> [f32; 4] {
    match profile {
        IntensityProfile::Chill => [0.66, 0.62, 0.72, 0.45],
        IntensityProfile::Balanced => [0.85, 0.87, 0.94, 0.73],
        IntensityProfile::Wild => [1.26, 1.31, 1.17, 0.93],
    }
}

pub fn intensity_ceiling(intensity: IntensityProfile) -> f32 {
    match intensity {
        IntensityProfile::Chill => 0.48,
        IntensityProfile::Balanced => 0.7,
        IntensityProfile::Wild => 0.93,
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
    fn settings_are_sanitized_before_runtime_use() {
        let settings = VisualSettings {
            pcm_buffer_seconds: 60,
            brightness: 4.0,
            style: VisualStyle::Burst,
            ..Default::default()
        }
        .sanitized();
        assert_eq!(settings.pcm_buffer_seconds, 30);
        assert_eq!(settings.brightness, 1.25);
        assert_eq!(settings.style, VisualStyle::Auto);
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

    #[test]
    fn intensity_profiles_set_the_dial_ceiling_without_forcing_the_dial() {
        assert_eq!(intensity_ceiling(IntensityProfile::Wild), 0.93);
        assert!(
            intensity_ceiling(IntensityProfile::Balanced)
                < intensity_ceiling(IntensityProfile::Wild)
        );
        assert!(intensity_ceiling(IntensityProfile::Balanced) < 0.8);
        assert!(intensity_ceiling(IntensityProfile::Chill) < 0.5);
    }

    #[test]
    fn drive_rises_and_falls_proportionally_with_audio() {
        let mut state = SmoothedVisualState::default();
        let quiet = VisualInputFrame {
            energy: 0.12,
            bass: 0.1,
            mids: 0.1,
            highs: 0.08,
            reactivity: 1.0,
            ..Default::default()
        };
        state.update(quiet, 0.2);
        let quiet_drive = state.drive;
        state.update(
            VisualInputFrame {
                energy: 0.95,
                bass: 0.9,
                mids: 0.8,
                highs: 0.85,
                beat_pulse: 0.9,
                onset: 0.8,
                impact: 0.9,
                reactivity: 1.0,
                ..Default::default()
            },
            0.2,
        );
        assert!(state.drive > quiet_drive + 0.6);
        assert!(state.drive <= 1.0);
    }

    #[test]
    fn frequency_changes_create_distinct_short_lived_visual_events() {
        let mut bass_state = SmoothedVisualState::default();
        bass_state.update(
            VisualInputFrame {
                energy: 0.72,
                sub: 0.88,
                bass: 0.96,
                mids: 0.18,
                highs: 0.08,
                beat_pulse: 0.9,
                reactivity: 1.0,
                ..Default::default()
            },
            0.05,
        );
        assert!(bass_state.bass_hit > bass_state.high_hit);

        let mut high_state = SmoothedVisualState::default();
        high_state.update(
            VisualInputFrame {
                energy: 0.62,
                sub: 0.08,
                bass: 0.12,
                mids: 0.32,
                highs: 0.98,
                onset: 0.92,
                reactivity: 1.0,
                ..Default::default()
            },
            0.05,
        );
        assert!(high_state.high_hit > high_state.bass_hit);
        assert!(high_state.energy_rise > 0.2);
    }

    #[test]
    fn no_live_audio_cannot_create_reactive_events() {
        let mut state = SmoothedVisualState::default();
        state.update(
            VisualInputFrame {
                energy: 1.0,
                bass: 1.0,
                mids: 1.0,
                highs: 1.0,
                beat_pulse: 1.0,
                onset: 1.0,
                impact: 1.0,
                reactivity: 0.0,
                ..Default::default()
            },
            0.1,
        );
        assert_eq!(state.bass_hit, 0.0);
        assert_eq!(state.mid_motion, 0.0);
        assert_eq!(state.high_hit, 0.0);
        assert_eq!(state.energy_rise, 0.0);
    }
}
