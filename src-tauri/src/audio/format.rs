#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub enum SampleFormat {
    Float32,
    Pcm16,
    Pcm24,
    Pcm32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioFormat {
    pub sample_format: SampleFormat,
    pub channels: u16,
    pub sample_rate: u32,
    pub block_align: u16,
}

impl AudioFormat {
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub fn validate(self) -> Result<Self, String> {
        let sample_bytes = match self.sample_format {
            SampleFormat::Pcm16 => 2,
            SampleFormat::Pcm24 => 3,
            SampleFormat::Float32 | SampleFormat::Pcm32 => 4,
        };
        let expected_align = self.channels as usize * sample_bytes;
        if self.channels == 0 || self.channels > 32 {
            return Err(format!("Unsupported channel count: {}", self.channels));
        }
        if !(8_000..=384_000).contains(&self.sample_rate) {
            return Err(format!("Unsupported sample rate: {} Hz", self.sample_rate));
        }
        if usize::from(self.block_align) < expected_align {
            return Err(format!(
                "Invalid audio block alignment: {} bytes for {} channels",
                self.block_align, self.channels
            ));
        }
        Ok(self)
    }

    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub fn label(self) -> &'static str {
        match self.sample_format {
            SampleFormat::Float32 => "32-bit float",
            SampleFormat::Pcm16 => "16-bit PCM",
            SampleFormat::Pcm24 => "24-bit PCM",
            SampleFormat::Pcm32 => "32-bit PCM",
        }
    }
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub fn convert_interleaved_to_mono_into(
    bytes: &[u8],
    frames: usize,
    format: AudioFormat,
    output: &mut Vec<f32>,
) -> Result<(), String> {
    let format = format.validate()?;
    let required = frames
        .checked_mul(usize::from(format.block_align))
        .ok_or_else(|| "Audio packet size overflow".to_string())?;
    if bytes.len() < required {
        return Err(format!(
            "Audio packet was shorter than its declared frame count ({} < {required})",
            bytes.len()
        ));
    }
    let sample_bytes = match format.sample_format {
        SampleFormat::Pcm16 => 2,
        SampleFormat::Pcm24 => 3,
        SampleFormat::Float32 | SampleFormat::Pcm32 => 4,
    };
    output.clear();
    output.reserve(frames.saturating_sub(output.capacity()));
    for frame in bytes[..required].chunks_exact(usize::from(format.block_align)) {
        let mut total = 0.0;
        for channel in 0..usize::from(format.channels) {
            let offset = channel * sample_bytes;
            total += decode_sample(&frame[offset..offset + sample_bytes], format.sample_format);
        }
        output.push((total / f32::from(format.channels)).clamp(-1.0, 1.0));
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct StreamingResampler {
    source_rate: u32,
    target_rate: u32,
    total_input: u64,
    next_position: f64,
    previous_sample: Option<f32>,
}

impl StreamingResampler {
    pub fn new(source_rate: u32, target_rate: u32) -> Result<Self, String> {
        if source_rate == 0 || target_rate == 0 {
            return Err("Audio resampler rates must be non-zero".to_string());
        }
        Ok(Self {
            source_rate,
            target_rate,
            total_input: 0,
            next_position: 0.0,
            previous_sample: None,
        })
    }

    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        output.clear();
        if input.is_empty() {
            return;
        }
        if self.source_rate == self.target_rate {
            output.extend_from_slice(input);
            self.total_input = self.total_input.saturating_add(input.len() as u64);
            self.previous_sample = input.last().copied();
            self.next_position = self.total_input as f64;
            return;
        }

        let packet_start = self.total_input as i128;
        let packet_end = packet_start + input.len() as i128;
        let source_per_output = self.source_rate as f64 / self.target_rate as f64;
        let expected = ((input.len() as f64 / source_per_output).ceil() as usize).saturating_add(2);
        output.reserve(expected.saturating_sub(output.capacity()));

        loop {
            let lower = self.next_position.floor() as i128;
            let upper = lower + 1;
            if upper >= packet_end {
                break;
            }
            let lower_sample = if lower < packet_start {
                match self.previous_sample {
                    Some(sample) if lower == packet_start - 1 => sample,
                    _ => {
                        self.next_position = packet_start as f64;
                        continue;
                    }
                }
            } else {
                input[(lower - packet_start) as usize]
            };
            let upper_sample = input[(upper - packet_start) as usize];
            let fraction = (self.next_position - lower as f64) as f32;
            output.push(lower_sample + (upper_sample - lower_sample) * fraction);
            self.next_position += source_per_output;
        }
        self.total_input = self.total_input.saturating_add(input.len() as u64);
        self.previous_sample = input.last().copied();
    }
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn decode_sample(bytes: &[u8], format: SampleFormat) -> f32 {
    match format {
        SampleFormat::Float32 => f32::from_le_bytes(bytes.try_into().unwrap_or([0; 4])),
        SampleFormat::Pcm16 => {
            f32::from(i16::from_le_bytes(bytes.try_into().unwrap_or([0; 2]))) / 32_768.0
        }
        SampleFormat::Pcm24 => {
            let raw =
                i32::from(bytes[0]) | (i32::from(bytes[1]) << 8) | (i32::from(bytes[2]) << 16);
            let signed = if raw & 0x0080_0000 != 0 {
                raw | !0x00ff_ffff
            } else {
                raw
            };
            signed as f32 / 8_388_608.0
        }
        SampleFormat::Pcm32 => {
            i32::from_le_bytes(bytes.try_into().unwrap_or([0; 4])) as f32 / 2_147_483_648.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_and_downmixes_supported_formats() {
        let stereo_i16 = [0xff, 0x7f, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00];
        let mut mono = Vec::new();
        convert_interleaved_to_mono_into(
            &stereo_i16,
            2,
            AudioFormat {
                sample_format: SampleFormat::Pcm16,
                channels: 2,
                sample_rate: 44_100,
                block_align: 4,
            },
            &mut mono,
        )
        .unwrap();
        assert!(mono[0] > 0.49 && mono[0] < 0.51);
        assert!(mono[1] < -0.49 && mono[1] > -0.51);

        let float = [0.5_f32.to_le_bytes(), (-0.5_f32).to_le_bytes()].concat();
        convert_interleaved_to_mono_into(
            &float,
            1,
            AudioFormat {
                sample_format: SampleFormat::Float32,
                channels: 2,
                sample_rate: 48_000,
                block_align: 8,
            },
            &mut mono,
        )
        .unwrap();
        assert!(mono[0].abs() < 0.0001);
    }

    #[test]
    fn rejects_invalid_layout_and_resamples_to_analysis_rate() {
        let invalid = AudioFormat {
            sample_format: SampleFormat::Pcm32,
            channels: 2,
            sample_rate: 48_000,
            block_align: 4,
        };
        assert!(invalid.validate().is_err());
        let mut resampler = StreamingResampler::new(24_000, 48_000).unwrap();
        let mut resampled = Vec::new();
        resampler.process(&[0.0, 0.5, 1.0, 0.5], &mut resampled);
        assert_eq!(resampled.len(), 6);
        assert!(resampled.iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn streaming_resampler_preserves_phase_across_packets() {
        let source = [0.0, 0.25, 0.5, 0.75, 1.0, 0.75, 0.5, 0.25];
        let mut whole = StreamingResampler::new(44_100, 48_000).unwrap();
        let mut expected = Vec::new();
        whole.process(&source, &mut expected);

        let mut streaming = StreamingResampler::new(44_100, 48_000).unwrap();
        let mut first = Vec::new();
        let mut second = Vec::new();
        streaming.process(&source[..4], &mut first);
        streaming.process(&source[4..], &mut second);
        first.extend(second);
        assert_eq!(first.len(), expected.len());
        assert!(first
            .iter()
            .zip(expected)
            .all(|(left, right)| (left - right).abs() < 0.0001));
    }
}
