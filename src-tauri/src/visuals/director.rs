use std::{collections::VecDeque, time::Duration};

use crate::{
    analysis::{MusicState, VisualInputFrame},
    phrase::{PhraseKind, PhraseProvenance, PlaybackContext},
};

use super::{IntensityProfile, PaletteName, SceneSelection, VisualStyle};

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
    MagneticSwarm = 26,
    LiquidRelic = 27,
    ImpossibleArchitecture = 28,
    AuroraVeil = 29,
    KineticSculpture = 30,
    TopographicOcean = 31,
    Tron = 32,
    TronGridHighway = 33,
    TronLightTrails = 34,
    TronLaserGates = 35,
    TronHexCorridor = 36,
    TronIdentityDiscs = 37,
    TronCircuitBoard = 38,
    TronNeonArena = 39,
    TronSolarSails = 40,
    TronDigitalCity = 41,
    TronHelixDrive = 42,
    TronDataRain = 43,
    TronReactorIris = 44,
}

const ALL_ILLUSIONS: [VisualFamily; 32] = [
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
    VisualFamily::MagneticSwarm,
    VisualFamily::LiquidRelic,
    VisualFamily::ImpossibleArchitecture,
    VisualFamily::AuroraVeil,
    VisualFamily::KineticSculpture,
    VisualFamily::TopographicOcean,
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
    pub fn is_spatial(self) -> bool {
        self as u32 >= 26
    }

    // Composition classes let Auto avoid another tunnel just after a tunnel,
    // even when the actual shader family has a different name.
    fn composition(self) -> u8 {
        match self {
            // IDs retain their 0.1.3 names for saved selection compatibility.
            Self::TopographicOcean => 1,
            Self::LiquidRelic
            | Self::AuroraVeil
            | Self::SineInterference
            | Self::QuantumWeave
            | Self::ElectricTopography => 2,
            Self::ImpossibleArchitecture
            | Self::WarpSpiral
            | Self::HyperbolicTunnel
            | Self::RibbonWormhole
            | Self::HelixPortal
            | Self::RadialEscalator
            | Self::PrismVortex
            | Self::VortexChevron => 3,
            Self::KineticSculpture
            | Self::GlassOrbit
            | Self::GravityLens
            | Self::EventHorizon
            | Self::OrbitalMesh
            | Self::MoireRings => 4,
            _ => 5,
        }
    }
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
    pub transformation: f32,
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
            transformation: 0.0,
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
    focus: Option<VisualFamily>,
    last_switch_seconds: f32,
    active_transition: Option<(VisualFamily, VisualFamily, f32, f32)>,
    last_phrase_key: Option<u64>,
    last_style: VisualStyle,
    fallback_state: MusicState,
    fallback_counter: u64,
    recent: VecDeque<(VisualFamily, Option<VisualFamily>, u64)>,
    active_modifiers: Vec<ActiveModifier>,
    last_modifier_key: Option<u64>,
    spectacle: SpectacleEnvelope,
    musical_fit: super::musical_fit::MusicalFit,
    pending_choice: Option<(u64, VisualFamily)>,
    palette_key: Option<u64>,
    current_palette: PaletteName,
    last_direction_kind: PhraseKind,
}

impl SceneDirector {
    pub fn new(session_seed: u64) -> Self {
        Self {
            session_seed: session_seed.max(1),
            current_primary: VisualFamily::WarpSpiral,
            focus: None,
            last_switch_seconds: -MIN_DWELL_SECONDS,
            active_transition: None,
            last_phrase_key: None,
            last_style: VisualStyle::Auto,
            fallback_state: MusicState::Quiet,
            fallback_counter: 0,
            recent: VecDeque::with_capacity(HISTORY_LIMIT),
            active_modifiers: Vec::with_capacity(2),
            last_modifier_key: None,
            spectacle: SpectacleEnvelope::default(),
            musical_fit: super::musical_fit::MusicalFit::default(),
            pending_choice: None,
            palette_key: None,
            current_palette: PaletteName::Ocean,
            last_direction_kind: PhraseKind::Unknown,
        }
    }

    pub fn set_focus(&mut self, selection: SceneSelection) {
        self.focus = match selection {
            SceneSelection::Auto => None,
            SceneSelection::Tron => Some(VisualFamily::Tron),
            SceneSelection::TronGridHighway => Some(VisualFamily::TronGridHighway),
            SceneSelection::TronLightTrails => Some(VisualFamily::TronLightTrails),
            SceneSelection::TronLaserGates => Some(VisualFamily::TronLaserGates),
            SceneSelection::TronHexCorridor => Some(VisualFamily::TronHexCorridor),
            SceneSelection::TronIdentityDiscs => Some(VisualFamily::TronIdentityDiscs),
            SceneSelection::TronCircuitBoard => Some(VisualFamily::TronCircuitBoard),
            SceneSelection::TronNeonArena => Some(VisualFamily::TronNeonArena),
            SceneSelection::TronSolarSails => Some(VisualFamily::TronSolarSails),
            SceneSelection::TronDigitalCity => Some(VisualFamily::TronDigitalCity),
            SceneSelection::TronHelixDrive => Some(VisualFamily::TronHelixDrive),
            SceneSelection::TronDataRain => Some(VisualFamily::TronDataRain),
            SceneSelection::TronReactorIris => Some(VisualFamily::TronReactorIris),

            SceneSelection::MagneticSwarm => Some(VisualFamily::MagneticSwarm),
            SceneSelection::LiquidRelic => Some(VisualFamily::LiquidRelic),
            SceneSelection::ImpossibleArchitecture => Some(VisualFamily::ImpossibleArchitecture),
            SceneSelection::AuroraVeil => Some(VisualFamily::AuroraVeil),
            SceneSelection::KineticSculpture => Some(VisualFamily::KineticSculpture),
            SceneSelection::TopographicOcean => Some(VisualFamily::TopographicOcean),
        };
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
        self.musical_fit.update(now_seconds, frame);
        let manual = self.focus.is_some() || style != VisualStyle::Auto;
        let phrase_fresh = phrase.provenance != PhraseProvenance::Unavailable
            && phrase
                .phrase
                .as_ref()
                .is_some_and(|segment| segment.confidence >= 0.45)
            && now.saturating_duration_since(phrase.updated_at) <= PHRASE_STALE_AFTER;
        let phrase_kind = if phrase_fresh {
            phrase.phrase.as_ref().map(|segment| segment.kind)
        } else {
            None
        };
        let (direction_kind, direction_key, reason) = if manual {
            (
                phrase_kind.unwrap_or_else(|| phrase_for_music_state(frame.state)),
                self.focus.unwrap_or_else(|| manual_family(style)) as u64,
                SceneReason::ManualOverride,
            )
        } else if let Some(segment) = phrase.phrase.as_ref().filter(|_| phrase_fresh) {
            let track_hash = phrase
                .stable_track_id
                .as_deref()
                .map(stable_hash)
                .or(phrase.structure_signature)
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
        // Capture a choice once per musical boundary. Band changes while
        // waiting for the downbeat cannot make the incoming scene oscillate.
        let desired = if manual {
            self.focus.unwrap_or_else(|| manual_family(style))
        } else if self.last_phrase_key == Some(direction_key) && self.last_style == style {
            self.active_transition
                .map_or(self.current_primary, |transition| transition.1)
        } else if let Some((key, family)) = self
            .pending_choice
            .filter(|choice| choice.0 == direction_key)
        {
            debug_assert_eq!(key, direction_key);
            family
        } else {
            let family = self.choose_primary(direction_kind, direction_key);
            self.pending_choice = Some((direction_key, family));
            family
        };
        // A held scene still receives section-directed colors. Palette choice
        // depends on musical sections, independently of the manual scene key.
        let palette_key = if phrase_fresh {
            phrase
                .phrase
                .as_ref()
                .map_or(0, |segment| segment.index.rotate_left(17))
        } else {
            0
        } ^ phrase_kind_id(direction_kind);
        if self.palette_key != Some(palette_key) {
            self.current_palette = palette_for_music(
                direction_kind,
                palette_key,
                self.musical_fit.dominant_band(),
            );
            self.palette_key = Some(palette_key);
        }
        let first_plan = self.last_phrase_key.is_none();
        if first_plan {
            self.current_primary = desired;
            self.last_switch_seconds = now_seconds;
            self.last_phrase_key = Some(direction_key);
            self.last_style = style;
            self.last_direction_kind = direction_kind;
            self.remember(desired, None, mix_seed(self.session_seed, direction_key));
        }
        let key_changed = self.last_phrase_key != Some(direction_key) || self.last_style != style;
        let residence = if self.current_primary.is_spatial() != desired.is_spatial() {
            12.0
        } else {
            MIN_DWELL_SECONDS
        };
        let dwell_satisfied = now_seconds - self.last_switch_seconds >= residence;
        let confirmed_drop = self.last_direction_kind == PhraseKind::Up
            && direction_kind == PhraseKind::Chorus
            && frame.reactivity > 0.5
            && frame.state == MusicState::Impact
            && frame.impact > 0.82
            && frame.beat_confidence >= 0.6
            && frame.energy > 0.55
            && frame.bass > 0.48
            && now_seconds - self.last_switch_seconds >= 3.0;
        if !first_plan
            && desired != self.current_primary
            && key_changed
            && self.active_transition.is_none()
            && (manual
                || frame.beat_confidence < 0.5
                || frame.impact > 0.65
                || frame.bar_phase < 0.10)
            && (dwell_satisfied || confirmed_drop || manual || self.recent.is_empty())
        {
            let mixed = self.current_primary.is_spatial() != desired.is_spatial();
            let minimum_duration: f32 = if mixed {
                match intensity {
                    IntensityProfile::Chill => 3.6,
                    IntensityProfile::Balanced => 2.8,
                    IntensityProfile::Wild => 2.0,
                }
            } else {
                1.2
            };
            let duration = if frame.beat_confidence < 0.5 {
                transition_duration(direction_kind, intensity).max(2.4)
            } else {
                transition_duration(direction_kind, intensity)
            }
            .max(minimum_duration);
            self.active_transition = Some((self.current_primary, desired, now_seconds, duration));
            self.last_switch_seconds = now_seconds;
            self.last_phrase_key = Some(direction_key);
            self.last_style = style;
            self.last_direction_kind = direction_kind;
        } else if key_changed && desired == self.current_primary {
            self.last_phrase_key = Some(direction_key);
            self.last_style = style;
            self.last_direction_kind = direction_kind;
            self.remember(desired, None, mix_seed(self.session_seed, direction_key));
        }

        // A queued scene must not change the visible scene's seed or budgets
        // before its dissolve begins. The compositor then preserves the old look.
        let committed_key = self.last_phrase_key.unwrap_or(direction_key);
        let budgets = budgets_for(self.last_direction_kind, intensity, frame.energy);
        let mut plan = ScenePlan {
            primary: self.current_primary,
            secondary: None,
            primary_mix: 1.0,
            secondary_mix: 0.0,
            variation_seed: mix_seed(self.session_seed, committed_key),
            palette: self.current_palette,
            motion: budgets.0,
            detail: budgets.1,
            density: budgets.2,
            brightness: budgets.3,
            modifiers: [ModifierState::default(); 2],
            transition: None,
            reason,
            transformation: self.spectacle.update(now_seconds, frame, intensity),
        };

        if let Some((from, to, start, duration)) = self.active_transition {
            let progress = ((now_seconds - start) / duration.max(0.001)).clamp(0.0, 1.0);
            let eased =
                progress * progress * progress * (progress * (progress * 6.0 - 15.0) + 10.0);
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
            self.last_direction_kind,
            committed_key,
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
        let last_composition = self.recent.back().map(|scene| scene.0.composition());
        let recent = |family: VisualFamily| {
            self.recent
                .iter()
                .rev()
                .take(4)
                .any(|scene| scene.0 == family)
        };
        // Prefer a fresh composition, then a fresh scene. Musical fit is the
        // main ranking; deterministic variation only breaks close matches.
        for relaxation in 0..3 {
            let best = candidates
                .iter()
                .copied()
                .filter(|family| {
                    (relaxation >= 1 || Some(family.composition()) != last_composition)
                        && (relaxation >= 2 || !recent(*family))
                })
                .max_by(|left, right| {
                    let score = |family: VisualFamily| {
                        self.musical_fit.score(family)
                            + (mix_seed(key ^ self.session_seed, family as u64) % 1000) as f32
                                * 0.00018
                    };
                    score(*left).total_cmp(&score(*right))
                });
            if let Some(family) = best {
                return family;
            }
        }
        VisualFamily::AuroraVeil
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
            let musical_trigger = frame.impact > 0.42
                || frame.onset > 0.54
                || (frame.beat_pulse > 0.58 && frame.energy > 0.48)
                || matches!(
                    phrase,
                    PhraseKind::Up | PhraseKind::Chorus | PhraseKind::Fill
                );
            let modifier_key = direction_key;
            let modifier_due = self.last_modifier_key != Some(modifier_key) && musical_trigger;
            if modifier_due {
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

fn candidates_for_phrase(kind: PhraseKind) -> &'static [VisualFamily] {
    match kind {
        PhraseKind::Intro | PhraseKind::Outro => &[
            VisualFamily::AuroraVeil,
            VisualFamily::TopographicOcean,
            VisualFamily::GlassOrbit,
            VisualFamily::SineInterference,
            VisualFamily::GravityLens,
            VisualFamily::RibbonWormhole,
            VisualFamily::QuantumWeave,
            VisualFamily::DiamondDrift,
            VisualFamily::OrbitalMesh,
            VisualFamily::EventHorizon,
        ],
        PhraseKind::Verse => &[
            VisualFamily::MagneticSwarm,
            VisualFamily::KineticSculpture,
            VisualFamily::LiquidRelic,
            VisualFamily::WarpSpiral,
            VisualFamily::InfiniteChecker,
            VisualFamily::NeonLattice,
            VisualFamily::TwistedStripes,
            VisualFamily::RotatingSnakes,
            VisualFamily::LiquidCircuit,
            VisualFamily::ElectricTopography,
        ],
        PhraseKind::Up => &[
            VisualFamily::ImpossibleArchitecture,
            VisualFamily::MagneticSwarm,
            VisualFamily::HyperbolicTunnel,
            VisualFamily::ChromaticMaze,
            VisualFamily::VortexChevron,
            VisualFamily::PolarFan,
            VisualFamily::RadialEscalator,
            VisualFamily::HelixPortal,
            VisualFamily::WarpSpiral,
        ],
        PhraseKind::Chorus | PhraseKind::Fill => &[
            VisualFamily::MagneticSwarm,
            VisualFamily::LiquidRelic,
            VisualFamily::ImpossibleArchitecture,
            VisualFamily::KineticSculpture,
            VisualFamily::MoireRings,
            VisualFamily::PrismVortex,
            VisualFamily::FractalCompass,
            VisualFamily::AlienHeads,
            VisualFamily::ImpossibleCubes,
            VisualFamily::NeonLattice,
            VisualFamily::VortexChevron,
        ],
        PhraseKind::Down => &[
            VisualFamily::AuroraVeil,
            VisualFamily::TopographicOcean,
            VisualFamily::LiquidRelic,
            VisualFamily::GlassOrbit,
            VisualFamily::GravityLens,
            VisualFamily::SineInterference,
            VisualFamily::EventHorizon,
            VisualFamily::RibbonWormhole,
            VisualFamily::DiamondDrift,
        ],
        PhraseKind::Bridge => &[
            VisualFamily::TopographicOcean,
            VisualFamily::KineticSculpture,
            VisualFamily::QuantumWeave,
            VisualFamily::LiquidCircuit,
            VisualFamily::OrbitalMesh,
            VisualFamily::ElectricTopography,
            VisualFamily::ImpossibleCubes,
        ],
        PhraseKind::Unknown => &ALL_ILLUSIONS,
    }
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
    if base.is_spatial() {
        return modifier == ModifierKind::PaletteDrift;
    }
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

fn palette_for_music(kind: PhraseKind, key: u64, band: usize) -> PaletteName {
    let pair = match kind {
        PhraseKind::Intro | PhraseKind::Down | PhraseKind::Outro => {
            [PaletteName::Ocean, PaletteName::PurpleBlue]
        }
        PhraseKind::Up => [PaletteName::Sunset, PaletteName::Warm],
        PhraseKind::Chorus | PhraseKind::Fill => [PaletteName::Neon, PaletteName::RainbowFlow],
        PhraseKind::Verse | PhraseKind::Bridge => match band {
            0 => [PaletteName::Electric, PaletteName::Infrared],
            2 => [PaletteName::Neon, PaletteName::Sunset],
            _ => [PaletteName::PurpleBlue, PaletteName::RainbowFlow],
        },
        PhraseKind::Unknown => [PaletteName::Ocean, PaletteName::Electric],
    };
    pair[(mix_seed(key, 0xc010_1234) % 2) as usize]
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
    fn pending_scene_does_not_change_the_outgoing_seed_or_budgets() {
        let now = Instant::now();
        let mut director = SceneDirector::new(51);
        let frame = VisualInputFrame {
            energy: 0.7,
            beat_confidence: 0.9,
            bar_phase: 0.5,
            reactivity: 1.0,
            ..Default::default()
        };
        let before = director.update(
            0.0,
            now,
            frame,
            &context(now, PhraseKind::Intro, 0),
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        let waiting = director.update(
            9.0,
            now,
            frame,
            &context(now, PhraseKind::Chorus, 1),
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        assert!(waiting.secondary.is_none());
        assert_eq!(before.variation_seed, waiting.variation_seed);
        assert_eq!(
            (before.motion, before.detail, before.brightness),
            (waiting.motion, waiting.detail, waiting.brightness)
        );
    }

    #[test]
    fn mixed_scenes_get_a_longer_residence_and_gentle_dissolve() {
        let now = Instant::now();
        let mut director = SceneDirector::new(51);
        director.set_focus(SceneSelection::LiquidRelic);
        director.update(
            0.0,
            now,
            VisualInputFrame::default(),
            &context(now, PhraseKind::Intro, 0),
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        director.set_focus(SceneSelection::Auto);
        // A low-energy intro favors an original calm scene after the held ribbon.
        let frame = VisualInputFrame {
            beat_confidence: 0.9,
            ..Default::default()
        };
        let next = context(now, PhraseKind::Intro, 1);
        let early = director.update(
            9.0,
            now,
            frame,
            &next,
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        assert!(early.secondary.is_none());
        let start = director.update(
            12.0,
            now,
            frame,
            &next,
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        assert!(start.transition.unwrap().duration_seconds >= 2.8);
        let opening = director.update(
            12.05,
            now,
            frame,
            &next,
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        assert!(opening.secondary_mix < 0.001);
    }

    #[test]
    fn scene_choices_match_bass_melody_and_percussion() {
        for key in 0..24 {
            let mut director = SceneDirector::new(key + 1);
            let mut frame = VisualInputFrame {
                energy: 0.72,
                sub: 0.0,
                bass: 0.95,
                mids: 0.05,
                highs: 0.05,
                reactivity: 1.0,
                ..Default::default()
            };
            director.musical_fit.update(0.0, frame);
            assert_eq!(
                director.choose_primary(PhraseKind::Verse, key),
                VisualFamily::MagneticSwarm
            );
            frame.bass = 0.05;
            frame.mids = 0.95;
            for second in 1..=10 {
                director.musical_fit.update(second as f32, frame);
            }
            assert_eq!(
                director.choose_primary(PhraseKind::Verse, key),
                VisualFamily::LiquidRelic
            );
            frame.mids = 0.05;
            frame.highs = 0.95;
            frame.onset = 0.9;
            for second in 11..=20 {
                director.musical_fit.update(second as f32, frame);
            }
            assert_eq!(
                director.choose_primary(PhraseKind::Chorus, key),
                VisualFamily::KineticSculpture
            );
        }
    }

    #[test]
    fn queued_choice_waits_for_the_bar_and_survives_a_brief_band_change() {
        let now = Instant::now();
        let mut director = SceneDirector::new(51);
        let frame = VisualInputFrame {
            state: MusicState::Groove,
            energy: 0.7,
            bass: 0.9,
            reactivity: 1.0,
            beat_confidence: 0.9,
            bar_phase: 0.5,
            ..Default::default()
        };
        director.update(
            0.0,
            now,
            frame,
            &context(now, PhraseKind::Intro, 0),
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        let next = context(now, PhraseKind::Verse, 1);
        let waiting = director.update(
            12.0,
            now,
            frame,
            &next,
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        assert!(waiting.secondary.is_none());
        let chosen = director.pending_choice.unwrap().1;
        let downbeat = VisualInputFrame {
            bass: 0.05,
            mids: 0.95,
            bar_phase: 0.0,
            ..frame
        };
        let changing = director.update(
            12.5,
            now,
            downbeat,
            &next,
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        assert_eq!(changing.secondary, Some(chosen));
    }

    #[test]
    fn confirmed_drop_can_resolve_a_recent_build_but_weak_timing_cannot() {
        let now = Instant::now();
        let build = VisualInputFrame {
            state: MusicState::Build,
            energy: 0.6,
            bass: 0.5,
            reactivity: 1.0,
            beat_confidence: 0.9,
            ..Default::default()
        };
        for confidence in [0.2, 0.9] {
            let mut director = SceneDirector::new(71);
            director.update(
                0.0,
                now,
                build,
                &context(now, PhraseKind::Up, 0),
                VisualStyle::Auto,
                IntensityProfile::Balanced,
            );
            let drop = VisualInputFrame {
                state: MusicState::Impact,
                energy: 0.95,
                bass: 0.95,
                impact: 0.95,
                beat_confidence: confidence,
                ..build
            };
            let plan = director.update(
                4.0,
                now,
                drop,
                &context(now, PhraseKind::Chorus, 1),
                VisualStyle::Auto,
                IntensityProfile::Balanced,
            );
            assert_eq!(plan.secondary.is_some(), confidence > 0.6);
        }
    }

    #[test]
    fn held_geometry_still_gets_section_colors_and_weak_phrases_fall_back() {
        let now = Instant::now();
        let mut director = SceneDirector::new(51);
        director.set_focus(SceneSelection::LiquidRelic);
        let frame = VisualInputFrame::default();
        let quiet = director.update(
            0.0,
            now,
            frame,
            &context(now, PhraseKind::Down, 0),
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        let peak = director.update(
            12.0,
            now,
            frame,
            &context(now, PhraseKind::Chorus, 1),
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        assert_eq!(quiet.primary, peak.primary);
        assert_ne!(quiet.palette, peak.palette);
        director.set_focus(SceneSelection::Auto);
        let mut weak = context(now, PhraseKind::Chorus, 2);
        weak.provenance = PhraseProvenance::Rekordbox;
        weak.phrase.as_mut().unwrap().confidence = 0.1;
        let fallback = director.update(
            24.0,
            now,
            frame,
            &weak,
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        assert_eq!(fallback.reason, SceneReason::InferredState);
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
    fn automatic_scene_stays_stable_until_the_musical_phrase_changes() {
        let now = Instant::now();
        let phrase = context(now, PhraseKind::Verse, 0);
        let mut director = SceneDirector::new(17);
        let frame = VisualInputFrame {
            state: MusicState::Flow,
            energy: 0.76,
            onset: 0.88,
            beat_pulse: 0.82,
            reactivity: 1.0,
            ..Default::default()
        };
        let first = director.update(
            0.0,
            now,
            frame,
            &phrase,
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        let same_phrase = director.update(
            60.0,
            now,
            frame,
            &phrase,
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        assert_eq!(same_phrase.primary, first.primary);
        assert!(same_phrase.secondary.is_none());

        let next_phrase = context(now, PhraseKind::Chorus, 1);
        let changed = director.update(
            60.1,
            now,
            frame,
            &next_phrase,
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        assert!(changed.secondary.is_some() || changed.primary != first.primary);
    }

    #[test]
    fn auto_library_contains_thirty_two_distinct_scenes() {
        let distinct = ALL_ILLUSIONS
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(ALL_ILLUSIONS.len(), 32);
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

/// A single expansion/reassembly gesture; a sustained impact cannot retrigger it.
#[derive(Debug)]
pub(super) struct SpectacleEnvelope {
    started: Option<f32>,
    last_trigger: f32,
    armed: bool,
}
impl Default for SpectacleEnvelope {
    fn default() -> Self {
        Self {
            started: None,
            last_trigger: -24.0,
            armed: true,
        }
    }
}
impl SpectacleEnvelope {
    pub(super) fn update(
        &mut self,
        time: f32,
        frame: VisualInputFrame,
        intensity: IntensityProfile,
    ) -> f32 {
        if frame.impact < 0.25 {
            self.armed = true;
        }
        if frame.reactivity < 0.2 || intensity == IntensityProfile::Chill {
            self.started = None;
            return 0.0;
        }
        if self.armed
            && frame.impact > 0.72
            && frame.energy > 0.55
            && frame.beat_confidence >= 0.5
            && time - self.last_trigger >= 24.0
        {
            self.started = Some(time);
            self.last_trigger = time;
            self.armed = false;
        }
        self.started.map_or(0.0, |start| {
            let age = time - start;
            let envelope = smoothstep(age / 0.85) * (1.0 - smoothstep((age - 1.5) / 3.0));
            envelope
                * if intensity == IntensityProfile::Wild {
                    1.0
                } else {
                    0.72
                }
        })
    }
}

#[cfg(test)]
mod spatial_tests {
    use super::*;
    #[test]
    fn spectacle_requires_confidence_rearms_and_obeys_cooldown() {
        let mut event = SpectacleEnvelope::default();
        let mut frame = VisualInputFrame {
            energy: 0.9,
            impact: 1.0,
            reactivity: 1.0,
            beat_confidence: 0.1,
            ..Default::default()
        };
        assert_eq!(event.update(0.0, frame, IntensityProfile::Wild), 0.0);
        frame.beat_confidence = 0.9;
        event.update(1.0, frame, IntensityProfile::Wild);
        assert!(event.update(2.0, frame, IntensityProfile::Wild) > 0.9);
        assert_eq!(event.update(30.0, frame, IntensityProfile::Wild), 0.0);
        frame.impact = 0.0;
        event.update(31.0, frame, IntensityProfile::Wild);
        frame.impact = 1.0;
        event.update(32.0, frame, IntensityProfile::Wild);
        assert!(event.update(33.0, frame, IntensityProfile::Wild) > 0.9);
        frame.impact = 0.0;
        event.update(34.0, frame, IntensityProfile::Wild);
        frame.impact = 1.0;
        assert_eq!(event.update(40.0, frame, IntensityProfile::Wild), 0.0);
        assert_eq!(event.update(60.0, frame, IntensityProfile::Chill), 0.0);
        frame.reactivity = 0.0;
        assert_eq!(event.update(61.0, frame, IntensityProfile::Wild), 0.0);
    }
    #[test]
    fn tron_selections_round_trip_and_remain_held() {
        let choices = [
            "tron",
            "tronGridHighway",
            "tronLightTrails",
            "tronLaserGates",
            "tronHexCorridor",
            "tronIdentityDiscs",
            "tronCircuitBoard",
            "tronNeonArena",
            "tronSolarSails",
            "tronDigitalCity",
            "tronHelixDrive",
            "tronDataRain",
            "tronReactorIris",
        ];
        for (index, key) in choices.iter().enumerate() {
            let selection: SceneSelection = serde_json::from_value(serde_json::json!(key)).unwrap();
            assert_eq!(
                serde_json::to_value(selection).unwrap(),
                serde_json::json!(key)
            );
            let mut director = SceneDirector::new(45);
            director.set_focus(selection);
            for time in [0.0, 20.0, 100.0] {
                let plan = director.update(
                    time,
                    std::time::Instant::now(),
                    VisualInputFrame::default(),
                    &PlaybackContext::default(),
                    VisualStyle::Auto,
                    IntensityProfile::Balanced,
                );
                assert_eq!(plan.primary.id(), (32 + index) as f32);
                assert!(plan.secondary.is_none());
            }
            director.set_focus(SceneSelection::Auto);
            assert!(director.focus.is_none());
        }
    }

    #[test]
    fn held_scene_survives_phrases_and_can_return_to_auto() {
        let now = std::time::Instant::now();
        let context = PlaybackContext::default();
        let frame = VisualInputFrame::default();
        let mut director = SceneDirector::new(45);
        director.set_focus(SceneSelection::LiquidRelic);
        for time in [0.0, 10.0, 100.0] {
            let plan = director.update(
                time,
                now,
                frame,
                &context,
                VisualStyle::Auto,
                IntensityProfile::Balanced,
            );
            assert_eq!(plan.primary, VisualFamily::LiquidRelic);
            assert!(plan.secondary.is_none());
        }
        director.set_focus(SceneSelection::MagneticSwarm);
        director.update(
            101.0,
            now,
            frame,
            &context,
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        let plan = director.update(
            105.0,
            now,
            frame,
            &context,
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        assert_eq!(plan.primary, VisualFamily::MagneticSwarm);
        director.set_focus(SceneSelection::Auto);
        director.update(
            130.0,
            now,
            frame,
            &context,
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        let plan = director.update(
            135.0,
            now,
            frame,
            &context,
            VisualStyle::Auto,
            IntensityProfile::Balanced,
        );
        assert!(!matches!(plan.reason, SceneReason::ManualOverride));
        assert_ne!(plan.primary, VisualFamily::MagneticSwarm);
    }

    #[test]
    fn library_is_reachable_and_composition_varies() {
        let mut reachable = std::collections::HashSet::new();
        for kind in [
            PhraseKind::Intro,
            PhraseKind::Verse,
            PhraseKind::Up,
            PhraseKind::Chorus,
            PhraseKind::Down,
            PhraseKind::Bridge,
        ] {
            reachable.extend(candidates_for_phrase(kind).iter().copied());
        }
        assert_eq!(reachable.len(), 32);
        let mut director = SceneDirector::new(71);
        director.remember(VisualFamily::LiquidRelic, None, 1);
        for key in 0..50 {
            assert_ne!(
                director
                    .choose_primary(PhraseKind::Chorus, key)
                    .composition(),
                VisualFamily::LiquidRelic.composition()
            );
        }
        for family in ALL_ILLUSIONS
            .into_iter()
            .filter(|family| family.is_spatial())
        {
            assert!(!modifier_compatible(family, ModifierKind::MirrorFold));
            assert!(!modifier_compatible(family, ModifierKind::ChromaticSplit));
        }
    }
}
