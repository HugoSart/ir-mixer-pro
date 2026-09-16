use crate::{DesignSystem, TextRole};
use egui::{RichText, Ui};

pub struct StatusBarView<'a> {
    pub message: &'a str,
    pub is_error: bool,
    pub sample_rate: &'a str,
    pub buffer_size: &'a str,
    pub active_irs: usize,
    pub total_irs: usize,
}

pub struct StatusBar<'a> {
    view: &'a StatusBarView<'a>,
}

impl<'a> StatusBar<'a> {
    pub fn new(view: &'a StatusBarView<'a>) -> Self {
        Self { view }
    }

    pub fn show(self, ui: &mut Ui) {
        let ds = DesignSystem::from_context(ui.ctx());
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 18.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                let (dot, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                ui.painter().circle_filled(
                    dot.center(),
                    5.0,
                    if self.view.is_error {
                        ds.colors.status_danger
                    } else {
                        ds.colors.status_success
                    },
                );
                ui.label(
                    RichText::new(self.view.message)
                        .font(TextRole::Metadata.font_id())
                        .color(ds.colors.text_secondary),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} / {} active IRs  |  {}  |  {}",
                            self.view.active_irs,
                            self.view.total_irs,
                            self.view.sample_rate,
                            self.view.buffer_size,
                        ))
                        .font(TextRole::Metadata.font_id())
                        .color(ds.colors.text_muted),
                    );
                });
            },
        );
    }
}
