use crate::{
    AnalysisState, AnalysisTab, AnalysisTrace, AppCommand, AppSnapshot, AudioBackend, BackendEvent,
    Choice, ContentState, ExportSettings, ExportState, FileLoadActivity, IrId, IrSlotState,
    OptionId, OutputState, ProjectState, SourceMode, SourceState, TransportState,
};

pub struct MockAudioBackend {
    snapshot: AppSnapshot,
    events: Vec<BackendEvent>,
    last_update_seconds: Option<f64>,
    next_ir_id: u64,
}

impl Default for MockAudioBackend {
    fn default() -> Self {
        Self {
            snapshot: demo_snapshot(),
            events: Vec::new(),
            last_update_seconds: None,
            next_ir_id: 7,
        }
    }
}

impl AudioBackend for MockAudioBackend {
    fn snapshot(&self) -> &AppSnapshot {
        &self.snapshot
    }

    fn dispatch(&mut self, command: AppCommand) {
        self.apply(command);
    }

    fn update(&mut self, now_seconds: f64) {
        let delta = self
            .last_update_seconds
            .replace(now_seconds)
            .map(|last| (now_seconds - last).clamp(0.0, 0.1))
            .unwrap_or(0.0);

        let source = &mut self.snapshot.project.source;
        if source.transport == TransportState::Playing && source.filename.is_some() {
            source.elapsed_seconds += delta;
            if source.elapsed_seconds >= source.duration_seconds {
                if source.looping {
                    source.elapsed_seconds = 0.0;
                } else {
                    source.elapsed_seconds = source.duration_seconds;
                    source.transport = TransportState::Stopped;
                }
            }
        }

        let t = now_seconds as f32;
        self.snapshot.output_levels_db = [-10.0 + t.sin() * 4.0, -12.0 + (t * 1.17).sin() * 5.0];
        self.snapshot.output_peaks_db = [-1.8, -2.5];
        self.snapshot.cpu_percent = 1.8 + (t * 0.37).sin().abs() * 1.1;
        for (trace_index, trace) in self.snapshot.spectrum_traces.iter_mut().enumerate() {
            for (sample_index, value) in trace.values.iter_mut().enumerate() {
                let x = sample_index as f32 / 180.0;
                *value =
                    -4.0 + (x * 8.0 + t * 1.7 + trace_index as f32).sin() * 3.0 - x.powi(4) * 13.0;
            }
        }

        let export = &mut self.snapshot.project.export.state;
        if let ExportState::Exporting { progress } = export {
            *progress = (*progress + delta as f32 * 0.7).min(1.0);
            if *progress >= 1.0 {
                let filename = self.snapshot.project.export.filename.clone();
                *export = ExportState::Complete {
                    message: format!("Exported {filename}"),
                };
                self.snapshot.status = "Mixed IR export complete".into();
                self.events.push(BackendEvent::ExportCompleted(filename));
            }
        }
    }

    fn drain_events(&mut self) -> Vec<BackendEvent> {
        std::mem::take(&mut self.events)
    }
}

impl MockAudioBackend {
    fn apply(&mut self, command: AppCommand) {
        use AppCommand::*;
        let changes_analysis = command_changes_analysis(&command);
        let changes_project = command_changes_project(&command);
        match command {
            SelectPreset(id) => {
                if self.snapshot.presets.iter().any(|preset| preset.id == id) {
                    let preset_key = id.0.clone();
                    self.snapshot.active_preset = id;
                    self.snapshot.project.name = self
                        .snapshot
                        .presets
                        .iter()
                        .find(|preset| preset.id == self.snapshot.active_preset)
                        .map(|preset| preset.label.clone())
                        .unwrap_or_else(|| "Untitled Mix".into());
                    if let Some(first) = self.snapshot.project.ir_slots.first_mut() {
                        first.gain_db = match preset_key.as_str() {
                            "tight" => -1.0,
                            "room" => -7.5,
                            _ => -3.0,
                        };
                    }
                    self.snapshot.project.dirty = false;
                    self.snapshot.status = "Preset loaded".into();
                }
            }
            SavePreset => {
                self.snapshot.project.dirty = false;
                self.snapshot.status = "Preset saved".into();
                self.events.push(BackendEvent::PresetSaved);
            }
            SavePresetAs => {
                let id = OptionId(format!("custom-{}", self.snapshot.presets.len() + 1));
                let label = format!("{} Copy", self.snapshot.project.name);
                self.snapshot.presets.push(Choice {
                    id: id.clone(),
                    label,
                });
                self.snapshot.active_preset = id;
                self.snapshot.project.dirty = false;
                self.snapshot.status = "Preset copy saved".into();
                self.events.push(BackendEvent::PresetSaved);
            }
            DeletePreset => {
                if self.snapshot.presets.len() > 1 {
                    let active = self.snapshot.active_preset.clone();
                    self.snapshot.presets.retain(|preset| preset.id != active);
                    self.snapshot.active_preset = self.snapshot.presets[0].id.clone();
                    self.snapshot.status = "Preset deleted".into();
                }
            }
            OpenSettings => self.snapshot.status = "Settings are mocked".into(),
            SetSourceMode(mode) => self.snapshot.project.source.mode = mode,
            BrowsePreview => {
                let source = &mut self.snapshot.project.source;
                source.filename = Some("guitar_DI_alt.wav".into());
                source.file_path = Some("Demo Audio/guitar_DI_alt.wav".into());
                source.metadata = "44.1 kHz | 24-bit | 00:29.180".into();
                source.duration_seconds = 29.18;
                source.elapsed_seconds = 0.0;
                source.content_state = ContentState::Ready;
            }
            RestartPreview => self.snapshot.project.source.elapsed_seconds = 0.0,
            PlayPreview => self.snapshot.project.source.transport = TransportState::Playing,
            PausePreview => self.snapshot.project.source.transport = TransportState::Paused,
            StopPreview => {
                self.snapshot.project.source.transport = TransportState::Stopped;
                self.snapshot.project.source.elapsed_seconds = 0.0;
            }
            SetLoop(value) => self.snapshot.project.source.looping = value,
            SetInputGainDb(value) => self.snapshot.project.source.gain_db = value,
            SetInputNormalize(value) => self.snapshot.project.source.normalize = value,
            SetInputDevice(id) => self.snapshot.project.source.device = id,
            SetInputChannel(id) => self.snapshot.project.source.channel = id,
            SetInputBufferSize(id) => self.snapshot.project.source.buffer_size = id,
            SetMonitoring(value) => self.snapshot.project.source.monitoring = value,
            AddIr => self.add_ir(),
            ClearAllIrs => {
                self.snapshot.project.ir_slots.clear();
                self.snapshot.project.selected_ir = None;
            }
            NormalizeAllIrs => {
                for slot in &mut self.snapshot.project.ir_slots {
                    slot.normalize = true;
                }
            }
            SetBalanceMode(value) => self.snapshot.project.set_balance_mode(value),
            SelectIr(id) => self.snapshot.project.selected_ir = Some(id),
            ReplaceIr(id) => {
                if let Some(slot) = self.slot_mut(id) {
                    slot.filename = format!("Replacement_{}.wav", id.0);
                    slot.file_path = Some(format!("Demo IRs/Replacement_{}.wav", id.0).into());
                    slot.load_state = ContentState::Ready;
                }
            }
            RemoveIr(id) => {
                self.snapshot.project.ir_slots.retain(|slot| slot.id != id);
                if self.snapshot.project.balance_mode {
                    self.snapshot.project.equalize_balance();
                }
                if self.snapshot.project.selected_ir == Some(id) {
                    self.snapshot.project.selected_ir =
                        self.snapshot.project.ir_slots.first().map(|slot| slot.id);
                }
            }
            MoveIrUp(id) => self.move_ir(id, -1),
            MoveIrDown(id) => self.move_ir(id, 1),
            SetIrEnabled(id, value) => self.with_slot(id, |slot| slot.enabled = value),
            SetIrGainDb(id, value) => self.with_slot(id, |slot| slot.gain_db = value),
            SetIrBalancePercent(id, value) => {
                self.snapshot.project.set_balance_percent(id, value);
            }
            SetIrDelaySamples(id, value) => self.with_slot(id, |slot| slot.delay_samples = value),
            SetIrPan(id, value) => self.with_slot(id, |slot| slot.pan = value),
            SetIrPolarity(id, value) => self.with_slot(id, |slot| slot.polarity_inverted = value),
            SetIrNormalize(id, value) => self.with_slot(id, |slot| slot.normalize = value),
            SetIrSolo(id, value) => self.with_slot(id, |slot| slot.soloed = value),
            SetIrMute(id, value) => self.with_slot(id, |slot| slot.muted = value),
            SetAnalysisTab(tab) => self.snapshot.project.analysis.tab = tab,
            SetAnalysisViewMode(id) => self.snapshot.project.analysis.view_mode = id,
            SetSmoothing(id) => self.snapshot.project.analysis.smoothing = id,
            SetOutputDevice(id) => self.snapshot.project.output.device = id,
            SetOutputChannel(id) => self.snapshot.project.output.channel = id,
            SetOutputBufferSize(id) => self.snapshot.project.output.buffer_size = id,
            SetOutputGainDb(value) => self.snapshot.project.output.gain_db = value,
            SetBypassed(value) => self.snapshot.project.output.bypassed = value,
            SetLimitOutput(value) => self.snapshot.project.output.limit_output = value,
            ChooseExportDestination => {
                self.snapshot.project.export.filename = "My_Mixed_IR_v2.wav".into();
                self.snapshot.project.export.destination =
                    Some("Mock Exports/My_Mixed_IR_v2.wav".into());
            }
            SetExportSampleRate(id) => self.snapshot.project.export.sample_rate = id,
            SetExportBitDepth(id) => self.snapshot.project.export.bit_depth = id,
            SetExportChannelMode(id) => self.snapshot.project.export.channel_mode = id,
            SetExportLength(id) => self.snapshot.project.export.length = id,
            SetTrimToLength(value) => self.snapshot.project.export.trim_to_length = value,
            SetExportNormalize(value) => self.snapshot.project.export.normalize = value,
            Export => {
                self.snapshot.project.export.state = ExportState::Exporting { progress: 0.0 };
                self.snapshot.status = "Exporting mixed IR".into();
            }
        }
        if changes_project {
            self.snapshot.project.dirty = true;
        }
        if changes_analysis {
            refresh_mock_analysis(&mut self.snapshot);
        }
    }

    fn slot_mut(&mut self, id: IrId) -> Option<&mut IrSlotState> {
        self.snapshot
            .project
            .ir_slots
            .iter_mut()
            .find(|slot| slot.id == id)
    }

    fn with_slot(&mut self, id: IrId, update: impl FnOnce(&mut IrSlotState)) {
        if let Some(slot) = self.slot_mut(id) {
            update(slot);
        }
    }

    fn move_ir(&mut self, id: IrId, offset: isize) {
        let slots = &mut self.snapshot.project.ir_slots;
        if let Some(index) = slots.iter().position(|slot| slot.id == id) {
            let target = index.saturating_add_signed(offset).min(slots.len() - 1);
            if target != index {
                slots.swap(index, target);
            }
        }
    }

    fn add_ir(&mut self) {
        let id = IrId(self.next_ir_id);
        self.next_ir_id += 1;
        let number = self.snapshot.project.ir_slots.len() + 1;
        let mut slot = demo_slot(id.0, &format!("Added_IR_{number}.wav"), -9.0, 0);
        slot.color_index = ((id.0 - 1) % 8) as u8;
        self.snapshot.project.ir_slots.push(slot);
        if self.snapshot.project.balance_mode {
            self.snapshot.project.equalize_balance();
        }
        self.snapshot.project.selected_ir = Some(id);
    }
}

fn command_changes_project(command: &AppCommand) -> bool {
    !matches!(
        command,
        AppCommand::SelectPreset(_)
            | AppCommand::SavePreset
            | AppCommand::SavePresetAs
            | AppCommand::DeletePreset
            | AppCommand::OpenSettings
            | AppCommand::PlayPreview
            | AppCommand::PausePreview
            | AppCommand::StopPreview
            | AppCommand::RestartPreview
            | AppCommand::Export
    )
}

fn command_changes_analysis(command: &AppCommand) -> bool {
    use AppCommand::*;
    matches!(
        command,
        SelectPreset(_)
            | AddIr
            | ClearAllIrs
            | NormalizeAllIrs
            | ReplaceIr(_)
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
            | SetOutputGainDb(_)
    )
}

fn refresh_mock_analysis(snapshot: &mut AppSnapshot) {
    let slots = &snapshot.project.ir_slots;
    let any_solo = slots.iter().any(|slot| slot.soloed);
    let output_gain = snapshot.project.output.gain_db;
    let mut combined_linear = vec![0.0_f32; 180];
    let mut combined_phase = vec![0.0_f32; 180];
    let mut active_count = 0.0_f32;
    let mut frequency = Vec::with_capacity(slots.len() + 1);
    let mut phase = Vec::with_capacity(slots.len() + 1);

    for (index, slot) in slots.iter().enumerate() {
        let audible = slot.enabled && !slot.muted && (!any_solo || slot.soloed);
        let level_db = if audible { slot.gain_db } else { -80.0 };
        let polarity = if slot.polarity_inverted { 180.0 } else { 0.0 };
        let frequency_values = (0..180)
            .map(|sample| {
                let x = sample as f32 / 179.0;
                level_db + (x * 7.0 + index as f32 * 0.8).sin() * 3.0 - x.powi(5) * 18.0
            })
            .collect::<Vec<_>>();
        let phase_values = (0..180)
            .map(|sample| {
                let x = sample as f32 / 179.0;
                ((x * 7.0 + index as f32 * 0.8).sin() * 36.0 - x * slot.delay_samples as f32 * 0.45
                    + polarity
                    + 180.0)
                    .rem_euclid(360.0)
                    - 180.0
            })
            .collect::<Vec<_>>();
        if audible {
            active_count += 1.0;
            for sample in 0..180 {
                combined_linear[sample] += 10.0_f32.powf(frequency_values[sample] / 20.0);
                combined_phase[sample] += phase_values[sample];
            }
        }
        frequency.push(AnalysisTrace {
            label: slot.filename.clone(),
            values: frequency_values,
            color_index: Some(slot.color_index),
            emphasized: false,
        });
        phase.push(AnalysisTrace {
            label: slot.filename.clone(),
            values: phase_values,
            color_index: Some(slot.color_index),
            emphasized: false,
        });
    }

    let combined_frequency = combined_linear
        .into_iter()
        .map(|value| 20.0 * value.max(0.0001).log10() + output_gain)
        .collect();
    if active_count > 0.0 {
        for value in &mut combined_phase {
            *value /= active_count;
        }
    }
    frequency.push(AnalysisTrace {
        label: "Sum (Mixed)".into(),
        values: combined_frequency,
        color_index: None,
        emphasized: true,
    });
    phase.push(AnalysisTrace {
        label: "Sum (Mixed)".into(),
        values: combined_phase,
        color_index: None,
        emphasized: true,
    });

    let output_scale = 10.0_f32.powf(output_gain / 20.0);
    snapshot.combined_waveform = (0..320)
        .map(|sample| {
            slots
                .iter()
                .filter(|slot| slot.enabled && !slot.muted && (!any_solo || slot.soloed))
                .map(|slot| {
                    let delay = (slot.delay_samples.max(0) as usize).min(sample);
                    let source_index = sample - delay;
                    let source = slot.waveform.get(source_index % slot.waveform.len().max(1));
                    let polarity = if slot.polarity_inverted { -1.0 } else { 1.0 };
                    source.copied().unwrap_or(0.0) * 10.0_f32.powf(slot.gain_db / 20.0) * polarity
                })
                .sum::<f32>()
                * output_scale
        })
        .collect();
    snapshot.frequency_traces = frequency;
    snapshot.phase_traces = phase;
}

fn demo_snapshot() -> AppSnapshot {
    let sample_rates = choices(&[
        ("44100", "44.1 kHz"),
        ("48000", "48 kHz"),
        ("96000", "96 kHz"),
    ]);
    let buffer_sizes = choices(&[
        ("64", "64 samples"),
        ("128", "128 samples (~2.7 ms)"),
        ("256", "256 samples"),
    ]);
    let project = ProjectState {
        name: "90-10 Close+Room".into(),
        ir_slots: vec![
            demo_slot(1, "York_Mix01.wav", -3.0, 0),
            demo_slot(2, "York_Room.wav", -12.0, 6),
            demo_slot(3, "V30_57.wav", -6.0, 0),
            demo_slot(4, "Greenback_121.wav", -8.0, 0),
            demo_slot(5, "421_Rear.wav", -18.0, 15),
            demo_slot(6, "Room_Far.wav", -20.0, 0),
        ],
        selected_ir: Some(IrId(1)),
        balance_mode: false,
        source: SourceState {
            mode: SourceMode::Preview,
            filename: Some("guitar_DI.wav".into()),
            file_path: Some("Demo Audio/guitar_DI.wav".into()),
            metadata: "44.1 kHz | 24-bit | 00:36.423".into(),
            waveform: waveform(0.3, 320),
            transport: TransportState::Stopped,
            elapsed_seconds: 0.0,
            duration_seconds: 36.423,
            looping: true,
            gain_db: 0.0,
            normalize: true,
            device: OptionId("ampero-input".into()),
            channel: OptionId("input-1".into()),
            sample_rate: OptionId("48000".into()),
            buffer_size: OptionId("128".into()),
            monitoring: true,
            content_state: ContentState::Ready,
        },
        output: OutputState {
            device: OptionId("ampero-output".into()),
            channel: OptionId("stereo-1-2".into()),
            buffer_size: OptionId("128".into()),
            gain_db: -2.0,
            bypassed: false,
            limit_output: true,
        },
        analysis: AnalysisState {
            tab: AnalysisTab::Frequency,
            view_mode: OptionId("magnitude".into()),
            smoothing: OptionId("1-12".into()),
            content_state: ContentState::Ready,
        },
        export: ExportSettings {
            filename: "My_Mixed_IR.wav".into(),
            destination: Some("Mock Exports/My_Mixed_IR.wav".into()),
            sample_rate: OptionId("48000".into()),
            bit_depth: OptionId("24".into()),
            channel_mode: OptionId("mono".into()),
            length: OptionId("2048".into()),
            trim_to_length: true,
            normalize: true,
            state: ExportState::Idle,
        },
        dirty: false,
    };
    AppSnapshot {
        project,
        presets: choices(&[
            ("close-room", "90-10 Close+Room"),
            ("tight", "Tight Modern"),
            ("room", "Wide Room"),
        ]),
        active_preset: OptionId("close-room".into()),
        input_devices: choices(&[
            ("ampero-input", "Ampero II USB Audio"),
            ("system", "System Default"),
        ]),
        input_channels: choices(&[("input-1", "Input 1"), ("input-2", "Input 2")]),
        sample_rates,
        buffer_sizes,
        output_devices: choices(&[
            ("ampero-output", "Ampero II USB Audio"),
            ("system", "System Default"),
        ]),
        output_channels: choices(&[("stereo-1-2", "1/2 (Stereo)"), ("mono-1", "1 (Mono)")]),
        analysis_view_modes: choices(&[("magnitude", "Magnitude (dB)"), ("phase", "Phase")]),
        smoothing_options: choices(&[("none", "None"), ("1-12", "1/12 Oct"), ("1-6", "1/6 Oct")]),
        bit_depths: choices(&[
            ("16", "16-bit PCM"),
            ("24", "24-bit PCM"),
            ("32f", "32-bit float"),
        ]),
        channel_modes: choices(&[("mono", "Mono"), ("stereo", "Stereo")]),
        export_lengths: choices(&[
            ("1024", "1024 samples"),
            ("2048", "2048 samples"),
            ("4096", "4096 samples"),
        ]),
        file_load_activity: FileLoadActivity::default(),
        frequency_traces: traces(false),
        phase_traces: traces(true),
        spectrum_traces: traces(false),
        combined_waveform: waveform(1.7, 320),
        output_levels_db: [-10.0, -12.0],
        output_peaks_db: [-1.8, -2.5],
        cpu_percent: 2.1,
        latency_ms: 5.3,
        status: "Audio Engine Running".into(),
        status_is_error: false,
    }
}

fn choices(items: &[(&str, &str)]) -> Vec<Choice> {
    items
        .iter()
        .map(|(id, label)| Choice::new(*id, *label))
        .collect()
}

fn demo_slot(id: u64, filename: &str, gain_db: f32, delay_samples: i32) -> IrSlotState {
    IrSlotState {
        id: IrId(id),
        filename: filename.into(),
        file_path: Some(format!("Demo IRs/{filename}").into()),
        file_reference: None,
        metadata: "48.0 kHz | 24-bit | 2048".into(),
        color_index: ((id - 1) % 8) as u8,
        enabled: id <= 4,
        gain_db,
        balance_percent: 0.0,
        delay_samples,
        sample_rate_hz: 48_000.0,
        pan: if id == 3 {
            -0.1
        } else if id == 4 {
            0.1
        } else {
            0.0
        },
        polarity_inverted: false,
        normalize: id <= 2,
        soloed: false,
        muted: false,
        load_state: ContentState::Ready,
        waveform: waveform(id as f32 * 0.71, 180),
    }
}

fn waveform(seed: f32, len: usize) -> Vec<f32> {
    (0..len)
        .map(|index| {
            let x = index as f32 / len.max(1) as f32;
            ((x * 42.0 + seed).sin() * 0.68 + (x * 97.0 + seed * 1.7).sin() * 0.22)
                * (-5.2 * x).exp()
        })
        .collect()
}

fn traces(phase: bool) -> Vec<AnalysisTrace> {
    let mut result = (0..4)
        .map(|index| AnalysisTrace {
            label: format!("IR {}", index + 1),
            values: (0..180)
                .map(|sample| {
                    let x = sample as f32 / 179.0;
                    let base = (x * 7.0 + index as f32 * 0.8).sin() * 3.0;
                    if phase {
                        base * 12.0
                    } else {
                        base - x.powi(5) * 18.0
                    }
                })
                .collect(),
            color_index: Some(index),
            emphasized: false,
        })
        .collect::<Vec<_>>();
    result.push(AnalysisTrace {
        label: "Sum (Mixed)".into(),
        values: (0..180)
            .map(|sample| {
                let x = sample as f32 / 179.0;
                (x * 5.0).sin() * if phase { 18.0 } else { 2.0 } - x.powi(5) * 16.0
            })
            .collect(),
        color_index: None,
        emphasized: true,
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PRESET_SCHEMA_VERSION, PresetDocument};

    #[test]
    fn mock_commands_update_state_and_preserve_color_identity() {
        let mut backend = MockAudioBackend::default();
        let original = backend.snapshot.project.ir_slots[1].color_index;
        let original_analysis = backend.snapshot.combined_waveform.clone();
        backend.dispatch(AppCommand::MoveIrUp(IrId(2)));
        assert_eq!(backend.snapshot.project.ir_slots[0].id, IrId(2));
        assert_eq!(backend.snapshot.project.ir_slots[0].color_index, original);
        backend.dispatch(AppCommand::SetIrMute(IrId(2), true));
        assert!(backend.snapshot.project.ir_slots[0].muted);
        assert_ne!(backend.snapshot.combined_waveform, original_analysis);
    }

    #[test]
    fn export_advances_to_completion() {
        let mut backend = MockAudioBackend::default();
        backend.dispatch(AppCommand::Export);
        backend.update(0.0);
        for step in 1..=20 {
            backend.update(step as f64 * 0.1);
        }
        assert!(matches!(
            backend.snapshot.project.export.state,
            ExportState::Complete { .. }
        ));
    }

    #[test]
    fn preset_document_round_trips_with_version() {
        let backend = MockAudioBackend::default();
        let document = PresetDocument::new(backend.snapshot.project.clone());
        let json = serde_json::to_string(&document).expect("demo preset should serialize");
        assert!(!json.contains("ampero-input"));
        assert!(!json.contains("ampero-output"));
        assert!(!json.contains("Mock Exports"));
        let decoded: PresetDocument =
            serde_json::from_str(&json).expect("demo preset should deserialize");
        assert_eq!(decoded.schema_version, PRESET_SCHEMA_VERSION);
        assert_eq!(decoded.project.ir_slots.len(), 6);
        assert!(decoded.validate_version().is_ok());
    }

    #[test]
    fn unsupported_preset_versions_are_rejected() {
        let backend = MockAudioBackend::default();
        let mut document = PresetDocument::new(backend.snapshot.project.clone());
        document.schema_version = PRESET_SCHEMA_VERSION + 1;
        assert_eq!(
            document.validate_version(),
            Err(crate::PresetVersionError {
                found: PRESET_SCHEMA_VERSION + 1,
                supported: PRESET_SCHEMA_VERSION,
            })
        );
    }

    #[test]
    fn schema_one_presets_migrate_file_references() {
        let backend = MockAudioBackend::default();
        let mut document = PresetDocument::new(backend.snapshot.project.clone());
        document.schema_version = 1;
        let migrated = document.migrate().expect("schema one should migrate");
        assert_eq!(migrated.schema_version, PRESET_SCHEMA_VERSION);
        assert!(migrated.project.ir_slots[0].file_reference.is_some());
    }
}
