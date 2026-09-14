use std::f32::consts::{PI, TAU};

use egui::{Align2, Color32, DragValue, Response, Sense, Stroke, Ui, Vec2, Widget, pos2, vec2};

use crate::{DesignSystem, TextRole};

pub struct AudioKnob<'a> {
    value: &'a mut f32,
    range: std::ops::RangeInclusive<f32>,
    default: f32,
    label: &'a str,
    suffix: &'a str,
    accent: Color32,
    formatter: fn(f32, &str) -> String,
}

impl<'a> AudioKnob<'a> {
    pub fn new(value: &'a mut f32, range: std::ops::RangeInclusive<f32>, label: &'a str) -> Self {
        let default = range.start().max(0.0).min(*range.end());
        Self {
            value,
            range,
            default,
            label,
            suffix: "",
            accent: DesignSystem::default().colors.accent,
            formatter: format_value,
        }
    }

    pub fn default_value(mut self, value: f32) -> Self {
        self.default = value;
        self
    }

    pub fn suffix(mut self, suffix: &'a str) -> Self {
        self.suffix = suffix;
        self
    }

    pub fn accent(mut self, color: Color32) -> Self {
        self.accent = color;
        self
    }

    pub fn value_formatter(mut self, formatter: fn(f32, &str) -> String) -> Self {
        self.formatter = formatter;
        self
    }
}

impl Widget for AudioKnob<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let ds = DesignSystem::default();
        let size = vec2(64.0, 88.0);
        let (rect, mut response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        response = response.on_hover_text(format!(
            "Drag to adjust · Shift for fine adjustment · Double-click to reset to {:.1}{}",
            self.default, self.suffix
        ));

        if response.double_clicked() {
            *self.value = self.default.clamp(*self.range.start(), *self.range.end());
            response.mark_changed();
        } else if response.dragged() {
            let fine = ui.input(|input| input.modifiers.shift);
            let span = *self.range.end() - *self.range.start();
            let sensitivity = if fine { 0.001 } else { 0.01 };
            *self.value = (*self.value - response.drag_delta().y * span * sensitivity)
                .clamp(*self.range.start(), *self.range.end());
            response.mark_changed();
            ui.ctx().request_repaint();
        }
        if response.clicked() {
            response.request_focus();
        }
        if adjust_with_keyboard(ui, &response, self.value, &self.range) {
            response.mark_changed();
        }
        response.widget_info(|| {
            egui::WidgetInfo::slider(ui.is_enabled(), *self.value as f64, self.label)
        });

        let center = pos2(rect.center().x, rect.top() + 29.0);
        let radius = 22.0;
        let start = PI * 0.75;
        let sweep = PI * 1.5;
        let normalized = ((*self.value - *self.range.start())
            / (*self.range.end() - *self.range.start()))
        .clamp(0.0, 1.0);
        let painter = ui.painter();
        painter.circle_filled(center, radius, ds.colors.surface_inset);
        painter.circle_stroke(center, radius, Stroke::new(2.0, ds.colors.border_control));
        paint_arc(
            painter,
            center,
            radius + 3.0,
            start,
            sweep,
            ds.colors.border_subtle,
            2.0,
        );
        paint_arc(
            painter,
            center,
            radius + 3.0,
            start,
            sweep * normalized,
            self.accent,
            3.0,
        );
        let angle = start + sweep * normalized;
        painter.line_segment(
            [
                center + vec2(angle.cos(), angle.sin()) * 7.0,
                center + vec2(angle.cos(), angle.sin()) * 17.0,
            ],
            Stroke::new(2.0, ds.colors.text_primary),
        );
        painter.text(
            pos2(rect.center().x, rect.bottom() - 25.0),
            Align2::CENTER_CENTER,
            self.label,
            TextRole::Metadata.font_id(),
            ds.colors.text_secondary,
        );
        painter.text(
            pos2(rect.center().x, rect.bottom() - 5.0),
            Align2::CENTER_BOTTOM,
            (self.formatter)(*self.value, self.suffix),
            TextRole::ControlLabel.font_id(),
            ds.colors.text_primary,
        );
        response
    }
}

pub struct PanKnob<'a>(AudioKnob<'a>);

impl<'a> PanKnob<'a> {
    pub fn new(value: &'a mut f32) -> Self {
        Self(
            AudioKnob::new(value, -1.0..=1.0, "Pan")
                .default_value(0.0)
                .value_formatter(format_pan),
        )
    }
}

impl Widget for PanKnob<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.0.ui(ui)
    }
}

pub struct MiniFader<'a> {
    value: &'a mut f32,
    range: std::ops::RangeInclusive<f32>,
    default: f32,
    color: Color32,
}

impl<'a> MiniFader<'a> {
    pub fn new(value: &'a mut f32, range: std::ops::RangeInclusive<f32>, color: Color32) -> Self {
        Self {
            value,
            range,
            default: 0.0,
            color,
        }
    }

    pub fn default_value(mut self, value: f32) -> Self {
        self.default = value;
        self
    }
}

impl Widget for MiniFader<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let ds = DesignSystem::default();
        let desired = vec2(116.0, 24.0);
        let (rect, mut response) = ui.allocate_exact_size(desired, Sense::click_and_drag());
        if response.double_clicked() {
            *self.value = self.default.clamp(*self.range.start(), *self.range.end());
            response.mark_changed();
        } else if response.dragged() && ui.input(|input| input.modifiers.shift) {
            let span = *self.range.end() - *self.range.start();
            *self.value = (*self.value + response.drag_delta().x * span * 0.001)
                .clamp(*self.range.start(), *self.range.end());
            response.mark_changed();
        } else if (response.dragged() || response.clicked())
            && let Some(pointer) = response.interact_pointer_pos()
        {
            set_from_pointer(self.value, &self.range, rect, pointer.x);
            response.mark_changed();
        }
        if response.clicked() {
            response.request_focus();
        }
        if adjust_with_keyboard(ui, &response, self.value, &self.range) {
            response.mark_changed();
        }
        response
            .widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), *self.value as f64, "Level"));

        let track = egui::Rect::from_center_size(rect.center(), vec2(rect.width() - 16.0, 4.0));
        let normalized = ((*self.value - *self.range.start())
            / (*self.range.end() - *self.range.start()))
        .clamp(0.0, 1.0);
        let x = egui::lerp(track.left()..=track.right(), normalized);
        ui.painter()
            .rect_filled(track, 2.0, ds.colors.border_subtle);
        ui.painter().rect_filled(
            egui::Rect::from_min_max(track.left_top(), pos2(x, track.bottom())),
            2.0,
            self.color,
        );
        ui.painter()
            .circle_filled(pos2(x, track.center().y), 7.0, ds.colors.text_primary);
        ui.painter().circle_stroke(
            pos2(x, track.center().y),
            7.0,
            Stroke::new(1.0, ds.colors.border_strong),
        );
        response.on_hover_text("Drag to adjust · Double-click to reset")
    }
}

pub struct DbValueEditor<'a> {
    value: &'a mut f32,
    range: std::ops::RangeInclusive<f32>,
}

impl<'a> DbValueEditor<'a> {
    pub fn new(value: &'a mut f32, range: std::ops::RangeInclusive<f32>) -> Self {
        Self { value, range }
    }
}

impl Widget for DbValueEditor<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.add(
            DragValue::new(self.value)
                .range(self.range)
                .speed(0.1)
                .fixed_decimals(1)
                .suffix(" dB"),
        )
        .on_hover_text("Drag or click to enter a precise value")
    }
}

pub struct SampleDelayEditor<'a> {
    samples: &'a mut i32,
    sample_rate: f32,
    range: std::ops::RangeInclusive<i32>,
}

impl<'a> SampleDelayEditor<'a> {
    pub fn new(samples: &'a mut i32, sample_rate: f32) -> Self {
        Self {
            samples,
            sample_rate,
            range: 0..=4096,
        }
    }

    pub fn range(mut self, range: std::ops::RangeInclusive<i32>) -> Self {
        self.range = range;
        self
    }
}

impl Widget for SampleDelayEditor<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let ds = DesignSystem::default();
        let mut response: Option<Response> = None;
        ui.vertical(|ui| {
            let value_response = ui.add(
                DragValue::new(self.samples)
                    .range(self.range)
                    .speed(1.0)
                    .suffix(" smp"),
            );
            response = Some(value_response);
            let milliseconds = samples_to_milliseconds(*self.samples, self.sample_rate);
            ui.label(
                egui::RichText::new(format!("{milliseconds:.3} ms"))
                    .font(TextRole::Metadata.font_id())
                    .color(ds.colors.text_secondary),
            );
        });
        response.unwrap_or_else(|| ui.allocate_response(Vec2::ZERO, Sense::hover()))
    }
}

pub fn samples_to_milliseconds(samples: i32, sample_rate: f32) -> f32 {
    if sample_rate <= 0.0 {
        0.0
    } else {
        samples as f32 * 1000.0 / sample_rate
    }
}

fn paint_arc(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    start: f32,
    sweep: f32,
    color: Color32,
    width: f32,
) {
    let segments = ((sweep.abs() / TAU) * 48.0).ceil().max(2.0) as usize;
    let points = (0..=segments)
        .map(|index| {
            let t = index as f32 / segments as f32;
            let angle = start + sweep * t;
            center + vec2(angle.cos(), angle.sin()) * radius
        })
        .collect::<Vec<_>>();
    painter.add(egui::Shape::line(points, Stroke::new(width, color)));
}

fn adjust_with_keyboard(
    ui: &Ui,
    response: &Response,
    value: &mut f32,
    range: &std::ops::RangeInclusive<f32>,
) -> bool {
    if !response.has_focus() {
        return false;
    }
    let direction = ui.input(|input| {
        let increase =
            input.key_pressed(egui::Key::ArrowUp) || input.key_pressed(egui::Key::ArrowRight);
        let decrease =
            input.key_pressed(egui::Key::ArrowDown) || input.key_pressed(egui::Key::ArrowLeft);
        increase as i8 - decrease as i8
    });
    if direction != 0 {
        let fine = ui.input(|input| input.modifiers.shift);
        let step = (*range.end() - *range.start()) * if fine { 0.001 } else { 0.01 };
        *value = (*value + direction as f32 * step).clamp(*range.start(), *range.end());
        true
    } else {
        false
    }
}

fn set_from_pointer(
    value: &mut f32,
    range: &std::ops::RangeInclusive<f32>,
    rect: egui::Rect,
    pointer_x: f32,
) {
    let normalized = ((pointer_x - rect.left() - 8.0) / (rect.width() - 16.0)).clamp(0.0, 1.0);
    *value = *range.start() + normalized * (*range.end() - *range.start());
}

fn format_value(value: f32, suffix: &str) -> String {
    format!("{value:.1}{suffix}")
}

fn format_pan(value: f32, _suffix: &str) -> String {
    if value.abs() < 0.005 {
        "C".to_owned()
    } else if value < 0.0 {
        format!("L {:.0}", value.abs() * 100.0)
    } else {
        format!("R {:.0}", value * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_convert_to_milliseconds() {
        assert!((samples_to_milliseconds(7, 48_000.0) - 0.145_833_33).abs() < 0.000_01);
    }

    #[test]
    fn invalid_sample_rate_is_safe() {
        assert_eq!(samples_to_milliseconds(7, 0.0), 0.0);
    }

    #[test]
    fn pan_value_uses_audio_labels() {
        assert_eq!(format_pan(0.0, ""), "C");
        assert_eq!(format_pan(-0.25, ""), "L 25");
        assert_eq!(format_pan(0.75, ""), "R 75");
    }
}
