use crate::{ConvolverError, PartitionedConvolver};
use ir_core::{AudioBuffer, constant_power_pan, db_to_gain, gain_to_db};
use ir_eq::{EqProcessor, EqualizerState};
use thiserror::Error;

#[derive(Clone, Copy, Debug)]
pub struct EngineConfig {
    pub sample_rate: u32,
    pub block_size: usize,
    pub max_active_irs: usize,
    pub max_delay_samples: usize,
    pub smoothing_ms: f32,
    pub limiter_lookahead_ms: f32,
    pub limiter_release_ms: f32,
    pub limiter_ceiling_dbfs: f32,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            block_size: 128,
            max_active_irs: 16,
            max_delay_samples: 4096,
            smoothing_ms: 10.0,
            limiter_lookahead_ms: 1.0,
            limiter_release_ms: 50.0,
            limiter_ceiling_dbfs: -0.3,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SlotParameters {
    pub enabled: bool,
    pub muted: bool,
    pub soloed: bool,
    pub gain_db: f32,
    pub delay_samples: usize,
    pub pan: f32,
    pub polarity_inverted: bool,
    pub normalize: bool,
}

impl Default for SlotParameters {
    fn default() -> Self {
        Self {
            enabled: true,
            muted: false,
            soloed: false,
            gain_db: 0.0,
            delay_samples: 0,
            pan: 0.0,
            polarity_inverted: false,
            normalize: false,
        }
    }
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error(transparent)]
    Convolver(#[from] ConvolverError),
    #[error("engine supports at most {maximum} active IRs")]
    Capacity { maximum: usize },
    #[error("IR sample rate {found} does not match engine sample rate {expected}")]
    SampleRate { expected: u32, found: u32 },
    #[error("processing buffers must all contain exactly {expected} samples")]
    BlockSize { expected: usize },
    #[error("unknown IR slot {0}")]
    UnknownSlot(u64),
    #[error(transparent)]
    Equalizer(#[from] ir_eq::EqError),
}

#[derive(Clone, Debug)]
pub struct PreparedEqualizer {
    processor: EqProcessor,
    output_gain: f32,
    bypassed: bool,
}

impl PreparedEqualizer {
    pub fn new(state: &EqualizerState, sample_rate: u32) -> Result<Self, EngineError> {
        Ok(Self {
            processor: EqProcessor::prepare(state, sample_rate)?,
            output_gain: db_to_gain(state.output_gain_db),
            bypassed: state.bypassed,
        })
    }

    fn seed_from(&mut self, previous: &Self) {
        self.processor.seed_from(&previous.processor);
    }

    fn transparent() -> Self {
        Self {
            processor: EqProcessor::default(),
            output_gain: 1.0,
            bypassed: false,
        }
    }
}

pub struct PreparedSlot {
    id: u64,
    left: PartitionedConvolver,
    right: PartitionedConvolver,
    normalization_gain: f32,
    params: SlotParameters,
    gain: SmoothedValue,
    pan_left: SmoothedValue,
    pan_right: SmoothedValue,
    polarity: SmoothedValue,
    audible_mix: SmoothedValue,
    equalizer: Box<PreparedEqualizer>,
    previous_equalizer: Option<Box<PreparedEqualizer>>,
    eq_fade_position: usize,
    eq_fade_duration: usize,
    eq_output_gain: SmoothedValue,
    eq_bypass_mix: SmoothedValue,
    delay_left: DelayLine,
    delay_right: DelayLine,
    scratch_left: Vec<f32>,
    scratch_right: Vec<f32>,
}

impl PreparedSlot {
    pub fn new(
        id: u64,
        audio: &AudioBuffer,
        config: EngineConfig,
        params: SlotParameters,
    ) -> Result<Self, EngineError> {
        Self::new_with_equalizer(id, audio, config, params, &EqualizerState::default())
    }

    pub fn new_with_equalizer(
        id: u64,
        audio: &AudioBuffer,
        config: EngineConfig,
        params: SlotParameters,
        equalizer: &EqualizerState,
    ) -> Result<Self, EngineError> {
        if audio.sample_rate().0 != config.sample_rate {
            return Err(EngineError::SampleRate {
                expected: config.sample_rate,
                found: audio.sample_rate().0,
            });
        }
        let (left, right) = audio.to_stereo();
        let smoothing =
            ((config.smoothing_ms * config.sample_rate as f32 / 1000.0).round() as usize).max(1);
        let (pan_left, pan_right) = constant_power_pan(params.pan);
        let gain = db_to_gain(params.gain_db)
            * if params.normalize {
                audio.normalization_gain(0.0)
            } else {
                1.0
            };
        Ok(Self {
            id,
            left: PartitionedConvolver::new(&left, config.block_size)?,
            right: PartitionedConvolver::new(&right, config.block_size)?,
            normalization_gain: audio.normalization_gain(0.0),
            params,
            gain: SmoothedValue::new(gain, smoothing),
            pan_left: SmoothedValue::new(pan_left, smoothing),
            pan_right: SmoothedValue::new(pan_right, smoothing),
            polarity: SmoothedValue::new(
                if params.polarity_inverted { -1.0 } else { 1.0 },
                smoothing,
            ),
            audible_mix: SmoothedValue::new(
                if params.enabled && !params.muted {
                    1.0
                } else {
                    0.0
                },
                smoothing,
            ),
            equalizer: Box::new(PreparedEqualizer::new(equalizer, config.sample_rate)?),
            previous_equalizer: None,
            eq_fade_position: smoothing,
            eq_fade_duration: smoothing,
            eq_output_gain: SmoothedValue::new(db_to_gain(equalizer.output_gain_db), smoothing),
            eq_bypass_mix: SmoothedValue::new(
                if equalizer.bypassed { 1.0 } else { 0.0 },
                smoothing,
            ),
            delay_left: DelayLine::new(config.max_delay_samples, config.block_size, smoothing),
            delay_right: DelayLine::new(config.max_delay_samples, config.block_size, smoothing),
            scratch_left: vec![0.0; config.block_size],
            scratch_right: vec![0.0; config.block_size],
        })
    }
    pub fn id(&self) -> u64 {
        self.id
    }

    fn replace_equalizer(
        &mut self,
        mut replacement: Box<PreparedEqualizer>,
    ) -> Option<Box<PreparedEqualizer>> {
        replacement.seed_from(&self.equalizer);
        self.eq_output_gain.set_target(replacement.output_gain);
        self.eq_bypass_mix
            .set_target(if replacement.bypassed { 1.0 } else { 0.0 });
        let old_current = std::mem::replace(&mut self.equalizer, replacement);
        let retired = self.previous_equalizer.replace(old_current);
        self.eq_fade_position = 0;
        retired
    }
}

pub struct DspEngine {
    config: EngineConfig,
    // Boxed slots let the callback return replaced processors to the control
    // thread without allocating or moving their large FFT state.
    #[allow(clippy::vec_box)]
    slots: Vec<Box<PreparedSlot>>,
    global_equalizer: Box<PreparedEqualizer>,
    previous_global_equalizer: Option<Box<PreparedEqualizer>>,
    global_eq_fade_position: usize,
    global_eq_fade_duration: usize,
    global_eq_output_gain: SmoothedValue,
    global_eq_bypass_mix: SmoothedValue,
    output_gain: SmoothedValue,
    bypass_mix: SmoothedValue,
    limiter_enabled: bool,
    limiter: LookaheadLimiter,
    sum_left: Vec<f32>,
    sum_right: Vec<f32>,
    meters: MeterSnapshot,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MeterSnapshot {
    pub rms_db: [f32; 2],
    pub peak_db: [f32; 2],
    pub clipped: bool,
}

impl DspEngine {
    pub fn new(config: EngineConfig) -> Self {
        let smoothing =
            ((config.smoothing_ms * config.sample_rate as f32 / 1000.0).round() as usize).max(1);
        Self {
            config,
            slots: Vec::with_capacity(config.max_active_irs),
            global_equalizer: Box::new(PreparedEqualizer::transparent()),
            previous_global_equalizer: None,
            global_eq_fade_position: smoothing,
            global_eq_fade_duration: smoothing,
            global_eq_output_gain: SmoothedValue::new(1.0, smoothing),
            global_eq_bypass_mix: SmoothedValue::new(0.0, smoothing),
            output_gain: SmoothedValue::new(1.0, smoothing),
            bypass_mix: SmoothedValue::new(0.0, smoothing),
            limiter_enabled: true,
            limiter: LookaheadLimiter::new(config),
            sum_left: vec![0.0; config.block_size],
            sum_right: vec![0.0; config.block_size],
            meters: MeterSnapshot::default(),
        }
    }
    pub fn config(&self) -> EngineConfig {
        self.config
    }
    pub fn latency_samples(&self) -> usize {
        self.limiter.lookahead
    }
    pub fn meters(&self) -> MeterSnapshot {
        self.meters
    }
    pub fn add_slot(&mut self, slot: PreparedSlot) -> Result<(), EngineError> {
        self.add_boxed_slot(Box::new(slot))
    }
    pub fn add_boxed_slot(&mut self, slot: Box<PreparedSlot>) -> Result<(), EngineError> {
        if self.slots.len() >= self.config.max_active_irs {
            return Err(EngineError::Capacity {
                maximum: self.config.max_active_irs,
            });
        }
        self.slots.push(slot);
        Ok(())
    }
    pub fn replace_boxed_slot(
        &mut self,
        replacement: Box<PreparedSlot>,
    ) -> Result<Option<Box<PreparedSlot>>, EngineError> {
        if let Some(index) = self.slots.iter().position(|slot| slot.id == replacement.id) {
            return Ok(Some(std::mem::replace(&mut self.slots[index], replacement)));
        }
        self.add_boxed_slot(replacement)?;
        Ok(None)
    }
    pub fn remove_slot(&mut self, id: u64) -> Result<Box<PreparedSlot>, EngineError> {
        let index = self
            .slots
            .iter()
            .position(|slot| slot.id == id)
            .ok_or(EngineError::UnknownSlot(id))?;
        Ok(self.slots.remove(index))
    }
    pub fn replace_slot_equalizer(
        &mut self,
        id: u64,
        replacement: Box<PreparedEqualizer>,
    ) -> Result<Option<Box<PreparedEqualizer>>, EngineError> {
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.id == id)
            .ok_or(EngineError::UnknownSlot(id))?;
        Ok(slot.replace_equalizer(replacement))
    }
    pub fn replace_global_equalizer(
        &mut self,
        mut replacement: Box<PreparedEqualizer>,
    ) -> Option<Box<PreparedEqualizer>> {
        replacement.seed_from(&self.global_equalizer);
        self.global_eq_output_gain
            .set_target(replacement.output_gain);
        self.global_eq_bypass_mix
            .set_target(if replacement.bypassed { 1.0 } else { 0.0 });
        let old_current = std::mem::replace(&mut self.global_equalizer, replacement);
        let retired = self.previous_global_equalizer.replace(old_current);
        self.global_eq_fade_position = 0;
        retired
    }
    pub fn take_retired_equalizer(&mut self) -> Option<Box<PreparedEqualizer>> {
        if self.global_eq_fade_position >= self.global_eq_fade_duration
            && self.previous_global_equalizer.is_some()
        {
            return self.previous_global_equalizer.take();
        }
        self.slots.iter_mut().find_map(|slot| {
            (slot.eq_fade_position >= slot.eq_fade_duration)
                .then(|| slot.previous_equalizer.take())
                .flatten()
        })
    }
    pub fn set_slot_parameters(
        &mut self,
        id: u64,
        params: SlotParameters,
    ) -> Result<(), EngineError> {
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.id == id)
            .ok_or(EngineError::UnknownSlot(id))?;
        slot.gain.set_target(
            db_to_gain(params.gain_db)
                * if params.normalize {
                    slot.normalization_gain
                } else {
                    1.0
                },
        );
        let (left, right) = constant_power_pan(params.pan);
        slot.pan_left.set_target(left);
        slot.pan_right.set_target(right);
        slot.polarity
            .set_target(if params.polarity_inverted { -1.0 } else { 1.0 });
        slot.params = params;
        Ok(())
    }
    pub fn set_output_gain_db(&mut self, db: f32) {
        self.output_gain.set_target(db_to_gain(db));
    }
    pub fn set_bypassed(&mut self, bypassed: bool) {
        self.bypass_mix.set_target(if bypassed { 1.0 } else { 0.0 });
    }
    pub fn set_limiter_enabled(&mut self, enabled: bool) {
        self.limiter_enabled = enabled;
    }

    pub fn process(
        &mut self,
        input_left: &[f32],
        input_right: &[f32],
        output_left: &mut [f32],
        output_right: &mut [f32],
    ) -> Result<(), EngineError> {
        let block = self.config.block_size;
        if [
            input_left.len(),
            input_right.len(),
            output_left.len(),
            output_right.len(),
        ]
        .iter()
        .any(|&len| len != block)
        {
            return Err(EngineError::BlockSize { expected: block });
        }
        self.sum_left.fill(0.0);
        self.sum_right.fill(0.0);
        let any_solo = self
            .slots
            .iter()
            .any(|slot| slot.params.enabled && slot.params.soloed);
        let any_audible_slot = self.slots.iter().any(|slot| {
            slot.params.enabled && !slot.params.muted && (!any_solo || slot.params.soloed)
        });
        for slot in &mut self.slots {
            let audible =
                slot.params.enabled && !slot.params.muted && (!any_solo || slot.params.soloed);
            slot.audible_mix.set_target(if audible { 1.0 } else { 0.0 });
            if !slot.params.enabled && slot.audible_mix.current == 0.0 {
                continue;
            }
            slot.left
                .process_block(input_left, &mut slot.scratch_left)?;
            slot.right
                .process_block(input_right, &mut slot.scratch_right)?;
            for index in 0..block {
                let dry_left = slot.scratch_left[index];
                let dry_right = slot.scratch_right[index];
                let (wet_left, wet_right) =
                    slot.equalizer.processor.process_stereo(dry_left, dry_right);
                let (wet_left, wet_right) = if let Some(previous) = &mut slot.previous_equalizer {
                    let (old_left, old_right) =
                        previous.processor.process_stereo(dry_left, dry_right);
                    let t = (slot.eq_fade_position as f32 / slot.eq_fade_duration as f32)
                        .clamp(0.0, 1.0);
                    if slot.eq_fade_position < slot.eq_fade_duration {
                        slot.eq_fade_position += 1;
                    }
                    (
                        old_left + (wet_left - old_left) * t,
                        old_right + (wet_right - old_right) * t,
                    )
                } else {
                    (wet_left, wet_right)
                };
                let gain = slot.eq_output_gain.next();
                let bypass = slot.eq_bypass_mix.next();
                slot.scratch_left[index] = wet_left * gain + (dry_left - wet_left * gain) * bypass;
                slot.scratch_right[index] =
                    wet_right * gain + (dry_right - wet_right * gain) * bypass;
            }
            let delay = slot.params.delay_samples.min(self.config.max_delay_samples);
            for index in 0..block {
                let mix = slot.audible_mix.next();
                let gain = slot.gain.next() * slot.polarity.next() * mix;
                let left = slot.delay_left.process(slot.scratch_left[index], delay)
                    * gain
                    * slot.pan_left.next();
                let right = slot.delay_right.process(slot.scratch_right[index], delay)
                    * gain
                    * slot.pan_right.next();
                self.sum_left[index] += left;
                self.sum_right[index] += right;
            }
        }
        let mut squared = [0.0_f32; 2];
        let mut peak = [0.0_f32; 2];
        let mut clipped = false;
        for index in 0..block {
            // An empty/fully inaudible rack is a transparent dry path rather
            // than silence. This keeps preview and live monitoring useful
            // before an IR is loaded or while every slot is disabled/muted.
            let mixed_left = if any_audible_slot {
                self.sum_left[index]
            } else {
                input_left[index]
            };
            let mixed_right = if any_audible_slot {
                self.sum_right[index]
            } else {
                input_right[index]
            };
            let (eq_left, eq_right) = self
                .global_equalizer
                .processor
                .process_stereo(mixed_left, mixed_right);
            let (eq_left, eq_right) = if let Some(previous) = &mut self.previous_global_equalizer {
                let (old_left, old_right) =
                    previous.processor.process_stereo(mixed_left, mixed_right);
                let t = (self.global_eq_fade_position as f32 / self.global_eq_fade_duration as f32)
                    .clamp(0.0, 1.0);
                if self.global_eq_fade_position < self.global_eq_fade_duration {
                    self.global_eq_fade_position += 1;
                }
                (
                    old_left + (eq_left - old_left) * t,
                    old_right + (eq_right - old_right) * t,
                )
            } else {
                (eq_left, eq_right)
            };
            let eq_gain = self.global_eq_output_gain.next();
            let eq_bypass = self.global_eq_bypass_mix.next();
            let wet_left = eq_left * eq_gain + (mixed_left - eq_left * eq_gain) * eq_bypass;
            let wet_right = eq_right * eq_gain + (mixed_right - eq_right * eq_gain) * eq_bypass;
            let gain = self.output_gain.next();
            let dry = self.bypass_mix.next();
            let wet_left = wet_left * gain;
            let wet_right = wet_right * gain;
            let left = wet_left + (input_left[index] - wet_left) * dry;
            let right = wet_right + (input_right[index] - wet_right) * dry;
            let (left, right) = self.limiter.process(left, right, self.limiter_enabled);
            output_left[index] = left;
            output_right[index] = right;
            squared[0] += left * left;
            squared[1] += right * right;
            peak[0] = peak[0].max(left.abs());
            peak[1] = peak[1].max(right.abs());
            clipped |= left.abs() > 1.0 || right.abs() > 1.0;
        }
        self.meters = MeterSnapshot {
            rms_db: [
                gain_to_db((squared[0] / block as f32).sqrt()),
                gain_to_db((squared[1] / block as f32).sqrt()),
            ],
            peak_db: [gain_to_db(peak[0]), gain_to_db(peak[1])],
            clipped,
        };
        Ok(())
    }
}

struct SmoothedValue {
    current: f32,
    target: f32,
    remaining: usize,
    duration: usize,
}
impl SmoothedValue {
    fn new(value: f32, duration: usize) -> Self {
        Self {
            current: value,
            target: value,
            remaining: 0,
            duration,
        }
    }
    fn set_target(&mut self, target: f32) {
        if (target - self.target).abs() > f32::EPSILON {
            self.target = target;
            self.remaining = self.duration;
        }
    }
    fn next(&mut self) -> f32 {
        if self.remaining > 0 {
            self.current += (self.target - self.current) / self.remaining as f32;
            self.remaining -= 1;
        }
        self.current
    }
}

struct DelayLine {
    samples: Vec<f32>,
    cursor: usize,
    current_delay: usize,
    target_delay: usize,
    fade: usize,
    fade_duration: usize,
}
impl DelayLine {
    fn new(max_delay: usize, block: usize, fade_duration: usize) -> Self {
        Self {
            samples: vec![0.0; max_delay + block + 1],
            cursor: 0,
            current_delay: 0,
            target_delay: 0,
            fade: fade_duration,
            fade_duration,
        }
    }
    fn process(&mut self, input: f32, delay: usize) -> f32 {
        if delay != self.target_delay {
            self.current_delay = self.effective_delay();
            self.target_delay = delay;
            self.fade = 0;
        }
        self.samples[self.cursor] = input;
        let old_read = (self.cursor + self.samples.len() - self.current_delay) % self.samples.len();
        let new_read = (self.cursor + self.samples.len() - self.target_delay) % self.samples.len();
        let t = (self.fade as f32 / self.fade_duration as f32).clamp(0.0, 1.0);
        let output = self.samples[old_read] * (1.0 - t) + self.samples[new_read] * t;
        if self.fade < self.fade_duration {
            self.fade += 1;
            if self.fade == self.fade_duration {
                self.current_delay = self.target_delay;
            }
        }
        self.cursor = (self.cursor + 1) % self.samples.len();
        output
    }
    fn effective_delay(&self) -> usize {
        if self.fade * 2 >= self.fade_duration {
            self.target_delay
        } else {
            self.current_delay
        }
    }
}

struct LookaheadLimiter {
    left: Vec<f32>,
    right: Vec<f32>,
    cursor: usize,
    lookahead: usize,
    gain: f32,
    release: f32,
    ceiling: f32,
}
impl LookaheadLimiter {
    fn new(config: EngineConfig) -> Self {
        let lookahead = ((config.limiter_lookahead_ms * config.sample_rate as f32 / 1000.0).round()
            as usize)
            .max(1);
        let release_samples =
            (config.limiter_release_ms * config.sample_rate as f32 / 1000.0).max(1.0);
        Self {
            left: vec![0.0; lookahead + 1],
            right: vec![0.0; lookahead + 1],
            cursor: 0,
            lookahead,
            gain: 1.0,
            release: (-1.0 / release_samples).exp(),
            ceiling: db_to_gain(config.limiter_ceiling_dbfs),
        }
    }
    fn process(&mut self, left: f32, right: f32, enabled: bool) -> (f32, f32) {
        self.left[self.cursor] = left;
        self.right[self.cursor] = right;
        let read = (self.cursor + 1) % self.left.len();
        let delayed = (self.left[read], self.right[read]);
        self.cursor = read;
        let target = if enabled {
            let peak = left.abs().max(right.abs());
            if peak > self.ceiling {
                self.ceiling / peak
            } else {
                1.0
            }
        } else {
            1.0
        };
        if target < self.gain {
            self.gain = target;
        } else {
            self.gain = 1.0 - (1.0 - self.gain) * self.release;
        }
        (delayed.0 * self.gain, delayed.1 * self.gain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ir_core::SampleRate;

    #[test]
    fn engine_processes_an_impulse_and_reports_meters() {
        let config = EngineConfig {
            block_size: 4,
            limiter_lookahead_ms: 0.01,
            ..EngineConfig::default()
        };
        let ir = AudioBuffer::mono(SampleRate(48_000), vec![1.0]);
        let slot = PreparedSlot::new(1, &ir, config, SlotParameters::default()).unwrap();
        let mut engine = DspEngine::new(config);
        engine.add_slot(slot).unwrap();
        engine.set_limiter_enabled(false);
        let mut left = [0.0; 4];
        let mut right = [0.0; 4];
        engine
            .process(
                &[1.0, 0.0, 0.0, 0.0],
                &[1.0, 0.0, 0.0, 0.0],
                &mut left,
                &mut right,
            )
            .unwrap();
        assert!(left.iter().any(|sample| sample.abs() > 0.9));
        assert!(engine.meters().peak_db[0].is_finite());
    }

    #[test]
    fn empty_or_inaudible_rack_passes_input_through() {
        let config = EngineConfig {
            block_size: 4,
            limiter_lookahead_ms: 0.01,
            ..EngineConfig::default()
        };
        let input = [0.25, -0.5, 0.75, -1.0];
        let mut left = [0.0; 4];
        let mut right = [0.0; 4];
        let mut engine = DspEngine::new(config);
        engine.set_limiter_enabled(false);

        engine
            .process(&input, &input, &mut left, &mut right)
            .unwrap();

        let delayed_input = [0.0, input[0], input[1], input[2]];
        assert_eq!(left, delayed_input);
        assert_eq!(right, delayed_input);

        let ir = AudioBuffer::mono(SampleRate(48_000), vec![1.0]);
        let muted = SlotParameters {
            muted: true,
            ..SlotParameters::default()
        };
        let mut engine = DspEngine::new(config);
        engine.set_limiter_enabled(false);
        engine
            .add_slot(PreparedSlot::new(1, &ir, config, muted).unwrap())
            .unwrap();
        engine
            .process(&input, &input, &mut left, &mut right)
            .unwrap();

        assert_eq!(left, delayed_input);
        assert_eq!(right, delayed_input);
    }

    #[test]
    fn warmed_up_processing_does_not_allocate_or_deallocate() {
        let config = EngineConfig {
            block_size: 64,
            ..EngineConfig::default()
        };
        let ir = AudioBuffer::mono(SampleRate(48_000), vec![1.0; 2048]);
        let mut engine = DspEngine::new(config);
        engine
            .add_slot(PreparedSlot::new(1, &ir, config, SlotParameters::default()).unwrap())
            .unwrap();
        let input = [0.0; 64];
        let mut left = [0.0; 64];
        let mut right = [0.0; 64];
        engine
            .process(&input, &input, &mut left, &mut right)
            .unwrap();
        crate::test_alloc::start();
        for _ in 0..32 {
            engine
                .process(&input, &input, &mut left, &mut right)
                .unwrap();
        }
        let operations = crate::test_alloc::stop();
        assert_eq!(operations, 0, "audio processing performed heap operations");
    }

    #[test]
    fn equalizer_transition_processes_without_heap_operations_and_retires_old_chain() {
        let config = EngineConfig {
            block_size: 64,
            smoothing_ms: 1.0,
            ..EngineConfig::default()
        };
        let ir = AudioBuffer::mono(SampleRate(48_000), vec![1.0; 256]);
        let mut engine = DspEngine::new(config);
        engine
            .add_slot(PreparedSlot::new(1, &ir, config, SlotParameters::default()).unwrap())
            .unwrap();
        let state = EqualizerState {
            bypassed: false,
            output_gain_db: -1.0,
            bands: vec![ir_eq::EqBand {
                id: ir_eq::EqBandId(1),
                enabled: true,
                shape: ir_eq::EqShape::Bell,
                frequency_hz: 2_000.0,
                gain_db: 4.0,
                q: 2.0,
            }],
        };
        let replacement = Box::new(PreparedEqualizer::new(&state, config.sample_rate).unwrap());
        assert!(
            engine
                .replace_slot_equalizer(1, replacement)
                .unwrap()
                .is_none()
        );
        let input = [0.1; 64];
        let mut left = [0.0; 64];
        let mut right = [0.0; 64];
        crate::test_alloc::start();
        engine
            .process(&input, &input, &mut left, &mut right)
            .unwrap();
        let retired = engine.take_retired_equalizer();
        let operations = crate::test_alloc::stop();
        assert_eq!(operations, 0, "EQ transition performed heap operations");
        assert!(retired.is_some());
        assert!(left.iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn global_equalizer_transition_is_finite_allocation_free_and_retired() {
        let config = EngineConfig {
            block_size: 64,
            smoothing_ms: 1.0,
            limiter_lookahead_ms: 0.01,
            ..EngineConfig::default()
        };
        let ir = AudioBuffer::mono(SampleRate(48_000), vec![1.0]);
        let mut engine = DspEngine::new(config);
        engine
            .add_slot(PreparedSlot::new(1, &ir, config, SlotParameters::default()).unwrap())
            .unwrap();
        engine.set_limiter_enabled(false);
        let state = EqualizerState {
            bypassed: false,
            output_gain_db: -2.0,
            bands: vec![ir_eq::EqBand {
                id: ir_eq::EqBandId(1),
                enabled: true,
                shape: ir_eq::EqShape::HighShelf,
                frequency_hz: 3_000.0,
                gain_db: 5.0,
                q: 0.8,
            }],
        };
        let replacement = Box::new(PreparedEqualizer::new(&state, config.sample_rate).unwrap());
        assert!(engine.replace_global_equalizer(replacement).is_none());
        let input = [0.1; 64];
        let mut left = [0.0; 64];
        let mut right = [0.0; 64];

        crate::test_alloc::start();
        engine
            .process(&input, &input, &mut left, &mut right)
            .unwrap();
        let retired = engine.take_retired_equalizer();
        let operations = crate::test_alloc::stop();

        assert_eq!(
            operations, 0,
            "global EQ transition performed heap operations"
        );
        assert!(retired.is_some());
        assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
    }
}
