use egui::{
    Align2, AsId, ComboBox, Id, Response, RichText, Sense, Stroke, Ui, Vec2, Widget, WidgetInfo,
    WidgetType, pos2,
};

use crate::{DesignSystem, TextRole};

pub struct Checkbox<'a> {
    value: &'a mut bool,
    label: &'a str,
    show_label: bool,
}

impl<'a> Checkbox<'a> {
    pub fn new(value: &'a mut bool, label: &'a str) -> Self {
        Self {
            value,
            label,
            show_label: true,
        }
    }

    pub fn show_label(mut self, show_label: bool) -> Self {
        self.show_label = show_label;
        self
    }
}

impl Widget for Checkbox<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let ds = DesignSystem::from_context(ui.ctx());
        let checkbox_size = 16.0;
        let height = ds.metrics.control_compact_height;
        let label_galley = self.show_label.then(|| {
            ui.painter().layout_no_wrap(
                self.label.to_owned(),
                TextRole::ControlLabel.font_id(),
                ds.colors.text_primary,
            )
        });
        let label_x_offset = height / 2.0 + checkbox_size / 2.0 + ds.metrics.space_sm;
        let width = label_galley
            .as_ref()
            .map_or(height, |galley| label_x_offset + galley.size().x);
        let (rect, mut response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());

        if response.clicked() {
            *self.value = !*self.value;
            response.mark_changed();
        }
        response.widget_info(|| {
            WidgetInfo::selected(
                WidgetType::Checkbox,
                ui.is_enabled(),
                *self.value,
                self.label,
            )
        });

        if ui.is_rect_visible(rect) {
            let enabled = ui.is_enabled();
            let icon_rect = egui::Rect::from_center_size(
                pos2(rect.left() + height / 2.0, rect.center().y),
                Vec2::splat(checkbox_size),
            );
            let (fill, border, check) = if !enabled {
                (
                    ds.colors.surface_control,
                    ds.colors.border_subtle,
                    ds.colors.text_muted,
                )
            } else if *self.value {
                let fill = if response.is_pointer_button_down_on() {
                    ds.colors.accent_pressed
                } else if response.hovered() {
                    ds.colors.accent_hover
                } else {
                    ds.colors.accent
                };
                (fill, fill, ds.colors.text_on_accent)
            } else {
                let fill = if response.is_pointer_button_down_on() {
                    ds.colors.surface_pressed
                } else if response.hovered() {
                    ds.colors.surface_hover
                } else {
                    ds.colors.surface_inset
                };
                let border = if response.hovered() {
                    ds.colors.border_strong
                } else {
                    ds.colors.border_control
                };
                (fill, border, ds.colors.text_on_accent)
            };
            ui.painter().rect(
                icon_rect,
                3,
                fill,
                Stroke::new(1.0, border),
                egui::StrokeKind::Inside,
            );
            if *self.value {
                let left = pos2(icon_rect.left() + 3.5, icon_rect.center().y);
                let middle = pos2(icon_rect.left() + 7.0, icon_rect.bottom() - 4.0);
                let right = pos2(icon_rect.right() - 3.0, icon_rect.top() + 4.0);
                ui.painter()
                    .line_segment([left, middle], Stroke::new(1.8, check));
                ui.painter()
                    .line_segment([middle, right], Stroke::new(1.8, check));
            }
            if response.has_focus() {
                ui.painter().rect_stroke(
                    icon_rect.expand(2.0),
                    4,
                    Stroke::new(2.0, ds.colors.accent_focus),
                    egui::StrokeKind::Outside,
                );
            }
            if let Some(galley) = label_galley {
                ui.painter().galley(
                    pos2(
                        rect.left() + label_x_offset,
                        rect.center().y - galley.size().y / 2.0,
                    ),
                    galley,
                    if enabled {
                        ds.colors.text_primary
                    } else {
                        ds.colors.text_muted
                    },
                );
            }
        }

        if self.show_label {
            response
        } else {
            response.on_hover_text(self.label)
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SelectorLabelPosition {
    #[default]
    None,
    Left,
    Top,
}

pub struct DropdownSelector<'a> {
    id: Id,
    options: &'a [&'a str],
    selected: &'a mut usize,
    label: Option<&'a str>,
    label_position: SelectorLabelPosition,
    width: f32,
    selected_text_max_chars: Option<usize>,
}

impl<'a> DropdownSelector<'a> {
    pub fn new(id_salt: impl AsId, options: &'a [&'a str], selected: &'a mut usize) -> Self {
        Self {
            id: Id::new(id_salt),
            options,
            selected,
            label: None,
            label_position: SelectorLabelPosition::None,
            width: 180.0,
            selected_text_max_chars: None,
        }
    }

    pub fn label(mut self, label: &'a str, position: SelectorLabelPosition) -> Self {
        self.label = Some(label);
        self.label_position = position;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn selected_text_max_chars(mut self, max_chars: usize) -> Self {
        self.selected_text_max_chars = Some(max_chars.max(1));
        self
    }

    fn show_combo(&mut self, ui: &mut Ui) -> Response {
        if self.options.is_empty() {
            return ui
                .add_enabled_ui(false, |ui| {
                    ComboBox::from_id_salt(self.id)
                        .width(self.width)
                        .selected_text("No options")
                        .show_ui(ui, |_| {})
                        .response
                })
                .inner;
        }

        *self.selected = (*self.selected).min(self.options.len() - 1);
        let full_text = self.options[*self.selected];
        let selected_text = self
            .selected_text_max_chars
            .map(|max_chars| truncate_with_ellipsis(full_text, max_chars))
            .unwrap_or_else(|| full_text.to_owned());
        let response = ComboBox::from_id_salt(self.id)
            .width(self.width)
            .selected_text(selected_text)
            .show_index(ui, self.selected, self.options.len(), |index| {
                self.options[index]
            });
        if self
            .selected_text_max_chars
            .is_some_and(|max_chars| full_text.chars().count() > max_chars)
        {
            response.on_hover_text(full_text)
        } else {
            response
        }
    }

    fn styled_label(ui: &mut Ui, label: &str) -> Response {
        ui.label(RichText::new(label).font(TextRole::ControlLabel.font_id()))
    }
}

fn truncate_with_ellipsis(value: &str, max_chars: usize) -> String {
    let count = value.chars().count();
    if count <= max_chars {
        return value.to_owned();
    }
    let visible = max_chars.saturating_sub(1);
    let mut truncated = value.chars().take(visible).collect::<String>();
    truncated.push('…');
    truncated
}

impl Widget for DropdownSelector<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        match (self.label, self.label_position) {
            (Some(label), SelectorLabelPosition::Left) => {
                ui.horizontal(|ui| {
                    let label_response = Self::styled_label(ui, label);
                    label_response.union(self.show_combo(ui))
                })
                .inner
            }
            (Some(label), SelectorLabelPosition::Top) => {
                ui.vertical(|ui| {
                    let label_response = Self::styled_label(ui, label);
                    label_response.union(self.show_combo(ui))
                })
                .inner
            }
            _ => self.show_combo(ui),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ListSelectorItem<'a> {
    pub key: &'a str,
    pub value: &'a str,
}

impl<'a> ListSelectorItem<'a> {
    pub const fn new(key: &'a str, value: &'a str) -> Self {
        Self { key, value }
    }
}

pub struct ListSelector<'a> {
    items: &'a [ListSelectorItem<'a>],
    selected: &'a mut usize,
    on_before_change: Option<&'a mut dyn FnMut(usize, usize) -> bool>,
    width: f32,
}

impl<'a> ListSelector<'a> {
    pub fn new(items: &'a [ListSelectorItem<'a>], selected: &'a mut usize) -> Self {
        Self {
            items,
            selected,
            on_before_change: None,
            width: 260.0,
        }
    }

    pub fn on_before_change(mut self, callback: &'a mut dyn FnMut(usize, usize) -> bool) -> Self {
        self.on_before_change = Some(callback);
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    fn try_select(&mut self, proposed: usize) -> bool {
        let current = *self.selected;
        if proposed == current || proposed >= self.items.len() {
            return false;
        }
        let allowed = self
            .on_before_change
            .as_mut()
            .is_none_or(|callback| callback(current, proposed));
        if allowed {
            *self.selected = proposed;
        }
        allowed
    }
}

impl Widget for ListSelector<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let ds = DesignSystem::from_context(ui.ctx());
        if !self.items.is_empty() {
            *self.selected = (*self.selected).min(self.items.len() - 1);
        }

        let mut combined: Option<Response> = None;
        let mut proposed = None;
        let mut has_focus = false;
        let key_font = TextRole::Metadata.font_id();
        let value_font = TextRole::Body.font_id();
        let widest_key = self
            .items
            .iter()
            .map(|item| {
                ui.painter()
                    .layout_no_wrap(
                        item.key.to_owned(),
                        key_font.clone(),
                        ds.colors.text_secondary,
                    )
                    .size()
                    .x
            })
            .fold(0.0_f32, f32::max);
        let frame_response = ds.inset_frame().show(ui, |ui| {
            ui.set_min_width(self.width);
            for (index, item) in self.items.iter().enumerate() {
                let selected = index == *self.selected;
                let (rect, response) = ui.allocate_exact_size(
                    Vec2::new(self.width, ds.metrics.control_compact_height),
                    Sense::click(),
                );
                response.widget_info(|| {
                    WidgetInfo::selected(
                        WidgetType::SelectableLabel,
                        ui.is_enabled(),
                        selected,
                        format!("{}: {}", item.key, item.value),
                    )
                });
                if response.clicked() {
                    response.request_focus();
                }
                has_focus |= response.has_focus();
                if response.clicked() {
                    proposed = Some(index);
                }
                if ui.is_rect_visible(rect) {
                    let fill = if selected {
                        if response.hovered() {
                            ds.colors.accent_hover
                        } else {
                            ds.colors.accent
                        }
                    } else if response.is_pointer_button_down_on() {
                        ds.colors.surface_pressed
                    } else if response.hovered() {
                        ds.colors.surface_hover
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    ui.painter().rect_filled(rect, 0, fill);
                    if response.has_focus() {
                        ui.painter().rect_stroke(
                            rect.shrink(1.0),
                            0,
                            Stroke::new(2.0, ds.colors.accent_focus),
                            egui::StrokeKind::Inside,
                        );
                    }
                    let key_color = if selected {
                        ds.colors.text_on_accent
                    } else {
                        ds.colors.text_secondary
                    };
                    let value_color = if selected {
                        ds.colors.text_on_accent
                    } else {
                        ds.colors.text_primary
                    };
                    let left = rect.left() + ds.metrics.space_sm;
                    ui.painter().text(
                        pos2(left, rect.center().y),
                        Align2::LEFT_CENTER,
                        item.key,
                        key_font.clone(),
                        key_color,
                    );
                    ui.painter().text(
                        pos2(left + widest_key + ds.metrics.space_md, rect.center().y),
                        Align2::LEFT_CENTER,
                        item.value,
                        value_font.clone(),
                        value_color,
                    );
                }
                combined = Some(match combined.take() {
                    Some(previous) => previous.union(response),
                    None => response,
                });
            }
        });

        if has_focus && !self.items.is_empty() {
            if ui.input(|input| input.key_pressed(egui::Key::ArrowDown)) {
                proposed = Some((*self.selected + 1).min(self.items.len() - 1));
            } else if ui.input(|input| input.key_pressed(egui::Key::ArrowUp)) {
                proposed = Some(self.selected.saturating_sub(1));
            }
        }

        let mut response = combined.unwrap_or(frame_response.response);
        if proposed.is_some_and(|index| self.try_select(index)) {
            response.mark_changed();
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::truncate_with_ellipsis;

    #[test]
    fn selected_text_truncation_preserves_the_character_limit() {
        assert_eq!(
            truncate_with_ellipsis("Voicemeeter Input", 32),
            "Voicemeeter Input"
        );
        let truncated = truncate_with_ellipsis("A very long output device display name", 12);
        assert_eq!(truncated, "A very long…");
        assert_eq!(truncated.chars().count(), 12);
    }
}
