use crate::{AudioBuffer, AudioBufferError, SampleRate, constant_power_pan, db_to_gain};
use ir_eq::{EqProcessor, EqualizerState};
use thiserror::Error;

pub struct MixSource<'a> {
    pub audio: &'a AudioBuffer,
    pub enabled: bool,
    pub muted: bool,
    pub soloed: bool,
    pub gain_db: f32,
    pub delay_samples: usize,
    pub pan: f32,
    pub polarity_inverted: bool,
    pub normalize: bool,
    pub equalizer: &'a EqualizerState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportChannels {
    Mono,
    Stereo,
}

#[derive(Clone, Copy, Debug)]
pub struct RenderSettings {
    pub sample_rate: SampleRate,
    pub frames: Option<usize>,
    pub channels: ExportChannels,
    pub output_gain_db: f32,
    pub normalize: bool,
    pub normalization_target_dbfs: f32,
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error(transparent)]
    Buffer(#[from] AudioBufferError),
    #[error("the mix contains no enabled impulse responses")]
    EmptyMix,
    #[error(transparent)]
    Equalizer(#[from] ir_eq::EqError),
}

pub fn render_mix(
    sources: &[MixSource<'_>],
    settings: RenderSettings,
) -> Result<AudioBuffer, RenderError> {
    let any_solo = sources.iter().any(|source| source.enabled && source.soloed);
    let active: Vec<_> = sources
        .iter()
        .filter(|source| source.enabled && !source.muted && (!any_solo || source.soloed))
        .collect();
    if active.is_empty() {
        return Err(RenderError::EmptyMix);
    }

    let mut prepared = Vec::with_capacity(active.len());
    let mut natural_frames = 0;
    for source in active {
        let audio = source.audio.resample(settings.sample_rate)?;
        let normalization = if source.normalize {
            audio.normalization_gain(0.0)
        } else {
            1.0
        };
        let audio = apply_equalizer(&audio, source.equalizer, normalization)?;
        natural_frames = natural_frames.max(source.delay_samples + audio.frame_count());
        prepared.push((source, audio));
    }
    let frames = settings.frames.unwrap_or(natural_frames);
    let mut left = vec![0.0_f32; frames];
    let mut right = vec![0.0_f32; frames];
    let output_gain = db_to_gain(settings.output_gain_db);
    for (source, audio) in prepared {
        let (source_left, source_right) = audio.to_stereo();
        let polarity = if source.polarity_inverted { -1.0 } else { 1.0 };
        let gain = db_to_gain(source.gain_db) * polarity * output_gain;
        let (pan_left, pan_right) = constant_power_pan(source.pan);
        for frame in 0..audio.frame_count() {
            let destination = frame + source.delay_samples;
            if destination >= frames {
                break;
            }
            left[destination] += source_left[frame] * gain * pan_left;
            right[destination] += source_right[frame] * gain * pan_right;
        }
    }
    let mut channels = match settings.channels {
        ExportChannels::Stereo => vec![left, right],
        ExportChannels::Mono => vec![
            left.into_iter()
                .zip(right)
                .map(|(l, r)| 0.5 * (l + r))
                .collect(),
        ],
    };
    if settings.normalize {
        let peak = channels
            .iter()
            .flatten()
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
        if peak > f32::EPSILON {
            let gain = db_to_gain(settings.normalization_target_dbfs) / peak;
            for sample in channels.iter_mut().flatten() {
                *sample *= gain;
            }
        }
    }
    Ok(AudioBuffer::new(settings.sample_rate, channels)?)
}

fn apply_equalizer(
    audio: &AudioBuffer,
    state: &EqualizerState,
    normalization_gain: f32,
) -> Result<AudioBuffer, RenderError> {
    if state.bypassed && normalization_gain == 1.0 {
        return Ok(audio.clone());
    }
    let (source_left, source_right) = audio.to_stereo();
    let mut processor = EqProcessor::prepare(state, audio.sample_rate().0)?;
    let gain = normalization_gain
        * if state.bypassed {
            1.0
        } else {
            db_to_gain(state.output_gain_db)
        };
    let reserve = audio.frame_count() + audio.sample_rate().0 as usize * 2;
    let mut left = Vec::with_capacity(reserve);
    let mut right = Vec::with_capacity(reserve);
    for (&left_in, &right_in) in source_left.iter().zip(&source_right) {
        let (left_out, right_out) = if state.bypassed {
            (left_in, right_in)
        } else {
            processor.process_stereo(left_in, right_in)
        };
        left.push(left_out * gain);
        right.push(right_out * gain);
    }
    if !state.bypassed && !processor.is_empty() {
        let threshold = db_to_gain(-120.0);
        let mut quiet_frames = 0;
        for _ in 0..audio.sample_rate().0 as usize * 2 {
            let (left_out, right_out) = processor.process_stereo(0.0, 0.0);
            let left_out = left_out * gain;
            let right_out = right_out * gain;
            left.push(left_out);
            right.push(right_out);
            if left_out.abs().max(right_out.abs()) < threshold {
                quiet_frames += 1;
                if quiet_frames >= 256 {
                    break;
                }
            } else {
                quiet_frames = 0;
            }
        }
    }
    Ok(AudioBuffer::stereo(audio.sample_rate(), left, right)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_applies_delay_polarity_and_mono_downmix() {
        let impulse = AudioBuffer::mono(SampleRate(48_000), vec![1.0, 0.5]);
        let equalizer = EqualizerState::default();
        let source = MixSource {
            audio: &impulse,
            enabled: true,
            muted: false,
            soloed: false,
            gain_db: 0.0,
            delay_samples: 1,
            pan: 0.0,
            polarity_inverted: true,
            normalize: false,
            equalizer: &equalizer,
        };
        let output = render_mix(
            &[source],
            RenderSettings {
                sample_rate: SampleRate(48_000),
                frames: Some(4),
                channels: ExportChannels::Mono,
                output_gain_db: 0.0,
                normalize: false,
                normalization_target_dbfs: -1.0,
            },
        )
        .unwrap();
        for (actual, expected) in output.channel(0).iter().zip([0.0, -1.0, -0.5, 0.0]) {
            assert!((actual - expected).abs() < 1.0e-6);
        }
    }

    #[test]
    fn render_honors_rate_length_stereo_and_final_normalization() {
        let impulse =
            AudioBuffer::stereo(SampleRate(24_000), vec![0.25, 0.125], vec![-0.5, -0.25]).unwrap();
        let equalizer = EqualizerState::default();
        let source = MixSource {
            audio: &impulse,
            enabled: true,
            muted: false,
            soloed: false,
            gain_db: 0.0,
            delay_samples: 0,
            pan: 0.0,
            polarity_inverted: false,
            normalize: false,
            equalizer: &equalizer,
        };

        let output = render_mix(
            &[source],
            RenderSettings {
                sample_rate: SampleRate(48_000),
                frames: Some(8),
                channels: ExportChannels::Stereo,
                output_gain_db: 0.0,
                normalize: true,
                normalization_target_dbfs: -1.0,
            },
        )
        .unwrap();

        assert_eq!(output.sample_rate(), SampleRate(48_000));
        assert_eq!(output.channel_count(), 2);
        assert_eq!(output.frame_count(), 8);
        assert!((output.peak() - db_to_gain(-1.0)).abs() < 1.0e-5);
    }

    #[test]
    fn mixed_rate_export_keeps_native_target_rate_samples() {
        let high_rate = AudioBuffer::mono(
            SampleRate(96_000),
            (0..960)
                .map(|index| if index % 2 == 0 { 0.25 } else { -0.25 })
                .collect(),
        );
        let low_rate_silence = AudioBuffer::mono(SampleRate(44_100), vec![0.0; 441]);
        let settings = RenderSettings {
            sample_rate: SampleRate(96_000),
            frames: None,
            channels: ExportChannels::Mono,
            output_gain_db: 0.0,
            normalize: false,
            normalization_target_dbfs: -1.0,
        };
        let equalizer = EqualizerState::default();
        let source = |audio| MixSource {
            audio,
            enabled: true,
            muted: false,
            soloed: false,
            gain_db: 0.0,
            delay_samples: 0,
            pan: 0.0,
            polarity_inverted: false,
            normalize: false,
            equalizer: &equalizer,
        };

        let high_only = render_mix(&[source(&high_rate)], settings).unwrap();
        let mixed = render_mix(&[source(&low_rate_silence), source(&high_rate)], settings).unwrap();

        assert_eq!(mixed.sample_rate(), SampleRate(96_000));
        assert_eq!(mixed.frame_count(), high_rate.frame_count());
        assert_eq!(mixed.channels(), high_only.channels());
    }

    #[test]
    fn equalizer_extends_an_untrimmed_iir_tail_but_trim_remains_exact() {
        let impulse = AudioBuffer::mono(SampleRate(48_000), vec![1.0]);
        let equalizer = EqualizerState {
            bypassed: false,
            output_gain_db: 0.0,
            bands: vec![ir_eq::EqBand {
                id: ir_eq::EqBandId(1),
                enabled: true,
                shape: ir_eq::EqShape::LowPass,
                frequency_hz: 1_000.0,
                gain_db: 0.0,
                q: 1.0,
            }],
        };
        let source = MixSource {
            audio: &impulse,
            enabled: true,
            muted: false,
            soloed: false,
            gain_db: 0.0,
            delay_samples: 0,
            pan: 0.0,
            polarity_inverted: false,
            normalize: false,
            equalizer: &equalizer,
        };
        let settings = |frames| RenderSettings {
            sample_rate: SampleRate(48_000),
            frames,
            channels: ExportChannels::Mono,
            output_gain_db: 0.0,
            normalize: false,
            normalization_target_dbfs: -1.0,
        };
        let natural = render_mix(std::slice::from_ref(&source), settings(None)).unwrap();
        let trimmed = render_mix(&[source], settings(Some(128))).unwrap();
        assert!(natural.frame_count() > impulse.frame_count());
        assert_eq!(trimmed.frame_count(), 128);
    }
}
