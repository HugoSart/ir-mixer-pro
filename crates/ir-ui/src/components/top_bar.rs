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
    pub window_controls: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TopBarAction {
    SelectPreset(usize),
    Save,
    SaveAs,
    Delete,
    OpenSettings,
    BeginWindowDrag,
    MinimizeWindow,
    ToggleMaximizeWindow,
    CloseWindow,
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
            let bar_size = egui::vec2(ui.available_width(), ui.available_height());
            ui.allocate_ui_with_layout(
                bar_size,
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(4.0);
                    ui.add(
                        egui::Image::from_bytes(
                            "bytes://ir-mixer-logo-transparent.png",
                            include_bytes!("../../../../design/logos/transparent.png"),
                        )
                        // Match the visible artwork's native 1.478:1 aspect ratio.
                        .fit_to_exact_size(egui::vec2(53.2, 36.0))
                        .maintain_aspect_ratio(false)
                        .uv(egui::Rect::from_min_max(
                            egui::pos2(114.0 / 1254.0, 282.0 / 1254.0),
                            egui::pos2(1141.0 / 1254.0, 977.0 / 1254.0),
                        ))
                        .alt_text("IR Mixer Pro logo"),
                    );
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        // Optical alignment: the font ascenders make the two-line
                        // block appear higher than its measured rectangle.
                        ui.add_space(8.0);
                        let mut title = egui::text::LayoutJob::default();
                        title.append(
                            self.view.product_name,
                            0.0,
                            egui::TextFormat {
                                font_id: TextRole::ProductTitle.font_id(),
                                color: ds.colors.text_primary,
                                ..Default::default()
                            },
                        );
                        title.append(
                            self.view.version,
                            8.0,
                            egui::TextFormat {
                                font_id: TextRole::Metadata.font_id(),
                                color: ds.colors.text_secondary,
                                valign: egui::Align::Center,
                                ..Default::default()
                            },
                        );
                        ui.label(title);
                        ui.label(
                            RichText::new(self.view.tagline)
                                .font(TextRole::Metadata.font_id())
                                .color(ds.colors.text_secondary),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.view.window_controls {
                            if ui
                                .add(IconButton::new(Lucide::X, "Close").kind(ButtonKind::Ghost))
                                .clicked()
                            {
                                actions.push(TopBarAction::CloseWindow);
                            }
                            if ui
                                .add(
                                    IconButton::new(Lucide::Square, "Maximize or restore")
                                        .kind(ButtonKind::Ghost),
                                )
                                .clicked()
                            {
                                actions.push(TopBarAction::ToggleMaximizeWindow);
                            }
                            if ui
                                .add(
                                    IconButton::new(Lucide::Minus, "Minimize")
                                        .kind(ButtonKind::Ghost),
                                )
                                .clicked()
                            {
                                actions.push(TopBarAction::MinimizeWindow);
                            }
                        }
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
                        if self.view.window_controls {
                            let drag = ui.allocate_response(
                                egui::vec2(ui.available_width(), bar_size.y),
                                egui::Sense::click_and_drag(),
                            );
                            if drag.drag_started() {
                                actions.push(TopBarAction::BeginWindowDrag);
                            }
                            if drag.double_clicked() {
                                actions.push(TopBarAction::ToggleMaximizeWindow);
                            }
                        }
                    });
                },
            );
            actions
        })
        .inner
    }
}
