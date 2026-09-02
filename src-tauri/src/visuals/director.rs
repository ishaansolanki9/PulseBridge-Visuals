use std::{collections::VecDeque, time::Duration};

use crate::{
    analysis::{MusicState, VisualInputFrame},
    phrase::{PhraseKind, PhraseProvenance, PlaybackContext},
};

use super::{IntensityProfile, PaletteName, VisualStyle};

const HISTORY_LIMIT: usize = 8;
const MIN_DWELL_SECONDS: f32 = 8.0;
const PHRASE_STALE_AFTER: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum VisualFamily {
    #[default]
    WarpSpiral = 0,
    MoireRings = 1,
    InfiniteChecker = 2,
    NeonLattice = 3,
    TwistedStripes = 4,
    RotatingSnakes = 5,
    HyperbolicTunnel = 6,
    ChromaticMaze = 7,
    VortexChevron = 8,
    GlassOrbit = 9,
    SineInterference = 10,
    ImpossibleCubes = 11,
    PolarFan = 12,
    GravityLens = 13,
    RibbonWormhole = 14,
    QuantumWeave = 15,
    FractalCompass = 16,
    LiquidCircuit = 17,
    AlienHeads = 18,
    PrismVortex = 19,
    DiamondDrift = 20,
    OrbitalMesh = 21,
    HelixPortal = 22,
    RadialEscalator = 23,
    ElectricTopography = 24,
    EventHorizon = 25,
}

const ALL_ILLUSIONS: [VisualFamily; 26] = [
    VisualFamily::WarpSpiral,
    VisualFamily::MoireRings,
    VisualFamily::InfiniteChecker,
    VisualFamily::NeonLattice,
    VisualFamily::TwistedStripes,
    VisualFamily::RotatingSnakes,
    VisualFamily::HyperbolicTunnel,
    VisualFamily::ChromaticMaze,
    VisualFamily::VortexChevron,
    VisualFamily::GlassOrbit,
    VisualFamily::SineInterference,
    VisualFamily::ImpossibleCubes,
    VisualFamily::PolarFan,
    VisualFamily::GravityLens,
    VisualFamily::RibbonWormhole,
    VisualFamily::QuantumWeave,
    VisualFamily::FractalCompass,
    VisualFamily::LiquidCircuit,
    VisualFamily::AlienHeads,
    VisualFamily::PrismVortex,
    VisualFamily::DiamondDrift,
    VisualFamily::OrbitalMesh,
    VisualFamily::HelixPortal,
    VisualFamily::RadialEscalator,
    VisualFamily::ElectricTopography,
    VisualFamily::EventHorizon,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ModifierKind {
    PaletteDrift = 0,
    BeatZoom = 1,
    BassWarp = 2,
    HighSparkle = 3,
    EchoTrails = 4,
    MirrorFold = 5,
    ChromaticSplit = 6,
    ImpactBloom = 7,
}

impl ModifierKind {
    pub fn id(self) -> f32 {
        self as u32 as f32
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ModifierState {
    pub kind: Option<ModifierKind>,
    pub strength: f32,
}

impl VisualFamily {
    pub fn id(self) -> f32 {
        self as u32 as f32
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SceneReason {
    Phrase,
    InferredState,
    ManualOverride,
    #[default]
    Fallback,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneTransition {
    pub start_seconds: f32,
    pub duration_seconds: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScenePlan {
    pub primary: VisualFamily,
    pub secondary: Option<VisualFamily>,
    pub primary_mix: f32,
    pub secondary_mix: f32,
    pub variation_seed: u64,
    pub palette: PaletteName,
    pub motion: f32,
    pub detail: f32,
    pub density: f32,
    pub brightness: f32,
    pub modifiers: [ModifierState; 2],
    pub transition: Option<SceneTransition>,
    pub reason: SceneReason,
}

impl Default for ScenePlan {
    fn default() -> Self {
        Self {
            primary: VisualFamily::WarpSpiral,
            secondary: None,
            primary_mix: 1.0,
            secondary_mix: 0.0,
            variation_seed: 1,
            palette: PaletteName::Ocean,
            motion: 0.55,
            detail: 0.45,
            density: 0.4,
            brightness: 0.72,
            modifiers: [ModifierState::default(); 2],
            transition: None,
            reason: SceneReason::Fallback,
        }
    }
}

impl ScenePlan {
    #[cfg(test)]
    pub fn active_family_count(self) -> usize {
        usize::from(self.primary_mix > 0.001) + usize::from(self.secondary_mix > 0.001)
    }

    #[cfg(test)]
    pub fn active_modifier_count(self) -> usize {
        self.modifiers
            .iter()
            .filter(|modifier| modifier.kind.is_some() && modifier.strength > 0.001)
            .count()
    }

    fn normalized(mut self) -> Self {
        self.primary_mix = finite_clamp(self.primary_mix, 0.0, 1.0);
        self.secondary_mix = finite_clamp(self.secondary_mix, 0.0, 1.0);
        let total = self.primary_mix + self.secondary_mix;
        if total <= 0.001 {
            self.primary_mix = 1.0;
            self.secondary_mix = 0.0;
            self.secondary = None;
        } else if total > 1.0 {
            self.primary_mix /= total;
            self.secondary_mix /= total;
        }
        if self.secondary.is_none() {
            self.secondary_mix = 0.0;
            self.primary_mix = 1.0;
        }
        self.motion = finite_clamp(self.motion, 0.2, 1.4);
        self.detail = finite_clamp(self.detail, 0.15, 1.0);
        self.density = finite_clamp(self.density, 0.1, 1.0);
        self.brightness = finite_clamp(self.brightness, 0.25, 1.0);
        for modifier in &mut self.modifiers {
            modifier.strength = finite_clamp(modifier.strength, 0.0, 1.0);
            if modifier.kind.is_none() {
                modifier.strength = 0.0;
            }
        }
        let modifier_load = self
            .modifiers
            .iter()
            .map(|modifier| modifier.strength)
            .sum::<f32>();
        self.brightness = self.brightness.min(1.0 - (modifier_load * 0.06).min(0.12));
        self
    }
}

#[derive(Clone, Copy, Debug)]
struct ActiveModifier {
    kind: ModifierKind,
    started_seconds: f32,
    attack_seconds: f32,
    hold_seconds: f32,
    release_seconds: f32,
    peak: f32,
}

impl ActiveModifier {
    fn strength(self, now_seconds: f32) -> f32 {
        let age = (now_seconds - self.started_seconds).max(0.0);
        if age < self.attack_seconds {
            smoothstep(age / self.attack_seconds.max(0.001)) * self.peak
        } else if age < self.attack_seconds + self.hold_seconds {
            self.peak
        } else {
            let release_age = age - self.attack_seconds - self.hold_seconds;
            (1.0 - smoothstep(release_age / self.release_seconds.max(0.001))) * self.peak
        }
        .clamp(0.0, 1.0)
    }

    fn expired(self, now_seconds: f32) -> bool {
        now_seconds
            >= self.started_seconds + self.attack_seconds + self.hold_seconds + self.release_seconds
    }
}

pub struct SceneDirector {
    session_seed: u64,
    current_primary: VisualFamily,
    last_switch_seconds: f32,
    active_transition: Option<(VisualFamily, VisualFamily, f32, f32)>,
    last_phrase_key: Option<u64>,
    last_style: VisualStyle,
    fallback_state: MusicState,
    fallback_counter: u64,
    recent: VecDeque<(VisualFamily, Option<VisualFamily>, u64)>,
    active_modifiers: Vec<ActiveModifier>,
    last_modifier_key: Option<u64>,
}

impl SceneDirector {
    pub fn new(session_seed: u64) -> Self {
        Self {
            session_seed: session_seed.max(1),
            current_primary: VisualFamily::WarpSpiral,
            last_switch_seconds: -MIN_DWELL_SECONDS,
            active_transition: None,
            last_phrase_key: None,
            last_style: VisualStyle::Auto,
            fallback_state: MusicState::Quiet,
            fallback_counter: 0,
            recent: VecDeque::with_capacity(HISTORY_LIMIT),
            active_modifiers: Vec::with_capacity(2),
            last_modifier_key: None,
        }
    }

    pub fn update(
        &mut self,
        now_seconds: f32,
        now: std::time::Instant,
        frame: VisualInputFrame,
        phrase: &PlaybackContext,
        style: VisualStyle,
        intensity: IntensityProfile,
    ) -> ScenePlan {
        let manual = style != VisualStyle::Auto;
        let phrase_fresh = phrase.provenance != PhraseProvenance::Unavailable
            && now.saturating_duration_since(phrase.updated_at) <= PHRASE_STALE_AFTER;
        let phrase_kind = if phrase_fresh {
            phrase.phrase.as_ref().map(|segment| segment.kind)
        } else {
            None
        };
        let (direction_kind, mut direction_key, reason) = if manual {
            (
                phrase_kind.unwrap_or_else(|| phrase_for_music_state(frame.state)),
                manual_family(style) as u64,
                SceneReason::ManualOverride,
            )
        } else if let Some(segment) = phrase.phrase.as_ref().filter(|_| phrase_fresh) {
            let track_hash = phrase
                .stable_track_id
                .as_deref()
                .map(stable_hash)
                .unwrap_or(self.session_seed);
            (
                segment.kind,
                track_hash ^ segment.index.rotate_left(17) ^ phrase_kind_id(segment.kind),
                if phrase.provenance == PhraseProvenance::AudioInferred {
                    SceneReason::InferredState
                } else {
                    SceneReason::Phrase
                },
            )
        } else {
            if frame.state != self.fallback_state
                && now_seconds - self.last_switch_seconds >= MIN_DWELL_SECONDS
            {
                self.fallback_state = frame.state;
                self.fallback_counter = self.fallback_counter.saturating_add(1);
            }
            (
                phrase_for_music_state(frame.state),
                self.session_seed
                    ^ self.fallback_counter.rotate_left(11)
                    ^ music_state_id(frame.state),
                SceneReason::InferredState,
            )
        };
        if !manual {
            let shuffle_seconds = match intensity {
                IntensityProfile::Chill => 18.0,
                IntensityProfile::Balanced => 14.0,
                IntensityProfile::Wild => 10.0,
            };
            let shuffle_slot = (now_seconds.max(0.0) / shuffle_seconds).floor() as u64;
            direction_key ^= mix_seed(self.session_seed, shuffle_slot).rotate_left(29);
        }

        let desired = if manual {
            manual_family(style)
        } else {
            self.choose_primary(direction_kind, direction_key)
        };
        let first_plan = self.last_phrase_key.is_none();
        if first_plan {
            self.current_primary = desired;
            self.last_switch_seconds = now_seconds;
            self.last_phrase_key = Some(direction_key);
            self.last_style = style;
            self.remember(desired, None, mix_seed(self.session_seed, direction_key));
        }
        let key_changed = self.last_phrase_key != Some(direction_key) || self.last_style != style;
        let dwell_satisfied = now_seconds - self.last_switch_seconds >= MIN_DWELL_SECONDS;
        if !first_plan
            && desired != self.current_primary
            && key_changed
            && (dwell_satisfied || manual || self.recent.is_empty())
        {
            let duration = transition_duration(direction_kind, intensity);
            self.active_transition = Some((self.current_primary, desired, now_seconds, duration));
            self.last_switch_seconds = now_seconds;
            self.last_phrase_key = Some(direction_key);
            self.last_style = style;
        } else if key_changed && desired == self.current_primary {
            self.last_phrase_key = Some(direction_key);
            self.last_style = style;
            self.remember(desired, None, mix_seed(self.session_seed, direction_key));
        }

        let budgets = budgets_for(direction_kind, intensity, frame.energy);
        let mut plan = ScenePlan {
            primary: self.current_primary,
            secondary: None,
            primary_mix: 1.0,
            secondary_mix: 0.0,
            variation_seed: mix_seed(self.session_seed, direction_key),
            palette: palette_for_phrase(direction_kind),
            motion: budgets.0,
            detail: budgets.1,
            density: budgets.2,
            brightness: budgets.3,
            modifiers: [ModifierState::default(); 2],
            transition: None,
            reason,
        };

        if let Some((from, to, start, duration)) = self.active_transition {
            let progress = ((now_seconds - start) / duration.max(0.001)).clamp(0.0, 1.0);
            let eased = progress * progress * (3.0 - 2.0 * progress);
            plan.primary = from;
            plan.secondary = Some(to);
            plan.primary_mix = 1.0 - eased;
            plan.secondary_mix = eased;
            plan.transition = Some(SceneTransition {
                start_seconds: start,
                duration_seconds: duration,
            });
            if progress >= 1.0 {
                self.current_primary = to;
                self.active_transition = None;
                self.remember(to, None, plan.variation_seed);
                plan.primary = to;
                plan.secondary = None;
                plan.primary_mix = 1.0;
                plan.secondary_mix = 0.0;
                plan.transition = None;
            }
        } else {
            plan.primary = self.current_primary;
        }
        plan.modifiers = self.update_modifiers(
            now_seconds,
            direction_kind,
            direction_key,
            intensity,
            frame,
            plan.primary,
            manual,
        );
        if let Some(incoming) = plan.secondary {
            for modifier in &mut plan.modifiers {
                if modifier
                    .kind
                    .is_some_and(|kind| !modifier_compatible(incoming, kind))
                {
                    *modifier = ModifierState::default();
                }
            }
        }
        plan.normalized()
    }

    fn choose_primary(&self, phrase: PhraseKind, key: u64) -> VisualFamily {
        let candidates = candidates_for_phrase(phrase);
        let start = mix_seed(self.session_seed, key) as usize % candidates.len();
        for offset in 0..candidates.len() {
            let candidate = candidates[(start + offset) % candidates.len()];
            let recently_used = self
                .recent
                .iter()
                .rev()
                .take(4)
                .any(|scene| scene.0 == candidate);
            if !recently_used {
                return candidate;
            }
        }
        candidates[start]
    }

    fn remember(&mut self, primary: VisualFamily, secondary: Option<VisualFamily>, seed: u64) {
        if self.recent.len() == HISTORY_LIMIT {
            self.recent.pop_front();
        }
        if self.recent.back().copied() != Some((primary, secondary, seed)) {
            self.recent.push_back((primary, secondary, seed));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn update_modifiers(
        &mut self,
        now_seconds: f32,
        phrase: PhraseKind,
        direction_key: u64,
        intensity: IntensityProfile,
        frame: VisualInputFrame,
        base: VisualFamily,
        manual: bool,
    ) -> [ModifierState; 2] {
        self.active_modifiers
            .retain(|modifier| !modifier.expired(now_seconds));
        if manual || intensity == IntensityProfile::Chill {
            self.active_modifiers.clear();
        } else {
            let rotation_seconds = if intensity == IntensityProfile::Wild {
                7.0
            } else {
                16.0
            };
            let rotation = (now_seconds.max(0.0) / rotation_seconds).floor() as u64;
            let modifier_key = direction_key ^ rotation.rotate_left(23);
            if self.last_modifier_key != Some(modifier_key) {
                self.last_modifier_key = Some(modifier_key);
                let candidate =
                    modifier_for_phrase(phrase, mix_seed(self.session_seed, modifier_key));
                if modifier_compatible(base, candidate)
                    && !self
                        .active_modifiers
                        .iter()
                        .any(|active| active.kind == candidate)
                    && self.active_modifiers.len() < 2
                {
                    self.active_modifiers.push(ActiveModifier {
                        kind: candidate,
                        started_seconds: now_seconds,
                        attack_seconds: if intensity == IntensityProfile::Wild {
                            0.16
                        } else {
                            0.45
                        },
                        hold_seconds: if intensity == IntensityProfile::Wild {
                            7.5
                        } else if phrase == PhraseKind::Chorus {
                            10.0
                        } else {
                            12.0
                        },
                        release_seconds: if intensity == IntensityProfile::Wild {
                            0.9
                        } else {
                            1.8
                        },
                        peak: if intensity == IntensityProfile::Wild {
                            1.0
                        } else {
                            0.62
                        },
                    });
                }
            }
            let impact_threshold = if intensity == IntensityProfile::Wild {
                0.56
            } else {
                0.72
            };
            if frame.impact > impact_threshold
                && modifier_compatible(base, ModifierKind::ImpactBloom)
                && !self
                    .active_modifiers
                    .iter()
                    .any(|active| active.kind == ModifierKind::ImpactBloom)
                && self.active_modifiers.len() < 2
            {
                self.active_modifiers.push(ActiveModifier {
                    kind: ModifierKind::ImpactBloom,
                    started_seconds: now_seconds,
                    attack_seconds: 0.04,
                    hold_seconds: if intensity == IntensityProfile::Wild {
                        0.26
                    } else {
                        0.16
                    },
                    release_seconds: if intensity == IntensityProfile::Wild {
                        0.34
                    } else {
                        0.5
                    },
                    peak: if intensity == IntensityProfile::Wild {
                        1.0
                    } else {
                        0.68
                    },
                });
            }
        }

        let mut slots = [ModifierState::default(); 2];
        for (slot, modifier) in slots.iter_mut().zip(self.active_modifiers.iter().copied()) {
            let strength = modifier.strength(now_seconds);
            if strength > 0.001 && modifier_compatible(base, modifier.kind) {
                *slot = ModifierState {
                    kind: Some(modifier.kind),
                    strength,
                };
            }
        }
        slots
    }
}

fn candidates_for_phrase(_kind: PhraseKind) -> &'static [VisualFamily] {
    &ALL_ILLUSIONS
}

fn modifier_for_phrase(phrase: PhraseKind, key: u64) -> ModifierKind {
    let candidates: &[ModifierKind] = match phrase {
        PhraseKind::Intro | PhraseKind::Outro => {
            &[ModifierKind::PaletteDrift, ModifierKind::EchoTrails]
        }
        PhraseKind::Verse => &[
            ModifierKind::BeatZoom,
            ModifierKind::PaletteDrift,
            ModifierKind::HighSparkle,
        ],
        PhraseKind::Up => &[
            ModifierKind::BassWarp,
            ModifierKind::ChromaticSplit,
            ModifierKind::BeatZoom,
        ],
        PhraseKind::Chorus => &[
            ModifierKind::BeatZoom,
            ModifierKind::MirrorFold,
            ModifierKind::ChromaticSplit,
        ],
        PhraseKind::Down => &[ModifierKind::EchoTrails, ModifierKind::PaletteDrift],
        PhraseKind::Bridge => &[
            ModifierKind::MirrorFold,
            ModifierKind::PaletteDrift,
            ModifierKind::HighSparkle,
        ],
        PhraseKind::Fill => &[ModifierKind::ImpactBloom, ModifierKind::BeatZoom],
        PhraseKind::Unknown => &[ModifierKind::PaletteDrift, ModifierKind::BeatZoom],
    };
    candidates[key as usize % candidates.len()]
}

fn modifier_compatible(base: VisualFamily, modifier: ModifierKind) -> bool {
    match modifier {
        ModifierKind::MirrorFold => !matches!(
            base,
            VisualFamily::WarpSpiral | VisualFamily::RotatingSnakes | VisualFamily::RadialEscalator
        ),
        ModifierKind::EchoTrails => !matches!(
            base,
            VisualFamily::MoireRings | VisualFamily::NeonLattice | VisualFamily::OrbitalMesh
        ),
        ModifierKind::HighSparkle => !matches!(base, VisualFamily::EventHorizon),
        ModifierKind::ChromaticSplit => !matches!(base, VisualFamily::PrismVortex),
        _ => true,
    }
}

fn budgets_for(
    phrase: PhraseKind,
    intensity: IntensityProfile,
    energy: f32,
) -> (f32, f32, f32, f32) {
    let (profile_motion, profile_detail, profile_brightness) = match intensity {
        IntensityProfile::Chill => (0.76, 0.55, 0.72),
        IntensityProfile::Balanced => (1.04, 0.76, 0.88),
        IntensityProfile::Wild => (1.45, 1.0, 1.0),
    };
    let (motion, detail, density, brightness) = match phrase {
        PhraseKind::Intro | PhraseKind::Outro => (0.58, 0.48, 0.36, 0.65),
        PhraseKind::Verse => (0.78, 0.62, 0.55, 0.76),
        PhraseKind::Up => (0.98, 0.72, 0.68, 0.84),
        PhraseKind::Chorus => (1.1, 0.82, 0.78, 0.94),
        PhraseKind::Down => (0.52, 0.42, 0.32, 0.58),
        PhraseKind::Bridge => (0.76, 0.68, 0.5, 0.72),
        PhraseKind::Fill => (0.92, 0.7, 0.58, 0.82),
        PhraseKind::Unknown => (0.68, 0.55, 0.45, 0.7),
    };
    (
        motion * profile_motion * (0.86 + energy * 0.28),
        detail * profile_detail,
        density * profile_detail,
        brightness * profile_brightness,
    )
}

fn transition_duration(kind: PhraseKind, intensity: IntensityProfile) -> f32 {
    let base = match kind {
        PhraseKind::Chorus | PhraseKind::Fill => 0.8,
        PhraseKind::Up | PhraseKind::Bridge => 1.4,
        PhraseKind::Intro | PhraseKind::Down | PhraseKind::Outro => 2.4,
        _ => 1.8,
    };
    match intensity {
        IntensityProfile::Chill => base * 1.25,
        IntensityProfile::Balanced => base,
        IntensityProfile::Wild => base * 0.58,
    }
}

fn palette_for_phrase(kind: PhraseKind) -> PaletteName {
    match kind {
        PhraseKind::Intro | PhraseKind::Down | PhraseKind::Outro => PaletteName::Ocean,
        PhraseKind::Verse => PaletteName::Electric,
        PhraseKind::Up => PaletteName::Sunset,
        PhraseKind::Chorus | PhraseKind::Fill => PaletteName::Neon,
        PhraseKind::Bridge => PaletteName::PurpleBlue,
        PhraseKind::Unknown => PaletteName::Ocean,
    }
}

fn phrase_for_music_state(state: MusicState) -> PhraseKind {
    match state {
        MusicState::Quiet => PhraseKind::Intro,
        MusicState::Flow | MusicState::Groove => PhraseKind::Verse,
        MusicState::Build => PhraseKind::Up,
        MusicState::Impact | MusicState::Peak => PhraseKind::Chorus,
        MusicState::Breakdown => PhraseKind::Down,
    }
}

fn manual_family(style: VisualStyle) -> VisualFamily {
    match style {
        VisualStyle::Auto | VisualStyle::Tunnel => VisualFamily::WarpSpiral,
        VisualStyle::Fluid => VisualFamily::LiquidCircuit,
        VisualStyle::Waves => VisualFamily::SineInterference,
        VisualStyle::Pulse => VisualFamily::MoireRings,
        VisualStyle::Burst => VisualFamily::PrismVortex,
    }
}

fn phrase_kind_id(kind: PhraseKind) -> u64 {
    match kind {
        PhraseKind::Intro => 1,
        PhraseKind::Verse => 2,
        PhraseKind::Up => 3,
        PhraseKind::Chorus => 4,
        PhraseKind::Down => 5,
        PhraseKind::Bridge => 6,
        PhraseKind::Outro => 7,
        PhraseKind::Fill => 8,
        PhraseKind::Unknown => 9,
    }
}

fn music_state_id(state: MusicState) -> u64 {
    match state {
        MusicState::Quiet => 1,
        MusicState::Flow => 2,
        MusicState::Groove => 3,
        MusicState::Build => 4,
        MusicState::Impact => 5,
        MusicState::Peak => 6,
        MusicState::Breakdown => 7,
    }
}

fn stable_hash(value: &str) -> u64 {
    value.bytes().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

fn mix_seed(left: u64, right: u64) -> u64 {
    let mut value = left ^ right.wrapping_add(0x9e3779b97f4a7c15);
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58476d1ce4e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

fn finite_clamp(value: f32, minimum: f32, maximum: f32) -> f32 {
    if value.is_finite() {
        value.clamp(minimum, maximum)
    } else {
        minimum
    }
}

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::phrase::{PhraseProvenance, PhraseSegment};

    fn context(now: Instant, kind: PhraseKind, index: u64) -> PlaybackContext {
        PlaybackContext {
            phrase: Some(PhraseSegment {
                kind,
                index,
                start_ms: 0,
                end_ms: None,
                confidence: 0.8,
            }),
            provenance: PhraseProvenance::AudioInferred,
            updated_at: now,
            ..Default::default()
        }
    }

    #[test]
    fn plans_are_deterministic_normalized_and_use_at_most_two_families() {
        let now = Instant::now();
        let frame = VisualInputFrame {
            state: MusicState::Peak,
            energy: 0.9,
            impact: 0.9,
            reactivity: 1.0,
            ..Default::default()
        };
        let phrase = context(now, PhraseKind::Chorus, 3);
        let mut first = SceneDirector::new(42);
        let mut second = SceneDirector::new(42);
        let first_plan = first.update(
            8.0,
            now,
            frame,
            &phrase,
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        let second_plan = second.update(
            8.0,
            now,
            frame,
            &phrase,
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        assert_eq!(first_plan, second_plan);
        assert!(first_plan.active_family_count() <= 2);
        assert!((first_plan.primary_mix + first_plan.secondary_mix - 1.0).abs() < 0.001);
        assert!(first_plan.primary_mix.is_finite() && first_plan.secondary_mix.is_finite());
    }

    #[test]
    fn dwell_and_crossfade_prevent_abrupt_phrase_switches() {
        let now = Instant::now();
        let mut director = SceneDirector::new(7);
        let frame = VisualInputFrame::default();
        let intro = context(now, PhraseKind::Intro, 0);
        let _ = director.update(
            0.0,
            now,
            frame,
            &intro,
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        let chorus = context(now, PhraseKind::Chorus, 1);
        let held = director.update(
            1.0,
            now,
            frame,
            &chorus,
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        assert!(held.transition.is_none());
        let changing = director.update(
            14.0,
            now,
            frame,
            &chorus,
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        assert!(changing.transition.is_some());
        assert_eq!(
            changing.active_family_count(),
            1,
            "transition starts at zero incoming mix"
        );
        let middle = director.update(
            14.4,
            now,
            frame,
            &chorus,
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        assert_eq!(middle.active_family_count(), 2);
    }

    #[test]
    fn stale_phrase_uses_inferred_state_and_manual_styles_stay_fixed() {
        let now = Instant::now();
        let mut stale = context(now - Duration::from_secs(8), PhraseKind::Chorus, 4);
        stale.provenance = PhraseProvenance::Rekordbox;
        let frame = VisualInputFrame {
            state: MusicState::Breakdown,
            ..Default::default()
        };
        let mut director = SceneDirector::new(9);
        let inferred = director.update(
            8.0,
            now,
            frame,
            &stale,
            VisualStyle::Auto,
            IntensityProfile::Chill,
        );
        assert_eq!(inferred.reason, SceneReason::InferredState);
        let manual = director.update(
            9.0,
            now,
            frame,
            &stale,
            VisualStyle::Tunnel,
            IntensityProfile::Wild,
        );
        assert!(
            manual.secondary.is_some(),
            "manual changes use a safe crossfade"
        );
        let settled = director.update(
            12.0,
            now,
            frame,
            &stale,
            VisualStyle::Tunnel,
            IntensityProfile::Wild,
        );
        assert_eq!(settled.primary, VisualFamily::WarpSpiral);
        assert!(settled.secondary.is_none());
    }

    #[test]
    fn recent_scene_history_is_bounded() {
        let mut director = SceneDirector::new(1);
        for index in 0..32 {
            director.remember(VisualFamily::LiquidCircuit, None, index);
        }
        assert_eq!(director.recent.len(), HISTORY_LIMIT);
    }

    #[test]
    fn auto_library_contains_twenty_six_distinct_illusions() {
        let distinct = ALL_ILLUSIONS
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(ALL_ILLUSIONS.len(), 26);
        assert_eq!(distinct.len(), ALL_ILLUSIONS.len());
    }

    #[test]
    fn a_primary_is_not_selected_for_a_third_consecutive_phrase() {
        let mut director = SceneDirector::new(33);
        let key = 19;
        let repeated = director.choose_primary(PhraseKind::Intro, key);
        director.remember(repeated, None, 1);
        director.remember(repeated, None, 2);
        assert_ne!(director.choose_primary(PhraseKind::Intro, key), repeated);
    }

    #[test]
    fn modifiers_are_bounded_compatible_and_do_not_create_a_second_base() {
        let now = Instant::now();
        let phrase = context(now, PhraseKind::Chorus, 2);
        let frame = VisualInputFrame {
            state: MusicState::Peak,
            energy: 0.92,
            bass: 0.86,
            highs: 0.8,
            impact: 0.9,
            ..Default::default()
        };
        let mut director = SceneDirector::new(71);
        let _ = director.update(
            16.0,
            now,
            frame,
            &phrase,
            VisualStyle::Auto,
            IntensityProfile::Wild,
        );
        let plan = director.update(
            16.5,
            now,
            frame,
            &phrase,
            VisualStyle::Auto,
            IntensityProfile::Wild,
        );
        assert_eq!(plan.active_family_count(), 1);
        assert!(plan.active_modifier_count() <= 2);
        assert!(plan.modifiers.iter().all(|modifier| modifier
            .kind
            .is_none_or(|kind| modifier_compatible(plan.primary, kind))));
    }

    #[test]
    fn modifier_envelopes_attack_hold_and_release_cleanly() {
        let modifier = ActiveModifier {
            kind: ModifierKind::BeatZoom,
            started_seconds: 10.0,
            attack_seconds: 0.5,
            hold_seconds: 1.0,
            release_seconds: 0.5,
            peak: 0.8,
        };
        assert_eq!(modifier.strength(10.0), 0.0);
        assert!((modifier.strength(10.5) - 0.8).abs() < 0.001);
        assert!((modifier.strength(11.25) - 0.8).abs() < 0.001);
        assert!(modifier.strength(11.75) > 0.0);
        assert_eq!(modifier.strength(12.0), 0.0);
        assert!(modifier.expired(12.0));
    }

    #[test]
    fn modifier_load_reduces_instead_of_multiplying_brightness_budget() {
        let plan = ScenePlan {
            brightness: 1.0,
            modifiers: [
                ModifierState {
                    kind: Some(ModifierKind::BeatZoom),
                    strength: 1.0,
                },
                ModifierState {
                    kind: Some(ModifierKind::ImpactBloom),
                    strength: 1.0,
                },
            ],
            ..Default::default()
        }
        .normalized();
        assert!(plan.brightness <= 0.88);
    }
}
