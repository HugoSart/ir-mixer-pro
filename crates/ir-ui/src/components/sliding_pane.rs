use crate::{
    DesignSystem, TextRole,
    widgets::{ButtonKind, IconButton},
};
use egui::{
    Color32, Context, CornerRadius, Id, Order, Rect, RichText, ScrollArea, Sense, Stroke, Ui,
    UiBuilder, Vec2, WidgetInfo, WidgetType,
};
use egui_lucide::Lucide;

/// Result from rendering a [`SlidingPane`].
pub struct SlidingPaneResponse<R> {
    /// The content result while the pane is mounted. Content is not invoked once fully closed.
    pub inner: Option<R>,
    /// True on the frame where a close control, backdrop click, or Escape requested dismissal.
    pub close_requested: bool,
    /// True once the closing animation has finished and the pane is no longer rendered.
    pub fully_closed: bool,
}

/// A controlled, right-edge overlay surface for focused tools and settings.
///
/// Call [`Self::show`] every frame, including while `open` is false, so egui can initialize and
/// retain the animation state. The caller may release pane-specific state when
/// [`SlidingPaneResponse::fully_closed`] becomes true.
pub struct SlidingPane<'a> {
    id: Id,
    title: &'a str,
    icon: Lucide,
    bounds: Rect,
    width: Option<f32>,
}

impl<'a> SlidingPane<'a> {
    pub fn new(id_salt: impl egui::AsId, title: &'a str, icon: Lucide, bounds: Rect) -> Self {
        Self {
            id: Id::new(id_salt),
            title,
            icon,
            bounds,
            width: None,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(0.0));
        self
    }

    pub fn show<R>(
        self,
        ctx: &Context,
        open: &mut bool,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> SlidingPaneResponse<R> {
        let ds = DesignSystem::from_context(ctx);
        let progress = ctx.animate_bool_with_time_and_easing(
            self.id.with("animation"),
            *open,
            ds.metrics.sliding_pane_animation_time,
            egui::emath::easing::cubic_out,
        );
        let fully_closed = !*open && progress <= f32::EPSILON;
        if fully_closed {
            return SlidingPaneResponse {
                inner: None,
                close_requested: false,
                fully_closed: true,
            };
        }
        if self.bounds.width() <= 0.0 || self.bounds.height() <= 0.0 {
            return SlidingPaneResponse {
                inner: None,
                close_requested: false,
                fully_closed: false,
            };
        }

        let width = self
            .width
            .unwrap_or(ds.metrics.sliding_pane_width)
            .min(self.bounds.width());
        let pane_x = self.bounds.right() - width * progress;
        let pane_rect = Rect::from_min_size(
            egui::pos2(pane_x, self.bounds.top()),
            Vec2::new(width, self.bounds.height()),
        );
        let mut close_requested = false;
        let mut inner = None;

        egui::Area::new(self.id.with("overlay"))
            .order(Order::Foreground)
            .fixed_pos(self.bounds.min)
            .default_size(self.bounds.size())
            .constrain(false)
            .movable(false)
            .show(ctx, |ui| {
                ui.set_min_size(self.bounds.size());
                ui.set_max_size(self.bounds.size());
                ui.set_clip_rect(self.bounds);

                let scrim_alpha = (112.0 * progress).round() as u8;
                ui.painter()
                    .rect_filled(self.bounds, 0.0, Color32::from_black_alpha(scrim_alpha));

                let backdrop_rect = Rect::from_min_max(
                    self.bounds.min,
                    egui::pos2(
                        pane_rect.left().max(self.bounds.left()),
                        self.bounds.bottom(),
                    ),
                );
                if backdrop_rect.is_positive() {
                    let backdrop =
                        ui.interact(backdrop_rect, self.id.with("backdrop"), Sense::click());
                    backdrop.widget_info(|| {
                        WidgetInfo::labeled(WidgetType::Button, true, "Dismiss sliding pane")
                    });
                    close_requested |= backdrop.clicked();
                }

                let mut pane_ui = ui.new_child(
                    UiBuilder::new()
                        .max_rect(pane_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                pane_ui.set_clip_rect(self.bounds);
                let corner_radius = CornerRadius {
                    nw: ds.metrics.radius_card,
                    ne: 0,
                    sw: ds.metrics.radius_card,
                    se: 0,
                };
                let frame = egui::Frame::new()
                    .fill(ds.colors.surface_panel)
                    .stroke(Stroke::new(1.0, ds.colors.border_control))
                    .corner_radius(corner_radius)
                    .shadow(egui::epaint::Shadow {
                        offset: [-4, 0],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(72),
                    })
                    .inner_margin(ds.metrics.space_md);

                let shown = frame.show(&mut pane_ui, |ui| {
                    let content_size = Vec2::new(
                        (width - ds.metrics.space_md * 2.0 - 2.0).max(0.0),
                        (self.bounds.height() - ds.metrics.space_md * 2.0 - 2.0).max(0.0),
                    );
                    ui.set_min_size(content_size);
                    ui.set_max_size(content_size);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = ds.metrics.space_sm;
                        ui.add(
                            self.icon
                                .size(ds.metrics.icon_small)
                                .stroke_width(1.0)
                                .color(ds.colors.text_secondary),
                        );
                        ui.label(
                            RichText::new(self.title)
                                .font(TextRole::SectionTitle.font_id())
                                .color(ds.colors.text_primary),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    IconButton::new(Lucide::X, "Close sliding pane")
                                        .kind(ButtonKind::Ghost),
                                )
                                .clicked()
                            {
                                close_requested = true;
                            }
                        });
                    });
                    ui.separator();
                    ScrollArea::vertical()
                        .id_salt(self.id.with("body"))
                        .auto_shrink([false, false])
                        .show(ui, add_contents)
                        .inner
                });
                inner = Some(shown.inner);
            });

        if *open
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            close_requested = true;
        }
        if close_requested {
            *open = false;
            ctx.request_repaint();
        }

        SlidingPaneResponse {
            inner,
            close_requested,
            fully_closed: false,
        }
    }
}
