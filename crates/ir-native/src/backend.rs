use crate::dialog::{self, DialogRequest, DialogResult};
use crate::runtime::{AudioRuntime, DeviceCatalog, RuntimeCommand, RuntimeConfig};
use crate::worker::{self, ExportSource, WorkerRequest, WorkerResult};
use crossbeam_channel::{Receiver, Sender};
use ir_app::{
    AnalysisTrace, AppCommand, AppSnapshot, AudioBackend, BackendEvent, Choice, ContentState,
    ExportState, IrFileReference, IrId, IrSlotState, MockAudioBackend, OptionId, PresetDocument,
    SourceMode, TransportState,
};
use ir_core::{AudioBuffer, ExportChannels, RenderSettings, SampleRate, WavEncoding};
use ir_dsp::{EngineConfig, SlotParameters};
use std::collections::HashMap;
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
    audio: HashMap<IrId, Arc<AudioBuffer>>,
    preview_audio: Option<Arc<AudioBuffer>>,
    next_ir_id: u64,
    preset_paths: HashMap<OptionId, PathBuf>,
    active_preset_path: Option<PathBuf>,
    max_active_irs: usize,
    analysis_generation: u64,
    analysis_dirty: bool,
    analysis_in_flight: bool,
    preview_start_pending: bool,
    export_after_destination: bool,
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
        snapshot.output_levels_db = [f32::NEG_INFINITY; 2];
        snapshot.output_peaks_db = [f32::NEG_INFINITY; 2];
        snapshot.frequency_traces.clear();
        snapshot.phase_traces.clear();
        snapshot.spectrum_traces.clear();
        snapshot.combined_waveform.clear();
        snapshot.cpu_percent = 0.0;
        snapshot.latency_ms = 0.0;
        snapshot.status = if catalog.inputs.is_empty() || catalog.outputs.is_empty() {
            "No compatible audio devices found".into()
        } else {
            "Audio Engine Stopped".into()
        };
        snapshot.status_is_error = catalog.inputs.is_empty() || catalog.outputs.is_empty();
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
            audio: HashMap::new(),
            preview_audio: None,
            next_ir_id: 1,
            preset_paths,
            active_preset_path: None,
            max_active_irs: 16,
            analysis_generation: 0,
            analysis_dirty: false,
            analysis_in_flight: false,
            preview_start_pending: false,
            export_after_destination: false,
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
        let _ = self.worker_tx.send(WorkerRequest::LoadPreview {
            path,
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
        );
        match command {
            BrowsePreview => self.choose_preview(),
            AddIr => self.choose_irs(None),
            ReplaceIr(id) => self.choose_irs(Some(id)),
            ClearAllIrs => self.clear_irs(),
            RemoveIr(id) => self.remove_ir(id),
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
            SetIrDelaySamples(id, value) => {
                self.update_slot(id, |slot| slot.delay_samples = value.clamp(0, 4096))
            }
            SetIrPan(id, value) => self.update_slot(id, |slot| slot.pan = value.clamp(-1.0, 1.0)),
            SetIrPolarity(id, value) => self.update_slot(id, |slot| slot.polarity_inverted = value),
            SetIrNormalize(id, value) => self.update_slot(id, |slot| slot.normalize = value),
            SetIrSolo(id, value) => self.update_slot(id, |slot| slot.soloed = value),
            SetIrMute(id, value) => self.update_slot(id, |slot| slot.muted = value),
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
                self.snapshot.project.source.mode = mode;
                self.send(RuntimeCommand::SetSourceLive(mode == SourceMode::Live));
            }
            RestartPreview => {
                self.snapshot.project.source.elapsed_seconds = 0.0;
                self.send(RuntimeCommand::Restart);
            }
            PlayPreview => {
                if self.ensure_runtime() {
                    self.snapshot.project.source.transport = TransportState::Playing;
                    self.preview_start_pending = true;
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
                if value {
                    self.ensure_runtime();
                }
                self.send(RuntimeCommand::SetMonitoring(value));
            }
            SetInputDevice(id) => {
                self.snapshot.project.source.device = id;
                self.refresh_input_channels();
                self.restart_runtime();
            }
            SetOutputDevice(id) => {
                self.snapshot.project.output.device = id;
                self.refresh_output_channels();
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
            SetSampleRate(id) => {
                self.snapshot.project.source.sample_rate = id;
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

    fn ensure_runtime(&mut self) -> bool {
        if self.runtime.is_some() {
            return true;
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
                if let Some(preview) = &self.preview_audio {
                    self.send(RuntimeCommand::SetPreview {
                        normalization_gain: preview.normalization_gain(0.0),
                        audio: Box::new((**preview).clone()),
                    });
                }
                if self.snapshot.project.source.transport == TransportState::Playing {
                    self.preview_start_pending = true;
                    self.send(RuntimeCommand::Play);
                }
                let config = self.engine_config();
                let jobs: Vec<_> = self
                    .audio
                    .iter()
                    .filter_map(|(id, audio)| {
                        self.slot_params(*id)
                            .map(|params| (*id, Arc::clone(audio), params))
                    })
                    .collect();
                for (id, audio, params) in jobs {
                    let _ = self.worker_tx.send(WorkerRequest::PrepareIr {
                        id,
                        audio,
                        config,
                        params,
                    });
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
        if self.runtime.take().is_some()
            && (self.snapshot.project.source.monitoring
                || self.snapshot.project.source.transport == TransportState::Playing)
        {
            self.ensure_runtime();
        }
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
            let _ = self.worker_tx.send(WorkerRequest::LoadIr {
                id,
                path,
                config: self.engine_config(),
                params,
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
        self.audio.clear();
    }
    fn remove_ir(&mut self, id: IrId) {
        self.send(RuntimeCommand::RemoveSlot(id.0));
        self.audio.remove(&id);
        self.snapshot.project.ir_slots.retain(|slot| slot.id != id);
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
                self.audio.insert(id, source.clone());
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
                    slot.sample_rate_hz = source.sample_rate().0 as f32;
                    slot.metadata = format!(
                        "{} kHz | {} samples",
                        source.sample_rate().0 / 1000,
                        source.frame_count()
                    );
                    slot.waveform = source.waveform_envelope(180);
                    slot.load_state = ContentState::Ready;
                }
                self.send(RuntimeCommand::ReplaceSlot(prepared));
                if was_loading {
                    self.finish_ir_load(id);
                }
                self.snapshot.status = "Impulse response ready".into();
                self.snapshot.status_is_error = false;
                self.analysis_dirty = true;
            }
            WorkerResult::IrFailed { id, message } => {
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
            WorkerResult::PreviewReady { path, audio } => {
                self.snapshot.project.source.filename = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_owned);
                self.snapshot.project.source.file_path = Some(path);
                self.snapshot.project.source.duration_seconds =
                    audio.frame_count() as f64 / audio.sample_rate().0 as f64;
                self.snapshot.project.source.elapsed_seconds = 0.0;
                self.snapshot.project.source.metadata = format!(
                    "{} kHz | {:.3} s",
                    audio.sample_rate().0 / 1000,
                    self.snapshot.project.source.duration_seconds
                );
                self.snapshot.project.source.waveform = audio.waveform_envelope(320);
                self.snapshot.project.source.content_state = ContentState::Ready;
                self.snapshot.file_load_activity.preview_loading = false;
                let audio = Arc::new(*audio);
                self.preview_audio = Some(Arc::clone(&audio));
                self.send(RuntimeCommand::SetPreview {
                    normalization_gain: audio.normalization_gain(0.0),
                    audio: Box::new((*audio).clone()),
                });
            }
            WorkerResult::PreviewFailed(message) => {
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
                            .map(|path| (slot.id, path, params(slot)))
                    })
                    .collect();
                for (id, path, params) in slots {
                    let _ = self.worker_tx.send(WorkerRequest::LoadIr {
                        id,
                        path,
                        config: self.engine_config(),
                        params,
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
                self.audio.get(&slot.id).map(|audio| ExportSource {
                    label: slot.filename.clone(),
                    color_index: Some(slot.color_index),
                    audio: Arc::clone(audio),
                    params: params(slot),
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
            | RemoveIr(_)
            | MoveIrUp(_)
            | MoveIrDown(_)
            | SetIrEnabled(_, _)
            | SetIrGainDb(_, _)
            | SetIrDelaySamples(_, _)
            | SetIrPan(_, _)
            | SetIrPolarity(_, _)
            | SetIrNormalize(_, _)
            | SetIrSolo(_, _)
            | SetIrMute(_, _)
            | SetSampleRate(_)
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
        delay_samples: 0,
        sample_rate_hz: 48_000.0,
        pan: 0.0,
        polarity_inverted: false,
        normalize: false,
        soloed: false,
        muted: false,
        load_state: ContentState::Loading {
            message: "Preparing IR".into(),
        },
        waveform: Vec::new(),
    }
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
    fn channel_choices_follow_selected_device_topology() {
        use crate::runtime::DeviceInfo;

        let catalog = DeviceCatalog {
            inputs: vec![DeviceInfo {
                id: "input-0".into(),
                name: "Four input interface".into(),
                channels: 4,
            }],
            outputs: vec![
                DeviceInfo {
                    id: "output-0".into(),
                    name: "Mono output".into(),
                    channels: 1,
                },
                DeviceInfo {
                    id: "output-1".into(),
                    name: "Three output interface".into(),
                    channels: 3,
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
