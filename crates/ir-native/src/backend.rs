use crate::dialog::{self, DialogRequest, DialogResult};
use crate::runtime::{
    AudioRuntime, DeviceCatalog, RuntimeCommand, RuntimeConfig, query_output_default_sample_rate,
};
use crate::worker::{self, ExportSource, WorkerRequest, WorkerResult};
use crossbeam_channel::{Receiver, Sender};
use ir_app::{
    AnalysisTrace, AppCommand, AppSnapshot, AudioBackend, BackendEvent, Choice, ContentState,
    EqBand, EqBandId, EqualizerState, EqualizerTarget, ExportState, IrFileReference, IrId,
    IrSlotState, MockAudioBackend, OptionId, PresetDocument, SourceMode, TransportState,
};
use ir_core::{AudioBuffer, ExportChannels, RenderSettings, SampleRate, WavEncoding};
use ir_dsp::{EngineConfig, PreparedEqualizer, SlotParameters};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct NativeAudioBackend {
    snapshot: AppSnapshot,
    events: Vec<BackendEvent>,
    runtime: Option<AudioRuntime>,
    device_catalog: DeviceCatalog,
    worker_tx: Sender<WorkerRequest>,
    worker_rx: Receiver<WorkerResult>,
    dialog_tx: Sender<DialogRequest>,
    dialog_rx: Receiver<DialogResult>,
    dialog_open: bool,
    // Native-rate decoded IRs are the source of truth for metadata, analysis,
    // export, and every real-time engine rebuild.
    original_audio: HashMap<IrId, Arc<AudioBuffer>>,
    preview_audio: Option<Arc<AudioBuffer>>,
    prepared_slots: HashMap<IrId, (u64, Box<ir_dsp::PreparedSlot>)>,
    preparing_irs: HashSet<IrId>,
    prepared_preview: Option<(u64, Arc<AudioBuffer>)>,
    preparing_preview: Option<u64>,
    engine_generation: u64,
    next_ir_id: u64,
    preset_paths: HashMap<OptionId, PathBuf>,
    active_preset_path: Option<PathBuf>,
    max_active_irs: usize,
    analysis_generation: u64,
    analysis_dirty: bool,
    analysis_in_flight: bool,
    preview_start_pending: bool,
    export_after_destination: bool,
    eq_clipboard: Option<EqualizerState>,
}

impl Default for NativeAudioBackend {
    fn default() -> Self {
        let mock = MockAudioBackend::default();
        let mut snapshot = mock.snapshot().clone();
        let catalog = DeviceCatalog::enumerate();
        snapshot.project.ir_slots.clear();
        snapshot.project.selected_ir = None;
        snapshot.project.name = "Untitled Mix".into();
        snapshot.project.dirty = false;
        snapshot.project.source.filename = None;
        snapshot.project.source.file_path = None;
        snapshot.project.source.waveform.clear();
        snapshot.project.source.metadata = "Choose a dry guitar WAV file".into();
        snapshot.project.source.transport = TransportState::Stopped;
        snapshot.project.source.monitoring = false;
        snapshot.project.source.content_state = ContentState::Ready;
        snapshot.project.export.destination = None;
        snapshot.project.export.state = ExportState::Idle;
        snapshot.input_devices = catalog
            .inputs
            .iter()
            .map(|device| Choice::new(&device.id, &device.name))
            .collect();
        snapshot.output_devices = catalog
            .outputs
            .iter()
            .map(|device| Choice::new(&device.id, &device.name))
            .collect();
        if let Some(device) = catalog
            .default_input_id
            .as_ref()
            .and_then(|id| {
                snapshot
                    .input_devices
                    .iter()
                    .find(|choice| choice.id.0 == *id)
            })
            .or_else(|| snapshot.input_devices.first())
        {
            snapshot.project.source.device = device.id.clone();
        }
        if let Some(device) = catalog
            .default_output_id
            .as_ref()
            .and_then(|id| {
                snapshot
                    .output_devices
                    .iter()
                    .find(|choice| choice.id.0 == *id)
            })
            .or_else(|| snapshot.output_devices.first())
        {
            snapshot.project.output.device = device.id.clone();
        }
        snapshot.input_channels = input_channel_choices(&catalog, &snapshot.project.source.device);
        snapshot.output_channels =
            output_channel_choices(&catalog, &snapshot.project.output.device);
        if let Some(channel) = snapshot.input_channels.first() {
            snapshot.project.source.channel = channel.id.clone();
        }
        if let Some(channel) = snapshot.output_channels.first() {
            snapshot.project.output.channel = channel.id.clone();
        }
        if let Some(rate) = output_default_sample_rate(&catalog, &snapshot.project.output.device) {
            snapshot.project.source.sample_rate = OptionId(rate.to_string());
        }
        snapshot.output_levels_db = [f32::NEG_INFINITY; 2];
        snapshot.output_peaks_db = [f32::NEG_INFINITY; 2];
        snapshot.frequency_traces.clear();
        snapshot.phase_traces.clear();
        snapshot.spectrum_traces.clear();
        snapshot.combined_waveform.clear();
        snapshot.cpu_percent = 0.0;
        snapshot.latency_ms = 0.0;
        snapshot.status = if catalog.outputs.is_empty() {
            "No compatible output devices found".into()
        } else {
            "Audio Engine Stopped".into()
        };
        snapshot.status_is_error = catalog.outputs.is_empty();
        let (worker_tx, worker_rx) = worker::spawn();
        let (dialog_tx, dialog_rx) = dialog::spawn();
        let (presets, preset_paths) = discover_presets();
        snapshot.presets = presets;
        snapshot.active_preset = snapshot
            .presets
            .first()
            .map(|choice| choice.id.clone())
            .unwrap_or_default();
        Self {
            snapshot,
            events: Vec::new(),
            runtime: None,
            device_catalog: catalog,
            worker_tx,
            worker_rx,
            dialog_tx,
            dialog_rx,
            dialog_open: false,
            original_audio: HashMap::new(),
            preview_audio: None,
            prepared_slots: HashMap::new(),
            preparing_irs: HashSet::new(),
            prepared_preview: None,
            preparing_preview: None,
            engine_generation: 1,
            next_ir_id: 1,
            preset_paths,
            active_preset_path: None,
            max_active_irs: 16,
            analysis_generation: 0,
            analysis_dirty: false,
            analysis_in_flight: false,
            preview_start_pending: false,
            export_after_destination: false,
            eq_clipboard: None,
        }
    }
}

impl AudioBackend for NativeAudioBackend {
    fn snapshot(&self) -> &AppSnapshot {
        &self.snapshot
    }

    fn dispatch(&mut self, command: AppCommand) {
        self.apply(command);
    }

    fn update(&mut self, _now_seconds: f64) {
        while let Ok(result) = self.dialog_rx.try_recv() {
            self.handle_dialog_result(result);
        }
        while let Ok(result) = self.worker_rx.try_recv() {
            self.handle_worker_result(result);
        }
        if self.analysis_dirty && !self.analysis_in_flight {
            self.analysis_dirty = false;
            self.request_analysis();
        }
        if let Some(runtime) = &mut self.runtime {
            runtime.poll();
            let (meters, cpu, latency, xruns) = runtime.meters();
            self.snapshot.output_levels_db = meters.rms_db;
            self.snapshot.output_peaks_db = meters.peak_db;
            self.snapshot.cpu_percent = cpu;
            self.snapshot.latency_ms = latency;
            let (preview_frame, preview_playing) = runtime.preview_state();
            if preview_playing {
                self.preview_start_pending = false;
            }
            if let Some(values) = runtime.take_spectrum() {
                self.snapshot.spectrum_traces = vec![AnalysisTrace {
                    label: "Output".into(),
                    values,
                    color_index: Some(0),
                    emphasized: true,
                }];
            }
            self.snapshot.project.source.elapsed_seconds = preview_frame as f64
                / selected_u32(&self.snapshot.project.source.sample_rate, 48_000) as f64;
            if preview_has_ended(
                self.snapshot.project.source.transport,
                self.snapshot.project.source.mode,
                preview_playing,
                self.preview_start_pending,
            ) {
                self.snapshot.project.source.transport = TransportState::Stopped;
            }
            if let Some(error) = runtime.take_stream_error() {
                self.snapshot.status = format!("Audio device error: {error}");
                self.snapshot.status_is_error = true;
                self.preview_start_pending = false;
                self.events
                    .push(BackendEvent::StatusChanged(self.snapshot.status.clone()));
                self.runtime = None;
            } else if xruns > 0 {
                self.snapshot.status = format!("Audio Engine Running · {xruns} xruns");
                self.snapshot.status_is_error = false;
            }
        }
    }

    fn drain_events(&mut self) -> Vec<BackendEvent> {
        std::mem::take(&mut self.events)
    }
}

impl NativeAudioBackend {
    pub fn load_ir_path(&mut self, path: PathBuf) -> IrId {
        self.queue_ir(path, None)
    }

    pub fn load_preview_path(&mut self, path: PathBuf) {
        self.snapshot.file_load_activity.preview_loading = true;
        self.snapshot.project.source.content_state = ContentState::Loading {
            message: "Loading preview WAV".into(),
        };
        self.preparing_preview = Some(self.engine_generation);
        let _ = self.worker_tx.send(WorkerRequest::LoadPreview {
            path,
            generation: self.engine_generation,
            sample_rate: SampleRate(self.engine_config().sample_rate),
        });
    }

    pub fn export_to_path(&mut self, path: PathBuf) {
        let path = ensure_wav_extension(path);
        self.snapshot.project.export.filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Mixed_IR.wav")
            .into();
        self.snapshot.project.export.destination = Some(path);
        self.export();
    }

    fn apply(&mut self, command: AppCommand) {
        use AppCommand::*;
        let changes_analysis = command_changes_analysis(&command);
        let changes_project = !matches!(
            &command,
            SavePreset
                | SavePresetAs
                | SelectPreset(_)
                | PlayPreview
                | PausePreview
                | StopPreview
                | RestartPreview
                | Export
                | SelectEqBand(_, _)
                | CopyEq(_)
        );
        match command {
            BrowsePreview => self.choose_preview(),
            AddIr => self.choose_irs(None),
            ReplaceIr(id) => self.choose_irs(Some(id)),
            ClearAllIrs => self.clear_irs(),
            RemoveIr(id) => self.remove_ir(id),
            SetBalanceMode(value) => {
                self.snapshot.project.set_balance_mode(value);
                self.sync_balance_slot_parameters();
            }
            MoveIrUp(id) => self.move_ir(id, -1),
            MoveIrDown(id) => self.move_ir(id, 1),
            SelectIr(id) => self.snapshot.project.selected_ir = Some(id),
            SetIrEnabled(id, value) => {
                if value
                    && self
                        .snapshot
                        .project
                        .ir_slots
                        .iter()
                        .filter(|slot| slot.enabled)
                        .count()
                        >= self.max_active_irs
                {
                    self.fail(format!(
                        "A maximum of {} IRs can be enabled in real time",
                        self.max_active_irs
                    ));
                } else {
                    self.update_slot(id, |slot| slot.enabled = value);
                }
            }
            SetIrGainDb(id, value) => self.update_slot(id, |slot| slot.gain_db = value),
            SetIrBalancePercent(id, value) => {
                if self.snapshot.project.set_balance_percent(id, value) {
                    self.sync_balance_slot_parameters();
                }
            }
            SetIrDelaySamples(id, value) => {
                self.update_slot(id, |slot| slot.delay_samples = value.clamp(0, 4096))
            }
            SetIrPan(id, value) => self.update_slot(id, |slot| slot.pan = value.clamp(-1.0, 1.0)),
            SetIrPolarity(id, value) => self.update_slot(id, |slot| slot.polarity_inverted = value),
            SetIrNormalize(id, value) => self.update_slot(id, |slot| slot.normalize = value),
            SetIrSolo(id, value) => self.update_slot(id, |slot| slot.soloed = value),
            SetIrMute(id, value) => self.update_slot(id, |slot| slot.muted = value),
            SelectEqBand(target, band) => {
                self.snapshot.selected_eq = band.map(|band| (target, band))
            }
            AddEqBand(target, frequency, gain) => {
                let at_capacity = self
                    .equalizer(target)
                    .is_some_and(|equalizer| equalizer.bands.len() >= ir_eq::MAX_BANDS);
                if at_capacity {
                    self.fail(format!(
                        "An equalizer supports at most {} bands",
                        ir_eq::MAX_BANDS
                    ));
                } else {
                    let next = self
                        .equalizer(target)
                        .and_then(|equalizer| {
                            equalizer.bands.iter().map(|band| band.id.0).max()
                        })
                        .unwrap_or(0)
                        + 1;
                    self.update_equalizer(target, |equalizer| {
                        equalizer
                            .bands
                            .push(EqBand::bell(EqBandId(next), frequency, gain))
                    });
                    self.snapshot.selected_eq = Some((target, EqBandId(next)));
                }
            }
            RemoveEqBand(target, band) => {
                self.update_equalizer(target, |equalizer| {
                    equalizer.bands.retain(|item| item.id != band)
                });
                if self.snapshot.selected_eq == Some((target, band)) {
                    self.snapshot.selected_eq = None;
                }
            }
            SetEqBandEnabled(target, band, value) => {
                self.update_eq_band(target, band, |item| item.enabled = value)
            }
            SetEqBandShape(target, band, value) => {
                self.update_eq_band(target, band, |item| item.shape = value)
            }
            SetEqBandFrequency(target, band, value) => {
                self.update_eq_band(target, band, |item| item.frequency_hz = value)
            }
            SetEqBandGain(target, band, value) => {
                self.update_eq_band(target, band, |item| item.gain_db = value)
            }
            SetEqBandQ(target, band, value) => {
                self.update_eq_band(target, band, |item| item.q = value)
            }
            SetEqBypassed(target, value) => {
                self.update_equalizer(target, |equalizer| equalizer.bypassed = value)
            }
            SetEqOutputGainDb(target, value) => {
                self.update_equalizer(target, |equalizer| equalizer.output_gain_db = value)
            }
            ResetEq(target) => {
                self.update_equalizer(target, |equalizer| {
                    *equalizer = EqualizerState::default()
                });
                self.snapshot.selected_eq = None;
            }
            CopyEq(target) => {
                self.eq_clipboard = self.equalizer(target).cloned();
                self.snapshot.eq_clipboard_available = self.eq_clipboard.is_some();
            }
            PasteEq(target) => {
                if let Some(mut equalizer) = self.eq_clipboard.clone() {
                    equalizer.bands.truncate(ir_eq::MAX_BANDS);
                    for (index, band) in equalizer.bands.iter_mut().enumerate() {
                        band.id = EqBandId(index as u64 + 1);
                    }
                    self.update_equalizer(target, |destination| *destination = equalizer);
                    self.snapshot.selected_eq = None;
                }
            }
            NormalizeAllIrs => {
                let ids: Vec<_> = self
                    .snapshot
                    .project
                    .ir_slots
                    .iter()
                    .map(|slot| slot.id)
                    .collect();
                for id in ids {
                    self.update_slot(id, |slot| slot.normalize = true);
                }
            }
            SetSourceMode(mode) => {
                if self.snapshot.project.source.mode != mode {
                    self.snapshot.project.source.mode = mode;
                    self.restart_runtime();
                }
            }
            RestartPreview => {
                self.snapshot.project.source.elapsed_seconds = 0.0;
                self.send(RuntimeCommand::Restart);
            }
            PlayPreview => {
                self.snapshot.project.source.transport = TransportState::Playing;
                self.preview_start_pending = true;
                self.ensure_runtime();
                if self.runtime.is_some() {
                    self.send(RuntimeCommand::Play);
                }
            }
            PausePreview => {
                self.snapshot.project.source.transport = TransportState::Paused;
                self.preview_start_pending = false;
                self.send(RuntimeCommand::Pause);
            }
            StopPreview => {
                self.snapshot.project.source.transport = TransportState::Stopped;
                self.snapshot.project.source.elapsed_seconds = 0.0;
                self.preview_start_pending = false;
                self.send(RuntimeCommand::Stop);
            }
            SetLoop(value) => {
                self.snapshot.project.source.looping = value;
                self.send(RuntimeCommand::SetLoop(value));
            }
            SetInputGainDb(value) => {
                self.snapshot.project.source.gain_db = value;
                self.send(RuntimeCommand::SetSourceGain(value));
            }
            SetInputNormalize(value) => {
                self.snapshot.project.source.normalize = value;
                self.send(RuntimeCommand::SetPreviewNormalize(value));
            }
            SetMonitoring(value) => {
                self.snapshot.project.source.monitoring = value;
                if self.snapshot.project.source.mode == SourceMode::Live {
                    // Input stream ownership changes with monitoring so an idle
                    // capture callback cannot fill an unconsumed ring buffer.
                    self.restart_runtime();
                } else if value {
                    self.ensure_runtime();
                } else {
                    self.send(RuntimeCommand::SetMonitoring(false));
                }
            }
            SetInputDevice(id) => {
                self.snapshot.project.source.device = id;
                self.refresh_input_channels();
                self.restart_runtime();
            }
            SetOutputDevice(id) => {
                self.snapshot.project.output.device = id;
                self.refresh_output_channels();
                self.refresh_output_sample_rate();
                self.reconcile_output_sample_rate();
                self.restart_runtime();
            }
            SetInputChannel(id) => {
                self.snapshot.project.source.channel = id;
                self.restart_runtime();
            }
            SetOutputChannel(id) => {
                self.snapshot.project.output.channel = id;
                self.restart_runtime();
            }
            SetInputBufferSize(id) => {
                self.snapshot.project.source.buffer_size = id;
                self.snapshot.project.output.buffer_size =
                    self.snapshot.project.source.buffer_size.clone();
                self.restart_runtime();
            }
            SetOutputBufferSize(id) => {
                self.snapshot.project.output.buffer_size = id;
                self.snapshot.project.source.buffer_size =
                    self.snapshot.project.output.buffer_size.clone();
                self.restart_runtime();
            }
            SetOutputGainDb(value) => {
                self.snapshot.project.output.gain_db = value;
                self.send(RuntimeCommand::SetOutputGain(value));
            }
            SetBypassed(value) => {
                self.snapshot.project.output.bypassed = value;
                self.send(RuntimeCommand::SetBypassed(value));
            }
            SetLimitOutput(value) => {
                self.snapshot.project.output.limit_output = value;
                self.send(RuntimeCommand::SetLimiter(value));
            }
            ChooseExportDestination => {
                self.export_after_destination = false;
                self.choose_export_destination();
            }
            Export => self.export(),
            SetExportSampleRate(id) => self.snapshot.project.export.sample_rate = id,
            SetExportBitDepth(id) => self.snapshot.project.export.bit_depth = id,
            SetExportChannelMode(id) => self.snapshot.project.export.channel_mode = id,
            SetExportLength(id) => self.snapshot.project.export.length = id,
            SetTrimToLength(value) => self.snapshot.project.export.trim_to_length = value,
            SetExportNormalize(value) => self.snapshot.project.export.normalize = value,
            SavePreset => self.save_preset(false),
            SavePresetAs => self.save_preset(true),
            SelectPreset(id) => self.load_preset(id),
            DeletePreset => self.delete_preset(),
            OpenSettings => {
                self.snapshot.status = "Settings UI is not implemented yet".into();
                self.snapshot.status_is_error = false;
            }
            SetAnalysisTab(tab) => self.snapshot.project.analysis.tab = tab,
            SetAnalysisViewMode(id) => self.snapshot.project.analysis.view_mode = id,
            SetSmoothing(id) => self.snapshot.project.analysis.smoothing = id,
        }
        if changes_project {
            self.snapshot.project.dirty = true;
        }
        if changes_analysis {
            self.analysis_dirty = true;
        }
    }

    fn engine_config(&self) -> EngineConfig {
        EngineConfig {
            sample_rate: selected_u32(&self.snapshot.project.source.sample_rate, 48_000),
            block_size: selected_usize(&self.snapshot.project.output.buffer_size, 128),
            max_active_irs: self.max_active_irs,
            ..EngineConfig::default()
        }
    }

    fn runtime_config(&self) -> RuntimeConfig {
        RuntimeConfig {
            input_device_id: self.snapshot.project.source.device.0.clone(),
            output_device_id: self.snapshot.project.output.device.0.clone(),
            input_channel: selected_index(&self.snapshot.project.source.channel),
            output_channel: selected_index(&self.snapshot.project.output.channel),
            sample_rate: selected_u32(&self.snapshot.project.source.sample_rate, 48_000),
            buffer_size: selected_usize(&self.snapshot.project.output.buffer_size, 128),
            max_active_irs: self.max_active_irs,
            live_input_enabled: self.snapshot.project.source.mode == SourceMode::Live
                && self.snapshot.project.source.monitoring,
        }
    }

    fn reconcile_output_sample_rate(&mut self) -> bool {
        let Some(rate) =
            output_default_sample_rate(&self.device_catalog, &self.snapshot.project.output.device)
        else {
            self.fail("Selected output device has no default sample rate".into());
            return false;
        };
        let changed = selected_u32(&self.snapshot.project.source.sample_rate, 0) != rate;
        self.snapshot.project.source.sample_rate = OptionId(rate.to_string());
        for slot in &mut self.snapshot.project.ir_slots {
            slot.sample_rate_hz = rate as f32;
        }
        if changed {
            self.analysis_dirty = true;
        }
        true
    }

    fn refresh_output_sample_rate(&mut self) {
        let device_id = &self.snapshot.project.output.device.0;
        if let Some(rate) = query_output_default_sample_rate(device_id)
            && let Some(device) = self
                .device_catalog
                .outputs
                .iter_mut()
                .find(|device| device.id == *device_id)
        {
            device.default_sample_rate = rate;
        }
    }

    fn refresh_input_channels(&mut self) {
        self.snapshot.input_channels =
            input_channel_choices(&self.device_catalog, &self.snapshot.project.source.device);
        if !self
            .snapshot
            .input_channels
            .iter()
            .any(|choice| choice.id == self.snapshot.project.source.channel)
        {
            self.snapshot.project.source.channel = self
                .snapshot
                .input_channels
                .first()
                .map(|choice| choice.id.clone())
                .unwrap_or_default();
        }
    }

    fn refresh_output_channels(&mut self) {
        self.snapshot.output_channels =
            output_channel_choices(&self.device_catalog, &self.snapshot.project.output.device);
        if !self
            .snapshot
            .output_channels
            .iter()
            .any(|choice| choice.id == self.snapshot.project.output.channel)
        {
            self.snapshot.project.output.channel = self
                .snapshot
                .output_channels
                .first()
                .map(|choice| choice.id.clone())
                .unwrap_or_default();
        }
    }

    fn schedule_engine_preparations(&mut self) {
        let generation = self.engine_generation;
        let config = self.engine_config();
        let jobs: Vec<_> = self
            .original_audio
            .iter()
            .filter(|(id, _)| {
                !self.preparing_irs.contains(id)
                    && self
                        .prepared_slots
                        .get(id)
                        .is_none_or(|(prepared_generation, _)| *prepared_generation != generation)
            })
            .filter_map(|(id, source)| {
                self.slot_params(*id).and_then(|params| {
                    self.snapshot
                        .project
                        .ir_slots
                        .iter()
                        .find(|slot| slot.id == *id)
                        .map(|slot| (*id, Arc::clone(source), params, slot.equalizer.clone()))
                })
            })
            .collect();
        for (id, source, params, equalizer) in jobs {
            self.preparing_irs.insert(id);
            let _ = self.worker_tx.send(WorkerRequest::PrepareIr {
                id,
                source,
                generation,
                config,
                params,
                equalizer,
            });
        }
        if let Some(source) = &self.preview_audio
            && self
                .prepared_preview
                .as_ref()
                .is_none_or(|(prepared_generation, _)| *prepared_generation != generation)
            && self.preparing_preview != Some(generation)
        {
            self.preparing_preview = Some(generation);
            let _ = self.worker_tx.send(WorkerRequest::PreparePreview {
                source: Arc::clone(source),
                generation,
                sample_rate: SampleRate(config.sample_rate),
            });
        }
    }

    fn engine_preparations_ready(&self) -> bool {
        let generation = self.engine_generation;
        self.original_audio.keys().all(|id| {
            self.prepared_slots
                .get(id)
                .is_some_and(|(prepared_generation, _)| *prepared_generation == generation)
        }) && self.preview_audio.as_ref().is_none_or(|_| {
            self.prepared_preview
                .as_ref()
                .is_some_and(|(prepared_generation, _)| *prepared_generation == generation)
        })
    }

    fn ensure_runtime(&mut self) -> bool {
        if self.runtime.is_some() {
            return true;
        }
        let previous_rate = selected_u32(&self.snapshot.project.source.sample_rate, 0);
        self.refresh_output_sample_rate();
        if !self.reconcile_output_sample_rate() {
            return false;
        }
        if previous_rate != selected_u32(&self.snapshot.project.source.sample_rate, 0) {
            self.invalidate_engine_preparations();
        }
        self.schedule_engine_preparations();
        if !self.engine_preparations_ready() {
            self.snapshot.status = "Preparing audio for the output device".into();
            self.snapshot.status_is_error = false;
            return false;
        }
        match AudioRuntime::start(self.runtime_config()) {
            Ok(runtime) => {
                self.runtime = Some(runtime);
                self.snapshot.status = "Audio Engine Running".into();
                self.snapshot.status_is_error = false;
                self.send(RuntimeCommand::SetSourceLive(
                    self.snapshot.project.source.mode == SourceMode::Live,
                ));
                self.send(RuntimeCommand::SetSourceGain(
                    self.snapshot.project.source.gain_db,
                ));
                self.send(RuntimeCommand::SetPreviewNormalize(
                    self.snapshot.project.source.normalize,
                ));
                self.send(RuntimeCommand::SetLoop(
                    self.snapshot.project.source.looping,
                ));
                self.send(RuntimeCommand::SetMonitoring(
                    self.snapshot.project.source.monitoring,
                ));
                self.send(RuntimeCommand::SetOutputGain(
                    self.snapshot.project.output.gain_db,
                ));
                self.send(RuntimeCommand::SetBypassed(
                    self.snapshot.project.output.bypassed,
                ));
                self.send(RuntimeCommand::SetLimiter(
                    self.snapshot.project.output.limit_output,
                ));
                self.install_global_equalizer();
                if let Some((_, preview)) = &self.prepared_preview {
                    self.send(RuntimeCommand::SetPreview {
                        normalization_gain: preview.normalization_gain(0.0),
                        audio: Box::new((**preview).clone()),
                    });
                }
                let prepared = std::mem::take(&mut self.prepared_slots);
                for (_, (_, slot)) in prepared {
                    self.send(RuntimeCommand::ReplaceSlot(slot));
                }
                if self.snapshot.project.source.transport == TransportState::Playing {
                    self.preview_start_pending = true;
                    self.send(RuntimeCommand::Play);
                }
                true
            }
            Err(error) => {
                self.fail(error.to_string());
                false
            }
        }
    }

    fn restart_runtime(&mut self) {
        self.runtime = None;
        self.invalidate_engine_preparations();
        self.schedule_engine_preparations();
        if self.snapshot.project.source.monitoring
            || self.snapshot.project.source.transport == TransportState::Playing
        {
            self.ensure_runtime();
        }
    }

    fn invalidate_engine_preparations(&mut self) {
        self.engine_generation = self.engine_generation.wrapping_add(1);
        self.prepared_slots.clear();
        self.preparing_irs.clear();
        self.prepared_preview = None;
        self.preparing_preview = None;
    }
    fn send(&mut self, command: RuntimeCommand) {
        if let Some(runtime) = &mut self.runtime
            && let Err(error) = runtime.send(command)
        {
            self.fail(error.to_string());
        }
    }

    fn choose_preview(&mut self) {
        if self.dialog_open {
            return;
        }
        self.snapshot.file_load_activity.preview_loading = true;
        if !self.open_dialog(DialogRequest::Preview) {
            self.snapshot.file_load_activity.preview_loading = false;
        }
    }

    fn choose_irs(&mut self, replacement: Option<IrId>) {
        if self.dialog_open {
            return;
        }
        if let Some(id) = replacement {
            if !self
                .snapshot
                .file_load_activity
                .replacing_ir_ids
                .contains(&id)
            {
                self.snapshot.file_load_activity.replacing_ir_ids.push(id);
            }
        } else {
            // A sentinel keeps the Add button busy while the native picker is open.
            self.snapshot.file_load_activity.adding_ir_count = 1;
        }
        if !self.open_dialog(DialogRequest::Irs { replacement }) {
            if let Some(id) = replacement {
                self.finish_replacement_load(id);
            } else {
                self.snapshot.file_load_activity.adding_ir_count = 0;
            }
        }
    }

    fn queue_ir(&mut self, path: PathBuf, replacement: Option<IrId>) -> IrId {
        let id = replacement.unwrap_or_else(|| {
            let id = IrId(self.next_ir_id);
            self.next_ir_id += 1;
            id
        });
        if replacement.is_none() {
            self.snapshot.file_load_activity.adding_ir_count += 1;
            self.snapshot.project.ir_slots.push(new_slot(id, &path));
            if self.snapshot.project.balance_mode {
                self.snapshot.project.equalize_balance();
                self.sync_balance_slot_parameters();
            }
            self.snapshot.project.selected_ir = Some(id);
        } else if let Some(slot) = self
            .snapshot
            .project
            .ir_slots
            .iter_mut()
            .find(|slot| slot.id == id)
        {
            if !self
                .snapshot
                .file_load_activity
                .replacing_ir_ids
                .contains(&id)
            {
                self.snapshot.file_load_activity.replacing_ir_ids.push(id);
            }
            slot.load_state = ContentState::Loading {
                message: "Preparing IR".into(),
            };
        }
        if let Some(params) = self.slot_params(id) {
            let equalizer = self
                .snapshot
                .project
                .ir_slots
                .iter()
                .find(|slot| slot.id == id)
                .map(|slot| slot.equalizer.clone())
                .unwrap_or_default();
            self.preparing_irs.insert(id);
            let _ = self.worker_tx.send(WorkerRequest::LoadIr {
                id,
                path,
                generation: self.engine_generation,
                config: self.engine_config(),
                params,
                equalizer,
            });
        }
        id
    }

    fn update_slot(&mut self, id: IrId, update: impl FnOnce(&mut IrSlotState)) {
        if let Some(slot) = self
            .snapshot
            .project
            .ir_slots
            .iter_mut()
            .find(|slot| slot.id == id)
        {
            update(slot);
        }
        if let Some(params) = self.slot_params(id) {
            self.send(RuntimeCommand::SetSlotParameters(id.0, params));
        }
    }
    fn equalizer(&self, target: EqualizerTarget) -> Option<&EqualizerState> {
        match target {
            EqualizerTarget::Global => Some(&self.snapshot.project.global_equalizer),
            EqualizerTarget::Ir(id) => self
                .snapshot
                .project
                .ir_slots
                .iter()
                .find(|slot| slot.id == id)
                .map(|slot| &slot.equalizer),
        }
    }

    fn equalizer_mut(&mut self, target: EqualizerTarget) -> Option<&mut EqualizerState> {
        match target {
            EqualizerTarget::Global => Some(&mut self.snapshot.project.global_equalizer),
            EqualizerTarget::Ir(id) => self
                .snapshot
                .project
                .ir_slots
                .iter_mut()
                .find(|slot| slot.id == id)
                .map(|slot| &mut slot.equalizer),
        }
    }

    fn update_equalizer(
        &mut self,
        target: EqualizerTarget,
        update: impl FnOnce(&mut EqualizerState),
    ) {
        let state = self.equalizer_mut(target).map(|equalizer| {
            update(equalizer);
            equalizer.sanitize();
            equalizer.clone()
        });
        if let Some(state) = state {
            match PreparedEqualizer::new(&state, self.engine_config().sample_rate) {
                Ok(equalizer) => match target {
                    EqualizerTarget::Global => self.send(
                        RuntimeCommand::ReplaceGlobalEqualizer(Box::new(equalizer)),
                    ),
                    EqualizerTarget::Ir(id) => self.send(RuntimeCommand::ReplaceSlotEqualizer(
                        id.0,
                        Box::new(equalizer),
                    )),
                },
                Err(error) => self.fail(error.to_string()),
            }
        }
    }
    fn update_eq_band(
        &mut self,
        target: EqualizerTarget,
        band: EqBandId,
        update: impl FnOnce(&mut EqBand),
    ) {
        self.update_equalizer(target, |equalizer| {
            if let Some(item) = equalizer.bands.iter_mut().find(|item| item.id == band) {
                update(item);
                item.sanitize();
            }
        });
    }

    fn install_global_equalizer(&mut self) {
        let state = self.snapshot.project.global_equalizer.clone();
        match PreparedEqualizer::new(&state, self.engine_config().sample_rate) {
            Ok(equalizer) => {
                self.send(RuntimeCommand::ReplaceGlobalEqualizer(Box::new(equalizer)))
            }
            Err(error) => self.fail(error.to_string()),
        }
    }
    fn slot_params(&self, id: IrId) -> Option<SlotParameters> {
        self.snapshot
            .project
            .ir_slots
            .iter()
            .find(|slot| slot.id == id)
            .map(params)
    }
    fn clear_irs(&mut self) {
        let ids: Vec<_> = self
            .snapshot
            .project
            .ir_slots
            .iter()
            .map(|slot| slot.id)
            .collect();
        for id in ids {
            self.send(RuntimeCommand::RemoveSlot(id.0));
        }
        self.snapshot.project.ir_slots.clear();
        self.snapshot.project.selected_ir = None;
        self.original_audio.clear();
        self.prepared_slots.clear();
        self.preparing_irs.clear();
    }
    fn remove_ir(&mut self, id: IrId) {
        self.send(RuntimeCommand::RemoveSlot(id.0));
        self.original_audio.remove(&id);
        self.prepared_slots.remove(&id);
        self.preparing_irs.remove(&id);
        self.snapshot.project.ir_slots.retain(|slot| slot.id != id);
        if self.snapshot.project.balance_mode {
            self.snapshot.project.equalize_balance();
            self.sync_balance_slot_parameters();
        }
        if self.snapshot.project.selected_ir == Some(id) {
            self.snapshot.project.selected_ir =
                self.snapshot.project.ir_slots.first().map(|slot| slot.id);
        }
    }
    fn move_ir(&mut self, id: IrId, offset: isize) {
        let slots = &mut self.snapshot.project.ir_slots;
        if let Some(index) = slots.iter().position(|slot| slot.id == id) {
            let target = index
                .saturating_add_signed(offset)
                .min(slots.len().saturating_sub(1));
            if target != index {
                slots.swap(index, target);
            }
        }
    }

    fn sync_balance_slot_parameters(&mut self) {
        let slots = self
            .snapshot
            .project
            .ir_slots
            .iter()
            .filter(|slot| self.original_audio.contains_key(&slot.id))
            .map(|slot| (slot.id, params(slot)))
            .collect::<Vec<_>>();
        for (id, slot_params) in slots {
            self.send(RuntimeCommand::SetSlotParameters(id.0, slot_params));
        }
    }

    fn choose_export_destination(&mut self) -> bool {
        self.open_dialog(DialogRequest::Export {
            filename: self.snapshot.project.export.filename.clone(),
        })
    }
    fn export(&mut self) {
        let Some(path) = self.snapshot.project.export.destination.clone() else {
            self.export_after_destination = true;
            if !self.choose_export_destination() {
                self.export_after_destination = false;
                self.set_export_error("Could not open the export destination dialog".into());
            }
            return;
        };
        let sources = self.export_sources();
        let export = &self.snapshot.project.export;
        let settings = RenderSettings {
            sample_rate: SampleRate(selected_u32(&export.sample_rate, 48_000)),
            frames: export
                .trim_to_length
                .then(|| selected_usize(&export.length, 2048)),
            channels: if export.channel_mode.0 == "stereo" {
                ExportChannels::Stereo
            } else {
                ExportChannels::Mono
            },
            output_gain_db: self.snapshot.project.output.gain_db,
            normalize: export.normalize,
            normalization_target_dbfs: -1.0,
        };
        let encoding = match export.bit_depth.0.as_str() {
            "16" => WavEncoding::Pcm16,
            "32f" => WavEncoding::Float32,
            _ => WavEncoding::Pcm24,
        };
        self.snapshot.project.export.state = ExportState::Exporting { progress: 0.05 };
        self.snapshot.status = "Exporting mixed IR".into();
        self.snapshot.status_is_error = false;
        if self
            .worker_tx
            .send(WorkerRequest::Export {
                path,
                sources,
                global_equalizer: self.snapshot.project.global_equalizer.clone(),
                settings,
                encoding,
            })
            .is_err()
        {
            self.set_export_error("The export worker is unavailable".into());
        }
    }

    fn save_preset(&mut self, save_as: bool) {
        if !save_as && let Some(path) = self.active_preset_path.clone() {
            self.save_preset_to(path);
            return;
        }
        self.open_dialog(DialogRequest::SavePreset {
            directory: preset_directory(),
            filename: format!("{}.json", safe_name(&self.snapshot.project.name)),
        });
    }

    fn open_dialog(&mut self, request: DialogRequest) -> bool {
        if !self.dialog_open && self.dialog_tx.try_send(request).is_ok() {
            self.dialog_open = true;
            self.snapshot.file_load_activity.dialog_open = true;
            true
        } else {
            false
        }
    }

    fn save_preset_to(&mut self, path: PathBuf) {
        let mut document = PresetDocument::new(self.snapshot.project.clone());
        if let Some(parent) = path.parent() {
            for slot in &mut document.project.ir_slots {
                if let Some(reference) = &mut slot.file_reference {
                    reference.relative_path = reference
                        .original_path
                        .strip_prefix(parent)
                        .ok()
                        .map(Path::to_path_buf);
                }
            }
        }
        let _ = self.worker_tx.send(WorkerRequest::SavePreset {
            path,
            document: Box::new(document),
        });
    }

    fn handle_dialog_result(&mut self, result: DialogResult) {
        self.dialog_open = false;
        self.snapshot.file_load_activity.dialog_open = false;
        match result {
            DialogResult::Preview(Some(path)) => self.load_preview_path(path),
            DialogResult::Irs { replacement, paths } => {
                if replacement.is_none() {
                    self.snapshot.file_load_activity.adding_ir_count = 0;
                }
                if paths.is_empty()
                    && let Some(id) = replacement
                {
                    self.finish_replacement_load(id);
                }
                for path in paths {
                    self.queue_ir(path, replacement);
                }
            }
            DialogResult::Export(Some(path)) => {
                let path = ensure_wav_extension(path);
                self.snapshot.project.export.filename = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Mixed_IR.wav")
                    .into();
                self.snapshot.project.export.destination = Some(path);
                if self.export_after_destination {
                    self.export_after_destination = false;
                    self.export();
                }
            }
            DialogResult::SavePreset(Some(path)) => self.save_preset_to(path),
            DialogResult::Preview(None) => {
                self.snapshot.file_load_activity.preview_loading = false;
            }
            DialogResult::Export(None) => {
                self.export_after_destination = false;
            }
            DialogResult::SavePreset(None) => {}
        }
    }
    fn load_preset(&mut self, id: OptionId) {
        if let Some(path) = self.preset_paths.get(&id).cloned() {
            let _ = self.worker_tx.send(WorkerRequest::LoadPreset { path });
        }
    }
    fn delete_preset(&mut self) {
        if let Some(path) = self.active_preset_path.clone() {
            let _ = self.worker_tx.send(WorkerRequest::DeletePreset { path });
        }
    }

    fn handle_worker_result(&mut self, result: WorkerResult) {
        match result {
            WorkerResult::IrReady {
                id,
                path,
                source,
                generation,
                prepared_sample_rate,
                prepared,
                file_reference,
            } => {
                let was_loading = self
                    .snapshot
                    .project
                    .ir_slots
                    .iter()
                    .find(|slot| slot.id == id)
                    .is_some_and(|slot| matches!(slot.load_state, ContentState::Loading { .. }));
                self.original_audio.insert(id, Arc::clone(&source));
                if let Some(slot) = self
                    .snapshot
                    .project
                    .ir_slots
                    .iter_mut()
                    .find(|slot| slot.id == id)
                {
                    if !path.as_os_str().is_empty() {
                        slot.file_path = Some(path.clone());
                        slot.filename = path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("IR.wav")
                            .into();
                    }
                    if file_reference.is_some() {
                        slot.file_reference = file_reference;
                    }
                    slot.metadata = ir_metadata(&source);
                    slot.waveform = source.waveform_envelope(180);
                    if generation == self.engine_generation {
                        // Delay is configured in engine samples, so its millisecond
                        // readout must use the prepared rate rather than file metadata.
                        slot.sample_rate_hz = prepared_sample_rate as f32;
                        slot.load_state = ContentState::Ready;
                    }
                }
                if generation == self.engine_generation {
                    self.preparing_irs.remove(&id);
                    if self.runtime.is_some() {
                        self.send(RuntimeCommand::ReplaceSlot(prepared));
                    } else {
                        self.prepared_slots.insert(id, (generation, prepared));
                    }
                    if was_loading {
                        self.finish_ir_load(id);
                    }
                    self.snapshot.status = "Impulse response ready".into();
                    self.snapshot.status_is_error = false;
                    self.analysis_dirty = true;
                } else {
                    self.schedule_engine_preparations();
                }
                if self.snapshot.project.source.monitoring
                    || self.snapshot.project.source.transport == TransportState::Playing
                {
                    self.ensure_runtime();
                }
            }
            WorkerResult::IrFailed {
                id,
                generation,
                message,
            } => {
                if generation != self.engine_generation && self.original_audio.contains_key(&id) {
                    self.schedule_engine_preparations();
                    return;
                }
                self.preparing_irs.remove(&id);
                let was_loading = self
                    .snapshot
                    .project
                    .ir_slots
                    .iter()
                    .find(|slot| slot.id == id)
                    .is_some_and(|slot| matches!(slot.load_state, ContentState::Loading { .. }));
                if let Some(slot) = self
                    .snapshot
                    .project
                    .ir_slots
                    .iter_mut()
                    .find(|slot| slot.id == id)
                {
                    slot.load_state = ContentState::Error {
                        message: message.clone(),
                    };
                }
                if was_loading {
                    self.finish_ir_load(id);
                }
                self.fail(message);
            }
            WorkerResult::PreviewReady {
                path,
                source,
                prepared,
                generation,
            } => {
                if !path.as_os_str().is_empty() {
                    self.snapshot.project.source.filename = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(str::to_owned);
                    self.snapshot.project.source.file_path = Some(path);
                }
                self.snapshot.project.source.duration_seconds =
                    source.frame_count() as f64 / source.sample_rate().0 as f64;
                self.snapshot.project.source.metadata = format!(
                    "{:.1} kHz | {:.3} s",
                    source.sample_rate().0 as f32 / 1000.0,
                    self.snapshot.project.source.duration_seconds
                );
                self.snapshot.project.source.waveform = source.waveform_envelope(320);
                self.preview_audio = Some(source);
                if generation == self.engine_generation {
                    self.snapshot.project.source.elapsed_seconds = 0.0;
                    self.snapshot.project.source.content_state = ContentState::Ready;
                    self.snapshot.file_load_activity.preview_loading = false;
                    self.preparing_preview = None;
                    let prepared = Arc::new(*prepared);
                    self.prepared_preview = Some((generation, Arc::clone(&prepared)));
                    if self.runtime.is_some() {
                        self.send(RuntimeCommand::SetPreview {
                            normalization_gain: prepared.normalization_gain(0.0),
                            audio: Box::new((*prepared).clone()),
                        });
                    }
                } else {
                    self.schedule_engine_preparations();
                }
                if self.snapshot.project.source.monitoring
                    || self.snapshot.project.source.transport == TransportState::Playing
                {
                    self.ensure_runtime();
                }
            }
            WorkerResult::PreviewFailed {
                generation,
                message,
            } => {
                if generation != self.engine_generation && self.preview_audio.is_some() {
                    self.schedule_engine_preparations();
                    return;
                }
                self.preparing_preview = None;
                self.snapshot.file_load_activity.preview_loading = false;
                self.snapshot.project.source.content_state = ContentState::Error {
                    message: message.clone(),
                };
                self.fail(message);
            }
            WorkerResult::ExportComplete(path) => {
                let message = format!("Exported {}", path.display());
                self.snapshot.project.export.state = ExportState::Complete {
                    message: message.clone(),
                };
                self.snapshot.status = message;
                self.snapshot.status_is_error = false;
                self.events
                    .push(BackendEvent::ExportCompleted(path.display().to_string()));
            }
            WorkerResult::ExportFailed(message) => {
                self.events
                    .push(BackendEvent::ExportFailed(message.clone()));
                self.set_export_error(message);
            }
            WorkerResult::AnalysisReady {
                generation,
                frequency,
                phase,
                waveform,
            } => {
                self.analysis_in_flight = false;
                if generation == self.analysis_generation {
                    self.snapshot.frequency_traces = frequency;
                    self.snapshot.phase_traces = phase;
                    self.snapshot.combined_waveform = waveform;
                    self.snapshot.project.analysis.content_state = ContentState::Ready;
                }
            }
            WorkerResult::PresetSaved(path) => {
                self.active_preset_path = Some(path);
                self.snapshot.project.dirty = false;
                self.refresh_presets();
                self.events.push(BackendEvent::PresetSaved);
                self.snapshot.status = "Preset saved".into();
                self.snapshot.status_is_error = false;
            }
            WorkerResult::PresetLoaded(path, document) => {
                let enabled = document
                    .project
                    .ir_slots
                    .iter()
                    .filter(|slot| slot.enabled)
                    .count();
                if enabled > self.max_active_irs {
                    self.fail(format!(
                        "Preset enables {enabled} IRs, but the real-time limit is {}",
                        self.max_active_irs
                    ));
                    return;
                }
                let source_device = self.snapshot.project.source.device.clone();
                let source_channel = self.snapshot.project.source.channel.clone();
                let sample_rate = self.snapshot.project.source.sample_rate.clone();
                let source_buffer = self.snapshot.project.source.buffer_size.clone();
                let monitoring = self.snapshot.project.source.monitoring;
                let output_device = self.snapshot.project.output.device.clone();
                let output_channel = self.snapshot.project.output.channel.clone();
                let output_buffer = self.snapshot.project.output.buffer_size.clone();
                self.clear_irs();
                self.snapshot.project = document.project;
                self.snapshot.project.source.device = source_device;
                self.snapshot.project.source.channel = source_channel;
                self.snapshot.project.source.sample_rate = sample_rate;
                self.snapshot.project.source.buffer_size = source_buffer;
                self.snapshot.project.source.monitoring = monitoring;
                self.snapshot.project.source.transport = TransportState::Stopped;
                self.snapshot.project.output.device = output_device;
                self.snapshot.project.output.channel = output_channel;
                self.snapshot.project.output.buffer_size = output_buffer;
                self.snapshot.project.export.destination = None;
                self.snapshot.project.export.state = ExportState::Idle;
                self.snapshot.project.dirty = false;
                self.install_global_equalizer();
                self.next_ir_id = self
                    .snapshot
                    .project
                    .ir_slots
                    .iter()
                    .map(|slot| slot.id.0)
                    .max()
                    .unwrap_or(0)
                    + 1;
                self.active_preset_path = Some(path);
                let slots: Vec<_> = self
                    .snapshot
                    .project
                    .ir_slots
                    .iter()
                    .filter_map(|slot| {
                        slot.file_path
                            .clone()
                            .map(|path| (slot.id, path, params(slot), slot.equalizer.clone()))
                    })
                    .collect();
                for (id, path, params, equalizer) in slots {
                    self.preparing_irs.insert(id);
                    let _ = self.worker_tx.send(WorkerRequest::LoadIr {
                        id,
                        path,
                        generation: self.engine_generation,
                        config: self.engine_config(),
                        params,
                        equalizer,
                    });
                }
                self.snapshot.status = "Preset loaded".into();
                self.snapshot.status_is_error = false;
            }
            WorkerResult::PresetDeleted => {
                self.active_preset_path = None;
                self.refresh_presets();
                self.snapshot.status = "Preset deleted".into();
                self.snapshot.status_is_error = false;
            }
            WorkerResult::PresetFailed(message) => self.fail(message),
        }
    }
    fn refresh_presets(&mut self) {
        let (choices, paths) = discover_presets();
        self.snapshot.presets = choices;
        self.preset_paths = paths;
        if let Some(active) = &self.active_preset_path
            && let Some((id, _)) = self.preset_paths.iter().find(|(_, path)| *path == active)
        {
            self.snapshot.active_preset = id.clone();
        }
    }
    fn fail(&mut self, message: String) {
        self.snapshot.status = message.clone();
        self.snapshot.status_is_error = true;
        self.events.push(BackendEvent::StatusChanged(message));
    }

    fn set_export_error(&mut self, message: String) {
        self.snapshot.project.export.state = ExportState::Error {
            message: message.clone(),
        };
        self.fail(message);
    }

    fn export_sources(&self) -> Vec<ExportSource> {
        self.snapshot
            .project
            .ir_slots
            .iter()
            .filter_map(|slot| {
                self.original_audio.get(&slot.id).map(|audio| ExportSource {
                    label: slot.filename.clone(),
                    color_index: Some(slot.color_index),
                    audio: Arc::clone(audio),
                    params: params(slot),
                    equalizer: slot.equalizer.clone(),
                })
            })
            .collect()
    }

    fn request_analysis(&mut self) {
        let sources = self.export_sources();
        if sources.is_empty() {
            self.snapshot.frequency_traces.clear();
            self.snapshot.phase_traces.clear();
            self.snapshot.combined_waveform.clear();
            self.snapshot.project.analysis.content_state = ContentState::Ready;
            self.analysis_in_flight = false;
            return;
        }
        self.analysis_generation += 1;
        let generation = self.analysis_generation;
        self.analysis_in_flight = true;
        let settings = RenderSettings {
            sample_rate: SampleRate(self.engine_config().sample_rate),
            frames: None,
            channels: ExportChannels::Stereo,
            output_gain_db: self.snapshot.project.output.gain_db,
            normalize: false,
            normalization_target_dbfs: -1.0,
        };
        let _ = self.worker_tx.send(WorkerRequest::Analyze {
            generation,
            sources,
            global_equalizer: self.snapshot.project.global_equalizer.clone(),
            settings,
        });
    }

    fn finish_replacement_load(&mut self, id: IrId) {
        self.snapshot
            .file_load_activity
            .replacing_ir_ids
            .retain(|candidate| *candidate != id);
    }

    fn finish_ir_load(&mut self, id: IrId) {
        if self.snapshot.file_load_activity.replacing_ir(id) {
            self.finish_replacement_load(id);
        } else {
            self.snapshot.file_load_activity.adding_ir_count = self
                .snapshot
                .file_load_activity
                .adding_ir_count
                .saturating_sub(1);
        }
    }
}

fn params(slot: &IrSlotState) -> SlotParameters {
    SlotParameters {
        enabled: slot.enabled,
        muted: slot.muted,
        soloed: slot.soloed,
        gain_db: slot.gain_db,
        delay_samples: slot.delay_samples.max(0) as usize,
        pan: slot.pan,
        polarity_inverted: slot.polarity_inverted,
        normalize: slot.normalize,
    }
}

fn command_changes_analysis(command: &AppCommand) -> bool {
    use AppCommand::*;
    matches!(
        command,
        ClearAllIrs
            | NormalizeAllIrs
            | SetBalanceMode(_)
            | RemoveIr(_)
            | MoveIrUp(_)
            | MoveIrDown(_)
            | SetIrEnabled(_, _)
            | SetIrGainDb(_, _)
            | SetIrBalancePercent(_, _)
            | SetIrDelaySamples(_, _)
            | SetIrPan(_, _)
            | SetIrPolarity(_, _)
            | SetIrNormalize(_, _)
            | SetIrSolo(_, _)
            | SetIrMute(_, _)
            | AddEqBand(_, _, _)
            | RemoveEqBand(_, _)
            | SetEqBandEnabled(_, _, _)
            | SetEqBandShape(_, _, _)
            | SetEqBandFrequency(_, _, _)
            | SetEqBandGain(_, _, _)
            | SetEqBandQ(_, _, _)
            | SetEqBypassed(_, _)
            | SetEqOutputGainDb(_, _)
            | ResetEq(_)
            | PasteEq(_)
            | SetOutputGainDb(_)
    )
}

fn preview_has_ended(
    transport: TransportState,
    source_mode: SourceMode,
    runtime_playing: bool,
    start_pending: bool,
) -> bool {
    transport == TransportState::Playing
        && source_mode == SourceMode::Preview
        && !runtime_playing
        && !start_pending
}

fn selected_u32(id: &OptionId, fallback: u32) -> u32 {
    id.0.parse().unwrap_or(fallback)
}

fn ensure_wav_extension(mut path: PathBuf) -> PathBuf {
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("wav"))
    {
        path.set_extension("wav");
    }
    path
}
fn selected_usize(id: &OptionId, fallback: usize) -> usize {
    id.0.parse().unwrap_or(fallback)
}
fn selected_index(id: &OptionId) -> usize {
    id.0.rsplit('-')
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

fn input_channel_choices(catalog: &DeviceCatalog, device_id: &OptionId) -> Vec<Choice> {
    catalog
        .inputs
        .iter()
        .find(|device| device.id == device_id.0)
        .map(|device| {
            (0..device.channels)
                .map(|index| Choice::new(format!("input-{index}"), format!("Input {}", index + 1)))
                .collect()
        })
        .unwrap_or_default()
}

fn output_default_sample_rate(catalog: &DeviceCatalog, device_id: &OptionId) -> Option<u32> {
    catalog
        .outputs
        .iter()
        .find(|device| device.id == device_id.0)
        .map(|device| device.default_sample_rate)
}

fn output_channel_choices(catalog: &DeviceCatalog, device_id: &OptionId) -> Vec<Choice> {
    let Some(device) = catalog
        .outputs
        .iter()
        .find(|device| device.id == device_id.0)
    else {
        return Vec::new();
    };
    let mut choices = Vec::new();
    let mut index = 0;
    while index + 1 < device.channels {
        choices.push(Choice::new(
            format!("output-{index}"),
            format!("Output {} / {}", index + 1, index + 2),
        ));
        index += 2;
    }
    if index < device.channels {
        choices.push(Choice::new(
            format!("output-{index}"),
            format!("Output {} (Mono)", index + 1),
        ));
    }
    choices
}

fn new_slot(id: IrId, path: &Path) -> IrSlotState {
    IrSlotState {
        id,
        filename: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("IR.wav")
            .into(),
        file_path: Some(path.to_path_buf()),
        file_reference: Some(IrFileReference {
            original_path: path.to_path_buf(),
            relative_path: None,
            size_bytes: None,
            content_fingerprint: None,
        }),
        metadata: String::new(),
        color_index: ((id.0 - 1) % 8) as u8,
        enabled: true,
        gain_db: 0.0,
        balance_percent: 0.0,
        delay_samples: 0,
        sample_rate_hz: 48_000.0,
        pan: 0.0,
        polarity_inverted: false,
        normalize: false,
        soloed: false,
        muted: false,
        equalizer: ir_eq::EqualizerState::default(),
        load_state: ContentState::Loading {
            message: "Preparing IR".into(),
        },
        waveform: Vec::new(),
    }
}

fn ir_metadata(audio: &AudioBuffer) -> String {
    format!(
        "{:.1} kHz | {} samples",
        audio.sample_rate().0 as f32 / 1000.0,
        audio.frame_count()
    )
}

fn preset_directory() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("IR Mixer Pro")
        .join("Presets")
}
fn safe_name(name: &str) -> String {
    let value: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    if value.is_empty() {
        "Untitled_Mix".into()
    } else {
        value
    }
}
fn discover_presets() -> (Vec<Choice>, HashMap<OptionId, PathBuf>) {
    let mut choices = Vec::new();
    let mut paths = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(preset_directory()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            let label = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("Preset")
                .replace('_', " ");
            let id = OptionId(path.display().to_string());
            choices.push(Choice {
                id: id.clone(),
                label,
            });
            paths.insert(id, path);
        }
    }
    choices.sort_by(|left, right| left.label.cmp(&right.label));
    (choices, paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn fixture(relative: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative)
    }

    fn wait_until(backend: &mut NativeAudioBackend, condition: impl Fn(&AppSnapshot) -> bool) {
        let started = Instant::now();
        while !condition(backend.snapshot()) {
            backend.update(started.elapsed().as_secs_f64());
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "native worker operation timed out: {}",
                backend.snapshot.status
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn loads_real_ir_and_computes_analysis() {
        let mut backend = NativeAudioBackend::default();
        backend.load_ir_path(fixture("samples/irs/01_Twin73_dome_edge_L19.wav"));
        assert!(backend.snapshot.file_load_activity.adding_irs());
        wait_until(&mut backend, |snapshot| {
            snapshot
                .project
                .ir_slots
                .first()
                .is_some_and(|slot| matches!(slot.load_state, ContentState::Ready))
        });
        wait_until(&mut backend, |snapshot| {
            !snapshot.frequency_traces.is_empty()
        });
        let slot = &backend.snapshot.project.ir_slots[0];
        assert!(
            slot.file_reference
                .as_ref()
                .and_then(|reference| reference.content_fingerprint.as_ref())
                .is_some()
        );
        assert!(!slot.waveform.is_empty());
        assert!(!backend.snapshot.file_load_activity.adding_irs());
    }

    #[test]
    fn decodes_preview_and_exports_mixed_ir() {
        let mut backend = NativeAudioBackend::default();
        backend.load_preview_path(fixture("samples/input/90_Em_GuitarLoop_SP_140_03.wav"));
        assert!(backend.snapshot.file_load_activity.preview_loading);
        wait_until(&mut backend, |snapshot| {
            snapshot.project.source.filename.is_some()
                && matches!(snapshot.project.source.content_state, ContentState::Ready)
        });
        assert!(!backend.snapshot.file_load_activity.preview_loading);
        backend.load_ir_path(fixture("samples/irs/02_Twin73_dome_edge_e609.wav"));
        wait_until(&mut backend, |snapshot| {
            snapshot
                .project
                .ir_slots
                .first()
                .is_some_and(|slot| matches!(slot.load_state, ContentState::Ready))
        });
        let output =
            std::env::temp_dir().join(format!("ir-mixer-export-{}.wav", std::process::id()));
        backend.export_to_path(output.clone());
        wait_until(&mut backend, |snapshot| {
            matches!(
                snapshot.project.export.state,
                ExportState::Complete { .. } | ExportState::Error { .. }
            )
        });
        assert!(
            matches!(
                backend.snapshot.project.export.state,
                ExportState::Complete { .. }
            ),
            "{}",
            backend.snapshot.status
        );
        let exported = ir_core::read_wav(&output).expect("exported WAV should be readable");
        assert_eq!(exported.frame_count(), 2048);
        let _ = std::fs::remove_file(output);
    }

    #[test]
    fn choosing_a_destination_from_export_continues_the_export_and_adds_wav_extension() {
        let mut backend = NativeAudioBackend::default();
        backend.load_ir_path(fixture("samples/irs/02_Twin73_dome_edge_e609.wav"));
        wait_until(&mut backend, |snapshot| {
            snapshot
                .project
                .ir_slots
                .first()
                .is_some_and(|slot| matches!(slot.load_state, ContentState::Ready))
        });
        backend.snapshot.project.export.destination = None;
        backend.export_after_destination = true;
        let path_without_extension =
            std::env::temp_dir().join(format!("ir-mixer-dialog-export-{}", std::process::id()));

        backend.handle_dialog_result(DialogResult::Export(Some(path_without_extension.clone())));
        wait_until(&mut backend, |snapshot| {
            matches!(
                snapshot.project.export.state,
                ExportState::Complete { .. } | ExportState::Error { .. }
            )
        });

        let wav_path = path_without_extension.with_extension("wav");
        assert!(matches!(
            backend.snapshot.project.export.state,
            ExportState::Complete { .. }
        ));
        assert_eq!(
            backend.snapshot.project.export.destination.as_deref(),
            Some(wav_path.as_path())
        );
        assert!(wav_path.exists());
        let _ = std::fs::remove_file(wav_path);
    }

    #[test]
    fn rapid_ir_tweaks_keep_analysis_visible_and_bound_work() {
        let mut backend = NativeAudioBackend::default();
        let id = backend.load_ir_path(fixture("samples/irs/01_Twin73_dome_edge_L19.wav"));
        wait_until(&mut backend, |snapshot| {
            !snapshot.combined_waveform.is_empty()
        });

        let initial_waveform = backend.snapshot.combined_waveform.clone();
        let initial_generation = backend.analysis_generation;
        backend.dispatch(AppCommand::SetIrGainDb(id, -18.0));
        backend.update(0.0);
        assert!(backend.analysis_in_flight);
        assert_eq!(
            backend.snapshot.project.analysis.content_state,
            ContentState::Ready
        );
        assert_eq!(backend.snapshot.combined_waveform, initial_waveform);

        for gain_db in [-15.0, -12.0, -9.0, -6.0] {
            backend.dispatch(AppCommand::SetIrGainDb(id, gain_db));
        }
        backend.dispatch(AppCommand::SetIrDelaySamples(id, 12));
        backend.dispatch(AppCommand::SetIrPolarity(id, true));

        let started = Instant::now();
        while backend.analysis_in_flight || backend.analysis_dirty {
            backend.update(started.elapsed().as_secs_f64());
            assert!(started.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(backend.analysis_generation, initial_generation + 2);
        assert_ne!(backend.snapshot.combined_waveform, initial_waveform);
        let slot = &backend.snapshot.project.ir_slots[0];
        assert_eq!(slot.gain_db, -6.0);
        assert_eq!(slot.delay_samples, 12);
        assert!(slot.polarity_inverted);
    }

    #[test]
    fn only_mix_affecting_commands_invalidate_analysis() {
        assert!(command_changes_analysis(&AppCommand::SetIrGainDb(
            IrId(1),
            -3.0
        )));
        assert!(command_changes_analysis(&AppCommand::SetIrPan(
            IrId(1),
            0.5
        )));
        assert!(command_changes_analysis(&AppCommand::SetOutputGainDb(-2.0)));
        assert!(!command_changes_analysis(&AppCommand::PlayPreview));
        assert!(!command_changes_analysis(&AppCommand::SetAnalysisTab(
            ir_app::AnalysisTab::Phase
        )));
        assert!(!command_changes_analysis(&AppCommand::SetExportBitDepth(
            OptionId("24".into())
        )));
    }

    #[test]
    fn ir_metadata_reports_the_native_rate_and_sample_count() {
        let audio = AudioBuffer::mono(SampleRate(44_100), vec![0.0; 22_050]);

        assert_eq!(ir_metadata(&audio), "44.1 kHz | 22050 samples");
    }

    #[test]
    fn export_sources_keep_each_ir_at_its_native_rate() {
        let mut backend = NativeAudioBackend::default();
        let low_rate_id = IrId(1);
        let high_rate_id = IrId(2);
        backend
            .snapshot
            .project
            .ir_slots
            .push(new_slot(low_rate_id, Path::new("low-rate.wav")));
        backend
            .snapshot
            .project
            .ir_slots
            .push(new_slot(high_rate_id, Path::new("high-rate.wav")));
        backend.original_audio.insert(
            low_rate_id,
            Arc::new(AudioBuffer::mono(SampleRate(44_100), vec![0.0; 441])),
        );
        backend.original_audio.insert(
            high_rate_id,
            Arc::new(AudioBuffer::mono(SampleRate(96_000), vec![0.0; 960])),
        );

        let sources = backend.export_sources();

        assert_eq!(sources[0].audio.sample_rate(), SampleRate(44_100));
        assert_eq!(sources[1].audio.sample_rate(), SampleRate(96_000));
    }

    #[test]
    fn channel_choices_follow_selected_device_topology() {
        use crate::runtime::DeviceInfo;

        let catalog = DeviceCatalog {
            inputs: vec![DeviceInfo {
                id: "input-0".into(),
                name: "Four input interface".into(),
                channels: 4,
                default_sample_rate: 48_000,
            }],
            outputs: vec![
                DeviceInfo {
                    id: "output-0".into(),
                    name: "Mono output".into(),
                    channels: 1,
                    default_sample_rate: 44_100,
                },
                DeviceInfo {
                    id: "output-1".into(),
                    name: "Three output interface".into(),
                    channels: 3,
                    default_sample_rate: 96_000,
                },
            ],
            default_input_id: Some("input-0".into()),
            default_output_id: Some("output-1".into()),
        };

        let inputs = input_channel_choices(&catalog, &OptionId("input-0".into()));
        assert_eq!(inputs.len(), 4);
        assert_eq!(inputs[3].label, "Input 4");

        let mono = output_channel_choices(&catalog, &OptionId("output-0".into()));
        assert_eq!(mono.len(), 1);
        assert_eq!(mono[0].label, "Output 1 (Mono)");

        let three = output_channel_choices(&catalog, &OptionId("output-1".into()));
        assert_eq!(three.len(), 2);
        assert_eq!(three[0].label, "Output 1 / 2");
        assert_eq!(three[1].label, "Output 3 (Mono)");
    }

    #[test]
    fn selected_output_default_rate_drives_the_engine_and_delay_units() {
        let mut backend = NativeAudioBackend {
            device_catalog: DeviceCatalog {
                inputs: Vec::new(),
                outputs: vec![crate::runtime::DeviceInfo {
                    id: "test-output".into(),
                    name: "96 kHz interface".into(),
                    channels: 2,
                    default_sample_rate: 96_000,
                }],
                default_input_id: None,
                default_output_id: Some("test-output".into()),
            },
            ..Default::default()
        };
        backend.snapshot.project.output.device = OptionId("test-output".into());
        backend
            .snapshot
            .project
            .ir_slots
            .push(new_slot(IrId(1), Path::new("native-44k.wav")));

        assert!(backend.reconcile_output_sample_rate());

        assert_eq!(backend.snapshot.project.source.sample_rate.0, "96000");
        assert_eq!(backend.engine_config().sample_rate, 96_000);
        assert_eq!(
            backend.snapshot.project.ir_slots[0].sample_rate_hz,
            96_000.0
        );
    }

    #[test]
    fn input_stream_is_requested_only_for_live_monitoring() {
        let mut backend = NativeAudioBackend::default();
        backend.snapshot.project.source.monitoring = true;
        backend.snapshot.project.source.mode = SourceMode::Preview;
        assert!(!backend.runtime_config().live_input_enabled);

        backend.snapshot.project.source.mode = SourceMode::Live;
        assert!(backend.runtime_config().live_input_enabled);

        backend.snapshot.project.source.monitoring = false;
        assert!(!backend.runtime_config().live_input_enabled);
    }

    #[test]
    fn pending_play_command_is_not_mistaken_for_end_of_preview() {
        assert!(!preview_has_ended(
            TransportState::Playing,
            SourceMode::Preview,
            false,
            true,
        ));
        assert!(preview_has_ended(
            TransportState::Playing,
            SourceMode::Preview,
            false,
            false,
        ));
        assert!(!preview_has_ended(
            TransportState::Playing,
            SourceMode::Preview,
            true,
            false,
        ));
    }
}
