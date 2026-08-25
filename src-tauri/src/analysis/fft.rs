use rustfft::{num_complex::Complex32, Fft, FftPlanner};
use std::sync::Arc;

use super::{normalization::RollingNormalizer, rhythm::RhythmTracker, FeatureFrame};

pub const FFT_SIZE: usize = 2048;
pub const ANALYSIS_HOP: usize = 960;

pub struct AudioAnalyzer {
    fft: Arc<dyn Fft<f32>>,
    buffer: Vec<Complex32>,
    previous_spectrum: Vec<f32>,
    energy_normalizer: RollingNormalizer,
    sub_normalizer: RollingNormalizer,
    bass_normalizer: RollingNormalizer,
    mid_normalizer: RollingNormalizer,
    high_normalizer: RollingNormalizer,
    flux_normalizer: RollingNormalizer,
    rhythm: RhythmTracker,
    energy_fast: f32,
    energy_slow: f32,
}

impl AudioAnalyzer {
    pub fn new() -> Self {
        let mut planner = FftPlanner::new();
        Self {
            fft: planner.plan_fft_forward(FFT_SIZE),
            buffer: vec![Complex32::default(); FFT_SIZE],
            previous_spectrum: vec![0.0; FFT_SIZE / 2 + 1],
            energy_normalizer: RollingNormalizer::new(6000),
            sub_normalizer: RollingNormalizer::new(6000),
            bass_normalizer: RollingNormalizer::new(6000),
            mid_normalizer: RollingNormalizer::new(6000),
            high_normalizer: RollingNormalizer::new(6000),
            flux_normalizer: RollingNormalizer::new(6000),
            rhythm: RhythmTracker::new(),
            energy_fast: 0.0,
            energy_slow: 0.0,
        }
    }

    pub fn analyze(&mut self, samples: &[f32], timestamp_ms: u64) -> FeatureFrame {
        debug_assert!(samples.len() >= FFT_SIZE);
        let mut square_sum = 0.0;
        for (index, (slot, sample)) in self.buffer.iter_mut().zip(samples).enumerate() {
            let phase = index as f32 / (FFT_SIZE - 1) as f32;
            let window = 0.5 - 0.5 * (std::f32::consts::TAU * phase).cos();
            square_sum += sample * sample;
            *slot = Complex32::new(sample * window, 0.0);
        }
        let rms = (square_sum / FFT_SIZE as f32).sqrt();
        self.fft.process(&mut self.buffer);

        let mut flux = 0.0;
        let mut band_sums = [0.0_f32; 4];
        let mut band_counts = [0_u32; 4];
        for bin in 1..=FFT_SIZE / 2 {
            let magnitude = self.buffer[bin].norm() / FFT_SIZE as f32;
            flux += (magnitude - self.previous_spectrum[bin]).max(0.0);
            self.previous_spectrum[bin] = magnitude;
            let frequency = bin as f32 * 48_000.0 / FFT_SIZE as f32;
            let band = if frequency < 80.0 {
                Some(0)
            } else if frequency < 250.0 {
                Some(1)
            } else if frequency < 2_500.0 {
                Some(2)
            } else if frequency < 12_000.0 {
                Some(3)
            } else {
                None
            };
            if let Some(band) = band {
                band_sums[band] += magnitude * magnitude;
                band_counts[band] += 1;
            }
        }
        let band_energy = std::array::from_fn::<_, 4, _>(|index| {
            (band_sums[index] / band_counts[index].max(1) as f32).sqrt()
        });

        let relative_energy = self.energy_normalizer.normalize(rms);
        self.energy_fast += (relative_energy - self.energy_fast) * 0.32;
        self.energy_slow += (relative_energy - self.energy_slow) * 0.035;
        let spectral_flux = self.flux_normalizer.normalize(flux);
        let onset_strength = spectral_flux.powf(1.3);
        let seconds = timestamp_ms as f32 / 1000.0;
        let (beat_phase, beat_strength) = self.rhythm.update(seconds, onset_strength);

        FeatureFrame {
            timestamp_ms,
            loudness: rms,
            sub_energy: self.sub_normalizer.normalize(band_energy[0]),
            bass_energy: self.bass_normalizer.normalize(band_energy[1]),
            mid_energy: self.mid_normalizer.normalize(band_energy[2]),
            high_energy: self.high_normalizer.normalize(band_energy[3]),
            spectral_flux,
            onset_strength,
            beat_strength,
            beat_phase,
            energy_fast: self.energy_fast,
            energy_slow: self.energy_slow,
        }
    }
}
