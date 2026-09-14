#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ContentStatusView<'a> {
    #[default]
    Ready,
    Loading(&'a str),
    Error(&'a str),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ExportStatusView<'a> {
    #[default]
    Idle,
    Exporting {
        progress: f32,
    },
    Complete(&'a str),
    Error(&'a str),
}

pub(crate) fn status_banner(ui: &mut egui::Ui, status: ContentStatusView<'_>) {
    use crate::{DesignSystem, TextRole};
    use egui::RichText;

    let ds = DesignSystem::from_context(ui.ctx());
    let (message, color) = match status {
        ContentStatusView::Ready => return,
        ContentStatusView::Loading(message) => (message, ds.colors.accent_focus),
        ContentStatusView::Error(message) => (message, ds.colors.status_danger),
    };
    ds.inset_frame().show(ui, |ui| {
        ui.label(
            RichText::new(message)
                .font(TextRole::Metadata.font_id())
                .color(color),
        );
    });
}
