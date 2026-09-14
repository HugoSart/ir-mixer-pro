use egui::{Align2, Color32, Response, Sense, Stroke, Ui, Vec2, Widget, pos2, vec2};

use crate::{DesignSystem, TextRole};

/// Custom-painted one- or two-channel level meter.
pub struct LevelMeter<'a> {
    levels_db: &'a [f32],
    peaks_db: &'a [f32],
    size: Vec2,
    accent: Option<Color32>,
}

impl<'a> LevelMeter<'a> {
    pub fn new(levels_db: &'a [f32], peaks_db: &'a [f32]) -> Self {
        Self {
            levels_db,
            peaks_db,
            size: vec2(54.0, 190.0),
            accent: None,
        }
    }

    pub fn size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    pub fn accent(mut self, accent: Color32) -> Self {
        self.accent = Some(accent);
        self
    }
}

impl Widget for LevelMeter<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let ds = DesignSystem::from_context(ui.ctx());
        let accent = self.accent.unwrap_or(ds.colors.accent);
        let channel_count = self.levels_db.len().clamp(1, 2);
        let (rect, response) = ui.allocate_exact_size(self.size, Sense::hover());
        ui.painter()
            .rect_filled(rect, ds.metrics.radius_control, ds.colors.surface_canvas);
        ui.painter().rect_stroke(
            rect,
            ds.metrics.radius_control,
            Stroke::new(1.0, ds.colors.border_subtle),
            egui::StrokeKind::Inside,
        );

        let meter_top = rect.top() + 9.0;
        let meter_bottom = rect.bottom() - 23.0;
        let gap = 7.0;
        let inner_width = rect.width() - 18.0;
        let channel_width =
            (inner_width - gap * (channel_count.saturating_sub(1)) as f32) / channel_count as f32;

        for channel in 0..channel_count {
            let left = rect.left() + 9.0 + channel as f32 * (channel_width + gap);
            let meter = egui::Rect::from_min_max(
                pos2(left, meter_top),
                pos2(left + channel_width, meter_bottom),
            );
            ui.painter()
                .rect_filled(meter, 2.0, ds.colors.surface_inset);
            let normalized = db_to_unit(self.levels_db[channel]);
            let fill_top = egui::lerp(meter.bottom()..=meter.top(), normalized);
            let fill = egui::Rect::from_min_max(pos2(meter.left(), fill_top), meter.right_bottom());
            ui.painter().rect_filled(fill, 2.0, accent);

            let peak = self
                .peaks_db
                .get(channel)
                .copied()
                .unwrap_or(self.levels_db[channel]);
            let peak_y = egui::lerp(meter.bottom()..=meter.top(), db_to_unit(peak));
            ui.painter().line_segment(
                [pos2(meter.left(), peak_y), pos2(meter.right(), peak_y)],
                Stroke::new(2.0, ds.colors.text_primary),
            );

            if peak >= 0.0 {
                ui.painter().rect_filled(
                    egui::Rect::from_min_max(
                        meter.left_top(),
                        pos2(meter.right(), meter.top() + 5.0),
                    ),
                    1.0,
                    ds.colors.status_danger,
                );
            }
            ui.painter().text(
                pos2(meter.center().x, rect.bottom() - 11.0),
                Align2::CENTER_CENTER,
                if channel == 0 { "L" } else { "R" },
                TextRole::GraphLabel.font_id(),
                ds.colors.text_secondary,
            );
        }

        response.on_hover_text(format!(
            "Level: {}",
            self.levels_db
                .iter()
                .take(channel_count)
                .map(|level| format!("{level:.1} dB"))
                .collect::<Vec<_>>()
                .join(" / ")
        ))
    }
}

pub fn db_to_unit(db: f32) -> f32 {
    ((db.clamp(-60.0, 6.0) + 60.0) / 66.0).powf(0.72)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_mapping_clamps() {
        assert_eq!(db_to_unit(-100.0), 0.0);
        assert_eq!(db_to_unit(20.0), 1.0);
        assert!(db_to_unit(-6.0) > db_to_unit(-18.0));
    }
}
