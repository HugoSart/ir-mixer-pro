use egui_kittest::{Harness, SnapshotResults, kittest::Queryable as _};
use ir_ui::components::*;

struct State {
    mode: SourceMode,
    actions: Vec<InputSourceAction>,
    initialized: bool,
}
fn harness(mode: SourceMode, scale: f32) -> Harness<'static, State> {
    Harness::builder()
        .with_size(egui::vec2(360.0, 650.0))
        .with_pixels_per_point(scale)
        .build_ui_state(
            |ui, state| {
                if !state.initialized {
                    ir_ui::install(ui.ctx());
                    egui_extras::install_image_loaders(ui.ctx());
                    state.initialized = true;
                    ui.ctx().request_repaint();
                    return;
                }
                let waveform = ir_ui::widgets::deterministic_waveform(1.0, 256);
                let view = InputSourceCardView {
                    mode: state.mode,
                    filename: Some("guitar_DI.wav"),
                    metadata: "44.1 kHz | 24-bit | 00:36.423",
                    waveform: &waveform,
                    transport: TransportState::Stopped,
                    elapsed_seconds: 0.0,
                    duration_seconds: 36.423,
                    looping: true,
                    gain_db: 0.0,
                    normalize: true,
                    devices: &["Default", "Interface"],
                    device: 0,
                    channels: &["Input 1", "Input 2"],
                    channel: 0,
                    sample_rates: &["44.1 kHz", "48 kHz"],
                    sample_rate: 0,
                    buffer_sizes: &["128 samples", "256 samples"],
                    buffer_size: 0,
                    monitoring: false,
                };
                state.actions.extend(
                    InputSourceCard::new("card", &view)
                        .width(320.0)
                        .show(ui)
                        .inner,
                );
            },
            State {
                mode,
                actions: vec![],
                initialized: false,
            },
        )
}

#[test]
fn card_emits_intents_without_mutating_view() {
    let mut h = harness(SourceMode::Preview, 1.0);
    h.get_by_label("Browse preview file").click();
    h.run();
    h.get_by_label("Play preview").click();
    h.run();
    h.get_by_label("Loop").click();
    h.run();
    h.get_by_label("Live Input").click();
    h.run();
    assert_eq!(
        h.state().actions,
        vec![
            InputSourceAction::Browse,
            InputSourceAction::Play,
            InputSourceAction::SetLoop(false),
            InputSourceAction::SetMode(SourceMode::Live)
        ]
    );
    assert_eq!(h.state().mode, SourceMode::Preview);
    let mut live = harness(SourceMode::Live, 1.0);
    live.get_by_label("Monitor input").click();
    live.run();
    assert_eq!(
        live.state().actions,
        vec![InputSourceAction::SetMonitoring(true)]
    );
}

#[test]
fn card_snapshots() {
    let mut results = SnapshotResults::new();
    for (mode, name) in [
        (SourceMode::Preview, "input_preview"),
        (SourceMode::Live, "input_live"),
    ] {
        for scale in [1, 2] {
            let mut h = harness(mode, scale as f32);
            h.snapshot(format!("{name}_{scale}x"));
            results.extend_harness(&mut h);
        }
    }
    results.unwrap();
}

struct OutputExportState {
    output_actions: Vec<OutputAction>,
    export_actions: Vec<ExportMixedIrAction>,
    initialized: bool,
}

fn output_export_harness(scale: f32) -> Harness<'static, OutputExportState> {
    Harness::builder()
        .with_size(egui::vec2(700.0, 700.0))
        .with_pixels_per_point(scale)
        .build_ui_state(
            |ui, state| {
                if !state.initialized {
                    ir_ui::install(ui.ctx());
                    egui_extras::install_image_loaders(ui.ctx());
                    state.initialized = true;
                    ui.ctx().request_repaint();
                    return;
                }

                ui.horizontal_top(|ui| {
                    let output = OutputCardView {
                        devices: &["Studio Interface", "USB Output"],
                        device: 0,
                        channels: &["1/2 (Stereo)", "1 (Mono)"],
                        channel: 0,
                        buffer_sizes: &["128 samples", "256 samples"],
                        buffer_size: 0,
                        gain_db: -2.0,
                        bypassed: false,
                        limit_output: true,
                        levels_db: &[-10.0, -12.0],
                        peaks_db: &[-1.8, -2.5],
                    };
                    state.output_actions.extend(
                        OutputCard::new("output", &output)
                            .width(320.0)
                            .show(ui)
                            .inner,
                    );
                    ui.add_space(16.0);
                    let export = ExportMixedIrCardView {
                        filename: "My_Mixed_IR.wav",
                        metadata: "48.0 kHz | 24-bit | WAV",
                        sample_rates: &["44.1 kHz", "48 kHz"],
                        sample_rate: 1,
                        bit_depths: &["16-bit PCM", "24-bit PCM"],
                        bit_depth: 1,
                        channel_modes: &["Mono", "Stereo"],
                        channel_mode: 0,
                        lengths: &["2048 samples", "4096 samples"],
                        length: 0,
                        trim_to_length: true,
                        normalize: true,
                        exporting: false,
                    };
                    state.export_actions.extend(
                        ExportMixedIrCard::new("export", &export)
                            .width(320.0)
                            .show(ui)
                            .inner,
                    );
                });
            },
            OutputExportState {
                output_actions: vec![],
                export_actions: vec![],
                initialized: false,
            },
        )
}

#[test]
fn output_and_export_cards_emit_intents_without_side_effects() {
    let mut h = output_export_harness(1.0);
    h.run();
    h.get_by_label("Limit Output (Prevent Clipping)").click();
    h.run();
    h.get_by_label("Choose export destination").click();
    h.run();
    h.get_by_label("Export IR").click();
    h.run();

    assert_eq!(
        h.state().output_actions,
        vec![OutputAction::SetLimitOutput(false)]
    );
    assert_eq!(
        h.state().export_actions,
        vec![
            ExportMixedIrAction::ChooseDestination,
            ExportMixedIrAction::Export
        ]
    );
}

#[test]
fn output_and_export_snapshots() {
    let mut results = SnapshotResults::new();
    for scale in [1, 2] {
        let mut h = output_export_harness(scale as f32);
        h.snapshot(format!("output_export_{scale}x"));
        results.extend_harness(&mut h);
    }
    results.unwrap();
}
