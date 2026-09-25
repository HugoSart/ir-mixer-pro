use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ir_core::{AudioBuffer, SampleRate, analyze_frequency_response};
use ir_dsp::{DspEngine, EngineConfig, MeterSnapshot, PreparedSlot, SlotParameters};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};
use std::time::Instant;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub channels: usize,
    pub default_sample_rate: u32,
}

#[derive(Clone, Debug, Default)]
pub struct DeviceCatalog {
    pub inputs: Vec<DeviceInfo>,
    pub outputs: Vec<DeviceInfo>,
    pub default_input_id: Option<String>,
    pub default_output_id: Option<String>,
}

impl DeviceCatalog {
    pub fn enumerate() -> Self {
        let host = cpal::default_host();
        let default_input_name = host
            .default_input_device()
            .and_then(|device| device.name().ok());
        let default_output_name = host
            .default_output_device()
            .and_then(|device| device.name().ok());
        let inputs: Vec<DeviceInfo> = host
            .input_devices()
            .map(|devices| {
                devices
                    .enumerate()
                    .filter_map(|(index, device)| {
                        let config = device.default_input_config().ok()?;
                        Some(DeviceInfo {
                            id: format!("input-{index}"),
                            name: device.name().unwrap_or_else(|_| format!("Input {index}")),
                            channels: config.channels() as usize,
                            default_sample_rate: config.sample_rate().0,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let outputs: Vec<DeviceInfo> = host
            .output_devices()
            .map(|devices| {
                devices
                    .enumerate()
                    .filter_map(|(index, device)| {
                        let config = device.default_output_config().ok()?;
                        Some(DeviceInfo {
                            id: format!("output-{index}"),
                            name: device.name().unwrap_or_else(|_| format!("Output {index}")),
                            channels: config.channels() as usize,
                            default_sample_rate: config.sample_rate().0,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let default_input_id = default_input_name.and_then(|name| {
            inputs
                .iter()
                .find(|device| device.name == name)
                .map(|device| device.id.clone())
        });
        let default_output_id = default_output_name.and_then(|name| {
            outputs
                .iter()
                .find(|device| device.name == name)
                .map(|device| device.id.clone())
        });
        Self {
            inputs,
            outputs,
            default_input_id,
            default_output_id,
        }
    }
}

pub fn query_output_default_sample_rate(device_id: &str) -> Option<u32> {
    let index = parse_index(device_id)?;
    cpal::default_host()
        .output_devices()
        .ok()?
        .nth(index)?
        .default_output_config()
        .ok()
        .map(|config| config.sample_rate().0)
}

#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub input_device_id: String,
    pub output_device_id: String,
    pub input_channel: usize,
    pub output_channel: usize,
    pub sample_rate: u32,
    pub buffer_size: usize,
    pub max_active_irs: usize,
    pub live_input_enabled: bool,
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("input device is unavailable")]
    InputUnavailable,
    #[error("output device is unavailable")]
    OutputUnavailable,
    #[error("audio device configuration failed: {0}")]
    Configuration(String),
    #[error("audio stream failed: {0}")]
    Stream(#[from] cpal::BuildStreamError),
    #[error("audio stream could not start: {0}")]
    Play(#[from] cpal::PlayStreamError),
    #[error("audio command queue is full")]
    CommandQueueFull,
}

pub enum RuntimeCommand {
    ReplaceSlot(Box<PreparedSlot>),
    RemoveSlot(u64),
    SetSlotParameters(u64, SlotParameters),
    SetOutputGain(f32),
    SetBypassed(bool),
    SetLimiter(bool),
    SetPreview {
        audio: Box<AudioBuffer>,
        normalization_gain: f32,
    },
    SetSourceLive(bool),
    SetSourceGain(f32),
    SetPreviewNormalize(bool),
    SetMonitoring(bool),
    Play,
    Pause,
    Stop,
    Restart,
    SetLoop(bool),
}

enum Retired {
    Slot(Box<PreparedSlot>),
    Preview(Box<AudioBuffer>),
}

#[derive(Default)]
struct RuntimeMetrics {
    rms_left: AtomicU32,
    rms_right: AtomicU32,
    peak_left: AtomicU32,
    peak_right: AtomicU32,
    cpu: AtomicU32,
    latency: AtomicU32,
    xruns: AtomicU64,
    preview_frame: AtomicU64,
    preview_playing: AtomicBool,
}

pub struct AudioRuntime {
    host_commands: crossbeam_channel::Sender<HostCommand>,
    host_thread: Option<std::thread::JoinHandle<()>>,
    metrics: Arc<RuntimeMetrics>,
    stream_error: Arc<Mutex<Option<String>>>,
    spectrum: crossbeam_channel::Receiver<Vec<f32>>,
}

enum HostCommand {
    Realtime(RuntimeCommand),
    Stop,
}

struct LocalAudioRuntime {
    _input: Option<cpal::Stream>,
    _output: cpal::Stream,
    commands: Producer<RuntimeCommand>,
    retired: Consumer<Retired>,
    spectrum: Consumer<SpectrumBlock>,
    spectrum_results: crossbeam_channel::Sender<Vec<f32>>,
}

impl AudioRuntime {
    pub fn start(config: RuntimeConfig) -> Result<Self, RuntimeError> {
        let metrics = Arc::new(RuntimeMetrics::default());
        let stream_error = Arc::new(Mutex::new(None));
        let (host_tx, host_rx) = crossbeam_channel::bounded(256);
        let (started_tx, started_rx) = crossbeam_channel::bounded(1);
        let (spectrum_tx, spectrum_rx) = crossbeam_channel::bounded(2);
        let thread_metrics = Arc::clone(&metrics);
        let thread_error = Arc::clone(&stream_error);
        let host_thread = std::thread::Builder::new()
            .name("ir-mixer-audio-host".into())
            .spawn(move || {
                match LocalAudioRuntime::start(config, thread_metrics, thread_error, spectrum_tx) {
                    Ok(mut runtime) => {
                        let _ = started_tx.send(Ok(()));
                        loop {
                            match host_rx.recv_timeout(std::time::Duration::from_millis(2)) {
                                Ok(HostCommand::Realtime(command)) => {
                                    let _ = runtime.send(command);
                                }
                                Ok(HostCommand::Stop)
                                | Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                            }
                            runtime.poll();
                        }
                    }
                    Err(error) => {
                        let _ = started_tx.send(Err(error.to_string()));
                    }
                }
            })
            .map_err(|error| RuntimeError::Configuration(error.to_string()))?;
        match started_rx
            .recv()
            .map_err(|error| RuntimeError::Configuration(error.to_string()))?
        {
            Ok(()) => Ok(Self {
                host_commands: host_tx,
                host_thread: Some(host_thread),
                metrics,
                stream_error,
                spectrum: spectrum_rx,
            }),
            Err(message) => {
                let _ = host_thread.join();
                Err(RuntimeError::Configuration(message))
            }
        }
    }

    pub fn send(&mut self, command: RuntimeCommand) -> Result<(), RuntimeError> {
        self.host_commands
            .try_send(HostCommand::Realtime(command))
            .map_err(|_| RuntimeError::CommandQueueFull)
    }

    pub fn poll(&mut self) {}

    pub fn meters(&self) -> (MeterSnapshot, f32, f32, u64) {
        let load = |value: &AtomicU32| f32::from_bits(value.load(Ordering::Relaxed));
        (
            MeterSnapshot {
                rms_db: [load(&self.metrics.rms_left), load(&self.metrics.rms_right)],
                peak_db: [
                    load(&self.metrics.peak_left),
                    load(&self.metrics.peak_right),
                ],
                clipped: false,
            },
            load(&self.metrics.cpu),
            load(&self.metrics.latency),
            self.metrics.xruns.load(Ordering::Relaxed),
        )
    }

    pub fn preview_state(&self) -> (u64, bool) {
        (
            self.metrics.preview_frame.load(Ordering::Relaxed),
            self.metrics.preview_playing.load(Ordering::Relaxed),
        )
    }

    pub fn take_spectrum(&self) -> Option<Vec<f32>> {
        self.spectrum.try_iter().last()
    }

    pub fn take_stream_error(&self) -> Option<String> {
        self.stream_error.lock().ok()?.take()
    }
}

impl Drop for AudioRuntime {
    fn drop(&mut self) {
        let _ = self.host_commands.send(HostCommand::Stop);
        if let Some(thread) = self.host_thread.take() {
            let _ = thread.join();
        }
    }
}

impl LocalAudioRuntime {
    fn start(
        config: RuntimeConfig,
        metrics: Arc<RuntimeMetrics>,
        stream_error: Arc<Mutex<Option<String>>>,
        spectrum_results: crossbeam_channel::Sender<Vec<f32>>,
    ) -> Result<Self, RuntimeError> {
        let host = cpal::default_host();
        let output_index =
            parse_index(&config.output_device_id).ok_or(RuntimeError::OutputUnavailable)?;
        let output_device = host
            .output_devices()
            .ok()
            .and_then(|mut devices| devices.nth(output_index))
            .ok_or(RuntimeError::OutputUnavailable)?;
        let output_supported = output_device
            .default_output_config()
            .map_err(|error| RuntimeError::Configuration(error.to_string()))?;
        if output_supported.sample_rate().0 != config.sample_rate {
            return Err(RuntimeError::Configuration(format!(
                "the output device default rate changed from {} Hz to {} Hz; restart the audio engine",
                config.sample_rate,
                output_supported.sample_rate().0
            )));
        }
        let output_channels = output_supported.channels() as usize;
        if output_channels == 0 {
            return Err(RuntimeError::Configuration(
                "selected audio device exposes no usable channels".into(),
            ));
        }
        let input_details = if config.live_input_enabled {
            let input_index =
                parse_index(&config.input_device_id).ok_or(RuntimeError::InputUnavailable)?;
            let input_device = host
                .input_devices()
                .ok()
                .and_then(|mut devices| devices.nth(input_index))
                .ok_or(RuntimeError::InputUnavailable)?;
            let input_supported = input_device
                .default_input_config()
                .map_err(|error| RuntimeError::Configuration(error.to_string()))?;
            if input_supported.sample_rate().0 != config.sample_rate {
                return Err(RuntimeError::Configuration(format!(
                    "live input is {} Hz but the selected output is {} Hz; set both Windows default formats to the same rate",
                    input_supported.sample_rate().0,
                    config.sample_rate
                )));
            }
            if input_supported.channels() == 0 {
                return Err(RuntimeError::Configuration(
                    "selected input device exposes no usable channels".into(),
                ));
            }
            Some((input_device, input_supported))
        } else {
            None
        };
        // Device topology may change after enumeration (or a preset may retain
        // an older selection). Clamp at stream creation so a stale UI choice
        // cannot prevent preview playback from starting.
        let input_channel = input_details
            .as_ref()
            .map(|(_, supported)| config.input_channel.min(supported.channels() as usize - 1))
            .unwrap_or(0);
        let output_channel = if output_channels > 1 {
            config.output_channel.min(output_channels - 2)
        } else {
            0
        };
        let output_config = cpal::StreamConfig {
            channels: output_supported.channels(),
            sample_rate: cpal::SampleRate(config.sample_rate),
            buffer_size: cpal::BufferSize::Fixed(config.buffer_size as u32),
        };

        let (input_producer, input_consumer) =
            RingBuffer::<StereoFrame>::new(config.buffer_size * 16);
        let (command_producer, command_consumer) = RingBuffer::<RuntimeCommand>::new(256);
        let (retire_producer, retire_consumer) = RingBuffer::<Retired>::new(64);
        let (spectrum_producer, spectrum_consumer) = RingBuffer::<SpectrumBlock>::new(8);
        let input = if let Some((input_device, input_supported)) = input_details {
            let input_config = cpal::StreamConfig {
                channels: input_supported.channels(),
                sample_rate: cpal::SampleRate(config.sample_rate),
                buffer_size: cpal::BufferSize::Fixed(config.buffer_size as u32),
            };
            Some(build_input_stream(
                &input_device,
                &input_config,
                input_supported.sample_format(),
                input_producer,
                input_channel,
                Arc::clone(&metrics),
                Arc::clone(&stream_error),
            )?)
        } else {
            None
        };
        let engine_config = EngineConfig {
            sample_rate: config.sample_rate,
            block_size: config.buffer_size,
            max_active_irs: config.max_active_irs,
            ..EngineConfig::default()
        };
        let processor = OutputProcessor::new(
            engine_config,
            input_consumer,
            command_consumer,
            retire_producer,
            spectrum_producer,
            Arc::clone(&metrics),
        );
        let output = build_output_stream(
            &output_device,
            &output_config,
            output_supported.sample_format(),
            processor,
            output_channel,
            Arc::clone(&stream_error),
        )?;
        output.play()?;
        if let Some(input) = &input {
            input.play()?;
        }
        Ok(Self {
            _input: input,
            _output: output,
            commands: command_producer,
            retired: retire_consumer,
            spectrum: spectrum_consumer,
            spectrum_results,
        })
    }

    fn send(&mut self, command: RuntimeCommand) -> Result<(), RuntimeError> {
        self.commands
            .push(command)
            .map_err(|_| RuntimeError::CommandQueueFull)
    }

    fn poll(&mut self) {
        while let Ok(retired) = self.retired.pop() {
            match retired {
                Retired::Slot(slot) => drop(slot),
                Retired::Preview(preview) => drop(preview),
            }
        }
        while let Ok(block) = self.spectrum.pop() {
            let audio = AudioBuffer::mono(SampleRate(block.sample_rate), block.samples.to_vec());
            let values = analyze_frequency_response(&audio, 180).magnitude_db;
            let _ = self.spectrum_results.try_send(values);
        }
    }
}

#[derive(Clone, Copy)]
struct SpectrumBlock {
    samples: [f32; 256],
    sample_rate: u32,
}

fn parse_index(id: &str) -> Option<usize> {
    id.rsplit('-').next()?.parse().ok()
}

#[derive(Clone, Copy)]
struct StereoFrame {
    left: f32,
    right: f32,
}

fn build_input_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    format: cpal::SampleFormat,
    producer: Producer<StereoFrame>,
    selected_channel: usize,
    metrics: Arc<RuntimeMetrics>,
    error: Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream, RuntimeError> {
    match format {
        cpal::SampleFormat::F32 => Ok(input_stream_typed(
            device,
            config,
            producer,
            selected_channel,
            metrics,
            error,
            |sample: f32| sample,
        )?),
        cpal::SampleFormat::I16 => Ok(input_stream_typed(
            device,
            config,
            producer,
            selected_channel,
            metrics,
            error,
            |sample: i16| sample as f32 / 32768.0,
        )?),
        cpal::SampleFormat::U16 => Ok(input_stream_typed(
            device,
            config,
            producer,
            selected_channel,
            metrics,
            error,
            |sample: u16| sample as f32 / 32767.5 - 1.0,
        )?),
        _ => Err(RuntimeError::Configuration(format!(
            "unsupported input sample format {format:?}"
        ))),
    }
}

fn input_stream_typed<T: cpal::SizedSample>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut producer: Producer<StereoFrame>,
    selected_channel: usize,
    metrics: Arc<RuntimeMetrics>,
    error: Arc<Mutex<Option<String>>>,
    convert: impl Fn(T) -> f32 + Send + 'static + Copy,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let channels = config.channels as usize;
    device.build_input_stream(
        config,
        move |data: &[T], _| {
            for frame in data.chunks(channels) {
                let sample = frame
                    .get(selected_channel)
                    .copied()
                    .map(convert)
                    .unwrap_or(0.0);
                if producer
                    .push(StereoFrame {
                        left: sample,
                        right: sample,
                    })
                    .is_err()
                {
                    metrics.xruns.fetch_add(1, Ordering::Relaxed);
                }
            }
        },
        move |stream_error| {
            if let Ok(mut target) = error.lock() {
                *target = Some(stream_error.to_string());
            }
        },
        None,
    )
}

fn build_output_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    format: cpal::SampleFormat,
    processor: OutputProcessor,
    output_channel: usize,
    error: Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream, RuntimeError> {
    match format {
        cpal::SampleFormat::F32 => Ok(output_stream_typed(
            device,
            config,
            processor,
            output_channel,
            error,
            |sample| sample,
        )?),
        cpal::SampleFormat::I16 => Ok(output_stream_typed(
            device,
            config,
            processor,
            output_channel,
            error,
            |sample| (sample.clamp(-1.0, 1.0) * 32767.0) as i16,
        )?),
        cpal::SampleFormat::U16 => Ok(output_stream_typed(
            device,
            config,
            processor,
            output_channel,
            error,
            |sample| ((sample.clamp(-1.0, 1.0) * 0.5 + 0.5) * 65535.0) as u16,
        )?),
        _ => Err(RuntimeError::Configuration(format!(
            "unsupported output sample format {format:?}"
        ))),
    }
}

fn output_stream_typed<T: cpal::SizedSample>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut processor: OutputProcessor,
    output_channel: usize,
    error: Arc<Mutex<Option<String>>>,
    convert: impl Fn(f32) -> T + Send + 'static + Copy,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let channels = config.channels as usize;
    device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            processor.process(data, channels, output_channel, convert);
        },
        move |stream_error| {
            if let Ok(mut target) = error.lock() {
                *target = Some(stream_error.to_string());
            }
        },
        None,
    )
}

struct OutputProcessor {
    engine: DspEngine,
    input: Consumer<StereoFrame>,
    commands: Consumer<RuntimeCommand>,
    retired: Producer<Retired>,
    spectrum: Producer<SpectrumBlock>,
    pending_retired: Option<Retired>,
    metrics: Arc<RuntimeMetrics>,
    input_left: Vec<f32>,
    input_right: Vec<f32>,
    output_left: Vec<f32>,
    output_right: Vec<f32>,
    output_position: usize,
    preview: Option<Box<AudioBuffer>>,
    preview_normalization_gain: f32,
    preview_cursor: usize,
    preview_playing: bool,
    preview_looping: bool,
    source_live: bool,
    source_gain: f32,
    preview_normalize: bool,
    monitoring: bool,
    spectrum_samples: [f32; 256],
    spectrum_position: usize,
}

impl OutputProcessor {
    fn new(
        config: EngineConfig,
        input: Consumer<StereoFrame>,
        commands: Consumer<RuntimeCommand>,
        retired: Producer<Retired>,
        spectrum: Producer<SpectrumBlock>,
        metrics: Arc<RuntimeMetrics>,
    ) -> Self {
        let block = config.block_size;
        metrics.latency.store(
            (block as f32 * 2000.0 / config.sample_rate as f32 + config.limiter_lookahead_ms)
                .to_bits(),
            Ordering::Relaxed,
        );
        Self {
            engine: DspEngine::new(config),
            input,
            commands,
            retired,
            spectrum,
            pending_retired: None,
            metrics,
            input_left: vec![0.0; block],
            input_right: vec![0.0; block],
            output_left: vec![0.0; block],
            output_right: vec![0.0; block],
            output_position: block,
            preview: None,
            preview_normalization_gain: 1.0,
            preview_cursor: 0,
            preview_playing: false,
            preview_looping: false,
            source_live: false,
            source_gain: 1.0,
            preview_normalize: false,
            monitoring: false,
            spectrum_samples: [0.0; 256],
            spectrum_position: 0,
        }
    }

    fn apply_commands(&mut self) {
        if let Some(retired) = self.pending_retired.take()
            && let Err(rtrb::PushError::Full(value)) = self.retired.push(retired)
        {
            self.pending_retired = Some(value);
            return;
        }
        while let Ok(command) = self.commands.pop() {
            let retired = match command {
                RuntimeCommand::ReplaceSlot(slot) => self
                    .engine
                    .replace_boxed_slot(slot)
                    .ok()
                    .flatten()
                    .map(Retired::Slot),
                RuntimeCommand::RemoveSlot(id) => {
                    self.engine.remove_slot(id).ok().map(Retired::Slot)
                }
                RuntimeCommand::SetSlotParameters(id, params) => {
                    let _ = self.engine.set_slot_parameters(id, params);
                    None
                }
                RuntimeCommand::SetOutputGain(value) => {
                    self.engine.set_output_gain_db(value);
                    None
                }
                RuntimeCommand::SetBypassed(value) => {
                    self.engine.set_bypassed(value);
                    None
                }
                RuntimeCommand::SetLimiter(value) => {
                    self.engine.set_limiter_enabled(value);
                    None
                }
                RuntimeCommand::SetPreview {
                    audio,
                    normalization_gain,
                } => {
                    self.preview_normalization_gain = normalization_gain;
                    self.preview.replace(audio).map(Retired::Preview)
                }
                RuntimeCommand::SetSourceLive(value) => {
                    self.source_live = value;
                    None
                }
                RuntimeCommand::SetSourceGain(db) => {
                    self.source_gain = ir_core::db_to_gain(db);
                    None
                }
                RuntimeCommand::SetPreviewNormalize(value) => {
                    self.preview_normalize = value;
                    None
                }
                RuntimeCommand::SetMonitoring(value) => {
                    self.monitoring = value;
                    None
                }
                RuntimeCommand::Play => {
                    if self
                        .preview
                        .as_ref()
                        .is_some_and(|preview| self.preview_cursor >= preview.frame_count())
                    {
                        self.preview_cursor = 0;
                    }
                    self.preview_playing = true;
                    None
                }
                RuntimeCommand::Pause => {
                    self.preview_playing = false;
                    None
                }
                RuntimeCommand::Stop => {
                    self.preview_playing = false;
                    self.preview_cursor = 0;
                    None
                }
                RuntimeCommand::Restart => {
                    self.preview_cursor = 0;
                    None
                }
                RuntimeCommand::SetLoop(value) => {
                    self.preview_looping = value;
                    None
                }
            };
            if let Some(retired) = retired
                && let Err(rtrb::PushError::Full(value)) = self.retired.push(retired)
            {
                self.pending_retired = Some(value);
                break;
            }
        }
    }

    fn fill_input(&mut self) {
        if self.source_live && self.monitoring {
            let mut underflow = false;
            for index in 0..self.input_left.len() {
                let frame = self.input.pop().unwrap_or_else(|_| {
                    underflow = true;
                    StereoFrame {
                        left: 0.0,
                        right: 0.0,
                    }
                });
                self.input_left[index] = frame.left * self.source_gain;
                self.input_right[index] = frame.right * self.source_gain;
            }
            if underflow {
                self.metrics.xruns.fetch_add(1, Ordering::Relaxed);
            }
        } else if !self.source_live {
            // The peak scan used to happen here for every DSP block. For a
            // long preview file that is unbounded work on the audio thread and
            // causes severe callback overruns. The worker/control side computes
            // this once when the preview is installed.
            let normalization = if self.preview_normalize {
                self.preview_normalization_gain
            } else {
                1.0
            };
            for index in 0..self.input_left.len() {
                let mut frame = StereoFrame {
                    left: 0.0,
                    right: 0.0,
                };
                if self.preview_playing
                    && let Some(audio) = &self.preview
                {
                    if self.preview_cursor >= audio.frame_count() {
                        if self.preview_looping {
                            self.preview_cursor = 0;
                        } else {
                            self.preview_playing = false;
                        }
                    }
                    if self.preview_playing && self.preview_cursor < audio.frame_count() {
                        frame.left = audio.channel(0)[self.preview_cursor];
                        frame.right = audio.channel(if audio.channel_count() > 1 { 1 } else { 0 })
                            [self.preview_cursor];
                        self.preview_cursor += 1;
                    }
                }
                self.input_left[index] = frame.left * self.source_gain * normalization;
                self.input_right[index] = frame.right * self.source_gain * normalization;
            }
        } else {
            self.input_left.fill(0.0);
            self.input_right.fill(0.0);
        }
        self.metrics
            .preview_frame
            .store(self.preview_cursor as u64, Ordering::Relaxed);
        self.metrics
            .preview_playing
            .store(self.preview_playing, Ordering::Relaxed);
    }

    fn process<T>(
        &mut self,
        data: &mut [T],
        channels: usize,
        first_channel: usize,
        convert: impl Fn(f32) -> T + Copy,
    ) {
        for frame in data.chunks_mut(channels) {
            if self.output_position == self.output_left.len() {
                self.render_block();
                self.output_position = 0;
            }
            for sample in frame.iter_mut() {
                *sample = convert(0.0);
            }
            let left = self.output_left[self.output_position];
            let right = self.output_right[self.output_position];
            if first_channel + 1 < channels {
                frame[first_channel] = convert(left);
                frame[first_channel + 1] = convert(right);
            } else if let Some(sample) = frame.get_mut(first_channel) {
                *sample = convert(0.5 * (left + right));
            }
            self.spectrum_samples[self.spectrum_position] = 0.5 * (left + right);
            self.spectrum_position += 1;
            if self.spectrum_position == self.spectrum_samples.len() {
                let _ = self.spectrum.push(SpectrumBlock {
                    samples: self.spectrum_samples,
                    sample_rate: self.engine.config().sample_rate,
                });
                self.spectrum_position = 0;
            }
            self.output_position += 1;
        }
    }

    fn render_block(&mut self) {
        let started = Instant::now();
        self.apply_commands();
        self.fill_input();
        if self
            .engine
            .process(
                &self.input_left,
                &self.input_right,
                &mut self.output_left,
                &mut self.output_right,
            )
            .is_err()
        {
            self.output_left.fill(0.0);
            self.output_right.fill(0.0);
        }
        let meters = self.engine.meters();
        self.metrics
            .rms_left
            .store(meters.rms_db[0].to_bits(), Ordering::Relaxed);
        self.metrics
            .rms_right
            .store(meters.rms_db[1].to_bits(), Ordering::Relaxed);
        self.metrics
            .peak_left
            .store(meters.peak_db[0].to_bits(), Ordering::Relaxed);
        self.metrics
            .peak_right
            .store(meters.peak_db[1].to_bits(), Ordering::Relaxed);
        let deadline = self.output_left.len() as f32 / self.engine.config().sample_rate as f32;
        self.metrics.cpu.store(
            (started.elapsed().as_secs_f32() / deadline * 100.0).to_bits(),
            Ordering::Relaxed,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_processor_accepts_variable_callback_sizes() {
        let config = EngineConfig {
            sample_rate: 48_000,
            block_size: 64,
            ..EngineConfig::default()
        };
        let (_input_producer, input_consumer) = RingBuffer::<StereoFrame>::new(256);
        let (_command_producer, command_consumer) = RingBuffer::<RuntimeCommand>::new(16);
        let (retired_producer, _retired_consumer) = RingBuffer::<Retired>::new(16);
        let (spectrum_producer, _spectrum_consumer) = RingBuffer::<SpectrumBlock>::new(8);
        let metrics = Arc::new(RuntimeMetrics::default());
        let mut processor = OutputProcessor::new(
            config,
            input_consumer,
            command_consumer,
            retired_producer,
            spectrum_producer,
            Arc::clone(&metrics),
        );

        let mut short_callback = vec![1.0_f32; 17 * 2];
        processor.process(&mut short_callback, 2, 0, |sample| sample);
        let mut long_callback = vec![1.0_f32; 113 * 2];
        processor.process(&mut long_callback, 2, 0, |sample| sample);

        assert_eq!(metrics.xruns.load(Ordering::Relaxed), 0);
        assert!(short_callback.iter().all(|sample| *sample == 0.0));
        assert!(long_callback.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn preview_loops_when_loop_was_enabled_before_playback() {
        let config = EngineConfig {
            sample_rate: 48_000,
            block_size: 4,
            ..EngineConfig::default()
        };
        let (_input_producer, input_consumer) = RingBuffer::<StereoFrame>::new(16);
        let (mut command_producer, command_consumer) = RingBuffer::<RuntimeCommand>::new(16);
        let (retired_producer, _retired_consumer) = RingBuffer::<Retired>::new(16);
        let (spectrum_producer, _spectrum_consumer) = RingBuffer::<SpectrumBlock>::new(8);
        let metrics = Arc::new(RuntimeMetrics::default());
        let mut processor = OutputProcessor::new(
            config,
            input_consumer,
            command_consumer,
            retired_producer,
            spectrum_producer,
            Arc::clone(&metrics),
        );
        command_producer
            .push(RuntimeCommand::SetPreview {
                audio: Box::new(AudioBuffer::mono(
                    SampleRate(48_000),
                    vec![0.1, 0.2, 0.3, 0.4],
                )),
                normalization_gain: 1.0,
            })
            .unwrap();
        command_producer
            .push(RuntimeCommand::SetLoop(true))
            .unwrap();
        command_producer.push(RuntimeCommand::Play).unwrap();

        let mut output = [0.0_f32; 24];
        processor.process(&mut output, 2, 0, |sample| sample);

        assert!(metrics.preview_playing.load(Ordering::Relaxed));
        assert_eq!(metrics.preview_frame.load(Ordering::Relaxed), 4);
    }

    #[test]
    fn play_at_end_restarts_preview_from_the_beginning() {
        let config = EngineConfig {
            sample_rate: 48_000,
            block_size: 4,
            ..EngineConfig::default()
        };
        let (_input_producer, input_consumer) = RingBuffer::<StereoFrame>::new(16);
        let (mut command_producer, command_consumer) = RingBuffer::<RuntimeCommand>::new(16);
        let (retired_producer, _retired_consumer) = RingBuffer::<Retired>::new(16);
        let (spectrum_producer, _spectrum_consumer) = RingBuffer::<SpectrumBlock>::new(8);
        let metrics = Arc::new(RuntimeMetrics::default());
        let mut processor = OutputProcessor::new(
            config,
            input_consumer,
            command_consumer,
            retired_producer,
            spectrum_producer,
            Arc::clone(&metrics),
        );
        command_producer
            .push(RuntimeCommand::SetPreview {
                audio: Box::new(AudioBuffer::mono(
                    SampleRate(48_000),
                    vec![0.1, 0.2, 0.3, 0.4],
                )),
                normalization_gain: 1.0,
            })
            .unwrap();
        command_producer.push(RuntimeCommand::Play).unwrap();

        let mut output = [0.0_f32; 16];
        processor.process(&mut output, 2, 0, |sample| sample);
        assert!(!metrics.preview_playing.load(Ordering::Relaxed));
        assert_eq!(metrics.preview_frame.load(Ordering::Relaxed), 4);

        command_producer.push(RuntimeCommand::Play).unwrap();
        let mut replay = [0.0_f32; 8];
        processor.process(&mut replay, 2, 0, |sample| sample);

        assert!(metrics.preview_playing.load(Ordering::Relaxed));
        assert_eq!(metrics.preview_frame.load(Ordering::Relaxed), 4);
    }
}
