use ir_ui::components::*;
use ir_ui::{
    DesignSystem, IR_COLORS,
    widgets::{deterministic_curve, deterministic_waveform},
};

const BALANCE_SILENCE_DB: f32 = -144.0;

pub struct CardsDemo {
    mode: SourceMode,
    transport: TransportState,
    looping: bool,
    gain: f32,
    normalize: bool,
    selections: [usize; 4],
    monitoring: bool,
    elapsed: f64,
    filename: &'static str,
    waveform: Vec<f32>,
    last_action: String,
}
impl Default for CardsDemo {
    fn default() -> Self {
        Self {
            mode: SourceMode::Preview,
            transport: TransportState::Stopped,
            looping: true,
            gain: 0.0,
            normalize: true,
            selections: [0; 4],
            monitoring: false,
            elapsed: 0.0,
            filename: "guitar_DI.wav",
            waveform: (0..2048)
                .map(|i| {
                    let t = i as f32 / 2048.0;
                    (t * 913.0).sin() * (0.2 + 0.6 * (t * 17.0).sin().abs())
                })
                .collect(),
            last_action: "Interact with either card to review its controls.".into(),
        }
    }
}
impl CardsDemo {
    pub fn live() -> Self {
        Self {
            mode: SourceMode::Live,
            ..Self::default()
        }
    }
    pub fn show(&mut self, ui: &mut egui::Ui, id: &'static str) {
        if self.transport == TransportState::Playing {
            self.elapsed += ui.input(|i| i.stable_dt) as f64;
            if self.elapsed >= 36.423 {
                if self.looping {
                    self.elapsed %= 36.423;
                } else {
                    self.elapsed = 36.423;
                    self.transport = TransportState::Stopped;
                }
            }
            ui.ctx().request_repaint();
        }
        let view = InputSourceCardView {
            mode: self.mode,
            filename: Some(self.filename),
            metadata: "44.1 kHz | 24-bit | 00:36.423",
            waveform: &self.waveform,
            transport: self.transport,
            elapsed_seconds: self.elapsed,
            duration_seconds: 36.423,
            looping: self.looping,
            gain_db: self.gain,
            normalize: self.normalize,
            devices: &["System Default", "Studio Interface"],
            device: self.selections[0],
            channels: &["Input 1 (Mono)", "Input 2 (Mono)", "Inputs 1/2 (Stereo)"],
            channel: self.selections[1],
            buffer_sizes: &["128 samples", "256 samples", "512 samples"],
            buffer_size: self.selections[3],
            monitoring: self.monitoring,
            standalone_routing: true,
            browse_enabled: true,
            browse_loading: false,
            content_status: ContentStatusView::Ready,
        };
        let actions = InputSourceCard::new(id, &view).width(320.0).show(ui).inner;
        for action in actions {
            self.last_action = format!("{action:?}");
            match action {
                InputSourceAction::SetMode(mode) => {
                    self.mode = mode;
                    self.transport = TransportState::Stopped;
                }
                InputSourceAction::Browse => {
                    self.filename = if self.filename == "guitar_DI.wav" {
                        "clean_rhythm.wav"
                    } else {
                        "guitar_DI.wav"
                    };
                    self.elapsed = 0.0;
                }
                InputSourceAction::Restart => self.elapsed = 0.0,
                InputSourceAction::Play => self.transport = TransportState::Playing,
                InputSourceAction::Pause => self.transport = TransportState::Paused,
                InputSourceAction::Stop => {
                    self.transport = TransportState::Stopped;
                    self.elapsed = 0.0;
                }
                InputSourceAction::SetLoop(value) => self.looping = value,
                InputSourceAction::SetGainDb(value) => self.gain = value,
                InputSourceAction::SetNormalize(value) => self.normalize = value,
                InputSourceAction::SetDevice(value) => self.selections[0] = value,
                InputSourceAction::SetChannel(value) => self.selections[1] = value,
                InputSourceAction::SetBufferSize(value) => self.selections[3] = value,
                InputSourceAction::SetMonitoring(value) => self.monitoring = value,
            }
        }
        ui.label(egui::RichText::new(&self.last_action).small());
    }
}

struct DummyIrSlot {
    id: u64,
    filename: String,
    waveform: Vec<f32>,
    color_index: usize,
    enabled: bool,
    gain_db: f32,
    balance_percent: f32,
    delay_samples: i32,
    pan: f32,
    polarity_inverted: bool,
    normalize: bool,
    soloed: bool,
    muted: bool,
}

pub struct IrRackDemo {
    slots: Vec<DummyIrSlot>,
    selected: Option<u64>,
    next_id: u64,
    balance_mode: bool,
    last_action: String,
}

impl Default for IrRackDemo {
    fn default() -> Self {
        Self {
            slots: vec![
                dummy_slot(1, "York_Mix01.wav", -3.0, 0, 0.0, true),
                dummy_slot(2, "York_Room.wav", -12.0, 6, 0.1, true),
                dummy_slot(3, "V30_57.wav", -6.0, 0, -0.1, true),
                dummy_slot(4, "Greenback_121.wav", -8.0, 0, 0.0, false),
                dummy_slot(5, "421_Rear.wav", -18.0, 15, 0.0, false),
                dummy_slot(6, "Room_Far.wav", -20.0, 0, 0.0, false),
            ],
            selected: Some(1),
            next_id: 7,
            balance_mode: false,
            last_action: "Six dummy IR slots are available for review.".into(),
        }
    }
}

impl IrRackDemo {
    pub fn show(&mut self, ui: &mut egui::Ui, width: f32) {
        let views = self
            .slots
            .iter()
            .enumerate()
            .map(|(index, slot)| IrRackSlotView {
                id: slot.id,
                number: index + 1,
                filename: &slot.filename,
                metadata: "48.0 kHz | 24-bit | 2048 samples",
                waveform: &slot.waveform,
                color: IR_COLORS[slot.color_index % IR_COLORS.len()],
                enabled: slot.enabled,
                gain_db: slot.gain_db,
                balance_percent: slot.balance_percent,
                delay_samples: slot.delay_samples,
                sample_rate: 48_000.0,
                pan: slot.pan,
                polarity_inverted: slot.polarity_inverted,
                normalize: slot.normalize,
                soloed: slot.soloed,
                muted: slot.muted,
                replace_enabled: true,
                replace_loading: false,
                load_status: ContentStatusView::Ready,
            })
            .collect::<Vec<_>>();
        let view = IrRackCardView {
            slots: &views,
            selected: self.selected,
            add_enabled: true,
            add_loading: false,
            balance_mode: self.balance_mode,
        };
        for action in IrRackCard::new("ir_rack_card", &view)
            .width(width)
            .rack_height(360.0)
            .show(ui)
            .inner
        {
            self.last_action = format!("{action:?}");
            self.apply(action);
        }
        ui.label(egui::RichText::new(&self.last_action).small());
    }

    fn apply(&mut self, action: IrRackAction) {
        match action {
            IrRackAction::EditGlobalEqualizer => {}
            IrRackAction::AddIr => {
                let id = self.next_id;
                self.next_id += 1;
                self.slots.push(dummy_slot(
                    id,
                    &format!("New_IR_{id:02}.wav"),
                    -9.0,
                    0,
                    0.0,
                    true,
                ));
                if self.balance_mode {
                    self.equalize_balance();
                }
            }
            IrRackAction::ClearAll => self.slots.clear(),
            IrRackAction::NormalizeAll => {
                for slot in &mut self.slots {
                    slot.normalize = true;
                }
            }
            IrRackAction::SetBalanceMode(enabled) => self.set_balance_mode(enabled),
            IrRackAction::Select { id } => self.selected = Some(id),
            IrRackAction::Remove { id } => {
                self.slots.retain(|slot| slot.id != id);
                if self.balance_mode {
                    self.equalize_balance();
                }
            }
            IrRackAction::MoveUp { id } => self.move_slot(id, -1),
            IrRackAction::MoveDown { id } => self.move_slot(id, 1),
            IrRackAction::Browse { id } => with_slot(&mut self.slots, id, |slot| {
                slot.filename = if slot.filename.ends_with("_alt.wav") {
                    slot.filename.trim_end_matches("_alt.wav").to_owned() + ".wav"
                } else {
                    slot.filename.trim_end_matches(".wav").to_owned() + "_alt.wav"
                };
            }),
            IrRackAction::SetEnabled { id, enabled } => {
                with_slot(&mut self.slots, id, |slot| slot.enabled = enabled)
            }
            IrRackAction::SetGainDb { id, gain_db } => {
                with_slot(&mut self.slots, id, |slot| slot.gain_db = gain_db)
            }
            IrRackAction::SetBalancePercent { id, percent } => {
                self.set_balance_percent(id, percent);
            }
            IrRackAction::SetDelaySamples { id, delay_samples } => {
                with_slot(&mut self.slots, id, |slot| {
                    slot.delay_samples = delay_samples
                })
            }
            IrRackAction::SetPan { id, pan } => {
                with_slot(&mut self.slots, id, |slot| slot.pan = pan)
            }
            IrRackAction::SetPolarity { id, inverted } => with_slot(&mut self.slots, id, |slot| {
                slot.polarity_inverted = inverted
            }),
            IrRackAction::SetNormalize { id, normalize } => {
                with_slot(&mut self.slots, id, |slot| slot.normalize = normalize)
            }
            IrRackAction::SetSolo { id, soloed } => {
                with_slot(&mut self.slots, id, |slot| slot.soloed = soloed)
            }
            IrRackAction::SetMute { id, muted } => {
                with_slot(&mut self.slots, id, |slot| slot.muted = muted)
            }
            IrRackAction::EditEqualizer { id } => self.selected = Some(id),
        }
    }

    fn move_slot(&mut self, id: u64, offset: isize) {
        if let Some(index) = self.slots.iter().position(|slot| slot.id == id) {
            let target = index
                .saturating_add_signed(offset)
                .min(self.slots.len().saturating_sub(1));
            if target != index {
                self.slots.swap(index, target);
            }
        }
    }

    fn set_balance_mode(&mut self, enabled: bool) {
        self.balance_mode = enabled;
        if enabled {
            self.normalize_balance_from_gains();
        }
    }

    fn equalize_balance(&mut self) {
        let count = self.slots.len();
        if count == 0 {
            return;
        }

        let share = 100.0 / count as f32;
        let mut assigned = 0.0;
        for slot in self.slots.iter_mut().take(count - 1) {
            slot.balance_percent = share;
            slot.gain_db = balance_percent_to_gain_db(share);
            assigned += share;
        }
        if let Some(slot) = self.slots.last_mut() {
            slot.balance_percent = 100.0 - assigned;
            slot.gain_db = balance_percent_to_gain_db(slot.balance_percent);
        }
    }

    fn set_balance_percent(&mut self, id: u64, percent: f32) {
        let Some(selected_index) = self.slots.iter().position(|slot| slot.id == id) else {
            return;
        };
        let count = self.slots.len();
        if count == 1 {
            self.equalize_balance();
            return;
        }

        let percent = percent.clamp(0.0, 100.0);
        let other_indices = (0..count)
            .filter(|index| *index != selected_index)
            .collect::<Vec<_>>();
        let current_other_total: f32 = other_indices
            .iter()
            .map(|index| self.slots[*index].balance_percent.max(0.0))
            .sum();
        let remaining = 100.0 - percent;
        self.slots[selected_index].balance_percent = percent;

        let mut assigned = percent;
        for index in other_indices.iter().take(other_indices.len() - 1) {
            let next = if current_other_total > f32::EPSILON {
                self.slots[*index].balance_percent.max(0.0) / current_other_total * remaining
            } else {
                remaining / other_indices.len() as f32
            };
            self.slots[*index].balance_percent = next;
            assigned += next;
        }
        if let Some(last_index) = other_indices.last() {
            self.slots[*last_index].balance_percent = (100.0 - assigned).max(0.0);
        }
        self.sync_balance_gains();
    }

    fn normalize_balance_from_gains(&mut self) {
        if self.slots.len() <= 1 {
            self.equalize_balance();
            return;
        }

        let total: f32 = self
            .slots
            .iter()
            .map(|slot| 10.0_f32.powf(slot.gain_db / 20.0).max(0.0))
            .sum();
        if total <= f32::EPSILON {
            self.equalize_balance();
            return;
        }

        let mut assigned = 0.0;
        let last = self.slots.len() - 1;
        for slot in self.slots.iter_mut().take(last) {
            slot.balance_percent = 10.0_f32.powf(slot.gain_db / 20.0).max(0.0) / total * 100.0;
            assigned += slot.balance_percent;
        }
        self.slots[last].balance_percent = 100.0 - assigned;
        self.sync_balance_gains();
    }

    fn sync_balance_gains(&mut self) {
        for slot in &mut self.slots {
            slot.gain_db = balance_percent_to_gain_db(slot.balance_percent);
        }
    }
}

fn balance_percent_to_gain_db(percent: f32) -> f32 {
    let linear_gain =
        (percent.clamp(0.0, 100.0) / 100.0).max(10.0_f32.powf(BALANCE_SILENCE_DB / 20.0));
    20.0 * linear_gain.log10()
}

fn dummy_slot(
    id: u64,
    filename: &str,
    gain_db: f32,
    delay_samples: i32,
    pan: f32,
    enabled: bool,
) -> DummyIrSlot {
    DummyIrSlot {
        id,
        filename: filename.into(),
        waveform: deterministic_waveform(id as f32 * 0.73, 256),
        color_index: (id as usize - 1) % IR_COLORS.len(),
        enabled,
        gain_db,
        balance_percent: 0.0,
        delay_samples,
        pan,
        polarity_inverted: false,
        normalize: id < 3,
        soloed: false,
        muted: false,
    }
}

fn with_slot(slots: &mut [DummyIrSlot], id: u64, update: impl FnOnce(&mut DummyIrSlot)) {
    if let Some(slot) = slots.iter_mut().find(|slot| slot.id == id) {
        update(slot);
    }
}

pub struct AnalysisDemo {
    selected_tab: usize,
    view_mode: usize,
    smoothing: usize,
    frequency: Vec<Vec<f32>>,
    phase: Vec<Vec<f32>>,
    spectrum: Vec<Vec<f32>>,
    impulse: Vec<f32>,
    last_action: String,
}

impl Default for AnalysisDemo {
    fn default() -> Self {
        Self {
            selected_tab: 0,
            view_mode: 0,
            smoothing: 1,
            frequency: (0..5)
                .map(|index| deterministic_curve(index as f32 * 0.83, 220))
                .collect(),
            phase: (0..4)
                .map(|index| deterministic_curve(index as f32 * 1.17 + 0.4, 220))
                .collect(),
            spectrum: (0..3)
                .map(|index| deterministic_curve(index as f32 * 0.61 + 1.2, 220))
                .collect(),
            impulse: deterministic_waveform(0.35, 512),
            last_action: "Choose an analysis tab or display option.".into(),
        }
    }
}

impl AnalysisDemo {
    pub fn show(&mut self, ui: &mut egui::Ui, width: f32) {
        let ds = DesignSystem::default();
        let frequency = analysis_traces(&self.frequency, true, ds.colors.text_primary);
        let phase = analysis_traces(&self.phase, false, ds.colors.text_primary);
        let spectrum = analysis_traces(&self.spectrum, false, ds.colors.text_primary);
        let view = AnalysisPreviewCardView {
            selected_tab: self.selected_tab,
            view_modes: &["Magnitude (dB)", "Magnitude (linear)"],
            view_mode: self.view_mode,
            smoothing_options: &["None", "1/12 Oct", "1/6 Oct", "1/3 Oct"],
            smoothing: self.smoothing,
            frequency_traces: &frequency,
            impulse_waveform: &self.impulse,
            impulse_color: IR_COLORS[0],
            phase_traces: &phase,
            spectrum_traces: &spectrum,
            sample_rate: "48.0 kHz",
            ir_length: "2048 samples (42.7 ms)",
            latency: "5.3 ms",
            cpu: "2.1%",
            content_status: ContentStatusView::Ready,
        };
        for action in AnalysisPreviewCard::new("analysis_card", &view)
            .width(width)
            .show(ui)
            .inner
        {
            self.last_action = format!("{action:?}");
            match action {
                AnalysisPreviewAction::SetTab(value) => self.selected_tab = value,
                AnalysisPreviewAction::SetViewMode(value) => self.view_mode = value,
                AnalysisPreviewAction::SetSmoothing(value) => self.smoothing = value,
            }
        }
        ui.label(egui::RichText::new(&self.last_action).small());
    }
}

fn analysis_traces<'a>(
    values: &'a [Vec<f32>],
    include_mix: bool,
    mixed_color: egui::Color32,
) -> Vec<AnalysisTraceView<'a>> {
    const LABELS: [&str; 5] = ["IR 1", "IR 2", "IR 3", "IR 4", "Sum (Mixed)"];
    values
        .iter()
        .enumerate()
        .map(|(index, curve)| {
            let emphasized = include_mix && index + 1 == values.len();
            AnalysisTraceView {
                label: LABELS[index.min(LABELS.len() - 1)],
                values: curve,
                color: if emphasized {
                    mixed_color
                } else {
                    IR_COLORS[index % IR_COLORS.len()]
                },
                emphasized,
            }
        })
        .collect()
}

pub struct OutputDemo {
    selections: [usize; 3],
    gain_db: f32,
    bypassed: bool,
    limit_output: bool,
    last_action: String,
}

impl Default for OutputDemo {
    fn default() -> Self {
        Self {
            selections: [0; 3],
            gain_db: -2.0,
            bypassed: false,
            limit_output: true,
            last_action: "Output is using dummy device data.".into(),
        }
    }
}

impl OutputDemo {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let time = ui.input(|input| input.time) as f32;
        let levels = [-10.0 + time.sin() * 4.0, -12.0 + (time * 1.17).sin() * 5.0];
        let view = OutputCardView {
            devices: &["Ampero II USB Audio", "Studio Interface"],
            device: self.selections[0],
            channels: &["1/2 (Stereo)", "1 (Mono)", "2 (Mono)"],
            channel: self.selections[1],
            buffer_sizes: &["128 samples (≈ 2.7 ms)", "256 samples (≈ 5.3 ms)"],
            buffer_size: self.selections[2],
            gain_db: self.gain_db,
            bypassed: self.bypassed,
            limit_output: self.limit_output,
            levels_db: &levels,
            peaks_db: &[-1.8, -2.5],
            standalone_routing: true,
        };
        for action in OutputCard::new("output_card", &view)
            .width(320.0)
            .show(ui)
            .inner
        {
            self.last_action = format!("{action:?}");
            match action {
                OutputAction::SetDevice(value) => self.selections[0] = value,
                OutputAction::SetChannel(value) => self.selections[1] = value,
                OutputAction::SetBufferSize(value) => self.selections[2] = value,
                OutputAction::SetGainDb(value) => self.gain_db = value,
                OutputAction::SetBypass(value) => self.bypassed = value,
                OutputAction::SetLimitOutput(value) => self.limit_output = value,
            }
        }
        ui.label(egui::RichText::new(&self.last_action).small());
        ui.ctx().request_repaint();
    }
}

pub struct ExportDemo {
    filename: &'static str,
    selections: [usize; 4],
    trim: bool,
    normalize: bool,
    exporting_until: Option<f64>,
    last_action: String,
}

impl Default for ExportDemo {
    fn default() -> Self {
        Self {
            filename: "My_Mixed_IR.wav",
            selections: [1, 1, 0, 0],
            trim: true,
            normalize: true,
            exporting_until: None,
            last_action: "Ready to export dummy IR data.".into(),
        }
    }
}

impl ExportDemo {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let now = ui.input(|input| input.time);
        if self.exporting_until.is_some_and(|until| now >= until) {
            self.exporting_until = None;
            self.last_action = "Dummy export complete: My_Mixed_IR.wav".into();
        }
        let view = ExportMixedIrCardView {
            filename: self.filename,
            metadata: "48.0 kHz | 24-bit | WAV",
            sample_rates: &["44.1 kHz", "48 kHz", "96 kHz"],
            sample_rate: self.selections[0],
            bit_depths: &["16-bit PCM", "24-bit PCM", "32-bit float"],
            bit_depth: self.selections[1],
            channel_modes: &["Mono", "Stereo"],
            channel_mode: self.selections[2],
            lengths: &["2048 samples", "4096 samples", "8192 samples"],
            length: self.selections[3],
            trim_to_length: self.trim,
            normalize: self.normalize,
            status: if self.exporting_until.is_some_and(|until| now < until) {
                ExportStatusView::Exporting { progress: 0.45 }
            } else {
                ExportStatusView::Idle
            },
        };
        for action in ExportMixedIrCard::new("export_card", &view)
            .width(320.0)
            .show(ui)
            .inner
        {
            self.last_action = format!("{action:?}");
            match action {
                ExportMixedIrAction::ChooseDestination => {
                    self.filename = if self.filename == "My_Mixed_IR.wav" {
                        "Studio_Mix.wav"
                    } else {
                        "My_Mixed_IR.wav"
                    };
                }
                ExportMixedIrAction::SetSampleRate(value) => self.selections[0] = value,
                ExportMixedIrAction::SetBitDepth(value) => self.selections[1] = value,
                ExportMixedIrAction::SetChannelMode(value) => self.selections[2] = value,
                ExportMixedIrAction::SetLength(value) => self.selections[3] = value,
                ExportMixedIrAction::SetTrimToLength(value) => self.trim = value,
                ExportMixedIrAction::SetNormalize(value) => self.normalize = value,
                ExportMixedIrAction::Export => self.exporting_until = Some(now + 1.0),
            }
        }
        ui.label(egui::RichText::new(&self.last_action).small());
        if self.exporting_until.is_some() {
            ui.ctx().request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balance_mode_normalizes_gains_and_redistributes_edits() {
        let mut demo = IrRackDemo::default();

        demo.set_balance_mode(true);
        assert!(demo.balance_mode);
        assert!((balance_total(&demo) - 100.0).abs() < 0.000_1);

        demo.set_balance_percent(1, 50.0);
        assert!((demo.slots[0].balance_percent - 50.0).abs() < f32::EPSILON);
        assert!((balance_total(&demo) - 100.0).abs() < 0.000_1);
        for slot in &demo.slots {
            assert!(
                (slot.gain_db - balance_percent_to_gain_db(slot.balance_percent)).abs() < 0.000_1
            );
        }
    }

    #[test]
    fn balance_mode_equalizes_after_adding_and_removing_slots() {
        let mut demo = IrRackDemo::default();
        demo.set_balance_mode(true);

        demo.apply(IrRackAction::Remove { id: 1 });
        assert!(
            demo.slots
                .iter()
                .all(|slot| (slot.balance_percent - 20.0).abs() < 0.000_1)
        );

        demo.apply(IrRackAction::AddIr);
        assert_eq!(demo.slots.len(), 6);
        assert!((balance_total(&demo) - 100.0).abs() < 0.000_1);
        assert!(demo.slots.iter().all(|slot| {
            (slot.balance_percent - 100.0 / demo.slots.len() as f32).abs() < 0.000_1
        }));
    }

    fn balance_total(demo: &IrRackDemo) -> f32 {
        demo.slots.iter().map(|slot| slot.balance_percent).sum()
    }
}
