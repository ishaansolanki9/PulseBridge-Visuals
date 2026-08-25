use std::sync::atomic::{AtomicU64, Ordering};

use crossbeam_queue::ArrayQueue;

pub const CAPTURE_SAMPLE_RATE: u32 = 48_000;

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub struct PcmRingBuffer {
    samples: ArrayQueue<f32>,
    dropped_samples: AtomicU64,
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
impl PcmRingBuffer {
    pub fn new(seconds: u8) -> Self {
        let seconds = seconds.clamp(5, 30) as usize;
        Self {
            samples: ArrayQueue::new(CAPTURE_SAMPLE_RATE as usize * seconds),
            dropped_samples: AtomicU64::new(0),
        }
    }

    pub fn push(&self, sample: f32) {
        if self.samples.push(sample.clamp(-1.0, 1.0)).is_err() {
            let _ = self.samples.pop();
            self.dropped_samples.fetch_add(1, Ordering::Relaxed);
            let _ = self.samples.push(sample.clamp(-1.0, 1.0));
        }
    }

    pub fn pop(&self) -> Option<f32> {
        self.samples.pop()
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    #[cfg(test)]
    pub fn capacity(&self) -> usize {
        self.samples.capacity()
    }

    pub fn dropped_samples(&self) -> u64 {
        self.dropped_samples.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_is_bounded_and_keeps_newest_audio() {
        let ring = PcmRingBuffer {
            samples: ArrayQueue::new(3),
            dropped_samples: AtomicU64::new(0),
        };
        ring.push(0.1);
        ring.push(0.2);
        ring.push(0.3);
        ring.push(0.4);
        assert_eq!(ring.len(), 3);
        assert_eq!(ring.dropped_samples(), 1);
        assert!((ring.pop().expect("sample") - 0.2).abs() < f32::EPSILON);
        assert!((ring.pop().expect("sample") - 0.3).abs() < f32::EPSILON);
        assert!((ring.pop().expect("sample") - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn configured_capacity_is_clamped() {
        assert_eq!(
            PcmRingBuffer::new(1).capacity(),
            CAPTURE_SAMPLE_RATE as usize * 5
        );
        assert_eq!(
            PcmRingBuffer::new(60).capacity(),
            CAPTURE_SAMPLE_RATE as usize * 30
        );
    }
}
