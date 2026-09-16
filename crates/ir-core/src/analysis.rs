use crate::{AudioBuffer, gain_to_db};
use rustfft::{FftPlanner, num_complex::Complex32};

#[derive(Clone, Debug, PartialEq)]
pub struct FrequencyAnalysis {
    pub frequencies_hz: Vec<f32>,
    pub magnitude_db: Vec<f32>,
    pub phase_degrees: Vec<f32>,
}

pub fn analyze_frequency_response(audio: &AudioBuffer, points: usize) -> FrequencyAnalysis {
    if points == 0 || audio.frame_count() == 0 {
        return FrequencyAnalysis {
            frequencies_hz: Vec::new(),
            magnitude_db: Vec::new(),
            phase_degrees: Vec::new(),
        };
    }
    let fft_len = (audio.frame_count().max(2048) * 2).next_power_of_two();
    let mut buffer = vec![Complex32::ZERO; fft_len];
    for channel in audio.channels() {
        let mut channel_fft = vec![Complex32::ZERO; fft_len];
        for (bin, &sample) in channel_fft.iter_mut().zip(channel) {
            bin.re = sample;
        }
        FftPlanner::new()
            .plan_fft_forward(fft_len)
            .process(&mut channel_fft);
        for (sum, value) in buffer.iter_mut().zip(channel_fft) {
            *sum += value / audio.channel_count() as f32;
        }
    }
    let nyquist = audio.sample_rate().0 as f32 * 0.5;
    let minimum = 20.0_f32.min(nyquist);
    let maximum = 20_000.0_f32.min(nyquist);
    let mut result = FrequencyAnalysis {
        frequencies_hz: Vec::with_capacity(points),
        magnitude_db: Vec::with_capacity(points),
        phase_degrees: Vec::with_capacity(points),
    };
    for index in 0..points {
        let t = index as f32 / points.saturating_sub(1).max(1) as f32;
        let frequency = minimum * (maximum / minimum).powf(t);
        let bin = ((frequency / audio.sample_rate().0 as f32) * fft_len as f32).round() as usize;
        let value = buffer[bin.min(fft_len / 2)];
        result.frequencies_hz.push(frequency);
        result
            .magnitude_db
            .push(gain_to_db(value.norm()).max(-120.0));
        result.phase_degrees.push(value.arg().to_degrees());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SampleRate;

    #[test]
    fn impulse_has_flat_magnitude_and_zero_phase() {
        let impulse = AudioBuffer::mono(SampleRate(48_000), vec![1.0]);
        let result = analyze_frequency_response(&impulse, 32);
        assert!(result.magnitude_db.iter().all(|value| value.abs() < 1.0e-5));
        assert!(
            result
                .phase_degrees
                .iter()
                .all(|value| value.abs() < 1.0e-5)
        );
    }
}
