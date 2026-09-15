use super::{CardFrame, CardVariant, ContentStatusView};
use crate::{DesignSystem, TextRole, widgets::*};
use egui::{InnerResponse, RichText, Ui, vec2};
use egui_lucide::Lucide;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SourceMode {
    #[default]
    Preview,
    Live,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransportState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

/// Borrowed display data; selection and transport are owned by the caller.
pub struct InputSourceCardView<'a> {
    pub mode: SourceMode,
    pub filename: Option<&'a str>,
    pub metadata: &'a str,
    pub waveform: &'a [f32],
    pub transport: TransportState,
    pub elapsed_seconds: f64,
    pub duration_seconds: f64,
    pub looping: bool,
    pub gain_db: f32,
    pub normalize: bool,
    pub devices: &'a [&'a str],
    pub device: usize,
    pub channels: &'a [&'a str],
    pub channel: usize,
    pub sample_rates: &'a [&'a str],
    pub sample_rate: usize,
    pub buffer_sizes: &'a [&'a str],
    pub buffer_size: usize,
    pub monitoring: bool,
    pub standalone_routing: bool,
    pub content_status: ContentStatusView<'a>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum InputSourceAction {
    SetMode(SourceMode),
    Browse,
    Restart,
    Play,
    Pause,
    Stop,
    SetLoop(bool),
    SetGainDb(f32),
    SetNormalize(bool),
    SetDevice(usize),
    SetChannel(usize),
    SetSampleRate(usize),
    SetBufferSize(usize),
    SetMonitoring(bool),
}

pub struct InputSourceCard<'a> {
    view: &'a InputSourceCardView<'a>,
    id: egui::Id,
    width: f32,
    min_height: f32,
}
impl<'a> InputSourceCard<'a> {
    pub fn new(id: impl egui::AsId, view: &'a InputSourceCardView<'a>) -> Self {
        Self {
            view,
            id: egui::Id::new(id),
            width: 300.0,
            min_height: 0.0,
        }
    }
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(280.0);
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = height.max(0.0);
        self
    }
    pub fn show(self, ui: &mut Ui) -> InnerResponse<Vec<InputSourceAction>> {
        ui.push_id(self.id, |ui| {
            CardFrame::new("Input Source")
                .icon(Lucide::Cable)
                .width(self.width)
                .min_height(self.min_height)
                .show(ui, |ui| {
                    let mut actions = Vec::new();
                    let v = self.view;
                    let mut mode = usize::from(v.mode == SourceMode::Live);
                    ui.add(SegmentedControl::new(
                        &["Preview File", "Live Input"],
                        &mut mode,
                    ));
                    let new_mode = if mode == 0 {
                        SourceMode::Preview
                    } else {
                        SourceMode::Live
                    };
                    if new_mode != v.mode {
                        actions.push(InputSourceAction::SetMode(new_mode));
                    }
                    ui.add_space(4.0);
                    super::status::status_banner(ui, v.content_status);
                    match v.mode {
                        SourceMode::Preview => self.preview(ui, &mut actions),
                        SourceMode::Live => self.live(ui, &mut actions),
                    }
                    ui.add_space(8.0);
                    CardFrame::new("Input Options")
                        .variant(CardVariant::Inset)
                        .width(ui.available_width())
                        .show(ui, |ui| {
                            let mut gain = v.gain_db;
                            let mut normalize = v.normalize;
                            ui.horizontal(|ui| {
                                ui.add(
                                    AudioKnob::new(&mut gain, -60.0..=12.0, "Input Gain")
                                        .suffix(" dB"),
                                );
                                ui.add(DbValueEditor::new(&mut gain, -60.0..=12.0));
                                ui.add(Checkbox::new(&mut normalize, "Normalize"));
                            });
                            if gain != v.gain_db {
                                actions.push(InputSourceAction::SetGainDb(gain));
                            }
                            if normalize != v.normalize {
                                actions.push(InputSourceAction::SetNormalize(normalize));
                            }
                        });
                    actions
                })
        })
        .inner
    }
    fn preview(&self, ui: &mut Ui, actions: &mut Vec<InputSourceAction>) {
        let v = self.view;
        let ds = DesignSystem::from_context(ui.ctx());
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 48.0).max(80.0);
            ds.inset_frame().show(ui, |ui| {
                ui.set_width(width - 16.0);
                ui.add(egui::Label::new(v.filename.unwrap_or("Choose a preview file")).truncate())
                    .on_hover_text(v.filename.unwrap_or("No file loaded"));
            });
            if ui
                .add(IconButton::new(Lucide::FolderOpen, "Browse preview file"))
                .clicked()
            {
                actions.push(InputSourceAction::Browse);
            }
        });
        ui.label(
            RichText::new(v.metadata)
                .font(TextRole::Metadata.font_id())
                .color(ds.colors.text_secondary),
        );
        ui.add(
            WaveformView::new(v.waveform, ds.colors.accent)
                .size(vec2(ui.available_width(), 100.0))
                .render_mode(WaveformRenderMode::Symmetric),
        );
        ui.horizontal(|ui| {
            let enabled = v.filename.is_some();
            if ui
                .add(IconButton::new(Lucide::SkipBack, "Return to start").enabled(enabled))
                .clicked()
            {
                actions.push(InputSourceAction::Restart);
            }
            let playing = v.transport == TransportState::Playing;
            if ui
                .add(
                    IconButton::new(
                        if playing { Lucide::Pause } else { Lucide::Play },
                        if playing {
                            "Pause preview"
                        } else {
                            "Play preview"
                        },
                    )
                    .enabled(enabled),
                )
                .clicked()
            {
                actions.push(if playing {
                    InputSourceAction::Pause
                } else {
                    InputSourceAction::Play
                });
            }
            if ui
                .add(IconButton::new(Lucide::Square, "Stop preview").enabled(enabled))
                .clicked()
            {
                actions.push(InputSourceAction::Stop);
            }
            let mut looping = v.looping;
            ui.add(Checkbox::new(&mut looping, "Loop"));
            if looping != v.looping {
                actions.push(InputSourceAction::SetLoop(looping));
            }
        });
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!(
                    "{} / {}",
                    time_text(v.elapsed_seconds),
                    time_text(v.duration_seconds)
                ));
            });
        });
    }
    fn live(&self, ui: &mut Ui, actions: &mut Vec<InputSourceAction>) {
        let v = self.view;
        if !v.standalone_routing {
            let ds = DesignSystem::from_context(ui.ctx());
            ds.inset_frame().show(ui, |ui| {
                ui.label("Audio input and buffer configuration are provided by the plugin host.");
            });
            return;
        }
        for (label, items, selected, action) in [
            (
                "Input device",
                v.devices,
                v.device,
                InputSourceAction::SetDevice as fn(usize) -> InputSourceAction,
            ),
            (
                "Input channel",
                v.channels,
                v.channel,
                InputSourceAction::SetChannel,
            ),
            (
                "Sample rate",
                v.sample_rates,
                v.sample_rate,
                InputSourceAction::SetSampleRate,
            ),
            (
                "Buffer size",
                v.buffer_sizes,
                v.buffer_size,
                InputSourceAction::SetBufferSize,
            ),
        ] {
            let mut value = selected;
            ui.add(
                DropdownSelector::new(label, items, &mut value)
                    .width(ui.available_width())
                    .label(label, SelectorLabelPosition::Top),
            );
            if value != selected {
                actions.push(action(value));
            }
        }
        let mut monitoring = v.monitoring;
        ui.add(Checkbox::new(&mut monitoring, "Monitor input"));
        if monitoring != v.monitoring {
            actions.push(InputSourceAction::SetMonitoring(monitoring));
        }
    }
}

fn time_text(seconds: f64) -> String {
    let seconds = if seconds.is_finite() {
        seconds.max(0.0) as u64
    } else {
        0
    };
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}
