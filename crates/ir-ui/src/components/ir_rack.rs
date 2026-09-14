use super::CardFrame;
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
}

/// Borrowed data for the reusable N-IR rack.
pub struct IrRackCardView<'a> {
    pub slots: &'a [IrRackSlotView<'a>],
}

#[derive(Clone, Debug, PartialEq)]
pub enum IrRackAction {
    AddIr,
    ClearAll,
    NormalizeAll,
    Browse { id: u64 },
    Remove { id: u64 },
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
        self.width = width.max(640.0);
        self
    }

    pub fn rack_height(mut self, height: f32) -> Self {
        self.rack_height = height.max(120.0);
        self
    }
    pub fn show(self, ui: &mut Ui) -> InnerResponse<Vec<IrRackAction>> {
        ui.push_id(self.id, |ui| {
            let actions = RefCell::new(Vec::new());
            let key = self.id.with("selected");
            let mut selected = ui.ctx().data_mut(|d| d.get_temp::<u64>(key));
            if !self.view.slots.iter().any(|s| Some(s.id) == selected) {
                selected = self.view.slots.first().map(|s| s.id);
            }
            let header_selected = selected;
            CardFrame::new("IRs (Mix up to N IRs)")
                .number(2)
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
                            ui.set_min_width(table_width());
                            let (rect, _) = ui.allocate_exact_size(
                                vec2(table_width(), 28.0),
                                egui::Sense::hover(),
                            );
                            for (i, (label, _)) in COLUMNS.iter().enumerate() {
                                cell(ui, rect, i, |ui| {
                                    ui.label(
                                        RichText::new(*label).font(TextRole::Metadata.font_id()),
                                    );
                                });
                            }
                            ScrollArea::vertical()
                                .id_salt("rows")
                                .max_height(self.rack_height)
                                .show(ui, |ui| {
                                    if self.view.slots.is_empty() {
                                        ui.label("No IRs loaded. Add an IR to begin mixing.");
                                    }
                                    for slot in self.view.slots {
                                        ui.push_id(slot.id, |ui| {
                                            row(ui, slot, &mut selected, &mut actions.borrow_mut())
                                        });
                                    }
                                });
                        });
                    },
                );
            if let Some(id) = selected {
                ui.ctx().data_mut(|d| d.insert_temp(key, id));
            }
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
    ("Level", 124.0),
    ("Pan", 48.0),
    ("Delay", 76.0),
    ("Polarity", 48.0),
    ("Normalize", 66.0),
    ("Solo", 40.0),
    ("Mute", 40.0),
    ("", 40.0),
];
fn table_width() -> f32 {
    COLUMNS.iter().map(|(_, w)| w).sum()
}
fn cell(ui: &mut Ui, row: egui::Rect, column: usize, body: impl FnOnce(&mut Ui)) {
    let offset: f32 = COLUMNS[..column].iter().map(|(_, w)| w).sum();
    let rect = egui::Rect::from_min_size(
        row.min + vec2(offset, 0.0),
        vec2(COLUMNS[column].1, row.height()),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(vec2(4.0, 0.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.set_clip_rect(ui.clip_rect().intersect(rect));
    body(&mut child);
}
fn row(ui: &mut Ui, s: &IrRackSlotView<'_>, selected: &mut Option<u64>, a: &mut Vec<IrRackAction>) {
    let ds = DesignSystem::default();
    let (rect, _) = ui.allocate_exact_size(vec2(table_width(), 68.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0, ds.colors.surface_inset);
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        egui::Stroke::new(1.0, ds.colors.border_subtle),
    );
    let id = s.id;
    cell(ui, rect, 0, |ui| {
        ui.label(s.number.to_string());
    });
    cell(ui, rect, 1, |ui| {
        let mut enabled = s.enabled;
        if ui
            .add(Checkbox::new(&mut enabled, &format!("Enable {}", s.filename)).show_label(false))
            .changed()
        {
            a.push(IrRackAction::SetEnabled { id, enabled });
        }
    });
    cell(ui, rect, 2, |ui| {
        ui.vertical(|ui| {
            if ui
                .add(
                    egui::Label::new(RichText::new(s.filename).color(if *selected == Some(id) {
                        ds.colors.accent_focus
                    } else {
                        ds.colors.text_primary
                    }))
                    .truncate()
                    .sense(egui::Sense::click()),
                )
                .on_hover_text(s.filename)
                .clicked()
            {
                *selected = Some(id);
            }
            ui.add(
                egui::Label::new(
                    RichText::new(s.metadata)
                        .font(TextRole::Metadata.font_id())
                        .color(ds.colors.text_secondary),
                )
                .truncate(),
            )
            .on_hover_text(s.metadata);
        });
    });
    cell(ui, rect, 3, |ui| {
        ui.add(
            WaveformView::new(s.waveform, s.color)
                .size(vec2(112.0, 48.0))
                .render_mode(WaveformRenderMode::Symmetric),
        );
    });
    cell(ui, rect, 4, |ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(format!("{:.1} dB", s.gain_db));
            let mut gain_db = s.gain_db;
            if ui
                .add(MiniFader::new(&mut gain_db, -60.0..=12.0, s.color))
                .changed()
            {
                a.push(IrRackAction::SetGainDb { id, gain_db });
            }
        });
    });
    cell(ui, rect, 5, |ui| {
        let mut pan = s.pan;
        if ui.add(PanKnob::new(&mut pan).small(true)).changed() {
            a.push(IrRackAction::SetPan { id, pan });
        }
    });
    cell(ui, rect, 6, |ui| {
        let mut delay_samples = s.delay_samples;
        if ui
            .add(SampleDelayEditor::new(&mut delay_samples, s.sample_rate))
            .changed()
        {
            a.push(IrRackAction::SetDelaySamples { id, delay_samples });
        }
    });
    cell(ui, rect, 7, |ui| {
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
    cell(ui, rect, 8, |ui| {
        let mut normalize = s.normalize;
        if ui
            .add(Checkbox::new(&mut normalize, "Normalize IR").show_label(false))
            .changed()
        {
            a.push(IrRackAction::SetNormalize { id, normalize });
        }
    });
    cell(ui, rect, 9, |ui| {
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
    cell(ui, rect, 10, |ui| {
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
    cell(ui, rect, 11, |ui| {
        ui.menu_button("…", |ui| {
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
