use egui_kittest::{Harness, SnapshotResults, kittest::Queryable as _};
use ir_app::{
    AppCommand, AppSnapshot, AudioBackend, EqualizerTarget, FrontendMode, MockAudioBackend,
};
use ir_ui::app::AppPage;

struct PageState {
    snapshot: AppSnapshot,
    mode: FrontendMode,
    initialized: bool,
    open_equalizer: Option<EqualizerTarget>,
    commands: Vec<AppCommand>,
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
            if let Some(target) = state.open_equalizer.take() {
                match target {
                    EqualizerTarget::Global => ir_ui::app::open_global_equalizer_pane(ui.ctx()),
                    EqualizerTarget::Ir(ir_id) => ir_ui::app::open_equalizer_pane(ui.ctx(), ir_id),
                }
            }
            state.commands.extend(
                AppPage::new(&state.snapshot)
                    .frontend_mode(state.mode)
                    .show(ui),
            );
        },
        PageState {
            snapshot,
            mode,
            initialized: false,
            open_equalizer: None,
            commands: Vec::new(),
        },
    )
}

#[test]
fn equalizer_pane_renders_the_graph_and_selected_band_contract() {
    let backend = MockAudioBackend::default();
    let snapshot = backend.snapshot().clone();
    let mut harness = page_harness_with_snapshot(
        egui::vec2(1536.0, 1024.0),
        FrontendMode::Standalone,
        snapshot,
    );
    let ir_id = harness.state().snapshot.project.ir_slots[0].id;
    harness.state_mut().open_equalizer = Some(EqualizerTarget::Ir(ir_id));
    harness.run();
    harness.get_by_label("Bypass EQ");
    harness.get_by_label("Copy");
    harness.get_by_label("Reset all");
    harness.snapshot("equalizer_pane_wide");
}

#[test]
fn global_equalizer_pane_renders_the_shared_editor_contract() {
    let backend = MockAudioBackend::default();
    let snapshot = backend.snapshot().clone();
    let mut harness = page_harness_with_snapshot(
        egui::vec2(1536.0, 1024.0),
        FrontendMode::Standalone,
        snapshot,
    );
    harness.get_by_label("Global EQ").click();
    harness.run();
    harness.get_by_label("Bypass EQ");
    harness.get_by_label("Copy");
    harness.get_by_label("Reset all");
    harness.event(egui::Event::PointerGone);
    harness.run();
    harness.snapshot("global_equalizer_pane_wide");
}

#[test]
fn equalizer_nodes_handle_wheel_q_and_shift_wheel_gain() {
    let backend = MockAudioBackend::default();
    let snapshot = backend.snapshot().clone();
    let mut harness = page_harness_with_snapshot(
        egui::vec2(1536.0, 1024.0),
        FrontendMode::Standalone,
        snapshot,
    );
    let ir_id = harness.state().snapshot.project.ir_slots[0].id;
    harness.state_mut().open_equalizer = Some(EqualizerTarget::Ir(ir_id));
    harness.run();

    let graph_rect = harness.get_by_label("Equalizer response graph").rect();
    let plot_rect = egui::Rect::from_min_max(
        graph_rect.min + egui::vec2(54.0, 34.0),
        graph_rect.max - egui::vec2(14.0, 28.0),
    );
    let band_position = |band: &ir_app::EqBand| {
        let frequency_t = (band.frequency_hz / ir_eq::MIN_FREQUENCY_HZ).ln()
            / (ir_eq::MAX_FREQUENCY_HZ / ir_eq::MIN_FREQUENCY_HZ).ln();
        let gain = if band.shape.has_gain() {
            band.gain_db
        } else {
            0.0
        };
        egui::pos2(
            egui::lerp(plot_rect.x_range(), frequency_t),
            egui::lerp(plot_rect.y_range(), (12.0 - gain) / 24.0),
        )
    };
    let (slot_id, high_pass_id, high_pass_q, high_pass_position, bell_id, bell_gain, bell_position) = {
        let slot = &harness.state().snapshot.project.ir_slots[0];
        let high_pass = &slot.equalizer.bands[0];
        let bell = &slot.equalizer.bands[1];
        (
            slot.id,
            high_pass.id,
            high_pass.q,
            band_position(high_pass),
            bell.id,
            bell.gain_db,
            band_position(bell),
        )
    };

    harness.event(egui::Event::PointerMoved(high_pass_position));
    harness.run();
    harness.state_mut().commands.clear();
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, 1.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::default(),
    });
    harness.run();
    assert!(harness.state().commands.iter().any(|command| matches!(
        command,
        AppCommand::SetEqBandQ(EqualizerTarget::Ir(ir_id), band_id, q)
            if *ir_id == slot_id && *band_id == high_pass_id && *q > high_pass_q
    )));

    harness.event(egui::Event::PointerMoved(bell_position));
    harness.run();
    harness.state_mut().commands.clear();
    let shift = egui::Modifiers {
        shift: true,
        ..Default::default()
    };
    harness.event_modifiers(
        egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 1.0),
            phase: egui::TouchPhase::Move,
            modifiers: shift,
        },
        shift,
    );
    harness.run();
    assert!(harness.state().commands.iter().any(|command| matches!(
        command,
        AppCommand::SetEqBandGain(EqualizerTarget::Ir(ir_id), band_id, gain)
            if *ir_id == slot_id && *band_id == bell_id && *gain > bell_gain
    )));
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
fn wide_page_without_irs_keeps_the_standard_column_gap() {
    let backend = MockAudioBackend::default();
    let mut snapshot = backend.snapshot().clone();
    snapshot.project.ir_slots.clear();
    snapshot.frequency_traces.clear();
    snapshot.phase_traces.clear();
    snapshot.spectrum_traces.clear();
    snapshot.combined_waveform.clear();

    let mut harness = page_harness_with_snapshot(
        egui::vec2(1536.0, 1024.0),
        FrontendMode::Standalone,
        snapshot,
    );
    harness.snapshot("app_page_empty");
}

#[test]
fn complete_page_renders_loading_and_error_states() {
    let backend = MockAudioBackend::default();
    let mut snapshot = backend.snapshot().clone();
    snapshot.project.source.content_state = ir_app::ContentState::Loading {
        message: "Preparing preview waveform…".into(),
    };
    snapshot.file_load_activity.preview_loading = true;
    snapshot.file_load_activity.adding_ir_count = 1;
    snapshot.file_load_activity.replacing_ir_ids = vec![snapshot.project.ir_slots[0].id];
    snapshot.project.ir_slots[1].load_state = ir_app::ContentState::Error {
        message: "Unsupported WAV encoding".into(),
    };
    snapshot.project.analysis.content_state = ir_app::ContentState::Error {
        message: "Analysis data is unavailable".into(),
    };
    snapshot.status = "Audio device configuration failed".into();
    snapshot.status_is_error = true;
    snapshot.project.export.state = ir_app::ExportState::Exporting { progress: 0.45 };
    let mut harness = page_harness_with_snapshot(
        egui::vec2(1536.0, 1024.0),
        FrontendMode::Standalone,
        snapshot,
    );
    harness.snapshot("app_page_states");
}
