use crate::{DesignSystem, TextRole, widgets::section_header};
use egui::{InnerResponse, RichText, Ui, Vec2};
use egui_lucide::Lucide;

#[derive(Clone, Copy, Default)]
pub enum CardVariant {
    #[default]
    Primary,
    Inset,
}

/// Shared card chrome. Width includes the frame and its padding.
pub struct CardFrame<'a> {
    title: &'a str,
    number: Option<u8>,
    icon: Option<Lucide>,
    width: f32,
    min_height: f32,
    variant: CardVariant,
}

impl<'a> CardFrame<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            number: None,
            icon: None,
            width: 300.0,
            min_height: 0.0,
            variant: CardVariant::Primary,
        }
    }
    pub fn number(mut self, number: u8) -> Self {
        self.number = Some(number);
        self.icon = None;
        self
    }
    pub fn icon(mut self, icon: Lucide) -> Self {
        self.icon = Some(icon);
        self.number = None;
        self
    }
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = height;
        self
    }
    pub fn variant(mut self, variant: CardVariant) -> Self {
        self.variant = variant;
        self
    }
    pub fn show<R>(self, ui: &mut Ui, body: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        self.show_with_header(ui, |_| {}, body)
    }
    pub fn show_with_header<R>(
        self,
        ui: &mut Ui,
        header_actions: impl FnOnce(&mut Ui),
        body: impl FnOnce(&mut Ui) -> R,
    ) -> InnerResponse<R> {
        let ds = DesignSystem::from_context(ui.ctx());
        let (frame, padding) = match self.variant {
            CardVariant::Primary => (ds.panel_frame(), ds.metrics.space_md),
            CardVariant::Inset => (ds.inset_frame(), ds.metrics.space_sm),
        };
        ui.allocate_ui_with_layout(
            Vec2::new(self.width, self.min_height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                frame.show(ui, |ui| {
                    ui.set_width((self.width - 2.0 * padding - 2.0).max(0.0));
                    ui.set_min_height((self.min_height - 2.0 * padding - 2.0).max(0.0));
                    ui.horizontal(|ui| {
                        if let Some(icon) = self.icon {
                            ui.spacing_mut().item_spacing.x = ds.metrics.space_sm;
                            ui.add(
                                icon.size(ds.metrics.icon_small)
                                    .stroke_width(1.0)
                                    .color(ds.colors.text_secondary),
                            );
                            ui.label(
                                RichText::new(self.title)
                                    .font(TextRole::SectionTitle.font_id())
                                    .color(ds.colors.text_primary),
                            );
                        } else if let Some(number) = self.number {
                            section_header(ui, number, self.title);
                        } else {
                            ui.label(
                                RichText::new(self.title).font(TextRole::SectionTitle.font_id()),
                            );
                        }
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            header_actions,
                        );
                    });
                    ui.separator();
                    body(ui)
                })
            },
        )
        .inner
    }
}
