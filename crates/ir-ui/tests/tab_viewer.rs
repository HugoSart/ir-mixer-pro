use egui::{RichText, Vec2};
use egui_kittest::{Harness, SnapshotResults, kittest::Queryable as _};
use ir_ui::{DesignSystem, TextRole, widgets::TabViewer};

#[derive(Default)]
struct TabState {
    initialized: bool,
    selected: usize,
}

fn harness(scale: f32) -> Harness<'static, TabState> {
    Harness::builder()
        .with_size(Vec2::new(680.0, 360.0))
        .with_pixels_per_point(scale)
        .build_ui_state(
            |ui, state| {
                if !state.initialized {
                    ir_ui::install(ui.ctx());
                    state.initialized = true;
                    ui.ctx().request_repaint();
                    return;
                }
                let ds = DesignSystem::default();
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(ds.colors.surface_canvas)
                            .inner_margin(16.0),
                    )
                    .show(ui, |ui| {
                        let labels = [
                            "Frequency Response",
                            "Impulse Response",
                            "Phase",
                            "Spectrogram",
                        ];
                        TabViewer::new(&labels, &mut state.selected)
                            .min_content_height(180.0)
                            .show(ui, |ui, active| {
                                ui.label(
                                    RichText::new(format!("Dummy pane {active}"))
                                        .font(TextRole::Body.font_id()),
                                );
                            });
                    });
            },
            TabState::default(),
        )
}

#[test]
fn tab_viewer_updates_the_caller_owned_selection() {
    let mut harness = harness(1.0);
    harness.get_by_label("Phase").click();
    harness.run();
    assert_eq!(harness.state().selected, 2);
}

#[test]
fn tab_viewer_snapshots_at_supported_scales() {
    let mut results = SnapshotResults::new();
    for (scale, name) in [(1.0, "tab_viewer_1x"), (2.0, "tab_viewer_2x")] {
        let mut harness = harness(scale);
        harness.snapshot(name);
        results.extend_harness(&mut harness);
    }
    results.unwrap();
}
