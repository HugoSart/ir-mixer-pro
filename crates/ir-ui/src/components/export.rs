use super::{CardFrame, ExportStatusView};
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
    pub status: ExportStatusView<'a>,
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
    min_height: f32,
}

impl<'a> ExportMixedIrCard<'a> {
    pub fn new(id: impl egui::AsId, view: &'a ExportMixedIrCardView<'a>) -> Self {
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

    pub fn show(self, ui: &mut Ui) -> InnerResponse<Vec<ExportMixedIrAction>> {
        ui.push_id(self.id, |ui| {
            CardFrame::new("Export Mixed IR")
                .icon(Lucide::Download)
                .width(self.width)
                .min_height(self.min_height)
                .show(ui, |ui| {
                    let v = self.view;
                    let ds = DesignSystem::from_context(ui.ctx());
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
                    export_status(ui, v.status);
                    if ui
                        .add(
                            ActionButton::new(
                                if matches!(v.status, ExportStatusView::Exporting { .. }) {
                                    "Exporting…"
                                } else {
                                    "Export IR"
                                },
                            )
                            .kind(ButtonKind::Primary)
                            .icon(Lucide::Download)
                            .min_width(ui.available_width())
                            .enabled(!matches!(v.status, ExportStatusView::Exporting { .. })),
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

fn export_status(ui: &mut Ui, status: ExportStatusView<'_>) {
    let ds = DesignSystem::from_context(ui.ctx());
    let (message, color, progress) = match status {
        ExportStatusView::Idle => return,
        ExportStatusView::Exporting { progress } => (
            "Preparing mixed IR…",
            ds.colors.accent_focus,
            Some(progress.clamp(0.0, 1.0)),
        ),
        ExportStatusView::Complete(message) => (message, ds.colors.status_success, None),
        ExportStatusView::Error(message) => (message, ds.colors.status_danger, None),
    };
    ui.label(
        RichText::new(message)
            .font(TextRole::Metadata.font_id())
            .color(color),
    );
    if let Some(progress) = progress {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 3.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, 1.0, ds.colors.surface_control);
        let filled = egui::Rect::from_min_max(
            rect.min,
            egui::pos2(rect.left() + rect.width() * progress, rect.bottom()),
        );
        ui.painter().rect_filled(filled, 1.0, ds.colors.accent);
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
