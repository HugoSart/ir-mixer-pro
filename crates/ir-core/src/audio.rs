use serde::{Deserialize, Serialize};
use std::f32::consts::PI;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SampleRate(pub u32);

#[derive(Clone, Debug, PartialEq)]
pub struct AudioBuffer {
    sample_rate: SampleRate,
    channels: Vec<Vec<f32>>,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AudioBufferError {
    #[error("audio buffer must contain one or two channels")]
    UnsupportedChannelCount,
    #[error("all audio channels must have the same number of frames")]
    UnequalChannelLengths,
    #[error("sample rate must be greater than zero")]
    InvalidSampleRate,
}

impl AudioBuffer {
    pub fn new(sample_rate: SampleRate, channels: Vec<Vec<f32>>) -> Result<Self, AudioBufferError> {
        if sample_rate.0 == 0 {
            return Err(AudioBufferError::InvalidSampleRate);
        }
        if !(1..=2).contains(&channels.len()) {
            return Err(AudioBufferError::UnsupportedChannelCount);
        }
        let frames = channels[0].len();
        if channels.iter().any(|channel| channel.len() != frames) {
            return Err(AudioBufferError::UnequalChannelLengths);
        }
        Ok(Self {
            sample_rate,
            channels,
        })
    }

    pub fn mono(sample_rate: SampleRate, samples: Vec<f32>) -> Self {
        Self {
            sample_rate,
            channels: vec![samples],
        }
    }

    pub fn stereo(
        sample_rate: SampleRate,
        left: Vec<f32>,
        right: Vec<f32>,
    ) -> Result<Self, AudioBufferError> {
        Self::new(sample_rate, vec![left, right])
    }

    pub fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }
    pub fn frame_count(&self) -> usize {
        self.channels[0].len()
    }
    pub fn channels(&self) -> &[Vec<f32>] {
        &self.channels
    }
    pub fn channel(&self, index: usize) -> &[f32] {
        &self.channels[index]
    }

    pub fn peak(&self) -> f32 {
        self.channels
            .iter()
            .flatten()
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()))
    }

    pub fn normalization_gain(&self, target_dbfs: f32) -> f32 {
        let peak = self.peak();
        if peak <= f32::EPSILON {
            1.0
        } else {
            db_to_gain(target_dbfs) / peak
        }
    }

    pub fn to_stereo(&self) -> (Vec<f32>, Vec<f32>) {
        if self.channels.len() == 1 {
            (self.channels[0].clone(), self.channels[0].clone())
        } else {
            (self.channels[0].clone(), self.channels[1].clone())
        }
    }

    /// Offline, windowed-sinc resampling. All allocations happen on the caller's worker thread.
    pub fn resample(&self, target: SampleRate) -> Result<Self, AudioBufferError> {
        if target.0 == 0 {
            return Err(AudioBufferError::InvalidSampleRate);
        }
        if target == self.sample_rate {
            return Ok(self.clone());
        }
        let ratio = target.0 as f64 / self.sample_rate.0 as f64;
        let output_len = ((self.frame_count() as f64) * ratio).round() as usize;
        let radius = 32_i64;
        let cutoff = ratio.min(1.0) as f32;
        let mut output = vec![vec![0.0; output_len]; self.channel_count()];
        for (source, destination) in self.channels.iter().zip(output.iter_mut()) {
            for (out_index, sample) in destination.iter_mut().enumerate() {
                let source_position = out_index as f64 / ratio;
                let center = source_position.floor() as i64;
                let mut sum = 0.0_f64;
                let mut weight_sum = 0.0_f64;
                for tap in (center - radius + 1)..=(center + radius) {
                    if tap < 0 || tap >= source.len() as i64 {
                        continue;
                    }
                    let distance = (source_position - tap as f64) as f32;
                    let normalized = distance / radius as f32;
                    if normalized.abs() >= 1.0 {
                        continue;
                    }
                    let sinc_arg = PI * distance * cutoff;
                    let sinc = if sinc_arg.abs() < 1.0e-6 {
                        1.0
                    } else {
                        sinc_arg.sin() / sinc_arg
                    };
                    let window = 0.5 + 0.5 * (PI * normalized).cos();
                    let weight = (sinc * window * cutoff) as f64;
                    sum += source[tap as usize] as f64 * weight;
                    weight_sum += weight;
                }
                *sample = if weight_sum.abs() > 1.0e-12 {
                    (sum / weight_sum) as f32
                } else {
                    0.0
                };
            }
        }
        Self::new(target, output)
    }

    pub fn waveform_envelope(&self, points: usize) -> Vec<f32> {
        if points == 0 || self.frame_count() == 0 {
            return Vec::new();
        }
        let mut result = Vec::with_capacity(points);
        for point in 0..points {
            let start = point * self.frame_count() / points;
            let end = ((point + 1) * self.frame_count() / points)
                .max(start + 1)
                .min(self.frame_count());
            let mut value = 0.0_f32;
            for channel in &self.channels {
                for &sample in &channel[start..end] {
                    if sample.abs() > value.abs() {
                        value = sample;
                    }
                }
            }
            result.push(value);
        }
        result
    }
}

pub fn db_to_gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

pub fn gain_to_db(gain: f32) -> f32 {
    if gain <= 0.0 {
        f32::NEG_INFINITY
    } else {
        20.0 * gain.log10()
    }
}

/// Equal-power stereo balance with unity gain in both channels at the center.
pub fn constant_power_pan(pan: f32) -> (f32, f32) {
    let angle = (pan.clamp(-1.0, 1.0) + 1.0) * PI / 4.0;
    let scale = 2.0_f32.sqrt();
    (angle.cos() * scale, angle.sin() * scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_reaches_requested_peak() {
        let buffer = AudioBuffer::mono(SampleRate(48_000), vec![-0.25, 0.5]);
        assert!((buffer.normalization_gain(0.0) - 2.0).abs() < 1.0e-6);
    }

    #[test]
    fn pan_is_unity_at_center_and_equal_power() {
        let (left, right) = constant_power_pan(0.0);
        assert!((left - 1.0).abs() < 1.0e-6 && (right - 1.0).abs() < 1.0e-6);
        let (left, right) = constant_power_pan(-1.0);
        assert!((left * left + right * right - 2.0).abs() < 1.0e-5);
    }

    #[test]
    fn resampling_preserves_duration() {
        let buffer = AudioBuffer::mono(SampleRate(48_000), vec![0.0; 4_800]);
        let output = buffer.resample(SampleRate(96_000)).unwrap();
        assert_eq!(output.frame_count(), 9_600);
    }
}
