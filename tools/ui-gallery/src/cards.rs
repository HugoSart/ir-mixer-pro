use ir_ui::components::*;

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
            sample_rates: &["44.1 kHz", "48 kHz", "96 kHz"],
            sample_rate: self.selections[2],
            buffer_sizes: &["128 samples", "256 samples", "512 samples"],
            buffer_size: self.selections[3],
            monitoring: self.monitoring,
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
                InputSourceAction::SetSampleRate(value) => self.selections[2] = value,
                InputSourceAction::SetBufferSize(value) => self.selections[3] = value,
                InputSourceAction::SetMonitoring(value) => self.monitoring = value,
            }
        }
        ui.label(egui::RichText::new(&self.last_action).small());
    }
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
            exporting: self.exporting_until.is_some_and(|until| now < until),
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
