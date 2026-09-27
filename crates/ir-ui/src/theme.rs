use std::sync::Arc;

use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke,
    TextStyle, Vec2, Visuals,
};

const INTER_REGULAR: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
const INTER_MEDIUM: &[u8] = include_bytes!("../assets/fonts/Inter-Medium.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");

const REGULAR: &str = "inter_regular";
const MEDIUM: &str = "inter_medium";
const SEMIBOLD: &str = "inter_semibold";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colors {
    pub surface_canvas: Color32,
    pub surface_toolbar: Color32,
    pub surface_panel: Color32,
    pub surface_inset: Color32,
    pub surface_control: Color32,
    pub surface_hover: Color32,
    pub surface_pressed: Color32,
    pub border_subtle: Color32,
    pub border_control: Color32,
    pub border_strong: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub text_on_accent: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_pressed: Color32,
    pub accent_focus: Color32,
    pub accent_soft: Color32,
    pub status_success: Color32,
    pub status_warning: Color32,
    pub status_danger: Color32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub space_xs: f32,
    pub space_sm: f32,
    pub space_md: f32,
    pub space_lg: f32,
    pub space_xl: f32,
    pub control_compact_height: f32,
    pub control_height: f32,
    pub control_large_height: f32,
    pub radius_control: u8,
    pub radius_card: u8,
    pub radius_dialog: u8,
    pub icon_small: f32,
    pub icon_default: f32,
    pub icon_large: f32,
    pub sliding_pane_width: f32,
    pub sliding_pane_animation_time: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DesignSystem {
    pub colors: Colors,
    pub metrics: Metrics,
}

impl Default for DesignSystem {
    fn default() -> Self {
        Self {
            colors: Colors {
                surface_canvas: rgb(0x07, 0x11, 0x17),
                surface_toolbar: rgb(0x0E, 0x1A, 0x24),
                surface_panel: rgb(0x0E, 0x1F, 0x29),
                surface_inset: rgb(0x0B, 0x16, 0x1D),
                surface_control: rgb(0x17, 0x22, 0x2D),
                surface_hover: rgb(0x1D, 0x2D, 0x3A),
                surface_pressed: rgb(0x10, 0x1B, 0x24),
                border_subtle: rgb(0x20, 0x31, 0x3E),
                border_control: rgb(0x30, 0x48, 0x5A),
                border_strong: rgb(0x41, 0x60, 0x77),
                text_primary: rgb(0xD5, 0xE4, 0xF5),
                text_secondary: rgb(0x9B, 0xB0, 0xC7),
                text_muted: rgb(0x61, 0x76, 0x8C),
                text_on_accent: rgb(0xF7, 0xFB, 0xFF),
                accent: rgb(0x0F, 0x82, 0xEC),
                accent_hover: rgb(0x24, 0x98, 0xFF),
                accent_pressed: rgb(0x08, 0x70, 0xC8),
                accent_focus: rgb(0x54, 0xB6, 0xFF),
                accent_soft: rgb(0x15, 0x39, 0x57),
                status_success: rgb(0x62, 0xD8, 0x89),
                status_warning: rgb(0xE6, 0xB9, 0x4E),
                status_danger: rgb(0xF0, 0x5B, 0x6A),
            },
            metrics: Metrics {
                space_xs: 4.0,
                space_sm: 8.0,
                space_md: 12.0,
                space_lg: 16.0,
                space_xl: 24.0,
                control_compact_height: 28.0,
                control_height: 32.0,
                control_large_height: 44.0,
                radius_control: 4,
                radius_card: 8,
                radius_dialog: 10,
                icon_small: 14.0,
                icon_default: 16.0,
                icon_large: 20.0,
                sliding_pane_width: 460.0,
                sliding_pane_animation_time: 0.24,
            },
        }
    }
}

impl DesignSystem {
    pub fn from_context(ctx: &egui::Context) -> Self {
        ctx.data(|data| {
            data.get_temp::<Self>(egui::Id::new("ir_mixer_design_system"))
                .unwrap_or_default()
        })
    }

    pub fn panel_frame(self) -> egui::Frame {
        egui::Frame::new()
            .fill(self.colors.surface_panel)
            .stroke(Stroke::new(1.0, self.colors.border_subtle))
            .corner_radius(CornerRadius::same(self.metrics.radius_card))
            .inner_margin(Margin::same(self.metrics.space_md as i8))
    }

    pub fn inset_frame(self) -> egui::Frame {
        egui::Frame::new()
            .fill(self.colors.surface_inset)
            .stroke(Stroke::new(1.0, self.colors.border_subtle))
            .corner_radius(CornerRadius::same(self.metrics.radius_control))
            .inner_margin(Margin::same(self.metrics.space_sm as i8))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextRole {
    ProductTitle,
    SectionTitle,
    ControlLabel,
    Body,
    Metadata,
    GraphLabel,
}

impl TextRole {
    pub fn font_id(self) -> FontId {
        let (size, family) = match self {
            Self::ProductTitle => (20.0, named_family(SEMIBOLD)),
            Self::SectionTitle => (15.0, named_family(SEMIBOLD)),
            Self::ControlLabel => (12.0, named_family(MEDIUM)),
            Self::Body => (12.0, named_family(REGULAR)),
            Self::Metadata => (11.0, named_family(REGULAR)),
            Self::GraphLabel => (10.0, named_family(REGULAR)),
        };
        FontId::new(size, family)
    }
}

pub fn install_theme(ctx: &egui::Context) {
    install_theme_with(ctx, DesignSystem::default());
}

pub fn install_theme_with(ctx: &egui::Context, ds: DesignSystem) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("ir_mixer_design_system"), ds));
    install_fonts(ctx);
    let c = ds.colors;
    let m = ds.metrics;
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();

    style.spacing.item_spacing = Vec2::new(m.space_sm, m.space_sm);
    style.spacing.button_padding = Vec2::new(m.space_md, m.space_sm);
    style.spacing.interact_size = Vec2::new(m.control_height, m.control_height);
    style.spacing.slider_width = 120.0;
    style.spacing.combo_width = 140.0;
    style.visuals = Visuals::dark();
    style.visuals.override_text_color = Some(c.text_primary);
    style.visuals.panel_fill = c.surface_canvas;
    style.visuals.window_fill = c.surface_panel;
    style.visuals.extreme_bg_color = c.surface_inset;
    style.visuals.faint_bg_color = c.surface_toolbar;
    style.visuals.code_bg_color = c.surface_inset;
    style.visuals.window_stroke = Stroke::new(1.0, c.border_control);
    style.visuals.window_corner_radius = CornerRadius::same(m.radius_dialog);
    style.visuals.menu_corner_radius = CornerRadius::same(m.radius_card);
    style.visuals.selection.bg_fill = c.accent;
    style.visuals.selection.stroke = Stroke::new(1.0, c.text_on_accent);
    style.visuals.hyperlink_color = c.accent_focus;
    style.visuals.widgets.noninteractive = widget_visual(
        c.surface_panel,
        c.border_subtle,
        c.text_secondary,
        m.radius_control,
    );
    style.visuals.widgets.inactive = widget_visual(
        c.surface_control,
        c.border_control,
        c.text_primary,
        m.radius_control,
    );
    style.visuals.widgets.hovered = widget_visual(
        c.surface_hover,
        c.border_strong,
        c.text_primary,
        m.radius_control,
    );
    style.visuals.widgets.active = widget_visual(
        c.surface_pressed,
        c.accent,
        c.text_primary,
        m.radius_control,
    );
    style.visuals.widgets.open =
        widget_visual(c.accent_soft, c.accent, c.text_primary, m.radius_control);

    style
        .text_styles
        .insert(TextStyle::Body, TextRole::Body.font_id());
    style
        .text_styles
        .insert(TextStyle::Button, TextRole::ControlLabel.font_id());
    style
        .text_styles
        .insert(TextStyle::Heading, TextRole::SectionTitle.font_id());
    style
        .text_styles
        .insert(TextStyle::Small, TextRole::Metadata.font_id());
    style
        .text_styles
        .insert(TextStyle::Monospace, TextRole::Body.font_id());
    style.text_styles.insert(
        TextStyle::Name("product_title".into()),
        TextRole::ProductTitle.font_id(),
    );
    style.text_styles.insert(
        TextStyle::Name("graph_label".into()),
        TextRole::GraphLabel.font_id(),
    );

    ctx.set_theme(egui::Theme::Dark);
    ctx.set_style_of(egui::Theme::Dark, style);
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::empty();
    insert_font(&mut fonts, REGULAR, INTER_REGULAR);
    insert_font(&mut fonts, MEDIUM, INTER_MEDIUM);
    insert_font(&mut fonts, SEMIBOLD, INTER_SEMIBOLD);
    fonts
        .families
        .insert(FontFamily::Proportional, vec![REGULAR.into()]);
    fonts
        .families
        .insert(FontFamily::Monospace, vec![REGULAR.into()]);
    fonts
        .families
        .insert(named_family(REGULAR), vec![REGULAR.into()]);
    fonts
        .families
        .insert(named_family(MEDIUM), vec![MEDIUM.into()]);
    fonts
        .families
        .insert(named_family(SEMIBOLD), vec![SEMIBOLD.into()]);
    ctx.set_fonts(fonts);
}

fn insert_font(fonts: &mut FontDefinitions, name: &str, bytes: &'static [u8]) {
    fonts
        .font_data
        .insert(name.into(), Arc::new(FontData::from_static(bytes)));
}

fn named_family(name: &'static str) -> FontFamily {
    FontFamily::Name(Arc::from(name))
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

fn widget_visual(
    fill: Color32,
    border: Color32,
    foreground: Color32,
    radius: u8,
) -> egui::style::WidgetVisuals {
    egui::style::WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: fill,
        bg_stroke: Stroke::new(1.0, border),
        corner_radius: CornerRadius::same(radius),
        fg_stroke: Stroke::new(1.0, foreground),
        expansion: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_design_system_is_available_to_widgets() {
        let context = egui::Context::default();
        let mut custom = DesignSystem::default();
        custom.colors.accent = Color32::from_rgb(1, 2, 3);
        install_theme_with(&context, custom);
        assert_eq!(DesignSystem::from_context(&context), custom);
    }
}
