use egui::{Button, Color32, CornerRadius, Response, RichText, Stroke, Ui, Vec2, Widget};
use egui_lucide::Lucide;

use crate::{DesignSystem, TextRole};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ButtonKind {
    Primary,
    #[default]
    Secondary,
    Ghost,
}

pub struct ActionButton<'a> {
    label: &'a str,
    kind: ButtonKind,
    icon: Option<Lucide>,
    selected: bool,
    enabled: bool,
    tooltip: Option<&'a str>,
    min_width: f32,
    corner_radius: Option<CornerRadius>,
}

impl<'a> ActionButton<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            kind: ButtonKind::Secondary,
            icon: None,
            selected: false,
            enabled: true,
            tooltip: None,
            min_width: 0.0,
            corner_radius: None,
        }
    }

    pub fn kind(mut self, kind: ButtonKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn icon(mut self, icon: Lucide) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn tooltip(mut self, tooltip: &'a str) -> Self {
        self.tooltip = Some(tooltip);
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    pub fn corner_radius(mut self, corner_radius: CornerRadius) -> Self {
        self.corner_radius = Some(corner_radius);
        self
    }
}

impl Widget for ActionButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let ds = DesignSystem::default();
        let c = ds.colors;
        let active = self.selected || self.kind == ButtonKind::Primary;
        let foreground = if active {
            c.text_on_accent
        } else {
            c.text_primary
        };
        let label = RichText::new(self.label)
            .font(TextRole::ControlLabel.font_id())
            .color(foreground);
        let button = if let Some(icon) = self.icon {
            Button::image_and_text(
                icon.color(foreground)
                    .size(ds.metrics.icon_default)
                    .stroke_width(1.8)
                    .image(),
                label,
            )
        } else {
            Button::new(label)
        }
        .min_size(Vec2::new(self.min_width, ds.metrics.control_height))
        .corner_radius(
            self.corner_radius
                .unwrap_or_else(|| CornerRadius::same(ds.metrics.radius_control)),
        )
        .selected(self.selected);

        let response = ui
            .scope(|ui| {
                let visuals = ui.visuals_mut();
                if active {
                    visuals.widgets.inactive.weak_bg_fill = c.accent;
                    visuals.widgets.inactive.bg_fill = c.accent;
                    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, c.accent);
                    visuals.widgets.hovered.weak_bg_fill = c.accent_hover;
                    visuals.widgets.hovered.bg_fill = c.accent_hover;
                    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, c.accent_hover);
                    visuals.widgets.active.weak_bg_fill = c.accent_pressed;
                    visuals.widgets.active.bg_fill = c.accent_pressed;
                } else if self.kind == ButtonKind::Ghost {
                    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                    visuals.widgets.inactive.bg_stroke = Stroke::NONE;
                }
                ui.add_enabled(self.enabled, button)
            })
            .inner;

        if let Some(tooltip) = self.tooltip {
            response.on_hover_text(tooltip)
        } else {
            response
        }
    }
}

pub struct IconButton<'a> {
    icon: Lucide,
    tooltip: &'a str,
    kind: ButtonKind,
    selected: bool,
    enabled: bool,
}

impl<'a> IconButton<'a> {
    pub fn new(icon: Lucide, tooltip: &'a str) -> Self {
        Self {
            icon,
            tooltip,
            kind: ButtonKind::Secondary,
            selected: false,
            enabled: true,
        }
    }

    pub fn kind(mut self, kind: ButtonKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Widget for IconButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let ds = DesignSystem::default();
        let c = ds.colors;
        let active = self.selected || self.kind == ButtonKind::Primary;
        let foreground = if active {
            c.text_on_accent
        } else {
            c.text_primary
        };
        let image = self
            .icon
            .color(foreground)
            .size(ds.metrics.icon_default)
            .stroke_width(1.8)
            .image()
            .alt_text(self.tooltip);
        let button = Button::image(image)
            .min_size(Vec2::splat(ds.metrics.control_height))
            .corner_radius(ds.metrics.radius_control)
            .selected(self.selected);

        ui.scope(|ui| {
            let visuals = ui.visuals_mut();
            if active {
                visuals.widgets.inactive.weak_bg_fill = c.accent;
                visuals.widgets.inactive.bg_fill = c.accent;
                visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, c.accent);
                visuals.widgets.hovered.weak_bg_fill = c.accent_hover;
                visuals.widgets.hovered.bg_fill = c.accent_hover;
                visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, c.accent_hover);
                visuals.widgets.active.weak_bg_fill = c.accent_pressed;
                visuals.widgets.active.bg_fill = c.accent_pressed;
            } else if self.kind == ButtonKind::Ghost {
                visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                visuals.widgets.inactive.bg_fill = Color32::TRANSPARENT;
                visuals.widgets.inactive.bg_stroke = Stroke::NONE;
            }
            ui.add_enabled(self.enabled, button)
        })
        .inner
        .on_hover_text(self.tooltip)
    }
}

pub struct ChannelToggle<'a> {
    label: &'a str,
    value: &'a mut bool,
    active_color: Color32,
    tooltip: &'a str,
}

impl<'a> ChannelToggle<'a> {
    pub fn new(
        label: &'a str,
        value: &'a mut bool,
        active_color: Color32,
        tooltip: &'a str,
    ) -> Self {
        Self {
            label,
            value,
            active_color,
            tooltip,
        }
    }
}

impl Widget for ChannelToggle<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let ds = DesignSystem::default();
        let c = ds.colors;
        let selected = *self.value;
        let foreground = if selected {
            c.surface_canvas
        } else {
            c.text_primary
        };
        let response = ui
            .scope(|ui| {
                if selected {
                    let visuals = ui.visuals_mut();
                    visuals.widgets.inactive.weak_bg_fill = self.active_color;
                    visuals.widgets.inactive.bg_fill = self.active_color;
                    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, self.active_color);
                    visuals.widgets.hovered.weak_bg_fill = self.active_color.gamma_multiply(1.1);
                }
                ui.add(
                    Button::new(
                        RichText::new(self.label)
                            .font(TextRole::ControlLabel.font_id())
                            .color(foreground),
                    )
                    .min_size(Vec2::splat(ds.metrics.control_compact_height))
                    .corner_radius(ds.metrics.radius_control)
                    .selected(selected),
                )
            })
            .inner
            .on_hover_text(self.tooltip);

        if response.clicked() {
            *self.value = !*self.value;
        }
        response
    }
}

pub struct SegmentedControl<'a> {
    labels: &'a [&'a str],
    selected: &'a mut usize,
}

impl<'a> SegmentedControl<'a> {
    pub fn new(labels: &'a [&'a str], selected: &'a mut usize) -> Self {
        Self { labels, selected }
    }
}

impl Widget for SegmentedControl<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let mut combined: Option<Response> = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let last = self.labels.len().saturating_sub(1);
            let radius = DesignSystem::default().metrics.radius_control;
            for (index, label) in self.labels.iter().enumerate() {
                let corner_radius = segmented_corner_radius(index, last + 1, radius);
                let response = ui.add(
                    ActionButton::new(label)
                        .selected(*self.selected == index)
                        .min_width(104.0)
                        .corner_radius(corner_radius),
                );
                if response.clicked() {
                    *self.selected = index;
                }
                combined = Some(match combined.take() {
                    Some(previous) => previous.union(response),
                    None => response,
                });
            }
        });
        combined.unwrap_or_else(|| ui.allocate_response(Vec2::ZERO, egui::Sense::hover()))
    }
}

fn segmented_corner_radius(index: usize, len: usize, radius: u8) -> CornerRadius {
    match (index, len) {
        (_, 0) => CornerRadius::ZERO,
        (_, 1) => CornerRadius::same(radius),
        (0, _) => CornerRadius {
            nw: radius,
            ne: 0,
            sw: radius,
            se: 0,
        },
        (index, len) if index + 1 == len => CornerRadius {
            nw: 0,
            ne: radius,
            sw: 0,
            se: radius,
        },
        _ => CornerRadius::ZERO,
    }
}

pub fn section_header(ui: &mut Ui, number: u8, title: &str) {
    let ds = DesignSystem::default();
    ui.horizontal(|ui| {
        let badge_size = Vec2::splat(28.0);
        let (rect, _) = ui.allocate_exact_size(badge_size, egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, ds.metrics.radius_card, ds.colors.accent);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            number,
            TextRole::SectionTitle.font_id(),
            ds.colors.text_on_accent,
        );
        ui.label(
            RichText::new(title)
                .font(TextRole::SectionTitle.font_id())
                .color(ds.colors.text_primary),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::segmented_corner_radius;
    use egui::CornerRadius;

    #[test]
    fn segmented_control_rounds_only_exterior_corners() {
        let radius = 4;
        assert_eq!(
            segmented_corner_radius(0, 1, radius),
            CornerRadius::same(radius)
        );
        assert_eq!(
            segmented_corner_radius(0, 3, radius),
            CornerRadius {
                nw: radius,
                ne: 0,
                sw: radius,
                se: 0,
            }
        );
        assert_eq!(segmented_corner_radius(1, 3, radius), CornerRadius::ZERO);
        assert_eq!(
            segmented_corner_radius(2, 3, radius),
            CornerRadius {
                nw: 0,
                ne: radius,
                sw: 0,
                se: radius,
            }
        );
    }
}
