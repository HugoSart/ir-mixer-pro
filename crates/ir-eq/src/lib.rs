//! Shared parametric-EQ model and biquad processing.

use biquad::{Biquad, Coefficients, DirectForm1, Hertz, Type};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MIN_FREQUENCY_HZ: f32 = 20.0;
pub const MAX_FREQUENCY_HZ: f32 = 20_000.0;
pub const MIN_GAIN_DB: f32 = -12.0;
pub const MAX_GAIN_DB: f32 = 12.0;
pub const MIN_Q: f32 = 0.1;
pub const MAX_Q: f32 = 12.0;
pub const MAX_BANDS: usize = 16;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EqBandId(pub u64);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum EqShape {
    #[default]
    Bell,
    LowShelf,
    HighShelf,
    Notch,
    HighPass,
    LowPass,
}

impl EqShape {
    pub const ALL: [Self; 6] = [
        Self::Bell,
        Self::LowShelf,
        Self::HighShelf,
        Self::Notch,
        Self::HighPass,
        Self::LowPass,
    ];

    pub fn has_gain(self) -> bool {
        matches!(self, Self::Bell | Self::LowShelf | Self::HighShelf)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Bell => "Bell",
            Self::LowShelf => "Low shelf",
            Self::HighShelf => "High shelf",
            Self::Notch => "Notch",
            Self::HighPass => "High-pass",
            Self::LowPass => "Low-pass",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EqBand {
    pub id: EqBandId,
    pub enabled: bool,
    pub shape: EqShape,
    pub frequency_hz: f32,
    pub gain_db: f32,
    pub q: f32,
}

impl EqBand {
    pub fn bell(id: EqBandId, frequency_hz: f32, gain_db: f32) -> Self {
        Self {
            id,
            enabled: true,
            shape: EqShape::Bell,
            frequency_hz: finite_or(frequency_hz, 1_000.0)
                .clamp(MIN_FREQUENCY_HZ, MAX_FREQUENCY_HZ),
            gain_db: finite_or(gain_db, 0.0).clamp(MIN_GAIN_DB, MAX_GAIN_DB),
            q: 1.0,
        }
    }

    pub fn sanitize(&mut self) {
        self.frequency_hz =
            finite_or(self.frequency_hz, 1_000.0).clamp(MIN_FREQUENCY_HZ, MAX_FREQUENCY_HZ);
        self.gain_db = finite_or(self.gain_db, 0.0).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
        self.q = finite_or(self.q, 1.0).clamp(MIN_Q, MAX_Q);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EqualizerState {
    pub bypassed: bool,
    pub output_gain_db: f32,
    pub bands: Vec<EqBand>,
}

impl EqualizerState {
    pub fn sanitize(&mut self) {
        self.output_gain_db = finite_or(self.output_gain_db, 0.0).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
        self.bands.truncate(MAX_BANDS);
        for band in &mut self.bands {
            band.sanitize();
        }
    }
}

#[derive(Debug, Error)]
pub enum EqError {
    #[error("invalid sample rate {0}")]
    InvalidSampleRate(u32),
    #[error("biquad coefficient generation failed")]
    Coefficients,
}

pub fn effective_frequency(frequency_hz: f32, sample_rate: u32) -> Result<f32, EqError> {
    if sample_rate == 0 {
        return Err(EqError::InvalidSampleRate(sample_rate));
    }
    Ok(finite_or(frequency_hz, 1_000.0)
        .clamp(MIN_FREQUENCY_HZ, MAX_FREQUENCY_HZ)
        .min(sample_rate as f32 * 0.49))
}

pub fn coefficients(band: &EqBand, sample_rate: u32) -> Result<Coefficients<f32>, EqError> {
    let frequency = effective_frequency(band.frequency_hz, sample_rate)?;
    let q = finite_or(band.q, 1.0).clamp(MIN_Q, MAX_Q);
    let gain = finite_or(band.gain_db, 0.0).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
    let kind = match band.shape {
        EqShape::Bell => Type::PeakingEQ(gain),
        EqShape::LowShelf => Type::LowShelf(gain),
        EqShape::HighShelf => Type::HighShelf(gain),
        EqShape::Notch => Type::Notch,
        EqShape::HighPass => Type::HighPass,
        EqShape::LowPass => Type::LowPass,
    };
    let sample_rate =
        Hertz::from_hz(sample_rate as f32).map_err(|_| EqError::InvalidSampleRate(sample_rate))?;
    let frequency = Hertz::from_hz(frequency).map_err(|_| EqError::Coefficients)?;
    Coefficients::from_params(kind, sample_rate, frequency, q).map_err(|_| EqError::Coefficients)
}

/// Returns the response of the enabled filter bands without EQ bypass or output trim.
///
/// This is the response shown by editors: bypass and output gain affect auditioning,
/// but do not hide or vertically offset the filter shape being edited.
pub fn filter_response_db(state: &EqualizerState, sample_rate: u32, frequency_hz: f32) -> f32 {
    let omega = 2.0 * core::f32::consts::PI * frequency_hz / sample_rate.max(1) as f32;
    let z1 = (omega.cos(), -omega.sin());
    let z2 = ((2.0 * omega).cos(), -(2.0 * omega).sin());
    let mut db = 0.0;
    for band in state
        .bands
        .iter()
        .take(MAX_BANDS)
        .filter(|band| band.enabled)
    {
        let Ok(c) = coefficients(band, sample_rate) else {
            continue;
        };
        let numerator = (c.b0 + c.b1 * z1.0 + c.b2 * z2.0, c.b1 * z1.1 + c.b2 * z2.1);
        let denominator = (1.0 + c.a1 * z1.0 + c.a2 * z2.0, c.a1 * z1.1 + c.a2 * z2.1);
        let magnitude = ((numerator.0 * numerator.0 + numerator.1 * numerator.1)
            / (denominator.0 * denominator.0 + denominator.1 * denominator.1)
                .max(f32::MIN_POSITIVE))
        .sqrt();
        db += 20.0 * magnitude.max(f32::MIN_POSITIVE).log10();
    }
    db
}

/// Returns the effective response used for level and headroom estimates.
///
/// Unlike [`filter_response_db`], this includes complete-EQ bypass and output trim.
pub fn response_db(state: &EqualizerState, sample_rate: u32, frequency_hz: f32) -> f32 {
    if state.bypassed {
        0.0
    } else {
        state.output_gain_db + filter_response_db(state, sample_rate, frequency_hz)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PreparedBand {
    pub id: EqBandId,
    coefficients: Coefficients<f32>,
    pub left: DirectForm1<f32>,
    pub right: DirectForm1<f32>,
}

#[derive(Clone, Debug, Default)]
pub struct EqProcessor {
    bands: Vec<PreparedBand>,
}

impl EqProcessor {
    pub fn prepare(state: &EqualizerState, sample_rate: u32) -> Result<Self, EqError> {
        let mut bands = Vec::with_capacity(state.bands.len().min(MAX_BANDS));
        for band in state
            .bands
            .iter()
            .take(MAX_BANDS)
            .filter(|band| band.enabled)
        {
            let coefficients = coefficients(band, sample_rate)?;
            bands.push(PreparedBand {
                id: band.id,
                coefficients,
                left: DirectForm1::new(coefficients),
                right: DirectForm1::new(coefficients),
            });
        }
        Ok(Self { bands })
    }

    pub fn seed_from(&mut self, previous: &Self) {
        for band in &mut self.bands {
            if let Some(old) = previous.bands.iter().find(|old| old.id == band.id) {
                band.left = old.left;
                band.right = old.right;
                band.left.update_coefficients(band.coefficients);
                band.right.update_coefficients(band.coefficients);
            }
        }
    }

    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        self.bands
            .iter_mut()
            .fold((left, right), |(left, right), band| {
                (band.left.run(left), band.right.run(right))
            })
    }

    pub fn is_empty(&self) -> bool {
        self.bands.is_empty()
    }
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shape_produces_finite_audio_at_q_bounds() {
        for shape in EqShape::ALL {
            for q in [MIN_Q, 1.0, MAX_Q] {
                let band = EqBand {
                    id: EqBandId(1),
                    enabled: true,
                    shape,
                    frequency_hz: 1_000.0,
                    gain_db: 6.0,
                    q,
                };
                let mut processor = EqProcessor::prepare(
                    &EqualizerState {
                        bypassed: false,
                        output_gain_db: 0.0,
                        bands: vec![band],
                    },
                    48_000,
                )
                .unwrap();
                for sample in [1.0, 0.0, 0.0, 0.0] {
                    let (left, right) = processor.process_stereo(sample, sample);
                    assert!(left.is_finite() && right.is_finite());
                }
            }
        }
    }

    #[test]
    fn sanitization_enforces_approved_limits() {
        let mut band = EqBand::bell(EqBandId(1), f32::INFINITY, -99.0);
        band.q = 99.0;
        band.sanitize();
        assert_eq!(band.frequency_hz, 1_000.0);
        assert_eq!(band.gain_db, MIN_GAIN_DB);
        assert_eq!(band.q, MAX_Q);
    }

    #[test]
    fn bell_has_requested_center_gain_and_empty_chain_is_flat() {
        let state = EqualizerState {
            bypassed: false,
            output_gain_db: 0.0,
            bands: vec![EqBand {
                id: EqBandId(1),
                enabled: true,
                shape: EqShape::Bell,
                frequency_hz: 1_000.0,
                gain_db: 6.0,
                q: 1.0,
            }],
        };
        assert!((response_db(&state, 48_000, 1_000.0) - 6.0).abs() < 0.01);
        assert_eq!(
            response_db(&EqualizerState::default(), 48_000, 1_000.0),
            0.0
        );
    }

    #[test]
    fn filter_preview_ignores_bypass_and_output_gain() {
        let mut state = EqualizerState {
            bypassed: false,
            output_gain_db: -9.0,
            bands: vec![EqBand {
                id: EqBandId(1),
                enabled: true,
                shape: EqShape::Bell,
                frequency_hz: 1_000.0,
                gain_db: 6.0,
                q: 1.0,
            }],
        };
        let visible = filter_response_db(&state, 48_000, 1_000.0);
        assert!((visible - 6.0).abs() < 0.01);
        assert!((response_db(&state, 48_000, 1_000.0) + 3.0).abs() < 0.01);

        state.bypassed = true;
        assert!((filter_response_db(&state, 48_000, 1_000.0) - visible).abs() < 0.01);
        assert_eq!(response_db(&state, 48_000, 1_000.0), 0.0);
    }

    #[test]
    fn stereo_filter_histories_are_independent() {
        let state = EqualizerState {
            bypassed: false,
            output_gain_db: 0.0,
            bands: vec![EqBand {
                id: EqBandId(1),
                enabled: true,
                shape: EqShape::LowPass,
                frequency_hz: 2_000.0,
                gain_db: 0.0,
                q: 0.71,
            }],
        };
        let mut processor = EqProcessor::prepare(&state, 48_000).unwrap();
        for index in 0..32 {
            let (left, right) = processor.process_stereo(if index == 0 { 1.0 } else { 0.0 }, 0.0);
            assert!(left.is_finite());
            assert_eq!(right, 0.0);
        }
    }

    #[test]
    fn low_sample_rates_constrain_frequency_below_nyquist() {
        let band = EqBand::bell(EqBandId(1), MAX_FREQUENCY_HZ, 3.0);
        assert!(coefficients(&band, 32_000).is_ok());
        assert_eq!(
            effective_frequency(MAX_FREQUENCY_HZ, 32_000).unwrap(),
            15_680.0
        );
    }
}
