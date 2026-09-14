use egui::{Align2, Color32, Response, Sense, Stroke, Ui, Vec2, Widget, pos2, vec2};

use crate::{DesignSystem, TextRole};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WaveformRenderMode {
    #[default]
    Signed,
    Symmetric,
}

pub struct WaveformView<'a> {
    samples: &'a [f32],
    color: Color32,
    size: Vec2,
    label: Option<&'a str>,
    render_mode: WaveformRenderMode,
}

impl<'a> WaveformView<'a> {
    pub fn new(samples: &'a [f32], color: Color32) -> Self {
        Self {
            samples,
            color,
            size: vec2(180.0, 64.0),
            label: None,
            render_mode: WaveformRenderMode::Signed,
        }
    }

    pub fn size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn render_mode(mut self, render_mode: WaveformRenderMode) -> Self {
        self.render_mode = render_mode;
        self
    }
}

impl Widget for WaveformView<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let ds = DesignSystem::default();
        let (rect, response) = ui.allocate_exact_size(self.size, Sense::hover());
        ui.painter()
            .rect_filled(rect, ds.metrics.radius_control, ds.colors.surface_inset);
        ui.painter().rect_stroke(
            rect,
            ds.metrics.radius_control,
            Stroke::new(1.0, ds.colors.border_control),
            egui::StrokeKind::Inside,
        );
        let content = rect.shrink(7.0);
        ui.painter().line_segment(
            [
                pos2(content.left(), content.center().y),
                pos2(content.right(), content.center().y),
            ],
            Stroke::new(1.0, ds.colors.border_subtle),
        );

        if self.samples.is_empty() {
            ui.painter().text(
                content.center(),
                Align2::CENTER_CENTER,
                "No waveform",
                TextRole::Metadata.font_id(),
                ds.colors.text_muted,
            );
        } else {
            let columns = content.width().max(1.0) as usize;
            match self.render_mode {
                WaveformRenderMode::Signed => {
                    let points = (0..columns).map(|column| {
                        let index = column * self.samples.len() / columns;
                        let sample =
                            self.samples[index.min(self.samples.len() - 1)].clamp(-1.0, 1.0);
                        let x = content.left()
                            + column as f32 / columns.saturating_sub(1).max(1) as f32
                                * content.width();
                        let y = content.center().y - sample * content.height() * 0.45;
                        pos2(x, y)
                    });
                    ui.painter().add(egui::Shape::line(
                        points.collect(),
                        Stroke::new(1.5, self.color),
                    ));
                }
                WaveformRenderMode::Symmetric => {
                    for (column, amplitude) in symmetric_envelope(self.samples, columns)
                        .into_iter()
                        .enumerate()
                    {
                        let x = content.left()
                            + column as f32 / columns.saturating_sub(1).max(1) as f32
                                * content.width();
                        let extent = amplitude * content.height() * 0.45;
                        ui.painter().line_segment(
                            [
                                pos2(x, content.center().y - extent),
                                pos2(x, content.center().y + extent),
                            ],
                            Stroke::new(1.5, self.color),
                        );
                    }
                }
            }
        }

        if let Some(label) = self.label {
            ui.painter().text(
                content.left_top() + vec2(2.0, 1.0),
                Align2::LEFT_TOP,
                label,
                TextRole::GraphLabel.font_id(),
                ds.colors.text_secondary,
            );
        }
        response
    }
}

fn symmetric_envelope(samples: &[f32], columns: usize) -> Vec<f32> {
    if samples.is_empty() || columns == 0 {
        return Vec::new();
    }

    (0..columns)
        .map(|column| {
            let start = column * samples.len() / columns;
            let end = ((column + 1) * samples.len() / columns)
                .max(start + 1)
                .min(samples.len());
            samples[start..end]
                .iter()
                .map(|sample| sample.abs().clamp(0.0, 1.0))
                .fold(0.0_f32, f32::max)
        })
        .collect()
}

pub struct GraphFrame<'a> {
    curves: &'a [(&'a [f32], Color32)],
    size: Vec2,
    empty_label: &'a str,
}

impl<'a> GraphFrame<'a> {
    pub fn new(curves: &'a [(&'a [f32], Color32)]) -> Self {
        Self {
            curves,
            size: vec2(520.0, 220.0),
            empty_label: "No analysis data",
        }
    }

    pub fn size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    pub fn empty_label(mut self, label: &'a str) -> Self {
        self.empty_label = label;
        self
    }
}

impl Widget for GraphFrame<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let ds = DesignSystem::default();
        let (rect, response) = ui.allocate_exact_size(self.size, Sense::hover());
        ui.painter()
            .rect_filled(rect, ds.metrics.radius_control, ds.colors.surface_inset);
        ui.painter().rect_stroke(
            rect,
            ds.metrics.radius_control,
            Stroke::new(1.0, ds.colors.border_control),
            egui::StrokeKind::Inside,
        );
        let plot = egui::Rect::from_min_max(
            rect.left_top() + vec2(42.0, 14.0),
            rect.right_bottom() - vec2(12.0, 28.0),
        );

        for division in 0..=8 {
            let t = division as f32 / 8.0;
            let x = egui::lerp(plot.left()..=plot.right(), t);
            let color = if division % 2 == 0 {
                ds.colors.border_control
            } else {
                ds.colors.border_subtle
            };
            ui.painter().line_segment(
                [pos2(x, plot.top()), pos2(x, plot.bottom())],
                Stroke::new(1.0, color),
            );
        }
        for division in 0..=6 {
            let t = division as f32 / 6.0;
            let y = egui::lerp(plot.top()..=plot.bottom(), t);
            ui.painter().line_segment(
                [pos2(plot.left(), y), pos2(plot.right(), y)],
                Stroke::new(
                    1.0,
                    if division == 3 {
                        ds.colors.border_control
                    } else {
                        ds.colors.border_subtle
                    },
                ),
            );
        }

        if self.curves.iter().all(|(curve, _)| curve.is_empty()) {
            ui.painter().text(
                plot.center(),
                Align2::CENTER_CENTER,
                self.empty_label,
                TextRole::Body.font_id(),
                ds.colors.text_muted,
            );
        } else {
            for (curve, color) in self.curves {
                if curve.len() < 2 {
                    continue;
                }
                let points = curve.iter().enumerate().map(|(index, value)| {
                    let t = index as f32 / (curve.len() - 1) as f32;
                    pos2(
                        egui::lerp(plot.left()..=plot.right(), t),
                        egui::lerp(
                            plot.bottom()..=plot.top(),
                            ((value + 24.0) / 36.0).clamp(0.0, 1.0),
                        ),
                    )
                });
                ui.painter().add(egui::Shape::line(
                    points.collect(),
                    Stroke::new(1.5, *color),
                ));
            }
        }

        for (t, label) in [
            (0.0, "20"),
            (0.25, "100"),
            (0.5, "1k"),
            (0.75, "5k"),
            (1.0, "20k"),
        ] {
            ui.painter().text(
                pos2(
                    egui::lerp(plot.left()..=plot.right(), t),
                    plot.bottom() + 9.0,
                ),
                Align2::CENTER_TOP,
                label,
                TextRole::GraphLabel.font_id(),
                ds.colors.text_secondary,
            );
        }
        ui.painter().text(
            pos2(rect.left() + 8.0, plot.center().y),
            Align2::LEFT_CENTER,
            "dB",
            TextRole::GraphLabel.font_id(),
            ds.colors.text_secondary,
        );
        response
    }
}

pub fn deterministic_waveform(seed: f32, len: usize) -> Vec<f32> {
    (0..len)
        .map(|index| {
            let t = index as f32 / len.max(1) as f32;
            let envelope = (-5.5 * t).exp();
            ((t * (35.0 + seed * 7.0)).sin() * 0.72 + (t * 91.0 + seed).sin() * 0.28) * envelope
        })
        .collect()
}

pub fn deterministic_curve(seed: f32, len: usize) -> Vec<f32> {
    (0..len)
        .map(|index| {
            let t = index as f32 / len.max(1) as f32;
            -4.0 + (t * 13.0 + seed).sin() * 2.2
                - (t * 5.0 + seed * 0.7).cos() * 1.3
                - t.powi(7) * 18.0
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_data_is_deterministic() {
        assert_eq!(
            deterministic_waveform(1.0, 64),
            deterministic_waveform(1.0, 64)
        );
        assert_ne!(deterministic_curve(1.0, 64), deterministic_curve(2.0, 64));
    }

    #[test]
    fn waveform_defaults_to_signed_rendering() {
        assert_eq!(WaveformRenderMode::default(), WaveformRenderMode::Signed);
    }

    #[test]
    fn symmetric_envelope_uses_peak_magnitude_per_column() {
        assert_eq!(
            symmetric_envelope(&[-0.2, 0.8, -1.4, 0.3], 2),
            vec![0.8, 1.0]
        );
        assert!(symmetric_envelope(&[], 4).is_empty());
        assert!(symmetric_envelope(&[0.5], 0).is_empty());
    }
}
