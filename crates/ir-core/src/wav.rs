use crate::{AudioBuffer, AudioBufferError, SampleRate};
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WavError {
    #[error("WAV I/O failed: {0}")]
    Hound(#[from] hound::Error),
    #[error(transparent)]
    Buffer(#[from] AudioBufferError),
    #[error("only mono and stereo WAV files are supported (found {0} channels)")]
    UnsupportedChannels(u16),
    #[error("unsupported WAV format: {bits}-bit {format:?}")]
    UnsupportedFormat { bits: u16, format: SampleFormat },
    #[error("PCM export would clip at a peak of {peak:.4}")]
    WouldClip { peak: f32 },
}

pub fn read_wav(path: impl AsRef<Path>) -> Result<AudioBuffer, WavError> {
    let mut reader = WavReader::open(path)?;
    let spec = reader.spec();
    if !(1..=2).contains(&spec.channels) {
        return Err(WavError::UnsupportedChannels(spec.channels));
    }
    let mut channels = vec![Vec::with_capacity(reader.duration() as usize); spec.channels as usize];
    match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Float, 32) => {
            for (index, sample) in reader.samples::<f32>().enumerate() {
                channels[index % spec.channels as usize].push(sample?);
            }
        }
        (SampleFormat::Int, bits @ (8 | 16 | 24 | 32)) => {
            let scale = (1_i64 << (bits - 1)) as f32;
            for (index, sample) in reader.samples::<i32>().enumerate() {
                channels[index % spec.channels as usize].push(sample? as f32 / scale);
            }
        }
        (format, bits) => return Err(WavError::UnsupportedFormat { bits, format }),
    }
    Ok(AudioBuffer::new(SampleRate(spec.sample_rate), channels)?)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WavEncoding {
    Pcm16,
    Pcm24,
    Float32,
}

pub fn write_wav(
    path: impl AsRef<Path>,
    audio: &AudioBuffer,
    encoding: WavEncoding,
) -> Result<(), WavError> {
    let peak = audio.peak();
    if encoding != WavEncoding::Float32 && peak > 1.0 + 1.0e-6 {
        return Err(WavError::WouldClip { peak });
    }
    let (sample_format, bits_per_sample) = match encoding {
        WavEncoding::Pcm16 => (SampleFormat::Int, 16),
        WavEncoding::Pcm24 => (SampleFormat::Int, 24),
        WavEncoding::Float32 => (SampleFormat::Float, 32),
    };
    let spec = WavSpec {
        channels: audio.channel_count() as u16,
        sample_rate: audio.sample_rate().0,
        bits_per_sample,
        sample_format,
    };
    let mut writer = WavWriter::create(path, spec)?;
    match encoding {
        WavEncoding::Float32 => {
            for frame in 0..audio.frame_count() {
                for channel in audio.channels() {
                    writer.write_sample(channel[frame])?;
                }
            }
        }
        WavEncoding::Pcm16 => write_pcm(&mut writer, audio, 16)?,
        WavEncoding::Pcm24 => write_pcm(&mut writer, audio, 24)?,
    }
    writer.finalize()?;
    Ok(())
}

fn write_pcm<W: std::io::Write + std::io::Seek>(
    writer: &mut WavWriter<W>,
    audio: &AudioBuffer,
    bits: u16,
) -> Result<(), hound::Error> {
    let scale = ((1_i64 << (bits - 1)) - 1) as f32;
    let mut noise = Dither::new(0x4952_4d58);
    for frame in 0..audio.frame_count() {
        for channel in audio.channels() {
            let dither = (noise.next() - noise.next()) / scale;
            let quantized = ((channel[frame].clamp(-1.0, 1.0) + dither) * scale).round() as i32;
            writer.write_sample(quantized)?;
        }
    }
    Ok(())
}

struct Dither(u32);
impl Dither {
    fn new(seed: u32) -> Self {
        Self(seed)
    }
    fn next(&mut self) -> f32 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 17;
        value ^= value << 5;
        self.0 = value;
        value as f32 / u32::MAX as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm_export_rejects_clipping() {
        let audio = AudioBuffer::mono(SampleRate(48_000), vec![1.1]);
        let path = std::env::temp_dir().join("ir-mixer-clipping-test.wav");
        assert!(matches!(
            write_wav(path, &audio, WavEncoding::Pcm24),
            Err(WavError::WouldClip { .. })
        ));
    }

    #[test]
    fn all_export_encodings_write_the_requested_wav_format() {
        let audio =
            AudioBuffer::stereo(SampleRate(44_100), vec![0.25, -0.25], vec![-0.5, 0.5]).unwrap();
        for (encoding, bits, format, suffix) in [
            (WavEncoding::Pcm16, 16, SampleFormat::Int, "16"),
            (WavEncoding::Pcm24, 24, SampleFormat::Int, "24"),
            (WavEncoding::Float32, 32, SampleFormat::Float, "32f"),
        ] {
            let path = std::env::temp_dir().join(format!(
                "ir-mixer-encoding-{suffix}-{}.wav",
                std::process::id()
            ));
            write_wav(&path, &audio, encoding).unwrap();
            let spec = WavReader::open(&path).unwrap().spec();
            assert_eq!(spec.sample_rate, 44_100);
            assert_eq!(spec.channels, 2);
            assert_eq!(spec.bits_per_sample, bits);
            assert_eq!(spec.sample_format, format);
            let _ = std::fs::remove_file(path);
        }
    }
}
