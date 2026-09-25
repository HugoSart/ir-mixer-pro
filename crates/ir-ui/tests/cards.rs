use egui_kittest::{Harness, SnapshotResults, kittest::Queryable as _};
use ir_ui::components::*;

struct State {
    mode: SourceMode,
    actions: Vec<InputSourceAction>,
    initialized: bool,
    loading: bool,
}
fn harness(mode: SourceMode, scale: f32) -> Harness<'static, State> {
    harness_with_loading(mode, scale, false)
}

fn harness_with_loading(mode: SourceMode, scale: f32, loading: bool) -> Harness<'static, State> {
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
                    buffer_sizes: &["128 samples", "256 samples"],
                    buffer_size: 0,
                    monitoring: false,
                    standalone_routing: true,
                    browse_enabled: !state.loading,
                    browse_loading: state.loading,
                    content_status: ContentStatusView::Ready,
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
                loading,
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
fn loading_browse_button_is_disabled_and_emits_no_intent() {
    let mut h = harness_with_loading(SourceMode::Preview, 1.0, true);
    h.get_by_label("Loading: Browse preview file").click();
    h.step();
    assert!(h.state().actions.is_empty());
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
                        standalone_routing: true,
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
                        status: ExportStatusView::Idle,
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

struct IrRackState {
    actions: Vec<IrRackAction>,
    initialized: bool,
}

fn ir_rack_harness(scale: f32) -> Harness<'static, IrRackState> {
    Harness::builder()
        .with_size(egui::vec2(960.0, 680.0))
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

                let waveform_a = ir_ui::widgets::deterministic_waveform(0.4, 256);
                let waveform_b = ir_ui::widgets::deterministic_waveform(1.2, 256);
                let slots = [
                    IrRackSlotView {
                        id: 1,
                        number: 1,
                        filename: "York_Mix01.wav",
                        metadata: "48.0 kHz | 24-bit | 2048 samples",
                        waveform: &waveform_a,
                        color: ir_ui::IR_COLORS[0],
                        enabled: true,
                        gain_db: -3.0,
                        balance_percent: 50.0,
                        delay_samples: 0,
                        sample_rate: 48_000.0,
                        pan: 0.0,
                        polarity_inverted: false,
                        normalize: true,
                        soloed: false,
                        muted: false,
                        replace_enabled: true,
                        replace_loading: false,
                        load_status: ContentStatusView::Ready,
                    },
                    IrRackSlotView {
                        id: 2,
                        number: 2,
                        filename: "York_Room.wav",
                        metadata: "48.0 kHz | 24-bit | 2048 samples",
                        waveform: &waveform_b,
                        color: ir_ui::IR_COLORS[1],
                        enabled: true,
                        gain_db: -12.0,
                        balance_percent: 50.0,
                        delay_samples: 6,
                        sample_rate: 48_000.0,
                        pan: 0.1,
                        polarity_inverted: false,
                        normalize: true,
                        soloed: false,
                        muted: false,
                        replace_enabled: true,
                        replace_loading: false,
                        load_status: ContentStatusView::Ready,
                    },
                ];
                state.actions.extend(
                    IrRackCard::new(
                        "rack",
                        &IrRackCardView {
                            slots: &slots,
                            selected: Some(1),
                            add_enabled: true,
                            add_loading: false,
                            balance_mode: false,
                        },
                    )
                    .width(900.0)
                    .rack_height(300.0)
                    .show(ui)
                    .inner,
                );
            },
            IrRackState {
                actions: vec![],
                initialized: false,
            },
        )
}

#[test]
fn ir_rack_emits_actions_without_mutating_slots() {
    let mut h = ir_rack_harness(1.0);
    h.get_by_label("Add IR").click();
    h.run();
    h.get_by_label("Enable York_Mix01.wav").click();
    h.run();
    h.get_by_label("York_Room.wav").click();
    h.run();
    assert_eq!(
        h.state().actions,
        vec![
            IrRackAction::AddIr,
            IrRackAction::SetEnabled {
                id: 1,
                enabled: false,
            },
            IrRackAction::Select { id: 2 },
        ]
    );
}

#[test]
fn ir_rack_snapshots_at_supported_scales() {
    let mut results = SnapshotResults::new();
    for scale in [1, 2] {
        let mut h = ir_rack_harness(scale as f32);
        h.snapshot(format!("ir_rack_{scale}x"));
        results.extend_harness(&mut h);
    }
    results.unwrap();
}

struct AnalysisState {
    actions: Vec<AnalysisPreviewAction>,
    initialized: bool,
}

fn analysis_harness(scale: f32) -> Harness<'static, AnalysisState> {
    Harness::builder()
        .with_size(egui::vec2(960.0, 650.0))
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

                let curves = (0..5)
                    .map(|index| ir_ui::widgets::deterministic_curve(index as f32 * 0.8, 180))
                    .collect::<Vec<_>>();
                let traces = curves
                    .iter()
                    .enumerate()
                    .map(|(index, curve)| AnalysisTraceView {
                        label: [
                            "YAFNM25ROOM YA FDMN 412 M25 ROOM.wav",
                            "IR 2",
                            "IR 3",
                            "IR 4",
                            "Sum (Mixed)",
                        ][index],
                        values: curve,
                        color: if index == 4 {
                            ir_ui::DesignSystem::default().colors.text_primary
                        } else {
                            ir_ui::IR_COLORS[index]
                        },
                        emphasized: index == 4,
                    })
                    .collect::<Vec<_>>();
                let impulse = ir_ui::widgets::deterministic_waveform(0.35, 512);
                let view = AnalysisPreviewCardView {
                    selected_tab: 0,
                    view_modes: &["Magnitude (dB)", "Magnitude (linear)"],
                    view_mode: 0,
                    smoothing_options: &["None", "1/12 Oct", "1/6 Oct"],
                    smoothing: 1,
                    frequency_traces: &traces,
                    impulse_waveform: &impulse,
                    impulse_color: ir_ui::IR_COLORS[0],
                    phase_traces: &traces[..4],
                    spectrum_traces: &traces[..3],
                    sample_rate: "48.0 kHz",
                    ir_length: "2048 samples (42.7 ms)",
                    latency: "5.3 ms",
                    cpu: "2.1%",
                    content_status: ContentStatusView::Ready,
                };
                state.actions.extend(
                    AnalysisPreviewCard::new("analysis", &view)
                        .width(900.0)
                        .graph_height(220.0)
                        .show(ui)
                        .inner,
                );
            },
            AnalysisState {
                actions: vec![],
                initialized: false,
            },
        )
}

#[test]
fn analysis_card_emits_tab_intent_without_mutating_view() {
    let mut h = analysis_harness(1.0);
    h.get_by_label("Phase").click();
    h.run();
    assert_eq!(h.state().actions, vec![AnalysisPreviewAction::SetTab(2)]);
}

#[test]
fn analysis_card_snapshots_at_supported_scales() {
    let mut results = SnapshotResults::new();
    for scale in [1, 2] {
        let mut h = analysis_harness(scale as f32);
        h.snapshot(format!("analysis_preview_{scale}x"));
        results.extend_harness(&mut h);
    }
    results.unwrap();
}
