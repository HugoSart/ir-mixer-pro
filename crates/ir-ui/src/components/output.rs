use super::CardFrame;
use crate::{DesignSystem, widgets::*};
use egui::{InnerResponse, Ui};
use egui_lucide::Lucide;

pub struct OutputCardView<'a> {
    pub devices: &'a [&'a str],
    pub device: usize,
    pub channels: &'a [&'a str],
    pub channel: usize,
    pub buffer_sizes: &'a [&'a str],
    pub buffer_size: usize,
    pub gain_db: f32,
    pub bypassed: bool,
    pub limit_output: bool,
    pub levels_db: &'a [f32],
    pub peaks_db: &'a [f32],
    pub standalone_routing: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OutputAction {
    SetDevice(usize),
    SetChannel(usize),
    SetBufferSize(usize),
    SetGainDb(f32),
    SetBypass(bool),
    SetLimitOutput(bool),
}

pub struct OutputCard<'a> {
    view: &'a OutputCardView<'a>,
    id: egui::Id,
    width: f32,
}

impl<'a> OutputCard<'a> {
    pub fn new(id: impl egui::AsId, view: &'a OutputCardView<'a>) -> Self {
        Self {
            view,
            id: egui::Id::new(id),
            width: 300.0,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(280.0);
        self
    }

    pub fn show(self, ui: &mut Ui) -> InnerResponse<Vec<OutputAction>> {
        ui.push_id(self.id, |ui| {
            CardFrame::new("Output")
                .icon(Lucide::Volume2)
                .width(self.width)
                .show(ui, |ui| {
                    let v = self.view;
                    let mut actions = Vec::new();
                    if v.standalone_routing {
                        let mut device = v.device;
                        ui.add(
                            DropdownSelector::new("output_device", v.devices, &mut device)
                                .width(ui.available_width())
                                .label("Output Device", SelectorLabelPosition::Top),
                        );
                        if device != v.device {
                            actions.push(OutputAction::SetDevice(device));
                        }

                        let mut channel = v.channel;
                        ui.add(
                            DropdownSelector::new("output_channels", v.channels, &mut channel)
                                .width(ui.available_width())
                                .label("Output Channels", SelectorLabelPosition::Top),
                        );
                        if channel != v.channel {
                            actions.push(OutputAction::SetChannel(channel));
                        }
                    } else {
                        DesignSystem::from_context(ui.ctx())
                            .inset_frame()
                            .show(ui, |ui| {
                                ui.label("Audio output is routed by the plugin host.");
                            });
                    }

                    ui.add_space(4.0);
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            let mut gain = v.gain_db;
                            ui.add(
                                AudioKnob::new(&mut gain, -60.0..=12.0, "Output Gain")
                                    .default_value(0.0)
                                    .suffix(" dB"),
                            );
                            ui.add(DbValueEditor::new(&mut gain, -60.0..=12.0));
                            if gain != v.gain_db {
                                actions.push(OutputAction::SetGainDb(gain));
                            }
                        });
                        ui.add_space(8.0);
                        ui.add(
                            LevelMeter::new(v.levels_db, v.peaks_db)
                                .size(egui::vec2(62.0, 154.0))
                                .accent(DesignSystem::from_context(ui.ctx()).colors.status_success),
                        );
                    });

                    if v.standalone_routing {
                        let mut buffer_size = v.buffer_size;
                        ui.add(
                            DropdownSelector::new(
                                "output_buffer",
                                v.buffer_sizes,
                                &mut buffer_size,
                            )
                            .width(ui.available_width())
                            .label("Buffer Size", SelectorLabelPosition::Top),
                        );
                        if buffer_size != v.buffer_size {
                            actions.push(OutputAction::SetBufferSize(buffer_size));
                        }
                    }

                    let mut limit_output = v.limit_output;
                    let mut bypassed = v.bypassed;
                    ui.add(Checkbox::new(
                        &mut limit_output,
                        "Limit Output (Prevent Clipping)",
                    ));
                    ui.add(Checkbox::new(&mut bypassed, "Bypass"));
                    if limit_output != v.limit_output {
                        actions.push(OutputAction::SetLimitOutput(limit_output));
                    }
                    if bypassed != v.bypassed {
                        actions.push(OutputAction::SetBypass(bypassed));
                    }
                    actions
                })
        })
        .inner
    }
}
