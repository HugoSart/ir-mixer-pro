use egui_kittest::{Harness, SnapshotResults, kittest::Queryable as _};
use ir_app::{AppSnapshot, AudioBackend, FrontendMode, MockAudioBackend};
use ir_ui::app::AppPage;

struct PageState {
    snapshot: AppSnapshot,
    mode: FrontendMode,
    initialized: bool,
}

fn page_harness(size: egui::Vec2, mode: FrontendMode) -> Harness<'static, PageState> {
    let backend = MockAudioBackend::default();
    let mut snapshot = backend.snapshot().clone();
    if mode == FrontendMode::Plugin {
        snapshot.project.source.mode = ir_app::SourceMode::Live;
    }
    page_harness_with_snapshot(size, mode, snapshot)
}

fn page_harness_with_snapshot(
    size: egui::Vec2,
    mode: FrontendMode,
    snapshot: AppSnapshot,
) -> Harness<'static, PageState> {
    Harness::builder().with_size(size).build_ui_state(
        |ui, state| {
            if !state.initialized {
                ir_ui::install(ui.ctx());
                egui_extras::install_image_loaders(ui.ctx());
                state.initialized = true;
                ui.ctx().request_repaint();
                return;
            }
            AppPage::new(&state.snapshot)
                .frontend_mode(state.mode)
                .show(ui);
        },
        PageState {
            snapshot,
            mode,
            initialized: false,
        },
    )
}

#[test]
fn plugin_mode_replaces_standalone_device_routing() {
    let harness = page_harness(egui::vec2(1120.0, 900.0), FrontendMode::Plugin);
    harness.get_by_label("Audio input and buffer configuration are provided by the plugin host.");
    harness.get_by_label("Audio output is routed by the plugin host.");
}

#[test]
fn complete_page_snapshots_cover_responsive_layouts() {
    let mut results = SnapshotResults::new();
    for (name, size) in [
        ("app_page_wide", egui::vec2(1536.0, 1024.0)),
        ("app_page_medium", egui::vec2(1120.0, 900.0)),
        ("app_page_minimum", egui::vec2(880.0, 760.0)),
    ] {
        let mut harness = page_harness(size, FrontendMode::Standalone);
        harness.snapshot(name);
        results.extend_harness(&mut harness);
    }
    results.unwrap();
}

#[test]
fn complete_page_renders_loading_and_error_states() {
    let backend = MockAudioBackend::default();
    let mut snapshot = backend.snapshot().clone();
    snapshot.project.source.content_state = ir_app::ContentState::Loading {
        message: "Preparing preview waveform…".into(),
    };
    snapshot.project.ir_slots[1].load_state = ir_app::ContentState::Error {
        message: "Unsupported WAV encoding".into(),
    };
    snapshot.project.analysis.content_state = ir_app::ContentState::Error {
        message: "Analysis data is unavailable".into(),
    };
    snapshot.project.export.state = ir_app::ExportState::Exporting { progress: 0.45 };
    let mut harness = page_harness_with_snapshot(
        egui::vec2(1536.0, 1024.0),
        FrontendMode::Standalone,
        snapshot,
    );
    harness.snapshot("app_page_states");
}
