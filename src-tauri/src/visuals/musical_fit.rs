//! Short-term musical character for scene ranking, not track/stem recognition.
use crate::analysis::VisualInputFrame;

use super::director::VisualFamily;

#[derive(Default)]
pub(super) struct MusicalFit {
    energy: f32,
    bands: [f32; 3],
    articulation: f32,
    last_seconds: Option<f32>,
}

impl MusicalFit {
    pub fn update(&mut self, seconds: f32, frame: VisualInputFrame) {
        let clean = |value: f32| {
            if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        let live = clean(frame.reactivity);
        let energy = clean(frame.energy) * live;
        let bands = [frame.bass.max(frame.sub), frame.mids, frame.highs].map(|v| clean(v) * live);
        let articulation = clean(frame.onset.max(frame.beat_pulse * frame.beat_confidence)) * live;
        let amount = self.last_seconds.map_or(1.0, |last| {
            1.0 - (-(seconds - last).clamp(0.0, 2.0) / 1.5).exp()
        });
        self.last_seconds = Some(seconds);
        self.energy += (energy - self.energy) * amount;
        for (current, target) in self.bands.iter_mut().zip(bands) {
            *current += (target - *current) * amount;
        }
        self.articulation += (articulation - self.articulation) * amount;
    }

    pub fn dominant_band(&self) -> usize {
        self.bands
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map_or(1, |(index, _)| index)
    }

    pub fn score(&self, family: VisualFamily) -> f32 {
        use VisualFamily::*;
        // Preferred energy, band affinity (bass/mids/highs), sharp articulation.
        // Phrase eligibility and recent composition filtering happen separately.
        let (energy, bands, articulation): (f32, [f32; 3], f32) = match family {
            Tron | TronGridHighway | TronLightTrails | TronLaserGates | TronHexCorridor
            | TronIdentityDiscs | TronCircuitBoard | TronNeonArena | TronSolarSails
            | TronDigitalCity | TronHelixDrive | TronDataRain | TronReactorIris => {
                (0.7, [0.8, 0.6, 0.8], 0.7)
            }
            NeonMandala | SpectrumBloom => (0.8, [0.7, 0.7, 0.8], 0.65),
            PlasmaWeave | ChromaticMoire => (0.5, [0.4, 0.9, 0.6], 0.4),
            OrbitFoundry | SynapseBloom => (0.72, [0.8, 0.9, 0.7], 0.65),
            GravityBraids | PrismConveyor => (0.6, [0.75, 0.85, 0.65], 0.55),
            MagneticSwarm => (0.75, [1.0, 0.45, 0.65], 0.8), // Bass Web
            LiquidRelic => (0.58, [0.6, 1.0, 0.6], 0.4),     // Ribbon Reactor
            ImpossibleArchitecture => (0.88, [0.9, 0.3, 0.7], 0.8), // Shockwave Tunnel
            AuroraVeil => (0.28, [0.2, 1.0, 0.7], 0.1),      // Aurora Strings
            KineticSculpture => (0.82, [0.5, 0.5, 1.0], 1.0), // Prism Surge
            TopographicOcean => (0.43, [1.0, 0.6, 0.3], 0.35), // Faultline
            SineInterference | QuantumWeave | TwistedStripes | LiquidCircuit => {
                (0.5, [0.45, 0.9, 0.4], 0.35)
            }
            GlassOrbit | GravityLens | DiamondDrift | EventHorizon => {
                (0.25, [0.45, 0.8, 0.55], 0.1)
            }
            RibbonWormhole | OrbitalMesh | RotatingSnakes => (0.43, [0.5, 0.8, 0.55], 0.25),
            NeonLattice | InfiniteChecker | ChromaticMaze | ImpossibleCubes => {
                (0.7, [0.5, 0.4, 0.9], 0.85)
            }
            MoireRings | FractalCompass | AlienHeads | PrismVortex => {
                (0.85, [0.65, 0.3, 0.85], 0.85)
            }
            ElectricTopography => (0.55, [0.85, 0.65, 0.3], 0.45),
            WarpSpiral | HyperbolicTunnel | VortexChevron | PolarFan | RadialEscalator
            | HelixPortal => (0.82, [0.8, 0.35, 0.6], 0.7),
        };
        let sum = self.bands.iter().sum::<f32>().max(0.001);
        let affinity: f32 = self
            .bands
            .iter()
            .zip(bands)
            .map(|(level, weight)| level / sum * weight)
            .sum();
        affinity * 2.8 - (self.energy - energy).powi(2) * 3.0
            + self.articulation * articulation * 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn musical_character_changes_rankings_at_equal_energy() {
        let mut fit = MusicalFit::default();
        fit.update(
            0.0,
            VisualInputFrame {
                energy: 0.7,
                bass: 0.95,
                sub: 0.8,
                mids: 0.08,
                highs: 0.06,
                reactivity: 1.0,
                ..Default::default()
            },
        );
        assert!(fit.score(VisualFamily::MagneticSwarm) > fit.score(VisualFamily::LiquidRelic));
        for index in 1..=60 {
            fit.update(
                index as f32 / 10.0,
                VisualInputFrame {
                    energy: 0.7,
                    bass: 0.06,
                    sub: 0.03,
                    mids: 0.95,
                    highs: 0.08,
                    reactivity: 1.0,
                    ..Default::default()
                },
            );
        }
        assert!(fit.score(VisualFamily::LiquidRelic) > fit.score(VisualFamily::MagneticSwarm));
    }
    #[test]
    fn one_transient_cannot_replace_the_smoothed_character() {
        let mut fit = MusicalFit::default();
        let frame = VisualInputFrame {
            energy: 0.5,
            bass: 0.9,
            mids: 0.1,
            highs: 0.1,
            reactivity: 1.0,
            ..Default::default()
        };
        fit.update(0.0, frame);
        fit.update(
            1.0 / 60.0,
            VisualInputFrame {
                bass: 0.0,
                mids: 0.1,
                highs: 1.0,
                ..frame
            },
        );
        assert_eq!(fit.dominant_band(), 0);
    }
    #[test]
    fn absent_audio_favors_open_calm_scenes() {
        let mut fit = MusicalFit::default();
        fit.update(
            0.0,
            VisualInputFrame {
                energy: 1.0,
                bass: 1.0,
                reactivity: 0.0,
                ..Default::default()
            },
        );
        assert!(
            fit.score(VisualFamily::AuroraVeil) > fit.score(VisualFamily::ImpossibleArchitecture)
        );
    }
}
