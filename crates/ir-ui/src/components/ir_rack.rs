use super::{CardFrame, ContentStatusView};
use crate::{DesignSystem, TextRole, widgets::*};
use egui::{Color32, InnerResponse, RichText, ScrollArea, Ui, vec2};
use egui_lucide::Lucide;
use std::cell::RefCell;

/// Borrowed display data for one IR rack row.
pub struct IrRackSlotView<'a> {
    pub id: u64,
    pub number: usize,
    pub filename: &'a str,
    pub metadata: &'a str,
    pub waveform: &'a [f32],
    pub color: Color32,
    pub enabled: bool,
    pub gain_db: f32,
    pub delay_samples: i32,
    pub sample_rate: f32,
    pub pan: f32,
    pub polarity_inverted: bool,
    pub normalize: bool,
    pub soloed: bool,
    pub muted: bool,
    pub load_status: ContentStatusView<'a>,
}

/// Borrowed data for the reusable N-IR rack.
pub struct IrRackCardView<'a> {
    pub slots: &'a [IrRackSlotView<'a>],
    pub selected: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum IrRackAction {
    AddIr,
    ClearAll,
    NormalizeAll,
    Select { id: u64 },
    Browse { id: u64 },
    Remove { id: u64 },
    MoveUp { id: u64 },
    MoveDown { id: u64 },
    SetEnabled { id: u64, enabled: bool },
    SetGainDb { id: u64, gain_db: f32 },
    SetDelaySamples { id: u64, delay_samples: i32 },
    SetPan { id: u64, pan: f32 },
    SetPolarity { id: u64, inverted: bool },
    SetNormalize { id: u64, normalize: bool },
    SetSolo { id: u64, soloed: bool },
    SetMute { id: u64, muted: bool },
}

/// A controlled, vertically scrollable rack for an arbitrary number of IRs.
pub struct IrRackCard<'a> {
    view: &'a IrRackCardView<'a>,
    id: egui::Id,
    width: f32,
    rack_height: f32,
}

impl<'a> IrRackCard<'a> {
    pub fn new(id: impl egui::AsId, view: &'a IrRackCardView<'a>) -> Self {
        Self {
            view,
            id: egui::Id::new(id),
            width: 760.0,
            rack_height: 430.0,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        // The table owns horizontal scrolling, so the card itself must honor the
        // width assigned by the responsive page grid.
        self.width = width.max(280.0);
        self
    }

    pub fn rack_height(mut self, height: f32) -> Self {
        self.rack_height = height.max(120.0);
        self
    }
    pub fn show(self, ui: &mut Ui) -> InnerResponse<Vec<IrRackAction>> {
        ui.push_id(self.id, |ui| {
            let actions = RefCell::new(Vec::new());
            let mut selected = self.view.selected;
            if !self.view.slots.iter().any(|s| Some(s.id) == selected) {
                selected = self.view.slots.first().map(|s| s.id);
            }
            let header_selected = selected;
            CardFrame::new("IRs (Mix up to N IRs)")
                .icon(Lucide::Speaker)
                .width(self.width)
                .min_height(self.rack_height + 80.0)
                .show_with_header(
                    ui,
                    |ui| {
                        if ui
                            .add(
                                ActionButton::new("Normalize All")
                                    .enabled(!self.view.slots.is_empty()),
                            )
                            .clicked()
                        {
                            actions.borrow_mut().push(IrRackAction::NormalizeAll);
                        }
                        if ui
                            .add(
                                ActionButton::new("Clear All").enabled(!self.view.slots.is_empty()),
                            )
                            .clicked()
                        {
                            actions.borrow_mut().push(IrRackAction::ClearAll);
                        }
                        if ui
                            .add(
                                ActionButton::new("Remove")
                                    .enabled(header_selected.is_some())
                                    .tooltip("Remove selected IR"),
                            )
                            .clicked()
                            && let Some(id) = header_selected
                        {
                            actions.borrow_mut().push(IrRackAction::Remove { id });
                        }
                        if ui
                            .add(ActionButton::new("Add IR").icon(Lucide::Plus))
                            .clicked()
                        {
                            actions.borrow_mut().push(IrRackAction::AddIr);
                        }
                    },
                    |ui| {
                        ScrollArea::horizontal().id_salt("columns").show(ui, |ui| {
                            let ds = DesignSystem::from_context(ui.ctx());
                            let minimum_width =
                                table_width() + 2.0 * ROW_HORIZONTAL_PADDING + 2.0 * TABLE_MARGIN;
                            ui.set_min_width(minimum_width.max(ui.available_width()));
                            egui::Frame::new()
                                .stroke(egui::Stroke::new(
                                    1.0,
                                    with_alpha(ds.colors.border_subtle, TABLE_BORDER_ALPHA),
                                ))
                                .inner_margin(egui::Margin::same(TABLE_MARGIN as i8))
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    let outer_width = (table_width()
                                        + 2.0 * ROW_HORIZONTAL_PADDING)
                                        .max(ui.available_width());
                                    let content_width = outer_width - 2.0 * ROW_HORIZONTAL_PADDING;
                                    let column_widths = column_widths(content_width);
                                    ui.set_min_width(outer_width);
                                    let (rect, _) = ui.allocate_exact_size(
                                        vec2(outer_width, 28.0),
                                        egui::Sense::hover(),
                                    );
                                    let header_rect =
                                        rect.shrink2(vec2(ROW_HORIZONTAL_PADDING, 0.0));
                                    for (i, (label, _)) in COLUMNS.iter().enumerate() {
                                        cell(ui, header_rect, &column_widths, i, |ui| {
                                            ui.label(
                                                RichText::new(*label)
                                                    .font(TextRole::Metadata.font_id()),
                                            );
                                        });
                                    }
                                    ScrollArea::vertical()
                                        .id_salt("rows")
                                        .max_height(self.rack_height)
                                        .show(ui, |ui| {
                                            ui.spacing_mut().item_spacing.y = 0.0;
                                            if self.view.slots.is_empty() {
                                                ui.add_space(ROW_HORIZONTAL_PADDING);
                                                ui.label(
                                                    "No IRs loaded. Add an IR to begin mixing.",
                                                );
                                            }
                                            for (index, slot) in self.view.slots.iter().enumerate()
                                            {
                                                ui.push_id(slot.id, |ui| {
                                                    row(
                                                        ui,
                                                        slot,
                                                        RowPosition {
                                                            index,
                                                            count: self.view.slots.len(),
                                                        },
                                                        outer_width,
                                                        &column_widths,
                                                        &mut selected,
                                                        &mut actions.borrow_mut(),
                                                    )
                                                });
                                            }
                                        });
                                });
                        });
                    },
                );
            actions.into_inner()
        })
    }
}

// Header and rows share exactly the same column rectangles.
const COLUMNS: [(&str, f32); 12] = [
    ("#", 24.0),
    ("Enable", 44.0),
    ("IR File", 172.0),
    ("Waveform", 120.0),
    ("Level", 56.0),
    ("Pan", 48.0),
    ("Delay", 76.0),
    ("Polarity", 48.0),
    ("Normalize", 66.0),
    ("Solo", 40.0),
    ("Mute", 40.0),
    ("", 40.0),
];
const TABLE_MARGIN: f32 = 4.0;
const ROW_HORIZONTAL_PADDING: f32 = 8.0;
const TABLE_BORDER_ALPHA: u8 = 72;
const ROW_DIVIDER_ALPHA: u8 = 168;

fn table_width() -> f32 {
    COLUMNS.iter().map(|(_, w)| w).sum()
}

fn column_widths(table_width: f32) -> [f32; COLUMNS.len()] {
    let mut widths = COLUMNS.map(|(_, width)| width);
    let extra = (table_width - self::table_width()).max(0.0);
    widths[2] += extra * 0.55;
    widths[3] += extra * 0.45;
    widths
}

fn cell(
    ui: &mut Ui,
    row: egui::Rect,
    widths: &[f32; COLUMNS.len()],
    column: usize,
    body: impl FnOnce(&mut Ui),
) {
    let offset: f32 = widths[..column].iter().sum();
    let rect = egui::Rect::from_min_size(
        row.min + vec2(offset, 0.0),
        vec2(widths[column], row.height()),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(vec2(6.0, 0.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.set_clip_rect(ui.clip_rect().intersect(rect));
    body(&mut child);
}
fn row(
    ui: &mut Ui,
    s: &IrRackSlotView<'_>,
    position: RowPosition,
    row_width: f32,
    widths: &[f32; COLUMNS.len()],
    selected: &mut Option<u64>,
    a: &mut Vec<IrRackAction>,
) {
    let ds = DesignSystem::from_context(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(row_width, 68.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0, ds.colors.surface_toolbar);
    ui.painter().hline(
        rect.shrink2(vec2(ROW_HORIZONTAL_PADDING, 0.0)).x_range(),
        rect.bottom(),
        egui::Stroke::new(1.0, with_alpha(ds.colors.border_control, ROW_DIVIDER_ALPHA)),
    );
    let content_rect = rect.shrink2(vec2(ROW_HORIZONTAL_PADDING, 0.0));
    let id = s.id;
    cell(ui, content_rect, widths, 0, |ui| {
        ui.label(s.number.to_string());
    });
    cell(ui, content_rect, widths, 1, |ui| {
        let mut enabled = s.enabled;
        if ui
            .add(Checkbox::new(&mut enabled, &format!("Enable {}", s.filename)).show_label(false))
            .changed()
        {
            a.push(IrRackAction::SetEnabled { id, enabled });
        }
    });
    cell(ui, content_rect, widths, 2, |ui| {
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), 34.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if ui
                    .add(
                        egui::Label::new(RichText::new(s.filename).color(
                            if *selected == Some(id) {
                                ds.colors.accent_focus
                            } else {
                                ds.colors.text_primary
                            },
                        ))
                        .truncate()
                        .sense(egui::Sense::click()),
                    )
                    .on_hover_text(s.filename)
                    .clicked()
                    && *selected != Some(id)
                {
                    *selected = Some(id);
                    a.push(IrRackAction::Select { id });
                }
                let (metadata, metadata_color) = match s.load_status {
                    ContentStatusView::Ready => (s.metadata, ds.colors.text_secondary),
                    ContentStatusView::Loading(message) => (message, ds.colors.accent_focus),
                    ContentStatusView::Error(message) => (message, ds.colors.status_danger),
                };
                ui.add(
                    egui::Label::new(
                        RichText::new(metadata)
                            .font(TextRole::Metadata.font_id())
                            .color(metadata_color),
                    )
                    .truncate(),
                )
                .on_hover_text(s.metadata);
            },
        );
    });
    cell(ui, content_rect, widths, 3, |ui| {
        ui.add(
            WaveformView::new(s.waveform, s.color)
                .size(vec2(112.0, 48.0))
                .render_mode(WaveformRenderMode::Symmetric),
        );
    });
    cell(ui, content_rect, widths, 4, |ui| {
        let mut gain_db = s.gain_db;
        if ui
            .add(
                AudioKnob::new(&mut gain_db, -60.0..=12.0, "IR Level")
                    .default_value(0.0)
                    .suffix(" dB")
                    .accent(ds.colors.border_strong)
                    .small(true),
            )
            .changed()
        {
            a.push(IrRackAction::SetGainDb { id, gain_db });
        }
    });
    cell(ui, content_rect, widths, 5, |ui| {
        let mut pan = s.pan;
        if ui.add(PanKnob::new(&mut pan).small(true)).changed() {
            a.push(IrRackAction::SetPan { id, pan });
        }
    });
    cell(ui, content_rect, widths, 6, |ui| {
        let mut delay_samples = s.delay_samples;
        if ui
            .add(SampleDelayEditor::new(&mut delay_samples, s.sample_rate).compact(true))
            .changed()
        {
            a.push(IrRackAction::SetDelaySamples { id, delay_samples });
        }
    });
    cell(ui, content_rect, widths, 7, |ui| {
        let mut inverted = s.polarity_inverted;
        if ui
            .add(ChannelToggle::new(
                "Ø",
                &mut inverted,
                ds.colors.accent,
                "Invert polarity",
            ))
            .clicked()
        {
            a.push(IrRackAction::SetPolarity { id, inverted });
        }
    });
    cell(ui, content_rect, widths, 8, |ui| {
        let mut normalize = s.normalize;
        if ui
            .add(Checkbox::new(&mut normalize, "Normalize IR").show_label(false))
            .changed()
        {
            a.push(IrRackAction::SetNormalize { id, normalize });
        }
    });
    cell(ui, content_rect, widths, 9, |ui| {
        let mut soloed = s.soloed;
        if ui
            .add(ChannelToggle::new(
                "S",
                &mut soloed,
                ds.colors.accent_focus,
                "Solo IR",
            ))
            .clicked()
        {
            a.push(IrRackAction::SetSolo { id, soloed });
        }
    });
    cell(ui, content_rect, widths, 10, |ui| {
        let mut muted = s.muted;
        if ui
            .add(ChannelToggle::new(
                "M",
                &mut muted,
                ds.colors.accent,
                "Mute IR",
            ))
            .clicked()
        {
            a.push(IrRackAction::SetMute { id, muted });
        }
    });
    cell(ui, content_rect, widths, 11, |ui| {
        ui.menu_button("…", |ui| {
            if ui
                .add_enabled(position.index > 0, ActionButton::new("Move up"))
                .clicked()
            {
                a.push(IrRackAction::MoveUp { id });
                ui.close();
            }
            if ui
                .add_enabled(
                    position.index + 1 < position.count,
                    ActionButton::new("Move down"),
                )
                .clicked()
            {
                a.push(IrRackAction::MoveDown { id });
                ui.close();
            }
            if ui.add(ActionButton::new("Replace IR file")).clicked() {
                a.push(IrRackAction::Browse { id });
                ui.close();
            }
            if ui.add(ActionButton::new("Remove IR")).clicked() {
                a.push(IrRackAction::Remove { id });
                ui.close();
            }
        });
    });
}

#[derive(Clone, Copy)]
struct RowPosition {
    index: usize,
    count: usize,
}

fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}
