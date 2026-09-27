use egui::{RichText, Vec2};
use egui_kittest::{Harness, SnapshotResults, kittest::Queryable as _};
use egui_lucide::Lucide;
use ir_ui::{
    DesignSystem, TextRole,
    components::SlidingPane,
    widgets::{ActionButton, ButtonKind, Checkbox},
};

#[derive(Debug)]
struct PaneState {
    initialized: bool,
    open: bool,
    option_enabled: bool,
    content_rendered: bool,
    fully_closed: bool,
}

impl Default for PaneState {
    fn default() -> Self {
        Self {
            initialized: false,
            open: false,
            option_enabled: true,
            content_rendered: false,
            fully_closed: true,
        }
    }
}

fn pane_harness(scale: f32, initially_open: bool) -> Harness<'static, PaneState> {
    let state = PaneState {
        open: initially_open,
        ..PaneState::default()
    };
    Harness::builder()
        .with_size(Vec2::new(900.0, 600.0))
        .with_pixels_per_point(scale)
        .build_ui_state(
            |ui, state| {
                if !state.initialized {
                    egui_extras::install_image_loaders(ui.ctx());
                    ir_ui::install(ui.ctx());
                    state.initialized = true;
                    ui.ctx().request_repaint();
                    return;
                }

                state.content_rendered = false;
                let ds = DesignSystem::default();
                let central = egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(ds.colors.surface_canvas)
                            .inner_margin(24.0),
                    )
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Sliding pane showcase")
                                .font(TextRole::ProductTitle.font_id()),
                        );
                        ui.add_space(12.0);
                        if ui
                            .add(
                                ActionButton::new("Open sliding pane")
                                    .kind(ButtonKind::Primary)
                                    .icon(Lucide::PanelRightOpen),
                            )
                            .clicked()
                        {
                            state.open = true;
                        }
                    });

                let open = &mut state.open;
                let option_enabled = &mut state.option_enabled;
                let content_rendered = &mut state.content_rendered;
                let response = SlidingPane::new(
                    "test_sliding_pane",
                    "Sliding Pane",
                    Lucide::SlidersHorizontal,
                    central.response.rect,
                )
                .show(ui.ctx(), open, |ui| {
                    *content_rendered = true;
                    ui.label("Arbitrary pane content");
                    ui.add_space(8.0);
                    ui.add(Checkbox::new(option_enabled, "Enable option"));
                });
                state.fully_closed = response.fully_closed;
            },
            state,
        )
}

#[test]
fn pane_opens_and_close_button_finishes_the_lifecycle() {
    let mut harness = pane_harness(1.0, false);
    harness.get_by_label("Open sliding pane").click();
    harness.run_steps(20);
    assert!(harness.state().open);
    assert!(harness.state().content_rendered);

    harness.get_by_label("Close sliding pane").click();
    harness.run_steps(20);
    assert!(!harness.state().open);
    assert!(harness.state().fully_closed);
    assert!(!harness.state().content_rendered);
}

#[test]
fn pane_dismisses_from_backdrop_and_escape_but_not_from_content() {
    let mut harness = pane_harness(1.0, true);
    harness.get_by_label("Enable option").click();
    harness.run();
    assert!(harness.state().open);
    assert!(!harness.state().option_enabled);

    harness.key_press(egui::Key::Escape);
    harness.run_steps(20);
    assert!(!harness.state().open);
    assert!(harness.state().fully_closed);

    harness.get_by_label("Open sliding pane").click();
    harness.run_steps(20);
    harness.get_by_label("Dismiss sliding pane").click();
    harness.run_steps(20);
    assert!(!harness.state().open);
    assert!(harness.state().fully_closed);
}

#[test]
fn pane_snapshot_at_supported_scales() {
    let mut results = SnapshotResults::new();
    for (scale, name) in [(1.0, "sliding_pane_1x"), (2.0, "sliding_pane_2x")] {
        let mut harness = pane_harness(scale, true);
        harness.snapshot(name);
        results.extend_harness(&mut harness);
    }
    results.unwrap();
}
