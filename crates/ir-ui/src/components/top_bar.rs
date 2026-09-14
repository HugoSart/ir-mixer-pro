use crate::{DesignSystem, TextRole, widgets::*};
use egui::{RichText, Ui};
use egui_lucide::Lucide;

pub struct TopBarView<'a> {
    pub product_name: &'a str,
    pub version: &'a str,
    pub tagline: &'a str,
    pub preset_names: &'a [&'a str],
    pub preset: usize,
    pub dirty: bool,
    pub cpu_percent: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TopBarAction {
    SelectPreset(usize),
    Save,
    SaveAs,
    Delete,
    OpenSettings,
}

pub struct TopBar<'a> {
    view: &'a TopBarView<'a>,
    id: egui::Id,
}

impl<'a> TopBar<'a> {
    pub fn new(id: impl egui::AsId, view: &'a TopBarView<'a>) -> Self {
        Self {
            view,
            id: egui::Id::new(id),
        }
    }

    pub fn show(self, ui: &mut Ui) -> Vec<TopBarAction> {
        ui.push_id(self.id, |ui| {
            let ds = DesignSystem::from_context(ui.ctx());
            let mut actions = Vec::new();
            ui.horizontal_centered(|ui| {
                ui.add(
                    Lucide::AudioWaveform
                        .color(ds.colors.accent)
                        .size(ds.metrics.icon_large)
                        .stroke_width(2.2),
                );
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(self.view.product_name)
                                .font(TextRole::ProductTitle.font_id())
                                .color(ds.colors.text_primary),
                        );
                        ui.label(
                            RichText::new(self.view.version)
                                .font(TextRole::Metadata.font_id())
                                .color(ds.colors.text_secondary),
                        );
                    });
                    ui.label(
                        RichText::new(self.view.tagline)
                            .font(TextRole::Metadata.font_id())
                            .color(ds.colors.text_secondary),
                    );
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            IconButton::new(Lucide::Settings, "Open settings")
                                .kind(ButtonKind::Ghost),
                        )
                        .clicked()
                    {
                        actions.push(TopBarAction::OpenSettings);
                    }
                    ui.menu_button("…", |ui| {
                        if ui.add(ActionButton::new("Save preset")).clicked() {
                            actions.push(TopBarAction::Save);
                            ui.close();
                        }
                        if ui.add(ActionButton::new("Save preset as…")).clicked() {
                            actions.push(TopBarAction::SaveAs);
                            ui.close();
                        }
                        if ui.add(ActionButton::new("Delete preset")).clicked() {
                            actions.push(TopBarAction::Delete);
                            ui.close();
                        }
                    });
                    let mut preset = self.view.preset;
                    ui.add(
                        DropdownSelector::new("preset", self.view.preset_names, &mut preset)
                            .width(180.0),
                    );
                    if preset != self.view.preset {
                        actions.push(TopBarAction::SelectPreset(preset));
                    }
                    ui.label(
                        RichText::new(if self.view.dirty {
                            "Presets •"
                        } else {
                            "Presets"
                        })
                        .font(TextRole::ControlLabel.font_id())
                        .color(if self.view.dirty {
                            ds.colors.accent_focus
                        } else {
                            ds.colors.text_secondary
                        }),
                    );
                    ui.label(
                        RichText::new(format!("CPU {:.1}%", self.view.cpu_percent))
                            .font(TextRole::Metadata.font_id())
                            .color(ds.colors.text_muted),
                    );
                });
            });
            actions
        })
        .inner
    }
}
