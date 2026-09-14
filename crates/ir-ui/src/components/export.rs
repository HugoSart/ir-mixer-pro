use super::CardFrame;
use crate::{DesignSystem, TextRole, widgets::*};
use egui::{InnerResponse, RichText, Ui};
use egui_lucide::Lucide;

pub struct ExportMixedIrCardView<'a> {
    pub filename: &'a str,
    pub metadata: &'a str,
    pub sample_rates: &'a [&'a str],
    pub sample_rate: usize,
    pub bit_depths: &'a [&'a str],
    pub bit_depth: usize,
    pub channel_modes: &'a [&'a str],
    pub channel_mode: usize,
    pub lengths: &'a [&'a str],
    pub length: usize,
    pub trim_to_length: bool,
    pub normalize: bool,
    pub exporting: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExportMixedIrAction {
    ChooseDestination,
    SetSampleRate(usize),
    SetBitDepth(usize),
    SetChannelMode(usize),
    SetLength(usize),
    SetTrimToLength(bool),
    SetNormalize(bool),
    Export,
}

pub struct ExportMixedIrCard<'a> {
    view: &'a ExportMixedIrCardView<'a>,
    id: egui::Id,
    width: f32,
}

impl<'a> ExportMixedIrCard<'a> {
    pub fn new(id: impl egui::AsId, view: &'a ExportMixedIrCardView<'a>) -> Self {
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

    pub fn show(self, ui: &mut Ui) -> InnerResponse<Vec<ExportMixedIrAction>> {
        ui.push_id(self.id, |ui| {
            CardFrame::new("Export Mixed IR")
                .number(6)
                .width(self.width)
                .show(ui, |ui| {
                    let v = self.view;
                    let ds = DesignSystem::default();
                    let mut actions = Vec::new();
                    ui.label(
                        RichText::new("Output File")
                            .font(TextRole::ControlLabel.font_id())
                            .color(ds.colors.text_secondary),
                    );
                    ui.horizontal(|ui| {
                        let field_width = (ui.available_width() - 48.0).max(80.0);
                        ds.inset_frame().show(ui, |ui| {
                            ui.set_width(field_width - 16.0);
                            ui.add(egui::Label::new(v.filename).truncate())
                                .on_hover_text(v.filename);
                        });
                        if ui
                            .add(IconButton::new(
                                Lucide::FolderOpen,
                                "Choose export destination",
                            ))
                            .clicked()
                        {
                            actions.push(ExportMixedIrAction::ChooseDestination);
                        }
                    });
                    ui.label(
                        RichText::new(v.metadata)
                            .font(TextRole::Metadata.font_id())
                            .color(ds.colors.text_secondary),
                    );
                    ui.add_space(4.0);

                    selector_row(
                        ui,
                        "Sample Rate",
                        "export_sample_rate",
                        v.sample_rates,
                        v.sample_rate,
                        ExportMixedIrAction::SetSampleRate,
                        &mut actions,
                    );
                    selector_row(
                        ui,
                        "Bit Depth",
                        "export_bit_depth",
                        v.bit_depths,
                        v.bit_depth,
                        ExportMixedIrAction::SetBitDepth,
                        &mut actions,
                    );
                    selector_row(
                        ui,
                        "Channels",
                        "export_channels",
                        v.channel_modes,
                        v.channel_mode,
                        ExportMixedIrAction::SetChannelMode,
                        &mut actions,
                    );

                    let mut trim = v.trim_to_length;
                    ui.add(Checkbox::new(&mut trim, "Trim to Length"));
                    if trim != v.trim_to_length {
                        actions.push(ExportMixedIrAction::SetTrimToLength(trim));
                    }
                    if trim {
                        selector_row(
                            ui,
                            "Length",
                            "export_length",
                            v.lengths,
                            v.length,
                            ExportMixedIrAction::SetLength,
                            &mut actions,
                        );
                    }
                    let mut normalize = v.normalize;
                    ui.add(Checkbox::new(&mut normalize, "Normalize"));
                    if normalize != v.normalize {
                        actions.push(ExportMixedIrAction::SetNormalize(normalize));
                    }
                    ui.add_space(4.0);
                    if ui
                        .add(
                            ActionButton::new(if v.exporting {
                                "Exporting…"
                            } else {
                                "Export IR"
                            })
                            .kind(ButtonKind::Primary)
                            .icon(Lucide::Download)
                            .min_width(ui.available_width())
                            .enabled(!v.exporting),
                        )
                        .clicked()
                    {
                        actions.push(ExportMixedIrAction::Export);
                    }
                    actions
                })
        })
        .inner
    }
}

fn selector_row(
    ui: &mut Ui,
    label: &str,
    id: &str,
    items: &[&str],
    selected: usize,
    action: fn(usize) -> ExportMixedIrAction,
    actions: &mut Vec<ExportMixedIrAction>,
) {
    let mut value = selected;
    ui.add(
        DropdownSelector::new(id, items, &mut value)
            .width(ui.available_width())
            .label(label, SelectorLabelPosition::Top),
    );
    if value != selected {
        actions.push(action(value));
    }
}
