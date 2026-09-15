use crate::{DesignSystem, TextRole, widgets::ActionButton};
use egui::{CornerRadius, Response, RichText, Ui, Vec2};

/// A controlled tab strip with one shared inset content pane.
///
/// The caller owns the selected index and supplies the contents for the active tab.
pub struct TabViewer<'a> {
    labels: &'a [&'a str],
    selected: &'a mut usize,
    min_content_height: f32,
    tab_min_width: f32,
}

impl<'a> TabViewer<'a> {
    pub fn new(labels: &'a [&'a str], selected: &'a mut usize) -> Self {
        Self {
            labels,
            selected,
            min_content_height: 220.0,
            tab_min_width: 128.0,
        }
    }

    pub fn min_content_height(mut self, height: f32) -> Self {
        self.min_content_height = height.max(0.0);
        self
    }

    pub fn tab_min_width(mut self, width: f32) -> Self {
        self.tab_min_width = width.max(0.0);
        self
    }

    pub fn show(self, ui: &mut Ui, content: impl FnOnce(&mut Ui, usize)) -> Response {
        self.show_with_header(ui, |_| {}, content)
    }

    pub fn show_with_header(
        self,
        ui: &mut Ui,
        header_actions: impl FnOnce(&mut Ui),
        content: impl FnOnce(&mut Ui, usize),
    ) -> Response {
        let ds = DesignSystem::from_context(ui.ctx());
        if self.labels.is_empty() {
            *self.selected = 0;
        } else {
            *self.selected = (*self.selected).min(self.labels.len() - 1);
        }

        ui.vertical(|ui| {
            let content_spacing_y = ui.spacing().item_spacing.y;
            ui.spacing_mut().item_spacing.y = 0.0;
            let mut response: Option<Response> = None;
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), 56.0),
                egui::Layout::left_to_right(egui::Align::Max),
                |ui| {
                    ui.spacing_mut().item_spacing.x = ds.metrics.space_xs;
                    for (index, label) in self.labels.iter().enumerate() {
                        let tab_response = ui.add(
                            ActionButton::new(label)
                                .selected(*self.selected == index)
                                .min_width(self.tab_min_width)
                                .corner_radius(tab_corner_radius(ds.metrics.radius_control)),
                        );
                        if tab_response.clicked() {
                            *self.selected = index;
                        }
                        response = Some(match response.take() {
                            Some(previous) => previous.union(tab_response),
                            None => tab_response,
                        });
                    }
                    header_actions(ui);
                },
            );
            let pane_width = ui.available_width();
            let pane_response = ui.allocate_ui_with_layout(
                Vec2::new(
                    pane_width,
                    self.min_content_height + 2.0 * ds.metrics.space_sm + 2.0,
                ),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ds.inset_frame().show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = content_spacing_y;
                        ui.set_width((pane_width - 2.0 * ds.metrics.space_sm - 2.0).max(0.0));
                        ui.set_min_height(self.min_content_height);
                        if self.labels.is_empty() {
                            ui.label(
                                RichText::new("No analysis views")
                                    .font(TextRole::Metadata.font_id())
                                    .color(ds.colors.text_muted),
                            );
                        } else {
                            content(ui, *self.selected);
                        }
                    })
                },
            );
            match response {
                Some(response) => response.union(pane_response.response),
                None => pane_response.response,
            }
        })
        .inner
    }
}

fn tab_corner_radius(radius: u8) -> CornerRadius {
    CornerRadius {
        nw: radius,
        ne: radius,
        sw: 0,
        se: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::tab_corner_radius;
    use egui::CornerRadius;

    #[test]
    fn tabs_round_only_their_top_corners() {
        assert_eq!(
            tab_corner_radius(4),
            CornerRadius {
                nw: 4,
                ne: 4,
                sw: 0,
                se: 0,
            }
        );
    }
}
